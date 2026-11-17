//! Pinned EquipmentContainer and its actual listener. Required player/content
//! receivers remain explicit; no successful placeholder equipment gameplay.
use super::{
    Error, Result,
    container::{self, Container, Event, Factory, Items},
    item::{Handle, Item},
};
use crate::{
    definitions::{
        item::{Handle as Definition, Registry, Shared},
        plugin,
        property::Property,
    },
    world::player_walk::NodeHandle,
};
use rs09_protocol::game::{
    Frame,
    updates::{self, WireItem},
};

pub const HAT: i32 = 0;
pub const CAPE: i32 = 1;
pub const AMULET: i32 = 2;
pub const WEAPON: i32 = 3;
pub const CHEST: i32 = 4;
pub const SHIELD: i32 = 5;
pub const LEGS: i32 = 7;
pub const HANDS: i32 = 9;
pub const FEET: i32 = 10;
pub const RING: i32 = 12;
pub const ARROWS: i32 = 13;
const BONUS_NAMES: [&str; 13] = [
    "Stab: ",
    "Slash: ",
    "Crush: ",
    "Magic: ",
    "Ranged: ",
    "Stab: ",
    "Slash: ",
    "Crush: ",
    "Magic: ",
    "Ranged: ",
    "Summoning: ",
    "Strength: ",
    "Prayer: ",
];

pub fn create(factory: &Factory, player: Option<NodeHandle>) -> Result<Container> {
    let mut container = factory.create(
        14,
        super::container::ContainerType::Default,
        super::container::SortType::Id,
    )?;
    container.bind_equipment(player.clone());
    container.register(Some(super::container_listener::Handle::new(Listener::new(
        player,
    ))));
    Ok(container)
}

