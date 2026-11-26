//! Original Skills state transitions, with explicit entity/world/content bindings.
//! Context implementations must belong to the same entity for every operation.
use serde_json::{Map, Value};
use std::sync::OnceLock;
pub const NUM_SKILLS: usize = 24;
pub const HITPOINTS: usize = 3;
pub const PRAYER: usize = 5;
pub const SUMMONING: usize = 23;
pub const NAMES: [&str; 24] = [
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Index,
    Null,
    Type,
    Number,
    Binding,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityKind {
    Player,
    Npc,
    Other,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Combat(i32),
    SummoningCombat(i32),
    Packet(usize),
    DynamicLevel {
        skill: i32,
        previous: i32,
        level: i32,
    },
    XpEvent {
        skill: i32,
        amount: f64,
    },
    XpPlugins {
        skill: i32,
        amount: f64,
    },
    Appearance,
    LevelUp {
        skill: i32,
        amount: i32,
    },
    FlashingIcons(i32),
    News(String),
    XpLog(Vec<(i32, f64)>),
    ConsumeGloves {
        id: i32,
        charges: i32,
    },
}
/// Actual world/assist/equipment/perk/plugin/packet implementations bind here.
/// Effects receive mutable state so synchronous content callbacks retain their
/// original opportunity to change skills before the next source operation.
pub trait Context {
    fn kind(&self) -> EntityKind;
    fn version(&self) -> i32;
    fn tick(&mut self) -> i32;
    fn world_tick(&mut self) -> i32;
    fn combat_level(&self) -> i32;
    fn summoning_combat_level(&self) -> i32;
    fn npc_combat_level(&mut self) -> Result<i32>;
    fn damage_spong_active(&mut self) -> Result<bool>;
    fn assisted_level(&mut self, skill: i32) -> Result<Option<i32>>;
    fn translate_experience(
        &mut self,
        state: &mut Skills,
        skill: i32,
        amount: f64,
        multiplier: f64,
    ) -> Result<bool>;
    fn brawling_present(&mut self) -> Result<bool>;
    fn brawling_for_skill(&mut self, skill: i32) -> Result<Option<i32>>;
    fn equipment_contains(&mut self, id: i32) -> Result<bool>;
    fn brawling_bonus(&mut self) -> Result<f64>;
    fn tutorial_complete(&mut self) -> Result<bool>;
    fn artificial(&mut self) -> Result<bool>;
    fn username(&mut self) -> Result<String>;
    fn permadeath_present(&self) -> bool;
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> Result<()>;
}
#[derive(Clone, Debug)]
pub struct Skills {
    pub experience_multiplier: f64,
    pub experience: [f64; 24],
    pub static_levels: [i32; 24],
    pub dynamic_levels: [i32; 24],
    pub prayer_points: f64,
    pub lifepoints: i32,
    pub lifepoints_increase: i32,
    pub experience_gained: f64,
    pub lifepoints_update: bool,
    pub combat_milestone: i32,
    pub skill_milestone: i32,
    pub last_trained_skill: i32,
    pub last_xp_gain: i32,
    pub last_update_xp: Option<[f64; 24]>,
    pub last_update: i32,
}
fn index(slot: i32) -> Result<usize> {
    let slot = slot as usize;
    if slot < 24 {
        Ok(slot)
    } else {
        Err(Error::Index)
    }
}
impl Skills {
    pub fn new(context: &mut impl Context) -> Result<Self> {
        let last_update = context.tick();
        let mut state = Self {
            experience_multiplier: 1.0,
            experience: [0.0; 24],
            static_levels: [1; 24],
            dynamic_levels: [1; 24],
            prayer_points: 1.0,
            lifepoints: 10,
            lifepoints_increase: 0,
            experience_gained: 0.0,
            lifepoints_update: false,
            combat_milestone: 0,
            skill_milestone: 0,
            last_trained_skill: -1,
            last_xp_gain: 0,
            last_update_xp: None,
            last_update,
        };
        state.experience[HITPOINTS] = 1154.0;
        state.static_levels[HITPOINTS] = 10;
        state.dynamic_levels[HITPOINTS] = 10;
        context.effect(&mut state, Effect::Combat(3))?;
        Ok(state)
    }
    pub fn is_combat(skill: i32) -> bool {
        (0..=6).contains(&skill) || skill == 23
    }
    /// Skills.pulse has no effects in the pinned implementation.
    pub fn pulse(&self) {}
    pub fn copy_from(&mut self, other: &Self) {
        self.static_levels = other.static_levels;
        self.dynamic_levels = other.dynamic_levels;
        self.experience = other.experience;
        self.prayer_points = other.prayer_points;
        self.lifepoints = other.lifepoints;
        self.lifepoints_increase = other.lifepoints_increase;
        self.experience_gained = other.experience_gained;
    }
    pub fn highest_combat_skill(&self) -> i32 {
        let (mut id, mut last) = (0, 0);
        for i in 0..5 {
            if self.static_levels[i] > last {
                last = self.static_levels[i];
                id = i as i32;
            }
        }
        id
    }
    pub fn level_from_xp(exp: f64) -> i32 {
        if exp.is_nan() {
            return 99;
        }
        let thresholds = &curve()[2..101];
        (thresholds.partition_point(|&threshold| exp > f64::from(threshold - 1)) + 1).min(99) as i32
    }
    pub fn experience_by_level(level: i32) -> i32 {
        if level <= 0 {
            return 0;
        }
        curve()[(level as usize).min(curve().len() - 1)]
    }
    pub fn static_level_by_experience(&self, slot: i32) -> Result<i32> {
        Ok(Self::level_from_xp(self.experience[index(slot)?]))
    }
    pub fn total_level(&self) -> i32 {
        self.static_levels
            .iter()
            .fold(0, |sum, &level| sum.wrapping_add(level))
    }
    pub fn total_xp(&self) -> i32 {
        self.experience
            .iter()
            .fold(0, |sum, &xp| (f64::from(sum) + xp) as i32)
    }
    pub fn mastered_skills(&self) -> i32 {
        self.static_levels
            .iter()
            .filter(|&&level| level >= 99)
            .count() as i32
    }
    pub fn skill_by_name(name: Option<&str>) -> i32 {
        name.and_then(|name| {
            NAMES
                .iter()
                .position(|candidate| java_ascii_name_equal(candidate, name))
        })
        .map_or(-1, |id| id as i32)
    }
    pub fn update_combat_level(&mut self, context: &mut impl Context) -> Result<bool> {
        let level = if context.kind() == EntityKind::Npc {
            context.npc_combat_level()?
        } else {
            let base = 0.25
                * (f64::from(self.static_levels[1].wrapping_add(self.static_levels[3]))
                    + (0.5 * f64::from(self.static_levels[5])).floor());
            let melee =
                0.325 * f64::from(self.static_levels[0].wrapping_add(self.static_levels[2]));
            let ranged = 0.325 * ((f64::from(self.static_levels[4]) / 2.0).floor() * 3.0);
            let magic = 0.325 * ((f64::from(self.static_levels[6]) / 2.0).floor() * 3.0);
            (base + melee.max(ranged.max(magic))) as i32
        };
        let mut update = level != context.combat_level();
        if context.kind() == EntityKind::Player {
            let summon = self.static_levels[23] / 8;
            if summon != context.summoning_combat_level() {
                context.effect(self, Effect::SummoningCombat(summon))?;
                update = true;
            }
        }
        context.effect(self, Effect::Combat(level))?;
        Ok(update)
    }
    pub fn set_level(&mut self, slot: i32, level: i32, context: &mut impl Context) -> Result<()> {
        if slot == HITPOINTS as i32 {
            self.lifepoints = level;
        } else if slot == PRAYER as i32 {
            self.prayer_points = f64::from(level);
        }
        let slot_index = index(slot)?;
        let previous = self.dynamic_levels[slot_index];
        self.dynamic_levels[slot_index] = level;
        if context.kind() == EntityKind::Player {
            context.effect(self, Effect::Packet(slot_index))?;
            context.effect(
                self,
                Effect::DynamicLevel {
                    skill: slot,
                    previous,
                    level,
                },
            )?;
        }
        Ok(())
    }
    pub fn level(
        &self,
        slot: i32,
        discard_assist: bool,
        context: &mut impl Context,
    ) -> Result<i32> {
        if !discard_assist
            && context.kind() == EntityKind::Player
            && let Some(level) = context.assisted_level(slot)?
        {
            let own = self.dynamic_levels[index(slot)?];
            if level > own {
                return Ok(level);
            }
        }
        Ok(self.dynamic_levels[index(slot)?])
    }
    pub fn set_static_level(
        &mut self,
        slot: i32,
        level: i32,
        context: &mut impl Context,
    ) -> Result<()> {
        let i = index(slot)?;
        self.dynamic_levels[i] = level;
        self.static_levels[i] = level;
        self.experience[i] = f64::from(Self::experience_by_level(level));
        self.packet(i, context)
    }
    pub fn set_lifepoints(&mut self, value: i32) {
        self.lifepoints = value.max(0);
        self.lifepoints_update = true;
    }
    pub fn maximum_lifepoints(&mut self, context: &mut impl Context) -> Result<i32> {
        self.lifepoints_increase =
            if context.kind() == EntityKind::Player && context.damage_spong_active()? {
                11
            } else {
                0
            };
        Ok(self.static_levels[HITPOINTS].wrapping_add(self.lifepoints_increase))
    }
    pub fn heal(&mut self, health: i32, context: &mut impl Context) -> Result<i32> {
        self.lifepoints = self.lifepoints.wrapping_add(health);
        let mut left = 0;
        if self.lifepoints > self.maximum_lifepoints(context)? {
            left = self
                .lifepoints
                .wrapping_sub(self.maximum_lifepoints(context)?);
            self.lifepoints = self.maximum_lifepoints(context)?;
        }
        self.lifepoints_update = true;
        Ok(left)
    }
    pub fn heal_no_restrictions(&mut self, amount: i32) {
        self.lifepoints = self.lifepoints.wrapping_add(amount);
        self.lifepoints_update = true;
    }
    pub fn hit(&mut self, damage: i32) -> i32 {
        self.lifepoints = self.lifepoints.wrapping_sub(damage);
        let mut left = 0;
        if self.lifepoints < 0 {
            left = self.lifepoints.wrapping_neg();
            self.lifepoints = 0;
        }
        self.lifepoints_update = true;
        left
    }
    fn packet(&mut self, slot: usize, context: &mut impl Context) -> Result<()> {
        if context.kind() == EntityKind::Player {
            context.effect(self, Effect::Packet(slot))?;
        }
        Ok(())
    }
    pub fn recharge_prayer(&mut self, context: &mut impl Context) -> Result<()> {
        self.prayer_points = f64::from(self.static_levels[PRAYER]);
        self.packet(PRAYER, context)
    }
    pub fn set_prayer(&mut self, value: f64, context: &mut impl Context) -> Result<()> {
        self.prayer_points = value;
        self.packet(PRAYER, context)
    }
    pub fn decrement_prayer(&mut self, amount: f64, context: &mut impl Context) -> Result<()> {
        self.prayer_points -= amount;
        if self.prayer_points < 0.0 {
            self.prayer_points = 0.0;
        }
        self.packet(PRAYER, context)
    }
    pub fn increment_prayer(&mut self, amount: f64, context: &mut impl Context) -> Result<()> {
        self.prayer_points += amount;
        if self.prayer_points < 0.0 {
            self.prayer_points = 0.0;
        }
        if self.prayer_points > f64::from(self.static_levels[PRAYER]) {
            self.prayer_points = f64::from(self.static_levels[PRAYER]);
        }
        self.packet(PRAYER, context)
    }
    pub fn update_level(
        &mut self,
        skill: i32,
        amount: i32,
        maximum: i32,
        context: &mut impl Context,
    ) -> Result<i32> {
        let slot = index(skill)?;
        if amount > 0 && self.dynamic_levels[slot] > maximum {
            return Ok(amount.wrapping_neg());
        }
        let left = self.dynamic_levels[slot]
            .wrapping_add(amount)
            .wrapping_sub(maximum);
        let level = self.dynamic_levels[slot].wrapping_add(amount);
        self.dynamic_levels[slot] = level;
        if level < 0 {
            self.dynamic_levels[slot] = 0;
        } else if (amount < 0 && level < maximum) || (amount > 0 && level > maximum) {
            self.dynamic_levels[slot] = maximum;
        }
        self.packet(slot, context)?;
        Ok(left)
    }
    pub fn update_level_default(
        &mut self,
        skill: i32,
        amount: i32,
        context: &mut impl Context,
    ) -> Result<i32> {
        let level = self.static_levels[index(skill)?];
        let maximum = if amount >= 0 {
            level.wrapping_add(amount)
        } else {
            level.wrapping_sub(amount)
        };
        self.update_level(skill, amount, maximum, context)
    }
    pub fn drain_level(
        &mut self,
        skill: i32,
        drain: f64,
        maximum: f64,
        context: &mut impl Context,
    ) -> Result<()> {
        let slot = index(skill)?;
        let amount = (f64::from(self.dynamic_levels[slot]) * drain) as i32;
        let minimum = (f64::from(self.static_levels[slot]) * (1.0 - maximum)) as i32;
        self.update_level(skill, amount.wrapping_neg(), minimum, context)?;
        Ok(())
    }
    pub fn restore(&mut self, context: &mut impl Context) -> Result<()> {
        for i in 0..24 {
            self.set_level(i as i32, self.static_levels[i], context)?;
        }
        self.recharge_prayer(context)
    }
    pub fn refresh(&mut self, context: &mut impl Context) -> Result<()> {
        if context.kind() != EntityKind::Player {
            return Ok(());
        }
        for i in 0..24 {
            context.effect(self, Effect::Packet(i))?;
        }
        context.effect(self, Effect::FlashingIcons(-1))
    }
    pub fn correct(&mut self, divisor: f64, context: &mut impl Context) -> Result<()> {
        for i in 0..24 {
            self.experience[i] /= divisor;
            self.static_levels[i] = Self::level_from_xp(self.experience[i]);
            self.dynamic_levels[i] = self.static_levels[i];
            if i == PRAYER {
                self.set_prayer(f64::from(self.static_levels[i]), context)?;
            }
            if i == HITPOINTS {
                self.set_lifepoints(self.static_levels[i]);
            }
        }
        self.experience_multiplier = 5.0;
        self.update_combat_level(context)?;
        Ok(())
    }
    pub fn add_experience(
        &mut self,
        slot: i32,
        experience: f64,
        _player_mod: bool,
        context: &mut impl Context,
    ) -> Result<()> {
        if self.last_update_xp.is_none() {
            self.last_update_xp = Some(self.experience);
        }
        let multiplier = self.experience_multiplier;
        if context.translate_experience(self, slot, experience, multiplier)? {
            return Ok(());
        }
        let i = index(slot)?;
        let already_200m = self.experience[i] == 200000000.0;
        let mut added = experience * multiplier;
        if context.brawling_present()? {
            let mut gloves = if context.brawling_for_skill(slot)?.is_some() {
                context.brawling_for_skill(slot)?
            } else {
                None
            };
            if gloves.is_none() && (slot == 1 || slot == 2) {
                gloves = Some(context.brawling_for_skill(0)?.ok_or(Error::Null)?);
            }
            if let Some(gloves) = gloves
                && context.equipment_contains(gloves)?
            {
                added += added * context.brawling_bonus()?;
                context.effect(
                    self,
                    Effect::ConsumeGloves {
                        id: gloves,
                        charges: 1,
                    },
                )?;
            }
        }
        if (context.equipment_contains(13660)? || context.equipment_contains(13659)?) && slot == 11
        {
            let mut count = 0;
            if context.equipment_contains(13660)? {
                count += 1;
            }
            if context.equipment_contains(13659)? {
                count += 1;
            }
            added += if count == 2 {
                0.05 * added
            } else {
                0.02 * added
            };
        }
        self.experience[i] += added;
        if self.experience[i] >= 200000000.0 {
            if !already_200m && !context.artificial()? {
                let name = context.username()?;
                context.effect(
                    self,
                    Effect::News(format!(
                        "{name} has just reached 200m experience in {}!",
                        NAMES[i]
                    )),
                )?;
            }
            self.experience[i] = 200000000.0;
        }
        if context.kind() == EntityKind::Player
            && self.experience[i] > 175.0
            && !context.tutorial_complete()?
            && i != HITPOINTS
        {
            self.experience[i] = 175.0;
        }
        self.experience_gained += added;
        context.effect(
            self,
            Effect::XpPlugins {
                skill: slot,
                amount: added,
            },
        )?;
        let new_level = Self::level_from_xp(self.experience[i]);
        if new_level > self.static_levels[i] {
            let amount = new_level.wrapping_sub(self.static_levels[i]);
            if self.dynamic_levels[i] < new_level {
                self.dynamic_levels[i] = self.dynamic_levels[i].wrapping_add(amount);
            }
            if i == HITPOINTS {
                self.lifepoints = self.lifepoints.wrapping_add(amount);
            }
            self.static_levels[i] = new_level;
            if context.kind() == EntityKind::Player {
                context.effect(self, Effect::Appearance)?;
                context.effect(
                    self,
                    Effect::LevelUp {
                        skill: slot,
                        amount,
                    },
                )?;
                self.update_combat_level(context)?;
            }
        }
        if context.kind() == EntityKind::Player {
            context.effect(self, Effect::Packet(i))?;
            context.effect(
                self,
                Effect::XpEvent {
                    skill: slot,
                    amount: added,
                },
            )?;
        }
        if context.tick().wrapping_sub(self.last_update) >= 200 {
            let previous = self.last_update_xp.as_ref().ok_or(Error::Null)?;
            let diffs = self
                .experience
                .iter()
                .zip(previous)
                .enumerate()
                .filter_map(|(i, (&xp, &old))| {
                    let diff = xp - old;
                    (diff != 0.0).then_some((i as i32, diff))
                })
                .collect();
            context.effect(self, Effect::XpLog(diffs))?;
            self.last_update_xp = Some(self.experience);
            self.last_update = context.tick();
        }
        self.last_trained_skill = slot;
        self.last_xp_gain = context.world_tick();
        Ok(())
    }
    pub fn parse(&mut self, rows: &Value, context: &mut impl Context) -> Result<()> {
        let rows = array(rows)?;
        for (row, value) in rows.iter().enumerate() {
            let data = object(value)?;
            let slot = int(required(data, "id")?)?;
            let dynamic = int(required(data, "dynamic")?)?;
            let slot = index(slot)?;
            self.dynamic_levels[slot] = dynamic;
            self.static_levels[slot] = int(required(data, "static")?)?;
            self.experience[slot] = double(required(data, "experience")?)?;
            let version = context.version();
            if row == HITPOINTS {
                if version < 3 && !data.contains_key("lifepoints") {
                    self.lifepoints = self.dynamic_levels[slot];
                    self.dynamic_levels[slot] = self.static_levels[slot];
                } else {
                    self.lifepoints = int(required(data, "lifepoints")?)?;
                }
            }
            if row == PRAYER {
                if version < 3 && !data.contains_key("prayerPoints") {
                    self.prayer_points = f64::from(self.dynamic_levels[slot]);
                    self.dynamic_levels[slot] = self.static_levels[slot];
                } else {
                    self.prayer_points = double(required(data, "prayerPoints")?)?;
                }
            }
        }
        Ok(())
    }
    pub fn parse_profile(&mut self, root: &Value, context: &mut impl Context) -> Result<()> {
        if root.is_null() {
            return Ok(());
        }
        let root = object(root)?;
        self.parse(required(root, "skills")?, context)?;
        self.experience_gained = nullable_double(root.get("totalEXP"))?;
        self.experience_multiplier = nullable_double(root.get("exp_multiplier"))?;
        if self.experience_multiplier >= 10.0 && !context.permadeath_present() {
            self.correct(self.experience_multiplier / 5.0, context)?;
        }
        if let Some(milestone) = root.get("milestone") {
            let milestone = object(milestone)?;
            self.combat_milestone = nullable_int(milestone.get("combatMilestone"))?;
            self.skill_milestone = nullable_int(milestone.get("skillMilestone"))?;
        }
        Ok(())
    }
    pub fn save_profile(&self, root: &mut Map<String, Value>) {
        let rows = (0..24)
            .map(|i| {
                let mut row = Map::new();
                row.insert("id".into(), Value::String(i.to_string()));
                row.insert(
                    "static".into(),
                    Value::String(self.static_levels[i].to_string()),
                );
                row.insert(
                    "dynamic".into(),
                    Value::String(self.dynamic_levels[i].to_string()),
                );
                row.insert(
                    "experience".into(),
                    Value::String(rs09_game_core::java_double::format(self.experience[i])),
                );
                if i == HITPOINTS {
                    row.insert(
                        "lifepoints".into(),
                        Value::String(self.lifepoints.to_string()),
                    );
                }
                if i == PRAYER {
                    row.insert(
                        "prayerPoints".into(),
                        Value::String(rs09_game_core::java_double::format(self.prayer_points)),
                    );
                }
                Value::Object(row)
            })
            .collect();
        root.insert("skills".into(), Value::Array(rows));
        root.insert(
            "totalEXP".into(),
            Value::String(rs09_game_core::java_double::format(self.experience_gained)),
        );
        root.insert(
            "exp_multiplier".into(),
            Value::String(rs09_game_core::java_double::format(
                self.experience_multiplier,
            )),
        );
        if self.combat_milestone > 0 || self.skill_milestone > 0 {
            root.insert("milestone".into(),serde_json::json!({"combatMilestone":self.combat_milestone.to_string(),"skillMilestone":self.skill_milestone.to_string()}));
        }
    }
}
fn curve() -> &'static Vec<i32> {
    static CURVE: OnceLock<Vec<i32>> = OnceLock::new();
    CURVE.get_or_init(|| {
        let mut curve = vec![0, 0];
        let mut points = 0i32;
        for level in 1..512 {
            points = (f64::from(points)
                + (f64::from(level) + 300.0 * 2.0f64.powf(f64::from(level) / 7.0)).floor())
                as i32;
            curve.push(points / 4);
            if points == i32::MAX {
                break;
            }
        }
        curve
    })
}
fn java_ascii_name_equal(left: &str, right: &str) -> bool {
    let right = right
        .chars()
        .map(|ch| match ch {
            '\u{131}' | '\u{130}' => 'i',
            '\u{17f}' => 's',
            '\u{212a}' => 'k',
            ch => ch.to_ascii_lowercase(),
        })
        .collect::<String>();
    left.eq_ignore_ascii_case(&right)
}
fn object(value: &Value) -> Result<&Map<String, Value>> {
    value.as_object().ok_or(if value.is_null() {
        Error::Null
    } else {
        Error::Type
    })
}
fn array(value: &Value) -> Result<&Vec<Value>> {
    value.as_array().ok_or(if value.is_null() {
        Error::Null
    } else {
        Error::Type
    })
}
fn required<'a>(data: &'a Map<String, Value>, key: &str) -> Result<&'a Value> {
    data.get(key)
        .filter(|value| !value.is_null())
        .ok_or(Error::Null)
}
fn string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) if value.is_f64() => {
            rs09_game_core::java_double::format(value.as_f64().unwrap())
        }
        _ => value.to_string(),
    }
}
fn int(value: &Value) -> Result<i32> {
    rs09_game_core::java_string::parse_i32(&string(value)).ok_or(Error::Number)
}
fn double(value: &Value) -> Result<f64> {
    rs09_game_core::java_double::parse(&string(value)).ok_or(Error::Number)
}
fn nullable_int(value: Option<&Value>) -> Result<i32> {
    rs09_game_core::java_string::parse_i32(&value.map_or_else(|| "null".into(), string))
        .ok_or(Error::Number)
}
fn nullable_double(value: Option<&Value>) -> Result<f64> {
    rs09_game_core::java_double::parse(&value.map_or_else(|| "null".into(), string))
        .ok_or(Error::Number)
}
