//! Builds `RS09VIS1` asset packs directly from the pinned JS5 cache.
//!
//! The packs were first produced by `tools/prepare_*.py`. This module is the
//! native replacement: it reads the same archives through `rs09-cache` and
//! writes the same bytes, so a client needs only the cache and no Python step.
//! Nothing is decoded or baked here beyond the dependency discovery the
//! original scripts performed; asset parsing stays with the runtime decoders.
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use std::{collections::HashMap, path::Path};

/// Named entries in first-insertion order. A repeated name keeps its original
/// position and takes the newer span, while the earlier bytes stay in the blob,
/// exactly as the original dictionary-based packer behaved.
#[derive(Default)]
pub struct Builder {
    names: Vec<String>,
    spans: Vec<[usize; 2]>,
    lookup: HashMap<String, usize>,
    blob: Vec<u8>,
}
impl Builder {
    pub fn add(&mut self, name: String, data: &[u8]) {
        let span = [self.blob.len(), data.len()];
        self.blob.extend_from_slice(data);
        if let Some(&slot) = self.lookup.get(&name) {
            self.spans[slot] = span;
        } else {
            self.lookup.insert(name.clone(), self.names.len());
            self.names.push(name);
            self.spans.push(span);
        }
    }
    pub fn len(&self) -> usize {
        self.names.len()
    }
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
    /// Compact JSON manifest, then the blob. Names and the source are ASCII
    /// without characters that need escaping; anything else is rejected.
    pub fn finish(self, source: &str) -> Result<Vec<u8>, String> {
        let plain = |text: &str| {
            text.bytes()
                .all(|b| (0x20..0x7f).contains(&b) && b != b'"' && b != b'\\')
        };
        if !plain(source) || !self.names.iter().all(|name| plain(name)) {
            return Err("pack names must be plain ASCII".into());
        }
        let mut manifest = format!("{{\"source\":\"{source}\",\"files\":{{");
        for (i, (name, [start, len])) in self.names.iter().zip(&self.spans).enumerate() {
            if i != 0 {
                manifest.push(',');
            }
            manifest.push_str(&format!("\"{name}\":[{start},{len}]"));
        }
        manifest.push_str("}}");
        let length = u32::try_from(manifest.len()).map_err(|_| "pack manifest too large")?;
        let mut out = Vec::with_capacity(12 + manifest.len() + self.blob.len());
        out.extend_from_slice(b"RS09VIS1");
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(manifest.as_bytes());
        out.extend_from_slice(&self.blob);
        Ok(out)
    }
}

/// The pinned cache opened for packing. Groups are read once each, so the
/// shared decode cache is not needed.
pub struct Source {
    store: Store,
    limits: Limits,
}
impl Source {
    pub fn open(directory: &Path) -> Result<Self, String> {
        // A directory an update is still filling is read as it fills.
        if let Some(store) = crate::cache_live::store_for(directory) {
            return Ok(Self::from_store(store));
        }
        let limits = Limits::default();
        let store = Store::open(directory, StorageMode::Resident, limits)
            .map_err(|e| format!("{}: {e}", directory.display()))?;
        Ok(Self { store, limits })
    }
    /// The same over a store that is already open: a snapshot, or the live
    /// view of the store the browser client downloads into.
    pub fn from_store(store: Store) -> Self {
        Self {
            store,
            limits: Limits::default(),
        }
    }
    pub fn archive(&self, id: u8) -> Result<Archive, String> {
        Archive::open(self.store.clone(), id, None, self.limits)
            .map_err(|e| format!("archive {id}: {e}"))
    }
}

fn group(
    archive: &Archive,
    label: u8,
    id: u32,
) -> Result<std::sync::Arc<rs09_cache::group::GroupFiles>, String> {
    archive
        .group(id, [0; 4])
        .map_err(|e| format!("archive {label} group {id}: {e}"))?
        .ok_or_else(|| format!("archive {label} group {id} is missing"))
}
fn file(archive: &Archive, label: u8, id: u32, file: u32) -> Result<Vec<u8>, String> {
    group(archive, label, id)?
        .get(file)
        .map(|bytes| bytes.as_ref().clone())
        .ok_or_else(|| format!("archive {label} group {id} file {file} is missing"))
}
fn named(archive: &Archive, label: u8, name: &str) -> Result<u32, String> {
    archive
        .index
        .group_by_ascii_name(name)
        .ok_or_else(|| format!("archive {label} has no group named {name}"))
}

