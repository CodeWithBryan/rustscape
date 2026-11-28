//! NPC death and drops in the live world: `npc_death::Host` and
//! `drop::Host` over the live world, with the ground manager
//! (`ground_manager::Context`) of [`super::super::ground_items`].
//!
//! `Entity.startDeath` from `ImpactHandler.handleImpact` runs
//! `npc_death::entity_start_death`; the death task's pulse is a strong pulse
//! of the entity's pulse manager on `GameWorld.Pulser`, whose `pulse` and
//! `stop` are `task_pulse` and `task_stop`; `finalizeDeath` rolls the NPC's
//! drop table (the start-up's `DropTableParser` and the shared tables loaded
//! at boot) and `createDrop` puts the items on the ground through the
//! ground manager. `npc_death`'s damage-log view is made from the combat
//! pulse's `ImpactHandler` at the start of each death call and written back
//! (disabled ticks, cleared logs) at its end.
//!
//! A player's death (`Player.commenceDeath`/`finalizeDeath`,
//! `world/player_death`) is [`super::player_death`].
use super::super::{ground_items::World, host::Live};
use super::{super::P, Attr, Submitted, host::bind};
use crate::{
    definitions::{
        item::{Handle as Definition, Shared},
        npc as definitions,
        property::Property,
    },
    inventory::item,
    pulses,
    world::{
        chunk::HandleChunk,
        chunk_sync::Items as ItemList,
        combat_swing::{self as cs},
        doors::Location,
        drop_tables::{
            self as tables, Looter, NpcDropTable, PlayerLooter, RollError, RolledItem,
            drop::{self, Error, Result},
            random::Source,
        },
        ground_item::{self as ground, Handle as Ground},
        ground_manager::{self as gm, Packet},
        npc,
        npc_death::{self, Attribute, ImpactLog, Task},
        player_walk::{Node, NodeHandle},
    },
};

fn binding(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}
fn refused<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}
fn gbinding(e: impl std::fmt::Debug) -> ground::Error {
    ground::Error::Binding(format!("{e:?}"))
}

impl Live {
    fn handle(&self, node: &NodeHandle) -> P {
        P(node.clone())
    }
    /// Writes `npc_death`'s view of the damage logs back to the impact
    /// handlers and drops the views.
    fn settle_death_logs(&mut self) {
        let logs: Vec<(Node, Shared<ImpactLog>)> = self.inner.combat.death_logs.drain().collect();
        for (node, log) in logs {
            let (disabled, empty) = {
                let log = log.read();
                (
                    log.disabled_ticks,
                    log.player_count() == 0 && log.npcs().is_empty(),
                )
            };
            if let Some(entry) = self.inner.combat.entities.get_mut(&node) {
                entry.impact.disabled_ticks = disabled;
                if empty {
                    let _ = entry.impact.clear_npc_log();
                    entry.impact.clear_player_log();
                }
            }
        }
    }
}

/// `Entity.startDeath(killer)` from `ImpactHandler.handleImpact`.
pub fn start_death(live: &mut Live, entity: &P, killer: Option<&P>) -> cs::Result<()> {
    let result = npc_death::entity_start_death(live, &entity.0, killer.map(|k| &k.0));
    live.settle_death_logs();
    result.map(|_| ()).map_err(bind)
}

/// The death task's `Pulse.update` body: `exec`, then `stop` once it is done.
/// A failure while the death finishes prints and stops the task again, as
/// `Pulse.update`'s catch does.
pub fn death_pulse(live: &mut Live, task: &Task) -> std::result::Result<bool, String> {
    let done = npc_death::task_pulse(live, task);
    let result = match done {
        Ok(false) => Ok(false),
        Ok(true) => match npc_death::task_stop_with(
            live,
            task,
            super::super::subclasses::npc_finalize_death,
        ) {
            Ok(()) => Ok(true),
            Err(error) => {
                live.inner
                    .log_line(format!("DeathTask printStackTrace {error:?}"));
                npc_death::task_stop_with(live, task, super::super::subclasses::npc_finalize_death)
                    .map(|_| true)
                    .map_err(|e| format!("{e:?}"))
            }
        },
        Err(error) => {
            live.inner
                .log_line(format!("DeathTask printStackTrace {error:?}"));
            npc_death::task_stop_with(live, task, super::super::subclasses::npc_finalize_death)
                .map(|_| true)
                .map_err(|e| format!("{e:?}"))
        }
    };
    live.settle_death_logs();
    result
}

