//! `ClimbActionHandler` (ladders, stairs and trapdoors), its `ClimbDialogue`
//! and the `SpecialLadders` table (`core/game/global/action`), which
//! `LadderManagingPlugin` calls.
use super::{Error, Host, Result, Task, special_ladders::SPECIAL_LADDERS};
use rs09_game_core::direction::Direction;
use std::collections::HashMap;

/// `ClimbActionHandler.CLIMB_UP`.
pub const CLIMB_UP: i32 = 828;
/// `ClimbActionHandler.CLIMB_DOWN`.
pub const CLIMB_DOWN: i32 = 827;

/// A SpecialLadders constant's `checkAchievement` override.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Achievement {
    None,
    /// `player.achievementDiaryManager.finishTask(player, type, level, task)`
    /// (`kind` indexes `world::player_managers::diary_data::TYPES`).
    Diary {
        kind: usize,
        level: i32,
        task: i32,
    },
    /// `ZogreFleshEatersListeners.ladderMakesSithikTurnIntoOgre(player)`.
    ZogreSithik,
}

/// One SpecialLadders constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpecialLadder {
    pub name: &'static str,
    pub ladder: [i32; 3],
    pub destination: [i32; 3],
    pub achievement: Achievement,
}

/// SpecialLadders' two maps: the first constant at a location wins
/// (`putIfAbsent`); `add` overwrites a destination.
#[derive(Clone, Debug, Default)]
pub struct Ladders {
    destinations: HashMap<[i32; 3], [i32; 3]>,
    ladders: HashMap<[i32; 3], usize>,
}
impl Ladders {
    /// The maps after SpecialLadders' static initializer.
    pub fn original() -> Self {
        let mut ladders = Self::default();
        for (i, entry) in SPECIAL_LADDERS.iter().enumerate() {
            ladders
                .destinations
                .entry(entry.ladder)
                .or_insert(entry.destination);
            ladders.ladders.entry(entry.ladder).or_insert(i);
        }
        ladders
    }
    /// `SpecialLadders.add(from, to)`.
    pub fn add(&mut self, from: [i32; 3], to: [i32; 3]) {
        self.destinations.insert(from, to);
    }
    /// `SpecialLadders.getDestination(location)`.
    pub fn destination(&self, at: [i32; 3]) -> Option<[i32; 3]> {
        self.destinations.get(&at).copied()
    }
    /// `SpecialLadders.getSpecialLadder(location)`.
    pub fn special(&self, at: [i32; 3]) -> Option<&'static SpecialLadder> {
        self.ladders.get(&at).map(|&i| &SPECIAL_LADDERS[i])
    }
}

/// `ClimbDialogue`: the player and the ladder it opened on.
#[derive(Clone, Debug, PartialEq)]
pub struct ClimbDialogue<N> {
    pub player: N,
    pub object: Option<N>,
}

/// `Location.transform(dx, dy, dz)`: Kotlin's `(z + dz) % 4` keeps a negative plane.
fn transform(l: [i32; 3], dx: i32, dy: i32, dz: i32) -> [i32; 3] {
    [
        l[0].wrapping_add(dx),
        l[1].wrapping_add(dy),
        l[2].wrapping_add(dz) % 4,
    ]
}

fn location<H: Host + ?Sized>(h: &mut H, node: &H::Node) -> Result<[i32; 3]> {
    h.location(node)?.ok_or(Error::Null)
}

