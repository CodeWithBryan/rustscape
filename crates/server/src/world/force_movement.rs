//! core/game/node/entity/impl/ForceMovement: a pulse that walks an entity
//! from a start tile to a destination with the ForceMove update mask, used by
//! agility obstacles, doors and shortcuts.
//!
//! The pulse's base fields (running, delay, ticks passed, node checks) are the
//! native `pulses::Pulse`; `update` is `pulses::update_base` with this class's
//! `start`/`pulse`/`stop` overrides, so a world drives a movement through the
//! same pulse runner as every other pulse. Animations are shared handles: the
//! static `WALK_ANIMATION` (one per world, [`Statics`]) and the callers'
//! objects whose priority `run` raises to `VERY_HIGH`. Locations are the
//! packed `Location` values (`prayer::location`). Every outside call is a
//! required [`Host`] service with no default.
use crate::definitions::item::Shared;
use crate::pulses::{self, Class, Error, Pulse, Result};
use crate::world::prayer::{distance, location};

/// The entity a movement drives (a pulse node).
pub type Node = pulses::Node;
/// A packed `Location`.
pub type Location = [i32; 3];

/// `ForceMovement.WALKING_SPEED` and `RUNNING_SPEED`.
pub const WALKING_SPEED: i32 = 10;
pub const RUNNING_SPEED: i32 = 20;
/// `WALK_ANIMATION`'s id.
pub const WALK_ANIMATION_ID: i32 = 819;
/// The pulse class name a runner reports.
pub const CLASS_NAME: &str = "core.game.node.entity.impl.ForceMovement";

/// `Animator.Priority`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Mid,
    High,
    VeryHigh,
}

impl Priority {
    pub fn name(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Mid => "MID",
            Self::High => "HIGH",
            Self::VeryHigh => "VERY_HIGH",
        }
    }
}

/// An `Animation` object: its id and (mutable) priority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Animation {
    pub id: i32,
    pub priority: Priority,
}

/// One animation object; clones share it.
pub type AnimationHandle = Shared<Animation>;

/// `Animation.create(id)` (priority MID).
pub fn create_animation(id: i32) -> AnimationHandle {
    Shared::new(Animation {
        id,
        priority: Priority::Mid,
    })
}

/// The class's static state, one per world: `WALK_ANIMATION`.
#[derive(Clone)]
pub struct Statics {
    pub walk_animation: AnimationHandle,
}

impl Default for Statics {
    fn default() -> Self {
        Self {
            walk_animation: create_animation(WALK_ANIMATION_ID),
        }
    }
}

/// `Direction`, by constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    NorthWest,
    North,
    NorthEast,
    West,
    East,
    SouthWest,
    South,
    SouthEast,
}

impl Direction {
    pub const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::West,
        Self::East,
        Self::SouthWest,
        Self::South,
        Self::SouthEast,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::NorthWest => "NORTH_WEST",
            Self::North => "NORTH",
            Self::NorthEast => "NORTH_EAST",
            Self::West => "WEST",
            Self::East => "EAST",
            Self::SouthWest => "SOUTH_WEST",
            Self::South => "SOUTH",
            Self::SouthEast => "SOUTH_EAST",
        }
    }

    pub fn value_of(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == name)
    }

    /// `Direction.getDirection(diffX, diffY)`.
    pub fn get(dx: i32, dy: i32) -> Self {
        if dx < 0 {
            if dy < 0 {
                Self::SouthWest
            } else if dy > 0 {
                Self::NorthWest
            } else {
                Self::West
            }
        } else if dx > 0 {
            if dy < 0 {
                Self::SouthEast
            } else if dy > 0 {
                Self::NorthEast
            } else {
                Self::East
            }
        } else if dy < 0 {
            Self::South
        } else {
            Self::North
        }
    }
}

/// `ForceMoveCtx`: the ForceMove mask's context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForceMoveCtx {
    pub start: Location,
    pub dest: Location,
    pub start_arrive: i32,
    pub dest_arrive: i32,
    pub direction: Direction,
}

