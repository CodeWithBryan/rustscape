//! `GraveController.serializeToServerStore` and `deserializeFromServerStore`:
//! the `active-graves` archive of the server store, as JSON-simple writes and
//! reads it.
use super::{
    Controller, Error, GraveType, Host, Result,
    grave::{produce_grave, setup_from_json},
    index, null,
};
use crate::{
    definitions::item::{Registry, Shared},
    inventory::item::Item,
    world::doors::Location,
};
use rs09_game_core::java_map::string_key_order;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// The archive's name.
pub const ARCHIVE: &str = "active-graves";

/// A JSON-simple value. Objects keep their keys in insertion order; their
/// iteration order is the `HashMap`'s, see [`Json::entries`].
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// JSON-simple reads an integer literal as a `Long`.
    Long(i64),
    /// Any other number is a `Double`.
    Double(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}
impl Json {
    /// `JSONObject.get(key)`.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    /// The entries in `HashMap` iteration order (a fresh table: a table that
    /// kept a larger size across `clear()` is not modelled).
    pub fn entries(&self) -> Vec<(String, Json)> {
        let Self::Object(entries) = self else {
            return Vec::new();
        };
        string_key_order(entries.iter().map(|(key, _)| key.clone()))
            .into_iter()
            .filter_map(|key| self.get(&key).map(|value| (key, value.clone())))
            .collect()
    }
    /// `value.toString()`. Kotlin compiles it as `String.valueOf`, so a
    /// missing value reads as `"null"`.
    fn text(&self) -> Result<String> {
        match self {
            Self::Null => Ok("null".into()),
            Self::Bool(value) => Ok(value.to_string()),
            Self::Long(value) => Ok(value.to_string()),
            Self::Str(value) => Ok(value.clone()),
            Self::Double(_) | Self::Array(_) | Self::Object(_) => Ok(to_json_simple(self)),
        }
    }
    fn insert(entries: &mut Vec<(String, Json)>, key: String, value: Json) {
        match entries.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => entries.push((key, value)),
        }
    }
}

struct JsonVisitor;
impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JSON")
    }
    fn visit_unit<E>(self) -> std::result::Result<Json, E> {
        Ok(Json::Null)
    }
    fn visit_bool<E>(self, value: bool) -> std::result::Result<Json, E> {
        Ok(Json::Bool(value))
    }
    fn visit_i64<E>(self, value: i64) -> std::result::Result<Json, E> {
        Ok(Json::Long(value))
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> std::result::Result<Json, E> {
        i64::try_from(value)
            .map(Json::Long)
            .map_err(|_| E::custom("integer beyond Long"))
    }
    fn visit_f64<E>(self, value: f64) -> std::result::Result<Json, E> {
        Ok(Json::Double(value))
    }
    fn visit_str<E>(self, value: &str) -> std::result::Result<Json, E> {
        Ok(Json::Str(value.to_owned()))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Json, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element::<Json>()? {
            values.push(value);
        }
        Ok(Json::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<Json, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            Json::insert(&mut entries, key, value);
        }
        Ok(Json::Object(entries))
    }
}
impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Json, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

/// Reads archive text as JSON-simple's parser does, keeping key order.
pub fn parse_text(text: &str) -> std::result::Result<Json, serde_json::Error> {
    serde_json::from_str(text)
}

/// `JSONValue.escape`.
fn escape(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '/' => out.push_str("\\/"),
            '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{2000}'..='\u{20ff}' => {
                out.push_str(&format!("\\u{:04X}", ch as u32));
            }
            _ => out.push(ch),
        }
    }
}

/// `JSONValue.toJSONString(value)`. A `Double` is written with Rust's
/// shortest form, which is not Java's `Double.toString` for every value; no
/// archive this module writes holds one.
pub fn to_json_simple(value: &Json) -> String {
    let mut out = String::new();
    write(value, &mut out);
    out
}
fn write(value: &Json, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(value) => out.push_str(&value.to_string()),
        Json::Long(value) => out.push_str(&value.to_string()),
        Json::Double(value) => out.push_str(&format!("{value:?}")),
        Json::Str(text) => {
            out.push('"');
            escape(text, out);
            out.push('"');
        }
        Json::Array(values) => {
            out.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write(value, out);
            }
            out.push(']');
        }
        Json::Object(_) => {
            out.push('{');
            for (index, (key, value)) in value.entries().iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push('"');
                escape(key, out);
                out.push_str("\":");
                write(value, out);
            }
            out.push('}');
        }
    }
}

/// `String.toInt()` (`Integer.parseInt`): an optional sign and decimal
/// digits. Only ASCII digits are accepted; the JDK also accepts other
/// Unicode decimal digits.
pub fn parse_int(text: &str) -> Result<i32> {
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Number);
    }
    let mut value: i64 = 0;
    for digit in digits.bytes() {
        value = value * 10 + i64::from(digit - b'0');
        if value > i64::from(i32::MAX) + 1 {
            return Err(Error::Number);
        }
    }
    let value = if negative { -value } else { value };
    i32::try_from(value).map_err(|_| Error::Number)
}

