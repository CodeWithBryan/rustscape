//! The engine's own timers (`core/game/system/timer/impl`): SkillRestore,
//! Poison, PoisonImmunity, Disease, Frozen, FrozenImmunity, Teleblock,
//! Skulled, HealOverTime, Miasmic, MiasmicImmunity, DragonFireImmunity,
//! SpellbookSwap and the timer part of AntiMacro, with the ContentAPI glue they
//! and their callers use (`spawnTimer`, `getOrStartTimer`, `hasTimerActive`,
//! `applyPoison`, `curePoison`, `isPoisoned`).
//!
//! [`Natives`] is the `crate::timers` [`Context`] and registry [`Factories`]
//! for these classes: the generic manager and registry call it, it runs each
//! class's members, and a timer's own fields live in [`State`] on the timer
//! itself. Skills and special energy are the native [`Skills`] and
//! [`Settings`]; everything else the classes reach is a required [`Host`]
//! service, called in the original order, with no default. Entity event hooks
//! (`Entity.hook`/`unhook`/`dispatch`) are host services too: a host's
//! dispatcher runs [`process_hook`] for the hooks these timers register. This
//! is a library: nothing in a session installs it yet. See
//! docs/content-timers.md.
use super::{
    Context, Error, Flag, Handle, Manager, Result, Timer,
    registry::{Factories, Level, Registry},
};
use crate::definitions::item::Shared;
use crate::definitions::item_config::json::Json;
use crate::player::{
    settings::{self, Settings},
    skills::{self, Skills},
};
use crate::world::combat_pulse::HitsplatType;
use crate::world::prayer::PrayerType;
use crate::world::spells::book::{SpellBook, SpellbookChangeSource};

pub mod antimacro;
mod combat;
mod restore;
mod swap;

/// The fourteen classes, in the order the class scan registers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    AntiMacro,
    Disease,
    DragonFireImmunity,
    Frozen,
    FrozenImmunity,
    HealOverTime,
    Miasmic,
    MiasmicImmunity,
    Poison,
    PoisonImmunity,
    SkillRestore,
    Skulled,
    SpellbookSwap,
    Teleblock,
}

impl Kind {
    pub const ALL: [Self; 14] = [
        Self::AntiMacro,
        Self::Disease,
        Self::DragonFireImmunity,
        Self::Frozen,
        Self::FrozenImmunity,
        Self::HealOverTime,
        Self::Miasmic,
        Self::MiasmicImmunity,
        Self::Poison,
        Self::PoisonImmunity,
        Self::SkillRestore,
        Self::Skulled,
        Self::SpellbookSwap,
        Self::Teleblock,
    ];

    /// The class's simple name.
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::AntiMacro => "AntiMacro",
            Self::Disease => "Disease",
            Self::DragonFireImmunity => "DragonFireImmunity",
            Self::Frozen => "Frozen",
            Self::FrozenImmunity => "FrozenImmunity",
            Self::HealOverTime => "HealOverTime",
            Self::Miasmic => "Miasmic",
            Self::MiasmicImmunity => "MiasmicImmunity",
            Self::Poison => "Poison",
            Self::PoisonImmunity => "PoisonImmunity",
            Self::SkillRestore => "SkillRestore",
            Self::Skulled => "Skulled",
            Self::SpellbookSwap => "SpellbookSwap",
            Self::Teleblock => "Teleblock",
        }
    }

    /// The identifier the constructor passes.
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::AntiMacro => "antimacro",
            Self::Disease => "disease",
            Self::DragonFireImmunity => "dragonfire:immunity",
            Self::Frozen => "frozen",
            Self::FrozenImmunity => "frozen:immunity",
            Self::HealOverTime => "healovertime",
            Self::Miasmic => "miasmic",
            Self::MiasmicImmunity => "miasmic:immunity",
            Self::Poison => "poison",
            Self::PoisonImmunity => "poison:immunity",
            Self::SkillRestore => "skillrestore",
            Self::Skulled => "skulled",
            Self::SpellbookSwap => "spellbook:swap",
            Self::Teleblock => "teleblock",
        }
    }

    /// The class of a timer, by its class name.
    pub fn of(timer: &Timer) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.class_name() == timer.class_name)
    }

    /// `timer is T` for this class (none of the fourteen has a subclass).
    pub fn matches(self, timer: &Timer) -> bool {
        timer.class_name == self.class_name()
    }

    /// A new instance: what the class's constructor sets (`retrieveInstance`).
    pub fn instance(self) -> Handle {
        use Kind::*;
        let (interval, soft, auto, clear) = match self {
            AntiMacro => (0, false, true, false),
            Disease => (30, false, false, true),
            DragonFireImmunity => (1, true, false, true),
            Frozen | FrozenImmunity | HealOverTime | Miasmic | MiasmicImmunity => {
                (1, false, false, true)
            }
            Poison => (30, false, false, true),
            PoisonImmunity => (1, true, false, true),
            SkillRestore => (1, true, true, false),
            Skulled | Teleblock => (1, false, false, true),
            // minutesToTicks(2)
            SpellbookSwap => (200, false, false, false),
        };
        let mut timer = Timer::new(
            interval,
            self.identifier().into(),
            self.class_name().into(),
            self != SkillRestore,
        );
        timer.is_soft = soft;
        timer.is_auto = auto;
        if clear {
            timer.flags.push(Flag::ClearOnDeath);
        }
        Shared::new(timer)
    }
}

