//! The engine timers (`timers::content`, docs/content-timers.md) on the live
//! players' timer managers: the fourteen prototypes registered in the world's
//! `TimerRegistry` (class-scan order), `TimerRegistry.addAutoTimers` at
//! `Entity.init`, `timers.processTimers()` at the end of `Entity.tick`, the
//! timers of a save (`saveTimers`/`parseTimers`), the entity event hooks
//! (`Entity.hook`/`unhook`/`dispatch`) and the timer queries of the walking
//! queue, the zones and the SkullManager.
//!
//! [`Host`] is the library's `content::Host` over the live world; [`Saving`]
//! answers the same services while a save file is written or read (inside
//! `Inner::environment_run`, where the world is borrowed by the save code):
//! the uid lookups, the clock, the shared generator and the log are answered
//! there, the messages and skull calls of a timer registered by the parse are
//! queued and made right after the parse returns ([`TimerCall`]), and every
//! other service refuses by name. Every call into a system without a port is a
//! refusal by name (listed in docs/live-world.md).
use super::{
    P,
    host::Live,
    inner::{Inner, SettingsContext, SkillsContext, placeholder_skills},
};
use crate::{
    player::{
        details::Rights,
        settings::{self, Settings},
        skills::{self, Effect, EntityKind as SkillsKind, Skills},
    },
    timers::{
        self, Error, Handle, Manager, Result,
        content::{self, EntityKind, Event, EventClass, Hook, Logout, Natives, Tab},
        registry::{Level, Registry},
    },
    world::{
        combat_pulse::HitsplatType,
        player_managers::skull,
        player_walk::{Node, NodeHandle},
        script_runtime, zones,
    },
};
use rs09_protocol::game::{Frame, updates};
use std::collections::{BTreeMap, HashMap};

/// `RandomEvents`: random event content is not ported, so no constant exists.
#[derive(Clone, Debug)]
pub enum NoRandomEvent {}
/// `RandomEventNPC`: not ported.
#[derive(Clone, Debug)]
pub enum NoEventNpc {}
/// A `WeaponInterface` extension as SpellbookSwap reaches it: not bound.
pub enum NoWeapon {}
/// A closed `DialoguePlugin`: dialogue close events are not dispatched to
/// timers in this world.
#[derive(Clone, Debug)]
pub enum NoDialogue {}

/// The world's timer registry and the entities' event hooks.
#[derive(Default)]
pub struct TimerWorld {
    /// `TimerRegistry` with the fourteen engine prototypes.
    pub registry: Registry,
    /// The prototypes were registered (`ClassScanner.loadTimers`), the first
    /// time a timer operation needed the registry.
    pub registered: bool,
    /// `Entity.hooks`: per entity, each event class's hook list in order.
    pub hooks: HashMap<Node, HashMap<EventClass, Vec<Hook>>>,
    /// Calls a timer registered by a save's parse made, made after the parse.
    pub calls: Vec<TimerCall>,
    /// NPCs whose `SkillRestore` was registered on joining the live timers
    /// (the start-up has no port of its hooks: [`Live::with_npc_timers`]).
    pub npc_restore: std::collections::HashSet<Node>,
}

/// A call of a timer registered while a save file was parsed, made right
/// after the parse returns.
#[derive(Clone, Debug)]
pub enum TimerCall {
    Message(P, String),
    Debug(P, String),
    Audio(P, i32, i32, i32),
    SkullIcon(P, i32),
    Skulled(P, bool),
    ResetSkull(P),
}

fn refused<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}
fn err(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// A new manager over the same three lists (`TimerList` handles alias).
pub fn alias(manager: &Manager) -> Manager {
    Manager {
        active: manager.active.clone(),
        pending: manager.pending.clone(),
        removing: manager.removing.clone(),
    }
}

/// `Entity.hook(class, hook)`: appended to that class's list.
fn hook(world: &mut TimerWorld, node: Node, class: EventClass, hook: Hook) {
    world
        .hooks
        .entry(node)
        .or_default()
        .entry(class)
        .or_default()
        .push(hook);
}
/// `Entity.unhook(hook)`: removed from every list holding it.
fn unhook(world: &mut TimerWorld, node: Node, hook: &Hook) {
    if let Some(classes) = world.hooks.get_mut(&node) {
        for list in classes.values_mut() {
            if let Some(at) = list.iter().position(|h| h.same(hook)) {
                list.remove(at);
            }
        }
    }
}

// ---------------------------------------------------- NPC skills

/// An NPC's `Skills` as the timers read them: its row's static and current
/// levels and lifepoints (an NPC's `lifepointsIncrease` is 0 and it has no
/// experience).
pub fn npc_skills(row: &crate::startup::npcs::Npc) -> Skills {
    let mut skills = placeholder_skills();
    skills
        .static_levels
        .copy_from_slice(&row.static_levels[..24]);
    skills.dynamic_levels.copy_from_slice(&row.levels[..24]);
    skills.lifepoints = row.lifepoints;
    skills.lifepoints_increase = 0;
    skills.prayer_points = f64::from(row.levels[5]);
    skills
}

/// Writes what a timer changed of an NPC's skills back to its row: the
/// current levels and the lifepoints (timers change no static level).
pub fn put_npc_skills(row: &mut crate::startup::npcs::Npc, skills: &Skills) {
    row.levels[..24].copy_from_slice(&skills.dynamic_levels);
    row.lifepoints = skills.lifepoints;
}

/// `GrandExchangeTimer`, the start-up's automatic timer on every NPC: its
/// class name there (`startup::timers`).
const GRAND_EXCHANGE_TIMER: &str = "GrandExchangeTimer";

/// The timers' `Context` for an NPC's manager: `timers::content`'s
/// [`Natives`], and the NPC branch of `GrandExchangeTimer`, which the
/// start-up registered on every NPC (`Entity.init`'s `addAutoTimers`):
/// no registration hooks, the constructor's interval as the initial delay,
/// and `run` answering false (`if (entity !is Player) return false`), so
/// the manager removes it at its first run, 500 ticks after its first
/// process.
pub struct NpcNatives<'n, 'a, 'h> {
    pub natives: &'n mut Natives<'a, Host<'h>>,
}

fn grand_exchange(timer: &Handle) -> bool {
    timer.read().class_name == GRAND_EXCHANGE_TIMER
}

