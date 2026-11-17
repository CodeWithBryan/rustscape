//! The global item handlers of `content/global/handlers/item` a playable
//! world needs, starting with the teleport items: the enchanted jewellery
//! (glory, games necklace, ring of dueling, skills necklace, combat
//! bracelet, ring of slaying, digsite pendant, ring of life) with its dialogue
//! and the EnchantedJewellery table, the jewellery enchanting tablets, the
//! teleport tablets, the house tablet, the teleport crystal (with its
//! dialogue), the ectophial, the explorer's ring and the Falador shield.
//!
//! | Original | Here |
//! | --- | --- |
//! | `EnchantedJewelleryListener` (and its `EnchantedJewelleryDialogueFile`), `content.data.EnchantedJewellery` | [`jewellery`] |
//! | `EnchantJewelleryTabListener`, `TeleTabsListener`, `HouseTeleTabOptionListener` | [`tablets`] |
//! | `TeleportCrystalPlugin` (and `TeleportCrystalDialogue`), `EctophialListener`, `ExplorersRingPlugin`, `FaladorShieldPlugin` | [`items`] |
//! | the classes' registrations, in the original's scan order | [`registry`] |
//!
//! Each Kotlin lambda and OptionHandler the original registers is a typed
//! native value ([`registry::Interaction`], [`registry::OptionPlugin`],
//! [`registry::DialoguePlugin`]); its body is a function here over [`Host`].
//! Queued scripts, pulses, dialogue actions and item-select callbacks are
//! [`Task`] values, dialogue files and plugins [`Dialogue`] values, which the
//! world stores and runs through [`registry::run_task`],
//! [`registry::run_dialogue`] and [`registry::open_dialogue`]. Every call the
//! original makes outside these bodies is a required [`Host`] method with no
//! default. See docs/global-items.md.
pub mod items;
pub mod jewellery;
pub mod registry;
pub mod tablets;

pub use crate::world::global_handlers::{Error, Kind, LogLevel, PulseType, Result, Store};
pub use crate::world::teleport::TeleportType;

/// An attribute value as the original stores it.
pub type Value = crate::world::global_handlers::Value<()>;

/// `FacialExpression` constants these handlers use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expression {
    Neutral,
    HalfGuilty,
    Friendly,
}
impl Expression {
    pub fn name(self) -> &'static str {
        match self {
            Self::Neutral => "NEUTRAL",
            Self::HalfGuilty => "HALF_GUILTY",
            Self::Friendly => "FRIENDLY",
        }
    }
}

/// `Animator.Priority`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Mid,
    High,
    VeryHigh,
}
impl Priority {
    pub fn name(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Mid => "MID",
            Self::High => "HIGH",
            Self::VeryHigh => "VERY_HIGH",
        }
    }
}

/// `Animation(id, delay, priority)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anim {
    pub id: i32,
    pub delay: i32,
    pub priority: Priority,
}
impl Anim {
    /// `Animation(id)`: delay 0, MID.
    pub const fn new(id: i32) -> Self {
        Self {
            id,
            delay: 0,
            priority: Priority::Mid,
        }
    }
    /// `Animation.RESET`: `Animation(-1, VERY_HIGH)`.
    pub const RESET: Self = Self {
        id: -1,
        delay: 0,
        priority: Priority::VeryHigh,
    };
}

/// `Graphics(id, height, delay)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gfx {
    pub id: i32,
    pub height: i32,
    pub delay: i32,
}
impl Gfx {
    /// `Graphics(id)`: height and delay 0.
    pub const fn new(id: i32) -> Self {
        Self {
            id,
            height: 0,
            delay: 0,
        }
    }
}

/// `QueueStrength`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueStrength {
    Weak,
    Normal,
    Strong,
    Soft,
}
impl QueueStrength {
    pub fn name(self) -> &'static str {
        match self {
            Self::Weak => "WEAK",
            Self::Normal => "NORMAL",
            Self::Strong => "STRONG",
            Self::Soft => "SOFT",
        }
    }
}

/// `TeleportMethod`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleportMethod {
    Jewelry,
    Spell,
    Npc,
}
impl TeleportMethod {
    pub fn name(self) -> &'static str {
        match self {
            Self::Jewelry => "JEWELRY",
            Self::Spell => "SPELL",
            Self::Npc => "NPC",
        }
    }
}

