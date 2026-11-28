//! DoorManagingPlugin's option registration, content decision and destination.
//! The owning world supplies its configured default-locale casing, player APIs,
//! native scenery transforms/replacement and the separately ported door rules.
use super::doors::{Error, Handle, Location, Result};

/// Concrete content object in the native option registry. Observation labels
/// never select this program; hosts clone the typed value before invoking it.
#[derive(Clone, Copy, Default)]
pub struct Program;
impl crate::definitions::option_handler::NativeOptionHandler for Program {}
impl Program {
    pub fn is_walk(&self) -> bool {
        true
    }
    pub fn is_walk_for(&self) -> bool {
        false
    }
    pub fn is_delayed(&self) -> bool {
        true
    }
    pub fn handle(
        &self,
        ctx: &mut impl Context,
        player: Option<usize>,
        object: Option<&Handle>,
        option: Option<&str>,
    ) -> Result<bool> {
        handle(ctx, player, object, option)
    }
    pub fn destination(
        &self,
        ctx: &mut impl Context,
        player: Option<usize>,
        object: Option<&Handle>,
    ) -> Result<Option<Location>> {
        destination(ctx, player, object)
    }
}
pub const CLASS: &str = "content.global.handlers.scenery.DoorManagingPlugin";
pub fn native_handler() -> crate::definitions::option_handler::Handle {
    crate::definitions::option_handler::Handler::new(CLASS, Program)
}

pub trait Context {
    type Properties;
    fn register(&mut self, option: &str, handler: usize) -> Result<()>;
    fn player_location(&mut self, player: Option<usize>) -> Result<Option<Location>>;
    /// String.toLowerCase() uses the world's configured JVM-compatible locale.
    fn lowercase(&self, text: &str) -> String;
    fn audio(&mut self, player: Option<usize>, id: i32) -> Result<()>;
    fn animate(&mut self, player: Option<usize>, id: i32, forced: bool) -> Result<()>;
    fn transform(&mut self, object: &Handle, id: i32) -> Result<Handle>;
    fn replace(&mut self, object: &Handle, replacement: Handle, restore: Option<i32>)
    -> Result<()>;
    fn teleport_permitted(&mut self, destination: &Location) -> Result<bool>;
    fn message(&mut self, player: Option<usize>, text: &str) -> Result<()>;
    fn properties(&mut self, player: Option<usize>) -> Result<Option<Self::Properties>>;
    fn set_teleport(&mut self, properties: &Self::Properties, destination: Location) -> Result<()>;
    fn door(&mut self, player: Option<usize>, object: &Handle) -> Result<()>;
    fn door_destination(&mut self, player: Option<usize>, object: &Handle) -> Result<Location>;
}

pub fn new_instance(ctx: &mut impl Context, handler: usize) -> Result<usize> {
    register_options(|option| ctx.register(option, handler))?;
    Ok(handler)
}
pub fn register_options<E>(
    mut register: impl FnMut(&str) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    for option in ["open", "close", "shut", "go-through"] {
        register(option)?;
    }
    Ok(())
}

fn player_location(ctx: &mut impl Context, player: Option<usize>) -> Result<Location> {
    ctx.player_location(player)?.ok_or(Error::Null)
}
fn name(h: &Handle) -> Result<String> {
    h.read().name.clone().ok_or(Error::Null)
}
pub fn handle(
    ctx: &mut impl Context,
    player: Option<usize>,
    object: Option<&Handle>,
    option: Option<&str>,
) -> Result<bool> {
    let object = object.ok_or(Error::Null)?;
    let shape = object.read().shape;
    if shape != 9
        && player_location(ctx, player)?.coordinates() != object.read().location.coordinates()
    {
        // isNextTo reads the player location a second time and ignores planes.
        let [x, y, _] = player_location(ctx, player)?.coordinates();
        let [ox, oy, _] = object.read().location.coordinates();
        let next = if oy == y {
            ox.wrapping_sub(x).wrapping_abs() == 1
        } else if ox == x {
            oy.wrapping_sub(y).wrapping_abs() == 1
        } else {
            false
        };
        if !next && name(object)?.contains("cupboard") {
            return Ok(true);
        }
    }
    let name = ctx.lowercase(&name(object)?);
    if name.contains("drawers") || name.contains("wardrobe") || name.contains("cupboard") {
        match option.ok_or(Error::Null)? {
            "open" | "go-through" => {
                if option == Some("open") {
                    if name.contains("drawers") {
                        ctx.audio(player, 64)?;
                    } else if name.contains("wardrobe") {
                        ctx.animate(player, 545, false)?;
                        ctx.audio(player, 96)?;
                    } else if name.contains("cupboard") {
                        ctx.audio(player, 58)?;
                    }
                }
                if object.read().active {
                    let id = object.read().id.wrapping_add(1);
                    let new = ctx.transform(object, id)?;
                    ctx.replace(object, new, Some(80))?;
                }
            }
            "close" | "shut" => {
                if name.contains("drawers") {
                    ctx.audio(player, 63)?;
                } else if name.contains("wardrobe") {
                    ctx.animate(player, 544, false)?;
                    ctx.audio(player, 95)?;
                } else if name.contains("cupboard") {
                    ctx.audio(player, 57)?;
                }
                let id = object.read().id.wrapping_sub(1);
                let new = ctx.transform(object, id)?;
                ctx.replace(object, new, None)?;
            }
            _ => {}
        }
        return Ok(true);
    }
    if name.contains("trapdoor") || name.contains("trap door") {
        let [x, y, z] = object.read().location.coordinates();
        let destination = Location::new(x, y.wrapping_add(6400), z);
        if !ctx.teleport_permitted(&destination)? {
            ctx.message(player, "This doesn't seem to go anywhere.")?;
            return Ok(true);
        }
        let properties = ctx.properties(player)?.ok_or(Error::Null)?;
        ctx.set_teleport(&properties, destination)?;
        return Ok(true);
    }
    if object.read().id == 25341 {
        return Ok(false);
    }
    if !["door", "gate", "fence", "wall", "exit", "entrance"]
        .iter()
        .any(|word| name.contains(word))
    {
        return Ok(false);
    }
    ctx.door(player, object)?;
    Ok(true)
}

/// The caller has performed the original Player cast. Only wall shapes request
/// DoorActionHandler's custom destination; others fall back to normal scenery.
pub fn destination(
    ctx: &mut impl Context,
    player: Option<usize>,
    object: Option<&Handle>,
) -> Result<Option<Location>> {
    let object = object.ok_or(Error::Null)?;
    let shape = object.read().shape;
    if shape < 4 || shape == 9 {
        Ok(Some(ctx.door_destination(player, object)?))
    } else {
        Ok(None)
    }
}
