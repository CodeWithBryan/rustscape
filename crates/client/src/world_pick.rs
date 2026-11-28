//! What is under the mouse in the world, by the original's rules.
//!
//! `rs09_client::scene_order` replays the order in which RT4 would draw the
//! scene and `rs09_client::picking` holds its hover and walk tests; this
//! module feeds them from the workbench. The scene is mirrored into the
//! original's tile structure once per map change, the frame's actors and
//! item piles are added, and every model the walk reaches is hover-tested.
//! The result is `MiniMenu.addEntries`' input: the walk tile, then every
//! picked key in drawing order.
//!
//! Locations come from what `SceneGraph.addLoc` left in the scene
//! (`loc_placement::Plan`): each entity is tested by itself at the scene
//! position it is drawn at, so a corner wall's two models are two tests.
//! Where this client's geometry still differs from the original's the tests
//! use this client's: an actor's body and spot animation are tested as one,
//! and baked meshes carry float positions that are rounded back to fine units.
//!
//! Roof removal: a tile is walked only when `roof_render` draws it this frame
//! (`ScriptRunner.roofVisibility` through `SceneGraph.setShapedTile`), so
//! nothing on a removed roof is offered and every level left standing is.
use crate::{NpcActor, PlayerPick, Workbench, scene_camera::SceneCamera};
use macroquad::prelude::*;
use rs09_client::{
    entity_menu,
    interaction::LocationTarget,
    occlusion::{Active, Occluder, Removed},
    picking::{self, Bounds, Rule, View},
    scene_order::{self, Draw, Frustum, Ground, Scene},
    world::Ray,
};

/// The menu action that marks where the viewport's entries belong.
pub(crate) const ENTRIES: i32 = i32::MIN;
const SIZE: i32 = scene_order::SIZE;
const EDGE: usize = SIZE as usize + 1;

#[derive(Clone, Copy)]
enum Part {
    Static(usize),
    Animated(usize),
    Npc(usize),
    Player(usize),
    Pile([i32; 2]),
}

/// One step of building the world's menu entries, in the original's order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Step {
    /// The walk (or spell-on-tile) entry and the world tile it resolves to.
    Walk(Option<[i32; 2]>),
    /// A valid location under the mouse.
    Location(LocationTarget),
    /// An NPC or player scene key, as `entity_menu::expand` takes them.
    Actor(u64),
    /// The item pile on a world tile.
    Pile([i32; 2]),
}

impl Step {
    pub(crate) fn walk(&self) -> Option<Option<[i32; 2]>> {
        match self {
            Self::Walk(tile) => Some(*tile),
            _ => None,
        }
    }
    pub(crate) fn location(&self) -> Option<LocationTarget> {
        match self {
            Self::Location(target) => Some(*target),
            _ => None,
        }
    }
    pub(crate) fn actor_keys(&self) -> Vec<u64> {
        match self {
            Self::Actor(key) => vec![*key],
            _ => Vec::new(),
        }
    }
    pub(crate) fn pile(&self) -> Option<[i32; 2]> {
        match self {
            Self::Pile(tile) => Some(*tile),
            _ => None,
        }
    }
}

/// What the frame drew.
pub(crate) struct Sources<'a> {
    pub world: &'a Workbench,
    pub players: &'a [PlayerPick],
    pub piles: &'a crate::ground_render::Renderer,
}

/// Where the 3D view is on screen and where the mouse is.
pub(crate) struct Pointer {
    /// The view rectangle in window pixels: left, top, width, height.
    pub rect: [f32; 4],
    /// The ray under the mouse, when it is inside the rectangle.
    pub mouse: Option<Ray>,
}

#[derive(Default)]
pub(crate) struct Picker {
    scene: Scene,
    fingerprint: u64,
    origin: [i32; 2],
    /// Parts placed from the map; actors and piles follow each frame.
    fixed: Vec<Part>,
    keys: Vec<i64>,
    bounds: Vec<Option<(Bounds, [i32; 3])>>,
    /// Tile corner heights, `[level][x][y]`.
    heights: Vec<i32>,
    /// Ground shape and rotation of each tile, 255 for none.
    shapes: Vec<[u8; 2]>,
    piles: Vec<[i32; 2]>,
    pile_level: usize,
    /// `SceneGraph.occluders`, made when a scene is built.
    occluders: Vec<Occluder>,
    occluder_scene: Option<(u64, [i32; 2])>,
    /// `getMinY()` of each part: the fixed ones, then this pick's actors.
    min_y: Vec<i32>,
    /// Leaves the occluders out (`--no-occlusion`).
    pub(crate) occlusion_off: bool,
    /// How many occluders the last pick or replay had active, and how long
    /// the last replay for drawing took: both for a capture's report.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) active: usize,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) replay_microseconds: u128,
    /// What the last replay for drawing was made for, and its result.
    verdict_key: Option<[u64; 2]>,
    verdict: crate::scene_cull::Verdict,
}

/// Whether an NPC of the frame's list goes into the pick scene: it needs a
/// model and the triangles of a drawn pose.
fn npc_enters_pick(npc: &NpcActor) -> bool {
    !npc.pick_triangles.is_empty() && npc.render_id.is_some()
}

