//! Original ItemConfigParser conversions and assignment order over native definitions.
pub mod constants;
pub mod json;
use super::{
    item::{Registry, Shared},
    property::{Animation, Audio, Property, Requirements},
};
use json::Json;
use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
};
#[derive(Debug)]
pub enum Error {
    Json(json::Error),
    Null,
    Type,
    Number,
    Bounds,
    Io(std::io::Error),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub trait Logger {
    fn parsed(&mut self, count: i32) -> Result<()>;
}
pub enum Reader {
    File(File),
    Text(Cursor<Vec<u8>>),
}
impl Read for Reader {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::File(file) => file.read(bytes),
            Self::Text(text) => text.read(bytes),
        }
    }
}
#[derive(Default)]
pub struct Parser {
    pub reader: Option<Reader>,
}
impl Parser {
    pub fn load_file(
        &mut self,
        path: &Path,
        definitions: &Registry,
        logger: &mut impl Logger,
    ) -> Result<i32> {
        let file = File::open(path).map_err(Error::Io)?;
        if file.metadata().map_err(Error::Io)?.is_dir() {
            return Err(Error::Io(std::io::Error::from(
                std::io::ErrorKind::IsADirectory,
            )));
        }
        self.reader = Some(Reader::File(file));
        self.load_reader(definitions, logger)
    }
    pub fn load_text(
        &mut self,
        text: &str,
        definitions: &Registry,
        logger: &mut impl Logger,
    ) -> Result<i32> {
        self.reader = Some(Reader::Text(Cursor::new(text.as_bytes().to_vec())));
        self.load_reader(definitions, logger)
    }
    fn load_reader(&mut self, definitions: &Registry, logger: &mut impl Logger) -> Result<i32> {
        let mut text = String::new();
        self.reader
            .as_mut()
            .ok_or(Error::Null)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(Error::Io)?;
        let data = json::parse(&text).map_err(Error::Json)?;
        self.apply(&data, definitions, logger)
    }
    pub fn apply(
        &self,
        data: &Json,
        definitions: &Registry,
        logger: &mut impl Logger,
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
            let Json::Object(entries) = row else {
                return Err(if matches!(row, Json::Null) {
                    Error::Null
                } else {
                    Error::Type
                });
            };
            let id = integer(&row.get("id").unwrap_or(&Json::Null).text())?;
            let definition = definitions.for_id(id);
            let requirements = Shared::new(Requirements::default());
            for index in json::key_order(entries) {
                let (key, value) = &entries[index];
                let text = value.text();
                if text.is_empty() || text == "null" {
                    continue;
                }
                let property = match key.as_str() {
                    "defence_anim" => {
                        Property::Animation(Shared::new(Animation::high(integer(&text)?)))
                    }
                    "requirements" => {
                        definition
                            .write()
                            .handlers
                            .insert(key.clone(), Property::Requirements(requirements.clone()));
                        for entry in text.split('-') {
                            let cleaned = entry.replace(['{', '}'], "");
                            let tokens: Vec<_> = cleaned.split(',').collect();
                            let skill = integer(tokens[0])?;
                            let level = integer(tokens.get(1).ok_or(Error::Bounds)?)?;
                            requirements.write().insert(skill, level);
                        }
                        continue;
                    }
                    "attack_audios" => {
                        let mut values = Vec::new();
                        for part in text.split(',') {
                            values.push(Shared::new(Audio::new(integer(part)?)));
                        }
                        Property::Audios(Shared::new(values))
                    }
                    "attack_anims" => {
                        let mut values = Vec::new();
                        for part in text.split(',') {
                            values.push(Some(Shared::new(Animation::high(integer(part)?))));
                        }
                        Property::Animations(Shared::new(values))
                    }
                    "absorb" | "bonuses" => {
                        let mut values = Vec::new();
                        for part in text.split(',') {
                            values.push(integer(part)?);
                        }
                        Property::IntArray(Shared::new(values))
                    }
                    "fun_weapon" | "rare_item" | "bankable" | "two_handed" | "has_special"
                    | "remove_sleeves" | "remove_beard" | "remove_head" | "hat" | "destroy"
                    | "lendable" | "tradeable" | "alchemizable" => {
                        Property::Bool(text.eq_ignore_ascii_case("true"))
                    }
                    "weight" => Property::Double(
                        rs09_game_core::java_double::parse(&text).ok_or(Error::Number)?,
                    ),
                    "equip_audio"
                    | "point_price"
                    | "tokkul_price"
                    | "archery_ticket_price"
                    | "castle_wars_ticket_price"
                    | "ge_buy_limit"
                    | "weapon_interface"
                    | "attack_speed"
                    | "equipment_slot"
                    | "render_anim"
                    | "turn90ccw_anim"
                    | "turn90cw_anim"
                    | "turn180_anim"
                    | "run_anim"
                    | "walk_anim"
                    | "stand_anim"
                    | "low_alchemy"
                    | "high_alchemy"
                    | "grand_exchange_price"
                    | "id" => Property::Int(integer(&text)?),
                    "examine" | "destroy_message" | "name" => Property::Text(text),
                    _ => continue,
                };
                definition.write().handlers.insert(key.clone(), property);
            }
            count = count.wrapping_add(1);
        }
        logger.parsed(count)?;
        Ok(count)
    }
}
fn integer(text: &str) -> Result<i32> {
    rs09_game_core::java_string::parse_i32(text).ok_or(Error::Number)
}