impl timers::Context<()> for NpcNatives<'_, '_, '_> {
    fn is_player(&self, state: &()) -> bool {
        timers::Context::<()>::is_player(self.natives, state)
    }
    fn modal_open(&mut self, state: &mut ()) -> Result<bool> {
        timers::Context::<()>::modal_open(self.natives, state)
    }
    fn script_delay(&mut self, state: &mut ()) -> Result<i32> {
        timers::Context::<()>::script_delay(self.natives, state)
    }
    fn ticks(&mut self, state: &mut ()) -> Result<i32> {
        timers::Context::<()>::ticks(self.natives, state)
    }
    fn entity_name(&mut self, state: &mut ()) -> Result<Option<String>> {
        timers::Context::<()>::entity_name(self.natives, state)
    }
    fn before_register(
        &mut self,
        timer: &Handle,
        manager: &mut Manager,
        state: &mut (),
    ) -> Result<()> {
        if grand_exchange(timer) {
            return Ok(());
        }
        self.natives.before_register(timer, manager, state)
    }
    fn on_register(&mut self, timer: &Handle, manager: &mut Manager, state: &mut ()) -> Result<()> {
        if grand_exchange(timer) {
            return Ok(());
        }
        self.natives.on_register(timer, manager, state)
    }
    fn on_removal(&mut self, timer: &Handle, manager: &mut Manager, state: &mut ()) -> Result<()> {
        if grand_exchange(timer) {
            return Ok(());
        }
        self.natives.on_removal(timer, manager, state)
    }
    fn run(&mut self, timer: &Handle, manager: &mut Manager, state: &mut ()) -> Result<bool> {
        if grand_exchange(timer) {
            return Ok(false);
        }
        self.natives.run(timer, manager, state)
    }
    fn initial_delay(
        &mut self,
        timer: &Handle,
        manager: &mut Manager,
        state: &mut (),
    ) -> Result<i32> {
        if grand_exchange(timer) {
            return Ok(timer.read().run_interval);
        }
        self.natives.initial_delay(timer, manager, state)
    }
    fn save(
        &mut self,
        timer: &Handle,
        root: &mut crate::definitions::item_config::json::Json,
        manager: &mut Manager,
        state: &mut (),
    ) -> Result<()> {
        self.natives.save(timer, root, manager, state)
    }
    fn parse(
        &mut self,
        timer: &Handle,
        data: &crate::definitions::item_config::json::Json,
        manager: &mut Manager,
        state: &mut (),
    ) -> Result<()> {
        self.natives.parse(timer, data, manager, state)
    }
    fn resolve(
        &mut self,
        identifier: &str,
        manager: &mut Manager,
        state: &mut (),
    ) -> Result<Option<Handle>> {
        self.natives.resolve(identifier, manager, state)
    }
    fn run_failure(
        &mut self,
        class_name: &str,
        entity_name: Option<&str>,
        state: &mut (),
    ) -> Result<()> {
        self.natives.run_failure(class_name, entity_name, state)
    }
    fn unknown_timer(&mut self, identifier: &str, state: &mut ()) -> Result<()> {
        self.natives.unknown_timer(identifier, state)
    }
    fn exception(&mut self, error: &Error, state: &mut ()) -> Result<()> {
        timers::Context::<()>::exception(self.natives, error, state)
    }
}

// ---------------------------------------------------- the runtime host

/// The skills and settings context of the timers' player: the live
/// [`SkillsContext`] and [`SettingsContext`] of `p`.
pub struct Ctx<'a> {
    pub live: &'a mut Live,
    pub p: P,
    /// The skills handed out are a failed lookup's placeholder: every
    /// effect refuses (the operation reports the failure).
    pub disabled: bool,
    /// The skills handed out are an NPC's (its row's levels): `kind` is NPC,
    /// so `Skills` asks none of the player services below.
    pub npc: bool,
}
impl Ctx<'_> {
    fn check(&self) -> skills::Result<()> {
        if self.disabled || self.npc {
            return Err(skills::Error::Binding);
        }
        Ok(())
    }
    fn skills(&mut self) -> SkillsContext<'_> {
        SkillsContext {
            inner: &mut self.live.inner,
            p: self.p.clone(),
        }
    }
    fn settings(&mut self) -> SettingsContext<'_> {
        SettingsContext {
            inner: &mut self.live.inner,
            p: self.p.clone(),
        }
    }
}
impl skills::Context for Ctx<'_> {
    fn kind(&self) -> SkillsKind {
        if self.npc {
            SkillsKind::Npc
        } else {
            SkillsKind::Player
        }
    }
    fn version(&self) -> i32 {
        6
    }
    fn tick(&mut self) -> i32 {
        self.live.inner.ticks
    }
    fn world_tick(&mut self) -> i32 {
        self.live.inner.ticks
    }
    fn combat_level(&self) -> i32 {
        if self.disabled || self.npc {
            return 3;
        }
        self.live
            .inner
            .with_profile(&self.p, |p| p.properties.combat_level)
            .unwrap_or(3)
    }
    fn summoning_combat_level(&self) -> i32 {
        if self.disabled || self.npc {
            return 0;
        }
        self.live
            .inner
            .with_profile(&self.p, |p| p.summoning_combat_level)
            .unwrap_or(0)
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        self.check()?;
        self.skills().npc_combat_level()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        self.check()?;
        self.skills().damage_spong_active()
    }
    fn assisted_level(&mut self, skill: i32) -> skills::Result<Option<i32>> {
        self.check()?;
        self.skills().assisted_level(skill)
    }
    fn translate_experience(
        &mut self,
        state: &mut Skills,
        skill: i32,
        amount: f64,
        multiplier: f64,
    ) -> skills::Result<bool> {
        self.check()?;
        self.skills()
            .translate_experience(state, skill, amount, multiplier)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        self.check()?;
        self.skills().brawling_present()
    }
    fn brawling_for_skill(&mut self, skill: i32) -> skills::Result<Option<i32>> {
        self.check()?;
        self.skills().brawling_for_skill(skill)
    }
    fn equipment_contains(&mut self, id: i32) -> skills::Result<bool> {
        self.check()?;
        self.skills().equipment_contains(id)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        self.check()?;
        self.skills().brawling_bonus()
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        self.check()?;
        self.skills().tutorial_complete()
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        self.check()?;
        self.skills().artificial()
    }
    fn username(&mut self) -> skills::Result<String> {
        self.check()?;
        self.skills().username()
    }
    fn permadeath_present(&self) -> bool {
        false
    }
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        self.check()?;
        self.skills().effect(state, effect)
    }
}
impl settings::Context for Ctx<'_> {
    fn varp(&mut self, s: &Settings, id: i32, value: i32) -> settings::Result<()> {
        self.settings().varp(s, id, value)
    }
    fn session(&mut self, s: &Settings) -> settings::Result<Option<usize>> {
        self.settings().session(s)
    }
    fn send(&mut self, s: &Settings, session: Option<usize>, frame: Frame) -> settings::Result<()> {
        self.settings().send(s, session, frame)
    }
    fn dispatcher(&mut self, s: &Settings) -> settings::Result<Option<usize>> {
        self.settings().dispatcher(s)
    }
    fn run_energy(&mut self, s: &Settings, d: Option<usize>) -> settings::Result<()> {
        self.settings().run_energy(s, d)
    }
    fn message(&mut self, s: &Settings, d: Option<usize>, text: &str) -> settings::Result<()> {
        self.settings().message(s, d, text)
    }
    fn string(
        &mut self,
        s: &Settings,
        d: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> settings::Result<()> {
        self.settings().string(s, d, text, interface, child)
    }
    fn properties(&mut self, s: &Settings) -> settings::Result<Option<usize>> {
        self.settings().properties(s)
    }
    fn retaliating(&mut self, s: &Settings, p: Option<usize>) -> settings::Result<bool> {
        self.settings().retaliating(s, p)
    }
    fn set_retaliating(
        &mut self,
        s: &Settings,
        p: Option<usize>,
        value: bool,
    ) -> settings::Result<()> {
        self.settings().set_retaliating(s, p, value)
    }
    fn ironman(&mut self, s: &Settings) -> settings::Result<bool> {
        self.settings().ironman(s)
    }
    fn container(
        &mut self,
        s: &Settings,
        kind: settings::ContainerKind,
    ) -> settings::Result<usize> {
        self.settings().container(s, kind)
    }
    fn item(
        &mut self,
        s: &Settings,
        container: usize,
        slot: i32,
    ) -> settings::Result<Option<usize>> {
        self.settings().item(s, container, slot)
    }
    fn weight(&mut self, s: &Settings, item: usize) -> settings::Result<f64> {
        self.settings().weight(s, item)
    }
}

