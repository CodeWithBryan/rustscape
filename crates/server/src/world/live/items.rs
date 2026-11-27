//! The packet handlers the driver routes (`world::packet_handlers`) bound to
//! the live world: `processItemAction` with `InteractPlugin.handleItemOption`
//! and its pulse, `processNpcAction`, `processPlayerAction`, the three
//! examine branches and `TrackFinished`, with `DropListener` and
//! `EquipHandler` registered in the world's native `InteractionListeners`
//! registry (`world::interaction_listeners`).
//!
//! [`Ctx`] is the libraries' world over [`Live`]: nodes are the canonical
//! players and NPCs, the canonical containers' item handles and the ground
//! items; an item's `InteractPlugin` is an option table made the first time
//! it is asked for (`setDefault` over the item definition's inventory
//! actions and `ItemDefinition.OPTION_HANDLERS`), an NPC's over its
//! definition's options, a player's is the one login set up.
//!
//! Every call into a system without a port is a named answer
//! ([`super::not_ported`]) or a refusal, listed in docs/live-world.md.
use super::{P, host::Live, tabs::Tabs};
use crate::{
    definitions::{
        item::{Handle as Definition, Shared, WeakShared},
        npc, option_handler as handlers, scenery,
    },
    inventory::{self, item::Handle as ItemHandle},
    pulses,
    world::{
        action_locks,
        component_objects::NativeObject,
        ground_item::{self as ground, Handle as Ground},
        interaction::OptionHandle,
        interaction_listeners::{self as listeners, Function, FunctionKind, Kind, Text},
        interaction_options::{self as options, Names, NamesKind},
        legacy_runtime,
        packet_handlers::{
            Base, Container, Error, Listener, NodeKind, Result, actions, drop, equip, examine,
            item_option,
        },
        player_walk::{self, Event, NodeHandle, Scripts},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::{ClientAction, Frame, updates};
use std::collections::HashMap;

mod component_use;
pub mod cooking;
pub mod everyday;
pub mod fishing;
pub mod milking;
pub mod mining;
mod slots;
pub mod smelting;
pub mod smithing;
pub mod use_with;
pub mod withnpc;
pub mod withobject;
pub mod woodcutting;

/// A node the handlers see: `==` is identity.
#[derive(Clone)]
pub enum Node {
    Player(NodeHandle),
    Npc(NodeHandle),
    Item(ItemHandle),
    Ground(Ground),
    /// A scene object (`RegionManager.getObject`, or its child for the
    /// player): only use-with reaches one here (super::items::use_with).
    Scenery(crate::world::doors::Handle),
}
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Player(a), Self::Player(b)) | (Self::Npc(a), Self::Npc(b)) => a.node == b.node,
            (Self::Item(a), Self::Item(b)) => a.same(b),
            (Self::Ground(a), Self::Ground(b)) => a.same(b),
            (Self::Scenery(a), Self::Scenery(b)) => a.same(b),
            _ => false,
        }
    }
}

/// A node's `InteractPlugin`: its option table and the node it belongs to.
#[derive(Clone)]
pub struct Interaction {
    table: options::Handle,
    owner: Node,
}

/// The listener objects `DropListener` and `EquipHandler` register.
struct Registered(Listener);
impl NativeObject for Registered {}

/// `content.global.skill.AttackListener`'s `on(IntType.NPC, "attack")`
/// function (`flagInstant()`).
struct AttackListener;
impl NativeObject for AttackListener {}
const ATTACK_LISTENER: &str = "content.global.skill.AttackListener";

/// GraveController's `on(GraveType.ids, IntType.NPC, option)` listeners
/// (super::graves).
struct GraveListener(super::graves::GraveOption);
impl NativeObject for GraveListener {}
const GRAVE_CONTROLLER: &str = "core.game.node.entity.combat.graves.GraveController";

/// `AttackListener`'s body: the Lumbridge dummies' style checks, the
/// killerwatt's aggression (content, refused), then `player.attack(npc)`.
/// Ported from the pinned AttackListener.kt; it has no differential corpus.
fn attack_listener(ctx: &mut Ctx<'_>, player: &Node, npc: &Node) -> Result<bool> {
    use crate::world::combat_pulse::Host as _;
    let p = ctx.player_of(player)?;
    let Node::Npc(handle) = npc else {
        return Err(Error::Cast);
    };
    let target = P(handle.clone());
    let id = ctx
        .0
        .inner
        .npcs
        .rows
        .get(&handle.node)
        .map(|r| r.id)
        .ok_or(Error::Null)?;
    let style = |ctx: &mut Ctx<'_>| -> Result<Option<crate::world::combat_swing::CombatStyle>> {
        let handler = ctx.0.swing_handler(&p, false).map_err(err)?;
        Ok(ctx.0.handler(&handler).style)
    };
    use crate::world::combat_swing::CombatStyle;
    if id == 4474 && style(ctx)? != Some(CombatStyle::Magic) {
        ctx.0
            .inner
            .message(&p, "You can only attack this with magic.")
            .map_err(err)?;
        return Ok(true);
    }
    if id == 7891 && style(ctx)? != Some(CombatStyle::Melee) {
        ctx.0
            .inner
            .message(&p, "You must use the training sword to attack this.")
            .map_err(err)?;
        return Ok(true);
    }
    if id == 3201 || id == 3202 {
        return refused("KillerwattBehavior.aggressUponKillerwatt (content)");
    }
    ctx.0.combat_attack(&p, &target).map_err(err)?;
    Ok(true)
}

/// The packet handlers' state in the live world.
#[derive(Default)]
pub struct ItemActions {
    /// `InteractionListeners`' registries.
    pub listeners: listeners::Registry,
    /// Inventory and equipment items' option tables, by item identity.
    tables: HashMap<usize, (WeakShared<inventory::item::Item>, options::Handle)>,
    /// NPCs' option tables, by node.
    npc_tables: HashMap<usize, options::Handle>,
    /// `InteractPlugin$1` pulses submitted to `GameWorld.Pulser`.
    pub pulses: Vec<(pulses::Handle, item_option::Task<Node>)>,
    /// `DropListener`'s soft queued scripts: due tick and captures.
    pub drops: Vec<(i32, drop::DropScript<Node>)>,
    /// `runUnhandledAction` pulses of use-with ([`use_with`]).
    pub use_pulses: Vec<(pulses::Handle, Node)>,
    /// `UseWithHandler.HANDLERS`.
    pub use_handlers: crate::world::packet_handlers::use_with::Registry,
}

/// The item ids `Pets.forId` answers (every constant's baby, grown and
/// overgrown item id), extracted from the pinned Pets.java, sorted. An absent
/// stage's -1 is not among them: `Pets.forId(-1)` is null.
const PET_ITEMS: &[i32] = &[
    1555, 1556, 1557, 1558, 1559, 1560, 1561, 1562, 1563, 1564, 1565, 1566, 1567, 1568, 1569, 1570,
    1571, 1572, 7581, 7582, 7583, 7771, 7772, 12469, 12470, 12471, 12472, 12473, 12474, 12475,
    12476, 12481, 12482, 12484, 12485, 12486, 12487, 12488, 12489, 12490, 12491, 12492, 12493,
    12496, 12497, 12498, 12499, 12500, 12501, 12503, 12504, 12505, 12506, 12507, 12508, 12509,
    12510, 12511, 12512, 12513, 12514, 12515, 12516, 12517, 12518, 12519, 12520, 12521, 12522,
    12523, 12548, 12549, 12550, 12551, 12552, 12553, 12682, 12683, 12684, 12685, 12686, 12687,
    12688, 12689, 12690, 12691, 12692, 12693, 12694, 12695, 12696, 12697, 12698, 12699, 12700,
    12701, 12702, 12703, 12704, 12705, 12706, 12707, 12708, 12709, 12710, 12711, 12712, 12713,
    12714, 12715, 12716, 12717, 12718, 12719, 12720, 12721, 12722, 12723, 12724, 12725, 12726,
    12727, 12728, 12729, 12730, 12731, 12732, 12733, 12734, 12735, 12736, 12737, 12738, 12739,
    12740, 12741, 12742, 12743, 12744, 12745, 12746, 12747, 12748, 12749, 12750, 12751, 12752,
    12753, 12754, 12755, 12756, 12757, 12758, 12759, 12760, 12761, 12762, 12763, 12764, 12765,
    12766, 12767, 12768, 12769, 12770, 12771, 12772, 12773, 12774, 12775, 13335, 14089, 14090,
    14533, 14626, 14627, 15092,
];

