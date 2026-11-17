//! The scene builder's side of the HD floor: the original floor groups
//! (`gl_tile`) cut into chunk meshes, fed by the shade walls and scenery bake
//! into the ground (`tile_shade`), and the merged normals of neighbouring
//! scenery (`normal_merge`) written back into the placed meshes.
//!
//! The original draws, per level, every underlay group, then every overlay
//! group with its blended faces a hundred depth layers nearer and without
//! depth writes. Chunks keep that order: all floor chunks of a level come
//! before those of the next and before every location, and a chunk's blended
//! faces are separate meshes flagged `blend`.
use super::{PickObject, PreparedChunk, Scene, render, tile};
use macroquad::prelude::*;
use rs09_client::{
    floor::{Overlay, Underlay},
    gl_tile,
    interaction::LocationType,
    loc_placement,
    model::Model,
    normal_merge::{self, Body, Piece, Placed},
    surface,
    terrain::Region,
    tile_shade,
};
use std::collections::{BTreeMap, HashMap, VecDeque};

/// Border tiles around the loaded regions. The builders leave the outermost
/// ring unbuilt and the ring inside it without shade, as at the edge of the
/// original's 104-tile scene; two rings keep both off the loaded tiles.
const PAD: i32 = 2;

/// A static location model as it was placed into a chunk mesh.
struct Record {
    body: Body,
    /// The packed vertex of each emitted mesh vertex, three per drawn face.
    corners: Vec<[u32; 3]>,
    /// Colour, texture and alpha of each drawn face, and the model's ambient
    /// intensity: what a vertex colour is packed from again when its merged
    /// normal changes its baked lightness.
    paint: Vec<(u16, i32, u8)>,
    ambient: i32,
    contrast: i32,
    chunk: usize,
    vertices: std::ops::Range<usize>,
}

#[derive(Default)]
pub(super) struct Builder {
    records: BTreeMap<[i32; 6], VecDeque<Record>>,
}

struct Segment {
    mesh: Mesh,
    blend: bool,
}

