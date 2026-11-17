//! TradeInterfacePlugin.handle (content/global/handlers/iface) and the two
//! Offer-X/Remove-X handler lambdas it gives ContentAPI.sendInputDialogue.
use super::{Error, Host, Module, Result, TradePlayer, module};
use crate::world::{
    component_objects::{Captures, NativeObject},
    input_prompt::Response,
    interfaces::Handle as Component,
};

/// The `(value) -> { ... }` lambda: it captures the module found when the
/// button was pressed and the slot, not the player's later module.
pub struct AmountScript<P> {
    pub module: Module<P>,
    pub slot: i32,
    /// Remove-X on 335 (k/K/m/M expanded) rather than Offer-X on 336 (k/K only).
    pub withdraw: bool,
}
impl<P> Clone for AmountScript<P> {
    fn clone(&self) -> Self {
        Self {
            module: self.module.clone(),
            slot: self.slot,
            withdraw: self.withdraw,
        }
    }
}
impl<P: TradePlayer> NativeObject for AmountScript<P> {
    fn captures(&self) -> Captures {
        Captures {
            nodes: self.module.retained_nodes(),
            ..Default::default()
        }
    }
}

/// `TradeInterfacePlugin.handle(player, component, opcode, button, slot, itemId)`.
pub fn handle<H: Host>(
    h: &mut H,
    player: &H::Player,
    component: &Component,
    opcode: i32,
    button: i32,
    slot: i32,
    _item: i32,
) -> Result<bool> {
    let Some(module) = module::extension(h, Some(player))? else {
        return Ok(true);
    };
    let id = component.read().id;
    match id {
        334 => match button {
            20 => module::set_accepted(h, &module, true, true)?,
            21 => module::decline(h, &module)?,
            _ => {}
        },
        335 => match opcode {
            155 => match button {
                16 => module::set_accepted(h, &module, true, true)?,
                18 => module::decline(h, &module)?,
                30 => module::withdraw(h, &module, slot, 1)?,
                _ => {}
            },
            196 => module::withdraw(h, &module, slot, 5)?,
            124 => module::withdraw(h, &module, slot, 10)?,
            199 => {
                let container = module.container().ok_or(Error::Null)?;
                let amount = container.amount(container.get(slot).as_ref());
                module::withdraw(h, &module, slot, amount)?;
            }
            234 => {
                let script = AmountScript {
                    module: module.clone(),
                    slot,
                    withdraw: true,
                };
                h.input_dialogue(player, false, "Enter the amount:", script)?;
            }
            9 => {
                let owner = if button == 32 {
                    module.target()
                } else {
                    Some(player.clone())
                };
                if module::extension(h, owner.as_ref())?.is_none() {
                    return Ok(true);
                }
                let shown = module::extension(h, owner.as_ref())?.ok_or(Error::Null)?;
                let item = shown
                    .container()
                    .ok_or(Error::Null)?
                    .get(slot)
                    .ok_or(Error::Null)?;
                let text = h.examine(&item)?;
                h.message(player, &text)?;
            }
            _ => {}
        },
        336 => match opcode {
            155 => module::offer(h, &module, slot, 1)?,
            196 => module::offer(h, &module, slot, 5)?,
            124 => module::offer(h, &module, slot, 10)?,
            199 => {
                let inventory = h.inventory(player)?;
                let amount = inventory.amount(inventory.get(slot).as_ref());
                module::offer(h, &module, slot, amount)?;
            }
            234 => {
                let script = AmountScript {
                    module: module.clone(),
                    slot,
                    withdraw: false,
                };
                h.input_dialogue(player, false, "Enter the amount:", script)?;
            }
            9 => {
                let item = h.inventory(player)?.get(slot).ok_or(Error::Null)?;
                let text = h.examine(&item)?;
                h.message(player, &text)?;
            }
            _ => {}
        },
        _ => {}
    }
    Ok(true)
}

/// `value.toString()`.
fn string(value: &Response) -> String {
    match value {
        Response::Int(n) => n.to_string(),
        Response::Text(text) => String::from_utf16_lossy(text),
    }
}

/// The handler body: expand the suffixes, `Integer.parseInt`, then withdraw or
/// offer on the captured module's container. A parse failure is
/// [`Error::Number`], which the input prompt turns into its dialogue.
pub fn run_amount<H: Host>(
    h: &mut H,
    script: &AmountScript<H::Player>,
    value: &Response,
) -> Result<()> {
    let mut s = string(value);
    // eplace("k","000").replace("K","000"): one pass gives the same text.
    s = s.replace(['k', 'K'], "000");
    if script.withdraw {
        s = s.replace(['m', 'M'], "000000");
    }
    let amount = rs09_game_core::java_string::parse_i32(&s).ok_or(Error::Number)?;
    if script.withdraw {
        module::withdraw(h, &script.module, script.slot, amount)
    } else {
        module::offer(h, &script.module, script.slot, amount)
    }
}
