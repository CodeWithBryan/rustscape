//! Where `SceneGraph.addLoc` puts a location, and what `LocType.getGlModel`,
//! `getStaticEntity`/`getAnimatedEntity` and `GlModel.alignToTerrain` do to its
//! model. Only the GL ("HD") branches of the original are reproduced, and
//! nothing here touches the GPU.
//!
//! Units are the original's: 128 per tile, heights negative upwards.
use crate::{
    animation::sin_cos,
    interaction::LocationType,
    location::{self, Definition},
    model::Model,
    surface,
    texture::Material,
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

/// `SceneGraph.WALL_DECO_ROT_SIZE_X_DIR` / `_Y_DIR`.
pub const DECOR_X: [i32; 4] = [1, 0, -1, 0];
pub const DECOR_Y: [i32; 4] = [0, -1, 0, 1];
/// `SceneGraph.WALL_OFFSET_X` / `WALL_OFFSET_Y`.
pub const DIAGONAL_X: [i32; 4] = [1, -1, -1, 1];
pub const DIAGONAL_Y: [i32; 4] = [-1, -1, 1, 1];

/// `Preferences.showGroundDecorations`: a static the scene build reads, as in
/// the original. The client mirrors the script-visible preference into it.
static GROUND_DECORATIONS: AtomicBool = AtomicBool::new(true);
pub fn set_ground_decorations(shown: bool) {
    GROUND_DECORATIONS.store(shown, Ordering::Relaxed);
}
pub fn ground_decorations() -> bool {
    GROUND_DECORATIONS.load(Ordering::Relaxed)
}

/// The `LocType` fields only placement reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Rules {
    /// Opcode 28, `walloff`.
    pub wall_offset: i32,
    /// `hillskewType`: opcode 21 sets 1, 81 sets 2, 93 sets 3, 94 sets 4, 95 sets 5.
    pub hill_type: u8,
    /// `hillskewAmount`, a short: `g1 * 256` from opcode 81, `g2` from opcode 93.
    pub hill_amount: i16,
    /// Opcode 82 (`render`): `addLoc` returns at once while the GL renderer is on.
    pub hidden_in_hd: bool,
    /// Opcode 90 (`clipped`): ground decoration drawn on the floor's depth layer.
    pub flat: bool,
    /// Opcode 73, `forcedecor`.
    pub force_decor: bool,
    /// Opcode 98, `dynamic`: the scene keeps a `Loc` rather than a baked model.
    pub dynamic: bool,
    /// `blockwalk` after `LocTypeList.get`'s break-route override.
    pub block_walk: i32,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            wall_offset: 16,
            hill_type: 0,
            hill_amount: -1,
            hidden_in_hd: false,
            flat: false,
            force_decor: false,
            dynamic: false,
            block_walk: 2,
        }
    }
}
impl Rules {
    /// One definition opcode; `operand` is the raw `g1`/`g2` value where the
    /// opcode has one. Returns false for opcodes this struct does not own.
    pub fn opcode(&mut self, opcode: i32, operand: i32) -> bool {
        match opcode {
            17 => self.block_walk = 0,
            21 => self.hill_type = 1,
            27 => self.block_walk = 1,
            28 => self.wall_offset = operand,
            73 => self.force_decor = true,
            81 => {
                self.hill_type = 2;
                self.hill_amount = operand.wrapping_mul(256) as i16;
            }
            82 => self.hidden_in_hd = true,
            90 => self.flat = true,
            93 => {
                self.hill_type = 3;
                self.hill_amount = operand as i16;
            }
            94 => self.hill_type = 4,
            95 => self.hill_type = 5,
            98 => self.dynamic = true,
            _ => return false,
        }
        true
    }
}

/// One entity `addLoc` hands to the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    /// The shape `getStaticEntity`/`Loc` asks the type for.
    pub shape: i32,
    /// The rotation it asks for, 0..=7.
    pub rotation: i32,
    /// `WallDecor.xOffset`/`yOffset`, added to the draw position.
    pub offset: [i32; 2],
    /// What `addLoc` subtracts from the centre when stamping the shadow.
    pub shadow: [i32; 2],
}

/// Everything `addLoc` decides for one placement before any model exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// Width and length after the quarter turn.
    pub size: [i32; 2],
    /// `xFine`/`yFine` relative to the tile's south-west corner: the centre
    /// `getStaticEntity` aligns to the terrain around and the shadow's centre.
    pub center: [i32; 2],
    /// Where the scene draws the entity, relative to the same corner. Walls,
    /// decorations and one-tile scenery sit on the tile centre whatever the
    /// definition's size.
    pub origin: [i32; 2],
    pub pieces: [Option<Piece>; 2],
}