impl Builder {
    /// Notes one entity of a static location (`loc_placement`'s pieces, the
    /// first before the second) whose model `loc_mesh` has just written to
    /// the vertex range of the mesh that becomes the chunk given by `at`.
    /// `model` is the entity as the scene holds it: turned and aligned to
    /// the terrain, its vertices relative to where the scene draws it.
    pub fn placed(
        &mut self,
        placement: [i32; 6],
        model: &Model,
        textures: &render::Textures,
        kind: &LocationType,
        (chunk, vertices): (usize, std::ops::Range<usize>),
    ) {
        let computed;
        let order = if model.draw_order.is_empty() {
            computed = surface::face_order(model, &textures.materials);
            &computed
        } else {
            &model.draw_order
        };
        let faces: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&i| surface::visible(model, i, &textures.materials))
            .collect();
        // Without prepared surfaces there are no packed vertices to merge.
        let mergeable = kind.mergeable
            && model.surfaces.len() == model.faces.len()
            && faces.len() * 3 == vertices.len();
        let (body, corners) = if model.surfaces.len() == model.faces.len() {
            Body::from_model(model, &faces, mergeable)
        } else {
            let mut body = Body::solid();
            body.vertices = model.vertices.clone();
            (body, Vec::new())
        };
        self.records
            .entry(placement)
            .or_default()
            .push_back(Record {
                body,
                corners,
                paint: faces
                    .iter()
                    .map(|&i| (model.colors[i], model.textures[i], model.alpha[i]))
                    .collect(),
                ambient: if model.lighting != [0; 2] {
                    model.lighting[0]
                } else {
                    64
                },
                contrast: if model.lighting[1] > 0 {
                    model.lighting[1]
                } else {
                    768
                },
                chunk,
                vertices,
            });
    }

    /// Builds the floor and puts its chunks before the location chunks, then
    /// merges the normals of the placed locations.
    #[allow(clippy::too_many_arguments)]
    pub async fn finish(
        mut self,
        scene: &Scene,
        regions: &[Region],
        textures: &render::Textures,
        types: &BTreeMap<u32, LocationType>,
        underlays: &[Underlay],
        overlays: &[Overlay],
        chunks: &mut Vec<PreparedChunk>,
        pick_objects: &mut [PickObject],
        faces: &mut usize,
    ) {
        let Some((origin, width, depth)) = grid(scene, regions) else {
            return;
        };
        let size = [width as i32, depth as i32];
        // Corner grids hold one more row and column than the tile grids.
        let mut heights: [Vec<i32>; 4] =
            std::array::from_fn(|_| vec![0; (width + 1) * (depth + 1)]);
        let mut grids: [[Vec<u8>; 5]; 4] =
            std::array::from_fn(|_| std::array::from_fn(|_| vec![0; width * depth]));
        for level in 0..4 {
            for x in 0..=width {
                for z in 0..=depth {
                    let t = tile(regions, level, origin[0] + x as i32, origin[1] + z as i32);
                    heights[level][x * (depth + 1) + z] = corner(scene, [x, z], t.height);
                    if x < width && z < depth {
                        let fields = [t.underlay, t.overlay, t.shape, t.rotation, t.flags];
                        for (grid, value) in grids[level].iter_mut().zip(fields) {
                            grid[x * depth + z] = value;
                        }
                    }
                }
            }
        }
        let height = |level: usize, x: i32, z: i32| {
            if level < 4 && (0..=size[0]).contains(&x) && (0..=size[1]).contains(&z) {
                heights[level][x as usize * (depth + 1) + z as usize]
            } else {
                0
            }
        };

        // SceneGraph.addLoc over the landscape, in its order.
        let mut shade = tile_shade::ShadowMap::new(width, depth);
        clear_shade(scene, &mut shade);
        let mut slots = normal_merge::Scene::new(width, depth);
        let mut merging = Vec::new();
        for (index, placement @ [id, x, z, level, shape, rotation]) in
            scene.locations.iter().copied().enumerate()
        {
            if index % 256 == 0 {
                rs09_client::slice::yield_point().await;
            }
            let (Some(kind), Some(d)) = (
                types.get(&(id as u32)),
                scene.definitions.get(&id.to_string()),
            ) else {
                continue;
            };
            if !(0..4).contains(&level) {
                continue;
            }
            // addLoc returns at once for a type hidden in HD and for ground
            // decoration the preference leaves out: no slot and no shade.
            if kind.placement.hidden_in_hd
                || (shape == 22
                    && !loc_placement::ground_decoration_shown(
                        loc_placement::ground_decorations(),
                        kind,
                    ))
            {
                continue;
            }
            let level = level as usize;
            let at = [x - origin[0], z - origin[1]];
            let animated = loc_placement::is_entity(d, kind);
            let mut pieces = [Piece::None, Piece::None];
            let mut records = Vec::new();
            if !(4..=8).contains(&shape) {
                for piece in pieces.iter_mut().take(if shape == 2 { 2 } else { 1 }) {
                    if animated {
                        *piece = Piece::Animated;
                    } else if let Some(mut record) = self
                        .records
                        .get_mut(&placement)
                        .and_then(VecDeque::pop_front)
                    {
                        let body = std::mem::take(&mut record.body);
                        records.push((body.mergeable.then(|| body.normals.clone()), record));
                        *piece = Piece::Model(body);
                    }
                }
            }
            let entities = normal_merge::add_loc(
                &mut slots,
                &mut shade,
                Placed {
                    level,
                    tile: at,
                    shape: shape as u8,
                    rotation: rotation as u8,
                    size: [d.width, d.length],
                    active: kind.active,
                    pieces,
                },
            );
            for ((before, record), entity) in
                records.into_iter().zip(entities.into_iter().flatten())
            {
                if let Some(before) = before {
                    merging.push((entity, before, record));
                }
            }
        }

        // The floor of each level, grouped as the original draws it.
        // Rasteriser.palette: 0.7 with high detail lighting.
        let palette = rs09_client::sd_light::palette();
        let baked = rs09_client::sd_light::enabled();
        let materials: Vec<[u8; 2]> = textures
            .materials
            .iter()
            .map(|m| m.as_ref().map_or([0; 2], |m| [m.brightness, m.speed]))
            .collect();
        let tables = gl_tile::Tables {
            underlays,
            overlays,
            palette: &palette,
            textures: &materials,
        };
        let mut segments = BTreeMap::<(usize, i32, i32), Vec<Segment>>::new();
        for level in 0..4 {
            rs09_client::slice::yield_point().await;
            let [under, over, shapes, rotations, flags] = &grids[level];
            let colors: Vec<i32> = rs09_client::floor::blend(width, depth, under, underlays)
                .into_iter()
                .map(i32::from)
                .collect();
            rs09_client::slice::yield_point().await;
            // SceneGraph.buildScene's two loops for the hardware renderer:
            // the shade alone, or slope and shade.
            let lightness = if baked {
                shade.lightness_sd(level, &heights[level], rs09_client::sd_light::sun())
            } else {
                shade.lightness(level)
            };
            rs09_client::slice::yield_point().await;
            let floor = gl_tile::build_sliced(
                &gl_tile::Level {
                    width,
                    depth,
                    level,
                    underlays: under,
                    overlays: over,
                    shapes,
                    rotations,
                    flags,
                    bridge_flags: &grids[1][4],
                    heights: &heights[level],
                    lightness: &lightness,
                    colors: &colors,
                    water: None,
                },
                &tables,
            )
            .await;
            for group in floor.groups() {
                rs09_client::slice::yield_point().await;
                // Upper-level tiles the original draws with the ground level
                // go to ground chunks, as the builder has always placed them.
                let mut by_chunk = BTreeMap::<(usize, i32, i32), Vec<&gl_tile::Face>>::new();
                for face in &group.faces {
                    let plane = if level != 0 && face.level == 0 {
                        0
                    } else {
                        level
                    };
                    by_chunk
                        .entry((plane, (face.x + origin[0]) >> 4, (face.y + origin[1]) >> 4))
                        .or_default()
                        .push(face);
                }
                for (key, faces) in by_chunk {
                    let fan = |face: &gl_tile::Face| -> Vec<u32> {
                        (1..face.vertices.len().saturating_sub(1))
                            .flat_map(|i| {
                                [face.vertices[0], face.vertices[i], face.vertices[i + 1]]
                            })
                            .collect()
                    };
                    let opaque: Vec<u32> = faces
                        .iter()
                        .filter(|f| !f.blend)
                        .flat_map(|f| fan(f))
                        .collect();
                    let blended: Vec<u32> = faces
                        .iter()
                        .flat_map(|f| {
                            let mut indices = f.blend_indices.clone().unwrap_or_default();
                            if f.blend {
                                indices.extend(fan(f));
                            }
                            indices
                        })
                        .collect();
                    let list = segments.entry(key).or_default();
                    for (indices, blend) in [(opaque, false), (blended, true)] {
                        emit(list, textures, group, &indices, blend, level, origin);
                    }
                }
            }
        }
        let floor_chunks: Vec<PreparedChunk> = segments
            .into_iter()
            .flat_map(|((plane, cx, cz), list)| {
                let center = vec2((cx * 16) as f32 + 8.0, (cz * 16) as f32 + 8.0);
                list.into_iter().map(move |segment| {
                    let mut chunk = PreparedChunk::new(segment.mesh, center, plane);
                    chunk.blend = segment.blend;
                    chunk.floor = true;
                    chunk
                })
            })
            .collect();
        *faces += floor_chunks
            .iter()
            .map(|c| c.mesh.indices.len() / 3)
            .sum::<usize>();
        let shift = floor_chunks.len();
        chunks.splice(0..0, floor_chunks);
        for object in pick_objects.iter_mut() {
            object.chunk += shift;
        }

        // SceneGraph.mergeSceneNormals, then the changed normals go back
        // into the meshes the way render::model scaled the originals.
        rs09_client::slice::yield_point().await;
        slots.merge(height);
        let mut changed = std::collections::BTreeSet::new();
        for (entity, before, record) in merging {
            let after = &slots.bodies[entity].normals;
            let Some(chunk) = chunks.get_mut(record.chunk + shift) else {
                continue;
            };
            if *after == before || record.corners.len() * 3 != record.vertices.len() {
                continue;
            }
            let bake = bake_of(&record);
            for (face, corners) in record.corners.iter().enumerate() {
                for (corner, &packed) in corners.iter().enumerate() {
                    let n = after[packed as usize];
                    if n == before[packed as usize] {
                        continue;
                    }
                    let scale = if n[3] == 0 {
                        3.0 / (record.contrast + record.contrast / 2) as f32
                    } else {
                        (3.0 / record.contrast as f32) / f32::from(n[3])
                    };
                    let Some(vertex) = chunk
                        .mesh
                        .vertices
                        .get_mut(record.vertices.start + face * 3 + corner)
                    else {
                        continue;
                    };
                    let normal = vec3(f32::from(n[0]), -f32::from(n[1]), f32::from(n[2])) * scale;
                    vertex.normal = normal.extend(vertex.normal.w);
                    // The colours are packed when the model is first drawn,
                    // after the merge, so a baked lightness is the merged
                    // normal's.
                    if let (Some(bake), Some(&(color, texture, alpha))) =
                        (&bake, record.paint.get(face))
                    {
                        let material = usize::try_from(texture)
                            .ok()
                            .and_then(|id| textures.materials.get(id)?.as_ref());
                        vertex.color = surface::pack_color(
                            color,
                            material,
                            bake.light.lightness(n.map(i32::from)),
                            alpha,
                            &bake.palette,
                        );
                    }
                }
            }
            changed.insert(record.chunk + shift);
        }
        #[cfg(not(target_arch = "wasm32"))]
        for index in changed {
            chunks[index].fingerprint = super::mesh_fingerprint(&chunks[index].mesh);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = changed;
    }
}