/// The `Quests` constants these handlers check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quest {
    HauntedMine,
    GhostsAhoy,
    MourningsEndPartI,
}
impl Quest {
    pub fn name(self) -> &'static str {
        match self {
            Self::HauntedMine => "HAUNTED_MINE",
            Self::GhostsAhoy => "GHOSTS_AHOY",
            Self::MourningsEndPartI => "MOURNINGS_END_PART_I",
        }
    }
}

/// The runecrafting `Altar` constants the altar tablets check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Altar {
    Air,
    Mind,
    Water,
    Earth,
    Fire,
    Body,
    Cosmic,
    Chaos,
    Nature,
    Law,
    Death,
    Blood,
    Astral,
}
impl Altar {
    pub fn name(self) -> &'static str {
        match self {
            Self::Air => "AIR",
            Self::Mind => "MIND",
            Self::Water => "WATER",
            Self::Earth => "EARTH",
            Self::Fire => "FIRE",
            Self::Body => "BODY",
            Self::Cosmic => "COSMIC",
            Self::Chaos => "CHAOS",
            Self::Nature => "NATURE",
            Self::Law => "LAW",
            Self::Death => "DEATH",
            Self::Blood => "BLOOD",
            Self::Astral => "ASTRAL",
        }
    }
}

/// `DiaryType` constants the handlers finish tasks of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diary {
    Varrock,
}
impl Diary {
    pub fn name(self) -> &'static str {
        match self {
            Self::Varrock => "VARROCK",
        }
    }
}

/// `PlayerMonitor` `LogType` constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorLog {
    DupeAlert,
}
impl MonitorLog {
    pub fn name(self) -> &'static str {
        match self {
            Self::DupeAlert => "DUPE_ALERT",
        }
    }
}

/// ContentAPI `removeItem(player, item: T, ...)`'s item: an Item or an id.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemArg<N> {
    Item(N),
    Id(i32),
}

/// A value in a `ServerStore` archive (a JSONObject): its boxed class.
#[derive(Clone, Debug, PartialEq)]
pub enum StoreValue {
    Int(i32),
    Long(i64),
    Double(f64),
    Float(f32),
    Text(String),
    Bool(bool),
}

/// `JSONObject.getInt(key, default)` (ServerStore): Long, Double, Float and
/// Integer values convert with Kotlin's `toInt`, anything else (absent
/// included) is the default.
pub fn store_int(value: Option<&StoreValue>, default: i32) -> i32 {
    match value {
        Some(StoreValue::Long(l)) => *l as i32,
        Some(StoreValue::Double(d)) => *d as i32,
        Some(StoreValue::Float(f)) => *f as i32,
        Some(StoreValue::Int(i)) => *i,
        _ => default,
    }
}

/// An argument a dialogue plugin is opened with (`Object... args`).
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Int(i32),
    Text(String),
}

/// What a deferred body is run with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskInput {
    /// A queued script's stage.
    Stage(i32),
    /// A pulse's `pulse()`.
    Pulse,
    /// A dialogue action's button.
    Button(i32),
    /// An item-select callback's slot and option index.
    Select { slot: i32, option: i32 },
}

/// What a deferred body hands back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A script's or pulse's boolean.
    Bool(bool),
    /// A `Unit` body.
    Unit,
}

/// The deferred bodies the handlers hand the world (queued scripts, pulses,
/// dialogue actions and item-select callbacks), with what each captured.
#[derive(Clone, Debug, PartialEq)]
pub enum Task<N> {
    /// `EnchantedJewellery.attemptTeleport`'s queued script.
    Jewellery(jewellery::Teleport<N>),
    /// EnchantJewelleryTabListener's queued script.
    Enchant { player: N, node: N, enchanted: i32 },
    /// EctophialListener's queued script.
    Ectophial { player: N, node: N },
    /// ExplorersRingPlugin's dialogue action (opens the item select).
    AlchemyAction { player: N, remaining: i32 },
    /// ExplorersRingPlugin's item-select callback.
    AlchemySelect { player: N, remaining: i32 },
    /// FaladorShieldPlugin's emote pulse, with its own `delay` counter.
    FaladorEmote { entity: N, level: i32, delay: i32 },
}

