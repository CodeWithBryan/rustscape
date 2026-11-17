//! `MiscCommandSet`: every command body, `resolveBooleanToggle`,
//! `setBooleanAttribute`, `setupScrollInterface`, `displayCommandInScroll`,
//! `chunkText`, `setPlaqueReadStatus` and the Grand Exchange views
//! (`showGeBuy`, `showGeSell`, `showGeBots`, `showGeBotsearch`,
//! `showOffers`, `showGeInputDialogue`, `showGeBook`).
use super::{
    Actor, Book, BookLine, Handler, Host, Input, Job, Level, Lock, PageSet, Timer, Value,
    chat_icon, chunk_text, contains_ignore_case, equals_ignore_case, is_headless, java_len,
    join_from, location_string, round_up_4,
};
use crate::player::details::Rights;
use crate::world::commands::kotlin::{to_int, to_int_or_null};
use crate::world::commands::{
    Command as RegistryCommand, Container, Error, InputType, Registry, Result, arg, notify, reject,
    rights,
};
use crate::world::repository::hash_map::JavaHashMap;
use rs09_game_core::java_lower::lowercase_root;

/// The class `this::class.java` names in the set's log calls.
pub const ORIGIN: &str = "core.game.system.command.sets.MiscCommandSet";
/// `Components.QUESTJOURNAL_SCROLL_275`.
pub const SCROLL: i32 = 275;
/// `Components.CREDIT_SHOP`.
pub const CREDIT_SHOP: i32 = 837;
/// `BookInterface.FANCY_BOOK_26`.
pub const FANCY_BOOK_26: i32 = 26;
/// `BookInterface.FANCY_BOOK_26_LINE_IDS`.
pub const FANCY_BOOK_26_LINE_IDS: [i32; 33] = [
    101, 65, 66, 97, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86,
    87, 88, 89, 90, 91, 92, 93, 94, 95, 96,
];
/// `GE_NEWS_MUTE_ATTRIBUTE`.
pub const GE_NEWS_MUTE: &str = "/save:ge:news-mute";
/// `Familiar.INFINITE_SPECIAL_MOVE_ATTRIBUTE`.
pub const INFINITE_SPECIAL_MOVE: &str = "infinite-special-move";
const DEFENCE: i32 = 1;
const FISHING: i32 = 10;

/// The set's commands, in definition order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    Debug,
    CalcAccuracy,
    AnmaCs,
    SetSaveVersion,
    Loc,
    Pos,
    Coords,
    CalcMaxHit,
    CalcDefence,
    Empty,
    Announce,
    Players,
    BotInfo,
    Shop,
    Ge,
    Commands,
    CommandSearch,
    Reply,
    XFace,
    CompleteDiaries,
    Permadeath,
    Log,
    LogDone,
    RollTrawlerLoot,
    FillBank,
    EmptyBank,
    SetConfig,
    GetObjectVarp,
    DefineVarbit,
    SetVarbit,
    SetVarbits,
    GetVarbit,
    SetVarp,
    SetVarc,
    Grow,
    FinishBins,
    ResetBins,
    DiseaseCrops,
    AddCredits,
    GetNpcParent,
    InfiniteSpecial,
    InfiniteSpecialMove,
    AllowAggro,
    Rules,
    ConfirmRules,
    SetPlayerStrong,
    SetPlaqueRead,
    BrewKit,
    Brew,
}

