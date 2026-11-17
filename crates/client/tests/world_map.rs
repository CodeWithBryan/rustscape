//! World map tests.
//!
//! The JVM expectations (the `fixture` constants) come from a probe that ran
//! the unchanged `rt4.WorldMap` and its dependencies over the pinned cache
//! with `WorldMap.randomLightOffset = -11`, `randomHueOffset = 5`,
//! `SceneGraph.hueJitter = -7`, `lightnessJitter = 5`, map-function jitter
//! `[3, -4, 7]`, map-scene jitter `[-6, 5, 9]`, the palette at brightness 0.7
//! and every player variable 0. Inputs are regenerated here from the same
//! linear congruential generator the probe used, and each constant is the
//! FNV-1a hash the probe printed for that section. The remaining tests are
//! transcription checks derived from the Java by hand.
use rs09_client::world_map::{
    Assets, Client, Config, Layer, LoadStep, Map, MapElement, MapElementList, MapList, Raster,
    WorldMap, WorldMapFont, breaks_to_spaces, equals_ignore_case, font::ALPHABET, latin1,
    multiply_lightness_grayscale, multiply_lightness_safe, pack_hsl, split_label,
    starts_with_ignore_case,
};
use std::collections::HashMap;

const BASIS: i32 = 0x811c9dc5u32 as i32;

fn fnv(hash: i32, value: i32) -> i32 {
    let mut h = hash as u32;
    for byte in value.to_be_bytes() {
        h = (h ^ u32::from(byte)).wrapping_mul(16777619);
    }
    h as i32
}

fn fnv_ints(values: impl IntoIterator<Item = i32>) -> i32 {
    values.into_iter().fold(BASIS, fnv)
}

fn fnv_bytes(values: impl IntoIterator<Item = i32>) -> i32 {
    let mut h = BASIS as u32;
    for value in values {
        h = (h ^ u32::from(value as u8)).wrapping_mul(16777619);
    }
    h as i32
}

/// The probe's generator: `state = state * 6364136223846793005 + 1442695040888963407`,
/// result `(state >>> 33) % bound`.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, bound: i32) -> i32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % bound as u64) as i32
    }
}

fn palette() -> Vec<u32> {
    rs09_client::surface::palette(0.7)
}

#[test]
fn overlay_shapes_and_highlight_circles_match_the_jvm_over_random_cases() {
    // fixture shapes 40000 -1567497719; fixture circles 4000 933829178
    let mut lcg = Lcg(99);
    let mut shapes = BASIS;
    for case in 0..40000 {
        let (bw, bh) = (24, 20);
        let mut pixels = vec![0u32; (bw * bh) as usize];
        let mut raster = Raster::new(&mut pixels, bw, bh).unwrap();
        let (left, top) = (lcg.next(8), lcg.next(8));
        let right = left + lcg.next(bw);
        let bottom = top + lcg.next(bh);
        raster.set_clip(left, top, right, bottom);
        let x = lcg.next(30) - 6;
        let y = lcg.next(26) - 6;
        let w = lcg.next(18);
        let h = lcg.next(18);
        let shape = lcg.next(13);
        let rotation = lcg.next(4);
        let fill = lcg.next(2) == 1;
        raster.fill_overlay_shape(
            0xaa0000 + case,
            x,
            rotation,
            0x00bb00 + case,
            shape,
            h,
            w,
            y,
            fill,
        );
        shapes = fnv(shapes, fnv_ints(pixels.iter().map(|&p| p as i32)));
    }
    assert_eq!(shapes, -1567497719);
    let mut circles = BASIS;
    for case in 0..4000i32 {
        let (bw, bh) = (40, 36);
        let mut pixels: Vec<u32> = (0..bw * bh)
            .map(|k: i32| (k.wrapping_mul(2654435).wrapping_add(case) & 0xffffff) as u32)
            .collect();
        let mut raster = Raster::new(&mut pixels, bw, bh).unwrap();
        let (left, top) = (lcg.next(10), lcg.next(10));
        let right = left + 1 + lcg.next(bw);
        let bottom = top + 1 + lcg.next(bh);
        raster.set_clip(left, top, right, bottom);
        let x = lcg.next(50) - 5;
        let y = lcg.next(46) - 5;
        let radius = lcg.next(17);
        let alpha = if lcg.next(8) == 0 {
            256
        } else if lcg.next(8) == 0 {
            0
        } else {
            lcg.next(256)
        };
        raster.fill_circle_alpha(x, y, radius, alpha);
        circles = fnv(circles, fnv_ints(pixels.iter().map(|&p| p as i32)));
    }
    assert_eq!(circles, 933829178);
}

fn shape(shape: i32, rotation: i32, fill: bool, clip: [i32; 4]) -> Vec<String> {
    let mut pixels = vec![b'#' as u32; 16];
    let mut raster = Raster::new(&mut pixels, 4, 4).unwrap();
    raster.set_clip(clip[0], clip[1], clip[2], clip[3]);
    raster.fill_overlay_shape(b'O' as i32, 0, rotation, b'.' as i32, shape, 4, 4, 0, fill);
    pixels
        .chunks(4)
        .map(|row| row.iter().map(|&p| p as u8 as char).collect())
        .collect()
}

#[test]
fn overlay_shapes_fill_the_original_halves() {
    let full = [0, 0, 4, 4];
    // Shape 1 is the diagonal half; each rotation turns it a quarter.
    assert_eq!(shape(1, 0, true, full), ["O...", "OO..", "OOO.", "OOOO"]);
    assert_eq!(shape(1, 1, true, full), ["OOOO", "OOO.", "OO..", "O..."]);
    assert_eq!(shape(1, 2, true, full), ["OOOO", ".OOO", "..OO", "...O"]);
    assert_eq!(shape(1, 3, true, full), ["...O", "..OO", ".OOO", "OOOO"]);
    // Shape 2 rotation 0: `row >> 1 >= col` with rows counted from the bottom.
    assert_eq!(shape(2, 0, true, full), ["OO..", "OO..", "O...", "O..."]);
    // Shape 6: halves split at w / 2 and h / 2, the middle column inclusive.
    assert_eq!(shape(6, 0, true, full), ["OOO.", "OOO.", "OOO.", "OOO."]);
    assert_eq!(shape(6, 1, true, full), ["OOOO", "OOOO", "OOOO", "...."]);
    // 9 is shape 1 turned once more, 10 shape 1 turned back, 11 shape 8 turned back.
    assert_eq!(shape(9, 0, true, full), shape(1, 1, true, full));
    assert_eq!(shape(10, 0, true, full), shape(1, 3, true, full));
    assert_eq!(shape(11, 1, true, full), shape(8, 0, true, full));
    assert_eq!(shape(8, 0, true, full), ["OOOO", "OOOO", "OOOO", ".OOO"]);
    // Without the underlay fill only the overlay half is written.
    assert_eq!(shape(1, 0, false, full), ["O###", "OO##", "OOO#", "OOOO"]);
    // The clip keeps the tile's own row and column numbering.
    assert_eq!(
        shape(1, 0, true, [1, 1, 3, 3]),
        ["####", "#O.#", "#OO#", "####"]
    );
    // Shapes 0 and 12 have no loop in the original.
    assert_eq!(shape(0, 0, true, full), ["####"; 4]);
    assert_eq!(shape(12, 2, true, full), ["####"; 4]);
}

#[test]
fn color_helpers_follow_color_utils() {
    assert_eq!(pack_hsl(128, 64, 100), 64 + 256 + 25600);
    // Bright colours lose saturation bits: 255 >> 4 = 15, 15 >> 5 = 0.
    assert_eq!(pack_hsl(250, 255, 0), 125);
    assert_eq!(pack_hsl(200, 255, 0), 100 + ((255 >> 2 >> 5) << 7));
    assert_eq!(multiply_lightness_safe(96, -1), 12345678);
    assert_eq!(multiply_lightness_safe(96, 0x1234), 0x1200 + 39);
    assert_eq!(multiply_lightness_safe(96, 0x1201), 0x1200 + 2);
    assert_eq!(multiply_lightness_grayscale(-2, 96), 12345678);
    assert_eq!(multiply_lightness_grayscale(-1, 200), 126);
    assert_eq!(multiply_lightness_grayscale(-1, 1), 2);
    assert_eq!(multiply_lightness_grayscale(0x12ff, 96), 0x1280 + 95);
}

