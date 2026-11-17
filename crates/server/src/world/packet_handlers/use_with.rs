//! `processUseWith`, `NodeUsageEvent` and `UseWithHandler`: its handler
//! registry (`addHandler`, `nodeAllowed`), `run` and the bodies of the three
//! pulses `run` schedules (`UseWithHandler$1` to `$3`).
use super::{Base, Error, NOTHING, NodeKind, Result, likely_container, plane};
use crate::definitions::item::Shared;
use crate::world::interaction_listeners::Kind;
use rs09_protocol::game::actions::ClientAction;
use std::collections::HashMap;

/// `UseWithHandler.ITEM_TYPE`.
pub const ITEM_TYPE: i32 = 0;
/// `UseWithHandler.NPC_TYPE`.
pub const NPC_TYPE: i32 = 1;
/// `UseWithHandler.OBJECT_TYPE`.
pub const OBJECT_TYPE: i32 = 2;
/// `UseWithHandler.PLAYER_TYPE`.
pub const PLAYER_TYPE: i32 = 3;

/// The five use-with packets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseWith {
    Npc {
        item_id: i32,
        npc_index: i32,
        iface: i32,
        slot: i32,
    },
    Player {
        item_id: i32,
        other_index: i32,
        iface: i32,
        slot: i32,
    },
    Item {
        used_id: i32,
        used_with_id: i32,
        used_slot: i32,
        used_with_slot: i32,
        used_iface: i32,
        used_with_iface: i32,
        used_child: i32,
        used_with_child: i32,
    },
    Scenery {
        item_id: i32,
        slot: i32,
        scenery_id: i32,
        x: i32,
        y: i32,
    },
    GroundItem {
        used_id: i32,
        with_id: i32,
        iface: i32,
        child: i32,
        slot: i32,
        x: i32,
        y: i32,
    },
}

impl UseWith {
    pub fn from_action(action: &ClientAction) -> Option<Self> {
        Some(match *action {
            ClientAction::UseWithNpc {
                item_id,
                npc_index,
                iface,
                slot,
            } => Self::Npc {
                item_id,
                npc_index,
                iface,
                slot,
            },
            ClientAction::UseWithPlayer {
                item_id,
                other_index,
                iface,
                slot,
            } => Self::Player {
                item_id,
                other_index,
                iface,
                slot,
            },
            ClientAction::UseWithItem {
                used_id,
                used_with_id,
                used_slot,
                used_with_slot,
                used_iface,
                used_with_iface,
                used_child,
                used_with_child,
            } => Self::Item {
                used_id,
                used_with_id,
                used_slot,
                used_with_slot,
                used_iface,
                used_with_iface,
                used_child,
                used_with_child,
            },
            ClientAction::UseWithScenery {
                item_id,
                slot,
                scenery_id,
                x,
                y,
            } => Self::Scenery {
                item_id,
                slot,
                scenery_id,
                x,
                y,
            },
            ClientAction::UseWithGroundItem {
                used_id,
                with_id,
                iface,
                child,
                slot,
                x,
                y,
            } => Self::GroundItem {
                used_id,
                with_id,
                iface,
                child,
                slot,
                x,
                y,
            },
            _ => return None,
        })
    }
}

/// `NodeUsageEvent(player, 0, used, with)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event<N> {
    pub player: N,
    pub used: N,
    pub with: N,
}

/// A registered `UseWithHandler`: its class's simple name, its mutable
/// `allowedNodes` array, whether it overrides `isDynamic` to true, and the
/// content behind its `handle` (a world handler id, never a label).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandlerData {
    pub name: String,
    pub allowed_nodes: Option<Vec<i32>>,
    pub dynamic: bool,
    pub handler: usize,
}
pub type UseHandler = Shared<HandlerData>;
/// One `HANDLERS` list: an `ArrayList` the pulses hold and walk live.
pub type HandlerList = Shared<Vec<UseHandler>>;

