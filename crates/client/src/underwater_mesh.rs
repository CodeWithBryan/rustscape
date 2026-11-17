//! The scene builder's and the frame's side of the underwater scene
//! (`rs09_client::underwater`): its floor, its scenery and the backdrop of the
//! surface's water as chunk meshes, and the pass that draws them before the
//! surface.
//!
//! The scene is the surface floor's grid (`terrain_mesh::grid`): in game the
//! loaded regions with a border, not the original's 104-tile window, so what
//! the original decides per scene (whether any region has underwater files,
//! which regions lack them and are cleared) is decided over the loaded
//! regions; a title scene is the original's 104 tiles (`title_window`).
//!
//! Chunks carry their [`Pass`]; the surface's draw never sees them.
use super::{PreparedChunk, Scene, loc_mesh, render, scene_lights, tile};
use macroquad::prelude::*;
use rs09_client::{
    floor::{Overlay, Underlay},
    gl_tile,
    interaction::LocationType,
    loc_placement, materials,
    model::Model,
    terrain::Region,
    underwater::{self, Pass, Source},
};
use std::collections::BTreeMap;

/// Sixteen-bit indices bound a mesh.
const LIMIT: usize = 60000;

/// Meshes of one pass kind, one per 16 x 16 tile chunk and pass parameters,
/// split where the vertex limit is reached.
#[derive(Default)]
struct Meshes<K: Ord + Copy> {
    list: BTreeMap<([i32; 2], K), Vec<Mesh>>,
}
impl<K: Ord + Copy> Meshes<K> {
    /// The mesh to append `vertices` more to.
    fn mesh(
        &mut self,
        textures: &render::Textures,
        chunk: [i32; 2],
        key: K,
        vertices: usize,
    ) -> &mut Mesh {
        let list = self.list.entry((chunk, key)).or_default();
        if !list
            .last()
            .is_some_and(|last| last.vertices.len() + vertices <= LIMIT)
        {
            list.push(textures.mesh());
        }
        list.last_mut().unwrap()
    }
    fn finish(self, chunks: &mut Vec<PreparedChunk>, faces: &mut usize, pass: impl Fn(K) -> Pass) {
        for (([cx, cz], key), list) in self.list {
            for mesh in list {
                if mesh.indices.is_empty() {
                    continue;
                }
                *faces += mesh.indices.len() / 3;
                let center = vec2((cx * 16) as f32 + 8.0, (cz * 16) as f32 + 8.0);
                let mut chunk = PreparedChunk::new(mesh, center, 0);
                chunk.underwater = Some(pass(key));
                chunks.push(chunk);
            }
        }
    }
}

/// Appends the triangles `indices` of a floor group to `mesh`. `attributes`
/// gives a vertex its texture coordinate, colour and `normal.w`.
fn emit(
    mesh: &mut Mesh,
    group: &gl_tile::GlTile,
    indices: &[u32],
    origin: [i32; 2],
    attributes: impl Fn(usize) -> (Vec2, [u8; 4], f32),
) {
    let mut placed = BTreeMap::new();
    for &index in indices {
        let i = index as usize;
        let slot = *placed.entry(index).or_insert_with(|| {
            let n = group.normals[i];
            let (uv, color, key) = attributes(i);
            mesh.vertices.push(Vertex {
                position: vec3(
                    group.x[i] as f32 / 128.0 + (origin[0] - 3200) as f32,
                    -(group.y[i] as f32) / 128.0,
                    group.z[i] as f32 / 128.0 + (origin[1] - 3200) as f32,
                ),
                uv,
                color,
                normal: vec4(n[0], -n[1], n[2], key),
            });
            (mesh.vertices.len() - 1) as u16
        });
        mesh.indices.push(slot);
    }
}

fn fan(face: &gl_tile::Face) -> Vec<u32> {
    (1..face.vertices.len().saturating_sub(1))
        .flat_map(|i| [face.vertices[0], face.vertices[i], face.vertices[i + 1]])
        .collect()
}

