#[path = "../../../oracle/world-update/native.rs"]
mod native;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/world-update/oracle.jsonl");
#[test]
fn original_packet_repository_and_update_sequence_semantics() {
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        let actual = native::evaluate(&c);
        for (j, (actual, expected)) in actual
            .as_array()
            .unwrap()
            .iter()
            .zip(c["expected"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(
                actual, expected,
                "case {i}, action {j}: {}",
                c["actions"][j]
            );
        }
        assert_eq!(
            actual.as_array().unwrap().len(),
            c["expected"].as_array().unwrap().len()
        );
    }
}
#[test]
fn world_update_fixture_and_reference_sources_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/world-update/provenance.json"
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

#[test]
fn concurrent_packet_producers_keep_fifo_and_defer_mid_batch_arrivals() {
    use rs09_server::world::packet_queue::{Context, Packet, Queue};
    use std::{convert::Infallible, sync::mpsc, time::Duration};
    struct Consumer {
        starts: Vec<mpsc::SyncSender<()>>,
        done: mpsc::Receiver<()>,
        seen: [Vec<usize>; 4],
        first: bool,
    }
    impl Context<(usize, usize)> for Consumer {
        type Error = Infallible;
        fn process(&mut self, p: &(usize, usize)) -> Result<(), Infallible> {
            if self.first {
                self.first = false;
                for start in &self.starts {
                    start.send(()).unwrap();
                }
                for _ in 0..4 {
                    self.done.recv_timeout(Duration::from_secs(10)).unwrap();
                }
            }
            self.seen[p.0].push(p.1);
            Ok(())
        }
        fn exception_text(&mut self, e: Infallible) -> Result<String, Infallible> {
            match e {}
        }
        fn log(&mut self, _: &(usize, usize), _: &str) -> Result<(), Infallible> {
            unreachable!()
        }
    }
    let queue = Queue::default();
    let (ready, ready_rx) = mpsc::channel();
    let (done, done_rx) = mpsc::channel();
    std::thread::scope(|s| {
        let mut starts = vec![];
        for producer in 0..4 {
            let q = queue.clone();
            let ready = ready.clone();
            let done = done.clone();
            let (start, start_rx) = mpsc::sync_channel(1);
            starts.push(start);
            s.spawn(move || {
                for i in 0..500 {
                    q.enqueue(Packet::Dispatch((producer, i)));
                }
                ready.send(()).unwrap();
                start_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                for i in 500..1000 {
                    q.enqueue(Packet::Dispatch((producer, i)));
                }
                done.send(()).unwrap();
            });
        }
        for _ in 0..4 {
            ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        }
        let mut c = Consumer {
            starts,
            done: done_rx,
            seen: std::array::from_fn(|_| vec![]),
            first: true,
        };
        queue.process(&mut c).unwrap();
        for seen in &c.seen {
            assert_eq!(*seen, (0..500).collect::<Vec<_>>());
        }
        assert_eq!(queue.lengths(), (2000, 0));
        queue.process(&mut c).unwrap();
        for seen in &c.seen {
            assert_eq!(*seen, (0..1000).collect::<Vec<_>>());
        }
        assert_eq!(queue.lengths(), (0, 0));
    });
}

#[test]
fn global_no_process_releases_permits_from_both_buffers() {
    use rs09_server::world::packet_queue::{Context, Packet, Queue};
    use std::{convert::Infallible, sync::Arc};
    use tokio::sync::{OwnedSemaphorePermit, Semaphore};
    struct Budgeted {
        id: usize,
        _permit: OwnedSemaphorePermit,
    }
    struct Consumer {
        queue: Queue<Budgeted>,
        budget: Arc<Semaphore>,
        seen: Vec<usize>,
    }
    impl Context<Budgeted> for Consumer {
        type Error = Infallible;
        fn process(&mut self, p: &Budgeted) -> Result<(), Infallible> {
            self.seen.push(p.id);
            self.queue.enqueue(Packet::Dispatch(Budgeted {
                id: 2,
                _permit: self.budget.clone().try_acquire_owned().unwrap(),
            }));
            Ok(())
        }
        fn exception_text(&mut self, e: Infallible) -> Result<String, Infallible> {
            match e {}
        }
        fn log(&mut self, _: &Budgeted, _: &str) -> Result<(), Infallible> {
            unreachable!()
        }
    }
    let queue = Queue::default();
    let budget = Arc::new(Semaphore::new(4));
    queue.enqueue(Packet::Dispatch(Budgeted {
        id: 0,
        _permit: budget.clone().try_acquire_owned().unwrap(),
    }));
    queue.enqueue(Packet::NoProcess);
    queue.enqueue(Packet::Dispatch(Budgeted {
        id: 1,
        _permit: budget.clone().try_acquire_owned().unwrap(),
    }));
    let mut c = Consumer {
        queue: queue.clone(),
        budget: budget.clone(),
        seen: vec![],
    };
    queue.process(&mut c).unwrap();
    assert_eq!(c.seen, [0]);
    assert_eq!(queue.lengths(), (0, 0));
    assert_eq!(budget.available_permits(), 4);
}

#[test]
fn concurrent_initialization_producers_retain_all_deferred_nodes() {
    use rs09_server::world::update::{Initialization, List, Result};
    struct Host {
        active: Vec<usize>,
        renderable: Vec<usize>,
    }
    impl Initialization<usize> for Host {
        fn active(&mut self, n: &usize, b: bool) -> Result<()> {
            assert!(b);
            self.active.push(*n);
            Ok(())
        }
        fn renderable(&mut self, n: &usize, b: bool) -> Result<()> {
            assert!(b);
            self.renderable.push(*n);
            Ok(())
        }
    }
    let list = List::default();
    std::thread::scope(|s| {
        for producer in 0..4 {
            let l = list.clone();
            s.spawn(move || {
                for i in 0..512 {
                    assert!(l.add(producer * 512 + i));
                }
            });
        }
    });
    assert!(list.values().is_empty());
    assert_eq!(list.pending().len(), 2048);
    let mut h = Host {
        active: vec![],
        renderable: vec![],
    };
    list.sync(&mut h).unwrap();
    assert_eq!(h.active, h.renderable);
    assert_eq!(list.values(), h.active);
    h.active.sort_unstable();
    assert_eq!(h.active, (0..2048).collect::<Vec<_>>());
    assert!(!list.add(0));
    assert!(list.pending().is_empty());
}