/// Inventory model IDs (opcode 1) of an item definition. This is the original
/// packer's dependency reader: it skips the operands of the opcodes it knows
/// and treats any other byte as an operand-less opcode.
fn item_models(data: &[u8], out: &mut std::collections::BTreeSet<u32>) -> Result<(), String> {
    let mut at = 0usize;
    let mut take = |count: usize| -> Result<&[u8], String> {
        let bytes = data
            .get(at..at + count)
            .ok_or("truncated item definition")?;
        at += count;
        Ok(bytes)
    };
    fn text<'a>(take: &mut impl FnMut(usize) -> Result<&'a [u8], String>) -> Result<(), String> {
        while take(1)?[0] != 0 {}
        Ok(())
    }
    loop {
        let op = take(1)?[0];
        match op {
            0 => return Ok(()),
            1 => {
                let b = take(2)?;
                out.insert(u32::from(u16::from_be_bytes([b[0], b[1]])));
            }
            2 | 30..=39 => text(&mut take)?,
            4..=8 | 23..=26 | 78 | 79 | 90..=93 | 95 | 97 | 98 | 110..=112 | 121 | 122 => {
                take(2)?;
            }
            12 => {
                take(4)?;
            }
            40 | 41 => {
                let count = usize::from(take(1)?[0]);
                take(count * 4)?;
            }
            42 => {
                let count = usize::from(take(1)?[0]);
                take(count)?;
            }
            96 | 113..=115 => {
                take(1)?;
            }
            100..=109 => {
                take(4)?;
            }
            125..=130 => {
                take(3)?;
            }
            249 => {
                for _ in 0..take(1)?[0] {
                    let is_text = take(1)?[0] == 1;
                    take(3)?;
                    if is_text {
                        text(&mut take)?;
                    } else {
                        take(4)?;
                    }
                }
            }
            _ => {}
        }
    }
}

/// The interface pack (`assets/visual/ui.pack`): every interface component,
/// client script, font, enum, item, varbit, inventory type, variable type and
/// parameter definition; the inventory models items name; every synthesised
/// sound; and the named chat/overhead sprites and Huffman table.
pub fn ui_pack(cache: &Path, server_commit: &str) -> Result<Vec<u8>, String> {
    let source = Source::open(cache)?;
    let mut pack = Builder::default();
    let mut models = std::collections::BTreeSet::from([0u32]);
    for (id, prefix) in [
        (3u8, "interface"),
        (12, "script"),
        (13, "font"),
        (17, "enum"),
        (19, "item"),
        (22, "varbit"),
    ] {
        let archive = source.archive(id)?;
        for info in &archive.index.groups {
            let files = group(&archive, id, info.id)?;
            for (fid, data) in files.iter() {
                let name = match id {
                    3 => format!("{prefix}/{}/{fid}", info.id),
                    17 | 19 => format!("{prefix}/{}", info.id * 256 + fid),
                    22 => format!("{prefix}/{}", info.id * 1024 + fid),
                    _ => format!("{prefix}/{}", info.id),
                };
                pack.add(name, data);
                if id == 19 {
                    item_models(data, &mut models).map_err(|e| format!("item {fid}: {e}"))?;
                }
            }
        }
    }
    let configs = source.archive(2)?;
    for (group_id, prefix) in [(5u32, "invtype"), (16, "varptype"), (11, "param")] {
        for (fid, data) in group(&configs, 2, group_id)?.iter() {
            pack.add(format!("{prefix}/{fid}"), data);
        }
    }
    pack.add("param-catalog-complete".into(), &[]);
    let model_archive = source.archive(7)?;
    for id in models {
        pack.add(format!("model/{id}"), &file(&model_archive, 7, id, 0)?);
    }
    let sounds = source.archive(4)?;
    for info in &sounds.index.groups {
        pack.add(format!("sound/{}", info.id), &file(&sounds, 4, info.id, 0)?);
    }
    let sprites = source.archive(8)?;
    let mut overhead = String::from("{");
    for (i, name) in [
        "p11_full",
        "p12_full",
        "b12_full",
        "hitmarks",
        "hitbar_default",
        "headicons_pk",
        "headicons_prayer",
        "hint_headicons",
    ]
    .into_iter()
    .enumerate()
    {
        if i != 0 {
            overhead.push(',');
        }
        overhead.push_str(&format!("\"{name}\":{}", named(&sprites, 8, name)?));
    }
    overhead.push('}');
    pack.add("overhead/assets".into(), overhead.as_bytes());
    let binary = source.archive(10)?;
    pack.add(
        "huffman".into(),
        &file(&binary, 10, named(&binary, 10, "huffman")?, 0)?,
    );
    for name in ["name_icons", "scrollbar"] {
        pack.add(
            format!("sprite/{name}"),
            &file(&sprites, 8, named(&sprites, 8, name)?, 0)?,
        );
    }
    pack.add(
        crate::title::Assets::ENTRY.into(),
        &title_assets(&source.archive(3)?, &source.archive(6)?, &sprites)?,
    );
    pack.finish(&format!("Pinned 2009scape cache / {server_commit}"))
}

