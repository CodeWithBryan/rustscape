//! ZoneBorders, RegionZone, MapZone's base behaviour, ZoneBuilder.configure,
//! MusicZone and ZoneMonitor from `core.game.world.map.zone`.
//!
//! A library: nothing installs it. Every system the original reaches that has
//! no native port (music player, messages, combat state, timers, locks, the
//! entity's own hooks) is a required `Host` service, called in the original
//! order with the original arguments. No service has a default and no lock is
//! held across a host or zone callback. Zone objects are typed native objects;
//! no label or numeric id selects behaviour.
use super::{doors::Location, player_walk::NodeHandle, region, region_activity};
use crate::definitions::item::Shared;
use std::{
    any::Any,
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// java.lang.NullPointerException.
    Null,
    /// java.lang.ClassCastException, e.g. `(Player) entity` on an NPC.
    ClassCast,
    /// java.util.ConcurrentModificationException from an ArrayList iterator.
    ConcurrentModification,
    NoSuchElement,
    /// java.lang.IllegalStateException.
    IllegalState(String),
    /// java.lang.IllegalArgumentException, e.g. a nonpositive random bound.
    IllegalArgument,
    /// java.lang.ArithmeticException: division by zero.
    Arithmetic,
    /// A native bound where the original would recurse or loop without one.
    Limit,
    /// The region is a DynamicRegion: not ported, never silently accepted.
    Dynamic,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// Native bounds; the original has none of them.
pub const MAX_EXCEPTION_DEPTH: usize = 1_024;
pub const MAX_REGION_IDS: i64 = 65_536;
pub const MAX_DRAWS: i32 = 65_536;
pub const MAX_WALKABLE_TRIES: i32 = 20;

/// java.lang.String.hashCode over UTF-16 code units.
pub fn string_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}
fn bool_hash(value: bool) -> i32 {
    if value { 1231 } else { 1237 }
}
fn objects_hash(parts: &[i32]) -> i32 {
    parts
        .iter()
        .fold(1i32, |hash, part| hash.wrapping_mul(31).wrapping_add(*part))
}
pub fn region_id(location: &Location) -> i32 {
    let [x, y, _] = location.coordinates();
    ((x >> 6) << 8) | (y >> 6)
}

