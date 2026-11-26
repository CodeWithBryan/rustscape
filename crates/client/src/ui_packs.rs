//! The two layered interface packs built from the pinned JS5 cache: the
//! native replacements for `tools/prepare_ui_models.py` and
//! `tools/prepare_player_models.py`.
//!
//! `ui_models_pack` extends an interface pack (`cache_pack::ui_pack`) with
//! what model widgets and chat heads need: every identity kit and its models,
//! the head models items and NPCs name, the models and sequences interface
//! components name, the pinned server's facial expressions, and the frames
//! and bases of those sequences. `ui_players_pack` extends that with what an
//! equipped player's body needs: the worn models of every item, every
//! movement set, and the sequences, frames and bases those name.
//!
//! Each writes the bytes its script wrote: the base pack's entries in their
//! manifest order, an entry met again keeping its place and taking the newer
//! bytes, then the additions in the script's order, and a blob holding each
//! entry once. Nothing is decoded beyond the dependency readers the scripts
//! had and the discovery the scripts ran through the client's own decoders.
use crate::{
    cache_pack::{Builder, Source},
    npc,
    pack::Pack,
    ui::{
        Component,
        item::{Catalog, ItemType},
    },
    world_pack::{Reader, file, group_files},
};
use std::collections::{BTreeSet, HashMap};

/// A pack and the counts its provenance file records.
pub struct Layered {
    /// The `RS09VIS1` pack.
    pub bytes: Vec<u8>,
    pub report: serde_json::Value,
}

/// The parts of a pack manifest the layered builders read.
#[derive(serde::Deserialize)]
struct Manifest {
    source: String,
    files: OrderedFiles,
}

/// The manifest's `files` object as its members are written, keeping their
/// order (the order the scripts' dictionaries had) and a name met again.
struct OrderedFiles(Vec<(String, [usize; 2])>);
impl<'de> serde::Deserialize<'de> for OrderedFiles {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = OrderedFiles;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object of [offset, size] spans")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<OrderedFiles, A::Error> {
                let mut files = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    files.push(entry);
                }
                Ok(OrderedFiles(files))
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}

/// Named entries in first-insertion order, each holding its latest bytes.
struct Entries {
    names: Vec<String>,
    data: Vec<Vec<u8>>,
    lookup: HashMap<String, usize>,
    source: String,
}
impl Entries {
    /// A pack's entries in the order its manifest lists them. That is the
    /// order the scripts' dictionaries had; it is not the order of the
    /// payloads where a builder wrote a name twice.
    fn from_pack(bytes: &[u8]) -> Result<Self, String> {
        let header = bytes.get(..12).ok_or("truncated pack")?;
        if &header[..8] != b"RS09VIS1" {
            return Err("not an RS09VIS1 pack".into());
        }
        let length = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;
        let manifest = bytes
            .get(12..12 + length)
            .ok_or("truncated pack manifest")?;
        let blob = &bytes[12 + length..];
        let manifest: Manifest =
            serde_json::from_slice(manifest).map_err(|e| format!("pack manifest: {e}"))?;
        let mut entries = Self {
            names: vec![],
            data: vec![],
            lookup: HashMap::new(),
            source: manifest.source,
        };
        for (name, [start, size]) in manifest.files.0 {
            let data = start
                .checked_add(size)
                .and_then(|end| blob.get(start..end))
                .ok_or_else(|| format!("{name} lies outside the pack"))?;
            entries.set(name, data.to_vec());
        }
        Ok(entries)
    }
    fn set(&mut self, name: String, data: Vec<u8>) {
        if let Some(&slot) = self.lookup.get(&name) {
            self.data[slot] = data;
        } else {
            self.lookup.insert(name.clone(), self.names.len());
            self.names.push(name);
            self.data.push(data);
        }
    }
    fn contains(&self, name: &str) -> bool {
        self.lookup.contains_key(name)
    }
    fn finish(self) -> Result<Vec<u8>, String> {
        let mut pack = Builder::default();
        for (name, data) in self.names.into_iter().zip(&self.data) {
            pack.add(name, data);
        }
        pack.finish(&self.source)
    }
}

