//! Original Player.hasItem evaluation order over captured container receivers.
use crate::inventory::{Result, container::Container, item::Handle};
pub trait Containers {
    fn inventory(&mut self) -> Result<Container>;
    fn bank(&mut self) -> Result<Container>;
    fn equipment(&mut self) -> Result<Container>;
}
pub fn has_item(item: Option<&Handle>, ctx: &mut impl Containers) -> Result<bool> {
    Ok(ctx.inventory()?.contains_item(item)?
        || ctx.bank()?.contains_item(item)?
        || ctx.equipment()?.contains_item(item)?)
}
