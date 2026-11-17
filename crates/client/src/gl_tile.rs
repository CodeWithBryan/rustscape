//! The HD floor of one scene level, as RT4 builds it for OpenGL:
//! `SceneGraph.buildGlPlainTiles`, `addGlTileVertices`, `buildGlShapedTiles`,
//! `addGlPlainTile`, `computeGlTileVertex`, `getTexturedColor`,
//! `getOrCreateGlTile` and the vertex/face store of `GlTile`.
//!
//! GPU independent. `Level::water` carries the two arguments the original
//! passes for the underwater scene (`waterColors`, `waterTileColors`); they
//! are null for the surface.
//!
//! Grids are x-major: a tile is `x * depth + y`, a corner
//! `x * (depth + 1) + y`. The original scene is 104 x 104 tiles with 105 x 105
//! corners and builds tiles 1..=102; this builds tiles `1..=width - 2` by
//! `1..=depth - 2` of whatever grid it is given.
use crate::floor::{Overlay, Underlay};
use std::collections::{BTreeMap, HashMap};

/// `SceneGraph.UNDERLAY_SHAPE_VERTICES`: despite the name, the polygon the
/// overlay covers for each tile shape (shape 0 is the whole tile).
pub const UNDERLAY_SHAPE_VERTICES: [&[i32]; 12] = [
    &[0, 128, 0, 0, 128, 0, 128, 128],
    &[0, 128, 0, 0, 128, 0],
    &[0, 0, 64, 128, 0, 128],
    &[128, 128, 64, 128, 128, 0],
    &[0, 0, 128, 0, 128, 128, 64, 128],
    &[0, 128, 0, 0, 128, 0, 64, 128],
    &[64, 128, 0, 128, 0, 0, 64, 0],
    &[0, 0, 64, 0, 0, 64],
    &[128, 0, 128, 128, 0, 128, 0, 64, 64, 0],
    &[0, 128, 0, 0, 32, 64, 64, 96, 128, 128],
    &[0, 0, 128, 0, 128, 128, 64, 96, 32, 64],
    &[0, 0, 128, 0, 96, 32, 32, 32],
];
/// `SceneGraph.OVERLAY_SHAPE_VERTICES`: the part of the tile the underlay
/// keeps under an overlay of each shape (nothing under shape 0).
pub const OVERLAY_SHAPE_VERTICES: [&[i32]; 12] = [
    &[],
    &[128, 0, 128, 128, 0, 128],
    &[0, 0, 128, 0, 128, 128, 64, 128],
    &[0, 128, 0, 0, 128, 0, 64, 128],
    &[0, 0, 64, 128, 0, 128],
    &[128, 128, 64, 128, 128, 0],
    &[64, 0, 128, 0, 128, 128, 64, 128],
    &[128, 0, 128, 128, 0, 128, 0, 64, 64, 0],
    &[0, 0, 64, 0, 0, 64],
    &[0, 0, 128, 0, 128, 128, 64, 96, 32, 64],
    &[0, 128, 0, 0, 32, 64, 64, 96, 128, 128],
    &[0, 128, 0, 0, 32, 32, 96, 32, 128, 0, 128, 128],
];
/// `SceneGraph.overlayEdgeLookup`, indexed `shape * 4 + side`.
pub const OVERLAY_EDGE_LOOKUP: [usize; 48] = [
    1, 1, 1, 1, 4, 1, 1, 5, 6, 1, 5, 0, 7, 0, 4, 1, 7, 2, 1, 1, 6, 1, 1, 3, 6, 1, 7, 0, 0, 6, 7, 0,
    1, 7, 6, 1, 1, 1, 5, 4, 3, 2, 1, 1, 0, 4, 1, 5,
];
/// `SceneGraph.tileEdgeMasks`: whether the two ends (and the middle, when
/// present) of a tile edge are blend vertices, i.e. fully transparent.
pub const TILE_EDGE_MASKS: [&[bool]; 8] = [
    &[true, true, true],
    &[false, false],
    &[false, true],
    &[true, false],
    &[false, true, true],
    &[true, false, true],
    &[false, true, false],
    &[true, false, false],
];

/// `SceneGraph.OVERLAY_TRIANGLE_FLAGS`: for the underwater floor, which
/// vertices of the polygon an overlay shape leaves the underlay stay at the
/// surface's height.
pub const OVERLAY_TRIANGLE_FLAGS: [&[bool]; 12] = [
    &[],
    &[true, false, true],
    &[true, false, false, true],
    &[false, false, true, true],
    &[true, true, false],
    &[false, true, true],
    &[true, false, false, true],
    &[false, false, false, true, true],
    &[false, true, true],
    &[true, false, true, true, true],
    &[false, true, true, true, true],
    &[false, true, true, true, true, false],
];

