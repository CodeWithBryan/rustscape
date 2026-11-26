//! `combat_swing::Host` and `combat_pulse::Host` over the live world.
//!
//! Each method answers the original call it names from the canonical player
//! (profile, skills, equipment, attributes, masks, animator), the start-up's
//! NPC rows (levels, lifepoints, properties, definitions), the canonical
//! actor state (locations, walking queues, locks, pulse managers) and the
//! combat state of [`super::CombatWorld`]. A call into a system without a
//! port refuses by name (spells, familiars, degradation, bolt effects, ...);
//! projectiles are [`super::projectiles`] and the special attacks
//! [`super::specials`]; see docs/live-world.md.
use super::super::{
    host::Live,
    inner::{SkillsContext, placeholder_skills},
};
use super::{super::P, MAGIC, MELEE, RANGE, Submitted, style_handler};
use crate::{
    definitions::{item::Shared, property::Property},
    inventory::item::Handle as ItemHandle,
    player::skills::Skills,
    pulses,
    world::{
        animation::{Animation, Graphics as AnimGraphics},
        combat_pulse::{
            self as cp, AmmunitionVisuals, ImpactState, Prayer, ProjectileRequest, PulseState,
            Specials, WorldPulse,
        },
        combat_swing::{
            self as cs, Ammunition, ArmourSet, AttackStyle, BattleState, BoltEffect, CombatStyle,
            Error, Graphics, Kind, Perk, RangeWeapon, Result, SlayerTask, WeaponInterfaces,
        },
        doors::Location,
        ground_item, ground_manager as gm, npc_behaviour,
        player_walk::Node,
        prayer::combat as prayer_combat,
        prepared_masks,
    },
};
use std::sync::Arc;

pub(super) fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}
pub(super) fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

fn location_of(at: [i32; 3]) -> Location {
    Location::new(at[0], at[1], at[2])
}