/// `applyPoison(victim, source, severity)` from a swing: the ContentAPI glue
/// of `timers::content` over the victim's timer manager (a player's, or an
/// NPC's: super::super::timers).
pub fn apply_poison(live: &mut Live, victim: &P, source: &P, severity: i32) -> cs::Result<()> {
    let result = live.with_timers(victim, |natives, manager| {
        crate::timers::content::apply_poison(natives, manager, source, severity)
    });
    result.map_err(|e| cs::Error::Binding(format!("{e:?}")))
}

/// `skills.restore()` of an NPC: every level back to its static level.
pub fn restore_npc_skills(live: &mut Live, npc: &P) {
    if let Some(row) = live.inner.npcs.rows.get_mut(&npc.node()) {
        row.levels = row.static_levels;
        row.lifepoints = row.static_levels[3];
    }
}

// ---------------------------------------------------- ground items

impl ground::Context for Live {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        World::with(self, |w| ground::Context::copy_item(w, id, amount, charge))
    }
    fn default_interaction(&mut self, item: &Ground) -> ground::Result<()> {
        World::with(self, |w| ground::Context::default_interaction(w, item))
    }
    fn uid(&mut self, player: &NodeHandle) -> ground::Result<i32> {
        World::with(self, |w| ground::Context::uid(w, player))
    }
    fn username(&mut self, player: &NodeHandle) -> ground::Result<Option<String>> {
        World::with(self, |w| ground::Context::username(w, player))
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(self.inner.ticks)
    }
    fn virtual_private(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        World::with(self, |w| ground::Context::virtual_private(w, item))
    }
    fn virtual_active(&mut self, item: &Ground) -> ground::Result<Option<bool>> {
        World::with(self, |w| ground::Context::virtual_active(w, item))
    }
}

impl gm::Context for Live {
    type Plugin = ();
    fn plugin(&mut self, definition: &Definition) -> gm::Result<Option<()>> {
        World::with(self, |w| gm::Context::plugin(w, definition))
    }
    fn plugin_remove(
        &mut self,
        plugin: (),
        player: Option<NodeHandle>,
        item: &Ground,
        amount: i32,
    ) -> gm::Result<()> {
        World::with(self, |w| {
            gm::Context::plugin_remove(w, plugin, player, item, amount)
        })
    }
    fn chunk(&mut self, location: Location) -> gm::Result<HandleChunk> {
        World::with(self, |w| gm::Context::chunk(w, location))
    }
    fn flagged(&mut self, chunk: &HandleChunk) -> gm::Result<()> {
        World::with(self, |w| gm::Context::flagged(w, chunk))
    }
    fn player_location(&mut self, player: &NodeHandle) -> gm::Result<Option<Location>> {
        World::with(self, |w| gm::Context::player_location(w, player))
    }
    fn playing(&mut self, player: &NodeHandle) -> gm::Result<bool> {
        World::with(self, |w| gm::Context::playing(w, player))
    }
    fn artificial(&mut self, player: &NodeHandle) -> gm::Result<bool> {
        World::with(self, |w| gm::Context::artificial(w, player))
    }
    fn ai_items(&mut self, player: &NodeHandle) -> gm::Result<Option<ItemList>> {
        World::with(self, |w| gm::Context::ai_items(w, player))
    }
    fn packet(
        &mut self,
        kind: Packet,
        player: Option<NodeHandle>,
        item: &Ground,
        old_amount: i32,
    ) -> gm::Result<()> {
        World::with(self, |w| {
            gm::Context::packet(w, kind, player, item, old_amount)
        })
    }
    fn auto_spawn(&mut self, item: &Ground) -> gm::Result<bool> {
        World::with(self, |w| gm::Context::auto_spawn(w, item))
    }
    fn respawn(&mut self, item: &Ground) -> gm::Result<()> {
        World::with(self, |w| gm::Context::respawn(w, item))
    }
}

