#[path = "../../../oracle/inventory/native.rs"]
mod native;
use rs09_server::{
    definitions::item::{Registry, Shared},
    inventory::{
        self,
        container::{Container, ContainerType, Context, Event, Factory, SortType},
        container_listener::{Handle, NativeListener},
        item,
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/container-listeners/oracle.jsonl");

#[test]
fn original_distinct_listener_objects_aliases_rebinding_and_callbacks_match() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for (i, line) in CORPUS.lines().enumerate() {
            let row: Value = serde_json::from_str(line).unwrap();
            let actual = native::evaluate(&row);
            if actual != row["expected"] {
                tx.send(Err(format!(
                    "listener object case {i}: native={actual}, original={}",
                    row["expected"]
                )))
                .unwrap();
                return;
            }
        }
        tx.send(Ok(())).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(60))
        .expect("native listener callback failed or deadlocked")
        .unwrap();
}

#[test]
fn listener_object_fixture_original_sources_and_previous_corpora_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/container-listeners/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
    for (name, hash) in p["unchangedBaselines"].as_object().unwrap() {
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(std::fs::read(root.join(name)).unwrap())
            ),
            hash.as_str().unwrap(),
            "{name}"
        );
    }
}

struct Recorder {
    label: i32,
    calls: Shared<Vec<i32>>,
}
impl NativeListener for Recorder {}
fn recorder(label: i32, calls: &Shared<Vec<i32>>) -> Handle {
    Handle::new(Recorder {
        label,
        calls: calls.clone(),
    })
}
fn container() -> Container {
    Factory::new(&Registry::default())
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap()
}
struct Host {
    replacement: Option<Handle>,
    reenter: bool,
}
impl Context for Host {
    fn update(
        &mut self,
        listener: Handle,
        c: &mut Container,
        _: Event,
        d: &Registry,
    ) -> inventory::Result<()> {
        self.refresh(listener, c, d)
    }
    fn refresh(
        &mut self,
        listener: Handle,
        c: &mut Container,
        d: &Registry,
    ) -> inventory::Result<()> {
        let receiver = listener
            .value::<Recorder>()
            .expect("actual native receiver");
        receiver.calls.write().push(receiver.label);
        if let Some(replacement) = self.replacement.take() {
            c.listener_list().set(1, Some(replacement))?;
            if self.reenter {
                c.clone().refresh(None, d, self)?;
            }
        }
        Ok(())
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<item::Handle>,
        _: &Registry,
    ) -> inventory::Result<()> {
        panic!("this listener operation cannot overflow inventory")
    }
}

#[test]
fn equal_payloads_are_distinct_objects_and_unregister_uses_reference_identity() {
    let calls = Shared::new(vec![]);
    let first = recorder(7, &calls);
    let second = recorder(7, &calls);
    let other = recorder(7, &calls);
    let mut c = container();
    c.register(Some(first.clone()));
    c.register(Some(second.clone()));
    c.register(Some(first.clone()));
    assert!(!first.same(&second));
    assert!(!c.unregister(Some(other)));
    assert!(c.unregister(Some(first.clone())));
    assert_eq!(c.listeners(), vec![Some(second.clone()), Some(first)]);
    assert!(c.unregister(Some(second)));
    c.refresh(
        None,
        &Registry::default(),
        &mut Host {
            replacement: None,
            reenter: false,
        },
    )
    .unwrap();
    assert_eq!(*calls.read(), [7]);
}

#[test]
fn replacement_and_reentrant_traversals_dispatch_exact_live_native_receivers() {
    let calls = Shared::new(vec![]);
    let first = recorder(1, &calls);
    let second = recorder(2, &calls);
    let third = recorder(3, &calls);
    let weak_second = second.downgrade();
    let mut c = container();
    c.register(Some(first));
    c.register(Some(second));
    c.refresh(
        None,
        &Registry::default(),
        &mut Host {
            replacement: Some(third),
            reenter: true,
        },
    )
    .unwrap();
    assert_eq!(*calls.read(), [1, 1, 3, 3]);
    assert!(weak_second.upgrade().is_none());
}

#[test]
fn exposed_list_retains_actual_listener_values_after_container_retirement() {
    let calls = Shared::new(vec![]);
    let listener = recorder(4, &calls);
    let weak = listener.downgrade();
    let mut c = container();
    c.register(Some(listener));
    let view = c.listener_list();
    drop(c);
    let captured = view.get(0).unwrap().unwrap();
    view.clear();
    assert!(weak.upgrade().unwrap().same(&captured));
    captured.value::<Recorder>().unwrap().calls.write().push(4);
    assert_eq!(*calls.read(), [4]);
    drop(captured);
    assert!(weak.upgrade().is_none());
    assert!(view.is_empty());
}

#[test]
fn typed_receiver_capture_retains_the_same_object_allocation() {
    let calls = Shared::new(vec![]);
    let listener = recorder(9, &calls);
    let weak = listener.downgrade();
    let identity = listener.identity();
    let receiver = listener.value::<Recorder>().unwrap();
    drop(listener);
    let upgraded = weak.upgrade().unwrap();
    assert_eq!(upgraded.identity(), identity);
    assert!(std::sync::Arc::ptr_eq(
        &receiver,
        &upgraded.value::<Recorder>().unwrap()
    ));
    drop(upgraded);
    receiver.calls.write().push(receiver.label);
    assert_eq!(*calls.read(), [9]);
    drop(receiver);
    assert!(weak.upgrade().is_none());
}
