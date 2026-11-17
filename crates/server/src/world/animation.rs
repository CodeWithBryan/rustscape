//! Complete Animation/Graphics records and Animator control flow. Definition
//! lookup, current world tick, actual clocks/masks and chunk flags are bindings.
use super::doors::{self, Location};
use crate::definitions::{animation as defs, item::Shared};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[repr(i32)]
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
    pub fn from_ordinal(n: i32) -> Option<Self> {
        [Self::Low, Self::Mid, Self::High, Self::VeryHigh]
            .get(n as usize)
            .copied()
    }
}
#[derive(Debug, PartialEq, Eq)]
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
/// The one actual Animation object shared by configuration, properties and
/// Animator. Cached definition/object state is not configuration serialization.
#[derive(serde::Serialize)]
pub struct AnimationFields {
    pub id: i32,
    pub delay: i32,
    pub priority: Option<Priority>,
    #[serde(skip_serializing)]
    definition: Option<defs::Handle>,
    #[serde(skip_serializing)]
    object: Option<doors::Handle>,
}
impl AnimationFields {
    pub fn high(id: i32) -> Self {
        Self {
            id,
            delay: 0,
            priority: Some(Priority::High),
            definition: None,
            object: None,
        }
    }
}
impl std::fmt::Debug for AnimationFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animation")
            .field("id", &self.id)
            .field("delay", &self.delay)
            .field("priority", &self.priority)
            .finish()
    }
}
#[derive(Clone)]
pub struct Animation(Shared<AnimationFields>);
pub trait Definitions {
    fn definition(&mut self, animation: &Animation, id: i32) -> Option<defs::Handle>;
}
impl Animation {
    pub fn from_shared(fields: Shared<AnimationFields>) -> Self {
        Self(fields)
    }
    pub fn shared(&self) -> Shared<AnimationFields> {
        self.0.clone()
    }
    pub fn new(id: i32) -> Self {
        Self::with(id, 0, Some(Priority::Mid))
    }
    pub fn with(id: i32, delay: i32, priority: Option<Priority>) -> Self {
        Self(Shared::new(AnimationFields {
            id,
            delay,
            priority,
            definition: None,
            object: None,
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn id(&self) -> i32 {
        self.0.read().id
    }
    pub fn delay(&self) -> i32 {
        self.0.read().delay
    }
    pub fn priority(&self) -> Option<Priority> {
        self.0.read().priority
    }
    pub fn set_id(&self, id: i32) {
        self.0.write().id = id;
    }
    pub fn set_priority(&self, priority: Option<Priority>) {
        self.0.write().priority = priority;
    }
    pub fn object(&self) -> Option<doors::Handle> {
        self.0.read().object.clone()
    }
    pub fn set_object(&self, object: Option<doors::Handle>) {
        self.0.write().object = object;
    }
    pub fn cached_definition(&self) -> Option<defs::Handle> {
        self.0.read().definition.clone()
    }
    pub fn definition(&self, ctx: &mut impl Definitions) -> Option<defs::Handle> {
        if let Some(definition) = self.cached_definition() {
            return Some(definition);
        }
        let id = self.id();
        let definition = ctx.definition(self, id);
        self.0.write().definition = definition.clone();
        definition
    }
    pub fn duration(&self, ctx: &mut impl Definitions) -> i32 {
        self.definition(ctx)
            .map_or(1, |d| d.read().duration_ticks())
    }
}
impl std::fmt::Display for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Animation [priority={}, id={}]",
            self.priority().map_or("null", Priority::name),
            self.id()
        )
    }
}
struct GraphicFields {
    id: i32,
    height: i32,
    delay: i32,
}
#[derive(Clone)]
pub struct Graphics(Arc<GraphicFields>);
impl Graphics {
    pub fn new(id: i32, height: i32, delay: i32) -> Self {
        Self(Arc::new(GraphicFields { id, height, delay }))
    }
    pub fn id(&self) -> i32 {
        self.0.id
    }
    pub fn height(&self) -> i32 {
        self.0.height
    }
    pub fn delay(&self) -> i32 {
        self.0.delay
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn send(
        ctx: &mut impl GraphicDelivery,
        graphic: Option<Self>,
        location: Option<Location>,
    ) -> Result<()> {
        let chunk = ctx.chunk(location.clone())?;
        let flag = ctx.make_flag(graphic, location)?;
        ctx.enqueue(chunk, flag)
    }
}
impl std::fmt::Display for Graphics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Graphics [id={}, height={}, delay={}]",
            self.id(),
            self.height(),
            self.delay()
        )
    }
}
pub trait GraphicDelivery {
    type Chunk;
    type Flag;
    fn chunk(&mut self, location: Option<Location>) -> Result<Option<Self::Chunk>>;
    fn make_flag(
        &mut self,
        graphic: Option<Graphics>,
        location: Option<Location>,
    ) -> Result<Self::Flag>;
    fn enqueue(&mut self, chunk: Option<Self::Chunk>, flag: Self::Flag) -> Result<()>;
}
struct ResetFields {
    animation: Animation,
    animator_animation: Animation,
    graphics: Graphics,
}
/// One factory per owning world, retaining the three distinct original static
/// reset objects and their mutable Animation aliases.
#[derive(Clone)]
pub struct Factory(Arc<ResetFields>);
impl Default for Factory {
    fn default() -> Self {
        Self(Arc::new(ResetFields {
            animation: Animation::with(-1, 0, Some(Priority::VeryHigh)),
            animator_animation: Animation::new(-1),
            graphics: Graphics::new(-1, 0, 0),
        }))
    }
}
impl Factory {
    pub fn animation_reset(&self) -> Animation {
        self.0.animation.clone()
    }
    pub fn animator_reset(&self) -> Animation {
        self.0.animator_animation.clone()
    }
    pub fn graphics_reset(&self) -> Graphics {
        self.0.graphics.clone()
    }
}
struct Fields<E> {
    entity: Option<E>,
    animation: Option<Animation>,
    graphics: Option<Graphics>,
    priority: Option<Priority>,
    ticks: i32,
}
pub struct Animator<E> {
    fields: Shared<Fields<E>>,
    factory: Factory,
}
impl<E> Clone for Animator<E> {
    fn clone(&self) -> Self {
        Self {
            fields: self.fields.clone(),
            factory: self.factory.clone(),
        }
    }
}
pub trait Context<E: Clone>: Definitions {
    fn world_ticks(&mut self, animator: &Animator<E>) -> Result<i32>;
    fn clocks(
        &mut self,
        animator: &Animator<E>,
        entity: Option<&E>,
    ) -> Result<Option<Shared<Vec<i32>>>>;
    fn register_animation(
        &mut self,
        animator: &Animator<E>,
        entity: Option<&E>,
        animation: Animation,
    ) -> Result<bool>;
    fn register_graphics(
        &mut self,
        animator: &Animator<E>,
        entity: Option<&E>,
        graphics: Graphics,
    ) -> Result<bool>;
}
impl<E: Clone> Animator<E> {
    pub fn new(entity: Option<E>, factory: Factory) -> Self {
        Self {
            fields: Shared::new(Fields {
                entity,
                animation: None,
                graphics: None,
                priority: Some(Priority::Low),
                ticks: 0,
            }),
            factory,
        }
    }
    pub fn same(&self, other: &Self) -> bool {
        self.fields.same(&other.fields)
    }
    pub fn owner(&self) -> Option<E> {
        self.fields.read().entity.clone()
    }
    pub fn animation(&self) -> Option<Animation> {
        self.fields.read().animation.clone()
    }
    pub fn set_animation(&self, value: Option<Animation>) {
        self.fields.write().animation = value;
    }
    pub fn graphics(&self) -> Option<Graphics> {
        self.fields.read().graphics.clone()
    }
    pub fn set_graphics(&self, value: Option<Graphics>) {
        self.fields.write().graphics = value;
    }
    pub fn priority(&self) -> Option<Priority> {
        self.fields.read().priority
    }
    pub fn set_priority(&self, value: Option<Priority>) {
        self.fields.write().priority = value;
    }
    pub fn deadline(&self) -> i32 {
        self.fields.read().ticks
    }
    fn clock(&self, ctx: &mut impl Context<E>, value: impl FnOnce() -> i32) -> Result<()> {
        let owner = self.owner();
        let clocks = ctx.clocks(self, owner.as_ref())?.ok_or(Error::Null)?;
        let right = value();
        *clocks.write().get_mut(1).ok_or(Error::Index)? = right;
        Ok(())
    }
    pub fn animate(
        &self,
        ctx: &mut impl Context<E>,
        animation: Option<Animation>,
        graphic: Option<Graphics>,
    ) -> Result<bool> {
        if let Some(animation) = animation {
            let ticks = self.deadline();
            let now = ctx.world_ticks(self)?;
            if ticks > now
                && self.priority().ok_or(Error::Null)? as i32
                    > animation.priority().ok_or(Error::Null)? as i32
            {
                return Ok(false);
            }
            if animation.id() == 0 {
                animation.set_id(-1);
            }
            self.fields.write().animation = Some(animation.clone());
            let end = if animation.id() != -1 {
                let now = ctx.world_ticks(self)?;
                now.wrapping_add(animation.duration(ctx))
            } else {
                0
            };
            self.fields.write().ticks = end;
            self.clock(ctx, || self.deadline())?;
            let owner = self.owner();
            ctx.register_animation(self, owner.as_ref(), animation.clone())?;
            self.fields.write().priority = animation.priority();
        }
        if let Some(graphic) = graphic {
            self.fields.write().graphics = Some(graphic.clone());
            let owner = self.owner();
            ctx.register_graphics(self, owner.as_ref(), graphic)?;
        }
        Ok(true)
    }
    pub fn stop(&self, ctx: &mut impl Context<E>) -> Result<()> {
        self.animate(ctx, Some(self.factory.animator_reset()), None)
            .map(|_| ())
    }
    pub fn force_animation(
        &self,
        ctx: &mut impl Context<E>,
        animation: Option<Animation>,
    ) -> Result<()> {
        self.fields.write().ticks = -1;
        self.animate(ctx, animation, None)?;
        self.fields.write().priority = Some(Priority::High);
        Ok(())
    }
    pub fn reset(&self, ctx: &mut impl Context<E>) -> Result<()> {
        self.animate(ctx, Some(self.factory.animator_reset()), None)?;
        self.clock(ctx, || 0)?;
        self.fields.write().ticks = 0;
        Ok(())
    }
    pub fn is_animating(&self, ctx: &mut impl Context<E>) -> Result<bool> {
        if self.animation().is_none_or(|a| a.id() == -1) {
            return Ok(false);
        }
        let ticks = self.deadline();
        Ok(ticks > ctx.world_ticks(self)?)
    }
}