/// The handlers' world over the live world.
pub struct Ctx<'a>(pub &'a mut Live);

fn err(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}
fn refused<T>(what: &str) -> Result<T> {
    Err(Error::Binding(Some(format!(
        "not ported in the live world: {what}"
    ))))
}
fn lerr(e: impl std::fmt::Debug) -> listeners::Error {
    listeners::Error::Binding(format!("{e:?}"))
}
fn lrefused<T>(what: &str) -> listeners::Result<T> {
    Err(listeners::Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}
/// `Location.toString`: `[x, y, z]`.
fn location_text([x, y, z]: [i32; 3]) -> String {
    format!("[{x}, {y}, {z}]")
}
fn text(s: &str) -> Text {
    s.encode_utf16().collect()
}
fn string(t: &[u16]) -> String {
    String::from_utf16_lossy(t)
}

impl Ctx<'_> {
    fn player_of(&self, node: &Node) -> Result<P> {
        match node {
            Node::Player(h) if self.0.inner.players.contains_key(&h.node) => Ok(P(h.clone())),
            _ => Err(Error::Cast),
        }
    }
    fn canonical(&self, node: &Node) -> Result<crate::player::owned::Player> {
        let p = self.player_of(node)?;
        Ok(self.0.inner.canonical(&p).player.clone())
    }
    fn container(
        &self,
        player: &Node,
        container: Container,
    ) -> Result<inventory::container::Container> {
        let player = self.canonical(player)?;
        match container {
            Container::Inventory => Ok(player.inventory()),
            Container::Equipment => Ok(player.equipment()),
            Container::Bank => Ok(player.bank().container()),
        }
    }
    fn locked(&mut self, player: &Node, kind: action_locks::Kind) -> Result<bool> {
        let p = self.player_of(player)?;
        let owner = p.0.clone();
        let state = &mut self.0.inner.state;
        state
            .locks(&owner)
            .map_err(err)?
            .is_kind_locked(kind, state, &mut action_locks::Clock)
            .map_err(err)
    }
    fn item(node: &Node) -> Result<ItemHandle> {
        match node {
            Node::Item(item) => Ok(item.clone()),
            Node::Ground(g) => Ok(ground::as_item(g)),
            _ => Err(Error::Cast),
        }
    }
    /// An item's InteractPlugin, set up once (`getInteraction()`).
    fn item_table(&mut self, item: &ItemHandle) -> Result<options::Handle> {
        let key = item.identity();
        if let Some((weak, table)) = self.0.item_actions.tables.get(&key)
            && weak.upgrade().is_some_and(|i| i.same(item))
        {
            return Ok(table.clone());
        }
        let table = options::Table::new(Some(key));
        let defaults = self.0.walk.ground.options.defaults();
        let registry = self.0.walk.ground.options.clone();
        let definitions = self.0.inner.items.clone();
        options::set_default(
            &mut ItemOptions {
                item,
                node: key,
                defaults: &defaults,
                registry: &registry,
                definitions: &definitions,
                constants: &self.0.inner.options,
            },
            &table,
        )
        .map_err(err)?;
        let tables = &mut self.0.item_actions.tables;
        tables.retain(|_, (weak, _)| weak.upgrade().is_some());
        tables.insert(key, (item.downgrade(), table.clone()));
        Ok(table)
    }
    /// An NPC's InteractPlugin, set up once.
    fn npc_table(&mut self, npc: &NodeHandle) -> Result<options::Handle> {
        if let Some(table) = self.0.item_actions.npc_tables.get(&npc.node) {
            return Ok(table.clone());
        }
        let row = self.0.inner.npcs.rows.get(&npc.node).ok_or(Error::Null)?;
        let (id, definition) = (row.id, row.definition.clone());
        let table = options::Table::new(Some(npc.node));
        options::set_default(
            &mut NpcOptions {
                node: npc.node,
                id,
                definition,
                not_ported: &mut self.0.inner.not_ported,
                constants: &self.0.inner.options,
            },
            &table,
        )
        .map_err(err)?;
        self.0
            .item_actions
            .npc_tables
            .insert(npc.node, table.clone());
        Ok(table)
    }
    fn with_container<T>(
        &mut self,
        player: &Node,
        container: Container,
        f: impl FnOnce(
            &mut inventory::container::Container,
            &crate::definitions::item::Registry,
            &mut crate::inventory::player_binding::Context<
                '_,
                '_,
                crate::player::owned::Services<'_>,
            >,
        ) -> inventory::Result<T>,
    ) -> Result<T> {
        let p = self.player_of(player)?;
        let canonical = self.0.inner.canonical(&p).player.clone();
        let mut target = self.container(player, container)?;
        let inner = &mut self.0.inner;
        let result = canonical
            .apply(
                &mut inner.state,
                &mut inner.frames,
                super::now_millis,
                |_, defs, ctx| f(&mut target, defs, ctx),
            )
            .map_err(err);
        inner.drain(&p);
        result
    }
    fn walk_event(&mut self, event: Event<'_>) -> Result<()> {
        use player_walk::Services;
        self.0
            .with_walk_host(|state, host| host.event(state, event))
            .map_err(err)
    }
}

/// `InteractPlugin.setDefault` over an inventory item.
struct ItemOptions<'a> {
    item: &'a ItemHandle,
    node: usize,
    defaults: &'a handlers::DefaultHandle,
    registry: &'a crate::world::ground_options::Registry,
    definitions: &'a crate::definitions::item::Registry,
    constants: &'a options::Constants,
}
struct Lookup<'a>(&'a crate::definitions::item::Registry);
impl handlers::Context for Lookup<'_> {
    fn definition(&mut self, id: i32) -> handlers::Result<Option<Definition>> {
        Ok(Some(self.0.for_id(id)))
    }
    fn missing_definition(&mut self, _: i32) -> handlers::Result<()> {
        Err(handlers::Error::Binding(
            "nonnull forId fallback became null".into(),
        ))
    }
}
impl options::Context for ItemOptions<'_> {
    type Definition = Definition;
    fn kind(&self, node: Option<usize>) -> options::Kind {
        if node == Some(self.node) {
            options::Kind::Item
        } else {
            options::Kind::Other
        }
    }
    fn node_id(&mut self, _: usize) -> options::Result<i32> {
        Ok(self.item.read().id())
    }
    fn item_has_location(&mut self, _: usize) -> options::Result<bool> {
        // An inventory or equipment Item has no location.
        Ok(false)
    }
    fn definition(&mut self, _: usize) -> options::Result<Option<Definition>> {
        Ok(self.item.read().definition.clone())
    }
    fn names(
        &mut self,
        definition: &Definition,
        kind: NamesKind,
    ) -> options::Result<Option<Names>> {
        let d = definition.read();
        Ok(match kind {
            NamesKind::Ground => d.ground_actions.clone(),
            NamesKind::Inventory => d.options.clone(),
            NamesKind::Options => {
                return Err(options::Error::Binding("non-item action array".into()));
            }
        })
    }
    fn lowercase(&mut self, name: &str) -> options::Result<String> {
        Ok(lowercase_root(name))
    }
    fn option_handler(
        &mut self,
        _: options::Kind,
        id: i32,
        name: &str,
    ) -> options::Result<Option<usize>> {
        let handler =
            handlers::item_handler(&mut Lookup(self.definitions), self.defaults, id, Some(name))
                .map_err(|e| options::Error::Binding(e.to_string()))?;
        handler.map(|h| self.registry.bind(h)).transpose()
    }
    fn send_option(&mut self, _: usize, _: i32, _: Option<String>) -> options::Result<()> {
        Err(options::Error::Binding(
            "an item table sent a player option".into(),
        ))
    }
    fn node_string(&mut self, _: Option<usize>) -> options::Result<String> {
        Ok(self.item.read().text())
    }
    fn constants(&self) -> &options::Constants {
        self.constants
    }
}

