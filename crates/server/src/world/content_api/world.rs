//! ContentAPI's NPC and scenery lookups, locations and borders, movement
//! (`forceWalk`, `truncateLoc`, `forceMove`, `flee`, `stopWalk`,
//! `teleport`), facing, pathable tiles, map zones, ladders and doors.
use super::{
    Anim, Arg, Error, Host, Kind, Location, Member as M, Pulse, PulseBody, PulseTarget,
    QueuedScript, Result, ScriptBody, Value, api_files::Direction, api_files::Vector, b, loc_of, n,
    nd, ns, state, unit,
};
use crate::world::content_api::services::{LogLevel, log};

/// `findNPC(id)`.
pub fn find_npc<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<Option<H::Node>> {
    n(h, M::FindNpc, vec![Arg::Int(id)])
}

/// `findNPC(refLoc, id)`: the first NPC of the repository with the id
/// within rendering distance (15) of the location.
pub fn find_npc_near<H: Host + ?Sized>(
    h: &mut H,
    at: Location,
    id: i32,
) -> Result<Option<H::Node>> {
    for npc in ns(h, M::RepositoryNpcs, vec![])? {
        let npc = npc.ok_or(Error::Null)?;
        if h.id(&npc) == id && loc_of(h, &npc)?.within_distance(at, 15) {
            return Ok(Some(npc));
        }
    }
    Ok(None)
}

/// `RegionManager.getLocalNPCs(location)` or `(location, distance)`.
fn local_npcs<H: Host + ?Sized>(
    h: &mut H,
    at: Location,
    distance: Option<i32>,
) -> Result<Vec<H::Node>> {
    let mut args = vec![Arg::Loc(at)];
    if let Some(d) = distance {
        args.push(Arg::Int(d));
    }
    ns(h, M::LocalNpcs, args)?
        .into_iter()
        .map(|n| n.ok_or(Error::Null))
        .collect()
}

/// `findLocalNPC(entity, id)`.
pub fn find_local_npc<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    id: i32,
) -> Result<Option<H::Node>> {
    let at = loc_of(h, e)?;
    Ok(local_npcs(h, at, None)?.into_iter().find(|n| h.id(n) == id))
}

/// `findLocalNPCs(location, distance)`.
pub fn find_local_npcs_at<H: Host + ?Sized>(
    h: &mut H,
    at: Location,
    distance: i32,
) -> Result<Vec<H::Node>> {
    local_npcs(h, at, Some(distance))
}

/// `findLocalNPCs(entity, ids)` and `findLocalNPCs(entity, ids, distance)`.
pub fn find_local_npcs<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    ids: &[i32],
    distance: Option<i32>,
) -> Result<Vec<H::Node>> {
    let at = loc_of(h, e)?;
    Ok(local_npcs(h, at, distance)?
        .into_iter()
        .filter(|n| ids.contains(&h.id(n)))
        .collect())
}

/// `getRegionBorders(regionId)`: `ZoneBorders.forRegion` (Kotlin's non-null return).
pub fn get_region_borders<H: Host + ?Sized>(h: &mut H, region: i32) -> Result<[i32; 5]> {
    h.call(M::BordersForRegion, vec![Arg::Int(region)])?
        .borders()?
        .ok_or(Error::Null)
}

/// `location(x, y, z)`.
pub fn location(x: i32, y: i32, z: i32) -> Location {
    Location::new(x, y, z)
}

/// `inBorders(entity, borders)`.
pub fn in_borders<H: Host + ?Sized>(h: &mut H, e: &H::Node, borders: [i32; 5]) -> Result<bool> {
    b(h, M::BordersInside, vec![Arg::borders(borders), nd(e)])
}

/// `inBorders(entity, swX, swY, neX, neY)`: plane 0.
pub fn in_corners<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    sw_x: i32,
    sw_y: i32,
    ne_x: i32,
    ne_y: i32,
) -> Result<bool> {
    in_borders(h, e, [sw_x, sw_y, ne_x, ne_y, 0])
}

