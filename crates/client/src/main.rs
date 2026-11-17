mod actor_scene;
#[cfg(any(target_os = "windows", target_arch = "wasm32"))]
mod ambient_audio;
#[cfg(target_arch = "wasm32")]
mod cache_host;
mod cache_update_screen;
mod client_cycle;
mod client_ui;
mod cursor_view;
mod display_host;
mod game;
mod ground_render;
mod lifecycle_host;
mod minimap_labels;
mod minimap_render;
#[cfg(test)]
mod part_pose_tests;
mod render;
mod roof_render;
mod scene_camera;
mod scene_cull;
mod scene_detail;
mod scene_lights;
mod screen_frame;
mod static_render;
mod title_world;
mod ui_audio;
mod ui_model_render;
mod ui_render;
mod widget_sprite_render;
mod world_cache;
mod world_map_render;
mod world_pick;
// The canvas, which is smaller than the window in the fixed window modes:
// these stand in for macroquad's of the same names.
use display_host::{screen_height, screen_width};
use macroquad::{camera::Camera, prelude::*};
use rs09_client::{
    animation::{
        Base, Frame, Pose, Sequence, animate_dual_hd_with_state, animate_hd_with_state,
        animate_masked_hd,
    },
    location::{self, Definition},
    model::Model,
    pack::Pack,
    terrain::{Region, Tile},
    walking::{self, ClippingDefinition, Movement, Navigation, Placement, PlayerBas},
};
use scene_camera::SceneCamera;
use serde::Deserialize;
use std::collections::{BTreeMap, VecDeque};

#[derive(Deserialize)]
struct Scene {
    #[serde(default = "default_scene_name")]
    name: String,
    #[serde(default = "default_start")]
    start: [i32; 2],
    regions: Vec<[i32; 2]>,
    locations: Vec<[i32; 6]>,
    definitions: BTreeMap<String, Definition>,
    avatar_kits: Vec<u32>,
    npc_types: Vec<u32>,
    npc_spawns: Vec<[i32; 5]>,
}
fn default_scene_name() -> String {
    "Lumbridge".into()
}
fn default_start() -> [i32; 2] {
    [3222, 3218]
}
struct NpcBody {
    definition: rs09_client::npc::Definition,
    model: Model,
}
struct SpotBody {
    definition: rs09_client::spot::Definition,
    model: Model,
}
struct SpotActor {
    /// `ProjAnim.pitch`: the model is rotated about X by this angle.
    pitch: Option<i32>,
    npc: bool,
    index: usize,
    fine: [i32; 2],
    yaw: i32,
    terrain: [i32; 2],
    track: rs09_client::actor_animation::Track,
    height: i32,
    world: Option<(i32, i32)>,
}
struct NpcActor {
    index: usize,
    id: u32,
    render_id: Option<u32>,
    motion: Movement,
    animation: Option<rs09_client::actor_animation::State>,
    home: [i32; 2],
    walks: bool,
    phase: u64,
    pick_triangles: Vec<[Vec3; 3]>,
    model_height: i32,
    terrain: rs09_client::spot::Alignment,
}
struct PlayerPick {
    index: usize,
    /// Absolute fine position, facing angle and size, as the actor is drawn.
    fine: [i32; 2],
    angle: i32,
    size: i32,
    triangles: Vec<[Vec3; 3]>,
}
#[derive(Clone, Copy)]
struct ActorPose<'a> {
    actor: (bool, usize),
    sequence: u32,
    tick: u64,
    tween: bool,
    cursor: Option<(usize, usize, i32, i32)>,
    state: Option<&'a rs09_client::actor_animation::State>,
    idle: i32,
}
type AnimatedPose<'a> = (&'a Sequence, Pose<'a>);
fn actor_poses<'a>(
    sequences: &'a BTreeMap<u32, Sequence>,
    frames: &'a BTreeMap<u32, Frame>,
    bases: &'a BTreeMap<u16, Base>,
    animation: ActorPose<'_>,
) -> (Option<AnimatedPose<'a>>, Option<AnimatedPose<'a>>) {
    let ActorPose {
        sequence,
        tick,
        tween,
        cursor: movement_cursor,
        state,
        ..
    } = animation;
    let movement = sequences.get(&sequence).and_then(|seq| {
        let (a, b, delta, length) = movement_cursor.or_else(|| seq.preview_cursor(tick))?;
        let frame = frames.get(seq.frames.get(a)?)?;
        Some((
            seq,
            Pose {
                base: bases.get(&frame.base)?,
                frame,
                next: if tween || seq.tween {
                    seq.frames.get(b).and_then(|id| frames.get(id))
                } else {
                    None
                },
                delta,
                length,
            },
        ))
    });
    let action = state
        .filter(|s| s.action.id != -1 && s.action.delay == 0)
        .and_then(|s| {
            let seq = sequences.get(&(s.action.id as u32))?;
            let (a, b, delta, length) = s.action.cursor(seq)?;
            let frame = frames.get(seq.frames.get(a)?)?;
            Some((
                seq,
                Pose {
                    base: bases.get(&frame.base)?,
                    frame,
                    next: if tween || seq.tween {
                        b.and_then(|b| seq.frames.get(b))
                            .and_then(|id| frames.get(id))
                    } else {
                        None
                    },
                    delta,
                    length,
                },
            ))
        });
    (movement, action)
}
struct SlotPose<'a> {
    slot: usize,
    id: i32,
    frame: i32,
    delay: i32,
    animation: AnimatedPose<'a>,
}
fn animation_asset<'a>(
    primary: &'a Pack,
    complete: Option<&'a Pack>,
    world: &'a Pack,
    name: &str,
) -> Result<&'a [u8], String> {
    primary
        .get(name)
        .or_else(|_| {
            complete
                .ok_or_else(|| format!("No complete animation catalog for {name}"))?
                .get(name)
        })
        .or_else(|_| world.get(name))
}
fn actor_slots<'a>(
    sequences: &'a BTreeMap<u32, Sequence>,
    frames: &'a BTreeMap<u32, Frame>,
    bases: &'a BTreeMap<u16, Base>,
    animation: ActorPose<'_>,
) -> Vec<SlotPose<'a>> {
    animation
        .state
        .into_iter()
        .flat_map(|s| s.slots.iter().enumerate())
        .filter_map(|(slot, track)| {
            // Original body construction displays frame zero during a slot delay.
            let track = track.as_ref()?;
            let seq = sequences.get(&(track.id as u32))?;
            let (a, b, delta, length) = track.cursor(seq)?;
            let frame = frames.get(seq.frames.get(a)?)?;
            Some(SlotPose {
                slot,
                id: track.id,
                frame: track.frame,
                delay: track.delay,
                animation: (
                    seq,
                    Pose {
                        base: bases.get(&frame.base)?,
                        frame,
                        next: if animation.tween || seq.tween {
                            b.and_then(|b| seq.frames.get(b))
                                .and_then(|id| frames.get(id))
                        } else {
                            None
                        },
                        delta,
                        length,
                    },
                ),
            })
        })
        .collect()
}
fn record_slot_pose(
    views: &std::cell::RefCell<Vec<serde_json::Value>>,
    actor: (bool, usize),
    model: &Model,
    slots: &[SlotPose<'_>],
) {
    if slots.is_empty() || views.borrow().len() >= 64 {
        return;
    }
    let checksum = model
        .vertices
        .iter()
        .flatten()
        .fold(0xcbf29ce484222325u64, |hash, n| {
            n.to_be_bytes()
                .into_iter()
                .fold(hash, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3))
        });
    views.borrow_mut().push(serde_json::json!({"npc":actor.0,"index":actor.1,"slots":slots.iter().map(|s|serde_json::json!({"slot":s.slot,"sequence":s.id,"frame":s.frame,"delay":s.delay,"delta":s.animation.1.delta,"length":s.animation.1.length,"matrix":model.part_matrices.get(s.slot).is_some_and(Option::is_some)})).collect::<Vec<_>>(),"vertices":model.vertices.len(),"source_bits":model.vertex_sources.iter().copied().fold(0u16,|a,b|a|b),"vertex_checksum":format!("{checksum:016x}"),"scope":"Body pose before actor resize and terrain alignment; submitted to the shared native/WASM GPU path"}));
}
fn actor_model(
    model: &Model,
    movement: Option<AnimatedPose<'_>>,
    action: Option<AnimatedPose<'_>>,
    sequence: u32,
    idle: i32,
    slots: &[SlotPose<'_>],
    origin_state: &std::cell::Cell<bool>,
) -> Model {
    let mut valid = origin_state.get();
    let mut overlays = None;
    for slot in slots {
        let matrix = model.part_matrices.get(slot.slot).and_then(Option::as_ref);
        overlays = Some(animate_masked_hd(
            overlays.as_ref().unwrap_or(model),
            slot.animation.1,
            1u16 << slot.slot,
            matrix,
            slot.animation.0.transform_normals,
            &mut valid,
        ));
    }
    let body = overlays.as_ref().unwrap_or(model);
    let result = if let Some((seq, primary)) = action {
        let secondary = movement.filter(|_| sequence != idle as u32);
        animate_dual_hd_with_state(
            body,
            primary,
            secondary.map(|(_, p)| p),
            seq.frame_group.as_deref(),
            seq.transform_normals || secondary.is_some_and(|(s, _)| s.transform_normals),
            &mut valid,
        )
    } else if let Some((seq, p)) = movement {
        animate_hd_with_state(body, p, seq.transform_normals, &mut valid)
    } else {
        body.clone()
    };
    origin_state.set(valid);
    result
}
struct WorldSound {
    sequence: u32,
    frame: i32,
    fine: [i32; 2],
    cycle: i32,
}
/// The actors a MAP_PROJANIM (104) can name as its launcher.
struct Launchers<'a> {
    npcs: &'a rs09_client::npc_updates::State,
    bas: &'a BTreeMap<usize, PlayerBas>,
    actors: &'a BTreeMap<usize, Movement>,
    motion: &'a Movement,
    /// The local player's server index.
    own: Option<usize>,
}
#[derive(Deserialize)]
struct NpcReplay {
    start: [i32; 2],
    frames: VecDeque<NpcReplayFrame>,
    /// Explicit offline self requests use the same original request path as a
    /// decoded PLAYER_INFO field. They do not claim live gameplay or wire input.
    #[serde(default)]
    player_spots: VecDeque<SpotReplay>,
    /// Authored self requests for rendering review; separate from PLAYER_INFO wire.
    #[serde(default)]
    player_slots: VecDeque<SlotReplay>,
}
#[derive(Deserialize)]
struct SlotReplay {
    tick: u64,
    requests: Vec<(i32, i32, i32)>,
}
#[derive(Deserialize)]
struct SpotReplay {
    tick: u64,
    id: i32,
    packed: i32,
}
#[derive(Deserialize)]
struct NpcReplayFrame {
    tick: u64,
    opcode: u8,
    payload: Vec<u8>,
}
struct NpcInput {
    cycle: i32,
    self_tile: [i32; 2],
    origin: [i32; 2],
    animation_origin: i32,
}
struct NpcUpdate {
    diagnostics: Vec<String>,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    ambient: Vec<rs09_client::npc_updates::Event>,
}
fn npc_snapshot(state: &rs09_client::npc_updates::State) -> serde_json::Value {
    serde_json::json!(state.local_ids.iter().filter_map(|index|{
        let n=state.actors.get(index)?;
        Some(serde_json::json!({"index":index,"type":n.type_id,"head":n.motion.head,"fine":n.motion.fine,"angle":n.motion.angle,"size":n.metadata.size,"sequence":n.motion.sequence,"frame":n.motion.frame,"action":[n.animation.action.id,n.animation.action.frame,n.animation.action.delay]}))
    }).collect::<Vec<_>>())
}
fn menu_snapshot(menu: &rs09_client::interaction::Menu) -> serde_json::Value {
    serde_json::json!({"open":menu.open,"entries":menu.entries.iter().map(|e|
        serde_json::json!({"option":e.option,"target":e.target,"action":e.action,
            "key":e.key,"args":e.args,"cursor":e.cursor})
    ).collect::<Vec<_>>()})
}
/// Read the server-fed inventory cache without changing widget or item state.
/// A malformed/extended container cannot make the status snapshot unbounded.
fn inventory_snapshot(ui: &rs09_client::ui::state::State) -> serde_json::Value {
    const LIMIT: usize = 128;
    let items = ui.containers.get(&93);
    serde_json::json!({
        "container":93,"present":items.is_some(),
        "slot_count":items.map_or(0, Vec::len),
        "truncated":items.is_some_and(|items| items.len()>LIMIT),
        "slots":items.into_iter().flatten().take(LIMIT).enumerate().map(|(slot,item)|
            serde_json::json!({"slot":slot,"id":item.id,"amount":item.amount})
        ).collect::<Vec<_>>()
    })
}
struct AnimatedLocation {
    target: rs09_client::interaction::LocationTarget,
    last_pose: Option<Model>,
    pick_bounds: [Vec3; 2],
    model: Model,
    sequence: u32,
    clock: location::Animation,
    position: Vec2,
    plane: usize,
    /// What `LocType.getAnimatedEntity` does to each pose, and where the
    /// scene draws the result (see `LocPiece`).
    site: LocSite,
    /// Which of the placement's entities this is (a corner wall has two).
    piece: u8,
    rotation: i32,
    cast_shadow: bool,
    shadow: Option<rs09_client::shadow::Sprite>,
    shadow_state: rs09_client::shadow_rules::LocShadow,
}
/// A `Loc` the scene resolves through `LocType.getMultiLoc` every frame.
#[derive(Clone, Copy)]
struct MultiLoc {
    loc: rs09_client::world::Location,
    /// The sequence a LOC_ANIM gave the `Loc`, replacing the parent's own.
    animation: Option<i32>,
    /// The child last built, None before the first resolution. `Some(None)`
    /// is a null child: nothing is drawn.
    built: Option<Option<u32>>,
    /// Whether any geometry of this slot is in the scene.
    shown: bool,
    /// A LOC_ANIM replaced the `Loc`: the next build applies its clock rule.
    replaced: bool,
}
/// Which `Loc` an animated entity is, for its clock.
#[derive(Clone, Copy)]
enum LocClock {
    /// `SceneGraph.addLoc`: a new `Loc`, started as its type allows.
    New,
    /// LOC_ANIM: a `Loc` replacing the one in the slot.
    Replaced,
    /// The same `Loc`, drawing another child.
    Kept,
}
impl MultiLoc {
    fn new(loc: rs09_client::world::Location, animation: Option<i32>) -> Self {
        Self {
            loc,
            animation,
            built: None,
            shown: false,
            replaced: false,
        }
    }
}
/// Where one entity of a placement stands: the terrain alignment's inputs
/// and the scene slot's draw position.
#[derive(Clone, Copy)]
struct LocSite {
    finish: rs09_client::loc_placement::Finish,
    /// Bounds of the model `alignToTerrain` is handed as its reference.
    bounds: rs09_client::loc_placement::Bounds,
    /// `xFine`/`yFine` in absolute fine units.
    center: [i32; 2],
    /// The `averageY` the alignment is relative to.
    base: i32,
    /// Tile position the scene draws the entity at, decoration offset included.
    draw: Vec2,
    /// World height of the scene slot (`zFine`).
    ground: f32,
    /// `diffAverageY`: how far the shadow is cast down to level 0.
    elevation: i32,
}
/// One entity `SceneGraph.addLoc` (or a `Loc`) puts into the scene.
struct LocPiece {
    /// `LocType.getGlModel`'s result; turned and ready to align when `baked`.
    model: Model,
    site: LocSite,
    rotation: i32,
    /// Tile position the shadow is stamped at.
    shadow: Vec2,
}
/// `LocType.getStaticEntity`/`getAnimatedEntity`'s terrain alignment over
/// the loaded regions, whose extent stands in for the 104-tile scene.
fn loc_align(regions: &[Region], plane: usize, site: &LocSite, model: &mut Model) {
    if site.finish.skew.is_none() {
        return;
    }
    let corner = |pick: fn(i32, i32) -> i32, axis: usize, extra: i32| {
        regions
            .iter()
            .map(|r| [r.x, r.z][axis] * 64 + extra)
            .reduce(pick)
            .unwrap_or(0)
    };
    let origin = [corner(i32::min, 0, 0), corner(i32::min, 1, 0)];
    let end = [corner(i32::max, 0, 64), corner(i32::max, 1, 64)];
    let ground = |x: i32, z: i32| tile(regions, plane, origin[0] + x, origin[1] + z).height;
    let above = |x: i32, z: i32| tile(regions, plane + 1, origin[0] + x, origin[1] + z).height;
    let heights = rs09_client::loc_placement::Heights {
        size: [end[0] - origin[0] + 1, end[1] - origin[1] + 1],
        ground: &ground,
        above: (plane < 3).then_some(&above as &dyn Fn(i32, i32) -> i32),
    };
    site.finish.align(
        model,
        site.bounds,
        &heights,
        [
            site.center[0] - origin[0] * 128,
            site.center[1] - origin[1] * 128,
        ],
        site.base,
    );
}
/// A location entity's triangles: the model is already aligned to the
/// terrain, and stands at `position` (tiles) on `ground` (world height).
fn loc_mesh(
    mesh: &mut Mesh,
    model: &Model,
    textures: &render::Textures,
    position: Vec2,
    ground: f32,
    plane: usize,
) {
    let first = mesh.vertices.len();
    render::model(
        mesh,
        model,
        textures,
        |v| {
            vec3(
                position.x - 3200.0 + v[0] as f32 / 128.0,
                ground - v[1] as f32 / 128.0,
                position.y - 3200.0 + v[2] as f32 / 128.0,
            )
        },
        |n| vec3(n[0] as f32, -n[1] as f32, n[2] as f32),
    );
    for vertex in &mut mesh.vertices[first..] {
        vertex.normal.w += plane as f32 * 2048.0;
    }
}
/// The entities of one placement. `d` and `kind` are the type drawn (a
/// multiloc's child); `placed` is the scene slot `addLoc` chose for the
/// placed type. `entity` selects `Loc`'s own centre and height (taken from
/// the drawn type and each piece's rotation) over `addLoc`'s; `animated`
/// leaves the model as `getAnimatedEntity` caches it, to be posed, turned
/// and aligned every frame. Err names a model that is not loaded.
#[allow(clippy::too_many_arguments)]
fn loc_pieces(
    regions: &[Region],
    models: &BTreeMap<u32, Model>,
    textures: &render::Textures,
    d: &Definition,
    kind: &rs09_client::interaction::LocationType,
    placed: &rs09_client::loc_placement::Placement,
    position: [i32; 2],
    plane: usize,
    entity: bool,
    animated: bool,
) -> Result<Vec<LocPiece>, u32> {
    use rs09_client::loc_placement::{Bounds, Finish, base_height, gl_model};
    let level = |plane: usize| move |x: i32, z: i32| tile(regions, plane, x, z).height;
    let slot = base_height(position, placed.size, level(plane));
    let mut pieces = Vec::new();
    for piece in placed.pieces.iter().flatten() {
        let Some(mut model) = gl_model(
            d,
            piece.shape,
            piece.rotation,
            animated,
            &textures.materials,
            |id| models.get(&id),
        )?
        else {
            continue;
        };
        let finish = Finish::new(kind, d, *piece);
        let (size, shadow) = if entity {
            let size = if piece.rotation & 1 != 0 {
                [d.length, d.width]
            } else {
                [d.width, d.length]
            };
            (size, [0; 2])
        } else {
            (placed.size, piece.shadow)
        };
        let center = [
            position[0] * 128 + (size[0] << 6),
            position[1] * 128 + (size[1] << 6),
        ];
        let base = base_height(position, size, level(plane));
        let mut bounds = Bounds::of(&model);
        if !animated {
            finish.turn(&mut model);
            bounds = Bounds::of(&model);
        }
        pieces.push(LocPiece {
            model,
            site: LocSite {
                finish,
                bounds,
                center,
                base,
                draw: vec2(
                    position[0] as f32 + (placed.origin[0] + piece.offset[0]) as f32 / 128.0,
                    position[1] as f32 + (placed.origin[1] + piece.offset[1]) as f32 / 128.0,
                ),
                ground: -(slot as f32) / 128.0,
                elevation: if plane == 0 {
                    0
                } else {
                    base - base_height(position, size, level(0))
                },
            },
            rotation: piece.rotation,
            shadow: vec2(
                (center[0] - shadow[0]) as f32 / 128.0,
                (center[1] - shadow[1]) as f32 / 128.0,
            ),
        });
    }
    Ok(pieces)
}
struct ShadowCaster {
    sprite: rs09_client::shadow::Sprite,
    position: [i32; 2],
    plane: usize,
    active: bool,
}
struct PickObject {
    target: rs09_client::interaction::LocationTarget,
    chunk: usize,
    vertices: std::ops::Range<usize>,
    /// Which of the placement's entities this is (a corner wall has two).
    piece: u8,
    /// Where the scene draws the entity: tile position and world height.
    draw: Vec2,
    ground: f32,
    shadow: Option<usize>,
    active: bool,
    reusable: bool,
    min_y: i32,
}
struct Chunk {
    mesh: Mesh,
    gpu: Option<static_render::MeshBuffers>,
    center: Vec2,
    plane: usize,
    /// Blended floor faces: drawn nearer and without depth writes.
    blend: bool,
    /// A floor mesh: drawn before the light meshes, which precede scenery.
    floor: bool,
    /// Part of the underwater scene, drawn before the surface by its pass.
    underwater: Option<rs09_client::underwater::Pass>,
    bounds: [[f32; 3]; 2],
    fingerprint: Option<u64>,
}
impl Chunk {
    fn new(mesh: Mesh, center: Vec2, plane: usize) -> Self {
        let bounds = geometry_bounds(&mesh, 0..mesh.vertices.len()).map(|v| v.to_array());
        Self {
            mesh,
            gpu: None,
            center,
            plane,
            blend: false,
            floor: false,
            underwater: None,
            bounds,
            fingerprint: None,
        }
    }
}
struct Workbench {
    name: String,
    start: [i32; 2],
    pack: Pack,
    animation_pack: Option<Pack>,
    regions: Vec<Region>,
    models: BTreeMap<u32, Model>,
    chunks: Vec<Chunk>,
    animated_locations: Vec<AnimatedLocation>,
    avatar: Vec<Model>,
    bases: BTreeMap<u16, Base>,
    frames: BTreeMap<u32, Frame>,
    sequences: BTreeMap<u32, Sequence>,
    prepared_actor_sequences: std::collections::BTreeSet<i32>,
    slot_origin_valid: std::cell::Cell<bool>,
    slot_views: std::cell::RefCell<Vec<serde_json::Value>>,
    faces: usize,
    objects: usize,
    textures: render::Textures,
    navigation: Navigation,
    placements: Vec<Placement>,
    bas: PlayerBas,
    shadows: render::Shadows,
    static_shadows: Vec<ShadowCaster>,
    shadow_roofs: bool,
    roofs: roof_render::State,
    point_lights: render::PointLights,
    npc_types: BTreeMap<u32, NpcBody>,
    npc_bases: BTreeMap<i32, rs09_client::npc::Bas>,
    npc_definitions: BTreeMap<u32, rs09_client::npc::Definition>,
    spot_definitions: BTreeMap<i32, rs09_client::spot::Definition>,
    spot_sequences: BTreeMap<i32, i32>,
    spot_models: BTreeMap<i32, SpotBody>,
    spot_views: Vec<serde_json::Value>,
    world_spots: rs09_client::world_spot::State,
    npc_spawns: Vec<[i32; 5]>,
    npcs: Vec<NpcActor>,
    location_types: BTreeMap<u32, rs09_client::interaction::LocationType>,
    definitions: BTreeMap<String, Definition>,
    clips: BTreeMap<u32, ClippingDefinition>,
    pick_objects: Vec<PickObject>,
    multilocs: BTreeMap<rs09_client::world::Slot, MultiLoc>,
    world: rs09_client::world::Scene,
    ground: rs09_protocol::game::ground::State,
    ground_supports: BTreeMap<rs09_protocol::game::ground::Tile, i32>,
    map_revision: u64,
    /// Counts scene rebuilds only; `map_revision` also counts location changes.
    scene_revision: u64,
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    audio_changes: Vec<(rs09_client::world::Slot, rs09_client::world::Location)>,
    zone: [i32; 2],
    live_origin: bool,
    overheads: BTreeMap<(bool, usize), rs09_client::overheads::State>,
    tick: i32,
    actor_scene: actor_scene::ActorScene,
    world_cache: Option<std::sync::Arc<world_cache::Cache>>,
}
fn geometry_bounds(mesh: &Mesh, range: std::ops::Range<usize>) -> [Vec3; 2] {
    let mut bounds = [Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)];
    for v in &mesh.vertices[range] {
        bounds[0] = bounds[0].min(v.position);
        bounds[1] = bounds[1].max(v.position);
    }
    bounds
}
fn tile(regions: &[Region], plane: usize, x: i32, z: i32) -> Tile {
    regions
        .iter()
        .find(|r| r.x == x.div_euclid(64) && r.z == z.div_euclid(64))
        .map_or(Tile::default(), |r| {
            r.tile(plane, x.rem_euclid(64) as usize, z.rem_euclid(64) as usize)
        })
}
fn height(regions: &[Region], plane: usize, x: f32, z: f32) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let dx = x - x.floor();
    let dz = z - z.floor();
    let h = |x, z| -(tile(regions, plane, x, z).height as f32) / 128.0;
    (h(ix, iz) * (1.0 - dx) + h(ix + 1, iz) * dx) * (1.0 - dz)
        + (h(ix, iz + 1) * (1.0 - dx) + h(ix + 1, iz + 1) * dx) * dz
}
fn actor_height(regions: &[Region], origin: [i32; 2], plane: usize, x: f32, z: f32) -> f32 {
    -rs09_client::terrain::scene_height(
        regions,
        origin,
        plane as i32,
        [(x * 128.0) as i32, (z * 128.0) as i32],
    ) as f32
        / 128.0
}
fn actor_terrain(
    regions: &[Region],
    origin: [i32; 2],
    plane: usize,
    dimensions: [i32; 2],
    yaw: i32,
    fine: [i32; 2],
) -> rs09_client::spot::Alignment {
    if dimensions.contains(&0) {
        return rs09_client::spot::Alignment::default();
    }
    let sample = |point| rs09_client::terrain::scene_height(regions, origin, plane as i32, point);
    rs09_client::spot::Alignment::sample(dimensions, yaw, fine, sample(fine), sample)
}
fn render_plane(regions: &[Region], plane: usize, position: [i32; 2]) -> usize {
    rs09_client::terrain::render_level(
        plane,
        tile(regions, plane, position[0], position[1]).flags,
        tile(regions, 1, position[0], position[1]).flags,
    )
}
fn location_mesh(
    mesh: &mut Mesh,
    model: &Model,
    textures: &render::Textures,
    regions: &[Region],
    position: Vec2,
    plane: usize,
    hill: bool,
) {
    let first = mesh.vertices.len();
    let ground = height(regions, plane, position.x, position.y);
    render::model(
        mesh,
        model,
        textures,
        |v| {
            let dx = v[0] as f32 / 128.0;
            let dz = v[2] as f32 / 128.0;
            let y = if hill {
                height(regions, plane, position.x + dx, position.y + dz)
            } else {
                ground
            };
            vec3(
                position.x - 3200.0 + dx,
                y - v[1] as f32 / 128.0,
                position.y - 3200.0 + dz,
            )
        },
        |n| vec3(n[0] as f32, -n[1] as f32, n[2] as f32),
    );
    for vertex in &mut mesh.vertices[first..] {
        vertex.normal.w += plane as f32 * 2048.0;
    }
}
struct Geometry {
    chunks: Vec<PreparedChunk>,
    animated_locations: Vec<AnimatedLocation>,
    faces: usize,
    objects: usize,
    shadows: render::ShadowMap,
    static_shadows: Vec<ShadowCaster>,
    pick_objects: Vec<PickObject>,
    /// Placements of types with children. The build draws nothing for them:
    /// `Workbench::sync_multilocs` resolves each against the variables.
    multilocs: Vec<(rs09_client::world::Slot, rs09_client::world::Location)>,
}
fn bind_scene_materials(workbench: &Workbench, materials: [&Material; 2]) {
    for material in materials {
        material.set_texture("LightGrid", workbench.point_lights.grid.clone());
        material.set_texture("LightData", workbench.point_lights.data.clone());
        material.set_uniform(
            "LightRegion",
            Vec4::from_array(rs09_client::sd_light::light_region([
                workbench.point_lights.origin.x - 3200.0,
                workbench.point_lights.origin.y - 3200.0,
                workbench.point_lights.size.x,
                workbench.point_lights.size.y,
            ])),
        );
        material.set_texture("ShadowMap", workbench.shadows.texture.clone());
        material.set_uniform(
            "ShadowRegion",
            vec4(
                workbench.shadows.origin.x - 3200.0,
                workbench.shadows.origin.y - 3200.0,
                (workbench.shadows.map.width - 2) as f32 / 16.0,
                (workbench.shadows.map.height - 2) as f32 / 16.0,
            ),
        );
    }
}
struct PreparedChunk {
    mesh: Mesh,
    center: Vec2,
    plane: usize,
    blend: bool,
    floor: bool,
    underwater: Option<rs09_client::underwater::Pass>,
    bounds: [[f32; 3]; 2],
    fingerprint: u64,
}
impl PreparedChunk {
    fn new(mesh: Mesh, center: Vec2, plane: usize) -> Self {
        let bounds = geometry_bounds(&mesh, 0..mesh.vertices.len()).map(|v| v.to_array());
        Self {
            fingerprint: mesh_fingerprint(&mesh),
            mesh,
            center,
            plane,
            blend: false,
            floor: false,
            underwater: None,
            bounds,
        }
    }
    fn activate(self) -> Chunk {
        Chunk {
            mesh: self.mesh,
            gpu: None,
            center: self.center,
            plane: self.plane,
            blend: self.blend,
            floor: self.floor,
            underwater: self.underwater,
            bounds: self.bounds,
            fingerprint: Some(self.fingerprint),
        }
    }
}
fn mesh_fingerprint(mesh: &Mesh) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    let mut mix = |word: u32| {
        hash = (hash ^ u64::from(word)).wrapping_mul(0x100000001b3);
    };
    mix(mesh.vertices.len() as u32);
    mix(mesh.indices.len() as u32);
    for vertex in &mesh.vertices {
        for value in vertex
            .position
            .to_array()
            .into_iter()
            .chain(vertex.uv.to_array())
            .chain(vertex.normal.to_array())
        {
            mix(value.to_bits());
        }
        mix(u32::from_le_bytes(vertex.color));
    }
    for &index in &mesh.indices {
        mix(u32::from(index));
    }
    hash
}
mod terrain_mesh;
mod underwater_mesh;

