//! The title screen's 3D scene on the main loop's state: what the original's
//! hardware renderer shows in the title interface's component with client
//! code 1403. The rules are `rs09_client::title_scene`; this module carries
//! them out with the client's own scene streaming, script camera and NPCs.
//!
//! The original's sequence, and where it is here:
//!
//! - `InterfaceList.resetToLoginScreen(true)` (start-up and every logout)
//!   opens the title interface, whose load scripts lock the camera on the
//!   first tile of a camera path picked at random (CS2 5500/5501 in script
//!   1886). It then puts the camera on that tile, sets up the scene around
//!   it and enters game state 28: [`Title::reset`], then
//!   [`Title::after_interfaces`] once the scripts' camera commands have
//!   been carried out.
//! - Game state 28 runs `LoginManager.rebuildMap` and nothing else, showing
//!   the last frame under "Loading - please wait."; when the scene is built
//!   it places the regions' NPCs and returns to game state 10:
//!   [`Title::loading`] and [`Title::installed`].
//! - Game state 10 (`client.mainUpdate`): each cycle every title NPC may
//!   start a step at random, then moves, turns and animates as in game; the
//!   camera advances unless a login or an account creation is under way;
//!   and when the camera has left the middle of the scene the scene is set
//!   up again around it, through game state 28: [`Title::npc_begin`],
//!   [`Title::npc_end`], [`Title::camera_runs`] and [`Title::camera_cycle`].
//! - A login ends it: `client.resetGameSessionState` drops the NPCs and the
//!   server's first rebuild replaces the scene: [`Title::leave`].
//!
//! Differences from the original:
//! - The scene is built on the map-streaming thread, as in game, so game
//!   state 28 lasts as long as that takes instead of one blocking call.
//! - This client keeps whole regions: the original's scene is the 104 by
//!   104 tiles around the centre zone, cut out of the same regions. Tiles of
//!   a loaded region beyond that square are drawn here and not there.
//! - Without map streaming (prepared packs and no cache) there is no title
//!   scene; the interface's component shows the title background instead
//!   (`title_screen.rs`).
//! - `ScriptRunner.refreshGameStateAfterSettingsChange` (a display setting
//!   changed on the title screen rebuilds the scene, 10 to 28) is not
//!   carried out: scenery here follows such settings without a rebuild.
//! - NPCs are kept in index order for both the per-cycle pass and drawing;
//!   the original draws them in the order they were placed.
use super::{Workbench, world_cache};
use macroquad::prelude::{Vec2, vec2};
use rs09_client::{
    atmosphere::Atmosphere,
    camera::Script,
    npc_updates,
    title_scene::{self, Central, Slots},
    walking::{Movement, Navigation},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    /// Not at the title screen, or no scene can be built.
    Off,
    /// `resetToLoginScreen` has opened the interface; the camera commands
    /// of its load scripts are still to be carried out.
    Reset,
    /// Game state 28.
    Loading,
    /// Game state 10 with the scene built.
    Shown,
}

/// The main loop's state a title scene touches.
pub struct Scene<'a> {
    pub world: &'a mut Workbench,
    pub map: Option<&'a mut world_cache::WorldMap>,
    pub camera: &'a mut Script,
    pub npcs: &'a mut npc_updates::State,
    /// `PlayerList.self`.
    pub motion: &'a mut Movement,
    pub atmosphere: &'a mut Atmosphere,
}

pub struct Title {
    supported: bool,
    phase: Phase,
    central: Central,
    slots: Slots,
    /// `regionBitPacked` of the regions whose files the scene has.
    regions: Vec<i32>,
    /// The NPCs placed: `loginscreenproperties` and size by index.
    npcs: BTreeMap<usize, (i32, i32)>,
    reported: BTreeSet<String>,
    /// When game state 28 began, for the console line that ends it.
    began: f64,
}

impl Title {
    /// `supported`: maps are streamed from a cache, so a scene can be built
    /// anywhere.
    pub fn new(supported: bool) -> Self {
        Self {
            supported,
            phase: Phase::Off,
            central: Central::default(),
            slots: Slots::default(),
            regions: Vec::new(),
            npcs: BTreeMap::new(),
            reported: BTreeSet::new(),
            began: 0.0,
        }
    }

    fn report(&mut self, message: String) {
        if self.reported.insert(message.clone()) {
            eprintln!("Title scene: {message}");
        }
    }

