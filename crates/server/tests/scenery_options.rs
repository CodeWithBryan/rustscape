use rs09_server::{
    definitions::{
        item::Shared,
        option_handler::{self, Handler},
        property::Property,
        scenery::{Registry as Definitions, SceneryDefinition},
    },
    world::{
        door_option,
        doors::{Handle, Location},
        interaction_options as options,
        player_walk::State,
        scenery_node,
        scenery_options::Registry,
    },
};
#[path = "../../../oracle/scenery-options/native.rs"]
mod native;
#[test]
fn joined_original_scenery_defaults_cached_definitions_and_handler_lookup() {
    for (i, line) in include_str!("../../../fixtures/scenery-options/oracle.jsonl")
        .lines()
        .enumerate()
    {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "scenery defaults {i}: {line}"
        );
    }
}
#[test]
fn joined_scenery_defaults_sources_and_corpus_are_pinned() {
    use sha2::{Digest, Sha256};
    let corpus = include_bytes!("../../../fixtures/scenery-options/oracle.jsonl");
    let p: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/scenery-options/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(corpus)));
    assert_eq!(
        p["caseCount"],
        std::str::from_utf8(corpus).unwrap().lines().count()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
}
struct Other;
impl option_handler::NativeOptionHandler for Other {}
fn fixture() -> (State, Registry, Definitions, Handle) {
    let mut defs = Definitions::default();
    for (id, name) in [(12348, "Open"), (10327, "Close")] {
        defs.definitions.insert(
            id,
            Shared::new(SceneryDefinition {
                id,
                name: Some("Door".into()),
                options: Some(Shared::new(vec![Some(name.into()), None, None, None, None])),
                ..Default::default()
            }),
        );
    }
    let object =
        scenery_node::construct(&mut defs, 12348, Location::new(3207, 3217, 0), 0, 1, false)
            .unwrap();
    let registry = Registry::default();
    registry.register_doors().unwrap();
    (State::default(), registry, defs, object)
}
fn initialize(s: &mut State, r: &Registry, d: &mut Definitions, h: &Handle) {
    r.initialize(s, d, h, |s| {
        Ok(rs09_game_core::java_lower::lowercase_root(s))
    })
    .unwrap()
}
#[test]
fn local_handlers_use_current_id_while_action_names_use_the_cached_definition() {
    let (mut s, r, mut d, h) = fixture();
    let original = r.defaults().read().get(Some("open")).unwrap();
    let other = Handler::new("test.other", Other);
    d.for_id(10327)
        .write()
        .handlers
        .write()
        .insert("option:open".into(), Property::OptionHandler(other.clone()));
    h.write().id = 10327;
    initialize(&mut s, &r, &mut d, &h);
    let capture = r.capture(&mut s, &h).unwrap();
    let option = options::get(&capture.table, 0).unwrap().unwrap();
    assert_eq!(option.read().name().as_deref(), Some("Open"));
    assert!(
        capture
            .handler(option.read().handler.unwrap())
            .unwrap()
            .same(&other)
    );
    assert!(original.read().value::<door_option::Program>().is_some());
}
#[test]
fn initialized_tables_keep_old_array_and_handler_identity_until_explicit_reinit() {
    let (mut s, r, mut d, h) = fixture();
    initialize(&mut s, &r, &mut d, &h);
    let c = r.capture(&mut s, &h).unwrap();
    let array = c.table.read().options();
    let old = options::get(&c.table, 0).unwrap().unwrap();
    let other = Handler::new("same observation label", Other);
    option_handler::set_option_handler(&r.defaults(), Some("open".into()), Some(other.clone()));
    initialize(&mut s, &r, &mut d, &h);
    assert!(c.table.read().options().same(&array));
    assert!(options::get(&c.table, 0).unwrap().unwrap().same(&old));
    c.table.write().initialized = false;
    initialize(&mut s, &r, &mut d, &h);
    assert!(!c.table.read().options().same(&array));
    let new = options::get(&c.table, 0).unwrap().unwrap();
    assert!(!new.same(&old));
    assert!(r.handler(new.read().handler.unwrap()).unwrap().same(&other));
    assert!(
        r.handler(old.read().handler.unwrap())
            .unwrap()
            .read()
            .value::<door_option::Program>()
            .is_some()
    );
}
#[test]
fn foreign_registry_and_actor_domain_rejected_before_lowercase_or_mutation() {
    let (mut s, r, mut d, h) = fixture();
    initialize(&mut s, &r, &mut d, &h);
    let c = r.capture(&mut s, &h).unwrap();
    let array = c.table.read().options();
    let mut other = State::with_runner(s.runner());
    assert!(
        r.initialize(&mut other, &mut d, &h, |_| panic!("foreign callback"))
            .is_err()
    );
    assert!(r.capture(&mut other, &h).is_err());
    assert!(
        Registry::default()
            .initialize(&mut s, &mut d, &h, |_| panic!("foreign registry callback"))
            .is_err()
    );
    assert!(c.table.read().options().same(&array));
}
#[test]
fn callback_reentry_and_partial_type_failure_preserve_the_installed_table() {
    let (mut s, r, mut d, h) = fixture();
    let def = d.for_id(12348);
    def.write().options = Some(Shared::new(vec![Some("Open".into()), Some("Close".into())]));
    let bad = def.clone();
    let mut calls = 0;
    let result = r.initialize(&mut s, &mut d, &h, |name| {
        calls += 1;
        // This must not deadlock on either the scenery, definition or arena.
        h.write().rotation = 2;
        bad.write()
            .handlers
            .write()
            .insert("option:close".into(), Property::Text("wrong".into()));
        Ok(name.to_lowercase())
    });
    assert!(result.is_err());
    assert_eq!(calls, 2);
    let c = r.capture(&mut s, &h).unwrap();
    assert!(!c.table.read().initialized);
    assert!(options::get(&c.table, 0).unwrap().is_some());
    assert!(options::get(&c.table, 1).unwrap().is_none());
    def.write().handlers.write().remove("option:close");
    initialize(&mut s, &r, &mut d, &h);
    assert!(c.table.read().initialized);
}
#[test]
fn scene_owned_target_identity_survives_collection_but_arrays_do_not_root_it() {
    let (mut s, r, mut d, h) = fixture();
    initialize(&mut s, &r, &mut d, &h);
    let c = r.capture(&mut s, &h).unwrap();
    let node = c.target.node;
    let array = c.table.read().options();
    let weak = h.downgrade();
    drop(c);
    for _ in 0..8 {
        s.collect();
        assert_eq!(r.capture(&mut s, &h).unwrap().target.node, node);
    }
    let weak_handler = r.defaults().read().get(Some("open")).unwrap().downgrade();
    drop(h);
    drop(d);
    drop(r);
    s.collect();
    assert!(s.scenery(node).is_err());
    assert!(weak.upgrade().is_none());
    assert!(weak_handler.upgrade().is_some());
    drop(array);
    assert!(weak_handler.upgrade().is_none());
}
#[test]
fn exposed_table_retains_the_actual_nested_family_and_final_release_collects_it() {
    let (mut s, r, mut d, _) = fixture();
    for (id, children) in [(1, vec![2]), (2, vec![12348])] {
        d.definitions.insert(
            id,
            Shared::new(SceneryDefinition {
                id,
                children_ids: Some(Shared::new(children)),
                ..Default::default()
            }),
        );
    }
    let root = scenery_node::construct(&mut d, 1, Location::new(1, 1, 0), 0, 0, false).unwrap();
    let child = scenery_node::children(&root).unwrap().get(0).unwrap();
    let leaf = scenery_node::children(&child).unwrap().get(0).unwrap();
    let wr = root.downgrade();
    let wc = child.downgrade();
    let wl = leaf.downgrade();
    initialize(&mut s, &r, &mut d, &leaf);
    let c = r.capture(&mut s, &leaf).unwrap();
    let node = c.target.node;
    let table = c.table.clone();
    drop(root);
    drop(child);
    drop(leaf);
    drop(c);
    s.collect();
    assert!(wr.upgrade().is_some());
    assert!(wc.upgrade().is_some());
    assert!(wl.upgrade().is_some());
    let view = s.scenery(node).unwrap();
    let canonical = s.retain_node(node).unwrap();
    drop(table);
    drop(view);
    s.collect();
    assert!(wr.upgrade().is_some());
    drop(canonical);
    for _ in 0..3 {
        s.collect();
    }
    assert!(s.scenery(node).is_err());
    assert!(wr.upgrade().is_none());
    assert!(wc.upgrade().is_none());
    assert!(wl.upgrade().is_none());
}
#[test]
fn shared_generic_arenas_never_accept_another_registry_id() {
    let (_, r, _, _) = fixture();
    let h = door_option::native_handler();
    let a = r.bind(h.clone()).unwrap();
    assert_eq!(a, r.bind(h.clone()).unwrap());
    let other = rs09_server::world::ground_options::Registry::default();
    let b = other.bind(h).unwrap();
    assert_ne!(a, b);
    assert!(r.handler(b).is_err());
    assert!(other.handler(a).is_err());
}
#[test]
fn exposed_canonical_scenery_view_alone_retains_the_family() {
    let (mut s, r, mut d, _) = fixture();
    d.for_id(1).write().children_ids = Some(Shared::new(vec![12348]));
    let root = scenery_node::construct(&mut d, 1, Location::new(1, 1, 0), 0, 0, false).unwrap();
    let leaf = scenery_node::children(&root).unwrap().get(0).unwrap();
    let weak = root.downgrade();
    initialize(&mut s, &r, &mut d, &leaf);
    let c = r.capture(&mut s, &leaf).unwrap();
    let node = c.target.node;
    let view = s.scenery(node).unwrap();
    drop(c);
    drop(root);
    drop(leaf);
    s.collect();
    assert!(weak.upgrade().is_some());
    assert_eq!(rs09_server::world::doors::wrapper(&view).read().id, 1);
    drop(view);
    for _ in 0..3 {
        s.collect();
    }
    assert!(weak.upgrade().is_none());
    assert!(s.scenery(node).is_err());
}