/// `--pick-probe`'s row for each NPC of the frame's list, as the pick sees it.
#[cfg(not(target_arch = "wasm32"))]
fn npc_rows(world: &Workbench) -> Vec<serde_json::Value> {
    npc_rows_named(&world.npcs, |id| {
        world.npc_definitions.get(&id).map(|d| d.name.as_str())
    })
}

/// [`npc_rows`] with each NPC's name from `name`.
#[cfg(not(target_arch = "wasm32"))]
fn npc_rows_named<'a>(
    npcs: &[NpcActor],
    name: impl Fn(u32) -> Option<&'a str>,
) -> Vec<serde_json::Value> {
    npcs.iter()
        .map(|npc| {
            serde_json::json!({
                "index": npc.index,
                "id": npc.id,
                "name": name(npc.id),
                "render_id": npc.render_id,
                "pick_triangles": npc.pick_triangles.len(),
                "enters_pick": npc_enters_pick(npc),
            })
        })
        .collect()
}

fn fine(world: f32, origin: i32) -> i32 {
    ((world + 3200.0 - origin as f32) * 128.0).round() as i32
}

/// `tileHeights[level][x][y]` from the picker's copy.
fn height_at(heights: &[i32], level: usize, x: i32, y: i32) -> i32 {
    let (x, y) = (x.clamp(0, SIZE) as usize, y.clamp(0, SIZE) as usize);
    heights
        .get(level.min(3) * EDGE * EDGE + x * EDGE + y)
        .copied()
        .unwrap_or(0)
}

/// `SceneGraph.buildActiveOccluders` for a view: the occluders that face the
/// camera, have a tile in the visibility map and are outside the roof groups
/// roof removal hides this frame.
fn active(occluders: &[Occluder], world: &Workbench, view: &View, frustum: &Frustum) -> Active {
    let tile = [view.eye[0], view.eye[2]].map(|n| n.clamp(0, SIZE * 128 - 1) / 128);
    let radius = picking::TILE_DISTANCE;
    Active::new(
        occluders,
        Removed::from(world.roofs.groups()),
        view.eye,
        radius,
        |column, row| frustum.contains(column - radius + tile[0], row - radius + tile[1]),
    )
}

impl Picker {
    fn height(&self, level: usize, x: i32, y: i32) -> i32 {
        let (x, y) = (x.clamp(0, SIZE) as usize, y.clamp(0, SIZE) as usize);
        self.heights
            .get(level.min(3) * EDGE * EDGE + x * EDGE + y)
            .copied()
            .unwrap_or(0)
    }

    /// A world position in the scene's fine units: `[x, height, y]`.
    fn local(&self, p: Vec3) -> [i32; 3] {
        [
            fine(p.x, self.origin[0]),
            (-p.y * 128.0).round() as i32,
            fine(p.z, self.origin[1]),
        ]
    }

