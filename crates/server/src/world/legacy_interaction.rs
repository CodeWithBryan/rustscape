//! InteractPlugin.handle, invalid/walk/default orchestration and captured pulse
//! bodies. Managers, constructors, locks, zones, facing, plugins and logging are
//! owning-world services. Option initialization and item interactions are separate.
use super::{
    doors::Location,
    interaction::{Approach, ApproachKind, OptionHandle},
    player_walk::Node,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Binding(Option<String>),
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
    Walk,
    Invalid,
    Delayed,
}
#[derive(Clone)]
pub struct Task {
    pub player: Option<Node>,
    pub target: Option<Node>,
    pub option: Option<OptionHandle>,
    pub kind: Kind,
}
impl Task {
    pub fn class_name(&self) -> &'static str {
        match self.kind {
            Kind::Walk => "core.game.interaction.InteractPlugin$3",
            Kind::Invalid => "core.game.interaction.InteractPlugin$2",
            Kind::Delayed => "core.game.interaction.InteractPlugin$4",
        }
    }
}
impl From<&Approach> for Task {
    fn from(a: &Approach) -> Self {
        Self {
            player: Some(a.player),
            target: a.target,
            option: a.option.clone(),
            kind: match a.kind {
                ApproachKind::Option => Kind::Walk,
                ApproachKind::Invalid => Kind::Invalid,
            },
        }
    }
}
pub trait Context {
    type Manager: Clone;
    type Pulse;
    fn class_name(&self) -> String {
        "core.game.interaction.InteractPlugin".into()
    }
    fn interaction_locked(&mut self, player: Option<Node>) -> Result<bool>;
    fn movement_locked(&mut self, player: Option<Node>) -> Result<bool>;
    fn location(&mut self, node: Option<Node>) -> Result<Option<Location>>;
    fn debug(&mut self, player: Option<Node>, text: &str) -> Result<()>;
    fn handler_walk(&mut self, handler: usize) -> Result<bool>;
    fn handler_walk_for(
        &mut self,
        handler: usize,
        player: Option<Node>,
        target: Option<Node>,
    ) -> Result<bool>;
    fn handler_delayed(&mut self, handler: usize, player: Option<Node>) -> Result<bool>;
    fn handler_name(&mut self, handler: usize) -> Result<String>;
    fn handler_handle(
        &mut self,
        handler: usize,
        player: Option<Node>,
        target: Option<Node>,
        option: &str,
    ) -> Result<bool>;
    /// The default-locale String.toLowerCase operation, without a game callback.
    fn lowercase(&self, text: &str) -> String;
    fn manager(&mut self, player: Option<Node>) -> Result<Option<Self::Manager>>;
    /// Construct only. The routing handler is the argument captured by the caller,
    /// independently of later changes to Task.option.handler during construction.
    fn movement(&mut self, task: Task, handler: Option<usize>) -> Result<Self::Pulse>;
    fn delayed(&mut self, task: Task) -> Result<Self::Pulse>;
    /// All manager operations here use PulseType.STANDARD.
    fn run(&mut self, manager: Self::Manager, pulse: Self::Pulse) -> Result<()>;
    fn clear(&mut self, manager: Self::Manager) -> Result<()>;
    fn unhandled(&mut self, manager: Self::Manager, player: Option<Node>) -> Result<()>;
    fn zone(
        &mut self,
        player: Option<Node>,
        target: Option<Node>,
        option: Option<&OptionHandle>,
    ) -> Result<bool>;
    fn combat_stop(&mut self, player: Option<Node>) -> Result<()>;
    fn player_location(&mut self, player: Option<Node>) -> Result<Option<Location>>;
    fn face_location(
        &mut self,
        target: Option<Node>,
        from: Option<Location>,
    ) -> Result<Option<Location>>;
    fn face_player(&mut self, player: Option<Node>, location: Option<Location>) -> Result<()>;
    fn message(&mut self, player: Option<Node>, text: &str) -> Result<()>;
    fn dispatch(&mut self, player: Option<Node>, target: Option<Node>, option: &str) -> Result<()>;
    fn exception(&mut self, error: &Error) -> Result<()>;
    fn error_message(&mut self, error: &Error) -> Result<Option<String>> {
        Ok(match error {
            Error::Null => None,
            Error::Binding(message) => message.clone(),
        })
    }
    fn log(&mut self, class: &str, message: &str) -> Result<()>;
}
fn option(o: Option<&OptionHandle>) -> Result<&OptionHandle> {
    o.ok_or(Error::Null)
}
fn handler(o: Option<&OptionHandle>) -> Result<Option<usize>> {
    Ok(option(o)?.read().handler)
}
fn required_handler(o: Option<&OptionHandle>) -> Result<usize> {
    handler(o)?.ok_or(Error::Null)
}
fn lower(ctx: &impl Context, o: Option<&OptionHandle>) -> Result<String> {
    let name = option(o)?.read().name().ok_or(Error::Null)?;
    Ok(ctx.lowercase(&name))
}
fn caught(ctx: &mut impl Context, class: &str, error: Error) -> Result<()> {
    ctx.exception(&error)?;
    let message = ctx.error_message(&error)?;
    ctx.log(
        class,
        &format!("{class}{}", message.as_deref().unwrap_or("null")),
    )
}
fn unhandled(ctx: &mut impl Context, player: Option<Node>) -> Result<()> {
    let manager = ctx.manager(player)?.ok_or(Error::Null)?;
    ctx.unhandled(manager, player)
}

