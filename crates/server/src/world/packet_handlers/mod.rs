//! `PacketProcessor`'s item, NPC, player, use-with, component use-with, slot
//! switch, examine and track-finished branches, and the core handlers they
//! reach: `InteractPlugin.handleItemOption` with its pulse body,
//! `UseWithHandler` (its registry, `run` and three pulse bodies),
//! `EquipHandler` (equip, unequip, registration) and `DropListener` (drop,
//! destroy, the queued drop script and the destroy dialogue action).
//!
//! | Packet | Original | Here |
//! | --- | --- | --- |
//! | `ItemAction` | `processItemAction` | [`actions::item_action`], [`item_option`] |
//! | `NpcAction` | `processNpcAction` | [`actions::npc_action`] |
//! | `PlayerAction` | `processPlayerAction` | [`actions::player_action`] |
//! | `UseWith*` | `processUseWith`, `UseWithHandler.run` | [`use_with`] |
//! | `Component*Action` | `processComponentUseWith` | [`component`] |
//! | `SlotSwitch*` | `processSlotSwitch`, `switchItem` | [`slot_switch`] |
//! | `ItemExamine`, `SceneryExamine`, `NpcExamine`, `TrackFinished` | `process` | [`examine`] |
//!
//! These are libraries over explicit services: every call the original makes
//! outside the ported bodies is a required method of [`Base`] or of a
//! branch's own `Context`, with no default. The world binds them to the
//! native systems that already exist (the InteractionListeners registry, the
//! legacy `InteractPlugin.handle`/`handleInvalidInteraction` bodies of
//! `world::legacy_interaction`, the containers of `inventory`, the
//! repository's node lists, the spell framework's `castSpell`, the music
//! player of `world::player_tabs::music`). The examine texts are computed
//! here from the native definitions themselves. Content (per-item, NPC,
//! object listeners and plugins, `UseWithHandler` subclasses, familiar
//! specials, pets, graves) stays content: it is reached through those
//! services. See docs/packet-handlers.md.
pub mod actions;
pub mod component;
pub mod drop;
pub mod equip;
pub mod examine;
pub mod item_option;
pub mod slot_switch;
pub mod use_with;

use super::{interaction::OptionHandle, interaction_listeners::Kind};
use crate::definitions::{item, property::Property};

/// What stops a call, as the original's exception classes do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `NullPointerException` (Kotlin's null checks included).
    Null,
    /// `IndexOutOfBoundsException`, with the JDK's message.
    Index(Option<String>),
    /// `ClassCastException`.
    Cast,
    /// A service failed with what it threw (its message, if any).
    Binding(Option<String>),
}
impl Error {
    /// `Throwable.getMessage()`.
    pub fn message(&self) -> Option<String> {
        match self {
            Self::Index(text) | Self::Binding(text) => text.clone(),
            Self::Null | Self::Cast => None,
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// `Components.INVENTORY_149`.
pub const INVENTORY_149: i32 = 149;
/// `Components.BANK_V2_MAIN_762`.
pub const BANK_V2_MAIN_762: i32 = 762;
/// `Components.EQUIP_SCREEN2_667`.
pub const EQUIP_SCREEN2_667: i32 = 667;
/// "Nothing interesting happens."
pub const NOTHING: &str = "Nothing interesting happens.";

/// The player's containers the packets reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Container {
    Inventory,
    Bank,
    Equipment,
}
impl Container {
    pub fn name(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Bank => "bank",
            Self::Equipment => "equipment",
        }
    }
}

/// `getLikelyContainerForIface`.
pub fn likely_container(iface: i32) -> Option<Container> {
    match iface {
        INVENTORY_149 => Some(Container::Inventory),
        BANK_V2_MAIN_762 => Some(Container::Bank),
        EQUIP_SCREEN2_667 => Some(Container::Equipment),
        _ => None,
    }
}

/// `IntType.name()`.
pub fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Item => "ITEM",
        Kind::Scenery => "SCENERY",
        Kind::Npc => "NPC",
        Kind::GroundItem => "GROUNDITEM",
        Kind::Player => "PLAYER",
    }
}

/// The node classes the handlers test with `instanceof` (a ground item is
/// also an `Item`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Item,
    GroundItem,
    Npc,
    Scenery,
    Player,
}
impl NodeKind {
    pub fn is_item(self) -> bool {
        matches!(self, Self::Item | Self::GroundItem)
    }
}

/// The listeners [`Base::on`] registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listener {
    /// `EquipHandler`: [`equip::handle_equip`].
    Equip,
    /// `DropListener`: [`drop::handle_drop_action`].
    Drop,
}

/// The services every branch uses. A method without a `Result` is a field
/// read of the original.
pub trait Base {
    /// A node (player, NPC, item, ground item, scenery); `==` is identity.
    type Node: Clone + PartialEq;
    /// A node's `InteractPlugin`.
    type Interaction: Clone;

