//! The everyday item options and item-on-object basics of
//! `content/global/handlers/item` (the second pass of that slice; the
//! teleport items are [`crate::world::global_items`]).
//!
//! | Original | Here |
//! | --- | --- |
//! | `EmptyOptionListener`, `LightSourceListeners`, `CasketListener`, `BirdNestScript`/`BirdNestPlugin` (and `content.data.tables.BirdNest`), `LampPlugin` (and `content.data.Lamps`), `StarterPackPlugin`, `RingUnchargePlugin`, `RunePackPlugin`, `HerbBoxPlugin`, `NewspaperListener`, `CadavaPotionListener`, `DrinkBlamishOilListener` | [`everyday`] |
//! | `SpadeDigOptionPlugin`, `SpadeDigListener`, `core.game.global.action.DigSpadeHandler` | [`dig`] |
//! | `ItemQuestRequirementListener`, `MorphItemListener`, `ToyListeners`, `ToyHorseListener` | [`toys`] |
//! | `withobject/WaterSourceListener`, `SandSourceListener`, `ChestKeyListener`, `HatStand`, `SackOnHay` | [`withobject`] |
//! | `equipment/brawling_gloves` (`BrawlingGloves`, `BrawlingGlovesManager`), `equipment/fistofguthixgloves` (`FOGGlovesListener`, `FOGGlovesManager`) | [`gloves`] |
//! | the tables they read (EmptyItem with the herblore potion products, Lamps, BirdNest, the herbs, the quest equipment, the water vessels) | [`tables`] |
//! | `RandomFunction`'s draws, `getChanceItem`, `ChanceItem.getRandomItem`, Kotlin's `random()` | [`random`] |
//! | `EnchantedGemListener` (and `EnchantedGemDialogue`), `PetRockListener` (and `PetRockDialogue`), the DialogueFile helpers they call (`npc`, `npcl`, `player`, `options`, `end`, `showTopics`) | [`dialogues`] |
//! | `toys/DiangoOptionHandler`, `toys/DiangoReclaimInterface`, `toys/SnowGlobePlugin` (and its `SnowGlobeInterface`) | [`diango`] |
//! | the classes' registrations, in the original's scan order | [`registry`] |
//!
//! Each Kotlin lambda and OptionHandler the original registers is a typed
//! native value ([`registry::Interaction`], [`registry::OptionPlugin`]); its
//! body is a function here over [`Host`]. Pulses and close events are
//! [`Task`] values the world stores and runs through [`registry::run_task`].
//! Every call the original makes outside these bodies is a required [`Host`]
//! method with no default. See docs/global-items2.md.
pub mod dialogues;
pub mod diango;
pub mod dig;
pub mod everyday;
pub mod gloves;
pub mod random;
pub mod registry;
pub mod tables;
pub mod toys;
pub mod withobject;

pub use crate::world::global_handlers::{Kind, LogLevel, Store};
pub use crate::world::global_items::{Anim, Gfx, ItemArg, Priority};

/// What stops a call, as the original's exception classes do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `NullPointerException` (Kotlin's null checks included).
    Null,
    /// `ClassCastException`.
    Cast,
    /// `IndexOutOfBoundsException` (arrays included).
    Index,
    /// `NumberFormatException` (`Integer.parseInt`).
    Number,
    /// `NoSuchElementException` (Kotlin's `random()` of an empty array).
    NoSuchElement,
    /// `IllegalArgumentException`.
    Argument,
    /// `IllegalStateException` (ContentAPI's `animate` with another type).
    State,
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

/// An attribute value as the original stores it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value<N> {
    /// A stored `null`.
    Null,
    Int(i32),
    Long(i64),
    Double(f64),
    Bool(bool),
    Text(String),
    Location([i32; 3]),
    /// A node (`setAttribute("lamp", node)`).
    Node(N),
    /// An `Item[]` (`setAttribute("diango-reclaimables", items)`).
    Items(Vec<Option<N>>),
    Object(Object<N>),
}

