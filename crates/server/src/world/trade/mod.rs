//! Player-to-player requests and trading: RequestManager, RequestType,
//! TradeModule, TradeContainer (and its TradeListener), TradeCloseEvent and the
//! TradeInterfacePlugin button/offer handler, in the original order.
//!
//! This is a library. It runs on the native containers (`inventory::container`),
//! the native InterfaceManager (`world::interfaces`) with a typed trade close
//! event, the native Ironman manager and a typed amount script for the native
//! input prompt. Every other outside call is a required [`Host`] method with no
//! default. Nothing installs it in a live world yet. See docs/trade.md.
pub mod module;
pub mod plugin;
pub mod request;
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
        component_objects::{Captures, NativeObject},
        interfaces::{self, CloseEvent, Handle as Component, Manager, Plugin},
        player_walk::NodeHandle,
    },
};
use rs09_protocol::game::Frame;
use std::sync::Arc;

pub use module::Module;
pub use plugin::AmountScript;

/// Java exception categories the original raises on these paths, plus a
/// failure of a host service. Messages of JVM exceptions are not compared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// NullPointerException (a `!!`, a null receiver or a missing extension).
    Null,
    /// NumberFormatException from Integer.parseInt.
    Number,
    /// ArrayIndexOutOfBoundsException.
    Bounds,
    /// IndexOutOfBoundsException from an ArrayList.
    ListBounds,
    /// StringIndexOutOfBoundsException.
    StringBounds,
    /// ClassCastException.
    ClassCast,
    /// ConcurrentModificationException.
    Concurrent,
    /// A host service failed; the text is the host's.
    Service(String),
}
pub type Result<T> = std::result::Result<T, Error>;

const TAG: &str = "trade-error:";
impl Error {
    /// Stable code, also used to carry the error through other native layers.
    pub fn code(&self) -> String {
        match self {
            Self::Null => "Null".into(),
            Self::Number => "Number".into(),
            Self::Bounds => "Bounds".into(),
            Self::ListBounds => "ListBounds".into(),
            Self::StringBounds => "StringBounds".into(),
            Self::ClassCast => "ClassCast".into(),
            Self::Concurrent => "Concurrent".into(),
            Self::Service(text) => format!("Service:{text}"),
        }
    }
    fn from_code(code: &str) -> Self {
        match code {
            "Null" => Self::Null,
            "Number" => Self::Number,
            "Bounds" => Self::Bounds,
            "ListBounds" => Self::ListBounds,
            "StringBounds" => Self::StringBounds,
            "ClassCast" => Self::ClassCast,
            "Concurrent" => Self::Concurrent,
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

/// A player as the host knows it. Identity is the original `===`.
pub trait TradePlayer: Clone + PartialEq + Send + Sync + 'static {
    /// The canonical node this handle retains, for ownership tracing.
    fn node(&self) -> Option<NodeHandle>;
}
impl TradePlayer for NodeHandle {
    fn node(&self) -> Option<NodeHandle> {
        Some(self.clone())
    }
}

/// `Location` x, y and plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl Location {
    /// Location.withinDistance(other, dist): same plane and
    /// `sqrt((dx*dx + dy*dy).toDouble()) <= dist` with Int arithmetic that wraps.
    pub fn within_distance(self, other: Location, dist: i32) -> bool {
        if other.z != self.z {
            return false;
        }
        let x = self.x.wrapping_sub(other.x);
        let y = self.y.wrapping_sub(other.y);
        let sum = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y));
        (sum as f64).sqrt() <= dist as f64
    }
}

/// RequestType.TRADE and RequestType.ASSIST. Their identity is the companion
/// object's: one value per kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestType {
    Trade,
    Assist,
}
impl RequestType {
    pub fn message(self) -> &'static str {
        match self {
            Self::Trade => "Sending a trade offer...",
            Self::Assist => "Sending assistance request...",
        }
    }
    pub fn request_message(self) -> &'static str {
        match self {
            Self::Trade => ":tradereq:",
            Self::Assist => ":assistreq:",
        }
    }
    /// RequestType.canRequest: the base body answers true for both kinds.
    pub fn can_request(self) -> bool {
        true
    }
}

/// What `AIRepository.PulseRepository[name]` holds for a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bot {
    /// `botScript is DoublingMoney`.
    pub doubling: bool,
}

