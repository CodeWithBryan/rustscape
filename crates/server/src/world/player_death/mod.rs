//! A player's death: the items kept on death (`DeathTask.getContainers`),
//! `Player.commenceDeath`/`finalizeDeath`, graves (`Grave`, `GraveController`,
//! `GraveType`), the Barrows durability rules a grave applies
//! (`BarrowsEquipment`) and the death-screen button (`DeathInterface`).
//!
//! These are libraries joined to native pieces: the kept and lost containers
//! and the player's inventory and equipment are native [`Container`]s, the
//! ground items are made, found and removed by the native ground manager
//! ([`gm`]) and [`ground`] items, the end of the death runs the native
//! `Entity.finalizeDeath` ([`npc_death::entity_finalize_death`]) and the
//! [`npc_death`] death task calls [`commence_death`] and [`finalize_death`]
//! through its `player_commence_death`/`player_finalize_death` services.
//! Every other system is a required service of [`KeptHost`] or [`Host`],
//! called in the original order with the original arguments. None has a
//! default answer.
//!
//! [`Container`]: crate::inventory::container::Container
use super::{
    doors::Location, ground_item as ground, ground_manager as gm, npc_death,
    player_walk::NodeHandle,
};
use crate::{inventory, player::ironman::Mode};

pub mod barrows;
mod death;
pub mod grave;
mod kept;
pub mod store;

pub use death::{commence_death, death_interface_button, finalize_death};
pub use grave::{Controller, GraveType};
pub use kept::get_containers;

/// What the original throws, as categories, and a failed service.
#[derive(Debug)]
pub enum Error {
    /// Everything the shared death and drop services return.
    Death(npc_death::Error),
    /// A container operation failed.
    Container(inventory::Error),
    /// `NumberFormatException` while reading the server store.
    Number,
    /// `UninitializedPropertyAccessException`: a grave's type read before
    /// it is configured.
    Uninitialized,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<npc_death::Error> for Error {
    fn from(error: npc_death::Error) -> Self {
        Self::Death(error)
    }
}
impl From<inventory::Error> for Error {
    fn from(error: inventory::Error) -> Self {
        Self::Container(error)
    }
}
impl From<gm::Error> for Error {
    fn from(error: gm::Error) -> Self {
        Self::Death(npc_death::Error::Ground(error))
    }
}
impl From<ground::Error> for Error {
    fn from(error: ground::Error) -> Self {
        Self::Death(npc_death::Error::from(error))
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// `NullPointerException`.
pub(crate) fn null() -> Error {
    Error::Death(npc_death::Error::Null)
}
/// `ArrayIndexOutOfBoundsException`.
pub(crate) fn index() -> Error {
    Error::Death(npc_death::Error::Index)
}

/// `"/save:" + STATS_BASE + ":" + STATS_DEATHS`.
pub const DEATHS: &str = "/save:stats_manager:deaths";
/// `GraveController.ATTR_GTYPE`.
pub const GRAVE_TYPE: &str = "/save:gravetype";
pub const ORIGINAL_LOCATION: &str = "/save:original-loc";
/// `Items.BONES_526`, `Items.COINS_995`.
pub const BONES: i32 = 526;
pub const COINS: i32 = 995;
/// `Skills.PRAYER`, `Skills.MAGIC`.
pub const PRAYER: i32 = 5;
pub const MAGIC: i32 = 6;
/// `ZoneType.SAFE.getId()`.
pub const SAFE_ZONE: i32 = 1;
/// The death screen, `Components.AIDE_DEATH_153`.
pub const DEATH_SCREEN: i32 = 153;

/// The two prayers the death paths ask about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prayer {
    ProtectItems,
    Retribution,
}

/// `Rights`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rights {
    RegularPlayer,
    PlayerModerator,
    Administrator,
}

/// `TickUtils.secondsToTicks`: Kotlin `Int` arithmetic.
pub fn seconds_to_ticks(seconds: i32) -> i32 {
    seconds.wrapping_mul(1000) / 600
}
/// `TickUtils.ticksToSeconds`.
pub fn ticks_to_seconds(ticks: i32) -> i32 {
    ticks.wrapping_mul(600) / 1000
}

/// `Globals.colorize(line)`: the six colour codes, then `%` followed by six
/// hexadecimal digits, then `</col>` and a space.
pub fn colorize(line: &str) -> String {
    let mut text = line
        .replace("%R", "<col=ff0000>")
        .replace("%O", "<col=ff6600>")
        .replace("%Y", "<col=ffff00>")
        .replace("%G", "<col=66ff33>")
        .replace("%B", "<col=3366ff>")
        .replace("%P", "<col=cc00ff>");
    let mut result = String::with_capacity(text.len());
    let bytes: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes[index] == '%'
            && index + 6 < bytes.len()
            && bytes[index + 1..index + 7]
                .iter()
                .all(char::is_ascii_hexdigit);
        if hex {
            result.push_str("<col=");
            result.extend(&bytes[index + 1..index + 7]);
            result.push('>');
            index += 7;
        } else {
            result.push(bytes[index]);
            index += 1;
        }
    }
    text = result;
    text.push_str("</col>");
    text.push(' ');
    text
}

