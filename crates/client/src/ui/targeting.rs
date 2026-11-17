//! Original MiniMenu selected-component state and transformed action payloads.
//! Eligibility and approach are evaluated by callers before packet emission.
use super::{Component, HookArg, Properties, state::Key};
use rs09_protocol::{
    buffer::{Reader, Writer},
    game::Frame,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Targeting {
    pub active: bool,
    pub source: Key,
    pub mask: i32,
    pub param: i32,
    pub cursor: i32,
    pub default_cursor: i32,
    pub verb: String,
    pub text: String,
}
impl Default for Targeting {
    fn default() -> Self {
        Self {
            active: false,
            source: Key { id: 0, child: -1 },
            mask: 0,
            param: 0,
            cursor: 0,
            default_cursor: 0,
            verb: String::new(),
            text: String::new(),
        }
    }
}
pub fn target_verb(c: &Component, properties: Properties) -> Option<&str> {
    (properties.target_mask() != 0
        && !c
            .option_circumfix
            .trim_matches(|c: char| c <= '\u{20}')
            .is_empty())
    .then_some(c.option_circumfix.as_str())
}
impl Targeting {
    pub fn packet(&self, action: i32, key: i32, args: [i32; 2]) -> Option<Frame> {
        let mut w = Writer::new();
        let opcode = match action {
            3 => {
                w.ip4(self.source.id);
                w.ip2add(args[0]);
                w.ip4(args[1]);
                w.p2add(key);
                w.ip2(self.source.child);
                253
            }
            11 => {
                w.mp4(self.source.id);
                w.p2add(args[0]);
                w.ip2add(self.source.child);
                w.p2add(args[1]);
                131
            }
            12 => {
                w.p2(self.source.child);
                w.imp4(args[1]);
                w.p4(self.source.id);
                w.ip2add(args[0]);
                82
            }
            15 => {
                w.p2add(self.source.child);
                w.ip4(self.source.id);
                w.ip2add(key);
                195
            }
            38 => {
                w.ip2add(args[1]);
                w.p2add(args[0]);
                w.ip2add(self.source.child);
                w.imp4(self.source.id);
                w.p2add(key);
                233
            }
            39 => {
                w.imp4(self.source.id);
                w.p2(args[1]);
                w.ip2add(key);
                w.ip2add(args[0]);
                w.ip2(self.source.child);
                73
            }
            45 => {
                w.ip4(self.source.id);
                w.p2add(self.source.child);
                w.ip2add(key);
                239
            }
            _ => return None,
        };
        Some(Frame {
            opcode,
            payload: w.into_vec(),
        })
    }
}
#[derive(Clone, Debug, Default)]
pub struct ParamType {
    pub kind: i32,
    pub default_int: i32,
    pub default_string: Option<String>,
}
impl ParamType {
    pub fn decode(data: &[u8]) -> rs09_protocol::Result<Self> {
        let mut r = Reader::new(data);
        let mut p = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok(p),
                1 => p.kind = r.g1()?,
                2 => p.default_int = r.g4()?,
                5 => p.default_string = Some(super::latin1(r.gjstr()?)),
                _ => {}
            }
        }
    }
}
pub fn read_params(r: &mut Reader<'_>) -> rs09_protocol::Result<BTreeMap<i32, HookArg>> {
    let mut values = BTreeMap::new();
    for _ in 0..r.g1()? {
        let text = r.g1()? != 0;
        let id = r.g3()?;
        let value = if text {
            HookArg::Text(super::latin1(r.gjstr()?))
        } else {
            HookArg::Int(r.g4()?)
        };
        values.insert(id, value);
    }
    Ok(values)
}
pub fn int_param(
    params: Option<&BTreeMap<i32, HookArg>>,
    id: i32,
    default: i32,
) -> Result<i32, String> {
    match params.and_then(|p| p.get(&id)) {
        None => Ok(default),
        Some(HookArg::Int(n)) => Ok(*n),
        _ => Err(format!("Non-integer target parameter {id}")),
    }
}
impl super::state::State {
    pub fn target_allowed(
        &mut self,
        bit: i32,
        params: Option<&BTreeMap<i32, HookArg>>,
    ) -> Result<bool, String> {
        let id = self.targeting.param;
        let default = if id == -1 {
            None
        } else {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.target_params.entry(id)
            {
                let name = format!("param/{id}");
                let value = if self.pack.manifest.files.contains_key(&name) {
                    ParamType::decode(self.pack.get(&name)?).map_err(|e| e.to_string())?
                } else if self
                    .pack
                    .manifest
                    .files
                    .contains_key("param-catalog-complete")
                {
                    ParamType::default()
                } else {
                    return Err(format!("Missing original target parameter metadata {id}"));
                };
                entry.insert(value);
            }
            Some(self.target_params[&id].default_int)
        };
        Ok(self.targeting.mask & bit != 0
            && match default {
                None => true,
                Some(default) => int_param(params, id, default)? != default,
            })
    }
    pub fn target_entry(
        &mut self,
        menu: &mut crate::interaction::Menu,
        bit: i32,
        params: Option<&BTreeMap<i32, HookArg>>,
        mut entry: crate::interaction::MenuEntry,
    ) -> Result<(), String> {
        if self.target_allowed(bit, params)? {
            entry.option = self.targeting.verb.clone();
            entry.target = format!("{} -> {}", self.targeting.text, entry.target);
            entry.cursor = self.targeting.cursor;
            menu.add(entry);
        }
        Ok(())
    }
}
