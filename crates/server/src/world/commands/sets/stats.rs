//! `StatsCommandSet`: the `stats` command and the book's `display`
//! callback (`StatsCommandSet.Companion.display` with `sendLine`).
use super::{Book, Host, Value};
use crate::world::commands::{Error, Result, arg, reject};

/// `BookInterface.FANCY_BOOK_26`.
pub const BOOK: i32 = 26;
/// `SPACER`.
pub const SPACER: &str = "<str>             </str>";
/// The attribute the command leaves for the callback.
pub const QUERY: &str = "stats-command-query-player";

pub const TUROTH: &[i32] = &[1622, 1623, 1626, 1627, 1628, 1629, 1630];
pub const KURASK: &[i32] = &[1608, 1609, 4229, 7811];
pub const GARGOYLE: &[i32] = &[1610, 1827, 6389];
pub const SPIRITUAL_MAGE: &[i32] = &[6221, 6231, 6257, 6278];
pub const GREEN_DRAGON: &[i32] = &[941, 4677, 4678, 4679, 4680, 5362];
pub const BLUE_DRAGON: &[i32] = &[55, 4681, 4682, 4683, 4684, 5178];
pub const RED_DRAGON: &[i32] = &[53, 4669, 4670, 4671, 4672];
pub const BLACK_DRAGON: &[i32] = &[54, 4673, 4674, 4675, 4676];
pub const BRONZE_DRAGON: &[i32] = &[1590];
pub const IRON_DRAGON: &[i32] = &[1591];
pub const STEEL_DRAGON: &[i32] = &[1592, 3590];
pub const MITHRIL_DRAGON: &[i32] = &[5363, 8424];
pub const SKELETAL_WYVERN: &[i32] = &[3068, 3069, 3070, 3071];

/// `BossKillCounter` ordinals the book reads.
mod boss {
    pub const KING_BLACK_DRAGON: usize = 0;
    pub const DAGANNOTH_SUPREME: usize = 2;
    pub const DAGANNOTH_PRIME: usize = 3;
    pub const DAGANNOTH_REX: usize = 4;
    pub const CHAOS_ELEMENTAL: usize = 5;
    pub const GIANT_MOLE: usize = 6;
    pub const SARADOMIN: usize = 7;
    pub const ZAMORAK: usize = 8;
    pub const BANDOS: usize = 9;
    pub const ARMADYL: usize = 10;
    pub const JAD: usize = 11;
    pub const KALPHITE_QUEEN: usize = 12;
    pub const CORPOREAL_BEAST: usize = 13;
    pub const TORMENTED_DEMONS: usize = 14;
}

/// `define("stats")`.
pub fn stats<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() > 2 {
        return reject(host, player, &["Usage: ::stats playername"]);
    }
    let mut query = None;
    if args.len() == 1 {
        query = Some(player.clone());
    }
    if args.len() == 2 {
        query = host.player_by_name(Some(arg(args, 1)?))?;
    }
    let Some(query) = query else {
        return reject(host, player, &["Invalid player or player not online."]);
    };
    host.entity_set_attribute(player, QUERY, Some(Value::Player(query)))?;
    host.open_book(player, BOOK, Book::Stats)
}

