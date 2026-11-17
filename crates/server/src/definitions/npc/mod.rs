//! Original server NPC cache decoder, lazy registry and definition queries.
//! Drop tables and concrete content handlers remain owning-world services.
use super::{
    item::Shared,
    property::{Animation, Property},
};
use rs09_cache::{CacheError, archive::Archive};
use rs09_protocol::buffer::Reader;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
pub mod config;
mod data;
pub use data::NpcDefinition;
pub type Handle = Shared<NpcDefinition>;
#[derive(Debug)]
pub enum Error {
    Decode(rs09_protocol::Error),
    Cache(CacheError),
    Null,
    Index,
    Type,
    Limit,
    Number,
    Json(rs09_game_core::java_json::Error),
    Io(std::io::Error),
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
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
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Serialize, Deserialize)]
pub struct Graphics {
    pub id: i32,
    pub height: i32,
    pub delay: i32,
}
pub trait Files {
    fn file(&mut self, registry: &mut Registry, id: i32) -> Result<Option<Vec<u8>>>;
}
pub trait PlayerContext: Files {
    fn varbit(&mut self, registry: &mut Registry, id: i32, player: usize) -> Result<Option<i32>>;
    fn varp(&mut self, registry: &mut Registry, id: i32, player: usize) -> Result<i32>;
}
#[derive(Default)]
pub struct Registry {
    pub definitions: HashMap<i32, Handle>,
}
impl Registry {
    pub fn for_id(&mut self, ctx: &mut impl Files, id: i32) -> Result<Handle> {
        if let Some(h) = self.definitions.get(&id) {
            return Ok(h.clone());
        }
        let h = Shared::new(NpcDefinition {
            id,
            ..NpcDefinition::default()
        });
        if let Some(bytes) = ctx.file(self, id)? {
            parse(&h, &mut Reader::new(&bytes))?;
        }
        self.definitions.insert(id, h.clone());
        Ok(h)
    }
    /// Prewarm the same lazy lookups; existing cached definitions retain identity.
    pub fn load_archive(&mut self, archive: &Archive, maximum: usize) -> Result<usize> {
        let count = archive
            .index
            .capacity
            .checked_sub(1)
            .and_then(|last| {
                last.checked_mul(128).and_then(|n| {
                    n.checked_add(archive.index.group(last).map_or(0, |g| g.capacity))
                })
            })
            .ok_or(Error::Limit)?;
        if count as usize > maximum || count > i32::MAX as u32 {
            return Err(Error::Limit);
        }
        struct Source<'a>(&'a Archive);
        impl Files for Source<'_> {
            fn file(&mut self, _: &mut Registry, id: i32) -> Result<Option<Vec<u8>>> {
                Ok(self
                    .0
                    .file((id as u32) >> 7, (id & 127) as u32, [0; 4])?
                    .map(|b| b.as_ref().clone()))
            }
        }
        for id in 0..count {
            self.for_id(&mut Source(archive), id as i32)?;
        }
        Ok(count as usize)
    }
    pub fn child(
        &mut self,
        ctx: &mut impl PlayerContext,
        h: &Handle,
        player: Option<usize>,
    ) -> Result<Handle> {
        if h.read()
            .child_npc_ids
            .as_ref()
            .is_none_or(|c| c.read().is_empty())
        {
            return Ok(h.clone());
        }
        let mut value = -1;
        if let Some(player) = player {
            let file = h.read().config_file_id;
            if file != -1 {
                value = ctx.varbit(self, file, player)?.ok_or(Error::Null)?;
            } else {
                let id = h.read().config_id;
                if id != -1 {
                    value = ctx.varp(self, id, player)?;
                }
            }
        } else {
            value = 0;
        }
        let children = h.read().child_npc_ids.clone().ok_or(Error::Null)?;
        let c = children.read();
        let len = c.len() as i32;
        let id = if value < 0
            || value >= len.wrapping_sub(1)
            || *c.get(value as usize).ok_or(Error::Index)? == -1
        {
            *c.get(c.len().wrapping_sub(1)).ok_or(Error::Index)?
        } else {
            c[value as usize]
        };
        drop(c);
        if id == -1 {
            Ok(h.clone())
        } else {
            self.for_id(ctx, id)
        }
    }
}
fn sentinel(v: i32) -> i32 {
    if v == 65535 { -1 } else { v }
}
fn string(r: &mut Reader<'_>) -> Result<String> {
    let mut s = String::new();
    loop {
        let v = r.g1b()?;
        if v == 0 {
            return Ok(s);
        }
        s.push(char::from_u32(u32::from(v as u16)).unwrap());
    }
}
/// Mutates the existing object, including partially populated arrays on failure.
pub fn parse(h: &Handle, r: &mut Reader<'_>) -> Result<()> {
    loop {
        let op = r.g1()?;
        match op {
            0 => return Ok(()),
            1 => {
                let n = r.g1()? as usize;
                let a = Shared::new(vec![0; n]);
                h.write().an_int_array868 = Some(a.clone());
                for i in 0..n {
                    a.write()[i] = sentinel(r.g2()?);
                }
            }
            2 => {
                let s = string(r)?;
                h.write().name = Some(s);
            }
            12 => h.write().size = r.g1()?,
            13 => h.write().stand_animation = r.g2b()?,
            14 => h.write().walk_animation = r.g2b()?,
            15 => h.write().turn_animation = r.g2b()?,
            16 => {
                r.g2()?;
            }
            17 => {
                h.write().walk_animation = r.g2b()?;
                h.write().turn180_animation = r.g2b()?;
                h.write().turn_cw_animation = r.g2b()?;
                h.write().turn_ccw_animation = r.g2b()?;
            }
            30..=34 => {
                let s = string(r)?;
                let a = h.read().options.clone().ok_or(Error::Null)?;
                *a.write().get_mut((op - 30) as usize).ok_or(Error::Index)? = Some(s);
            }
            40 | 41 => {
                let n = r.g1()? as usize;
                let a = Shared::new(vec![0; n]);
                let b = Shared::new(vec![0; n]);
                {
                    let mut d = h.write();
                    if op == 40 {
                        d.a_short_array859 = Some(b.clone());
                        d.a_short_array896 = Some(a.clone());
                    } else {
                        d.a_short_array880 = Some(a.clone());
                        d.a_short_array866 = Some(b.clone());
                    }
                }
                for i in 0..n {
                    a.write()[i] = r.g2()? as i16;
                    b.write()[i] = r.g2()? as i16;
                }
            }
            42 => {
                let n = r.g1()? as usize;
                let a = Shared::new(vec![0; n]);
                h.write().a_byte_array861 = Some(a.clone());
                for i in 0..n {
                    a.write()[i] = r.g1b()? as i8;
                }
            }
            60 => {
                let n = r.g1()? as usize;
                let a = Shared::new(vec![0; n]);
                h.write().an_int_array892 = Some(a.clone());
                for i in 0..n {
                    a.write()[i] = r.g2()?;
                }
            }
            93 => h.write().is_visible_on_map = false,
            95 => h.write().combat_level = r.g2()?,
            97 => h.write().an_int864 = r.g2()?,
            98 => h.write().an_int899 = r.g2()?,
            99 => h.write().a_boolean863 = true,
            100 => h.write().an_int869 = r.g1b()?,
            101 => h.write().an_int897 = r.g1b()? * 5,
            102 => h.write().head_icons = r.g2()?,
            103 => h.write().an_int853 = r.g2()?,
            106 | 118 => {
                h.write().config_file_id = sentinel(r.g2()?);
                h.write().config_id = sentinel(r.g2()?);
                let default = if op == 118 { sentinel(r.g2()?) } else { -1 };
                let n = r.g1()? as usize;
                let a = Shared::new(vec![0; n + 2]);
                h.write().child_npc_ids = Some(a.clone());
                for i in 0..=n {
                    a.write()[i] = sentinel(r.g2()?);
                }
                a.write()[n + 1] = default;
            }
            107 => h.write().a_boolean841 = false,
            109 => h.write().a_boolean852 = false,
            111 => h.write().a_boolean857 = false,
            113 => {
                h.write().a_short862 = r.g2()? as i16;
                h.write().a_short894 = r.g2()? as i16;
            }
            114 => {
                h.write().a_byte851 = r.g1b()? as i8;
                h.write().a_byte854 = r.g1b()? as i8;
            }
            115 => {
                r.g1()?;
                r.g1()?;
            }
            119 | 125 | 128 => {
                r.g1()?;
            }
            121 => {
                let n = r.g1()?;
                for _ in 0..n {
                    r.g1()?;
                    r.g1()?;
                    r.g1()?;
                    r.g1()?;
                }
            }
            122 | 123 | 137 => {
                r.g2()?;
            }
            126 => {
                r.g2()?;
                h.write().render_animation_id = r.g2b()?;
            }
            127 => h.write().render_animation_id = r.g2b()?,
            134 => {
                r.g2()?;
                r.g2()?;
                r.g2()?;
                r.g2()?;
                r.g1()?;
            }
            135 | 136 => {
                r.g1()?;
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
            _ => {}
        }
    }
}
impl NpcDefinition {
    pub fn has_action(&self, value: Option<&str>) -> bool {
        self.options.as_ref().is_some_and(|a| {
            a.read().iter().flatten().any(|s| {
                value.is_some_and(|v| rs09_game_core::java_string::equals_ignore_case(s, v))
            })
        })
    }
    pub fn has_attack_option(&self) -> Result<bool> {
        let a = self.options.as_ref().ok_or(Error::Null)?;
        Ok(a.read()
            .iter()
            .flatten()
            .any(|s| rs09_game_core::java_string::equals_ignore_case(s, "attack")))
    }
    pub fn has_options(&self, examine: bool) -> Result<bool> {
        let name = self.name.as_ref().ok_or(Error::Null)?;
        if name == "null" {
            return Ok(false);
        }
        Ok(self.options.as_ref().is_some_and(|a| {
            a.read()
                .iter()
                .flatten()
                .any(|s| s != "null" && (examine || s != "Examine"))
        }))
    }
    pub fn combat_animation(&self, index: i32) -> Result<Option<Shared<Animation>>> {
        let key = match index {
            0 => "melee_animation",
            1 => "magic_animation",
            2 => "range_animation",
            3 => "defence_animation",
            4 => "death_animation",
            _ => "",
        };
        match self.handlers.read().get(key) {
            None | Some(Property::Null) => Ok(None),
            Some(Property::Animation(a)) => Ok(Some(a.clone())),
            _ => Err(Error::Type),
        }
    }
}
/// Original initCombatGraphics argument-map versus definition-height lookup.
/// Only present keys replace entries; earlier writes survive a later failure.
pub fn init_combat_graphics(
    h: &Handle,
    config: &Shared<std::collections::BTreeMap<String, Property>>,
) -> Result<()> {
    for (index, key, height_key, default) in [
        (0, "start_gfx", "start_height", 0),
        (1, "projectile", "prj_height", 42),
        (2, "end_gfx", "end_height", 96),
    ] {
        let value = config.read().get(key).cloned();
        if let Some(value) = value {
            let array = h.read().combat_graphics.clone();
            let id = match value {
                Property::Int(v) => v,
                Property::Null => return Err(Error::Null),
                _ => return Err(Error::Type),
            };
            let handlers = h.read().handlers.clone();
            let height = match handlers.read().get(height_key) {
                None | Some(Property::Null) => default,
                Some(Property::Int(v)) => *v,
                _ => return Err(Error::Type),
            };
            let g = Shared::new(Graphics {
                id,
                height,
                delay: 0,
            });
            let array = array.ok_or(Error::Null)?;
            *array.write().get_mut(index).ok_or(Error::Index)? = Some(g);
        }
    }
    Ok(())
}
