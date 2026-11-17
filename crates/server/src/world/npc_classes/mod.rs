//! The NPC subclass framework and the global NPC handlers
//! (`core/game/node/entity/npc/AbstractNPC`, `IdleAbstractNPC` and
//! `content/global/handlers/npc`).
//!
//! | Original | Here |
//! | --- | --- |
//! | `AbstractNPC` (the id registry `mapping`, `newInstance`, `forId`, `construct`, the autowalk constructors) and `NPC.create` | [`registry`] |
//! | `ShopNPC`, `CowNPC`, `DuckNPC`, `RatNPC`, `GuardNPC`, `MonkNPC` (and its swing handler), `HighwayManNPC`, `IceGiantNPC`, `DarkWizardNPC` | [`Class`], [`classes`] |
//! | `BankerNPC` (its listeners, destination override, `init` booth search and companion) | [`banker`] |
//! | `NPCTalkListener` | [`talk`] |
//! | `IdleAbstractNPC`, `NPCDisturbPlugin`, `HairCutPlugin` | [`idle`] |
//! | the classes' registrations, in the original's scan order | [`listeners`] |
//!
//! Every call the original makes outside these bodies is a required
//! [`Host`] (or [`Factory`]) method with no default. The NPC subclass of a
//! node is data the world keeps with the NPC ([`Class`]); the virtual
//! members a subclass overrides are dispatched here by that class
//! ([`classes::init`], [`classes::tick`], ...). See docs/npc-classes.md.
pub mod banker;
pub mod classes;
pub mod idle;
pub mod listeners;
pub mod registry;
pub mod talk;

pub use rs09_game_core::direction::Direction;

/// What stops a call, as the original's exception classes do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `NullPointerException` (Kotlin's null checks included).
    Null,
    /// `ClassCastException`.
    Cast,
    /// `IndexOutOfBoundsException`.
    Index,
    /// A service failed with what it threw (its message, if any).
    Binding(Option<String>),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// The node classes the bodies cast to or test with `is`/`instanceof`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    /// An `NPC` (its subclass, if any, is [`Host::class`]).
    Npc,
    /// `RandomEventNPC` (also an `NPC`).
    RandomEventNpc,
    /// `HolidayRandomEventNPC` (also an `NPC`).
    HolidayRandomEventNpc,
    /// An `IdleAbstractNPC` subclass (also an `NPC`).
    Idle,
    Scenery,
    Item,
}
impl Kind {
    pub fn is_npc(self) -> bool {
        matches!(
            self,
            Self::Npc | Self::RandomEventNpc | Self::HolidayRandomEventNpc | Self::Idle
        )
    }
}

/// An attribute's value as the original stores it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i32),
    Long(i64),
    Bool(bool),
    Text(String),
    Location([i32; 3]),
}
impl Value {
    /// Kotlin's/Java's unboxing cast to `Boolean`.
    pub fn bool(v: Option<Self>, fail: bool) -> Result<bool> {
        match v {
            None => Ok(fail),
            Some(Self::Bool(b)) => Ok(b),
            Some(_) => Err(Error::Cast),
        }
    }
    /// The unboxing cast to `Integer`.
    pub fn int(v: Option<Self>, fail: i32) -> Result<i32> {
        match v {
            None => Ok(fail),
            Some(Self::Int(i)) => Ok(i),
            Some(_) => Err(Error::Cast),
        }
    }
    /// The unboxing cast to `Long`.
    pub fn long(v: Option<Self>, fail: i64) -> Result<i64> {
        match v {
            None => Ok(fail),
            Some(Self::Long(l)) => Ok(l),
            Some(_) => Err(Error::Cast),
        }
    }
}

/// `Log` levels the classes log at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Err,
}
impl LogLevel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Err => "ERR",
        }
    }
}

/// `CombatStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatStyle {
    Melee,
    Range,
    Magic,
}
impl CombatStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Melee => "MELEE",
            Self::Range => "RANGE",
            Self::Magic => "MAGIC",
        }
    }
}

/// `InteractionType`, as a swing handler's `canSwing` answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionType {
    StillInteract,
    MoveInteract,
    NoInteract,
}

/// What `SpellBook.getSpell` returns: a `CombatSpell` (its button id), or a
/// spell of another class (the `(CombatSpell)` cast fails on it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spell {
    Combat(i32),
    Other,
}