/// The services `getContainers` needs. Each method is one original call.
pub trait KeptHost: inventory::container::Context {
    /// `player.getInventory()`.
    fn inventory(
        &mut self,
        player: &NodeHandle,
    ) -> npc_death::Result<inventory::container::Container>;
    /// `player.getEquipment()`.
    fn equipment(
        &mut self,
        player: &NodeHandle,
    ) -> npc_death::Result<inventory::container::Container>;
    /// `player.getSkullManager().isSkulled()`.
    fn skulled(&mut self, player: &NodeHandle) -> npc_death::Result<bool>;
    /// `player.getPrayer().get(type)`.
    fn prayer_active(&mut self, player: &NodeHandle, prayer: Prayer) -> npc_death::Result<bool>;
    /// `player.getIronmanManager().getMode()`, which can be null.
    fn ironman_mode(&mut self, player: &NodeHandle) -> npc_death::Result<Option<Mode>>;
}

/// Services of systems that are not ported, on top of the death task's, the
/// drop's and the ground manager's own (`npc_death::Host`, `drop::Host`,
/// `gm::Context`). Each method is one original call.
pub trait Host: KeptHost + npc_death::Host {
    /// An item's equipment plugin, `getConfiguration("equipment", null)`.
    type EquipmentPlugin;

