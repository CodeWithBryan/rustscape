//! Original active/server varp distinction and hash-bucket timer iteration.
use std::collections::BTreeMap;
#[derive(Default)]
pub struct Pending {
    pub values: BTreeMap<i32, i32>,
    timers: Vec<(i32, i64)>,
}
impl Pending {
    pub fn server(&mut self, id: i32, value: i32, now_ms: i64) {
        self.values.insert(id, value);
        if let Some((_, deadline)) = self.timers.iter_mut().find(|(key, _)| *key == id) {
            if *deadline != 0x4000000000000001 {
                *deadline = now_ms.wrapping_add(500) | 0x4000000000000000;
            }
        } else {
            self.timers.push((id, 0x4000000000000001));
        }
    }
    pub fn client(&mut self, id: i32, now_ms: i64) {
        let deadline = now_ms.wrapping_add(500);
        if let Some((_, value)) = self.timers.iter_mut().find(|(key, _)| *key == id) {
            *value = deadline;
        } else {
            self.timers.push((id, deadline));
        }
    }
    pub fn poll(&mut self, active: &mut BTreeMap<i32, i32>, now_ms: i64) -> Vec<i32> {
        self.poll_with(active, now_ms, |_, _| {})
    }
    /// Observes each original poll return before the next pending value becomes
    /// active, preserving refreshMagicVarp's updateMulti ordering.
    pub fn poll_with(
        &mut self,
        active: &mut BTreeMap<i32, i32>,
        now_ms: i64,
        mut observed: impl FnMut(i32, &BTreeMap<i32, i32>),
    ) -> Vec<i32> {
        let mut changed = Vec::new();
        for bucket in 0..16 {
            self.timers.retain(|&(id, deadline)| {
                if id & 15 == bucket && (deadline & 0x3fffffffffffffff) < now_ms {
                    if deadline & 0x4000000000000000 != 0 {
                        active.insert(id, self.values.get(&id).copied().unwrap_or(0));
                        observed(id, active);
                        changed.push(id);
                    }
                    false
                } else {
                    true
                }
            });
        }
        changed
    }
}