/// The underwater arguments of `buildGlPlainTiles`.
#[derive(Clone, Copy)]
pub struct Water<'a> {
    /// `waterColors` (`tileWaterData`) on the tile grid: the water colour in
    /// the low 24 bits, the opacity above them.
    pub data: &'a [i32],
    /// `waterTileColors`: despite the name, the surface's ground-level
    /// corner heights.
    pub surface: &'a [i32],
}

/// One level of the scene.
pub struct Level<'a> {
    pub width: usize,
    pub depth: usize,
    /// The physical level, 0..=3.
    pub level: usize,
    pub underlays: &'a [u8],
    pub overlays: &'a [u8],
    /// `tileShapes`: the map's shape, 0..=11.
    pub shapes: &'a [u8],
    /// `tileAngles`, 0..=3.
    pub rotations: &'a [u8],
    /// `renderFlags[level]`.
    pub flags: &'a [u8],
    /// `renderFlags[1]`, whose bit 2 marks a bridge column.
    pub bridge_flags: &'a [u8],
    /// `tileHeights[level]`, per corner.
    pub heights: &'a [i32],
    /// `tileLightness`, per corner but stored on the tile grid.
    pub lightness: &'a [i32],
    /// `underlayColors`: the blended underlay HSL of each tile.
    pub colors: &'a [i32],
    /// Set for the underwater scene, which builds the underlay groups only:
    /// each group is also keyed by its tiles' water colour, and every vertex
    /// carries how far it lies below the surface.
    pub water: Option<Water<'a>>,
}

/// Floor definitions and the colour inputs of `getTexturedColor`.
pub struct Tables<'a> {
    /// `FluTypeList.get(id - 1)`.
    pub underlays: &'a [Underlay],
    /// `FloTypeList.get(id - 1)`.
    pub overlays: &'a [Overlay],
    /// `Rasteriser.palette`.
    pub palette: &'a [u32],
    /// `[getTextureBrightness, getTextureSpeed]` per texture.
    pub textures: &'a [[u8; 2]],
}

/// A face of a `GlTile`: a triangle fan over `vertices`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    /// The draw level (`facePlane`): 0 for forced-ground tiles, one lower on
    /// a bridge column.
    pub level: i32,
    pub x: i32,
    pub y: i32,
    pub vertices: Vec<u32>,
    /// `faceBlendIndices`: extra triangles drawn in the blended pass.
    pub blend_indices: Option<Vec<u32>>,
    /// `faceBlend`: the fan itself belongs to the blended pass.
    pub blend: bool,
}

/// All floor faces of one level that share a texture, scale and (for
/// overlays) brightness and water colour.
#[derive(Clone, Debug, Default)]
pub struct GlTile {
    /// The hash key the original sorts its draw order by.
    pub key: i64,
    pub texture: i32,
    pub scale: f32,
    /// Built by `buildGlShapedTiles`: drawn half a depth layer nearer, with a
    /// blended pass a hundred layers nearer still and without depth writes.
    pub blend: bool,
    pub water_color: i32,
    /// `GlTile.underwater`: the vertices carry `depths`.
    pub underwater: bool,
    pub x: Vec<i32>,
    /// Heights (negative is up).
    pub y: Vec<i32>,
    pub z: Vec<i32>,
    /// `rgb << 8 | alpha`.
    pub colors: Vec<u32>,
    pub normals: Vec<[f32; 3]>,
    /// `vertexUnderwaterCoord`: the vertex's drop below the surface over the
    /// tile's fog range, the coordinate of the underwater material's
    /// gradient. Empty unless `underwater`.
    pub depths: Vec<f32>,
    pub faces: Vec<Face>,
    dedup: HashMap<(i32, i32, u32), u32>,
}
impl GlTile {
    fn new(key: i64, texture: i32, scale: f32, blend: bool, water_color: i32) -> Self {
        Self {
            key,
            texture,
            scale,
            blend,
            water_color,
            ..Self::default()
        }
    }
    /// `GlTile.addVertex`: vertices on a tile edge are shared when position
    /// and colour agree, keeping the first normal and the lower height value.
    fn add_vertex(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        normal: [f32; 3],
        color: u32,
        depth: f32,
    ) -> u32 {
        let shared = x & 0x7f == 0 || z & 0x7f == 0;
        if shared {
            if let Some(&index) = self.dedup.get(&(x, z, color)) {
                let stored = &mut self.y[index as usize];
                if y < *stored {
                    *stored = y;
                }
                return index;
            }
        }
        let index = self.x.len() as u32;
        self.x.push(x);
        self.y.push(y);
        self.z.push(z);
        self.normals.push(normal);
        self.colors.push(color);
        if self.underwater {
            self.depths.push(depth);
        }
        if shared {
            self.dedup.insert((x, z, color), index);
        }
        index
    }
}

