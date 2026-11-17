//! Dragonfire and the NPC attacks that switch between styles.
//!
//! Ports content/global/handlers/item/equipment/special/DragonfireSwingHandler
//! ([`handler`]), core/game/node/entity/combat/equipment/FireType
//! ([`fire_type`]), SwitchAttack ([`SwitchAttack`]), MultiSwingHandler.kt and
//! MixedSwingHandlers.kt ([`multi`]), the dragonfire protection checks of
//! ContentAPI (`calculateDragonfireMaxHit`, `hasDragonfireShieldProtection`)
//! and `Player.getDragonfireProtection` ([`api`]), and
//! content/global/handlers/item/DragonfireShieldListener.kt with its
//! `DFSItemPlugin` ([`shield`]), and content/global/handlers/npc's
//! `ChromaticDragonNPC`, `MetalDragonNPC` and `ChromaticDragonBehavior`
//! ([`dragons`]).
//!
//! `DragonfireSwingHandler` and `MultiSwingHandler` extend the abstract
//! `CombatSwingHandler` directly. A host keeps a [`cs::Handler`] for each
//! instance (its mutable `type` and flags) and the subclass's own fields
//! ([`Subclass`]); [`overrides`] answers `combat_pulse::Host::overrides` with
//! every member the class overrides, so `CombatPulse`, the impact pulse and
//! the base classes reach these bodies exactly where the original's virtual
//! calls do. The MixedSwingHandlers objects override `calculateDefence` only:
//! they are [`cs::Handler::mixed`] instances of the melee or range class.
//! Everything outside the classes is a required [`Host`] service with no
//! default. See docs/dragonfire.md.
pub mod api;
pub mod dragons;
pub mod fire_type;
pub mod handler;
pub mod multi;
pub mod shield;

use crate::world::combat_pulse::{self as cp, Overrides};
use crate::world::combat_swing::{BattleState, CombatStyle, Error, Graphics, Handler, Result};
use crate::world::special_attacks as sa;

/// `Projectile.create(source, victim, id, startHeight, endHeight, startDelay,
/// speed, angle, distance)` kept as a template on an attack.
pub struct ProjectileTemplate<H: cp::Host> {
    pub source: Option<H::Entity>,
    pub victim: Option<H::Entity>,
    pub id: i32,
    pub start_height: i32,
    pub end_height: i32,
    pub start_delay: i32,
    pub speed: i32,
    pub angle: i32,
    pub distance: i32,
}

impl<H: cp::Host> Clone for ProjectileTemplate<H> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            victim: self.victim.clone(),
            ..*self
        }
    }
}

impl<H: cp::Host> ProjectileTemplate<H> {
    /// The nine-argument `Projectile.create`.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        source: Option<H::Entity>,
        victim: Option<H::Entity>,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        speed: i32,
        angle: i32,
        distance: i32,
    ) -> Self {
        Self {
            source,
            victim,
            id,
            start_height,
            end_height,
            start_delay,
            speed,
            angle,
            distance,
        }
    }
}

/// `SwitchAttack`: one attack a switching handler can pick.
pub struct SwitchAttack<H: cp::Host> {
    /// `handler`; `setHandler` assigns it after construction.
    pub handler: Option<H::HandlerRef>,
    pub animation: Option<H::Animation>,
    pub start_graphic: Option<Graphics>,
    pub end_graphic: Option<Graphics>,
    pub projectile: Option<ProjectileTemplate<H>>,
    /// `useHandler`: the handler visualises and handles the attack.
    pub use_handler: bool,
    /// `maximumHit`; -1 asks the handler.
    pub maximum_hit: i32,
}

impl<H: cp::Host> Clone for SwitchAttack<H> {
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            animation: self.animation.clone(),
            start_graphic: self.start_graphic,
            end_graphic: self.end_graphic,
            projectile: self.projectile.clone(),
            use_handler: self.use_handler,
            maximum_hit: self.maximum_hit,
        }
    }
}

