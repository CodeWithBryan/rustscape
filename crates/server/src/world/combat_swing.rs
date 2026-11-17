//! Combat swing calculation and swing-time `BattleState` assembly.
//!
//! Ports the accuracy, defence and maximum-hit calculations, the hit decision
//! and the `swing` bodies of the pinned `CombatSwingHandler`, `MeleeSwingHandler`,
//! `RangeSwingHandler` and `MagicSwingHandler`, with `BattleState` and the
//! equipment tables they read. Everything outside those classes is a required
//! [`Host`] service, called in the original order. Nothing here schedules a
//! combat pulse, applies an impact or sends a packet. See docs/combat-swing.md.
use std::sync::Arc;

pub type Result<T> = std::result::Result<T, Error>;

/// The original failure classes this slice can raise, plus a host failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A null dereference in the original.
    Null,
    /// An array or list index outside its bounds in the original.
    Index,
    /// A required service failed; preceding effects are kept.
    Binding(String),
}

/// `Skills` slot numbers read by the swing handlers.
pub mod skill {
    pub const ATTACK: i32 = 0;
    pub const DEFENCE: i32 = 1;
    pub const STRENGTH: i32 = 2;
    pub const RANGE: i32 = 4;
    pub const MAGIC: i32 = 6;
    pub const SLAYER: i32 = 18;
}

/// `EquipmentContainer` slot numbers read by the swing handlers.
pub mod slot {
    pub const AMULET: i32 = 2;
    pub const WEAPON: i32 = 3;
    pub const ARROWS: i32 = 13;
}

pub const BONUS_STAB: i32 = 0;
pub const BONUS_SLASH: i32 = 1;
pub const BONUS_CRUSH: i32 = 2;
pub const BONUS_MAGIC: i32 = 3;
pub const BONUS_RANGE: i32 = 4;
pub const STYLE_ACCURATE: i32 = 0;
pub const STYLE_AGGRESSIVE: i32 = 1;
pub const STYLE_CONTROLLED: i32 = 2;
pub const STYLE_DEFENSIVE: i32 = 3;
pub const STYLE_RANGE_ACCURATE: i32 = 4;
pub const STYLE_RAPID: i32 = 5;
pub const STYLE_LONG_RANGE: i32 = 6;
pub const STYLE_DEFENSIVE_CAST: i32 = 7;
pub const STYLE_CAST: i32 = 8;

const SALVE_AMULET: i32 = 4081;
const SALVE_AMULET_E: i32 = 10588;
const DRAGON_SLAYER_GLOVES: i32 = 12862;
const KALPHITE_QUEEN_PHASE_ONE: i32 = 1158;
const IRON_TITAN_POUCH: i32 = 12822;
const STEEL_TITAN_POUCH: i32 = 12790;
const GEYSER_TITAN_POUCH: i32 = 12786;
const WOLPERTINGER_POUCH: i32 = 12089;

/// `CombatStyle`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatStyle {
    Melee,
    Range,
    Magic,
}

impl CombatStyle {
    pub const ALL: [Self; 3] = [Self::Melee, Self::Range, Self::Magic];

    pub fn name(self) -> &'static str {
        match self {
            Self::Melee => "MELEE",
            Self::Range => "RANGE",
            Self::Magic => "MAGIC",
        }
    }

    /// The protection prayer named by the original enum constructor.
    pub fn protection_prayer(self) -> &'static str {
        match self {
            Self::Melee => "PROTECT_FROM_MELEE",
            Self::Range => "PROTECT_FROM_MISSILES",
            Self::Magic => "PROTECT_FROM_MAGIC",
        }
    }
}

/// `InteractionType`, in declaration order. `canSwing` itself is not ported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionType {
    StillInteract,
    MoveInteract,
    NoInteract,
}

impl InteractionType {
    pub const ALL: [Self; 3] = [Self::StillInteract, Self::MoveInteract, Self::NoInteract];

    pub fn name(self) -> &'static str {
        match self {
            Self::StillInteract => "STILL_INTERACT",
            Self::MoveInteract => "MOVE_INTERACT",
            Self::NoInteract => "NO_INTERACT",
        }
    }
}

/// `SwingHandlerFlag`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    IgnoreStatBoostsDamage,
    IgnoreStatBoostsAccuracy,
    IgnorePrayerBoostsDamage,
    IgnorePrayerBoostsAccuracy,
    IgnoreStatReduction,
}

impl Flag {
    pub const ALL: [Self; 5] = [
        Self::IgnoreStatBoostsDamage,
        Self::IgnoreStatBoostsAccuracy,
        Self::IgnorePrayerBoostsDamage,
        Self::IgnorePrayerBoostsAccuracy,
        Self::IgnoreStatReduction,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::IgnoreStatBoostsDamage => "IGNORE_STAT_BOOSTS_DAMAGE",
            Self::IgnoreStatBoostsAccuracy => "IGNORE_STAT_BOOSTS_ACCURACY",
            Self::IgnorePrayerBoostsDamage => "IGNORE_PRAYER_BOOSTS_DAMAGE",
            Self::IgnorePrayerBoostsAccuracy => "IGNORE_PRAYER_BOOSTS_ACCURACY",
            Self::IgnoreStatReduction => "IGNORE_STAT_REDUCTION",
        }
    }
}

/// The runtime class tests the handlers make on an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Npc,
    /// A summoning familiar: an NPC subclass.
    Familiar,
    /// Neither a player nor an NPC.
    Other,
}

impl Kind {
    pub fn is_player(self) -> bool {
        self == Self::Player
    }

    pub fn is_npc(self) -> bool {
        matches!(self, Self::Npc | Self::Familiar)
    }
}

/// `WeaponInterface.AttackStyle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackStyle {
    pub style: i32,
    pub bonus_type: i32,
}

macro_rules! s {
    ($style:expr, $bonus:expr) => {
        AttackStyle {
            style: $style,
            bonus_type: $bonus,
        }
    };
}

/// `WeaponInterface.WeaponInterfaces`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponInterfaces {
    Unarmed,
    Staff,
    Axe,
    Scepter,
    Pickaxe,
    SwordDagger,
    Scimitar,
    TwoHSword,
    Mace,
    Claws,
    WarhammerMaul,
    Whip,
    Flowers,
    MudPie,
    Spear,
    Halberd,
    Bow,
    Crossbow,
    ThrownWeapons,
    Chinchompa,
    FixedDevice,
    Salamander,
    Scythe,
    IvandisFlail,
}

