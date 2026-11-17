//! The RT4 world map: its archive 23 data, loading steps, pan/zoom/label logic
//! and software renderer, as one owned state object without any GPU types.
//!
//! Transcribed from `WorldMap`, `WorldMapFont`, `MapList`, `Map`, `MapChunk`,
//! `MapElementList`, `MapElement`, `MsiType.getSprite`, the `SoftwareRaster`,
//! `SoftwareSprite` and `SoftwareIndexedSprite` routines the map calls,
//! `ColorUtils`, and `Cs1ScriptRunner.renderWorldMapOverview`. Every method
//! names the Java method it mirrors; integer arithmetic wraps as Java's does.
//!
//! Scope: this is the map itself. The CS2 commands 5200..=5220, the component
//! with client code 1400/1401 and the mouse handling of `InterfaceList` are
//! callers and are not part of this module.
//!
//! Deliberate differences from the Java:
//! - Statics of other classes are explicit inputs: [`Config`] (palette, the two
//!   `WorldMap` colour offsets, the map-scene palette jitter, the typeface),
//!   [`Client`] (player tile, `Math.random`, `SceneGraph` jitter) and
//!   [`WorldMap::component`]. `Cs1ScriptRunner.worldMapViewportX/Y` and
//!   `mapHighlightPulseCounter` are fields here.
//! - Effects on the rest of the client are returned, not performed:
//!   [`LoadStep`] for `client.clearScene`, `ClientProt.ping` and
//!   `GameShell.resetTimer`, and [`WorldMap::reset`]'s doc for `setGameState(25)`.
//! - Rendering follows the `GlRenderer.enabled` branch, which draws the map
//!   into the off-screen `renderBuffer` at the origin; the caller presents
//!   that buffer where `GlRaster.drawPixels` would. The loading box, which
//!   that branch draws with `GlRaster`, uses the software raster arithmetic.
//!   The "Loading..." text and the `Cheat.displayFps` overlay are left to the
//!   caller. The overview is composed entirely in software.
//! - Label glyphs come from `fontdue` over caller-supplied TrueType data rather
//!   than from AWT's "Helvetica"; see [`font`]. Without a typeface no label is drawn.
//! - Definitions are resolved once per map load and kept until the next
//!   `clear`, where the Java consults soft caches. The water overlay
//!   (`FloType.waterOverlayId`) is the last overlay in ascending order with opcode 8.
//! - Bounds: truncated files, a map without area or a viewport narrower than
//!   one tile return `Err` where the Java throws, and so does an underlay or
//!   location definition with an opcode the shared decoders do not know, which
//!   the Java skips. Out-of-range overlay, map-function and pixel indices are
//!   ignored, a zero-sized overview component leaves the view alone, and a
//!   label of more than five lines keeps its first five.
//!
//! Preserved quirks include the secondary overlay painting black (0) beside a
//! shaped overlay that has no primary overlay under it, `setViewY` leaving
//! `panTargetX` alone, map elements recording their square's corner instead of
//! their tile, a pan easing by sixteenths so that it stops short of a target
//! ahead of it, morphing locations resolved with the variables of load time,
//! glyph blits that drop one extra row and column at the clip's far edges, and
//! `readLocs` skipping two bytes for a three-byte map scene in a square outside
//! the map, which loses its place in such a file exactly as the original does.
//!
//! Pixels are Java ints. Only the low 24 bits are colour: a blended glyph can
//! leave the top byte set, and 0 is an ordinary black here because the map is
//! presented opaque.
//!
//! `tests/world_map.rs` holds hashes observed from the unchanged Java classes
//! over the pinned cache for the layers, renders, overview and view logic.
pub mod font;
mod list;
pub mod raster;
mod render;

pub mod cache;

pub use font::{Face, Typeface, WorldMapFont};
pub use list::{
    Map, MapChunk, MapElement, MapElementList, MapList, breaks_to_spaces, equals_ignore_case,
    jag_bytes, latin1, map_functions, starts_with_ignore_case,
};
pub use raster::{Raster, Sprite};
pub use render::split_label;

use crate::{floor::Underlay, interaction::LocationType, minimap::Indexed, minimap::MapScene};
use rs09_protocol::buffer::Reader;
use std::collections::HashMap;

/// What the map reads from the cache and the running client.
///
/// Group and file names are the JagStrings the Java passes to `Js5`, which
/// lower-cases them before hashing.
pub trait Assets {
    /// `js5Archive23.isGroupReady(name)`.
    fn group_ready(&self, group: &str) -> bool;
    /// `js5Archive23.isGroupNameValid(name)`.
    fn group_valid(&self, group: &str) -> bool;
    /// `js5Archive23.getPercentageComplete(name)`, 0..=100.
    fn group_percentage(&self, group: &str) -> i32;
    /// `js5Archive23.fetchFile(file, group)`: `underlay`, `overlay`,
    /// `overlay2` or `loc` of a map group.
    fn map_file(&self, group: &str, file: &str) -> Option<Vec<u8>>;
    /// Every file of an archive 23 group in `getFileIds` order (`details`,
    /// `<map>_labels`), or None if the group does not exist.
    fn group_files(&self, group: &str) -> Option<Vec<Vec<u8>>>;
    /// `FluTypeList`: archive 2 group 1.
    fn underlay(&self, id: i32) -> Option<Vec<u8>>;
    /// `FloTypeList`: archive 2 group 4.
    fn overlay(&self, id: i32) -> Option<Vec<u8>>;
    /// `FloTypeList.capacity`: `getGroupCapacity(4)` of archive 2.
    fn overlay_capacity(&self) -> i32;
    /// `MsiTypeList`: archive 2 group 34.
    fn map_scene(&self, id: i32) -> Option<Vec<u8>>;
    /// `LocTypeList`: archive 16, group `id >>> 8`, file `id & 0xFF`.
    fn location(&self, id: i32) -> Option<Vec<u8>>;
    /// A sprite sheet: archive 8 group `id`, file 0. None while not ready.
    fn sprite(&self, id: i32) -> Option<Vec<u8>>;
    /// `Rasteriser.textureProvider.isTextureRepeating`.
    fn texture_repeating(&self, id: i32) -> bool;
    /// `Rasteriser.textureProvider.getAverageColor`.
    fn texture_average_color(&self, id: i32) -> i32;
    /// `VarpDomain.getVarbit`, for locations that morph.
    fn varbit(&self, id: i32) -> i32;
    /// `VarpDomain.activeVarps[id]`.
    fn varp(&self, id: i32) -> i32;
}