/// `content::Host` over the live world. The skills of each player a timer
/// reaches are taken out of its profile for the operation and put back by
/// [`Host::finish`].
pub struct Host<'a> {
    held: BTreeMap<Node, (P, Skills)>,
    /// A player's `spellBook` field while a timer holds it.
    book: Option<(P, i32)>,
    /// Skills a timer asked of an entity that has none here.
    failures: Vec<String>,
    pub ctx: Ctx<'a>,
}

impl<'a> Host<'a> {
    pub fn new(live: &'a mut Live, p: P) -> Self {
        Self {
            held: BTreeMap::new(),
            book: None,
            failures: Vec::new(),
            ctx: Ctx {
                live,
                p,
                disabled: false,
                npc: false,
            },
        }
    }
    fn inner(&mut self) -> &mut Inner {
        &mut self.ctx.live.inner
    }
    /// Writes skills taken for an operation back to their owner: a player's
    /// profile or an NPC's row (levels and lifepoints).
    fn put_back(&mut self, node: Node, p: &P, skills: Skills) -> Result<()> {
        if node == usize::MAX {
            // The placeholder of a failed lookup: never written back.
            return Ok(());
        }
        if let Some(row) = self.ctx.live.inner.npcs.rows.get_mut(&node) {
            put_npc_skills(row, &skills);
            return Ok(());
        }
        if !self.ctx.live.inner.players.contains_key(&p.node()) {
            // A player who left during the operation.
            return Ok(());
        }
        self.ctx
            .live
            .inner
            .with_profile(p, |profile| profile.skills = skills)
            .map_err(err)
    }
    /// Puts `entity`'s held skills back before a service that reads them
    /// from their owner (a hit reads and writes the lifepoints there); the
    /// next `skills` call takes them again.
    fn release(&mut self, entity: &P) -> Result<()> {
        match self.held.remove(&entity.node()) {
            Some((p, skills)) => self.put_back(entity.node(), &p, skills),
            None => Ok(()),
        }
    }
    fn is_player(&self, entity: &P) -> bool {
        self.ctx.live.inner.players.contains_key(&entity.node())
    }
    fn player(&self, entity: &P) -> Result<()> {
        if self.is_player(entity) {
            Ok(())
        } else {
            refused("an NPC's player members")
        }
    }
    /// Puts back what the operation took out of the profiles.
    pub fn finish(mut self) -> Result<()> {
        for (node, (p, skills)) in std::mem::take(&mut self.held) {
            self.put_back(node, &p, skills)?;
        }
        if let Some((p, book)) = self.book.take().filter(|(p, _)| self.is_player(p)) {
            self.ctx
                .live
                .inner
                .with_profile(&p, |profile| profile.spell_book = book)
                .map_err(err)?;
        }
        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(Error::Binding(self.failures.join("; ")))
        }
    }
}

impl<'a> content::Host for Host<'a> {
    type Entity = P;
    type RandomEvent = NoRandomEvent;
    type EventNpc = NoEventNpc;
    type Weapon = NoWeapon;
    type Dialogue = NoDialogue;
    type SkillsContext = Ctx<'a>;
    type SettingsContext = Ctx<'a>;

