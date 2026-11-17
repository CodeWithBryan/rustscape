//! The starting-scene pack built from the pinned JS5 cache: the native
//! replacement for `tools/prepare_visual.py`.
//!
//! It collects the same entries in the same order: the terrain and location
//! files of the chosen regions, every location definition they place, the
//! models, sequences, frames and bases those need, the default player's
//! identity kits, the preview NPCs with their movement sets, floors, lights,
//! textures, sprites, map scenes, the minimap sprite IDs, a few sounds and
//! `scene.json`. Nothing is decoded beyond the dependency readers the script
//! had; the runtime decoders parse the assets.
//!
//! Beyond the script: the regions' underwater map files as
//! `underwater/terrain/{x}_{z}` and `underwater/locations/{x}_{z}`, the raw
//! definitions and models of the locations those place (which are not part of
//! `scene.json`), and an empty `underwater-complete` entry saying the pack
//! lists every underwater file its regions have.
//!
//! The script took three files from the pinned server checkout. Here they are
//! optional inputs: region keys (without a region's key its locations cannot
//! be read), door replacement pairs (extra definitions only) and NPC spawns
//! (the offline preview's NPCs).
use crate::cache_pack::{Builder, Source};
use rs09_cache::archive::Archive;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// The server-side files the original script read. All may be empty.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    /// Region ID (`x << 8 | z`) to its four XTEA key words.
    pub keys: BTreeMap<i32, [u32; 4]>,
    /// `door_configs.json`: each `(id, replaceId)` pair.
    pub doors: Vec<(i32, i32)>,
    /// `npc_spawns.json`: `(npc, x, z, plane, walk radius, direction ordinal)`.
    pub spawns: Vec<[i32; 6]>,
}

impl Inputs {
    /// `xteas.json` in any of the three shapes the script accepted: an object
    /// with an `xteas` list of `{regionId, keys: "a,b,c,d"}`, a list of
    /// `{region, keys | key}`, or an object from region ID to four words.
    pub fn parse_keys(json: &[u8]) -> Result<BTreeMap<i32, [u32; 4]>, String> {
        use serde_json::Value;
        let value: Value = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        let words = |value: &Value| -> Result<[u32; 4], String> {
            let list: Vec<i64> = match value {
                Value::String(text) => text
                    .split(',')
                    .map(|word| word.trim().parse::<i64>().map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?,
                Value::Array(items) => items
                    .iter()
                    .map(|item| item.as_i64().ok_or("map key word"))
                    .collect::<Result<_, _>>()?,
                _ => return Err("map key list".into()),
            };
            <[i64; 4]>::try_from(list)
                .map(|key| key.map(|word| word as u32))
                .map_err(|_| "a map key has four words".into())
        };
        let region = |value: &Value| -> Result<i32, String> {
            match value {
                Value::String(text) => text.parse().map_err(|_| "region id".to_owned()),
                other => other
                    .as_i64()
                    .map(|id| id as i32)
                    .ok_or_else(|| "region id".to_owned()),
            }
        };
        let mut keys = BTreeMap::new();
        match &value {
            Value::Object(object) if object.contains_key("xteas") => {
                for row in object["xteas"].as_array().ok_or("xteas list")? {
                    keys.insert(region(&row["regionId"])?, words(&row["keys"])?);
                }
            }
            Value::Object(object) => {
                for (id, key) in object {
                    keys.insert(id.parse().map_err(|_| "region id")?, words(key)?);
                }
            }
            Value::Array(rows) => {
                for row in rows {
                    let key = if row["keys"].is_null() {
                        &row["key"]
                    } else {
                        &row["keys"]
                    };
                    keys.insert(region(&row["region"])?, words(key)?);
                }
            }
            _ => return Err("map keys must be an object or a list".into()),
        }
        Ok(keys)
    }

    /// `door_configs.json`.
    pub fn parse_doors(json: &[u8]) -> Result<Vec<(i32, i32)>, String> {
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(json).map_err(|e| e.to_string())?;
        rows.iter()
            .map(|row| Ok((integer(&row["id"])?, integer(&row["replaceId"])?)))
            .collect()
    }

    /// `npc_spawns.json`: rows of an NPC ID and `{x,z,plane,walk,direction}-`
    /// groups.
    pub fn parse_spawns(json: &[u8]) -> Result<Vec<[i32; 6]>, String> {
        let rows: Vec<serde_json::Value> =
            serde_json::from_slice(json).map_err(|e| e.to_string())?;
        let mut spawns = vec![];
        for row in &rows {
            let npc = integer(&row["npc_id"])?;
            let places = row["loc_data"].as_str().ok_or("spawn locations")?;
            for place in places.split('-').filter(|place| !place.is_empty()) {
                let fields: Vec<i32> = place
                    .trim_matches(|c| c == '{' || c == '}')
                    .split(',')
                    .map(|field| field.parse::<i32>().map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
                let [x, z, plane, walk, direction] =
                    <[i32; 5]>::try_from(fields).map_err(|_| "a spawn has five fields")?;
                spawns.push([npc, x, z, plane, walk, direction]);
            }
        }
        Ok(spawns)
    }
}

fn integer(value: &serde_json::Value) -> Result<i32, String> {
    match value {
        serde_json::Value::String(text) => text.trim().parse().map_err(|_| "integer".to_owned()),
        other => other
            .as_i64()
            .map(|n| n as i32)
            .ok_or_else(|| "integer".to_owned()),
    }
}

/// Which regions to pack and what to call the scene.
#[derive(Clone, Debug)]
pub struct Options {
    /// The starting tile.
    pub center: [i32; 2],
    pub label: String,
    /// Pack the regions of the original normal rebuild around the centre
    /// instead of the two-by-two block.
    pub login_window: bool,
    /// Inclusive `[west, south, east, north]` region bounds.
    pub region_bounds: Option<[i32; 4]>,
    /// Extra NPC types whose bodies are packed.
    pub npc_types: Vec<u32>,
    /// Also pack the bodies of every NPC spawned in the regions.
    pub region_npcs: bool,
    /// Skip a region's locations when its file is missing or its key is
    /// wrong, noting it, instead of failing as the script did.
    pub lenient: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            center: [3222, 3218],
            label: "Lumbridge".into(),
            login_window: false,
            region_bounds: None,
            npc_types: vec![],
            region_npcs: false,
            lenient: false,
        }
    }
}