/// `DiaryType` constants the classes reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diary {
    Falador,
}
impl Diary {
    pub fn name(self) -> &'static str {
        match self {
            Self::Falador => "FALADOR",
        }
    }
}

/// An argument of `DialogueInterpreter.open(key, args...)`.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg<N> {
    Node(Option<N>),
    Int(i32),
    Bool(bool),
    /// A `BarcrawlType` constant (its name).
    Barcrawl(&'static str),
}

/// The AbstractNPC subclasses this port has ([`classes`] holds their
/// overrides).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    Shop,
    Banker,
    Cow,
    Duck,
    Rat,
    Guard,
    Monk,
    HighwayMan,
    IceGiant,
    DarkWizard,
}

/// `getIds()` of each class.
pub mod ids {
    /// `ShopNPC.ID`.
    pub const SHOP: [i32; 168] = [
        410, 528, 529, 525, 524, 1436, 590, 591, 971, 1917, 597, 1083, 3541, 7396, 1040, 563, 5798,
        582, 526, 527, 873, 532, 533, 568, 2154, 1334, 2352, 4516, 520, 521, 4716, 3922, 1254,
        2086, 1866, 530, 531, 836, 516, 560, 471, 522, 523, 4651, 4652, 4653, 4654, 4655, 4656,
        4650, 534, 588, 2356, 1860, 550, 575, 683, 682, 4563, 4558, 4559, 519, 559, 562, 581, 554,
        601, 1301, 1039, 2353, 3166, 2161, 2162, 600, 593, 545, 585, 5268, 2305, 2307, 2304, 1783,
        557, 1038, 1433, 7053, 571, 5487, 536, 4294, 4293, 584, 570, 540, 2157, 69669, 569, 572,
        573, 1303, 595, 297, 704, 587, 5110, 5109, 556, 1865, 543, 2198, 580, 1862, 583, 553, 461,
        4513, 3097, 903, 1435, 606, 2623, 594, 579, 2160, 6750, 6898, 6893, 589, 5485, 542, 549,
        3038, 544, 1434, 209, 1980, 546, 6988, 558, 576, 5266, 586, 602, 552, 551, 541, 692, 797,
        564, 2152, 1208, 2233, 3205, 70, 1598, 1596, 1597, 1599, 2259, 3162, 1167, 2620, 2151,
        5486, 4250, 1079, 793, 2572,
    ];
    /// `BankerNPC.NPC_IDS`.
    pub const BANKER: [i32; 49] = [
        44, 45, 494, 495, 496, 497, 498, 499, 1036, 1360, 2163, 2164, 2354, 2355, 2568, 2569, 2570,
        3198, 3199, 5258, 5259, 5260, 5261, 5776, 5777, 5912, 5913, 6200, 6532, 6533, 6534, 6535,
        7445, 7446, 7605, 902, 1702, 166, 3046, 5488, 2619, 4519, 958, 4907, 4296, 7049, 7050,
        6538, 5383,
    ];
    pub const COW: [i32; 6] = [81, 397, 955, 1766, 1767, 3309];
    pub const DUCK: [i32; 3] = [46, 2693, 6113];
    /// `RatNPC.ID`.
    pub const RAT: [i32; 22] = [
        47, 2682, 2980, 2981, 3007, 3008, 3009, 3010, 3011, 3012, 3013, 3014, 3015, 3016, 3017,
        3018, 4396, 4415, 7202, 7204, 7417, 7461,
    ];
    /// `GuardNPC.ID`.
    pub const GUARD: [i32; 76] = [
        9, 32, 163, 164, 196, 197, 206, 253, 254, 255, 256, 274, 275, 296, 298, 299, 447, 448, 449,
        609, 678, 799, 837, 842, 862, 870, 877, 917, 1200, 1203, 1204, 1317, 1710, 1711, 1712,
        2073, 2074, 2134, 2135, 2136, 2236, 2571, 2699, 2700, 2701, 2702, 2703, 3228, 3229, 3230,
        3231, 3232, 3233, 3241, 3407, 3408, 3715, 4257, 4258, 4259, 4260, 4307, 4308, 4309, 4310,
        4311, 4336, 4375, 4603, 4604, 4605, 4606, 5800, 5801, 5919, 5920,
    ];
    pub const MONK: [i32; 1] = [7727];
    pub const HIGHWAY_MAN: [i32; 2] = [180, 2677];
    pub const ICE_GIANT: [i32; 5] = [111, 3072, 4685, 4686, 4687];
    /// `DarkWizardNPC.ID`.
    pub const DARK_WIZARD: [i32; 2] = [172, 174];
}