    /// `node.isActive()`.
    fn is_active(&mut self, node: &NodeHandle) -> npc_death::Result<bool>;
    /// `player.isMale()`.
    fn is_male(&mut self, player: &NodeHandle) -> npc_death::Result<bool>;
    /// `player.getName()`.
    fn get_name(&mut self, player: &NodeHandle) -> npc_death::Result<Option<String>>;
    /// The `name` field, which `finalizeDeath` reads directly.
    fn name_field(&mut self, player: &NodeHandle) -> npc_death::Result<Option<String>>;
    /// `player.getDetails().getRights()`.
    fn rights(&mut self, player: &NodeHandle) -> npc_death::Result<Rights>;
    /// `GlobalStats.incrementDeathCount()`.
    fn increment_death_count(&mut self) -> npc_death::Result<()>;
    /// `settings.setSpecialEnergy(value)`.
    fn set_special_energy(&mut self, player: &NodeHandle, value: i32) -> npc_death::Result<()>;
    /// `settings.getRunEnergy()`.
    fn run_energy(&mut self, player: &NodeHandle) -> npc_death::Result<f64>;
    /// `settings.updateRunEnergy(value)`.
    fn update_run_energy(&mut self, player: &NodeHandle, value: f64) -> npc_death::Result<()>;
    /// `getPacketDispatch().sendMessage(message)`.
    fn dispatch_message(&mut self, player: &NodeHandle, message: &str) -> npc_death::Result<()>;
    /// `packetDispatch.sendTempMusic(id)`.
    fn temp_music(&mut self, player: &NodeHandle, id: i32) -> npc_death::Result<()>;
    /// `ContentAPI.sendMessage(player, message)`.
    fn api_message(&mut self, player: &NodeHandle, message: &str) -> npc_death::Result<()>;
    /// `getZoneMonitor().handleDeath(killer)` for a dying player; the killer
    /// can be null when `finalizeDeath` is called directly.
    fn zone_player_death(
        &mut self,
        player: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> npc_death::Result<bool>;
    /// `getProperties().isSafeZone()`.
    fn safe_zone(&mut self, player: &NodeHandle) -> npc_death::Result<bool>;
    /// `getZoneMonitor().getType()`.
    fn zone_type(&mut self, player: &NodeHandle) -> npc_death::Result<i32>;
    /// `zoneMonitor.isRestricted(ZoneRestriction.GRAVES)`.
    fn graves_restricted(&mut self, player: &NodeHandle) -> npc_death::Result<bool>;
    /// `skullManager.isWilderness()`.
    fn skull_wilderness(&mut self, player: &NodeHandle) -> npc_death::Result<bool>;
    /// `WildernessZone.isInZone(player)`: WildernessZone is not ported.
    fn in_wilderness_zone(&mut self, player: &NodeHandle) -> npc_death::Result<bool>;
    /// `skullManager.setSkulled(value)`.
    fn set_skulled(&mut self, player: &NodeHandle, value: bool) -> npc_death::Result<()>;
    /// `prayer.startRetribution(killer)`.
    fn start_retribution(
        &mut self,
        player: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> npc_death::Result<()>;
    /// `getPrayer().reset()`.
    fn prayer_reset(&mut self, player: &NodeHandle) -> npc_death::Result<()>;
    /// `entity.getAttribute(key, fallback)` for a Location; `None` when absent.
    fn attribute_location(
        &mut self,
        entity: &NodeHandle,
        key: &str,
    ) -> npc_death::Result<Option<Option<Location>>>;
    /// `getAttributes().containsKey(key)`.
    fn attributes_contain(&mut self, entity: &NodeHandle, key: &str) -> npc_death::Result<bool>;
    /// The `location` field.
    fn location_field(&mut self, node: &NodeHandle) -> npc_death::Result<Option<Location>>;
    /// Writing the `location` field, as Kotlin does from `Grave`.
    fn set_location_field(
        &mut self,
        node: &NodeHandle,
        location: Option<Location>,
    ) -> npc_death::Result<()>;
    /// `Repository.sendNews(message)`.
    fn repository_news(&mut self, message: &str) -> npc_death::Result<()>;
    /// `getSkills().getTotalLevel()`.
    fn total_level(&mut self, player: &NodeHandle) -> npc_death::Result<i32>;
    /// `permadeath(player)` (`Permadeath.kt`): not ported.
    fn permadeath(&mut self, player: &NodeHandle) -> npc_death::Result<()>;
    /// `InteractionListeners.run(id, player, item, isEquip)`.
    fn run_equip_listeners(
        &mut self,
        id: i32,
        player: &NodeHandle,
        item: &inventory::item::Handle,
        equip: bool,
    ) -> npc_death::Result<()>;
    /// `item.getDefinition().getConfiguration("equipment", null)`.
    fn equipment_plugin(
        &mut self,
        definition: &crate::definitions::item::Handle,
    ) -> npc_death::Result<Option<Self::EquipmentPlugin>>;
    /// `equipPlugin.fireEvent("unequip", player, item)`.
    fn fire_unequip(
        &mut self,
        plugin: Self::EquipmentPlugin,
        player: &NodeHandle,
        item: &inventory::item::Handle,
    ) -> npc_death::Result<()>;
    /// `ContentAPI.getItemName(id)`.
    fn item_name(&mut self, id: i32) -> npc_death::Result<Option<String>>;
    /// `ContentAPI.itemDefinition(id)`.
    fn item_definition(&mut self, id: i32) -> npc_death::Result<crate::definitions::item::Handle>;
    /// `ContentAPI.getStatLevel(entity, skill)`.
    fn stat_level(&mut self, entity: &NodeHandle, skill: i32) -> npc_death::Result<i32>;
    /// `plugin.getDeathItem(item)`.
    fn plugin_death_item(
        &mut self,
        plugin: Self::Plugin,
        item: &inventory::item::Handle,
    ) -> npc_death::Result<Option<inventory::item::Handle>>;
    /// `PlayerMonitor.log(player, LogType.PK, details)`.
    fn monitor_pk(&mut self, player: &NodeHandle, details: &str) -> npc_death::Result<()>;
    /// `SlayerEquipmentFlags.updateFlags(player)`.
    fn slayer_flags(&mut self, player: &NodeHandle) -> npc_death::Result<()>;
    /// `familiarManager.dismiss()`.
    fn dismiss_familiar(&mut self, player: &NodeHandle) -> npc_death::Result<()>;
    /// `interfaceManager.openDefaultTabs()`.
    fn open_default_tabs(&mut self, player: &NodeHandle) -> npc_death::Result<()>;
    /// `ContentAPI.setComponentVisibility(player, iface, child, hide)`.
    fn component_visibility(
        &mut self,
        player: &NodeHandle,
        iface: i32,
        child: i32,
        hide: bool,
    ) -> npc_death::Result<()>;
    /// `appearance.sync()`.
    fn appearance_sync(&mut self, player: &NodeHandle) -> npc_death::Result<()>;
    /// `getSavedData().getGlobalData().isDeathScreenDisabled()`.
    fn death_screen_disabled(&mut self, player: &NodeHandle) -> npc_death::Result<bool>;
    /// `getSavedData().getGlobalData().setDisableDeathScreen(value)`.
    fn set_death_screen_disabled(
        &mut self,
        player: &NodeHandle,
        value: bool,
    ) -> npc_death::Result<()>;
    /// `getInterfaceManager().open(new Component(id))`.
    fn open_component(&mut self, player: &NodeHandle, id: i32) -> npc_death::Result<()>;
    /// `ContentAPI.closeInterface(player)`.
    fn close_interface(&mut self, player: &NodeHandle) -> npc_death::Result<()>;

    /// `new Grave()`: the binding constructs the grave's NPC
    /// (`AbstractNPC(GRAVESTONE_6571, Location.create(0, 0, 0), false)`) and
    /// returns [`grave::Grave::new`] over it.
    fn new_grave(&mut self) -> npc_death::Result<grave::Handle>;
    /// `npc.setRespawn(value)`.
    fn set_respawn(&mut self, npc: &NodeHandle, value: bool) -> npc_death::Result<()>;
    /// `npc.setWalks(value)`.
    fn set_walks(&mut self, npc: &NodeHandle, value: bool) -> npc_death::Result<()>;
    /// `npc.setNeverWalks(value)`.
    fn set_never_walks(&mut self, npc: &NodeHandle, value: bool) -> npc_death::Result<()>;
    /// `npc.init()`.
    fn npc_init(&mut self, npc: &NodeHandle) -> npc_death::Result<()>;
    /// `Repository.uid_map[uid]`.
    fn uid_map(&mut self, uid: i32) -> npc_death::Result<Option<NodeHandle>>;
    /// `ContentAPI.registerHintIcon(player, node)`.
    fn register_hint_icon(
        &mut self,
        player: &NodeHandle,
        node: &NodeHandle,
    ) -> npc_death::Result<()>;
    /// `ContentAPI.clearHintIcon(player)`.
    fn clear_hint_icon(&mut self, player: &NodeHandle) -> npc_death::Result<()>;
    /// `ContentAPI.setVarbit(player, id, value)`.
    fn set_varbit(&mut self, player: &NodeHandle, id: i32, value: i32) -> npc_death::Result<()>;
    /// `ContentAPI.openInterface(player, id)`.
    fn open_interface(&mut self, player: &NodeHandle, id: i32) -> npc_death::Result<()>;
    /// `ContentAPI.setInterfaceText(player, text, iface, child)`.
    fn interface_text(
        &mut self,
        player: &NodeHandle,
        text: &str,
        iface: i32,
        child: i32,
    ) -> npc_death::Result<()>;
    /// `ContentAPI.playAudio(player, id)`.
    fn play_audio(&mut self, player: &NodeHandle, id: i32) -> npc_death::Result<()>;
    /// `ContentAPI.animate(entity, id)`.
    fn animate(&mut self, entity: &NodeHandle, id: i32) -> npc_death::Result<()>;
    /// `player.skills.prayerPoints`.
    fn prayer_points(&mut self, player: &NodeHandle) -> npc_death::Result<f64>;
    /// `player.skills.prayerPoints = value`.
    fn set_prayer_points(&mut self, player: &NodeHandle, value: f64) -> npc_death::Result<()>;
    /// `ServerStore.getArchive(name)`; the archive itself is the caller's.
    fn store_archive(&mut self, name: &str) -> npc_death::Result<()>;
}