pub struct WorldPack {
    /// The `RS09VIS1` pack.
    pub bytes: Vec<u8>,
    /// What a lenient build left out.
    pub notes: Vec<String>,
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(count)
            .filter(|&end| end <= self.data.len());
        let end = end.ok_or("truncated asset")?;
        let bytes = &self.data[self.at..end];
        self.at = end;
        Ok(bytes)
    }
    fn u1(&mut self) -> Result<i32, String> {
        Ok(i32::from(self.take(1)?[0]))
    }
    fn u2(&mut self) -> Result<i32, String> {
        let bytes = self.take(2)?;
        Ok(i32::from(u16::from_be_bytes([bytes[0], bytes[1]])))
    }
    fn smart(&mut self) -> Result<i32, String> {
        if *self.data.get(self.at).ok_or("truncated asset")? < 128 {
            self.u1()
        } else {
            Ok(self.u2()? - 32768)
        }
    }
    /// A zero-terminated Windows-1252 string.
    fn text(&mut self) -> Result<String, String> {
        let length = self.data[self.at.min(self.data.len())..]
            .iter()
            .position(|&byte| byte == 0)
            .ok_or("unterminated text")?;
        let bytes = self.take(length)?;
        self.take(1)?;
        bytes.iter().map(|&byte| cp1252(byte)).collect()
    }
    /// The parameter table of opcode 249.
    fn params(&mut self) -> Result<(), String> {
        for _ in 0..self.u1()? {
            let text = self.u1()?;
            self.take(3)?;
            if text != 0 {
                self.text()?;
            } else {
                self.take(4)?;
            }
        }
        Ok(())
    }
}

fn cp1252(byte: u8) -> Result<char, String> {
    const HIGH: [u16; 32] = [
        0x20ac, 0, 0x201a, 0x192, 0x201e, 0x2026, 0x2020, 0x2021, 0x2c6, 0x2030, 0x160, 0x2039,
        0x152, 0, 0x17d, 0, 0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x2dc,
        0x2122, 0x161, 0x203a, 0x153, 0, 0x17e, 0x178,
    ];
    let code = match byte {
        0x80..=0x9f => HIGH[usize::from(byte - 0x80)],
        other => u16::from(other),
    };
    if code == 0 {
        return Err("byte without a Windows-1252 character".into());
    }
    char::from_u32(u32::from(code)).ok_or_else(|| "text".to_owned())
}

