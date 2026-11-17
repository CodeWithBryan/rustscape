//! WoodcuttingNode, MiningNode, SkillingResource and their lookups.
//!
//! Rows come from `tables.rs` (declaration order). The constructor switches
//! below are ported by hand, including what they leave at field defaults.
use super::tables;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq)]
pub struct WoodcuttingNode {
    pub name: &'static str,
    pub ordinal: usize,
    pub full: i32,
    pub empty: i32,
    pub identifier: i8,
    pub farming: bool,
    pub reward: i32,
    pub respawn_rate: i32,
    pub level: i32,
    pub reward_amount: i32,
    pub experience: f64,
    pub rate: f64,
    pub base_low: f64,
    pub base_high: f64,
    pub tier_mod_low: f64,
    pub tier_mod_high: f64,
}
#[allow(clippy::too_many_arguments)]
impl WoodcuttingNode {
    fn set(&mut self, reward: i32, min: i32, max: i32, rate: f64, experience: f64, level: i32) {
        self.reward = reward;
        self.respawn_rate = min | max << 16;
        self.rate = rate;
        self.experience = experience;
        self.level = level;
    }
    fn chance(&mut self, base_low: f64, base_high: f64, tier_mod_low: f64, tier_mod_high: f64) {
        self.base_low = base_low;
        self.base_high = base_high;
        self.tier_mod_low = tier_mod_low;
        self.tier_mod_high = tier_mod_high;
    }
    fn new(ordinal: usize, row: &(&'static str, i32, i32, i8, Option<bool>)) -> Self {
        let &(name, full, empty, identifier, farming) = row;
        let mut n = Self {
            name,
            ordinal,
            full,
            empty,
            identifier,
            farming: farming.unwrap_or(false),
            reward: 0,
            respawn_rate: 0,
            level: 0,
            // Only the three-argument constructor starts the amount at one.
            reward_amount: i32::from(farming.is_none()),
            experience: 0.0,
            rate: 0.0,
            base_low: 2.0,
            base_high: 6.0,
            tier_mod_low: 1.0,
            tier_mod_high: 3.0,
        };
        let key = i32::from(identifier) & 0xff;
        if farming.is_none() {
            match key {
                1..=4 => {
                    n.set(1511, 50, 100, 0.05, 25.0, 1);
                    n.chance(64.0, 200.0, 32.0, 100.0);
                }
                5 => {
                    n.set(1511, 50, 100, 0.15, 100.0, 1);
                    n.reward_amount = 2;
                    n.chance(64.0, 200.0, 32.0, 100.0);
                }
                6 => {
                    n.set(2862, 50, 100, 0.05, 25.0, 1);
                    n.chance(64.0, 200.0, 32.0, 100.0);
                }
                7 => {
                    n.set(1521, 14, 22, 0.15, 37.5, 15);
                    n.reward_amount = 10;
                    n.chance(32.0, 100.0, 16.0, 50.0);
                }
                8 => {
                    n.set(1519, 14, 22, 0.3, 67.8, 30);
                    n.reward_amount = 20;
                    n.chance(16.0, 50.0, 8.0, 25.0);
                }
                9 => {
                    n.set(6333, 35, 60, 0.7, 85.0, 35);
                    n.reward_amount = 25;
                    n.chance(15.0, 46.0, 8.0, 23.5);
                }
                10 => {
                    n.set(1517, 58, 100, 0.65, 100.0, 45);
                    n.reward_amount = 30;
                    n.chance(8.0, 25.0, 4.0, 12.5);
                }
                11 => {
                    n.set(3239, 58, 100, 0.6, 82.5, 45);
                    n.reward_amount = 30;
                    n.chance(18.0, 26.0, 10.0, 14.0);
                }
                12 => {
                    n.set(6332, 62, 115, 0.7, 125.0, 50);
                    n.reward_amount = 35;
                    n.chance(8.0, 25.0, 4.0, 12.5);
                }
                13 => {
                    n.set(10810, 75, 130, 0.73, 40.0, 54);
                    n.reward_amount = 35;
                    n.chance(6.0, 30.0, 3.0, 13.5);
                }
                14 => {
                    n.set(12581, 80, 140, 0.77, 165.0, 58);
                    n.reward_amount = 35;
                }
                15 => {
                    n.set(1515, 100, 162, 0.8, 175.0, 60);
                    n.reward_amount = 40;
                    n.chance(4.0, 12.5, 2.0, 6.25);
                }
                16 => {
                    n.set(1513, 200, 317, 0.9, 250.0, 75);
                    n.reward_amount = 50;
                    n.chance(2.0, 6.0, 1.0, 3.0);
                }
                17 => {
                    n.set(1513, 200, 317, 0.95, 275.0, 82);
                    n.reward_amount = 50;
                }
                18 => {
                    n.set(771, 0, 0, 0.05, 25.0, 36);
                    n.respawn_rate = -1;
                    n.reward_amount = i32::MAX;
                    n.chance(255.0, 255.0, 0.0, 0.0);
                }
                30 => {
                    n.set(3692, 0, 0, 0.05, 1.0, 40);
                    n.respawn_rate = -1;
                    n.reward_amount = i32::MAX;
                }
                _ => {}
            }
        } else {
            match key {
                19 => {
                    n.set(1521, 14, 22, 0.15, 37.5, 15);
                    n.reward_amount = 10;
                    n.chance(32.0, 100.0, 16.0, 50.0);
                }
                20 => {
                    n.set(1519, 14, 22, 0.3, 67.8, 30);
                    n.reward_amount = 20;
                    n.chance(16.0, 50.0, 8.0, 25.0);
                }
                21 => {
                    n.set(1517, 58, 100, 0.65, 100.0, 45);
                    n.reward_amount = 30;
                    n.chance(8.0, 25.0, 4.0, 12.5);
                }
                22 => {
                    n.set(1515, 100, 162, 0.8, 175.0, 60);
                    n.reward_amount = 40;
                    n.chance(4.0, 12.5, 2.0, 6.25);
                }
                23 => {
                    n.set(1513, 200, 317, 0.9, 250.0, 75);
                    n.reward_amount = 50;
                    n.chance(2.0, 6.0, 1.0, 3.0);
                }
                _ => {}
            }
        }
        n
    }
    /// `WoodcuttingNode.values()`.
    pub fn values() -> &'static [WoodcuttingNode] {
        static ROWS: OnceLock<Vec<WoodcuttingNode>> = OnceLock::new();
        ROWS.get_or_init(|| {
            tables::WOODCUTTING_ROWS
                .iter()
                .enumerate()
                .map(|(i, r)| Self::new(i, r))
                .collect()
        })
    }
    /// `NODE_MAP.putIfAbsent`: the first declared row owns a repeated id.
    pub fn for_id(id: i32) -> Option<&'static WoodcuttingNode> {
        Self::values().iter().find(|n| n.full == id)
    }
    /// `EMPTY_MAP.get(id) != null`.
    pub fn is_empty(id: i32) -> bool {
        Self::values().iter().any(|n| n.empty == id)
    }
    pub fn minimum_respawn(&self) -> i32 {
        self.respawn_rate & 0xffff
    }
    pub fn maximum_respawn(&self) -> i32 {
        (self.respawn_rate >> 16) & 0xffff
    }
    pub fn respawn_duration(&self, max_players: i32, players: i32) -> i32 {
        respawn_duration(self.respawn_rate, max_players, players)
    }
}

