//! Combat in the live world: `CombatPulse` and the swing handlers
//! (`world::combat_pulse`, `world::combat_swing`) for every entity, the
//! `ImpactHandler` with hit marks in the player and NPC update masks, NPC
//! aggression (`world::npc_behaviour`), NPC death and drops
//! (`world::npc_death`, `world::drop_tables`), prayer (`world::prayer`) and
//! the impact hits the engine timers deal.
//!
//! The live world is each library's host: [`host`] answers
//! `combat_swing::Host` and `combat_pulse::Host`, [`behaviour`]
//! `npc_behaviour::Host`, [`death`] `npc_death::Host` with `drop::Host` and the
//! ground manager, [`prayer`] `prayer::Host`, [`masks`] the NPC update masks.
//! State lives where the original keeps it: one [`Entry`] (the combat pulse
//! and the impact handler) per entity that fought, the swing handler
//! singletons, `EXPERIENCE_MOD`, the NPCs' masks and animators, the NPCs'
//! attributes and AI, each player's prayer.
//!
//! Every call into a system without a port is a refusal or a named answer
//! of [`super::not_ported`]; see docs/live-world.md.
pub mod behaviour;
pub mod death;
pub mod host;
pub mod masks;
pub mod prayer;

use super::{P, host::Live, inner::Inner};
use crate::{
    definitions::item::Shared,
    player::attributes::AttributeValue,
    pulses,
    world::{
        animation::Animator,
        combat_pulse::{self as cp, HitsplatType, ImpactState, PulseState, Specials, WorldPulse},
        combat_swing::{self as cs, ArmourSet, CombatStyle, HandlerKind},
        doors::Location,
        drop_tables::shared::SharedTables,
        npc_behaviour::Ai,
        npc_death::{self, ImpactLog},
        player_walk::Node,
        prayer::PrayerState,
    },
};
use std::collections::{BTreeMap, HashMap, HashSet};

/// The three `CombatStyle` swing handler singletons, by index.
pub const MELEE: usize = 0;
pub const RANGE: usize = 1;
pub const MAGIC: usize = 2;

/// An attribute value combat and death keep on an entity. A player's
/// boolean, integer and long attributes live in its canonical attribute
/// store; entity values and every NPC attribute live here.
#[derive(Clone)]
pub enum Attr {
    Bool(bool),
    Int(i32),
    Long(i64),
    Entity(P),
}

/// One entity's `CombatPulse` and `ImpactHandler`.
pub struct Entry {
    pub state: PulseState<Live>,
    pub impact: ImpactState<Live>,
    /// The combat pulse as the entity's pulse manager and the world runner
    /// see it: class `Combat`, its running flag kept equal to the pulse's.
    pub handle: pulses::Handle,
    /// The tile the combat movement last walked toward.
    pub chase: Option<Location>,
}

/// One `CombatSwingHandler` instance with its private special map.
pub struct HandlerSlot {
    pub handler: cs::Handler,
    pub specials: Specials<Live>,
    pub class: &'static str,
}

/// A pulse combat or death submitted to `GameWorld.Pulser`.
pub enum Submitted {
    /// An impact, second-arrow or delayed manual-hit pulse.
    World(WorldPulse<Live>),
    /// `Entity.faceTemporary`'s reset pulse.
    FaceReset(P),
    /// `DeathTask`'s pulse (`NodeTask.schedule`).
    Death(npc_death::Task),
    /// `PrayerManager`'s head-icon pulse.
    HeadIcon(P),
}

