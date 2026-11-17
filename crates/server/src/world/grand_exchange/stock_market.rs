//! StockMarket.kt (the Grand Exchange interface 105, its sell tab 107 and
//! the item search 389) and PacketProcessor's `GESetOfferItem` branch, in
//! the original order.
use super::{
    COINS, Error, Host, InputScript, MarketButtons, MarketClose, MarketOpen, Offer, OfferState,
    Result, SearchClose, SearchOpen, SideButtons,
    components::{OBJDIALOG_389, STOCKMARKET_105, STOCKSIDE_107},
    exchange, offer, price_index, records,
    sounds::{GE_PLACE_ITEM_4043, GE_TRADE_ERROR_4039},
    ui::{self, Items, Ui},
};
use crate::{
    definitions::item::Shared,
    inventory::item::{Handle as Item, Item as ItemValue},
    player::ironman,
    world::{
        drop_tables::LogLevel,
        input_prompt::{InputType, Response},
        interface_listeners::{Button, Callback, Registry},
        interfaces,
        player_walk::NodeHandle,
    },
};
use rs09_protocol::game::updates::ScriptArg;

/// `defineInterfaceListeners()`: onOpen/onClose of 105 and 389, `on(107)`
/// and `on(105)`, in that order.
pub fn define_interface_listeners(registry: &Registry) -> Result<()> {
    registry.add_open(STOCKMARKET_105, Callback::open(MarketOpen))?;
    registry.add_close(STOCKMARKET_105, Callback::open(MarketClose))?;
    registry.add_open(OBJDIALOG_389, Callback::open(SearchOpen))?;
    registry.add_close(OBJDIALOG_389, Callback::open(SearchClose))?;
    registry.add(STOCKSIDE_107, None, Callback::button(SideButtons))?;
    registry.add(STOCKMARKET_105, None, Callback::button(MarketButtons))?;
    Ok(())
}

/// `IfaceSettingsBuilder().enableOptions(0, 1).build()`.
pub const SIDE_SETTINGS: i32 = (1 << 1) | (1 << 2);

/// `onOpen(105)`.
pub fn market_open<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<bool> {
    ui::send_interface_config(h, p, 105, 193, true)?;
    ui::send_iface_settings(h, p, 6, 211, 105, -1, -1)?;
    ui::send_iface_settings(h, p, 6, 209, 105, -1, -1)?;
    h.set_varp(p, 1112, -1)?;
    Ok(true)
}
/// `onClose(105)`: the result is `closeSingleTab()`'s.
pub fn market_close<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<bool> {
    ui::send_run_script(h, p, 571, "", &[])?;
    let m = h.interface_manager(p)?;
    interfaces::close_chatbox(&mut Ui::new(h, p), &m)?;
    let m = h.interface_manager(p)?;
    Ok(interfaces::close_single_tab(&mut Ui::new(h, p), &m)?)
}
/// `onOpen(389)`.
pub fn search_open<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<bool> {
    ui::send_run_script(
        h,
        p,
        570,
        "s",
        &[ScriptArg::Text(b"Grand Exchange Item Search".to_vec())],
    )?;
    Ok(true)
}
/// `onClose(389)`.
pub fn search_close<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<bool> {
    ui::send_run_script(h, p, 5781, "", &[])?;
    Ok(true)
}

fn new_item<H: Host + ?Sized>(h: &mut H, id: i32, amount: i32) -> Result<Item> {
    let definitions = h.definitions()?;
    Ok(Shared::new(ItemValue::new(id, amount, 1000, &definitions)))
}

/// The "ge-temp" attribute with a new offer as the default.
fn temp_offer<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<Offer> {
    Ok(h.temp_offer(p)?.unwrap_or_default())
}