impl<H: cp::Host> SwitchAttack<H> {
    /// `SwitchAttack(handler, animation, startGraphic, endGraphic, projectile)`;
    /// the shorter constructors pass null for what they leave out.
    pub fn new(
        handler: Option<H::HandlerRef>,
        animation: Option<H::Animation>,
        start_graphic: Option<Graphics>,
        end_graphic: Option<Graphics>,
        projectile: Option<ProjectileTemplate<H>>,
    ) -> Self {
        Self {
            handler,
            animation,
            start_graphic,
            end_graphic,
            projectile,
            use_handler: false,
            maximum_hit: -1,
        }
    }

    /// `canSelect(entity, victim, state)`: the base class always answers true.
    pub fn can_select(&self) -> bool {
        true
    }
}

/// `SwitchAttack(style)`: the style's singleton and no visuals.
pub fn style_attack<H: Host>(h: &H, style: CombatStyle) -> SwitchAttack<H> {
    SwitchAttack::new(Some(h.style_handler(style)), None, None, None, None)
}

/// `getStyle()`: `handler.getType()`, failing on a null handler.
pub fn attack_style<H: Host>(h: &mut H, attack: &SwitchAttack<H>) -> Result<Option<CombatStyle>> {
    let r = attack.handler.clone().ok_or(Error::Null)?;
    Ok(h.handler(&r).style)
}

/// The anonymous `DragonfireSwingHandler` of the dragonfire shield's
/// "operate": the lambda's captured player and shield item, and the
/// listener's recharge field.
pub struct Shield<H: cp::Host> {
    pub player: H::Entity,
    pub item: H::Item,
    pub recharge: i32,
}

impl<H: cp::Host> Clone for Shield<H> {
    fn clone(&self) -> Self {
        Self {
            player: self.player.clone(),
            item: self.item.clone(),
            recharge: self.recharge,
        }
    }
}

/// `DragonfireSwingHandler`'s fields (and the shield subclass's captures).
pub struct Dragonfire<H: cp::Host> {
    pub melee_range: bool,
    pub maximum_hit: i32,
    /// `attack`: only its visuals are read, never its handler.
    pub attack: Option<SwitchAttack<H>>,
    /// `fire`: false for icy (wyvern) breath.
    pub fire: bool,
    /// `Some` for the dragonfire shield's anonymous subclass.
    pub shield: Option<Shield<H>>,
}

impl<H: cp::Host> Clone for Dragonfire<H> {
    fn clone(&self) -> Self {
        Self {
            attack: self.attack.clone(),
            shield: self.shield.clone(),
            ..*self
        }
    }
}

/// `MultiSwingHandler`'s fields. `current` and `next` index `attacks`: the
/// original only ever assigns elements of that array to them.
pub struct Multi<H: cp::Host> {
    pub attacks: Vec<SwitchAttack<H>>,
    /// `isMeleeDistance`.
    pub melee_distance: bool,
    pub current: usize,
    pub next: usize,
}

impl<H: cp::Host> Clone for Multi<H> {
    fn clone(&self) -> Self {
        Self {
            attacks: self.attacks.clone(),
            ..*self
        }
    }
}

/// The subclass of a handler instance, with its own fields.
pub enum Subclass<H: cp::Host> {
    Dragonfire(Dragonfire<H>),
    Multi(Multi<H>),
}

/// A projectile one of these classes sends.
pub enum DragonProjectile<'a, H: cp::Host> {
    /// `template.copy(source, victim, speedMultiplier).send()`.
    Copy {
        template: &'a ProjectileTemplate<H>,
        source: &'a H::Entity,
        victim: Option<&'a H::Entity>,
        speed: f64,
    },
    /// `template.transform(source, victim, npc, baseSpeed, modifiedSpeed).send()`:
    /// the original changes the shared template in place.
    Transform {
        template: &'a ProjectileTemplate<H>,
        source: &'a H::Entity,
        victim: Option<&'a H::Entity>,
        npc: bool,
        base_speed: i32,
        modified_speed: i32,
    },
}

/// An attribute value these classes store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i32),
    Bool(bool),
}

