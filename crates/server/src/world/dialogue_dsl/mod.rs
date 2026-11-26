//! The dialogue DSLs content is written in: DialogueLabeller (with
//! DialogueOption and the companion `open`), DialogueBuilder/DialogueBuilderFile
//! and their nodes, SkillDialogueHandler with its SkillDialogue types,
//! SkillDialoguePlugin and ActivateEnchantedGem; the ContentAPI helpers they
//! call (`openDialogue`, `face` with a location, `sendInputDialogue`) and
//! `Component.setUnclosable`; and the first consumer, BankerNPC's
//! `BankerDialogueLabellerFile` conversation (with `canAccessBank`).
//!
//! This is a library on top of `world::dialogue`. Everything outside the
//! dialogue package is a required [`Host`] service with no default. No lock
//! is held across a host or content call. See docs/dialogue-dsl.md.
pub mod api;
pub mod banker;
pub mod builder;
pub mod gem;
pub mod labeller;
pub mod skill;

pub use crate::world::dialogue::{
    Arg, Entity, Error, FacialExpression, File, Lines, NodeHandle, Result, Text, text,
};
pub use crate::world::input_prompt::{InputType, Response};

use crate::world::dialogue as d;
use std::sync::Arc;

/// `ChatAnim`, the DialogueLabeller alias of FacialExpression.
pub type ChatAnim = FacialExpression;

/// `DialUtils.splitLines(message)` with the default 54-unit limit, as a new
/// message array.
pub fn split(message: &[u16]) -> Result<Lines> {
    let lines = crate::world::plain_dialogue::split_lines(message, 54)
        .map_err(|e| Error::Binding(e.to_string()))?;
    Ok(Lines::new(lines.into_iter().map(Some).collect()))
}
/// A new message array holding these texts.
pub fn lines(values: &[Text]) -> Lines {
    Lines::new(values.iter().cloned().map(Some).collect())
}
/// UTF-16 texts for string literals.
pub fn texts(values: &[&str]) -> Vec<Text> {
    values.iter().map(|v| text(v)).collect()
}
/// The messages of a shared array, for a Kotlin spread.
pub fn spread(lines: &Lines) -> Lines {
    Lines::new(read_lines(lines))
}
fn read_lines(lines: &Lines) -> Vec<Option<Text>> {
    lines.read().clone()
}

/// The `runscript` attribute a prompt stores: `(value: Any) -> Unit`.
#[allow(clippy::type_complexity)]
pub struct RunScript<H: Host>(Arc<dyn Fn(&mut H, Response) -> Result<()> + Send + Sync>);
impl<H: Host> Clone for RunScript<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> RunScript<H> {
    pub fn new(f: impl Fn(&mut H, Response) -> Result<()> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }
    /// Runs the callback with the value the input packet delivered.
    pub fn invoke(&self, host: &mut H, value: Response) -> Result<()> {
        (self.0)(host, value)
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// An attribute value these classes store on a player.
pub enum Value<H: Host> {
    Bool(bool),
    Int(i32),
    InputType(InputType),
    Script(RunScript<H>),
}

/// A component close event created here. `Unclosable` is the anonymous
/// CloseEvent of `Component.setUnclosable`; [`close_event`] runs its body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseEvent {
    Unclosable,
}
/// `CloseEvent.close(player, c)` for an event created here.
pub fn close_event<H: Host>(host: &mut H, event: CloseEvent, player: &NodeHandle) -> Result<bool> {
    match event {
        CloseEvent::Unclosable => Ok(!host.attribute_bool(player, "close_c_", false)?),
    }
}

type HookBody<H> = dyn Fn(&mut H, &NodeHandle) -> Result<()> + Send + Sync;
/// The `EventHook<DialogueCloseEvent>` DialogueLabeller.afterClose hooks on a
/// player. The host runs [`CloseHook::process`] when that player dispatches a
/// DialogueCloseEvent (Entity.dispatch over a copy of the hook list).
pub struct CloseHook<H: Host>(Arc<HookBody<H>>);
impl<H: Host> Clone for CloseHook<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> CloseHook<H> {
    pub fn new(f: impl Fn(&mut H, &NodeHandle) -> Result<()> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    /// `process(entity, event)`: unhooks itself, then runs the callback.
    pub fn process(&self, host: &mut H, entity: &NodeHandle) -> Result<()> {
        host.unhook(entity, self)?;
        (self.0)(host, entity)
    }
}

/// The `Object dialogue` of ContentAPI.openDialogue.
pub enum Dialogue<H: Host> {
    /// An Integer dialogue key.
    Key(i32),
    File(File<H>),
    Skill(skill::Handler<H>),
    /// Any other object; only its class simple name is read.
    Other(&'static str),
}
impl<H: Host> Clone for Dialogue<H> {
    fn clone(&self) -> Self {
        match self {
            Self::Key(k) => Self::Key(*k),
            Self::File(f) => Self::File(f.clone()),
            Self::Skill(s) => Self::Skill(s.clone()),
            Self::Other(n) => Self::Other(n),
        }
    }
}

/// Everything these classes call outside the dialogue package, beyond
/// `dialogue::Host`. Each method is one original call; none has a default.
pub trait Host: d::Host {
    type Location: Clone + Send + Sync + 'static;

