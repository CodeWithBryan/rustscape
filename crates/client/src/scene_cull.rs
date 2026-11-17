//! The original's occluders over the workbench's scene.
//!
//! `rs09_client::occlusion` holds the occluders and their queries and
//! `rs09_client::scene_order` asks them while it replays the original's walk
//! over the tiles. This module builds the occluders from the loaded map,
//! answers the walk's questions for the parts `world_pick` gives it, and
//! once a frame replays the walk without actors to learn which map
//! locations the original would leave undrawn and in what order it would
//! draw the rest. `roof_render` drops the first from the chunks' index
//! lists; the second orders the translucent faces drawn after the opaque
//! scene.
use crate::Workbench;
use rs09_client::{
    occlusion::{self, Flags, Occluder},
    scene_order::Occlusion,
};

/// `SceneGraph.occluders` for the 104 by 104 tiles from the workbench's
/// origin: `buildScene`'s floor marks, `addLoc`'s marks for every location
/// of the map, and the runs made of them.
///
/// The original marks while it loads a scene and never again; a location
/// the server adds or removes later changes no occluder. Here the map is
/// read when this is called, so the caller calls it once per built scene.
pub(crate) fn occluders(world: &Workbench) -> Vec<Occluder> {
    let origin = world.navigation.origin;
    let tile = |level: usize, x: i32, y: i32| {
        crate::tile(&world.regions, level, origin[0] + x, origin[1] + y)
    };
    let shows_underlay: Vec<bool> = crate::scene_floors(&world.pack)
        .map(|(_, overlays)| overlays.iter().map(|o| !o.occlude_underlay).collect())
        .unwrap_or_default();
    let mut flags = Flags::new();
    for level in 1..4 {
        for x in 1..103 {
            for y in 1..103 {
                let t = tile(level, x, y);
                if t.underlay == 0 && t.overlay == 0 {
                    continue;
                }
                let occludes = !usize::from(t.overlay)
                    .checked_sub(1)
                    .and_then(|id| shows_underlay.get(id))
                    .copied()
                    .unwrap_or(false);
                flags.floor(
                    level,
                    [x, y],
                    t.underlay,
                    t.overlay,
                    t.shape,
                    occludes,
                    [
                        t.height,
                        tile(level, x + 1, y).height,
                        tile(level, x + 1, y + 1).height,
                        tile(level, x, y + 1).height,
                    ],
                );
            }
        }
    }
    for (slot, location) in &world.world.locations {
        let [x, y] = [slot.tile[0] - origin[0], slot.tile[1] - origin[1]];
        // SceneGraph.readLocs keeps the scene's outermost ring empty.
        if x <= 0 || y <= 0 || x >= 103 || y >= 103 || slot.plane > 3 {
            continue;
        }
        let Some(kind) = u32::try_from(location.id)
            .ok()
            .and_then(|id| world.location_types.get(&id))
        else {
            continue;
        };
        // addLoc returns before any mark for a type hidden in HD.
        if kind.placement.hidden_in_hd {
            continue;
        }
        flags.location(
            slot.plane,
            [x, y],
            i32::from(location.shape),
            i32::from(location.rotation),
            kind.occlude,
        );
    }
    flags.build(|level, x, y| tile(level, x, y).height)
}

/// One walk's occlusion answers: the frame's queries, each model's
/// `getMinY()`, and the parts the walk reached and left undrawn.
pub(crate) struct Answers<'a, H, M> {
    frame: occlusion::Frame<'a, H>,
    min_y: M,
    pub culled: Vec<u32>,
}

impl<'a, H: Fn(usize, i32, i32) -> i32, M: Fn(u32) -> i32> Answers<'a, H, M> {
    pub(crate) fn new(active: &'a occlusion::Active, height: H, min_y: M) -> Self {
        Self {
            frame: occlusion::Frame::new(active, height),
            min_y,
            culled: Vec::new(),
        }
    }
}

impl<H: Fn(usize, i32, i32) -> i32, M: Fn(u32) -> i32> Occlusion for Answers<'_, H, M> {
    fn tile(&mut self, level: usize, x: i32, y: i32) -> bool {
        self.frame.tile(level, x, y)
    }
    fn wall(&mut self, level: usize, x: i32, y: i32, flags: i32) -> bool {
        self.frame.wall(level, x, y, flags)
    }
    fn decor(&mut self, level: usize, x: i32, y: i32, part: u32) -> bool {
        self.frame.object(level, x, y, (self.min_y)(part))
    }
    fn area(&mut self, level: usize, min: [i32; 2], max: [i32; 2], part: u32) -> bool {
        self.frame.area(level, min, max, (self.min_y)(part))
    }
    fn culled(&mut self, part: u32) {
        self.culled.push(part);
    }
}

/// What one replay of the walk says about the map's locations.
#[derive(Default)]
pub(crate) struct Verdict {
    /// Indices into `Workbench::pick_objects` the original reaches and
    /// leaves undrawn.
    pub statics: Vec<usize>,
    /// The same for `Workbench::animated_locations`.
    pub animated: Vec<usize>,
    /// `pick_objects` indices in the order the original draws them.
    pub order: Vec<usize>,
    /// Changes with every new replay.
    pub serial: u64,
}

impl Workbench {
    /// One frame of occlusion for drawing, after `roof_frame`: `rect` is the
    /// scene's rectangle in the window.
    pub(crate) fn cull_frame(
        &mut self,
        picker: &mut crate::world_pick::Picker,
        camera: &crate::scene_camera::SceneCamera,
        rect: [f32; 4],
    ) {
        let verdict = picker.cull(self, camera, rect);
        self.occlude(verdict);
    }
}
