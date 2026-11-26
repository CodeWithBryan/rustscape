//! The underwater scene of the HD renderer: what RT4 loads and builds besides
//! the surface when a region has `um{x}_{z}` / `ul{x}_{z}` map files, and the
//! constants of the material it is drawn with. GPU independent.
//!
//! In the original (`LoginManager.rebuildMap`, `SceneGraph`):
//!
//! - With the GL renderer and high water detail, a scene has an underwater
//!   map when any of its regions has either file. `SceneGraph.init` then
//!   allocates a second, one-level set of tiles and heights and the
//!   `tileWaterData` grid.
//! - The surface `buildScene` fills `tileWaterData` for ground-level tiles:
//!   the water colour and opacity of the tile's own water overlay, or of the
//!   first water overlay among its eight neighbours ([`water_data`]).
//! - `readTerrain(underwater)` reads one level of tiles from each `um` file:
//!   underlay, overlay, shape and rotation as on the surface, and a height
//!   that is a drop below the surface corner ([`Layer`], [`Scene::read`]).
//!   Render flags are not read. A region without the file is cleared like a
//!   surface region without terrain ([`Scene::clear`]).
//! - `readLocs(highmem)` adds the `ul` locations to the underwater tiles:
//!   never interactive, without shadows, tile shade, occluders or merged
//!   normals, their terrain alignment reading the surface as the level above.
//!   Scenery spreads the water data of its footprint ([`Scene::place`]).
//! - The underwater `buildScene` builds only `buildGlPlainTiles`, with the
//!   water data and the surface heights as its two extra arguments
//!   (`gl_tile::Water`): no overlay polygons, no light meshes.
//! - Each frame the underwater scene is drawn before the surface
//!   (`SceneGraph.setPlainTile`): first `applyFogFade` paints the surface's
//!   ground-level water faces in their plain colour wherever an underwater
//!   tile exists, without depth test or depth writes; then the floor groups
//!   at depth layer 251 and the scenery at 201.5, all under material 3
//!   (`UnderwaterMaterialRenderer`), which no texture can replace while
//!   `renderingUnderwater` is set. The surface follows, its water faces
//!   blended over the result with depth writes on.
//!
//! Units are the original's: 128 per tile, heights negative upwards.
use crate::{
    floor::{self, Overlay},
    gl_tile,
    interaction::LocationType,
    loc_placement::{self, Bounds, Finish, Heights, Plan, Planned},
    location::Definition,
    model::Model,
    terrain::{OcclusionLevel, Tile},
    texture::Material,
    tile_shade::ShadowMap,
};
use rs09_protocol::{Result, buffer::Reader};

/// The underwater files of one map region.
#[derive(Clone, Debug, Default)]
pub struct Layer {
    /// `um{x}_{z}`: 64 x 64 tiles, `x * 64 + z`. A tile's `height` is its drop
    /// below the surface corner (`g1 * 8`); `flags` stay zero, because the
    /// underwater read leaves the render flags alone. None without the file.
    pub tiles: Option<Vec<Tile>>,
    /// Opcode 129 records after the tiles. `readTerrain` applies them to the
    /// surface's occlusion heights, and a file without any clears a block of
    /// them (`follow_camera::Occlusion::read`). No file of the pinned cache
    /// has one.
    pub occlusion: Vec<[OcclusionLevel; 4]>,
    /// `ul{x}_{z}`: `[id, x, z, level, shape, rotation]` at absolute tiles, in
    /// file order. The file is not encrypted.
    pub locations: Vec<[i32; 6]>,
}

/// What is known about a region's underwater files.
#[derive(Clone, Debug, Default)]
pub enum Source {
    /// The region came from somewhere that does not say (a pack made before
    /// underwater maps were packed, an instanced scene).
    #[default]
    Unknown,
    /// The cache has neither file for the region.
    Absent,
    Files(Layer),
}
impl Source {
    pub fn layer(&self) -> Option<&Layer> {
        match self {
            Self::Files(layer) => Some(layer),
            _ => None,
        }
    }
    /// From the two files as the cache has them.
    pub fn decode(
        region: [i32; 2],
        terrain: Option<&[u8]>,
        locations: Option<&[u8]>,
    ) -> Result<Self> {
        if terrain.is_none() && locations.is_none() {
            return Ok(Self::Absent);
        }
        Ok(Self::Files(Layer::decode(region, terrain, locations)?))
    }
}

impl Layer {
    /// `SceneGraph.readTerrain(.., underwater = true, ..)` and
    /// `SceneGraph.readLocs` over the raw files of region `(x, z)`.
    pub fn decode(
        region: [i32; 2],
        terrain: Option<&[u8]>,
        locations: Option<&[u8]>,
    ) -> Result<Self> {
        let mut layer = Self::default();
        if let Some(data) = terrain {
            let mut r = Reader::new(data);
            let mut tiles = vec![Tile::default(); 64 * 64];
            for tile in &mut tiles {
                // SceneGraph.readTile with `underwater` set.
                loop {
                    match r.g1()? {
                        0 => break,
                        1 => {
                            // Unlike the surface, a height of 1 stays 1.
                            tile.height = r.g1()? * 8;
                            break;
                        }
                        op @ 2..=49 => {
                            tile.overlay = r.g1()? as u8;
                            tile.shape = ((op - 2) / 4) as u8;
                            tile.rotation = ((op - 2) & 3) as u8;
                        }
                        50..=81 => {}
                        op => tile.underlay = (op - 81) as u8,
                    }
                }
            }
            layer.tiles = Some(tiles);
            // The environment and light entries that may follow a surface map
            // are not read underwater; the loop stops at the first of them.
            while r.remaining() > 0 && data[r.position()] == 129 {
                r.g1()?;
                let mut record: [OcclusionLevel; 4] = Default::default();
                for level in &mut record {
                    level.kind = r.g1b()? as i8;
                    if level.kind == 1 {
                        level.heights = r.take(256)?.to_vec();
                    }
                }
                layer.occlusion.push(record);
            }
        }
        if let Some(data) = locations {
            let mut r = Reader::new(data);
            let mut id = -1i32;
            loop {
                let offset = r.g_var_smart()?;
                if offset == 0 {
                    break;
                }
                id = id.wrapping_add(offset);
                let mut position = 0i32;
                loop {
                    let offset = r.gsmarts()?;
                    if offset == 0 {
                        break;
                    }
                    position = position.wrapping_add(offset - 1);
                    let flags = r.g1()?;
                    layer.locations.push([
                        id,
                        region[0] * 64 + (position >> 6 & 63),
                        region[1] * 64 + (position & 63),
                        position >> 12,
                        flags >> 2,
                        flags & 3,
                    ]);
                }
            }
        }
        Ok(layer)
    }
}