    /// `InterfaceList.resetToLoginScreen(true)` has opened the title
    /// interface and run its load scripts. `LoginManager.processLogout`
    /// before it (and the client's start) leave `Camera.originX/Y` and
    /// `SceneGraph.centralZoneX/Y` at 0 and no NPCs; the scripts' camera
    /// commands, carried out by the main loop's next interface cycle,
    /// convert their tiles against that origin.
    pub fn reset(&mut self, world: &mut Workbench) {
        if !self.supported {
            return;
        }
        self.phase = Phase::Reset;
        self.central = Central::default();
        self.npcs.clear();
        world.navigation.origin = [0, 0];
    }

    /// A login succeeded (`client.resetGameSessionState`): the scene now
    /// belongs to the game.
    pub fn leave(&mut self) {
        self.phase = Phase::Off;
        self.npcs.clear();
    }

    /// The scene component shows the scene, not the stand-in background.
    pub fn active(&self) -> bool {
        self.phase != Phase::Off
    }

    /// Game state 28: the last frame stays up under the loading box, and
    /// neither the interface's logins nor the scene's own cycle run.
    pub fn loading(&self) -> bool {
        matches!(self.phase, Phase::Reset | Phase::Loading)
    }

    /// Game state 28 proper: the scene is being built, and the interface's
    /// load scripts have been carried out.
    pub fn building(&self) -> bool {
        self.phase == Phase::Loading
    }

    /// The rest of `resetToLoginScreen` (InterfaceList.java:1113-1121) once
    /// the load scripts' camera commands are in effect: the camera goes to
    /// its locked tile (or takes one spline step), the fog jumps to its new
    /// values, the scene is set up around the camera, and the game state is
    /// 28. `pending` is whether camera commands are still queued.
    pub fn after_interfaces(&mut self, pending: bool, mut scene: Scene<'_>) {
        if self.phase != Phase::Reset || pending {
            return;
        }
        scene.camera.title_reset(self.central.origin);
        scene.atmosphere.instant = true;
        self.setup(&mut scene);
        self.phase = Phase::Loading;
    }

    /// `LoginManager.setupLoadingScreenRegion` and the `loadRegion` it ends
    /// in.
    fn setup(&mut self, scene: &mut Scene<'_>) {
        let Some(cache) = scene.world.world_cache.clone() else {
            return;
        };
        let render = scene.camera.render;
        let selection = title_scene::select([render[0], render[1]], self.central.origin, |x, y| {
            cache.has_map(&format!("n{x}_{y}"))
        });
        let Some(loaded) = self.central.load(selection.zone) else {
            return;
        };
        self.phase = Phase::Loading;
        self.began = macroquad::time::get_time();
        self.regions = selection
            .regions
            .iter()
            .filter(|(_, listed)| *listed)
            .map(|(packed, _)| *packed)
            .collect();
        // NPCs outside the new scene are dropped; the others stay where
        // they are (their tiles are absolute here).
        let fine_origin = loaded.origin.map(|n| n.wrapping_mul(128));
        let kept: Vec<usize> = scene
            .npcs
            .actors
            .iter()
            .filter(|(_, npc)| {
                title_scene::npc_kept([
                    npc.motion.fine[0].wrapping_sub(fine_origin[0]),
                    npc.motion.fine[1].wrapping_sub(fine_origin[1]),
                ])
            })
            .map(|(&index, _)| index)
            .collect();
        scene.npcs.actors.retain(|index, _| kept.contains(index));
        self.npcs.retain(|index, _| kept.contains(index));
        for index in scene.npcs.local_ids.drain(..) {
            if !kept.contains(&index) {
                scene.world.overheads.remove(&(true, index));
            }
        }
        scene.npcs.local_ids = kept;
        // Player.plane = 0; PlayerList.self.teleport(8, false, 8).
        scene.world.ground.plane = 0;
        *scene.motion = Movement::new(self.central.self_tile());
        scene.camera.rebase(loaded.delta);
        if loaded.instant_fade {
            scene.atmosphere.instant = true;
        }
        // SceneGraph.spotanims.clear(); SceneGraph.projectiles.clear()
        scene.world.world_spots.effects.clear();
        scene.world.world_spots.projectiles.clear();
        // Camera.originX/Y; the collision maps are filled again when the
        // scene is installed.
        scene.world.navigation =
            Navigation::new(&scene.world.regions, &scene.world.placements, loaded.origin);
        if let Some(map) = scene.map.as_deref_mut() {
            map.title(selection.loaded().collect());
        }
        eprintln!(
            "Game state 28: title scene around zone {:?} (base {:?}), regions {:?}, {} NPCs kept",
            selection.zone,
            loaded.origin,
            selection.loaded().collect::<Vec<_>>(),
            scene.npcs.local_ids.len()
        );
    }

