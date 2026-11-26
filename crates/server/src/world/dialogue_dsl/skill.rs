//! SkillDialogueHandler, its SkillDialogue types and SkillDialoguePlugin.
//!
//! A handler is opened as dialogue key `SKILL_DIALOGUE` with itself as the
//! only argument ([`Arg::Object`]); the plugin casts it back, displays the
//! type and opens its chatbox. Content overrides `create`, `getAll` and
//! `getName` through [`HandlerBody`].
use super::{Arg, Error, Host, NodeHandle, Response, Result, RunScript, Text, api};
use crate::world::dialogue::{Plugin, PluginBody, text};
use std::{
    any::Any,
    sync::{Arc, Mutex, PoisonError},
};

/// `SkillDialogueHandler.SKILL_DIALOGUE`: `3 shl 16`.
pub const SKILL_DIALOGUE: i32 = 3 << 16;

/// The open members of a SkillDialogueHandler subclass.
pub trait HandlerBody<H: Host>: Send + Sync + 'static {
    /// `create(amount, index)`: the base does nothing.
    fn create(
        &self,
        _host: &mut H,
        _handler: &Handler<H>,
        _amount: i32,
        _index: i32,
    ) -> Result<()> {
        Ok(())
    }
    /// `getAll(index)`.
    fn get_all(&self, host: &mut H, handler: &Handler<H>, index: i32) -> Result<i32> {
        handler.base_get_all(host, index)
    }
    /// `getName(item)`.
    fn get_name(&self, host: &mut H, handler: &Handler<H>, item: &H::Item) -> Result<Text> {
        handler.base_get_name(host, item)
    }
}