/// `table.roll(looter)`'s questions to a looting player.
struct Looting {
    ring: i32,
    worn: Vec<i32>,
    skipped: Vec<&'static str>,
}
impl PlayerLooter for Looting {
    fn removes_nothings(&mut self) -> bool {
        // shouldRemoveNothings: a ring of wealth (2572) in the ring slot.
        self.ring == 2572
    }
    fn in_equipment(&mut self, item_id: i32) -> bool {
        self.worn.contains(&item_id)
    }
    fn update_glove_charges(&mut self) {
        self.skipped
            .push("FOGGlovesManager.updateCharges (Fist of Guthix not ported)");
    }
}

/// The roll's logging services.
#[derive(Default)]
pub(in crate::world::live) struct RollLog {
    pub lines: Vec<String>,
    pub skipped: Vec<&'static str>,
}
impl tables::Host for RollLog {
    fn clue_scrolls(&self, _: tables::ClueLevel) -> Option<&[i32]> {
        // ClueScrollPlugin.ORGANIZED: clue scrolls are content and none is
        // registered.
        None
    }
    fn log(&mut self, origin: &'static str, level: tables::LogLevel, message: &str) {
        self.lines.push(format!("{origin} {level:?} {message}"));
    }
    fn stack_trace(&mut self, origin: &'static str, failure: tables::shared::ParseFailure) {
        self.lines
            .push(format!("{origin} printStackTrace {failure:?}"));
    }
}