pub fn run<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    command: Command,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    use Command::*;
    match command {
        Debug => toggle_debug(host, player),
        CalcAccuracy => calc_accuracy(host, player, args),
        AnmaCs => host.start_cutscene(player, "AnmaCutscene"),
        SetSaveVersion => set_save_version(host, player, args),
        Loc => loc(host, player),
        Pos | Coords => notify(host, player, "Do you mean ::loc?"),
        CalcMaxHit => calc_max_hit(host, player),
        CalcDefence => calc_defence(host, player, args),
        Empty => {
            host.container_clear(player, Container::Inventory)?;
            host.container_refresh(player, Container::Inventory)
        }
        Announce => host.send_news(&join_from(args, 1, " "), 12, "CC6600"),
        Players => players(host, player),
        BotInfo => bot_info(host, player, args),
        Shop => {
            if host.locked(player, Lock::Interaction)? || host.locked(player, Lock::Movement)? {
                host.api_message(player, "You can't open the shop right now.")
            } else {
                host.open_component(player, CREDIT_SHOP)
            }
        }
        Ge => ge(host, player, args),
        Commands => commands(host, registry, player, args),
        CommandSearch => command_search(host, registry, player, args),
        Reply => reply(host, player),
        XFace => {
            host.close_interfaces(player)?;
            host.close_dialogue(player).map(|_| ())
        }
        CompleteDiaries => {
            for (diary, levels) in host.diaries(player).into_iter().enumerate() {
                for (level, tasks) in levels.into_iter().enumerate() {
                    for task in 0..tasks.max(0) {
                        host.finish_task(player, diary as i32, level as i32, task)?;
                    }
                }
            }
            Ok(())
        }
        Permadeath => permadeath(host, player, args),
        Log => log(host, player),
        LogDone => log_done(host, player),
        RollTrawlerLoot => {
            let rolls = if args.len() < 2 {
                100
            } else {
                to_int(arg(args, 1)?)?
            };
            let level = host.level(&Actor::Player(player.clone()), FISHING)?;
            host.trawler_loot(player, level, rolls, false)
        }
        FillBank => {
            for i in 0..host.bank_size() {
                host.add(player, Container::Bank, i, 1)?;
            }
            Ok(())
        }
        EmptyBank => {
            host.container_clear(player, Container::Bank)?;
            host.container_update(player, Container::Bank)
        }
        SetConfig => {
            if args.len() < 3 {
                reject::<_, ()>(host, player, &["Syntax: ::setconfig configID value"])?;
            }
            let id = to_int(arg(args, 1)?)?;
            let value = to_int(arg(args, 2)?)?;
            host.set_varp(player, id, value, false)
        }
        GetObjectVarp => {
            if args.len() < 2 {
                reject::<_, ()>(host, player, &["Syntax: ::getobjectvarp objectid"])?;
            }
            let object = to_int(arg(args, 1)?)?;
            host.scenery_definition(object)?;
            let varbit = host.scenery_varbit_id(object)?;
            let def = host.varbit_for_object(varbit)?.ok_or(Error::Null)?;
            let varp = host.varbit_varp(def)?;
            notify(host, player, &varp.to_string())
        }
        DefineVarbit => {
            if args.len() < 2 {
                reject::<_, ()>(host, player, &["Syntax: ::define_varbit varbitId"])?;
            }
            let id = to_int(arg(args, 1)?)?;
            let text = match host.varbit_for_id(id)? {
                None => "null".to_owned(),
                Some(def) => host.varbit_string(def)?,
            };
            notify(host, player, &format!("{id}: {text}"))
        }
        SetVarbit => {
            if args.len() != 3 {
                reject::<_, ()>(host, player, &["Usage: ::setvarbit <varbit-id> <value>"])?;
            }
            let index = to_int_or_null(arg(args, 1)?);
            let value = to_int_or_null(arg(args, 2)?);
            let (Some(index), Some(value)) = (index, value) else {
                return reject(host, player, &["Usage ::setvarbit <varbit-id> <value>"]);
            };
            host.set_varbit(player, index, value, false)
        }
        SetVarbits => {
            if args.len() != 4 {
                reject::<_, ()>(
                    host,
                    player,
                    &["Usage: ::setvarbits <fromvarbit> <tovarbit> <value>"],
                )?;
            }
            let from = to_int_or_null(arg(args, 1)?).ok_or(Error::Null)?;
            let to = to_int_or_null(arg(args, 2)?).ok_or(Error::Null)?;
            let value = to_int_or_null(arg(args, 3)?).ok_or(Error::Null)?;
            if from <= to {
                for index in from..=to {
                    host.set_varbit(player, index, value, false)?;
                }
            }
            Ok(())
        }
        GetVarbit => {
            if args.len() != 2 {
                reject::<_, ()>(host, player, &["Usage: ::getvarbit <id>"])?;
            }
            let Some(index) = to_int_or_null(arg(args, 1)?) else {
                return Ok(());
            };
            let value = host.get_varbit(player, index)?;
            notify(host, player, &format!("Varbit {index}: Currently {value}"))
        }
        SetVarp => {
            if args.len() < 4 {
                reject::<_, ()>(host, player, &["Usage: ::setvarp index offset value"])?;
            }
            let index = to_int_or_null(arg(args, 1)?);
            let offset = to_int_or_null(arg(args, 2)?);
            let value = to_int_or_null(arg(args, 3)?);
            let (Some(index), Some(offset), Some(value)) = (index, offset, value) else {
                return reject(host, player, &["Usage ::setvarp index offset value"]);
            };
            host.set_varp(player, index, value.wrapping_shl(offset as u32), false)
        }
        SetVarc => {
            if args.len() < 3 {
                reject::<_, ()>(host, player, &["Usage: ::setvarc index value"])?;
            }
            let index = to_int_or_null(arg(args, 1)?).and_then(|v| i16::try_from(v).ok());
            let value = to_int_or_null(arg(args, 2)?);
            let (Some(index), Some(value)) = (index, value) else {
                return reject(host, player, &["Usage ::setvarc index value"]);
            };
            host.send_varc(player, index, value)
        }
        Grow => {
            host.start_timer(player, Timer::CropGrowth)?;
            for patch in 0..host.patches(player)? {
                let now = host.clock();
                host.set_next_growth(player, patch, now.wrapping_sub(1))?;
            }
            host.crop_run(player).map(|_| ())
        }
        FinishBins => {
            host.start_timer(player, Timer::Compost)?;
            for bin in 0..host.bins(player)? {
                if !host.bin_finished(player, bin)? && host.bin_closed(player, bin)? {
                    host.bin_finish(player, bin)?;
                }
            }
            Ok(())
        }
        ResetBins => {
            host.start_timer(player, Timer::Compost)?;
            for bin in 0..host.bins(player)? {
                host.bin_reset(player, bin)?;
            }
            Ok(())
        }
        DiseaseCrops => {
            host.start_timer(player, Timer::CropGrowth)?;
            for patch in 0..host.patches(player)? {
                host.set_disease_mod(player, patch, -128)?;
                let now = host.clock();
                host.set_next_growth(player, patch, now.wrapping_add(1))?;
            }
            host.crop_run(player).map(|_| ())
        }
        AddCredits => {
            let account = host.account(player);
            let mut info = crate::auth::storage::lock(&account);
            info.credits = info.credits.wrapping_add(100);
            Ok(())
        }
        GetNpcParent => {
            if args.len() < 2 {
                reject::<_, ()>(host, player, &["Usage: ::getnpcparent npcid"])?;
            }
            let Some(npc) = to_int_or_null(arg(args, 1)?) else {
                return reject(host, player, &["Invalid NPC ID."]);
            };
            host.launch_job(Job::NpcParent {
                player: player.clone(),
                npc,
            })
        }
        InfiniteSpecial => toggle(
            host,
            player,
            args,
            "infinite-special",
            "Usage: ::infinitespecial [true|false]",
            "Infinite special attack",
        ),
        InfiniteSpecialMove => toggle(
            host,
            player,
            args,
            INFINITE_SPECIAL_MOVE,
            "Usage: ::infinitespecialmove [true|false]",
            "Infinite familiar special move energy",
        ),
        AllowAggro => toggle(
            host,
            player,
            args,
            "/save:allow_admin_aggression",
            "Usage: ::allowaggro [true|false]",
            "Admin NPC aggression",
        ),
        Rules => {
            host.rules_and_info(player)?;
            host.send_interface_config(player, 384, 17, true)?;
            host.open_component(player, 384)
        }
        ConfirmRules => confirm_rules(host, player, args),
        SetPlayerStrong => set_player_strong(host, player, args),
        SetPlaqueRead => set_plaque_read(host, player, args),
        BrewKit => brew_kit(host, player, args),
        Brew => brew(host, player, args),
    }
}

/// `Player.toggleDebug()`.
fn toggle_debug<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let debug =
        super::as_bool(host.entity_attribute(player, "debug", Some(Value::Bool(false)))?)?
            .ok_or(Error::Null)?;
    host.entity_set_attribute(player, "debug", Some(Value::Bool(!debug)))?;
    host.dispatch_message(
        player,
        &format!("Your debug mode is toggled to {}.", !debug),
    )
}