impl WeaponInterfaces {
    pub const ALL: [Self; 24] = [
        Self::Unarmed,
        Self::Staff,
        Self::Axe,
        Self::Scepter,
        Self::Pickaxe,
        Self::SwordDagger,
        Self::Scimitar,
        Self::TwoHSword,
        Self::Mace,
        Self::Claws,
        Self::WarhammerMaul,
        Self::Whip,
        Self::Flowers,
        Self::MudPie,
        Self::Spear,
        Self::Halberd,
        Self::Bow,
        Self::Crossbow,
        Self::ThrownWeapons,
        Self::Chinchompa,
        Self::FixedDevice,
        Self::Salamander,
        Self::Scythe,
        Self::IvandisFlail,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Unarmed => "UNARMED",
            Self::Staff => "STAFF",
            Self::Axe => "AXE",
            Self::Scepter => "SCEPTER",
            Self::Pickaxe => "PICKAXE",
            Self::SwordDagger => "SWORD_DAGGER",
            Self::Scimitar => "SCIMITAR",
            Self::TwoHSword => "TWO_H_SWORD",
            Self::Mace => "MACE",
            Self::Claws => "CLAWS",
            Self::WarhammerMaul => "WARHAMMER_MAUL",
            Self::Whip => "WHIP",
            Self::Flowers => "FLOWERS",
            Self::MudPie => "MUD_PIE",
            Self::Spear => "SPEAR",
            Self::Halberd => "HALBERD",
            Self::Bow => "BOW",
            Self::Crossbow => "CROSSBOW",
            Self::ThrownWeapons => "THROWN_WEAPONS",
            Self::Chinchompa => "CHINCHOMPA",
            Self::FixedDevice => "FIXED_DEVICE",
            Self::Salamander => "SALAMANDER",
            Self::Scythe => "SCYTHE",
            Self::IvandisFlail => "IVANDIS_FLAIL",
        }
    }

    pub fn interface_id(self) -> i32 {
        match self {
            Self::Unarmed => 92,
            Self::Staff => 90,
            Self::Axe => 75,
            Self::Scepter | Self::IvandisFlail => 85,
            Self::Pickaxe => 83,
            Self::SwordDagger => 89,
            Self::Scimitar => 81,
            Self::TwoHSword => 82,
            Self::Mace => 88,
            Self::Claws => 78,
            Self::WarhammerMaul | Self::Flowers => 76,
            Self::Whip => 93,
            Self::MudPie | Self::ThrownWeapons => 91,
            Self::Spear => 87,
            Self::Halberd => 84,
            Self::Bow => 77,
            Self::Crossbow => 79,
            Self::Chinchompa => 473,
            Self::FixedDevice => 80,
            Self::Salamander => 474,
            Self::Scythe => 86,
        }
    }

    pub fn attack_styles(self) -> &'static [AttackStyle] {
        const CRUSH3: [AttackStyle; 3] = [s!(0, 2), s!(1, 2), s!(3, 2)];
        const RANGE3: [AttackStyle; 3] = [s!(4, 4), s!(5, 4), s!(6, 4)];
        const SLASH_CRUSH: [AttackStyle; 4] = [s!(0, 1), s!(1, 1), s!(1, 2), s!(3, 1)];
        const SLASH_STAB: [AttackStyle; 4] = [s!(0, 1), s!(1, 1), s!(2, 0), s!(3, 1)];
        match self {
            Self::Unarmed
            | Self::Scepter
            | Self::WarhammerMaul
            | Self::Flowers
            | Self::IvandisFlail => &CRUSH3,
            Self::Staff => &[s!(0, 2), s!(1, 2), s!(3, 2), s!(7, 3), s!(8, 3)],
            Self::Axe | Self::TwoHSword => &SLASH_CRUSH,
            Self::Pickaxe => &[s!(0, 0), s!(1, 0), s!(1, 2), s!(3, 0)],
            Self::SwordDagger => &[s!(0, 0), s!(1, 0), s!(1, 1), s!(3, 0)],
            Self::Scimitar | Self::Claws => &SLASH_STAB,
            Self::Mace => &[s!(0, 2), s!(1, 2), s!(2, 0), s!(3, 2)],
            Self::Whip => &[s!(0, 1), s!(2, 1), s!(3, 1)],
            Self::MudPie | Self::Bow | Self::Crossbow | Self::ThrownWeapons | Self::Chinchompa => {
                &RANGE3
            }
            Self::Spear => &[s!(2, 0), s!(2, 1), s!(2, 2), s!(3, 0)],
            Self::Halberd => &[s!(2, 0), s!(1, 1), s!(3, 0)],
            Self::FixedDevice => &[s!(4, 4), s!(1, 2)],
            Self::Salamander => &[s!(1, 1), s!(4, 4), s!(7, 3)],
            Self::Scythe => &[s!(0, 1), s!(1, 0), s!(1, 2), s!(3, 1)],
        }
    }
}

/// A `Graphics` record as the handlers pass it to `Entity.graphics`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Graphics {
    pub id: i32,
    pub height: i32,
    pub delay: i32,
}

/// `ArmourSet`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmourSet {
    Ahrim,
    Dharok,
    Guthan,
    Karil,
    Torag,
    Verac,
    BerserkerNecklaceSets,
}

impl ArmourSet {
    pub const ALL: [Self; 7] = [
        Self::Ahrim,
        Self::Dharok,
        Self::Guthan,
        Self::Karil,
        Self::Torag,
        Self::Verac,
        Self::BerserkerNecklaceSets,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Ahrim => "AHRIM",
            Self::Dharok => "DHAROK",
            Self::Guthan => "GUTHAN",
            Self::Karil => "KARIL",
            Self::Torag => "TORAG",
            Self::Verac => "VERAC",
            Self::BerserkerNecklaceSets => "BERSERKER_NECKLACE_SETS",
        }
    }

    pub fn end_graphic(self) -> Option<Graphics> {
        let id = match self {
            Self::Ahrim => 401,
            Self::Guthan => 398,
            Self::Karil => 400,
            Self::Torag => 399,
            Self::Dharok | Self::Verac | Self::BerserkerNecklaceSets => return None,
        };
        Some(Graphics {
            id,
            height: 96,
            delay: 0,
        })
    }

    /// One row per worn piece; any id of a row satisfies it.
    pub fn sets(self) -> &'static [&'static [i32]] {
        match self {
            Self::Ahrim => &[
                &[4708, 4856, 4857, 4858, 4859],
                &[4710, 4862, 4863, 4864, 4865],
                &[4712, 4868, 4869, 4870, 4871],
                &[4714, 4874, 4875, 4876, 4877],
            ],
            Self::Dharok => &[
                &[4716, 4880, 4881, 4882, 4883],
                &[4718, 4886, 4887, 4888, 4889],
                &[4720, 4892, 4893, 4894, 4895],
                &[4722, 4898, 4899, 4900, 4901],
            ],
            Self::Guthan => &[
                &[4724, 4904, 4905, 4906, 4907],
                &[4726, 4910, 4911, 4912, 4913],
                &[4728, 4916, 4917, 4918, 4919],
                &[4730, 4922, 4923, 4924, 4925],
            ],
            Self::Karil => &[
                &[4732, 4928, 4929, 4930, 4931],
                &[4734, 4934, 4935, 4936, 4937],
                &[4736, 4940, 4941, 4942, 4943],
                &[4738, 4946, 4947, 4948, 4949],
            ],
            Self::Torag => &[
                &[4745, 4952, 4953, 4954, 4955],
                &[4747, 4958, 4959, 4960, 4961],
                &[4749, 4964, 4965, 4966, 4967],
                &[4751, 4970, 4971, 4972, 4973],
            ],
            Self::Verac => &[
                &[4753, 4976, 4977, 4978, 4979],
                &[4755, 4982, 4983, 4984, 4985],
                &[4757, 4988, 4989, 4990, 4991],
                &[4759, 4994, 4995, 4996, 4997],
            ],
            Self::BerserkerNecklaceSets => &[&[11128], &[6523, 6525, 6527, 6528]],
        }
    }

    /// The NPC id that always counts as wearing the set.
    fn brother(self) -> Option<i32> {
        match self {
            Self::Ahrim => Some(2025),
            Self::Dharok => Some(2026),
            Self::Guthan => Some(2027),
            Self::Karil => Some(2028),
            Self::Torag => Some(2029),
            Self::Verac => Some(2030),
            Self::BerserkerNecklaceSets => None,
        }
    }

    /// `ArmourSet.isUsing`. The per-set `effect` bodies are not ported.
    pub fn is_using<H: Host>(self, h: &mut H, e: Option<&H::Entity>) -> Result<bool> {
        let Some(e) = e else { return Ok(false) };
        let kind = h.kind(e);
        if let Some(id) = self.brother()
            && kind.is_npc()
            && h.id(e)? == id
        {
            return Ok(true);
        }
        if !kind.is_player() {
            return Ok(false);
        }
        let mut hits = 0;
        for row in self.sets() {
            for &id in *row {
                if h.equipment_contains_one(e, id)? {
                    hits += 1;
                    break;
                }
            }
        }
        Ok(hits == self.sets().len())
    }
}

/// `Weapon.WeaponType`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponType {
    Default,
    Crossbow,
    DoubleShot,
    Thrown,
    Degrading,
    Staff,
    Chinchompa,
}

impl WeaponType {
    pub const ALL: [Self; 7] = [
        Self::Default,
        Self::Crossbow,
        Self::DoubleShot,
        Self::Thrown,
        Self::Degrading,
        Self::Staff,
        Self::Chinchompa,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Crossbow => "CROSSBOW",
            Self::DoubleShot => "DOUBLE_SHOT",
            Self::Thrown => "THROWN",
            Self::Degrading => "DEGRADING",
            Self::Staff => "STAFF",
            Self::Chinchompa => "CHINCHOMPA",
        }
    }
}

