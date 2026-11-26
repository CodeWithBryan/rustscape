//! Dragons in the live world: `world::dragonfire` (DragonfireSwingHandler,
//! MultiSwingHandler, the protection checks) with the live world as its
//! `Host`, and the dragon classes of `dragonfire::dragons`, which the
//! start-up constructs (`creatures::classes::WorldClass::Dragon`).
//!
//! Each class's static `DRAGONFIRE` attack is made the first time a dragon
//! of the class is asked for its swing handler, and each dragon's
//! `combatAction` (a MultiSwingHandler over two melee attacks and the
//! dragonfire) the first time that dragon is; making them draws nothing, so
//! only object creation moves from construction to first use. Every handler
//! instance is a slot of the combat handler table whose subclass this module
//! keeps; `combat_pulse::Host::overrides` answers `dragonfire::overrides` for
//! it. MonkNPC's static `CombatAction` is a slot too, whose `swing` is
//! `npc_classes::classes::monk_swing` (super::subclasses).
//!
//! The dragonfire shield's own handler and listener (`dragonfire::shield`:
//! operate, charges, `DFSItemPlugin`) are not installed: their services
//! refuse by name. Ported without a differential corpus of the live binding
//! itself: the library is replayed against dragonfire-oracle.
use super::{
    P,
    combat::{Attr, HandlerSlot},
    host::Live,
};
use crate::world::{
    combat_pulse::{Overrides, Specials},
    combat_swing::{self as cs, CombatStyle, Error, Handler, Result},
    creatures::classes::WorldClass,
    doors::Location,
    dragonfire::{
        self as df, Container, DragonProjectile, ItemValue, Subclass, SwitchAttack, Value,
        dragons::{self, DragonClass},
    },
    player_walk::Node,
};
use std::collections::HashMap;

fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}
fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// The handler instances this module made, and the dragons' attacks.
#[derive(Default)]
pub struct Dragons {
    /// The subclass of each handler instance made here, by handler slot.
    pub subclasses: HashMap<usize, Subclass<Live>>,
    /// Each class's static `DRAGONFIRE`.
    pub dragonfire: HashMap<DragonClass, SwitchAttack<Live>>,
    /// Each dragon's `combatAction`.
    pub actions: HashMap<Node, usize>,
    /// `MonkNPC.COMBAT`.
    pub monk: Option<usize>,
    /// `ServerConstants.BETTER_DFS`.
    pub better_dfs: bool,
}

/// A dragon's `getSwingHandler(swing)`: its `combatAction`.
pub fn combat_action(live: &mut Live, npc: &P, class: DragonClass) -> Result<usize> {
    if let Some(handler) = live.content.dragons.actions.get(&npc.node()) {
        return Ok(*handler);
    }
    let attack = match live.content.dragons.dragonfire.get(&class) {
        Some(attack) => attack.clone(),
        None => {
            let attack = dragons::dragonfire(live, class);
            live.content
                .dragons
                .dragonfire
                .insert(class, attack.clone());
            attack
        }
    };
    let handler = dragons::combat_action(live, class, &attack)?;
    live.content.dragons.actions.insert(npc.node(), handler);
    Ok(handler)
}

/// `MonkNPC.COMBAT`: one MeleeSwingHandler subclass object for every monk.
pub fn monk_handler(live: &mut Live) -> usize {
    if let Some(handler) = live.content.dragons.monk {
        return handler;
    }
    live.inner.combat.handlers.push(HandlerSlot {
        handler: Handler::new(cs::HandlerKind::Melee, &[]),
        specials: Specials::default(),
        class: "content.global.handlers.npc.MonkNPC$CombatAction",
        special: None,
    });
    let handler = live.inner.combat.handlers.len() - 1;
    live.content.dragons.monk = Some(handler);
    handler
}

impl Live {
    /// The overriding members of a handler instance made by this module.
    pub fn content_overrides(&self, handler: &usize) -> Option<Overrides<Live>> {
        if self.content.dragons.monk == Some(*handler) {
            let mut o = Overrides::none();
            o.swing = Some(super::subclasses::monk_swing);
            return Some(o);
        }
        self.content
            .dragons
            .subclasses
            .get(handler)
            .map(df::overrides::<Live>)
    }
}

impl df::Host for Live {
    fn subclass(&self, handler: &usize) -> Option<&Subclass<Self>> {
        self.content.dragons.subclasses.get(handler)
    }
    fn subclass_mut(&mut self, handler: &usize) -> Option<&mut Subclass<Self>> {
        self.content.dragons.subclasses.get_mut(handler)
    }
    fn style_handler(&self, style: CombatStyle) -> usize {
        super::combat::style_handler(style)
    }
    fn new_handler(&mut self, base: Handler, subclass: Subclass<Self>) -> usize {
        let class = match &subclass {
            Subclass::Dragonfire(_) => {
                "content.global.handlers.item.equipment.special.DragonfireSwingHandler"
            }
            Subclass::Multi(_) => "core.game.node.entity.combat.MultiSwingHandler",
        };
        self.inner.combat.handlers.push(HandlerSlot {
            handler: base,
            specials: Specials::default(),
            class,
            special: None,
        });
        let handler = self.inner.combat.handlers.len() - 1;
        self.content.dragons.subclasses.insert(handler, subclass);
        handler
    }
    fn set_item_charge(&mut self, item: &Self::Item, charge: i32) {
        item.write().set_charge(charge);
    }
    fn item_slot(&self, item: &Self::Item) -> i32 {
        // Item.getSlot(): the Node index the container gave it.
        item.read().index
    }
    fn better_dfs(&self) -> bool {
        self.content.dragons.better_dfs
    }