/// `on(107)`: the sell tab's inventory (button 18): Examine (196) and Offer
/// (155).
pub fn side_button<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, b: Button) -> Result<bool> {
    if b.button != 18 || b.slot < 0 || b.slot > 27 {
        return Ok(false);
    }
    let inventory = h.inventory(p)?;
    let Some(item) = inventory.get(b.slot) else {
        return Ok(false);
    };
    let (item_id, item_amount) = {
        let item = item.read();
        (item.id(), item.amount())
    };
    match b.opcode {
        196 => {
            let examine = h.examine(item_id)?;
            h.message(p, &examine)?;
        }
        155 => {
            let o = temp_offer(h, p)?;
            let index = h.ge_index(p, -1)?;
            if item_id == COINS {
                h.content_message(p, "You can't sell money!")?;
                return Ok(true);
            }
            let mut id = item_id;
            let unnoted = item
                .read()
                .definition
                .as_ref()
                .ok_or(Error::Null)?
                .read()
                .unnoted;
            if !unnoted {
                id = item.read().note_change()?;
            }
            if !price_index::can_trade(h, id)? {
                h.content_message(p, "This item can't be sold on the Grand Exchange.")?;
                return Ok(true);
            }
            {
                let mut f = o.fields();
                f.item_id = id;
                f.sell = true;
                f.player = Some(p.clone());
            }
            let price = exchange::recommended_price(h, id, false)?;
            o.fields().offered_value = price;
            o.fields().amount = item_amount;
            o.fields().index = index;
            update_varbits(h, p, Some(&o), index, Some(true))?;
            h.set_temp_offer(p, o)?;
            let stats = exchange::offer_stats(h, id, true)?;
            ui::send_string(h, p, &stats, 105, 142)?;
        }
        _ => {}
    }
    Ok(true)
}