/// ContentAPI's `Container`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Inventory,
    Equipment,
}

impl Container {
    pub fn name(self) -> &'static str {
        match self {
            Self::Inventory => "INVENTORY",
            Self::Equipment => "EQUIPMENT",
        }
    }
}

/// `new Item(id, amount, charge)`.
pub type ItemValue = (i32, i32, i32);

/// Everything these classes reach beyond the combat slices' and the special
/// attacks' services. The first group are plain accessors to state and class
/// identity; each fallible method is one original call, and none has a
/// default.
pub trait Host: sa::Host {
    /// The subclass of a handler instance made by this module.
    fn subclass(&self, handler: &Self::HandlerRef) -> Option<&Subclass<Self>>;
    fn subclass_mut(&mut self, handler: &Self::HandlerRef) -> Option<&mut Subclass<Self>>;
    /// `CombatStyle.X.getSwingHandler()`.
    fn style_handler(&self, style: CombatStyle) -> Self::HandlerRef;
    /// A new handler instance of a subclass with its base fields.
    fn new_handler(&mut self, base: Handler, subclass: Subclass<Self>) -> Self::HandlerRef;
    /// `Item.setCharge(charge)`: the item object's packed id and charge.
    fn set_item_charge(&mut self, item: &Self::Item, charge: i32);
    /// `Item.getSlot()`.
    fn item_slot(&self, item: &Self::Item) -> i32;
    /// `ServerConstants.BETTER_DFS`.
    fn better_dfs(&self) -> bool;

