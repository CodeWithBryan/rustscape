//! Native DialogueInterpreter, DialoguePlugin, DialogueFile, EmptyPlugin,
//! Topic/IfTopic, SimpleDialoguePlugin, FacialExpression and DialogueAction.
//!
//! This is a library, not an installed runtime service. Everything the
//! original classes reach outside the dialogue package is a required [`Host`]
//! service: none has a default that fabricates success. Plain message sending
//! and line splitting are the already verified `plain_dialogue` kernel.
//! See docs/dialogue.md for the scope and the remaining parity work.
mod expression;
mod file;
mod interpreter;
mod plugin;
mod substitution;

pub use super::plain_dialogue::{Lines, Text};
pub use super::player_walk::NodeHandle;
pub use expression::FacialExpression;
pub use file::{File, FileBody};
pub use interpreter::{Interpreter, Registry};
pub use plugin::{EmptyPlugin, Plugin, PluginBody, SimpleDialoguePlugin};
pub use substitution::substitute;

use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub const START_DIALOGUE: i32 = 0;
pub const END_DIALOGUE: i32 = 1_000_000;
/// Stage `sendChoices` selects when it shows more than one option.
pub const DIALOGUE_INITIAL_OPTIONS_HANDLE: i32 = 1001;
/// `SimpleDialoguePlugin.getIds()`.
pub const SIMPLE_DIALOGUE: i32 = 70099;

/// The original failure classes these bodies can raise. `Binding` is a host
/// service failure; it is not a Java exception class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// NullPointerException.
    Null,
    /// (Array)IndexOutOfBoundsException.
    Index,
    /// ClassCastException.
    Cast,
    /// NumberFormatException.
    Number,
    /// IllegalArgumentException with its message.
    IllegalArgument(String),
    /// A required host service failed or is unavailable.
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}
pub(crate) fn write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}
pub fn text(value: &str) -> Text {
    value.encode_utf16().collect()
}
/// A one-element `String...` array.
pub fn line(value: &[u16]) -> Lines {
    Lines::new(vec![Some(value.to_vec())])
}

/// The `Entity` argument of the sendDialogues overloads.
pub enum Entity<H: Host> {
    Player(NodeHandle),
    Npc(H::Npc),
}
impl<H: Host> Clone for Entity<H> {
    fn clone(&self) -> Self {
        match self {
            Self::Player(p) => Self::Player(p.clone()),
            Self::Npc(n) => Self::Npc(n.clone()),
        }
    }
}

