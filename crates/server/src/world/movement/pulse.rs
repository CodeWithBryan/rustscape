use super::*;
#[derive(Clone, Copy, Debug, Default)]
pub enum Constructor {
    #[default]
    Basic,
    Option(Option<usize>),
    UseWith(Option<usize>),
    Destination {
        flag: Option<Flag>,
        override_method: Option<usize>,
    },
}
pub struct Fields {
    pub id: usize,
    pub base: pulses::Handle,
    pub mover: Option<Node>,
    pub destination: Option<Node>,
    pub last: Option<Location>,
    pub finder: Finder,
    pub force_run: bool,
    pub option: Option<usize>,
    pub use_with: Option<usize>,
    pub flag: Option<Flag>,
    pub interact: Option<Location>,
    pub near: bool,
    pub override_method: Option<usize>,
    pub previous: Option<Location>,
    pub truncated: bool,
    pub move_near_set: bool,
}
#[derive(Clone)]
pub struct Movement {
    pub state: Shared<Fields>,
}
impl Movement {
    pub fn id(&self) -> usize {
        self.state.read().id
    }
    pub fn base(&self) -> pulses::Handle {
        self.state.read().base.clone()
    }
    pub fn mover(&self) -> Option<Node> {
        self.state.read().mover
    }
    pub fn destination(&self) -> Option<Node> {
        self.state.read().destination
    }
    pub fn last(&self) -> Option<Location> {
        self.state.read().last.clone()
    }
    pub fn finder(&self) -> Finder {
        self.state.read().finder
    }
    pub fn force_run(&self) -> bool {
        self.state.read().force_run
    }
    pub fn option(&self) -> Option<usize> {
        self.state.read().option
    }
    pub fn use_with(&self) -> Option<usize> {
        self.state.read().use_with
    }
    pub fn flag(&self) -> Option<Flag> {
        self.state.read().flag
    }
    pub fn interact(&self) -> Option<Location> {
        self.state.read().interact.clone()
    }
    pub fn near(&self) -> bool {
        self.state.read().near
    }
    pub fn override_method(&self) -> Option<usize> {
        self.state.read().override_method
    }
    pub fn previous(&self) -> Option<Location> {
        self.state.read().previous.clone()
    }
    pub fn truncated(&self) -> bool {
        self.state.read().truncated
    }
    pub fn move_near_set(&self) -> bool {
        self.state.read().move_near_set
    }
    pub fn new(
        ctx: &mut impl Context,
        id: usize,
        mover: Option<Node>,
        destination: Option<Node>,
        finder: Option<Finder>,
        force_run: bool,
        constructor: Constructor,
    ) -> Result<Self> {
        let base = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(vec![mover, destination])),
            pulses::Class::Movement,
            "MovementPulse".into(),
        ));
        let kind = mover
            .map(|m| ctx.node(m).map(|n| n.kind))
            .transpose()?
            .unwrap_or(Kind::Other);
        let finder = if let Some(f) = finder {
            f
        } else {
            match kind {
                Kind::Player => SMART,
                Kind::Npc => ctx.npc_finder(mover.unwrap())?.unwrap_or(DUMB),
                _ => DUMB,
            }
        };
        let flag = if destination
            .map(|d| {
                ctx.node(d)
                    .map(|n| matches!(n.kind, Kind::Player | Kind::Npc))
            })
            .transpose()?
            .unwrap_or(false)
        {
            Some(Flag::Entity)
        } else {
            None
        };
        let mover_id = mover.ok_or(Error::Null)?;
        if let Some(previous) = ctx.current_movement(mover_id)? {
            ctx.stop_previous(previous)?;
            ctx.walk_reset(None, mover_id, None)?;
        }
        ctx.assign_current(mover_id, id)?;
        let p = Self {
            state: Shared::new(Fields {
                id,
                base,
                mover,
                destination,
                last: None,
                finder,
                force_run,
                option: None,
                use_with: None,
                flag,
                interact: None,
                near: false,
                override_method: None,
                previous: None,
                truncated: false,
                move_near_set: false,
            }),
        };
        match constructor {
            Constructor::Basic => {}
            Constructor::Option(o) => p.state.write().option = o,
            Constructor::UseWith(u) => p.state.write().use_with = u,
            Constructor::Destination {
                flag,
                override_method,
            } => {
                p.state.write().flag = flag;
                p.state.write().override_method = override_method;
            }
        }
        Ok(p)
    }
    fn m(&self) -> Result<Node> {
        self.mover().ok_or(Error::Null)
    }
    fn d(&self) -> Result<Node> {
        self.destination().ok_or(Error::Null)
    }
    fn target_entity(&self, ctx: &impl Context) -> Result<bool> {
        Ok(self
            .destination()
            .map(|n| ctx.node(n).map(|v| v.kind.entity()))
            .transpose()?
            .unwrap_or(false))
    }
    fn reach(&self, ctx: &mut impl Context) -> Result<bool> {
        let m = self.m()?;
        let d = self.d()?;
        let from = location(ctx, Some(m))?.coordinates();
        let [dx, dy, _] = location(ctx, Some(d))?.coordinates();
        rectangle(
            ctx,
            pathfinding::Reach {
                from,
                mover: [ctx.node(m)?.size; 2],
                to: [dx, dy],
                size: [ctx.node(d)?.size; 2],
                kind: 0,
                rotation: 0,
                walking_flag: 0,
            },
            false,
        )
    }
    fn inside(&self, ctx: &mut impl Context, l: &Location) -> Result<bool> {
        if !self.target_entity(ctx)? {
            return Ok(false);
        }
        let d = self.d()?;
        if ctx.node(d)?.walking.moving {
            return Ok(false);
        }
        let from = l.coordinates();
        let [dx, dy, _] = location(ctx, Some(d))?.coordinates();
        rectangle(
            ctx,
            pathfinding::Reach {
                from,
                mover: [ctx.node(self.m()?)?.size; 2],
                to: [dx, dy],
                size: [ctx.node(d)?.size; 2],
                kind: 0,
                rotation: 0,
                walking_flag: 0,
            },
            true,
        )
    }
    fn without_moving(&self, ctx: &mut impl Context) -> Result<bool> {
        if !self.target_entity(ctx)? {
            return Ok(false);
        }
        let m = location(ctx, self.mover())?;
        let d = location(ctx, self.destination())?;
        if m.coordinates() == d.coordinates() {
            return Ok(false);
        }
        if self.inside(ctx, &location(ctx, self.mover())?)? {
            return Ok(false);
        }
        if node(ctx, self.destination())?.walking.has_path {
            let [x, y, _] = m.coordinates();
            let [dx, dy, _] = d.coordinates();
            if x.wrapping_sub(dx)
                .wrapping_abs()
                .max(y.wrapping_sub(dy).wrapping_abs())
                <= node(ctx, self.destination())?.size
            {
                return Ok(true);
            }
        }
        self.reach(ctx)
    }
    fn immediate(&mut self, ctx: &mut impl Context) -> Result<bool> {
        if self.target_entity(ctx)? {
            ctx.face(self, self.m()?, self.destination())?;
        }
        let outcome = (|| {
            if ctx.pulse(self)? {
                if kind(ctx, self.mover())? == Some(Kind::Player) {
                    ctx.clear_minimap(self, self.m()?)?;
                }
                return Ok(true);
            }
            Ok(false)
        })();
        match outcome {
            Ok(v) => Ok(v),
            Err(e) => {
                ctx.exception(self, &e)?;
                Ok(false)
            }
        }
    }
    fn inactive(&mut self, ctx: &mut impl Context) -> Result<bool> {
        let Some(checks) = self.base().read().checks.clone() else {
            return Ok(false);
        };
        let size = checks.read().len();
        for i in 0..size {
            let checks = self.base().read().checks.clone().ok_or(Error::Null)?;
            let n = *checks.read().get(i).ok_or(Error::Index)?;
            if let Some(n) = n
                && !ctx.active(self, n)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn stop(&mut self, ctx: &mut impl Context) -> Result<()> {
        pulses::stop_base(&self.base());
        if self.target_entity(ctx)? {
            ctx.face(self, self.m()?, None)?;
        }
        self.state.write().last = None;
        Ok(())
    }
    pub fn update(&mut self, ctx: &mut impl Context) -> Result<bool> {
        if !ctx
            .region(&location(ctx, self.mover())?)?
            .ok_or(Error::Null)?
        {
            return Ok(false);
        }
        if !pulses::is_running_base(&self.base()) {
            return Ok(true);
        }
        let valid = self.mover().is_some()
            && self.destination().is_some()
            && ctx.region(&location(ctx, self.mover())?)?.is_some()
            && !self.inactive(ctx)?
            && pulses::is_running_base(&self.base());
        if !valid {
            self.stop(ctx)?;
            return Ok(true);
        }
        ctx.scripts(self, self.m()?, true)?;
        ctx.scripts(self, self.m()?, false)?;
        ctx.face(self, self.m()?, None)?;
        if self.without_moving(ctx)? && self.immediate(ctx)? {
            self.stop(ctx)?;
            return Ok(true);
        }
        self.update_path(ctx)?;
        if self.try_interact(ctx)? {
            self.stop(ctx)?;
            return Ok(true);
        }
        Ok(false)
    }
    fn try_interact(&mut self, ctx: &mut impl Context) -> Result<bool> {
        let ml = location(ctx, self.mover())?;
        let radius = if self.target_entity(ctx)? && node(ctx, self.destination())?.walking.has_path
        {
            1
        } else {
            0
        };
        let Some(interact) = self.interact().clone() else {
            return Ok(false);
        };
        let [x, y, _] = ml.coordinates();
        let [ix, iy, _] = interact.coordinates();
        let at = x
            .wrapping_sub(ix)
            .wrapping_abs()
            .max(y.wrapping_sub(iy).wrapping_abs())
            <= radius;
        let mut current = false;
        if !at && self.target_entity(ctx)? {
            let dl = location(ctx, self.destination())?;
            if ml.coordinates() != dl.coordinates() {
                current = self.reach_from(ctx, &ml, &dl)?;
            }
        }
        if at || current {
            let outcome = (|| {
                if self.near() || ctx.pulse(self)? {
                    if kind(ctx, self.mover())? == Some(Kind::Player) {
                        if self.near() {
                            ctx.message(self, self.m()?, "I can't reach that.")?;
                        }
                        ctx.clear_minimap(self, self.m()?)?;
                    }
                    self.stop(ctx)?;
                    return Ok(true);
                }
                Ok(false)
            })();
            return match outcome {
                Ok(v) => Ok(v),
                Err(e) => {
                    ctx.exception(self, &e)?;
                    self.stop(ctx)?;
                    Ok(false)
                }
            };
        }
        Ok(false)
    }
    fn reach_from(&self, ctx: &mut impl Context, ml: &Location, dl: &Location) -> Result<bool> {
        let [dx, dy, _] = dl.coordinates();
        rectangle(
            ctx,
            pathfinding::Reach {
                from: ml.coordinates(),
                mover: [node(ctx, self.mover())?.size; 2],
                to: [dx, dy],
                size: [node(ctx, self.destination())?.size; 2],
                kind: 0,
                rotation: 0,
                walking_flag: 0,
            },
            false,
        )
    }
    fn allow_movement(&self, ctx: &impl Context) -> Result<bool> {
        if !self.target_entity(ctx)? {
            return Ok(true);
        }
        let d = self.d()?;
        let loc = location(ctx, Some(d))?;
        let path = ctx.node(d)?.walking.points;
        let mut allowed = true;
        if ctx.node(d)?.walking.has_path
            && ctx.node(d)?.combat_running
            && ctx.node(d)?.victim == self.mover()
        {
            allowed = false;
        }
        if !allowed && path.read().len() == 1 {
            let point = path.read()[0].clone();
            let point = point.read();
            let [x, y, _] = loc.coordinates();
            if point.x() == x && point.y() == y {
                allowed = true;
            }
        }
        Ok(allowed)
    }
    pub fn update_path(&mut self, ctx: &mut impl Context) -> Result<()> {
        if kind(ctx, self.mover())? == Some(Kind::Npc) && node(ctx, self.mover())?.never_walks {
            return Ok(());
        }
        if self.destination().is_none() || node(ctx, self.destination())?.location.is_none() {
            return Ok(());
        }
        let mut loc = None;
        if let Some(option) = self.option() {
            loc = ctx.destination(
                self.mover(),
                self.destination(),
                DestinationService::Option(option),
            )?;
        } else if let Some(use_with) = self.use_with() {
            if kind(ctx, self.mover())?.is_some_and(|k| k != Kind::Player) {
                return Err(Error::Type);
            }
            loc = ctx.destination(
                self.mover(),
                self.destination(),
                DestinationService::UseWith(use_with),
            )?;
        } else if self.inside(ctx, &location(ctx, self.mover())?)? {
            loc = self.border(ctx, &location(ctx, self.destination())?)?;
        }
        if loc.is_none() && self.flag().is_some() && self.override_method().is_none() {
            loc = self
                .flag()
                .unwrap()
                .destination(ctx, self.mover(), self.destination())?;
        } else if loc.is_none()
            && let Some(method) = self.override_method()
        {
            loc = ctx.destination(
                self.mover(),
                self.destination(),
                DestinationService::Override(method),
            )?;
            let target = node(ctx, self.destination())?.location;
            if same(&loc, &target) {
                if let Some(flag) = self.flag() {
                    loc = flag.destination(ctx, self.mover(), self.destination())?;
                } else {
                    loc = None;
                }
            }
        }
        if kind(ctx, self.destination())? == Some(Kind::Npc)
            && node(ctx, self.mover())?.victim != self.destination()
        {
            let l = match loc.clone() {
                Some(l) => l,
                None => location(ctx, self.destination())?,
            };
            loc = self.interrupt(ctx, Some(l))?;
        }
        if self.interact().is_none() {
            self.state.write().interact = loc.clone();
        }
        let needs = self.target_entity(ctx)?
            || self.interact().is_none()
            || (node(ctx, self.mover())?.walking.points.read().len() <= 1
                && distance(
                    self.interact().as_ref().unwrap(),
                    &location(ctx, self.mover())?,
                ) > 0.0)
            || (self.truncated()
                && distance(
                    &location(ctx, self.destination())?,
                    &location(ctx, self.mover())?,
                ) < 14.0);
        if needs {
            if !self.allow_movement(ctx)? {
                return Ok(());
            }
            if self.target_entity(ctx)?
                && self.previous().is_some()
                && equal(&self.previous(), &loc)
                && node(ctx, self.mover())?.walking.has_path
            {
                return Ok(());
            }
            let l = match loc.clone() {
                Some(l) => l,
                None => location(ctx, self.destination())?,
            };
            let (truncated, l) = truncate(ctx, self.m()?, &l)?;
            let path = if truncated {
                let p = ctx.find(self, Target::Location(l), self.finder())?;
                self.state.write().truncated = true;
                p
            } else {
                let target = if let Some(l) = &loc {
                    Target::Location(l.clone())
                } else {
                    Target::Node(self.d()?)
                };
                let p = ctx.find(self, target, self.finder())?;
                self.state.write().interact = None;
                self.state.write().truncated = false;
                p
            };
            let path = path.ok_or(Error::Null)?;
            self.state.write().near = !path.read().successful || path.read().move_near;
            if !path.read().points.read().is_empty() {
                let _point = path
                    .read()
                    .points
                    .read()
                    .back()
                    .cloned()
                    .ok_or(Error::NoSuchElement)?;
                let mover = self.m()?;
                let running = if self.force_run() { Some(true) } else { None };
                ctx.walk_reset(Some(self), mover, running)?;
                let size = path.read().points.read().len();
                for i in 0..size {
                    let point = path
                        .read()
                        .points
                        .write()
                        .pop_front()
                        .ok_or(Error::NoSuchElement)?;
                    let p = point.read();
                    let (x, y) = (p.x(), p.y());
                    drop(p);
                    ctx.walk_add(self, self.m()?, x, y)?;
                    let target = if self.target_entity(ctx)? {
                        self.destination()
                    } else {
                        None
                    };
                    ctx.face(self, self.m()?, target)?;
                    if i == size - 1 && self.interact().is_none() {
                        let z = location(ctx, self.mover())?.coordinates()[2];
                        self.state.write().interact = Some(Location::new(x, y, z));
                    }
                }
            }
            self.state.write().previous = loc;
        }
        self.state.write().last = node(ctx, self.destination())?.location;
        if kind(ctx, self.mover())? == Some(Kind::Player) && node(ctx, self.mover())?.draw_intersect
        {
            ctx.hint_clear(self, self.m()?)?;
            ctx.hint_add(self, self.m()?, self.interact().clone(), 5)?;
        }
        Ok(())
    }
    fn interrupt(&self, ctx: &mut impl Context, loc: Option<Location>) -> Result<Option<Location>> {
        let ml = location(ctx, self.mover())?;
        let _dl = location(ctx, self.destination())?;
        if let Some(l) = loc.as_ref()
            && self.target_entity(ctx)?
            && node(ctx, self.destination())?.walking.has_path
        {
            let points = node(ctx, self.destination())?
                .walking
                .points
                .read()
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            if !points.is_empty() {
                let mut point = points[0].clone();
                let mut predictive = None;
                for (i, p) in points.iter().enumerate() {
                    let close = self.closest_border(ctx, p, l.coordinates()[2])?;
                    if !ctx.teleport_permitted(&close)? {
                        continue;
                    }
                    let [x, y, _] = ml.coordinates();
                    let [cx, cy, _] = close.coordinates();
                    let dist = x
                        .wrapping_sub(cx)
                        .wrapping_abs()
                        .max(y.wrapping_sub(cy).wrapping_abs());
                    let denominator = (i as i32 + 1)
                        / if node(ctx, self.mover())?.walking.running {
                            2
                        } else {
                            1
                        };
                    let ratio = dist as f32 / denominator as f32;
                    if predictive.is_none() && ratio <= 1.0 {
                        predictive = Some(p.clone());
                        break;
                    }
                    if dist <= 2 {
                        point = p.clone();
                    }
                }
                if let Some(p) = predictive {
                    point = p;
                }
                let end = self.closest_border(ctx, &point, l.coordinates()[2])?;
                if !ctx.teleport_permitted(&end)? {
                    return Ok(loc);
                }
                return Ok(Some(end));
            }
        }
        Ok(loc)
    }
    fn closest_border(&self, ctx: &impl Context, point: &Step, plane: i32) -> Result<Location> {
        let p = point.read();
        let predicted = Location::new(p.x(), p.y(), plane);
        drop(p);
        let diff = between(&location(ctx, self.destination())?, &predicted);
        let target = node(ctx, self.destination())?;
        let loc = target.location.ok_or(Error::Null)?;
        let top = transformed(
            &loc,
            target.size.wrapping_sub(1),
            target.size.wrapping_sub(1),
        );
        let [x, y, _] = loc.coordinates();
        let [tx, ty, _] = top.coordinates();
        let predicted = Location::new(
            ((x as f64 + tx as f64) / 2.0 + diff[0]).floor() as i32,
            ((y as f64 + ty as f64) / 2.0 + diff[1]).floor() as i32,
            plane,
        );
        let m = node(ctx, self.mover())?;
        let center = transformed(&m.location.ok_or(Error::Null)?, m.size >> 1, m.size >> 1);
        let to = between(&predicted, &center);
        let mag = magnitude(to);
        let scale = node(ctx, self.destination())?.size.wrapping_add(1) as f64;
        Ok(transform_vector(
            &predicted,
            to[0] / mag * scale,
            to[1] / mag * scale,
        ))
    }
    fn border(&self, ctx: &mut impl Context, center_dest: &Location) -> Result<Option<Location>> {
        let size = node(ctx, self.destination())?.size;
        let cd = transformed(center_dest, size >> 1, size >> 1);
        let ms = node(ctx, self.mover())?.size;
        let center = transformed(&location(ctx, self.mover())?, ms >> 1, ms >> 1);
        let mut direction = logical(&cd, &center);
        let [x, y, _] = center_dest.coordinates();
        let [mx, my, mz] = location(ctx, self.mover())?.coordinates();
        let delta = Location::new(
            mx.wrapping_sub(x),
            my.wrapping_sub(y),
            mz.wrapping_sub(center_dest.coordinates()[2]),
        );
        let mut budget = 0usize;
        'main: for _ in 0..4 {
            let [dx, dy, _] = delta.coordinates();
            let amount = match direction {
                Direction::North => size.wrapping_sub(dy),
                Direction::East => size.wrapping_sub(dx),
                Direction::South => node(ctx, self.mover())?.size.wrapping_add(dy),
                Direction::West => node(ctx, self.mover())?.size.wrapping_add(dx),
                _ => return Ok(None),
            };
            for j in 0..amount {
                let mut s = 0i32;
                while s < node(ctx, self.mover())?.size {
                    budget += 1;
                    if budget > 65_536 {
                        return Err(Error::Limit);
                    }
                    let offset = match direction {
                        Direction::North => (s, j.wrapping_add(node(ctx, self.mover())?.size)),
                        Direction::East => (j.wrapping_add(node(ctx, self.mover())?.size), s),
                        Direction::South => (s, j.wrapping_add(1).wrapping_neg()),
                        Direction::West => (j.wrapping_add(1).wrapping_neg(), s),
                        _ => return Ok(None),
                    };
                    if !can_move(
                        ctx,
                        &transformed(&location(ctx, self.mover())?, offset.0, offset.1),
                        direction,
                    )? {
                        direction =
                            Direction::from_code((direction.code() + 1) & 3).ok_or(Error::Index)?;
                        continue 'main;
                    }
                    s = s.wrapping_add(1);
                }
            }
            let (dx, dy) = direction.step();
            return Ok(Some(transformed(
                &location(ctx, self.mover())?,
                dx.wrapping_mul(amount),
                dy.wrapping_mul(amount),
            )));
        }
        Ok(None)
    }
}