/// Client statics that stay fixed while the map exists.
pub struct Config {
    /// `Rasteriser.palette`: 65536 RGB values indexed by packed HSL.
    pub palette: Vec<u32>,
    /// `WorldMap.randomLightOffset`: `(int) (Math.random() * 33.0) - 16`.
    pub random_light_offset: i32,
    /// `WorldMap.randomHueOffset`: `(int) (Math.random() * 17.0) - 8`.
    pub random_hue_offset: i32,
    /// `MsiTypeList.redDelta`, `greenDelta`, `blueDelta`.
    pub map_scene_jitter: [i32; 3],
    /// The face `WorldMapFont` rasterises; None draws no labels.
    pub typeface: Option<Typeface>,
}

/// Client state the Java reads while loading.
pub struct Client<'a> {
    /// The local player's absolute tile:
    /// `(PlayerList.self.xFine >> 7) + Camera.originX` and the same for y.
    pub player: [i32; 2],
    /// `Math.random`.
    pub random: &'a mut dyn FnMut() -> f64,
    /// `SceneGraph.hueJitter` and `SceneGraph.lightnessJitter`.
    pub jitter: [i32; 2],
}

/// Client calls one `load` step asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadStep {
    /// `client.clearScene()`: the scene graph and collision maps are dropped.
    pub clear_scene: bool,
    /// `ClientProt.ping(true)`.
    pub ping: bool,
    /// `GameShell.resetTimer()`.
    pub reset_timer: bool,
}
impl LoadStep {
    fn merge(&mut self, other: Self) {
        self.clear_scene |= other.clear_scene;
        self.ping |= other.ping;
        self.reset_timer |= other.reset_timer;
    }
}

/// What one `render` call did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rendered {
    /// Set while loading: the loading box was drawn and the caller draws
    /// `LocalizedText.LOADINGDOTDOTDOT` with `Fonts.b12Full.renderCenter` at
    /// this centre x and baseline y, in 0xFFFFFF.
    pub loading_text: Option<[i32; 2]>,
    /// The load step this frame ran, if any.
    pub step: LoadStep,
}

/// A lazily allocated grid of 64x64 squares, `[x >> 6][y >> 6][4096]` in the Java.
struct Grid<T> {
    cols: i32,
    rows: i32,
    cells: Vec<Option<Box<[T]>>>,
}
impl<T: Copy + Default> Grid<T> {
    fn new(cols: i32, rows: i32) -> Self {
        Self {
            cols,
            rows,
            cells: (0..cols.max(0) as usize * rows.max(0) as usize)
                .map(|_| None)
                .collect(),
        }
    }
    fn slot(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && x < self.cols && y >= 0 && y < self.rows)
            .then(|| x as usize * self.rows as usize + y as usize)
    }
    fn chunk(&self, x: i32, y: i32) -> Option<&[T]> {
        self.cells[self.slot(x, y)?].as_deref()
    }
    fn chunk_mut(&mut self, x: i32, y: i32) -> Option<&mut [T]> {
        let slot = self.slot(x, y)?;
        Some(self.cells[slot].get_or_insert_with(|| vec![T::default(); 4096].into_boxed_slice()))
    }
    fn existing_mut(&mut self, x: i32, y: i32) -> Option<&mut [T]> {
        let slot = self.slot(x, y)?;
        self.cells[slot].as_deref_mut()
    }
}
impl<T> Default for Grid<T> {
    fn default() -> Self {
        Self {
            cols: 0,
            rows: 0,
            cells: Vec::new(),
        }
    }
}

/// The `LocType` fields the map reads.
#[derive(Clone, Debug)]
struct Loc {
    map_function: i32,
    /// The location this one currently morphs into; `Some(None)` is the
    /// original's null from `getMultiLoc`.
    morph: Option<Option<i32>>,
}

/// The `FloType` fields the map reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FloType {
    base_color: i32,
    secondary_color: i32,
    texture: i32,
    water: bool,
}
impl FloType {
    /// `FloType.decode`. Unknown opcodes are ignored, as the original does.
    fn decode(data: Option<&[u8]>) -> Result<Self, String> {
        let mut out = Self {
            base_color: 0,
            secondary_color: -1,
            texture: -1,
            water: false,
        };
        let Some(data) = data else {
            return Ok(out);
        };
        let mut r = Reader::new(data);
        let mut read = || -> rs09_protocol::Result<()> {
            loop {
                match r.g1()? {
                    0 => return Ok(()),
                    1 => out.base_color = crate::floor::color(r.g3()?),
                    2 => out.texture = r.g1()?,
                    3 => {
                        out.texture = r.g2()?;
                        if out.texture == 65535 {
                            out.texture = -1;
                        }
                    }
                    7 => out.secondary_color = crate::floor::color(r.g3()?),
                    8 => out.water = true,
                    9 => {
                        r.g2()?;
                    }
                    11 | 14 => {
                        r.g1()?;
                    }
                    13 => {
                        r.g3()?;
                    }
                    _ => {}
                }
            }
        };
        read().map_err(|e| format!("world map overlay type: {e}"))?;
        Ok(out)
    }
}

/// The layers `load` fills, named after the Java arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// `blendedUnderlayColors`: RGB per tile, 0 where there is no underlay.
    BlendedUnderlay,
    /// `overlayIds`: overlay type + 1.
    Overlay,
    /// `overlayShapes`: shape << 2 | rotation.
    OverlayShape,
    /// `secondaryOverlayIds`.
    SecondaryOverlay,
    /// `secondaryOverlayShapes`.
    SecondaryOverlayShape,
    /// `wallData`: the wall stroke code 1..=28.
    Wall,
    /// `scenery`: map scene + 1, angle << 14, width << 16, length << 20.
    Scenery,
    /// `underlayColors`: the location id + 1 of a map function.
    MapFunction,
}

/// The eight label fonts, by `WorldMapFont` size.
const FONT_SIZES: [i32; 8] = [11, 12, 14, 17, 19, 22, 26, 30];

