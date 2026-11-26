//! `Projectile` and `ProjectileUpdateFlag`: every factory (`create` in its
//! seven forms, `magic`, `ranged`, the empty constructor), `getSpeed`,
//! `getLocation`, both `transform` pairs, `copy`, the three `send`s, the
//! setters, and the chunk flag's `write` (opcode 16 inside the chunk update),
//! with `UpdateFlag`'s `data`/`ordinal`/`compareTo`/`equals`.
//!
//! A projectile is a shared object: `transform` changes it and answers the
//! same object, and the chunk flag keeps it, so the setters and transforms
//! made after `send` show in the bytes `write` produces. `write` reads the
//! victim's location (unless the projectile is location-based) and its index
//! when it is written, as the original does.
//!
//! The world is the [`Host`]: the entity kind (`instanceof Player`/`NPC`),
//! `Node.size()`/`getLocation()`/`getIndex()` (calls), the `size` and
//! `location` fields that `Node.getCenterLocation` reads directly, and
//! `RegionManager.getRegionChunk(location).flag(flag)`. No guard spans a
//! host call. Differentially tested by projectiles-oracle
//! (docs/projectiles.md).
use super::doors::Location;
use crate::definitions::item::Shared;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A `NullPointerException` of the original.
    Null,
    /// A failure of a host service.
    Binding(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

/// `instanceof Player` / `instanceof NPC` of an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Npc,
    Other,
}

pub trait Host {
    type Entity: Clone;
    /// What `RegionManager.getRegionChunk` answers.
    type Chunk;
    fn kind(&self, entity: &Self::Entity) -> Kind;
    /// The `size` field (read directly by `Node.getCenterLocation`).
    fn size_field(&self, entity: &Self::Entity) -> i32;
    /// The `location` field (read directly by `Node.getCenterLocation`).
    fn location_field(&self, entity: &Self::Entity) -> Option<Location>;
    /// `Node.size()`.
    fn size(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `Node.getLocation()`.
    fn location(&mut self, entity: &Self::Entity) -> Result<Option<Location>>;
    /// `Node.getIndex()`.
    fn index(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `RegionManager.getRegionChunk(location)`.
    fn region_chunk(&mut self, location: Option<&Location>) -> Result<Self::Chunk>;
    /// `chunk.flag(new ProjectileUpdateFlag(projectile))`.
    fn flag(&mut self, chunk: Self::Chunk, projectile: Projectile<Self::Entity>) -> Result<()>;
}

/// The fields of one `Projectile` object.
#[derive(Clone, Debug)]
pub struct Fields<E> {
    pub source: Option<E>,
    pub source_location: Option<Location>,
    pub victim: Option<E>,
    pub projectile_id: i32,
    pub start_height: i32,
    pub end_height: i32,
    pub start_delay: i32,
    pub speed: i32,
    pub angle: i32,
    pub distance: i32,
    pub end_location: Option<Location>,
}

impl<E> Default for Fields<E> {
    fn default() -> Self {
        Self {
            source: None,
            source_location: None,
            victim: None,
            projectile_id: 0,
            start_height: 0,
            end_height: 0,
            start_delay: 0,
            speed: 0,
            angle: 0,
            distance: 0,
            end_location: None,
        }
    }
}

impl<E> Fields<E> {
    /// `isLocationBased()`.
    pub fn is_location_based(&self) -> bool {
        self.end_location.is_some()
    }
}

/// A `Projectile` object (shared: the chunk flag keeps the same object).
pub type Projectile<E> = Shared<Fields<E>>;

/// `new Projectile()`.
pub fn empty<E>() -> Projectile<E> {
    Shared::new(Fields::default())
}

/// `Location.getDistance(first, second)`: Int arithmetic, then `sqrt`.
pub fn distance(first: &Location, second: &Location) -> f64 {
    let [ax, ay, _] = first.coordinates();
    let [bx, by, _] = second.coordinates();
    let dx = ax.wrapping_sub(bx);
    let dy = ay.wrapping_sub(by);
    f64::from(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))).sqrt()
}

/// `Location.transform(diffX, diffY, z)`.
fn transformed(location: &Location, dx: i32, dy: i32, dz: i32) -> Location {
    let [x, y, z] = location.coordinates();
    let z = z.wrapping_add(dz) % 4;
    Location::new(x.wrapping_add(dx), y.wrapping_add(dy), (z % 4 + 4) % 4)
}

/// `Node.getCenterLocation()`: the fields, not the getters.
fn center<H: Host>(h: &H, entity: &H::Entity) -> Result<Location> {
    let offset = h.size_field(entity) >> 1;
    let location = h.location_field(entity).ok_or(Error::Null)?;
    Ok(transformed(&location, offset, offset, 0))
}