/// Sequence IDs named by the pinned server's `FacialExpression.java`: every
/// `NAME(number)` in the source, as the script's expression found them.
pub fn facial_sequences(java: &[u8]) -> BTreeSet<i32> {
    let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut found = BTreeSet::new();
    let mut at = 0;
    while at < java.len() {
        let starts = java[at].is_ascii_uppercase() && (at == 0 || !word(java[at - 1]));
        if !starts {
            at += 1;
            continue;
        }
        let mut end = at + 1;
        while end < java.len()
            && (java[end].is_ascii_uppercase() || java[end].is_ascii_digit() || java[end] == b'_')
        {
            end += 1;
        }
        let digits = end + 1;
        let mut close = digits;
        while close < java.len() && java[close].is_ascii_digit() {
            close += 1;
        }
        if java.get(end) == Some(&b'(') && close > digits && java.get(close) == Some(&b')') {
            if let Some(id) = std::str::from_utf8(&java[digits..close])
                .ok()
                .and_then(|text| text.parse().ok())
            {
                found.insert(id);
            }
            at = close + 1;
        } else {
            at += 1;
        }
    }
    found
}

/// The frame groups a sequence names, by the scripts' reader: the frame lists
/// of opcodes 1 and 12, leaving out the entry that stands for "no frame".
fn sequence_frame_groups(id: i32, data: &[u8], groups: &mut BTreeSet<i32>) -> Result<(), String> {
    let mut r = Reader::new(data);
    loop {
        let op = r.u1()?;
        match op {
            0 => return Ok(()),
            1 | 12 => {
                let count = if op == 1 { r.u2()? } else { r.u1()? } as usize;
                if op == 1 {
                    r.take(count * 2)?;
                }
                let mut lows = Vec::with_capacity(count);
                for _ in 0..count {
                    lows.push(r.u2()?);
                }
                for low in lows {
                    let high = r.u2()?;
                    if (high << 16 | low) != 65535 {
                        groups.insert(high);
                    }
                }
            }
            2 | 6 | 7 => {
                r.take(2)?;
            }
            4 | 14..=16 => {}
            5 | 8..=11 => {
                r.take(1)?;
            }
            3 => {
                let count = r.u1()? as usize;
                r.take(count)?;
            }
            13 => {
                for _ in 0..r.u2()? {
                    let count = r.u1()? as usize;
                    if count != 0 {
                        r.take(3 + (count - 1) * 2)?;
                    }
                }
            }
            other => return Err(format!("sequence {id}: unknown dependency opcode {other}")),
        }
    }
}

/// Adds the sequences, then the frames of the groups they name, then the
/// bases those frames name. Returns the group and base counts.
fn add_animations(
    source: &Source,
    entries: &mut Entries,
    sequences: &BTreeSet<i32>,
) -> Result<[usize; 2], String> {
    let archive = source.archive(20)?;
    let mut groups = BTreeSet::new();
    for &id in sequences {
        let data = file(&archive, 20, (id >> 7) as u32, (id & 127) as u32)?
            .ok_or_else(|| format!("seq/{id} is missing"))?;
        sequence_frame_groups(id, &data, &mut groups)?;
        entries.set(format!("seq/{id}"), data);
    }
    let frames = source.archive(0)?;
    let mut bases = BTreeSet::new();
    for &group in &groups {
        let mut files = group_files(&frames, 0, group as u32)?;
        files.sort_by_key(|(id, _)| *id);
        for (id, data) in files {
            let base = data.get(..2).ok_or("truncated animation frame")?;
            bases.insert(u16::from_be_bytes([base[0], base[1]]));
            entries.set(format!("frame/{group}/{id}"), data);
        }
    }
    let archive = source.archive(1)?;
    for &base in &bases {
        let data = file(&archive, 1, u32::from(base), 0)?
            .ok_or_else(|| format!("base/{base} is missing"))?;
        entries.set(format!("base/{base}"), data);
    }
    Ok([groups.len(), bases.len()])
}