/// java.util.ArrayList as the zone classes use it: a shared handle with a
/// modification count, so exposed lists and iterators behave as the original.
pub struct ArrayList<T> {
    values: Vec<T>,
    mod_count: i32,
}
pub struct List<T>(Shared<ArrayList<T>>);
impl<T> Clone for List<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> Default for List<T> {
    fn default() -> Self {
        Self(Shared::new(ArrayList {
            values: Vec::new(),
            mod_count: 0,
        }))
    }
}
impl<T: Clone> List<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn values(&self) -> Vec<T> {
        self.0.read().values.clone()
    }
    pub fn len(&self) -> usize {
        self.0.read().values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn add(&self, value: T) {
        let mut list = self.0.write();
        list.values.push(value);
        list.mod_count = list.mod_count.wrapping_add(1);
    }
    pub fn remove_at(&self, index: usize) -> Result<T> {
        let mut list = self.0.write();
        if index >= list.values.len() {
            return Err(Error::Binding("IndexOutOfBounds".into()));
        }
        list.mod_count = list.mod_count.wrapping_add(1);
        Ok(list.values.remove(index))
    }
    pub fn clear(&self) {
        let mut list = self.0.write();
        list.values.clear();
        list.mod_count = list.mod_count.wrapping_add(1);
    }
    pub fn iter(&self) -> Cursor {
        Cursor {
            cursor: 0,
            last: None,
            expected: self.0.read().mod_count,
        }
    }
}
/// ArrayList.Itr: `has_next` reads the live size, `next` checks the
/// modification count first, `remove` resynchronizes it.
pub struct Cursor {
    cursor: usize,
    last: Option<usize>,
    expected: i32,
}
impl Cursor {
    pub fn has_next<T: Clone>(&self, list: &List<T>) -> bool {
        self.cursor != list.len()
    }
    pub fn next<T: Clone>(&mut self, list: &List<T>) -> Result<T> {
        let guard = list.0.read();
        if guard.mod_count != self.expected {
            return Err(Error::ConcurrentModification);
        }
        let value = guard
            .values
            .get(self.cursor)
            .cloned()
            .ok_or(Error::NoSuchElement)?;
        self.last = Some(self.cursor);
        self.cursor += 1;
        Ok(value)
    }
    pub fn remove<T: Clone>(&mut self, list: &List<T>) -> Result<()> {
        let index = self
            .last
            .ok_or_else(|| Error::IllegalState(String::new()))?;
        let mut guard = list.0.write();
        if guard.mod_count != self.expected {
            return Err(Error::ConcurrentModification);
        }
        guard.values.remove(index);
        guard.mod_count = guard.mod_count.wrapping_add(1);
        self.cursor = index;
        self.last = None;
        self.expected = guard.mod_count;
        Ok(())
    }
}
/// The enhanced for loop. The closure is called with no list guard held.
pub(crate) fn each<T: Clone, R>(
    list: &List<T>,
    mut body: impl FnMut(T) -> Result<Option<R>>,
) -> Result<Option<R>> {
    let mut cursor = list.iter();
    while cursor.has_next(list) {
        let value = cursor.next(list)?;
        if let Some(found) = body(value)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// ZoneBorders. The four corners are final; plane, the zero-plane flag and
/// the exception list are not, so borders are shared mutable objects.
pub struct ZoneBorders {
    south_west_x: i32,
    south_west_y: i32,
    north_east_x: i32,
    north_east_y: i32,
    plane: i32,
    zero_plane_check: bool,
    exceptions: Option<List<Borders>>,
}
#[derive(Clone)]
pub struct Borders(Shared<ZoneBorders>);
impl Borders {
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self(Shared::new(ZoneBorders {
            south_west_x: x1.min(x2),
            south_west_y: y1.min(y2),
            north_east_x: x1.max(x2),
            north_east_y: y1.max(y2),
            plane: 0,
            zero_plane_check: false,
            exceptions: None,
        }))
    }
    pub fn with_plane(x1: i32, y1: i32, x2: i32, y2: i32, plane: i32) -> Self {
        let borders = Self::new(x1, y1, x2, y2);
        borders.set_plane(plane);
        borders
    }
    pub fn with_plane_check(
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        plane: i32,
        zero_plane_check: bool,
    ) -> Self {
        let borders = Self::with_plane(x1, y1, x2, y2, plane);
        borders.0.write().zero_plane_check = zero_plane_check;
        borders
    }
    /// `ZoneBorders(Location l1, Location l2)`: the plane is the first one's.
    pub fn between(first: &Location, second: &Location) -> Self {
        let ([x1, y1, z], [x2, y2, _]) = (first.coordinates(), second.coordinates());
        Self::with_plane(x1, y1, x2, y2, z)
    }
    pub fn for_region(region_id: i32) -> Self {
        let base_x = ((region_id >> 8) & 0xFF) << 6;
        let base_y = (region_id & 0xFF) << 6;
        Self::new(base_x, base_y, base_x + 63, base_y + 63)
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn south_west_x(&self) -> i32 {
        self.0.read().south_west_x
    }
    pub fn south_west_y(&self) -> i32 {
        self.0.read().south_west_y
    }
    pub fn north_east_x(&self) -> i32 {
        self.0.read().north_east_x
    }
    pub fn north_east_y(&self) -> i32 {
        self.0.read().north_east_y
    }
    pub fn plane(&self) -> i32 {
        self.0.read().plane
    }
    pub fn set_plane(&self, plane: i32) {
        self.0.write().plane = plane;
    }
    pub fn zero_plane_check(&self) -> bool {
        self.0.read().zero_plane_check
    }
    /// The live exception list, or None before the first `add_exception`.
    pub fn exceptions(&self) -> Option<List<Borders>> {
        self.0.read().exceptions.clone()
    }
    pub fn add_exception(&self, exception: Borders) {
        let list = {
            let mut borders = self.0.write();
            borders.exceptions.get_or_insert_with(List::new).clone()
        };
        list.add(exception);
    }
    pub fn inside(&self, x: i32, y: i32, z: i32) -> Result<bool> {
        self.inside_at(x, y, z, 0)
    }
    fn inside_at(&self, x: i32, y: i32, z: i32, depth: usize) -> Result<bool> {
        // The original recurses until the stack overflows on a cyclic list.
        if depth > MAX_EXCEPTION_DEPTH {
            return Err(Error::Limit);
        }
        let exceptions = {
            let b = self.0.read();
            if if b.zero_plane_check {
                z != b.plane
            } else {
                b.plane != 0 && z != b.plane
            } {
                return Ok(false);
            }
            if !(b.south_west_x <= x
                && b.south_west_y <= y
                && b.north_east_x >= x
                && b.north_east_y >= y)
            {
                return Ok(false);
            }
            b.exceptions.clone()
        };
        if let Some(exceptions) = exceptions {
            // `exceptions.toArray()`: a snapshot.
            for exception in exceptions.values() {
                if exception.inside_at(x, y, z, depth + 1)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    /// `insideBorder(int x, int y)`: plane zero.
    pub fn inside_xy(&self, x: i32, y: i32) -> Result<bool> {
        self.inside(x, y, 0)
    }
    pub fn inside_location(&self, location: Option<&Location>) -> Result<bool> {
        let [x, y, z] = location.ok_or(Error::Null)?.coordinates();
        self.inside(x, y, z)
    }
    /// `insideBorder(Node)`: one location read.
    pub fn inside_node<H: Host>(&self, host: &mut H, node: &NodeHandle) -> Result<bool> {
        let location = host.location(node)?;
        self.inside_location(location.as_ref())
    }
    /// `insideRegion(Node)`: two location reads, chunk coordinates, plane 0.
    pub fn inside_region<H: Host>(&self, host: &mut H, node: &NodeHandle) -> Result<bool> {
        let x = host.location(node)?.ok_or(Error::Null)?.coordinates()[0] >> 3;
        let y = host.location(node)?.ok_or(Error::Null)?.coordinates()[1] >> 3;
        self.inside_xy(x, y)
    }
    /// x outer, y inner, `y | x << 8`. Bounded natively to 65,536 ids.
    pub fn region_ids(&self) -> Result<Vec<i32>> {
        let (sw_x, sw_y, ne_x, ne_y) = {
            let b = self.0.read();
            (
                b.south_west_x >> 6,
                b.south_west_y >> 6,
                (b.north_east_x >> 6).wrapping_add(1),
                (b.north_east_y >> 6).wrapping_add(1),
            )
        };
        let columns = (i64::from(ne_x) - i64::from(sw_x)).max(0);
        let rows = (i64::from(ne_y) - i64::from(sw_y)).max(0);
        if columns.saturating_mul(rows) > MAX_REGION_IDS {
            return Err(Error::Limit);
        }
        let mut ids = Vec::new();
        for x in sw_x..ne_x {
            for y in sw_y..ne_y {
                ids.push(y | x.wrapping_shl(8));
            }
        }
        Ok(ids)
    }
    /// `getWeightedRandomLoc`: RandomFunction.RANDOM draws, plane 0.
    pub fn weighted_random_loc<H: Host>(&self, host: &mut H, intensity: i32) -> Result<Location> {
        let (sw_x, sw_y, ne_x, ne_y) = self.corners();
        let x = match ne_x.wrapping_sub(sw_x) {
            0 => sw_x,
            width => normal_rand_dist(host, width, intensity)?.wrapping_add(sw_x),
        };
        let y = match ne_y.wrapping_sub(sw_y) {
            0 => sw_y,
            height => normal_rand_dist(host, height, intensity)?.wrapping_add(sw_y),
        };
        Ok(Location::new(x, y, 0))
    }
    /// `getRandomLoc`: one `new Random()` per coordinate with a width.
    pub fn random_loc<H: Host>(&self, host: &mut H) -> Result<Location> {
        let (sw_x, sw_y, ne_x, ne_y) = self.corners();
        let x = match ne_x.wrapping_sub(sw_x) {
            0 => sw_x,
            width => host
                .fresh_random_next_int(width.wrapping_add(1))?
                .wrapping_add(sw_x),
        };
        let y = match ne_y.wrapping_sub(sw_y) {
            0 => sw_y,
            height => host
                .fresh_random_next_int(height.wrapping_add(1))?
                .wrapping_add(sw_y),
        };
        Ok(Location::new(x, y, self.plane()))
    }
    /// `getRandomWalkableLoc`: at most twenty further draws.
    pub fn random_walkable_loc<H: Host>(&self, host: &mut H) -> Result<Location> {
        let mut location = self.random_loc(host)?;
        let mut tries = 0;
        while !host.teleport_permitted(&location)? && tries < MAX_WALKABLE_TRIES {
            location = self.random_loc(host)?;
            tries += 1;
        }
        Ok(location)
    }
    fn corners(&self) -> (i32, i32, i32, i32) {
        let b = self.0.read();
        (
            b.south_west_x,
            b.south_west_y,
            b.north_east_x,
            b.north_east_y,
        )
    }
    pub fn equals(&self, other: &Self) -> Result<bool> {
        self.equals_at(other, 0)
    }
    fn equals_at(&self, other: &Self, depth: usize) -> Result<bool> {
        if self.same(other) {
            return Ok(true);
        }
        if depth > MAX_EXCEPTION_DEPTH {
            return Err(Error::Limit);
        }
        let (mine, theirs) = {
            let (a, b) = (self.0.read(), other.0.read());
            if !(a.south_west_x == b.south_west_x
                && a.south_west_y == b.south_west_y
                && a.north_east_x == b.north_east_x
                && a.north_east_y == b.north_east_y
                && a.plane == b.plane
                && a.zero_plane_check == b.zero_plane_check)
            {
                return Ok(false);
            }
            (a.exceptions.clone(), b.exceptions.clone())
        };
        match (mine, theirs) {
            (None, None) => Ok(true),
            (Some(mine), Some(theirs)) => {
                if mine.same(&theirs) {
                    return Ok(true);
                }
                let (mine, theirs) = (mine.values(), theirs.values());
                if mine.len() != theirs.len() {
                    return Ok(false);
                }
                for (a, b) in mine.iter().zip(&theirs) {
                    if !a.equals_at(b, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    pub fn hash_code(&self) -> Result<i32> {
        self.hash_at(0)
    }
    fn hash_at(&self, depth: usize) -> Result<i32> {
        if depth > MAX_EXCEPTION_DEPTH {
            return Err(Error::Limit);
        }
        let (parts, exceptions) = {
            let b = self.0.read();
            (
                [
                    b.south_west_x,
                    b.south_west_y,
                    b.north_east_x,
                    b.north_east_y,
                    b.plane,
                    bool_hash(b.zero_plane_check),
                ],
                b.exceptions.clone(),
            )
        };
        let list = match exceptions {
            None => 0,
            Some(list) => {
                let mut hash = 1i32;
                for exception in list.values() {
                    hash = hash
                        .wrapping_mul(31)
                        .wrapping_add(exception.hash_at(depth + 1)?);
                }
                hash
            }
        };
        Ok(objects_hash(&[
            parts[0], parts[1], parts[2], parts[3], parts[4], list, parts[5],
        ]))
    }
    pub fn java_string(&self) -> Result<String> {
        self.string_at(0)
    }
    fn string_at(&self, depth: usize) -> Result<String> {
        if depth > MAX_EXCEPTION_DEPTH {
            return Err(Error::Limit);
        }
        let (sw_x, sw_y, ne_x, ne_y) = self.corners();
        let exceptions = match self.exceptions() {
            None => "null".to_owned(),
            Some(list) => {
                let mut parts = Vec::new();
                for exception in list.values() {
                    parts.push(exception.string_at(depth + 1)?);
                }
                format!("[{}]", parts.join(", "))
            }
        };
        Ok(format!(
            "ZoneBorders [southWestX={sw_x}, southWestY={sw_y}, northEastX={ne_x}, northEastY={ne_y}, exceptions={exceptions}]"
        ))
    }
}
/// RandomFunction.normalRandDist(max, intensity). The draw loop is bounded
/// natively to 65,536 draws.
pub fn normal_rand_dist<H: Host>(host: &mut H, max: i32, intensity: i32) -> Result<i32> {
    if intensity > MAX_DRAWS {
        return Err(Error::Limit);
    }
    let mut sum = 0i32;
    for _ in 0..intensity.max(0) {
        sum = sum.wrapping_add(host.random_next_int(max)?);
    }
    if intensity == 0 {
        return Err(Error::Arithmetic);
    }
    Ok(sum.wrapping_div(intensity))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restriction {
    Followers,
    RandomEvents,
    Fires,
    Members,
    Cannon,
    Graves,
    Teleport,
    OffMap,
}
impl Restriction {
    pub const ALL: [Self; 8] = [
        Self::Followers,
        Self::RandomEvents,
        Self::Fires,
        Self::Members,
        Self::Cannon,
        Self::Graves,
        Self::Teleport,
        Self::OffMap,
    ];
    pub fn flag(self) -> i32 {
        1 << (self as i32)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneType {
    Default,
    Safe,
    Poh,
    CastleWars,
    TroubleBrewing,
    BarbarianAssault,
}
impl ZoneType {
    pub const ALL: [Self; 6] = [
        Self::Default,
        Self::Safe,
        Self::Poh,
        Self::CastleWars,
        Self::TroubleBrewing,
        Self::BarbarianAssault,
    ];
    pub fn id(self) -> i32 {
        self as i32
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Player,
    Npc,
    /// An NPC that is a Familiar.
    Familiar,
    Other,
}
impl EntityKind {
    /// `instanceof NPC`.
    pub fn is_npc(self) -> bool {
        matches!(self, Self::Npc | Self::Familiar)
    }
}

/// ZoneMonitor.MID_WILDY_TELEPORT_JEWELLERY (ConstLib item ids).
pub const MID_WILDY_TELEPORT_JEWELLERY: [i32; 17] = [
    1704, 1706, 1708, 1710, 10362, 10360, 10358, 10356, 11113, 11111, 11109, 11107, 11126, 11124,
    11122, 11120, 2570,
];

/// Required services. None has a default; an owning world answers each from
/// the real system or fails. `kind`, `monitor`, `region` and `target_entity`
/// are lookups (`instanceof`, a field read, RegionManager.forId).
pub trait Host: Sized + 'static {
    /// A `core.game.node.Node` argument (entity, item or scenery).
    type Target: Clone;
    /// Option, CombatStyle, RequestType and used-Item arguments: handed to the
    /// zone callbacks untouched.
    type Opaque;
    fn kind(&mut self, entity: &NodeHandle) -> Result<EntityKind>;
    fn monitor(&mut self, entity: &NodeHandle) -> Result<Monitor<Self>>;
    /// RegionManager.forId: the cached region, created when absent.
    fn region(&mut self, region_id: i32) -> Result<region::Handle>;
    /// `target instanceof Entity`, and the entity when it is one.
    fn target_entity(&mut self, target: &Self::Target) -> Result<Option<NodeHandle>>;
    fn location(&mut self, entity: &NodeHandle) -> Result<Option<Location>>;
    fn is_artificial(&mut self, player: &NodeHandle) -> Result<bool>;
    /// Entity.updateLocation(last).
    fn entity_update_location(
        &mut self,
        entity: &NodeHandle,
        last: Option<&Location>,
    ) -> Result<()>;
    fn set_invisible(&mut self, npc: &NodeHandle, invisible: bool) -> Result<()>;
    /// Entity.continueAttack(target, style, message).
    fn entity_continue_attack(
        &mut self,
        entity: &NodeHandle,
        target: &NodeHandle,
        style: Option<&Self::Opaque>,
        message: bool,
    ) -> Result<bool>;
    fn skull_wilderness(&mut self, player: &NodeHandle) -> Result<bool>;
    fn skull_level(&mut self, player: &NodeHandle) -> Result<i32>;
    /// `player.getPacketDispatch().sendMessage`.
    fn packet_message(&mut self, player: &NodeHandle, message: &str) -> Result<()>;
    /// `player.sendMessage`.
    fn player_message(&mut self, player: &NodeHandle, message: &str) -> Result<()>;
    /// `entity.timers.getTimer(name) != null`.
    fn has_timer(&mut self, entity: &NodeHandle, name: &str) -> Result<bool>;
    fn teleport_locked(&mut self, entity: &NodeHandle) -> Result<bool>;
    /// `node.asItem().getId()`; the service owns the null and cast failures.
    fn target_item_id(&mut self, target: Option<&Self::Target>) -> Result<i32>;
    /// System.currentTimeMillis().
    fn now_millis(&mut self) -> Result<i64>;
    fn multi_zone(&mut self, entity: &NodeHandle) -> Result<bool>;
    /// Entity.isIgnoreMultiBoundaries(victim).
    fn entity_ignore_multi(&mut self, entity: &NodeHandle, victim: &NodeHandle) -> Result<bool>;
    fn familiar_owner(&mut self, familiar: &NodeHandle) -> Result<Option<NodeHandle>>;
    /// The stored "combat-attacker" attribute; the caller applies the default.
    fn combat_attacker(&mut self, entity: &NodeHandle) -> Result<Option<NodeHandle>>;
    /// The stored "combat-time" attribute; the caller applies -1.
    fn combat_time(&mut self, entity: &NodeHandle) -> Result<Option<i64>>;
    fn active(&mut self, entity: &NodeHandle) -> Result<bool>;
    fn id(&mut self, entity: &NodeHandle) -> Result<i32>;
    fn music_playing(&mut self, player: &NodeHandle) -> Result<bool>;
    fn music_play_default(&mut self, player: &NodeHandle) -> Result<()>;
    /// MusicPlayer.unlock(id) when `play` is None, unlock(id, play) otherwise.
    fn music_unlock(&mut self, player: &NodeHandle, id: i32, play: Option<bool>) -> Result<()>;
    /// RandomFunction.RANDOM.nextInt(bound).
    fn random_next_int(&mut self, bound: i32) -> Result<i32>;
    /// `new Random().nextInt(bound)`: a generator nobody else sees.
    fn fresh_random_next_int(&mut self, bound: i32) -> Result<i32>;
    /// RegionManager.isTeleportPermitted.
    fn teleport_permitted(&mut self, location: &Location) -> Result<bool>;
}

/// The overridable members of MapZone. Each default is the base class body.
#[allow(clippy::too_many_arguments)]
pub trait Behavior<H: Host>: Send + Sync + 'static {
    /// The concrete type stands for `getClass()` in MapZone.equals.
    fn as_any(&self) -> &dyn Any;
    fn enter(&self, zone: &Zone<H>, host: &mut H, entity: &NodeHandle) -> Result<bool> {
        base_enter(zone, host, entity)
    }
    fn leave(&self, _: &Zone<H>, _: &mut H, _: &NodeHandle, _logout: bool) -> Result<bool> {
        Ok(true)
    }
    fn can_logout(&self, _: &Zone<H>, _: &mut H, _player: &NodeHandle) -> Result<bool> {
        Ok(true)
    }
    fn death(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _entity: &NodeHandle,
        _killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(false)
    }
    fn interact(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _entity: &NodeHandle,
        _target: Option<&H::Target>,
        _option: Option<&H::Opaque>,
    ) -> Result<bool> {
        Ok(false)
    }
    fn handle_use_with(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _player: &NodeHandle,
        _used: Option<&H::Opaque>,
        _with: Option<&H::Target>,
    ) -> Result<bool> {
        Ok(false)
    }
    fn action_button(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _player: &NodeHandle,
        _interface_id: i32,
        _button_id: i32,
        _slot: i32,
        _item_id: i32,
        _opcode: i32,
    ) -> Result<bool> {
        Ok(false)
    }
    fn continue_pvp(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _entity: Option<&NodeHandle>,
        _target: Option<&H::Target>,
        _style: Option<&H::Opaque>,
        _message: bool,
    ) -> Result<bool> {
        Ok(false)
    }
    fn continue_attack(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _entity: &NodeHandle,
        _target: Option<&H::Target>,
        _style: Option<&H::Opaque>,
        _message: bool,
    ) -> Result<bool> {
        Ok(true)
    }
    fn ignore_multi_boundaries(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _attacker: &NodeHandle,
        _victim: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(false)
    }
    fn teleport(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _entity: &NodeHandle,
        _kind: i32,
        _node: Option<&H::Target>,
    ) -> Result<bool> {
        Ok(true)
    }
    fn start_death(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _entity: Option<&NodeHandle>,
        _killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(true)
    }
    fn can_request(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _request: Option<&H::Opaque>,
        _player: &NodeHandle,
        _target: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(true)
    }
    fn move_entity(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _entity: &NodeHandle,
        _from: Option<&Location>,
        _to: Option<&Location>,
    ) -> Result<bool> {
        Ok(true)
    }
    fn parse_command(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _player: Option<&NodeHandle>,
        _name: Option<&str>,
        _arguments: &[String],
    ) -> Result<bool> {
        Ok(false)
    }
    fn location_update(
        &self,
        _: &Zone<H>,
        _: &mut H,
        _entity: &NodeHandle,
        _last: Option<&Location>,
    ) -> Result<()> {
        Ok(())
    }
    fn configure(&self, _: &Zone<H>, _: &mut H) -> Result<()> {
        Ok(())
    }
    fn is_dynamic_zone(&self, _: &Zone<H>, _: &mut H) -> Result<bool> {
        Ok(false)
    }
}
/// MapZone.enter: only a Familiar in a zone restricting followers is touched.
pub fn base_enter<H: Host>(zone: &Zone<H>, host: &mut H, entity: &NodeHandle) -> Result<bool> {
    if host.kind(entity)? == EntityKind::Familiar
        && zone.is_restricted(Restriction::Followers.flag())
    {
        host.set_invisible(entity, true)?;
    }
    Ok(true)
}
/// MapZone.message: players only.
pub fn message<H: Host>(host: &mut H, entity: &NodeHandle, text: &str) -> Result<()> {
    if host.kind(entity)? != EntityKind::Player {
        return Ok(());
    }
    host.packet_message(entity, text)
}

struct Fields {
    uid: i32,
    name: Option<String>,
    overlappable: bool,
    fire_random_events: bool,
    restriction: i32,
    zone_type: i32,
}
struct Object<H: Host> {
    fields: RwLock<Fields>,
    behavior: Box<dyn Behavior<H>>,
}
/// A MapZone object: the base fields and the subclass behaviour.
pub struct Zone<H: Host>(Arc<Object<H>>);
impl<H: Host> Clone for Zone<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
#[allow(clippy::too_many_arguments)]
impl<H: Host> Zone<H> {
    /// `MapZone(name, overlappable, restrictions...)`. Random events start
    /// off, as the field default is false.
    pub fn new(
        name: Option<&str>,
        overlappable: bool,
        restrictions: &[Restriction],
        behavior: impl Behavior<H>,
    ) -> Self {
        Self(Arc::new(Object {
            fields: RwLock::new(Fields {
                uid: 0,
                name: name.map(str::to_owned),
                overlappable,
                fire_random_events: false,
                restriction: restrictions.iter().fold(0, |all, r| all | r.flag()),
                zone_type: 0,
            }),
            behavior: Box::new(behavior),
        }))
    }
    fn fields(&self) -> std::sync::RwLockReadGuard<'_, Fields> {
        self.0.fields.read().unwrap_or_else(|e| e.into_inner())
    }
    fn fields_mut(&self) -> std::sync::RwLockWriteGuard<'_, Fields> {
        self.0.fields.write().unwrap_or_else(|e| e.into_inner())
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn behavior<T: 'static>(&self) -> Option<&T> {
        self.0.behavior.as_any().downcast_ref()
    }
    pub fn name(&self) -> Option<String> {
        self.fields().name.clone()
    }
    pub fn set_name(&self, name: Option<&str>) {
        self.fields_mut().name = name.map(str::to_owned);
    }
    pub fn is_overlappable(&self) -> bool {
        self.fields().overlappable
    }
    pub fn set_overlappable(&self, overlappable: bool) {
        self.fields_mut().overlappable = overlappable;
    }
    /// `getUid()`: always the name's hash, never the stored field.
    pub fn uid(&self) -> Result<i32> {
        self.fields()
            .name
            .as_deref()
            .map(string_hash)
            .ok_or(Error::Null)
    }
    pub fn set_uid(&self, uid: i32) {
        self.fields_mut().uid = uid;
    }
    pub fn is_fire_randoms(&self) -> bool {
        self.fields().fire_random_events
    }
    /// The protected field; subclasses write it directly.
    pub fn set_fire_random_events(&self, fire: bool) {
        self.fields_mut().fire_random_events = fire;
    }
    pub fn disable_random_events(&self) {
        self.set_fire_random_events(false);
    }
    pub fn add_restriction(&self, flag: i32) {
        self.fields_mut().restriction |= flag;
    }
    pub fn is_restricted(&self, flag: i32) -> bool {
        self.fields().restriction & flag != 0
    }
    pub fn restriction(&self) -> i32 {
        self.fields().restriction
    }
    pub fn zone_type(&self) -> i32 {
        self.fields().zone_type
    }
    pub fn set_zone_type(&self, zone_type: i32) {
        self.fields_mut().zone_type = zone_type;
    }
    /// MapZone.equals: same object, or same class and equal name hashes.
    pub fn equals(&self, other: &Self) -> Result<bool> {
        if self.same(other) {
            return Ok(true);
        }
        if self.0.behavior.as_any().type_id() != other.0.behavior.as_any().type_id() {
            return Ok(false);
        }
        Ok(self.uid()? == other.uid()?)
    }
    /// MapZone.hashCode: the stored uid field, not `getUid()`.
    pub fn hash_code(&self) -> i32 {
        let f = self.fields();
        objects_hash(&[
            f.uid,
            f.name.as_deref().map_or(0, string_hash),
            bool_hash(f.overlappable),
            bool_hash(f.fire_random_events),
            f.restriction,
            f.zone_type,
        ])
    }
    pub fn enter(&self, host: &mut H, entity: &NodeHandle) -> Result<bool> {
        self.0.behavior.enter(self, host, entity)
    }
    pub fn leave(&self, host: &mut H, entity: &NodeHandle, logout: bool) -> Result<bool> {
        self.0.behavior.leave(self, host, entity, logout)
    }
    pub fn can_logout(&self, host: &mut H, player: &NodeHandle) -> Result<bool> {
        self.0.behavior.can_logout(self, host, player)
    }
    pub fn death(
        &self,
        host: &mut H,
        entity: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        self.0.behavior.death(self, host, entity, killer)
    }
    pub fn interact(
        &self,
        host: &mut H,
        entity: &NodeHandle,
        target: Option<&H::Target>,
        option: Option<&H::Opaque>,
    ) -> Result<bool> {
        self.0.behavior.interact(self, host, entity, target, option)
    }
    pub fn handle_use_with(
        &self,
        host: &mut H,
        player: &NodeHandle,
        used: Option<&H::Opaque>,
        with: Option<&H::Target>,
    ) -> Result<bool> {
        self.0
            .behavior
            .handle_use_with(self, host, player, used, with)
    }
    pub fn action_button(
        &self,
        host: &mut H,
        player: &NodeHandle,
        interface_id: i32,
        button_id: i32,
        slot: i32,
        item_id: i32,
        opcode: i32,
    ) -> Result<bool> {
        self.0.behavior.action_button(
            self,
            host,
            player,
            interface_id,
            button_id,
            slot,
            item_id,
            opcode,
        )
    }
    pub fn continue_pvp(
        &self,
        host: &mut H,
        entity: Option<&NodeHandle>,
        target: Option<&H::Target>,
        style: Option<&H::Opaque>,
        message: bool,
    ) -> Result<bool> {
        self.0
            .behavior
            .continue_pvp(self, host, entity, target, style, message)
    }
    pub fn continue_attack(
        &self,
        host: &mut H,
        entity: &NodeHandle,
        target: Option<&H::Target>,
        style: Option<&H::Opaque>,
        message: bool,
    ) -> Result<bool> {
        self.0
            .behavior
            .continue_attack(self, host, entity, target, style, message)
    }
    pub fn ignore_multi_boundaries(
        &self,
        host: &mut H,
        attacker: &NodeHandle,
        victim: Option<&NodeHandle>,
    ) -> Result<bool> {
        self.0
            .behavior
            .ignore_multi_boundaries(self, host, attacker, victim)
    }
    pub fn teleport(
        &self,
        host: &mut H,
        entity: &NodeHandle,
        kind: i32,
        node: Option<&H::Target>,
    ) -> Result<bool> {
        self.0.behavior.teleport(self, host, entity, kind, node)
    }
    pub fn start_death(
        &self,
        host: &mut H,
        entity: Option<&NodeHandle>,
        killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        self.0.behavior.start_death(self, host, entity, killer)
    }
    pub fn can_request(
        &self,
        host: &mut H,
        request: Option<&H::Opaque>,
        player: &NodeHandle,
        target: Option<&NodeHandle>,
    ) -> Result<bool> {
        self.0
            .behavior
            .can_request(self, host, request, player, target)
    }
    pub fn move_entity(
        &self,
        host: &mut H,
        entity: &NodeHandle,
        from: Option<&Location>,
        to: Option<&Location>,
    ) -> Result<bool> {
        self.0.behavior.move_entity(self, host, entity, from, to)
    }
    pub fn parse_command(
        &self,
        host: &mut H,
        player: Option<&NodeHandle>,
        name: Option<&str>,
        arguments: &[String],
    ) -> Result<bool> {
        self.0
            .behavior
            .parse_command(self, host, player, name, arguments)
    }
    pub fn location_update(
        &self,
        host: &mut H,
        entity: &NodeHandle,
        last: Option<&Location>,
    ) -> Result<()> {
        self.0.behavior.location_update(self, host, entity, last)
    }
    pub fn is_dynamic_zone(&self, host: &mut H) -> Result<bool> {
        self.0.behavior.is_dynamic_zone(self, host)
    }
    /// ZoneBuilder.configure: the uid field, then the zone's own `configure`.
    pub fn configure(&self, host: &mut H) -> Result<()> {
        let uid = self.uid()?;
        self.set_uid(uid);
        self.0.behavior.configure(self, host)
    }
    /// MapZone.register: one new RegionZone per region id of the borders, in
    /// `getRegionIds` order, each added through Region.add.
    pub fn register(&self, host: &mut H, borders: &Borders) -> Result<()> {
        for id in borders.region_ids()? {
            let region = host.region(id)?;
            region_activity::add_zone(
                host,
                &region,
                RegionZone::new(self.clone(), borders.clone()),
            )?;
        }
        Ok(())
    }
    /// MapZone.unregister: Region.remove of an equal RegionZone per region.
    pub fn unregister(&self, host: &mut H, borders: &Borders) -> Result<()> {
        for id in borders.region_ids()? {
            let region = host.region(id)?;
            region_activity::remove_zone(
                host,
                &region,
                &RegionZone::new(self.clone(), borders.clone()),
            )?;
        }
        Ok(())
    }
    pub fn register_region(&self, host: &mut H, region_id: i32) -> Result<()> {
        self.register(host, &Borders::for_region(region_id))
    }
    pub fn register_region_borders(
        &self,
        host: &mut H,
        region_id: i32,
        borders: &Borders,
    ) -> Result<()> {
        let region = host.region(region_id)?;
        region_activity::add_zone(
            host,
            &region,
            RegionZone::new(self.clone(), borders.clone()),
        )
    }
    /// MapZone.unregisterRegion: removes by zone identity through the list
    /// iterator; no entity is told.
    pub fn unregister_region(&self, host: &mut H, region_id: i32) -> Result<()> {
        let region = host.region(region_id)?;
        let list = region_activity::region_zones::<H>(&region)?;
        let mut cursor = list.iter();
        while cursor.has_next(&list) {
            if cursor.next(&list)?.zone().same(self) {
                cursor.remove(&list)?;
            }
        }
        Ok(())
    }
}

struct RegionZoneFields<H: Host> {
    zone: Zone<H>,
    borders: Borders,
}
/// RegionZone: an immutable pair with object identity.
pub struct RegionZone<H: Host>(Arc<RegionZoneFields<H>>);
impl<H: Host> Clone for RegionZone<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> RegionZone<H> {
    pub fn new(zone: Zone<H>, borders: Borders) -> Self {
        Self(Arc::new(RegionZoneFields { zone, borders }))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn zone(&self) -> &Zone<H> {
        &self.0.zone
    }
    pub fn borders(&self) -> &Borders {
        &self.0.borders
    }
    pub fn equals(&self, other: &Self) -> Result<bool> {
        if self.same(other) {
            return Ok(true);
        }
        Ok(self.0.zone.equals(&other.0.zone)? && self.0.borders.equals(&other.0.borders)?)
    }
    pub fn hash_code(&self) -> Result<i32> {
        Ok(objects_hash(&[
            self.0.zone.hash_code(),
            self.0.borders.hash_code()?,
        ]))
    }
}

/// MusicZone: a track id and its borders.
#[derive(Clone)]
pub struct MusicZone(Arc<(i32, Borders)>);
impl MusicZone {
    pub fn new(music_id: i32, borders: Borders) -> Self {
        Self(Arc::new((music_id, borders)))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn music_id(&self) -> i32 {
        self.0.0
    }
    pub fn borders(&self) -> &Borders {
        &self.0.1
    }
    pub fn enter<H: Host>(&self, host: &mut H, entity: &NodeHandle) -> Result<bool> {
        if host.kind(entity)? != EntityKind::Player {
            return Err(Error::IllegalState("Music is for players only!".into()));
        }
        host.music_unlock(entity, self.music_id(), None)?;
        Ok(true)
    }
    pub fn leave(&self, _logout: bool) -> bool {
        true
    }
}

struct MonitorFields<H: Host> {
    entity: NodeHandle,
    zones: List<RegionZone<H>>,
    music_zones: List<MusicZone>,
}
/// ZoneMonitor. It holds a node lease for its entity, as the original holds
/// the entity; an owning world drops the monitor with the entity.
pub struct Monitor<H: Host>(Arc<MonitorFields<H>>);
impl<H: Host> Clone for Monitor<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
#[allow(clippy::too_many_arguments)]
impl<H: Host> Monitor<H> {
    pub fn new(entity: NodeHandle) -> Self {
        Self(Arc::new(MonitorFields {
            entity,
            zones: List::new(),
            music_zones: List::new(),
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn entity(&self) -> &NodeHandle {
        &self.0.entity
    }
    /// `getZones()`: the live list.
    pub fn zones(&self) -> List<RegionZone<H>> {
        self.0.zones.clone()
    }
    /// `getMusicZones()`: the live list.
    pub fn music_zones(&self) -> List<MusicZone> {
        self.0.music_zones.clone()
    }
    fn player(&self, host: &mut H) -> Result<&NodeHandle> {
        if host.kind(&self.0.entity)? != EntityKind::Player {
            return Err(Error::ClassCast);
        }
        Ok(&self.0.entity)
    }
    pub fn get_type(&self) -> Result<i32> {
        Ok(each(&self.0.zones, |z| {
            Ok(Some(z.zone().zone_type()).filter(|kind| *kind != 0))
        })?
        .unwrap_or(0))
    }
    pub fn can_logout(&self, host: &mut H) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            let player = self.player(host)?.clone();
            Ok((!z.zone().can_logout(host, &player)?).then_some(()))
        })?
        .is_none())
    }
    pub fn is_restricted(&self, flag: i32) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok(z.zone().is_restricted(flag).then_some(()))
        })?
        .is_some())
    }
    pub fn handle_death(&self, host: &mut H, killer: Option<&NodeHandle>) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok(z.zone().death(host, &self.0.entity, killer)?.then_some(()))
        })?
        .is_some())
    }
    pub fn is_pvpable(
        &self,
        host: &mut H,
        player: Option<&NodeHandle>,
        target: Option<&H::Target>,
        style: Option<&H::Opaque>,
        message: bool,
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok(z.zone()
                .continue_pvp(host, player, target, style, message)?
                .then_some(()))
        })?
        .is_some())
    }
    pub fn continue_attack(
        &self,
        host: &mut H,
        target: Option<&H::Target>,
        style: Option<&H::Opaque>,
        message: bool,
    ) -> Result<bool> {
        let entity = &self.0.entity;
        let target_entity = match target {
            Some(target) => host.target_entity(target)?,
            None => None,
        };
        if let Some(other) = &target_entity {
            if !host.entity_continue_attack(entity, other, style, message)? {
                return Ok(false);
            }
        }
        if each(&self.0.zones, |z| {
            Ok((!z
                .zone()
                .continue_attack(host, entity, target, style, message)?)
            .then_some(()))
        })?
        .is_some()
        {
            return Ok(false);
        }
        if let Some(other) = &target_entity {
            if host.kind(entity)? == EntityKind::Player
                && host.kind(other)? == EntityKind::Player
                && (!host.skull_wilderness(entity)? || !host.skull_wilderness(other)?)
            {
                if message {
                    host.packet_message(
                        entity,
                        "You can only attack other players in the wilderness.",
                    )?;
                }
                return Ok(false);
            }
            if !check_multi(host, entity, other, message)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn interact(
        &self,
        host: &mut H,
        target: Option<&H::Target>,
        option: Option<&H::Opaque>,
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok(z.zone()
                .interact(host, &self.0.entity, target, option)?
                .then_some(()))
        })?
        .is_some())
    }
    pub fn use_with(
        &self,
        host: &mut H,
        used: Option<&H::Opaque>,
        with: Option<&H::Target>,
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            let player = self.player(host)?.clone();
            Ok(z.zone()
                .handle_use_with(host, &player, used, with)?
                .then_some(()))
        })?
        .is_some())
    }
    pub fn click_button(
        &self,
        host: &mut H,
        interface_id: i32,
        button_id: i32,
        slot: i32,
        item_id: i32,
        opcode: i32,
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            let player = self.player(host)?.clone();
            Ok(z.zone()
                .action_button(
                    host,
                    &player,
                    interface_id,
                    button_id,
                    slot,
                    item_id,
                    opcode,
                )?
                .then_some(()))
        })?
        .is_some())
    }
    pub fn is_ignore_multi_boundaries(
        &self,
        host: &mut H,
        victim: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok(z.zone()
                .ignore_multi_boundaries(host, &self.0.entity, victim)?
                .then_some(()))
        })?
        .is_some())
    }
    /// `teleport(type, node)`: the tele-block refusal, then every zone.
    pub fn teleport(&self, host: &mut H, kind: i32, node: Option<&H::Target>) -> Result<bool> {
        let entity = &self.0.entity;
        if kind != -1
            && is_tele_blocked(host, entity)?
            && !self.can_teleport_by_jewellery(host, kind, node)?
        {
            if host.kind(entity)? == EntityKind::Player {
                host.player_message(entity, "A magical force has stopped you from teleporting.")?;
            }
            return Ok(false);
        }
        Ok(each(&self.0.zones, |z| {
            Ok((!z.zone().teleport(host, entity, kind, node)?).then_some(()))
        })?
        .is_none())
    }
    fn can_teleport_by_jewellery(
        &self,
        host: &mut H,
        kind: i32,
        node: Option<&H::Target>,
    ) -> Result<bool> {
        let entity = &self.0.entity;
        if kind != 1 || !MID_WILDY_TELEPORT_JEWELLERY.contains(&host.target_item_id(node)?) {
            return Ok(false);
        }
        if host.has_timer(entity, "teleblock")? {
            return Ok(false);
        }
        if host
            .monitor(entity)?
            .is_restricted(Restriction::Teleport.flag())?
        {
            return Ok(false);
        }
        if host.teleport_locked(entity)? && host.kind(entity)? == EntityKind::Player {
            return Ok(host.skull_level(entity)? >= 1 && host.skull_level(entity)? <= 30);
        }
        Ok(false)
    }
    pub fn start_death(
        &self,
        host: &mut H,
        entity: Option<&NodeHandle>,
        killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok((!z.zone().start_death(host, entity, killer)?).then_some(()))
        })?
        .is_none())
    }
    pub fn can_fire_random_event(&self) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok((!z.zone().is_fire_randoms()).then_some(()))
        })?
        .is_none())
    }
    /// `clear()`: a refusing zone leaves both lists as they are.
    pub fn clear(&self, host: &mut H) -> Result<bool> {
        if each(&self.0.zones, |z| {
            Ok((!z.zone().leave(host, &self.0.entity, true)?).then_some(()))
        })?
        .is_some()
        {
            return Ok(false);
        }
        each(&self.0.music_zones, |z| {
            z.leave(true);
            Ok(None::<()>)
        })?;
        self.0.zones.clear();
        self.0.music_zones.clear();
        Ok(true)
    }
    pub fn move_entity(
        &self,
        host: &mut H,
        location: Option<&Location>,
        destination: Option<&Location>,
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok((!z
                .zone()
                .move_entity(host, &self.0.entity, location, destination)?)
            .then_some(()))
        })?
        .is_none())
    }
    /// `updateLocation(last)`: music, the entity's own hook, leaving in list
    /// order, then entering in the region's registration order. A refusal or
    /// a failure keeps everything done before it.
    pub fn update_location(&self, host: &mut H, last: Option<&Location>) -> Result<bool> {
        let entity = &self.0.entity;
        if host.kind(entity)? == EntityKind::Player && !host.is_artificial(entity)? {
            self.check_music_zones(host)?;
        }
        host.entity_update_location(entity, last)?;
        let zones = &self.0.zones;
        let mut cursor = zones.iter();
        while cursor.has_next(zones) {
            let zone = cursor.next(zones)?;
            if !zone.borders().inside_node(host, entity)? {
                if zone.zone().is_dynamic_zone(host)? {
                    continue;
                }
                if !zone.zone().leave(host, entity, false)? {
                    return Ok(false);
                }
                cursor.remove(zones)?;
            }
        }
        let location = host.location(entity)?.ok_or(Error::Null)?;
        let region = host.region(region_id(&location))?;
        let region_zones = region_activity::region_zones::<H>(&region)?;
        let refused = each(&region_zones, |zone| {
            if !zone.borders().inside_node(host, entity)? {
                return Ok(None);
            }
            let already_entered =
                each(zones, |z| Ok(z.zone().same(zone.zone()).then_some(())))?.is_some();
            if already_entered {
                zone.zone().location_update(host, entity, last)?;
                return Ok(None);
            }
            if !zone.zone().enter(host, entity)? {
                return Ok(Some(()));
            }
            zones.add(zone.clone());
            zone.zone().location_update(host, entity, last)?;
            Ok(None)
        })?;
        Ok(refused.is_none())
    }
    /// `checkMusicZones()`.
    pub fn check_music_zones(&self, host: &mut H) -> Result<()> {
        let player = &self.0.entity;
        if host.kind(player)? != EntityKind::Player {
            return Ok(());
        }
        let location = host.location(player)?;
        let list = &self.0.music_zones;
        let mut cursor = list.iter();
        while cursor.has_next(list) {
            let zone = cursor.next(list)?;
            let [x, y, _] = location.as_ref().ok_or(Error::Null)?.coordinates();
            if !zone.borders().inside_xy(x, y)? && zone.leave(false) {
                cursor.remove(list)?;
            }
        }
        let second = host.location(player)?.ok_or(Error::Null)?;
        let region = host.region(region_id(&second))?;
        let music_zones = region.read().activity.read().music_zones.clone();
        if let Some(zone) = each(&music_zones, |zone| {
            let [x, y, _] = location.as_ref().ok_or(Error::Null)?.coordinates();
            Ok(zone.borders().inside_xy(x, y)?.then_some(zone))
        })? {
            zone.enter(host, player)?;
            return Ok(());
        }
        // `r.getMusic()` is read after the loop.
        let music = region.read().activity.read().music;
        if music == -1 {
            if !host.music_playing(player)? {
                host.music_play_default(player)?;
            }
        } else {
            host.music_unlock(player, music, Some(true))?;
        }
        Ok(())
    }
    pub fn parse_command(
        &self,
        host: &mut H,
        player: Option<&NodeHandle>,
        name: Option<&str>,
        arguments: &[String],
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            Ok(z.zone()
                .parse_command(host, player, name, arguments)?
                .then_some(()))
        })?
        .is_some())
    }
    pub fn can_request(
        &self,
        host: &mut H,
        request: Option<&H::Opaque>,
        target: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(each(&self.0.zones, |z| {
            let player = self.player(host)?.clone();
            Ok((!z.zone().can_request(host, request, &player, target)?).then_some(()))
        })?
        .is_none())
    }
    pub fn is_in_zone(&self, name: Option<&str>) -> Result<bool> {
        let uid = string_hash(name.ok_or(Error::Null)?);
        Ok(each(
            &self.0.zones,
            |z| Ok((z.zone().uid()? == uid).then_some(())),
        )?
        .is_some())
    }
    /// `remove(MapZone)`: the first entry of that zone object.
    pub fn remove(&self, zone: &Zone<H>) -> Result<()> {
        let list = &self.0.zones;
        let mut cursor = list.iter();
        while cursor.has_next(list) {
            if cursor.next(list)?.zone().same(zone) {
                cursor.remove(list)?;
                break;
            }
        }
        Ok(())
    }
}

