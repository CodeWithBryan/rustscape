//! Roof removal applied to what the workbench draws and picks.
//!
//! `rs09_client::roof` decides which tiles are hidden each frame. Static
//! geometry is one mesh per 16 by 16 tiles per plane, so every chunk keeps,
//! for each run of triangles above the ground level, the floor tile or the
//! location it belongs to, and is drawn with an index list that leaves the
//! hidden runs out. The original does the same per frame: `GlTile.renderTiles`
//! writes the indices of the visible tiles' faces, and `SceneGraph.renderTile`
//! draws the walls, decorations and scenery of visible tiles only.
//!
//! Nothing here is stored with the chunk meshes. The owners are recovered from
//! data the workbench already keeps: the vertex range of every placed location
//! (`PickObject`), and the plane and floor marks packed into `normal.w`
//! (material + 1024 for a floor + 2048 per plane; `scene_lights` puts an
//! object's light key above those, in units of 8192).
//!
//! The same runs carry what else decides how a location's triangles are
//! drawn: whether the original's occluders hide the location this frame
//! (`scene_cull`), which depth layer it is drawn on
//! (`rs09_client::depth_layers`: wall decorations half a layer nearer, flat
//! ground decorations just in front of the floor), and whether its triangles
//! are translucent, which are left out of the chunk's opaque index list and
//! drawn after the opaque scene.
use crate::{Chunk, Workbench, scene_camera::SceneCamera, static_render::IndexBuffer};
use macroquad::prelude::{Material, Vec2, gl_use_material, vec2};
use rs09_client::{
    depth_layers::offset,
    interaction::LocationTarget,
    roof::{self, Mode},
};
use std::ops::Range;

/// The `DepthBias` a placed location is drawn with: `SceneGraph.renderTile`
/// draws wall decorations (shapes 4 to 8) half a layer nearer than the rest
/// of a tile and ground decoration of a type with opcode 90 just in front
/// of the floor.
pub(crate) fn layer(world: &Workbench, target: &LocationTarget) -> [f32; 2] {
    match target.shape {
        4..=8 => offset::WALL_DECOR,
        22 if world
            .location_types
            .get(&target.id)
            .is_some_and(|kind| kind.placement.flat) =>
        {
            offset::FLAT_DECOR
        }
        _ => offset::NONE,
    }
}

/// The `DepthBias` of a whole chunk, before its runs' own: blended floor
/// faces are drawn a hundred layers nearer than the floor.
pub(crate) fn chunk_bias(chunk: &Chunk) -> [f32; 2] {
    if chunk.floor && chunk.blend {
        offset::BLENDED_FLOOR
    } else {
        offset::NONE
    }
}