/// Everything a force movement reaches outside itself. Each method is one
/// original call; a failure leaves the earlier effects in place.
pub trait Host {
    /// `entity instanceof Player` (a plain type test, not a call).
    fn is_player(&self, entity: Node) -> bool;
    /// `entity.getLocation()`.
    fn location(&mut self, entity: Node) -> Result<Option<Location>>;
    /// `entity.animate(animation)`.
    fn animate(&mut self, entity: Node, animation: Option<&AnimationHandle>) -> Result<bool>;
    /// `entity.getImpactHandler().setDisabledTicks(ticks)`.
    fn set_disabled_ticks(&mut self, entity: Node, ticks: i32) -> Result<()>;
    /// `entity.getUpdateMasks().register(EntityFlag.ForceMove, context)`.
    fn register_force_move(&mut self, entity: Node, context: ForceMoveCtx) -> Result<bool>;
    /// `entity.getWalkingQueue().updateRegion(start, destination, forced)`.
    fn update_region(
        &mut self,
        entity: Node,
        start: Location,
        destination: Location,
        forced: bool,
    ) -> Result<bool>;
    /// `entity.getProperties().setTeleportLocation(location)`.
    fn set_teleport_location(&mut self, entity: Node, location: Option<Location>) -> Result<()>;
    /// `entity.lock()`.
    fn lock(&mut self, entity: Node) -> Result<()>;
    /// `entity.unlock()`.
    fn unlock(&mut self, entity: Node) -> Result<()>;
    /// `GameWorld.getPulser().submit(movement)`.
    fn submit(&mut self, movement: &Handle) -> Result<()>;
    /// `node.isActive()` (Pulse's node check).
    fn node_active(&mut self, node: Node) -> Result<bool>;
    /// `exception.printStackTrace()` in `Pulse.update`: an observation.
    fn exception(&mut self, error: &Error) -> Result<()>;
}

/// One `ForceMovement` object.
pub struct ForceMovement {
    /// The `Pulse` base: running, delay, ticks passed and `checks` (`[e]`).
    pub pulse: pulses::Handle,
    pub entity: Option<Node>,
    pub start: Option<Location>,
    pub destination: Option<Location>,
    pub start_anim: Option<AnimationHandle>,
    pub animation: Option<AnimationHandle>,
    pub end_animation: Option<AnimationHandle>,
    pub direction: Option<Direction>,
    pub commence_speed: i32,
    pub path_speed: i32,
    pub unlock_after: bool,
}

/// A movement object; clones share it.
pub type Handle = Shared<ForceMovement>;

/// `ForceMovement.direction(s, d)`: the larger axis of `Location.getDelta`.
pub fn direction(s: Location, d: Location) -> Direction {
    let delta = location(
        d[0].wrapping_sub(s[0]),
        d[1].wrapping_sub(s[1]),
        d[2].wrapping_sub(s[2]),
    );
    let x = delta[0].wrapping_abs();
    let y = delta[1].wrapping_abs();
    if x > y {
        Direction::get(delta[0], 0)
    } else {
        Direction::get(0, delta[1])
    }
}

fn required<T>(value: Option<T>) -> Result<T> {
    value.ok_or(Error::Null)
}

fn entity_location<H: Host>(host: &mut H, entity: Option<Node>) -> Result<Location> {
    required(host.location(required(entity)?)?)
}

/// `(int) Math.ceil(a / (speed * 0.1))`.
fn ceil_ticks(distance: f64, speed: i32) -> i32 {
    (distance / (f64::from(speed) * 0.1)).ceil() as i32
}

impl ForceMovement {
    /// The nine-argument constructor (`super(1, e)`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entity: Option<Node>,
        start: Option<Location>,
        destination: Option<Location>,
        start_anim: Option<AnimationHandle>,
        animation: Option<AnimationHandle>,
        direction: Option<Direction>,
        commence_speed: i32,
        path_speed: i32,
        unlock_after: bool,
    ) -> Handle {
        let pulse = Shared::new(Pulse::new(
            1,
            Some(Shared::new(vec![entity])),
            Class::Other,
            CLASS_NAME.into(),
        ));
        Shared::new(Self {
            pulse,
            entity,
            start,
            destination,
            start_anim,
            animation,
            end_animation: None,
            direction,
            commence_speed,
            path_speed,
            unlock_after,
        })
    }

    /// `ForceMovement(e, start, end, animation, speed)`.
    pub fn with_speed(
        statics: &Statics,
        e: Option<Node>,
        start: Option<Location>,
        end: Option<Location>,
        animation: Option<AnimationHandle>,
        speed: i32,
    ) -> Result<Handle> {
        let d = direction(required(start)?, required(end)?);
        Ok(Self::new(
            e,
            start,
            end,
            Some(statics.walk_animation.clone()),
            animation,
            Some(d),
            WALKING_SPEED,
            speed,
            true,
        ))
    }

    /// `ForceMovement(e, destination, startSpeed, animSpeed)`: two location reads.
    pub fn with_speeds<H: Host>(
        host: &mut H,
        statics: &Statics,
        e: Option<Node>,
        destination: Option<Location>,
        start_speed: i32,
        anim_speed: i32,
    ) -> Result<Handle> {
        let start = host.location(required(e)?)?;
        let from = entity_location(host, e)?;
        let d = direction(from, required(destination)?);
        let walk = Some(statics.walk_animation.clone());
        Ok(Self::new(
            e,
            start,
            destination,
            walk.clone(),
            walk,
            Some(d),
            start_speed,
            anim_speed,
            true,
        ))
    }

    /// `ForceMovement(e, start, destination, animation)`.
    pub fn with_animation(
        statics: &Statics,
        e: Option<Node>,
        start: Option<Location>,
        destination: Option<Location>,
        animation: Option<AnimationHandle>,
    ) -> Result<Handle> {
        let d = direction(required(start)?, required(destination)?);
        Ok(Self::new(
            e,
            start,
            destination,
            Some(statics.walk_animation.clone()),
            animation,
            Some(d),
            WALKING_SPEED,
            WALKING_SPEED,
            true,
        ))
    }

    /// `ForceMovement(start, destination, animation)`: no entity.
    pub fn detached(
        statics: &Statics,
        start: Option<Location>,
        destination: Option<Location>,
        animation: Option<AnimationHandle>,
    ) -> Result<Handle> {
        Self::with_animation(statics, None, start, destination, animation)
    }
}