/// `Projectile.getLocation(n)`: null for a null node, else its centre.
pub fn get_location<H: Host>(h: &H, entity: Option<&H::Entity>) -> Result<Option<Location>> {
    match entity {
        None => Ok(None),
        Some(e) => center(h, e).map(Some),
    }
}

/// `target.getDistance(entity.getLocation())`: the receiver is evaluated
/// first, then the argument (a null entity fails here), then the receiver's
/// and the Kotlin parameter's null checks.
fn distance_to<H: Host>(
    h: &mut H,
    target: Option<Location>,
    entity: Option<&H::Entity>,
) -> Result<f64> {
    let to = h.location(entity.ok_or(Error::Null)?)?;
    let from = target.ok_or(Error::Null)?;
    Ok(distance(&from, &to.ok_or(Error::Null)?))
}

/// `(int) (base + distance * factor)`.
fn speed(base: f64, distance: f64, factor: f64) -> i32 {
    (base + distance * factor) as i32
}

/// `source.size() << 5`.
fn shifted<H: Host>(h: &mut H, source: Option<&H::Entity>) -> Result<i32> {
    Ok(h.size(source.ok_or(Error::Null)?)?.wrapping_shl(5))
}

/// `source == null ? 11 : source.size() << 5`.
fn guarded<H: Host>(h: &mut H, source: Option<&H::Entity>) -> Result<i32> {
    match source {
        None => Ok(11),
        Some(_) => shifted(h, source),
    }
}

/// The private entity constructor.
#[allow(clippy::too_many_arguments)]
fn construct<H: Host>(
    h: &H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    speed: i32,
    angle: i32,
    distance: i32,
) -> Result<Projectile<H::Entity>> {
    let source_location = get_location(h, source)?;
    Ok(Shared::new(Fields {
        source: source.cloned(),
        source_location,
        victim: victim.cloned(),
        projectile_id: id,
        start_height,
        end_height,
        start_delay,
        speed,
        angle,
        distance,
        end_location: None,
    }))
}

/// `46 + getLocation(source).getDistance(victim.getLocation()) * factor`.
fn speed_from<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    factor: f64,
) -> Result<i32> {
    let from = get_location(h, source)?;
    Ok(speed(46.0, distance_to(h, from, victim)?, factor))
}

/// `Projectile.create(source, victim, id)`.
pub fn create<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
) -> Result<Projectile<H::Entity>> {
    create_with_delay(h, source, victim, id, 40, 36, 41)
}

/// `Projectile.create(source, victim, id, startHeight, endHeight)`.
pub fn create_with_heights<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
) -> Result<Projectile<H::Entity>> {
    create_with_delay(h, source, victim, id, start_height, end_height, 41)
}

/// `Projectile.create(source, victim, id, startHeight, endHeight, startDelay)`.
pub fn create_with_delay<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
) -> Result<Projectile<H::Entity>> {
    let speed = speed_from(h, source, victim, 5.0)?;
    let distance = guarded(h, source)?;
    construct(
        h,
        source,
        victim,
        id,
        start_height,
        end_height,
        start_delay,
        speed,
        5,
        distance,
    )
}

/// `Projectile.create(source, victim, id, startHeight, endHeight, startDelay,
/// speed)`.
#[allow(clippy::too_many_arguments)]
pub fn create_with_speed<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    speed: i32,
) -> Result<Projectile<H::Entity>> {
    let distance = guarded(h, source)?;
    construct(
        h,
        source,
        victim,
        id,
        start_height,
        end_height,
        start_delay,
        speed,
        5,
        distance,
    )
}

/// `Projectile.create(source, victim, id, startHeight, endHeight, startDelay,
/// speed, angle)`: `source.size() << 5` without the null guard.
#[allow(clippy::too_many_arguments)]
pub fn create_with_angle<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    speed: i32,
    angle: i32,
) -> Result<Projectile<H::Entity>> {
    let distance = shifted(h, source)?;
    construct(
        h,
        source,
        victim,
        id,
        start_height,
        end_height,
        start_delay,
        speed,
        angle,
        distance,
    )
}

/// `Projectile.create(source, victim, id, startHeight, endHeight, startDelay,
/// speed, angle, distance)`.
#[allow(clippy::too_many_arguments)]
pub fn create_full<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    speed: i32,
    angle: i32,
    distance: i32,
) -> Result<Projectile<H::Entity>> {
    construct(
        h,
        source,
        victim,
        id,
        start_height,
        end_height,
        start_delay,
        speed,
        angle,
        distance,
    )
}

