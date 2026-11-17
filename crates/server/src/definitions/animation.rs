//! Original server AnimationDefinition decoder, mutable cached definitions and
//! duration arithmetic. Its high-word expressions differ from the RT4 client.
use super::item::Shared;
use rs09_cache::archive::Archive;
use rs09_protocol::buffer::Reader;
use std::collections::HashMap;
pub type Ints = Shared<Vec<i32>>;
pub type Bools = Shared<Vec<bool>>;
pub type Sounds = Shared<Vec<Option<Ints>>>;
pub type Handle = Shared<Definition>;
#[derive(Debug)]
pub enum Error {
    Decode(rs09_protocol::Error),
    Binding(String),
}
impl From<rs09_protocol::Error> for Error {
    fn from(e: rs09_protocol::Error) -> Self {
        Self::Decode(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub struct Definition {
    pub an_int2136: i32,
    pub an_int2137: i32,
    pub frames: Option<Ints>,
    pub an_int2140: i32,
    pub a_boolean2141: bool,
    pub an_int2142: i32,
    pub emote_item: i32,
    pub an_int2144: i32,
    pub handled_sounds: Option<Sounds>,
    pub flow: Option<Bools>,
    pub secondary_frames: Option<Ints>,
    pub a_boolean2152: bool,
    pub durations: Option<Ints>,
    pub an_int2155: i32,
    pub a_boolean2158: bool,
    pub a_boolean2159: bool,
    pub an_int2162: i32,
    pub an_int2163: i32,
    pub new_header: bool,
    pub sound_min_delay: Option<Ints>,
    pub sound_max_delay: Option<Ints>,
    pub an_int_array1362: Option<Ints>,
    pub effect2_sound: bool,
}
impl Default for Definition {
    fn default() -> Self {
        Self {
            an_int2136: 99,
            an_int2137: 0,
            frames: None,
            an_int2140: -1,
            a_boolean2141: false,
            an_int2142: 5,
            emote_item: -1,
            an_int2144: -1,
            handled_sounds: None,
            flow: None,
            secondary_frames: None,
            a_boolean2152: false,
            durations: None,
            an_int2155: 2,
            a_boolean2158: false,
            a_boolean2159: false,
            an_int2162: -1,
            an_int2163: -1,
            new_header: false,
            sound_min_delay: None,
            sound_max_delay: None,
            an_int_array1362: None,
            effect2_sound: false,
        }
    }
}
impl Definition {
    pub fn duration_ms(&self) -> i32 {
        self.durations.as_ref().map_or(0, |v| {
            v.read()
                .iter()
                .filter(|&&n| n <= 100)
                .fold(0i32, |a, &n| a.wrapping_add(n.wrapping_mul(20)))
        })
    }
    pub fn cycles(&self) -> i32 {
        self.durations
            .as_ref()
            .map_or(0, |v| v.read().iter().fold(0i32, |a, &n| a.wrapping_add(n)))
    }
    pub fn duration_ticks(&self) -> i32 {
        (self.duration_ms() / 600).max(1)
    }
    pub fn finish(&mut self) {
        if self.an_int2140 == -1 {
            self.an_int2140 = if self.flow.is_none() { 0 } else { 2 };
        }
        if self.an_int2162 == -1 {
            self.an_int2162 = if self.flow.is_none() { 0 } else { 2 };
        }
    }
    pub fn read_value_loop(&mut self, r: &mut Reader<'_>) -> Result<()> {
        loop {
            let opcode = r.g1()?;
            if opcode == 0 {
                return Ok(());
            }
            self.read_values(r, opcode)?;
        }
    }
    pub fn read_values(&mut self, r: &mut Reader<'_>, opcode: i32) -> Result<()> {
        match opcode {
            1 => {
                let count = r.g2()? as usize;
                let durations = Shared::new(vec![0; count]);
                self.durations = Some(durations.clone());
                for i in 0..count {
                    durations.write()[i] = r.g2()?;
                }
                let frames = Shared::new(vec![0; count]);
                self.frames = Some(frames.clone());
                for i in 0..count {
                    frames.write()[i] = r.g2()?;
                }
                for i in 0..count {
                    // Java parses short & 0xFFFF << 16 as short & (0xFFFF << 16).
                    let high = (r.g2()? as i16 as i32) & !0xffff;
                    let value = frames.read()[i];
                    frames.write()[i] = high.wrapping_add(value);
                }
            }
            2 => self.an_int2163 = r.g2()?,
            3 => {
                let flow = Shared::new(vec![false; 256]);
                self.flow = Some(flow.clone());
                let count = r.g1()?;
                for _ in 0..count {
                    let i = r.g1()? as usize;
                    flow.write()[i] = true;
                }
            }
            4 => self.a_boolean2152 = true,
            5 => self.an_int2142 = r.g1()?,
            6 => self.an_int2144 = r.g2()?,
            7 => self.emote_item = r.g2()?,
            8 => self.an_int2136 = r.g1()?,
            9 => self.an_int2140 = r.g1()?,
            10 => self.an_int2162 = r.g1()?,
            11 => self.an_int2155 = r.g1()?,
            12 => {
                let count = r.g1()? as usize;
                let frames = Shared::new(vec![0; count]);
                self.secondary_frames = Some(frames.clone());
                for i in 0..count {
                    frames.write()[i] = r.g2()?;
                }
                for i in 0..count {
                    let high = (r.g2()? as i16 as i32) & !0xffff;
                    let value = frames.read()[i];
                    frames.write()[i] = high.wrapping_add(value);
                }
            }
            13 => {
                let count = r.g2()? as usize;
                let sounds = Shared::new(vec![None; count]);
                self.handled_sounds = Some(sounds.clone());
                for i in 0..count {
                    let size = r.g1()? as usize;
                    if size > 0 {
                        let row = Shared::new(vec![0; size]);
                        sounds.write()[i] = Some(row.clone());
                        // ByteBufferUtils.getMedium performs three individual byte reads.
                        row.write()[0] = (r.g1()? << 16) + (r.g1()? << 8) + r.g1()?;
                        for n in 1..size {
                            row.write()[n] = r.g2()?;
                        }
                    }
                }
            }
            14 => self.a_boolean2141 = true,
            _ => {}
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Registry {
    pub entries: Shared<HashMap<i32, Option<Handle>>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            entries: Shared::new(HashMap::new()),
        }
    }
}
pub trait Source {
    fn file(&mut self, registry: &Registry, group: i32, file: i32) -> Result<Option<Vec<u8>>>;
}
impl Registry {
    /// Like the complete original forId, failures are not cached and return null.
    pub fn for_id(&self, source: &mut impl Source, id: i32) -> Option<Handle> {
        self.resolve(source, id).ok()
    }
    pub fn resolve(&self, source: &mut impl Source, id: i32) -> Result<Handle> {
        if let Some(Some(value)) = self.entries.read().get(&id).cloned() {
            return Ok(value);
        }
        let data = source.file(self, ((id as u32) >> 7) as i32, id & 127)?;
        let mut definition = Definition::default();
        if let Some(data) = data {
            definition.read_value_loop(&mut Reader::new(&data))?;
        }
        definition.finish();
        let definition = Shared::new(definition);
        self.entries.write().insert(id, Some(definition.clone()));
        Ok(definition)
    }
}
pub struct CacheSource(pub Archive);
impl Source for CacheSource {
    fn file(&mut self, _: &Registry, group: i32, file: i32) -> Result<Option<Vec<u8>>> {
        self.0
            .file(group as u32, file as u32, [0; 4])
            .map(|v| v.map(|v| v.as_ref().clone()))
            .map_err(|e| Error::Binding(e.to_string()))
    }
}