/// Shared `getRespawnDuration` arithmetic. `players` is
/// `Repository.getPlayers().size()` and `max_players` is
/// `ServerConstants.MAX_PLAYERS`; a zero population divides by zero as a double.
pub fn respawn_duration(respawn_rate: i32, max_players: i32, players: i32) -> i32 {
    let minimum = respawn_rate & 0xffff;
    let maximum = (respawn_rate >> 16) & 0xffff;
    let player_ratio = f64::from(max_players) / f64::from(players);
    (f64::from(minimum) + (f64::from(maximum - minimum) / player_ratio)) as i32
}

#[derive(Clone, Debug, PartialEq)]
pub struct MiningNode {
    pub name: &'static str,
    pub ordinal: usize,
    pub full: i32,
    pub empty: i32,
    pub identifier: i8,
    pub respawn_rate: i32,
    pub reward: i32,
    pub level: i32,
    pub rate: f64,
    pub experience: f64,
}
impl MiningNode {
    fn new(ordinal: usize, row: &(&'static str, i32, i32, i8)) -> Self {
        let &(name, full, empty, identifier) = row;
        let (min, max, experience, rate, reward, level) = match i32::from(identifier) & 0xff {
            1 | 2 => (4, 8, 17.5, 0.05, if identifier == 1 { 436 } else { 438 }, 1),
            3 => (1, 1, 5.0, 0.1, 434, 1),
            4 => (10, 20, 26.5, 0.2, 3211, 10),
            5 => (10, 20, 17.5, 0.2, 668, 10),
            6 => (15, 25, 35.0, 0.2, 440, 15),
            7 => (100, 200, 40.0, 0.3, 442, 20),
            8 => (50, 100, 50.0, 0.4, 453, 30),
            9 => (100, 200, 65.0, 0.6, 444, 40),
            10 => (200, 400, 80.0, 0.70, 447, 55),
            11 => (400, 800, 95.0, 0.85, 449, 70),
            12 => (1250, 2500, 125.0, 0.95, 451, 85),
            13 => (166, 175, 65.0, 0.95, 1625, 40),
            14 => (1, 1, 5.0, 0.1, 1436, 1),
            15 => (30, 60, 30.0, 0.2, 6971, 35),
            16 => (10, 20, 50.0, 0.2, 6979, 45),
            17 => (50, 100, 17.5, 0.6, 12630, 46),
            18 => (100, 200, 0.0, 0.3, 4703, 20),
            _ => (0, 0, 0.0, 0.0, 0, 0),
        };
        Self {
            name,
            ordinal,
            full,
            empty,
            identifier,
            respawn_rate: min | max << 16,
            reward,
            level,
            rate,
            experience,
        }
    }
    /// `MiningNode.values()`.
    pub fn values() -> &'static [MiningNode] {
        static ROWS: OnceLock<Vec<MiningNode>> = OnceLock::new();
        ROWS.get_or_init(|| {
            tables::MINING_ROWS
                .iter()
                .enumerate()
                .map(|(i, r)| Self::new(i, r))
                .collect()
        })
    }
    /// `NODE_MAP.putIfAbsent`: the first declared row owns a repeated id.
    pub fn for_id(id: i32) -> Option<&'static MiningNode> {
        Self::values().iter().find(|n| n.full == id)
    }
    pub fn is_empty(id: i32) -> bool {
        Self::values().iter().any(|n| n.empty == id)
    }
    pub fn by_name(name: &str) -> Option<&'static MiningNode> {
        Self::values().iter().find(|n| n.name == name)
    }
    /// `getRewardAmount()` is the constant one.
    pub fn reward_amount(&self) -> i32 {
        1
    }
    pub fn minimum_respawn(&self) -> i32 {
        self.respawn_rate & 0xffff
    }
    pub fn maximum_respawn(&self) -> i32 {
        (self.respawn_rate >> 16) & 0xffff
    }
    pub fn respawn_duration(&self, max_players: i32, players: i32) -> i32 {
        respawn_duration(self.respawn_rate, max_players, players)
    }
}

