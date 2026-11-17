//! Pinned PacketProcessor.processSceneryAction. The owning world supplies object
//! lookup/construction, morph selection and retained interaction/option handles.
//! Listener registries, plugins and the approach/arrival path are separate native
//! services; decoding a click alone must never bypass them and open a door.
use super::doors::{Handle, Location};
use rs09_game_core::direction::Direction;
use rs09_protocol::game::actions::ClientAction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub player: Option<usize>,
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub option: i32,
}
impl Request {
    pub fn from_action(player: usize, action: &ClientAction) -> Option<Self> {
        match *action {
            ClientAction::SceneryAction {
                opt_index,
                id,
                x,
                y,
            } => Some(Self {
                player: Some(player),
                id,
                x,
                y,
                option: opt_index,
            }),
            _ => None,
        }
    }
}

pub trait Context {
    /// Both types must retain the original receiver/option across callbacks.
    type Interaction: Clone;
    type Option: Clone;
    fn player_location(&mut self, player: usize) -> Result<Option<Location>>;
    fn object(&mut self, x: i32, y: i32, z: i32, id: i32, shape: i32) -> Result<Option<Handle>>;
    /// The original two-argument Scenery constructor (type 10, rotation 0).
    fn construct(&mut self, id: i32, location: Location) -> Result<Handle>;
    fn child(&mut self, object: &Handle, player: usize) -> Result<Option<Handle>>;
    fn interaction(&mut self, object: &Handle) -> Result<Option<Self::Interaction>>;
    fn option(
        &mut self,
        interaction: &Self::Interaction,
        index: i32,
    ) -> Result<Option<Self::Option>>;
    fn option_name(option: &Self::Option) -> Option<String>;
    fn option_index(option: &Self::Option) -> i32;
    /// Resolve VarbitDefinition.forObjectID, returning the definition's own ID.
    fn varbit_definition(&mut self, id: i32) -> Result<Option<i32>>;
    fn debug(&mut self, player: usize, text: &str) -> Result<()>;
    /// This always receives the original scenery, even when a morph has no option.
    fn invalid(&mut self, player: usize, object: Option<&Handle>) -> Result<()>;
    fn clear_minimap(&mut self, player: usize) -> Result<()>;
    fn reset_scripts(&mut self, player: usize) -> Result<()>;
    fn listeners(
        &mut self,
        id: i32,
        option: Option<String>,
        player: usize,
        object: &Handle,
    ) -> Result<bool>;
    fn plugin(&mut self, player: usize, object: &Handle) -> Result<bool>;
    fn handle(
        &mut self,
        interaction: &Self::Interaction,
        player: usize,
        option: &Self::Option,
    ) -> Result<()>;
}

fn plane(ctx: &mut impl Context, player: usize) -> Result<i32> {
    Ok(ctx
        .player_location(player)?
        .ok_or(Error::Null)?
        .coordinates()[2])
}
fn direction(d: Direction) -> &'static str {
    match d {
        Direction::North => "NORTH",
        Direction::East => "EAST",
        Direction::South => "SOUTH",
        Direction::West => "WEST",
        Direction::NorthEast => "NORTH_EAST",
        Direction::SouthEast => "SOUTH_EAST",
        Direction::SouthWest => "SOUTH_WEST",
        Direction::NorthWest => "NORTH_WEST",
    }
}
fn definition(h: &Handle) -> Result<crate::definitions::scenery::Handle> {
    h.read().definition.clone().ok_or(Error::Null)
}

pub fn process<C: Context>(ctx: &mut C, request: Request) -> Result<()> {
    let player = request.player.ok_or(Error::Null)?;
    let z = plane(ctx, player)?;
    // Lookup happens even for the hardcoded levers that replace its result.
    let mut scenery = ctx.object(request.x, request.y, z, request.id, -1)?;
    let mut id = request.id;
    if request.id == 6898 {
        scenery = Some(ctx.construct(6898, Location::new(3219, 9618, 0))?);
    }
    if request.id == 6899 {
        scenery = Some(ctx.construct(6899, Location::new(3221, 9618, 0))?);
    }
    if (2421..=2426).contains(&id) && id % 2 == 0 {
        let z = plane(ctx, player)?;
        scenery = Some(ctx.construct(id - 1, Location::new(request.x, request.y, z))?);
        id -= 1;
    }
    if scenery
        .as_ref()
        .is_none_or(|h| h.read().id != id || !h.read().active)
    {
        ctx.debug(player, "[SCENERY INTERACT] NULL OR MISMATCH OR INACTIVE")?;
        ctx.invalid(player, scenery.as_ref())?;
        return ctx.clear_minimap(player);
    }
    let scenery = scenery.ok_or(Error::Null)?;
    let child = ctx.child(&scenery, player)?.ok_or(Error::Null)?;
    let interaction = ctx.interaction(&child)?.ok_or(Error::Null)?;
    let Some(option) = ctx.option(&interaction, request.option)? else {
        ctx.debug(player, "[SCENERY INTERACT] NULL OPTION")?;
        ctx.invalid(player, Some(&scenery))?;
        return ctx.clear_minimap(player);
    };
    let has_wrapper = child.read().id != scenery.read().id;
    ctx.debug(player, "[SCENERY INTERACT]------------------------------")?;
    let id = child.read().id;
    let name = C::option_name(&option).unwrap_or_else(|| "null".into());
    let index = C::option_index(&option);
    ctx.debug(player, &format!("ID: {id}, Option: {name}[{index}]"))?;
    let location = scenery.read().location.clone();
    let dir = scenery.read().direction;
    ctx.debug(player, &format!("Loc: {location}, Dir: {}", direction(dir)))?;
    if has_wrapper {
        let id = scenery.read().id;
        let file = definition(&scenery)?.read().config_file_id;
        let file = if file == -1 {
            None
        } else {
            ctx.varbit_definition(file)?
        };
        let config = match file {
            Some(id) => format!("Varbit: {id}"),
            // The original reads definition again after a null varbit lookup.
            None => format!("Varp: {}", definition(&scenery)?.read().config_id),
        };
        ctx.debug(player, &format!("WrapperID: {id}, {config}"))?;
    }
    ctx.debug(player, "------------------------------------------------")?;
    ctx.reset_scripts(player)?;
    let id = child.read().id;
    let name = C::option_name(&option);
    if ctx.listeners(id, name, player, &child)? || ctx.plugin(player, &child)? {
        return Ok(());
    }
    // Do not reuse the receiver read before option selection. A callback may
    // have replaced the child's interaction while the selected option survives.
    let interaction = ctx.interaction(&child)?.ok_or(Error::Null)?;
    ctx.handle(&interaction, player, &option)
}
