//! The minimap's drift, player dots and text labels against the original.
//!
//! `fixtures/minimap/drift-dots-labels.jsonl` is written by
//! `cargo xtask minimap-oracle`: the original statements run in a JVM (see
//! that command's scope note for what is excerpted and what is an adapter).
//! These are JVM differentials unless a test says otherwise.
use rs09_client::{
    minimap::{
        Drift, LabelFont, Labels, View,
        labels::{Draw, layout},
        player_dot, username_key,
    },
    ui::font::Metrics,
    world_map::MapElementList,
};
use rs09_game_core::random::JavaRandom;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

fn root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cases(kind: &str) -> Vec<Value> {
    let text =
        std::fs::read_to_string(root().join("fixtures/minimap/drift-dots-labels.jsonl")).unwrap();
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["case"] == kind)
        .collect()
}

fn ints(value: &Value) -> Vec<i32> {
    serde_json::from_value(value.clone()).unwrap()
}

fn key(value: &Value) -> i64 {
    value.as_str().unwrap().parse().unwrap()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn oracle_sources_are_the_pinned_reference_files() {
    let provenance: Value = serde_json::from_slice(
        &std::fs::read(root().join("fixtures/minimap/drift-dots-labels.provenance.json")).unwrap(),
    )
    .unwrap();
    let mut reference = 0;
    for (path, expected) in provenance["sources"].as_object().unwrap() {
        if !path.starts_with("reference/") {
            continue;
        }
        let data: Vec<u8> = std::fs::read(root().join(path))
            .unwrap()
            .into_iter()
            .filter(|&b| b != b'\r')
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            expected.as_str().unwrap(),
            "{path}"
        );
        reference += 1;
    }
    // The six excerpted files, the unchanged classes and the two signlink files.
    assert!(reference >= 42, "{reference}");
}

fn drift(values: &[i32]) -> Drift {
    Drift {
        camera_offset: [values[0], values[1]],
        yaw_drift: values[2],
        angle_offset: values[3],
        zoom_offset: values[4],
        camera_step: [values[5], values[6]],
        yaw_step: values[7],
        angle_step: values[8],
        zoom_step: values[9],
        camera_timer: values[10],
        minimap_timer: values[11],
    }
}

