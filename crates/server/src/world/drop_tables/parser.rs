//! `DropTableParser.load`: `drop_tables.json` into one table per row, handed
//! to every NPC the row names.
//!
//! Building a table draws from `RandomFunction.RANDOM` (each weighted entry
//! goes to a random position), so the stored order of every table depends on
//! the generator's state when loading starts and on every row before it.
use super::{LogLevel, NpcDropTable, WeightedItem, random::Source};
use rs09_game_core::{
    java_double,
    java_json::{self, Json},
    java_string::parse_i32,
};
use std::sync::Arc;

/// The exceptions that leave `load`. All but [`LoadError::File`] and
/// [`LoadError::Json`] can come after earlier rows were handed out; those rows
/// keep their tables.
#[derive(Debug)]
pub enum LoadError {
    /// The file cannot be read (`FileNotFoundException`).
    File(std::io::Error),
    /// JSON-simple refused the text (`ParseException`).
    Json(java_json::Error),
    /// A required value is JSON null or absent (`NullPointerException`).
    Null,
    /// A value has the wrong JSON type (`ClassCastException`).
    Type,
    /// An id or amount is not a Java `int`, or a weight not a Java `double`
    /// (`NumberFormatException`).
    Number,
    /// The definition lookup failed.
    Definition(String),
}
impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LoadError {}

/// `NPCDefinition`, as far as the parser uses it. NPC definitions do not hold
/// drop tables natively yet, so the owner of the tables implements this.
pub trait Definitions {
    /// `NPCDefinition.forId(id).dropTables`, then `table = table` on it.
    /// `Ok(false)` when the definition's drop tables are null: the id is
    /// passed over and not counted. A null definition is [`LoadError::Null`].
    fn assign(&mut self, id: i32, table: &Arc<NpcDropTable>) -> Result<bool, LoadError>;
    /// `NPCDefinition.forId(id).name`, read only to report a refused row.
    fn name(&mut self, id: i32) -> Result<Option<String>, LoadError>;
    /// `core.api.log(DropTableParser::class.java, level, message)`.
    fn log(&mut self, level: LogLevel, message: &str);
}

/// The message of the one failure `load` catches.
pub const INVALID_AMOUNTS: &str =
    "Table is invalid! Specified minimum amount is > specified maximum amount.";

enum RowError {
    /// `ConfigParseException`: caught, the row is dropped.
    InvalidAmounts,
    Fatal(LoadError),
}
impl From<LoadError> for RowError {
    fn from(error: LoadError) -> Self {
        Self::Fatal(error)
    }
}

#[derive(Clone, Copy)]
enum Section {
    Main,
    Charm,
    Tertiary,
    Default,
}

/// `load()` for a configuration directory prefix: the original opens
/// `CONFIG_PATH + "drop_tables.json"`, and a null prefix reads as `null`.
/// The file is read as UTF-8; the pinned file is ASCII.
pub fn load_config(
    prefix: Option<&str>,
    random: &mut dyn Source,
    definitions: &mut dyn Definitions,
) -> Result<i32, LoadError> {
    let path = format!("{}drop_tables.json", prefix.unwrap_or("null"));
    let text = std::fs::read_to_string(path).map_err(LoadError::File)?;
    load(&text, random, definitions)
}

/// `load()` over the file's text. Returns the number of NPC ids that received
/// a table, which the original logs.
pub fn load(
    text: &str,
    random: &mut dyn Source,
    definitions: &mut dyn Definitions,
) -> Result<i32, LoadError> {
    let data = java_json::parse(text).map_err(LoadError::Json)?;
    let rows = array(Some(&data))?;
    let mut count = 0i32;
    for row in rows {
        let tab = object(row)?;
        // A missing "ids" is the text "null", which fails as a number later.
        let ids = tab.get("ids").unwrap_or(&Json::Null).text();
        let ids: Vec<&str> = ids.split(',').collect();
        match parse_row(tab, random) {
            Ok(table) => {
                let table = Arc::new(table);
                for n in &ids {
                    let id = parse_i32(n).ok_or(LoadError::Number)?;
                    if definitions.assign(id, &table)? {
                        count = count.wrapping_add(1);
                    }
                }
            }
            Err(RowError::InvalidAmounts) => {
                let id = parse_i32(ids[0]).ok_or(LoadError::Number)?;
                let name = definitions.name(id)?;
                // The original appends the exception's stack trace; only its
                // first line is reproduced.
                definitions.log(
                    LogLevel::Err,
                    &format!(
                        "Error parsing drop tables for NPC {}: core.api.utils.ConfigParseException: {INVALID_AMOUNTS}",
                        name.as_deref().unwrap_or("null")
                    ),
                );
            }
            Err(RowError::Fatal(error)) => return Err(error),
        }
    }
    definitions.log(LogLevel::Fine, &format!("Parsed {count} drop tables."));
    Ok(count)
}

fn parse_row(tab: &Json, random: &mut dyn Source) -> Result<NpcDropTable, RowError> {
    let mut table = NpcDropTable::new();
    parse_table(array(tab.get("main"))?, &mut table, random, Section::Main)?;
    parse_table(array(tab.get("charm"))?, &mut table, random, Section::Charm)?;
    // `as? JSONArray`: anything that is not an array is passed over.
    if let Some(Json::Array(tertiary)) = tab.get("tertiary") {
        parse_table(tertiary, &mut table, random, Section::Tertiary)?;
    }
    parse_table(
        array(tab.get("default"))?,
        &mut table,
        random,
        Section::Default,
    )?;
    Ok(table)
}

fn parse_table(
    data: &[Json],
    dest: &mut NpcDropTable,
    random: &mut dyn Source,
    section: Section,
) -> Result<(), RowError> {
    for it in data {
        let item = object(it)?;
        let int = |key: &str| {
            parse_i32(&item.get(key).unwrap_or(&Json::Null).text()).ok_or(LoadError::Number)
        };
        let id = int("id")?;
        let min_amount = int("minAmount")?;
        let max_amount = int("maxAmount")?;
        if min_amount > max_amount {
            return Err(RowError::InvalidAmounts);
        }
        let weight = java_double::parse(&item.get("weight").unwrap_or(&Json::Null).text())
            .ok_or(LoadError::Number)?;
        let new_item = WeightedItem::new(
            id,
            min_amount,
            max_amount,
            weight,
            matches!(section, Section::Default),
        );
        match section {
            Section::Charm => dest.add_to_charms(random, new_item),
            Section::Tertiary => dest.add_to_tertiary(random, new_item),
            Section::Main | Section::Default => dest.add(random, new_item),
        };
    }
    Ok(())
}

/// `value as JSONArray`.
fn array(value: Option<&Json>) -> Result<&[Json], LoadError> {
    match value {
        Some(Json::Array(values)) => Ok(values),
        None | Some(Json::Null) => Err(LoadError::Null),
        Some(_) => Err(LoadError::Type),
    }
}

/// `value as JSONObject`.
fn object(value: &Json) -> Result<&Json, LoadError> {
    match value {
        Json::Object(_) => Ok(value),
        Json::Null => Err(LoadError::Null),
        _ => Err(LoadError::Type),
    }
}
