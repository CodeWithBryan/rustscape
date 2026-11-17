//! The player's prayers: `Prayer`, `PrayerType` and `PrayerCategory` of the
//! pinned server, the prayer tab's button handler (`PrayerTabInterface`) and
//! the two `Player` members that start prayer effects (`commenceDeath`'s
//! retribution and `fullRestore`'s reset).
//!
//! State a `Prayer` owns lives in [`PrayerState`]; the host keeps one per
//! player and hands it out through [`Host::prayer`], so no borrow of it is
//! held across a service call. Prayer points, static levels and healing are
//! the native [`Skills`] the host hands out with [`Host::skills`]. Everything
//! else the original reaches is a required [`Host`] service called in the
//! original order; none has a default. [`combat`] answers the combat slices'
//! prayer services from this state. This is a library: nothing in a session
//! calls it yet. See docs/prayer.md.
pub mod combat;

use crate::player::skills::{self, Skills};
use crate::world::combat_pulse::HitsplatType;
use crate::world::combat_swing::{CombatStyle, Error, Result};

/// `Skills` slot numbers read here.
pub mod skill {
    pub const ATTACK: i32 = 0;
    pub const DEFENCE: i32 = 1;
    pub const STRENGTH: i32 = 2;
    pub const RANGE: i32 = 4;
    pub const PRAYER: i32 = 5;
    pub const MAGIC: i32 = 6;
}

/// `Sounds` constants the classes name (their ConstLib values).
pub mod sound {
    pub const CANCEL_PRAYER: i32 = 2663;
    pub const PRAYER_DRAIN: i32 = 2672;
    pub const REDEMPTION_HEAL: i32 = 2681;
    pub const FIREBREATH: i32 = 159;
}

/// `PrayerCategory`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    BabyBlue,
    Green,
    Pink,
    LimeGreen,
    Orange,
    Purple,
    DarkGreen,
    DarkBrown,
    LightBrown,
    Red,
    Magenta,
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Self::BabyBlue => "BABY_BLUE",
            Self::Green => "GREEN",
            Self::Pink => "PINK",
            Self::LimeGreen => "LIME_GREEN",
            Self::Orange => "ORANGE",
            Self::Purple => "PURPLE",
            Self::DarkGreen => "DARK_GREEN",
            Self::DarkBrown => "DARK_BROWN",
            Self::LightBrown => "LIGHT_BROWN",
            Self::Red => "RED",
            Self::Magenta => "MAGENTA",
        }
    }
}

/// `PrayerType`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrayerType {
    ThickSkin,
    BurstOfStrength,
    ClarityOfThought,
    SharpEye,
    MysticWill,
    RockSkin,
    SuperhumanStrength,
    ImprovedReflexes,
    RapidRestore,
    RapidHeal,
    ProtectItems,
    HawkEye,
    MysticLore,
    SteelSkin,
    UltimateStrength,
    IncredibleReflexes,
    ProtectFromSummoning,
    ProtectFromMagic,
    ProtectFromMissiles,
    ProtectFromMelee,
    EagleEye,
    MysticMight,
    Retribution,
    Redemption,
    Smite,
    Chivalry,
    Piety,
}

/// One `SkillBonus` (`baseBonus` is always 0 for prayers and never read).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillBonus {
    pub skill: i32,
    pub bonus: f64,
}

/// The constructor arguments of one constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub level: i32,
    pub drain: i32,
    pub config: i32,
    pub button: i32,
    pub restriction: Category,
    pub second_restriction: Option<Category>,
    pub sound: i32,
    pub defence_req: i32,
    pub bonuses: &'static [SkillBonus],
}

macro_rules! b {
    ($skill:expr, $bonus:expr) => {
        SkillBonus {
            skill: $skill,
            bonus: $bonus,
        }
    };
}

#[allow(clippy::too_many_arguments)]
const fn row(
    level: i32,
    drain: i32,
    config: i32,
    button: i32,
    restriction: Category,
    second_restriction: Option<Category>,
    sound: i32,
    defence_req: i32,
    bonuses: &'static [SkillBonus],
) -> Row {
    Row {
        level,
        drain,
        config,
        button,
        restriction,
        second_restriction,
        sound,
        defence_req,
        bonuses,
    }
}

use Category as C;
use skill::{ATTACK, DEFENCE, MAGIC, RANGE, STRENGTH};

impl PrayerType {
    /// `PrayerType.values()`.
    pub const ALL: [Self; 27] = [
        Self::ThickSkin,
        Self::BurstOfStrength,
        Self::ClarityOfThought,
        Self::SharpEye,
        Self::MysticWill,
        Self::RockSkin,
        Self::SuperhumanStrength,
        Self::ImprovedReflexes,
        Self::RapidRestore,
        Self::RapidHeal,
        Self::ProtectItems,
        Self::HawkEye,
        Self::MysticLore,
        Self::SteelSkin,
        Self::UltimateStrength,
        Self::IncredibleReflexes,
        Self::ProtectFromSummoning,
        Self::ProtectFromMagic,
        Self::ProtectFromMissiles,
        Self::ProtectFromMelee,
        Self::EagleEye,
        Self::MysticMight,
        Self::Retribution,
        Self::Redemption,
        Self::Smite,
        Self::Chivalry,
        Self::Piety,
    ];