/// `BoltEffect`, in declaration order. Only the base `canFire`/`impact` bodies,
/// which `DIAMOND` inherits and `RangeSwingHandler.swing` calls, are ported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoltEffect {
    Opal,
    Jade,
    Pearl,
    Topaz,
    Sapphire,
    Emerald,
    Ruby,
    Diamond,
    Dragon,
    Onyx,
}

impl BoltEffect {
    pub const ALL: [Self; 10] = [
        Self::Opal,
        Self::Jade,
        Self::Pearl,
        Self::Topaz,
        Self::Sapphire,
        Self::Emerald,
        Self::Ruby,
        Self::Diamond,
        Self::Dragon,
        Self::Onyx,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Opal => "OPAL",
            Self::Jade => "JADE",
            Self::Pearl => "PEARL",
            Self::Topaz => "TOPAZ",
            Self::Sapphire => "SAPPHIRE",
            Self::Emerald => "EMERALD",
            Self::Ruby => "RUBY",
            Self::Diamond => "DIAMOND",
            Self::Dragon => "DRAGON",
            Self::Onyx => "ONYX",
        }
    }

    pub fn item_id(self) -> i32 {
        9236 + self as i32
    }

    pub fn graphics(self) -> Graphics {
        let (id, height) = match self {
            Self::Opal => (749, 0),
            Self::Jade => (755, 0),
            Self::Pearl => (750, 0),
            Self::Topaz => (757, 0),
            Self::Sapphire => (759, 100),
            Self::Emerald => (752, 0),
            Self::Ruby => (754, 0),
            Self::Diamond => (758, 0),
            Self::Dragon => (756, 0),
            Self::Onyx => (753, 0),
        };
        Graphics {
            id,
            height,
            delay: 0,
        }
    }

    pub fn sound(self) -> i32 {
        match self {
            Self::Opal => 2918,
            Self::Jade => 2916,
            Self::Pearl => 2920,
            Self::Topaz => 2914,
            Self::Sapphire => 2912,
            Self::Emerald => 2919,
            Self::Ruby => 2911,
            Self::Diamond => 2913,
            Self::Dragon => 2915,
            Self::Onyx => 2917,
        }
    }

    /// `BoltEffect.forId`.
    pub fn for_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|effect| effect.item_id() == id)
    }

    /// The base `canFire`: one draw, then the slayer-requirement veto.
    pub fn can_fire_base<H: Host>(h: &mut H, state: &BattleState<H>) -> Result<bool> {
        let roll_success = random(h, 13)? == 5;
        let Some(victim) = state.victim.as_ref().filter(|v| h.kind(v).is_npc()) else {
            return Ok(roll_success);
        };
        let Some(attacker) = state.entity.as_ref().filter(|a| h.kind(a).is_player()) else {
            return Ok(roll_success);
        };
        if h.npc_task(victim)?.is_none() {
            return Ok(roll_success);
        }
        let task = h.npc_task(victim)?.ok_or(Error::Null)?;
        if task.level_req > h.level(attacker, skill::SLAYER)? {
            return Ok(false);
        }
        Ok(roll_success)
    }

    /// The base `impact`: the sound at each player's location, then the graphic.
    pub fn impact_base<H: Host>(self, h: &mut H, state: &BattleState<H>) -> Result<()> {
        let victim = state.victim.as_ref();
        if let Some(attacker) = state.entity.as_ref().filter(|a| h.kind(a).is_player()) {
            let location = h.location(attacker)?.ok_or(Error::Null)?;
            h.play_global_audio(location, self.sound())?;
        }
        if let Some(victim) = victim.filter(|v| h.kind(v).is_player()) {
            let location = h.location(victim)?.ok_or(Error::Null)?;
            h.play_global_audio(location, self.sound())?;
        }
        h.graphics(victim.ok_or(Error::Null)?, self.graphics())
    }
}

/// A `RangeWeapon` row. Its attack animation is not read by this slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeWeapon {
    pub item_id: i32,
    pub attack_speed: i32,
    pub ammunition_slot: i32,
    pub weapon_type: i32,
    pub drop_ammo: bool,
    pub ammunition: Vec<i32>,
}

impl RangeWeapon {
    /// `getWeaponType`: indexes `WeaponType.values()` with the stored number.
    pub fn kind(&self) -> Result<WeaponType> {
        usize::try_from(self.weapon_type)
            .ok()
            .and_then(|index| WeaponType::ALL.get(index).copied())
            .ok_or(Error::Index)
    }
}

/// An `Ammunition` row. Its graphics and projectile are not read by this slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ammunition {
    pub item_id: i32,
    pub poison_damage: i32,
    pub effect: Option<BoltEffect>,
}

/// The two static lookup maps `RangeWeapon.get` and `Ammunition.get` read.
/// Loading them from the ranged configuration files is not part of this slice.
#[derive(Clone, Debug, Default)]
pub struct RangeData {
    pub weapons: std::collections::HashMap<i32, Arc<RangeWeapon>>,
    pub ammunition: std::collections::HashMap<i32, Arc<Ammunition>>,
}

impl RangeData {
    pub fn range_weapon(&self, id: i32) -> Option<Arc<RangeWeapon>> {
        self.weapons.get(&id).cloned()
    }

    pub fn ammunition(&self, id: i32) -> Option<Arc<Ammunition>> {
        self.ammunition.get(&id).cloned()
    }
}

/// The fields of a slayer `Tasks` constant the handlers read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlayerTask {
    pub ids: Vec<i32>,
    pub level_req: i32,
    pub dragon: bool,
    pub undead: bool,
    /// Whether the constant is `Tasks.KALPHITES`.
    pub kalphites: bool,
}

/// The skillcape perks the handlers query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perk {
    PrecisionStrikes,
    AccurateMarksman,
}

/// The object types a world supplies. Handles keep their own identity and
/// lifetime; a retained [`BattleState`] holds them, never a numeric index.
pub trait Types {
    type Entity: Clone;
    type Item: Clone;
    type Spell: Clone;
}

/// Everything the swing handlers reach outside their own classes. Each
/// fallible method is one original call; a failure leaves earlier effects.
pub trait Host: Types + Sized {
    fn kind(&self, entity: &Self::Entity) -> Kind;
    fn same_spell(&self, a: &Self::Spell, b: &Self::Spell) -> bool;
    fn item_id(&self, item: &Self::Item) -> i32;
    fn item_name(&self, item: &Self::Item) -> String;
    fn item_amount(&self, item: &Self::Item) -> i32;
    /// `RangeWeapon.get(id)`: the world's static range weapon map.
    fn range_weapon(&self, id: i32) -> Option<Arc<RangeWeapon>>;
    /// `Ammunition.get(id)`: the world's static ammunition map.
    fn ammunition(&self, id: i32) -> Option<Arc<Ammunition>>;

