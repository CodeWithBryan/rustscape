//! Joins the gathering services to the native skills and item containers.
//!
//! [`Host`] is a [`Context`] that answers, from real native state, the
//! requests whose original target is `Skills`, the inventory `Container`, the
//! equipment slots or an item definition, with the pinned ContentAPI bodies
//! where the original goes through one. Every other request is handed to the
//! owner's `required` context unchanged, and the callbacks the native skills
//! and containers make themselves (packets, level-up, XP events, listeners)
//! go to the owner's `skill_services` and `item_services`. Nothing here
//! supplies a default for a system that has no native port.
//!
//! Not bound: scripts and clocks, pulses, the listener registry, attributes,
//! messages, scenery, familiars, perks, diaries, generators, `inEquipment`
//! (it needs the item configuration's equip slot), bank and beast-of-burden
//! containers, and `EquipmentContainer`'s own overrides: the equipment here is
//! a base container of fourteen slots.
use super::service::{Arg, Context, Error, Result, Service, Task};
use crate::{
    definitions::item::{Registry, Shared},
    inventory::{
        self,
        container::{self, Container},
        item::{Handle, Item},
    },
    player::skills::{self, Effect, EntityKind, Skills},
};

/// `EquipmentSlot` constants in declaration order; the ordinal is the slot.
pub const EQUIPMENT_SLOTS: [&str; 14] = [
    "HEAD", "CAPE", "NECK", "WEAPON", "CHEST", "SHIELD", "HIDDEN_1", "LEGS", "HIDDEN_2", "HANDS",
    "FEET", "HIDDEN_3", "RING", "AMMO",
];

pub struct Host<'a, G, S, C> {
    pub skills: &'a mut Skills,
    pub inventory: &'a mut Container,
    pub equipment: &'a mut Container,
    pub definitions: &'a Registry,
    /// Every gathering service that is not answered here.
    pub required: &'a mut G,
    /// What the native skills ask of their entity and world.
    pub skill_services: &'a mut S,
    /// Container listeners and ground items.
    pub item_services: &'a mut C,
}

/// The player's skills over the bound equipment: `player.getEquipment()
/// .containsItem(new Item(id))` is answered here, the rest by the owner.
struct SkillBridge<'a, S> {
    equipment: &'a Container,
    definitions: &'a Registry,
    inner: &'a mut S,
}
impl<S: skills::Context> skills::Context for SkillBridge<'_, S> {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        self.inner.version()
    }
    fn tick(&mut self) -> i32 {
        self.inner.tick()
    }
    fn world_tick(&mut self) -> i32 {
        self.inner.world_tick()
    }
    fn combat_level(&self) -> i32 {
        self.inner.combat_level()
    }
    fn summoning_combat_level(&self) -> i32 {
        self.inner.summoning_combat_level()
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        self.inner.npc_combat_level()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        self.inner.damage_spong_active()
    }
    fn assisted_level(&mut self, skill: i32) -> skills::Result<Option<i32>> {
        self.inner.assisted_level(skill)
    }
    fn translate_experience(
        &mut self,
        state: &mut Skills,
        skill: i32,
        amount: f64,
        multiplier: f64,
    ) -> skills::Result<bool> {
        self.inner
            .translate_experience(state, skill, amount, multiplier)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        self.inner.brawling_present()
    }
    fn brawling_for_skill(&mut self, skill: i32) -> skills::Result<Option<i32>> {
        self.inner.brawling_for_skill(skill)
    }
    fn equipment_contains(&mut self, id: i32) -> skills::Result<bool> {
        let item = Shared::new(Item::new(id, 1, 1000, self.definitions));
        self.equipment
            .contains_item(Some(&item))
            .map_err(|_| skills::Error::Binding)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        self.inner.brawling_bonus()
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        self.inner.tutorial_complete()
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        self.inner.artificial()
    }
    fn username(&mut self) -> skills::Result<String> {
        self.inner.username()
    }
    fn permadeath_present(&self) -> bool {
        self.inner.permadeath_present()
    }
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        self.inner.effect(state, effect)
    }
}

