//! The follow camera, RT4 `Camera.cameraType == 1`, as the 2009scape-modified
//! client runs it: `Camera.updatePlayerCamera`, `clampCameraAngle` and
//! `calculateRenderPosition`, the arrow keys of `GameShell.mainInputLoop`, the
//! drift of `Protocol`'s main loop and the follow branch of
//! `ScriptRunner.renderGameScene`.
//!
//! Units are the original's: scene-local fine coordinates (128 per tile) with
//! height negative upward and 2048 angle units per turn. The angle targets are
//! `double`, as the modified client made them for its per-frame key input.
use crate::{
    animation::sin_cos,
    camera::Pose,
    terrain::{OcclusionLevel, Region, Tile},
};

/// `Keyboard.KEY_LEFT` and the other arrow key codes.
pub const KEY_LEFT: i32 = 96;
pub const KEY_RIGHT: i32 = 97;
pub const KEY_UP: i32 = 98;
pub const KEY_DOWN: i32 = 99;

/// What the camera reads from `SceneGraph`. Coordinates are scene-local; an
/// implementation returns 0 outside its arrays, where the original would throw.
pub trait Scene {
    /// `SceneGraph.getTileHeight(plane, x, y)` for fine coordinates.
    fn ground(&self, plane: i32, fine: [i32; 2]) -> i32;
    /// `SceneGraph.renderFlags[level][x][y]`.
    fn flags(&self, level: i32, x: i32, y: i32) -> u8;
    /// `SceneGraph.tileHeights[level][x][y]`.
    fn height(&self, level: i32, x: i32, y: i32) -> i32;
    /// `SceneGraph.occlusionHeights[level][x][y]`.
    fn occlusion(&self, level: i32, x: i32, y: i32) -> u8;
}

/// The arrow keys held this frame (`Keyboard.pressedKeys`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Held {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Follow {
    /// `Camera.pitchTarget`, 128 to 383.
    pub pitch_target: f64,
    /// `Camera.yawTarget`.
    pub yaw_target: f64,
    /// `Camera.cameraX` and `cameraY`: the point the camera orbits, which
    /// eases toward the player.
    pub position: [i32; 2],
    /// `Camera.cameraOffsetX` and `cameraOffsetY`.
    pub offset: [i32; 2],
    /// `Camera.offsetXDelta` and `Protocol.cameraYOffsetDelta`.
    pub offset_delta: [i32; 2],
    /// `Camera.yawDrift`.
    pub yaw_drift: i32,
    /// `Protocol.yawDriftDelta`.
    pub yaw_drift_delta: i32,
    /// `Camera.maxPitchDistance`: the eased terrain minimum pitch, times 256.
    pub max_pitch_distance: i32,
    /// `Camera.ZOOM`. Nothing in the client changes it; plugins may, through
    /// `API.SetCameraZoom` and `API.UpdateCameraZoom`.
    pub zoom: i32,
    /// `Protocol.cameraJitterTimer`.
    pub jitter_timer: i32,
}

impl Default for Follow {
    fn default() -> Self {
        Self {
            pitch_target: 128.0,
            yaw_target: 0.0,
            position: [0; 2],
            offset: [0; 2],
            offset_delta: [2; 2],
            yaw_drift: 0,
            yaw_drift_delta: 1,
            max_pitch_distance: 0,
            zoom: 600,
            jitter_timer: 0,
        }
    }
}

impl Follow {
    /// The login reset in `client.java`: random drift offsets and a yaw within
    /// ten units of north. `random` is `Math.random()`.
    pub fn login(&mut self, mut random: impl FnMut() -> f64) {
        self.offset[0] = (random() * 100.0) as i32 - 50;
        self.yaw_target = f64::from(((random() * 20.0) as i32 - 10) & 0x7ff);
        self.offset[1] = (random() * 110.0) as i32 - 55;
        self.yaw_drift = (random() * 80.0) as i32 - 40;
    }