    /// `properties.bonuses`; `None` is a null array.
    fn bonuses(&mut self, entity: &Self::Entity) -> Result<Option<Vec<i32>>>;
    fn attack_style(&mut self, entity: &Self::Entity) -> Result<Option<AttackStyle>>;
    fn armour_set(&mut self, entity: &Self::Entity) -> Result<Option<ArmourSet>>;
    /// `properties.combatPulse.getVictim()`.
    fn pulse_victim(&mut self, entity: &Self::Entity) -> Result<Option<Self::Entity>>;
    fn stop_combat_pulse(&mut self, entity: &Self::Entity) -> Result<()>;
    fn update_combat_style(&mut self, entity: &Self::Entity) -> Result<()>;
    fn spell(&mut self, entity: &Self::Entity) -> Result<Option<Self::Spell>>;
    fn clear_spell(&mut self, entity: &Self::Entity) -> Result<()>;
    fn autocast_spell(&mut self, entity: &Self::Entity) -> Result<Option<Self::Spell>>;
    /// `skills.getLevel(slot)`.
    fn level(&mut self, entity: &Self::Entity, slot: i32) -> Result<i32>;
    /// `skills.getLevel(slot, discardAssist)`.
    fn level_with(&mut self, entity: &Self::Entity, slot: i32, discard_assist: bool)
    -> Result<i32>;
    fn static_level(&mut self, entity: &Self::Entity, slot: i32) -> Result<i32>;
    fn lifepoints(&mut self, entity: &Self::Entity) -> Result<i32>;
    fn maximum_lifepoints(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `prayer.getSkillBonus(slot)`.
    fn prayer_skill_bonus(&mut self, entity: &Self::Entity, slot: i32) -> Result<f64>;
    /// `prayer.get(PROTECT_FROM_SUMMONING)`.
    fn protects_from_summoning(&mut self, entity: &Self::Entity) -> Result<bool>;
    fn has_protection_prayer(&mut self, entity: &Self::Entity, style: CombatStyle) -> Result<bool>;
    fn perk_active(&mut self, entity: &Self::Entity, perk: Perk) -> Result<bool>;
    fn wearing_void(&mut self, entity: &Self::Entity, style: CombatStyle) -> Result<bool>;
    /// `getName()`; `None` is a null name.
    fn name(&mut self, entity: &Self::Entity) -> Result<Option<String>>;
    fn id(&mut self, entity: &Self::Entity) -> Result<i32>;
    fn size(&mut self, entity: &Self::Entity) -> Result<i32>;
    fn location(&mut self, entity: &Self::Entity) -> Result<Option<[i32; 3]>>;
    /// `entity.getLevelMod(entity, victim)`.
    fn level_mod(&mut self, entity: &Self::Entity, victim: Option<&Self::Entity>) -> Result<f64>;
    /// `equipment.get(slot)`.
    fn equipment(&mut self, entity: &Self::Entity, slot: i32) -> Result<Option<Self::Item>>;
    /// `equipment.getNew(slot)`: the item or a fresh id-0 placeholder.
    fn equipment_new(&mut self, entity: &Self::Entity, slot: i32) -> Result<Self::Item>;
    /// `equipment.containsAtLeastOneItem(id)`.
    fn equipment_contains_one(&mut self, entity: &Self::Entity, id: i32) -> Result<bool>;
    /// The item definition's configured `bonuses` array.
    fn item_bonuses(&mut self, item: &Self::Item) -> Result<Option<Vec<i32>>>;
    fn slayer_task(&mut self, entity: &Self::Entity) -> Result<Option<SlayerTask>>;
    /// `SlayerEquipmentFlags.getDamAccBonus`.
    fn slayer_bonus(&mut self, entity: &Self::Entity) -> Result<f64>;
    fn in_equipment(&mut self, entity: &Self::Entity, id: i32, amount: i32) -> Result<bool>;
    /// `familiarManager?.familiar` and its pouch id; `None` covers a null
    /// manager and a null familiar. The handlers swallow a failure here.
    fn familiar_pouch(&mut self, entity: &Self::Entity) -> Result<Option<i32>>;
    /// `NPC.getTask()`.
    fn npc_task(&mut self, entity: &Self::Entity) -> Result<Option<SlayerTask>>;
    /// `NPC.getDefinition().getCombatDistance()`.
    fn npc_combat_distance(&mut self, entity: &Self::Entity) -> Result<i32>;
    fn send_message(&mut self, entity: &Self::Entity, text: &str) -> Result<()>;
    /// `log(origin, Log.ERR, text)`.
    fn log_error(&mut self, origin: &'static str, text: &str) -> Result<()>;
    /// `playGlobalAudio(location, id)` with its default delay, loops and radius.
    fn play_global_audio(&mut self, location: [i32; 3], id: i32) -> Result<()>;
    fn graphics(&mut self, entity: &Self::Entity, graphics: Graphics) -> Result<()>;
    /// `getExtension(WeaponInterface.class) != null`.
    fn weapon_interface(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `WeaponInterface.selectAutoSpell`.
    fn select_auto_spell(
        &mut self,
        entity: &Self::Entity,
        button: i32,
        adjust_attack_style: bool,
        forget_autocast: bool,
    ) -> Result<()>;
    fn spell_meets_requirements(
        &mut self,
        spell: &Self::Spell,
        caster: &Self::Entity,
        message: bool,
        remove: bool,
    ) -> Result<bool>;
    fn spell_maximum_impact(
        &mut self,
        spell: &Self::Spell,
        entity: &Self::Entity,
        victim: Option<&Self::Entity>,
        state: &BattleState<Self>,
    ) -> Result<i32>;
    /// `spell.getTargets(entity, victim)`; `None` is a null array.
    #[allow(clippy::type_complexity)]
    fn spell_targets(
        &mut self,
        spell: &Self::Spell,
        entity: &Self::Entity,
        victim: Option<&Self::Entity>,
    ) -> Result<Option<Vec<Option<BattleState<Self>>>>>;
    /// `spell.getType() === SpellType.BLITZ`.
    fn spell_is_blitz(&mut self, spell: &Self::Spell) -> Result<bool>;
    /// One `Math.random()` draw.
    fn math_random(&mut self) -> Result<f64>;
    /// One `RandomFunction.RANDOM.nextInt(bound)` draw; `bound` is positive.
    fn next_int(&mut self, bound: i32) -> Result<i32>;
}

/// `Weapon`. The constructor ignores its type argument, so `kind` stays unset
/// until `RangeSwingHandler` assigns it.
pub struct Weapon<T: Types> {
    pub id: i32,
    pub item: Option<T::Item>,
    pub name: String,
    pub ammunition_slot: i32,
    pub ammunition: Option<T::Item>,
    pub kind: Option<WeaponType>,
}

impl<H: Host> Weapon<H> {
    /// `Weapon(item)`.
    pub fn new(h: &H, item: Option<H::Item>) -> Self {
        Self::with_ammunition(h, item, -1, None)
    }

    /// `Weapon(item, ammunitionSlot, ammunition)`.
    pub fn with_ammunition(
        h: &H,
        item: Option<H::Item>,
        ammunition_slot: i32,
        ammunition: Option<H::Item>,
    ) -> Self {
        Self {
            id: item.as_ref().map_or(-1, |i| h.item_id(i)),
            name: item
                .as_ref()
                .map_or_else(|| "null".to_owned(), |i| h.item_name(i)),
            item,
            ammunition_slot,
            ammunition,
            kind: None,
        }
    }
}

/// One element of `BattleState.targets`.
pub enum Target<T: Types> {
    /// The array holds the state that owns it.
    This,
    State(Box<BattleState<T>>),
}

/// `BattleState`. An owned value: `Target::This` is the only alias it can
/// express, which is the only one the ported methods distinguish.
pub struct BattleState<T: Types> {
    pub entity: Option<T::Entity>,
    pub victim: Option<T::Entity>,
    pub estimated_hit: i32,
    pub maximum_hit: i32,
    pub secondary_hit: i32,
    pub recoil_damage: i32,
    pub poison_damage: i32,
    pub frozen: bool,
    pub targets: Option<Vec<Option<Target<T>>>>,
    pub weapon: Option<Weapon<T>>,
    pub range_weapon: Option<Arc<RangeWeapon>>,
    pub ammunition: Option<Arc<Ammunition>>,
    pub spell: Option<T::Spell>,
    pub style: Option<CombatStyle>,
    pub armour_effect: Option<ArmourSet>,
}

impl<T: Types> Default for BattleState<T> {
    /// `BattleState()`.
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl<T: Types> BattleState<T> {
    /// `BattleState(entity, victim)`.
    pub fn new(entity: Option<T::Entity>, victim: Option<T::Entity>) -> Self {
        Self {
            entity,
            victim,
            estimated_hit: 0,
            maximum_hit: 0,
            secondary_hit: -1,
            recoil_damage: -1,
            poison_damage: -1,
            frozen: false,
            targets: None,
            weapon: None,
            range_weapon: None,
            ammunition: None,
            spell: None,
            style: None,
            armour_effect: None,
        }
    }

    /// `neutralizeHits`.
    pub fn neutralize_hits(&mut self) {
        if self.estimated_hit > 0 {
            self.estimated_hit = 0;
        }
        if self.secondary_hit > 0 {
            self.secondary_hit = 0;
        }
    }

    /// `getTotalDamage`: this state and its direct targets other than itself.
    pub fn total_damage(&self) -> i32 {
        let own = |s: &Self| s.estimated_hit.max(0).wrapping_add(s.secondary_hit.max(0));
        let mut hit = own(self);
        for target in self.targets.iter().flatten().flatten() {
            if let Target::State(state) = target {
                hit = hit.wrapping_add(own(state));
            }
        }
        hit
    }
}

/// `Location.getDistance`.
pub fn distance(first: [i32; 3], second: [i32; 3]) -> f64 {
    let x = first[0].wrapping_sub(second[0]);
    let y = first[1].wrapping_sub(second[1]);
    f64::from(x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y))).sqrt()
}

/// `RandomFunction.random(maxValue)`: no draw unless the bound is positive.
fn random<H: Host>(h: &mut H, max_value: i32) -> Result<i32> {
    if max_value <= 0 {
        return Ok(0);
    }
    h.next_int(max_value)
}

/// `RandomFunction.roll(chance)`.
fn roll<H: Host>(h: &mut H, chance: i32) -> Result<bool> {
    if chance <= 1 {
        return Ok(true);
    }
    Ok(random(h, chance)? == 1)
}

/// `CombatSwingHandler.getDragonfireMessage`.
pub fn dragonfire_message(protection: i32, fire_name: &str) -> String {
    if protection & 0x4 != 0 {
        if protection & 0x2 != 0 {
            return format!(
                "Your potion and shield fully protects you from the dragon's {fire_name}."
            );
        }
        return format!("Your shield absorbs most of the dragon's {fire_name}.");
    }
    if protection & 0x2 != 0 {
        return format!(
            "Your antifire potion helps you defend the against the dragon's {fire_name}."
        );
    }
    if protection & 0x8 != 0 {
        return format!("Your magic prayer absorbs some of the dragon's {fire_name}.");
    }
    format!("You are horribly burnt by the dragon's {fire_name}.")
}

/// `MeleeSwingHandler.checkUndead`: case-sensitive names and fragments.
pub fn check_undead(name: &str) -> bool {
    name == "Aberrant spectre"
        || name == "Ankou"
        || name.contains("anshee")
        || name == "Crawling Hand"
        || name == "Ghost"
        || name == "Ghast"
        || name == "Mummy"
        || name == "Tarn"
        || name.contains("Revenant")
        || name.contains("Shade")
        || name.contains("keleton")
        || name == "Skogre"
        || name == "Tortured Soul"
        || name.contains("Undead")
        || name == "Zogre"
        || name.contains("ombie")
}

/// Which of the three concrete handler classes an instance is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandlerKind {
    Melee,
    Range,
    Magic,
}

/// A swing handler instance: its class, mutable `type` and mutable flag list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handler {
    pub kind: HandlerKind,
    /// `CombatSwingHandler.type`.
    pub style: Option<CombatStyle>,
    /// `CombatSwingHandler.flags`: a list, so duplicates and order are kept.
    pub flags: Vec<Flag>,
    /// A subclass whose only override is `calculateDefence` answering
    /// `CombatStyle.X.swingHandler.calculateDefence(victim, attacker)`
    /// (MixedSwingHandlers.kt): the class of that singleton. `None` for the
    /// three classes themselves.
    pub defence_of: Option<HandlerKind>,
}