    fn kind(&self, entity: &P) -> EntityKind {
        if self.is_player(entity) {
            EntityKind::Player
        } else if self.ctx.live.inner.npcs.rows.contains_key(&entity.node()) {
            // Familiars are not ported: every NPC of this world is a plain NPC.
            EntityKind::Npc
        } else {
            EntityKind::Other
        }
    }
    fn skills(&mut self, entity: &P) -> (&mut Skills, &mut Ctx<'a>) {
        let node = entity.node();
        if !self.held.contains_key(&node) {
            let taken = if self.is_player(entity) {
                self.ctx
                    .live
                    .inner
                    .with_profile(entity, |profile| {
                        std::mem::replace(&mut profile.skills, placeholder_skills())
                    })
                    .map_err(|e| format!("{e:?}"))
            } else if let Some(row) = self.ctx.live.inner.npcs.rows.get(&node) {
                Ok(npc_skills(row))
            } else {
                Err(
                    "timers asked the skills of a node that is neither a player nor an NPC"
                        .to_owned(),
                )
            };
            match taken {
                Ok(skills) => {
                    self.held.insert(node, (entity.clone(), skills));
                }
                Err(failure) => {
                    // Never written back; the operation reports the failure.
                    self.failures.push(failure);
                    self.held
                        .insert(usize::MAX, (entity.clone(), placeholder_skills()));
                    self.ctx.disabled = true;
                    self.ctx.npc = false;
                    let (_, skills) = self.held.get_mut(&usize::MAX).expect("inserted");
                    return (skills, &mut self.ctx);
                }
            }
        }
        self.ctx.p = entity.clone();
        self.ctx.disabled = false;
        self.ctx.npc = !self.is_player(entity);
        let (_, skills) = self.held.get_mut(&node).expect("held");
        (skills, &mut self.ctx)
    }
    fn settings(&mut self, player: &P) -> (Settings, &mut Ctx<'a>) {
        let settings = match self.ctx.live.inner.players.get(&player.node()) {
            Some(slot) => slot.canonical.player.settings(),
            None => {
                self.failures
                    .push("not ported in the live world: NPC settings in timers".into());
                Settings::default()
            }
        };
        self.ctx.p = player.clone();
        (settings, &mut self.ctx)
    }
    fn spell_book(&mut self, player: &P) -> &mut i32 {
        let held = self.book.as_ref().is_some_and(|(p, _)| p == player);
        if !held {
            if let Some((p, book)) = self.book.take() {
                if self.is_player(&p) {
                    let _ = self
                        .ctx
                        .live
                        .inner
                        .with_profile(&p, |profile| profile.spell_book = book);
                }
            }
            let book = if self.is_player(player) {
                self.ctx
                    .live
                    .inner
                    .with_profile(player, |profile| profile.spell_book)
                    .unwrap_or_else(|e| {
                        self.failures.push(format!("{e:?}"));
                        0
                    })
            } else {
                self.failures
                    .push("not ported in the live world: an NPC's spell book in timers".into());
                0
            };
            self.book = Some((player.clone(), book));
        }
        &mut self.book.as_mut().expect("held").1
    }
    fn random_event_name(&self, event: &NoRandomEvent) -> String {
        match *event {}
    }
    fn random_event_value_of(&self, _: &str) -> Option<NoRandomEvent> {
        // RandomEvents is content (not ported): valueOf throws for any name.
        None
    }
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.inner().ticks)
    }
    fn modal_open(&mut self, player: &P) -> Result<bool> {
        self.player(player)?;
        let manager = self.inner().canonical(player).player.manager();
        Ok(script_runtime::has_modal_open(&manager))
    }
    fn script_delay(&mut self, entity: &P) -> Result<i32> {
        let scripts = self.inner().state.scripts(&entity.0).map_err(err)?;
        let delay = scripts.fields.read().delay;
        Ok(delay)
    }
    fn remove_weak_scripts(&mut self, entity: &P) -> Result<()> {
        // Entity.scripts.removeWeakScripts() over the canonical processor.
        let scripts = self.inner().state.scripts(&entity.0).map_err(err)?;
        scripts.remove_weak();
        Ok(())
    }
    fn name(&mut self, entity: &P) -> Result<Option<String>> {
        if let Some(slot) = self.ctx.live.inner.players.get(&entity.node()) {
            return Ok(Some(slot.name.clone()));
        }
        // NPC.getName(): the definition's name.
        match self.ctx.live.inner.npcs.rows.get(&entity.node()) {
            Some(row) => Ok(row.definition.read().name.clone()),
            None => refused("the name of a node that is neither a player nor an NPC"),
        }
    }
    fn location(&mut self, entity: &P) -> Result<Option<[i32; 3]>> {
        Ok(self
            .inner()
            .state
            .node_location(entity.node())
            .map_err(err)?
            .map(|l| l.coordinates()))
    }
    fn log(&mut self, level: Level, message: &str) -> Result<()> {
        self.inner().log_line(format!("{level:?} {message}"));
        Ok(())
    }
    fn exception(&mut self, error: &Error) -> Result<()> {
        self.inner()
            .log_line(format!("printStackTrace (timer) {error:?}"));
        Ok(())
    }
    fn send_message(&mut self, player: &P, text: &str) -> Result<()> {
        if !self.is_player(player) {
            // ContentAPI.sendMessage: only a player receives it.
            return Ok(());
        }
        self.inner().message(player, text).map_err(err)
    }
    fn debug(&mut self, player: &P, text: &str) -> Result<()> {
        self.player(player)?;
        let debug = self
            .inner()
            .canonical(player)
            .player
            .attributes()
            .boolean("debug", false)
            .map_err(|_| Error::Type)?;
        if debug {
            self.inner().message(player, text).map_err(err)?;
        }
        Ok(())
    }
    fn play_audio(&mut self, player: &P, id: i32, delay: i32, loops: i32) -> Result<()> {
        self.player(player)?;
        self.inner()
            .queue_native(player, updates::synth_sound(id, loops, delay));
        Ok(())
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        self.inner()
            .generators
            .shared_next_int(bound)
            .ok_or_else(|| Error::Binding("RANDOM.nextInt: bound must be positive".into()))
    }
    fn manual_hit(&mut self, owner: &P, source: &P, hit: i32, kind: HitsplatType) -> Result<()> {
        // The hit reads and writes the owner's lifepoints where they live.
        self.release(owner)?;
        self.ctx
            .live
            .timer_manual_hit(owner, source, hit, kind)
            .map_err(Error::Binding)
    }
    fn worn(&mut self, player: &P, slot: i32) -> Result<Option<i32>> {
        self.player(player)?;
        let item = self.inner().canonical(player).player.equipment().get(slot);
        Ok(item.map(|i| i.read().id()))
    }
    fn artificial(&mut self, player: &P) -> Result<bool> {
        // Every player of this world is a session's player (AIPlayer is not
        // ported and none is created).
        self.player(player)?;
        Ok(false)
    }
    fn administrator(&mut self, player: &P) -> Result<bool> {
        self.player(player)?;
        let rights = self
            .inner()
            .with_profile(player, |profile| profile.details.rights())
            .map_err(err)?
            .map_err(err)?;
        Ok(rights == Rights::Administrator)
    }
    fn uid(&mut self, player: &P) -> Result<i32> {
        match self.ctx.live.inner.players.get(&player.node()) {
            Some(slot) => Ok(slot.uid),
            None => Err(Error::Type),
        }
    }
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<P>> {
        Ok(self
            .ctx
            .live
            .inner
            .players
            .values()
            .find(|slot| slot.uid == uid && slot.active)
            .map(|slot| P(slot.canonical.owner.clone())))
    }
    fn npc_task(&mut self, npc: &P) -> Result<Option<i32>> {
        // NPC.task: Tasks.forId(id) as configure() set it (the current id).
        match self.ctx.live.inner.npcs.rows.get(&npc.node()) {
            Some(row) => Ok(crate::world::slayer_tasks::level_req(row.id)),
            None => refused("the slayer task of a node that is not an NPC"),
        }
    }
    fn familiar_owner(&mut self, _: &P) -> Result<Option<P>> {
        refused("familiars")
    }
    fn poison_immune(&mut self, entity: &P) -> Result<bool> {
        self.ctx
            .live
            .timer_poison_immune(entity)
            .map_err(Error::Binding)
    }
    fn set_skull_icon(&mut self, player: &P, icon: i32) -> Result<()> {
        skull::set_skull_icon(self.ctx.live, &player.0, icon).map_err(err)
    }
    fn set_skulled(&mut self, player: &P, skulled: bool) -> Result<()> {
        skull::answers::set_skulled(self.ctx.live, &player.0, skulled).map_err(err)
    }
    fn reset_skull(&mut self, player: &P) -> Result<()> {
        skull::reset(self.ctx.live, &player.0).map_err(err)
    }
    fn hook(&mut self, entity: &P, class: EventClass, h: Hook) -> Result<()> {
        hook(&mut self.inner().timers, entity.node(), class, h);
        Ok(())
    }
    fn unhook(&mut self, entity: &P, h: &Hook) -> Result<()> {
        unhook(&mut self.inner().timers, entity.node(), h);
        Ok(())
    }
    fn dispatch(
        &mut self,
        entity: &P,
        event: Event<NoDialogue>,
        timers: &mut Manager,
        registry: &Registry,
    ) -> Result<()> {
        // Entity.dispatch: every hook of the event's class, from a copy.
        let hooks = self
            .inner()
            .timers
            .hooks
            .get(&entity.node())
            .and_then(|classes| classes.get(&event.class()))
            .cloned()
            .unwrap_or_default();
        for h in hooks {
            let mut natives = Natives {
                host: &mut *self,
                entity: entity.clone(),
                registry: registry.clone(),
            };
            content::process_hook(&mut natives, timers, &h, &event)?;
        }
        Ok(())
    }
    fn register_logout_listener(&mut self, _: &P, _: &str, _: Logout) -> Result<()> {
        refused("SpellbookSwap's logout listener (Lunar spells are not ported)")
    }
    fn clear_logout_listener(&mut self, _: &P, _: &str) -> Result<()> {
        refused("SpellbookSwap's logout listener (Lunar spells are not ported)")
    }
    fn weapon_interface(&mut self, _: &P) -> Result<Option<NoWeapon>> {
        refused("SpellbookSwap's weapon interface (Lunar spells are not ported)")
    }
    fn autocast_set(&mut self, _: &P) -> Result<bool> {
        refused("SpellbookSwap's autocast spell (Lunar spells are not ported)")
    }
    fn select_auto_spell(
        &mut self,
        _: &P,
        weapon: &NoWeapon,
        _: i32,
        _: bool,
        _: bool,
    ) -> Result<()> {
        match *weapon {}
    }
    fn open_tab(&mut self, _: &P, _: Tab<'_, NoWeapon>) -> Result<()> {
        refused("SpellbookSwap's spell book tab (Lunar spells are not ported)")
    }
    fn set_varbit(&mut self, _: &P, _: i32, _: i32, _: bool) -> Result<()> {
        refused("SpellbookSwap's varbit (Lunar spells are not ported)")
    }
    fn attribute_flag(&mut self, entity: &P, key: &str, default: bool) -> Result<bool> {
        self.player(entity)?;
        self.inner()
            .canonical(entity)
            .player
            .attributes()
            .boolean(key, default)
            .map_err(|_| Error::Type)
    }
    fn remove_attribute(&mut self, entity: &P, key: &str) -> Result<()> {
        self.player(entity)?;
        self.inner()
            .canonical(entity)
            .player
            .attributes()
            .remove(key);
        Ok(())
    }
    fn event_npc(&mut self, player: &P) -> Result<Option<NoEventNpc>> {
        self.player(player)?;
        match self
            .inner()
            .canonical(player)
            .player
            .attributes()
            .get("re-npc")
        {
            None => Ok(None),
            // No RandomEventNPC exists here: any other value fails the cast.
            Some(_) => Err(Error::Type),
        }
    }
    fn set_event_npc(&mut self, _: &P, npc: &NoEventNpc) -> Result<()> {
        match *npc {}
    }
    fn random_events_restricted(&mut self, player: &P) -> Result<bool> {
        let monitor = self.ctx.live.monitor(&player.0);
        monitor
            .is_restricted(zones::Restriction::RandomEvents.flag())
            .map_err(err)
    }
    fn skill_random_event(&mut self, _: i32) -> Result<Option<NoRandomEvent>> {
        refused("RandomEvents.getSkillBasedRandomEvent (random event content)")
    }
    fn non_skill_random_event(&mut self) -> Result<NoRandomEvent> {
        refused("RandomEvents.getNonSkillRandom (random event content)")
    }
    fn create_event_npc(&mut self, event: &NoRandomEvent, _: &P) -> Result<NoEventNpc> {
        match *event {}
    }
    fn spawn_location_missing(&mut self, npc: &NoEventNpc) -> Result<bool> {
        match *npc {}
    }
    fn init_event_npc(&mut self, npc: &NoEventNpc) -> Result<()> {
        match *npc {}
    }
    fn terminate_event_npc(&mut self, npc: &NoEventNpc) -> Result<()> {
        match *npc {}
    }
    fn roll_event_loot(&mut self, npc: &NoEventNpc, _: &P) -> Result<Option<Vec<(i32, i32)>>> {
        match *npc {}
    }
    fn dialogue_ids(&mut self, dialogue: &NoDialogue) -> Result<Option<Vec<i32>>> {
        match *dialogue {}
    }
}

