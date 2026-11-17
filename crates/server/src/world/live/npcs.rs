//! NPCs in the live world: the start-up's NPCs joined to the live
//! repository (same slots and indices) and to the scene's chunks, `NPC.tick`
//! through `world::npc_walk` (the `world::npc` tick kernel on the canonical
//! actor's walking queue, navigation and pulse runner), `NPC.reset`, and
//! `NPCRenderer.render` (`world::npc_render`) with entity-region candidates.
//!
//! The NPC's behaviour hooks answer through `world::npc_behaviour` with the
//! start-up's NPC store as its host (the registry and AI the start-up made);
//! what has no port (aggression, combat, NPC update masks, NPC attributes,
//! force talk) is a named answer in [`super::not_ported`].
use super::{P, host::Live, walk::WalkHost};
use crate::{
    definitions::property::Property,
    world::{
        action_locks,
        doors::Location,
        entity_regions::{self, Select},
        npc, npc_behaviour, npc_render, npc_walk, player_render,
        player_walk::{Node, State},
        region_activity,
    },
};
use rs09_protocol::game::Frame;

fn nerror(e: impl std::fmt::Debug) -> npc::Error {
    npc::Error::Binding(format!("{e:?}"))
}

fn locked(state: &mut State, node: Node, kind: action_locks::Kind) -> npc::Result<bool> {
    let owner = state.retain_node(node).map_err(nerror)?;
    let locks = state.locks(&owner).map_err(nerror)?;
    locks
        .is_kind_locked(kind, state, &mut action_locks::Clock)
        .map_err(nerror)
}

