//! The title screen's 3D scene against the pinned cache: the map groups it
//! reads, the camera paths its interface scripts define and the NPCs its
//! regions place. These read `assets/cache`; without it they pass without
//! establishing anything (and say so).
#![cfg(not(target_arch = "wasm32"))]
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_client::{
    npc,
    title_scene::{self, Central, Slots},
    ui::script::Script,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// The maps, the NPC types and the scripts.
fn archives() -> Option<[Archive; 3]> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cache");
    if !path.join("main_file_cache.dat2").exists() {
        eprintln!("assets/cache absent: the title sweep verified nothing");
        return None;
    }
    let limits = Limits::default();
    let store = Store::open(&path, StorageMode::File, limits).unwrap();
    Some([5, 18, 12].map(|id| Archive::open(store.clone(), id, None, limits).unwrap()))
}

fn title_regions(maps: &Archive) -> Vec<[i32; 2]> {
    let mut regions = Vec::new();
    for x in 0..256 {
        for y in 0..256 {
            if maps
                .index
                .group_by_ascii_name(&format!("n{x}_{y}"))
                .is_some()
            {
                regions.push([x, y]);
            }
        }
    }
    regions
}

/// The camera path points (absolute tiles) each path script writes with CS2
/// 5406, read off its constant arguments.
fn camera_paths(scripts: &Archive) -> BTreeMap<u32, Vec<[i32; 2]>> {
    let mut paths = BTreeMap::new();
    for group in &scripts.index.groups {
        let Ok(Some(files)) = scripts.group(group.id, [0; 4]) else {
            continue;
        };
        let Some(bytes) = files.get(0) else { continue };
        let Ok(script) = Script::decode(&bytes) else {
            continue;
        };
        let mut points = Vec::new();
        for (at, &op) in script.opcodes.iter().enumerate() {
            // path, point, position, height, look, look height, speed.
            if op != 5406 || at < 7 || script.opcodes[at - 7..at].iter().any(|&op| op != 0) {
                continue;
            }
            for coordinate in [script.int_operands[at - 5], script.int_operands[at - 3]] {
                points.push([coordinate >> 14 & 0x3fff, coordinate & 0x3fff]);
            }
        }
        if !points.is_empty() {
            paths.insert(group.id, points);
        }
    }
    paths
}

/// The pinned cache has an NPC file for 30 regions. Their terrain and
/// scenery are stored without a key (the original reads a title scene's
/// files with an all-zero key), except for one region whose scenery is
/// encrypted and whose NPC file is empty.
#[test]
fn thirty_regions_have_npc_files_and_unencrypted_maps() {
    let Some([maps, _, _]) = archives() else {
        return;
    };
    let regions = title_regions(&maps);
    assert_eq!(regions.len(), 30);
    let mut encrypted = Vec::new();
    for &[x, y] in &regions {
        let read = |prefix: &str| {
            let id = maps
                .index
                .group_by_ascii_name(&format!("{prefix}{x}_{y}"))?;
            Some(maps.file(id, 0, [0; 4]))
        };
        assert!(matches!(read("m"), Some(Ok(Some(_)))), "terrain of {x},{y}");
        assert!(matches!(read("n"), Some(Ok(Some(_)))), "NPCs of {x},{y}");
        if !matches!(read("l"), Some(Ok(Some(_)))) {
            encrypted.push([x, y]);
        }
    }
    assert_eq!(encrypted, [[25, 83]]);
    let empty = maps.index.group_by_ascii_name("n25_83").unwrap();
    assert!(maps.file(empty, 0, [0; 4]).unwrap().unwrap().is_empty());
}