/// `MiningNode.gemRockGems` in declaration order: (id, minimum, maximum, weight).
/// The original list is shared mutable state: every roll shuffles it in place.
pub const GEM_ROCK_GEMS: [(i32, i32, i32, i32); 7] = [
    (1625, 1, 1, 60),
    (1627, 1, 1, 30),
    (1629, 1, 1, 15),
    (1623, 1, 1, 9),
    (1621, 1, 1, 5),
    (1619, 1, 1, 5),
    (1617, 1, 1, 4),
];

#[derive(Clone, Debug, PartialEq)]
pub struct SkillingResource {
    pub name: &'static str,
    pub ordinal: usize,
    pub id: i32,
    pub level: i32,
    pub rate: f64,
    pub respawn_rate: i32,
    pub experience: f64,
    pub reward: i32,
    pub reward_amount: i32,
    pub label: &'static str,
    pub empty_id: i32,
    pub skill_id: i32,
    pub farming: bool,
}
impl SkillingResource {
    /// `SkillingResource.values()`. Every pinned row has a null animation.
    pub fn values() -> &'static [SkillingResource] {
        static ROWS: OnceLock<Vec<SkillingResource>> = OnceLock::new();
        ROWS.get_or_init(|| {
            tables::RESOURCE_ROWS
                .iter()
                .enumerate()
                .map(|(ordinal, r)| SkillingResource {
                    name: r.0,
                    ordinal,
                    id: r.1,
                    level: r.2,
                    rate: r.3,
                    respawn_rate: r.4,
                    experience: r.5,
                    reward: r.6,
                    reward_amount: r.7,
                    label: r.8,
                    empty_id: r.9,
                    skill_id: r.10,
                    farming: r.11,
                })
                .collect()
        })
    }
    /// `RESOURCES.put`: unlike the node enums, the last declared row owns a repeated id.
    pub fn for_id(id: i32) -> Option<&'static SkillingResource> {
        Self::values().iter().rev().find(|r| r.id == id)
    }
    pub fn is_empty(id: i32) -> bool {
        Self::values().iter().any(|r| r.empty_id == id)
    }
    pub fn minimum_respawn(&self) -> i32 {
        self.respawn_rate & 0xffff
    }
    pub fn maximum_respawn(&self) -> i32 {
        (self.respawn_rate >> 16) & 0xffff
    }
    pub fn respawn_duration(&self, max_players: i32, players: i32) -> i32 {
        respawn_duration(self.respawn_rate, max_players, players)
    }
}
