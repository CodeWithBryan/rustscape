//! RT4 client-side item definitions, including inventory art and menu metadata.
pub use super::item_fields::ItemType;
use super::{HookArg, latin1};
use crate::pack::Pack;
use rs09_protocol::{Result, buffer::Reader};
use std::collections::{BTreeMap, BTreeSet};
impl ItemType {
    pub fn decode(id: i32, data: &[u8]) -> Result<(Self, usize)> {
        let mut c = Self {
            id,
            ..Self::default()
        };
        let mut r = Reader::new(data);
        loop {
            match r.g1()? {
                0 => return Ok((c, r.position())),
                1 => c.model = r.g2()?,
                2 => c.name = latin1(r.gjstr()?),
                4 => c.zoom2d = r.g2()?,
                5 => c.xAngle2D = r.g2()?,
                6 => c.yAngle2D = r.g2()?,
                7 => c.xOffset2D = r.g2b()?,
                8 => c.yOffset2D = r.g2b()?,
                11 => c.stackable = 1,
                12 => c.cost = r.g4()?,
                16 => c.members = true,
                23 => c.manwear = r.g2()?,
                24 => c.manwear2 = r.g2()?,
                25 => c.womanwear = r.g2()?,
                26 => c.womanwear2 = r.g2()?,
                op @ 30..=34 => {
                    let s = latin1(r.gjstr()?);
                    c.ops[op as usize - 30] = (!s.eq_ignore_ascii_case("hidden")).then_some(s);
                }
                op @ 35..=39 => c.iops[op as usize - 35] = Some(latin1(r.gjstr()?)),
                op @ (40 | 41) => {
                    let n = r.g1()?;
                    let (mut src, mut dst) = (Vec::new(), Vec::new());
                    for _ in 0..n {
                        src.push(r.g2b()?);
                        dst.push(r.g2b()?);
                    }
                    if op == 40 {
                        c.recol_s = Some(src);
                        c.recol_d = Some(dst);
                    } else {
                        c.retex_s = Some(src);
                        c.retex_d = Some(dst);
                    }
                }
                42 => {
                    let n = r.g1()?;
                    c.recol_p = Some((0..n).map(|_| r.g1b()).collect::<Result<_>>()?);
                }
                65 => c.stockMarket = true,
                78 => c.manwear3 = r.g2()?,
                79 => c.womanwear3 = r.g2()?,
                90 => c.manhead = r.g2()?,
                91 => c.womanhead = r.g2()?,
                92 => c.manhead2 = r.g2()?,
                93 => c.womanhead2 = r.g2()?,
                95 => c.zAngle2D = r.g2()?,
                96 => c.dummyItem = r.g1()?,
                97 => c.certlink = r.g2()?,
                98 => c.certtemplate = r.g2()?,
                op @ 100..=109 => {
                    c.countobj.get_or_insert_with(|| vec![0; 10])[op as usize - 100] = r.g2()?;
                    c.countco.get_or_insert_with(|| vec![0; 10])[op as usize - 100] = r.g2()?;
                }
                110 => c.resizeX = r.g2()?,
                111 => c.resizeY = r.g2()?,
                112 => c.resizeZ = r.g2()?,
                113 => c.ambient = r.g1b()?,
                114 => c.contrast = r.g1b()? * 5,
                115 => c.team = r.g1()?,
                121 => c.lentLink = r.g2()?,
                122 => c.lentTemplate = r.g2()?,
                125 => {
                    c.manWearXOff = r.g1b()?;
                    c.manWearYOff = r.g1b()?;
                    c.manWearZOff = r.g1b()?;
                }
                126 => {
                    c.womanWearXOff = r.g1b()?;
                    c.womanWearYOff = r.g1b()?;
                    c.womanWearZOff = r.g1b()?;
                }
                127 => {
                    c.cursor1Op = r.g1()?;
                    c.cursor1 = r.g2()?;
                }
                128 => {
                    c.cursor2Op = r.g1()?;
                    c.cursor2 = r.g2()?;
                }
                129 | 130 => {
                    r.g1()?;
                    r.g2()?;
                }
                249 => {
                    for _ in 0..r.g1()? {
                        let is_text = r.g1()? == 1;
                        let key = r.g3()?;
                        let value = if is_text {
                            HookArg::Text(latin1(r.gjstr()?))
                        } else {
                            HookArg::Int(r.g4()?)
                        };
                        c.params
                            .get_or_insert_with(BTreeMap::new)
                            .entry(key)
                            .or_insert(value);
                    }
                }
                _ => {}
            }
        }
    }
    pub fn count_variant(&self, count: i32) -> i32 {
        let mut result = self.id;
        if count > 1 {
            if let (Some(ids), Some(amounts)) = (&self.countobj, &self.countco) {
                for i in 0..10 {
                    if amounts[i] != 0 && amounts[i] <= count {
                        result = ids[i];
                    }
                }
            }
        }
        result
    }
}
#[derive(Default)]
pub struct Catalog {
    pub types: BTreeMap<i32, ItemType>,
    pub fallbacks: BTreeSet<i32>,
}
impl Catalog {
    pub fn get(&mut self, pack: &Pack, id: i32) -> std::result::Result<ItemType, String> {
        self.load(pack, id, 0)
    }
    fn load(
        &mut self,
        pack: &Pack,
        id: i32,
        depth: usize,
    ) -> std::result::Result<ItemType, String> {
        if let Some(c) = self.types.get(&id) {
            return Ok(c.clone());
        }
        if depth > 64 {
            return Err("Cyclic item templates".into());
        }
        let mut c = if let Ok(raw) = pack.get(&format!("item/{id}")) {
            ItemType::decode(id, raw).map_err(|e| e.to_string())?.0
        } else {
            // -1 is the original empty-item sentinel used by IF_SETOBJECT.
            if id != -1 {
                self.fallbacks.insert(id);
            }
            ItemType {
                id,
                ..ItemType::default()
            }
        };
        if c.certtemplate != -1 {
            let link = self.load(pack, c.certlink, depth + 1)?;
            let template = self.load(pack, c.certtemplate, depth + 1)?;
            c.certificate(&link, &template);
        }
        if c.lentTemplate != -1 {
            let link = self.load(pack, c.lentLink, depth + 1)?;
            let template = self.load(pack, c.lentTemplate, depth + 1)?;
            c.lent(&link, &template);
        }
        self.types.insert(id, c.clone());
        Ok(c)
    }
}