// ---------------------------------------------------- the save file

/// `content::Host` while a save file is written or parsed: the timers' own
/// fields, uids, the clock, the shared generator and the log; a parsed
/// timer's messages and skull calls are queued ([`TimerCall`]).
pub struct Saving<'a> {
    pub ticks: i32,
    /// (node, uid, active) of every live player.
    pub players: Vec<(P, i32, bool)>,
    /// The NPC nodes.
    pub npcs: &'a std::collections::BTreeMap<Node, crate::startup::npcs::Npc>,
    /// The rights of the player whose file this is (read before the parse).
    pub rights: Option<Rights>,
    pub generators: &'a mut crate::startup::generators::WorldGenerators,
    pub calls: &'a mut Vec<TimerCall>,
    pub lines: Vec<String>,
    pub skills: Skills,
    pub book: i32,
    pub rest: super::inner::Rest,
    pub failures: Vec<String>,
}

impl content::Host for Saving<'_> {
    type Entity = P;
    type RandomEvent = NoRandomEvent;
    type EventNpc = NoEventNpc;
    type Weapon = NoWeapon;
    type Dialogue = NoDialogue;
    type SkillsContext = super::inner::Rest;
    type SettingsContext = super::inner::Rest;

    fn kind(&self, entity: &P) -> EntityKind {
        if self.players.iter().any(|(p, _, _)| p == entity) {
            EntityKind::Player
        } else if self.npcs.contains_key(&entity.node()) {
            EntityKind::Npc
        } else {
            EntityKind::Other
        }
    }
    fn skills(&mut self, _: &P) -> (&mut Skills, &mut super::inner::Rest) {
        self.failures
            .push("not ported in the live world: skills in a timer's save or parse".into());
        (&mut self.skills, &mut self.rest)
    }
    fn settings(&mut self, _: &P) -> (Settings, &mut super::inner::Rest) {
        self.failures
            .push("not ported in the live world: settings in a timer's save or parse".into());
        (Settings::default(), &mut self.rest)
    }
    fn spell_book(&mut self, _: &P) -> &mut i32 {
        self.failures
            .push("not ported in the live world: the spell book in a timer's save or parse".into());
        &mut self.book
    }
    fn random_event_name(&self, event: &NoRandomEvent) -> String {
        match *event {}
    }
    fn random_event_value_of(&self, _: &str) -> Option<NoRandomEvent> {
        None
    }
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.ticks)
    }
    fn modal_open(&mut self, _: &P) -> Result<bool> {
        refused("hasModalOpen while a save file is read")
    }
    fn script_delay(&mut self, _: &P) -> Result<i32> {
        refused("scripts while a save file is read")
    }
    fn remove_weak_scripts(&mut self, _: &P) -> Result<()> {
        refused("scripts while a save file is read")
    }
    fn name(&mut self, _: &P) -> Result<Option<String>> {
        refused("names while a save file is read")
    }
    fn location(&mut self, _: &P) -> Result<Option<[i32; 3]>> {
        refused("locations while a save file is read")
    }
    fn log(&mut self, level: Level, message: &str) -> Result<()> {
        self.lines.push(format!("{level:?} {message}"));
        Ok(())
    }
    fn exception(&mut self, error: &Error) -> Result<()> {
        self.lines
            .push(format!("printStackTrace (timer) {error:?}"));
        Ok(())
    }
    fn send_message(&mut self, player: &P, text: &str) -> Result<()> {
        self.calls
            .push(TimerCall::Message(player.clone(), text.into()));
        Ok(())
    }
    fn debug(&mut self, player: &P, text: &str) -> Result<()> {
        self.calls
            .push(TimerCall::Debug(player.clone(), text.into()));
        Ok(())
    }
    fn play_audio(&mut self, player: &P, id: i32, delay: i32, loops: i32) -> Result<()> {
        self.calls
            .push(TimerCall::Audio(player.clone(), id, delay, loops));
        Ok(())
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        self.generators
            .shared_next_int(bound)
            .ok_or_else(|| Error::Binding("RANDOM.nextInt: bound must be positive".into()))
    }
    fn manual_hit(&mut self, _: &P, _: &P, _: i32, _: HitsplatType) -> Result<()> {
        refused("hits while a save file is read")
    }
    fn worn(&mut self, _: &P, _: i32) -> Result<Option<i32>> {
        refused("equipment while a save file is read")
    }
    fn artificial(&mut self, _: &P) -> Result<bool> {
        Ok(false)
    }
    fn administrator(&mut self, _: &P) -> Result<bool> {
        match self.rights {
            Some(rights) => Ok(rights == Rights::Administrator),
            None => Err(Error::Null),
        }
    }
    fn uid(&mut self, player: &P) -> Result<i32> {
        self.players
            .iter()
            .find(|(p, _, _)| p == player)
            .map(|(_, uid, _)| *uid)
            .ok_or(Error::Type)
    }
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<P>> {
        Ok(self
            .players
            .iter()
            .find(|(_, u, active)| *u == uid && *active)
            .map(|(p, _, _)| p.clone()))
    }
    fn npc_task(&mut self, _: &P) -> Result<Option<i32>> {
        refused("NPC slayer tasks")
    }
    fn familiar_owner(&mut self, _: &P) -> Result<Option<P>> {
        refused("familiars")
    }
    fn poison_immune(&mut self, _: &P) -> Result<bool> {
        refused("poison immunity while a save file is read")
    }
    fn set_skull_icon(&mut self, player: &P, icon: i32) -> Result<()> {
        self.calls.push(TimerCall::SkullIcon(player.clone(), icon));
        Ok(())
    }
    fn set_skulled(&mut self, player: &P, skulled: bool) -> Result<()> {
        self.calls.push(TimerCall::Skulled(player.clone(), skulled));
        Ok(())
    }
    fn reset_skull(&mut self, player: &P) -> Result<()> {
        self.calls.push(TimerCall::ResetSkull(player.clone()));
        Ok(())
    }
    fn hook(&mut self, _: &P, _: EventClass, _: Hook) -> Result<()> {
        refused("entity hooks while a save file is read")
    }
    fn unhook(&mut self, _: &P, _: &Hook) -> Result<()> {
        refused("entity hooks while a save file is read")
    }
    fn dispatch(
        &mut self,
        _: &P,
        _: Event<NoDialogue>,
        _: &mut Manager,
        _: &Registry,
    ) -> Result<()> {
        refused("entity events while a save file is read")
    }
    fn register_logout_listener(&mut self, _: &P, _: &str, _: Logout) -> Result<()> {
        refused("SpellbookSwap's logout listener (Lunar spells are not ported)")
    }
    fn clear_logout_listener(&mut self, _: &P, _: &str) -> Result<()> {
        refused("SpellbookSwap's logout listener (Lunar spells are not ported)")
    }
    fn weapon_interface(&mut self, _: &P) -> Result<Option<NoWeapon>> {
        refused("SpellbookSwap's weapon interface (Lunar spells are not ported)")
    }
    fn autocast_set(&mut self, _: &P) -> Result<bool> {
        refused("SpellbookSwap's autocast spell (Lunar spells are not ported)")
    }
    fn select_auto_spell(
        &mut self,
        _: &P,
        weapon: &NoWeapon,
        _: i32,
        _: bool,
        _: bool,
    ) -> Result<()> {
        match *weapon {}
    }
    fn open_tab(&mut self, _: &P, _: Tab<'_, NoWeapon>) -> Result<()> {
        refused("SpellbookSwap's spell book tab (Lunar spells are not ported)")
    }
    fn set_varbit(&mut self, _: &P, _: i32, _: i32, _: bool) -> Result<()> {
        refused("SpellbookSwap's varbit (Lunar spells are not ported)")
    }
    fn attribute_flag(&mut self, _: &P, _: &str, _: bool) -> Result<bool> {
        refused("attributes while a save file is read")
    }
    fn remove_attribute(&mut self, _: &P, _: &str) -> Result<()> {
        refused("attributes while a save file is read")
    }
    fn event_npc(&mut self, _: &P) -> Result<Option<NoEventNpc>> {
        refused("attributes while a save file is read")
    }
    fn set_event_npc(&mut self, _: &P, npc: &NoEventNpc) -> Result<()> {
        match *npc {}
    }
    fn random_events_restricted(&mut self, _: &P) -> Result<bool> {
        refused("zones while a save file is read")
    }
    fn skill_random_event(&mut self, _: i32) -> Result<Option<NoRandomEvent>> {
        refused("RandomEvents.getSkillBasedRandomEvent (random event content)")
    }
    fn non_skill_random_event(&mut self) -> Result<NoRandomEvent> {
        refused("RandomEvents.getNonSkillRandom (random event content)")
    }
    fn create_event_npc(&mut self, event: &NoRandomEvent, _: &P) -> Result<NoEventNpc> {
        match *event {}
    }
    fn spawn_location_missing(&mut self, npc: &NoEventNpc) -> Result<bool> {
        match *npc {}
    }
    fn init_event_npc(&mut self, npc: &NoEventNpc) -> Result<()> {
        match *npc {}
    }
    fn terminate_event_npc(&mut self, npc: &NoEventNpc) -> Result<()> {
        match *npc {}
    }
    fn roll_event_loot(&mut self, npc: &NoEventNpc, _: &P) -> Result<Option<Vec<(i32, i32)>>> {
        match *npc {}
    }
    fn dialogue_ids(&mut self, dialogue: &NoDialogue) -> Result<Option<Vec<i32>>> {
        match *dialogue {}
    }
}