/// `InteractPlugin.setDefault` over an NPC.
struct NpcOptions<'a> {
    node: usize,
    id: i32,
    definition: npc::Handle,
    not_ported: &'a mut super::not_ported::NotPorted,
    constants: &'a options::Constants,
}
impl options::Context for NpcOptions<'_> {
    type Definition = npc::Handle;
    fn kind(&self, node: Option<usize>) -> options::Kind {
        if node == Some(self.node) {
            options::Kind::Npc
        } else {
            options::Kind::Other
        }
    }
    fn node_id(&mut self, _: usize) -> options::Result<i32> {
        Ok(self.id)
    }
    fn item_has_location(&mut self, _: usize) -> options::Result<bool> {
        Err(options::Error::Binding("an NPC is not an item".into()))
    }
    fn definition(&mut self, _: usize) -> options::Result<Option<npc::Handle>> {
        Ok(Some(self.definition.clone()))
    }
    fn names(&mut self, definition: &npc::Handle, _: NamesKind) -> options::Result<Option<Names>> {
        Ok(definition.read().options.clone())
    }
    fn lowercase(&mut self, name: &str) -> options::Result<String> {
        Ok(lowercase_root(name))
    }
    fn option_handler(
        &mut self,
        _: options::Kind,
        _: i32,
        _: &str,
    ) -> options::Result<Option<usize>> {
        // NPCDefinition's option handlers are registered by content plugins.
        self.not_ported.default_answer(
            "NPCDefinition.getOptionHandler (NPC option handlers are content: null)",
        );
        Ok(None)
    }
    fn send_option(&mut self, _: usize, _: i32, _: Option<String>) -> options::Result<()> {
        Err(options::Error::Binding(
            "an NPC table sent a player option".into(),
        ))
    }
    fn node_string(&mut self, _: Option<usize>) -> options::Result<String> {
        Ok(format!("NPC {}", self.id))
    }
    fn constants(&self) -> &options::Constants {
        self.constants
    }
}