/// `tileWaterData` as the surface `buildScene` leaves it: for every
/// ground-level tile `1..=width - 2` by `1..=depth - 2` with an underlay or an
/// overlay, `waterColor + (waterOpacity << 24)` of its own water overlay, else
/// of the first water overlay among its eight neighbours (west column first,
/// south to north). Zero elsewhere.
///
/// `underlays` and `overlays` are the surface's ground level on the tile grid
/// (`x * depth + y`); `water` says whether a texture has the water material.
pub fn water_data(
    width: usize,
    depth: usize,
    underlays: &[u8],
    overlays: &[u8],
    floors: &[Overlay],
    water: impl Fn(i32) -> bool,
) -> Vec<i32> {
    let mut data = vec![0i32; width * depth];
    // The original would fail on an overlay past the definition table.
    let of = |id: u8| {
        floors
            .get(usize::from(id).wrapping_sub(1))
            .filter(|floor| floor.texture != -1 && water(floor.texture))
            .map(|floor| {
                (floor.water_color as i32).wrapping_add(i32::from(floor.water_opacity) << 24)
            })
    };
    for x in 1..width.saturating_sub(1) {
        for y in 1..depth.saturating_sub(1) {
            let tile = x * depth + y;
            if underlays[tile] == 0 && overlays[tile] == 0 {
                continue;
            }
            if let Some(own) = (overlays[tile] != 0).then(|| of(overlays[tile])).flatten() {
                data[tile] = own;
                continue;
            }
            'search: for nx in x - 1..=x + 1 {
                for ny in y - 1..=y + 1 {
                    if (nx, ny) == (x, y) || nx >= width || ny >= depth {
                        continue;
                    }
                    let neighbour = overlays[nx * depth + ny];
                    if let Some(value) = (neighbour != 0).then(|| of(neighbour)).flatten() {
                        data[tile] = value;
                        break 'search;
                    }
                }
            }
        }
    }
    data
}

/// The colour of a `tileWaterData` value.
pub fn water_color(data: i32) -> u32 {
    data as u32 & 0xffffff
}
/// `tileWaterData >>> 24 << 3`: the depth over which the underwater material
/// fades to the water colour (`MaterialManager.setUnderwaterFogRange`).
pub fn fog_range(data: i32) -> i32 {
    ((data as u32 >> 24) << 3) as i32
}

/// The underwater half of a scene: `underWaterGroundTiles`,
/// `underwaterTileHeights` and the map arrays `SceneGraph.load(true)`
/// allocates, over a grid of any size (the original's is 104 x 104).
///
/// Tile grids are x-major, `x * depth + y`; corner grids `x * (depth + 1) + y`.
pub struct Scene {
    pub width: usize,
    pub depth: usize,
    pub underlays: Vec<u8>,
    pub overlays: Vec<u8>,
    pub shapes: Vec<u8>,
    pub rotations: Vec<u8>,
    /// `underwaterTileHeights[0]`, per corner.
    pub heights: Vec<i32>,
    /// `tileWaterData`, which underwater scenery adds to.
    pub water: Vec<i32>,
    /// `shadowmap` of the underwater load. Locations do not shade it; only
    /// regions without terrain do. Level 0 is the only one read.
    pub shade: ShadowMap,
    /// Whether `underWaterGroundTiles[0][x][y]` is a tile object: the tiles
    /// `applyFogFade` and `GlTile.renderTiles` draw faces for.
    pub tiles: Vec<bool>,
    /// `Tile.sceneryLen`.
    scenery: Vec<u8>,
}