/// `on(105)`: every button of the exchange interface.
pub fn market_button<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, b: Button) -> Result<bool> {
    let temp = temp_offer(h, p)?;
    let mut opened_index = h.ge_index(p, -1)?;
    let instance = records::get_instance(h, Some(p))?;
    let mut opened = records::get_offer(h, &instance, opened_index)?;
    let op = b.opcode;
    match b.button {
        209 | 211 => match &opened {
            None => {
                h.log_ge("[WARN] Player tried to withdraw item with null openedOffer!")?;
                return Ok(false);
            }
            Some(o) => withdraw(h, p, o, (b.button - 209) >> 1, op)?,
        },
        190 => {
            confirm_offer(h, p, &temp, opened_index)?;
            return Ok(true);
        }
        194 => open_search(h, p)?,
        203 => abort_offer(h, p, opened.as_ref())?,
        18 | 34 | 50 | 69 | 88 | 107 => {
            opened_index = (b.button - 18) >> 4;
            let instance = records::get_instance(h, Some(p))?;
            opened = records::get_offer(h, &instance, opened_index)?;
            if op == 205 {
                abort_offer(h, p, opened.as_ref())?;
            } else {
                update_varbits(h, p, opened.as_ref(), opened_index, None)?;
            }
            if let Some(o) = &opened {
                let stats = exchange::offer_stats(h, o.item_id(), o.sell())?;
                ui::send_string(h, p, &stats, 105, 142)?;
            }
        }
        30 | 46 | 62 | 81 | 100 | 119 => {
            opened_index = (b.button - 30) >> 4;
            let instance = records::get_instance(h, Some(p))?;
            opened = records::get_offer(h, &instance, opened_index)?;
            update_varbits(h, p, opened.as_ref(), opened_index, None)?;
            open_search(h, p)?;
        }
        31 | 47 | 63 | 82 | 101 | 120 => {
            opened_index = (b.button - 31) >> 4;
            let instance = records::get_instance(h, Some(p))?;
            opened = records::get_offer(h, &instance, opened_index)?;
            update_varbits(h, p, opened.as_ref(), opened_index, Some(true))?;
            let m = h.interface_manager(p)?;
            let side = h.ui_component(p, STOCKSIDE_107)?;
            let tab = interfaces::open_single_tab(&mut Ui::new(h, p), &m, &side)?;
            let m = h.interface_manager(p)?;
            interfaces::component_open(&mut Ui::new(h, p), &m, &tab)?;
            let text = |s: &str| ScriptArg::Text(s.as_bytes().to_vec());
            ui::send_run_script(
                h,
                p,
                149,
                "IviiiIsssss",
                &[
                    text(""),
                    text(""),
                    text(""),
                    text("Examine"),
                    text("Offer"),
                    ScriptArg::Int(-1),
                    ScriptArg::Int(0),
                    ScriptArg::Int(7),
                    ScriptArg::Int(4),
                    ScriptArg::Int(93),
                    ScriptArg::Int(7012370),
                ],
            )?;
            ui::send_iface_settings(h, p, SIDE_SETTINGS, 18, 107, 0, 27)?;
        }
        157 => {
            let amount = offer::amount(h, &temp)?.wrapping_sub(1);
            update_offer_amount(h, p, &temp, amount)?;
        }
        159 => {
            let amount = offer::amount(h, &temp)?.wrapping_add(1);
            update_offer_amount(h, p, &temp, amount)?;
        }
        162 | 164 | 166 => {
            let step = match b.button {
                162 => 1,
                164 => 10,
                _ => 100,
            };
            let amount = if temp.sell() {
                step
            } else {
                offer::amount(h, &temp)?.wrapping_add(step)
            };
            update_offer_amount(h, p, &temp, amount)?;
        }
        168 => {
            let amount = if temp.sell() {
                inventory_amount(h, p, temp.item_id())?
            } else {
                offer::amount(h, &temp)?.wrapping_add(1000)
            };
            update_offer_amount(h, p, &temp, amount)?;
        }
        170 => h.input_dialogue(
            p,
            InputType::Short,
            "Enter the amount:",
            InputScript {
                player: p.clone(),
                offer: temp.clone(),
                amount: true,
            },
        )?,
        180 => {
            let price = exchange::recommended_price(h, temp.item_id(), false)?;
            update_offer_value(h, p, &temp, price)?;
        }
        177 => {
            let price = exchange::recommended_price(h, temp.item_id(), false)?;
            update_offer_value(
                h,
                p,
                &temp,
                exchange::java_double_to_int(f64::from(price) * 0.95),
            )?;
        }
        183 => {
            let price = exchange::recommended_price(h, temp.item_id(), false)?;
            update_offer_value(
                h,
                p,
                &temp,
                exchange::java_double_to_int(f64::from(price) * 1.05),
            )?;
        }
        171 => {
            let value = temp.offered_value().wrapping_sub(1);
            update_offer_value(h, p, &temp, value)?;
        }
        173 => {
            let value = temp.offered_value().wrapping_add(1);
            update_offer_value(h, p, &temp, value)?;
        }
        185 => h.input_dialogue(
            p,
            InputType::Amount,
            "Enter the amount:",
            InputScript {
                player: p.clone(),
                offer: temp.clone(),
                amount: false,
            },
        )?,
        195 => {
            let m = h.interface_manager(p)?;
            interfaces::close(&mut Ui::new(h, p), &m)?;
        }
        127 => {
            let m = h.interface_manager(p)?;
            interfaces::close_single_tab(&mut Ui::new(h, p), &m)?;
            let m = h.interface_manager(p)?;
            interfaces::close_chatbox(&mut Ui::new(h, p), &m)?;
        }
        _ => {}
    }
    h.set_ge_index(p, opened_index)?;
    h.set_temp_offer(p, temp)?;
    Ok(true)
}

/// `player.interfaceManager.openChatbox(Components.OBJDIALOG_389)`.
fn open_search<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<()> {
    let m = h.interface_manager(p)?;
    let search = h.ui_component(p, OBJDIALOG_389)?;
    interfaces::open_chatbox(&mut Ui::new(h, p), &m, &search)?;
    Ok(())
}