/// The static `run` forms, each as the original delegates to the full one.
pub enum RunForm {
    /// `run(e, destination)`.
    Destination,
    /// `run(e, start, destination)`: the direction is from the entity's location.
    Start,
    /// `run(e, start, destination, animation)`.
    Animation(Option<AnimationHandle>),
    /// `run(e, start, destination, animation, speed)`.
    AnimationSpeed(Option<AnimationHandle>, i32),
    /// `run(e, start, destination, startAnim, animation)`.
    Animations(Option<AnimationHandle>, Option<AnimationHandle>),
    /// `run(e, start, destination, startAnim, animation, direction)`.
    Direction(
        Option<AnimationHandle>,
        Option<AnimationHandle>,
        Option<Direction>,
    ),
    /// `run(e, start, destination, startAnim, animation, direction, pathSpeed)`.
    PathSpeed(
        Option<AnimationHandle>,
        Option<AnimationHandle>,
        Option<Direction>,
        i32,
    ),
    /// `run(e, start, destination, startAnim, animation, direction, commenceSpeed, pathSpeed)`.
    Speeds(
        Option<AnimationHandle>,
        Option<AnimationHandle>,
        Option<Direction>,
        i32,
        i32,
    ),
    /// The full form.
    Full(
        Option<AnimationHandle>,
        Option<AnimationHandle>,
        Option<Direction>,
        i32,
        i32,
        bool,
    ),
    /// `run(e, destination, commenceSpeed, pathSpeed)`.
    DestinationSpeeds(i32, i32),
}

/// The static `ForceMovement.run(...)` of the given form.
pub fn run_static<H: Host>(
    host: &mut H,
    statics: &Statics,
    e: Option<Node>,
    start: Option<Location>,
    destination: Option<Location>,
    form: RunForm,
) -> Result<Handle> {
    let walk = || Some(statics.walk_animation.clone());
    match form {
        RunForm::Destination => {
            let from = host.location(required(e)?)?;
            let here = entity_location(host, e)?;
            let d = direction(here, required(destination)?);
            run(
                host,
                e,
                from,
                destination,
                walk(),
                walk(),
                Some(d),
                WALKING_SPEED,
                WALKING_SPEED,
                true,
            )
        }
        RunForm::Start => {
            let here = entity_location(host, e)?;
            let d = direction(here, required(destination)?);
            run(
                host,
                e,
                start,
                destination,
                walk(),
                walk(),
                Some(d),
                WALKING_SPEED,
                WALKING_SPEED,
                true,
            )
        }
        RunForm::Animation(anim) => {
            let d = direction(required(start)?, required(destination)?);
            run(
                host,
                e,
                start,
                destination,
                walk(),
                anim,
                Some(d),
                WALKING_SPEED,
                WALKING_SPEED,
                true,
            )
        }
        RunForm::AnimationSpeed(anim, speed) => {
            let d = direction(required(start)?, required(destination)?);
            run(
                host,
                e,
                start,
                destination,
                walk(),
                anim,
                Some(d),
                WALKING_SPEED,
                speed,
                true,
            )
        }
        RunForm::Animations(start_anim, anim) => {
            let d = direction(required(start)?, required(destination)?);
            run(
                host,
                e,
                start,
                destination,
                start_anim,
                anim,
                Some(d),
                WALKING_SPEED,
                WALKING_SPEED,
                true,
            )
        }
        RunForm::Direction(start_anim, anim, d) => run(
            host,
            e,
            start,
            destination,
            start_anim,
            anim,
            d,
            WALKING_SPEED,
            WALKING_SPEED,
            true,
        ),
        RunForm::PathSpeed(start_anim, anim, d, path) => run(
            host,
            e,
            start,
            destination,
            start_anim,
            anim,
            d,
            WALKING_SPEED,
            path,
            true,
        ),
        RunForm::Speeds(start_anim, anim, d, commence, path) => run(
            host,
            e,
            start,
            destination,
            start_anim,
            anim,
            d,
            commence,
            path,
            true,
        ),
        RunForm::Full(start_anim, anim, d, commence, path, unlock) => run(
            host,
            e,
            start,
            destination,
            start_anim,
            anim,
            d,
            commence,
            path,
            unlock,
        ),
        RunForm::DestinationSpeeds(commence, path) => {
            let from = host.location(required(e)?)?;
            let here = entity_location(host, e)?;
            let d = direction(here, required(destination)?);
            run(
                host,
                e,
                from,
                destination,
                walk(),
                walk(),
                Some(d),
                commence,
                path,
                true,
            )
        }
    }
}

