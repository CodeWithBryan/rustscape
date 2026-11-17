#[path = "../../../oracle/entity-regions/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/entity-regions/oracle.jsonl");
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
        if a.len() == b.len() {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(d) = diff(a, b, format!("{path}/{i}")) {
                    return Some(d);
                }
            }
        }
    }
    Some(format!("{path}:native={a},original={b}"))
}
#[test]
fn original_chunk_membership_query_order_move_callbacks_and_tolerances() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("entity regions case{i}:{d}");
        }
    }
}
#[test]
fn entity_region_fixture_and_original_sources_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/entity-regions/provenance.json"
    ))
    .unwrap();
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

#[test]
fn chunk_list_aliases_and_duplicate_memberships_retain_original_node_identity() {
    use rs09_server::{
        definitions::item::Shared,
        world::{
            chunk::RegionChunk, doors::Location, entity_regions as spatial, player_walk::State,
        },
    };
    let mut state = State::default();
    let node = state.add_npc(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let handle = state.retain_node(node).unwrap();
    let chunk = Shared::new(RegionChunk::new(Location::new(3200, 3216, 0), 0));
    spatial::add_npc(&chunk, Some(handle.clone()));
    spatial::add_npc(&chunk, Some(handle.clone()));
    let exposed = chunk.read().npcs.clone();
    state.actor_mut(node).unwrap().connected = false;
    spatial::remove_npc(&chunk, Some(handle));
    assert_eq!(exposed.values().len(), 1);
    drop(chunk);
    state.collect();
    assert!(state.actor(node).is_ok());
    let later = state.add_npc(1, Location::new(3208, 3216, 0), [1, 2, 3, 4]);
    let later_handle = state.retain_node(later).unwrap();
    assert!(exposed.values()[0].as_ref().unwrap() != &later_handle);
    exposed.clear();
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(state.actor(later).is_ok());
}

#[test]
fn node_identity_in_different_worlds_is_distinct_even_when_numeric_ids_match() {
    use rs09_server::world::{doors::Location, player_walk::State};
    let (mut a, mut b) = (State::default(), State::default());
    let x = a.add_npc(1, Location::new(1, 1, 0), [1, 2, 3, 4]);
    let y = b.add_npc(1, Location::new(1, 1, 0), [1, 2, 3, 4]);
    assert_eq!(x, y);
    assert!(a.retain_node(x).unwrap() != b.retain_node(y).unwrap());
}

#[test]
fn local_query_temporaries_and_results_keep_entities_alive_across_callbacks() {
    use rs09_server::{
        definitions::item::Shared,
        world::{
            chunk::{HandleChunk, RegionChunk},
            doors::Location,
            entity_regions::{self as spatial, Context, Error, Kind, Result, Select},
            player_walk::{NodeHandle, State},
            region,
        },
    };
    struct Host {
        state: State,
        chunk: HandleChunk,
        earlier: usize,
        later: usize,
        calls: usize,
        collected: bool,
    }
    impl Context for Host {
        fn chunk(&mut self, l: Location) -> Result<HandleChunk> {
            self.calls += 1;
            if l.coordinates() == [3200, 3216, 0] {
                Ok(self.chunk.clone())
            } else {
                Ok(Shared::new(RegionChunk::new(l, 0)))
            }
        }
        fn kind(&mut self, _: &NodeHandle) -> Result<Kind> {
            Ok(Kind::Npc)
        }
        fn location(&mut self, n: &NodeHandle) -> Result<Option<Location>> {
            let l = self.state.actor(n.node).unwrap().queue.entity().location;
            if n.node == self.later && !self.collected {
                self.collected = true;
                self.chunk.read().npcs.clear();
                self.state.collect();
                assert!(
                    self.state.actor(self.earlier).is_ok(),
                    "the original temporary ArrayList still contains the filtered-out earlier entity"
                );
            }
            Ok(l)
        }
        fn region(&mut self, _: Location) -> Result<region::Handle> {
            Err(Error::Null)
        }
        fn as_player(&mut self, _: &NodeHandle) -> Result<NodeHandle> {
            Err(Error::Null)
        }
        fn name(&mut self, _: &NodeHandle) -> Result<String> {
            Err(Error::Null)
        }
        fn flag_active(&mut self, _: region::Handle) -> Result<()> {
            Err(Error::Null)
        }
        fn check_inactive(&mut self, _: region::Handle) -> Result<()> {
            Err(Error::Null)
        }
        fn footprint(&mut self, _: &NodeHandle) -> Result<Option<Location>> {
            Err(Error::Null)
        }
        fn zone_update(&mut self, _: &NodeHandle, _: Option<Location>) -> Result<()> {
            Err(Error::Null)
        }
        fn unknown_entity(&mut self, _: &NodeHandle) -> Result<()> {
            Err(Error::Null)
        }
        fn now_millis(&mut self) -> Result<i64> {
            Err(Error::Null)
        }
    }
    let mut state = State::default();
    let a = state.add_npc(1, Location::new(3230, 3216, 0), [1, 2, 3, 4]);
    let b = state.add_npc(2, Location::new(3208, 3216, 0), [1, 2, 3, 4]);
    let chunk = Shared::new(RegionChunk::new(Location::new(3200, 3216, 0), 0));
    for n in [a, b] {
        state.actor_mut(n).unwrap().connected = false;
        spatial::add_npc(&chunk, Some(state.retain_node(n).unwrap()));
    }
    let mut host = Host {
        state,
        chunk,
        earlier: a,
        later: b,
        calls: 0,
        collected: false,
    };
    assert!(matches!(
        spatial::local(
            &mut host,
            Location::new(3207, 3216, 0),
            i32::MAX,
            Select::Npcs
        ),
        Err(Error::Limit)
    ));
    assert_eq!(
        host.calls, 0,
        "bounded rejection must precede chunk service work"
    );
    let result = spatial::local(&mut host, Location::new(3207, 3216, 0), 15, Select::Npcs).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].node, b);
    assert!(host.collected);
    host.state.collect();
    assert!(host.state.actor(a).is_err());
    assert!(host.state.actor(b).is_ok());
    drop(result);
    host.state.collect();
    assert_eq!(host.state.retained_counts(), [0, 0, 0]);
}