/// Whether every region of the scene says what underwater files it has: then
/// the water is drawn as the original draws it, translucent over whatever the
/// underwater pass left (the cleared fog colour where it left nothing).
/// Otherwise (a pack made before underwater maps were packed, an instanced
/// scene) the water keeps compositing over its own backdrop.
pub(super) fn exact(regions: &[Region]) -> bool {
    !regions.is_empty()
        && regions
            .iter()
            .all(|region| !matches!(region.underwater, Source::Unknown))
}

/// The underwater files of a prepared pack's regions, with the definitions
/// of the types they place. A pack lists its underwater files under
/// `underwater/`; one without the `underwater-complete` entry predates them
/// and says nothing.
pub(super) fn load_pack(
    pack: &rs09_client::pack::Pack,
    scene: &mut Scene,
    types: &mut BTreeMap<u32, LocationType>,
    regions: &mut [Region],
) -> Result<(), String> {
    if pack.get("underwater-complete").is_err() {
        return Ok(());
    }
    for region in regions {
        let (x, z) = (region.x, region.z);
        region.underwater = Source::decode(
            [x, z],
            pack.get(&format!("underwater/terrain/{x}_{z}")).ok(),
            pack.get(&format!("underwater/locations/{x}_{z}")).ok(),
        )
        .map_err(|e| format!("underwater map {x}_{z}: {e}"))?;
        let Some(layer) = region.underwater.layer() else {
            continue;
        };
        for &[id, ..] in &layer.locations {
            let Ok(key) = u32::try_from(id) else {
                continue;
            };
            if types.contains_key(&key) {
                continue;
            }
            // A type the pack lacks is the original's default: no models.
            let Ok(raw) = pack.get(&format!("loc/{id}")) else {
                continue;
            };
            let (interaction, definition, _) =
                LocationType::decode_with_visual(raw).map_err(|e| format!("loc/{id}: {e}"))?;
            types.insert(key, interaction);
            scene.definitions.insert(id.to_string(), definition);
        }
    }
    Ok(())
}