/// `chance to hit` of calcaccuracy and calcdefence's `hitChance`.
fn hit_chance(accuracy: i32, defence: i32) -> f64 {
    if accuracy > defence {
        1.0 - ((f64::from(defence) + 2.0) / (2.0 * (f64::from(accuracy) + 1.0)))
    } else {
        f64::from(accuracy) / (2.0 * (f64::from(defence) + 1.0))
    }
}

fn double(value: f64) -> String {
    rs09_game_core::java_double::format(value)
}

fn calc_accuracy<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    let me = Actor::Player(player.clone());
    let handler = host.swing_handler(player, false)?;
    host.message(player, &format!("handler type: {}", handler.style()))?;
    let accuracy = host.calculate_accuracy(handler, &me)?;
    host.message(player, &format!("calculateAccuracy: {accuracy}"))?;
    if args.len() > 1 {
        let id = to_int(arg(args, 1)?)?;
        let npc = host.new_npc(id, None)?;
        host.npc_init_config(npc)?;
        let name = host.npc_name(npc)?;
        let level = host.level(&Actor::Npc(npc), DEFENCE)?;
        host.message(player, &format!("npc: {name}. npc defence: {level}"))?;
        let defence = host.calculate_defence(handler, &Actor::Npc(npc), &me)?;
        host.message(player, &format!("calculateDefence: {defence}"))?;
        let chance = hit_chance(accuracy, defence);
        host.message(player, &format!("chance to hit: {}", double(chance)))?;
    }
    Ok(())
}

fn set_save_version<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    match to_int(arg(args, 1)?) {
        Ok(version) => {
            host.set_version(player, version)?;
            let version = host.version(player);
            notify(host, player, &format!("Setting save version to {version}"))
        }
        Err(Error::NumberFormat(_)) => {
            reject(host, player, &["Save versions can only be an integer"])
        }
        Err(e) => Err(e),
    }
}

fn loc<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let l = host.location(player)?.ok_or(Error::Null)?;
    let [x, y, z] = l;
    let region_id = ((x >> 6) << 8) | (y >> 6);
    let dynamic = host.region(region_id)?;
    let (local_x, local_y) = (x & 63, y & 63);
    let (chunk_x, chunk_y) = (local_x & 7, local_y & 7);
    let permitted = host.teleport_permitted(l)?;
    let flag = host.clipping_flag(l)?;
    let landscape = host.is_landscape(l)?;
    notify(
        host,
        player,
        &format!(
            "Absolute: {}, regional: [{local_x}, {local_y}], chunk: [{chunk_x}, {chunk_y}], flag: [{permitted}, {flag}, {landscape}].",
            location_string(Some(l))
        ),
    )?;
    let active = host.region_active(region_id)?;
    let object = host.region_object(l)?;
    notify(
        host,
        player,
        &format!(
            "Region: [id={region_id}, active={active}, instanced={dynamic}], obj={}.",
            object.map_or("null".into(), |o| o.java_string())
        ),
    )?;
    notify(
        host,
        player,
        &format!(
            "Jagex: {z}_{}_{}_{local_x}_{local_y}",
            region_id >> 8,
            region_id & 0xFF
        ),
    )?;
    let obj = host.region_object(l)?;
    notify(
        host,
        player,
        &format!(
            "Object: {}.",
            obj.as_ref().map_or("null".into(), |o| o.java_string())
        ),
    )?;
    // obj?.definition?.configFile?.varpId, ?.startBit and the size term.
    let config = |host: &mut H| -> Result<Option<i32>> {
        match &obj {
            None => Ok(None),
            Some(o) => {
                host.scenery_definition(o.id)?;
                host.scenery_config_file(o.id)
            }
        }
    };
    let varp = match config(host)? {
        None => None,
        Some(v) => Some(host.varbit_varp(v)?),
    };
    let offset = match config(host)? {
        None => None,
        Some(v) => Some(host.varbit_start_bit(v)?),
    };
    let size = match config(host)? {
        None => None,
        Some(v) => {
            let start = host.varbit_start_bit(v)?;
            let other = match config(host)? {
                None => return Err(Error::Null),
                Some(v) => host.varbit_start_bit(v)?,
            };
            Some(start.wrapping_sub(other))
        }
    };
    let text = |v: Option<i32>| v.map_or("null".into(), |v| v.to_string());
    notify(
        host,
        player,
        &format!(
            "Object Varp: {} offset: {} size: {}",
            text(varp),
            text(offset),
            text(size)
        ),
    )?;
    let scene = host.last_scene_graph(player);
    let scene_xy = |scene: Option<[i32; 3]>| -> Result<(i32, i32)> {
        let [sx, sy, _] = scene.ok_or(Error::Null)?;
        Ok((
            x.wrapping_sub(((sx >> 3) - 6) << 3),
            y.wrapping_sub(((sy >> 3) - 6) << 3),
        ))
    };
    let (scene_x, scene_y) = scene_xy(scene)?;
    host.log(
        ORIGIN,
        Level::Fine,
        &format!("Viewport: {scene_x},{scene_y}"),
    )?;
    let text = format!("Location({x}, {y}, {z})");
    let scene = location_string(host.last_scene_graph(player));
    host.log(
        ORIGIN,
        Level::Fine,
        &format!("{text}; {scene}, {local_x}, {local_y}"),
    )?;
    match host.set_clipboard(&text) {
        Ok(()) => notify(host, player, "Coordinates copied to clipboard."),
        Err(e) if is_headless(&e) => reject(
            host,
            player,
            &["NOTE: Paste will not be available due to remote server."],
        ),
        Err(e) => Err(e),
    }
}

fn calc_max_hit<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let me = Actor::Player(player.clone());
    let handler = host.swing_handler(player, false)?;
    let hit = host.calculate_hit(handler, &me, &me, 1.0)?;
    notify(
        host,
        player,
        &format!("max hit: {hit} ({})", handler.class_name()),
    )
}

/// calcdefence's handler table, in `mapOf` order.
const HANDLERS: [(&str, Handler); 6] = [
    ("melee", Handler::Melee),
    ("ranged", Handler::Range),
    ("magic", Handler::Magic),
    ("ma_melee", Handler::MagicalMelee),
    ("ma_ranged", Handler::MagicalRanged),
    ("ra_melee", Handler::RangedMelee),
];