impl npc_walk::Services for WalkHost<'_> {
    fn brain(&self, node: Node) -> npc::Result<npc::State> {
        self.npcs
            .rows
            .get(&node)
            .map(|row| row.brain.clone())
            .ok_or(npc::Error::Null)
    }
    fn npc_region_active(
        &mut self,
        _: &mut State,
        _: Node,
        location: Location,
    ) -> npc::Result<bool> {
        // `getLocation().getRegion().isActive()`.
        let region = self.walk.scene.region(super::walk::region_id(&location));
        Ok(region_activity::is_active(&region))
    }
    fn npc_query(&mut self, state: &mut State, node: Node, query: npc::Query) -> npc::Result<bool> {
        Ok(match query {
            npc::Query::Behavior => {
                // behavior.tick(this) through the start-up's registry.
                npc_behaviour::tick::query(&mut *self.npcs, &node, query)
                    .map_err(npc_behaviour::tick::npc_error)?
                    .ok_or(npc::Error::Null)?
            }
            npc::Query::InteractionLocked => locked(state, node, action_locks::Kind::Interaction)?,
            npc::Query::MovementLocked => locked(state, node, action_locks::Kind::Movement)?,
            npc::Query::Attacking => {
                self.not_ported.default_answer(
                    "CombatPulse.isAttacking for NPCs (combat pulse not installed: false)",
                );
                false
            }
            npc::Query::InCombat => {
                self.not_ported.default_answer(
                    "CombatPulse.isInCombat for NPCs (combat pulse not installed: false)",
                );
                false
            }
            npc::Query::SelectTarget => {
                self.not_ported.skip(
                    "AggressiveHandler.selectTarget (NPC aggression not installed: no target)",
                );
                false
            }
            npc::Query::WaterNpc => {
                let row = self.npcs.rows.get(&node).ok_or(npc::Error::Null)?;
                let handlers = row.definition.read().handlers.clone();
                let value = handlers.read().get("water_npc").cloned();
                match value {
                    None | Some(Property::Null) => false,
                    Some(Property::Bool(v)) => v,
                    Some(_) => {
                        return Err(npc::Error::Binding("water_npc is not a boolean".into()));
                    }
                }
            }
            npc::Query::PulseRunning => {
                return Err(npc::Error::Binding(
                    "PulseRunning is answered by npc_walk".into(),
                ));
            }
        })
    }
    fn npc_event(&mut self, state: &mut State, node: Node, event: npc::Event) -> npc::Result<()> {
        match event {
            npc::Event::LockMovement(ticks) => {
                let owner = state.retain_node(node).map_err(nerror)?;
                let locks = state.locks(&owner).map_err(nerror)?;
                locks
                    .lock_kind(
                        action_locks::Kind::Movement,
                        ticks,
                        state,
                        &mut action_locks::Clock,
                    )
                    .map_err(nerror)
            }
            npc::Event::UnlockMovement => {
                let owner = state.retain_node(node).map_err(nerror)?;
                let locks = state.locks(&owner).map_err(nerror)?;
                locks
                    .unlock_kind(action_locks::Kind::Movement)
                    .map_err(nerror)
            }
            npc::Event::Inactivity => super::npc_life::on_region_inactivity(state, self, node),
            npc::Event::BehaviorRespawn
            | npc::Event::Respawn
            | npc::Event::Restore
            | npc::Event::PauseAggression(_) => {
                // Through npc_behaviour with the start-up's NPC store.
                let owned = npc_behaviour::tick::event(&mut *self.npcs, &node, &event)
                    .map_err(npc_behaviour::tick::npc_error)?;
                if owned {
                    Ok(())
                } else {
                    Err(npc::Error::Binding(format!(
                        "{event:?} is not npc_behaviour's"
                    )))
                }
            }
            npc::Event::ImpactDisabled(_) => {
                self.not_ported
                    .skip("ImpactHandler.setDisabledTicks for NPCs (combat not installed)");
                Ok(())
            }
            npc::Event::StopCombat => {
                self.not_ported
                    .skip("CombatPulse.stop for NPCs (combat not installed)");
                Ok(())
            }
            npc::Event::Attribute(_) => {
                self.not_ported
                    .skip("NPC attributes (return-to-spawn, lastForceTalk: not stored)");
                Ok(())
            }
            npc::Event::Chat(_) => {
                self.not_ported
                    .skip("NPC.sendChat (NPC update masks not joined: force talk not shown)");
                Ok(())
            }
            other => Err(npc::Error::Binding(format!(
                "not ported in the live world: NPC event {other:?}"
            ))),
        }
    }
    fn dialogue_active(&mut self, _: &mut State, _: usize) -> npc::Result<bool> {
        Err(npc::Error::Binding(
            "not ported in the live world: an NPC's dialogue player (dialogues not installed)"
                .into(),
        ))
    }
    fn dialogue_chatbox(&mut self, _: &mut State, _: usize) -> npc::Result<bool> {
        Err(npc::Error::Binding(
            "not ported in the live world: an NPC's dialogue player (dialogues not installed)"
                .into(),
        ))
    }
    fn random_bool(&mut self) -> npc::Result<bool> {
        // RandomFunction.nextBool(): RANDOM.nextBoolean().
        Ok(self.random.shared.next_bool())
    }
    fn randomize(&mut self, bound: i32) -> npc::Result<i32> {
        // RandomFunction.randomize(value): RANDOM.nextInt(value), 0 for value <= 0.
        if bound <= 0 {
            return Ok(0);
        }
        self.random
            .shared_next_int(bound)
            .ok_or(npc::Error::Binding("nextInt bound".into()))
    }
    fn random_range(&mut self, min: i32, max: i32) -> npc::Result<i32> {
        // RandomFunction.random(a, b): a + RANDOM.nextInt(b - a), a when
        // the bound is not positive.
        let n = max.wrapping_sub(min).wrapping_abs();
        if n <= 0 {
            return Ok(min.min(max));
        }
        let draw = self
            .random
            .shared_next_int(n)
            .ok_or(npc::Error::Binding("nextInt bound".into()))?;
        Ok(min.min(max).wrapping_add(draw))
    }
    fn npc_can_move(&mut self, _: &mut State, _: Node, _: Option<Location>) -> npc::Result<bool> {
        // Entity.canMove(destination) returns true; NPC does not override it
        // (content NPC classes that do are not ported).
        Ok(true)
    }
    fn npc_water_path(
        &mut self,
        state: &mut State,
        node: Node,
        location: Location,
    ) -> npc::Result<crate::world::pathfinding::Path> {
        // Pathfinder.findWater(npc, destination, true, Pathfinder.DUMB) for a
        // water_npc: the water clipping of the cache scene.
        state
            .context(self)
            .find_water_path(node, location)
            .map_err(nerror)
    }
}

impl Live {
    /// `NPC.tick()`.
    pub fn npc_tick_of(&mut self, p: &P) -> npc::Result<()> {
        let node = p.node();
        let result = self.with_walk_host(|state, host| npc_walk::tick(state, host, node));
        self.project_npc(node);
        result
    }

    /// The NPC renderer's view of an NPC from its walking entity.
    pub fn project_npc(&mut self, node: Node) {
        let Ok(actor) = self.inner.state.actor(node) else {
            return;
        };
        let entity = actor.queue.entity();
        let (walk, run) = (actor.queue.walk_dir(), actor.queue.run_dir());
        let index = actor.wire_index;
        let Some(row) = self.inner.npcs.rows.get(&node) else {
            return;
        };
        let id = row.id;
        let aggressive = row.brain.fields.read().aggressive;
        let view = self
            .npc_views
            .entry(node)
            .or_insert_with(|| {
                crate::definitions::item::Shared::new(npc_render::Npc::new(
                    index,
                    id,
                    entity
                        .location
                        .clone()
                        .unwrap_or_else(|| Location::new(0, 0, 0)),
                ))
            })
            .clone();
        let mut v = view.write();
        v.index = index;
        v.location = entity.location.clone();
        v.teleporting = entity.teleporting;
        v.direction = entity.direction.map_or(v.direction, |d| d.code());
        v.walk_dir = walk;
        v.run_dir = run;
        v.aggressive = aggressive;
        // NPC update masks are not joined: no mask block is written.
        v.masks_required = false;
    }

