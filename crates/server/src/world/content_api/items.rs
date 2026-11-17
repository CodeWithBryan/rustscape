//! ContentAPI's inventory, bank, equipment and beast-of-burden functions,
//! `note`/`unnote`, `equipSlot`, `dumpContainer`, `dumpBeastOfBurden` and
//! `decantContainer`.
use super::{
    Arg, Container, EquipmentSlot, Error, God, Holder, Host, Kind, Member as M, Result, Value,
    api_files, b, i, n, nd, new_item, ns, unit,
};
use crate::world::content_api::ui::send_message;

/// `ItemDefinition.forId(id)` with Kotlin's non-null use.
fn definition<H: Host + ?Sized>(h: &H, id: i32) -> Result<super::ItemDef<H::Node>> {
    h.item_definition(id).ok_or(Error::Null)
}

/// `getConfiguration(key, -1)` read as an Int (a stored value of another
/// class fails the cast).
pub fn config_int<N>(stored: &Option<Value<N>>, fallback: i32) -> Result<i32> {
    match stored {
        None | Some(Value::Null) => Ok(fallback),
        Some(Value::Int(v)) => Ok(*v),
        Some(_) => Err(Error::Cast),
    }
}

/// `getConfiguration(key, false)` read as a Boolean.
pub fn config_bool<N>(stored: &Option<Value<N>>, fallback: bool) -> Result<bool> {
    match stored {
        None | Some(Value::Null) => Ok(fallback),
        Some(Value::Bool(v)) => Ok(*v),
        Some(_) => Err(Error::Cast),
    }
}

/// `getTool(player, pickaxe)`: the `SkillingTool` constant, if any.
pub fn get_tool<H: Host + ?Sized>(h: &mut H, p: &H::Node, pickaxe: bool) -> Result<Option<String>> {
    let m = if pickaxe { M::PickaxeOf } else { M::HatchetOf };
    h.call(m, vec![nd(p)])?.constant()
}

/// `amountInInventory(player, id)`.
pub fn amount_in_inventory<H: Host + ?Sized>(h: &mut H, p: &H::Node, id: i32) -> Result<i32> {
    let inv = h.container(p, Holder::Inventory);
    i(h, M::ContainerGetAmount, vec![nd(&inv), Arg::Int(id)])
}

/// The private `getAmountInBank(player, id, secondary)`: the first stack of
/// the id answers.
fn amount_in_one_bank<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    secondary: bool,
) -> Result<i32> {
    let bank = h.container(
        p,
        if secondary {
            Holder::BankSecondary
        } else {
            Holder::BankPrimary
        },
    );
    for item in ns(h, M::ContainerToArray, vec![nd(&bank)])?
        .into_iter()
        .flatten()
    {
        if h.id(&item) == id {
            return Ok(h.item_amount(&item));
        }
    }
    Ok(0)
}

/// `amountInBank(player, id, includeSecondary = true)`.
pub fn amount_in_bank<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    include_secondary: bool,
) -> Result<i32> {
    let primary = amount_in_one_bank(h, p, id, false)?;
    let secondary = if include_secondary {
        amount_in_one_bank(h, p, id, true)?
    } else {
        0
    };
    Ok(primary.wrapping_add(secondary))
}

/// `amountInEquipment(player, id)`: the slot from the definition's
/// `equipment_slot`, then that slot's item if it is this id.
pub fn amount_in_equipment<H: Host + ?Sized>(h: &mut H, p: &H::Node, id: i32) -> Result<i32> {
    let slot = config_int(&definition(h, id)?.equip_slot, -1)?;
    let equipment = h.container(p, Holder::Equipment);
    let Some(equipped) = n(h, M::ContainerGet, vec![nd(&equipment), Arg::Int(slot)])? else {
        return Ok(0);
    };
    Ok(if h.id(&equipped) == id {
        h.item_amount(&equipped)
    } else {
        0
    })
}

/// `inInventory(player, id, amount = 1)`.
pub fn in_inventory<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    amount: i32,
) -> Result<bool> {
    let inv = h.container(p, Holder::Inventory);
    b(
        h,
        M::ContainerContains,
        vec![nd(&inv), Arg::Int(id), Arg::Int(amount)],
    )
}

