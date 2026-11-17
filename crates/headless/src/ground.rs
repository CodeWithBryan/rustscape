//! Shared logical item observations; no mesh, graphics device or audio engine.
use rs09_client_data::{
    item::{Catalog, ItemType},
    pack::Pack,
};
use rs09_protocol::{
    Result,
    game::{
        Frame,
        ground::{self, Context, Info},
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io, path::Path, sync::Arc};

/// Precomputed once, then shared immutably by every client in this process.
#[derive(Debug)]
pub struct Definitions {
    values: Vec<Option<Info>>,
    default: Info,
    sha256: String,
    source: String,
    pack_fallbacks: BTreeSet<i32>,
}
impl Definitions {
    pub fn load(path: &Path) -> io::Result<Self> {
        Self::decode(std::fs::read(path)?)
    }
    fn decode(bytes: Vec<u8>) -> io::Result<Self> {
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let pack = Pack::decode(bytes).map_err(io::Error::other)?;
        let mut ids = Vec::new();
        for name in pack.manifest.files.keys() {
            if let Some(id) = name.strip_prefix("item/") {
                let id: u16 = id
                    .parse()
                    .map_err(|_| io::Error::other("item asset ID outside ground packet range"))?;
                ids.push(i32::from(id));
            }
        }
        if ids.is_empty() {
            return Err(io::Error::other("ground pack has no item definitions"));
        }
        let mut catalog = Catalog::default();
        let mut values = vec![None; ids.iter().max().unwrap().to_owned() as usize + 1];
        for id in ids {
            let item = catalog.get(&pack, id).map_err(io::Error::other)?;
            values[id as usize] = Some(Info {
                cost: item.cost,
                stackable: item.stackable,
            });
        }
        let default = ItemType::default();
        Ok(Self {
            values,
            default: Info {
                cost: default.cost,
                stackable: default.stackable,
            },
            sha256,
            source: pack.manifest.source,
            pack_fallbacks: catalog.fallbacks,
        })
    }
    fn get(&self, id: i32) -> Option<Info> {
        usize::try_from(id)
            .ok()
            .and_then(|id| self.values.get(id))
            .copied()
            .flatten()
    }
}

struct Services<'a> {
    definitions: &'a Definitions,
    observed_fallbacks: &'a mut BTreeSet<i32>,
}
impl Context for Services<'_> {
    fn info(&mut self, _: &mut ground::State, id: i32) -> Result<Info> {
        Ok(self.definitions.get(id).unwrap_or_else(|| {
            self.observed_fallbacks.insert(id);
            self.definitions.default
        }))
    }
    fn height(&mut self, _: &mut ground::State, _: i32, _: i32, _: i32) -> Result<i32> {
        // Rendering is deliberately absent. This value is never reported as a
        // measured terrain height, and cannot affect list/pile type selection.
        Ok(0)
    }
}

