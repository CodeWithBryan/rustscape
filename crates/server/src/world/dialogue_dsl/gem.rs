//! ActivateEnchantedGem: the enchanted gem's "activate" option handler.
//! The slayer master conversation it opens (key 77777) is content.
use super::{Error, Host, NodeHandle, Result};
use crate::world::dialogue::text;

/// `Items.ENCHANTED_GEM_4155`.
pub const ENCHANTED_GEM: i32 = 4155;
/// The slayer conversation's dialogue key.
pub const SLAYER_DIALOGUE: i32 = 77777;

/// `newInstance(arg)`: registers itself as the gem's `option:activate`.
pub fn new_instance<H: Host>(host: &mut H) -> Result<()> {
    host.register_item_option(ENCHANTED_GEM, "option:activate")
}

/// `isWalk()`.
pub const fn is_walk() -> bool {
    false
}

/// `handle(player, node, option)`: always true.
pub fn handle<H: Host>(host: &mut H, player: &NodeHandle) -> Result<bool> {
    if !host.slayer_started(player)? {
        let dispatch = host.dispatch(player)?;
        host.dispatch_message(dispatch, text("You try to activate the gem..."))?;
        return Ok(true);
    }
    let interpreter = host.interpreter(player)?.ok_or(Error::Null)?;
    interpreter.open(host, SLAYER_DIALOGUE, &[])?;
    Ok(true)
}
