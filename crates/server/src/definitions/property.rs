//! Typed content configuration values. Scalar serialization retains the existing
//! definition observations; runtime animation/audio objects have explicit types.
use super::item::Shared;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Debug, Default)]
pub struct Requirements {
    values: BTreeMap<i32, i32>,
    insertion: Vec<i32>,
}
impl Requirements {
    pub fn insert(&mut self, key: i32, value: i32) {
        if !self.values.contains_key(&key) {
            self.insertion.push(key);
        }
        self.values.insert(key, value);
    }
    pub fn get(&self, key: i32) -> Option<i32> {
        self.values.get(&key).copied()
    }
    pub fn values(&self) -> &BTreeMap<i32, i32> {
        &self.values
    }
    pub fn order(&self) -> Vec<i32> {
        rs09_game_core::java_map::int_key_order(self.insertion.iter().copied())
    }
}
impl Serialize for Requirements {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.values.serialize(serializer)
    }
}

pub use crate::world::animation::{AnimationFields as Animation, Priority};
#[derive(Debug, Serialize)]
pub struct Audio {
    pub id: i32,
    pub delay: i32,
    pub loops: i32,
    pub radius: i32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CombatStyle {
    Melee,
    Range,
    Magic,
}
impl CombatStyle {
    pub const ALL: [Self; 3] = [Self::Melee, Self::Range, Self::Magic];
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClueLevel {
    Easy,
    Medium,
    Hard,
    Unknown,
}
impl ClueLevel {
    pub const ALL: [Self; 4] = [Self::Easy, Self::Medium, Self::Hard, Self::Unknown];
}
impl Audio {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            delay: 0,
            loops: 1,
            radius: 8,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Property {
    Null,
    Int(i32),
    Long(i64),
    Double(f64),
    Bool(bool),
    Text(String),
    IntArray(Shared<Vec<i32>>),
    Requirements(Shared<Requirements>),
    Animation(Shared<Animation>),
    Animations(Shared<Vec<Option<Shared<Animation>>>>),
    Audios(Shared<Vec<Shared<Audio>>>),
    CombatStyle(CombatStyle),
    ClueLevel(ClueLevel),
    /// Installed by native content registration, not reconstructed from JSON.
    OptionHandler(super::option_handler::Handle),
    Plugin(super::plugin::Handle),
    /// Untyped data accepted by existing observation/authoring APIs, not a Java
    /// plugin-object or arbitrary Java collection compatibility implementation.
    Json(Value),
}
impl Property {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(i64::from(*value)),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }
    pub fn observation(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Int(v) => json!(v),
            Self::Long(v) => json!(v),
            Self::Double(v) if v.is_finite() => json!(v),
            Self::Double(v) => json!({"$double":format!("{:016x}",v.to_bits())}),
            Self::Bool(v) => json!(v),
            Self::Text(v) => json!(v),
            Self::IntArray(v) => json!(v),
            Self::Requirements(v) => json!(v),
            Self::Animation(v) => json!({"$animation":v}),
            Self::Animations(v) => json!({"$animations":v}),
            Self::Audios(v) => json!({"$audios":v}),
            Self::CombatStyle(v) => json!({"$combatStyle":v}),
            Self::ClueLevel(v) => json!({"$clueLevel":v}),
            Self::OptionHandler(v) => json!({"$optionHandler":v.read().label()}),
            Self::Plugin(v) => json!({"$plugin":v.label()}),
            Self::Json(v) => v.clone(),
        }
    }
}
impl From<String> for Property {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl From<bool> for Property {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
impl Serialize for Property {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.observation().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Property {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        Ok(match value {
            Value::Null => Self::Null,
            Value::Bool(v) => Self::Bool(v),
            Value::String(v) => Self::Text(v),
            Value::Number(v) if v.is_f64() => Self::Double(v.as_f64().unwrap()),
            Value::Number(v) => match v.as_i64() {
                Some(v) => match i32::try_from(v) {
                    Ok(v) => Self::Int(v),
                    Err(_) => Self::Long(v),
                },
                None => Self::Json(Value::Number(v)),
            },
            value => Self::Json(value),
        })
    }
}