/// The script's `loc_def`: the fields the scene needs, as it wrote them into
/// `scene.json`, and the morph children when the definition has any.
fn loc_def(data: &[u8]) -> Result<(serde_json::Value, Vec<i32>), String> {
    use serde_json::json;
    let mut r = Reader { data, at: 0 };
    let (mut models, mut shapes) = (Vec::<i32>::new(), None::<Vec<i32>>);
    let mut name = String::new();
    let (mut width, mut length) = (1, 1);
    let (mut resize, mut offset) = ([128; 3], [0; 3]);
    let (mut mirror, mut hill) = (false, false);
    let (mut recolor, mut retexture) = (Vec::<[i32; 2]>::new(), Vec::<[i32; 2]>::new());
    let (mut ambient, mut contrast) = (0, 0);
    let mut animation = -1;
    let (mut randomized_animation, mut cast_shadow) = (true, true);
    let (mut blockwalk, mut blockrange, mut breakroute) = (2, true, false);
    let mut children = None::<Vec<i32>>;
    // What the script did not read: the fields placement goes by.
    let mut placement = crate::loc_placement::Rules::default();
    loop {
        let op = r.u1()?;
        match op {
            0 => break,
            1 => {
                let mut entries = vec![];
                for _ in 0..r.u1()? {
                    entries.push((r.u2()?, r.u1()?));
                }
                if models.is_empty() {
                    models = entries.iter().map(|entry| entry.0).collect();
                    shapes = Some(entries.iter().map(|entry| entry.1).collect());
                }
            }
            5 => {
                let mut listed = vec![];
                for _ in 0..r.u1()? {
                    listed.push(r.u2()?);
                }
                if models.is_empty() {
                    models = listed;
                }
            }
            2 => name = r.text()?,
            14 => width = r.u1()?,
            15 => length = r.u1()?,
            65..=67 => resize[(op - 65) as usize] = r.u2()?,
            70..=72 => offset[(op - 70) as usize] = i32::from(r.u2()? as u16 as i16),
            62 => mirror = true,
            21 => {
                hill = true;
                placement.opcode(21, 0);
            }
            24 => animation = r.u2()?,
            89 => randomized_animation = false,
            88 => cast_shadow = false,
            17 => (blockwalk, blockrange) = (0, false),
            18 => blockrange = false,
            27 => blockwalk = 1,
            74 => breakroute = true,
            30..=34 => {
                r.text()?;
            }
            40 | 41 => {
                let mut pairs = vec![];
                for _ in 0..r.u1()? {
                    pairs.push([r.u2()?, r.u2()?]);
                }
                if op == 40 {
                    recolor = pairs;
                } else {
                    retexture = pairs;
                }
            }
            42 => {
                let count = r.u1()? as usize;
                r.take(count)?;
            }
            77 | 92 => {
                r.take(4)?;
                let fallback = if op == 92 { r.u2()? } else { 65535 };
                let mut list = vec![];
                for _ in 0..r.u1()? + 1 {
                    list.push(r.u2()?);
                }
                list.push(fallback);
                children = Some(list);
            }
            78 => {
                r.take(3)?;
            }
            79 => {
                r.take(5)?;
                let count = r.u1()? as usize;
                r.take(count * 2)?;
            }
            99 | 100 => {
                r.take(3)?;
            }
            249 => r.params()?,
            29 => ambient = i32::from(r.u1()? as u8 as i8),
            39 => contrast = i32::from(r.u1()? as u8 as i8),
            28 | 81 => {
                let value = r.u1()?;
                placement.opcode(op, value);
            }
            93 => {
                let value = r.u2()?;
                placement.opcode(93, value);
            }
            73 | 82 | 90 | 94 | 95 | 98 => {
                placement.opcode(op, 0);
            }
            19 | 69 | 75 | 101 => {
                r.take(1)?;
            }
            60 | 102 => {
                r.take(2)?;
            }
            22 | 23 | 64 | 91 | 96 | 97 => {}
            other => return Err(format!("unknown location opcode {other}")),
        }
    }
    if breakroute {
        (blockwalk, blockrange) = (0, false);
    }
    let mut definition = json!({
        "models": models, "shapes": shapes, "name": name, "width": width, "length": length,
        "resize": resize, "offset": offset, "mirror": mirror, "recolor": recolor,
        "retexture": retexture, "ambient": ambient, "contrast": contrast, "hill": hill,
        "animation": animation, "randomized_animation": randomized_animation,
        "cast_shadow": cast_shadow, "blockwalk": blockwalk, "blockrange": blockrange,
        "breakroute": breakroute,
    });
    if let Some(children) = &children {
        definition["children"] = json!(children);
    }
    placement.block_walk = blockwalk;
    definition["placement"] = serde_json::to_value(placement).map_err(|e| e.to_string())?;
    Ok((definition, children.unwrap_or_default()))
}

/// The script's `npc_dependencies`: body models, movement set and morph
/// children of an NPC definition. As the pinned decoder does, it treats an
/// unknown opcode as having no operands.
fn npc_dependencies(data: &[u8]) -> Result<(Vec<i32>, i32, BTreeSet<i32>), String> {
    let mut r = Reader { data, at: 0 };
    let (mut models, mut bas, mut children) = (vec![], -1, BTreeSet::new());
    loop {
        let op = r.u1()?;
        match op {
            0 => break,
            1 => {
                for _ in 0..r.u1()? {
                    let model = r.u2()?;
                    if model != 65535 {
                        models.push(model);
                    }
                }
            }
            127 => bas = r.u2()?,
            2 | 30..=34 => {
                r.text()?;
            }
            12 | 100 | 101 | 119 | 125 | 128 => {
                r.take(1)?;
            }
            40 | 41 => {
                let count = r.u1()? as usize;
                r.take(count * 4)?;
            }
            42 => {
                let count = r.u1()? as usize;
                r.take(count)?;
            }
            60 => {
                let count = r.u1()? as usize;
                r.take(count * 2)?;
            }
            95 | 97 | 98 | 102 | 103 | 114 | 115 | 122 | 123 | 126 | 137 => {
                r.take(2)?;
            }
            113 => {
                r.take(4)?;
            }
            106 | 118 => {
                r.take(4)?;
                if op == 118 {
                    children.insert(r.u2()?);
                }
                for _ in 0..r.u1()? + 1 {
                    children.insert(r.u2()?);
                }
            }
            121 => {
                let count = r.u1()? as usize;
                r.take(count * 4)?;
            }
            134 => {
                r.take(9)?;
            }
            135 | 136 => {
                r.take(3)?;
            }
            249 => r.params()?,
            _ => {}
        }
    }
    children.remove(&65535);
    Ok((models, bas, children))
}

