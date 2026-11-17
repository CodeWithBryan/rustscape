#[path = "../../../oracle/chunk-sync/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/chunk-sync/oracle.jsonl");
fn diff(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = diff(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = diff(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
        return Some(format!(
            "{path}: native length={}, original length={}",
            a.len(),
            b.len()
        ));
    }
    Some(format!("{path}:native={a},original={b}"))
}
#[test]
fn original_chunk_rebuild_flags_ground_visibility_and_dirty_reset() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("chunk sync case{i}:{d}");
        }
    }
}
#[test]
fn chunk_sync_fixture_and_original_sources_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/chunk-sync/provenance.json")).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, hash) in p["sources"].as_object().unwrap() {
        let p = root.join(path);
        if p.is_file() {
            let b: Vec<_> = std::fs::read(&p)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(b)),
                hash.as_str().unwrap(),
                "{}",
                p.display()
            );
        }
    }
}

mod ownership {
    use rs09_server::{
        definitions::item::{self as definitions, Shared},
        inventory::item,
        world::{
            chunk::{HandleChunk, RegionChunk},
            chunk_sync::{self as sync, Buffer, Context, DirtyTracker, Flag, Result},
            doors::{Handle as Object, Location},
            ground_item::{self as ground, Handle as Ground},
            player_walk::{NodeHandle, State},
            scenery_node::Layer,
        },
    };
    #[derive(Default)]
    struct Host {
        state: State,
        defs: definitions::Registry,
        dirty: DirtyTracker,
        target: Option<usize>,
        observed: bool,
    }
    impl ground::Context for Host {
        fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
            Ok(Shared::new(item::Item::new(id, amount, charge, &self.defs)))
        }
        fn default_interaction(&mut self, _: &Ground) -> ground::Result<()> {
            Ok(())
        }
        fn uid(&mut self, _: &NodeHandle) -> ground::Result<i32> {
            Ok(7)
        }
        fn username(&mut self, _: &NodeHandle) -> ground::Result<Option<String>> {
            Ok(Some("owner".into()))
        }
        fn ticks(&mut self) -> ground::Result<i32> {
            Ok(0)
        }
    }
    impl Context for Host {
        fn last_scene(&mut self, _: &NodeHandle) -> Result<Location> {
            Ok(Location::new(3207, 3216, 0))
        }
        fn send(&mut self, _: &NodeHandle, _: &Buffer) -> Result<()> {
            Ok(())
        }
        fn attribute(&mut self, _: &NodeHandle, _: &str) -> Result<bool> {
            Ok(false)
        }
        fn clipping(&mut self, _: i32, _: i32, _: i32) -> Result<i32> {
            unreachable!()
        }
        fn layer(&mut self, _: &Object) -> Result<Option<Layer>> {
            unreachable!()
        }
        fn foreign_flag(&mut self, id: usize, _: &Buffer, _: &NodeHandle) -> Result<()> {
            if id == 1 {
                self.dirty.reset_all();
                self.state.collect();
                assert!(self.state.actor(self.target.unwrap()).is_ok());
                self.observed = true;
            } else {
                assert_eq!(id, 2);
            }
            Ok(())
        }
    }
    fn chunk() -> HandleChunk {
        Shared::new(RegionChunk::new(Location::new(3200, 3200, 0), 0))
    }
    #[test]
    fn ground_owner_rebind_and_exposed_flags_retain_exact_nodes_without_leaking() {
        let mut h = Host::default();
        let l = Location::new(3207, 3216, 0);
        let old = h.state.add(1, l.clone(), [1, 2, 3, 4]);
        let p = h.state.retain_node(old).unwrap();
        let input = Shared::new(item::Item::new(1, 5, 1000, &h.defs));
        let g = ground::construct(&mut h, &input, Some(l.clone()), 200, Some(p.clone())).unwrap();
        h.state.actor_mut(old).unwrap().connected = false;
        drop(p);
        h.state.collect();
        assert!(h.state.actor(old).is_ok());
        let later = h.state.add(1, l, [1, 2, 3, 4]);
        let p = h.state.retain_node(later).unwrap();
        assert!(g.read().dropper.as_ref().unwrap() != &p);
        assert!(ground::dropped_by(&mut h, &g, &p).unwrap());
        h.state.collect();
        assert!(h.state.actor(old).is_err());
        h.state.actor_mut(later).unwrap().connected = false;
        drop(p);
        let a = chunk();
        let b = chunk();
        let f = Shared::new(Flag::Item {
            item: g.clone(),
            kind: 0,
            old_amount: 0,
        });
        let exposed = f.clone();
        for c in [&a, &b] {
            sync::flag(c, Some(f.clone()), &mut |c| {
                h.dirty.on_flagged(c);
                Ok(())
            })
            .unwrap();
        }
        let weak = g.downgrade();
        drop(g);
        drop(f);
        assert_eq!(h.dirty.len(), 2);
        sync::reset_flags(&a);
        assert_eq!(h.dirty.len(), 2);
        assert_eq!(b.read().update_flags.read().len(), 1);
        h.dirty.reset_all();
        assert!(a.read().update_flags.read().is_empty());
        assert!(b.read().update_flags.read().is_empty());
        assert!(weak.upgrade().is_some());
        h.state.collect();
        assert!(h.state.actor(later).is_ok());
        drop(exposed);
        assert!(weak.upgrade().is_none());
        h.state.collect();
        assert!(h.state.actor(later).is_err());
        assert_eq!(h.state.retained_counts(), [0, 0, 0]);
    }
    #[test]
    fn update_snapshot_keeps_foreign_captures_alive_after_reentrant_queue_reset() {
        let mut h = Host::default();
        let l = Location::new(3207, 3216, 0);
        let viewer = h.state.add(1, l.clone(), [1, 2, 3, 4]);
        let p = h.state.retain_node(viewer).unwrap();
        let target = h.state.add_npc(2, l, [1, 2, 3, 4]);
        h.target = Some(target);
        h.state.actor_mut(target).unwrap().connected = false;
        let capture = Shared::new(h.state.retain_node(target).unwrap());
        let weak = capture.downgrade();
        let a = chunk();
        let first = Shared::new(Flag::Foreign(1));
        let second = Shared::new(Flag::Foreign(2)).keeping_from(&capture);
        for f in [first, second] {
            sync::flag(&a, Some(f), &mut |c| {
                h.dirty.on_flagged(c);
                Ok(())
            })
            .unwrap();
        }
        drop(capture);
        sync::update(&mut h, &a, &p).unwrap();
        assert!(h.observed);
        assert!(weak.upgrade().is_none());
        h.state.collect();
        assert!(h.state.actor(target).is_err());
        assert!(h.state.actor(viewer).is_ok());
    }
}

