//! Equipment registers and the two weapon-class swing handlers.
//!
//! Ports, from content/global/handlers/item/equipment:
//!
//! - `BarrowsEquipment` ([`barrows`]): the 24 pieces, their degradation
//!   states, repair cost and the grave's durability loss;
//! - `EquipmentDegrader` with `BarrowsEquipmentRegister`,
//!   `PVPEquipmentRegister` and `CrystalEquipmentRegister` ([`degrader`]);
//! - `HastaEquipListener` ([`hasta`]);
//! - special/`ChinchompaSwingHandler` ([`chinchompa`]) and
//!   special/`SalamanderSwingHandler` ([`salamander`]).
//!
//! The Barrows set effects themselves are `ArmourSet` (combat_swing and
//! combat_pulse); the degraded states this module lists are the ids
//! `ArmourSet` already accepts.
//!
//! The two handlers are subclasses of `RangeSwingHandler` and of the
//! abstract `CombatSwingHandler`. A host keeps a [`cs::Handler`] for each
//! instance and its [`Subclass`]; [`overrides`] answers
//! `combat_pulse::Host::overrides` with the members the class overrides, so
//! `CombatPulse`, the impact pulse and the base bodies reach them where the
//! original's virtual calls do. Everything outside the classes is a required
//! [`Host`] service with no default. See docs/equipment-registers.md.
pub mod barrows;
pub mod chinchompa;
pub mod degrader;
pub mod hasta;
pub mod salamander;

use crate::world::combat_pulse::Overrides;
use crate::world::combat_swing::{CombatStyle, Error, Handler, HandlerKind, Result};
use crate::world::dragonfire::{self as df, ItemValue};

/// The subclass of a handler instance made by this module, with its fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subclass {
    /// `ChinchompaSwingHandler` (no fields of its own).
    Chinchompa,
    /// `SalamanderSwingHandler`: its private, mutable `style`.
    Salamander { style: CombatStyle },
}

impl Subclass {
    /// The runtime class simple name.
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Chinchompa => "ChinchompaSwingHandler",
            Self::Salamander { .. } => "SalamanderSwingHandler",
        }
    }

    /// The base-class fields of a new instance: `ChinchompaSwingHandler()`
    /// is `RangeSwingHandler()`; `SalamanderSwingHandler(style)` passes its
    /// style as the type of the abstract class (the singleton's is RANGE).
    pub fn instance(self) -> Handler {
        match self {
            Self::Chinchompa => Handler::new(HandlerKind::Range, &[]),
            Self::Salamander { style } => {
                let mut handler = Handler::new(HandlerKind::Range, &[]);
                handler.style = Some(style);
                handler
            }
        }
    }

    /// The singletons `Player.getSwingHandler` answers:
    /// `ChinchompaSwingHandler.getInstance()` and
    /// `SalamanderSwingHandler.INSTANCE` (`SalamanderSwingHandler(RANGE)`).
    pub const CHINCHOMPA: Self = Self::Chinchompa;
    pub const SALAMANDER: Self = Self::Salamander {
        style: CombatStyle::Range,
    };
}

/// Everything these classes reach beyond the combat slices', the special
/// attacks' and the dragonfire services. The first group are plain accessors
/// to state; each fallible method is one original call, and none has a
/// default.
pub trait Host: df::Host {
    /// The subclass of a handler instance made by this module.
    fn equipment_subclass(&self, handler: &Self::HandlerRef) -> Option<Subclass>;
    /// Writes the salamander's private `style` field.
    fn set_salamander_style(&mut self, handler: &Self::HandlerRef, style: CombatStyle);

    /// `equipment.remove(item)` (`Container.remove(Item)`).
    fn equipment_remove(&mut self, player: &Self::Entity, item: &Self::Item) -> Result<bool>;
    /// `equipment.remove(new Item(id, amount))`.
    fn equipment_remove_value(&mut self, player: &Self::Entity, item: ItemValue) -> Result<bool>;
    /// `equipment.add(item, inventorySlot, fire, fromInventory)`.
    fn equipment_add(
        &mut self,
        player: &Self::Entity,
        item: ItemValue,
        inventory_slot: i32,
        fire: bool,
        from_inventory: bool,
    ) -> Result<bool>;
    /// `equipment.refresh()`.
    fn equipment_refresh(&mut self, player: &Self::Entity) -> Result<()>;
    /// ContentAPI `addItemOrDrop(player, id, amount)`.
    fn add_item_or_drop(&mut self, player: &Self::Entity, id: i32, amount: i32) -> Result<()>;
    /// ContentAPI `getAttribute(entity, key, default)` of an `Int`.
    fn attribute_int(&mut self, entity: &Self::Entity, key: &str, default: i32) -> Result<i32>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn send_dialogue(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `InteractionListener.onEquip(ids) { .. }` of HastaEquipListener.
    fn on_equip(&mut self, ids: &[i32]) -> Result<()>;
}

/// The members a subclass of this module overrides, for
/// `combat_pulse::Host::overrides`.
pub fn overrides<H: Host>(subclass: Subclass) -> Overrides<H> {
    let mut o = Overrides::none();
    match subclass {
        Subclass::Chinchompa => {
            o.swing = Some(chinchompa::swing::<H>);
            o.adjust_battle_state = Some(chinchompa::adjust_battle_state::<H>);
            o.add_experience = Some(chinchompa::add_experience::<H>);
            o.visualize = Some(chinchompa::visualize::<H>);
            o.impact = Some(chinchompa::impact::<H>);
            o.visualize_impact = Some(chinchompa::visualize_impact::<H>);
        }
        Subclass::Salamander { .. } => {
            o.can_swing = Some(salamander::can_swing::<H>);
            o.swing = Some(salamander::swing::<H>);
            o.impact = Some(salamander::impact::<H>);
            o.visualize = Some(salamander::visualize::<H>);
            o.visualize_impact = Some(salamander::visualize_impact::<H>);
            o.calculate_accuracy = Some(salamander::calculate_accuracy::<H>);
            o.calculate_hit = Some(salamander::calculate_hit::<H>);
            o.add_experience = Some(salamander::add_experience::<H>);
            o.calculate_defence = Some(salamander::calculate_defence::<H>);
            o.set_multiplier = Some(salamander::set_multiplier::<H>);
            // Inherited from CombatSwingHandler itself, not RangeSwingHandler.
            o.adjust_battle_state = Some(salamander::base_adjust::<H>);
            o.post_swing = Some(salamander::no_post_swing::<H>);
            o.armour_set = Some(salamander::no_armour_set::<H>);
        }
    }
    o
}

/// The instance's subclass, failing for an instance of another class.
pub(crate) fn subclass_of<H: Host>(h: &H, r: &H::HandlerRef) -> Result<Subclass> {
    h.equipment_subclass(r)
        .ok_or_else(|| Error::Binding("not an equipment register handler".into()))
}

/// `RandomFunction.random(maxValue)`: no draw unless the bound is positive.
pub(crate) fn random<H: Host>(h: &mut H, max_value: i32) -> Result<i32> {
    if max_value <= 0 {
        return Ok(0);
    }
    h.next_int(max_value)
}
