//! The start-up configuration loaders that had no native port: `XteaParser`,
//! `ObjectConfigParser`, `ObjectOverrideParser`, `ShopParser`,
//! `MusicConfigLoader`, `RangedConfigLoader` and `ClueRewardParser`.
//!
//! Each follows its Kotlin source statement by statement: the same reads in
//! the same order, the same evaluation order inside one row, the same
//! exception (as a [`LoadError`]) at the same point, and the rows before a
//! failure kept. The JSON grammar and value stringification are JSON-simple's
//! (`rs09_game_core::java_json`); numbers are parsed with `Integer.parseInt` /
//! `Double.parseDouble` rules. Verified against the unchanged original loaders
//! by `fixtures/startup-loaders` (see docs/startup.md).
pub mod clue_rewards;
pub mod music;
pub mod object_config;
pub mod object_override;
pub mod ranged;
pub mod shops;
pub mod xtea;

use crate::world::drop_tables::LogLevel;
use rs09_game_core::{
    java_double,
    java_json::{self, Json},
    java_string::parse_i32,
};

/// The exceptions that leave a loader. The original classes do not catch any
/// of them, so they leave `GameWorld.prompt` and stop the start-up.
#[derive(Debug)]
pub enum LoadError {
    /// `java.io.FileNotFoundException` (the file cannot be opened).
    File(std::io::Error),
    /// `org.json.simple.parser.ParseException`.
    Json(java_json::Error),
    /// `java.lang.NullPointerException` (a cast of a JSON null or absent value).
    Null,
    /// `java.lang.ClassCastException`.
    Type,
    /// `java.lang.NumberFormatException`.
    Number,
    /// `java.lang.IndexOutOfBoundsException` (a Kotlin `List` index).
    ListIndex,
    /// `java.lang.ArrayIndexOutOfBoundsException` (a Kotlin `Array` index).
    ArrayIndex,
    /// A host service failed.
    Binding(String),
}
impl LoadError {
    /// The class of the Java exception this stands for.
    pub fn java_class(&self) -> &'static str {
        match self {
            Self::File(_) => "java.io.FileNotFoundException",
            Self::Json(_) => "org.json.simple.parser.ParseException",
            Self::Null => "java.lang.NullPointerException",
            Self::Type => "java.lang.ClassCastException",
            Self::Number => "java.lang.NumberFormatException",
            Self::ListIndex => "java.lang.IndexOutOfBoundsException",
            Self::ArrayIndex => "java.lang.ArrayIndexOutOfBoundsException",
            Self::Binding(_) => "binding",
        }
    }
}
impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LoadError {}
pub type Result<T> = std::result::Result<T, LoadError>;

/// `core.api.log(origin, level, message)`; `origin` is the simple class name.
pub trait Log {
    fn log(&mut self, origin: &'static str, level: LogLevel, message: &str);
}

/// `FileReader(CONFIG_PATH + name)` followed by reading it all. A null
/// `CONFIG_PATH` reads as the text `null`. The file is decoded as UTF-8 (the
/// JVM's file encoding in the oracle); bytes that are not UTF-8 are refused
/// rather than replaced, a documented difference.
pub fn read_config(prefix: Option<&str>, name: &str) -> Result<String> {
    let path = format!("{}{name}", prefix.unwrap_or("null"));
    let bytes = std::fs::read(path).map_err(LoadError::File)?;
    String::from_utf8(bytes)
        .map_err(|_| LoadError::Binding("configuration file is not UTF-8".into()))
}

/// `JSONParser().parse(reader)`.
pub fn parse(text: &str) -> Result<Json> {
    java_json::parse(text).map_err(LoadError::Json)
}

/// `value as JSONArray`.
pub fn array(value: Option<&Json>) -> Result<&[Json]> {
    match value {
        Some(Json::Array(values)) => Ok(values),
        None | Some(Json::Null) => Err(LoadError::Null),
        Some(_) => Err(LoadError::Type),
    }
}

/// `value as JSONObject`.
pub fn object(value: Option<&Json>) -> Result<&Json> {
    match value {
        Some(value @ Json::Object(_)) => Ok(value),
        None | Some(Json::Null) => Err(LoadError::Null),
        Some(_) => Err(LoadError::Type),
    }
}

/// `e[key].toString()` in Kotlin: an absent key or JSON null is `"null"`.
pub fn text(row: &Json, key: &str) -> String {
    row.get(key).unwrap_or(&Json::Null).text()
}

/// `String.toInt()` / `Integer.parseInt`.
pub fn int(text: &str) -> Result<i32> {
    parse_i32(text).ok_or(LoadError::Number)
}

/// `String.toDouble()` / `Double.parseDouble`.
pub fn double(text: &str) -> Result<f64> {
    java_double::parse(text).ok_or(LoadError::Number)
}

/// Kotlin `Char.isWhitespace()`: `Character.isWhitespace || Character.isSpaceChar`.
pub fn kotlin_whitespace(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Kotlin `CharSequence.trim()`.
pub fn kotlin_trim(text: &str) -> &str {
    text.trim_matches(kotlin_whitespace)
}

/// Kotlin `CharSequence.isNotBlank()`.
pub fn kotlin_not_blank(text: &str) -> bool {
    !text.chars().all(kotlin_whitespace)
}

/// Kotlin `String.removeSurrounding(prefix, suffix)`.
pub fn remove_surrounding<'a>(text: &'a str, prefix: &str, suffix: &str) -> &'a str {
    if text.len() >= prefix.len() + suffix.len()
        && text.starts_with(prefix)
        && text.ends_with(suffix)
    {
        &text[prefix.len()..text.len() - suffix.len()]
    } else {
        text
    }
}

/// `list[index]` on a Kotlin `List`.
pub fn list_get<T>(list: &[T], index: usize) -> Result<&T> {
    list.get(index).ok_or(LoadError::ListIndex)
}

/// `array[index]` on a Kotlin `Array`.
pub fn array_get<T>(array: &[T], index: usize) -> Result<&T> {
    array.get(index).ok_or(LoadError::ArrayIndex)
}