impl Scene {
    /// `SceneGraph.init(.., true)` and `load(true)` after the surface was
    /// built: `water` is the surface's [`water_data`].
    pub fn new(width: usize, depth: usize, water: Vec<i32>) -> Self {
        assert_eq!(
            water.len(),
            width * depth,
            "water data must cover the tiles"
        );
        Self {
            width,
            depth,
            underlays: vec![0; width * depth],
            overlays: vec![0; width * depth],
            shapes: vec![0; width * depth],
            rotations: vec![0; width * depth],
            heights: vec![0; (width + 1) * (depth + 1)],
            water,
            shade: ShadowMap::new(width, depth),
            tiles: vec![false; width * depth],
            scenery: vec![0; width * depth],
        }
    }
    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.depth
    }
    fn tile(&self, x: i32, y: i32) -> usize {
        x as usize * self.depth + y as usize
    }
    fn corner(&self, x: i32, y: i32) -> usize {
        x as usize * (self.depth + 1) + y as usize
    }

    /// `readTerrain(underwater)` for the map file whose south-west tile is
    /// scene tile `origin`. `surface` is `surfaceTileHeights[0]` on the
    /// corner grid.
    pub fn read(&mut self, origin: [i32; 2], tiles: &[Tile], surface: &[i32]) {
        for lx in 0..64 {
            for ly in 0..64 {
                let (x, y) = (origin[0] + lx, origin[1] + ly);
                let Some(tile) = tiles.get((lx * 64 + ly) as usize) else {
                    return;
                };
                if !self.inside(x, y) {
                    continue;
                }
                let corner = self.corner(x, y);
                self.heights[corner] = surface[corner].wrapping_add(tile.height);
                let index = self.tile(x, y);
                // An opcode the tile lacks leaves the array as `load` made it.
                self.underlays[index] = tile.underlay;
                self.overlays[index] = tile.overlay;
                self.shapes[index] = tile.shape;
                self.rotations[index] = tile.rotation;
            }
        }
    }

    /// `clearTerrainRegion(0, ..)`, which `readStaticTerrain(true)` runs after
    /// every file was read for each region of the scene without one (while
    /// the scene is on the surface map, `centralZoneY < 800`): the region is
    /// fully shaded, its heights are zero, and its west column and south row
    /// continue the neighbours' heights.
    pub fn clear(&mut self, origin: [i32; 2]) {
        let [x, y] = origin;
        self.shade.clear_region(0, x, y, 64, 64);
        for tx in x..x + 64 {
            for ty in y..y + 64 {
                if self.inside(tx, ty) {
                    let corner = self.corner(tx, ty);
                    self.heights[corner] = 0;
                }
            }
        }
        let inside = |n: i32, size: usize| n > 0 && (n as usize) < size;
        if inside(x, self.width) {
            for ty in y + 1..y + 64 {
                if ty >= 0 && (ty as usize) < self.depth {
                    let (to, from) = (self.corner(x, ty), self.corner(x - 1, ty));
                    self.heights[to] = self.heights[from];
                }
            }
        }
        if inside(y, self.depth) {
            for tx in x + 1..x + 64 {
                if tx >= 0 && (tx as usize) < self.width {
                    let (to, from) = (self.corner(tx, y), self.corner(tx, y - 1));
                    self.heights[to] = self.heights[from];
                }
            }
        }
        if !self.inside(x, y) {
            return;
        }
        let at = |scene: &Self, tx: i32, ty: i32| scene.heights[scene.corner(tx, ty)];
        let corner = self.corner(x, y);
        if x > 0 && at(self, x - 1, y) != 0 {
            self.heights[corner] = at(self, x - 1, y);
        } else if y > 0 && at(self, x, y - 1) != 0 {
            self.heights[corner] = at(self, x, y - 1);
        } else if x > 0 && y > 0 && at(self, x - 1, y - 1) != 0 {
            self.heights[corner] = at(self, x - 1, y - 1);
        }
    }

    /// What `addLoc(.., highmem = true, ..)` leaves on the tiles for a
    /// location that has an entity: `size` is its footprint after the quarter
    /// turn. Walls, decorations and ground decoration make their tile exist;
    /// scenery (shapes 9 and above) takes a slot on every tile of its
    /// footprint unless it leaves the scene or a tile already holds five, and
    /// then gives the footprint's zero tiles the water data of its last tile
    /// that has any (`addSceneryEntity`). Returns whether it entered the scene.
    pub fn place(&mut self, x: i32, y: i32, shape: i32, size: [i32; 2]) -> bool {
        if !(9..22).contains(&shape) {
            if !self.inside(x, y) {
                return false;
            }
            let index = self.tile(x, y);
            self.tiles[index] = true;
            return true;
        }
        let size = if matches!(shape, 10 | 11) {
            size
        } else {
            [1, 1]
        };
        let footprint =
            || (x..x + size[0]).flat_map(move |tx| (y..y + size[1]).map(move |ty| (tx, ty)));
        if footprint().any(|(tx, ty)| !self.inside(tx, ty) || self.scenery[self.tile(tx, ty)] >= 5)
        {
            return false;
        }
        let mut water = 0;
        for (tx, ty) in footprint() {
            let index = self.tile(tx, ty);
            self.tiles[index] = true;
            self.scenery[index] += 1;
            if self.water[index] != 0 {
                water = self.water[index];
            }
        }
        if water != 0 {
            for (tx, ty) in footprint() {
                let index = self.tile(tx, ty);
                if self.water[index] == 0 {
                    self.water[index] = water;
                }
            }
        }
        true
    }

    /// The tile loop of the underwater `buildScene`: `setTile` makes a tile
    /// object for every tile `1..=width - 2` by `1..=depth - 2` that has an
    /// underlay or an overlay.
    pub fn finish(&mut self) {
        for x in 1..self.width.saturating_sub(1) {
            for y in 1..self.depth.saturating_sub(1) {
                let index = x * self.depth + y;
                if self.underlays[index] != 0 || self.overlays[index] != 0 {
                    self.tiles[index] = true;
                }
            }
        }
    }

    /// `readLocs(.., highmem = true, ..)` over the scene's `ul` placements,
    /// `[id, x, y, level, shape, rotation]` at scene tiles in file order:
    /// what `addLoc` leaves in the scene for each, None for one that leaves
    /// nothing. `lookup` gives a type's definition, None for one the cache
    /// lacks.
    ///
    /// Only placements inside the scene's outer ring are added, as in the
    /// original. One on a level above the ground makes the original fail
    /// (the underwater height array has a single level) and is skipped here.
    /// An animated or multi-location type would be a `Loc` resolved every
    /// frame; no underwater file of the pinned cache places one, and none is
    /// built here.
    pub fn add_locations<'a>(
        &mut self,
        placements: &[[i32; 6]],
        lookup: impl Fn(i32) -> Option<(&'a Definition, &'a LocationType)>,
        ground_decorations: bool,
    ) -> Vec<Option<Planned>> {
        let admitted = |row: &[i32; 6]| {
            let [_, x, y, level, _, _] = *row;
            level == 0 && x > 0 && y > 0 && x < self.width as i32 - 1 && y < self.depth as i32 - 1
        };
        let rows: Vec<[i32; 6]> = placements
            .iter()
            .map(|row| {
                if admitted(row) {
                    *row
                } else {
                    // A shape `addLoc` has no branch for: nothing is planned.
                    [row[0], row[1], row[2], 0, -1, 0]
                }
            })
            .collect();
        let mut entries = Plan::new(
            &rows,
            |id| lookup(id).filter(|(d, kind)| !loc_placement::is_entity(d, kind)),
            ground_decorations,
        )
        .entries;
        for (entry, &[_, x, y, _, shape, _]) in entries.iter_mut().zip(&rows) {
            if let Some(planned) = entry
                && !self.place(x, y, shape, planned.placement.size)
            {
                *entry = None;
            }
        }
        entries
    }

    fn height(&self, x: i32, y: i32) -> i32 {
        if x >= 0 && y >= 0 && x as usize <= self.width && y as usize <= self.depth {
            self.heights[self.corner(x, y)]
        } else {
            0
        }
    }

    /// The models `addLoc` builds for one planned placement at scene tile
    /// `tile`: `LocType.getStaticEntity` with the underwater heights as the
    /// entity's level and `surface` (`surfaceTileHeights[0]`, on the corner
    /// grid) as the level above it. Err names a model `fetch` lacks.
    #[allow(clippy::too_many_arguments)]
    pub fn entities<'a>(
        &self,
        surface: &[i32],
        tile: [i32; 2],
        planned: &Planned,
        d: &Definition,
        kind: &LocationType,
        materials: &[Option<Material>],
        fetch: impl Fn(u32) -> Option<&'a Model>,
    ) -> std::result::Result<Vec<Entity>, u32> {
        let [x, y] = tile;
        let placement = &planned.placement;
        let [width, length] = placement.size;
        // A location overhanging the scene's far edge reads the corners of
        // its own tile.
        let (west, east) = if x + width <= self.width as i32 {
            (x + (width >> 1), x + ((width + 1) >> 1))
        } else {
            (x, x + 1)
        };
        let (south, north) = if length + y > self.depth as i32 {
            (y, y + 1)
        } else {
            ((length >> 1) + y, y + ((length + 1) >> 1))
        };
        let base = self
            .height(west, north)
            .wrapping_add(self.height(east, south))
            .wrapping_add(self.height(west, south))
            .wrapping_add(self.height(east, north))
            >> 2;
        let center = [(width << 6) + (x << 7), (length << 6) + (y << 7)];
        let ground = |x: i32, y: i32| self.height(x, y);
        let above = |x: i32, y: i32| {
            if x >= 0 && y >= 0 && x as usize <= self.width && y as usize <= self.depth {
                surface[self.corner(x, y)]
            } else {
                0
            }
        };
        let heights = Heights {
            size: [self.width as i32 + 1, self.depth as i32 + 1],
            ground: &ground,
            above: Some(&above),
        };
        let mut out = Vec::new();
        for (piece, entity) in placement.pieces.iter().enumerate() {
            let Some(entity) = entity else {
                continue;
            };
            let Some(mut model) = loc_placement::gl_model(
                d,
                entity.shape,
                entity.rotation,
                false,
                materials,
                &fetch,
            )?
            else {
                continue;
            };
            let finish = Finish::new(kind, d, *entity);
            finish.turn(&mut model);
            let bounds = Bounds::of(&model);
            finish.align(&mut model, bounds, &heights, center, base);
            out.push(Entity {
                piece,
                model,
                draw: [
                    (x << 7) + placement.origin[0] + entity.offset[0],
                    (y << 7) + placement.origin[1] + entity.offset[1],
                ],
                offset: entity.offset,
                base,
            });
        }
        Ok(out)
    }

    /// The underwater `buildScene` after its tile loop: the blended underlay
    /// colours, the tile lightness and `buildGlPlainTiles` with the water
    /// data and the surface heights. `flags` are the surface's
    /// `renderFlags[0]` and `renderFlags[1]`, which the underwater read
    /// leaves in place.
    pub fn floor(
        &self,
        surface: &[i32],
        flags: [&[u8]; 2],
        tables: &gl_tile::Tables,
    ) -> gl_tile::Floor {
        let colors: Vec<i32> =
            floor::blend(self.width, self.depth, &self.underlays, tables.underlays)
                .into_iter()
                .map(i32::from)
                .collect();
        gl_tile::build(
            &gl_tile::Level {
                width: self.width,
                depth: self.depth,
                level: 0,
                underlays: &self.underlays,
                overlays: &self.overlays,
                shapes: &self.shapes,
                rotations: &self.rotations,
                flags: flags[0],
                bridge_flags: flags[1],
                heights: &self.heights,
                // buildScene's loop without high detail lighting reads the
                // underwater heights, which are the scene's while it is built.
                lightness: &if crate::sd_light::enabled() {
                    self.shade
                        .lightness_sd(0, &self.heights, crate::sd_light::sun())
                } else {
                    self.shade.lightness(0)
                },
                colors: &colors,
                water: Some(gl_tile::Water {
                    data: &self.water,
                    surface,
                }),
            },
            tables,
        )
    }

    /// `(tileWaterData colour, fog range, material argument)` of the tile a
    /// piece of underwater scenery is drawn with (`SceneGraph.renderTile`):
    /// the argument is minus the mean of the surface's four corner heights.
    pub fn scenery_material(&self, x: i32, y: i32, surface: &[i32]) -> (u32, i32, i32) {
        if !self.inside(x, y) {
            return (0, 0, 0);
        }
        let data = self.water[self.tile(x, y)];
        let mean = surface[self.corner(x, y)]
            .wrapping_add(surface[self.corner(x + 1, y)])
            .wrapping_add(surface[self.corner(x, y + 1)])
            .wrapping_add(surface[self.corner(x + 1, y + 1)])
            >> 2;
        (water_color(data), fog_range(data), mean.wrapping_neg())
    }
}