/// The full static `run`: raises both animations to `VERY_HIGH`, constructs,
/// starts, locks the entity and submits the pulse.
#[allow(clippy::too_many_arguments)]
pub fn run<H: Host>(
    host: &mut H,
    e: Option<Node>,
    start: Option<Location>,
    destination: Option<Location>,
    start_anim: Option<AnimationHandle>,
    animation: Option<AnimationHandle>,
    direction: Option<Direction>,
    commence_speed: i32,
    path_speed: i32,
    unlock_after: bool,
) -> Result<Handle> {
    if let Some(a) = &start_anim {
        a.write().priority = Priority::VeryHigh;
    }
    if let Some(a) = &animation {
        a.write().priority = Priority::VeryHigh;
    }
    let fm = ForceMovement::new(
        e,
        start,
        destination,
        start_anim,
        animation,
        direction,
        commence_speed,
        path_speed,
        unlock_after,
    );
    self::start(host, &fm)?;
    host.lock(required(e)?)?;
    host.submit(&fm)?;
    Ok(fm)
}

/// The instance `run(e, speed)`.
pub fn run_with<H: Host>(host: &mut H, fm: &Handle, e: Option<Node>, speed: i32) -> Result<()> {
    fm.write().entity = e;
    // start.getDistance(e.getLocation()): the argument is read before the receiver is used.
    let start = fm.read().start;
    let here = host.location(required(e)?)?;
    let start = required(start)?;
    let mut commence = distance(start, required(here)?) as i32;
    let commence_speed = fm.read().commence_speed;
    if commence != 0 && commence_speed != 0 {
        commence = (1.0 + f64::from(commence) / (f64::from(commence_speed) * 0.1)) as i32;
    }
    let destination = required(fm.read().destination)?;
    let path_speed = fm.read().path_speed;
    let path = 1i32.wrapping_add(ceil_ticks(distance(start, destination), path_speed));
    {
        let mut f = fm.write();
        f.path_speed = if path_speed == 0 { path } else { speed };
        f.commence_speed = commence;
    }
    self::start(host, fm)?;
    host.lock(required(e)?)?;
    host.submit(fm)
}

/// The instance `run(e)`.
pub fn run_entity<H: Host>(host: &mut H, fm: &Handle, e: Option<Node>) -> Result<()> {
    run_with(host, fm, e, 0)
}

/// The instance `run()`.
pub fn run_self<H: Host>(host: &mut H, fm: &Handle) -> Result<()> {
    let e = fm.read().entity;
    run_entity(host, fm, e)
}

/// `start()`: the speeds become tick counts, the first animation plays, the
/// impact handler is disabled for the whole movement and the mask registered.
pub fn start<H: Host>(host: &mut H, fm: &Handle) -> Result<()> {
    let (start, entity, commence_speed, path_speed) = {
        let f = fm.read();
        (f.start, f.entity, f.commence_speed, f.path_speed)
    };
    // start.getDistance(entity.getLocation()): the argument is read before the receiver is used.
    let here = host.location(required(entity)?)?;
    let start = required(start)?;
    let commence = ceil_ticks(distance(start, required(here)?), commence_speed);
    fm.write().commence_speed = commence;
    let destination = required(fm.read().destination)?;
    let path = ceil_ticks(distance(start, destination), path_speed);
    fm.write().path_speed = path;
    let e = required(entity)?;
    let pulse = fm.read().pulse.clone();
    if commence != 0 {
        let anim = fm.read().start_anim.clone();
        host.animate(e, anim.as_ref())?;
        pulse.write().delay = commence;
    } else {
        let anim = fm.read().animation.clone();
        host.animate(e, anim.as_ref())?;
        pulse.write().delay = path;
    }
    let ticks = 1i32.wrapping_add(commence).wrapping_add(path);
    host.set_disabled_ticks(e, ticks)?;
    let (destination, direction) = {
        let f = fm.read();
        (f.destination, f.direction)
    };
    let context = ForceMoveCtx {
        start,
        dest: required(destination)?,
        start_arrive: commence.wrapping_mul(30),
        dest_arrive: path.wrapping_mul(30),
        direction: required(direction)?,
    };
    host.register_force_move(e, context)?;
    if host.is_player(e) {
        host.update_region(e, start, required(destination)?, false)?;
    }
    pulses::start_base(&pulse);
    Ok(())
}