/// Sequences a movement set (BAS) names.
fn bas_sequences(data: &[u8], sequences: &mut BTreeSet<i32>) -> Result<(), String> {
    let mut r = Reader { data, at: 0 };
    loop {
        let op = r.u1()?;
        match op {
            0 => return Ok(()),
            1 => {
                for _ in 0..2 {
                    let sequence = r.u2()?;
                    if sequence != 65535 {
                        sequences.insert(sequence);
                    }
                }
            }
            2..=9 | 38..=42 => {
                sequences.insert(r.u2()?);
            }
            26 => {
                r.take(2)?;
            }
            27 => {
                r.take(13)?;
            }
            29 | 31 | 34 | 37 => {
                r.take(1)?;
            }
            30 | 32 | 33 | 35 | 36 | 43 | 44 | 45 => {
                r.take(2)?;
            }
            _ => {}
        }
    }
}

/// The frame groups of a sequence: its frame list is opcode 1.
fn sequence_frame_groups(id: i32, data: &[u8], groups: &mut BTreeSet<i32>) -> Result<(), String> {
    let mut r = Reader { data, at: 0 };
    let mut op = r.u1()?;
    while op != 1 && op != 0 {
        match op {
            2 | 6 | 7 => {
                r.take(2)?;
            }
            5 | 8..=11 => {
                r.take(1)?;
            }
            4 | 14..=16 => {}
            3 => {
                let count = r.u1()? as usize;
                r.take(count)?;
            }
            other => {
                return Err(format!(
                    "sequence {id} unexpected opcode {other} before frames"
                ));
            }
        }
        op = r.u1()?;
    }
    if op == 1 {
        let count = r.u2()? as usize;
        r.take(count * 4)?;
        for _ in 0..count {
            groups.insert(r.u2()?);
        }
    }
    Ok(())
}

fn group_files(archive: &Archive, label: u8, id: u32) -> Result<Vec<(u32, Vec<u8>)>, String> {
    let files = archive
        .group(id, [0; 4])
        .map_err(|e| format!("archive {label} group {id}: {e}"))?
        .ok_or_else(|| format!("archive {label} group {id} is missing"))?;
    Ok(files.iter().map(|(id, data)| (id, data.to_vec())).collect())
}

fn file(archive: &Archive, label: u8, group: u32, file: u32) -> Result<Option<Vec<u8>>, String> {
    if archive.index.group(group).is_none() {
        return Ok(None);
    }
    archive
        .file(group, file, [0; 4])
        .map(|bytes| bytes.map(|bytes| bytes.to_vec()))
        .map_err(|e| format!("archive {label} group {group} file {file}: {e}"))
}

fn required(
    archive: &Archive,
    label: u8,
    group: i32,
    id: i32,
    what: &str,
) -> Result<Vec<u8>, String> {
    file(archive, label, group as u32, id as u32)?.ok_or_else(|| format!("missing {what}"))
}