/// `inBank(player, id, amount = 1)`.
pub fn in_bank<H: Host + ?Sized>(h: &mut H, p: &H::Node, id: i32, amount: i32) -> Result<bool> {
    Ok(amount_in_bank(h, p, id, true)? >= amount)
}

/// `inEquipment(player, id, amount = 1)`.
pub fn in_equipment<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    amount: i32,
) -> Result<bool> {
    Ok(amount_in_equipment(h, p, id)? >= amount)
}

/// `anyInInventory(player, ids...)`.
pub fn any_in_inventory<H: Host + ?Sized>(h: &mut H, p: &H::Node, ids: &[i32]) -> Result<bool> {
    for id in ids {
        if in_inventory(h, p, *id, 1)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `inEquipmentOrInventory(player, id, amount = 1)`.
pub fn in_equipment_or_inventory<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    amount: i32,
) -> Result<bool> {
    Ok(in_equipment(h, p, id, amount)? || in_inventory(h, p, id, amount)?)
}

/// `allInEquipment(player, ids...)`.
pub fn all_in_equipment<H: Host + ?Sized>(h: &mut H, p: &H::Node, ids: &[i32]) -> Result<bool> {
    for id in ids {
        if !in_equipment(h, p, *id, 1)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `anyInEquipment(player, ids...)`.
pub fn any_in_equipment<H: Host + ?Sized>(h: &mut H, p: &H::Node, ids: &[i32]) -> Result<bool> {
    for id in ids {
        if in_equipment(h, p, *id, 1)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `getItemFromEquipment(player, slot)`: `equipment.get(slot.ordinal)`.
pub fn get_item_from_equipment<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    slot: EquipmentSlot,
) -> Result<Option<H::Node>> {
    let equipment = h.container(p, Holder::Equipment);
    n(
        h,
        M::ContainerGet,
        vec![nd(&equipment), Arg::Int(slot as i32)],
    )
}

/// `ContainerisedItem(container, itemId)`.
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerisedItem<N> {
    pub container: Option<N>,
    pub item_id: i32,
}
impl<N: Clone + PartialEq + std::fmt::Debug> ContainerisedItem<N> {
    /// `remove()`: `container?.remove(itemId.asItem()) ?: false`.
    pub fn remove<H: Host<Node = N> + ?Sized>(&self, h: &mut H) -> Result<bool> {
        let Some(c) = &self.container else {
            return Ok(false);
        };
        let item = api_files::as_item(h, self.item_id)?;
        b(h, M::ContainerRemove, vec![nd(c), nd(&item)])
    }
    /// `exists()`.
    pub fn exists(&self) -> bool {
        self.container.is_some() && self.item_id > -1
    }
}

/// `StorableFamily.values()`.
pub const STORABLE_FAMILIES: [&str; 10] = [
    "FANCY_DRESS",
    "ARMOUR_CASE",
    "MAGIC_WARDROBE",
    "TREASURE_CHEST_LOW",
    "TREASURE_CHEST_MED",
    "TREASURE_CHEST_HIGH",
    "BOOKCASE",
    "CAPE_RACK",
    "CAPE_RACK_SKILL",
    "TOY_BOX",
];

/// `hasAnItem(player, ids, checkInactiveBank = false, checkPOH = false)`
/// (the single-id overload passes `arrayOf(id)`).
pub fn has_an_item<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    ids: &[i32],
    check_inactive_bank: bool,
    check_poh: bool,
) -> Result<ContainerisedItem<H::Node>> {
    let (active, inactive) = if h.use_secondary_bank(p) {
        (Holder::BankSecondary, Holder::BankPrimary)
    } else {
        (Holder::BankPrimary, Holder::BankSecondary)
    };
    let mut space = vec![Holder::Inventory, Holder::Equipment, active];
    if check_inactive_bank {
        space.push(inactive);
    }
    for which in space {
        let container = h.container(p, which);
        for id in ids {
            if b(
                h,
                M::ContainerContainItems,
                vec![nd(&container), Arg::ints(&[*id])],
            )? {
                return Ok(ContainerisedItem {
                    container: Some(container),
                    item_id: *id,
                });
            }
        }
    }
    if check_poh {
        for id in ids {
            for family in STORABLE_FAMILIES {
                if b(
                    h,
                    M::StorageContains,
                    vec![Arg::text(family), Arg::Int(*id)],
                )? {
                    return Ok(ContainerisedItem {
                        container: Some(h.container(p, Holder::PohProxy)),
                        item_id: *id,
                    });
                }
            }
        }
    }
    Ok(ContainerisedItem {
        container: None,
        item_id: -1,
    })
}

/// `hasGodItem(player, god)`.
pub fn has_god_item<H: Host + ?Sized>(h: &mut H, p: &H::Node, god: God) -> Result<bool> {
    for id in god.valid_items() {
        if in_equipment(h, p, *id, 1)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `shouldRemoveNothings(player)`: the ring of wealth (2572) is worn.
pub fn should_remove_nothings<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    let ring = get_item_from_equipment(h, p, EquipmentSlot::Ring)?;
    Ok(ring.is_some_and(|r| h.id(&r) == 2572))
}

/// `player.familiarManager.familiar` cast to a beast of burden.
fn burden<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<H::Node> {
    let familiar = n(h, M::FamiliarGet, vec![nd(p)])?.ok_or(Error::Null)?;
    if h.kind(&familiar) != Kind::BurdenBeast {
        return Err(Error::Cast);
    }
    Ok(familiar)
}

/// `hasFamiliar() && familiar.isBurdenBeast` (the familiar read once).
fn has_burden<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    if !b(h, M::FamiliarHas, vec![nd(p)])? {
        return Ok(false);
    }
    let familiar = n(h, M::FamiliarGet, vec![nd(p)])?.ok_or(Error::Null)?;
    Ok(h.kind(&familiar) == Kind::BurdenBeast)
}

/// The item argument of `removeItem<T>`: an Item, an Int (`Item(id)`), or
/// anything else (`IllegalStateException`).
fn item_arg<H: Host + ?Sized>(h: &mut H, item: &Value<H::Node>, message: &str) -> Result<H::Node> {
    match item {
        Value::Node(n) if h.kind(n).is_item() => Ok(n.clone()),
        Value::Int(id) => new_item(h, *id, 1, 1000),
        _ => Err(Error::State(message.into())),
    }
}

/// `removeItem(player, item, container = INVENTORY)`.
pub fn remove_item<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    item: &Value<H::Node>,
    container: Container,
) -> Result<bool> {
    if *item == Value::Null {
        return Ok(false);
    }
    let it = item_arg(h, item, "Invalid value passed for item")?;
    let remove = |h: &mut H, which: Holder| -> Result<bool> {
        let c = h.container(p, which);
        b(h, M::ContainerRemove, vec![nd(&c), nd(&it)])
    };
    match container {
        Container::Inventory => remove(h, Holder::Inventory),
        Container::Bank => Ok(remove(h, Holder::Bank)? || remove(h, Holder::BankSecondary)?),
        Container::Equipment => remove(h, Holder::Equipment),
        Container::BoB => {
            if has_burden(h, p)? {
                let beast = burden(h, p)?;
                let c = h.burden_container(&beast);
                b(h, M::ContainerRemove, vec![nd(&c), nd(&it)])
            } else {
                Ok(false)
            }
        }
    }
}

/// `removeItemsIfPlayerHasEnough(player, items...)`: each item needs one in
/// the inventory, then each is removed.
pub fn remove_items_if_player_has_enough<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    items: &[H::Node],
) -> Result<bool> {
    for item in items {
        let id = h.id(item);
        if amount_in_inventory(h, p, id)? >= 1 {
            continue;
        }
        return Ok(false);
    }
    for item in items {
        if remove_item(h, p, &Value::Node(item.clone()), Container::Inventory)? {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

/// `addItem(player, id, amount = 1, container = INVENTORY)`.
pub fn add_item<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    amount: i32,
    container: Container,
) -> Result<bool> {
    let c = match container {
        Container::Inventory => h.container(p, Holder::Inventory),
        Container::Bank => h.container(p, Holder::Bank),
        Container::Equipment => h.container(p, Holder::Equipment),
        Container::BoB => {
            if has_burden(h, p)? {
                let beast = burden(h, p)?;
                h.burden_container(&beast)
            } else {
                return Ok(false);
            }
        }
    };
    let item = new_item(h, id, amount, 1000)?;
    b(h, M::ContainerAdd, vec![nd(&c), nd(&item)])
}

/// `Item.toString()`.
pub fn item_to_string<H: Host + ?Sized>(h: &H, item: &H::Node) -> String {
    format!(
        "Item id={}, name={}, amount={}",
        h.id(item),
        h.name(item).unwrap_or_else(|| "null".into()),
        h.item_amount(item)
    )
}

/// `replaceSlot(player, slot, item, currentItem = null, container = INVENTORY)`.
pub fn replace_slot<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    slot: i32,
    item: &H::Node,
    current: Option<&H::Node>,
    container: Container,
) -> Result<Option<H::Node>> {
    let c = match container {
        Container::Inventory => h.container(p, Holder::Inventory),
        Container::Equipment => h.container(p, Holder::Equipment),
        Container::Bank => h.container(p, Holder::Bank),
        Container::BoB => {
            let beast = burden(h, p)?;
            h.burden_container(&beast)
        }
    };
    let replace = |h: &mut H, with: Arg<H::Node>| {
        n(h, M::ContainerReplace, vec![nd(&c), with, Arg::Int(slot)])
    };
    if h.id(item) == 65535 || h.item_amount(item) <= 0 {
        return replace(h, Arg::Null);
    }
    let Some(current) = current else {
        return replace(h, nd(item));
    };
    if b(
        h,
        M::ContainerRemove,
        vec![nd(&c), nd(current), Arg::Int(slot), Arg::Bool(true)],
    )? {
        return Ok(replace(h, nd(item))?.or_else(|| Some(current.clone())));
    }
    let text = format!(
        "Potential slot-replacement-based dupe attempt, slot: {slot}, item: {}",
        item_to_string(h, item)
    );
    unit(
        h,
        M::MonitorLog,
        vec![nd(p), Arg::Enum("DUPE_ALERT"), Arg::Text(text)],
    )?;
    let other = if container == Container::Inventory {
        Container::Equipment
    } else {
        Container::Inventory
    };
    if remove_item(h, p, &Value::Node(current.clone()), other)? {
        return Ok(replace(h, nd(item))?.or_else(|| Some(current.clone())));
    }
    Ok(None)
}

/// `replaceAllItems(player, itemId, replaceId)`.
pub fn replace_all_items<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    item_id: i32,
    replace_id: i32,
) -> Result<()> {
    let item = new_item(h, item_id, 1, 1000)?;
    let mut containers = vec![
        h.container(p, Holder::Inventory),
        h.container(p, Holder::Equipment),
        h.container(p, Holder::BankPrimary),
        h.container(p, Holder::BankSecondary),
    ];
    if has_burden(h, p)? {
        let beast = burden(h, p)?;
        containers.push(h.burden_container(&beast));
    }
    let primary = h.container(p, Holder::BankPrimary);
    let secondary = h.container(p, Holder::BankSecondary);
    let stackable = definition(h, h.id(&item))?.stackable;
    for c in containers {
        let found = ns(h, M::ContainerGetAll, vec![nd(&c), nd(&item)])?;
        let found = found
            .into_iter()
            .map(|f| f.ok_or(Error::Null))
            .collect::<Result<Vec<_>>>()?;
        if !stackable && c != primary && c != secondary {
            for target in &found {
                let new = new_item(h, replace_id, h.item_amount(target), 1000)?;
                let slot = h.item_slot(target);
                n(
                    h,
                    M::ContainerReplace,
                    vec![nd(&c), nd(&new), Arg::Int(slot), Arg::Bool(true)],
                )?;
            }
        } else {
            if let Some(target) = found.first() {
                let mut count = 0i32;
                for x in &found {
                    count = count.wrapping_add(h.item_amount(x));
                }
                let new = new_item(h, replace_id, count, 1000)?;
                let slot = h.item_slot(target);
                n(
                    h,
                    M::ContainerReplace,
                    vec![nd(&c), nd(&new), Arg::Int(slot), Arg::Bool(true)],
                )?;
            }
            for extra in found.iter().skip(1) {
                let slot = h.item_slot(extra);
                b(
                    h,
                    M::ContainerRemove,
                    vec![nd(&c), nd(extra), Arg::Int(slot), Arg::Bool(true)],
                )?;
            }
        }
    }
    Ok(())
}

/// `GroundItemManager.create(item, player)`.
fn drop_for<H: Host + ?Sized>(h: &mut H, item: &H::Node, p: &H::Node) -> Result<()> {
    n(h, M::GroundCreate, vec![nd(item), nd(p)]).map(|_| ())
}

/// `addItemOrDrop(player, id, amount = 1)`: one stack, or one item at a
/// time for an unstackable amount.
pub fn add_item_or_drop<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    amount: i32,
) -> Result<()> {
    let item = new_item(h, id, amount, 1000)?;
    let inv = h.container(p, Holder::Inventory);
    if amount == 1 || definition(h, id)?.stackable {
        if !b(h, M::ContainerAdd, vec![nd(&inv), nd(&item)])? {
            drop_for(h, &item, p)?;
        }
    } else {
        let single = new_item(h, id, 1, 1000)?;
        for _ in 0..amount.max(0) {
            if !b(h, M::ContainerAdd, vec![nd(&inv), nd(&single)])? {
                drop_for(h, &single, p)?;
            }
        }
    }
    Ok(())
}