impl Class {
    pub const ALL: [Self; 10] = [
        Self::Shop,
        Self::Banker,
        Self::Cow,
        Self::Duck,
        Self::Rat,
        Self::Guard,
        Self::Monk,
        Self::HighwayMan,
        Self::IceGiant,
        Self::DarkWizard,
    ];
    /// `getClass().getName()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Shop => "content.global.handlers.npc.ShopNPC",
            Self::Banker => "content.global.handlers.npc.BankerNPC",
            Self::Cow => "content.global.handlers.npc.CowNPC",
            Self::Duck => "content.global.handlers.npc.DuckNPC",
            Self::Rat => "content.global.handlers.npc.RatNPC",
            Self::Guard => "content.global.handlers.npc.GuardNPC",
            Self::Monk => "content.global.handlers.npc.MonkNPC",
            Self::HighwayMan => "content.global.handlers.npc.HighwayManNPC",
            Self::IceGiant => "content.global.handlers.npc.IceGiantNPC",
            Self::DarkWizard => "content.global.handlers.npc.DarkWizardNPC",
        }
    }
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
    /// `getIds()`.
    pub fn ids(self) -> &'static [i32] {
        match self {
            Self::Shop => &ids::SHOP,
            Self::Banker => &ids::BANKER,
            Self::Cow => &ids::COW,
            Self::Duck => &ids::DUCK,
            Self::Rat => &ids::RAT,
            Self::Guard => &ids::GUARD,
            Self::Monk => &ids::MONK,
            Self::HighwayMan => &ids::HIGHWAY_MAN,
            Self::IceGiant => &ids::ICE_GIANT,
            Self::DarkWizard => &ids::DARK_WIZARD,
        }
    }
    /// `getWalkRadius()` where the class overrides it (ShopNPC: 3).
    pub fn walk_radius(self) -> Option<i32> {
        match self {
            Self::Shop => Some(3),
            _ => None,
        }
    }
}

/// What a class's constructor does after `NPC(id, location)`:
/// `super.setWalks(autowalk)` (every AbstractNPC constructor), then its own
/// `setAggressive` (GuardNPC: false; DarkWizardNPC's no-argument one: true).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Constructor {
    pub walks: bool,
    pub aggressive: Option<bool>,
}

/// An AbstractNPC subclass as the registry sees it: its class identity
/// (compared as `getClass()` objects), names, ids and constructors.
pub trait Abstract: Copy + Eq {
    /// `getClass().getName()`.
    fn name(&self) -> &'static str;
    /// `getClass().getSimpleName()`.
    fn simple_name(&self) -> &'static str {
        let name = self.name();
        let name = name.rsplit('.').next().unwrap_or(name);
        name.rsplit('$').next().unwrap_or(name)
    }
    /// `getIds()`.
    fn ids(&self) -> Vec<i32>;
    /// The no-argument constructor `ClassScanner` calls: the id it passes to
    /// `NPC(id, null)` and what it does after.
    fn prototype(&self) -> (i32, Constructor);
    /// `construct(id, location, objects...)`: the constructor it calls, or
    /// `None` when it returns null. `Err(Null)` for a Kotlin override's
    /// non-null location check.
    fn construct(&self, location: Option<[i32; 3]>) -> Result<Option<Constructor>>;
}

impl Abstract for Class {
    fn name(&self) -> &'static str {
        Class::name(*self)
    }
    fn ids(&self) -> Vec<i32> {
        Class::ids(*self).to_vec()
    }
    fn prototype(&self) -> (i32, Constructor) {
        let walks = true;
        match self {
            // MonkNPC(): this(7727, null).
            Self::Monk => (
                7727,
                Constructor {
                    walks,
                    aggressive: None,
                },
            ),
            Self::Guard => (
                0,
                Constructor {
                    walks,
                    aggressive: Some(false),
                },
            ),
            Self::DarkWizard => (
                0,
                Constructor {
                    walks,
                    aggressive: Some(true),
                },
            ),
            _ => (
                0,
                Constructor {
                    walks,
                    aggressive: None,
                },
            ),
        }
    }
    fn construct(&self, location: Option<[i32; 3]>) -> Result<Option<Constructor>> {
        // BankerNPC.construct(id: Int, location: Location, ...): Kotlin checks
        // the non-null parameter on entry.
        if *self == Self::Banker && location.is_none() {
            return Err(Error::Null);
        }
        Ok(Some(Constructor {
            walks: true,
            aggressive: (*self == Self::Guard).then_some(false),
        }))
    }
}