#[test]
fn scene_flags_keep_chunk_identity_across_coordinate_replacement_and_release_old_chunks() {
    use rs09_server::{
        definitions::{item::Shared, scenery},
        world::{
            chunk::RegionChunk,
            doors::{self, Location, Scenery},
            scene::{Assets, Scene},
            scenery::ObjectUpdate,
        },
    };
    struct Empty;
    impl Assets for Empty {
        fn archive_id(&self, _: &str) -> rs09_server::world::scene::Result<i32> {
            unreachable!()
        }
        fn mapscape(&self, _: i32) -> rs09_server::world::scene::Result<Option<Vec<u8>>> {
            unreachable!()
        }
        fn landscape(
            &self,
            _: i32,
            _: [u32; 4],
        ) -> rs09_server::world::scene::Result<Option<Vec<u8>>> {
            unreachable!()
        }
        fn keys(&self, _: i32) -> rs09_server::world::scene::Result<[u32; 4]> {
            unreachable!()
        }
    }
    let mut a = Scene::new(
        Empty,
        scenery::Registry::default(),
        doors::Registry::default(),
    );
    let l = Location::new(3200, 3200, 0);
    let old = a.chunk(&l).unwrap();
    let old_token = a.scenery_chunk(&l).unwrap();
    let weak = old.downgrade();
    let current = Shared::new(RegionChunk::new(l.clone(), 0));
    let r = a.region(12850);
    r.read().chunks.as_ref().unwrap().write()[0][0][0] = Some(current.clone());
    assert!(a.chunk(&l).unwrap().same(&current));
    let current_token = a.scenery_chunk(&l).unwrap();
    assert_ne!(old_token, current_token);
    let object = Shared::new(Scenery::new(1, l.clone(), 0, 0, None, false).unwrap());
    for token in [old_token, current_token] {
        a.flag_token(
            token,
            ObjectUpdate {
                object: object.clone(),
                remove: false,
            },
        )
        .unwrap();
    }
    assert_eq!(a.dirty_chunks.len(), 2);
    assert_eq!(old.read().update_flags.read().len(), 1);
    assert_eq!(current.read().update_flags.read().len(), 1);
    let mut b = Scene::new(
        Empty,
        scenery::Registry::default(),
        doors::Registry::default(),
    );
    let foreign = b.chunk(&l).unwrap();
    let foreign_token = b.scenery_chunk(&l).unwrap();
    assert_ne!(foreign_token, current_token);
    assert!(
        a.flag_token(
            foreign_token,
            ObjectUpdate {
                object: object.clone(),
                remove: false
            }
        )
        .is_err()
    );
    assert!(
        a.flag_object(
            &foreign,
            ObjectUpdate {
                object: object.clone(),
                remove: true
            }
        )
        .is_err()
    );
    assert!(foreign.read().update_flags.read().is_empty());
    drop(old);
    a.collect_pulses();
    assert!(weak.upgrade().is_some());
    a.clear_updates();
    assert!(weak.upgrade().is_none());
    assert!(a.dirty_chunks.is_empty());
    assert!(current.read().update_flags.read().is_empty());
    assert!(
        a.flag_token(
            old_token,
            ObjectUpdate {
                object,
                remove: false
            }
        )
        .is_err()
    );
}
