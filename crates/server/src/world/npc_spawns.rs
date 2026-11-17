//! Original NPCSpawner.load parsing and publication order. Creation, definition
//! configuration and NPC.init are explicit owning-world services.
use super::doors::Location;
use rs09_game_core::{
    direction::Direction,
    java_json::{self, Json},
    java_string::parse_i32,
};

#[derive(Debug)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
    Json(java_json::Error),
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub trait Context {
    type Npc;
    fn create(&mut self, id: i32, location: Location) -> Result<Self::Npc>;
    fn walks(&mut self, npc: &Self::Npc, walks: bool) -> Result<()>;
    fn direction(&mut self, npc: &Self::Npc, direction: Direction) -> Result<()>;
    fn attribute(&mut self, npc: &Self::Npc, key: &'static str, value: bool) -> Result<()>;
    fn facing_booth(&mut self, npc: &Self::Npc) -> Result<bool>;
    fn init(&mut self, npc: &Self::Npc) -> Result<()>;
    fn log(&mut self, count: i32) -> Result<()>;
}
pub fn load(ctx: &mut impl Context, text: &str) -> Result<i32> {
    let data = java_json::parse(text).map_err(Error::Json)?;
    apply(ctx, &data)
}
pub fn apply(ctx: &mut impl Context, data: &Json) -> Result<i32> {
    let Json::Array(rows) = data else {
        return Err(if matches!(data, Json::Null) {
            Error::Null
        } else {
            Error::Type
        });
    };
    let mut count = 0i32;
    for row in rows {
        if !matches!(row, Json::Object(_)) {
            return Err(if matches!(row, Json::Null) {
                Error::Null
            } else {
                Error::Type
            });
        }
        let text = row.get("loc_data").unwrap_or(&Json::Null).text();
        let id =
            parse_i32(&row.get("npc_id").unwrap_or(&Json::Null).text()).ok_or(Error::Number)?;
        for part in text.split('-') {
            if part.is_empty() {
                continue;
            }
            let part = part.replace(['{', '}'], "");
            let tokens: Vec<_> = part.split(',').collect();
            let token = |index: usize| {
                tokens
                    .get(index)
                    .map(|s| s.trim_matches(|c: char| c <= ' '))
                    .ok_or(Error::Index)
            };
            let int = |index| parse_i32(token(index)?).ok_or(Error::Number);
            let location = Location::new(int(0)?, int(1)?, int(2)?);
            let npc = ctx.create(id, location)?;
            ctx.walks(&npc, token(3)? == "1")?;
            let direction = Direction::ALL
                .get(int(4)? as usize)
                .copied()
                .ok_or(Error::Index)?;
            ctx.direction(&npc, direction)?;
            ctx.attribute(&npc, "spawned:npc", true)?;
            if ctx.facing_booth(&npc)? {
                ctx.attribute(&npc, "facing_booth", true)?;
            }
            ctx.init(&npc)?;
            count = count.wrapping_add(1);
        }
    }
    ctx.log(count)?;
    Ok(count)
}