    fn timer_active(&mut self, entity: &P, identifier: &str) -> Result<bool> {
        // A player's or an NPC's timer manager (super::timers).
        self.has_timer_of(entity.node(), identifier).map_err(bind)
    }
    fn dragonfire_immunity(&mut self, entity: &P) -> Result<bool> {
        df::Host::timer_active(self, entity, "dragonfire:immunity")
    }
    fn npc_dragonfire_protection(&mut self, entity: &P, _: bool) -> Result<i32> {
        // The dragon classes answer every token; NPC.getDragonfireProtection 0.
        let class = self
            .inner
            .npcs
            .rows
            .get(&entity.node())
            .ok_or(Error::Null)?
            .class;
        Ok(match class {
            Some(WorldClass::Dragon(_)) => dragons::PROTECTION,
            _ => 0,
        })
    }
    fn has_fire_resistance(&mut self, entity: &P) -> Result<bool> {
        // getAttribute("fire:immune", 0) >= GameWorld.getTicks().
        let value = match self.inner.attribute(entity.node(), "fire:immune") {
            None => 0,
            Some(Attr::Int(v)) => v,
            Some(_) => return Err(Error::Binding("fire:immune is not an integer".into())),
        };
        Ok(value >= self.inner.ticks)
    }
    fn set_attribute_value(&mut self, entity: &P, key: &str, value: Value) -> Result<()> {
        let value = match value {
            Value::Int(v) => Attr::Int(v),
            Value::Bool(v) => Attr::Bool(v),
        };
        self.inner.set_attribute(entity.node(), key, value);
        Ok(())
    }
    fn update_bonuses(&mut self, _: &P) -> Result<()> {
        refuse("EquipmentContainer.updateBonuses from a dragonfire shield's charge")
    }
    fn face_location(&mut self, entity: &P, at: Option<[i32; 3]>) -> Result<()> {
        if self.inner.is_npc_node(entity.node()) {
            let at = at.map(|[x, y, z]| Location::new(x, y, z));
            return self
                .npc_face_location(entity.node(), at)
                .map(|_| ())
                .map_err(bind);
        }
        refuse("a player's faceLocation from a dragonfire shield's charge")
    }
    fn send_dragon_projectile(&mut self, request: DragonProjectile<'_, Self>) -> Result<()> {
        match request {
            DragonProjectile::Copy {
                template,
                source,
                victim,
                speed,
            } => {
                // template.copy(source, victim, speed): speed + distance *
                // multiplier from the source's to the victim's location, the
                // template's other numbers, then send().
                let victim = victim.ok_or(Error::Null)?;
                let from = self.cloc(source).ok_or(Error::Null)?;
                let to = self.cloc(victim).ok_or(Error::Null)?;
                let speed = (f64::from(template.speed)
                    + cs::distance(from.coordinates(), to.coordinates()) * speed)
                    as i32;
                let mut projectile = self.projectile_create(
                    source,
                    Some(victim),
                    template.id,
                    template.start_height,
                    template.end_height,
                    template.start_delay,
                    speed,
                    template.angle,
                );
                projectile.distance = template.distance;
                self.projectile_send(projectile)
            }
            DragonProjectile::Transform { .. } => {
                refuse("Projectile.transform of a switching attack (no installed NPC uses one)")
            }
        }
    }
    fn set_varp(&mut self, _: &P, _: i32, _: i32) -> Result<()> {
        refuse("the dragonfire shield's varp (shield operate is not installed)")
    }
    fn is_locked(&mut self, _: &P, _: &str) -> Result<bool> {
        refuse("the dragonfire shield's lock (shield operate is not installed)")
    }
    fn lock_key(&mut self, _: &P, _: &str, _: i32) -> Result<()> {
        refuse("the dragonfire shield's lock (shield operate is not installed)")
    }
    fn delay_entity(&mut self, _: &P, _: i32) -> Result<()> {
        refuse("delayEntity from the dragonfire shield (not installed)")
    }
    fn replace_slot(
        &mut self,
        _: &P,
        _: i32,
        _: ItemValue,
        _: Option<ItemValue>,
        _: Container,
    ) -> Result<()> {
        refuse("replaceSlot from the dragonfire shield (not installed)")
    }
    fn inventory_item(&mut self, _: &P, _: i32) -> Result<Option<Self::Item>> {
        refuse("the dragonfire shield's item plugin (not installed)")
    }
    fn drop_item(&mut self, _: &P, _: &Self::Item) -> Result<bool> {
        refuse("the dragonfire shield's item plugin (not installed)")
    }
    fn register_item_plugin(&mut self, _: i32) -> Result<()> {
        refuse("DFSItemPlugin registration (not installed)")
    }
    fn listen(&mut self, _: &[i32], _: &str) -> Result<()> {
        refuse("DragonfireShieldListener (not installed)")
    }
}
