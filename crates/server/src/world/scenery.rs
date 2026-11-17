//! SceneryBuilder state transitions and its four captured pulse bodies.
//! Landscape, chunks, concrete definition creation and ground spawning are bindings.
use super::doors::{Handle, Location};
use crate::{definitions::item::Shared, pulses};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    Direction,
    State(&'static str),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub type Chunk = usize;
pub type GroundItems = Shared<Box<[Option<usize>]>>;
#[derive(Clone)]
pub struct ObjectUpdate {
    pub object: Handle,
    pub remove: bool,
}
impl ObjectUpdate {
    pub fn frame(&self) -> rs09_protocol::game::Frame {
        let object = self.object.read();
        let [x, y, _] = object.location.coordinates();
        if self.remove {
            rs09_protocol::game::updates::loc_del(x & 7, y & 7, object.shape, object.rotation)
        } else {
            rs09_protocol::game::updates::loc_add(
                x & 7,
                y & 7,
                object.shape,
                object.rotation,
                object.id,
            )
        }
    }
}
#[derive(Clone)]
pub enum Action {
    ClientRestore {
        remove: Handle,
        construct: Handle,
    },
    Restore {
        object: Handle,
    },
    Expire {
        object: Handle,
        items: Option<GroundItems>,
    },
    Respawn {
        object: Handle,
    },
}
#[derive(Clone)]
pub struct SceneryPulse {
    pub pulse: pulses::Handle,
    pub action: Action,
    pub check: Option<Handle>,
}
impl SceneryPulse {
    fn new(delay: i32, action: Action, check: Option<Handle>, suffix: i32) -> Self {
        let checks = check
            .as_ref()
            .map(|h| Shared::new(vec![Some(h.identity())]));
        Self {
            pulse: Shared::new(pulses::Pulse::new(
                delay,
                checks,
                pulses::Class::Other,
                format!("core.game.node.scenery.SceneryBuilder${suffix}"),
            )),
            action,
            check,
        }
    }
}
pub trait Context {
    fn construct(
        &mut self,
        id: i32,
        location: Location,
        shape: i32,
        rotation: i32,
    ) -> Result<Handle>;
    fn remove_landscape(&mut self, object: &Handle) -> Result<Option<Handle>>;
    fn add_landscape(&mut self, object: &Handle) -> Result<()>;
    fn chunk(&mut self, location: Location) -> Result<Chunk>;
    fn flag(&mut self, chunk: Chunk, update: ObjectUpdate) -> Result<()>;
    fn stop(&mut self, pulse: &pulses::Handle) -> Result<()>;
    fn pulse_direct(&mut self, pulse: &pulses::Handle) -> Result<bool>;
    fn submit(&mut self, pulse: SceneryPulse) -> Result<()>;
    fn ground_create(&mut self, item: Option<usize>) -> Result<()>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplaceMode {
    Swap { clip: bool, permanent: bool },
    Temporary { clip: bool, ticks: i32 },
}
impl Default for ReplaceMode {
    fn default() -> Self {
        Self::Swap {
            clip: true,
            permanent: false,
        }
    }
}
pub fn wrapper(h: &Handle) -> Handle {
    super::doors::wrapper(h)
}
pub fn equals(a: &Handle, b: Option<&Handle>) -> bool {
    let Some(b) = b else {
        return false;
    };
    let (id, shape, rotation, loc) = {
        let a = a.read();
        (a.id, a.shape, a.rotation, a.location.coordinates())
    };
    let b = b.read();
    id == b.id && shape == b.shape && rotation == b.rotation && loc == b.location.coordinates()
}
pub fn as_constructed(ctx: &mut impl Context, h: &Handle) -> Result<Handle> {
    if h.read().constructed {
        return Ok(h.clone());
    }
    let (id, l, shape, rot) = {
        let h = h.read();
        (h.id, h.location.clone(), h.shape, h.rotation)
    };
    let h = ctx.construct(id, l, shape, rot)?;
    h.write().constructed = true;
    Ok(h)
}
pub fn set_active(ctx: &mut impl Context, h: &Handle, active: bool) -> Result<()> {
    let destruction = {
        let h = h.read();
        if h.active && !active {
            h.destruction_pulse.clone()
        } else {
            None
        }
    };
    if let Some(pulse) = destruction {
        ctx.pulse_direct(&pulse)?;
    }
    h.write().active = active;
    Ok(())
}
fn client_replace(
    ctx: &mut impl Context,
    remove: Option<&Handle>,
    construct: Option<&Handle>,
    ticks: i32,
) -> Result<bool> {
    let remove = remove.ok_or(Error::Null)?;
    let location = remove.read().location.clone();
    let chunk = ctx.chunk(location)?;
    ctx.flag(
        chunk,
        ObjectUpdate {
            object: remove.clone(),
            remove: true,
        },
    )?;
    let construct = construct.ok_or(Error::Null)?;
    let location = construct.read().location.clone();
    let chunk = ctx.chunk(location)?;
    ctx.flag(
        chunk,
        ObjectUpdate {
            object: construct.clone(),
            remove: false,
        },
    )?;
    if ticks > 0 {
        ctx.submit(SceneryPulse::new(
            ticks,
            Action::ClientRestore {
                remove: remove.clone(),
                construct: construct.clone(),
            },
            None,
            1,
        ))?;
    }
    Ok(true)
}
pub fn replace(
    ctx: &mut impl Context,
    remove: Option<&Handle>,
    construct: Option<&Handle>,
    mode: ReplaceMode,
) -> Result<bool> {
    let (clip, ticks) = match mode {
        ReplaceMode::Swap { clip, .. } => (clip, -1),
        ReplaceMode::Temporary { clip, ticks } => (clip, ticks),
    };
    if !clip {
        return client_replace(ctx, remove, construct, ticks);
    }
    let remove = wrapper(remove.ok_or(Error::Null)?);
    let Some(current) = ctx.remove_landscape(&remove)? else {
        return Ok(false);
    };
    let restore = current.read().restore_pulse.clone();
    if let Some(p) = restore {
        ctx.stop(&p)?;
        current.write().restore_pulse = None;
    }
    let previous = {
        let c = current.read();
        if c.constructed {
            c.replaced.clone()
        } else {
            None
        }
    };
    if let Some(previous) = previous {
        if equals(&previous, construct) {
            if matches!(mode, ReplaceMode::Temporary { .. }) {
                return Err(Error::State(
                    "Can't temporarily replace an already temporary object!",
                ));
            }
            ctx.add_landscape(&previous)?;
            update(ctx, &[Some(current), Some(previous)])?;
            return Ok(true);
        }
    }
    let constructed = as_constructed(ctx, construct.ok_or(Error::Null)?)?;
    if !matches!(
        mode,
        ReplaceMode::Swap {
            permanent: true,
            ..
        }
    ) {
        constructed.write().replaced = Some(current.clone());
    }
    ctx.add_landscape(&constructed)?;
    update(ctx, &[Some(current), Some(constructed.clone())])?;
    if let ReplaceMode::Temporary { ticks, .. } = mode {
        if ticks < 0 {
            return Ok(true);
        }
        let pulse = SceneryPulse::new(
            ticks,
            Action::Restore {
                object: constructed.clone(),
            },
            None,
            2,
        );
        constructed.write().restore_pulse = Some(pulse.pulse.clone());
        ctx.submit(pulse)?;
    }
    Ok(true)
}
pub fn add(
    ctx: &mut impl Context,
    object: Option<&Handle>,
    ticks: i32,
    items: Option<GroundItems>,
) -> Result<Handle> {
    let object = wrapper(object.ok_or(Error::Null)?);
    let constructed = as_constructed(ctx, &object)?;
    ctx.add_landscape(&constructed)?;
    update(ctx, &[Some(constructed.clone())])?;
    if ticks > -1 {
        ctx.submit(SceneryPulse::new(
            ticks,
            Action::Expire {
                object: constructed.clone(),
                items,
            },
            Some(object),
            3,
        ))?;
    }
    Ok(constructed)
}
pub fn remove(ctx: &mut impl Context, object: Option<&Handle>) -> Result<bool> {
    let Some(object) = object else {
        return Ok(false);
    };
    let object = wrapper(object);
    let Some(current) = ctx.remove_landscape(&object)? else {
        return Ok(false);
    };
    update(ctx, &[Some(current)])?;
    Ok(true)
}
pub fn remove_respawn(ctx: &mut impl Context, object: Option<&Handle>, ticks: i32) -> Result<bool> {
    if remove(ctx, object)? {
        ctx.submit(SceneryPulse::new(
            ticks,
            Action::Respawn {
                object: object.ok_or(Error::Null)?.clone(),
            },
            None,
            4,
        ))?;
        return Ok(true);
    }
    Ok(false)
}
pub fn update(ctx: &mut impl Context, objects: &[Option<Handle>]) -> Result<()> {
    for object in objects.iter().flatten() {
        let location = object.read().location.clone();
        let chunk = ctx.chunk(location)?;
        let remove = !object.read().active;
        ctx.flag(
            chunk,
            ObjectUpdate {
                object: object.clone(),
                remove,
            },
        )?;
    }
    Ok(())
}
pub fn pulse_body(ctx: &mut impl Context, pulse: &SceneryPulse) -> Result<bool> {
    match &pulse.action {
        Action::ClientRestore { remove, construct } => {
            client_replace(ctx, Some(construct), Some(remove), -1)
        }
        Action::Restore { object } => {
            let previous = object.read().replaced.clone();
            replace(ctx, Some(object), previous.as_ref(), ReplaceMode::default())?;
            Ok(true)
        }
        Action::Expire { object, items } => {
            remove(ctx, Some(object))?;
            if let Some(items) = items {
                let count = items.read().len();
                for i in 0..count {
                    let item = items.read()[i];
                    ctx.ground_create(item)?;
                }
            }
            Ok(true)
        }
        Action::Respawn { object } => {
            add(
                ctx,
                Some(object),
                -1,
                Some(Shared::new(Vec::new().into_boxed_slice())),
            )?;
            Ok(true)
        }
    }
}
