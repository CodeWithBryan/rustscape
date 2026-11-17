//! `TeleportCommandSet`: to, tele, teleto, teletome and home. `::to` sends
//! the player through `TeleportManager` (native: `world::teleport`, bound by
//! the host's [`Host::teleport`]); the others set
//! `properties.teleportLocation`, which the movement update applies.
use super::kotlin::to_int;
use super::{Host, Loc, Result, TeleportType, arg, reject};

/// `ServerConstants.TELEPORT_DESTINATIONS`: a location and its names, in
/// the original's order. The table ends with [`CASTLE_WARS`], whose location
/// `CastleWars.lobbyBankArea.randomLoc` drew when the constants were
/// initialised ([`Host::castle_wars_destination`]).
pub const DESTINATIONS: &[(Loc, &[&str])] = &[
    ([2974, 4383, 2], &["corp", "corporal", "corporeal"]),
    ([2659, 2649, 0], &["pc", "pest control", "pest"]),
    ([3293, 3184, 0], &["al kharid", "alkharid", "kharid"]),
    ([3222, 3217, 0], &["lumbridge", "lumby"]),
    (
        [3110, 3168, 0],
        &["wizard tower", "wizards tower", "tower", "wizards"],
    ),
    ([3083, 3249, 0], &["draynor", "draynor village"]),
    ([3019, 3244, 0], &["port sarim", "sarim"]),
    ([2956, 3209, 0], &["rimmington"]),
    ([2965, 3380, 0], &["fally", "falador"]),
    ([2895, 3436, 0], &["taverley"]),
    ([3080, 3423, 0], &["barbarian village", "barb"]),
    ([3213, 3428, 0], &["varrock"]),
    ([3164, 3485, 0], &["grand exchange", "ge"]),
    ([2917, 3175, 0], &["karamja"]),
    ([2450, 5165, 0], &["tzhaar"]),
    ([2795, 3177, 0], &["brimhaven"]),
    ([2849, 2961, 0], &["shilo village", "shilo"]),
    ([2605, 3093, 0], &["yanille"]),
    ([2663, 3305, 0], &["ardougne", "ardy"]),
    ([2450, 3422, 0], &["gnome stronghold", "gnome"]),
    ([2730, 3485, 0], &["camelot", "cammy", "seers"]),
    ([2805, 3435, 0], &["catherby"]),
    ([2659, 3657, 0], &["rellekka"]),
    ([2890, 3676, 0], &["trollheim"]),
    ([2914, 3746, 0], &["godwars", "gwd", "god wars"]),
    ([3180, 3684, 0], &["bounty hunter", "bh"]),
    ([3272, 3687, 0], &["clan wars", "clw"]),
    (
        [3090, 3957, 0],
        &["mage arena", "mage", "magearena", "arena"],
    ),
    ([3069, 10257, 0], &["king black dragon", "kbd"]),
    ([3359, 3416, 0], &["digsite"]),
    ([3488, 3489, 0], &["canifis"]),
    ([3428, 3526, 0], &["slayer tower", "slayer"]),
    (
        [3502, 9483, 2],
        &["kalphite queen", "kq", "kalphite hive", "kalphite"],
    ),
    ([3233, 2913, 0], &["pyramid"]),
    ([3419, 2917, 0], &["nardah"]),
    ([3482, 3090, 0], &["uzer"]),
    ([3358, 2970, 0], &["pollnivneach", "poln"]),
    ([3305, 2788, 0], &["sophanem"]),
    ([2898, 3544, 0], &["burthorpe", "burthorp"]),
    ([3088, 3491, 0], &["edge", "edgeville"]),
    ([3169, 3034, 0], &["bedabin"]),
    ([3565, 3289, 0], &["barrows"]),
    ([3016, 3513, 0], &["bkf", "black knights fortress"]),
    ([3052, 3481, 0], &["monastery"]),
    ([1945, 4959, 0], &["blast furnace", "blast"]),
    ([2408, 4449, 0], &["zanaris"]),
    ([3656, 3517, 0], &["ectofuntus", "ecto"]),
    ([2408, 4449, 0], &["tower of life lower"]),
    ([2894, 4756, 0], &["test area"]),
    ([2722, 4886, 0], &["quest the golem 1"]),
    ([2704, 5349, 0], &["dorgeshuun", "dorg"]),
    ([2711, 10132, 0], &["brine rats"]),
    ([2328, 3677, 0], &["piscatoris"]),
    ([2660, 3158, 0], &["fishing trawler", "trawler"]),
    ([2800, 3667, 0], &["mountain camp"]),
    ([2575, 3250, 0], &["clocktower"]),
    ([2544, 3759, 0], &["waterbirth"]),
    ([2899, 4450, 0], &["dks"]),
];