    /// The constant's constructor arguments, as the pinned enum declares them.
    pub const fn row(self) -> Row {
        match self {
            Self::ThickSkin => row(
                1,
                12,
                83,
                5,
                C::BabyBlue,
                None,
                2690,
                1,
                &[b!(DEFENCE, 0.05)],
            ),
            Self::BurstOfStrength => {
                row(4, 12, 84, 7, C::Green, None, 2688, 1, &[b!(STRENGTH, 0.05)])
            }
            Self::ClarityOfThought => {
                row(7, 12, 85, 9, C::Pink, None, 2664, 1, &[b!(ATTACK, 0.05)])
            }
            Self::SharpEye => row(
                8,
                12,
                862,
                11,
                C::LimeGreen,
                None,
                2685,
                1,
                &[b!(RANGE, 0.05)],
            ),
            Self::MysticWill => row(
                9,
                12,
                863,
                13,
                C::LimeGreen,
                None,
                2670,
                1,
                &[b!(MAGIC, 0.05)],
            ),
            Self::RockSkin => row(
                10,
                6,
                86,
                15,
                C::BabyBlue,
                None,
                2684,
                1,
                &[b!(DEFENCE, 0.1)],
            ),
            Self::SuperhumanStrength => {
                row(13, 6, 87, 17, C::Green, None, 2689, 1, &[b!(STRENGTH, 0.1)])
            }
            Self::ImprovedReflexes => {
                row(16, 6, 88, 19, C::Pink, None, 2662, 1, &[b!(ATTACK, 0.1)])
            }
            Self::RapidRestore => row(19, 26, 89, 21, C::Purple, None, 2679, 1, &[]),
            Self::RapidHeal => row(22, 18, 90, 23, C::Purple, None, 2678, 1, &[]),
            Self::ProtectItems => row(25, 18, 91, 25, C::DarkGreen, None, 1982, 1, &[]),
            Self::HawkEye => row(
                26,
                6,
                864,
                27,
                C::LimeGreen,
                None,
                2666,
                1,
                &[b!(RANGE, 0.1)],
            ),
            Self::MysticLore => row(
                27,
                6,
                865,
                29,
                C::LimeGreen,
                None,
                2668,
                1,
                &[b!(MAGIC, 0.1)],
            ),
            Self::SteelSkin => row(
                28,
                3,
                92,
                31,
                C::BabyBlue,
                None,
                2687,
                1,
                &[b!(DEFENCE, 0.15)],
            ),
            Self::UltimateStrength => row(
                31,
                3,
                93,
                33,
                C::Green,
                None,
                2691,
                1,
                &[b!(STRENGTH, 0.15)],
            ),
            Self::IncredibleReflexes => {
                row(34, 3, 94, 35, C::Pink, None, 2667, 1, &[b!(ATTACK, 0.15)])
            }
            Self::ProtectFromSummoning => row(
                35,
                2,
                1168,
                53,
                C::DarkBrown,
                Some(C::Magenta),
                4262,
                1,
                &[],
            ),
            Self::ProtectFromMagic => row(37, 3, 95, 37, C::LightBrown, None, 2675, 1, &[]),
            Self::ProtectFromMissiles => row(40, 3, 96, 39, C::LightBrown, None, 2677, 1, &[]),
            Self::ProtectFromMelee => row(43, 4, 97, 41, C::LightBrown, None, 2676, 1, &[]),
            Self::EagleEye => row(
                44,
                3,
                866,
                43,
                C::LimeGreen,
                None,
                2665,
                1,
                &[b!(RANGE, 0.15)],
            ),
            Self::MysticMight => row(
                45,
                3,
                867,
                45,
                C::LimeGreen,
                None,
                2669,
                1,
                &[b!(MAGIC, 0.15)],
            ),
            Self::Retribution => row(
                46,
                12,
                98,
                47,
                C::LightBrown,
                Some(C::Magenta),
                2682,
                1,
                &[],
            ),
            Self::Redemption => row(49, 6, 99, 49, C::LightBrown, Some(C::Magenta), 2680, 1, &[]),
            Self::Smite => row(
                52,
                2,
                100,
                51,
                C::LightBrown,
                Some(C::Magenta),
                2686,
                1,
                &[],
            ),
            Self::Chivalry => row(
                60,
                2,
                1052,
                55,
                C::Pink,
                None,
                3826,
                65,
                &[b!(DEFENCE, 0.2), b!(STRENGTH, 0.18), b!(ATTACK, 0.15)],
            ),
            Self::Piety => row(
                70,
                2,
                1053,
                57,
                C::Pink,
                None,
                3825,
                70,
                &[b!(DEFENCE, 0.25), b!(STRENGTH, 0.23), b!(ATTACK, 0.2)],
            ),
        }
    }

