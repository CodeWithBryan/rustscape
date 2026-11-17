//! ContentAPI's skills, slayer, ironman, bank account, Grand Exchange,
//! shop, house, cutscene and credit functions, `announceIfRare`, logging and
//! `getServerConfig`.
use super::{
    Arg, Error, Holder, Host, Kind, Member as M, Result, Value, b, i, items, n, nd, state, unit,
};
use crate::world::content_api::ui::{send_message, send_news};

/// `Skills.SKILL_NAME`.
pub const SKILL_NAME: [&str; 24] = [
    "Attack",
    "Defence",
    "Strength",
    "Hitpoints",
    "Ranged",
    "Prayer",
    "Magic",
    "Cooking",
    "Woodcutting",
    "Fletching",
    "Fishing",
    "Firemaking",
    "Crafting",
    "Smithing",
    "Mining",
    "Herblore",
    "Agility",
    "Thieving",
    "Slayer",
    "Farming",
    "Runecrafting",
    "Hunter",
    "Construction",
    "Summoning",
];

/// `hasLevelDyn(player, skill, level)`.
pub fn has_level_dyn<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    skill: i32,
    level: i32,
) -> Result<bool> {
    Ok(get_dyn_level(h, p, skill)? >= level)
}

/// `hasLevelStat(player, skill, level)`.
pub fn has_level_stat<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    skill: i32,
    level: i32,
) -> Result<bool> {
    Ok(get_stat_level(h, p, skill)? >= level)
}

/// `rewardXP(player, skill, amount)`.
pub fn reward_xp<H: Host + ?Sized>(h: &mut H, p: &H::Node, skill: i32, amount: f64) -> Result<()> {
    unit(
        h,
        M::AddExperience,
        vec![nd(p), Arg::Int(skill), Arg::Double(amount)],
    )
}

/// `setTempLevel(entity, skill, level)`.
pub fn set_temp_level<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    skill: i32,
    level: i32,
) -> Result<()> {
    unit(
        h,
        M::SetLevel,
        vec![nd(e), Arg::Int(skill), Arg::Int(level)],
    )
}

/// `getStatLevel(entity, skill)`.
pub fn get_stat_level<H: Host + ?Sized>(h: &mut H, e: &H::Node, skill: i32) -> Result<i32> {
    i(h, M::GetStaticLevel, vec![nd(e), Arg::Int(skill)])
}

/// `getDynLevel(entity, skill)`.
pub fn get_dyn_level<H: Host + ?Sized>(h: &mut H, e: &H::Node, skill: i32) -> Result<i32> {
    i(h, M::GetLevel, vec![nd(e), Arg::Int(skill)])
}

/// `adjustLevel(entity, skill, amount)`: from the static level.
pub fn adjust_level<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    skill: i32,
    amount: i32,
) -> Result<()> {
    let level = get_stat_level(h, e, skill)?.wrapping_add(amount);
    set_temp_level(h, e, skill, level)
}

/// `heal(entity, amount)`.
pub fn heal<H: Host + ?Sized>(h: &mut H, e: &H::Node, amount: i32) -> Result<()> {
    i(h, M::Heal, vec![nd(e), Arg::Int(amount)]).map(|_| ())
}

/// `getMasteredSkillNames(player)`: the skills at static 99.
pub fn get_mastered_skill_names<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if i(h, M::MasteredSkills, vec![nd(p)])? > 0 {
        for (id, name) in SKILL_NAME.iter().enumerate() {
            if has_level_stat(h, p, id as i32, 99)? {
                out.push((*name).to_string());
            }
        }
    }
    Ok(out)
}

/// `getFamiliarBoost(player, skill)`.
pub fn get_familiar_boost<H: Host + ?Sized>(h: &mut H, p: &H::Node, skill: i32) -> Result<i32> {
    i(h, M::FamiliarBoost, vec![nd(p), Arg::Int(skill)])
}