/// The combat state of the live world.
pub struct CombatWorld {
    pub entities: HashMap<Node, Entry>,
    pub handlers: Vec<HandlerSlot>,
    /// `CombatSwingHandler.EXPERIENCE_MOD`.
    pub experience_mod: f64,
    pub submitted: Vec<(pulses::Handle, Submitted)>,
    pub npc_masks: HashMap<Node, masks::NpcMasks>,
    pub npc_animators: HashMap<Node, Animator<masks::NpcOwner>>,
    pub attrs: HashMap<Node, HashMap<String, Attr>>,
    /// The NPCs' `Ai` with the live world as their host (aggression,
    /// behaviour, class).
    pub ai: BTreeMap<Node, Ai<Live>>,
    /// `properties.armourSet` of NPCs (a player's is in its profile).
    pub armour: HashMap<Node, Option<ArmourSet>>,
    /// `RangeWeapon.get` and `Ammunition.get`, from the start-up's
    /// `RangedConfigLoader`.
    pub range: cs::RangeData,
    /// The shared drop tables (`RareDropTable` and the six others).
    pub shared_tables: SharedTables,
    /// `npc_death`'s view of each entity's damage logs.
    pub death_logs: HashMap<Node, Shared<ImpactLog>>,
    /// Each player's `PrayerManager`.
    pub prayers: HashMap<Node, PrayerState>,
    /// Players whose weapon tab asked `CombatPulse.updateStyle`; applied at
    /// the next read of the combat pulse.
    pub style_updates: HashSet<Node>,
    /// The style of the `continueAttack` call in progress (the zone
    /// monitor's opaque style argument).
    pub zone_style: Option<CombatStyle>,
    /// Entities whose combat pulse changed in the current call.
    pub touched: HashSet<Node>,
    /// `Ammunition.AMMUNITION` and `RangeWeapon.RANGE_WEAPONS` as loaded.
    pub ranged: crate::startup::loaders::ranged::RangedTables,
    /// The state of the stand-in for Kotlin's `Random.Default`.
    pub kotlin_seed: u64,
    /// NPCs `ZoneBuilder.startup` gave the WILDERNESS AggressiveHandler.
    pub wilderness_aggressors: HashSet<Node>,
    /// `GameWorld.settings.isPvp()` (the configuration's `pvp`).
    pub pvp_world: bool,
}

impl CombatWorld {
    pub fn new() -> Self {
        let slot = |kind, class| HandlerSlot {
            handler: cs::Handler::new(kind, &[]),
            specials: Specials::default(),
            class,
        };
        Self {
            entities: HashMap::new(),
            handlers: vec![
                slot(HandlerKind::Melee, "MeleeSwingHandler"),
                slot(HandlerKind::Range, "RangeSwingHandler"),
                slot(HandlerKind::Magic, "MagicSwingHandler"),
            ],
            experience_mod: 4.0,
            submitted: Vec::new(),
            npc_masks: HashMap::new(),
            npc_animators: HashMap::new(),
            attrs: HashMap::new(),
            ai: BTreeMap::new(),
            armour: HashMap::new(),
            range: cs::RangeData::default(),
            shared_tables: SharedTables::new(),
            death_logs: HashMap::new(),
            prayers: HashMap::new(),
            style_updates: HashSet::new(),
            zone_style: None,
            touched: HashSet::new(),
            ranged: Default::default(),
            kotlin_seed: 0,
            wilderness_aggressors: HashSet::new(),
            pvp_world: false,
        }
    }
}

impl Default for CombatWorld {
    fn default() -> Self {
        Self::new()
    }
}

/// `CombatStyle.getSwingHandler()`.
pub fn style_handler(style: CombatStyle) -> usize {
    match style {
        CombatStyle::Melee => MELEE,
        CombatStyle::Range => RANGE,
        CombatStyle::Magic => MAGIC,
    }
}

