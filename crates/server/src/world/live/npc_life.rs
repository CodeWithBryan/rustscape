//! `NPC.onRegionInactivity` in the live world: `world::npc_behaviour`'s
//! lifecycle body over the canonical NPC (walking queue reset, pulse manager
//! clear, the return-to-spawn branch with its chunk moves and arrival, the
//! renderable list), with the walk services as its host. The behaviour host
//! here answers only what that body reaches; every other member refuses by
//! name (the NPC's combat, aggression and content stay the start-up store's
//! refusals as well).
use super::walk::{Deferred, WalkHost};
use crate::world::{
    combat_swing::{self as cs, CombatStyle},
    doors::Location,
    entity_regions, movement, npc,
    npc_behaviour::{self as behaviour, Ai, Hitsplat, Kind, Rights, lifecycle},
    player_walk::{Node, State},
};

pub struct Lifecycle<'a, 'b> {
    pub state: &'a mut State,
    pub host: &'a mut WalkHost<'b>,
}

fn unbound<T>(what: &str) -> behaviour::Result<T> {
    Err(cs::Error::Binding(format!(
        "not ported in the live world: {what} (NPC behaviour host)"
    )))
}
fn binding(e: impl std::fmt::Debug) -> cs::Error {
    cs::Error::Binding(format!("{e:?}"))
}

/// Runs `onRegionInactivity` for one NPC.
pub fn on_region_inactivity(
    state: &mut State,
    host: &mut WalkHost<'_>,
    node: Node,
) -> npc::Result<()> {
    lifecycle::on_region_inactivity(&mut Lifecycle { state, host }, &node)
        .map_err(behaviour::tick::npc_error)
}

impl cs::Types for Lifecycle<'_, '_> {
    type Entity = Node;
    type Item = ();
    type Spell = ();
}