impl Geometry {
    #[allow(clippy::too_many_arguments)]
    fn build(
        scene: &Scene,
        regions: &[Region],
        models: &BTreeMap<u32, Model>,
        sequences: &BTreeMap<u32, Sequence>,
        textures: &render::Textures,
        location_types: &BTreeMap<u32, rs09_client::interaction::LocationType>,
        underlays: &[rs09_client::floor::Underlay],
        overlays: &[rs09_client::floor::Overlay],
    ) -> Result<Self, String> {
        let min_x = scene
            .regions
            .iter()
            .map(|r| r[0] * 64)
            .min()
            .ok_or("No regions")?;
        let min_z = scene
            .regions
            .iter()
            .map(|r| r[1] * 64)
            .min()
            .ok_or("No regions")?;
        let width = (scene.regions.iter().map(|r| r[0] * 64 + 64).max().unwrap() - min_x) as usize;
        let depth = (scene.regions.iter().map(|r| r[1] * 64 + 64).max().unwrap() - min_z) as usize;
        let direction = regions
            .iter()
            .find(|r| r.x == scene.start[0] >> 6 && r.z == scene.start[1] >> 6)
            .map_or_else(rs09_client::atmosphere::Environment::default, |r| {
                r.environment
            })
            .direction;
        // FogManager.light0Position while this scene's models and floors are
        // packed, for colours baked without high detail lighting.
        rs09_client::sd_light::set_sun(direction);
        let mut shadows = render::ShadowMap::new(
            vec2(min_x as f32, min_z as f32),
            width / 8,
            depth / 8,
            direction,
        );
        let mut pick_objects = Vec::new();
        let mut by_chunk = BTreeMap::<_, Vec<_>>::new();
        for (index, placement @ [_, x, z, plane, _, _]) in
            scene.locations.iter().copied().enumerate()
        {
            by_chunk
                .entry((x >> 4, z >> 4, plane as usize))
                .or_default()
                .push((index, placement));
        }
        // SceneGraph.addLoc over the whole map, in file order.
        let plan = rs09_client::loc_placement::Plan::new(
            &scene.locations,
            |id| {
                Some((
                    scene.definitions.get(&id.to_string())?,
                    location_types.get(&u32::try_from(id).ok()?)?,
                ))
            },
            rs09_client::loc_placement::ground_decorations(),
        );
        let mut multilocs = Vec::new();
        let mut static_shadows = Vec::new();
        // SceneGraph.buildScene: the floors of the upper levels shade the
        // ground. Kept as ordinary casters, on the ground level.
        for plane in 1..4 {
            for x in min_x..min_x + width as i32 {
                for z in min_z..min_z + depth as i32 {
                    let t = tile(regions, plane, x, z);
                    if t.underlay == 0 && t.overlay == 0 {
                        continue;
                    }
                    if let Some(caster) = scene_lights::floor_shadow(
                        &shadows,
                        regions,
                        (underlays, overlays),
                        textures,
                        plane,
                        [x, z],
                        t,
                    ) {
                        shadows.map.stamp(&caster.sprite, caster.position, false);
                        static_shadows.push(caster);
                    }
                }
            }
        }
        let mut chunks = Vec::new();
        let mut terrain = terrain_mesh::Builder::default();
        let mut animated_locations = Vec::new();
        let mut faces = 0;
        let mut objects = 0;
        // Spatial chunks bound GPU index buffers and allow camera-distance culling.
        for r in regions {
            for cx in 0..4 {
                for cz in 0..4 {
                    for plane in 0..4 {
                        let ox = r.x * 64 + cx * 16;
                        let oz = r.z * 64 + cz * 16;
                        let center = vec2(ox as f32 + 8.0, oz as f32 + 8.0);
                        let mut mesh = textures.mesh();
                        let objects_in_chunk = by_chunk
                            .get(&(ox >> 4, oz >> 4, plane))
                            .map(Vec::as_slice)
                            .unwrap_or_default();
                        for &(index, [id, x, z, p, shape, rotation]) in objects_in_chunk {
                            // What SceneGraph.addLoc leaves in the scene for
                            // this row: nothing for a type hidden in HD, a
                            // replaced wall or an unwanted ground decoration.
                            let Some(planned) = plan.entries[index] else {
                                continue;
                            };
                            let d = &scene.definitions[&id.to_string()];
                            let kind = &location_types[&(id as u32)];
                            let target = rs09_client::interaction::LocationTarget {
                                id: id as u32,
                                tile: [x, z],
                                plane,
                                shape: shape as u8,
                                rotation: rotation as u8,
                                size: [d.width, d.length],
                                blocked_sides: kind.blocked_sides,
                            };
                            if kind.children.is_some() {
                                if let Some(layer) = rs09_client::world::layer(shape as u8) {
                                    multilocs.push((
                                        rs09_client::world::Slot {
                                            plane,
                                            tile: [x, z],
                                            layer,
                                        },
                                        rs09_client::world::Location {
                                            id,
                                            shape: shape as u8,
                                            rotation: rotation as u8,
                                        },
                                    ));
                                }
                                continue;
                            }
                            let ground_visible = plane > 0
                                && rs09_client::terrain::render_level(
                                    plane,
                                    tile(regions, plane, x, z).flags,
                                    tile(regions, 1, x, z).flags,
                                ) == 0;
                            let display_plane = if ground_visible { 0 } else { plane };
                            let animated = ![-1, 65535].contains(&d.animation);
                            let pieces = loc_pieces(
                                regions,
                                models,
                                textures,
                                d,
                                kind,
                                &planned.placement,
                                [x, z],
                                plane,
                                animated,
                                animated,
                            )
                            .map_err(|id| format!("missing location model {id}"))?;
                            if !pieces.is_empty() {
                                objects += 1;
                            }
                            for (part, piece) in pieces.into_iter().enumerate() {
                                let mut model = piece.model;
                                if animated {
                                    let sequence = d.animation as u32;
                                    let seq = sequences.get(&sequence).ok_or_else(|| {
                                        format!("missing location sequence {sequence}")
                                    })?;
                                    // Controlled random inputs keep both platforms/captures reproducible.
                                    // Each entity of a two-piece placement is its own Loc.
                                    let seed = ((id as u32).wrapping_mul(73856093)
                                        ^ (x as u32).wrapping_mul(19349663)
                                        ^ (z as u32).wrapping_mul(83492791)
                                        ^ p as u32)
                                        ^ (part as u32).wrapping_mul(0x9e3779b9);
                                    let random = d.randomized_animation.then_some([
                                        f64::from(seed) / 4294967296.0,
                                        f64::from(
                                            seed.wrapping_mul(1664525).wrapping_add(1013904223),
                                        ) / 4294967296.0,
                                    ]);
                                    let clock = location::Animation::new(seq, 0, random)
                                        .ok_or_else(|| {
                                            format!("invalid location sequence {sequence}")
                                        })?;
                                    faces += model.draw_order.len();
                                    animated_locations.push(AnimatedLocation {
                                        target,
                                        last_pose: None,
                                        pick_bounds: [Vec3::ZERO; 2],
                                        model,
                                        sequence,
                                        clock,
                                        position: vec2(
                                            piece.site.center[0] as f32 / 128.0,
                                            piece.site.center[1] as f32 / 128.0,
                                        ),
                                        plane,
                                        site: piece.site,
                                        piece: part as u8,
                                        rotation: piece.rotation,
                                        cast_shadow: d.cast_shadow,
                                        shadow: None,
                                        shadow_state: Default::default(),
                                    });
                                    continue;
                                }
                                // The shadow is projected before the terrain alignment.
                                let mut shadow_index = None;
                                if d.cast_shadow {
                                    if let Some(sprite) = rs09_client::shadow::project(
                                        &model,
                                        &textures.materials,
                                        shadows.light,
                                    ) {
                                        let position =
                                            shadows.position(piece.shadow, piece.site.elevation);
                                        shadows.map.stamp(&sprite, position, false);
                                        shadow_index = Some(static_shadows.len());
                                        static_shadows.push(ShadowCaster {
                                            sprite,
                                            position,
                                            plane: display_plane,
                                            active: true,
                                        });
                                    }
                                }
                                loc_align(regions, plane, &piece.site, &mut model);
                                if (ground_visible && !mesh.indices.is_empty())
                                    || (!ground_visible
                                        && mesh.vertices.len() + model.faces.len() * 3 > 60000)
                                {
                                    faces += mesh.indices.len() / 3;
                                    chunks.push(PreparedChunk::new(mesh, center, plane));
                                    mesh = textures.mesh();
                                }
                                let mut separate_mesh = textures.mesh();
                                let output = if ground_visible {
                                    &mut separate_mesh
                                } else {
                                    &mut mesh
                                };
                                let first_vertex = output.vertices.len();
                                loc_mesh(
                                    output,
                                    &model,
                                    textures,
                                    piece.site.draw,
                                    piece.site.ground,
                                    plane,
                                );
                                scene_lights::stamp(
                                    &mut output.vertices[first_vertex..],
                                    vec2(min_x as f32, min_z as f32),
                                    scene_lights::location_object(
                                        shape,
                                        rotation,
                                        planned.placement.size,
                                        ground_visible,
                                        part,
                                    ),
                                    [x, z],
                                    display_plane,
                                );
                                let vertices = first_vertex..output.vertices.len();
                                // One entity of the placement, the first
                                // before the second, as addLoc hands them on.
                                terrain.placed(
                                    [id, x, z, p, shape, rotation],
                                    &model,
                                    textures,
                                    kind,
                                    (chunks.len(), vertices.clone()),
                                );
                                pick_objects.push(PickObject {
                                    target,
                                    chunk: chunks.len(),
                                    vertices,
                                    piece: part as u8,
                                    draw: piece.site.draw,
                                    ground: piece.site.ground,
                                    shadow: shadow_index,
                                    active: true,
                                    reusable: false,
                                    min_y: i32::from(
                                        model.vertices.iter().fold(32767, |n, v| n.min(v[1]))
                                            as i16,
                                    ),
                                });
                                if ground_visible {
                                    faces += separate_mesh.indices.len() / 3;
                                    chunks.push(PreparedChunk::new(
                                        separate_mesh,
                                        center,
                                        display_plane,
                                    ));
                                }
                            }
                        }
                        if !mesh.indices.is_empty() {
                            faces += mesh.indices.len() / 3;
                            chunks.push(PreparedChunk::new(mesh, center, plane));
                        }
                    }
                }
            }
        }
        terrain.finish(
            scene,
            regions,
            textures,
            location_types,
            underlays,
            overlays,
            &mut chunks,
            &mut pick_objects,
            &mut faces,
        );
        underwater_mesh::build(
            scene,
            regions,
            models,
            textures,
            location_types,
            underlays,
            overlays,
            &mut chunks,
            &mut faces,
        )?;
        Ok(Self {
            chunks,
            animated_locations,
            faces,
            objects,
            shadows,
            static_shadows,
            pick_objects,
            multilocs,
        })
    }
}
fn scene_floors(
    pack: &Pack,
) -> Result<
    (
        Vec<rs09_client::floor::Underlay>,
        Vec<rs09_client::floor::Overlay>,
    ),
    String,
