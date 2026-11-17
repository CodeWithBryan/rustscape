//! Canonical queue/scene bindings for original entity-region kernels. Region
//! activity and zone callbacks remain controlled empty-content fixture services.
use super::{Host, Location, Node, State, player_walk};
use rs09_server::world::{
    chunk,
    entity_regions::{self as spatial, Context, Error, Kind, Result, Select},
    movement,
    player_walk::NodeHandle,
    region,
};
struct Spatial<'a> {
    state: &'a mut State,
    host: &'a mut Host,
}
fn error(e: impl std::fmt::Display) -> Error {
    Error::Binding(e.to_string())
}
impl Context for Spatial<'_> {
    fn chunk(&mut self, l: Location) -> Result<chunk::HandleChunk> {
        self.host.scene.chunk(&l).map_err(error)
    }
    fn region(&mut self, l: Location) -> Result<region::Handle> {
        let [x, y, _] = l.coordinates();
        Ok(self.host.scene.region(((x >> 6) << 8) | (y >> 6)))
    }
    fn kind(&mut self, h: &NodeHandle) -> Result<Kind> {
        Ok(match self.state.actor(h.node).map_err(error)?.view.kind {
            movement::Kind::Player => Kind::Player,
            movement::Kind::Npc => Kind::Npc,
            _ => Kind::Other,
        })
    }
    fn location(&mut self, h: &NodeHandle) -> Result<Option<Location>> {
        Ok(self
            .state
            .actor(h.node)
            .map_err(error)?
            .queue
            .entity()
            .location)
    }
    fn as_player(&mut self, h: &NodeHandle) -> Result<NodeHandle> {
        if self.kind(h)? != Kind::Player {
            return Err(error("not a player"));
        }
        Ok(h.clone())
    }
    fn name(&mut self, h: &NodeHandle) -> Result<String> {
        self.host
            .maps
            .names
            .get(&h.node)
            .cloned()
            .ok_or(Error::Null)
    }
    fn flag_active(&mut self, r: region::Handle) -> Result<()> {
        let id = r.read().id();
        if !self.host.active.contains(&id) {
            let d = r.read().data.clone();
            let (x, y) = {
                let d = d.read();
                (d.x, d.y)
            };
            self.host
                .activate(&Location::new(x << 6, y << 6, 0))
                .map_err(error)?;
        }
        Ok(())
    }
    fn check_inactive(&mut self, _: region::Handle) -> Result<()> {
        Ok(())
    }
    fn footprint(&mut self, h: &NodeHandle) -> Result<Option<Location>> {
        Ok(self.state.actor(h.node).map_err(error)?.queue.foot_print())
    }
    fn zone_update(&mut self, _: &NodeHandle, _: Option<Location>) -> Result<()> {
        Ok(())
    }
    fn unknown_entity(&mut self, _: &NodeHandle) -> Result<()> {
        Err(error("fixture has no other entity subclasses"))
    }
    fn now_millis(&mut self) -> Result<i64> {
        Ok(super::now())
    }
}
pub(super) fn move_entity(
    state: &mut State,
    host: &mut Host,
    n: Node,
    from: Option<Location>,
    to: Location,
) -> player_walk::Result<()> {
    let h = state.retain_node(n)?;
    spatial::move_entity(&mut Spatial { state, host }, &h, from, to)
        .map_err(|e| player_walk::Error::Binding(e.to_string()))
}
pub(super) fn local(
    state: &mut State,
    host: &mut Host,
    l: Location,
    d: i32,
    select: Select,
) -> Result<Vec<NodeHandle>> {
    spatial::local(&mut Spatial { state, host }, l, d, select)
}
pub(super) fn detach(state: &mut State, host: &mut Host, n: Node) -> player_walk::Result<()> {
    let h = state.retain_node(n)?;
    let location = state.actor(n)?.queue.entity().location;
    let mut ctx = Spatial { state, host };
    if let Some(l) = location {
        let c = ctx
            .chunk(l.clone())
            .map_err(|e| player_walk::Error::Binding(e.to_string()))?;
        match ctx
            .kind(&h)
            .map_err(|e| player_walk::Error::Binding(e.to_string()))?
        {
            Kind::Npc => spatial::remove_npc(&c, Some(h)),
            Kind::Player => {
                spatial::remove_player(&mut ctx, &c, Some(h.clone()))
                    .map_err(|e| player_walk::Error::Binding(e.to_string()))?;
                let r = ctx
                    .region(l)
                    .map_err(|e| player_walk::Error::Binding(e.to_string()))?;
                let map = r.read().tolerances.clone();
                let name = ctx
                    .name(&h)
                    .map_err(|e| player_walk::Error::Binding(e.to_string()))?;
                map.write().remove(&name);
            }
            Kind::Other => {}
        }
    }
    Ok(())
}
