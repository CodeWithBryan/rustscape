#[path = "../../../oracle/player-inventory/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/player-inventory/oracle.jsonl");
#[test]
fn actual_inventory_listener_settings_definition_fields_and_packets_match_original_composition() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "player inventory case{i} op={} hooks={}",
            row["op"],
            row["hooks"]
        );
    }
}
#[test]
fn player_inventory_fixture_and_original_source_hashes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/player-inventory/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, digest) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if !path.is_file() {
            continue;
        }
        let data: Vec<_> = std::fs::read(path)
            .unwrap()
            .into_iter()
            .filter(|b| *b != b'\r')
            .collect();
        assert_eq!(
            digest.as_str().unwrap(),
            format!("{:x}", Sha256::digest(data))
        );
    }
}
#[test]
fn inventory_binding_rejects_another_worlds_listener_even_when_node_numbers_match() {
    use rs09_server::{
        definitions::item::{Registry, Shared},
        inventory::{
            self,
            container::{ContainerType, Factory, SortType},
            container_listener::Handle as ListenerHandle,
            listener::Listener,
            player_binding::{Context, Links},
        },
        player::settings::Settings,
        world::{doors::Location, interfaces::Manager, player_walk::State},
    };
    use std::{collections::BTreeSet, sync::Arc};
    let mut first = State::default();
    let a = first.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let a = first.retain_node(a).unwrap();
    let mut second = State::default();
    let b = second.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let b = second.retain_node(b).unwrap();
    assert_eq!(a.node, b.node);
    assert!(a != b);
    let definitions = Arc::new(Registry::default());
    let factory = Factory::new(&definitions);
    let mut inventory = factory
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap();
    inventory.register(Some(ListenerHandle::new(Listener::new(Some(b)))));
    let old = inventory.event();
    let mut equipment = factory
        .create(14, ContainerType::Default, SortType::Id)
        .unwrap();
    let settings = Settings::default();
    settings.fields.write().weight = 99.0;
    let mut host = native::Host {
        row: serde_json::json!({"hooks":[]}),
        definitions: definitions.clone(),
        frames: vec![],
        done: BTreeSet::new(),
        manager: Manager::default(),
        pouch: Shared::new(false),
        syncs: 0,
    };
    let mut links = Links {
        player: &a,
        inventory: inventory.clone(),
        equipment: &mut equipment,
        settings: &settings,
        null_item: &factory.null_item,
    };
    let result = inventory.refresh(
        None,
        &definitions,
        &mut Context {
            links: &mut links,
            services: &mut host,
        },
    );
    assert!(matches!(result, Err(inventory::Error::Binding(_))));
    assert!(host.frames.is_empty());
    assert_eq!(settings.fields.read().weight, 99.0);
    assert!(inventory.event().same(&old));
}