/// Constructing NPCs: what `NPC(id, location[, direction])`, the AbstractNPC
/// constructors and `AbstractNPC.newInstance` reach.
pub trait Factory {
    /// An NPC (or any node the bodies are handed).
    type Npc: Clone;
    /// The subclass identity the world records.
    type Class: Abstract;
    /// `new NPC(id, location, direction)` (a subclass's `super(id,
    /// location)` passes `Direction.SOUTH`): the NPC object with its class.
    fn new_npc(
        &mut self,
        class: Option<Self::Class>,
        id: i32,
        location: Option<[i32; 3]>,
        direction: Option<Direction>,
    ) -> Result<Self::Npc>;
    /// `NPC.setWalks`.
    fn set_walks(&mut self, npc: &Self::Npc, walks: bool) -> Result<()>;
    /// `NPC.setAggressive`.
    fn set_aggressive(&mut self, npc: &Self::Npc, aggressive: bool) -> Result<()>;
    /// `ContentAPI.log(origin, level, message)`.
    fn log(&mut self, origin: &str, level: LogLevel, message: &str) -> Result<()>;
}

/// The services the bodies reach, in the original's terms. Getters the
/// original reads as plain fields (`getId`, `getLocation`) are not
/// recorded calls; every other method is.
pub trait Host: Factory<Class = Class> {
    /// A swing handler object the host owns (the base NPC's, a player's).
    type Handler: Clone;
    /// A dialogue file object (`GCCompletionDialogue`).
    type File;

    // ---------------------------------------------------------------- node facts
    fn kind(&self, node: &Self::Npc) -> Kind;
    /// The NPC's AbstractNPC subclass, if it has one of this port's.
    fn class(&self, npc: &Self::Npc) -> Option<Class>;
    /// Object identity (`==` on nodes).
    fn same(&self, a: &Self::Npc, b: &Self::Npc) -> bool;
    /// `getId()`.
    fn id(&mut self, node: &Self::Npc) -> Result<i32>;
    /// `getLocation()`.
    fn location(&mut self, node: &Self::Npc) -> Result<Option<[i32; 3]>>;