/// One element of the `Object... args` passed to `open`.
pub enum Arg<H: Host> {
    Null,
    Int(i32),
    Bool(bool),
    Text(Text),
    Npc(H::Npc),
    File(File<H>),
    /// Any other object; only its class simple name is ever read.
    Other(&'static str),
}
impl<H: Host> Clone for Arg<H> {
    fn clone(&self) -> Self {
        match self {
            Self::Null => Self::Null,
            Self::Int(v) => Self::Int(*v),
            Self::Bool(v) => Self::Bool(*v),
            Self::Text(v) => Self::Text(v.clone()),
            Self::Npc(v) => Self::Npc(v.clone()),
            Self::File(v) => Self::File(v.clone()),
            Self::Other(v) => Self::Other(v),
        }
    }
}
impl<H: Host> Arg<H> {
    /// `getClass().getSimpleName()`; `None` is the null reference.
    pub fn simple_name(&self) -> Option<String> {
        Some(match self {
            Self::Null => return None,
            Self::Int(_) => "Integer".into(),
            Self::Bool(_) => "Boolean".into(),
            Self::Text(_) => "String".into(),
            Self::Npc(_) => "NPC".into(),
            Self::File(f) => f.class_name().into(),
            Self::Other(name) => (*name).into(),
        })
    }
}

/// `Topic.toStage`: the original tests it for DialogueFile, then Integer, and
/// ignores every other type.
pub enum Target<H: Host> {
    Stage(i32),
    File(File<H>),
    Other,
}
pub struct TopicData<H: Host> {
    pub expr: FacialExpression,
    pub text: Text,
    pub to_stage: Target<H>,
    pub skip_player: bool,
    /// `Some` makes this an IfTopic carrying that showCondition.
    pub show_condition: Option<bool>,
}
/// Topic/IfTopic handle; clones are the same object.
pub struct Topic<H: Host>(Arc<TopicData<H>>);
impl<H: Host> Clone for Topic<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> std::ops::Deref for Topic<H> {
    type Target = TopicData<H>;
    fn deref(&self) -> &TopicData<H> {
        &self.0
    }
}
impl<H: Host> Topic<H> {
    pub fn new(data: TopicData<H>) -> Self {
        Self(Arc::new(data))
    }
    /// The two-argument constructors: FacialExpression.ASKING, skipPlayer false.
    pub fn asking(text: Text, to_stage: Target<H>) -> Self {
        Self::new(TopicData {
            expr: FacialExpression::Asking,
            text,
            to_stage,
            skip_player: false,
            show_condition: None,
        })
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// DialogueAction: a queued one-shot reward run by the Continue packet.
pub trait DialogueAction<H: Host>: Send + Sync + 'static {
    fn handle(&self, host: &mut H, player: &NodeHandle, button_id: i32) -> Result<()>;
    fn as_any(&self) -> &dyn std::any::Any;
}
pub type Action<H> = Arc<dyn DialogueAction<H>>;
/// The live `getActions()` list; null entries are storable as in the original.
pub type Actions<H> = crate::definitions::item::Shared<Vec<Option<Action<H>>>>;
/// The live public `activeTopics` list.
pub type Topics<H> = crate::definitions::item::Shared<Vec<Topic<H>>>;

/// The object a DialogueOptionSelectionEvent carries.
pub enum Selection<H: Host> {
    Plugin(Plugin<H>),
    File(File<H>),
}
pub enum Event<H: Host> {
    Open(Plugin<H>),
    OptionSelection {
        dialogue: Selection<H>,
        current_stage: i32,
        option_id: i32,
    },
    Close(Plugin<H>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attribute {
    Int(i32),
    Bool(bool),
}
/// One `Object` of a run-script argument list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptValue {
    Null,
    Int(i32),
    Bool(bool),
    Text(Text),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Fine,
    Info,
    Warn,
    Err,
    Debug,
}

/// Everything the dialogue classes call outside their own package. Handle
/// types may themselves stand for a null reference: a service called on one
/// returns `Error::Null`, as the original method call would throw. Each
/// method is one original call, in the original order; getters are separate
/// so late reads and replaced receivers behave as in the reference.
pub trait Host: Sized + 'static {
    type Npc: Clone + Send + Sync + 'static;
    type Item: Clone + Send + Sync + 'static;
    type ItemDefinition;
    type Dispatch;
    type Manager;
    type Component: Clone;
    type WalkingQueue;
    type PulseManager;