> {
    use rs09_client::floor::{Overlay, Underlay};
    let mut underlays = vec![Underlay::default(); 256];
    let mut overlays = vec![Overlay::default(); 256];
    for name in pack.manifest.files.keys() {
        if let Some(id) = name.strip_prefix("underlay/") {
            underlays[id.parse::<usize>().map_err(|e| e.to_string())?] =
                Underlay::decode(pack.get(name)?).map_err(|e| e.to_string())?;
        }
        if let Some(id) = name.strip_prefix("overlay/") {
            overlays[id.parse::<usize>().map_err(|e| e.to_string())?] =
                Overlay::decode(pack.get(name)?).map_err(|e| e.to_string())?;
        }
    }
    Ok((underlays, overlays))
}

impl Workbench {
    fn rebuild_navigation(&mut self) {
        self.placements = self
            .world
            .locations
            .iter()
            .filter_map(|(slot, loc)| {
                let d = self.clips.get(&(loc.id as u32))?;
                Some(Placement {
                    position: slot.tile,
                    plane: slot.plane,
                    shape: loc.shape,
                    rotation: loc.rotation,
                    size: d.size,
                    walk: d.walk,
                    projectiles: d.projectiles,
                })
            })
            .collect();
        self.navigation = Navigation::new(&self.regions, &self.placements, self.navigation.origin);
    }
    fn refresh_navigation(&mut self, tile: [i32; 2]) {
        if self.live_origin {
            return;
        }
        let local = [
            tile[0] - self.navigation.origin[0],
            tile[1] - self.navigation.origin[1],
        ];
        if local.into_iter().any(|n| !(16..88).contains(&n)) {
            self.navigation = Navigation::new(
                &self.regions,
                &self.placements,
                tile.map(|n| ((n >> 3) - 6) << 3),
            );
        }
    }
    fn environment(&self, position: Vec2) -> rs09_client::atmosphere::Environment {
        let tile = [position.x as i32, position.y as i32];
        self.regions
            .iter()
            .find(|r| r.x == tile[0].div_euclid(64) && r.z == tile[1].div_euclid(64))
            .map_or_else(rs09_client::atmosphere::Environment::default, |r| {
                r.environment_at(
                    tile[0].rem_euclid(64) as usize,
                    tile[1].rem_euclid(64) as usize,
                )
            })
    }
    fn load(pack: Pack) -> Result<Self, String> {
        let mut scene: Scene =
            serde_json::from_slice(pack.get("scene.json")?).map_err(|e| e.to_string())?;
        let mut location_types = BTreeMap::new();
        for id in scene.definitions.keys() {
            location_types.insert(
                id.parse::<u32>().map_err(|e| e.to_string())?,
                rs09_client::interaction::LocationType::decode(pack.get(&format!("loc/{id}"))?)
                    .map_err(|e| e.to_string())?,
            );
        }
        let mut world = rs09_client::world::Scene::default();
        for &[id, x, y, p, shape, rotation] in &scene.locations {
            if let Some(layer) = rs09_client::world::layer(shape as u8) {
                world.locations.insert(
                    rs09_client::world::Slot {
                        plane: p as usize,
                        tile: [x, y],
                        layer,
                    },
                    rs09_client::world::Location {
                        id,
                        shape: shape as u8,
                        rotation: rotation as u8,
                    },
                );
            }
        }
        let mut regions = Vec::new();
        let mut models = BTreeMap::new();
        let mut bases = BTreeMap::new();
        let mut frames = BTreeMap::new();
        let mut sequences = BTreeMap::new();
        let mut spot_definitions = BTreeMap::new();
        for [x, z] in &scene.regions {
            regions.push(
                Region::decode(*x, *z, pack.get(&format!("terrain/{x}_{z}"))?)
                    .map_err(|e| e.to_string())?,
            );
        }
        underwater_mesh::load_pack(&pack, &mut scene, &mut location_types, &mut regions)?;
        for name in pack.manifest.files.keys() {
            if let Some(id) = name.strip_prefix("spot/") {
                spot_definitions.insert(
                    id.parse::<i32>().map_err(|e| e.to_string())?,
                    rs09_client::spot::Definition::decode(pack.get(name)?)
                        .map_err(|e| format!("{name}: {e}"))?
                        .0,
                );
            }
            if let Some(id) = name.strip_prefix("model/") {
                models.insert(
                    id.parse::<u32>().map_err(|e| e.to_string())?,
                    Model::decode(pack.get(name)?).map_err(|e| format!("{name}: {e}"))?,
                );
            }
            if let Some(id) = name.strip_prefix("base/") {
                let id = id.parse::<u16>().map_err(|e| e.to_string())?;
                bases.insert(
                    id,
                    Base::decode(id, pack.get(name)?).map_err(|e| format!("{name}: {e}"))?,
                );
            }
            if let Some(id) = name.strip_prefix("seq/") {
                sequences.insert(
                    id.parse::<u32>().map_err(|e| e.to_string())?,
                    Sequence::decode(pack.get(name)?).map_err(|e| format!("{name}: {e}"))?,
                );
            }
        }
        for name in pack
            .manifest
            .files
            .keys()
            .filter(|n| n.starts_with("frame/"))
        {
            let parts: Vec<_> = name.split('/').collect();
            let group = parts[1].parse::<u32>().map_err(|e| e.to_string())?;
            let file = parts[2].parse::<u32>().map_err(|e| e.to_string())?;
            let data = pack.get(name)?;
            let base = u16::from_be_bytes(data[..2].try_into().unwrap());
            frames.insert(
                group << 16 | file,
                Frame::decode(data, bases.get(&base).ok_or("missing animation base")?)
                    .map_err(|e| e.to_string())?,
            );
        }
        let textures = render::Textures::load(&pack)?;
        let bas = PlayerBas::decode(pack.get("bas/1426")?).map_err(|e| e.to_string())?;
        let clipping: BTreeMap<_, _> = scene
            .definitions
            .keys()
            .map(|id| {
                let bytes = pack.get(&format!("loc/{id}"))?;
                Ok((
                    id.clone(),
                    ClippingDefinition::decode(bytes).map_err(|e| format!("loc/{id}: {e}"))?,
                ))
            })
            .collect::<Result<_, String>>()?;
        let placements: Vec<_> = scene
            .locations
            .iter()
            .map(|&[id, x, z, p, shape, rotation]| {
                let d = &clipping[&id.to_string()];
                Placement {
                    position: [x, z],
                    plane: p as usize,
                    shape: shape as u8,
                    rotation: rotation as u8,
                    size: d.size,
                    walk: d.walk,
                    projectiles: d.projectiles,
                }
            })
            .collect();
        let navigation = Navigation::new(
            &regions,
            &placements,
            scene.start.map(|n| ((n >> 3) - 6) << 3),
        );
        let (underlays, overlays) = scene_floors(&pack)?;
        let Geometry {
            chunks,
            animated_locations,
            faces,
            objects,
            shadows,
            static_shadows,
            pick_objects,
            multilocs,
        } = Geometry::build(
            &scene,
            &regions,
            &models,
            &sequences,
            &textures,
            &location_types,
            &underlays,
            &overlays,
        )?;
        let mut avatar_model =
            rs09_client::appearance::assemble(&pack, &models, &scene.avatar_kits, [0; 5])?;
        avatar_model.lighting = [64, 850];
        avatar_model.sort_by_priority = true;
        avatar_model.draw_order =
            rs09_client::surface::face_order(&avatar_model, &textures.materials);
        avatar_model.surfaces = rs09_client::surface::prepare(&avatar_model, &textures.materials);
        let avatar = vec![avatar_model];
        let point_lights = render::PointLights::new(&regions, &pack, navigation.origin)?;
        let mut npc_definitions = BTreeMap::new();
        for name in pack.manifest.files.keys() {
            if let Some(id) = name
                .strip_prefix("npc/")
                .and_then(|s| s.parse::<u32>().ok())
            {
                npc_definitions.insert(
                    id,
                    rs09_client::npc::Definition::decode(pack.get(name)?)
                        .map_err(|e| format!("NPC{id}: {e}"))?,
                );
            }
        }
        let mut npc_types = BTreeMap::new();
        let mut npc_bases = BTreeMap::new();
        for id in &scene.npc_types {
            let definition = rs09_client::npc::Definition::decode(pack.get(&format!("npc/{id}"))?)
                .map_err(|e| e.to_string())?;
            let bas = rs09_client::npc::Bas::decode(pack.get(&format!("bas/{}", definition.bas))?)
                .map_err(|e| e.to_string())?;
            npc_bases.insert(definition.bas, bas.clone());
            if definition.children.is_some() {
                continue;
            }
            let mut model = definition.assemble_with_bas(&models, &bas.movement)?;
            model.draw_order = rs09_client::surface::face_order(&model, &textures.materials);
            model.surfaces = rs09_client::surface::prepare(&model, &textures.materials);
            npc_types.insert(*id, NpcBody { definition, model });
        }
        let spot_sequences = if pack.get("spot-catalog-complete").is_ok() {
            (0..65536)
                .map(|id| (id, spot_definitions.get(&id).map_or(-1, |d| d.sequence)))
                .collect()
        } else {
            spot_definitions
                .iter()
                .map(|(&id, d)| (id, d.sequence))
                .collect()
        };
        Ok(Self {
            name: scene.name,
            start: scene.start,
            pack,
            animation_pack: None,
            regions,
            models,
            chunks: chunks.into_iter().map(PreparedChunk::activate).collect(),
            animated_locations,
            avatar,
            bases,
            frames,
            sequences,
            prepared_actor_sequences: Default::default(),
            slot_origin_valid: Default::default(),
            slot_views: Default::default(),
            faces,
            objects,
            textures,
            navigation,
            placements,
            bas,
            shadows: render::Shadows::from_map(shadows),
            static_shadows,
            // Every level's scenery shades the ground, as SceneGraph.addLoc has it.
            shadow_roofs: true,
            roofs: Default::default(),
            point_lights,
            npc_types,
            npc_bases,
            npc_definitions,
            spot_sequences,
            spot_definitions,
            spot_models: BTreeMap::new(),
            spot_views: Vec::new(),
            world_spots: rs09_client::world_spot::State::default(),
            npc_spawns: scene.npc_spawns,
            npcs: Vec::new(),
            location_types,
            definitions: scene.definitions,
            clips: clipping
                .into_iter()
                .map(|(id, clip)| (id.parse::<u32>().unwrap(), clip))
                .collect(),
            pick_objects,
            multilocs: multilocs
                .into_iter()
                .map(|(slot, loc)| (slot, MultiLoc::new(loc, None)))
                .collect(),
            world,
            ground: rs09_protocol::game::ground::State::default(),
            ground_supports: BTreeMap::new(),
            map_revision: 0,
            scene_revision: 0,
            #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
            audio_changes: Vec::new(),
            zone: [0; 2],
            live_origin: false,
            overheads: BTreeMap::new(),
            tick: 0,
            actor_scene: Default::default(),
            world_cache: None,
        })
    }
    fn dynamic_location(
        &mut self,
        slot: rs09_client::world::Slot,
        loc: rs09_client::world::Location,
        animation: Option<i32>,
    ) -> Result<(), String> {
        let multiloc = u32::try_from(loc.id)
            .ok()
            .and_then(|id| self.location_types.get(&id))
            .is_some_and(|kind| kind.children.is_some());
        if !multiloc {
            self.multilocs.remove(&slot);
            let clock = if animation.is_some() {
                LocClock::Replaced
            } else {
                LocClock::New
            };
            let drawn = u32::try_from(loc.id).ok();
            return self.replace_location(slot, loc, drawn, animation, clock, true);
        }
        // The scene holds a Loc, whose child `sync_multilocs` resolves before
        // the next frame is drawn. A LOC_ANIM replaces the Loc with one that
        // plays the sequence; what is drawn stays until that rebuild.
        let same = self.multilocs.get(&slot).copied().filter(|old| {
            animation.is_some()
                && (old.loc.id, old.loc.shape, old.loc.rotation)
                    == (loc.id, loc.shape, loc.rotation)
        });
        let entry = match same {
            Some(old) => MultiLoc {
                animation,
                built: None,
                replaced: true,
                ..old
            },
            None => {
                self.replace_location(slot, loc, None, None, LocClock::New, true)?;
                MultiLoc::new(loc, animation)
            }
        };
        self.multilocs.insert(slot, entry);
        Ok(())
    }
    /// The type a placed location currently shows (`LocType.getMultiLoc`),
    /// with its id: the type itself, or the child `sync_multilocs` last
    /// resolved for a multiloc. None when it shows nothing. Drawing, menu
    /// entries and Examine all go through that one resolution.
    fn menu_location(
        &self,
        target: &rs09_client::interaction::LocationTarget,
    ) -> Option<(u32, &rs09_client::interaction::LocationType)> {
        let kind = self.location_types.get(&target.id)?;
        if kind.children.is_none() {
            return Some((target.id, kind));
        }
        let slot = rs09_client::world::Slot {
            plane: target.plane,
            tile: target.tile,
            layer: rs09_client::world::layer(target.shape)?,
        };
        let form = self.multilocs.get(&slot)?.built??;
        Some((form, self.location_types.get(&form)?))
    }
    /// `Loc.getOrBuildEntity`'s `LocType.getMultiLoc()`, which the original
    /// evaluates for every multiloc each frame: a changed child rebuilds the
    /// model and its shadow, a null child leaves nothing. `read` returns a
    /// varbit (`true`) or a varp (`false`).
    fn sync_multilocs(&mut self, mut read: impl FnMut(bool, i32) -> i32) -> Result<(), String> {
        if self.multilocs.is_empty() {
            return Ok(());
        }
        let mut values = BTreeMap::new();
        let mut due = Vec::new();
        for (slot, entry) in &self.multilocs {
            let Some(kind) = u32::try_from(entry.loc.id)
                .ok()
                .and_then(|id| self.location_types.get(&id))
            else {
                continue;
            };
            let child = kind.select_morph(|bit, key| {
                *values.entry((bit, key)).or_insert_with(|| read(bit, key))
            });
            if entry.built != Some(child) {
                due.push((*slot, child));
            }
        }
        for (slot, child) in due {
            // Natively a child the scene has not met is read from the cache.
            #[cfg(not(target_arch = "wasm32"))]
            let usable = child.is_none_or(|child| self.ensure_location(child).is_ok());
            #[cfg(target_arch = "wasm32")]
            let usable = true;
            if let Some(child) = child {
                let loaded = self.location_types.contains_key(&child)
                    && self
                        .definitions
                        .get(&child.to_string())
                        .is_some_and(|d| d.models.iter().all(|id| self.models.contains_key(id)));
                if usable && !loaded {
                    // Still streaming in, or absent from a prepared pack:
                    // asked again next frame.
                    continue;
                }
            }
            let MultiLoc {
                loc,
                animation,
                shown,
                replaced,
                ..
            } = self.multilocs[&slot];
            let drawn = child.filter(|_| usable);
            if shown || drawn.is_some() {
                let clock = if replaced {
                    LocClock::Replaced
                } else {
                    LocClock::Kept
                };
                self.replace_location(slot, loc, drawn, animation, clock, shown)?;
            }
            if let Some(entry) = self.multilocs.get_mut(&slot) {
                entry.built = Some(child);
                entry.shown = drawn.is_some();
                entry.replaced = false;
            }
        }
        Ok(())
    }
    /// Takes what the slot shows out of the scene (when `clear`) and puts
    /// `loc` there, drawn as the type `drawn`: the placed type itself, or a
    /// multiloc's current child. None draws nothing.
    fn replace_location(
        &mut self,
        slot: rs09_client::world::Slot,
        loc: rs09_client::world::Location,
        drawn: Option<u32>,
        animation: Option<i32>,
        clock: LocClock,
        clear: bool,
    ) -> Result<(), String> {
        let mut reusable = None;
        for object in &mut self.pick_objects {
            if !clear
                || !object.active
                || object.target.plane != slot.plane
                || object.target.tile != slot.tile
                || rs09_client::world::layer(object.target.shape) != Some(slot.layer)
            {
                continue;
            }
            let chunk = &mut self.chunks[object.chunk];
            // Invalidate before either index removal or complete buffer reuse.
            // New Chunk values always start without GPU buffers, even when the
            // replacement happens to have identical vertex/index counts.
            chunk.gpu = None;
            chunk.fingerprint = None;
            let mesh = &mut chunk.mesh;
            if object.reusable {
                reusable = Some(object.chunk);
                mesh.indices.clear();
                mesh.vertices.clear();
            } else {
                mesh.indices
                    .retain(|&i| !object.vertices.contains(&(i as usize)));
            }
            object.active = false;
            if let Some(index) = object.shadow {
                let caster = &mut self.static_shadows[index];
                if caster.active && (self.shadow_roofs || caster.plane == 0) {
                    self.shadows
                        .map
                        .stamp(&caster.sprite, caster.position, true);
                }
                caster.active = false;
            }
        }
        // The clocks of the Locs being replaced, first entity first.
        let mut previous = Vec::new();
        self.animated_locations.retain(|old| {
            let remove = clear
                && old.target.plane == slot.plane
                && old.target.tile == slot.tile
                && rs09_client::world::layer(old.target.shape) == Some(slot.layer);
            if remove {
                previous.push((old.sequence, old.clock.clone()));
                if let Some(sprite) = &old.shadow {
                    self.shadows.map.stamp(
                        sprite,
                        self.shadows.position(old.position, old.site.elevation),
                        true,
                    );
                }
            }
            !remove
        });
        let Some(drawn) = drawn else {
            return Ok(());
        };
        // `placed` is the type in the map or packet, which owns the scene
        // slot; `d` is the type whose model fills it.
        let placed = self
            .definitions
            .get(&loc.id.to_string())
            .ok_or_else(|| format!("Waiting for location definition {}", loc.id))?;
        let placed_kind = self
            .location_types
            .get(&(loc.id as u32))
            .ok_or_else(|| format!("Waiting for location definition {}", loc.id))?;
        let d = self
            .definitions
            .get(&drawn.to_string())
            .ok_or_else(|| format!("Waiting for location definition {drawn}"))?;
        let kind = self
            .location_types
            .get(&drawn)
            .ok_or_else(|| format!("Waiting for location definition {drawn}"))?;
        if placed_kind.placement.hidden_in_hd
            || (loc.shape == 22
                && !rs09_client::loc_placement::ground_decoration_shown(
                    rs09_client::loc_placement::ground_decorations(),
                    placed_kind,
                ))
        {
            return Ok(());
        }
        // A decoration takes its offset from the wall standing on its tile.
        let wall_offset = self
            .world
            .locations
            .get(&rs09_client::world::Slot { layer: 0, ..slot })
            .and_then(|wall| self.location_types.get(&u32::try_from(wall.id).ok()?))
            .map_or(16, |wall| wall.placement.wall_offset);
        // LOC_ANIM's scenery branch turns shape 11 into shape 10 without the
        // rotation + 4 that addLoc gives it, so the diagonal turn is lost.
        let shape = if animation.is_some() && loc.shape == 11 {
            10
        } else {
            i32::from(loc.shape)
        };
        let Some(site) = rs09_client::loc_placement::placement(
            shape,
            i32::from(loc.rotation),
            [placed.width, placed.length],
            wall_offset,
        ) else {
            return Ok(());
        };
        let seq = animation.unwrap_or(placed.animation);
        let animated = ![-1, 65535].contains(&seq);
        let entity =
            animation.is_some() || rs09_client::loc_placement::is_entity(placed, placed_kind);
        let pieces = loc_pieces(
            &self.regions,
            &self.models,
            &self.textures,
            d,
            kind,
            &site,
            slot.tile,
            slot.plane,
            entity,
            animated,
        )
        .map_err(|id| format!("Waiting for model {id}"))?;
        let target = rs09_client::interaction::LocationTarget {
            id: loc.id as u32,
            tile: slot.tile,
            plane: slot.plane,
            shape: loc.shape,
            rotation: loc.rotation,
            size: [placed.width, placed.length],
            blocked_sides: placed_kind.blocked_sides,
        };
        let (cast_shadow, randomized) = (d.cast_shadow, placed.randomized_animation);
        let display_plane = render_plane(&self.regions, slot.plane, slot.tile);
        if animated {
            let sequence = seq as u32;
            let s = self
                .sequences
                .get(&sequence)
                .ok_or_else(|| format!("Waiting for sequence {seq}"))?;
            for (part, piece) in pieces.into_iter().enumerate() {
                let old = previous.get(part).map(|(id, clock)| (*id, clock));
                let retained = match clock {
                    // addLoc builds a new Loc.
                    LocClock::New => None,
                    // LOC_ANIM: Loc's constructor continues the replaced Loc's
                    // clock for the same sequence with exactmove 0.
                    LocClock::Replaced => {
                        rs09_client::loc_placement::retained_clock(sequence, s.exact_move, old)
                    }
                    // The same Loc drawing another child.
                    LocClock::Kept => rs09_client::loc_placement::retained_clock(sequence, 0, old),
                };
                let clock = match retained {
                    Some(clock) => clock,
                    None => {
                        // addLoc lets the type randomize the start; the
                        // inputs are controlled, as in the scene build.
                        let seed = ((loc.id as u32).wrapping_mul(73856093)
                            ^ (slot.tile[0] as u32).wrapping_mul(19349663)
                            ^ (slot.tile[1] as u32).wrapping_mul(83492791)
                            ^ slot.plane as u32)
                            ^ (part as u32).wrapping_mul(0x9e3779b9)
                            ^ self.tick as u32;
                        let random = (matches!(clock, LocClock::New) && randomized).then_some([
                            f64::from(seed) / 4294967296.0,
                            f64::from(seed.wrapping_mul(1664525).wrapping_add(1013904223))
                                / 4294967296.0,
                        ]);
                        location::Animation::new(s, self.tick, random)
                            .ok_or("Invalid location animation")?
                    }
                };
                self.animated_locations.push(AnimatedLocation {
                    target,
                    last_pose: None,
                    pick_bounds: [Vec3::ZERO; 2],
                    model: piece.model,
                    sequence,
                    clock,
                    position: vec2(
                        piece.site.center[0] as f32 / 128.0,
                        piece.site.center[1] as f32 / 128.0,
                    ),
                    plane: slot.plane,
                    site: piece.site,
                    piece: part as u8,
                    rotation: piece.rotation,
                    cast_shadow,
                    shadow: None,
                    shadow_state: Default::default(),
                });
            }
            return Ok(());
        }
        // Every entity of the placement shares one mesh and chunk.
        let mut mesh = self.textures.mesh();
        let mut parts = Vec::new();
        let mut position = vec2(slot.tile[0] as f32 + 0.5, slot.tile[1] as f32 + 0.5);
        for (part, piece) in pieces.into_iter().enumerate() {
            let mut model = piece.model;
            // The shadow is projected before the terrain alignment.
            let sprite = cast_shadow
                .then(|| {
                    rs09_client::shadow::project(
                        &model,
                        &self.textures.materials,
                        self.shadows.light,
                    )
                })
                .flatten();
            loc_align(&self.regions, slot.plane, &piece.site, &mut model);
            let first = mesh.vertices.len();
            loc_mesh(
                &mut mesh,
                &model,
                &self.textures,
                piece.site.draw,
                piece.site.ground,
                slot.plane,
            );
            scene_lights::stamp(
                &mut mesh.vertices[first..],
                self.point_lights.origin,
                scene_lights::location_object(
                    i32::from(loc.shape),
                    i32::from(loc.rotation),
                    site.size,
                    display_plane != slot.plane,
                    part,
                ),
                slot.tile,
                display_plane,
            );
            position = piece.site.draw;
            parts.push((
                first..mesh.vertices.len(),
                sprite.map(|sprite| (sprite, piece.shadow, piece.site.elevation)),
                i32::from(model.vertices.iter().fold(32767, |n, v| n.min(v[1])) as i16),
                (part as u8, piece.site.draw, piece.site.ground),
            ));
        }
        if mesh.indices.is_empty() {
            return Ok(());
        }
        let chunk = if let Some(index) = reusable.or_else(|| {
            self.chunks
                .iter()
                .position(|c| c.mesh.vertices.is_empty() && c.mesh.indices.is_empty())
        }) {
            self.chunks[index] = Chunk::new(mesh, position, display_plane);
            index
        } else {
            let index = self.chunks.len();
            self.chunks.push(Chunk::new(mesh, position, display_plane));
            index
        };
        for (vertices, sprite, min_y, (piece, draw, ground)) in parts {
            if vertices.is_empty() {
                continue;
            }
            let mut shadow = None;
            if let Some((sprite, at, elevation)) = sprite {
                let position = self.shadows.position(at, elevation);
                if self.shadow_roofs || display_plane == 0 {
                    self.shadows.map.stamp(&sprite, position, false);
                }
                let caster = ShadowCaster {
                    sprite,
                    position,
                    plane: display_plane,
                    active: true,
                };
                if let Some(index) = self.static_shadows.iter().position(|c| !c.active) {
                    self.static_shadows[index] = caster;
                    shadow = Some(index);
                } else {
                    shadow = Some(self.static_shadows.len());
                    self.static_shadows.push(caster);
                }
            }
            let object = PickObject {
                target,
                chunk,
                vertices,
                piece,
                draw,
                ground,
                shadow,
                active: true,
                reusable: true,
                min_y,
            };
            if let Some(index) = self.pick_objects.iter().position(|o| !o.active) {
                self.pick_objects[index] = object;
            } else {
                self.pick_objects.push(object);
            }
        }
        Ok(())
    }
    fn ground_support(&self, plane: usize, tile: [i32; 2]) -> i32 {
        let covers = |target: rs09_client::interaction::LocationTarget| {
            let mut size = target.size;
            if target.rotation & 1 != 0 {
                size.swap(0, 1);
            }
            target.plane == plane
                && rs09_client::world::layer(target.shape) == Some(2)
                && tile[0] >= target.tile[0]
                && tile[0] < target.tile[0] + size[0]
                && tile[1] >= target.tile[1]
                && tile[1] < target.tile[1] + size[1]
                && self.location_types[&target.id].support_items == 1
        };
        let static_objects = self
            .pick_objects
            .iter()
            .filter(|o| o.active && covers(o.target))
            .map(|o| (true, o.min_y));
        let animated = self
            .animated_locations
            .iter()
            .filter(|o| covers(o.target))
            .map(|o| {
                let m = o.last_pose.as_ref().unwrap_or(&o.model);
                (
                    true,
                    i32::from(m.vertices.iter().fold(32767, |n, v| n.min(v[1])) as i16),
                )
            });
        rs09_client::ground_geometry::support_height(static_objects.chain(animated))
    }
    fn server_frame(
        &mut self,
        frame: &rs09_protocol::game::Frame,
        ui: Option<&mut rs09_client::ui::state::State>,
        cycle: i32,
    ) -> Result<bool, String> {
        let origin = self.navigation.origin;
        let world_handled = self
            .world_spots
            .receive(
                frame,
                rs09_client::world_spot::Context {
                    origin,
                    plane: self.ground.plane,
                    cycle,
                },
                &self.spot_sequences,
                &self.sequences,
                &mut |plane, fine| {
                    rs09_client::ground_geometry::tile_height(plane, fine, |p, x, y| {
                        let t = tile(&self.regions, p as usize, x + origin[0], y + origin[1]);
                        (t.height, t.flags)
                    })
                },
            )
            .map_err(|e| e.to_string())?;
        let mut ground = std::mem::take(&mut self.ground);
        let mut supports = std::mem::take(&mut self.ground_supports);
        let result = ground.apply_frame(
            frame,
            &mut ground_render::Context {
                ui,
                world: self,
                supports: &mut supports,
            },
        );
        self.ground = ground;
        self.ground_supports = supports;
        let handled = result.map_err(|e| e.to_string())?;
        // Ground State applies each envelope recursively once. Location children
        // use their separate reducer, so enclosed item updates are not doubled.
        Ok(self.location_frame(frame)? || handled || world_handled)
    }
    /// MAP_PROJANIM (104) and MAP_PROJANIM_3 (121), read on their own or as
    /// children of the zone envelope: launch from twice the current zone
    /// origin, moved by the launching actor's BAS slot where 104 names one.
    fn launch_projectile(
        &mut self,
        packet: &rs09_protocol::game::events::HalfTileProjectile,
        cycle: i32,
        from: Launchers<'_>,
    ) {
        let origin = self.navigation.origin;
        let context = rs09_client::world_spot::Context {
            origin,
            plane: self.ground.plane,
            cycle,
        };
        let (npc_bases, own_bas) = (&self.npc_bases, &self.bas);
        let launcher = |npc: bool, index: usize, slot: usize| {
            let (bas, angle) = if npc {
                let actor = from.npcs.actors.get(&index)?;
                (
                    &npc_bases.get(&actor.metadata.bas)?.movement,
                    actor.motion.angle,
                )
            } else if Some(index) == from.own {
                (own_bas, from.motion.angle)
            } else {
                (from.bas.get(&index)?, from.actors.get(&index)?.angle)
            };
            let transform = *bas.body_transforms.as_ref()?.get(slot)?.as_ref()?;
            Some((transform, angle))
        };
        let regions = &self.regions;
        let mut spots = std::mem::take(&mut self.world_spots);
        spots.launch(packet, context, launcher, &mut |plane, fine| {
            rs09_client::ground_geometry::tile_height(plane, fine, |p, x, y| {
                let t = tile(regions, p as usize, x + origin[0], y + origin[1]);
                (t.height, t.flags)
            })
        });
        self.world_spots = spots;
    }
    fn refresh_ground(
        &mut self,
        ui: Option<&mut rs09_client::ui::state::State>,
    ) -> Result<(), String> {
        let mut ground = std::mem::take(&mut self.ground);
        ground.clear_rendered();
        let mut supports = BTreeMap::new();
        let mut context = ground_render::Context {
            ui,
            world: self,
            supports: &mut supports,
        };
        let result = (|| {
            for tile in ground.occupied() {
                if tile.plane == ground.plane {
                    ground.refresh(tile.x, tile.y, &mut context)?;
                }
            }
            rs09_protocol::Result::Ok(())
        })();
        self.ground = ground;
        self.ground_supports = supports;
        result.map_err(|e| e.to_string())
    }
    fn location_frame(&mut self, frame: &rs09_protocol::game::Frame) -> Result<bool, String> {
        use rs09_protocol::game::updates::ServerUpdate;
        let Some(update) = ServerUpdate::decode(frame).map_err(|e| e.to_string())? else {
            return Ok(false);
        };
        let origin = self.navigation.origin;
        match update {
            ServerUpdate::ZoneOrigin { full, x, y } => {
                self.zone = [x, y];
                if full {
                    for c in &mut self.world.changes {
                        let t = [c.slot.tile[0] - origin[0], c.slot.tile[1] - origin[1]];
                        if c.slot.plane == self.ground.plane as usize
                            && t[0] >= x
                            && t[0] < x + 8
                            && t[1] >= y
                            && t[1] < y + 8
                        {
                            c.reset_loops = 0;
                        }
                    }
                }
            }
            ServerUpdate::ZoneEnclosed { x, y, frames } => {
                self.zone = [x, y];
                for frame in frames {
                    self.location_frame(&frame)?;
                }
            }
            ServerUpdate::LocAdd {
                x,
                y,
                shape,
                rotation,
                id,
            } => {
                self.ensure_location(id as u32)?;
                let layer =
                    rs09_client::world::layer(shape as u8).ok_or("Invalid location shape")?;
                self.world.push(
                    rs09_client::world::Slot {
                        plane: self.ground.plane as usize,
                        tile: [origin[0] + self.zone[0] + x, origin[1] + self.zone[1] + y],
                        layer,
                    },
                    rs09_client::world::Location {
                        id,
                        shape: shape as u8,
                        rotation: rotation as u8,
                    },
                    0,
                    -1,
                );
            }
            ServerUpdate::LocDel {
                x,
                y,
                shape,
                rotation,
            } => {
                let layer =
                    rs09_client::world::layer(shape as u8).ok_or("Invalid location shape")?;
                self.world.push(
                    rs09_client::world::Slot {
                        plane: self.ground.plane as usize,
                        tile: [origin[0] + self.zone[0] + x, origin[1] + self.zone[1] + y],
                        layer,
                    },
                    rs09_client::world::Location {
                        id: -1,
                        shape: shape as u8,
                        rotation: rotation as u8,
                    },
                    0,
                    -1,
                );
            }
            ServerUpdate::LocAnim {
                x,
                y,
                shape,
                sequence,
                ..
            } => {
                let layer =
                    rs09_client::world::layer(shape as u8).ok_or("Invalid animation shape")?;
                let slot = rs09_client::world::Slot {
                    plane: self.ground.plane as usize,
                    tile: [origin[0] + self.zone[0] + x, origin[1] + self.zone[1] + y],
                    layer,
                };
                if let Some(&loc) = self.world.locations.get(&slot) {
                    self.dynamic_location(slot, loc, Some(sequence))?;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    fn advance_world(&mut self) -> Result<(), String> {
        let changes = self.world.advance(self.navigation.origin, |loc| {
            self.definitions
                .get(&loc.id.to_string())
                .is_some_and(|d| d.models.iter().all(|id| self.models.contains_key(id)))
        });
        if changes.is_empty() {
            return Ok(());
        }
        self.map_revision = self.map_revision.wrapping_add(1);
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        self.audio_changes.extend(changes.iter().copied());
        for (slot, loc) in changes {
            self.dynamic_location(slot, loc, None)?;
        }
        self.placements = self
            .world
            .locations
            .iter()
            .filter_map(|(slot, loc)| {
                let d = self.clips.get(&(loc.id as u32)).copied().or_else(|| {
                    ClippingDefinition::decode(self.pack.get(&format!("loc/{}", loc.id)).ok()?).ok()
                })?;
                Some(Placement {
                    position: slot.tile,
                    plane: slot.plane,
                    shape: loc.shape,
                    rotation: loc.rotation,
                    size: d.size,
                    walk: d.walk,
                    projectiles: d.projectiles,
                })
            })
            .collect();
        self.navigation = Navigation::new(&self.regions, &self.placements, self.navigation.origin);
        Ok(())
    }
    fn spawn_npcs(&mut self, showcase: bool) {
        let spawns = if let Some(list) = option("--showcase-npcs").filter(|_| showcase) {
            // Capture aid: "id,x,y;id,x,y" replaces the showcase NPCs, for
            // views such as several actors standing on one tile.
            list.split(';')
                .filter_map(|entry| {
                    let v: Vec<i32> = entry.split(',').filter_map(|n| n.parse().ok()).collect();
                    (v.len() == 3 && self.npc_definitions.contains_key(&(v[0] as u32)))
                        .then(|| [v[0], v[1], v[2], 0, 0])
                })
                .collect()
        } else if showcase {
            vec![
                [0, 3221, 3214, 1, 0],
                [9, 3223, 3214, 1, 0],
                [100, 3225, 3214, 1, 0],
                [1, 3221, 3217, 1, 0],
                [4, 3223, 3217, 1, 0],
                [81, 3225, 3217, 1, 0],
                [41, 3221, 3220, 1, 0],
                [47, 3223, 3220, 1, 0],
            ]
        } else {
            self.npc_spawns.clone()
        };
        self.npcs = spawns
            .into_iter()
            .enumerate()
            .map(|(i, [id, x, z, walk, angle])| {
                let d = &self.npc_definitions[&(id as u32)];
                let bas = &self.npc_bases[&d.bas];
                let mut motion = Movement::new([x, z]);
                motion.fine = [x, z].map(|n| n * 128 + d.size * 64);
                motion.angle = angle;
                motion.target_angle = angle;
                motion.sequence = bas.movement.idle;
                motion.turn_speed = d.turn_speed;
                motion.rotates_on_move = d.rotates;
                NpcActor {
                    index: i,
                    animation: None,
                    id: id as u32,
                    render_id: d.children.is_none().then_some(id as u32),
                    motion,
                    home: [x, z],
                    walks: walk != 0,
                    phase: i as u64 * 23,
                    pick_triangles: Vec::new(),
                    model_height: 200,
                    terrain: Default::default(),
                }
            })
            .collect();
    }
    fn npc_metadata(&self, id: i32) -> rs09_protocol::Result<rs09_client::npc_updates::Metadata> {
        let missing = rs09_client::npc::Definition::decode(&[0])?;
        let d = match self.npc_definitions.get(&(id as u32)) {
            Some(d) => d,
            None if self.pack.get("npc-catalog-complete").is_ok() => &missing,
            None => {
                return Err(rs09_protocol::Error::new(
                    0,
                    "NPC definition absent from prepared catalog",
                ));
            }
        };
        let direct =
            |d: &rs09_client::npc::Definition| [0, 2, 3].into_iter().any(|i| d.sounds[i] != -1);
        let area_sound = d.children.as_ref().map_or_else(
            || direct(d),
            |ids| {
                ids.iter()
                    .filter_map(|id| self.npc_definitions.get(&(*id as u32)))
                    .any(direct)
            },
        );
        Ok(rs09_client::npc_updates::Metadata {
            size: d.size,
            bas: d.bas,
            turn_speed: d.turn_speed,
            rotates: d.rotates,
            area_sound,
        })
    }
    fn receive_npcs(
        &mut self,
        state: &mut rs09_client::npc_updates::State,
        frame: &rs09_protocol::game::Frame,
        input: NpcInput,
        sounds: &mut Vec<WorldSound>,
    ) -> Result<NpcUpdate, String> {
        let update = state
            .decode(
                frame,
                rs09_client::npc_updates::Context {
                    cycle: input.cycle,
                    self_tile: input.self_tile,
                    origin: input.origin,
                    sequences: &self.sequences,
                    spots: &self.spot_sequences,
                },
                |id| self.npc_metadata(id),
            )
            .map_err(|e| e.to_string())?;
        let mut diagnostics = update.diagnostics;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let ambient = update
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    rs09_client::npc_updates::Event::Type { .. }
                        | rs09_client::npc_updates::Event::AreaAdd { .. }
                        | rs09_client::npc_updates::Event::AreaRemove(_)
                )
            })
            .cloned()
            .collect();
        for &index in &update.removed {
            self.overheads.remove(&(true, index));
        }
        for event in update.events {
            match event {
                rs09_client::npc_updates::Event::Hit {
                    index,
                    damage,
                    kind,
                    cycle,
                } => {
                    self.overheads
                        .entry((true, index))
                        .or_default()
                        .hit(kind, cycle, damage);
                }
                rs09_client::npc_updates::Event::Sound { index, sound } => {
                    if let Some(npc) = state.actors.get(&index) {
                        sounds.push(WorldSound {
                            sequence: sound.sequence as u32,
                            frame: sound.frame,
                            fine: npc.motion.fine,
                            cycle: input.cycle.wrapping_sub(input.animation_origin),
                        });
                    }
                }
                rs09_client::npc_updates::Event::AreaAdd { .. }
                | rs09_client::npc_updates::Event::AreaRemove(_) => {
                    #[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
                    diagnostics.push("NPC ambient playback is currently Windows-only".into());
                }
                _ => {}
            }
        }
        for mask in update.masks {
            let index = mask[0] as usize;
            if let Some(npc) = state.actors.get(&index) {
                let visual = self.overheads.entry((true, index)).or_default();
                if mask[1] & 0x40 != 0 {
                    visual.bar = npc.hit_bar;
                    visual.bar_until = npc.hit_bar_until;
                }
                if mask[1] & 0x20 != 0 {
                    visual.chat = Some(rs09_client::overheads::Chat {
                        text: rs09_client::ui::latin1(&npc.chat),
                        color: 0,
                        effect: 0,
                        loops: 100,
                    });
                }
            }
        }
        for index in &state.local_ids {
            let id = state.actors[index].type_id as u32;
            let prepare = self
                .npc_definitions
                .get(&id)
                .ok_or_else(|| format!("NPC{id}: definition unavailable"))
                .map(|d| d.bas)
                .and_then(|bas| self.ensure_npc_bas(bas))
                .and_then(|()| {
                    if self.npc_definitions[&id].children.is_none() {
                        self.ensure_npc_body(id)
                    } else {
                        Ok(())
                    }
                });
            if let Err(error) = prepare {
                diagnostics.push(format!("NPC{id} body unavailable: {error}"));
            }
        }
        Ok(NpcUpdate {
            diagnostics,
            #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
            ambient,
        })
    }
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    fn audio_services<'a>(
        &'a self,
        ui: &'a rs09_client::ui::state::State,
        vm: &'a rs09_client::ui::vm::Vm,
        npcs: &'a rs09_client::npc_updates::State,
    ) -> ambient_audio::Services<'a> {
        ambient_audio::Services {
            ui,
            vm,
            npcs,
            definitions: &self.npc_definitions,
            location_types: &self.location_types,
            world_pack: &self.pack,
            plane: self.ground.plane,
        }
    }
    fn ensure_npc_body(&mut self, id: u32) -> Result<(), String> {
        if self.npc_types.contains_key(&id) {
            return Ok(());
        }
        let definition = self
            .npc_definitions
            .get(&id)
            .ok_or_else(|| format!("NPC{id}: no cached body definition"))?
            .clone();
        self.ensure_npc_bas(definition.bas)?;
        if let Some(cache) = &self.world_cache {
            for &id in &definition.models {
                if let std::collections::btree_map::Entry::Vacant(entry) = self.models.entry(id) {
                    if let Some(model) = cache.model(id)? {
                        entry.insert(model);
                    }
                }
            }
        }
        let mut model = definition.assemble_with_palette(
            &self.models,
            &self.npc_bases[&definition.bas].movement,
            &self.actor_scene.palette.colors,
        )?;
        model.draw_order = rs09_client::surface::face_order(&model, &self.textures.materials);
        model.surfaces = rs09_client::surface::prepare(&model, &self.textures.materials);
        self.npc_types.insert(id, NpcBody { definition, model });
        Ok(())
    }
    fn ensure_npc_bas(&mut self, id: i32) -> Result<(), String> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.npc_bases.entry(id) {
            if let Some(cache) = &self.world_cache {
                if let Some(bas) = cache.bas(id)? {
                    entry.insert(bas);
                    return Ok(());
                }
            }
            entry.insert(
                rs09_client::npc::Bas::decode(if id == -1 {
                    &[0]
                } else {
                    self.pack.get(&format!("bas/{id}"))?
                })
                .map_err(|e| e.to_string())?,
            );
        }
        Ok(())
    }

    fn ensure_location(&mut self, id: u32) -> Result<(), String> {
        let Some(cache) = &self.world_cache else {
            return Ok(());
        };
        let key = id.to_string();
        if !self.definitions.contains_key(&key) {
            let raw = cache
                .location(id)?
                .ok_or_else(|| format!("Location {id} unavailable"))?;
            let (loc, visual, clip) =
                rs09_client::interaction::LocationType::decode_with_visual(&raw)
                    .map_err(|e| format!("loc/{id}: {e}"))?;
            self.definitions.insert(key.clone(), visual);
            self.location_types.insert(id, loc);
            self.clips.insert(id, clip);
        }
        for &id in &self.definitions[&key].models {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.models.entry(id) {
                if let Some(model) = cache.model(id)? {
                    entry.insert(model);
                }
            }
        }
        Ok(())
    }
    fn npc_choice(
        &self,
        id: u32,
        vm: &rs09_client::ui::vm::Vm,
        ui: Option<&rs09_client::ui::state::State>,
    ) -> Result<Option<u32>, String> {
        let d = self
            .npc_definitions
            .get(&id)
            .ok_or_else(|| format!("NPC{id}: definition unavailable"))?;
        if d.children.is_none() {
            return Ok(Some(id));
        }
        let mut error = None;
        let result = d.select_morph(|bit, key| {
            if bit {
                match ui
                    .ok_or_else(|| "NPC morph requires varbit assets".to_string())
                    .and_then(|ui| vm.varbit(ui, key))
                {
                    Ok(value) => value,
                    Err(e) => {
                        error = Some(e);
                        0
                    }
                }
            } else {
                vm.varps.get(&key).copied().unwrap_or(0)
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        Ok(result)
    }
    fn npc_views(&self) -> Vec<serde_json::Value> {
        self.npcs
            .iter()
            .map(|npc| {
                serde_json::json!({
                    "index":npc.index,"type":npc.id,"render_type":npc.render_id,
                    "fine":npc.motion.fine,"sequence":npc.motion.sequence,"frame":npc.motion.frame,
                    "terrain":npc.terrain
                })
            })
            .collect()
    }
    fn update_npc_views(
        &mut self,
        vm: &rs09_client::ui::vm::Vm,
        ui: Option<&rs09_client::ui::state::State>,
    ) -> Vec<String> {
        let mut errors = Vec::new();
        for i in 0..self.npcs.len() {
            let result = (|| {
                let mut id = self.npcs[i].id;
                let mut seen = std::collections::BTreeSet::new();
                loop {
                    if !seen.insert(id) || seen.len() > 128 {
                        return Err(format!("NPC{id}: cyclic/deep morph graph"));
                    }
                    let Some(next) = self.npc_choice(id, vm, ui)? else {
                        return Ok(None);
                    };
                    if self.npc_definitions[&id].children.is_none() {
                        self.ensure_npc_body(id)?;
                        return Ok(Some(id));
                    }
                    id = next;
                }
            })();
            self.npcs[i].render_id = match result {
                Ok(id) => id,
                Err(e) => {
                    errors.push(e);
                    None
                }
            };
        }
        errors
    }
    fn advance_npcs(&mut self, tick: u64) -> Vec<WorldSound> {
        let mut sounds = vec![];
        for npc in &mut self.npcs {
            let d = &self.npc_definitions[&npc.id];
            let bas = &self.npc_bases[&d.bas];
            let size = d.size;
            // Controlled local wander commands exercise the real movement/animation
            // kernels. This is a visual preview, independent of server NPC AI.
            let phase = (tick + npc.phase) % 240;
            if npc.walks && npc.motion.queue.is_empty() && (phase == 1 || phase == 121) {
                let current = npc.motion.fine.map(|n| (n - size * 64).div_euclid(128));
                let destination = if phase == 1 {
                    [npc.home[0] + 1, npc.home[1]]
                } else {
                    npc.home
                };
                let dx = (destination[0] - current[0]).signum();
                let dz = (destination[1] - current[1]).signum();
                let origin = self.navigation.origin;
                let clear = (0..size).all(|x| {
                    (0..size).all(|z| {
                        walking::can_step(
                            &self.navigation.maps[0],
                            current[0] + x - origin[0],
                            current[1] + z - origin[1],
                            dx,
                            dz,
                        )
                    })
                });
                if clear {
                    npc.motion.enqueue(destination, 1);
                }
            }
            npc.motion.advance_sized(&bas.movement, size);
            if let Some(seq) = self.sequences.get(&(npc.motion.sequence as u32)) {
                for frame in npc.motion.animate(seq) {
                    sounds.push(WorldSound {
                        sequence: npc.motion.sequence as u32,
                        frame: frame as i32,
                        fine: npc.motion.fine,
                        cycle: tick as i32,
                    });
                }
            }
        }
        sounds
    }
    fn npc_meshes(
        &mut self,
        position: Vec2,
        tween: bool,
        spots: &BTreeMap<(bool, usize), Mesh>,
    ) -> (Vec<(usize, Mesh)>, Mesh) {
        let mut meshes = Vec::new();
        let mut shadows = self.textures.mesh();
        for npc in &mut self.npcs {
            npc.pick_triangles.clear();
            if self.actor_scene.thinned_npcs.contains(&npc.index) {
                continue;
            }
            let p = vec2(
                npc.motion.fine[0] as f32 / 128.0,
                npc.motion.fine[1] as f32 / 128.0,
            );
            if p.distance(position) > 55.0 {
                continue;
            }
            let Some(render_id) = npc.render_id else {
                continue;
            };
            let body = &self.npc_types[&render_id];
            let d = &self.npc_definitions[&npc.id];
            let bas = &self.npc_bases[&d.bas];
            let cursor = self
                .sequences
                .get(&(npc.motion.sequence as u32))
                .and_then(|s| {
                    s.delays.get(npc.motion.frame).map(|&n| {
                        (
                            npc.motion.frame,
                            npc.motion.next_frame,
                            npc.motion.progress - 1,
                            i32::from(n),
                        )
                    })
                });
            let animation = ActorPose {
                actor: (true, npc.index),
                sequence: npc.motion.sequence as u32,
                tick: 0,
                tween,
                cursor,
                state: npc.animation.as_ref(),
                idle: bas.movement.idle,
            };
            let (movement, action) =
                actor_poses(&self.sequences, &self.frames, &self.bases, animation);
            let slots = actor_slots(&self.sequences, &self.frames, &self.bases, animation);
            let mut pose = actor_model(
                &body.model,
                movement,
                action,
                npc.motion.sequence as u32,
                bas.movement.idle,
                &slots,
                &self.slot_origin_valid,
            );
            record_slot_pose(&self.slot_views, animation.actor, &pose, &slots);
            let scale = [
                body.definition.resize[0],
                body.definition.resize[1],
                body.definition.resize[0],
            ];
            for v in &mut pose.vertices {
                for axis in 0..3 {
                    v[axis] = v[axis].wrapping_mul(scale[axis]) >> 7;
                }
            }
            npc.model_height = pose.vertices.iter().map(|v| -v[1]).max().unwrap_or(200);
            // Npc.render captures bounds and builds its shadow before body terrain
            // transforms. The actor's parent BAS supplies alignment dimensions.
            let shadow = d.shadow.then(|| {
                rs09_client::shadow::character_styled(
                    &pose,
                    [54 * 128 + 64, 50 * 128 + 64],
                    npc.motion.angle,
                    npc.animation.as_ref().map_or_else(
                        || movement.is_some_and(|(s, _)| s.stretches),
                        |s| s.stretches,
                    ),
                    movement
                        .filter(|_| action.is_none() || npc.motion.sequence != bas.movement.idle)
                        .or(action)
                        .map(|(_, p)| (p.base, p.frame)),
                    rs09_client::shadow::CharacterStyle {
                        size: d.size,
                        colors: d.shadow_colors,
                        alpha: d.shadow_alpha,
                    },
                )
            });
            // Npc.render: applyBodyLean, then alignToTerrain.
            self.actor_scene.lean((true, npc.index)).apply(&mut pose);
            npc.terrain = actor_terrain(
                &self.regions,
                self.navigation.origin,
                self.ground.plane as usize,
                bas.terrain,
                npc.motion.angle,
                npc.motion.fine,
            );
            npc.terrain.apply(&mut pose);
            let angle = npc.motion.angle as f32 * std::f32::consts::TAU / 2048.0;
            let mut mesh = self.textures.mesh();
            let y = actor_height(
                &self.regions,
                self.navigation.origin,
                self.ground.plane as usize,
                p.x,
                p.y,
            );
            render::model(
                &mut mesh,
                &pose,
                &self.textures,
                |v| {
                    let x = v[0] as f32 / 128.0;
                    let z = v[2] as f32 / 128.0;
                    vec3(
                        p.x - 3200.0 + x * angle.cos() + z * angle.sin(),
                        y - v[1] as f32 / 128.0,
                        p.y - 3200.0 + z * angle.cos() - x * angle.sin(),
                    )
                },
                |n| {
                    vec3(
                        n[0] as f32 * angle.cos() + n[2] as f32 * angle.sin(),
                        -n[1] as f32,
                        n[2] as f32 * angle.cos() - n[0] as f32 * angle.sin(),
                    )
                },
            );
            // An actor is lit as scenery over the tiles it is filed under, on
            // the level it is drawn on.
            let (light_tile, light_size) = scene_lights::actor_area(npc.motion.fine, d.size);
            scene_lights::stamp(
                &mut mesh.vertices,
                self.point_lights.origin,
                rs09_client::light_select::Object::Area { size: light_size },
                light_tile,
                render_plane(&self.regions, self.ground.plane as usize, light_tile),
            );
            npc.pick_triangles.extend(
                mesh.vertices
                    .chunks_exact(3)
                    .map(|t| [t[0].position, t[1].position, t[2].position]),
            );
            if let Some(spot) = spots.get(&(true, npc.index)) {
                npc.pick_triangles.extend(
                    spot.vertices
                        .chunks_exact(3)
                        .map(|t| [t[0].position, t[1].position, t[2].position]),
                );
            }
            meshes.push((npc.index, mesh));
            if let Some(shadow) = shadow {
                render::model(
                    &mut shadows,
                    &shadow,
                    &self.textures,
                    |v| {
                        vec3(
                            p.x - 3200.0 + v[0] as f32 / 128.0,
                            actor_height(
                                &self.regions,
                                self.navigation.origin,
                                self.ground.plane as usize,
                                p.x + v[0] as f32 / 128.0,
                                p.y + v[2] as f32 / 128.0,
                            ) - v[1] as f32 / 128.0,
                            p.y - 3200.0 + v[2] as f32 / 128.0,
                        )
                    },
                    |n| vec3(n[0] as f32, -n[1] as f32, n[2] as f32),
                );
            }
        }
        (meshes, shadows)
    }
    fn prepare_locations(
        &mut self,
        tick: i32,
        tween: bool,
        position: Vec2,
        roofs: bool,
        sounds: &mut Vec<WorldSound>,
    ) -> Vec<Mesh> {
        self.tick = tick;
        if self.shadow_roofs != roofs {
            self.shadows.map = rs09_client::shadow::Map::new(
                (self.shadows.map.width - 2) / 128,
                (self.shadows.map.height - 2) / 128,
            );
            for caster in &self.static_shadows {
                if caster.active && (roofs || caster.plane == 0) {
                    self.shadows
                        .map
                        .stamp(&caster.sprite, caster.position, false);
                }
            }
            for loc in &mut self.animated_locations {
                loc.shadow = None;
                loc.shadow_state = Default::default();
            }
            self.shadow_roofs = roofs;
        }
        let mut meshes = Vec::new();
        let mut mesh = self.textures.mesh();
        // Each mesh is drawn on one depth layer (`roof_render::layer`).
        let mut layers = Vec::new();
        let mut layer = rs09_client::depth_layers::offset::NONE;
        for loc in &mut self.animated_locations {
            if self.roofs.hides(&loc.target)
                || self.roofs.culls(&loc.target, loc.piece)
                || loc.position.distance(position) >= 70.0
            {
                continue;
            }
            let own = match loc.target.shape {
                4..=8 => rs09_client::depth_layers::offset::WALL_DECOR,
                22 if self
                    .location_types
                    .get(&loc.target.id)
                    .is_some_and(|kind| kind.placement.flat) =>
                {
                    rs09_client::depth_layers::offset::FLAT_DECOR
                }
                _ => rs09_client::depth_layers::offset::NONE,
            };
            if own != layer {
                if !mesh.indices.is_empty() {
                    meshes.push(std::mem::replace(&mut mesh, self.textures.mesh()));
                    layers.push(layer);
                }
                layer = own;
            }
            let seq = &self.sequences[&loc.sequence];
            for frame in loc.clock.advance(seq, tick) {
                sounds.push(WorldSound {
                    sequence: loc.sequence,
                    frame: frame as i32,
                    fine: [
                        (loc.position.x * 128.0) as i32,
                        (loc.position.y * 128.0) as i32,
                    ],
                    cycle: tick,
                });
            }
            let mut pose = if let Some(frame) = loc
                .clock
                .active
                .then(|| self.frames.get(&seq.frames[loc.clock.frame]))
                .flatten()
            {
                let next = if tween || seq.tween {
                    loc.clock
                        .next_frame
                        .and_then(|i| self.frames.get(&seq.frames[i]))
                } else {
                    None
                };
                location::animate(
                    &loc.model,
                    loc.rotation,
                    seq,
                    &self.bases[&frame.base],
                    frame,
                    next,
                    loc.clock.progress,
                    i32::from(seq.delays[loc.clock.frame]),
                )
            } else {
                loc.model.clone()
            };
            // getAnimatedEntity: the diagonal turn, the shadow, then the terrain.
            loc.site.finish.turn(&mut pose);
            if loc.cast_shadow {
                let position = self.shadows.position(loc.position, loc.site.elevation);
                // Loc.getOrBuildEntity: rebuilt when `needsShadowUpdate` says
                // so (`self.shadows.mode` is the scenery-shadow preference),
                // the first one held back while its place is wholly in shade.
                scene_lights::animated_shadow(
                    &mut self.shadows,
                    &self.textures.materials,
                    &mut loc.shadow_state,
                    &mut loc.shadow,
                    &pose,
                    loc.target.id as i32,
                    loc.clock.active.then(|| {
                        [
                            loc.clock.frame as i32,
                            loc.clock.next_frame.map_or(-1, |frame| frame as i32),
                        ]
                    }),
                    seq.update_shadows || tween,
                    position,
                );
            }
            loc_align(&self.regions, loc.plane, &loc.site, &mut pose);
            if mesh.vertices.len() + pose.faces.len() * 3 > 60000 {
                meshes.push(mesh);
                layers.push(layer);
                mesh = self.textures.mesh();
            }
            let first_vertex = mesh.vertices.len();
            loc_mesh(
                &mut mesh,
                &pose,
                &self.textures,
                loc.site.draw,
                loc.site.ground,
                loc.plane,
            );
            let mut size = loc.target.size;
            if loc.target.rotation & 1 != 0 {
                size.swap(0, 1);
            }
            let draw_plane = render_plane(&self.regions, loc.plane, loc.target.tile);
            scene_lights::stamp(
                &mut mesh.vertices[first_vertex..],
                self.point_lights.origin,
                scene_lights::location_object(
                    i32::from(loc.target.shape),
                    i32::from(loc.target.rotation),
                    size,
                    draw_plane != loc.plane,
                    usize::from(loc.piece),
                ),
                loc.target.tile,
                draw_plane,
            );
            loc.pick_bounds = geometry_bounds(&mesh, first_vertex..mesh.vertices.len());
            loc.last_pose = Some(pose);
        }
        if !mesh.indices.is_empty() {
            meshes.push(mesh);
            layers.push(layer);
        }
        self.roofs.animated_layers = layers;
        self.shadows.upload();
        meshes
    }
    fn player_sequences(&mut self, pack: &Pack, bas: &PlayerBas) -> Result<(), String> {
        for id in bas.sequences() {
            self.player_sequence(pack, id)?;
        }
        Ok(())
    }
    fn player_sequence(&mut self, pack: &Pack, id: i32) -> Result<(), String> {
        if id == -1 {
            return Ok(());
        }
        let s = match self.sequences.get(&(id as u32)) {
            Some(s) => s.clone(),
            None => Sequence::decode(animation_asset(
                pack,
                self.animation_pack.as_ref(),
                &self.pack,
                &format!("seq/{id}"),
            )?)
            .map_err(|e| e.to_string())?,
        };
        for &id in &s.frames {
            if id == 65535 || self.frames.contains_key(&id) {
                continue;
            }
            let raw = animation_asset(
                pack,
                self.animation_pack.as_ref(),
                &self.pack,
                &format!("frame/{}/{}", id >> 16, id & 65535),
            )?;
            let bid = u16::from_be_bytes(
                raw.get(..2)
                    .ok_or("Player frame header")?
                    .try_into()
                    .unwrap(),
            );
            if let std::collections::btree_map::Entry::Vacant(e) = self.bases.entry(bid) {
                e.insert(
                    Base::decode(
                        bid,
                        animation_asset(
                            pack,
                            self.animation_pack.as_ref(),
                            &self.pack,
                            &format!("base/{bid}"),
                        )?,
                    )
                    .map_err(|e| e.to_string())?,
                );
            }
            self.frames.insert(
                id,
                Frame::decode(raw, &self.bases[&bid]).map_err(|e| e.to_string())?,
            );
        }
        self.sequences.insert(id as u32, s);
        Ok(())
    }
    fn spot_mesh(&mut self, actor: &SpotActor, tween: bool) -> Result<Option<Mesh>, String> {
        let track = &actor.track;
        if track.id == -1 || track.frame < 0 {
            return Ok(None);
        }
        if let std::collections::btree_map::Entry::Vacant(entry) = self.spot_models.entry(track.id)
        {
            let definition = match self.spot_definitions.get(&track.id) {
                Some(d) => d.clone(),
                None if self.pack.get("spot-catalog-complete").is_ok() => {
                    rs09_client::spot::Definition::default()
                }
                None => return Err(format!("Spot{} metadata unavailable", track.id)),
            };
            let mut model = definition.assemble(&self.models)?;
            model.draw_order = rs09_client::surface::face_order(&model, &self.textures.materials);
            model.surfaces = rs09_client::surface::prepare(&model, &self.textures.materials);
            entry.insert(SpotBody { definition, model });
        }
        let body = &self.spot_models[&track.id];
        let animation = if body.definition.sequence == -1 {
            None
        } else {
            let sequence = self
                .sequences
                .get(&(body.definition.sequence as u32))
                .ok_or_else(|| {
                    format!(
                        "Spot{} sequence{} unavailable",
                        track.id, body.definition.sequence
                    )
                })?;
            let (frame, next, delta, length) = track
                .cursor(sequence)
                .ok_or_else(|| format!("Spot{} frame{} unavailable", track.id, track.frame))?;
            let first = self
                .frames
                .get(&sequence.frames[frame])
                .ok_or_else(|| format!("Spot{} first frame unavailable", track.id))?;
            Some((
                sequence,
                Pose {
                    base: self
                        .bases
                        .get(&first.base)
                        .ok_or("Spot animation base unavailable")?,
                    frame: first,
                    next: if tween || sequence.tween {
                        next.and_then(|n| sequence.frames.get(n))
                            .and_then(|id| self.frames.get(id))
                    } else {
                        None
                    },
                    delta,
                    length,
                },
            ))
        };
        let mut pose = body.definition.pose(&body.model, animation);
        let origin = self.navigation.origin;
        let sample = |fine: [i32; 2]| {
            rs09_client::ground_geometry::tile_height(
                self.ground.plane,
                [fine[0] - origin[0] * 128, fine[1] - origin[1] * 128],
                |plane, x, y| {
                    let t = tile(&self.regions, plane as usize, x + origin[0], y + origin[1]);
                    (t.height, t.flags)
                },
            )
        };
        let alignment = if actor.world.is_none() && body.definition.align_to_terrain {
            rs09_client::spot::Alignment::sample(
                actor.terrain,
                actor.yaw,
                actor.fine,
                sample(actor.fine),
                sample,
            )
        } else {
            rs09_client::spot::Alignment::default()
        };
        if actor.world.is_none() {
            body.definition.attach(&mut pose, actor.height, alignment);
        }
        if let Some(pitch) = actor.pitch {
            // ProjAnim.getModel: `model.rotateX(this.pitch)`.
            rs09_client::spot::Alignment {
                pitch,
                roll: 0,
                offset: 0,
            }
            .apply(&mut pose);
        }
        let position = actor.fine.map(|n| n as f32 / 128.0);
        let heading = actor.yaw as f32 * std::f32::consts::TAU / 2048.0;
        let y = if let Some((_, z)) = actor.world {
            -(z as f32) / 128.0
        } else {
            actor_height(
                &self.regions,
                self.navigation.origin,
                self.ground.plane as usize,
                position[0],
                position[1],
            )
        };
        let mut mesh = self.textures.mesh();
        render::model(
            &mut mesh,
            &pose,
            &self.textures,
            |v| {
                let x = v[0] as f32 / 128.0;
                let y_model = v[1] as f32 / 128.0;
                let z = v[2] as f32 / 128.0;
                vec3(
                    position[0] - 3200.0 + x * heading.cos() + z * heading.sin(),
                    y - y_model,
                    position[1] - 3200.0 + z * heading.cos() - x * heading.sin(),
                )
            },
            |n| {
                let (ny, nz) = (n[1] as f32, n[2] as f32);
                vec3(
                    n[0] as f32 * heading.cos() + nz * heading.sin(),
                    -ny,
                    nz * heading.cos() - n[0] as f32 * heading.sin(),
                )
            },
        );
        let checksum = pose
            .vertices
            .iter()
            .flatten()
            .fold(0xcbf29ce484222325u64, |h, &v| {
                (h ^ v as u32 as u64).wrapping_mul(0x100000001b3)
            });
        self.spot_views.push(serde_json::json!({"npc":actor.npc,"index":actor.index,"id":track.id,"sequence":body.definition.sequence,"frame":track.frame,"next":track.next,"clock":track.clock,"height":actor.height,"fine":actor.fine,"yaw":actor.yaw,"vertices":pose.vertices.len(),"triangles":mesh.indices.len()/3,"vertex_checksum":format!("{checksum:016x}"),"terrain":alignment}));
        if let Some((plane, z)) = actor.world {
            let view = self.spot_views.last_mut().unwrap();
            if actor.pitch.is_some() {
                view["projectile"] = true.into();
            }
            view["world"] = serde_json::json!(true);
            view["plane"] = serde_json::json!(plane);
            view["z"] = serde_json::json!(z);
            for vertex in &mut mesh.vertices {
                vertex.normal.w += plane as f32 * 2048.0;
            }
        }
        Ok(Some(mesh))
    }
    fn avatar_mesh(
        &self,
        models: &[Model],
        animation: ActorPose<'_>,
        position: Vec2,
        heading: f32,
        form: rs09_client::player_form::Form,
        terrain_dimensions: [i32; 2],
    ) -> (Mesh, Mesh, i32) {
        let sequence = animation.sequence;
        let idle = animation.idle;
        let slots = actor_slots(&self.sequences, &self.frames, &self.bases, animation);
        let (movement_pose, action_pose) =
            actor_poses(&self.sequences, &self.frames, &self.bases, animation);
        let mut mesh = self.textures.mesh();
        let mut shadow_mesh = self.textures.mesh();
        let mut model_height = None;
        let terrain = actor_terrain(
            &self.regions,
            self.navigation.origin,
            self.ground.plane as usize,
            terrain_dimensions,
            ((heading / std::f32::consts::TAU) * 2048.0).round() as i32 & 2047,
            [(position.x * 128.0) as i32, (position.y * 128.0) as i32],
        );
        for model in models {
            let mut pose = actor_model(
                model,
                movement_pose,
                action_pose,
                sequence,
                idle,
                &slots,
                &self.slot_origin_valid,
            );
            record_slot_pose(&self.slot_views, animation.actor, &pose, &slots);
            form.resize_model(&mut pose);
            if let Some(top) = pose.vertices.iter().map(|v| -v[1]).max() {
                model_height = Some(model_height.map_or(top, |n: i32| n.max(top)));
            }
            let y = actor_height(
                &self.regions,
                self.navigation.origin,
                self.ground.plane as usize,
                position.x,
                position.y,
            );
            // Player.render: the shadow follows the movement sequence unless
            // that is absent or is the idle one under an action sequence
            // (`movementSeq == null ? seq : movementSeq`), and stretches with
            // the entity's `seqStretches`.
            let shadow_pose = movement_pose
                .filter(|_| action_pose.is_none() || i64::from(sequence) != i64::from(idle))
                .or(action_pose);
            let shadow_animation = shadow_pose.map(|(_, p)| (p.base, p.frame));
            let shadow = rs09_client::shadow::character(
                &pose,
                // ShadowModelList caches the ring mesh on the first player draw.
                // Initial fine coordinates in the workbench's 104-tile window.
                [54 * 128 + 64, 50 * 128 + 64],
                ((heading / std::f32::consts::TAU) * 2048.0).round() as i32 & 2047,
                animation.state.map_or_else(
                    || shadow_pose.is_some_and(|(seq, _)| seq.stretches),
                    |s| s.stretches,
                ),
                shadow_animation,
            );
            if form.shadow {
                render::model(
                    &mut shadow_mesh,
                    &shadow,
                    &self.textures,
                    |v| {
                        vec3(
                            position.x - 3200.0 + v[0] as f32 / 128.0,
                            actor_height(
                                &self.regions,
                                self.navigation.origin,
                                self.ground.plane as usize,
                                position.x + v[0] as f32 / 128.0,
                                position.y + v[2] as f32 / 128.0,
                            ) - v[1] as f32 / 128.0,
                            position.y - 3200.0 + v[2] as f32 / 128.0,
                        )
                    },
                    |n| vec3(n[0] as f32, -n[1] as f32, n[2] as f32),
                );
            }
            if animation.actor == (false, rs09_protocol::game::players::SELF) {
                self.actor_scene
                    .own_extent
                    .set(Some(rs09_client::hint_arrow::body_extent(&pose)));
            }
            // Player.render: applyBodyLean, then alignToTerrain.
            self.actor_scene.lean(animation.actor).apply(&mut pose);
            terrain.apply(&mut pose);
            render::model(
                &mut mesh,
                &pose,
                &self.textures,
                |v| {
                    let x = v[0] as f32 / 128.0;
                    let z = v[2] as f32 / 128.0;
                    vec3(
                        position.x - 3200.0 + x * heading.cos() + z * heading.sin(),
                        y - v[1] as f32 / 128.0,
                        position.y - 3200.0 + z * heading.cos() - x * heading.sin(),
                    )
                },
                |n| {
                    vec3(
                        n[0] as f32 * heading.cos() + n[2] as f32 * heading.sin(),
                        -n[1] as f32,
                        n[2] as f32 * heading.cos() - n[0] as f32 * heading.sin(),
                    )
                },
            );
        }
        // An actor is lit as scenery over the tiles it is filed under, on the
        // level it is drawn on. A player's size is taken as one tile.
        let (light_tile, light_size) = scene_lights::actor_area(
            [(position.x * 128.0) as i32, (position.y * 128.0) as i32],
            1,
        );
        scene_lights::stamp(
            &mut mesh.vertices,
            self.point_lights.origin,
            rs09_client::light_select::Object::Area { size: light_size },
            light_tile,
            render_plane(&self.regions, self.ground.plane as usize, light_tile),
        );
        (mesh, shadow_mesh, model_height.unwrap_or(200))
    }
}
fn configuration() -> macroquad::conf::Conf {
    // A live client opens the frame `GameShell.startApplication` does and
    // multisamples as its preferences say; the offline workbench keeps its
    // own window and four samples, so its captures do not change.
    let live = option("--connect").is_some();
    // `--window WIDTHxHEIGHT`, for instance 765x503, the original's fixed size.
    let (window_width, window_height) = option("--window")
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse().ok()?, height.parse().ok()?))
        })
        .unwrap_or(if live {
            let [width, height] = rs09_client::display::APPLICATION_FRAME;
            (width, height)
        } else {
            (1280, 800)
        });
    // Capture runs read no preferences, live or not.
    let sample_count = if live && !std::env::args().any(|arg| arg.starts_with("--capture")) {
        saved_samples()
    } else {
        4
    };
    WINDOW_SAMPLES.store(sample_count, std::sync::atomic::Ordering::Relaxed);
    macroquad::conf::Conf {
        miniquad_conf: Conf {
            window_title: "2009scape Rust | Visual workbench".into(),
            window_width,
            window_height,
            high_dpi: true,
            sample_count,
            ..Conf::default()
        },
        draw_call_vertex_capacity: 65536,
        draw_call_index_capacity: 65536,
        ..Default::default()
    }
}
/// The sample count the window was made with.
static WINDOW_SAMPLES: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
/// The window's sample count from the saved anti-aliasing preference, read
/// before the window exists because miniquad cannot change it afterwards
/// (see `display_host::samples`). `client.mainLoad` zeroes the preference
/// when the last start left safe mode set (client.java:1674-1679).
#[cfg(not(target_arch = "wasm32"))]
fn saved_samples() -> i32 {
    use rs09_client::preferences_file::{MAX_MEMORY, Store};
    let preferences = Store::locate(
        option("--preferences").map(std::path::PathBuf::from),
        std::env::var_os("APPDATA"),
    )
    .map(|store| store.load(MAX_MEMORY).0);
    display_host::samples(
        preferences
            .filter(|preferences| !preferences.safe_mode)
            .map_or(0, |preferences| preferences.anti_aliasing),
    )
}
/// The browser's drawing buffer is made by the page.
#[cfg(target_arch = "wasm32")]
fn saved_samples() -> i32 {
    4
}
/// The starting scene from the cache alone (`--cache DIR`, else
/// `assets/cache`), for when no prepared pack exists. Region keys come from
/// `--world-keys`; a region whose key is not there keeps its terrain and loses
/// its locations until the server's first rebuild streams the real scene.
/// `--start X,Y` chooses the tile.
#[cfg(not(target_arch = "wasm32"))]
fn native_world_pack() -> Result<Pack, String> {
    use rs09_client::world_pack::{Inputs, Options, world_pack};
    let directory = option("--cache").unwrap_or_else(|| "assets/cache".into());
    let commit = std::fs::read("upstream.lock.json")
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|lock| lock["server"]["commit"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "unrecorded".into());
    let mut inputs = Inputs::default();
    let keys = option("--world-keys").unwrap_or_else(|| "runs/player-session/xteas.json".into());
    if let Ok(bytes) = std::fs::read(&keys) {
        inputs.keys = Inputs::parse_keys(&bytes).map_err(|e| format!("{keys}: {e}"))?;
    }
    let mut options = Options {
        lenient: true,
        ..Options::default()
    };
    if let Some(start) = option("--start") {
        let (x, y) = start.split_once(',').ok_or("--start takes X,Y")?;
        options.center = [
            x.parse().map_err(|_| "--start takes X,Y")?,
            y.parse().map_err(|_| "--start takes X,Y")?,
        ];
        options.label = format!("{x},{y}");
    }
    let built = world_pack(directory.as_ref(), &inputs, &options, &commit)?;
    for note in &built.notes {
        eprintln!("Starting scene: {note}");
    }
    Pack::decode(built.bytes)
}
fn option(name: &str) -> Option<String> {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == name {
            return args.next();
        }
    }
    None
}
#[derive(Default)]
struct ActionOutbox {
    history: Vec<rs09_protocol::game::Frame>,
    pending: VecDeque<rs09_protocol::game::Frame>,
    live: bool,
}
impl ActionOutbox {
    fn push(&mut self, frame: rs09_protocol::game::Frame) {
        if self.live {
            self.pending.push_back(frame.clone());
        }
        self.history.push(frame);
    }
}
impl std::ops::Deref for ActionOutbox {
    type Target = Vec<rs09_protocol::game::Frame>;
    fn deref(&self) -> &Self::Target {
        &self.history
    }
}
impl std::ops::DerefMut for ActionOutbox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.history
    }
}
fn queue_packet(outbox: &mut ActionOutbox, bytes: &[u8]) {
    let mut decoder = rs09_protocol::game::Decoder::new(
        rs09_protocol::game::Direction::ClientToServer,
        65536,
        65535,
    );
    decoder.push(bytes).expect("encoded client packet");
    if let Some(frame) = decoder.next_frame().expect("encoded client frame") {
        if outbox.len() >= 512 {
            outbox.remove(0);
        }
        outbox.push(frame);
    }
}