    /// `NPC.reset()`: `Entity.reset` (masks, teleporting).
    pub fn npc_reset_of(&mut self, p: &P) -> npc::Result<()> {
        let actor = self.inner.state.actor(p.node()).map_err(nerror)?;
        actor.queue.state.write().entity.teleporting = false;
        if let Some(view) = self.npc_views.get(&p.node()) {
            view.write().teleporting = false;
        }
        self.inner
            .not_ported
            .skip("NPC update masks reset (NPC masks not joined)");
        Ok(())
    }

    /// `NPCRenderer.render(player)`.
    pub fn render_npcs_of(&mut self, p: &P) -> npc_render::Result<()> {
        let local = self.inner.canonical(p).render.clone();
        let mut ctx = Render {
            live: self,
            frames: Vec::new(),
        };
        npc_render::render(&mut ctx, &local)?;
        let frames = ctx.frames;
        if let Some(session) = self.inner.session_of(p) {
            for frame in frames {
                self.inner.direct_native(session, frame);
            }
        }
        Ok(())
    }
}

struct Render<'a> {
    live: &'a mut Live,
    frames: Vec<Frame>,
}

fn rerror(e: impl std::fmt::Debug) -> player_render::Error {
    player_render::Error::Binding(format!("{e:?}"))
}

impl Render<'_> {
    fn node_of(&self, npc: &npc_render::Handle) -> Option<Node> {
        self.live
            .npc_views
            .iter()
            .find(|(_, v)| v.same(npc))
            .map(|(n, _)| *n)
    }
}

impl npc_render::Context for Render<'_> {
    fn hidden(
        &mut self,
        _: &player_render::Handle,
        npc: &npc_render::Handle,
    ) -> npc_render::Result<bool> {
        // NPC.isHidden(player) = isInvisible(): inactive, respawning, or
        // the entity's invisible flag.
        let node = self.node_of(npc).ok_or(player_render::Error::Null)?;
        let ticks = self.live.inner.ticks;
        let row = self
            .live
            .inner
            .npcs
            .rows
            .get(&node)
            .ok_or(player_render::Error::Null)?;
        if !row.active || row.brain.fields.read().respawn_tick > ticks {
            return Ok(true);
        }
        Ok(self
            .live
            .inner
            .npcs
            .ai
            .get(&node)
            .is_some_and(|ai| ai.invisible))
    }
    fn candidates(
        &mut self,
        location: Location,
        distance: i32,
    ) -> npc_render::Result<Vec<Option<npc_render::Handle>>> {
        let nodes = self
            .live
            .local(location, distance, Select::Npcs)
            .map_err(rerror)?;
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            if !self.live.npc_views.contains_key(&node.node) {
                self.live.project_npc(node.node);
            }
            out.push(self.live.npc_views.get(&node.node).cloned());
        }
        Ok(out)
    }
    fn masks(
        &mut self,
        _: &player_render::Handle,
        _: &npc_render::Handle,
        _: &mut Vec<u8>,
        _: bool,
    ) -> npc_render::Result<()> {
        Err(player_render::Error::Binding(
            "not ported in the live world: NPC update masks".into(),
        ))
    }
    fn remove_tolerance(&mut self, _: &npc_render::Handle, _: i32) -> npc_render::Result<()> {
        self.live
            .inner
            .not_ported
            .skip("AggressiveHandler tolerance (NPC aggression not installed)");
        Ok(())
    }
    fn add_tolerance(&mut self, _: &npc_render::Handle, _: i32) -> npc_render::Result<()> {
        self.live
            .inner
            .not_ported
            .skip("AggressiveHandler tolerance (NPC aggression not installed)");
        Ok(())
    }
    fn send(&mut self, _: &player_render::Handle, frame: Frame) -> npc_render::Result<()> {
        self.frames.push(frame);
        Ok(())
    }
}

/// Joins the start-up's NPCs: the live repository's NPC list in slot order
/// (each lands in its own slot) and the scene chunk of its location
/// (`RegionManager.move(npc, null, location)`'s chunk part; the start-up ran
/// the move against its own chunk store).
pub fn join(live: &mut Live) -> Result<usize, String> {
    let startup = live.inner.npcs.repository.npcs().occupied();
    let mut joined = 0;
    for (slot, handle) in startup {
        let added = live.repository.npcs().add(P(handle.clone()));
        if added != Some(slot) {
            return Err(format!(
                "NPC slot {slot} landed in {added:?} of the live repository"
            ));
        }
        let Some(location) = live
            .inner
            .state
            .node_location(handle.node)
            .map_err(|e| e.to_string())?
        else {
            continue;
        };
        let chunk = entity_regions::Context::chunk(live, location).map_err(|e| e.to_string())?;
        entity_regions::add_npc(&chunk, Some(handle));
        joined += 1;
    }
    Ok(joined)
}