    // ---------------------------------------------------------------- NPC/Entity members
    /// `NPC.init()` (the subclass's `super.init()`).
    fn base_init(&mut self, npc: &Self::Npc) -> Result<()>;
    /// `NPC.tick()`.
    fn base_tick(&mut self, npc: &Self::Npc) -> Result<()>;
    /// `NPC.handleTickActions()`.
    fn base_tick_actions(&mut self, npc: &Self::Npc) -> Result<()>;
    /// `NPC.finalizeDeath(killer)`.
    fn base_finalize_death(&mut self, npc: &Self::Npc, killer: Option<&Self::Npc>) -> Result<()>;
    /// `Entity.onAttack(target)`.
    fn base_on_attack(&mut self, npc: &Self::Npc, target: Option<&Self::Npc>) -> Result<()>;
    /// `NPC.onImpact(entity, state)`.
    fn base_on_impact(&mut self, npc: &Self::Npc, entity: Option<&Self::Npc>) -> Result<()>;
    /// `NPC.getWalkRadius()` (the field).
    fn base_walk_radius(&mut self, npc: &Self::Npc) -> Result<i32>;
    /// An `IdleAbstractNPC`'s own fields (`activeId`, `idle`, `timeout`),
    /// read where the original reads them; `None` for any other node.
    fn idle(&mut self, npc: &Self::Npc) -> Result<Option<idle::Idle>>;
    /// Writes an `IdleAbstractNPC`'s fields where the original assigns one.
    fn set_idle(&mut self, npc: &Self::Npc, state: idle::Idle) -> Result<()>;
    /// `handler.swing(entity, victim, state)` of a host swing handler.
    fn handler_swing(
        &mut self,
        handler: &Self::Handler,
        entity: &Self::Npc,
        victim: Option<&Self::Npc>,
    ) -> Result<i32>;
    /// `NPC.getSwingHandler(swing)`.
    fn base_swing_handler(&mut self, npc: &Self::Npc, swing: bool) -> Result<Self::Handler>;
    /// `entity.getSwingHandler(swing)` for an entity that is no ported NPC
    /// class (a player, another NPC).
    fn swing_handler(&mut self, entity: &Self::Npc, swing: bool) -> Result<Self::Handler>;
    /// `handler.canSwing(entity, victim)`.
    fn can_swing(
        &mut self,
        handler: &Self::Handler,
        entity: &Self::Npc,
        victim: &Self::Npc,
    ) -> Result<Option<InteractionType>>;
    /// `MeleeSwingHandler.swing(entity, victim, state)`.
    fn melee_swing(&mut self, entity: &Self::Npc, victim: Option<&Self::Npc>) -> Result<i32>;
    /// `NPC.transform(id)`.
    fn transform(&mut self, npc: &Self::Npc, id: i32) -> Result<()>;
    /// `NPC.reTransform()`.
    fn re_transform(&mut self, npc: &Self::Npc) -> Result<()>;
    /// `NPC.isInvisible()`.
    fn is_invisible(&mut self, npc: &Self::Npc) -> Result<bool>;
    /// `NPC.getTask()`: its `levelReq`, or `None` for no task.
    fn task_level(&mut self, npc: &Self::Npc) -> Result<Option<i32>>;
    /// `Entity.sendChat(text)`.
    fn send_chat(&mut self, entity: &Self::Npc, text: &str) -> Result<bool>;
    /// `Entity.getDirection()`.
    fn direction(&mut self, entity: &Self::Npc) -> Result<Option<Direction>>;
    /// `Entity.setDirection(direction)`.
    fn set_direction(&mut self, entity: &Self::Npc, direction: Option<Direction>) -> Result<()>;
    /// `Entity.faceLocation(location)`.
    fn face_location(&mut self, entity: &Self::Npc, location: Option<[i32; 3]>) -> Result<()>;
    /// `Entity.inCombat()`.
    fn in_combat(&mut self, entity: &Self::Npc) -> Result<bool>;
    /// `Entity.animate(Animation(id))`.
    fn animate(&mut self, entity: &Self::Npc, animation: i32) -> Result<()>;
    /// `Entity.getAttribute(key[, default])`: the stored value.
    fn get_attribute(&mut self, entity: &Self::Npc, key: &str) -> Result<Option<Value>>;
    /// `Entity.setAttribute(key, value)`.
    fn set_attribute(&mut self, entity: &Self::Npc, key: &str, value: Value) -> Result<()>;
    /// `Entity.removeAttribute(key)`.
    fn remove_attribute(&mut self, entity: &Self::Npc, key: &str) -> Result<()>;
    /// `ContentAPI.getAttribute(entity, key, default)`: the stored value.
    fn api_get_attribute(&mut self, entity: &Self::Npc, key: &str) -> Result<Option<Value>>;