impl drop::Host for Live {
    type Clan = ();
    fn is_player(&mut self, entity: &NodeHandle) -> bool {
        self.inner.is_player_node(entity.node)
    }
    fn is_ai_player(&mut self, _: &NodeHandle) -> bool {
        false
    }
    fn random(&mut self) -> &mut dyn Source {
        &mut self.inner.generators
    }
    fn roll(
        &mut self,
        table: &NpcDropTable,
        looter: Option<&NodeHandle>,
    ) -> std::result::Result<Vec<RolledItem>, RollError> {
        let mut looting = looter
            .filter(|l| self.inner.is_player_node(l.node))
            .map(|l| {
                let p = P(l.clone());
                let items = self.inner.canonical(&p).player.equipment().items();
                let worn: Vec<i32> = items
                    .read()
                    .iter()
                    .flatten()
                    .map(|i| i.read().id())
                    .collect();
                let ring = self.equipment_slot(&p, 12).map_or(-1, |i| i.read().id());
                Looting {
                    ring,
                    worn,
                    skipped: Vec::new(),
                }
            });
        let mut log = RollLog {
            lines: Vec::new(),
            skipped: Vec::new(),
        };
        let receiver = match (&mut looting, looter) {
            (Some(l), _) => Looter::Player(l),
            (None, Some(_)) => Looter::Entity,
            (None, None) => Looter::None,
        };
        let inner = &mut self.inner;
        let result = {
            let mut ctx = tables::Context::new(
                &mut inner.generators,
                &mut inner.combat.shared_tables,
                &mut log,
            );
            table.roll(&mut ctx, receiver, 1)
        };
        for line in log.lines {
            self.inner.log_line(line);
        }
        for what in log
            .skipped
            .into_iter()
            .chain(looting.map(|l| l.skipped).unwrap_or_default())
        {
            self.inner.not_ported.skip(what);
        }
        self.inner
            .not_ported
            .default_answer("ClueScrollPlugin.ORGANIZED (no clue scrolls registered: none)");
        result
    }
    fn manager(&mut self) -> gm::Manager {
        self.walk.ground.manager.clone()
    }
    fn on_drop_table_rolled(
        &mut self,
        npc: &NodeHandle,
        looter: &NodeHandle,
        drops: &mut Vec<RolledItem>,
    ) -> Result<()> {
        let npc = self.handle(npc);
        let looter = self.handle(looter);
        let behaviour = crate::world::npc_behaviour::behaviour(self, &npc).map_err(binding)?;
        behaviour
            .on_drop_table_rolled(self, &npc, &looter, drops)
            .map_err(binding)
    }
    fn npc_id(&mut self, npc: &NodeHandle) -> Result<i32> {
        // Node.getId() of a player (Entity.finalizeDeath's Loar shade
        // test): a player's id is -1.
        if self.inner.is_player_node(npc.node) {
            return Ok(-1);
        }
        self.inner
            .npcs
            .rows
            .get(&npc.node)
            .map(|r| r.id)
            .ok_or(Error::Null)
    }
    fn npc_drop_location(&mut self, npc: &NodeHandle) -> Result<Option<Location>> {
        // NPC.getDropLocation(): the NPC's location (content classes
        // override it).
        Ok(self.cloc(&self.handle(npc)))
    }
    fn npc_location(&mut self, npc: &NodeHandle) -> Result<Option<Location>> {
        Ok(self.cloc(&self.handle(npc)))
    }
    fn npc_multi_zone(&mut self, npc: &NodeHandle) -> Result<bool> {
        Ok(self.zones.multi.contains(&npc.node))
    }
    fn plugin_create_drop(
        &mut self,
        _: (),
        _: &item::Handle,
        _: &NodeHandle,
        _: &NodeHandle,
        _: &Location,
    ) -> Result<bool> {
        refused("ItemPlugin.createDrop (no item plugin is registered)")
    }
    fn plugin_get_item(
        &mut self,
        _: (),
        _: &item::Handle,
        _: &NodeHandle,
    ) -> Result<Option<item::Handle>> {
        refused("ItemPlugin.getItem (no item plugin is registered)")
    }
    fn send_message(&mut self, player: &NodeHandle, message: &str) -> Result<()> {
        let p = self.handle(player);
        self.inner.message(&p, message).map_err(binding)
    }
    fn resizable(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.handle(player);
        Ok(self.inner.canonical(&p).client.is_resizable())
    }
    fn bone_crusher_enabled(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.handle(player);
        self.inner
            .with_profile(&p, |profile| profile.saved.global.enable_bone_crusher)
            .map_err(binding)
    }
    fn add_experience(&mut self, player: &NodeHandle, skill: i32, experience: f64) -> Result<()> {
        let p = self.handle(player);
        crate::world::combat_pulse::Host::add_experience(self, &p, skill, experience, true)
            .map_err(binding)
    }
    fn clan(&mut self, _: &NodeHandle) -> Result<Option<()>> {
        refused("player.getCommunication().getClan() for loot share (clan chat state is social's)")
    }
    fn clan_loot_requirement(&mut self, _: &()) -> Result<i32> {
        refused("ClanRepository.getLootRequirement")
    }
    fn loot_share(&mut self, _: &NodeHandle) -> Result<bool> {
        refused("Communication.isLootShare")
    }
    fn loot_requirement(&mut self, _: &NodeHandle) -> Result<i32> {
        refused("Communication.getLootRequirement")
    }
    fn ironman(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.handle(player);
        Ok(self.inner.canonical(&p).player.ironman().is_ironman())
    }
    fn loot_share_points(&mut self, _: &NodeHandle) -> Result<i32> {
        refused("GlobalData.getLootSharePoints")
    }
    fn set_loot_share_points(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refused("GlobalData.setLootSharePoints")
    }
    fn local_players(
        &mut self,
        location: Option<&Location>,
        distance: i32,
    ) -> Result<Vec<Option<NodeHandle>>> {
        let location = location.cloned().ok_or(Error::Null)?;
        let nodes = self
            .local(
                location,
                distance,
                crate::world::entity_regions::Select::Players,
            )
            .map_err(binding)?;
        Ok(nodes.into_iter().map(Some).collect())
    }
    fn recommended_price(&mut self, _: i32, _: bool) -> Result<i32> {
        refused("GrandExchange.getRecommendedPrice (the Grand Exchange is not installed)")
    }
    fn repository_players(&mut self) -> Result<Vec<NodeHandle>> {
        Ok(self
            .inner
            .players
            .values()
            .filter(|s| s.active)
            .map(|s| s.canonical.owner.clone())
            .collect())
    }
    fn send_news(&mut self, _: &str) -> Result<()> {
        self.inner
            .not_ported
            .skip("core.api.sendNews (rare drop news)");
        Ok(())
    }
    fn increment_rare_drop(&mut self, _: &NodeHandle, _: &item::Handle) -> Result<()> {
        self.inner
            .not_ported
            .skip("PlayerStatsCounter.incrementRareDrop (statistics database not ported)");
        Ok(())
    }
    fn ai_add_item(&mut self, _: &Ground) -> Result<()> {
        refused("AIRepository.addItem (no artificial players)")
    }
    fn bot_script_running(&mut self, _: &NodeHandle) -> Result<bool> {
        // getAttribute("botting:script", null): no bot runs here.
        Ok(false)
    }
    fn bot_add_drop(&mut self, _: &NodeHandle, _: &Ground) -> Result<()> {
        refused("botting:drops (no artificial players)")
    }
}

