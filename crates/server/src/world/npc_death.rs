//! NPC death as far as it leads to the drop: `DeathTask`, the death and kill
//! credit parts of `ImpactHandler`, `Entity.startDeath`/`finalizeDeath` and
//! `NPC.commenceDeath`/`finalizeDeath`/`handleDrops`.
//!
//! The task runs as a strong pulse on the native [`pulses`] manager and runner,
//! the respawn fields are the native [`npc::State`]'s, the respawn delay, the
//! death sound and the drop tables come from the native NPC definition, and
//! the drop is [`drop::drop`]. Everything else is a required service of
//! [`Host`], called in the original order. A dying player's own
//! `commenceDeath`/`finalizeDeath` and `DeathTask.getContainers` are not
//! ported.
use super::{doors::Location, drop_tables::drop, npc, player_walk::NodeHandle};
use crate::{
    definitions::{item::Shared, npc as definitions, property::Property},
    pulses,
};
use rs09_game_core::java_map::int_key_layout;

pub use drop::{Error, Result};

pub const STATE_DEATH: &str = "state:death";
pub const TICK_DEATH: &str = "tick:death";
pub const COMBAT_TIME: &str = "combat-time";
pub const DISABLE_DROP: &str = "disable:drop";
/// `"/save:" + STATS_BASE + ":" + STATS_ENEMIES_KILLED`.
pub const ENEMIES_KILLED: &str = "/save:stats_manager:enemies_killed";
/// `NPCConfigParser.RESPAWN_DELAY` and its default.
pub const RESPAWN_DELAY: &str = "respawn_delay";
pub const DEFAULT_RESPAWN_DELAY: i32 = 17;
/// `NPCConfigParser.COMBAT_AUDIO`; the death sound is its third entry.
pub const COMBAT_AUDIO: &str = "combat_audio";
/// The class name of the pulse `NodeTask.schedule` builds.
pub const PULSE_CLASS: &str = "core.game.system.task.NodeTask$1";

/// A value `DeathTask` stores in the entity's attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attribute {
    Bool(bool),
    Int(i32),
}

/// `ImpactHandler`'s two damage logs and its disabled-until tick.
///
/// Both logs are `HashMap`s in the original and kill credit walks them in
/// iteration order, so that order is kept: players by account uid, NPCs by
/// the identity hash the binding gives for each source.
pub struct ImpactLog {
    players: Vec<(i32, i32)>,
    player_capacity: usize,
    npcs: Vec<(Option<NodeHandle>, i32, i32)>,
    npc_capacity: usize,
    /// `disabledTicks`: the tick until which impacts are ignored.
    pub disabled_ticks: i32,
}
impl Default for ImpactLog {
    fn default() -> Self {
        Self {
            players: Vec::new(),
            player_capacity: 16,
            npcs: Vec::new(),
            npc_capacity: 16,
            disabled_ticks: 0,
        }
    }
}
impl ImpactLog {
    /// `playerImpactLog.put(uid, value == null ? hit : hit + value)`.
    pub fn add_player(&mut self, uid: i32, hit: i32) {
        match self.players.iter_mut().find(|(key, _)| *key == uid) {
            Some((_, value)) => *value = hit.wrapping_add(*value),
            None => {
                self.players.push((uid, hit));
                let keys = self.players.iter().map(|(key, _)| *key);
                self.player_capacity = int_key_layout(keys, self.player_capacity).1;
            }
        }
    }
    /// `npcImpactLog.put(source, value == null ? hit : hit + value)`. `hash`
    /// is the source's `hashCode()` (0 for a null source). Two different
    /// sources with one hash are refused: their order is not modelled.
    pub fn add_npc(&mut self, source: Option<NodeHandle>, hash: i32, hit: i32) -> Result<()> {
        if let Some((_, _, value)) = self.npcs.iter_mut().find(|(key, _, _)| *key == source) {
            *value = hit.wrapping_add(*value);
            return Ok(());
        }
        if self.npcs.iter().any(|(_, other, _)| *other == hash) {
            return Err(Error::Binding("identity hash collision".into()));
        }
        self.npcs.push((source, hash, hit));
        let keys = self.npcs.iter().map(|(_, hash, _)| *hash);
        self.npc_capacity = int_key_layout(keys, self.npc_capacity).1;
        Ok(())
    }
    /// `getPlayerImpactLog().size()`.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }
    /// The player log in the original's iteration order.
    pub fn players(&self) -> Vec<(i32, i32)> {
        let keys = self.players.iter().map(|(key, _)| *key);
        int_key_layout(keys, self.player_capacity)
            .0
            .into_iter()
            .filter_map(|uid| self.players.iter().find(|(key, _)| *key == uid).copied())
            .collect()
    }
    /// The NPC log in the original's iteration order.
    pub fn npcs(&self) -> Vec<(Option<NodeHandle>, i32)> {
        let keys = self.npcs.iter().map(|(_, hash, _)| *hash);
        int_key_layout(keys, self.npc_capacity)
            .0
            .into_iter()
            .filter_map(|hash| {
                self.npcs
                    .iter()
                    .find(|(_, other, _)| *other == hash)
                    .map(|(source, _, amount)| (source.clone(), *amount))
            })
            .collect()
    }
    /// `getNpcImpactLog().clear()`: the table keeps its size.
    pub fn clear_npcs(&mut self) {
        self.npcs.clear();
    }
    /// `getPlayerImpactLog().clear()`: the table keeps its size.
    pub fn clear_players(&mut self) {
        self.players.clear();
    }
}