/// `pulse()`: after the commencing walk, play the path animation, wait the
/// path ticks and teleport to the start; then finish.
pub fn pulse<H: Host>(host: &mut H, fm: &Handle) -> Result<bool> {
    if fm.read().commence_speed != 0 {
        let (e, anim, path, start) = {
            let f = fm.read();
            (f.entity, f.animation.clone(), f.path_speed, f.start)
        };
        let e = required(e)?;
        host.animate(e, anim.as_ref())?;
        fm.read().pulse.write().delay = path;
        fm.write().commence_speed = 0;
        host.set_teleport_location(e, start)?;
        return Ok(false);
    }
    Ok(true)
}

/// `stop()`: the pulse stops, the entity is placed at the destination, the
/// end animation plays and the entity is unlocked when asked to.
pub fn stop<H: Host>(host: &mut H, fm: &Handle) -> Result<()> {
    let pulse = fm.read().pulse.clone();
    pulses::stop_base(&pulse);
    let (e, destination, end, unlock) = {
        let f = fm.read();
        (
            f.entity,
            f.destination,
            f.end_animation.clone(),
            f.unlock_after,
        )
    };
    let e = required(e)?;
    host.set_teleport_location(e, destination)?;
    if let Some(end) = end {
        host.animate(e, Some(&end))?;
    }
    if unlock {
        host.unlock(e)?;
    }
    Ok(())
}

/// The pulse runner's view of one movement: `Pulse.update` with this class's
/// overrides.
struct Driver<'a, H> {
    host: &'a mut H,
    movement: Handle,
}

impl<H: Host> pulses::Context<()> for Driver<'_, H> {
    fn pulse(&mut self, _: &pulses::Handle, _: &mut ()) -> Result<bool> {
        pulse(self.host, &self.movement)
    }
    fn node_active(&mut self, node: Node, _: &mut ()) -> Result<bool> {
        self.host.node_active(node)
    }
    fn exception(&mut self, error: &Error, _: &mut ()) -> Result<()> {
        self.host.exception(error)
    }
    fn start(&mut self, _: &pulses::Handle, _: &mut ()) -> Result<()> {
        start(self.host, &self.movement)
    }
    fn stop(&mut self, _: &pulses::Handle, _: &mut ()) -> Result<()> {
        stop(self.host, &self.movement)
    }
    // The members below belong to the pulse manager and runner, which do not
    // reach a movement through `update`.
    fn remove_weak_scripts(&mut self, _: &mut ()) -> Result<()> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> Result<()> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn is_dead(&mut self, _: &mut ()) -> Result<bool> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn send_message(&mut self, _: Node, _: &str, _: &mut ()) -> Result<()> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn now_millis(&mut self, _: &mut ()) -> Result<i64> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn log(&mut self, _: pulses::Level, _: String, _: &mut ()) -> Result<()> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn pulse_length(&mut self, _: &str, _: i32, _: &mut ()) -> Result<()> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn count_pulse(&mut self, _: &str, _: &mut ()) -> Result<()> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        false
    }
    fn set_bot_total(&mut self, _: i32, _: &mut ()) -> Result<()> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
    fn set_other_total(&mut self, _: i32, _: &mut ()) -> Result<()> {
        Err(Error::Binding("not a Pulse.update member".into()))
    }
}

/// `update()` (Pulse's): the node check, the running check, the tick count
/// against the delay, then `pulse()` and `stop()`; a failing body is printed
/// and stops the pulse.
pub fn update<H: Host>(host: &mut H, fm: &Handle) -> Result<bool> {
    let pulse = fm.read().pulse.clone();
    let mut driver = Driver {
        host,
        movement: fm.clone(),
    };
    pulses::update_base(&pulse, &mut (), &mut driver)
}
