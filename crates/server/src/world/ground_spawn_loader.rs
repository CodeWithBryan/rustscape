//! GroundSpawnLoader.load parsing, per-entry catch and initialization order.
//! Concrete definition/plugin, chunk, player and logging services belong to the
//! world; this loader constructs the actual native subtype and manager entries.
use super::{
    doors::{ConfigReader, Location},
    ground_item,
    ground_manager::{self as manager, Manager},
    ground_spawn::{self as spawn, Registry},
};
use crate::definitions::item::Shared;
use rs09_game_core::{
    java_json::{self, Json},
    java_string::parse_i32,
};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
};

#[derive(Debug)]
pub enum Error {
    Null,
    Type,
    Number(String),
    Index,
    Json(java_json::Error),
    Io(std::io::Error),
    Ground(ground_item::Error),
    Manager(manager::Error),
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub trait Context: manager::Context {
    fn entry_error(&mut self, entry: &Json, error: &Error) -> Result<()>;
    fn initialized(&mut self, count: i32) -> Result<()>;
}
pub fn apply(
    ctx: &mut impl Context,
    registry: &Registry,
    manager: &Manager,
    data: &Json,
) -> Result<i32> {
    let Json::Array(rows) = data else {
        return Err(if matches!(data, Json::Null) {
            Error::Null
        } else {
            Error::Type
        });
    };
    let mut count = 0i32;
    for row in rows {
        let result = (|| {
            if !matches!(row, Json::Object(_)) {
                return Err(if matches!(row, Json::Null) {
                    Error::Null
                } else {
                    Error::Type
                });
            }
            let locations = row.get("loc_data").unwrap_or(&Json::Null).text();
            let id = row.get("item_id").unwrap_or(&Json::Null).text();
            let id = parse_i32(&id).ok_or_else(|| Error::Number(id.clone()))?;
            for part in locations.split('-') {
                if part.is_empty() {
                    continue;
                }
                let part = part.replace(['{', '}'], "");
                let tokens: Vec<_> = part.split(',').collect();
                let int = |index: usize| {
                    let token = tokens.get(index).ok_or(Error::Index)?;
                    parse_i32(token).ok_or_else(|| Error::Number((*token).into()))
                };
                // Argument order is observable: rate, amount and Item creation
                // precede parsing x/y/plane. No token whitespace is trimmed.
                let rate = int(4)?;
                let amount = int(0)?;
                let item = ground_item::Context::copy_item(ctx, id, amount, 1000)
                    .map_err(Error::Ground)?;
                let location = Location::new(int(1)?, int(2)?, int(3)?);
                let handle = registry
                    .construct(ctx, rate, &item, Some(location))
                    .map_err(Error::Ground)?;
                spawn::init(ctx, manager, &handle).map_err(Error::Manager)?;
                count = count.wrapping_add(1);
            }
            Ok(())
        })();
        if let Err(error) = result {
            ctx.entry_error(row, &error)?;
        }
    }
    ctx.initialized(count)?;
    Ok(count)
}
/// Like the reference, a new reader is published only after a successful open,
/// remains available after parse errors and is not explicitly closed by load.
#[derive(Clone)]
pub struct Loader {
    reader: Shared<Option<Shared<ConfigReader>>>,
}
impl Default for Loader {
    fn default() -> Self {
        Self {
            reader: Shared::new(None),
        }
    }
}
impl Loader {
    pub fn reader(&self) -> Option<Shared<ConfigReader>> {
        self.reader.read().clone()
    }
    pub fn load_config(
        &self,
        prefix: Option<&str>,
        ctx: &mut impl Context,
        registry: &Registry,
        manager: &Manager,
    ) -> Result<i32> {
        let name = format!("{}ground_spawns.json", prefix.unwrap_or("null"));
        self.load_file(Path::new(&name), ctx, registry, manager)
    }
    pub fn load_file(
        &self,
        path: &Path,
        ctx: &mut impl Context,
        registry: &Registry,
        manager: &Manager,
    ) -> Result<i32> {
        let file = File::open(path).map_err(Error::Io)?;
        if file.metadata().map_err(Error::Io)?.is_dir() {
            return Err(Error::Io(std::io::ErrorKind::IsADirectory.into()));
        }
        *self.reader.write() = Some(Shared::new(ConfigReader::File(file)));
        self.read(ctx, registry, manager)
    }
    pub fn load_text(
        &self,
        text: &str,
        ctx: &mut impl Context,
        registry: &Registry,
        manager: &Manager,
    ) -> Result<i32> {
        *self.reader.write() = Some(Shared::new(ConfigReader::Text(Cursor::new(
            text.as_bytes().to_vec(),
        ))));
        self.read(ctx, registry, manager)
    }
    fn read(&self, ctx: &mut impl Context, registry: &Registry, manager: &Manager) -> Result<i32> {
        let mut text = String::new();
        self.reader()
            .expect("reader was published before parsing")
            .write()
            .by_ref()
            .take(64 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(Error::Io)?;
        let data = java_json::parse(&text).map_err(Error::Json)?;
        apply(ctx, registry, manager, &data)
    }
}