    /// `name()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::ThickSkin => "THICK_SKIN",
            Self::BurstOfStrength => "BURST_OF_STRENGTH",
            Self::ClarityOfThought => "CLARITY_OF_THOUGHT",
            Self::SharpEye => "SHARP_EYE",
            Self::MysticWill => "MYSTIC_WILL",
            Self::RockSkin => "ROCK_SKIN",
            Self::SuperhumanStrength => "SUPERHUMAN_STRENGTH",
            Self::ImprovedReflexes => "IMPROVED_REFLEXES",
            Self::RapidRestore => "RAPID_RESTORE",
            Self::RapidHeal => "RAPID_HEAL",
            Self::ProtectItems => "PROTECT_ITEMS",
            Self::HawkEye => "HAWK_EYE",
            Self::MysticLore => "MYSTIC_LORE",
            Self::SteelSkin => "STEEL_SKIN",
            Self::UltimateStrength => "ULTIMATE_STRENGTH",
            Self::IncredibleReflexes => "INCREDIBLE_REFLEXES",
            Self::ProtectFromSummoning => "PROTECT_FROM_SUMMONING",
            Self::ProtectFromMagic => "PROTECT_FROM_MAGIC",
            Self::ProtectFromMissiles => "PROTECT_FROM_MISSILES",
            Self::ProtectFromMelee => "PROTECT_FROM_MELEE",
            Self::EagleEye => "EAGLE_EYE",
            Self::MysticMight => "MYSTIC_MIGHT",
            Self::Retribution => "RETRIBUTION",
            Self::Redemption => "REDEMPTION",
            Self::Smite => "SMITE",
            Self::Chivalry => "CHIVALRY",
            Self::Piety => "PIETY",
        }
    }

    pub fn ordinal(self) -> usize {
        self as usize
    }

    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }

    /// The protection prayer of a combat style (`CombatStyle.getProtectionPrayer`).
    pub fn protection(style: CombatStyle) -> Self {
        match style {
            CombatStyle::Melee => Self::ProtectFromMelee,
            CombatStyle::Range => Self::ProtectFromMissiles,
            CombatStyle::Magic => Self::ProtectFromMagic,
        }
    }

    /// `PrayerType.get(button)`: the first constant with that button.
    pub fn for_button(button: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.row().button == button)
    }

    /// `getByBonus(ids)`: an array as long as `values()`, one entry for
    /// every matching (bonus, id) pair in declaration order, then nulls.
    pub fn by_bonus(ids: &[i32]) -> Result<Vec<Option<Self>>> {
        let mut types = vec![None; Self::ALL.len()];
        let mut count = 0;
        for t in Self::ALL {
            for bonus in t.row().bonuses {
                for &id in ids {
                    if id == bonus.skill {
                        *types.get_mut(count).ok_or(Error::Index)? = Some(t);
                        count += 1;
                    }
                }
            }
        }
        Ok(types)
    }

    /// `getMeleeTypes()`.
    pub fn melee_types() -> Result<Vec<Option<Self>>> {
        Self::by_bonus(&[ATTACK, STRENGTH])
    }

    /// `getRangeTypes()`.
    pub fn range_types() -> Result<Vec<Option<Self>>> {
        Self::by_bonus(&[RANGE])
    }

    /// `getMagicTypes()`.
    pub fn magic_types() -> Result<Vec<Option<Self>>> {
        Self::by_bonus(&[MAGIC])
    }

    /// `StringUtils.formatDisplayName(name().toLowerCase().replace("_", " "))`.
    /// Constant names are ASCII, so the default-locale casing is plain ASCII.
    pub fn display_name(self) -> String {
        let name = self.name().to_ascii_lowercase().replace('_', " ");
        let mut out = String::with_capacity(name.len());
        let mut was_space = true;
        for c in name.chars() {
            if was_space {
                out.push(c.to_ascii_uppercase());
                was_space = false;
            } else {
                out.push(c);
            }
            if c == ' ' {
                was_space = true;
            }
        }
        out
    }
}

/// One `ICON_CACHE` element: the array is `Object[]`, so a row mixes
/// constants and boxed integers.
#[derive(Clone, Copy)]
enum IconCell {
    Type(PrayerType),
    Int(i32),
}

use IconCell::{Int, Type};
use PrayerType as P;

/// `PrayerType.ICON_CACHE`.
const ICON_CACHE: [&[IconCell]; 7] = [
    &[Type(P::Redemption), Int(5)],
    &[Type(P::Retribution), Int(3)],
    &[Type(P::Smite), Int(4)],
    &[Type(P::ProtectFromMagic), Int(2), Int(10)],
    &[Type(P::ProtectFromMelee), Int(0), Int(8)],
    &[Type(P::ProtectFromMissiles), Int(1), Int(9)],
    &[
        Type(P::ProtectFromSummoning),
        Int(7),
        Type(P::ProtectFromMelee),
        Int(8),
        Type(P::ProtectFromMissiles),
        Int(9),
        Type(P::ProtectFromMagic),
        Int(10),
    ],
];