/// `addItemOrBank(player, id, amount = 1)`.
pub fn add_item_or_bank<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    amount: i32,
) -> Result<()> {
    let item = new_item(h, id, amount, 1000)?;
    let inv = h.container(p, Holder::Inventory);
    if b(h, M::ContainerAdd, vec![nd(&inv), nd(&item)])? {
        return Ok(());
    }
    let name = h.name(&item).unwrap_or_else(|| "null".into());
    let not_ultimate = |h: &mut H| -> Result<bool> {
        Ok(h.call(M::IronmanMode, vec![nd(p)])?.constant()?.as_deref() != Some("ULTIMATE"))
    };
    let primary = h.container(p, Holder::BankPrimary);
    let secondary = h.container(p, Holder::BankSecondary);
    if not_ultimate(h)? && b(h, M::ContainerAdd, vec![nd(&primary), nd(&item)])? {
        send_message(
            h,
            p,
            &api_files::colorize(&format!("%RThe {name} has been sent to your bank.")),
        )
    } else if not_ultimate(h)? && b(h, M::ContainerAdd, vec![nd(&secondary), nd(&item)])? {
        send_message(
            h,
            p,
            &api_files::colorize(&format!(
                "%RThe {name} has been sent to your secondary bank."
            )),
        )
    } else {
        drop_for(h, &item, p)?;
        send_message(
            h,
            p,
            &api_files::colorize(&format!(
                "%RAs your inventory and bank account(s) are all full, the {name} has been placed on the ground under your feet. Don't forget to grab it. (Also consider cleaning out some stuff, maybe? I mean, Jesus!)"
            )),
        )
    }
}