/// Builds the underwater scene of the loaded regions and appends its chunks.
/// Nothing is built when no region has underwater files.
#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    scene: &Scene,
    regions: &[Region],
    models: &BTreeMap<u32, Model>,
    textures: &render::Textures,
    types: &BTreeMap<u32, LocationType>,
    underlays: &[Underlay],
    overlays: &[Overlay],
    chunks: &mut Vec<PreparedChunk>,
    faces: &mut usize,
) -> Result<(), String> {
    if !regions.iter().any(|r| r.underwater.layer().is_some()) {
        return Ok(());
    }
    let min_x = regions.iter().map(|r| r.x * 64).min().unwrap();
    let min_z = regions.iter().map(|r| r.z * 64).min().unwrap();
    // The same grid as the surface floor (a title scene: the original's).
    let Some((origin, width, depth)) = super::terrain_mesh::grid(scene, regions) else {
        return Ok(());
    };

    // The surface's ground level, as `terrain_mesh` lays it out.
    let mut heights = vec![0; (width + 1) * (depth + 1)];
    let mut grids: [Vec<u8>; 6] = std::array::from_fn(|_| vec![0; width * depth]);
    for x in 0..=width {
        for z in 0..=depth {
            let (ax, az) = (origin[0] + x as i32, origin[1] + z as i32);
            let t = tile(regions, 0, ax, az);
            heights[x * (depth + 1) + z] = super::terrain_mesh::corner(scene, [x, z], t.height);
            if x < width && z < depth {
                let bridge = tile(regions, 1, ax, az).flags;
                let fields = [t.underlay, t.overlay, t.shape, t.rotation, t.flags, bridge];
                for (grid, value) in grids.iter_mut().zip(fields) {
                    grid[x * depth + z] = value;
                }
            }
        }
    }
    let [under, over, shapes, rotations, flags, bridge_flags] = &grids;
    let is_water = |texture: i32| {
        usize::try_from(texture)
            .ok()
            .and_then(|id| textures.materials.get(id)?.as_ref())
            .is_some_and(|material| material.material_type == materials::WATER)
    };
    let water = underwater::water_data(width, depth, under, over, overlays, is_water);

    // LoginManager.rebuildMap with hasUnderWaterMap: every region's map, the
    // regions without one cleared, then every region's locations.
    let mut ordered: Vec<&Region> = regions.iter().collect();
    ordered.sort_by_key(|r| (r.x, r.z));
    let local = |r: &Region| [r.x * 64 - origin[0], r.z * 64 - origin[1]];
    let mut scene_under = underwater::Scene::new(width, depth, water);
    for region in &ordered {
        if let Some(tiles) = region.underwater.layer().and_then(|l| l.tiles.as_ref()) {
            scene_under.read(local(region), tiles, &heights);
        }
    }
    // `SceneGraph.centralZoneY < 800`: only on the surface map.
    let clears = scene
        .window
        .as_ref()
        .map_or((scene.start[1] >> 3) < 800, |window| window.clears());
    if clears {
        for region in &ordered {
            if region
                .underwater
                .layer()
                .is_none_or(|layer| layer.tiles.is_none())
            {
                scene_under.clear(local(region));
            }
        }
    }
    let placements: Vec<[i32; 6]> = ordered
        .iter()
        .filter_map(|region| region.underwater.layer())
        .flat_map(|layer| &layer.locations)
        // readLocs: a title scene keeps tiles 1 to 102 only.
        .filter(|&&[_, x, z, ..]| {
            scene
                .window
                .as_ref()
                .is_none_or(|window| window.keeps_location(x, z))
        })
        .map(|&[id, x, z, level, shape, rotation]| {
            [id, x - origin[0], z - origin[1], level, shape, rotation]
        })
        .collect();
    let lookup = |id: i32| {
        Some((
            scene.definitions.get(&id.to_string())?,
            types.get(&u32::try_from(id).ok()?)?,
        ))
    };
    let planned =
        scene_under.add_locations(&placements, lookup, loc_placement::ground_decorations());
    scene_under.finish();

    // Rasteriser.palette: 0.7 with high detail lighting.
    let palette = rs09_client::sd_light::palette();
    let texture_facts: Vec<[u8; 2]> = textures
        .materials
        .iter()
        .map(|m| m.as_ref().map_or([0; 2], |m| [m.brightness, m.speed]))
        .collect();
    let tables = gl_tile::Tables {
        underlays,
        overlays,
        palette: &palette,
        textures: &texture_facts,
    };
    let chunk_of = |x: i32, z: i32| [(x + origin[0]) >> 4, (z + origin[1]) >> 4];

    // UnderwaterMaterialRenderer.applyFogFade: the surface's ground-level
    // water faces where an underwater tile exists, in their water colour.
    // Only positions matter, so the surface floor is built unlit.
    let unlit = vec![0; width * depth];
    let surface = gl_tile::build(
        &gl_tile::Level {
            width,
            depth,
            level: 0,
            underlays: under,
            overlays: over,
            shapes,
            rotations,
            flags,
            bridge_flags,
            heights: &heights,
            lightness: &unlit,
            colors: &unlit,
            water: None,
        },
        &tables,
    );
    let mut backdrop = Meshes::<()>::default();
    for group in surface.groups().filter(|g| is_water(g.texture)) {
        let rgb = group.water_color as u32;
        let color = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255];
        let mut by_chunk = BTreeMap::<[i32; 2], Vec<u32>>::new();
        for face in &group.faces {
            // `tiles[facePlane][x][y]` of the one-level underwater scene.
            let exists =
                face.level == 0 && scene_under.tiles[face.x as usize * depth + face.y as usize];
            if !exists {
                continue;
            }
            let indices = by_chunk.entry(chunk_of(face.x, face.y)).or_default();
            indices.extend(face.blend_indices.iter().flatten());
            indices.extend(fan(face));
        }
        for (chunk, indices) in by_chunk {
            let mesh = backdrop.mesh(textures, chunk, (), indices.len());
            // Untextured: the atlas's white cell, as a floor so that no
            // point light is added.
            emit(mesh, group, &indices, origin, |_| {
                (Vec2::ZERO, color, 1023.0 + 1024.0)
            });
        }
    }
    backdrop.finish(chunks, faces, |()| Pass::Backdrop);

    // buildGlPlainTiles with the water arguments, in the groups' order.
    let floor = scene_under.floor(&heights, [flags, bridge_flags], &tables);
    let mut floors = Meshes::<u32>::default();
    for group in &floor.plain {
        let material = if group.texture < 0 {
            1023
        } else {
            group.texture
        };
        // The vertex stage divides the position by the scale it finds where
        // scenery has its light key; that field holds 0 to 2047. Every
        // underlay of the pinned cache's underwater maps is within it.
        let scale = if (1.0..2048.0).contains(&group.scale) {
            group.scale.floor()
        } else {
            128.0
        };
        let key = material as f32 + 1024.0 + scale * scene_lights::KEY_UNIT;
        let mut by_chunk = BTreeMap::<[i32; 2], Vec<u32>>::new();
        for face in &group.faces {
            by_chunk
                .entry(chunk_of(face.x, face.y))
                .or_default()
                .extend(fan(face));
        }
        for (chunk, indices) in by_chunk {
            let mesh = floors.mesh(textures, chunk, group.water_color as u32, indices.len());
            emit(mesh, group, &indices, origin, |i| {
                let color = group.colors[i];
                (
                    vec2(group.depths[i], 0.0),
                    [
                        (color >> 24) as u8,
                        (color >> 16) as u8,
                        (color >> 8) as u8,
                        color as u8,
                    ],
                    key,
                )
            });
        }
    }
    floors.finish(chunks, faces, |water| Pass::Floor { water });

    // The locations, each drawn with its tile's water colour, fog range and
    // surface height. Scenery of several tiles is drawn by whichever of its
    // tiles the original's sweep reaches last; its south-west tile is used.
    let mut scenery = Meshes::<(u32, i32, i32)>::default();
    for (entry, &[id, x, z, _, shape, rotation]) in planned.iter().zip(&placements) {
        let (Some(planned), Some((d, kind))) = (entry, lookup(id)) else {
            continue;
        };
        let entities = scene_under
            .entities(
                &heights,
                [x, z],
                planned,
                d,
                kind,
                &textures.materials,
                |id| models.get(&id),
            )
            .map_err(|id| format!("missing underwater location model {id}"))?;
        let key = scene_under.scenery_material(x, z, &heights);
        for entity in entities {
            let mesh = scenery.mesh(textures, chunk_of(x, z), key, entity.model.faces.len() * 3);
            let first = mesh.vertices.len();
            loc_mesh(
                mesh,
                &entity.model,
                textures,
                vec2(
                    entity.draw[0] as f32 / 128.0 + origin[0] as f32,
                    entity.draw[1] as f32 / 128.0 + origin[1] as f32,
                ),
                -(entity.base as f32) / 128.0,
                0,
            );
            scene_lights::stamp(
                &mut mesh.vertices[first..],
                vec2(min_x as f32, min_z as f32),
                scene_lights::location_object(
                    shape,
                    rotation,
                    planned.placement.size,
                    false,
                    entity.piece,
                ),
                [x + origin[0], z + origin[1]],
                0,
            );
        }
    }
    scenery.finish(chunks, faces, |(water, range, height)| Pass::Scenery {
        water,
        range,
        height,
    });
    Ok(())
}