/// One entity of an underwater location as the scene holds it.
#[derive(Clone, Debug)]
pub struct Entity {
    /// Which of the placement's entities this is (a corner wall and a double
    /// decoration have two).
    pub piece: usize,
    /// The model after `getGlModel`, the diagonal turn and the terrain
    /// alignment; its vertices are relative to `draw` and `base`.
    pub model: Model,
    /// Where the scene draws it, in scene fine units: the slot's `xFine` and
    /// `yFine` plus a wall decoration's offset.
    pub draw: [i32; 2],
    /// The wall decoration's offset inside `draw`.
    pub offset: [i32; 2],
    /// The slot's `zFine`: `averageY` of the underwater heights.
    pub base: i32,
}

/// How one part of the underwater scene is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pass {
    /// `UnderwaterMaterialRenderer.applyFogFade`: the surface's ground-level
    /// water faces in their plain water colour, unlit and untextured, fogged
    /// from [`crate::materials::BACKDROP_FOG_START`], with neither depth test
    /// nor depth writes. Nothing else has been drawn when it runs.
    Backdrop,
    /// Floor groups of one water colour: material 3 with argument -1 (the
    /// depth coordinate comes with the vertices), depth layer [`FLOOR_LAYER`].
    Floor { water: u32 },
    /// Scenery of tiles with one water colour, fog range and surface height:
    /// material 3 with `height` as its argument, depth layer [`SCENERY_LAYER`].
    Scenery { water: u32, range: i32, height: i32 },
}

