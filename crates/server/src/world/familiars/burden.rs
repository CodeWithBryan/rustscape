//! `BurdenBeast` (a familiar with a container), its
//! `BurdenContainerListener`, and `Forager` (a beast of burden of 30 that
//! produces items now and then and refuses to store the owner's).
use super::{Error, Familiar, Host, Item, Result};
use rs09_game_core::java_lower::lowercase_root;

/// `BurdenBeast.container`, and a forager's fields.
pub struct Burden<H: Host> {
    pub container: H::Container,
    pub forager: Option<ForagerState>,
}

/// `Forager.items` and `passiveDelay`.
#[derive(Clone, Debug, PartialEq)]
pub struct ForagerState {
    pub items: Vec<Item>,
    pub passive_delay: i32,
}

/// The options of the two `generateItems` calls of `openInterface`.
pub const STORE_OPTIONS: [&str; 6] = [
    "Examine",
    "Store-X",
    "Store-All",
    "Store-10",
    "Store-5",
    "Store-1",
];
pub const WITHDRAW_OPTIONS: [&str; 6] = [
    "Examine",
    "Withdraw-X",
    "Withdraw-All",
    "Withdraw-10",
    "Withdraw-5",
    "Withdraw-1",
];

impl<H: Host> Burden<H> {
    /// `new Container(size).register(new BurdenContainerListener(owner))`.
    pub(crate) fn construct(
        host: &mut H,
        owner: Option<&H::Node>,
        size: i32,
        items: Option<Vec<Item>>,
    ) -> Result<Self> {
        let container = host.new_container(size)?;
        host.register_burden_listener(&container, owner)?;
        Ok(Self {
            container,
            forager: items.map(|items| ForagerState {
                items,
                passive_delay: 0,
            }),
        })
    }
}

/// `BurdenContainerListener.update(c, event)`: the event's items and slots.
pub fn listener_update<H: Host>(
    host: &mut H,
    player: &H::Node,
    items: &[Option<Item>],
    slots: &[i32],
) -> Result<()> {
    host.container_packet(player, items, None, Some(slots))
}

/// `BurdenContainerListener.refresh(c)`: the whole container.
pub fn listener_refresh<H: Host>(host: &mut H, player: &H::Node, c: &H::Container) -> Result<()> {
    let items = host.to_array(c)?;
    let capacity = host.capacity(c)?;
    host.container_packet(player, &items, Some(capacity), None)
}

/// The close event `openInterface` sets: `closeSingleTab()`, then true.
pub fn close_event<H: Host>(host: &mut H, player: &H::Node) -> Result<bool> {
    host.close_single_tab(player)?;
    Ok(true)
}

impl<H: Host> Familiar<H> {
    /// `BurdenBeast.dismiss()`'s own part: close the interface, drop every
    /// item on the familiar's tile for 500 ticks, clear the container.
    pub(crate) fn burden_dismiss(&self, host: &mut H) -> Result<()> {
        let owner = self.owner_or_null()?;
        if host.has_main_component(&owner, 671)? {
            host.close(&owner)?;
        }
        let container = self.container()?;
        let items = host.to_array(&container)?;
        for item in items.into_iter().flatten() {
            let location = host.location(&self.npc())?;
            host.ground_item(&item, location, 500, &owner)?;
        }
        host.clear(&container)
    }