/// The bodies of the two input handlers (button 170's amount, read as a
/// string with `k`/`m` expanded; 185's price, an AMOUNT prompt).
pub fn run_input<H: Host + ?Sized>(
    h: &mut H,
    script: &InputScript,
    value: &Response,
) -> Result<()> {
    let p = &script.player;
    let m = h.interface_manager(p)?;
    let chatbox = m.fields.read().chatbox.clone().ok_or(Error::Null)?;
    if chatbox.read().id == 389 {
        open_search(h, p)?;
    }
    let mut s = match value {
        Response::Int(v) => v.to_string(),
        Response::Text(t) => String::from_utf16_lossy(t),
    };
    if script.amount {
        s = s.replace('k', "000");
        s = s.replace('K', "000");
        s = s.replace('m', "000000");
        s = s.replace('M', "000000");
        let amount = rs09_game_core::java_string::parse_i32(&s).ok_or(Error::Number)?;
        update_offer_amount(h, p, &script.offer, amount)?;
    } else {
        let price = rs09_game_core::java_string::parse_i32(&s).ok_or(Error::Number)?;
        update_offer_value(h, p, &script.offer, price)?;
    }
    h.set_temp_offer(p, script.offer.clone())
}

/// `updateOfferValue(player, offer, newAmt)`.
pub fn update_offer_value<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    o: &Offer,
    value: i32,
) -> Result<()> {
    o.fields().offered_value = value;
    h.set_varp(p, 1111, value)
}
/// `updateOfferAmount(player, offer, newAmt)`.
pub fn update_offer_amount<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    o: &Offer,
    amount: i32,
) -> Result<()> {
    o.fields().amount = amount;
    h.set_varp(p, 1110, amount)
}

/// `abortOffer(player, offer)`: an active offer becomes ABORTED with what is
/// left returned to its withdraw slots.
pub fn abort_offer<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, o: Option<&Offer>) -> Result<()> {
    let Some(o) = o else {
        return h.log(
            LogLevel::Warn,
            "Opened offer was null and was attempted to be aborted!",
        );
    };
    h.content_message(
        p,
        "Abort request acknowledged. Please be aware that your offer may",
    )?;
    h.content_message(p, "have already been completed.")?;
    if !offer::is_active(o) {
        let (uid, index, sell, item) = {
            let f = o.fields();
            (f.uid, f.index, f.sell, f.item_id)
        };
        let total = offer::amount(h, o)?;
        return h.log(
            LogLevel::Warn,
            &format!(
                "Offer {uid}[{index}]: {} {item}x {total} was NO LONGER active when abort attempted",
                if sell { "s" } else { "b" }
            ),
        );
    }
    o.fields().state = OfferState::Aborted;
    if o.sell() {
        let left = offer::amount_left(h, o)?;
        offer::add_withdraw_item(h, o, o.item_id(), left)?;
    } else {
        let left = offer::amount_left(h, o)?;
        let coins = left.wrapping_mul(o.offered_value());
        offer::add_withdraw_item(h, o, COINS, coins)?;
    }
    offer::update(h, o)?;
    offer::visualize(h, o, Some(p))
}

/// `OfferConfirmResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmResult {
    Success,
    ZeroCoins,
    TooManyCoins,
    NotEnoughItemsOrCoins,
    ItemRemovalFailure,
    OfferPlacementError,
}
impl ConfirmResult {
    pub fn name(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::ZeroCoins => "ZeroCoins",
            Self::TooManyCoins => "TooManyCoins",
            Self::NotEnoughItemsOrCoins => "NotEnoughItemsOrCoins",
            Self::ItemRemovalFailure => "ItemRemovalFailure",
            Self::OfferPlacementError => "OfferPlacementError",
        }
    }
}

