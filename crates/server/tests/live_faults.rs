//! The live world's isolation of a failing NPC (world::live::faults) and the
//! slayer task table NPC poison reads (world::slayer_tasks).
//!
//! The update sequence below is the verified `world::update::Sequence`
//! (`UpdateSequence`) over a recording context: once as the original runs
//! it (a failing NPC ends the sequence for every entity after it), once with
//! each NPC's tick and reset isolated exactly as the live binding does
//! (`Live`'s `npc_tick`/`npc_reset` through `faults::isolate`).
use rs09_server::world::{
    live::faults::{EntityFaults, isolate},
    slayer_tasks,
    update::{self, Context, Initialization, List, Metric, Sequence},
};

/// Records the calls of one sequence; NPC 2 fails its tick, and its reset
/// unless `reset_fails` is off. The isolated faults are the live binding's:
/// one record for ticks, one for resets.
struct Recorder {
    calls: Vec<String>,
    failing: usize,
    reset_fails: bool,
    isolated: Option<(EntityFaults, EntityFaults)>,
    logged: Vec<String>,
    tick: i32,
    players: List<usize>,
    npcs: List<usize>,
}

impl Recorder {
    fn new(isolated: bool) -> Self {
        let players = List::plain();
        players.add(100);
        players.add(101);
        let npcs = List::plain();
        for n in 1..=4 {
            npcs.add(n);
        }
        Self {
            calls: Vec::new(),
            failing: 2,
            reset_fails: true,
            isolated: isolated.then(Default::default),
            logged: Vec::new(),
            tick: 1,
            players,
            npcs,
        }
    }
    fn step(&mut self, what: &str, node: usize) -> update::Result<()> {
        self.calls.push(format!("{what} {node}"));
        let result = if node == self.failing && (what == "tick" || self.reset_fails) {
            Err(update::Error::Binding(format!("refused: {what} of {node}")))
        } else {
            Ok(())
        };
        match self.isolated.as_mut() {
            None => result,
            Some((ticks, resets)) => {
                let faults = if what == "reset" { resets } else { ticks };
                let logged = &mut self.logged;
                isolate(
                    faults,
                    node,
                    self.tick,
                    || format!("NPC {node}: {what}"),
                    result,
                    |line| logged.push(line),
                );
                Ok(())
            }
        }
    }
}

impl Initialization<usize> for Recorder {
    fn active(&mut self, _: &usize, _: bool) -> update::Result<()> {
        Ok(())
    }
    fn renderable(&mut self, _: &usize, _: bool) -> update::Result<()> {
        Ok(())
    }
}

impl Context<usize> for Recorder {
    fn lobby(&mut self) -> update::Result<List<usize>> {
        Ok(List::plain())
    }
    fn players(&mut self) -> update::Result<List<usize>> {
        Ok(self.players.clone())
    }
    fn npcs(&mut self) -> update::Result<List<usize>> {
        Ok(self.npcs.clone())
    }
    fn clear_minimap(&mut self, _: usize) -> update::Result<()> {
        Ok(())
    }
    fn npc_tick(&mut self, node: usize) -> update::Result<()> {
        self.step("tick", node)
    }
    fn player_tick(&mut self, node: usize) -> update::Result<()> {
        self.calls.push(format!("player tick {node}"));
        Ok(())
    }
    fn player_update(&mut self, node: usize) -> update::Result<()> {
        self.calls.push(format!("player update {node}"));
        Ok(())
    }
    fn player_reset(&mut self, node: usize) -> update::Result<()> {
        self.calls.push(format!("player reset {node}"));
        Ok(())
    }
    fn npc_reset(&mut self, node: usize) -> update::Result<()> {
        self.step("reset", node)
    }
    fn now_millis(&mut self) -> update::Result<i64> {
        Ok(0)
    }
    fn metric(&mut self, _: Metric, _: i32) -> update::Result<()> {
        Ok(())
    }
    fn reset_chunks(&mut self) -> update::Result<()> {
        Ok(())
    }
    fn ground_items(&mut self) -> update::Result<()> {
        Ok(())
    }
}

/// `handleTickActions`' start, run and end, stopping at the first error as
/// the original's catch does.
fn one_tick(ctx: &mut Recorder) -> update::Result<()> {
    let sequence = Sequence::default();
    sequence.start(ctx)?;
    sequence.run(ctx)?;
    sequence.end(ctx)
}

#[test]
fn unisolated_failing_npc_ends_the_tick_for_everyone_after_it() {
    let mut ctx = Recorder::new(false);
    assert!(one_tick(&mut ctx).is_err());
    // NPCs 3 and 4 and both players never tick or update.
    assert_eq!(ctx.calls, ["tick 1", "tick 2"]);
}

