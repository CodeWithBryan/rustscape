//! The grave type: GraveController keeps it as the saved attribute
//! `/save:gravetype`, so it reaches the file through `attributes`. Graves
//! themselves are not ported.
use crate::player::{
    attributes::{AttributeValue, GameAttributes},
    save::{Error, GRAVE_TYPES, Result},
};

pub const ATTRIBUTE: &str = "/save:gravetype";

/// updateGraveType's attribute write. Entity.setAttribute then dispatches an
/// AttributeSetEvent, which is the caller's service.
pub fn update(attributes: &mut GameAttributes, ordinal: i32) -> AttributeValue {
    let value = AttributeValue::Int(ordinal);
    attributes.set(ATTRIBUTE, Some(value.clone()));
    value
}
/// getGraveType: `GraveType.values()[getAttribute(player, ATTR_GTYPE, 0)]`.
/// Kotlin unboxes the generic Int through `Number.intValue()`, so any
/// integral attribute is read, a long wrapping to its low 32 bits. A
/// non-numeric value fails the cast. An object attribute is refused natively
/// even when the original would have read it as a Number.
pub fn get(attributes: &GameAttributes) -> Result<i32> {
    let ordinal = match attributes.get(ATTRIBUTE) {
        None => 0,
        Some(AttributeValue::Int(value)) => *value,
        Some(AttributeValue::Long(value)) => *value as i32,
        Some(AttributeValue::Short(value)) => i32::from(*value),
        Some(AttributeValue::Byte(value)) => i32::from(*value),
        Some(_) => return Err(Error::Type),
    };
    if !(0..GRAVE_TYPES).contains(&ordinal) {
        return Err(Error::Index);
    }
    Ok(ordinal)
}