/// `modPrayerPoints(player, amount)`.
pub fn mod_prayer_points<H: Host + ?Sized>(h: &mut H, p: &H::Node, amount: f64) -> Result<()> {
    if amount > 0.0 {
        unit(h, M::IncrementPrayer, vec![nd(p), Arg::Double(amount)])
    } else if amount < 0.0 {
        unit(
            h,
            M::DecrementPrayer,
            vec![nd(p), Arg::Double(amount.abs())],
        )
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------- dragonfire

/// `hasDragonfireShieldProtection(player, wyvern = false)`.
pub fn has_dragonfire_shield_protection<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    wyvern: bool,
) -> Result<bool> {
    let Some(shield) = items::get_item_from_equipment(h, p, super::EquipmentSlot::Shield)? else {
        return Ok(false);
    };
    Ok(match h.id(&shield) {
        2890 | 9731 => wyvern,
        1540 => !wyvern,
        11283 | 11284 => true,
        _ => false,
    })
}

/// `calculateDragonfireMaxHit(entity, maxDamage, wyvern = false,
/// unprotectableDamage = 0, sendMessage = false)`.
pub fn calculate_dragonfire_max_hit<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    max_damage: i32,
    wyvern: bool,
    unprotectable: i32,
    message: bool,
) -> Result<i32> {
    let (shield, potion, prayer);
    if h.kind(e) == Kind::Player {
        shield = has_dragonfire_shield_protection(h, e, wyvern)?;
        potion = !wyvern && state::has_timer_active_of(h, e, "DragonFireImmunity")?;
        prayer = b(
            h,
            M::PrayerGet,
            vec![nd(e), Arg::Enum("PROTECT_FROM_MAGIC")],
        )?;
        if message {
            let breath = if wyvern { "icy" } else { "fiery" };
            let text = if shield && potion {
                format!("Your potion and shield fully absorb the {breath} breath.")
            } else if shield {
                format!("Your shield absorbs most of the {breath} breath.")
            } else if potion {
                "Your potion absorbs some of the fiery breath.".to_string()
            } else if prayer {
                format!("Your prayer absorbs some of the {breath} breath.")
            } else {
                format!("You are horribly burnt by the {breath} breath.")
            };
            send_message(h, e, &text)?;
        }
    } else {
        let tokens = i(
            h,
            M::EntityDragonfireProtection,
            vec![nd(e), Arg::Bool(!wyvern)],
        )?;
        shield = tokens & 0x2 != 0;
        potion = tokens & 0x4 != 0;
        prayer = tokens & 0x8 != 0;
    }
    let mut effective = max_damage as f64;
    if prayer && !shield && !potion {
        effective -= 0.6 * max_damage as f64;
    } else {
        if shield {
            effective -= 0.9 * max_damage as f64;
        }
        if potion {
            effective -= 0.1 * max_damage as f64;
        }
    }
    Ok(unprotectable.max(super::java_double_to_int(effective)))
}

// ---------------------------------------------------------------- slayer

/// `SlayerManager.getInstance(player)`.
fn slayer<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    unit(h, M::SlayerInstance, vec![nd(p)])
}

/// `getSlayerTask(player)`: the `Tasks` constant.
pub fn get_slayer_task<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<Option<String>> {
    slayer(h, p)?;
    h.call(M::SlayerActiveTask, vec![nd(p)])?.constant()
}

/// `getSlayerTaskName(player)`.
pub fn get_slayer_task_name<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<String> {
    slayer(h, p)?;
    h.call(M::SlayerTaskName, vec![nd(p)])?
        .text()?
        .ok_or(Error::Null)
}

/// `getSlayerTaskKillsRemaining(player)`.
pub fn get_slayer_task_kills_remaining<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<i32> {
    slayer(h, p)?;
    i(h, M::SlayerAmount, vec![nd(p)])
}

/// `getSlayerMaster(player)`: `findNPC(master?.npc as Int) as NPC`.
pub fn get_slayer_master<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<H::Node> {
    slayer(h, p)?;
    let master = h
        .call(M::SlayerMaster, vec![nd(p)])?
        .constant()?
        .ok_or(Error::Null)?;
    let id = h.master_npc(&master);
    super::world::find_npc(h, id)?.ok_or(Error::Null)
}

/// `getSlayerMasterLocation(player)`.
pub fn get_slayer_master_location<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
) -> Result<&'static str> {
    let master = get_slayer_master(h, p)?;
    Ok(match h.id(&master) {
        1598 => "Zanaris",
        8275 => "Shilo Village",
        8274 => "Canifis",
        8273 => "Taverley",
        1597 => "Edgeville Dungeon",
        _ => "The Backrooms",
    })
}

/// `hasSlayerTask(player)`.
pub fn has_slayer_task<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    slayer(h, p)?;
    b(h, M::SlayerHasTask, vec![nd(p)])
}

/// `getSlayerTip(player)`.
pub fn get_slayer_tip<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<Vec<String>> {
    if has_slayer_task(h, p)? {
        let task = get_slayer_task(h, p)?.ok_or(Error::Null)?;
        let tip = h.call(M::TaskTip, vec![Arg::Text(task)])?.texts()?;
        return tip.ok_or(Error::Null);
    }
    Ok(vec!["You need something new to hunt.".into()])
}

