//! RT4's depth layers.
//!
//! The HD client draws everything in one depth buffer but moves each kind of
//! thing a little along the view axis first: `GlRenderer.setDepthLayer(n)`
//! is `setDepthBias(3000, n * 1.5)`, which rewrites the projection so that a
//! point drawn on layer `n` gets the depth a point `n * 1.5` units farther
//! would get at a distance of 3000 (and less nearer the camera). A higher
//! layer is farther back up to a little over twice that distance, where the
//! matrix stops separating layers; in the few tiles between there and the
//! far plane the order is the other way round, in the original as here.
//! The layers, per level `l` (`Tile.level`):
//!
//! | what | layer | depth writes |
//! |---|---|---|
//! | floor group that cannot blend (`GlTile.blend` false) | `201 - 50 l` | on |
//! | floor group that can blend, its opaque faces | `200.5 - 50 l` | on |
//! | the same group's blended faces | `100.5 - 50 l` | off |
//! | shadow map, after the ground level's floors | `101.5` | off |
//! | light meshes, after every level's floors | `200 - 50 l` | off |
//! | walls, scenery, item piles, actors of a tile | `151.5 - 50 l` | on |
//! | ground decoration with opcode 90 | `200 - 50 l` | on |
//! | wall decoration | `151 - 50 l` | on |
//! | a character's shadow and the hint arrow at its feet | `51.5 - 50 l` | off |
//! | underwater scene: floors, then everything on them | `251` / `250.5`, `201.5` | material 3 turns them off |
//!
//! So a floor lies fifty layers behind what stands on it, a rug half a
//! layer to one layer in front of its floor and far behind everything else
//! on the level, a wall decoration half a layer in front of its wall, and
//! the floor of the next level up half a layer to one layer in front of
//! this level's things.
//!
//! The world shader computes the layer of a vertex from what the vertex
//! already carries (whether it is floor, and its level) plus the draw call's
//! `DepthBias` uniform; [`glsl`] is that code and [`shift`] is the same
//! arithmetic for tests and for draws that use another shader.
//!
//! Floors take the level of the floor group they are built into, as in the
//! original: the ground's for a tile the map forces to the ground (flag 8)
//! and for a bridge deck. Locations take their tile's `drawLevel`
//! (`terrain::draw_level`): their own level on a forced-ground tile, as in
//! the original (`renderTile`'s `201.5 - (tile.level + 1) * 50`).
//!
//! One departure: above a bridge the original keeps `tile.level` for the
//! layer and lights with the lowered `drawLevel`; here both are the lowered
//! level. Floor and contents of such a tile move together, so their order is
//! the original's; only their distance in layers from other tiles' things
//! differs.

use crate::projection::{FAR, NEAR};

/// `GlRenderer.setDepthLayer`'s projection distance.
pub const DISTANCE: f32 = 3000.0;
/// Depth-bias units per layer.
pub const UNIT: f32 = 1.5;

/// Walls, scenery, item piles and actors: `renderTile`'s `depthLayer`.
pub fn entities(level: usize) -> f32 {
    201.5 - (level as f32 + 1.0) * 50.0
}
/// A floor group's opaque faces (`setShapedTile`'s `depth`).
pub fn floor(level: usize, blend_group: bool) -> f32 {
    201.5 - level as f32 * 50.0 - if blend_group { 1.0 } else { 0.5 }
}
/// The blended faces of a floor group that can blend (`GlTile.renderTiles`).
pub fn blended_floor(level: usize) -> f32 {
    floor(level, true) - 100.0
}
/// `ShadowManager.renderShadowMapGL`'s layer.
pub const SHADOW_MAP: f32 = 101.5;
/// `LightingManager.renderLightMeshes`.
pub fn light_mesh(level: usize) -> f32 {
    201.5 - level as f32 * 50.0 - 1.5
}
/// Ground decoration of a type with opcode 90.
pub fn flat_decor(level: usize) -> f32 {
    entities(level) + 50.0 - 1.5
}
/// Wall decoration.
pub fn wall_decor(level: usize) -> f32 {
    entities(level) - 0.5
}
/// `Player.render` and `Npc.render` draw the shadow, and
/// `Player.renderHintArrow` the arrow, 150 bias units nearer.
pub fn character_shadow(level: usize) -> f32 {
    entities(level) - 150.0 / UNIT
}
/// The underwater scene's floors and what stands on them.
pub fn underwater_floor(blend_group: bool) -> f32 {
    251.5 - if blend_group { 1.0 } else { 0.5 }
}
pub const UNDERWATER_ENTITIES: f32 = 201.5;