    /// The tile the scene is streamed around while the title scene is up:
    /// the middle of the scene, whose region and its neighbours hold every
    /// region the original asks for.
    pub fn stream_tile(&self) -> Option<[i32; 2]> {
        matches!(self.phase, Phase::Loading | Phase::Shown).then(|| self.central.middle())
    }

    /// The region the scene is installed under once built.
    pub fn stream_region(&self) -> [i32; 2] {
        self.central.middle().map(|tile| tile >> 6)
    }

    /// The end of `LoginManager.rebuildMap` in game state 28, once the
    /// scene asked for is the one installed: the regions' NPC files are
    /// decoded and the game state is 10. `cycle` is `client.loop`.
    pub fn installed(&mut self, mut scene: Scene<'_>, cycle: i32) {
        if self.phase != Phase::Loading {
            return;
        }
        let center = self.stream_region();
        // The original waits for its map files for as long as it takes. A
        // scene that cannot be built at all gives way to the stand-in
        // background, so that the title screen can still be used.
        if scene
            .map
            .as_deref()
            .is_none_or(|map| map.build_failed(center))
        {
            self.report("the scene could not be built".into());
            self.phase = Phase::Off;
            return;
        }
        if !scene
            .map
            .as_deref()
            .is_some_and(|map| map.title_installed(center))
        {
            return;
        }
        self.decode_npcs(&mut scene, cycle);
        self.phase = Phase::Shown;
        eprintln!(
            "Game state 28 -> 10: title scene built in {:.2} s, {} NPCs",
            macroquad::time::get_time() - self.began,
            scene.npcs.local_ids.len()
        );
    }

