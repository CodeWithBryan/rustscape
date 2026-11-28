//! Fishing in the live world: `FishingListener`, the fishing spots' options
//! ("net", "lure", "bait", "harpoon", "cage", "fish") as an interaction
//! script of the player's ScriptProcessor (super::super::scripts). The body
//! is `world::gathering::fishing`'s, verified by the gathering corpus
//! (docs/gathering.md); its `service::Context` over the live world is
//! super::woodcutting's `Gather`, written without a corpus of its own.
//!
//! Answered through the native systems, beyond what woodcutting and mining
//! already ask: the spot's id, active flag and location, the
//! `interact:option` attribute, `inInventory`, `hasLevelDyn`,
//! `hasSpaceFor`, `removeItem` from the inventory (the bait),
//! `incrementAttribute`, `RandomFunction.RANDOM.nextDouble()` and the
//! world's dev-mode setting.
//!
//! Not ported, and answered by name (docs/live-world.md): the familiar
//! (none, so no forager acts and the boost is 0), SkillcapePerks
//! (inactive), the ResourceProducedEvent and the Fist of Guthix gloves'
//! charges (skipped). Barehand fishing's Fremennik diary task is a
//! refusal; nothing here sets the barbarian training attribute it needs.
//! The spots do not move (`FishingNPC` and `FishSpots` are not ported), and
//! the legacy FishingPulse and its option handlers are not installed.
use super::{
    super::{host::Live, scripts::Interaction},
    P,
    woodcutting::{NODE, PLAYER, Target},
};
use crate::{
    definitions::item::Shared,
    scripts,
    world::{
        gathering::fishing,
        interaction_listeners::{self as listeners},
        player_walk::NodeHandle,
    },
};

/// The listener's class (`scan.contentInterfaces`).
pub const LISTENER_CLASS: &str = "content.global.skill.gather.fishing.FishingListener";

impl Live {
    /// `FishingListener.defineListeners`: the spots' six options as one NPC
    /// interaction (persistent, distance 1) whose handler is
    /// [`Interaction::Fishing`].
    pub fn define_fishing_listener(&mut self) {
        let registration = fishing::registration();
        let options: Vec<Vec<u16>> = registration
            .options
            .iter()
            .map(|o| o.encode_utf16().collect())
            .collect();
        self.item_actions.listeners.add_metadata(
            &super::super::global::RootCasing,
            &registration.ids,
            listeners::Kind::Npc,
            &options,
            Shared::new(listeners::Metadata {
                function: Interaction::Fishing.function(),
                distance: registration.allowed_distance,
                persist: registration.persistent,
            }),
        );
    }

    /// `handleFishing(player, node, state)`: one execution of the
    /// interaction script, with the processor's target as the spot
    /// (`node as? NPC`: a target that is not an NPC clears the scripts).
    pub fn fishing_script(
        &mut self,
        p: &P,
        target: &NodeHandle,
        stage: i32,
    ) -> scripts::Result<bool> {
        if !self.inner.is_npc_node(target.node) {
            return Err(crate::world::script_runtime::error(
                "not ported in the live world: FishingListener's script on a target that is not an NPC",
            ));
        }
        let spot = Target::Npc(P(target.clone()));
        self.gather_at(p, spot, "fishing", |host| {
            fishing::handle_fishing(host, PLAYER, Some(NODE), stage)
        })
        .map_err(crate::world::script_runtime::error)
    }
}