impl Base for Ctx<'_> {
    type Node = Node;
    type Interaction = Interaction;
    fn id(&mut self, node: &Node) -> Result<i32> {
        match node {
            Node::Item(i) => Ok(i.read().id()),
            Node::Ground(g) => Ok(ground::as_item(g).read().id()),
            Node::Npc(n) => self
                .0
                .inner
                .npcs
                .rows
                .get(&n.node)
                .map(|r| r.id)
                .ok_or(Error::Null),
            // Node.id of a player is never set: 0.
            Node::Player(_) => Ok(0),
            Node::Scenery(s) => Ok(s.read().id),
        }
    }
    fn node_kind(&mut self, node: &Node) -> NodeKind {
        match node {
            Node::Player(_) => NodeKind::Player,
            Node::Npc(_) => NodeKind::Npc,
            Node::Item(_) => NodeKind::Item,
            Node::Ground(_) => NodeKind::GroundItem,
            Node::Scenery(_) => NodeKind::Scenery,
        }
    }
    fn index(&mut self, node: &Node) -> Result<i32> {
        match node {
            Node::Item(i) => Ok(i.read().index),
            Node::Ground(g) => Ok(g.read().item.read().index),
            Node::Player(h) => Ok(self.0.inner.players.get(&h.node).ok_or(Error::Null)?.index),
            Node::Npc(h) => Ok(self.0.inner.state.actor(h.node).map_err(err)?.wire_index),
            Node::Scenery(_) => refused("Node.getIndex of scenery"),
        }
    }
    fn set_index(&mut self, node: &Node, index: i32) -> Result<()> {
        match node {
            Node::Item(i) => {
                i.write().index = index;
                Ok(())
            }
            _ => refused("Node.setIndex of a non-item"),
        }
    }
    fn active(&mut self, node: &Node) -> Result<bool> {
        Ok(match node {
            Node::Player(h) => self.0.inner.players.get(&h.node).is_some_and(|s| s.active),
            Node::Npc(h) => self
                .0
                .inner
                .npcs
                .rows
                .get(&h.node)
                .is_some_and(|r| r.active),
            Node::Item(_) | Node::Ground(_) => true,
            Node::Scenery(s) => s.read().active,
        })
    }
    fn text(&mut self, node: Option<&Node>) -> Result<String> {
        Ok(match node {
            None => "null".into(),
            Some(Node::Item(i)) => i.read().text(),
            Some(Node::Ground(g)) => ground::as_item(g).read().text(),
            // NPC.toString: "NPC " + id + ", " + getLocation() + ", " + getIndex().
            Some(Node::Npc(h)) => {
                let id = self.id(&Node::Npc(h.clone()))?;
                let at = self
                    .0
                    .inner
                    .state
                    .node_location(h.node)
                    .map_err(err)?
                    .map_or_else(|| "null".into(), |l| location_text(l.coordinates()));
                let index = self.0.inner.state.actor(h.node).map_err(err)?.wire_index;
                format!("NPC {id}, {at}, {index}")
            }
            // Scenery.toString.
            Some(Node::Scenery(s)) => {
                let s = s.read();
                format!(
                    "[Scenery {}, {}, type={}, rot={}]",
                    s.id,
                    location_text(s.location.coordinates()),
                    s.shape,
                    s.rotation
                )
            }
            // Player.toString: "Player [name=" + name + ", getRights()=" +
            // getRights() + "]" (the name is the details' username; the
            // rights are the enum constant's name).
            Some(Node::Player(h)) => {
                let p = self.player_of(&Node::Player(h.clone()))?;
                let rights = self
                    .0
                    .inner
                    .with_profile(&p, |profile| profile.details.rights())
                    .map_err(err)?
                    .map_err(err)?;
                let rights = match rights {
                    crate::player::details::Rights::RegularPlayer => "REGULAR_PLAYER",
                    crate::player::details::Rights::PlayerModerator => "PLAYER_MODERATOR",
                    crate::player::details::Rights::Administrator => "ADMINISTRATOR",
                };
                let name = &self.0.inner.slot(&p).name;
                format!("Player [name={name}, getRights()={rights}]")
            }
        })
    }
    fn lowercase_default(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn debug(&mut self, player: &Node, text: &str) -> Result<()> {
        // Player.debug: a message only when the debug attribute is set.
        let canonical = self.canonical(player)?;
        if canonical
            .attributes()
            .boolean("debug", false)
            .map_err(err)?
        {
            let p = self.player_of(player)?;
            self.0.inner.message(&p, text).map_err(err)?;
        }
        Ok(())
    }
    fn message(&mut self, player: &Node, text: Option<&str>) -> Result<()> {
        let p = self.player_of(player)?;
        self.0
            .inner
            .message(&p, text.unwrap_or("null"))
            .map_err(err)
    }
    fn packet_message(&mut self, player: &Node, text: &str) -> Result<()> {
        let p = self.player_of(player)?;
        self.0.inner.message(&p, text).map_err(err)
    }
    fn api_message(&mut self, player: &Node, text: &str) -> Result<()> {
        let p = self.player_of(player)?;
        self.0.inner.message(&p, text).map_err(err)
    }
    fn clear_minimap(&mut self, player: &Node) -> Result<()> {
        let p = self.player_of(player)?;
        self.0.inner.queue_native(
            &p,
            Frame {
                opcode: 153,
                payload: vec![],
            },
        );
        Ok(())
    }
    fn reset_scripts(&mut self, player: &Node) -> Result<()> {
        let p = self.player_of(player)?;
        self.walk_event(Event::Scripts(p.node(), Scripts::Reset))
    }
    fn interaction_locked(&mut self, player: &Node) -> Result<bool> {
        self.locked(player, action_locks::Kind::Interaction)
    }
    fn movement_locked(&mut self, player: &Node) -> Result<bool> {
        self.locked(player, action_locks::Kind::Movement)
    }
    fn has_equipment_lock(&mut self, player: &Node) -> Result<bool> {
        let p = self.player_of(player)?;
        Ok(self
            .0
            .inner
            .state
            .locks(&p.0)
            .map_err(err)?
            .get(action_locks::Kind::Equipment)
            .is_some())
    }
    fn equipment_locked(&mut self, player: &Node) -> Result<bool> {
        self.locked(player, action_locks::Kind::Equipment)
    }
    fn equipment_lock_message(&mut self, _: &Node) -> Result<Option<String>> {
        refused("Lock.getMessage (nothing in this world sets an equipment lock)")
    }
    fn location(&mut self, player: &Node) -> Result<Option<[i32; 3]>> {
        let node = match player {
            Node::Player(h) | Node::Npc(h) => h.node,
            _ => return refused("the location of a non-entity"),
        };
        Ok(self
            .0
            .inner
            .state
            .node_location(node)
            .map_err(err)?
            .map(|l| l.coordinates()))
    }
    fn interaction(&mut self, node: &Node) -> Result<Option<Interaction>> {
        let table = match node {
            Node::Item(item) => self.item_table(item)?,
            Node::Npc(npc) => self.npc_table(npc)?,
            Node::Player(h) => {
                let p = self.player_of(&Node::Player(h.clone()))?;
                self.0.inner.canonical(&p).options.clone()
            }
            Node::Ground(_) => {
                return refused("a ground item's interaction from the item handlers");
            }
            Node::Scenery(_) => {
                return refused("a scenery's interaction from the item handlers");
            }
        };
        Ok(Some(Interaction {
            table,
            owner: node.clone(),
        }))
    }
    fn option(&mut self, interaction: &Interaction, index: i32) -> Result<Option<OptionHandle>> {
        options::get(&interaction.table, index).map_err(|e| match e {
            options::Error::Null => Error::Null,
            options::Error::Index => Error::Index(Some(format!(
                "Index {index} out of bounds for length {}",
                interaction.table.read().options().read().len()
            ))),
            e => err(e),
        })
    }
    fn handle(
        &mut self,
        interaction: &Interaction,
        player: &Node,
        option: &OptionHandle,
    ) -> Result<()> {
        let target = match &interaction.owner {
            Node::Player(h) | Node::Npc(h) => h.node,
            _ => return refused("InteractPlugin.handle for an item"),
        };
        let p = self.player_of(player)?;
        let option = option.clone();
        // A player's InteractPlugin: its option handlers are the player
        // handler arena's (super::follow).
        let of_player = matches!(&interaction.owner, Node::Player(_));
        self.0
            .with_walk_host(|state, host| {
                host.walk.player_dispatch = of_player;
                let result =
                    legacy_runtime::handle(state, host, Some(target), Some(p.node()), Some(option));
                host.walk.player_dispatch = false;
                result
            })
            .map_err(err)
    }
    fn invalid(&mut self, player: &Node, node: &Node) -> Result<()> {
        let target = match node {
            Node::Player(h) | Node::Npc(h) => h.node,
            _ => return refused("handleInvalidInteraction for an item"),
        };
        let p = self.player_of(player)?;
        let null = self.0.inner.options.null.clone();
        self.0
            .with_walk_host(|state, host| {
                legacy_runtime::invalid(state, host, Some(target), Some(p.node()), Some(null))
            })
            .map_err(err)
    }
    fn listeners(
        &mut self,
        id: i32,
        kind: Kind,
        option: Option<&str>,
        player: &Node,
        node: &Node,
    ) -> Result<bool> {
        // InteractionListeners.run(id, type, option, player, node): Kotlin's
        // non-null parameter check first.
        let option = option.ok_or(Error::Null)?;
        let registry = self.0.item_actions.listeners.clone();
        registry
            .run(self, id, kind, &text(option), player.clone(), node.clone())
            .map_err(|e| match e {
                listeners::Error::Null => Error::Null,
                e => err(e),
            })
    }
    fn capacity(&mut self, player: &Node, container: Container) -> Result<i32> {
        Ok(self.container(player, container)?.capacity())
    }
    fn get(&mut self, player: &Node, container: Container, slot: i32) -> Result<Option<Node>> {
        Ok(self.container(player, container)?.get(slot).map(Node::Item))
    }
    fn replace(
        &mut self,
        player: &Node,
        container: Container,
        item: Option<&Node>,
        slot: i32,
        fire: bool,
    ) -> Result<Option<Node>> {
        let item = item.map(Self::item).transpose()?;
        self.with_container(player, container, |c, defs, ctx| {
            c.replace(item.as_ref(), slot, fire, defs, ctx)
        })
        .map(|old| old.map(Node::Item))
    }
    fn npc(&mut self, index: i32) -> Result<Option<Node>> {
        let npcs = self.0.repository.npcs();
        let found = npcs.get(index).map_err(|_| {
            Error::Index(Some(format!(
                "Index {index} out of bounds for length {}",
                crate::world::repository::MAX_NPCS
            )))
        })?;
        Ok(found.map(|p| Node::Npc(p.0)))
    }
    fn other_player(&mut self, index: i32) -> Result<Option<Node>> {
        let players = self.0.repository.players();
        let found = players.get(index).map_err(|_| {
            Error::Index(Some(format!(
                "Index {index} out of bounds for length {}",
                crate::world::repository::MAX_PLAYERS
            )))
        })?;
        Ok(found.map(|p| Node::Player(p.0)))
    }
    fn shown_npc(&mut self, npc: &Node, _: &Node) -> Result<Node> {
        // NPC.getShownNPC: the NPC itself unless its definition has child
        // ids (a varbit/varp morph), which are not resolved here.
        let Node::Npc(h) = npc else {
            return Err(Error::Cast);
        };
        let row = self.0.inner.npcs.rows.get(&h.node).ok_or(Error::Null)?;
        if row.definition.read().child_npc_ids.is_some() {
            return refused("NPC.getShownNPC for a morphing NPC (child ids)");
        }
        Ok(npc.clone())
    }
    fn ground(&mut self, id: i32, location: [i32; 3], player: &Node) -> Result<Option<Node>> {
        let p = self.player_of(player)?;
        let [x, y, z] = location;
        super::ground_items::World::with(self.0, |w| {
            crate::world::ground_manager::get(
                w,
                id,
                Some(crate::world::doors::Location::new(x, y, z)),
                Some(p.0.clone()),
            )
        })
        .map(|g| g.map(Node::Ground))
        .map_err(err)
    }
    fn max_players(&mut self) -> i32 {
        crate::world::repository::MAX_PLAYERS
    }
    fn max_npcs(&mut self) -> i32 {
        crate::world::repository::MAX_NPCS
    }
    fn item_definition_of(&mut self, item: &Node) -> Result<Definition> {
        Self::item(item)?
            .read()
            .definition
            .clone()
            .ok_or(Error::Null)
    }
    fn audio(&mut self, player: &Node, id: i32) -> Result<()> {
        let p = self.player_of(player)?;
        self.0
            .inner
            .queue_native(&p, updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn on(&mut self, kind: Kind, options: &[&str], listener: Listener) -> Result<()> {
        let class = match listener {
            Listener::Drop => "core.game.global.action.DropListener",
            Listener::Equip => "core.game.global.action.EquipHandler",
        };
        let function = Function::new(FunctionKind::On, class, Registered(listener));
        let options: Vec<Text> = options.iter().map(|o| text(o)).collect();
        let registry = self.0.item_actions.listeners.clone();
        registry
            .add_generics(self, &options, kind.ordinal(), function)
            .map_err(err)
    }
}

// ---------------------------------------------------- InteractPlugin$1

impl item_option::Context for Ctx<'_> {
    fn owner(&mut self, interaction: &Interaction) -> Result<Node> {
        Ok(interaction.owner.clone())
    }
    fn clear_pulses(&mut self, player: &Node) -> Result<()> {
        // player.getPulseManager().clear(PulseType.STANDARD).
        let p = self.player_of(player)?;
        let node = p.node();
        self.0
            .with_walk(|world| {
                let manager = world.state.actor(node)?.manager.clone();
                world.clear_standard(node, manager)
            })
            .map_err(err)
    }
    fn submit(&mut self, task: item_option::Task<Node>) -> Result<()> {
        let p = self.player_of(&task.player)?;
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(vec![Some(p.node())])),
            pulses::Class::Other,
            item_option::CLASS.into(),
        ));
        self.0
            .driver
            .pulser()
            .submit(Some(handle.clone()))
            .map_err(err)?;
        self.0.item_actions.pulses.push((handle, task));
        Ok(())
    }
    fn zone_interact(&mut self, player: &Node, node: &Node, option: &OptionHandle) -> Result<bool> {
        let p = self.player_of(player)?;
        let _ = (node, option);
        let monitor = self.0.monitor(&p.0);
        if monitor.zones().is_empty() {
            // ZoneMonitor.interact with no zone: false.
            return Ok(false);
        }
        refused("ZoneMonitor.interact inside a region zone (zone interactions are content)")
    }
    fn handler_handle(&mut self, handler: usize, _: &Node, _: &Node, _: &str) -> Result<bool> {
        refused(&format!(
            "the item option handler {handler} (item option handlers are content)"
        ))
    }
    fn handler_name(&mut self, handler: usize) -> Result<String> {
        let h = self.0.walk.ground.options.handler(handler).map_err(err)?;
        let label = h.read().label().to_owned();
        Ok(label.rsplit('.').next().unwrap_or_default().to_owned())
    }
    fn log(&mut self, origin: &str, level: &str, message: &str) -> Result<()> {
        self.0.inner.log_line(format!("{origin} {level} {message}"));
        Ok(())
    }
}