/// `Projectile.create(start, destination, id, startHeight, endHeight,
/// startDelay, speed, angle, distance)`: a location-based projectile that
/// keeps both location objects.
#[allow(clippy::too_many_arguments)]
pub fn create_at<E>(
    start: Option<Location>,
    destination: Option<Location>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    speed: i32,
    angle: i32,
    distance: i32,
) -> Projectile<E> {
    Shared::new(Fields {
        source: None,
        source_location: start,
        victim: None,
        projectile_id: id,
        start_height,
        end_height,
        start_delay,
        speed,
        angle,
        distance,
        end_location: destination,
    })
}

/// `Projectile.magic(...)`: speed factor 10.
#[allow(clippy::too_many_arguments)]
pub fn magic<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    angle: i32,
) -> Result<Projectile<H::Entity>> {
    speed_based(
        h,
        source,
        victim,
        id,
        start_height,
        end_height,
        start_delay,
        angle,
        10.0,
    )
}

/// `Projectile.ranged(...)`: speed factor 5.
#[allow(clippy::too_many_arguments)]
pub fn ranged<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    angle: i32,
) -> Result<Projectile<H::Entity>> {
    speed_based(
        h,
        source,
        victim,
        id,
        start_height,
        end_height,
        start_delay,
        angle,
        5.0,
    )
}

#[allow(clippy::too_many_arguments)]
fn speed_based<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    id: i32,
    start_height: i32,
    end_height: i32,
    start_delay: i32,
    angle: i32,
    factor: f64,
) -> Result<Projectile<H::Entity>> {
    let speed = speed_from(h, source, victim, factor)?;
    let distance = shifted(h, source)?;
    construct(
        h,
        source,
        victim,
        id,
        start_height,
        end_height,
        start_delay,
        speed,
        angle,
        distance,
    )
}

/// `Projectile.getSpeed(source, targetLoc)`.
pub fn get_speed<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    target: Option<&Location>,
) -> Result<i32> {
    let from = get_location(h, source)?.ok_or(Error::Null)?;
    let to = target.ok_or(Error::Null)?;
    Ok(speed(46.0, distance(&from, to), 5.0))
}

/// `transform(source, victim)`: handled as an NPC when the source is one,
/// base 46, factor 5.
pub fn transform<H: Host>(
    h: &mut H,
    p: &Projectile<H::Entity>,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
) -> Result<Projectile<H::Entity>> {
    let npc = source.is_some_and(|s| h.kind(s) == Kind::Npc);
    transform_with(h, p, source, victim, npc, 46, 5)
}

/// `transform(source, victim, npc, baseSpeed, modifiedSpeed)`.
pub fn transform_with<H: Host>(
    h: &mut H,
    p: &Projectile<H::Entity>,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    npc: bool,
    base_speed: i32,
    modified_speed: i32,
) -> Result<Projectile<H::Entity>> {
    p.write().source = source.cloned();
    let location = get_location(h, source)?;
    p.write().source_location = location;
    p.write().victim = victim.cloned();
    let target = p.read().source_location.clone();
    let d = distance_to(h, target, victim)?;
    p.write().speed = speed(f64::from(base_speed), d, f64::from(modified_speed));
    if npc {
        let distance = shifted(h, source)?;
        p.write().distance = distance;
    }
    Ok(p.clone())
}

/// `transform(source, location)`.
pub fn transform_to<H: Host>(
    h: &mut H,
    p: &Projectile<H::Entity>,
    source: Option<&H::Entity>,
    location: Option<Location>,
) -> Result<Projectile<H::Entity>> {
    let npc = source.is_some_and(|s| h.kind(s) == Kind::Npc);
    transform_to_with(h, p, source, location, npc, 46, 5)
}

/// `transform(source, location, npc, baseSpeed, modifiedSpeed)`.
pub fn transform_to_with<H: Host>(
    h: &mut H,
    p: &Projectile<H::Entity>,
    source: Option<&H::Entity>,
    location: Option<Location>,
    npc: bool,
    base_speed: i32,
    modified_speed: i32,
) -> Result<Projectile<H::Entity>> {
    p.write().source = source.cloned();
    let start = get_location(h, source)?;
    p.write().source_location = start;
    p.write().end_location = location.clone();
    let from = p.read().source_location.clone().ok_or(Error::Null)?;
    let to = location.ok_or(Error::Null)?;
    p.write().speed = speed(
        f64::from(base_speed),
        distance(&from, &to),
        f64::from(modified_speed),
    );
    if npc {
        let distance = shifted(h, source)?;
        p.write().distance = distance;
    }
    Ok(p.clone())
}

