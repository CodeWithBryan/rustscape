//! What happens around and after a combat swing.
//!
//! Ports the rest of the pinned `CombatSwingHandler` and its melee, range and
//! magic subclasses (`handler`), the `CombatPulse` state machine with the
//! pulses it submits (`pulse`) and `ImpactHandler` (`impact`). The swing
//! calculation itself stays in [`crate::world::combat_swing`].
//!
//! This is a library. State a pulse or an impact handler owns lives in
//! [`PulseState`] and [`ImpactState`]; the host keeps one of each per entity
//! and hands them out through plain accessors, so no borrow of that state is
//! ever held across a service call. Everything the original reaches outside
//! these classes is a required [`Host`] service called in the original order.
//! None has a default. Nothing in a session calls this yet. See
//! docs/combat-pulse.md.
pub mod handler;
pub mod impact;
pub mod pulse;

use crate::world::combat_swing::{
    self as cs, ArmourSet, BattleState, BoltEffect, CombatStyle, Error, Graphics, Result,
    WeaponInterfaces,
};
pub use impact::{HitsplatType, Impact, ImpactState};
pub use pulse::{PulseState, Task, WorldPulse};

/// `EquipmentContainer` slot numbers read here.
pub mod slot {
    pub const HAT: i32 = 0;
    pub const CAPE: i32 = 1;
    pub const WEAPON: i32 = 3;
    pub const CHEST: i32 = 4;
    pub const SHIELD: i32 = 5;
    pub const RING: i32 = 12;
    pub const ARROWS: i32 = 13;
}

/// `Skills` slot numbers read here.
pub mod skill {
    pub const ATTACK: i32 = 0;
    pub const DEFENCE: i32 = 1;
    pub const STRENGTH: i32 = 2;
    pub const HITPOINTS: i32 = 3;
    pub const RANGE: i32 = 4;
    pub const MAGIC: i32 = 6;
    pub const AGILITY: i32 = 16;
}

/// The prayers these classes query besides the ones the swing slice reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prayer {
    Smite,
    Redemption,
}

impl Prayer {
    pub fn name(self) -> &'static str {
        match self {
            Self::Smite => "SMITE",
            Self::Redemption => "REDEMPTION",
        }
    }
}

/// The fields of an `Ammunition` row `RangeSwingHandler.visualize` reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AmmunitionVisuals {
    pub start_graphics: Option<Graphics>,
    pub dark_bow_graphics: Option<Graphics>,
    /// `projectile.projectileId`; `None` is a null projectile.
    pub projectile_id: Option<i32>,
}

/// One `Projectile` factory call followed by `send()`.
pub enum ProjectileRequest<'a, H: Host> {
    /// `ammunition.projectile.copy(source, victim, speed)`.
    AmmunitionCopy {
        ammunition_id: i32,
        source: &'a H::Entity,
        victim: Option<&'a H::Entity>,
        speed: f64,
    },
    /// `Projectile.create(source, victim, id, startHeight, endHeight, startDelay, speed, angle)`.
    Create {
        source: &'a H::Entity,
        victim: Option<&'a H::Entity>,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        speed: i32,
        angle: i32,
    },
    /// `Projectile.ranged(source, victim, id, startHeight, endHeight, startDelay, angle)`.
    Ranged {
        source: &'a H::Entity,
        victim: Option<&'a H::Entity>,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        angle: i32,
    },
}

/// A handler's private `specialHandlers` map. Only lookups and insertions
/// happen, so iteration order is never observed.
pub struct Specials<H: Host> {
    /// `None` until the first `register` or `getSpecial` call creates the map.
    pub entries: Option<Vec<(i32, H::HandlerRef)>>,
}

impl<H: Host> Default for Specials<H> {
    fn default() -> Self {
        Self { entries: None }
    }
}

/// Everything the ported classes reach outside themselves. Each fallible
/// method is one original call; a failure leaves earlier effects in place.
/// The first group are plain accessors to state and class identity: they are
/// not service calls and are never recorded.
#[allow(clippy::type_complexity)]
pub trait Host: cs::Host {
    /// One `CombatSwingHandler` instance.
    type HandlerRef: Clone;
    /// One `Animation` object.
    type Animation: Clone;
    /// One `GroundItem` object.
    type Ground;