/// `addLoc`'s choice of entities for a shape (0..=22) and rotation (0..=3).
/// `wall_offset` is the `walloff` of the wall on the same tile, 16 without one.
pub fn placement(shape: i32, rotation: i32, size: [i32; 2], wall_offset: i32) -> Option<Placement> {
    let turn = (rotation & 3) as usize;
    let rotation = rotation & 3;
    let size = if rotation & 1 != 0 {
        [size[1], size[0]]
    } else {
        size
    };
    let piece = |shape, rotation| Piece {
        shape,
        rotation,
        offset: [0; 2],
        shadow: [0; 2],
    };
    let opposite = ((rotation + 2) & 3) + 4;
    let half = wall_offset / 2;
    let pieces = match shape {
        0 | 1 | 3 | 9 | 12..=22 => [Some(piece(shape, rotation)), None],
        2 => [
            Some(piece(2, rotation + 4)),
            Some(piece(2, (rotation + 1) & 3)),
        ],
        4 => [Some(piece(4, rotation)), None],
        5 => [
            Some(Piece {
                offset: [
                    wall_offset.wrapping_mul(DECOR_X[turn]),
                    DECOR_Y[turn].wrapping_mul(wall_offset),
                ],
                shadow: [DECOR_X[turn] * 8, DECOR_Y[turn] * 8],
                ..piece(4, rotation)
            }),
            None,
        ],
        6 => [
            Some(Piece {
                offset: [
                    half.wrapping_mul(DIAGONAL_X[turn]),
                    half.wrapping_mul(DIAGONAL_Y[turn]),
                ],
                shadow: [DIAGONAL_X[turn] * 8, DIAGONAL_Y[turn] * 8],
                ..piece(4, rotation + 4)
            }),
            None,
        ],
        7 => [Some(piece(4, opposite)), None],
        8 => [
            Some(Piece {
                offset: [
                    half.wrapping_mul(DIAGONAL_X[turn]),
                    DIAGONAL_Y[turn].wrapping_mul(half),
                ],
                shadow: [DIAGONAL_X[turn] * 8, DIAGONAL_Y[turn] * 8],
                ..piece(4, rotation + 4)
            }),
            // The second model has no offset, but its shadow is stamped at
            // the first one's shifted position.
            Some(Piece {
                shadow: [DIAGONAL_X[turn] * 8, DIAGONAL_Y[turn] * 8],
                ..piece(4, opposite)
            }),
        ],
        10 => [Some(piece(10, rotation)), None],
        11 => [Some(piece(10, rotation + 4)), None],
        _ => return None,
    };
    let center = [size[0] << 6, size[1] << 6];
    Some(Placement {
        size,
        center,
        origin: if matches!(shape, 10 | 11) {
            center
        } else {
            [64; 2]
        },
        pieces,
    })
}

/// `SceneGraph.setWallDecoration`: a wall with a non-default `walloff` placed
/// after a decoration rescales the decoration's offset.
pub fn rescale(offset: [i32; 2], wall_offset: i32) -> [i32; 2] {
    offset.map(|n| n.wrapping_mul(wall_offset) / 16)
}

/// The rule `addLoc` (and `LoginManager`'s location reader) applies to shape 22.
pub fn ground_decoration_shown(preference: bool, kind: &LocationType) -> bool {
    preference
        || kind.interactable != 0
        || kind.placement.block_walk == 1
        || kind.placement.force_decor
}

/// `averageY`: the mean of the four height corners around the centre.
/// `height` reads `tileHeights[plane]` at a tile corner. The original's
/// special case for a location overhanging the 104-tile scene edge has no
/// counterpart here, because this scene has no such edge.
pub fn base_height(tile: [i32; 2], size: [i32; 2], height: impl Fn(i32, i32) -> i32) -> i32 {
    let west = tile[0] + (size[0] >> 1);
    let east = tile[0] + ((size[0] + 1) >> 1);
    let south = tile[1] + (size[1] >> 1);
    let north = tile[1] + ((size[1] + 1) >> 1);
    height(west, north)
        .wrapping_add(height(east, south))
        .wrapping_add(height(west, south))
        .wrapping_add(height(east, north))
        >> 2
}

/// The model ids `getGlModel` builds a shape from; None where it returns null.
pub fn models(d: &Definition, shape: i32) -> Option<&[u32]> {
    match &d.shapes {
        None => (shape == 10 && !d.models.is_empty()).then_some(d.models.as_slice()),
        Some(shapes) => {
            let index = shapes.iter().position(|&s| s == shape)?;
            d.models.get(index..=index)
        }
    }
}

/// Whether `addLoc` builds a `Loc` (resolved every frame) instead of a model.
pub fn is_entity(d: &Definition, kind: &LocationType) -> bool {
    ![-1, 65535].contains(&d.animation) || kind.children.is_some() || kind.placement.dynamic
}

/// `LocType.getGlModel(rotation, animated, shape)`: Ok(None) where it returns
/// null, Err with the id of a model `fetch` does not have.
pub fn gl_model<'a>(
    d: &Definition,
    shape: i32,
    rotation: i32,
    animated: bool,
    materials: &[Option<Material>],
    fetch: impl Fn(u32) -> Option<&'a Model>,
) -> Result<Option<Model>, u32> {
    let Some(ids) = models(d, shape) else {
        return Ok(None);
    };
    let mut model = if let [id] = ids {
        fetch(*id).ok_or(*id)?.clone()
    } else {
        let parts = ids
            .iter()
            .map(|&id| fetch(id).cloned().ok_or(id))
            .collect::<Result<Vec<_>, _>>()?;
        Model::combine(&parts)
    };
    model.sort_by_priority = animated;
    model.draw_order = surface::face_order(&model, materials);
    model.surfaces = surface::prepare(&model, materials);
    // Opcode 39 stores `g1b * 5` and the model is lit with `contrast * 5 + 768`.
    model.lighting = [
        64i32.wrapping_add(d.ambient),
        d.contrast.wrapping_mul(25).wrapping_add(768),
    ];
    transform(&mut model, d, shape, rotation);
    Ok(Some(model))
}