/// One line of the book: `sendLine(player, line, child)`.
enum Line {
    Text(&'static str),
    Clue(usize),
    Slayer,
    Quest,
    Ironman,
    Stat(&'static str, &'static str),
    Boss(&'static str, usize),
    Barrows,
    Kills(&'static str, &'static [i32]),
    Drops(&'static str, i32),
    Empty,
}

/// The line of `page` for child `i` (68..=97), as `display`'s `when`.
fn line(page: i32, i: i32) -> Option<Line> {
    use Line::*;
    Some(match page {
        0 => match i {
            97 => Clue(0),
            68 => Clue(1),
            69 => Clue(2),
            70 | 75 => Text(SPACER),
            71 => Slayer,
            72 => Quest,
            73 => Ironman,
            74 => Stat("Deaths", "deaths"),
            76 => Stat("Logs Chopped", "logs_chopped"),
            77 => Stat("Rocks Mined", "rocks_mined"),
            78 => Stat("Fish Caught", "fish_caught"),
            79 => Stat("Essence Crafted", "essence_crafted"),
            80 => Stat("Food Cooked", "food_cooked"),
            81 => Stat("Cats Raised", "cats_raised"),
            82 => Boss("KBD KC", boss::KING_BLACK_DRAGON),
            83 => Boss("TDs KC", boss::TORMENTED_DEMONS),
            84 => Boss("Supreme KC", boss::DAGANNOTH_SUPREME),
            85 => Boss("Rex KC", boss::DAGANNOTH_REX),
            86 => Boss("Prime KC", boss::DAGANNOTH_PRIME),
            87 => Barrows,
            88 => Boss("Chaos Ele", boss::CHAOS_ELEMENTAL),
            89 => Boss("Mole KC", boss::GIANT_MOLE),
            90 => Boss("Sara KC", boss::SARADOMIN),
            91 => Boss("Zammy KC", boss::ZAMORAK),
            92 => Boss("Bandos KC", boss::BANDOS),
            93 => Boss("Arma KC", boss::ARMADYL),
            94 => Boss("Jad KC", boss::JAD),
            95 => Boss("KQ KC", boss::KALPHITE_QUEEN),
            96 => Boss("Corp KC", boss::CORPOREAL_BEAST),
            _ => Empty,
        },
        1 => match i {
            97 => Kills("Turoths", TUROTH),
            68 => Kills("Kurasks", KURASK),
            69 => Drops("Leaf-bladed swords", 13290),
            70 | 73 | 76 | 79 | 86 | 92 => Text(SPACER),
            71 => Kills("Gargoyles", GARGOYLE),
            72 => Drops("Granite mauls", 4153),
            74 => Kills("Spiritual mages", SPIRITUAL_MAGE),
            75 => Drops("Dragon boots", 11732),
            77 => Kills("Abyssal demons", &[1615]),
            78 => Drops("Abyssal whips", 4151),
            80 => Kills("Dark beasts", &[2783]),
            81 => Drops("Dark bows", 11235),
            82 => Kills("Green Dragons", GREEN_DRAGON),
            83 => Kills("Blue Dragons", BLUE_DRAGON),
            84 => Kills("Red Dragons", RED_DRAGON),
            85 => Kills("Black Dragons", BLACK_DRAGON),
            87 => Kills("Bronze Dragons", BRONZE_DRAGON),
            88 => Kills("Iron Dragons", IRON_DRAGON),
            89 => Kills("Steel Dragons", STEEL_DRAGON),
            90 => Kills("Mithril Dragons", MITHRIL_DRAGON),
            91 => Kills("Skeletal Wyverns", SKELETAL_WYVERN),
            93 => Drops("Draconic visages", 11286),
            _ => Empty,
        },
        2 => match i {
            97 => Drops("Ahrim's hood", 4708),
            68 => Drops("Ahrim's staff", 4710),
            69 => Drops("Ahrim's robetop", 4712),
            70 => Drops("Ahrim's robeskirt", 4714),
            71 | 76 | 86 | 91 => Text(SPACER),
            72 => Drops("Dharok's helm", 4716),
            73 => Drops("Dharok's greataxe", 4718),
            74 => Drops("Dharok's platebody", 4720),
            75 => Drops("Dharok's platelegs", 4722),
            77 => Drops("Guthan's helm", 4724),
            78 => Drops("Guthan's warspear", 4726),
            79 => Drops("Guthan's platebody", 4728),
            80 => Drops("Guthan's chainskirt", 4730),
            82 => Drops("Karil's coif", 4732),
            83 => Drops("Karil's crossbow", 4734),
            84 => Drops("Karil's leathertop", 4736),
            85 => Drops("Karil's leatherskirt", 4738),
            87 => Drops("Torag's helm", 4745),
            88 => Drops("Torag's hammers", 4747),
            89 => Drops("Torag's platebody", 4749),
            90 => Drops("Torag's platelegs", 4751),
            92 => Drops("Verac's helm", 4753),
            93 => Drops("Verac's flail", 4755),
            94 => Drops("Verac's brassard", 4757),
            95 => Drops("Verac's plateskirt", 4759),
            _ => Empty,
        },
        _ => return None,
    })
}

fn index_of(values: &[i32], index: usize) -> Result<i32> {
    values.get(index).copied().ok_or_else(|| {
        Error::Index(format!(
            "Index {index} out of bounds for length {}",
            values.len()
        ))
    })
}

/// `StatsCommandSet.display(player, pageNum, buttonId)`.
pub fn display<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    page: i32,
    _button: i32,
) -> Result<bool> {
    let query = match host.entity_attribute(player, QUERY, None)? {
        None => return Ok(false),
        Some(Value::Player(p)) => p,
        Some(_) => return Err(Error::Cast),
    };
    let username = host.username(&query);
    let fake: Vec<super::PageSet> = vec![vec![Vec::new(), Vec::new()]; 3];
    host.page_setup(player, BOOK, &format!("{username}'s Stats"), &fake, true)?;
    for i in 68..=97 {
        let Some(kind) = line(page, i) else {
            continue;
        };
        let text = match kind {
            Line::Text(t) => t.to_owned(),
            Line::Empty => String::new(),
            Line::Clue(level) => {
                let label = ["Easy Clues", "Medium Clues", "Hard Clues"][level];
                let clues = host.completed_clues(&query)?;
                format!("{label}: {}", index_of(&clues, level)?)
            }
            Line::Slayer => format!("Slayer Tasks: {}", host.slayer_completed_tasks(&query)?),
            Line::Quest => format!("Quest Points: {}", host.quest_points(&query)?),
            Line::Ironman => format!(
                "Ironman Mode: {}",
                host.ironman_mode(&query)?.name().to_ascii_lowercase()
            ),
            Line::Stat(label, key) => {
                let value = match host.entity_attribute(
                    &query,
                    &format!("stats_manager:{key}"),
                    Some(Value::Int(0)),
                )? {
                    // The template reads the generic T as Int.
                    Some(Value::Int(v)) => v.to_string(),
                    None => return Err(Error::Null),
                    Some(_) => return Err(Error::Cast),
                };
                format!("{label}: {value}")
            }
            Line::Boss(label, ordinal) => {
                let counters = host.boss_counters(&query);
                format!("{label}: {}", index_of(&counters, ordinal)?)
            }
            Line::Barrows => format!("Barrows KC: {}", host.barrows_loots(&query)),
            Line::Kills(label, npcs) => {
                format!("{label}: {}", host.stat_kills(&query, npcs)?)
            }
            Line::Drops(label, item) => {
                format!("{label}: {}", host.stat_rare_drops(&query, item)?)
            }
        };
        host.send_string(player, &text, BOOK, i)?;
    }
    Ok(true)
}