#[test]
fn isolated_failing_npc_is_logged_and_the_tick_goes_on() {
    let mut ctx = Recorder::new(true);
    one_tick(&mut ctx).expect("the sequence completes");
    assert_eq!(
        ctx.calls,
        [
            "tick 1",
            "tick 2",
            "tick 3",
            "tick 4",
            "player tick 100",
            "player tick 101",
            "player update 100",
            "player update 101",
            "player reset 100",
            "player reset 101",
            "reset 1",
            "reset 2",
            "reset 3",
            "reset 4",
        ]
    );
    assert_eq!(
        ctx.logged,
        [
            "NPC 2: tick failed at tick 1 (isolated; the tick goes on): Binding(\"refused: tick of 2\")",
            "NPC 2: reset failed at tick 1 (isolated; the tick goes on): Binding(\"refused: reset of 2\")",
        ]
    );
    let (ticks, resets) = ctx.isolated.as_ref().unwrap();
    assert_eq!((ticks.failing(), resets.failing()), (1, 1));
    assert_eq!(ticks.fault(2).unwrap().count, 1);
    assert_eq!(resets.fault(2).unwrap().count, 1);
}

#[test]
fn a_healthy_reset_does_not_end_a_failing_tick_run() {
    // The tick fails every world tick, the reset succeeds: the tick's run
    // goes on (logged at the 1st, 10th and 100th failure), and no recovery
    // is logged.
    let mut ctx = Recorder::new(true);
    ctx.reset_fails = false;
    for tick in 1..=250 {
        ctx.tick = tick;
        one_tick(&mut ctx).expect("the sequence completes");
    }
    assert_eq!(ctx.logged.len(), 3, "{:?}", ctx.logged);
    assert!(ctx.logged[2].starts_with("NPC 2: tick failed 100 times in a row since tick 1"));
    let (ticks, resets) = ctx.isolated.as_ref().unwrap();
    assert_eq!(ticks.fault(2).unwrap().count, 250);
    assert_eq!(resets.failing(), 0);
}

#[test]
fn failures_are_logged_at_1_10_100_and_when_the_error_changes() {
    let mut faults = EntityFaults::default();
    let mut logged = Vec::new();
    for tick in 1..=250 {
        if let Some(line) = faults.failed(7, tick, "NPC 7: tick", "refused: A") {
            logged.push((tick, line));
        }
    }
    let ticks: Vec<i32> = logged.iter().map(|(t, _)| *t).collect();
    assert_eq!(ticks, [1, 10, 100]);
    assert_eq!(
        logged[2].1,
        "NPC 7: tick failed 100 times in a row since tick 1 (isolated): refused: A"
    );
    // A different error is logged at once.
    let line = faults.failed(7, 251, "NPC 7: tick", "refused: B");
    assert!(line.is_some_and(|l| l.ends_with("refused: B")));
    assert_eq!(faults.fault(7).unwrap().error, "refused: B");
    // The same error again is not.
    assert!(faults.failed(7, 252, "NPC 7: tick", "refused: B").is_none());
    assert_eq!(faults.total, 252);
}

#[test]
fn a_success_after_failures_logs_one_recovery_and_forgets_the_entity() {
    let mut faults = EntityFaults::default();
    faults.failed(3, 10, "NPC 3: tick", "boom");
    faults.failed(3, 11, "NPC 3: tick", "boom");
    let line = faults
        .succeeded(3, 12, "NPC 3: tick")
        .expect("recovery line");
    assert_eq!(
        line,
        "NPC 3: tick recovered at tick 12 after 2 failures in a row (since tick 10)"
    );
    assert!(faults.fault(3).is_none());
    assert!(faults.succeeded(3, 13, "NPC 3: tick").is_none());
    // A new run starts again at the first failure.
    let again = faults.failed(3, 20, "NPC 3: tick", "boom");
    assert_eq!(
        again.as_deref(),
        Some("NPC 3: tick failed at tick 20 (isolated; the tick goes on): boom")
    );
}

#[test]
fn isolate_of_a_healthy_entity_logs_nothing_and_builds_no_label() {
    let mut faults = EntityFaults::default();
    let mut lines = Vec::new();
    isolate::<String>(
        &mut faults,
        1,
        1,
        || panic!("no label is built for a healthy entity"),
        Ok(()),
        |line| lines.push(line),
    );
    assert!(lines.is_empty());
    assert_eq!(faults.total, 0);
}

#[test]
fn slayer_task_table_is_tasks_for_id() {
    // 93 constants, 1,127 distinct ids.
    assert_eq!(slayer_tasks::TASKS.len(), 1127);
    assert!(
        slayer_tasks::TASKS.windows(2).all(|w| w[0].0 < w[1].0),
        "sorted by id, no repeats"
    );
    assert_eq!(slayer_tasks::for_id(1604), Some(("ABERRANT_SPECTRES", 60)));
    assert_eq!(slayer_tasks::level_req(1615), Some(85));
    // NPCs.KILLERWATT_3201
    assert_eq!(slayer_tasks::for_id(3201), Some(("KILLERWATTS", 37)));
    // putIfAbsent in declaration order: the first constant listing an id wins.
    assert_eq!(slayer_tasks::for_id(7640), Some(("CRAWLING_HAND", 5)));
    assert_eq!(slayer_tasks::for_id(7641), Some(("CRAWLING_HAND", 5)));
    assert_eq!(slayer_tasks::for_id(6608), Some(("HOBGOBLINS", 1)));
    assert_eq!(slayer_tasks::for_id(2044), Some(("OGRES", 1)));
    // An id no task lists (Hans, a man).
    assert_eq!(slayer_tasks::for_id(0), None);
    assert_eq!(slayer_tasks::level_req(1), None);
}
