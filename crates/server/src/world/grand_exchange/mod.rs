//! The Grand Exchange engine: `core.game.ge` (GrandExchange, GrandExchangeOffer,
//! GrandExchangeRecords, GrandExchangeTimer, GEDB, PriceIndex, GEGuidePrice,
//! GEItemSet, OfferState, BotPrices, GEAutoStock) and the interface handlers
//! that drive it (`content.global.handlers.iface.ge`: StockMarket,
//! GrandExchangeInterface, ExchangeItemSets), plus PacketProcessor's
//! `GESetOfferItem` branch, in the original order.
//!
//! The offers and prices live in an SQLite database with the original schema
//! and statements ([`db`]). This is a library: it runs on the native
//! containers, the native InterfaceManager, the native InterfaceListeners
//! registry (typed StockMarket/ExchangeItemSets callbacks), the native input
//! prompt (typed GE input scripts), the native Ironman manager and the native
//! item definitions. Every other outside call is a required [`Host`] method
//! with no default. The original's "GE Update Worker" thread is
//! [`exchange::run_worker`], which the world calls; nothing installs this
//! library in a live world yet. See docs/grand-exchange.md.
pub mod auto_stock;
pub mod bot_prices;
pub mod db;
pub mod exchange;
pub mod guide_price;
pub mod interface;
pub mod item_sets;
pub mod offer;
pub mod price_index;
pub mod records;
pub mod sets_interface;
pub mod stock_market;
pub mod timer;
pub mod ui;

use crate::{
    definitions::item::Registry,
    inventory::{
        container::{Container, Event, Factory},
        container_listener::Handle as Listener,
    },
    player::ironman,
    world::{
        component_objects::{Captures, NativeObject, Plugin},
        drop_tables::LogLevel,
        input_prompt::InputType,
        interface_listeners,
        interfaces::{self, CloseEvent, Handle as Component, Manager},
        player_walk::NodeHandle,
    },
};
pub use db::{Db, Kind, Param};
pub use guide_price::GuideType;
pub use offer::{Offer, OfferState};
pub use records::{OfferRecord, Records};
use rs09_protocol::game::Frame;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

/// Java exception categories the original raises on these paths, plus a
/// failure of a host service. Messages of JVM exceptions are not compared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// NullPointerException (a `!!`, a null receiver, a null cast).
    Null,
    /// NumberFormatException from `toInt()`.
    Number,
    /// ArrayIndexOutOfBoundsException.
    Bounds,
    /// IndexOutOfBoundsException from a Kotlin `List` index.
    ListBounds,
    /// ClassCastException.
    ClassCast,
    /// NoSuchElementException (`ArrayDeque.pop` of an empty deque).
    NoSuchElement,
    /// ArithmeticException (division by zero).
    Arithmetic,
    /// kotlin.UninitializedPropertyAccessException (`GEDB.db` before `init`).
    Uninitialized,
    /// org.json.simple.parser.ParseException.
    Parse,
    /// java.io.FileNotFoundException from `FileReader`.
    File,
    /// java.sql.SQLException from the database; the text is SQLite's.
    Sql(String),
    /// A host service failed; the text is the host's.
    Service(String),
}
pub type Result<T> = std::result::Result<T, Error>;