fn calc_defence<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() > 1 && to_int_or_null(arg(args, 1)?).is_none() {
        return reject(
            host,
            player,
            &["Syntax error: ::calcdefence <lt>NPC ID<gt> <lt>opt: swing handler<gt>"],
        );
    }
    if args.len() > 2 {
        let key = lowercase_root(arg(args, 2)?);
        if !HANDLERS.iter().any(|(n, _)| *n == key) {
            let names: Vec<&str> = HANDLERS.iter().map(|(n, _)| *n).collect();
            return reject(
                host,
                player,
                &[&format!("Valid handlers: {}", names.join(", "))],
            );
        }
    }
    let me = Actor::Player(player.clone());
    let mut attacker = me.clone();
    if args.len() > 1 {
        let id = to_int(arg(args, 1)?)?;
        let npc = host.new_npc(id, None)?;
        host.npc_init_config(npc)?;
        attacker = Actor::Npc(npc);
        let bonuses = host.npc_bonuses(npc)?;
        let mut max_bonus = i32::MIN;
        let mut max_index = 0;
        for i in 0..=4usize {
            if let Some(v) = bonuses.get(i)
                && *v > max_bonus
            {
                max_bonus = *v;
                max_index = i as i32;
            }
        }
        if max_bonus == i32::MIN {
            max_index = 0;
        }
        host.npc_set_attack_style(npc, 2, max_index)?;
        let name = host.npc_name(npc)?;
        notify(
            host,
            player,
            &format!("Calculating defence against NPC: {name} (ID {id})"),
        )?;
    } else {
        notify(
            host,
            player,
            "Calculating defence against yourself (no NPC specified).",
        )?;
    }
    let report = |host: &mut H, name: &str, handler: Handler| -> Result<()> {
        let accuracy = host.calculate_accuracy(handler, &attacker)?;
        let defence = host.calculate_defence(handler, &me, &attacker)?;
        let chance = round_up_4(hit_chance(accuracy, defence))? * 100.0;
        notify(
            host,
            player,
            &format!(
                "Your {name} defence: {defence}; Attacker acc.: {accuracy}; Hit chance: {}%",
                double(chance)
            ),
        )
    };
    if args.len() < 3 {
        for (name, handler) in &HANDLERS[..3] {
            report(host, name, *handler)?;
        }
    } else {
        let name = arg(args, 2)?;
        let handler = HANDLERS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, h)| *h)
            .ok_or(Error::Null)?;
        report(host, name, handler)?;
    }
    Ok(())
}

fn players<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let ordinal = rights(host, player)?.ordinal();
    let busy = (host.interface_opened(player)? && {
        host.opened_component(player)?.ok_or(Error::Null)? != SCROLL
    }) || host.locked(player, Lock::Movement)?
        || host.locked(player, Lock::Teleport)?;
    if busy {
        reject::<_, ()>(host, player, &["Please finish what you're doing first."])?;
    }
    host.open_component(player, SCROLL)?;
    for i in 0..257 {
        host.send_string(player, "", SCROLL, i)?;
    }
    host.send_string(player, "<col=8A0808>Players</col>", SCROLL, 2)?;
    let mut line = 11;
    for p in host.online_players() {
        if !host.is_artificial(&p)? {
            let icon = chat_icon(host, &p)?;
            let name = host.username(&p);
            let extra = if ordinal > 0 {
                let ip = host.ip_address(&p)?;
                let comp = host.comp_name(&p)?;
                format!(" [ip={ip}, name={comp}]")
            } else {
                String::new()
            };
            let text = format!("<col=8A0808><img={}>{name}{extra}", icon.wrapping_sub(1));
            host.send_string(player, &text, SCROLL, line)?;
            line += 1;
        }
    }
    Ok(())
}

fn bot_info<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let name = lowercase_root(&join_from(args, 1, " "));
    let Some(info) = host.bot_script(&name)? else {
        return reject(host, player, &["No bot with that name found."]);
    };
    let units: Vec<u16> = info.encode_utf16().collect();
    for chunk in units.chunks(80) {
        notify(host, player, &String::from_utf16_lossy(chunk))?;
    }
    Ok(())
}

fn ge<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 2 {
        reject::<_, ()>(
            host,
            player,
            &[
                "Usage: ::ge mode",
                "Available modes: buying, selling, search, bots, botsearch, mute",
            ],
        )?;
    }
    match arg(args, 1)? {
        "buying" => show_ge_buy(host, player),
        "selling" => show_ge_sell(host, player),
        "search" => {
            if args.len() > 2 {
                let target = lowercase_root(&join_from(args, 2, " "));
                show_offers(host, player, &target)
            } else {
                host.input(
                    player,
                    InputType::StringLong,
                    "Enter search term:",
                    Input::GeSearch {
                        player: player.clone(),
                    },
                )
            }
        }
        "bots" => show_ge_bots(host, player),
        "botsearch" => {
            if args.len() > 2 {
                let target = lowercase_root(&join_from(args, 2, " "));
                show_ge_botsearch(host, player, &target)
            } else {
                host.input(
                    player,
                    InputType::StringLong,
                    "Enter search term:",
                    Input::GeBotSearch {
                        player: player.clone(),
                    },
                )
            }
        }
        "mute" => {
            let muted = super::as_bool(host.entity_attribute(
                player,
                GE_NEWS_MUTE,
                Some(Value::Bool(false)),
            )?)?
            .ok_or(Error::Null)?;
            if muted {
                host.entity_remove_attribute(player, &GE_NEWS_MUTE.replace("/save:", ""))?;
            } else {
                host.entity_set_attribute(player, GE_NEWS_MUTE, Some(Value::Bool(true)))?;
            }
            host.api_message(
                player,
                &format!(
                    "GE sell offer news is now {}.",
                    if muted { "visible" } else { "hidden" }
                ),
            )
        }
        _ => reject(
            host,
            player,
            &[
                "Invalid mode used. Available modes are: buying, selling, search, bots, botsearch, mute",
            ],
        ),
    }
}