/// Each lookup is separate from its method call, preserving captured receivers
/// and reentrant field replacement. Implementations must not hold field locks.
pub trait Context: container::Context {
    type Dispatch;
    type Properties;
    type Weapon;
    type CombatPulse;
    type Settings;
    type Appearance;
    type Interfaces;
    fn inventory(&mut self, player: &NodeHandle) -> Result<Container>;
    fn equipment(&mut self, player: &NodeHandle) -> Result<Container>;
    fn requirements(
        &mut self,
        definition: Definition,
        player: Option<&NodeHandle>,
        message: bool,
        equip: bool,
    ) -> Result<bool>;
    fn dispatch(&mut self, player: &NodeHandle) -> Result<Self::Dispatch>;
    fn message(&mut self, dispatch: Self::Dispatch, text: &str) -> Result<()>;
    fn text(
        &mut self,
        dispatch: Self::Dispatch,
        text: Option<String>,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn unequip_plugin(
        &mut self,
        plugin: plugin::Handle,
        player: &NodeHandle,
        item: Handle,
    ) -> Result<Property>;
    fn interaction(
        &mut self,
        id: i32,
        player: &NodeHandle,
        item: Handle,
        equip: bool,
    ) -> Result<bool>;
    fn send(&mut self, packet: Packet, container: &mut Container) -> Result<()>;
    fn properties(&mut self, player: &NodeHandle) -> Result<Self::Properties>;
    fn attack_speed(&mut self, properties: Self::Properties, speed: i32) -> Result<()>;
    fn extension(&mut self, player: &NodeHandle) -> Result<Option<Self::Weapon>>;
    fn update_weapon(&mut self, weapon: Self::Weapon) -> Result<()>;
    fn defence_animation(&mut self, properties: Self::Properties) -> Result<()>;
    fn cape_perks(
        &mut self,
        player: &NodeHandle,
        container: &mut Container,
        slot: usize,
    ) -> Result<()>;
    fn attribute_bool(&mut self, player: &NodeHandle, key: &str, default: bool) -> Result<bool>;
    fn remove_attribute(&mut self, player: &NodeHandle, key: &str) -> Result<()>;
    fn combat_pulse(&mut self, properties: Self::Properties) -> Result<Self::CombatPulse>;
    fn clear_combat_handler(&mut self, pulse: Self::CombatPulse) -> Result<()>;
    fn settings(&mut self, player: &NodeHandle) -> Result<Self::Settings>;
    fn special_toggled(&mut self, settings: Self::Settings) -> Result<bool>;
    fn set_varp(&mut self, player: &NodeHandle, id: i32, value: i32) -> Result<()>;
    fn appearance(&mut self, player: &NodeHandle) -> Result<Self::Appearance>;
    fn set_animations(&mut self, appearance: Self::Appearance) -> Result<()>;
    fn update_appearance(&mut self, player: &NodeHandle) -> Result<()>;
    fn update_weight(&mut self, settings: Self::Settings) -> Result<()>;
    fn perk(&mut self, player: &NodeHandle, perk: Perk) -> Result<bool>;
    fn set_bonuses(
        &mut self,
        properties: Self::Properties,
        bonuses: Shared<Vec<i32>>,
    ) -> Result<()>;
    fn bonuses(&mut self, properties: Self::Properties) -> Result<Shared<Vec<i32>>>;
    fn interfaces(&mut self, player: &NodeHandle) -> Result<Self::Interfaces>;
    fn has_main(&mut self, interfaces: Self::Interfaces, id: i32) -> Result<bool>;
}
#[derive(Clone, Copy, Debug)]
pub enum Perk {
    FineAttunement,
    GrandBullwark,
}
fn definition(item: &Handle) -> Result<Definition> {
    item.read().definition.clone().ok_or(Error::Null)
}
fn property(item: &Handle, key: &str) -> Result<Option<Property>> {
    Ok(definition(item)?
        .read()
        .handlers
        .get(key)
        .filter(|p| !p.is_null())
        .cloned())
}
fn integer(item: &Handle, key: &str, default: i32) -> Result<i32> {
    match property(item, key)? {
        None => Ok(default),
        Some(Property::Int(value)) => Ok(value),
        _ => Err(Error::Type),
    }
}
fn boolean(item: &Handle, key: &str) -> Result<bool> {
    match property(item, key)? {
        None => Ok(false),
        Some(Property::Bool(value)) => Ok(value),
        _ => Err(Error::Type),
    }
}
fn bonus(item: &Handle) -> Result<Shared<Vec<i32>>> {
    match property(item, "bonuses")? {
        None => Ok(Shared::new(vec![0; 15])),
        Some(Property::IntArray(value)) => Ok(value),
        _ => Err(Error::Type),
    }
}
fn player(container: &Container) -> Result<Option<NodeHandle>> {
    container
        .equipment_owner()
        .ok_or_else(|| Error::Binding("not an EquipmentContainer".into()))
}

/// The two-argument override delegates through this overload, which captures
/// the inventory receiver before looking up the supplied item's current ID.
pub fn add(
    container: &mut Container,
    item: Option<&Handle>,
    fire: bool,
    from_inventory: bool,
    definitions: &Registry,
    ctx: &mut impl Context,
) -> Result<bool> {
    let owner = player(container)?.ok_or(Error::Null)?;
    let inventory = ctx.inventory(&owner)?;
    let slot = inventory.slot(item, false);
    add_at(
        container,
        item,
        slot,
        fire,
        from_inventory,
        definitions,
        ctx,
    )
}

pub fn add_at(
    container: &mut Container,
    item: Option<&Handle>,
    inventory_slot: i32,
    fire: bool,
    from_inventory: bool,
    definitions: &Registry,
    ctx: &mut impl Context,
) -> Result<bool> {
    let new_item = item.ok_or(Error::Null)?;
    let slot = integer(new_item, "equipment_slot", -1)?;
    if slot < 0 {
        return Ok(false);
    }
    let owner = player(container)?;
    if !ctx.requirements(definition(new_item)?, owner.as_ref(), true, true)? {
        return Ok(false);
    }
    let mut remove = Vec::new();
    if let Some(current) = container.get(slot) {
        remove.push(current);
    }
    if boolean(new_item, "two_handed")? {
        if let Some(current) = container.get(SHIELD) {
            remove.push(current);
        }
    } else if slot == SHIELD {
        let equipment = ctx.equipment(owner.as_ref().ok_or(Error::Null)?)?;
        if let Some(current) = equipment.get(WEAPON) {
            if boolean(&current, "two_handed")? {
                if let Some(current) = container.get(WEAPON) {
                    remove.push(current);
                }
            }
        }
    }
    if from_inventory {
        let mut inventory = ctx.inventory(owner.as_ref().ok_or(Error::Null)?)?;
        if !inventory.remove(Some(new_item), Some(inventory_slot), true, definitions, ctx)? {
            return Ok(false);
        }
    }
    if !remove.is_empty() {
        let mut invalidated = Vec::new();
        for current in &remove {
            let current_id = current.read().id();
            if current_id == new_item.read().id() && current.read().stackable()? {
                stack(
                    container,
                    new_item,
                    from_inventory,
                    slot,
                    current,
                    definitions,
                    ctx,
                )?;
                invalidated.push(current.clone());
            }
        }
        remove.retain(|item| !invalidated.iter().any(|other| other.same(item)));
        if remove.is_empty() {
            return Ok(true);
        }
        if !unequip(
            container,
            &remove,
            new_item,
            inventory_slot,
            owner.as_ref(),
            definitions,
            ctx,
        )? {
            if from_inventory {
                let mut inventory = ctx.inventory(owner.as_ref().ok_or(Error::Null)?)?;
                inventory.add(Some(new_item), true, -1, definitions, ctx)?;
            }
            return Ok(false);
        }
    }
    container.replace(Some(new_item), slot, fire, definitions, ctx)?;
    if new_item.read().index == WEAPON {
        let dispatch = ctx.dispatch(owner.as_ref().ok_or(Error::Null)?)?;
        let name = new_item.read().name.clone();
        ctx.text(dispatch, name, 92, 0)?;
    }
    Ok(true)
}
fn stack(
    container: &mut Container,
    item: &Handle,
    from_inventory: bool,
    slot: i32,
    current: &Handle,
    definitions: &Registry,
    ctx: &mut impl Context,
) -> Result<()> {
    let mut amount = container.maximum_add(Some(item))?;
    if item.read().amount() > amount {
        amount = amount.wrapping_add(current.read().amount());
    } else {
        amount = current.read().amount().wrapping_add(item.read().amount());
    }
    let transfer = Shared::new(Item::new(current.read().id(), amount, 1000, definitions));
    if from_inventory {
        let owner = player(container)?.ok_or(Error::Null)?;
        let mut inventory = ctx.inventory(&owner)?;
        inventory.remove(Some(&transfer), None, true, definitions, ctx)?;
    }
    container.replace(Some(&transfer), slot, true, definitions, ctx)?;
    Ok(())
}
fn unequip(
    container: &mut Container,
    current: &[Handle],
    _new_item: &Handle,
    preferred: i32,
    owner: Option<&NodeHandle>,
    definitions: &Registry,
    ctx: &mut impl Context,
) -> Result<bool> {
    if current.is_empty() {
        return Ok(true);
    }
    let owner = owner.ok_or(Error::Null)?;
    let free = ctx.inventory(owner)?.free_slots();
    let mut needed = 0;
    for item in current {
        if !item.read().stackable()? {
            needed += 1;
        } else {
            let inventory = ctx.inventory(owner)?;
            if inventory.amount_id(item.read().id(), definitions) == 0 {
                needed += 1;
            }
        }
    }
    if free < needed {
        let dispatch = ctx.dispatch(owner)?;
        ctx.message(dispatch, "Not enough space in your inventory!")?;
        return Ok(false);
    }
    let mut allowed = true;
    for item in current {
        if let Some(value) = property(item, "equipment")? {
            let Property::Plugin(plugin) = value else {
                return Err(Error::Type);
            };
            match ctx.unequip_plugin(plugin, owner, item.clone())? {
                Property::Null => {}
                Property::Bool(false) => {
                    allowed = false;
                    break;
                }
                Property::Bool(true) => {}
                _ => return Err(Error::Type),
            }
        }
        let id = item.read().id();
        allowed = ctx.interaction(id, owner, item.clone(), false)?;
        if !allowed {
            break;
        }
    }
    let mut removed = true;
    for item in current {
        if !container.remove(Some(item), None, true, definitions, ctx)? {
            removed = false;
            break;
        }
    }
    let mut added = removed;
    if removed {
        for item in current {
            let mut inventory = ctx.inventory(owner)?;
            if !inventory.add(Some(item), true, preferred, definitions, ctx)? {
                added = false;
                break;
            }
        }
    }
    if allowed && removed && added {
        return Ok(true);
    }
    for item in current {
        if !container.contains_item(Some(item))? {
            container.add(Some(item), true, -1, definitions, ctx)?;
        }
    }
    Ok(false)
}

pub struct Packet {
    pub player: Option<NodeHandle>,
    pub items: Items,
    pub slots: Option<Vec<usize>>,
}
impl Packet {
    pub fn frame(&self, null_item: &Handle) -> Result<Frame> {
        let items = self.items.read();
        let wire = |item: &Handle| {
            let item = item.read();
            WireItem {
                id: item.id(),
                amount: item.amount(),
            }
        };
        let result = if let Some(slots) = &self.slots {
            let values = slots
                .iter()
                .map(|&slot| {
                    Ok((
                        slot as i32,
                        items
                            .get(slot)
                            .ok_or(Error::Bounds)?
                            .as_ref()
                            .filter(|item| !item.same(null_item))
                            .map(wire),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            updates::inventory_partial((387 << 16) | 28, 94, &values)
        } else {
            let values = items
                .iter()
                .map(|item| item.as_ref().map(wire))
                .collect::<Vec<_>>();
            updates::inventory_full((387 << 16) | 28, 94, &values)
        };
        result.map_err(|e| Error::Binding(e.to_string()))
    }
}
pub struct Listener {
    player: Option<NodeHandle>,
}
impl super::container_listener::NativeListener for Listener {
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        self.player.clone().into_iter().collect()
    }
}
impl Listener {
    pub fn new(player: Option<NodeHandle>) -> Self {
        Self { player }
    }
    pub fn owner(&self) -> Option<NodeHandle> {
        self.player.clone()
    }
    pub fn update_event(
        &self,
        ctx: &mut impl Context,
        c: &mut Container,
        event: Option<&Event>,
        definitions: &Registry,
    ) -> Result<()> {
        let event = event.ok_or(Error::Null)?;
        let slots = event.read().slots();
        let items = event.read().items.clone();
        ctx.send(
            Packet {
                player: self.player.clone(),
                items,
                slots: Some(slots.clone()),
            },
            c,
        )?;
        self.update(ctx, c, definitions)?;
        let mut defence = false;
        for slot in slots {
            if slot == WEAPON as usize {
                let owner = self.player.as_ref().ok_or(Error::Null)?;
                let properties = ctx.properties(owner)?;
                let speed = integer(&c.get_new(slot as i32, definitions)?, "attack_speed", 4)?;
                ctx.attack_speed(properties, speed)?;
                let Some(weapon) = ctx.extension(owner)? else {
                    break;
                };
                ctx.update_weapon(weapon)?;
                defence = true;
            } else if slot == SHIELD as usize {
                defence = true;
            } else if slot == CAPE as usize {
                ctx.cape_perks(self.player.as_ref().ok_or(Error::Null)?, c, slot)?;
            }
        }
        if defence {
            let properties = ctx.properties(self.player.as_ref().ok_or(Error::Null)?)?;
            ctx.defence_animation(properties)?;
        }
        Ok(())
    }
    pub fn refresh(
        &self,
        ctx: &mut impl Context,
        c: &mut Container,
        definitions: &Registry,
    ) -> Result<()> {
        let owner = self.player.as_ref().ok_or(Error::Null)?;
        let properties = ctx.properties(owner)?;
        let speed = integer(&c.get_new(3, definitions)?, "attack_speed", 4)?;
        ctx.attack_speed(properties, speed)?;
        if let Some(weapon) = ctx.extension(owner)? {
            ctx.update_weapon(weapon)?;
        }
        ctx.send(
            Packet {
                player: self.player.clone(),
                items: c.items(),
                slots: None,
            },
            c,
        )?;
        self.update(ctx, c, definitions)?;
        let properties = ctx.properties(owner)?;
        ctx.defence_animation(properties)
    }
    pub fn update(
        &self,
        ctx: &mut impl Context,
        c: &mut Container,
        definitions: &Registry,
    ) -> Result<()> {
        let shield = c.get_new(SHIELD, definitions)?;
        let shield_id = shield.read().id();
        if shield_id != 11283
            && ctx.attribute_bool(self.player.as_ref().ok_or(Error::Null)?, "dfs_spec", false)?
        {
            let owner = self.player.as_ref().ok_or(Error::Null)?;
            ctx.remove_attribute(owner, "dfs_spec")?;
            let properties = ctx.properties(owner)?;
            let pulse = ctx.combat_pulse(properties)?;
            ctx.clear_combat_handler(pulse)?;
            let settings = ctx.settings(owner)?;
            if !ctx.special_toggled(settings)? {
                ctx.set_varp(owner, 301, 0)?;
            }
        }
        let owner = self.player.as_ref().ok_or(Error::Null)?;
        let appearance = ctx.appearance(owner)?;
        ctx.set_animations(appearance)?;
        ctx.update_appearance(owner)?;
        let settings = ctx.settings(owner)?;
        ctx.update_weight(settings)?;
        update_bonuses(owner, ctx)
    }
}
pub fn update_bonuses(player: &NodeHandle, ctx: &mut impl Context) -> Result<()> {
    let mut bonuses = vec![0i32; 15];
    let equipment = ctx.equipment(player)?;
    let items = equipment.items();
    let length = items.read().len();
    for slot in 0..length {
        let item = items.read()[slot].clone();
        if let Some(item) = item {
            let array = bonus(&item)?;
            for (i, value) in array.read().iter().copied().enumerate() {
                let target = bonuses.get_mut(i).ok_or(Error::Bounds)?;
                *target = target.wrapping_add(value);
            }
        }
    }
    let weapon = ctx.equipment(player)?.get(WEAPON);
    if let Some(weapon) = weapon {
        let requirements = definition(&weapon)?
            .read()
            .handlers
            .get("requirements")
            .cloned();
        let requirement = match requirements {
            None | Some(Property::Null) => 0,
            Some(Property::Requirements(r)) => r.read().get(2).unwrap_or(0),
            _ => return Err(Error::Type),
        };
        if requirement > 0 && ctx.perk(player, Perk::FineAttunement)? {
            let array = bonus(&weapon)?;
            let value = *array.read().get(11).ok_or(Error::Bounds)?;
            bonuses[11] = (f64::from(bonuses[11]) + (f64::from(value) * 0.20).ceil()) as i32;
        }
    }
    let shield = ctx.equipment(player)?.get(SHIELD);
    if shield.is_some() && ctx.perk(player, Perk::GrandBullwark)? {
        for i in [5, 6, 7, 9, 10] {
            bonuses[i] = (f64::from(bonuses[i]) + (f64::from(bonuses[i]) * 0.20).ceil()) as i32;
        }
    }
    if let Some(shield) = shield {
        if shield.read().id() == 11283 {
            let increase = shield.read().charge() / 20;
            for i in [5, 6, 7, 9, 10] {
                bonuses[i] = bonuses[i].wrapping_add(increase);
            }
        }
    }
    let properties = ctx.properties(player)?;
    ctx.set_bonuses(properties, Shared::new(bonuses))?;
    let interfaces = ctx.interfaces(player)?;
    if ctx.has_main(interfaces, 667)? {
        update(player, ctx)?;
    }
    Ok(())
}
pub fn update(player: &NodeHandle, ctx: &mut impl Context) -> Result<()> {
    let properties = ctx.properties(player)?;
    let bonuses = ctx.bonuses(properties)?;
    let mut index = 0;
    for child in 36..50 {
        if child == 47 {
            continue;
        }
        let value = *bonuses.read().get(index).ok_or(Error::Bounds)?;
        let value = if value > -1 {
            format!("+{value}")
        } else {
            value.to_string()
        };
        let dispatch = ctx.dispatch(player)?;
        let text = format!("{}{value}", BONUS_NAMES[index]);
        index += 1;
        ctx.text(dispatch, Some(text), 667, child)?;
    }
    let settings = ctx.settings(player)?;
    ctx.update_weight(settings)?;
    let dispatch = ctx.dispatch(player)?;
    ctx.text(dispatch, Some("Attack bonus".into()), 667, 34)
}