struct Inner<H: Host> {
    player: NodeHandle,
    kind: Option<SkillDialogue>,
    items: Vec<H::Item>,
    body: Box<dyn HandlerBody<H>>,
}
/// `SkillDialogueHandler(player, type, vararg items)`; clones are the same
/// object.
pub struct Handler<H: Host>(Arc<Inner<H>>);
impl<H: Host> Clone for Handler<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<H: Host> Handler<H> {
    pub fn new(
        player: NodeHandle,
        kind: Option<SkillDialogue>,
        items: Vec<H::Item>,
        body: impl HandlerBody<H>,
    ) -> Self {
        Self(Arc::new(Inner {
            player,
            kind,
            items,
            body: Box::new(body),
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn player(&self) -> &NodeHandle {
        &self.0.player
    }
    /// `getType()`.
    pub fn kind(&self) -> Option<SkillDialogue> {
        self.0.kind
    }
    pub fn items(&self) -> &[H::Item] {
        &self.0.items
    }
    fn item(&self, index: usize) -> Result<&H::Item> {
        self.0.items.get(index).ok_or(Error::Index)
    }
    /// `open()`: `player.dialogueInterpreter.open(SKILL_DIALOGUE, this)`.
    pub fn open(&self, host: &mut H) -> Result<()> {
        let interpreter = host.interpreter(&self.0.player)?.ok_or(Error::Null)?;
        let me: Arc<dyn Any + Send + Sync> = Arc::new(self.clone());
        interpreter.open(
            host,
            SKILL_DIALOGUE,
            &[Arg::Object("SkillDialogueHandler", me)],
        )?;
        Ok(())
    }
    /// `display()`.
    pub fn display(&self, host: &mut H) -> Result<()> {
        match self.0.kind {
            None => host.debug(&self.0.player, text("Error! Type is null.")),
            Some(kind) => kind.display(host, &self.0.player, self),
        }
    }
    pub fn create(&self, host: &mut H, amount: i32, index: i32) -> Result<()> {
        self.0.body.create(host, self, amount, index)
    }
    pub fn get_all(&self, host: &mut H, index: i32) -> Result<i32> {
        self.0.body.get_all(host, self, index)
    }
    pub fn get_name(&self, host: &mut H, item: &H::Item) -> Result<Text> {
        self.0.body.get_name(host, self, item)
    }
    /// The base `getAll`: `player.inventory.getAmount(items[0])`.
    pub fn base_get_all(&self, host: &mut H, _index: i32) -> Result<i32> {
        let item = self.item(0)?.clone();
        host.inventory_amount(&self.0.player, &item)
    }
    /// The base `getName`: the display form of the name without "Unfired".
    pub fn base_get_name(&self, host: &mut H, item: &H::Item) -> Result<Text> {
        let name = host.item_name(item)?.ok_or(Error::Null)?;
        let name = String::from_utf16_lossy(&name).replace("Unfired", "");
        Ok(text(&crate::world::social::names::format_display_name(
            &name,
        )))
    }
}

/// `"<br><br><br><br>" + name`: Kotlin string concatenation prints null.
fn label(name: Option<Text>) -> Text {
    let mut out = text("<br><br><br><br>");
    out.extend(name.unwrap_or_else(|| text("null")));
    out
}

/// `SkillDialogueHandler.SkillDialogue`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkillDialogue {
    OneOption,
    MakeSetOneOption,
    TwoOption,
    ThreeOption,
    FourOption,
    FiveOption,
}

const FIVE_POSITIONS: [[i32; 2]; 5] = [[10, 30], [117, 10], [217, 20], [317, 15], [408, 15]];

impl SkillDialogue {
    pub const ALL: [Self; 6] = [
        Self::OneOption,
        Self::MakeSetOneOption,
        Self::TwoOption,
        Self::ThreeOption,
        Self::FourOption,
        Self::FiveOption,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::OneOption => "ONE_OPTION",
            Self::MakeSetOneOption => "MAKE_SET_ONE_OPTION",
            Self::TwoOption => "TWO_OPTION",
            Self::ThreeOption => "THREE_OPTION",
            Self::FourOption => "FOUR_OPTION",
            Self::FiveOption => "FIVE_OPTION",
        }
    }
    pub fn value_of(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
    /// `(interfaceId, baseButton, length)`.
    fn data(self) -> (i32, i32, i32) {
        match self {
            Self::OneOption => (309, 5, 1),
            Self::MakeSetOneOption => (582, 4, 1),
            Self::TwoOption => (303, 7, 2),
            Self::ThreeOption => (304, 8, 3),
            Self::FourOption => (305, 9, 4),
            Self::FiveOption => (306, 7, 5),
        }
    }
    pub fn interface_id(self) -> i32 {
        self.data().0
    }
    /// `forLength(length)`: the first type of that length.
    pub fn for_length(length: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.data().2 == length)
    }

    fn zoom<H: Host>(
        host: &mut H,
        player: &NodeHandle,
        item: &H::Item,
        zoom: i32,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        let dispatch = host.dispatch(player)?;
        let id = host.item_id(item)?;
        host.send_item_zoom_on_interface(dispatch, id, zoom, iface, child)
    }
    /// The base `display`: does nothing (every type overrides it).
    pub fn display<H: Host>(
        self,
        host: &mut H,
        player: &NodeHandle,
        handler: &Handler<H>,
    ) -> Result<()> {
        match self {
            Self::OneOption => {
                let item = handler.item(0)?.clone();
                let dispatch = host.dispatch(player)?;
                let name = host.item_name(&item)?;
                host.send_string(dispatch, Some(label(name)), 309, 6)?;
                Self::zoom(host, player, &item, 160, 309, 2)?;
                host.send_child_position(player, 309, 6, 60, 20)?;
                host.send_child_position(player, 309, 2, 210, 30)
            }
            Self::MakeSetOneOption => {
                let item = handler.item(0)?.clone();
                Self::zoom(host, player, &item, 160, 582, 2)?;
                let dispatch = host.dispatch(player)?;
                let name = host.item_name(&item)?;
                host.send_string(dispatch, Some(label(name)), 582, 5)?;
                for (child, x, y) in [
                    (0, 12, 15),
                    (1, 431, 15),
                    (6, 0, 12),
                    (2, 207, 23),
                    (3, 58, 27),
                    (4, 58, 27),
                    (5, 58, 27),
                ] {
                    host.send_child_position(player, 582, child, x, y)?;
                }
                Ok(())
            }
            Self::TwoOption => {
                let manager = host.manager(player)?;
                host.open_chatbox(manager, 306)?;
                for i in 0..handler.items().len() {
                    let item = handler.item(i)?.clone();
                    let dispatch = host.dispatch(player)?;
                    let name = label(Some(handler.get_name(host, &item)?));
                    host.send_string(dispatch, Some(name), 303, 7 + i as i32)?;
                    Self::zoom(host, player, &item, 160, 303, 2 + i as i32)?;
                }
                Ok(())
            }
            Self::ThreeOption | Self::FourOption => {
                let (iface, count) = if self == Self::ThreeOption {
                    (304, 3)
                } else {
                    (305, 4)
                };
                for i in 0..count {
                    let item = handler.item(i)?.clone();
                    Self::zoom(host, player, &item, 135, iface, 2 + i as i32)?;
                    let dispatch = host.dispatch(player)?;
                    let name = host.item_name(&item)?;
                    host.send_string(
                        dispatch,
                        Some(label(name)),
                        iface,
                        iface - 296 + i as i32 * 4,
                    )?;
                }
                Ok(())
            }
            Self::FiveOption => {
                let manager = host.manager(player)?;
                host.open_chatbox(manager, 306)?;
                for i in 0..handler.items().len() {
                    let item = handler.item(i)?.clone();
                    let dispatch = host.dispatch(player)?;
                    let name = label(Some(handler.get_name(host, &item)?));
                    host.send_string(dispatch, Some(name), 306, 10 + 4 * i as i32)?;
                    Self::zoom(host, player, &item, 160, 306, 2 + i as i32)?;
                    let position = FIVE_POSITIONS.get(i).ok_or(Error::Index)?;
                    host.send_child_position(player, 306, 2 + i as i32, position[0], position[1])?;
                }
                Ok(())
            }
        }
    }

    /// `getAmount(handler, buttonId)`; -1 asks for an amount.
    pub fn get_amount<H: Host>(
        self,
        host: &mut H,
        handler: &Handler<H>,
        button: i32,
    ) -> Result<i32> {
        Ok(match self {
            Self::OneOption => match button {
                5 => 1,
                4 => 5,
                3 => -1,
                _ => {
                    let index = self.get_index(button);
                    return handler.get_all(host, index);
                }
            },
            Self::MakeSetOneOption => match button {
                5 | 4 => 1,
                3 => 5,
                _ => 10,
            },
            Self::TwoOption => match button {
                6 | 10 => 1,
                5 | 9 => 5,
                4 | 8 => 10,
                3 | 7 => -1,
                _ => 1,
            },
            Self::ThreeOption => match button {
                7 | 11 | 15 => 1,
                6 | 10 | 14 => 5,
                5 | 9 | 13 => 10,
                4 | 8 | 12 => -1,
                _ => 1,
            },
            Self::FourOption => match button {
                8 | 12 | 16 | 20 => 1,
                7 | 11 | 15 | 19 => 5,
                6 | 10 | 14 | 18 => 10,
                5 | 9 | 13 | 17 => -1,
                _ => 1,
            },
            Self::FiveOption => match button {
                9 | 13 | 17 | 21 | 25 => 1,
                8 | 12 | 16 | 20 | 24 => 5,
                7 | 11 | 15 | 19 | 23 => 10,
                6 | 10 | 14 | 18 | 22 => -1,
                _ => 1,
            },
        })
    }

    /// `getIndex(handler, buttonId)`.
    pub fn get_index(self, button: i32) -> i32 {
        match self {
            Self::OneOption | Self::MakeSetOneOption => {
                let (_, base, length) = self.data();
                let mut index = 0;
                for k in 0..=3 {
                    for i in 1..length {
                        let value = base + k + 4 * i;
                        if value == button {
                            return index + 1;
                        } else if value <= button {
                            index += 1;
                        }
                    }
                    index = 0;
                }
                index
            }
            Self::TwoOption => match button {
                3..=6 => 0,
                _ => 1,
            },
            Self::ThreeOption => match button {
                4..=7 => 0,
                8..=11 => 1,
                12..=15 => 2,
                _ => 1,
            },
            Self::FourOption => match button {
                5..=8 => 0,
                9..=12 => 1,
                13..=16 => 2,
                17..=20 => 3,
                _ => 0,
            },
            Self::FiveOption => match button {
                6..=9 => 0,
                10..=13 => 1,
                14..=17 => 2,
                18..=21 => 3,
                22..=25 => 4,
                _ => 0,
            },
        }
    }
}

/// `SkillDialoguePlugin`.
pub struct SkillDialoguePlugin<H: Host> {
    handler: Mutex<Option<Handler<H>>>,
}
impl<H: Host> SkillDialoguePlugin<H> {
    /// The registered prototype (the no-argument constructor).
    pub fn prototype() -> Plugin<H> {
        Plugin::prototype(Self {
            handler: Mutex::new(None),
        })
    }
    fn handler(&self) -> Option<Handler<H>> {
        self.handler
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}
impl<H: Host> PluginBody<H> for SkillDialoguePlugin<H> {
    fn class_name(&self) -> &str {
        "SkillDialoguePlugin"
    }
    fn ids(&self, _host: &mut H, _this: &Plugin<H>) -> Result<Vec<i32>> {
        Ok(vec![SKILL_DIALOGUE])
    }
    fn new_instance(
        &self,
        host: &mut H,
        _this: &Plugin<H>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Plugin<H>>> {
        Plugin::new(
            host,
            player,
            Self {
                handler: Mutex::new(None),
            },
        )
        .map(Some)
    }
    /// `open(args)`: casts the handler, displays it, opens its chatbox.
    fn open(&self, host: &mut H, this: &Plugin<H>, args: &[Arg<H>]) -> Result<bool> {
        let handler = match args.first().ok_or(Error::Index)? {
            Arg::Object(_, object) => object
                .downcast_ref::<Handler<H>>()
                .cloned()
                .ok_or(Error::Cast)?,
            // `(SkillDialogueHandler) null` succeeds; `handler.display()` fails.
            Arg::Null => {
                *self.handler.lock().unwrap_or_else(PoisonError::into_inner) = None;
                return Err(Error::Null);
            }
            _ => return Err(Error::Cast),
        };
        *self.handler.lock().unwrap_or_else(PoisonError::into_inner) = Some(handler.clone());
        handler.display(host)?;
        let player = this.player().ok_or(Error::Null)?;
        let manager = host.manager(&player)?;
        let kind = handler.kind().ok_or(Error::Null)?;
        host.open_chatbox(manager, kind.interface_id())?;
        Ok(true)
    }
    /// `handle(interfaceId, buttonId)`: amount and index, `end()`, then
    /// `create` or an amount prompt whose answer creates.
    fn handle(&self, host: &mut H, this: &Plugin<H>, _interface: i32, button: i32) -> Result<bool> {
        let handler = self.handler().ok_or(Error::Null)?;
        let amount = handler
            .kind()
            .ok_or(Error::Null)?
            .get_amount(host, &handler, button)?;
        let index = handler.kind().ok_or(Error::Null)?.get_index(button);
        this.end(host)?;
        if amount != -1 {
            handler.create(host, amount, index)?;
        } else {
            let player = this.player().ok_or(Error::Null)?;
            let creator = handler.clone();
            let script = RunScript::new(move |host: &mut H, value: Response| {
                let amount = match value {
                    Response::Text(t) => String::from_utf16(&t)
                        .ok()
                        .and_then(|s| rs09_game_core::java_string::parse_i32(&s))
                        .ok_or(Error::Number)?,
                    Response::Int(v) => v,
                };
                creator.create(host, amount, index)
            });
            api::send_input_dialogue(host, &player, true, text("Enter the amount:"), script)?;
        }
        Ok(true)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