/// The three companion components of TradeModule, shared by every trade of a
/// world as in the original (one Component object each).
#[derive(Clone)]
pub struct Components {
    pub overlay: Component,
    pub main: Component,
    pub accept: Component,
}
/// TradeCloseEvent: stateless, one instance on each of 335 and 334.
pub struct TradeClose;
impl NativeObject for TradeClose {}
/// TradeInterfacePlugin, registered on the definitions of 334, 335 and 336.
pub struct TradePlugin;
impl NativeObject for TradePlugin {
    fn captures(&self) -> Captures {
        Captures::default()
    }
}
/// `ComponentDefinition.put(334/335/336, plugin)` as TradeInterfacePlugin.newInstance does.
pub fn register_plugin(registry: &mut interfaces::Registry) -> Plugin {
    let plugin = Plugin::new(TradePlugin);
    for id in [334, 335, 336] {
        registry.for_id(id).write().plugin = Some(plugin.clone());
    }
    plugin
}
/// TradeModule's companion initialisation order: OVERLAY, MAIN, ACCEPT.
pub fn components(registry: &mut interfaces::Registry) -> Components {
    let overlay = registry.component(336);
    let main = registry.component(335);
    main.write().close_event = Some(CloseEvent::new(TradeClose));
    let accept = registry.component(334);
    accept.write().close_event = Some(CloseEvent::new(TradeClose));
    Components {
        overlay,
        main,
        accept,
    }
}

