//! NPC types the server names, read on demand as `NpcTypeList.get` reads
//! them: from the held definitions, else from the open cache, else (prepared
//! packs only) the defaults when the pack's catalog is complete. A type that
//! cannot be had is reported once and its NPC stays in the update state with
//! nothing drawn; it never ends the session (`rs09_client::on_demand`).
use super::Workbench;
use rs09_client::{
    npc::Definition,
    npc_updates::Metadata,
    on_demand::{self, Found},
};
use rs09_protocol::game::players::RawAppearance;
use std::borrow::Cow;

fn children(definition: &Definition) -> Vec<i32> {
    definition.children.clone().unwrap_or_default()
}
/// The session's diagnostics list keeps each message once and at most 32.
fn note(diagnostics: &mut Vec<String>, message: String) {
    if diagnostics.len() < 32 && !diagnostics.contains(&message) {
        diagnostics.push(message);
    }
}
fn defaults() -> Result<Definition, String> {
    // `new NpcType()` then `postDecode()`: nothing read.
    Definition::decode(&[0]).map_err(|e| e.to_string())
}

impl Workbench {
    fn npc_catalog_complete(&self) -> bool {
        self.pack.get("npc-catalog-complete").is_ok()
    }
    /// [`on_demand::find`] for one NPC type, without keeping what was read.
    fn npc_lookup(&self, id: u32) -> Result<Found<Definition>, String> {
        let cache = self
            .world_cache
            .as_ref()
            .map(|cache| move || cache.npc_file(id));
        on_demand::find(
            self.npc_definitions.contains_key(&id),
            cache,
            self.npc_catalog_complete(),
        )
    }
    /// The type as the update decoder needs it, read but not kept (the
    /// decoder holds the world only for reading; the update pass that
    /// follows keeps it, [`Self::ensure_npc_type`]). A type not to be had
    /// gives the defaults `NpcTypeList.get` keeps for a type without data.
    pub(super) fn npc_type(&self, id: u32) -> Result<Cow<'_, Definition>, String> {
        Ok(match self.npc_lookup(id) {
            Ok(Found::Held) => Cow::Borrowed(&self.npc_definitions[&id]),
            Ok(Found::Read(definition)) => Cow::Owned(definition),
            _ => Cow::Owned(defaults()?),
        })
    }
    /// The update decoder's view of a type (`NpcList` reads size, BAS, turn
    /// speed and rotation from `NpcTypeList.get`).
    pub(super) fn npc_update_metadata(&self, id: i32) -> Result<Metadata, String> {
        let d = self.npc_type(id as u32)?;
        let direct = |d: &Definition| [0, 2, 3].into_iter().any(|i| d.sounds[i] != -1);
        let area_sound = match &d.children {
            None => direct(&d),
            Some(ids) => ids.iter().any(|&child| {
                u32::try_from(child)
                    .ok()
                    .and_then(|child| self.npc_type(child).ok())
                    .is_some_and(|d| direct(&d))
            }),
        };
        Ok(Metadata {
            size: d.size,
            bas: d.bas,
            turn_speed: d.turn_speed,
            rotates: d.rotates,
            area_sound,
        })
    }
    /// Reads and keeps type `id` and the types its morph table names.
    /// Returns whether `id` is now held, and the diagnostics to show: one per
    /// type the first time it cannot be had. A type still downloading is
    /// asked for again on the next call, unreported.
    pub(super) fn ensure_npc_type(&mut self, id: u32) -> (bool, Vec<String>) {
        if self.npc_definitions.contains_key(&id) && self.npc_definitions[&id].children.is_none() {
            return (true, Vec::new());
        }
        let gathered = on_demand::gather(
            id,
            |id| self.npc_definitions.get(&id).map(children),
            |id| self.npc_lookup(id),
            defaults,
            children,
        );
        self.npc_definitions.extend(gathered.add);
        let mut diagnostics = Vec::new();
        for missing in gathered.unknown {
            if self.on_demand.first("npc", i64::from(missing)) {
                diagnostics.push(format!(
                    "NPC{missing}: definition not in the prepared packs and no cache is open; it is not drawn"
                ));
            }
        }
        for (failed, error) in gathered.failed {
            if self.on_demand.first("npc", i64::from(failed)) {
                diagnostics.push(format!("NPC{failed}: {error}; it is not drawn"));
            }
        }
        for message in &diagnostics {
            eprintln!("{message}");
        }
        (self.npc_definitions.contains_key(&id), diagnostics)
    }
    /// Reads the NPC types the looks name (`PlayerAppearance.npcId`: a player
    /// shown as an NPC), adding first-time diagnostics to `diagnostics`.
    pub(super) fn ensure_look_types<'a>(
        &mut self,
        looks: impl IntoIterator<Item = &'a RawAppearance>,
        diagnostics: &mut Vec<String>,
    ) {
        for look in looks {
            if let Ok(id) = u32::try_from(look.npc_id) {
                let (_, missing) = self.ensure_npc_type(id);
                missing.into_iter().for_each(|m| note(diagnostics, m));
            }
        }
    }
    /// `player_form::size` with the look's NPC type read on demand. A type
    /// not to be had has the defaults' size, 1, as `NpcTypeList.get` gives a
    /// type it has no data for.
    pub(super) fn player_size(
        &mut self,
        raw: &RawAppearance,
        diagnostics: &mut Vec<String>,
    ) -> i32 {
        self.ensure_look_types([raw], diagnostics);
        rs09_client::player_form::size(raw, &self.npc_definitions).unwrap_or(1)
    }
    /// `player_form::appearance_position` with the looks' NPC types read on
    /// demand. When one is not to be had the position is left as it is (the
    /// player is not drawn in that look) and the session goes on.
    pub(super) fn player_appearance_position(
        &mut self,
        motion: &mut rs09_client::walking::Movement,
        previous: Option<&RawAppearance>,
        raw: &RawAppearance,
        diagnostics: &mut Vec<String>,
    ) {
        self.ensure_look_types(previous.into_iter().chain([raw]), diagnostics);
        let types = &self.npc_definitions;
        let known = |look: &RawAppearance| {
            u32::try_from(look.npc_id).map_or(true, |id| types.contains_key(&id))
        };
        if previous.is_none_or(known)
            && known(raw)
            && let Err(error) = rs09_client::player_form::appearance_position(
                motion,
                previous,
                raw,
                &self.npc_definitions,
            )
        {
            note(diagnostics, error);
        }
    }
}