    /// `node.getId()`.
    fn id(&mut self, node: &Self::Node) -> Result<i32>;
    /// The node's class, for `instanceof`.
    fn node_kind(&mut self, node: &Self::Node) -> NodeKind;
    /// `node.getIndex()` (an item's slot).
    fn index(&mut self, node: &Self::Node) -> Result<i32>;
    /// `node.setIndex(index)`.
    fn set_index(&mut self, node: &Self::Node, index: i32) -> Result<()>;
    /// `node.isActive()`.
    fn active(&mut self, node: &Self::Node) -> Result<bool>;
    /// `String.valueOf(node)`, as string concatenation calls it.
    fn text(&mut self, node: Option<&Self::Node>) -> Result<String>;
    /// `String.toLowerCase()` in the world's default locale.
    fn lowercase_default(&self, text: &str) -> String;
    /// `player.debug(text)`.
    fn debug(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `player.sendMessage(text)`.
    fn message(&mut self, player: &Self::Node, text: Option<&str>) -> Result<()>;
    /// `player.getPacketDispatch().sendMessage(text)`.
    fn packet_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `ContentAPI.sendMessage(player, text)`.
    fn api_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `sendClearMinimap(player)`: the ClearMinimapFlag packet.
    fn clear_minimap(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.scripts.reset()`.
    fn reset_scripts(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.locks.isInteractionLocked()`.
    fn interaction_locked(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.locks.isMovementLocked()`.
    fn movement_locked(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.locks.equipmentLock != null`.
    fn has_equipment_lock(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.locks.equipmentLock.isLocked()`.
    fn equipment_locked(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.locks.equipmentLock.getMessage()`.
    fn equipment_lock_message(&mut self, player: &Self::Node) -> Result<Option<String>>;
    /// `player.location` (null fails where it is dereferenced).
    fn location(&mut self, player: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `node.getInteraction()` (with its default initialization).
    fn interaction(&mut self, node: &Self::Node) -> Result<Option<Self::Interaction>>;
    /// `interaction.get(index)`: the options array element (out of range
    /// fails as the array does).
    fn option(
        &mut self,
        interaction: &Self::Interaction,
        index: i32,
    ) -> Result<Option<OptionHandle>>;
    /// `InteractPlugin.handle(player, option)`: `legacy_interaction::handle`.
    fn handle(
        &mut self,
        interaction: &Self::Interaction,
        player: &Self::Node,
        option: &OptionHandle,
    ) -> Result<()>;
    /// `InteractPlugin.handleInvalidInteraction(player, node, Option.NULL)`:
    /// `legacy_interaction::invalid`.
    fn invalid(&mut self, player: &Self::Node, node: &Self::Node) -> Result<()>;
    /// `InteractionListeners.run(id, type, option, player, node)`.
    fn listeners(
        &mut self,
        id: i32,
        kind: Kind,
        option: Option<&str>,
        player: &Self::Node,
        node: &Self::Node,
    ) -> Result<bool>;
    /// `container.capacity()` (`toArray().size` is the same array).
    fn capacity(&mut self, player: &Self::Node, container: Container) -> Result<i32>;
    /// `container.get(slot)`: null outside the array.
    fn get(
        &mut self,
        player: &Self::Node,
        container: Container,
        slot: i32,
    ) -> Result<Option<Self::Node>>;
    /// `container.replace(item, slot, fire)`: the item it replaced.
    fn replace(
        &mut self,
        player: &Self::Node,
        container: Container,
        item: Option<&Self::Node>,
        slot: i32,
        fire: bool,
    ) -> Result<Option<Self::Node>>;
    /// `Repository.npcs[index]` (`NodeList.get`).
    fn npc(&mut self, index: i32) -> Result<Option<Self::Node>>;
    /// `Repository.players[index]`.
    fn other_player(&mut self, index: i32) -> Result<Option<Self::Node>>;
    /// `npc.getShownNPC(player)`.
    fn shown_npc(&mut self, npc: &Self::Node, player: &Self::Node) -> Result<Self::Node>;
    /// `GroundItemManager.get(id, location, player)`.
    fn ground(
        &mut self,
        id: i32,
        location: [i32; 3],
        player: &Self::Node,
    ) -> Result<Option<Self::Node>>;
    /// `ServerConstants.MAX_PLAYERS`.
    fn max_players(&mut self) -> i32;
    /// `ServerConstants.MAX_NPCS`.
    fn max_npcs(&mut self) -> i32;
    /// `item.getDefinition()`: the item's cached definition.
    fn item_definition_of(&mut self, item: &Self::Node) -> Result<item::Handle>;
    /// `playAudio(player, id)`.
    fn audio(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `InteractionListener.on(type, options, handler)` at registration.
    fn on(&mut self, kind: Kind, options: &[&str], listener: Listener) -> Result<()>;
}

/// The player's plane: `player.location.z`.
pub(crate) fn plane<C: Base + ?Sized>(ctx: &mut C, player: &C::Node) -> Result<i32> {
    Ok(ctx.location(player)?.ok_or(Error::Null)?[2])
}

/// A string template's text for a nullable string.
pub(crate) fn or_null(text: Option<String>) -> String {
    text.unwrap_or_else(|| "null".into())
}

/// `definition.getConfiguration<Int>(key, fail)`, unboxed where Kotlin
/// unboxes it: a missing or null value is `fail`, any other type fails the
/// cast.
pub fn config_int(definition: &item::Handle, key: &str, fail: i32) -> Result<i32> {
    match definition.read().handlers.get(key) {
        None | Some(Property::Null) => Ok(fail),
        Some(Property::Int(value)) => Ok(*value),
        Some(_) => Err(Error::Cast),
    }
}

/// `definition.getConfiguration<Plugin<Any>>(key, null)`.
pub fn config_plugin(
    definition: &item::Handle,
    key: &str,
) -> Result<Option<crate::definitions::plugin::Handle>> {
    match definition.read().handlers.get(key) {
        None | Some(Property::Null) => Ok(None),
        Some(Property::Plugin(plugin)) => Ok(Some(plugin.clone())),
        Some(_) => Err(Error::Cast),
    }
}
