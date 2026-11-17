//! Where the minimap's cache data is read from.
//!
//! The original reads everything the minimap draws from its cache:
//! `Sprites.load` finds the `mapfunction`, `mapdots`, `mapflag`, `compass`,
//! `hint_mapmarkers` and `hint_mapedge` sprite groups by name, `MsiTypeList`
//! reads the map scene definitions (config group 34) and `FluTypeList` /
//! `FloTypeList` the floors (config groups 1 and 4). The Rust client gets them
//! from one of three places, in this order:
//!
//! 1. the cache, when the client has one open (`--cache`, the default cache a
//!    run without a UI pack falls back to, or the browser's downloaded cache):
//!    its lazy pack lists `minimap/assets` and every `mapscene/`, `underlay/`
//!    and `overlay/` entry;
//! 2. the starting-scene pack, when it carries them (the scene built from the
//!    cache by `world_pack` does; so do packs from the current
//!    `tools/prepare_visual.py`);
//! 3. the tables below, which are the pinned cache's own values, for a
//!    prepared pack written from that cache before the packers emitted them
//!    (the recorded `assets/visual/demo.pack` and the tour packs). They apply
//!    only to a pack whose manifest names the pinned cache.
//!
//! Each family is taken from the first place that has it. Locations are not
//! read here: the scene's location types (`Workbench::location_types`, filled
//! from the cache for every streamed region and its morph children) are, and
//! [`retain`] keeps the ones the minimap's icons need.
use super::MapScene;
use crate::{
    floor::{Overlay, Underlay},
    interaction::LocationType,
    pack::Pack,
};
use std::collections::BTreeMap;

/// The pack entry holding the sprite group IDs, as JSON `{"name": group}`.
pub const ASSETS: &str = "minimap/assets";
/// The named sprite groups `minimap/assets` lists, in the packers' order.
/// `floorshadows` is not a minimap sprite; the packs keep its ID here.
pub const SPRITE_NAMES: [&str; 7] = [
    "mapfunction",
    "mapdots",
    "mapflag",
    "compass",
    "hint_mapmarkers",
    "hint_mapedge",
    "floorshadows",
];

