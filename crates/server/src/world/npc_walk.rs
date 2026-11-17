//! NPC.tick connected to the same canonical entity queues, navigation and world
//! pulse runner as players. Owners retain node leases and provide content,
//! region/lock/definition/dialogue/RNG services; no graphics or JVM is involved.
use super::{
    doors::Location,
    movement, npc, pathfinding,
    player_walk::{self, Node, State},
};
pub trait Services: player_walk::Services {
    fn brain(&self, node: Node) -> npc::Result<npc::State>;
    fn npc_region_active(
        &mut self,
        state: &mut State,
        node: Node,
        location: Location,
    ) -> npc::Result<bool>;
    fn npc_query(&mut self, state: &mut State, node: Node, query: npc::Query) -> npc::Result<bool>;
    fn npc_event(&mut self, state: &mut State, node: Node, event: npc::Event) -> npc::Result<()>;
    fn dialogue_active(&mut self, state: &mut State, player: usize) -> npc::Result<bool>;
    fn dialogue_chatbox(&mut self, state: &mut State, player: usize) -> npc::Result<bool>;
    fn random_bool(&mut self) -> npc::Result<bool>;
    fn randomize(&mut self, bound: i32) -> npc::Result<i32>;
    fn random_range(&mut self, min: i32, max: i32) -> npc::Result<i32>;
    fn npc_can_move(
        &mut self,
        state: &mut State,
        node: Node,
        location: Option<Location>,
    ) -> npc::Result<bool>;
    fn npc_water_path(
        &mut self,
        _: &mut State,
        _: Node,
        _: Location,
    ) -> npc::Result<pathfinding::Path> {
        Err(npc::Error::Binding("water clipping is not bound".into()))
    }
}
fn error(e: player_walk::Error) -> npc::Error {
    npc::Error::Binding(e.to_string())
}
pub fn tick(state: &mut State, services: &mut impl Services, node: Node) -> npc::Result<()> {
    let brain = services.brain(node)?;
    brain.tick(&mut Bridge {
        state,
        services,
        node,
    })
}
pub fn arrive(state: &mut State, services: &mut impl Services, node: Node) -> npc::Result<bool> {
    let brain = services.brain(node)?;
    brain.return_arrival(&mut Bridge {
        state,
        services,
        node,
    })
}
struct Bridge<'a, S> {
    state: &'a mut State,
    services: &'a mut S,
    node: Node,
}
impl<S: Services> npc::Context for Bridge<'_, S> {
    fn location(&self, _: &npc::State) -> Option<Location> {
        self.state
            .actor(self.node)
            .ok()
            .and_then(|a| a.queue.entity().location)
    }
    fn ticks(&mut self, _: &npc::State) -> npc::Result<i32> {
        Ok(self.state.ticks)
    }
    fn region_active(&mut self, _: &npc::State, location: Location) -> npc::Result<bool> {
        self.services
            .npc_region_active(self.state, self.node, location)
    }
    fn query(&mut self, _: &npc::State, query: npc::Query) -> npc::Result<bool> {
        if query == npc::Query::PulseRunning {
            return self
                .state
                .context(self.services)
                .manager_running(self.node)
                .map_err(error);
        }
        self.services.npc_query(self.state, self.node, query)
    }
    fn event(&mut self, brain: &npc::State, event: npc::Event) -> npc::Result<()> {
        match event {
            npc::Event::EntityTick => self
                .state
                .context(self.services)
                .entity(self.node)
                .map_err(error),
            npc::Event::Teleport(location) => {
                self.state
                    .actor(self.node)
                    .map_err(error)?
                    .queue
                    .state
                    .write()
                    .entity
                    .teleport_location = location;
                Ok(())
            }
            npc::Event::SubmitReturn(id) => {
                self.state.context(self.services).submit(id).map_err(error)
            }
            npc::Event::Attribute("return-to-spawn-pulse" | "remove:return-to-spawn-pulse") => {
                let id = brain.fields.read().return_pulse;
                self.state.attribute_motion(self.node, id).map_err(error)?;
                self.services.npc_event(self.state, self.node, event)
            }
            _ => self.services.npc_event(self.state, self.node, event),
        }
    }
    fn pulse_running(&mut self, _: &npc::State, id: usize) -> npc::Result<bool> {
        Ok(crate::pulses::is_running_base(
            &self.state.movement(id).map_err(error)?.base(),
        ))
    }
    fn return_pulse(&mut self, _: &npc::State, location: Option<Location>) -> npc::Result<usize> {
        self.state
            .context(self.services)
            .return_to_spawn(self.node, location)
            .map_err(error)
    }
    fn dialogue_active(&mut self, _: &npc::State, player: usize) -> npc::Result<bool> {
        self.services.dialogue_active(self.state, player)
    }
    fn dialogue_chatbox(&mut self, _: &npc::State, player: usize) -> npc::Result<bool> {
        self.services.dialogue_chatbox(self.state, player)
    }
    fn random_bool(&mut self, _: &npc::State) -> npc::Result<bool> {
        self.services.random_bool()
    }
    fn randomize(&mut self, _: &npc::State, bound: i32) -> npc::Result<i32> {
        self.services.randomize(bound)
    }
    fn random_range(&mut self, _: &npc::State, min: i32, max: i32) -> npc::Result<i32> {
        self.services.random_range(min, max)
    }
    fn can_move(&mut self, _: &npc::State, location: Option<Location>) -> npc::Result<bool> {
        self.services.npc_can_move(self.state, self.node, location)
    }
    fn walk(&mut self, _: &npc::State, location: Option<Location>, water: bool) -> npc::Result<()> {
        let location = location.ok_or(npc::Error::Null)?;
        let path = if water {
            self.services
                .npc_water_path(self.state, self.node, location)?
        } else {
            self.state
                .context(self.services)
                .find_path(self.node, location, movement::DUMB, false)
                .map_err(error)?
        };
        self.state
            .context(self.services)
            .walk_path(self.node, path)
            .map_err(error)
    }
}
