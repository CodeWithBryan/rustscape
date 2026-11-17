//! Original CS2 bytecode format. Execution is bounded separately from decoding.
use super::latin1;
use rs09_protocol::{Error, Result, buffer::Reader};
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Script {
    pub int_args: i32,
    pub int_locals: i32,
    pub string_args: i32,
    pub opcodes: Vec<i32>,
    pub switch_tables: Option<Vec<BTreeMap<i32, i32>>>,
    pub name: Option<String>,
    pub string_locals: i32,
    pub string_operands: Vec<Option<String>>,
    pub int_operands: Vec<i32>,
}
impl Script {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        r.seek(
            data.len()
                .checked_sub(2)
                .ok_or(Error::new(0, "truncated CS2 trailer"))?,
        )?;
        let trailer_len = r.g2()? as usize;
        let trailer = data
            .len()
            .checked_sub(trailer_len + 14)
            .ok_or(Error::new(0, "invalid CS2 trailer"))?;
        r.seek(trailer)?;
        let count = r.g4()?;
        if !(0..=1_000_000).contains(&count) {
            return Err(Error::new(trailer, "invalid CS2 instruction count"));
        }
        let int_locals = r.g2()?;
        let string_locals = r.g2()?;
        let int_args = r.g2()?;
        let string_args = r.g2()?;
        let n = r.g1()?;
        let switch_tables = if n == 0 {
            None
        } else {
            let mut tables = Vec::new();
            for _ in 0..n {
                let mut table = BTreeMap::new();
                for _ in 0..r.g2()? {
                    let key = r.g4()?;
                    let value = r.g4()?;
                    table.insert(key, value);
                }
                tables.push(table);
            }
            Some(tables)
        };
        r.seek(0)?;
        let name = if data.first() == Some(&0) {
            r.g1()?;
            None
        } else {
            Some(latin1(r.gjstr()?))
        };
        let mut opcodes = vec![0; count as usize];
        let mut int_operands = vec![0; count as usize];
        let mut string_operands = vec![None; count as usize];
        let mut i = 0;
        while r.position() < trailer {
            if i >= count as usize {
                return Err(Error::new(r.position(), "too many CS2 instructions"));
            }
            let op = r.g2()?;
            if op == 3 {
                string_operands[i] = Some(latin1(r.gjstr()?));
            } else if op >= 100 || matches!(op, 21 | 38 | 39) {
                int_operands[i] = r.g1()?;
            } else {
                int_operands[i] = r.g4()?;
            }
            opcodes[i] = op;
            i += 1;
        }
        Ok(Self {
            int_args,
            int_locals,
            string_args,
            opcodes,
            switch_tables,
            name,
            string_locals,
            string_operands,
            int_operands,
        })
    }
}