    // ---------------------------------------------------------------- combat, skills
    /// `properties.combatPulse.getVictim()`.
    fn combat_victim(&mut self, entity: &Self::Npc) -> Result<Option<Self::Npc>>;
    /// `properties.combatPulse.isAttacking()`.
    fn is_attacking(&mut self, entity: &Self::Npc) -> Result<bool>;
    /// `properties.combatPulse.setNextAttack(ticks)`.
    fn set_next_attack(&mut self, entity: &Self::Npc, ticks: i32) -> Result<()>;
    /// `properties.combatPulse.setStyle(style)`.
    fn set_combat_style(&mut self, entity: &Self::Npc, style: CombatStyle) -> Result<()>;
    /// `properties.combatPulse.setCombatFlags(victim)`.
    fn set_combat_flags(&mut self, entity: &Self::Npc, victim: &Self::Npc) -> Result<()>;
    /// `properties.combatPulse.attack(victim)`.
    fn combat_attack(&mut self, entity: &Self::Npc, victim: Option<&Self::Npc>) -> Result<()>;
    /// `SpellBook.MODERN.getSpell(button)`.
    fn modern_spell(&mut self, button: i32) -> Result<Option<Spell>>;
    /// `properties.setAutocastSpell(spell)`.
    fn set_autocast_spell(&mut self, entity: &Self::Npc, spell: Option<i32>) -> Result<()>;
    /// `skills.getLifepoints()`.
    fn lifepoints(&mut self, entity: &Self::Npc) -> Result<i32>;
    /// `skills.getMaximumLifepoints()`.
    fn maximum_lifepoints(&mut self, entity: &Self::Npc) -> Result<i32>;
    /// `skills.heal(amount)`.
    fn heal(&mut self, entity: &Self::Npc, amount: i32) -> Result<()>;
    /// `skills.getLevel(skill)`.
    fn level(&mut self, entity: &Self::Npc, skill: i32) -> Result<i32>;
    /// `DeathTask.isDead(entity)`.
    fn is_dead(&mut self, entity: &Self::Npc) -> Result<bool>;
    /// `entity.getZoneMonitor().continueAttack(target, style, message)`.
    fn continue_attack(
        &mut self,
        entity: &Self::Npc,
        target: &Self::Npc,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool>;
    /// `RandomFunction.random(bound)`.
    fn random(&mut self, bound: i32) -> Result<i32>;
    /// `RandomFunction.randomize(bound)`.
    fn randomize(&mut self, bound: i32) -> Result<i32>;
    /// `System.currentTimeMillis()`.
    fn current_time_millis(&mut self) -> Result<i64>;

    // ---------------------------------------------------------------- quests, diaries, items
    /// `getQuestRepository().getQuest(quest)`: `false` for null.
    fn get_quest(&mut self, player: &Self::Npc, quest: &str) -> Result<bool>;
    /// `quest.isStarted(player)`.
    fn quest_started(&mut self, player: &Self::Npc, quest: &str) -> Result<bool>;
    /// `getAchievementDiaryManager().getDiary(type)`: `false` for null.
    fn get_diary(&mut self, player: &Self::Npc, diary: Diary) -> Result<bool>;
    /// `diary.isComplete(level, task)`.
    fn diary_complete(&mut self, diary: Diary, level: i32, task: i32) -> Result<bool>;
    /// `diary.updateTask(player, level, index, complete)`.
    fn diary_update(
        &mut self,
        diary: Diary,
        player: &Self::Npc,
        level: i32,
        index: i32,
        complete: bool,
    ) -> Result<()>;
    /// `GroundItemManager.create(Item(id, amount), location, player)`.
    fn ground_item(
        &mut self,
        id: i32,
        amount: i32,
        location: Option<[i32; 3]>,
        player: &Self::Npc,
    ) -> Result<()>;
    /// `ContentAPI.getScenery(location)`.
    fn get_scenery(&mut self, location: [i32; 3]) -> Result<Option<Self::Npc>>;

    // ---------------------------------------------------------------- messages, dialogues
    /// `ContentAPI.sendMessage(player, text)`.
    fn api_message(&mut self, player: &Self::Npc, text: &str) -> Result<()>;
    /// `Player.sendMessage(text)`.
    fn player_message(&mut self, player: &Self::Npc, text: &str) -> Result<()>;
    /// `getPacketDispatch().sendMessage(text)`.
    fn packet_message(&mut self, player: &Self::Npc, text: &str) -> Result<()>;
    /// `dialogueInterpreter.open(id, args...)`.
    fn open_dialogue_id(
        &mut self,
        player: &Self::Npc,
        id: i32,
        args: &[Arg<Self::Npc>],
    ) -> Result<bool>;
    /// `dialogueInterpreter.open(name, args...)`.
    fn open_dialogue_name(
        &mut self,
        player: &Self::Npc,
        name: &str,
        args: &[Arg<Self::Npc>],
    ) -> Result<bool>;
    /// `dialogueInterpreter.sendDialogue(lines...)`.
    fn send_dialogue(&mut self, player: &Self::Npc, lines: &[&str]) -> Result<()>;
    /// `GCCompletionDialogue(job)` (gnome cooking content; the job's constant name).
    fn gnome_completion_dialogue(&mut self, job: &'static str) -> Result<Self::File>;
    /// `dialogueInterpreter.open(file)`.
    fn open_file(&mut self, player: &Self::Npc, file: Self::File) -> Result<()>;
    /// `DialogueLabeller.open(player, BankerDialogueLabellerFile(), npc)` (the
    /// conversation is DialogueLabeller content, not ported).
    fn banker_conversation(&mut self, player: &Self::Npc, npc: &Self::Npc) -> Result<()>;
    /// `ContentAPI.openDialogue(player, id, npc)`.
    fn api_open_dialogue(&mut self, player: &Self::Npc, id: i32, npc: &Self::Npc) -> Result<()>;

    // ---------------------------------------------------------------- banking
    /// `ContentAPI.removeItem(player, Item(id, amount))` (inventory).
    fn remove_item(&mut self, player: &Self::Npc, id: i32, amount: i32) -> Result<bool>;
    /// `ContentAPI.openBankAccount(player)`.
    fn open_bank_account(&mut self, player: &Self::Npc) -> Result<()>;
    /// `ContentAPI.openGrandExchangeCollectionBox(player)`.
    fn open_collection_box(&mut self, player: &Self::Npc) -> Result<()>;
    /// `ContentAPI.hasSealOfPassage(player)`.
    fn has_seal_of_passage(&mut self, player: &Self::Npc) -> Result<bool>;

    // ---------------------------------------------------------------- content the talk listener reaches
    /// `RandomEvents.randomIDs` (random event content).
    fn random_event_ids(&mut self) -> Result<Vec<i32>>;
    /// `AntiMacro.getEventNpc(player)`.
    fn anti_macro_event_npc(&mut self, player: &Self::Npc) -> Result<Option<Self::Npc>>;
    /// `RandomEventNPC.finalized` / `HolidayRandomEventNPC.finalized`.
    fn finalized(&mut self, npc: &Self::Npc) -> Result<bool>;
    /// `talkTo(npc)` of a random event or holiday NPC.
    fn talk_to(&mut self, event: &Self::Npc, npc: &Self::Npc) -> Result<()>;
    /// `HolidayRandomEvents.holidayRandomIDs`.
    fn holiday_random_ids(&mut self) -> Result<Vec<i32>>;
    /// `HolidayRandoms.getEventNpc(player)`.
    fn holiday_event_npc(&mut self, player: &Self::Npc) -> Result<Option<Self::Npc>>;
    /// `BarcrawlManager.getInstance(player)`.
    fn barcrawl_instance(&mut self, player: &Self::Npc) -> Result<()>;
    /// `instance.isFinished()`.
    fn barcrawl_finished(&mut self, player: &Self::Npc) -> Result<bool>;
    /// `instance.isStarted()`.
    fn barcrawl_started(&mut self, player: &Self::Npc) -> Result<bool>;
    /// `instance.isCompleted(index)`.
    fn barcrawl_completed(&mut self, player: &Self::Npc, index: i32) -> Result<bool>;
}

/// Kotlin `node as NPC`: null fails Kotlin's check, another class the cast.
pub(crate) fn as_npc<'a, H: Host + ?Sized>(h: &H, node: Option<&'a H::Npc>) -> Result<&'a H::Npc> {
    let node = node.ok_or(Error::Null)?;
    if h.kind(node).is_npc() {
        Ok(node)
    } else {
        Err(Error::Cast)
    }
}