impl Live {
    /// The NPC row of an entity, if it is an NPC.
    pub(crate) fn npc_row(&self, p: &P) -> Option<&crate::startup::npcs::Npc> {
        self.inner.npcs.rows.get(&p.node())
    }
    pub(crate) fn ckind(&self, p: &P) -> Kind {
        if self.inner.is_npc_node(p.node()) {
            Kind::Npc
        } else if self.inner.is_player_node(p.node()) {
            Kind::Player
        } else {
            Kind::Other
        }
    }
    pub(crate) fn cloc(&self, p: &P) -> Option<Location> {
        self.inner.state.node_location(p.node()).ok().flatten()
    }
    /// A player's Skills with its context, taken out of the profile for the
    /// call and put back after it.
    pub(crate) fn with_skills<T>(
        &mut self,
        p: &P,
        f: impl FnOnce(&mut Skills, &mut SkillsContext<'_>) -> T,
    ) -> Result<T> {
        let mut skills = self
            .inner
            .with_profile(p, |profile| {
                std::mem::replace(&mut profile.skills, placeholder_skills())
            })
            .map_err(bind)?;
        let result = f(
            &mut skills,
            &mut SkillsContext {
                inner: &mut self.inner,
                p: p.clone(),
            },
        );
        self.inner
            .with_profile(p, |profile| profile.skills = skills)
            .map_err(bind)?;
        Ok(result)
    }
    pub(super) fn equipment_slot(&self, p: &P, slot: i32) -> Option<ItemHandle> {
        if !self.inner.is_player_node(p.node()) {
            return None;
        }
        self.inner.canonical(p).player.equipment().get(slot)
    }
    pub(super) fn equipment_has(&self, p: &P, id: i32, amount: i32) -> bool {
        if !self.inner.is_player_node(p.node()) {
            return false;
        }
        let items = self.inner.canonical(p).player.equipment().items();
        let total: i64 = items
            .read()
            .iter()
            .flatten()
            .filter(|i| i.read().id() == id)
            .map(|i| i64::from(i.read().amount()))
            .sum();
        total >= i64::from(amount)
    }
    /// `Entity.face(target)`.
    pub(super) fn entity_face(&mut self, e: &P, target: Option<&P>) -> Result<()> {
        if self.inner.is_npc_node(e.node()) {
            let target = target.map(|t| self.face_target(t));
            self.npc_face(e.node(), target).map_err(bind)?;
            return Ok(());
        }
        let target = match target {
            None => None,
            Some(t) if self.inner.is_npc_node(t.node()) => {
                if !self.npc_views.contains_key(&t.node()) {
                    self.project_npc(t.node());
                }
                let view = self.npc_views.get(&t.node()).cloned().ok_or(Error::Null)?;
                Some(prepared_masks::FaceTarget::Npc(t.0.clone(), view))
            }
            Some(t) => {
                let render = self.inner.canonical(t).render.clone();
                Some(prepared_masks::FaceTarget::Player(t.0.clone(), render))
            }
        };
        self.inner
            .canonical(e)
            .player
            .masks()
            .face_entity(target)
            .map_err(bind)?;
        Ok(())
    }
    /// `Entity.animate`/`graphics`/`visualize` through the animator.
    pub(crate) fn entity_visualize(
        &mut self,
        e: &P,
        animation: Option<Animation>,
        graphics: Option<AnimGraphics>,
    ) -> Result<()> {
        if self.inner.is_npc_node(e.node()) {
            self.npc_animate(e.node(), animation, graphics)
                .map_err(bind)?;
            return Ok(());
        }
        let animator = self.inner.canonical(e).animator.clone();
        animator
            .animate(&mut self.walk.animations, animation, graphics)
            .map_err(bind)?;
        Ok(())
    }
    /// `playGlobalAudio(location, id)`: the area sound (97) after the zone
    /// origin (26) for every player within the default radius 8.
    pub(super) fn global_audio(&mut self, at: Location, id: i32) -> Result<()> {
        self.global_audio_delayed(at, id, 0)
    }
    /// `playGlobalAudio(location, id, delay)`.
    pub(super) fn global_audio_delayed(&mut self, at: Location, id: i32, delay: i32) -> Result<()> {
        let players = self
            .local(at.clone(), 8, crate::world::entity_regions::Select::Players)
            .map_err(bind)?;
        let [x, y, z] = at.coordinates();
        let base = Location::new(x & !7, y & !7, z);
        for player in players {
            let Some(p) = self.player_of(player.node) else {
                continue;
            };
            let offset = super::super::ground_items::World::with(self, |w| {
                crate::world::chunk_sync::scene_offset(w, &player, &base)
            })
            .map_err(bind)?;
            let mut origin = rs09_protocol::buffer::Writer::new();
            origin.p1neg(offset[0]);
            origin.p1(offset[1]);
            self.inner.queue_native(
                &p,
                rs09_protocol::game::Frame {
                    opcode: 26,
                    payload: origin.into_vec(),
                },
            );
            self.inner.queue_native(
                &p,
                rs09_protocol::game::updates::area_sound(x, y, id, 8, 1, delay),
            );
        }
        Ok(())
    }
    fn npc_definition_handlers(
        &self,
        p: &P,
    ) -> Option<Shared<std::collections::BTreeMap<String, Property>>> {
        self.npc_row(p)
            .map(|row| row.definition.read().handlers.clone())
    }
    fn npc_animation(&self, p: &P, slot: usize) -> Option<Animation> {
        self.npc_row(p)
            .and_then(|row| row.properties.animations[slot].clone())
            .map(Animation::from_shared)
    }
    /// A locked kind of the entity's action locks.
    pub(super) fn locked_kind(
        &mut self,
        p: &P,
        kind: crate::world::action_locks::Kind,
    ) -> Result<bool> {
        let state = &mut self.inner.state;
        let locks = state.locks(&p.0).map_err(bind)?;
        locks
            .is_kind_locked(kind, state, &mut crate::world::action_locks::Clock)
            .map_err(bind)
    }
    /// `walkingQueue.reset()`.
    pub(super) fn walk_reset(&mut self, p: &P) -> Result<()> {
        let node = p.node();
        self.with_walk(|world| crate::world::movement::Context::walk_reset(world, None, node, None))
            .map_err(bind)
    }
    /// The private `MovementPulse.updatePath()` of a combat pulse: a path to
    /// `DestinationFlag.ENTITY`'s tile beside the victim, found with the
    /// mover's pathfinder (smart for players, dumb for NPCs) and walked,
    /// unless the last path went toward the victim's same tile and the queue
    /// still has steps. This is not the MovementPulse body: the
    /// interrupt, truncation and face steps of `updatePath` are not run.
    fn chase(&mut self, e: &P) -> Result<()> {
        let node = e.node();
        let Some(victim) = self
            .inner
            .combat
            .entities
            .get(&node)
            .and_then(|entry| entry.state.victim.clone())
        else {
            return Ok(());
        };
        if let Some(row) = self.npc_row(e) {
            if row.brain.fields.read().never_walks {
                return Ok(());
            }
        }
        let Some(target) = self.cloc(&victim) else {
            return Ok(());
        };
        let has_path = self
            .inner
            .state
            .actor(node)
            .map(|a| a.queue.has_path())
            .unwrap_or(false);
        let last = self
            .inner
            .combat
            .entities
            .get(&node)
            .and_then(|entry| entry.chase.clone());
        if has_path
            && last
                .as_ref()
                .is_some_and(|l| l.coordinates() == target.coordinates())
        {
            return Ok(());
        }
        let finder = if self.inner.is_npc_node(node) {
            crate::world::movement::DUMB
        } else {
            crate::world::movement::SMART
        };
        let victim_node = victim.node();
        self.with_walk(|world| {
            // DestinationFlag.ENTITY's destination beside the victim, as the
            // pulse's flag gives it, else the victim itself.
            let destination = crate::world::movement::Flag::Entity
                .destination(world, Some(node), Some(victim_node))
                .map_err(crate::world::player_walk::Error::Movement)?;
            let path = match destination {
                Some(destination) => world.find_path(node, destination, finder, true)?,
                None => world.find_node_path(node, victim_node)?,
            };
            world.walk_path(node, path)
        })
        .map_err(bind)?;
        if let Some(entry) = self.inner.combat.entities.get_mut(&node) {
            entry.chase = Some(target);
        }
        Ok(())
    }
}

impl cs::Types for Live {
    type Entity = P;
    type Item = ItemHandle;
    /// A combat spell's id; no spell is ever selected (spells are not
    /// installed).
    type Spell = i32;
}

impl cs::Host for Live {
    fn kind(&self, entity: &P) -> Kind {
        self.ckind(entity)
    }
    fn same_spell(&self, a: &i32, b: &i32) -> bool {
        a == b
    }
    fn item_id(&self, item: &ItemHandle) -> i32 {
        item.read().id()
    }
    fn item_name(&self, item: &ItemHandle) -> String {
        item.read().name.clone().unwrap_or_else(|| "null".into())
    }
    fn item_amount(&self, item: &ItemHandle) -> i32 {
        item.read().amount()
    }
    fn range_weapon(&self, id: i32) -> Option<Arc<RangeWeapon>> {
        self.inner.combat.range.range_weapon(id)
    }
    fn ammunition(&self, id: i32) -> Option<Arc<Ammunition>> {
        self.inner.combat.range.ammunition(id)
    }
    fn bonuses(&mut self, entity: &P) -> Result<Option<Vec<i32>>> {
        if let Some(row) = self.npc_row(entity) {
            return Ok(row.properties.bonuses.as_ref().map(|b| b.read().clone()));
        }
        self.inner
            .with_profile(entity, |p| Some(p.properties.bonuses.clone()))
            .map_err(bind)
    }
    fn attack_style(&mut self, entity: &P) -> Result<Option<AttackStyle>> {
        if let Some(row) = self.npc_row(entity) {
            // AttackStyle(STYLE_CONTROLLED, bonus index) from configure().
            return Ok(row.properties.attack_style_bonus.map(|bonus| AttackStyle {
                style: cs::STYLE_CONTROLLED,
                bonus_type: bonus,
            }));
        }
        self.inner
            .with_profile(entity, |p| p.properties.attack_style)
            .map_err(bind)
    }
    fn armour_set(&mut self, entity: &P) -> Result<Option<ArmourSet>> {
        if self.inner.is_npc_node(entity.node()) {
            return Ok(self
                .inner
                .combat
                .armour
                .get(&entity.node())
                .copied()
                .flatten());
        }
        self.inner
            .with_profile(entity, |p| p.properties.armour_set)
            .map_err(bind)
    }
    fn pulse_victim(&mut self, entity: &P) -> Result<Option<P>> {
        Ok(self.inner.combat_entry(entity).state.victim.clone())
    }
    fn stop_combat_pulse(&mut self, entity: &P) -> Result<()> {
        cp::pulse::stop(self, entity)
    }
    fn update_combat_style(&mut self, entity: &P) -> Result<()> {
        cp::pulse::update_style(self, entity)
    }
    fn spell(&mut self, entity: &P) -> Result<Option<i32>> {
        if self.inner.is_npc_node(entity.node()) {
            return Ok(None);
        }
        let spell = self
            .inner
            .with_profile(entity, |p| p.properties.spell)
            .map_err(bind)?;
        match spell {
            None => Ok(None),
            Some(_) => refuse("a selected combat spell (spells are not installed)"),
        }
    }
    fn clear_spell(&mut self, entity: &P) -> Result<()> {
        if self.inner.is_player_node(entity.node()) {
            self.inner
                .with_profile(entity, |p| p.properties.spell = None)
                .map_err(bind)?;
        }
        Ok(())
    }
    fn autocast_spell(&mut self, entity: &P) -> Result<Option<i32>> {
        if self.inner.is_npc_node(entity.node()) {
            // NPC autocast spells (DefaultCombatSpell) are not ported: the
            // start-up counts them as a gap and sets none.
            return Ok(None);
        }
        let autocast = self
            .inner
            .with_profile(entity, |p| p.persisted.spells.autocast.is_some())
            .map_err(bind)?;
        if autocast {
            return refuse("an autocast spell (spells are not installed)");
        }
        Ok(None)
    }
    fn level(&mut self, entity: &P, slot: i32) -> Result<i32> {
        cs::Host::level_with(self, entity, slot, false)
    }
    fn level_with(&mut self, entity: &P, slot: i32, discard_assist: bool) -> Result<i32> {
        if let Some(row) = self.npc_row(entity) {
            return row.levels.get(slot as usize).copied().ok_or(Error::Index);
        }
        self.with_skills(entity, |skills, ctx| {
            skills.level(slot, discard_assist, ctx)
        })?
        .map_err(bind)
    }
    fn static_level(&mut self, entity: &P, slot: i32) -> Result<i32> {
        if let Some(row) = self.npc_row(entity) {
            return row
                .static_levels
                .get(slot as usize)
                .copied()
                .ok_or(Error::Index);
        }
        self.inner
            .with_profile(entity, |p| {
                p.skills.static_levels.get(slot as usize).copied()
            })
            .map_err(bind)?
            .ok_or(Error::Index)
    }
    fn lifepoints(&mut self, entity: &P) -> Result<i32> {
        if let Some(row) = self.npc_row(entity) {
            return Ok(row.lifepoints);
        }
        self.inner
            .with_profile(entity, |p| p.skills.lifepoints)
            .map_err(bind)
    }
    fn maximum_lifepoints(&mut self, entity: &P) -> Result<i32> {
        if let Some(row) = self.npc_row(entity) {
            return Ok(row.static_levels[3]);
        }
        self.with_skills(entity, |skills, ctx| skills.maximum_lifepoints(ctx))?
            .map_err(bind)
    }
    fn prayer_skill_bonus(&mut self, entity: &P, slot: i32) -> Result<f64> {
        let state = self.prayer_state(entity)?;
        Ok(prayer_combat::skill_bonus(&state, slot))
    }
    fn protects_from_summoning(&mut self, entity: &P) -> Result<bool> {
        let state = self.prayer_state(entity)?;
        Ok(prayer_combat::protects_from_summoning(&state))
    }
    fn has_protection_prayer(&mut self, entity: &P, style: CombatStyle) -> Result<bool> {
        if let Some(row) = self.npc_row(entity) {
            // NPC.hasProtectionPrayer: protectStyle == style.
            let protect = match &row.properties.protect_style {
                Some(Property::CombatStyle(s)) => Some(match s {
                    crate::definitions::property::CombatStyle::Melee => CombatStyle::Melee,
                    crate::definitions::property::CombatStyle::Range => CombatStyle::Range,
                    crate::definitions::property::CombatStyle::Magic => CombatStyle::Magic,
                }),
                _ => None,
            };
            return Ok(prayer_combat::npc_has_protection_prayer(
                protect,
                Some(style),
            ));
        }
        let state = self.prayer_state(entity)?;
        Ok(prayer_combat::player_has_protection_prayer(
            &state,
            Some(style),
        ))
    }
    fn perk_active(&mut self, _: &P, _: Perk) -> Result<bool> {
        self.inner
            .not_ported
            .default_answer("skillcape perks (skillcape_perks off: false)");
        Ok(false)
    }
    fn wearing_void(&mut self, entity: &P, style: CombatStyle) -> Result<bool> {
        // Player.isWearingVoid(style).
        let helm = match style {
            CombatStyle::Melee => 11665,
            CombatStyle::Range => 11664,
            CombatStyle::Magic => 11663,
        };
        let legs = self.equipment_has(entity, 8840, 1);
        let top = self.equipment_has(entity, 8839, 1) || self.equipment_has(entity, 10611, 1);
        let gloves = self.equipment_has(entity, 8842, 1);
        Ok(self.equipment_has(entity, helm, 1) && legs && top && gloves)
    }
    fn name(&mut self, entity: &P) -> Result<Option<String>> {
        if let Some(row) = self.npc_row(entity) {
            return Ok(row.definition.read().name.clone());
        }
        Ok(self
            .inner
            .players
            .get(&entity.node())
            .map(|s| s.name.clone()))
    }
    fn id(&mut self, entity: &P) -> Result<i32> {
        // Node.getId(): an NPC's id; a player's is 0.
        Ok(self.npc_row(entity).map_or(0, |row| row.id))
    }
    fn size(&mut self, entity: &P) -> Result<i32> {
        Ok(self
            .npc_row(entity)
            .map_or(1, |row| row.definition.read().size))
    }
    fn location(&mut self, entity: &P) -> Result<Option<[i32; 3]>> {
        Ok(self.cloc(entity).map(|l| l.coordinates()))
    }
    fn level_mod(&mut self, _: &P, _: Option<&P>) -> Result<f64> {
        // Entity.getLevelMod: 0 (only content NPC classes override it).
        Ok(0.0)
    }
    fn equipment(&mut self, entity: &P, slot: i32) -> Result<Option<ItemHandle>> {
        Ok(self.equipment_slot(entity, slot))
    }
    fn equipment_new(&mut self, entity: &P, slot: i32) -> Result<ItemHandle> {
        if let Some(item) = self.equipment_slot(entity, slot) {
            return Ok(item);
        }
        Ok(Shared::new(crate::inventory::item::Item::new(
            0,
            1,
            0,
            &self.inner.items,
        )))
    }
    fn equipment_contains_one(&mut self, entity: &P, id: i32) -> Result<bool> {
        Ok(self.equipment_has(entity, id, 1))
    }
    fn item_bonuses(&mut self, item: &ItemHandle) -> Result<Option<Vec<i32>>> {
        let definition = item.read().definition.clone();
        let Some(definition) = definition else {
            return Ok(None);
        };
        let value = definition.read().handlers.get("bonuses").cloned();
        Ok(match value {
            Some(Property::IntArray(values)) => Some(values.read().clone()),
            _ => None,
        })
    }
    fn slayer_task(&mut self, _: &P) -> Result<Option<SlayerTask>> {
        self.inner
            .not_ported
            .default_answer("SlayerManager.getTask (slayer not ported: no task)");
        Ok(None)
    }
    fn slayer_bonus(&mut self, _: &P) -> Result<f64> {
        // SlayerEquipmentFlags.getDamAccBonus: 1.0 without slayer equipment
        // flags (isSlayerEq answers false).
        self.inner
            .not_ported
            .default_answer("SlayerEquipmentFlags.isSlayerEq (false)");
        Ok(1.0)
    }
    fn in_equipment(&mut self, entity: &P, id: i32, amount: i32) -> Result<bool> {
        Ok(self.equipment_has(entity, id, amount))
    }
    fn familiar_pouch(&mut self, _: &P) -> Result<Option<i32>> {
        self.inner
            .not_ported
            .default_answer("FamiliarManager.hasFamiliar (false)");
        Ok(None)
    }
    fn npc_task(&mut self, _: &P) -> Result<Option<SlayerTask>> {
        // NPC.getTask(): Tasks.forId is not ported (the start-up counts it
        // as a gap), so no NPC has a task.
        self.inner
            .not_ported
            .default_answer("NPC.getTask (Tasks.forId not ported: null)");
        Ok(None)
    }
    fn npc_combat_distance(&mut self, entity: &P) -> Result<i32> {
        self.npc_row(entity)
            .map(|row| row.definition.read().combat_distance)
            .ok_or(Error::Null)
    }
    fn send_message(&mut self, entity: &P, text: &str) -> Result<()> {
        if self.inner.is_player_node(entity.node()) {
            self.inner.message(entity, text).map_err(bind)?;
        }
        Ok(())
    }
    fn log_error(&mut self, origin: &'static str, text: &str) -> Result<()> {
        self.inner.log_line(format!("{origin} ERR {text}"));
        Ok(())
    }
    fn play_global_audio(&mut self, location: [i32; 3], id: i32) -> Result<()> {
        self.global_audio(location_of(location), id)
    }
    fn graphics(&mut self, entity: &P, graphics: Graphics) -> Result<()> {
        self.entity_visualize(
            entity,
            None,
            Some(AnimGraphics::new(
                graphics.id,
                graphics.height,
                graphics.delay,
            )),
        )
    }
    fn weapon_interface(&mut self, entity: &P) -> Result<bool> {
        Ok(self.inner.is_player_node(entity.node())
            && self.inner.canonical(entity).weapon.is_some())
    }
    fn select_auto_spell(&mut self, _: &P, _: i32, _: bool, _: bool) -> Result<()> {
        refuse("WeaponInterface.selectAutoSpell (spells are not installed)")
    }
    fn spell_meets_requirements(&mut self, _: &i32, _: &P, _: bool, _: bool) -> Result<bool> {
        refuse("MagicSpell.meetsRequirements (spells are not installed)")
    }
    fn spell_maximum_impact(
        &mut self,
        _: &i32,
        _: &P,
        _: Option<&P>,
        _: &BattleState<Self>,
    ) -> Result<i32> {
        refuse("CombatSpell.getMaximumImpact (spells are not installed)")
    }
    fn spell_targets(
        &mut self,
        _: &i32,
        _: &P,
        _: Option<&P>,
    ) -> Result<Option<Vec<Option<BattleState<Self>>>>> {
        refuse("CombatSpell.getTargets (spells are not installed)")
    }
    fn spell_is_blitz(&mut self, _: &i32) -> Result<bool> {
        refuse("CombatSpell.getType (spells are not installed)")
    }
    fn math_random(&mut self) -> Result<f64> {
        Ok(self.inner.generators.math_random())
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        self.inner
            .generators
            .shared_next_int(bound)
            .ok_or(Error::Binding("nextInt bound".into()))
    }
}

impl cp::Host for Live {
    type HandlerRef = usize;
    type Animation = Animation;
    type Ground = ground_item::Handle;