impl Inner {
    pub fn is_npc_node(&self, node: Node) -> bool {
        self.npcs.rows.contains_key(&node)
    }
    pub fn is_player_node(&self, node: Node) -> bool {
        self.players.contains_key(&node)
    }
    /// `Node.isActive()`.
    pub fn node_active(&self, node: Node) -> bool {
        match self.players.get(&node) {
            Some(slot) => slot.active,
            None => self.npcs.rows.get(&node).is_some_and(|row| row.active),
        }
    }
    /// The entity's combat pulse and impact handler, made the first time
    /// they are asked for (`new CombatPulse(entity)`: the melee handler and
    /// style; an NPC's style is its `combat_style` configuration).
    pub fn combat_entry(&mut self, p: &P) -> &mut Entry {
        let node = p.node();
        if !self.combat.entities.contains_key(&node) {
            let mut state = PulseState::new(p.clone(), MELEE);
            if let Some(row) = self.npcs.rows.get(&node) {
                use crate::definitions::property::{CombatStyle as Configured, Property};
                if let Some(Property::CombatStyle(style)) = &row.properties.combat_style {
                    state.style = match style {
                        Configured::Melee => CombatStyle::Melee,
                        Configured::Range => CombatStyle::Range,
                        Configured::Magic => CombatStyle::Magic,
                    };
                }
            }
            let handle = Shared::new(pulses::Pulse::new(
                1,
                Some(Shared::new(vec![Some(node)])),
                pulses::Class::Combat,
                "core.game.node.entity.combat.CombatPulse".into(),
            ));
            handle.write().running = false;
            self.combat.entities.insert(
                node,
                Entry {
                    state,
                    impact: ImpactState::default(),
                    handle,
                    chase: None,
                },
            );
        }
        if self.combat.style_updates.remove(&node) {
            self.apply_style_update(p);
        }
        self.combat.touched.insert(node);
        self.combat
            .entities
            .get_mut(&node)
            .expect("the entry just made")
    }
    /// `CombatPulse.updateStyle()` from the weapon tab, without service
    /// calls: a spell or autocast spell is magic, otherwise the attack
    /// style's bonus type.
    fn apply_style_update(&mut self, p: &P) {
        let Ok((spell, style)) = self.with_profile(p, |profile| {
            (
                profile.properties.spell.is_some(),
                profile.properties.attack_style,
            )
        }) else {
            return;
        };
        let style = if spell {
            Some(CombatStyle::Magic)
        } else {
            style.map(|s| match s.bonus_type {
                cs::BONUS_MAGIC => CombatStyle::Magic,
                cs::BONUS_RANGE => CombatStyle::Range,
                _ => CombatStyle::Melee,
            })
        };
        if let (Some(style), Some(entry)) = (style, self.combat.entities.get_mut(&p.node())) {
            entry.state.style = style;
        }
    }
    /// `CombatPulse.isAttacking()` without service calls.
    pub fn combat_is_attacking(&self, node: Node) -> bool {
        self.combat.entities.get(&node).is_some_and(|entry| {
            entry.state.running
                && entry
                    .state
                    .victim
                    .as_ref()
                    .is_some_and(|victim| self.node_active(victim.node()))
        })
    }
    /// `Entity.getAttribute(key)` of the attributes combat reads.
    pub fn attribute(&self, node: Node, key: &str) -> Option<Attr> {
        if let Some(Attr::Entity(p)) = self.combat.attrs.get(&node).and_then(|a| a.get(key)) {
            return Some(Attr::Entity(p.clone()));
        }
        if let Some(slot) = self.players.get(&node) {
            return match slot.canonical.player.attributes().get(key) {
                Some(AttributeValue::Bool(v)) => Some(Attr::Bool(v)),
                Some(AttributeValue::Int(v)) => Some(Attr::Int(v)),
                Some(AttributeValue::Long(v)) => Some(Attr::Long(v)),
                Some(_) => None,
                None => None,
            };
        }
        self.combat
            .attrs
            .get(&node)
            .and_then(|a| a.get(key))
            .cloned()
    }
    /// Whether the entity has the attribute at all (`attributes.containsKey`).
    pub fn has_attribute(&self, node: Node, key: &str) -> bool {
        if self
            .combat
            .attrs
            .get(&node)
            .is_some_and(|a| a.contains_key(key))
        {
            return true;
        }
        self.players
            .get(&node)
            .is_some_and(|slot| slot.canonical.player.attributes().get(key).is_some())
    }
    pub fn set_attribute(&mut self, node: Node, key: &str, value: Attr) {
        if let Some(slot) = self.players.get(&node) {
            let stored = match &value {
                Attr::Bool(v) => Some(AttributeValue::Bool(*v)),
                Attr::Int(v) => Some(AttributeValue::Int(*v)),
                Attr::Long(v) => Some(AttributeValue::Long(*v)),
                Attr::Entity(_) => None,
            };
            if let Some(stored) = stored {
                slot.canonical
                    .player
                    .attributes()
                    .set_value(key, Some(stored));
                if let Some(a) = self.combat.attrs.get_mut(&node) {
                    a.remove(key);
                }
                return;
            }
            slot.canonical.player.attributes().remove(key);
        }
        self.combat
            .attrs
            .entry(node)
            .or_default()
            .insert(key.to_owned(), value);
    }
    pub fn remove_attribute(&mut self, node: Node, key: &str) {
        if let Some(slot) = self.players.get(&node) {
            slot.canonical.player.attributes().remove(key);
        }
        if let Some(a) = self.combat.attrs.get_mut(&node) {
            a.remove(key);
        }
    }
}

