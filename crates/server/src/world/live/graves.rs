//! Graves in the live world: `world::player_death`'s `Grave`,
//! `GraveController` and `GraveType` over a grave NPC made at runtime
//! ([`super::runtime_npcs`]).
//!
//! - `new Grave()`: `AbstractNPC(GRAVESTONE_6571, Location.create(0, 0, 0),
//!   false)`, the plain NPC constructor (`setWalks(false)`), not
//!   `NPC.create`; the grave is kept by its NPC's node, which is how
//!   `node as? Grave` and the NPC tick find it.
//! - `GraveController.tick()` is a tick listener: it runs in
//!   `GameWorld.tickListeners`, after the world's pulses.
//! - `Grave.tick()` replaces `NPC.tick()` for the grave's NPC: the owner,
//!   online, gets the hint icon (ContentAPI `registerHintIcon`); there is no
//!   `Entity.tick` (no walking, no mask preparation).
//! - The read, bless, repair and demolish options are GraveController's
//!   `InteractionListener` (`on(GraveType.ids, NPC, ...)`), registered at
//!   boot with the item listeners (items.rs) and dispatched by identity to
//!   [`Live::grave_option`]; these listeners are not instant, and the
//!   movement pulse of a content listener is still refused by name, so a
//!   player choosing one gets that refusal before the body runs.
//! - `GraveController.hasGraveAt(loc)` answers DropListener over the active
//!   graves ([`Live::has_grave_at`]).
//!
//! Not ported here: the server store's grave archive (`PersistWorld`:
//! `serializeToServerStore`/`deserializeFromServerStore` are library code
//! the live world does not call: graves do not survive a restart),
//! `GravePurchaseInterface`, `updateGraveType` and the `forcegravedeath`
//! command.
use super::{P, host::Live};
use crate::world::{
    doors::Location,
    drop_tables::drop::{Error, Result},
    npc,
    player_death::{self as pd, grave},
    player_walk::{Node, NodeHandle},
};
use rs09_game_core::direction::Direction;

/// `NPCs.GRAVESTONE_6571`.
pub const GRAVESTONE: i32 = 6571;

fn binding(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

impl Live {
    /// `new Grave()`.
    pub fn new_grave_npc(&mut self) -> Result<grave::Handle> {
        let npc = self
            .npc_new(
                GRAVESTONE,
                Some(Location::new(0, 0, 0)),
                Some(Direction::South),
            )
            .map_err(binding)?;
        // AbstractNPC(id, location, false): super.setWalks(autowalk).
        self.inner
            .npcs
            .rows
            .get(&npc.node)
            .ok_or(Error::Null)?
            .brain
            .fields
            .write()
            .walks = false;
        let grave = crate::definitions::item::Shared::new(grave::Grave::new(npc.clone()));
        self.content.graves.insert(npc.node, grave.clone());
        Ok(grave)
    }

    /// The grave whose NPC is `node` (`node as? Grave`).
    pub fn grave_of(&self, node: Node) -> Option<grave::Handle> {
        self.content.graves.get(&node).cloned()
    }

    /// `GraveController.tick()` (a tick listener).
    pub fn graves_tick(&mut self) -> Result<()> {
        let mut controller = std::mem::take(&mut self.inner.combat.graves);
        let result = controller.tick(self);
        // A grave made while the controller was out (none is) would be lost:
        // the controller is only ever taken for the length of one call.
        self.inner.combat.graves = controller;
        self.drain_all();
        result.map_err(death_error)
    }

    /// `NPC.tick()` of a grave NPC: `Grave.tick()`. `None` when the NPC is
    /// not a grave.
    pub fn grave_npc_tick(&mut self, p: &P) -> Option<npc::Result<()>> {
        let grave = self.grave_of(p.node())?;
        let result =
            grave::npc_tick(self, &grave).map_err(|e| npc::Error::Binding(format!("{e:?}")));
        self.project_npc(p.node());
        Some(result)
    }

    /// Frames the libraries queued for each player, into its session.
    pub fn drain_all(&mut self) {
        for node in self.inner.frames.keys().copied().collect::<Vec<_>>() {
            if let Some(p) = self.player_of(node) {
                self.inner.drain(&p);
            }
        }
    }

    /// `GraveController.hasGraveAt(loc)`: an active grave's NPC location
    /// equals it (ported from the pinned GraveController.kt without a
    /// differential corpus).
    pub fn has_grave_at(&self, at: Option<[i32; 3]>) -> bool {
        self.inner.combat.graves.entries().iter().any(|(_, grave)| {
            let node = grave.read().npc.node;
            let location = self
                .inner
                .state
                .node_location(node)
                .ok()
                .flatten()
                .map(|l| l.coordinates());
            location == at
        })
    }

    /// GraveController's four option handlers: `node as? Grave` (false when
    /// the NPC is not a grave), then the library's handler.
    pub fn grave_option(
        &mut self,
        option: GraveOption,
        player: &NodeHandle,
        npc: &NodeHandle,
    ) -> Result<bool> {
        let grave = self.grave_of(npc.node);
        let mut controller = std::mem::take(&mut self.inner.combat.graves);
        let result = match option {
            GraveOption::Read => grave::on_read(self, player, grave.as_ref()),
            GraveOption::Bless => grave::on_bless(self, player, grave.as_ref()),
            GraveOption::Repair => grave::on_repair(self, player, grave.as_ref()),
            GraveOption::Demolish => {
                grave::on_demolish(self, &mut controller, player, grave.as_ref())
            }
        };
        self.inner.combat.graves = controller;
        self.drain_all();
        result.map_err(death_error)
    }
}

/// The four options GraveController listens to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraveOption {
    Read,
    Bless,
    Repair,
    Demolish,
}
impl GraveOption {
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::Read, "read"),
        (Self::Bless, "bless"),
        (Self::Repair, "repair"),
        (Self::Demolish, "demolish"),
    ];
}

fn death_error(error: pd::Error) -> Error {
    match error {
        pd::Error::Death(e) => e,
        other => Error::Binding(format!("{other:?}")),
    }
}