/// The objects these classes store in attributes.
#[derive(Clone, Debug, PartialEq)]
pub enum Object<N> {
    /// An OptionHandler object (`setAttribute("caller", this)`), with the
    /// identity of the registered object.
    Handler {
        plugin: registry::OptionPlugin,
        identity: usize,
    },
    /// A `BrawlingGlovesManager` (`setAttribute("bg-manager", instance)`).
    GloveManager(gloves::ManagerRef<N>),
}

/// An item definition as the definitions table holds it (EmptyItem's name
/// scan and HatStand's head-slot scan read these).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub id: i32,
    pub name: Option<String>,
    /// The `equipment_slot` configuration, if the definition has one.
    pub equipment_slot: Option<i32>,
}

/// A player's item containers the bodies call directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Inventory,
    Equipment,
    Bank,
}
impl Container {
    pub fn name(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Equipment => "equipment",
            Self::Bank => "bank",
        }
    }
}

/// `DiaryType` constants these handlers update.
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

/// `FacialExpression` constants these handlers use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expression {
    Neutral,
    Friendly,
    Asking,
    HalfAsking,
    Happy,
    HalfThinking,
}
impl Expression {
    pub fn name(self) -> &'static str {
        match self {
            Self::Neutral => "NEUTRAL",
            Self::Friendly => "FRIENDLY",
            Self::Asking => "ASKING",
            Self::HalfAsking => "HALF_ASKING",
            Self::Happy => "HAPPY",
            Self::HalfThinking => "HALF_THINKING",
        }
    }
}

/// `QueueStrength` of a queued script.
pub use crate::world::global_items::QueueStrength;

/// Who a chat box shows (`interpreter.sendDialogues(entity or id, ...)`).
#[derive(Clone, Debug, PartialEq)]
pub enum Speaker<N> {
    Node(N),
    Id(i32),
}

/// The dialogue files the handlers open, with their state.
#[derive(Clone, Debug, PartialEq)]
pub enum Dialogue<N> {
    /// `EnchantedGemDialogue` (`npc` is set on every handle).
    Gem {
        first_run: bool,
        stage: i32,
        npc: Option<N>,
    },
    /// `PetRockDialogue`.
    PetRock { stage: i32 },
}
impl<N> Dialogue<N> {
    pub fn stage(&self) -> i32 {
        match self {
            Self::Gem { stage, .. } | Self::PetRock { stage } => *stage,
        }
    }
    /// `file.stage = value` (the interpreter sets it for a chosen topic).
    pub fn set_stage(&mut self, value: i32) {
        match self {
            Self::Gem { stage, .. } | Self::PetRock { stage } => *stage = value,
        }
    }
}

/// The body a `runTask` pulse runs, with what the lambda captured.
#[derive(Clone, Debug, PartialEq)]
pub enum RunBody<N> {
    /// ToyListeners' snow globe, after the shake (opens a second runTask).
    SnowGlobeHold { player: N },
    /// ToyListeners' snow globe, the second runTask.
    SnowGlobeInterface { player: N },
    /// ToyListeners' spinning plate.
    SpinningPlate { player: N },
    /// SandSourceListener's fill, with the shared `animationTrigger`.
    Sand { player: N, used: N, trigger: i32 },
}

/// The deferred bodies the handlers hand the world (pulses and close
/// events), with what each captured. `C` is the content callback type of
/// [`dig`]'s registries.
#[derive(Clone, Debug, PartialEq)]
pub enum Task<N, C> {
    /// LampPlugin's close event for interface 134 (captures the player).
    LampClose { player: N },
    /// DigSpadeHandler's world pulse that runs a dig action.
    Dig { player: N, action: C },
    /// ContentAPI `runTask`'s pulse: `task(); return --cycles <= 0`.
    RunTask { cycles: i32, body: RunBody<N> },
    /// PetRockDialogue's queued script after stroking the rock.
    PetRockStroke { player: N },
    /// SnowGlobeInterface's pulse after `continue`.
    SnowGlobe { player: N },
    /// WaterSourceListener's fill pulse.
    WaterFill {
        player: N,
        used: N,
        with: N,
        vessel: tables::WaterVessel,
    },
}