/// The floor of one level in draw order: the underlay groups, then the
/// overlay groups, each sorted by key.
pub struct Floor {
    pub plain: Vec<GlTile>,
    pub shaped: Vec<GlTile>,
    /// The diagonal `buildGlPlainTiles` chose for each tile without an
    /// overlay (it stores this into `tileAngles`): 0 joins north-west to
    /// south-east, 1 north-east to south-west.
    pub diagonals: Vec<u8>,
}
impl Floor {
    /// Every group in the order `setShapedTile` draws them.
    pub fn groups(&self) -> impl Iterator<Item = &GlTile> {
        self.plain.iter().chain(&self.shaped)
    }
}

struct Scene<'a> {
    level: &'a Level<'a>,
    tables: &'a Tables<'a>,
    /// `-256 / length`, `dx / length` and `dy / length` of each corner's slope.
    up: Vec<f32>,
    east: Vec<f32>,
    north: Vec<f32>,
}
static NO_OVERLAY: std::sync::LazyLock<Overlay> = std::sync::LazyLock::new(Overlay::default);
static NO_UNDERLAY: std::sync::LazyLock<Underlay> = std::sync::LazyLock::new(Underlay::default);

impl Scene<'_> {
    fn tile(&self, x: usize, y: usize) -> usize {
        x * self.level.depth + y
    }
    fn corner(&self, x: usize, y: usize) -> usize {
        x * (self.level.depth + 1) + y
    }
    /// The original would fail on an id past the definition table; an absent
    /// definition is the default one here.
    fn overlay(&self, id: u8) -> &Overlay {
        self.tables
            .overlays
            .get(usize::from(id) - 1)
            .unwrap_or(&NO_OVERLAY)
    }
    fn underlay(&self, id: u8) -> &Underlay {
        self.tables
            .underlays
            .get(usize::from(id) - 1)
            .unwrap_or(&NO_UNDERLAY)
    }
    fn render_level(&self, x: usize, y: usize) -> i32 {
        let tile = self.tile(x, y);
        if self.level.flags[tile] & 8 != 0 {
            0
        } else if self.level.bridge_flags[tile] & 2 == 2 && self.level.level > 0 {
            self.level.level as i32 - 1
        } else {
            self.level.level as i32
        }
    }
    fn height(&self, x: usize, y: usize) -> i32 {
        self.level.heights[self.corner(x, y)]
    }
    fn interpolate_height(&self, x: usize, y: usize, vx: i32, vy: i32) -> i32 {
        self.interpolate(self.level.heights, x, y, vx, vy)
    }
    /// `SceneGraph.interpolateHeight` over a corner grid.
    fn interpolate(&self, heights: &[i32], x: usize, y: usize, vx: i32, vy: i32) -> i32 {
        let height = |x: usize, y: usize| heights[self.corner(x, y)];
        let near = vx
            .wrapping_mul(height(x + 1, y))
            .wrapping_add((128 - vx).wrapping_mul(height(x, y)))
            >> 7;
        let far = height(x, y + 1)
            .wrapping_mul(128 - vx)
            .wrapping_add(height(x + 1, y + 1).wrapping_mul(vx))
            >> 7;
        near.wrapping_mul(128 - vy)
            .wrapping_add(vy.wrapping_mul(far))
            >> 7
    }
    /// `SceneGraph.getTexturedColor`.
    fn textured_color(&self, texture: i32, color: i32, lightness: i32) -> i32 {
        let light = (lightness.wrapping_mul(color & 0x7f) >> 7).clamp(2, 126);
        let mut rgb = self
            .tables
            .palette
            .get(((color & 0xff80) + light) as usize)
            .copied()
            .unwrap_or(0) as i32;
        // Texture 0 is excluded by the original comparison.
        if texture > 0 {
            let [brightness, speed] = self
                .tables
                .textures
                .get((texture & 0xffff) as usize)
                .copied()
                .unwrap_or([0; 2]);
            let brightness = i32::from(brightness);
            if brightness != 0 {
                let gray = if lightness < 0 {
                    0
                } else if lightness > 127 {
                    0xffffff
                } else {
                    lightness * 131586
                };
                let rest = 256 - brightness;
                let green = (gray & 0xff00)
                    .wrapping_mul(brightness)
                    .wrapping_add(rest.wrapping_mul(rgb & 0xff00))
                    & 0xff0000;
                let red_blue = brightness
                    .wrapping_mul(gray & 0xff00ff)
                    .wrapping_add((rgb & 0xff00ff).wrapping_mul(rest))
                    & 0xff00ff00u32 as i32;
                rgb = green.wrapping_add(red_blue) >> 8;
            }
            if speed != 0 {
                let factor = i32::from(speed) + 256;
                let red = ((rgb >> 16 & 0xff) * factor).min(65535);
                let green = ((rgb >> 8 & 0xff) * factor).min(65535);
                let blue = (factor * (rgb & 0xff)).min(65535);
                rgb = (blue >> 8) + (green & 0xff00) + (red << 8 & 0xff001f);
            }
        }
        rgb
    }
    /// The four corner colours of tile (x, y) for a surface: south-west,
    /// south-east, north-east, north-west.
    fn corner_colors(&self, x: usize, y: usize, texture: i32, colors: [i32; 4]) -> [i32; 4] {
        let l = self.level.lightness;
        [
            self.textured_color(texture, colors[0], l[self.tile(x, y)]),
            self.textured_color(texture, colors[1], l[self.tile(x + 1, y)]),
            self.textured_color(texture, colors[2], l[self.tile(x + 1, y + 1)]),
            self.textured_color(texture, colors[3], l[self.tile(x, y + 1)]),
        ]
    }
    fn normal(&self, corner: usize) -> [f32; 3] {
        [self.east[corner], self.up[corner], self.north[corner]]
    }
    /// `SceneGraph.computeGlTileVertex`. `packed` is south-west, south-east,
    /// north-east, north-west, each `rgb << 8 | alpha`. For the underwater
    /// floor `water` says whether the vertex takes the surface's height and
    /// gives the tile's fog range (`depthScale`).
    #[allow(clippy::too_many_arguments)]
    fn vertex(
        &self,
        tile: &mut GlTile,
        x: usize,
        y: usize,
        packed: [i32; 4],
        rotation: u8,
        blend: bool,
        water: Option<(bool, f32)>,
        mut vx: i32,
        mut vy: i32,
    ) -> u32 {
        match rotation {
            1 => (vx, vy) = (vy, 128 - vx),
            2 => (vx, vy) = (128 - vx, 128 - vy),
            3 => (vx, vy) = (128 - vy, vx),
            _ => {}
        }
        let (normal, color) = match (vx, vy) {
            (0, 0) => (self.normal(self.corner(x, y)), packed[0]),
            (128, 0) => (self.normal(self.corner(x + 1, y)), packed[1]),
            (128, 128) => (self.normal(self.corner(x + 1, y + 1)), packed[2]),
            (0, 128) => (self.normal(self.corner(x, y + 1)), packed[3]),
            _ => {
                let fy = vy as f32 / 128.0;
                let fx = vx as f32 / 128.0;
                let lerp = |values: &[f32]| {
                    let sw = values[self.corner(x, y)];
                    let near = sw + (values[self.corner(x + 1, y)] - sw) * fx;
                    let nw = values[self.corner(x, y + 1)];
                    let far = nw + (values[self.corner(x + 1, y + 1)] - nw) * fx;
                    near + fy * (far - near)
                };
                let south = smooth(vx, packed[0], packed[1]);
                let north = smooth(vx, packed[3], packed[2]);
                (
                    [lerp(&self.east), lerp(&self.up), lerp(&self.north)],
                    smooth(vy, south, north),
                )
            }
        };
        let (height, depth) = match (self.level.water, water) {
            (Some(arguments), Some((at_surface, scale))) => {
                let surface = self.interpolate(arguments.surface, x, y, vx, vy);
                let height = if at_surface {
                    surface
                } else {
                    self.interpolate_height(x, y, vx, vy)
                };
                (height, height.wrapping_sub(surface) as f32 / scale)
            }
            _ => (self.interpolate_height(x, y, vx, vy), 0.0),
        };
        tile.add_vertex(
            ((x as i32) << 7) + vx,
            height,
            vy + ((y as i32) << 7),
            normal,
            (if blend {
                color & 0xffffff00u32 as i32
            } else {
                color
            }) as u32,
            depth,
        )
    }
}

