//! Original base NPC creation, initialization and configuration orchestration.
//! The owning world supplies stable nodes, canonical entity fields, properties,
//! skills, repository/content services and roots for captured child references.
use super::{doors::Location, npc};
use crate::definitions::{
    item::Shared,
    npc::{self as defs, Graphics, Handle as Definition},
    property::{Animation, Property},
};
use rs09_game_core::direction::Direction;
pub type Node = usize;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub enum Error {
    Null,
    Index,
    Type,
    Limit,
    Definition(defs::Error),
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<defs::Error> for Error {
    fn from(e: defs::Error) -> Self {
        Self::Definition(e)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Attack,
    Defence,
    Death,
    Magic,
    Range,
}
pub enum Change {
    CombatLevel(i32),
    Animation(Slot, Option<Shared<Animation>>),
    Bonuses(Shared<Vec<i32>>),
    AttackSpeed(i32),
    /// Original public-field assignment, rather than a setter callback.
    DeathGraphic(Shared<Graphics>),
}
#[derive(Clone, Copy, Debug)]
pub enum Skill {
    Static,
    Current,
}
pub trait Context {
    type Arguments;
    fn prototype(&mut self, id: i32) -> Result<Option<Node>>;
    fn construct(
        &mut self,
        prototype: Node,
        id: i32,
        location: Option<Location>,
        objects: &Self::Arguments,
    ) -> Result<Option<Node>>;
    fn base(
        &mut self,
        id: i32,
        location: Option<Location>,
        direction: Option<Direction>,
    ) -> Result<Node>;
    fn definition(&self, node: Node) -> Result<Option<Definition>>;
    fn brain(&self, node: Node) -> Result<npc::State>;
    fn location(&self, node: Node) -> Result<Option<Location>>;
    fn index(&self, node: Node) -> Result<i32>;
    fn size(&self, node: Node) -> Result<i32>;
    fn set_index(&mut self, node: Node, index: i32) -> Result<()>;
    fn set_size(&mut self, node: Node, size: i32) -> Result<()>;
    fn children(&self, node: Node) -> Result<Option<Shared<Vec<Option<Node>>>>>;
    fn set_children(&mut self, node: Node, children: Shared<Vec<Option<Node>>>) -> Result<()>;
    fn entity_init(&mut self, node: Node) -> Result<()>;
    fn properties(&mut self, node: Node) -> Result<Option<usize>>;
    fn skills(&mut self, node: Node) -> Result<Option<usize>>;
    fn set_property(&mut self, properties: Option<usize>, change: Change) -> Result<()>;
    fn get_animation(
        &mut self,
        properties: Option<usize>,
        slot: Slot,
    ) -> Result<Option<Shared<Animation>>>;
    fn set_skill(&mut self, skills: Option<usize>, skill: Skill, id: i32, level: i32)
    -> Result<()>;
    fn maximum_life_points(&mut self, skills: Option<usize>) -> Result<i32>;
    fn familiar(&self, node: Node) -> Result<bool>;
    fn attribute_hp(&mut self, node: Node, fallback: i32) -> Result<i32>;
    fn spawn_location(
        &mut self,
        properties: Option<usize>,
        location: Option<Location>,
    ) -> Result<()>;
    fn add_repository(&mut self, node: Node) -> Result<()>;
    fn region_move(
        &mut self,
        node: Node,
        from: Option<Location>,
        to: Option<Location>,
    ) -> Result<()>;
    fn region_active(&mut self, location: Location) -> Result<bool>;
    fn add_renderable(&mut self, node: Node) -> Result<()>;
    fn interact_default(&mut self, node: Node) -> Result<()>;
    fn configure(&mut self, node: Node) -> Result<()>;
    fn default_behavior(&mut self, node: Node) -> Result<()>;
    fn behavior_created(&mut self, node: Node) -> Result<()>;
}
pub fn create<C: Context>(
    ctx: &mut C,
    id: i32,
    location: Option<Location>,
    direction: Option<Direction>,
    objects: &C::Arguments,
) -> Result<Node> {
    let mut node = ctx.prototype(id)?;
    if let Some(prototype) = node {
        node = ctx.construct(prototype, id, location.clone(), objects)?;
    }
    match node {
        Some(node) => Ok(node),
        None => ctx.base(id, location, direction),
    }
}
fn definition(ctx: &impl Context, node: Node) -> Result<Definition> {
    ctx.definition(node)?.ok_or(Error::Null)
}
fn value(ctx: &impl Context, node: Node, key: &str) -> Result<Option<Property>> {
    let d = definition(ctx, node)?;
    let handlers = d.read().handlers.clone();
    Ok(handlers.read().get(key).cloned())
}
fn integer(ctx: &impl Context, node: Node, key: &str, fallback: i32) -> Result<i32> {
    match value(ctx, node, key)? {
        None | Some(Property::Null) => Ok(fallback),
        Some(Property::Int(v)) => Ok(v),
        _ => Err(Error::Type),
    }
}
fn animation(ctx: &impl Context, node: Node, index: i32) -> Result<Option<Shared<Animation>>> {
    Ok(definition(ctx, node)?.read().combat_animation(index)?)
}
fn combat(ctx: &impl Context, node: Node) -> Result<i32> {
    Ok(definition(ctx, node)?.read().combat_level)
}
pub fn init_config(ctx: &mut impl Context, node: Node) -> Result<()> {
    let default = combat(ctx, node)? / 2;
    let p = ctx.properties(node)?;
    let level = combat(ctx, node)?;
    ctx.set_property(p, Change::CombatLevel(level))?;
    for (id, key) in [
        (0, "attack_level"),
        (2, "strength_level"),
        (1, "defence_level"),
        (4, "range_level"),
        (6, "magic_level"),
        (3, "lifepoints"),
    ] {
        let skills = ctx.skills(node)?;
        let level = integer(ctx, node, key, default)?;
        ctx.set_skill(skills, Skill::Static, id, level)?;
    }
    let skills = ctx.skills(node)?;
    let mut lp = ctx.maximum_life_points(skills)?;
    if ctx.familiar(node)? {
        lp = ctx.attribute_hp(node, lp)?;
    }
    let skills = ctx.skills(node)?;
    ctx.set_skill(skills, Skill::Current, 3, lp)?;
    let fallback = ctx.brain(node)?.fields.read().aggressive;
    let aggressive = match value(ctx, node, "aggressive")? {
        None | Some(Property::Null) => fallback,
        Some(Property::Bool(v)) => v,
        _ => return Err(Error::Type),
    };
    ctx.brain(node)?.fields.write().aggressive = aggressive;
    let mut first = None;
    for i in 0..3 {
        if animation(ctx, node, i)?.is_some() {
            let p = ctx.properties(node)?;
            first = animation(ctx, node, i)?;
            ctx.set_property(p, Change::Animation(Slot::Attack, first.clone()))?;
            break;
        }
    }
    for (index, slot) in [
        (3, Slot::Defence),
        (4, Slot::Death),
        (1, Slot::Magic),
        (2, Slot::Range),
    ] {
        if animation(ctx, node, index)?.is_some() {
            let p = ctx.properties(node)?;
            let a = animation(ctx, node, index)?;
            ctx.set_property(p, Change::Animation(slot, a))?;
        }
    }
    for slot in [Slot::Attack, Slot::Magic, Slot::Range] {
        let p = ctx.properties(node)?;
        if ctx.get_animation(p, slot)?.is_none() && first.is_some() {
            let p = ctx.properties(node)?;
            ctx.set_property(p, Change::Animation(slot, first.clone()))?;
        }
    }
    let d = definition(ctx, node)?;
    let argument = definition(ctx, node)?.read().handlers.clone();
    defs::init_combat_graphics(&d, &argument)?;
    let p = ctx.properties(node)?;
    let bonuses = match value(ctx, node, "bonuses")? {
        None | Some(Property::Null) => Shared::new(vec![0; 15]),
        Some(Property::IntArray(v)) => v,
        _ => return Err(Error::Type),
    };
    ctx.set_property(p, Change::Bonuses(bonuses))?;
    let p = ctx.properties(node)?;
    let speed = integer(ctx, node, "attack_speed", 5)?;
    ctx.set_property(p, Change::AttackSpeed(speed))?;
    let talk = match value(ctx, node, "force_talk")? {
        None | Some(Property::Null) => None,
        Some(Property::Text(s)) => Some(s.encode_utf16().collect()),
        _ => return Err(Error::Type),
    };
    ctx.brain(node)?.fields.write().force_talk = talk;
    if value(ctx, node, "movement_radius")?.is_some_and(|v| !v.is_null()) {
        let radius = integer(ctx, node, "movement_radius", 0)?;
        ctx.brain(node)?.fields.write().radius = radius;
    }
    if value(ctx, node, "death_gfx")?.is_some_and(|v| !v.is_null()) {
        let p = ctx.properties(node)?;
        let id = integer(ctx, node, "death_gfx", 0)?;
        ctx.set_property(
            p,
            Change::DeathGraphic(Shared::new(Graphics {
                id,
                height: 0,
                delay: 0,
            })),
        )?;
    }
    Ok(())
}
pub fn init(ctx: &mut impl Context, node: Node) -> Result<()> {
    ctx.entity_init(node)?;
    let p = ctx.properties(node)?;
    let location = ctx.location(node)?;
    ctx.spawn_location(p, location)?;
    init_config(ctx, node)?;
    ctx.add_repository(node)?;
    let location = ctx.location(node)?;
    ctx.region_move(node, None, location)?;
    let location = ctx.location(node)?.ok_or(Error::Null)?;
    if ctx.region_active(location)? {
        ctx.add_renderable(node)?;
    }
    ctx.interact_default(node)?;
    ctx.configure(node)?;
    ctx.default_behavior(node)?;
    let children = definition(ctx, node)?.read().child_npc_ids.clone();
    if let Some(children) = children {
        let count = children.read().len();
        if count > 65_536 {
            return Err(Error::Limit);
        }
        ctx.set_children(node, Shared::new(vec![None; count]))?;
        let mut i = 0;
        while i < ctx.children(node)?.ok_or(Error::Null)?.read().len() {
            if i >= 65_536 {
                return Err(Error::Limit);
            }
            let out = ctx.children(node)?.ok_or(Error::Null)?;
            let ids = definition(ctx, node)?
                .read()
                .child_npc_ids
                .clone()
                .ok_or(Error::Null)?;
            let id = *ids.read().get(i).ok_or(Error::Index)?;
            let child = ctx.base(id, None, Some(Direction::South))?;
            *out.write().get_mut(i).ok_or(Error::Index)? = Some(child);
            ctx.interact_default(child)?;
            let index = ctx.index(node)?;
            ctx.set_index(child, index)?;
            let size = ctx.size(node)?;
            ctx.set_size(child, size)?;
            i += 1;
        }
    }
    ctx.behavior_created(node)
}
