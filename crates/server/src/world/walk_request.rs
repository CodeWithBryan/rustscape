//! Pinned PacketProcessor.processWalkPacket, including its anonymous arrival
//! pulse. Interface/dialogue/scripts and pulse construction/scheduling are live
//! services of the owning world, rather than inferred from a decoded walk.
use super::doors::Location;
use rs09_protocol::game::actions::ClientAction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Worldspace,
    Interact,
    Minimap,
}
#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub player: Option<usize>,
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    pub running: bool,
}
impl Request {
    pub fn from_action(player: usize, action: &ClientAction) -> Option<Self> {
        let (kind, x, y, running) = match *action {
            ClientAction::WorldspaceWalk {
                dest_x,
                dest_y,
                is_run,
            } => (Kind::Worldspace, dest_x, dest_y, is_run),
            ClientAction::InteractWalk {
                dest_x,
                dest_y,
                is_run,
            } => (Kind::Interact, dest_x, dest_y, is_run),
            ClientAction::MinimapWalk {
                dest_x,
                dest_y,
                is_run,
                ..
            } => (Kind::Minimap, dest_x, dest_y, is_run),
            _ => return None,
        };
        Some(Self {
            player: Some(player),
            kind,
            x,
            y,
            running,
        })
    }
}
#[derive(Clone, Copy, Default)]
pub struct Interfaces {
    pub opened: bool,
    pub opened_walkable: Option<bool>,
    pub chatbox: bool,
    pub chatbox_walkable: Option<bool>,
}
pub const CANCELED: &str = "[WALK ACTION]-- Action canceled. Either player is locked, interfaces can't close, or distance is beyond server pathfinding limit.";
pub trait Context {
    type Pulse;
    fn location(&self, player: usize) -> Result<Option<Location>>;
    fn movement_locked(&mut self, player: usize) -> Result<bool>;
    fn interfaces(&self, player: usize) -> Result<Interfaces>;
    fn close_interface(&mut self, player: usize) -> Result<bool>;
    fn close_chatbox(&mut self, player: usize) -> Result<()>;
    fn close_dialogue(&mut self, player: usize) -> Result<bool>;
    fn debug(&mut self, player: usize, text: &str) -> Result<()>;
    fn clear_minimap(&mut self, player: usize) -> Result<()>;
    fn set_running(&mut self, player: usize, running: bool) -> Result<()>;
    fn face_none(&mut self, player: usize) -> Result<()>;
    fn face_location_none(&mut self, player: usize) -> Result<()>;
    fn reset_scripts(&mut self, player: usize) -> Result<()>;
    fn construct(
        &mut self,
        player: usize,
        location: Location,
        force_run: bool,
    ) -> Result<Self::Pulse>;
    fn run(&mut self, player: usize, pulse: Self::Pulse) -> Result<()>;
    fn max_distance(&self) -> i32;
}
fn location(ctx: &impl Context, player: usize) -> Result<Location> {
    ctx.location(player)?.ok_or(Error::Null)
}
pub fn process(ctx: &mut impl Context, request: Option<Request>) -> Result<()> {
    let Some(r) = request else {
        return Ok(());
    };
    let player = r.player.ok_or(Error::Null)?;
    let z = location(ctx, player)?.coordinates()[2];
    let mut loc = Location::new(r.x, r.y, z);
    let mut can_walk = !ctx.movement_locked(player)?;
    let [x, y, _] = location(ctx, player)?.coordinates();
    let [dx, dy, _] = loc.coordinates();
    let dx = dx.wrapping_sub(x) as f64;
    let dy = dy.wrapping_sub(y) as f64;
    let magnitude = (dx * dx + dy * dy).sqrt();
    if magnitude > ctx.max_distance() as f64 {
        let scale = ctx.max_distance().wrapping_sub(1) as f64;
        let vx = (dx / magnitude * scale).floor() as i32;
        let vy = (dy / magnitude * scale).floor() as i32;
        let [x, y, _] = location(ctx, player)?.coordinates();
        // Location.transform(Vector) adds floored components and uses plane 0.
        loc = Location::new(x.wrapping_add(vx), y.wrapping_add(vy), 0);
    }
    if can_walk {
        let ui = ctx.interfaces(player)?;
        if ui.opened && !ui.opened_walkable.ok_or(Error::Null)? {
            can_walk = ctx.close_interface(player)?;
        }
    }
    if can_walk {
        let ui = ctx.interfaces(player)?;
        if ui.chatbox && !ui.chatbox_walkable.ok_or(Error::Null)? {
            ctx.close_chatbox(player)?;
        }
    }
    if !can_walk || !ctx.close_dialogue(player)? {
        ctx.debug(player, CANCELED)?;
        return ctx.clear_minimap(player);
    }
    if r.kind == Kind::Interact {
        return ctx.set_running(player, r.running);
    }
    ctx.face_none(player)?;
    ctx.face_location_none(player)?;
    ctx.reset_scripts(player)?;
    let pulse = ctx.construct(player, loc, r.running)?;
    ctx.run(player, pulse)
}
/// Anonymous walk pulse captures the packet's run flag, independently of any
/// later mutation to MovementPulse.forceRun. The normal completion returns true.
pub fn arrival(ctx: &mut impl Context, player: usize, captured_running: bool) -> Result<bool> {
    if captured_running {
        ctx.set_running(player, false)?;
    }
    Ok(true)
}
