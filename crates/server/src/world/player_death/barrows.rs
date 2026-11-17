//! The parts of `BarrowsEquipment` a death uses: the 24 pieces with their six
//! degradation states (fully repaired, 100, 75, 50, 25, broken), the lookups
//! and `graveDeathDurabilityReduction`.
use crate::{
    definitions::item::{Registry, Shared},
    inventory::item::{self, Item},
};

/// `DEGRADATION_TICKS_PER_TIER`: fifteen hours of combat over four tiers.
pub const TICKS_PER_TIER: i32 = (15 * 6000) / 4;
const MAX_DURABILITY: i32 = TICKS_PER_TIER * 4;

/// `barrowsItemDefinitions` in declaration order: each piece's item ids from
/// fully repaired (index 0) to broken (index 5).
pub const PIECES: [[i32; 6]; 24] = [
    [4716, 4880, 4881, 4882, 4883, 4884],
    [4718, 4886, 4887, 4888, 4889, 4890],
    [4720, 4892, 4893, 4894, 4895, 4896],
    [4722, 4898, 4899, 4900, 4901, 4902],
    [4724, 4904, 4905, 4906, 4907, 4908],
    [4726, 4910, 4911, 4912, 4913, 4914],
    [4728, 4916, 4917, 4918, 4919, 4920],
    [4730, 4922, 4923, 4924, 4925, 4926],
    [4745, 4952, 4953, 4954, 4955, 4956],
    [4747, 4958, 4959, 4960, 4961, 4962],
    [4749, 4964, 4965, 4966, 4967, 4968],
    [4751, 4970, 4971, 4972, 4973, 4974],
    [4753, 4976, 4977, 4978, 4979, 4980],
    [4755, 4982, 4983, 4984, 4985, 4986],
    [4757, 4988, 4989, 4990, 4991, 4992],
    [4759, 4994, 4995, 4996, 4997, 4998],
    [4708, 4856, 4857, 4858, 4859, 4860],
    [4710, 4862, 4863, 4864, 4865, 4866],
    [4712, 4868, 4869, 4870, 4871, 4872],
    [4714, 4874, 4875, 4876, 4877, 4878],
    [4732, 4928, 4929, 4930, 4931, 4932],
    [4734, 4934, 4935, 4936, 4937, 4938],
    [4736, 4940, 4941, 4942, 4943, 4944],
    [4738, 4946, 4947, 4948, 4949, 4950],
];

/// `getDefinition(itemId)`: the piece's states. Each id belongs to one piece
/// (the original's map keeps the last piece for a repeated id; there is none).
pub fn piece(id: i32) -> Option<&'static [i32; 6]> {
    PIECES.iter().find(|states| states.contains(&id))
}
/// `isBarrowsItem(itemId)`.
pub fn is_barrows(id: i32) -> bool {
    piece(id).is_some()
}
/// `isFullyRepaired(itemId)`.
pub fn is_fully_repaired(id: i32) -> bool {
    piece(id).is_some_and(|states| states[0] == id)
}
/// `isBroken(itemId)`.
pub fn is_broken(id: i32) -> bool {
    piece(id).is_some_and(|states| states[5] == id)
}
/// `getDefinition(itemId).getBrokenId()`.
pub fn broken_id(id: i32) -> Option<i32> {
    piece(id).map(|states| states[5])
}

/// `calculateTotalRemainingDurability`.
fn remaining(states: &[i32; 6], id: i32, charge: i32) -> i32 {
    match states.iter().position(|state| *state == id) {
        None | Some(5) => 0,
        Some(0) => MAX_DURABILITY,
        Some(index) => {
            let tiers = 4 - index as i32;
            tiers.wrapping_mul(TICKS_PER_TIER).wrapping_add(charge)
        }
    }
}

/// `graveDeathDurabilityReduction(item)`: `None` for an item that is not a
/// Barrows piece, the same item when it is broken, otherwise the piece with
/// a fifth of its remaining durability taken away (the new item keeps the
/// amount; a broken result has the default charge). `Err(())` is the
/// original's index failure for a durability above every tier.
#[allow(clippy::result_unit_err)]
pub fn grave_death_durability_reduction(
    item: &item::Handle,
    definitions: &Registry,
) -> Result<Option<item::Handle>, ()> {
    let (id, amount, charge) = {
        let item = item.read();
        (item.id(), item.amount(), item.charge())
    };
    let Some(states) = piece(id) else {
        return Ok(None);
    };
    if is_broken(id) {
        return Ok(Some(item.clone()));
    }
    let total = remaining(states, id, charge);
    let reduction = (f64::from(total) * 0.2) as i32;
    let durability = total.wrapping_sub(reduction);
    if durability <= 0 {
        return Ok(Some(Shared::new(Item::new(
            states[5],
            amount,
            1000,
            definitions,
        ))));
    }
    let tier = durability.wrapping_sub(1) / TICKS_PER_TIER + 1;
    let index = 5 - tier;
    let new_id = *usize::try_from(index)
        .ok()
        .and_then(|index| states.get(index))
        .ok_or(())?;
    let new_charge = durability.wrapping_sub(1) % TICKS_PER_TIER + 1;
    Ok(Some(Shared::new(Item::new(
        new_id,
        amount,
        new_charge,
        definitions,
    ))))
}