/// The services the handlers call, in the original's terms. A method that
/// returns no `Result` is a field read of the original.
pub trait Host {
    /// A node (player, NPC, scenery, item); `==` is identity.
    type Node: Clone + PartialEq + std::fmt::Debug;
    /// A content callback registered with SpadeDigListener or
    /// DigSpadeHandler (a Kotlin `(Player) -> Unit` or a `DigAction`).
    type Callback: Clone + PartialEq + std::fmt::Debug;

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
    /// `item.getCharge()`.
    fn charge(&mut self, item: &Self::Node) -> Result<i32>;
    /// `item.setCharge(charge)`.
    fn set_charge(&mut self, item: &Self::Node, charge: i32) -> Result<()>;
    /// `node.getLocation()`.
    fn location(&mut self, node: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `String.toLowerCase()` / `lowercase(Locale.getDefault())`.
    fn lowercase_default(&self, text: &str) -> String;
    /// Kotlin's `lowercase()` (`Locale.ROOT`).
    fn lowercase_root(&self, text: &str) -> String;
    /// `ItemDefinition.forId(id).getName()` as a table's constant item read
    /// it when its class initialised.
    fn item_name(&mut self, id: i32) -> Option<String>;
    /// `ItemDefinition.getDefinitions().values`, in that HashMap's order.
    fn definitions(&mut self) -> Vec<Definition>;

    // ---------------------------------------------------------- objects
    /// `new Item(id, amount)` (the definition lookup).
    fn new_item(&mut self, id: i32, amount: i32) -> Result<Self::Node>;
    /// A table's constant `Item(id, amount)`, built when its class
    /// initialised (no call is made now).
    fn constant_item(&mut self, id: i32, amount: i32) -> Self::Node;
    /// The generators the originals draw from.
    fn random(&mut self) -> &mut dyn random::Random;
    /// The statics of the classes ported here (casket table, dig registries).
    fn statics(&mut self) -> &mut Statics<Self::Callback>;

    // ---------------------------------------------------------- attributes
    /// `entity.getAttribute(key[, default])` with GameAttributes' "/save:"
    /// rule: the stored value, if any (a stored null reads as none).
    fn get_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Node>>>;
    /// `entity.setAttribute(key, value)`.
    fn set_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
        value: Value<Self::Node>,
    ) -> Result<()>;
    /// `entity.removeAttribute(key)`.
    fn remove_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<()>;

