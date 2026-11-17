//! `HintIconManager` with `HintIconContext` and the `HintIcon` packet: eight
//! hint icon slots, the register overloads, removal, clear and the free
//! slot. Each register builds the context, writes the packet to the
//! context's player and only then stores the context, so an out-of-range
//! slot fails after its packet was sent. A removal sends the stored context
//! with target type 0; contexts are shared objects, so a change made through
//! `getIcon` shows in the removal packet.
use super::Result;
use crate::{definitions::item::Shared, world::player_managers::Error};
use rs09_protocol::game::Frame;

/// `MAXIMUM_SIZE`.
pub const MAXIMUM_SIZE: usize = 8;
/// `DEFAULT_ARROW`.
pub const DEFAULT_ARROW: i32 = 1;
/// `DEFAULT_MODEL`.
pub const DEFAULT_MODEL: i32 = -1;
/// `ARROW_CIRCLE_MODEL`.
pub const ARROW_CIRCLE_MODEL: i32 = 40497;
/// The HintIcon packet's opcode.
pub const OPCODE: u8 = 217;

/// A hint icon target: a player, another entity (an NPC) or a plain node.
#[derive(Clone, Debug)]
pub enum Target<E, N> {
    Player(E),
    Entity(E),
    Node(N),
}

/// `HintIconContext`.
#[derive(Clone, Debug)]
pub struct HintIconContext<P> {
    pub player: P,
    pub slot: i32,
    pub target_type: i32,
    pub arrow_id: i32,
    /// The entity's index, -1 for a node.
    pub index: i32,
    pub model_id: i32,
    /// The node's location, None for an entity (or a node without one).
    pub location: Option<[i32; 3]>,
    pub height: i32,
}

/// A stored context: the manager's slot and every `getIcon` view share it.
pub type Icon<P> = Shared<HintIconContext<P>>;

/// The manager's slots.
#[derive(Clone, Debug)]
pub struct HintIconManager<P> {
    pub icons: [Option<Icon<P>>; MAXIMUM_SIZE],
}
impl<P> Default for HintIconManager<P> {
    fn default() -> Self {
        Self {
            icons: std::array::from_fn(|_| None),
        }
    }
}
impl<P> HintIconManager<P> {
    /// `freeSlot()`: the first empty slot, or -1.
    pub fn free_slot(&self) -> i32 {
        self.icons
            .iter()
            .position(Option::is_none)
            .map_or(-1, |slot| slot as i32)
    }
    /// `getIcon(slot)`.
    pub fn icon(&self, slot: i32) -> Result<Option<Icon<P>>> {
        let slot = usize::try_from(slot).map_err(|_| Error::Index)?;
        self.icons.get(slot).cloned().ok_or(Error::Index)
    }
}

/// Every call HintIconManager and the packet make outside these classes.
pub trait Host {
    type Player: Clone;
    type Entity: Clone;
    type Node: Clone;
    /// `player.getHintIconManager()`.
    fn hint_icons(&mut self, player: &Self::Player) -> Result<&mut HintIconManager<Self::Player>>;
    /// `entity.getIndex()`.
    fn index(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `node.getLocation()`.
    fn node_location(&mut self, node: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// The context player's session: the frame with the zero ISAAC word.
    fn send(&mut self, player: &Self::Player, frame: Frame) -> Result<()>;
}

/// The HintIcon packet of a context.
pub fn frame<P>(context: &HintIconContext<P>) -> Frame {
    let mut payload = vec![
        (context.slot.wrapping_shl(6) | context.target_type) as u8,
        context.arrow_id as u8,
    ];
    let short = |payload: &mut Vec<u8>, value: i32| {
        payload.extend_from_slice(&(value as i16).to_be_bytes())
    };
    if context.arrow_id > 0 {
        if context.target_type == 1 || context.target_type == 10 {
            short(&mut payload, context.index);
            short(&mut payload, 0);
            payload.push(0);
        } else if let Some([x, y, _]) = context.location {
            short(&mut payload, x);
            short(&mut payload, y);
            payload.push(context.height as u8);
        } else {
            short(&mut payload, 0);
            short(&mut payload, 0);
            payload.push(0);
        }
        short(&mut payload, context.model_id);
    }
    Frame {
        opcode: OPCODE,
        payload,
    }
}

type HTarget<H> = Target<<H as Host>::Entity, <H as Host>::Node>;

/// The seven-argument constructor: an entity's index, otherwise the node's
/// location (a null target fails here).
#[allow(clippy::too_many_arguments)]
fn context<H: Host>(
    h: &mut H,
    player: &H::Player,
    slot: i32,
    arrow_id: i32,
    target_type: i32,
    target: Option<&HTarget<H>>,
    model_id: i32,
    height: i32,
) -> Result<HintIconContext<H::Player>> {
    let (index, location) = match target.ok_or(Error::Null)? {
        Target::Player(e) | Target::Entity(e) => (h.index(e)?, None),
        Target::Node(n) => (-1, h.node_location(n)?),
    };
    Ok(HintIconContext {
        player: player.clone(),
        slot,
        target_type,
        arrow_id,
        index,
        model_id,
        location,
        height,
    })
}

/// `instanceof Entity ? (instanceof Player ? 10 : 1) : 2`.
fn target_type<E, N>(target: Option<&Target<E, N>>) -> i32 {
    match target {
        Some(Target::Player(_)) => 10,
        Some(Target::Entity(_)) => 1,
        _ => 2,
    }
}

/// Sends the context to its player, then stores it in `player`'s manager.
fn publish<H: Host>(
    h: &mut H,
    player: &H::Player,
    slot: i32,
    icon: HintIconContext<H::Player>,
) -> Result<i32> {
    h.send(&icon.player.clone(), frame(&icon))?;
    let manager = h.hint_icons(player)?;
    let index = usize::try_from(slot)
        .ok()
        .filter(|s| *s < MAXIMUM_SIZE)
        .ok_or(Error::Index)?;
    manager.icons[index] = Some(Shared::new(icon));
    Ok(slot)
}

fn free_slot<H: Host>(h: &mut H, player: &H::Player) -> Result<i32> {
    Ok(h.hint_icons(player)?.free_slot())
}

/// `registerHintIcon(player, target)`.
pub fn register<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: Option<&HTarget<H>>,
) -> Result<i32> {
    let slot = free_slot(h, player)?;
    register_slot(h, player, target, DEFAULT_ARROW, DEFAULT_MODEL, slot)
}

/// `registerHeightHintIcon(player, target, height)`.
pub fn register_height<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: Option<&HTarget<H>>,
    height: i32,
) -> Result<i32> {
    let slot = free_slot(h, player)?;
    register_slot_height(
        h,
        player,
        target,
        DEFAULT_ARROW,
        DEFAULT_MODEL,
        slot,
        height,
    )
}