    /// Mirror the map into the original's tile structure when it changed.
    fn rebuild(&mut self, world: &Workbench) {
        let origin = world.navigation.origin;
        let mut hash = 0xcbf29ce484222325_u64;
        let mut mix = |word: u64| hash = (hash ^ word).wrapping_mul(0x100000001b3);
        mix(origin[0] as u64);
        mix(origin[1] as u64);
        mix(world.regions.len() as u64);
        mix(world.map_revision);
        for object in &world.pick_objects {
            let t = object.target;
            mix(u64::from(object.active) | u64::from(t.id) << 1 | u64::from(object.piece) << 40);
            mix((t.tile[0] as u64) << 32 | t.tile[1] as u32 as u64);
            mix((t.plane as u64) << 16 | u64::from(t.shape) << 8 | u64::from(t.rotation));
        }
        for loc in &world.animated_locations {
            let t = loc.target;
            mix(u64::from(t.id) << 1 | 1 | u64::from(loc.piece) << 40);
            mix((t.tile[0] as u64) << 32 | t.tile[1] as u32 as u64);
            mix((t.plane as u64) << 16 | u64::from(t.shape) << 8 | u64::from(t.rotation));
        }
        if hash == self.fingerprint && !self.heights.is_empty() {
            return;
        }
        self.fingerprint = hash;
        self.origin = origin;
        self.scene = Scene::new();
        self.scene.fill_level(0);
        self.fixed.clear();
        self.keys.clear();
        self.min_y.clear();
        self.piles.clear();
        // The original makes its occluders while it loads a scene and never
        // again: later location changes leave them as they are.
        let built = (world.scene_revision, origin);
        if self.occluder_scene != Some(built) {
            self.occluder_scene = Some(built);
            self.occluders = crate::scene_cull::occluders(world);
        }
        self.heights = vec![0; 4 * EDGE * EDGE];
        self.shapes = vec![[255, 0]; 4 * (SIZE * SIZE) as usize];
        let mut bridges = Vec::new();
        for level in 0..4 {
            for x in 0..=SIZE {
                for y in 0..=SIZE {
                    let tile = crate::tile(&world.regions, level, origin[0] + x, origin[1] + y);
                    self.heights[level * EDGE * EDGE + x as usize * EDGE + y as usize] =
                        tile.height;
                    if x == SIZE || y == SIZE {
                        continue;
                    }
                    if level == 1 && tile.flags & 2 != 0 {
                        bridges.push([x, y]);
                    }
                    if tile.underlay == 0 && tile.overlay == 0 {
                        continue;
                    }
                    let shape = if tile.overlay == 0 { 0 } else { tile.shape + 1 };
                    self.shapes[level * (SIZE * SIZE) as usize + (x * SIZE + y) as usize] =
                        [shape, tile.rotation];
                    let ground = if shape <= 1 {
                        Ground::Plain { coloured: true }
                    } else {
                        Ground::Shaped
                    };
                    self.scene.set_ground(level, x, y, ground);
                }
            }
        }
        // The map files list locations by id, then level and position.
        // Each entry is one entity; the two of a corner wall (shape 2) or a
        // double diagonal decoration (shape 8) follow each other.
        let mut placed: Vec<(Part, LocationTarget, u8)> = world
            .pick_objects
            .iter()
            .enumerate()
            .filter(|(_, object)| object.active)
            .map(|(index, object)| (Part::Static(index), object.target, object.piece))
            .chain(
                world
                    .animated_locations
                    .iter()
                    .enumerate()
                    .map(|(index, loc)| (Part::Animated(index), loc.target, loc.piece)),
            )
            .collect();
        placed.sort_by_key(|(_, t, piece)| {
            (
                t.tile[0] >> 6,
                t.tile[1] >> 6,
                t.id,
                t.plane,
                t.tile[0] & 63,
                t.tile[1] & 63,
                t.shape,
                t.rotation,
                *piece,
            )
        });
        let mut placed = placed.into_iter().peekable();
        while let Some((kind, target, _)) = placed.next() {
            let [x, y] = [target.tile[0] - origin[0], target.tile[1] - origin[1]];
            // SceneGraph.readLocs keeps the scene's outermost ring empty.
            if x <= 0 || y <= 0 || x >= SIZE - 1 || y >= SIZE - 1 || target.plane > 3 {
                continue;
            }
            let Some(definition) = world.location_types.get(&target.id) else {
                continue;
            };
            let part = self.fixed.len() as u32;
            // The second entity of the same placement, when there is one.
            let second = placed
                .next_if(|(_, next, piece)| {
                    *piece == 1 && *next == target && matches!(target.shape, 2 | 8)
                })
                .map(|(kind, ..)| kind);
            let other = Some(part + u32::from(second.is_some()));
            let (level, shape, rotation) = (
                target.plane,
                i32::from(target.shape),
                usize::from(target.rotation & 3),
            );
            let key = picking::location_key(
                [x, y],
                shape,
                rotation as i32,
                target.id as i32,
                definition.interactable != 0,
                definition.support_items == 1,
                definition.has_animation,
            );
            // `getMinY()`: a baked model's top; a `Loc` (animated, a
            // multiloc or opcode 98) always answers -32768.
            let entity = world
                .definitions
                .get(&target.id.to_string())
                .is_some_and(|d| rs09_client::loc_placement::is_entity(d, definition));
            let top = |kind: Part| match kind {
                Part::Static(index) if !entity => {
                    world.pick_objects.get(index).map_or(-32768, |o| o.min_y)
                }
                _ => -32768,
            };
            let side = scene_order::WALL_SIDES[rotation];
            let corner = scene_order::WALL_CORNERS[rotation];
            let scene = &mut self.scene;
            // SceneGraph.addLoc, by shape.
            match shape {
                22 => scene.set_ground_decor(level, x, y, part, key & 0x8000_0000 != 0),
                10 | 11 => {
                    let mut size = target.size;
                    if rotation & 1 != 0 {
                        size.swap(0, 1);
                    }
                    let centre = [x * 128 + size[0] * 64, y * 128 + size[1] * 64];
                    scene.add_scenery(level, [x, y], size, centre, part, true, false);
                }
                9 | 12.. => {
                    let centre = [x * 128 + 64, y * 128 + 64];
                    scene.add_scenery(level, [x, y], [1, 1], centre, part, true, false);
                }
                0 => scene.set_wall(level, x, y, part, None, side, 0),
                1 | 3 => scene.set_wall(level, x, y, part, None, corner, 0),
                2 => scene.set_wall(
                    level,
                    x,
                    y,
                    part,
                    other,
                    side,
                    scene_order::WALL_SIDES[(rotation + 1) & 3],
                ),
                4 | 5 => scene.set_wall_decor(level, x, y, part, None, side, 0),
                6 => scene.set_wall_decor(level, x, y, part, None, 256, rotation as i32),
                7 => {
                    let opposite = ((rotation + 2) & 3) as i32;
                    scene.set_wall_decor(level, x, y, part, None, 256, opposite);
                }
                8 => scene.set_wall_decor(level, x, y, part, other, 256, rotation as i32),
                _ => continue,
            }
            for kind in std::iter::once(kind).chain(second) {
                self.fixed.push(kind);
                self.keys.push(key);
                self.min_y.push(top(kind));
            }
        }
        for [x, y] in bridges {
            self.scene.bridge(x, y);
        }
        self.bounds = vec![None; self.fixed.len()];
    }