/// `Location(x, y, z)`: the plane normalised to 0..3, then the fifteen-bit
/// signed packing of `LocationHash`.
pub fn location(x: i32, y: i32, z: i32) -> [i32; 3] {
    crate::world::prayer::location(x, y, z)
}
/// `Location.transform(dx, dy, dz)`: `(z + dz) % 4` before the constructor.
pub fn transform(l: [i32; 3], dx: i32, dy: i32, dz: i32) -> [i32; 3] {
    location(
        l[0].wrapping_add(dx),
        l[1].wrapping_add(dy),
        l[2].wrapping_add(dz) % 4,
    )
}
/// `Location.transform(direction, steps)`.
pub fn transform_direction(l: [i32; 3], direction: Direction, steps: i32) -> [i32; 3] {
    let (dx, dy) = direction.step();
    location(
        l[0].wrapping_add(dx.wrapping_mul(steps)),
        l[1].wrapping_add(dy.wrapping_mul(steps)),
        l[2],
    )
}
/// `Location.getRegionId()`.
pub fn region_id(l: [i32; 3]) -> i32 {
    ((l[0] >> 6) << 8) | (l[1] >> 6)
}
/// `Direction.name()` in the original's constant names.
pub fn direction_name(d: Direction) -> &'static str {
    match d {
        Direction::NorthWest => "NORTH_WEST",
        Direction::North => "NORTH",
        Direction::NorthEast => "NORTH_EAST",
        Direction::West => "WEST",
        Direction::East => "EAST",
        Direction::SouthWest => "SOUTH_WEST",
        Direction::South => "SOUTH",
        Direction::SouthEast => "SOUTH_EAST",
    }
}
/// The constant of a name, as `Direction.valueOf`.
pub fn direction_named(name: &str) -> Option<Direction> {
    Direction::ALL
        .into_iter()
        .find(|d| direction_name(*d) == name)
}