impl behaviour::Host for Lifecycle<'_, '_> {
    type Handler = ();
    type Region = i32;
    fn kind(&self, entity: &Node) -> Kind {
        if self.host.npcs.rows.contains_key(entity) {
            Kind::Npc
        } else if self.host.players.contains_key(entity) {
            Kind::Player
        } else {
            Kind::Other
        }
    }
    fn same(&self, a: &Node, b: &Node) -> bool {
        a == b
    }
    fn ai(&mut self, _: &Node) -> Option<&mut Ai<Self>> {
        None
    }
    fn brain(&self, npc: &Node) -> Option<npc::State> {
        self.host.npcs.rows.get(npc).map(|r| r.brain.clone())
    }
    fn location(&self, entity: &Node) -> Option<Location> {
        self.state.node_location(*entity).ok().flatten()
    }
    fn set_location(&mut self, npc: &Node, location: Option<Location>) {
        if let Ok(actor) = self.state.actor_mut(*npc) {
            actor.view.location = location.clone();
            actor.queue.state.write().entity.location = location;
        }
    }
    fn style_handler(&self, _: CombatStyle) {}
    fn region_dynamic(&self, _: &i32) -> bool {
        // No DynamicRegion is built in this world.
        false
    }
    fn max_players(&self) -> i32 {
        crate::world::repository::MAX_PLAYERS
    }
    fn ticks(&mut self) -> behaviour::Result<i32> {
        Ok(self.state.ticks)
    }
    fn current_time_millis(&mut self) -> behaviour::Result<i64> {
        Ok(super::now_millis())
    }
    fn next_int(&mut self, _: i32) -> behaviour::Result<i32> {
        unbound("RandomFunction.RANDOM")
    }
    fn math_random(&mut self) -> behaviour::Result<f64> {
        unbound("Math.random")
    }
    fn is_active(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("Node.isActive")
    }
    fn is_dead(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("DeathTask.isDead")
    }
    fn interaction_locked(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("locks")
    }
    fn attribute_bool(&mut self, _: &Node, _: &str, _: bool) -> behaviour::Result<bool> {
        unbound("NPC attributes")
    }
    fn attribute_long(&mut self, _: &Node, _: &str, _: i64) -> behaviour::Result<i64> {
        unbound("NPC attributes")
    }
    fn attribute_entity(&mut self, _: &Node, _: &str) -> behaviour::Result<Option<Node>> {
        unbound("NPC attributes")
    }
    fn set_attribute_entity(&mut self, _: &Node, _: &str, _: &Node) -> behaviour::Result<()> {
        unbound("NPC attributes")
    }
    fn remove_attribute(&mut self, _: &Node, _: &str) -> behaviour::Result<()> {
        unbound("NPC attributes")
    }
    fn config_bool(&mut self, _: &Node, _: &str, _: bool) -> behaviour::Result<bool> {
        unbound("NPC configuration")
    }
    fn config_int(&mut self, _: &Node, _: &str, _: i32) -> behaviour::Result<i32> {
        unbound("NPC configuration")
    }
    fn definition_combat_level(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("NPC definitions")
    }
    fn has_action(&mut self, _: &Node, _: &str) -> behaviour::Result<bool> {
        unbound("NPCDefinition.hasAction")
    }
    fn shown_npc(&mut self, _: &Node, _: &Node) -> behaviour::Result<Node> {
        unbound("getShownNPC")
    }
    fn is_attacking(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the combat pulse")
    }
    fn combat_victim(&mut self, _: &Node) -> behaviour::Result<Option<Node>> {
        unbound("the combat pulse")
    }
    fn combat_set_victim(&mut self, _: &Node, _: &Node) -> behaviour::Result<()> {
        unbound("the combat pulse")
    }
    fn combat_attack(&mut self, _: &Node, _: Option<&Node>) -> behaviour::Result<()> {
        unbound("the combat pulse")
    }
    fn combat_stop(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("the combat pulse")
    }
    fn next_attack(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("the combat pulse")
    }
    fn combat_style(&mut self, _: &Node) -> behaviour::Result<Option<CombatStyle>> {
        unbound("the combat pulse")
    }
    fn set_last_received_attack(&mut self, _: &Node, _: i32) -> behaviour::Result<()> {
        unbound("the combat state")
    }
    fn last_sent_attack(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("the combat state")
    }
    fn set_last_sent_attack(&mut self, _: &Node, _: i32) -> behaviour::Result<()> {
        unbound("the combat state")
    }
    fn face(&mut self, _: &Node, _: Option<&Node>) -> behaviour::Result<()> {
        unbound("Entity.face")
    }
    fn is_multi_zone(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("properties.isMultiZone")
    }
    fn current_combat_level(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("properties.getCurrentCombatLevel")
    }
    fn is_retaliating(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("properties.isRetaliating")
    }
    fn set_npc_walkable(&mut self, _: &Node, _: bool) -> behaviour::Result<()> {
        unbound("properties.setNPCWalkable")
    }
    fn set_combat_time_out(&mut self, _: &Node, _: i32) -> behaviour::Result<()> {
        unbound("properties.setCombatTimeOut")
    }
    fn local_players(&mut self, _: Location, _: i32) -> behaviour::Result<Vec<Node>> {
        unbound("RegionManager.getLocalPlayers")
    }
    fn wilderness_in_zone(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("WildernessZone")
    }
    fn region_for_id(&mut self, id: i32) -> behaviour::Result<i32> {
        // RegionManager.forId: the region is only asked for its class.
        Ok(id)
    }
    fn is_tolerated(&mut self, _: &i32, _: &Node) -> behaviour::Result<bool> {
        unbound("Region.isTolerated")
    }
    fn rights(&mut self, _: &Node) -> behaviour::Result<Option<Rights>> {
        unbound("player rights")
    }
    fn player_invisible(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("Player.isInvisible")
    }
    fn skull_wilderness(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the skull manager")
    }
    fn wilderness_protection(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the skull manager")
    }
    fn settings(&mut self) -> behaviour::Result<bool> {
        unbound("GameWorld.settings")
    }
    fn wild_pvp_enabled(&mut self) -> behaviour::Result<bool> {
        unbound("GameWorld.settings")
    }
    fn zone_continue_attack(
        &mut self,
        _: &Node,
        _: &Node,
        _: Option<CombatStyle>,
        _: bool,
    ) -> behaviour::Result<bool> {
        unbound("zones")
    }
    fn zone_is_pvpable(
        &mut self,
        _: &Node,
        _: &Node,
        _: &Node,
        _: Option<CombatStyle>,
        _: bool,
    ) -> behaviour::Result<bool> {
        unbound("zones")
    }
    fn level(&mut self, _: &Node, _: i32) -> behaviour::Result<i32> {
        unbound("skills")
    }
    fn familiar_owner(&mut self, _: &Node) -> behaviour::Result<Option<Node>> {
        unbound("familiars")
    }
    fn send_message(&mut self, _: &Node, _: &str) -> behaviour::Result<()> {
        unbound("messages")
    }
    fn path_find(&mut self, _: Option<&Node>, _: Option<Location>) -> behaviour::Result<()> {
        unbound("pathfinding")
    }
    fn path_walk(&mut self, _: Option<&Node>) -> behaviour::Result<()> {
        unbound("pathfinding")
    }
    fn move_step(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("movement")
    }
    fn walking_has_path(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the walking queue")
    }
    fn pulse_manager_is_moving(&mut self, _: &Node) -> behaviour::Result<bool> {
        unbound("the pulse manager")
    }
    fn ring_id(&mut self, _: &Node) -> behaviour::Result<i32> {
        unbound("equipment")
    }
    fn send_chat(&mut self, _: &Node, _: &str) -> behaviour::Result<()> {
        unbound("chat")
    }
    fn manual_hit(&mut self, _: &Node, _: &Node, _: i32, _: Hitsplat) -> behaviour::Result<()> {
        unbound("impacts")
    }
    fn handle_recoil_effect(
        &mut self,
        _: &Node,
        _: Option<&Node>,
        _: i32,
    ) -> behaviour::Result<()> {
        unbound("recoil")
    }
    fn check_armour_degrades(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("degradation")
    }
    fn walking_reset(&mut self, npc: &Node) -> behaviour::Result<()> {
        // getWalkingQueue().reset().
        let mut world = self.state.context(&mut *self.host);
        movement::Context::walk_reset(&mut world, None, *npc, None).map_err(binding)
    }
    fn pulse_manager_clear(&mut self, npc: &Node) -> behaviour::Result<()> {
        self.state
            .context(&mut *self.host)
            .clear_pulses(*npc)
            .map_err(binding)
    }
    fn masks_reset(&mut self, _: &Node) -> behaviour::Result<()> {
        self.host
            .not_ported
            .skip("NPC update masks reset (NPC masks not joined)");
        Ok(())
    }
    fn chunk_remove(&mut self, npc: &Node, location: Location) -> behaviour::Result<()> {
        let chunk = self
            .host
            .walk
            .scene
            .chunk(&location)
            .map_err(|e| cs::Error::Binding(e.0))?;
        let owner = self.state.retain_node(*npc).map_err(binding)?;
        entity_regions::remove_npc(&chunk, Some(owner));
        Ok(())
    }
    fn chunk_add(&mut self, npc: &Node, location: Location) -> behaviour::Result<()> {
        let chunk = self
            .host
            .walk
            .scene
            .chunk(&location)
            .map_err(|e| cs::Error::Binding(e.0))?;
        let owner = self.state.retain_node(*npc).map_err(binding)?;
        entity_regions::add_npc(&chunk, Some(owner));
        Ok(())
    }
    fn return_arrival(&mut self, npc: &Node) -> behaviour::Result<bool> {
        crate::world::npc_walk::arrive(self.state, &mut *self.host, *npc).map_err(binding)
    }
    fn pulse_stop(&mut self, _: &Node, pulse: usize) -> behaviour::Result<()> {
        let base = self.state.movement(pulse).map_err(binding)?.base();
        crate::pulses::stop_base(&base);
        Ok(())
    }
    fn remove_renderable(&mut self, npc: &Node) -> behaviour::Result<()> {
        // Repository.removeRenderableNPC: the repository is the live world's;
        // it runs right after this NPC's tick returns.
        let owner = self.state.retain_node(*npc).map_err(binding)?;
        self.host.deferred.push(Deferred::RemoveRenderable(owner));
        Ok(())
    }
    fn npc_clear(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("NPC.clear (dynamic regions are not built)")
    }
    fn skills_restore(&mut self, _: &Node) -> behaviour::Result<()> {
        unbound("skills")
    }
    fn remove_timer(&mut self, _: &Node, _: &str) -> behaviour::Result<()> {
        unbound("timers")
    }
    fn clipping_flag(&mut self, _: i32, _: i32, _: i32) -> behaviour::Result<i32> {
        unbound("clipping")
    }
    fn other_clipping_flag(&mut self, _: u32, _: i32, _: i32, _: i32) -> behaviour::Result<i32> {
        unbound("clipping")
    }
}