/// The manifest source of packs written from the pinned cache.
pub const PINNED_SOURCE: &str = "Pinned 2009scape cache / 9921b98d49308edd0489dd50ee98011d53914a57";
/// [`SPRITE_NAMES`] resolved in the pinned cache's sprite index (archive 8).
pub const PINNED_SPRITES: [(&str, i32); 7] = [
    ("mapfunction", 318),
    ("mapdots", 300),
    ("mapflag", 1222),
    ("compass", 169),
    ("hint_mapmarkers", 13),
    ("hint_mapedge", 14),
    ("floorshadows", 1243),
];
/// Every file of the pinned cache's config group 34 (`MsiType`), raw.
pub const PINNED_MAPSCENES: &[(u32, &[u8])] = &[
    (0, &[1, 6, 67, 0]),
    (1, &[1, 6, 68, 0]),
    (2, &[1, 6, 69, 0]),
    (3, &[1, 6, 70, 0]),
    (4, &[1, 6, 71, 0]),
    (5, &[1, 6, 72, 0]),
    (6, &[1, 6, 73, 0]),
    (7, &[1, 6, 74, 0]),
    (8, &[1, 6, 75, 0]),
    (9, &[1, 6, 76, 0]),
    (10, &[1, 6, 77, 0]),
    (11, &[1, 6, 78, 0]),
    (12, &[1, 6, 79, 0]),
    (13, &[1, 6, 80, 0]),
    (14, &[1, 6, 81, 0]),
    (15, &[1, 6, 82, 0]),
    (16, &[1, 6, 83, 0]),
    (17, &[1, 6, 84, 0]),
    (18, &[1, 6, 85, 0]),
    (19, &[1, 6, 86, 0]),
    (20, &[1, 6, 87, 0]),
    (21, &[1, 6, 88, 0]),
    (22, &[4, 0]),
    (23, &[1, 6, 89, 0]),
    (24, &[1, 6, 90, 0]),
    (25, &[1, 6, 91, 0]),
    (26, &[1, 6, 92, 0]),
    (27, &[1, 6, 93, 0]),
    (28, &[1, 6, 94, 0]),
    (29, &[1, 6, 95, 0]),
    (30, &[1, 6, 96, 0]),
    (31, &[1, 6, 97, 0]),
    (32, &[1, 6, 98, 0]),
    (33, &[1, 6, 99, 0]),
    (34, &[1, 6, 100, 0]),
    (35, &[1, 6, 101, 0]),
    (36, &[4, 0]),
    (37, &[4, 0]),
    (38, &[4, 0]),
    (39, &[4, 0]),
    (40, &[4, 0]),
    (41, &[1, 6, 102, 0]),
    (42, &[1, 6, 103, 0]),
    (43, &[1, 6, 104, 0]),
    (44, &[1, 6, 105, 0]),
    (45, &[1, 6, 106, 0]),
    (46, &[1, 6, 107, 0]),
    (47, &[1, 6, 108, 0]),
    (48, &[1, 6, 109, 0]),
    (49, &[1, 6, 110, 0]),
    (50, &[1, 6, 111, 0]),
    (51, &[1, 6, 112, 0]),
    (52, &[1, 6, 113, 0]),
    (53, &[1, 6, 114, 0]),
    (54, &[1, 6, 115, 0]),
    (55, &[1, 6, 116, 0]),
    (56, &[1, 6, 117, 0]),
    (57, &[1, 6, 118, 0]),
    (58, &[1, 6, 119, 0]),
    (59, &[1, 6, 120, 0]),
    (60, &[1, 6, 121, 0]),
    (61, &[1, 6, 122, 0]),
    (62, &[1, 6, 123, 0]),
    (63, &[1, 6, 124, 0]),
    (64, &[1, 6, 125, 0]),
    (65, &[1, 6, 126, 0]),
    (66, &[1, 6, 127, 0]),
    (67, &[1, 6, 128, 0]),
    (68, &[1, 6, 129, 0]),
    (69, &[1, 6, 130, 0]),
    (70, &[1, 6, 131, 0]),
    (71, &[1, 6, 132, 0]),
    (72, &[1, 6, 133, 0]),
    (73, &[1, 6, 134, 0]),
    (74, &[1, 6, 135, 0]),
    (75, &[1, 6, 136, 0]),
    (76, &[1, 6, 137, 0]),
    (77, &[1, 6, 138, 0]),
    (78, &[1, 6, 139, 0]),
    (79, &[1, 6, 140, 0]),
    (80, &[1, 6, 141, 0]),
    (81, &[1, 6, 142, 0]),
    (82, &[1, 6, 143, 0]),
    (83, &[1, 6, 144, 0]),
    (84, &[1, 6, 145, 0]),
    (85, &[1, 6, 146, 0]),
    (86, &[1, 6, 147, 0]),
    (87, &[1, 6, 148, 0]),
    (88, &[1, 6, 149, 0]),
    (89, &[1, 6, 150, 0]),
    (90, &[1, 6, 151, 0]),
    (91, &[1, 6, 152, 0]),
    (92, &[1, 6, 153, 0]),
    (93, &[4, 0]),
    (94, &[4, 0]),
    (95, &[4, 0]),
];

/// Where one family of minimap data was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Cache,
    Scene,
    Pinned,
}

/// What the minimap reads once, at start-up.
pub struct Tables {
    pub assets: BTreeMap<String, i32>,
    pub mapscenes: BTreeMap<i32, MapScene>,
    pub underlays: Vec<Underlay>,
    pub overlays: Vec<Overlay>,
    /// Origins of the sprite names, the map scenes and the floors.
    pub origins: [Origin; 3],
}

fn lists(pack: &Pack, prefix: &str) -> bool {
    pack.manifest
        .files
        .range(prefix.to_owned()..)
        .next()
        .is_some_and(|(name, _)| name.starts_with(prefix))
}

/// `prefix`-family entries of `pack` by numeric ID.
fn family<T>(
    pack: &Pack,
    prefix: &str,
    decode: impl Fn(&[u8]) -> Result<T, String>,
) -> Result<BTreeMap<u32, T>, String> {
    let mut out = BTreeMap::new();
    for name in pack.manifest.files.keys() {
        if let Some(id) = name.strip_prefix(prefix) {
            let id = id.parse().map_err(|_| format!("Minimap entry {name}"))?;
            out.insert(
                id,
                decode(pack.get(name)?).map_err(|e| format!("{name}: {e}"))?,
            );
        }
    }
    Ok(out)
}