impl Handler {
    /// The class constructor with its vararg flags.
    pub fn new(kind: HandlerKind, flags: &[Flag]) -> Self {
        Self {
            kind,
            style: Some(match kind {
                HandlerKind::Melee => CombatStyle::Melee,
                HandlerKind::Range => CombatStyle::Range,
                HandlerKind::Magic => CombatStyle::Magic,
            }),
            flags: flags.to_vec(),
            defence_of: None,
        }
    }

    /// A MixedSwingHandlers.kt object: the `kind` class whose
    /// `calculateDefence` is the `defence_of` singleton's.
    pub fn mixed(kind: HandlerKind, defence_of: HandlerKind) -> Self {
        Self {
            defence_of: Some(defence_of),
            ..Self::new(kind, &[])
        }
    }

    fn has(&self, flag: Flag) -> bool {
        self.flags.contains(&flag)
    }

    /// `properties.attackStyle.style`: one read per original expression.
    fn style_of<H: Host>(h: &mut H, e: &H::Entity) -> Result<i32> {
        Ok(h.attack_style(e)?.ok_or(Error::Null)?.style)
    }

    /// `properties.bonuses[index] + 64` after both operands were read.
    fn bonus(bonuses: Option<Vec<i32>>, index: i32) -> Result<i32> {
        let bonuses = bonuses.ok_or(Error::Null)?;
        let value = usize::try_from(index)
            .ok()
            .and_then(|i| bonuses.get(i))
            .ok_or(Error::Index)?;
        Ok(value.wrapping_add(64))
    }

    /// The `+3` defensive/long-range or `+1` controlled stance of a defender.
    fn defensive_stance<H: Host>(h: &mut H, victim: &H::Entity, level: &mut f64) -> Result<()> {
        if Self::style_of(h, victim)? == STYLE_DEFENSIVE
            || Self::style_of(h, victim)? == STYLE_LONG_RANGE
        {
            *level += 3.0;
        } else if Self::style_of(h, victim)? == STYLE_CONTROLLED {
            *level += 1.0;
        }
        Ok(())
    }

    /// The familiar read shared by the bonus helpers. A failing lookup is
    /// swallowed, as the original catches every exception around it.
    fn familiar<H: Host>(h: &mut H, e: &H::Entity) -> Option<i32> {
        if !h.kind(e).is_player() {
            return None;
        }
        h.familiar_pouch(e).ok().flatten()
    }

    /// The melee salve-amulet and slayer-task multipliers.
    fn melee_task_bonus<H: Host>(
        h: &mut H,
        e: &H::Entity,
        level: &mut f64,
        accuracy: bool,
    ) -> Result<()> {
        let victim = h.pulse_victim(e)?;
        let victim_name = match &victim {
            Some(v) => h.name(v)?.unwrap_or_else(|| "none".to_owned()),
            None => "none".to_owned(),
        };
        let amulet = h.equipment(e, slot::AMULET)?;
        let amulet_id = amulet.as_ref().map_or(0, |item| h.item_id(item));
        if (amulet_id == SALVE_AMULET || amulet_id == SALVE_AMULET_E) && check_undead(&victim_name)
        {
            *level *= if amulet_id == SALVE_AMULET { 1.15 } else { 1.2 };
            return Ok(());
        }
        let on_task = match h.slayer_task(e)? {
            Some(task) => {
                let victim_id = match h.pulse_victim(e)? {
                    Some(v) => h.id(&v)?,
                    None => 0,
                };
                task.ids.contains(&victim_id)
                    || (task.kalphites && victim_id == KALPHITE_QUEEN_PHASE_ONE)
            }
            None => false,
        };
        if on_task {
            *level *= h.slayer_bonus(e)?;
            if accuracy
                && h.slayer_task(e)?.is_some_and(|task| task.dragon)
                && h.in_equipment(e, DRAGON_SLAYER_GLOVES, 1)?
            {
                *level *= 1.1;
            }
        }
        Ok(())
    }

    /// `calculateAccuracy`.
    pub fn calculate_accuracy<H: Host>(
        &self,
        h: &mut H,
        entity: Option<&H::Entity>,
    ) -> Result<i32> {
        let Some(e) = entity else { return Ok(0) };
        let kind = h.kind(e);
        let bonuses = h.bonuses(e)?;
        let index = match self.kind {
            HandlerKind::Magic => BONUS_MAGIC,
            _ => h.attack_style(e)?.ok_or(Error::Null)?.bonus_type,
        };
        let style_bonus = Self::bonus(bonuses, index)?;
        let slot = match self.kind {
            HandlerKind::Melee => skill::ATTACK,
            HandlerKind::Range => skill::RANGE,
            HandlerKind::Magic => skill::MAGIC,
        };
        if kind.is_npc() {
            return Ok(h.level(e, slot)?.wrapping_add(9).wrapping_mul(style_bonus));
        }
        if !kind.is_player() {
            return Ok(0);
        }
        let mut level = f64::from(match self.kind {
            HandlerKind::Magic => h.level_with(e, slot, true)?,
            _ => h.level(e, slot)?,
        });
        if !self.has(Flag::IgnorePrayerBoostsAccuracy) {
            level = (level + h.prayer_skill_bonus(e, slot)? * level).floor();
        }
        match self.kind {
            HandlerKind::Melee => {
                if Self::style_of(h, e)? == STYLE_ACCURATE {
                    level += 3.0;
                } else if Self::style_of(h, e)? == STYLE_CONTROLLED {
                    level += 1.0;
                }
                level += 8.0;
                if h.perk_active(e, Perk::PrecisionStrikes)? {
                    level += 6.0;
                }
                level *= self.set_multiplier(h, Some(e), slot)?;
            }
            HandlerKind::Range => {
                if Self::style_of(h, e)? == STYLE_RANGE_ACCURATE {
                    level += 3.0;
                }
                level += 8.0;
                level += Self::familiar_offence_bonus(h, e)?;
                level *= self.set_multiplier(h, Some(e), slot)?;
                if h.perk_active(e, Perk::AccurateMarksman)? {
                    level *= 1.1;
                }
            }
            HandlerKind::Magic => {
                level += 8.0;
                level *= self.set_multiplier(h, Some(e), slot)?;
            }
        }
        level = level.floor();
        if self.has(Flag::IgnoreStatBoostsAccuracy) {
            level *= 64.0;
        } else {
            level *= f64::from(style_bonus);
        }
        if self.kind == HandlerKind::Melee {
            Self::melee_task_bonus(h, e, &mut level, true)?;
        }
        Ok(level as i32)
    }