// ---------------------------------------------------- the world's operations

impl Live {
    /// Runs `f` with the timers' host for `p` and an alias of `p`'s timer
    /// manager, registering the prototypes first if this is the first timer
    /// operation (`ClassScanner.loadTimers`).
    pub fn with_timers<T>(
        &mut self,
        p: &P,
        f: impl FnOnce(&mut Natives<'_, Host<'_>>, &mut Manager) -> Result<T>,
    ) -> Result<T> {
        if !self.inner.players.contains_key(&p.node()) {
            if self.inner.npcs.rows.contains_key(&p.node()) {
                return self.with_npc_timers(p, f);
            }
            return refused("timers of a node that is neither a player nor an NPC");
        }
        let mut manager = self
            .inner
            .with_profile(p, |profile| alias(&profile.timers))
            .map_err(err)?;
        let registry = self.inner.timers.registry.clone();
        let first = !self.inner.timers.registered;
        self.inner.timers.registered = true;
        let mut host = Host::new(self, p.clone());
        let result = (|| {
            let mut natives = Natives {
                host: &mut host,
                entity: p.clone(),
                registry,
            };
            if first {
                // A throwaway manager: registration touches no entity.
                content::register_prototypes(&mut natives, &mut Manager::default())?;
            }
            f(&mut natives, &mut manager)
        })();
        let finished = host.finish();
        let value = result?;
        finished?;
        Ok(value)
    }

