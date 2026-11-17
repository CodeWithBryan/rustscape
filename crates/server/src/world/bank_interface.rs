//! Complete BankInterface decisions with real captured function objects.
//! Dialogue, deferred amount input and bank/world fields are required services.
use super::{
    component_objects::NativeObject,
    interface_listeners::{Button, Callback, Registry},
    interfaces::{Error, Handle, Result},
    player_walk::NodeHandle,
};
use crate::definitions::item::Shared;
use std::sync::OnceLock;
pub type Text = Vec<u16>;
fn text(value: &str) -> Text {
    value.encode_utf16().collect()
}
pub const OPEN_SETTINGS: i32 = 2046 | (1 << 21) | (2 << 18);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Open,
    Main,
    Help,
    Inventory,
}
struct Function {
    _program: Option<Program>,
    kind: Kind,
}
impl NativeObject for Function {}
#[derive(Clone)]
pub struct Program(Shared<()>);
impl Default for Program {
    fn default() -> Self {
        Self(Shared::new(()))
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Dialogue {
    Help,
    Deposit,
}
pub trait Context {
    type Container: Clone;
    type Item: Clone;
    type Definition;
    type Dispatch;
    type Dialogue;
    fn bank_size(&mut self) -> Result<i32>;
    fn bank(&mut self, player: &NodeHandle) -> Result<Self::Container>;
    fn inventory(&mut self, player: &NodeHandle) -> Result<Self::Container>;
    fn item(&mut self, container: Self::Container, slot: i32) -> Result<Option<Self::Item>>;
    fn amount(&mut self, container: Self::Container, item: Self::Item) -> Result<i32>;
    fn amount_id(&mut self, container: Self::Container, id: i32) -> Result<i32>;
    fn last_amount_x(&mut self, bank: Self::Container) -> Result<i32>;
    fn take(&mut self, bank: Self::Container, slot: i32, amount: i32) -> Result<()>;
    fn deposit(&mut self, bank: Self::Container, slot: i32, amount: i32) -> Result<()>;
    fn transfer_x(&mut self, player: &NodeHandle, slot: i32, withdraw: bool) -> Result<()>;
    fn tab_index(&mut self, bank: Self::Container) -> Result<i32>;
    fn set_tab_index(&mut self, bank: Self::Container, index: i32) -> Result<()>;
    fn collapse_tab(&mut self, bank: Self::Container, index: i32) -> Result<()>;
    fn insert(&mut self, bank: Self::Container) -> Result<bool>;
    fn set_insert(&mut self, bank: Self::Container, value: bool) -> Result<()>;
    fn note(&mut self, bank: Self::Container) -> Result<bool>;
    fn set_note(&mut self, bank: Self::Container, value: bool) -> Result<()>;
    fn last_tab(&mut self, player: &NodeHandle) -> Result<i32>;
    fn varc(&mut self, player: &NodeHandle, id: i32, value: i32) -> Result<()>;
    fn search(&mut self, player: &NodeHandle) -> Result<()>;
    fn definition(&mut self, item: Self::Item) -> Result<Self::Definition>;
    fn examine(&mut self, definition: Self::Definition) -> Result<Option<Text>>;
    fn id(&mut self, definition: Self::Definition) -> Result<i32>;
    fn name(&mut self, definition: Self::Definition) -> Result<Option<Text>>;
    fn message(&mut self, player: &NodeHandle, text: Option<Text>) -> Result<()>;
    fn debug(&mut self, player: &NodeHandle, text: Text) -> Result<()>;
    fn dispatch(&mut self, player: &NodeHandle) -> Result<Self::Dispatch>;
    fn settings(&mut self, dispatch: Self::Dispatch, fields: [i32; 5]) -> Result<()>;
    fn create_dialogue(&mut self, kind: Dialogue) -> Result<Self::Dialogue>;
    fn open_dialogue(&mut self, player: &NodeHandle, dialogue: Self::Dialogue) -> Result<()>;
    fn open_bank(&mut self, player: &NodeHandle) -> Result<()>;
}
impl Program {
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn define(&self, registry: &Registry) -> Result<()> {
        let function = |kind| {
            let value = Function {
                _program: Some(self.clone()),
                kind,
            };
            if kind == Kind::Open {
                Callback::open(value)
            } else {
                Callback::button(value)
            }
        };
        registry.add_open(762, function(Kind::Open))?;
        registry.add(762, None, function(Kind::Main))?;
        static HELP: OnceLock<Callback> = OnceLock::new();
        registry.add(
            767,
            None,
            HELP.get_or_init(|| {
                Callback::button(Function {
                    _program: None,
                    kind: Kind::Help,
                })
            })
            .clone(),
        )?;
        registry.add(763, None, function(Kind::Inventory))?;
        Ok(())
    }
    pub fn kind(callback: &Callback) -> Option<Kind> {
        callback.value::<Function>().map(|f| f.kind)
    }
    pub fn invoke<C: Context>(
        callback: Callback,
        ctx: &mut C,
        player: &NodeHandle,
        _component: &Handle,
        button: Button,
    ) -> Result<bool> {
        let function = callback
            .value::<Function>()
            .ok_or_else(|| Error::Binding("not a bank interface callback".into()))?;
        match function.kind {
            Kind::Open => {
                let dispatch = ctx.dispatch(player)?;
                let size = ctx.bank_size()?;
                ctx.settings(dispatch, [OPEN_SETTINGS, 73, 762, 0, size])?;
                reset_search(ctx, player)?;
                Ok(false)
            }
            Kind::Main => {
                match button.button {
                    23 | 18 => {
                        let dialogue = ctx.create_dialogue(if button.button == 23 {
                            Dialogue::Help
                        } else {
                            Dialogue::Deposit
                        })?;
                        ctx.open_dialogue(player, dialogue)?;
                    }
                    14 => {
                        let target = ctx.bank(player)?;
                        let source = ctx.bank(player)?;
                        let value = ctx.insert(source)?;
                        ctx.set_insert(target, !value)?;
                    }
                    16 => {
                        let target = ctx.bank(player)?;
                        let source = ctx.bank(player)?;
                        let value = ctx.note(source)?;
                        ctx.set_note(target, !value)?;
                    }
                    20 => ctx.search(player)?,
                    73 => {
                        menu(ctx, player, button, true)?;
                    }
                    25 | 27 | 29 | 31 | 33 | 35 | 37 | 39 | 41 => {
                        reset_search(ctx, player)?;
                        let index = -(button.button.wrapping_sub(41) / 2);
                        match button.opcode {
                            155 => {
                                let bank = ctx.bank(player)?;
                                if ctx.tab_index(bank)? != index {
                                    let bank = ctx.bank(player)?;
                                    ctx.set_tab_index(bank, index)?;
                                }
                            }
                            196 => {
                                let bank = ctx.bank(player)?;
                                ctx.collapse_tab(bank, index)?;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                Ok(true)
            }
            Kind::Help => {
                if button.button == 10 {
                    ctx.open_bank(player)?;
                }
                Ok(true)
            }
            Kind::Inventory => menu(ctx, player, button, false),
        }
    }
}
fn reset_search(ctx: &mut impl Context, player: &NodeHandle) -> Result<()> {
    let last = ctx.last_tab(player)?;
    let bank = ctx.bank(player)?;
    ctx.set_tab_index(bank, last)?;
    ctx.varc(player, 190, 1)
}
fn menu(
    ctx: &mut impl Context,
    player: &NodeHandle,
    button: Button,
    withdraw: bool,
) -> Result<bool> {
    let container = if withdraw {
        ctx.bank(player)?
    } else {
        ctx.inventory(player)?
    };
    let Some(item) = ctx.item(container, button.slot)? else {
        return Ok(true);
    };
    reset_search(ctx, player)?;
    match button.opcode {
        234 => ctx.transfer_x(player, button.slot, withdraw)?,
        9 => {
            let definition = ctx.definition(item.clone())?;
            let mut examine = ctx.examine(definition)?;
            let definition = ctx.definition(item.clone())?;
            let id = ctx.id(definition)?;
            let container = if withdraw {
                ctx.bank(player)?
            } else {
                ctx.inventory(player)?
            };
            let amount = ctx.amount_id(container.clone(), id)?;
            if id == 995 && amount >= 100000 {
                let amount = ctx.amount_id(container, id)?;
                let mut message = text(&format!("{amount} x "));
                let definition = ctx.definition(item)?;
                message.extend(ctx.name(definition)?.unwrap_or_else(|| text("null")));
                message.push(u16::from(b'.'));
                examine = Some(message);
            }
            ctx.message(player, examine)?;
        }
        155 | 196 | 124 | 199 | 168 | 166 if withdraw || button.opcode != 166 => {
            let bank = ctx.bank(player)?;
            let amount = match button.opcode {
                155 => 1,
                196 => 5,
                124 => 10,
                199 => {
                    let source = ctx.bank(player)?;
                    ctx.last_amount_x(source)?
                }
                _ => {
                    let source = if withdraw {
                        ctx.bank(player)?
                    } else {
                        ctx.inventory(player)?
                    };
                    ctx.amount(source, item)?
                        .wrapping_sub(i32::from(button.opcode == 166))
                }
            };
            if withdraw {
                ctx.take(bank, button.slot, amount)?;
            } else {
                ctx.deposit(bank, button.slot, amount)?;
            }
        }
        opcode => ctx.debug(
            player,
            text(&format!(
                "Unknown {} menu opcode {opcode}",
                if withdraw { "bank" } else { "inventory" }
            )),
        )?,
    }
    Ok(true)
}
