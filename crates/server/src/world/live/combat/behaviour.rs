//! `npc_behaviour::Host` over the live world: the NPC's AI with the live
//! world as its host (aggression through `AggressiveHandler.selectTarget`,
//! the behaviour hooks, the runtime class), and the members of `Entity`,
//! `NPC` and `Player` on the combat path (`npc_behaviour::combat`).
//!
//! Each NPC's `Ai` is made the first time it is asked for, from the
//! start-up's: the start-up's NPC store is a host of its own, so the live
//! world keeps a second `AggressiveHandler` object whose fields are the
//! start-up's (radius, chance ratio, tolerance, target switching; the pause
//! tick is read from the start-up's object at each selection, where
//! `NPC.tick` writes it) and whose behaviour is DEFAULT, or WILDERNESS for
//! the NPCs `ZoneBuilder.startup` gave the wilderness handler.
use super::super::host::Live;
use super::{
    super::P,
    Attr,
    host::{bind, refuse},
    style_handler,
};
use crate::{
    definitions::property::Property,
    world::{
        combat_pulse as cp,
        combat_swing::{CombatStyle, Error, Kind, Result},
        doors::Location,
        entity_regions::Select,
        npc,
        npc_behaviour::{
            self as nb, Ai, DefaultBehaviour, Hitsplat, PlainNpc, Rights,
            aggression::{self, AggressiveBehaviour},
        },
        player_walk::Node,
        wilderness_zone::WildernessZone,
    },
};
use std::sync::Arc;

impl Live {
    /// The live AI of an NPC, built from the start-up's on first use.
    fn live_ai(&mut self, npc: &P) -> Option<&mut Ai<Live>> {
        let node = npc.node();
        if !self.inner.combat.ai.contains_key(&node) {
            let startup = self.inner.npcs.ai.get(&node)?;
            let invisible = startup.invisible;
            let fields = startup.handler.as_ref().map(|h| {
                let f = h.0.read();
                (
                    f.radius,
                    f.target_switching,
                    f.chance_ratio,
                    f.allow_tolerance,
                )
            });
            let mut ai = Ai::new(Arc::new(DefaultBehaviour), Arc::new(PlainNpc));
            ai.invisible = invisible;
            if let Some((radius, switching, chance, tolerance)) = fields {
                let behaviour: Arc<dyn AggressiveBehaviour<Live>> =
                    if self.inner.combat.wilderness_aggressors.contains(&node) {
                        Arc::new(aggression::Wilderness)
                    } else {
                        Arc::new(aggression::DefaultAggression)
                    };
                let handler = aggression::Handler::new(self, npc.clone(), behaviour);
                {
                    let mut f = handler.0.write();
                    f.radius = radius;
                    f.target_switching = switching;
                    f.chance_ratio = chance;
                    f.allow_tolerance = tolerance;
                }
                ai.handler = Some(handler);
            }
            self.inner.combat.ai.insert(node, ai);
        }
        self.inner.combat.ai.get_mut(&node)
    }

    /// `AggressiveHandler.selectTarget()` from `NPC.handleTickActions`.
    pub fn npc_select_target(&mut self, npc: &P) -> Result<bool> {
        let handler = self.live_ai(npc).and_then(|ai| ai.handler.clone());
        let Some(handler) = handler else {
            return Err(Error::Null);
        };
        // The pause tick NPC.tick wrote on the start-up's handler object.
        let pause = self
            .inner
            .npcs
            .ai
            .get(&npc.node())
            .and_then(|ai| ai.handler.as_ref().map(|h| h.0.read().pause_ticks));
        if let Some(pause) = pause {
            handler.0.write().pause_ticks = pause;
        }
        let result = handler.select_target(self);
        self.sync_combat();
        result
    }

    fn npc_config(&self, npc: &P, key: &str) -> Option<Property> {
        let row = self.inner.npcs.rows.get(&npc.node())?;
        let handlers = row.definition.read().handlers.clone();

        handlers.read().get(key).cloned()
    }
}

/// `NPC.getSwingHandler(swing)`: the runtime class's override when the NPC
/// was constructed as a subclass (MonkNPC, the dragons:
/// super::super::subclasses), else the combat pulse style's handler offered
/// to the behaviour. The base member does not read `swing`.
pub fn npc_swing_handler(live: &mut Live, npc: &P) -> Result<usize> {
    super::super::subclasses::swing_handler(live, npc, true)
}