fn skill_error(error: skills::Error) -> Error {
    match error {
        skills::Error::Index => Error::IndexOutOfBounds,
        skills::Error::Null => Error::NullPointer,
        skills::Error::Type => Error::ClassCast,
        skills::Error::Number | skills::Error::Binding => Error::Service,
    }
}
fn item_error(error: inventory::Error) -> Error {
    match error {
        inventory::Error::Null => Error::NullPointer,
        inventory::Error::Bounds | inventory::Error::ListBounds => Error::IndexOutOfBounds,
        inventory::Error::Type => Error::ClassCast,
        _ => Error::Service,
    }
}
fn int(arg: Option<&Arg>) -> Result<i32> {
    match arg {
        Some(Arg::Int(v)) => i32::try_from(*v).map_err(|_| Error::Reply),
        _ => Err(Error::Reply),
    }
}
fn double(arg: Option<&Arg>) -> Result<f64> {
    match arg {
        Some(Arg::Double(v)) => Ok(*v),
        _ => Err(Error::Reply),
    }
}
fn id_reply(item: Option<Handle>) -> Arg {
    item.map_or(Arg::Null, |item| item.read().id().into())
}

impl<G, S, C> Host<'_, G, S, C>
where
    G: Context,
    S: skills::Context,
    C: container::Context,
{
    fn item(&self, id: i32, amount: i32) -> Handle {
        Shared::new(Item::new(id, amount, 1000, self.definitions))
    }
    /// `Skills.getLevel(slot)`: the dynamic level, or an assisting player's.
    fn level(&mut self, skill: i32) -> Result<i32> {
        let mut bridge = SkillBridge {
            equipment: self.equipment,
            definitions: self.definitions,
            inner: &mut *self.skill_services,
        };
        self.skills
            .level(skill, false, &mut bridge)
            .map_err(skill_error)
    }
    /// `Skills.addExperience(slot, experience, playerMod)`.
    fn add_experience(&mut self, skill: i32, experience: f64, player_mod: bool) -> Result<()> {
        let mut bridge = SkillBridge {
            equipment: self.equipment,
            definitions: self.definitions,
            inner: &mut *self.skill_services,
        };
        self.skills
            .add_experience(skill, experience, player_mod, &mut bridge)
            .map_err(skill_error)
    }
    /// `Container.add(item)` on the inventory: fires its listeners.
    fn add(&mut self, id: i32, amount: i32) -> Result<bool> {
        let item = self.item(id, amount);
        self.inventory
            .add(
                Some(&item),
                true,
                -1,
                self.definitions,
                &mut *self.item_services,
            )
            .map_err(item_error)
    }
    /// The pinned `addItemOrDrop(player, id, amount)`.
    fn add_item_or_drop(&mut self, id: i32, amount: i32) -> Result<()> {
        let item = self.item(id, amount);
        let stackable = item.read().stackable().map_err(item_error)?;
        if amount == 1 || stackable {
            if !self.add(id, amount)? {
                self.required.call(
                    Service::GroundItemManagerCreate,
                    &[id.into(), amount.into()],
                )?;
            }
        } else {
            for _ in 0..amount {
                if !self.add(id, 1)? {
                    self.required
                        .call(Service::GroundItemManagerCreate, &[id.into(), 1.into()])?;
                }
            }
        }
        Ok(())
    }
    /// `Container.remove(Item(id))` with listeners. The service carries the
    /// id only, so the removed item is one unit with the default charge.
    fn remove(&mut self, id: i32, equipment: bool) -> Result<bool> {
        let item = self.item(id, 1);
        let container = if equipment {
            &mut *self.equipment
        } else {
            &mut *self.inventory
        };
        container
            .remove(
                Some(&item),
                None,
                true,
                self.definitions,
                &mut *self.item_services,
            )
            .map_err(item_error)
    }
}