/// `WorldMap`: the whole class as one state object.
pub struct WorldMap {
    /// `MapList`.
    pub maps: MapList,
    config: Config,
    /// `currentMap`, as an index into `maps.maps`.
    current_map: Option<usize>,
    /// `loadPercentage`: 0..=100; the map draws at 100.
    pub load_percentage: i32,
    /// `zoom`: twice the pixels per tile. 3, 4, 6, 8 and 16 are the stops.
    pub zoom: f32,
    /// `targetZoom`.
    pub target_zoom: f32,
    /// `originX`: the absolute tile x of the map's left edge.
    pub origin_x: i32,
    /// `originY`: the absolute tile y of the map's bottom edge.
    pub origin_y: i32,
    /// `width` in tiles.
    pub width: i32,
    /// `length` in tiles.
    pub length: i32,
    /// `viewX`: the viewport centre in map tiles from the left edge.
    pub view_x: i32,
    /// `viewY`: the viewport centre in map tiles from the top edge.
    pub view_y: i32,
    /// `panTargetX`, -1 when idle.
    pub pan_target_x: i32,
    /// `panTargetY`, -1 when idle.
    pub pan_target_y: i32,
    /// `viewportWidth` in tiles, set by `render`.
    pub viewport_width: i32,
    /// `viewportHeight` in tiles, set by `render`.
    pub viewport_height: i32,
    /// `Cs1ScriptRunner.worldMapViewportX`: the viewport's left map tile.
    pub viewport_x: i32,
    /// `Cs1ScriptRunner.worldMapViewportY`: the viewport's top map tile.
    pub viewport_y: i32,
    /// `component`: the width and height of the client code 1400 component
    /// seen by this logic cycle, or None. `Protocol` clears it before the
    /// interface pass and `InterfaceList` sets it when it meets the component.
    pub component: Option<[i32; 2]>,
    /// `previousMapName`.
    pub previous_map_name: Option<String>,
    /// `labels`.
    pub labels: Option<MapElementList>,
    /// `mapElements`: every map function found by `readLocs`.
    pub map_elements: Vec<MapElement>,
    /// `highlightedMapFunction`.
    pub highlighted_map_function: i32,
    /// `highlightPulseCount`: pulses left.
    pub highlight_pulse_count: i32,
    /// `Cs1ScriptRunner.mapHighlightPulseCounter`: 20 down to 1 within a pulse.
    pub highlight_pulse_counter: i32,
    label_iter_index: i32,
    overlay_colors: Vec<i32>,
    water_overlay_id: i32,
    fonts: [Option<WorldMapFont>; 8],
    overlay_ids: Grid<u8>,
    overlay_shapes: Grid<u8>,
    secondary_overlay_ids: Grid<u8>,
    secondary_overlay_shapes: Grid<u8>,
    wall_data: Grid<u8>,
    scenery: Grid<i32>,
    /// `underlayColors` in the Java: location id + 1 of each map function.
    map_function_locs: Grid<i32>,
    blended_underlay_colors: Grid<i32>,
    locs: HashMap<i32, Loc>,
    map_scenes: HashMap<i32, MapScene>,
    map_scene_sprites: HashMap<i32, Indexed>,
    highlighted_elements: Vec<MapElement>,
    /// `overviewSprite`.
    overview: Option<Sprite>,
}

impl WorldMap {
    /// The state the Java class has before any map is selected.
    pub fn new(maps: MapList, config: Config) -> Result<Self, String> {
        if config.palette.len() != 65536 {
            return Err("world map: the palette needs 65536 entries".into());
        }
        Ok(Self {
            maps,
            config,
            current_map: None,
            load_percentage: 0,
            zoom: 0.0,
            target_zoom: 0.0,
            origin_x: 0,
            origin_y: 0,
            width: 0,
            length: 0,
            view_x: 0,
            view_y: 0,
            pan_target_x: -1,
            pan_target_y: -1,
            viewport_width: 0,
            viewport_height: 0,
            viewport_x: 0,
            viewport_y: 0,
            component: None,
            previous_map_name: None,
            labels: None,
            map_elements: Vec::new(),
            highlighted_map_function: 0,
            highlight_pulse_count: 0,
            highlight_pulse_counter: 0,
            label_iter_index: 0,
            overlay_colors: Vec::new(),
            water_overlay_id: 0,
            fonts: Default::default(),
            overlay_ids: Grid::default(),
            overlay_shapes: Grid::default(),
            secondary_overlay_ids: Grid::default(),
            secondary_overlay_shapes: Grid::default(),
            wall_data: Grid::default(),
            scenery: Grid::default(),
            map_function_locs: Grid::default(),
            blended_underlay_colors: Grid::default(),
            locs: HashMap::new(),
            map_scenes: HashMap::new(),
            map_scene_sprites: HashMap::new(),
            highlighted_elements: Vec::new(),
            overview: None,
        })
    }

    /// The fixed inputs given to [`WorldMap::new`].
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// `font11` .. `font30` by size (11, 12, 14, 17, 19, 22, 26 or 30).
    pub fn font(&self, size: i32) -> Option<&WorldMapFont> {
        self.fonts[FONT_SIZES.iter().position(|&s| s == size)?].as_ref()
    }

    /// Replace one label font, for a glyph table made elsewhere. `load` and
    /// `clear` overwrite it like the fonts they create.
    pub fn set_font(&mut self, size: i32, font: Option<WorldMapFont>) -> bool {
        let Some(slot) = FONT_SIZES.iter().position(|&s| s == size) else {
            return false;
        };
        self.fonts[slot] = font;
        true
    }

    /// `FloType.waterOverlayId`, as of the last load.
    pub fn water_overlay_id(&self) -> i32 {
        self.water_overlay_id
    }

    /// The loaded map's size in 64-tile squares: columns and rows.
    pub fn squares(&self) -> [i32; 2] {
        [
            self.blended_underlay_colors.cols,
            self.blended_underlay_colors.rows,
        ]
    }

    /// One 64x64 square of a loaded layer as the Java array holds it (bytes
    /// sign-extended), indexed `(y & 63) << 6 | x & 63` with y counted from
    /// the map's top; None where the Java has no array.
    pub fn square(&self, layer: Layer, x: i32, y: i32) -> Option<Vec<i32>> {
        let bytes = |grid: &Grid<u8>| {
            grid.chunk(x, y)
                .map(|chunk| chunk.iter().map(|&b| i32::from(b as i8)).collect())
        };
        match layer {
            Layer::BlendedUnderlay => self
                .blended_underlay_colors
                .chunk(x, y)
                .map(<[i32]>::to_vec),
            Layer::Overlay => bytes(&self.overlay_ids),
            Layer::OverlayShape => bytes(&self.overlay_shapes),
            Layer::SecondaryOverlay => bytes(&self.secondary_overlay_ids),
            Layer::SecondaryOverlayShape => bytes(&self.secondary_overlay_shapes),
            Layer::Wall => bytes(&self.wall_data),
            Layer::Scenery => self.scenery.chunk(x, y).map(<[i32]>::to_vec),
            Layer::MapFunction => self.map_function_locs.chunk(x, y).map(<[i32]>::to_vec),
        }
    }

    /// `WorldMap.getCurrentMap` (commands 5210, 5211 and 5218).
    pub fn current_map(&self) -> Option<&Map> {
        self.maps.maps.get(self.current_map?)
    }