#[test]
fn session_reset_and_per_cycle_drift_match_the_original_statements() {
    let cases = cases("drift");
    assert_eq!(cases.len(), 12);
    let (mut turned, mut changes) = (0, 0);
    for case in &cases {
        let mut random = JavaRandom::new(case["seed"].as_i64().unwrap());
        let mut state = drift(&ints(&case["start"]));
        if case["reset"] == true {
            assert_eq!(state, Drift::default(), "field initialisers");
            let yaw = state.reset(|| random.next_double());
            assert_eq!(yaw, case["yawTarget"].as_i64().unwrap() as i32);
            assert_eq!(state, drift(&ints(&case["afterReset"])));
        }
        let expected: BTreeMap<i64, Vec<i32>> = case["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| (entry[0].as_i64().unwrap(), ints(&entry[1])))
            .collect();
        let cycles = case["cycles"].as_i64().unwrap();
        let mut previous = state;
        for cycle in 1..=cycles {
            state.cycle(|| random.next_double());
            let moved = Drift {
                camera_timer: 0,
                minimap_timer: 0,
                ..state
            } != Drift {
                camera_timer: 0,
                minimap_timer: 0,
                ..previous
            };
            match expected.get(&cycle) {
                Some(values) => assert_eq!(state, drift(values), "cycle {cycle}"),
                None => assert!(!moved, "unexpected change at cycle {cycle}"),
            }
            assert_eq!(moved || cycle == cycles, expected.contains_key(&cycle));
            if state.angle_step != previous.angle_step || state.zoom_step != previous.zoom_step {
                turned += 1;
            }
            previous = state;
        }
        changes += expected.len();
    }
    // The corpus reaches the bounds, not only the free walk.
    assert!(turned >= 8 && changes > 800, "{turned} {changes}");
}

#[test]
fn floor_jitter_start_and_rebuild_steps_match_the_original_statements() {
    let start = &cases("floorStart")[0];
    let mut random = JavaRandom::new(start["seed"].as_i64().unwrap());
    assert_eq!(
        Drift::initial_floor(|| random.next_double()).to_vec(),
        ints(&start["values"])
    );
    let cases = cases("floorStep");
    assert_eq!(cases.len(), 6);
    for case in &cases {
        let mut random = JavaRandom::new(case["seed"].as_i64().unwrap());
        let start = ints(&case["start"]);
        let mut floor = [start[0], start[1]];
        let values = ints(&case["values"]);
        assert_eq!(values.len(), 800);
        for pair in values.chunks(2) {
            Drift::step_floor(&mut floor, || random.next_double());
            assert_eq!(floor.as_slice(), pair);
        }
    }
}

#[test]
fn username_key_is_the_original_decode_title_case_encode_round_trip() {
    let case = &cases("username")[0];
    let keys = case["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 600);
    let mut rejected = 0;
    for entry in keys {
        let expected = (!entry[1].is_null()).then(|| key(&entry[1]));
        assert_eq!(username_key(key(&entry[0])), expected, "{entry}");
        rejected += usize::from(expected.is_none());
    }
    assert!(rejected > 100 && rejected < 400, "{rejected}");
}

#[test]
fn player_dots_follow_the_original_friend_clan_team_priority() {
    let cases = cases("dots");
    assert_eq!(cases.len(), 250);
    let mut seen = [0usize; 6];
    for case in &cases {
        let friends: Vec<(i64, i32)> = case["friends"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (key(&f[0]), f[1].as_i64().unwrap() as i32))
            .collect();
        let clan: Vec<i64> = case["clan"].as_array().unwrap().iter().map(key).collect();
        let team = case["team"].as_i64().unwrap() as i32;
        let frames: Vec<usize> = case["players"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                player_dot(
                    username_key(key(&p[0])).unwrap(),
                    friends.iter().copied(),
                    clan.iter().copied(),
                    team,
                    p[1].as_i64().unwrap() as i32,
                )
            })
            .collect();
        let expected: Vec<usize> = ints(&case["frames"]).iter().map(|&f| f as usize).collect();
        assert_eq!(frames, expected, "{case}");
        for frame in frames {
            seen[frame] += 1;
        }
    }
    // Every outcome occurs, including a friend who is also a clan or team mate.
    assert!(
        seen[2] > 100 && seen[3] > 100 && seen[4] > 100 && seen[5] > 100,
        "{seen:?}"
    );
}

fn fonts() -> [LabelFont; 3] {
    let cases = cases("font");
    assert_eq!(cases.len(), 3);
    let fonts: Vec<LabelFont> = cases
        .iter()
        .map(|case| {
            let boxes = ints(&case["boxes"]);
            let font = LabelFont::new(
                Metrics::decode(&unhex(case["data"].as_str().unwrap())).unwrap(),
                boxes.chunks(4).map(|b| [b[0], b[1]]).collect(),
                boxes.chunks(4).map(|b| [b[2], b[3]]).collect(),
            )
            .unwrap();
            assert_eq!(
                font.metrics.line_height,
                case["lineHeight"].as_i64().unwrap() as i32
            );
            assert_eq!(
                font.padding.to_vec(),
                ints(&case["padding"]),
                "{}",
                case["name"]
            );
            font
        })
        .collect();
    fonts.try_into().unwrap()
}

#[test]
fn label_layout_matches_every_glyph_and_line_call_of_the_original() {
    let fonts = fonts();
    let lists: BTreeMap<String, MapElementList> = cases("list")
        .iter()
        .map(|case| {
            let files: Vec<Vec<u8>> = case["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| unhex(f.as_str().unwrap()))
                .collect();
            let list = MapElementList::decode(files.iter().map(Vec::as_slice)).unwrap();
            assert_eq!(i64::from(list.count()), case["count"].as_i64().unwrap());
            let shown = (0..list.count())
                .filter(|&k| list.is_minimap_label_visible(k))
                .count();
            assert_eq!(shown as i64, case["minimap"].as_i64().unwrap());
            (case["name"].as_str().unwrap().to_owned(), list)
        })
        .collect();
    // The pinned cache's one world map marks no label for the minimap.
    assert_eq!(lists["main"].count(), 211);
    let cases = cases("labels");
    assert_eq!(cases.len(), 210);
    let (mut glyphs, mut lines, mut translucent, mut empty) = (0, 0, 0, 0);
    for case in &cases {
        let n = ints(&case["args"]);
        let draws = layout(
            &lists[case["list"].as_str().unwrap()],
            &fonts,
            View {
                yaw: n[6],
                angle_offset: n[7],
                zoom_offset: n[8],
            },
            [n[0], n[1]],
            [n[2], n[3]],
            [n[4], n[5]],
        );
        let actual: Vec<Vec<i32>> = draws
            .iter()
            .map(|draw| match *draw {
                Draw::Glyph {
                    font,
                    glyph,
                    at,
                    size,
                    color,
                    alpha,
                } => {
                    glyphs += 1;
                    translucent += usize::from(alpha != 256);
                    vec![
                        font as i32,
                        i32::from(glyph),
                        at[0],
                        at[1],
                        size[0],
                        size[1],
                        color,
                        alpha,
                    ]
                }
                Draw::Line { at, width, color } => {
                    lines += 1;
                    vec![-1, at[0], at[1], width, color]
                }
            })
            .collect();
        let expected: Vec<Vec<i32>> = case["draws"].as_array().unwrap().iter().map(ints).collect();
        assert_eq!(actual, expected, "{} {:?}", case["list"], n);
        empty += usize::from(expected.is_empty());
    }
    // All twenty unforced cache cases draw nothing; the rest cover text,
    // rules and translucent glyphs.
    assert_eq!(empty, 20);
    assert!(glyphs > 20_000 && lines > 100 && translucent > 100);
}

/// A transcription test of `SceneGraph.checkPlaneChange` (2817-2824) with
/// `LoginManager.loadRegion`'s reset of `minimapRenderedPlane` (824) and
/// `SceneGraph.buildScene`'s jitter step (799-812); nothing original runs.
#[test]
fn raster_is_rebuilt_on_scene_rebuild_and_plane_change_only() {
    use rs09_client::minimap::{Redraw, redraw};
    let scene = (7, [3168, 3168]);
    // minimapRenderedPlane starts at -1: the first frame renders.
    assert_eq!(redraw(None, scene, 0), Redraw::Scene);
    // Same scene and plane: nothing, whatever locations changed meanwhile.
    assert_eq!(redraw(Some((scene, 0)), scene, 0), Redraw::Keep);
    // Player.plane != minimapRenderedPlane: renderMap alone, in both directions.
    assert_eq!(redraw(Some((scene, 0)), scene, 1), Redraw::Plane);
    assert_eq!(redraw(Some((scene, 1)), scene, 0), Redraw::Plane);
    // loadRegion sets minimapRenderedPlane to -1 after buildScene stepped the jitter.
    assert_eq!(redraw(Some((scene, 0)), (8, scene.1), 0), Redraw::Scene);
    assert_eq!(
        redraw(Some((scene, 2)), (7, [3176, 3168]), 2),
        Redraw::Scene
    );
    assert_eq!(redraw(Some((scene, 0)), (8, scene.1), 3), Redraw::Scene);
}

/// Not a differential: the pinned cache through this client's own readers.
#[test]
fn cache_labels_survive_the_pack_and_no_cache_label_is_drawn_on_the_minimap() {
    use rs09_client::{cache_pack, pack::Pack};
    let cache = root().join("assets/cache");
    let direct = Labels::from_cache(&cache).unwrap();
    let archive = cache_pack::Source::open(&cache)
        .unwrap()
        .archive(23)
        .unwrap();
    let mut builder = cache_pack::Builder::default();
    for (name, data) in
        Labels::pack_entries(rs09_client::minimap::labels::cache_group(&archive)).unwrap()
    {
        builder.add(name, &data);
    }
    let packed = Labels::from_pack(&Pack::decode(builder.finish("test").unwrap()).unwrap())
        .unwrap()
        .expect("label entries");
    let describe = |labels: &Labels| {
        labels
            .lists()
            .map(|(map, list)| (map.group.clone(), list.cloned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(describe(&direct), describe(&packed));
    assert_eq!(describe(&direct).len(), 1);
    // Lumbridge's scene (origin 3168,3168) reads the list of the "main" map.
    let list = direct.for_origin([3168, 3168]).expect("main labels");
    assert_eq!(list.count(), 211);
    assert!((0..list.count()).all(|k| !list.is_minimap_label_visible(k)));
    // A scene outside every map has the original's empty list.
    assert!(direct.for_origin([0, 0]).is_none());
    let empty = Pack::decode(cache_pack::Builder::default().finish("test").unwrap()).unwrap();
    assert!(Labels::from_pack(&empty).unwrap().is_none());
}