/// What the `DepthBias` uniform adds to a vertex's own layer (`[layers,
/// 0]`), for the kinds a draw call can be.
pub mod offset {
    /// The shader's own layers: a floor that cannot blend, and tile entities.
    pub const NONE: [f32; 2] = [0.0; 2];
    /// Blended floor faces, against the shader's floor layer.
    pub const BLENDED_FLOOR: [f32; 2] = [-100.5, 0.0];
    pub const WALL_DECOR: [f32; 2] = [-0.5, 0.0];
    pub const FLAT_DECOR: [f32; 2] = [48.5, 0.0];
    pub const CHARACTER_SHADOW: [f32; 2] = [-100.0, 0.0];
    /// Vertices marked with a level other than the one they are drawn on.
    pub fn level(marked: usize, drawn: usize) -> f32 {
        (marked as f32 - drawn as f32) * 50.0
    }
    /// A draw whose layer does not come from its vertices.
    pub fn absolute(layer: f32) -> [f32; 2] {
        [layer, 1.0]
    }
}

/// The layer the shader gives a vertex: `floor` and `level` are what the
/// vertex carries, `bias` the draw call's `DepthBias`.
pub fn vertex_layer(floor: bool, level: usize, bias: [f32; 2]) -> f32 {
    if bias[1] > 0.5 {
        bias[0]
    } else {
        (if floor { 201.0 } else { 151.5 }) - 50.0 * level as f32 + bias[0]
    }
}

/// `GlRenderer.setupPerspectiveView`'s z offset (`matrix[14]`), in tiles.
fn z_offset() -> f32 {
    let (near, far) = (NEAR as f32 / 128.0, FAR as f32 / 128.0);
    -(2.0 * near * far) / (far - near)
}

/// What a layer adds to clip-space z at `depth` tiles in front of the
/// camera: the difference `GlRenderer.setDepthBias` makes to `matrix[10]`
/// and `matrix[14]`, applied to the point. Written so that layer 0 is
/// exactly no change.
pub fn shift(layer: f32, depth: f32) -> f32 {
    let bias = layer * UNIT / 128.0;
    let distance = DISTANCE / 128.0;
    let total = bias + distance;
    z_offset() * bias * (depth - bias - 2.0 * distance) / (total * total)
}

/// [`shift`] as the two numbers a shader of the older form
/// `z += c.y - c.x * depth` takes (the changes to `matrix[10]` and
/// `matrix[14]`).
pub fn coefficients(layer: f32) -> [f32; 2] {
    let bias = layer * UNIT / 128.0;
    let distance = DISTANCE / 128.0;
    let scale = z_offset() * bias / ((bias + distance) * (bias + distance));
    [-scale, -scale * (bias + 2.0 * distance)]
}