/// `freeSlots(player)`.
pub fn free_slots<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<i32> {
    let inv = h.container(p, Holder::Inventory);
    i(h, M::ContainerFreeSlots, vec![nd(&inv)])
}

/// `hasSpaceFor(player, item)`.
pub fn has_space_for<H: Host + ?Sized>(h: &mut H, p: &H::Node, item: &H::Node) -> Result<bool> {
    let inv = h.container(p, Holder::Inventory);
    b(h, M::ContainerHasSpaceFor, vec![nd(&inv), nd(item)])
}

/// `removeAll(player, item, container = INVENTORY)`: every unit of the id.
pub fn remove_all<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    item: &Value<H::Node>,
    container: Container,
) -> Result<bool> {
    let it = match item {
        Value::Null => return Ok(false),
        Value::Node(n) if h.kind(n).is_item() => h.id(n),
        Value::Int(id) => *id,
        _ => return Err(Error::State("Invalid value passed as item".into())),
    };
    let remove = |h: &mut H, c: &H::Node, amount: i32| -> Result<bool> {
        let item = new_item(h, it, amount, 1000)?;
        b(h, M::ContainerRemove, vec![nd(c), nd(&item)])
    };
    match container {
        Container::Equipment => {
            let c = h.container(p, Holder::Equipment);
            let amount = amount_in_equipment(h, p, it)?;
            remove(h, &c, amount)
        }
        Container::Bank => {
            let primary = amount_in_bank(h, p, it, false)?;
            let secondary = amount_in_bank(h, p, it, true)?.wrapping_sub(primary);
            let bank = h.container(p, Holder::Bank);
            if !remove(h, &bank, primary)? {
                return Ok(false);
            }
            let second = h.container(p, Holder::BankSecondary);
            remove(h, &second, secondary)
        }
        Container::Inventory => {
            let c = h.container(p, Holder::Inventory);
            let amount = amount_in_inventory(h, p, it)?;
            remove(h, &c, amount)
        }
        Container::BoB => {
            if has_burden(h, p)? {
                let beast = burden(h, p)?;
                let c = h.burden_container(&beast);
                let amount = i(h, M::ContainerGetAmount, vec![nd(&c), Arg::Int(it)])?;
                remove(h, &c, amount)
            } else {
                Ok(false)
            }
        }
    }
}