/// What `ui_models_pack` takes from the two packs through the client's own
/// decoders: the head models of items and NPCs, and the models and sequences
/// of model components.
pub struct ModelDependencies {
    pub models: BTreeSet<i32>,
    pub sequences: BTreeSet<i32>,
    pub components: usize,
    pub items: usize,
    pub npcs: usize,
}
impl ModelDependencies {
    pub fn discover(ui: &Pack, world: &Pack) -> Result<Self, String> {
        let mut found = Self {
            models: BTreeSet::new(),
            sequences: BTreeSet::new(),
            components: 0,
            items: 0,
            npcs: 0,
        };
        for name in ui.manifest.files.keys() {
            if let Some(id) = name.strip_prefix("item/") {
                let id = id.parse().map_err(|_| format!("{name}: not an item"))?;
                let (item, _) =
                    ItemType::decode(id, ui.get(name)?).map_err(|e| format!("{name}: {e:?}"))?;
                found.items += 1;
                for id in [item.manhead, item.manhead2, item.womanhead, item.womanhead2] {
                    if id >= 0 {
                        found.models.insert(id);
                    }
                }
            } else if let Some(id) = name.strip_prefix("interface/") {
                let id = id
                    .split_once('/')
                    .and_then(|(group, component)| {
                        Some((group.parse::<i32>().ok()? << 16) | component.parse::<i32>().ok()?)
                    })
                    .ok_or_else(|| format!("{name}: not a component"))?;
                let (component, _) =
                    Component::decode(id, ui.get(name)?).map_err(|e| format!("{name}: {e:?}"))?;
                found.components += 1;
                if component.kind == 6 {
                    for (kind, id) in [
                        (component.model_type, component.model_id),
                        (component.active_model_type, component.active_model_id),
                    ] {
                        if kind == 1 && id >= 0 {
                            found.models.insert(id);
                        }
                    }
                    for id in [component.model_seq_id, component.active_model_seq_id] {
                        if id >= 0 {
                            found.sequences.insert(id);
                        }
                    }
                }
            }
        }
        for name in world.manifest.files.keys() {
            // `npc-catalog-complete` is a marker, not a definition.
            if !name.starts_with("npc/") {
                continue;
            }
            let definition =
                npc::Definition::decode(world.get(name)?).map_err(|e| format!("{name}: {e:?}"))?;
            found.npcs += 1;
            if let Some(heads) = definition.head_models {
                found.models.extend(heads.into_iter().map(|id| id as i32));
            }
        }
        Ok(found)
    }
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "models": self.models,
            "sequences": self.sequences,
            "components": self.components,
            "items": self.items,
            "npcs": self.npcs,
        })
    }
}

/// The interface pack `ui` extended for model widgets and chat heads. `world`
/// is a starting-scene pack; its NPC definitions name the heads. `facial` is
/// [`facial_sequences`] of the pinned server's source.
pub fn ui_models_pack(
    source: &Source,
    ui: &[u8],
    world: &[u8],
    facial: &BTreeSet<i32>,
) -> Result<Layered, String> {
    let dependencies =
        ModelDependencies::discover(&Pack::decode(ui.to_vec())?, &Pack::decode(world.to_vec())?)?;
    let mut entries = Entries::from_pack(ui)?;
    let mut models = dependencies.models.clone();
    let mut sequences = dependencies.sequences.clone();
    let mut kits = group_files(&source.archive(2)?, 2, 3)?;
    kits.sort_by_key(|(id, _)| *id);
    let kit_count = kits.len();
    for (id, data) in kits {
        let mut r = Reader::new(&data);
        loop {
            let op = r.u1()?;
            match op {
                0 => break,
                1 => {
                    r.take(1)?;
                }
                2 => {
                    for _ in 0..r.u1()? {
                        models.insert(r.u2()?);
                    }
                }
                3 => {}
                40 | 41 => {
                    let count = r.u1()? as usize;
                    r.take(count * 4)?;
                }
                60..=69 => {
                    models.insert(r.u2()?);
                }
                other => return Err(format!("kit/{id}: unknown dependency opcode {other}")),
            }
        }
        entries.set(format!("kit/{id}"), data);
    }
    sequences.extend(facial);
    let archive = source.archive(7)?;
    let mut missing = vec![];
    for &id in &models {
        match file(&archive, 7, id as u32, 0)? {
            Some(data) => entries.set(format!("model/{id}"), data),
            None => missing.push(id),
        }
    }
    let [groups, bases] = add_animations(source, &mut entries, &sequences)?;
    let report = serde_json::json!({
        "nativeDependencyDiscovery": dependencies.json(),
        "identityKits": kit_count,
        "widgetSequences": sequences.len(),
        "frameGroups": groups,
        "bases": bases,
        "modelsRequested": models.len(),
        "missingModels": missing,
    });
    Ok(Layered {
        bytes: entries.finish()?,
        report,
    })
}

/// The worn models of every item in an interface pack, resolved through the
/// client's item catalogue (so certificates and lent items name their
/// originals'), and the number of items read.
pub fn player_models(ui: &Pack) -> Result<(BTreeSet<i32>, usize), String> {
    let mut catalog = Catalog::default();
    let mut models = BTreeSet::new();
    let ids: Vec<i32> = ui
        .manifest
        .files
        .keys()
        .filter_map(|name| name.strip_prefix("item/")?.parse().ok())
        .collect();
    for &id in &ids {
        let item = catalog
            .get(ui, id)
            .map_err(|e| format!("item/{id}: {e:?}"))?;
        for group in [
            [item.manwear, item.manwear2, item.manwear3],
            [item.womanwear, item.womanwear2, item.womanwear3],
        ] {
            if group[0] != -1 {
                models.extend(group.into_iter().filter(|&id| id != -1));
            }
        }
    }
    Ok((models, ids.len()))
}