    /// The camera as the hover tests see it.
    fn view(&self, camera: &SceneCamera, pointer: &Pointer) -> View {
        let forward = (camera.target - camera.position).normalize_or_zero();
        let units = 2048.0 / std::f32::consts::TAU;
        let pitch = ((-forward.y).clamp(-1.0, 1.0).asin() * units).round() as i32;
        let yaw = ((-forward.x).atan2(forward.z) * units).round() as i32;
        let mut view = View::new(self.local(camera.position), pitch, yaw);
        let direction = |ray: Ray| {
            view.direction([
                f64::from(ray.direction[0]),
                f64::from(-ray.direction[1]),
                f64::from(ray.direction[2]),
            ])
        };
        let [left, top, width, height] = pointer.rect;
        let corners = (
            direction(camera.ray(vec2(left, top))),
            direction(camera.ray(vec2(left + width, top + height))),
        );
        let pick = pointer.mouse.and_then(direction);
        if let (Some(lower), Some(upper)) = corners {
            view.lower = [lower[0].min(upper[0]), lower[1].min(upper[1])];
            view.upper = [lower[0].max(upper[0]), lower[1].max(upper[1])];
        }
        view.pick = pick;
        view
    }

    /// The bounds and scene position of a map location's mesh.
    fn frame(&self, world: &Workbench, index: usize) -> Option<(Bounds, [i32; 3])> {
        let object = world.pick_objects.get(index)?;
        // Where SceneGraph.addLoc put the entity: the tile centre for walls
        // and decorations (with the decoration's offset), the footprint's
        // centre for scenery, at the average corner height.
        let position = self.local(vec3(
            object.draw.x - 3200.0,
            object.ground,
            object.draw.y - 3200.0,
        ));
        let vertices = self.mesh(world, index, position)?;
        Some((Bounds::of(&vertices), position))
    }

    fn mesh(&self, world: &Workbench, index: usize, position: [i32; 3]) -> Option<Vec<[i32; 3]>> {
        let object = world.pick_objects.get(index)?;
        let vertices = world
            .chunks
            .get(object.chunk)?
            .mesh
            .vertices
            .get(object.vertices.clone())?;
        Some(self.relative(vertices.iter().map(|v| v.position), position, 0))
    }

    /// World positions as model vertices around `position`, turned back by
    /// the model's `yaw` so the original's own turn restores them.
    fn relative(
        &self,
        points: impl Iterator<Item = Vec3>,
        position: [i32; 3],
        yaw: i32,
    ) -> Vec<[i32; 3]> {
        let angle = yaw as f32 * std::f32::consts::TAU / 2048.0;
        let (sin, cos) = angle.sin_cos();
        points
            .map(|p| {
                let x = (p.x + 3200.0 - self.origin[0] as f32) * 128.0 - position[0] as f32;
                let z = (p.z + 3200.0 - self.origin[1] as f32) * 128.0 - position[2] as f32;
                let y = (-p.y * 128.0).round() as i32 - position[1];
                if yaw == 0 {
                    [x.round() as i32, y, z.round() as i32]
                } else {
                    [
                        (x * cos - z * sin).round() as i32,
                        y,
                        (x * sin + z * cos).round() as i32,
                    ]
                }
            })
            .collect()
    }

    fn actor(
        &self,
        world: &Workbench,
        view: &View,
        triangles: &[[Vec3; 3]],
        (fine, yaw): ([i32; 2], i32),
        rule: Rule,
        plane: usize,
    ) -> bool {
        let height =
            rs09_client::terrain::scene_height(&world.regions, self.origin, plane as i32, fine);
        let position = [
            fine[0] - self.origin[0] * 128,
            height,
            fine[1] - self.origin[1] * 128,
        ];
        let vertices = self.relative(triangles.iter().flatten().copied(), position, yaw & 2047);
        picking::hover(
            view,
            &Bounds::of(&vertices),
            &vertices,
            None,
            yaw & 2047,
            position,
            rule,
        )
    }

    fn hovered(
        &mut self,
        &Sources {
            world,
            players,
            piles,
        }: &Sources,
        view: &View,
        part: usize,
        kind: Part,
        plane: usize,
    ) -> bool {
        match kind {
            Part::Static(index) => {
                if self.bounds[part].is_none() {
                    self.bounds[part] = self.frame(world, index);
                }
                let Some((bounds, position)) = self.bounds[part] else {
                    return false;
                };
                picking::near(view, &bounds, position, false)
                    && self.mesh(world, index, position).is_some_and(|vertices| {
                        picking::hover(view, &bounds, &vertices, None, 0, position, Rule::Triangles)
                    })
            }
            Part::Animated(index) => {
                let Some(loc) = world.animated_locations.get(index) else {
                    return false;
                };
                let Some(model) = &loc.last_pose else {
                    return false;
                };
                // The pose is turned and aligned to the terrain already, and
                // stands where the scene draws the entity.
                let position = self.local(vec3(
                    loc.site.draw.x - 3200.0,
                    loc.site.ground,
                    loc.site.draw.y - 3200.0,
                ));
                picking::hover(
                    view,
                    &Bounds::of(&model.vertices),
                    &model.vertices,
                    Some(&model.faces),
                    0,
                    position,
                    Rule::Triangles,
                )
            }
            Part::Npc(index) => {
                let npc = &world.npcs[index];
                let size = world.npc_definitions.get(&npc.id).map_or(1, |d| d.size);
                let rule = if size == 1 {
                    Rule::Box
                } else {
                    Rule::Triangles
                };
                let stance = (npc.motion.fine, npc.motion.angle);
                self.actor(world, view, &npc.pick_triangles, stance, rule, plane)
            }
            Part::Player(index) => {
                let player = &players[index];
                self.actor(
                    world,
                    view,
                    &player.triangles,
                    (player.fine, player.angle),
                    Rule::Box,
                    plane,
                )
            }
            Part::Pile(tile) => {
                let world_tile = [tile[0] + self.origin[0], tile[1] + self.origin[1]];
                let Some(vertices) = piles.pile(plane, world_tile) else {
                    return false;
                };
                let position = [
                    tile[0] * 128 + 64,
                    self.height(plane, tile[0], tile[1]),
                    tile[1] * 128 + 64,
                ];
                let vertices = self.relative(vertices.iter().map(|v| v.position), position, 0);
                picking::hover(
                    view,
                    &Bounds::of(&vertices),
                    &vertices,
                    None,
                    0,
                    position,
                    Rule::Box,
                )
            }
        }
    }

