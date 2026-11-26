//! Pinned WalkingQueue/Point behavior. Entity, timer, zone, packet and ground-item
//! services are explicit; pathfinding and the owning world's phases are separate.
use super::doors::Location;
use crate::definitions::item::Shared;
use rs09_game_core::direction::Direction;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    ConcurrentModification,
    Limit,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// Point coordinates/deltas/direction are immutable in the original class;
/// retained handles may still change its run-disabled flag.
pub struct Point {
    x: i32,
    y: i32,
    direction: Option<Direction>,
    diff_x: i32,
    diff_y: i32,
    pub run_disabled: bool,
}
impl Point {
    pub fn new(
        x: i32,
        y: i32,
        direction: Option<Direction>,
        diff_x: i32,
        diff_y: i32,
        run_disabled: bool,
    ) -> Self {
        Self {
            x,
            y,
            direction,
            diff_x,
            diff_y,
            run_disabled,
        }
    }
    pub fn origin(x: i32, y: i32) -> Self {
        Self::new(x, y, None, 0, 0, false)
    }
    pub fn x(&self) -> i32 {
        self.x
    }
    pub fn y(&self) -> i32 {
        self.y
    }
    pub fn direction(&self) -> Option<Direction> {
        self.direction
    }
    pub fn diff_x(&self) -> i32 {
        self.diff_x
    }
    pub fn diff_y(&self) -> i32 {
        self.diff_y
    }
}
pub type Step = Shared<Point>;
#[derive(Clone)]
pub struct RouteItem {
    pub token: usize,
    pub location: Location,
}
#[derive(Default)]
pub struct RouteItems {
    values: Vec<Option<RouteItem>>,
    version: i32,
}
impl RouteItems {
    pub fn values(&self) -> &[Option<RouteItem>] {
        &self.values
    }
    pub fn push(&mut self, item: Option<RouteItem>) {
        self.values.push(item);
        self.version = self.version.wrapping_add(1);
    }
    pub fn clear(&mut self) {
        self.values.clear();
        self.version = self.version.wrapping_add(1);
    }
}
/// Fields used by WalkingQueue, not a replacement for the full Entity/Player.
#[derive(Clone)]
pub struct Entity {
    pub player: bool,
    pub location: Option<Location>,
    pub last_scene: Option<Location>,
    pub update_scene: bool,
    pub teleport_location: Option<Location>,
    pub teleporting: bool,
    pub direction: Option<Direction>,
    pub run_toggled: bool,
    pub energy: f64,
    pub weight: f64,
    pub agility: i32,
    pub run_incrementer: i32,
    pub draw_path: bool,
}
impl Entity {
    pub fn new(player: bool, location: Option<Location>) -> Self {
        Self {
            player,
            location,
            last_scene: None,
            update_scene: false,
            teleport_location: None,
            teleporting: false,
            direction: None,
            run_toggled: false,
            energy: 100.0,
            weight: 0.0,
            agility: 1,
            run_incrementer: 0,
            draw_path: false,
        }
    }
}
/// Callbacks run without a held entity/location/point lock and may reenter the
/// queue. Energy/toggle state is published before its packet callback.
pub trait Context {
    fn timer(&mut self, queue: &mut Queue, name: &str) -> Result<bool>;
    fn marathon(&mut self, queue: &mut Queue) -> Result<bool>;
    fn energy(&mut self, queue: &mut Queue, drain: f64) -> Result<()>;
    fn run_toggled(&mut self, queue: &mut Queue, value: bool) -> Result<()>;
    fn zone_move(&mut self, queue: &mut Queue, from: Location, to: Location) -> Result<bool>;
    fn region_move(
        &mut self,
        queue: &mut Queue,
        from: Option<Location>,
        to: Option<Location>,
    ) -> Result<()>;
    fn clear_pulses(&mut self, queue: &mut Queue) -> Result<()>;
    fn route_create(&mut self, queue: &mut Queue, location: Location) -> Result<RouteItem>;
    fn route_add(&mut self, queue: &mut Queue, item: RouteItem) -> Result<()>;
    fn route_remove(&mut self, queue: &mut Queue, item: RouteItem) -> Result<()>;
    fn null_location(&mut self, queue: &mut Queue) -> Result<()>;
}
#[derive(Clone)]
pub struct Fields {
    pub entity: Entity,
    pub points: Shared<VecDeque<Step>>,
    pub walk_dir: i32,
    pub run_dir: i32,
    pub running: bool,
    pub run_disabled: bool,
    pub foot_print: Option<Location>,
    pub route_items: Shared<RouteItems>,
}
fn moved(location: &Location, dx: i32, dy: i32) -> Location {
    let [x, y, z] = location.coordinates();
    Location::new(x.wrapping_add(dx), y.wrapping_add(dy), z)
}
fn ordinal(direction: Direction) -> i32 {
    Direction::ALL.iter().position(|d| *d == direction).unwrap() as i32
}
#[derive(Clone)]
pub struct Queue {
    pub state: Shared<Fields>,
}
impl Queue {
    pub fn entity(&self) -> Entity {
        self.state.read().entity.clone()
    }
    pub fn points(&self) -> Shared<VecDeque<Step>> {
        self.state.read().points.clone()
    }
    pub fn walk_dir(&self) -> i32 {
        self.state.read().walk_dir
    }
    pub fn run_dir(&self) -> i32 {
        self.state.read().run_dir
    }
    pub fn running(&self) -> bool {
        self.state.read().running
    }
    pub fn run_disabled(&self) -> bool {
        self.state.read().run_disabled
    }
    pub fn foot_print(&self) -> Option<Location> {
        self.state.read().foot_print.clone()
    }
    pub fn route_items(&self) -> Shared<RouteItems> {
        self.state.read().route_items.clone()
    }
    pub fn new(entity: Entity) -> Self {
        Self {
            state: Shared::new(Fields {
                foot_print: entity.location.clone(),
                entity,
                points: Shared::new(VecDeque::new()),
                walk_dir: -1,
                run_dir: -1,
                running: false,
                run_disabled: false,
                route_items: Shared::new(RouteItems::default()),
            }),
        }
    }
    fn location(&self) -> Result<Location> {
        self.entity().location.clone().ok_or(Error::Null)
    }
    pub fn is_running_both(&self) -> bool {
        !self.run_disabled()
            && ((self.entity().player && self.entity().run_toggled) || self.running())
    }
    pub fn has_path(&self) -> bool {
        !self.points().read().is_empty()
    }
    pub fn is_moving(&self) -> bool {
        self.walk_dir() != -1 || self.run_dir() != -1
    }
    pub fn reset(&mut self, ctx: &mut impl Context, running: Option<bool>) -> Result<()> {
        let running = running.unwrap_or(self.running());
        let loc = self.entity().location.clone();
        if loc.is_none() {
            ctx.null_location(self)?;
        }
        self.points().write().clear();
        let [x, y, _] = loc.ok_or(Error::Null)?.coordinates();
        self.points()
            .write()
            .push_back(Shared::new(Point::origin(x, y)));
        self.state.write().running = running;
        Ok(())
    }
    pub fn add_point(&mut self, x: i32, y: i32, run_disabled: bool) {
        let Some(point) = self.points().read().back().cloned() else {
            return;
        };
        let p = point.read();
        let dx = x.wrapping_sub(p.x);
        let dy = y.wrapping_sub(p.y);
        drop(p);
        self.points().write().push_back(Shared::new(Point {
            x,
            y,
            direction: Some(Direction::from_delta(dx, dy)),
            diff_x: dx,
            diff_y: dy,
            run_disabled,
        }));
    }
    pub fn add_path(
        &mut self,
        ctx: &mut impl Context,
        x: i32,
        y: i32,
        run_disabled: Option<bool>,
    ) -> Result<()> {
        let run_disabled = run_disabled.unwrap_or(self.run_disabled());
        let Some(point) = self.points().read().back().cloned() else {
            return Ok(());
        };
        if self.entity().draw_path && self.entity().player {
            let z = self.location()?.coordinates()[2];
            let item = ctx.route_create(self, Location::new(x, y, z))?;
            self.route_items().write().push(Some(item.clone()));
            ctx.route_add(self, item)?;
        }
        let p = point.read();
        let mut dx = x.wrapping_sub(p.x);
        let mut dy = y.wrapping_sub(p.y);
        drop(p);
        let max = dx.wrapping_abs().max(dy.wrapping_abs());
        // Protect runtime callers from malformed billion-point paths; normal
        // original paths are bounded by the pathfinder's 104-square build area.
        if max > 65_536 {
            return Err(Error::Limit);
        }
        for _ in 0..max {
            dx = dx.wrapping_sub(dx.signum());
            dy = dy.wrapping_sub(dy.signum());
            self.add_point(x.wrapping_sub(dx), y.wrapping_sub(dy), run_disabled);
        }
        Ok(())
    }
    pub fn walk_back(&mut self, ctx: &mut impl Context) -> Result<()> {
        ctx.clear_pulses(self)?;
        self.reset(ctx, None)?;
        let [x, y, _] = self.foot_print().as_ref().ok_or(Error::Null)?.coordinates();
        self.add_path(ctx, x, y, None)
    }
    pub fn update_run_energy(&mut self, ctx: &mut impl Context, decrease: bool) -> Result<()> {
        if !self.entity().player {
            return Ok(());
        }
        if decrease && ctx.marathon(self)? {
            if self.entity().run_incrementer == 3 {
                self.state.write().entity.run_incrementer = 0;
                return Ok(());
            }
            self.state.write().entity.run_incrementer =
                self.entity().run_incrementer.wrapping_add(1);
        }
        if !decrease && self.entity().energy >= 100.0 {
            return Ok(());
        }
        let drain = if decrease {
            let mut rate = 0.55;
            if self.entity().weight > 0.0 {
                rate *= 1.0 + self.entity().weight / 100.0;
            }
            if ctx.timer(self, "hamstrung")? {
                rate *= 4.0;
            }
            rate
        } else {
            -(100.0 / (175i32.wrapping_sub(self.entity().agility) as f64 / 0.6))
        };
        self.state.write().entity.energy -= drain;
        // Retain NaN and negative zero, as the original Settings comparisons do.
        self.state.write().entity.energy = self.entity().energy.clamp(0.0, 100.0);
        ctx.energy(self, drain)
    }
    pub fn update_region(
        &mut self,
        ctx: &mut impl Context,
        previous: Option<Location>,
        location: Option<Location>,
        move_entity: bool,
    ) -> Result<bool> {
        if !self.entity().player {
            return Err(Error::Type);
        }
        let last = self
            .entity()
            .last_scene
            .clone()
            .or_else(|| location.clone())
            .ok_or(Error::Null)?;
        let l = last.coordinates();
        let c = location.as_ref().ok_or(Error::Null)?.coordinates();
        if ((l[0] >> 3).wrapping_sub(c[0] >> 3)).wrapping_abs() >= 4
            || ((l[1] >> 3).wrapping_sub(c[1] >> 3)).wrapping_abs() >= 4
        {
            self.state.write().entity.update_scene = true;
        }
        if move_entity && self.entity().update_scene {
            ctx.region_move(self, previous, location)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn update_teleport(&mut self, ctx: &mut impl Context) -> Result<bool> {
        if self.entity().teleport_location.is_none() {
            return Ok(false);
        }
        self.reset(ctx, Some(false))?;
        let previous = self.entity().location.clone();
        self.state.write().entity.location = self.entity().teleport_location.clone();
        self.state.write().entity.teleport_location = None;
        if self.entity().player {
            let last = self
                .entity()
                .last_scene
                .clone()
                .or_else(|| self.entity().location.clone())
                .ok_or(Error::Null)?;
            let l = last.coordinates();
            let c = self.location()?.coordinates();
            if ((l[0] >> 3).wrapping_sub(c[0] >> 3)).wrapping_abs() >= 4
                || ((l[1] >> 3).wrapping_sub(c[1] >> 3)).wrapping_abs() >= 4
            {
                self.state.write().entity.update_scene = true;
            }
        }
        ctx.region_move(self, previous, self.entity().location.clone())?;
        self.state.write().foot_print = self.entity().location.clone();
        self.state.write().entity.teleporting = true;
        Ok(true)
    }
    pub fn update(&mut self, ctx: &mut impl Context) -> Result<()> {
        let player = self.entity().player;
        self.state.write().walk_dir = -1;
        self.state.write().run_dir = -1;
        let previous = self.entity().location.clone();
        if previous.is_none() {
            return Ok(());
        }
        if self.update_teleport(ctx)? {
            return Ok(());
        }
        if player
            && self.update_region(ctx, previous.clone(), self.entity().location.clone(), true)?
        {
            return Ok(());
        }
        if ctx.timer(self, "frozen")? {
            return Ok(());
        }
        let mut point = self.points().write().pop_front();
        let draw_path = self.entity().draw_path;
        if point.is_none() {
            self.update_run_energy(ctx, false)?;
            if player && draw_path {
                let items = self.route_items().clone();
                let expected = items.read().version;
                let mut cursor = 0;
                loop {
                    let list = items.read();
                    if cursor == list.values.len() {
                        break;
                    }
                    if expected != list.version {
                        return Err(Error::ConcurrentModification);
                    }
                    let item = list
                        .values
                        .get(cursor)
                        .cloned()
                        .ok_or(Error::ConcurrentModification)?;
                    drop(list);
                    cursor += 1;
                    if let Some(item) = item {
                        ctx.route_remove(self, item)?;
                    }
                }
                self.route_items().write().clear();
            }
            return Ok(());
        }
        if player && self.entity().energy < 1.0 {
            self.state.write().running = false;
            self.state.write().entity.run_toggled = false;
            ctx.run_toggled(self, false)?;
        }
        if point.as_ref().unwrap().read().direction.is_none() {
            point = self.points().write().pop_front();
        }
        let mut run_point = None;
        if self.is_running_both() && point.as_ref().is_none_or(|p| !p.read().run_disabled) {
            run_point = self.points().write().pop_front();
        }
        let mut walk_direction = -1;
        let mut run_direction = -1;
        if let Some(p) = &point {
            let Some(d) = p.read().direction else {
                return Ok(());
            };
            walk_direction = ordinal(d);
        }
        if let Some(p) = &run_point {
            run_direction = ordinal(p.read().direction.ok_or(Error::Null)?);
        }
        let (mut dx, mut dy) = (0i32, 0i32);
        if walk_direction != -1 {
            let p = point.as_ref().unwrap().read();
            dx = p.diff_x;
            dy = p.diff_y;
        }
        if run_direction != -1 {
            self.state.write().foot_print = Some(moved(&self.location()?, dx, dy));
            {
                let p = run_point.as_ref().unwrap().read();
                dx = dx.wrapping_add(p.diff_x);
                dy = dy.wrapping_add(p.diff_y);
            }
            self.update_run_energy(ctx, true)?;
        } else {
            self.update_run_energy(ctx, false)?;
        }
        if dx != 0 || dy != 0 {
            let mut walk = self.location()?;
            if let Some(p) = &point {
                let p = p.read();
                walk = moved(&walk, p.diff_x, p.diff_y);
                drop(p);
                if !ctx.zone_move(self, self.location()?, walk.clone())? {
                    self.reset(ctx, None)?;
                    return Ok(());
                }
            }
            let mut dest = moved(&self.location()?, dx, dy);
            if let Some(p) = &run_point
                && !ctx.zone_move(self, walk, dest.clone())?
            {
                let p = p.read();
                dest = moved(&dest, p.diff_x.wrapping_neg(), p.diff_y.wrapping_neg());
                drop(p);
                run_point = None;
                run_direction = -1;
                self.reset(ctx, None)?;
            }
            if let Some(p) = run_point.as_ref().or(point.as_ref())
                && let Some(d) = p.read().direction
            {
                self.state.write().entity.direction = Some(d);
            }
            self.state.write().foot_print = self.entity().location.clone();
            self.state.write().entity.location = Some(dest.clone());
            ctx.region_move(self, previous, Some(dest))?;
        }
        self.state.write().walk_dir = walk_direction;
        self.state.write().run_dir = run_direction;
        Ok(())
    }
}