    /// `Camera.clampCameraAngle`. Besides clamping the targets, every call
    /// moves the terrain minimum pitch one step toward what the nine by nine
    /// tiles around the camera ask for.
    pub fn clamp(&mut self, plane: i32, scene: &impl Scene) {
        // Two comparisons in the original; a NaN passes through both forms.
        self.pitch_target = self.pitch_target.clamp(128.0, 383.0);
        // Camera.mod with 2047.0, not 2048.0, as the original has it.
        self.yaw_target = ((self.yaw_target % 2047.0) + 2047.0) % 2047.0;
        let [tile_x, tile_y] = self.position.map(|n| n >> 7);
        let ground = scene.ground(plane, self.position);
        let mut max_height = 0i32;
        if tile_x > 3 && tile_y > 3 && tile_x < 100 && tile_y < 100 {
            for i in tile_x - 4..=tile_x + 4 {
                for j in tile_y - 4..=tile_y + 4 {
                    let mut level = plane;
                    if level < 3 && scene.flags(1, i, j) & 2 == 2 {
                        level += 1;
                    }
                    let difference = i32::from(scene.occlusion(level, i, j))
                        .wrapping_mul(8)
                        .wrapping_add(ground)
                        .wrapping_sub(scene.height(level, i, j));
                    if difference > max_height {
                        max_height = difference;
                    }
                }
            }
        }
        let goal = max_height.wrapping_mul(192).clamp(32768, 98048);
        let current = self.max_pitch_distance;
        if current < goal {
            self.max_pitch_distance = current.wrapping_add(goal.wrapping_sub(current) / 24);
        } else if goal < current {
            self.max_pitch_distance = current.wrapping_add(goal.wrapping_sub(current) / 80);
        }
    }

    /// `GameShell.mainInputLoop`, once per rendered frame: held arrow keys turn
    /// the targets by `calcRenderDelta(18)` and `calcRenderDelta(24)`, which
    /// are 324 and 576 units per second. `render_delta` is the nanoseconds
    /// since the previous frame was drawn.
    pub fn keys(&mut self, render_delta: i64, held: Held, plane: i32, scene: &impl Scene) {
        if !(held.up || held.down || held.left || held.right) {
            return;
        }
        let delta = |value: f64| value / 1_000_000_000.0 * render_delta as f64 * value;
        let vertical = delta(18.0);
        if held.up {
            self.pitch_target += vertical;
        } else if held.down {
            self.pitch_target -= vertical;
        }
        let horizontal = delta(24.0);
        if held.left {
            self.yaw_target -= horizontal;
        } else if held.right {
            self.yaw_target += horizontal;
        }
        self.clamp(plane, scene);
    }

    /// CAM_FORCEANGLE and CS2 5504: both targets are replaced, then clamped.
    pub fn set_angles(&mut self, pitch: f64, yaw: f64, plane: i32, scene: &impl Scene) {
        self.pitch_target = pitch;
        self.yaw_target = yaw;
        self.clamp(plane, scene);
    }

    /// `Camera.updatePlayerCamera`, once per 20 ms cycle while the follow
    /// camera is in charge. `player` is `PlayerList.self.xFine, yFine`.
    /// `typed` is the cycle's key codes (`InterfaceList.keyCodes`) when the
    /// keyboard-camera preference is on and None when it is off.
    pub fn cycle(
        &mut self,
        player: [i32; 2],
        plane: i32,
        typed: Option<&[i32]>,
        scene: &impl Scene,
    ) {
        let goal = [
            player[0].wrapping_add(self.offset[0]),
            player[1].wrapping_add(self.offset[1]),
        ];
        let [dx, dy] = [0, 1].map(|axis| self.position[axis].wrapping_sub(goal[axis]));
        if !(-500..=500).contains(&dx) || !(-500..=500).contains(&dy) {
            self.position = goal;
        }
        for axis in [1, 0] {
            if self.position[axis] != goal[axis] {
                self.position[axis] = self.position[axis]
                    .wrapping_add(goal[axis].wrapping_sub(self.position[axis]) / 16);
            }
        }
        if let Some(codes) = typed {
            for &code in codes {
                match code {
                    KEY_UP => self.pitch_target += 47.0,
                    KEY_DOWN => self.pitch_target -= 17.0,
                    KEY_LEFT => self.yaw_target -= 65.0,
                    KEY_RIGHT => self.yaw_target += 191.0,
                    _ => {}
                }
            }
            self.clamp(plane, scene);
        }
    }

    /// The camera's share of `Protocol`'s main loop, once per game cycle
    /// whatever the camera type: every 501st cycle the yaw drift and the two
    /// offsets each move a step or stay, and the steps turn round at their
    /// limits. `random` is `Math.random()`, drawn only on that cycle.
    pub fn drift(&mut self, mut random: impl FnMut() -> f64) {
        self.jitter_timer = self.jitter_timer.wrapping_add(1);
        if self.jitter_timer > 500 {
            self.jitter_timer = 0;
            let bits = (random() * 8.0) as i32;
            if bits & 4 == 4 {
                self.yaw_drift = self.yaw_drift.wrapping_add(self.yaw_drift_delta);
            }
            if bits & 2 == 2 {
                self.offset[1] = self.offset[1].wrapping_add(self.offset_delta[1]);
            }
            if bits & 1 == 1 {
                self.offset[0] = self.offset[0].wrapping_add(self.offset_delta[0]);
            }
        }
        if self.offset[0] < -50 {
            self.offset_delta[0] = 2;
        }
        if self.offset[1] < -55 {
            self.offset_delta[1] = 2;
        }
        if self.offset[1] > 55 {
            self.offset_delta[1] = -2;
        }
        if self.yaw_drift < -40 {
            self.yaw_drift_delta = 1;
        }
        if self.offset[0] > 50 {
            self.offset_delta[0] = -2;
        }
        if self.yaw_drift > 40 {
            self.yaw_drift_delta = -1;
        }
    }