/// `251.5F - (hdTile.blend ? 1.0F : 0.5F)` for the underwater floor groups,
/// none of which blend.
pub const FLOOR_LAYER: f32 = 251.0;
/// `depthLayer = 201.5F` for every underwater tile's entities.
pub const SCENERY_LAYER: f32 = 201.5;
/// `201.5F - 1.0F`: the surface's ground-level overlay groups, the water
/// among them, which the client's static renderer draws without a bias.
pub const SURFACE_WATER_LAYER: f32 = 200.5;

/// `ClientProt.ping(true)` calls of one `LoginManager.rebuildMap`: after the
/// collection, the terrain read, the unload and the light grid, and three
/// more around the underwater read and build.
pub fn rebuild_keep_alives(has_underwater_map: bool) -> u32 {
    if has_underwater_map { 7 } else { 4 }
}

/// `UnderwaterMaterialRenderer`'s alpha gradient: eight `GL_ALPHA` texels.
/// (`alphaData[i++] = (byte) (i * 159 / 8 + 96)` takes the index before the
/// increment and the value after it.)
pub fn ramp_texels() -> [u8; 8] {
    std::array::from_fn(|i| ((i as i32 + 1) * 159 / 8 + 96) as u8)
}

/// The gradient at texture coordinate `s`, as `GL_LINEAR` with
/// `GL_CLAMP_TO_EDGE` samples it: how far the colour has gone to the water
/// colour. 115 / 255 at the surface and above, 1 from 15 / 16 of the range.
pub fn ramp(s: f32) -> f32 {
    let texels = ramp_texels();
    let u = (s * 8.0 - 0.5).clamp(0.0, 7.0);
    let low = u.floor();
    let high = (low + 1.0).min(7.0);
    let fraction = u - low;
    (f32::from(texels[low as usize]) * (1.0 - fraction)
        + f32::from(texels[high as usize]) * fraction)
        / 255.0
}

/// The gradient coordinate of underwater scenery at height `y`: the unit's
/// eye-linear S plane is `(0, 1 / range, 0, argument / range)` in scene
/// space, the argument being minus the surface height. A range of zero
/// divides by zero, as in the original.
pub fn scenery_depth(y: f32, argument: i32, range: i32) -> f32 {
    y * (1.0 / range as f32) + argument as f32 * 1.0 / range as f32
}

/// The coordinate of the ridged noise volume the material adds to the colour
/// (`GL_ADD` on unit 1): S and T planes `(0.001, 0.0009, 0, 0)` and
/// `(0, 0.0009, 0.001, 0)` over the scene position, R the clock.
pub fn caustics([x, y, z]: [f32; 3], clock: i32) -> [f32; 3] {
    [
        0.001f32 * x + 9.0e-4f32 * y,
        9.0e-4f32 * y + 0.001f32 * z,
        clock as f32 * 0.005f32,
    ]
}

/// `WaterMaterialRenderer.getWaterFogColor`: the colour the material
/// interpolates to and fogs with, from the tile's water colour, the
/// atmosphere's light colour, and its ambient and first diffuse strengths.
pub fn water_fog(water: u32, light: i32, ambient: f32, diffuse: f32) -> [f32; 3] {
    let sum = ambient + diffuse;
    let channel = |color: u32, shift: u32| (color >> shift & 0xff) as f32 / 255.0f32;
    let light = light as u32;
    let scale = 0.58823526f32;
    [
        channel(water, 16) * channel(light, 16) * scale * sum,
        sum * scale * channel(light, 8) * channel(water, 8),
        channel(water, 0) * channel(light, 0) * scale * sum,
    ]
}