    /// `hasTimerActive(entity, identifier)`.
    fn timer_active(&mut self, entity: &Self::Entity, identifier: &str) -> Result<bool>;
    /// `hasTimerActive<DragonFireImmunity>(entity)`.
    fn dragonfire_immunity(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `entity.getDragonfireProtection(fire)` of a non-player entity.
    fn npc_dragonfire_protection(&mut self, entity: &Self::Entity, fire: bool) -> Result<i32>;
    /// `Entity.hasFireResistance()`.
    fn has_fire_resistance(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `setAttribute(key, value)`.
    fn set_attribute_value(&mut self, entity: &Self::Entity, key: &str, value: Value)
    -> Result<()>;
    /// `EquipmentContainer.updateBonuses(player)`.
    fn update_bonuses(&mut self, player: &Self::Entity) -> Result<()>;
    /// `Entity.faceLocation(location)`.
    fn face_location(&mut self, entity: &Self::Entity, at: Option<[i32; 3]>) -> Result<()>;
    fn send_dragon_projectile(&mut self, request: DragonProjectile<'_, Self>) -> Result<()>;
    /// ContentAPI `setVarp(player, index, value)`.
    fn set_varp(&mut self, player: &Self::Entity, index: i32, value: i32) -> Result<()>;
    /// `locks.isLocked(key)`.
    fn is_locked(&mut self, entity: &Self::Entity, key: &str) -> Result<bool>;
    /// `locks.lock(key, ticks)`.
    fn lock_key(&mut self, entity: &Self::Entity, key: &str, ticks: i32) -> Result<()>;
    /// ContentAPI `delayEntity(entity, ticks)`.
    fn delay_entity(&mut self, entity: &Self::Entity, ticks: i32) -> Result<()>;
    /// ContentAPI `replaceSlot(player, slot, item, currentItem, container)`.
    fn replace_slot(
        &mut self,
        player: &Self::Entity,
        slot: i32,
        item: ItemValue,
        current: Option<ItemValue>,
        container: Container,
    ) -> Result<()>;
    /// `inventory.get(slot)`.
    fn inventory_item(&mut self, player: &Self::Entity, slot: i32) -> Result<Option<Self::Item>>;
    /// `DropListener.drop(player, item)`.
    fn drop_item(&mut self, player: &Self::Entity, item: &Self::Item) -> Result<bool>;
    /// `ItemPlugin.register(id)`.
    fn register_item_plugin(&mut self, id: i32) -> Result<()>;
    /// `on(ids, IntType.ITEM, option) { .. }` of an `InteractionListener`.
    fn listen(&mut self, ids: &[i32], option: &str) -> Result<()>;
}

/// The members a subclass of this module overrides, for
/// `combat_pulse::Host::overrides`.
pub fn overrides<H: Host>(subclass: &Subclass<H>) -> Overrides<H> {
    let mut o = Overrides::none();
    match subclass {
        Subclass::Dragonfire(d) => {
            o.can_swing = Some(handler::can_swing::<H>);
            o.swing = Some(handler::swing_member::<H>);
            o.visualize = Some(handler::visualize::<H>);
            o.visualize_impact = Some(handler::visualize_impact::<H>);
            o.impact = Some(handler::impact::<H>);
            o.adjust_battle_state = Some(handler::adjust_battle_state::<H>);
            o.formatted_hit = Some(handler::formatted_hit::<H>);
            o.calculate_accuracy = Some(handler::calculate_accuracy::<H>);
            o.calculate_hit = Some(handler::calculate_hit::<H>);
            o.calculate_defence = Some(handler::calculate_defence::<H>);
            o.set_multiplier = Some(handler::set_multiplier::<H>);
            o.add_experience = Some(base_add_experience::<H>);
            o.post_swing = Some(no_post_swing::<H>);
            o.armour_set = Some(no_armour_set::<H>);
            if d.shield.is_some() {
                o.swing = Some(shield::swing_member::<H>);
                o.visualize_impact = Some(shield::visualize_impact::<H>);
            }
        }
        Subclass::Multi(_) => {
            o.can_swing = Some(multi::can_swing::<H>);
            o.swing = Some(multi::swing::<H>);
            o.visualize = Some(multi::visualize::<H>);
            o.impact = Some(multi::impact::<H>);
            o.adjust_battle_state = Some(multi::adjust_battle_state::<H>);
            o.add_experience = Some(multi::add_experience::<H>);
            o.post_swing = Some(multi::post_swing::<H>);
            o.visualize_impact = Some(multi::visualize_impact::<H>);
            o.calculate_accuracy = Some(multi::calculate_accuracy::<H>);
            o.calculate_hit = Some(multi::calculate_hit::<H>);
            o.calculate_defence = Some(multi::calculate_defence::<H>);
            o.set_multiplier = Some(multi::set_multiplier::<H>);
            o.armour_set = Some(no_armour_set::<H>);
        }
    }
    o
}

/// `CombatSwingHandler.addExperience`, which a direct subclass inherits.
fn base_add_experience<H: Host>(
    h: &mut H,
    _: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&BattleState<H>>,
) -> Result<()> {
    cp::handler::base_add_experience(h, entity, victim, state)
}

/// `CombatSwingHandler.postSwing`: an empty hook.
fn no_post_swing<H: Host>(
    _: &mut H,
    _: &H::HandlerRef,
    _: Option<&H::Entity>,
    _: Option<&H::Entity>,
    _: &BattleState<H>,
) -> Result<()> {
    Ok(())
}

/// `CombatSwingHandler.getArmourSet`: null.
fn no_armour_set<H: Host>(
    _: &mut H,
    _: &H::HandlerRef,
    _: Option<&H::Entity>,
) -> Result<Option<crate::world::combat_swing::ArmourSet>> {
    Ok(None)
}

/// `RandomFunction.random(maxValue)`: no draw unless the bound is positive.
pub(crate) fn random<H: Host>(h: &mut H, max_value: i32) -> Result<i32> {
    if max_value <= 0 {
        return Ok(0);
    }
    h.next_int(max_value)
}

/// `RandomFunction.randomize(value)`.
pub(crate) fn randomize<H: Host>(h: &mut H, value: i32) -> Result<i32> {
    if value < 1 {
        return Ok(0);
    }
    h.next_int(value)
}

/// `Graphics.create(id)` and `new Graphics(id, height)`.
pub(crate) const fn gfx(id: i32, height: i32) -> Graphics {
    Graphics {
        id,
        height,
        delay: 0,
    }
}