    /// The follow branch of `ScriptRunner.renderGameScene`: where the camera
    /// is this frame. `view_height` is the viewport height after
    /// `calculateViewportBounds`; `view_distance` is `ScriptRunner`'s
    /// `[minViewDistance, maxViewDistance]`; `pitch_floor` is
    /// `cameraAmplitude[4] + 128` while shake channel 4 is active.
    pub fn pose(
        &self,
        view_height: i32,
        view_distance: [i32; 2],
        player: [i32; 2],
        plane: i32,
        pitch_floor: Option<i32>,
        scene: &impl Scene,
    ) -> Pose {
        let yaw = self.yaw_drift.wrapping_add(self.yaw_target as i32) & 0x7ff;
        let mut pitch = self.pitch_target as i32;
        if pitch < self.max_pitch_distance / 256 {
            pitch = self.max_pitch_distance / 256;
        }
        if let Some(floor) = pitch_floor {
            if floor > pitch {
                pitch = floor;
            }
        }
        render_position(
            self.position,
            view_height,
            scene.ground(plane, player).wrapping_sub(50),
            self.zoom.wrapping_add(pitch.wrapping_mul(3)),
            yaw,
            pitch,
            view_distance,
        )
    }
}

/// `Camera.calculateRenderPosition` with the HD renderer enabled: the point
/// `distance` behind `target` at height `z`, looking along `pitch` and `yaw`.
/// A viewport taller than 334 pixels moves the camera back, up to
/// `view_distance[1] / 256` of the distance at 434 pixels.
pub fn render_position(
    target: [i32; 2],
    view_height: i32,
    z: i32,
    distance: i32,
    yaw: i32,
    pitch: i32,
    view_distance: [i32; 2],
) -> Pose {
    let scaled = view_height.wrapping_sub(334).clamp(0, 100);
    let view = scaled.wrapping_mul(view_distance[1].wrapping_sub(view_distance[0])) / 100
        + view_distance[0];
    let distance = view.wrapping_mul(distance) >> 8;
    let pitch_turn = 2048i32.wrapping_sub(pitch) & 0x7ff;
    let yaw_turn = 2048i32.wrapping_sub(yaw) & 0x7ff;
    let (mut offset_x, mut offset_z, mut offset_y) = (0, distance, 0);
    if pitch_turn != 0 {
        let (sin, cos) = sin_cos(pitch_turn);
        offset_y = sin.wrapping_mul(distance.wrapping_neg()) >> 16;
        offset_z = cos.wrapping_mul(distance) >> 16;
    }
    if yaw_turn != 0 {
        let (sin, cos) = sin_cos(yaw_turn);
        offset_x = sin.wrapping_mul(offset_z) >> 16;
        offset_z = offset_z.wrapping_mul(cos) >> 16;
    }
    Pose {
        position: [
            target[0].wrapping_sub(offset_x),
            target[1].wrapping_sub(offset_z),
            z.wrapping_sub(offset_y),
        ],
        pitch,
        yaw,
    }
}

/// `LoginManager.loadRegion`'s camera and map-flag share for a rebuild the
/// server asked for (`fromLogin == false`). With the HD renderer the original
/// ignores a rebuild whose zone is the current one, so nothing happens when
/// the scene base is unchanged. Otherwise a standing destination flag moves
/// with the base and the view goes back to the follow camera: the return
/// value says to release the scripted camera (`Camera.cameraType = 1`; its
/// shakes carry on). The follow camera itself is not moved; its next cycle
/// finds the player more than 500 units away and snaps. `flag` is
/// scene-local, with x 0 meaning no flag.
pub fn rebuild(flag: &mut [i32; 2], old_base: Option<[i32; 2]>, base: [i32; 2]) -> bool {
    if old_base == Some(base) {
        return false;
    }
    if let Some(old) = old_base {
        if flag[0] != 0 {
            flag[1] = flag[1].wrapping_sub(base[1].wrapping_sub(old[1]));
            flag[0] = flag[0].wrapping_sub(base[0].wrapping_sub(old[0]));
        }
    }
    true
}