    /// `player.getLocation()`.
    fn location(&mut self, player: &NodeHandle) -> Result<Option<Self::Location>>;
    /// `entity.faceLocation(location)`.
    fn face_location(&mut self, npc: &Self::Npc, location: &Self::Location) -> Result<()>;
    /// `new NPC(id)`.
    fn new_npc(&mut self, id: i32) -> Result<Self::Npc>;
    /// `new Item(id, amount)`.
    fn new_item_amount(&mut self, id: i32, amount: i32) -> Result<Self::Item>;
    /// `player.setAttribute(key, value)`.
    fn set_value(&mut self, player: &NodeHandle, key: &str, value: Value<Self>) -> Result<()>;
    /// `player.getAttribute(key, default)` read as a Boolean (`Cast` for
    /// another stored type).
    fn attribute_bool(&mut self, player: &NodeHandle, key: &str, default: bool) -> Result<bool>;
    /// `component.setCloseEvent(event)`.
    fn set_close_event(&mut self, component: &Self::Component, event: CloseEvent) -> Result<()>;
    /// `player.hook(Event.DialogueClosed, hook)`.
    fn hook_dialogue_closed(&mut self, player: &NodeHandle, hook: CloseHook<Self>) -> Result<()>;
    /// `player.unhook(hook)`.
    fn unhook(&mut self, player: &NodeHandle, hook: &CloseHook<Self>) -> Result<()>;
    /// `player.questRepository.getStage(quest)`.
    fn quest_stage(&mut self, player: &NodeHandle, quest: &str) -> Result<i32>;
    /// `System.out.printf(text)`.
    fn stdout(&mut self, message: Text) -> Result<()>;
    /// `player.debug(message)`.
    fn debug(&mut self, player: &NodeHandle, message: Text) -> Result<()>;
    /// `item.getName()`.
    fn item_name(&mut self, item: &Self::Item) -> Result<Option<Text>>;
    /// `PacketDispatch.sendItemZoomOnInterface(item, zoom, interface, child)`.
    fn send_item_zoom_on_interface(
        &mut self,
        dispatch: Self::Dispatch,
        item: i32,
        zoom: i32,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `PacketRepository.send(RepositionChild.class, new ChildPositionContext(player, ...))`.
    fn send_child_position(
        &mut self,
        player: &NodeHandle,
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<()>;
    /// `player.getInventory().getAmount(item)`.
    fn inventory_amount(&mut self, player: &NodeHandle, item: &Self::Item) -> Result<i32>;
    /// `PacketDispatch.sendMessage(message)`.
    fn dispatch_message(&mut self, dispatch: Self::Dispatch, message: Text) -> Result<()>;
    /// `SlayerManager.getInstance(player).hasStarted()`.
    fn slayer_started(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `ItemDefinition.forId(item).getHandlers().put(key, ActivateEnchantedGem)`.
    fn register_item_option(&mut self, item: i32, key: &str) -> Result<()>;
}
