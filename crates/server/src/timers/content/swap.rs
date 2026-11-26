//! SpellbookSwap: the Lunar spellbook swap's timer, its three hooks and its
//! logout listener.
use super::{Event, EventClass, Hook, Host, Kind, Logout, Natives, Tab};
use crate::timers::{Error, Handle, Manager, Result};
use crate::world::spells::book::{SpellBook, SpellbookChangeSource};

/// The spellbook swap selection dialogue's id.
pub const SWAP_DIALOGUE: i32 = 3264731;
/// The logout listener key.
pub const LOGOUT_KEY: &str = "spellbook:swap";

pub(super) fn on_register<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<()> {
    let entity = ctx.entity.clone();
    ctx.host.hook(
        &entity,
        EventClass::SpellCast,
        Hook::SpellCast(timer.clone()),
    )?;
    ctx.host.hook(
        &entity,
        EventClass::SpellbookChanged,
        Hook::SpellbookChange(timer.clone()),
    )?;
    ctx.host.hook(
        &entity,
        EventClass::DialogueClosed,
        Hook::DialogueClose(timer.clone()),
    )?;
    if ctx.is_player() {
        ctx.host
            .register_logout_listener(&entity, LOGOUT_KEY, Logout(timer.clone()))?;
    }
    Ok(())
}

pub(super) fn on_removal<H: Host>(ctx: &mut Natives<'_, H>, timer: &Handle) -> Result<()> {
    let entity = ctx.entity.clone();
    ctx.host.unhook(&entity, &Hook::SpellCast(timer.clone()))?;
    ctx.host
        .unhook(&entity, &Hook::SpellbookChange(timer.clone()))?;
    ctx.host
        .unhook(&entity, &Hook::DialogueClose(timer.clone()))?;
    if ctx.is_player() {
        ctx.host.clear_logout_listener(&entity, LOGOUT_KEY)?;
    }
    Ok(())
}

fn book<H: Host>(ctx: &mut Natives<'_, H>) -> Option<SpellBook> {
    let entity = ctx.entity.clone();
    SpellBook::for_interface(*ctx.host.spell_book(&entity))
}

/// `revertSpellbook(entity)`.
pub(super) fn revert<H: Host>(ctx: &mut Natives<'_, H>, manager: &mut Manager) -> Result<()> {
    if !ctx.is_player() {
        return Ok(());
    }
    if book(ctx) == Some(SpellBook::Lunar) {
        return Ok(());
    }
    let player = ctx.entity.clone();
    let weapon = ctx.host.weapon_interface(&player)?;
    if let Some(weapon) = &weapon
        && ctx.host.autocast_set(&player)?
    {
        ctx.host
            .select_auto_spell(&player, weapon, -1, true, true)?;
    }
    if ctx.host.attribute_flag(&player, "autocast_select", false)? {
        ctx.host.remove_attribute(&player, "autocast_select")?;
        ctx.host.remove_attribute(&player, "autocast_component")?;
        if let Some(weapon) = &weapon {
            ctx.host.open_tab(&player, Tab::Weapon(weapon))?;
        }
    }
    // SpellbookChangeEvent's non-null parameter rejects a field naming no book.
    let old = book(ctx).ok_or(Error::Null)?;
    let registry = ctx.registry.clone();
    ctx.host.dispatch(
        &player,
        Event::SpellbookChange {
            old,
            new: SpellBook::Lunar,
            source: SpellbookChangeSource::SpellbookSwapRestore,
        },
        manager,
        &registry,
    )?;
    // spellBookManager.setSpellBook(LUNAR), then update(player).
    *ctx.host.spell_book(&player) = SpellBook::Lunar.interface_id();
    let field = *ctx.host.spell_book(&player);
    ctx.host.open_tab(&player, Tab::Book(field))?;
    let ordinal = book(ctx).ok_or(Error::Null)?.ordinal();
    ctx.host.set_varbit(&player, 357, ordinal, false)
}

fn remove_swap<H: Host>(ctx: &mut Natives<'_, H>, manager: &mut Manager) -> Result<()> {
    ctx.remove_class(manager, Kind::SpellbookSwap, &mut ())
}

/// `spellCastHook.process`.
pub(super) fn spell_cast<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    spell_book: SpellBook,
    spell: i32,
) -> Result<()> {
    if spell_book == SpellBook::Lunar && spell == 12 {
        return Ok(());
    }
    revert(ctx, manager)?;
    remove_swap(ctx, manager)
}

/// `spellBookChangeHook.process`.
pub(super) fn book_change<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    source: SpellbookChangeSource,
) -> Result<()> {
    if source != SpellbookChangeSource::SpellbookSwapCast
        && source != SpellbookChangeSource::SpellbookSwapRestore
    {
        remove_swap(ctx, manager)?;
    }
    Ok(())
}

/// `dialogueCloseHook.process`.
pub(super) fn dialogue_close<H: Host>(
    ctx: &mut Natives<'_, H>,
    manager: &mut Manager,
    dialogue: &H::Dialogue,
) -> Result<()> {
    let ids = ctx.host.dialogue_ids(dialogue)?;
    if !ids.is_some_and(|ids| ids.contains(&SWAP_DIALOGUE)) {
        return Ok(());
    }
    if !ctx.is_player() {
        return Ok(());
    }
    if book(ctx) == Some(SpellBook::Lunar) {
        let player = ctx.entity.clone();
        ctx.host.remove_attribute(&player, "spell:runes")?;
        remove_swap(ctx, manager)?;
    }
    Ok(())
}