impl<G, S, C> Context for Host<'_, G, S, C>
where
    G: Context,
    S: skills::Context,
    C: container::Context,
{
    fn call(&mut self, service: Service, args: &[Arg]) -> Result<Arg> {
        let first = args.first();
        let second = args.get(1);
        match service {
            Service::SkillsGetLevel | Service::ApiGetDynLevel => {
                Ok(self.level(int(first)?)?.into())
            }
            Service::ApiHasLevelDyn => Ok((self.level(int(first)?)? >= int(second)?).into()),
            Service::SkillsGetStaticLevel => usize::try_from(int(first)?)
                .ok()
                .and_then(|slot| self.skills.static_levels.get(slot))
                .map(|&level| level.into())
                .ok_or(Error::IndexOutOfBounds),
            Service::SkillsAddExperience3 => {
                let player_mod = matches!(args.get(2), Some(Arg::Bool(true)));
                self.add_experience(int(first)?, double(second)?, player_mod)?;
                Ok(Arg::Null)
            }
            Service::SkillsAddExperience2 | Service::ApiRewardXp => {
                self.add_experience(int(first)?, double(second)?, false)?;
                Ok(Arg::Null)
            }
            Service::InventoryFreeSlots | Service::ApiFreeSlots => {
                Ok(self.inventory.free_slots().into())
            }
            Service::InventoryContains | Service::ApiInInventory => {
                Ok(self.inventory.contains(int(first)?, int(second)?).into())
            }
            Service::InventoryAdd => Ok(self.add(int(first)?, int(second)?)?.into()),
            Service::InventoryHasSpaceFor | Service::ApiHasSpaceFor => {
                let item = self.item(int(first)?, int(second)?);
                self.inventory
                    .has_space_for(Some(&item))
                    .map(Into::into)
                    .map_err(item_error)
            }
            // The pinned `addItem`: bank and beast of burden stay with the owner.
            Service::ApiAddItem => match args.get(2) {
                Some(Arg::Text(name)) if name == "INVENTORY" => {
                    Ok(self.add(int(first)?, int(second)?)?.into())
                }
                Some(Arg::Text(name)) if name == "EQUIPMENT" => {
                    let item = self.item(int(first)?, int(second)?);
                    self.equipment
                        .add(
                            Some(&item),
                            true,
                            -1,
                            self.definitions,
                            &mut *self.item_services,
                        )
                        .map(Into::into)
                        .map_err(item_error)
                }
                _ => self.required.call(service, args),
            },
            Service::ApiAddItemOrDrop => {
                self.add_item_or_drop(int(first)?, int(second)?)?;
                Ok(Arg::Null)
            }
            // The pinned `removeItem`.
            Service::ApiRemoveItem => match second {
                Some(Arg::Text(name)) if name == "INVENTORY" => {
                    Ok(self.remove(int(first)?, false)?.into())
                }
                Some(Arg::Text(name)) if name == "EQUIPMENT" => {
                    Ok(self.remove(int(first)?, true)?.into())
                }
                _ => self.required.call(service, args),
            },
            Service::EquipmentGet => Ok(id_reply(self.equipment.get(int(first)?))),
            Service::EquipmentGetNew => self
                .equipment
                .get_new(int(first)?, self.definitions)
                .map(|item| item.read().id().into())
                .map_err(item_error),
            // `player.equipment.get(slot.ordinal)`.
            Service::ApiGetItemFromEquipment => {
                let Some(Arg::Text(name)) = first else {
                    return Err(Error::Reply);
                };
                let slot = EQUIPMENT_SLOTS
                    .iter()
                    .position(|slot| slot == name)
                    .ok_or(Error::Reply)?;
                Ok(id_reply(self.equipment.get(slot as i32)))
            }
            // `ItemDefinition.forId(id).getName()` and a new `Item(id).getName()`.
            Service::ApiGetItemName | Service::ItemDefinitionName | Service::ItemName => self
                .definitions
                .for_id(int(first)?)
                .read()
                .name
                .clone()
                .map(Arg::Text)
                .ok_or(Error::NullPointer),
            _ => self.required.call(service, args),
        }
    }
    fn defer(&mut self, service: Service, args: &[Arg], task: Task) -> Result<()> {
        self.required.defer(service, args, task)
    }
}