/// The tables from the first source holding each family; see the module
/// documentation. `cache` is the cache-backed pack, if the client opened a
/// cache; `scene` the starting-scene pack. Fails when no source has the
/// sprite names or the map scenes.
pub fn tables(cache: Option<&Pack>, scene: &Pack) -> Result<Tables, String> {
    let pinned = scene.manifest.source == PINNED_SOURCE;
    let packs = cache.into_iter().map(|p| (p, Origin::Cache));
    let packs: Vec<(&Pack, Origin)> = packs.chain([(scene, Origin::Scene)]).collect();

    let (assets, assets_origin) = match packs.iter().find(|(p, _)| p.get(ASSETS).is_ok()) {
        Some(&(pack, origin)) => (
            serde_json::from_slice(pack.get(ASSETS)?).map_err(|e| format!("{ASSETS}: {e}"))?,
            origin,
        ),
        None if pinned => (
            PINNED_SPRITES
                .iter()
                .map(|&(name, id)| (name.to_owned(), id))
                .collect(),
            Origin::Pinned,
        ),
        None => {
            return Err(format!(
                "no {ASSETS} in the cache or the scene pack ({})",
                scene.manifest.source
            ));
        }
    };
    let decode_scene = |data: &[u8]| MapScene::decode(data).map_err(|e| e.to_string());
    let (mapscenes, scenes_origin) = match packs.iter().find(|(p, _)| lists(p, "mapscene/")) {
        Some(&(pack, origin)) => (family(pack, "mapscene/", decode_scene)?, origin),
        None if pinned => {
            let mut out = BTreeMap::new();
            for &(id, data) in PINNED_MAPSCENES {
                out.insert(
                    id,
                    decode_scene(data).map_err(|e| format!("mapscene/{id}: {e}"))?,
                );
            }
            (out, Origin::Pinned)
        }
        None => {
            return Err(format!(
                "no map scenes in the cache or the scene pack ({})",
                scene.manifest.source
            ));
        }
    };
    let mapscenes = mapscenes
        .into_iter()
        .map(|(id, scene)| (id as i32, scene))
        .collect();
    // The scene pack always has floors: its terrain is drawn with them.
    let (floors, floors_origin) = *packs
        .iter()
        .find(|(p, _)| lists(p, "underlay/") || lists(p, "overlay/"))
        .unwrap_or(&(scene, Origin::Scene));
    let mut underlays = vec![Underlay::default(); 256];
    let mut overlays = vec![Overlay::default(); 256];
    let decoded = family(floors, "underlay/", |d| {
        Underlay::decode(d).map_err(|e| e.to_string())
    })?;
    for (id, underlay) in decoded {
        *underlays
            .get_mut(id as usize)
            .ok_or_else(|| format!("Minimap underlay {id}"))? = underlay;
    }
    let decoded = family(floors, "overlay/", |d| {
        Overlay::decode(d).map_err(|e| e.to_string())
    })?;
    for (id, overlay) in decoded {
        *overlays
            .get_mut(id as usize)
            .ok_or_else(|| format!("Minimap overlay {id}"))? = overlay;
    }
    Ok(Tables {
        assets,
        mapscenes,
        underlays,
        overlays,
        origins: [assets_origin, scenes_origin, floors_origin],
    })
}

/// A location type by ID: the scene's (from the starting pack or, for a
/// streamed region, from the cache), else one [`retain`] kept.
pub fn location<'a>(
    id: u32,
    scene: &'a BTreeMap<u32, LocationType>,
    retained: &'a BTreeMap<u32, LocationType>,
) -> Option<&'a LocationType> {
    scene.get(&id).or_else(|| retained.get(&id))
}

