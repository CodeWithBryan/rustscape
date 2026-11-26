#[path = "../../../oracle/pulses/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/pulses/oracle.jsonl");
fn diff(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for k in a.keys().chain(b.keys()) {
            if let Some(d) = diff(
                a.get(k).unwrap_or(&Value::Null),
                b.get(k).unwrap_or(&Value::Null),
                format!("{path}/{k}"),
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
fn pulses_entity_manager_and_world_queue_match_original() {
    for (index, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&c);
        if let Some(d) = diff(&actual, &c["expected"], String::new()) {
            panic!("pulse case {index}: {d}; actions={}", c["actions"]);
        }
    }
}
#[test]
fn pulse_fixture_sources_and_world_boundary_adapters_are_pinned() {
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/pulses/provenance.json")).unwrap();
    assert_eq!(format!("{:x}", Sha256::digest(CORPUS)), p["fixtureSha256"]);
    assert_eq!(CORPUS.lines().count() as u64, p["caseCount"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (source, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(source);
        if path.is_file() {
            let data = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{source}"
            );
        }
    }
}

#[test]
fn parallel_producers_and_deferred_children_match_serial_queue_replay() {
    use rs09_server::{
        definitions::item::Shared,
        pulses::{Class, Context, Error, Handle, Level, Pulse, Result, Runner},
    };
    use std::sync::{Arc, Barrier};
    fn task(name: String) -> Handle {
        Shared::new(Pulse::new(0, None, Class::Other, name))
    }
    struct Bindings {
        runner: Runner,
        seen: Vec<String>,
        clock: i64,
    }
    impl Context<()> for Bindings {
        fn pulse(&mut self, p: &Handle, _: &mut ()) -> Result<bool> {
            let name = p.read().class_name.clone();
            self.seen.push(name.clone());
            if !name.starts_with("child:") && name.ends_with(":0") {
                self.runner.submit(Some(task(format!("child:{name}"))))?
            }
            Ok(true)
        }
        fn node_active(&mut self, _: usize, _: &mut ()) -> Result<bool> {
            Ok(true)
        }
        fn exception(&mut self, e: &Error, _: &mut ()) -> Result<()> {
            panic!("unexpected pulse error {e:?}")
        }
        fn remove_weak_scripts(&mut self, _: &mut ()) -> Result<()> {
            Ok(())
        }
        fn remove_normal_scripts(&mut self, _: &mut ()) -> Result<()> {
            Ok(())
        }
        fn is_dead(&mut self, _: &mut ()) -> Result<bool> {
            Ok(false)
        }
        fn send_message(&mut self, _: usize, _: &str, _: &mut ()) -> Result<()> {
            Ok(())
        }
        fn now_millis(&mut self, _: &mut ()) -> Result<i64> {
            self.clock += 1;
            Ok(self.clock)
        }
        fn log(&mut self, _: Level, _: String, _: &mut ()) -> Result<()> {
            Ok(())
        }
        fn pulse_length(&mut self, _: &str, _: i32, _: &mut ()) -> Result<()> {
            Ok(())
        }
        fn count_pulse(&mut self, _: &str, _: &mut ()) -> Result<()> {
            Ok(())
        }
        fn metrics_enabled(&self, _: &()) -> bool {
            false
        }
        fn set_bot_total(&mut self, _: i32, _: &mut ()) -> Result<()> {
            panic!("disabled metric write")
        }
        fn set_other_total(&mut self, _: i32, _: &mut ()) -> Result<()> {
            panic!("disabled metric write")
        }
    }
    let runner = Runner::default();
    let barrier = Arc::new(Barrier::new(5));
    let producers = (0..4)
        .map(|producer| {
            let runner = runner.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for sequence in 0..128 {
                    runner
                        .submit(Some(task(format!("{producer}:{sequence}"))))
                        .unwrap();
                }
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    for producer in producers {
        producer.join().unwrap();
    }
    let order = runner
        .current()
        .iter()
        .map(|p| p.read().class_name.clone())
        .collect::<Vec<_>>();
    assert_eq!(order.len(), 512);
    for producer in 0..4 {
        let prefix = format!("{producer}:");
        let actual = order
            .iter()
            .filter_map(|n| n.strip_prefix(&prefix).map(|n| n.parse::<usize>().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(actual, (0..128).collect::<Vec<_>>());
    }
    let serial = Runner::default();
    for name in &order {
        serial.submit(Some(task(name.clone()))).unwrap();
    }
    let mut parallel_bindings = Bindings {
        runner: runner.clone(),
        seen: Vec::new(),
        clock: 0,
    };
    let mut serial_bindings = Bindings {
        runner: serial.clone(),
        seen: Vec::new(),
        clock: 0,
    };
    runner.update_all(&mut (), &mut parallel_bindings).unwrap();
    serial.update_all(&mut (), &mut serial_bindings).unwrap();
    assert_eq!(parallel_bindings.seen, order);
    assert_eq!(parallel_bindings.seen, serial_bindings.seen);
    let children = runner
        .current()
        .iter()
        .map(|p| p.read().class_name.clone())
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 4);
    assert_eq!(
        children,
        serial
            .current()
            .iter()
            .map(|p| p.read().class_name.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(parallel_bindings.clock, 1024);
    runner.update_all(&mut (), &mut parallel_bindings).unwrap();
    serial.update_all(&mut (), &mut serial_bindings).unwrap();
    assert_eq!(parallel_bindings.seen, serial_bindings.seen);
    assert_eq!(parallel_bindings.seen.len(), 516);
    assert!(runner.current().is_empty());
    assert!(serial.current().is_empty());
}