/// The tile grid the floor is built over: south-west tile, width and depth.
/// A title scene is the original's 104-tile scene itself, whose outer rings
/// the builders treat as the original does; any other scene is its loaded
/// regions with [`PAD`] border tiles.
pub(super) fn grid(scene: &Scene, regions: &[Region]) -> Option<([i32; 2], usize, usize)> {
    if let Some(window) = &scene.window {
        let size = rs09_client::title_window::SIZE as usize;
        return Some((window.base, size, size));
    }
    let min_x = regions.iter().map(|r| r.x * 64).min()?;
    let min_z = regions.iter().map(|r| r.z * 64).min()?;
    let origin = [min_x - PAD, min_z - PAD];
    let width = (regions.iter().map(|r| r.x * 64 + 64).max()? - origin[0] + PAD) as usize;
    let depth = (regions.iter().map(|r| r.z * 64 + 64).max()? - origin[1] + PAD) as usize;
    Some((origin, width, depth))
}

/// The height of grid corner `at` given the regions' height there.
pub(super) fn corner(scene: &Scene, at: [usize; 2], height: i32) -> i32 {
    scene
        .window
        .as_ref()
        .map_or(height, |window| window.corner_height(at, height))
}

/// `clearTerrainRegion`'s shade for the title scene's regions without
/// terrain, every level, before any location shades the ground
/// (LoginManager.java:1525-1535, SceneGraph.java:5073-5079).
fn clear_shade(scene: &Scene, shade: &mut tile_shade::ShadowMap) {
    let Some(window) = scene.window.as_ref().filter(|window| window.clears()) else {
        return;
    };
    for &[x, z] in &window.unread {
        let [lx, lz] = window.local([x * 64, z * 64]);
        for level in 0..4 {
            shade.clear_region(level, lx, lz, 64, 64);
        }
    }
}

