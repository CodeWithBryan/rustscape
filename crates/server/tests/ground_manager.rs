#[path = "../../../oracle/chunk-sync/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/ground-manager/oracle.jsonl");
fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (index, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{index}")) {
                return Some(d);
            }
        }
        return Some(format!("{path}: lengths {} != {}", a.len(), b.len()));
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn original_manager_chunk_mutations_private_streams_and_lifetime_prefixes() {
    for (index, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        if let Some(diff) = difference(&native::evaluate(&case), &case["expected"], String::new()) {
            panic!("ground manager case {index}: {diff}");
        }
    }
}
#[test]
fn ground_manager_fixture_and_original_sources_are_pinned() {
    let provenance: Value = serde_json::from_str(include_str!(
        "../../../fixtures/ground-manager/provenance.json"
    ))
    .unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, hash) in provenance["sources"].as_object().unwrap() {
        let p = root.join(path);
        if p.is_file() {
            let bytes: Vec<_> = std::fs::read(&p)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{}",
                p.display()
            );
        }
    }
}

#[test]
fn private_and_public_manager_packets_reach_the_shared_client_reducer() {
    use rs09_protocol::{
        Result,
        game::{Decoder, Direction, Frame, ground},
    };
    struct Metadata;
    impl ground::Context for Metadata {
        fn info(&mut self, _: &mut ground::State, _: i32) -> Result<ground::Info> {
            Ok(ground::Info {
                cost: 1,
                stackable: 0,
            })
        }
        fn height(&mut self, _: &mut ground::State, _: i32, _: i32, _: i32) -> Result<i32> {
            Ok(0)
        }
    }
    let result = native::evaluate(&serde_json::json!({"manager":true,"actions":[
        {"op":"newGround","id":1,"p":0,"location":[3201,3202,0]},
        {"op":"managerCreate","g":0},
        {"op":"newGround","id":2,"p":null,"location":[3203,3202,0]},
        {"op":"setGround","g":1,"forceVisible":true},
        {"op":"managerCreate","g":1},
        {"op":"update","p":0},{"op":"update","p":1},
        {"op":"managerIncrease","id":1,"amount":2,"p":0,"location":[3201,3202,0]},
        {"op":"playerState","p":0,"location":[3201,3202,0]},
        {"op":"managerDestroy","g":0}
    ]}));
    let mut clients = [ground::State::default(), ground::State::default()];
    let private = ground::Tile {
        plane: 0,
        x: 49,
        y: 34,
    };
    let public = ground::Tile {
        plane: 0,
        x: 51,
        y: 34,
    };
    let mut read = 0;
    for (step, row) in result["results"].as_array().unwrap().iter().enumerate() {
        assert!(row.get("error").is_none(), "{step}: {row}");
        for sent in row["state"]["sent"].as_array().unwrap().iter().skip(read) {
            let player = sent[0].as_u64().unwrap() as usize;
            let packet = &sent[1];
            let opcode = packet["opcode"].as_u64().unwrap() as u8;
            let hex = packet["hex"].as_str().unwrap();
            let payload: Vec<_> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            let mut wire = vec![];
            if opcode == 26 {
                // Original private IoBuffer combines fixed26 and the item
                // opcode. It must be framed as two native packets.
                wire.push(opcode);
                wire.extend_from_slice(&payload);
            } else {
                Frame { opcode, payload }
                    .write(Direction::ServerToClient, None, &mut wire)
                    .unwrap();
            }
            let mut decoder = Decoder::new(Direction::ServerToClient, 4096, 4096);
            let mut canonical = vec![];
            for byte in wire.iter() {
                decoder.push(&[*byte]).unwrap();
                while let Some(frame) = decoder.next_frame().unwrap() {
                    frame
                        .write(Direction::ServerToClient, None, &mut canonical)
                        .unwrap();
                    clients[player].apply_frame(&frame, &mut Metadata).unwrap();
                }
            }
            assert_eq!(canonical, wire);
            assert_eq!(decoder.buffered(), 0);
        }
        read = row["state"]["sent"].as_array().unwrap().len();
        if step == 1 {
            assert_eq!(
                clients[0].stack(private).unwrap().unwrap().items()[0]
                    .get()
                    .amount,
                1
            );
            assert!(clients[1].stack(private).unwrap().is_none());
        }
        if step == 6 {
            for client in &clients {
                assert_eq!(
                    client.stack(public).unwrap().unwrap().items()[0].get().id,
                    2
                );
            }
        }
        if step == 7 {
            assert_eq!(
                clients[0].stack(private).unwrap().unwrap().items()[0]
                    .get()
                    .amount,
                3
            );
        }
    }
    assert_eq!(read, 5);
    for client in clients {
        assert!(client.stack(private).unwrap().is_none());
        assert_eq!(client.occupied(), [public]);
    }
}