#[derive(Clone, Copy)]
enum ClickKind {
    Walk,
    Interact,
}

struct ClickCross {
    at: [i32; 2],
    started: f64,
    kind: ClickKind,
}

/// `Mouse.clickX` and `Mouse.clickY`, where the cross is drawn (`Cross.x =
/// Mouse.clickX`): the last click a cycle latched.
static CLICK_POINT: [std::sync::atomic::AtomicI32; 2] = [
    std::sync::atomic::AtomicI32::new(0),
    std::sync::atomic::AtomicI32::new(0),
];

impl ClickCross {
    /// Notes the click the cycle's `Mouse.loop` latched.
    fn clicked(point: [i32; 2]) {
        for (slot, value) in CLICK_POINT.iter().zip(point) {
            slot.store(value, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn new(kind: ClickKind) -> Self {
        let [x, y] =
            [0, 1].map(|axis| CLICK_POINT[axis].load(std::sync::atomic::Ordering::Relaxed));
        Self {
            at: [x - 8, y - 8],
            started: get_time(),
            kind,
        }
    }

    fn frame(&self, now: f64) -> Option<usize> {
        // The original viewport cycles through four frames, one per 100 ms.
        let frame = ((now - self.started) * 10.0) as usize;
        (frame < 4).then(|| {
            frame
                + match self.kind {
                    ClickKind::Walk => 0,
                    ClickKind::Interact => 4,
                }
        })
    }
}

#[cfg(test)]
#[test]
fn click_cross_uses_four_frames_for_each_action_and_expires() {
    for (kind, first) in [(ClickKind::Walk, 0), (ClickKind::Interact, 4)] {
        let cross = ClickCross {
            at: [12, 20],
            started: 0.0,
            kind,
        };
        for (index, time) in [0.0, 0.15, 0.25, 0.35].into_iter().enumerate() {
            assert_eq!(cross.frame(time), Some(first + index));
        }
        assert_eq!(cross.frame(0.4), None);
    }
}

#[macroquad::main(configuration)]
async fn main() {
    let launch = rs09_client::launch::read().expect("live launch configuration");
    let live_mode = launch.is_some();
    let capture_realtime = std::env::args().any(|arg| arg == "--capture-realtime");
    let capture_npcs = std::env::args().any(|arg| arg == "--capture-npcs");
    let capture_animation = std::env::args().any(|arg| arg == "--capture-animation");
    let capture_models = std::env::args().any(|arg| arg == "--capture-ui-models");
    let capture = capture_npcs
        || capture_animation
        || capture_models
        || std::env::args().any(|arg| arg == "--capture");
    #[cfg(not(target_arch = "wasm32"))]
    if capture {
        std::fs::create_dir_all("runs/visual").expect("capture output directory");
    }
    let capture_frame = option("--capture-frame")
        .and_then(|n| n.parse::<i32>().ok())
        .filter(|&n| n > 0);
    let portrait = std::env::args().any(|arg| arg == "--portrait");
    let capture_route = std::env::args().any(|arg| arg == "--capture-route");
    let capture_route_run = std::env::args().any(|arg| arg == "--capture-route-run");
    let frustum_culling = !std::env::args().any(|arg| arg == "--no-frustum-culling");
    let capture_route_target = option("--capture-route-target")
        .map(|s| {
            let (x, y) = s.split_once(',').expect("capture target requires x,y");
            [
                x.parse::<i32>().expect("capture target x"),
                y.parse::<i32>().expect("capture target y"),
            ]
        })
        .unwrap_or([3227, 3218]);
    let capture_route_issued = false;
    #[cfg(not(target_arch = "wasm32"))]
    let capture_travel = option("--capture-travel").map(|path| {
        assert!(
            capture && live_mode,
            "--capture-travel requires a live capture"
        );
        world_cache::TravelCapture::open(path.as_ref()).expect("travel capture route")
    });
    clear_background(BLACK);
    draw_text(
        "Loading revision 530 cache assets...",
        40.0,
        60.0,
        28.0,
        WHITE,
    );
    next_frame().await;
    // Fetch or repair the cache from the game server first, when asked to
    // or when there is none (see cache_update_screen).
    #[cfg(not(target_arch = "wasm32"))]
    cache_update_screen::run_if_requested().await;
    // The browser build downloads it into memory when the page names an
    // update server, and then reads it as the desktop build reads `--cache`.
    #[cfg(target_arch = "wasm32")]
    cache_host::start().await;
    // The graphics preferences the starting scene and the textures are built
    // with (scene_detail): the saved ones, which a capture run does not
    // read, under the launch switches.
    let detail = scene_detail::Detail::from_arguments(std::env::args());
    {
        #[cfg(not(target_arch = "wasm32"))]
        let saved = (!capture)
            .then(|| {
                rs09_client::preferences_file::Store::locate(
                    option("--preferences").map(std::path::PathBuf::from),
                    std::env::var_os("APPDATA"),
                )
            })
            .flatten()
            .map(|store| store.load(rs09_client::preferences_file::MAX_MEMORY).0);
        #[cfg(target_arch = "wasm32")]
        let saved = None::<rs09_client::ui::vm::Preferences>;
        let brightness = match &saved {
            Some(saved) => saved.brightness,
            None => detail.brightness().unwrap_or(3),
        };
        detail.apply(&saved.unwrap_or_default(), brightness, None);
    }
    let pack_path = option("--pack").unwrap_or_else(|| "assets/visual/demo.pack".into());
    // Set when the starting scene was built from the cache: map streaming is
    // then on by default, since no prepared pack covers anywhere else.
    let mut scene_from_cache = false;
    #[cfg(not(target_arch = "wasm32"))]
    let prepared = load_file(&pack_path).await.map_err(|e| e.to_string());
    #[cfg(target_arch = "wasm32")]
    let prepared = cache_host::prepared(&pack_path).await;
    let loaded = match prepared {
        Ok(bytes) => Pack::decode(bytes).and_then(Workbench::load),
        // Without a prepared pack the starting scene is built from the cache.
        #[cfg(not(target_arch = "wasm32"))]
        Err(e) => native_world_pack()
            .map_err(|native| {
                format!("{pack_path}: {e}\nBuilding the starting scene from the cache: {native}")
            })
            .and_then(Workbench::load)
            .inspect(|_| scene_from_cache = true),
        #[cfg(target_arch = "wasm32")]
        Err(e) => cache_host::world_pack()
            .await
            .map_err(|cache| {
                format!("{pack_path}: {e}\nBuilding the starting scene from the cache: {cache}")
            })
            .and_then(Workbench::load)
            .inspect(|_| scene_from_cache = true),
    };
    let mut workbench = match loaded {
        Ok(w) => w,
        Err(error) => {
            eprintln!("{error}");
            assert!(!capture, "capture asset load failed: {error}");
            loop {
                clear_background(Color::from_hex(0x1b1d23));
                draw_multiline_text(
                    format!("Asset load failed\n{error}"),
                    30.0,
                    60.0,
                    22.0,
                    Some(1.5),
                    WHITE,
                );
                if is_key_pressed(KeyCode::Escape) {
                    return;
                }
                next_frame().await;
            }
        }
    };
    let npc_replay =
        if let Some(path) = rs09_client::launch::npc_replay_path().expect("NPC preview path") {
            assert!(
                !live_mode,
                "NPC replay cannot be combined with a live world"
            );
            let input: NpcReplay =
                serde_json::from_slice(&load_file(&path).await.expect("NPC packet fixture"))
                    .expect("NPC fixture JSON");
            assert!(
                input
                    .frames
                    .iter()
                    .all(|f| matches!(f.opcode, 17 | 26 | 32 | 37 | 56 | 84 | 112 | 172 | 230)),
                "unsupported NPC fixture opcode"
            );
            assert!(
                input
                    .frames
                    .iter()
                    .zip(input.frames.iter().skip(1))
                    .all(|(a, b)| a.tick <= b.tick),
                "NPC fixture is not ordered"
            );
            workbench.start = input.start;
            Some(input)
        } else {
            None
        };
    let npc_packet_mode = live_mode || npc_replay.is_some();
    workbench.live_origin = live_mode;
    if live_mode {
        workbench.avatar.clear();
    }
    let mut replay = VecDeque::new();
    let ui_pack_path =
        option("--ui-pack").unwrap_or_else(|| "assets/visual/ui-players.pack".into());
    // `--cache DIR` reads interfaces, definitions, models, animations and
    // sounds straight from the JS5 cache on demand; no prepared UI, model or
    // animation pack is needed. Without it the prepared packs are used, and the
    // default cache is the fallback when the UI pack is absent.
    #[cfg(not(target_arch = "wasm32"))]
    let cache_ui = {
        let open = |directory: &str| {
            let commit = std::fs::read("upstream.lock.json")
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|lock| lock["server"]["commit"].as_str().map(str::to_owned))
                .unwrap_or_else(|| "unrecorded".into());
            rs09_client::cache_pack::cache_pack(directory.as_ref(), &commit)
        };
        match option("--cache") {
            Some(directory) => Some(open(&directory).unwrap_or_else(|error| {
                panic!("Open cache {directory}: {error}");
            })),
            None if option("--ui-pack").is_none()
                && !std::path::Path::new(&ui_pack_path).exists() =>
            {
                open("assets/cache").ok()
            }
            None => None,
        }
    };
    #[cfg(target_arch = "wasm32")]
    let cache_ui = cache_host::ui_pack().await;
    let from_cache = cache_ui.is_some();
    let mut ui = match cache_ui {
        Some(pack) => Some(rs09_client::ui::state::State::new(pack)),
        None => match load_file(&ui_pack_path).await {
            Ok(bytes) => Some(rs09_client::ui::state::State::new(
                Pack::decode(bytes).expect("UI pack"),
            )),
            Err(error) => {
                eprintln!(
                    "UI assets unavailable: {error}; pass --cache DIR to read the game cache"
                );
                None
            }
        },
    };
    // The cache pack already lists every sequence, frame and base.
    workbench.animation_pack = if from_cache {
        None
    } else {
        match load_file("assets/visual/animations.pack").await {
            Ok(bytes) => Some(Pack::decode(bytes).expect("animation archive pack")),
            Err(error) => {
                eprintln!(
                    "Complete animation assets unavailable: {error}; using available prepared UI/world subsets. Pass --cache DIR to read the game cache"
                );
                None
            }
        }
    };
    // Priority/replay metadata is available before packet requests. Full frame
    // dependencies remain lazy; authored UI/world definitions keep precedence.
    for pack in ui
        .as_ref()
        .map(|u| &u.pack)
        .into_iter()
        .chain(workbench.animation_pack.as_ref())
    {
        for name in pack.manifest.files.keys() {
            if let Some(id) = name
                .strip_prefix("seq/")
                .and_then(|s| s.parse::<u32>().ok())
            {
                if let std::collections::btree_map::Entry::Vacant(e) = workbench.sequences.entry(id)
                {
                    e.insert(
                        Sequence::decode(pack.get(name).expect("sequence catalog entry"))
                            .expect("sequence catalog decode"),
                    );
                }
            }
        }
    }
    let ui_renderer = ui_render::Renderer::default();
    let mut ui_models =
        ui_model_render::Renderer::new(&workbench.textures).expect("UI model shader");
    let ground_renderer = ground_render::Renderer::default();
    let mut world_picker = world_pick::Picker::default();
    // A comparison aid: draw and pick as if the scene had no occluders.
    world_picker.occlusion_off = std::env::args().any(|arg| arg == "--no-occlusion");
    // The minimap reads its sprite names, map scenes and floors from the
    // cache when one is open (`minimap::source`).
    let minimap = minimap_render::Renderer::new(
        &workbench,
        ui.as_ref().filter(|_| from_cache).map(|ui| &ui.pack),
        if capture {
            0
        } else {
            (miniquad::date::now() * 1000.0) as i64
        },
    )
    .map_err(|error| eprintln!("Minimap unavailable: {error}"))
    .ok();
    // The world map list (the original's main load state 130) is read from
    // the cache; the browser build has none and keeps the map empty.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(ui) = &mut ui {
        world_map_render::install(ui, minimap.as_ref(), capture);
    }
    // client.resetGameSessionState's random camera and minimap offsets, then
    // Protocol's per-cycle drift. The original draws from Math.random().
    let mut drift_random = rs09_game_core::random::JavaRandom::new(if capture {
        0
    } else {
        (miniquad::date::now() * 1000.0) as i64 ^ 0x5DEE_CE66D
    });
    let mut drift = rs09_client::minimap::Drift::default();
    drift.reset(|| drift_random.next_double());
    let mut ui_audio = ui_audio::Effects::default();
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    ui_audio.record_pcm(capture || rs09_client::launch::audio_review());
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    let ambient_audio = ambient_audio::Locations::new(
        &workbench.pack,
        if capture {
            0
        } else {
            (get_time() * 1_000_000.0) as i64
        },
    )
    .expect("location audio metadata");
    let mut ui_vm = rs09_client::ui::vm::Vm::default();
    // `--script-cursor ID` stands in for CS2 5426, which no script in the
    // pinned cache runs, so the cursor presentation can be looked at.
    if let Some(cursor) = option("--script-cursor").and_then(|id| id.parse().ok()) {
        ui_vm.script_cursor = rs09_client::cursor::Id(cursor);
    }
    // The browser has no command line: the page's `?cursor=ID` is that aid.
    #[cfg(target_arch = "wasm32")]
    if let Ok(cursor) = cache_host::launch_value(7).parse() {
        ui_vm.script_cursor = rs09_client::cursor::Id(cursor);
    }
    // Saved preferences: the original's binary file, at `--preferences PATH`
    // or in the per-user settings folder, with this client's earlier JSON
    // file read when there is no binary one yet. Capture runs neither read
    // nor write them, so they stay repeatable.
    #[cfg(not(target_arch = "wasm32"))]
    let preferences_store = if capture {
        None
    } else {
        rs09_client::preferences_file::Store::locate(
            option("--preferences").map(std::path::PathBuf::from),
            std::env::var_os("APPDATA"),
        )
    };
    #[cfg(not(target_arch = "wasm32"))]
    let preferences_loaded = preferences_store.as_ref().is_some_and(|store| {
        use rs09_client::preferences_file::{MAX_MEMORY, Source, main_load};
        let (preferences, source, warning) = store.load(MAX_MEMORY);
        if let Some(warning) = warning {
            eprintln!("Saved preferences ignored: {warning}");
        }
        ui_vm.preferences = preferences;
        // client.mainLoad: safe mode is set and written, and stays in the
        // file unless this run gets as far as drawing an interface.
        main_load(&mut ui_vm.preferences);
        if let Err(error) = store.save(&ui_vm.preferences) {
            eprintln!(
                "Preferences not saved to {}: {error}",
                store.binary.display()
            );
        }
        source != Source::Defaults
    });
    #[cfg(target_arch = "wasm32")]
    let preferences_loaded = false;
    // Music needs the cache's song, instrument and sample archives and a PCM
    // device. Capture runs stay silent so they remain repeatable.
    #[cfg(target_os = "windows")]
    let music = if capture || std::env::args().any(|arg| arg == "--no-music") {
        None
    } else {
        let directory = option("--cache").unwrap_or_else(|| "assets/cache".into());
        rs09_client::music::CacheSource::open(directory.as_ref())
            .and_then(|source| {
                let music = rs09_client::music::Music::new(
                    Box::new(source),
                    ui_vm.preferences.music_volume,
                );
                ui_audio.music(music.device()).map(|()| music)
            })
            .map_err(|error| eprintln!("Music unavailable: {error}"))
            .ok()
    };
    // In the browser the same player reads the downloaded cache and hands
    // its blocks to the page's audio thread.
    #[cfg(target_arch = "wasm32")]
    let music = cache_host::music(&mut ui_audio, ui_vm.preferences.music_volume);
    // None until the first frame has applied the starting preferences.
    let preferences_applied = None::<u32>;
    // Brightness, scenery shadows on and character shadows as last seen.
    let preferences_shown = None::<(i32, bool, bool)>;
    if let Some(ui) = &ui {
        let ids: BTreeMap<String, i32> = serde_json::from_slice(
            ui.pack
                .get("overhead/assets")
                .expect("overhead named sprites"),
        )
        .expect("overhead assets");
        ui_vm.minimap.head_hint_count = rs09_client::ui::sprite::Sheet::decode(
            workbench
                .pack
                .get(&format!("sprite/{}", ids["hint_headicons"]))
                .expect("head hint sprites"),
        )
        .expect("head hint sheet")
        .frames
        .len();
    }

    // DisplayMode. A live client keeps the start-up canvas, the whole frame,
    // until the title opens in the remembered mode (lifecycle_host). The
    // offline workbench has no start-up: it shows the resizable game frame
    // in mode 2 or, with `--root 548`, the fixed one in mode 1 on the fixed
    // canvas. `--window-mode N` pairs either frame with the other mode, and
    // `--window-mode 3` asks for full screen at the screen's largest mode
    // (CS2 5300), which the first frame then carries out.
    ui_vm.display =
        rs09_client::display::Display::new(display_host::window(), display_host::system());
    let fixed_root = option("--root").as_deref() == Some("548");
    if !live_mode {
        let mode = option("--window-mode")
            .and_then(|mode| mode.parse().ok())
            .unwrap_or(if fixed_root { 1 } else { 2 });
        let largest = ui_vm.display.display_modes().last().copied();
        match largest.filter(|_| mode == 3) {
            Some(screen) => {
                ui_vm.display.script_full_screen(
                    screen.width,
                    screen.height,
                    &mut ui_vm.preferences,
                );
            }
            None => {
                ui_vm
                    .display
                    .script_window_mode(mode, &mut ui_vm.preferences);
                ui_vm.display.effects.clear();
            }
        }
    }
    ui_vm.window_mode = ui_vm.display.window_mode();
    display_host::place(&ui_vm.display);
    #[cfg(not(target_arch = "wasm32"))]
    if live_mode && !capture {
        // The window was made with the samples of the preference as saved
        // (`saved_samples`); a change is for the next start.
        eprintln!(
            "Window samples: {} (anti-aliasing preference {})",
            WINDOW_SAMPLES.load(std::sync::atomic::Ordering::Relaxed),
            ui_vm.preferences.anti_aliasing
        );
    }
    // Capture aid: `--fpson` starts as if `::fpson` had been typed.
    if std::env::args().any(|arg| arg == "--fpson") {
        ui_vm.cheat.display_fps = true;
    }
    ui_vm.cycle = if capture {
        100
    } else {
        (get_time() * 50.0) as i32
    };
    ui_vm.location = [workbench.start[0], workbench.start[1], 0];
    let show_ui =
        live_mode || option("--ui").is_some() || (cfg!(target_arch = "wasm32") && ui.is_some());
    if let Some(ui) = &mut ui {
        if !live_mode {
            // `--root 548` previews the fixed game frame, whose scene viewport
            // is a component smaller than the window.
            if fixed_root {
                ui.open_root(548).expect("fixed game frame");
            } else {
                ui.open_root(746).expect("game frame");
                ui.open_sub(746 << 16 | 96, 149, 1).expect("inventory");
            }
            if let Some(group) = option("--ui").and_then(|s| s.parse::<i32>().ok()) {
                ui.open_sub(746 << 16 | 6, group, 0)
                    .expect("preview interface");
            }
            ui.layout(screen_width() as i32, screen_height() as i32)
                .expect("initial interface layout");
            ui_vm.drain(ui);
        }
    }
    let default_gameframe = cfg!(target_arch = "wasm32")
        && !live_mode
        && ui.is_some()
        && option("--replay").is_none()
        && workbench.pack.get("replay.s2c").is_err();
    let replay_bytes = if let Some(path) = option("--replay") {
        Some(load_file(&path).await.expect("read server packet replay"))
    } else if default_gameframe {
        Some(
            load_file("assets/visual/ui-gameframe.bin")
                .await
                .expect("offline browser gameframe preview"),
        )
    } else {
        workbench.pack.get("replay.s2c").ok().map(<[u8]>::to_vec)
    };
    if let Some(bytes) = replay_bytes.filter(|_| !live_mode) {
        let mut decoder = rs09_protocol::game::Decoder::new(
            rs09_protocol::game::Direction::ServerToClient,
            16 * 1024 * 1024,
            65535,
        );
        decoder.push(&bytes).expect("server packet replay size");
        while let Some(frame) = decoder.next_frame().expect("server packet replay framing") {
            replay.push_back(frame);
        }
        assert_eq!(decoder.buffered(), 0, "truncated server replay");
    }
    if option("--replay-now").is_some() || default_gameframe {
        while let Some(frame) = replay.pop_front() {
            let handled = if let Some(ui) = &mut ui {
                ui_vm
                    .apply_frame(ui, &frame)
                    .expect("initial interface replay")
            } else {
                false
            };
            if !handled {
                workbench
                    .server_frame(&frame, ui.as_mut(), ui_vm.cycle)
                    .expect("initial world replay");
            }
        }
        if let Some(ui) = &mut ui {
            ui.layout(screen_width() as i32, screen_height() as i32)
                .expect("replay layout");
            ui_vm.drain(ui);
        }
    }
    println!(
        "Loaded {} models, {} placed meshes, {} triangles, {} animation frames",
        workbench.models.len(),
        workbench.objects,
        workbench.faces,
        workbench.frames.len()
    );
    println!(
        "Texture atlas: {} native texture graphs",
        workbench.textures.generated
    );
    let mut sounds = Vec::new();
    for name in workbench
        .pack
        .manifest
        .files
        .keys()
        .filter(|n| n.starts_with("sound/"))
    {
        match rs09_client::audio::SoundEffect::decode(workbench.pack.get(name).unwrap()) {
            Ok(effect) => match macroquad::audio::load_sound_from_bytes(&effect.wav()).await {
                Ok(sound) => sounds.push((name[6..].parse::<u32>().unwrap(), sound)),
                Err(error) => eprintln!("audio backend {name}: {error}"),
            },
            Err(error) => eprintln!("audio decoder {name}: {error}"),
        }
    }
    println!("Loaded {} synthesized sound effects", sounds.len());
    let sound_index = 0;
    let muted = false;
    let audio_played = false;
    let material = render::material(&workbench.textures, true).expect("world shader");
    let static_renderer = if std::env::args().any(|arg| arg == "--cpu-static-mesh") {
        None
    } else {
        match static_render::Renderer::new() {
            Ok(renderer) => Some(renderer),
            Err(error) => {
                eprintln!(
                    "Persistent static mesh shader unavailable: {error}; using streaming meshes"
                );
                None
            }
        }
    };
    let character_shadow_material =
        render::material(&workbench.textures, false).expect("character shadow shader");
    // GlRenderer.setDepthLayer: the scene's things are drawn on the depth
    // layer of their kind and level, and a character's shadow and the hint
    // arrow at its feet 150 bias units nearer than the character.
    material.set_uniform(
        "DepthBias",
        Vec2::from(rs09_client::depth_layers::offset::NONE),
    );
    character_shadow_material.set_uniform(
        "DepthBias",
        Vec2::from(rs09_client::depth_layers::offset::CHARACTER_SHADOW),
    );
    bind_scene_materials(&workbench, [&material, &character_shadow_material]);
    #[cfg(not(target_arch = "wasm32"))]
    let world_map = option("--world-cache")
        .or_else(|| {
            scene_from_cache.then(|| option("--cache").unwrap_or_else(|| "assets/cache".into()))
        })
        // A session begins at the title screen, whose scene lies in regions
        // no prepared pack holds: a cache named for it is streamed from.
        .or_else(|| option("--cache").filter(|_| live_mode))
        .map(|path| {
            let keys =
                option("--world-keys").unwrap_or_else(|| "runs/player-session/xteas.json".into());
            world_cache::WorldMap::open(path.as_ref(), keys.as_ref(), &mut workbench)
                .unwrap_or_else(|error| panic!("Open world map: {error}"))
        });
    #[cfg(target_arch = "wasm32")]
    let world_map = cache_host::world_map(scene_from_cache, &mut workbench);
    let streaming_map = world_map.is_some();
    // The title screen's 3D scene is built by the same streaming.
    let title_world = title_world::Title::new(streaming_map && live_mode);
    let mut position = vec2(
        workbench.start[0] as f32 + 0.5,
        workbench.start[1] as f32 + 0.5,
    );
    let npc_showcase = !npc_packet_mode && workbench.start == default_start();
    if !npc_packet_mode {
        workbench.spawn_npcs(npc_showcase);
    }
    let mut motion = Movement::new(workbench.start);
    let route_steps = VecDeque::new();
    let destination_flag = walking::DestinationFlag::default();
    let click_cross = None;
    let movement_tick = 0u64;
    let route_run = false;
    let route_status = String::from("Click ground to walk");
    let atmosphere = rs09_client::atmosphere::Atmosphere::default();
    let atmosphere_tick = 0u64;
    // RT4 selects the directional light when the scene loads, independently of
    // fades: LoginManager reads it for the player's chunk after each terrain load.
    let light_direction = workbench.environment(position).direction;
    let lit_scene = workbench.point_lights.revision;
    let light_meshes = scene_lights::LightMeshes::new().expect("light mesh shader");
    // Capture switches over the two preferences.
    let fog_allowed = !std::env::args().any(|arg| arg == "--no-fog");
    let flicker_allowed = !std::env::args().any(|arg| arg == "--no-flicker");
    // Captures of upper floors: the level the workbench player stands on.
    if let Some(plane) = option("--plane").and_then(|s| s.parse().ok()) {
        workbench.ground.plane = plane;
    }
    // The follow camera. A live session starts as the original does (pitch
    // 128, then the login's random yaw and drift); the offline workbench
    // keeps its own starting view, given as before in radians: `--yaw` is the
    // orbit angle and `--pitch` the tilt. `--zoom` is `Camera.ZOOM`.
    let mut follow = rs09_client::follow_camera::Follow::default();
    if !live_mode {
        let radians =
            |name: &str, default: f32| option(name).and_then(|s| s.parse().ok()).unwrap_or(default);
        follow.yaw_target = f64::from(scene_camera::minimap_yaw(radians("--yaw", 2.7)));
        follow.pitch_target =
            f64::from((radians("--pitch", 0.70) * 2048.0 / std::f32::consts::TAU).round());
    }
    if portrait {
        // The portrait capture stands closer than the game does.
        follow.zoom = 300;
    }
    if let Some(zoom) = option("--zoom").and_then(|s| s.parse::<i32>().ok()) {
        // API.SetCameraZoom.
        follow.zoom = zoom.clamp(1, 2000);
    }
    let scene_occlusion = scene_camera::SceneOcclusion::default();
    let scene_view = scene_camera::view(None, &ui_vm.viewport);
    // The cycle's key codes for the keyboard-camera preference.
    let camera_keys = Vec::<i32>::new();
    let tween = true;
    let roofs = std::env::args().any(|arg| arg == "--roofs");
    // `--loc-varbit ID=VALUE` gives multiloc scenery a varbit value where no
    // session or interface pack supplies one (captures).
    let loc_varbit: Option<(i32, i32)> = option("--loc-varbit").and_then(|pair| {
        let (id, value) = pair.split_once('=')?;
        Some((id.parse().ok()?, value.parse().ok()?))
    });
    let shadows_enabled = !std::env::args().any(|arg| arg == "--no-shadows");
    // `Preferences.characterShadowsOn`; `shadows_enabled` is the scenery's.
    let character_shadows = shadows_enabled;
    let brightness = detail.brightness().unwrap_or(3);
    let corrected_lights = std::env::args().any(|arg| arg == "--correct-lights");
    workbench
        .point_lights
        .place(&workbench.regions, corrected_lights);
    let paused = false;
    let ticks = 0.0f64;
    let world_ticks = 0.0f64;
    let manual = 808;
    let frame = 0i32;
    let context_menu = client_ui::ContextMenu::default();
    // Sub-interface opens and closes the open menu has been trimmed for.
    let menu_trims_seen = 0u32;
    let cursor_view = cursor_view::View::default();
    let tooltip_seed = rs09_client::ui::action_tooltip::SeedClock::default();
    let outbox = ActionOutbox {
        live: live_mode,
        ..Default::default()
    };
    // The login runs behind the lifecycle, which hands the connection over
    // when the world accepts it and takes it back on logout or loss. Boxed:
    // it holds connections, and this function's frame is large already.
    let session_life = launch.map(|launch| {
        // The login block carries the CRC of each archive index on disk.
        #[allow(unused_mut)]
        let mut crc_sums = [0; 29];
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(Some(sums)) = rs09_client::cache_update::stored_checksums(
            option("--cache")
                .unwrap_or_else(|| "assets/cache".into())
                .as_ref(),
        ) {
            crc_sums = rs09_client::cache_update::login_crc_sums(&sums);
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(sums) = cache_host::crc_sums() {
            crc_sums = sums;
        }
        Box::new(lifecycle_host::Host::new(
            launch,
            option("--world").and_then(|s| s.parse().ok()),
            capture,
            crc_sums,
        ))
    });
    let connection = None::<rs09_client::live::Connection>;
    let player_state = rs09_protocol::game::players::State::new(0, 0);
    let live_base = None::<[i32; 2]>;
    let live_actors = BTreeMap::<usize, Movement>::new();
    let live_npcs = rs09_client::npc_updates::State::default();
    let live_appearances = BTreeMap::<usize, Model>::new();
    let live_bas = BTreeMap::<usize, PlayerBas>::new();
    let mut live_decoded = BTreeMap::<usize, rs09_protocol::game::players::RawAppearance>::new();
    let mut motion_appearances =
        BTreeMap::<usize, rs09_protocol::game::players::RawAppearance>::new();
    let appearance_cache = BTreeMap::<usize, rs09_protocol::game::players::RawAppearance>::new();
    let mut live_forms = BTreeMap::<usize, rs09_client::player_form::Form>::new();
    let player_picks = Vec::<PlayerPick>::new();
    let mut body_overrides = BTreeMap::<usize, [i32; 2]>::new();
    let mut actor_states = BTreeMap::<usize, rs09_client::actor_animation::State>::new();
    let camera_script = rs09_client::camera::Script::default();
    let spline_finished = rs09_client::camera::SplineFinished::default();
    let mut actor_sounds = Vec::<(usize, rs09_client::actor_animation::Sound, i32)>::new();
    let world_sounds = Vec::<WorldSound>::new();
    let animation_origin = ui_vm.cycle;
    if let Some(path) =
        rs09_client::launch::appearance_preview_path().expect("Appearance preview configuration")
    {
        assert!(
            !live_mode,
            "Appearance preview cannot override a live player"
        );
        let raw = rs09_protocol::game::players::RawAppearance::decode(
            &load_file(&path).await.expect("Appearance preview file"),
        )
        .expect("Appearance preview");
        let ui = ui.as_mut().expect("Appearance preview UI assets");
        let name = format!("bas/{}", raw.bas);
        let bas = PlayerBas::decode(
            ui.pack
                .get(&name)
                .or_else(|_| workbench.pack.get(&name))
                .expect("Preview BAS"),
        )
        .expect("Preview movement BAS");
        workbench
            .player_sequences(&ui.pack, &bas)
            .expect("Preview movement animations");
        let form = ui_models
            .player_form(ui, &workbench, &ui_vm, &raw)
            .expect("Preview player form");
        workbench.avatar = if form.visible() {
            vec![
                ui_models
                    .player_body(ui, &workbench, &raw, [-1; 2], form)
                    .expect("Preview equipped body"),
            ]
        } else {
            vec![]
        };
        rs09_client::player_form::appearance_position(
            &mut motion,
            None,
            &raw,
            &workbench.npc_definitions,
        )
        .expect("Preview appearance position");
        position = vec2(motion.fine[0] as f32 / 128.0, motion.fine[1] as f32 / 128.0);
        live_forms.insert(rs09_protocol::game::players::SELF, form);
        motion_appearances.insert(rs09_protocol::game::players::SELF, raw.clone());
        workbench.bas = bas;
        ui_vm.player_head_id = ui_models.sync_self(ui, &raw).expect("Preview head hash");
        ui_vm.combat_level = raw.combat;
        live_decoded.insert(rs09_protocol::game::players::SELF, raw);
        body_overrides.insert(rs09_protocol::game::players::SELF, [-1; 2]);
    }
    if let Some(id) = option("--sequence")
        .and_then(|s| s.parse::<i32>().ok())
        .or(portrait.then_some(863))
    {
        if let Some(ui) = &ui {
            if let Err(e) = workbench.player_sequence(&ui.pack, id) {
                eprintln!("Preview animation assets: {e}");
            }
        }
        match actor_states
            .entry(rs09_protocol::game::players::SELF)
            .or_default()
            .request(id, 0, 0, &workbench.sequences)
        {
            Ok(events) => actor_sounds.extend(
                events
                    .into_iter()
                    .map(|s| (rs09_protocol::game::players::SELF, s, animation_origin)),
            ),
            Err(error) => eprintln!("Preview animation: {error}"),
        }
    }
    let live_diagnostics = Vec::<String>::new();
    let missing_scene_regions = 0;
    // The one 20 ms logic clock and the packets the main loop sends by itself.
    let cycle_driver = client_cycle::Driver::new();
    let render_profile_started = None::<f64>;
    let static_submission_seconds = 0.0f64;
    let last_render_metrics = serde_json::Value::Null;
    let ui_time = f64::from(ui_vm.cycle);
    let ui_cycle = ui_vm.cycle;
    let ui_scene_delta = 0i32;
    // Keyboard and mouse as the original client sees them: events are taken
    // once per frame, and what the game reads changes once per logic cycle.
    let input = rs09_client::input::Input::default();
    let input_events = rs09_client::input::platform::Events::new();
    // F9, F10 and what they unlock (Escape to quit, the letter toggles) are
    // workbench bindings. In a live session they would shadow the game's own
    // keys, so they need `--workbench-keys` there.
    let workbench_keys = !live_mode || std::env::args().any(|arg| arg == "--workbench-keys");
    let published_inventory_press_active = false;
    let model_cycle = ui_cycle;
    let world_spot_tick = 0u64;
    let ui_drag = rs09_client::ui::drag::Controller::default();
    let ui_debug = false;
    // Everything the frames share, off this function's frame.
    let mut game = Box::new(game::Game {
        live_mode,
        capture_realtime,
        capture_npcs,
        capture_animation,
        capture_models,
        capture,
        capture_frame,
        portrait,
        capture_route,
        capture_route_run,
        frustum_culling,
        detail,
        capture_route_target,
        npc_packet_mode,
        streaming_map,
        workbench_keys,
        fog_allowed,
        flicker_allowed,
        loc_varbit,
        animation_origin,
        preferences_loaded,
        capture_route_issued,
        #[cfg(not(target_arch = "wasm32"))]
        capture_travel,
        workbench,
        npc_replay,
        replay,
        ui,
        ui_renderer,
        ui_models,
        ground_renderer,
        world_picker,
        minimap,
        drift_random,
        drift,
        ui_audio,
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        ambient_audio,
        ui_vm,
        #[cfg(not(target_arch = "wasm32"))]
        preferences_store,
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        music,
        preferences_applied,
        preferences_shown,
        show_ui,
        sounds,
        sound_index,
        muted,
        audio_played,
        material,
        static_renderer,
        character_shadow_material,
        world_map,
        title_world,
        position,
        npc_showcase,
        motion,
        route_steps,
        destination_flag,
        click_cross,
        movement_tick,
        route_run,
        route_status,
        atmosphere,
        atmosphere_tick,
        light_direction,
        lit_scene,
        light_meshes,
        follow,
        scene_occlusion,
        scene_view,
        camera_keys,
        tween,
        roofs,
        shadows_enabled,
        character_shadows,
        brightness,
        corrected_lights,
        paused,
        ticks,
        world_ticks,
        manual,
        frame,
        context_menu,
        menu_trims_seen,
        cursor_view,
        tooltip_seed,
        outbox,
        session_life,
        connection,
        player_state,
        live_base,
        live_actors,
        live_npcs,
        live_appearances,
        live_bas,
        live_decoded,
        motion_appearances,
        appearance_cache,
        live_forms,
        player_picks,
        body_overrides,
        actor_states,
        camera_script,
        spline_finished,
        actor_sounds,
        world_sounds,
        live_diagnostics,
        missing_scene_regions,
        cycle_driver,
        render_profile_started,
        static_submission_seconds,
        last_render_metrics,
        ui_time,
        ui_cycle,
        ui_scene_delta,
        input,
        input_events,
        published_inventory_press_active,
        model_cycle,
        world_spot_tick,
        ui_drag,
        ui_debug,
        display_shown: None,
        dt: 0.0,
        cycles: 0,
        roofs_frame: false,
        wheel_rotation: 0,
        direction: [0; 2],
        running: false,
        self_size: 1,
        view_distance: [0; 2],
        input_camera: None,
        plane: 0,
        ui_nodes: Vec::new(),
        hover_menu: Default::default(),
        at_title: false,
        viewport: None,
        captured_journey_finished: false,
        menu_click: Default::default(),
        walk: Default::default(),
        walk_spell: None,
        world_ran: false,
        interfaces_ran: false,
        #[cfg(not(target_arch = "wasm32"))]
        frame_sleep: option("--frame-sleep")
            .and_then(|ms| ms.parse().ok())
            .map(std::time::Duration::from_millis),
    });
    while game.frame().await {}
}
