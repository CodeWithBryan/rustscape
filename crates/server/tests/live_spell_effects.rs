//! The combat spells' `fireEffect` over the booted live world
//! (`combat_pulse::Host::spell_fire_effect` of `world::live`, answered by
//! `spells::effects` through the live spell host): what each effect leaves in
//! an NPC victim's row (levels, lifepoints) and timer manager. The casters
//! here are NPCs too, so the effects that write to the caster as a player
//! (the binds' `entangleDelay`, Teleblock) are covered by the `effects`
//! scenario of `live_world` and by `spell_effects`.
//!
//! Needs the pinned cache (`assets/cache`) and the world data
//! (`runs/startup-loaders-oracle/template`, or `RS09_STARTUP_WORLD`);
//! without them the test prints SKIPPED and checks nothing.
use rs09_server::world::{
    combat_pulse::Host as _,
    combat_swing::{BattleState, CombatStyle},
    live::{
        P,
        boot::{self, Options},
        host::Live,
        magic::SpellKey,
    },
    slayer_tasks,
    spells::{SpellBook, SpellClass, SpellType},
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
        "rs09-live-spell-effects-{name}-{}",
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

/// Plain start-up NPCs (no subclass, no slayer task, not poison immune)
/// with at least 30 hitpoints and Attack, Defence and Magic levels a 5%
/// drain lowers.
fn plain(live: &mut Live, count: usize) -> Vec<P> {
    let mut nodes: Vec<usize> = live
        .inner
        .npcs
        .rows
        .iter()
        .filter(|(_, row)| {
            row.class.is_none()
                && row.static_levels[3] >= 30
                && row.static_levels[0] >= 20
                && row.static_levels[1] >= 20
                && row.static_levels[6] >= 20
                && row.levels[..7] == row.static_levels[..7]
                && slayer_tasks::level_req(row.id).is_none()
        })
        .map(|(node, _)| *node)
        .collect();
    nodes.sort_unstable();
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

/// The registered spell of a class and type.
fn key(live: &Live, class: SpellClass, kind: SpellType) -> SpellKey {
    let mut found: Vec<SpellKey> = live
        .round
        .magic
        .registry
        .iter()
        .filter(|(_, s)| s.class == class && s.kind == kind)
        .map(|(k, _)| *k)
        .collect();
    found.sort_by_key(|k| k.1);
    *found.first().expect("a registered spell")
}

/// `spell.fireEffect(entity, victim, state)` with a magic state of that
/// estimated hit; the state after it.
fn fire(live: &mut Live, spell: SpellKey, entity: &P, victim: &P, hit: i32) -> BattleState<Live> {
    let mut state = BattleState::new(Some(entity.clone()), Some(victim.clone()));
    state.estimated_hit = hit;
    state.spell = Some(spell);
    state.style = Some(CombatStyle::Magic);
    live.spell_fire_effect(&spell, entity, Some(victim), &mut state)
        .unwrap_or_else(|e| panic!("fireEffect of {spell:?}: {e:?}"));
    state
}

fn has(live: &Live, npc: &P, identifier: &str) -> bool {
    live.has_timer_of(npc.node(), identifier)
        .expect("timer query")
}

fn level(live: &Live, npc: &P, slot: usize) -> i32 {
    live.inner.npcs.rows[&npc.node()].levels[slot]
}

fn fixed(live: &Live, npc: &P, slot: usize) -> i32 {
    live.inner.npcs.rows[&npc.node()].static_levels[slot]
}

#[test]
fn curses_and_god_spells_drain_an_npc() {
    let Some(mut live) = booted("drains") else {
        return;
    };
    let mut npcs = plain(&mut live, 4);
    let caster = npcs.pop().unwrap();

    // Confuse: 5% of the current Attack, never below 95% of the static level.
    let victim = npcs.pop().unwrap();
    let attack = fixed(&live, &victim, 0);
    let confuse = key(&live, SpellClass::Curse, SpellType::Confuse);
    let state = fire(&mut live, confuse, &caster, &victim, -1);
    assert_eq!(state.estimated_hit, -1, "a splash stays a splash");
    assert_eq!(level(&live, &victim, 0), attack, "a splash drains nothing");
    let state = fire(&mut live, confuse, &caster, &victim, 0);
    assert_eq!(state.estimated_hit, -2, "a curse shows no hit");
    let floor = (f64::from(attack) * 0.95) as i32;
    let once = (attack - (f64::from(attack) * 0.05) as i32).max(floor);
    assert_eq!(level(&live, &victim, 0), once);
    fire(&mut live, confuse, &caster, &victim, 0);
    assert_eq!(level(&live, &victim, 0), floor, "the drain stops at 95%");

    // Stun drains 10% of Attack down to 90%; Vulnerability Defence.
    let stun = key(&live, SpellClass::Curse, SpellType::Stun);
    fire(&mut live, stun, &caster, &victim, 0);
    fire(&mut live, stun, &caster, &victim, 0);
    assert_eq!(level(&live, &victim, 0), (f64::from(attack) * 0.9) as i32);
    let defence = fixed(&live, &victim, 1);
    let vulnerability = key(&live, SpellClass::Curse, SpellType::Vulnerability);
    fire(&mut live, vulnerability, &caster, &victim, 0);
    assert_eq!(
        level(&live, &victim, 1),
        defence - (f64::from(defence) * 0.1) as i32
    );

    // The god spells drain whatever the hit: Claws of Guthix Defence,
    // Flames of Zamorak Magic; Saradomin Strike's prayer drain of an NPC is
    // a named skip and changes no level.
    let victim = npcs.pop().unwrap();
    let before = live.inner.npcs.rows[&victim.node()].levels;
    fire(&mut live, (SpellBook::Modern, 41), &caster, &victim, 10);
    assert_eq!(live.inner.npcs.rows[&victim.node()].levels, before);
    let defence = fixed(&live, &victim, 1);
    fire(&mut live, (SpellBook::Modern, 42), &caster, &victim, -1);
    assert_eq!(
        level(&live, &victim, 1),
        defence - (f64::from(defence) * 0.05) as i32
    );
    let magic = fixed(&live, &victim, 6);
    fire(&mut live, (SpellBook::Modern, 43), &caster, &victim, 10);
    assert_eq!(
        level(&live, &victim, 6),
        magic - (f64::from(magic) * 0.05) as i32
    );

    // Shadow: the truncated tenth of the static Attack each hit, down to
    // the truncated nine tenths.
    let victim = npcs.pop().unwrap();
    let attack = fixed(&live, &victim, 0);
    let shadow = key(&live, SpellClass::Shadow, SpellType::Rush);
    fire(&mut live, shadow, &caster, &victim, -1);
    assert_eq!(level(&live, &victim, 0), attack);
    let tenth = f64::from(attack) * 0.1;
    let floor = (f64::from(attack) - tenth) as i32;
    fire(&mut live, shadow, &caster, &victim, 3);
    assert_eq!(level(&live, &victim, 0), (attack - tenth as i32).max(floor));
    fire(&mut live, shadow, &caster, &victim, 3);
    fire(&mut live, shadow, &caster, &victim, 3);
    assert_eq!(level(&live, &victim, 0), floor, "not below nine tenths");
}

#[test]
fn ancient_spells_freeze_heal_poison_and_slow_an_npc() {
    let Some(mut live) = booted("ancients") else {
        return;
    };
    let mut npcs = plain(&mut live, 3);
    let caster = npcs.pop().unwrap();
    let victim = npcs.pop().unwrap();

    // Ice: a splash freezes nothing; a hit registers Frozen; a Barrage on a
    // frozen victim only marks the state.
    let rush = key(&live, SpellClass::Ice, SpellType::Rush);
    fire(&mut live, rush, &caster, &victim, -1);
    assert!(!has(&live, &victim, "frozen"));
    let state = fire(&mut live, rush, &caster, &victim, 0);
    assert!(has(&live, &victim, "frozen"));
    assert!(!state.frozen);
    let state = fire(&mut live, rush, &caster, &victim, 5);
    assert!(!state.frozen, "only a Barrage marks the state");
    let barrage = key(&live, SpellClass::Ice, SpellType::Barrage);
    let state = fire(&mut live, barrage, &caster, &victim, 5);
    assert!(state.frozen);
    // Rush's eight ticks pass, then the immunity: still no second freeze.
    for _ in 0..9 {
        live.inner.ticks += 1;
        live.process_npc_timers(&victim).expect("processTimers");
    }
    assert!(!has(&live, &victim, "frozen"));
    assert!(has(&live, &victim, "frozen:immunity"));
    let state = fire(&mut live, barrage, &caster, &victim, 5);
    assert!(state.frozen);
    assert!(!has(&live, &victim, "frozen"));

    // Blood: a quarter of the hit heals the caster, up to its maximum.
    let blood = key(&live, SpellClass::Blood, SpellType::Barrage);
    let node = caster.node();
    let maximum = live.inner.npcs.rows[&node].static_levels[3];
    live.inner.npcs.rows.get_mut(&node).unwrap().lifepoints = maximum - 10;
    fire(&mut live, blood, &caster, &victim, 3);
    assert_eq!(live.inner.npcs.rows[&node].lifepoints, maximum - 10);
    fire(&mut live, blood, &caster, &victim, 21);
    assert_eq!(live.inner.npcs.rows[&node].lifepoints, maximum - 5);
    fire(&mut live, blood, &caster, &victim, 29);
    fire(&mut live, blood, &caster, &victim, 29);
    assert_eq!(live.inner.npcs.rows[&node].lifepoints, maximum);

    // Smoke: poison from a hit only.
    let victim = npcs.pop().unwrap();
    let smoke = key(&live, SpellClass::Smoke, SpellType::Blitz);
    fire(&mut live, smoke, &caster, &victim, -1);
    assert!(!has(&live, &victim, "poison"));
    fire(&mut live, smoke, &caster, &victim, 0);
    assert!(has(&live, &victim, "poison"));

    // Miasmic: its timer whatever the hit. Teleblock does nothing to an NPC.
    let miasmic = key(&live, SpellClass::Miasmic, SpellType::Rush);
    fire(&mut live, miasmic, &caster, &victim, -1);
    assert!(has(&live, &victim, "miasmic"));
    let teleblock = key(&live, SpellClass::Teleblock, SpellType::Teleblock);
    fire(&mut live, teleblock, &caster, &victim, 0);
    assert!(!has(&live, &victim, "teleblock"));
}