/// Thirteen scripts define a camera path (script 1239 picks one of them by
/// number), and every point of every path lies in a region with an NPC
/// file: the camera never looks from a place the title scene leaves empty.
#[test]
fn the_thirteen_camera_paths_stay_inside_the_title_regions() {
    let Some([maps, _, scripts]) = archives() else {
        return;
    };
    let regions = title_regions(&maps);
    let paths = camera_paths(&scripts);
    assert_eq!(
        paths.keys().copied().collect::<Vec<_>>(),
        [
            1240, 1241, 1242, 1243, 1244, 1245, 1246, 1247, 1430, 1859, 1860, 1866, 1950
        ]
    );
    let mut visited = BTreeSet::new();
    for (script, points) in &paths {
        for point in points {
            let region = point.map(|n| n >> 6);
            assert!(regions.contains(&region), "script {script} point {point:?}");
            visited.insert(region);
        }
    }
    // The first point of script 1240, as the scene test's transcription
    // uses it.
    assert_eq!(paths[&1240][0], [2498, 2855]);
    // The paths' own points lie in 15 of the 30 regions; the others are
    // their neighbours, shown when a scene's 104 tiles reach into them.
    assert_eq!(
        visited.into_iter().collect::<Vec<_>>(),
        [
            [26, 82],
            [39, 44],
            [39, 51],
            [40, 44],
            [40, 51],
            [41, 51],
            [42, 83],
            [45, 58],
            [46, 50],
            [47, 155],
            [50, 53],
            [51, 50],
            [53, 54],
            [56, 152],
            [59, 44]
        ]
    );
}

/// The NPCs `decodeNpcFiles` places for a scene base, by region, through
/// the library's rules and the cache's own NPC types.
fn placed(maps: &Archive, npcs: &Archive, zone: [i32; 2]) -> (Vec<[i32; 2]>, usize, usize) {
    let mut central = Central::default();
    // The camera on the middle of the zone, with the origin still 0.
    let render = zone.map(|n| (n * 8 + 4) << 7);
    let selection = title_scene::select(render, central.origin, |x, y| {
        maps.index
            .group_by_ascii_name(&format!("n{x}_{y}"))
            .is_some()
    });
    assert_eq!(selection.zone, zone);
    let loaded = central.load(zone).unwrap();
    let mut slots = Slots::default();
    let (mut count, mut wanderers) = (0, 0);
    for [x, y] in selection.loaded() {
        let packed = (x << 8) + y;
        let id = maps
            .index
            .group_by_ascii_name(&format!("n{x}_{y}"))
            .unwrap();
        let data = maps.file(id, 0, [0; 4]).unwrap().unwrap();
        assert_eq!(data.len() % 4, 0, "n{x}_{y}");
        let slot = slots.slot(packed).unwrap();
        for spawn in title_scene::spawns(&data, packed, slot, loaded.origin) {
            let id = spawn.npc as u32;
            let raw = npcs.file(id >> 7, id & 127, [0; 4]).unwrap().unwrap();
            let definition = npc::Definition::decode(&raw).unwrap();
            assert!(title_scene::spawn_angle(definition.spawn_direction).is_some());
            if title_scene::spawn_placed(&spawn, definition.login_screen, definition.size) {
                count += 1;
                wanderers += usize::from(definition.login_screen & 2 > 0);
            }
        }
    }
    (selection.loaded().collect(), count, wanderers)
}

/// The four title scenes a live run of the client reported (the console
/// line "Game state 28: title scene around zone .. regions ..", then
/// ".. built, N NPCs"), recomputed from the cache: the same regions and
/// the same number of NPCs.
#[test]
fn live_title_scenes_hold_the_regions_and_npcs_the_cache_gives() {
    let Some([maps, npcs, _]) = archives() else {
        return;
    };
    for (zone, regions, count) in [
        ([338, 667], vec![[42, 83]], 35),
        ([414, 404], vec![[51, 49], [51, 50]], 24),
        ([450, 1219], vec![[56, 152]], 43),
        ([363, 467], vec![[45, 58]], 31),
    ] {
        let (loaded, placed, wanderers) = placed(&maps, &npcs, zone);
        assert_eq!(loaded, regions, "{zone:?}");
        assert_eq!(placed, count, "{zone:?}");
        // Some of each scene's NPCs wander.
        assert!(
            wanderers > 0 && wanderers <= placed,
            "{zone:?}: {wanderers}"
        );
    }
}