impl Live {
    /// Keeps each touched combat pulse's runner object and the canonical
    /// actor's view (`combat_running`, `victim`) equal to the pulse.
    pub fn sync_combat(&mut self) {
        let touched: Vec<Node> = self.inner.combat.touched.drain().collect();
        for node in touched {
            let Some(entry) = self.inner.combat.entities.get(&node) else {
                continue;
            };
            let running = entry.state.running;
            let victim = entry.state.victim.as_ref().map(|v| v.node());
            entry.handle.write().running = running;
            if let Ok(actor) = self.inner.state.actor_mut(node) {
                actor.view.combat_running = running;
                actor.view.victim = victim;
            }
        }
    }

    /// At boot: `RangeWeapon.get`/`Ammunition.get` from the start-up's
    /// `RangedConfigLoader`, and the seven shared drop tables (their
    /// `StartupListener`s, read from `<config>/shared_tables/<name>.xml`;
    /// the original runs them during the start-up, here they run after it,
    /// drawing the rare table's positions from the shared generator).
    pub fn install_combat_tables(
        &mut self,
        ranged: crate::startup::loaders::ranged::RangedTables,
        config_path: &str,
        pvp_world: bool,
    ) {
        use crate::world::drop_tables::{self as tables, shared::SharedKind};
        let mut data = cs::RangeData::default();
        for (id, w) in &ranged.weapons {
            data.weapons.insert(
                *id,
                std::sync::Arc::new(cs::RangeWeapon {
                    item_id: w.item_id,
                    attack_speed: w.attack_speed,
                    ammunition_slot: w.ammunition_slot,
                    weapon_type: w.weapon_type,
                    drop_ammo: w.drop_ammo,
                    ammunition: w.ammunition.clone(),
                }),
            );
        }
        for (id, a) in &ranged.ammunition {
            data.ammunition.insert(
                *id,
                std::sync::Arc::new(cs::Ammunition {
                    item_id: a.item_id,
                    poison_damage: a.poison_damage,
                    effect: a.effect.map(|e| cs::BoltEffect::ALL[e as usize]),
                }),
            );
        }
        self.inner.combat.range = data;
        self.inner.combat.ranged = ranged;
        self.inner.combat.pvp_world = pvp_world;
        struct Log(Vec<String>);
        impl tables::Host for Log {
            fn clue_scrolls(&self, _: tables::ClueLevel) -> Option<&[i32]> {
                None
            }
            fn log(&mut self, origin: &'static str, level: tables::LogLevel, message: &str) {
                self.0.push(format!("{origin} {level:?} {message}"));
            }
            fn stack_trace(&mut self, origin: &'static str, failure: tables::shared::ParseFailure) {
                self.0.push(format!("{origin} printStackTrace {failure:?}"));
            }
        }
        let mut log = Log(Vec::new());
        let base = config_path.trim_end_matches(['/', '\\']);
        for kind in SharedKind::ALL {
            let path = format!("{base}/shared_tables/{}.xml", kind.abbreviation());
            let text = std::fs::read_to_string(&path).ok();
            let Inner {
                combat, generators, ..
            } = &mut self.inner;
            combat
                .shared_tables
                .startup(kind, Some(&path), text.as_deref(), generators, &mut log);
        }
        for line in log.0 {
            self.inner.log_line(line);
        }
    }

    /// Hands the player's queued impacts to its prepared masks, at most the
    /// two `UpdateMasks.prepare` polls, with the hit mark's health ratio
    /// (`lifepoints * 255 / maximum`, 255 at full health).
    pub fn hand_impacts_to_masks(&mut self, p: &P) {
        let node = p.node();
        if !self
            .inner
            .combat
            .entities
            .get(&node)
            .is_some_and(|e| !e.impact.queue.is_empty())
        {
            return;
        }
        let (lifepoints, maximum) = self.health_of(node);
        let ratio = if maximum > lifepoints {
            lifepoints.wrapping_mul(255) / maximum.max(1)
        } else {
            255
        };
        let masks = self.inner.canonical(p).player.masks();
        let room = 2usize.saturating_sub(masks.queued_impacts());
        let Some(entry) = self.inner.combat.entities.get_mut(&node) else {
            return;
        };
        for _ in 0..room {
            let Some(impact) = entry.impact.queue.pop_front() else {
                break;
            };
            masks.queue_impact(impact.amount, impact.kind.map_or(0, |k| k as i32), ratio);
        }
    }

    /// `entity.attack(victim)` (`CombatPulse.attack`).
    pub fn combat_attack(&mut self, entity: &P, victim: &P) -> cs::Result<()> {
        let result = cp::pulse::attack(self, entity, Some(victim));
        self.sync_combat();
        result
    }

