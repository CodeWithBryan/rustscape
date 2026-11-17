//! RegionManager entity movement/local queries and RegionChunk entity-list
//! operations. Owning worlds bind chunk/region lookup, activity and zone services.
use super::{chunk::HandleChunk, doors::Location, player_walk::NodeHandle, region, update};
#[derive(Debug)]
pub enum Error {
    Null,
    Limit,
    ConcurrentModification,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
impl From<update::Error> for Error {
    fn from(e: update::Error) -> Self {
        match e {
            update::Error::ConcurrentModification => Self::ConcurrentModification,
            update::Error::Null => Self::Null,
            _ => Self::Binding(e.to_string()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Npc,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    All,
    Players,
    Npcs,
}
pub trait Context {
    fn chunk(&mut self, location: Location) -> Result<HandleChunk>;
    fn region(&mut self, location: Location) -> Result<region::Handle>;
    fn kind(&mut self, node: &NodeHandle) -> Result<Kind>;
    fn location(&mut self, node: &NodeHandle) -> Result<Option<Location>>;
    fn as_player(&mut self, node: &NodeHandle) -> Result<NodeHandle>;
    fn name(&mut self, node: &NodeHandle) -> Result<String>;
    fn flag_active(&mut self, region: region::Handle) -> Result<()>;
    fn check_inactive(&mut self, region: region::Handle) -> Result<()>;
    fn footprint(&mut self, node: &NodeHandle) -> Result<Option<Location>>;
    fn zone_update(&mut self, node: &NodeHandle, location: Option<Location>) -> Result<()>;
    fn unknown_entity(&mut self, node: &NodeHandle) -> Result<()>;
    fn now_millis(&mut self) -> Result<i64>;
}
pub fn add_npc(chunk: &HandleChunk, node: Option<NodeHandle>) {
    chunk.read().npcs.add(node);
}
pub fn remove_npc(chunk: &HandleChunk, node: Option<NodeHandle>) {
    chunk.read().npcs.remove(&node);
}
pub fn add_player(
    ctx: &mut impl Context,
    chunk: &HandleChunk,
    node: Option<NodeHandle>,
) -> Result<()> {
    let list = chunk.read().players.clone();
    list.add(node);
    let base = chunk.read().current_base.clone();
    let region = ctx.region(base)?;
    ctx.flag_active(region)
}
pub fn remove_player(
    ctx: &mut impl Context,
    chunk: &HandleChunk,
    node: Option<NodeHandle>,
) -> Result<()> {
    let list = chunk.read().players.clone();
    list.remove(&node);
    let base = chunk.read().current_base.clone();
    let region = ctx.region(base)?;
    ctx.check_inactive(region)
}
pub fn move_entity(
    ctx: &mut impl Context,
    node: &NodeHandle,
    src: Option<Location>,
    dst: Location,
) -> Result<()> {
    let from = src.as_ref().map(|l| ctx.chunk(l.clone())).transpose()?;
    let to = ctx.chunk(dst.clone())?;
    let changed = from.as_ref().is_none_or(|c| !c.same(&to));
    drop((from, to));
    if changed {
        match ctx.kind(node)? {
            Kind::Player => {
                let player = ctx.as_player(node)?;
                let a = src.as_ref().map(|l| ctx.region(l.clone())).transpose()?;
                let b = ctx.region(dst.clone())?;
                let same_region = a.as_ref().is_some_and(|a| a.same(&b));
                drop((a, b));
                if let Some(source) = src.as_ref().filter(|_| same_region) {
                    let c = ctx.chunk(source.clone())?;
                    remove_player(ctx, &c, Some(player.clone()))?;
                    let c = ctx.chunk(dst.clone())?;
                    add_player(ctx, &c, Some(player))?;
                } else {
                    if let Some(source) = src {
                        let c = ctx.chunk(source.clone())?;
                        remove_player(ctx, &c, Some(player.clone()))?;
                        let r = ctx.region(source)?;
                        let map = r.read().tolerances.clone();
                        let name = ctx.name(&player)?;
                        map.write().remove(&name);
                    }
                    let c = ctx.chunk(dst.clone())?;
                    add_player(ctx, &c, Some(node.clone()))?;
                    let r = ctx.region(dst)?;
                    let map = r.read().tolerances.clone();
                    let player = ctx.as_player(node)?;
                    let name = ctx.name(&player)?;
                    let now = ctx.now_millis()?;
                    map.write().insert(name, Some(now));
                }
            }
            Kind::Npc => {
                if let Some(src) = src {
                    let c = ctx.chunk(src)?;
                    remove_npc(&c, Some(node.clone()));
                }
                let c = ctx.chunk(dst)?;
                add_npc(&c, Some(node.clone()));
            }
            Kind::Other => ctx.unknown_entity(node)?,
        }
    }
    let footprint = ctx.footprint(node)?;
    ctx.zone_update(node, footprint)
}
/// A new result list retains the same entity identities. Chunk scan order is
/// x then y, and mixed queries append players before NPCs within each chunk.
pub fn local(
    ctx: &mut impl Context,
    location: Location,
    distance: i32,
    select: Select,
) -> Result<Vec<NodeHandle>> {
    let [x, y, z] = location.coordinates();
    let radius = (distance / 8).wrapping_add(1);
    let minx = (x >> 3).wrapping_sub(radius);
    let maxx = (x >> 3).wrapping_add(radius);
    let miny = (y >> 3).wrapping_sub(radius);
    let maxy = (y >> 3).wrapping_add(radius);
    if minx > maxx || miny > maxy {
        return Ok(vec![]);
    }
    if (i64::from(maxx) - i64::from(minx) + 1).saturating_mul(i64::from(maxy) - i64::from(miny) + 1)
        > 65_536
    {
        return Err(Error::Limit);
    }
    let mut entities = Vec::new();
    for cx in minx..=maxx {
        for cy in miny..=maxy {
            let c = ctx.chunk(Location::new(cx.wrapping_shl(3), cy.wrapping_shl(3), z))?;
            if select != Select::Npcs {
                let list = c.read().players.clone();
                let mut err = None;
                list.each(|n| {
                    if let Some(n) = n {
                        match ctx.kind(&n) {
                            Ok(kind) if select == Select::All || kind == Kind::Player => {
                                entities.push(n)
                            }
                            Ok(_) => {}
                            Err(e) => {
                                err = Some(e);
                                return Err(update::Error::Binding("entity class lookup".into()));
                            }
                        }
                    }
                    Ok(())
                })
                .map_err(|e| err.unwrap_or_else(|| e.into()))?;
            }
            if select != Select::Players {
                let list = c.read().npcs.clone();
                let mut err = None;
                list.each(|n| {
                    if let Some(n) = n {
                        match ctx.kind(&n) {
                            Ok(kind) if select == Select::All || kind == Kind::Npc => {
                                entities.push(n)
                            }
                            Ok(_) => {}
                            Err(e) => {
                                err = Some(e);
                                return Err(update::Error::Binding("entity class lookup".into()));
                            }
                        }
                    }
                    Ok(())
                })
                .map_err(|e| err.unwrap_or_else(|| e.into()))?;
            }
        }
    }
    let (x1, x2) = (x.wrapping_sub(distance), x.wrapping_add(distance));
    let (y1, y2) = (y.wrapping_sub(distance), y.wrapping_add(distance));
    let mut result = Vec::new();
    for node in &entities {
        let nx = ctx.location(node)?.ok_or(Error::Null)?.coordinates()[0];
        let ny = ctx.location(node)?.ok_or(Error::Null)?.coordinates()[1];
        if x1.min(x2) <= nx && nx <= x1.max(x2) && y1.min(y2) <= ny && ny <= y1.max(y2) {
            result.push(node.clone());
        }
    }
    Ok(result)
}
pub fn is_tolerated(
    ctx: &mut impl Context,
    region: &region::Handle,
    player: &NodeHandle,
) -> Result<bool> {
    let now = ctx.now_millis()?;
    let map = region.read().tolerances.clone();
    let name = ctx.name(player)?;
    let fallback = ctx.now_millis()?;
    let previous = map
        .read()
        .get(&name)
        .copied()
        .unwrap_or(Some(fallback))
        .ok_or(Error::Null)?;
    Ok(now.wrapping_sub(previous) > 600_000)
}