    /// `WorldMap.load`: one loading step per call, as the Java runs one per frame.
    pub fn load(
        &mut self,
        assets: &dyn Assets,
        client: &mut Client<'_>,
    ) -> Result<LoadStep, String> {
        let mut step = LoadStep::default();
        let Some(map) = self.current_map().cloned() else {
            return Ok(step);
        };
        if self.load_percentage < 10 {
            if !assets.group_ready(&map.group) {
                self.load_percentage = assets.group_percentage(&map.group) / 10;
                return Ok(step);
            }
            step.clear_scene = true;
            self.load_percentage = 10;
        }
        let read = |name: &str| {
            assets
                .map_file(&map.group, name)
                .ok_or_else(|| format!("world map {}: {name} is missing", map.group))
        };
        match self.load_percentage {
            10 => {
                self.begin(assets, client, &map)?;
                self.load_percentage = 20;
                return Ok(step);
            }
            20 => {
                self.read_underlay(assets, &read("underlay")?)?;
                self.load_percentage = 30;
                step.ping = true;
            }
            30 => {
                let data = read("overlay")?;
                read_overlay(self, &data, false)?;
                self.load_percentage = 40;
            }
            40 => {
                let data = read("overlay2")?;
                read_overlay(self, &data, true)?;
                self.load_percentage = 50;
            }
            50 => {
                self.read_locs(assets, &read("loc")?)?;
                self.load_percentage = 60;
                step.ping = true;
            }
            60 => {
                let group = format!("{}_labels", map.group);
                if assets.group_valid(&group) {
                    if !assets.group_ready(&group) {
                        return Ok(step);
                    }
                    self.labels = Some(match assets.group_files(&group) {
                        Some(files) => MapElementList::decode(files.iter().map(Vec::as_slice))?,
                        None => MapElementList::default(),
                    });
                } else {
                    self.labels = Some(MapElementList::default());
                }
                self.load_percentage = 70;
            }
            percentage => {
                // 70, 73, 76, 79, 82, 85, 88 and 91: one font each.
                let slot = match percentage {
                    70 => 0,
                    73 => 1,
                    76 => 2,
                    79 => 3,
                    82 => 4,
                    85 => 5,
                    88 => 6,
                    _ => 7,
                };
                if let Some(face) = &self.config.typeface {
                    self.fonts[slot] = Some(WorldMapFont::from_source(
                        true,
                        &face.sized(FONT_SIZES[slot]),
                    )?);
                }
                self.load_percentage = if slot == 7 { 100 } else { percentage + 3 };
                step.ping = true;
            }
        }
        step.reset_timer = true;
        Ok(step)
    }

    /// The `loadPercentage == 10` step: bounds, default zoom, initial view,
    /// empty layers and overlay colours.
    fn begin(
        &mut self,
        assets: &dyn Assets,
        client: &mut Client<'_>,
        map: &Map,
    ) -> Result<(), String> {
        self.origin_x = map.min_x >> 6 << 6;
        self.origin_y = map.min_y >> 6 << 6;
        self.length = (map.max_y >> 6 << 6)
            .wrapping_add(64)
            .wrapping_sub(self.origin_y);
        self.width = (map.max_x >> 6 << 6)
            .wrapping_add(64)
            .wrapping_sub(self.origin_x);
        if self.width <= 0 || self.length <= 0 {
            return Err(format!("world map {} has no area", map.group));
        }
        let zoom = match map.default_zoom {
            37 => 3.0,
            50 => 4.0,
            75 => 6.0,
            100 => 8.0,
            200 => 16.0,
            _ => 8.0,
        };
        self.zoom = zoom;
        self.target_zoom = zoom;
        let player_x = client.player[0].wrapping_sub(self.origin_x);
        let view_x = player_x
            .wrapping_add(((client.random)() * 10.0) as i32)
            .wrapping_sub(5);
        let player_y = self
            .origin_y
            .wrapping_add(self.length)
            .wrapping_sub(client.player[1])
            .wrapping_sub(1);
        let view_y = player_y
            .wrapping_add(((client.random)() * 10.0) as i32)
            .wrapping_sub(5);
        if view_x >= 0 && self.width > view_x && view_y >= 0 && view_y < self.length {
            self.view_x = view_x;
            self.view_y = view_y;
        } else {
            self.view_y = self
                .origin_y
                .wrapping_add(self.length)
                .wrapping_sub(map.origin_y.wrapping_mul(64))
                .wrapping_sub(1);
            self.view_x = map.origin_x.wrapping_mul(64).wrapping_sub(self.origin_x);
        }
        self.clamp_view_position();
        let cols = self.width >> 6;
        let rows = self.length >> 6;
        let light_jitter = client.jitter[1] >> 2 << 10;
        let hue_jitter = client.jitter[0] >> 1;
        self.overlay_shapes = Grid::new(cols, rows);
        self.wall_data = Grid::new(cols, rows);
        self.map_function_locs = Grid::new(cols, rows);
        self.overlay_ids = Grid::new(cols, rows);
        self.blended_underlay_colors = Grid::new(cols, rows);
        self.secondary_overlay_ids = Grid::new(cols, rows);
        self.secondary_overlay_shapes = Grid::new(cols, rows);
        self.scenery = Grid::new(cols, rows);
        self.load_overlay_colors(assets, hue_jitter, light_jitter)
    }

    fn palette(&self, hsl: i32) -> i32 {
        usize::try_from(hsl)
            .ok()
            .and_then(|i| self.config.palette.get(i))
            .map_or(0, |&rgb| rgb as i32)
    }

    /// `WorldMap.loadOverlayColors`. The parameter names are the Java's: the
    /// first shifts lightness and the second shifts hue.
    fn load_overlay_colors(
        &mut self,
        assets: &dyn Assets,
        hue_jitter: i32,
        light_jitter: i32,
    ) -> Result<(), String> {
        let capacity = assets.overlay_capacity().max(0);
        self.overlay_colors = vec![0; capacity as usize + 1];
        let adjusted = |hsl: i32| {
            let light = ((hsl & 0x7f) + hue_jitter).clamp(0, 127);
            (hsl & 0x380) + (light_jitter.wrapping_add(hsl) & 0xfc00) + light
        };
        for id in 0..capacity {
            let flo = FloType::decode(assets.overlay(id).as_deref())?;
            if flo.water {
                self.water_overlay_id = id;
            }
            let mut texture = flo.texture;
            if texture >= 0 && !assets.texture_repeating(texture) {
                texture = -1;
            }
            let color = if flo.secondary_color >= 0 {
                self.palette(multiply_lightness_grayscale(
                    adjusted(flo.secondary_color),
                    96,
                ))
            } else if texture >= 0 {
                self.palette(multiply_lightness_grayscale(
                    assets.texture_average_color(texture),
                    96,
                ))
            } else if flo.base_color == -1 {
                -1
            } else {
                self.palette(multiply_lightness_grayscale(adjusted(flo.base_color), 96))
            };
            self.overlay_colors[id as usize + 1] = color;
        }
        Ok(())
    }