/// The dialogue files and plugins the handlers open, with their state.
#[derive(Clone, Debug, PartialEq)]
pub enum Dialogue<N> {
    /// `EnchantedJewelleryListener.EnchantedJewelleryDialogueFile`.
    Jewellery {
        player: N,
        jewellery: jewellery::Jewellery,
        item: N,
        replace: bool,
        equipped: bool,
        stage: i32,
    },
    /// `TeleportCrystalPlugin.TeleportCrystalDialogue` (`itemId` is null
    /// until `open`).
    Crystal {
        player: N,
        item_id: Option<i32>,
        stage: i32,
    },
}
impl<N> Dialogue<N> {
    /// The file's or plugin's stage.
    pub fn stage(&self) -> i32 {
        match self {
            Self::Jewellery { stage, .. } | Self::Crystal { stage, .. } => *stage,
        }
    }
}

/// The services the handlers call, in the original's terms. A method that
/// returns no `Result` is a field read of the original.
pub trait Host {
    /// A node (player, NPC, scenery, item); `==` is identity.
    type Node: Clone + PartialEq + std::fmt::Debug;

    // ---------------------------------------------------------- node fields
    fn kind(&self, node: &Self::Node) -> Kind;
    /// `node.getId()`.
    fn id(&mut self, node: &Self::Node) -> Result<i32>;
    /// `node.getName()`.
    fn name(&mut self, node: &Self::Node) -> Result<Option<String>>;
    /// `item.getAmount()`.
    fn amount(&mut self, item: &Self::Node) -> Result<i32>;
    /// `item.getSlot()`.
    fn slot(&mut self, item: &Self::Node) -> Result<i32>;
    /// `node.getLocation()`.
    fn location(&mut self, node: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `String.toLowerCase()` / `lowercase(Locale.getDefault())`.
    fn lowercase_default(&self, text: &str) -> String;
    /// Kotlin's `lowercase()` (`Locale.ROOT`).
    fn lowercase_root(&self, text: &str) -> String;
    /// `GameWorld.ticks`.
    fn ticks(&mut self) -> i32;
    /// `System.currentTimeMillis()`.
    fn current_time_millis(&mut self) -> i64;
    /// `ServerConstants.HOME_LOCATION` as EnchantedJewellery's RING_OF_LIFE
    /// read it when the table was initialised (`!!`: `None` is `Null`).
    fn home_location(&mut self) -> Option<[i32; 3]>;
    /// `player.username` (`getUsername()`).
    fn username(&mut self, player: &Self::Node) -> Result<String>;

    // ---------------------------------------------------------- objects
    /// `new Item(id, amount)` (the definition lookup).
    fn new_item(&mut self, id: i32, amount: i32) -> Result<Self::Node>;
    /// `new NPC(id)`.
    fn new_npc(&mut self, id: i32) -> Result<()>;

    // ---------------------------------------------------------- attributes
    /// `entity.getAttribute(key[, default])` with GameAttributes' "/save:"
    /// rule: the stored value, if any.
    fn get_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<Option<Value>>;
    /// `entity.setAttribute(key, value)` ("/save:" stripped and saved).
    fn set_attribute(&mut self, entity: &Self::Node, key: &str, value: Value) -> Result<()>;

    // ---------------------------------------------------------- messages and dialogue
    /// `sendMessage`, `player.sendMessage`, `packetDispatch.sendMessage`.
    fn send_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `packetDispatch.sendMessages(texts...)`.
    fn send_messages(&mut self, player: &Self::Node, texts: &[&str]) -> Result<()>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn send_dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// ContentAPI `sendNPCDialogue(player, npc, msg, expr, hide)`.
    fn npc_dialogue(
        &mut self,
        player: &Self::Node,
        npc: i32,
        message: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()>;
    /// ContentAPI `openDialogue(player, file)`: the world closes the current
    /// dialogue, loads the file and stores it to run through
    /// [`registry::run_dialogue`].
    fn open_dialogue(&mut self, player: &Self::Node, dialogue: Dialogue<Self::Node>) -> Result<()>;
    /// `dialogueInterpreter.open(key, args...)`.
    fn open_dialogue_plugin(&mut self, player: &Self::Node, key: i32, args: &[Arg])
    -> Result<bool>;
    /// `interpreter.sendOptions(title, options...)`.
    fn send_options(&mut self, player: &Self::Node, title: &str, options: &[&str]) -> Result<()>;
    /// `interpreter.close()`.
    fn interpreter_close(&mut self, player: &Self::Node) -> Result<bool>;
    /// ContentAPI `addDialogueAction(player, action)`.
    fn add_dialogue_action(&mut self, player: &Self::Node, action: Task<Self::Node>) -> Result<()>;
    /// ContentAPI `sendItemSelect(player, options..., keepAlive, callback)`.
    fn send_item_select(
        &mut self,
        player: &Self::Node,
        options: &[&str],
        keep_alive: bool,
        callback: Task<Self::Node>,
    ) -> Result<()>;
    /// ContentAPI `setVarp(player, index, value, save)`.
    fn set_varp(&mut self, player: &Self::Node, index: i32, value: i32, save: bool) -> Result<()>;

    // ---------------------------------------------------------- requirements
    /// ContentAPI `hasRequirement(player, quest, message)`.
    fn has_requirement(&mut self, player: &Self::Node, quest: Quest, message: bool)
    -> Result<bool>;
    /// ContentAPI `isQuestComplete(player, quest)`.
    fn is_quest_complete(&mut self, player: &Self::Node, quest: Quest) -> Result<bool>;
    /// ContentAPI `hasLevelStat(player, skill, level)`.
    fn has_level_stat(&mut self, player: &Self::Node, skill: i32, level: i32) -> Result<bool>;
    /// `MysteriousRuinListener.checkReq(player, altar)` (runecrafting content).
    fn altar_requirement(&mut self, player: &Self::Node, altar: Altar) -> Result<bool>;
    /// `WildernessZone.checkTeleport(player, level)`.
    fn wilderness_check_teleport(&mut self, player: &Self::Node, level: i32) -> Result<bool>;
    /// `zoneMonitor.teleport(type, node)`.
    fn zone_teleport(&mut self, player: &Self::Node, kind: i32, node: &Self::Node) -> Result<bool>;
    /// `entity.isTeleBlocked()`.
    fn is_tele_blocked(&mut self, entity: &Self::Node) -> Result<bool>;

    // ---------------------------------------------------------- scripts, locks, pulses
    /// ContentAPI `queueScript(entity, delay, strength, persist, script)`.
    fn queue_script(
        &mut self,
        entity: &Self::Node,
        delay: i32,
        strength: QueueStrength,
        persist: bool,
        script: Task<Self::Node>,
    ) -> Result<()>;
    /// ContentAPI `delayScript(entity, ticks)`.
    fn delay_script(&mut self, entity: &Self::Node, ticks: i32) -> Result<bool>;
    /// ContentAPI `stopExecuting(entity)`.
    fn stop_executing(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `entity.scripts.delay = value`.
    fn set_script_delay(&mut self, entity: &Self::Node, delay: i32) -> Result<()>;
    /// `entity.lock(ticks)`.
    fn lock(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// `entity.lock()`.
    fn lock_all(&mut self, entity: &Self::Node) -> Result<()>;
    /// `entity.unlock()`.
    fn unlock(&mut self, entity: &Self::Node) -> Result<()>;
    /// `pulseManager.clear(type)`.
    fn pulse_clear(&mut self, entity: &Self::Node, kind: PulseType) -> Result<()>;
    /// `GameWorld.getPulser().submit(pulse)`.
    fn submit_world(&mut self, pulse: Task<Self::Node>) -> Result<()>;

    // ---------------------------------------------------------- visuals and audio
    /// `animator.animate(animation)` (`entity.animate`).
    fn animate(&mut self, entity: &Self::Node, animation: Anim) -> Result<bool>;
    /// `animator.forceAnimation(animation)`.
    fn force_animation(&mut self, entity: &Self::Node, animation: Anim) -> Result<bool>;
    /// `animator.animate(animation, graphics)` (`entity.visualize`).
    fn visualize(&mut self, entity: &Self::Node, animation: Anim, graphics: Gfx) -> Result<bool>;
    /// `animator.graphics(graphics)` (`entity.graphics`).
    fn graphics(&mut self, entity: &Self::Node, graphics: Gfx) -> Result<bool>;
    /// `Animation.getDuration()` (the definition's duration ticks, 1 without one).
    fn animation_duration(&mut self, id: i32) -> Result<i32>;
    /// ContentAPI `playAudio(player, id, delay, loops, location, radius)`.
    fn play_audio(
        &mut self,
        player: &Self::Node,
        id: i32,
        delay: i32,
        loops: i32,
        location: Option<[i32; 3]>,
        radius: i32,
    ) -> Result<()>;
    /// ContentAPI `playGlobalAudio(location, id, delay, loops, radius)`.
    fn play_global_audio(
        &mut self,
        location: [i32; 3],
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> Result<()>;
    /// `entity.faceLocation(location)`.
    fn face_location(&mut self, entity: &Self::Node, location: [i32; 3]) -> Result<()>;

    // ---------------------------------------------------------- teleporting
    /// `entity.teleporter.send(location, type)` (native `teleport::Manager`).
    fn teleport_send(
        &mut self,
        entity: &Self::Node,
        location: [i32; 3],
        kind: TeleportType,
    ) -> Result<bool>;
    /// `entity.properties.teleportLocation = location`.
    fn set_teleport_location(&mut self, entity: &Self::Node, location: [i32; 3]) -> Result<()>;
    /// `impactHandler.disabledTicks = ticks`.
    fn set_disabled_ticks(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// `player.dispatch(TeleportEvent(type, method, source, location))`.
    fn teleport_event(
        &mut self,
        player: &Self::Node,
        kind: TeleportType,
        method: TeleportMethod,
        source: &Self::Node,
        location: [i32; 3],
    ) -> Result<()>;
    /// ContentAPI `closeInterface(player)`.
    fn close_interface(&mut self, player: &Self::Node) -> Result<()>;
    /// ContentAPI `closeAllInterfaces(player)`.
    fn close_all_interfaces(&mut self, player: &Self::Node) -> Result<()>;
    /// `achievementDiaryManager.finishTask(player, type, level, task)`.
    fn finish_diary_task(
        &mut self,
        player: &Self::Node,
        diary: Diary,
        level: i32,
        task: i32,
    ) -> Result<()>;

    // ---------------------------------------------------------- containers
    /// ContentAPI `removeItem(player, item, container)`.
    fn remove_item(
        &mut self,
        player: &Self::Node,
        item: ItemArg<Self::Node>,
        store: Store,
    ) -> Result<bool>;
    /// ContentAPI `replaceSlot(player, slot, item, currentItem, container)`:
    /// the item that was in the slot.
    fn replace_slot(
        &mut self,
        player: &Self::Node,
        slot: i32,
        item: &Self::Node,
        current: Option<&Self::Node>,
        store: Store,
    ) -> Result<Option<Self::Node>>;
    /// ContentAPI `addItem(player, id, amount, container)`.
    fn add_item(&mut self, player: &Self::Node, id: i32, amount: i32, store: Store)
    -> Result<bool>;
    /// ContentAPI `addItemOrDrop(player, id, amount)`.
    fn add_item_or_drop(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<()>;
    /// ContentAPI `inInventory(player, id, amount)`.
    fn in_inventory(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `inventory.remove(item)`.
    fn inventory_remove(&mut self, player: &Self::Node, item: &Self::Node) -> Result<bool>;
    /// `inventory.add(item)`.
    fn inventory_add(&mut self, player: &Self::Node, item: &Self::Node) -> Result<bool>;
    /// `inventory[slot]`.
    fn inventory_get(&mut self, player: &Self::Node, slot: i32) -> Result<Option<Self::Node>>;

    // ---------------------------------------------------------- logs
    /// ContentAPI `log(origin, type, message)`.
    fn log(&mut self, class: &str, level: LogLevel, message: &str) -> Result<()>;
    /// `PlayerMonitor.log(player, type, details)`.
    fn monitor_log(&mut self, player: &Self::Node, kind: MonitorLog, details: &str) -> Result<()>;

    // ---------------------------------------------------------- slayer (content)
    /// `SlayerManager.getInstance(player).hasTask()`.
    fn slayer_has_task(&mut self, player: &Self::Node) -> Result<bool>;
    /// `SlayerManager.getInstance(player).master?.npc`.
    fn slayer_master_npc(&mut self, player: &Self::Node) -> Result<Option<i32>>;
    /// ContentAPI `getSlayerTaskName(player)`.
    fn slayer_task_name(&mut self, player: &Self::Node) -> Result<String>;
    /// ContentAPI `getSlayerTaskKillsRemaining(player)`.
    fn slayer_kills_remaining(&mut self, player: &Self::Node) -> Result<i32>;
    /// `SlayerManager.getInstance(player).flags.taskFlags`.
    fn slayer_task_flags(&mut self, player: &Self::Node) -> Result<i32>;

    // ---------------------------------------------------------- house (construction content)
    /// `houseManager.location.exitLocation`.
    fn house_exit_location(&mut self, player: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `houseManager.preEnter(player, buildingMode)`.
    fn house_pre_enter(&mut self, player: &Self::Node, building: bool) -> Result<()>;
    /// `houseManager.getEnterLocation()`.
    fn house_enter_location(&mut self, player: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `houseManager.postEnter(player, buildingMode)`.
    fn house_post_enter(&mut self, player: &Self::Node, building: bool) -> Result<()>;

    // ---------------------------------------------------------- store, energy, magic, prayer
    /// `ServerStore.getArchive(name)`.
    fn store_archive(&mut self, name: &str) -> Result<()>;
    /// The archive's `get(key)`.
    fn store_get(&mut self, archive: &str, key: &str) -> Result<Option<StoreValue>>;
    /// The archive's `put(key, value)`.
    fn store_put(&mut self, archive: &str, key: &str, value: StoreValue) -> Result<()>;
    /// `settings.runEnergy`.
    fn run_energy(&mut self, player: &Self::Node) -> Result<f64>;
    /// `settings.updateRunEnergy(delta)`.
    fn update_run_energy(&mut self, player: &Self::Node, delta: f64) -> Result<()>;
    /// `ModernListeners().alchemize(player, item, high, explorersRing)`.
    fn alchemize(
        &mut self,
        player: &Self::Node,
        item: &Self::Node,
        high: bool,
        explorers_ring: bool,
    ) -> Result<bool>;
    /// `PrayerEffect(base, bonus, roundUp).activate(player)`.
    fn prayer_effect(
        &mut self,
        player: &Self::Node,
        base: f64,
        bonus: f64,
        round_up: bool,
    ) -> Result<()>;
}

/// Java `node.asItem()` / `(Item) node`: another class fails the cast.
pub(crate) fn as_item<'a, H: Host + ?Sized>(h: &H, node: &'a H::Node) -> Result<&'a H::Node> {
    if h.kind(node) == Kind::Item {
        Ok(node)
    } else {
        Err(Error::Cast)
    }
}

/// `Item.toString()`: `"Item id=" + getId() + ", name=" + getName() + ", amount=" + amount`.
pub fn item_to_string<H: Host + ?Sized>(h: &mut H, item: Option<&H::Node>) -> Result<String> {
    let Some(item) = item else {
        return Ok("null".into());
    };
    let id = h.id(item)?;
    let name = h.name(item)?;
    let amount = h.amount(item)?;
    Ok(format!(
        "Item id={id}, name={}, amount={amount}",
        name.as_deref().unwrap_or("null")
    ))
}

// ---------------------------------------------------------------------- ContentAPI
//
// The ContentAPI functions whose bodies the oracle compiles verbatim; every
// other ContentAPI function is a Host service.

/// ContentAPI `teleport(entity, loc, type = INSTANT)`: INSTANT writes the
/// teleport location and answers true, any other type is `teleporter.send`.
pub fn teleport<H: Host + ?Sized>(
    h: &mut H,
    entity: &H::Node,
    location: [i32; 3],
    kind: TeleportType,
) -> Result<bool> {
    if kind == TeleportType::Instant {
        h.set_teleport_location(entity, location)?;
        Ok(true)
    } else {
        h.teleport_send(entity, location, kind)
    }
}

/// ContentAPI `delayEntity(entity, ticks)`: the script delay, then `lock`.
pub fn delay_entity<H: Host + ?Sized>(h: &mut H, entity: &H::Node, ticks: i32) -> Result<()> {
    let delay = h.ticks().wrapping_add(ticks);
    h.set_script_delay(entity, delay)?;
    h.lock(entity, ticks)
}

/// ContentAPI `resetAnimator(player)`: `animate(Animation(-1, VERY_HIGH))`.
pub fn reset_animator<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<()> {
    h.animate(player, Anim::RESET)?;
    Ok(())
}

/// ContentAPI `animate(entity, anim, forced)`.
pub fn animate<H: Host + ?Sized>(
    h: &mut H,
    entity: &H::Node,
    animation: Anim,
    forced: bool,
) -> Result<()> {
    if forced {
        h.force_animation(entity, animation)?;
    } else {
        h.animate(entity, animation)?;
    }
    Ok(())
}