/// The part of `getGlModel` after the model is built, in its order: mirror,
/// the diagonal decoration turn, the quarter turn, recolour, retexture,
/// resize, translate.
pub fn transform(model: &mut Model, d: &Definition, shape: i32, rotation: i32) {
    if d.mirror != (shape == 2 && rotation > 3) {
        mirror_z(model);
    }
    if shape == 4 && rotation > 3 {
        rotate_45(model);
        for v in &mut model.vertices {
            v[0] = v[0].wrapping_add(45);
            v[2] = v[2].wrapping_sub(45);
        }
    }
    location::rotate(model, rotation & 3, true);
    for pair in &d.recolor {
        model.recolor(&[*pair]);
    }
    for &[from, to] in &d.retexture {
        for texture in &mut model.textures {
            if *texture == i32::from(from) {
                *texture = i32::from(to);
            }
        }
    }
    if d.resize != [128; 3] {
        for v in &mut model.vertices {
            for (value, scale) in v.iter_mut().zip(d.resize) {
                *value = value.wrapping_mul(scale) >> 7;
            }
        }
    }
    if d.offset != [0; 3] {
        for v in &mut model.vertices {
            for (value, offset) in v.iter_mut().zip(d.offset) {
                *value = value.wrapping_add(offset);
            }
        }
    }
}

/// `GlModel.mirrorZ`.
pub fn mirror_z(model: &mut Model) {
    for v in &mut model.vertices {
        v[2] = v[2].wrapping_neg();
    }
    for face in &mut model.faces {
        face.swap(0, 2);
    }
    for surface in &mut model.surfaces {
        surface.uv.swap(0, 2);
        surface.normals.swap(0, 2);
        for n in &mut surface.normals {
            n[2] = i32::from(n[2].wrapping_neg() as i16);
        }
    }
}

/// `GlModel.rotate45Degrees`: vertices and normals by 256 of 2048 units.
pub fn rotate_45(model: &mut Model) {
    rotate_y(model, 256);
    let (sin, cos) = sin_cos(256);
    for surface in &mut model.surfaces {
        for n in &mut surface.normals {
            let x = n[2].wrapping_mul(sin).wrapping_add(n[0].wrapping_mul(cos)) >> 16;
            n[2] = i32::from(
                (n[2].wrapping_mul(cos).wrapping_sub(n[0].wrapping_mul(sin)) >> 16) as i16,
            );
            n[0] = i32::from(x as i16);
        }
    }
}

/// `GlModel.rotateY`: vertices only, normals stay as they were.
pub fn rotate_y(model: &mut Model, angle: i32) {
    let (sin, cos) = sin_cos(angle);
    for v in &mut model.vertices {
        let x = v[2].wrapping_mul(sin).wrapping_add(v[0].wrapping_mul(cos)) >> 16;
        v[2] = v[2].wrapping_mul(cos).wrapping_sub(v[0].wrapping_mul(sin)) >> 16;
        v[0] = x;
    }
}

/// `GlModel.calculateBounds`, with its short casts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    pub min: [i16; 3],
    pub max: [i16; 3],
}
impl Bounds {
    pub fn of(model: &Model) -> Self {
        let (mut min, mut max) = ([32767i32; 3], [-32768i32; 3]);
        for v in &model.vertices {
            for (axis, &n) in v.iter().enumerate() {
                min[axis] = min[axis].min(n);
                max[axis] = max[axis].max(n);
            }
        }
        Self {
            min: min.map(|n| n as i16),
            max: max.map(|n| n as i16),
        }
    }
}

/// The height arrays `alignToTerrain` reads, in scene-local tile corners.
/// `size` is the arrays' extent (the original's are 105 by 105); a read
/// outside it never happens for input the original accepts.
pub struct Heights<'a> {
    pub size: [i32; 2],
    /// `tileHeights[level]`.
    pub ground: &'a dyn Fn(i32, i32) -> i32,
    /// `tileHeights[level + 1]`, absent on the top level.
    pub above: Option<&'a dyn Fn(i32, i32) -> i32>,
}
impl Heights<'_> {
    fn outside(&self, min: [i32; 2], max: [i32; 2]) -> bool {
        min[0] < 0
            || (max[0].wrapping_add(128) >> 7) >= self.size[0]
            || min[1] < 0
            || (max[1].wrapping_add(128) >> 7) >= self.size[1]
    }
    fn inside(&self, x: i32, z: i32) -> bool {
        x >= 0 && z >= 0 && x + 1 < self.size[0] && z + 1 < self.size[1]
    }
}

fn bilinear(h: &dyn Fn(i32, i32) -> i32, x: i32, z: i32) -> i32 {
    let (fx, fz, tx, tz) = (x & 127, z & 127, x >> 7, z >> 7);
    let near = (h(tx, tz)
        .wrapping_mul(128 - fx)
        .wrapping_add(h(tx + 1, tz).wrapping_mul(fx)))
        >> 7;
    let far = (h(tx, tz + 1)
        .wrapping_mul(128 - fx)
        .wrapping_add(h(tx + 1, tz + 1).wrapping_mul(fx)))
        >> 7;
    (near
        .wrapping_mul(128 - fz)
        .wrapping_add(far.wrapping_mul(fz)))
        >> 7
}