    /// `overlayColors[index]`: 0 is no overlay, -1 an overlay without colour.
    pub fn overlay_color(&self, index: i32) -> i32 {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.overlay_colors.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// `WorldMap.readUnderlay`: the underlay ids, then the blended colours.
    fn read_underlay(&mut self, assets: &dyn Assets, data: &[u8]) -> Result<(), String> {
        let light_offset = self.config.random_light_offset >> 1;
        let hue_offset = self.config.random_hue_offset >> 2 << 10;
        let (width, length) = (self.width as usize, self.length as usize);
        let mut underlays = vec![0u8; width * length];
        let mut r = Reader::new(data);
        let short = |e: rs09_protocol::Error| format!("world map underlay: {e}");
        while r.remaining() > 0 {
            let mut sub = None;
            if r.g1().map_err(short)? == 1 {
                sub = Some([r.g1().map_err(short)?, r.g1().map_err(short)?]);
            }
            let square_x = r.g1().map_err(short)?;
            let square_y = r.g1().map_err(short)?;
            let base_x = square_x * 64 - self.origin_x;
            let base_y = self.length + self.origin_y - square_y * 64 - 1;
            if base_x >= 0 && base_y - 63 >= 0 && self.width > base_x + 63 && self.length > base_y {
                for x in 0..64 {
                    for y in 0..64 {
                        if sub.is_none_or(|[sx, sy]| {
                            x >= sx * 8 && sx * 8 + 8 > x && y >= sy * 8 && y < sy * 8 + 8
                        }) {
                            underlays[(base_x + x) as usize * length + (base_y - y) as usize] =
                                r.g1().map_err(short)? as u8;
                        }
                    }
                }
            } else {
                skip(&mut r, if sub.is_some() { 64 } else { 4096 });
            }
        }
        let mut types = HashMap::new();
        for &id in &underlays {
            if id != 0 && !types.contains_key(&id) {
                let flu = match assets.underlay(i32::from(id) - 1) {
                    Some(data) => {
                        Underlay::decode(&data).map_err(|e| format!("underlay type: {e}"))?
                    }
                    None => Underlay::default(),
                };
                types.insert(id, flu);
            }
        }
        // sums[row] = [hue, saturation, lightness, chroma, count]
        let mut sums = vec![[0i32; 5]; length];
        let width = self.width;
        let length_i = self.length;
        for x in -5..width {
            for (row, sum) in sums.iter_mut().enumerate() {
                let right = x + 5;
                if width > right {
                    let id = underlays[right as usize * length + row];
                    if id > 0 {
                        let flu = &types[&id];
                        sum[0] = sum[0].wrapping_add(flu.weighted_hue);
                        sum[1] = sum[1].wrapping_add(flu.saturation);
                        sum[2] = sum[2].wrapping_add(flu.lightness);
                        sum[3] = sum[3].wrapping_add(flu.chroma);
                        sum[4] += 1;
                    }
                }
                let left = x - 5;
                if left >= 0 {
                    let id = underlays[left as usize * length + row];
                    if id > 0 {
                        let flu = &types[&id];
                        sum[0] = sum[0].wrapping_sub(flu.weighted_hue);
                        sum[1] = sum[1].wrapping_sub(flu.saturation);
                        sum[2] = sum[2].wrapping_sub(flu.lightness);
                        sum[3] = sum[3].wrapping_sub(flu.chroma);
                        sum[4] -= 1;
                    }
                }
            }
            if x < 0 {
                continue;
            }
            let mut window = [0i32; 5];
            for col in -5..length_i {
                let add = col + 5;
                if length_i > add {
                    for (total, value) in window.iter_mut().zip(sums[add as usize]) {
                        *total = total.wrapping_add(value);
                    }
                }
                let sub = col - 5;
                if sub >= 0 {
                    for (total, value) in window.iter_mut().zip(sums[sub as usize]) {
                        *total = total.wrapping_sub(value);
                    }
                }
                let [hue, saturation, lightness, chroma, count] = window;
                if col >= 0 && count > 0 {
                    let hsl = if chroma == 0 {
                        0
                    } else {
                        pack_hsl(
                            lightness / count,
                            saturation / count,
                            hue.wrapping_mul(256).wrapping_div(chroma),
                        )
                    };
                    let index = (((col & 0x3f) << 6) + (x & 0x3f)) as usize;
                    if underlays[x as usize * length + col as usize] != 0 {
                        let light = (light_offset + (hsl & 0x7f)).clamp(0, 127);
                        let adjusted =
                            light + (hsl & 0x380) + (hsl.wrapping_add(hue_offset) & 0xfc00);
                        let rgb = self.palette(multiply_lightness_safe(96, adjusted));
                        if let Some(chunk) =
                            self.blended_underlay_colors.chunk_mut(x >> 6, col >> 6)
                        {
                            chunk[index] = rgb;
                        }
                    } else if let Some(chunk) =
                        self.blended_underlay_colors.existing_mut(x >> 6, col >> 6)
                    {
                        chunk[index] = 0;
                    }
                }
            }
        }
        Ok(())
    }

    /// `LocTypeList.get(id)`, reduced to what the map reads.
    fn loc(&mut self, assets: &dyn Assets, id: i32) -> Result<Loc, String> {
        if let Some(loc) = self.locs.get(&id) {
            return Ok(loc.clone());
        }
        let loc = match assets.location(id) {
            Some(data) => {
                let kind = LocationType::decode(&data)
                    .map_err(|e| format!("world map location {id}: {e}"))?;
                Loc {
                    map_function: kind.map_function,
                    morph: kind.children.is_some().then(|| {
                        kind.select_morph(|varbit, id| {
                            if varbit {
                                assets.varbit(id)
                            } else {
                                assets.varp(id)
                            }
                        })
                        .map(|id| id as i32)
                    }),
                }
            }
            None => Loc {
                map_function: -1,
                morph: None,
            },
        };
        self.locs.insert(id, loc.clone());
        Ok(loc)
    }

    /// `WorldMap.readLocs`: walls, map scenes and map functions.
    fn read_locs(&mut self, assets: &dyn Assets, data: &[u8]) -> Result<(), String> {
        let mut r = Reader::new(data);
        let short = |e: rs09_protocol::Error| format!("world map locs: {e}");
        while r.remaining() > 0 {
            let mut sub = None;
            if r.g1().map_err(short)? == 1 {
                sub = Some([r.g1().map_err(short)?, r.g1().map_err(short)?]);
            }
            let square_x = r.g1().map_err(short)?;
            let square_y = r.g1().map_err(short)?;
            let base_x = square_x * 64 - self.origin_x;
            let base_y = self.origin_y + self.length - square_y * 64 - 1;
            if base_x >= 0 && base_y - 63 >= 0 && self.width > base_x + 63 && base_y < self.length {
                let (cx, cy) = (base_x >> 6, base_y >> 6);
                for col in 0..64 {
                    for row in 0..64 {
                        if !sub.is_none_or(|[sx, sy]| {
                            sx * 8 <= col && col < sx * 8 + 8 && row >= sy * 8 && row < sy * 8 + 8
                        }) {
                            continue;
                        }
                        let flags = r.g1().map_err(short)?;
                        if flags == 0 {
                            continue;
                        }
                        let index = (col + ((63 - row) << 6)) as usize;
                        if flags & 1 == 1 {
                            let wall = r.g1().map_err(short)?;
                            if let Some(chunk) = self.wall_data.chunk_mut(cx, cy) {
                                chunk[index] = wall as u8;
                            }
                        }
                        if flags & 2 == 2 {
                            let scene = r.g3().map_err(short)?;
                            if let Some(chunk) = self.scenery.chunk_mut(cx, cy) {
                                chunk[index] = scene;
                            }
                        }
                        if flags & 4 == 4 {
                            let mut id = r.g3().map_err(short)? - 1;
                            self.map_function_locs.chunk_mut(cx, cy);
                            let mut loc = self.loc(assets, id)?;
                            if let Some(morph) = loc.morph {
                                let Some(child) = morph else {
                                    continue;
                                };
                                id = child;
                                loc = self.loc(assets, id)?;
                                if loc.map_function == -1 {
                                    continue;
                                }
                            }
                            if let Some(chunk) = self.map_function_locs.chunk_mut(cx, cy) {
                                chunk[index] = id.wrapping_add(1);
                            }
                            self.map_elements.push(MapElement {
                                id: loc.map_function,
                                map_x: base_x,
                                map_y: base_y,
                            });
                        }
                    }
                }
            } else {
                for _ in 0..if sub.is_some() { 64 } else { 4096 } {
                    let flags = r.g1().map_err(short)?;
                    if flags != 0 {
                        if flags & 1 == 1 {
                            skip(&mut r, 1);
                        }
                        if flags & 2 == 2 {
                            skip(&mut r, 2);
                        }
                        if flags & 4 == 4 {
                            skip(&mut r, 3);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// `WorldMap.clampViewPosition`.
    pub fn clamp_view_position(&mut self) {
        if self.view_x < 0 {
            self.pan_target_y = -1;
            self.view_x = 0;
            self.pan_target_x = -1;
        }
        if self.view_x > self.width {
            self.pan_target_y = -1;
            self.view_x = self.width;
            self.pan_target_x = -1;
        }
        if self.view_y < 0 {
            self.pan_target_x = -1;
            self.pan_target_y = -1;
            self.view_y = 0;
        }
        if self.length < self.view_y {
            self.view_y = self.length;
            self.pan_target_y = -1;
            self.pan_target_x = -1;
        }
    }

    /// `WorldMap.clear(saveName)`: drop the loaded map, remembering its group
    /// for `restore_previous_map` when asked. `client.setGameState(25 | 28)`
    /// calls this with true; logout and `client.unload` with false.
    pub fn clear(&mut self, save_name: bool) {
        self.overlay_shapes = Grid::default();
        self.map_function_locs = Grid::default();
        self.component = None;
        self.overlay_ids = Grid::default();
        self.overlay_colors = Vec::new();
        self.secondary_overlay_shapes = Grid::default();
        self.previous_map_name = match self.current_map() {
            Some(map) if save_name => Some(map.group.clone()),
            _ => None,
        };
        self.wall_data = Grid::default();
        self.secondary_overlay_ids = Grid::default();
        self.scenery = Grid::default();
        self.blended_underlay_colors = Grid::default();
        self.load_percentage = 0;
        self.current_map = None;
        self.map_elements.clear();
        self.labels = None;
        self.pan_target_y = -1;
        self.fonts = Default::default();
        self.overview = None;
        self.pan_target_x = -1;
        // The Java keeps these in soft caches that outlive the map.
        self.locs.clear();
        self.map_scenes.clear();
        self.map_scene_sprites.clear();
    }

    /// `WorldMap.reset`, which `Protocol` runs for `IF_OPENSUB` with reset 2.
    /// The caller then performs `client.setGameState(25)`, whose own
    /// `clear(true)` finds no map and so leaves no previous name.
    pub fn reset(&mut self) {
        self.clear(false);
    }

    /// `WorldMap.restorePreviousMap`, run by `LoginManager` once a rebuilt
    /// scene is ready.
    pub fn restore_previous_map(&mut self) {
        if let Some(name) = self.previous_map_name.take() {
            self.switch_map(&name);
        }
    }

    /// `WorldMap.switchMap` (command 5205).
    pub fn switch_map(&mut self, name: &str) {
        self.clear(false);
        self.set_map_by_name(name);
    }

    /// `WorldMap.setMapByName`: an unknown name leaves the map unset.
    pub fn set_map_by_name(&mut self, name: &str) {
        if let Some(index) = self.maps.maps.iter().position(|map| map.group == name) {
            self.current_map = Some(index);
        }
    }

    /// `WorldMap.setTargetZoom` (command 5200): 37, 50, 75, 100 or 200.
    pub fn set_target_zoom(&mut self, level: i32) {
        self.pan_target_y = -1;
        match level {
            37 => self.target_zoom = 3.0,
            50 => self.target_zoom = 4.0,
            75 => self.target_zoom = 6.0,
            100 => self.target_zoom = 8.0,
            200 => self.target_zoom = 16.0,
            _ => {}
        }
        self.pan_target_y = -1;
    }

    /// `WorldMap.getTargetZoom` (command 5201).
    pub fn get_target_zoom(&self) -> i32 {
        if f64::from(self.target_zoom) == 3.0 {
            37
        } else if f64::from(self.target_zoom) == 4.0 {
            50
        } else if f64::from(self.target_zoom) == 6.0 {
            75
        } else if f64::from(self.target_zoom) == 8.0 {
            100
        } else {
            200
        }
    }

    /// `WorldMap.updateZoomAndScroll`: one logic cycle of zoom easing and
    /// panning. `Protocol` calls it only while `component` is set.
    pub fn update_zoom_and_scroll(&mut self) {
        if self.zoom < self.target_zoom {
            self.zoom = (f64::from(self.zoom) + f64::from(self.zoom) / 30.0) as f32;
            if self.target_zoom < self.zoom {
                self.zoom = self.target_zoom;
            }
            self.clamp_view_position();
        } else if self.target_zoom < self.zoom {
            self.zoom = (f64::from(self.zoom) - f64::from(self.zoom) / 30.0) as f32;
            if self.target_zoom > self.zoom {
                self.zoom = self.target_zoom;
            }
            self.clamp_view_position();
        }
        if self.pan_target_x == -1 || self.pan_target_y == -1 {
            return;
        }
        // `if (dx < 2 || dx > 2) dx >>= 4`: every distance but exactly 2 is eased.
        let mut dx = self.pan_target_x.wrapping_sub(self.view_x);
        if dx != 2 {
            dx >>= 4;
        }
        let mut dy = self.pan_target_y.wrapping_sub(self.view_y);
        if dy != 2 {
            dy >>= 4;
        }
        self.view_y = self.view_y.wrapping_add(dy);
        self.view_x = self.view_x.wrapping_add(dx);
        if dx == 0 && dy == 0 {
            self.pan_target_x = -1;
            self.pan_target_y = -1;
        }
        self.clamp_view_position();
    }

    /// `WorldMap.setViewX`: the drag handler's horizontal move.
    pub fn set_view_x(&mut self, x: i32) {
        self.pan_target_y = -1;
        self.pan_target_x = -1;
        self.view_x = x;
        self.clamp_view_position();
    }

    /// `WorldMap.setViewY`. The original clears `panTargetY` twice and
    /// `panTargetX` not at all.
    pub fn set_view_y(&mut self, y: i32) {
        self.pan_target_y = -1;
        self.view_y = y;
        self.clamp_view_position();
    }

    /// `WorldMap.setViewFromMousePosition`: a press on the overview
    /// component (client code 1401) of `comp_w` by `comp_h` at the
    /// component-relative mouse position.
    pub fn set_view_from_mouse_position(
        &mut self,
        comp_w: i32,
        mouse_y: i32,
        mouse_x: i32,
        comp_h: i32,
    ) {
        if comp_w == 0 || comp_h == 0 {
            return;
        }
        self.view_x = self.width.wrapping_mul(mouse_x).wrapping_div(comp_w);
        self.view_y = self.length.wrapping_mul(mouse_y).wrapping_div(comp_h);
        self.pan_target_x = -1;
        self.pan_target_y = -1;
        self.clamp_view_position();
    }

    /// `WorldMap.panToCoords` (command 5214) to an absolute tile. The
    /// original dereferences `component` after assigning `panTargetX`; with
    /// no component this stops at the same point and returns false.
    pub fn pan_to_coords(&mut self, x: i32, y: i32) -> bool {
        self.pan_target_x = x.wrapping_sub(self.origin_x);
        let Some([comp_w, comp_h]) = self.component else {
            return false;
        };
        let across = (comp_w as f32 / self.zoom) as i32;
        let down = (comp_h as f32 / self.zoom) as i32;
        let view_left = self.pan_target_x.wrapping_sub(across);
        let view_right = self.pan_target_x.wrapping_add(across);
        if view_left < 0 {
            self.pan_target_x = across;
        }
        self.pan_target_y = self
            .length
            .wrapping_add(self.origin_y)
            .wrapping_sub(y)
            .wrapping_sub(1);
        let view_bottom = down.wrapping_add(self.pan_target_y);
        let view_top = self.pan_target_y.wrapping_sub(down);
        if view_right > self.width {
            self.pan_target_x = self.width.wrapping_sub(across);
        }
        if view_top < 0 {
            self.pan_target_y = down;
        }
        if self.length < view_bottom {
            self.pan_target_y = self.length.wrapping_sub(down);
        }
        true
    }

    /// `WorldMap.findLabelByPrefix`: the first label whose name, with line
    /// breaks as spaces, starts with `prefix` ignoring case.
    pub fn find_label_by_prefix(&self, prefix: &str) -> i32 {
        let prefix = jag_bytes(prefix);
        let Some(labels) = self.labels.as_ref().filter(|_| !prefix.is_empty()) else {
            return -1;
        };
        labels
            .names
            .iter()
            .position(|name| starts_with_ignore_case(&breaks_to_spaces(name), &prefix))
            .map_or(-1, |i| i as i32)
    }

    /// `WorldMap.findLabelByExactName`: case-sensitive.
    pub fn find_label_by_exact_name(&self, name: &str) -> i32 {
        let wanted = jag_bytes(name);
        let Some(labels) = self.labels.as_ref().filter(|_| !wanted.is_empty()) else {
            return -1;
        };
        labels
            .names
            .iter()
            .position(|name| breaks_to_spaces(name) == wanted)
            .map_or(-1, |i| i as i32)
    }

    fn pan_to_label(&mut self, index: i32) {
        let Some(labels) = self.labels.as_ref().filter(|_| index >= 0) else {
            return;
        };
        let x = i32::from(labels.coord_x[index as usize]);
        let y = i32::from(labels.coord_y[index as usize]);
        self.pan_to_coords(x, y);
    }

    /// `WorldMap.panToLabelByPrefix` (command 5203).
    pub fn pan_to_label_by_prefix(&mut self, prefix: &str) {
        self.pan_to_label(self.find_label_by_prefix(prefix));
    }

    /// `WorldMap.panToLabelByExactName` (command 5219).
    pub fn pan_to_label_by_exact_name(&mut self, name: &str) {
        self.pan_to_label(self.find_label_by_exact_name(name));
    }

    /// `WorldMap.getLabelTextByPrefix` (command 5204): the found label's
    /// name with line breaks as spaces, or the empty string.
    pub fn get_label_text_by_prefix(&self, prefix: &str) -> String {
        self.label_text(self.find_label_by_prefix(prefix))
    }

    /// `labels.names[index].replaceAll(SPACE, BR_TAG)` as commands 5212 and
    /// 5213 push it; the empty string for -1.
    pub fn label_text(&self, index: i32) -> String {
        self.labels
            .as_ref()
            .zip(usize::try_from(index).ok())
            .and_then(|(labels, i)| labels.names.get(i))
            .map_or_else(String::new, |name| latin1(&breaks_to_spaces(name)))
    }

    /// `labels.getLabelSize(index)` as commands 5212 and 5213 push it; 0 for -1.
    pub fn label_size(&self, index: i32) -> i32 {
        self.labels
            .as_ref()
            .filter(|_| index >= 0)
            .map_or(0, |labels| labels.get_label_size(index))
    }

    /// `WorldMap.getFirstVisibleLabel` (command 5212).
    pub fn get_first_visible_label(&mut self) -> i32 {
        self.label_iter_index = 0;
        self.get_next_visible_label()
    }

    /// `WorldMap.getNextVisibleLabel` (command 5213); -1 at the end.
    pub fn get_next_visible_label(&mut self) -> i32 {
        let Some(labels) = &self.labels else {
            return -1;
        };
        while self.label_iter_index < labels.count() {
            let index = self.label_iter_index;
            self.label_iter_index += 1;
            if labels.is_visible(index) {
                return index;
            }
        }
        -1
    }

    /// `WorldMap.highlightMapElement` (command 5202): three pulses.
    pub fn highlight_map_element(&mut self, map_function: i32) {
        self.highlighted_map_function = map_function;
        self.highlight_pulse_counter = 20;
        self.highlight_pulse_count = 3;
    }

    /// Command 5208: `viewportWidth` and `viewportHeight`.
    pub fn viewport_size(&self) -> [i32; 2] {
        [self.viewport_width, self.viewport_height]
    }

    /// Command 5209: the absolute tile at the viewport centre.
    pub fn view_coord(&self) -> [i32; 2] {
        [
            self.origin_x.wrapping_add(self.view_x),
            self.origin_y
                .wrapping_add(self.length)
                .wrapping_sub(self.view_y)
                .wrapping_sub(1),
        ]
    }

    /// Command 5210: the current map's default centre tile, or zeros.
    pub fn current_map_center(&self) -> [i32; 2] {
        self.current_map().map_or([0, 0], |map| {
            [map.origin_x.wrapping_mul(64), map.origin_y.wrapping_mul(64)]
        })
    }

    /// Command 5211: the current map's display width and height, or zeros.
    pub fn current_map_size(&self) -> [i32; 2] {
        self.current_map()
            .map_or([0, 0], |map| [map.max_x - map.min_x, map.max_y - map.min_y])
    }

    /// Command 5218: the current map's default zoom, or -1.
    pub fn current_map_default_zoom(&self) -> i32 {
        self.current_map().map_or(-1, |map| map.default_zoom)
    }

    /// Command 5220.
    pub fn is_loaded(&self) -> bool {
        self.load_percentage == 100
    }

    /// The absolute tile under a point of the 1400 component, as
    /// `InterfaceList` computes it for the staff ctrl-click teleport.
    pub fn tile_at(&self, comp: [i32; 2], point: [i32; 2]) -> [i32; 2] {
        let dx = (f64::from(point[0] - comp[0] / 2) * 2.0 / f64::from(self.zoom)) as i32;
        let dy = (f64::from(point[1] - comp[1] / 2) * 2.0 / f64::from(self.zoom)) as i32;
        let map_y = self.view_y.wrapping_add(dy);
        [
            self.view_x.wrapping_add(dx).wrapping_add(self.origin_x),
            self.length
                .wrapping_add(self.origin_y)
                .wrapping_sub(map_y)
                .wrapping_sub(1),
        ]
    }

    /// `InterfaceList`'s drag of the 1400 component: the view moves from
    /// where it was at the first movement by the mouse travel since the
    /// press, scaled by `targetZoom`.
    pub fn drag(&mut self, start_view: [i32; 2], press: [i32; 2], mouse: [i32; 2]) {
        let zoom = f64::from(self.target_zoom);
        self.set_view_x(
            start_view[0].wrapping_add((f64::from(press[0] - mouse[0]) * 2.0 / zoom) as i32),
        );
        self.set_view_y(
            start_view[1].wrapping_add((f64::from(press[1] - mouse[1]) * 2.0 / zoom) as i32),
        );
    }
}

/// `WorldMap.readOverlay` and `readOverlay2`, which differ only in their layers.
fn read_overlay(map: &mut WorldMap, data: &[u8], secondary: bool) -> Result<(), String> {
    let mut r = Reader::new(data);
    let short = |e: rs09_protocol::Error| format!("world map overlay: {e}");
    while r.remaining() > 0 {
        let mut sub = None;
        if r.g1().map_err(short)? == 1 {
            sub = Some([r.g1().map_err(short)?, r.g1().map_err(short)?]);
        }
        let square_x = r.g1().map_err(short)?;
        let square_y = r.g1().map_err(short)?;
        let base_y = map.origin_y + map.length - square_y * 64 - 1;
        let base_x = square_x * 64 - map.origin_x;
        if base_x >= 0 && base_y - 63 >= 0 && base_x + 63 < map.width && base_y < map.length {
            let (cx, cy) = (base_x >> 6, base_y >> 6);
            let (ids, shapes) = if secondary {
                (
                    &mut map.secondary_overlay_ids,
                    &mut map.secondary_overlay_shapes,
                )
            } else {
                (&mut map.overlay_ids, &mut map.overlay_shapes)
            };
            for col in 0..64 {
                for row in 0..64 {
                    if sub.is_none_or(|[sx, sy]| {
                        col >= sx * 8 && col < sx * 8 + 8 && row >= sy * 8 && row < sy * 8 + 8
                    }) {
                        let id = r.g1().map_err(short)? as u8;
                        if id != 0 {
                            let index = (col + ((63 - row) << 6)) as usize;
                            if let Some(chunk) = ids.chunk_mut(cx, cy) {
                                chunk[index] = id;
                            }
                            let shape = r.g1().map_err(short)? as u8;
                            if let Some(chunk) = shapes.chunk_mut(cx, cy) {
                                chunk[index] = shape;
                            }
                        }
                    }
                }
            }
        } else {
            for _ in 0..if sub.is_some() { 64 } else { 4096 } {
                if r.g1().map_err(short)? != 0 {
                    skip(&mut r, 1);
                }
            }
        }
    }
    Ok(())
}

/// `data.offset += count`, which may run past the end and so end the file.
fn skip(r: &mut Reader<'_>, count: usize) {
    let _ = r.take(count.min(r.remaining()));
}

/// `ColorUtils.packHsl(lightness, saturation, hue)`.
pub fn pack_hsl(lightness: i32, mut saturation: i32, hue: i32) -> i32 {
    if lightness > 243 {
        saturation >>= 4;
    } else if lightness > 217 {
        saturation >>= 3;
    } else if lightness > 192 {
        saturation >>= 2;
    } else if lightness > 179 {
        saturation >>= 1;
    }
    (lightness >> 1) + (saturation >> 5 << 7) + (hue >> 2 << 10)
}

/// `ColorUtils.multiplyLightnessSafe(multiplier, hsl)`.
pub fn multiply_lightness_safe(multiplier: i32, hsl: i32) -> i32 {
    if hsl == -1 {
        return 12345678;
    }
    (multiplier.wrapping_mul(hsl & 0x7f) >> 7).clamp(2, 126) + (hsl & 0xff80)
}

/// `ColorUtils.multiplyLightnessGrayscale(hsl, lightness)`.
pub fn multiply_lightness_grayscale(hsl: i32, lightness: i32) -> i32 {
    if hsl == -2 {
        12345678
    } else if hsl == -1 {
        lightness.clamp(2, 126)
    } else {
        (hsl & 0xff80) + ((hsl & 0x7f).wrapping_mul(lightness) >> 7).clamp(2, 126)
    }
}