fn remove_item<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, id: i32, amount: i32) -> Result<bool> {
    let item = new_item(h, id, amount)?;
    let mut inventory = h.inventory(p)?;
    let definitions = h.definitions()?;
    Ok(inventory.remove(
        Some(&item),
        None,
        true,
        &definitions,
        &mut Items { host: h },
    )?)
}
fn add_item<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, id: i32, amount: i32) -> Result<bool> {
    let item = new_item(h, id, amount)?;
    let mut inventory = h.inventory(p)?;
    let definitions = h.definitions()?;
    Ok(inventory.add(Some(&item), true, -1, &definitions, &mut Items { host: h })?)
}
fn note_of<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<i32> {
    Ok(h.definitions()?.for_id(id).read().note_id)
}

/// `confirmOffer(player, offer, index)`: checks the price and quantity,
/// takes the items (unnoted first, then notes) or the coins, and dispatches.
pub fn confirm_offer<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    o: &Offer,
    index: i32,
) -> Result<ConfirmResult> {
    if o.offered_value() < 1 {
        h.audio(p, GE_TRADE_ERROR_4039)?;
        h.content_message(p, "You can't make an offer for 0 coins.")?;
        return Ok(ConfirmResult::ZeroCoins);
    }
    if offer::amount(h, o)? > i32::MAX / o.offered_value() {
        h.audio(p, GE_TRADE_ERROR_4039)?;
        let verb = if o.sell() { "sell" } else { "buy" };
        h.content_message(p, &format!("You can't {verb} this much!"))?;
        return Ok(ConfirmResult::TooManyCoins);
    }
    o.fields().index = index;
    if o.sell() {
        let max = inventory_amount(h, p, o.item_id())?;
        if offer::amount(h, o)? > max {
            h.audio(p, GE_TRADE_ERROR_4039)?;
            h.content_message(
                p,
                "You do not have enough of this item in your inventory to cover the",
            )?;
            h.content_message(p, "offer.")?;
            return Ok(ConfirmResult::NotEnoughItemsOrCoins);
        }
        let held = {
            let inventory = h.inventory(p)?;
            let definitions = h.definitions()?;
            inventory.amount_id(o.item_id(), &definitions)
        };
        let unnoted = held.min(offer::amount(h, o)?);
        let left = offer::amount(h, o)?.wrapping_sub(unnoted);
        let removed_unnoted = if unnoted > 0 {
            remove_item(h, p, o.item_id(), unnoted)?
        } else {
            true
        };
        let removed_noted = if left > 0 {
            let note = note_of(h, o.item_id())?;
            remove_item(h, p, note, left)?
        } else {
            true
        };
        if !removed_unnoted || !removed_noted {
            return Ok(ConfirmResult::ItemRemovalFailure);
        }
        if exchange::dispatch(h, p, o)? {
            h.remove_temp_offer(p)?;
        } else {
            if unnoted > 0 {
                let back = offer::amount(h, o)?.wrapping_sub(left);
                add_item(h, p, o.item_id(), back)?;
            }
            if left > 0 {
                let note = note_of(h, o.item_id())?;
                add_item(h, p, note, left)?;
            }
            h.content_message(p, "Unable to place GE offer. Please try again.")?;
            return Ok(ConfirmResult::OfferPlacementError);
        }
    } else {
        let total = offer::amount(h, o)?.wrapping_mul(o.offered_value());
        let has = h.inventory(p)?.contains(COINS, total);
        if !has {
            h.audio(p, GE_TRADE_ERROR_4039)?;
            h.content_message(p, "You do not have enough coins to cover the offer.")?;
            return Ok(ConfirmResult::NotEnoughItemsOrCoins);
        }
        if exchange::dispatch(h, p, o)? && remove_item(h, p, COINS, total)? {
            h.remove_temp_offer(p)?;
        }
    }
    h.audio(p, GE_PLACE_ITEM_4043)?;
    offer::visualize(h, o, Some(p))?;
    to_main_interface(h, p)?;
    Ok(ConfirmResult::Success)
}