#[test]
fn manager_chunk_and_exposed_flags_retain_then_release_the_exact_owner() {
    use rs09_server::{
        definitions::item::{self as definitions, Shared},
        inventory::item,
        world::{
            chunk::{HandleChunk, RegionChunk},
            chunk_sync::{DirtyTracker, Items},
            doors::Location,
            ground_item::{self as ground, Handle},
            ground_manager::{self as gm, Packet},
            player_walk::{NodeHandle, State},
        },
    };
    struct Host {
        defs: definitions::Registry,
        actors: State,
        chunk: HandleChunk,
        dirty: DirtyTracker,
    }
    impl ground::Context for Host {
        fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
            Ok(Shared::new(item::Item::new(id, amount, charge, &self.defs)))
        }
        fn default_interaction(&mut self, _: &Handle) -> ground::Result<()> {
            Ok(())
        }
        fn uid(&mut self, _: &NodeHandle) -> ground::Result<i32> {
            Ok(7)
        }
        fn username(&mut self, _: &NodeHandle) -> ground::Result<Option<String>> {
            Ok(None)
        }
        fn ticks(&mut self) -> ground::Result<i32> {
            Ok(0)
        }
    }
    impl gm::Context for Host {
        type Plugin = ();
        fn tradeable(&mut self, _: &definitions::Handle) -> gm::Result<bool> {
            Ok(true)
        }
        fn plugin(&mut self, _: &definitions::Handle) -> gm::Result<Option<()>> {
            Ok(None)
        }
        fn plugin_remove(
            &mut self,
            _: (),
            _: Option<NodeHandle>,
            _: &Handle,
            _: i32,
        ) -> gm::Result<()> {
            unreachable!()
        }
        fn chunk(&mut self, _: Location) -> gm::Result<HandleChunk> {
            Ok(self.chunk.clone())
        }
        fn flagged(&mut self, c: &HandleChunk) -> gm::Result<()> {
            self.dirty.on_flagged(c);
            Ok(())
        }
        fn player_location(&mut self, _: &NodeHandle) -> gm::Result<Option<Location>> {
            unreachable!()
        }
        fn playing(&mut self, _: &NodeHandle) -> gm::Result<bool> {
            unreachable!()
        }
        fn artificial(&mut self, _: &NodeHandle) -> gm::Result<bool> {
            unreachable!()
        }
        fn ai_items(&mut self, _: &NodeHandle) -> gm::Result<Option<Items>> {
            unreachable!()
        }
        fn packet(
            &mut self,
            _: Packet,
            _: Option<NodeHandle>,
            _: &Handle,
            _: i32,
        ) -> gm::Result<()> {
            unreachable!()
        }
    }
    let location = Location::new(3201, 3202, 0);
    let mut host = Host {
        defs: definitions::Registry::default(),
        actors: State::default(),
        chunk: Shared::new(RegionChunk::new(Location::new(3200, 3200, 0), 0)),
        dirty: DirtyTracker::default(),
    };
    let id = host.actors.add(1, location.clone(), [1, 2, 3, 4]);
    let owner = host.actors.retain_node(id).unwrap();
    let raw = Shared::new(item::Item::new(1, 1, 1000, &host.defs));
    let g = ground::construct(&mut host, &raw, Some(location), 200, Some(owner.clone())).unwrap();
    g.write().force_visible = true;
    let weak = g.downgrade();
    let manager = gm::Manager::default();
    gm::create(&mut host, &manager, Some(g.clone())).unwrap();
    host.actors.actor_mut(id).unwrap().connected = false;
    drop(owner);
    drop(g);
    host.actors.collect();
    assert!(host.actors.actor(id).is_ok());
    let g = manager.get_items().read().get(0).unwrap().clone().unwrap();
    let exposed_item = ground::as_item(&g);
    gm::destroy(&mut host, &manager, Some(g.clone())).unwrap();
    drop(g);
    assert!(manager.get_items().read().is_empty());
    assert!(host.chunk.read().items.as_ref().unwrap().read().is_empty());
    let retained = host.chunk.read().update_flags.read()[0].clone().unwrap();
    host.dirty.reset_all();
    host.actors.collect();
    assert!(host.actors.actor(id).is_ok());
    assert!(weak.upgrade().is_some());
    drop(retained);
    host.actors.collect();
    assert!(weak.upgrade().is_some());
    assert!(host.actors.actor(id).is_ok());
    drop(exposed_item);
    host.actors.collect();
    assert!(weak.upgrade().is_none());
    assert!(host.actors.actor(id).is_err());
}
