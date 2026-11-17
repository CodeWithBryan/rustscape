//! Renders the world map from a cache directory to a PNG, or replays the log
//! of a JVM probe that drove the original classes and counts agreements.
//!
//! `cargo run -p rs09-client --example world_map -- [--cache DIR] [--map NAME]
//! [--zoom 37|50|75|100|200] [--center X,Y] [--size WxH] [--output PATH.png]
//! [--overview] [--font PATH.ttf] [--no-labels] [--highlight ID] [--list]`
//!
//! `--jvm DIR` reads `DIR/probe.txt`, `palette.bin` and `font_*.bin`.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("world_map: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use rs09_client::world_map::{
        Client, Config, MapElementList, MapList, Typeface, WorldMap, cache::CacheAssets,
    };
    use std::{collections::BTreeMap, path::Path};

    const WINDOWS_FONT: &str = r"C:\Windows\Fonts\arialbd.ttf";

    fn png(path: &str, pixels: &[u32], width: i32, height: i32) {
        let mut bytes = Vec::with_capacity(pixels.len() * 4);
        // export_png writes its rows bottom-up, as for a framebuffer capture.
        for row in pixels.chunks_exact(width as usize).rev() {
            for &pixel in row {
                bytes.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8, 255]);
            }
        }
        macroquad::texture::Image {
            bytes,
            width: width as u16,
            height: height as u16,
        }
        .export_png(path);
    }

    pub fn run() -> Result<(), String> {
        let mut args = std::env::args().skip(1);
        let mut options = BTreeMap::new();
        while let Some(arg) = args.next() {
            let name = arg
                .strip_prefix("--")
                .ok_or_else(|| format!("unexpected argument {arg}"))?
                .to_owned();
            let value = match name.as_str() {
                "overview" | "no-labels" | "list" => String::new(),
                _ => args
                    .next()
                    .ok_or_else(|| format!("--{name} needs a value"))?,
            };
            options.insert(name, value);
        }
        let cache = options
            .get("cache")
            .map_or("assets/cache", String::as_str)
            .to_owned();
        let assets = CacheAssets::open(Path::new(&cache))?;
        if let Some(dir) = options.get("jvm") {
            return super::replay::run(&assets, Path::new(dir));
        }
        if let Some(dir) = options.get("jvm-fonts") {
            let font = options.get("font").map_or(WINDOWS_FONT, String::as_str);
            let data = std::fs::read(font).map_err(|e| format!("{font}: {e}"))?;
            return super::replay::fonts(Path::new(dir), &Typeface::new(&data, None)?);
        }
        let sprite = assets
            .map_function_sprite()
            .ok_or("the cache has no mapfunction sprites")?;
        let maps = MapList::init(&assets, sprite, [0; 3])?;
        if options.contains_key("list") {
            for map in &maps.maps {
                println!(
                    "{} \"{}\" origin {},{} zoom {} x {}..={} y {}..={} valid {}",
                    map.group,
                    map.name,
                    map.origin_x,
                    map.origin_y,
                    map.default_zoom,
                    map.min_x,
                    map.max_x,
                    map.min_y,
                    map.max_y,
                    map.valid
                );
            }
            return Ok(());
        }
        let font = options.get("font").map_or(WINDOWS_FONT, String::as_str);
        let typeface = match std::fs::read(font) {
            Ok(data) if !options.contains_key("no-labels") => Some(Typeface::new(&data, None)?),
            _ => None,
        };
        if typeface.is_none() {
            eprintln!("world_map: no label font ({font}); labels are not drawn");
        }
        let mut world = WorldMap::new(
            maps,
            Config {
                palette: rs09_client::surface::palette(0.7),
                random_light_offset: 0,
                random_hue_offset: 0,
                map_scene_jitter: [0; 3],
                typeface,
            },
        )?;
        let name = options.get("map").map_or("main", String::as_str);
        world.switch_map(name);
        if world.current_map().is_none() {
            return Err(format!("no map named {name}; try --list"));
        }
        let center = match options.get("center") {
            Some(text) => {
                let (x, y) = text.split_once(',').ok_or("--center takes X,Y")?;
                Some([
                    x.parse::<i32>().map_err(|e| e.to_string())?,
                    y.parse::<i32>().map_err(|e| e.to_string())?,
                ])
            }
            None => None,
        };
        let (width, height) = match options.get("size") {
            Some(text) => {
                let (w, h) = text.split_once('x').ok_or("--size takes WxH")?;
                (
                    w.parse::<i32>().map_err(|e| e.to_string())?,
                    h.parse::<i32>().map_err(|e| e.to_string())?,
                )
            }
            None => (765, 503),
        };
        if width <= 0 || height <= 0 || width > 8192 || height > 8192 {
            return Err("--size must be within 1..=8192".into());
        }
        let mut random = || 0.5;
        let mut client = Client {
            // Outside every map, so the view starts at the map's own centre.
            player: center.unwrap_or([-1, -1]),
            random: &mut random,
            jitter: [0, 0],
        };
        let mut steps = 0;
        while world.load_percentage < 100 {
            world.load(&assets, &mut client)?;
            steps += 1;
            if steps > 1000 {
                return Err("the map did not finish loading".into());
            }
        }
        if let Some(zoom) = options.get("zoom") {
            let level = zoom.parse::<i32>().map_err(|e| e.to_string())?;
            if ![37, 50, 75, 100, 200].contains(&level) {
                return Err("--zoom takes 37, 50, 75, 100 or 200".into());
            }
            world.set_target_zoom(level);
            world.zoom = world.target_zoom;
        }
        if let Some([x, y]) = center {
            world.set_view_x(x - world.origin_x);
            world.set_view_y(world.origin_y + world.length - y - 1);
        }
        if let Some(id) = options.get("highlight") {
            world.highlight_map_element(id.parse().map_err(|_| "--highlight takes an id")?);
            world.highlight_pulse_counter = 10;
        }
        let mut pixels = vec![0u32; (width * height) as usize];
        world.render(&assets, &mut client, &mut pixels, width, height)?;
        let labels = world.labels.as_ref().map_or(0, MapElementList::count);
        println!(
            "{name}: {}x{} tiles from {},{}; zoom {}; view {:?}; {labels} labels; {} map functions; {steps} load steps",
            world.width,
            world.length,
            world.origin_x,
            world.origin_y,
            world.get_target_zoom(),
            world.view_coord(),
            world.map_elements.len(),
        );
        if options.contains_key("overview") {
            let (w, h) = (width.min(512), height.min(512));
            pixels = vec![0u32; (w * h) as usize];
            world.render_overview(&assets, &mut pixels, w, h, false)?;
            let output = options
                .get("output")
                .map_or("world-map-overview.png", String::as_str);
            png(output, &pixels, w, h);
            println!("wrote {output}");
        } else {
            let output = options
                .get("output")
                .map_or("world-map.png", String::as_str);
            png(output, &pixels, width, height);
            println!("wrote {output}");
        }
        Ok(())
    }
}