const TAG: &str = "ge-error:";
impl Error {
    /// Stable code, also used to carry the error through other native layers.
    pub fn code(&self) -> String {
        match self {
            Self::Null => "Null".into(),
            Self::Number => "Number".into(),
            Self::Bounds => "Bounds".into(),
            Self::ListBounds => "ListBounds".into(),
            Self::ClassCast => "ClassCast".into(),
            Self::NoSuchElement => "NoSuchElement".into(),
            Self::Arithmetic => "Arithmetic".into(),
            Self::Uninitialized => "Uninitialized".into(),
            Self::Parse => "Parse".into(),
            Self::File => "File".into(),
            Self::Sql(text) => format!("Sql:{text}"),
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
            "NoSuchElement" => Self::NoSuchElement,
            "Arithmetic" => Self::Arithmetic,
            "Uninitialized" => Self::Uninitialized,
            "Parse" => Self::Parse,
            "File" => Self::File,
            other => match other.strip_prefix("Sql:") {
                Some(text) => Self::Sql(text.into()),
                None => Self::Service(other.strip_prefix("Service:").unwrap_or(other).into()),
            },
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
impl From<ironman::Error> for Error {
    fn from(e: ironman::Error) -> Self {
        match e {
            ironman::Error::Null => Self::Null,
            ironman::Error::Binding(text) => match text.strip_prefix(TAG) {
                Some(code) => Self::from_code(code),
                None => Self::Service(text),
            },
        }
    }
}
impl From<Error> for ironman::Error {
    fn from(e: Error) -> Self {
        match e {
            Error::Null => Self::Null,
            other => Self::Binding(format!("{TAG}{}", other.code())),
        }
    }
}

/// `GrandExchange.isRunning` and the state of its "GE Update Worker" thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Worker {
    /// `boot()` has not run.
    NotStarted,
    /// Started; it waits on `pendingOffers.takeFirst()` between offers.
    Waiting,
    /// An exception left the thread's loop: the thread is gone and offers
    /// stay queued for good.
    Dead,
}

/// The world's Grand Exchange state the original keeps in statics: the
/// database (`GEDB.db`), `GrandExchange.pendingOffers`, the worker,
/// `InterfaceContainer.index` and the PersistPlayer/LoginListener instance
/// the class scan registered. One per world; clones are the same tables.
pub struct Tables {
    pub db: Option<Db>,
    pub pending: VecDeque<Offer>,
    pub worker: Worker,
    /// `InterfaceContainer.index` (600 at start; shared by every user of
    /// InterfaceContainer in the world).
    pub container_index: i32,
    /// The GrandExchangeRecords the class scan constructed (player null):
    /// `parsePlayer`, `savePlayer` and `login` run on it.
    pub hook: Records,
}
#[derive(Clone)]
pub struct Exchange(Arc<Mutex<Tables>>);
impl Exchange {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Tables {
            db: None,
            pending: VecDeque::new(),
            worker: Worker::NotStarted,
            container_index: 600,
            hook: Records::new(None),
        })))
    }
    pub fn tables(&self) -> MutexGuard<'_, Tables> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    /// `GEDB.db`; [`Error::Uninitialized`] before `init`.
    pub fn db(&self) -> Result<Db> {
        self.tables().db.clone().ok_or(Error::Uninitialized)
    }
    pub fn set_db(&self, db: Db) {
        self.tables().db = Some(db);
    }
    pub fn hook(&self) -> Records {
        self.tables().hook.clone()
    }
    pub fn pending(&self) -> Vec<Offer> {
        self.tables().pending.iter().cloned().collect()
    }
    pub fn worker(&self) -> Worker {
        self.tables().worker
    }
}
impl Default for Exchange {
    fn default() -> Self {
        Self::new()
    }
}

/// A pulse the GE classes give `GameWorld.Pulser.submit`; the world runs it
/// with [`run_pulse`] when the pulse is due.
#[derive(Clone)]
pub enum Pulse {
    /// GrandExchangeInterface.handle's `Pulse(1, player)`.
    Interface(interface::ButtonPulse),
    /// GrandExchange.dispatch's `Pulse(5)` when `I_AM_A_CHEATER` is set.
    Cheat { offer: Offer, other: Offer },
}
impl Pulse {
    /// The pulse's delay in ticks.
    pub fn delay(&self) -> i32 {
        match self {
            Self::Interface(_) => 1,
            Self::Cheat { .. } => 5,
        }
    }
}
/// `pulse()` of a submitted pulse: true when it is finished.
pub fn run_pulse<H: Host + ?Sized>(h: &mut H, pulse: &Pulse) -> Result<bool> {
    match pulse {
        Pulse::Interface(button) => interface::run_pulse(h, button),
        Pulse::Cheat { offer, other } => exchange::run_cheat_pulse(h, offer, other),
    }
}

/// The handler a GE input dialogue was given (a Kotlin lambda capturing the
/// player and the temporary offer of the button press).
#[derive(Clone)]
pub struct InputScript {
    pub player: NodeHandle,
    pub offer: Offer,
    /// Button 170's amount (`sendInputDialogue(player, false, ...)`) rather
    /// than 185's price (`InputType.AMOUNT`).
    pub amount: bool,
}
impl NativeObject for InputScript {
    fn captures(&self) -> Captures {
        Captures {
            nodes: vec![self.player.clone()],
            ..Default::default()
        }
    }
}

/// The GE's InterfaceListeners callbacks and its ComponentPlugin (Kotlin
/// lambda objects and the GrandExchangeInterface instance; they capture
/// nothing).
pub struct MarketOpen;
impl NativeObject for MarketOpen {}
pub struct MarketClose;
impl NativeObject for MarketClose {}
pub struct SearchOpen;
impl NativeObject for SearchOpen {}
pub struct SearchClose;
impl NativeObject for SearchClose {}
pub struct SideButtons;
impl NativeObject for SideButtons {}
pub struct MarketButtons;
impl NativeObject for MarketButtons {}
pub struct SetsClose;
impl NativeObject for SetsClose {}
/// The GrandExchangeInterface ComponentPlugin.
pub struct InterfacePlugin;
impl NativeObject for InterfacePlugin {}

