//! Original VarbitDefinition lookup/decode and custom-definition loading.
//! Registry locks never span cache/reader/log callbacks; captured definitions
//! survive replacement in the real mutable mapping.
use super::item::Shared;
use rs09_game_core::{
    java_json::{self, Json},
    java_string::parse_i32,
};
use std::collections::HashMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Number,
    Underflow,
    Limit,
    Json(java_json::Error),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub id: i32,
    pub varp_id: i32,
    pub start_bit: i32,
    pub end_bit: i32,
}
pub type Handle = Shared<Definition>;
impl Definition {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            varp_id: 0,
            start_bit: 0,
            end_bit: 0,
        }
    }
    pub fn mask(&self) -> Result<i32> {
        if self.start_bit > self.end_bit {
            return Ok(0);
        }
        // Cache fields are bytes. Reject malformed custom ranges whose original
        // inclusive int loop would wrap forever or take billions of iterations.
        if self.end_bit == i32::MAX
            || i64::from(self.end_bit) - i64::from(self.start_bit) > 1_000_000
        {
            return Err(Error::Limit);
        }
        let mut mask = 0i32;
        for i in self.start_bit..=self.end_bit {
            mask |= 1i32.wrapping_shl(i.wrapping_sub(self.start_bit) as u32);
        }
        Ok(mask)
    }
    pub fn text(&self) -> String {
        format!(
            "ConfigFileDefinition [id={}, configId={}, bitShift={}, bitSize={}]",
            self.id, self.varp_id, self.start_bit, self.end_bit
        )
    }
}
pub type Mapping = Shared<HashMap<i32, Option<Handle>>>;
#[derive(Clone)]
pub struct Registry {
    mapping: Mapping,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            mapping: Shared::new(HashMap::new()),
        }
    }
}
pub trait Assets {
    fn file(&mut self, registry: &Registry, group: u32, file: u32) -> Result<Option<Vec<u8>>>;
}
impl Assets for std::sync::Arc<rs09_cache::archive::Archive> {
    fn file(&mut self, _: &Registry, group: u32, file: u32) -> Result<Option<Vec<u8>>> {
        rs09_cache::archive::Archive::file(self, group, file, [0; 4])
            .map(|v| v.map(|v| v.as_ref().clone()))
            .map_err(|e| Error::Binding(e.to_string()))
    }
}
impl Registry {
    pub fn mapping(&self) -> Mapping {
        self.mapping.clone()
    }
    pub fn create(&self, varp: i32, id: i32, start: i32, end: i32) {
        self.mapping.write().insert(
            id,
            Some(Shared::new(Definition {
                id,
                varp_id: varp,
                start_bit: start,
                end_bit: end,
            })),
        );
    }
    pub fn for_id(&self, id: i32, assets: &mut impl Assets) -> Result<Handle> {
        if let Some(handle) = self.mapping.read().get(&id).cloned().flatten() {
            return Ok(handle);
        }
        let mut definition = Definition::new(id);
        if let Some(data) = assets.file(self, (id as u32) >> 10, (id & 1023) as u32)? {
            let mut offset = 0;
            loop {
                let opcode = *data.get(offset).ok_or(Error::Underflow)?;
                offset += 1;
                match opcode {
                    0 => break,
                    1 => {
                        let bytes = data.get(offset..offset + 4).ok_or(Error::Underflow)?;
                        definition.varp_id = i32::from(u16::from_be_bytes([bytes[0], bytes[1]]));
                        definition.start_bit = i32::from(bytes[2]);
                        definition.end_bit = i32::from(bytes[3]);
                        offset += 4;
                    }
                    _ => {}
                }
            }
        }
        let handle = Shared::new(definition);
        self.mapping.write().insert(id, Some(handle.clone()));
        Ok(handle)
    }
    pub fn for_object_id(&self, id: i32, assets: &mut impl Assets) -> Result<Handle> {
        self.for_id(id, assets)
    }
    pub fn for_npc_id(&self, id: i32, assets: &mut impl Assets) -> Result<Handle> {
        self.for_id(id, assets)
    }
    pub fn for_item_id(&self, id: i32, assets: &mut impl Assets) -> Result<Handle> {
        self.for_id(id, assets)
    }
}
/// An actual opened reader token is published only after open succeeds. It may
/// be retained/rebound by the file service; registry writes are not rolled back.
#[derive(Clone)]
pub struct Loader {
    reader: Shared<Option<Reader>>,
}
impl Default for Loader {
    fn default() -> Self {
        Self {
            reader: Shared::new(None),
        }
    }
}
#[derive(Clone)]
pub struct Reader(pub Shared<String>);
pub trait Files {
    fn open(&mut self, path: &str) -> Result<Reader>;
    fn parse(&mut self, reader: Option<Reader>) -> Result<Json>;
    fn log(&mut self, count: i32) -> Result<()>;
}
impl Loader {
    pub fn reader(&self) -> Option<Reader> {
        self.reader.read().clone()
    }
    pub fn set_reader(&self, reader: Option<Reader>) {
        *self.reader.write() = reader;
    }
    pub fn load(
        &self,
        registry: &Registry,
        prefix: Option<&str>,
        files: &mut impl Files,
    ) -> Result<i32> {
        let path = format!("{}varbit_definitions.json", prefix.unwrap_or("null"));
        let reader = files.open(&path)?;
        self.set_reader(Some(reader));
        let root = files.parse(self.reader())?;
        let Json::Array(rows) = root else {
            return Err(if matches!(root, Json::Null) {
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
            let read = |key: &str| {
                parse_i32(&row.get(key).unwrap_or(&Json::Null).text()).ok_or(Error::Number)
            };
            let varp = read("varpId")?;
            let id = read("varbitId")?;
            let start = read("startBit")?;
            let end = read("endBit")?;
            registry.create(varp, id, start, end);
            count = count.wrapping_add(1);
        }
        files.log(count)?;
        Ok(count)
    }
    pub fn load_text(registry: &Registry, text: &str) -> Result<i32> {
        struct Text<'a>(&'a str);
        impl Files for Text<'_> {
            fn open(&mut self, _: &str) -> Result<Reader> {
                Ok(Reader(Shared::new(self.0.into())))
            }
            fn parse(&mut self, reader: Option<Reader>) -> Result<Json> {
                java_json::parse(&reader.ok_or(Error::Null)?.0.read()).map_err(Error::Json)
            }
            fn log(&mut self, _: i32) -> Result<()> {
                Ok(())
            }
        }
        Self::default().load(registry, Some(""), &mut Text(text))
    }
}