/// The pulse `new DeathTask().schedule(entity, killer)` returns, with the two
/// nodes it captured.
#[derive(Clone)]
pub struct Task {
    pub pulse: pulses::Handle,
    pub entity: NodeHandle,
    pub killer: NodeHandle,
}

/// Services of systems that are not ported. Each method is one original call.
pub trait Host: drop::Host {
    /// `entity instanceof NPC`.
    fn is_npc(&mut self, entity: &NodeHandle) -> bool;
    /// `source.hashCode()` of a non-player damage source: a platform input.
    fn identity_hash(&mut self, entity: &NodeHandle) -> i32;
    /// The entity's `ImpactHandler` logs.
    fn impact_log(&mut self, entity: &NodeHandle) -> Result<Shared<ImpactLog>>;
    /// `Repository.getPlayerByUid(uid)`.
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<NodeHandle>>;

    /// `entity.setAttribute(key, value)`.
    fn set_attribute(&mut self, entity: &NodeHandle, key: &str, value: Attribute) -> Result<()>;
    /// `entity.removeAttribute(key)`.
    fn remove_attribute(&mut self, entity: &NodeHandle, key: &str) -> Result<()>;
    /// `entity.getAttribute(key, fallback)` for a Boolean.
    fn attribute_bool(&mut self, entity: &NodeHandle, key: &str, fallback: bool) -> Result<bool>;
    /// `entity.getAttribute(key, fallback)` for an Integer.
    fn attribute_int(&mut self, entity: &NodeHandle, key: &str, fallback: i32) -> Result<i32>;
    /// `player.incrementAttribute(key)`.
    fn increment_attribute(&mut self, player: &NodeHandle, key: &str) -> Result<()>;

