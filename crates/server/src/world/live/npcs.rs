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

impl WalkHost<'_> {
    /// `CombatPulse.isAttacking()` of an entity from the combat state.
    fn is_attacking(&self, node: Node) -> bool {
        let active = |n: Node| {
            self.players
                .get(&n)
                .map(|s| s.active)
                .or_else(|| self.npcs.rows.get(&n).map(|r| r.active))
                .unwrap_or(false)
        };
        self.combat.entities.get(&node).is_some_and(|entry| {
            entry.state.running
                && entry
                    .state
                    .victim
                    .as_ref()
                    .is_some_and(|victim| active(victim.node()))
        })
    }
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
                // CombatPulse.isAttacking: a victim that is active and a
                // running pulse.
                self.is_attacking(node)
            }
            npc::Query::InCombat => {
                // CombatPulse.isInCombat: the combat-attacker attribute's
                // pulse is attacking.
                match self
                    .combat
                    .attrs
                    .get(&node)
                    .and_then(|a| a.get("combat-attacker"))
                {
                    Some(super::combat::Attr::Entity(attacker)) => {
                        self.is_attacking(attacker.node())
                    }
                    _ => false,
                }
            }
            npc::Query::SelectTarget => {
                // AggressiveHandler.selectTarget needs the whole world: it
                // runs right after this tick's kernel returns; the kernel
                // continues as if no target was selected.
                self.deferred
                    .push(super::walk::Deferred::SelectTarget(node));
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
            npc::Event::Restore => {
                // Entity.fullRestore(): skills.restore() (every level back to
                // its static level, lifepoints to the maximum), then the
                // poison timers' removal (NPC timers are not installed: an
                // NPC has none).
                let row = self.npcs.rows.get_mut(&node).ok_or(npc::Error::Null)?;
                row.levels = row.static_levels;
                row.lifepoints = row.static_levels[3];
                self.not_ported
                    .default_answer("NPC timer queries (none: no NPC timer is registered)");
                Ok(())
            }
            npc::Event::BehaviorRespawn | npc::Event::Respawn | npc::Event::PauseAggression(_) => {
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
            npc::Event::ImpactDisabled(ticks) => {
                // Run with the whole world right after this walk call.
                self.deferred
                    .push(super::walk::Deferred::ImpactDisabled(node, ticks));
                Ok(())
            }
            npc::Event::StopCombat => {
                self.deferred.push(super::walk::Deferred::CombatStop(node));
                Ok(())
            }
            npc::Event::Attribute(_) => {
                self.not_ported
                    .skip("NPC attributes (return-to-spawn, lastForceTalk: not stored)");
                Ok(())
            }
            npc::Event::Chat(text) => {
                // Entity.sendChat(text): the force chat mask.
                let text = text.ok_or(npc::Error::Null)?;
                let masks = self
                    .combat
                    .npc_masks
                    .entry(node)
                    .or_insert_with(|| {
                        crate::world::update_masks::Masks::new(
                            node,
                            crate::world::update_masks::Kind::Npc,
                        )
                    })
                    .clone();
                masks
                    .register(
                        &mut super::combat::masks::Registry,
                        crate::world::update_masks::Flag::ForceChat,
                        Some(super::combat::masks::Payload::Chat(
                            String::from_utf16_lossy(&text),
                        )),
                        false,
                    )
                    .map_err(nerror)?;
                Ok(())
            }
            other => Err(npc::Error::Binding(format!(
                "not ported in the live world: NPC event {other:?}"
            ))),
        }
    }
    fn dialogue_active(&mut self, _: &mut State, player: usize) -> npc::Result<bool> {
        // dialoguePlayer.isActive() (the dialogue interpreter sets the
        // NPC's dialogue player, super::talk).
        Ok(self.players.get(&player).is_some_and(|slot| slot.active))
    }
    fn dialogue_chatbox(&mut self, _: &mut State, player: usize) -> npc::Result<bool> {
        // dialoguePlayer.getInterfaceManager().hasChatbox().
        Ok(self.players.get(&player).is_some_and(|slot| {
            crate::world::interfaces::has_chatbox(&slot.canonical.player.manager())
        }))
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
        // NPC.tick reaches Entity.tick (and updateMasks.prepare) unless its
        // region is inactive or it waits for its respawn tick.
        let reaches_entity_tick = {
            let location = self.inner.state.node_location(node).ok().flatten();
            let active = location.is_some_and(|l| {
                let region = self.walk.scene.region(super::walk::region_id(&l));
                region_activity::is_active(&region)
            });
            let waiting = self
                .inner
                .npcs
                .rows
                .get(&node)
                .is_some_and(|row| row.brain.fields.read().respawn_tick > self.inner.ticks);
            active && !waiting
        };
        let result = self.with_walk_host(|state, host| npc_walk::tick(state, host, node));
        if result.is_ok() && reaches_entity_tick {
            self.npc_prepare_masks(node).map_err(nerror)?;
        }
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
        drop(v);
        // UpdateMasks.isUpdateRequired of the NPC's masks.
        let required = self.npc_masks_required(node);
        view.write().masks_required = required;
    }

    /// `NPC.reset()`: `Entity.reset` (masks, teleporting).
    pub fn npc_reset_of(&mut self, p: &P) -> npc::Result<()> {
        let actor = self.inner.state.actor(p.node()).map_err(nerror)?;
        actor.queue.state.write().entity.teleporting = false;
        self.npc_reset_masks(p.node());
        if let Some(view) = self.npc_views.get(&p.node()) {
            let mut v = view.write();
            v.teleporting = false;
            v.masks_required = false;
        }
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
        local: &player_render::Handle,
        npc: &npc_render::Handle,
        out: &mut Vec<u8>,
        sync: bool,
    ) -> npc_render::Result<()> {
        // NPCRenderer.writeMaskUpdates over the NPC's UpdateMasks.
        let node = self.node_of(npc).ok_or(player_render::Error::Null)?;
        let viewer = self
            .live
            .inner
            .by_render(local)
            .map(|p| p.node())
            .ok_or(player_render::Error::Null)?;
        self.live
            .write_npc_masks(node, viewer, out, sync)
            .map_err(rerror)
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