    /// `calculateHit`.
    pub fn calculate_hit<H: Host>(
        &self,
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
        modifier: f64,
    ) -> Result<i32> {
        if self.kind == HandlerKind::Magic {
            return Self::magic_hit(h, entity, victim, modifier);
        }
        let Some(e) = entity else { return Ok(0) };
        let kind = h.kind(e);
        let melee = self.kind == HandlerKind::Melee;
        let mut style_bonus = Self::bonus(h.bonuses(e)?, if melee { 11 } else { 14 })?;
        let slot = if melee { skill::STRENGTH } else { skill::RANGE };
        if kind.is_npc() {
            let level = h.level(e, slot)?.wrapping_add(9);
            let scaled = f64::from(level.wrapping_mul(style_bonus)) / 640.0;
            return Ok(((0.5 + scaled).floor() * modifier) as i32);
        }
        if !kind.is_player() {
            return Ok(0);
        }
        if !melee && h.equipment(e, slot::WEAPON)?.is_some() {
            let weapon = h.equipment(e, slot::WEAPON)?.ok_or(Error::Null)?;
            let range_weapon = h.range_weapon(h.item_id(&weapon)).ok_or(Error::Null)?;
            if range_weapon.ammunition_slot != slot::ARROWS
                && h.equipment(e, slot::ARROWS)?.is_some()
            {
                let arrows = h.equipment(e, slot::ARROWS)?.ok_or(Error::Null)?;
                let configured = h.item_bonuses(&arrows)?.ok_or(Error::Null)?;
                style_bonus = style_bonus.wrapping_sub(*configured.get(14).ok_or(Error::Index)?);
            }
        }
        let mut level = f64::from(h.level(e, slot)?);
        if !melee && self.has(Flag::IgnoreStatReduction) {
            let static_level = f64::from(h.static_level(e, slot)?);
            if static_level > level {
                level = static_level;
            }
        }
        if !self.has(Flag::IgnorePrayerBoostsDamage) {
            level = (level + h.prayer_skill_bonus(e, slot)? * level).floor();
        }
        if melee {
            if Self::style_of(h, e)? == STYLE_AGGRESSIVE {
                level += 3.0;
            } else if Self::style_of(h, e)? == STYLE_CONTROLLED {
                level += 1.0;
            }
            level += 8.0;
        } else {
            if Self::style_of(h, e)? == STYLE_RANGE_ACCURATE {
                level += 3.0;
            }
            level += 8.0;
            level += Self::familiar_offence_bonus(h, e)?;
        }
        level *= self.set_multiplier(h, Some(e), slot)?;
        level = level.floor();
        if self.has(Flag::IgnoreStatBoostsDamage) {
            level *= 64.0;
        } else {
            level *= f64::from(style_bonus);
        }
        if melee {
            Self::melee_task_bonus(h, e, &mut level, false)?;
        }
        Ok(((0.5 + level / 640.0).floor() * modifier) as i32)
    }

    /// `MagicSwingHandler.calculateHit`: the modifier is the spell's base damage.
    fn magic_hit<H: Host>(
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
        modifier: f64,
    ) -> Result<i32> {
        let mut base_damage = modifier;
        let kind = entity.map(|e| h.kind(e));
        if base_damage < 2.0 || kind.is_some_and(Kind::is_npc) {
            if base_damage > 2.0 {
                base_damage = 1.0;
            }
            let e = entity.ok_or(Error::Null)?;
            let level = h.level_with(e, skill::MAGIC, true)?;
            let index = if h.kind(e).is_player() { 14 } else { 13 };
            let bonuses = h.bonuses(e)?.ok_or(Error::Null)?;
            let bonus = *bonuses.get(index).ok_or(Error::Index)?;
            let strength = f64::from(level);
            let scaled =
                (14.0 + strength + f64::from(bonus) / 8.0 + strength * f64::from(bonus) * 0.016865)
                    * base_damage;
            return Ok(1i32.wrapping_add((scaled as i32) / 10));
        }
        let e = entity.ok_or(Error::Null)?;
        let mut level_mod = 1.0;
        let entity_mod = h.level_mod(e, victim)?;
        if entity_mod != 0.0 {
            level_mod += entity_mod;
        }
        Ok(((base_damage * level_mod) as i32).wrapping_add(1))
    }

    /// `calculateDefence`.
    pub fn calculate_defence<H: Host>(
        &self,
        h: &mut H,
        victim: Option<&H::Entity>,
        attacker: Option<&H::Entity>,
    ) -> Result<i32> {
        if let Some(kind) = self.defence_of {
            return Self::new(kind, &[]).calculate_defence(h, victim, attacker);
        }
        let (Some(v), Some(a)) = (victim, attacker) else {
            return Ok(0);
        };
        let kind = h.kind(v);
        let bonuses = h.bonuses(v)?;
        let index = match self.kind {
            HandlerKind::Magic => BONUS_MAGIC + 5,
            _ => h
                .attack_style(a)?
                .ok_or(Error::Null)?
                .bonus_type
                .wrapping_add(5),
        };
        let style_bonus = Self::bonus(bonuses, index)?;
        if kind.is_npc() {
            let slot = if self.kind == HandlerKind::Magic {
                skill::MAGIC
            } else {
                skill::DEFENCE
            };
            return Ok(h.level(v, slot)?.wrapping_add(9).wrapping_mul(style_bonus));
        }
        if !kind.is_player() {
            return Ok(0);
        }
        let mut level = f64::from(h.level(v, skill::DEFENCE)?);
        level = (level + h.prayer_skill_bonus(v, skill::DEFENCE)? * level).floor();
        Self::defensive_stance(h, v, &mut level)?;
        match self.kind {
            HandlerKind::Melee => {
                level += 8.0;
                level *= self.set_multiplier(h, Some(v), skill::DEFENCE)?;
                level *= match Self::familiar(h, v) {
                    Some(IRON_TITAN_POUCH) => 1.10,
                    Some(STEEL_TITAN_POUCH) => 1.15,
                    _ => 1.0,
                };
            }
            HandlerKind::Range => {
                level += 8.0;
                level *= self.set_multiplier(h, Some(v), skill::DEFENCE)?;
            }
            HandlerKind::Magic => {
                level *= self.set_multiplier(h, Some(v), skill::DEFENCE)?;
                let mut magic = f64::from(h.level(v, skill::MAGIC)?);
                magic = (magic + h.prayer_skill_bonus(v, skill::MAGIC)? * magic).floor();
                level = level * 0.3 + magic * 0.7 + 8.0;
                level *= match Self::familiar(h, v) {
                    Some(WOLPERTINGER_POUCH) => 1.05,
                    _ => 1.0,
                };
            }
        }
        Ok((level as i32).wrapping_mul(style_bonus))
    }

    /// `RangeSwingHandler.familiarOffenceBonus`.
    fn familiar_offence_bonus<H: Host>(h: &mut H, e: &H::Entity) -> Result<f64> {
        match Self::familiar(h, e) {
            Some(GEYSER_TITAN_POUCH) => {
                Ok(1.0 + (f64::from(h.level(e, skill::RANGE)?) * 0.03).ceil())
            }
            _ => Ok(0.0),
        }
    }