/// `GlModel.alignToTerrain(type, amount, model, tileHeights, renderHeights,
/// sceneX, baseHeight, sceneZ)` on `vertices`. `bounds` are the bounds of the
/// model the original passes as its third argument.
///
/// Three inputs make the original throw: a type 2 reference model whose
/// lowest point is 0, a type 2 amount of 0, and a type 5 model without
/// height. They leave the affected vertices unchanged here. A vertex that
/// falls outside the height arrays (possible only for a pose that leaves the
/// reference model's bounds) is left unchanged too.
pub fn align_to_terrain(
    vertices: &mut [[i32; 3]],
    kind: u8,
    amount: i16,
    bounds: Bounds,
    heights: &Heights<'_>,
    center: [i32; 2],
    base: i32,
) {
    let amount = i32::from(amount);
    let min = [
        center[0].wrapping_add(i32::from(bounds.min[0])),
        center[1].wrapping_add(i32::from(bounds.min[2])),
    ];
    let max = [
        center[0].wrapping_add(i32::from(bounds.max[0])),
        center[1].wrapping_add(i32::from(bounds.max[2])),
    ];
    if matches!(kind, 1 | 2 | 3 | 5) && heights.outside(min, max) {
        return;
    }
    if matches!(kind, 4 | 5) {
        if heights.above.is_none() || heights.outside(min, max) {
            return;
        }
    } else {
        let (x0, x1) = (min[0] >> 7, max[0].wrapping_add(127) >> 7);
        let (z0, z1) = (min[1] >> 7, max[1].wrapping_add(127) >> 7);
        // Bounds that wrapped in their short casts can pass the check above
        // and still name a corner outside the arrays; the original throws.
        if x0.max(x1) >= heights.size[0] || z0.max(z1) >= heights.size[1] || x1 < 0 || z1 < 0 {
            return;
        }
        let h = heights.ground;
        if h(x0, z0) == base && h(x1, z0) == base && h(x0, z1) == base && h(x1, z1) == base {
            return;
        }
    }
    let sample = |h: &dyn Fn(i32, i32) -> i32, v: &[i32; 3]| {
        let x = v[0].wrapping_add(center[0]);
        let z = v[2].wrapping_add(center[1]);
        heights.inside(x >> 7, z >> 7).then(|| bilinear(h, x, z))
    };
    let height = i32::from(bounds.max[1]) - i32::from(bounds.min[1]);
    match kind {
        1 => {
            for v in vertices {
                if let Some(ground) = sample(heights.ground, v) {
                    v[1] = v[1].wrapping_add(ground).wrapping_sub(base);
                }
            }
        }
        2 => {
            let lowest = i32::from(bounds.min[1]);
            if lowest == 0 {
                return;
            }
            for v in vertices {
                let share = v[1].wrapping_shl(16).wrapping_div(lowest);
                if share < amount {
                    if let Some(ground) = sample(heights.ground, v).filter(|_| amount != 0) {
                        v[1] = v[1].wrapping_add(
                            ground
                                .wrapping_sub(base)
                                .wrapping_mul(amount.wrapping_sub(share))
                                .wrapping_div(amount),
                        );
                    }
                }
            }
        }
        3 => tilt(
            vertices,
            heights,
            center,
            base,
            [(amount & 0xff) * 4, ((amount >> 8) & 0xff) * 4],
        ),
        4 => {
            let Some(above) = heights.above else {
                return;
            };
            for v in vertices {
                if let Some(over) = sample(above, v) {
                    v[1] = v[1]
                        .wrapping_add(over)
                        .wrapping_add(height)
                        .wrapping_sub(base);
                }
            }
        }
        5 => {
            let Some(above) = heights.above else {
                return;
            };
            if height == 0 {
                return;
            }
            for v in vertices {
                let (Some(ground), Some(over)) = (sample(heights.ground, v), sample(above, v))
                else {
                    continue;
                };
                v[1] = (v[1]
                    .wrapping_shl(8)
                    .wrapping_div(height)
                    .wrapping_mul(ground.wrapping_sub(over))
                    >> 8)
                    .wrapping_sub(base.wrapping_sub(ground));
            }
        }
        _ => {}
    }
}