/// Entity.isTeleBlocked: timer, then lock, then the monitor's restriction.
pub fn is_tele_blocked<H: Host>(host: &mut H, entity: &NodeHandle) -> Result<bool> {
    Ok(host.has_timer(entity, "teleblock")?
        || host.teleport_locked(entity)?
        || host
            .monitor(entity)?
            .is_restricted(Restriction::Teleport.flag())?)
}

fn owner_of<H: Host>(host: &mut H, entity: Option<NodeHandle>) -> Result<Option<NodeHandle>> {
    match entity {
        Some(entity) if host.kind(&entity)? == EntityKind::Familiar => host.familiar_owner(&entity),
        other => Ok(other),
    }
}
fn identical(a: &Option<NodeHandle>, b: &Option<NodeHandle>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a == b,
        (None, None) => true,
        _ => false,
    }
}
/// MapZone.checkMulti(e, t, message).
pub fn check_multi<H: Host>(
    host: &mut H,
    e: &NodeHandle,
    t: &NodeHandle,
    message: bool,
) -> Result<bool> {
    let time = host.now_millis()?;
    let multi = host.multi_zone(t)? && host.multi_zone(e)?;
    if multi
        || host.entity_ignore_multi(e, t)?
        || host.monitor(e)?.is_ignore_multi_boundaries(host, Some(t))?
    {
        return Ok(true);
    }
    let attacker = owner_of(host, Some(e.clone()))?;
    let target = Some(host.combat_attacker(t)?.unwrap_or_else(|| e.clone()));
    let target = owner_of(host, target)?;
    if host.combat_time(t)?.unwrap_or(-1) > time
        && !identical(&target, &attacker)
        && host.active(target.as_ref().ok_or(Error::Null)?)?
    {
        if message && host.kind(e)? == EntityKind::Player {
            let text = if host.kind(t)? == EntityKind::Player {
                "Someone else is already fighting this player."
            } else {
                "Someone else is already fighting this."
            };
            host.packet_message(e, text)?;
        }
        return Ok(false);
    }
    let attacker_on_e = Some(host.combat_attacker(e)?.unwrap_or_else(|| t.clone()));
    let attacker_on_e = owner_of(host, attacker_on_e)?;
    let resolved = owner_of(host, Some(t.clone()))?;
    if host.combat_time(e)?.unwrap_or(-1) > time
        && !identical(&attacker_on_e, &resolved)
        && host.active(attacker_on_e.as_ref().ok_or(Error::Null)?)?
    {
        if host.id(t)? == 1614 || host.id(t)? == 1613 {
            return Ok(true);
        }
        if message && host.kind(e)? == EntityKind::Player {
            host.packet_message(e, "You're already under attack!")?;
        }
        return Ok(false);
    }
    Ok(true)
}
