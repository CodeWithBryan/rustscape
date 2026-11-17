//! `BarrowsEquipment`: the 24 Barrows pieces with their brother, slot type,
//! name, repair cost and six degradation states (fully repaired, 100, 75,
//! 50, 25, broken), the lookups, `getRepairCost` and
//! `graveDeathDurabilityReduction`. The state table is the one
//! `world::player_death::barrows` already uses for a death.
use crate::world::combat_swing::{Error, Result};
use crate::world::dragonfire::ItemValue;
pub use crate::world::player_death::barrows::{PIECES, TICKS_PER_TIER};

/// `MAX_DURABILITY`.
pub const MAX_DURABILITY: i32 = TICKS_PER_TIER * 4;

/// One `BarrowsItemDefinition`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub brother: &'static str,
    pub equipment_type: &'static str,
    pub item_name: &'static str,
    pub base_repair_cost: i32,
    pub degradation_states: [i32; 6],
}

impl Piece {
    pub fn repaired_id(&self) -> i32 {
        self.degradation_states[0]
    }
    pub fn broken_id(&self) -> i32 {
        self.degradation_states[5]
    }
    /// `contains(itemId)`.
    pub fn contains(&self, id: i32) -> bool {
        self.degradation_states.contains(&id)
    }
    /// `getDegradationIndex(itemId)`: -1 when absent.
    pub fn degradation_index(&self, id: i32) -> i32 {
        self.degradation_states
            .iter()
            .position(|state| *state == id)
            .map_or(-1, |i| i as i32)
    }
}

const BROTHERS: [&str; 6] = ["dharok", "guthan", "torag", "verac", "ahrim", "karil"];
const TYPES: [&str; 4] = ["helm", "weapon", "body", "legs"];
const COSTS: [i32; 4] = [60_000, 100_000, 90_000, 80_000];
const NAMES: [&str; 24] = [
    "Dharok's helm",
    "Dharok's greataxe",
    "Dharok's platebody",
    "Dharok's platelegs",
    "Guthan's helm",
    "Guthan's warspear",
    "Guthan's platebody",
    "Guthan's chainskirt",
    "Torag's helm",
    "Torag's hammers",
    "Torag's platebody",
    "Torag's platelegs",
    "Verac's helm",
    "Verac's flail",
    "Verac's brassard",
    "Verac's plateskirt",
    "Ahrim's hood",
    "Ahrim's staff",
    "Ahrim's robetop",
    "Ahrim's robeskirt",
    "Karil's coif",
    "Karil's crossbow",
    "Karil's leathertop",
    "Karil's leatherskirt",
];

/// `barrowsItemDefinitions` in declaration order (brothers in the order
/// above, each helm, weapon, body, legs).
pub fn pieces() -> [Piece; 24] {
    std::array::from_fn(|i| Piece {
        brother: BROTHERS[i / 4],
        equipment_type: TYPES[i % 4],
        item_name: NAMES[i],
        base_repair_cost: COSTS[i % 4],
        degradation_states: PIECES[i],
    })
}

/// `getDefinition(itemId)`: each id belongs to one piece.
pub fn definition(id: i32) -> Option<Piece> {
    pieces().into_iter().find(|p| p.contains(id))
}

/// `getAllEquipmentSets()`: each piece's states, in declaration order.
pub fn all_equipment_sets() -> Vec<Vec<i32>> {
    PIECES.iter().map(|states| states.to_vec()).collect()
}

/// `getAllRepairableBarrowsIds()`: the definition map's keys (pieces in
/// order, each piece's states in order) that are not fully repaired.
pub fn all_repairable_ids() -> Vec<i32> {
    PIECES
        .iter()
        .flat_map(|states| states.iter().copied())
        .filter(|id| !is_fully_repaired(*id))
        .collect()
}

/// `isBarrowsItem(itemId)`.
pub fn is_barrows(id: i32) -> bool {
    definition(id).is_some()
}

/// `isFullyRepaired(itemId)`.
pub fn is_fully_repaired(id: i32) -> bool {
    definition(id).is_some_and(|p| p.repaired_id() == id)
}

/// `isBroken(itemId)`.
pub fn is_broken(id: i32) -> bool {
    definition(id).is_some_and(|p| p.broken_id() == id)
}

/// `calculateTotalRemainingDurability(item, def)` of an item's id and
/// (packed) charge.
pub fn remaining_durability(piece: &Piece, id: i32, charge: i32) -> i32 {
    match piece.degradation_index(id) {
        -1 => 0,
        0 => MAX_DURABILITY,
        5 => 0,
        index => (4 - index)
            .wrapping_mul(TICKS_PER_TIER)
            .wrapping_add(charge),
    }
}

/// `getRepairCost(item)`: the share of the base cost the lost durability
/// is, truncated; -1 for an item that is not a Barrows piece or is fully
/// repaired.
pub fn repair_cost(id: i32, charge: i32) -> i32 {
    let Some(piece) = definition(id) else {
        return -1;
    };
    if is_fully_repaired(id) {
        return -1;
    }
    let total = remaining_durability(&piece, id, charge);
    let lost = MAX_DURABILITY.wrapping_sub(total);
    ((f64::from(lost) / f64::from(MAX_DURABILITY)) * f64::from(piece.base_repair_cost)) as i32
}

/// What `graveDeathDurabilityReduction(item)` answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reduced {
    /// `null`: not a Barrows piece.
    None,
    /// The same item object: a broken piece.
    Same,
    /// A new `Item(id, amount, charge)`.
    New(ItemValue),
}

/// `graveDeathDurabilityReduction(item)`: a fifth of the remaining
/// durability taken away (`createItemFromDurability`; a broken result has the
/// default charge 1000).
pub fn grave_death_durability_reduction(id: i32, amount: i32, charge: i32) -> Result<Reduced> {
    let Some(piece) = definition(id) else {
        return Ok(Reduced::None);
    };
    if is_broken(id) {
        return Ok(Reduced::Same);
    }
    let total = remaining_durability(&piece, id, charge);
    let reduction = (f64::from(total) * 0.2) as i32;
    let durability = total.wrapping_sub(reduction);
    Ok(Reduced::New(item_from_durability(
        &piece, durability, amount,
    )?))
}

/// `createItemFromDurability(def, durability, amount)`.
pub fn item_from_durability(piece: &Piece, durability: i32, amount: i32) -> Result<ItemValue> {
    if durability <= 0 {
        return Ok((piece.broken_id(), amount, 1000));
    }
    let tier = durability.wrapping_sub(1) / TICKS_PER_TIER + 1;
    let index = 5 - tier;
    let id = usize::try_from(index)
        .ok()
        .and_then(|i| piece.degradation_states.get(i))
        .copied()
        .ok_or(Error::Index)?;
    let charge = durability.wrapping_sub(1) % TICKS_PER_TIER + 1;
    Ok((id, amount, charge))
}
