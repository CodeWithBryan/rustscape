#[path = "../../../oracle/scenery-node/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/scenery-node/oracle.jsonl");
#[test]
fn retained_child_and_array_views_keep_wrappers_alive_and_release_the_family() {
    use rs09_server::{
        definitions::{
            item::Shared,
            scenery::{Registry, SceneryDefinition},
        },
        world::{
            doors::{self, Location},
            scenery_node as node,
        },
    };
    let mut registry = Registry::default();
    registry.definitions.insert(
        1,
        Shared::new(SceneryDefinition {
            id: 1,
            children_ids: Some(Shared::new(vec![2, -1])),
            ..SceneryDefinition::default()
        }),
    );
    let root =
        node::construct(&mut registry, 1, Location::new(3207, 3217, 0), 0, 1, false).unwrap();
    let weak_root = root.downgrade();
    let array = node::children(&root).unwrap();
    let child = array.get(0).unwrap();
    let weak_child = child.downgrade();
    assert!(doors::wrapper(&child).same(&root));
    drop(root);
    assert!(weak_root.upgrade().is_some());
    drop(array);
    assert!(weak_root.upgrade().is_some());
    // Traverse both directions repeatedly; owner chains must not accumulate.
    for _ in 0..4096 {
        let wrapper = doors::wrapper(&child);
        let from_array = node::children(&wrapper).unwrap().get(0).unwrap();
        assert!(from_array.same(&child));
    }
    drop(child);
    assert!(weak_root.upgrade().is_none());
    assert!(weak_child.upgrade().is_none());
}

#[test]
fn nested_morph_views_retain_the_root_without_parent_child_cycles() {
    use rs09_server::{
        definitions::{
            item::Shared,
            scenery::{Registry, SceneryDefinition},
        },
        world::{
            doors::{self, Location},
            scenery_node as node,
        },
    };
    let mut registry = Registry::default();
    for (id, children) in [(1, vec![2]), (2, vec![3])] {
        registry.definitions.insert(
            id,
            Shared::new(SceneryDefinition {
                id,
                children_ids: Some(Shared::new(children)),
                ..SceneryDefinition::default()
            }),
        );
    }
    let root = node::construct(&mut registry, 1, Location::new(1, 1, 0), 10, 0, false).unwrap();
    let weak_root = root.downgrade();
    let child = node::children(&root).unwrap().get(0).unwrap();
    let weak_child = child.downgrade();
    let grandchild = node::children(&child).unwrap().get(0).unwrap();
    let weak_grandchild = grandchild.downgrade();
    drop(root);
    drop(child);
    let parent = doors::wrapper(&grandchild);
    assert_eq!(parent.read().id, 2);
    let ancestor = doors::wrapper(&parent);
    assert_eq!(ancestor.read().id, 1);
    drop(parent);
    drop(ancestor);
    drop(grandchild);
    assert!(weak_root.upgrade().is_none());
    assert!(weak_child.upgrade().is_none());
    assert!(weak_grandchild.upgrade().is_none());
}
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
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array())
        && a.len() == b.len()
    {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = diff(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn scenery_constructor_retained_definitions_morphs_geometry_and_identity_match_original() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("scenery-node case {i}: {d}; actions={}", c["actions"]);
        }
    }
}
#[test]
fn scenery_node_sources_and_fixture_bytes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/scenery-node/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
