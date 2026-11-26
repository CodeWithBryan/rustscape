//! Native ServerConfigParser assignments. Runtime game/world consumers are separate.
mod schema;
mod toml_compat;
use crate::auth::factory::{DatabaseSettings, Settings};
use rs09_game_core::java_string::parse_i32;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Copy)]
enum Initial {
    Null,
    Bool(bool),
    Int(i32),
    Text(&'static str),
    Location([i32; 3]),
}
impl Initial {
    fn value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(b) => json!(b),
            Self::Int(n) => json!(n),
            Self::Text(s) => json!(s),
            Self::Location(p) => json!(p),
        }
    }
}
#[derive(Clone, Copy)]
enum Kind {
    Text(Option<&'static str>),
    Bool(Option<bool>),
    Int(Option<i64>),
    Path,
    Location,
    Database,
    ServerGeName,
    LogLevel,
}
struct Rule {
    field: &'static str,
    key: &'static str,
    kind: Kind,
    initial: Initial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    Syntax,
    Missing,
    Type,
    Value,
    Path,
    Io,
    FileMissing,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub key: String,
    pub kind: ErrorKind,
}
impl Error {
    fn at(key: &str, kind: ErrorKind) -> Self {
        Self {
            key: key.to_owned(),
            kind,
        }
    }
    /// The reference exits with code zero on these configuration failures.
    pub fn reference_exit_code(&self) -> Option<i32> {
        matches!(self.kind, ErrorKind::Path | ErrorKind::FileMissing).then_some(0)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "configuration {:?} at {}", self.kind, self.key)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

pub fn parse_path(text: &str, separator: char) -> String {
    let split = if text.contains('/') {
        '/'
    } else if text.contains('\\') {
        '\\'
    } else {
        return text.into();
    };
    let mut path = String::new();
    for token in text.split(split).filter(|token| !token.is_empty()) {
        path.push_str(token);
        path.push(separator);
    }
    path
}
pub fn parse_location(text: &str) -> Result<[i32; 3]> {
    let tokens = text
        .split(',')
        .map(|token| parse_i32(token).ok_or_else(|| Error::at("location", ErrorKind::Value)))
        .collect::<Result<Vec<_>>>()?;
    if tokens.len() < 3 {
        return Err(Error::at("location", ErrorKind::Value));
    }
    // Location's constructor stores signed 15-bit x/y and a two-bit plane.
    let signed = |n: i32| {
        let bits = n & 0x7fff;
        if bits >= 0x4000 { bits - 0x8000 } else { bits }
    };
    Ok([
        signed(tokens[0]),
        signed(tokens[1]),
        tokens[2].rem_euclid(4),
    ])
}
pub fn parse_java_version(text: &str) -> Result<i32> {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let range = if text.starts_with("1.") { 2..3 } else { 0..2 };
    let part = units
        .get(range)
        .and_then(|units| String::from_utf16(units).ok())
        .ok_or_else(|| Error::at("java.version", ErrorKind::Value))?;
    parse_i32(&part).ok_or_else(|| Error::at("java.version", ErrorKind::Value))
}
struct Data<'a> {
    root: &'a toml::Table,
}
impl Data<'_> {
    fn get(&self, key: &str) -> Option<&toml::Value> {
        let mut parts = key.split('.');
        let mut value = self.root.get(parts.next()?)?;
        for part in parts {
            value = value.as_table()?.get(part)?;
        }
        Some(value)
    }
    fn text(&self, key: &str, default: Option<&str>) -> Result<Option<String>> {
        match self.get(key) {
            None => Ok(default.map(str::to_owned)),
            Some(toml::Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(Error::at(key, ErrorKind::Type)),
        }
    }
    fn required_text(&self, key: &str) -> Result<String> {
        self.text(key, None)?
            .ok_or_else(|| Error::at(key, ErrorKind::Missing))
    }
    fn boolean(&self, key: &str, default: Option<bool>) -> Result<bool> {
        match self.get(key) {
            None => default.ok_or_else(|| Error::at(key, ErrorKind::Missing)),
            Some(toml::Value::Boolean(b)) => Ok(*b),
            Some(_) => Err(Error::at(key, ErrorKind::Type)),
        }
    }
    fn integer(&self, key: &str, default: Option<i64>) -> Result<i32> {
        match self.get(key) {
            None => default
                .map(|n| n as i32)
                .ok_or_else(|| Error::at(key, ErrorKind::Missing)),
            Some(toml::Value::Integer(n)) => Ok(*n as i32),
            Some(_) => Err(Error::at(key, ErrorKind::Type)),
        }
    }
    fn integer_text(&self, key: &str) -> Result<i32> {
        parse_i32(&self.required_text(key)?).ok_or_else(|| Error::at(key, ErrorKind::Value))
    }
}

/// Startup data contains credentials. Do not log the complete configuration.
pub struct Configuration {
    constants: BTreeMap<String, Value>,
    world: Option<WorldSettings>,
    toml_data: Option<toml::Table>,
}
/// Validated startup settings; values are immutable after parsing.
pub struct WorldSettings {
    values: BTreeMap<String, Value>,
}
impl WorldSettings {
    pub fn values(&self) -> &BTreeMap<String, Value> {
        &self.values
    }
    pub fn world_id(&self) -> i32 {
        self.values["worldId"].as_i64().unwrap() as i32
    }
    pub fn summary(&self) -> String {
        format!(
            "GameSettings [name={}, debug={}, devMode={}, gui={}, worldId={}]",
            self.values["name"].as_str().unwrap(),
            self.values["isBeta"],
            self.values["isDevMode"],
            self.values["isGui"],
            self.world_id()
        )
    }
}
impl Default for Configuration {
    fn default() -> Self {
        let mut constants = schema::SERVER
            .iter()
            .map(|rule| (rule.field.to_owned(), rule.initial.value()))
            .collect::<BTreeMap<_, _>>();
        constants.insert("JAVA_VERSION".into(), json!(8));
        Self {
            constants,
            world: None,
            toml_data: None,
        }
    }
}
impl Configuration {
    pub fn constants(&self) -> &BTreeMap<String, Value> {
        &self.constants
    }
    pub fn world(&self) -> Option<&WorldSettings> {
        self.world.as_ref()
    }
    pub fn toml_loaded(&self) -> bool {
        self.toml_data.is_some()
    }
    pub fn data(&self) -> Option<&toml::Table> {
        self.toml_data.as_ref()
    }
    /// ContentAPI.getServerConfig Boolean access, including an empty pre-load view.
    pub fn boolean(&self, key: &str, default: bool) -> Result<bool> {
        match self.toml_data.as_ref() {
            None => Ok(default),
            Some(root) => Data { root }.boolean(key, Some(default)),
        }
    }
    pub fn load(path: &Path) -> Result<Self> {
        let raw = path
            .to_str()
            .ok_or_else(|| Error::at("file", ErrorKind::Type))?;
        let mut reference = parse_path(raw, std::path::MAIN_SEPARATOR);
        // parsePath drops a leading separator; the file itself is read from the
        // absolute path a caller gave (a Unix world directory).
        if raw.starts_with('/') && std::path::MAIN_SEPARATOR == '/' {
            reference.insert(0, '/');
        }
        // java.io.File discards the final separator that parsePath appends to
        // filenames. Keep stored game paths unchanged; normalize only I/O.
        let file = Path::new(reference.trim_end_matches(std::path::MAIN_SEPARATOR));
        let text = std::fs::read_to_string(file).map_err(|error| {
            Error::at(
                "file",
                if error.kind() == std::io::ErrorKind::NotFound {
                    ErrorKind::FileMissing
                } else {
                    ErrorKind::Io
                },
            )
        })?;
        let mut config = Self::default();
        config.apply(&text, std::path::MAIN_SEPARATOR, None)?;
        Ok(config)
    }
    /// Legacy Java version is an oracle input, never a runtime JVM dependency.
    pub fn apply(
        &mut self,
        text: &str,
        separator: char,
        legacy_java_version: Option<&str>,
    ) -> Result<()> {
        let table = toml_compat::parse(text)?;
        self.toml_data = Some(table);
        let data = Data {
            root: self.toml_data.as_ref().unwrap(),
        };
        for rule in schema::SERVER {
            let value = match rule.kind {
                Kind::Text(default) => {
                    let text = data.text(rule.key, default)?;
                    if text.is_none() && !matches!(rule.initial, Initial::Null) {
                        return Err(Error::at(rule.key, ErrorKind::Missing));
                    }
                    text.map_or(Value::Null, Value::String)
                }
                Kind::Bool(default) => json!(data.boolean(rule.key, default)?),
                Kind::Int(default) => json!(data.integer(rule.key, default)?),
                Kind::Location => json!(
                    parse_location(&data.required_text(rule.key)?)
                        .map_err(|_| Error::at(rule.key, ErrorKind::Value))?
                ),
                Kind::Path => {
                    let result = (|| -> Result<String> {
                        let raw = data.required_or_path(rule.key)?;
                        let base = self.constants["DATA_PATH"]
                            .as_str()
                            .ok_or_else(|| Error::at(rule.key, ErrorKind::Missing))?;
                        Ok(parse_path(&raw.replace("@data", base), separator))
                    })();
                    json!(result.map_err(|_| Error::at(rule.key, ErrorKind::Path))?)
                }
                Kind::Database => {
                    let address = self.constants["DATABASE_ADDRESS"]
                        .as_str()
                        .unwrap_or("null");
                    let port = self.constants["DATABASE_PORT"].as_str().unwrap_or("null");
                    json!([
                        format!("{address}:{port}"),
                        self.constants["DATABASE_NAME"],
                        self.constants["DATABASE_USER"],
                        self.constants["DATABASE_PASS"]
                    ])
                }
                Kind::ServerGeName => data
                    .text(rule.key, None)?
                    .map_or_else(|| self.constants["SERVER_NAME"].clone(), Value::String),
                Kind::LogLevel => {
                    let level = data
                        .text(rule.key, Some("VERBOSE"))?
                        .unwrap()
                        .to_uppercase();
                    json!(match level.as_str() {
                        "SILENT" | "CAUTIOUS" | "DETAILED" | "VERBOSE" => level,
                        _ => "VERBOSE".into(),
                    })
                }
            };
            self.constants.insert(rule.field.into(), value);
        }
        self.world = Some(self.parse_world(&data)?);
        // The pinned POM targets JDK 11. Native persistence must use its compact
        // JSON branch, without consulting a JVM or instantiating a script engine.
        let version = legacy_java_version
            .map(parse_java_version)
            .transpose()?
            .unwrap_or(11);
        self.constants.insert("JAVA_VERSION".into(), json!(version));
        Ok(())
    }
    fn parse_world(&self, data: &Data<'_>) -> Result<WorldSettings> {
        let mut values = BTreeMap::new();
        macro_rules! put {
            ($name:expr,$value:expr) => {
                values.insert($name.into(), json!($value));
            };
        }
        put!("name", self.constants["SERVER_NAME"]);
        put!("isBeta", data.boolean("world.debug", None)?);
        put!("isDevMode", data.boolean("world.dev", None)?);
        put!("isGui", data.boolean("world.start_gui", None)?);
        put!("worldId", data.integer_text("world.world_id")?);
        put!("countryIndex", data.integer_text("world.country_id")?);
        put!("activity", data.required_text("world.activity")?);
        put!("isMembers", data.boolean("world.members", None)?);
        put!("isPvp", data.boolean("world.pvp", None)?);
        put!("isQuickChat", false);
        put!("isLootshare", false);
        put!("msAddress", data.required_text("server.msip")?);
        for (field, key) in [
            ("enable_default_clan", "world.enable_default_clan"),
            ("enable_bots", "world.enable_bots"),
            ("autostock_ge", "world.autostock_ge"),
            ("allow_token_purchase", "world.allow_token_purchase"),
            ("skillcape_perks", "world.skillcape_perks"),
            ("increased_door_time", "world.increased_door_time"),
            ("enabled_botting", "world.enable_botting"),
        ] {
            put!(field, data.boolean(key, None)?);
        }
        put!("max_adv_bots", data.integer("world.max_adv_bots", None)?);
        put!(
            "enable_doubling_money_scammers",
            data.boolean("world.enable_doubling_money_scammers", Some(false))?
        );
        put!(
            "wild_pvp_enabled",
            data.boolean("world.wild_pvp_enabled", None)?
        );
        put!(
            "jad_practice_enabled",
            data.boolean("world.jad_practice_enabled", None)?
        );
        put!(
            "ge_announcement_limit",
            data.integer("world.ge_announcement_limit", Some(500))?
        );
        put!(
            "smartpathfinder_bfs",
            data.boolean("world.smartpathfinder_bfs", Some(false))?
        );
        put!(
            "enable_castle_wars",
            data.boolean("world.enable_castle_wars", Some(false))?
        );
        put!("message_model", data.integer_text("world.motw_identifier")?);
        put!(
            "message_string",
            data.required_text("world.motw_text")?
                .replace("@name", self.constants["SERVER_NAME"].as_str().unwrap())
        );
        let dev = values["isDevMode"].as_bool().unwrap();
        put!("isHosted", !dev);
        Ok(WorldSettings { values })
    }
    pub fn auth_settings(&self) -> Settings {
        let string = |key: &str| self.constants[key].as_str().map(str::to_owned);
        Settings {
            use_auth: self.constants["USE_AUTH"].as_bool().unwrap(),
            persist_accounts: self.constants["PERSIST_ACCOUNTS"].as_bool().unwrap(),
            default_admin: self.constants["NOAUTH_DEFAULT_ADMIN"].as_bool().unwrap(),
            database: DatabaseSettings {
                address: string("DATABASE_ADDRESS"),
                name: string("DATABASE_NAME"),
                username: string("DATABASE_USER"),
                password: string("DATABASE_PASS"),
                port: string("DATABASE_PORT"),
            },
        }
    }
    pub fn listener_ports(&self) -> Option<(i32, Option<i32>)> {
        let world = self.world.as_ref()?.world_id();
        let websocket = if self.constants["WEBSOCKET_ENABLED"].as_bool().unwrap() {
            let port = self.constants["WEBSOCKET_PORT"].as_i64().unwrap() as i32;
            Some(if port > 0 {
                port
            } else {
                53594i32.wrapping_add(world)
            })
        } else {
            None
        };
        Some((43594i32.wrapping_add(world), websocket))
    }
}
impl Data<'_> {
    fn required_or_path(&self, key: &str) -> Result<String> {
        Ok(self.text(key, Some("@data/"))?.unwrap())
    }
}