/// `poofClear(npc)`: a world pulse that hides the NPC with the smoke
/// graphics on its third run and clears it on its fourth.
pub fn poof_clear<H: Host + ?Sized>(h: &mut H, npc: &H::Node) -> Result<()> {
    h.submit_pulse(
        PulseTarget::World,
        Pulse {
            delay: 1,
            body: PulseBody::PoofClear {
                npc: npc.clone(),
                counter: 0,
            },
        },
    )
    .map(|_| ())
}

/// `transformNpc(npc, transformTo, restoreTicks)`.
pub fn transform_npc<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Node,
    to: i32,
    restore: i32,
) -> Result<()> {
    unit(h, M::NpcTransform, vec![nd(npc), Arg::Int(to)])?;
    h.submit_pulse(
        PulseTarget::World,
        Pulse {
            delay: restore,
            body: PulseBody::TransformRestore { npc: npc.clone() },
        },
    )
    .map(|_| ())
}

/// `transformNpcWithHpCarryover(npc, transformTo, restoreTicks)`.
pub fn transform_npc_with_hp_carryover<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Node,
    to: i32,
    restore: i32,
) -> Result<()> {
    unit(h, M::NpcTransformHp, vec![nd(npc), Arg::Int(to)])?;
    state::queue_script(
        h,
        npc,
        0,
        state::QueueStrength::Soft,
        false,
        ScriptBody::TransformHp {
            npc: npc.clone(),
            ticks: restore,
        },
    )
}

/// `forceWalk(entity, dest, type)`: "clip" runs a ForceMovement, "smart"
/// the SMART pathfinder, anything else DUMB.
pub fn force_walk<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    dest: Location,
    kind: &str,
) -> Result<()> {
    if kind == "clip" {
        return unit(
            h,
            M::ForceMovementRun,
            vec![nd(e), Arg::Loc(dest), Arg::Int(10), Arg::Int(10)],
        );
    }
    let finder = if kind == "smart" { "SMART" } else { "DUMB" };
    let path = h
        .call(
            M::PathfinderFind,
            vec![nd(e), Arg::Loc(dest), Arg::Bool(true), Arg::Enum(finder)],
        )?
        .path()?
        .ok_or(Error::Null)?;
    unit(h, M::PathWalk, vec![nd(&path.handle), nd(e)])
}

/// `truncateLoc(mover, destination)`: (truncated, the location moved
/// towards the destination by at most 14 tiles for an NPC or
/// `MAX_PATHFIND_DISTANCE` otherwise).
pub fn truncate_loc<H: Host + ?Sized>(
    h: &mut H,
    mover: &H::Node,
    destination: Location,
) -> Result<(bool, Location)> {
    let from = loc_of(h, mover)?;
    let vector = Vector::between_locs(from, destination);
    let norm = vector.normalized();
    let magnitude = vector.magnitude();
    let multiplier = if h.kind(mover).is_npc() {
        14.0
    } else {
        h.max_pathfind_distance() as f64
    };
    let clamped = java_min(multiplier, magnitude);
    let truncated = multiplier == clamped;
    Ok((truncated, from.transform_vector(norm.times(clamped))))
}

/// `Math.min(double, double)`.
pub fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { a } else { b }
    } else if a <= b {
        a
    } else {
        b
    }
}

/// `cyclesToTicks(cycles)`: `ceil(cycles / 30.0)`.
pub fn cycles_to_ticks(cycles: i32) -> i32 {
    super::java_double_to_int((cycles as f64 / 30.0).ceil())
}

