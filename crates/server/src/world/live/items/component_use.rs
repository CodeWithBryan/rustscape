//! `processComponentUseWith` (`world::packet_handlers::component`) over the
//! live world: a spell from the spell book on an NPC, a player, an item or a
//! ground item (`SpellListeners.run`, then `MagicSpell.castSpell`, both in
//! super::super::magic). Spells on scenery (the charge-orb listeners) and
//! familiar specials are refused by name.
use super::super::{P, host::Live, magic};
use super::{Ctx, Node, err, refused};
use crate::world::{
    packet_handlers::{
        Base, Result,
        component::{self, ComponentAction},
    },
    spells::SpellBook,
};

impl Ctx<'_> {
    /// An entity node as the world's entity handle; `None` for an item, a
    /// ground item or scenery.
    fn entity(node: &Node) -> Option<P> {
        match node {
            Node::Player(h) | Node::Npc(h) => Some(P(h.clone())),
            Node::Item(_) | Node::Ground(_) | Node::Scenery(_) => None,
        }
    }
}

impl component::Context for Ctx<'_> {
    fn object_at(&mut self, _: i32, _: i32, _: i32) -> Result<Option<Node>> {
        refused("a spell on scenery (scenery is not a node of the item handlers here)")
    }
    fn attribute_int(&mut self, player: &Node, key: &str, default: i32) -> Result<i32> {
        let canonical = self.canonical(player)?;
        canonical.attributes().integer(key, default).map_err(err)
    }
    fn ticks(&mut self) -> i32 {
        self.0.inner.ticks
    }
    fn spell_book(&mut self, player: &Node) -> Result<i32> {
        let p = self.player_of(player)?;
        self.0
            .inner
            .with_profile(&p, |profile| profile.spell_book)
            .map_err(err)
    }
    fn spell_listeners(
        &mut self,
        button: i32,
        kind: i32,
        book: &str,
        player: &Node,
        node: &Node,
    ) -> Result<()> {
        let p = self.player_of(player)?;
        let id = self.id(node)?;
        magic::spell_listeners(self.0, button, kind, book, &p, id)
            .map_err(|e| crate::world::packet_handlers::Error::Binding(Some(e)))
    }
    fn cast_spell(
        &mut self,
        player: &Node,
        book: SpellBook,
        spell: i32,
        target: &Node,
    ) -> Result<()> {
        let p = self.player_of(player)?;
        let target = Self::entity(target);
        magic::cast_spell(self.0, &p, book, spell, target.as_ref())
            .map_err(|e| crate::world::packet_handlers::Error::Binding(Some(e)))?;
        self.0.drain_all();
        Ok(())
    }
    fn has_familiar(&mut self, _: &Node) -> Result<bool> {
        refused("familiarManager.hasFamiliar (summoning is not ported)")
    }
    fn familiar_special(
        &mut self,
        _: &Node,
        _: &Node,
        _: i32,
        _: i32,
        _: Option<&Node>,
    ) -> Result<()> {
        refused("Familiar.executeSpecialMove (summoning is not ported)")
    }
}

impl Live {
    /// `processComponentUseWith`.
    pub fn component_use_with(
        &mut self,
        player: &P,
        action: &rs09_protocol::game::ClientAction,
    ) -> Result<()> {
        let Some(request) = ComponentAction::from_action(action) else {
            return refused("a component use-with packet without its decoded form");
        };
        let node = Node::Player(player.0.clone());
        component::process(&mut Ctx(self), &node, request)
    }
}