#[test]
fn jag_string_helpers_fold_case_like_the_original() {
    assert!(equals_ignore_case(b"MAIN", b"main"));
    // 0xC9 (E acute) folds to 0xE9; 0xD7 (the multiplication sign) does not fold.
    assert!(equals_ignore_case(&[0xc9], &[0xe9]));
    assert!(!equals_ignore_case(&[0xd7], &[0xf7]));
    assert!(!equals_ignore_case(b"main", b"mai"));
    assert!(starts_with_ignore_case(b"Lumbridge Swamp", b"lumBRIDGE s"));
    assert!(!starts_with_ignore_case(b"Lum", b"Lumb"));
    assert_eq!(
        breaks_to_spaces(b"Kingdom of<br>Misthalin"),
        b"Kingdom of Misthalin"
    );
    assert_eq!(breaks_to_spaces(b"<br><br>x<b"), b"  x<b");
}

#[test]
fn labels_split_at_line_breaks_only() {
    let lines =
        |name: &[u8]| -> Vec<String> { split_label(name).iter().map(|l| latin1(l)).collect() };
    assert_eq!(lines(b"A<br>B C<br>D"), ["A<br>", "B C<br>", "D"]);
    assert_eq!(lines(b"x<lt>y"), ["x<lt>y"]);
    // An unclosed tag swallows the rest; a lone '>' is an ordinary character.
    assert_eq!(lines(b"a<b"), ["a"]);
    assert_eq!(lines(b"a>b"), ["a>b"]);
    assert_eq!(lines(b"a<<br>b"), ["a<br>", "b"]);
    assert_eq!(lines(b""), Vec::<String>::new());
    assert_eq!(lines(b"1<br>2<br>3<br>4<br>5<br>6<br>7").len(), 5);
}

fn details(
    group: &str,
    name: &str,
    origin: [u16; 2],
    background: i32,
    valid: u8,
    zoom: u8,
    chunks: &[[u16; 4]],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(group.bytes().chain([0]));
    out.extend(name.bytes().chain([0]));
    out.extend(origin[0].to_be_bytes());
    out.extend(origin[1].to_be_bytes());
    out.extend(background.to_be_bytes());
    out.extend([valid, zoom, chunks.len() as u8]);
    for chunk in chunks {
        for value in chunk {
            out.extend(value.to_be_bytes());
        }
    }
    out
}

fn label(name: &str, flags: u8, x: u16, y: u16, color: i32) -> Vec<u8> {
    let mut out: Vec<u8> = name.bytes().chain([0]).collect();
    out.push(flags);
    out.extend(x.to_be_bytes());
    out.extend(y.to_be_bytes());
    out.extend(color.to_be_bytes());
    out
}

#[test]
fn details_and_labels_decode_from_hand_made_buffers() {
    let data = details(
        "cave",
        "Deep Cave",
        [40, 150],
        0x112233,
        0,
        255,
        &[[2560, 9600, 2623, 9663], [2500, 9700, 2700, 9727]],
    );
    let map = Map::decode(&data).unwrap();
    assert_eq!(
        (map.group.as_str(), map.name.as_str()),
        ("cave", "Deep Cave")
    );
    assert_eq!([map.origin_x, map.origin_y], [40, 150]);
    assert_eq!(map.background_color, 0x112233);
    assert!(!map.valid);
    // 255 reads as no default zoom.
    assert_eq!(map.default_zoom, 0);
    // File order is displayMinX, displayMaxX (smallest y), displayMaxZ, displayMinZ.
    assert_eq!(map.chunks[1].min_x, 2500);
    assert_eq!(map.chunks[1].min_y, 9700);
    assert_eq!(map.chunks[1].max_x, 2700);
    assert_eq!(map.chunks[1].max_y, 9727);
    assert_eq!(
        [map.min_x, map.min_y, map.max_x, map.max_y],
        [2500, 9600, 2700, 9727]
    );
    // Inside the bounds but in neither rectangle.
    assert!(!map.contains_display_coordinate(9650, 2510));
    assert!(map.contains_display_coordinate(9663, 2623));
    assert!(map.contains_display_coordinate(9700, 2700));
    assert!(!map.contains_display_coordinate(9728, 2600));
    assert!(Map::decode(&data[..data.len() - 1]).is_err());
    let mut empty = Map::decode(&details("e", "", [0, 0], -1, 1, 37, &[])).unwrap();
    empty.compute_bounds();
    assert_eq!(
        [empty.min_x, empty.min_y, empty.max_x, empty.max_y],
        [12800, 12800, 0, 0]
    );

    let files = [
        label("Tiny<br>Town", 8 | 1, 3210, 3220, -1),
        label("Far", 16 | 8 | 4 | 2, 0xfff6, 40000, 0x00ff00),
    ];
    let labels = MapElementList::decode(files.iter().map(Vec::as_slice)).unwrap();
    assert_eq!(labels.count(), 2);
    assert_eq!(labels.names[0], b"Tiny<br>Town");
    assert_eq!([labels.get_label_size(0), labels.get_label_size(1)], [1, 2]);
    assert!(labels.is_text_label(0) && labels.is_visible(0));
    assert!(!labels.is_minimap_label_visible(0) && labels.is_minimap_label_visible(1));
    assert!(!labels.is_visible(1));
    // Coordinates are Java shorts.
    assert_eq!([labels.coord_x[1], labels.coord_y[1]], [-10, -25536]);
    assert_eq!(labels.colors, [-1, 0x00ff00]);
    assert!(MapElementList::decode([&files[0][..5]]).is_err());
}

/// One frame of 2x2 white pixels in the sprite sheet layout.
fn sheet() -> Vec<u8> {
    vec![
        0, 1, 1, 1, 1, // flags, palette indices
        0xff, 0xff, 0xff, // palette entry 1
        0, 2, 0, 2, 1, // sheet width, height, palette size - 1
        0, 0, 0, 0, 0, 2, 0, 2, // x offset, y offset, width, height
        0, 1, // frames
    ]
}