/// `note(item)`.
pub fn note<H: Host + ?Sized>(h: &mut H, item: &H::Node) -> Result<H::Node> {
    let def = definition(h, h.id(item))?;
    if !def.unnoted || def.note_id < 0 {
        return Ok(item.clone());
    }
    new_item(h, def.note_id, h.item_amount(item), h.item_charge(item))
}

/// `unnote(item)`: `Item(item.noteChange, amount, charge)` for a noted item.
pub fn unnote<H: Host + ?Sized>(h: &mut H, item: &H::Node) -> Result<H::Node> {
    let def = definition(h, h.id(item))?;
    if def.unnoted {
        return Ok(item.clone());
    }
    let change = if def.note_id > -1 {
        def.note_id
    } else {
        h.id(item)
    };
    new_item(h, change, h.item_amount(item), h.item_charge(item))
}

/// `equipSlot(item)`: `EquipmentSlot.values().getOrNull(slot)`.
pub fn equip_slot<H: Host + ?Sized>(h: &mut H, item: i32) -> Result<Option<EquipmentSlot>> {
    let slot = config_int(&definition(h, item)?.equip_slot, -1)?;
    Ok(EquipmentSlot::from_ordinal(slot))
}

/// `hasSealOfPassage(player)`.
pub fn has_seal_of_passage<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    in_equipment_or_inventory(h, p, 9083, 1)
}

