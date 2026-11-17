//! The shop engine: `core.game.shops.Shop` (Shop, ShopItem, ShopListener) and
//! `core.game.shops.Shops` (the start-up listener that builds every shop from
//! `shops.json`, the tick listener that restocks and clears the general player
//! stock, the NPC "trade"/"shop" listeners, the destination override and the
//! interface listeners of 620 and 621), in the original order.
//!
//! This is a library. It runs on the native containers (`inventory::container`),
//! the native InterfaceManager (`world::interfaces`), the native
//! InterfaceListeners registry (`world::interface_listeners`, with typed shop
//! callbacks), the native Ironman manager, native item definitions and a typed
//! amount script for the native input prompt. Every other outside call is a
//! required [`Host`] method with no default. Nothing installs it in a live
//! world yet. See docs/shops.md.
pub mod controller;
pub mod shop;
pub mod ui;

use crate::{
    definitions::item::Registry,
    inventory::{
        container::{Container, Event, Factory},
        container_listener::Handle as Listener,
        item::Handle as Item,
    },
    player::ironman,
    world::{
        component_objects::{Captures, NativeObject, Plugin},
        drop_tables::LogLevel,
        interface_listeners,
        interfaces::{self, CloseEvent, Handle as Component, Manager},
        player_walk::NodeHandle,
        repository::hash_map::JavaHashMap,
    },
};
use rs09_protocol::game::Frame;
use std::sync::{Arc, Mutex, MutexGuard};

pub use shop::{AmountScript, Shop, ShopItem, ShopListener, Status};

/// Java exception categories the original raises on these paths, plus a
/// failure of a host service. Messages of JVM exceptions are not compared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// NullPointerException (a `!!`, a null receiver, a null cast).
    Null,
    /// NumberFormatException from `toInt()`.
    Number,
    /// ArrayIndexOutOfBoundsException (a Kotlin `Array` index).
    Bounds,
    /// IndexOutOfBoundsException from an ArrayList.
    ListBounds,
    /// ClassCastException.
    ClassCast,
    /// ConcurrentModificationException.
    Concurrent,
    /// ArithmeticException (`% 0`).
    Arithmetic,
    /// IllegalArgumentException (`roundToInt` of NaN).
    Argument,
    /// org.json.simple.parser.ParseException.
    Parse,
    /// java.io.FileNotFoundException from `FileReader`.
    File,
    /// A host service failed; the text is the host's.
    Service(String),
}
pub type Result<T> = std::result::Result<T, Error>;

const TAG: &str = "shops-error:";
impl Error {
    /// Stable code, also used to carry the error through other native layers.
    pub fn code(&self) -> String {
        match self {
            Self::Null => "Null".into(),
            Self::Number => "Number".into(),
            Self::Bounds => "Bounds".into(),
            Self::ListBounds => "ListBounds".into(),
            Self::ClassCast => "ClassCast".into(),
            Self::Concurrent => "Concurrent".into(),
            Self::Arithmetic => "Arithmetic".into(),
            Self::Argument => "Argument".into(),
            Self::Parse => "Parse".into(),
            Self::File => "File".into(),
            Self::Service(text) => format!("Service:{text}"),
        }
    }
    pub fn from_code(code: &str) -> Self {
        match code {
            "Null" => Self::Null,
            "Number" => Self::Number,
            "Bounds" => Self::Bounds,
            "ListBounds" => Self::ListBounds,
            "ClassCast" => Self::ClassCast,
            "Concurrent" => Self::Concurrent,
            "Arithmetic" => Self::Arithmetic,
            "Argument" => Self::Argument,
            "Parse" => Self::Parse,
            "File" => Self::File,
            other => Self::Service(other.strip_prefix("Service:").unwrap_or(other).into()),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code())
    }
}
impl std::error::Error for Error {}
impl From<crate::inventory::Error> for Error {
    fn from(e: crate::inventory::Error) -> Self {
        use crate::inventory::Error as E;
        match e {
            E::Null => Self::Null,
            E::Bounds => Self::Bounds,
            E::ListBounds => Self::ListBounds,
            E::Type => Self::ClassCast,
            E::Number => Self::Number,
            E::ConcurrentModification => Self::Concurrent,
            E::Binding(text) => match text.strip_prefix(TAG) {
                Some(code) => Self::from_code(code),
                None => Self::Service(text),
            },
            other => Self::Service(format!("{other:?}")),
        }
    }
}
impl From<Error> for crate::inventory::Error {
    fn from(e: Error) -> Self {
        use crate::inventory::Error as E;
        match e {
            Error::Null => E::Null,
            Error::Bounds => E::Bounds,
            Error::ListBounds => E::ListBounds,
            Error::ClassCast => E::Type,
            Error::Number => E::Number,
            Error::Concurrent => E::ConcurrentModification,
            other => E::Binding(format!("{TAG}{}", other.code())),
        }
    }
}
impl From<interfaces::Error> for Error {
    fn from(e: interfaces::Error) -> Self {
        use interfaces::Error as E;
        match e {
            E::Null => Self::Null,
            E::Index => Self::Bounds,
            E::Number => Self::Number,
            E::Type => Self::ClassCast,
            E::Binding(text) => match text.strip_prefix(TAG) {
                Some(code) => Self::from_code(code),
                None => Self::Service(text),
            },
            other => Self::Service(format!("{other:?}")),
        }
    }
}
impl From<Error> for interfaces::Error {
    fn from(e: Error) -> Self {
        use interfaces::Error as E;
        match e {
            Error::Null => E::Null,
            Error::Bounds => E::Index,
            Error::Number => E::Number,
            Error::ClassCast => E::Type,
            other => E::Binding(format!("{TAG}{}", other.code())),
        }
    }
}