/// `ClimbActionHandler.climbLadder(player, startLadder, option)`.
pub fn climb_ladder<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    start: Option<&H::Node>,
    option: Option<&str>,
) -> Result<bool> {
    let start_ref = start.ok_or(Error::Null)?;
    let mut animation = Some(CLIMB_UP);
    if h.name(start_ref)?.ok_or(Error::Null)?.starts_with("Stair") {
        animation = None;
    }
    let at = location(h, start_ref)?;
    if h.special_destination(at)?.is_some() {
        let at = location(h, start_ref)?;
        let destination = h.special_destination(at)?.ok_or(Error::Null)?;
        climb(h, player, animation, destination, &[])?;
        let at = location(h, start_ref)?;
        if h.special_ladder(at)?.is_some() {
            let at = location(h, start_ref)?;
            let ladder = h.special_ladder(at)?.ok_or(Error::Null)?;
            check_achievement(h, player, ladder)?;
        }
        return Ok(true);
    }
    let end = match option.ok_or(Error::Null)? {
        "climb-up" | "walk-up" => get_ladder(h, start_ref, false)?,
        "climb-down" | "walk-down" => {
            if h.name(start_ref)?.ok_or(Error::Null)? == "Trapdoor" {
                animation = Some(CLIMB_DOWN);
            }
            get_ladder(h, start_ref, true)?
        }
        "climb" => {
            let upper = get_ladder(h, start_ref, false)?;
            let down = get_ladder(h, start_ref, true)?;
            if upper.is_none() && down.is_some() {
                return climb_ladder(h, player, start, Some("climb-down"));
            }
            if upper.is_some() && down.is_none() {
                return climb_ladder(h, player, start, Some("climb-up"));
            }
            // CLIMB_DIALOGUE.newInstance(player); dialogue.open(startLadder)
            let dialogue = ClimbDialogue {
                player: player.clone(),
                object: start.cloned(),
            };
            h.send_options(
                player,
                "What would you like to do?",
                &["Climb Up.", "Climb Down."],
            )?;
            h.set_dialogue(player, dialogue)?;
            return Ok(false);
        }
        _ => None,
    };
    let destination = match &end {
        Some(end) => get_destination(h, Some(end))?,
        None => None,
    };
    let (Some(_), Some(destination)) = (end, destination) else {
        h.packet_message(player, "The ladder doesn't seem to lead anywhere.")?;
        return Ok(false);
    };
    climb(h, player, animation, destination, &[])?;
    Ok(true)
}

/// A SpecialLadders constant's `checkAchievement(player)`.
fn check_achievement<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    ladder: &SpecialLadder,
) -> Result<()> {
    match ladder.achievement {
        Achievement::None => Ok(()),
        Achievement::Diary { kind, level, task } => h.finish_task(player, kind, level, task),
        Achievement::ZogreSithik => h.zogre_ladder(player),
    }
}

/// `Direction.forWalkFlag(walkingFlag, rotation)` with Java's shift counts.
pub fn for_walk_flag(mut flag: i32, rotation: i32) -> Option<Direction> {
    if rotation != 0 {
        flag = (flag.wrapping_shl(rotation as u32) & 0xf)
            .wrapping_add(flag.wrapping_shr(4i32.wrapping_sub(rotation) as u32));
    }
    if flag > 0 {
        for (mask, direction) in [
            (0x8, Direction::West),
            (0x2, Direction::East),
            (0x4, Direction::South),
            (0x1, Direction::North),
        ] {
            if flag & mask == 0 {
                return Some(direction);
            }
        }
    }
    None
}

/// `ClimbActionHandler.getDestination(object)`.
pub fn get_destination<H: Host + ?Sized>(
    h: &mut H,
    object: Option<&H::Node>,
) -> Result<Option<[i32; 3]>> {
    let object = object.ok_or(Error::Null)?;
    let [mut size_x, mut size_y] = h.scenery_size(object)?;
    if h.scenery_rotation(object)? % 2 != 0 {
        std::mem::swap(&mut size_x, &mut size_y);
    }
    let flag = h.scenery_walking_flag(object)?;
    let rotation = h.scenery_rotation(object)?;
    if let Some(dir) = for_walk_flag(flag, rotation) {
        return destination_from(h, object, size_x, size_y, dir, 0);
    }
    let dir = match h.scenery_rotation(object)? {
        0 => Direction::South,
        1 => Direction::East,
        2 => Direction::North,
        3 => Direction::West,
        _ => return Ok(None),
    };
    destination_from(h, object, size_x, size_y, dir, 0)
}

/// `Direction.canMove(location)`: the location's own clipping flag against
/// every traversal mask of the direction.
fn can_move<H: Host + ?Sized>(h: &mut H, dir: Direction, at: [i32; 3]) -> Result<bool> {
    let flag = h.clipping_flag(at)?;
    Ok(dir.traversal().iter().all(|&(_, _, mask)| flag & mask == 0))
}

