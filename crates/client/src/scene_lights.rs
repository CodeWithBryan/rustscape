//! Where the scene meets the ported lighting and shadow rules: the light key
//! baked into object vertices, the terrain light meshes and their draw pass,
//! floor shadows, and the refresh rule for animated location shadows.
//!
//! The scene here is the loaded regions (up to 192 tiles a side), not the
//! original's 104-tile window, so the original's rules that stop at the
//! window's edge stop at the loaded area's edge instead.
use super::{ShadowCaster, Workbench, render, tile};
use macroquad::prelude::*;
use rs09_client::{
    floor::{Overlay, Underlay},
    light_mesh,
    light_select::{self, Object},
    model::Model,
    shadow_rules::{self, FloorTile, LocShadow},
    terrain::{Region, Tile},
};

/// `Vertex::normal.w` holds the material (0-1023), 1024 for a floor, the
/// level times 2048 and, above that, the light key times this.
pub const KEY_UNIT: f32 = 8192.0;

/// How `renderTile` lights one entity of a location of this shape and
/// rotation (the location's own, 0 to 3). `size` is its footprint as placed;
/// `raised` says its tile is drawn one level below its own (a bridge);
/// `piece` is the entity's place among the placement's (`loc_placement`): the
/// second model of a shape 2 corner is the wall of the next side, as
/// `addLoc` hands both to `setWall`.
pub fn location_object(
    shape: i32,
    rotation: i32,
    size: [i32; 2],
    raised: bool,
    piece: usize,
) -> Object {
    let side = 1 << (rotation & 3);
    match shape {
        2 if piece == 1 => Object::Wall {
            flag: 1 << ((rotation + 1) & 3),
            diagonal: None,
            raised,
        },
        0 | 2 => Object::Wall {
            flag: side,
            diagonal: None,
            raised,
        },
        1 => Object::Wall {
            flag: side << 4,
            diagonal: Some(rotation & 3),
            raised,
        },
        3 => Object::Wall {
            flag: side << 4,
            diagonal: None,
            raised,
        },
        9 => Object::DiagonalScenery { rotation },
        4..=8 | 22 => Object::Area { size: [1, 1] },
        _ => Object::Area { size },
    }
}

/// The tiles `SceneGraph`'s temporary-entity insert files an actor under:
/// south-west tile and extent. The original widens the box by a tile on the
/// side a stretching animation faces; that is not repeated.
pub fn actor_area(fine: [i32; 2], size: i32) -> ([i32; 2], [i32; 2]) {
    let radius = (size.max(1) - 1) * 64 + 60;
    let min = fine.map(|n| (n - radius) >> 7);
    let max = fine.map(|n| (n + radius) >> 7);
    (min, [max[0] - min[0] + 1, max[1] - min[1] + 1])
}

/// Writes the light key and the level the object is drawn on into model
/// vertices. `origin` is the light grid's south-west tile; `tile` the
/// object's south-west tile in world tiles. The vertex's tile is formed with
/// the shader's own arithmetic so both agree on tile edges.
pub fn stamp(
    vertices: &mut [Vertex],
    origin: Vec2,
    object: Object,
    tile: [i32; 2],
    draw_plane: usize,
) {
    let shift = [origin.x - 3200.0, origin.y - 3200.0];
    let local = [tile[0] - origin.x as i32, tile[1] - origin.y as i32];
    for vertex in vertices {
        let vertex_tile = [
            (vertex.position.x - shift[0]).floor() as i32,
            (vertex.position.z - shift[1]).floor() as i32,
        ];
        let key = light_select::key(object, local, vertex_tile);
        vertex.normal.w =
            vertex.normal.w.rem_euclid(2048.0) + draw_plane as f32 * 2048.0 + key as f32 * KEY_UNIT;
    }
}