/// `"world.personalized_shops"` (Shops.personalizedShops).
pub const PERSONALIZED_SHOPS: &str = "world.personalized_shops";

/// `Shop.fixedPriceItems`: spirit shards, pouch, both magic capes.
pub fn fixed_price(id: i32) -> Option<i32> {
    match id {
        12183 => Some(25),
        12155 => Some(1),
        9762 | 9763 => Some(99000),
        _ => None,
    }
}

/// Java `String.hashCode` over UTF-16 units (the shared stock key is
/// `ServerConstants.SERVER_NAME.hashCode()`).
pub fn java_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |h, unit| {
        h.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

/// The two companion objects' tables (`Shops.shopsById`, `Shops.shopsByNpc`,
/// `Shop.generalPlayerStock`, `Shop.listenerInstances`), one per world as the
/// original has one per JVM. Clones are the same tables.
pub struct Tables {
    pub shops_by_id: JavaHashMap<i32, Shop>,
    pub shops_by_npc: JavaHashMap<i32, Shop>,
    pub general_player_stock: Container,
    pub listener_instances: JavaHashMap<i32, Listener>,
}
#[derive(Clone)]
pub struct World(Arc<Mutex<Tables>>);
impl World {
    /// The tables a fresh JVM starts with: empty maps and the general player
    /// stock, `Container(40, ContainerType.SHOP)`.
    pub fn new(factory: &Factory) -> Result<Self> {
        let general = factory.create(
            40,
            crate::inventory::container::ContainerType::Shop,
            crate::inventory::container::SortType::Id,
        )?;
        Ok(Self(Arc::new(Mutex::new(Tables {
            shops_by_id: JavaHashMap::new(),
            shops_by_npc: JavaHashMap::new(),
            general_player_stock: general,
            listener_instances: JavaHashMap::new(),
        }))))
    }
    pub fn tables(&self) -> MutexGuard<'_, Tables> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn general_player_stock(&self) -> Container {
        self.tables().general_player_stock.clone()
    }
    pub fn shop_by_id(&self, id: i32) -> Option<Shop> {
        self.tables().shops_by_id.get(&id).cloned()
    }
    pub fn shop_by_npc(&self, id: i32) -> Option<Shop> {
        self.tables().shops_by_npc.get(&id).cloned()
    }
    /// `listenerInstances[uid]`.
    pub fn listener(&self, uid: i32) -> Option<Listener> {
        self.tables().listener_instances.get(&uid).cloned()
    }
    pub fn set_listener(&self, uid: i32, listener: Listener) {
        self.tables().listener_instances.put(uid, listener);
    }
}

/// The NPC as the trade listeners and the destination override read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Npc {
    pub id: i32,
    /// `direction.stepX`, `direction.stepY`.
    pub step: (i32, i32),
    pub location: Location,
}
/// `Location` x, y and plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// The quests the NPC listeners ask about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quest {
    FremennikTrials,
    MerlinsCrystal,
}
impl Quest {
    pub fn name(self) -> &'static str {
        match self {
            Self::FremennikTrials => "THE_FREMENNIK_TRIALS",
            Self::MerlinsCrystal => "MERLINS_CRYSTAL",
        }
    }
}

/// The two shop events `player.dispatch` receives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopEvent {
    Purchase,
    Sale,
}

/// A content item view (`ShopItemView`) given to `openFor`; the default view
/// is `None`. The host runs it ([`Host::view`]).
pub type View = Option<Plugin>;