/// `SceneGraph.occlusionHeights`: one byte per tile and level of the 104 by
/// 104 scene, from map opcode 129.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Occlusion {
    heights: Vec<u8>,
}

impl Default for Occlusion {
    fn default() -> Self {
        Self {
            heights: vec![0; 4 * 104 * 104],
        }
    }
}

impl Occlusion {
    pub fn get(&self, level: i32, x: i32, y: i32) -> u8 {
        Self::index(level, x, y).map_or(0, |index| self.heights[index])
    }

    fn index(level: i32, x: i32, y: i32) -> Option<usize> {
        ((0..4).contains(&level) && (0..104).contains(&x) && (0..104).contains(&y))
            .then(|| (level * 104 * 104 + x * 104 + y) as usize)
    }

    fn set(&mut self, level: i32, x: i32, y: i32, value: u8) {
        if let Some(index) = Self::index(level, x, y) {
            self.heights[index] = value;
        }
    }

    /// The opcode 129 part of `SceneGraph.readTerrain` for one map file whose
    /// south-west tile is at scene tile `(tile_x, base_y)`. The original's
    /// clearing (type 0) and copying (type 2) loops advance their row counter
    /// once only, so they reach a single column; a file without any record
    /// clears a 16 by 16 block addressed in quarter tiles. Both are kept.
    pub fn read(&mut self, tile_x: i32, base_y: i32, records: &[[OcclusionLevel; 4]]) {
        let bound = |n: i32| n.clamp(0, 104);
        for record in records {
            for (level, entry) in (0i32..).zip(record) {
                let start_x = bound(tile_x);
                let end_x = bound(tile_x.wrapping_add(64));
                let (start_y, end_y) = (bound(base_y), bound(base_y.wrapping_add(64)));
                if entry.kind == 0 {
                    if start_x < end_x {
                        for y in start_y..end_y {
                            self.set(level, start_x, y, 0);
                        }
                    }
                } else if entry.kind == 1 {
                    for x in (0..64).step_by(4) {
                        for y in (0..64).step_by(4) {
                            let value = entry.heights.get((x / 4 * 16 + y / 4) as usize);
                            let value = value.copied().unwrap_or(0);
                            for px in tile_x.wrapping_add(x)..tile_x.wrapping_add(x + 4) {
                                for py in base_y.wrapping_add(y)..base_y.wrapping_add(y + 4) {
                                    self.set(level, px, py, value);
                                }
                            }
                        }
                    }
                } else if entry.kind == 2 && level > 0 && end_x > start_x {
                    for y in start_y..end_y {
                        let below = self.get(level - 1, start_x, y);
                        self.set(level, start_x, y, below);
                    }
                }
            }
        }
        if !records.is_empty() {
            return;
        }
        for level in 0..4 {
            for x in 0..16 {
                for y in 0..16 {
                    let (px, py) = ((tile_x >> 2) + x, y + (base_y >> 2));
                    if (0..26).contains(&px) && (0..26).contains(&py) {
                        self.set(level, px, py, 0);
                    }
                }
            }
        }
    }

    /// The occlusion heights of an ordinary scene whose base tile is `origin`,
    /// read in the original's order: the map files of [`scene_regions`], those
    /// present in `regions`.
    ///
    /// An instanced scene (`SceneGraph.readDynamicTerrain`) is not laid out
    /// here: its regions are assembled chunk by chunk and carry no records, so
    /// its heights are all zero.
    pub fn scene(origin: [i32; 2], regions: &[Region]) -> Self {
        let mut out = Self::default();
        for [x, z] in scene_regions(origin) {
            if let Some(region) = regions.iter().find(|r| r.x == x && r.z == z) {
                out.read(
                    x.wrapping_mul(64).wrapping_sub(origin[0]),
                    z.wrapping_mul(64).wrapping_sub(origin[1]),
                    &region.occlusion,
                );
            }
        }
        // `LoginManager.rebuildMap` then reads the underwater maps through
        // the same `readTerrain` (with the GL renderer and high water detail,
        // when any region of the scene has an underwater file): their
        // records land in the same heights, and a map without any (every one
        // of the pinned cache) clears its block of them again.
        let layers: Vec<_> = scene_regions(origin)
            .into_iter()
            .filter_map(|[x, z]| {
                let region = regions.iter().find(|r| r.x == x && r.z == z)?;
                Some(([x, z], region.underwater.layer()?))
            })
            .collect();
        if crate::materials::high_water_detail() {
            for ([x, z], layer) in layers {
                if layer.tiles.is_some() {
                    out.read(
                        x.wrapping_mul(64).wrapping_sub(origin[0]),
                        z.wrapping_mul(64).wrapping_sub(origin[1]),
                        &layer.occlusion,
                    );
                }
            }
        }
        out
    }
}