    /// The instance's `type` and `flags`, with its concrete class.
    fn handler(&mut self, handler: &Self::HandlerRef) -> &mut cs::Handler;
    /// `handler is SalamanderSwingHandler`.
    fn is_salamander(&self, handler: &Self::HandlerRef) -> bool;
    /// The instance's runtime class simple name.
    fn handler_class_name(&self, handler: &Self::HandlerRef) -> String;
    fn specials(&mut self, handler: &Self::HandlerRef) -> &mut Specials<Self>;
    /// Reference identity of two entities.
    fn same_entity(&self, a: &Self::Entity, b: &Self::Entity) -> bool;
    /// `entity instanceof AIPlayer`.
    fn is_ai_player(&self, entity: &Self::Entity) -> bool;
    /// `Object.hashCode()` of an entity: a platform input that orders the
    /// NPC impact log.
    fn identity_hash(&self, entity: &Self::Entity) -> i32;
    /// `properties.combatPulse`.
    fn combat_pulse(&mut self, entity: &Self::Entity) -> &mut PulseState<Self>;
    /// `getImpactHandler()`.
    fn impact_handler(&mut self, entity: &Self::Entity) -> &mut ImpactState<Self>;
    /// The static `CombatSwingHandler.EXPERIENCE_MOD`.
    fn experience_mod(&mut self) -> &mut f64;
    fn item_charge(&self, item: &Self::Item) -> i32;
    fn animation_id(&self, animation: &Self::Animation) -> i32;
    fn ammunition_visuals(&self, item_id: i32) -> AmmunitionVisuals;
    /// `RangeWeapon.get(id).animation`.
    fn range_weapon_animation(&self, item_id: i32) -> Option<Self::Animation>;

