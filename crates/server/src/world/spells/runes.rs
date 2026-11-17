//! `Runes`, `CombinationRune`, `MagicStaff`, `Runes.isInfinite` and
//! `SpellUtils.usingStaff`/`hasRunes`.
use super::{Error, Host, Result, SpellType, Value};
use crate::world::repository::hash_map::JavaHashMap;

/// `Runes`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rune {
    Air,
    Water,
    Earth,
    Fire,
    Mind,
    Nature,
    Chaos,
    Death,
    Cosmic,
    Blood,
    Soul,
    Astral,
    Law,
    Body,
    SaradominStaff,
    GuthixStaff,
    ZamorakStaff,
    ZurielsStaff,
}

impl Rune {
    pub const ALL: [Self; 18] = [
        Self::Air,
        Self::Water,
        Self::Earth,
        Self::Fire,
        Self::Mind,
        Self::Nature,
        Self::Chaos,
        Self::Death,
        Self::Cosmic,
        Self::Blood,
        Self::Soul,
        Self::Astral,
        Self::Law,
        Self::Body,
        Self::SaradominStaff,
        Self::GuthixStaff,
        Self::ZamorakStaff,
        Self::ZurielsStaff,
    ];

    /// `getId()`.
    pub fn id(self) -> i32 {
        match self {
            Self::Air => 556,
            Self::Water => 555,
            Self::Earth => 557,
            Self::Fire => 554,
            Self::Mind => 558,
            Self::Nature => 561,
            Self::Chaos => 562,
            Self::Death => 560,
            Self::Cosmic => 564,
            Self::Blood => 565,
            Self::Soul => 566,
            Self::Astral => 9075,
            Self::Law => 563,
            Self::Body => 559,
            Self::SaradominStaff => 2415,
            Self::GuthixStaff => 2416,
            Self::ZamorakStaff => 2417,
            Self::ZurielsStaff => 13867,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Air => "AIR_RUNE",
            Self::Water => "WATER_RUNE",
            Self::Earth => "EARTH_RUNE",
            Self::Fire => "FIRE_RUNE",
            Self::Mind => "MIND_RUNE",
            Self::Nature => "NATURE_RUNE",
            Self::Chaos => "CHAOS_RUNE",
            Self::Death => "DEATH_RUNE",
            Self::Cosmic => "COSMIC_RUNE",
            Self::Blood => "BLOOD_RUNE",
            Self::Soul => "SOUL_RUNE",
            Self::Astral => "ASTRAL_RUNE",
            Self::Law => "LAW_RUNE",
            Self::Body => "BODY_RUNE",
            Self::SaradominStaff => "SARADOMIN_STAFF",
            Self::GuthixStaff => "GUTHIX_STAFF",
            Self::ZamorakStaff => "ZAMORAK_STAFF",
            Self::ZurielsStaff => "ZURIELS_STAFF",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.name() == name)
    }

    /// `Runes.forId(id)`: the first constant with that id.
    pub fn for_id(id: i32) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.id() == id)
    }
}

/// `Runes.isInfinite(rune, weapon, types)`. `weapon` is the item id or
/// `None` for null; `types` is `None` for a null array.
pub fn is_infinite(rune: Option<Rune>, weapon: Option<i32>, types: Option<&[SpellType]>) -> bool {
    let (Some(weapon), Some(rune)) = (weapon, rune) else {
        return false;
    };
    if let Some(types) = types {
        let only = |t: SpellType| types.len() == 1 && types[0] == t;
        if weapon == 2415 && rune == Rune::SaradominStaff && only(SpellType::GodStrike) {
            return true;
        }
        if weapon == 2416 && rune == Rune::GuthixStaff && only(SpellType::GodStrike) {
            return true;
        }
        if weapon == 13867
            && rune == Rune::ZurielsStaff
            && types.len() == 1
            && matches!(
                types[0],
                SpellType::Barrage | SpellType::Blitz | SpellType::Rush | SpellType::Burst
            )
        {
            return true;
        }
        if weapon == 2417 && rune == Rune::ZamorakStaff && only(SpellType::GodStrike) {
            return true;
        }
    }
    match rune {
        // Items.STAFF_OF_AIR_1381, AIR_BATTLESTAFF_1397 and so on.
        Rune::Air => weapon == 1381 || weapon == 1397,
        Rune::Water => weapon == 1383 || weapon == 1395,
        Rune::Earth => weapon == 1385 || weapon == 1399,
        Rune::Fire => weapon == 1387 || weapon == 1393,
        _ => false,
    }
}

/// `CombinationRune`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombinationRune {
    Lava,
    Steam,
    Mist,
    Dust,
    Smoke,
    Mud,
    Elemental,
    Catalytic,
}

impl CombinationRune {
    pub const ALL: [Self; 8] = [
        Self::Lava,
        Self::Steam,
        Self::Mist,
        Self::Dust,
        Self::Smoke,
        Self::Mud,
        Self::Elemental,
        Self::Catalytic,
    ];

    /// The public `id` field.
    pub fn id(self) -> i32 {
        match self {
            Self::Lava => 4699,
            Self::Steam => 4694,
            Self::Mist => 4695,
            Self::Dust => 4696,
            Self::Smoke => 4697,
            Self::Mud => 4698,
            Self::Elemental => 12850,
            Self::Catalytic => 12851,
        }
    }