/// A location's vertices in a chunk.
struct Placed {
    vertices: Range<usize>,
    owner: Owner,
    /// Its index in `Workbench::pick_objects`.
    object: u32,
    bias: [f32; 2],
    hash: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum Owner {
    Floor {
        plane: usize,
        tile: [i32; 2],
    },
    Location {
        plane: usize,
        tile: [i32; 2],
        shape: u8,
        rotation: u8,
        size: [i32; 2],
    },
}
struct Run {
    /// Triangles, counted in the chunk's index list.
    triangles: Range<usize>,
    owner: Owner,
    hidden: bool,
    /// The location's index in `Workbench::pick_objects`; none for a floor.
    object: Option<u32>,
    /// Marked with a level above the ground: roof removal can hide it.
    roofed: bool,
    /// What the run adds to its chunk's `DepthBias`.
    bias: [f32; 2],
    /// Some vertex colour is not opaque.
    translucent: bool,
}
/// The opaque triangles of a chunk that are drawn this frame, when that is
/// not simply all of the chunk on one depth layer.
pub(crate) struct Shown {
    pub indices: Vec<u16>,
    pub gpu: Option<IndexBuffer>,
    /// Ranges of `indices` and the `DepthBias` each adds to its chunk's.
    pub groups: Vec<(Range<usize>, [f32; 2])>,
}
/// Translucent triangles of one location: a range of its chunk's own index
/// list, drawn after the opaque scene.
#[derive(Clone)]
pub(crate) struct Glass {
    pub chunk: usize,
    pub indices: Range<usize>,
    pub bias: [f32; 2],
    object: u32,
}
#[derive(Default)]
struct View {
    key: Option<[u64; 3]>,
    runs: Vec<Run>,
    shown: Option<Shown>,
    glass: Vec<Glass>,
    /// The runs are new: the lists have to be made whatever is hidden.
    fresh: bool,
}

#[derive(Default)]
pub(crate) struct State {
    roofs: roof::Roofs,
    scene: Option<roof::Scene>,
    origin: [i32; 2],
    /// What the scene and the views were built from.
    geometry: u64,
    /// The mode and plane `updateRoofRemovalMode` last ran for.
    updated: Option<(Mode, i32)>,
    mask: roof::Mask,
    views: Vec<View>,
    /// Counts changes of `mask`.
    revision: u64,
    /// By `pick_objects` index: left undrawn by the original's occluders.
    culled: Vec<bool>,
    /// The animated locations left undrawn the same way.
    culled_animated: Vec<(LocationTarget, u8)>,
    /// By `pick_objects` index: the place in the original's drawing order,
    /// `u32::MAX` for what its walk did not reach.
    rank: Vec<u32>,
    /// The `scene_cull::Verdict` last applied.
    verdict: u64,
    /// The `DepthBias` of each mesh `prepare_locations` returned.
    pub(crate) animated_layers: Vec<[f32; 2]>,
}
impl State {
    /// `ScriptRunner.roofGroupMaxHeight` and its companions: the bounds of
    /// the roof groups this frame removes, which switch off the occluders
    /// inside them.
    pub(crate) fn groups(&self) -> &roof::Groups {
        &self.roofs.groups
    }
    /// Changes whenever the set of tiles roof removal hides does.
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
    /// Whether the original's occluders leave an animated location undrawn
    /// this frame.
    pub(crate) fn culls(&self, target: &LocationTarget, piece: u8) -> bool {
        self.culled_animated
            .iter()
            .any(|(other, part)| other == target && *part == piece)
    }
    /// For a capture: how many runs of triangles are on each depth layer or
    /// translucent, with some of the locations concerned, and what the
    /// occluders leave undrawn.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn summary(&self, world: &Workbench) -> serde_json::Value {
        let runs = || self.views.iter().flat_map(|view| &view.runs);
        let sample = |pick: &dyn Fn(&Run) -> bool| {
            let mut seen = std::collections::BTreeSet::new();
            runs()
                .filter(|run| pick(run))
                .filter_map(|run| world.pick_objects.get(run.object? as usize))
                .filter(|object| seen.insert(object.target.id))
                .take(40)
                .map(|object| {
                    serde_json::json!({
                        "id": object.target.id,
                        "tile": object.target.tile,
                        "level": object.target.plane,
                        "shape": object.target.shape,
                    })
                })
                .collect::<Vec<_>>()
        };
        let count = |pick: &dyn Fn(&Run) -> bool| runs().filter(|run| pick(run)).count();
        let culled: Vec<_> = self
            .culled
            .iter()
            .enumerate()
            .filter(|(_, culled)| **culled)
            .filter_map(|(index, _)| world.pick_objects.get(index))
            .take(60)
            .map(|object| {
                serde_json::json!({
                    "id": object.target.id,
                    "tile": object.target.tile,
                    "level": object.target.plane,
                    "shape": object.target.shape,
                })
            })
            .collect();
        serde_json::json!({
            "wall_decoration_runs": count(&|run| run.bias == offset::WALL_DECOR),
            "flat_decoration_runs": count(&|run| run.bias == offset::FLAT_DECOR),
            "flat_decorations": sample(&|run| run.bias == offset::FLAT_DECOR),
            "translucent_runs": count(&|run| run.translucent),
            "translucent_locations": sample(&|run| run.translucent),
            "floor_runs_drawn_on_another_level": count(&|run| {
                run.object.is_none() && run.bias != offset::NONE
            }),
            "left_undrawn_by_occluders": self.culled(),
            "left_undrawn": culled,
        })
    }
    /// How many map locations the occluders leave undrawn this frame.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn culled(&self) -> usize {
        self.culled.iter().filter(|&&culled| culled).count() + self.culled_animated.len()
    }
    /// The translucent triangles to draw this frame, in the original's
    /// drawing order where its walk reached them and far to near before
    /// that for the rest. `position` is the tile the scene is drawn around.
    pub(crate) fn glass(&self, chunks: &[Chunk], position: Vec2) -> Vec<Glass> {
        let mut list: Vec<(u32, f32, &Glass)> = self
            .views
            .iter()
            .flat_map(|view| &view.glass)
            .filter_map(|glass| {
                let distance = chunks.get(glass.chunk)?.center.distance(position);
                let rank = self
                    .rank
                    .get(glass.object as usize)
                    .copied()
                    .unwrap_or(u32::MAX);
                (distance < 70.0).then_some((rank, distance, glass))
            })
            .collect();
        list.sort_by(|a, b| {
            let unreached = |rank: u32| rank == u32::MAX;
            unreached(b.0).cmp(&unreached(a.0)).then(if unreached(a.0) {
                b.1.total_cmp(&a.1)
            } else {
                a.0.cmp(&b.0)
            })
        });
        list.into_iter()
            .map(|(_, _, glass)| glass.clone())
            .collect()
    }
    /// Whether this frame hides a location.
    pub(crate) fn hides(&self, target: &LocationTarget) -> bool {
        self.scene
            .as_ref()
            .is_some_and(|scene| hidden(scene, &self.mask, owner(target, self.origin)))
    }
    /// Whether this frame leaves out a tile of the tile array (`level` is the
    /// array's, one lower than the plane over a bridge), given scene-local
    /// tile coordinates from `origin`: `SceneGraph.setShapedTile`'s test.
    pub(crate) fn hides_tile(&self, origin: [i32; 2], level: usize, x: i32, y: i32) -> bool {
        self.scene.is_some() && self.origin == origin && self.mask.hidden(level, x, y)
    }
    /// Whether this frame leaves out the floor of a world tile on `plane`.
    pub(crate) fn hides_floor(&self, plane: usize, tile: [i32; 2]) -> bool {
        self.scene.as_ref().is_some_and(|scene| {
            hidden(
                scene,
                &self.mask,
                Owner::Floor {
                    plane,
                    tile: [tile[0] - self.origin[0], tile[1] - self.origin[1]],
                },
            )
        })
    }
    /// The reduced index list of a chunk, or none when all of it is drawn.
    pub(crate) fn shown(&mut self, chunk: usize) -> Option<&mut Shown> {
        self.views.get_mut(chunk)?.shown.as_mut()
    }
    /// Whether every triangle of a chunk is hidden.
    pub(crate) fn hides_chunk(&self, chunk: usize) -> bool {
        self.views
            .get(chunk)
            .and_then(|view| view.shown.as_ref())
            .is_some_and(|shown| shown.indices.is_empty())
    }
}