/// A one-square map at (50, 50) with every kind of layer entry.
struct Tiny {
    groups: HashMap<String, Vec<Vec<u8>>>,
    files: HashMap<(String, String), Vec<u8>>,
    ready: bool,
}
impl Tiny {
    fn new() -> Self {
        let mut groups = HashMap::new();
        groups.insert(
            "details".to_owned(),
            vec![
                details(
                    "tiny",
                    "Tiny Isle",
                    [50, 50],
                    -1,
                    1,
                    37,
                    &[[3200, 3200, 3263, 3263]],
                ),
                details(
                    "cave",
                    "Deep Cave",
                    [40, 150],
                    0x112233,
                    0,
                    255,
                    &[[2560, 9600, 2623, 9663]],
                ),
                details(
                    "Twin",
                    "Twin Isle",
                    [50, 50],
                    -1,
                    1,
                    100,
                    &[[3200, 3200, 3263, 3263]],
                ),
            ],
        );
        groups.insert(
            "tiny_labels".to_owned(),
            vec![
                label("Tiny<br>Town", 8 | 1, 3210, 3220, -1),
                label("Hidden", 16 | 8, 3230, 3230, 0x00ff00),
                label("tiny dock", 8, 3202, 3203, -1),
            ],
        );
        let mut files = HashMap::new();
        let file = |name: &str| ("tiny".to_owned(), name.to_owned());
        // Underlay 1 everywhere except the eight westmost columns.
        let mut underlay = vec![0, 50, 50];
        for x in 0..64 {
            underlay.extend([u8::from(x >= 8); 64]);
        }
        files.insert(file("underlay"), underlay);
        // An 8x8 block at columns 8..16, rows 16..24: a shaped overlay, a
        // plain one, then nothing. A second square lies outside the map.
        let mut overlay = vec![1, 1, 2, 50, 50, 2, 1 << 2, 3, 0];
        overlay.extend([0; 62]);
        overlay.extend([1, 0, 0, 60, 60, 9, 9]);
        overlay.extend([0; 63]);
        files.insert(file("overlay"), overlay);
        let mut overlay2 = vec![1, 3, 3, 50, 50, 2, 3 << 2 | 1];
        overlay2.extend([0; 63]);
        files.insert(file("overlay2"), overlay2);
        // The block at columns 0..8, rows 0..8: a wall, a door, a map scene
        // and, at column 4 row 2, a map function. A skipped square follows.
        let mut locs = vec![1, 0, 0, 50, 50];
        let mut entries = vec![vec![0u8]; 64];
        entries[0] = vec![1, 1];
        entries[1] = vec![1, 5];
        entries[2] = vec![2, 0x11, 0x40, 0x01];
        entries[4 * 8 + 2] = vec![4, 0, 0, 8];
        entries[4 * 8 + 3] = vec![4, 0, 0, 10];
        locs.extend(entries.concat());
        locs.extend([0, 70, 70]);
        locs.extend([0; 4096]);
        files.insert(file("loc"), locs);
        Self {
            groups,
            files,
            ready: true,
        }
    }
}
impl Assets for Tiny {
    fn group_ready(&self, group: &str) -> bool {
        self.ready && self.group_valid(group)
    }
    fn group_valid(&self, group: &str) -> bool {
        group == "tiny" || self.groups.contains_key(group)
    }
    fn group_percentage(&self, _: &str) -> i32 {
        57
    }
    fn map_file(&self, group: &str, file: &str) -> Option<Vec<u8>> {
        self.files
            .get(&(group.to_owned(), file.to_owned()))
            .cloned()
    }
    fn group_files(&self, group: &str) -> Option<Vec<Vec<u8>>> {
        self.groups.get(group).cloned()
    }
    fn underlay(&self, id: i32) -> Option<Vec<u8>> {
        (id == 0).then(|| vec![1, 0x40, 0x80, 0x20, 0])
    }
    fn overlay(&self, id: i32) -> Option<Vec<u8>> {
        match id {
            1 => Some(vec![1, 0xff, 0x00, 0x00, 0]),
            2 => Some(vec![1, 0x00, 0x00, 0xff, 8, 0]),
            3 => Some(vec![1, 0xff, 0x00, 0xff, 0]),
            _ => None,
        }
    }
    fn overlay_capacity(&self) -> i32 {
        4
    }
    fn map_scene(&self, _: i32) -> Option<Vec<u8>> {
        None
    }
    fn location(&self, id: i32) -> Option<Vec<u8>> {
        match id {
            7 => Some(vec![60, 0, 0, 0]),
            // Morphs by varbit 3 into location 7, or into nothing.
            9 => Some(vec![77, 0, 3, 0xff, 0xff, 1, 0xff, 0xff, 0, 7, 0]),
            _ => None,
        }
    }
    fn sprite(&self, id: i32) -> Option<Vec<u8>> {
        (id == 1).then(sheet)
    }
    fn texture_repeating(&self, _: i32) -> bool {
        false
    }
    fn texture_average_color(&self, _: i32) -> i32 {
        0
    }
    fn varbit(&self, id: i32) -> i32 {
        i32::from(id == 3)
    }
    fn varp(&self, _: i32) -> i32 {
        0
    }
}

fn tiny_world(assets: &Tiny) -> WorldMap {
    let maps = MapList::init(assets, 1, [0; 3]).unwrap();
    WorldMap::new(
        maps,
        Config {
            palette: palette(),
            random_light_offset: 0,
            random_hue_offset: 0,
            map_scene_jitter: [0; 3],
            typeface: None,
        },
    )
    .unwrap()
}

/// Loads `tiny` with the player at an absolute tile and both random draws equal.
fn load(world: &mut WorldMap, assets: &Tiny, player: [i32; 2], draw: f64) -> (Vec<i32>, [i32; 3]) {
    let mut random = || draw;
    let mut client = Client {
        player,
        random: &mut random,
        jitter: [0, 0],
    };
    let mut trail = Vec::new();
    let mut effects = [0; 3];
    while world.load_percentage < 100 && trail.len() < 100 {
        let step = world.load(assets, &mut client).unwrap();
        effects[0] += i32::from(step.clear_scene);
        effects[1] += i32::from(step.ping);
        effects[2] += i32::from(step.reset_timer);
        trail.push(world.load_percentage);
    }
    (trail, effects)
}

fn render(world: &mut WorldMap, assets: &dyn Assets, target: &mut [u32], w: i32, h: i32) {
    let mut random = || 0.5;
    let mut client = Client {
        player: [0, 0],
        random: &mut random,
        jitter: [0, 0],
    };
    world.render(assets, &mut client, target, w, h).unwrap();
}

#[test]
fn map_list_queries_follow_the_original() {
    let assets = Tiny::new();
    let mut maps = MapList::init(&assets, 1, [0; 3]).unwrap();
    assert_eq!(maps.maps.len(), 3);
    assert_eq!(maps.get("cave").unwrap().name, "Deep Cave");
    // MapList.get is case-sensitive.
    assert!(maps.get("Cave").is_none());
    // The first valid map wins; the invalid cave is never a source.
    assert_eq!(
        maps.get_containing_source(3200, 3263).unwrap().group,
        "tiny"
    );
    assert!(maps.get_containing_source(2600, 9620).is_none());
    assert!(maps.get_containing_source(3264, 3200).is_none());
    let both: Vec<&str> = maps
        .find_maps_at_coordinate(3230, 3230)
        .iter()
        .map(|map| map.group.as_str())
        .collect();
    assert_eq!(both, ["tiny", "Twin"]);
    // Command 5215 compares with equalsIgnoreCase.
    assert!(maps.contains_at(3230, 3230, "TWIN"));
    assert!(!maps.contains_at(3230, 3230, "cave"));
    assert!(!maps.contains_at(2600, 9620, "cave"));
    // One map-function sprite, so one visibility flag.
    assert_eq!(maps.sprites.len(), 1);
    assert_eq!(maps.sprites[0].pixels, [0xffffff; 4]);
    assert!(!maps.is_map_visible(0));
    maps.toggle_map_visibility(0);
    assert!(maps.is_map_visible(0));
    maps.toggle_map_visibility(1);
    maps.toggle_map_visibility(-1);
    assert!(!maps.is_map_visible(1) && !maps.is_map_visible(-1));
    // adjustRgb clamps each channel to 1..=255 and leaves transparent pixels.
    let jittered = MapList::init(&assets, 1, [5, -300, 0]).unwrap();
    assert_eq!(jittered.sprites[0].pixels, [0xff01ff; 4]);
}

