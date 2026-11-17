//! `processItemAction`, `processNpcAction` and `processPlayerAction`.
use super::{Base, Error, Result, item_option, likely_container, or_null};
use crate::world::interaction_listeners::Kind;
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::actions::ClientAction;

/// `Packet.ItemAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemAction {
    pub opt_index: i32,
    pub item_id: i32,
    pub slot: i32,
    pub iface: i32,
    pub child: i32,
}
/// `Packet.NpcAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NpcAction {
    pub opt_index: i32,
    pub npc_index: i32,
}
/// `Packet.PlayerAction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerAction {
    pub opt_index: i32,
    pub other_index: i32,
}

impl ItemAction {
    pub fn from_action(action: &ClientAction) -> Option<Self> {
        match *action {
            ClientAction::ItemAction {
                opt_index,
                item_id,
                slot,
                iface,
                child,
            } => Some(Self {
                opt_index,
                item_id,
                slot,
                iface,
                child,
            }),
            _ => None,
        }
    }
}
impl NpcAction {
    pub fn from_action(action: &ClientAction) -> Option<Self> {
        match *action {
            ClientAction::NpcAction {
                opt_index,
                npc_index,
            } => Some(Self {
                opt_index,
                npc_index,
            }),
            _ => None,
        }
    }
}
impl PlayerAction {
    pub fn from_action(action: &ClientAction) -> Option<Self> {
        match *action {
            ClientAction::PlayerAction {
                opt_index,
                other_index,
            } => Some(Self {
                opt_index,
                other_index,
            }),
            _ => None,
        }
    }
}

/// `processItemAction`.
pub fn item_action<C: item_option::Context>(
    ctx: &mut C,
    player: &C::Node,
    p: ItemAction,
) -> Result<()> {
    let Some(container) = likely_container(p.iface) else {
        return Ok(());
    };
    let capacity = ctx.capacity(player, container)?;
    if !(0..capacity).contains(&p.slot) {
        return Ok(());
    }
    let Some(item) = ctx.get(player, container, p.slot)? else {
        return Ok(());
    };
    if ctx.id(&item)? != p.item_id {
        return Ok(());
    }
    let interaction = ctx.interaction(&item)?.ok_or(Error::Null)?;
    let Some(option) = ctx.option(&interaction, p.opt_index)? else {
        return Ok(());
    };
    if ctx.interaction_locked(player)? {
        return Ok(());
    }
    // `item.interaction` is read again for the call.
    let interaction = ctx.interaction(&item)?.ok_or(Error::Null)?;
    item_option::handle_item_option(ctx, &interaction, player, &option, container)?;
    ctx.reset_scripts(player)?;
    let id = ctx.id(&item)?;
    let name = or_null(option.read().name());
    ctx.debug(
        player,
        &format!("[ITEM INTERACT] ID: {id}, Slot: {}, Opt: {name}", p.slot),
    )
}

/// `Location.toString()`.
fn location_text(location: Option<[i32; 3]>) -> String {
    match location {
        Some([x, y, z]) => format!("[{x}, {y}, {z}]"),
        None => "null".into(),
    }
}

/// What `processNpcAction` reads from an NPC besides the base services.
pub trait NpcContext: Base {
    /// `npc.properties.spawnLocation`.
    fn spawn_location(&mut self, npc: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `npc.definition.configFileId`.
    fn config_file_id(&mut self, npc: &Self::Node) -> Result<i32>;
    /// `PluginInteractionManager.handle(player, npc, option)`.
    fn plugin_npc(
        &mut self,
        player: &Self::Node,
        npc: &Self::Node,
        option: &crate::world::interaction::OptionHandle,
    ) -> Result<bool>;
}

/// `processNpcAction`.
pub fn npc_action<C: NpcContext>(ctx: &mut C, player: &C::Node, p: NpcAction) -> Result<()> {
    let max = ctx.max_npcs();
    if !(1..max).contains(&p.npc_index) {
        return ctx.clear_minimap(player);
    }
    let Some(npc) = ctx.npc(p.npc_index)? else {
        return ctx.clear_minimap(player);
    };
    let wrapper = ctx.shown_npc(&npc, player)?;
    let interaction = ctx.interaction(&wrapper)?.ok_or(Error::Null)?;
    let Some(option) = ctx.option(&interaction, p.opt_index)? else {
        ctx.invalid(player, &npc)?;
        return ctx.clear_minimap(player);
    };
    let has_wrapper = ctx.id(&wrapper)? != ctx.id(&npc)?;
    ctx.debug(player, "[NPC INTERACT]-------------------")?;
    let id = ctx.id(&wrapper)?;
    ctx.debug(player, &format!("ID: {id}, Index: {}", p.npc_index))?;
    let name = or_null(option.read().name());
    let index = option.read().index();
    ctx.debug(player, &format!("Option: {name}[{index}]"))?;
    let spawn = location_text(ctx.spawn_location(&npc)?);
    ctx.debug(player, &format!("SpawnLoc: {spawn}"))?;
    if has_wrapper {
        let id = ctx.id(&npc)?;
        let varbit = ctx.config_file_id(&npc)?;
        ctx.debug(player, &format!("WrapperID: {id}, Varbit: {varbit}"))?;
    }
    ctx.debug(player, "---------------------------------")?;
    ctx.reset_scripts(player)?;
    let id = ctx.id(&wrapper)?;
    let name = option.read().name();
    // The listeners receive the NPC itself, keyed by the shown NPC's id.
    if ctx.listeners(id, Kind::Npc, name.as_deref(), player, &npc)? {
        return Ok(());
    }
    if ctx.plugin_npc(player, &wrapper, &option)? {
        return Ok(());
    }
    // The NPC's own interaction handles the shown NPC's option.
    let interaction = ctx.interaction(&npc)?.ok_or(Error::Null)?;
    ctx.handle(&interaction, player, &option)
}

/// `processPlayerAction`.
pub fn player_action<C: Base>(ctx: &mut C, player: &C::Node, p: PlayerAction) -> Result<()> {
    let max = ctx.max_players();
    if !(1..max).contains(&p.other_index) {
        return ctx.clear_minimap(player);
    }
    ctx.reset_scripts(player)?;
    let other = ctx.other_player(p.other_index)?;
    let other = match other {
        Some(other) if ctx.active(&other)? => other,
        _ => return ctx.clear_minimap(player),
    };
    let interaction = ctx.interaction(&other)?.ok_or(Error::Null)?;
    let Some(option) = ctx.option(&interaction, p.opt_index)? else {
        return ctx.clear_minimap(player);
    };
    // Kotlin's `lowercase()` is the ROOT locale's.
    let name = lowercase_root(&option.read().name().ok_or(Error::Null)?);
    if !ctx.listeners(-1, Kind::Player, Some(&name), player, &other)? {
        let interaction = ctx.interaction(&other)?.ok_or(Error::Null)?;
        ctx.handle(&interaction, player, &option)?;
    }
    Ok(())
}