impl npc_death::Host for Live {
    fn is_npc(&mut self, entity: &NodeHandle) -> bool {
        self.inner.is_npc_node(entity.node)
    }
    fn identity_hash(&mut self, entity: &NodeHandle) -> i32 {
        entity.node as i32
    }
    fn impact_log(&mut self, entity: &NodeHandle) -> Result<Shared<ImpactLog>> {
        if let Some(log) = self.inner.combat.death_logs.get(&entity.node) {
            return Ok(log.clone());
        }
        let p = self.handle(entity);
        let entry = self.inner.combat_entry(&p);
        let mut log = ImpactLog::default();
        log.disabled_ticks = entry.impact.disabled_ticks;
        let players = entry.impact.player_order();
        for uid in players {
            if let Some((_, hit)) = entry.impact.player_log.iter().find(|(u, _)| *u == uid) {
                log.add_player(uid, *hit);
            }
        }
        let order = entry.impact.npc_order().map_err(binding)?;
        let npcs: Vec<_> = order
            .into_iter()
            .filter_map(|i| entry.impact.npc_log.get(i).cloned())
            .collect();
        for (source, hash, hit) in npcs {
            log.add_npc(source.map(|s| s.0), hash, hit)?;
        }
        let log = Shared::new(log);
        self.inner
            .combat
            .death_logs
            .insert(entity.node, log.clone());
        Ok(log)
    }
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<NodeHandle>> {
        Ok(self
            .inner
            .players
            .values()
            .find(|s| s.uid == uid && s.active)
            .map(|s| s.canonical.owner.clone()))
    }
    fn set_attribute(&mut self, entity: &NodeHandle, key: &str, value: Attribute) -> Result<()> {
        let value = match value {
            Attribute::Bool(v) => Attr::Bool(v),
            Attribute::Int(v) => Attr::Int(v),
        };
        self.inner.set_attribute(entity.node, key, value);
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &NodeHandle, key: &str) -> Result<()> {
        self.inner.remove_attribute(entity.node, key);
        Ok(())
    }
    fn attribute_bool(&mut self, entity: &NodeHandle, key: &str, fallback: bool) -> Result<bool> {
        match self.inner.attribute(entity.node, key) {
            None => Ok(fallback),
            Some(Attr::Bool(v)) => Ok(v),
            Some(_) => Err(Error::Type),
        }
    }
    fn attribute_int(&mut self, entity: &NodeHandle, key: &str, fallback: i32) -> Result<i32> {
        match self.inner.attribute(entity.node, key) {
            None => Ok(fallback),
            Some(Attr::Int(v)) => Ok(v),
            Some(_) => Err(Error::Type),
        }
    }
    fn increment_attribute(&mut self, player: &NodeHandle, key: &str) -> Result<()> {
        let value = match self.inner.attribute(player.node, key) {
            None => 0,
            Some(Attr::Int(v)) => v,
            Some(_) => return Err(Error::Type),
        };
        self.inner
            .set_attribute(player.node, key, Attr::Int(value.wrapping_add(1)));
        Ok(())
    }
    fn run_strong(&mut self, task: &Task) -> Result<()> {
        // PulseManager.run(pulse, STRONG): drop what is not running, return
        // while a strong pulse runs, stop every other pulse, then put, start
        // (DeathTask.start again) and submit.
        let node = task.entity.node;
        let manager = self
            .inner
            .state
            .actor(node)
            .map_err(binding)?
            .manager
            .clone();
        for (kind, pulse) in manager.entries() {
            if let Some(pulse) = pulse
                && !pulse.read().running
            {
                manager.put(kind, None);
            }
        }
        if manager
            .get(Some(pulses::Kind::Strong))
            .is_some_and(|p| p.read().running)
        {
            return Ok(());
        }
        self.with_walk(|world| world.clear_standard(node, manager.clone()))
            .map_err(binding)?;
        self.with_walk(|world| world.clear_pulses(node))
            .map_err(binding)?;
        manager.put(Some(pulses::Kind::Strong), Some(task.pulse.clone()));
        npc_death::task_start(self, task)?;
        if task.pulse.read().running {
            self.driver
                .pulser()
                .submit(Some(task.pulse.clone()))
                .map_err(binding)?;
            self.inner
                .combat
                .submitted
                .push((task.pulse.clone(), Submitted::Death(task.clone())));
        }
        Ok(())
    }
    fn stop_combat_pulse(&mut self, entity: &NodeHandle) -> Result<()> {
        let p = self.handle(entity);
        crate::world::combat_pulse::pulse::stop(self, &p).map_err(binding)
    }
    fn zone_start_death(
        &mut self,
        entity: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        let monitor = self.monitor(entity);
        monitor
            .start_death(self, Some(entity), killer)
            .map_err(binding)
    }
    fn zone_handle_death(&mut self, entity: &NodeHandle, killer: &NodeHandle) -> Result<bool> {
        let monitor = self.monitor(entity);
        monitor.handle_death(self, Some(killer)).map_err(binding)
    }
    fn walking_reset(&mut self, entity: &NodeHandle) -> Result<()> {
        let p = self.handle(entity);
        self.walk_reset(&p).map_err(binding)
    }
    fn lock(&mut self, entity: &NodeHandle, ticks: i32) -> Result<()> {
        let state = &mut self.inner.state;
        let locks = state.locks(entity).map_err(binding)?;
        locks
            .lock(ticks, state, &mut crate::world::action_locks::Clock)
            .map_err(binding)
    }
    fn unlock(&mut self, entity: &NodeHandle) -> Result<()> {
        self.inner.state.locks(entity).map_err(binding)?.unlock();
        Ok(())
    }
    fn face(&mut self, entity: &NodeHandle, target: Option<&NodeHandle>) -> Result<()> {
        let p = self.handle(entity);
        let target = target.map(|t| self.handle(t));
        self.entity_face(&p, target.as_ref()).map_err(binding)
    }
    fn play_global_audio(&mut self, location: Option<&Location>, id: i32) -> Result<()> {
        let location = location.cloned().ok_or(Error::Null)?;
        self.global_audio(location, id).map_err(binding)
    }
    fn reset_graphics(&mut self, entity: &NodeHandle) -> Result<()> {
        let p = self.handle(entity);
        let reset = self.walk.animations.factory.resets().graphics_reset();
        self.entity_visualize(&p, None, Some(reset))
            .map_err(binding)
    }
    fn visualize_death(&mut self, entity: &NodeHandle) -> Result<()> {
        let p = self.handle(entity);
        let (animation, graphics) = self.death_visuals(&p)?;
        self.entity_visualize(&p, animation, graphics)
            .map_err(binding)
    }
    fn force_death_animation(&mut self, entity: &NodeHandle) -> Result<()> {
        let p = self.handle(entity);
        let (animation, _) = self.death_visuals(&p)?;
        self.force_animation_of(&p, animation)
    }
    fn force_reset_animation(&mut self, entity: &NodeHandle) -> Result<()> {
        let p = self.handle(entity);
        let reset = self.walk.animations.factory.resets().animator_reset();
        self.force_animation_of(&p, Some(reset))
    }
    fn death_animation_duration(&mut self, entity: &NodeHandle) -> Result<i32> {
        let p = self.handle(entity);
        let (animation, _) = self.death_visuals(&p)?;
        let animation = animation.ok_or(Error::Null)?;
        Ok(animation.duration(&mut self.walk.animations))
    }
    fn respawn_location(&mut self, entity: &NodeHandle) -> Result<Option<Location>> {
        // properties.isSafeZone() ? safeRespawn : getSpawnLocation().
        if let Some(row) = self.inner.npcs.rows.get(&entity.node) {
            return Ok(row.properties.spawn.clone());
        }
        let p = self.handle(entity);
        self.inner
            .with_profile(&p, |profile| {
                if profile.properties.safe_zone {
                    profile.properties.safe_respawn.clone()
                } else {
                    profile.properties.spawn_location.clone()
                }
            })
            .map_err(binding)
    }
    fn set_teleport_location(
        &mut self,
        entity: &NodeHandle,
        location: Option<&Location>,
    ) -> Result<()> {
        let actor = self.inner.state.actor(entity.node).map_err(binding)?;
        actor.queue.state.write().entity.teleport_location = location.cloned();
        Ok(())
    }
    fn dispatch_self_death(&mut self, _: &NodeHandle, _: &NodeHandle) -> Result<()> {
        self.inner
            .not_ported
            .skip("SelfDeathEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn player_commence_death(&mut self, player: &NodeHandle, killer: &NodeHandle) -> Result<()> {
        super::player_death::commence(self, player, killer)
    }
    fn player_finalize_death(&mut self, player: &NodeHandle, killer: &NodeHandle) -> Result<()> {
        super::player_death::finalize(self, player, killer)
    }
    fn skills_restore(&mut self, entity: &NodeHandle) -> Result<()> {
        let p = self.handle(entity);
        if self.inner.is_npc_node(entity.node) {
            restore_npc_skills(self, &p);
            return Ok(());
        }
        self.with_skills(&p, |skills, ctx| skills.restore(ctx))
            .map_err(binding)?
            .map_err(binding)
    }
    fn recharge_prayer(&mut self, entity: &NodeHandle) -> Result<()> {
        if self.inner.is_npc_node(entity.node) {
            return Ok(());
        }
        let p = self.handle(entity);
        self.with_skills(&p, |skills, ctx| skills.recharge_prayer(ctx))
            .map_err(binding)?
            .map_err(binding)
    }
    fn clear_impact_queue(&mut self, entity: &NodeHandle) -> Result<()> {
        let p = self.handle(entity);
        self.inner.combat_entry(&p).impact.queue.clear();
        Ok(())
    }
    fn timers_on_death(&mut self, entity: &NodeHandle) -> Result<()> {
        // TimerManager.onEntityDeath: the ClearOnDeath timers are removed.
        let p = self.handle(entity);
        if self.inner.is_npc_node(entity.node) {
            return self.npc_timers_on_death(&p).map_err(binding);
        }
        self.with_timers(&p, |natives, manager| manager.death(&mut (), natives))
            .map_err(binding)
    }
    fn transform(&mut self, npc: &NodeHandle, id: i32) -> Result<()> {
        // NPC.transform(id) (world/live/runtime_npcs.rs).
        self.npc_transform(npc.node, id).map_err(binding)
    }
    fn npc_state(&mut self, npc: &NodeHandle) -> Result<npc::State> {
        self.inner
            .npcs
            .rows
            .get(&npc.node)
            .map(|r| r.brain.clone())
            .ok_or(Error::Null)
    }
    fn npc_definition(&mut self, npc: &NodeHandle) -> Result<Option<definitions::Handle>> {
        Ok(self
            .inner
            .npcs
            .rows
            .get(&npc.node)
            .map(|r| r.definition.clone()))
    }
    fn npc_original_id(&mut self, npc: &NodeHandle) -> Result<i32> {
        // originalId: kept from the NPC's first transform (until then its
        // id is the original).
        if let Some(original) = self.content.spawning.original.get(&npc.node) {
            return Ok(*original);
        }
        drop::Host::npc_id(self, npc)
    }
    fn npc_respawns(&mut self, npc: &NodeHandle) -> Result<bool> {
        // isRespawn(): true unless content cleared it.
        let respawn = !matches!(
            self.inner.attribute(npc.node, "npc:no-respawn"),
            Some(Attr::Bool(true))
        );
        Ok(respawn)
    }
    fn npc_clear(&mut self, npc: &NodeHandle) -> Result<()> {
        // NPC.clear() (world/live/runtime_npcs.rs).
        let npc = self.handle(npc).0;
        Live::npc_clear(self, &npc).map_err(binding)
    }
    fn on_death_started(&mut self, npc: &NodeHandle, killer: &NodeHandle) -> Result<()> {
        let (npc, killer) = (self.handle(npc), self.handle(killer));
        let behaviour = crate::world::npc_behaviour::behaviour(self, &npc).map_err(binding)?;
        behaviour
            .on_death_started(self, &npc, &killer)
            .map_err(binding)
    }
    fn on_death_finished(&mut self, npc: &NodeHandle, killer: &NodeHandle) -> Result<()> {
        let (npc, killer) = (self.handle(npc), self.handle(killer));
        let behaviour = crate::world::npc_behaviour::behaviour(self, &npc).map_err(binding)?;
        behaviour
            .on_death_finished(self, &npc, &killer)
            .map_err(binding)
    }
    fn increment_kills(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        self.inner
            .not_ported
            .skip("PlayerStatsCounter.incrementKills (statistics database not ported)");
        Ok(())
    }
    fn dispatch_npc_kill(&mut self, _: &NodeHandle, _: &NodeHandle) -> Result<()> {
        self.inner
            .not_ported
            .skip("NPCKillEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
}

impl Live {
    /// `properties.getDeathAnimation()` and `deathGfx`.
    fn death_visuals(
        &self,
        p: &P,
    ) -> Result<(
        Option<crate::world::animation::Animation>,
        Option<crate::world::animation::Graphics>,
    )> {
        if let Some(row) = self.npc_row(p) {
            let animation = row.properties.animations[2]
                .clone()
                .map(crate::world::animation::Animation::from_shared);
            let graphics = row.properties.death_graphics.as_ref().map(|g| {
                let g = g.read();
                crate::world::animation::Graphics::new(g.id, g.height, g.delay)
            });
            return Ok((animation, graphics));
        }
        self.inner
            .with_profile(p, |profile| {
                (
                    profile.properties.death_animation.clone(),
                    profile.properties.death_gfx.clone(),
                )
            })
            .map_err(binding)
    }
    fn force_animation_of(
        &mut self,
        p: &P,
        animation: Option<crate::world::animation::Animation>,
    ) -> Result<()> {
        if self.inner.is_npc_node(p.node()) {
            return self
                .npc_force_animation(p.node(), animation)
                .map_err(binding);
        }
        let animator = self.inner.canonical(p).animator.clone();
        animator
            .force_animation(&mut self.walk.animations, animation)
            .map_err(binding)
    }
}

/// The death configuration properties npc_death reads, re-exported for the
/// boot's checks.
pub fn has_drop_tables(definition: &definitions::Handle) -> bool {
    definition.read().drop_tables.is_some()
}

#[allow(dead_code)]
fn property_is_null(value: Option<&Property>) -> bool {
    matches!(value, None | Some(Property::Null))
}

#[allow(dead_code)]
fn unused(_: &mut Live) -> ground::Result<()> {
    Err(gbinding("unused"))
}