/// `Model.alignToTerrain(heightMap, xFine, height, yFine, sizeX, sizeY)`: the
/// type 3 skew tilts the whole model over a footprint of `size` and moves it
/// to the mean height. `Model.interpolateHeight` reads 0 outside the map.
fn tilt(
    vertices: &mut [[i32; 3]],
    heights: &Heights<'_>,
    center: [i32; 2],
    base: i32,
    size: [i32; 2],
) {
    let read = |dx: i32, dz: i32| {
        let (x, z) = (center[0].wrapping_add(dx), center[1].wrapping_add(dz));
        if heights.inside(x >> 7, z >> 7) {
            bilinear(heights.ground, x, z)
        } else {
            0
        }
    };
    let [w, l] = size;
    let south_west = read(-w / 2, -l / 2);
    let south_east = read(w / 2, -l / 2);
    let north_west = read(-w / 2, l / 2);
    let north_east = read(w / 2, l / 2);
    if l != 0 {
        let south = south_west.min(south_east);
        let north = north_west.min(north_east);
        let pitch =
            ((f64::from(south.wrapping_sub(north)).atan2(f64::from(l)) * 325.95) as i32) & 0x7ff;
        if pitch != 0 {
            let (sin, cos) = sin_cos(pitch);
            for v in vertices.iter_mut() {
                let y = v[1].wrapping_mul(cos).wrapping_sub(v[2].wrapping_mul(sin)) >> 16;
                v[2] = v[1].wrapping_mul(sin).wrapping_add(v[2].wrapping_mul(cos)) >> 16;
                v[1] = y;
            }
        }
    }
    if w != 0 {
        let west = south_west.min(north_west);
        let east = south_east.min(north_east);
        let roll =
            ((f64::from(west.wrapping_sub(east)).atan2(f64::from(w)) * 325.95) as i32) & 0x7ff;
        if roll != 0 {
            let (sin, cos) = sin_cos(roll);
            for v in vertices.iter_mut() {
                let x = v[1].wrapping_mul(sin).wrapping_add(v[0].wrapping_mul(cos)) >> 16;
                v[1] = v[1].wrapping_mul(cos).wrapping_sub(v[0].wrapping_mul(sin)) >> 16;
                v[0] = x;
            }
        }
    }
    let mean = (south_west
        .wrapping_add(north_east)
        .min(south_east.wrapping_add(north_west))
        >> 1)
        .wrapping_sub(base);
    if mean != 0 {
        for v in vertices.iter_mut() {
            v[1] = v[1].wrapping_add(mean);
        }
    }
}

/// What `getStaticEntity` and `getAnimatedEntity` do to the model after
/// `getGlModel` (and after the animation, for an animated one).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Finish {
    /// Shape 10 asked for with a rotation above 3: `rotateY(256)`.
    pub diagonal: bool,
    /// `hillskewType` and `hillskewAmount`, when the type is not 0.
    pub skew: Option<(u8, i16)>,
}
impl Finish {
    pub fn new(kind: &LocationType, d: &Definition, piece: Piece) -> Self {
        let rules = kind.placement;
        // A definition read from a prepared scene carries opcode 21 only.
        let hill = if rules.hill_type == 0 && d.hill {
            1
        } else {
            rules.hill_type
        };
        Self {
            diagonal: piece.shape == 10 && piece.rotation > 3,
            skew: (hill != 0).then_some((hill, rules.hill_amount)),
        }
    }
    /// The turn both entity kinds apply before the shadow is projected.
    pub fn turn(&self, model: &mut Model) {
        if self.diagonal {
            rotate_y(model, 256);
        }
    }
    /// The terrain alignment, applied after the shadow is projected.
    /// `bounds` are those of the cached model: for a static entity the model
    /// after `turn`, for an animated one the model before animation and turn.
    pub fn align(
        &self,
        model: &mut Model,
        bounds: Bounds,
        heights: &Heights<'_>,
        center: [i32; 2],
        base: i32,
    ) {
        if let Some((kind, amount)) = self.skew {
            align_to_terrain(
                &mut model.vertices,
                kind,
                amount,
                bounds,
                heights,
                center,
                base,
            );
        }
    }
}

/// What the scene holds for one placement once every location of a map has
/// been added in file order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Planned {
    pub placement: Placement,
    /// `loc.clipped` for ground decoration.
    pub flat: bool,
}

/// `addLoc` over a whole map: which placements end up in the scene, and with
/// which decoration offsets. A tile has one wall, one wall decoration and one
/// ground decoration, so a later one replaces an earlier one; a decoration
/// takes its offset from the wall already on its tile, and a wall with a
/// non-default `walloff` rescales the decoration already there.
pub struct Plan {
    pub entries: Vec<Option<Planned>>,
}
#[derive(Default)]
struct Tile {
    wall: Option<(usize, i32)>,
    decoration: Option<usize>,
    ground: Option<usize>,
}
impl Plan {
    /// `locations` are `[id, x, z, plane, shape, rotation]` rows in the order
    /// the map files list them.
    pub fn new<'a>(
        locations: &[[i32; 6]],
        lookup: impl Fn(i32) -> Option<(&'a Definition, &'a LocationType)>,
        ground_decorations: bool,
    ) -> Self {
        let mut entries: Vec<Option<Planned>> = vec![None; locations.len()];
        let mut tiles = BTreeMap::<[i32; 3], Tile>::new();
        for (index, &[id, x, z, plane, shape, rotation]) in locations.iter().enumerate() {
            let Some((d, kind)) = lookup(id) else {
                continue;
            };
            let rules = kind.placement;
            if rules.hidden_in_hd {
                continue;
            }
            if shape == 22 && !ground_decoration_shown(ground_decorations, kind) {
                continue;
            }
            let tile = tiles.entry([plane, x, z]).or_default();
            let wall_offset = tile.wall.map_or(16, |(_, offset)| offset);
            let Some(placement) = placement(shape, rotation, [d.width, d.length], wall_offset)
            else {
                continue;
            };
            // A Loc is never null; a baked model is null when the type has
            // no model for the shape, and then nothing enters the scene.
            let present = is_entity(d, kind)
                || placement
                    .pieces
                    .iter()
                    .flatten()
                    .any(|piece| models(d, piece.shape).is_some());
            let planned = Planned {
                placement,
                flat: rules.flat,
            };
            match shape {
                0..=3 => {
                    if present {
                        if let Some((old, _)) = tile.wall.replace((index, rules.wall_offset)) {
                            entries[old] = None;
                        }
                        entries[index] = Some(planned);
                    }
                }
                4..=8 => {
                    // setWallDecor ignores a null first entity.
                    let first = is_entity(d, kind)
                        || placement.pieces[0].is_some_and(|p| models(d, p.shape).is_some());
                    if first {
                        if let Some(old) = tile.decoration.replace(index) {
                            entries[old] = None;
                        }
                        entries[index] = Some(planned);
                    }
                }
                22 => {
                    if present {
                        if let Some(old) = tile.ground.replace(index) {
                            entries[old] = None;
                        }
                        entries[index] = Some(planned);
                    }
                }
                _ => {
                    if present {
                        entries[index] = Some(planned);
                    }
                }
            }
            if matches!(shape, 0 | 2 | 9) && rules.wall_offset != 16 {
                if let Some(planned) = tile.decoration.and_then(|i| entries[i].as_mut()) {
                    for piece in planned.placement.pieces.iter_mut().flatten() {
                        piece.offset = rescale(piece.offset, rules.wall_offset);
                    }
                }
            }
        }
        Self { entries }
    }
}

