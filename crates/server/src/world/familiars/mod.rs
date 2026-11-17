//! The summoning familiar framework of
//! content/global/skill/summoning: `FamiliarManager` ([`manager`]), the
//! base NPC `Familiar` ([`familiar`]), `BurdenBeast` with
//! `BurdenContainerListener` and `Forager` ([`burden`]), `FamiliarSpecial`
//! ([`special`]), and the pets: `Pet` ([`pet`]), `Pets` and `PetDetails`
//! ([`pets`]; the details are the persisted objects of
//! `player::persisted::familiars`).
//!
//! A familiar is an NPC subclass object: [`Familiar`] keeps the original
//! class's fields natively and its NPC (made by `new NPC(id, null)`, the
//! base constructor, never `AbstractNPC`) is a host [`Host::Node`]. The
//! individual familiar classes, summoning pouches and scrolls, the
//! summoning skill, the NPC/entity base members, containers, interfaces and
//! everything else outside these classes are required [`Host`] services:
//! none has a default and no lock is held across one. Getters the original
//! reads as plain fields (location, ids, name, visibility, kind, the
//! player's inventory and save version, the familiar manager and the
//! `FAMILIARS` registry) are the unrecorded accessors at the top of the
//! trait; every other method is one original call. See docs/familiars.md.
pub mod burden;
pub mod familiar;
pub mod manager;
pub mod pet;
pub mod pets;
pub mod special;

pub use crate::player::persisted::familiars::{Details, PetDetails, PetStore};
pub use crate::world::combat_swing::CombatStyle;
pub use familiar::{Familiar, Kind, Shape, Spec};
pub use manager::{Manager, Registry};
pub use special::FamiliarSpecial;

use rs09_game_core::java_json::Json;

/// What the original throws, by class. `Binding` is a host service failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// NullPointerException.
    Null,
    /// ClassCastException.
    Cast,
    /// NumberFormatException.
    Number,
    /// ArrayIndexOutOfBoundsException / IndexOutOfBoundsException.
    Index,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// A tile: x, y, plane.
pub type Loc = [i32; 3];

/// `Skills.SUMMONING`, `Skills.SLAYER`.
pub const SUMMONING: i32 = 23;
pub const SLAYER: i32 = 18;
/// `WeaponInterface.STYLE_DEFENSIVE`.
pub const STYLE_DEFENSIVE: i32 = 3;
/// `Familiar.INFINITE_SPECIAL_MOVE_ATTRIBUTE`.
pub const INFINITE_SPECIAL_MOVE_ATTRIBUTE: &str = "infinite-special-move";

/// An `Item` value: id, amount and slot (`index`, -1 for a new item).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: i32,
    pub amount: i32,
    pub slot: i32,
}
impl Item {
    /// `new Item(id, amount)`.
    pub fn new(id: i32, amount: i32) -> Self {
        Self {
            id,
            amount,
            slot: -1,
        }
    }
    /// `new Item(id)`.
    pub fn one(id: i32) -> Self {
        Self::new(id, 1)
    }
}

/// The `SummoningPouch` constant `SummoningPouch.get` answers: the members
/// the framework reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pouch {
    pub level_required: i32,
    pub summon_cost: i32,
    pub npc_id: i32,
    pub summon_experience: f64,
    pub peaceful: bool,
    pub abyssal: bool,
}

/// The `SummoningScroll` constant of a pouch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scroll {
    pub item_id: i32,
    pub experience: f64,
}

/// `SkillBonus`: a skill id and its bonus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillBonus {
    pub skill: i32,
    pub bonus: f64,
}

/// What an `instanceof` test sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Player,
    Npc,
    Other,
}

/// `Log` levels the framework logs at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Err,
}

/// The pulses the framework hands to a pulse manager or the world pulser;
/// the host keeps them and runs them with [`Pulse::pulse`].
pub enum Pulse<H: Host> {
    /// `startFollowing`'s anonymous `MovementPulse` (its `pulse()` answers
    /// false: the movement itself is the host's MovementPulse).
    Follow,
    /// `sendFamiliarHit`'s anonymous `Pulse`.
    Hit(familiar::HitPulse<H>),
}
impl<H: Host> Pulse<H> {
    /// `pulse()`.
    pub fn pulse(&self, host: &mut H) -> Result<bool> {
        match self {
            Self::Follow => Ok(false),
            Self::Hit(hit) => hit.pulse(host),
        }
    }
}