/// Build the starting-scene pack.
pub fn world_pack(
    cache: &Path,
    inputs: &Inputs,
    options: &Options,
    server_commit: &str,
) -> Result<WorldPack, String> {
    world_pack_from(&Source::open(cache)?, inputs, options, server_commit)
}
/// [`world_pack`] over an opened store (see `cache_pack::Source::from_store`).
pub fn world_pack_from(
    source: &Source,
    inputs: &Inputs,
    options: &Options,
    server_commit: &str,
) -> Result<WorldPack, String> {
    use serde_json::json;
    let (maps, locations, configs) = (source.archive(5)?, source.archive(16)?, source.archive(2)?);
    let mut pack = Builder::default();
    let mut notes = vec![];
    let center = options.center;

    let mut regions: Vec<[i32; 2]> = vec![];
    if let Some([west, south, east, north]) = options.region_bounds {
        for x in west..=east {
            for z in south..=north {
                regions.push([x, z]);
            }
        }
    } else if options.login_window {
        let zone = center.map(|n| n >> 3);
        for x in (zone[0] - 6).div_euclid(8)..=(zone[0] + 6).div_euclid(8) {
            for z in (zone[1] - 6).div_euclid(8)..=(zone[1] + 6).div_euclid(8) {
                regions.push([x, z]);
            }
        }
    } else {
        let first = center.map(|n| (n - 32).div_euclid(64));
        for x in first[0]..first[0] + 2 {
            for z in first[1]..first[1] + 2 {
                regions.push([x, z]);
            }
        }
    }

    // Definitions in the order the scene first places them.
    let mut placed = vec![];
    let mut order = vec![];
    let mut definitions = BTreeMap::<i32, (serde_json::Value, Vec<i32>)>::new();
    let mut raw_definitions = BTreeMap::<i32, Vec<u8>>::new();
    let mut models = BTreeSet::<i32>::new();
    let mut sequences: BTreeSet<i32> = [808, 819, 820, 821, 822, 823, 824, 863, 866, 859]
        .into_iter()
        .collect();
    let mut define = |id: i32,
                      pack: &mut Builder,
                      definitions: &mut BTreeMap<i32, (serde_json::Value, Vec<i32>)>,
                      raw_definitions: &mut BTreeMap<i32, Vec<u8>>,
                      order: &mut Vec<i32>|
     -> Result<(), String> {
        let raw = required(&locations, 16, id >> 8, id & 255, "location definition")
            .map_err(|e| format!("loc/{id}: {e}"))?;
        pack.add(format!("loc/{id}"), &raw);
        let definition = loc_def(&raw).map_err(|e| format!("loc/{id}: {e}"))?;
        for model in definition.0["models"].as_array().into_iter().flatten() {
            models.insert(model.as_i64().unwrap_or(0) as i32);
        }
        let animation = definition.0["animation"].as_i64().unwrap_or(-1) as i32;
        if animation != -1 && animation != 65535 {
            sequences.insert(animation);
        }
        definitions.insert(id, definition);
        raw_definitions.insert(id, raw);
        order.push(id);
        Ok(())
    };
    let mut underwater_ids = BTreeSet::<i32>::new();
    for &[x, z] in &regions {
        let terrain = maps
            .index
            .group_by_ascii_name(&format!("m{x}_{z}"))
            .ok_or_else(|| format!("no terrain for region {x},{z}"))?;
        let terrain = required(&maps, 5, terrain as i32, 0, "terrain")?;
        pack.add(format!("terrain/{x}_{z}"), &terrain);
        // The underwater map and its locations (neither is encrypted),
        // where the region has them.
        for (prefix, name) in [("um", "terrain"), ("ul", "locations")] {
            let Some(group) = maps.index.group_by_ascii_name(&format!("{prefix}{x}_{z}")) else {
                continue;
            };
            let Some(data) = file(&maps, 5, group, 0)? else {
                continue;
            };
            pack.add(format!("underwater/{name}/{x}_{z}"), &data);
            if prefix == "ul" {
                let layer = crate::underwater::Layer::decode([x, z], None, Some(&data))
                    .map_err(|e| format!("ul{x}_{z}: {e}"))?;
                underwater_ids.extend(layer.locations.iter().map(|row| row[0]));
            }
        }
        let key = inputs.keys.get(&((x << 8) | z)).copied().unwrap_or([0; 4]);
        let raw = maps
            .index
            .group_by_ascii_name(&format!("l{x}_{z}"))
            .ok_or_else(|| format!("no locations for region {x},{z}"))
            .and_then(|group| {
                maps.file(group, 0, key)
                    .map_err(|e| format!("locations of region {x},{z}: {e}"))?
                    .map(|bytes| bytes.to_vec())
                    .ok_or_else(|| format!("no locations for region {x},{z}"))
            });
        let raw = match raw {
            Ok(raw) => raw,
            Err(error) if options.lenient => {
                notes.push(error);
                // An empty location file: the terminating zero only.
                vec![0]
            }
            Err(error) => return Err(error),
        };
        pack.add(format!("locations/{x}_{z}"), &raw);
        let mut r = Reader { data: &raw, at: 0 };
        let mut id = -1;
        loop {
            let mut delta = r.smart()?;
            if delta == 0 {
                break;
            }
            while delta == 32767 {
                id += delta;
                delta = r.smart()?;
            }
            id += delta;
            let mut position = 0;
            loop {
                let delta = r.smart()?;
                if delta == 0 {
                    break;
                }
                position += delta - 1;
                let info = r.u1()?;
                let (plane, lx, lz) = (position >> 12, position >> 6 & 63, position & 63);
                placed.push([id, x * 64 + lx, z * 64 + lz, plane, info >> 2, info & 3]);
                if !definitions.contains_key(&id) {
                    define(
                        id,
                        &mut pack,
                        &mut definitions,
                        &mut raw_definitions,
                        &mut order,
                    )?;
                }
            }
        }
    }

    // Door replacements are dependencies only: the server decides when a door
    // changes, and the client then needs the other definition.
    let mut replacements = BTreeMap::<i32, BTreeSet<i32>>::new();
    for &(a, b) in &inputs.doors {
        replacements.entry(a).or_default().insert(b);
        replacements.entry(b).or_default().insert(a);
    }
    let mut pending = order.clone();
    while let Some(id) = pending.pop() {
        for &other in replacements.get(&id).into_iter().flatten() {
            if definitions.contains_key(&other) {
                continue;
            }
            define(
                other,
                &mut pack,
                &mut definitions,
                &mut raw_definitions,
                &mut order,
            )?;
            pending.push(other);
        }
    }
    // Morph children keep their raw definitions for the minimap's icons,
    // without their models.
    let mut seen: BTreeSet<i32> = definitions.keys().copied().collect();
    let mut pending = order.clone();
    while let Some(id) = pending.pop() {
        let children = match definitions.get(&id) {
            Some(definition) => definition.1.clone(),
            None => loc_def(&raw_definitions[&id])?.1,
        };
        for child in children {
            if child == 65535 || !seen.insert(child) {
                continue;
            }
            let raw = required(
                &locations,
                16,
                child >> 8,
                child & 255,
                "location definition",
            )
            .map_err(|e| format!("loc/{child}: {e}"))?;
            pack.add(format!("loc/{child}"), &raw);
            raw_definitions.insert(child, raw);
            pending.push(child);
        }
    }

    // Underwater locations are drawn, never placed in `scene.json`: the
    // pack keeps their raw definitions and their models. The entry after
    // them says the pack lists every underwater file of its regions.
    for &id in &underwater_ids {
        let raw = match raw_definitions.get(&id) {
            Some(raw) => raw.clone(),
            None => {
                let Some(raw) = file(&locations, 16, (id >> 8) as u32, (id & 255) as u32)? else {
                    continue;
                };
                pack.add(format!("loc/{id}"), &raw);
                raw
            }
        };
        let definition = loc_def(&raw).map_err(|e| format!("loc/{id}: {e}"))?;
        for model in definition.0["models"].as_array().into_iter().flatten() {
            models.insert(model.as_i64().unwrap_or(0) as i32);
        }
    }
    pack.add("underwater-complete".into(), &[]);

    // The default player: identity kits in equipment-slot order.
    let avatar = [18, 26, 36, 0, 33, 42, 10];
    let mut avatar_models = vec![];
    for kit in avatar {
        let raw = required(&configs, 2, 3, kit, "identity kit")?;
        pack.add(format!("kit/{kit}"), &raw);
        let mut r = Reader { data: &raw, at: 0 };
        loop {
            let op = r.u1()?;
            match op {
                0 => break,
                1 => {
                    r.take(1)?;
                }
                2 => {
                    for _ in 0..r.u1()? {
                        avatar_models.push(r.u2()?);
                    }
                }
                3 => {}
                40 | 41 => {
                    let count = r.u1()? as usize;
                    r.take(count * 4)?;
                }
                60..=69 => {
                    r.take(2)?;
                }
                other => return Err(format!("unknown identity-kit opcode {other}")),
            }
        }
    }
    models.extend(avatar_models.iter().copied());

    // NPCs: every raw definition, and the bodies of the preview's types.
    let npcs = source.archive(18)?;
    let roots: BTreeSet<i32> = [0, 1, 4, 9, 41, 47, 81, 100]
        .into_iter()
        .chain(options.npc_types.iter().map(|&id| id as i32))
        .collect();
    // `Direction.values()` ordinals of the pinned server as RT4 yaw.
    let directions = [768, 1024, 1280, 512, 1536, 256, 0, 1792];
    let mut spawns = vec![];
    let mut region_types = BTreeSet::new();
    for &[npc, x, z, plane, walk, direction] in &inputs.spawns {
        if regions.contains(&[x.div_euclid(64), z.div_euclid(64)]) {
            region_types.insert(npc);
            if plane == 0 && roots.contains(&npc) {
                let yaw = *directions
                    .get(direction as usize)
                    .ok_or("spawn direction out of range")?;
                spawns.push([npc, x, z, walk, yaw]);
            }
        }
    }
    for group in &npcs.index.groups {
        for (id, data) in group_files(&npcs, 18, group.id)? {
            pack.add(format!("npc/{}", group.id * 128 + id), &data);
        }
    }
    pack.add("npc-catalog-complete".into(), &[]);
    let mut npc_ids = BTreeSet::new();
    let mut pending: Vec<i32> = roots.iter().rev().copied().collect();
    let mut live_roots: Vec<i32> = if options.region_npcs {
        region_types.iter().rev().copied().collect()
    } else {
        vec![]
    };
    let mut preview_ids = None;
    while !pending.is_empty() || !live_roots.is_empty() {
        if pending.is_empty() {
            preview_ids = Some(npc_ids.iter().copied().collect::<Vec<_>>());
            pending = std::mem::take(&mut live_roots);
        }
        let npc = pending.pop().expect("checked above");
        if !npc_ids.insert(npc) {
            continue;
        }
        let raw = required(&npcs, 18, npc >> 7, npc & 127, "NPC definition")
            .map_err(|e| format!("npc/{npc}: {e}"))?;
        pack.add(format!("npc/{npc}"), &raw);
        let (body, bas, children) = npc_dependencies(&raw)?;
        models.extend(body);
        let mut new: Vec<i32> = children.difference(&npc_ids).copied().collect();
        new.reverse();
        pending.extend(new);
        if bas == -1 {
            continue;
        }
        let raw = required(&configs, 2, 32, bas, "movement set")
            .map_err(|e| format!("bas/{bas}: {e}"))?;
        pack.add(format!("bas/{bas}"), &raw);
        bas_sequences(&raw, &mut sequences)?;
    }
    let npc_ids: Vec<i32> = npc_ids.into_iter().collect();

    let model_archive = source.archive(7)?;
    let mut missing = vec![];
    for &model in &models {
        match file(&model_archive, 7, model as u32, 0)? {
            Some(data) => pack.add(format!("model/{model}"), &data),
            None => missing.push(model),
        }
    }
    for (group, name) in [(1, "underlay"), (4, "overlay")] {
        for (id, data) in group_files(&configs, 2, group)? {
            pack.add(format!("{name}/{id}"), &data);
        }
    }
    if configs.index.group(31).is_some() {
        for (id, data) in group_files(&configs, 2, 31)? {
            pack.add(format!("light/{id}"), &data);
        }
    }
    pack.add(
        "bas/1426".into(),
        &required(&configs, 2, 32, 1426, "player movement set")?,
    );
    let sequence_archive = source.archive(20)?;
    let mut frame_groups = BTreeSet::new();
    for &sequence in &sequences {
        // A movement set may leave a slot empty; the script would have failed.
        if sequence == 65535 {
            continue;
        }
        let raw = required(
            &sequence_archive,
            20,
            sequence >> 7,
            sequence & 127,
            "sequence",
        )
        .map_err(|e| format!("seq/{sequence}: {e}"))?;
        pack.add(format!("seq/{sequence}"), &raw);
        sequence_frame_groups(sequence, &raw, &mut frame_groups)?;
    }
    let frame_archive = source.archive(0)?;
    let mut bases = BTreeSet::new();
    for &group in &frame_groups {
        for (id, data) in group_files(&frame_archive, 0, group as u32)? {
            pack.add(format!("frame/{group}/{id}"), &data);
            let base = data.get(..2).ok_or("truncated animation frame")?;
            bases.insert(u16::from_be_bytes([base[0], base[1]]));
        }
    }
    let base_archive = source.archive(1)?;
    for base in bases {
        pack.add(
            format!("base/{base}"),
            &required(&base_archive, 1, i32::from(base), 0, "animation base")?,
        );
    }
    pack.add(
        "materials".into(),
        &required(&source.archive(26)?, 26, 0, 0, "materials")?,
    );
    for (archive, name) in [(9u8, "texture"), (8, "sprite")] {
        let opened = source.archive(archive)?;
        for group in &opened.index.groups {
            let data = required(&opened, archive, group.id as i32, 0, name)?;
            pack.add(format!("{name}/{}", group.id), &data);
        }
    }
    let mut scenes = group_files(&configs, 2, 34)?;
    scenes.sort_by_key(|(id, _)| *id);
    for (id, data) in scenes {
        pack.add(format!("mapscene/{id}"), &data);
    }
    let sprites = source.archive(8)?;
    let mut ids = String::from("{");
    for (i, name) in [
        "mapfunction",
        "mapdots",
        "mapflag",
        "compass",
        "hint_mapmarkers",
        "hint_mapedge",
        // Not a minimap sprite: the floor-shadow masks, found by name here
        // because this entry is where the pack keeps named sprite IDs.
        "floorshadows",
    ]
    .into_iter()
    .enumerate()
    {
        let group = sprites
            .index
            .group_by_ascii_name(name)
            .ok_or_else(|| format!("no sprite named {name}"))?;
        ids.push_str(&format!(
            "{}\"{name}\":{group}",
            if i == 0 { "" } else { "," }
        ));
    }
    ids.push('}');
    pack.add("minimap/assets".into(), ids.as_bytes());
    // The world map details and every map's labels, for the minimap's text.
    let world_maps = source.archive(23)?;
    for (name, data) in
        crate::minimap::Labels::pack_entries(crate::minimap::labels::cache_group(&world_maps))?
    {
        pack.add(name, &data);
    }
    let sounds = source.archive(4)?;
    for sound in [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 127, 223, 224, 380, 385, 386, 387,
    ] {
        if let Some(data) = file(&sounds, 4, sound, 0)? {
            pack.add(format!("sound/{sound}"), &data);
        }
    }

    let scene = json!({
        "regions": regions,
        "locations": placed,
        "definitions": definitions
            .iter()
            .map(|(id, definition)| (id.to_string(), definition.0.clone()))
            .collect::<serde_json::Map<_, _>>(),
        "avatar_models": avatar_models,
        "avatar_kits": avatar,
        "missing_models": missing,
        "npc_types": preview_ids.filter(|ids| !ids.is_empty()).unwrap_or(npc_ids),
        "npc_spawns": spawns,
        "name": options.label,
        "start": center,
    });
    pack.add(
        "scene.json".into(),
        &serde_json::to_vec(&scene).map_err(|e| e.to_string())?,
    );
    Ok(WorldPack {
        bytes: pack.finish(&format!("Pinned 2009scape cache / {server_commit}"))?,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_read_in_all_three_shapes() {
        let expected = BTreeMap::from([(12850, [1, 2, 0xffff_ffff, 4])]);
        for json in [
            r#"{"xteas":[{"regionId":"12850","keys":"1,2,-1,4"}]}"#,
            r#"[{"region":12850,"keys":[1,2,-1,4]}]"#,
            r#"[{"region":12850,"key":[1,2,4294967295,4]}]"#,
            r#"{"12850":[1,2,-1,4]}"#,
        ] {
            assert_eq!(
                Inputs::parse_keys(json.as_bytes()).unwrap(),
                expected,
                "{json}"
            );
        }
        assert!(Inputs::parse_keys(br#"{"1":[1,2,3]}"#).is_err());
        assert!(Inputs::parse_keys(b"7").is_err());
    }

    #[test]
    fn doors_and_spawns_are_read_as_the_script_read_them() {
        let doors =
            Inputs::parse_doors(br#"[{"id":"1530","replaceId":"1531"},{"id":3,"replaceId":4}]"#);
        assert_eq!(doors.unwrap(), [(1530, 1531), (3, 4)]);
        let spawns = Inputs::parse_spawns(
            br#"[{"npc_id":"1","loc_data":"{3222,3218,0,5,3}-{3200,3200,1,0,6}-"}]"#,
        );
        assert_eq!(
            spawns.unwrap(),
            [[1, 3222, 3218, 0, 5, 3], [1, 3200, 3200, 1, 0, 6]]
        );
        assert!(Inputs::parse_spawns(br#"[{"npc_id":"1","loc_data":"{1,2,3}-"}]"#).is_err());
    }

    #[test]
    fn location_definitions_keep_the_scripts_fields() {
        // Models with shapes, a name, size 2 x 3, an animation, a recolour,
        // signed ambient, break-route, then morph children.
        let data = [
            &[1, 2, 0, 10, 4, 0, 11, 22][..],
            &[5, 1, 0, 99],
            &[2, b'G', b'a', b't', b'e', 0],
            &[14, 2, 15, 3],
            &[24, 1, 44],
            &[40, 1, 0, 1, 0, 2],
            &[29, 0xfb],
            &[70, 0xff, 0xfe],
            &[74],
            &[77, 0, 0, 0, 0, 1, 0, 7, 0xff, 0xff],
            &[0],
        ]
        .concat();
        let (definition, children) = loc_def(&data).unwrap();
        assert_eq!(definition["models"], serde_json::json!([10, 11]));
        assert_eq!(definition["shapes"], serde_json::json!([4, 22]));
        assert_eq!(definition["name"], "Gate");
        assert_eq!(definition["width"], 2);
        assert_eq!(definition["length"], 3);
        assert_eq!(definition["animation"], 300);
        assert_eq!(definition["recolor"], serde_json::json!([[1, 2]]));
        assert_eq!(definition["ambient"], -5);
        assert_eq!(definition["offset"], serde_json::json!([-2, 0, 0]));
        // Break-route clears blocking at the end.
        assert_eq!(definition["blockwalk"], 0);
        assert_eq!(definition["blockrange"], false);
        assert_eq!(children, [7, 65535, 65535]);
        assert_eq!(definition["children"], serde_json::json!([7, 65535, 65535]));
        assert!(loc_def(&[200]).is_err());
        // A plain definition has no children entry and the defaults.
        let (plain, children) = loc_def(&[0]).unwrap();
        assert!(plain.get("children").is_none() && children.is_empty());
        assert_eq!(plain["blockwalk"], 2);
        assert_eq!(plain["shapes"], serde_json::Value::Null);
    }

    #[test]
    fn dependency_readers_follow_the_scripts_opcode_tables() {
        // NPC: two models (one empty), a movement set, morph children with the
        // extra default of opcode 118, and an unknown opcode without operands.
        let npc = [
            &[1, 2, 0, 50, 0xff, 0xff][..],
            &[127, 0, 9],
            &[138],
            &[118, 0, 0, 0, 0, 0, 70, 1, 0, 71, 0xff, 0xff],
            &[0],
        ]
        .concat();
        let (models, bas, children) = npc_dependencies(&npc).unwrap();
        assert_eq!((models, bas), (vec![50], 9));
        assert_eq!(children.into_iter().collect::<Vec<_>>(), [70, 71]);

        let mut sequences = BTreeSet::new();
        bas_sequences(
            &[
                1, 0, 5, 0xff, 0xff, 2, 0, 6, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 40, 0, 7,
                0,
            ],
            &mut sequences,
        )
        .unwrap();
        assert_eq!(sequences.into_iter().collect::<Vec<_>>(), [5, 6, 7]);

        let mut groups = BTreeSet::new();
        // Opcode 5 first, then two frames: four bytes each, then their groups.
        sequence_frame_groups(
            1,
            &[5, 9, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 0, 31],
            &mut groups,
        )
        .unwrap();
        assert_eq!(groups.into_iter().collect::<Vec<_>>(), [30, 31]);
        // A sequence without frames has none.
        let mut groups = BTreeSet::new();
        sequence_frame_groups(1, &[0], &mut groups).unwrap();
        assert!(groups.is_empty());
        assert!(sequence_frame_groups(1, &[77], &mut BTreeSet::new()).is_err());
    }

    #[test]
    fn windows_1252_text_is_decoded() {
        let mut reader = Reader {
            data: &[b'a', 0x80, 0xe9, 0, 0x81, 0],
            at: 0,
        };
        assert_eq!(reader.text().unwrap(), "a\u{20ac}\u{e9}");
        assert!(reader.text().is_err());
    }
}
