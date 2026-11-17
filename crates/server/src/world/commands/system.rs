//! `SystemCommandSet`: update, cancelupdate, resetpassword,
//! setpasswordother, giveitem, removeitem, removeitemall, potato, shutdown
//! (in [`super::run`]) and charge.
use super::deferred::InputHandler;
use super::kotlin::{to_int, to_int_or_null};
use super::{Container, Error, Host, InputType, ItemRef, Result, arg, notify, reject};
use rs09_game_core::java_lower::lowercase_root;

/// `Items.ROTTEN_POTATO_5733`.
pub const ROTTEN_POTATO: i32 = 5733;

/// `::update [seconds]`.
pub fn update<H: Host + ?Sized>(host: &mut H, args: &[String]) -> Result<()> {
    if args.len() > 1 {
        let seconds = to_int(arg(args, 1)?)?;
        host.set_update_countdown(seconds)?;
    }
    host.flag_updating()
}

/// `::cancelupdate`.
pub fn cancel_update<H: Host + ?Sized>(host: &mut H) -> Result<()> {
    host.cancel_update()
}

/// `::resetpassword`: the rest is [`super::deferred::run_input`].
pub fn reset_password<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    host.input_dialogue(
        player,
        InputType::StringShort,
        "Enter Current Password:",
        InputHandler::CurrentPassword {
            player: player.clone(),
        },
    )
}

/// `::setpasswordother <name> <password>`.
pub fn set_password_other<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    if args.len() != 3 {
        reject::<_, ()>(
            host,
            player,
            &[
                "Usage: ::resetpasswordother user new",
                "WARNING: THIS IS PERMANENT.",
                "WARNING: PASSWORD CAN NOT CONTAIN SPACES.",
            ],
        )?;
    }
    let other = arg(args, 1)?.to_owned();
    let password = arg(args, 2)?.to_owned();
    if host.username_taken(&other)? {
        let length = password.encode_utf16().count();
        if !(5..=20).contains(&length) {
            reject::<_, ()>(
                host,
                player,
                &["NEW PASSWORD MUST BE BETWEEN 5 AND 20 CHARACTERS"],
            )?;
        }
        if password == other {
            reject::<_, ()>(host, player, &["PASSWORD CAN NOT BE SAME AS USERNAME."])?;
        }
        host.update_password(&other, &password)?;
        notify(host, player, "Password updated successfully.")
    } else {
        reject(host, player, &["USER DOES NOT EXIST!"])
    }
}

/// The item id checks of giveitem, removeitem and removeitemall.
fn valid_id<H: Host + ?Sized>(host: &mut H, id: Option<i32>) -> Result<bool> {
    Ok(match id {
        None => false,
        Some(id) if id <= 0 => false,
        Some(id) => id <= host.item_definitions_size()?,
    })
}

/// `::giveitem <name> <id> [amount]`.
pub fn give_item<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    if args.len() != 3 && args.len() != 4 {
        return reject(
            host,
            player,
            &["WRONG USAGE. USE giveitem target itemID || giveitem target itemID amt"],
        );
    }
    let victim = host.player_by_name(Some(arg(args, 1)?))?;
    let id = to_int_or_null(arg(args, 2)?);
    let amount = args.get(3).and_then(|a| to_int_or_null(a)).unwrap_or(1);
    if victim.is_none() {
        reject::<_, ()>(host, player, &["INVALID TARGET USERNAME."])?;
    }
    if !valid_id(host, id)? {
        reject::<_, ()>(host, player, &["INVALID ITEM ID ENTERED."])?;
    }
    if amount <= 0 {
        reject::<_, ()>(host, player, &["INVALID ITEM ID ENTERED."])?;
    }
    let id = id.ok_or(Error::Null)?;
    let victim = victim.ok_or(Error::Null)?;
    let full = host.is_full(&victim, Container::Inventory)?;
    let syntax = if amount > 1 { "items" } else { "item" };
    if full {
        host.add(&victim, Container::Bank, id, amount)?;
    } else {
        host.add(&victim, Container::Inventory, id, amount)?;
    }
    let name = host.item_name(id)?;
    let victim_name = host.username(&victim);
    let sent = if full {
        format!("The {syntax} were sent to their bank.")
    } else {
        String::new()
    };
    notify(
        host,
        player,
        &format!("Successfully gave {victim_name} {amount} {name}. {sent}"),
    )?;
    let name = host.item_name(id)?;
    let username = host.username(player);
    let placed = if full {
        format!("The {syntax} were placed in your bank.")
    } else {
        String::new()
    };
    notify(
        host,
        &victim,
        &format!("You received {amount} {name} from {username}. {placed}"),
    )
}

