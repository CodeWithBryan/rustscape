//! The live world's state for the systems installed after combat: runtime
//! NPCs ([`super::runtime_npcs`]), graves ([`super::graves`]) and the
//! players' `HintIconManager`s (`world::player_managers::hint_icons`, with
//! ContentAPI `registerHintIcon`/`clearHintIcon`).
use super::{P, host::Live, runtime_npcs::Spawning};
use crate::{
    player::attributes::AttributeValue,
    world::{
        player_death::grave,
        player_managers::{
            self as pm,
            hint_icons::{self, HintIconManager, Target},
        },
        player_walk::{Node, NodeHandle},
    },
};
use rs09_protocol::game::Frame;
use std::collections::BTreeMap;

/// The state of the systems installed here.
pub struct ContentWorld {
    pub spawning: Spawning,
    /// Each grave by its NPC's node (`node as? Grave`).
    pub graves: BTreeMap<Node, grave::Handle>,
    /// `player.getHintIconManager()` of each player.
    pub hint_icons: BTreeMap<Node, HintIconManager<P>>,
    /// The dragons' and MonkNPC's swing handler instances.
    pub dragons: super::dragons::Dragons,
}

impl ContentWorld {
    pub fn new(spawning: Spawning) -> Self {
        Self {
            spawning,
            graves: BTreeMap::new(),
            hint_icons: BTreeMap::new(),
            dragons: Default::default(),
        }
    }
}

/// `HintIconManager`'s calls outside its classes, over the live world.
impl hint_icons::Host for Live {
    type Player = P;
    type Entity = NodeHandle;
    type Node = NodeHandle;
    fn hint_icons(&mut self, player: &P) -> pm::Result<&mut HintIconManager<P>> {
        Ok(self.content.hint_icons.entry(player.node()).or_default())
    }
    fn index(&mut self, entity: &NodeHandle) -> pm::Result<i32> {
        if let Some(slot) = self.inner.players.get(&entity.node) {
            return Ok(slot.index);
        }
        self.inner
            .state
            .actor(entity.node)
            .map(|actor| actor.wire_index)
            .map_err(|e| pm::Error::Binding(format!("{e:?}")))
    }
    fn node_location(&mut self, node: &NodeHandle) -> pm::Result<Option<[i32; 3]>> {
        Ok(self
            .inner
            .state
            .node_location(node.node)
            .map_err(|e| pm::Error::Binding(format!("{e:?}")))?
            .map(|l| l.coordinates()))
    }
    fn send(&mut self, player: &P, frame: Frame) -> pm::Result<()> {
        if self.inner.players.contains_key(&player.node()) {
            self.inner.queue_native(player, frame);
        }
        Ok(())
    }
}

impl Live {
    /// `GameWorld.tickListeners` that are installed, in the class scan's
    /// order (`listeners.tick` of `startup/scan_order.txt`): SpotManager,
    /// GraveController.
    pub fn content_tick_listeners(&mut self) -> Result<(), String> {
        // SpotManager (super::items::barbarian).
        self.barbarian_tick();
        self.graves_tick()
            .map_err(|e| format!("GraveController.tick: {e:?}"))?;
        Ok(())
    }

    /// ContentAPI `registerHintIcon(player, node)`: nothing while the
    /// `hinticon` attribute is set; otherwise the manager's
    /// `registerHintIcon(player, node)` and the slot in the attribute.
    pub fn register_hint_icon(&mut self, player: &P, node: &NodeHandle) -> pm::Result<()> {
        let attributes = self.inner.canonical(player).player.attributes();
        if attributes.get("hinticon").is_some() {
            return Ok(());
        }
        let target = if self.inner.players.contains_key(&node.node) {
            Target::Player(node.clone())
        } else if self.inner.npcs.rows.contains_key(&node.node) {
            Target::Entity(node.clone())
        } else {
            Target::Node(node.clone())
        };
        let slot = hint_icons::register(self, player, Some(&target))?;
        attributes.set_value("hinticon", Some(AttributeValue::Int(slot)));
        Ok(())
    }
    /// ContentAPI `clearHintIcon(player)`: the attribute's slot (-1 when
    /// absent) is removed from the attributes, then from the manager.
    pub fn clear_hint_icon(&mut self, player: &P) -> pm::Result<()> {
        let attributes = self.inner.canonical(player).player.attributes();
        let slot = match attributes.get("hinticon") {
            Some(AttributeValue::Int(slot)) => slot,
            None => -1,
            Some(_) => {
                return Err(pm::Error::Binding(
                    "the hinticon attribute is not an integer".into(),
                ));
            }
        };
        attributes.remove("hinticon");
        hint_icons::remove(self, player, slot)
    }
}