/// `SceneGraph.getSmoothedHeight`: despite the name, the blend of two packed
/// RGBA colours along a tile edge.
fn smooth(frac: i32, a: i32, b: i32) -> i32 {
    if a == b {
        return a;
    }
    let inverse = 128 - frac;
    let (a, b) = (a as u32, b as u32);
    let (frac, inverse) = (frac as u32, inverse as u32);
    let high = frac
        .wrapping_mul(b >> 7 & 0x1fe01fe)
        .wrapping_add(inverse.wrapping_mul(a >> 7 & 0x1fe01fe))
        & 0xff00ff00;
    let low = inverse
        .wrapping_mul(a & 0xff00ff)
        .wrapping_add((b & 0xff00ff).wrapping_mul(frac))
        & 0xff00ff00;
    // The original shifts a signed int; the masked sum is taken as one.
    high.wrapping_add((low as i32 >> 7) as u32) as i32
}

fn pack(colors: [i32; 4], alpha: [bool; 4]) -> [i32; 4] {
    std::array::from_fn(|i| (colors[i] << 8).wrapping_add(if alpha[i] { 255 } else { 0 }))
}

/// The loop over `neighborIds` in `buildGlShapedTiles`: at most eight
/// distinct neighbouring overlays, in first-seen order.
fn remember(ids: &mut [u8; 8], count: &mut usize, id: u8) {
    if !ids[..*count].contains(&id) && *count < 8 {
        ids[*count] = id;
        *count += 1;
    }
}