/// `::removeitem <loc> <name> <id> [amount]`.
pub fn remove_item<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    if args.len() != 4 && args.len() != 5 {
        return reject(
            host,
            player,
            &[
                "WRONG USAGE. USE removeitem itemLoc target itemID || removeitem itemLoc target itemID amt",
                "ItemLoc: inv = inventory | equip = equipment | bank |",
            ],
        );
    }
    let location = lowercase_root(arg(args, 1)?);
    let victim = host.player_by_name(Some(arg(args, 2)?))?;
    let id = to_int_or_null(arg(args, 3)?);
    let mut amount = args.get(4).and_then(|a| to_int_or_null(a)).unwrap_or(1);
    if victim.is_none() {
        reject::<_, ()>(host, player, &["INVALID TARGET USERNAME."])?;
    }
    if !valid_id(host, id)? {
        reject::<_, ()>(host, player, &["INVALID ITEM ID ENTERED."])?;
    }
    if amount <= 0 {
        reject::<_, ()>(host, player, &["INVALID ITEM AMOUNT ENTERED."])?;
    }
    let id = id.ok_or(Error::Null)?;
    let container = match location.as_str() {
        "i" | "inv" | "inventory" => Some(Container::Inventory),
        "b" | "bk" | "bank" => Some(Container::Bank),
        "e" | "equip" | "equipment" => Some(Container::Equipment),
        _ => None,
    };
    let mut total = 0;
    match container {
        Some(container) => {
            let victim = victim.as_ref().ok_or(Error::Null)?;
            total = host
                .item_amount(victim, container, id, amount)?
                .ok_or(Error::Null)?;
            host.remove(victim, container, id, amount)?;
        }
        None => reject::<_, ()>(
            host,
            player,
            &[
                "INVALID ITEM LOCATION ENTERED. USE: ",
                "i, inv, inventory | b, bk, bank | e, equip, equipment",
            ],
        )?,
    }
    if amount > total {
        amount = total;
    }
    let victim = victim.ok_or(Error::Null)?;
    let name = host.item_name(id)?;
    let victim_name = host.username(&victim);
    notify(
        host,
        player,
        &format!("Successfully removed {amount} {name} from {victim_name}."),
    )?;
    let username = host.username(player);
    let name = host.item_name(id)?;
    notify(
        host,
        &victim,
        &format!("{username} removed {amount} {name} from your inventory."),
    )
}

/// `::removeitemall <name> <id>`.
pub fn remove_item_all<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    if args.len() != 3 {
        return reject(
            host,
            player,
            &["WRONG USAGE. USE removeitemall target itemID"],
        );
    }
    let victim = host.player_by_name(Some(arg(args, 1)?))?;
    let id = to_int_or_null(arg(args, 2)?);
    if victim.is_none() {
        reject::<_, ()>(host, player, &["INVALID TARGET USERNAME."])?;
    }
    if !valid_id(host, id)? {
        reject::<_, ()>(host, player, &["INVALID ITEM ID ENTERED."])?;
    }
    let id = id.ok_or(Error::Null)?;
    let note = host.note_change(id)?;
    let victim = victim.ok_or(Error::Null)?;
    use Container::{Bank, Equipment, Inventory};
    let mut untotal = 0i32;
    for c in [Inventory, Bank, Equipment] {
        untotal = untotal.wrapping_add(host.amount(&victim, c, id)?);
    }
    let mut nototal = 0i32;
    for c in [Inventory, Bank, Equipment] {
        nototal = nototal.wrapping_add(host.amount_item(&victim, c, note)?);
    }
    let eqtotal = host
        .amount(&victim, Equipment, id)?
        .wrapping_add(host.amount_item(&victim, Equipment, note)?);
    let total = untotal.wrapping_add(nototal).wrapping_add(eqtotal);
    if total == 0 {
        reject::<_, ()>(host, player, &["USER HAS NONE OF THOSE ITEMS."])?;
    }
    for c in [Inventory, Bank, Equipment] {
        let all = host.amount(&victim, c, id)?;
        host.remove(&victim, c, id, all)?;
    }
    for c in [Inventory, Bank, Equipment] {
        let noted = host.note_change(id)?;
        let all = host.amount_item(&victim, c, note)?;
        host.remove(&victim, c, noted, all)?;
    }
    let name = host.item_name(id)?;
    let victim_name = host.username(&victim);
    notify(
        host,
        player,
        &format!("Successfully removed {total} {name} from {victim_name}."),
    )?;
    let username = host.username(player);
    let name = host.item_name(id)?;
    notify(
        host,
        &victim,
        &format!("{username} removed {total} {name} from your account."),
    )
}

/// `::potato`.
pub fn potato<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    host.add(player, Container::Inventory, ROTTEN_POTATO, 1)
        .map(|_| ())
}