/// `registerHintIcon(player, target, arrowId)`.
pub fn register_arrow<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: Option<&HTarget<H>>,
    arrow_id: i32,
) -> Result<i32> {
    let slot = free_slot(h, player)?;
    register_slot(h, player, target, arrow_id, DEFAULT_MODEL, slot)
}

/// `registerHintIcon(player, target, arrowId, modelId)`.
pub fn register_model<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: Option<&HTarget<H>>,
    arrow_id: i32,
    model_id: i32,
) -> Result<i32> {
    let slot = free_slot(h, player)?;
    register_slot(h, player, target, arrow_id, model_id, slot)
}

/// `registerHintIcon(player, target, arrowId, modelId, slot)`: -1 for a
/// negative slot, 0 (not the slot) for a null target; the context's height
/// is 0 and its type follows the target.
pub fn register_slot<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: Option<&HTarget<H>>,
    arrow_id: i32,
    model_id: i32,
    slot: i32,
) -> Result<i32> {
    if slot < 0 {
        return Ok(-1);
    }
    if target.is_none() {
        return Ok(0);
    }
    let mut icon = context(h, player, slot, arrow_id, -1, target, model_id, 0)?;
    icon.target_type = target_type(target);
    publish(h, player, slot, icon)
}

/// `registerHintIcon(player, target, arrowId, modelId, slot, height)`.
pub fn register_slot_height<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: Option<&HTarget<H>>,
    arrow_id: i32,
    model_id: i32,
    slot: i32,
    height: i32,
) -> Result<i32> {
    let kind = target_type(target);
    register_typed(h, player, target, arrow_id, model_id, slot, height, kind)
}

/// `registerHintIcon(player, target, arrowId, modelId, slot, height,
/// targetType)`.
#[allow(clippy::too_many_arguments)]
pub fn register_typed<H: Host>(
    h: &mut H,
    player: &H::Player,
    target: Option<&HTarget<H>>,
    arrow_id: i32,
    model_id: i32,
    slot: i32,
    height: i32,
    target_type: i32,
) -> Result<i32> {
    if slot < 0 {
        return Ok(-1);
    }
    let icon = context(
        h,
        player,
        slot,
        arrow_id,
        target_type,
        target,
        model_id,
        height,
    )?;
    publish(h, player, slot, icon)
}

/// `removeHintIcon(player, slot)`: the stored context goes out with target
/// type 0, then the slot is emptied.
pub fn remove<H: Host>(h: &mut H, player: &H::Player, slot: i32) -> Result<()> {
    if slot < 0 {
        return Ok(());
    }
    let Some(icon) = h.hint_icons(player)?.icon(slot)? else {
        return Ok(());
    };
    let (owner, packet) = {
        let mut context = icon.write();
        context.target_type = 0;
        (context.player.clone(), frame(&context))
    };
    h.send(&owner, packet)?;
    h.hint_icons(player)?.icons[slot as usize] = None;
    Ok(())
}

/// `clear()` of `owner`'s manager: each filled slot, read as the loop
/// reaches it, removed through its context's player.
pub fn clear<H: Host>(h: &mut H, owner: &H::Player) -> Result<()> {
    for slot in 0..MAXIMUM_SIZE {
        let icon = h.hint_icons(owner)?.icons[slot].clone();
        if let Some(icon) = icon {
            let player = icon.read().player.clone();
            remove(h, &player, slot as i32)?;
        }
    }
    Ok(())
}