    // ---------------------------------------------------------- messages, dialogue, interfaces
    /// `sendMessage`, `player.sendMessage`, `packetDispatch.sendMessage`.
    fn send_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `player.debug(text)`.
    fn debug(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `dialogueInterpreter.sendItemMessage(item, messages...)`.
    fn item_message(
        &mut self,
        player: &Self::Node,
        item: &Self::Node,
        lines: &[&str],
    ) -> Result<()>;
    /// ContentAPI `sendItemDialogue(player, item, message)`.
    fn item_dialogue(
        &mut self,
        player: &Self::Node,
        item: ItemArg<Self::Node>,
        text: &str,
    ) -> Result<()>;
    /// ContentAPI `sendPlayerDialogue(player, msg, expr, hide)`.
    fn player_dialogue(
        &mut self,
        player: &Self::Node,
        text: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()>;
    /// ContentAPI `sendDialogueLines(player, lines...)`.
    fn dialogue_lines(&mut self, player: &Self::Node, lines: &[&str]) -> Result<()>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn send_dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// ContentAPI `openInterface(player, id)`.
    fn open_interface(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// ContentAPI `setInterfaceText(player, text, iface, child)`.
    fn interface_text(
        &mut self,
        player: &Self::Node,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()>;
    /// ContentAPI `closeAllInterfaces(player)`.
    fn close_all_interfaces(&mut self, player: &Self::Node) -> Result<()>;
    /// `interfaceManager.open(Component(id).setCloseEvent(event))`.
    fn open_component(
        &mut self,
        player: &Self::Node,
        id: i32,
        close: Option<Task<Self::Node, Self::Callback>>,
    ) -> Result<()>;
    /// `interfaceManager.openDefaultTabs()`.
    fn open_default_tabs(&mut self, player: &Self::Node) -> Result<()>;
    /// `interfaceManager.removeTabs(tabs...)`.
    fn remove_tabs(&mut self, player: &Self::Node, tabs: &[i32]) -> Result<()>;

    // ---------------------------------------------------------- requirements
    /// ContentAPI `hasRequirement(player, quest, message)` (`Quests` name).
    fn has_requirement(&mut self, player: &Self::Node, quest: &str, message: bool) -> Result<bool>;
    /// `QuestRepository.getQuests().values`' quest points, in that order.
    fn quests(&mut self) -> Result<Vec<i32>>;
    /// `player.questRepository.points`.
    fn quest_points(&mut self, player: &Self::Node) -> Result<i32>;

    // ---------------------------------------------------------- locks, pulses, movement
    /// `entity.lock(ticks)`.
    fn lock(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// `entity.unlock()`.
    fn unlock(&mut self, entity: &Self::Node) -> Result<()>;
    /// ContentAPI `lockInteractions(entity, duration)`.
    fn lock_interactions(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// ContentAPI `stopWalk(entity)`.
    fn stop_walk(&mut self, entity: &Self::Node) -> Result<()>;
    /// `entity.pulseManager.run(pulse)` (the pulse's delay is `delay`).
    fn pulse_run(
        &mut self,
        entity: &Self::Node,
        pulse: Task<Self::Node, Self::Callback>,
        delay: i32,
    ) -> Result<()>;
    /// `GameWorld.getPulser().submit(pulse)`.
    fn submit_world(&mut self, pulse: Task<Self::Node, Self::Callback>, delay: i32) -> Result<()>;

    // ---------------------------------------------------------- visuals and audio
    /// `animator.animate(animation)`.
    fn animate(&mut self, entity: &Self::Node, animation: Anim) -> Result<bool>;
    /// `animator.animate(animation, graphics)` (`entity.visualize`).
    fn visualize(&mut self, entity: &Self::Node, animation: Anim, graphics: Gfx) -> Result<bool>;
    /// `entity.sendChat(message)`.
    fn chat(&mut self, entity: &Self::Node, text: &str) -> Result<()>;
    /// `player.appearance.transformNPC(id)`.
    fn transform_npc(&mut self, player: &Self::Node, id: i32) -> Result<()>;
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
    /// ContentAPI `playJingle(player, id)`.
    fn play_jingle(&mut self, player: &Self::Node, id: i32) -> Result<()>;

    // ---------------------------------------------------------- containers
    /// `container.add(item)`.
    fn container_add(
        &mut self,
        player: &Self::Node,
        c: Container,
        item: &Self::Node,
    ) -> Result<bool>;
    /// `container.add(items...)`.
    fn container_add_all(
        &mut self,
        player: &Self::Node,
        c: Container,
        items: &[Option<Self::Node>],
    ) -> Result<bool>;
    /// `container.remove(item)`.
    fn container_remove(
        &mut self,
        player: &Self::Node,
        c: Container,
        item: Option<&Self::Node>,
    ) -> Result<bool>;
    /// `container.remove(item, slot, fireListener)`.
    fn container_remove_slot(
        &mut self,
        player: &Self::Node,
        c: Container,
        item: &Self::Node,
        slot: i32,
        fire: bool,
    ) -> Result<bool>;
    /// `container.freeSlots()`.
    fn container_free_slots(&mut self, player: &Self::Node, c: Container) -> Result<i32>;
    /// `container.replace(item, slot)`: the item that was there.
    fn container_replace(
        &mut self,
        player: &Self::Node,
        c: Container,
        item: &Self::Node,
        slot: i32,
    ) -> Result<Option<Self::Node>>;
    /// `container.get(slot)`.
    fn container_get(
        &mut self,
        player: &Self::Node,
        c: Container,
        slot: i32,
    ) -> Result<Option<Self::Node>>;
    /// `container.toArray()`.
    fn container_to_array(
        &mut self,
        player: &Self::Node,
        c: Container,
    ) -> Result<Vec<Option<Self::Node>>>;
    /// ContentAPI `removeItem(player, item, container)`.
    fn remove_item(
        &mut self,
        player: &Self::Node,
        item: ItemArg<Self::Node>,
        store: Store,
    ) -> Result<bool>;
    /// ContentAPI `replaceSlot(player, slot, item, currentItem, container)`.
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
    /// ContentAPI `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, player: &Self::Node, id: i32) -> Result<i32>;
    /// ContentAPI `produceGroundItem(player, id)`.
    fn produce_ground_item(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `GroundItemManager.create(item, player)`.
    fn ground_item(&mut self, player: &Self::Node, item: &Self::Node) -> Result<()>;
    /// ContentAPI `replaceScenery(scenery, with, forTicks, location)`.
    fn replace_scenery(
        &mut self,
        scenery: &Self::Node,
        with: i32,
        ticks: i32,
        location: Option<[i32; 3]>,
    ) -> Result<()>;

    // ---------------------------------------------------------- player state
    /// `achievementDiaryManager.getDiary(type)`.
    fn get_diary(&mut self, player: &Self::Node, diary: Diary) -> Result<()>;
    /// The diary's `isComplete(level, task)`.
    fn diary_complete(
        &mut self,
        player: &Self::Node,
        diary: Diary,
        level: i32,
        task: i32,
    ) -> Result<bool>;
    /// The diary's `updateTask(player, level, task, complete)`.
    fn diary_update(
        &mut self,
        player: &Self::Node,
        diary: Diary,
        level: i32,
        task: i32,
        complete: bool,
    ) -> Result<()>;
    /// `skills.getStaticLevel(skill)`.
    fn static_level(&mut self, player: &Self::Node, skill: i32) -> Result<i32>;
    /// `skills.addExperience(skill, experience)`.
    fn add_experience(&mut self, player: &Self::Node, skill: i32, experience: f64) -> Result<()>;
    /// `skullManager.getLevel()` (the wilderness level).
    fn wilderness_level(&mut self, player: &Self::Node) -> Result<i32>;

    // ---------------------------------------------------------- logs and content callbacks
    /// ContentAPI `log(origin, type, message)`.
    fn log(&mut self, class: &str, level: LogLevel, message: &str) -> Result<()>;
    /// `exception.printStackTrace()` of a caught exception (its class).
    fn stack_trace(&mut self, exception: &str);
    /// Runs a SpadeDigListener callback (`method(player)`).
    fn run_spade_listener(&mut self, player: &Self::Node, callback: &Self::Callback) -> Result<()>;
    /// Runs a DigSpadeHandler action (`action.run(player)`).
    fn run_dig_action(&mut self, player: &Self::Node, action: &Self::Callback) -> Result<()>;

    // ---------------------------------------------------------- dialogue files (EnchantedGemDialogue, PetRockDialogue)
    /// `player.username`.
    fn username(&mut self, player: &Self::Node) -> Result<String>;
    /// ContentAPI `openDialogue(player, file)`: the world loads the file and
    /// runs its `handle` through [`registry::run_dialogue`].
    fn open_dialogue(&mut self, player: &Self::Node, dialogue: Dialogue<Self::Node>) -> Result<()>;
    /// `interpreter.sendDialogues(entity or id, expression, messages...)`.
    fn send_dialogues(
        &mut self,
        player: &Self::Node,
        speaker: Speaker<Self::Node>,
        expression: Option<Expression>,
        lines: &[String],
    ) -> Result<()>;
    /// `interpreter.activeTopics.add(topic)`: the expression, text and stage
    /// the interpreter moves the file to when the topic is chosen.
    fn add_topic(
        &mut self,
        player: &Self::Node,
        expression: Expression,
        text: &str,
        stage: i32,
    ) -> Result<()>;
    /// `interpreter.activeTopics.clear()`.
    fn clear_topics(&mut self, player: &Self::Node) -> Result<()>;
    /// `interpreter.sendOptions(title, options...)`.
    fn send_options(&mut self, player: &Self::Node, title: &str, options: &[&str]) -> Result<()>;
    /// `interpreter.close()`.
    fn interpreter_close(&mut self, player: &Self::Node) -> Result<bool>;
    /// ContentAPI `getSlayerMaster(player)` (`None` fails its casts).
    fn slayer_master(&mut self, player: &Self::Node) -> Result<Option<Self::Node>>;
    /// ContentAPI `getSlayerTip(player)`.
    fn slayer_tip(&mut self, player: &Self::Node) -> Result<Vec<String>>;
    /// ContentAPI `hasSlayerTask(player)`.
    fn has_slayer_task(&mut self, player: &Self::Node) -> Result<bool>;
    /// ContentAPI `getSlayerTaskName(player)`.
    fn slayer_task_name(&mut self, player: &Self::Node) -> Result<String>;
    /// ContentAPI `getSlayerTaskKillsRemaining(player)`.
    fn slayer_kills_remaining(&mut self, player: &Self::Node) -> Result<i32>;
    /// ContentAPI `getSlayerTaskFlags(player)`.
    fn slayer_task_flags(&mut self, player: &Self::Node) -> Result<i32>;
    /// ContentAPI `setVarp(player, index, value, save)`.
    fn set_varp(&mut self, player: &Self::Node, index: i32, value: i32, save: bool) -> Result<()>;
    /// ContentAPI `queueScript(entity, delay, strength, persist, script)`.
    fn queue_script(
        &mut self,
        entity: &Self::Node,
        delay: i32,
        strength: QueueStrength,
        persist: bool,
        script: Task<Self::Node, Self::Callback>,
    ) -> Result<()>;
    /// ContentAPI `stopExecuting(entity)`.
    fn stop_executing(&mut self, entity: &Self::Node) -> Result<bool>;
    /// ContentAPI `animationDuration(Animation(id))`.
    fn animation_duration(&mut self, id: i32) -> Result<i32>;
    /// `Projectile.getLocation(entity)`.
    fn projectile_location(&mut self, entity: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `Location.getRandomLocation(main, radius, reachable)`.
    fn random_location(
        &mut self,
        main: [i32; 3],
        radius: i32,
        reachable: bool,
    ) -> Result<Option<[i32; 3]>>;
    /// `entity.faceLocation(location)`.
    fn face_location(&mut self, entity: &Self::Node, location: [i32; 3]) -> Result<()>;
    /// ContentAPI `spawnProjectile(source, dest, projectile, startHeight,
    /// endHeight, delay, speed, angle)`.
    #[allow(clippy::too_many_arguments)]
    fn spawn_projectile(
        &mut self,
        source: [i32; 3],
        dest: [i32; 3],
        projectile: i32,
        start_height: i32,
        end_height: i32,
        delay: i32,
        speed: i32,
        angle: i32,
    ) -> Result<()>;

    // ---------------------------------------------------------- Diango's reclaim interface
    /// `interfaceManager.getOpened()`: the open component's id.
    fn opened_component(&mut self, player: &Self::Node) -> Result<Option<i32>>;
    /// `component.close(player)`.
    fn component_close(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `interfaceManager.close()`.
    fn close_interface(&mut self, player: &Self::Node) -> Result<bool>;
    /// `container.containsItem(item)`.
    fn container_contains(
        &mut self,
        player: &Self::Node,
        c: Container,
        item: &Self::Node,
    ) -> Result<bool>;
    /// `POHStorageState.getContainer(family).contains(id)`.
    fn storage_contains(&mut self, player: &Self::Node, family: &str, id: i32) -> Result<bool>;
    /// ContentAPI `inInventory(player, id, amount)`.
    fn in_inventory(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// ContentAPI `inEquipment(player, id, amount)`.
    fn in_equipment(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `InterfaceContainer.generateItems(player, items, options, iface, child, x, y)`.
    #[allow(clippy::too_many_arguments)]
    fn generate_items(
        &mut self,
        player: &Self::Node,
        items: &[Option<Self::Node>],
        options: &[&str],
        iface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<i32>;
    /// `ItemDefinition.forId(id).getExamine()`.
    fn item_examine(&mut self, id: i32) -> Option<String>;
}

/// The static fields of the ported classes that hold state.
#[derive(Clone, Debug, PartialEq)]
pub struct Statics<C> {
    /// `CasketListener.loot` (built when the class initialised).
    pub casket: Option<crate::world::drop_tables::WeightBasedTable>,
    /// `SpadeDigListener.listeners` (a HashMap keyed by location).
    pub spade_listeners: Vec<([i32; 3], C)>,
    /// `DigSpadeHandler.ACTIONS`.
    pub dig_actions: Vec<([i32; 3], C)>,
    /// `DiangoReclaimInterface.ITEMS` (each `newInstance` appends the holiday items).
    pub diango_items: Vec<i32>,
}
impl<C> Default for Statics<C> {
    fn default() -> Self {
        Self {
            casket: None,
            spade_listeners: Vec::new(),
            dig_actions: Vec::new(),
            diango_items: Vec::new(),
        }
    }
}

/// Java `(Item) node` / `node.asItem()`: another class fails the cast.
pub(crate) fn as_item<'a, H: Host + ?Sized>(h: &H, node: &'a H::Node) -> Result<&'a H::Node> {
    if h.kind(node) == Kind::Item {
        Ok(node)
    } else {
        Err(Error::Cast)
    }
}

/// `StringUtils.isPlusN(word)` (`word.toLowerCase()` in the default locale;
/// an empty word fails at `charAt(0)`).
pub fn is_plus_n<H: Host + ?Sized>(h: &H, word: Option<&str>) -> Result<bool> {
    let Some(word) = word else {
        return Ok(false);
    };
    let s: Vec<char> = h.lowercase_default(word).chars().collect();
    let first = *s.first().ok_or(Error::Index)?;
    Ok(
        matches!(first, 'a' | 'e' | 'i' | 'o' | 'u')
            || (first == 'h' && s.len() > 1 && s[1] != 'e'),
    )
}

/// `core.tools.colorize(line)`.
pub fn colorize(line: &str) -> String {
    let named = line
        .replace("%R", "<col=ff0000>")
        .replace("%O", "<col=ff6600>")
        .replace("%Y", "<col=ffff00>")
        .replace("%G", "<col=66ff33>")
        .replace("%B", "<col=3366ff>")
        .replace("%P", "<col=cc00ff>");
    // `.replace(Regex("%[0-9a-fA-F]{6}")) { "<col=${it.value.substring(1)}>" }`
    let chars: Vec<char> = named.chars().collect();
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

/// Java's `Character.toLowerCase(char)` (Kotlin's `Char.toLowerCase()`): the
/// one-to-one simple mapping.
pub fn char_lower(c: char) -> char {
    if c.is_ascii() {
        return c.to_ascii_lowercase();
    }
    if c == '\u{130}' {
        return 'i';
    }
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Kotlin's `CharSequence.contains(other, ignoreCase = true)`.
pub fn contains_ignore_case(text: &str, other: &str) -> bool {
    let a: Vec<char> = text.chars().collect();
    let b: Vec<char> = other.chars().collect();
    if b.is_empty() {
        return true;
    }
    let same = |x: char, y: char| {
        x == y || {
            let (ux, uy) = (char_upper(x), char_upper(y));
            ux == uy || char_lower(ux) == char_lower(uy)
        }
    };
    (0..=a.len().saturating_sub(b.len())).any(|start| {
        start + b.len() <= a.len() && b.iter().enumerate().all(|(k, &c)| same(a[start + k], c))
    })
}
fn char_upper(c: char) -> char {
    if c.is_ascii() {
        return c.to_ascii_uppercase();
    }
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}

/// `Integer.parseInt(text)` (radix 10).
pub fn parse_int(text: &str) -> Result<i32> {
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Number);
    }
    let mut value: i64 = 0;
    for b in digits.bytes() {
        value = value * 10 + i64::from(b - b'0');
        if value > i64::from(i32::MAX) + 1 {
            return Err(Error::Number);
        }
    }
    let value = if negative { -value } else { value };
    i32::try_from(value).map_err(|_| Error::Number)
}

// ---------------------------------------------------------------------- ContentAPI
//
// The ContentAPI functions whose bodies the oracle compiles verbatim; every
// other ContentAPI function is a Host service.

/// ContentAPI `visualize(entity, anim, gfx)` with an animation and a
/// graphics id or object.
pub fn visualize<H: Host + ?Sized>(
    h: &mut H,
    entity: &H::Node,
    animation: Anim,
    graphics: Gfx,
) -> Result<()> {
    h.visualize(entity, animation, graphics)?;
    Ok(())
}

/// ContentAPI `animate(entity, anim)`: an Int or an Animation; anything else
/// (a missing map entry) is `IllegalStateException`.
pub fn animate<H: Host + ?Sized>(
    h: &mut H,
    entity: &H::Node,
    animation: Option<Anim>,
) -> Result<()> {
    let animation = animation.ok_or(Error::State)?;
    h.animate(entity, animation)?;
    Ok(())
}

/// ContentAPI `runTask(entity, delay, repeatTimes, task)`: a pulse of
/// `delay` whose body runs the task and finishes when `--cycles <= 0`.
pub fn run_task<H: Host + ?Sized>(
    h: &mut H,
    entity: &H::Node,
    delay: i32,
    repeat: i32,
    body: RunBody<H::Node>,
) -> Result<()> {
    h.pulse_run(
        entity,
        Task::RunTask {
            cycles: repeat,
            body,
        },
        delay,
    )
}

/// ContentAPI `getUsedOption(player)`: the `interact:option` attribute
/// (`"INVALID"` without one); another type fails Kotlin's String cast.
pub fn used_option<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<String> {
    match h.get_attribute(player, "interact:option")? {
        None | Some(Value::Null) => Ok("INVALID".into()),
        Some(Value::Text(t)) => Ok(t),
        Some(_) => Err(Error::Cast),
    }
}

/// ContentAPI `sendChat(entity, message)` (no delay: `entity.sendChat`).
pub fn send_chat<H: Host + ?Sized>(h: &mut H, entity: &H::Node, text: &str) -> Result<()> {
    h.chat(entity, text)
}

/// ContentAPI `getItemFromEquipment(player, slot)`: `equipment.get(slot.ordinal)`.
pub fn item_from_equipment<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    slot: i32,
) -> Result<Option<H::Node>> {
    h.container_get(player, Container::Equipment, slot)
}