/// Builds the floor of `level`. Tiles on the outermost ring of the grid are
/// read but not built, as in the original.
pub fn build(level: &Level, tables: &Tables) -> Floor {
    let (w, d) = (level.width, level.depth);
    let corners = (w + 1) * (d + 1);
    assert!(w >= 3 && d >= 3, "a floor needs a ring of border tiles");
    for (name, len) in [
        ("underlays", level.underlays.len()),
        ("overlays", level.overlays.len()),
        ("shapes", level.shapes.len()),
        ("rotations", level.rotations.len()),
        ("flags", level.flags.len()),
        ("bridge flags", level.bridge_flags.len()),
        ("lightness", level.lightness.len()),
        ("colors", level.colors.len()),
        (
            "water data",
            level.water.map_or(w * d, |water| water.data.len()),
        ),
    ] {
        assert_eq!(len, w * d, "{name} must cover the tile grid");
    }
    assert_eq!(level.heights.len(), corners, "heights must cover corners");
    if let Some(water) = level.water {
        assert_eq!(water.surface.len(), corners, "surface must cover corners");
    }
    let mut scene = Scene {
        level,
        tables,
        up: vec![0.0; corners],
        east: vec![0.0; corners],
        north: vec![0.0; corners],
    };
    // SceneGraph.buildScene: the slope normal of corners 1..=103.
    for y in 1..d {
        for x in 1..w {
            let dy = scene.height(x, y + 1).wrapping_sub(scene.height(x, y - 1));
            let dx = scene.height(x + 1, y).wrapping_sub(scene.height(x - 1, y));
            let length = f64::from(
                dx.wrapping_mul(dx)
                    .wrapping_add(dy.wrapping_mul(dy))
                    .wrapping_add(65536),
            )
            .sqrt() as f32;
            let corner = scene.corner(x, y);
            scene.east[corner] = dx as f32 / length;
            scene.up[corner] = -256.0 / length;
            scene.north[corner] = dy as f32 / length;
        }
    }
    let mut diagonals = vec![0; w * d];
    let plain = plain(&scene, &mut diagonals);
    // The underwater buildScene stops after buildGlPlainTiles.
    let shaped = if level.water.is_some() {
        Vec::new()
    } else {
        shaped(&scene)
    };
    Floor {
        plain,
        shaped,
        diagonals,
    }
}

fn finish(groups: BTreeMap<i64, GlTile>) -> Vec<GlTile> {
    // ArrayUtils.sort orders the groups by ascending signed key.
    groups
        .into_values()
        .filter(|tile| !tile.x.is_empty())
        .map(|mut tile| {
            tile.dedup = HashMap::new();
            tile
        })
        .collect()
}