/// The map files of the scene whose base tile is `origin`, in the order
/// `Protocol.readRebuildPacket` lists and `LoginManager.readStaticTerrain`
/// reads them, without the files the original leaves out around the tutorial.
pub fn scene_regions(origin: [i32; 2]) -> Vec<[i32; 2]> {
    let zone = origin.map(|n| (n >> 3).wrapping_add(6));
    let blacklisted = ((zone[0] / 8 == 48 || zone[0] / 8 == 49) && zone[1] / 8 == 48)
        || (zone[0] / 8 == 48 && zone[1] / 8 == 148);
    let mut out = Vec::new();
    for x in zone[0].wrapping_sub(6) / 8..=zone[0].wrapping_add(6) / 8 {
        for y in zone[1].wrapping_sub(6) / 8..=zone[1].wrapping_add(6) / 8 {
            let left_out = y == 49 || y == 149 || y == 147 || x == 50 || (x == 49 && y == 47);
            if !(blacklisted && left_out) {
                out.push([x, y]);
            }
        }
    }
    out
}

/// The prepared regions as the camera's scene.
pub struct RegionScene<'a> {
    pub regions: &'a [Region],
    /// The scene base in tiles.
    pub origin: [i32; 2],
    pub occlusion: &'a Occlusion,
}

impl RegionScene<'_> {
    fn tile(&self, level: i32, x: i32, y: i32) -> Tile {
        if !(0..4).contains(&level) {
            return Tile::default();
        }
        let (x, z) = (
            self.origin[0].wrapping_add(x),
            self.origin[1].wrapping_add(y),
        );
        self.regions
            .iter()
            .find(|r| r.x == x.div_euclid(64) && r.z == z.div_euclid(64))
            .map_or(Tile::default(), |r| {
                r.tile(
                    level as usize,
                    x.rem_euclid(64) as usize,
                    z.rem_euclid(64) as usize,
                )
            })
    }
}

