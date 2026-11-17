//! `processComponentUseWith`: a spell (or a familiar special) used on a
//! ground item, player, scenery, NPC or inventory item, with SpellUtils'
//! `getBookFromInterface` and `validateSpellbookInterface`.
use super::{Base, Container, Error, Result, plane};
use crate::world::spells::SpellBook;
use rs09_protocol::game::actions::ClientAction;

/// `SpellListener.NPC`.
pub const NPC: i32 = -1;
/// `SpellListener.OBJECT`.
pub const OBJECT: i32 = -2;
/// `SpellListener.ITEM`.
pub const ITEM: i32 = -3;
/// `SpellListener.PLAYER`.
pub const PLAYER: i32 = -4;
/// `SpellListener.GROUND_ITEM`.
pub const GROUND_ITEM: i32 = -6;
/// The familiar special interface.
pub const FAMILIAR_DETAILS_662: i32 = 662;

/// The five component (interface-on-node) packets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentAction {
    GroundItem {
        iface: i32,
        child: i32,
        slot: i32,
        item_id: i32,
        x: i32,
        y: i32,
    },
    Player {
        other_index: i32,
        iface: i32,
        child: i32,
    },
    Scenery {
        iface: i32,
        child: i32,
        scenery_id: i32,
        x: i32,
        y: i32,
    },
    Npc {
        iface: i32,
        child: i32,
        npc_index: i32,
    },
    Item {
        iface: i32,
        child: i32,
        item_id: i32,
        slot: i32,
    },
}

impl ComponentAction {
    pub fn from_action(action: &ClientAction) -> Option<Self> {
        Some(match *action {
            ClientAction::ComponentGroundItemAction {
                iface,
                child,
                slot,
                item_id,
                x,
                y,
            } => Self::GroundItem {
                iface,
                child,
                slot,
                item_id,
                x,
                y,
            },
            ClientAction::ComponentPlayerAction {
                other_index,
                iface,
                child,
            } => Self::Player {
                other_index,
                iface,
                child,
            },
            ClientAction::ComponentSceneryAction {
                iface,
                child,
                scenery_id,
                x,
                y,
            } => Self::Scenery {
                iface,
                child,
                scenery_id,
                x,
                y,
            },
            ClientAction::ComponentNpcAction {
                iface,
                child,
                npc_index,
            } => Self::Npc {
                iface,
                child,
                npc_index,
            },
            ClientAction::ComponentItemAction {
                iface,
                child,
                item_id,
                slot,
            } => Self::Item {
                iface,
                child,
                item_id,
                slot,
            },
            _ => return None,
        })
    }
}

pub trait Context: Base {
    /// `RegionManager.getObject(x, y, z)`.
    fn object_at(&mut self, x: i32, y: i32, z: i32) -> Result<Option<Self::Node>>;
    /// `player.getAttribute(key, default)` for an `Int`.
    fn attribute_int(&mut self, player: &Self::Node, key: &str, default: i32) -> Result<i32>;
    /// `GameWorld.ticks`.
    fn ticks(&mut self) -> i32;
    /// `player.spellBookManager.spellBook`.
    fn spell_book(&mut self, player: &Self::Node) -> Result<i32>;
    /// `SpellListeners.run(button, type, book, player, node)`.
    fn spell_listeners(
        &mut self,
        button: i32,
        kind: i32,
        book: &str,
        player: &Self::Node,
        node: &Self::Node,
    ) -> Result<()>;
    /// `MagicSpell.castSpell(player, book, spellId, target)`
    /// (`world::spells::cast::cast_spell`); its result is discarded.
    fn cast_spell(
        &mut self,
        player: &Self::Node,
        book: SpellBook,
        spell: i32,
        target: &Self::Node,
    ) -> Result<()>;
    /// `player.familiarManager.hasFamiliar()`.
    fn has_familiar(&mut self, player: &Self::Node) -> Result<bool>;
    /// `familiar.executeSpecialMove(FamiliarSpecial(target, iface, child, item))`.
    fn familiar_special(
        &mut self,
        player: &Self::Node,
        target: &Self::Node,
        iface: i32,
        child: i32,
        item: Option<&Self::Node>,
    ) -> Result<()>;
}