/// A timer's own fields, beyond those of `RSTimer`. `E` is the host's entity
/// handle (Poison's `damageSource`), `R` its random event (AntiMacro's
/// `nextRandom`).
#[derive(Clone, Debug)]
pub enum State<E, R> {
    /// Classes without fields of their own.
    None,
    /// `ticksSinceLastRestore` and `restoreTicks` (25 slots each).
    SkillRestore {
        since: [i32; 25],
        restore: [i32; 25],
    },
    /// `damageSource` (`None` while the lateinit field is unset) and
    /// `severity`.
    Poison { source: Option<E>, severity: i32 },
    /// `hitsLeft`.
    Disease { hits_left: i32 },
    /// `ticksRemaining` of PoisonImmunity, DragonFireImmunity and
    /// FrozenImmunity (whose field nothing reads).
    Remaining { ticks: i32 },
    /// `shouldApplyImmunity`.
    Frozen { apply_immunity: bool },
    /// `healRemaining` and `healPerTick`.
    Heal { remaining: i32, per_tick: i32 },
    /// `paused` and `nextRandom`.
    AntiMacro {
        paused: bool,
        next_random: Option<R>,
    },
}

impl<E, R> State<E, R> {
    /// The field initialisers of the class.
    pub fn initial(kind: Kind) -> Self {
        match kind {
            Kind::SkillRestore => Self::SkillRestore {
                since: [0; 25],
                restore: [100; 25],
            },
            Kind::Poison => Self::Poison {
                source: None,
                severity: 0,
            },
            Kind::Disease => Self::Disease { hits_left: 25 },
            Kind::PoisonImmunity | Kind::DragonFireImmunity | Kind::FrozenImmunity => {
                Self::Remaining { ticks: 0 }
            }
            Kind::Frozen => Self::Frozen {
                apply_immunity: false,
            },
            Kind::HealOverTime => Self::Heal {
                remaining: 0,
                per_tick: 0,
            },
            Kind::AntiMacro => Self::AntiMacro {
                paused: false,
                next_random: None,
            },
            _ => Self::None,
        }
    }
}

fn binding(what: &str) -> Error {
    Error::Binding(what.to_owned())
}

/// What a read of the unset lateinit `damageSource` throws.
pub const UNINITIALIZED: &str = "kotlin.UninitializedPropertyAccessException";

/// Runs `f` on the timer's own fields, creating them with the class's
/// initialisers on first use. No host call happens while the timer is locked.
pub fn with_state<E, R, T>(timer: &Handle, f: impl FnOnce(&mut State<E, R>) -> T) -> Result<T>
where
    E: Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    let mut timer = timer.write();
    if timer.content.is_none() {
        let kind = Kind::of(&timer).ok_or_else(|| binding("fields of a non-native timer"))?;
        timer.content = Some(Box::new(State::<E, R>::initial(kind)));
    }
    let state = timer
        .content
        .as_mut()
        .and_then(|content| content.downcast_mut::<State<E, R>>())
        .ok_or_else(|| binding("timer fields of another host"))?;
    Ok(f(state))
}

/// A copy of the timer's own fields.
pub fn state<E, R>(timer: &Handle) -> Result<State<E, R>>
where
    E: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
{
    with_state(timer, |s: &mut State<E, R>| s.clone())
}

/// One argument of `spawnTimer`/`getTimer` (`vararg args: Any`).
#[derive(Clone, Debug)]
pub enum Arg<E> {
    /// A boxed `Int`.
    Int(i32),
    /// A boxed `Boolean`.
    Bool(bool),
    /// An entity.
    Entity(E),
    /// Anything else (a `Long`, a string, ...): no `as?` cast of these
    /// classes accepts it.
    Other,
}

impl<E> Arg<E> {
    fn int(args: &[Self], index: usize) -> Option<i32> {
        match args.get(index) {
            Some(Self::Int(value)) => Some(*value),
            _ => None,
        }
    }
    fn bool(args: &[Self], index: usize) -> Option<bool> {
        match args.get(index) {
            Some(Self::Bool(value)) => Some(*value),
            _ => None,
        }
    }
}

/// What the timers' code checks an entity to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Player,
    Npc,
    /// A summoning familiar (also an NPC).
    Familiar,
    Other,
}

impl EntityKind {
    pub fn is_player(self) -> bool {
        self == Self::Player
    }
    pub fn is_npc(self) -> bool {
        matches!(self, Self::Npc | Self::Familiar)
    }
}

/// The event classes the timers hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventClass {
    PrayerActivated,
    PrayerDeactivated,
    SpellCast,
    SpellbookChanged,
    DialogueClosed,
}

impl EventClass {
    /// The event class's simple name.
    pub fn simple_name(self) -> &'static str {
        match self {
            Self::PrayerActivated => "PrayerActivatedEvent",
            Self::PrayerDeactivated => "PrayerDeactivatedEvent",
            Self::SpellCast => "SpellCastEvent",
            Self::SpellbookChanged => "SpellbookChangeEvent",
            Self::DialogueClosed => "DialogueCloseEvent",
        }
    }
}

