//! RT4 DelayedStateChange ordering, with an explicit monotonic millisecond clock.
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    pub kind: i32,
    pub id: i32,
    pub value: i32,
    pub extra: [i32; 2],
    pub text: Option<String>,
}
#[derive(Default)]
struct Entry {
    deadline: i64,
    server: bool,
    value: i32,
    extra: [i32; 2],
    text: Option<String>,
}
#[derive(Default)]
pub struct Queue {
    changes: BTreeMap<i64, Entry>,
    client: VecDeque<i64>,
    server: VecDeque<i64>,
}
impl Queue {
    fn key(kind: i32, id: i32) -> i64 {
        (i64::from(kind) << 32) | i64::from(id)
    }
    fn unlink(&mut self, key: i64) {
        self.client.retain(|&k| k != key);
        self.server.retain(|&k| k != key);
    }
    pub fn client(&mut self, kind: i32, id: i32, now_ms: i64) {
        let key = Self::key(kind, id);
        self.changes.entry(key).or_default().deadline = now_ms.wrapping_add(500) & i64::MAX;
        self.unlink(key);
        self.client.push_back(key);
    }
    pub fn server(&mut self, kind: i32, id: i32, value: i32, text: Option<String>) {
        self.server_values(kind, id, [value, 0, 0], text);
    }
    pub fn server_values(&mut self, kind: i32, id: i32, values: [i32; 3], text: Option<String>) {
        let key = Self::key(kind, id);
        let entry = self.changes.entry(key).or_default();
        entry.server = true;
        entry.value = values[0];
        entry.extra = [values[1], values[2]];
        entry.text = text;
        if entry.deadline == 0 {
            self.unlink(key);
            self.server.push_back(key);
        }
    }
    pub fn poll(&mut self, now_ms: i64) -> Option<Change> {
        loop {
            let key = if let Some(key) = self.server.pop_front() {
                key
            } else {
                let &key = self.client.front()?;
                if self.changes[&key].deadline > now_ms {
                    return None;
                }
                self.client.pop_front();
                key
            };
            let entry = self.changes.remove(&key).unwrap();
            if entry.server {
                return Some(Change {
                    kind: ((key as u64 >> 32) & 255) as i32,
                    id: key as i32,
                    value: entry.value,
                    extra: entry.extra,
                    text: entry.text,
                });
            }
        }
    }
}