/// `SpellUtils.getBookFromInterface(id)`.
pub fn book_from_interface(id: i32) -> &'static str {
    match id {
        192 => "modern",
        193 => "ancient",
        430 => "lunar",
        _ => "none",
    }
}

/// `SpellUtils.validateSpellbookInterface(player, id)`.
pub fn validate_spellbook_interface<C: Context>(
    ctx: &mut C,
    player: &C::Node,
    id: i32,
) -> Result<bool> {
    Ok(id == ctx.spell_book(player)?)
}

/// `processComponentUseWith`.
pub fn process<C: Context>(ctx: &mut C, player: &C::Node, request: ComponentAction) -> Result<()> {
    let iface: i32;
    let child: i32;
    let target: C::Node;
    let target_id: i32;
    let kind: i32;
    match request {
        ComponentAction::GroundItem {
            iface: i,
            child: c,
            item_id,
            x,
            y,
            ..
        } => {
            child = c;
            iface = i;
            let z = plane(ctx, player)?;
            let Some(found) = ctx.ground(item_id, [x, y, z], player)? else {
                ctx.clear_minimap(player)?;
                return ctx.api_message(player, "Too late!");
            };
            target = found;
            target_id = item_id;
            kind = GROUND_ITEM;
        }
        ComponentAction::Player {
            other_index,
            iface: i,
            child: c,
        } => {
            child = c;
            iface = i;
            let Some(found) = ctx.other_player(other_index)? else {
                return ctx.clear_minimap(player);
            };
            target_id = ctx.id(&found)?;
            target = found;
            kind = PLAYER;
        }
        ComponentAction::Scenery {
            iface: i,
            child: c,
            scenery_id,
            x,
            y,
        } => {
            child = c;
            iface = i;
            let z = plane(ctx, player)?;
            let Some(found) = ctx.object_at(x, y, z)? else {
                return ctx.clear_minimap(player);
            };
            target = found;
            target_id = scenery_id;
            kind = OBJECT;
        }
        ComponentAction::Npc {
            iface: i,
            child: c,
            npc_index,
        } => {
            let max = ctx.max_npcs();
            if !(1..max).contains(&npc_index) {
                return ctx.clear_minimap(player);
            }
            child = c;
            iface = i;
            let Some(found) = ctx.npc(npc_index)? else {
                return ctx.clear_minimap(player);
            };
            target_id = ctx.id(&found)?;
            target = found;
            kind = NPC;
        }
        ComponentAction::Item {
            iface: i,
            child: c,
            item_id,
            slot,
        } => {
            child = c;
            iface = i;
            let Some(found) = ctx.get(player, Container::Inventory, slot)? else {
                return Ok(());
            };
            target = found;
            target_id = item_id;
            kind = ITEM;
        }
    }
    if target_id != ctx.id(&target)? {
        return Ok(());
    }
    if ctx.attribute_int(player, "magic:delay", -1)? > ctx.ticks() {
        return Ok(());
    }
    if [192, 193, 430].contains(&iface) && !validate_spellbook_interface(ctx, player, iface)? {
        return Ok(());
    }
    let book = book_from_interface(iface);
    if book != "none" {
        ctx.spell_listeners(child, kind, book, player, &target)?;
    }
    match iface {
        430 | 192 | 193 => {
            let book = SpellBook::for_interface(iface).ok_or(Error::Null)?;
            ctx.cast_spell(player, book, child, &target)?;
        }
        FAMILIAR_DETAILS_662 => {
            if ctx.has_familiar(player)? {
                let item = ctx.node_kind(&target).is_item().then(|| target.clone());
                ctx.familiar_special(player, &target, iface, child, item.as_ref())?;
            } else {
                ctx.message(player, Some("You don't have a familiar."))?;
            }
        }
        _ => {}
    }
    Ok(())
}