/// One dispatched event the timers' hooks process. `D` is the host's
/// dialogue (DialogueCloseEvent's `dialogue`).
#[derive(Clone, Debug)]
pub enum Event<D> {
    PrayerActivated(PrayerType),
    PrayerDeactivated(PrayerType),
    SpellCast {
        book: SpellBook,
        spell: i32,
    },
    SpellbookChange {
        old: SpellBook,
        new: SpellBook,
        source: SpellbookChangeSource,
    },
    DialogueClose(D),
}

impl<D> Event<D> {
    pub fn class(&self) -> EventClass {
        match self {
            Self::PrayerActivated(_) => EventClass::PrayerActivated,
            Self::PrayerDeactivated(_) => EventClass::PrayerDeactivated,
            Self::SpellCast { .. } => EventClass::SpellCast,
            Self::SpellbookChange { .. } => EventClass::SpellbookChanged,
            Self::DialogueClose(_) => EventClass::DialogueClosed,
        }
    }
}

/// A hook these timers register: SkillRestore's two shared objects, or one
/// of a SpellbookSwap instance's three (identified by that timer).
#[derive(Clone, Debug)]
pub enum Hook {
    PrayerActivated,
    PrayerDeactivated,
    SpellCast(Handle),
    SpellbookChange(Handle),
    DialogueClose(Handle),
}

impl Hook {
    /// Reference identity, as `ArrayList.contains`/`remove` compare hooks.
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::PrayerActivated, Self::PrayerActivated)
            | (Self::PrayerDeactivated, Self::PrayerDeactivated) => true,
            (Self::SpellCast(a), Self::SpellCast(b))
            | (Self::SpellbookChange(a), Self::SpellbookChange(b))
            | (Self::DialogueClose(a), Self::DialogueClose(b)) => a.same(b),
            _ => false,
        }
    }
}

/// SpellbookSwap's logout listener (`registerLogoutListener(player,
/// "spellbook:swap") { revertSpellbook(it) }`), capturing its timer.
#[derive(Clone, Debug)]
pub struct Logout(pub Handle);

/// The component `InterfaceManager.openTab` opens.
pub enum Tab<'w, W> {
    /// `new Component(spellBook)` (SpellBookManager.update).
    Book(i32),
    /// The player's weapon interface extension.
    Weapon(&'w W),
}

/// Everything the timers reach outside themselves. The first group are plain
/// accessors to state and identity, not calls. Every other method is one
/// original call; a failure leaves the earlier effects in place.
pub trait Host {
    /// One entity handle; handles keep their identity.
    type Entity: Clone + Send + Sync + 'static;
    /// A `RandomEvents` constant.
    type RandomEvent: Clone + Send + Sync + 'static;
    /// A `RandomEventNPC`.
    type EventNpc: Clone;
    /// A `WeaponInterface` extension.
    type Weapon;
    /// A closed `DialoguePlugin`.
    type Dialogue;
    /// The binding of the native skills to that entity's world.
    type SkillsContext: skills::Context;
    /// The binding of the native settings to that player's world.
    type SettingsContext: settings::Context;

    /// What the entity is (`is Player`, `is NPC`, `is Familiar`).
    fn kind(&self, entity: &Self::Entity) -> EntityKind;
    /// `entity.getSkills()` and the context its members need.
    fn skills(&mut self, entity: &Self::Entity) -> (&mut Skills, &mut Self::SkillsContext);
    /// `player.getSettings()` (a shared handle) and its context.
    fn settings(&mut self, player: &Self::Entity) -> (Settings, &mut Self::SettingsContext);
    /// `player.getSpellBookManager().spellBook`, the field itself.
    fn spell_book(&mut self, player: &Self::Entity) -> &mut i32;
    /// `RandomEvents.name`.
    fn random_event_name(&self, event: &Self::RandomEvent) -> String;
    /// `RandomEvents.valueOf(name)`; `None` where it throws.
    fn random_event_value_of(&self, name: &str) -> Option<Self::RandomEvent>;

