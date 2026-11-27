//! `fireEffect` of the combat spell classes (`world::spells::effects`)
//! against a recording host: the calls each class makes, in order, and what
//! it writes to the battle state. The expectations are written from the
//! pinned `BindSpell`, `CurseSpells`, `GodSpells`, `TeleblockSpell`,
//! `IceSpells`, `BloodSpells`, `ShadowSpells`, `SmokeSpells` and
//! `MiasmicSpells`; no JVM corpus covers these members.
use rs09_server::world::{
    combat_swing::CombatStyle,
    spells::{
        Animation, Error, Graphics, Host, NodeKind, Projectile, ProjectileTarget, Result, Spell,
        SpellBook, SpellClass, SpellType, Value,
        effects::{self, EffectState, Effects, TimerArg},
        table,
    },
};
use std::collections::HashMap;

const PLAYER: usize = 0;
const NPC: usize = 1;
const OTHER: usize = 2;

type Key = (SpellBook, i32);

struct World {
    registry: HashMap<Key, Spell>,
    calls: Vec<String>,
    timers: Vec<(usize, &'static str)>,
    movement_locked: bool,
    accurate: bool,
    protect: bool,
    npc_name: Option<String>,
    attack: i32,
    book: i32,
}

impl World {
    fn new() -> Self {
        Self {
            registry: table::combat_spells()
                .into_iter()
                .map(|(book, button, spell)| ((book, button), spell))
                .collect(),
            calls: Vec::new(),
            timers: Vec::new(),
            movement_locked: false,
            accurate: true,
            protect: false,
            npc_name: Some("Chicken".into()),
            attack: 60,
            book: 0,
        }
    }
    /// The registered spell of a class and type.
    fn key(&self, class: SpellClass, kind: SpellType) -> Key {
        let mut found: Vec<Key> = self
            .registry
            .iter()
            .filter(|(_, s)| s.class == class && s.kind == kind)
            .map(|(k, _)| *k)
            .collect();
        found.sort_by_key(|k| k.1);
        *found.first().expect("a registered spell")
    }
    /// `fireEffect` with a state of that estimated hit; the result, the
    /// state after it and the calls made.
    fn fire(
        &mut self,
        key: Key,
        entity: usize,
        victim: Option<usize>,
        hit: i32,
    ) -> (Result<()>, EffectState<Key>, Vec<String>) {
        let mut state = EffectState {
            estimated_hit: hit,
            frozen: false,
            spell: Some(key),
            style: Some(CombatStyle::Magic),
        };
        self.calls.clear();
        let result = effects::fire_effect(self, &key, &entity, victim.as_ref(), &mut state);
        (result, state, std::mem::take(&mut self.calls))
    }
}

fn unused<T>(what: &str) -> T {
    panic!("fireEffect does not call {what}")
}

impl Host for World {
    type Node = usize;
    type Spell = Key;

