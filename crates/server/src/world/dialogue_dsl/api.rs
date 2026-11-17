//! The ContentAPI functions and the Component static the DSLs call:
//! `openDialogue`, `sendInputDialogue` (both overloads) and
//! `Component.setUnclosable`. `face(entity, location)` is inlined where
//! DialogueLabeller.open calls it.
use super::{Arg, Dialogue, Error, Host, InputType, NodeHandle, Result, RunScript, Text, Value};
use crate::world::dialogue::{LogLevel, ScriptValue, text};

fn interpreter<H: Host>(
    host: &mut H,
    player: &NodeHandle,
) -> Result<crate::world::dialogue::Interpreter<H>> {
    host.interpreter(player)?.ok_or(Error::Null)
}

/// `openDialogue(player, dialogue, *args)`: closes the player's current
/// dialogue, then opens a key, a file or a skill handler; any other object
/// is logged.
pub fn open_dialogue<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    dialogue: &Dialogue<H>,
    args: &[Arg<H>],
) -> Result<()> {
    interpreter(host, player)?.close(host)?;
    match dialogue {
        Dialogue::Key(key) => {
            interpreter(host, player)?.open(host, *key, args)?;
        }
        Dialogue::File(file) => interpreter(host, player)?.open_file(host, Some(file), args)?,
        Dialogue::Skill(handler) => handler.open(host)?,
        Dialogue::Other(name) => {
            let message = format!("Invalid object type passed to openDialogue() -> {name}");
            host.log("ContentAPI", LogLevel::Err, text(&message))?;
        }
    }
    Ok(())
}

/// `sendInputDialogue(player, numeric, prompt, handler)`.
pub fn send_input_dialogue<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    numeric: bool,
    prompt: Text,
    handler: RunScript<H>,
) -> Result<()> {
    let kind = if numeric {
        InputType::Numeric
    } else {
        InputType::Short
    };
    send_input_dialogue_typed(host, player, kind, prompt, handler)
}

/// `sendInputDialogue(player, type, prompt, handler)`.
pub fn send_input_dialogue_typed<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    kind: InputType,
    prompt: Text,
    handler: RunScript<H>,
) -> Result<()> {
    match kind {
        InputType::Amount => {
            host.set_value(player, "parseamount", Value::Bool(true))?;
            interpreter(host, player)?.send_input(host, true, vec![ScriptValue::Text(prompt)])?;
        }
        InputType::Numeric | InputType::Short => {
            let string = kind != InputType::Numeric;
            interpreter(host, player)?.send_input(host, string, vec![ScriptValue::Text(prompt)])?;
        }
        InputType::Long => {
            interpreter(host, player)?.send_long_input(host, vec![ScriptValue::Text(prompt)])?;
        }
        InputType::Message => interpreter(host, player)?.send_message_input(host, Some(prompt))?,
    }
    host.set_value(player, "runscript", Value::Script(handler))?;
    host.set_value(player, "input-type", Value::InputType(kind))
}

/// `Component.setUnclosable(player, component)`.
pub fn set_unclosable<H: Host>(
    host: &mut H,
    player: Option<&NodeHandle>,
    component: &H::Component,
) -> Result<()> {
    host.set_value(player.ok_or(Error::Null)?, "close_c_", Value::Bool(true))?;
    host.set_close_event(component, super::CloseEvent::Unclosable)
}