    /// `isAllowed(owner, item)`: value, tradeability, the rune essence of a
    /// non-abyssal familiar, bankability, and an abyssal familiar's
    /// unnoted essence only. Each pouch test looks the pouch up again.
    pub fn is_allowed(&self, host: &mut H, owner: &H::Node, item: &Item) -> Result<bool> {
        if host.item_value(item)? > 50000 {
            host.packet_message(
                owner,
                "This item is too valuable to trust to this familiar.",
            )?;
            return Ok(false);
        }
        if !host.item_tradeable(item.id)? {
            host.packet_message(
                owner,
                "You can't trade this item, not even to your familiar.",
            )?;
            return Ok(false);
        }
        let pouch_id = self.pouch_id();
        let abyssal = host.pouch(pouch_id)?.ok_or(Error::Null)?.abyssal;
        if (!abyssal && (item.id == 1436 || item.id == 7936)) || !host.item_bankable(item.id)? {
            let name = lowercase_root(&host.item_name(item)?);
            host.packet_message(owner, &format!("You can't store {name} in this familiar."))?;
            return Ok(false);
        }
        if host.pouch(self.pouch_id())?.ok_or(Error::Null)?.abyssal {
            if !lowercase_root(&host.item_name(item)?).contains("essence") {
                host.packet_message(owner, "You can only give unnoted essence to this familiar.")?;
                return Ok(false);
            }
            if item.id == 1437 || item.id == 7937 {
                host.packet_message(owner, "You can't give noted essence to this familiar.")?;
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `transfer(item, amount, withdraw)`.
    pub fn transfer(
        &self,
        host: &mut H,
        item: Option<Item>,
        amount: i32,
        withdraw: bool,
    ) -> Result<()> {
        if self.is_forager() && !withdraw {
            host.packet_message(
                &self.owner_or_null()?,
                "You can't store your items in this familiar.",
            )?;
            return Ok(());
        }
        let (Some(mut item), Some(owner)) = (item, self.owner()) else {
            return Ok(());
        };
        let mut amount = amount;
        if !withdraw {
            let stored = if host.item_stackable(item.id)? {
                amount
            } else {
                1
            };
            if !self.is_allowed(host, &owner, &Item::new(item.id, stored))? {
                return Ok(());
            }
        }
        let container = self.container()?;
        let to = if withdraw {
            host.inventory(&owner)?
        } else {
            container.clone()
        };
        let from = if withdraw {
            container
        } else {
            host.inventory(&owner)?
        };
        let from_amount = host.get_amount(&from, &item)?;
        if amount > from_amount {
            amount = from_amount;
        }
        let maximum = host.maximum_add(&to, &item)?;
        if amount > maximum {
            amount = maximum;
        }
        if amount < 1 {
            let text = if withdraw {
                "Not enough space in your inventory."
            } else {
                "Your familiar can't carry any more items."
            };
            host.packet_message(&owner, text)?;
            return Ok(());
        }
        if !host.item_stackable(item.id)? && item.slot > -1 {
            host.replace(&from, None, item.slot, None)?;
            host.add(&to, &Item::new(item.id, 1), None)?;
            amount = amount.wrapping_sub(1);
        }
        if amount > 0 {
            item = Item::new(item.id, amount);
            if host.remove(&from, &item, None)? {
                host.add(&to, &item, None)?;
            }
        }
        Ok(())
    }

    /// `withdrawAll()`: every slot (the capacity read each round), as much
    /// as the inventory takes, then both containers update.
    pub fn withdraw_all(&self, host: &mut H) -> Result<()> {
        let container = self.container()?;
        let mut i = 0;
        while i < host.capacity(&container)? {
            let slot = i;
            i += 1;
            let Some(mut item) = host.get(&container, slot)? else {
                continue;
            };
            let owner = self.owner_or_null()?;
            let inventory = host.inventory(&owner)?;
            let amount = host.maximum_add(&inventory, &item)?;
            if item.amount > amount {
                item = Item::new(item.id, amount);
                host.remove(&container, &item, Some(false))?;
                host.add(&inventory, &item, Some(false))?;
            } else {
                host.replace(&container, None, slot, Some(false))?;
                host.add(&inventory, &item, Some(false))?;
            }
        }
        host.update(&container)?;
        let owner = self.owner_or_null()?;
        let inventory = host.inventory(&owner)?;
        host.update(&inventory)
    }

    /// `openInterface()`.
    pub fn open_interface(&self, host: &mut H) -> Result<()> {
        let container = self.container()?;
        let owner = self.owner_or_null()?;
        if host.item_count(&container)? == 0 && self.is_forager() {
            host.packet_message(
                &owner,
                "Your familiar is not carrying any items that you can withdraw.",
            )?;
            return Ok(());
        }
        if !host.open(&owner, 671)? {
            return Err(Error::Null);
        }
        host.set_close_event(&owner, 671)?;
        host.shift(&container)?;
        host.open_single_tab(&owner, 665)?;
        let inventory = host.inventory(&owner)?;
        let items = host.to_array(&inventory)?;
        host.generate_items(&owner, &items, &STORE_OPTIONS, 665, 0, 7, 4, 93)?;
        let items = host.to_array(&container)?;
        host.generate_items(&owner, &items, &WITHDRAW_OPTIONS, 671, 27, 5, 6, 30)?;
        host.refresh(&container)
    }

    // ------------------------------------------------------------ Forager
    /// Forager's `handleFamiliarTick()` (after the content class's body).
    pub(crate) fn forager_tick(&self, host: &mut H) -> Result<()> {
        let Some(state) = self.forager() else {
            return Ok(());
        };
        if !state.items.is_empty() && state.passive_delay < host.ticks()? {
            let bound = host.forager_random(self)?;
            if host.random(bound)? < 4 {
                let items = self.forager().map(|f| f.items).unwrap_or_default();
                let index = host.random(items.len() as i32)?;
                let item = *usize::try_from(index)
                    .ok()
                    .and_then(|i| items.get(i))
                    .ok_or(Error::Index)?;
                self.produce_item(host, &item)?;
            }
            self.set_random_passive(host)?;
        }
        Ok(())
    }

    /// `produceItem(item)`.
    pub fn produce_item(&self, host: &mut H, item: &Item) -> Result<bool> {
        let container = self.container()?;
        let owner = self.owner_or_null()?;
        if !host.has_space_for(&container, item)? {
            host.packet_message(&owner, "Your familar is too full to collect items.")?;
            return Ok(false);
        }
        let text = if item.amount == 1 {
            "Your familar has produced an item."
        } else {
            "Your familiar has produced items."
        };
        host.packet_message(&owner, text)?;
        host.add(&container, item, None)
    }

    /// `produceItem()`: one of the items at random.
    pub fn produce_random(&self, host: &mut H) -> Result<bool> {
        let items = self.forager().ok_or(Error::Cast)?.items;
        if items.is_empty() {
            return Ok(false);
        }
        let index = host.random(items.len() as i32)?;
        let item = *usize::try_from(index)
            .ok()
            .and_then(|i| items.get(i))
            .ok_or(Error::Index)?;
        self.produce_item(host, &item)
    }

    /// `handlePassiveAction()`: nothing.
    pub fn handle_passive_action(&self) {}

    /// `setRandomPassive()`: now plus 100 to 440 ticks.
    pub fn set_random_passive(&self, host: &mut H) -> Result<()> {
        let now = host.ticks()?;
        let delay = now.wrapping_add(host.random_range(100, 440)?);
        self.with(|s| {
            if let super::Kind::Burden(Burden {
                forager: Some(f), ..
            }) = &mut s.kind
            {
                f.passive_delay = delay;
            }
        });
        Ok(())
    }

    /// The forager's `passiveDelay`.
    pub fn passive_delay(&self) -> Option<i32> {
        self.forager().map(|f| f.passive_delay)
    }
}