/// Kotlin `String.trim()` for the characters this archive can hold: Java
/// whitespace and space characters in ASCII, Rust whitespace beyond it.
fn kotlin_trim(text: &str) -> &str {
    text.trim_matches(|c: char| {
        if c.is_ascii() {
            matches!(
                c,
                '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1c}'..='\u{1f}' | ' '
            )
        } else {
            c.is_whitespace()
        }
    })
}

/// `Location.fromString(text)`: `[x, y, z]`, every part read before the
/// first three are used.
pub fn location_from_string(text: &str) -> Result<Location> {
    let inner = if text.len() >= 2 && text.starts_with('[') && text.ends_with(']') {
        &text[1..text.len() - 1]
    } else {
        text
    };
    let parts = inner
        .split(',')
        .map(|part| parse_int(kotlin_trim(part)))
        .collect::<Result<Vec<_>>>()?;
    if parts.len() < 3 {
        return Err(index());
    }
    Ok(Location::new(parts[0], parts[1], parts[2]))
}
/// `Location.toString()`.
pub fn location_text(location: &Location) -> String {
    let [x, y, z] = location.coordinates();
    format!("[{x}, {y}, {z}]")
}

/// `GraveController.serializeToServerStore()`: the archive is fetched and
/// cleared, then every active grave is written under its owner's uid in the
/// controller's map order. Returns the archive as JSON-simple writes it.
pub fn serialize<H: Host>(host: &mut H, controller: &Controller) -> Result<Json> {
    host.store_archive(ARCHIVE)?;
    let mut archive = Vec::new();
    for (uid, grave) in controller.entries() {
        let (remaining, since, npc, kind, username, items) = {
            let grave = grave.read();
            (
                grave.ticks_remaining,
                grave.ticks_since_death,
                grave.npc.clone(),
                grave.kind()?,
                grave.owner_username.clone(),
                grave.items(),
            )
        };
        let mut g = Vec::new();
        Json::insert(
            &mut g,
            "ticksRemaining".into(),
            Json::Long(i64::from(remaining)),
        );
        Json::insert(
            &mut g,
            "ticksSinceDeath".into(),
            Json::Long(i64::from(since)),
        );
        let location = host.npc_location(&npc)?.ok_or_else(null)?;
        Json::insert(
            &mut g,
            "location".into(),
            Json::Str(location_text(&location)),
        );
        Json::insert(&mut g, "type".into(), Json::Long(i64::from(kind.ordinal())));
        Json::insert(&mut g, "username".into(), Json::Str(username));
        let mut list = Vec::new();
        for item in items {
            let item = item.read().item.clone();
            let item = item.read();
            let mut i = Vec::new();
            Json::insert(&mut i, "id".into(), Json::Long(i64::from(item.id())));
            Json::insert(
                &mut i,
                "amount".into(),
                Json::Long(i64::from(item.amount())),
            );
            Json::insert(
                &mut i,
                "charge".into(),
                Json::Long(i64::from(item.charge())),
            );
            list.push(Json::Object(i));
        }
        Json::insert(&mut g, "items".into(), Json::Array(list));
        Json::insert(&mut archive, uid.to_string(), Json::Object(g));
    }
    Ok(Json::Object(archive))
}

/// `GraveController.deserializeFromServerStore()` over the archive the store
/// holds: each entry in map order becomes a grave of its type with its items
/// on the ground, registered under its uid. A failure keeps the graves made
/// before it.
pub fn deserialize<H: Host>(
    host: &mut H,
    controller: &mut Controller,
    archive: &Json,
    definitions: &Registry,
) -> Result<()> {
    host.store_archive(ARCHIVE)?;
    for (key, value) in archive.entries() {
        let g = match &value {
            Json::Object(_) => value.clone(),
            Json::Null => return Err(null()),
            _ => return Err(Error::Death(crate::world::npc_death::Error::Type)),
        };
        let uid = parse_int(&key)?;
        let field = |name: &str| g.get(name).cloned().unwrap_or(Json::Null);
        let kind = parse_int(&field("type").text()?)?;
        let ticks = parse_int(&field("ticksRemaining").text()?)?;
        let since = match g.get("ticksSinceDeath") {
            None | Some(Json::Null) => 0,
            Some(value) => parse_int(&value.text()?)?,
        };
        let location = location_from_string(&field("location").text()?)?;
        let username = field("username").text()?;
        let raw = match field("items") {
            Json::Array(values) => values,
            Json::Null => return Err(null()),
            _ => return Err(Error::Death(crate::world::npc_death::Error::Type)),
        };
        let mut items = Vec::new();
        for raw in raw {
            let item = match &raw {
                Json::Object(_) => raw.clone(),
                Json::Null => return Err(null()),
                _ => return Err(Error::Death(crate::world::npc_death::Error::Type)),
            };
            let get = |name: &str| item.get(name).cloned().unwrap_or(Json::Null);
            let id = parse_int(&get("id").text()?)?;
            let amount = parse_int(&get("amount").text()?)?;
            let charge = parse_int(&get("charge").text()?)?;
            items.push(Shared::new(Item::new(id, amount, charge, definitions)));
        }
        let kind = GraveType::from_ordinal(kind).ok_or_else(index)?;
        let grave = produce_grave(host, kind)?;
        setup_from_json(host, &grave, uid, ticks, location, &items, username)?;
        grave.write().ticks_since_death = since;
        controller.put(uid, grave);
    }
    Ok(())
}