    /// Runs `f` with the timers' host for the NPC `npc` and an alias of its
    /// timer manager (the start-up's, `startup::npcs::Npc::timers`).
    ///
    /// The start-up's `Entity.init` registered three of the four automatic
    /// timers (`startup::timers`); `SkillRestore`, whose hooks it has no port
    /// of, is registered here on the NPC's first timer operation (before
    /// `f`, so its place in the manager's lists is the original's: after the
    /// start-up's, before any timer registered later). Its first run is
    /// due one interval after the NPC's first `processTimers`, as the
    /// original's, which sets it then too.
    pub fn with_npc_timers<T>(
        &mut self,
        npc: &P,
        f: impl FnOnce(&mut Natives<'_, Host<'_>>, &mut Manager) -> Result<T>,
    ) -> Result<T> {
        let node = npc.node();
        let mut manager = match self.inner.npcs.rows.get(&node) {
            Some(row) => alias(&row.timers),
            None => return refused("timers of a node that is not an NPC"),
        };
        let registry = self.inner.timers.registry.clone();
        let first = !self.inner.timers.registered;
        self.inner.timers.registered = true;
        let join = self.inner.timers.npc_restore.insert(node);
        let mut host = Host::new(self, npc.clone());
        let result = (|| {
            let mut natives = Natives {
                host: &mut host,
                entity: npc.clone(),
                registry,
            };
            if first {
                content::register_prototypes(&mut natives, &mut Manager::default())?;
            }
            if join && !content::has(&manager, content::Kind::SkillRestore)? {
                let restore =
                    content::spawn(&mut natives, &mut manager, content::Kind::SkillRestore, &[])?;
                manager.register(Some(&restore), &mut (), &mut natives)?;
            }
            f(&mut natives, &mut manager)
        })();
        let finished = host.finish();
        let value = result?;
        finished?;
        Ok(value)
    }