/// Where a pack name lives in the cache: archive, group and file.
///
/// These are the naming rules the original packers used, extended to every
/// member of each family, so any asset the client names resolves on demand.
/// Names that are not cache files (generated JSON, catalogue markers) return
/// None and are supplied inline by `cache_pack`.
pub fn locate(name: &str, named: impl Fn(u8, &str) -> Option<u32>) -> Option<(u8, u32, u32)> {
    match name {
        "materials" => return Some((26, 0, 0)),
        "huffman" => return Some((10, named(10, "huffman")?, 0)),
        _ => {}
    }
    let (family, rest) = name.split_once('/')?;
    if let Some((group, file)) = rest.split_once('/') {
        let archive = match family {
            "interface" => 3,
            "frame" => 0,
            _ => return None,
        };
        return Some((archive, group.parse().ok()?, file.parse().ok()?));
    }
    if matches!(family, "qcphrase" | "qccat") {
        // Quick chat: local archive 24 below 32768, global archive 25 above;
        // group 0 holds categories and group 1 phrases.
        let id: u32 = rest.parse().ok()?;
        if id.to_string() != rest || id > 65535 {
            return None;
        }
        let group = u32::from(family == "qcphrase");
        return Some(if id < 32768 {
            (24, group, id)
        } else {
            (25, group, id & 0x7fff)
        });
    }
    if family == "sprite" && rest.parse::<u32>().is_err() {
        return Some((8, named(8, rest)?, 0));
    }
    let id: u32 = rest.parse().ok()?;
    // Canonical decimal only, so each asset has exactly one name.
    if id.to_string() != rest {
        return None;
    }
    Some(match family {
        "script" => (12, id, 0),
        "font" => (13, id, 0),
        "model" => (7, id, 0),
        "sound" => (4, id, 0),
        "sprite" => (8, id, 0),
        "texture" => (9, id, 0),
        "base" => (1, id, 0),
        "enum" => (17, id >> 8, id & 255),
        "item" => (19, id >> 8, id & 255),
        "loc" => (16, id >> 8, id & 255),
        "spot" => (21, id >> 8, id & 255),
        "varbit" => (22, id >> 10, id & 1023),
        "seq" => (20, id >> 7, id & 127),
        "npc" => (18, id >> 7, id & 127),
        "underlay" => (2, 1, id),
        "kit" => (2, 3, id),
        "overlay" => (2, 4, id),
        "invtype" => (2, 5, id),
        "param" => (2, 11, id),
        "varptype" => (2, 16, id),
        "struct" => (2, 26, id),
        "light" => (2, 31, id),
        "bas" => (2, 32, id),
        "cursor" => (2, 33, id),
        "mapscene" => (2, 34, id),
        _ => return None,
    })
}