/// The origin class `ContentAPI.log` is handed.
pub enum Origin<'a, H: Host> {
    /// `Pets` (`getNpcId`).
    Pets,
    /// The class of a familiar object (`Familiar.newInstance`).
    Familiar(&'a Familiar<H>),
}

/// Every service the framework reaches. One `Node` type stands for players,
/// NPCs (a familiar's own NPC included) and any other entity.
pub trait Host: Sized {
    type Node: Clone + PartialEq + std::fmt::Debug;
    type Container: Clone + PartialEq + std::fmt::Debug;
    /// A `CombatSwingHandler` object.
    type Handler: Clone + std::fmt::Debug;
    /// A `BattleState` object.
    type State;
    /// The content class of a familiar object (which subclass `construct`,
    /// `specialMove`, `getIds` and the other content members belong to).
    type Class: Clone + std::fmt::Debug;

    // ---- plain fields and getters (not calls the original makes)
    /// `Node.getLocation()` / the `location` field.
    fn location(&mut self, node: &Self::Node) -> Result<Option<Loc>>;
    /// `location = loc` on an NPC.
    fn set_location(&mut self, npc: &Self::Node, loc: Option<Loc>) -> Result<()>;
    /// `NPC.getId()`.
    fn npc_id(&mut self, npc: &Self::Node) -> Result<i32>;
    /// `NPC.getOriginalId()`.
    fn original_id(&mut self, npc: &Self::Node) -> Result<i32>;
    /// `Entity.getName()`.
    fn name(&mut self, node: &Self::Node) -> Result<Option<String>>;
    /// `Node.isInvisible()`.
    fn is_invisible(&mut self, node: &Self::Node) -> Result<bool>;
    /// `instanceof Player` / `instanceof NPC`.
    fn kind(&mut self, node: &Self::Node) -> Result<NodeKind>;
    /// `Player.getInventory()`.
    fn inventory(&mut self, player: &Self::Node) -> Result<Self::Container>;
    /// `Player.version`.
    fn version(&mut self, player: &Self::Node) -> Result<i32>;
    /// `Player.getFamiliarManager()`.
    fn manager(&mut self, player: &Self::Node) -> Result<Manager<Self>>;
    /// `FamiliarManager.FAMILIARS` (one per world).
    fn registry(&mut self) -> Result<Registry<Self>>;
    /// The class value of `Pet` objects.
    fn pet_class(&mut self) -> Self::Class;

