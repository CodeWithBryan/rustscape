//! Player varp/save-flag stores and unchanged ContentAPI variable algorithms.
//! Value maps contain primitives only; exposed raw maps do not retain players.
use crate::{
    definitions::{item::Shared, varbit},
    world::player_walk::NodeHandle,
};
use rs09_game_core::{java_json::Json, java_map::int_key_layout, java_string::parse_i32};
use std::collections::HashMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Number,
    ConcurrentModification,
    Definition(varbit::Error),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub struct MapFields<T> {
    values: HashMap<i32, Option<T>>,
    insertion: Vec<i32>,
    capacity: usize,
    modifications: u32,
}
#[derive(Clone)]
pub struct Map<T>(Shared<MapFields<T>>);
impl<T> Default for Map<T> {
    fn default() -> Self {
        Self(Shared::new(MapFields {
            values: HashMap::new(),
            insertion: Vec::new(),
            capacity: 16,
            modifications: 0,
        }))
    }
}
impl<T: Copy> Map<T> {
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn get(&self, id: i32) -> Option<T> {
        self.0.read().values.get(&id).copied().flatten()
    }
    pub fn contains_key(&self, id: i32) -> bool {
        self.0.read().values.contains_key(&id)
    }
    pub fn put(&self, id: i32, value: Option<T>) -> Option<Option<T>> {
        let mut f = self.0.write();
        if !f.values.contains_key(&id) {
            f.modifications = f.modifications.wrapping_add(1);
            f.insertion.push(id);
        }
        f.values.insert(id, value)
    }
    pub fn clear(&self) {
        let mut f = self.0.write();
        let (_, capacity) = int_key_layout(f.insertion.iter().copied(), f.capacity);
        f.capacity = capacity;
        f.values.clear();
        f.insertion.clear();
        f.modifications = f.modifications.wrapping_add(1);
    }
    pub fn snapshot(&self) -> HashMap<i32, Option<T>> {
        self.0.read().values.clone()
    }
    /// The keys in the order the original HashMap iterates them.
    pub fn keys(&self) -> Vec<i32> {
        self.iteration().0
    }
    fn iteration(&self) -> (Vec<i32>, u32) {
        let f = self.0.read();
        (
            int_key_layout(f.insertion.iter().copied(), f.capacity).0,
            f.modifications,
        )
    }
    fn modifications(&self) -> u32 {
        self.0.read().modifications
    }
}
pub type Values = Map<i32>;
pub type Saved = Map<bool>;
impl Saved {
    pub fn remove(&self, id: i32) -> Option<Option<bool>> {
        let mut f = self.0.write();
        let old = f.values.remove(&id);
        if old.is_some() {
            f.insertion.retain(|v| *v != id);
            f.modifications = f.modifications.wrapping_add(1);
        }
        old
    }
}
pub struct Fields {
    pub values: Option<Values>,
    pub saved: Option<Saved>,
}
#[derive(Clone)]
pub struct Store(pub Shared<Fields>);
impl Default for Store {
    fn default() -> Self {
        Self(Shared::new(Fields {
            values: Some(Values::default()),
            saved: Some(Saved::default()),
        }))
    }
}
pub trait Context {
    type Dispatch;
    fn values(&mut self, player: &NodeHandle) -> Result<Option<Values>>;
    fn saved(&mut self, player: &NodeHandle) -> Result<Option<Saved>>;
    fn dispatch(&mut self, player: &NodeHandle) -> Result<Self::Dispatch>;
    fn send_varp(&mut self, dispatch: Self::Dispatch, id: i32, value: i32) -> Result<()>;
    fn send_varc(&mut self, dispatch: Self::Dispatch, id: i16, value: i32) -> Result<()>;
    fn definition(&mut self, id: i32) -> Result<Option<varbit::Handle>>;
    fn missing_definition(&mut self, id: i32) -> Result<()>;
}
pub fn get_varp(ctx: &mut impl Context, player: Option<&NodeHandle>, id: i32) -> Result<i32> {
    Ok(ctx
        .values(player.ok_or(Error::Null)?)?
        .ok_or(Error::Null)?
        .get(id)
        .unwrap_or(0))
}
pub fn get_varbit_definition(
    ctx: &mut impl Context,
    player: Option<&NodeHandle>,
    definition: Option<&varbit::Handle>,
) -> Result<i32> {
    let player = player.ok_or(Error::Null)?;
    let definition = definition.ok_or(Error::Null)?;
    let mask = definition.read().mask().map_err(Error::Definition)?;
    let varp = definition.read().varp_id;
    let current = get_varp(ctx, Some(player), varp)?;
    Ok(current.wrapping_shr(definition.read().start_bit as u32) & mask)
}
pub fn get_varbit(ctx: &mut impl Context, player: Option<&NodeHandle>, id: i32) -> Result<i32> {
    let player = player.ok_or(Error::Null)?;
    let definition = ctx.definition(id)?;
    get_varbit_definition(ctx, Some(player), definition.as_ref())
}
/// VarbitDefinition.getValue re-resolves the current mapping by its final ID.
pub fn definition_value(
    ctx: &mut impl Context,
    player: Option<&NodeHandle>,
    definition: Option<&varbit::Handle>,
) -> Result<i32> {
    let id = definition.ok_or(Error::Null)?.read().id;
    get_varbit(ctx, player, id)
}
pub fn set_varp(
    ctx: &mut impl Context,
    player: Option<&NodeHandle>,
    id: i32,
    value: i32,
    save: bool,
) -> Result<()> {
    let player = player.ok_or(Error::Null)?;
    ctx.values(player)?.ok_or(Error::Null)?.put(id, Some(value));
    let saved = ctx.saved(player)?.ok_or(Error::Null)?;
    if saved.get(id) != Some(true) && save {
        ctx.saved(player)?.ok_or(Error::Null)?.put(id, Some(true));
    }
    let dispatch = ctx.dispatch(player)?;
    ctx.send_varp(dispatch, id, value)
}
pub fn save_varp(ctx: &mut impl Context, player: Option<&NodeHandle>, id: i32) -> Result<()> {
    ctx.saved(player.ok_or(Error::Null)?)?
        .ok_or(Error::Null)?
        .put(id, Some(true));
    Ok(())
}
pub fn unsave_varp(ctx: &mut impl Context, player: Option<&NodeHandle>, id: i32) -> Result<()> {
    ctx.saved(player.ok_or(Error::Null)?)?
        .ok_or(Error::Null)?
        .remove(id);
    Ok(())
}
pub fn set_varbit_definition(
    ctx: &mut impl Context,
    player: Option<&NodeHandle>,
    definition: Option<&varbit::Handle>,
    value: i32,
    save: bool,
) -> Result<()> {
    let player = player.ok_or(Error::Null)?;
    let definition = definition.ok_or(Error::Null)?;
    let mask = definition.read().mask().map_err(Error::Definition)?;
    let varp = definition.read().varp_id;
    let current = get_varp(ctx, Some(player), varp)?;
    let current = current & !mask.wrapping_shl(definition.read().start_bit as u32);
    let new = (value & mask).wrapping_shl(definition.read().start_bit as u32);
    let varp = definition.read().varp_id;
    set_varp(ctx, Some(player), varp, current | new, save)
}
pub fn set_varbit(
    ctx: &mut impl Context,
    player: Option<&NodeHandle>,
    id: i32,
    value: i32,
    save: bool,
) -> Result<()> {
    let player = player.ok_or(Error::Null)?;
    let definition = ctx.definition(id)?;
    if definition.is_none() {
        return ctx.missing_definition(id);
    }
    set_varbit_definition(ctx, Some(player), definition.as_ref(), value, save)
}
pub fn set_varc(
    ctx: &mut impl Context,
    player: Option<&NodeHandle>,
    id: i32,
    value: i32,
) -> Result<()> {
    let dispatch = ctx.dispatch(player.ok_or(Error::Null)?)?;
    ctx.send_varc(dispatch, id as i16, value)
}
pub fn reinit(ctx: &mut impl Context, player: Option<&NodeHandle>) -> Result<()> {
    let player = player.ok_or(Error::Null)?;
    let values = ctx.values(player)?.ok_or(Error::Null)?;
    let (keys, expected) = values.iteration();
    for key in keys {
        if values.modifications() != expected {
            return Err(Error::ConcurrentModification);
        }
        let value = values.get(key).ok_or(Error::Null)?;
        let save = ctx
            .saved(player)?
            .ok_or(Error::Null)?
            .get(key)
            .unwrap_or(false);
        set_varp(ctx, Some(player), key, value, save)?;
    }
    Ok(())
}
struct ManagerFields {
    player: NodeHandle,
}
#[derive(Clone)]
pub struct Manager(Shared<ManagerFields>);
impl Manager {
    pub fn new(player: Option<NodeHandle>) -> Result<Self> {
        Ok(Self(Shared::new(ManagerFields {
            player: player.ok_or(Error::Null)?,
        })))
    }
    pub fn owner(&self) -> NodeHandle {
        self.0.read().player.clone()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub(crate) fn ownership(&self) -> (usize, usize, Option<NodeHandle>) {
        (self.0.identity(), self.0.strong_count(), Some(self.owner()))
    }
    pub fn save(&self, root: Option<&mut Json>) -> Result<()> {
        match root.ok_or(Error::Null)? {
            Json::Object(_) => Ok(()),
            Json::Null => Err(Error::Null),
            _ => Err(Error::Type),
        }
    }
    pub fn parse(&self, data: Option<&Json>, ctx: &mut impl Context) -> Result<()> {
        let data = data.ok_or(Error::Null)?;
        let Json::Array(rows) = data else {
            return Err(if matches!(data, Json::Null) {
                Error::Null
            } else {
                Error::Type
            });
        };
        for row in rows {
            if !matches!(row, Json::Object(_)) {
                return Err(if matches!(row, Json::Null) {
                    Error::Null
                } else {
                    Error::Type
                });
            }
            let index =
                parse_i32(&row.get("index").unwrap_or(&Json::Null).text()).ok_or(Error::Number)?;
            let bits = row.get("bitArray").unwrap_or(&Json::Null);
            let Json::Array(bits) = bits else {
                return Err(if matches!(bits, Json::Null) {
                    Error::Null
                } else {
                    Error::Type
                });
            };
            let mut total = 0;
            for bit in bits {
                if !matches!(bit, Json::Object(_)) {
                    return Err(if matches!(bit, Json::Null) {
                        Error::Null
                    } else {
                        Error::Type
                    });
                }
                let offset = parse_i32(&bit.get("offset").unwrap_or(&Json::Null).text())
                    .ok_or(Error::Number)?;
                let value = parse_i32(&bit.get("value").unwrap_or(&Json::Null).text())
                    .ok_or(Error::Number)?;
                total |= value.wrapping_shl(offset as u32);
            }
            ctx.values(&self.owner())?
                .ok_or(Error::Null)?
                .put(index, Some(total));
            ctx.saved(&self.owner())?
                .ok_or(Error::Null)?
                .put(index, Some(true));
        }
        Ok(())
    }
}