/// Every call the ported classes make outside themselves. None has a default;
/// a host must not answer one from state that is not the real system.
pub trait Host {
    // World.
    /// The world's GE tables.
    fn exchange(&mut self) -> Result<Exchange>;
    fn definitions(&mut self) -> Result<Arc<Registry>>;
    /// `ItemDefinition.getDefinitions().values` iteration order (ids); see
    /// [`db::definition_map_order`].
    fn definition_order(&mut self) -> Result<Vec<i32>>;
    /// The world's container factory (its single NULL_ITEM).
    fn factory(&mut self) -> Result<Factory>;
    /// `System.currentTimeMillis()`.
    fn now(&mut self) -> Result<i64>;
    /// `ServerConstants.I_AM_A_CHEATER`.
    fn cheater(&mut self) -> Result<bool>;
    /// `ServerConstants.BOTS_INFLUENCE_PRICE_INDEX`.
    fn bots_influence_price_index(&mut self) -> Result<bool>;
    /// `ServerConstants.BOTSTOCK_LIMIT`.
    fn botstock_limit(&mut self) -> Result<i32>;
    /// `ServerConstants.GE_AUTOSTOCK_ENABLED`.
    fn autostock_enabled(&mut self) -> Result<bool>;
    /// `ServerConstants.GRAND_EXCHANGE_DATA_PATH` (`None` is Java null).
    fn data_path(&mut self) -> Result<Option<String>>;
    /// `FileReader(path)` read whole; [`Error::File`] when it cannot open.
    fn read_file(&mut self, path: &str) -> Result<String>;
    /// `core.api.log(origin, level, message)`.
    fn log(&mut self, level: LogLevel, message: &str) -> Result<()>;
    /// `SystemLogger.logGE(message)`.
    fn log_ge(&mut self, message: &str) -> Result<()>;
    /// Before each statement the original executes on the GE database, with
    /// the JDBC call and the parameters bound at that point. It observes; the
    /// statement itself runs natively.
    fn statement(&mut self, kind: Kind, sql: &str, params: &[Param]) -> Result<()>;
    /// The world's InterfaceListeners registry.
    fn interface_listeners(&mut self) -> Result<interface_listeners::Registry>;
    /// `Thread { ... }.start()` in `boot()`: the "GE Update Worker" whose
    /// loop is [`exchange::run_worker`]; the world decides where it runs.
    fn start_worker(&mut self) -> Result<()>;
    /// `GameWorld.Pulser.submit(pulse)`.
    fn submit_pulse(&mut self, pulse: Pulse) -> Result<()>;
    /// `Repository.sendGrandExchangeNews(text)`.
    fn news(&mut self, text: &str) -> Result<()>;
    /// `Discord.postNewOffer(sell, item, value, amount, username)`.
    fn discord_new_offer(
        &mut self,
        sell: bool,
        item: i32,
        value: i32,
        amount: i32,
        username: &str,
    ) -> Result<()>;
    /// `PlayerDetails.getDetails(username).uid`.
    fn details_uid(&mut self, username: &str) -> Result<i32>;
    /// `Repository.uid_map[uid]`.
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<NodeHandle>>;
    /// `Repository.players.firstOrNull { it.details.accountInfo.uid == uid }`.
    fn player_by_account(&mut self, uid: i32) -> Result<Option<NodeHandle>>;
    /// `ItemDefinition.getExamine()` of the definition of `id`.
    fn examine(&mut self, id: i32) -> Result<String>;
    /// `CS2Mapping.forId(mapping)`: `None` when there is no mapping, else
    /// `getMap().get(key)` (`None` is Java null).
    fn cs2_mapping(&mut self, mapping: i32, key: i32) -> Result<Option<Option<String>>>;
    // Player fields.
    /// `player.details.uid`.
    fn uid(&mut self, p: &NodeHandle) -> Result<i32>;
    fn username(&mut self, p: &NodeHandle) -> Result<String>;
    /// `player.isArtificial`.
    fn artificial(&mut self, p: &NodeHandle) -> Result<bool>;
    fn inventory(&mut self, p: &NodeHandle) -> Result<Container>;
    fn ironman(&mut self, p: &NodeHandle) -> Result<ironman::Manager>;
    fn interface_manager(&mut self, p: &NodeHandle) -> Result<Manager>;
    /// `player.bankPinManager.isUnlocked`.
    fn pin_unlocked(&mut self, p: &NodeHandle) -> Result<bool>;
    /// `player.bankPinManager.openType(kind)`.
    fn pin_open(&mut self, p: &NodeHandle, kind: i32) -> Result<()>;
    // Attributes: getAttribute with a null value answering the default, as
    // GameAttributes does.
    /// "ge-records".
    fn records(&mut self, p: &NodeHandle) -> Result<Option<Records>>;
    fn set_records(&mut self, p: &NodeHandle, records: Records) -> Result<()>;
    /// "ge-temp".
    fn temp_offer(&mut self, p: &NodeHandle) -> Result<Option<Offer>>;
    fn set_temp_offer(&mut self, p: &NodeHandle, offer: Offer) -> Result<()>;
    fn remove_temp_offer(&mut self, p: &NodeHandle) -> Result<()>;
    /// "ge-index".
    fn ge_index(&mut self, p: &NodeHandle, default: i32) -> Result<i32>;
    fn set_ge_index(&mut self, p: &NodeHandle, index: i32) -> Result<()>;
    /// "guide-price".
    fn guide_price(&mut self, p: &NodeHandle) -> Result<Option<GuideType>>;
    fn set_guide_price(&mut self, p: &NodeHandle, kind: GuideType) -> Result<()>;
    /// "container-key".
    fn container_key(&mut self, p: &NodeHandle, default: i32) -> Result<i32>;
    fn set_container_key(&mut self, p: &NodeHandle, key: i32) -> Result<()>;
    fn remove_container_key(&mut self, p: &NodeHandle) -> Result<()>;
    /// "ge-listener" (ExchangeItemSets' inventory listener).
    fn sets_listener(&mut self, p: &NodeHandle) -> Result<Option<Listener>>;
    fn set_sets_listener(&mut self, p: &NodeHandle, listener: Listener) -> Result<()>;
    fn remove_sets_listener(&mut self, p: &NodeHandle) -> Result<()>;
    // Services.
    /// `Player.sendMessage` and `PacketDispatch.sendMessage`.
    fn message(&mut self, p: &NodeHandle, text: &str) -> Result<()>;
    /// `ContentAPI.sendMessage` (it splits at 86 characters itself).
    fn content_message(&mut self, p: &NodeHandle, text: &str) -> Result<()>;
    /// `ContentAPI.notify` (the command bodies).
    fn notify(&mut self, p: &NodeHandle, text: &str) -> Result<()>;
    /// The player's session write of an encoded frame.
    fn send(&mut self, p: &NodeHandle, frame: Frame) -> Result<()>;
    /// `ContentAPI.setVarp(player, id, value)` (not saved).
    fn set_varp(&mut self, p: &NodeHandle, id: i32, value: i32) -> Result<()>;
    /// `ContentAPI.playAudio(player, id)`.
    fn audio(&mut self, p: &NodeHandle, id: i32) -> Result<()>;
    /// `ContentAPI.playJingle(player, id)`.
    fn jingle(&mut self, p: &NodeHandle, id: i32) -> Result<()>;
    /// `sendInputDialogue(player, type, prompt, handler)`; the handler body is
    /// [`stock_market::run_input`].
    fn input_dialogue(
        &mut self,
        p: &NodeHandle,
        kind: InputType,
        prompt: &str,
        script: InputScript,
    ) -> Result<()>;
    /// A container listener that is not ExchangeItemSets'.
    fn listener_update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
    ) -> Result<()>;
    fn listener_refresh(&mut self, listener: Listener, container: &mut Container) -> Result<()>;
    /// An InterfaceListeners callback that is not one of the GE's.
    fn other_listener(
        &mut self,
        p: &NodeHandle,
        callback: interface_listeners::Callback,
        component: &Component,
        button: Option<interface_listeners::Button>,
    ) -> Result<bool>;
    // InterfaceManager services the GE does not own (see ui.rs).
    fn ui_validate(&mut self, p: &NodeHandle, m: &Manager) -> Result<()>;
    fn ui_component(&mut self, p: &NodeHandle, id: i32) -> Result<Component>;
    fn ui_resizable(&mut self, p: &NodeHandle) -> Result<bool>;
    fn ui_log(&mut self, p: &NodeHandle, text: String) -> Result<()>;
    /// `plugin.open(player, component)` of a plugin that is not the GE's.
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

/// The interface components the GE classes use (`org.rs09.consts.Components`).
pub mod components {
    pub const STOCKMARKET_105: i32 = 105;
    pub const STOCKSIDE_107: i32 = 107;
    pub const STOCKCOLLECT_109: i32 = 109;
    pub const OBJDIALOG_389: i32 = 389;
    pub const EXCHANGE_SETS_SIDE_644: i32 = 644;
    pub const EXCHANGE_ITEMSETS_645: i32 = 645;
    pub const GUIDE_PRICE_642: i32 = 642;
    pub const HISTORY_643: i32 = 643;
}
/// The sounds the GE classes play (`org.rs09.consts.Sounds`).
pub mod sounds {
    pub const GE_TRADE_ERROR_4039: i32 = 4039;
    pub const GE_PLACE_ITEM_4043: i32 = 4043;
    pub const GE_TRADE_OK_4044: i32 = 4044;
}
/// The jingle of a collection notice.
pub const JINGLE: i32 = 284;
/// Coins.
pub const COINS: i32 = 995;
