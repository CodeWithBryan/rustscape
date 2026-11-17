//! Original NPCConfigParser conversions, field iteration and publication order.
use super::{Error, Files, Registry, Result};
pub use crate::definitions::item_config::Reader;
use crate::definitions::{
    item::Shared,
    property::{Animation, ClueLevel, CombatStyle, Property},
};
use rs09_game_core::{
    java_json::{self, Json},
    java_string::parse_i32,
};
use std::{
    fs::File,
    io::{Cursor, Read},
    path::Path,
};
#[derive(Clone, Copy, Debug)]
pub enum Level {
    Fine,
    Warn,
}
pub trait Context: Files {
    fn log(&mut self, registry: &mut Registry, level: Level, text: String) -> Result<()>;
}
#[derive(Default)]
pub struct Parser {
    pub reader: Option<Reader>,
}
impl Parser {
    pub fn load_file(
        &mut self,
        path: &Path,
        registry: &mut Registry,
        ctx: &mut impl Context,
    ) -> Result<i32> {
        let file = File::open(path).map_err(Error::Io)?;
        if file.metadata().map_err(Error::Io)?.is_dir() {
            return Err(Error::Io(std::io::Error::from(
                std::io::ErrorKind::IsADirectory,
            )));
        }
        self.reader = Some(Reader::File(file));
        self.load_reader(registry, ctx)
    }
    pub fn load_text(
        &mut self,
        text: &str,
        registry: &mut Registry,
        ctx: &mut impl Context,
    ) -> Result<i32> {
        self.reader = Some(Reader::Text(Cursor::new(text.as_bytes().to_vec())));
        self.load_reader(registry, ctx)
    }
    fn load_reader(&mut self, registry: &mut Registry, ctx: &mut impl Context) -> Result<i32> {
        let mut text = String::new();
        self.reader
            .as_mut()
            .ok_or(Error::Null)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(Error::Io)?;
        if text.len() > 64 * 1024 * 1024 {
            return Err(Error::Limit);
        }
        let data = java_json::parse(&text).map_err(Error::Json)?;
        self.apply(&data, registry, ctx)
    }
    pub fn apply(
        &self,
        data: &Json,
        registry: &mut Registry,
        ctx: &mut impl Context,
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
            let definition = registry.for_id(ctx, id)?;
            let handlers = definition.read().handlers.clone();
            for index in java_json::key_order(entries) {
                let (key, value) = &entries[index];
                let text = value.text();
                if text.is_empty() || text == "null" {
                    continue;
                }
                let value = match key.as_str() {
                    "melee_animation" | "range_animation" | "magic_animation"
                    | "death_animation" | "defence_animation" => {
                        Property::Animation(Shared::new(Animation::high(integer(&text)?)))
                    }
                    "combat_style" | "protect_style" => Property::CombatStyle(
                        *CombatStyle::ALL
                            .get(integer(&text)? as usize)
                            .ok_or(Error::Index)?,
                    ),
                    "clue_level" => Property::ClueLevel(
                        *ClueLevel::ALL
                            .get(integer(&text)? as usize)
                            .ok_or(Error::Index)?,
                    ),
                    "name" | "examine" | "force_talk" => Property::Text(text),
                    "combat_audio" | "bonuses" => {
                        let a = text.split(',').map(integer).collect::<Result<Vec<_>>>()?;
                        Property::IntArray(Shared::new(a))
                    }
                    "spawn_animation" | "id" | "lifepoints" | "attack_level" | "strength_level"
                    | "defence_level" | "range_level" | "movement_radius" | "agg_radius"
                    | "attack_speed" | "poison_amount" | "respawn_delay" | "start_gfx"
                    | "projectile" | "end_gfx" | "weakness" | "slayer_task" | "start_height"
                    | "prj_height" | "end_height" | "spell_id" | "death_gfx" | "magic_level" => {
                        Property::Int(integer(&text)?)
                    }
                    "slayer_exp" => Property::Double(
                        rs09_game_core::java_double::parse(&text).ok_or(Error::Number)?,
                    ),
                    "safespot" | "aggressive" | "poisonous" | "poison_immune" | "facing_booth"
                    | "can_tolerate" | "water_npc" => Property::Bool(
                        rs09_game_core::java_string::equals_ignore_case(&text, "true"),
                    ),
                    _ => {
                        ctx.log(
                            registry,
                            Level::Warn,
                            format!("Unhandled key for npc config: {key}"),
                        )?;
                        continue;
                    }
                };
                handlers.write().insert(key.clone(), value);
            }
            count = count.wrapping_add(1);
        }
        ctx.log(
            registry,
            Level::Fine,
            format!("Parsed {count} NPC configurations"),
        )?;
        Ok(count)
    }
}
fn integer(s: &str) -> Result<i32> {
    parse_i32(s).ok_or(Error::Number)
}