/// `getInventoryAmount(player, itemId)`: the item and its note.
pub fn inventory_amount<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, id: i32) -> Result<i32> {
    let item = new_item(h, id, 1)?;
    let inventory = h.inventory(p)?;
    let mut amount = inventory.amount(Some(&item));
    let note = item
        .read()
        .definition
        .as_ref()
        .ok_or(Error::Null)?
        .read()
        .note_id;
    if note > -1 {
        let noted = new_item(h, note, 1)?;
        amount = amount.wrapping_add(inventory.amount(Some(&noted)));
    }
    Ok(amount)
}

/// `StockMarket.openFor(player)`: refused for an ironman, the bank PIN
/// first when locked, else interface 105.
pub fn open_for<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<()> {
    let manager = h.ironman(p)?;
    if manager.check_restriction(&mut Messages { host: h })? {
        return Ok(());
    }
    if !h.pin_unlocked(p)? {
        return h.pin_open(p, 4);
    }
    let m = h.interface_manager(p)?;
    let main = h.ui_component(p, STOCKMARKET_105)?;
    interfaces::open(&mut Ui::new(h, p), &m, Some(main))?;
    Ok(())
}

/// IronmanManager's message context (`player.sendMessage`).
struct Messages<'a, H: Host + ?Sized> {
    host: &'a mut H,
}
impl<H: Host + ?Sized> ironman::Context for Messages<'_, H> {
    fn message(&mut self, player: Option<&NodeHandle>, text: &str) -> ironman::Result<()> {
        let player = player.ok_or(ironman::Error::Null)?;
        self.host.message(player, text).map_err(Into::into)
    }
}

/// `updateVarbits(player, offer, index, sale)`: varps 1109 to 1116 and the
/// offer's withdraw container.
pub fn update_varbits<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    o: Option<&Offer>,
    index: i32,
    sale: Option<bool>,
) -> Result<()> {
    let is_sale = sale.unwrap_or_else(|| o.is_some_and(Offer::sell));
    let mut low = 0;
    let mut high = 0;
    let item = o.map_or(0, Offer::item_id);
    let recommended = exchange::recommended_price(h, item, false)?;
    if price_index::can_trade(h, o.map_or(0, Offer::item_id))? {
        low = exchange::java_double_to_int(f64::from(recommended) * 0.95);
        high = exchange::java_double_to_int(f64::from(recommended) * 1.05);
    }
    h.set_varp(p, 1109, o.map_or(-1, Offer::item_id))?;
    let amount = match o {
        Some(o) => offer::amount(h, o)?,
        None => 0,
    };
    h.set_varp(p, 1110, amount)?;
    h.set_varp(p, 1111, o.map_or(0, Offer::offered_value))?;
    h.set_varp(p, 1112, index)?;
    h.set_varp(p, 1113, i32::from(is_sale))?;
    h.set_varp(p, 1114, recommended)?;
    h.set_varp(p, 1115, low)?;
    h.set_varp(p, 1116, high)?;
    if let Some(o) = o {
        let (index, withdraw) = {
            let f = o.fields();
            (f.index, f.withdraw)
        };
        ui::send_withdraw_container(h, p, 523i32.wrapping_add(index), &withdraw)?;
    }
    Ok(())
}

/// The ten rings of wealth the original replaces with 2572 when withdrawn.
pub const INAUTHENTIC_RINGS: [i32; 10] = [
    14638, 14639, 14640, 14641, 14642, 14643, 14644, 14645, 14646, 14647,
];