/// The vertex-stage function `depthLayer(surface)`: the clip-z shift of
/// the vertex being drawn. `surface` is the vertex's packed material word
/// below the light key (material, 1024 for a floor, 2048 per level). It
/// reads the `position` attribute and the `DepthBias`, `CameraPosition` and
/// `CameraForward` uniforms.
pub fn glsl() -> String {
    format!(
        "float depthLayer(float surface){{\n float own=(mod(surface,2048.0)>=1024.0?201.0:151.5)-50.0*floor(surface/2048.0);\n float layer=DepthBias.y>0.5?DepthBias.x:own+DepthBias.x;\n float b=layer*{unit:?};float t=b+{distance:?};\n float d=dot(position-CameraPosition,CameraForward);\n return {offset:?}*b*(d-b-{twice:?})/(t*t);\n}}\n",
        unit = UNIT / 128.0,
        distance = DISTANCE / 128.0,
        twice = 2.0 * DISTANCE / 128.0,
        offset = z_offset(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shadow::depth_bias;

    /// Clip z of a point `depth` tiles ahead under `GlRenderer.setDepthBias`'s
    /// matrix for `layer` (the port of it in `shadow::depth_bias`).
    fn clip(layer: f32, depth: f32) -> f64 {
        let [scale, offset] = depth_bias(
            NEAR as f32 / 128.0,
            FAR as f32 / 128.0,
            DISTANCE / 128.0,
            layer * UNIT / 128.0,
        );
        f64::from(scale) * -f64::from(depth) + f64::from(offset)
    }

    #[test]
    fn the_shift_is_the_change_set_depth_bias_makes_to_the_projection() {
        for layer in [
            -49.5, 0.5, 1.5, 51.5, 100.5, 101.5, 151.0, 151.5, 200.0, 201.0, 251.0,
        ] {
            for depth in [0.5, 2.0, 7.5, 20.0, 40.0, 55.0] {
                let expected = clip(layer, depth) - clip(0.0, depth);
                let actual = f64::from(shift(layer, depth));
                assert!(
                    (actual - expected).abs() < 2e-5 * expected.abs().max(1.0),
                    "layer {layer} depth {depth}: {actual} {expected}"
                );
            }
        }
        assert_eq!(shift(0.0, 10.0), 0.0);
        for layer in [50.0, 150.0, 200.0] {
            let c = coefficients(layer);
            let expected = shift(layer, 12.5);
            assert!((c[1] - c[0] * 12.5 - expected).abs() < 1e-6, "{layer}");
        }
    }

    #[test]
    fn a_higher_layer_is_farther_back_at_every_distance() {
        let layers = [
            character_shadow(0),
            blended_floor(0),
            SHADOW_MAP,
            floor(1, true),
            wall_decor(0),
            entities(0),
            light_mesh(0),
            floor(0, true),
            floor(0, false),
            UNDERWATER_ENTITIES,
            underwater_floor(false),
        ];
        assert!(
            layers.windows(2).all(|pair| pair[0] < pair[1]),
            "{layers:?}"
        );
        assert_eq!(flat_decor(0), light_mesh(0));
        // The next level's plain floor shares a layer with this level's
        // wall decorations.
        assert_eq!(floor(1, false), wall_decor(0));
        for depth in [0.6, 3.0, 12.0, 30.0, 46.0] {
            for pair in layers.windows(2) {
                assert!(shift(pair[0], depth) < shift(pair[1], depth), "{pair:?}");
            }
        }
        // The original's matrix stops separating layers a little beyond
        // twice its projection distance; from there to the far plane a
        // higher layer comes out nearer.
        assert!(shift(floor(0, false), 54.0) < shift(entities(0), 54.0));
        // Things two units apart at a typical distance keep their real
        // order across half a layer, and lose it across fifty.
        let ndc = |layer: f32, depth: f32| {
            (clip(0.0, depth) + f64::from(shift(layer, depth))) / f64::from(depth)
        };
        let depth = 1500.0 / 128.0;
        let apart = 2.0 / 128.0;
        assert!(ndc(entities(0), depth) < ndc(wall_decor(0), depth + apart));
        assert!(ndc(floor(0, false), depth) > ndc(entities(0), depth + apart));
    }

    #[test]
    fn the_shader_offsets_give_the_original_layers() {
        for level in 0..4 {
            assert_eq!(vertex_layer(false, level, offset::NONE), entities(level));
            assert_eq!(vertex_layer(true, level, offset::NONE), floor(level, false));
            assert_eq!(
                vertex_layer(true, level, offset::BLENDED_FLOOR),
                blended_floor(level)
            );
            assert_eq!(
                vertex_layer(false, level, offset::WALL_DECOR),
                wall_decor(level)
            );
            assert_eq!(
                vertex_layer(false, level, offset::FLAT_DECOR),
                flat_decor(level)
            );
            assert_eq!(
                vertex_layer(false, level, offset::CHARACTER_SHADOW),
                character_shadow(level)
            );
            assert_eq!(
                vertex_layer(true, level, offset::absolute(light_mesh(level))),
                light_mesh(level)
            );
        }
        // A floor marked level 1 and drawn with the ground's things.
        assert_eq!(
            vertex_layer(true, 1, [offset::level(1, 0), 0.0]),
            floor(0, false)
        );
    }

    #[test]
    fn the_shader_code_carries_the_same_constants() {
        let code = glsl();
        assert!(code.starts_with("float depthLayer(float surface){"));
        for constant in [
            format!("{:?}", UNIT / 128.0),
            format!("{:?}", DISTANCE / 128.0),
            format!("{:?}", z_offset()),
        ] {
            assert!(code.contains(&constant), "{constant}");
        }
        assert!(!code.contains('@'));
    }
}