/// The groups client.mainLoad finds by name for the title screen (the
/// interface, the song, the background and the logo) as the JSON object
/// [crate::title::Assets::read] takes: the runtime packs keep no names.
pub fn title_assets(
    interfaces: &Archive,
    songs: &Archive,
    sprites: &Archive,
) -> Result<Vec<u8>, String> {
    use crate::login_flow::{TITLE_BACKGROUND, TITLE_INTERFACE, TITLE_LOGO, TITLE_SONG};
    Ok(format!(
        "{{\"{TITLE_INTERFACE}\":{},\"{TITLE_SONG}\":{},\"{TITLE_BACKGROUND}\":{},\"{TITLE_LOGO}\":{}}}",
        named(interfaces, 3, TITLE_INTERFACE)?,
        named(songs, 6, TITLE_SONG)?,
        named(sprites, 8, TITLE_BACKGROUND)?,
        named(sprites, 8, TITLE_LOGO)?,
    )
    .into_bytes())
}

/// Named sprite groups written as JSON, in the original packers' key order.
fn sprite_ids(sprites: &Archive, names: &[&str]) -> Result<Vec<u8>, String> {
    let mut json = String::from("{");
    for (i, name) in names.iter().enumerate() {
        if i != 0 {
            json.push(',');
        }
        json.push_str(&format!("\"{name}\":{}", named(sprites, 8, name)?));
    }
    json.push('}');
    Ok(json.into_bytes())
}

