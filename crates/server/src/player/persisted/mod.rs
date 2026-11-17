//! The persisted state of the player systems the save file reaches through
//! `save::Host`: what a character file holds for each, how a new character
//! starts, and the original save and parse methods of each system. Nothing
//! here ports the systems' gameplay. [`host::Native`] answers the ported
//! `save::Host` sections from this state and forwards the rest to a caller's
//! [`host::Remaining`], whose services stay required and have no defaults.
//!
//! House loading is not ported: its parse method builds rooms from content
//! registries and stays a required service. Familiar loading is
//! `world::familiars::Manager::parse`, on this module's shared pet store; the
//! save host forwards `parse_familiars` to it where a world installs it.
pub mod diaries;
pub mod emotes;
pub mod familiars;
pub mod graves;
pub mod host;
pub mod house;
pub mod music;
pub mod poh_storage;
pub mod pouches;
pub mod quests;
pub mod spells;

use super::save::{Error, Result};
use rs09_game_core::{java_json::Json, java_map::int_key_layout};
use std::collections::BTreeMap;

/// A Java `HashMap<Integer, V>` that is put into, read and cleared, never
/// removed from one key at a time. Iteration follows the JDK's buckets for the
/// keys' insertion order and the table size, which `clear` keeps.
#[derive(Clone, Debug)]
pub struct IntHashMap<V> {
    insertion: Vec<i32>,
    values: BTreeMap<i32, V>,
    capacity: usize,
}
impl<V> Default for IntHashMap<V> {
    fn default() -> Self {
        Self {
            insertion: Vec::new(),
            values: BTreeMap::new(),
            capacity: 16,
        }
    }
}
impl<V> IntHashMap<V> {
    /// `put`: a present key keeps its place and takes the new value.
    pub fn put(&mut self, key: i32, value: V) -> Option<V> {
        if !self.values.contains_key(&key) {
            self.insertion.push(key);
        }
        self.values.insert(key, value)
    }
    pub fn get(&self, key: i32) -> Option<&V> {
        self.values.get(&key)
    }
    pub fn get_mut(&mut self, key: i32) -> Option<&mut V> {
        self.values.get_mut(&key)
    }
    pub fn contains_key(&self, key: i32) -> bool {
        self.values.contains_key(&key)
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    /// `clear`: the entries go, the table keeps the size it grew to.
    pub fn clear(&mut self) {
        self.capacity = int_key_layout(self.insertion.iter().copied(), self.capacity).1;
        self.insertion.clear();
        self.values.clear();
    }
    /// Keys in the order the JDK's HashMap iterates them.
    pub fn keys(&self) -> Vec<i32> {
        int_key_layout(self.insertion.iter().copied(), self.capacity).0
    }
    pub fn iter(&self) -> impl Iterator<Item = (i32, &V)> {
        self.keys().into_iter().map(|key| (key, &self.values[&key]))
    }
}

/// A missing key or JSON null: what `get` returns as Java null.
fn absent(value: Option<&Json>) -> bool {
    matches!(value, None | Some(Json::Null))
}
/// Java `value.toString()`: null throws.
pub(crate) fn java_text(value: Option<&Json>) -> Result<String> {
    if absent(value) {
        return Err(Error::Null);
    }
    Ok(value.map(Json::text).unwrap_or_default())
}
/// Kotlin `value.toString()`: null reads as the text "null".
pub(crate) fn kotlin_text(value: Option<&Json>) -> String {
    value.map_or_else(|| "null".into(), Json::text)
}
/// `Integer.parseInt` / Kotlin `String.toInt`.
pub(crate) fn parse_int(text: &str) -> Result<i32> {
    rs09_game_core::java_string::parse_i32(text).ok_or(Error::Number)
}
/// Java `(JSONArray) value`: null passes the cast and fails on first use.
pub(crate) fn java_array(value: Option<&Json>) -> Result<Option<&[Json]>> {
    match value {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Array(values)) => Ok(Some(values)),
        Some(_) => Err(Error::Type),
    }
}
/// Java `(JSONObject) value`.
pub(crate) fn java_object(value: Option<&Json>) -> Result<Option<&Json>> {
    match value {
        None | Some(Json::Null) => Ok(None),
        Some(object @ Json::Object(_)) => Ok(Some(object)),
        Some(_) => Err(Error::Type),
    }
}
/// Kotlin `value as T` to a non-null type: null fails as a null error.
pub(crate) fn kotlin_array(value: Option<&Json>) -> Result<&[Json]> {
    java_array(value)?.ok_or(Error::Null)
}
pub(crate) fn kotlin_object(value: Option<&Json>) -> Result<&Json> {
    java_object(value)?.ok_or(Error::Null)
}
/// Java `(boolean) value`: unboxing null fails as null, another type as a cast.
pub(crate) fn java_bool(value: Option<&Json>) -> Result<bool> {
    match value {
        None | Some(Json::Null) => Err(Error::Null),
        Some(Json::Bool(value)) => Ok(*value),
        Some(_) => Err(Error::Type),
    }
}