#[test]
fn loading_steps_layers_and_labels_of_a_hand_made_map() {
    let mut assets = Tiny::new();
    let mut world = tiny_world(&assets);
    // No map selected: load does nothing.
    let mut random = || 0.0;
    let mut client = Client {
        player: [0, 0],
        random: &mut random,
        jitter: [0, 0],
    };
    assert_eq!(
        world.load(&assets, &mut client).unwrap(),
        LoadStep::default()
    );
    assert_eq!(world.load_percentage, 0);
    world.set_map_by_name("nowhere");
    assert!(world.current_map().is_none());
    world.switch_map("tiny");
    assert_eq!(world.current_map_center(), [3200, 3200]);
    assert_eq!(world.current_map_size(), [63, 63]);
    assert_eq!(world.current_map_default_zoom(), 37);
    // While the group is not ready the percentage follows the download.
    assets.ready = false;
    assert_eq!(
        world.load(&assets, &mut client).unwrap(),
        LoadStep::default()
    );
    assert_eq!(world.load_percentage, 5);
    assets.ready = true;
    // (int) (0.99 * 10.0) - 5 = 4 on both axes; the player is at map (20, 53).
    let (trail, effects) = load(&mut world, &assets, [3220, 3210], 0.99);
    // The first call clears the scene and runs the bounds step too.
    assert_eq!(
        trail,
        [20, 30, 40, 50, 60, 70, 73, 76, 79, 82, 85, 88, 91, 100]
    );
    assert_eq!(effects, [1, 10, 13]);
    assert!(world.is_loaded());
    assert_eq!(
        [world.origin_x, world.origin_y, world.width, world.length],
        [3200, 3200, 64, 64]
    );
    assert_eq!([world.zoom, world.target_zoom], [3.0, 3.0]);
    assert_eq!([world.view_x, world.view_y], [24, 57]);
    assert_eq!(world.view_coord(), [3224, 3206]);
    assert_eq!(world.squares(), [1, 1]);
    // Overlay 2 carries opcode 8.
    assert_eq!(world.water_overlay_id(), 2);
    assert_eq!(world.overlay_color(0), 0);
    assert_ne!(world.overlay_color(2), 0);
    // Type 0 has no data and so the default colour 0, which is not "none".
    assert_ne!(world.overlay_color(1), 0);

    // Layers are indexed (63 - row) << 6 | column, rows counted from the south.
    let wall = world.square(Layer::Wall, 0, 0).unwrap();
    assert_eq!([wall[63 << 6], wall[62 << 6]], [1, 5]);
    assert_eq!(wall.iter().filter(|&&v| v != 0).count(), 2);
    let scenery = world.square(Layer::Scenery, 0, 0).unwrap();
    assert_eq!(scenery[61 << 6], 0x114001);
    let functions = world.square(Layer::MapFunction, 0, 0).unwrap();
    // Location 7 as stored, and location 9 resolved to 7 by its varbit.
    assert_eq!([functions[(61 << 6) + 4], functions[(60 << 6) + 4]], [8, 8]);
    // Map elements record the square's corner, not their tile.
    let corner = MapElement {
        id: 0,
        map_x: 0,
        map_y: 63,
    };
    assert_eq!(world.map_elements, [corner, corner]);
    let overlay = world.square(Layer::Overlay, 0, 0).unwrap();
    let shapes = world.square(Layer::OverlayShape, 0, 0).unwrap();
    assert_eq!([overlay[(47 << 6) + 8], shapes[(47 << 6) + 8]], [2, 4]);
    assert_eq!([overlay[(46 << 6) + 8], shapes[(46 << 6) + 8]], [3, 0]);
    assert_eq!(overlay.iter().filter(|&&v| v != 0).count(), 2);
    let second = world.square(Layer::SecondaryOverlay, 0, 0).unwrap();
    let second_shapes = world.square(Layer::SecondaryOverlayShape, 0, 0).unwrap();
    assert_eq!(
        [second[(39 << 6) + 24], second_shapes[(39 << 6) + 24]],
        [2, 13]
    );
    // Tiles without an underlay stay 0 in an allocated square.
    let blended = world.square(Layer::BlendedUnderlay, 0, 0).unwrap();
    assert_eq!(blended[7], 0);
    assert_ne!(blended[8], 0);
    assert_eq!(blended.iter().filter(|&&v| v == 0).count(), 8 * 64);

    let labels = world.labels.as_ref().unwrap();
    assert_eq!(labels.count(), 3);
    assert_eq!(world.get_first_visible_label(), 0);
    assert_eq!(world.label_text(0), "Tiny Town");
    assert_eq!(world.label_size(0), 1);
    // The hidden label is skipped by the iteration.
    assert_eq!(world.get_next_visible_label(), 2);
    assert_eq!(world.get_next_visible_label(), -1);
    assert_eq!(
        (world.label_text(-1), world.label_size(-1)),
        (String::new(), 0)
    );
    assert_eq!(world.find_label_by_prefix("tiny"), 0);
    assert_eq!(world.find_label_by_prefix("TINY D"), 2);
    assert_eq!(world.find_label_by_prefix(""), -1);
    assert_eq!(world.find_label_by_exact_name("Tiny Town"), 0);
    assert_eq!(world.find_label_by_exact_name("tiny town"), -1);
    assert_eq!(world.get_label_text_by_prefix("hid"), "Hidden");
    assert_eq!(world.get_label_text_by_prefix("zz"), "");

    // clear(true) remembers the group for restorePreviousMap; reset does not.
    world.clear(true);
    assert_eq!(world.previous_map_name.as_deref(), Some("tiny"));
    assert!(world.current_map().is_none() && world.labels.is_none());
    assert_eq!(world.load_percentage, 0);
    assert!(world.map_elements.is_empty());
    world.restore_previous_map();
    assert_eq!(world.current_map().unwrap().group, "tiny");
    assert!(world.previous_map_name.is_none());
    world.reset();
    assert!(world.current_map().is_none() && world.previous_map_name.is_none());
    assert_eq!(world.current_map_default_zoom(), -1);
    assert_eq!(
        [world.current_map_center(), world.current_map_size()],
        [[0, 0]; 2]
    );

    // A player outside the map starts the view at the map's own centre.
    world.switch_map("tiny");
    load(&mut world, &assets, [5000, 5000], 0.5);
    assert_eq!([world.view_x, world.view_y], [0, 63]);
}

#[test]
fn zoom_view_and_pan_follow_the_original() {
    let assets = Tiny::new();
    let mut world = tiny_world(&assets);
    world.switch_map("tiny");
    load(&mut world, &assets, [0, 0], 0.5);

    for (level, zoom) in [(37, 3.0), (50, 4.0), (75, 6.0), (100, 8.0), (200, 16.0)] {
        world.set_target_zoom(level);
        assert_eq!(world.target_zoom, zoom);
        assert_eq!(world.get_target_zoom(), level);
    }
    // Other levels change nothing, and any other zoom reads as 200.
    world.set_target_zoom(99);
    assert_eq!(world.target_zoom, 16.0);
    world.target_zoom = 5.0;
    assert_eq!(world.get_target_zoom(), 200);

    // Zoom eases by a thirtieth of itself per cycle, computed in double.
    world.zoom = 3.0;
    world.target_zoom = 4.0;
    world.update_zoom_and_scroll();
    assert_eq!(world.zoom, 3.1);
    for _ in 0..20 {
        world.update_zoom_and_scroll();
    }
    assert_eq!(world.zoom, 4.0);
    world.target_zoom = 3.0;
    world.update_zoom_and_scroll();
    assert_eq!(world.zoom, (4.0f64 - 4.0 / 30.0) as f32);

    // Panning moves a sixteenth of the distance, so it stops short of a
    // target ahead; a distance of exactly 2 is taken whole.
    world.zoom = 3.0;
    world.view_x = 10;
    world.view_y = 10;
    world.pan_target_x = 50;
    world.pan_target_y = 12;
    world.update_zoom_and_scroll();
    assert_eq!([world.view_x, world.view_y], [12, 12]);
    for _ in 0..40 {
        world.update_zoom_and_scroll();
    }
    assert_eq!([world.view_x, world.view_y], [35, 12]);
    assert_eq!([world.pan_target_x, world.pan_target_y], [-1, -1]);
    // Backwards the arithmetic shift keeps moving one tile until it arrives.
    world.pan_target_x = 30;
    world.pan_target_y = 12;
    for _ in 0..10 {
        world.update_zoom_and_scroll();
    }
    assert_eq!([world.view_x, world.pan_target_x], [30, -1]);

    // The view clamps to the map and cancels a pan when it does.
    world.pan_target_x = 7;
    world.pan_target_y = 7;
    world.set_view_x(-5);
    assert_eq!(
        [world.view_x, world.pan_target_x, world.pan_target_y],
        [0, -1, -1]
    );
    world.set_view_x(1000);
    assert_eq!(world.view_x, 64);
    world.set_view_y(-1);
    assert_eq!(world.view_y, 0);
    world.set_view_y(65);
    assert_eq!(world.view_y, 64);
    // setViewY leaves panTargetX alone.
    world.pan_target_x = 9;
    world.set_view_y(20);
    assert_eq!(
        [world.view_y, world.pan_target_x, world.pan_target_y],
        [20, 9, -1]
    );
    world.set_view_from_mouse_position(100, 30, 50, 60);
    assert_eq!(
        [world.view_x, world.view_y, world.pan_target_x],
        [32, 32, -1]
    );
    world.set_view_from_mouse_position(100, 90, -3, 60);
    assert_eq!([world.view_x, world.view_y], [0, 64]);

    // Pan targets keep a whole component's worth of tiles from each edge.
    world.zoom = 4.0;
    world.component = None;
    assert!(!world.pan_to_coords(3240, 3210));
    assert_eq!([world.pan_target_x, world.pan_target_y], [40, -1]);
    world.component = Some([64, 64]);
    assert!(world.pan_to_coords(3240, 3210));
    assert_eq!([world.pan_target_x, world.pan_target_y], [40, 48]);
    assert!(world.pan_to_coords(3203, 3260));
    assert_eq!([world.pan_target_x, world.pan_target_y], [16, 16]);
    assert!(world.pan_to_coords(3262, 3232));
    assert_eq!([world.pan_target_x, world.pan_target_y], [48, 31]);
    world.pan_to_label_by_prefix("tiny d");
    assert_eq!([world.pan_target_x, world.pan_target_y], [16, 48]);
    world.pan_to_label_by_exact_name("Hidden");
    assert_eq!([world.pan_target_x, world.pan_target_y], [30, 33]);
    world.pan_to_label_by_exact_name("hidden");
    assert_eq!([world.pan_target_x, world.pan_target_y], [30, 33]);

    // Mouse helpers of the map component.
    world.view_x = 20;
    world.view_y = 30;
    assert_eq!(world.tile_at([64, 64], [32, 32]), [3220, 3233]);
    assert_eq!(world.tile_at([64, 64], [40, 20]), [3224, 3239]);
    world.target_zoom = 4.0;
    world.drag([20, 20], [100, 100], [90, 110]);
    assert_eq!([world.view_x, world.view_y], [25, 15]);

    world.highlight_map_element(7);
    assert_eq!(
        [
            world.highlighted_map_function,
            world.highlight_pulse_count,
            world.highlight_pulse_counter
        ],
        [7, 3, 20]
    );
}

