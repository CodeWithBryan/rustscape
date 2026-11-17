//! ContentAPI's varp and varbit functions: thin wrappers over
//! [`crate::player::variables`], which holds the original bodies (variables
//! corpus) over the player's real varp maps. `Context` is that module's host.
pub use crate::player::variables::{Context, Error, Result, Saved, Values};
use crate::{definitions::varbit, player::variables, world::player_walk::NodeHandle};

/// `getVarp(player, varpIndex)`: 0 when unset.
pub fn get_varp(ctx: &mut impl Context, p: &NodeHandle, id: i32) -> Result<i32> {
    variables::get_varp(ctx, Some(p), id)
}

/// `getVarbit(player, varbitId)`.
pub fn get_varbit(ctx: &mut impl Context, p: &NodeHandle, id: i32) -> Result<i32> {
    variables::get_varbit(ctx, Some(p), id)
}

/// `getVarbit(player, def)`.
pub fn get_varbit_definition(
    ctx: &mut impl Context,
    p: &NodeHandle,
    def: Option<&varbit::Handle>,
) -> Result<i32> {
    variables::get_varbit_definition(ctx, Some(p), def)
}

/// `setVarp(player, varpIndex, value, save = false)`.
pub fn set_varp(
    ctx: &mut impl Context,
    p: &NodeHandle,
    id: i32,
    value: i32,
    save: bool,
) -> Result<()> {
    variables::set_varp(ctx, Some(p), id, value, save)
}

/// `saveVarp(player, varpIndex)`.
pub fn save_varp(ctx: &mut impl Context, p: &NodeHandle, id: i32) -> Result<()> {
    variables::save_varp(ctx, Some(p), id)
}

/// `unsaveVarp(player, varpIndex)`.
pub fn unsave_varp(ctx: &mut impl Context, p: &NodeHandle, id: i32) -> Result<()> {
    variables::unsave_varp(ctx, Some(p), id)
}

/// `setVarbit(player, varbitId, value, save = false)`: a missing definition
/// is the context's `missing_definition` (ContentAPI's `logWithStack`).
pub fn set_varbit(
    ctx: &mut impl Context,
    p: &NodeHandle,
    id: i32,
    value: i32,
    save: bool,
) -> Result<()> {
    variables::set_varbit(ctx, Some(p), id, value, save)
}

/// `setVarbit(player, def, value, save = false)`.
pub fn set_varbit_definition(
    ctx: &mut impl Context,
    p: &NodeHandle,
    def: Option<&varbit::Handle>,
    value: i32,
    save: bool,
) -> Result<()> {
    variables::set_varbit_definition(ctx, Some(p), def, value, save)
}

/// `setVarc(player, varc, value)`: the index is narrowed to a short.
pub fn set_varc(ctx: &mut impl Context, p: &NodeHandle, id: i32, value: i32) -> Result<()> {
    variables::set_varc(ctx, Some(p), id, value)
}

/// `reinitVarps(player)`: every varp sent again in the map's order with its
/// save flag.
pub fn reinit_varps(ctx: &mut impl Context, p: &NodeHandle) -> Result<()> {
    variables::reinit(ctx, Some(p))
}