/// `ColorUtils.getRgbFloat`: the colour `applyFogFade` paints a water tile
/// with. Lighting is off there, so the atmosphere's light is in the colour;
/// it is [`water_fog`]'s colour computed in another order.
pub fn backdrop_color(water: u32, light: i32, ambient: f32, diffuse: f32) -> [f32; 3] {
    let total = ambient + diffuse;
    let scale = 0.58823526f32;
    let light = light as u32;
    let of = |color: u32, shift: u32| (color >> shift & 0xff) as f32;
    let (light_r, light_g, light_b) = (
        of(light, 16) / 255.0f32,
        of(light, 8) / 255.0f32,
        of(light, 0) / 255.0f32,
    );
    [
        total * scale * light_r * (of(water, 16) / 255.0f32),
        total * of(water, 8) / 255.0f32 * light_g * scale,
        of(water, 0) / 255.0f32 * light_b * scale * total,
    ]
}

/// The colour of the material for one fragment: `lit` is the lit, textured
/// colour of unit 0, `noise` the filtered luminance of the ridged volume at
/// [`caustics`], `depth` the gradient coordinate and `fog` [`water_fog`].
/// Unit 1 adds the noise, unit 2 interpolates to the fog colour by the
/// gradient's alpha; the alpha of unit 0 passes through both.
pub fn shade(lit: [f32; 3], noise: f32, depth: f32, fog: [f32; 3]) -> [f32; 3] {
    let t = ramp(depth);
    std::array::from_fn(|i| {
        let added = (lit[i] + noise).clamp(0.0, 1.0);
        fog[i] * t + added * (1.0 - t)
    })
}

/// What one pass of the underwater scene sets differently from the surface in
/// the world shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Uniforms {
    /// Ambient light and the two directional lights.
    pub lights: [[f32; 3]; 3],
    /// Fog start and end, in tiles.
    pub fog_range: [f32; 2],
    pub fog_color: [f32; 3],
    /// `materials::view`'s uniform for the pass.
    pub material_view: [f32; 4],
    /// The pass's depth layer as the world shader's `DepthBias`: an absolute
    /// layer (`depth_layers::offset::absolute`), since the pass's layer does
    /// not come from its vertices. The backdrop is on layer 0, no shift.
    pub depth_bias: [f32; 2],
    pub depth_write: bool,
}