/// `getSlayerFlags(player)` (the manager's flags object).
pub fn get_slayer_flags<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    slayer(h, p)
}

/// `getSlayerTaskFlags(player)`.
pub fn get_slayer_task_flags<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<i32> {
    get_slayer_flags(h, p)?;
    i(h, M::SlayerTaskFlags, vec![nd(p)])
}

// ---------------------------------------------------------------- ironman, bank, GE, shops

/// `IronmanMode` in declaration order.
pub const IRONMAN_MODES: [&str; 4] = ["NONE", "STANDARD", "HARDCORE", "ULTIMATE"];

fn mode_ordinal(mode: &str) -> Result<usize> {
    IRONMAN_MODES
        .iter()
        .position(|m| *m == mode)
        .ok_or(Error::Argument(format!(
            "No enum constant IronmanMode.{mode}"
        )))
}

/// `hasIronmanRestriction(player, restriction)`.
pub fn has_ironman_restriction<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    restriction: &str,
) -> Result<bool> {
    if !b(h, M::IronmanIs, vec![nd(p)])? {
        return Ok(false);
    }
    let mode = h
        .call(M::IronmanMode, vec![nd(p)])?
        .constant()?
        .ok_or(Error::Null)?;
    Ok(mode_ordinal(&mode)? >= mode_ordinal(restriction)?)
}

/// `restrictForIronman(player, restriction) { action }`: the action runs
/// unless the restriction applies.
pub fn restrict_for_ironman<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    restriction: &'static str,
    action: impl FnOnce(&mut H) -> Result<()>,
) -> Result<()> {
    if !b(h, M::IronmanCheck, vec![nd(p), Arg::Enum(restriction)])? {
        action(h)?;
    }
    Ok(())
}

/// `restrictForIronman` with a content callback.
pub fn restrict_for_ironman_callback<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    restriction: &'static str,
    action: &H::Node,
) -> Result<()> {
    restrict_for_ironman(h, p, restriction, |h| {
        unit(h, M::Callback, vec![nd(action)])
    })
}

/// `openGrandExchange(player)`.
pub fn open_grand_exchange<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    restrict_for_ironman(h, p, "ULTIMATE", |h| {
        unit(h, M::StockMarketOpen, vec![nd(p)])
    })
}

/// `hasAwaitingGrandExchangeCollections(player)`: only the first record is
/// looked at (the loop returns on its first pass).
pub fn has_awaiting_grand_exchange_collections<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
) -> Result<bool> {
    unit(h, M::GeRecordsInstance, vec![nd(p)])?;
    if let Some(record) = h.offer_records(p)?.into_iter().next() {
        let offer = n(
            h,
            M::GeRecordsOffer,
            vec![nd(p), Arg::opt_node(record.as_ref())],
        )?;
        return Ok(offer.is_some_and(|o| h.offer_has_withdraw(&o)));
    }
    Ok(false)
}

/// `openGrandExchangeCollectionBox(player)`.
pub fn open_grand_exchange_collection_box<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    restrict_for_ironman(h, p, "ULTIMATE", |h| {
        unit(h, M::GeRecordsInstance, vec![nd(p)])?;
        unit(h, M::GeRecordsOpenCollection, vec![nd(p)])
    })
}

/// `openBankAccount(player)`.
pub fn open_bank_account<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    restrict_for_ironman(h, p, "ULTIMATE", |h| {
        let bank = h.container(p, Holder::Bank);
        unit(h, M::BankOpen, vec![nd(&bank)])
    })
}

/// `openDepositBox(player)`.
pub fn open_deposit_box<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    restrict_for_ironman(h, p, "ULTIMATE", |h| {
        let bank = h.container(p, Holder::Bank);
        unit(h, M::BankOpenDepositBox, vec![nd(&bank)])
    })
}

/// `openBankPinSettings(player)`.
pub fn open_bank_pin_settings<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    restrict_for_ironman(h, p, "ULTIMATE", |h| {
        unit(h, M::BankPinOpenSettings, vec![nd(p)])
    })
}

/// `SecondaryBankAccountActivationResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondaryBankActivation {
    Success,
    AlreadyActive,
    NotEnoughMoney,
    InternalFailure,
}
impl SecondaryBankActivation {
    pub fn name(self) -> &'static str {
        [
            "SUCCESS",
            "ALREADY_ACTIVE",
            "NOT_ENOUGH_MONEY",
            "INTERNAL_FAILURE",
        ][self as usize]
    }
}