fn owner(target: &LocationTarget, origin: [i32; 2]) -> Owner {
    Owner::Location {
        plane: target.plane,
        tile: [target.tile[0] - origin[0], target.tile[1] - origin[1]],
        shape: target.shape,
        rotation: target.rotation,
        size: target.size,
    }
}
fn hidden(scene: &roof::Scene, mask: &roof::Mask, owner: Owner) -> bool {
    match owner {
        Owner::Floor { plane, tile } => scene.floor_hidden(mask, plane, tile),
        Owner::Location {
            plane,
            tile,
            shape,
            rotation,
            size,
        } => scene.location_hidden(mask, plane, tile, shape, rotation, size),
    }
}
fn mix(hash: &mut u64, value: u64) {
    *hash = (*hash ^ value).wrapping_mul(0x100000001b3);
}
fn mix_target(hash: &mut u64, target: &LocationTarget) {
    mix(hash, u64::from(target.id));
    mix(hash, target.tile[0] as u64);
    mix(hash, target.tile[1] as u64);
    mix(
        hash,
        (target.plane as u64) << 16 | u64::from(target.shape) << 8 | u64::from(target.rotation),
    );
}
/// Changes whenever the regions, the origin, a chunk mesh or a placed location
/// does. Location changes do not all pass through one place, so this is
/// compared every frame instead.
fn fingerprint(world: &Workbench) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let origin = world.navigation.origin;
    mix(&mut hash, origin[0] as u64);
    mix(&mut hash, origin[1] as u64);
    mix(&mut hash, world.map_revision);
    mix(&mut hash, world.regions.as_ptr() as usize as u64);
    mix(&mut hash, world.regions.len() as u64);
    mix(&mut hash, world.world.locations.len() as u64);
    for chunk in &world.chunks {
        mix(&mut hash, chunk.mesh.vertices.as_ptr() as usize as u64);
        mix(&mut hash, chunk.mesh.vertices.len() as u64);
        mix(&mut hash, chunk.mesh.indices.len() as u64);
    }
    for object in &world.pick_objects {
        mix(&mut hash, u64::from(object.active));
        mix(&mut hash, object.chunk as u64);
        mix(&mut hash, object.vertices.start as u64);
        mix(&mut hash, object.vertices.end as u64);
        mix_target(&mut hash, &object.target);
    }
    for location in &world.animated_locations {
        mix_target(&mut hash, &location.target);
    }
    hash
}

