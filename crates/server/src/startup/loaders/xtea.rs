//! `XteaParser`: `xteas.json` into `REGION_XTEA`, the map-file keys per region.
use super::{Log, Result, array, int, list_get, object, parse, read_config, text};
use crate::world::drop_tables::LogLevel;
use std::collections::HashMap;

/// `XteaParser.REGION_XTEA` (a `HashMap<Int, IntArray>`; nothing reads its
/// iteration order) and `getRegionXTEA`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegionKeys {
    keys: HashMap<i32, [i32; 4]>,
}
impl RegionKeys {
    /// `getRegionXTEA(regionId)`: four zeros for a region without keys.
    pub fn get(&self, region: i32) -> [i32; 4] {
        self.keys.get(&region).copied().unwrap_or([0; 4])
    }
    pub fn contains(&self, region: i32) -> bool {
        self.keys.contains_key(&region)
    }
    pub fn len(&self) -> usize {
        self.keys.len()
    }
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (i32, [i32; 4])> + '_ {
        self.keys.iter().map(|(k, v)| (*k, *v))
    }
}

/// `load()` with `CONFIG_PATH`.
pub fn load_config(prefix: Option<&str>, keys: &mut RegionKeys, log: &mut dyn Log) -> Result<i32> {
    let text = read_config(prefix, "xteas.json")?;
    load(&text, keys, log)
}

/// `load()` over the file's text. Rows before a failure keep their keys.
pub fn load(source: &str, keys: &mut RegionKeys, log: &mut dyn Log) -> Result<i32> {
    let mut count = 0i32;
    let data = parse(source)?;
    let obj = object(Some(&data))?;
    let configlist = array(obj.get("xteas"))?;
    for config in configlist {
        let e = object(Some(config))?;
        let id = int(&text(e, "regionId"))?;
        // `split(",").map(String::toInt)`: every part is parsed before any is used.
        let parts = text(e, "keys")
            .split(',')
            .map(int)
            .collect::<Result<Vec<i32>>>()?;
        let row = [
            *list_get(&parts, 0)?,
            *list_get(&parts, 1)?,
            *list_get(&parts, 2)?,
            *list_get(&parts, 3)?,
        ];
        keys.keys.insert(id, row);
        count = count.wrapping_add(1);
    }
    log.log(
        "XteaParser",
        LogLevel::Fine,
        &format!("Parsed {count} region keys."),
    );
    Ok(count)
}