/// `dumpContainer(player, container)`: the number of items banked.
pub fn dump_container<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    container: &H::Node,
) -> Result<i32> {
    let bank = h.container(p, Holder::Bank);
    let mut dumped = 0i32;
    'deposit: {
        if super::services::has_ironman_restriction(h, p, "ULTIMATE")? {
            break 'deposit;
        }
        let items: Vec<H::Node> = ns(h, M::ContainerToArray, vec![nd(container)])?
            .into_iter()
            .flatten()
            .collect();
        for item in items {
            if !b(h, M::ContainerHasSpaceFor, vec![nd(&bank), nd(&item)])? {
                send_message(h, p, "You have no more space in your bank.")?;
                break 'deposit;
            }
            let name = h.name(&item).unwrap_or_else(|| "null".into());
            if !b(h, M::ContainerCanAdd, vec![nd(&bank), nd(&item)])? {
                send_message(
                    h,
                    p,
                    &format!("A magical force prevents you from banking the {name}."),
                )?;
                continue;
            }
            if h.is_equipment_container(container) {
                let id = h.id(&item);
                if !b(
                    h,
                    M::ListenersRun,
                    vec![Arg::Int(id), nd(p), nd(&item), Arg::Bool(false)],
                )? {
                    send_message(
                        h,
                        p,
                        &format!("A magical force prevents you from removing your {name}."),
                    )?;
                    continue;
                }
                if b(h, M::IsSlayerEq, vec![Arg::Int(id)])? {
                    unit(h, M::SlayerUpdateFlags, vec![nd(p)])?;
                }
            }
            b(h, M::ContainerRemove, vec![nd(container), nd(&item)])?;
            let unnoted = unnote(h, &item)?;
            b(
                h,
                M::ContainerAdd,
                vec![nd(&bank), nd(&unnoted), Arg::Bool(false)],
            )?;
            dumped = dumped.wrapping_add(1);
        }
    }
    unit(h, M::ContainerUpdate, vec![nd(container)])?;
    unit(h, M::ContainerUpdate, vec![nd(&bank)])?;
    Ok(dumped)
}