/// The floor shadow an upper-level tile casts, as `buildScene` asks for it.
pub fn floor_shadow(
    shadows: &render::ShadowMap,
    regions: &[Region],
    (underlays, overlays): (&[Underlay], &[Overlay]),
    textures: &render::Textures,
    plane: usize,
    [x, z]: [i32; 2],
    t: Tile,
) -> Option<ShadowCaster> {
    let sprites = textures.floor_shadows.as_ref()?;
    if plane == 0 {
        return None;
    }
    let underlay_open = t
        .underlay
        .checked_sub(1)
        .and_then(|id| underlays.get(usize::from(id)))
        .is_none_or(|underlay| !underlay.block_shadow);
    let overlay = t
        .overlay
        .checked_sub(1)
        .and_then(|id| overlays.get(usize::from(id)));
    let overlay_open = overlay.is_none_or(|overlay| {
        let textured = usize::try_from(overlay.texture)
            .ok()
            .and_then(|id| textures.materials.get(id))
            .and_then(Option::as_ref)
            .is_some_and(|material| material.repeating);
        !textured && overlay.color == -1 || !overlay.casts_shadow
    });
    let corner = |dx: i32, dz: i32| {
        tile(regions, plane, x + dx, z + dz)
            .height
            .wrapping_sub(tile(regions, 0, x + dx, z + dz).height)
    };
    let (sprite, position) = shadow_rules::floor(
        sprites,
        shadows.light,
        [shadows.map.width, shadows.map.height],
        FloorTile {
            shape: if t.overlay == 0 {
                0
            } else {
                usize::from(t.shape) + 1
            },
            rotation: i32::from(t.rotation),
            overlay_open,
            underlay_open,
            tile: [x - shadows.origin.x as i32, z - shadows.origin.y as i32],
            heights: [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)],
        },
    )?;
    Some(ShadowCaster {
        sprite,
        position,
        plane: 0,
        active: true,
    })
}

/// `Loc.getOrBuildEntity`'s shadow step for one animated location: rebuild
/// when the rule says so, and hold the first shadow back until part of where
/// it falls is still lit.
#[allow(clippy::too_many_arguments)]
pub fn animated_shadow(
    shadows: &mut render::Shadows,
    materials: &[Option<rs09_client::texture::Material>],
    state: &mut LocShadow,
    current: &mut Option<rs09_client::shadow::Sprite>,
    pose: &Model,
    type_id: i32,
    frames: Option<[i32; 2]>,
    continuous: bool,
    position: [i32; 2],
) {
    if !state.needs_update(type_id, frames, continuous, shadows.mode) {
        return;
    }
    if let Some(old) = current.take() {
        shadows.map.stamp(&old, position, true);
    }
    let sprite = rs09_client::shadow::project(pose, materials, shadows.light);
    if state.pending
        && sprite
            .as_ref()
            .is_some_and(|sprite| shadow_rules::open_to_light(&shadows.map, sprite, position))
    {
        state.pending = false;
    }
    if !state.pending {
        if let Some(sprite) = &sprite {
            shadows.map.stamp(sprite, position, false);
        }
        *current = sprite;
    }
    state.last_type = type_id;
    state.last_frame = frames.map_or(0, |[frame, _]| frame);
}

struct Batch {
    mesh: Mesh,
    /// Vertex ranges with the light each belongs to.
    lights: Vec<(std::ops::Range<usize>, usize)>,
    center: Vec2,
    /// The world tiles of the light's span profile that the scene has on the
    /// light's level; their visibility decides whether the mesh is drawn.
    tiles: Vec<[i32; 2]>,
}

