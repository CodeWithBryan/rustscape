//! `content.global.skill.thieving.Pickpockets` and the "pickpocket" /
//! "pick-pocket" listener of `ThievingListeners` with its
//! `pickpocketRoll`.
//!
//! Differential corpus: `fixtures/thieving` (oracle/thieving), replayed by
//! `tests/thieving.rs` over these functions with the oracle's declared
//! state. Every outside call is a [`Host`] method; the loot tables
//! themselves are `drop_tables::WeightBasedTable`'s (corpus
//! `fixtures/drop-tables`), reached through [`Host::can_roll`],
//! [`Host::exclude_clues`] and [`Host::roll`].
//!
//! Not covered: doors (`PickableDoorHandler`) and the Dig Site's two calls
//! of `pickpocketRoll`. Stalls are `super::thieving_stall` and chests
//! `super::thieving_chest`.
use super::{drop_tables::WeightedItem, gathering::random::skill_success_chance};

pub const CLASS: &str = "content.global.skill.thieving.ThievingListeners";
/// The listener's lambda.
pub const LAMBDA: &str = "content.global.skill.thieving.ThievingListeners$defineListeners$1";
pub const OPTIONS: [&str; 2] = ["pickpocket", "pick-pocket"];
/// `PICKPOCKET_ANIM` (priority HIGH) and `NPC_ANIM`.
pub const PICKPOCKET_ANIM: i32 = 881;
pub const NPC_ANIM: i32 = 422;
pub const GLOVES_OF_SILENCE: i32 = 10075;
/// `Sounds.PICK_2581`.
pub const PICK_SOUND: i32 = 2581;
/// `WeightBasedTable.SLOT_CLUE_EASY` (`Items.TOOLKIT_1`).
pub const SLOT_CLUE_EASY: i32 = 1;
/// `WeightBasedTable.CLUE_SCROLL_SLOTS` (`Items.TOOLKIT_1`,
/// `Items.ROTTEN_POTATO_5733`, `Items.GRANITE_LOBSTER_POUCH_12070`).
pub const CLUE_SCROLL_SLOTS: [i32; 3] = [SLOT_CLUE_EASY, 5733, 12070];

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// An IndexOutOfBoundsException.
    Index,
    /// A ClassCastException.
    Cast,
    /// The IllegalStateException of `animate(node, null)`.
    IllegalState,
    /// The table's roll failed (`drop_tables::RollError`).
    Roll(String),
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// A `Pickpockets` constant. `table` is the `WeightBasedTable.create`
/// arguments in order; `easy_clue` the weight of its `insertEasyClue`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pickpocket {
    pub name: &'static str,
    pub ids: &'static [i32],
    pub level: i32,
    pub low: f64,
    pub high: f64,
    pub experience: f64,
    pub stun_damage_min: i32,
    pub stun_damage_max: i32,
    pub stun_time: i32,
    pub table: &'static [WeightedItem],
    pub easy_clue: Option<f64>,
}
const fn e(id: i32, min: i32, max: i32, weight: f64, guaranteed: bool) -> WeightedItem {
    WeightedItem::new(id, min, max, weight, guaranteed)
}