    /// `getSetMultiplier`.
    pub fn set_multiplier<H: Host>(
        &self,
        h: &mut H,
        e: Option<&H::Entity>,
        slot: i32,
    ) -> Result<f64> {
        match self.kind {
            HandlerKind::Melee => {
                let e = e.ok_or(Error::Null)?;
                if h.armour_set(e)? == Some(ArmourSet::Dharok) && slot == skill::STRENGTH {
                    let missing = h.maximum_lifepoints(e)?.wrapping_sub(h.lifepoints(e)?);
                    return Ok(1.0 + f64::from(missing) * 0.01);
                }
                let player = h.kind(e).is_player();
                let mut multiplier = 1.0;
                if player
                    && h.wearing_void(e, CombatStyle::Melee)?
                    && (slot == skill::ATTACK || slot == skill::STRENGTH)
                {
                    multiplier += 0.1;
                }
                if player
                    && h.armour_set(e)? == Some(ArmourSet::BerserkerNecklaceSets)
                    && slot == skill::STRENGTH
                {
                    multiplier += 0.2;
                }
                Ok(multiplier)
            }
            HandlerKind::Range | HandlerKind::Magic => {
                let (own, style, value) = if self.kind == HandlerKind::Range {
                    (skill::RANGE, CombatStyle::Range, 1.2)
                } else {
                    (skill::MAGIC, CombatStyle::Magic, 1.3)
                };
                if slot == own
                    && let Some(e) = e
                    && h.kind(e).is_player()
                    && h.wearing_void(e, style)?
                {
                    return Ok(value);
                }
                Ok(1.0)
            }
        }
    }

    /// `getArmourSet`: the first set, in the class's own order, that is worn.
    pub fn armour_set<H: Host>(
        &self,
        h: &mut H,
        e: Option<&H::Entity>,
    ) -> Result<Option<ArmourSet>> {
        let order: &[ArmourSet] = match self.kind {
            HandlerKind::Melee => &[
                ArmourSet::Dharok,
                ArmourSet::Guthan,
                ArmourSet::Verac,
                ArmourSet::BerserkerNecklaceSets,
                ArmourSet::Torag,
            ],
            HandlerKind::Range => &[ArmourSet::Karil],
            HandlerKind::Magic => &[ArmourSet::Ahrim],
        };
        for &set in order {
            if set.is_using(h, e)? {
                return Ok(Some(set));
            }
        }
        Ok(None)
    }

    /// The probability the original compares one `Math.random()` draw with.
    /// `None` when it answers `false` without drawing (no victim or style).
    pub fn hit_chance<H: Host>(
        &self,
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
        style: Option<CombatStyle>,
        accuracy_mod: f64,
        defence_mod: f64,
    ) -> Result<Option<f64>> {
        let mut modifier = 1.0;
        let (Some(v), Some(_)) = (victim, style) else {
            return Ok(None);
        };
        if h.kind(v).is_player()
            && entity.is_some_and(|e| h.kind(e) == Kind::Familiar)
            && h.protects_from_summoning(v)?
        {
            modifier = 0.0;
        }
        let attack = f64::from(self.calculate_accuracy(h, entity)?) * accuracy_mod * modifier;
        let defence = f64::from(self.calculate_defence(h, victim, entity)?) * defence_mod;
        Ok(Some(if attack > defence {
            1.0 - ((defence + 2.0) / (2.0 * (attack + 1.0)))
        } else {
            attack / (2.0 * (defence + 1.0))
        }))
    }

    /// `isAccurateImpact(entity, victim, style, accuracyMod, defenceMod)`.
    pub fn is_accurate_impact<H: Host>(
        &self,
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
        style: Option<CombatStyle>,
        accuracy_mod: f64,
        defence_mod: f64,
    ) -> Result<bool> {
        match self.hit_chance(h, entity, victim, style, accuracy_mod, defence_mod)? {
            Some(chance) => Ok(h.math_random()? < chance),
            None => Ok(false),
        }
    }

    /// `isAccurateImpact(entity, victim)`: the handler's current `type`.
    pub fn is_accurate_impact_typed<H: Host>(
        &self,
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
    ) -> Result<bool> {
        self.is_accurate_impact(h, entity, victim, self.style, 1.0, 1.0)
    }

    /// `getCombatDistance`. Both entities are non-null parameters.
    pub fn combat_distance<H: Host>(
        &self,
        h: &mut H,
        e: Option<&H::Entity>,
        v: Option<&H::Entity>,
        raw_distance: i32,
    ) -> Result<i32> {
        let (Some(e), Some(v)) = (e, v) else {
            return Err(Error::Null);
        };
        let mut distance = raw_distance;
        if h.kind(e).is_npc() && h.npc_combat_distance(e)? > 0 {
            distance = h.npc_combat_distance(e)?;
        }
        Ok((h.size(e)? >> 1)
            .wrapping_add(h.size(v)? >> 1)
            .wrapping_add(distance))
    }

    /// `formatHit`. The victim is a non-null parameter.
    pub fn format_hit<H: Host>(
        &self,
        h: &mut H,
        victim: Option<&H::Entity>,
        raw_hit: i32,
    ) -> Result<i32> {
        let victim = victim.ok_or(Error::Null)?;
        let mut hit = raw_hit;
        if hit < 1 {
            return Ok(hit);
        }
        if hit > h.lifepoints(victim)? {
            hit = h.lifepoints(victim)?;
        }
        Ok(hit)
    }

    /// The two lifepoint caps every `swing` ends with.
    fn cap_to_lifepoints<H: Host>(
        h: &mut H,
        victim: &H::Entity,
        state: &mut BattleState<H>,
    ) -> Result<()> {
        if state.estimated_hit > h.lifepoints(victim)? {
            state.estimated_hit = h.lifepoints(victim)?;
        }
        if state.estimated_hit.wrapping_add(state.secondary_hit) > h.lifepoints(victim)? {
            let secondary = state.secondary_hit;
            let excess = state
                .estimated_hit
                .wrapping_add(state.secondary_hit)
                .wrapping_sub(h.lifepoints(victim)?);
            state.secondary_hit = secondary.wrapping_sub(excess);
        }
        Ok(())
    }

    /// `swing`: fills the state and returns the ticks before impact, or `-1`
    /// when the swing does not happen. `range` is only read by the range class.
    pub fn swing<H: Host>(
        &mut self,
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
        state: &mut BattleState<H>,
    ) -> Result<i32> {
        match self.kind {
            HandlerKind::Melee => self.melee_swing(h, entity, victim, state),
            HandlerKind::Range => self.range_swing(h, entity, victim, state),
            HandlerKind::Magic => self.magic_swing(h, entity, victim, state),
        }
    }

    fn melee_swing<H: Host>(
        &self,
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
        state: &mut BattleState<H>,
    ) -> Result<i32> {
        let mut hit = 0;
        state.style = Some(CombatStyle::Melee);
        if let Some(e) = entity.filter(|e| h.kind(e).is_player()) {
            let item = h.equipment(e, slot::WEAPON)?;
            state.weapon = Some(Weapon::new(h, item));
        }
        let e = entity.ok_or(Error::Null)?;
        if h.armour_set(e)? == Some(ArmourSet::Verac) && roll(h, 4)? {
            state.armour_effect = Some(ArmourSet::Verac);
        }
        if state.armour_effect == Some(ArmourSet::Verac)
            || self.is_accurate_impact(h, entity, victim, Some(CombatStyle::Melee), 1.0, 1.0)?
        {
            let mut max = self.calculate_hit(h, entity, victim, 1.0)?;
            if let Some(v) = victim
                && h.kind(e).is_npc()
                && state.armour_effect == Some(ArmourSet::Verac)
                && h.has_protection_prayer(v, CombatStyle::Melee)?
            {
                max = max.wrapping_mul(2) / 3;
            }
            state.maximum_hit = max;
            hit = random(h, max.wrapping_add(1))?;
        }
        state.estimated_hit = hit;
        if let Some(v) = victim {
            Self::cap_to_lifepoints(h, v, state)?;
        }
        Ok(1)
    }