/// `forceMove`'s queued script and what it captured.
#[derive(Clone, Debug, PartialEq)]
pub struct ForceMove<N> {
    pub player: N,
    pub start: Location,
    pub dest: Location,
    pub start_arrive: i32,
    pub dest_arrive: i32,
    pub direction: Direction,
    pub anim: i32,
    pub callback: Option<N>,
    pub start_tick: i32,
    pub dest_tick: i32,
    pub mask_set: bool,
}
impl<N> ForceMove<N> {
    /// `ForceMoveCtx.toString()`.
    pub fn context(&self) -> String {
        format!(
            "ForceMoveCtx(start={}, dest={}, startArrive={}, destArrive={}, direction={})",
            self.start,
            self.dest,
            self.start_arrive,
            self.dest_arrive,
            self.direction.name()
        )
    }
}

/// `forceMove(player, start, dest, startArrive, destArrive, dir = null,
/// anim = 819, callback = null)`.
#[allow(clippy::too_many_arguments)]
pub fn force_move<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    start: Location,
    dest: Location,
    start_arrive: i32,
    dest_arrive: i32,
    dir: Option<Direction>,
    anim: i32,
    callback: Option<&H::Node>,
) -> Result<()> {
    let direction = match dir {
        Some(d) => d,
        None => {
            let delta = start.delta(dest);
            if delta.x().wrapping_abs() > delta.y().wrapping_abs() {
                Direction::get(delta.x(), 0)
            } else {
                Direction::get(0, delta.y())
            }
        }
    };
    let start_tick = h
        .ticks()
        .wrapping_add(cycles_to_ticks(start_arrive))
        .wrapping_add(1);
    let dest_tick = start_tick.wrapping_add(cycles_to_ticks(dest_arrive));
    state::delay_entity(h, p, dest_tick.wrapping_sub(h.ticks()).wrapping_add(1))?;
    let body = ForceMove {
        player: p.clone(),
        start,
        dest,
        start_arrive,
        dest_arrive,
        direction,
        anim,
        callback: callback.cloned(),
        start_tick,
        dest_tick,
        mask_set: false,
    };
    state::queue_script(
        h,
        p,
        0,
        state::QueueStrength::Soft,
        false,
        ScriptBody::ForceMove(Box::new(body)),
    )
}

/// One run of `forceMove`'s script.
pub fn run_force_move<H: Host + ?Sized>(h: &mut H, m: &mut ForceMove<H::Node>) -> Result<bool> {
    let p = m.player.clone();
    if !state::finished_moving(h, &p)? {
        return state::keep_running(h, &p);
    }
    if !m.mask_set {
        b(
            h,
            M::UpdateMaskRegister,
            vec![
                nd(&p),
                Arg::Enum("ForceMove"),
                Arg::Display(m.context()),
                Arg::Bool(false),
            ],
        )?;
        m.mask_set = true;
    }
    let tick = h.ticks();
    if tick < m.start_tick {
        return state::keep_running(h, &p);
    }
    if tick < m.dest_tick {
        if state::animation_finished(h, &p)? {
            super::visuals::animate(h, &p, &Value::Int(m.anim), false)?;
        }
        return state::keep_running(h, &p);
    }
    if let Some(callback) = m.callback.clone()
        && let Err(e) = h.call(M::Callback, vec![nd(&callback)])
    {
        h.print_stack_trace(&e)?;
    }
    unit(h, M::SetTeleportLocation, vec![nd(&p), Arg::Loc(m.dest)])?;
    state::stop_executing(h, &p)
}

/// `stopWalk(entity)`.
pub fn stop_walk<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<()> {
    unit(h, M::WalkingReset, vec![nd(e)])
}

/// What `face` turns to: a location or a node.
#[derive(Clone, Debug, PartialEq)]
pub enum Face<N> {
    Loc(Location),
    Node(N),
}

/// `face(entity, toFace, duration = -1)`.
pub fn face<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    to: &Face<H::Node>,
    duration: i32,
) -> Result<()> {
    if duration == -1 {
        match to {
            Face::Loc(l) => unit(h, M::EntityFaceLocation, vec![nd(e), Arg::Loc(*l)]),
            Face::Node(n) if h.kind(n).is_entity() => {
                b(h, M::EntityFace, vec![nd(e), nd(n)]).map(|_| ())
            }
            Face::Node(_) => Ok(()),
        }
    } else {
        match to {
            // `toFace.asNpc()` casts the Location.
            Face::Loc(_) => Err(Error::Cast),
            Face::Node(n) if h.kind(n).is_entity() => b(
                h,
                M::EntityFaceTemporary,
                vec![nd(e), nd(n), Arg::Int(duration)],
            )
            .map(|_| ()),
            Face::Node(_) => Err(Error::Cast),
        }
    }
}