/// Every call the ported classes make outside themselves. None has a
/// default; a host must not answer one from state that is not the real system.
pub trait Host {
    type Player: TradePlayer;
    // World.
    fn ticks(&mut self) -> Result<i32>;
    /// `GameWorld.getSettings().isDevMode()`.
    fn dev_mode(&mut self) -> Result<bool>;
    fn definitions(&mut self) -> Result<Arc<Registry>>;
    /// The world's container factory (its single NULL_ITEM).
    fn factory(&mut self) -> Result<Factory>;
    fn components(&mut self) -> Result<Components>;
    // Player fields.
    fn username(&mut self, p: &Self::Player) -> Result<String>;
    fn location(&mut self, p: &Self::Player) -> Result<Location>;
    fn is_active(&mut self, p: &Self::Player) -> Result<bool>;
    fn is_artificial(&mut self, p: &Self::Player) -> Result<bool>;
    fn inventory(&mut self, p: &Self::Player) -> Result<Container>;
    fn ironman(&mut self, p: &Self::Player) -> Result<ironman::Manager>;
    fn interface_manager(&mut self, p: &Self::Player) -> Result<Manager>;
    fn ip_address(&mut self, p: &Self::Player) -> Result<String>;
    fn mac_address(&mut self, p: &Self::Player) -> Result<String>;
    fn computer_name(&mut self, p: &Self::Player) -> Result<String>;
    // Attributes (`getAttribute(key, default)` / `setAttribute`).
    fn attribute_int(&mut self, p: &Self::Player, key: &'static str, default: i32) -> Result<i32>;
    fn set_attribute_int(&mut self, p: &Self::Player, key: &'static str, value: i32) -> Result<()>;
    fn attribute_bool(
        &mut self,
        p: &Self::Player,
        key: &'static str,
        default: bool,
    ) -> Result<bool>;
    /// `getAttribute<RequestType>("lastRequest", null)`.
    fn last_request(&mut self, p: &Self::Player) -> Result<Option<RequestType>>;
    fn set_last_request(&mut self, p: &Self::Player, kind: RequestType) -> Result<()>;
    /// The `target` field of the player's RequestManager.
    fn request_target(&mut self, p: &Self::Player) -> Result<Option<Self::Player>>;
    fn set_request_target(&mut self, p: &Self::Player, target: Option<Self::Player>) -> Result<()>;
    /// `getExtension(TradeModule.class)`, `addExtension`, `removeExtension`.
    fn trade_module(&mut self, p: &Self::Player) -> Result<Option<Module<Self::Player>>>;
    fn add_trade_module(&mut self, p: &Self::Player, module: Module<Self::Player>) -> Result<()>;
    fn remove_trade_module(&mut self, p: &Self::Player) -> Result<()>;
    // Services.
    /// `PacketDispatch.sendMessage`.
    fn message(&mut self, p: &Self::Player, text: &str) -> Result<()>;
    /// `ContentAPI.sendMessage`.
    fn content_message(&mut self, p: &Self::Player, text: &str) -> Result<()>;
    /// `ContentAPI.setVarp(player, id, value)` with save false.
    fn set_varp(&mut self, p: &Self::Player, id: i32, value: i32) -> Result<()>;
    /// The player's session write of an encoded frame.
    fn send(&mut self, p: &Self::Player, frame: Frame) -> Result<()>;
    /// `DialogueInterpreter.close()`; its result is discarded by every caller here.
    fn dialogue_close(&mut self, p: &Self::Player) -> Result<()>;
    /// `ZoneMonitor.canRequest(type, target)`.
    fn zone_can_request(
        &mut self,
        p: &Self::Player,
        kind: RequestType,
        target: &Self::Player,
    ) -> Result<bool>;
    /// `AIRepository.PulseRepository[name]`.
    fn bot(&mut self, name: &str) -> Result<Option<Bot>>;
    /// `DoublingMoney.tradeReceived(player)` of the named bot.
    fn bot_trade_received(&mut self, bot: &str, from: &Self::Player) -> Result<()>;
    /// `DoublingMoney.itemsReceived(player, container)` of the named bot.
    fn bot_items_received(
        &mut self,
        bot: &str,
        from: &Self::Player,
        items: &Container,
    ) -> Result<()>;
    /// `AssistSession.open(player, target)` (content).
    fn assist_open(&mut self, p: &Self::Player, target: &Self::Player) -> Result<()>;
    /// `PlayerMonitor.logTrade(player, target, pContainer, oContainer)`.
    fn log_trade(
        &mut self,
        p: &Self::Player,
        target: &Self::Player,
        mine: &Container,
        theirs: &Container,
    ) -> Result<()>;
    /// `GroundItemManager.create(item, player)`.
    fn ground(&mut self, item: &Item, p: &Self::Player) -> Result<()>;
    /// `GameWorld.getPulser().submit(new Pulse(1, player) {...})`; the body is
    /// [`module::alert_pulse`].
    fn submit_alert(&mut self, p: &Self::Player, slot: i32) -> Result<()>;
    /// `ContentAPI.sendInputDialogue(player, numeric, prompt, handler)`; the
    /// handler body is [`plugin::run_amount`].
    fn input_dialogue(
        &mut self,
        p: &Self::Player,
        numeric: bool,
        prompt: &str,
        script: AmountScript<Self::Player>,
    ) -> Result<()>;
    /// `item.getDefinition().getExamine()`.
    fn examine(&mut self, item: &Item) -> Result<String>;
    /// A container listener that is not a TradeListener.
    fn listener_update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
    ) -> Result<()>;
    fn listener_refresh(&mut self, listener: Listener, container: &mut Container) -> Result<()>;
    // InterfaceManager services the trade does not own (see ui.rs).
    fn ui_validate(&mut self, p: &Self::Player, m: &Manager) -> Result<()>;
    fn ui_component(&mut self, p: &Self::Player, id: i32) -> Result<Component>;
    fn ui_resizable(&mut self, p: &Self::Player) -> Result<bool>;
    fn ui_log(&mut self, p: &Self::Player, text: String) -> Result<()>;
    fn ui_listener_open(&mut self, p: &Self::Player, c: &Component) -> Result<()>;
    fn ui_listener_close(&mut self, p: &Self::Player, c: &Component) -> Result<bool>;
    fn ui_plugin_open(&mut self, p: &Self::Player, plugin: Plugin, c: &Component) -> Result<()>;
    fn ui_close_event(
        &mut self,
        p: &Self::Player,
        event: CloseEvent,
        c: &Component,
    ) -> Result<bool>;
    fn ui_dispatch_open(&mut self, p: &Self::Player, c: Component) -> Result<()>;
    fn ui_dispatch_close(&mut self, p: &Self::Player, c: Option<Component>) -> Result<()>;
    fn ui_weapon_extension(&mut self, p: &Self::Player) -> Result<Option<Component>>;
    fn ui_create_weapon(&mut self, p: &Self::Player) -> Result<Component>;
    fn ui_add_weapon(&mut self, p: &Self::Player, c: Component) -> Result<()>;
    fn ui_spellbook(&mut self, p: &Self::Player) -> Result<i32>;
    fn ui_has_familiar(&mut self, p: &Self::Player) -> Result<bool>;
    fn ui_friends_component(&mut self, p: &Self::Player) -> Result<Component>;
    fn ui_autocast(&mut self, p: &Self::Player) -> Result<Option<usize>>;
    fn ui_auto_spell_id(
        &mut self,
        p: &Self::Player,
        weapon: &Component,
        spell: usize,
    ) -> Result<i32>;
    fn ui_select_auto_spell(&mut self, p: &Self::Player, weapon: &Component, id: i32)
    -> Result<()>;
    fn ui_window_mode(&mut self, p: &Self::Player) -> Result<i32>;
    fn ui_set_window_mode(&mut self, p: &Self::Player, mode: i32) -> Result<()>;
    fn ui_tutorial_complete(&mut self, p: &Self::Player) -> Result<bool>;
    fn ui_tutorial_tabs(&mut self, p: &Self::Player) -> Result<()>;
    /// `getAttribute("runscript", null) != null` and its removal.
    fn ui_has_runscript(&mut self, p: &Self::Player) -> Result<bool>;
    fn ui_remove_runscript(&mut self, p: &Self::Player) -> Result<()>;
    fn ui_set_unclosable(&mut self, p: &Self::Player, value: bool) -> Result<()>;
    fn ui_unclosable(&mut self, p: &Self::Player) -> Result<bool>;
}

/// Kotlin `lowercase()` (ROOT).
pub(crate) fn lowercase(text: &str) -> String {
    rs09_game_core::java_lower::lowercase_root(text)
}