#[test]
fn renders_the_loading_box_then_tiles_walls_icons_and_background() {
    let assets = Tiny::new();
    let mut world = tiny_world(&assets);
    let (w, h) = (400, 300);
    let mut pixels = vec![0x777777u32; (w * h) as usize];
    let at = |pixels: &[u32], x: i32, y: i32| pixels[(y * w + x) as usize];
    let mut random = || 0.5;
    let mut client = Client {
        player: [0, 0],
        random: &mut random,
        jitter: [0, 0],
    };
    // No map: an empty bar, and the caller is told where "Loading..." goes.
    let rendered = world
        .render(&assets, &mut client, &mut pixels, w, h)
        .unwrap();
    assert_eq!(rendered.loading_text, Some([200, 132]));
    assert_eq!(rendered.step, LoadStep::default());
    assert_eq!([at(&pixels, 0, 0), at(&pixels, 47, 112)], [0, 0]);
    assert_eq!([at(&pixels, 48, 112), at(&pixels, 351, 145)], [0x8c1111; 2]);
    assert_eq!([at(&pixels, 49, 113), at(&pixels, 50, 114)], [0, 0]);
    world.switch_map("tiny");
    let rendered = world
        .render(&assets, &mut client, &mut pixels, w, h)
        .unwrap();
    assert!(rendered.step.clear_scene && rendered.loading_text.is_some());
    // 20%: sixty of the three hundred bar pixels.
    assert_eq!(world.load_percentage, 20);
    assert_eq!([at(&pixels, 50, 114), at(&pixels, 109, 143)], [0x8c1111; 2]);
    assert_eq!([at(&pixels, 110, 114), at(&pixels, 349, 143)], [0, 0]);
    let mut frames = 1;
    while world
        .render(&assets, &mut client, &mut pixels, w, h)
        .unwrap()
        .loading_text
        .is_some()
    {
        frames += 1;
    }
    // The frame that finishes loading already draws the map.
    assert_eq!(frames, 13);
    assert!(world.is_loaded());

    // 64x64 pixels at zoom 4 show map tiles 0..32 by 32..64, two pixels each.
    let (w, h) = (64, 64);
    let at = |pixels: &[u32], x: i32, y: i32| pixels[(y * w + x) as usize];
    let mut pixels = vec![0u32; (w * h) as usize];
    world.zoom = 4.0;
    world.view_x = 16;
    world.view_y = 48;
    render(&mut world, &assets, &mut pixels, w, h);
    assert_eq!(world.viewport_size(), [32, 32]);
    assert_eq!([world.viewport_x, world.viewport_y], [0, 32]);
    let blended = world.square(Layer::BlendedUnderlay, 0, 0).unwrap();
    // A wall on the west edge of the south-west tile, a door (red) above it.
    assert_eq!([at(&pixels, 0, 62), at(&pixels, 0, 63)], [0xcccccc; 2]);
    assert_eq!([at(&pixels, 0, 60), at(&pixels, 0, 61)], [0xcc0000; 2]);
    // No underlay there: the fill is 1, never the transparent 0.
    assert_eq!([at(&pixels, 1, 62), at(&pixels, 1, 60)], [1, 1]);
    // Underlay tiles take their blended colour.
    assert_eq!(at(&pixels, 40, 10), blended[(37 << 6) + 20] as u32);
    // The shaped overlay at tile (8, 47): shape 1 keeps the north-east pixel.
    let red = world.overlay_color(2) as u32;
    let under = blended[(47 << 6) + 8] as u32;
    assert_eq!(
        [
            at(&pixels, 16, 30),
            at(&pixels, 17, 30),
            at(&pixels, 16, 31),
            at(&pixels, 17, 31)
        ],
        [red, under, red, red]
    );
    // The plain overlay above it fills its tile.
    let blue = world.overlay_color(3) as u32;
    assert_eq!([at(&pixels, 16, 28), at(&pixels, 17, 29)], [blue; 2]);
    // The secondary overlay at tile (24, 39), shape 3 rotation 1 with no
    // primary under it: the overlay part, and black (0) beside it.
    assert_eq!([at(&pixels, 48, 15), at(&pixels, 49, 15)], [red, red]);
    assert_eq!([at(&pixels, 48, 14), at(&pixels, 49, 14)], [0, 0]);
    // The two map-function icons sit seven pixels up and left of their tiles.
    assert_eq!([at(&pixels, 1, 51), at(&pixels, 2, 52)], [0xffffff; 2]);
    assert_eq!([at(&pixels, 1, 49), at(&pixels, 2, 50)], [0xffffff; 2]);
    assert_ne!(at(&pixels, 3, 51), 0xffffff);
    world.maps.toggle_map_visibility(0);
    render(&mut world, &assets, &mut pixels, w, h);
    assert_ne!(at(&pixels, 1, 51), 0xffffff);
    world.maps.toggle_map_visibility(0);

    // West of the map the background is a four-tile water chequer.
    world.view_x = 0;
    render(&mut world, &assets, &mut pixels, w, h);
    assert_eq!(world.viewport_x, -16);
    let water = world.overlay_color(world.water_overlay_id() + 1) as u32;
    assert_eq!([at(&pixels, 0, 0), at(&pixels, 7, 7)], [water; 2]);
    assert_eq!([at(&pixels, 8, 0), at(&pixels, 0, 8)], [0x4b5368; 2]);
    assert_eq!(at(&pixels, 8, 8), water);

    // The overview shows the whole map; the frame marks the last viewport.
    let mut overview = vec![0u32; 64 * 64];
    world
        .render_overview(&assets, &mut overview, 64, 64, true)
        .unwrap();
    assert_eq!(world.overview_viewport(64, 64), [-16, 32, 32, 32]);
    // Left edge of the frame is clipped away; its top edge is white.
    assert_eq!([at(&overview, 0, 32), at(&overview, 15, 32)], [0xffffff; 2]);
    assert_eq!(at(&overview, 16, 32), blended[(32 << 6) + 16] as u32);
    assert!(world.overview_highlights(64, 64).is_none());
    world.highlight_map_element(0);
    world.highlight_pulse_counter = 16;
    assert_eq!(
        world.overview_highlights(64, 64),
        Some((100, vec![[0, 63]; 2]))
    );
    // A viewport narrower than one tile cannot be scaled.
    world.zoom = 16.0;
    assert!(
        world
            .render(&assets, &mut client, &mut pixels[..64], 8, 8)
            .is_err()
    );
}

/// The probe's `synthetic`: a glyph table of random blocks.
fn synthetic(size: i32, gray: bool) -> Vec<u8> {
    let mut lcg = Lcg((5000 + size * 2 + i32::from(gray)) as u64);
    let count = ALPHABET.chars().count();
    let mut data = vec![0u8; count * 9];
    for i in 0..count {
        let at = data.len();
        let w = 2 + lcg.next(size / 2 + 2);
        let h = 2 + lcg.next(size / 2 + 4);
        let x_offset = lcg.next(3);
        let rise = lcg.next(h + 3);
        let advance = w + 1 + lcg.next(3);
        data[i * 9..i * 9 + 9].copy_from_slice(&[
            (at / 16384) as u8,
            ((at / 128) & 0x7f) as u8,
            (at & 0x7f) as u8,
            w as u8,
            h as u8,
            x_offset as u8,
            rise as u8,
            advance as u8,
            (size + 3) as u8,
        ]);
        for _ in 0..w * h {
            data.push(if gray {
                lcg.next(256) as u8
            } else if lcg.next(3) == 0 {
                0
            } else {
                255
            });
        }
    }
    data
}