    /// An NPC's `timers.processTimers()` at the end of its `Entity.tick`.
    pub fn process_npc_timers(&mut self, npc: &P) -> Result<()> {
        self.with_npc_timers(npc, |natives, manager| {
            manager.process(&mut (), &mut NpcNatives { natives })
        })
    }

    /// `TimerManager.onEntityDeath()` of an NPC: its ClearOnDeath timers.
    pub fn npc_timers_on_death(&mut self, npc: &P) -> Result<()> {
        self.with_npc_timers(npc, |natives, manager| {
            manager.death(&mut (), &mut NpcNatives { natives })
        })
    }

    /// `timers.removeTimer(identifier)` of an NPC (`fullRestore`).
    pub fn remove_npc_timer(&mut self, npc: &P, identifier: &str) -> Result<()> {
        self.with_npc_timers(npc, |natives, manager| {
            manager.remove_identifier(identifier, &mut (), &mut NpcNatives { natives })
        })
    }

    /// `TimerRegistry.addAutoTimers(entity)` at `Entity.init`.
    pub fn add_auto_timers_of(&mut self, p: &P) -> Result<()> {
        // GrandExchangeTimer and HolidayRandoms are auto timers of the
        // original's registry without a port here (the Grand Exchange is
        // rs09-w-ge's; the holiday events are content).
        self.inner
            .not_ported
            .skip("addAutoTimers: GrandExchangeTimer and HolidayRandoms (not ported)");
        self.with_timers(p, |natives, manager| {
            let registry = natives.registry.clone();
            registry.add_automatic(manager, &mut (), natives)
        })
    }

    /// `timers.processTimers()`.
    pub fn process_timers_of(&mut self, p: &P) -> Result<()> {
        self.with_timers(p, |natives, manager| manager.process(&mut (), natives))
    }

    /// `hasTimerActive(entity, identifier)` for a live player.
    pub fn has_timer_of(&self, node: Node, identifier: &str) -> Result<bool> {
        let Some(slot) = self.inner.players.get(&node) else {
            return match self.inner.npcs.rows.get(&node) {
                Some(row) => content::has_identifier(&row.timers, identifier),
                None => refused("timers of a node that is neither a player nor an NPC"),
            };
        };
        slot.canonical
            .player
            .with_profile(|profile| content::has_identifier(&profile.timers, identifier))
            .map_err(err)?
    }

    /// Entity.dispatch of an event the timers hook (prayer events).
    pub fn dispatch_timer_event(&mut self, p: &P, event: Event<NoDialogue>) -> Result<()> {
        self.with_timers(p, |natives, manager| {
            let registry = natives.registry.clone();
            let entity = natives.entity.clone();
            content::Host::dispatch(natives.host, &entity, event, manager, &registry)
        })
    }

    /// `Entity.isPoisonImmune()`: a Player's is `timers.getTimer(
    /// "poison:immunity") != null`.
    pub fn timer_poison_immune(&mut self, entity: &P) -> std::result::Result<bool, String> {
        if self.inner.players.contains_key(&entity.node()) {
            return self
                .has_timer_of(entity.node(), "poison:immunity")
                .map_err(|e| format!("{e:?}"));
        }
        self.npc_poison_immune(entity)
    }

    /// Makes the calls a parsed save's timers queued.
    pub fn run_timer_calls(&mut self) {
        for call in std::mem::take(&mut self.inner.timers.calls) {
            let result: std::result::Result<(), String> = match call {
                TimerCall::Message(p, text) => {
                    self.inner.message(&p, &text).map_err(|e| format!("{e:?}"))
                }
                TimerCall::Debug(p, text) => {
                    let debug = self
                        .inner
                        .canonical(&p)
                        .player
                        .attributes()
                        .boolean("debug", false)
                        .unwrap_or(false);
                    if debug {
                        self.inner.message(&p, &text).map_err(|e| format!("{e:?}"))
                    } else {
                        Ok(())
                    }
                }
                TimerCall::Audio(p, id, delay, loops) => {
                    self.inner
                        .queue_native(&p, updates::synth_sound(id, loops, delay));
                    Ok(())
                }
                TimerCall::SkullIcon(p, icon) => {
                    skull::set_skull_icon(self, &p.0, icon).map_err(|e| format!("{e:?}"))
                }
                TimerCall::Skulled(p, value) => {
                    skull::answers::set_skulled(self, &p.0, value).map_err(|e| format!("{e:?}"))
                }
                TimerCall::ResetSkull(p) => skull::reset(self, &p.0).map_err(|e| format!("{e:?}")),
            };
            if let Err(e) = result {
                self.inner
                    .log_line(format!("a parsed timer's call failed: {e}"));
            }
        }
    }
}

/// `spawnTimer(identifier, runInterval)` as the SkullManager asks it: the
/// registry holds a prototype under that identifier (the instance itself is
/// made by [`register_spawned`], which has the player; making it has no
/// effect of its own).
pub fn spawnable(live: &Live, identifier: &str) -> bool {
    let key = rs09_game_core::java_lower::lowercase_root(identifier);
    live.inner.timers.registered
        && live
            .inner
            .timers
            .registry
            .entries()
            .iter()
            .any(|(k, v)| *k == key && v.is_some())
}

/// `TimerRegistry.getTimerInstance(identifier, runInterval)` and
/// `entity.timers.registerTimer(timer)`.
pub fn register_spawned(
    live: &mut Live,
    player: &NodeHandle,
    identifier: &str,
    run_interval: i32,
) -> Result<()> {
    let p = P(player.clone());
    live.with_timers(&p, |natives, manager| {
        let registry = natives.registry.clone();
        let timer = registry.instance(
            identifier,
            &[content::Arg::Int(run_interval)],
            manager,
            &mut (),
            natives,
        )?;
        manager.register_optional(timer.as_ref(), &mut (), natives)
    })
}

/// `removeTimer(player, identifier)`.
pub fn remove_timer(live: &mut Live, player: &NodeHandle, identifier: &str) -> Result<()> {
    let p = P(player.clone());
    live.with_timers(&p, |natives, manager| {
        manager.remove_identifier(identifier, &mut (), natives)
    })
}

/// The timers' `Handle` type, for hosts that hold one.
pub type TimerHandle = Handle;
pub use timers::content::Kind as TimerKind;