    /// `RangeSwingHandler.configureRangeData`.
    pub fn configure_range_data<H: Host>(
        &self,
        h: &mut H,
        entity: Option<&H::Entity>,
        state: &mut BattleState<H>,
    ) -> Result<()> {
        state.style = Some(CombatStyle::Range);
        let weapon = if let Some(e) = entity.filter(|e| h.kind(e).is_player()) {
            let held = h.equipment_new(e, slot::WEAPON)?;
            let Some(range_weapon) = h.range_weapon(h.item_id(&held)) else {
                let again = h.equipment_new(e, slot::WEAPON)?;
                let text = format!(
                    "Unhandled range weapon used - [item id={}].",
                    h.item_id(&again)
                );
                h.log_error("RangeSwingHandler", &text)?;
                return Ok(());
            };
            let item = h.equipment(e, slot::WEAPON)?;
            let ammunition = h.equipment_new(e, range_weapon.ammunition_slot)?;
            let ammunition_id = h.item_id(&ammunition);
            let mut weapon =
                Weapon::with_ammunition(h, item, range_weapon.ammunition_slot, Some(ammunition));
            weapon.kind = Some(range_weapon.kind()?);
            state.range_weapon = Some(range_weapon);
            state.ammunition = h.ammunition(ammunition_id);
            weapon
        } else {
            Weapon::new(h, None)
        };
        state.weapon = Some(weapon);
        Ok(())
    }

    /// `RangeSwingHandler.hasAmmo`.
    pub fn has_ammo<H: Host>(
        h: &mut H,
        e: Option<&H::Entity>,
        state: &mut BattleState<H>,
    ) -> Result<bool> {
        let Some(e) = e.filter(|e| h.kind(e).is_player()) else {
            return Ok(true);
        };
        loop {
            let weapon = state.weapon.as_ref().ok_or(Error::Null)?;
            let kind = weapon.kind;
            let amount = if kind == Some(WeaponType::DoubleShot) {
                2
            } else {
                1
            };
            if kind == Some(WeaponType::Degrading) {
                return Ok(true);
            }
            let item = h.equipment(e, weapon.ammunition_slot)?;
            if let Some(item) = item.filter(|item| h.item_amount(item) >= amount) {
                let accepted = state.range_weapon.as_ref().ok_or(Error::Null)?;
                if !accepted.ammunition.contains(&h.item_id(&item)) {
                    h.send_message(e, "You can't use this type of ammunition with this bow.")?;
                    return Ok(false);
                }
                return Ok(true);
            }
            if kind == Some(WeaponType::DoubleShot) {
                state.weapon.as_mut().ok_or(Error::Null)?.kind = Some(WeaponType::Default);
                continue;
            }
            h.send_message(e, "You do not have enough ammo left.")?;
            return Ok(false);
        }
    }

    fn range_swing<H: Host>(
        &mut self,
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
        state: &mut BattleState<H>,
    ) -> Result<i32> {
        self.configure_range_data(h, entity, state)?;
        if state.weapon.is_none() || !Self::has_ammo(h, entity, state)? {
            h.stop_combat_pulse(entity.ok_or(Error::Null)?)?;
            return Ok(-1);
        }
        let mut hit = 0;
        let effect = state.ammunition.as_ref().and_then(|ammo| ammo.effect);
        let armour_pierce =
            effect == Some(BoltEffect::Diamond) && BoltEffect::can_fire_base(h, state)?;
        if armour_pierce
            || self.is_accurate_impact(h, entity, victim, Some(CombatStyle::Range), 1.0, 1.0)?
        {
            let max = if armour_pierce {
                BoltEffect::Diamond.impact_base(h, state)?;
                self.flags.push(Flag::IgnoreStatReduction);
                let max = (f64::from(self.calculate_hit(h, entity, victim, 1.0)?) * 1.15) as i32;
                if let Some(at) = self
                    .flags
                    .iter()
                    .position(|flag| *flag == Flag::IgnoreStatReduction)
                {
                    self.flags.remove(at);
                }
                max
            } else {
                self.calculate_hit(h, entity, victim, 1.0)?
            };
            state.maximum_hit = max;
            hit = random(h, max.wrapping_add(1))?;
        }
        state.estimated_hit = hit;
        if state.weapon.as_ref().ok_or(Error::Null)?.kind == Some(WeaponType::DoubleShot) {
            if self.is_accurate_impact(h, entity, victim, Some(CombatStyle::Range), 1.0, 1.0)? {
                let max = self.calculate_hit(h, entity, victim, 1.0)?;
                hit = random(h, max.wrapping_add(1))?;
            }
            state.secondary_hit = hit;
        }
        let Some(e) = entity else { return Ok(-1) };
        let v = victim.ok_or(Error::Null)?;
        if h.location(v)?.is_none() {
            return Ok(-1);
        }
        Self::cap_to_lifepoints(h, v, state)?;
        // Both locations are read before either null is noticed.
        let from = h.location(e)?;
        let to = h.location(v)?;
        let span = distance(from.ok_or(Error::Null)?, to.ok_or(Error::Null)?);
        Ok(1i32.wrapping_add((span * 0.3).ceil() as i32))
    }

    fn magic_swing<H: Host>(
        &self,
        h: &mut H,
        entity: Option<&H::Entity>,
        victim: Option<&H::Entity>,
        state: &mut BattleState<H>,
    ) -> Result<i32> {
        let e = entity.ok_or(Error::Null)?;
        let spell = match h.spell(e)? {
            Some(spell) => spell,
            None => match h.autocast_spell(e)? {
                Some(spell) => spell,
                None => return Ok(-1),
            },
        };
        state.style = Some(CombatStyle::Magic);
        if !h.spell_meets_requirements(&spell, e, true, true)? {
            h.clear_spell(e)?;
            if h.weapon_interface(e)? {
                h.select_auto_spell(e, -1, true, true)?;
                h.update_combat_style(e)?;
            }
            return Ok(-1);
        }
        let base = h.spell_maximum_impact(&spell, e, victim, state)?;
        let max = self.calculate_hit(h, entity, victim, f64::from(base))?;
        let targets = h.spell_targets(&spell, e, victim)?;
        state.targets = targets.map(|list| {
            list.into_iter()
                .map(|target| target.map(|s| Target::State(Box::new(s))))
                .collect()
        });
        state.spell = Some(spell.clone());
        let count = state.targets.as_ref().ok_or(Error::Null)?.len();
        for index in 0..count {
            let mut hit = -1;
            let target_victim = {
                let target = Self::target(state, index)?;
                target.spell = Some(spell.clone());
                target.victim.clone()
            };
            let style = Some(CombatStyle::Magic);
            let accurate =
                self.is_accurate_impact(h, entity, target_victim.as_ref(), style, 1.0, 1.0)?;
            if accurate {
                Self::target(state, index)?.maximum_hit = max;
                hit = random(h, max)?;
            }
            let target = Self::target(state, index)?;
            target.style = style;
            target.estimated_hit = hit;
        }
        if h.spell(e)?
            .is_some_and(|current| h.same_spell(&spell, &current))
        {
            h.clear_spell(e)?;
            if h.autocast_spell(e)?.is_none() {
                h.stop_combat_pulse(e)?;
            }
        }
        // The caster's location is read before a missing victim is noticed, and
        // both locations before either null is.
        let from = h.location(e)?;
        let v = victim.ok_or(Error::Null)?;
        let to = h.location(v)?;
        let span = distance(from.ok_or(Error::Null)?, to.ok_or(Error::Null)?);
        let mut ticks = 2i32.wrapping_add((span * 0.5).ceil() as i32);
        if h.spell_is_blitz(&spell)? {
            ticks = ticks.wrapping_add(1);
        }
        Self::cap_to_lifepoints(h, v, state)?;
        Ok(ticks)
    }

    /// The state behind `targets[index]`; a null element is a null dereference.
    fn target<T: Types>(state: &mut BattleState<T>, index: usize) -> Result<&mut BattleState<T>> {
        match state
            .targets
            .as_mut()
            .and_then(|targets| targets.get_mut(index))
            .ok_or(Error::Null)?
        {
            Some(Target::State(target)) => Ok(target),
            // A spell cannot hand back the state it was not given.
            Some(Target::This) | None => Err(Error::Null),
        }
    }
}