/// `SceneGraph.buildGlPlainTiles`.
fn plain(scene: &Scene, diagonals: &mut [u8]) -> Vec<GlTile> {
    let level = scene.level;
    let (w, d) = (level.width, level.depth);
    let u = |x: usize, y: usize| level.underlays[x * d + y];
    // The texture key of each corner: its own underlay, else the one to the
    // west, south, then south-west.
    let mut keys = vec![0i32; (w + 1) * (d + 1)];
    for x in 1..w {
        for y in 1..d {
            let mut id = u(x, y);
            if id == 0 {
                id = u(x - 1, y);
            }
            if id == 0 {
                id = u(x, y - 1);
            }
            if id == 0 {
                id = u(x - 1, y - 1);
            }
            if id != 0 {
                let floor = scene.underlay(id);
                keys[scene.corner(x, y)] =
                    (floor.texture.wrapping_add(1) << 16).wrapping_add(floor.scale);
            }
        }
    }
    let mut groups = BTreeMap::<i64, GlTile>::new();
    for x in 1..=w - 2 {
        for y in 1..=d - 2 {
            let underlay = u(x, y);
            if underlay == 0 {
                continue;
            }
            let tile = scene.tile(x, y);
            let render_level = scene.render_level(x, y);
            // `waterColors[tx][ty] & 0xFFFFFF` and `>>> 24 << 3`; 0 and 128
            // without the underwater arguments.
            let (water_color, depth_scale) = match level.water {
                Some(water) => (
                    water.data[tile] & 0xffffff,
                    ((water.data[tile] as u32 >> 24) << 3) as f32,
                ),
                None => (0, 128.0),
            };
            let (vertices, rotation, flags) = if level.overlays[tile] == 0 {
                let same = |x: usize, y: usize| if u(x, y) == underlay { 1i32 } else { -1 };
                let mut sw = same(x - 1, y - 1);
                let mut se = same(x + 1, y - 1);
                let south = same(x, y - 1);
                se += south;
                sw += south;
                let mut ne = same(x + 1, y + 1);
                let east = same(x + 1, y);
                ne += east;
                se += east;
                let mut nw = same(x - 1, y + 1);
                let north = same(x, y + 1);
                nw += north;
                ne += north;
                let west = same(x - 1, y);
                nw += west;
                sw += west;
                let rotation = u8::from((se - nw).abs() > (sw - ne).abs());
                diagonals[tile] = rotation;
                (UNDERLAY_SHAPE_VERTICES[0], rotation, None)
            } else {
                let vertices = OVERLAY_SHAPE_VERTICES
                    .get(usize::from(level.shapes[tile]))
                    .copied()
                    .unwrap_or_default();
                if vertices.is_empty() {
                    continue;
                }
                (
                    vertices,
                    level.rotations[tile],
                    OVERLAY_TRIANGLE_FLAGS
                        .get(usize::from(level.shapes[tile]))
                        .copied(),
                )
            };
            // South-west, south-east, north-east, north-west.
            let corner_keys = [
                keys[scene.corner(x, y)],
                keys[scene.corner(x + 1, y)],
                keys[scene.corner(x + 1, y + 1)],
                keys[scene.corner(x, y + 1)],
            ];
            let colors = [
                level.colors[tile],
                level.colors[scene.tile(x + 1, y)],
                level.colors[scene.tile(x + 1, y + 1)],
                level.colors[scene.tile(x, y + 1)],
            ];
            for (i, &key) in corner_keys.iter().enumerate() {
                if corner_keys[..i].contains(&key) {
                    continue;
                }
                let texture = (key >> 16) - 1;
                let packed = pack(
                    scene.corner_colors(x, y, texture, colors),
                    corner_keys.map(|corner| key <= corner),
                );
                let group_key = i64::from(key) << 32 | i64::from(water_color);
                let group = groups.entry(group_key).or_insert_with(|| {
                    let mut group = GlTile::new(
                        group_key,
                        texture,
                        (key & 0xffff) as f32,
                        false,
                        water_color,
                    );
                    group.underwater = level.water.is_some();
                    group
                });
                let fan = vertices
                    .chunks_exact(2)
                    .enumerate()
                    .map(|(corner, v)| {
                        // A vertex the shape flags stays on the surface.
                        let water = level.water.map(|_| {
                            let flagged = flags
                                .is_some_and(|flags| flags.get(corner).copied().unwrap_or(false));
                            (flagged, depth_scale)
                        });
                        scene.vertex(group, x, y, packed, rotation, false, water, v[0], v[1])
                    })
                    .collect();
                group.faces.push(Face {
                    level: render_level,
                    x: x as i32,
                    y: y as i32,
                    vertices: fan,
                    blend_indices: None,
                    blend: false,
                });
            }
        }
    }
    finish(groups)
}