    /// `GameWorld.ticks` (`getWorldTicks()`).
    fn ticks(&mut self) -> Result<i32>;
    /// `Player.hasModalOpen()`.
    fn modal_open(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `entity.scripts.delay`.
    fn script_delay(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `entity.scripts.removeWeakScripts()`.
    fn remove_weak_scripts(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `Node.getName()`.
    fn name(&mut self, entity: &Self::Entity) -> Result<Option<String>>;
    /// `Node.getLocation()`.
    fn location(&mut self, entity: &Self::Entity) -> Result<Option<[i32; 3]>>;
    /// `log(origin, level, message)`.
    fn log(&mut self, level: Level, message: &str) -> Result<()>;
    /// `exception.printStackTrace()`: an observation, not a failing call.
    fn exception(&mut self, error: &Error) -> Result<()>;
    /// `sendMessage(player, text)`.
    fn send_message(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `Player.debug(text)`.
    fn debug(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `playAudio(player, id, delay, loops)` (no location, radius 8).
    fn play_audio(&mut self, player: &Self::Entity, id: i32, delay: i32, loops: i32) -> Result<()>;
    /// One `RandomFunction.RANDOM.nextInt(bound)` draw; `bound` is positive.
    fn next_int(&mut self, bound: i32) -> Result<i32>;
    /// `owner.getImpactHandler().manualHit(source, hit, type)`.
    fn manual_hit(
        &mut self,
        owner: &Self::Entity,
        source: &Self::Entity,
        hit: i32,
        kind: HitsplatType,
    ) -> Result<()>;
    /// `player.getEquipment().get(slot)`: the item id, `None` for an empty slot.
    fn worn(&mut self, player: &Self::Entity, slot: i32) -> Result<Option<i32>>;
    /// `Player.isArtificial()`.
    fn artificial(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `player.getRights() == Rights.ADMINISTRATOR` (one rights read).
    fn administrator(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `player.getDetails().getUid()`.
    fn uid(&mut self, player: &Self::Entity) -> Result<i32>;
    /// `Repository.getPlayerByUid(uid)`.
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<Self::Entity>>;
    /// `npc.getTask()`: its level requirement, `None` for no task.
    fn npc_task(&mut self, npc: &Self::Entity) -> Result<Option<i32>>;
    /// `familiar.getOwner()`.
    fn familiar_owner(&mut self, familiar: &Self::Entity) -> Result<Option<Self::Entity>>;
    /// `entity.isPoisonImmune()`.
    fn poison_immune(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `skullManager.setSkullIcon(icon)`.
    fn set_skull_icon(&mut self, player: &Self::Entity, icon: i32) -> Result<()>;
    /// `skullManager.setSkulled(skulled)`.
    fn set_skulled(&mut self, player: &Self::Entity, skulled: bool) -> Result<()>;
    /// `skullManager.reset()`.
    fn reset_skull(&mut self, player: &Self::Entity) -> Result<()>;
    /// `entity.hook(class, hook)`.
    fn hook(&mut self, entity: &Self::Entity, class: EventClass, hook: Hook) -> Result<()>;
    /// `entity.unhook(hook)`.
    fn unhook(&mut self, entity: &Self::Entity, hook: &Hook) -> Result<()>;
    /// `entity.dispatch(event)`. The host runs every hook registered for the
    /// event's class; for these timers' hooks it calls [`process_hook`] with
    /// this entity's timer manager and the registry it is given.
    fn dispatch(
        &mut self,
        entity: &Self::Entity,
        event: Event<Self::Dialogue>,
        timers: &mut Manager,
        registry: &Registry,
    ) -> Result<()>;
    /// `registerLogoutListener(player, key, listener)`.
    fn register_logout_listener(
        &mut self,
        player: &Self::Entity,
        key: &str,
        listener: Logout,
    ) -> Result<()>;
    /// `clearLogoutListener(player, key)`.
    fn clear_logout_listener(&mut self, player: &Self::Entity, key: &str) -> Result<()>;
    /// `player.getExtension(WeaponInterface.class)`.
    fn weapon_interface(&mut self, player: &Self::Entity) -> Result<Option<Self::Weapon>>;
    /// `player.getProperties().getAutocastSpell() != null`.
    fn autocast_set(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `weaponInterface.selectAutoSpell(button, adjust, forget)`.
    fn select_auto_spell(
        &mut self,
        player: &Self::Entity,
        weapon: &Self::Weapon,
        button: i32,
        adjust: bool,
        forget: bool,
    ) -> Result<()>;
    /// `player.getInterfaceManager().openTab(component)`.
    fn open_tab(&mut self, player: &Self::Entity, tab: Tab<'_, Self::Weapon>) -> Result<()>;
    /// `setVarbit(player, id, value, save)`.
    fn set_varbit(&mut self, player: &Self::Entity, id: i32, value: i32, save: bool) -> Result<()>;
    /// `entity.getAttribute(key, default)` of a boolean attribute.
    fn attribute_flag(&mut self, entity: &Self::Entity, key: &str, default: bool) -> Result<bool>;
    /// `entity.removeAttribute(key)`.
    fn remove_attribute(&mut self, entity: &Self::Entity, key: &str) -> Result<()>;
    /// `getAttribute(player, "re-npc", null)` read as a RandomEventNPC (a
    /// value of another class fails as a cast).
    fn event_npc(&mut self, player: &Self::Entity) -> Result<Option<Self::EventNpc>>;
    /// `setAttribute(player, "re-npc", npc)`.
    fn set_event_npc(&mut self, player: &Self::Entity, npc: &Self::EventNpc) -> Result<()>;
    /// `zoneMonitor.isRestricted(ZoneRestriction.RANDOM_EVENTS)`.
    fn random_events_restricted(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `RandomEvents.getSkillBasedRandomEvent(skill)`.
    fn skill_random_event(&mut self, skill: i32) -> Result<Option<Self::RandomEvent>>;
    /// `RandomEvents.getNonSkillRandom()`.
    fn non_skill_random_event(&mut self) -> Result<Self::RandomEvent>;
    /// `event.npc.create(player, event.loot, event.type)`.
    fn create_event_npc(
        &mut self,
        event: &Self::RandomEvent,
        player: &Self::Entity,
    ) -> Result<Self::EventNpc>;
    /// `npc.spawnLocation == null`.
    fn spawn_location_missing(&mut self, npc: &Self::EventNpc) -> Result<bool>;
    /// `npc.init()`.
    fn init_event_npc(&mut self, npc: &Self::EventNpc) -> Result<()>;
    /// `npc.terminate()`.
    fn terminate_event_npc(&mut self, npc: &Self::EventNpc) -> Result<()>;
    /// `npc.loot?.roll(player)`: `None`, without a call, when the NPC has no
    /// loot table.
    fn roll_event_loot(
        &mut self,
        npc: &Self::EventNpc,
        player: &Self::Entity,
    ) -> Result<Option<Vec<(i32, i32)>>>;
    /// `dialogue.getIds()`.
    fn dialogue_ids(&mut self, dialogue: &Self::Dialogue) -> Result<Option<Vec<i32>>>;
}

/// The timers' bindings for one entity: the host, the entity whose timers
/// run, and the world's timer registry.
pub struct Natives<'a, H: Host> {
    pub host: &'a mut H,
    pub entity: H::Entity,
    pub registry: Registry,
}

pub(crate) fn skills_error(error: skills::Error) -> Error {
    match error {
        skills::Error::Index => Error::Index,
        skills::Error::Null => Error::Null,
        skills::Error::Type => Error::Type,
        skills::Error::Number => Error::Number,
        skills::Error::Binding => Error::Binding("skills".into()),
    }
}

pub(crate) fn settings_error(error: settings::Error) -> Error {
    match error {
        settings::Error::Index => Error::Index,
        settings::Error::Null => Error::Null,
        settings::Error::Type => Error::Type,
        settings::Error::Number => Error::Number,
        settings::Error::Binding => Error::Binding("settings".into()),
    }
}

/// `RandomFunction.random(maxValue)`.
pub(crate) fn random<H: Host>(host: &mut H, max: i32) -> Result<i32> {
    if max <= 0 {
        return Ok(0);
    }
    host.next_int(max)
}

/// `RandomFunction.random(a, b)`.
pub(crate) fn random_between<H: Host>(host: &mut H, a: i32, b: i32) -> Result<i32> {
    let n = b.wrapping_sub(a).wrapping_abs();
    let r = if n == 0 { 0 } else { random(host, n)? };
    Ok(a.min(b).wrapping_add(r))
}

/// `core.tools.colorize(line)`.
pub fn colorize(line: &str) -> String {
    let text = line
        .replace("%R", "<col=ff0000>")
        .replace("%O", "<col=ff6600>")
        .replace("%Y", "<col=ffff00>")
        .replace("%G", "<col=66ff33>")
        .replace("%B", "<col=3366ff>")
        .replace("%P", "<col=cc00ff>");
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%'
            && i + 7 <= chars.len()
            && chars[i + 1..i + 7].iter().all(char::is_ascii_hexdigit)
        {
            out.push_str("<col=");
            out.extend(&chars[i + 1..i + 7]);
            out.push('>');
            i += 7;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out + "</col> "
}

impl<H: Host> Natives<'_, H> {
    pub(crate) fn kind(&self) -> EntityKind {
        self.host.kind(&self.entity)
    }

    pub(crate) fn is_player(&self) -> bool {
        self.kind().is_player()
    }

    pub(crate) fn fields<T>(
        &self,
        timer: &Handle,
        f: impl FnOnce(&mut State<H::Entity, H::RandomEvent>) -> T,
    ) -> Result<T> {
        with_state(timer, f)
    }

    /// `sendMessage(entity, text)` when the entity is a player.
    pub(crate) fn message(&mut self, text: &str) -> Result<()> {
        let entity = self.entity.clone();
        self.host.send_message(&entity, text)
    }

    pub(crate) fn debug(&mut self, text: &str) -> Result<()> {
        let entity = self.entity.clone();
        self.host.debug(&entity, text)
    }

    /// `registerTimer(entity, timer)` (`spawnTimer` answers non-null here).
    pub(crate) fn register<S>(
        &mut self,
        manager: &mut Manager,
        timer: Option<&Handle>,
        state: &mut S,
    ) -> Result<()> {
        manager.register_optional(timer, state, self)
    }

    /// `spawnTimer<T>(*args)`.
    pub(crate) fn spawn<S>(
        &mut self,
        manager: &mut Manager,
        kind: Kind,
        args: &[Arg<H::Entity>],
        state: &mut S,
    ) -> Result<Handle> {
        let registry = self.registry.clone();
        registry.spawn_where(|t| kind.matches(t), args, manager, state, self)
    }

    /// `removeTimer<T>(entity)`.
    pub(crate) fn remove_class<S>(
        &mut self,
        manager: &mut Manager,
        kind: Kind,
        state: &mut S,
    ) -> Result<()> {
        manager.remove_where(|t| kind.matches(t), state, self)
    }
}

/// `hasTimerActive<T>(entity)`.
pub fn has(manager: &Manager, kind: Kind) -> Result<bool> {
    Ok(get(manager, kind)?.is_some())
}

/// `getTimer<T>(entity)`.
pub fn get(manager: &Manager, kind: Kind) -> Result<Option<Handle>> {
    manager.get_where(|t| kind.matches(t))
}

/// `hasTimerActive(entity, identifier)`: the answer the spells, teleport,
/// walking and zone slices' timer queries (`"teleblock"`, `"frozen"`, ...)
/// expect.
pub fn has_identifier(manager: &Manager, identifier: &str) -> Result<bool> {
    Ok(manager.get_identifier(identifier)?.is_some())
}

/// `ClassScanner.loadTimers` for these classes: one prototype each, in scan
/// order, through `TimerRegistry.registerTimer`.
pub fn register_prototypes<H: Host>(ctx: &mut Natives<'_, H>, manager: &mut Manager) -> Result<()> {
    let registry = ctx.registry.clone();
    for kind in Kind::ALL {
        registry.register(Some(&kind.instance()), manager, &mut (), ctx)?;
    }
    Ok(())
}

/// `spawnTimer<T>(*args)`.
pub fn spawn<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    kind: Kind,
    args: &[Arg<H::Entity>],
) -> Result<Handle> {
    ctx.spawn(manager, kind, args, &mut ())
}

/// `getOrStartTimer<T>(entity, *args)`.
pub fn get_or_start<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    kind: Kind,
    args: &[Arg<H::Entity>],
) -> Result<Handle> {
    let registry = ctx.registry.clone();
    registry.get_or_start_where(|t| kind.matches(t), args, manager, &mut (), ctx)
}

/// `applyPoison(entity, source, severity)`.
pub fn apply_poison<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    source: &H::Entity,
    severity: i32,
) -> Result<()> {
    if has(manager, Kind::PoisonImmunity)? {
        return Ok(());
    }
    let entity = ctx.entity.clone();
    if ctx.host.poison_immune(&entity)? {
        return Ok(());
    }
    if let Some(existing) = get(manager, Kind::Poison)? {
        let current = ctx.fields(&existing, |s| match s {
            State::Poison { severity, .. } => Ok(*severity),
            _ => Err(binding("Poison fields")),
        })??;
        if current > severity {
            return Ok(());
        }
        ctx.fields(&existing, |s| {
            if let State::Poison {
                severity: field,
                source: from,
            } = s
            {
                *field = severity;
                *from = Some(source.clone());
            }
        })?;
        return Ok(());
    }
    let timer = ctx.spawn(
        manager,
        Kind::Poison,
        &[Arg::Entity(source.clone()), Arg::Int(severity)],
        &mut (),
    )?;
    ctx.register(manager, Some(&timer), &mut ())
}

/// `isPoisoned(entity)`.
pub fn is_poisoned(manager: &Manager) -> Result<bool> {
    has(manager, Kind::Poison)
}

/// `curePoison(entity)`.
pub fn cure_poison<H: Host>(ctx: &mut Natives<'_, H>, manager: &mut Manager) -> Result<()> {
    if !has(manager, Kind::Poison)? {
        return Ok(());
    }
    ctx.remove_class(manager, Kind::Poison, &mut ())?;
    if ctx.is_player() {
        ctx.message("Your poison has been cured.")?;
    }
    Ok(())
}

/// `hook.process(entity, event)` for one of these timers' hooks. A hook given
/// an event of another class fails as the bridge method's cast does.
pub fn process_hook<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    hook: &Hook,
    event: &Event<H::Dialogue>,
) -> Result<()> {
    match (hook, event) {
        (Hook::PrayerActivated, Event::PrayerActivated(t)) => {
            restore::prayer(ctx, manager, *t, true)
        }
        (Hook::PrayerDeactivated, Event::PrayerDeactivated(t)) => {
            restore::prayer(ctx, manager, *t, false)
        }
        (Hook::SpellCast(_), Event::SpellCast { book, spell }) => {
            swap::spell_cast(ctx, manager, *book, *spell)
        }
        (Hook::SpellbookChange(_), Event::SpellbookChange { source, .. }) => {
            swap::book_change(ctx, manager, *source)
        }
        (Hook::DialogueClose(_), Event::DialogueClose(dialogue)) => {
            swap::dialogue_close(ctx, manager, dialogue)
        }
        _ => Err(Error::Type),
    }
}

/// The logout listener SpellbookSwap registers, run by `Player.clear`.
pub fn logout<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    _listener: &Logout,
) -> Result<()> {
    swap::revert(ctx, manager)
}

fn get_timer<E: Clone + Send + Sync + 'static, R: Send + Sync + 'static>(
    kind: Kind,
    args: &[Arg<E>],
) -> Result<Handle> {
    let timer = kind.instance();
    let set = |state: State<E, R>| with_state(&timer, |s: &mut State<E, R>| *s = state);
    let interval = |default: i32| Arg::int(args, 0).unwrap_or(default);
    match kind {
        Kind::Disease => set(State::Disease {
            hits_left: interval(25),
        })?,
        Kind::DragonFireImmunity | Kind::PoisonImmunity => set(State::Remaining {
            ticks: interval(100),
        })?,
        Kind::Frozen => {
            timer.write().run_interval = interval(10);
            set(State::Frozen {
                apply_immunity: Arg::bool(args, 1).unwrap_or(false),
            })?;
        }
        Kind::FrozenImmunity => timer.write().run_interval = interval(7),
        Kind::HealOverTime => {
            timer.write().run_interval = interval(100);
            set(State::Heal {
                remaining: Arg::int(args, 1).unwrap_or(10),
                per_tick: Arg::int(args, 2).unwrap_or(2),
            })?;
        }
        Kind::Miasmic | Kind::MiasmicImmunity | Kind::Teleblock => {
            timer.write().run_interval = interval(100)
        }
        Kind::Skulled => timer.write().run_interval = interval(500),
        Kind::Poison => {
            let Some(Arg::Entity(source)) = args.first() else {
                return Ok(timer);
            };
            set(State::Poison {
                source: Some(source.clone()),
                severity: 0,
            })?;
            // args[1] on a one-element array.
            let severity = match args.get(1).ok_or(Error::Index)? {
                Arg::Int(value) => *value,
                _ => return Ok(timer),
            };
            with_state(&timer, |s: &mut State<E, R>| {
                if let State::Poison {
                    severity: field, ..
                } = s
                {
                    *field = severity;
                }
            })?;
        }
        // getTimer is not overridden: retrieveInstance.
        Kind::AntiMacro | Kind::SkillRestore | Kind::SpellbookSwap => {}
    }
    Ok(timer)
}

impl<S, H: Host> Context<S> for Natives<'_, H> {
    fn is_player(&self, _: &S) -> bool {
        self.kind().is_player()
    }
    fn modal_open(&mut self, _: &mut S) -> Result<bool> {
        let entity = self.entity.clone();
        self.host.modal_open(&entity)
    }
    fn script_delay(&mut self, _: &mut S) -> Result<i32> {
        let entity = self.entity.clone();
        self.host.script_delay(&entity)
    }
    fn ticks(&mut self, _: &mut S) -> Result<i32> {
        self.host.ticks()
    }
    fn entity_name(&mut self, _: &mut S) -> Result<Option<String>> {
        let entity = self.entity.clone();
        self.host.name(&entity)
    }
    fn before_register(
        &mut self,
        timer: &Handle,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<()> {
        match native(timer)? {
            Kind::Disease => combat::disease_before_register(self, timer, manager, state),
            Kind::Frozen => combat::frozen_before_register(self, timer, manager, state),
            Kind::Miasmic => combat::miasmic_before_register(self, timer, manager, state),
            _ => Ok(()),
        }
    }
    fn on_register(&mut self, timer: &Handle, manager: &mut Manager, state: &mut S) -> Result<()> {
        match native(timer)? {
            Kind::AntiMacro => antimacro::on_register(self, timer, manager, state),
            Kind::Poison => combat::poison_on_register(self, timer, manager, state),
            Kind::PoisonImmunity => self.remove_class(manager, Kind::Poison, state),
            Kind::FrozenImmunity => {
                combat::frozen_immunity_on_register(self, timer, manager, state)
            }
            Kind::HealOverTime => combat::heal_on_register(self, timer, manager, state),
            Kind::MiasmicImmunity => {
                if has(manager, Kind::Miasmic)? {
                    self.remove_class(manager, Kind::Miasmic, state)?;
                }
                Ok(())
            }
            Kind::SkillRestore => restore::on_register(self),
            Kind::Skulled => {
                if self.is_player() {
                    let entity = self.entity.clone();
                    self.host.set_skull_icon(&entity, 0)?;
                    self.host.set_skulled(&entity, true)?;
                }
                Ok(())
            }
            Kind::SpellbookSwap => swap::on_register(self, timer),
            Kind::Teleblock => {
                if self.is_player() {
                    self.message("You have been teleblocked.")?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn on_removal(&mut self, timer: &Handle, _: &mut Manager, _: &mut S) -> Result<()> {
        match native(timer)? {
            Kind::Skulled => {
                if self.is_player() {
                    let entity = self.entity.clone();
                    self.host.reset_skull(&entity)?;
                }
                Ok(())
            }
            Kind::SpellbookSwap => swap::on_removal(self, timer),
            _ => Ok(()),
        }
    }
    fn run(&mut self, timer: &Handle, manager: &mut Manager, state: &mut S) -> Result<bool> {
        match native(timer)? {
            Kind::AntiMacro => antimacro::run(self, timer),
            Kind::Disease => combat::disease_run(self, timer),
            Kind::DragonFireImmunity => combat::dragonfire_run(self, timer),
            Kind::Frozen => combat::frozen_run(self, timer, manager, state),
            Kind::FrozenImmunity => {
                if self.is_player() {
                    self.debug("Removed frozen immunity")?;
                }
                Ok(false)
            }
            Kind::HealOverTime => combat::heal_run(self, timer),
            Kind::Miasmic => {
                let entity = self.entity.clone();
                let immunity = self.spawn(
                    manager,
                    Kind::MiasmicImmunity,
                    &[Arg::Entity(entity), Arg::Int(7)],
                    state,
                )?;
                self.register(manager, Some(&immunity), state)?;
                Ok(false)
            }
            Kind::MiasmicImmunity | Kind::Teleblock => Ok(false),
            Kind::Poison => combat::poison_run(self, timer),
            Kind::PoisonImmunity => combat::poison_immunity_run(self, timer),
            Kind::SkillRestore => restore::run(self, timer),
            Kind::Skulled => {
                if !self.is_player() {
                    return Ok(false);
                }
                let entity = self.entity.clone();
                self.host.reset_skull(&entity)?;
                Ok(false)
            }
            Kind::SpellbookSwap => {
                swap::revert(self, manager)?;
                Ok(false)
            }
        }
    }
    fn initial_delay(&mut self, timer: &Handle, _: &mut Manager, _: &mut S) -> Result<i32> {
        // None of the fourteen overrides getInitialRunDelay.
        native(timer)?;
        Ok(timer.read().run_interval)
    }
    fn save(
        &mut self,
        timer: &Handle,
        root: &mut Json,
        _: &mut Manager,
        state: &mut S,
    ) -> Result<()> {
        match native(timer)? {
            Kind::AntiMacro => antimacro::save(self, timer, root),
            Kind::Poison => combat::poison_save(self, timer, root),
            Kind::Disease => {
                let hits = self.fields(timer, |s| match s {
                    State::Disease { hits_left } => *hits_left,
                    _ => 0,
                })?;
                put(root, "hitsLeft", Json::Text(hits.to_string()))
            }
            Kind::DragonFireImmunity | Kind::PoisonImmunity => {
                let ticks = self.fields(timer, |s| match s {
                    State::Remaining { ticks } => *ticks,
                    _ => 0,
                })?;
                put(root, "ticksRemaining", Json::Text(ticks.to_string()))
            }
            Kind::Frozen => {
                super::save_base(timer, root, state, self)?;
                let apply = self.fields(timer, |s| {
                    matches!(
                        s,
                        State::Frozen {
                            apply_immunity: true
                        }
                    )
                })?;
                put(root, "applyImmunity", Json::Bool(apply))
            }
            Kind::HealOverTime => {
                super::save_base(timer, root, state, self)?;
                let (remaining, per_tick) = self.fields(timer, |s| match s {
                    State::Heal {
                        remaining,
                        per_tick,
                    } => (*remaining, *per_tick),
                    _ => (0, 0),
                })?;
                put(root, "healRemaining", Json::Text(remaining.to_string()))?;
                put(root, "healPerTick", Json::Text(per_tick.to_string()))
            }
            Kind::SkillRestore => Err(Error::Type),
            _ => super::save_base(timer, root, state, self),
        }
    }
    fn parse(&mut self, timer: &Handle, data: &Json, _: &mut Manager, _: &mut S) -> Result<()> {
        match native(timer)? {
            Kind::AntiMacro => antimacro::parse(self, timer, data),
            Kind::Poison => combat::poison_parse(self, timer, data),
            Kind::Disease => {
                let hits = integer(data, "hitsLeft")?;
                self.fields(timer, |s| {
                    if let State::Disease { hits_left } = s {
                        *hits_left = hits;
                    }
                })
            }
            Kind::DragonFireImmunity | Kind::PoisonImmunity => {
                let value = integer(data, "ticksRemaining")?;
                self.fields(timer, |s| {
                    if let State::Remaining { ticks } = s {
                        *ticks = value;
                    }
                })
            }
            Kind::Frozen => {
                super::parse_base(timer, data)?;
                let apply = matches!(data.get("applyImmunity"), Some(Json::Bool(true)));
                self.fields(timer, |s| {
                    if let State::Frozen { apply_immunity } = s {
                        *apply_immunity = apply;
                    }
                })
            }
            Kind::HealOverTime => {
                super::parse_base(timer, data)?;
                let remaining = integer(data, "healRemaining")?;
                self.fields(timer, |s| {
                    if let State::Heal {
                        remaining: field, ..
                    } = s
                    {
                        *field = remaining;
                    }
                })?;
                let per_tick = integer(data, "healPerTick")?;
                self.fields(timer, |s| {
                    if let State::Heal {
                        per_tick: field, ..
                    } = s
                    {
                        *field = per_tick;
                    }
                })
            }
            Kind::SkillRestore => Err(Error::Type),
            _ => super::parse_base(timer, data),
        }
    }
    fn resolve(
        &mut self,
        identifier: &str,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<Option<Handle>> {
        let registry = self.registry.clone();
        registry.instance(identifier, &[], manager, state, self)
    }
    fn run_failure(
        &mut self,
        class_name: &str,
        entity_name: Option<&str>,
        _: &mut S,
    ) -> Result<()> {
        let message = format!(
            "Prematurely removing timer {class_name} from {} because it threw an exception when ran. Exception follows:",
            entity_name.unwrap_or("null")
        );
        self.host.log(Level::Error, &message)
    }
    fn unknown_timer(&mut self, identifier: &str, _: &mut S) -> Result<()> {
        let message = format!(
            "Tried to load data for persistent timer identified by {identifier}, but no such timer seems to exist."
        );
        self.host.log(Level::Error, &message)
    }
    fn exception(&mut self, error: &Error, _: &mut S) -> Result<()> {
        self.host.exception(error)
    }
}

impl<S, H: Host> Factories<S> for Natives<'_, H> {
    type Argument = Arg<H::Entity>;
    fn instantiate(
        &mut self,
        prototype: &Handle,
        args: Option<&[Arg<H::Entity>]>,
        _: &Registry,
        _: &mut Manager,
        _: &mut S,
    ) -> Result<Option<Handle>> {
        let kind = native(prototype)?;
        Ok(Some(match args {
            None => kind.instance(),
            Some(args) => get_timer::<H::Entity, H::RandomEvent>(kind, args)?,
        }))
    }
    fn registry_log(
        &mut self,
        level: Level,
        message: String,
        _: &Registry,
        _: &mut Manager,
        _: &mut S,
    ) -> Result<()> {
        self.host.log(level, &message)
    }
}

/// The class of a timer these bindings run; any other class is not ported.
fn native(timer: &Handle) -> Result<Kind> {
    let timer = timer.read();
    Kind::of(&timer).ok_or_else(|| Error::Binding(format!("{} is not ported", timer.class_name)))
}

pub(crate) fn put(root: &mut Json, key: &str, value: Json) -> Result<()> {
    root.put(key, value).map(|_| ()).map_err(|_| Error::Type)
}

/// `root[key].toString().toInt()`.
pub(crate) fn integer(root: &Json, key: &str) -> Result<i32> {
    let text = root.get(key).unwrap_or(&Json::Null).text();
    rs09_game_core::java_string::parse_i32(&text).ok_or(Error::Number)
}