/// The pack of [`ui_models_pack`] extended for equipped player bodies.
pub fn ui_players_pack(source: &Source, ui: &[u8]) -> Result<Layered, String> {
    let (worn, items) = player_models(&Pack::decode(ui.to_vec())?)?;
    let mut entries = Entries::from_pack(ui)?;
    let archive = source.archive(7)?;
    let (mut added, mut missing) = (0usize, vec![]);
    for &id in &worn {
        let name = format!("model/{id}");
        if entries.contains(&name) {
            continue;
        }
        match file(&archive, 7, id as u32, 0)? {
            Some(data) => {
                entries.set(name, data);
                added += 1;
            }
            None => missing.push(id),
        }
    }
    let mut sets = group_files(&source.archive(2)?, 2, 32)?;
    sets.sort_by_key(|(id, _)| *id);
    let set_count = sets.len();
    let mut sequences = BTreeSet::new();
    for (id, data) in sets {
        let mut r = Reader::new(&data);
        loop {
            let op = r.u1()?;
            match op {
                0 => break,
                1 => {
                    sequences.insert(r.u2()?);
                    sequences.insert(r.u2()?);
                }
                2..=9 | 38..=42 => {
                    sequences.insert(r.u2()?);
                }
                26 | 30 | 32 | 33 | 35 | 36 | 43 | 44 | 45 => {
                    r.take(2)?;
                }
                27 => {
                    r.take(13)?;
                }
                29 | 31 | 34 | 37 => {
                    r.take(1)?;
                }
                other => return Err(format!("bas/{id}: unknown opcode {other}")),
            }
        }
        entries.set(format!("bas/{id}"), data);
    }
    sequences.remove(&65535);
    let [groups, bases] = add_animations(source, &mut entries, &sequences)?;
    let report = serde_json::json!({
        "nativeDependencyDiscovery": { "models": worn, "items": items },
        "additionalModels": added,
        "missingModels": missing,
        "basDefinitions": set_count,
        "movementSequences": sequences.len(),
        "frameGroups": groups,
        "bases": bases,
    });
    Ok(Layered {
        bytes: entries.finish()?,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facial_expressions_are_found_as_the_script_found_them() {
        let java = b"enum F { HAPPY(588), OLD_SAD2(9760),\n lower(1), X9_(12) , A (3), B(), C(4x), _D(5), aE(6), F(7)G(8); }";
        assert_eq!(
            facial_sequences(java).into_iter().collect::<Vec<_>>(),
            [7, 8, 12, 588, 9760]
        );
    }

    #[test]
    fn sequence_frame_lists_leave_out_the_empty_frame() {
        // Opcode 1: two frames with durations, the second "no frame"; a
        // one-byte opcode; opcode 12 with one frame; opcode 13 with an empty
        // and a three-sound entry.
        let data = [
            1, 0, 2, 0, 5, 0, 6, 0, 9, 0xff, 0xff, 0, 40, 0, 0, 5, 3, 12, 1, 0, 1, 0, 41, 13, 0, 2,
            0, 3, 9, 9, 9, 8, 8, 7, 7, 0,
        ];
        let mut groups = BTreeSet::new();
        sequence_frame_groups(1, &data, &mut groups).unwrap();
        assert_eq!(groups.into_iter().collect::<Vec<_>>(), [40, 41]);
        assert!(sequence_frame_groups(1, &[200, 0], &mut BTreeSet::new()).is_err());
    }

    #[test]
    fn entries_keep_the_manifest_order_and_the_latest_bytes() {
        let mut pack = Builder::default();
        pack.add("b".into(), &[1]);
        pack.add("a".into(), &[2, 2]);
        pack.add("b".into(), &[3, 3, 3]);
        let bytes = pack.finish("test").unwrap();
        let mut entries = Entries::from_pack(&bytes).unwrap();
        assert_eq!(entries.names, ["b", "a"]);
        assert_eq!(entries.data, [vec![3, 3, 3], vec![2, 2]]);
        entries.set("a".into(), vec![4]);
        entries.set("c".into(), vec![]);
        let rebuilt = Pack::decode(entries.finish().unwrap()).unwrap();
        assert_eq!(rebuilt.manifest.source, "test");
        // The blob now holds each entry once, in order.
        assert_eq!(rebuilt.manifest.files["b"], [0, 3]);
        assert_eq!(rebuilt.manifest.files["a"], [3, 1]);
        assert_eq!(rebuilt.manifest.files["c"], [4, 0]);
        assert!(Entries::from_pack(b"RS09VIS1").is_err());
    }
}
