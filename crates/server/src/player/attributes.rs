//! Entity attributes and the attribute sections of the original character save.
//! World event dispatch belongs to Entity; this store performs no dispatch itself.
mod legacy;
mod text;
use crate::auth::Clock;
pub use legacy::LegacyLoad;
use rs09_game_core::java_string::{parse_i32, parse_i64};
use serde_json::{Map, Value as Json};
use std::{
    any::Any,
    borrow::Cow,
    collections::HashMap,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
    Base64,
    Xml,
    Io,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub trait AttributeObject: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn java_string(&self) -> String;
    /// Kotlin function-interface category for actual native function objects.
    fn function_arity(&self) -> Option<u8> {
        None
    }
    /// Physical strong fields, once per object, without gameplay callbacks.
    /// Scalar Display objects have no canonical captures. Native callback
    /// payloads must explicitly report every supported strong field.
    fn captures(&self) -> crate::world::component_objects::Captures {
        Default::default()
    }
}
impl<T: Any + Send + Sync + fmt::Display> AttributeObject for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn java_string(&self) -> String {
        self.to_string()
    }
}
/// Shared, mutable coordinate identity for a Location stored as an attribute.
#[derive(Clone, Debug)]
pub struct AttributeLocation(Arc<AtomicI32>);
impl AttributeLocation {
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(Arc::new(AtomicI32::new(
            ((z & 3) << 30) | ((x & 32767) << 15) | (y & 32767),
        )))
    }
    pub fn coordinates(&self) -> [i32; 3] {
        let hash = self.0.load(Ordering::Relaxed);
        let signed = |n: i32| if n >= 16384 { n - 32768 } else { n };
        [
            signed((hash >> 15) & 32767),
            signed(hash & 32767),
            ((hash as u32) >> 30) as i32,
        ]
    }
    pub fn set(&self, x: i32, y: i32, z: i32) {
        self.0.store(
            ((z & 3) << 30) | ((x & 32767) << 15) | (y & 32767),
            Ordering::Relaxed,
        );
    }
    pub fn parse(value: &str) -> Result<Self> {
        let value = value
            .strip_prefix('[')
            .and_then(|part| part.strip_suffix(']'))
            .unwrap_or(value);
        let coordinates = value
            .split(',')
            .map(|n| parse_i32(text::kotlin_trim(n)).ok_or(Error::Number))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self::new(
            *coordinates.first().ok_or(Error::Index)?,
            *coordinates.get(1).ok_or(Error::Index)?,
            *coordinates.get(2).ok_or(Error::Index)?,
        ))
    }
}
impl fmt::Display for AttributeLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [x, y, z] = self.coordinates();
        write!(f, "[{x}, {y}, {z}]")
    }
}
#[derive(Clone)]
pub enum AttributeValue {
    Int(i32),
    Long(i64),
    Short(i16),
    Byte(i8),
    Bool(bool),
    Text(String),
    Location(AttributeLocation),
    Object(Arc<dyn AttributeObject>),
}
impl AttributeValue {
    pub fn java_string(&self) -> String {
        match self {
            Self::Int(v) => v.to_string(),
            Self::Long(v) => v.to_string(),
            Self::Short(v) => v.to_string(),
            Self::Byte(v) => v.to_string(),
            Self::Bool(v) => v.to_string(),
            Self::Text(v) => v.clone(),
            Self::Location(v) => v.to_string(),
            Self::Object(v) => v.java_string(),
        }
    }
    pub fn saved_type(&self) -> &'static str {
        match self {
            Self::Int(_) => "int",
            Self::Long(_) => "long",
            Self::Short(_) => "short",
            Self::Byte(_) => "byte",
            Self::Bool(_) => "bool",
            Self::Text(_) => "str",
            Self::Location(_) => "location",
            Self::Object(_) => "null",
        }
    }
}
#[derive(Default)]
pub struct GameAttributes {
    pub values: HashMap<String, Option<AttributeValue>>,
    pub null_value: Option<Option<AttributeValue>>,
    pub saved: Vec<Option<String>>,
    pub expirations: HashMap<String, Option<i64>>,
    pub null_expiration: Option<Option<i64>>,
}
impl GameAttributes {
    pub fn set(&mut self, key: &str, value: Option<AttributeValue>) {
        let saved = key.starts_with("/save:");
        let key = key.strip_prefix("/save:").unwrap_or(key).to_owned();
        if saved
            && !self
                .saved
                .iter()
                .any(|saved| saved.as_deref() == Some(&key))
        {
            self.saved.push(Some(key.clone()));
        }
        self.values.insert(key, value);
    }
    pub fn get_raw(&self, key: Option<&str>) -> Option<&AttributeValue> {
        match key {
            Some(key) => self.values.get(key),
            None => self.null_value.as_ref(),
        }
        .and_then(Option::as_ref)
    }
    pub fn get(&self, key: &str) -> Option<&AttributeValue> {
        let key = if key.contains("/save:") {
            Cow::Owned(key.replace("/save:", ""))
        } else {
            Cow::Borrowed(key)
        };
        self.get_raw(Some(&key))
    }
    pub fn get_or<'a>(&'a self, key: &str, fallback: &'a AttributeValue) -> &'a AttributeValue {
        self.get(key).unwrap_or(fallback)
    }
    pub fn remove(&mut self, key: &str) {
        if let Some(index) = self
            .saved
            .iter()
            .position(|saved| saved.as_deref() == Some(key))
        {
            self.saved.remove(index);
        }
        self.values.remove(key);
    }
    /// The original deprecated dump method has an empty body.
    pub fn dump(&self, _file: &str) {}
    pub fn parse_json(
        &mut self,
        root: &Json,
        clock: &Clock,
        legacy_prefix: &str,
        player_name: &str,
    ) -> Result<LegacyLoad> {
        self.parse_json_with_log(root, clock, legacy_prefix, player_name, &mut |_| {})
    }
    pub fn parse_json_with_log(
        &mut self,
        root: &Json,
        clock: &Clock,
        legacy_prefix: &str,
        player_name: &str,
        log: &mut dyn FnMut(&str),
    ) -> Result<LegacyLoad> {
        let root = root.as_object().ok_or(if root.is_null() {
            Error::Null
        } else {
            Error::Type
        })?;
        let Some(attributes) = root.get("attributes") else {
            return Ok(self.parse_legacy_file(legacy_prefix, &format!("{player_name}.xml")));
        };
        let list = attributes.as_array().ok_or(if attributes.is_null() {
            Error::Null
        } else {
            Error::Type
        })?;
        for attribute in list {
            let attribute = attribute.as_object().ok_or(if attribute.is_null() {
                Error::Null
            } else {
                Error::Type
            })?;
            let key = text::nullable_string(attribute.get("key"));
            let kind = text::nullable_string(attribute.get("type"));
            let expirable = match attribute.get("expirable") {
                None => false,
                Some(Json::Bool(value)) => *value,
                Some(Json::Null) => return Err(Error::Null),
                _ => return Err(Error::Type),
            };
            if expirable {
                let expiry = parse_i64(&text::nullable_string(attribute.get("expiration-time")))
                    .ok_or(Error::Number)?;
                if expiry < clock() {
                    continue;
                }
                self.expirations.insert(key.clone(), Some(expiry));
            }
            self.saved.push(Some(key.clone()));
            let value = attribute.get("value");
            let string = || text::nullable_string(value);
            let parsed = match kind.as_str() {
                "int" => Some(AttributeValue::Int(
                    parse_i32(&string()).ok_or(Error::Number)?,
                )),
                "long" => Some(AttributeValue::Long(
                    parse_i64(&string()).ok_or(Error::Number)?,
                )),
                "short" => Some(AttributeValue::Short(
                    i16::try_from(parse_i32(&string()).ok_or(Error::Number)?)
                        .map_err(|_| Error::Number)?,
                )),
                "str" => Some(AttributeValue::Text(string())),
                "bool" => Some(AttributeValue::Bool(match value {
                    Some(Json::Bool(value)) => *value,
                    None | Some(Json::Null) => return Err(Error::Null),
                    _ => return Err(Error::Type),
                })),
                "byte" => Some(AttributeValue::Byte(text::first_base64_byte(&string())?)),
                "location" => Some(AttributeValue::Location(AttributeLocation::parse(
                    &string(),
                )?)),
                _ => {
                    log(&format!(
                        "Invalid data type for key: {key} in PlayerSaveParser.kt Line 115"
                    ));
                    None
                }
            };
            self.values.insert(key, parsed);
        }
        Ok(LegacyLoad::NotUsed)
    }
    pub fn save_json(&self, root: &mut Map<String, Json>) {
        self.save_json_with_log(root, &mut |_| {});
    }
    pub fn save_json_with_log(&self, root: &mut Map<String, Json>, log: &mut dyn FnMut(&str)) {
        if self.saved.is_empty() {
            return;
        }
        let mut attributes = Vec::new();
        for key in &self.saved {
            let Some(value) = self.get_raw(key.as_deref()) else {
                continue;
            };
            if matches!(value, AttributeValue::Object(_)) {
                log(&format!(
                    "Invalid attribute type for key: {} in PlayerSaver.kt Line 115",
                    key.as_deref().unwrap_or("null")
                ));
            }
            let mut attribute = Map::new();
            attribute.insert("type".into(), Json::String(value.saved_type().into()));
            attribute.insert(
                "key".into(),
                key.as_ref()
                    .map_or(Json::Null, |key| Json::String(key.clone())),
            );
            let data = match value {
                AttributeValue::Bool(value) => Json::Bool(*value),
                AttributeValue::Byte(value) => Json::String(text::base64_byte(*value)),
                _ => Json::String(value.java_string()),
            };
            attribute.insert("value".into(), data);
            if let Some(expiry) = match key {
                Some(key) => self.expirations.get(key),
                None => self.null_expiration.as_ref(),
            } {
                attribute.insert("expirable".into(), Json::Bool(true));
                attribute.insert(
                    "expiration-time".into(),
                    Json::String(expiry.map_or_else(|| "null".into(), |value| value.to_string())),
                );
            }
            attributes.push(Json::Object(attribute));
        }
        root.insert("attributes".into(), Json::Array(attributes));
    }
}