/// `LocType.getMultiLoc` for a placement: the type drawn in the parent's
/// place, None when the parent draws nothing. A type without children is its
/// own choice.
pub fn resolve(id: u32, kind: &LocationType, read: impl FnMut(bool, i32) -> i32) -> Option<u32> {
    if kind.children.is_some() {
        kind.select_morph(read)
    } else {
        Some(id)
    }
}

/// The variable a multiloc reads: `(true, varbit)` or `(false, varp)`.
pub fn variable(kind: &LocationType) -> Option<(bool, i32)> {
    kind.children.as_ref()?;
    if kind.morph_varbit != -1 {
        Some((true, kind.morph_varbit))
    } else if kind.morph_varp != -1 {
        Some((false, kind.morph_varp))
    } else {
        None
    }
}

/// `Loc`'s constructor for LOC_ANIM: the new clock continues the replaced
/// one when both play the same sequence and it has `exactmove` 0.
pub fn retained_clock(
    sequence: u32,
    exact_move: i32,
    previous: Option<(u32, &location::Animation)>,
) -> Option<location::Animation> {
    let (old, clock) = previous?;
    (exact_move == 0 && old == sequence).then(|| clock.clone())
}

/// Transcription tests: the expected values are worked out by hand from
/// `SceneGraph.addLoc`, `setWallDecoration`, `LocType.decode` and `Loc`'s
/// constructor. The models and the terrain alignment are compared with the
/// running original in `tests/loc_placement.rs`.
#[cfg(test)]
mod tests {
    use super::*;

    fn kind(bytes: &[u8]) -> (LocationType, Definition) {
        let mut data = bytes.to_vec();
        data.push(0);
        let (kind, visual, _) = LocationType::decode_with_visual(&data).unwrap();
        (kind, visual)
    }
    fn piece(shape: i32, rotation: i32, offset: [i32; 2], shadow: [i32; 2]) -> Option<Piece> {
        Some(Piece {
            shape,
            rotation,
            offset,
            shadow,
        })
    }

    #[test]
    fn rules_follow_the_definition_opcodes() {
        assert_eq!(kind(&[]).0.placement, Rules::default());
        let rules = |bytes: &[u8]| kind(bytes).0.placement;
        assert_eq!(rules(&[21]).hill_type, 1);
        assert_eq!(
            (rules(&[81, 3]).hill_type, rules(&[81, 3]).hill_amount),
            (2, 768)
        );
        // 128 * 256 does not fit the short it is stored in.
        assert_eq!(rules(&[81, 128]).hill_amount, -32768);
        assert_eq!(
            (
                rules(&[93, 0x12, 0x34]).hill_type,
                rules(&[93, 0x12, 0x34]).hill_amount
            ),
            (3, 0x1234)
        );
        assert_eq!(rules(&[93, 0xff, 0xff]).hill_amount, -1);
        assert_eq!(rules(&[94]).hill_type, 4);
        assert_eq!(rules(&[95]).hill_type, 5);
        // The last opcode sets the type; the amount of an earlier one stays.
        let both = rules(&[81, 2, 21]);
        assert_eq!((both.hill_type, both.hill_amount), (1, 512));
        assert_eq!(rules(&[28, 32]).wall_offset, 32);
        assert!(rules(&[82]).hidden_in_hd && rules(&[90]).flat);
        assert!(rules(&[73]).force_decor && rules(&[98]).dynamic);
        assert_eq!(
            [
                rules(&[17]).block_walk,
                rules(&[27]).block_walk,
                rules(&[27, 74]).block_walk
            ],
            [0, 1, 0]
        );
    }