/// What a placed model's colours are packed with without high detail
/// lighting (`render::model` does the same for the unmerged normals).
fn bake_of(record: &Record) -> Option<rs09_client::sd_light::Bake> {
    rs09_client::sd_light::bake(record.ambient, record.contrast)
}

/// Appends the triangles `indices` of a floor group to the last mesh of a
/// chunk when that mesh is of the same kind and has room, else to a new one.
fn emit(
    list: &mut Vec<Segment>,
    textures: &render::Textures,
    group: &gl_tile::GlTile,
    indices: &[u32],
    blend: bool,
    level: usize,
    origin: [i32; 2],
) {
    if indices.is_empty() {
        return;
    }
    if !list.last().is_some_and(|last| {
        last.blend == blend && last.mesh.vertices.len() + indices.len() <= 60000
    }) {
        list.push(Segment {
            mesh: textures.mesh(),
            blend,
        });
    }
    let mesh = &mut list.last_mut().unwrap().mesh;
    let scale = if group.scale == 0.0 {
        128.0
    } else {
        group.scale
    };
    let material = if group.texture < 0 {
        1023
    } else {
        group.texture
    };
    // The water material makes its own coordinates from the position and
    // reads the tile's `FloType.waterColor` from the texture coordinate.
    let water = usize::try_from(group.texture)
        .ok()
        .and_then(|id| textures.materials.get(id)?.as_ref())
        .filter(|material| material.material_type == rs09_client::materials::WATER)
        .map(|_| Vec2::from_array(rs09_client::materials::water_uv(group.water_color as u32)));
    let mut placed = HashMap::new();
    for &index in indices {
        let i = index as usize;
        let slot = *placed.entry(index).or_insert_with(|| {
            let color = group.colors[i];
            let n = group.normals[i];
            mesh.vertices.push(Vertex {
                position: vec3(
                    group.x[i] as f32 / 128.0 + (origin[0] - 3200) as f32,
                    -(group.y[i] as f32) / 128.0,
                    group.z[i] as f32 / 128.0 + (origin[1] - 3200) as f32,
                ),
                // The original measures from its scene base, a multiple of
                // eight tiles; world coordinates give the same phase for the
                // scales that divide 1024.
                uv: water.unwrap_or_else(|| {
                    vec2(
                        (group.x[i] + origin[0] * 128) as f32 / scale,
                        (group.z[i] + origin[1] * 128) as f32 / scale,
                    )
                }),
                color: [
                    (color >> 24) as u8,
                    (color >> 16) as u8,
                    (color >> 8) as u8,
                    color as u8,
                ],
                normal: vec4(
                    n[0],
                    -n[1],
                    n[2],
                    material as f32 + 1024.0 + level as f32 * 2048.0,
                ),
            });
            (mesh.vertices.len() - 1) as u16
        });
        mesh.indices.push(slot);
    }
}