/// The light meshes of the placed lights, rebuilt whenever the lights are
/// placed again, and the pass that multiplies them into the terrain.
pub struct LightMeshes {
    material: Material,
    revision: Option<u64>,
    /// Batches per level.
    levels: [Vec<Batch>; 4],
    hidden_overlays: Option<Vec<bool>>,
    pub lights: usize,
    pub triangles: usize,
}
impl LightMeshes {
    pub fn new() -> Result<Self, macroquad::Error> {
        use macroquad::miniquad::{
            BlendFactor, BlendState, BlendValue, Comparison, CullFace, Equation, PipelineParams,
        };
        let material = load_material(
            ShaderSource::Glsl {
                vertex: r#"#version 100
attribute vec3 position;attribute vec2 texcoord;attribute vec4 color0;
varying lowp vec3 color;varying highp float fogDistance;
uniform mat4 Model;uniform mat4 Projection;
uniform vec3 CameraPosition;uniform vec3 CameraForward;uniform vec2 DepthBias;
void main(){
 gl_Position=Projection*Model*vec4(position,1.0);
 gl_Position.z+=DepthBias.y-DepthBias.x*dot(position-CameraPosition,CameraForward);
 color=color0.rgb/255.0*texcoord.x;
 fogDistance=abs(dot(position-CameraPosition,CameraForward));
}
"#,
                fragment: r#"#version 100
precision highp float;
varying lowp vec3 color;varying highp float fogDistance;
uniform vec2 FogRange;
void main(){
 float fogFactor=clamp((FogRange.y-fogDistance)/(FogRange.y-FogRange.x),0.0,1.0);
 gl_FragColor=vec4(color*fogFactor,1.0);
}
"#,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    cull_face: CullFace::Nothing,
                    depth_test: Comparison::LessOrEqual,
                    depth_write: false,
                    // GL_DST_COLOR, GL_ONE: the terrain is brightened by the
                    // mesh colour; the fog colour is black for this pass.
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::DestinationColor),
                        BlendFactor::One,
                    )),
                    ..Default::default()
                },
                uniforms: vec![
                    UniformDesc::new("CameraPosition", UniformType::Float3),
                    UniformDesc::new("CameraForward", UniformType::Float3),
                    UniformDesc::new("DepthBias", UniformType::Float2),
                    UniformDesc::new("FogRange", UniformType::Float2),
                ],
                textures: vec![],
            },
        )?;
        // `renderLightMeshes` sets each light's depth layer, a layer in front
        // of its level's floor: `draw` gives the uniform per level.
        Ok(Self {
            material,
            revision: None,
            levels: Default::default(),
            hidden_overlays: None,
            lights: 0,
            triangles: 0,
        })
    }

    /// Rebuilds the meshes if the lights were placed since the last call, and
    /// says whether it did.
    pub fn sync(&mut self, world: &Workbench) -> bool {
        let lights = &world.point_lights;
        if self.revision == Some(lights.revision) {
            return false;
        }
        self.revision = Some(lights.revision);
        let hidden = self.hidden_overlays.get_or_insert_with(|| {
            super::scene_floors(&world.pack)
                .map(|(_, overlays)| overlays.iter().map(|o| o.color == -1).collect())
                .unwrap_or_default()
        });
        let origin = [lights.origin.x as i32, lights.origin.y as i32];
        let size = [lights.size.x as usize, lights.size.y as usize];
        let sources: Vec<light_mesh::Source<'_>> = lights
            .lights
            .iter()
            .map(|light| {
                let [x, y, z] = light.coordinates;
                light_mesh::Source {
                    level: light.level,
                    tile: [x >> 7, y >> 7],
                    // The pinned loader leaves the fields as `addVertex`
                    // reads them; the corrected placement keeps y horizontal
                    // and the height in z.
                    position: if lights.corrected {
                        [x, z, y]
                    } else {
                        [x, y, z]
                    },
                    radius: light.radius,
                    spans: &light.spans,
                    color: light.color,
                }
            })
            .collect();
        self.levels = Default::default();
        self.lights = 0;
        self.triangles = 0;
        if sources.is_empty() {
            return true;
        }
        let diagonal_layer = rs09_client::world::layer(9).unwrap_or(2);
        for level in 0..4 {
            if !sources.iter().any(|light| light.level == level) {
                continue;
            }
            let mut underlays = vec![0u8; size[0] * size[1]];
            let mut overlays = underlays.clone();
            let mut shapes = underlays.clone();
            let mut rotations = underlays.clone();
            let mut heights = vec![0i32; (size[0] + 1) * (size[1] + 1)];
            for x in 0..=size[0] {
                for y in 0..=size[1] {
                    let t = tile(
                        &world.regions,
                        level,
                        origin[0] + x as i32,
                        origin[1] + y as i32,
                    );
                    heights[x * (size[1] + 1) + y] = t.height;
                    if x < size[0] && y < size[1] {
                        let index = x * size[1] + y;
                        underlays[index] = t.underlay;
                        overlays[index] = t.overlay;
                        shapes[index] = t.shape;
                        rotations[index] = t.rotation;
                    }
                }
            }
            let built = light_mesh::build(
                level,
                &light_mesh::Level {
                    size,
                    underlays: &underlays,
                    overlays: &overlays,
                    shapes: &shapes,
                    rotations: &rotations,
                    heights: &heights,
                },
                &sources,
                |overlay| {
                    usize::from(overlay)
                        .checked_sub(1)
                        .and_then(|id| hidden.get(id))
                        .copied()
                        .unwrap_or(false)
                },
                |x, y| {
                    world
                        .world
                        .locations
                        .get(&rs09_client::world::Slot {
                            plane: level,
                            tile: [origin[0] + x, origin[1] + y],
                            layer: diagonal_layer,
                        })
                        .filter(|location| location.shape == 9)
                        .map(|location| i32::from(location.rotation))
                },
            );
            for (index, mesh) in built.into_iter().enumerate() {
                let Some(mesh) = mesh else {
                    continue;
                };
                self.lights += 1;
                self.triangles += mesh.indices.len() / 3;
                let center = vec2(
                    origin[0] as f32 + sources[index].tile[0] as f32 + 0.5,
                    origin[1] as f32 + sources[index].tile[1] as f32 + 0.5,
                );
                // One batch per light keeps indices within 16 bits and lets a
                // distant light be skipped whole.
                let mut batch = Batch {
                    mesh: Mesh {
                        vertices: Vec::with_capacity(mesh.positions.len()),
                        indices: Vec::with_capacity(mesh.indices.len()),
                        texture: None,
                    },
                    lights: vec![(0..mesh.positions.len(), index)],
                    center,
                    tiles: span_tiles(&sources[index], origin, |tile| {
                        tile_exists(world, level, tile)
                    }),
                };
                if mesh.positions.len() > usize::from(u16::MAX) {
                    continue;
                }
                for (position, color) in mesh.positions.iter().zip(&mesh.colors) {
                    batch.mesh.vertices.push(Vertex {
                        position: vec3(
                            origin[0] as f32 + position[0] as f32 / 128.0 - 3200.0,
                            -(position[1] as f32) / 128.0,
                            origin[1] as f32 + position[2] as f32 / 128.0 - 3200.0,
                        ),
                        uv: Vec2::ZERO,
                        color: [color[0], color[1], color[2], 255],
                        normal: Vec4::ZERO,
                    });
                }
                batch
                    .mesh
                    .indices
                    .extend(mesh.indices.iter().map(|&i| i as u16));
                self.levels[level].push(batch);
            }
        }
        true
    }

    /// `renderLightMeshes`: a light's whole mesh is drawn when any tile of
    /// its span profile exists and is visible, which here means roof removal
    /// has left one standing (`tile.visible` also covers the view frustum,
    /// which the pass leaves to clipping). `Light.onBridge` is not applied.
    pub fn draw(
        &mut self,
        world: &Workbench,
        params: rs09_client::atmosphere::Parameters,
        camera: &super::SceneCamera,
        position: Vec2,
    ) {
        if self.lights == 0 {
            return;
        }
        self.material.set_uniform("CameraPosition", camera.position);
        self.material.set_uniform(
            "CameraForward",
            (camera.target - camera.position).normalize(),
        );
        self.material.set_uniform(
            "FogRange",
            Vec2::from_array(params.fog_range(56 * 128, 512.0)) / 128.0,
        );
        gl_use_material(&self.material);
        for (level, batches) in self.levels.iter_mut().enumerate() {
            if batches.is_empty() {
                continue;
            }
            self.material.set_uniform(
                "DepthBias",
                Vec2::from(rs09_client::depth_layers::coefficients(
                    rs09_client::depth_layers::light_mesh(level),
                )),
            );
            for batch in batches {
                if batch.center.distance(position) >= 70.0
                    || (!batch.tiles.is_empty()
                        && batch
                            .tiles
                            .iter()
                            .all(|&tile| world.roofs.hides_floor(level, tile)))
                {
                    continue;
                }
                for (range, light) in &batch.lights {
                    let alpha = world
                        .point_lights
                        .alphas
                        .get(*light)
                        .copied()
                        .unwrap_or(0.0);
                    for vertex in &mut batch.mesh.vertices[range.clone()] {
                        vertex.uv.x = alpha;
                    }
                }
                draw_mesh(&batch.mesh);
            }
        }
    }
}