impl Scene for RegionScene<'_> {
    fn ground(&self, plane: i32, fine: [i32; 2]) -> i32 {
        crate::terrain::scene_height(
            self.regions,
            self.origin,
            plane,
            [0, 1].map(|axis| fine[axis].wrapping_add(self.origin[axis].wrapping_mul(128))),
        )
    }
    fn flags(&self, level: i32, x: i32, y: i32) -> u8 {
        self.tile(level, x, y).flags
    }
    fn height(&self, level: i32, x: i32, y: i32) -> i32 {
        self.tile(level, x, y).height
    }
    fn occlusion(&self, level: i32, x: i32, y: i32) -> u8 {
        self.occlusion.get(level, x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat;
    impl Scene for Flat {
        fn ground(&self, _: i32, _: [i32; 2]) -> i32 {
            -200
        }
        fn flags(&self, _: i32, _: i32, _: i32) -> u8 {
            0
        }
        fn height(&self, _: i32, _: i32, _: i32) -> i32 {
            -200
        }
        fn occlusion(&self, _: i32, _: i32, _: i32) -> u8 {
            0
        }
    }

    // Transcription tests: the expected values are worked out by hand from the
    // Java. The JVM differential is tests/camera_reference.rs.

    #[test]
    fn held_arrows_turn_324_and_576_units_a_second_within_the_pitch_limits() {
        let mut camera = Follow::default();
        let held = Held {
            up: true,
            right: true,
            ..Held::default()
        };
        // Half a second: 18 * 18 / 2 = 162 up and 24 * 24 / 2 = 288 right.
        camera.keys(500_000_000, held, 0, &Flat);
        assert_eq!((camera.pitch_target, camera.yaw_target), (290.0, 288.0));
        camera.keys(500_000_000, held, 0, &Flat);
        assert_eq!((camera.pitch_target, camera.yaw_target), (383.0, 576.0));
        // Up wins over down and left over right, as the else-if chains have it.
        let all = Held {
            up: true,
            down: true,
            left: true,
            right: true,
        };
        camera.keys(1_000_000_000, all, 0, &Flat);
        assert_eq!((camera.pitch_target, camera.yaw_target), (383.0, 0.0));
        // The yaw wraps at 2047, not 2048.
        camera.keys(
            250_000_000,
            Held {
                left: true,
                ..Held::default()
            },
            0,
            &Flat,
        );
        assert_eq!(camera.yaw_target, 2047.0 - 144.0);
        // No key held: nothing moves, not even the terrain minimum.
        let before = camera.clone();
        camera.keys(1_000_000_000, Held::default(), 0, &Flat);
        assert_eq!(camera, before);
    }

    #[test]
    fn the_orbit_point_eases_by_a_sixteenth_and_snaps_beyond_500() {
        let mut camera = Follow::default();
        camera.cycle([6400, 6400], 0, None, &Flat);
        assert_eq!(camera.position, [6400, 6400], "a far player is snapped to");
        camera.cycle([6464, 6300], 0, None, &Flat);
        // +64 / 16 = 4; -100 / 16 truncates toward zero to -6.
        assert_eq!(camera.position, [6404, 6394]);
        // Within sixteen units the division gives nothing: the lag stays.
        camera.cycle([6419, 6394], 0, None, &Flat);
        assert_eq!(camera.position, [6404, 6394]);
        camera.cycle([6404 + 501, 6394], 0, None, &Flat);
        assert_eq!(camera.position, [6905, 6394]);
        // The drift offsets move the point it eases toward.
        camera.offset = [32, -16];
        camera.cycle([6905, 6394], 0, None, &Flat);
        assert_eq!(camera.position, [6907, 6393]);
    }

    #[test]
    fn typed_arrows_step_the_targets_when_the_keyboard_camera_is_on() {
        let mut camera = Follow {
            pitch_target: 200.0,
            yaw_target: 100.0,
            ..Follow::default()
        };
        let keys = [KEY_UP, KEY_DOWN, KEY_DOWN, KEY_LEFT, KEY_RIGHT, 84];
        camera.cycle([0, 0], 0, None, &Flat);
        assert_eq!((camera.pitch_target, camera.yaw_target), (200.0, 100.0));
        camera.cycle([0, 0], 0, Some(&keys), &Flat);
        // 200 + 47 - 17 - 17 and 100 - 65 + 191.
        assert_eq!((camera.pitch_target, camera.yaw_target), (213.0, 226.0));
    }

    #[test]
    fn terrain_minimum_pitch_rises_by_a_24th_and_falls_by_an_80th() {
        struct Hill(i32);
        impl Scene for Hill {
            fn ground(&self, _: i32, _: [i32; 2]) -> i32 {
                -200
            }
            fn flags(&self, level: i32, x: i32, y: i32) -> u8 {
                // A bridge tile: the level above is sampled there.
                if level == 1 && [x, y] == [50, 54] {
                    2
                } else {
                    0
                }
            }
            fn height(&self, level: i32, x: i32, y: i32) -> i32 {
                match (level, x, y) {
                    (0, 54, 46) => -200 - self.0,
                    (1, 50, 54) => -200 - self.0 - 40,
                    _ => -200,
                }
            }
            fn occlusion(&self, level: i32, x: i32, y: i32) -> u8 {
                // 255 is read unsigned: 2040 above its tile.
                if (level, x, y) == (0, 46, 46) { 255 } else { 0 }
            }
        }
        let mut camera = Follow {
            position: [50 * 128 + 64, 50 * 128 + 64],
            ..Follow::default()
        };
        // No hill: 255 * 8 = 2040 from the occlusion byte; * 192 is far above
        // the cap of 98048, so the first step is 98048 / 24.
        camera.clamp(0, &Hill(0));
        assert_eq!(camera.max_pitch_distance, 98048 / 24);
        // Level 1 holds neither the hill nor the occlusion byte: only the
        // bridge tile's 40 counts, which is under the floor of 32768.
        let mut upper = Follow {
            position: camera.position,
            ..Follow::default()
        };
        upper.clamp(1, &Hill(300));
        assert_eq!(upper.max_pitch_distance, 32768 / 24);
        // The 9 x 9 window ends at tile +-4: a camera one tile east loses the
        // occlusion column and keeps the hill and the bridge tile.
        let mut east = Follow {
            position: [51 * 128, 50 * 128],
            max_pitch_distance: 98048,
            ..Follow::default()
        };
        east.clamp(0, &Hill(300));
        // max(300, 340) * 192 = 65280; falling: (65280 - 98048) / 80 = -409.
        assert_eq!(east.max_pitch_distance, 98048 - 409);
        // Within three tiles of the scene edge nothing is sampled.
        let mut edge = Follow {
            position: [3 * 128, 50 * 128],
            max_pitch_distance: 98048,
            ..Follow::default()
        };
        edge.clamp(0, &Hill(300));
        assert_eq!(edge.max_pitch_distance, 98048 + (32768 - 98048) / 80);
    }

    #[test]
    fn the_pose_sits_zoom_plus_three_pitch_behind_the_orbit_point() {
        let camera = Follow {
            pitch_target: 128.9,
            yaw_target: 0.0,
            position: [6464, 6464],
            max_pitch_distance: 0,
            ..Follow::default()
        };
        // Viewport 334 high: view distance 256, so 600 + 128 * 3 = 984.
        let pose = camera.pose(334, [256, 320], [6464, 6464], 0, None, &Flat);
        assert_eq!((pose.pitch, pose.yaw), (128, 0));
        // sin(2048 - 128) = -25079, cos = 60547 (16.16 of 22.5 degrees).
        // y: -25079 * -984 >> 16 = 376; z: 60547 * 984 >> 16 = 909.
        assert_eq!(pose.position, [6464, 6464 - 909, -250 - 376]);
        // 434 high and above: the distance grows by 320 / 256.
        let tall = camera.pose(900, [256, 320], [6464, 6464], 0, None, &Flat);
        // 320 * 984 >> 8 = 1230: y = 470 (25079 * 1230 >> 16), z = 1136.
        assert_eq!(tall.position, [6464, 6464 - 1136, -250 - 470]);
        // The terrain minimum and the shake floor raise the pitch, never lower.
        let lifted = Follow {
            max_pitch_distance: 300 * 256 + 255,
            yaw_drift: -5,
            ..camera.clone()
        };
        let pose = lifted.pose(334, [256, 320], [6464, 6464], 0, Some(250), &Flat);
        assert_eq!((pose.pitch, pose.yaw), (300, 2043));
        let pose = lifted.pose(334, [256, 320], [6464, 6464], 0, Some(383), &Flat);
        assert_eq!(pose.pitch, 383);
    }

    #[test]
    fn drift_steps_every_501st_cycle_and_turns_round_at_its_limits() {
        let mut camera = Follow::default();
        let mut draws = 0;
        for _ in 0..500 {
            camera.drift(|| unreachable!("no draw before the timer passes 500"));
        }
        camera.drift(|| {
            draws += 1;
            0.999
        });
        assert_eq!(draws, 1);
        assert_eq!((camera.offset, camera.yaw_drift), ([2, 2], 1));
        assert_eq!(camera.jitter_timer, 0);
        // Bit 1 alone moves the y offset; past 55 its step turns negative.
        camera.offset = [50, 54];
        camera.jitter_timer = 500;
        camera.drift(|| 2.5 / 8.0);
        assert_eq!(camera.offset, [50, 56]);
        assert_eq!(camera.offset_delta, [2, -2]);
        camera.yaw_drift = -41;
        camera.yaw_drift_delta = -1;
        camera.drift(|| unreachable!());
        assert_eq!(camera.yaw_drift_delta, 1);
    }

    #[test]
    fn login_draws_offsets_and_a_yaw_near_north() {
        let mut camera = Follow::default();
        let mut values = [0.0, 0.0, 0.999, 0.5].into_iter();
        camera.login(|| values.next().unwrap());
        // -50; (0 - 10) & 0x7FF = 2038; 109 - 55 = 54; 40 - 40 = 0.
        assert_eq!(camera.offset, [-50, 54]);
        assert_eq!((camera.yaw_target, camera.yaw_drift), (2038.0, 0));
        assert_eq!(camera.pitch_target, 128.0);
    }

    #[test]
    fn destination_flag_moves_with_the_scene_base() {
        // (That the flag goes out on arrival is `crowd::frame`, the port of
        // addPlayersToScene, checked against the JVM in tests/actors_oracle.rs.)
        let mut flag = [40, 52];
        assert!(rebuild(&mut flag, Some([3200, 3200]), [3216, 3192]));
        assert_eq!(flag, [24, 60]);
        // No flag: nothing to move, even though y is nonzero.
        let mut flag = [0, 52];
        rebuild(&mut flag, Some([3200, 3200]), [3216, 3192]);
        assert_eq!(flag, [0, 52]);
        // The same base again is not a rebuild: the scripted camera stays.
        let mut flag = [40, 52];
        assert!(!rebuild(&mut flag, Some([3200, 3200]), [3200, 3200]));
        assert!(rebuild(&mut flag, None, [3200, 3200]));
        assert_eq!(flag, [40, 52]);
    }

    #[test]
    fn scene_regions_follow_the_rebuild_packet_order() {
        // Lumbridge: zone (402, 402), base (3168, 3168); 396 / 8 to 408 / 8.
        assert_eq!(
            scene_regions([3168, 3168]),
            [
                [49, 49],
                [49, 50],
                [49, 51],
                [50, 49],
                [50, 50],
                [50, 51],
                [51, 49],
                [51, 50],
                [51, 51]
            ]
        );
        // A base on a region corner needs two files each way.
        assert_eq!(
            scene_regions([3200 - 48, 3264 - 48]),
            [[49, 50], [49, 51], [50, 50], [50, 51]]
        );
        // Around the tutorial (zone x / 8 of 48 or 49, y / 8 of 48) the
        // original leaves out row 49 and column 50.
        assert_eq!(scene_regions([3072, 3072]), [[48, 48], [49, 48]]);
    }

    fn level(kind: i8, fill: u8) -> OcclusionLevel {
        OcclusionLevel {
            kind,
            heights: if kind == 1 {
                (0..=255u8).map(|n| n ^ fill).collect()
            } else {
                vec![]
            },
        }
    }

    #[test]
    fn occlusion_blocks_cover_four_tiles_and_clip_to_the_scene() {
        let mut grid = Occlusion::default();
        let record = [level(1, 0), level(0, 0), level(2, 0), level(7, 0)];
        grid.read(-8, 96, std::slice::from_ref(&record));
        // Block (x 2, y 1) is byte 2 * 16 + 1 = 33 and covers scene tiles
        // x 0..4, y 100..104.
        assert_eq!(grid.get(0, 0, 100), 33);
        assert_eq!(grid.get(0, 3, 103), 33);
        assert_eq!(grid.get(0, 4, 103), 49);
        assert_eq!(grid.get(0, 3, 99), 32);
        // Type 2 copies the level below along the first column only, and here
        // level 1 was cleared (type 0), so level 2 stays empty.
        assert_eq!(grid.get(2, 0, 100), 0);
        let copy = [level(1, 255), level(2, 0), level(2, 0), level(0, 0)];
        let mut grid = Occlusion::default();
        grid.read(8, 16, std::slice::from_ref(&copy));
        assert_eq!(grid.get(0, 8, 16), 255);
        assert_eq!(grid.get(1, 8, 16), 255, "the first column is copied");
        assert_eq!(grid.get(1, 8, 79), 255 ^ 15);
        assert_eq!(grid.get(1, 9, 16), 0, "the other columns are not");
        assert_eq!(grid.get(2, 8, 40), grid.get(0, 8, 40));
        // A later file without records clears quarter-tile addresses.
        grid.read(72, 16, &[]);
        // (72 >> 2) = 18 .. 26 and (16 >> 2) = 4 .. 20.
        assert_eq!(grid.get(0, 18, 19), 0);
        assert_eq!(grid.get(0, 17, 19), 255 ^ 0x20, "block (2, 0)");
        assert_eq!(grid.get(0, 18, 20), 255 ^ 0x21, "block (2, 1)");
    }

    #[test]
    fn an_underwater_map_clears_the_heights_its_surface_map_set() {
        // Transcribed from LoginManager.rebuildMap and SceneGraph.readTerrain.
        let region = |underwater| Region {
            x: 50,
            z: 50,
            tiles: vec![Tile::default(); 4 * 64 * 64],
            environment: Default::default(),
            point_lights: vec![],
            chunk_environments: vec![],
            occlusion: vec![[level(1, 255), level(1, 255), level(0, 0), level(0, 0)]],
            underwater,
        };
        // Scene base (3168, 3168): the region starts at scene tile (32, 32).
        let origin = [3168, 3168];
        let surface = Occlusion::scene(origin, &[region(Default::default())]);
        assert_eq!(surface.get(0, 32, 32), 255);
        assert_eq!(surface.get(1, 95, 95), 0);
        let layer = crate::underwater::Layer {
            tiles: Some(vec![Tile::default(); 64 * 64]),
            ..Default::default()
        };
        let both = Occlusion::scene(origin, &[region(crate::underwater::Source::Files(layer))]);
        // The block cleared is addressed in quarter tiles: 32 >> 2 = 8 .. 24.
        assert_eq!(both.get(0, 8, 8), 0);
        assert_eq!(
            both.get(0, 32, 32),
            255,
            "tile 32 is not in 8..24 on both axes"
        );
        assert_eq!(surface.get(0, 32, 20), 0);
        let lower = Occlusion::scene([3200 - 8, 3200 - 8], &[region(Default::default())]);
        assert_eq!(lower.get(0, 8, 8), 255);
        let layer = crate::underwater::Layer {
            tiles: Some(vec![Tile::default(); 64 * 64]),
            ..Default::default()
        };
        let cleared = Occlusion::scene(
            [3200 - 8, 3200 - 8],
            &[region(crate::underwater::Source::Files(layer))],
        );
        // (8 >> 2) = 2 .. 18 on both axes.
        assert_eq!(cleared.get(0, 8, 8), 0);
        assert_eq!(cleared.get(0, 17, 17), 0);
        assert_eq!(cleared.get(0, 18, 18), lower.get(0, 18, 18));
        assert_ne!(lower.get(0, 17, 17), 0);
    }
}