/// The names of the table's last row.
pub const CASTLE_WARS: &[&str] = &[
    "cwars",
    "castle wars",
    "castle war",
    "castlewars",
    "castlewar",
    "castle",
];

/// `::to <place...>`: the last row naming the place wins.
pub fn to<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let place = args.get(1..).unwrap_or(&[]).join(" ");
    let mut destination = None;
    for (location, names) in DESTINATIONS {
        if names.contains(&place.as_str()) {
            destination = Some(*location);
        }
    }
    if CASTLE_WARS.contains(&place.as_str()) {
        destination = Some(host.castle_wars_destination());
    }
    match destination {
        Some(location) => host
            .teleport(player, location, TeleportType::Normal)
            .map(|_| ()),
        None => reject(
            host,
            player,
            &[&format!(
                "Could not locate teleport destination [name={place}]!"
            )],
        ),
    }
}

/// `::tele x y [z]`, `::tele z,rx,ry,lx,ly` or `::tele [z_]rx_ry[_ox_oy]`.
pub fn tele<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() == 2 && arg(args, 1)?.contains(',') {
        let parts: Vec<&str> = args[1].split(',').collect();
        let at = |i: usize| super::arg_list(&parts, i);
        let x = (to_int(at(1)?)? << 6) | to_int(at(3)?)?;
        let y = (to_int(at(2)?)? << 6) | to_int(at(4)?)?;
        let z = to_int(at(0)?)?;
        return host.set_teleport_location(player, Some([x, y, z]));
    }
    if args.len() == 2 && arg(args, 1)?.contains('_') {
        let tokens: Vec<&str> = args[1].split('_').collect();
        let int = |i: usize| to_int(tokens[i]);
        let location = match tokens.len() {
            2 => {
                let (rx, ry) = (int(0)?, int(1)?);
                [(rx << 6) | 15, (ry << 6) | 15, 0]
            }
            3 => {
                let (plane, rx, ry) = (int(0)?, int(1)?, int(2)?);
                [(rx << 6) | 15, (ry << 6) | 15, plane]
            }
            4 => {
                let (rx, ry, ox, oy) = (int(0)?, int(1)?, int(2)?, int(3)?);
                [(rx << 6) | ox, (ry << 6) | oy, 0]
            }
            5 => {
                let (plane, rx, ry, ox, oy) = (int(0)?, int(1)?, int(2)?, int(3)?, int(4)?);
                [(rx << 6) | ox, (ry << 6) | oy, plane]
            }
            _ => {
                return reject(
                    host,
                    player,
                    &["Usage: regionX_regionY OR regionX_regionY_offsetX_offsetY"],
                );
            }
        };
        return host.set_teleport_location(player, Some(location));
    }
    if args.len() < 2 {
        reject::<_, ()>(
            host,
            player,
            &["Usage: x,y,(optional)z OR regionX_regionY OR regionX_regionY_offsetX_offsetY"],
        )?;
    }
    let x = to_int(arg(args, 1)?)?;
    let y = to_int(arg(args, 2)?)?;
    let z = if args.len() > 3 {
        to_int(arg(args, 3)?)?
    } else {
        0
    };
    host.set_teleport_location(player, Some([x, y, z]))
}

/// The target of teleto and teletome: the rest of the line joined with
/// underscores, online and not in the Fight Caves.
fn target<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<H::Player> {
    let name = args.get(1..).unwrap_or(&[]).join("_");
    let Some(target) = host.player_by_name(Some(&name))? else {
        return reject(host, player, &["syntax error: name"]);
    };
    if host.attribute(&target, "fc_wave")?.is_some() {
        reject::<_, ()>(
            host,
            player,
            &["You cannot teleport to a player who is in the Fight Caves."],
        )?;
    }
    Ok(target)
}

/// `::teleto <name...>`.
pub fn tele_to<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    let target = target(host, player, args)?;
    let location = host.location(&target)?;
    host.set_teleport_location(player, location)
}

/// `::teletome <name...>`.
pub fn tele_to_me<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    let target = target(host, player, args)?;
    let location = host.location(player)?;
    host.set_teleport_location(&target, location)
}

/// `::home`.
pub fn home<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<()> {
    let home = host.home_location();
    host.set_teleport_location(player, home)
}