/// The entries of an `Int -> Int` HashMap sorted by item name (a stable
/// sort over the map's walk order).
fn by_name<H: Host + ?Sized>(host: &mut H, map: &JavaHashMap<i32, i32>) -> Vec<(i32, i32)> {
    let mut entries: Vec<(String, i32, i32)> =
        map.iter().map(|(k, v)| (String::new(), *k, *v)).collect();
    for entry in &mut entries {
        entry.0 = host.item_definition_name(entry.1);
    }
    entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    entries.into_iter().map(|(_, k, v)| (k, v)).collect()
}

fn get(map: &JavaHashMap<i32, i32>, key: i32) -> Option<i32> {
    map.get(&key).copied()
}

fn show_ge_buy<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let mut amounts = JavaHashMap::new();
    let mut highest = JavaHashMap::new();
    for offer in host.valid_offers()? {
        if offer.sell {
            continue;
        }
        let amount = get(&amounts, offer.item)
            .unwrap_or(0)
            .wrapping_add(offer.amount_left);
        let mut price = get(&highest, offer.item).unwrap_or(0);
        if offer.offered_value > price {
            price = offer.offered_value;
        }
        amounts.put(offer.item, amount);
        highest.put(offer.item, price);
    }
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (item, amount) in by_name(host, &amounts) {
        let name = host.item_definition_name(item);
        left.push(format!("{name} (<col=357f44>x{amount}</col>)"));
        right.push(format!(
            "Highest: <col=3a3414>{}</col>gp",
            opt(get(&highest, item))
        ));
    }
    show_ge_book(host, player, "Active Buy Offers", left, right)
}

fn show_ge_sell<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let mut amounts = JavaHashMap::new();
    let mut lowest = JavaHashMap::new();
    for offer in host.valid_offers()? {
        if !offer.sell {
            continue;
        }
        let amount = get(&amounts, offer.item)
            .unwrap_or(0)
            .wrapping_add(offer.amount_left);
        let mut price = get(&lowest, offer.item).unwrap_or(i32::MAX);
        if offer.offered_value < price {
            price = offer.offered_value;
        }
        amounts.put(offer.item, amount);
        lowest.put(offer.item, price);
    }
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (item, amount) in by_name(host, &amounts) {
        let name = host.item_definition_name(item);
        left.push(format!("{name} (<col=357f44>x{amount}</col>)"));
        right.push(format!(
            "Price: <col=3a3414>{}</col>gp",
            opt(get(&lowest, item))
        ));
    }
    show_ge_book(host, player, "Active Sell Offers", left, right)
}

fn opt(v: Option<i32>) -> String {
    v.map_or("null".into(), |v| v.to_string())
}

fn matches_term<H: Host + ?Sized>(host: &mut H, item: i32, term: &str) -> bool {
    let name = host.item_definition_name(item);
    contains_ignore_case(&name, term) || equals_ignore_case(&name, term)
}

fn bot_book<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    title: &str,
    offers: Vec<super::Offer>,
) -> Result<()> {
    let mut amounts = JavaHashMap::new();
    let mut prices = JavaHashMap::new();
    for offer in offers {
        amounts.put(offer.item, offer.amount);
        prices.put(offer.item, offer.offered_value);
    }
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (item, amount) in by_name(host, &amounts) {
        let name = host.item_definition_name(item);
        left.push(format!("{name} (<col=357f44>x{amount}</col>)"));
        right.push(format!(
            "Price: <col=3a3414>{}</col>gp",
            opt(get(&prices, item))
        ));
    }
    show_ge_book(host, player, title, left, right)
}

fn show_ge_bots<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let offers = host.bot_offers()?;
    bot_book(host, player, "Bot Stock", offers)
}

/// `showGeBotsearch(player, searchTerm)`.
pub fn show_ge_botsearch<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    term: &str,
) -> Result<()> {
    let offers: Vec<_> = host
        .bot_offers()?
        .into_iter()
        .filter(|o| matches_term(host, o.item, term))
        .collect();
    bot_book(host, player, &format!("Bot Stock - \"{term}\""), offers)
}

/// `showOffers(player, searchTerm)`.
pub fn show_offers<H: Host + ?Sized>(host: &mut H, player: &H::Player, term: &str) -> Result<()> {
    let offers: Vec<_> = host
        .valid_offers()?
        .into_iter()
        .filter(|o| matches_term(host, o.item, term))
        .collect();
    if offers.is_empty() {
        return host.api_message(player, "No results.");
    }
    let mut buying_amount = JavaHashMap::new();
    let mut buying_highest = JavaHashMap::new();
    let mut selling_amount = JavaHashMap::new();
    let mut selling_lowest = JavaHashMap::new();
    for offer in &offers {
        if offer.sell {
            let mut price = get(&selling_lowest, offer.item).unwrap_or(i32::MAX);
            if offer.offered_value < price {
                price = offer.offered_value;
            }
            let amount = get(&selling_amount, offer.item)
                .unwrap_or(0)
                .wrapping_add(offer.amount_left);
            selling_amount.put(offer.item, amount);
            selling_lowest.put(offer.item, price);
        } else {
            let mut price = get(&buying_highest, offer.item).unwrap_or(0);
            if offer.offered_value > price {
                price = offer.offered_value;
            }
            let amount = get(&buying_amount, offer.item)
                .unwrap_or(0)
                .wrapping_add(offer.amount_left);
            buying_amount.put(offer.item, amount);
            buying_highest.put(offer.item, price);
        }
    }
    let buying: Vec<(i32, i32)> = buying_amount.iter().map(|(k, v)| (*k, *v)).collect();
    let selling: Vec<(i32, i32)> = selling_amount.iter().map(|(k, v)| (*k, *v)).collect();
    let lines = offers.len() + 1;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for i in 0..=lines {
        if i < buying.len() {
            let (item, amount) = buying[i];
            let name = host.item_definition_name(item);
            left.push(format!("[BUYING] {name} (<col=357f44>x{amount}</col>)"));
            right.push(format!(
                "Highest: <col=3a3414>{}</col>gp",
                opt(get(&buying_highest, item))
            ));
        } else if i == buying.len() {
            left.push(String::new());
            right.push(String::new());
        } else {
            let Some((item, amount)) = selling.get(i - buying.len() - 1).copied() else {
                continue;
            };
            let name = host.item_definition_name(item);
            left.push(format!("[SELLING] {name} (<col=357f44>x{amount}</col>)"));
            right.push(format!(
                "Lowest: <col=3a3414>{}</col>gp",
                opt(get(&selling_lowest, item))
            ));
        }
    }
    show_ge_book(
        host,
        player,
        &format!("Results for \"{term}\""),
        left,
        right,
    )
}

