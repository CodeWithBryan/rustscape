//! Original NPC tick/automatic movement orchestration. Concrete combat, locks,
//! regions, attributes, pulses, RNG and content behavior are owning-world services.
use super::doors::Location;
use crate::definitions::item::Shared;
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
pub struct Fields {
    pub location: Option<Location>,
    pub spawn: Option<Location>,
    pub radius: i32,
    /// A subclass's `getWalkRadius()` override (ShopNPC: 3), read where the
    /// original calls the method rather than the `walkRadius` field.
    pub walk_radius_override: Option<i32>,
    pub walks: bool,
    pub path_bound: bool,
    pub path: Option<Shared<Vec<Option<Location>>>>,
    pub path_index: i32,
    pub next_walk: i32,
    pub aggressive: bool,
    pub has_handler: bool,
    pub never_walks: bool,
    pub no_spawn_return: bool,
    pub return_pulse: Option<usize>,
    pub returning: bool,
    pub dialogue: Option<usize>,
    pub force_talk: Option<Vec<u16>>,
    pub last_force_talk: i32,
    pub respawn_tick: i32,
    pub respawning: bool,
}
pub struct State {
    pub fields: Shared<Fields>,
}
impl Clone for State {
    fn clone(&self) -> Self {
        Self {
            fields: self.fields.clone(),
        }
    }
}
impl State {
    pub fn new(location: Location) -> Self {
        Self {
            fields: Shared::new(Fields {
                spawn: Some(location.clone()),
                location: Some(location),
                radius: 11,
                walk_radius_override: None,
                walks: false,
                path_bound: false,
                path: None,
                path_index: 0,
                next_walk: 0,
                aggressive: false,
                has_handler: false,
                never_walks: false,
                no_spawn_return: false,
                return_pulse: None,
                returning: false,
                dialogue: None,
                force_talk: None,
                last_force_talk: 0,
                respawn_tick: -1,
                respawning: false,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Query {
    Behavior,
    InteractionLocked,
    MovementLocked,
    PulseRunning,
    Attacking,
    InCombat,
    SelectTarget,
    WaterNpc,
}
#[derive(Clone, Debug)]
pub enum Event {
    Inactivity,
    Teleport(Option<Location>),
    BehaviorRespawn,
    Respawn,
    Restore,
    EntityTick,
    PauseAggression(i32),
    LockMovement(i32),
    UnlockMovement,
    ImpactDisabled(i32),
    Attribute(&'static str),
    SubmitReturn(usize),
    StopCombat,
    Chat(Option<Vec<u16>>),
}
pub trait Context {
    /// Owners with a canonical Entity/WalkingQueue location override these
    /// getters rather than keeping a second independently mutable position.
    fn location(&self, state: &State) -> Option<Location> {
        state.fields.read().location.clone()
    }
    fn spawn(&self, state: &State) -> Option<Location> {
        state.fields.read().spawn.clone()
    }
    fn ticks(&mut self, state: &State) -> Result<i32>;
    fn region_active(&mut self, state: &State, location: Location) -> Result<bool>;
    fn query(&mut self, state: &State, query: Query) -> Result<bool>;
    fn event(&mut self, state: &State, event: Event) -> Result<()>;
    fn pulse_running(&mut self, state: &State, pulse: usize) -> Result<bool>;
    fn return_pulse(&mut self, state: &State, destination: Option<Location>) -> Result<usize>;
    fn dialogue_active(&mut self, state: &State, player: usize) -> Result<bool>;
    fn dialogue_chatbox(&mut self, state: &State, player: usize) -> Result<bool>;
    fn random_bool(&mut self, state: &State) -> Result<bool>;
    fn randomize(&mut self, state: &State, bound: i32) -> Result<i32>;
    fn random_range(&mut self, state: &State, min: i32, max: i32) -> Result<i32>;
    fn can_move(&mut self, state: &State, location: Option<Location>) -> Result<bool>;
    fn walk(&mut self, state: &State, location: Option<Location>, water: bool) -> Result<()>;
}
impl State {
    pub fn tick(&self, ctx: &mut impl Context) -> Result<()> {
        let location = ctx.location(self).ok_or(Error::Null)?;
        if !ctx.region_active(self, location)? {
            ctx.event(self, Event::Inactivity)?;
            return Ok(());
        }
        let respawn = self.fields.read().respawn_tick;
        if respawn > ctx.ticks(self)? {
            let respawn = self.fields.read().respawn_tick;
            if respawn.wrapping_sub(ctx.ticks(self)?) == 2 {
                let spawn = ctx.spawn(self);
                ctx.event(self, Event::Teleport(spawn))?;
            }
            return Ok(());
        }
        let respawning = self.fields.read().respawning;
        if respawning {
            let respawn = self.fields.read().respawn_tick;
            if respawn <= ctx.ticks(self)? {
                ctx.event(self, Event::BehaviorRespawn)?;
                ctx.event(self, Event::Respawn)?;
                ctx.event(self, Event::Restore)?;
                self.fields.write().respawning = false;
            }
        }
        self.actions(ctx)?;
        ctx.event(self, Event::EntityTick)
    }
    pub fn actions(&self, ctx: &mut impl Context) -> Result<()> {
        if !ctx.query(self, Query::Behavior)? {
            return Ok(());
        }
        if !ctx.query(self, Query::InteractionLocked)? {
            if !ctx.query(self, Query::MovementLocked)? {
                let eligible = {
                    let f = self.fields.read();
                    !f.path_bound && f.radius > 0 && f.radius <= 20
                };
                let should_return = if eligible {
                    let location = ctx.location(self).ok_or(Error::Null)?;
                    let spawn = ctx.spawn(self).ok_or(Error::Null)?;
                    let radius = self.fields.read().radius;
                    !within(&location, &spawn, (f64::from(radius) * 1.5) as i32)
                        && !self.fields.read().no_spawn_return
                } else {
                    false
                };
                if should_return {
                    let pulse = self.fields.read().return_pulse;
                    if let Some(p) = pulse
                        && ctx.pulse_running(self, p)?
                    {
                        return Ok(());
                    }
                    {
                        let mut f = self.fields.write();
                        if !f.never_walks && f.radius == 0 {
                            f.radius = 3;
                        }
                    }
                    let handler = self.fields.read().has_handler;
                    if handler {
                        let pause = self.fields.read().radius.wrapping_add(1);
                        ctx.event(self, Event::PauseAggression(pause))?;
                    }
                    let ticks = ctx.ticks(self)?;
                    let radius = self.fields.read().radius;
                    self.fields.write().next_walk = ticks.wrapping_add(radius).wrapping_add(1);
                    ctx.event(self, Event::LockMovement(100))?;
                    ctx.event(self, Event::ImpactDisabled(100))?;
                    self.fields.write().returning = true;
                    ctx.event(self, Event::Attribute("return-to-spawn"))?;
                    let spawn = ctx.spawn(self);
                    let pulse = ctx.return_pulse(self, spawn)?;
                    self.fields.write().return_pulse = Some(pulse);
                    ctx.event(self, Event::Attribute("return-to-spawn-pulse"))?;
                    ctx.event(self, Event::SubmitReturn(pulse))?;
                    return Ok(());
                }
                let dialogue = self.fields.read().dialogue;
                let clear = if let Some(player) = dialogue {
                    if !ctx.dialogue_active(self, player)? {
                        true
                    } else {
                        let player = self.fields.read().dialogue.ok_or(Error::Null)?;
                        !ctx.dialogue_chatbox(self, player)?
                    }
                } else {
                    true
                };
                if clear {
                    self.fields.write().dialogue = None;
                    let walks = self.fields.read().walks;
                    if walks
                        && !ctx.query(self, Query::PulseRunning)?
                        && !ctx.query(self, Query::Attacking)?
                        && !ctx.query(self, Query::InCombat)?
                    {
                        let next = self.fields.read().next_walk;
                        if next < ctx.ticks(self)? && self.walk_next(ctx)? {
                            return Ok(());
                        }
                    }
                }
            }
            let aggressive = self.fields.read().aggressive;
            let handler = self.fields.read().has_handler;
            if aggressive && handler && ctx.query(self, Query::SelectTarget)? {
                return Ok(());
            }
            if ctx.query(self, Query::Attacking)? {
                return Ok(());
            }
        }
        let has_talk = self.fields.read().force_talk.is_some();
        let last = self.fields.read().last_force_talk;
        if has_talk && last < ctx.ticks(self)? {
            let text = self.fields.read().force_talk.clone();
            ctx.event(self, Event::Chat(text))?;
            let ticks = ctx.ticks(self)?;
            let delay = ctx.random_range(self, 15, 30)?;
            self.fields.write().last_force_talk = ticks.wrapping_add(delay);
            ctx.event(self, Event::Attribute("lastForceTalk"))?;
        }
        Ok(())
    }
    pub fn walk_next(&self, ctx: &mut impl Context) -> Result<bool> {
        if ctx.random_bool(self)? {
            return Ok(true);
        }
        self.set_next_walk(ctx)?;
        let location = self.destination(ctx)?;
        if ctx.can_move(self, location.clone())? {
            let water = ctx.query(self, Query::WaterNpc)?;
            ctx.walk(self, location, water)?;
        }
        Ok(false)
    }
    pub fn set_next_walk(&self, ctx: &mut impl Context) -> Result<()> {
        let ticks = ctx.ticks(self)?;
        let delay = ctx.randomize(self, 10)?;
        self.fields.write().next_walk = ticks.wrapping_add(5).wrapping_add(delay);
        Ok(())
    }
    pub fn destination(&self, ctx: &mut impl Context) -> Result<Option<Location>> {
        let (fixed, path) = {
            let f = self.fields.read();
            (f.path_bound, f.path.clone())
        };
        if !fixed || path.as_ref().is_none_or(|p| p.read().is_empty()) {
            let radius = {
                let f = self.fields.read();
                f.walk_radius_override.unwrap_or(f.radius) / 2
            };
            let spawn = ctx.spawn(self).ok_or(Error::Null)?;
            let dx = ctx.random_range(self, radius.wrapping_neg(), radius.wrapping_add(1))?;
            let dy = ctx.random_range(self, radius.wrapping_neg(), radius.wrapping_add(1))?;
            let [x, y, z] = spawn.coordinates();
            let mut target = Location::new(x.wrapping_add(dx), y.wrapping_add(dy), z);
            let location = ctx.location(self).ok_or(Error::Null)?;
            let dist = distance(&location, &target) as i32;
            if dist > 14 {
                let from = ctx.location(self).ok_or(Error::Null)?;
                let [a, b, _] = from.coordinates();
                let [x, y, _] = target.coordinates();
                let dx = f64::from(x.wrapping_sub(a));
                let dy = f64::from(y.wrapping_sub(b));
                let magnitude = (dx * dx + dy * dy).sqrt();
                let vx = dx / magnitude * 14.0;
                let vy = dy / magnitude * 14.0;
                let origin = ctx.location(self).ok_or(Error::Null)?;
                let [x, y, _] = origin.coordinates();
                target = Location::new(
                    x.wrapping_add(vx.floor() as i32),
                    y.wrapping_add(vy.floor() as i32),
                    0,
                );
            }
            return Ok(Some(target));
        }
        let path = path.unwrap();
        let index = self.fields.read().path_index;
        self.fields.write().path_index = index.wrapping_add(1);
        let value = path
            .read()
            .get(index as usize)
            .cloned()
            .ok_or(Error::Index)?;
        if self.fields.read().path_index == path.read().len() as i32 {
            self.fields.write().path_index = 0;
        }
        Ok(value)
    }
    pub fn return_arrival(&self, ctx: &mut impl Context) -> Result<bool> {
        ctx.event(self, Event::StopCombat)?;
        ctx.event(self, Event::UnlockMovement)?;
        ctx.event(self, Event::ImpactDisabled(0))?;
        self.fields.write().returning = false;
        ctx.event(self, Event::Attribute("remove:return-to-spawn"))?;
        self.fields.write().return_pulse = None;
        ctx.event(self, Event::Attribute("remove:return-to-spawn-pulse"))?;
        Ok(true)
    }
}
fn distance(a: &Location, b: &Location) -> f64 {
    let [x, y, _] = a.coordinates();
    let [ex, ey, _] = b.coordinates();
    let x = f64::from(x.wrapping_sub(ex));
    let y = f64::from(y.wrapping_sub(ey));
    (x * x + y * y).sqrt()
}
fn within(a: &Location, b: &Location, d: i32) -> bool {
    a.coordinates()[2] == b.coordinates()[2] && distance(a, b) <= f64::from(d)
}
