//! Spot animation types (`SpotAnimTypeList.get`) for what the server sends:
//! world spots (SPOTANIM_SPECIFIC) and actor spots. With a cache open the
//! whole archive is read once, as the original holds it after the start-up
//! sequence, and an id without a file has the defaults (no sequence). With
//! prepared packs only, an id the pack lacks places nothing and is reported
//! once (`rs09_client::on_demand`); the session goes on.
use super::{Workbench, world_cache};

impl Workbench {
    /// Reads every spot animation type of the cache; ids without a file get
    /// the defaults (sequence -1), as `SpotAnimTypeList.get` gives them.
    pub(super) fn attach_spot_catalog(&mut self, cache: &world_cache::Cache) -> Result<(), String> {
        for (id, data) in cache.spot_files()? {
            let Ok(id) = i32::try_from(id) else {
                continue;
            };
            if let std::collections::btree_map::Entry::Vacant(entry) =
                self.spot_definitions.entry(id)
            {
                let (definition, _) = rs09_client::spot::Definition::decode(&data)
                    .map_err(|e| format!("spot/{id}: {e}"))?;
                entry.insert(definition);
            }
        }
        for id in 0..65536 {
            let sequence = self.spot_definitions.get(&id).map_or(-1, |d| d.sequence);
            self.spot_sequences.entry(id).or_insert(sequence);
        }
        Ok(())
    }
    /// The world spots received since the last call whose type or sequence
    /// was not to be had, as diagnostics said once per id.
    pub(super) fn take_spot_notes(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        for id in std::mem::take(&mut self.world_spots.unavailable) {
            if self.on_demand.first("spot", i64::from(id)) {
                let note = format!(
                    "Spot{id}: definition or sequence not in the prepared packs and no cache is open; it is not drawn"
                );
                eprintln!("{note}");
                notes.push(note);
            }
        }
        notes
    }
}