    /// `CombatPulse.stop()` of an entity, from its pulse manager.
    pub fn combat_stop_of(&mut self, node: Node) -> cs::Result<()> {
        let Some(p) = self.combat_entity(node) else {
            return Ok(());
        };
        let result = cp::pulse::stop(self, &p);
        self.sync_combat();
        result
    }

    /// The live entity of a node (player or NPC).
    pub fn combat_entity(&mut self, node: Node) -> Option<P> {
        if let Some(p) = self.player_of(node) {
            return Some(p);
        }
        if self.inner.npcs.rows.contains_key(&node) {
            return self.inner.state.retain_node(node).ok().map(P);
        }
        None
    }

    /// A pulse combat or death submitted to the driver's runner, if this
    /// world submitted it: the combat pulse's `Pulse.update`, an impact
    /// pulse, a face reset, the death task, the head-icon pulse.
    pub fn combat_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool, String>> {
        let owner = self
            .inner
            .combat
            .entities
            .iter()
            .find(|(_, e)| e.handle.same(pulse))
            .map(|(n, _)| *n);
        if let Some(node) = owner {
            let p = self.combat_entity(node)?;
            self.inner.combat.touched.insert(node);
            let result = cp::pulse::update(self, &p).map_err(|e| format!("{e:?}"));
            self.sync_combat();
            return Some(result);
        }
        let at = self
            .inner
            .combat
            .submitted
            .iter()
            .position(|(h, _)| h.same(pulse))?;
        let (handle, body) = self.inner.combat.submitted.remove(at);
        let result = match body {
            Submitted::World(mut world) => {
                let done = world.update(self).map_err(|e| format!("{e:?}"));
                if matches!(done, Ok(false)) {
                    self.inner
                        .combat
                        .submitted
                        .push((handle, Submitted::World(world)));
                }
                done
            }
            Submitted::FaceReset(p) => {
                // faceTemporary's pulse: face(null), then done.
                cp::Host::face(self, &p, None)
                    .map(|_| true)
                    .map_err(|e| format!("{e:?}"))
            }
            Submitted::Death(task) => {
                let result = death::death_pulse(self, &task);
                if matches!(result, Ok(false)) {
                    self.inner
                        .combat
                        .submitted
                        .push((handle, Submitted::Death(task)));
                }
                result
            }
            Submitted::HeadIcon(p) => prayer::head_icon_pulse(self, &p),
        };
        self.sync_combat();
        Some(result)
    }

    /// Submits a pulse body to `GameWorld.Pulser`.
    pub fn submit_combat(
        &mut self,
        delay: i32,
        checks: Vec<Option<Node>>,
        class: &str,
        body: Submitted,
    ) -> Result<(), String> {
        let handle = Shared::new(pulses::Pulse::new(
            delay,
            Some(Shared::new(checks)),
            pulses::Class::Other,
            class.into(),
        ));
        self.driver
            .pulser()
            .submit(Some(handle.clone()))
            .map_err(|e| format!("{e:?}"))?;
        self.inner.combat.submitted.push((handle, body));
        Ok(())
    }

    /// `ImpactHandler.manualHit(source, hit, type)` from a timer (poison,
    /// disease).
    pub fn timer_manual_hit(
        &mut self,
        owner: &P,
        source: &P,
        hit: i32,
        kind: HitsplatType,
    ) -> Result<(), String> {
        let result = cp::impact::manual_hit(self, owner, Some(source), hit, Some(kind))
            .map(|_| ())
            .map_err(|e| format!("{e:?}"));
        self.sync_combat();
        result
    }

    /// `NPC.isPoisonImmune()`: the definition's `poison_immune` configuration.
    pub fn npc_poison_immune(&mut self, npc: &P) -> Result<bool, String> {
        use crate::definitions::property::Property;
        let row = self
            .inner
            .npcs
            .rows
            .get(&npc.node())
            .ok_or("NPC.isPoisonImmune of a node that is no NPC")?;
        let handlers = row.definition.read().handlers.clone();
        let value = handlers.read().get("poison_immune").cloned();
        match value {
            None | Some(Property::Null) => Ok(false),
            Some(Property::Bool(v)) => Ok(v),
            Some(_) => Err("poison_immune is not a boolean".into()),
        }
    }
}