/// `EquipmentSlot.slotByName(input)?.ordinal`.
pub fn slot_by_name(input: &str) -> Option<i32> {
    Some(match lowercase_root(input).as_str() {
        "head" | "helm" | "helmet" => 0,
        "cape" | "back" => 1,
        "neck" | "amulet" => 2,
        "weapon" | "main" => 3,
        "chest" | "body" | "torso" => 4,
        "shield" | "off" => 5,
        "legs" | "leg" => 7,
        "hands" | "hand" | "brace" | "bracelet" => 9,
        "feet" => 10,
        "ring" => 12,
        "ammo" | "ammunition" => 13,
        _ => return None,
    })
}

/// `getItemAndContainer(player, id)`.
fn item_and_container<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    id: i32,
) -> Result<Option<(ItemRef, Container)>> {
    if id < 0 || id >= host.item_definitions_size()? {
        return Ok(None);
    }
    for container in [
        Container::Inventory,
        Container::Equipment,
        Container::BankPrimary,
        Container::BankSecondary,
    ] {
        if let Some(item) = host
            .slots(player, container)?
            .into_iter()
            .flatten()
            .find(|item| item.id == id)
        {
            return Ok(Some((item, container)));
        }
    }
    Ok(None)
}

/// The charge command's usage lines.
const CHARGE_USAGE: [&str; 5] = [
    "Usage: ::charge <lt>equipment slot name | item id<gt> [sd] [<lt>charge<gt>]",
    "Get internal: ::charge 4718",
    "Get distinct: ::charge ring d",
    "Set internal: ::charge head s 500",
    "Set distinct: ::charge 1704 sd 4",
];

/// `::charge <slot | id> [sd] [charge]`.
pub fn charge<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 2 {
        reject::<_, ()>(host, player, &CHARGE_USAGE)?;
    }
    let first = arg(args, 1)?.to_owned();
    let (item, container) = match to_int_or_null(&first) {
        Some(id) => match item_and_container(host, player, id)? {
            Some(found) => found,
            None => return reject(host, player, &[&format!("Item not found: {id}.")]),
        },
        None => match slot_by_name(&first) {
            Some(slot) => match host.get(player, Container::Equipment, slot)? {
                Some(item) => (item, Container::Equipment),
                None => {
                    return reject(
                        host,
                        player,
                        &[&format!("No item equipped at slot: {first}.")],
                    );
                }
            },
            None => return reject(host, player, &[&format!("No slot: {first}.")]),
        },
    };
    let flags = args.get(2).cloned().unwrap_or_default();
    match flags.as_str() {
        "" => {
            let name = host.item_name(item.id)?;
            notify(host, player, &format!("{name}: {} charge.", item.charge))
        }
        "d" => {
            if host.charged_item(item.id)? {
                let name = host.item_name(item.id)?;
                let charge = host.charged_charge(item.id)?;
                notify(
                    host,
                    player,
                    &format!(
                        "{name}: ({}) charge.",
                        charge.map_or("null".into(), |c| c.to_string())
                    ),
                )
            } else {
                let name = host.item_name(item.id)?;
                reject(
                    host,
                    player,
                    &[&format!("{name} is not a distinct(#) item.")],
                )
            }
        }
        "s" | "sd" | "ds" => {
            if args.len() < 4 {
                reject::<_, ()>(host, player, &["Must enter a charge value."])?;
            }
            let Some(charge) = to_int_or_null(arg(args, 3)?) else {
                return reject(host, player, &["Charge must be an integer."]);
            };
            if flags == "s" {
                if charge < 1 {
                    reject::<_, ()>(host, player, &["Charge must be a positive integer."])?;
                }
                host.set_charge(player, container, &item, charge)?;
                let name = host.item_name(item.id)?;
                notify(
                    host,
                    player,
                    &format!("Set internal charge of {name} to {charge}."),
                )
            } else if host.charged_item(item.id)? {
                let id = host.charged_for_charge(item.id, charge)?;
                host.replace(player, container, id, item.slot)?;
                let name = host.item_name(item.id)?;
                let distinct = host.charged_charge(id)?;
                notify(
                    host,
                    player,
                    &format!(
                        "Set distinct charge of {name} to ({}).",
                        distinct.map_or("null".into(), |c| c.to_string())
                    ),
                )
            } else {
                let name = host.item_name(item.id)?;
                reject(
                    host,
                    player,
                    &[&format!("{name} is not a distinct(#) item.")],
                )
            }
        }
        _ => reject(
            host,
            player,
            &[
                "Please enter valid flags: no flag/empty, s, d, sd.",
                "Use ::charge for examples.",
            ],
        ),
    }
}