    /// `getAttribute(key, false)`.
    fn attribute_flag(&mut self, entity: &Self::Entity, key: &str) -> Result<bool>;
    /// `getAttribute("autocast_component", null) != null`.
    fn autocast_component(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `attributes.containsKey(key)`.
    fn has_attribute(&mut self, entity: &Self::Entity, key: &str) -> Result<bool>;
    fn remove_attribute(&mut self, entity: &Self::Entity, key: &str) -> Result<()>;
    /// `getAttribute(victim, "re-npc", null)`.
    fn random_event_npc(&mut self, entity: &Self::Entity) -> Result<Option<Self::Entity>>;
    /// `getAttribute("combat-attacker")`.
    fn combat_attacker(&mut self, entity: &Self::Entity) -> Result<Option<Self::Entity>>;
    /// `setAttribute("combat-time", millis)`.
    fn set_combat_time(&mut self, entity: &Self::Entity, millis: i64) -> Result<()>;
    /// `setAttribute("combat-attacker", attacker)`.
    fn set_combat_attacker(&mut self, entity: &Self::Entity, attacker: &Self::Entity)
    -> Result<()>;
    /// `setAttribute("combat-stop", tick)`.
    fn set_combat_stop(&mut self, entity: &Self::Entity, tick: i32) -> Result<()>;
    /// `setAttribute("fam-exp", true)`.
    fn set_fam_exp(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `getAttribute("duel:ammo", null) != null`.
    fn duel_ammo(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `(getAttribute("duel:ammo") as ArrayList).add(drop)`.
    fn duel_ammo_add(&mut self, entity: &Self::Entity, drop: Self::Ground) -> Result<()>;
    fn index(&mut self, entity: &Self::Entity) -> Result<i32>;
    fn client_index(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `Node.isActive()`.
    fn is_active(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `Node.getCenterLocation()`.
    fn center_location(&mut self, entity: &Self::Entity) -> Result<[i32; 3]>;
    /// `Entity.getClosestOccupiedTile(other)`.
    fn closest_occupied_tile(
        &mut self,
        entity: &Self::Entity,
        other: Option<[i32; 3]>,
    ) -> Result<Option<[i32; 3]>>;
    /// `Entity.getSwingHandler(swing)`.
    fn swing_handler(&mut self, entity: &Self::Entity, swing: bool) -> Result<Self::HandlerRef>;
    /// `victim.isAttackable(attacker, style, message)`.
    fn entity_attackable(
        &mut self,
        victim: &Self::Entity,
        attacker: &Self::Entity,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool>;
    fn face(&mut self, entity: &Self::Entity, target: Option<&Self::Entity>) -> Result<()>;
    /// `faceTemporary(target, ticks)`.
    fn face_temporary(
        &mut self,
        entity: &Self::Entity,
        target: Option<&Self::Entity>,
        ticks: i32,
    ) -> Result<()>;
    /// `properties.attackAnimation`.
    fn attack_animation(&mut self, entity: &Self::Entity) -> Result<Option<Self::Animation>>;
    /// `properties.defenceAnimation`.
    fn defence_animation(&mut self, entity: &Self::Entity) -> Result<Option<Self::Animation>>;
    /// `properties.getCombatAnimation(index)`.
    fn combat_animation(
        &mut self,
        entity: &Self::Entity,
        index: i32,
    ) -> Result<Option<Self::Animation>>;
    fn animate(&mut self, entity: &Self::Entity, animation: Option<&Self::Animation>)
    -> Result<()>;
    /// `Entity.visualize(animation, graphics)`.
    fn visualize_entity(
        &mut self,
        entity: &Self::Entity,
        animation: Option<&Self::Animation>,
        graphics: Option<Graphics>,
    ) -> Result<()>;
    /// `victim.onImpact(attacker, state)`.
    fn entity_on_impact(
        &mut self,
        victim: &Self::Entity,
        attacker: Option<&Self::Entity>,
        state: &mut BattleState<Self>,
    ) -> Result<()>;
    fn send_impact(&mut self, entity: &Self::Entity, state: &mut BattleState<Self>) -> Result<()>;
    fn check_impact(&mut self, entity: &Self::Entity, state: &mut BattleState<Self>) -> Result<()>;
    fn full_restore(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `Entity.getFormattedHit(state, hit)`.
    fn entity_formatted_hit(
        &mut self,
        entity: &Self::Entity,
        state: &BattleState<Self>,
        hit: i32,
    ) -> Result<f64>;
    /// `familiarManager.adjustBattleState(state)`.
    fn familiar_adjust_battle_state(
        &mut self,
        entity: &Self::Entity,
        state: &mut BattleState<Self>,
    ) -> Result<()>;
    /// `degrader.checkWeaponDegrades(player)`.
    fn check_weapon_degrades(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `prayer.get(type)`.
    fn prayer_active(&mut self, entity: &Self::Entity, prayer: Prayer) -> Result<bool>;
    fn prayer_points(&mut self, entity: &Self::Entity) -> Result<f64>;
    fn decrement_prayer_points(&mut self, entity: &Self::Entity, amount: f64) -> Result<()>;
    /// `properties.protectStyle`.
    fn protect_style(&mut self, entity: &Self::Entity) -> Result<Option<CombatStyle>>;
    /// `DegradableEquipment.degrade(player, victim, weapon)`.
    fn degrade(&mut self, player: &Self::Entity, victim: &Self::Entity, weapon: bool)
    -> Result<()>;
    /// `applyPoison(victim, source, severity)`.
    fn apply_poison(
        &mut self,
        victim: &Self::Entity,
        source: &Self::Entity,
        severity: i32,
    ) -> Result<()>;
    fn npc_poisonous(&mut self, entity: &Self::Entity) -> Result<bool>;
    fn npc_poison_severity(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `ironmanManager.isIronman`.
    fn is_ironman(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `Familiar.getOwner()`.
    fn familiar_owner(&mut self, entity: &Self::Entity) -> Result<Option<Self::Entity>>;
    /// `Familiar.getAttackStyle()`.
    fn familiar_attack_style(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `npc.behavior.getXpMultiplier(npc, player)`.
    fn npc_xp_multiplier(&mut self, npc: &Self::Entity, player: &Self::Entity) -> Result<f64>;
    /// `skills.addExperience(slot, experience, playerMod)`.
    fn add_experience(
        &mut self,
        entity: &Self::Entity,
        slot: i32,
        experience: f64,
        player_mod: bool,
    ) -> Result<()>;
    fn spell_id(&mut self, spell: &Self::Spell) -> Result<i32>;
    fn spell_fire_effect(
        &mut self,
        spell: &Self::Spell,
        entity: &Self::Entity,
        victim: Option<&Self::Entity>,
        state: &mut BattleState<Self>,
    ) -> Result<()>;
    fn spell_visualize(
        &mut self,
        spell: &Self::Spell,
        entity: &Self::Entity,
        victim: Option<&Self::Entity>,
    ) -> Result<()>;
    fn spell_visualize_impact(
        &mut self,
        spell: &Self::Spell,
        entity: Option<&Self::Entity>,
        victim: Option<&Self::Entity>,
        state: &mut BattleState<Self>,
    ) -> Result<()>;
    fn spell_add_experience(
        &mut self,
        spell: &Self::Spell,
        entity: &Self::Entity,
        hit: i32,
    ) -> Result<()>;
    /// `equipment.replace(item, slot)`; the item is `(id, amount, charge)`.
    fn equipment_replace(
        &mut self,
        entity: &Self::Entity,
        item: Option<(i32, i32, i32)>,
        slot: i32,
    ) -> Result<()>;
    /// `equipment.contains(id, amount)`.
    fn equipment_contains(&mut self, entity: &Self::Entity, id: i32, amount: i32) -> Result<bool>;
    /// `RegionManager.getClippingFlag`.
    fn clipping_flag(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
    /// `RegionManager.getProjectileFlag`.
    fn projectile_flag(&mut self, z: i32, x: i32, y: i32) -> Result<i32>;
    /// `GroundItemManager.increase(Item(id, amount), location, player)`.
    fn ground_item_increase(
        &mut self,
        id: i32,
        amount: i32,
        location: [i32; 3],
        player: &Self::Entity,
    ) -> Result<Option<Self::Ground>>;
    /// `item.definition.maleWornModelId1`.
    fn item_male_worn_model(&mut self, item: &Self::Item) -> Result<i32>;
    /// The item definition's configured attack audio array; `None` entries
    /// are null elements.
    fn item_attack_audios(&mut self, item: &Self::Item) -> Result<Option<Vec<Option<i32>>>>;
    /// `walkingQueue.isMoving`.
    fn is_moving(&mut self, entity: &Self::Entity) -> Result<bool>;
    fn is_running_both(&mut self, entity: &Self::Entity) -> Result<bool>;
    fn run_dir(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `walkingQueue.reset()`.
    fn walking_reset(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `locks.isMovementLocked`.
    fn movement_locked(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `locks.isInteractionLocked`.
    fn interaction_locked(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `interfaceManager.close(autocastComponent)`.
    fn close_component(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `interfaceManager.openTab(weaponInterface)`; `present` is false for null.
    fn open_weapon_tab(&mut self, entity: &Self::Entity, present: bool) -> Result<()>;
    /// `Entity.attack(victim)`.
    fn attack(&mut self, entity: &Self::Entity, victim: &Self::Entity) -> Result<()>;
    /// `WeaponInterface.updateInterface()`.
    fn update_weapon_interface(&mut self, entity: &Self::Entity) -> Result<()>;
    fn debug(&mut self, entity: &Self::Entity, text: &str) -> Result<()>;
    /// `WeaponInterface.getWeaponInterface()`.
    fn weapon_interfaces(&mut self, entity: &Self::Entity) -> Result<Option<WeaponInterfaces>>;
    /// `DeathTask.isDead(entity)`.
    fn is_dead(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `RegionManager.forId(regionId).isActive`.
    fn region_active(&mut self, region_id: i32) -> Result<bool>;
    /// `properties.combatTimeOut`.
    fn combat_time_out(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `properties.attackSpeed`.
    fn attack_speed(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `GameWorld.ticks`.
    fn ticks(&mut self) -> Result<i32>;
    /// `hasTimerActive<Miasmic>(entity)`.
    fn miasmic_active(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `npc.isHidden(player)`.
    fn npc_hidden(&mut self, npc: &Self::Entity, player: &Self::Entity) -> Result<bool>;
    /// `npc.canStartCombat(victim)`.
    fn npc_can_start_combat(&mut self, npc: &Self::Entity, victim: &Self::Entity) -> Result<bool>;
    /// The pulse's private `MovementPulse.updatePath()`.
    fn movement_update_path(&mut self, entity: &Self::Entity) -> Result<()>;
    /// The pulse's private `MovementPulse.setLast(null)`.
    fn movement_set_last(&mut self, entity: &Self::Entity) -> Result<()>;
    /// The pulse's private `MovementPulse.setDestination(victim)`.
    fn movement_set_destination(
        &mut self,
        entity: &Self::Entity,
        victim: Option<&Self::Entity>,
    ) -> Result<()>;
    /// `interfaceManager.close()`.
    fn interface_close(&mut self, entity: &Self::Entity) -> Result<()>;
    fn interface_close_chatbox(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `skullManager.isWilderness`.
    fn is_wilderness(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `skullManager.checkSkull(victim)`.
    fn check_skull(&mut self, entity: &Self::Entity, victim: &Self::Entity) -> Result<()>;
    /// `pulseManager.isMovingPulse`.
    fn pulse_manager_is_moving(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `pulseManager.clear()`.
    fn pulse_manager_clear(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `pulseManager.run(combatPulse)`. The manager calls [`pulse::start`]
    /// and queues the pulse when it is running afterwards.
    fn pulse_manager_run(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `System.currentTimeMillis()`.
    fn current_time_millis(&mut self) -> Result<i64>;
    /// `properties.combatLevel`.
    fn combat_level(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `Player.sendMessage(text)`.
    fn player_send_message(&mut self, entity: &Self::Entity, text: &str) -> Result<()>;
    /// `skills.updateLevel(slot, amount, maximum)`.
    fn update_level(
        &mut self,
        entity: &Self::Entity,
        slot: i32,
        amount: i32,
        maximum: i32,
    ) -> Result<i32>;
    fn on_attack(&mut self, entity: &Self::Entity, victim: &Self::Entity) -> Result<()>;
    /// `scripts.removeWeakScripts()`.
    fn remove_weak_scripts(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `properties.armourSet = set`.
    fn set_armour_set(&mut self, entity: &Self::Entity, set: Option<ArmourSet>) -> Result<()>;
    /// `GameWorld.Pulser.submit(pulse)`. The runner calls
    /// [`WorldPulse::update`] once a tick until it answers true.
    fn submit(&mut self, pulse: WorldPulse<Self>) -> Result<()>;
    /// `npc.behavior.beforeAttackFinalized(npc, victim, state)`.
    fn before_attack_finalized(
        &mut self,
        npc: &Self::Entity,
        victim: &Self::Entity,
        state: &mut BattleState<Self>,
    ) -> Result<()>;
    /// `npc.getAudio(index)`: the audio id, or `None` for null.
    fn npc_audio(&mut self, entity: &Self::Entity, index: i32) -> Result<Option<i32>>;
    /// `playHurtAudio(player)`.
    fn play_hurt_audio(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `settings.attackStyleIndex`.
    fn attack_style_index(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `npc.definition.combatGraphics`; `None` is a null array.
    fn npc_combat_graphics(
        &mut self,
        entity: &Self::Entity,
    ) -> Result<Option<Vec<Option<Graphics>>>>;
    fn send_projectile(&mut self, request: ProjectileRequest<'_, Self>) -> Result<()>;
    /// `skills.hit(damage)`: the damage that could not be dealt.
    fn skills_hit(&mut self, entity: &Self::Entity, damage: i32) -> Result<i32>;
    /// `skills.heal(amount)`.
    fn heal(&mut self, entity: &Self::Entity, amount: i32) -> Result<i32>;
    /// `skills.getDynamicLevels()`; `None` is a null array.
    fn dynamic_levels(&mut self, entity: &Self::Entity) -> Result<Option<Vec<i32>>>;
    /// `settings.updateRunEnergy(amount)`.
    fn update_run_energy(&mut self, entity: &Self::Entity, amount: f64) -> Result<()>;
    /// `details.uid`.
    fn uid(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `familiarManager.hasFamiliar()`.
    fn has_familiar(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `familiarManager.getFamiliar() instanceof Pet`.
    fn familiar_is_pet(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `Entity.startDeath(killer)`.
    fn start_death(&mut self, entity: &Self::Entity, killer: Option<&Self::Entity>) -> Result<()>;
    /// `Repository.getPlayerByUid(uid)`.
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<Self::Entity>>;
    /// `prayer.startRedemption()`.
    fn start_redemption(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `zoneMonitor.type`.
    fn zone_type(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `EnchantedJewellery.RING_OF_LIFE.attemptTeleport(player, Item(2570), 0, true, true)`.
    fn ring_of_life_teleport(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `savedData.globalData.recoilDamage`.
    fn recoil_damage(&mut self, entity: &Self::Entity) -> Result<i32>;
    fn set_recoil_damage(&mut self, entity: &Self::Entity, damage: i32) -> Result<()>;
    /// `log(this::class.java, Log.ERR, text)`: `origin` is the simple name of
    /// the handler's runtime class.
    fn log_class_error(&mut self, origin: &str, text: &str) -> Result<()>;
    /// `Exception.printStackTrace()` inside `Pulse.update`.
    fn print_stack_trace(&mut self, error: &Error) -> Result<()>;
    /// `effect.canFire(state)` of a bolt effect other than `DIAMOND`. Those
    /// overrides are not ported.
    fn bolt_can_fire(&mut self, effect: BoltEffect, state: &BattleState<Self>) -> Result<bool>;
    /// `effect.impact(state)` of a bolt effect other than `DIAMOND`.
    fn bolt_impact(&mut self, effect: BoltEffect, state: &mut BattleState<Self>) -> Result<()>;
}