pub struct Observer {
    state: ground::State,
    definitions: Arc<Definitions>,
    observed_fallbacks: BTreeSet<i32>,
    frames: usize,
}
impl Observer {
    pub fn new(definitions: Arc<Definitions>) -> Self {
        Self {
            state: ground::State::default(),
            definitions,
            observed_fallbacks: BTreeSet::new(),
            frames: 0,
        }
    }
    pub fn frame(&mut self, frame: &Frame, plane: i32, self_id: i32) -> Result<()> {
        self.state.plane = plane;
        self.state.self_id = self_id;
        self.state.apply_frame(
            frame,
            &mut Services {
                definitions: &self.definitions,
                observed_fallbacks: &mut self.observed_fallbacks,
            },
        )?;
        self.state.take_dirty();
        self.frames += 1;
        Ok(())
    }
    pub fn rebuild(&mut self, delta: [i32; 2], plane: i32, self_id: i32) -> Result<()> {
        self.state.rebase_stacks(delta);
        self.state.plane = plane;
        self.state.self_id = self_id;
        self.state.clear_rendered();
        let mut context = Services {
            definitions: &self.definitions,
            observed_fallbacks: &mut self.observed_fallbacks,
        };
        for tile in self.state.occupied() {
            if tile.plane == plane {
                self.state.refresh(tile.x, tile.y, &mut context)?;
            }
        }
        self.state.take_dirty();
        Ok(())
    }
    pub fn snapshot(&self, base: [i32; 2]) -> Value {
        let item = |item: &ground::ItemRef| {
            let value = item.get();
            json!({"id":value.id,"amount":value.amount})
        };
        let tiles:Vec<_>=self.state.occupied().into_iter().map(|tile|json!({
            "location":[base[0]+tile.x,base[1]+tile.y,tile.plane],
            "items":self.state.stack(tile).unwrap().unwrap().items().iter().map(item).collect::<Vec<_>>(),
            "selectedTypes":self.state.pile(tile).map(|pile|[Some(&pile.primary),pile.secondary.as_ref(),pile.tertiary.as_ref()].map(|entry|entry.map(item))),
        })).collect();
        json!({
            "scope":"remembered local scene ground items; original logical order/selection, no rendering or terrain height",
            "frames":self.frames,"tiles":tiles,
            "metadata":{"packSha256":self.definitions.sha256,"source":self.definitions.source,"definitions":self.definitions.values.iter().filter(|entry|entry.is_some()).count(),"packFallbacks":self.definitions.pack_fallbacks,"observedFallbacks":self.observed_fallbacks}
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::game::updates;

    fn definitions() -> Definitions {
        // Ordinary item, note, and template use the shared production ObjType
        // decoder and its unchanged certificate transfer, rather than metadata
        // constants in the observer. Other asset bytes are never interpreted.
        let raw = [
            vec![12, 0, 0, 0, 25, 0],
            vec![97, 0, 0, 98, 0, 2, 0],
            vec![0],
        ];
        let files: serde_json::Map<_, _> = raw
            .iter()
            .enumerate()
            .scan(0, |offset, (id, data)| {
                let entry = (format!("item/{id}"), json!([*offset, data.len()]));
                *offset += data.len();
                Some(entry)
            })
            .collect();
        let manifest =
            serde_json::to_vec(&json!({"source":"test cache definitions","files":files})).unwrap();
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&manifest);
        for data in raw {
            bytes.extend(data);
        }
        Definitions::decode(bytes).unwrap()
    }

    #[test]
    fn shared_cache_metadata_drives_real_packets_rebase_and_network_owner_filter() {
        let definitions = Arc::new(definitions());
        assert_eq!(
            definitions.get(0),
            Some(Info {
                cost: 25,
                stackable: 0
            })
        );
        assert_eq!(
            definitions.get(1),
            Some(Info {
                cost: 25,
                stackable: 1
            })
        );
        let mut a = Observer::new(definitions.clone());
        let mut b = Observer::new(definitions.clone());
        assert!(Arc::ptr_eq(&a.definitions, &b.definitions));
        let frames = [
            updates::ground_add(2, 3, 0, 1),
            updates::ground_add_excluding(2, 3, 1, 10, 7),
        ];
        let frame = updates::zone_enclosed(8, 16, &frames).unwrap();
        a.frame(&frame, 2, 7).unwrap();
        b.frame(&frame, 2, 8).unwrap();
        assert_eq!(
            a.snapshot([3200, 3200])["tiles"][0]["items"],
            json!([{"id":0,"amount":1}])
        );
        let snapshot = b.snapshot([3200, 3200]);
        assert_eq!(snapshot["tiles"][0]["location"], json!([3210, 3219, 2]));
        assert_eq!(
            snapshot["tiles"][0]["items"],
            json!([{"id":1,"amount":10},{"id":0,"amount":1}])
        );
        b.rebuild([8, 8], 2, 8).unwrap();
        assert_eq!(
            b.snapshot([3208, 3208])["tiles"][0]["location"],
            snapshot["tiles"][0]["location"]
        );
        b.frame(&updates::ground_add(0, 0, 65535, 1), 2, 8).unwrap();
        assert_eq!(
            b.snapshot([3208, 3208])["metadata"]["observedFallbacks"],
            json!([65535])
        );
        assert!(
            a.snapshot([3200, 3200])["metadata"]["observedFallbacks"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn non_item_and_malformed_asset_packs_fail_before_starting_clients() {
        assert!(Definitions::decode(vec![]).is_err());
        let manifest = br#"{"source":"empty","files":{}}"#;
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
        bytes.extend_from_slice(manifest);
        assert!(
            Definitions::decode(bytes)
                .unwrap_err()
                .to_string()
                .contains("no item definitions")
        );
    }

    #[tokio::test]
    async fn live_tcp_observer_consumes_coalesced_login_ground_and_player_frames() {
        use rs09_protocol::{
            buffer::Writer,
            game::Direction,
            login::{self, AuthResponse},
        };
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
            time::{Duration, timeout},
        };
        timeout(Duration::from_secs(5), async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut handshake = [0; 2];
                stream.read_exact(&mut handshake).await.unwrap();
                assert_eq!(handshake.as_slice(), login::handshake(b"groundtest"));
                stream.write_all(&[0; 9]).await.unwrap();
                let mut header = [0; 3];
                stream.read_exact(&mut header).await.unwrap();
                assert_eq!(header[0], 16);
                let mut request = vec![0; u16::from_be_bytes([header[1], header[2]]) as usize];
                stream.read_exact(&mut request).await.unwrap();
                // Explicit successful-auth transport fixture. Password/RSA/
                // account admission behavior is covered by separate oracles.
                let mut bytes = login::reply(AuthResponse::Success, 0, 7, 0);
                let mut rebuild = Writer::new();
                rebuild.p2add(50);
                rebuild.p1sub(2);
                rebuild.p2(406);
                rebuild.p2add(406);
                rebuild.p2add(51);
                let mut next_rebuild = Writer::new();
                next_rebuild.p2add(42);
                next_rebuild.p1sub(2);
                next_rebuild.p2(407);
                next_rebuild.p2add(407);
                next_rebuild.p2add(43);
                let ground = updates::zone_enclosed(
                    48,
                    48,
                    &[
                        updates::ground_add(2, 3, 0, 1),
                        updates::ground_add_excluding(2, 3, 1, 10, 7),
                        updates::ground_add_excluding(2, 3, 1, 10, 8),
                        updates::ground_add(2, 3, 65535, 1),
                    ],
                )
                .unwrap();
                for frame in [
                    Frame {
                        opcode: 162,
                        payload: rebuild.into_vec(),
                    },
                    ground,
                    Frame {
                        opcode: 225,
                        payload: vec![0, 127, 240],
                    },
                    Frame {
                        opcode: 162,
                        payload: next_rebuild.into_vec(),
                    },
                    Frame {
                        opcode: 225,
                        payload: vec![0, 127, 240],
                    },
                ] {
                    frame
                        .write(Direction::ServerToClient, None, &mut bytes)
                        .unwrap();
                }
                stream.write_all(&bytes).await.unwrap();
                let mut end = [0];
                assert_eq!(stream.read(&mut end).await.unwrap(), 0);
            });
            let observation = crate::world::observe(
                address,
                false,
                "groundtest".into(),
                "testing".into(),
                crate::world::Options {
                    scenery: vec![],
                    updates: 2,
                    walk: None,
                    npc_stream: false,
                    ground: Some(Arc::new(definitions())),
                },
            )
            .await
            .unwrap();
            assert_eq!(observation["networkPlayerIndex"], 7);
            assert_eq!(observation["self"]["location"], json!([3250, 3251, 2]));
            // LoginManager shifts old queues, then calls teleport(false), even
            // when the received tile equals the shifted current head.
            assert_eq!(observation["self"]["queuedMoves"], 1);
            let ground = &observation["groundItems"];
            assert_eq!(ground["tiles"][0]["location"], json!([3250, 3251, 2]));
            assert_eq!(
                ground["tiles"][0]["items"],
                json!([{"id":1,"amount":10},{"id":0,"amount":1},{"id":65535,"amount":1}])
            );
            assert_eq!(ground["metadata"]["observedFallbacks"], json!([65535]));
            assert_eq!(ground["frames"], 1);
            server.await.unwrap();
        })
        .await
        .expect("owned observer transport test timed out");
    }
}