/// Replays a `WorldMapProbe` log against this crate.
#[cfg(not(target_arch = "wasm32"))]
mod replay {
    use rs09_client::world_map::{
        Assets, Client, Config, Layer, Map, MapElementList, MapList, Raster, WorldMap,
        WorldMapFont, cache::CacheAssets, latin1,
    };
    use std::{collections::BTreeMap, path::Path};

    fn fnv_ints(values: impl IntoIterator<Item = i32>) -> i32 {
        let mut h = 0x811c9dc5u32;
        for value in values {
            for byte in value.to_be_bytes() {
                h = (h ^ u32::from(byte)).wrapping_mul(16777619);
            }
        }
        h as i32
    }

    fn fnv_bytes(values: impl IntoIterator<Item = i32>) -> i32 {
        let mut h = 0x811c9dc5u32;
        for value in values {
            h = (h ^ u32::from(value as u8)).wrapping_mul(16777619);
        }
        h as i32
    }

    fn unhex(text: &str) -> Option<String> {
        match text {
            "null" => None,
            "-" => Some(String::new()),
            _ => Some(latin1(
                &(0..text.len() / 2)
                    .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap_or(b'?'))
                    .collect::<Vec<_>>(),
            )),
        }
    }

    #[derive(Default)]
    struct Tally(BTreeMap<String, [u64; 2]>);
    impl Tally {
        fn check(&mut self, kind: &str, ok: bool, detail: impl FnOnce() -> String) {
            let entry = self.0.entry(kind.to_owned()).or_default();
            entry[1] += 1;
            if ok {
                entry[0] += 1;
            } else if entry[1] - entry[0] <= 5 {
                eprintln!("MISMATCH {kind}: {}", detail());
            }
        }
        fn add(&mut self, kind: &str, matched: u64, total: u64) {
            let entry = self.0.entry(kind.to_owned()).or_default();
            entry[0] += matched;
            entry[1] += total;
        }
    }

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

    /// The probe's `synthetic`: a glyph table of random blocks from the shared generator.
    fn synthetic(size: i32, gray: bool) -> Vec<u8> {
        let mut lcg = Lcg((5000 + size * 2 + i32::from(gray)) as u64);
        let count = rs09_client::world_map::font::ALPHABET.chars().count();
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

    /// The cache with the synthetic map's four files in place of archive 23.
    struct Synthetic<'a> {
        base: &'a CacheAssets,
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
        fn group_files(&self, _: &str) -> Option<Vec<Vec<u8>>> {
            None
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

    fn state(world: &WorldMap) -> Vec<i32> {
        vec![
            world.zoom.to_bits() as i32,
            world.target_zoom.to_bits() as i32,
            world.view_x,
            world.view_y,
            world.pan_target_x,
            world.pan_target_y,
            world.load_percentage,
            world.viewport_width,
            world.viewport_height,
            world.viewport_x,
            world.viewport_y,
            world.highlight_pulse_count,
            world.highlight_pulse_counter,
            world.highlighted_map_function,
        ]
    }

    /// How close the `fontdue` glyph tables come to the JVM's AWT ones.
    pub fn fonts(dir: &Path, face: &rs09_client::world_map::Typeface) -> Result<(), String> {
        use rs09_client::world_map::font::ALPHABET;
        let space = ALPHABET.chars().position(|c| c == ' ').unwrap() * 9;
        for size in [11, 12, 14, 17, 19, 22, 26, 30] {
            let java =
                std::fs::read(dir.join(format!("font_{size}.bin"))).map_err(|e| e.to_string())?;
            let rust = WorldMapFont::from_source(true, &face.sized(size))?;
            let rust = rust.data();
            let count = ALPHABET.chars().count();
            let glyph = |data: &[u8], i: usize| {
                let head = &data[i * 9..i * 9 + 9];
                let at = head[0] as usize * 16384 + head[1] as usize * 128 + head[2] as usize;
                let len = head[3] as usize * head[4] as usize;
                (head[3..8].to_vec(), data[at..at + len].to_vec())
            };
            let mut same = [0usize; 4];
            for i in 0..count {
                let (jh, jp) = glyph(&java, i);
                let (rh, rp) = glyph(rust, i);
                same[0] += usize::from(jh[4] == rh[4]);
                same[1] += usize::from(jh[..2] == rh[..2]);
                same[2] += usize::from(jh[2..4] == rh[2..4]);
                same[3] += usize::from(jh[..2] == rh[..2] && jp == rp);
            }
            println!(
                "size {size}: line height {} vs {}; max ascent {} vs {}; canvas height {} vs {}; of {count} glyphs: advance {} box size {} box origin {} identical bitmap {}; bytes {} vs {}",
                java[8],
                rust[8],
                java[space + 6],
                rust[space + 6],
                java[space + 4],
                rust[space + 4],
                same[0],
                same[1],
                same[2],
                same[3],
                java.len(),
                rust.len(),
            );
        }
        Ok(())
    }

    pub fn run(assets: &CacheAssets, dir: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(dir.join("probe.txt")).map_err(|e| e.to_string())?;
        let lines: Vec<Vec<&str>> = text.lines().map(|l| l.split(' ').collect()).collect();
        let int = |token: &str| token.parse::<i32>().unwrap_or(i32::MIN);
        let mut tally = Tally::default();
        let inputs = lines
            .first()
            .filter(|l| l[0] == "inputs" && l.len() >= 14)
            .ok_or("probe.txt does not start with inputs")?;
        let input: Vec<i32> = inputs[1..].iter().map(|t| int(t)).collect();
        let palette: Vec<u32> = std::fs::read(dir.join("palette.bin"))
            .map_err(|e| e.to_string())?
            .chunks_exact(4)
            .skip(2)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let native = rs09_client::surface::palette(0.7);
        tally.add(
            "palette entry (surface::palette(0.7) vs Rasteriser.palette)",
            palette.iter().zip(&native).filter(|(a, b)| a == b).count() as u64,
            palette.len() as u64,
        );
        let maps = MapList::init(assets, input[10], [input[4], input[5], input[6]])?;
        tally.check(
            "mapfunction sprite count",
            maps.sprites.len() as i32 == input[11],
            || format!("{} vs {}", maps.sprites.len(), input[11]),
        );
        let mut world = WorldMap::new(
            maps,
            Config {
                palette,
                random_light_offset: input[0],
                random_hue_offset: input[1],
                map_scene_jitter: [input[7], input[8], input[9]],
                typeface: None,
            },
        )?;
        let jitter = [input[2], input[3]];
        let mut player = [0, 0];
        let mut randoms = [0.5f64; 2];
        let mut buffer: Option<(i32, i32, Vec<u32>)> = None;
        let mut map_index = 0usize;
        let mut chunk_index = 0usize;
        let mut element_index = 0usize;
        let mut label_index = 0i32;
        let mut squares = BTreeMap::<&str, u64>::new();
        let mut shapes = Lcg(99);
        let mut pending_state = false;
        let mut jvm_fonts = BTreeMap::<i32, WorldMapFont>::new();
        let mut load_buffer: Option<Vec<u32>> = None;
        let mut harvested = [Vec::new(), Vec::new()];
        let layer = |name: &str| match name {
            "blended" => Layer::BlendedUnderlay,
            "overlay" => Layer::Overlay,
            "overlayshape" => Layer::OverlayShape,
            "overlay2" => Layer::SecondaryOverlay,
            "overlay2shape" => Layer::SecondaryOverlayShape,
            "wall" => Layer::Wall,
            "scenery" => Layer::Scenery,
            _ => Layer::MapFunction,
        };
        let is_bytes = |layer: Layer| {
            !matches!(
                layer,
                Layer::BlendedUnderlay | Layer::Scenery | Layer::MapFunction
            )
        };
        let mut i = 1;
        while i < lines.len() {
            let line = &lines[i];
            i += 1;
            match line[0] {
                "map" => {
                    let map = world.maps.maps.get(map_index);
                    map_index += 1;
                    chunk_index = 0;
                    let ok = map.is_some_and(|m| {
                        Some(m.group.clone()) == unhex(line[1])
                            && Some(m.name.clone()) == unhex(line[2])
                            && [
                                m.origin_x,
                                m.origin_y,
                                m.background_color,
                                i32::from(m.valid),
                                m.default_zoom,
                                m.min_x,
                                m.min_y,
                                m.max_x,
                                m.max_y,
                            ]
                            .iter()
                            .zip(&line[3..12])
                            .all(|(a, b)| *a == int(b))
                    });
                    tally.check("map details", ok, || format!("{line:?} vs {map:?}"));
                }
                "mapchunk" => {
                    let chunk = world
                        .maps
                        .maps
                        .get(map_index.wrapping_sub(1))
                        .and_then(|m| m.chunks.get(chunk_index));
                    chunk_index += 1;
                    let ok = chunk.is_some_and(|c| {
                        [c.min_x, c.min_y, c.max_x, c.max_y]
                            .iter()
                            .zip(&line[1..5])
                            .all(|(a, b)| *a == int(b))
                    });
                    tally.check("map chunk", ok, || format!("{line:?} vs {chunk:?}"));
                }
                "q" => match line[1] {
                    "source" => {
                        let (x, y) = (int(line[2]), int(line[3]));
                        let source = world
                            .maps
                            .get_containing_source(x, y)
                            .map(|m| m.group.clone());
                        let all: Vec<String> = world
                            .maps
                            .find_maps_at_coordinate(x, y)
                            .iter()
                            .map(|m| m.group.clone())
                            .collect();
                        let expected: Vec<String> = line[5..]
                            .iter()
                            .map(|t| world.maps.maps[int(t) as usize].group.clone())
                            .collect();
                        tally.check(
                            "getContainingSource / containsDisplayCoordinate",
                            source == unhex(line[4]) && all == expected,
                            || format!("{line:?} vs {source:?} {all:?}"),
                        );
                    }
                    "visible" => {
                        let got = i32::from(world.maps.is_map_visible(int(line[2])));
                        tally.check("isMapVisible", got == int(line[3]), || format!("{line:?}"));
                    }
                    "find" => {
                        let prefix = unhex(line[2]).unwrap_or_default();
                        let got = (
                            world.find_label_by_prefix(&prefix),
                            world.find_label_by_exact_name(&prefix),
                            world.get_label_text_by_prefix(&prefix),
                        );
                        tally.check(
                            "findLabelByPrefix / findLabelByExactName / getLabelTextByPrefix",
                            got.0 == int(line[3])
                                && got.1 == int(line[4])
                                && Some(got.2.clone()) == unhex(line[5]),
                            || format!("{line:?} vs {got:?}"),
                        );
                    }
                    "first" => {
                        let got = world.get_first_visible_label();
                        tally.check("getFirstVisibleLabel", got == int(line[2]), || {
                            format!("{line:?} vs {got}")
                        });
                    }
                    "next" => {
                        let got = world.get_next_visible_label();
                        tally.check("getNextVisibleLabel", got == int(line[2]), || {
                            format!("{line:?} vs {got}")
                        });
                    }
                    "getzoom" => {
                        let got = world.get_target_zoom();
                        tally.check("getTargetZoom", got == int(line[2]), || {
                            format!("{line:?} vs {got}")
                        });
                    }
                    "visiblelabels" => {
                        let mut got = Vec::new();
                        let mut label = world.get_first_visible_label();
                        while label != -1 {
                            got.push(label);
                            label = world.get_next_visible_label();
                        }
                        let expected: Vec<i32> = line[2..].iter().map(|t| int(t)).collect();
                        tally.check("visible label order", got == expected, || {
                            format!("{} vs {} labels", got.len(), expected.len())
                        });
                    }
                    "previous" => {
                        tally.check(
                            "previousMapName",
                            world.previous_map_name == unhex(line[2]),
                            || format!("{line:?} vs {:?}", world.previous_map_name),
                        );
                    }
                    "current" => {
                        let got = world.current_map().map(|m| m.group.clone());
                        let mut ok = got == unhex(line[2]);
                        if line.len() > 3 {
                            ok &= world.previous_map_name == unhex(line[3]);
                        }
                        tally.check("getCurrentMap", ok, || format!("{line:?} vs {got:?}"));
                    }
                    other => return Err(format!("unknown query {other}")),
                },
                "op" => {
                    pending_state = true;
                    match line[1] {
                        "switch" => {
                            let name = unhex(line[2]).unwrap_or_default();
                            player = [int(line[3]), int(line[4])];
                            // The view the JVM reached tells which Math.random
                            // results it drew; feed the same ones.
                            let loaded = &lines[i];
                            let after = &lines[i + 1];
                            let origin = [int(loaded[2]), int(loaded[3])];
                            let length = int(loaded[5]);
                            let kx = int(after[3]) - (player[0] - origin[0]) + 5;
                            let ky = int(after[4]) - (origin[1] + length - player[1] - 1) + 5;
                            randoms = [0.5, 0.5];
                            if (0..10).contains(&kx) && (0..10).contains(&ky) {
                                randoms =
                                    [(f64::from(kx) + 0.5) / 10.0, (f64::from(ky) + 0.5) / 10.0];
                            }
                            world.switch_map(&name);
                            buffer = None;
                        }
                        "set" => {
                            world.zoom = f32::from_bits(int(line[2]) as u32);
                            world.target_zoom = f32::from_bits(int(line[3]) as u32);
                            world.view_x = int(line[4]);
                            world.view_y = int(line[5]);
                            world.pan_target_x = int(line[6]);
                            world.pan_target_y = int(line[7]);
                            world.component =
                                (int(line[8]) >= 0).then(|| [int(line[8]), int(line[9])]);
                        }
                        "render" => {
                            let (w, h) = (int(line[2]), int(line[3]));
                            if buffer.as_ref().is_none_or(|b| b.0 != w || b.1 != h) {
                                buffer = Some((w, h, vec![0; (w * h) as usize]));
                            }
                            let target = &mut buffer.as_mut().unwrap().2;
                            let saved = world.labels.clone();
                            if line[4] == "0" {
                                world.labels = Some(MapElementList::default());
                            }
                            let mut random = || 0.5;
                            let mut client = Client {
                                player,
                                random: &mut random,
                                jitter,
                            };
                            world.render(assets, &mut client, target, w, h)?;
                            world.labels = saved;
                            let ok = fnv_ints(target.iter().map(|&p| p as i32)) == int(line[5]);
                            let kind = if line[4] == "0" {
                                "render pixels (tiles, walls, map scenes, icons)"
                            } else {
                                "render pixels (same plus labels from the JVM glyph tables)"
                            };
                            let total = (w * h) as u64;
                            let mut matched = if ok { total } else { 0 };
                            if !ok && line[6] != "-" {
                                let java =
                                    std::fs::read(dir.join(line[6])).map_err(|e| e.to_string())?;
                                matched = java
                                    .chunks_exact(4)
                                    .skip(2)
                                    .zip(target.iter())
                                    .filter(|(a, b)| {
                                        u32::from_be_bytes([a[0], a[1], a[2], a[3]]) == **b
                                    })
                                    .count() as u64;
                                let mine = dir.join(format!("{}.rust.bin", line[6]));
                                let mut out = Vec::new();
                                out.extend(w.to_be_bytes());
                                out.extend(h.to_be_bytes());
                                for pixel in target.iter() {
                                    out.extend(pixel.to_be_bytes());
                                }
                                std::fs::write(mine, out).map_err(|e| e.to_string())?;
                            }
                            tally.add(kind, matched, total);
                            tally.check("render buffer", ok, || format!("line {i}: {line:?}"));
                        }
                        "overview" | "overviewfull" => {
                            let (w, h) = (int(line[2]), int(line[3]));
                            let mut target = vec![0u32; (w * h) as usize];
                            if line[1] == "overview" {
                                if let Some(sprite) = world.overview(assets, w, h)? {
                                    for (dst, src) in target.iter_mut().zip(&sprite.pixels) {
                                        *dst = *src as u32;
                                    }
                                }
                            } else {
                                world.render_overview(assets, &mut target, w, h, line[4] == "1")?;
                            }
                            let ok = fnv_ints(target.iter().map(|&p| p as i32)) == int(line[5]);
                            tally.add(
                                "overview pixels",
                                if ok { (w * h) as u64 } else { 0 },
                                (w * h) as u64,
                            );
                            tally.check("overview buffer", ok, || format!("line {i}: {line:?}"));
                        }
                        "synthfonts" => {
                            for size in [11, 12, 14, 17, 19, 22, 26, 30] {
                                let gray = line[2] == "1";
                                let font = WorldMapFont::from_data(synthetic(size, gray), gray)?;
                                world.set_font(size, Some(font));
                            }
                        }
                        "realfonts" => {
                            for (size, font) in &jvm_fonts {
                                world.set_font(*size, Some(font.clone()));
                            }
                        }
                        "synth" => {
                            let files = synth_files(&mut Lcg(31337), &harvested[0], &harvested[1]);
                            let got: Vec<i32> = files
                                .iter()
                                .flat_map(|f| {
                                    [f.len() as i32, fnv_bytes(f.iter().map(|&b| i32::from(b)))]
                                })
                                .collect();
                            let expected: Vec<i32> = line[2..10].iter().map(|t| int(t)).collect();
                            tally.check("synthetic map files", got == expected, || {
                                format!("{expected:?} vs {got:?}")
                            });
                            let mut details = b"synth\0Synthetic\0".to_vec();
                            details.extend([0, 51, 0, 51, 0, 0x20, 0x30, 0x40, 1, 100, 1]);
                            for value in [3200u16, 3200, 3327, 3327] {
                                details.extend(value.to_be_bytes());
                            }
                            world.clear(false);
                            world.maps.maps.push(Map::decode(&details)?);
                            world.set_map_by_name("synth");
                            let synthetic = Synthetic {
                                base: assets,
                                files,
                            };
                            let mut random = || 0.5;
                            let mut client = Client {
                                player: [0, 0],
                                random: &mut random,
                                jitter,
                            };
                            while world.load_percentage < 100 {
                                world.load(&synthetic, &mut client)?;
                            }
                            buffer = None;
                            element_index = 0;
                            squares.clear();
                        }
                        "switchonly" => {
                            world.switch_map(&unhex(line[2]).unwrap_or_default());
                            player = [int(line[3]), int(line[4])];
                            buffer = None;
                        }
                        "loadstep" => {
                            let mut random = || 0.5;
                            let mut client = Client {
                                player,
                                random: &mut random,
                                jitter,
                            };
                            world.load(assets, &mut client)?;
                        }
                        "loadrender" | "softrender" => {
                            let (w, h) = (int(line[2]), int(line[3]));
                            let target =
                                load_buffer.get_or_insert_with(|| vec![0; (w * h) as usize]);
                            let mut random = || 0.5;
                            let mut client = Client {
                                player,
                                random: &mut random,
                                jitter,
                            };
                            let rendered = world.render(assets, &mut client, target, w, h)?;
                            let hash = fnv_ints(target.iter().map(|&p| p as i32));
                            if line[1] == "loadrender" {
                                // The Java stops at the text it cannot draw.
                                let ok = rendered.loading_text.is_some() == (line[4] == "npe")
                                    && rendered
                                        .loading_text
                                        .is_none_or(|at| at == [w / 2, h / 2 - 18])
                                    && hash == int(line[5]);
                                tally.check("loading box frames (software branch)", ok, || {
                                    format!("line {i}: {line:?} vs {rendered:?} {hash}")
                                });
                            } else {
                                tally.check(
                                    "software-branch render at the origin",
                                    hash == int(line[4]),
                                    || format!("line {i}: {line:?} vs {hash}"),
                                );
                            }
                        }
                        "highlight" => world.highlight_map_element(int(line[2])),
                        "pulse" => {
                            world.highlight_pulse_count = int(line[2]);
                            world.highlight_pulse_counter = int(line[3]);
                        }
                        "toggle" => world.maps.toggle_map_visibility(int(line[2])),
                        "zoom" => world.set_target_zoom(int(line[2])),
                        "update" => world.update_zoom_and_scroll(),
                        "pan" => {
                            let completed = world.pan_to_coords(int(line[2]), int(line[3]));
                            tally.check(
                                "panToCoords without a component",
                                completed == (line[4] == "ok"),
                                || format!("{line:?}"),
                            );
                        }
                        "viewx" => world.set_view_x(int(line[2])),
                        "viewy" => world.set_view_y(int(line[2])),
                        "mouse" => world.set_view_from_mouse_position(
                            int(line[2]),
                            int(line[3]),
                            int(line[4]),
                            int(line[5]),
                        ),
                        "panprefix" => {
                            world.pan_to_label_by_prefix(&unhex(line[2]).unwrap_or_default())
                        }
                        "panexact" => {
                            world.pan_to_label_by_exact_name(&unhex(line[2]).unwrap_or_default())
                        }
                        "clear" => {
                            world.clear(line[2] == "1");
                            buffer = None;
                            load_buffer = None;
                        }
                        "restore" => world.restore_previous_map(),
                        "reset" => world.reset(),
                        other => return Err(format!("unknown op {other}")),
                    }
                }
                "loaded" => {
                    let mut random_calls = 0;
                    let mut random = || {
                        random_calls += 1;
                        randoms[(random_calls - 1usize).min(1)]
                    };
                    let mut client = Client {
                        player,
                        random: &mut random,
                        jitter,
                    };
                    let mut trail = Vec::new();
                    let mut effects = [0; 3];
                    while world.load_percentage < 100 && trail.len() < 1000 {
                        let step = world.load(assets, &mut client)?;
                        effects[0] += i32::from(step.clear_scene);
                        effects[1] += i32::from(step.ping);
                        effects[2] += i32::from(step.reset_timer);
                        trail.push(world.load_percentage);
                    }
                    let mut got = vec![
                        trail.len() as i32,
                        world.origin_x,
                        world.origin_y,
                        world.width,
                        world.length,
                        world.water_overlay_id(),
                    ];
                    got.extend(effects);
                    got.extend(trail);
                    let expected: Vec<i32> = line[1..].iter().map(|t| int(t)).collect();
                    tally.check(
                        "load steps, bounds, water overlay, clearScene/ping/resetTimer counts",
                        got == expected,
                        || format!("{expected:?} vs {got:?}"),
                    );
                    pending_state = true;
                    element_index = 0;
                    label_index = 0;
                    squares.clear();
                }
                "state" => {
                    if pending_state {
                        let mut got = state(&world);
                        let mut expected: Vec<i32> = line[1..].iter().map(|t| int(t)).collect();
                        // labelIterIndex is private here; the label queries cover it.
                        expected.truncate(14);
                        got.truncate(14);
                        tally.check("state after an operation", got == expected, || {
                            format!("line {i} after {:?}: {expected:?} vs {got:?}", lines[i - 2])
                        });
                    }
                    pending_state = false;
                }
                "overlaycolors" => {
                    let colors: Vec<i32> =
                        (0..int(line[1])).map(|c| world.overlay_color(c)).collect();
                    tally.check(
                        "overlayColors hash",
                        fnv_ints(colors) == int(line[2]),
                        || format!("{line:?}"),
                    );
                }
                "oc" => {
                    let got = world.overlay_color(int(line[1]));
                    tally.check("overlay colour", got == int(line[2]), || {
                        format!("{line:?} vs {got}")
                    });
                }
                "chunk" => {
                    let which = layer(line[1]);
                    *squares.entry(line[1]).or_default() += 1;
                    let square = world.square(which, int(line[2]), int(line[3]));
                    let hash = square.map(|values| {
                        if is_bytes(which) {
                            fnv_bytes(values)
                        } else {
                            fnv_ints(values)
                        }
                    });
                    let name = format!("{} squares (4096 values each)", line[1]);
                    tally.check(&name, hash == Some(int(line[4])), || {
                        format!("{line:?} vs {hash:?}")
                    });
                }
                "endchunks" => {
                    let [cols, rows] = world.squares();
                    for name in [
                        "blended",
                        "overlay",
                        "overlayshape",
                        "overlay2",
                        "overlay2shape",
                        "wall",
                        "scenery",
                        "function",
                    ] {
                        let mut allocated = 0;
                        for x in 0..cols {
                            for y in 0..rows {
                                allocated += u64::from(world.square(layer(name), x, y).is_some());
                            }
                        }
                        let expected = squares.get(name).copied().unwrap_or(0);
                        tally.check("allocated squares per layer", allocated == expected, || {
                            format!("{name}: {allocated} vs {expected}")
                        });
                    }
                }
                "element" => {
                    let got = world.map_elements.get(element_index);
                    element_index += 1;
                    let ok = got.is_some_and(|e| {
                        [e.id, e.map_x, e.map_y] == [int(line[1]), int(line[2]), int(line[3])]
                    });
                    tally.check("map elements", ok, || format!("{line:?} vs {got:?}"));
                }
                "endelements" => {
                    tally.check(
                        "map element count",
                        world.map_elements.len() as i32 == int(line[1]),
                        || format!("{line:?} vs {}", world.map_elements.len()),
                    );
                }
                "label" => {
                    let index = label_index;
                    label_index += 1;
                    let ok = world.labels.as_ref().is_some_and(|l| {
                        let at = index as usize;
                        at < l.names.len()
                            && Some(latin1(&l.names[at])) == unhex(line[1])
                            && i32::from(l.flags[at]) == int(line[2])
                            && i32::from(l.coord_x[at]) == int(line[3])
                            && i32::from(l.coord_y[at]) == int(line[4])
                            && l.colors[at] == int(line[5])
                    });
                    tally.check("labels", ok, || format!("{line:?}"));
                }
                "endlabels" => {
                    let count = world.labels.as_ref().map_or(-1, MapElementList::count);
                    tally.check("label count", count == int(line[1]), || {
                        format!("{line:?} vs {count}")
                    });
                }
                "font" => {
                    let size = int(line[1]);
                    let data = std::fs::read(dir.join(format!("font_{size}.bin")))
                        .map_err(|e| e.to_string())?;
                    let font = WorldMapFont::from_data(data, line[2] == "1")?;
                    tally.check(
                        "JVM glyph table metrics (getAscent, getLineHeight)",
                        font.get_ascent() == int(line[5]) && font.get_line_height() == int(line[6]),
                        || format!("{line:?}"),
                    );
                    jvm_fonts.insert(size, font.clone());
                    world.set_font(size, Some(font));
                }
                "shape" => {
                    let (bw, bh) = (24, 20);
                    let mut pixels = vec![0u32; (bw * bh) as usize];
                    let mut raster = Raster::new(&mut pixels, bw, bh)?;
                    let (left, top) = (shapes.next(8), shapes.next(8));
                    let right = left + shapes.next(bw);
                    let bottom = top + shapes.next(bh);
                    raster.set_clip(left, top, right, bottom);
                    let x = shapes.next(30) - 6;
                    let y = shapes.next(26) - 6;
                    let w = shapes.next(18);
                    let h = shapes.next(18);
                    let shape = shapes.next(13);
                    let rotation = shapes.next(4);
                    let fill = shapes.next(2);
                    let case = tally.0.get("fillOverlayShape buffers").map_or(0, |e| e[1]) as i32;
                    raster.fill_overlay_shape(
                        0xaa0000 + case,
                        x,
                        rotation,
                        0x00bb00 + case,
                        shape,
                        h,
                        w,
                        y,
                        fill == 1,
                    );
                    let got = fnv_ints(pixels.iter().map(|&p| p as i32));
                    tally.check("fillOverlayShape buffers", got == int(line[1]), || {
                        format!("case {case}: clip {left},{top},{right},{bottom} at {x},{y} {w}x{h} shape {shape} rotation {rotation} fill {fill}")
                    });
                }
                "circle" => {
                    let (bw, bh) = (40, 36);
                    let case = tally.0.get("fillCircleAlpha buffers").map_or(0, |e| e[1]) as i32;
                    let mut pixels: Vec<u32> = (0..bw * bh)
                        .map(|k: i32| {
                            (k.wrapping_mul(2654435).wrapping_add(case) & 0xffffff) as u32
                        })
                        .collect();
                    let mut raster = Raster::new(&mut pixels, bw, bh)?;
                    let (left, top) = (shapes.next(10), shapes.next(10));
                    let right = left + 1 + shapes.next(bw);
                    let bottom = top + 1 + shapes.next(bh);
                    raster.set_clip(left, top, right, bottom);
                    let x = shapes.next(50) - 5;
                    let y = shapes.next(46) - 5;
                    let radius = shapes.next(17);
                    let alpha = if shapes.next(8) == 0 {
                        256
                    } else if shapes.next(8) == 0 {
                        0
                    } else {
                        shapes.next(256)
                    };
                    raster.fill_circle_alpha(x, y, radius, alpha);
                    let got = fnv_ints(pixels.iter().map(|&p| p as i32));
                    tally.check("fillCircleAlpha buffers", got == int(line[1]), || {
                        format!("case {case}: clip {left},{top},{right},{bottom} at {x},{y} radius {radius} alpha {alpha}")
                    });
                }
                "harvest" => {
                    harvested = harvest(&world);
                    let got = [
                        harvested[0].len() as i32,
                        fnv_ints(harvested[0].iter().copied()),
                        harvested[1].len() as i32,
                        fnv_ints(harvested[1].iter().copied()),
                    ];
                    let expected = [int(line[1]), int(line[2]), int(line[3]), int(line[4])];
                    tally.check("location and map-scene ids in use", got == expected, || {
                        format!("{expected:?} vs {got:?}")
                    });
                }
                "end" | "fixture" => {}
                other => return Err(format!("unknown record {other}")),
            }
        }
        let mut failed = false;
        println!(
            "inputs: randomLightOffset {} randomHueOffset {} hueJitter {} lightnessJitter {}",
            input[0], input[1], input[2], input[3]
        );
        for (kind, [matched, total]) in &tally.0 {
            println!("{matched:>9} / {total:<9} {kind}");
            failed |= matched != total && !kind.starts_with("palette entry");
        }
        if failed {
            return Err("the replay disagrees with the JVM".into());
        }
        Ok(())
    }
}