/// `dumpBeastOfBurden(player)`.
pub fn dump_beast_of_burden<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    if !b(h, M::FamiliarHas, vec![nd(p)])? {
        return send_message(h, p, "You don't have a familiar.");
    }
    let familiar = n(h, M::FamiliarGet, vec![nd(p)])?;
    if familiar.is_none_or(|f| h.kind(&f) != Kind::BurdenBeast) {
        return send_message(h, p, "Your familiar is not a Beast of Burden.");
    }
    let beast = burden(h, p)?;
    let c = h.burden_container(&beast);
    if b(h, M::ContainerIsEmpty, vec![nd(&c)])? {
        return send_message(h, p, "Your familiar's inventory is empty.");
    }
    let count = i(h, M::ContainerItemCount, vec![nd(&c)])?;
    let dumped = dump_container(h, p, &c)?;
    if dumped == count {
        send_message(
            h,
            p,
            "Your familiar's inventory was deposited into your bank.",
        )?;
    } else if dumped > 0 {
        let left = count.wrapping_sub(dumped);
        let phrase = if left == 1 {
            "item remains"
        } else {
            "items remain"
        };
        send_message(
            h,
            p,
            &format!("{left} {phrase} in your familiar's inventory."),
        )?;
    }
    Ok(())
}

/// The first `(n)` of a name, as `Pattern.compile("(\\([0-9]\\))")` finds it.
fn dose(name: &str) -> Option<i32> {
    let bytes: Vec<char> = name.chars().collect();
    bytes
        .windows(3)
        .find(|w| w[0] == '(' && w[1].is_ascii_digit() && w[2] == ')')
        .map(|w| w[1] as i32 - '0' as i32)
}

/// The iteration order of a JDK `HashMap` whose keys (distinct objects) have
/// these hash codes, inserted in this order: entries sorted by their final
/// bucket, insertion order within a bucket. Tree bins (eight or more
/// colliding keys in a table of 64+) are not modelled.
pub fn java_hash_order(hashes: &[i32]) -> Vec<usize> {
    let mut capacity = 16usize;
    while hashes.len() as f64 > capacity as f64 * 0.75 {
        capacity *= 2;
    }
    let bucket = |h: i32| ((h ^ ((h as u32) >> 16) as i32) as u32 as usize) & (capacity - 1);
    let mut order: Vec<usize> = (0..hashes.len()).collect();
    order.sort_by_key(|i| bucket(hashes[*i]));
    order
}

/// What `decantContainer` answers: (items to remove, items to add).
pub type Decanted<N> = (Vec<N>, Vec<N>);

/// `decantContainer(container)`: (items to remove, items to add).
pub fn decant_container<H: Host + ?Sized>(
    h: &mut H,
    container: &H::Node,
) -> Result<Decanted<H::Node>> {
    let mut potions: Vec<(H::Node, Vec<i32>, i32)> = Vec::new();
    let mut remove = Vec::new();
    let mut add = Vec::new();
    for item in ns(h, M::ContainerToArray, vec![nd(container)])?
        .into_iter()
        .flatten()
    {
        let id = h.id(&item);
        let Some(consumable) = n(h, M::ConsumableById, vec![Arg::Int(id)])? else {
            continue;
        };
        let Some(ids) = h.potion_ids(&consumable) else {
            continue;
        };
        let name = h.name(&item).ok_or(Error::Null)?;
        let Some(dosage) = dose(&name) else {
            continue;
        };
        match potions.iter_mut().find(|(k, _, _)| *k == consumable) {
            Some(entry) => entry.2 = entry.2.wrapping_add(dosage),
            None => potions.push((consumable, ids, dosage)),
        }
        remove.push(item);
    }
    let hashes: Vec<i32> = potions.iter().map(|(k, _, _)| h.identity_hash(k)).collect();
    for index in java_hash_order(&hashes) {
        let (_, ids, count) = &potions[index];
        let max = ids.len() as i32;
        let full = count / max;
        let rest = count % max;
        if full > 0 {
            add.push(new_item(h, ids[0], full, 1000)?);
        }
        if rest > 0 {
            add.push(new_item(h, ids[(max - rest) as usize], 1, 1000)?);
        }
    }
    let mut added = 0i32;
    for a in &add {
        added = added.wrapping_add(h.item_amount(a));
    }
    let vials = (remove.len() as i32).wrapping_sub(added);
    if vials > 0 {
        add.push(new_item(h, 229, vials, 1000)?);
    }
    Ok((remove, add))
}