/// What the frame's draw needs besides the chunks.
pub(super) struct Frame<'a> {
    pub renderer: Option<&'a super::static_render::Renderer>,
    /// The world material and the one without depth writes.
    pub materials: [&'a Material; 2],
    pub camera: &'a super::SceneCamera,
    pub atmosphere: rs09_client::atmosphere::Parameters,
    pub direction: [i32; 3],
    pub time: f32,
    pub shadows: bool,
}

/// Takes the underwater chunks out of the frame's visible static chunks, in
/// the original's order: the backdrop, the floor, the scenery. Also tells the
/// water material whether the scene is drawn exactly (see [`exact`]).
pub(super) fn split(world: &super::Workbench, visible: &mut Vec<usize>) -> Vec<usize> {
    materials::set_underwater_pass(exact(&world.regions));
    let mut under: Vec<usize> = Vec::new();
    visible.retain(|&index| {
        let own = world.chunks[index].underwater.is_some();
        if own {
            under.push(index);
        }
        !own
    });
    // `hasUnderWaterMap` needs high water detail; without it the water is
    // the opaque liquid material and nothing is drawn below it.
    if !materials::high_water_detail() {
        under.clear();
    }
    let order = |index: &usize| match world.chunks[*index].underwater {
        Some(Pass::Backdrop) => 0,
        Some(Pass::Floor { .. }) => 1,
        _ => 2,
    };
    under.sort_by_key(order);
    under
}