// ---------------------------------------------------- DropListener

impl crate::inventory::item::Bindings for Ctx<'_> {
    type Handler = ();
    type Plugin = ();
    fn drop_replacement(&mut self, id: i32) -> inventory::Result<i32> {
        // DegradableEquipment.getDropReplacement: its lists are filled by
        // content subclasses; none is registered, so the item's own id.
        self.0.inner.not_ported.default_answer(
            "DegradableEquipment.getDropReplacement (no degradable equipment registered: the same id)",
        );
        Ok(id)
    }
    fn operate_handler(&mut self, _: i32) -> inventory::Result<Option<()>> {
        Err(inventory::Error::Binding(
            "operate handlers are content".into(),
        ))
    }
    fn plugin(&mut self, _: &Definition) -> inventory::Result<Option<()>> {
        Err(inventory::Error::Binding("item plugins are content".into()))
    }
}

impl drop::Context for Ctx<'_> {
    fn used_option(&mut self, player: &Node) -> Result<String> {
        // getUsedOption: the "interact:option" attribute, default "INVALID".
        let canonical = self.canonical(player)?;
        match canonical.attributes().get("interact:option") {
            None => Ok("INVALID".into()),
            Some(crate::player::attributes::AttributeValue::Text(t)) => Ok(t),
            Some(_) => Err(Error::Cast),
        }
    }
    fn is_pet(&mut self, id: i32) -> Result<bool> {
        // Pets.forId(id) != null: the baby, grown and overgrown item ids of
        // the Pets enum (its static maps, -1 included).
        Ok(PET_ITEMS.binary_search(&id).is_ok())
    }
    fn summon(&mut self, _: &Node, _: &Node, _: bool, _: bool) -> Result<()> {
        refused("FamiliarManager.summon")
    }
    fn has_grave_at(&mut self, at: Option<[i32; 3]>) -> Result<bool> {
        // GraveController.hasGraveAt over the active graves (super::graves).
        Ok(self.0.has_grave_at(at))
    }
    fn close_all_interfaces(&mut self, player: &Node) -> Result<()> {
        let p = self.player_of(player)?;
        // interfaceManager.close() with the world's interface context (the
        // global handlers' close listeners, super::global_ui), then
        // closeChatbox().
        self.0
            .with_world_ui(&p, |ui, m| {
                crate::world::interfaces::close(ui, m).map(|_| ())
            })
            .map_err(err)?;
        self.0.inner.current = Some(p.clone());
        let manager = self.0.inner.canonical(&p).player.manager();
        crate::world::interfaces::close_chatbox(&mut self.0.inner, &manager).map_err(err)
    }
    fn queue_soft_script(&mut self, _: &Node, script: drop::DropScript<Node>) -> Result<()> {
        // queueScript(player, 1, SOFT): due at the next tick.
        let due = self.0.inner.ticks.wrapping_add(1);
        self.0.item_actions.drops.push((due, script));
        Ok(())
    }
    fn stop_executing(&mut self, _: &Node) -> Result<bool> {
        Ok(true)
    }
    fn dupe_alert(&mut self, _: &Node, text: &str) -> Result<()> {
        self.0.inner.log_line(format!("DUPE_ALERT {text}"));
        self.0
            .inner
            .not_ported
            .skip("PlayerMonitor.log DUPE_ALERT (player monitor not ported: logged only)");
        Ok(())
    }
    fn player_name(&mut self, player: &Node) -> Result<Option<String>> {
        let p = self.player_of(player)?;
        Ok(Some(self.0.inner.slot(&p).name.clone()))
    }
    fn amount(&mut self, item: &Node) -> Result<i32> {
        Ok(Self::item(item)?.read().amount())
    }
    fn name(&mut self, item: &Node) -> Result<Option<String>> {
        Ok(Self::item(item)?.read().name.clone())
    }
    fn drop_item(&mut self, item: &Node) -> Result<Node> {
        let item = Self::item(item)?;
        let definitions = self.0.inner.items.clone();
        crate::inventory::item::drop_item(&item, &definitions, self)
            .map(Node::Item)
            .map_err(err)
    }
    fn ground_create(
        &mut self,
        item: &Node,
        location: Option<[i32; 3]>,
        player: &Node,
    ) -> Result<()> {
        let item = Self::item(item)?;
        let p = self.player_of(player)?;
        let location = location.map(|[x, y, z]| crate::world::doors::Location::new(x, y, z));
        self.0
            .ground_create(item, location, Some(p.0.clone()))
            .map(|_| ())
            .map_err(err)
    }
    fn world_ticks(&mut self) -> i32 {
        self.0.inner.ticks
    }
    fn set_attribute(&mut self, player: &Node, key: &str, value: i32) -> Result<()> {
        self.canonical(player)?.attributes().set_value(
            key,
            Some(crate::player::attributes::AttributeValue::Int(value)),
        );
        Ok(())
    }
    fn save(&mut self, player: &Node) -> Result<()> {
        // PlayerParser.save(player): the save task (built here, written by
        // the saver thread).
        let p = self.player_of(player)?;
        crate::world::repository::Host::save_immediately(self.0, &p).map_err(err)
    }
    fn send_destroy_item(&mut self, _: &Node, _: i32, _: Option<&str>) -> Result<()> {
        refused("DialogueInterpreter.sendDestroyItem (the dialogue interpreter is not installed)")
    }
    fn add_dialogue_action(&mut self, _: &Node, _: drop::DestroyAction<Node>) -> Result<()> {
        refused("addDialogueAction (the dialogue interpreter is not installed)")
    }
    fn remove_item(&mut self, player: &Node, item: &Node) -> Result<bool> {
        let item = Self::item(item)?;
        self.with_container(player, Container::Inventory, |c, defs, ctx| {
            c.remove(Some(&item), None, true, defs, ctx)
        })
    }
}

// ---------------------------------------------------- EquipHandler

