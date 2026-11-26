//! Isolation of one entity's failing tick.
//!
//! The original's `UpdateSequence.start` runs `npcList.forEach(NPC::tick)`
//! with no catch of its own: an exception from one NPC ends the rest of that
//! tick's sequence (the other NPCs, every player's tick and update, the
//! resets) in `MajorUpdateWorker.handleTickActions`' catch. The verified
//! driver keeps that (`world::driver`, `world::tick`). In this world most
//! failures are not the original's exceptions but refusals of services that
//! have no port yet, and a refusal an NPC reaches every tick would end every
//! tick's sequence at that NPC: the world stalls for everyone.
//!
//! The live binding therefore isolates an NPC's tick and reset
//! (`super::host`'s `npc_tick`/`npc_reset`): a failure is recorded against
//! that NPC, logged, and answered as done, so the sequence goes on with the
//! next entity. This is a deviation from the original (docs/live-world.md).
//!
//! The log is rate limited per entity: the first failure, then the 10th,
//! 100th, 1,000th... of a run of failures, and again whenever the error
//! text changes. A success after a run of failures logs one recovery line
//! and forgets the entity.
use std::collections::HashMap;

/// A run of consecutive failures of one entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    /// The world tick of the run's first failure.
    pub first_tick: i32,
    /// The world tick of the latest failure.
    pub last_tick: i32,
    /// Failures in the run.
    pub count: u64,
    /// The latest error text.
    pub error: String,
}

/// The failing entities, by node, with a running total.
#[derive(Clone, Debug, Default)]
pub struct EntityFaults {
    faults: HashMap<usize, Fault>,
    /// Every failure recorded since the world started.
    pub total: u64,
}

/// Whether the `count`th failure of a run is logged: 1, 10, 100, ...
fn milestone(count: u64) -> bool {
    let mut n = count;
    while n >= 10 && n % 10 == 0 {
        n /= 10;
    }
    n == 1
}

impl EntityFaults {
    /// Records a failure of `what` (for example `NPC 81 tick`) of `node` at
    /// `tick`; answers the line to log, if this failure is logged.
    pub fn failed(&mut self, node: usize, tick: i32, what: &str, error: &str) -> Option<String> {
        self.total = self.total.wrapping_add(1);
        let fault = self.faults.entry(node).or_insert_with(|| Fault {
            first_tick: tick,
            last_tick: tick,
            count: 0,
            error: String::new(),
        });
        let changed = fault.count > 0 && fault.error != error;
        fault.count = fault.count.wrapping_add(1);
        fault.last_tick = tick;
        fault.error = error.to_owned();
        if !(changed || milestone(fault.count)) {
            return None;
        }
        Some(if fault.count == 1 {
            format!("{what} failed at tick {tick} (isolated; the tick goes on): {error}")
        } else {
            format!(
                "{what} failed {} times in a row since tick {} (isolated): {error}",
                fault.count, fault.first_tick
            )
        })
    }

    /// Records a success of `node`; answers a recovery line if it was failing.
    pub fn succeeded(&mut self, node: usize, tick: i32, what: &str) -> Option<String> {
        if self.faults.is_empty() {
            return None;
        }
        let fault = self.faults.remove(&node)?;
        Some(format!(
            "{what} recovered at tick {tick} after {} failures in a row (since tick {})",
            fault.count, fault.first_tick
        ))
    }

    /// Records the outcome of one entity's step and answers what to log.
    pub fn record<E: std::fmt::Debug>(
        &mut self,
        node: usize,
        tick: i32,
        what: &str,
        result: &Result<(), E>,
    ) -> Option<String> {
        match result {
            Ok(()) => self.succeeded(node, tick, what),
            Err(error) => self.failed(node, tick, what, &format!("{error:?}")),
        }
    }

    /// The current run of failures of `node`.
    pub fn fault(&self, node: usize) -> Option<&Fault> {
        self.faults.get(&node)
    }

    /// The entities failing now.
    pub fn failing(&self) -> usize {
        self.faults.len()
    }

    /// Forgets an entity that left the world.
    pub fn forget(&mut self, node: usize) {
        self.faults.remove(&node);
    }
}

/// Runs one entity's step, records the outcome and hands any line to `log`;
/// the step's failure is not passed on.
pub fn isolate<E: std::fmt::Debug>(
    faults: &mut EntityFaults,
    node: usize,
    tick: i32,
    what: impl FnOnce() -> String,
    result: Result<(), E>,
    mut log: impl FnMut(String),
) {
    if result.is_ok() && faults.fault(node).is_none() {
        return;
    }
    if let Some(line) = faults.record(node, tick, &what(), &result) {
        log(line);
    }
}