/// The world tiles `renderLightMeshes` walks for a light: its span profile's
/// rows from `y - radius`, each from `x + (span >> 8) - radius` for
/// `span & 255` tiles, less those `exists` rejects (a null `Tile`). `origin`
/// is the light grid's south-west tile.
fn span_tiles(
    light: &light_mesh::Source<'_>,
    origin: [i32; 2],
    exists: impl Fn([i32; 2]) -> bool,
) -> Vec<[i32; 2]> {
    let mut tiles = Vec::new();
    for (row, &span) in light.spans.iter().enumerate() {
        let span = i32::from(span);
        let y = light.tile[1] - light.radius + row as i32;
        let start = light.tile[0] + (span >> 8) - light.radius;
        for x in start..start + (span & 0xff) {
            let tile = [origin[0] + x, origin[1] + y];
            if exists(tile) {
                tiles.push(tile);
            }
        }
    }
    tiles
}

/// Whether the scene has a `Tile` at a world tile of `level`: the original
/// makes one, on its level and every level below, for each floor it sets
/// and each location it adds. A location counts on its own south-west tile
/// only here, not over the rest of a larger footprint.
fn tile_exists(world: &Workbench, level: usize, position: [i32; 2]) -> bool {
    (level..4).any(|plane| {
        let t = tile(&world.regions, plane, position[0], position[1]);
        t.underlay != 0
            || t.overlay != 0
            || (0..4).any(|layer| {
                world
                    .world
                    .locations
                    .contains_key(&rs09_client::world::Slot {
                        plane,
                        tile: position,
                        layer,
                    })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests against SceneGraph.addLoc's wall flags
    // (WALL_ROTATION_TYPE1, WALL_ROTATION_FLAGS) and the temporary-entity box.
    #[test]
    fn locations_are_lit_as_the_piece_of_scenery_addloc_makes_them() {
        let wall = |flag, diagonal| Object::Wall {
            flag,
            diagonal,
            raised: false,
        };
        assert_eq!(location_object(0, 0, [1, 1], false, 0), wall(1, None));
        assert_eq!(location_object(0, 3, [1, 1], false, 0), wall(8, None));
        assert_eq!(location_object(1, 2, [1, 1], false, 0), wall(64, Some(2)));
        assert_eq!(location_object(3, 1, [1, 1], false, 0), wall(32, None));
        // A corner wall's two models: this side, then the next one round.
        assert_eq!(location_object(2, 3, [1, 1], false, 0), wall(8, None));
        assert_eq!(location_object(2, 3, [1, 1], false, 1), wall(1, None));
        assert_eq!(
            location_object(9, 3, [1, 1], false, 0),
            Object::DiagonalScenery { rotation: 3 }
        );
        // Decorations are lit from their own tile; scenery over its footprint.
        assert_eq!(
            location_object(5, 1, [3, 2], false, 0),
            Object::Area { size: [1, 1] }
        );
        assert_eq!(
            location_object(10, 1, [3, 2], false, 0),
            Object::Area { size: [3, 2] }
        );
    }

    // Transcription test against `LightingManager.renderLightMeshes`' walk.
    #[test]
    fn a_light_walks_its_span_profile_from_its_radius() {
        // Radius 1 at tile (10, 20): rows 19 to 21; the first row starts one
        // tile in and is one tile long, the others start at the edge.
        let light = light_mesh::Source {
            level: 1,
            tile: [10, 20],
            position: [0; 3],
            radius: 1,
            spans: &[0x0101, 0x0003, 0x0002],
            color: 0,
        };
        assert_eq!(
            span_tiles(&light, [3000, 3100], |_| true),
            [
                [3010, 3119],
                [3009, 3120],
                [3010, 3120],
                [3011, 3120],
                [3009, 3121],
                [3010, 3121]
            ]
        );
        // A tile the scene does not have is never looked at.
        assert_eq!(
            span_tiles(&light, [3000, 3100], |tile| tile[1] == 3119),
            [[3010, 3119]]
        );
    }

    #[test]
    fn an_actor_spans_the_tiles_its_sixty_unit_box_touches() {
        assert_eq!(actor_area([640 + 64, 1280 + 64], 1), ([5, 10], [1, 1]));
        // Three units from the tile's edge the box reaches the next tile.
        assert_eq!(actor_area([640 + 3, 1280 + 64], 1), ([4, 10], [2, 1]));
        assert_eq!(actor_area([640 + 128, 1280 + 128], 2), ([5, 10], [2, 2]));
    }

    #[test]
    fn a_stamped_vertex_keeps_its_material_and_takes_the_draw_level() {
        let mut vertices = [Vertex {
            position: vec3(10.5 - 3200.0 + 3136.0, 0.0, 20.5 - 3200.0 + 3136.0),
            uv: Vec2::ZERO,
            color: [0; 4],
            normal: vec4(0.0, 1.0, 0.0, 37.0 + 2048.0),
        }];
        stamp(
            &mut vertices,
            vec2(3136.0, 3136.0),
            Object::Area { size: [2, 1] },
            [3136 + 10, 3136 + 20],
            0,
        );
        let w = vertices[0].normal.w;
        let key = (w / KEY_UNIT).floor();
        assert_eq!(w - key * KEY_UNIT, 37.0);
        assert_eq!(
            key as u32,
            light_select::key(Object::Area { size: [2, 1] }, [10, 20], [10, 20])
        );
    }
}