fn int(cell: IconCell) -> i32 {
    match cell {
        Int(v) => v,
        // `(int) ICON_CACHE[i][k]` never meets a constant at the indexes read.
        Type(_) => unreachable!("ICON_CACHE integer cell"),
    }
}

/// The fields of one `Prayer` object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PrayerState {
    /// `active`: an `ArrayList` in insertion order.
    pub active: Vec<PrayerType>,
    /// `prayerActiveTicks`.
    pub prayer_active_ticks: i32,
    /// The list's `modCount`, which its iterators compare.
    pub mod_count: u32,
}

impl PrayerState {
    /// `get(type)` (`active.contains`).
    pub fn get(&self, t: PrayerType) -> bool {
        self.active.contains(&t)
    }

    fn add(&mut self, t: PrayerType) {
        self.active.push(t);
        self.mod_count = self.mod_count.wrapping_add(1);
    }

    /// `ArrayList.remove(Object)`: the first occurrence; no change when absent.
    fn remove(&mut self, t: PrayerType) {
        if let Some(i) = self.active.iter().position(|&a| a == t) {
            self.active.remove(i);
            self.mod_count = self.mod_count.wrapping_add(1);
        }
    }

    fn clear(&mut self) {
        self.active.clear();
        self.mod_count = self.mod_count.wrapping_add(1);
    }

    /// `getSkillBonus(skillId)`: summed over the active list in order.
    pub fn skill_bonus(&self, skill_id: i32) -> f64 {
        let mut bonus = 0.0;
        for t in &self.active {
            for b in t.row().bonuses {
                if b.skill == skill_id {
                    bonus += b.bonus;
                }
            }
        }
        bonus
    }

    /// `getIcon(player, type)`.
    pub fn icon(&self, t: PrayerType) -> i32 {
        for row in ICON_CACHE {
            let Type(first) = row[0] else { continue };
            if row.len() == 2 && t == first {
                return int(row[1]);
            } else if row.len() == 3 && t == first {
                if self.get(P::ProtectFromSummoning) {
                    return int(row[2]);
                }
                return int(row[1]);
            } else if row.len() == 8 && t == first {
                for k in 2..row.len() {
                    // `active.contains(Integer)` is false.
                    if let Type(candidate) = row[k]
                        && self.get(candidate)
                    {
                        return int(row[k + 1]);
                    }
                }
                return int(row[1]);
            }
        }
        -1
    }

    /// `hasIcon(player)`.
    pub fn has_icon(&self) -> bool {
        self.active.iter().filter(|&&t| self.icon(t) != -1).count() != 0
    }
}

/// The event classes `Player.dispatch` receives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrayerEvent {
    Activated(PrayerType),
    Deactivated(PrayerType),
}

/// The skillcape perks read here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perk {
    DivineFavor,
}

/// `Projectile.create(start, destination, id, startHeight, endHeight,
/// startDelay, speed, angle, distance)` followed by `send()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileRequest {
    pub start: [i32; 3],
    pub destination: [i32; 3],
    pub id: i32,
    pub start_height: i32,
    pub end_height: i32,
    pub start_delay: i32,
    pub speed: i32,
    pub angle: i32,
    pub distance: i32,
}

/// Everything `Prayer`, `PrayerType` and the ported `Player` members reach
/// outside themselves. The first group are plain accessors to state and
/// class identity: they are not service calls. Every other method is one
/// original call; a failure leaves earlier effects in place.
pub trait Host {
    /// One entity handle (a player or an NPC); handles keep their identity.
    type Entity: Clone;
    /// The binding of the native skills to that entity's world.
    type SkillsContext: skills::Context;

    /// Reference identity of two entities.
    fn same_entity(&self, a: &Self::Entity, b: &Self::Entity) -> bool;
    /// `entity instanceof NPC`.
    fn is_npc(&self, entity: &Self::Entity) -> bool;
    /// `player.getPrayer()`.
    fn prayer(&mut self, player: &Self::Entity) -> &mut PrayerState;
    /// `player.getSkills()` and the context its methods need.
    fn skills(&mut self, player: &Self::Entity) -> (&mut Skills, &mut Self::SkillsContext);