/// `StockMarket.withdraw(player, offer, index, op)`: 155 takes the item
/// noted, 196 as it is; any other option empties the slot without giving
/// anything. A finished offer with both slots empty is REMOVED, goes to the
/// history when anything was traded, and leaves the player's records.
pub fn withdraw<H: Host + ?Sized>(
    h: &mut H,
    p: &NodeHandle,
    o: &Offer,
    index: i32,
    op: i32,
) -> Result<()> {
    let slot = usize::try_from(index)
        .ok()
        .filter(|i| *i < 2)
        .ok_or(Error::Bounds)?;
    let Some(item) = o.withdraw()[slot] else {
        return h.log(LogLevel::Warn, &format!("Offer withdraw[{index}] is null!"));
    };
    let (mut id, amount) = (item.id, item.amount);
    if INAUTHENTIC_RINGS.contains(&id) {
        id = 2572;
    }
    match op {
        155 => {
            let note = {
                let definitions = h.definitions()?;
                let n = definitions.for_id(id).read().note_id;
                if n > -1 { n } else { id }
            };
            if note == -1 {
                h.content_message(p, "This item cannot be noted")?;
                return Ok(());
            }
            let wanted = new_item(h, note, amount)?;
            if h.inventory(p)?.has_space_for(Some(&wanted))? {
                add_item(h, p, note, amount)?;
            } else {
                h.audio(p, GE_TRADE_ERROR_4039)?;
                h.content_message(p, "You do not have enough room in your inventory.")?;
                return Ok(());
            }
        }
        196 => {
            let wanted = new_item(h, id, amount)?;
            if h.inventory(p)?.has_space_for(Some(&wanted))? {
                add_item(h, p, id, amount)?;
            } else {
                h.audio(p, GE_TRADE_ERROR_4039)?;
                h.content_message(p, "You do not have enough room in your inventory.")?;
                return Ok(());
            }
        }
        _ => {}
    }
    o.fields().withdraw[slot] = None;
    let empty = o.withdraw().iter().all(Option::is_none);
    if !offer::is_active(o) && empty {
        o.fields().state = OfferState::Removed;
        if o.completed_amount() > 0 {
            let instance = records::get_instance(h, Some(p))?;
            let old = instance.history();
            let instance = records::get_instance(h, Some(p))?;
            let mut history: [Option<Offer>; 5] = [None, None, None, None, None];
            history[0] = Some(o.clone());
            history[1..5].clone_from_slice(&old[0..4]);
            instance.fields().history = history;
        }
        let instance = records::get_instance(h, Some(p))?;
        instance.set_record(o.index(), None)?;
        to_main_interface(h, p)?;
    }
    offer::update(h, o)?;
    offer::visualize(h, o, Some(p))
}

/// `toMainInterface(player)`: varp 1112 to -1 twice, the chatbox and the
/// single tab closed, "ge-index" -1.
pub fn to_main_interface<H: Host + ?Sized>(h: &mut H, p: &NodeHandle) -> Result<()> {
    ui::send_config(h, p, 1112, -1)?;
    ui::send_config(h, p, 1112, -1)?;
    let m = h.interface_manager(p)?;
    interfaces::close_chatbox(&mut Ui::new(h, p), &m)?;
    let m = h.interface_manager(p)?;
    interfaces::close_single_tab(&mut Ui::new(h, p), &m)?;
    h.set_ge_index(p, -1)
}

/// PacketProcessor's `GESetOfferItem` branch: the item chosen in the search
/// becomes a one-unit buy offer at the guide price.
pub fn set_offer_item<H: Host + ?Sized>(h: &mut H, p: &NodeHandle, item: i32) -> Result<()> {
    let o = temp_offer(h, p)?;
    let index = h.ge_index(p, -1)?;
    o.fields().item_id = item;
    o.fields().sell = false;
    if !price_index::can_trade(h, item)? {
        return h.content_message(p, "That item is blacklisted from the Grand Exchange.");
    }
    o.fields().player = Some(p.clone());
    o.fields().amount = 1;
    let price = exchange::recommended_price(h, item, false)?;
    o.fields().offered_value = price;
    o.fields().index = index;
    update_varbits(h, p, Some(&o), index, Some(false))?;
    h.set_temp_offer(p, o)?;
    let stats = exchange::offer_stats(h, item, false)?;
    ui::send_string(h, p, &stats, 105, 142)?;
    let m = h.interface_manager(p)?;
    interfaces::close_chatbox(&mut Ui::new(h, p), &m)?;
    Ok(())
}