    /// `SceneGraph.drawPlainTile` / `drawShapedTile`'s walk test for a tile.
    fn on_tile(&self, view: &View, point: [i32; 2], level: usize, [x, y]: [i32; 2]) -> bool {
        let Some(&[shape, rotation]) = self
            .shapes
            .get(level * (SIZE * SIZE) as usize + (x * SIZE + y) as usize)
        else {
            return false;
        };
        let heights = [
            self.height(level, x, y),
            self.height(level, x + 1, y),
            self.height(level, x + 1, y + 1),
            self.height(level, x, y + 1),
        ];
        match shape {
            255 => false,
            0 | 1 => picking::plain_tile(view, point, [x, y], heights),
            _ => rs09_client::tile_mesh::build(usize::from(shape), usize::from(rotation), heights)
                .is_some_and(|mesh| {
                    let faces = mesh.faces.iter().map(|&(_, face)| face);
                    picking::shaped_tile(view, point, [x, y], &mesh.vertices, faces)
                }),
        }
    }

    /// Takes the last pick's actors and item piles out of the scene.
    fn clear_dynamic(&mut self) {
        self.scene.clear_temporary();
        for [x, y] in std::mem::take(&mut self.piles) {
            self.scene.remove_obj_stack(self.pile_level, x, y);
        }
        self.min_y.truncate(self.fixed.len());
    }

    /// An actor's `getMinY()`: the top of the body it was last drawn with,
    /// above the ground it stands on (negative is up).
    fn actor_top(
        &self,
        world: &Workbench,
        triangles: &[[Vec3; 3]],
        fine: [i32; 2],
        plane: usize,
    ) -> i32 {
        let Some(top) = triangles
            .iter()
            .flatten()
            .map(|point| point.y)
            .reduce(f32::max)
        else {
            return -32768;
        };
        let ground =
            rs09_client::terrain::scene_height(&world.regions, self.origin, plane as i32, fine);
        (-top * 128.0).round() as i32 - ground
    }