/// `SceneGraph.setPlainTile`'s underwater pass, drawn before any surface
/// chunk and right after the scene was cleared.
pub(super) fn draw(world: &mut super::Workbench, chunks: &[usize], frame: Frame<'_>) {
    if chunks.is_empty() {
        return;
    }
    let passes: Vec<(usize, underwater::Uniforms)> = chunks
        .iter()
        .filter_map(|&index| {
            let pass = world.chunks[index].underwater?;
            Some((index, underwater::uniforms(pass, &frame.atmosphere)))
        })
        .collect();
    if let Some(renderer) = frame.renderer {
        renderer.draw_underwater(
            world,
            &passes,
            super::static_render::View {
                camera: frame.camera,
                atmosphere: frame.atmosphere,
                direction: frame.direction,
                time: frame.time,
                shadows: false,
            },
        );
        return;
    }
    // The streaming path: the same uniforms on the two world materials, then
    // the frame's own values back.
    let [solid, no_depth_write] = frame.materials;
    for (index, uniforms) in passes {
        let material = if uniforms.depth_write {
            solid
        } else {
            no_depth_write
        };
        let [ambient, diffuse0, diffuse1] = uniforms.lights;
        material.set_uniform("AmbientLight", Vec3::from_array(ambient));
        material.set_uniform("DiffuseLight0", Vec3::from_array(diffuse0));
        material.set_uniform("DiffuseLight1", Vec3::from_array(diffuse1));
        material.set_uniform("FogRange", Vec2::from_array(uniforms.fog_range));
        material.set_uniform("FogColor", Vec3::from_array(uniforms.fog_color));
        material.set_uniform("MaterialView", Vec4::from_array(uniforms.material_view));
        // The pass's own layer, not the chunk's: these chunks have no roof
        // runs or layer groups, so the mesh is drawn whole.
        material.set_uniform("DepthBias", Vec2::from_array(uniforms.depth_bias));
        material.set_uniform("ShadowsEnabled", 0.0f32);
        gl_use_material(material);
        macroquad::models::draw_mesh(&world.chunks[index].mesh);
    }
    for material in [solid, no_depth_write] {
        material.set_uniform("ShadowsEnabled", if frame.shadows { 1.0f32 } else { 0.0 });
        render::atmosphere(
            material,
            frame.atmosphere,
            frame.direction,
            frame.camera.position,
            frame.camera.target,
        );
    }
    // The materials go back to what their other users draw with.
    use rs09_client::depth_layers::offset;
    solid.set_uniform("DepthBias", Vec2::from_array(offset::NONE));
    no_depth_write.set_uniform("DepthBias", Vec2::from_array(offset::CHARACTER_SHADOW));
    gl_use_material(solid);
}