    /// `LoginManager.decodeNpcFiles` (LoginManager.java:1314-1358).
    fn decode_npcs(&mut self, scene: &mut Scene<'_>, cycle: i32) {
        let Some(cache) = scene.world.world_cache.clone() else {
            return;
        };
        let origin = self.central.origin;
        for packed in self.regions.clone() {
            let region = [packed >> 8, packed & 0xff];
            let data = match cache.title_spawns(region) {
                Ok(Some(data)) => data,
                Ok(None) => continue,
                Err(error) => {
                    self.report(error);
                    continue;
                }
            };
            let Some(slot) = self.slots.slot(packed) else {
                self.report("more than 64 regions with NPC files".into());
                continue;
            };
            for spawn in title_scene::spawns(&data, packed, slot, origin) {
                let id = spawn.npc as u32;
                // NpcTypeList.get: the prepared catalogue may lack a type
                // the game's server never sends.
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    scene.world.npc_definitions.entry(id)
                {
                    match cache.npc(id) {
                        Ok(Some(definition)) => {
                            entry.insert(definition);
                        }
                        Ok(None) => continue,
                        Err(error) => {
                            self.report(error);
                            continue;
                        }
                    }
                }
                let definition = &scene.world.npc_definitions[&id];
                let (properties, size) = (definition.login_screen, definition.size);
                if scene.npcs.actors.contains_key(&spawn.index)
                    || !title_scene::spawn_placed(&spawn, properties, size)
                {
                    continue;
                }
                let Some(angle) = title_scene::spawn_angle(definition.spawn_direction) else {
                    self.report(format!("NPC {id} faces no known direction"));
                    continue;
                };
                let metadata = match scene.world.npc_metadata(spawn.npc) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        self.report(format!("NPC {id}: {error}"));
                        continue;
                    }
                };
                let children = definition.children.is_some();
                let prepared = scene.world.ensure_npc_bas(metadata.bas).and_then(|()| {
                    if children {
                        Ok(())
                    } else {
                        scene.world.ensure_npc_body(id)
                    }
                });
                if let Err(error) = prepared {
                    self.report(format!("NPC {id} body: {error}"));
                    continue;
                }
                // npc.teleport(size, tileX, tileY, true)
                let tile = [spawn.tile[0] + origin[0], spawn.tile[1] + origin[1]];
                let mut motion = Movement::new(tile);
                motion.fine = tile.map(|n| n.wrapping_mul(128).wrapping_add(size.wrapping_mul(64)));
                motion.target_angle = angle;
                motion.turn_speed = metadata.turn_speed;
                motion.angle = if metadata.turn_speed == 0 { 0 } else { angle };
                motion.rotates_on_move = metadata.rotates;
                scene.npcs.actors.insert(
                    spawn.index,
                    npc_updates::Actor {
                        type_id: spawn.npc,
                        metadata,
                        last_seen: cycle,
                        motion,
                        animation: Default::default(),
                        chat: Vec::new(),
                        chat_loops: 100,
                        hit_bar: 0,
                        hit_bar_until: -1000,
                    },
                );
                scene.npcs.local_ids.push(spawn.index);
                self.npcs.insert(spawn.index, (properties, size));
            }
        }
        scene.npcs.local_ids.sort_unstable();
    }

    /// The start of an NPC's turn in `client.mainUpdate`'s pass
    /// (client.java:1167-1192): its wander test. Returns true when the rest
    /// of its turn is not to run: the original's `continue nextNpc`, and
    /// all of game state 28, in which `mainUpdate` is not called.
    pub fn npc_begin(
        &mut self,
        index: usize,
        npc: &mut npc_updates::Actor,
        navigation: &mut Navigation,
        random: impl FnMut() -> f64,
    ) -> bool {
        match self.phase {
            Phase::Off => return false,
            Phase::Reset | Phase::Loading => return true,
            Phase::Shown => {}
        }
        let Some(&(properties, size)) = self.npcs.get(&index) else {
            return false;
        };
        let origin = navigation.origin;
        let fine =
            [0, 1].map(|axis| npc.motion.fine[axis].wrapping_sub(origin[axis].wrapping_mul(128)));
        let wander = title_scene::wander(
            &mut navigation.maps[0],
            fine,
            size,
            properties,
            npc.motion.queue.is_empty(),
            random,
        );
        if let Some(head) = wander.head {
            let head = [head[0] + origin[0], head[1] + origin[1]];
            // movementQueueX/Y[0] and movementQueueSpeed[0].
            npc.motion.head = head;
            npc.motion.head_speed = 1;
            if wander.walk {
                npc.motion.queue.push_back((head, 1));
            }
        }
        wander.skip
    }

    /// The end of an NPC's turn: its tiles are marked in the collision map
    /// (client.java:1196).
    pub fn npc_end(&self, index: usize, npc: &npc_updates::Actor, navigation: &mut Navigation) {
        if self.phase != Phase::Shown {
            return;
        }
        let Some(&(_, size)) = self.npcs.get(&index) else {
            return;
        };
        let origin = navigation.origin;
        let fine =
            [0, 1].map(|axis| npc.motion.fine[axis].wrapping_sub(origin[axis].wrapping_mul(128)));
        title_scene::mark(&mut navigation.maps[0], fine, size);
    }

    /// The camera advances this cycle. At the title screen it does so only
    /// in game state 10 and while `LoginManager.step` and
    /// `CreateManager.step` are 0 (client.java:1202); `busy` is either of
    /// them not being 0.
    pub fn camera_runs(&self, busy: bool) -> bool {
        match self.phase {
            Phase::Off => true,
            Phase::Reset | Phase::Loading => false,
            Phase::Shown => !busy,
        }
    }

    /// After the camera's update in game state 10 (client.java:1208-1210):
    /// a camera that has left the middle of the scene has the scene set up
    /// around it again.
    pub fn camera_cycle(&mut self, mut scene: Scene<'_>) {
        if self.phase != Phase::Shown {
            return;
        }
        let render = scene.camera.render;
        if title_scene::camera_outside([render[0], render[1]]) {
            self.setup(&mut scene);
        }
    }

    /// Where the scene is looked at from, in absolute tiles: the camera.
    /// In game state 10 the fog and sky are those of the map chunk the
    /// camera is over (`FogManager.updateAtmosphere(.., Camera.renderY >>
    /// 10, .., Camera.renderX >> 10)`, ScriptRunner.java:306-307), and what
    /// is drawn is what lies around the camera, not around a player.
    pub fn eye(&self, camera: &Script) -> Option<Vec2> {
        self.active().then(|| {
            vec2(
                camera.render[0] as f32 / 128.0 + self.central.origin[0] as f32,
                camera.render[1] as f32 / 128.0 + self.central.origin[1] as f32,
            )
        })
    }

    /// The tile whose map chunk gives the sun's direction: the local
    /// player's, which `loadRegion` puts on scene tile (8, 8)
    /// (`FogManager.setLightPosition` in `LoginManager.rebuildMap`).
    pub fn sun(&self) -> Option<Vec2> {
        self.active().then(|| {
            let [x, y] = self.central.self_tile();
            vec2(x as f32 + 0.5, y as f32 + 0.5)
        })
    }
}