/// The scene the fill reads, from the map tiles and every location in it.
fn scene(world: &Workbench) -> roof::Scene {
    let origin = world.navigation.origin;
    let tile = |plane: usize, x: usize, y: usize| {
        crate::tile(
            &world.regions,
            plane,
            origin[0] + x as i32,
            origin[1] + y as i32,
        )
    };
    let mut scene = roof::Scene::new(
        |p, x, y| tile(p, x, y).flags,
        |p, x, y| tile(p, x, y).height,
    );
    let undrawn = |id: u32| {
        world
            .location_types
            .get(&id)
            .is_some_and(|kind| kind.placement.hidden_in_hd)
    };
    // (region, id, plane, tile) is the order the map files list locations in;
    // it decides which wall a tile keeps and which scenery a full tile drops.
    let mut placed = std::collections::BTreeMap::new();
    let mut add = |target: &LocationTarget, entity: bool| {
        placed
            .entry((
                [target.tile[0] >> 6, target.tile[1] >> 6],
                target.id,
                target.plane,
                target.tile,
                target.shape,
                target.rotation,
            ))
            .or_insert((target.size, entity));
    };
    // Everything drawn has an entity; the original places none for opcode 82.
    for object in world.pick_objects.iter().filter(|object| object.active) {
        add(&object.target, !undrawn(object.target.id));
    }
    for location in &world.animated_locations {
        add(&location.target, !undrawn(location.target.id));
    }
    // Locations the workbench draws nothing for can still be in the
    // original's tile array, as a `Loc` without a model.
    for (slot, location) in &world.world.locations {
        let Ok(id) = u32::try_from(location.id) else {
            continue;
        };
        let (Some(definition), Some(kind)) = (
            world.definitions.get(&id.to_string()),
            world.location_types.get(&id),
        ) else {
            continue;
        };
        let entity = roof::has_entity(
            location.shape,
            definition.shapes.as_deref(),
            definition.models.len(),
            rs09_client::loc_placement::is_entity(definition, kind),
            kind.placement.hidden_in_hd,
        );
        add(
            &LocationTarget {
                id,
                tile: slot.tile,
                plane: slot.plane,
                shape: location.shape,
                rotation: location.rotation,
                size: [definition.width, definition.length],
                blocked_sides: 0,
            },
            entity,
        );
    }
    for ((_, _, plane, tile, shape, rotation), (size, entity)) in placed {
        scene.add(roof::Placement {
            plane,
            tile: [tile[0] - origin[0], tile[1] - origin[1]],
            shape,
            rotation,
            size,
            entity,
        });
    }
    scene.bridge();
    scene
}