pub const PICKPOCKETS: [Pickpocket; 18] = [
    Pickpocket {
        name: "MAN",
        ids: &[
            1, 2, 3, 4, 5, 6, 16, 24, 25, 170, 1086, 2683, 2684, 3224, 3915, 3226, 3227, 5924, 5923,
        ],
        level: 1,
        low: 180.0,
        high: 240.0,
        experience: 8.0,
        stun_damage_min: 1,
        stun_damage_max: 1,
        stun_time: 5,
        table: &[e(995, 3, 3, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "FARMER",
        ids: &[7, 1757, 1758],
        level: 10,
        low: 180.0,
        high: 240.0,
        experience: 14.5,
        stun_damage_min: 1,
        stun_damage_max: 1,
        stun_time: 5,
        table: &[e(995, 9, 9, 1.0, true), e(5318, 1, 1, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "MALE_HAM_MEMBER",
        ids: &[1714],
        level: 20,
        low: 117.0,
        high: 240.0,
        experience: 22.5,
        stun_damage_min: 1,
        stun_damage_max: 3,
        stun_time: 4,
        table: &[
            e(995, 1, 21, 5.5, false),
            e(590, 1, 1, 5.0, false),
            e(1511, 1, 1, 7.0, false),
            e(1627, 1, 1, 2.5, false),
            e(1625, 1, 1, 2.5, false),
            e(321, 1, 1, 7.0, false),
            e(2138, 1, 1, 3.5, false),
            e(4304, 1, 1, 0.25, false),
            e(4302, 1, 1, 0.25, false),
            e(4306, 1, 1, 0.25, false),
            e(4300, 1, 1, 0.25, false),
            e(4298, 1, 1, 0.25, false),
            e(4310, 1, 1, 1.0, false),
            e(4308, 1, 1, 1.0, false),
            e(1265, 1, 1, 5.0, false),
            e(1267, 1, 1, 5.0, false),
            e(1269, 1, 1, 2.5, false),
            e(199, 1, 1, 2.0, false),
            e(205, 1, 1, 2.0, false),
            e(213, 1, 1, 2.0, false),
            e(201, 1, 1, 1.5, false),
            e(686, 1, 1, 3.5, false),
            e(698, 1, 1, 3.5, false),
            e(689, 1, 1, 3.2, false),
            e(687, 1, 1, 3.1, false),
            e(688, 1, 1, 3.0, false),
        ],
        easy_clue: Some(1.0),
    },
    Pickpocket {
        name: "FEMALE_HAM_MEMBER",
        ids: &[1715],
        level: 15,
        low: 135.0,
        high: 240.0,
        experience: 18.5,
        stun_damage_min: 1,
        stun_damage_max: 3,
        stun_time: 4,
        table: &[
            e(995, 1, 21, 5.5, false),
            e(590, 1, 1, 5.0, false),
            e(1511, 1, 1, 7.0, false),
            e(1627, 1, 1, 2.5, false),
            e(1625, 1, 1, 2.5, false),
            e(321, 1, 1, 7.0, false),
            e(2138, 1, 1, 3.5, false),
            e(4304, 1, 1, 0.25, false),
            e(4302, 1, 1, 0.25, false),
            e(4306, 1, 1, 0.25, false),
            e(4298, 1, 1, 0.25, false),
            e(4300, 1, 1, 0.25, false),
            e(4310, 1, 1, 1.0, false),
            e(4308, 1, 1, 1.0, false),
            e(1265, 1, 1, 5.0, false),
            e(1267, 1, 1, 5.0, false),
            e(1269, 1, 1, 2.5, false),
            e(199, 1, 1, 2.0, false),
            e(205, 1, 1, 2.0, false),
            e(213, 1, 1, 2.0, false),
            e(201, 1, 1, 1.5, false),
            e(686, 1, 1, 3.5, false),
            e(698, 1, 1, 3.5, false),
            e(689, 1, 1, 3.2, false),
            e(687, 1, 1, 3.1, false),
            e(688, 1, 1, 3.0, false),
        ],
        easy_clue: Some(1.0),
    },
    Pickpocket {
        name: "WARRIOR",
        ids: &[15, 18],
        level: 25,
        low: 84.0,
        high: 240.0,
        experience: 26.0,
        stun_damage_min: 2,
        stun_damage_max: 2,
        stun_time: 5,
        table: &[e(995, 18, 18, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "ROGUE",
        ids: &[187, 2267, 2268, 2269, 8122],
        level: 32,
        low: 74.0,
        high: 240.0,
        experience: 35.5,
        stun_damage_min: 2,
        stun_damage_max: 2,
        stun_time: 5,
        table: &[
            e(995, 25, 40, 5.0, true),
            e(1993, 1, 1, 6.0, false),
            e(556, 8, 8, 8.0, false),
            e(1523, 1, 1, 5.0, false),
            e(1219, 1, 1, 1.0, false),
        ],
        easy_clue: None,
    },
    Pickpocket {
        name: "CAVE_GOBLIN",
        ids: &[
            5752, 5753, 5754, 5755, 5756, 5757, 5758, 5759, 5760, 5761, 5762, 5763, 5764, 5765,
            5766, 5767, 5768,
        ],
        level: 36,
        low: 72.0,
        high: 240.0,
        experience: 40.0,
        stun_damage_min: 1,
        stun_damage_max: 1,
        stun_time: 5,
        table: &[
            e(995, 30, 30, 6.5, false),
            e(4522, 1, 1, 0.5, false),
            e(4544, 1, 1, 0.5, false),
            e(596, 1, 1, 0.5, false),
            e(590, 1, 1, 0.5, false),
            e(1939, 1, 1, 0.5, false),
            e(441, 1, 4, 0.25, false),
        ],
        easy_clue: None,
    },
    Pickpocket {
        name: "MASTER_FARMER",
        ids: &[2234, 2235, 3299],
        level: 38,
        low: 90.0,
        high: 240.0,
        experience: 43.0,
        stun_damage_min: 3,
        stun_damage_max: 3,
        stun_time: 5,
        table: &[
            e(5318, 1, 3, 50.0, false),
            e(5319, 1, 3, 50.0, false),
            e(5324, 1, 3, 50.0, false),
            e(5322, 1, 2, 50.0, false),
            e(5320, 1, 2, 50.0, false),
            e(5323, 1, 1, 25.0, false),
            e(5321, 1, 1, 8.0, false),
            e(5305, 1, 4, 50.0, false),
            e(5307, 1, 3, 50.0, false),
            e(5308, 1, 3, 50.0, false),
            e(5306, 1, 3, 50.0, false),
            e(5309, 1, 2, 25.0, false),
            e(5310, 1, 2, 25.0, false),
            e(5311, 1, 1, 8.0, false),
            e(5096, 1, 1, 50.0, false),
            e(5098, 1, 1, 50.0, false),
            e(5097, 1, 1, 50.0, false),
            e(5099, 1, 1, 50.0, false),
            e(5100, 1, 1, 25.0, false),
            e(5101, 1, 1, 50.0, false),
            e(5102, 1, 1, 50.0, false),
            e(5103, 1, 1, 25.0, false),
            e(5104, 1, 1, 25.0, false),
            e(5105, 1, 1, 25.0, false),
            e(5291, 1, 1, 50.0, false),
            e(5292, 1, 1, 50.0, false),
            e(5293, 1, 1, 50.0, false),
            e(5294, 1, 1, 25.0, false),
            e(5295, 1, 1, 8.0, false),
            e(5296, 1, 1, 8.0, false),
            e(5297, 1, 1, 8.0, false),
            e(5298, 1, 1, 8.0, false),
            e(5299, 1, 1, 8.0, false),
            e(5300, 1, 1, 5.0, false),
            e(5301, 1, 1, 8.0, false),
            e(5302, 1, 1, 5.0, false),
            e(5303, 1, 1, 5.0, false),
            e(5304, 1, 1, 5.0, false),
        ],
        easy_clue: None,
    },
    Pickpocket {
        name: "GUARD",
        ids: &[
            9, 32, 206, 296, 297, 298, 299, 344, 345, 346, 368, 678, 812, 9, 32, 296, 297, 298,
            299, 2699, 2700, 2701, 2702, 2703, 3228, 3229, 3230, 3231, 3232, 3233, 3241, 3407,
            3408, 4307, 4308, 4309, 4310, 4311, 5919, 5920,
        ],
        level: 40,
        low: 50.0,
        high: 240.0,
        experience: 46.5,
        stun_damage_min: 2,
        stun_damage_max: 2,
        stun_time: 5,
        table: &[e(995, 30, 30, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "FREMENNIK_CITIZEN",
        ids: &[1305, 1306, 1308, 1309, 1310, 1311, 1312, 1313, 1314, 2462],
        level: 45,
        low: 65.0,
        high: 240.0,
        experience: 65.0,
        stun_damage_min: 2,
        stun_damage_max: 2,
        stun_time: 5,
        table: &[e(995, 40, 40, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "BEARDED_BANDIT",
        ids: &[1880, 1881, 6174],
        level: 45,
        low: 50.0,
        high: 240.0,
        experience: 65.0,
        stun_damage_min: 5,
        stun_damage_max: 5,
        stun_time: 5,
        table: &[
            e(2446, 1, 1, 1.0, false),
            e(1523, 1, 1, 2.0, false),
            e(995, 1, 1, 4.0, false),
        ],
        easy_clue: None,
    },
    Pickpocket {
        name: "DESERT_BANDIT",
        ids: &[1926, 1921],
        level: 53,
        low: 50.0,
        high: 240.0,
        experience: 79.5,
        stun_damage_min: 3,
        stun_damage_max: 3,
        stun_time: 5,
        table: &[
            e(995, 50, 1, 3.0, false),
            e(2446, 1, 1, 1.0, false),
            e(1523, 1, 1, 1.0, false),
        ],
        easy_clue: None,
    },
    Pickpocket {
        name: "KNIGHT_OF_ADROUGNE",
        ids: &[23, 26],
        level: 55,
        low: 50.0,
        high: 240.0,
        experience: 84.3,
        stun_damage_min: 3,
        stun_damage_max: 3,
        stun_time: 5,
        table: &[e(995, 50, 50, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "YANILLE_WATCHMAN",
        ids: &[34],
        level: 65,
        low: 50.0,
        high: 240.0,
        experience: 137.5,
        stun_damage_min: 3,
        stun_damage_max: 3,
        stun_time: 5,
        table: &[e(995, 60, 60, 1.0, true), e(2309, 1, 1, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "MENAPHITE_THUG",
        ids: &[1905],
        level: 65,
        low: 50.0,
        high: 240.0,
        experience: 137.5,
        stun_damage_min: 5,
        stun_damage_max: 5,
        stun_time: 5,
        table: &[e(995, 60, 60, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "PALADIN",
        ids: &[20, 2256],
        level: 70,
        low: 50.0,
        high: 150.0,
        experience: 151.75,
        stun_damage_min: 3,
        stun_damage_max: 3,
        stun_time: 5,
        table: &[e(995, 80, 80, 1.0, true), e(562, 2, 2, 1.0, true)],
        easy_clue: None,
    },
    Pickpocket {
        name: "GNOME",
        ids: &[
            66, 67, 68, 159, 160, 161, 168, 169, 2249, 2250, 2251, 2371, 2649, 2650, 6002, 6004,
        ],
        level: 75,
        low: 8.0,
        high: 120.0,
        experience: 198.5,
        stun_damage_min: 1,
        stun_damage_max: 1,
        stun_time: 5,
        table: &[
            e(995, 300, 300, 2.5, false),
            e(557, 1, 1, 3.5, false),
            e(445, 1, 1, 1.0, false),
            e(569, 1, 1, 5.0, false),
            e(2150, 1, 1, 8.0, false),
            e(2162, 1, 1, 9.0, false),
        ],
        easy_clue: None,
    },
    Pickpocket {
        name: "HERO",
        ids: &[21],
        level: 80,
        low: 6.0,
        high: 100.0,
        experience: 273.3,
        stun_damage_min: 6,
        stun_damage_max: 6,
        stun_time: 4,
        table: &[
            e(995, 200, 300, 1.5, false),
            e(560, 2, 2, 1.0, false),
            e(565, 1, 1, 0.5, false),
            e(569, 1, 1, 2.5, false),
            e(1601, 1, 1, 2.0, false),
            e(444, 1, 1, 1.5, false),
            e(1993, 1, 1, 3.0, false),
        ],
        easy_clue: None,
    },
];
/// `Pickpockets.GNOME`'s place.
pub const GNOME: usize = 16;

impl Pickpocket {
    /// The entries the constant's table is given, in the order it is given
    /// them: the `create` arguments, then the easy clue slot.
    pub fn entries(&self) -> Vec<WeightedItem> {
        let mut entries = self.table.to_vec();
        if let Some(weight) = self.easy_clue {
            entries.push(e(SLOT_CLUE_EASY, 1, 1, weight, false));
        }
        entries
    }
}

/// `Pickpockets.forID(id)`: `idMap` is filled in declaration order, so the
/// last constant listing an id holds it.
pub fn for_id(id: i32) -> Option<usize> {
    PICKPOCKETS.iter().rposition(|p| p.ids.contains(&id))
}

/// Every id the map holds (the ids worth declaring to a lookup).
pub fn ids() -> Vec<i32> {
    let mut ids: Vec<i32> = PICKPOCKETS
        .iter()
        .flat_map(|p| p.ids.iter().copied())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// What the lambda asks of the world, for its player and its NPC.
pub trait Host {
    /// `player.inCombat()`.
    fn in_combat(&mut self) -> Result<bool>;
    /// `player.sendMessage(message)`.
    fn message(&mut self, message: &str) -> Result<()>;
    /// `player.skills.getLevel(Skills.THIEVING)`.
    fn level(&mut self) -> Result<i32>;
    /// `PICKPOCKETS[n]`'s `table.canRoll(player)`.
    fn can_roll(&mut self, n: usize) -> Result<bool>;
    /// `player.animator.animate(PICKPOCKET_ANIM)`.
    fn animate_player(&mut self) -> Result<()>;
    /// `TreasureTrailManager.getInstance(player).hasClue()`.
    fn has_clue(&mut self) -> Result<bool>;
    /// `table.excluding(*WeightBasedTable.CLUE_SCROLL_SLOTS)` of
    /// `PICKPOCKETS[n]`: the table the roll then uses.
    fn exclude_clues(&mut self, n: usize) -> Result<()>;
    /// `SkillcapePerks.isActive(SkillcapePerks.SMOOTH_HANDS, player)`.
    fn smooth_hands(&mut self) -> Result<bool>;
    /// `player.equipment.contains(Items.GLOVES_OF_SILENCE_10075, 1)`.
    fn wears_gloves_of_silence(&mut self) -> Result<bool>;
    /// `RandomFunction.randomDouble(1.0, 100.0)`.
    fn random_double(&mut self) -> Result<f64>;
    /// `roll()` of `PICKPOCKETS[n]`'s table, or of the one
    /// [`Host::exclude_clues`] made when `excluded`: each item's id and
    /// amount.
    fn roll(&mut self, n: usize, excluded: bool) -> Result<Vec<(i32, i32)>>;
    /// `node.asNpc().face(player)`.
    fn npc_face_player(&mut self) -> Result<()>;
    /// `animate(node, node.properties.attackAnimation)`.
    fn npc_attack_animation(&mut self) -> Result<()>;
    /// `animate(node, NPC_ANIM)`.
    fn npc_animate(&mut self, id: i32) -> Result<()>;
    /// `playHurtAudio(player, delay)`.
    fn play_hurt_audio(&mut self, delay: i32) -> Result<()>;
    /// `stun(player, ticks)`.
    fn stun(&mut self, ticks: i32) -> Result<()>;
    /// `RandomFunction.random(a, b)`.
    fn random(&mut self, a: i32, b: i32) -> Result<i32>;
    /// `player.impactHandler.manualHit(node.asNpc(), hit, NORMAL)`.
    fn manual_hit(&mut self, hit: i32) -> Result<()>;
    /// `node.asNpc().face(null)`.
    fn npc_face_nothing(&mut self) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
    /// `player.lock(ticks)`.
    fn lock(&mut self, ticks: i32) -> Result<()>;
    /// `player.inventory.add(Item(id, amount))`; its answer is not read.
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()>;
    /// `player.skills.addExperience(Skills.THIEVING, xp)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
}

/// `ThievingListeners.pickpocketRoll(player, low, high, table)`, with the
/// table named as [`Host::roll`] names it: `None` is its `null` (a failed
/// attempt), a list (which may be empty) the loot.
pub fn pickpocket_roll<H: Host>(
    host: &mut H,
    low: f64,
    high: f64,
    n: usize,
    excluded: bool,
) -> Result<Option<Vec<(i32, i32)>>> {
    let mut success_mod = 0.0;
    if host.smooth_hands()? {
        success_mod += 25.0;
    }
    if host.wears_gloves_of_silence()? {
        success_mod += 3.0;
    }
    let chance = host.random_double()?;
    let fail_threshold = skill_success_chance(low, high, host.level()?) + success_mod;
    if chance > fail_threshold {
        return Ok(None);
    }
    host.roll(n, excluded).map(Some)
}

/// The lambda, for an NPC of id `npc`: its answer.
pub fn pickpocket<H: Host>(host: &mut H, npc: i32) -> Result<bool> {
    let Some(n) = for_id(npc) else {
        return Ok(false);
    };
    let data = PICKPOCKETS[n];
    if host.in_combat()? {
        host.message("You can't pickpocket while in combat.")?;
        return Ok(true);
    }
    if host.level()? < data.level {
        host.message(&format!(
            "You need a Thieving level of {} to do that.",
            data.level
        ))?;
        return Ok(true);
    }
    if !host.can_roll(n)? {
        host.message("You don't have enough inventory space to do that.")?;
        return Ok(true);
    }
    host.animate_player()?;
    let excluded = host.has_clue()?;
    if excluded {
        host.exclude_clues(n)?;
    }
    match pickpocket_roll(host, data.low, data.high, n, excluded)? {
        None => {
            host.npc_face_player()?;
            if PICKPOCKETS[GNOME].ids.contains(&npc) {
                host.npc_attack_animation()?;
            } else {
                host.npc_animate(NPC_ANIM)?;
            }
            host.play_hurt_audio(20)?;
            host.stun(data.stun_time)?;
            let hit = host.random(data.stun_damage_min, data.stun_damage_max)?;
            host.manual_hit(hit)?;
            host.npc_face_nothing()?;
        }
        Some(loot) => {
            host.play_audio(PICK_SOUND)?;
            host.lock(2)?;
            for (id, amount) in loot {
                host.add_item(id, amount)?;
            }
            host.add_experience(data.experience)?;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_pinned_enum() {
        assert_eq!(PICKPOCKETS[GNOME].name, "GNOME");
        assert_eq!(
            PICKPOCKETS.iter().map(|p| p.table.len()).sum::<usize>(),
            133
        );
        // GUARD lists 9, 32 and 296 to 299 twice.
        assert_eq!(for_id(9), Some(8));
        assert_eq!(for_id(1), Some(0));
        assert_eq!(for_id(8), None);
        assert_eq!(PICKPOCKETS[2].entries().len(), 27);
    }
}
