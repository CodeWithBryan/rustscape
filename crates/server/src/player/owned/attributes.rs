//! Attributes on the actual canonical player. Views retain that player; returned
//! object values retain only their own actual fields, traced by the owner graph.
use super::Player;
use crate::player::attributes::{AttributeValue, Error, Result};
#[derive(Clone)]
pub struct Attributes(pub(super) Player);
impl Attributes {
    /// Store the actual value/alias, preserving GameAttributes save-prefix rules.
    /// This does not execute the object. Concrete callback dispatch must validate
    /// its captured world/receiver before changing gameplay state.
    pub fn set_value(&self, key: &str, value: Option<AttributeValue>) {
        self.0.0.write().attributes.set(key, value);
    }
    pub fn get(&self, key: &str) -> Option<AttributeValue> {
        self.0.0.read().attributes.get(key).cloned()
    }
    /// Raw getAttributes().containsKey, including a key whose value is null.
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.0.read().attributes.values.contains_key(key)
    }
    pub fn integer(&self, key: &str, default: i32) -> Result<i32> {
        match self.get(key) {
            None => Ok(default),
            Some(AttributeValue::Int(n)) => Ok(n),
            _ => Err(Error::Type),
        }
    }
    pub fn boolean(&self, key: &str, default: bool) -> Result<bool> {
        match self.get(key) {
            None => Ok(default),
            Some(AttributeValue::Bool(value)) => Ok(value),
            _ => Err(Error::Type),
        }
    }
    pub fn set_integer(&self, key: &str, value: i32) {
        self.0
            .0
            .write()
            .attributes
            .set(key, Some(AttributeValue::Int(value)));
    }
    pub fn set_boolean(&self, key: &str, value: bool) {
        self.0
            .0
            .write()
            .attributes
            .set(key, Some(AttributeValue::Bool(value)));
    }
    pub fn set_null(&self, key: &str) {
        self.0.0.write().attributes.set(key, None);
    }
    pub fn remove(&self, key: &str) {
        self.0.0.write().attributes.remove(key);
    }
}
