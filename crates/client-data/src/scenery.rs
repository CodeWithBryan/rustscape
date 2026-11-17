//! Client scene replacements from RT4 ChangeLocRequest. Gameplay outcomes come
//! from server updates; this queue only waits for assets and applies presentation.
use std::collections::BTreeMap;

/// Logical LoginManager rebuild phases precede any renderer availability check.
/// Ground lists keep their identities while old published views/supports belong
/// to the scene being replaced. Cache/terrain loading follows this operation.
pub fn rebuild_logical(
    rebuild: &rs09_protocol::game::rebuild::NormalRebuild,
    previous_base: &mut Option<[i32; 2]>,
    players: &mut rs09_protocol::game::players::State,
    ground: &mut rs09_protocol::game::ground::State,
    supports: &mut BTreeMap<rs09_protocol::game::ground::Tile, i32>,
) -> [i32; 2] {
    let base = rebuild.base();
    if let Some(old) = *previous_base {
        let delta = [base[0] - old[0], base[1] - old[1]];
        players.rebase(delta);
        ground.rebase_stacks(delta);
    } else {
        *players = rs09_protocol::game::players::State::new(0, 0);
    }
    players
        .self_player_mut()
        .teleport(rebuild.local_x, rebuild.local_y, false);
    players.plane = rebuild.plane;
    ground.plane = rebuild.plane;
    ground.clear_rendered();
    supports.clear();
    *previous_base = Some(base);
    base
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Slot {
    pub plane: usize,
    pub tile: [i32; 2],
    pub layer: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    pub id: i32,
    pub shape: u8,
    pub rotation: u8,
}
pub fn layer(shape: u8) -> Option<u8> {
    match shape {
        0..=3 => Some(0),
        4..=8 => Some(1),
        9..=21 => Some(2),
        22 => Some(3),
        _ => None,
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub slot: Slot,
    pub original: Location,
    pub replacement: Location,
    pub set_loops: i32,
    pub reset_loops: i32,
}
#[derive(Default)]
pub struct Scene {
    pub locations: BTreeMap<Slot, Location>,
    pub changes: Vec<Change>,
}
impl Scene {
    pub fn push(&mut self, slot: Slot, replacement: Location, set_loops: i32, reset_loops: i32) {
        if let Some(change) = self.changes.iter_mut().find(|c| c.slot == slot) {
            change.replacement = replacement;
            change.set_loops = set_loops;
            change.reset_loops = reset_loops;
        } else {
            let original = self.locations.get(&slot).copied().unwrap_or(Location {
                id: -1,
                shape: 0,
                rotation: 0,
            });
            self.changes.push(Change {
                slot,
                original,
                replacement,
                set_loops,
                reset_loops,
            });
        }
    }
    /// Tick in insertion order. Missing models defer a replacement/reversion.
    pub fn advance(
        &mut self,
        origin: [i32; 2],
        mut ready: impl FnMut(Location) -> bool,
    ) -> Vec<(Slot, Location)> {
        let mut applied = Vec::new();
        let mut i = 0;
        while i < self.changes.len() {
            let c = &mut self.changes[i];
            let mut remove = false;
            if c.reset_loops > 0 {
                c.reset_loops -= 1;
            }
            if c.reset_loops != 0 {
                if c.set_loops > 0 {
                    c.set_loops -= 1;
                }
                let local = [c.slot.tile[0] - origin[0], c.slot.tile[1] - origin[1]];
                if c.set_loops == 0
                    && local.iter().all(|n| (1..=102).contains(n))
                    && (c.replacement.id < 0 || ready(c.replacement))
                {
                    if c.replacement.id < 0 {
                        self.locations.remove(&c.slot);
                    } else {
                        self.locations.insert(c.slot, c.replacement);
                    }
                    applied.push((c.slot, c.replacement));
                    c.set_loops = -1;
                    remove = (c.original.id == c.replacement.id && c.original.id == -1)
                        || c.replacement == c.original;
                }
            } else if c.original.id < 0 || ready(c.original) {
                if c.original.id < 0 {
                    self.locations.remove(&c.slot);
                } else {
                    self.locations.insert(c.slot, c.original);
                }
                applied.push((c.slot, c.original));
                remove = true;
            }
            if remove {
                self.changes.remove(i);
            } else {
                i += 1;
            }
        }
        applied
    }
    pub fn flush(&mut self) {
        self.changes.retain_mut(|c| {
            if c.reset_loops != -1 {
                return false;
            }
            c.set_loops = 0;
            c.original = self.locations.get(&c.slot).copied().unwrap_or(Location {
                id: -1,
                shape: 0,
                rotation: 0,
            });
            true
        });
    }
}