/// `resetFace(entity)`.
pub fn reset_face<H: Host + ?Sized>(h: &mut H, e: &H::Node) -> Result<()> {
    b(h, M::EntityFace, vec![nd(e), Arg::Null])?;
    let at = h.location(e);
    unit(h, M::EntityFaceLocation, vec![nd(e), Arg::loc(at)])
}

/// `flee(entity, from)`.
pub fn flee<H: Host + ?Sized>(h: &mut H, e: &H::Node, from: &H::Node) -> Result<()> {
    state::lock(h, e, 5)?;
    face(h, e, &Face::Node(from.clone()), 5)?;
    let at = loc_of(h, e)?;
    let other = loc_of(h, from)?;
    let dx = at.x().wrapping_sub(other.x());
    let dy = at.y().wrapping_sub(other.y());
    let dest = loc_of(h, e)?.transform(dx, dy, 0);
    force_walk(h, e, dest, "DUMB")
}

/// `TeleportManager.TeleportType`.
pub const TELEPORT_TYPES: [&str; 14] = [
    "NORMAL",
    "ANCIENT",
    "LUNAR",
    "TELETABS",
    "INSTANT",
    "OBELISK",
    "TELE_OTHER",
    "FAIRY_RING",
    "PURO_PURO",
    "ECTOPHIAL",
    "CHRISTMAS",
    "CABBAGE",
    "ENTRANA_MAGIC_DOOR",
    "MINIGAME",
];

/// `teleport(entity, loc, type = INSTANT)`.
pub fn teleport<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    loc: Location,
    kind: &'static str,
) -> Result<bool> {
    if kind == "INSTANT" {
        unit(h, M::SetTeleportLocation, vec![nd(e), Arg::Loc(loc)])?;
        return Ok(true);
    }
    b(
        h,
        M::TeleportSend,
        vec![nd(e), Arg::Loc(loc), Arg::Enum(kind)],
    )
}

/// `Pathfinder.find(start, dest, moverSize)`'s path (Kotlin dereferences it).
fn find_from<H: Host + ?Sized>(
    h: &mut H,
    start: Location,
    dest: Option<Location>,
    size: i32,
) -> Result<super::Path<H::Node>> {
    h.call(
        M::PathfinderFind,
        vec![Arg::Loc(start), Arg::loc(dest), Arg::Int(size)],
    )?
    .path()?
    .ok_or(Error::Null)
}

/// `getPathableRandomLocalCoordinate(target, radius, center, maxAttempts = 3)`.
pub fn get_pathable_random_local_coordinate<H: Host + ?Sized>(
    h: &mut H,
    target: &H::Node,
    radius: i32,
    center: Location,
    max_attempts: i32,
) -> Result<Location> {
    let max_radius = Vector::derive_with_equal_components(h.max_pathfind_distance() as f64).x - 1.0;
    let r = radius.min(super::java_double_to_int(max_radius));
    let sw = center.transform(r.wrapping_neg(), r.wrapping_neg(), center.z());
    let ne = center.transform(r, r, center.z());
    let borders = [sw.x(), sw.y(), ne.x(), ne.y(), center.z()];
    let mut attempts = max_attempts;
    while {
        let go = attempts > 0;
        attempts = attempts.wrapping_sub(1);
        go
    } {
        let dest = h
            .call(M::BordersRandomLoc, vec![Arg::borders(borders)])?
            .loc()?;
        let size = h.size(target);
        let path = find_from(h, center, dest, size)?;
        if path.successful && !path.move_near {
            return dest.ok_or(Error::Null);
        }
    }
    h.location(target).ok_or(Error::Null)
}