/// The private `getDestination(object, sizeX, sizeY, dir, count)`.
fn destination_from<H: Host + ?Sized>(
    h: &mut H,
    object: &H::Node,
    size_x: i32,
    size_y: i32,
    dir: Direction,
    count: i32,
) -> Result<Option<[i32; 3]>> {
    let at = location(h, object)?;
    let (step_x, step_y) = dir.step();
    if dir.code() % 2 != 0 {
        let mut x = step_x;
        if x > 0 {
            x = x.wrapping_mul(size_x);
        }
        for y in 0..size_y {
            let l = transform(at, x, y, 0);
            if h.teleport_permitted(l)? && can_move(h, dir, l)? {
                return Ok(Some(l));
            }
        }
    } else {
        let mut y = step_y;
        if y > 0 {
            y = y.wrapping_mul(size_y);
        }
        for x in 0..size_x {
            let l = transform(at, x, y, 0);
            if h.teleport_permitted(l)? && can_move(h, dir, l)? {
                return Ok(Some(l));
            }
        }
    }
    if count == 3 {
        return Ok(None);
    }
    let next = Direction::from_code((dir.code() + 1) % 4)
        .ok_or(Error::Binding(Some("Invalid direction value".into())))?;
    destination_from(h, object, size_x, size_y, next, count + 1)
}

/// `ClimbActionHandler.climb(player, animation, destination, messages...)`.
pub fn climb<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    animation: Option<i32>,
    destination: [i32; 3],
    messages: &[&str],
) -> Result<()> {
    h.lock_ticks(player, 2)?;
    h.animate_entity(player, animation)?;
    h.submit(
        1,
        &[],
        Task::Climb {
            player: player.clone(),
            destination,
            messages: messages.iter().map(|m| (*m).to_owned()).collect(),
        },
    )
}

/// The climb pulse: the teleport location, then the messages; finished.
pub fn climb_pulse<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    destination: [i32; 3],
    messages: &[String],
) -> Result<bool> {
    h.set_teleport_location(player, destination)?;
    for message in messages {
        h.packet_message(player, message)?;
    }
    Ok(true)
}

/// `getLadder(object, down)`.
fn get_ladder<H: Host + ?Sized>(
    h: &mut H,
    object: &H::Node,
    down: bool,
) -> Result<Option<H::Node>> {
    let m = if down { -1 } else { 1 };
    let at = location(h, object)?;
    let mut ladder = h.get_object(transform(at, 0, 0, m))?;
    let is = match &ladder {
        Some(l) => is_ladder(h, l)?,
        None => false,
    };
    if !is {
        if let Some(l) = ladder.clone() {
            let name = h.name(&l)?.ok_or(Error::Null)?;
            if Some(name) == h.name(object)? {
                let below = location(h, &l)?;
                ladder = h.get_object(transform(below, 0, 0, m))?;
                if ladder.is_some() {
                    return Ok(ladder);
                }
            }
        }
        let at = location(h, object)?;
        ladder = find_ladder(h, transform(at, 0, 0, m))?;
        if ladder.is_none() {
            let at = location(h, object)?;
            ladder = h.get_object(transform(at, 0, m.wrapping_mul(-6400), 0))?;
            if ladder.is_none() {
                let at = location(h, object)?;
                ladder = find_ladder(h, transform(at, 0, m.wrapping_mul(-6400), 0))?;
            }
        }
    }
    Ok(ladder)
}

/// `findLadder(location)`: the first ladder in the 11 by 11 square, x outer.
fn find_ladder<H: Host + ?Sized>(h: &mut H, at: [i32; 3]) -> Result<Option<H::Node>> {
    for x in -5..6 {
        for y in -5..6 {
            if let Some(object) = h.get_object(transform(at, x, y, 0))?
                && is_ladder(h, &object)?
            {
                return Ok(Some(object));
            }
        }
    }
    Ok(None)
}

/// `isLadder(object)`: a "Climb" option, or the name Trapdoor.
fn is_ladder<H: Host + ?Sized>(h: &mut H, object: &H::Node) -> Result<bool> {
    let options = h.scenery_options(object)?.ok_or(Error::Null)?;
    if options.iter().flatten().any(|o| o.contains("Climb")) {
        return Ok(true);
    }
    Ok(h.name(object)?.ok_or(Error::Null)? == "Trapdoor")
}

/// `ClimbDialogue.handle(interfaceId, buttonId)` (stage 0).
pub fn climb_dialogue_handle<H: Host + ?Sized>(
    h: &mut H,
    dialogue: &ClimbDialogue<H::Node>,
    button: i32,
) -> Result<bool> {
    let option = match button {
        1 => "climb-up",
        2 => "climb-down",
        _ => return Ok(true),
    };
    h.lock_ticks(&dialogue.player, 1)?;
    h.submit(
        1,
        &[],
        Task::ClimbChoice {
            player: dialogue.player.clone(),
            object: dialogue.object.clone(),
            option,
        },
    )?;
    h.dialogue_end(&dialogue.player)?;
    Ok(true)
}