    fn kind(&self, node: &usize) -> NodeKind {
        match *node {
            PLAYER | OTHER => NodeKind::Player,
            _ => NodeKind::Npc,
        }
    }
    fn same_node(&self, a: &usize, b: &usize) -> bool {
        a == b
    }
    fn same_spell(&self, a: &Key, b: &Key) -> bool {
        a == b
    }
    fn spell(&mut self, spell: &Key) -> &mut Spell {
        self.registry.get_mut(spell).expect("a registered spell")
    }
    fn registered(&self, book: SpellBook, button: i32) -> Option<Key> {
        self.registry
            .contains_key(&(book, button))
            .then_some((book, button))
    }
    fn item_name(&self, _: i32) -> String {
        unused("item names")
    }
    fn experience_mod(&mut self) -> f64 {
        unused("EXPERIENCE_MOD")
    }
    fn spell_book(&mut self, _: &usize) -> &mut i32 {
        &mut self.book
    }
    fn staff_interface(&mut self, _: &usize) -> bool {
        unused("the weapon interface")
    }
    fn set_god_cast(&mut self, _: &usize, _: usize, _: i32) {
        unused("god casts")
    }
    fn ticks(&mut self) -> Result<i32> {
        self.calls.push("ticks".into());
        Ok(1000)
    }
    fn dev_mode(&mut self) -> Result<bool> {
        unused("dev mode")
    }
    fn name(&mut self, node: &usize) -> Result<Option<String>> {
        self.calls.push(format!("name({node})"));
        Ok(self.npc_name.clone())
    }
    fn location(&mut self, _: &usize) -> Result<Option<[i32; 3]>> {
        unused("locations")
    }
    fn level(&mut self, _: &usize, _: i32) -> Result<i32> {
        unused("levels")
    }
    fn level_with(&mut self, _: &usize, _: i32, _: bool) -> Result<i32> {
        unused("levels")
    }
    fn add_experience(&mut self, _: &usize, _: i32, _: f64, _: bool) -> Result<()> {
        unused("experience")
    }
    fn add_experience_default(&mut self, _: &usize, _: i32, _: f64) -> Result<()> {
        unused("experience")
    }
    fn equipment(&mut self, _: &usize, _: i32) -> Result<Option<i32>> {
        unused("equipment")
    }
    fn equipment_new(&mut self, _: &usize, _: i32) -> Result<i32> {
        unused("equipment")
    }
    fn inventory_amount(&mut self, _: &usize, _: i32) -> Result<i32> {
        unused("the inventory")
    }
    fn inventory_remove(&mut self, _: &usize, _: i32, _: i32) -> Result<bool> {
        unused("the inventory")
    }
    fn send_message(&mut self, player: &usize, text: &str) -> Result<()> {
        self.calls.push(format!("sendMessage({player}, {text})"));
        Ok(())
    }
    fn packet_message(&mut self, player: &usize, text: &str) -> Result<()> {
        self.calls.push(format!("packetMessage({player}, {text})"));
        Ok(())
    }
    fn interface_config(&mut self, _: &usize, _: i32, _: i32, _: bool) -> Result<()> {
        unused("interface configs")
    }
    fn dispatch_cast(&mut self, _: &usize, _: SpellBook, _: i32, _: Option<&usize>) -> Result<()> {
        unused("the cast event")
    }
    fn face_location(&mut self, _: &usize, _: Option<[i32; 3]>) -> Result<()> {
        unused("facing")
    }
    fn face_temporary(&mut self, _: &usize, _: Option<&usize>, _: i32) -> Result<()> {
        unused("facing")
    }
    fn face(&mut self, _: &usize, _: Option<&usize>) -> Result<()> {
        unused("facing")
    }
    fn graphics(&mut self, _: &usize, _: Option<Graphics>) -> Result<()> {
        unused("graphics")
    }
    fn animate(&mut self, _: &usize, _: Option<Animation>) -> Result<()> {
        unused("animations")
    }
    fn force_animation(&mut self, _: &usize, _: Option<Animation>) -> Result<()> {
        unused("animations")
    }
    fn global_audio(&mut self, _: [i32; 3], _: i32, _: i32, _: i32, _: i32) -> Result<()> {
        unused("audio")
    }
    fn projectile_magic(
        &mut self,
        _: &usize,
        _: Option<&usize>,
        _: i32,
        _: i32,
        _: i32,
        _: i32,
        _: i32,
    ) -> Result<Projectile> {
        unused("projectiles")
    }
    fn send_projectile(
        &mut self,
        _: &Projectile,
        _: &usize,
        _: ProjectileTarget<'_, usize>,
        _: bool,
        _: i32,
        _: i32,
    ) -> Result<()> {
        unused("projectiles")
    }
    fn attribute(&mut self, _: &usize, _: &str, _: Value) -> Result<Value> {
        unused("attribute reads")
    }
    fn set_attribute(&mut self, entity: &usize, key: &str, value: Value) -> Result<()> {
        self.calls
            .push(format!("setAttribute({entity}, {key}, {value:?})"));
        Ok(())
    }
    fn remove_attribute(&mut self, _: &usize, _: &str) -> Result<()> {
        unused("attribute removals")
    }
    fn autocast(&mut self, _: &usize) -> Result<Option<Key>> {
        unused("the autocast spell")
    }
    fn set_autocast(&mut self, _: &usize, _: Option<&Key>) -> Result<()> {
        unused("the autocast spell")
    }
    fn set_spell(&mut self, _: &usize, _: &Key) -> Result<()> {
        unused("the selected spell")
    }
    fn is_attacking(&mut self, _: &usize) -> Result<bool> {
        unused("the combat pulse")
    }
    fn set_victim(&mut self, _: &usize, _: Option<&usize>) -> Result<()> {
        unused("the combat pulse")
    }
    fn update_style(&mut self, _: &usize) -> Result<()> {
        unused("the combat pulse")
    }
    fn pulse_start(&mut self, _: &usize) -> Result<()> {
        unused("the combat pulse")
    }
    fn pulse_attack(&mut self, _: &usize, _: Option<&usize>) -> Result<()> {
        unused("the combat pulse")
    }
    fn attack_style(&mut self, _: &usize) -> Result<Option<i32>> {
        unused("the attack style")
    }
    fn attack_style_index(&mut self, _: &usize) -> Result<i32> {
        unused("the attack style")
    }
    fn magic_animation(&mut self, _: &usize) -> Result<Option<Animation>> {
        unused("animations")
    }
    fn attack_animation(&mut self, _: &usize) -> Result<Option<Animation>> {
        unused("animations")
    }
    fn multi_zone(&mut self, _: &usize) -> Result<bool> {
        unused("multi zones")
    }
    fn bonuses(&mut self, _: &usize) -> Result<Option<Vec<i32>>> {
        unused("bonuses")
    }
    fn in_zone(&mut self, _: &usize, _: &str) -> Result<bool> {
        unused("zones")
    }
    fn timer_active(&mut self, entity: &usize, name: &str) -> Result<bool> {
        self.calls.push(format!("hasTimerActive({entity}, {name})"));
        Ok(self.timers.iter().any(|(e, n)| e == entity && *n == name))
    }
    fn god_casts(&mut self, _: &usize) -> Result<Option<Vec<i32>>> {
        unused("god casts")
    }
    fn npc_id(&mut self, _: &usize) -> Result<i32> {
        unused("NPC ids")
    }
    fn npc_task(&mut self, _: &usize) -> Result<Option<bool>> {
        unused("slayer tasks")
    }
    fn combat_graphics(&mut self, _: &usize) -> Result<Option<Vec<Option<Graphics>>>> {
        unused("combat graphics")
    }
    fn multi_targets(&mut self, _: &usize, _: &usize) -> Result<Vec<usize>> {
        unused("multi targets")
    }
    fn open_tab(&mut self, _: &usize, _: i32) -> Result<()> {
        unused("tabs")
    }
    fn set_varbit(&mut self, _: &usize, _: i32, _: i32, _: bool) -> Result<()> {
        unused("varbits")
    }
    fn component_visibility(&mut self, _: &usize, _: i32, _: i32, _: bool) -> Result<()> {
        unused("component visibility")
    }
    fn set_attack_style(&mut self, _: &usize, _: i32) -> Result<bool> {
        unused("the attack style")
    }
    fn math_random(&mut self) -> Result<f64> {
        unused("Math.random")
    }
}

impl Effects for World {
    fn movement_locked(&mut self, entity: &usize) -> Result<bool> {
        self.calls.push(format!("isMovementLocked({entity})"));
        Ok(self.movement_locked)
    }
    fn reset_walking(&mut self, entity: &usize) -> Result<()> {
        self.calls.push(format!("walkingQueue.reset({entity})"));
        Ok(())
    }
    fn lock_movement(&mut self, entity: &usize, ticks: i32) -> Result<()> {
        self.calls.push(format!("lockMovement({entity}, {ticks})"));
        Ok(())
    }
    fn drain_level(&mut self, entity: &usize, slot: i32, drain: f64, maximum: f64) -> Result<()> {
        self.calls
            .push(format!("drainLevel({entity}, {slot}, {drain}, {maximum})"));
        Ok(())
    }
    fn decrement_prayer_points(&mut self, entity: &usize, amount: f64) -> Result<()> {
        self.calls
            .push(format!("decrementPrayerPoints({entity}, {amount})"));
        Ok(())
    }
    fn static_level(&mut self, entity: &usize, slot: i32) -> Result<i32> {
        self.calls.push(format!("getStaticLevel({entity}, {slot})"));
        Ok(self.attack)
    }
    fn update_level(
        &mut self,
        entity: &usize,
        slot: i32,
        amount: i32,
        maximum: i32,
    ) -> Result<i32> {
        self.calls.push(format!(
            "updateLevel({entity}, {slot}, {amount}, {maximum})"
        ));
        Ok(0)
    }
    fn heal(&mut self, entity: &usize, amount: i32) -> Result<i32> {
        self.calls.push(format!("heal({entity}, {amount})"));
        Ok(0)
    }
    fn accurate_impact(
        &mut self,
        style: CombatStyle,
        entity: &usize,
        victim: &usize,
    ) -> Result<bool> {
        self.calls
            .push(format!("isAccurateImpact({style:?}, {entity}, {victim})"));
        Ok(self.accurate)
    }
    fn protect_from_magic(&mut self, player: &usize) -> Result<bool> {
        self.calls.push(format!("protectFromMagic({player})"));
        Ok(self.protect)
    }
    fn register_timer(
        &mut self,
        entity: &usize,
        identifier: &str,
        args: &[TimerArg],
    ) -> Result<()> {
        self.calls
            .push(format!("registerTimer({entity}, {identifier}, {args:?})"));
        Ok(())
    }
    fn apply_poison(&mut self, victim: &usize, source: &usize, severity: i32) -> Result<()> {
        self.calls
            .push(format!("applyPoison({victim}, {source}, {severity})"));
        Ok(())
    }
}

#[test]
fn the_empty_base_classes_do_nothing() {
    let mut w = World::new();
    for class in [
        SpellClass::Air,
        SpellClass::Water,
        SpellClass::Earth,
        SpellClass::Fire,
        SpellClass::CrumbleUndead,
        SpellClass::IbanBlast,
        SpellClass::MagicDart,
        SpellClass::SonicWave,
    ] {
        let key = *w
            .registry
            .iter()
            .find(|(_, s)| s.class == class)
            .map(|(k, _)| k)
            .expect("a registered spell");
        let (result, state, calls) = w.fire(key, PLAYER, Some(NPC), 5);
        assert_eq!(result, Ok(()), "{class:?}");
        assert_eq!(state.estimated_hit, 5, "{class:?}");
        assert!(calls.is_empty(), "{class:?}: {calls:?}");
    }
}

#[test]
fn binds_lock_movement_for_their_length_and_set_the_delay() {
    let mut w = World::new();
    for (kind, ticks, hit_after) in [
        (SpellType::Bind, 9, -2),
        (SpellType::Snare, 17, 2),
        (SpellType::Entangle, 25, 2),
    ] {
        let key = w.key(SpellClass::Bind, kind);
        // A player victim that can move is told.
        let (result, state, calls) = w.fire(key, PLAYER, Some(OTHER), 2);
        assert_eq!(result, Ok(()));
        assert_eq!(state.estimated_hit, hit_after, "{kind:?}");
        assert_eq!(
            calls,
            [
                format!("isMovementLocked({OTHER})"),
                format!("packetMessage({OTHER}, A magical force stops you from moving!)"),
                format!("walkingQueue.reset({OTHER})"),
                format!("lockMovement({OTHER}, {ticks})"),
                "ticks".to_owned(),
                format!(
                    "setAttribute({PLAYER}, entangleDelay, Int({}))",
                    1000 + ticks + 2
                ),
            ],
            "{kind:?}"
        );
    }
    // A splash binds nothing.
    let snare = w.key(SpellClass::Bind, SpellType::Snare);
    let (result, state, calls) = w.fire(snare, PLAYER, Some(OTHER), -1);
    assert_eq!((result, state.estimated_hit), (Ok(()), -1));
    assert!(calls.is_empty(), "{calls:?}");
    // An NPC's name is read; an NPC and an already locked player get no message.
    let (_, _, calls) = w.fire(snare, PLAYER, Some(NPC), 0);
    assert_eq!(calls[0], format!("name({NPC})"));
    assert!(!calls.iter().any(|c| c.starts_with("packetMessage")));
    w.movement_locked = true;
    let (_, _, calls) = w.fire(snare, PLAYER, Some(OTHER), 0);
    assert!(!calls.iter().any(|c| c.starts_with("packetMessage")));
    assert!(calls.contains(&format!("lockMovement({OTHER}, 17)")));
}

#[test]
fn an_impling_is_bound_even_on_a_splash() {
    let mut w = World::new();
    w.npc_name = Some("Baby impling".into());
    let snare = w.key(SpellClass::Bind, SpellType::Snare);
    let (result, state, calls) = w.fire(snare, PLAYER, Some(NPC), -1);
    assert_eq!((result, state.estimated_hit), (Ok(()), -2));
    assert!(calls.contains(&format!("lockMovement({NPC}, 17)")));
    // A null name fails as the original's NullPointerException.
    w.npc_name = None;
    let (result, _, _) = w.fire(snare, PLAYER, Some(NPC), 3);
    assert_eq!(result, Err(Error::Null));
}

#[test]
fn curses_drain_their_skill_and_hide_the_hit() {
    let mut w = World::new();
    for (kind, slot, amount) in [
        (SpellType::Confuse, 0, "0.05"),
        (SpellType::Weaken, 2, "0.05"),
        (SpellType::Curse, 1, "0.05"),
        (SpellType::Vulnerability, 1, "0.1"),
        (SpellType::Enfeeble, 2, "0.1"),
        (SpellType::Stun, 0, "0.1"),
    ] {
        let key = w.key(SpellClass::Curse, kind);
        let (result, state, calls) = w.fire(key, PLAYER, Some(NPC), 0);
        assert_eq!((result, state.estimated_hit), (Ok(()), -2), "{kind:?}");
        assert_eq!(
            calls,
            [format!("drainLevel({NPC}, {slot}, {amount}, {amount})")],
            "{kind:?}"
        );
        let (result, state, calls) = w.fire(key, PLAYER, Some(NPC), -1);
        assert_eq!((result, state.estimated_hit), (Ok(()), -1), "{kind:?}");
        assert!(calls.is_empty());
    }
}

#[test]
fn god_spells_drain_whether_or_not_the_hit_landed() {
    let mut w = World::new();
    for (button, call) in [
        (41, format!("decrementPrayerPoints({NPC}, 1)")),
        (42, format!("drainLevel({NPC}, 1, 0.05, 0.05)")),
        (43, format!("drainLevel({NPC}, 6, 0.05, 0.05)")),
    ] {
        for hit in [-1, 0, 20] {
            let (result, state, calls) =
                w.fire((SpellBook::Modern, button), PLAYER, Some(NPC), hit);
            assert_eq!((result, state.estimated_hit), (Ok(()), hit));
            assert_eq!(calls, std::slice::from_ref(&call), "button {button}");
        }
    }
}

#[test]
fn teleblock_registers_its_timer_on_a_player_only() {
    let mut w = World::new();
    let key = w.key(SpellClass::Teleblock, SpellType::Teleblock);
    let (result, _, calls) = w.fire(key, PLAYER, Some(OTHER), 0);
    assert_eq!(result, Ok(()));
    assert_eq!(
        calls,
        [
            format!("hasTimerActive({OTHER}, teleblock)"),
            format!("isAccurateImpact(Magic, {PLAYER}, {OTHER})"),
            format!("protectFromMagic({OTHER})"),
            format!("registerTimer({OTHER}, teleblock, [Int(500)])"),
        ]
    );
    // Protect from Magic halves it.
    w.protect = true;
    let (_, _, calls) = w.fire(key, PLAYER, Some(OTHER), 0);
    assert_eq!(
        calls.last().unwrap(),
        &format!("registerTimer({OTHER}, teleblock, [Int(250)])")
    );
    // An inaccurate impact: the timer is asked again, nothing else.
    w.accurate = false;
    let (_, _, calls) = w.fire(key, PLAYER, Some(OTHER), 0);
    assert_eq!(
        calls,
        [
            format!("hasTimerActive({OTHER}, teleblock)"),
            format!("isAccurateImpact(Magic, {PLAYER}, {OTHER})"),
            format!("hasTimerActive({OTHER}, teleblock)"),
        ]
    );
    // An NPC victim: no accuracy roll.
    let (_, _, calls) = w.fire(key, PLAYER, Some(NPC), 0);
    assert_eq!(
        calls,
        [
            format!("hasTimerActive({NPC}, teleblock)"),
            format!("hasTimerActive({NPC}, teleblock)"),
        ]
    );
    // Already blocked: the caster is told; an NPC caster fails the cast.
    w.timers.push((OTHER, "teleblock"));
    let (result, _, calls) = w.fire(key, PLAYER, Some(OTHER), 0);
    assert_eq!(result, Ok(()));
    assert_eq!(
        calls.last().unwrap(),
        &format!("sendMessage({PLAYER}, Your target is already blocked from teleporting.)")
    );
    let (result, _, _) = w.fire(key, NPC, Some(OTHER), 0);
    assert_eq!(result, Err(Error::Cast));
    // A null victim fails before anything is asked.
    let (result, _, calls) = w.fire(key, PLAYER, None, 0);
    assert_eq!(result, Err(Error::Null));
    assert!(calls.is_empty());
}

#[test]
fn ice_spells_freeze_by_type_and_mark_a_frozen_barrage() {
    let mut w = World::new();
    for (kind, ticks) in [
        (SpellType::Rush, 8),
        (SpellType::Burst, 16),
        (SpellType::Blitz, 24),
        (SpellType::Barrage, 32),
    ] {
        let key = w.key(SpellClass::Ice, kind);
        let (result, state, calls) = w.fire(key, PLAYER, Some(NPC), 0);
        assert_eq!(result, Ok(()));
        assert!(!state.frozen);
        assert_eq!(
            calls,
            [
                format!("hasTimerActive({NPC}, frozen)"),
                format!("hasTimerActive({NPC}, frozen:immunity)"),
                format!("registerTimer({NPC}, frozen, [Int({ticks}), Bool(true)])"),
            ],
            "{kind:?}"
        );
        let (_, _, calls) = w.fire(key, PLAYER, Some(NPC), -1);
        assert!(calls.is_empty(), "a splash freezes nothing");
    }
    w.timers.push((NPC, "frozen:immunity"));
    for kind in [
        SpellType::Rush,
        SpellType::Burst,
        SpellType::Blitz,
        SpellType::Barrage,
    ] {
        let key = w.key(SpellClass::Ice, kind);
        let (result, state, calls) = w.fire(key, PLAYER, Some(NPC), 4);
        assert_eq!(result, Ok(()));
        assert_eq!(state.frozen, kind == SpellType::Barrage, "{kind:?}");
        assert!(!calls.iter().any(|c| c.starts_with("registerTimer")));
    }
}

#[test]
fn blood_spells_heal_a_quarter_of_the_hit() {
    let mut w = World::new();
    let key = w.key(SpellClass::Blood, SpellType::Barrage);
    let (_, _, calls) = w.fire(key, PLAYER, Some(NPC), 21);
    assert_eq!(
        calls,
        [
            format!("heal({PLAYER}, 5)"),
            format!("packetMessage({PLAYER}, You drain some of your opponent's health.)"),
        ]
    );
    let (_, _, calls) = w.fire(key, NPC, Some(PLAYER), 8);
    assert_eq!(calls, [format!("heal({NPC}, 2)")]);
    for hit in [-2, -1, 0, 3] {
        let (result, _, calls) = w.fire(key, PLAYER, Some(NPC), hit);
        assert_eq!(result, Ok(()));
        assert!(calls.is_empty(), "hit {hit}: {calls:?}");
    }
}

#[test]
fn shadow_spells_lower_attack_by_a_tenth_of_the_static_level() {
    let mut w = World::new();
    let key = w.key(SpellClass::Shadow, SpellType::Rush);
    for (level, amount, maximum) in [(60, -6, 54), (99, -9, 89), (5, 0, 4), (1, 0, 0)] {
        w.attack = level;
        let (_, _, calls) = w.fire(key, PLAYER, Some(NPC), 0);
        assert_eq!(
            calls,
            [
                format!("getStaticLevel({NPC}, 0)"),
                format!("updateLevel({NPC}, 0, {amount}, {maximum})"),
            ],
            "level {level}"
        );
    }
    let (_, _, calls) = w.fire(key, PLAYER, Some(NPC), -1);
    assert!(calls.is_empty());
}

#[test]
fn smoke_spells_poison_by_type() {
    let mut w = World::new();
    for (kind, severity) in [
        (SpellType::Rush, 2),
        (SpellType::Burst, 2),
        (SpellType::Blitz, 4),
        (SpellType::Barrage, 4),
    ] {
        let key = w.key(SpellClass::Smoke, kind);
        let (_, _, calls) = w.fire(key, PLAYER, Some(NPC), 0);
        assert_eq!(calls, [format!("applyPoison({NPC}, {PLAYER}, {severity})")]);
        let (_, _, calls) = w.fire(key, PLAYER, Some(NPC), -1);
        assert!(calls.is_empty());
    }
}

#[test]
fn miasmic_spells_register_by_spell_id_unless_immune() {
    let mut w = World::new();
    let mut keys: Vec<Key> = w
        .registry
        .iter()
        .filter(|(_, s)| s.class == SpellClass::Miasmic)
        .map(|(k, _)| *k)
        .collect();
    keys.sort_by_key(|k| k.1);
    assert_eq!(keys.len(), 4);
    for key in &keys {
        // Whatever the hit: the original does not read it.
        let (result, _, calls) = w.fire(*key, PLAYER, Some(NPC), -1);
        assert_eq!(result, Ok(()));
        assert_eq!(
            calls,
            [
                format!("hasTimerActive({NPC}, miasmic:immunity)"),
                format!(
                    "registerTimer({NPC}, miasmic, [Int({})])",
                    (key.1 - 15) * 20
                ),
            ]
        );
    }
    w.timers.push((NPC, "miasmic:immunity"));
    let (_, _, calls) = w.fire(keys[0], PLAYER, Some(NPC), 5);
    assert_eq!(calls, [format!("hasTimerActive({NPC}, miasmic:immunity)")]);
}