/// `getPathableCardinal(target, center)`: west, south, east, north.
pub fn get_pathable_cardinal<H: Host + ?Sized>(
    h: &mut H,
    target: &H::Node,
    center: Location,
) -> Result<Location> {
    for tile in center.cardinal_tiles() {
        let size = h.size(target);
        let path = find_from(h, center, Some(tile), size)?;
        if path.successful && !path.move_near {
            return Ok(tile);
        }
    }
    Ok(center)
}

/// `registerMapZone(zone, borders)`.
pub fn register_map_zone<H: Host + ?Sized>(
    h: &mut H,
    zone: &H::Node,
    borders: [i32; 5],
) -> Result<()> {
    unit(h, M::ZoneConfigure, vec![nd(zone)])?;
    unit(h, M::MapZoneRegister, vec![nd(zone), Arg::borders(borders)])
}

/// `addClimbDest(ladderLoc, dest)`.
pub fn add_climb_dest<H: Host + ?Sized>(h: &mut H, ladder: Location, dest: Location) -> Result<()> {
    unit(h, M::LadderAdd, vec![Arg::Loc(ladder), Arg::Loc(dest)])
}

/// `handleDoor(entity, door)`: players only.
pub fn handle_door<H: Host + ?Sized>(h: &mut H, e: &H::Node, door: &H::Node) -> Result<()> {
    if h.kind(e) != Kind::Player {
        return log(
            h,
            "ContentAPI",
            LogLevel::Err,
            "Handledoor for non-player entities is not implemented",
        );
    }
    unit(h, M::DoorHandle, vec![nd(e), nd(door)])
}

/// `handleAutowalkDoor(entity, door)`.
pub fn handle_autowalk_door<H: Host + ?Sized>(
    h: &mut H,
    e: Option<&H::Node>,
    door: Option<&H::Node>,
) -> Result<bool> {
    let end = h
        .call(
            M::DoorEndLocation,
            vec![Arg::opt_node(e), Arg::opt_node(door)],
        )?
        .loc()?;
    b(
        h,
        M::DoorAutowalk,
        vec![Arg::opt_node(e), Arg::opt_node(door), Arg::loc(end)],
    )
}

/// `hasLineOfSight(entity, target)`.
pub fn has_line_of_sight<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    target: &H::Node,
) -> Result<bool> {
    b(
        h,
        M::ProjectileClipped,
        vec![nd(e), nd(target), Arg::Bool(false)],
    )
}

/// `isPlayer(node)`.
pub fn is_player<H: Host + ?Sized>(h: &H, node: &H::Node) -> bool {
    h.kind(node) == Kind::Player
}

/// `getWorldTicks()`.
pub fn get_world_ticks<H: Host + ?Sized>(h: &H) -> i32 {
    h.ticks()
}

/// One run of `poofClear`'s pulse (`when (counter++)`).
pub fn run_poof_clear<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Node,
    counter: &mut i32,
) -> Result<bool> {
    let current = *counter;
    *counter = counter.wrapping_add(1);
    match current {
        2 => {
            unit(h, M::NpcSetInvisible, vec![nd(npc), Arg::Bool(true)])?;
            let at = loc_of(h, npc)?;
            if b(h, M::RegionIsActive, vec![Arg::Int(at.region_id())])? {
                unit(
                    h,
                    M::GraphicsSend,
                    vec![Arg::gfx(super::Gfx::new(86)), Arg::Loc(at)],
                )?;
            }
            Ok(false)
        }
        3 => {
            unit(h, M::NpcClear, vec![nd(npc)])?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// `Anim` for an emote's animation id.
pub fn emote_animation<N>(id: i32) -> Anim<N> {
    Anim::new(id)
}

/// `QueuedScript` built by `queueScript` (re-exported here for the bodies
/// above).
pub type Script<N> = QueuedScript<N>;