/// Squares of the probe's synthetic map: x, y and whether it comes as 8x8 blocks.
const SYNTH_SQUARES: [[i32; 3]; 7] = [
    [50, 50, 0],
    [51, 50, 1],
    [50, 51, 1],
    [51, 51, 0],
    [60, 60, 0],
    [61, 61, 1],
    [49, 50, 1],
];

fn synth_file(lcg: &mut Lcg, mut entry: impl FnMut(&mut Lcg, &mut Vec<u8>, bool)) -> Vec<u8> {
    let mut out = Vec::new();
    for [x, y, blocks] in SYNTH_SQUARES {
        let inside = (50..=51).contains(&x) && (50..=51).contains(&y);
        if blocks == 0 {
            out.extend([0, x as u8, y as u8]);
            for _ in 0..4096 {
                entry(lcg, &mut out, inside);
            }
        } else {
            for _ in 0..6 {
                out.push(1);
                out.push(lcg.next(8) as u8);
                out.push(lcg.next(8) as u8);
                out.extend([x as u8, y as u8]);
                for _ in 0..64 {
                    entry(lcg, &mut out, inside);
                }
            }
        }
    }
    out
}

/// The probe's synthetic underlay, overlay, overlay2 and loc files.
fn synth_files(lcg: &mut Lcg, locs: &[i32], scenes: &[i32]) -> [Vec<u8>; 4] {
    let underlay = synth_file(lcg, |lcg, out, _| {
        out.push(if lcg.next(5) == 0 {
            0
        } else {
            (1 + lcg.next(48)) as u8
        });
    });
    let overlay_entry = |lcg: &mut Lcg, out: &mut Vec<u8>, _: bool| {
        let id = if lcg.next(3) == 0 {
            1 + lcg.next(185)
        } else {
            0
        };
        out.push(id as u8);
        if id != 0 {
            let shape = lcg.next(12);
            let rotation = lcg.next(4);
            out.push((shape << 2 | rotation) as u8);
        }
    };
    let overlay = synth_file(lcg, overlay_entry);
    let overlay2 = synth_file(lcg, overlay_entry);
    let loc = synth_file(lcg, |lcg, out, inside| {
        let mut flags = if lcg.next(5) == 0 { 1 + lcg.next(7) } else { 0 };
        if !inside {
            flags &= !2;
        }
        out.push(flags as u8);
        if flags & 1 != 0 {
            out.push((1 + lcg.next(28)) as u8);
        }
        if flags & 2 != 0 {
            let scene = scenes[lcg.next(scenes.len() as i32) as usize];
            let angle = lcg.next(4);
            let across = 1 + lcg.next(3);
            let down = 1 + lcg.next(3);
            let packed = scene | angle << 14 | across << 16 | down << 20;
            out.extend(&packed.to_be_bytes()[1..]);
        }
        if flags & 4 != 0 {
            let id = locs[lcg.next(locs.len() as i32) as usize] + 1;
            out.extend(&id.to_be_bytes()[1..]);
        }
    });
    [underlay, overlay, overlay2, loc]
}

/// The location and map-scene ids a loaded map uses, ascending.
fn harvest(world: &WorldMap) -> [Vec<i32>; 2] {
    let [cols, rows] = world.squares();
    let mut locs = std::collections::BTreeSet::new();
    let mut scenes = std::collections::BTreeSet::new();
    for x in 0..cols {
        for y in 0..rows {
            for value in world.square(Layer::MapFunction, x, y).unwrap_or_default() {
                if value != 0 {
                    locs.insert(value - 1);
                }
            }
            for value in world.square(Layer::Scenery, x, y).unwrap_or_default() {
                if value & 0x3fff != 0 {
                    scenes.insert(value & 0x3fff);
                }
            }
        }
    }
    [locs.into_iter().collect(), scenes.into_iter().collect()]
}

/// The cache with the synthetic map in place of archive 23.
struct Synthetic<'a> {
    base: &'a rs09_client::world_map::cache::CacheAssets,
    files: [Vec<u8>; 4],
}
impl Assets for Synthetic<'_> {
    fn group_ready(&self, group: &str) -> bool {
        group == "synth"
    }
    fn group_valid(&self, group: &str) -> bool {
        group == "synth"
    }
    fn group_percentage(&self, _: &str) -> i32 {
        100
    }
    fn map_file(&self, group: &str, file: &str) -> Option<Vec<u8>> {
        let slot = ["underlay", "overlay", "overlay2", "loc"]
            .iter()
            .position(|name| *name == file)?;
        (group == "synth").then(|| self.files[slot].clone())
    }
    fn group_files(&self, group: &str) -> Option<Vec<Vec<u8>>> {
        (group == "details").then(|| {
            vec![details(
                "synth",
                "Synthetic",
                [51, 51],
                0x203040,
                1,
                100,
                &[[3200, 3200, 3327, 3327]],
            )]
        })
    }
    fn underlay(&self, id: i32) -> Option<Vec<u8>> {
        self.base.underlay(id)
    }
    fn overlay(&self, id: i32) -> Option<Vec<u8>> {
        self.base.overlay(id)
    }
    fn overlay_capacity(&self) -> i32 {
        self.base.overlay_capacity()
    }
    fn map_scene(&self, id: i32) -> Option<Vec<u8>> {
        self.base.map_scene(id)
    }
    fn location(&self, id: i32) -> Option<Vec<u8>> {
        self.base.location(id)
    }
    fn sprite(&self, id: i32) -> Option<Vec<u8>> {
        self.base.sprite(id)
    }
    fn texture_repeating(&self, id: i32) -> bool {
        self.base.texture_repeating(id)
    }
    fn texture_average_color(&self, id: i32) -> i32 {
        self.base.texture_average_color(id)
    }
    fn varbit(&self, id: i32) -> i32 {
        self.base.varbit(id)
    }
    fn varp(&self, id: i32) -> i32 {
        self.base.varp(id)
    }
}

/// Mirrors the probe's `set` and `render`, including the persistent
/// `renderBuffer` that is reallocated only when the size changes.
struct Session {
    world: WorldMap,
    assets: rs09_client::world_map::cache::CacheAssets,
    buffer: (i32, i32, Vec<u32>),
}
impl Session {
    fn set(&mut self, zoom: f32, target: f32, view: [i32; 2], pan: [i32; 2], component: [i32; 2]) {
        self.world.zoom = zoom;
        self.world.target_zoom = target;
        [self.world.view_x, self.world.view_y] = view;
        [self.world.pan_target_x, self.world.pan_target_y] = pan;
        self.world.component = (component[0] >= 0).then_some(component);
    }
    fn render(&mut self, w: i32, h: i32, labels: bool) -> i32 {
        if self.buffer.0 != w || self.buffer.1 != h {
            self.buffer = (w, h, vec![0; (w * h) as usize]);
        }
        let saved = self.world.labels.clone();
        if !labels {
            self.world.labels = Some(MapElementList::default());
        }
        render(&mut self.world, &self.assets, &mut self.buffer.2, w, h);
        self.world.labels = saved;
        fnv_ints(self.buffer.2.iter().map(|&p| p as i32))
    }
    fn overview(&mut self, w: i32, h: i32, frame: Option<bool>) -> i32 {
        let mut target = vec![0u32; (w * h) as usize];
        match frame {
            Some(white) => self
                .world
                .render_overview(&self.assets, &mut target, w, h, white)
                .unwrap(),
            None => {
                let sprite = self.world.overview(&self.assets, w, h).unwrap().unwrap();
                for (dst, src) in target.iter_mut().zip(&sprite.pixels) {
                    *dst = *src as u32;
                }
            }
        }
        fnv_ints(target.iter().map(|&p| p as i32))
    }
}