    /// `entity.getPulseManager().run(pulse, PulseType.STRONG)`. The binding
    /// dispatches the pulse's `start`, `pulse`, `stop` and `removeFor` to
    /// [`task_start`], [`task_pulse`], [`task_stop`] and [`TASK_REMOVE_FOR`].
    fn run_strong(&mut self, task: &Task) -> Result<()>;
    /// `entity.getProperties().getCombatPulse().stop()`.
    fn stop_combat_pulse(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `zoneMonitor.startDeath(entity, killer)`.
    fn zone_start_death(
        &mut self,
        entity: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> Result<bool>;
    /// `getZoneMonitor().handleDeath(killer)`.
    fn zone_handle_death(&mut self, entity: &NodeHandle, killer: &NodeHandle) -> Result<bool>;

    /// `entity.getWalkingQueue().reset()`.
    fn walking_reset(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.lock(ticks)`.
    fn lock(&mut self, entity: &NodeHandle, ticks: i32) -> Result<()>;
    /// `entity.unlock()`.
    fn unlock(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.face(target)`.
    fn face(&mut self, entity: &NodeHandle, target: Option<&NodeHandle>) -> Result<()>;
    /// `playGlobalAudio(location, id)`.
    fn play_global_audio(&mut self, location: Option<&Location>, id: i32) -> Result<()>;
    /// `entity.graphics(Animator.RESET_G)`.
    fn reset_graphics(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.visualize(properties.getDeathAnimation(), properties.deathGfx)`.
    fn visualize_death(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.getAnimator().forceAnimation(properties.getDeathAnimation())`.
    fn force_death_animation(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.getAnimator().forceAnimation(Animator.RESET_A)`.
    fn force_reset_animation(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `entity.getProperties().getDeathAnimation().getDuration()`.
    fn death_animation_duration(&mut self, entity: &NodeHandle) -> Result<i32>;
    /// `properties.isSafeZone() ? properties.safeRespawn : properties.getSpawnLocation()`.
    fn respawn_location(&mut self, entity: &NodeHandle) -> Result<Option<Location>>;
    /// `entity.getProperties().setTeleportLocation(location)`.
    fn set_teleport_location(
        &mut self,
        entity: &NodeHandle,
        location: Option<&Location>,
    ) -> Result<()>;
    /// `entity.dispatch(new SelfDeathEvent(killer))`.
    fn dispatch_self_death(&mut self, entity: &NodeHandle, killer: &NodeHandle) -> Result<()>;

    /// `Player.commenceDeath(killer)`, not ported.
    fn player_commence_death(&mut self, player: &NodeHandle, killer: &NodeHandle) -> Result<()>;
    /// `Player.finalizeDeath(killer)`, not ported.
    fn player_finalize_death(&mut self, player: &NodeHandle, killer: &NodeHandle) -> Result<()>;

    /// `skills.restore()`.
    fn skills_restore(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `skills.rechargePrayerPoints()`.
    fn recharge_prayer(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `impactHandler.getImpactQueue().clear()`.
    fn clear_impact_queue(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `timers.onEntityDeath()`.
    fn timers_on_death(&mut self, entity: &NodeHandle) -> Result<()>;
    /// `asNpc().transform(id)`.
    fn transform(&mut self, npc: &NodeHandle, id: i32) -> Result<()>;

    /// The NPC's tick state, which holds `respawnTick` and `isRespawning`.
    fn npc_state(&mut self, npc: &NodeHandle) -> Result<npc::State>;
    /// The NPC's `definition` field.
    fn npc_definition(&mut self, npc: &NodeHandle) -> Result<Option<definitions::Handle>>;
    /// `originalId`.
    fn npc_original_id(&mut self, npc: &NodeHandle) -> Result<i32>;
    /// `isRespawn()`.
    fn npc_respawns(&mut self, npc: &NodeHandle) -> Result<bool>;
    /// `NPC.clear()`.
    fn npc_clear(&mut self, npc: &NodeHandle) -> Result<()>;
    /// `behavior.onDeathStarted(npc, killer)`.
    fn on_death_started(&mut self, npc: &NodeHandle, killer: &NodeHandle) -> Result<()>;
    /// `behavior.onDeathFinished(npc, killer)`.
    fn on_death_finished(&mut self, npc: &NodeHandle, killer: &NodeHandle) -> Result<()>;
    /// `PlayerStatsCounter.incrementKills(player, npcId)`.
    fn increment_kills(&mut self, player: &NodeHandle, npc_id: i32) -> Result<()>;
    /// `killer.dispatch(new NPCKillEvent(npc))`.
    fn dispatch_npc_kill(&mut self, killer: &NodeHandle, npc: &NodeHandle) -> Result<()>;
}

fn now<H: Host>(host: &mut H) -> Result<i32> {
    Ok(host.ticks()?)
}

/// `ImpactHandler.setDisabledTicks(ticks)`.
pub fn set_disabled_ticks<H: Host>(host: &mut H, entity: &NodeHandle, ticks: i32) -> Result<()> {
    let log = host.impact_log(entity)?;
    let until = now(host)?.wrapping_add(ticks);
    log.write().disabled_ticks = until;
    Ok(())
}

/// The statement of `handleImpact` that writes the damage logs: a positive
/// hit is added under the player's uid, or under the source itself.
pub fn log_hit<H: Host>(
    host: &mut H,
    entity: &NodeHandle,
    source: Option<&NodeHandle>,
    hit: i32,
) -> Result<()> {
    if hit <= 0 {
        return Ok(());
    }
    let log = host.impact_log(entity)?;
    match source {
        Some(player) if host.is_player(player) => {
            let uid = host.uid(player)?;
            log.write().add_player(uid, hit);
        }
        _ => {
            let hash = match source {
                Some(source) => host.identity_hash(source),
                None => 0,
            };
            log.write().add_npc(source.cloned(), hash, hit)?;
        }
    }
    Ok(())
}

/// `ImpactHandler.getMostDamageEntity(killer)`: who a kill is credited to.
///
/// A dying player and an artificial killer keep the hitter. Otherwise, with no
/// player in the log, it is the non-player source with the most damage, or the
/// dying entity itself; with players in the log it is whoever
/// `getPlayerByUid` finds for the uid with the most damage, which is nobody
/// if that player has left. Ties go to the first in map order.
pub fn most_damage_entity<H: Host>(
    host: &mut H,
    entity: &NodeHandle,
    killer: Option<&NodeHandle>,
) -> Result<Option<NodeHandle>> {
    if host.is_player(entity) {
        return Ok(killer.cloned());
    }
    if let Some(killer) = killer {
        if host.is_ai_player(killer) {
            return Ok(Some(killer.clone()));
        }
    }
    let log = host.impact_log(entity)?;
    let players = log.read().players();
    let mut damage = -1;
    if players.is_empty() {
        let mut credited = Some(entity.clone());
        for (source, amount) in log.read().npcs() {
            if source.as_ref() == Some(entity) {
                continue;
            }
            if amount > damage {
                damage = amount;
                credited = source;
            }
        }
        return Ok(credited);
    }
    let mut player = 0;
    for (uid, amount) in players {
        if amount > damage {
            damage = amount;
            player = uid;
        }
    }
    host.player_by_uid(player)
}

/// The branch of `handleImpact` taken when the hit leaves the entity without
/// lifepoints.
pub fn lethal_impact<H: Host>(
    host: &mut H,
    entity: &NodeHandle,
    source: Option<&NodeHandle>,
) -> Result<Option<Task>> {
    host.stop_combat_pulse(entity)?;
    host.face(entity, source)?;
    let killer = most_damage_entity(host, entity, source)?;
    entity_start_death(host, entity, killer.as_ref())
}

/// `Entity.startDeath(killer)`.
pub fn entity_start_death<H: Host>(
    host: &mut H,
    entity: &NodeHandle,
    killer: Option<&NodeHandle>,
) -> Result<Option<Task>> {
    if host.zone_start_death(entity, killer)? {
        return start_death(host, entity, killer);
    }
    Ok(None)
}

/// `DeathTask.isDead(entity)`.
pub fn is_dead<H: Host>(host: &mut H, entity: &NodeHandle) -> Result<bool> {
    if host.is_npc(entity) {
        let respawn_tick = host.npc_state(entity)?.fields.read().respawn_tick;
        if respawn_tick > now(host)? {
            return Ok(true);
        }
    }
    host.attribute_bool(entity, STATE_DEATH, false)
}

/// `DeathTask.startDeath(entity, killer)`. An entity without a killer is its
/// own.
///
/// `schedule` starts the pulse and the pulse manager starts it again, so
/// [`task_start`] runs twice for one death. If another strong pulse is already
/// current the manager returns before the second start and never submits the
/// pulse: the entity is left in its death state.
pub fn start_death<H: Host>(
    host: &mut H,
    entity: &NodeHandle,
    killer: Option<&NodeHandle>,
) -> Result<Option<Task>> {
    if is_dead(host, entity)? {
        return Ok(None);
    }
    let task = Task {
        pulse: Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(vec![Some(entity.node)])),
            pulses::Class::Other,
            PULSE_CLASS.into(),
        )),
        entity: entity.clone(),
        killer: killer.unwrap_or(entity).clone(),
    };
    task_start(host, &task)?;
    host.run_strong(&task)?;
    Ok(Some(task))
}

/// The pulse's `start()`: the base start, then `DeathTask.start`.
pub fn task_start<H: Host>(host: &mut H, task: &Task) -> Result<()> {
    pulses::start_base(&task.pulse);
    let (entity, killer) = (&task.entity, &task.killer);
    host.walking_reset(entity)?;
    host.set_attribute(entity, STATE_DEATH, Attribute::Bool(true))?;
    let ticks = now(host)?;
    host.set_attribute(entity, TICK_DEATH, Attribute::Int(ticks))?;
    host.lock(entity, 50)?;
    host.face(entity, None)?;
    let is_npc = host.is_npc(entity);
    if is_npc {
        host.remove_attribute(killer, COMBAT_TIME)?;
        if let Some(id) = death_audio(host, entity)? {
            let location = host.npc_location(entity)?;
            host.play_global_audio(location.as_ref(), id)?;
        }
    }
    host.reset_graphics(entity)?;
    host.visualize_death(entity)?;
    host.force_death_animation(entity)?;
    if is_npc {
        host.on_death_started(entity, killer)?;
    } else {
        host.player_commence_death(entity, killer)?;
    }
    set_disabled_ticks(host, entity, 50)
}

/// `NPC.getAudio(2)`: the third `combat_audio` entry unless it is 0.
fn death_audio<H: Host>(host: &mut H, npc: &NodeHandle) -> Result<Option<i32>> {
    let definition = host.npc_definition(npc)?.ok_or(Error::Null)?;
    let handlers = definition.read().handlers.clone();
    let audios = match handlers.read().get(COMBAT_AUDIO) {
        None | Some(Property::Null) => return Ok(None),
        Some(Property::IntArray(audios)) => audios.clone(),
        Some(_) => return Err(Error::Type),
    };
    let id = *audios.read().get(2).ok_or(Error::Index)?;
    Ok((id != 0).then_some(id))
}

/// The pulse's `pulse()`: `DeathTask.exec`. True once the death animation's
/// duration (six ticks when it is outside 1 to 30) has passed.
pub fn task_pulse<H: Host>(host: &mut H, task: &Task) -> Result<bool> {
    let mut ticks = host.death_animation_duration(&task.entity)?;
    if !(1..=30).contains(&ticks) {
        ticks = 6;
    }
    let started = host.attribute_int(&task.entity, TICK_DEATH, -1)?;
    Ok(started <= now(host)?.wrapping_sub(ticks))
}

/// The pulse's `removeFor(String)`: a death is never replaced.
pub const TASK_REMOVE_FOR: bool = false;

/// The pulse's `stop()`: the base stop, then `DeathTask.stop`.
pub fn task_stop<H: Host>(host: &mut H, task: &Task) -> Result<()> {
    task_stop_with(host, task, npc_finalize_death)
}

/// [`task_stop`] with the NPC's `finalizeDeath(killer)` given by the caller:
/// a host whose NPCs have subclasses overriding the member passes its
/// virtual dispatch (which reaches [`npc_finalize_death`] as `super`).
pub fn task_stop_with<H: Host>(
    host: &mut H,
    task: &Task,
    npc_finalize: impl FnOnce(&mut H, &NodeHandle, &NodeHandle) -> Result<()>,
) -> Result<()> {
    pulses::stop_base(&task.pulse);
    let (entity, killer) = (&task.entity, &task.killer);
    host.remove_attribute(entity, STATE_DEATH)?;
    host.remove_attribute(entity, TICK_DEATH)?;
    let spawn = host.respawn_location(entity)?;
    host.force_reset_animation(entity)?;
    host.set_teleport_location(entity, spawn.as_ref())?;
    host.unlock(entity)?;
    if host.is_npc(entity) {
        npc_finalize(host, entity, killer)?;
    } else {
        host.player_finalize_death(entity, killer)?;
    }
    let log = host.impact_log(entity)?;
    log.write().clear_npcs();
    log.write().clear_players();
    set_disabled_ticks(host, entity, 4)?;
    host.dispatch_self_death(entity, killer)
}

/// `Entity.finalizeDeath(killer)`, as an NPC reaches it.
pub fn entity_finalize_death<H: Host>(host: &mut H, entity: &NodeHandle) -> Result<()> {
    host.skills_restore(entity)?;
    host.recharge_prayer(entity)?;
    host.clear_impact_queue(entity)?;
    set_disabled_ticks(host, entity, 10)?;
    host.timers_on_death(entity)?;
    host.remove_attribute(entity, COMBAT_TIME)?;
    host.face(entity, None)?;
    // A Loar shade turns back into its shadow.
    let id = host.npc_id(entity)?;
    if id == 1240 || id == 1241 {
        host.transform(entity, 1240)?;
    }
    Ok(())
}

/// `NPC.finalizeDeath(killer)`.
pub fn npc_finalize_death<H: Host>(
    host: &mut H,
    npc: &NodeHandle,
    killer: &NodeHandle,
) -> Result<()> {
    entity_finalize_death(host, npc)?;
    if host.zone_handle_death(npc, killer)? {
        return Ok(());
    }
    let player = host.is_player(killer).then_some(killer);
    if let Some(player) = player {
        host.increment_attribute(player, ENEMIES_KILLED)?;
        let original_id = host.npc_original_id(npc)?;
        host.increment_kills(player, original_id)?;
    }
    handle_drops(host, npc, player, killer)?;
    if !host.npc_respawns(npc)? {
        host.npc_clear(npc)?;
    }
    let state = host.npc_state(npc)?;
    state.fields.write().respawning = true;
    host.on_death_finished(npc, killer)?;
    host.dispatch_npc_kill(killer, npc)?;
    let ticks = now(host)?;
    let definition = host.npc_definition(npc)?.ok_or(Error::Null)?;
    let handlers = definition.read().handlers.clone();
    let delay = match handlers.read().get(RESPAWN_DELAY) {
        None | Some(Property::Null) => DEFAULT_RESPAWN_DELAY,
        Some(Property::Int(delay)) => *delay,
        Some(_) => return Err(Error::Type),
    };
    state.fields.write().respawn_tick = ticks.wrapping_add(delay);
    Ok(())
}

/// `NPC.handleDrops(p, killer)`: nothing drops with `disable:drop` set, for an
/// ironman killer when more than one player damaged the NPC, or without drop
/// tables on the definition.
pub fn handle_drops<H: Host>(
    host: &mut H,
    npc: &NodeHandle,
    player: Option<&NodeHandle>,
    killer: &NodeHandle,
) -> Result<()> {
    if host.attribute_bool(npc, DISABLE_DROP, false)? {
        return Ok(());
    }
    if let Some(player) = player {
        if host.ironman(player)? && host.impact_log(npc)?.read().player_count() > 1 {
            return Ok(());
        }
    }
    let Some(definition) = host.npc_definition(npc)? else {
        return Ok(());
    };
    let tables = definition.read().drop_tables.clone();
    let Some(tables) = tables else {
        return Ok(());
    };
    drop::drop(host, &tables, npc, Some(killer))
}