    fn handler(&mut self, handler: &usize) -> &mut cs::Handler {
        &mut self.inner.combat.handlers[*handler].handler
    }
    fn is_salamander(&self, _: &usize) -> bool {
        false
    }
    fn handler_class_name(&self, handler: &usize) -> String {
        self.inner.combat.handlers[*handler].class.to_owned()
    }
    fn overrides(&self, handler: &usize) -> Option<cp::Overrides<Self>> {
        // A special attack instance's overriding members
        // (world::special_attacks, combat/specials.rs); the three
        // CombatStyle singletons override nothing. NPC subclasses (dragons,
        // world::dragonfire) and MonkNPC's CombatAction are answered by
        // super::super::dragons.
        if let Some(overrides) = self.content_overrides(handler) {
            return Some(overrides);
        }
        self.inner
            .combat
            .handlers
            .get(*handler)
            .and_then(|slot| slot.special)
            .map(crate::world::special_attacks::overrides::<Self>)
    }
    fn specials(&mut self, handler: &usize) -> &mut Specials<Self> {
        &mut self.inner.combat.handlers[*handler].specials
    }
    fn same_entity(&self, a: &P, b: &P) -> bool {
        a.node() == b.node()
    }
    fn is_ai_player(&self, _: &P) -> bool {
        false
    }
    fn identity_hash(&self, entity: &P) -> i32 {
        // Object.hashCode(): a platform input; the node id stands in.
        entity.node() as i32
    }
    fn combat_pulse(&mut self, entity: &P) -> &mut PulseState<Self> {
        &mut self.inner.combat_entry(entity).state
    }
    fn impact_handler(&mut self, entity: &P) -> &mut ImpactState<Self> {
        &mut self.inner.combat_entry(entity).impact
    }
    fn experience_mod(&mut self) -> &mut f64 {
        &mut self.inner.combat.experience_mod
    }
    fn item_charge(&self, item: &ItemHandle) -> i32 {
        item.read().charge()
    }
    fn animation_id(&self, animation: &Animation) -> i32 {
        animation.id()
    }
    fn ammunition_visuals(&self, item_id: i32) -> AmmunitionVisuals {
        let Some(a) = self.inner.combat.ranged.ammunition.get(&item_id) else {
            return AmmunitionVisuals::default();
        };
        let g = |g: crate::startup::loaders::ranged::Graphics| Graphics {
            id: g.id,
            height: g.height,
            delay: g.delay,
        };
        AmmunitionVisuals {
            start_graphics: Some(g(a.start_graphics)),
            dark_bow_graphics: a.dark_bow_graphics.map(g),
            projectile_id: Some(a.projectile.projectile_id),
        }
    }
    fn range_weapon_animation(&self, item_id: i32) -> Option<Animation> {
        self.inner
            .combat
            .ranged
            .weapons
            .get(&item_id)
            .map(|w| Animation::new(w.animation))
    }
    fn attribute_flag(&mut self, entity: &P, key: &str) -> Result<bool> {
        match self.inner.attribute(entity.node(), key) {
            None => Ok(false),
            Some(super::Attr::Bool(v)) => Ok(v),
            Some(_) => Err(Error::Binding(format!("{key} is not a boolean"))),
        }
    }
    fn autocast_component(&mut self, entity: &P) -> Result<bool> {
        Ok(self
            .inner
            .has_attribute(entity.node(), "autocast_component"))
    }
    fn has_attribute(&mut self, entity: &P, key: &str) -> Result<bool> {
        Ok(self.inner.has_attribute(entity.node(), key))
    }
    fn remove_attribute(&mut self, entity: &P, key: &str) -> Result<()> {
        self.inner.remove_attribute(entity.node(), key);
        Ok(())
    }
    fn random_event_npc(&mut self, entity: &P) -> Result<Option<P>> {
        match self.inner.attribute(entity.node(), "re-npc") {
            Some(super::Attr::Entity(p)) => Ok(Some(p)),
            _ => Ok(None),
        }
    }
    fn combat_attacker(&mut self, entity: &P) -> Result<Option<P>> {
        match self.inner.attribute(entity.node(), "combat-attacker") {
            Some(super::Attr::Entity(p)) => Ok(Some(p)),
            None => Ok(None),
            Some(_) => Err(Error::Binding("combat-attacker is not an entity".into())),
        }
    }
    fn set_combat_time(&mut self, entity: &P, millis: i64) -> Result<()> {
        self.inner
            .set_attribute(entity.node(), "combat-time", super::Attr::Long(millis));
        Ok(())
    }
    fn set_combat_attacker(&mut self, entity: &P, attacker: &P) -> Result<()> {
        self.inner.set_attribute(
            entity.node(),
            "combat-attacker",
            super::Attr::Entity(attacker.clone()),
        );
        Ok(())
    }
    fn set_combat_stop(&mut self, entity: &P, tick: i32) -> Result<()> {
        self.inner
            .set_attribute(entity.node(), "combat-stop", super::Attr::Int(tick));
        Ok(())
    }
    fn set_fam_exp(&mut self, entity: &P) -> Result<()> {
        self.inner
            .set_attribute(entity.node(), "fam-exp", super::Attr::Bool(true));
        Ok(())
    }
    fn duel_ammo(&mut self, entity: &P) -> Result<bool> {
        Ok(self.inner.has_attribute(entity.node(), "duel:ammo"))
    }
    fn duel_ammo_add(&mut self, _: &P, _: ground_item::Handle) -> Result<()> {
        refuse("the duel arena's ammunition list (duel arena not ported)")
    }
    fn index(&mut self, entity: &P) -> Result<i32> {
        if let Some(slot) = self.inner.players.get(&entity.node()) {
            return Ok(slot.index);
        }
        self.inner
            .state
            .actor(entity.node())
            .map(|a| a.wire_index)
            .map_err(bind)
    }
    fn client_index(&mut self, entity: &P) -> Result<i32> {
        let target = self.face_target(entity);
        Ok(super::super::host::Live::client_index(self, &target))
    }
    fn is_active(&mut self, entity: &P) -> Result<bool> {
        Ok(self.inner.node_active(entity.node()))
    }
    fn center_location(&mut self, entity: &P) -> Result<[i32; 3]> {
        // Node.getCenterLocation(): location plus size / 2.
        let at = self.cloc(entity).ok_or(Error::Null)?.coordinates();
        let half = cs::Host::size(self, entity)? >> 1;
        Ok([at[0] + half, at[1] + half, at[2]])
    }
    fn closest_occupied_tile(
        &mut self,
        entity: &P,
        other: Option<[i32; 3]>,
    ) -> Result<Option<[i32; 3]>> {
        // Entity.getClosestOccupiedTile(other): the occupied tile nearest
        // to `other` (the first of equals in x-then-y order).
        let other = other.ok_or(Error::Null)?;
        let at = self.cloc(entity).ok_or(Error::Null)?.coordinates();
        let size = cs::Host::size(self, entity)?;
        let mut best: Option<([i32; 3], f64)> = None;
        for x in 0..size {
            for y in 0..size {
                let tile = [at[0] + x, at[1] + y, at[2]];
                let dx = f64::from(tile[0] - other[0]);
                let dy = f64::from(tile[1] - other[1]);
                let distance = (dx * dx + dy * dy).sqrt();
                if best.is_none_or(|(_, d)| distance < d) {
                    best = Some((tile, distance));
                }
            }
        }
        Ok(best.map(|(tile, _)| tile))
    }
    fn swing_handler(&mut self, entity: &P, swing: bool) -> Result<usize> {
        if self.inner.is_npc_node(entity.node()) {
            return super::behaviour::npc_swing_handler(self, entity);
        }
        // Player.getSwingHandler(swing): special_attacks::player_swing_handler
        // over the three CombatStyle singletons and the registered specials.
        let _ = style_handler;
        let chosen = crate::world::special_attacks::player_swing_handler(
            self,
            entity,
            swing,
            [&MELEE, &RANGE, &MAGIC],
        )?;
        match chosen {
            crate::world::special_attacks::Chosen::Handler(handler) => Ok(handler),
            crate::world::special_attacks::Chosen::Chinchompa => refuse("ChinchompaSwingHandler"),
            crate::world::special_attacks::Chosen::Salamander => refuse("SalamanderSwingHandler"),
        }
    }
    fn entity_attackable(
        &mut self,
        victim: &P,
        attacker: &P,
        style: Option<CombatStyle>,
        message: bool,
    ) -> Result<bool> {
        npc_behaviour::combat::is_attackable(self, victim, attacker, style, message)
    }
    fn face(&mut self, entity: &P, target: Option<&P>) -> Result<()> {
        self.entity_face(entity, target)
    }
    fn face_temporary(&mut self, entity: &P, target: Option<&P>, ticks: i32) -> Result<()> {
        // Entity.faceTemporary: face, then a pulse of ticks + 1 faces null.
        self.entity_face(entity, target)?;
        self.submit_combat(
            ticks.wrapping_add(1),
            vec![Some(entity.node())],
            "core.game.node.entity.Entity$faceTemporary$1",
            Submitted::FaceReset(entity.clone()),
        )
        .map_err(bind)
    }
    fn attack_animation(&mut self, entity: &P) -> Result<Option<Animation>> {
        if self.inner.is_npc_node(entity.node()) {
            return Ok(self.npc_animation(entity, 0));
        }
        self.inner
            .with_profile(entity, |p| p.properties.attack_animation.clone())
            .map_err(bind)
    }
    fn defence_animation(&mut self, entity: &P) -> Result<Option<Animation>> {
        if self.inner.is_npc_node(entity.node()) {
            return Ok(self.npc_animation(entity, 1));
        }
        self.inner
            .with_profile(entity, |p| p.properties.defence_animation.clone())
            .map_err(bind)
    }
    fn combat_animation(&mut self, entity: &P, index: i32) -> Result<Option<Animation>> {
        // Properties.getCombatAnimation: attack, magic, range, defence, death.
        if self.inner.is_npc_node(entity.node()) {
            let slot = match index {
                0 => 0,
                1 => 3,
                2 => 4,
                3 => 1,
                _ => 2,
            };
            return Ok(self.npc_animation(entity, slot));
        }
        self.inner
            .with_profile(entity, |p| match index {
                0 => p.properties.attack_animation.clone(),
                1 => p.properties.magic_animation.clone(),
                2 => p.properties.range_animation.clone(),
                3 => p.properties.defence_animation.clone(),
                _ => p.properties.death_animation.clone(),
            })
            .map_err(bind)
    }
    fn animate(&mut self, entity: &P, animation: Option<&Animation>) -> Result<()> {
        self.entity_visualize(entity, animation.cloned(), None)
    }
    fn visualize_entity(
        &mut self,
        entity: &P,
        animation: Option<&Animation>,
        graphics: Option<Graphics>,
    ) -> Result<()> {
        self.entity_visualize(
            entity,
            animation.cloned(),
            graphics.map(|g| AnimGraphics::new(g.id, g.height, g.delay)),
        )
    }
    fn entity_on_impact(
        &mut self,
        victim: &P,
        attacker: Option<&P>,
        state: &mut BattleState<Self>,
    ) -> Result<()> {
        // NPC.onImpact with a subclass's override (super::super::subclasses).
        super::super::subclasses::on_impact(self, victim, attacker, state)
    }
    fn send_impact(&mut self, entity: &P, _: &mut BattleState<Self>) -> Result<()> {
        npc_behaviour::combat::send_impact(self, entity)
    }
    fn check_impact(&mut self, entity: &P, state: &mut BattleState<Self>) -> Result<()> {
        npc_behaviour::combat::check_impact(self, entity, state)
    }
    fn full_restore(&mut self, entity: &P) -> Result<()> {
        super::prayer::full_restore(self, entity)
    }
    fn entity_formatted_hit(&mut self, _: &P, state: &BattleState<Self>, hit: i32) -> Result<f64> {
        let kinds: Vec<(Node, Kind)> = [state.entity.as_ref(), state.victim.as_ref()]
            .into_iter()
            .flatten()
            .map(|p| (p.node(), self.ckind(p)))
            .collect();
        let kind = |p: &P| {
            kinds
                .iter()
                .find(|(n, _)| *n == p.node())
                .map_or(Kind::Other, |(_, k)| *k)
        };
        let mut answers = Vec::new();
        if let (Some(victim), Some(style)) = (state.victim.as_ref(), state.style) {
            answers.push(cs::Host::has_protection_prayer(self, victim, style)?);
        }
        let mut answers = answers.into_iter();
        prayer_combat::formatted_hit(state, hit, kind, |_, _| Ok(answers.next().unwrap_or(false)))
    }
    fn familiar_adjust_battle_state(&mut self, _: &P, _: &mut BattleState<Self>) -> Result<()> {
        // FamiliarManager.adjustBattleState: no familiar is summoned here.
        self.inner
            .not_ported
            .default_answer("FamiliarManager.hasFamiliar (false)");
        Ok(())
    }
    fn check_weapon_degrades(&mut self, _: &P) -> Result<()> {
        self.inner.not_ported.default_answer(
            "DegradableEquipment.checkWeaponDegrades (no degradable equipment registered)",
        );
        Ok(())
    }
    fn prayer_active(&mut self, entity: &P, prayer: Prayer) -> Result<bool> {
        let state = self.prayer_state(entity)?;
        Ok(prayer_combat::active(&state, prayer))
    }
    fn prayer_points(&mut self, entity: &P) -> Result<f64> {
        self.inner
            .with_profile(entity, |p| p.skills.prayer_points)
            .map_err(bind)
    }
    fn decrement_prayer_points(&mut self, entity: &P, amount: f64) -> Result<()> {
        self.with_skills(entity, |skills, ctx| skills.decrement_prayer(amount, ctx))?
            .map_err(bind)
    }
    fn protect_style(&mut self, entity: &P) -> Result<Option<CombatStyle>> {
        if self.inner.is_npc_node(entity.node()) {
            let row = self.npc_row(entity).ok_or(Error::Null)?;
            return Ok(match &row.properties.protect_style {
                Some(Property::CombatStyle(s)) => Some(match s {
                    crate::definitions::property::CombatStyle::Melee => CombatStyle::Melee,
                    crate::definitions::property::CombatStyle::Range => CombatStyle::Range,
                    crate::definitions::property::CombatStyle::Magic => CombatStyle::Magic,
                }),
                _ => None,
            });
        }
        self.inner
            .with_profile(entity, |p| p.properties.protect_style)
            .map_err(bind)
    }
    fn degrade(&mut self, _: &P, _: &P, _: bool) -> Result<()> {
        self.inner
            .not_ported
            .default_answer("DegradableEquipment.degrade (no degradable equipment registered)");
        Ok(())
    }
    fn apply_poison(&mut self, victim: &P, source: &P, severity: i32) -> Result<()> {
        super::death::apply_poison(self, victim, source, severity)
    }
    fn npc_poisonous(&mut self, entity: &P) -> Result<bool> {
        // definition.getConfiguration("poisonous", false).
        let handlers = self.npc_definition_handlers(entity).ok_or(Error::Null)?;
        let value = handlers.read().get("poisonous").cloned();
        match value {
            None | Some(Property::Null) => Ok(false),
            Some(Property::Bool(v)) => Ok(v),
            Some(_) => Err(Error::Binding("poisonous is not a boolean".into())),
        }
    }
    fn npc_poison_severity(&mut self, entity: &P) -> Result<i32> {
        let handlers = self.npc_definition_handlers(entity).ok_or(Error::Null)?;
        let value = handlers.read().get("poison_amount").cloned();
        match value {
            None | Some(Property::Null) => Ok(0),
            Some(Property::Int(v)) => Ok(v),
            Some(_) => Err(Error::Binding("poison_amount is not an integer".into())),
        }
    }
    fn is_ironman(&mut self, entity: &P) -> Result<bool> {
        if !self.inner.is_player_node(entity.node()) {
            return Ok(false);
        }
        Ok(self.inner.canonical(entity).player.ironman().is_ironman())
    }
    fn familiar_owner(&mut self, _: &P) -> Result<Option<P>> {
        refuse("Familiar.getOwner (familiars are not spawned)")
    }
    fn familiar_attack_style(&mut self, _: &P) -> Result<i32> {
        refuse("Familiar.getAttackStyle (familiars are not spawned)")
    }
    fn npc_xp_multiplier(&mut self, npc: &P, player: &P) -> Result<f64> {
        let behaviour = npc_behaviour::behaviour(self, npc)?;
        behaviour.xp_multiplier(self, npc, player)
    }
    fn add_experience(
        &mut self,
        entity: &P,
        slot: i32,
        experience: f64,
        player_mod: bool,
    ) -> Result<()> {
        if !self.inner.is_player_node(entity.node()) {
            return Ok(());
        }
        self.with_skills(entity, |skills, ctx| {
            skills.add_experience(slot, experience, player_mod, ctx)
        })?
        .map_err(bind)
    }
    fn spell_id(&mut self, spell: &i32) -> Result<i32> {
        Ok(*spell)
    }
    fn spell_fire_effect(
        &mut self,
        _: &i32,
        _: &P,
        _: Option<&P>,
        _: &mut BattleState<Self>,
    ) -> Result<()> {
        refuse("CombatSpell.fireEffect (spells are not installed)")
    }
    fn spell_visualize(&mut self, _: &i32, _: &P, _: Option<&P>) -> Result<()> {
        refuse("CombatSpell.visualize (spells are not installed)")
    }
    fn spell_visualize_impact(
        &mut self,
        _: &i32,
        _: Option<&P>,
        _: Option<&P>,
        _: &mut BattleState<Self>,
    ) -> Result<()> {
        refuse("CombatSpell.visualizeImpact (spells are not installed)")
    }
    fn spell_add_experience(&mut self, _: &i32, _: &P, _: i32) -> Result<()> {
        refuse("MagicSpell.addExperience (spells are not installed)")
    }
    fn equipment_replace(
        &mut self,
        entity: &P,
        item: Option<(i32, i32, i32)>,
        slot: i32,
    ) -> Result<()> {
        // equipment.replace(item, slot): Container.replace with fire, over
        // the canonical equipment and its listeners.
        if !self.inner.is_player_node(entity.node()) {
            return Err(Error::Null);
        }
        let definitions = self.inner.items.clone();
        let item = item.map(|(id, amount, charge)| {
            Shared::new(crate::inventory::item::Item::new(
                id,
                amount,
                charge,
                &definitions,
            ))
        });
        let mut equipment = self.inner.canonical(entity).player.equipment();
        let listeners = self.item_actions.listeners.clone();
        let mut ctx = super::super::inner::EquipmentContext {
            inner: &mut self.inner,
            p: entity.clone(),
            listeners: Some(listeners),
        };
        let result = equipment.replace(item.as_ref(), slot, true, &definitions, &mut ctx);
        self.inner.drain(entity);
        result.map(|_| ()).map_err(bind)
    }
    fn equipment_contains(&mut self, entity: &P, id: i32, amount: i32) -> Result<bool> {
        Ok(self.equipment_has(entity, id, amount))
    }
    fn clipping_flag(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.walk
            .scene
            .context(&mut self.walk.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| Error::Binding(e.0))
    }
    fn projectile_flag(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.walk
            .scene
            .context(&mut self.walk.maps)
            .clipping(Location::new(x, y, z), true)
            .map_err(|e| Error::Binding(e.0))
    }
    fn ground_item_increase(
        &mut self,
        id: i32,
        amount: i32,
        location: [i32; 3],
        player: &P,
    ) -> Result<Option<ground_item::Handle>> {
        let item = Shared::new(crate::inventory::item::Item::new(
            id,
            amount,
            0,
            &self.inner.items,
        ));
        let manager = self.walk.ground.manager.clone();
        let owner = player.0.clone();
        super::super::ground_items::World::with(self, |w| {
            gm::increase(
                w,
                &manager,
                Some(item),
                Some(location_of(location)),
                Some(owner),
            )
        })
        .map(Some)
        .map_err(bind)
    }
    fn item_male_worn_model(&mut self, item: &ItemHandle) -> Result<i32> {
        let definition = item.read().definition.clone().ok_or(Error::Null)?;
        let model = definition.read().male_worn_model_id1;
        Ok(model)
    }
    fn item_attack_audios(&mut self, item: &ItemHandle) -> Result<Option<Vec<Option<i32>>>> {
        let definition = item.read().definition.clone().ok_or(Error::Null)?;
        let value = definition.read().handlers.get("attack_audios").cloned();
        Ok(match value {
            Some(Property::IntArray(values)) => {
                Some(values.read().iter().map(|v| Some(*v)).collect())
            }
            _ => None,
        })
    }
    fn is_moving(&mut self, entity: &P) -> Result<bool> {
        let actor = self.inner.state.actor(entity.node()).map_err(bind)?;
        Ok(actor.queue.is_moving())
    }
    fn is_running_both(&mut self, entity: &P) -> Result<bool> {
        let actor = self.inner.state.actor(entity.node()).map_err(bind)?;
        Ok(actor.queue.is_running_both())
    }
    fn run_dir(&mut self, entity: &P) -> Result<i32> {
        let actor = self.inner.state.actor(entity.node()).map_err(bind)?;
        Ok(actor.queue.run_dir())
    }
    fn walking_reset(&mut self, entity: &P) -> Result<()> {
        self.walk_reset(entity)
    }
    fn movement_locked(&mut self, entity: &P) -> Result<bool> {
        self.locked_kind(entity, crate::world::action_locks::Kind::Movement)
    }
    fn interaction_locked(&mut self, entity: &P) -> Result<bool> {
        self.locked_kind(entity, crate::world::action_locks::Kind::Interaction)
    }
    fn close_component(&mut self, _: &P) -> Result<()> {
        refuse("interfaceManager.close(autocast component) (spells are not installed)")
    }
    fn open_weapon_tab(&mut self, _: &P, _: bool) -> Result<()> {
        refuse("interfaceManager.openTab(weapon interface) from combat (autocast)")
    }
    fn attack(&mut self, entity: &P, victim: &P) -> Result<()> {
        cp::pulse::attack(self, entity, Some(victim))
    }
    fn update_weapon_interface(&mut self, _: &P) -> Result<()> {
        refuse("WeaponInterface.updateInterface from combat (autocast)")
    }
    fn debug(&mut self, entity: &P, text: &str) -> Result<()> {
        // Player.debug: only shown in debug mode.
        if self.inner.settings.dev_mode && self.inner.is_player_node(entity.node()) {
            self.inner.message(entity, text).map_err(bind)?;
        }
        Ok(())
    }
    fn weapon_interfaces(&mut self, entity: &P) -> Result<Option<WeaponInterfaces>> {
        if !self.inner.is_player_node(entity.node()) {
            return Ok(None);
        }
        Ok(self
            .inner
            .canonical(entity)
            .weapon_tab
            .as_ref()
            .and_then(|tab| tab.weapon_interface()))
    }
    fn is_dead(&mut self, entity: &P) -> Result<bool> {
        crate::world::npc_death::is_dead(self, &entity.0).map_err(bind)
    }
    fn region_active(&mut self, region_id: i32) -> Result<bool> {
        let region = self.walk.scene.region(region_id);
        Ok(crate::world::region_activity::is_active(&region))
    }
    fn combat_time_out(&mut self, entity: &P) -> Result<i32> {
        if self.inner.is_npc_node(entity.node()) {
            return Ok(self
                .inner
                .combat
                .attrs
                .get(&entity.node())
                .and_then(|a| match a.get("properties:combatTimeOut") {
                    Some(super::Attr::Int(v)) => Some(*v),
                    _ => None,
                })
                .unwrap_or(10));
        }
        self.inner
            .with_profile(entity, |p| p.properties.combat_time_out)
            .map_err(bind)
    }
    fn attack_speed(&mut self, entity: &P) -> Result<i32> {
        if let Some(row) = self.npc_row(entity) {
            return Ok(row.properties.attack_speed);
        }
        self.inner
            .with_profile(entity, |p| p.properties.attack_speed)
            .map_err(bind)
    }
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.inner.ticks)
    }
    fn miasmic_active(&mut self, entity: &P) -> Result<bool> {
        // A player's or an NPC's timer manager (super::super::timers).
        self.has_timer_of(entity.node(), "miasmic").map_err(bind)
    }
    fn npc_hidden(&mut self, npc: &P, player: &P) -> Result<bool> {
        npc_behaviour::combat::npc_is_hidden(self, npc, player)
    }
    fn npc_can_start_combat(&mut self, npc: &P, victim: &P) -> Result<bool> {
        let class = npc_behaviour::class(self, npc)?;
        class.can_start_combat(self, npc, victim)
    }
    fn movement_update_path(&mut self, entity: &P) -> Result<()> {
        self.chase(entity)
    }
    fn movement_set_last(&mut self, entity: &P) -> Result<()> {
        if let Some(entry) = self.inner.combat.entities.get_mut(&entity.node()) {
            entry.chase = None;
        }
        Ok(())
    }
    fn movement_set_destination(&mut self, _: &P, _: Option<&P>) -> Result<()> {
        // The destination is the pulse's victim, read by updatePath.
        Ok(())
    }
    fn interface_close(&mut self, entity: &P) -> Result<()> {
        if !self.inner.is_player_node(entity.node()) {
            return Ok(());
        }
        self.inner.current = Some(entity.clone());
        let manager = self.inner.canonical(entity).player.manager();
        crate::world::interfaces::close(&mut self.inner, &manager)
            .map(|_| ())
            .map_err(bind)
    }
    fn interface_close_chatbox(&mut self, entity: &P) -> Result<()> {
        if !self.inner.is_player_node(entity.node()) {
            return Ok(());
        }
        self.inner.current = Some(entity.clone());
        let manager = self.inner.canonical(entity).player.manager();
        crate::world::interfaces::close_chatbox(&mut self.inner, &manager).map_err(bind)
    }
    fn is_wilderness(&mut self, entity: &P) -> Result<bool> {
        crate::world::player_managers::skull::answers::is_wilderness(self, &entity.0).map_err(bind)
    }
    fn check_skull(&mut self, entity: &P, victim: &P) -> Result<()> {
        crate::world::player_managers::skull::check_skull(self, &entity.0, Some(&victim.0))
            .map_err(bind)
    }
    fn pulse_manager_is_moving(&mut self, entity: &P) -> Result<bool> {
        let node = entity.node();
        self.with_walk(|world| {
            let manager = world.state.actor(node)?.manager.clone();
            Ok::<_, crate::world::player_walk::Error>(manager.current().is_some_and(|p| {
                let p = p.read();
                p.running && matches!(p.class, pulses::Class::Movement | pulses::Class::Combat)
            }))
        })
        .map_err(bind)
    }
    fn pulse_manager_clear(&mut self, entity: &P) -> Result<()> {
        let node = entity.node();
        self.with_walk(|world| world.clear_pulses(node))
            .map_err(bind)
    }
    fn pulse_manager_run(&mut self, entity: &P) -> Result<()> {
        // PulseManager.run(combatPulse) with STANDARD: drop pulses that are
        // not running, return while a strong pulse runs, stop the current
        // standard pulse, then put, start and submit the combat pulse.
        let node = entity.node();
        let manager = self.inner.state.actor(node).map_err(bind)?.manager.clone();
        for (kind, pulse) in manager.entries() {
            if let Some(pulse) = pulse {
                if !pulse.read().running {
                    manager.put(kind, None);
                }
            }
        }
        if manager
            .get(Some(pulses::Kind::Strong))
            .is_some_and(|p| p.read().running)
        {
            return Ok(());
        }
        self.with_walk(|world| world.clear_standard(node, manager.clone()))
            .map_err(bind)?;
        let handle = self.inner.combat_entry(entity).handle.clone();
        manager.put(Some(pulses::Kind::Standard), Some(handle.clone()));
        cp::pulse::start(self, entity)?;
        let running = self.inner.combat_entry(entity).state.running;
        handle.write().running = running;
        if running {
            self.driver.pulser().submit(Some(handle)).map_err(bind)?;
        }
        Ok(())
    }
    fn current_time_millis(&mut self) -> Result<i64> {
        Ok(super::super::now_millis())
    }
    fn combat_level(&mut self, entity: &P) -> Result<i32> {
        if let Some(row) = self.npc_row(entity) {
            return Ok(row.properties.combat_level);
        }
        self.inner
            .with_profile(entity, |p| p.properties.combat_level)
            .map_err(bind)
    }
    fn player_send_message(&mut self, entity: &P, text: &str) -> Result<()> {
        cs::Host::send_message(self, entity, text)
    }
    fn update_level(&mut self, entity: &P, slot: i32, amount: i32, maximum: i32) -> Result<i32> {
        if self.inner.is_npc_node(entity.node()) {
            // Skills.updateLevel over the NPC's levels (no packet).
            let row = self
                .inner
                .npcs
                .rows
                .get_mut(&entity.node())
                .ok_or(Error::Null)?;
            let i = usize::try_from(slot).map_err(|_| Error::Index)?;
            let level = *row.levels.get(i).ok_or(Error::Index)?;
            if amount > 0 && level > maximum {
                return Ok(amount.wrapping_neg());
            }
            let left = level.wrapping_add(amount).wrapping_sub(maximum);
            let mut next = level.wrapping_add(amount);
            if next < 0 {
                next = 0;
            } else if (amount < 0 && next < maximum) || (amount > 0 && next > maximum) {
                next = maximum;
            }
            row.levels[i] = next;
            return Ok(left);
        }
        self.with_skills(entity, |skills, ctx| {
            skills.update_level(slot, amount, maximum, ctx)
        })?
        .map_err(bind)
    }
    fn on_attack(&mut self, entity: &P, victim: &P) -> Result<()> {
        super::behaviour::on_attack(self, entity, victim)
    }
    fn remove_weak_scripts(&mut self, entity: &P) -> Result<()> {
        let node = entity.node();
        self.with_walk(|world| {
            crate::world::player_walk::Services::event(
                &mut *world.services,
                world.state,
                crate::world::player_walk::Event::Scripts(
                    node,
                    crate::world::player_walk::Scripts::Weak,
                ),
            )
        })
        .map_err(bind)
    }
    fn set_armour_set(&mut self, entity: &P, set: Option<ArmourSet>) -> Result<()> {
        if self.inner.is_npc_node(entity.node()) {
            self.inner.combat.armour.insert(entity.node(), set);
            return Ok(());
        }
        self.inner
            .with_profile(entity, |p| p.properties.armour_set = set)
            .map_err(bind)
    }
    fn submit(&mut self, pulse: WorldPulse<Self>) -> Result<()> {
        let class = match &pulse.task {
            cp::Task::Impact { .. } => "core.game.node.entity.combat.CombatPulse$Companion$swing$1",
            cp::Task::SecondHit { .. } => "core.game.node.entity.combat.RangeSwingHandler$impact$1",
            cp::Task::ManualHit { .. } => "core.game.node.entity.combat.ImpactHandler$manualHit$1",
        };
        // The driver's runner object only carries the class; the pulse's own
        // fields (delay, checks, running) are the WorldPulse's.
        self.submit_combat(1, Vec::new(), class, Submitted::World(pulse))
            .map_err(bind)
    }
    fn before_attack_finalized(
        &mut self,
        npc: &P,
        victim: &P,
        state: &mut BattleState<Self>,
    ) -> Result<()> {
        let behaviour = npc_behaviour::behaviour(self, npc)?;
        behaviour.before_attack_finalized(self, npc, victim, state)
    }
    fn npc_audio(&mut self, entity: &P, index: i32) -> Result<Option<i32>> {
        // NPC.getAudio(index): the combat_audio entry unless it is 0.
        let handlers = self.npc_definition_handlers(entity).ok_or(Error::Null)?;
        let value = handlers.read().get("combat_audio").cloned();
        match value {
            None | Some(Property::Null) => Ok(None),
            Some(Property::IntArray(audios)) => {
                let id = *audios.read().get(index as usize).ok_or(Error::Index)?;
                Ok((id != 0).then_some(id))
            }
            Some(_) => Err(Error::Binding("combat_audio is not an int array".into())),
        }
    }
    fn play_hurt_audio(&mut self, entity: &P) -> Result<()> {
        // playHurtAudio(player): one of four sounds by gender, drawn with
        // Kotlin's Random.Default (a generator of its own here).
        let male = self.inner.canonical(entity).player.appearance().is_male();
        let sounds = if male {
            [516, 517, 518, 522]
        } else {
            [506, 507, 508, 510]
        };
        let id = sounds[self.inner.combat.kotlin_random(4) as usize];
        self.inner
            .queue_native(entity, rs09_protocol::game::updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn attack_style_index(&mut self, entity: &P) -> Result<i32> {
        Ok(self
            .inner
            .canonical(entity)
            .player
            .settings()
            .fields
            .read()
            .attack_style_index)
    }
    fn npc_combat_graphics(&mut self, entity: &P) -> Result<Option<Vec<Option<Graphics>>>> {
        let row = self.npc_row(entity).ok_or(Error::Null)?;
        let graphics = row.definition.read().combat_graphics.clone();
        Ok(graphics.map(|list| {
            list.read()
                .iter()
                .map(|g| {
                    g.as_ref().map(|g| {
                        let g = g.read();
                        Graphics {
                            id: g.id,
                            height: g.height,
                            delay: g.delay,
                        }
                    })
                })
                .collect()
        }))
    }
    fn send_projectile(&mut self, request: ProjectileRequest<'_, Self>) -> Result<()> {
        // The Projectile factory call, then send() (combat/projectiles.rs).
        let projectile = match request {
            ProjectileRequest::AmmunitionCopy {
                ammunition_id,
                source,
                victim,
                speed,
            } => {
                let template = self
                    .inner
                    .combat
                    .ranged
                    .ammunition
                    .get(&ammunition_id)
                    .map(|a| a.projectile)
                    .ok_or(Error::Null)?;
                let victim = victim.ok_or(Error::Null)?;
                self.projectile_copy(&template, source, victim, speed)?
            }
            ProjectileRequest::Create {
                source,
                victim,
                id,
                start_height,
                end_height,
                start_delay,
                speed,
                angle,
            } => self.projectile_create(
                source,
                victim,
                id,
                start_height,
                end_height,
                start_delay,
                speed,
                angle,
            ),
            ProjectileRequest::Ranged {
                source,
                victim,
                id,
                start_height,
                end_height,
                start_delay,
                angle,
            } => {
                let victim = victim.ok_or(Error::Null)?;
                self.projectile_speed_based(
                    source,
                    victim,
                    id,
                    start_height,
                    end_height,
                    start_delay,
                    angle,
                    5.0,
                )?
            }
        };
        self.projectile_send(projectile)
    }
    fn skills_hit(&mut self, entity: &P, damage: i32) -> Result<i32> {
        if let Some(row) = self.inner.npcs.rows.get_mut(&entity.node()) {
            // Skills.hit over the NPC's lifepoints.
            row.lifepoints = row.lifepoints.wrapping_sub(damage);
            let mut left = 0;
            if row.lifepoints < 0 {
                left = row.lifepoints.wrapping_neg();
                row.lifepoints = 0;
            }
            return Ok(left);
        }
        self.inner
            .with_profile(entity, |p| p.skills.hit(damage))
            .map_err(bind)
    }
    fn heal(&mut self, entity: &P, amount: i32) -> Result<i32> {
        if let Some(row) = self.inner.npcs.rows.get_mut(&entity.node()) {
            row.lifepoints = row.lifepoints.wrapping_add(amount);
            let maximum = row.static_levels[3];
            let mut left = 0;
            if row.lifepoints > maximum {
                left = row.lifepoints.wrapping_sub(maximum);
                row.lifepoints = maximum;
            }
            return Ok(left);
        }
        self.with_skills(entity, |skills, ctx| skills.heal(amount, ctx))?
            .map_err(bind)
    }
    fn dynamic_levels(&mut self, entity: &P) -> Result<Option<Vec<i32>>> {
        if let Some(row) = self.npc_row(entity) {
            return Ok(Some(row.levels.to_vec()));
        }
        self.inner
            .with_profile(entity, |p| Some(p.skills.dynamic_levels.to_vec()))
            .map_err(bind)
    }
    fn update_run_energy(&mut self, _: &P, _: f64) -> Result<()> {
        refuse("settings.updateRunEnergy from combat (the karil set effect)")
    }
    fn uid(&mut self, entity: &P) -> Result<i32> {
        self.inner
            .players
            .get(&entity.node())
            .map(|s| s.uid)
            .ok_or(Error::Null)
    }
    fn has_familiar(&mut self, _: &P) -> Result<bool> {
        self.inner
            .not_ported
            .default_answer("FamiliarManager.hasFamiliar (false)");
        Ok(false)
    }
    fn familiar_is_pet(&mut self, _: &P) -> Result<bool> {
        refuse("FamiliarManager.getFamiliar (familiars are not spawned)")
    }
    fn start_death(&mut self, entity: &P, killer: Option<&P>) -> Result<()> {
        super::death::start_death(self, entity, killer)
    }
    fn player_by_uid(&mut self, uid: i32) -> Result<Option<P>> {
        Ok(self
            .inner
            .players
            .values()
            .find(|s| s.uid == uid && s.active)
            .map(|s| P(s.canonical.owner.clone())))
    }
    fn start_redemption(&mut self, entity: &P) -> Result<()> {
        super::prayer::start_redemption(self, entity)
    }
    fn zone_type(&mut self, entity: &P) -> Result<i32> {
        let monitor = self.monitor(&entity.0);
        monitor.get_type().map_err(bind)
    }
    fn ring_of_life_teleport(&mut self, _: &P) -> Result<bool> {
        refuse("EnchantedJewellery.RING_OF_LIFE (teleports are not installed)")
    }
    fn recoil_damage(&mut self, entity: &P) -> Result<i32> {
        self.inner
            .with_profile(entity, |p| p.saved.global.recoil_damage)
            .map_err(bind)
    }
    fn set_recoil_damage(&mut self, entity: &P, damage: i32) -> Result<()> {
        self.inner
            .with_profile(entity, |p| p.saved.global.recoil_damage = damage)
            .map_err(bind)
    }
    fn log_class_error(&mut self, origin: &str, text: &str) -> Result<()> {
        self.inner.log_line(format!("{origin} ERR {text}"));
        Ok(())
    }
    fn print_stack_trace(&mut self, error: &Error) -> Result<()> {
        eprintln!("[live] combat printStackTrace {error:?}");
        self.inner
            .log_line(format!("combat printStackTrace {error:?}"));
        Ok(())
    }
    fn bolt_can_fire(&mut self, effect: BoltEffect, _: &BattleState<Self>) -> Result<bool> {
        refuse(&format!("BoltEffect.{}.canFire override", effect.name()))
    }
    fn bolt_impact(&mut self, effect: BoltEffect, _: &mut BattleState<Self>) -> Result<()> {
        refuse(&format!("BoltEffect.{}.impact override", effect.name()))
    }
}

impl super::CombatWorld {
    /// Kotlin's `Random.Default.nextInt(bound)`: a generator of its own,
    /// seeded from the clock at the first draw (the original's is not one of
    /// the modelled Java generators).
    pub fn kotlin_random(&mut self, bound: i32) -> i32 {
        if self.kotlin_seed == 0 {
            self.kotlin_seed = (super::super::now_millis() as u64) | 1;
        }
        let mut x = self.kotlin_seed;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.kotlin_seed = x;
        (x % bound.max(1) as u64) as i32
    }
}

// Unused handler constants are named for clarity in dispatch.
const _: [usize; 3] = [MELEE, RANGE, MAGIC];