/// `entity.onAttack(target)`: `Entity`'s empty body, or a subclass's
/// override (HighwayManNPC: super::super::subclasses).
pub fn on_attack(live: &mut Live, entity: &P, target: &P) -> Result<()> {
    super::super::subclasses::on_attack(live, entity, target)
}

impl nb::Host for Live {
    type Handler = usize;
    type Region = i32;
    fn kind(&self, entity: &P) -> Kind {
        self.ckind(entity)
    }
    fn same(&self, a: &P, b: &P) -> bool {
        a.node() == b.node()
    }
    fn ai(&mut self, npc: &P) -> Option<&mut Ai<Self>> {
        self.live_ai(npc)
    }
    fn brain(&self, npc: &P) -> Option<npc::State> {
        self.inner
            .npcs
            .rows
            .get(&npc.node())
            .map(|r| r.brain.clone())
    }
    fn location(&self, entity: &P) -> Option<Location> {
        self.cloc(entity)
    }
    fn set_location(&mut self, npc: &P, location: Option<Location>) {
        if let Ok(actor) = self.inner.state.actor_mut(npc.node()) {
            actor.view.location = location.clone();
            actor.queue.state.write().entity.location = location;
        }
    }
    fn style_handler(&self, style: CombatStyle) -> usize {
        style_handler(style)
    }
    fn region_dynamic(&self, _: &i32) -> bool {
        false
    }
    fn max_players(&self) -> i32 {
        crate::world::repository::MAX_PLAYERS
    }
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.inner.ticks)
    }
    fn current_time_millis(&mut self) -> Result<i64> {
        Ok(super::super::now_millis())
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        crate::world::combat_swing::Host::next_int(self, bound)
    }
    fn math_random(&mut self) -> Result<f64> {
        Ok(self.inner.generators.math_random())
    }
    fn is_active(&mut self, entity: &P) -> Result<bool> {
        Ok(self.inner.node_active(entity.node()))
    }
    fn is_dead(&mut self, entity: &P) -> Result<bool> {
        crate::world::npc_death::is_dead(self, &entity.0).map_err(bind)
    }
    fn interaction_locked(&mut self, entity: &P) -> Result<bool> {
        self.locked_kind(entity, crate::world::action_locks::Kind::Interaction)
    }
    fn attribute_bool(&mut self, entity: &P, key: &str, default: bool) -> Result<bool> {
        match self.inner.attribute(entity.node(), key) {
            None => Ok(default),
            Some(Attr::Bool(v)) => Ok(v),
            Some(_) => Err(Error::Binding(format!("{key} is not a boolean"))),
        }
    }
    fn attribute_long(&mut self, entity: &P, key: &str, default: i64) -> Result<i64> {
        match self.inner.attribute(entity.node(), key) {
            None => Ok(default),
            Some(Attr::Long(v)) => Ok(v),
            Some(_) => Err(Error::Binding(format!("{key} is not a long"))),
        }
    }
    fn attribute_entity(&mut self, entity: &P, key: &str) -> Result<Option<P>> {
        match self.inner.attribute(entity.node(), key) {
            None => Ok(None),
            Some(Attr::Entity(p)) => Ok(Some(p)),
            Some(_) => Err(Error::Binding(format!("{key} is not an entity"))),
        }
    }
    fn set_attribute_entity(&mut self, entity: &P, key: &str, value: &P) -> Result<()> {
        self.inner
            .set_attribute(entity.node(), key, Attr::Entity(value.clone()));
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &P, key: &str) -> Result<()> {
        self.inner.remove_attribute(entity.node(), key);
        Ok(())
    }
    fn config_bool(&mut self, npc: &P, key: &str, default: bool) -> Result<bool> {
        match self.npc_config(npc, key) {
            None | Some(Property::Null) => Ok(default),
            Some(Property::Bool(v)) => Ok(v),
            Some(_) => Err(Error::Binding(format!("{key} is not a boolean"))),
        }
    }
    fn config_int(&mut self, npc: &P, key: &str, default: i32) -> Result<i32> {
        match self.npc_config(npc, key) {
            None | Some(Property::Null) => Ok(default),
            Some(Property::Int(v)) => Ok(v),
            Some(_) => Err(Error::Binding(format!("{key} is not an integer"))),
        }
    }
    fn definition_combat_level(&mut self, npc: &P) -> Result<i32> {
        self.npc_row(npc)
            .map(|row| row.definition.read().combat_level)
            .ok_or(Error::Null)
    }
    fn has_action(&mut self, npc: &P, action: &str) -> Result<bool> {
        // NPCDefinition.hasAction: an option equal ignoring case.
        let row = self.npc_row(npc).ok_or(Error::Null)?;
        let options = row.definition.read().options.clone();
        Ok(options.is_some_and(|o| {
            o.read()
                .iter()
                .flatten()
                .any(|name| name.eq_ignore_ascii_case(action))
        }))
    }
    fn shown_npc(&mut self, npc: &P, _: &P) -> Result<P> {
        let row = self.npc_row(npc).ok_or(Error::Null)?;
        if row.definition.read().child_npc_ids.is_some() {
            return refuse("NPC.getShownNPC for a morphing NPC (child ids)");
        }
        Ok(npc.clone())
    }
    fn is_attacking(&mut self, entity: &P) -> Result<bool> {
        cp::pulse::is_attacking(self, entity)
    }
    fn combat_victim(&mut self, entity: &P) -> Result<Option<P>> {
        Ok(self.inner.combat_entry(entity).state.victim.clone())
    }
    fn combat_set_victim(&mut self, entity: &P, victim: &P) -> Result<()> {
        cp::pulse::set_victim(self, entity, Some(victim))
    }
    fn combat_attack(&mut self, entity: &P, victim: Option<&P>) -> Result<()> {
        cp::pulse::attack(self, entity, victim)
    }
    fn combat_stop(&mut self, entity: &P) -> Result<()> {
        cp::pulse::stop(self, entity)
    }
    fn next_attack(&mut self, entity: &P) -> Result<i32> {
        Ok(self.inner.combat_entry(entity).state.next_attack)
    }
    fn combat_style(&mut self, entity: &P) -> Result<Option<CombatStyle>> {
        Ok(Some(self.inner.combat_entry(entity).state.style))
    }
    fn set_last_received_attack(&mut self, entity: &P, tick: i32) -> Result<()> {
        self.inner.combat_entry(entity).state.last_received_attack = tick;
        Ok(())
    }
    fn last_sent_attack(&mut self, entity: &P) -> Result<i32> {
        Ok(self.inner.combat_entry(entity).state.last_sent_attack)
    }
    fn set_last_sent_attack(&mut self, entity: &P, tick: i32) -> Result<()> {
        self.inner.combat_entry(entity).state.last_sent_attack = tick;
        Ok(())
    }
    fn face(&mut self, entity: &P, target: Option<&P>) -> Result<()> {
        self.entity_face(entity, target)
    }
    fn is_multi_zone(&mut self, entity: &P) -> Result<bool> {
        if self.inner.is_player_node(entity.node()) {
            return self
                .inner
                .with_profile(entity, |p| p.properties.multi_zone)
                .map_err(bind);
        }
        Ok(self.zones.multi.contains(&entity.node()))
    }
    fn current_combat_level(&mut self, entity: &P) -> Result<i32> {
        if let Some(row) = self.npc_row(entity) {
            return Ok(row.properties.combat_level);
        }
        // Properties.getCurrentCombatLevel for a player: no familiar is
        // summoned, so isUsingSummoning is false.
        let pvp = self.inner.combat.pvp_world;
        let wild_pvp = self.zones.wild_pvp;
        let in_wilderness = if wild_pvp {
            crate::world::player_managers::skull::answers::is_wilderness(self, &entity.0)
                .map_err(bind)?
        } else {
            false
        };
        self.inner
            .with_profile(entity, |p| {
                p.properties
                    .current_combat_level(crate::player::properties::CombatLevelInputs {
                        pvp_world: pvp,
                        wild_pvp,
                        in_wilderness,
                        using_summoning: false,
                        summoning_combat_level: p.summoning_combat_level,
                    })
            })
            .map_err(bind)
    }
    fn is_retaliating(&mut self, entity: &P) -> Result<bool> {
        if self.inner.is_player_node(entity.node()) {
            return self
                .inner
                .with_profile(entity, |p| p.properties.retaliating)
                .map_err(bind);
        }
        Ok(!matches!(
            self.inner
                .attribute(entity.node(), "properties:retaliating"),
            Some(Attr::Bool(false))
        ))
    }
    fn set_npc_walkable(&mut self, npc: &P, value: bool) -> Result<()> {
        self.inner
            .set_attribute(npc.node(), "properties:npcWalkable", Attr::Bool(value));
        Ok(())
    }
    fn set_combat_time_out(&mut self, npc: &P, value: i32) -> Result<()> {
        self.inner
            .set_attribute(npc.node(), "properties:combatTimeOut", Attr::Int(value));
        Ok(())
    }
    fn local_players(&mut self, location: Location, radius: i32) -> Result<Vec<P>> {
        let nodes = self
            .local(location, radius, Select::Players)
            .map_err(bind)?;
        Ok(nodes
            .into_iter()
            .filter(|n| self.inner.players.contains_key(&n.node))
            .map(P)
            .collect())
    }
    fn wilderness_in_zone(&mut self, entity: &P) -> Result<bool> {
        // WildernessZone.isInZone(entity) over the INSTANCE borders.
        let zone = WildernessZone::instance_borders();
        let at = self.cloc(entity);
        for borders in &zone {
            if borders.inside_location(at.as_ref()).map_err(bind)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn region_for_id(&mut self, region_id: i32) -> Result<i32> {
        Ok(region_id)
    }
    fn is_tolerated(&mut self, region: &i32, player: &P) -> Result<bool> {
        // Region.isTolerated: ten minutes since the player entered.
        let name = self
            .inner
            .players
            .get(&player.node())
            .map(|s| s.name.clone())
            .ok_or(Error::Null)?;
        let now = super::super::now_millis();
        let handle = self.walk.scene.region(*region);
        let map = handle.read().tolerances.clone();
        let entered = match map.read().get(&name) {
            None => now,
            Some(Some(t)) => *t,
            Some(None) => return Err(Error::Null),
        };
        Ok(now.wrapping_sub(entered) > 600_000)
    }
    fn rights(&mut self, player: &P) -> Result<Option<Rights>> {
        let rights = self
            .inner
            .with_profile(player, |p| p.details.rights().ok())
            .map_err(bind)?;
        Ok(rights.map(|r| match r {
            crate::player::details::Rights::Administrator => Rights::Administrator,
            crate::player::details::Rights::PlayerModerator => Rights::PlayerModerator,
            _ => Rights::RegularPlayer,
        }))
    }
    fn player_invisible(&mut self, player: &P) -> Result<bool> {
        self.inner
            .with_profile(player, |p| p.flags.invisible)
            .map_err(bind)
    }
    fn skull_wilderness(&mut self, player: &P) -> Result<bool> {
        crate::world::player_managers::skull::answers::is_wilderness(self, &player.0).map_err(bind)
    }
    fn wilderness_protection(&mut self, player: &P) -> Result<bool> {
        crate::world::player_managers::skull::answers::has_wilderness_protection(self, &player.0)
            .map_err(bind)
    }
    fn settings(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn wild_pvp_enabled(&mut self) -> Result<bool> {
        Ok(self.zones.wild_pvp)
    }
    fn zone_continue_attack(
        &mut self,
        entity: &P,
        target: &P,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool> {
        let monitor = self.monitor(&entity.0);
        let previous = self.inner.combat.zone_style.replace_with_style(style);
        let result = monitor.continue_attack(self, Some(&target.0), Some(&()), message);
        self.inner.combat.zone_style = previous;
        result.map_err(bind)
    }
    fn zone_is_pvpable(
        &mut self,
        owner: &P,
        attacker: &P,
        victim: &P,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool> {
        let monitor = self.monitor(&owner.0);
        let previous = self.inner.combat.zone_style.replace_with_style(style);
        let result =
            monitor.is_pvpable(self, Some(&attacker.0), Some(&victim.0), Some(&()), message);
        self.inner.combat.zone_style = previous;
        result.map_err(bind)
    }
    fn level(&mut self, entity: &P, slot: i32) -> Result<i32> {
        crate::world::combat_swing::Host::level(self, entity, slot)
    }
    fn familiar_owner(&mut self, _: &P) -> Result<Option<P>> {
        refuse("Familiar.getOwner (familiars are not spawned)")
    }
    fn send_message(&mut self, player: &P, text: &str) -> Result<()> {
        crate::world::combat_swing::Host::send_message(self, player, text)
    }
    fn path_find(&mut self, _: Option<&P>, _: Option<Location>) -> Result<()> {
        refuse("Pathfinder.find for the safespot rule")
    }
    fn path_walk(&mut self, _: Option<&P>) -> Result<()> {
        refuse("Path.walk for the safespot rule")
    }
    fn move_step(&mut self, _: &P) -> Result<()> {
        refuse("Entity.moveStep for the safespot rule")
    }
    fn walking_has_path(&mut self, entity: &P) -> Result<bool> {
        let actor = self.inner.state.actor(entity.node()).map_err(bind)?;
        Ok(actor.queue.has_path())
    }
    fn pulse_manager_is_moving(&mut self, entity: &P) -> Result<bool> {
        cp::Host::pulse_manager_is_moving(self, entity)
    }
    fn ring_id(&mut self, player: &P) -> Result<i32> {
        Ok(self.equipment_slot(player, 12).map_or(0, |i| i.read().id()))
    }
    fn send_chat(&mut self, entity: &P, text: &str) -> Result<()> {
        if self.inner.is_npc_node(entity.node()) {
            self.npc_chat(entity.node(), text.to_owned())
                .map_err(bind)?;
            return Ok(());
        }
        refuse("Player.sendChat (force chat on a player)")
    }
    fn manual_hit(&mut self, target: &P, source: &P, hit: i32, _: Hitsplat) -> Result<()> {
        cp::impact::manual_hit(
            self,
            target,
            Some(source),
            hit,
            Some(cp::HitsplatType::Normal),
        )
        .map(|_| ())
    }
    fn handle_recoil_effect(&mut self, player: &P, attacker: Option<&P>, hit: i32) -> Result<()> {
        cp::impact::handle_recoil_effect(self, player, attacker.ok_or(Error::Null)?, hit)
    }
    fn check_armour_degrades(&mut self, _: &P) -> Result<()> {
        self.inner.not_ported.default_answer(
            "DegradableEquipment.checkArmourDegrades (no degradable equipment registered)",
        );
        Ok(())
    }
    fn walking_reset(&mut self, entity: &P) -> Result<()> {
        self.walk_reset(entity)
    }
    fn pulse_manager_clear(&mut self, entity: &P) -> Result<()> {
        cp::Host::pulse_manager_clear(self, entity)
    }
    fn masks_reset(&mut self, entity: &P) -> Result<()> {
        self.npc_reset_masks(entity.node());
        Ok(())
    }
    fn chunk_remove(&mut self, _: &P, _: Location) -> Result<()> {
        refuse("NPC chunk moves outside the NPC tick")
    }
    fn chunk_add(&mut self, _: &P, _: Location) -> Result<()> {
        refuse("NPC chunk moves outside the NPC tick")
    }
    fn return_arrival(&mut self, _: &P) -> Result<bool> {
        refuse("the return-to-spawn arrival outside the NPC tick")
    }
    fn pulse_stop(&mut self, _: &P, _: usize) -> Result<()> {
        refuse("the return-to-spawn pulse outside the NPC tick")
    }
    fn remove_renderable(&mut self, _: &P) -> Result<()> {
        refuse("Repository.removeRenderableNPC outside the NPC tick")
    }
    fn npc_clear(&mut self, _: &P) -> Result<()> {
        refuse("NPC.clear (dynamic regions are not built)")
    }
    fn skills_restore(&mut self, entity: &P) -> Result<()> {
        super::death::restore_npc_skills(self, entity);
        Ok(())
    }
    fn remove_timer(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("NPC timers (not installed)")
    }
    fn clipping_flag(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        cp::Host::clipping_flag(self, z, x, y)
    }
    fn other_clipping_flag(&mut self, _: u32, _: i32, _: i32, _: i32) -> Result<i32> {
        refuse("content NPC clipping suppliers")
    }
}

/// Replaces the zone monitor's style argument for one call.
trait ReplaceStyle {
    fn replace_with_style(&mut self, style: Option<CombatStyle>) -> Option<CombatStyle>;
}
impl ReplaceStyle for Option<CombatStyle> {
    fn replace_with_style(&mut self, style: Option<CombatStyle>) -> Option<CombatStyle> {
        std::mem::replace(self, style)
    }
}

/// The NPCs' node ids, for the aggression list.
pub fn is_npc(live: &Live, node: Node) -> bool {
    live.inner.is_npc_node(node)
}