/// `UseWithHandler.HANDLERS`.
#[derive(Default)]
pub struct Registry {
    handlers: HashMap<i32, HandlerList>,
}
impl Registry {
    /// `addHandler(id, type, handler)`.
    pub fn add_handler(&mut self, id: i32, kind: i32, handler: &UseHandler) {
        let key = id | kind.wrapping_shl(16);
        let list = self
            .handlers
            .entry(key)
            .or_insert_with(|| Shared::new(Vec::with_capacity(20)))
            .clone();
        if kind == PLAYER_TYPE {
            let mut data = handler.write();
            match &mut data.allowed_nodes {
                None => data.allowed_nodes = Some(vec![id]),
                // A new array, one longer, with the id last.
                Some(nodes) => nodes.push(id),
            }
        }
        list.write().push(handler.clone());
    }
    /// `HANDLERS.get(key)`.
    pub fn get(&self, key: i32) -> Option<HandlerList> {
        self.handlers.get(&key).cloned()
    }
}

/// `nodeAllowed(nodeId)`: a null `allowedNodes` fails the loop.
pub fn node_allowed(handler: &UseHandler, id: i32) -> Result<bool> {
    let data = handler.read();
    if data.dynamic {
        return Ok(true);
    }
    Ok(data
        .allowed_nodes
        .as_ref()
        .ok_or(Error::Null)?
        .contains(&id))
}

/// The body of a pulse `run` scheduled, with its captures.
#[derive(Clone)]
pub enum Body<N> {
    /// `UseWithHandler$1`: a MovementPulse to the target with no handler.
    Unhandled(Event<N>),
    /// `UseWithHandler$2`: item on item, a `Pulse(1, player, used, with)`.
    Items(Event<N>, HandlerList),
    /// `UseWithHandler$3`: a MovementPulse with the first handler.
    Handlers(Event<N>, HandlerList),
}
impl<N> Body<N> {
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Unhandled(_) => "core.game.interaction.UseWithHandler$1",
            Self::Items(..) => "core.game.interaction.UseWithHandler$2",
            Self::Handlers(..) => "core.game.interaction.UseWithHandler$3",
        }
    }
}