fn show_ge_book<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    title: &str,
    left: Vec<String>,
    right: Vec<String>,
) -> Result<()> {
    if left.is_empty() {
        return host.api_message(player, "No results.");
    }
    let mut contents: Vec<PageSet> = Vec::new();
    for (i, chunk) in left.chunks(15).enumerate() {
        let right_chunk = right.chunks(15).nth(i).ok_or_else(|| {
            Error::Index(format!(
                "Index {i} out of bounds for length {}",
                right.chunks(15).count()
            ))
        })?;
        let mut left_page = Vec::new();
        let mut right_page = Vec::new();
        for (j, text) in chunk.iter().enumerate() {
            left_page.push(BookLine {
                message: text.clone(),
                child: FANCY_BOOK_26_LINE_IDS[j + 3],
            });
            let other = right_chunk.get(j).ok_or_else(|| {
                Error::Index(format!(
                    "Index {j} out of bounds for length {}",
                    right_chunk.len()
                ))
            })?;
            right_page.push(BookLine {
                message: other.clone(),
                child: FANCY_BOOK_26_LINE_IDS[j + 3 + 15],
            });
        }
        contents.push(vec![left_page, right_page]);
    }
    host.close_interfaces(player)?;
    host.open_book(
        player,
        FANCY_BOOK_26,
        Book::Ge {
            title: title.to_owned(),
            contents,
        },
    )
}

/// The GE book's `display(player, pageNum, buttonID)`.
pub fn ge_display<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    title: &str,
    contents: &[PageSet],
) -> Result<bool> {
    host.page_setup(player, FANCY_BOOK_26, title, contents, true)?;
    Ok(true)
}

/// `setupScrollInterface(player, title)`.
fn setup_scroll<H: Host + ?Sized>(host: &mut H, player: &H::Player, title: &str) -> Result<i32> {
    host.close_interfaces(player)?;
    for i in 0..=310 {
        host.send_string(player, "", SCROLL, i)?;
    }
    host.send_string(player, title, SCROLL, 2)?;
    Ok(11)
}

/// `displayCommandInScroll(player, command, playerRights, lineId)`.
fn display_command<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    command: &RegistryCommand,
    player_rights: i32,
    line: i32,
) -> Result<i32> {
    let level = command.privilege.ordinal();
    if level > player_rights {
        return Ok(line);
    }
    let mut title = command.name.clone();
    if level > 0 {
        title = format!("(<img={}>) {title}", level - 1);
    }
    let mut current = line;
    host.send_string(player, &title, SCROLL, current)?;
    current += 1;
    if !command.usage.is_empty() {
        for chunk in chunk_text(&format!("Usage: {}", command.usage), 70) {
            host.send_string(player, &chunk, SCROLL, current)?;
            current += 1;
        }
    }
    if !command.description.is_empty() {
        for chunk in chunk_text(&command.description, 70) {
            host.send_string(player, &chunk, SCROLL, current)?;
            current += 1;
        }
    }
    host.send_string(
        player,
        "<str>-------------------------------</str>",
        SCROLL,
        current,
    )?;
    Ok(current + 1)
}

fn commands<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    let page = if args.len() > 1 {
        to_int_or_null(arg(args, 1)?).unwrap_or(1).wrapping_sub(1)
    } else {
        0
    };
    let pages = registry.page_indices(rights(host, player)?.ordinal());
    let all = registry.commands();
    let size = pages.len() as i32;
    let index = |i: i32| -> Result<i32> {
        usize::try_from(i)
            .ok()
            .and_then(|i| pages.get(i).copied())
            .ok_or_else(|| Error::Index(format!("Index {i} out of bounds for length {size}")))
    };
    let end = if page < size - 1 {
        index(page.wrapping_add(1))?
    } else {
        all.len() as i32
    };
    if page < 0 {
        reject::<_, ()>(host, player, &["Usage: ::commands <lt>page<gt>"])?;
    }
    if page > size {
        reject::<_, ()>(
            host,
            player,
            &["That page number is too high, you don't have access to that many."],
        )?;
    }
    let title = format!(
        "Commands{}",
        if pages.len() > 1 {
            format!(" ({}/{})", page + 1, pages.len())
        } else {
            String::new()
        }
    );
    let mut line = setup_scroll(host, player, &title)?;
    let start = index(page)?;
    let ordinal = rights(host, player)?.ordinal();
    for i in start..end {
        let command = all.get(i as usize).ok_or_else(|| {
            Error::Index(format!("Index {i} out of bounds for length {}", all.len()))
        })?;
        line = display_command(host, player, command, ordinal, line)?;
        if line > 306 {
            host.send_string(
                player,
                &format!("To view the next page, use ::commands {}", page + 2),
                SCROLL,
                line,
            )?;
            break;
        }
    }
    host.open_component(player, SCROLL)
}