    /// `setVarp(player, id, value, save)`.
    fn set_varp(&mut self, player: &Self::Entity, id: i32, value: i32, save: bool) -> Result<()>;
    /// `getVarbit(player, id)`.
    fn get_varbit(&mut self, player: &Self::Entity, id: i32) -> Result<i32>;
    /// `setVarbit(player, id, value, save)`.
    fn set_varbit(&mut self, player: &Self::Entity, id: i32, value: i32, save: bool) -> Result<()>;
    /// `playAudio(player, id)` with its defaults (delay 0, one loop, no
    /// location, radius 8).
    fn play_audio(&mut self, player: &Self::Entity, id: i32) -> Result<()>;
    /// `player.dispatch(event)`.
    fn dispatch(&mut self, player: &Self::Entity, event: PrayerEvent) -> Result<()>;
    /// `appearance.getHeadIcon()`.
    fn head_icon(&mut self, player: &Self::Entity) -> Result<i32>;
    /// `appearance.setHeadIcon(icon)`.
    fn set_head_icon(&mut self, player: &Self::Entity, icon: i32) -> Result<()>;
    /// `appearance.sync()`.
    fn sync_appearance(&mut self, player: &Self::Entity) -> Result<()>;
    /// `sendDialogue(player, text)`.
    fn send_dialogue(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `hasRequirement(player, Quests.KINGS_RANSOM)` (message true).
    fn has_kings_ransom(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `GameWorld.getPulser().submit(new Pulse(1) {...})` from `reset`. The
    /// runner calls [`head_icon_pulse`] when the pulse fires.
    fn submit_head_icon_pulse(&mut self, player: &Self::Entity, delay: i32) -> Result<()>;
    /// `player.sendMessage(text)`.
    fn send_message(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `getProperties().getBonuses()`; `None` is a null array.
    fn bonuses(&mut self, player: &Self::Entity) -> Result<Option<Vec<i32>>>;
    /// `SkillcapePerks.isActive(perk, player)`.
    fn perk_active(&mut self, player: &Self::Entity, perk: Perk) -> Result<bool>;
    /// One `RandomFunction.RANDOM.nextInt(bound)` draw; `bound` is positive.
    fn next_int(&mut self, bound: i32) -> Result<i32>;
    /// `player.graphics(Graphics.create(id))`.
    fn graphics(&mut self, player: &Self::Entity, id: i32) -> Result<()>;
    /// `getLocation()`; `None` is null.
    fn location(&mut self, entity: &Self::Entity) -> Result<Option<[i32; 3]>>;
    fn send_projectile(&mut self, request: ProjectileRequest) -> Result<()>;
    /// `owner.getImpactHandler().manualHit(source, hit, type)`.
    fn manual_hit(
        &mut self,
        owner: &Self::Entity,
        source: &Self::Entity,
        hit: i32,
        kind: HitsplatType,
    ) -> Result<()>;
    /// `getProperties().isMultiZone()`.
    fn is_multi_zone(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `RegionManager.getLocalNPCs(location, distance)`.
    fn local_npcs(&mut self, location: [i32; 3], distance: i32) -> Result<Vec<Self::Entity>>;
    /// `RegionManager.getLocalPlayers(location, distance)`.
    fn local_players(&mut self, location: [i32; 3], distance: i32) -> Result<Vec<Self::Entity>>;
    /// `entity.isAttackable(attacker, style, message)`.
    fn is_attackable(
        &mut self,
        entity: &Self::Entity,
        attacker: &Self::Entity,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool>;
    /// `Player.isPlaying()`.
    fn is_playing(&mut self, player: &Self::Entity) -> Result<bool>;
    /// `Entity.commenceDeath(killer)` (the superclass body).
    fn entity_commence_death(
        &mut self,
        player: &Self::Entity,
        killer: Option<&Self::Entity>,
    ) -> Result<()>;
    /// `settings.setSpecialEnergy(value)`.
    fn set_special_energy(&mut self, player: &Self::Entity, value: i32) -> Result<()>;
    /// `settings.updateRunEnergy(amount)`.
    fn update_run_energy(&mut self, player: &Self::Entity, amount: f64) -> Result<()>;
    /// `Entity.fullRestore()` (the superclass body).
    fn entity_full_restore(&mut self, player: &Self::Entity) -> Result<()>;
}

fn skills_error(e: skills::Error) -> Error {
    match e {
        skills::Error::Index => Error::Index,
        skills::Error::Null => Error::Null,
        other => Error::Binding(format!("skills {other:?}")),
    }
}

fn prayer_points<H: Host>(h: &mut H, p: &H::Entity) -> f64 {
    h.skills(p).0.prayer_points
}

/// `hasLevelStat(player, skill, level)`: `getStaticLevel(skill) >= level`.
fn has_level<H: Host>(h: &mut H, p: &H::Entity, slot: i32, level: i32) -> bool {
    h.skills(p).0.static_levels[slot as usize] >= level
}

fn static_prayer<H: Host>(h: &mut H, p: &H::Entity) -> i32 {
    h.skills(p).0.static_levels[skill::PRAYER as usize]
}

fn decrement_prayer_points<H: Host>(h: &mut H, p: &H::Entity, amount: f64) -> Result<()> {
    let (s, c) = h.skills(p);
    s.decrement_prayer(amount, c).map_err(skills_error)
}

fn set_prayer_points<H: Host>(h: &mut H, p: &H::Entity, amount: f64) -> Result<()> {
    let (s, c) = h.skills(p);
    s.set_prayer(amount, c).map_err(skills_error)
}

fn heal<H: Host>(h: &mut H, p: &H::Entity, amount: i32) -> Result<i32> {
    let (s, c) = h.skills(p);
    s.heal(amount, c).map_err(skills_error)
}

/// `Prayer.toggle(type)`.
pub fn toggle<H: Host>(h: &mut H, player: &H::Entity, t: PrayerType) -> Result<bool> {
    if !permitted(h, player, t)? {
        return Ok(false);
    }
    let on = !h.prayer(player).get(t);
    type_toggle(h, player, t, on)
}

/// `Prayer.permitted(type)` (private).
fn permitted<H: Host>(h: &mut H, player: &H::Entity, t: PrayerType) -> Result<bool> {
    Ok(prayer_points(h, player) > 0.0 && type_permitted(h, player, t)?)
}

/// `PrayerType.permitted(player)`.
pub fn type_permitted<H: Host>(h: &mut H, player: &H::Entity, t: PrayerType) -> Result<bool> {
    let row = t.row();
    let prayer_ok = has_level(h, player, skill::PRAYER, row.level);
    if !prayer_ok || !has_level(h, player, skill::DEFENCE, row.defence_req) {
        let mut text = String::from("You need a <col=08088A>");
        if !has_level(h, player, skill::PRAYER, row.level) {
            text.push_str(&format!("Prayer level of {}", row.level));
            if !has_level(h, player, skill::DEFENCE, row.defence_req) {
                text.push_str(" and a ");
            }
        }
        if !has_level(h, player, skill::DEFENCE, row.defence_req) {
            text.push_str(&format!("Defence level of {}", row.defence_req));
        }
        text.push_str(&format!(" to use {}.", t.display_name()));
        h.send_dialogue(player, &text)?;
        return Ok(false);
    }
    if row.defence_req == 65 || row.defence_req == 70 {
        if !h.has_kings_ransom(player)? {
            return Ok(false);
        } else if h.get_varbit(player, 3909)? != 8 {
            h.set_varbit(player, 3909, 8, false)?;
        }
    }
    Ok(true)
}

/// `PrayerType.toggle(player, on)`.
pub fn type_toggle<H: Host>(
    h: &mut H,
    player: &H::Entity,
    t: PrayerType,
    on: bool,
) -> Result<bool> {
    h.set_varp(player, t.row().config, if on { 1 } else { 0 }, false)?;
    if on {
        flag(h, player, t)?;
        h.prayer(player).add(t);
        let icon = h.prayer(player).icon(t);
        iconify(h, player, icon)?;
        h.play_audio(player, t.row().sound)?;
        h.dispatch(player, PrayerEvent::Activated(t))?;
    } else {
        h.prayer(player).remove(t);
        h.play_audio(player, sound::CANCEL_PRAYER)?;
        find_next_icon(h, player, t)?;
        h.dispatch(player, PrayerEvent::Deactivated(t))?;
    }
    Ok(true)
}

fn conflicts(b: i32, bb: i32) -> bool {
    bb == b
        || (b == STRENGTH || b == ATTACK) && (bb == MAGIC || bb == RANGE)
        || (b == RANGE || b == MAGIC) && (bb == ATTACK || bb == STRENGTH)
        || (b == DEFENCE && bb == DEFENCE)
}

/// `PrayerType.flag(player, type)`. An active prayer is listed once per
/// conflicting bonus pair and switched off once per listing; the array holds
/// `active.size() + 10` entries.
pub fn flag<H: Host>(h: &mut H, player: &H::Entity, t: PrayerType) -> Result<()> {
    let row = t.row();
    let remove = {
        let state = h.prayer(player);
        let capacity = state.active.len() + 10;
        let mut remove = Vec::new();
        let mut push = |a: PrayerType| {
            if remove.len() >= capacity {
                return Err(Error::Index);
            }
            remove.push(a);
            Ok(())
        };
        for &a in &state.active {
            let ar = a.row();
            if ar.restriction == row.restriction
                || ar.second_restriction.is_some()
                    && row.second_restriction.is_some()
                    && ar.second_restriction == row.second_restriction
            {
                push(a)?;
                continue;
            }
            for b in ar.bonuses {
                for bb in row.bonuses {
                    if conflicts(b.skill, bb.skill) {
                        push(a)?;
                    }
                }
            }
        }
        remove
    };
    for r in remove {
        type_toggle(h, player, r, false)?;
    }
    Ok(())
}

/// `PrayerType.iconify(player, icon)`; always answers false.
pub fn iconify<H: Host>(h: &mut H, player: &H::Entity, icon: i32) -> Result<bool> {
    if icon == -1 {
        return Ok(false);
    }
    h.set_head_icon(player, icon)?;
    h.sync_appearance(player)?;
    Ok(false)
}

/// The `ArrayList` iterator's `next()`: fails when the list changed.
fn next_active<H: Host>(
    h: &mut H,
    player: &H::Entity,
    cursor: usize,
    expected: u32,
) -> Result<Option<PrayerType>> {
    let state = h.prayer(player);
    if cursor == state.active.len() {
        return Ok(None);
    }
    if state.mod_count != expected {
        return Err(Error::Binding("ConcurrentModificationException".into()));
    }
    state
        .active
        .get(cursor)
        .copied()
        .map(Some)
        .ok_or(Error::Binding("ConcurrentModificationException".into()))
}

/// `PrayerType.findNextIcon(player)` of the constant `t`.
pub fn find_next_icon<H: Host>(h: &mut H, player: &H::Entity, t: PrayerType) -> Result<()> {
    if !h.prayer(player).has_icon() {
        h.set_head_icon(player, -1)?;
        h.sync_appearance(player)?;
    }
    if matches!(
        t,
        P::ProtectFromMelee | P::ProtectFromMissiles | P::ProtectFromMagic
    ) && h.prayer(player).get(P::ProtectFromSummoning)
    {
        iconify(h, player, 7)?;
    } else if t == P::ProtectFromSummoning {
        let expected = h.prayer(player).mod_count;
        let mut cursor = 0;
        while let Some(a) = next_active(h, player, cursor, expected)? {
            cursor += 1;
            let icon = h.prayer(player).icon(a);
            iconify(h, player, icon)?;
        }
        if h.head_icon(player)? == 7 {
            h.set_head_icon(player, -1)?;
            h.sync_appearance(player)?;
        }
    }
    Ok(())
}

/// `Prayer.reset()`. The head icon clears a tick later, in the pulse it
/// submits ([`head_icon_pulse`]).
pub fn reset<H: Host>(h: &mut H, player: &H::Entity) -> Result<()> {
    let expected = h.prayer(player).mod_count;
    let mut cursor = 0;
    while let Some(t) = next_active(h, player, cursor, expected)? {
        cursor += 1;
        h.set_varp(player, t.row().config, 0, false)?;
        h.dispatch(player, PrayerEvent::Deactivated(t))?;
    }
    h.prayer(player).clear();
    h.submit_head_icon_pulse(player, 1)
}

/// The body of the `Pulse(1)` that `reset` submits; it finishes at once.
pub fn head_icon_pulse<H: Host>(h: &mut H, player: &H::Entity) -> Result<bool> {
    h.set_head_icon(player, -1)?;
    h.sync_appearance(player)?;
    Ok(true)
}

/// `Prayer.startRedemption()`.
pub fn start_redemption<H: Host>(h: &mut H, player: &H::Entity) -> Result<()> {
    h.play_audio(player, sound::REDEMPTION_HEAL)?;
    h.graphics(player, 436)?;
    let amount = (f64::from(static_prayer(h, player)) * 0.25) as i32;
    heal(h, player, amount)?;
    set_prayer_points(h, player, 0.0)?;
    reset(h, player)
}

/// `Location(x, y, z)`: z normalised to 0..3, then the fifteen-bit signed
/// packing of `LocationHash`.
pub fn location(x: i32, y: i32, z: i32) -> [i32; 3] {
    let unpack = |v: i32| {
        let raw = v & 0x7FFF;
        if raw >= 0x4000 { raw - 0x8000 } else { raw }
    };
    [unpack(x), unpack(y), z.rem_euclid(4) & 0x3]
}

/// `Location.transform(diffX, diffY, z)`.
pub fn transform(l: [i32; 3], dx: i32, dy: i32, dz: i32) -> [i32; 3] {
    location(
        l[0].wrapping_add(dx),
        l[1].wrapping_add(dy),
        l[2].wrapping_add(dz) % 4,
    )
}

/// `Location.getDistance(first, second)`.
pub fn distance(first: [i32; 3], second: [i32; 3]) -> f64 {
    let x = first[0].wrapping_sub(second[0]);
    let y = first[1].wrapping_sub(second[1]);
    f64::from(x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y))).sqrt()
}

/// `Location.withinDistance(other, dist)`.
pub fn within_distance(this: [i32; 3], other: [i32; 3], dist: i32) -> bool {
    if other[2] != this[2] {
        return false;
    }
    distance(this, other) <= f64::from(dist)
}

/// `RandomFunction.randomize(value)`: no draw below 1.
fn randomize<H: Host>(h: &mut H, value: i32) -> Result<i32> {
    if value < 1 {
        return Ok(0);
    }
    h.next_int(value)
}

/// `RandomFunction.random(maxValue)`: no draw for a nonpositive bound.
fn random<H: Host>(h: &mut H, value: i32) -> Result<i32> {
    if value <= 0 {
        return Ok(0);
    }
    h.next_int(value)
}

/// `Prayer.startRetribution(killer)`.
pub fn start_retribution<H: Host>(
    h: &mut H,
    player: &H::Entity,
    killer: Option<&H::Entity>,
) -> Result<()> {
    let l = h.location(player)?;
    for x in -1..2 {
        for y in -1..2 {
            if x != 0 || y != 0 {
                let l = l.ok_or(Error::Null)?;
                let destination = transform(l, x, y, 0);
                h.send_projectile(ProjectileRequest {
                    start: l,
                    destination,
                    id: 438,
                    start_height: 0,
                    end_height: 0,
                    start_delay: 10,
                    speed: 20,
                    angle: 0,
                    distance: 11,
                })?;
            }
        }
    }
    h.play_audio(player, sound::FIREBREATH)?;
    h.graphics(player, 437)?;
    let maximum = ((f64::from(static_prayer(h, player)) * 0.25) as i32).wrapping_sub(1);
    let is_player = killer.is_some_and(|k| h.same_entity(k, player));
    if !is_player {
        let k = killer.ok_or(Error::Null)?;
        // The argument is evaluated before the null receiver fails.
        let from = h.location(k)?;
        let to = h.location(player)?;
        let (from, to) = (from.ok_or(Error::Null)?, to.ok_or(Error::Null)?);
        if within_distance(from, to, 1) {
            let hit = 1i32.wrapping_add(randomize(h, maximum)?);
            h.manual_hit(k, player, hit, HitsplatType::Normal)?;
        }
    }
    if h.is_multi_zone(player)? {
        let npc = killer.is_some_and(|k| h.is_npc(k));
        let at = h.location(player)?.ok_or(Error::Null)?;
        let targets = if npc {
            h.local_npcs(at, 1)?
        } else {
            h.local_players(at, 1)?
        };
        for entity in &targets {
            if h.same_entity(entity, player) || killer.is_some_and(|k| h.same_entity(entity, k)) {
                continue;
            }
            if h.is_attackable(entity, player, CombatStyle::Magic, false)? {
                let hit = 1i32.wrapping_add(randomize(h, maximum)?);
                h.manual_hit(entity, player, hit, HitsplatType::Normal)?;
            }
        }
    }
    Ok(())
}

/// `Prayer.tick()`, called from `Player.tick` once a game tick.
pub fn tick<H: Host>(h: &mut H, player: &H::Entity) -> Result<()> {
    {
        let state = h.prayer(player);
        if !state.active.is_empty() {
            state.prayer_active_ticks = state.prayer_active_ticks.wrapping_add(1);
        } else {
            state.prayer_active_ticks = 0;
        }
    }
    let ticks = h.prayer(player).prayer_active_ticks;
    if ticks > 0 && ticks % 2 == 0 {
        if prayer_points(h, player) == 0.0 {
            h.play_audio(player, sound::PRAYER_DRAIN)?;
            h.send_message(
                player,
                "You have run out of prayer points; you must recharge at an altar.",
            )?;
            reset(h, player)?;
            return Ok(());
        }
        let mut amount_drain = 0.0;
        let expected = h.prayer(player).mod_count;
        let mut cursor = 0;
        while let Some(t) = next_active(h, player, cursor, expected)? {
            cursor += 1;
            let mut drain = f64::from(t.row().drain);
            let bonuses = h.bonuses(player)?.ok_or(Error::Null)?;
            let item = *bonuses.get(12).ok_or(Error::Index)?;
            // `(1/30f) * bonuses[12]` is float arithmetic, widened afterwards.
            let bonus = f64::from((1.0f32 / 30.0f32) * item as f32);
            drain *= 1.0 + bonus;
            drain = 0.6 / drain;
            amount_drain += drain;
        }
        if h.perk_active(player, Perk::DivineFavor)? && random(h, 100)? <= 10 {
            amount_drain = 0.0;
        }
        decrement_prayer_points(h, player, amount_drain)?;
    }
    Ok(())
}

/// `PrayerTabInterface`'s handler for component 271 (`Components.PRAYER_271`).
pub const PRAYER_TAB: i32 = 271;

/// The listener `PrayerTabInterface` registers on the prayer tab:
/// `PrayerType.get(buttonID) ?: return true`, then `player.prayer.toggle`.
pub fn tab_button<H: Host>(h: &mut H, player: &H::Entity, button: i32) -> Result<bool> {
    match PrayerType::for_button(button) {
        None => Ok(true),
        Some(t) => toggle(h, player, t),
    }
}

/// `Player.commenceDeath(killer)`.
pub fn commence_death<H: Host>(
    h: &mut H,
    player: &H::Entity,
    killer: Option<&H::Entity>,
) -> Result<()> {
    if !h.is_playing(player)? {
        return Ok(());
    }
    h.entity_commence_death(player, killer)?;
    if h.prayer(player).get(P::Retribution) {
        start_retribution(h, player, killer)?;
    }
    Ok(())
}

/// `Player.fullRestore()`.
pub fn full_restore<H: Host>(h: &mut H, player: &H::Entity) -> Result<()> {
    reset(h, player)?;
    h.set_special_energy(player, 100)?;
    h.update_run_energy(player, -100.0)?;
    h.entity_full_restore(player)
}
