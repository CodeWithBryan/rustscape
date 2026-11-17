//! Scenery construction and its retained definition/morph family. Player config
//! access and logging remain explicit bindings; collision/storage use scenery.rs.
use super::{
    doors::{Handle, Location, Scenery},
    scenery,
};
use crate::{
    definitions::{
        item::Shared,
        scenery::{self as definitions, Registry},
    },
    player::attributes::GameAttributes,
};
#[derive(Debug)]
pub enum Error {
    Definition(definitions::Error),
    Scenery(super::doors::Error),
    Null,
    Index,
    Limit,
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<definitions::Error> for Error {
    fn from(e: definitions::Error) -> Self {
        Self::Definition(e)
    }
}
impl From<super::doors::Error> for Error {
    fn from(e: super::doors::Error) -> Self {
        Self::Scenery(e)
    }
}
pub trait Context: definitions::PlayerContext {
    fn varbit_exists(&mut self, id: i32) -> Result<bool>;
    fn set_varbit(&mut self, player: usize, id: i32, value: i32) -> Result<()>;
    fn set_varp(&mut self, player: usize, id: i32, value: i32) -> Result<()>;
    fn log_invalid_shape(&mut self, shape: i32, constructed: bool) -> Result<()>;
}
/// Constructs a complete morph family. Descendant edges retain children; weak
/// parent edges avoid cycles. Returned child views retain their exposed owner.
pub fn construct(
    registry: &mut Registry,
    id: i32,
    location: Location,
    shape: i32,
    rotation: i32,
    constructed: bool,
) -> Result<Handle> {
    // Original constructor fields plus recursion/allocation bounds.
    #[allow(clippy::too_many_arguments)]
    fn build(
        registry: &mut Registry,
        id: i32,
        l: Location,
        shape: i32,
        rotation: i32,
        constructed: bool,
        depth: usize,
        count: &mut usize,
    ) -> Result<Handle> {
        if depth > 256 || *count >= 65536 {
            return Err(Error::Limit);
        }
        *count += 1;
        let name = registry.for_id(id).read().name.clone();
        let mut object = Scenery::new(id, l.clone(), shape, rotation, name, false)?;
        let definition = registry.for_id(id);
        object.node_size = definition.read().size_x;
        object.definition = Some(definition.clone());
        object.constructed = constructed;
        let shape = object.shape;
        let rotation = object.rotation;
        let h = Shared::new(object);
        let children = definition.read().children_ids.clone();
        if let Some(children) = children.filter(|c| !c.read().is_empty()) {
            let length = children.read().len();
            let mut values = Vec::with_capacity(length);
            for i in 0..length {
                let child = build(
                    registry,
                    children.read()[i],
                    l.clone(),
                    shape,
                    rotation,
                    false,
                    depth + 1,
                    count,
                )?;
                child.write().parent_wrapper = Some(h.downgrade());
                values.push(child);
            }
            h.write().children = Some(Shared::new(values));
        }
        Ok(h)
    }
    build(
        registry,
        id,
        location,
        shape,
        rotation,
        constructed,
        0,
        &mut 0,
    )
}
pub struct Children {
    values: Shared<Vec<Handle>>,
    owner: Handle,
}
impl Clone for Children {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            owner: self.owner.clone(),
        }
    }
}
impl Children {
    pub fn len(&self) -> usize {
        self.values.read().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Option<Handle> {
        self.values
            .read()
            .get(index)
            .map(|h| h.keeping_from(&self.owner))
    }
    pub fn identity(&self) -> usize {
        self.values.identity()
    }
}
pub fn children(h: &Handle) -> Option<Children> {
    h.read().children.clone().map(|values| Children {
        values,
        owner: h.clone(),
    })
}
pub fn set_wrapper(h: &Handle, wrapper: Option<Handle>) {
    let mut h = h.write();
    h.parent_wrapper = None;
    h.wrapper = wrapper;
}
/// Recreate the family anchor for a canonical node's exposed receiver. Internal
/// arena edges stay unowned, avoiding parent/child cycles through that arena.
pub(crate) fn retained_view(h: &Handle) -> Result<Handle> {
    let mut owner = h.clone();
    for _ in 0..257 {
        let parent = owner
            .read()
            .parent_wrapper
            .as_ref()
            .and_then(|p| p.upgrade());
        match parent {
            Some(parent) => owner = parent,
            None => return Ok(h.keeping_from(&owner)),
        }
    }
    Err(Error::Limit)
}
pub fn definition(h: &Handle) -> Result<definitions::Handle> {
    h.read().definition.clone().ok_or(Error::Null)
}
pub fn size_x(h: &Handle) -> Result<i32> {
    let d = definition(h)?;
    let direction = h.read().direction.code();
    let d = d.read();
    Ok(if direction % 2 != 0 {
        d.size_y
    } else {
        d.size_x
    })
}
pub fn size_y(h: &Handle) -> Result<i32> {
    let d = definition(h)?;
    let direction = h.read().direction.code();
    let d = d.read();
    Ok(if direction % 2 != 0 {
        d.size_x
    } else {
        d.size_y
    })
}
pub fn center(h: &Handle) -> Result<Location> {
    let l = h.read().location.clone();
    let x = size_x(h)? >> 1;
    let y = size_y(h)? >> 1;
    let [a, b, z] = l.coordinates();
    Ok(Location::new(a.wrapping_add(x), b.wrapping_add(y), z))
}
pub fn child(
    registry: &mut Registry,
    ctx: &mut impl definitions::PlayerContext,
    h: &Handle,
    player: Option<usize>,
) -> Result<Handle> {
    if let Some(children) = children(h) {
        let d = registry.child(ctx, &definition(h)?, player)?;
        for i in 0..children.len() {
            let child = children.get(i).ok_or(Error::Null)?;
            if child.read().id == d.read().id {
                return Ok(child);
            }
        }
    }
    Ok(h.clone())
}
pub fn set_child_index(
    ctx: &mut impl Context,
    h: &Handle,
    player: usize,
    index: i32,
) -> Result<()> {
    let mut def = definition(h)?;
    if h.read().children.is_none() {
        let wrapper = super::doors::wrapper(h);
        if !wrapper.same(h) {
            def = definition(&wrapper)?;
        }
    }
    let file = def.read().config_file_id;
    if file > -1 {
        if ctx.varbit_exists(file)? {
            let file = def.read().config_file_id;
            ctx.set_varbit(player, file, index)?;
        }
    } else {
        let id = def.read().config_id;
        if id > -1 {
            ctx.set_varp(player, id, index)?;
        }
    }
    Ok(())
}
pub fn transform(
    registry: &mut Registry,
    h: &Handle,
    id: i32,
    rotation: Option<i32>,
    location: Option<Location>,
    shape: Option<i32>,
) -> Result<Handle> {
    let (l, s, r) = {
        let h = h.read();
        (h.location.clone(), h.shape, h.rotation)
    };
    construct(
        registry,
        id,
        location.unwrap_or(l),
        shape.unwrap_or(s),
        rotation.unwrap_or(r),
        false,
    )
}
pub fn as_constructed(registry: &mut Registry, h: &Handle) -> Result<Handle> {
    if h.read().constructed {
        return Ok(h.clone());
    }
    let (id, l, s, r) = {
        let h = h.read();
        (h.id, h.location.clone(), h.shape, h.rotation)
    };
    construct(registry, id, l, s, r, true)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Wall,
    WallDecor,
    Ground,
    GroundDecor,
}
pub fn layer(ctx: &mut impl Context, h: &Handle) -> Result<Option<Layer>> {
    let (shape, constructed) = {
        let h = h.read();
        (h.shape, h.constructed)
    };
    Ok(if shape <= 3 || shape == 9 {
        Some(Layer::Wall)
    } else if shape <= 8 {
        Some(Layer::WallDecor)
    } else if shape <= 21 {
        Some(Layer::Ground)
    } else if shape == 22 {
        Some(Layer::GroundDecor)
    } else {
        ctx.log_invalid_shape(shape, constructed)?;
        None
    })
}
pub fn occupied(h: &Handle) -> Result<Vec<Location>> {
    let mut size_x = size_x(h)?;
    let mut size_y = size_y(h)?;
    let (l, rotation) = {
        let h = h.read();
        (h.location.clone(), h.rotation)
    };
    let mut out = vec![l.clone()];
    if rotation % 2 == 1 {
        std::mem::swap(&mut size_x, &mut size_y);
    }
    if size_x > 65536 || size_y > 65536 {
        return Err(Error::Limit);
    }
    let [a, b, z] = l.coordinates();
    let negative = rotation >= 2;
    for i in 1..size_x {
        let offset = if negative { -i } else { i };
        out.push(Location::new(a.wrapping_add(offset), b, z));
    }
    for i in 1..size_y {
        let offset = if negative { -i } else { i };
        out.push(Location::new(a, b.wrapping_add(offset), z));
    }
    Ok(out)
}
pub fn attributes(h: &Handle) -> Shared<GameAttributes> {
    h.read().attributes.clone()
}
pub fn equals(a: &Handle, b: Option<&Handle>) -> bool {
    scenery::equals(a, b)
}
