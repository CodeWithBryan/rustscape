//! Otto Godblessed's dialogue in the live world
//! (`world::dialogue_dsl::otto`, ported without a JVM corpus):
//! OttoGodblessedDialogue's "talk-to" listener for NPC 2725, registered at
//! boot when the class is in the scan (super::everyday's
//! `install_content_use`), and the conversation's ContentAPI calls.
//!
//! Joined: the stage attributes, `setVarbit`, `inInventory`, `inBank` (both
//! banks), `addItemOrDrop`, `removeItem` and the Fremennik diary's task.
//! `hasRequirement` answers false by name (no quest is registered in this
//! world), so the spear branch always takes its rejection. The two books
//! the conversation hands out (BarbarianSkillsBook, MyNotesBook) have no
//! "read" handler here: those classes are not ported.
use super::{Ctx, Node, P, everyday::Everyday, woodcutting::Bound};
use crate::inventory::item::Handle as ItemHandle;
use crate::world::{
    dialogue::{self as d, NodeHandle},
    dialogue_dsl::otto::{self, OttoHost},
    global_items2,
    interaction_listeners::{self as listeners, Function, FunctionKind},
    live::host::Live,
    player_managers::diary,
};

/// The "talk-to" lambda.
pub struct OttoTalk;
impl crate::world::component_objects::NativeObject for OttoTalk {}

fn bind(e: impl std::fmt::Debug) -> d::Error {
    d::Error::Binding(format!("{e:?}"))
}

impl Live {
    /// `OttoGodblessedDialogue.defineListeners`.
    pub fn define_otto_listener(&mut self) -> Result<(), String> {
        let casing = super::super::global::RootCasing;
        self.item_actions
            .listeners
            .clone()
            .add(
                &casing,
                otto::OTTO_GODBLESSED,
                listeners::Kind::Npc.ordinal(),
                &["talk-to".encode_utf16().collect()],
                Function::new(FunctionKind::On, otto::LAMBDA, OttoTalk),
            )
            .map_err(|e| format!("{}: {e:?}", otto::CLASS))
    }

    /// The "talk-to" lambda: `DialogueLabeller.open(player,
    /// OttoGodblessedDialogueFile(), node as NPC)`.
    pub fn otto_talk(&mut self, player: &Node, npc: &Node) -> Bound<bool> {
        let (Node::Player(player), Node::Npc(npc)) = (player, npc) else {
            return Err(super::woodcutting::bind(
                "Otto's talk-to of a node that is not an NPC",
            ));
        };
        let result = otto::talk_to(self, player, &P(npc.clone()));
        self.inner.drain(&P(player.clone()));
        result.map_err(super::woodcutting::bind)
    }

    fn otto_player(&self, player: &NodeHandle) -> d::Result<P> {
        if self.inner.players.contains_key(&player.node) {
            Ok(P(player.clone()))
        } else {
            Err(d::Error::Cast)
        }
    }
}

impl OttoHost for Live {
    fn attribute_int(&mut self, player: &NodeHandle, key: &str, default: i32) -> d::Result<i32> {
        let p = self.otto_player(player)?;
        let attributes = self.inner.canonical(&p).player.attributes();
        // A value of another type is the generic read's ClassCastException.
        attributes.integer(key, default).map_err(|_| d::Error::Cast)
    }
    fn set_attribute_int(&mut self, player: &NodeHandle, key: &str, value: i32) -> d::Result<()> {
        let p = self.otto_player(player)?;
        let attributes = self.inner.canonical(&p).player.attributes();
        attributes.set_integer(key, value);
        Ok(())
    }
    fn remove_attribute(&mut self, player: &NodeHandle, key: &str) -> d::Result<()> {
        let p = self.otto_player(player)?;
        self.inner.canonical(&p).player.attributes().remove(key);
        Ok(())
    }
    fn set_varbit(
        &mut self,
        player: &NodeHandle,
        id: i32,
        value: i32,
        save: bool,
    ) -> d::Result<()> {
        let p = self.otto_player(player)?;
        crate::world::login::Host::set_varbit(self, &p, id, value, save).map_err(bind)
    }
    fn in_inventory(&mut self, player: &NodeHandle, id: i32) -> d::Result<bool> {
        let p = self.otto_player(player)?;
        Ok(self.inner.canonical(&p).player.inventory().contains(id, 1))
    }
    fn in_bank(&mut self, player: &NodeHandle, id: i32) -> d::Result<bool> {
        // amountInBank(player, id) >= 1: in each of the primary and the
        // secondary bank, the amount of the first slot holding the id.
        let p = self.otto_player(player)?;
        let player = self.inner.canonical(&p).player.clone();
        let first = |bank: crate::inventory::bank::Bank| {
            let items = bank.container().items();
            let items = items.read();
            items
                .iter()
                .flatten()
                .map(|item| item.read())
                .find(|item| item.id() == id)
                .map_or(0, |item| item.amount())
        };
        Ok(first(player.bank_primary()).wrapping_add(first(player.bank_secondary())) >= 1)
    }
    fn add_item_or_drop(&mut self, player: &NodeHandle, id: i32, amount: i32) -> d::Result<()> {
        let p = self.otto_player(player)?;
        let added = global_items2::Host::add_item_or_drop(
            &mut Everyday(Ctx(self)),
            &Node::Player(player.clone()),
            id,
            amount,
        );
        self.inner.drain(&p);
        added.map_err(bind)
    }
    fn remove_item(&mut self, player: &NodeHandle, item: &ItemHandle) -> d::Result<bool> {
        let p = self.otto_player(player)?;
        self.remove_from_inventory(&p, item).map_err(bind)
    }
    fn has_requirement(&mut self, player: &NodeHandle, quest: &str) -> d::Result<bool> {
        // No quest is registered in this world: false, counted by name.
        let p = self.otto_player(player)?;
        crate::world::login::Host::has_requirement(self, &p, quest, true).map_err(bind)
    }
    fn finish_task(
        &mut self,
        player: &NodeHandle,
        kind: usize,
        level: i32,
        index: i32,
    ) -> d::Result<()> {
        let p = self.otto_player(player)?;
        super::super::subclasses::with_diaries(self, &p, |h, owner| {
            diary::finish_task(h, owner, owner, Some(kind), level, index)
        })
        .map_err(bind)
    }
}