    /// The walk replayed for drawing, without actors: which map locations
    /// the original's occluders leave undrawn for this camera, and the order
    /// in which it draws the others. `rect` is the scene's rectangle in the
    /// window. The result is kept until the camera, the map or the roofs
    /// change.
    pub(crate) fn cull(
        &mut self,
        world: &Workbench,
        camera: &SceneCamera,
        rect: [f32; 4],
    ) -> &crate::scene_cull::Verdict {
        self.rebuild(world);
        let origin = self.origin;
        let view = self.view(camera, &Pointer { rect, mouse: None });
        let mut key = [self.fingerprint, 0xcbf29ce484222325_u64];
        let mut mix = |word: i64| key[1] = (key[1] ^ word as u64).wrapping_mul(0x100000001b3);
        for word in view
            .eye
            .into_iter()
            .chain([view.sin_pitch, view.cos_pitch, view.sin_yaw, view.cos_yaw])
            .chain(view.lower)
            .chain(view.upper)
        {
            mix(i64::from(word));
        }
        mix(world.roofs.revision() as i64);
        mix(i64::from(self.occlusion_off));
        let groups = world.roofs.groups();
        for list in [
            &groups.max_height,
            &groups.min_x,
            &groups.max_x,
            &groups.min_z,
            &groups.max_z,
        ] {
            for &word in list {
                mix(i64::from(word));
            }
        }
        if self.verdict_key == Some(key) {
            return &self.verdict;
        }
        self.verdict_key = Some(key);
        #[cfg(not(target_arch = "wasm32"))]
        let started = std::time::Instant::now();
        self.clear_dynamic();
        let frustum = Frustum::new(
            &view,
            picking::TILE_DISTANCE,
            |x, y| self.height(0, x, y),
            |x, y| self.height(3, x, y),
        );
        let none = Vec::new();
        let occluders = if self.occlusion_off {
            &none
        } else {
            &self.occluders
        };
        let shades = active(occluders, world, &view, &frustum);
        let (heights, tops, fixed) = (&self.heights, &self.min_y, &self.fixed);
        let mut answers = crate::scene_cull::Answers::new(
            &shades,
            |level, x, y| height_at(heights, level, x, y),
            |part| tops.get(part as usize).copied().unwrap_or(-32768),
        );
        let mut order = Vec::new();
        self.scene.draw_occluded(
            [view.eye[0], view.eye[2]],
            picking::TILE_DISTANCE,
            |level, x, y| frustum.contains(x, y) && !world.roofs.hides_tile(origin, level, x, y),
            None,
            &mut answers,
            |draw| {
                if let Draw::Model(part) = draw
                    && let Some(Part::Static(index)) = fixed.get(part as usize)
                {
                    order.push(*index);
                }
            },
        );
        let (mut statics, mut animated) = (Vec::new(), Vec::new());
        for part in answers.culled {
            match fixed.get(part as usize) {
                Some(Part::Static(index)) => statics.push(*index),
                Some(Part::Animated(index)) => animated.push(*index),
                _ => {}
            }
        }
        self.active = shades.len();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.replay_microseconds = started.elapsed().as_micros();
        }
        self.verdict = crate::scene_cull::Verdict {
            statics,
            animated,
            order,
            serial: self.verdict.serial.wrapping_add(1),
        };
        &self.verdict
    }

    /// How many occluders the scene has.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn occluders(&self) -> usize {
        self.occluders.len()
    }

    /// Everything the original would put in `Model.pickResults` this frame,
    /// with its walk tile, as the steps of `MiniMenu.addEntries`.
    pub(crate) fn pick(
        &mut self,
        sources: &Sources,
        camera: &SceneCamera,
        pointer: &Pointer,
        plane: usize,
    ) -> Vec<Step> {
        let (world, players) = (sources.world, sources.players);
        self.rebuild(world);
        let origin = self.origin;
        let plane = plane.min(3);
        self.clear_dynamic();
        self.pile_level = plane;
        let mut parts = self.fixed.clone();
        for (tile, _) in world
            .ground
            .piles()
            .filter(|(tile, _)| tile.plane == plane as i32)
        {
            if !(0..SIZE).contains(&tile.x) || !(0..SIZE).contains(&tile.y) {
                continue;
            }
            let raised = world.ground_supports.get(&tile).copied().unwrap_or(0) != 0;
            let part = Some(parts.len() as u32);
            self.scene
                .set_obj_stack(plane, tile.x, tile.y, [part, None, None], raised);
            self.piles.push([tile.x, tile.y]);
            parts.push(Part::Pile([tile.x, tile.y]));
            // A pile is not asked about by itself.
            self.min_y.push(0);
        }
        // ScriptRunner.renderGameScene: the local player, the NPCs drawn
        // with top priority, the other players, then the other NPCs, all on
        // the local player's level and without the actors crowd thinning
        // left out. The order is the one the last drawn frame was planned
        // with (`Workbench::plan_actors`), which is also the frame the pick
        // meshes are from.
        for &(is_npc, actor) in &world.actor_scene.inserted {
            if is_npc {
                let Some((index, npc)) = world
                    .npcs
                    .iter()
                    .enumerate()
                    .find(|(_, npc)| npc.index == actor)
                else {
                    continue;
                };
                if !npc_enters_pick(npc) {
                    continue;
                }
                let size = world.npc_definitions.get(&npc.id).map_or(1, |d| d.size);
                let local = [
                    npc.motion.fine[0] - origin[0] * 128,
                    npc.motion.fine[1] - origin[1] * 128,
                ];
                let part = parts.len() as u32;
                if self.scene.add_actor(
                    plane,
                    local,
                    size * 64 + 60 - 64,
                    part,
                    npc.motion.angle & 2047,
                    false,
                ) {
                    parts.push(Part::Npc(index));
                    let top = self.actor_top(world, &npc.pick_triangles, npc.motion.fine, plane);
                    self.min_y.push(top);
                }
            } else {
                // The local player's pick mesh is in `players` as index 2047
                // when it was drawn.
                let Some((index, player)) = players
                    .iter()
                    .enumerate()
                    .find(|(_, player)| player.index == actor)
                else {
                    continue;
                };
                let local = [
                    player.fine[0] - origin[0] * 128,
                    player.fine[1] - origin[1] * 128,
                ];
                let radius = (player.size - 1) * 64 + 60;
                let part = parts.len() as u32;
                if self
                    .scene
                    .add_actor(plane, local, radius, part, player.angle & 2047, false)
                {
                    parts.push(Part::Player(index));
                    let top = self.actor_top(world, &player.triangles, player.fine, plane);
                    self.min_y.push(top);
                }
            }
        }
        let view = self.view(camera, pointer);
        let Some(point) = view.pick else {
            return Vec::new();
        };
        let frustum = Frustum::new(
            &view,
            picking::TILE_DISTANCE,
            |x, y| self.height(0, x, y),
            |x, y| self.height(3, x, y),
        );
        let mut draws = Vec::new();
        // What the original's occluders hide is not drawn, so not picked.
        let none = Vec::new();
        let occluders = if self.occlusion_off {
            &none
        } else {
            &self.occluders
        };
        let shades = active(occluders, world, &view, &frustum);
        self.active = shades.len();
        let (heights, tops) = (&self.heights, &self.min_y);
        let mut answers = crate::scene_cull::Answers::new(
            &shades,
            |level, x, y| height_at(heights, level, x, y),
            |part| tops.get(part as usize).copied().unwrap_or(-32768),
        );
        self.scene.draw_occluded(
            [view.eye[0], view.eye[2]],
            picking::TILE_DISTANCE,
            |level, x, y| frustum.contains(x, y) && !world.roofs.hides_tile(origin, level, x, y),
            Some(plane),
            &mut answers,
            |draw| draws.push(draw),
        );
        let mut walk = None;
        let mut tested = vec![false; parts.len()];
        let mut hits = Vec::new();
        for draw in draws {
            match draw {
                Draw::Ground { level, tile } => {
                    let tile = [i32::from(tile[0]), i32::from(tile[1])];
                    if self.on_tile(&view, point, usize::from(level), tile) {
                        walk = Some([tile[0] + origin[0], tile[1] + origin[1]]);
                    }
                }
                Draw::Model(part) => {
                    let part = part as usize;
                    if part >= parts.len() || std::mem::replace(&mut tested[part], true) {
                        continue;
                    }
                    let kind = parts[part];
                    let key = match kind {
                        Part::Static(_) | Part::Animated(_) => self.keys[part] as u64,
                        Part::Npc(index) => {
                            let npc = &world.npcs[index];
                            let interactive = world
                                .npc_definitions
                                .get(&npc.id)
                                .is_some_and(|d| d.interactive);
                            entity_menu::actor_key(entity_menu::Kind::Npc, npc.index, interactive)
                        }
                        Part::Player(index) => entity_menu::actor_key(
                            entity_menu::Kind::Player,
                            players[index].index,
                            true,
                        ),
                        Part::Pile([x, y]) => (x + (y << 7) + 0x6000_0000) as u64,
                    };
                    // `key > 0L`: nothing else enters the pick results.
                    if entity_menu::pickable(key) && self.hovered(sources, &view, part, kind, plane)
                    {
                        hits.push((key, kind));
                    }
                }
            }
        }
        let mut steps = vec![Step::Walk(walk)];
        let mut last = u64::MAX;
        for (key, kind) in hits {
            if key == last {
                continue;
            }
            last = key;
            match kind {
                Part::Static(_) | Part::Animated(_) => {
                    // SceneGraph.isLocValid: the key belongs to something on
                    // the local player's level of that tile.
                    let [x, y] = [(key & 0x7f) as i32, (key >> 7 & 0x7f) as i32];
                    let valid = self
                        .scene
                        .parts_at(plane, x, y)
                        .into_iter()
                        .any(|part| self.keys.get(part as usize) == Some(&(key as i64)));
                    let target = match kind {
                        Part::Static(index) => world.pick_objects.get(index).map(|o| o.target),
                        Part::Animated(index) => {
                            world.animated_locations.get(index).map(|l| l.target)
                        }
                        _ => None,
                    };
                    if let Some(target) = target.filter(|_| valid) {
                        steps.push(Step::Location(target));
                    }
                }
                Part::Npc(_) | Part::Player(_) => steps.push(Step::Actor(key)),
                Part::Pile([x, y]) => steps.push(Step::Pile([x + origin[0], y + origin[1]])),
            }
        }
        steps
    }
}

