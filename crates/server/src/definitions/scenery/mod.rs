//! Server scenery cache data, registry ordering and child/action selection.
//! Content handlers, player varp/varbit access and logging are native bindings.
use super::{item::Shared, property::Property};
use rs09_cache::{CacheError, archive::Archive};
use rs09_protocol::buffer::Reader;
use std::{collections::HashMap, fmt};
mod data;
pub use data::SceneryDefinition;
pub type Handle = Shared<SceneryDefinition>;
#[derive(Debug)]
pub enum Error {
    Decode(rs09_protocol::Error),
    Cache(CacheError),
    Null,
    Index,
    Position,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<rs09_protocol::Error> for Error {
    fn from(e: rs09_protocol::Error) -> Self {
        Self::Decode(e)
    }
}
impl From<CacheError> for Error {
    fn from(e: CacheError) -> Self {
        Self::Cache(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub trait LogContext {
    fn log(&mut self, registry: &mut Registry, message: String) -> Result<()>;
}
pub trait PlayerContext {
    /// Resolves the original varbit definition and calls its getValue binding.
    fn varbit(&mut self, id: i32, player: usize) -> Result<Option<i32>>;
    fn varp(&mut self, id: i32, player: usize) -> Result<i32>;
}
#[derive(Default)]
pub struct Registry {
    pub definitions: HashMap<i32, Handle>,
}
impl Registry {
    pub fn for_id(&mut self, id: i32) -> Handle {
        self.definitions
            .entry(id)
            .or_insert_with(|| {
                Shared::new(SceneryDefinition {
                    id,
                    ..SceneryDefinition::default()
                })
            })
            .clone()
    }
    pub fn configure(&mut self, h: &Handle) -> Result<()> {
        if h.read().interactable == -1 {
            h.write().interactable = 0;
            let (models, config) = {
                let h = h.read();
                (h.model_ids.clone(), h.model_configuration.clone())
            };
            if models.is_some()
                && match config {
                    None => true,
                    Some(c) => *c.read().first().ok_or(Error::Index)? == 10,
                }
            {
                h.write().interactable = 1;
            }
            for i in 0..5 {
                let options = h.read().options.clone().ok_or(Error::Null)?;
                if options.read().get(i).ok_or(Error::Index)?.is_some() {
                    h.write().interactable = 1;
                    break;
                }
            }
        }
        let children = h.read().children_ids.clone();
        if let Some(children) = children {
            let count = children.read().len();
            for i in 0..count {
                let id = children.read()[i];
                let child = self.for_id(id);
                let config = h.read().config_file_id;
                child.write().config_file_id = config;
            }
        }
        if h.read().an_int3855 == -1 {
            let clip = h.read().clip_type;
            h.write().an_int3855 = if clip == 0 { 0 } else { 1 };
        }
        let id = h.read().id;
        if id == 31017 {
            let mut h = h.write();
            h.size_x = 2;
            h.size_y = 2;
        }
        if id == 29292 {
            h.write().projectile_clipped = false;
        }
        Ok(())
    }
    pub fn child_at_index(&mut self, h: &Handle, index: i32) -> Handle {
        let children = h.read().children_ids.clone();
        let Some(children) = children.filter(|a| !a.read().is_empty()) else {
            return h.clone();
        };
        let children = children.read();
        let id = if index < 0
            || index as usize >= children.len() - 1
            || children[index as usize] == -1
        {
            children[children.len() - 1]
        } else {
            children[index as usize]
        };
        if id == -1 { h.clone() } else { self.for_id(id) }
    }
    pub fn child(
        &mut self,
        ctx: &mut impl PlayerContext,
        h: &Handle,
        player: Option<usize>,
    ) -> Result<Handle> {
        if h.read()
            .children_ids
            .as_ref()
            .is_none_or(|a| a.read().is_empty())
        {
            return Ok(h.clone());
        }
        let mut value = -1;
        if let Some(player) = player {
            let (file, id) = {
                let h = h.read();
                (h.config_file_id, h.config_id)
            };
            if file != -1 {
                if let Some(v) = ctx.varbit(file, player)? {
                    value = v;
                }
            } else if id != -1 {
                value = ctx.varp(id, player)?;
            }
        } else {
            value = 0;
        }
        let child = self.child_at_index(h, value);
        let config = h.read().config_file_id;
        child.write().config_file_id = config;
        Ok(child)
    }
    pub fn has_actions(&mut self, h: &Handle) -> Result<bool> {
        if h.read().interactable > 0 {
            return Ok(true);
        }
        let children = h.read().children_ids.clone();
        if let Some(children) = children {
            for i in 0..children.read().len() {
                let id = children.read()[i];
                if id != -1 && self.for_id(id).read().has_options(false)? {
                    return Ok(true);
                }
            }
        }
        h.read().has_options(false)
    }
    /// Each completed definition is published immediately; a failure keeps prior
    /// writes and any child placeholders created while configuring the next row.
    pub fn parse_files(
        &mut self,
        ctx: &mut impl LogContext,
        count: i32,
        mut file: impl FnMut(i32) -> Result<Option<Vec<u8>>>,
    ) -> Result<()> {
        for id in 0..count {
            let h = if let Some(data) = file(id)? {
                self.decode(ctx, id, &mut Reader::new(&data))?
            } else {
                Shared::new(SceneryDefinition::default())
            };
            self.definitions.insert(id, h);
        }
        Ok(())
    }
    /// Strict archive-16 loader. The caller bounds sparse allocation up front.
    pub fn load_archive(
        &mut self,
        ctx: &mut impl LogContext,
        archive: &Archive,
        maximum: usize,
    ) -> Result<usize> {
        let count = archive
            .index
            .capacity
            .checked_sub(1)
            .and_then(|last| {
                last.checked_mul(256).and_then(|n| {
                    n.checked_add(archive.index.group(last).map_or(0, |g| g.capacity))
                })
            })
            .ok_or(Error::Position)?;
        if count as usize > maximum || count > i32::MAX as u32 {
            return Err(Error::Binding(
                "scenery definition count exceeds limit".into(),
            ));
        }
        self.parse_files(ctx, count as i32, |id| {
            Ok(archive
                .file((id as u32) >> 8, (id & 255) as u32, [0; 4])?
                .map(|d| d.as_ref().clone()))
        })?;
        Ok(count as usize)
    }
    pub fn decode(
        &mut self,
        ctx: &mut impl LogContext,
        id: i32,
        r: &mut Reader<'_>,
    ) -> Result<Handle> {
        let mut d = SceneryDefinition {
            id,
            ..SceneryDefinition::default()
        };
        loop {
            if r.remaining() == 0 {
                ctx.log(self, format!("Buffer empty for {id}"))?;
                break;
            }
            let op = r.g1()?;
            match op {
                1 | 5 => {
                    let n = r.g1()? as usize;
                    if d.model_ids.is_none() {
                        let mut models = vec![0; n];
                        let mut config = if op == 1 { Some(vec![0; n]) } else { None };
                        for i in 0..n {
                            models[i] = r.g2()?;
                            if let Some(c) = &mut config {
                                c[i] = r.g1()?;
                            }
                        }
                        d.model_ids = Some(Shared::new(models));
                        d.model_configuration = config.map(Shared::new);
                    } else {
                        let n = n * if op == 1 { 3 } else { 2 };
                        let end = r.position() + n;
                        r.seek(end).map_err(|_| Error::Position)?;
                    }
                }
                2 => d.name = Some(string(r)?),
                14 => d.size_x = r.g1()?,
                15 => d.size_y = r.g1()?,
                17 => {
                    d.projectile_clipped = false;
                    d.clip_type = 0;
                }
                18 => d.projectile_clipped = false,
                19 => d.interactable = r.g1()?,
                21 => d.a_byte3912 = 1,
                22 => d.a_boolean3867 = true,
                23 => d.boolean1 = true,
                24 => d.animation_id = sentinel(r.g2()?),
                27 => d.clip_type = 1,
                28 => d.an_int3892 = r.g1()? << 2,
                29 => d.an_int3878 = r.g1b()?,
                39 => d.an_int3840 = r.g1b()? * 5,
                30..=34 => {
                    let text = string(r)?;
                    let hidden = text == "Hidden";
                    d.options.as_ref().unwrap().write()[(op - 30) as usize] =
                        if hidden { None } else { Some(text) };
                    d.has_hidden_options |= hidden;
                }
                40 | 41 => {
                    let n = r.g1()? as usize;
                    let (mut a, mut b) = (vec![0; n], vec![0; n]);
                    for i in 0..n {
                        a[i] = r.g2()? as i16;
                        b[i] = r.g2()? as i16;
                    }
                    if op == 40 {
                        d.original_colors = Some(Shared::new(a));
                        d.modified_colors = Some(Shared::new(b));
                    } else {
                        d.a_short_array3920 = Some(Shared::new(a));
                        d.a_short_array3919 = Some(Shared::new(b));
                    }
                }
                42 => {
                    let n = r.g1()?;
                    let mut a = vec![];
                    for _ in 0..n {
                        a.push(r.g1b()? as i8);
                    }
                    d.a_byte_array3858 = Some(Shared::new(a));
                }
                60 => d.map_icon = r.g2()? as i16,
                62 => d.a_boolean3839 = true,
                64 => d.a_boolean3872 = false,
                65 => d.an_int3902 = r.g2()?,
                66 => d.an_int3841 = r.g2()?,
                67 => d.an_int3917 = r.g2()?,
                68 => {
                    r.g2()?;
                }
                69 => d.walking_flag = r.g1()?,
                70 => d.an_int3883 = r.g2b()? << 2,
                71 => d.an_int3889 = r.g2b()? << 2,
                72 => d.an_int3915 = r.g2b()? << 2,
                73 => d.second_bool = true,
                74 => d.not_clipped = true,
                75 => d.an_int3855 = r.g1()?,
                77 | 92 => {
                    d.config_file_id = sentinel(r.g2()?);
                    d.config_id = sentinel(r.g2()?);
                    let default = if op == 92 { sentinel(r.g2()?) } else { -1 };
                    let n = r.g1()? as usize;
                    let mut children = vec![0; n + 2];
                    for c in children.iter_mut().take(n + 1) {
                        *c = sentinel(r.g2()?);
                    }
                    children[n + 1] = default;
                    d.children_ids = Some(Shared::new(children));
                }
                78 => {
                    d.an_int3860 = r.g2()?;
                    d.an_int3904 = r.g1()?;
                }
                79 => {
                    d.an_int3900 = r.g2()?;
                    d.an_int3905 = r.g2()?;
                    d.an_int3904 = r.g1()?;
                    let n = r.g1()?;
                    let mut a = vec![];
                    for _ in 0..n {
                        a.push(r.g2()?);
                    }
                    d.an_int_array3859 = Some(Shared::new(a));
                }
                81 => {
                    d.a_byte3912 = 2;
                    d.an_int3882 = (256 * r.g1b()?) & 255;
                }
                82 | 88 | 96 | 97 => {}
                89 => d.a_boolean3895 = false,
                90 => d.a_boolean3870 = true,
                91 => d.members_only = true,
                93 => {
                    d.a_byte3912 = 3;
                    d.an_int3882 = r.g2()?;
                }
                94 => d.a_byte3912 = 4,
                95 => d.a_byte3912 = 5,
                100 => {
                    r.g1()?;
                    r.g2()?;
                }
                101 => {
                    r.g1()?;
                }
                102 => {
                    r.g2()?;
                }
                249 => {
                    let n = r.g1()?;
                    for _ in 0..n {
                        let text = r.g1b()? == 1;
                        r.g1()?;
                        r.g1()?;
                        r.g1()?;
                        if text {
                            string(r)?;
                        } else {
                            r.g4()?;
                        }
                    }
                }
                _ => {
                    if op != 0 {
                        ctx.log(self, format!("Unhandled object definition opcode: {op}"))?;
                    }
                    break;
                }
            }
        }
        let h = Shared::new(d);
        self.configure(&h)?;
        if h.read().not_clipped {
            let mut h = h.write();
            h.clip_type = 0;
            h.projectile_clipped = false;
        }
        Ok(h)
    }
}
impl SceneryDefinition {
    pub fn has_action(&self, action: Option<&str>) -> bool {
        self.options.as_ref().is_some_and(|a| {
            a.read().iter().flatten().any(|s| {
                action.is_some_and(|a| rs09_game_core::java_string::equals_ignore_case(s, a))
            })
        })
    }
    pub fn has_options(&self, examine: bool) -> Result<bool> {
        let name = self.name.as_ref().ok_or(Error::Null)?;
        if name == "null" {
            return Ok(false);
        }
        let Some(a) = &self.options else {
            return Ok(false);
        };
        Ok(a.read()
            .iter()
            .flatten()
            .any(|s| s != "null" && (examine || s != "Examine")))
    }
    pub fn container_id(id: i32) -> u32 {
        (id as u32) >> 8
    }
}
fn sentinel(n: i32) -> i32 {
    if n == 65535 { -1 } else { n }
}
fn string(r: &mut Reader<'_>) -> Result<String> {
    let mut s = String::new();
    loop {
        let b = r.g1b()?;
        if b == 0 {
            return Ok(s);
        }
        s.push(char::from_u32(u32::from(b as u16)).unwrap());
    }
}