    #[test]
    fn every_shape_gets_the_entities_add_loc_builds() {
        let at = |shape, rotation| placement(shape, rotation, [1, 1], 16).unwrap().pieces;
        for rotation in 0..4 {
            for shape in [0, 1, 3, 4, 9, 10, 12, 17, 21, 22] {
                assert_eq!(
                    at(shape, rotation),
                    [piece(shape, rotation, [0; 2], [0; 2]), None]
                );
            }
            // The corner wall: the mirrored piece first, then the next side.
            assert_eq!(
                at(2, rotation),
                [
                    piece(2, rotation + 4, [0; 2], [0; 2]),
                    piece(2, (rotation + 1) & 3, [0; 2], [0; 2])
                ]
            );
            assert_eq!(
                at(7, rotation),
                [piece(4, ((rotation + 2) & 3) + 4, [0; 2], [0; 2]), None]
            );
            assert_eq!(
                at(11, rotation),
                [piece(10, rotation + 4, [0; 2], [0; 2]), None]
            );
        }
        let offsets = |shape, wall| {
            [0, 1, 2, 3].map(|r| {
                let p = placement(shape, r, [1, 1], wall).unwrap().pieces[0].unwrap();
                (p.shape, p.rotation, p.offset, p.shadow)
            })
        };
        assert_eq!(
            offsets(5, 32),
            [
                (4, 0, [32, 0], [8, 0]),
                (4, 1, [0, -32], [0, -8]),
                (4, 2, [-32, 0], [-8, 0]),
                (4, 3, [0, 32], [0, 8])
            ]
        );
        // Half the wall's offset, rounded towards zero before the sign.
        assert_eq!(
            offsets(6, 21),
            [
                (4, 4, [10, -10], [8, -8]),
                (4, 5, [-10, -10], [-8, -8]),
                (4, 6, [-10, 10], [-8, 8]),
                (4, 7, [10, 10], [8, 8])
            ]
        );
        assert_eq!(
            at(8, 3),
            [
                piece(4, 7, [8, 8], [8, 8]),
                // No offset, but the shadow of the first.
                piece(4, 5, [0; 2], [8, 8])
            ]
        );
        assert!(placement(23, 0, [1, 1], 16).is_none());

        // The centre follows the turned size; only centrepieces are drawn there.
        let wide = placement(11, 1, [2, 3], 16).unwrap();
        assert_eq!(
            (wide.size, wide.center, wide.origin),
            ([3, 2], [192, 128], [192, 128])
        );
        let wall = placement(0, 2, [2, 3], 16).unwrap();
        assert_eq!(
            (wall.size, wall.center, wall.origin),
            ([2, 3], [128, 192], [64, 64])
        );
        assert_eq!(placement(9, 0, [2, 2], 16).unwrap().origin, [64, 64]);
    }