/// Of the points that hit an actor, the one nearest their mean: well inside
/// its picked shape, where a moving actor is still found a moment later.
#[cfg(not(target_arch = "wasm32"))]
fn middle(points: &[[i32; 2]]) -> Option<[i32; 2]> {
    let n = points.len() as i64;
    let sum = points.iter().fold([0i64; 2], |s, p| {
        [s[0] + i64::from(p[0]), s[1] + i64::from(p[1])]
    });
    let mean = [sum[0] / n.max(1), sum[1] / n.max(1)];
    points.iter().copied().min_by_key(|p| {
        let d = [i64::from(p[0]) - mean[0], i64::from(p[1]) - mean[1]];
        d[0] * d[0] + d[1] * d[1]
    })
}

/// `--pick-probe`: what a mouse at each point of a grid over the view would
/// pick, as if the local player stood on each level in turn. Written with a
/// capture so a run can show which locations are offered where.
///
/// `build_menu` calls it right after the frame's mouse pick with that pick's
/// sources, camera and rectangle, so it answers what a click would get. The
/// redraw that follows replaces the actors' pick meshes; a probe taken there
/// once offered live NPCs that no click could reach.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn probe(
    picker: &mut Picker,
    sources: &Sources,
    camera: &SceneCamera,
    rect: [f32; 4],
    roofs: bool,
) -> serde_json::Value {
    let world = sources.world;
    let origin = world.navigation.origin;
    // The frame's replay for drawing, timed (the picks below make their own).
    let replay = picker.replay_microseconds;
    let mut levels = Vec::new();
    for plane in 0..4 {
        let (mut points, mut walkable, mut deepest) = (0, 0, 0);
        let mut found = std::collections::BTreeMap::new();
        let mut actors = std::collections::BTreeMap::<u64, [i32; 4]>::new();
        // The grid points that hit each actor: points a mouse can be put
        // on to point at it (the area's corners may miss it).
        let mut hits = std::collections::BTreeMap::<u64, Vec<[i32; 2]>>::new();
        let mut samples = Vec::new();
        let mut y = rect[1] + 4.0;
        while y < rect[1] + rect[3] {
            let mut x = rect[0] + 4.0;
            while x < rect[0] + rect[2] {
                let pointer = Pointer {
                    rect,
                    mouse: Some(camera.ray(vec2(x, y))),
                };
                let steps = picker.pick(sources, camera, &pointer, plane);
                points += 1;
                walkable += usize::from(steps.iter().any(|s| s.walk().flatten().is_some()));
                for key in steps.iter().flat_map(Step::actor_keys) {
                    hits.entry(key).or_default().push([x as i32, y as i32]);
                    let area = actors.entry(key).or_insert([i32::MAX, i32::MAX, 0, 0]);
                    *area = [
                        area[0].min(x as i32),
                        area[1].min(y as i32),
                        area[2].max(x as i32),
                        area[3].max(y as i32),
                    ];
                }
                let mut menu = rs09_client::interaction::Menu::default();
                let mut names = Vec::new();
                for target in steps.iter().filter_map(Step::location) {
                    let Some((_, definition)) = world.menu_location(&target) else {
                        continue;
                    };
                    menu.location(definition, target, origin);
                    names.push(definition.name.clone());
                    found
                        .entry((target.plane, target.tile, target.id, target.shape))
                        .or_insert_with(|| (definition.name.clone(), [x as i32, y as i32]));
                }
                if names.len() > deepest
                    || (names.len() >= 2 && samples.len() < 12 && names[0] != names[1])
                {
                    deepest = deepest.max(names.len());
                    menu.sort();
                    samples.push(serde_json::json!({
                        "pixel": [x as i32, y as i32],
                        "walk": steps.iter().find_map(|s| s.walk()).flatten(),
                        "locations": names,
                        "menu": menu.entries.iter().rev().map(crate::client_ui::label).collect::<Vec<_>>(),
                    }));
                }
                x += 8.0;
            }
            y += 8.0;
        }
        levels.push(serde_json::json!({
            "player_level": plane,
            "points": points,
            "points_with_walk_tile": walkable,
            "most_locations_at_a_point": deepest,
            "locations": found.iter().map(|((level, tile, id, shape), (name, pixel))| serde_json::json!({
                "level": level, "tile": tile, "id": id, "shape": shape, "name": name, "first_pixel": pixel,
            })).collect::<Vec<_>>(),
            "actors": actors.iter().map(|(key, area)| serde_json::json!({
                "npc": key >> 29 & 3 == 1, "index": key >> 32, "pixels": area,
                "first_pixel": hits.get(key).and_then(|h| h.first()),
                "middle_pixel": hits.get(key).and_then(|h| middle(h)),
            })).collect::<Vec<_>>(),
            "samples": samples,
        }));
    }
    serde_json::json!({
        "rect": rect,
        "roofs": roofs,
        "occlusion": {
            "off": picker.occlusion_off,
            "occluders_in_scene": picker.occluders(),
            "active_for_this_view": picker.active,
            "replay_for_drawing_microseconds": replay,
        },
        "drawing": world.roofs.summary(world),
        "npcs": npc_rows(world),
        "scope": "Taken in the frame's menu build right after its mouse pick, from the same inputs. World picks only; npcs is the frame's NPC list as that pick sees it. The menu lists are the location entries MiniMenu.addEntries would add for the picked keys, top entry first, without walk, actor or interface entries. Roof removal is the frame's own (the real player level's), whatever level is probed.",
        "levels": levels,
    })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// An NPC at the frame's mouse pick as the live rebuild before
    /// CodeWithBryan/rustscape#11 left it: `update_npc_views` has given it a
    /// model, but its pick triangles are empty until the draw.
    fn rebuilt(index: usize) -> NpcActor {
        NpcActor {
            index,
            id: 3,
            render_id: Some(3),
            motion: rs09_client::walking::Movement::new([3222, 3218]),
            animation: None,
            home: [3222, 3218],
            walks: false,
            phase: 0,
            pick_triangles: Vec::new(),
            model_height: 200,
            terrain: Default::default(),
        }
    }

    /// What `npc_meshes` does for that NPC later in the frame.
    fn drawn(npc: &mut NpcActor) {
        npc.pick_triangles.push([Vec3::ZERO, Vec3::X, Vec3::Z]);
    }

    /// What the probe's NPC rows must say at the pick. This does not catch
    /// the probe moving back after the draw: when in the frame it runs rests
    /// on the call in `build_menu`, which needs a loaded `Workbench` to test.
    #[test]
    fn npc_rows_at_pick_time_leave_an_undrawn_npc_out() {
        let mut npcs = vec![rebuilt(7)];
        // `build_menu`: the frame's mouse pick, then the probe from its inputs.
        let at_pick = npc_rows_named(&npcs, |_| None);
        // The redraw refills the pick mesh before the capture is written.
        drawn(&mut npcs[0]);
        let at_redraw = npc_rows_named(&npcs, |_| None);
        assert_eq!(at_pick[0]["index"], 7);
        assert_eq!(at_pick[0]["enters_pick"], false);
        assert_eq!(at_pick[0]["pick_triangles"], 0);
        // A probe taken at the redraw (where it used to be) would have
        // called the NPC pickable although no click in that frame could be.
        assert_eq!(at_redraw[0]["enters_pick"], true);
    }

    #[test]
    fn a_drawn_npc_enters_the_pick() {
        let mut npc = rebuilt(1);
        assert!(!npc_enters_pick(&npc), "a model without triangles");
        drawn(&mut npc);
        assert!(npc_enters_pick(&npc));
        npc.render_id = None;
        assert!(!npc_enters_pick(&npc), "triangles without a model");
    }
}