/// `SceneGraph.getOrCreateGlTile`.
fn overlay_group<'a>(groups: &'a mut BTreeMap<i64, GlTile>, floor: &Overlay) -> &'a mut GlTile {
    let key = ((i64::from(floor.texture) + 1) << 16)
        .wrapping_add(i64::from(floor.scale))
        .wrapping_add(i64::from(floor.texture_brightness) << 56)
        .wrapping_add(i64::from(floor.water_color) << 32);
    groups.entry(key).or_insert_with(|| {
        GlTile::new(
            key,
            floor.texture,
            floor.scale as f32,
            true,
            floor.water_color as i32,
        )
    })
}

/// `SceneGraph.buildGlShapedTiles`.
fn shaped(scene: &Scene) -> Vec<GlTile> {
    let level = scene.level;
    let (w, d) = (level.width, level.depth);
    let o = |x: usize, y: usize| level.overlays[x * d + y];
    let edge = |x: usize, y: usize, turn: u8| {
        let tile = x * d + y;
        OVERLAY_EDGE_LOOKUP
            .get(
                usize::from(level.shapes[tile]) * 4
                    + usize::from(level.rotations[tile].wrapping_add(turn) & 3),
            )
            .copied()
            .unwrap_or(1)
    };
    let mut groups = BTreeMap::<i64, GlTile>::new();
    for x in 1..=w - 2 {
        for y in 1..=d - 2 {
            let tile = scene.tile(x, y);
            let underlay = level.underlays[tile];
            let overlay = level.overlays[tile];
            let render_level = scene.render_level(x, y);
            if overlay != 0 {
                let floor = scene.overlay(overlay);
                if floor.color == -1 {
                    continue;
                }
                let shape = level.shapes[tile];
                let colors = scene.corner_colors(x, y, floor.texture, [floor.color; 4]);
                let group = overlay_group(&mut groups, floor);
                overlay_face(
                    scene,
                    group,
                    [x, y],
                    render_level,
                    pack(colors, [true; 4]),
                    shape,
                    level.rotations[tile],
                    underlay != 0 && floor.blend,
                );
            }
            if !(underlay != 0 || overlay != 0 && level.shapes[tile] == 0) {
                continue;
            }
            let blends = |id: u8| {
                id != 0 && id != overlay && {
                    let floor = scene.overlay(id);
                    floor.blend && floor.color != -1
                }
            };
            let mut ids = [0u8; 8];
            let mut count = 0;
            // A diagonal neighbour counts unless both of its edges that
            // meet at the shared corner are fully transparent there.
            let mut diagonal = |nx: usize, ny: usize, first: u8, second: u8| {
                let id = o(nx, ny);
                if !blends(id) {
                    return 0;
                }
                if TILE_EDGE_MASKS[edge(nx, ny, first)][1]
                    && TILE_EDGE_MASKS[edge(nx, ny, second)][0]
                {
                    return 0;
                }
                remember(&mut ids, &mut count, id);
                id
            };
            let north_west = diagonal(x - 1, y + 1, 3, 2);
            let south_west = diagonal(x - 1, y - 1, 0, 3);
            let south_east = diagonal(x + 1, y - 1, 1, 0);
            let north_east = diagonal(x + 1, y + 1, 2, 1);
            let mut side = |nx: usize, ny: usize, turn: u8| {
                let id = o(nx, ny);
                if blends(id) {
                    remember(&mut ids, &mut count, id);
                    (id, edge(nx, ny, turn))
                } else {
                    (id, 0)
                }
            };
            let north = side(x, y + 1, 2);
            let west = side(x - 1, y, 3);
            let south = side(x, y - 1, 0);
            let east = side(x + 1, y, 1);
            for &id in &ids[..count] {
                let mask = |(neighbour, edge): (u8, usize)| {
                    TILE_EDGE_MASKS[if neighbour == id { edge } else { 0 }]
                };
                let (n, w_, s, e) = (mask(north), mask(west), mask(south), mask(east));
                let floor = scene.overlay(id);
                let colors = scene.corner_colors(x, y, floor.texture, [floor.color; 4]);
                let packed = colors.map(|c| c << 8 | 0xff);
                let clear_sw = id != south_west && s[0] && w_[1];
                let clear_ne = id != north_east && n[0] && e[1];
                let clear_nw = north_west != id && w_[0] && n[1];
                let clear_se = id != south_east && e[0] && s[1];
                let group = overlay_group(&mut groups, floor);
                let mut at = |blend: bool, vx: i32, vy: i32| {
                    scene.vertex(group, x, y, packed, 0, blend, None, vx, vy)
                };
                let center = at(true, 64, 64);
                let nw = at(clear_nw, 0, 128);
                let ne = at(clear_ne, 128, 128);
                let sw = at(clear_sw, 0, 0);
                let se = at(clear_se, 128, 0);
                let mut fan = vec![center, ne];
                if n.len() > 2 {
                    fan.push(at(n[2], 64, 128));
                }
                fan.push(nw);
                if w_.len() > 2 {
                    fan.push(at(w_[2], 0, 64));
                }
                fan.push(sw);
                if s.len() > 2 {
                    fan.push(at(s[2], 64, 0));
                }
                fan.push(se);
                if e.len() > 2 {
                    fan.push(at(e[2], 128, 64));
                }
                fan.push(ne);
                group.faces.push(Face {
                    level: render_level,
                    x: x as i32,
                    y: y as i32,
                    vertices: fan,
                    blend_indices: None,
                    blend: true,
                });
            }
        }
    }
    finish(groups)
}