/// `activateSecondaryBankAccount(player)`: 5,000,000 coins, inventory first.
pub fn activate_secondary_bank_account<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
) -> Result<SecondaryBankActivation> {
    if has_ironman_restriction(h, p, "ULTIMATE")? {
        return Ok(SecondaryBankActivation::InternalFailure);
    }
    if has_activated_secondary_bank_account(h, p)? {
        return Ok(SecondaryBankActivation::AlreadyActive);
    }
    let cost = 5_000_000;
    let inventory = items::amount_in_inventory(h, p, 995)?;
    let bank = items::amount_in_bank(h, p, 995, true)?;
    let total = inventory.wrapping_add(bank);
    if cost > total {
        return Ok(SecondaryBankActivation::NotEnoughMoney);
    }
    let remove = |h: &mut H, amount: i32, c: super::Container| -> Result<bool> {
        let item = super::new_item(h, 995, amount, 1000)?;
        items::remove_item(h, p, &Value::Node(item), c)
    };
    let done = if cost > inventory {
        remove(h, inventory, super::Container::Inventory)?
            && remove(h, cost - inventory, super::Container::Bank)?
    } else {
        remove(h, cost, super::Container::Inventory)?
    };
    if done {
        state::set_attribute(h, p, "/save:UnlockedSecondaryBank", &Value::Bool(true))?;
        Ok(SecondaryBankActivation::Success)
    } else {
        send_message(h, p, &format!("{cost};{inventory};{bank};{total}"))?;
        Ok(SecondaryBankActivation::InternalFailure)
    }
}

/// `hasActivatedSecondaryBankAccount(player)`.
pub fn has_activated_secondary_bank_account<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
) -> Result<bool> {
    match state::get_attribute(h, p, "UnlockedSecondaryBank", Value::Bool(false))? {
        Value::Bool(v) => Ok(v),
        _ => Err(Error::Cast),
    }
}

/// `toggleBankAccount(player)`.
pub fn toggle_bank_account<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    restrict_for_ironman(h, p, "ULTIMATE", |h| {
        if !has_activated_secondary_bank_account(h, p)? {
            return Ok(());
        }
        let now = h.use_secondary_bank(p);
        h.set_use_secondary_bank(p, !now);
        Ok(())
    })
}

/// `isUsingSecondaryBankAccount(player)`.
pub fn is_using_secondary_bank_account<H: Host + ?Sized>(h: &H, p: &H::Node) -> bool {
    h.use_secondary_bank(p)
}

/// `getBankAccountName(player, invert = false)`.
pub fn get_bank_account_name<H: Host + ?Sized>(h: &H, p: &H::Node, invert: bool) -> &'static str {
    match (h.use_secondary_bank(p), invert) {
        (true, false) | (false, true) => "secondary",
        _ => "primary",
    }
}

/// `openNpcShop(player, npc)`.
pub fn open_npc_shop<H: Host + ?Sized>(h: &mut H, p: &H::Node, npc: i32) -> Result<bool> {
    let Some(shop) = h.shop_by_npc(npc) else {
        return Ok(false);
    };
    unit(h, M::ShopOpen, vec![nd(&shop), nd(p)])?;
    Ok(true)
}

/// `hasHouse(player)`.
pub fn has_house<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    b(h, M::HouseHas, vec![nd(p)])
}

fn bits_arg<N>(bits: (i64, i64)) -> Arg<N> {
    Arg::List(vec![Arg::Long(bits.0), Arg::Long(bits.1)])
}

/// `storeBookInHouse(player, itemId)`: a book read for the first time goes
/// to the bookcase; the game book is always stored.
pub fn store_book_in_house<H: Host + ?Sized>(h: &mut H, p: &H::Node, item: i32) -> Result<bool> {
    let Some(bit) = h
        .call(M::StorableByDisplayId, vec![Arg::Int(item)])?
        .opt_int()?
    else {
        return Ok(false);
    };
    if bit == -1 {
        return Ok(true);
    }
    let bits = h
        .call(M::BookcaseGet, vec![nd(p)])?
        .bits()?
        .ok_or(Error::Null)?;
    let read = b(h, M::BookcaseIsBitSet, vec![bits_arg(bits), Arg::Int(bit)])?;
    let add = |h: &mut H| {
        unit(
            h,
            M::StorageAddItem,
            vec![Arg::text("BOOKCASE"), Arg::Int(item)],
        )
    };
    let container = |h: &mut H| unit(h, M::StorageContainerOf, vec![nd(p), Arg::Enum("BOOKCASE")]);
    if item == 7681 {
        let updated = h
            .call(M::BookcaseSetBit, vec![bits_arg(bits), Arg::Int(bit)])?
            .bits()?
            .ok_or(Error::Null)?;
        container(h)?;
        if !read {
            unit(h, M::BookcaseSet, vec![nd(p), bits_arg(updated)])?;
        }
        add(h)?;
        return Ok(true);
    }
    if !read {
        let updated = h
            .call(M::BookcaseSetBit, vec![bits_arg(bits), Arg::Int(bit)])?
            .bits()?
            .ok_or(Error::Null)?;
        unit(h, M::BookcaseSet, vec![nd(p), bits_arg(updated)])?;
        container(h)?;
        add(h)?;
    }
    Ok(true)
}