impl equip::Context for Ctx<'_> {
    fn name(&mut self, node: &Node) -> Result<Option<String>> {
        Ok(Self::item(node)?.read().name.clone())
    }
    fn amount(&mut self, item: &Node) -> Result<i32> {
        Ok(Self::item(item)?.read().amount())
    }
    fn equip_listeners(
        &mut self,
        id: i32,
        player: &Node,
        node: &Node,
        equip: bool,
    ) -> Result<bool> {
        // InteractionListeners.run(id, player, node, isEquip).
        let registry = self.0.item_actions.listeners.clone();
        registry
            .run_equip(self, id, player.clone(), node.clone(), equip)
            .map_err(err)
    }
    fn fire_event(
        &mut self,
        _: &crate::definitions::plugin::Handle,
        _: &str,
        _: &Node,
        _: &Node,
    ) -> Result<equip::Fired> {
        refused("an equipment plugin's fireEvent (equipment plugins are content)")
    }
    fn equipment_add(
        &mut self,
        player: &Node,
        item: &Node,
        slot: i32,
        fire: bool,
        from_inventory: bool,
    ) -> Result<bool> {
        let item = Self::item(item)?;
        let p = self.player_of(player)?;
        super::equipment::add(self.0, &p, &item, slot, fire, from_inventory).map_err(err)
    }
    fn equipment_remove(&mut self, player: &Node, item: &Node) -> Result<bool> {
        let item = Self::item(item)?;
        let p = self.player_of(player)?;
        super::equipment::remove(self.0, &p, &item).map_err(err)
    }
    fn inventory_add(&mut self, player: &Node, item: &Node) -> Result<()> {
        let item = Self::item(item)?;
        self.with_container(player, Container::Inventory, |c, defs, ctx| {
            c.add_two(Some(&item), true, defs, ctx)
        })
        .map(|_| ())
    }
    fn maximum_add(&mut self, player: &Node, item: &Node) -> Result<i32> {
        let item = Self::item(item)?;
        self.container(player, Container::Inventory)?
            .maximum_add(Some(&item))
            .map_err(err)
    }
    fn register_glove(&mut self, _: &Node, _: i32) -> Result<()> {
        refused("BrawlingGlovesManager.registerGlove")
    }
    fn dialogue_close(&mut self, player: &Node) -> Result<()> {
        // player.getDialogueInterpreter().close() (super::talk).
        let p = self.player_of(player)?;
        self.0.close_dialogue_of(&p).map(|_| ()).map_err(err)
    }
    fn has_autocast(&mut self, player: &Node) -> Result<bool> {
        let p = self.player_of(player)?;
        self.0
            .inner
            .with_profile(&p, |profile| profile.persisted.spells.autocast.is_some())
            .map_err(err)
    }
    fn clear_autocast(&mut self, _: &Node) -> Result<()> {
        refused("clearing the autocast spell (autocast spells are not installed)")
    }
    fn has_weapon_interface(&mut self, player: &Node) -> Result<bool> {
        let p = self.player_of(player)?;
        Ok(self.0.inner.canonical(&p).weapon.is_some())
    }
    fn select_auto_spell(&mut self, _: &Node, _: i32, _: bool, _: bool) -> Result<()> {
        refused("WeaponInterface.selectAutoSpell (autocast spells are not installed)")
    }
    fn open_autocast_select(&mut self, _: &Node) -> Result<()> {
        refused("WeaponInterface.openAutocastSelect (autocast spells are not installed)")
    }
    fn is_slayer_equipment(&mut self, id: i32) -> Result<bool> {
        // SlayerEquipmentFlags.isSlayerEq: the slayer helm family and the
        // other slayer items it lists are content; none is ported.
        let _ = id;
        self.0
            .inner
            .not_ported
            .default_answer("SlayerEquipmentFlags.isSlayerEq (slayer content not ported: false)");
        Ok(false)
    }
    fn update_slayer_flags(&mut self, _: &Node) -> Result<()> {
        refused("SlayerEquipmentFlags.updateFlags")
    }
    fn dispatch_equip(&mut self, _: &Node, _: i32, _: i32) -> Result<()> {
        self.0
            .inner
            .not_ported
            .skip("ItemEquipEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn dispatch_unequip(&mut self, _: &Node, _: i32, _: i32) -> Result<()> {
        self.0
            .inner
            .not_ported
            .skip("ItemUnequipEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn send_string(&mut self, player: &Node, text: &str, iface: i32, child: i32) -> Result<()> {
        let p = self.player_of(player)?;
        self.0.inner.text(&p, text, iface, child).map_err(err)
    }
}

// ---------------------------------------------------- examine, track finished

impl examine::Context for Ctx<'_> {
    fn item_definition(&mut self, id: i32) -> Result<Option<Definition>> {
        Ok(Some(self.0.inner.items.for_id(id)))
    }
    fn scenery_definition(&mut self, id: i32) -> Result<Option<scenery::Handle>> {
        Ok(Some(self.0.walk.scene.definitions.for_id(id)))
    }
    fn npc_definition(&mut self, id: i32) -> Result<Option<npc::Handle>> {
        // NPCDefinition.forId over the definitions the start-up's NPCs hold.
        match self.0.inner.npcs.rows.values().find(|r| r.id == id) {
            Some(row) => Ok(Some(row.definition.clone())),
            None => refused("NPCDefinition.forId of an NPC id no spawned NPC uses"),
        }
    }
    fn varbit(&mut self, _: i32) -> Result<Option<i32>> {
        refused("VarbitDefinition.forObjectID for scenery examine")
    }
    fn music_looping(&mut self, player: &Node) -> Result<bool> {
        let p = self.player_of(player)?;
        Ok(self.0.inner.canonical(&p).music.looping)
    }
    fn music_replay(&mut self, player: &Node) -> Result<()> {
        let p = self.player_of(player)?;
        Tabs::new(&mut self.0.inner, p)
            .music(|tabs, music| music.replay(tabs))
            .map_err(err)
    }
    fn music_set_playing(&mut self, player: &Node, playing: bool) -> Result<()> {
        let p = self.player_of(player)?;
        self.0.inner.slot_mut(&p).canonical.music.playing = playing;
        Ok(())
    }
}

impl actions::NpcContext for Ctx<'_> {
    fn spawn_location(&mut self, npc: &Node) -> Result<Option<[i32; 3]>> {
        let Node::Npc(h) = npc else {
            return Err(Error::Cast);
        };
        Ok(self
            .0
            .inner
            .npcs
            .rows
            .get(&h.node)
            .and_then(|r| r.properties.spawn.clone())
            .map(|l| l.coordinates()))
    }
    fn config_file_id(&mut self, npc: &Node) -> Result<i32> {
        let Node::Npc(h) = npc else {
            return Err(Error::Cast);
        };
        Ok(self
            .0
            .inner
            .npcs
            .rows
            .get(&h.node)
            .ok_or(Error::Null)?
            .definition
            .read()
            .config_file_id)
    }
    fn plugin_npc(&mut self, _: &Node, _: &Node, _: &OptionHandle) -> Result<bool> {
        // PluginInteractionManager: interaction plugins are content.
        self.0.inner.not_ported.default_answer(
            "PluginInteractionManager.handle for NPCs (content plugins not ported: false)",
        );
        Ok(false)
    }
}

// ---------------------------------------------------- InteractionListeners

impl listeners::Casing for Ctx<'_> {
    fn lowercase_default(&self, s: &[u16]) -> Text {
        text(&lowercase_root(&string(s)))
    }
    fn lowercase_root(&self, s: &[u16]) -> Text {
        text(&lowercase_root(&string(s)))
    }
}