fn command_search<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    if args.len() < 2 {
        return reject(
            host,
            player,
            &["Usage: ::commandsearch <search term> [--chat]"],
        );
    }
    let raw = &args[1..];
    let chat = raw.iter().any(|a| equals_ignore_case(a, "--chat"));
    let terms: Vec<&str> = raw
        .iter()
        .filter(|a| !equals_ignore_case(a, "--chat"))
        .map(String::as_str)
        .collect();
    if terms.is_empty() {
        return reject(
            host,
            player,
            &["Usage: ::commandsearch <search term> [--chat]"],
        );
    }
    let search = lowercase_root(&terms.join(" "));
    if java_len(&search) < 2 {
        return reject(
            host,
            player,
            &["Search term must be at least 2 characters long"],
        );
    }
    let player_rights = rights(host, player)?.ordinal();
    let matching: Vec<&RegistryCommand> = registry
        .commands()
        .into_iter()
        .filter(|c| {
            lowercase_root(&c.name).contains(&search)
                || lowercase_root(&c.usage).contains(&search)
                || lowercase_root(&c.description).contains(&search)
        })
        .collect();
    if matching.is_empty() {
        return notify(
            host,
            player,
            &format!("No commands found matching '{search}'"),
        );
    }
    let mut accessible: Vec<&RegistryCommand> = matching
        .iter()
        .copied()
        .filter(|c| c.privilege.ordinal() <= player_rights)
        .collect();
    accessible.sort_by(|a, b| a.name.encode_utf16().cmp(b.name.encode_utf16()));
    let inaccessible = matching
        .iter()
        .filter(|c| c.privilege.ordinal() > player_rights)
        .count();
    if chat {
        for command in &accessible {
            let arguments = if !command.usage.is_empty() {
                match command.usage.split_once(' ') {
                    Some((_, rest)) => rest.to_owned(),
                    None => String::new(),
                }
            } else {
                String::new()
            };
            let message = if !arguments.is_empty() {
                format!(
                    "CMD: {} ARGS: {arguments} DESC: {}",
                    command.name, command.description
                )
            } else {
                format!("CMD: {} DESC: {}", command.name, command.description)
            };
            for chunk in chunk_text(&message, 70) {
                notify(host, player, &chunk)?;
            }
        }
        if accessible.is_empty() {
            notify(
                host,
                player,
                &format!(
                    "Found {inaccessible} command(s) matching '{search}' but you don't have permission to use any of them"
                ),
            )?;
        } else {
            let summary = format!(
                "Found {} accessible command(s) for '{search}'",
                accessible.len()
            );
            notify(host, player, &summary)?;
            if inaccessible > 0 {
                notify(
                    host,
                    player,
                    &format!("({inaccessible} additional command(s) require higher privileges)"),
                )?;
            }
        }
        return Ok(());
    }
    let mut line = setup_scroll(host, player, &format!("Command Search: '{search}'"))?;
    if accessible.is_empty() {
        host.send_string(player, "No accessible commands found.", SCROLL, line)?;
        line += 1;
    }
    for command in &accessible {
        line = display_command(host, player, command, player_rights, line)?;
        if line > 306 {
            host.send_string(
                player,
                &format!("Results truncated. Use ::commandsearch {search} --chat"),
                SCROLL,
                line,
            )?;
            break;
        }
    }
    if inaccessible > 0 && line <= 306 {
        host.send_string(
            player,
            &format!("{inaccessible} more command(s) need higher privileges."),
            SCROLL,
            line,
        )?;
    }
    host.open_component(player, SCROLL)
}

fn reply<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    if host.interface_opened(player)? {
        reject::<_, ()>(
            host,
            player,
            &["<col=e74c3c>Please finish what you're doing first."],
        )?;
    }
    if host.has_attribute(player, "replyTo")? {
        host.entity_set_attribute(player, "keepDialogueAlive", Some(Value::Bool(true)))?;
        let reply_to =
            match host.entity_attribute(player, "replyTo", Some(Value::Text(String::new())))? {
                Some(Value::Text(t)) => t,
                None => return Err(Error::Null),
                Some(_) => return Err(Error::Cast),
            }
            .replace('_', " ");
        let prompt = crate::world::social::names::format_display_name(&reply_to);
        host.input(
            player,
            InputType::Message,
            &prompt,
            Input::Reply {
                player: player.clone(),
                reply_to,
            },
        )
    } else {
        reject(
            host,
            player,
            &["<col=3498db>You have not recieved any recent messages to which you can reply."],
        )
    }
}

fn permadeath<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let mut target = player.clone();
    if args.len() > 1 {
        let name = join_from(args, 1, "_");
        let found = host.player_by_name(Some(&name))?;
        match found {
            Some(p) => target = p,
            None => {
                return reject(
                    host,
                    player,
                    &[&format!("Invalid player \"{name}\" or player not online")],
                );
            }
        }
    }
    host.permadeath(&target)
}

fn loc_log<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<Option<super::ListRef>> {
    match host.entity_attribute(player, "loc-log", None)? {
        None => Ok(None),
        Some(Value::List(list)) => Ok(Some(list)),
        Some(_) => Err(Error::Cast),
    }
}

fn log<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let list = loc_log(host, player)?.unwrap_or_default();
    let [x, y, z] = host.location(player)?.ok_or(Error::Null)?;
    list.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(format!("{{{x},{y},{z},1,0}}"));
    host.entity_set_attribute(player, "loc-log", Some(Value::List(list)))
}

fn log_done<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let Some(list) = loc_log(host, player)? else {
        return Ok(());
    };
    let text = list
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .join("-");
    host.set_clipboard(&text)?;
    list.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    host.entity_set_attribute(player, "loc-log", Some(Value::List(list)))
}

/// `resolveBooleanToggle` then `setBooleanAttribute`.
fn toggle<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
    attribute: &str,
    usage: &str,
    label: &str,
) -> Result<()> {
    let enabled = if args.len() < 2 {
        let current =
            super::as_bool(host.entity_attribute(player, attribute, Some(Value::Bool(false)))?)?
                .ok_or(Error::Null)?;
        Some(!current)
    } else if equals_ignore_case(arg(args, 1)?, "true") {
        Some(true)
    } else if equals_ignore_case(arg(args, 1)?, "false") {
        Some(false)
    } else {
        None
    };
    let Some(enabled) = enabled else {
        return reject(host, player, &[usage]);
    };
    if enabled {
        host.entity_set_attribute(player, attribute, Some(Value::Bool(true)))?;
    } else {
        host.entity_remove_attribute(player, &attribute.replace("/save:", ""))?;
    }
    host.api_message(
        player,
        &format!(
            "{label} is now {}.",
            if enabled { "enabled" } else { "disabled" }
        ),
    )
}

fn confirm_rules<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    let confirmed = super::as_bool(host.entity_attribute(
        player,
        "rules:confirmed",
        Some(Value::Bool(false)),
    )?)?
    .ok_or(Error::Null)?;
    if confirmed {
        reject::<_, ()>(host, player, &["You have already confirmed the rules."])?;
    }
    if args.len() < 2 {
        reject::<_, ()>(host, player, &["Usage: ::confirmrules PIN"])?;
    }
    let pin = match to_int_or_null(arg(args, 1)?) {
        Some(pin) => pin,
        None => return reject(host, player, &["Please enter a valid number."]),
    };
    let stored = match host.entity_attribute(player, "rules:pin", Some(Value::Int(-1)))? {
        Some(Value::Int(v)) => v,
        None => return Err(Error::Null),
        Some(_) => return Err(Error::Cast),
    };
    if pin == stored {
        host.entity_set_attribute(player, "/save:rules:confirmed", Some(Value::Bool(true)))?;
        host.close_interfaces(player)?;
        host.dialogue(player, "Thank you!")?;
        host.unlock(player)?;
        host.entity_remove_attribute(player, "rules:pin")?;
        if host.new_player_announcement() {
            let name = host.username(player);
            host.send_news(
                &format!("A new player has joined. Welcome {name}!"),
                12,
                "CC6600",
            )?;
        }
        Ok(())
    } else {
        host.dialogue(player, "Wrong pin. Try again.")
    }
}