#[test]
fn the_main_map_of_the_pinned_cache_matches_the_jvm() {
    use rs09_client::world_map::cache::CacheAssets;
    let cache = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cache");
    if !cache.join("main_file_cache.dat2").exists() {
        eprintln!(
            "skipped: {} is absent, so nothing was compared",
            cache.display()
        );
        return;
    }
    let assets = CacheAssets::open(&cache).unwrap();
    // The details group lists every map: this revision has one.
    assert_eq!(assets.map_function_sprite(), Some(318));
    let maps = MapList::init(&assets, 318, [3, -4, 7]).unwrap();
    assert_eq!(maps.maps.len(), 1);
    assert_eq!(maps.sprites.len(), 98);
    let main = maps.maps[0].clone();
    assert_eq!(
        (main.group.as_str(), main.name.as_str()),
        ("main", "RuneScape surface")
    );
    assert_eq!(
        [main.origin_x, main.origin_y, main.default_zoom],
        [50, 50, 75]
    );
    assert_eq!((main.background_color, main.valid), (-1, true));
    assert_eq!(
        [main.min_x, main.min_y, main.max_x, main.max_y],
        [2048, 2496, 3903, 4159]
    );
    assert_eq!(main.chunks.len(), 1);
    assert_eq!(
        maps.get_containing_source(3222, 3222).unwrap().group,
        "main"
    );
    assert!(maps.get_containing_source(3222, 9000).is_none());

    let mut world = WorldMap::new(
        maps,
        Config {
            palette: palette(),
            random_light_offset: -11,
            random_hue_offset: 5,
            map_scene_jitter: [-6, 5, 9],
            typeface: None,
        },
    )
    .unwrap();
    world.switch_map("main");
    // The JVM's view after loading was (1150, 962) for a player at
    // (3203, 3198), which is Math.random draws of [0.0, 0.1) and [0.6, 0.7).
    let mut draws = [0.05, 0.65].into_iter();
    let mut random = || draws.next().unwrap();
    let mut client = Client {
        player: [3203, 3198],
        random: &mut random,
        jitter: [-7, 5],
    };
    let mut trail = Vec::new();
    let mut effects = [0; 3];
    while world.load_percentage < 100 && trail.len() < 100 {
        let step = world.load(&assets, &mut client).unwrap();
        effects[0] += i32::from(step.clear_scene);
        effects[1] += i32::from(step.ping);
        effects[2] += i32::from(step.reset_timer);
        trail.push(world.load_percentage);
    }
    // loaded 14 2048 2496 1856 1664 5 1 10 13 20 30 ... 100
    assert_eq!(
        trail,
        [20, 30, 40, 50, 60, 70, 73, 76, 79, 82, 85, 88, 91, 100]
    );
    assert_eq!(effects, [1, 10, 13]);
    assert_eq!(
        [world.origin_x, world.origin_y, world.width, world.length],
        [2048, 2496, 1856, 1664]
    );
    assert_eq!(world.water_overlay_id(), 5);
    assert_eq!([world.zoom, world.target_zoom], [6.0, 6.0]);
    assert_eq!([world.view_x, world.view_y], [1150, 962]);
    assert_eq!(world.squares(), [29, 26]);

    // fixture layers -1468076789: every square of all eight layers.
    let layers = [
        Layer::BlendedUnderlay,
        Layer::Overlay,
        Layer::OverlayShape,
        Layer::SecondaryOverlay,
        Layer::SecondaryOverlayShape,
        Layer::Wall,
        Layer::Scenery,
        Layer::MapFunction,
    ];
    let mut fx = BASIS;
    let mut allocated = [0; 8];
    for x in 0..29 {
        for y in 0..26 {
            for (slot, layer) in layers.into_iter().enumerate() {
                let bytes = !matches!(
                    layer,
                    Layer::BlendedUnderlay | Layer::Scenery | Layer::MapFunction
                );
                fx = fnv(
                    fx,
                    world.square(layer, x, y).map_or(0, |values| {
                        allocated[slot] += 1;
                        if bytes {
                            fnv_bytes(values)
                        } else {
                            fnv_ints(values)
                        }
                    }),
                );
            }
        }
    }
    assert_eq!(fx, -1468076789);
    // The number of squares the JVM allocated per layer.
    assert_eq!(allocated, [451, 532, 532, 44, 44, 255, 320, 254]);
    // fixture elements 1252 1755014198
    assert_eq!(world.map_elements.len(), 1252);
    let elements = world.map_elements.clone();
    assert_eq!(
        fnv_ints(elements.iter().flat_map(|e| [e.id, e.map_x, e.map_y])),
        1755014198
    );
    // fixture labels 211 230169167
    let labels = world.labels.clone().unwrap();
    assert_eq!(labels.count(), 211);
    let mut fx = BASIS;
    for i in 0..211 {
        for &byte in &labels.names[i] {
            fx = fnv(fx, i32::from(byte));
        }
        for value in [
            i32::from(labels.flags[i]),
            i32::from(labels.coord_x[i]),
            i32::from(labels.coord_y[i]),
            labels.colors[i],
        ] {
            fx = fnv(fx, value);
        }
    }
    assert_eq!(fx, 230169167);
    // 151 small, 45 medium and 15 large text labels, all listed, none coloured.
    let sizes = |size| {
        (0..211)
            .filter(|&i| labels.get_label_size(i) == size)
            .count()
    };
    assert_eq!([sizes(0), sizes(1), sizes(2)], [151, 45, 15]);
    assert!((0..211).all(|i| labels.is_text_label(i) && labels.is_visible(i)));
    assert!(labels.colors.iter().all(|&color| color == -1));
    assert_eq!(labels.names[0], b"Baxtorian<br>Falls");
    assert_eq!(world.find_label_by_exact_name("Lumbridge"), 102);
    assert_eq!([labels.coord_x[102], labels.coord_y[102]], [3239, 3234]);
    assert_eq!(world.find_label_by_exact_name("Varrock"), 143);
    assert_eq!(world.find_label_by_prefix("kingdom of m"), 29);
    assert_eq!(world.label_text(29), "Kingdom of Misthalin");
    assert_eq!(world.label_size(29), 2);
    assert_eq!(world.label_text(210), "Stealing Creation");
    // fixture overlaycolors 186 -646617312
    assert_eq!(
        fnv_ints((0..186).map(|i| world.overlay_color(i))),
        -646617312
    );

    let mut session = Session {
        world,
        assets,
        buffer: (0, 0, Vec::new()),
    };
    let (width, length) = (1856, 1664);
    let center = [50 * 64 - 2048, 2496 + 1664 - 50 * 64 - 1];
    let stops = [3.0f32, 4.0, 6.0, 8.0, 16.0];
    let mut lcg = Lcg(777);

    // fixture views 40 -1101051496: random views, zooms and sizes, no labels.
    let mut fx = BASIS;
    for i in 0..40 {
        let zoom = if i % 4 == 3 {
            3.0 + lcg.next(1300) as f32 / 100.0
        } else {
            stops[lcg.next(5) as usize]
        };
        let (cw, ch) = (40 + lcg.next(261), 40 + lcg.next(201));
        let view = [lcg.next(width + 1), lcg.next(length + 1)];
        session.set(zoom, zoom, view, [-1, -1], [cw, ch]);
        fx = fnv(fx, session.render(cw, ch, false));
    }
    assert_eq!(fx, -1101051496);

    // fixture highlight 45 -1006727742: 45 frames of a highlight pulse with
    // the overview's dots on every tenth.
    let target = elements[lcg.next(1252) as usize];
    session.world.highlight_map_element(target.id);
    session.set(
        4.0,
        4.0,
        [target.map_x + 30, target.map_y - 30],
        [-1, -1],
        [200, 150],
    );
    let mut fx = BASIS;
    for frame in 0..45 {
        fx = fnv(fx, session.render(200, 150, false));
        if frame % 10 == 4 {
            fx = fnv(fx, session.overview(90, 80, Some(false)));
        }
    }
    assert_eq!(fx, -1006727742);
    assert_eq!(
        [
            session.world.highlight_pulse_count,
            session.world.highlight_pulse_counter
        ],
        [1, 15]
    );
    session.world.highlight_pulse_count = 0;

    // fixture toggle 2 -462107705: one map function hidden, then shown.
    let target = elements[lcg.next(1252) as usize];
    session.world.maps.toggle_map_visibility(target.id);
    session.set(
        3.0,
        3.0,
        [target.map_x + 32, target.map_y - 32],
        [-1, -1],
        [240, 180],
    );
    let hidden = session.render(240, 180, false);
    session.world.maps.toggle_map_visibility(target.id);
    let shown = session.render(240, 180, false);
    assert_ne!(hidden, shown);
    assert_eq!(fnv(fnv(BASIS, hidden), shown), -462107705);

    // fixture labelrender0 10 -444840740 and labelrender1 10 469091887:
    // labels drawn with stamped, then blended, glyph tables both sides build.
    for (gray, expected) in [(false, -444840740), (true, 469091887)] {
        for size in [11, 12, 14, 17, 19, 22, 26, 30] {
            let font = WorldMapFont::from_data(synthetic(size, gray), gray).unwrap();
            assert!(session.world.set_font(size, Some(font)));
        }
        let mut fx = BASIS;
        for i in 0..10 {
            let zoom = stops[i % 5];
            let (view, size) = if i < 5 {
                (center, [320, 214])
            } else {
                ([lcg.next(width + 1), lcg.next(length + 1)], [260, 200])
            };
            session.set(zoom, zoom, view, [-1, -1], size);
            fx = fnv(fx, session.render(size[0], size[1], true));
        }
        assert_eq!(fx, expected);
    }
    for size in [11, 12, 14, 17, 19, 22, 26, 30] {
        session.world.set_font(size, None);
    }

    // fixture overview 3 -791286836
    let mut fx = fnv(BASIS, session.overview(171, 190, None));
    fx = fnv(fx, session.overview(64, 64, None));
    fx = fnv(fx, session.overview(150, 170, Some(true)));
    assert_eq!(fx, -791286836);

    // fixture logic 800 1505856805: 800 pan, zoom and label operations.
    let mut lcg = Lcg(4242);
    session.set(8.0, 6.0, center, [-1, -1], [320, 214]);
    let world = &mut session.world;
    let levels = [37, 50, 75, 100, 200, 0, 99];
    let mut fx = BASIS;
    for _ in 0..800 {
        match lcg.next(14) {
            0 => world.set_target_zoom(levels[lcg.next(7) as usize]),
            1..=4 => world.update_zoom_and_scroll(),
            5 => {
                let x = world.origin_x + lcg.next(width + 200) - 100;
                let y = world.origin_y + lcg.next(length + 200) - 100;
                world.pan_to_coords(x, y);
            }
            6 => world.set_view_x(lcg.next(width + 100) - 50),
            7 => world.set_view_y(lcg.next(length + 100) - 50),
            8 => {
                let cw = 100 + lcg.next(100);
                let ch = 100 + lcg.next(100);
                let my = lcg.next(ch + 20) - 10;
                let mx = lcg.next(cw + 20) - 10;
                world.set_view_from_mouse_position(cw, my, mx, ch);
            }
            9 => {
                let name = breaks_to_spaces(&labels.names[lcg.next(211) as usize]);
                let length = 1 + lcg.next(name.len() as i32) as usize;
                let take = if lcg.next(4) == 0 { name.len() } else { length };
                let mut prefix = name[..take].to_vec();
                for byte in &mut prefix {
                    if lcg.next(3) == 0 && byte.is_ascii_lowercase() {
                        *byte -= 32;
                    }
                }
                let prefix = latin1(&prefix);
                fx = fnv(fx, world.find_label_by_prefix(&prefix));
                fx = fnv(fx, world.find_label_by_exact_name(&prefix));
                for ch in world.get_label_text_by_prefix(&prefix).chars() {
                    fx = fnv(fx, ch as i32);
                }
                if lcg.next(2) == 0 {
                    world.pan_to_label_by_prefix(&prefix);
                } else {
                    world.pan_to_label_by_exact_name(&prefix);
                }
            }
            10 => fx = fnv(fx, world.get_first_visible_label()),
            11 => fx = fnv(fx, world.get_next_visible_label()),
            12 => fx = fnv(fx, world.get_target_zoom()),
            _ => {
                let cw = if lcg.next(3) == 0 {
                    -1
                } else {
                    100 + lcg.next(400)
                };
                let ch = 100 + lcg.next(300);
                world.component = (cw >= 0).then_some([cw, ch]);
            }
        }
        for value in [
            world.zoom.to_bits() as i32,
            world.target_zoom.to_bits() as i32,
            world.view_x,
            world.view_y,
            world.pan_target_x,
            world.pan_target_y,
        ] {
            fx = fnv(fx, value);
        }
    }
    assert_eq!(fx, 1505856805);

    // A two-by-two-square map the probe fed straight to readUnderlay,
    // readOverlay, readOverlay2 and readLocs: whole squares, 8x8 blocks,
    // squares outside the map, every wall code and overlay shape, and a
    // background colour. Its files come from the shared generator and from
    // the location and map-scene ids the main map uses.
    let Session {
        world: main,
        assets,
        ..
    } = session;
    // harvest 93 -1631989067 41 -551199938
    let [locs, scenes] = harvest(&main);
    assert_eq!([locs.len(), scenes.len()], [93, 41]);
    assert_eq!(fnv_ints(locs.iter().copied()), -1631989067);
    assert_eq!(fnv_ints(scenes.iter().copied()), -551199938);
    let mut lcg = Lcg(31337);
    let files = synth_files(&mut lcg, &locs, &scenes);
    // op synth 13953 -1004102266 18518 1473104991 18498 2102578575 23284 246737891
    assert_eq!(files.each_ref().map(Vec::len), [13953, 18518, 18498, 23284]);
    let world = {
        let synthetic = Synthetic {
            base: &assets,
            files,
        };
        let mut world = WorldMap::new(
            MapList::init(&synthetic, 318, [3, -4, 7]).unwrap(),
            Config {
                palette: palette(),
                random_light_offset: -11,
                random_hue_offset: 5,
                map_scene_jitter: [-6, 5, 9],
                typeface: None,
            },
        )
        .unwrap();
        world.switch_map("synth");
        let mut random = || 0.5;
        let mut client = Client {
            player: [0, 0],
            random: &mut random,
            jitter: [-7, 5],
        };
        while world.load_percentage < 100 {
            world.load(&synthetic, &mut client).unwrap();
        }
        world
    };
    assert_eq!(
        [world.origin_x, world.origin_y, world.width, world.length],
        [3200, 3200, 128, 128]
    );
    assert_eq!((world.zoom, [world.view_x, world.view_y]), (8.0, [64, 63]));
    // fixture synthlayers -565714758
    let mut fx = BASIS;
    for x in 0..2 {
        for y in 0..2 {
            for layer in layers {
                let bytes = !matches!(
                    layer,
                    Layer::BlendedUnderlay | Layer::Scenery | Layer::MapFunction
                );
                fx = fnv(
                    fx,
                    world.square(layer, x, y).map_or(0, |values| {
                        if bytes {
                            fnv_bytes(values)
                        } else {
                            fnv_ints(values)
                        }
                    }),
                );
            }
        }
    }
    assert_eq!(fx, -565714758);
    // fixture synthelements 1009 1011764564
    let elements = world.map_elements.clone();
    assert_eq!(elements.len(), 1009);
    assert_eq!(
        fnv_ints(elements.iter().flat_map(|e| [e.id, e.map_x, e.map_y])),
        1011764564
    );
    // fixture synthviews 32 -1218138715: thirty views, a highlight from the
    // twenty-first on, the plain overview and the framed one with its dots.
    let mut session = Session {
        world,
        assets,
        buffer: (0, 0, Vec::new()),
    };
    let mut fx = BASIS;
    for i in 0..30 {
        let zoom = if i % 4 == 3 {
            3.0 + lcg.next(1300) as f32 / 100.0
        } else {
            stops[lcg.next(5) as usize]
        };
        let (cw, ch) = (40 + lcg.next(261), 40 + lcg.next(201));
        let view = [lcg.next(129), lcg.next(129)];
        session.set(zoom, zoom, view, [-1, -1], [cw, ch]);
        if i == 20 {
            let id = elements[lcg.next(1009) as usize].id;
            session.world.highlight_map_element(id);
        }
        fx = fnv(fx, session.render(cw, ch, false));
    }
    fx = fnv(fx, session.overview(128, 128, None));
    fx = fnv(fx, session.overview(77, 53, Some(false)));
    assert_eq!(fx, -1218138715);
}