/// The morph children of `parents` that neither `scene` nor `retained`
/// defines. A scene streamed from the cache brings every child with it; a
/// prepared pack written before the packers added morph children does not,
/// and those are read from the cache when one is open.
pub fn missing_children<'a>(
    parents: impl IntoIterator<Item = &'a LocationType>,
    scene: &BTreeMap<u32, LocationType>,
    retained: &BTreeMap<u32, LocationType>,
) -> std::collections::BTreeSet<u32> {
    parents
        .into_iter()
        .filter_map(|d| d.children.as_ref())
        .flatten()
        .filter_map(|&id| u32::try_from(id).ok())
        .filter(|id| location(*id, scene, retained).is_none())
        .collect()
}

/// A location type from the cache-backed pack.
pub fn cached_location(cache: &Pack, id: u32) -> Result<LocationType, String> {
    LocationType::decode(cache.get(&format!("loc/{id}"))?).map_err(|e| format!("loc/{id}: {e}"))
}

/// Keeps the scene's definitions of the map-function locations `ids` and of
/// their morph children, which the icon pass looks up after the scene that
/// placed them may have been replaced. Definitions already kept stay.
pub fn retain(
    retained: &mut BTreeMap<u32, LocationType>,
    scene: &BTreeMap<u32, LocationType>,
    ids: impl IntoIterator<Item = u32>,
) {
    for id in ids {
        if let Some(definition) = scene.get(&id) {
            if let Some(children) = &definition.children {
                for child in children.iter().filter_map(|id| u32::try_from(*id).ok()) {
                    if let Some(variant) = scene.get(&child) {
                        retained.entry(child).or_insert_with(|| variant.clone());
                    }
                }
            }
            retained.entry(id).or_insert_with(|| definition.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Not transcribed from the Java: these establish the choice of source.
    //! `pinned_tables_are_the_pinned_cache` regenerates the tables from
    //! `assets/cache` and is skipped without it.
    use super::*;
    use crate::cache_pack::Builder;

    fn file_pack(source: &str, entries: &[(&str, &[u8])]) -> Pack {
        let mut builder = Builder::default();
        for (name, data) in entries {
            builder.add((*name).to_owned(), data);
        }
        Pack::decode(builder.finish(source).unwrap()).unwrap()
    }
    fn cache_pack(entries: &[(&str, &[u8])]) -> Pack {
        let entries: Vec<(String, Vec<u8>)> = entries
            .iter()
            .map(|(name, data)| ((*name).to_owned(), data.to_vec()))
            .collect();
        // Listed names resolve on demand, as the cache pack's do.
        let lookup = entries.clone();
        Pack::lazy(
            "cache".into(),
            entries.iter().map(|(name, _)| name.clone()),
            vec![],
            Box::new(move |name| {
                lookup
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, d)| d.clone())
                    .ok_or_else(|| format!("{name} unreadable"))
            }),
        )
    }
    // MsiType: opcode 1 sprite, 2 tint, 3 stretch.
    const SCENE_A: &[u8] = &[1, 0, 7, 0];
    const SCENE_B: &[u8] = &[1, 0, 9, 2, 1, 2, 3, 3, 0];
    // FluType opcode 1 colour; FloType opcode 1 colour.
    const UNDERLAY: &[u8] = &[1, 0x12, 0x34, 0x56, 0];
    const OVERLAY: &[u8] = &[1, 0x65, 0x43, 0x21, 0];

    #[test]
    fn an_open_cache_is_read_before_the_scene_pack() {
        let cache = cache_pack(&[
            (ASSETS, br#"{"mapfunction":11,"compass":12}"#),
            ("mapscene/3", SCENE_B),
            ("underlay/2", UNDERLAY),
            ("overlay/5", OVERLAY),
        ]);
        let scene = file_pack(
            PINNED_SOURCE,
            &[
                (ASSETS, br#"{"mapfunction":1,"compass":2}"#),
                ("mapscene/3", SCENE_A),
                ("underlay/2", &[0]),
                ("overlay/5", &[0]),
            ],
        );
        let t = tables(Some(&cache), &scene).unwrap();
        assert_eq!(t.origins, [Origin::Cache; 3]);
        assert_eq!(t.assets["mapfunction"], 11);
        assert_eq!(t.assets["compass"], 12);
        assert_eq!(t.mapscenes.len(), 1);
        assert_eq!(t.mapscenes[&3].sprite, 9);
        assert_eq!(t.mapscenes[&3].tint, 0x010203);
        assert!(t.mapscenes[&3].stretch);
        let from_cache = Underlay::decode(UNDERLAY).unwrap();
        assert_eq!(t.underlays[2].weighted_hue, from_cache.weighted_hue);
        assert_eq!(t.underlays[2].lightness, from_cache.lightness);
        assert_eq!(t.overlays[5].color, Overlay::decode(OVERLAY).unwrap().color);
        assert_eq!(t.underlays.len(), 256);
        assert_eq!(t.overlays.len(), 256);
    }

    #[test]
    fn without_a_cache_the_scene_pack_supplies_everything_it_holds() {
        let scene = file_pack(
            "a pack from another cache",
            &[
                (ASSETS, br#"{"mapfunction":1}"#),
                ("mapscene/0", SCENE_A),
                ("mapscene/4", SCENE_B),
                ("underlay/2", UNDERLAY),
            ],
        );
        let t = tables(None, &scene).unwrap();
        assert_eq!(t.origins, [Origin::Scene; 3]);
        assert_eq!(t.assets["mapfunction"], 1);
        assert_eq!(t.mapscenes.keys().copied().collect::<Vec<_>>(), [0, 4]);
        assert_eq!(t.mapscenes[&0].sprite, 7);
        // A cache pack without minimap entries (an older one) is passed over.
        let empty_cache = cache_pack(&[("loc/1", &[0])]);
        let t = tables(Some(&empty_cache), &scene).unwrap();
        assert_eq!(t.origins, [Origin::Scene; 3]);
    }

    #[test]
    fn a_pinned_pack_without_the_entries_takes_the_pinned_tables() {
        let scene = file_pack(
            PINNED_SOURCE,
            &[("underlay/1", UNDERLAY), ("sprite/0", &[])],
        );
        let t = tables(None, &scene).unwrap();
        assert_eq!(t.origins, [Origin::Pinned, Origin::Pinned, Origin::Scene]);
        assert_eq!(
            t.assets,
            PINNED_SPRITES
                .iter()
                .map(|&(n, id)| (n.to_owned(), id))
                .collect::<BTreeMap<_, _>>()
        );
        assert_eq!(t.mapscenes.len(), PINNED_MAPSCENES.len());
        // Each family separately: names from the pack, scenes pinned.
        let scene = file_pack(PINNED_SOURCE, &[(ASSETS, br#"{"compass":5}"#)]);
        let t = tables(None, &scene).unwrap();
        assert_eq!(t.origins, [Origin::Scene, Origin::Pinned, Origin::Scene]);
        assert_eq!(t.assets["compass"], 5);
    }

    #[test]
    fn a_pack_from_another_cache_without_the_entries_is_refused() {
        let scene = file_pack("Pinned 2009scape cache / 0000", &[("underlay/1", UNDERLAY)]);
        let error = tables(None, &scene).err().unwrap();
        assert!(error.contains(ASSETS), "{error}");
        let scene = file_pack("other", &[(ASSETS, b"{}")]);
        let error = tables(None, &scene).err().unwrap();
        assert!(error.contains("map scenes"), "{error}");
        // Malformed entries are reported, not panicked on.
        let scene = file_pack("other", &[(ASSETS, b"[")]);
        assert!(tables(None, &scene).is_err());
        let scene = file_pack("other", &[(ASSETS, b"{}"), ("mapscene/x", SCENE_A)]);
        assert!(tables(None, &scene).is_err());
        let scene = file_pack(
            "other",
            &[
                (ASSETS, b"{}"),
                ("mapscene/1", SCENE_A),
                ("underlay/256", UNDERLAY),
            ],
        );
        assert!(tables(None, &scene).is_err());
    }

    fn location(map_function: i32, children: Option<Vec<i32>>) -> LocationType {
        let mut out = LocationType::decode(&[0]).unwrap();
        out.map_function = map_function;
        out.children = children;
        out
    }

    #[test]
    fn streamed_definitions_are_kept_with_their_morph_children() {
        // The starting scene's types, then a streamed scene that replaced them.
        let mut retained = BTreeMap::from([(1, location(3, None))]);
        let streamed = BTreeMap::from([
            (10, location(-1, Some(vec![-1, 11, 12, 99]))),
            (11, location(-1, None)),
            (12, location(17, None)),
            (20, location(5, None)),
            (30, location(6, None)),
        ]);
        retain(&mut retained, &streamed, [10, 20, 404]);
        assert_eq!(
            retained.keys().copied().collect::<Vec<_>>(),
            [1, 10, 11, 12, 20]
        );
        assert_eq!(retained[&12].map_function, 17);
        // The scene's own type wins over a kept one; kept ones answer for
        // locations no longer in the scene.
        let older = BTreeMap::from([(20, location(9, None))]);
        let mut kept = older.clone();
        retain(&mut kept, &streamed, [20]);
        assert_eq!(kept[&20].map_function, 9, "kept definitions stay");
        assert_eq!(
            super::location(20, &streamed, &kept).unwrap().map_function,
            5
        );
        assert_eq!(
            super::location(1, &streamed, &retained)
                .unwrap()
                .map_function,
            3
        );
        assert!(super::location(404, &streamed, &retained).is_none());
    }

    #[test]
    fn children_a_prepared_pack_lacks_are_found_and_read_from_the_cache() {
        let scene = BTreeMap::from([
            (1, location(-1, Some(vec![-1, 2, 3]))),
            (2, location(-1, None)),
        ]);
        let retained = BTreeMap::from([(4, location(-1, Some(vec![3, 5, 2])))]);
        let missing = missing_children(scene.values().chain(retained.values()), &scene, &retained);
        assert_eq!(missing.into_iter().collect::<Vec<_>>(), [3, 5]);
        // LocType opcode 60 (map function) = 7.
        let cache = cache_pack(&[("loc/3", &[60, 0, 7, 0])]);
        assert_eq!(cached_location(&cache, 3).unwrap().map_function, 7);
        assert!(cached_location(&cache, 5).is_err());
    }

    #[test]
    fn pinned_tables_are_the_pinned_cache() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let lock: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("upstream.lock.json")).unwrap())
                .unwrap();
        assert_eq!(
            PINNED_SOURCE,
            format!(
                "Pinned 2009scape cache / {}",
                lock["server"]["commit"].as_str().unwrap()
            )
        );
        let cache = root.join("assets/cache");
        if !cache.is_dir() {
            eprintln!("skipped: no cache");
            return;
        }
        let source = crate::cache_pack::Source::open(&cache).unwrap();
        let sprites = source.archive(8).unwrap();
        let names: Vec<(&str, i32)> = SPRITE_NAMES
            .iter()
            .map(|&name| {
                (
                    name,
                    sprites.index.group_by_ascii_name(name).unwrap() as i32,
                )
            })
            .collect();
        let configs = source.archive(2).unwrap();
        let scenes: Vec<(u32, Vec<u8>)> = configs
            .group(34, [0; 4])
            .unwrap()
            .unwrap()
            .iter()
            .map(|(id, data)| (id, data.to_vec()))
            .collect();
        let pinned: Vec<(u32, Vec<u8>)> = PINNED_MAPSCENES
            .iter()
            .map(|&(id, data)| (id, data.to_vec()))
            .collect();
        if names != PINNED_SPRITES || scenes != pinned {
            let mut text = format!("pub const PINNED_SPRITES: [(&str, i32); 7] = {names:?};\n");
            text.push_str("pub const PINNED_MAPSCENES: &[(u32, &[u8])] = &[\n");
            for (id, data) in &scenes {
                text.push_str(&format!("    ({id}, &{data:?}),\n"));
            }
            text.push_str("];\n");
            panic!("the pinned tables differ from the cache:\n{text}");
        }
        // The cache-backed pack serves the same values.
        let lazy = crate::cache_pack::cache_pack_from(&source, "x").unwrap();
        let from_cache: BTreeMap<String, i32> =
            serde_json::from_slice(lazy.get(ASSETS).unwrap()).unwrap();
        for (name, id) in PINNED_SPRITES.iter().take(6) {
            assert_eq!(from_cache[*name], *id, "{name}");
        }
        for (id, data) in PINNED_MAPSCENES {
            assert_eq!(lazy.get(&format!("mapscene/{id}")).unwrap(), *data);
        }
    }
}