    #[test]
    fn the_plan_keeps_what_add_loc_leaves_in_the_scene() {
        // 1: wall with models for shapes 0 and 2 and walloff 32.
        // 2: decoration (shape 4 model). 3: hidden in HD. 4: plain ground
        // decoration. 5: the same with an option. 6: diagonal wall (shape 9),
        // walloff 8. 7: wall with a shape 2 model only. 8: multiloc parent.
        // 9: forced decoration. 10: ground decoration that blocks. 11: thin wall.
        let types: BTreeMap<i32, (LocationType, Definition)> = [
            (1, kind(&[1, 2, 0, 1, 0, 0, 2, 2, 28, 32])),
            (2, kind(&[1, 1, 0, 3, 4])),
            (3, kind(&[1, 1, 0, 1, 0, 28, 64, 82])),
            (4, kind(&[1, 1, 0, 4, 22, 17])),
            (5, kind(&[1, 1, 0, 4, 22, 17, 30, b'U', b's', b'e', 0])),
            (6, kind(&[1, 1, 0, 5, 9, 28, 8])),
            (7, kind(&[1, 1, 0, 2, 2, 28, 48])),
            (8, kind(&[77, 0, 5, 255, 255, 1, 0, 2, 255, 255])),
            (9, kind(&[1, 1, 0, 4, 22, 17, 73])),
            (10, kind(&[1, 1, 0, 4, 22, 27])),
            (11, kind(&[1, 1, 0, 1, 0])),
        ]
        .into_iter()
        .collect();
        let plan = |rows: &[[i32; 6]], decorations: bool| {
            Plan::new(
                rows,
                |id| types.get(&id).map(|(kind, d)| (d, kind)),
                decorations,
            )
            .entries
        };
        let offset = |entry: &Option<Planned>| entry.unwrap().placement.pieces[0].unwrap().offset;

        // A decoration placed after its wall reads the wall's offset; placed
        // before, it starts from 16 (or 8) and the wall rescales it.
        let after = plan(&[[1, 5, 5, 0, 0, 0], [2, 5, 5, 0, 5, 0]], true);
        assert_eq!(offset(&after[1]), [32, 0]);
        let before = plan(&[[2, 5, 5, 0, 5, 0], [1, 5, 5, 0, 0, 0]], true);
        assert_eq!(offset(&before[0]), [32, 0]);
        let diagonal = plan(&[[2, 5, 5, 0, 6, 1], [1, 5, 5, 0, 2, 0]], true);
        assert_eq!(offset(&diagonal[0]), [-16, -16]);
        // Another tile or level does not count.
        let apart = plan(&[[1, 5, 5, 1, 0, 0], [2, 5, 5, 0, 5, 0]], true);
        assert_eq!(offset(&apart[1]), [16, 0]);

        // A diagonal wall is scenery: getWallKey does not see it, but it
        // rescales a decoration that is already there.
        let unseen = plan(&[[6, 5, 5, 0, 9, 0], [2, 5, 5, 0, 5, 2]], true);
        assert_eq!(offset(&unseen[1]), [-16, 0]);
        let rescaled = plan(&[[2, 5, 5, 0, 5, 2], [6, 5, 5, 0, 9, 0]], true);
        assert_eq!(offset(&rescaled[0]), [-8, 0]);
        // Both a wall and a diagonal wall after the decoration: 16 * 32 / 16 * 8 / 16.
        let twice = plan(
            &[[2, 5, 5, 0, 5, 0], [1, 5, 5, 0, 0, 0], [6, 5, 5, 0, 9, 0]],
            true,
        );
        assert_eq!(offset(&twice[0]), [16, 0]);

        // One wall and one decoration per tile: the later one stays.
        let walls = plan(&[[1, 5, 5, 0, 0, 0], [11, 5, 5, 0, 0, 1]], true);
        assert!(walls[0].is_none() && walls[1].is_some());
        let decorations = plan(&[[2, 5, 5, 0, 4, 0], [2, 5, 5, 0, 5, 1]], true);
        assert!(decorations[0].is_none() && decorations[1].is_some());

        // A type hidden in HD enters nothing: no wall, no offset for others.
        let hidden = plan(&[[3, 5, 5, 0, 0, 0], [2, 5, 5, 0, 5, 0]], true);
        assert!(hidden[0].is_none());
        assert_eq!(offset(&hidden[1]), [16, 0]);

        // Without a model for the shape there is no wall: the earlier wall
        // stays and keeps giving its offset. The rescale happens regardless.
        let absent = plan(
            &[[1, 5, 5, 0, 0, 0], [7, 5, 5, 0, 0, 0], [2, 5, 5, 0, 5, 0]],
            true,
        );
        assert!(absent[0].is_some() && absent[1].is_none());
        assert_eq!(offset(&absent[2]), [32, 0]);
        let unmodelled = plan(&[[2, 5, 5, 0, 5, 0], [7, 5, 5, 0, 0, 0]], true);
        assert_eq!(offset(&unmodelled[0]), [48, 0]);

        // A multiloc parent is a Loc, never null, whatever it draws.
        let parent = plan(&[[1, 5, 5, 0, 0, 0], [8, 5, 5, 0, 0, 0]], true);
        assert!(parent[0].is_none() && parent[1].is_some());

        // Ground decoration with the preference off: only what can be used,
        // blocks or is forced.
        let rows = [
            [4, 1, 1, 0, 22, 0],
            [5, 2, 1, 0, 22, 0],
            [9, 3, 1, 0, 22, 0],
            [10, 4, 1, 0, 22, 0],
        ];
        assert!(plan(&rows, true).iter().all(Option::is_some));
        assert_eq!(
            plan(&rows, false)
                .iter()
                .map(Option::is_some)
                .collect::<Vec<_>>(),
            [false, true, true, true]
        );
    }

    #[test]
    fn the_wall_rescale_divides_towards_zero() {
        assert_eq!(rescale([16, 0], 32), [32, 0]);
        assert_eq!(rescale([-8, 8], 21), [-10, 10]);
        assert_eq!(rescale([0, -16], 0), [0, 0]);
    }

    #[test]
    fn the_base_height_is_the_floored_mean_of_four_corners() {
        let height = |x: i32, z: i32| -(x * 10 + z * 3) - 1;
        // 1 x 1: the tile's corners.
        assert_eq!(
            base_height([4, 6], [1, 1], height),
            (-59 - 62 - 69 - 72) >> 2
        );
        // 2 x 2: the middle corner four times.
        assert_eq!(base_height([4, 6], [2, 2], height), -72);
        // 3 x 1: west = x + 1, east = x + 2.
        assert_eq!(
            base_height([4, 6], [3, 1], height),
            (-69 - 72 - 79 - 82) >> 2
        );
    }

    #[test]
    fn a_replacing_loc_keeps_the_clock_only_for_the_same_free_sequence() {
        let seq = crate::animation::Sequence {
            frames: vec![1, 2, 3],
            delays: vec![4, 4, 4],
            ..Default::default()
        };
        let mut clock = location::Animation::new(&seq, 100, None).unwrap();
        clock.advance(&seq, 107);
        let kept = retained_clock(7, 0, Some((7, &clock))).unwrap();
        assert_eq!(
            (kept.frame, kept.progress, kept.start_loop, kept.next_frame),
            (
                clock.frame,
                clock.progress,
                clock.start_loop,
                clock.next_frame
            )
        );
        // Started at loop 99 with 8 loops elapsed: one 4-loop frame is done.
        assert!(kept.frame == 1 && kept.progress == 4 && kept.start_loop == 103);
        // Another sequence, a sequence that is not exactmove 0 (2 is the
        // default), or no Loc before: a fresh clock.
        assert!(retained_clock(8, 0, Some((7, &clock))).is_none());
        assert!(retained_clock(7, 2, Some((7, &clock))).is_none());
        assert!(retained_clock(7, 0, None).is_none());
    }
}