/// The triangles of a chunk that are not simply drawn with the chunk: those
/// of placed locations, and the floors above the ground level.
fn runs(chunk: &Chunk, objects: &[Placed], origin: [i32; 2]) -> Vec<Run> {
    let mesh = &chunk.mesh;
    let mut runs: Vec<Run> = Vec::new();
    for (triangle, corners) in mesh.indices.chunks_exact(3).enumerate() {
        let Some(first) = mesh.vertices.get(usize::from(corners[0])) else {
            continue;
        };
        // Below the light key (`scene_lights::KEY_UNIT`).
        let packed = first.normal.w.rem_euclid(crate::scene_lights::KEY_UNIT);
        let plane = (packed / 2048.0).floor() as usize;
        let vertex = usize::from(corners[0]);
        let at = objects.partition_point(|placed| placed.vertices.start <= vertex);
        let placed = at
            .checked_sub(1)
            .map(|i| &objects[i])
            .filter(|placed| vertex < placed.vertices.end);
        let (owner, object, bias) = if let Some(placed) = placed {
            (placed.owner, Some(placed.object), placed.bias)
        } else if plane > 0 && packed % 2048.0 >= 1024.0 {
            // A floor triangle lies inside its tile.
            let mut centre = macroquad::math::Vec3::ZERO;
            for &corner in corners {
                let Some(v) = mesh.vertices.get(usize::from(corner)) else {
                    continue;
                };
                centre += v.position / 3.0;
            }
            let owner = Owner::Floor {
                plane,
                tile: [
                    (centre.x + 3200.0).floor() as i32 - origin[0],
                    (centre.z + 3200.0).floor() as i32 - origin[1],
                ],
            };
            // A floor marked with its own level in a chunk drawn with
            // another (a bridge deck among the ground) takes that level's
            // depth layer, as everything standing on it does.
            (owner, None, [offset::level(plane, chunk.plane), 0.0])
        } else {
            continue;
        };
        let translucent = object.is_some()
            && corners.iter().any(|&corner| {
                mesh.vertices
                    .get(usize::from(corner))
                    .is_some_and(|v| v.color[3] < 255)
            });
        match runs.last_mut() {
            Some(run)
                if run.owner == owner
                    && run.object == object
                    && run.translucent == translucent
                    && run.triangles.end == triangle =>
            {
                run.triangles.end += 1;
            }
            _ => runs.push(Run {
                triangles: triangle..triangle + 1,
                owner,
                hidden: false,
                object,
                // Level 0, or the ground linked under a bridge, is never
                // hidden by roof removal.
                roofed: plane > 0,
                bias,
                translucent,
            }),
        }
    }
    runs
}
fn views(state: &mut State, world: &Workbench) {
    let origin = world.navigation.origin;
    let mut objects: Vec<Vec<Placed>> = Vec::new();
    objects.resize_with(world.chunks.len(), Vec::new);
    for (index, object) in world.pick_objects.iter().enumerate() {
        if !object.active {
            continue;
        }
        if let Some(list) = objects.get_mut(object.chunk) {
            let bias = layer(world, &object.target);
            let mut hash = 0;
            mix_target(&mut hash, &object.target);
            mix(&mut hash, index as u64);
            mix(&mut hash, u64::from(bias[0].to_bits()));
            list.push(Placed {
                vertices: object.vertices.clone(),
                owner: owner(&object.target, origin),
                object: index as u32,
                bias,
                hash,
            });
        }
    }
    state.views.truncate(world.chunks.len());
    state.views.resize_with(world.chunks.len(), View::default);
    for ((chunk, view), mut objects) in world.chunks.iter().zip(&mut state.views).zip(objects) {
        objects.sort_by_key(|placed| placed.vertices.start);
        let mesh = &chunk.mesh;
        // A mesh is identified by where it is, what it holds and which
        // locations it carries; an edited or replaced chunk differs in one.
        let mut key = [
            mesh.vertices.as_ptr() as usize as u64,
            (mesh.vertices.len() as u64) << 32 | mesh.indices.len() as u64,
            0xcbf29ce484222325,
        ];
        for value in [origin[0], origin[1], chunk.plane as i32] {
            mix(&mut key[2], value as u64);
        }
        for value in [chunk.center.x, chunk.center.y] {
            mix(&mut key[2], u64::from(value.to_bits()));
        }
        for vertex in mesh
            .vertices
            .first()
            .into_iter()
            .chain(mesh.vertices.last())
        {
            for value in vertex.position.to_array() {
                mix(&mut key[2], u64::from(value.to_bits()));
            }
        }
        for placed in &objects {
            mix(&mut key[2], placed.vertices.start as u64);
            mix(&mut key[2], placed.vertices.end as u64);
            mix(&mut key[2], placed.hash);
        }
        if view.key == Some(key) {
            continue;
        }
        *view = View {
            key: Some(key),
            runs: runs(chunk, &objects, origin),
            shown: None,
            glass: Vec::new(),
            fresh: true,
        };
    }
}
/// Marks every run with this frame's roof mask and occlusion verdict, and
/// rewrites the lists of the chunks where a run changed.
fn apply(state: &mut State, world: &Workbench) {
    let Some(scene) = &state.scene else {
        return;
    };
    for (index, (view, chunk)) in state.views.iter_mut().zip(&world.chunks).enumerate() {
        let mut changed = std::mem::take(&mut view.fresh);
        for run in &mut view.runs {
            let now = (run.roofed && hidden(scene, &state.mask, run.owner))
                || run
                    .object
                    .is_some_and(|object| state.culled.get(object as usize) == Some(&true));
            changed |= now != run.hidden;
            run.hidden = now;
        }
        if !changed {
            continue;
        }
        let whole = view
            .runs
            .iter()
            .all(|run| !run.hidden && !run.translucent && run.bias == offset::NONE);
        view.shown = (!whole).then(|| shown(&chunk.mesh.indices, &view.runs));
        view.glass = view
            .runs
            .iter()
            .filter(|run| run.translucent && !run.hidden)
            .filter_map(|run| {
                Some(Glass {
                    chunk: index,
                    indices: run.triangles.start * 3..run.triangles.end * 3,
                    bias: run.bias,
                    object: run.object?,
                })
            })
            .collect();
    }
}
/// A chunk's opaque indices without its hidden runs, grouped by depth
/// layer; within a layer they keep their order, and what belongs to no run
/// comes with the runs that add nothing to the chunk's layer.
fn shown(source: &[u16], runs: &[Run]) -> Shown {
    let mut layers: Vec<([f32; 2], Vec<u16>)> = vec![(offset::NONE, Vec::new())];
    let mut next = 0;
    for run in runs {
        layers[0]
            .1
            .extend_from_slice(&source[next..run.triangles.start * 3]);
        next = run.triangles.end * 3;
        if run.hidden || run.translucent {
            continue;
        }
        let at = layers
            .iter()
            .position(|(bias, _)| *bias == run.bias)
            .unwrap_or_else(|| {
                layers.push((run.bias, Vec::new()));
                layers.len() - 1
            });
        layers[at]
            .1
            .extend_from_slice(&source[run.triangles.start * 3..next]);
    }
    layers[0].1.extend_from_slice(&source[next..]);
    let mut indices = Vec::with_capacity(source.len());
    let mut groups = Vec::new();
    for (bias, list) in layers {
        if list.is_empty() {
            continue;
        }
        groups.push((indices.len()..indices.len() + list.len(), bias));
        indices.extend(list);
    }
    Shown {
        indices,
        gpu: None,
        groups,
    }
}
impl Workbench {
    /// One frame of roof removal, before anything is drawn or picked. `player`
    /// is the local player's fine world position; `follow` is false while a
    /// cutscene or spline holds the camera (`Camera.cameraType != 1`).
    pub(crate) fn roof_frame(
        &mut self,
        mode: Mode,
        player: [i32; 2],
        camera: &SceneCamera,
        follow: bool,
    ) {
        let mut state = std::mem::take(&mut self.roofs);
        let origin = self.navigation.origin;
        let plane = self.ground.plane;
        let geometry = fingerprint(self);
        let rebuilt = state.scene.is_none() || state.geometry != geometry;
        if rebuilt {
            state.scene = Some(scene(self));
            state.origin = origin;
            state.geometry = geometry;
            views(&mut state, self);
        }
        let scene = state.scene.as_ref().unwrap();
        // The original runs updateRoofRemovalMode after a scene is built, when
        // the plane changes and when a preference does. Here a location
        // change also counts as a rebuilt scene.
        if rebuilt || state.updated != Some((mode, plane)) {
            state.roofs.update_mode(mode, scene, plane);
            state.updated = Some((mode, plane));
        }
        state.roofs.cycle = state.roofs.cycle.wrapping_add(1);
        let fine =
            |tile: f32, origin: i32| ((tile + 3200.0 - origin as f32) * 128.0).round() as i32;
        let forward = (camera.target - camera.position).normalize_or_zero();
        state.roofs.frame(
            mode,
            scene,
            plane,
            [player[0] - origin[0] * 128, player[1] - origin[1] * 128],
            &roof::Camera {
                follow,
                position: [
                    fine(camera.position.x, origin[0]),
                    fine(camera.position.z, origin[1]),
                    (-camera.position.y * 128.0).round() as i32,
                ],
                pitch: ((-forward.y).clamp(-1.0, 1.0).asin() * 2048.0 / std::f32::consts::TAU)
                    .round() as i32,
            },
        );
        let mask = state.roofs.mask(mode, plane);
        if rebuilt || mask != state.mask {
            state.mask = mask;
            state.revision = state.revision.wrapping_add(1);
            apply(&mut state, self);
        }
        self.roofs = state;
    }
    /// Takes this frame's occlusion verdict: what the original's occluders
    /// leave undrawn goes out of the chunks' lists, and its drawing order
    /// orders the translucent faces.
    pub(crate) fn occlude(&mut self, verdict: &crate::scene_cull::Verdict) {
        let mut state = std::mem::take(&mut self.roofs);
        if state.verdict != verdict.serial || state.rank.len() != self.pick_objects.len() {
            state.verdict = verdict.serial;
            let mut culled = vec![false; self.pick_objects.len()];
            for &index in &verdict.statics {
                if let Some(slot) = culled.get_mut(index) {
                    *slot = true;
                }
            }
            state.culled_animated = verdict
                .animated
                .iter()
                .filter_map(|&index| self.animated_locations.get(index))
                .map(|location| (location.target, location.piece))
                .collect();
            let mut rank = vec![u32::MAX; self.pick_objects.len()];
            for (place, &index) in verdict.order.iter().enumerate() {
                if let Some(slot) = rank.get_mut(index) {
                    *slot = place as u32;
                }
            }
            state.rank = rank;
            if culled != state.culled {
                state.culled = culled;
                apply(&mut state, self);
            }
        }
        self.roofs = state;
    }
    /// Draws a chunk through the streaming path: its opaque triangles that
    /// are not hidden, each depth layer with its own `DepthBias`.
    /// `materials` are the world material and the one without depth writes
    /// that blended floor faces need.
    pub(crate) fn draw_chunk(&mut self, index: usize, materials: [&Material; 2]) {
        let chunk = &mut self.chunks[index];
        let base = chunk_bias(chunk);
        let material = materials[usize::from(chunk.blend)];
        gl_use_material(material);
        let mesh = &mut chunk.mesh;
        match self.roofs.shown(index) {
            Some(shown) => {
                for (range, bias) in &shown.groups {
                    material.set_uniform("DepthBias", vec2(base[0] + bias[0], bias[1]));
                    let mut part = shown.indices[range.clone()].to_vec();
                    std::mem::swap(&mut mesh.indices, &mut part);
                    macroquad::models::draw_mesh(mesh);
                    std::mem::swap(&mut mesh.indices, &mut part);
                }
            }
            None => {
                material.set_uniform("DepthBias", Vec2::from(base));
                macroquad::models::draw_mesh(mesh);
            }
        }
        // The material goes back to what its other users draw with.
        material.set_uniform(
            "DepthBias",
            Vec2::from(if chunk.blend {
                offset::CHARACTER_SHADOW
            } else {
                offset::NONE
            }),
        );
    }
    /// Draws translucent triangles through the streaming path.
    pub(crate) fn draw_glass(&mut self, glass: &[Glass], material: &Material) {
        gl_use_material(material);
        for item in glass {
            let Some(chunk) = self.chunks.get_mut(item.chunk) else {
                continue;
            };
            let Some(mut part) = chunk
                .mesh
                .indices
                .get(item.indices.clone())
                .map(<[u16]>::to_vec)
            else {
                continue;
            };
            material.set_uniform("DepthBias", Vec2::from(item.bias));
            std::mem::swap(&mut chunk.mesh.indices, &mut part);
            macroquad::models::draw_mesh(&chunk.mesh);
            std::mem::swap(&mut chunk.mesh.indices, &mut part);
        }
        material.set_uniform("DepthBias", Vec2::from(offset::NONE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::*;

    /// One triangle inside world tile `tile`, packed as `render.rs` packs it.
    fn triangle(mesh: &mut Mesh, tile: [i32; 2], plane: usize, floor: bool, material: f32) {
        let start = mesh.vertices.len() as u16;
        for [dx, dz] in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]] {
            mesh.vertices.push(Vertex {
                position: vec3(
                    (tile[0] - 3200) as f32 + dx,
                    2.0 * plane as f32,
                    (tile[1] - 3200) as f32 + dz,
                ),
                uv: Vec2::ZERO,
                color: [255; 4],
                normal: vec4(
                    0.0,
                    1.0,
                    0.0,
                    material + if floor { 1024.0 } else { 0.0 } + plane as f32 * 2048.0,
                ),
            });
        }
        mesh.indices.extend([start, start + 1, start + 2]);
    }

    /// The owners are recovered from the vertex marks and the recorded vertex
    /// ranges; hidden runs leave the index list without disturbing the order
    /// of the rest; each depth layer is a group of its own; translucent
    /// triangles are kept apart.
    #[test]
    fn triangles_are_grouped_by_owner_and_layer_and_dropped_when_hidden() {
        let origin = [3168, 3168];
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        triangle(&mut mesh, [3210, 3215], 0, true, 7.0); // ground: never hidden
        triangle(&mut mesh, [3210, 3215], 1, true, 7.0);
        triangle(&mut mesh, [3210, 3215], 1, true, 1023.0);
        triangle(&mut mesh, [3211, 3215], 1, true, 7.0);
        triangle(&mut mesh, [3210, 3215], 0, false, 3.0); // a ground-level decoration
        triangle(&mut mesh, [3212, 3215], 2, false, 3.0); // a roof piece...
        triangle(&mut mesh, [3219, 3215], 2, false, 1023.0); // ...reaching far
        triangle(&mut mesh, [3210, 3215], 1, false, 3.0); // not a known location
        triangle(&mut mesh, [3213, 3215], 0, false, 3.0); // a window: frame...
        triangle(&mut mesh, [3213, 3215], 0, false, 3.0); // ...and pane
        for vertex in &mut mesh.vertices[27..30] {
            vertex.color[3] = 120;
        }
        let location = |plane, x, shape| Owner::Location {
            plane,
            tile: [x, 47],
            shape,
            rotation: 1,
            size: [1, 1],
        };
        let (decor, roof, window) = (location(0, 42, 4), location(2, 44, 12), location(0, 45, 0));
        let placed = |vertices, owner, object, bias| Placed {
            vertices,
            owner,
            object,
            bias,
            hash: 0,
        };
        let chunk = Chunk::new(mesh, vec2(3216.0, 3224.0), 1);
        let mut runs = runs(
            &chunk,
            &[
                placed(12..15, decor, 7, offset::WALL_DECOR),
                placed(15..21, roof, 8, offset::NONE),
                placed(24..30, window, 9, offset::NONE),
            ],
            origin,
        );
        let floor = |x| Owner::Floor {
            plane: 1,
            tile: [x, 47],
        };
        let found: Vec<_> = runs
            .iter()
            .map(|run| {
                (
                    run.triangles.clone(),
                    run.object,
                    run.roofed,
                    run.translucent,
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                (1..3, None, true, false),
                (3..4, None, true, false),
                (4..5, Some(7), false, false),
                (5..7, Some(8), true, false),
                (8..9, Some(9), false, false),
                (9..10, Some(9), false, true),
            ]
        );
        assert!(runs[0].owner == floor(42) && runs[1].owner == floor(43));
        assert!(runs[2].owner == decor && runs[3].owner == roof);

        // Nothing hidden: the decoration has its own layer and the pane is
        // left to the translucent pass.
        let all = chunk.mesh.indices.clone();
        let list = shown(&all, &runs);
        let opaque: Vec<u16> = (0..12).chain(15..27).collect();
        assert_eq!(list.indices, [opaque.clone(), vec![12, 13, 14]].concat());
        assert_eq!(
            list.groups,
            [(0..24, offset::NONE), (24..27, offset::WALL_DECOR)]
        );
        runs[0].hidden = true;
        runs[3].hidden = true;
        let list = shown(&all, &runs);
        assert_eq!(
            list.indices,
            [0, 1, 2, 9, 10, 11, 21, 22, 23, 24, 25, 26, 12, 13, 14]
        );
        assert_eq!(
            list.groups,
            [(0..12, offset::NONE), (12..15, offset::WALL_DECOR)]
        );
        // The decoration hidden (by an occluder): its group goes.
        runs[2].hidden = true;
        let list = shown(&all, &runs);
        assert_eq!(list.groups, [(0..12, offset::NONE)]);
    }

    /// A floor marked with a level other than its chunk's is drawn on that
    /// chunk's layer.
    #[test]
    fn a_bridge_deck_among_the_ground_takes_the_ground_layer() {
        let mut mesh = Mesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture: None,
        };
        triangle(&mut mesh, [3210, 3215], 0, true, 7.0);
        triangle(&mut mesh, [3211, 3215], 1, true, 7.0);
        let chunk = Chunk::new(mesh, vec2(3216.0, 3224.0), 0);
        let runs = runs(&chunk, &[], [3168, 3168]);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].bias, [50.0, 0.0]);
        let list = shown(&chunk.mesh.indices, &runs);
        assert_eq!(list.groups, [(0..3, offset::NONE), (3..6, [50.0, 0.0])]);
        let marked = rs09_client::depth_layers::vertex_layer(true, 1, [50.0, 0.0]);
        assert_eq!(marked, rs09_client::depth_layers::floor(0, false));
    }
}
