//! NPC timers in the live world (world::live::timers) and the isolation of a
//! failing NPC in its tick (world::live::faults), in-process over the booted
//! world (`world::live::boot`, as `rs09-server --world` boots it).
//!
//! Needs the pinned cache (`assets/cache`) and the world data
//! (`runs/startup-loaders-oracle/template`, or `RS09_STARTUP_WORLD`);
//! without them the test prints SKIPPED and checks nothing.
use rs09_server::{
    timers::content::{self, Arg, Kind},
    world::{
        driver::Host as _,
        live::{
            P,
            boot::{self, Options},
            combat::death,
            host::Live,
        },
        slayer_tasks,
    },
};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn inputs() -> Option<(PathBuf, PathBuf)> {
    let cache = std::env::var_os("RS09_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("assets/cache"));
    let data = std::env::var_os("RS09_STARTUP_WORLD")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("runs/startup-loaders-oracle/template"));
    (cache.join("main_file_cache.dat2").exists()
        && data.join("worldprops/default.conf").exists()
        && data.join("data/configs/npc_spawns.json").exists())
    .then_some((cache, data))
}

fn booted(name: &str) -> Option<Live> {
    let Some((cache, data)) = inputs() else {
        eprintln!("SKIPPED: needs assets/cache and the start-up world data");
        return None;
    };
    let saves = std::env::temp_dir().join(format!(
        "rs09-live-npc-timers-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&saves);
    let (sender, _receiver) = std::sync::mpsc::channel();
    let booted = boot::boot(
        &Options {
            data,
            cache,
            saves,
            verbose: false,
        },
        sender,
    )
    .expect("the live world boots");
    Some(booted.live)
}

/// Plain start-up NPCs (no subclass, no slayer task, not poison immune, at
/// least 30 hitpoints, so a full poison does not kill them).
fn plain(live: &mut Live, count: usize) -> Vec<P> {
    let nodes: Vec<usize> = live
        .inner
        .npcs
        .rows
        .iter()
        .filter(|(_, row)| {
            row.class.is_none()
                && row.static_levels[3] >= 30
                && slayer_tasks::level_req(row.id).is_none()
        })
        .map(|(node, _)| *node)
        .collect();
    let mut found = Vec::new();
    for node in nodes {
        let npc = live.combat_entity(node).expect("an NPC handle");
        if !live.npc_poison_immune(&npc).expect("poison immunity") {
            found.push(npc);
        }
        if found.len() == count {
            break;
        }
    }
    assert_eq!(found.len(), count, "enough plain NPCs");
    found
}

/// The victim and a source.
fn plain_npcs(live: &mut Live) -> (P, P) {
    let mut found = plain(live, 2);
    let source = found.pop().unwrap();
    (found.pop().unwrap(), source)
}

/// One world tick for the NPC's timers: the clock moves, then
/// `processTimers` (its region need not be active).
fn tick_timers(live: &mut Live, npc: &P) {
    live.inner.ticks += 1;
    live.process_npc_timers(npc).expect("processTimers");
}

fn has(live: &Live, npc: &P, identifier: &str) -> bool {
    live.has_timer_of(npc.node(), identifier)
        .expect("timer query")
}

#[test]
fn npc_poison_hits_and_wears_off_and_skill_restore_is_joined() {
    let Some(mut live) = booted("poison") else {
        return;
    };
    let (victim, source) = plain_npcs(&mut live);
    let node = victim.node();
    let maximum = live.inner.npcs.rows[&node].static_levels[3];
    assert_eq!(live.inner.npcs.rows[&node].lifepoints, maximum);
    // The start-up's automatic timer is there before the live world joins.
    assert!(has(&live, &victim, "GE periodic poll"));
    assert!(!has(&live, &victim, "skillrestore"));

    // applyPoison from a swing (severity 10: 2, 2, 2, 2, 2, 1, 1, 1, 1, 1).
    death::apply_poison(&mut live, &victim, &source, 10).expect("applyPoison on an NPC");
    assert!(has(&live, &victim, "poison"));
    // The first timer operation registered SkillRestore (Entity.init's
    // fourth automatic timer).
    assert!(has(&live, &victim, "skillrestore"));
    // A second, weaker poison changes nothing.
    death::apply_poison(&mut live, &victim, &source, 4).expect("weaker poison");

    // The first process moves Poison to the active list, due in 30 ticks.
    tick_timers(&mut live, &victim);
    for _ in 1..30 {
        tick_timers(&mut live, &victim);
    }
    assert_eq!(live.inner.npcs.rows[&node].lifepoints, maximum);
    tick_timers(&mut live, &victim);
    assert_eq!(live.inner.npcs.rows[&node].lifepoints, maximum - 2);

    // Ten hits in all, 15 damage; SkillRestore heals 1 every 100 ticks
    // meanwhile (its hitpoints counter is due at its 101st run, 202nd, ...).
    let mut runs = 31;
    while has(&live, &victim, "poison") {
        tick_timers(&mut live, &victim);
        runs += 1;
        assert!(runs < 400, "the poison wears off");
    }
    let heals = (1..=runs).filter(|run| run % 101 == 0).count() as i32;
    assert_eq!(
        live.inner.npcs.rows[&node].lifepoints,
        maximum - 15 + heals,
        "after {runs} runs"
    );
}

#[test]
fn skill_restore_heals_and_restores_an_npc_and_the_ge_timer_leaves() {
    let Some(mut live) = booted("restore") else {
        return;
    };
    let (npc, _) = plain_npcs(&mut live);
    let node = npc.node();
    let (maximum, attack) = {
        let row = live.inner.npcs.rows.get_mut(&node).unwrap();
        row.lifepoints = row.static_levels[3] - 3;
        row.levels[0] = row.static_levels[0] - 2;
        (row.static_levels[3], row.static_levels[0])
    };
    // 202 runs: two restorations of each skill.
    for _ in 0..203 {
        tick_timers(&mut live, &npc);
    }
    let row = &live.inner.npcs.rows[&node];
    assert_eq!(row.lifepoints, maximum - 1);
    assert_eq!(row.levels[0], attack);
    // GrandExchangeTimer.run answers false for an NPC: removed at its first
    // run, 500 ticks after its first process.
    assert!(has(&live, &npc, "GE periodic poll"));
    for _ in 203..501 {
        tick_timers(&mut live, &npc);
    }
    assert!(!has(&live, &npc, "GE periodic poll"));
    assert!(has(&live, &npc, "skillrestore"));
}

#[test]
fn a_frozen_npc_answers_the_walking_queue_and_death_clears_it() {
    let Some(mut live) = booted("frozen") else {
        return;
    };
    let (npc, _) = plain_npcs(&mut live);
    live.with_timers(&npc, |natives, manager| {
        let frozen = content::spawn(
            natives,
            manager,
            Kind::Frozen,
            &[Arg::Int(5), Arg::Bool(true)],
        )?;
        manager.register(Some(&frozen), &mut (), natives)
    })
    .expect("Frozen on an NPC");
    assert!(has(&live, &npc, "frozen"));
    // Frozen runs once after 5 ticks, spawning FrozenImmunity(7).
    for _ in 0..7 {
        tick_timers(&mut live, &npc);
    }
    assert!(!has(&live, &npc, "frozen"));
    assert!(has(&live, &npc, "frozen:immunity"));
    // A freeze while immune is removed again before it is processed.
    live.with_timers(&npc, |natives, manager| {
        let frozen = content::spawn(natives, manager, Kind::Frozen, &[Arg::Int(5)])?;
        manager.register(Some(&frozen), &mut (), natives)
    })
    .expect("a second freeze");
    assert!(!has(&live, &npc, "frozen"));
    // TimerManager.onEntityDeath removes the active ClearOnDeath timers
    // (Poison is one); a timer still pending stays, as in the original.
    let source = npc.clone();
    death::apply_poison(&mut live, &npc, &source, 5).expect("poison");
    live.npc_timers_on_death(&npc).expect("onEntityDeath");
    assert!(has(&live, &npc, "poison"), "pending: not cleared");
    tick_timers(&mut live, &npc);
    live.npc_timers_on_death(&npc).expect("onEntityDeath");
    assert!(!has(&live, &npc, "poison"));
}

#[test]
fn a_failing_npc_tick_is_isolated_and_the_world_keeps_ticking() {
    let Some(mut live) = booted("isolation") else {
        return;
    };
    // A handle of a node that is no NPC: its tick fails every time.
    let (npc, _) = plain_npcs(&mut live);
    let bogus = {
        let mut handle = npc.0.clone();
        handle.node = usize::MAX - 7;
        P(handle)
    };
    for _ in 0..3 {
        live.npc_tick(&bogus).expect("the failure is isolated");
    }
    let fault = live.npc_faults.fault(bogus.node()).expect("recorded");
    assert_eq!(fault.count, 3);
    assert!(
        live.inner
            .log
            .iter()
            .any(|l| l.contains("tick failed at tick")),
        "the first failure is logged"
    );
    // A healthy NPC's tick records nothing.
    live.npc_tick(&npc).expect("a healthy NPC");
    assert!(live.npc_faults.fault(npc.node()).is_none());
    // A whole world tick with the failing NPC first in the renderable list
    // and three NPCs after it: they still tick (an NPC whose region is
    // inactive takes itself out of the renderable list in its tick).
    let after_it = plain(&mut live, 4).split_off(1);
    let renderable = live.repository.renderable_npcs();
    renderable.push_direct(bogus.clone());
    for npc in &after_it {
        renderable.push_direct(npc.clone());
    }
    let driver = live.driver.clone();
    driver.tick(&mut live, false).expect("the tick completes");
    let after = live.repository.renderable_npcs().values();
    for npc in &after_it {
        assert!(
            !after.contains(npc),
            "NPC {} after the failing one ticked",
            npc.node()
        );
    }
    // Its tick (and its reset) failed again, isolated.
    assert!(live.npc_faults.fault(bogus.node()).unwrap().count >= 4);
}
