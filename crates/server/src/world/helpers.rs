//! Small original members the live world needs, each with its own host
//! trait: `AttackListener` (the NPC "attack" option: `defineListeners` and
//! the handler body), `Player.toggleDebug`/`debug`/`isDebug`, and
//! `GraveController.hasGraveAt`.
//!
//! Every outside call is a required host service; no guard spans one.
//! Differentially tested by helpers-oracle (docs/helpers.md).
use super::doors::Location;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A `NullPointerException` of the original.
    Null,
    /// A `ClassCastException` of the original.
    Cast,
    /// A failure of a host service.
    Binding(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

// ------------------------------------------------------------ AttackListener

/// `CombatStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Melee,
    Range,
    Magic,
}

/// The class name `flagInstant()` registers.
pub const ATTACK_LISTENER: &str = "content.global.skill.AttackListener";
/// `NPCs.MAGIC_DUMMY_4474`.
pub const MAGIC_DUMMY: i32 = 4474;
/// `NPCs.MELEE_DUMMY_7891`.
pub const MELEE_DUMMY: i32 = 7891;
/// `NPCs.KILLERWATT_3201` and `NPCs.KILLERWATT_3202`.
pub const KILLERWATTS: [i32; 2] = [3201, 3202];

/// What `AttackListener.defineListeners()` registers with.
pub trait Listeners {
    /// `flagInstant()`: the class name joins `instantClasses`.
    fn flag_instant(&mut self, class: &str) -> Result<()>;
    /// `on(IntType.NPC, "attack") { player, npc -> ... }`; the handler is
    /// [`attack`].
    fn on_npc(&mut self, options: &[&str]) -> Result<()>;
}

/// `AttackListener.defineListeners()`.
pub fn define_listeners(listeners: &mut impl Listeners) -> Result<()> {
    listeners.flag_instant(ATTACK_LISTENER)?;
    listeners.on_npc(&["attack"])
}

pub trait AttackHost {
    type Player;
    type Node;
    /// `npc.id` (`Node.getId()`).
    fn id(&self, node: &Self::Node) -> i32;
    /// `npc as NPC` succeeds.
    fn is_npc(&self, node: &Self::Node) -> bool;
    /// `player.getSwingHandler(false).type` (a handler without a type
    /// answers `None`; a null handler is `Error::Null`).
    fn swing_handler_type(&mut self, player: &Self::Player) -> Result<Option<Style>>;
    /// ContentAPI `sendMessage(player, message)`.
    fn send_message(&mut self, player: &Self::Player, message: &str) -> Result<()>;
    /// `KillerwattBehavior.aggressUponKillerwatt(player, npc)`.
    fn aggress_upon_killerwatt(&mut self, player: &Self::Player, npc: &Self::Node) -> Result<()>;
    /// `player.attack(npc)` (`getProperties().getCombatPulse().attack(node)`).
    fn attack(&mut self, player: &Self::Player, npc: &Self::Node) -> Result<()>;
}

/// The handler of `on(IntType.NPC, "attack")`: the Lumbridge dummies refuse
/// a swing handler of the wrong type (the handler is asked only for a
/// dummy), a killerwatt is provoked first, then the player attacks. Always
/// handled.
pub fn attack<H: AttackHost>(h: &mut H, player: &H::Player, npc: &H::Node) -> Result<bool> {
    if h.id(npc) == MAGIC_DUMMY && h.swing_handler_type(player)? != Some(Style::Magic) {
        h.send_message(player, "You can only attack this with magic.")?;
        return Ok(true);
    }
    if h.id(npc) == MELEE_DUMMY && h.swing_handler_type(player)? != Some(Style::Melee) {
        h.send_message(player, "You must use the training sword to attack this.")?;
        return Ok(true);
    }
    if h.id(npc) == KILLERWATTS[0] || h.id(npc) == KILLERWATTS[1] {
        if !h.is_npc(npc) {
            return Err(Error::Cast);
        }
        h.aggress_upon_killerwatt(player, npc)?;
    }
    h.attack(player, npc)?;
    Ok(true)
}

// ------------------------------------------------------------- Player debug

/// What `getAttribute("debug", false)` finds stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stored {
    Bool(bool),
    /// A value that is not a `Boolean`.
    Other,
}

pub trait DebugHost {
    type Player;
    /// The object stored under `key` (`getAttribute`'s lookup; `None` when
    /// absent, in which case the caller's default answers).
    fn attribute(&mut self, player: &Self::Player, key: &str) -> Result<Option<Stored>>;
    /// `Entity.setAttribute(key, value)`.
    fn set_attribute(&mut self, player: &Self::Player, key: &str, value: bool) -> Result<()>;
    /// `getPacketDispatch().sendMessage(text)`.
    fn send_message(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    /// `details.getRights() == Rights.ADMINISTRATOR`.
    fn is_administrator(&mut self, player: &Self::Player) -> Result<bool>;
}

/// `getAttribute("debug", false)` as a `boolean`.
fn debug_attribute<H: DebugHost>(h: &mut H, player: &H::Player) -> Result<bool> {
    match h.attribute(player, "debug")? {
        None => Ok(false),
        Some(Stored::Bool(value)) => Ok(value),
        Some(Stored::Other) => Err(Error::Cast),
    }
}

/// `Player.toggleDebug()`.
pub fn toggle_debug<H: DebugHost>(h: &mut H, player: &H::Player) -> Result<()> {
    let debug = debug_attribute(h, player)?;
    h.set_attribute(player, "debug", !debug)?;
    h.send_message(
        player,
        &format!("Your debug mode is toggled to {}.", !debug),
    )
}

/// `Player.debug(string)`.
pub fn debug<H: DebugHost>(h: &mut H, player: &H::Player, text: &str) -> Result<()> {
    if debug_attribute(h, player)? {
        h.send_message(player, text)?;
    }
    Ok(())
}

/// `Player.isDebug()`: the attribute is read only for an administrator.
pub fn is_debug<H: DebugHost>(h: &mut H, player: &H::Player) -> Result<bool> {
    Ok(h.is_administrator(player)? && debug_attribute(h, player)?)
}

// ------------------------------------------------------------ GraveController

/// `GraveController.hasGraveAt(loc)`: any active grave whose location equals
/// `loc` (`Location.equals`: the packed coordinates). `graves` are the
/// `location` fields of `activeGraves.values`; a null `loc` fails Kotlin's
/// parameter check.
pub fn has_grave_at(
    graves: impl IntoIterator<Item = Option<Location>>,
    loc: Option<&Location>,
) -> Result<bool> {
    let loc = loc.ok_or(Error::Null)?;
    let wanted = loc.coordinates();
    Ok(graves
        .into_iter()
        .any(|grave| grave.is_some_and(|l| l.coordinates() == wanted)))
}