pub fn handle(
    ctx: &mut impl Context,
    target: Option<Node>,
    player: Option<Node>,
    o: Option<OptionHandle>,
) -> Result<()> {
    fn body(
        ctx: &mut impl Context,
        target: Option<Node>,
        player: Option<Node>,
        o: Option<OptionHandle>,
    ) -> Result<()> {
        if ctx.interaction_locked(player)? || o.is_none() {
            return Ok(());
        }
        let name = option(o.as_ref())?
            .read()
            .name()
            .unwrap_or_else(|| "null".into());
        ctx.debug(player, &format!("Received interaction request {name}"))?;
        let has_handler = handler(o.as_ref())?.is_some();
        let mut walk = has_handler && ctx.handler_walk(required_handler(o.as_ref())?)?;
        if !walk
            && has_handler
            && ctx.handler_walk_for(required_handler(o.as_ref())?, player, target)?
        {
            walk = true;
        }
        if !has_handler || walk {
            walk_option(ctx, target, player, o.clone())?;
        } else {
            let name = ctx.handler_name(required_handler(o.as_ref())?)?;
            ctx.debug(player, &format!("Option handler being used={name}"))?;
            default_option(ctx, target, player, o.clone())?;
        }
        let name = lower(ctx, o.as_ref())?;
        ctx.dispatch(player, target, &name)
    }
    match body(ctx, target, player, o) {
        Ok(()) => Ok(()),
        Err(error) => {
            let class = ctx.class_name();
            caught(ctx, &class, error)
        }
    }
}

pub fn invalid(
    ctx: &mut impl Context,
    target: Option<Node>,
    player: Option<Node>,
    o: Option<OptionHandle>,
) -> Result<()> {
    if target.is_none() {
        return Ok(());
    }
    if ctx.location(target)?.is_none() {
        return unhandled(ctx, player);
    }
    if ctx.movement_locked(player)? {
        return Ok(());
    }
    let manager = ctx.manager(player)?;
    let pulse = ctx.movement(
        Task {
            player,
            target,
            option: o,
            kind: Kind::Invalid,
        },
        None,
    )?;
    ctx.run(manager.ok_or(Error::Null)?, pulse)
}
pub fn walk_option(
    ctx: &mut impl Context,
    target: Option<Node>,
    player: Option<Node>,
    o: Option<OptionHandle>,
) -> Result<()> {
    if ctx.location(target)?.is_none() {
        return unhandled(ctx, player);
    }
    if ctx.movement_locked(player)? {
        let manager = ctx.manager(player)?.ok_or(Error::Null)?;
        return ctx.clear(manager);
    }
    let manager = ctx.manager(player)?;
    let handler = handler(o.as_ref())?;
    let pulse = ctx.movement(
        Task {
            player,
            target,
            option: o,
            kind: Kind::Walk,
        },
        handler,
    )?;
    ctx.run(manager.ok_or(Error::Null)?, pulse)
}
pub fn default_option(
    ctx: &mut impl Context,
    target: Option<Node>,
    player: Option<Node>,
    o: Option<OptionHandle>,
) -> Result<()> {
    if !ctx.handler_delayed(required_handler(o.as_ref())?, player)? {
        if ctx.zone(player, target, o.as_ref())? {
            return Ok(());
        }
        ctx.combat_stop(player)?;
        let receiver = handler(o.as_ref())?;
        let name = lower(ctx, o.as_ref())?;
        if !ctx.handler_handle(receiver.ok_or(Error::Null)?, player, target, &name)? {
            unhandled(ctx, player)?;
        }
        return Ok(());
    }
    let manager = ctx.manager(player)?;
    let pulse = ctx.delayed(Task {
        player,
        target,
        option: o,
        kind: Kind::Delayed,
    })?;
    ctx.run(manager.ok_or(Error::Null)?, pulse)
}
pub const NOTHING: &str = "Nothing interesting happens.";
/// Node.getFaceLocation's reflected mathematical center. Location packing also
/// applies to the intermediate top-right point, before its coordinates are read.
pub fn node_face_location(location: &Location, size: i32, from: &Location) -> Location {
    let [x, y, z] = location.coordinates();
    let [tx, ty, _] = Location::new(
        x.wrapping_add(size.wrapping_sub(1)),
        y.wrapping_add(size.wrapping_sub(1)),
        z,
    )
    .coordinates();
    let [fx, fy, fz] = from.coordinates();
    Location::new(x + tx - fx, y + ty - fy, fz)
}
pub fn arrival(ctx: &mut impl Context, task: &Task) -> Result<bool> {
    fn body(ctx: &mut impl Context, t: &Task) -> Result<bool> {
        if t.kind != Kind::Delayed {
            let from = ctx.player_location(t.player)?;
            let face = ctx.face_location(t.target, from)?;
            ctx.face_player(t.player, face)?;
        }
        if ctx.interaction_locked(t.player)? || ctx.zone(t.player, t.target, t.option.as_ref())? {
            return Ok(true);
        }
        if t.kind == Kind::Invalid {
            ctx.message(t.player, NOTHING)?;
            return Ok(true);
        }
        if t.kind == Kind::Walk && (t.option.is_none() || handler(t.option.as_ref())?.is_none()) {
            ctx.message(t.player, NOTHING)?;
            return Ok(true);
        }
        let receiver = handler(t.option.as_ref())?;
        let name = lower(ctx, t.option.as_ref())?;
        if !ctx.handler_handle(receiver.ok_or(Error::Null)?, t.player, t.target, &name)? {
            ctx.message(t.player, NOTHING)?;
        }
        Ok(true)
    }
    match body(ctx, task) {
        Err(error) if task.kind != Kind::Delayed => {
            caught(ctx, task.class_name(), error)?;
            Ok(true)
        }
        result => result,
    }
}