/// The uniforms of `pass` under the frame's atmosphere.
///
/// - Backdrop: lighting off (`GlRenderer.setLightingEnabled(false)`) and
///   [`backdrop_color`] as the one colour. The vertices carry the water
///   colour and the ambient light the factor that makes it that colour; the
///   frame's fog colour with the start `applyFogFade` sets. Nothing is drawn before it, so its disabled depth
///   test is the enabled one against a cleared buffer.
/// - Floor and scenery: the frame's lights, fog toward
///   `getWaterFogColor()` (`FogManager.setFogColor`) with the frame's range,
///   and material 3 on every face.
pub fn uniforms(pass: Pass, atmosphere: &crate::atmosphere::Parameters) -> Uniforms {
    use crate::{depth_layers::offset, materials};
    let bias = offset::absolute;
    let range = atmosphere
        .fog_range(materials::VIEW_DISTANCE, materials::VIEW_FADE_DISTANCE)
        .map(|n| n / 128.0);
    let fog = |water: u32| {
        water_fog(
            water,
            atmosphere.color,
            atmosphere.ambient,
            atmosphere.diffuse[0],
        )
    };
    match pass {
        Pass::Backdrop => Uniforms {
            lights: [
                backdrop_color(
                    0xffffff,
                    atmosphere.color,
                    atmosphere.ambient,
                    atmosphere.diffuse[0],
                ),
                [0.0; 3],
                [0.0; 3],
            ],
            fog_range: [materials::BACKDROP_FOG_START / 128.0, range[1]],
            fog_color: crate::atmosphere::rgb(atmosphere.fog_color),
            material_view: [0.0, 1.0, 1.0, 0.0],
            depth_bias: bias(0.0),
            depth_write: false,
        },
        Pass::Floor { water } => Uniforms {
            lights: atmosphere.light_colors(),
            fog_range: range,
            fog_color: fog(water),
            material_view: [0.0, 0.0, 1.0, f32::from(materials::UNDERWATER)],
            depth_bias: bias(FLOOR_LAYER),
            depth_write: true,
        },
        Pass::Scenery {
            water,
            range: fade,
            height,
        } => Uniforms {
            lights: atmosphere.light_colors(),
            fog_range: range,
            fog_color: fog(water),
            material_view: [
                fade as f32,
                height as f32,
                1.0,
                f32::from(materials::UNDERWATER),
            ],
            depth_bias: bias(SCENERY_LAYER),
            depth_write: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests: the expected values are worked by hand from
    // SceneGraph.readTile, buildScene, addSceneryEntity, clearTerrainRegion
    // and UnderwaterMaterialRenderer.

    #[test]
    fn a_tile_drops_below_the_surface_and_keeps_height_one() {
        // Tile 0: underlay 3 (opcode 84), overlay 42 shape 2 rotation 1
        // (opcode 2 + 2 * 4 + 1), drop 1. Tile 1: flags (ignored), level.
        let mut um = vec![84, 11, 42, 1, 1, 60, 0];
        um.resize(um.len() + 4094, 0);
        let mut ul = Vec::new();
        // Location 5 at local (1, 3) level 0, shape 10 rotation 3.
        ul.extend([6, 64 + 3 + 1, 10 << 2 | 3, 0, 0]);
        let layer = Layer::decode([50, 50], Some(&um), Some(&ul)).unwrap();
        let tiles = layer.tiles.as_ref().unwrap();
        assert_eq!(
            (
                tiles[0].underlay,
                tiles[0].overlay,
                tiles[0].shape,
                tiles[0].rotation,
                tiles[0].height
            ),
            (3, 42, 2, 1, 8)
        );
        assert_eq!((tiles[1].flags, tiles[1].height), (0, 0));
        assert_eq!(layer.locations, vec![[5, 3201, 3203, 0, 10, 3]]);
        assert!(layer.occlusion.is_empty());
        assert!(matches!(
            Source::decode([0, 0], None, None).unwrap(),
            Source::Absent
        ));
        assert!(Layer::decode([0, 0], Some(&[1]), None).is_err());

        let mut scene = Scene::new(4, 4, vec![0; 16]);
        let surface: Vec<i32> = (0..25).map(|i| -100 - i).collect();
        scene.read([1, 1], tiles, &surface);
        // Scene tile (1, 1) is the file's tile 0; corner 1 * 5 + 1.
        assert_eq!(scene.heights[6], -106 + 8);
        assert_eq!(scene.heights[7], -107);
        assert_eq!((scene.underlays[5], scene.overlays[5]), (3, 42));
        // Corners outside the file stay as `init` left them.
        assert_eq!(scene.heights[0], 0);
    }

    #[test]
    fn water_data_is_the_tiles_own_overlay_or_the_first_neighbours() {
        let floors = vec![
            Overlay {
                texture: 7,
                water_color: 0x122b3d,
                water_opacity: 16,
                ..Overlay::default()
            },
            Overlay {
                texture: 9,
                ..Overlay::default()
            },
            Overlay {
                texture: 7,
                water_color: 0x010203,
                water_opacity: 200,
                ..Overlay::default()
            },
        ];
        let (w, d) = (6, 6);
        let mut underlays = vec![1u8; w * d];
        let mut overlays = vec![0u8; w * d];
        overlays[2 * d + 2] = 1; // water
        overlays[3 * d + 3] = 3; // other water, north-east of it
        overlays[4 * d + 1] = 2; // not water
        underlays[4 * d + 4] = 0; // a hole
        let data = water_data(w, d, &underlays, &overlays, &floors, |texture| texture == 7);
        let at = |x: usize, y: usize| data[x * d + y];
        assert_eq!(at(2, 2), 0x122b3d + (16 << 24));
        assert_eq!(
            (fog_range(at(2, 2)), water_color(at(2, 2))),
            (128, 0x122b3d)
        );
        // An opacity above 127 wraps the int and still reads back.
        assert_eq!(
            (fog_range(at(3, 3)), water_color(at(3, 3))),
            (1600, 0x010203)
        );
        // (3, 2) sees (2, 2) before (3, 3): the west column is searched first.
        assert_eq!(at(3, 2), at(2, 2));
        // (4, 2) sees (3, 3) only.
        assert_eq!(at(4, 2), at(3, 3));
        // A non-water overlay takes its neighbours' data like a plain tile.
        assert_eq!(at(4, 1), 0);
        assert_eq!(at(3, 1), at(2, 2));
        // No floor, no data; the outer ring is never visited.
        assert_eq!(at(4, 4), 0);
        assert_eq!(at(1, 1), at(2, 2));
        assert_eq!(at(0, 1), 0);
    }

    #[test]
    fn scenery_spreads_water_data_and_a_sixth_entity_is_refused() {
        let mut water = vec![0; 36];
        water[2 * 6 + 2] = 5;
        water[3 * 6 + 3] = 9;
        let mut scene = Scene::new(6, 6, water);
        // A 2 x 2 centrepiece over (2, 2)..(3, 3): the last tile with data wins.
        assert!(scene.place(2, 2, 10, [2, 2]));
        assert_eq!(
            [
                scene.water[14],
                scene.water[15],
                scene.water[20],
                scene.water[21]
            ],
            [5, 9, 9, 9]
        );
        assert!(scene.tiles[15] && !scene.tiles[0]);
        // A diagonal wall is scenery of one tile whatever its type's size.
        for _ in 0..4 {
            assert!(scene.place(2, 2, 9, [3, 3]));
        }
        assert!(!scene.place(2, 2, 10, [1, 1]));
        assert!(!scene.place(5, 5, 10, [2, 1]));
        // Walls and ground decoration only make the tile exist.
        assert!(scene.place(4, 4, 0, [1, 1]) && scene.tiles[28]);
        assert_eq!(scene.water[28], 0);
        let surface = vec![-8; 49];
        assert_eq!(scene.scenery_material(2, 3, &surface), (9, 0, 8));
    }

    #[test]
    fn a_region_without_terrain_is_shaded_and_joins_its_neighbours() {
        let mut scene = Scene::new(70, 70, vec![0; 4900]);
        scene.heights.fill(40);
        scene.clear([4, 6]);
        let at = |scene: &Scene, x: usize, y: usize| scene.heights[x * 71 + y];
        assert_eq!(at(&scene, 5, 7), 0);
        // West column and south row copy the tiles beside them.
        assert_eq!((at(&scene, 4, 7), at(&scene, 5, 6)), (40, 40));
        assert_eq!(at(&scene, 4, 6), 40);
        assert_eq!(scene.shade.levels[0][4 * 71 + 6], 127);
        assert_eq!(scene.shade.levels[0][68 * 71 + 69], 127);
        assert_eq!(scene.shade.levels[0][3 * 71 + 6], 0);
    }

    #[test]
    fn each_pass_sets_its_own_light_fog_material_and_depth_layer() {
        let atmosphere = crate::atmosphere::Parameters {
            color: 0xffffff,
            ambient: 1.1523438,
            diffuse: [0.69921875, 1.2],
            fog_color: 0xc8c0a8,
            fog_depth: 0,
        };
        // applyFogFade: unlit, the frame's fog colour from 6144 to 6656.
        let backdrop = uniforms(Pass::Backdrop, &atmosphere);
        // The water colour times the ambient factor is getRgbFloat's colour.
        let lit = backdrop_color(0x122b3d, 0xffffff, 1.1523438, 0.69921875);
        for (channel, value) in [18.0f32, 43.0, 61.0].into_iter().enumerate() {
            assert!((value / 255.0 * backdrop.lights[0][channel] - lit[channel]).abs() < 1e-6);
        }
        assert_eq!(backdrop.lights[1..], [[0.0; 3], [0.0; 3]]);
        assert_eq!(backdrop.fog_range, [48.0, 52.0]);
        assert_eq!(
            backdrop.fog_color,
            [200.0 / 255.0, 192.0 / 255.0, 168.0 / 255.0]
        );
        assert_eq!(
            (backdrop.material_view[3], backdrop.depth_write),
            (0.0, false)
        );
        assert_eq!(backdrop.depth_bias, [0.0, 1.0]);
        // The floor is fifty and a half layers behind the water surface, the
        // scenery one layer behind it; both write depth under material 3.
        let floor = uniforms(Pass::Floor { water: 0x122b3d }, &atmosphere);
        let scenery = uniforms(
            Pass::Scenery {
                water: 0x122b3d,
                range: 128,
                height: 200,
            },
            &atmosphere,
        );
        assert_eq!(floor.material_view, [0.0, 0.0, 1.0, 3.0]);
        assert_eq!(scenery.material_view, [128.0, 200.0, 1.0, 3.0]);
        assert_eq!(
            floor.fog_color,
            water_fog(0x122b3d, 0xffffff, 1.1523438, 0.69921875)
        );
        assert_eq!(floor.fog_range, [48.0, 52.0]);
        assert_eq!(floor.lights, atmosphere.light_colors());
        assert!(floor.depth_write && scenery.depth_write);
        // Both are absolute layers. At any eye depth the floor ends up behind
        // the scenery, and both behind the surface's ground-level floors.
        assert_eq!(floor.depth_bias, [FLOOR_LAYER, 1.0]);
        assert_eq!(scenery.depth_bias, [SCENERY_LAYER, 1.0]);
        assert_eq!(
            (FLOOR_LAYER, SCENERY_LAYER),
            (
                crate::depth_layers::underwater_floor(false),
                crate::depth_layers::UNDERWATER_ENTITIES
            )
        );
        let shift = crate::depth_layers::shift;
        let surface = crate::depth_layers::floor(0, false);
        for depth in [1.0f32, 10.0, 40.0] {
            assert!(shift(FLOOR_LAYER, depth) > shift(SCENERY_LAYER, depth));
            assert!(shift(SCENERY_LAYER, depth) > shift(surface, depth));
        }
    }

    #[test]
    fn the_gradient_is_linear_between_its_end_texels() {
        assert_eq!(ramp_texels(), [115, 135, 155, 175, 195, 215, 235, 255]);
        assert_eq!(ramp(-3.0), 115.0 / 255.0);
        assert_eq!(ramp(0.0625), 115.0 / 255.0);
        assert_eq!(ramp(0.1875), 135.0 / 255.0);
        assert_eq!(ramp(0.25), 145.0 / 255.0);
        assert_eq!(ramp(0.9375), 1.0);
        assert_eq!(ramp(f32::INFINITY), 1.0);
        // The shader's closed form of the same lookup.
        for step in -8..80 {
            let s = step as f32 / 64.0;
            let closed = (115.0 + 20.0 * (s * 8.0 - 0.5).clamp(0.0, 7.0)) / 255.0;
            assert!((ramp(s) - closed).abs() < 1e-6, "{s}");
        }
    }

    #[test]
    fn material_three_adds_noise_then_fades_to_the_lit_water_colour() {
        // Default atmosphere at brightness 3: white light, ambient
        // 1.1523438 * 1.0, diffuse 0.69921875.
        let fog = water_fog(0x122b3d, 0xffffff, 1.1523438, 0.69921875);
        let sum = 1.1523438f32 + 0.69921875;
        assert_eq!(fog[0], 18.0 / 255.0 * 1.0 * 0.58823526 * sum);
        assert_eq!(fog[1], sum * 0.58823526 * 1.0 * (43.0 / 255.0));
        assert_eq!(fog[2], 61.0 / 255.0 * 1.0 * 0.58823526 * sum);
        // At the surface 115 / 255 of the way; from 15 / 16 of the range all.
        let out = shade([0.5, 0.25, 0.9], 0.2, 0.0, fog);
        let t = 115.0 / 255.0;
        assert!((out[0] - (fog[0] * t + 0.7 * (1.0 - t))).abs() < 1e-6);
        assert!((out[2] - (fog[2] * t + 1.0 * (1.0 - t))).abs() < 1e-6);
        assert_eq!(shade([0.5, 0.25, 0.9], 0.2, 1.0, fog), fog);
        assert_eq!(scenery_depth(-100.0, 200, 128), 100.0 / 128.0);
        assert_eq!(scenery_depth(8.0, 16, 0), f32::INFINITY);
        assert!(scenery_depth(8.0, 0, 0).is_nan());
        assert_eq!(caustics([1000.0, 0.0, 2000.0], 100), [1.0, 2.0, 0.5]);
        assert_eq!(
            (rebuild_keep_alives(false), rebuild_keep_alives(true)),
            (4, 7)
        );
        assert_eq!(
            (FLOOR_LAYER, SCENERY_LAYER, SURFACE_WATER_LAYER),
            (251.0, 201.5, 200.5)
        );
    }
}