/// The whole pinned cache as one lazily resolved pack.
///
/// Every interface, script, definition, model, animation, sprite, texture and
/// sound is listed under the name the prepared packs use and is read from the
/// cache the first time it is asked for. Only the index tables are read here.
/// Map files are not included: they need per-region keys and are streamed.
pub fn cache_pack(cache: &Path, server_commit: &str) -> Result<crate::pack::Pack, String> {
    cache_pack_from(&Source::open(cache)?, server_commit)
}
/// [`cache_pack`] over an opened store. Every archive's index has to be in
/// it already; groups may still be arriving, and one that has not arrived
/// fails to resolve when it is asked for.
pub fn cache_pack_from(source: &Source, server_commit: &str) -> Result<crate::pack::Pack, String> {
    let mut archives = std::collections::BTreeMap::new();
    let mut names = Vec::new();
    // (archive, family, how a group/file pair is named)
    let single = |family: &'static str| move |group: u32, _: u32| format!("{family}/{group}");
    let pair =
        |family: &'static str| move |group: u32, file: u32| format!("{family}/{group}/{file}");
    let packed = |family: &'static str, shift: u32| {
        move |group: u32, file: u32| format!("{family}/{}", (group << shift) + file)
    };
    type Namer = Box<dyn Fn(u32, u32) -> String>;
    let families: Vec<(u8, Namer, bool)> = vec![
        (3, Box::new(pair("interface")), true),
        (0, Box::new(pair("frame")), true),
        (12, Box::new(single("script")), false),
        (13, Box::new(single("font")), false),
        (7, Box::new(single("model")), false),
        (4, Box::new(single("sound")), false),
        (8, Box::new(single("sprite")), false),
        (9, Box::new(single("texture")), false),
        (1, Box::new(single("base")), false),
        (17, Box::new(packed("enum", 8)), true),
        (19, Box::new(packed("item", 8)), true),
        (16, Box::new(packed("loc", 8)), true),
        (21, Box::new(packed("spot", 8)), true),
        (22, Box::new(packed("varbit", 10)), true),
        (20, Box::new(packed("seq", 7)), true),
        (18, Box::new(packed("npc", 7)), true),
    ];
    for (id, namer, per_file) in families {
        let archive = source.archive(id)?;
        for group in &archive.index.groups {
            if per_file {
                names.extend(group.files.iter().map(|file| namer(group.id, file.id)));
            } else if group.file(0).is_some() {
                names.push(namer(group.id, 0));
            }
        }
        archives.insert(id, archive);
    }
    let configs = source.archive(2)?;
    for (group, family) in [
        (1u32, "underlay"),
        (3, "kit"),
        (4, "overlay"),
        (5, "invtype"),
        (11, "param"),
        (16, "varptype"),
        (26, "struct"),
        (31, "light"),
        (32, "bas"),
        (33, "cursor"),
        (34, "mapscene"),
    ] {
        if let Some(info) = configs.index.group(group) {
            names.extend(
                info.files
                    .iter()
                    .map(|file| format!("{family}/{}", file.id)),
            );
        }
    }
    archives.insert(2, configs);
    // Quick chat phrase counts are the capacity of group 1 in each archive.
    let mut quick_chat_counts = [0u32; 2];
    for (slot, id) in [24u8, 25].into_iter().enumerate() {
        let archive = source.archive(id)?;
        let offset = if id == 25 { 32768 } else { 0 };
        for (group, family) in [(0u32, "qccat"), (1, "qcphrase")] {
            if let Some(info) = archive.index.group(group) {
                names.extend(
                    info.files
                        .iter()
                        .map(|file| format!("{family}/{}", file.id + offset)),
                );
                if group == 1 {
                    quick_chat_counts[slot] = info.capacity;
                }
            }
        }
        archives.insert(id, archive);
    }
    for id in [10u8, 26] {
        archives.insert(id, source.archive(id)?);
    }
    names.extend(["materials".to_owned(), "huffman".to_owned()]);
    let count = |prefix: &str| names.iter().filter(|name| name.starts_with(prefix)).count();
    let animation_counts = format!(
        "{{\"seq\":{},\"frame\":{},\"base\":{}}}",
        count("seq/"),
        count("frame/"),
        count("base/")
    )
    .into_bytes();
    let sprites = &archives[&8];
    names.extend(["name_icons", "scrollbar"].map(|name| format!("sprite/{name}")));
    let inline = vec![
        (
            "overhead/assets".to_owned(),
            sprite_ids(
                sprites,
                &[
                    "p11_full",
                    "p12_full",
                    "b12_full",
                    "hitmarks",
                    "hitbar_default",
                    "headicons_pk",
                    "headicons_prayer",
                    "hint_headicons",
                ],
            )?,
        ),
        (
            "minimap/assets".to_owned(),
            sprite_ids(
                sprites,
                &[
                    "mapfunction",
                    "mapdots",
                    "mapflag",
                    "compass",
                    "hint_mapmarkers",
                    "hint_mapedge",
                    // Not a minimap sprite: the floor-shadow masks
                    // (`ShadowManager.addFloorShadow`), which `world_pack`
                    // also lists here; without them the floors of the upper
                    // levels cast no shadow in a scene built from the cache.
                    "floorshadows",
                ],
            )?,
        ),
        (
            "quickchat/counts".to_owned(),
            format!("[{},{}]", quick_chat_counts[0], quick_chat_counts[1]).into_bytes(),
        ),
        (
            crate::title::Assets::ENTRY.to_owned(),
            title_assets(&archives[&3], &source.archive(6)?, sprites)?,
        ),
        ("param-catalog-complete".to_owned(), vec![]),
        ("npc-catalog-complete".to_owned(), vec![]),
        ("spot-catalog-complete".to_owned(), vec![]),
        // Every cursor type the cache holds is listed, so an unlisted id is
        // one `CursorTypeList.get` would also find no file for.
        ("cursor-catalog-complete".to_owned(), vec![]),
        // The animation packer recorded its three counts in this marker.
        ("animation-catalog-complete".to_owned(), animation_counts),
    ];
    let resolver = move |name: &str| -> Result<Vec<u8>, String> {
        let by_name = |archive: u8, name: &str| {
            archives
                .get(&archive)
                .and_then(|a| a.index.group_by_ascii_name(name))
        };
        let (archive, group_id, file_id) =
            locate(name, by_name).ok_or_else(|| format!("{name} is not a cache asset name"))?;
        let archive_ref = archives
            .get(&archive)
            .ok_or_else(|| format!("{name}: archive {archive} is not open"))?;
        file(archive_ref, archive, group_id, file_id).map_err(|e| format!("{name}: {e}"))
    };
    Ok(crate::pack::Pack::lazy(
        format!("Pinned 2009scape cache / {server_commit}"),
        names,
        inline,
        Box::new(resolver),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_names_keep_their_position_and_the_blob_keeps_both_payloads() {
        let mut pack = Builder::default();
        pack.add("a".into(), b"one");
        pack.add("b".into(), b"two!");
        pack.add("a".into(), b"3");
        assert_eq!(pack.len(), 2);
        let bytes = pack.finish("test").unwrap();
        let manifest = r#"{"source":"test","files":{"a":[7,1],"b":[3,4]}}"#;
        let mut expected = b"RS09VIS1".to_vec();
        expected.extend((manifest.len() as u32).to_le_bytes());
        expected.extend(manifest.as_bytes());
        expected.extend(b"onetwo!3");
        assert_eq!(bytes, expected);
        let decoded = crate::pack::Pack::decode(bytes).unwrap();
        assert_eq!(decoded.get("a").unwrap(), b"3");
        assert_eq!(decoded.get("b").unwrap(), b"two!");
    }

    #[test]
    fn names_that_would_need_json_escaping_are_rejected() {
        for name in ["quote\"", "back\\slash", "caf\u{e9}", "tab\t"] {
            let mut pack = Builder::default();
            pack.add(name.into(), b"");
            assert!(pack.finish("ok").is_err(), "{name}");
        }
        assert!(Builder::default().finish("bad\"source").is_err());
    }

    #[test]
    fn item_dependency_reader_collects_inventory_models_only() {
        let mut models = std::collections::BTreeSet::new();
        // op 1 model 300; op 2 name; op 23 (male model, skipped); op 40 one
        // recolour pair; op 249 with one int and one string parameter; op 200
        // (unknown, no operand); end.
        let data = [
            &[1, 1, 44][..],
            &[2, b'A', 0],
            &[23, 9, 9],
            &[40, 1, 0, 1, 0, 2],
            &[249, 2, 0, 0, 0, 5, 0, 0, 0, 7, 1, 0, 0, 6, b'x', 0],
            &[200],
            &[0],
        ]
        .concat();
        item_models(&data, &mut models).unwrap();
        assert_eq!(models.into_iter().collect::<Vec<_>>(), [300]);
        let mut models = std::collections::BTreeSet::new();
        assert!(item_models(&[1, 1], &mut models).is_err());
        assert!(item_models(&[2, b'A'], &mut models).is_err());
    }

    #[test]
    fn pack_names_map_to_the_original_cache_locations() {
        let named = |archive: u8, name: &str| match (archive, name) {
            (8, "scrollbar") => Some(77),
            (10, "huffman") => Some(1),
            _ => None,
        };
        for (name, expected) in [
            ("interface/548/12", Some((3, 548, 12))),
            ("frame/40/3", Some((0, 40, 3))),
            ("script/1004", Some((12, 1004, 0))),
            ("font/495", Some((13, 495, 0))),
            ("model/65535", Some((7, 65535, 0))),
            ("sound/2", Some((4, 2, 0))),
            ("sprite/494", Some((8, 494, 0))),
            ("sprite/scrollbar", Some((8, 77, 0))),
            ("sprite/unknown", None),
            ("texture/5", Some((9, 5, 0))),
            ("base/9", Some((1, 9, 0))),
            ("enum/1000", Some((17, 3, 232))),
            ("item/4151", Some((19, 16, 55))),
            ("loc/12348", Some((16, 48, 60))),
            ("spot/369", Some((21, 1, 113))),
            ("varbit/5000", Some((22, 4, 904))),
            ("seq/808", Some((20, 6, 40))),
            ("npc/8590", Some((18, 67, 14))),
            ("underlay/3", Some((2, 1, 3))),
            ("kit/18", Some((2, 3, 18))),
            ("overlay/3", Some((2, 4, 3))),
            ("invtype/93", Some((2, 5, 93))),
            ("param/1", Some((2, 11, 1))),
            ("varptype/300", Some((2, 16, 300))),
            ("struct/7", Some((2, 26, 7))),
            ("light/2", Some((2, 31, 2))),
            ("bas/1426", Some((2, 32, 1426))),
            ("cursor/7", Some((2, 33, 7))),
            ("mapscene/4", Some((2, 34, 4))),
            ("qcphrase/5", Some((24, 1, 5))),
            ("qcphrase/32773", Some((25, 1, 5))),
            ("qccat/0", Some((24, 0, 0))),
            ("qccat/32768", Some((25, 0, 0))),
            ("qcphrase/65536", None),
            ("materials", Some((26, 0, 0))),
            ("huffman", Some((10, 1, 0))),
            // Generated or map entries are not plain cache files.
            ("overhead/assets", None),
            ("scene.json", None),
            ("terrain/50_50", None),
            ("model/007", None),
            ("model/-1", None),
            ("interface/1/2/3", None),
            ("unknown/1", None),
        ] {
            assert_eq!(locate(name, named), expected, "{name}");
        }
    }
}
