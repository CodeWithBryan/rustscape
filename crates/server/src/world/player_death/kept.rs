//! `DeathTask.getContainers`: the items a player keeps on death.
use super::{KeptHost, Prayer, Result, null};
use crate::{
    definitions::item::{Registry, Shared},
    inventory::{
        container::{Container, ContainerType, Factory, SortType},
        item::{self, Item, definition_alchemy},
    },
    player::ironman::Mode,
    world::player_walk::NodeHandle,
};

/// The high alchemy value `getContainers` compares: the item's own cached
/// definition, followed to its unnoted form.
fn alchemy(item: &item::Handle, definitions: &Registry) -> Result<i32> {
    let definition = item.read().definition.clone().ok_or_else(null)?;
    Ok(definition_alchemy(&definition, true, definitions)?)
}

/// `DeathTask.getContainers(player)`: index 0 holds the kept items, index 1
/// the lost ones.
///
/// Inventory then equipment go into one always-stacking container of 42
/// slots. Three items are kept, none when skulled, one more with Protect Item
/// (so one when both), none at all for an ultimate ironman (a null mode is
/// not ultimate). The selection repeats its whole pass once per kept slot:
/// every item on every pass is compared with the kept slots in order and
/// goes in front of the first one whose value is lower or equal, so a later
/// item of equal value goes first; the item pushed off the end goes back
/// into the worn container (two-argument add, which can fail when that
/// container is full and the item then disappears) and one of the item is
/// taken. Kept items are single items with their charge. The lost items are
/// the rest, added to a default container of 42 slots.
pub fn get_containers<H: KeptHost>(
    host: &mut H,
    player: &NodeHandle,
    factory: &Factory,
    definitions: &Registry,
) -> Result<[Container; 2]> {
    let mut worn = factory.create(42, ContainerType::AlwaysStack, SortType::Id)?;
    let inventory = host.inventory(player)?;
    worn.add_many(&inventory.items(), definitions, host)?;
    let equipment = host.equipment(player)?;
    worn.add_many(&equipment.items(), definitions, host)?;
    let mut count = 3;
    if host.skulled(player)? {
        count -= 3;
    }
    if host.prayer_active(player, Prayer::ProtectItems)? {
        count += 1;
    }
    let mut kept = factory.create(count, ContainerType::NeverStack, SortType::Id)?;
    if host.ironman_mode(player)? != Some(Mode::Ultimate) {
        for _ in 0..count {
            for j in 0..42 {
                let Some(item) = worn.get(j) else {
                    continue;
                };
                let mut x = 0;
                while x < count {
                    let mut displaced = kept.get(x);
                    let take = match &displaced {
                        None => true,
                        Some(current) => {
                            alchemy(current, definitions)? <= alchemy(&item, definitions)?
                        }
                    };
                    if take {
                        let (id, charge) = {
                            let item = item.read();
                            (item.id(), item.charge())
                        };
                        let single = Shared::new(Item::new(id, 1, charge, definitions));
                        kept.replace(Some(&single), x, true, definitions, host)?;
                        x += 1;
                        while x < count {
                            let next = kept.get(x);
                            kept.replace(displaced.as_ref(), x, true, definitions, host)?;
                            x += 1;
                            displaced = next;
                        }
                        if let Some(displaced) = displaced {
                            worn.add_two(Some(&displaced), false, definitions, host)?;
                        }
                        let current = worn.get(j).ok_or_else(null)?;
                        let (id, amount, charge) = {
                            let current = current.read();
                            (current.id(), current.amount(), current.charge())
                        };
                        let less =
                            Shared::new(Item::new(id, amount.wrapping_sub(1), charge, definitions));
                        worn.replace(Some(&less), j, true, definitions, host)?;
                        break;
                    }
                    x += 1;
                }
            }
        }
    }
    let mut lost = factory.create(42, ContainerType::Default, SortType::Id)?;
    lost.add_many(&worn.items(), definitions, host)?;
    Ok([kept, lost])
}
