//! `WarningMessages` and its inner `WarningMessage`: the warning screen
//! (component 583) with its config varp 1045, and the per-index messages
//! created on first lookup. As pinned, no message is ever active or toggled
//! and the config value is always 0, so a toggle only tells the player it
//! cannot be done; the unreachable remainder of `toggle` is kept as written.
use super::{Error, Result};

/// `CONFIG`.
pub const CONFIG: i32 = 1045;
/// The warning interface.
pub const COMPONENT: i32 = 583;

/// The `messages` list: each WarningMessage is its index, in creation order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WarningMessages {
    pub messages: Vec<i32>,
}

/// Every call WarningMessages makes outside its class.
pub trait Host {
    type Player;
    /// `getInterfaceManager().open(new Component(id))`.
    fn open_component(&mut self, player: &Self::Player, id: i32) -> Result<()>;
    /// `ContentAPI.setVarp(player, id, value, save)`.
    fn set_varp(&mut self, player: &Self::Player, id: i32, value: i32, save: bool) -> Result<()>;
    /// `getPacketDispatch().sendMessage(text)`.
    fn message(&mut self, player: &Self::Player, text: &str) -> Result<()>;
}

/// `getConfigValue()`.
fn config_value() -> i32 {
    0
}

/// `refresh(player)`.
fn refresh<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    h.set_varp(player, CONFIG, config_value(), true)
}

/// `open(player)`: the interface, then the config.
pub fn open<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    h.open_component(player, COMPONENT)?;
    refresh(h, player)
}

impl WarningMessages {
    /// `getMessage(index)`: the first message with this index, or a new one
    /// appended. Returns its position in the list.
    pub fn message(&mut self, index: i32) -> usize {
        if let Some(position) = self.messages.iter().position(|m| *m == index) {
            return position;
        }
        self.messages.push(index);
        self.messages.len() - 1
    }
}

/// `WarningMessage.isActive()` and `isToggled()`, as pinned.
fn is_active() -> bool {
    false
}
fn is_toggled() -> bool {
    false
}

/// `WarningMessage.toggle(player)`; None is a null player.
pub fn toggle<H: Host>(h: &mut H, player: Option<&H::Player>) -> Result<()> {
    if !is_active() {
        let player = player.ok_or(Error::Null)?;
        h.message(
            player,
            "You cannot toggle the warning screen on or off. You need to go to the area it",
        )?;
        h.message(
            player,
            "is linked to enough times to have the option to do so.",
        )?;
        return Ok(());
    }
    let player = player.ok_or(Error::Null)?;
    let on = !is_toggled();
    refresh(h, player)?;
    h.message(
        player,
        &format!(
            "You have toggled this warning screen {}. You will {}",
            if on { "on" } else { "off" },
            if on {
                "see this interface again."
            } else {
                "no longer see this warning screen."
            }
        ),
    )
}
