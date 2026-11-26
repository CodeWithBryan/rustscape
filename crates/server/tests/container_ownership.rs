#[path = "../../../oracle/inventory/native.rs"]
mod native;
use rs09_server::{
    definitions::item::{Registry, Shared},
    inventory::{
        self,
        container::{Container, ContainerType, Context, Event, Factory, SortType},
        container_listener::Handle as ListenerHandle,
        item::Handle,
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/container-ownership/oracle.jsonl");
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
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array())
        && a.len() == b.len()
    {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}
#[test]
fn original_aliased_receivers_listener_views_and_reentrant_prefixes_match() {
    // A callback locking a retained receiver must fail this regression within a
    // bounded time instead of hanging the entire verification process.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if let Some(d) = difference(&actual, &row["expected"], String::new()) {
                tx.send(Err(format!("shared container case {i}: {d}")))
                    .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("shared receiver replay failed or deadlocked")
        .unwrap();
}
#[test]
fn complete_original_classes_and_unchanged_inventory_baseline_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/container-ownership/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    assert_eq!(
        p["baselineInventoryUnchanged"],
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(root.join("fixtures/inventory/oracle.jsonl")).unwrap())
        )
    );
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
struct NoCallbacks;
impl Context for NoCallbacks {
    fn update(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: Event,
        _: &Registry,
    ) -> inventory::Result<()> {
        panic!("this mutation must not fire listeners")
    }
    fn refresh(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: &Registry,
    ) -> inventory::Result<()> {
        panic!("this mutation must not refresh listeners")
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<Handle>,
        _: &Registry,
    ) -> inventory::Result<()> {
        panic!("no ground service in this ownership regression")
    }
}
fn container() -> Container {
    Factory::new(&Registry::default())
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap()
}
#[test]
fn retired_container_identity_and_live_list_survive_replacement_without_retaining_dead_fields() {
    let old = container();
    let weak = old.downgrade();
    let mut captured = old.clone();
    let items = old.items();
    let event = old.event();
    let list = old.listener_list();
    drop(old);
    let replacement = container();
    assert!(!captured.same(&replacement));
    captured
        .clear(false, &Registry::default(), &mut NoCallbacks)
        .unwrap();
    assert!(!captured.items().same(&items));
    assert!(captured.event().same(&event));
    assert!(captured.event().read().clear);
    let listener = ListenerHandle::new(rs09_server::inventory::listener::Listener::new(None));
    list.add(Some(listener.clone()));
    assert_eq!(captured.listeners(), vec![Some(listener.clone())]);
    assert!(replacement.listeners().is_empty());
    assert!(weak.upgrade().is_some());
    drop(captured);
    assert!(weak.upgrade().is_none());
    assert_eq!(list.values(), vec![Some(listener)]);
    assert!(!event.read().items.same(&items));
}
#[test]
fn exposed_container_view_keeps_exact_profile_owner_and_retires_weak_anchor() {
    let core = container();
    let parent = Shared::new(core.clone());
    let weak_parent = parent.downgrade();
    let view = core.keeping_from(&parent);
    let weak = view.downgrade();
    drop(view);
    let mut upgraded = weak.upgrade().unwrap();
    drop(parent);
    assert!(weak_parent.upgrade().is_some());
    assert!(upgraded.same(&core));
    let old = core.items();
    upgraded
        .clear(false, &Registry::default(), &mut NoCallbacks)
        .unwrap();
    assert!(!core.items().same(&old));
    drop(upgraded);
    assert!(weak_parent.upgrade().is_none());
    assert!(weak.upgrade().is_none());
    assert_eq!(core.capacity(), 28);
}