/// `storeBookInHouse(player, node)`: items only.
pub fn store_book_node_in_house<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    node: &H::Node,
) -> Result<bool> {
    if !h.kind(node).is_item() {
        return Ok(false);
    }
    let id = h.id(node);
    store_book_in_house(h, p, id)
}

/// `Player.getCutscene()`.
pub fn get_cutscene<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<Option<H::Node>> {
    match state::get_attribute(h, p, "cutscene", Value::Null)? {
        Value::Null => Ok(None),
        Value::Node(n) if h.kind(&n) == Kind::Other => Ok(Some(n)),
        _ => Err(Error::Cast),
    }
}

/// `Player.getCutsceneStage()`.
pub fn get_cutscene_stage<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<i32> {
    match state::get_attribute(h, p, "cutscene:stage", Value::Int(0))? {
        Value::Int(v) => Ok(v),
        _ => Err(Error::Cast),
    }
}

/// `getCredits(player)`.
pub fn get_credits<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<i32> {
    i(h, M::CreditsGet, vec![nd(p)])
}

/// `updateCredits(player, amount)`: refused below zero.
pub fn update_credits<H: Host + ?Sized>(h: &mut H, p: &H::Node, amount: i32) -> Result<bool> {
    let credits = get_credits(h, p)?.wrapping_add(amount);
    if credits < 0 {
        return Ok(false);
    }
    unit(h, M::CreditsSet, vec![nd(p), Arg::Int(credits)])?;
    Ok(true)
}

/// `announceIfRare(player, item)`.
pub fn announce_if_rare<H: Host + ?Sized>(h: &mut H, p: &H::Node, item: &H::Node) -> Result<()> {
    let def = h.item_definition(h.id(item)).ok_or(Error::Null)?;
    if items::config_bool(&def.rare, false)? {
        let text = format!(
            "{} has just received: {} x {}.",
            h.username(p),
            h.item_amount(item),
            h.name(item).unwrap_or_else(|| "null".into())
        );
        send_news(h, &text)?;
        unit(h, M::StatsRareDrop, vec![nd(p), nd(item)])?;
    }
    Ok(())
}

// ---------------------------------------------------------------- logging and configuration

/// `core.tools.Log`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Fine,
    Info,
    Warn,
    Err,
    Debug,
}
impl LogLevel {
    pub const ALL: [Self; 5] = [Self::Fine, Self::Info, Self::Warn, Self::Err, Self::Debug];
    pub fn name(self) -> &'static str {
        ["FINE", "INFO", "WARN", "ERR", "DEBUG"][self as usize]
    }
}

/// `log(origin, type, message)` (`origin` is the class's simple name).
pub fn log<H: Host + ?Sized>(
    h: &mut H,
    origin: &str,
    level: LogLevel,
    message: &str,
) -> Result<()> {
    unit(
        h,
        M::Log,
        vec![
            Arg::text(origin),
            Arg::Enum(level.name()),
            Arg::text(message),
        ],
    )
}

/// `logWithStack(origin, type, message)`: the message is an exception's
/// stack trace.
pub fn log_with_stack<H: Host + ?Sized>(
    h: &mut H,
    origin: &str,
    level: LogLevel,
    message: &str,
) -> Result<()> {
    let trace = h.exception_to_string(message);
    log(h, origin, level, &trace)
}

/// `exceptionToString(e)`.
pub fn exception_to_string<H: Host + ?Sized>(h: &mut H, message: &str) -> String {
    h.exception_to_string(message)
}

/// `getServerConfig()`: the parsed configuration or a new empty Toml.
pub fn get_server_config<H: Host + ?Sized>(h: &mut H) -> Result<H::Node> {
    if let Some(t) = h.toml_data() {
        return Ok(t);
    }
    n(h, M::NewToml, vec![])?.ok_or(Error::Null)
}