    /// The public `types` field, in constructor order.
    pub fn types(self) -> &'static [Rune] {
        match self {
            Self::Lava => &[Rune::Fire, Rune::Earth],
            Self::Steam => &[Rune::Fire, Rune::Water],
            Self::Mist => &[Rune::Water, Rune::Air],
            Self::Dust => &[Rune::Air, Rune::Earth],
            Self::Smoke => &[Rune::Fire, Rune::Air],
            Self::Mud => &[Rune::Earth, Rune::Water],
            Self::Elemental => &[Rune::Air, Rune::Water, Rune::Earth, Rune::Fire],
            Self::Catalytic => &[
                Rune::Mind,
                Rune::Chaos,
                Rune::Death,
                Rune::Blood,
                Rune::Soul,
                Rune::Astral,
            ],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Lava => "LAVA_RUNE",
            Self::Steam => "STEAM_RUNE",
            Self::Mist => "MIST_RUNE",
            Self::Dust => "DUST_RUNE",
            Self::Smoke => "SMOKE_RUNE",
            Self::Mud => "MUD_RUNE",
            Self::Elemental => "ELEMENTAL_RUNE",
            Self::Catalytic => "CATALYTIC_RUNE",
        }
    }

    /// `eligibleFor(rune)`: once per matching type, so a combination that
    /// listed a rune twice would appear twice.
    pub fn eligible_for(rune: Option<Rune>) -> Vec<Self> {
        let mut out = Vec::new();
        for combo in Self::ALL {
            for t in combo.types() {
                if Some(*t) == rune {
                    out.push(combo);
                }
            }
        }
        out
    }
}

/// `MagicStaff`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagicStaff {
    FireRune,
    WaterRune,
    AirRune,
    EarthRune,
}

impl MagicStaff {
    pub const ALL: [Self; 4] = [
        Self::FireRune,
        Self::WaterRune,
        Self::AirRune,
        Self::EarthRune,
    ];

    pub fn rune_id(self) -> i32 {
        match self {
            Self::FireRune => 554,
            Self::WaterRune => 555,
            Self::AirRune => 556,
            Self::EarthRune => 557,
        }
    }

    pub fn staves(self) -> &'static [i32] {
        match self {
            Self::FireRune => &[1387, 1393, 1401, 3053, 3054, 3055, 3056, 11736, 11738],
            Self::WaterRune => &[1383, 1395, 1403, 6563, 11736, 11738, 6562],
            Self::AirRune => &[1381, 1397, 1405, 21777],
            Self::EarthRune => &[3053, 3054, 3055, 3056, 1385, 1399, 1407, 557, 6563, 6562],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::FireRune => "FIRE_RUNE",
            Self::WaterRune => "WATER_RUNE",
            Self::AirRune => "AIR_RUNE",
            Self::EarthRune => "EARTH_RUNE",
        }
    }

    /// `forId(runeId)`: the static map keyed by rune id (a later constant
    /// with the same rune id would replace an earlier one; none does).
    pub fn for_id(rune_id: i32) -> Option<Self> {
        Self::ALL
            .iter()
            .rev()
            .copied()
            .find(|s| s.rune_id() == rune_id)
    }
}

/// `MagicSpell.usingStaff(p, rune)` and `SpellUtils.usingStaff(p, rune)`:
/// the same body, one `equipment.get(3)`.
pub fn using_staff<H: Host>(h: &mut H, player: &H::Node, rune: i32) -> Result<bool> {
    let Some(weapon) = h.equipment(player, super::slot::WEAPON)? else {
        return Ok(false);
    };
    let Some(staff) = MagicStaff::for_id(rune) else {
        return Ok(false);
    };
    Ok(staff.staves().contains(&weapon))
}

/// The attribute `hasRunes` leaves behind and `meetsRequirements` removes.
pub const SPELL_RUNES: &str = "spell:runes";

/// `SpellUtils.hasRunes(p, runes)`: the first missing rune with the amount
/// still owed, or `None` after storing the removal list in `spell:runes`.
///
/// The cost is a `HashMap<Integer, Integer>` and the leftover walk follows
/// its table order, so both the missing rune reported and the order of the
/// removal list are the JDK's. Combination runes come first, in declaration
/// order, each taking up to the largest remaining cost of its types and
/// lowering every type by that usage; a combination with no stock still
/// adds an entry of amount 0.
pub fn has_runes<H: Host>(
    h: &mut H,
    player: &H::Node,
    runes: &[(i32, i32)],
) -> Result<Option<(i32, i32)>> {
    let mut cost: JavaHashMap<i32, i32> = JavaHashMap::new();
    for &(id, amount) in runes {
        if using_staff(h, player, id)? {
            continue;
        }
        let old = cost.get(&id).copied().unwrap_or(0);
        cost.put(id, old.wrapping_add(amount));
    }
    let mut remove = Vec::new();
    for combo in CombinationRune::ALL {
        let available = h.inventory_amount(player, combo.id())?;
        let usage = combo
            .types()
            .iter()
            .filter_map(|t| cost.get(&t.id()).copied())
            .max()
            .unwrap_or(0);
        if usage > 0 && available >= 0 {
            let usage = usage.min(available);
            remove.push((combo.id(), usage));
            for t in combo.types() {
                let old = cost.get(&t.id()).copied().unwrap_or(0);
                cost.put(t.id(), old.wrapping_sub(usage));
            }
        }
    }
    let entries: Vec<(i32, i32)> = cost.iter().map(|(k, v)| (*k, *v)).collect();
    for (id, amount) in entries {
        if amount <= 0 {
            continue;
        }
        let available = h.inventory_amount(player, id)?;
        if available < amount {
            return Ok(Some((id, amount)));
        }
        remove.push((id, amount));
    }
    h.set_attribute(player, SPELL_RUNES, Value::Items(remove))?;
    Ok(None)
}

/// `Value::Items` or the failing cast of another stored value.
pub(super) fn items(value: Value) -> Result<Vec<(i32, i32)>> {
    match value {
        Value::Items(items) => Ok(items),
        Value::Int(_) => Err(Error::Cast),
    }
}