/// `SceneGraph.addGlPlainTile`: the overlay polygon of a tile and, for a
/// blending overlay above an underlay, the triangles that fade it out over
/// the rest of the tile.
#[allow(clippy::too_many_arguments)]
fn overlay_face(
    scene: &Scene,
    group: &mut GlTile,
    [x, y]: [usize; 2],
    render_level: i32,
    packed: [i32; 4],
    shape: u8,
    rotation: u8,
    blend: bool,
) {
    let vertices = UNDERLAY_SHAPE_VERTICES
        .get(usize::from(shape))
        .copied()
        .unwrap_or_default();
    let v: Vec<u32> = vertices
        .chunks_exact(2)
        .map(|p| scene.vertex(group, x, y, packed, rotation, false, None, p[0], p[1]))
        .collect();
    let mut clear =
        |vx: i32, vy: i32| scene.vertex(group, x, y, packed, rotation, true, None, vx, vy);
    let blend_indices = if !blend {
        None
    } else {
        match shape {
            1 => {
                let a = clear(64, 128);
                let b = clear(128, 64);
                Some(vec![b, a, v[2], a, v[0], v[2]])
            }
            2 => {
                let a = clear(128, 128);
                let b = clear(64, 0);
                Some(vec![v[0], b, a, a, v[1], v[0]])
            }
            3 => {
                let a = clear(0, 128);
                let b = clear(64, 0);
                Some(vec![v[2], v[1], a, a, b, v[2]])
            }
            4 => {
                let a = clear(0, 128);
                Some(vec![v[3], a, v[0]])
            }
            5 => {
                let a = clear(128, 128);
                Some(vec![v[2], a, v[3]])
            }
            6 => {
                let a = clear(128, 0);
                let b = clear(128, 128);
                Some(vec![v[3], a, b, b, v[0], v[3]])
            }
            7 => {
                let a = clear(0, 128);
                let b = clear(128, 0);
                Some(vec![v[1], b, a, a, v[2], v[1]])
            }
            8 => {
                let a = clear(0, 0);
                Some(vec![v[3], a, v[4]])
            }
            9 => {
                let a = clear(128, 64);
                let b = clear(96, 32);
                let c = clear(64, 0);
                Some(vec![
                    b, a, v[4], b, v[4], v[3], b, v[3], v[2], b, v[2], v[1], b, v[1], c,
                ])
            }
            10 => {
                let a = clear(0, 128);
                Some(vec![v[2], a, v[3], v[3], a, v[4], v[4], a, v[0]])
            }
            11 => {
                let a = clear(0, 64);
                let b = clear(128, 64);
                Some(vec![
                    v[3], a, v[0], v[3], v[2], a, v[2], b, a, v[2], v[1], b,
                ])
            }
            _ => None,
        }
    };
    group.faces.push(Face {
        level: render_level,
        x: x as i32,
        y: y as i32,
        vertices: v,
        blend_indices,
        blend: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_colours_blend_per_channel_and_keep_equal_ends() {
        assert_eq!(smooth(64, 0x11223344, 0x11223344), 0x11223344);
        // Red and blue keep eight fraction bits, green and alpha seven.
        assert_eq!(smooth(64, 0, -1), 0x7f7e7f7e);
        assert_eq!(smooth(0, 0x10203040, 0x50607080), 0x10203040);
    }

    #[test]
    fn neighbour_ids_are_kept_once_in_first_seen_order() {
        let mut ids = [0; 8];
        let mut count = 0;
        for id in [5, 3, 5, 9, 3] {
            remember(&mut ids, &mut count, id);
        }
        assert_eq!((&ids[..count], count), (&[5, 3, 9][..], 3));
    }
}