    // ---- NPC and entity members
    /// `new NPC(id, null)`, the base constructor (`super(id, null)`): the
    /// live world answers with `npc_classes::Factory::new_npc(None, id,
    /// None, Some(Direction::South))`.
    fn new_npc(&mut self, id: i32) -> Result<Self::Node>;
    /// `NPCDefinition.forId(id).getName()`.
    fn npc_definition_name(&mut self, id: i32) -> Result<Option<String>>;
    fn set_invisible(&mut self, node: &Self::Node, invisible: bool) -> Result<()>;
    /// `NPC.init()` (`super.init()`).
    fn npc_init(&mut self, npc: &Self::Node) -> Result<()>;
    /// `NPC.clear()`.
    fn npc_clear(&mut self, npc: &Self::Node) -> Result<()>;
    /// `NPC.transform(id)`.
    fn npc_transform(&mut self, npc: &Self::Node, id: i32) -> Result<()>;
    /// `getPulseManager().run(pulse, PulseType.STANDARD)`; the movement
    /// pulse's mover and destination are given.
    fn pulse_run(
        &mut self,
        node: &Self::Node,
        pulse: Pulse<Self>,
        mover: &Self::Node,
        destination: Option<&Self::Node>,
    ) -> Result<()>;
    /// `getPulseManager().clear()`.
    fn pulse_clear(&mut self, node: &Self::Node) -> Result<()>;
    /// `getPulseManager().hasPulseRunning()`.
    fn has_pulse_running(&mut self, node: &Self::Node) -> Result<bool>;
    /// `GameWorld.getPulser().submit(pulse)`; `delay` and `nodes` are the
    /// pulse's constructor arguments.
    fn submit_pulse(&mut self, delay: i32, nodes: &[Self::Node], pulse: Pulse<Self>) -> Result<()>;
    /// `Entity.face(node)`.
    fn face(&mut self, node: &Self::Node, target: Option<&Self::Node>) -> Result<()>;
    /// `Entity.sendChat(text)` (the NPC's own, `super.sendChat`).
    fn send_chat(&mut self, node: &Self::Node, text: &str) -> Result<()>;
    /// `Entity.graphics(Graphics.create(id))`.
    fn graphics(&mut self, node: &Self::Node, id: i32) -> Result<()>;
    /// `Node.size()`.
    fn size(&mut self, node: &Self::Node) -> Result<i32>;
    /// `Entity.isRenderable()`.
    fn is_renderable(&mut self, node: &Self::Node) -> Result<bool>;
    /// `getWalkingQueue().update()`.
    fn walking_update(&mut self, node: &Self::Node) -> Result<()>;
    /// `getUpdateMasks().prepare(node)`.
    fn masks_prepare(&mut self, node: &Self::Node) -> Result<()>;
    /// `getProperties().setTeleportLocation(loc)`.
    fn set_teleport_location(&mut self, node: &Self::Node, loc: Loc) -> Result<()>;
    /// `getZoneMonitor().isInZone(name)`.
    fn in_zone(&mut self, node: &Self::Node, name: &str) -> Result<bool>;
    /// `getZoneMonitor().isRestricted(ZoneRestriction.FOLLOWERS)`.
    fn restricted_followers(&mut self, node: &Self::Node) -> Result<bool>;
    /// `getLocks().isLocked(key)`.
    fn is_locked(&mut self, node: &Self::Node, key: &str) -> Result<bool>;
    /// `getLocks().isInteractionLocked()`.
    fn interaction_locked(&mut self, node: &Self::Node) -> Result<bool>;
    /// `getAttribute(key, default)` read as an int.
    fn attribute_int(&mut self, node: &Self::Node, key: &str, default: i32) -> Result<i32>;
    /// `getAttribute(key, default)` read as a boolean.
    fn attribute_bool(&mut self, node: &Self::Node, key: &str, default: bool) -> Result<bool>;
    /// `getAttribute(key)` read as an entity.
    fn attribute_node(&mut self, node: &Self::Node, key: &str) -> Result<Option<Self::Node>>;
    /// `setAttribute(key, value)` with an int.
    fn set_attribute_int(&mut self, node: &Self::Node, key: &str, value: i32) -> Result<()>;
    /// `Player.incrementAttribute(key)`.
    fn increment_attribute(&mut self, node: &Self::Node, key: &str) -> Result<()>;
    /// `node.isAttackable(attacker, style, message)`, dispatched on the
    /// node's class (a familiar's is [`Familiar::is_attackable`]).
    fn is_attackable(
        &mut self,
        node: &Self::Node,
        attacker: &Self::Node,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool>;
    /// `NPC.isAttackable` (`super.isAttackable` of a familiar).
    fn npc_is_attackable(
        &mut self,
        npc: &Self::Node,
        attacker: &Self::Node,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool>;
    /// `NPC.getSwingHandler(swing)` (`super.getSwingHandler`).
    fn npc_swing_handler(&mut self, npc: &Self::Node, swing: bool) -> Result<Self::Handler>;
    /// `getProperties().getCombatPulse().isAttacking()`.
    fn combat_attacking(&mut self, node: &Self::Node) -> Result<bool>;
    /// `getProperties().getCombatPulse().getVictim()`.
    fn combat_victim(&mut self, node: &Self::Node) -> Result<Option<Self::Node>>;
    /// `getProperties().getCombatPulse().attack(victim)`.
    fn combat_attack(&mut self, node: &Self::Node, victim: &Self::Node) -> Result<()>;
    /// `getProperties().getCombatPulse().setNextAttack(ticks)`.
    fn combat_next_attack(&mut self, node: &Self::Node, ticks: i32) -> Result<()>;
    /// `getProperties().getCombatPulse().getStyle()`.
    fn combat_style(&mut self, node: &Self::Node) -> Result<CombatStyle>;
    /// `Entity.inCombat()`.
    fn in_combat(&mut self, node: &Self::Node) -> Result<bool>;
    /// `getProperties().isMultiZone()`.
    fn multi_zone(&mut self, node: &Self::Node) -> Result<bool>;
    /// `Player.getSkullManager().isWilderness()`.
    fn skull_wilderness(&mut self, player: &Self::Node) -> Result<bool>;
    /// `((NPC) node).getTask()`: the task's `levelReq`, or none.
    fn npc_task(&mut self, npc: &Self::Node) -> Result<Option<i32>>;
    /// `Player.isActive()`.
    fn is_active(&mut self, player: &Self::Node) -> Result<bool>;
    /// `RegionManager.getSpawnLocation(owner, node)`.
    fn spawn_location(&mut self, owner: &Self::Node, node: &Self::Node) -> Result<Option<Loc>>;
    /// `NPC.getSkills().getLifepoints()` (what the save writes).
    fn lifepoints(&mut self, node: &Self::Node) -> Result<i32>;

    // ---- the player's skills, messages, varps and interfaces
    fn skill_level(&mut self, player: &Self::Node, skill: i32) -> Result<i32>;
    fn static_level(&mut self, player: &Self::Node, skill: i32) -> Result<i32>;
    /// `getSkills().updateLevel(skill, amount, maximum)`.
    fn update_level(
        &mut self,
        player: &Self::Node,
        skill: i32,
        amount: i32,
        maximum: i32,
    ) -> Result<()>;
    /// `getSkills().addExperience(skill, experience[, playerMod])`.
    fn add_experience(
        &mut self,
        player: &Self::Node,
        skill: i32,
        experience: f64,
        player_mod: Option<bool>,
    ) -> Result<()>;
    /// `getPacketDispatch().sendMessage`.
    fn packet_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `Player.sendMessage`.
    fn player_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `ContentAPI.sendMessage(player, text)`.
    fn api_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `getDialogueInterpreter().sendDialogue(text)`.
    fn send_dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `ContentAPI.setVarp(player, varp, value)`.
    fn set_varp(&mut self, player: &Self::Node, varp: i32, value: i32) -> Result<()>;
    /// `ContentAPI.setVarbit(player, varbit, value)`.
    fn set_varbit(&mut self, player: &Self::Node, varbit: i32, value: i32) -> Result<()>;
    /// `ContentAPI.playAudio(player, sound)`.
    fn play_audio(&mut self, player: &Self::Node, sound: i32) -> Result<()>;
    /// `Player.visualize(Animation.create(anim), Graphics.create(gfx))`.
    fn visualize(&mut self, player: &Self::Node, anim: i32, gfx: i32) -> Result<()>;
    /// `Entity.animate(new Animation(id))`.
    fn animate(&mut self, player: &Self::Node, anim: i32) -> Result<()>;
    /// `getAppearance().sync()`.
    fn appearance_sync(&mut self, player: &Self::Node) -> Result<()>;
    /// `getQuestRepository().isComplete(Quests.WOLF_WHISTLE)`.
    fn wolf_whistle_complete(&mut self, player: &Self::Node) -> Result<bool>;
    /// `SkillcapePerks.isActive(SkillcapePerks.PET_MASTERY, player)`.
    fn pet_mastery(&mut self, player: &Self::Node) -> Result<bool>;
    /// `getInterfaceManager().openTab(new Component(id))`.
    fn open_tab(&mut self, player: &Self::Node, component: i32) -> Result<()>;
    /// `getInterfaceManager().setViewedTab(tab)`.
    fn set_viewed_tab(&mut self, player: &Self::Node, tab: i32) -> Result<()>;
    /// `getInterfaceManager().openInfoBars()`.
    fn open_info_bars(&mut self, player: &Self::Node) -> Result<()>;
    /// `getInterfaceManager().removeTabs(tabs...)`.
    fn remove_tabs(&mut self, player: &Self::Node, tabs: &[i32]) -> Result<()>;
    /// `getInterfaceManager().hasMainComponent(id)`.
    fn has_main_component(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// `getInterfaceManager().close()`.
    fn close(&mut self, player: &Self::Node) -> Result<()>;
    /// `getInterfaceManager().open(new Component(id))`: false when it
    /// answers null.
    fn open(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// `setCloseEvent` on the component `open` answered; the event is
    /// [`burden::close_event`].
    fn set_close_event(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `getInterfaceManager().openSingleTab(new Component(id))`.
    fn open_single_tab(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `getInterfaceManager().closeSingleTab()`.
    fn close_single_tab(&mut self, player: &Self::Node) -> Result<()>;
    /// `InterfaceContainer.generateItems(player, items, options, interfaceId,
    /// childId, x, y, key)`.
    #[allow(clippy::too_many_arguments)]
    fn generate_items(
        &mut self,
        player: &Self::Node,
        items: &[Option<Item>],
        options: &[&str],
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
        key: i32,
    ) -> Result<()>;
    /// `PacketRepository.send(ContainerPacket.class, new
    /// ContainerContext(player, -1, -2, 30, items, ...))`: `slots` is the
    /// event's slots (an update) and `capacity` the length (a refresh).
    fn container_packet(
        &mut self,
        player: &Self::Node,
        items: &[Option<Item>],
        capacity: Option<i32>,
        slots: Option<&[i32]>,
    ) -> Result<()>;

    // ---- combat services of the hit pulse and specials
    /// `new BattleState(attacker, victim)`.
    fn battle_state(&mut self, attacker: &Self::Node, victim: &Self::Node) -> Result<Self::State>;
    /// `style.getSwingHandler().isAccurateImpact(attacker, victim)`.
    fn accurate_impact(
        &mut self,
        style: CombatStyle,
        attacker: &Self::Node,
        victim: &Self::Node,
    ) -> Result<bool>;
    fn set_estimated_hit(&mut self, state: &mut Self::State, hit: i32) -> Result<()>;
    fn neutralize_hits(&mut self, state: &mut Self::State) -> Result<()>;
    fn set_poison_damage(&mut self, state: &mut Self::State, damage: i32) -> Result<()>;
    /// `state.getStyle()`.
    fn state_style(&mut self, state: &mut Self::State) -> Result<Option<CombatStyle>>;
    /// `target.getImpactHandler().handleImpact(source, hit, style, state)`.
    fn handle_impact(
        &mut self,
        target: &Self::Node,
        source: &Self::Node,
        hit: i32,
        style: CombatStyle,
        state: &mut Self::State,
    ) -> Result<()>;
    /// `Projectile.magic(source, target, id, start, end, delay, speed).send()`.
    #[allow(clippy::too_many_arguments)]
    fn projectile(
        &mut self,
        source: &Self::Node,
        target: &Self::Node,
        id: i32,
        start_height: i32,
        end_height: i32,
        delay: i32,
        speed: i32,
    ) -> Result<()>;
    /// `GameWorld.getTicks()`.
    fn ticks(&mut self) -> Result<i32>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32>;
    /// `RandomFunction.random(a, b)`.
    fn random_range(&mut self, a: i32, b: i32) -> Result<i32>;
    /// `RandomFunction.randomize(value)`.
    fn randomize(&mut self, value: i32) -> Result<i32>;

    // ---- containers and items
    /// `new Container(size)`.
    fn new_container(&mut self, size: i32) -> Result<Self::Container>;
    /// `container.register(new BurdenContainerListener(owner))`.
    fn register_burden_listener(
        &mut self,
        container: &Self::Container,
        owner: Option<&Self::Node>,
    ) -> Result<()>;
    fn get_amount(&mut self, c: &Self::Container, item: &Item) -> Result<i32>;
    fn maximum_add(&mut self, c: &Self::Container, item: &Item) -> Result<i32>;
    /// `replace(item, slot[, fire])`: the item that was there.
    fn replace(
        &mut self,
        c: &Self::Container,
        item: Option<Item>,
        slot: i32,
        fire: Option<bool>,
    ) -> Result<Option<Item>>;
    fn add(&mut self, c: &Self::Container, item: &Item, fire: Option<bool>) -> Result<bool>;
    fn remove(&mut self, c: &Self::Container, item: &Item, fire: Option<bool>) -> Result<bool>;
    fn to_array(&mut self, c: &Self::Container) -> Result<Vec<Option<Item>>>;
    fn get(&mut self, c: &Self::Container, slot: i32) -> Result<Option<Item>>;
    fn capacity(&mut self, c: &Self::Container) -> Result<i32>;
    fn item_count(&mut self, c: &Self::Container) -> Result<i32>;
    fn is_empty(&mut self, c: &Self::Container) -> Result<bool>;
    fn clear(&mut self, c: &Self::Container) -> Result<()>;
    fn shift(&mut self, c: &Self::Container) -> Result<()>;
    fn refresh(&mut self, c: &Self::Container) -> Result<()>;
    fn update(&mut self, c: &Self::Container) -> Result<()>;
    fn free_slots(&mut self, c: &Self::Container) -> Result<i32>;
    fn has_space_for(&mut self, c: &Self::Container, item: &Item) -> Result<bool>;
    /// `contains(id, amount)`.
    fn contains(&mut self, c: &Self::Container, id: i32, amount: i32) -> Result<bool>;
    fn contains_one(&mut self, c: &Self::Container, id: i32) -> Result<bool>;
    /// `Container.parse(JSONArray)`.
    fn parse_container(&mut self, c: &Self::Container, data: &Json) -> Result<()>;
    /// The save's copy of a container (`saveContainer`).
    fn saved_container(
        &mut self,
        c: &Self::Container,
    ) -> Result<crate::inventory::container::Container>;
    /// `item.getDefinition().isStackable()`.
    fn item_stackable(&mut self, id: i32) -> Result<bool>;
    /// `item.getDefinition().isTradeable()`.
    fn item_tradeable(&mut self, id: i32) -> Result<bool>;
    /// `item.getDefinition().getConfiguration(ItemConfigParser.BANKABLE, true)`.
    fn item_bankable(&mut self, id: i32) -> Result<bool>;
    /// `Item.getValue()`.
    fn item_value(&mut self, item: &Item) -> Result<i32>;
    /// `Item.getName()`.
    fn item_name(&mut self, item: &Item) -> Result<String>;
    /// `ItemDefinition.forId(id).getName()`.
    fn item_definition_name(&mut self, id: i32) -> Result<String>;
    /// `GroundItemManager.create(new GroundItem(item, location, decay, owner))`.
    fn ground_item(
        &mut self,
        item: &Item,
        location: Option<Loc>,
        decay: i32,
        owner: &Self::Node,
    ) -> Result<()>;

    // ---- content: pouches, scrolls and the familiar classes
    /// `SummoningPouch.get(id)`.
    fn pouch(&mut self, id: i32) -> Result<Option<Pouch>>;
    /// `SummoningScroll.forPouch(pouchId)`.
    fn scroll(&mut self, pouch_id: i32) -> Result<Option<Scroll>>;
    /// The class's `construct(owner, id)`.
    fn construct(
        &mut self,
        prototype: &Familiar<Self>,
        owner: &Self::Node,
        id: i32,
    ) -> Result<Familiar<Self>>;
    /// The class's `specialMove(special)`.
    fn special_move(
        &mut self,
        familiar: &Familiar<Self>,
        special: &FamiliarSpecial<Self>,
    ) -> Result<bool>;
    /// The class's `getIds()`.
    fn ids(&mut self, familiar: &Familiar<Self>) -> Result<Vec<i32>>;
    /// The class's `configureFamiliar()`.
    fn configure(&mut self, familiar: &Familiar<Self>) -> Result<()>;
    /// The class's `getText()`.
    fn text(&mut self, familiar: &Familiar<Self>) -> Result<String>;
    /// The class's own `handleFamiliarTick()` body, run before the body it
    /// inherits (a forager's passive production).
    fn familiar_tick(&mut self, familiar: &Familiar<Self>) -> Result<()>;
    /// The class's `adjustPlayerBattle(state)`.
    fn adjust_player_battle(
        &mut self,
        familiar: &Familiar<Self>,
        state: &mut Self::State,
    ) -> Result<()>;
    /// A forager's `getRandom()` (11 unless the class overrides it).
    fn forager_random(&mut self, familiar: &Familiar<Self>) -> Result<i32>;
    /// `ContentAPI.log(origin.getClass(), level, message)`.
    fn log(&mut self, origin: Origin<'_, Self>, level: LogLevel, message: &str) -> Result<()>;
}

/// `Location.getDistance(first, second)`: int differences squared and
/// summed with int overflow, then the square root.
pub fn distance(first: Loc, second: Loc) -> f64 {
    let x = first[0].wrapping_sub(second[0]);
    let y = first[1].wrapping_sub(second[1]);
    (x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y)) as f64).sqrt()
}

/// `a.getDistance(b)` where either location may be null (null throws).
pub(crate) fn distance_of(first: Option<Loc>, second: Option<Loc>) -> Result<f64> {
    match (first, second) {
        (Some(a), Some(b)) => Ok(distance(a, b)),
        _ => Err(Error::Null),
    }
}