fn set_player_strong<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    if args.len() < 2 {
        let stage = host.test_stage(player);
        return notify(
            host,
            player,
            &format!("Player Stronghold progression currently: {stage}"),
        );
    }
    let stage = match to_int_or_null(arg(args, 1)?) {
        Some(stage) => stage,
        None => return reject(host, player, &["Please enter a valid number"]),
    };
    if !(0..=4).contains(&stage) {
        return reject(host, player, &["Only stages 0-4 are valid"]);
    }
    host.set_test_stage(player, stage);
    notify(host, player, &format!("Setting test stage to {stage}"))?;
    if (0..=2).contains(&stage) {
        host.set_varp(player, 1203, 0, true)?;
        notify(host, player, "The poster passage is now hidden")?;
    } else {
        host.set_varp(player, 1203, 1 << 29, true)?;
        notify(host, player, "The poster passage is now usable")?;
    }
    if stage == 4 {
        host.set_varbit(player, 4499, 1, true)?;
        notify(host, player, "The loot has been taken already")
    } else {
        host.set_varbit(player, 4499, 0, true)?;
        notify(host, player, "The loot can be reacquired")
    }
}

fn set_plaque_read<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    if args.len() == 1 {
        let read = host.read_plaques(player).iter().all(|r| *r);
        return notify(
            host,
            player,
            &format!(
                "Currently the plaques {} been read",
                if read { "have" } else { "have not" }
            ),
        );
    }
    let status = match arg(args, 1)? {
        "true" => true,
        "false" => false,
        _ => return reject(host, player, &["Only true or false can be used"]),
    };
    for i in 0..host.read_plaques(player).len() {
        host.set_read_plaque(player, i, status);
    }
    notify(
        host,
        player,
        &format!("Setting plaques read to: {}", arg(args, 1)?),
    )
}

/// brewkit's ingredient by kit number.
const KITS: [i32; 11] = [
    5994, 5996, 255, 5998, 6000, 6004, 6043, 1975, 6002, 5992, 6113,
];

fn brew_kit<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() == 1 {
        for line in [
            "Kit numbers: 0 - Dwarven Stout, 1 - Asgarnian Ale, 2 - Greenman's Ale,",
            "Kit numbers: 3 - Wizard's Mind Bomb, 4 - Dragon Bitter, 5 - Moonlight Mead,",
            "Kit numbers: 6 - Axeman's Folly, 7 - Chef's Delight, 8 - Slayer's Respite,",
            "Kit numbers: 9 - Cider, 10 - Kelda Stout",
        ] {
            host.api_message(player, line)?;
        }
        return Ok(());
    }
    let kit = to_int(arg(args, 1)?)?;
    let Some(item) = usize::try_from(kit).ok().and_then(|k| KITS.get(k)).copied() else {
        return reject(host, player, &["Invalid brew kit specified."]);
    };
    let kit = to_int(arg(args, 1)?)?;
    host.add_item_or_drop(player, 1929, if kit == 9 { 0 } else { 2 })?;
    host.add_item_or_drop(player, 6008, if kit == 9 { 0 } else { 2 })?;
    host.add_item_or_drop(player, item, if kit == 6 || kit == 10 { 1 } else { 4 })?;
    host.add_item_or_drop(player, 5767, 1)
}

fn brew<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let mut force = [false; 4];
    if args.len() == 1 {
        force[0] = true;
    }
    if args.len() != 1 {
        match to_int_or_null(arg(args, 1)?) {
            Some(mode @ 0..=3) => force[mode as usize] = true,
            _ => {
                let text = format!("{} is not a valid option.", arg(args, 1)?);
                reject::<_, ()>(host, player, &[&text])?;
            }
        }
    }
    host.start_timer(player, Timer::BrewGrowth)?;
    for vat in 0..host.vats(player)? {
        host.brew(player, vat, force)?;
    }
    Ok(())
}

/// An input dialogue's handler of this set, with the value it was given.
pub fn run_input<H: Host + ?Sized>(
    host: &mut H,
    handler: &Input<H::Player>,
    value: &crate::world::commands::deferred::InputValue,
) -> Result<()> {
    use crate::world::commands::deferred::InputValue;
    let text = || match value {
        InputValue::Text(t) => Ok(t.clone()),
        InputValue::Int(_) => Err(Error::Cast),
    };
    match handler {
        Input::Reply { player, reply_to } => {
            let message = text()?;
            host.private_message(player, &lowercase_root(reply_to), &message)?;
            host.entity_remove_attribute(player, "keepDialogueAlive")
        }
        Input::GeSearch { player } => {
            let term = text()?;
            show_offers(host, player, &term)
        }
        Input::GeBotSearch { player } => {
            let term = text()?;
            show_ge_botsearch(host, player, &term)
        }
    }
}

/// A launched coroutine of this set.
pub fn run_job<H: Host + ?Sized>(host: &mut H, job: &Job<H::Player>) -> Result<()> {
    match job {
        Job::NpcParent { player, npc } => {
            for def in host.npc_definitions()? {
                let Some((id, children)) = def else {
                    continue;
                };
                let Some(children) = children else {
                    continue;
                };
                if children.contains(npc) {
                    return notify(host, player, &format!("Parent NPC: {id}"));
                }
            }
            notify(host, player, "No parent NPC found.")
        }
    }
}

/// Rights the `players` command shows (for tests).
pub fn rights_name(rights: Rights) -> &'static str {
    match rights {
        Rights::RegularPlayer => "REGULAR_PLAYER",
        Rights::PlayerModerator => "PLAYER_MODERATOR",
        Rights::Administrator => "ADMINISTRATOR",
    }
}