/// `copy(source, victim, speedMultiplier)`: a new projectile with this one's
/// numbers, its speed plus the distance between the two entities'
/// locations (not their centres) times the multiplier, and its distance.
pub fn copy<H: Host>(
    h: &mut H,
    p: &Projectile<H::Entity>,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    multiplier: f64,
) -> Result<Projectile<H::Entity>> {
    let base = p.read().speed;
    let from = h.location(source.ok_or(Error::Null)?)?;
    let d = distance_to(h, from, victim)?;
    let speed = speed(f64::from(base), d, multiplier);
    let f = p.read().clone();
    construct(
        h,
        source,
        victim,
        f.projectile_id,
        f.start_height,
        f.end_height,
        f.start_delay,
        speed,
        f.angle,
        f.distance,
    )
}

/// `send()` / `Projectile.send(p)`: `RegionManager.getRegionChunk(
/// p.getSourceLocation()).flag(new ProjectileUpdateFlag(p))`.
pub fn send<H: Host>(h: &mut H, p: &Projectile<H::Entity>) -> Result<()> {
    let location = p.read().source_location.clone();
    let chunk = h.region_chunk(location.as_ref())?;
    h.flag(chunk, p.clone())
}

/// `Projectile.send(source, victim, p)`.
pub fn send_to<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    p: &Projectile<H::Entity>,
) -> Result<()> {
    let p = transform(h, p, source, victim)?;
    send(h, &p)
}

/// `Projectile.send(source, location, p)`.
pub fn send_at<H: Host>(
    h: &mut H,
    source: Option<&H::Entity>,
    location: Option<Location>,
    p: &Projectile<H::Entity>,
) -> Result<()> {
    let p = transform_to(h, p, source, location)?;
    send(h, &p)
}

/// `ProjectileUpdateFlag.data()`.
pub const DATA: i32 = 0;
/// `ProjectileUpdateFlag.ordinal()`.
pub const ORDINAL: i32 = 2;

/// `UpdateFlag.compareTo(flag)` of two flags with these ordinals.
pub fn compare_to(ordinal: i32, other: i32) -> i32 {
    if other == ordinal {
        0
    } else if other < ordinal {
        1
    } else {
        -1
    }
}

/// `UpdateFlag.equals(o)` of two update flags.
pub fn equals(data: i32, ordinal: i32, other_data: i32, other_ordinal: i32) -> bool {
    other_data == data && other_ordinal == ordinal
}

/// `Location.getChunkOffsetX()`/`Y`: `localX and 7`.
fn chunk_offset(c: i32) -> i32 {
    (c & 63) & 7
}

/// `ProjectileUpdateFlag.write(buffer)`: the bytes are appended to `out` as
/// they are put, so a failure leaves the original's prefix.
pub fn write<H: Host>(h: &mut H, p: &Projectile<H::Entity>, out: &mut Vec<u8>) -> Result<()> {
    let start = p.read().source_location.clone();
    let target = p.read().victim.clone();
    let end = if p.read().is_location_based() {
        p.read().end_location.clone()
    } else {
        h.location(target.as_ref().ok_or(Error::Null)?)?
    };
    out.push(16);
    let start = start.ok_or(Error::Null)?;
    let [sx, sy, _] = start.coordinates();
    out.push(((chunk_offset(sx) << 4) | (chunk_offset(sy) & 0x7)) as u8);
    let end = end.ok_or(Error::Null)?;
    let ex = end.coordinates()[0];
    let sx = start.coordinates()[0];
    out.push(ex.wrapping_sub(sx) as u8);
    let ey = end.coordinates()[1];
    let sy = start.coordinates()[1];
    out.push(ey.wrapping_sub(sy) as u8);
    let index = match &target {
        None => -1,
        Some(t) => {
            let player = h.kind(t) == Kind::Player;
            let i = h.index(t)?.wrapping_add(1);
            if player { i.wrapping_neg() } else { i }
        }
    };
    out.extend_from_slice(&(index as u16).to_be_bytes());
    let f = p.read().clone();
    out.extend_from_slice(&(f.projectile_id as u16).to_be_bytes());
    out.push(f.start_height as u8);
    out.push(f.end_height as u8);
    out.extend_from_slice(&(f.start_delay as u16).to_be_bytes());
    out.extend_from_slice(&(f.speed as u16).to_be_bytes());
    out.push(f.angle as u8);
    out.push(f.distance as u8);
    Ok(())
}