/// Every call the ported classes make outside themselves. None has a default;
/// a host must not answer one from state that is not the real system.
pub trait Host {
    // World.
    /// The world's companion tables.
    fn world(&mut self) -> Result<World>;
    /// `GameWorld.ticks` (also `getWorldTicks()`).
    fn ticks(&mut self) -> Result<i32>;
    fn definitions(&mut self) -> Result<Arc<Registry>>;
    /// The world's container factory (its single NULL_ITEM).
    fn factory(&mut self) -> Result<Factory>;
    /// `getServerConfig().getBoolean(key, default)`.
    fn config_bool(&mut self, key: &str, default: bool) -> Result<bool>;
    /// `ServerConstants.SERVER_NAME`.
    fn server_name(&mut self) -> Result<String>;
    /// `ServerConstants.CONFIG_PATH` (`None` is Java null).
    fn config_path(&mut self) -> Result<Option<String>>;
    /// `ServerConstants.PLAYER_STOCK_CLEAR_INTERVAL`.
    fn stock_clear_interval(&mut self) -> Result<i32>;
    /// `ServerConstants.PLAYER_STOCK_RECIRCULATE`.
    fn stock_recirculate(&mut self) -> Result<bool>;
    /// `FileReader(path)` read whole; [`Error::File`] when it cannot open.
    fn read_file(&mut self, path: &str) -> Result<String>;
    /// `core.api.log(origin, level, message)`.
    fn log(&mut self, level: LogLevel, message: &str) -> Result<()>;
    /// The world's InterfaceListeners registry (the shop's callbacks are registered there).
    fn interface_listeners(&mut self) -> Result<interface_listeners::Registry>;
    // Player fields.
    /// `player.details.uid`.
    fn uid(&mut self, p: &NodeHandle) -> Result<i32>;
    fn username(&mut self, p: &NodeHandle) -> Result<String>;
    fn inventory(&mut self, p: &NodeHandle) -> Result<Container>;
    fn ironman(&mut self, p: &NodeHandle) -> Result<ironman::Manager>;
    fn interface_manager(&mut self, p: &NodeHandle) -> Result<Manager>;
    // Attributes ("shop", "shop-cont", "shop-main"): getAttribute with a null
    // value answering the default, as GameAttributes does.
    fn shop_attribute(&mut self, p: &NodeHandle) -> Result<Option<Shop>>;
    fn set_shop_attribute(&mut self, p: &NodeHandle, shop: Shop) -> Result<()>;
    fn shop_container(&mut self, p: &NodeHandle) -> Result<Option<Container>>;
    fn set_shop_container(&mut self, p: &NodeHandle, container: Container) -> Result<()>;
    fn shop_main(&mut self, p: &NodeHandle, default: bool) -> Result<bool>;
    fn set_shop_main(&mut self, p: &NodeHandle, main: bool) -> Result<()>;
    // Services.
    /// `Player.sendMessage`.
    fn message(&mut self, p: &NodeHandle, text: &str) -> Result<()>;
    /// `ContentAPI.sendMessage` (it splits at 86 characters itself).
    fn content_message(&mut self, p: &NodeHandle, text: &str) -> Result<()>;
    /// `ContentAPI.sendDialogue`.
    fn dialogue(&mut self, p: &NodeHandle, text: &str) -> Result<()>;
    /// `Player.debug`.
    fn debug(&mut self, p: &NodeHandle, text: &str) -> Result<()>;
    /// The player's session write of an encoded frame.
    fn send(&mut self, p: &NodeHandle, frame: Frame) -> Result<()>;
    /// `itemDefinition(id).examine`.
    fn examine(&mut self, id: i32) -> Result<String>;
    /// `player.dispatch(ItemShopPurchaseEvent / ItemShopSellEvent(itemId, amount, currency))`.
    fn shop_event(
        &mut self,
        p: &NodeHandle,
        kind: ShopEvent,
        item: i32,
        amount: i32,
        currency: &Item,
    ) -> Result<()>;
    /// A content `ShopItemView` invoked with the listener's player.
    fn view(&mut self, p: &NodeHandle, item: &Item, view: &Plugin) -> Result<Option<Item>>;
    /// `sendInputDialogue(player, InputType.AMOUNT, prompt, handler)`; the
    /// handler body is [`shop::run_amount`].
    fn input_dialogue(&mut self, p: &NodeHandle, prompt: &str, script: AmountScript) -> Result<()>;
    /// `GrandExchange.addBotOffer(id, amount)`.
    fn ge_offer(&mut self, id: i32, amount: i32) -> Result<bool>;
    /// `TanningProduct.open(player, npc)` (content).
    fn tanning(&mut self, p: &NodeHandle, npc: i32) -> Result<()>;
    /// `getQuestPoints(player)`.
    fn quest_points(&mut self, p: &NodeHandle) -> Result<i32>;
    /// `getQuestStage(player, quest)`.
    fn quest_stage(&mut self, p: &NodeHandle, quest: Quest) -> Result<i32>;
    /// `isQuestComplete(player, quest)`.
    fn quest_complete(&mut self, p: &NodeHandle, quest: Quest) -> Result<bool>;
    /// `sendNPCDialogue(player, npc, msg, expression)` with `hide` false.
    fn npc_dialogue(
        &mut self,
        p: &NodeHandle,
        npc: i32,
        text: &str,
        expression: &'static str,
    ) -> Result<()>;
    /// `npc.getAttribute("facing_booth", false)`.
    fn npc_facing_booth(&mut self, npc: &Npc) -> Result<bool>;
    /// A container listener that is not a ShopListener.
    fn listener_update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
    ) -> Result<()>;
    fn listener_refresh(&mut self, listener: Listener, container: &mut Container) -> Result<()>;
    /// An InterfaceListeners callback that is not one of the shop's.
    fn other_listener(
        &mut self,
        p: &NodeHandle,
        callback: interface_listeners::Callback,
        component: &Component,
        button: Option<interface_listeners::Button>,
    ) -> Result<bool>;
    // InterfaceManager services the shop does not own (see ui.rs).
    fn ui_validate(&mut self, p: &NodeHandle, m: &Manager) -> Result<()>;
    fn ui_component(&mut self, p: &NodeHandle, id: i32) -> Result<Component>;
    fn ui_resizable(&mut self, p: &NodeHandle) -> Result<bool>;
    fn ui_log(&mut self, p: &NodeHandle, text: String) -> Result<()>;
    fn ui_plugin_open(&mut self, p: &NodeHandle, plugin: Plugin, c: &Component) -> Result<()>;
    fn ui_close_event(&mut self, p: &NodeHandle, event: CloseEvent, c: &Component) -> Result<bool>;
    fn ui_dispatch_open(&mut self, p: &NodeHandle, c: Component) -> Result<()>;
    fn ui_dispatch_close(&mut self, p: &NodeHandle, c: Option<Component>) -> Result<()>;
    fn ui_set_varp(&mut self, p: &NodeHandle, id: i32, value: i32) -> Result<()>;
    fn ui_weapon_extension(&mut self, p: &NodeHandle) -> Result<Option<Component>>;
    fn ui_create_weapon(&mut self, p: &NodeHandle) -> Result<Component>;
    fn ui_add_weapon(&mut self, p: &NodeHandle, c: Component) -> Result<()>;
    fn ui_spellbook(&mut self, p: &NodeHandle) -> Result<i32>;
    fn ui_has_familiar(&mut self, p: &NodeHandle) -> Result<bool>;
    fn ui_friends_component(&mut self, p: &NodeHandle) -> Result<Component>;
    fn ui_autocast(&mut self, p: &NodeHandle) -> Result<Option<usize>>;
    fn ui_auto_spell_id(&mut self, p: &NodeHandle, weapon: &Component, spell: usize)
    -> Result<i32>;
    fn ui_select_auto_spell(&mut self, p: &NodeHandle, weapon: &Component, id: i32) -> Result<()>;
    fn ui_window_mode(&mut self, p: &NodeHandle) -> Result<i32>;
    fn ui_set_window_mode(&mut self, p: &NodeHandle, mode: i32) -> Result<()>;
    fn ui_tutorial_complete(&mut self, p: &NodeHandle) -> Result<bool>;
    fn ui_tutorial_tabs(&mut self, p: &NodeHandle) -> Result<()>;
    /// `getAttribute("runscript", null) != null` and its removal.
    fn ui_has_runscript(&mut self, p: &NodeHandle) -> Result<bool>;
    fn ui_remove_runscript(&mut self, p: &NodeHandle) -> Result<()>;
    fn ui_set_unclosable(&mut self, p: &NodeHandle, value: bool) -> Result<()>;
    fn ui_unclosable(&mut self, p: &NodeHandle) -> Result<bool>;
}

/// The shop's four InterfaceListeners callbacks (Kotlin lambda objects; they
/// capture nothing).
pub struct TemplateButtons;
impl NativeObject for TemplateButtons {}
pub struct SideOpen;
impl NativeObject for SideOpen {}
pub struct TemplateClose;
impl NativeObject for TemplateClose {}
pub struct SideButtons;
impl NativeObject for SideButtons {}

/// Captures of objects that retain a player.
pub(crate) fn node_captures(nodes: Vec<NodeHandle>) -> Captures {
    Captures {
        nodes,
        ..Default::default()
    }
}

/// Kotlin `lowercase()` (ROOT).
pub(crate) fn lowercase(text: &str) -> String {
    rs09_game_core::java_lower::lowercase_root(text)
}