pub trait Context: Base {
    type Pulse;
    /// `RegionManager.getObject(x, y, z, id, shape)`.
    fn object(&mut self, x: i32, y: i32, z: i32, id: i32, shape: i32)
    -> Result<Option<Self::Node>>;
    /// `scenery.getChild(player)`.
    fn scenery_child(
        &mut self,
        scenery: &Self::Node,
        player: &Self::Node,
    ) -> Result<Option<Self::Node>>;
    /// `player.zoneMonitor.useWith(item, node)`.
    fn zone_use_with(
        &mut self,
        player: &Self::Node,
        item: &Self::Node,
        node: &Self::Node,
    ) -> Result<bool>;
    /// `InteractionListeners.run(used, with, type, player)`.
    fn listeners_use(
        &mut self,
        used: &Self::Node,
        with: &Self::Node,
        kind: Kind,
        player: &Self::Node,
    ) -> Result<bool>;
    /// `PluginInteractionManager.handle(player, event)`.
    fn plugin_use(&mut self, player: &Self::Node, event: &Event<Self::Node>) -> Result<bool>;
    /// `HANDLERS.get(key)` of the world's [`Registry`].
    fn handlers(&mut self, key: i32) -> Result<Option<HandlerList>>;
    /// `player.getInterfaceManager().close()`.
    fn close_interface(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.getPulseManager().runUnhandledAction(player, STANDARD)`.
    fn unhandled(&mut self, player: &Self::Node) -> Result<()>;
    /// `new MovementPulse(player, destination[, handler]) { body }`.
    fn movement(
        &mut self,
        player: &Self::Node,
        destination: &Self::Node,
        handler: Option<&UseHandler>,
        body: Body<Self::Node>,
    ) -> Result<Self::Pulse>;
    /// `new Pulse(1, player, used, with) { body }`.
    fn delayed(&mut self, event: &Event<Self::Node>, body: Body<Self::Node>)
    -> Result<Self::Pulse>;
    /// `player.getPulseManager().run(pulse, STANDARD)`.
    fn run_pulse(&mut self, player: &Self::Node, pulse: Self::Pulse) -> Result<()>;
    /// `player.dispatch(new UseWithEvent(used, with))`.
    fn dispatch_use(&mut self, player: &Self::Node, used: i32, with: i32) -> Result<()>;
    /// `handler.handle(event)`.
    fn use_handle(&mut self, handler: &UseHandler, event: &Event<Self::Node>) -> Result<bool>;
    /// `node.getFaceLocation(from)`.
    fn face_location(
        &mut self,
        node: &Self::Node,
        from: Option<[i32; 3]>,
    ) -> Result<Option<[i32; 3]>>;
    /// `player.faceLocation(location)`.
    fn face(&mut self, player: &Self::Node, location: Option<[i32; 3]>) -> Result<()>;
}

fn minimap<C: Base>(ctx: &mut C, player: &C::Node) -> Result<()> {
    ctx.clear_minimap(player)
}

/// `processUseWith`.
pub fn process<C: Context>(ctx: &mut C, player: &C::Node, request: UseWith) -> Result<()> {
    let item: C::Node;
    let node: C::Node;
    let mut child: Option<C::Node> = None;
    let item_id: i32;
    let node_id: i32;
    let kind: Kind;
    match request {
        UseWith::Npc {
            item_id: id,
            npc_index,
            iface,
            slot,
        } => {
            let Some(container) = likely_container(iface) else {
                return minimap(ctx, player);
            };
            item_id = id;
            let Some(found) = ctx.get(player, container, slot)? else {
                return minimap(ctx, player);
            };
            item = found;
            let Some(npc) = ctx.npc(npc_index)? else {
                return minimap(ctx, player);
            };
            child = Some(ctx.shown_npc(&npc, player)?);
            node_id = ctx.id(&npc)?;
            node = npc;
            kind = Kind::Npc;
        }
        UseWith::Scenery {
            item_id: id,
            slot,
            scenery_id,
            x,
            y,
        } => {
            let Some(found) = ctx.get(player, super::Container::Inventory, slot)? else {
                return minimap(ctx, player);
            };
            item = found;
            let z = plane(ctx, player)?;
            let Some(scenery) = ctx.object(x, y, z, scenery_id, -1)? else {
                return minimap(ctx, player);
            };
            child = ctx.scenery_child(&scenery, player)?;
            item_id = id;
            node_id = ctx.id(&scenery)?;
            node = scenery;
            kind = Kind::Scenery;
        }
        UseWith::Item {
            used_id,
            used_with_id,
            used_slot,
            used_with_slot,
            used_iface,
            used_with_iface,
            ..
        } => {
            let Some(used) = likely_container(used_iface) else {
                return Ok(());
            };
            let Some(with) = likely_container(used_with_iface) else {
                return Ok(());
            };
            let Some(found) = ctx.get(player, used, used_slot)? else {
                return Ok(());
            };
            item = found;
            let Some(found) = ctx.get(player, with, used_with_slot)? else {
                return Ok(());
            };
            node = found;
            item_id = used_id;
            node_id = used_with_id;
            kind = Kind::Item;
        }
        UseWith::GroundItem {
            used_id,
            with_id,
            iface,
            slot,
            x,
            y,
            ..
        } => {
            let Some(container) = likely_container(iface) else {
                return Ok(());
            };
            let Some(found) = ctx.get(player, container, slot)? else {
                return Ok(());
            };
            item = found;
            item_id = used_id;
            let z = plane(ctx, player)?;
            let Some(ground) = ctx.ground(with_id, [x, y, z], player)? else {
                return Ok(());
            };
            node = ground;
            node_id = with_id;
            kind = Kind::GroundItem;
        }
        UseWith::Player {
            item_id: id,
            other_index,
            iface,
            slot,
        } => {
            let Some(container) = likely_container(iface) else {
                return minimap(ctx, player);
            };
            let Some(found) = ctx.get(player, container, slot)? else {
                return minimap(ctx, player);
            };
            item = found;
            item_id = id;
            let Some(other) = ctx.other_player(other_index)? else {
                return minimap(ctx, player);
            };
            kind = Kind::Player;
            node_id = ctx.id(&other)?;
            node = other;
        }
    }
    if ctx.id(&item)? != item_id {
        return minimap(ctx, player);
    }
    if ctx.id(&node)? != node_id {
        return minimap(ctx, player);
    }
    if ctx.interaction_locked(player)? || ctx.movement_locked(player)? {
        return minimap(ctx, player);
    }
    ctx.reset_scripts(player)?;
    if ctx.zone_use_with(player, &item, &node)? {
        return Ok(());
    }
    if ctx.listeners_use(&item, &node, kind, player)? {
        return Ok(());
    }
    if let Some(child) = &child {
        if ctx.id(child)? != ctx.id(&node)? && ctx.listeners_use(&item, child, kind, player)? {
            return Ok(());
        }
    }
    let flipped = kind == Kind::Item && ctx.id(&item)? < ctx.id(&node)?;
    let event = if flipped {
        Event {
            player: player.clone(),
            used: node,
            with: item,
        }
    } else {
        Event {
            player: player.clone(),
            used: item,
            with: child.unwrap_or(node),
        }
    };
    if ctx.plugin_use(player, &event)? {
        return Ok(());
    }
    run(ctx, &event)
}

/// `(Item) node`: the cast the original makes before reading an id.
fn item_id<C: Base>(ctx: &mut C, node: &C::Node) -> Result<i32> {
    if !ctx.node_kind(node).is_item() {
        return Err(Error::Cast);
    }
    ctx.id(node)
}

/// `UseWithHandler.run(event)`: every exception is caught and printed.
pub fn run<C: Context>(ctx: &mut C, event: &Event<C::Node>) -> Result<()> {
    fn body<C: Context>(ctx: &mut C, event: &Event<C::Node>) -> Result<()> {
        let player = &event.player;
        ctx.close_interface(player)?;
        let n = &event.with;
        let with_kind = ctx.node_kind(n);
        let handler = match with_kind {
            NodeKind::Item | NodeKind::GroundItem => {
                let used = item_id(ctx, &event.used)?;
                match ctx.handlers(used)? {
                    Some(list) => Some(list),
                    None => {
                        let with = item_id(ctx, n)?;
                        ctx.handlers(with)?
                    }
                }
            }
            NodeKind::Npc => {
                let id = ctx.id(n)?;
                ctx.handlers(id | (NPC_TYPE << 16))?
            }
            NodeKind::Scenery => {
                let id = ctx.id(n)?;
                ctx.handlers(id | (OBJECT_TYPE << 16))?
            }
            NodeKind::Player => {
                let id = item_id(ctx, &event.used)?;
                ctx.handlers(id | (PLAYER_TYPE << 16))?
            }
        };
        let items = with_kind.is_item() && ctx.node_kind(&event.used) != NodeKind::Player;
        let Some(handlers) = handler else {
            if items {
                return ctx.unhandled(player);
            }
            let pulse = ctx.movement(player, n, None, Body::Unhandled(event.clone()))?;
            return ctx.run_pulse(player, pulse);
        };
        if items {
            let pulse = ctx.delayed(event, Body::Items(event.clone(), handlers.clone()))?;
            return ctx.run_pulse(player, pulse);
        }
        // `handler.get(0)`: an empty list fails before construction.
        let first = handlers
            .read()
            .first()
            .cloned()
            .ok_or_else(|| Error::Index(Some("Index 0 out of bounds for length 0".into())))?;
        let pulse = ctx.movement(
            player,
            n,
            Some(&first),
            Body::Handlers(event.clone(), handlers.clone()),
        )?;
        ctx.run_pulse(player, pulse)
    }
    // `catch (Exception e) { e.printStackTrace(); }`.
    let _ = body(ctx, event);
    Ok(())
}

/// The handler at `index` of a live list, if the walk has not ended.
fn at(list: &HandlerList, index: usize) -> Option<UseHandler> {
    list.read().get(index).cloned()
}

/// `getUsedItem()`: the used node if it is an item.
fn used_item<C: Base>(ctx: &mut C, event: &Event<C::Node>) -> Option<C::Node> {
    ctx.node_kind(&event.used)
        .is_item()
        .then(|| event.used.clone())
}

/// A scheduled pulse's `pulse()`. None of the three catches.
pub fn pulse<C: Context>(ctx: &mut C, body: &Body<C::Node>) -> Result<bool> {
    match body {
        Body::Unhandled(event) => {
            let player = &event.player;
            ctx.debug(player, "Unhandled use with interaction:")?;
            let used = ctx.text(Some(&event.used))?;
            ctx.debug(player, &format!("Used: {used}"))?;
            let with = ctx.text(Some(&event.with))?;
            ctx.debug(player, &format!("With: {with}"))?;
            ctx.packet_message(player, NOTHING)?;
            Ok(true)
        }
        Body::Items(event, handlers) => {
            let player = &event.player;
            let used = ctx.id(&event.used)?;
            let with = ctx.id(&event.with)?;
            ctx.dispatch_use(player, used, with)?;
            let mut handled = false;
            ctx.close_interface(player)?;
            let mut index = 0;
            while let Some(h) = at(handlers, index) {
                index += 1;
                let with = item_id(ctx, &event.with)?;
                let skip = (!node_allowed(&h, with)? && {
                    let used = used_item(ctx, event).ok_or(Error::Null)?;
                    let used = ctx.id(&used)?;
                    !node_allowed(&h, used)?
                }) || !ctx.use_handle(&h, event)?;
                if skip {
                    continue;
                }
                let name = h.read().name.clone();
                let used = used_item(ctx, event);
                let used = ctx.text(used.as_ref())?;
                let with = ctx.text(Some(&event.with))?;
                ctx.debug(
                    player,
                    &format!("Handler={name}, used item={used}, used with={with}"),
                )?;
                handled = true;
                break;
            }
            if !handled {
                let used = used_item(ctx, event);
                let used = ctx.text(used.as_ref())?;
                ctx.debug(player, &format!("Handler=none, used item={used}"))?;
                let with = ctx.text(Some(&event.with))?;
                ctx.debug(player, &format!("used with={with}"))?;
                ctx.packet_message(player, NOTHING)?;
            }
            Ok(true)
        }
        Body::Handlers(event, handlers) => {
            let player = &event.player;
            let used = ctx.id(&event.used)?;
            let with = ctx.id(&event.with)?;
            ctx.dispatch_use(player, used, with)?;
            let from = ctx.location(player)?;
            let face = ctx.face_location(&event.with, from)?;
            ctx.face(player, face)?;
            let mut handled = false;
            // `(Item) event.getUsed()`.
            if !ctx.node_kind(&event.used).is_item() {
                return Err(Error::Cast);
            }
            let used = event.used.clone();
            let mut index = 0;
            while let Some(h) = at(handlers, index) {
                index += 1;
                let id = ctx.id(&used)?;
                if !node_allowed(&h, id)? || !ctx.use_handle(&h, event)? {
                    continue;
                }
                let name = h.read().name.clone();
                let used = used_item(ctx, event);
                let used = ctx.text(used.as_ref())?;
                let with = ctx.text(Some(&event.with))?;
                ctx.debug(
                    player,
                    &format!("Handler={name}, used item={used}, used with={with}"),
                )?;
                handled = true;
                break;
            }
            if !handled {
                let used = used_item(ctx, event);
                let used = ctx.text(used.as_ref())?;
                let with = ctx.text(Some(&event.with))?;
                ctx.debug(
                    player,
                    &format!("Handler=none, used item={used}, used with={with}"),
                )?;
                ctx.packet_message(player, NOTHING)?;
            }
            Ok(true)
        }
    }
}