    // Player.
    fn interpreter(&mut self, player: &NodeHandle) -> Result<Option<Interpreter<Self>>>;
    fn dispatch(&mut self, player: &NodeHandle) -> Result<Self::Dispatch>;
    fn manager(&mut self, player: &NodeHandle) -> Result<Self::Manager>;
    fn username(&mut self, player: &NodeHandle) -> Result<Option<Text>>;
    fn is_male(&mut self, player: &NodeHandle) -> Result<bool>;
    fn is_debug(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `Player.getId()` through the Entity overloads.
    fn player_id(&mut self, player: &NodeHandle) -> Result<i32>;
    fn send_message(&mut self, player: &NodeHandle, message: Text) -> Result<()>;
    fn set_attribute(&mut self, player: &NodeHandle, key: &str, value: Attribute) -> Result<()>;
    fn dispatch_event(&mut self, player: &NodeHandle, event: Event<Self>) -> Result<()>;

    // PacketDispatch.
    fn send_string(
        &mut self,
        dispatch: Self::Dispatch,
        text: Option<Text>,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn send_interface_config(
        &mut self,
        dispatch: Self::Dispatch,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()>;
    fn send_animation_interface(
        &mut self,
        dispatch: Self::Dispatch,
        animation: i32,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn send_item_on_interface(
        &mut self,
        dispatch: Self::Dispatch,
        item: i32,
        amount: i32,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn send_npc_on_interface(
        &mut self,
        dispatch: Self::Dispatch,
        npc: i32,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn send_player_on_interface(
        &mut self,
        dispatch: Self::Dispatch,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn send_reposition_on_interface(
        &mut self,
        dispatch: Self::Dispatch,
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<()>;
    fn send_angle_on_interface(
        &mut self,
        dispatch: Self::Dispatch,
        interface: i32,
        child: i32,
        zoom: i32,
        pitch: i32,
        yaw: i32,
    ) -> Result<()>;
    fn send_run_script(
        &mut self,
        dispatch: Self::Dispatch,
        id: i32,
        types: &str,
        arguments: Vec<ScriptValue>,
    ) -> Result<()>;

    // InterfaceManager and Component.
    fn open_chatbox(&mut self, manager: Self::Manager, interface: i32) -> Result<()>;
    fn close_chatbox(&mut self, manager: Self::Manager) -> Result<()>;
    fn chatbox(&mut self, manager: Self::Manager) -> Result<Option<Self::Component>>;
    fn component_id(&mut self, component: &Self::Component) -> Result<i32>;
    /// `getCloseEvent() != null`.
    fn has_close_event(&mut self, component: &Self::Component) -> Result<bool>;

    // NPC.
    fn npc_id(&mut self, npc: &Self::Npc) -> Result<i32>;
    fn shown_npc(&mut self, npc: &Self::Npc, player: Option<&NodeHandle>) -> Result<Self::Npc>;
    fn set_dialogue_player(&mut self, npc: &Self::Npc, player: Option<&NodeHandle>) -> Result<()>;
    fn walking_queue(&mut self, npc: &Self::Npc) -> Result<Self::WalkingQueue>;
    fn reset_walking_queue(&mut self, queue: Self::WalkingQueue) -> Result<()>;
    fn pulse_manager(&mut self, npc: &Self::Npc) -> Result<Self::PulseManager>;
    fn clear_pulses(&mut self, manager: Self::PulseManager) -> Result<()>;

    // Definitions, items and world settings.
    /// `NPCDefinition.forId(id).getName()`.
    fn npc_name(&mut self, id: i32) -> Result<Option<Text>>;
    /// `ItemDefinition.forId(id)`.
    fn item_definition(&mut self, id: i32) -> Result<Self::ItemDefinition>;
    fn model_zoom(&mut self, definition: &Self::ItemDefinition) -> Result<i32>;
    fn model_rotation_x(&mut self, definition: &Self::ItemDefinition) -> Result<i32>;
    fn model_rotation_y(&mut self, definition: &Self::ItemDefinition) -> Result<i32>;
    fn definition_name(&mut self, definition: &Self::ItemDefinition) -> Result<Option<Text>>;
    /// The configured destroy message; `None` selects the original default.
    fn destroy_message(&mut self, definition: &Self::ItemDefinition) -> Result<Option<Text>>;
    /// `new Item(id)`.
    fn new_item(&mut self, id: i32) -> Result<Self::Item>;
    fn item_id(&mut self, item: &Self::Item) -> Result<i32>;
    fn item_amount(&mut self, item: &Self::Item) -> Result<i32>;
    /// `GameWorld.getSettings().getName()`.
    fn server_name(&mut self) -> Result<Option<Text>>;

    // Diagnostics the original writes; they never select behaviour.
    fn log(&mut self, origin: &str, level: LogLevel, message: Text) -> Result<()>;
    /// `System.err.println`.
    fn stderr(&mut self, message: Text) -> Result<()>;
    /// `Exception.printStackTrace()` for a caught failure.
    fn stack_trace(&mut self, error: &Error) -> Result<()>;
}