impl listeners::Context for Ctx<'_> {
    type Player = Node;
    type Node = Node;
    type Scripts = crate::scripts::Runtime;
    type Locks = Node;
    type Script = crate::scripts::RuntimeScript;
    type Manager = ();
    type Pulse = crate::world::movement::Movement;
    type Zone = Node;
    type Location = Option<crate::world::doors::Location>;
    fn scripts(&mut self, p: &Node) -> listeners::Result<crate::scripts::Runtime> {
        // player.scripts: the canonical actor's processor.
        let p = self.player_of(p).map_err(lerr)?;
        self.0.inner.state.scripts(&p.0).map_err(lerr)
    }
    fn remove_weak(&mut self, scripts: crate::scripts::Runtime) -> listeners::Result<()> {
        scripts.remove_weak();
        Ok(())
    }
    fn locks(&mut self, p: &Node) -> listeners::Result<Node> {
        Ok(p.clone())
    }
    fn interaction_locked(&mut self, p: Node) -> listeners::Result<bool> {
        self.locked(&p, action_locks::Kind::Interaction)
            .map_err(lerr)
    }
    fn movement_locked(&mut self, p: Node) -> listeners::Result<bool> {
        self.locked(&p, action_locks::Kind::Movement).map_err(lerr)
    }
    fn set_option(&mut self, p: &Node, option: Text) -> listeners::Result<()> {
        // setAttribute(player, "interact:option", option).
        self.canonical(p).map_err(lerr)?.attributes().set_value(
            "interact:option",
            Some(crate::player::attributes::AttributeValue::Text(string(
                &option,
            ))),
        );
        Ok(())
    }
    fn dispatch_on(&mut self, _: &Node, _: &Node, _: Text) -> listeners::Result<()> {
        self.0
            .inner
            .not_ported
            .skip("InteractionEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn dispatch_use(&mut self, _: &Node, _: i32, _: i32) -> listeners::Result<()> {
        self.0
            .inner
            .not_ported
            .skip("UseWithEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn interaction(
        &mut self,
        f: Function,
        distance: i32,
        persist: bool,
    ) -> listeners::Result<crate::scripts::RuntimeScript> {
        // Interaction(handler, distance, persist): only a handler this
        // world runs (super::scripts); the bank booths' and every other
        // content class's stay refused.
        if !super::host::Live::script_installed(&f) {
            return lrefused(
                "a defineInteraction script this world did not install (InteractionListener metadata other than the gathering listeners')",
            );
        }
        Ok(Shared::new(crate::scripts::Script::new_in(
            self.0.inner.state.identity(),
            crate::scripts::Form::Interaction { distance },
            Some(f),
            persist,
        )))
    }
    fn install(
        &mut self,
        scripts: crate::scripts::Runtime,
        n: &Node,
        script: crate::scripts::RuntimeScript,
    ) -> listeners::Result<()> {
        // scripts.setInteractionScript(node, script): the node in the
        // canonical state (a scene object's is the one its walk goes to).
        let target = match n {
            Node::Npc(h) | Node::Player(h) => h.clone(),
            Node::Scenery(object) => self
                .0
                .inner
                .state
                .scenery_target(object.clone())
                .map_err(lerr)?,
            _ => return lrefused("an interaction script whose target is an item"),
        };
        self.0
            .with_walk_host(|state, host| {
                let mut ctx = crate::world::script_runtime::Context::new(state, host, &scripts)?;
                scripts.interact_in(state, &mut ctx, Some(target), Some(&script))
            })
            .map_err(lerr)
    }
    fn manager(&mut self, p: &Node) -> listeners::Result<()> {
        // player.pulseManager: the canonical actor's (super::talk).
        self.player_of(p).map_err(lerr)?;
        Ok(())
    }
    fn movement(
        &mut self,
        a: listeners::Arrival<Node, Node>,
        flag: Option<listeners::Flag>,
        dest: Option<Function>,
    ) -> listeners::Result<crate::world::movement::Movement> {
        self.0.listener_movement(a, flag, dest)
    }
    fn run(&mut self, _: (), pulse: crate::world::movement::Movement) -> listeners::Result<()> {
        self.0.run_listener_movement(pulse)
    }
    fn invoke_on(&mut self, f: Function, p: &Node, n: &Node) -> listeners::Result<bool> {
        if let Some(interaction) = f.value::<crate::world::npc_classes::listeners::Interaction>() {
            return self.0.npc_listener(*interaction, p, n);
        }
        // The global handlers' RequestOption listeners ("trade with", "req
        // assist") and their other on listeners (super::global).
        if let Some(interaction) = f.value::<crate::world::global_handlers::registry::Interaction>()
        {
            if matches!(n, Node::Scenery(_)) {
                return lrefused(
                    "the global handlers' scenery listeners (the bank chest and deposit box)",
                );
            }
            let entity = |n: &Node| match n {
                Node::Player(h) | Node::Npc(h) => Ok(super::global::GNode::Entity(P(h.clone()))),
                _ => Err(listeners::Error::Type),
            };
            let (player, node) = (entity(p)?, entity(n)?);
            return crate::world::global_handlers::registry::run_interaction(
                &mut super::global::GHost { live: self.0 },
                *interaction,
                &player,
                Some(&node),
                None,
            )
            .map_err(lerr);
        }
        // ConsumableListener's "eat" and "drink" (super::eating).
        if f.value::<super::eating::ConsumableListener>().is_some() {
            let player = self.player_of(p).map_err(lerr)?;
            let Node::Item(item) = n else {
                return Err(listeners::Error::Type);
            };
            return self.0.consume(&player, item).map_err(lerr);
        }
        if let Some(shop) = f.value::<super::shopping::ShopListener>() {
            let (Node::Player(player), Node::Npc(npc)) = (p, n) else {
                return Err(listeners::Error::Type);
            };
            return self
                .0
                .shop_listener(shop.0, &P(player.clone()), &P(npc.clone()))
                .map_err(lerr);
        }
        if f.value::<AttackListener>().is_some() {
            return attack_listener(self, p, n).map_err(lerr);
        }
        // SapCollectListener's "empty" and CoalTruckListener's options
        // (super::withobject).
        if let Some(interaction) = f.value::<crate::world::item_on_object::Interaction>() {
            return crate::world::item_on_object::run_option(
                &mut withobject::WithObject(everyday::Everyday(Ctx(self.0))),
                *interaction,
                p,
                n,
            )
            .map_err(lerr);
        }
        // ProspectListener's "prospect" (super::mining).
        if f.value::<mining::Prospect>().is_some() {
            return self.0.prospect_option(p, n).map_err(lerr);
        }
        if let Some(option) = f.value::<GraveListener>().map(|g| g.0) {
            let (Node::Player(player), Node::Npc(npc)) = (p, n) else {
                return Err(listeners::Error::Type);
            };
            return self.0.grave_option(option, player, npc).map_err(lerr);
        }
        match f.value::<Registered>().map(|r| r.0) {
            Some(Listener::Drop) => drop::drop(self, p, n).map_err(lerr),
            Some(Listener::Equip) => {
                equip::handle_equip(self, p, n).map_err(lerr)?;
                Ok(true)
            }
            None => lrefused("a content interaction listener"),
        }
    }
    fn invoke_use(
        &mut self,
        f: Function,
        p: &Node,
        used: &Node,
        with: &Node,
    ) -> listeners::Result<bool> {
        // The installed content use-with lambdas (everyday).
        match self.0.content_use(&f, p, used, with) {
            Some(result) => result.map_err(lerr),
            None => lrefused("a content use-with listener this world did not register"),
        }
    }
    fn predicate(&mut self, _: Function, _: i32, _: i32) -> listeners::Result<bool> {
        lrefused("content use-with predicates")
    }
    fn id(&mut self, n: &Node) -> listeners::Result<i32> {
        Base::id(self, n).map_err(lerr)
    }
    fn is_player(&self, n: &Node) -> bool {
        matches!(n, Node::Player(_))
    }
    fn zone(&mut self, p: &Node) -> listeners::Result<Node> {
        Ok(p.clone())
    }
    fn zone_on(&mut self, z: Node, n: &Node, _: Text, _: i32) -> listeners::Result<bool> {
        // player.zoneMonitor.interact(node, option): the region zones the
        // player is in (none of the installed zones takes an entity option
        // but the wilderness' and the multiway zone's own checks).
        let p = self.player_of(&z).map_err(lerr)?;
        let monitor = self.0.monitor(&p.0);
        let target = match n {
            Node::Npc(h) | Node::Player(h) => h.clone(),
            // A scenery: of the six installed zones (ZoneBuilder.startup's)
            // only WildernessZone and DarkZone override interact, the first
            // for a Revenant NPC and the second for an Item, so every one
            // answers false.
            Node::Scenery(_) => return Ok(false),
            // An item is not a zone target here: with no zone the answer
            // is false, and inside one the call is refused.
            _ if monitor.zones().is_empty() => return Ok(false),
            _ => {
                return lrefused(
                    "ZoneMonitor.interact for an item from a listener inside a region zone",
                );
            }
        };
        monitor
            .interact(self.0, Some(&target), Some(&()))
            .map_err(lerr)
    }
    fn as_item(&mut self, n: &Node) -> listeners::Result<Node> {
        match n {
            Node::Item(_) | Node::Ground(_) => Ok(n.clone()),
            _ => Err(listeners::Error::Type),
        }
    }
    fn zone_use(&mut self, z: Node, _: &Node, _: &Node) -> listeners::Result<bool> {
        let p = self.player_of(&z).map_err(lerr)?;
        let monitor = self.0.monitor(&p.0);
        if monitor.zones().is_empty() {
            // ZoneMonitor.useWith with no zone: false.
            return Ok(false);
        }
        // None of the six installed zones (ZoneBuilder.startup's) overrides
        // handleUseWith, so each answers false. A zone that does needs the
        // inventory item as a canonical node first.
        self.0.inner.not_ported.default_answer(
            "ZoneMonitor.useWith inside a region zone (no installed region zone overrides handleUseWith: false)",
        );
        Ok(false)
    }
    fn location(&mut self, n: &Node) -> listeners::Result<Option<crate::world::doors::Location>> {
        match n {
            Node::Npc(h) | Node::Player(h) => {
                self.0.inner.state.node_location(h.node).map_err(lerr)
            }
            Node::Scenery(s) => Ok(Some(s.read().location.clone())),
            _ => lrefused("the location of an item in listeners"),
        }
    }
    fn face(
        &mut self,
        p: &Node,
        l: Option<crate::world::doors::Location>,
    ) -> listeners::Result<()> {
        // player.faceLocation(node.location).
        let p = self.player_of(p).map_err(lerr)?;
        let masks = self.0.inner.canonical(&p).player.masks();
        masks.face_location(l).map_err(lerr)?;
        Ok(())
    }
}

// ---------------------------------------------------- the live world's calls

impl Live {
    /// DropListener and EquipHandler's `defineListeners`, at start-up.
    pub fn define_item_listeners(&mut self) -> Result<()> {
        drop::define(&mut Ctx(self))?;
        equip::define(&mut Ctx(self))?;
        // AttackListener.defineListeners: flagInstant(), then
        // on(IntType.NPC, "attack").
        let registry = self.item_actions.listeners.clone();
        registry
            .instant_classes
            .write()
            .insert(ATTACK_LISTENER.to_owned());
        let function = Function::new(FunctionKind::On, ATTACK_LISTENER, AttackListener);
        registry
            .add_generics(&Ctx(self), &[text("attack")], Kind::Npc.ordinal(), function)
            .map_err(err)?;
        // GraveController.defineListeners: read, bless, repair, demolish.
        let ids = crate::world::player_death::GraveType::ids();
        for (option, name) in super::graves::GraveOption::ALL {
            let function = Function::new(FunctionKind::On, GRAVE_CONTROLLER, GraveListener(option));
            registry
                .add_ids(
                    &Ctx(self),
                    &ids,
                    Kind::Npc.ordinal(),
                    &[text(name)],
                    function,
                )
                .map_err(err)?;
        }
        Ok(())
    }
    /// The routed packet handlers of `world::packet_handlers`.
    pub fn item_packet(&mut self, player: &P, action: &ClientAction) -> Result<()> {
        let node = Node::Player(player.0.clone());
        let mut ctx = Ctx(self);
        if let Some(a) = actions::ItemAction::from_action(action) {
            return actions::item_action(&mut ctx, &node, a);
        }
        if let Some(a) = actions::NpcAction::from_action(action) {
            return actions::npc_action(&mut ctx, &node, a);
        }
        if let Some(a) = actions::PlayerAction::from_action(action) {
            return actions::player_action(&mut ctx, &node, a);
        }
        match *action {
            ClientAction::ItemExamine { id } => examine::item_examine(&mut ctx, &node, id),
            ClientAction::NpcExamine { id } => examine::npc_examine(&mut ctx, &node, id),
            ClientAction::SceneryExamine { id } => examine::scenery_examine(&mut ctx, &node, id),
            ClientAction::TrackFinished { .. } => examine::track_finished(&mut ctx, &node),
            _ => refused("a packet these handlers do not take"),
        }
    }
    /// `EquipHandler.unequip(player, slot, itemId)`: for the equipment tab's
    /// button (content, not installed here) to call.
    pub fn unequip(&mut self, player: &P, slot: i32, item_id: i32) -> Result<()> {
        let node = Node::Player(player.0.clone());
        equip::unequip(&mut Ctx(self), &node, slot, item_id)
    }
    /// An `InteractPlugin$1` pulse's body, if this world submitted it.
    pub fn item_pulse(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let at = self
            .item_actions
            .pulses
            .iter()
            .position(|(h, _)| h.same(pulse))?;
        let (_, task) = self.item_actions.pulses.remove(at);
        Some(item_option::pulse(&mut Ctx(self), &task))
    }
    /// The drop scripts due for a player at its tick (the script queue's
    /// SOFT entries this world queues).
    pub fn run_drop_scripts(&mut self, player: &P) -> Result<()> {
        let ticks = self.inner.ticks;
        let mut due = Vec::new();
        self.item_actions.drops.retain(|(at, script)| {
            let mine = matches!(&script.player, Node::Player(h) if h.node == player.node());
            if mine && *at <= ticks {
                due.push(script.clone());
                false
            } else {
                true
            }
        });
        for script in due {
            drop::script(&mut Ctx(self), &script)?;
        }
        Ok(())
    }
}

/// `ItemDefinition.hasRequirement(player, wield, message)`.
pub fn has_requirement(
    inner: &mut super::inner::Inner,
    p: &P,
    definition: &Definition,
    wield: bool,
    message: bool,
) -> std::result::Result<bool, String> {
    // world::packet_handlers::requirement (fixtures/wield-requirements).
    let configuration = definition.read().handlers.get("requirements").cloned();
    let levels = inner
        .with_profile(p, |profile| profile.skills.static_levels)
        .map_err(|e| format!("{e:?}"))?;
    let mut messages = Vec::new();
    let result = crate::world::packet_handlers::requirement::has_requirement(
        configuration.as_ref(),
        |skill| levels.get(skill).copied().ok_or(Error::Index(None)),
        wield,
        message,
        |text| {
            messages.push(text.to_owned());
            Ok(())
        },
        lowercase_root,
    )
    .map_err(|e| format!("{e:?}"))?;
    for text in messages {
        inner.message(p, &text).map_err(|e| format!("{e:?}"))?;
    }
    Ok(result)
}

impl Live {
    /// `InteractionListeners.run(id, SCENERY, option, player, node)` of an
    /// `on` listener, deferred from a scenery option packet
    /// (super::scenery): the walk to the object and, at the arrival, the
    /// lambda.
    pub fn scenery_listener_run(
        &mut self,
        player: NodeHandle,
        id: i32,
        option: &[u16],
        object: crate::world::doors::Handle,
    ) -> Result<()> {
        let registry = self.item_actions.listeners.clone();
        registry
            .run(
                &mut Ctx(self),
                id,
                Kind::Scenery,
                option,
                Node::Player(player),
                Node::Scenery(object),
            )
            .map(|_| ())
            .map_err(err)
    }
}
