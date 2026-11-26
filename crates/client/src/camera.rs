//! Server-controlled camera: RT4 `Camera` type 2 (locked position and look-at)
//! and the five shake channels, from `Camera.java`, `Protocol.readPacketInternal`
//! and `ScriptRunner`'s per-frame shake application.
//!
//! Coordinates are scene-local fine units (128 per tile) with height negative
//! upward, and angles are 2048 units per turn, exactly as the original. The
//! renderer converts; no floating camera state lives here except the two
//! original `Math.atan2`/`Math.sqrt` calls.
use rs09_protocol::game::events::ServerEvent;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Shake {
    pub active: bool,
    pub jitter: i32,
    pub amplitude: i32,
    pub frequency: i32,
    pub phase: i32,
}

/// A camera pose: position `[x, y, height]`, then pitch and yaw.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pose {
    pub position: [i32; 3],
    pub pitch: i32,
    pub yaw: i32,
}

/// What a camera message asks of the caller beyond this state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Applied {
    /// The packet's verification counter (`setVerifyId`).
    pub counter: i32,
    /// CAM_FORCEANGLE's clamped `[yaw, pitch]` targets for the follow camera.
    pub angle: Option<[i32; 2]>,
}

/// `Camera.cameraType == 3`: the fly-through the login screen's scripts set up
/// with command 5502. The camera follows one cubic segment while looking at a
/// point on another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spline {
    /// Control points `[x, height, y]` in absolute fine units.
    pub position: [[i32; 3]; 4],
    pub look: [[i32; 3]; 4],
    pub speed_start: i32,
    pub speed_end: i32,
    /// 0 to 65535 along the segment.
    pub progress: i32,
    pub finished: bool,
    /// True for the one update on which the segment is completed.
    pub just_finished: bool,
    pitch_radians: u32,
    yaw_radians: u32,
}
impl Spline {
    /// `Camera.pitchRadians`: the HD renderer aims with this, not the 2048-step
    /// angle, while the spline camera is in charge.
    pub fn pitch_radians(&self) -> f32 {
        f32::from_bits(self.pitch_radians)
    }
    pub fn yaw_radians(&self) -> f32 {
        f32::from_bits(self.yaw_radians)
    }
    /// The curve through one coordinate of four control points at `t`.
    fn curve(points: &[[i32; 3]; 4], axis: usize, t: f32) -> f32 {
        let p = points.map(|point| point[axis]);
        let c3 = p[0];
        let c0 = p[0].wrapping_mul(3);
        let c1 = p[1].wrapping_mul(3);
        let c2 = p[2].wrapping_add(p[2]).wrapping_sub(p[3]).wrapping_mul(3);
        let linear = c1.wrapping_sub(c0);
        let quadratic = c2.wrapping_add(c0).wrapping_sub(c1.wrapping_mul(2));
        let cubic = p[2].wrapping_add(c1).wrapping_sub(c3).wrapping_sub(c2);
        c3 as f32 + ((cubic as f32 * t + quadratic as f32) * t + linear as f32) * t
    }
}

/// `Camera.splineJustFinished` as the interface walk reads it
/// (`InterfaceList` 1035, the `onMinimapUnlock` hooks).
///
/// The original's flag is a static that only `updateLoginScreenCamera`
/// writes, and only past its early return: it is true from the update that
/// completes the path until the next update of a spline camera, so it stays
/// true if the camera leaves the spline before that. The walk of the
/// following cycle reads it.
///
/// This client runs a frame's walks before its camera updates instead of
/// alternating them, so the walks cannot simply read the flag: a completion
/// is remembered until one walk has seen it, and a second walk before the
/// next update reads what that update would leave (false while the spline
/// camera is still there to clear it).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SplineFinished {
    /// The original's flag after the latest update.
    flag: bool,
    /// A completion no walk has read yet.
    unseen: bool,
    /// The latest update found a spline camera.
    active: bool,
}
impl SplineFinished {
    /// After [`Script::spline_cycle`]. Without a spline the original returns
    /// before touching the flag.
    pub fn updated(&mut self, script: &Script) {
        self.active = script.spline.is_some();
        if let Some(spline) = &script.spline {
            self.flag = spline.just_finished;
            self.unseen |= spline.just_finished;
        }
    }
    /// The flag for one interface walk.
    pub fn read(&mut self) -> bool {
        std::mem::take(&mut self.unseen) || self.flag && !self.active
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Script {
    /// `Camera.cameraType == 3`.
    pub spline: Option<Spline>,
    /// `Camera.cameraType == 2`.
    pub locked: bool,
    pub render: [i32; 3],
    pub pitch: i32,
    pub yaw: i32,
    target: [i32; 2],
    target_height: i32,
    move_speed: i32,
    min_move_step: i32,
    look_at: [i32; 2],
    look_at_height: i32,
    angle_speed: i32,
    min_step: i32,
    pub shakes: [Shake; 5],
}

fn clamp_pitch(pitch: i32) -> i32 {
    pitch.clamp(128, 383)
}
/// `(int) (Math.atan2(dz, distance) * 325.949) & 0x7FF`, clamped; and the yaw.
fn angles(from: [i32; 3], to: [i32; 3]) -> [i32; 2] {
    let dx = to[0].wrapping_sub(from[0]);
    let dy = to[1].wrapping_sub(from[1]);
    let dz = to[2].wrapping_sub(from[2]);
    let distance = f64::from(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))).sqrt() as i32;
    let pitch = ((f64::from(dz).atan2(f64::from(distance)) * 325.949) as i32) & 0x7ff;
    let yaw = ((f64::from(dx).atan2(f64::from(dy)) * -325.949) as i32) & 0x7ff;
    [clamp_pitch(pitch), yaw]
}
fn centre(tile: [i32; 2]) -> [i32; 2] {
    tile.map(|n| n.wrapping_mul(128).wrapping_add(64))
}

impl Script {
    /// While the follow camera is in charge the renderer reports its pose, so a
    /// later lock eases from where the view actually is.
    pub fn follow(&mut self, pose: Pose) {
        if !self.locked && self.spline.is_none() {
            self.render = pose.position;
            self.pitch = pose.pitch;
            self.yaw = pose.yaw;
        }
    }

    /// `height` is `SceneGraph.getTileHeight(Player.plane, x, y)` for local
    /// fine coordinates. Returns None for messages that are not camera messages.
    pub fn apply(
        &mut self,
        event: &ServerEvent,
        mut height: impl FnMut([i32; 2]) -> i32,
    ) -> Option<Applied> {
        let mut angle = None;
        let counter = match *event {
            ServerEvent::CameraReset { counter } => {
                for shake in &mut self.shakes {
                    shake.active = false;
                }
                self.min_step = 0;
                self.angle_speed = 0;
                self.locked = false;
                self.spline = None;
                counter
            }
            ServerEvent::CameraShake {
                counter,
                axis,
                jitter,
                amplitude,
                frequency,
                phase,
            } => {
                // The original indexes five-entry arrays; other axes are ignored
                // here instead of raising its ArrayIndexOutOfBoundsException.
                if let Some(shake) = usize::try_from(axis)
                    .ok()
                    .and_then(|axis| self.shakes.get_mut(axis))
                {
                    *shake = Shake {
                        active: true,
                        jitter,
                        amplitude,
                        frequency,
                        phase,
                    };
                }
                counter
            }
            ServerEvent::CameraLookAt {
                counter,
                x,
                y,
                height: offset,
                min_step,
                speed,
            } => {
                self.min_step = min_step;
                self.look_at = [x, y];
                self.angle_speed = speed;
                self.look_at_height = offset;
                if speed >= 100 {
                    let fine = centre(self.look_at);
                    let z = height(fine).wrapping_sub(offset);
                    [self.pitch, self.yaw] = angles(self.render, [fine[0], fine[1], z]);
                }
                self.locked = true;
                counter
            }
            ServerEvent::CameraMoveTo {
                counter,
                x,
                y,
                height: offset,
                min_step,
                speed,
            } => {
                self.move_speed = speed;
                self.target_height = offset;
                self.target = [x, y];
                self.min_move_step = min_step;
                if speed >= 100 {
                    let fine = centre(self.target);
                    self.render = [fine[0], fine[1], height(fine).wrapping_sub(offset)];
                }
                self.locked = true;
                counter
            }
            ServerEvent::CameraAngle {
                counter,
                yaw,
                pitch,
            } => {
                // clampCameraAngle: pitch 128..=383 and Camera.mod(yaw, 2047.0).
                let pitch = clamp_pitch(pitch);
                let yaw = ((f64::from(yaw) % 2047.0 + 2047.0) % 2047.0) as i32;
                if self.locked {
                    self.pitch = pitch;
                    self.yaw = yaw;
                }
                angle = Some([yaw, pitch]);
                counter
            }
            _ => return None,
        };
        Some(Applied { counter, angle })
    }

    /// Script camera commands (CS2 5500 to 5504). They mirror the server
    /// messages except that a scripted move never snaps to its target, since
    /// the original passes `initialize = false`. Tiles are absolute; `origin`
    /// is the scene base. Returns the clamped `[yaw, pitch]` of an angle request.
    pub fn script(
        &mut self,
        request: crate::ui::vm::CameraRequest,
        origin: [i32; 2],
        height: impl FnMut([i32; 2]) -> i32,
    ) -> Option<[i32; 2]> {
        use crate::ui::vm::CameraRequest;
        let local = |tile: [i32; 2]| [tile[0] - origin[0], tile[1] - origin[1]];
        let event = match request {
            CameraRequest::MoveTo {
                tile,
                height,
                min_step,
                speed,
            } => {
                self.move_speed = speed;
                self.target_height = height;
                self.target = local(tile);
                self.min_move_step = min_step;
                self.locked = true;
                self.spline = None;
                return None;
            }
            CameraRequest::Spline {
                position,
                look,
                speed_start,
                speed_end,
            } => {
                self.spline = Some(Spline {
                    position,
                    look,
                    speed_start,
                    speed_end,
                    progress: 0,
                    finished: false,
                    just_finished: false,
                    pitch_radians: 0,
                    yaw_radians: 0,
                });
                self.locked = false;
                return None;
            }
            CameraRequest::LookAt {
                tile,
                height,
                min_step,
                speed,
            } => {
                let [x, y] = local(tile);
                ServerEvent::CameraLookAt {
                    counter: 0,
                    x,
                    y,
                    height,
                    min_step,
                    speed,
                }
            }
            CameraRequest::Unlock => ServerEvent::CameraReset { counter: 0 },
            CameraRequest::Angle { pitch, yaw } => ServerEvent::CameraAngle {
                counter: 0,
                yaw,
                pitch,
            },
        };
        self.apply(&event, height).and_then(|applied| applied.angle)
    }

    /// `InterfaceList.resetToLoginScreen` (InterfaceList.java:1113-1118),
    /// before it sets up the title scene: a locked camera is put on its
    /// target tile (`renderX = lockedTargetX << 7`, the tile's corner, with
    /// the height left as it was); any other camera gets one spline update.
    pub fn title_reset(&mut self, origin: [i32; 2]) {
        if self.locked {
            self.render[0] = self.target[0] << 7;
            self.render[1] = self.target[1] << 7;
        } else {
            self.spline_cycle(origin);
        }
    }

    /// `LoginManager.loadRegion` with `fromLogin` (LoginManager.java:
    /// 806-812): the scene base moved by `delta` tiles, and the camera and
    /// the locked camera's targets move back by as much so that they stay
    /// where they are in the world. The camera's type is kept.
    pub fn rebase(&mut self, delta: [i32; 2]) {
        #[allow(clippy::needless_range_loop)]
        for axis in 0..2 {
            self.render[axis] = self.render[axis].wrapping_sub(delta[axis].wrapping_mul(128));
            self.target[axis] = self.target[axis].wrapping_sub(delta[axis]);
            self.look_at[axis] = self.look_at[axis].wrapping_sub(delta[axis]);
        }
    }

    /// `updateLoginScreenCamera`: advance the spline camera by one cycle. The
    /// original runs this every cycle the camera type is neither 1 nor 2: on
    /// the login screen (`client.java` 1203-1206) and in game
    /// (`Protocol.loop` 2895-2901). `origin` is the scene base in tiles.
    pub fn spline_cycle(&mut self, origin: [i32; 2]) {
        let Some(spline) = &mut self.spline else {
            return;
        };
        let speed = (spline
            .progress
            .wrapping_mul(spline.speed_end.wrapping_sub(spline.speed_start))
            >> 16)
            .wrapping_add(spline.speed_start);
        spline.progress = spline.progress.wrapping_add(speed);
        if spline.progress >= 65535 {
            spline.progress = 65535;
            spline.just_finished = !spline.finished;
            spline.finished = true;
        } else {
            spline.finished = false;
            spline.just_finished = false;
        }
        let t = spline.progress as f32 / 65535.0;
        let at: [f32; 3] = std::array::from_fn(|axis| Spline::curve(&spline.position, axis, t));
        let look: [f32; 3] = std::array::from_fn(|axis| Spline::curve(&spline.look, axis, t));
        self.render = [
            (at[0] as i32).wrapping_sub(origin[0].wrapping_mul(128)),
            (at[2] as i32).wrapping_sub(origin[1].wrapping_mul(128)),
            (at[1] as i32).wrapping_neg(),
        ];
        let dx = look[0] - at[0];
        let dz = look[2] - at[2];
        let dy = -(look[1] - at[1]);
        let distance = f64::from(dz * dz + dx * dx).sqrt();
        let pitch = f64::from(dy).atan2(distance) as f32;
        let yaw = -(f64::from(dx).atan2(f64::from(dz)) as f32);
        spline.pitch_radians = pitch.to_bits();
        spline.yaw_radians = yaw.to_bits();
        self.pitch = ((f64::from(pitch) * 325.949) as i32) & 0x7ff;
        self.yaw = ((f64::from(yaw) * 325.949) as i32) & 0x7ff;
    }

    /// One client cycle: `updateLockedCamera` while locked, then every shake
    /// phase advances whether or not its channel is active.
    pub fn cycle(&mut self, mut height: impl FnMut([i32; 2]) -> i32) {
        if self.locked {
            let fine = centre(self.target);
            let goal = [
                fine[0],
                fine[1],
                height(fine).wrapping_sub(self.target_height),
            ];
            if self.move_speed >= 100 {
                self.render = goal;
            } else {
                // Original axis order: x up, z up, x down, y up, z down, y down.
                let (speed, step) = (self.move_speed, self.min_move_step);
                let toward = |from: &mut i32, to: i32, rising: bool| {
                    if rising && *from < to {
                        *from += step + speed * (to - *from) / 1000;
                        if *from > to {
                            *from = to;
                        }
                    }
                    if !rising && *from > to {
                        *from -= step + (*from - to) * speed / 1000;
                        if *from < to {
                            *from = to;
                        }
                    }
                };
                toward(&mut self.render[0], goal[0], true);
                toward(&mut self.render[2], goal[2], true);
                toward(&mut self.render[0], goal[0], false);
                toward(&mut self.render[1], goal[1], true);
                toward(&mut self.render[2], goal[2], false);
                toward(&mut self.render[1], goal[1], false);
            }
            let fine = centre(self.look_at);
            let [target_pitch, target_yaw] = angles(
                self.render,
                [
                    fine[0],
                    fine[1],
                    height(fine).wrapping_sub(self.look_at_height),
                ],
            );
            let (speed, step) = (self.angle_speed, self.min_step);
            if self.pitch < target_pitch {
                self.pitch += step + speed * (target_pitch - self.pitch) / 1000;
                if self.pitch > target_pitch {
                    self.pitch = target_pitch;
                }
            }
            if self.pitch > target_pitch {
                self.pitch -= (self.pitch - target_pitch) * speed / 1000 + step;
                if self.pitch < target_pitch {
                    self.pitch = target_pitch;
                }
            }
            let wrap = |mut delta: i32| {
                if delta > 1024 {
                    delta -= 2048;
                }
                if delta < -1024 {
                    delta += 2048;
                }
                delta
            };
            let delta = wrap(target_yaw - self.yaw);
            if delta > 0 {
                self.yaw = (self.yaw + delta * speed / 1000 + step) & 0x7ff;
            }
            if delta < 0 {
                self.yaw = (self.yaw - (speed * -delta / 1000 + step)) & 0x7ff;
            }
            let after = wrap(target_yaw - self.yaw);
            if (after < 0 && delta > 0) || (after > 0 && delta < 0) {
                self.yaw = target_yaw;
            }
        }
        for shake in &mut self.shakes {
            shake.phase = shake.phase.wrapping_add(1);
        }
    }

    /// The pose to draw this frame: channels 0, 1 and 2 displace x, height and
    /// y; 3 turns the yaw; 4 tilts the pitch within its clamp. `random` is
    /// `Math.random()`, drawn once per active channel in ascending order.
    pub fn shaken(&self, pose: Pose, mut random: impl FnMut() -> f64) -> Pose {
        let mut out = pose;
        for (axis, shake) in self.shakes.iter().enumerate() {
            if !shake.active {
                continue;
            }
            let offset = (-f64::from(shake.jitter)
                + f64::from(shake.jitter * 2 + 1) * random()
                + (f64::from(shake.phase) * (f64::from(shake.frequency) / 100.0)).sin()
                    * f64::from(shake.amplitude)) as i32;
            match axis {
                0 => out.position[0] += offset,
                1 => out.position[2] += offset,
                2 => out.position[1] += offset,
                3 => out.yaw = (offset + out.yaw) & 0x7ff,
                _ => out.pitch = clamp_pitch(out.pitch + offset),
            }
        }
        out
    }

    /// The follow camera never tilts below this while channel 4 is shaking.
    pub fn pitch_floor(&self) -> Option<i32> {
        self.shakes[4]
            .active
            .then_some(self.shakes[4].amplitude + 128)
    }
    /// `Camera.lockedTargetX/Y` and `lockedLookAtX/Y`, scene-local tiles.
    pub fn locked_tiles(&self) -> [[i32; 2]; 2] {
        [self.target, self.look_at]
    }
    pub fn pose(&self) -> Pose {
        Pose {
            position: self.render,
            pitch: self.pitch,
            yaw: self.yaw,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(_: [i32; 2]) -> i32 {
        -200
    }

    /// Transcription test: `Camera.updateLoginScreenCamera` 250-263 as
    /// `InterfaceList` 1035 reads it on the following cycle's walk.
    #[test]
    fn the_finish_flag_is_seen_by_the_next_walk_and_outlives_the_spline() {
        use crate::ui::vm::CameraRequest;
        let points = [[3200 * 128, 500, 3200 * 128]; 4];
        let spline = CameraRequest::Spline {
            position: points,
            look: points,
            speed_start: 40000,
            speed_end: 40000,
        };
        let mut camera = Script::default();
        let mut finished = SplineFinished::default();
        let update = |camera: &mut Script, finished: &mut SplineFinished| {
            camera.spline_cycle([3168, 3168]);
            finished.updated(camera);
        };
        // No spline: the original returns before it touches the flag.
        update(&mut camera, &mut finished);
        assert!(!finished.read());
        camera.script(spline, [3168, 3168], flat);
        update(&mut camera, &mut finished);
        assert!(!finished.read(), "progress 40000");
        update(&mut camera, &mut finished);
        assert!(finished.read(), "completed");
        assert!(!finished.read(), "the next update would clear it");
        update(&mut camera, &mut finished);
        assert!(!finished.read(), "true for one update only");
        // Two updates between walks: the completion is still seen once.
        camera.script(spline, [3168, 3168], flat);
        for _ in 0..3 {
            update(&mut camera, &mut finished);
        }
        assert!(finished.read());
        assert!(!finished.read());
        // The camera is unlocked on the cycle the path completes: nothing
        // resets the original's flag, so every later walk still reads it.
        camera.script(spline, [3168, 3168], flat);
        update(&mut camera, &mut finished);
        update(&mut camera, &mut finished);
        camera.script(CameraRequest::Unlock, [3168, 3168], flat);
        assert!(camera.spline.is_none());
        for _ in 0..3 {
            update(&mut camera, &mut finished);
            assert!(finished.read());
        }
    }

    #[test]
    fn spline_camera_follows_its_segment_and_reports_the_finish() {
        use crate::ui::vm::CameraRequest;
        let fine = |x: i32, height: i32, y: i32| [x * 128, height, y * 128];
        // Both controls sit on the end points, so the segment runs from the
        // first point to the third; the fourth mirrors the second control.
        let position = [
            fine(3200, 500, 3200),
            fine(3200, 500, 3200),
            fine(3210, 300, 3200),
            fine(3210, 300, 3200),
        ];
        let north = [fine(3210, 300, 3220); 4];
        let mut camera = Script::default();
        camera.script(
            CameraRequest::Spline {
                position,
                look: north,
                speed_start: 65535,
                speed_end: 0,
            },
            [3168, 3168],
            flat,
        );
        assert!(!camera.locked && camera.spline.is_some());
        camera.spline_cycle([3168, 3168]);
        let spline = camera.spline.unwrap();
        assert!(spline.finished && spline.just_finished);
        // The end of the segment, scene-local, with height negated.
        assert_eq!(camera.render, [42 * 128, 32 * 128, -300]);
        // Looking level and due north.
        assert_eq!([camera.pitch, camera.yaw], [0, 0]);
        assert_eq!([spline.pitch_radians(), spline.yaw_radians()], [0.0, 0.0]);
        camera.spline_cycle([3168, 3168]);
        let spline = camera.spline.unwrap();
        assert!(spline.finished && !spline.just_finished);
        assert_eq!(spline.progress, 65535);

        // Ten tiles east and 1280 units above: 45 degrees up, facing east.
        let east_up = [fine(3220, 1580, 3200); 4];
        camera.script(
            CameraRequest::Spline {
                position,
                look: east_up,
                speed_start: 65535,
                speed_end: 0,
            },
            [3168, 3168],
            flat,
        );
        camera.spline_cycle([3168, 3168]);
        // (int) (-pi/4 * 325.949) = -255 and (int) (-pi/2 * 325.949) = -511.
        assert_eq!([camera.pitch, camera.yaw], [2048 - 255, 2048 - 511]);

        // The speed eases from its start value toward its end value.
        camera.script(
            CameraRequest::Spline {
                position,
                look: north,
                speed_start: 1000,
                speed_end: 3000,
            },
            [3168, 3168],
            flat,
        );
        camera.spline_cycle([3168, 3168]);
        assert_eq!(camera.spline.unwrap().progress, 1000);
        camera.spline_cycle([3168, 3168]);
        // (1000 * 2000 >> 16) + 1000 = 1030.
        assert_eq!(camera.spline.unwrap().progress, 2030);
        assert!(!camera.spline.unwrap().finished);
        // Unlocking ends it; so does a locked camera request.
        camera.script(CameraRequest::Unlock, [3168, 3168], flat);
        assert!(camera.spline.is_none());
    }
    fn move_to(x: i32, y: i32, height: i32, min_step: i32, speed: i32) -> ServerEvent {
        ServerEvent::CameraMoveTo {
            counter: 7,
            x,
            y,
            height,
            min_step,
            speed,
        }
    }
    fn look_at(x: i32, y: i32, height: i32, min_step: i32, speed: i32) -> ServerEvent {
        ServerEvent::CameraLookAt {
            counter: 8,
            x,
            y,
            height,
            min_step,
            speed,
        }
    }

    #[test]
    fn instant_move_and_look_snap_to_the_tile_centres() {
        let mut camera = Script::default();
        let applied = camera.apply(&move_to(50, 60, 300, 0, 100), flat).unwrap();
        assert_eq!(
            applied,
            Applied {
                counter: 7,
                angle: None
            }
        );
        assert!(camera.locked);
        // Tile centre, and the height offset is subtracted (up is negative).
        assert_eq!(camera.render, [50 * 128 + 64, 60 * 128 + 64, -500]);

        // Looking due north at a point 500 below the camera's own height.
        camera.apply(&look_at(50, 70, -200, 0, 100), flat).unwrap();
        // dz = (-200 - -200) - -500 = 500, distance = 1280:
        // atan2(500, 1280) * 325.949 = 121.4, below the 128 floor.
        assert_eq!((camera.pitch, camera.yaw), (128, 0));
        // Due east is -atan2(dx, 0) * 325.949 = -511.9995, which the int cast
        // truncates to -511 before the mask: 1537, not 1536.
        camera.apply(&look_at(60, 60, -200, 0, 100), flat).unwrap();
        assert_eq!(camera.yaw, 1537);
        // A steep look down is capped at 383.
        camera.apply(&look_at(50, 60, -200, 0, 100), flat).unwrap();
        assert_eq!(camera.pitch, 383);
    }

    #[test]
    fn slow_move_eases_each_axis_and_stops_exactly_on_the_target() {
        let mut camera = Script::default();
        camera.follow(Pose {
            position: [1000, 2000, -300],
            pitch: 200,
            yaw: 100,
        });
        // Target fine (6464, 1344, -700): x rises, y falls, height falls.
        camera.apply(&move_to(50, 10, 500, 2, 10), flat).unwrap();
        assert_eq!(
            camera.render,
            [1000, 2000, -300],
            "a slow move does not snap"
        );
        camera.apply(&look_at(50, 10, 0, 0, 0), flat).unwrap();
        camera.cycle(flat);
        // x: 1000 + 2 + 10 * 5464 / 1000 = 1056.
        // z: -300 - ((-300 - -700) * 10 / 1000 + 2) = -306.
        // y: 2000 - (2 + 656 * 10 / 1000) = 1992.
        assert_eq!(camera.render, [1056, 1992, -306]);
        for _ in 0..5000 {
            camera.cycle(flat);
        }
        assert_eq!(camera.render, [6464, 1344, -700]);
    }

    #[test]
    fn yaw_turns_the_short_way_and_snaps_when_it_would_overshoot() {
        let mut camera = Script::default();
        camera.apply(&move_to(50, 50, 0, 0, 100), flat).unwrap();
        camera.cycle(flat);
        camera.yaw = 2000;
        camera.pitch = 128;
        // Look north (yaw 0) below the instant-snap speed of 100: the short
        // way from 2000 is +48, not -2000.
        camera.apply(&look_at(50, 60, 0, 5, 50), flat).unwrap();
        assert_eq!(camera.yaw, 2000, "a slow look does not snap");
        let mut seen = vec![];
        for _ in 0..9 {
            camera.cycle(flat);
            seen.push(camera.yaw);
        }
        // Each cycle adds delta * 50 / 1000 + 5; the ninth would reach 2052,
        // which wraps past the target, so it snaps to exactly 0.
        assert_eq!(seen, [2007, 2014, 2020, 2026, 2032, 2037, 2042, 2047, 0]);
        camera.cycle(flat);
        assert_eq!(camera.yaw, 0);
    }

    #[test]
    fn scripted_moves_never_snap_and_use_absolute_tiles() {
        use crate::ui::vm::CameraRequest;
        let mut camera = Script::default();
        camera.follow(Pose {
            position: [100, 100, -50],
            pitch: 200,
            yaw: 0,
        });
        let origin = [3200, 3400];
        // Speed 100 would snap a server move; a scripted one still eases.
        let request = CameraRequest::MoveTo {
            tile: [3250, 3460],
            height: 300,
            min_step: 0,
            speed: 100,
        };
        assert_eq!(camera.script(request, origin, flat), None);
        assert!(camera.locked);
        assert_eq!(camera.render, [100, 100, -50]);
        // The first cycle then applies the instant-speed rule of the update.
        camera.cycle(flat);
        assert_eq!(camera.render, [50 * 128 + 64, 60 * 128 + 64, -500]);
        let look = CameraRequest::LookAt {
            tile: [3250, 3470],
            height: -300,
            min_step: 0,
            speed: 100,
        };
        assert_eq!(camera.script(look, origin, flat), None);
        assert_eq!(camera.yaw, 0, "looking north");
        assert_eq!(
            camera.script(
                CameraRequest::Angle {
                    pitch: 500,
                    yaw: 100
                },
                origin,
                flat
            ),
            Some([100, 383])
        );
        assert_eq!(camera.script(CameraRequest::Unlock, origin, flat), None);
        assert!(!camera.locked);
    }

    #[test]
    fn reset_releases_the_lock_and_stops_shaking() {
        let mut camera = Script::default();
        camera.apply(&move_to(50, 50, 0, 0, 100), flat).unwrap();
        camera
            .apply(
                &ServerEvent::CameraShake {
                    counter: 1,
                    axis: 4,
                    jitter: 0,
                    amplitude: 100,
                    frequency: 0,
                    phase: 0,
                },
                flat,
            )
            .unwrap();
        assert_eq!(camera.pitch_floor(), Some(228));
        let applied = camera
            .apply(&ServerEvent::CameraReset { counter: 9 }, flat)
            .unwrap();
        assert_eq!(applied.counter, 9);
        assert!(!camera.locked && camera.pitch_floor().is_none());
        // Once released, the follow camera's pose is accepted again.
        let pose = Pose {
            position: [1, 2, 3],
            pitch: 300,
            yaw: 9,
        };
        camera.follow(pose);
        assert_eq!(camera.pose(), pose);
    }

    #[test]
    fn forced_angle_is_clamped_and_only_moves_a_locked_camera_directly() {
        let mut camera = Script::default();
        let event = ServerEvent::CameraAngle {
            counter: 3,
            yaw: 2047,
            pitch: 50,
        };
        let applied = camera.apply(&event, flat).unwrap();
        // Camera.mod uses 2047.0, so 2047 becomes 0; pitch floors at 128.
        assert_eq!(applied.angle, Some([0, 128]));
        assert_eq!(
            (camera.yaw, camera.pitch),
            (0, 0),
            "the follow camera owns its pose"
        );
        camera.apply(&move_to(1, 1, 0, 0, 100), flat).unwrap();
        camera
            .apply(
                &ServerEvent::CameraAngle {
                    counter: 4,
                    yaw: 700,
                    pitch: 900,
                },
                flat,
            )
            .unwrap();
        assert_eq!((camera.yaw, camera.pitch), (700, 383));
    }

    #[test]
    fn shake_channels_displace_their_own_axis() {
        let mut camera = Script::default();
        let pose = Pose {
            position: [1000, 2000, -300],
            pitch: 380,
            yaw: 2040,
        };
        for axis in 0..5 {
            camera
                .apply(
                    &ServerEvent::CameraShake {
                        counter: 0,
                        axis,
                        jitter: 4,
                        amplitude: 0,
                        frequency: 0,
                        phase: 0,
                    },
                    flat,
                )
                .unwrap();
        }
        // random = 1.0 - epsilon gives the maximum jitter of +4 on every channel.
        let shaken = camera.shaken(pose, || 0.999_999);
        assert_eq!(shaken.position, [1004, 2004, -296]);
        assert_eq!(shaken.yaw, (2040 + 4) & 0x7ff);
        assert_eq!(shaken.pitch, 383, "pitch stays within its clamp");
        // random = 0 gives -4.
        let shaken = camera.shaken(pose, || 0.0);
        assert_eq!(shaken.position, [996, 1996, -304]);
        assert_eq!((shaken.yaw, shaken.pitch), (2036, 376));
        // An out-of-range channel is ignored.
        assert!(
            camera
                .apply(
                    &ServerEvent::CameraShake {
                        counter: 5,
                        axis: 9,
                        jitter: 1,
                        amplitude: 1,
                        frequency: 1,
                        phase: 1
                    },
                    flat
                )
                .is_some()
        );
    }

    #[test]
    fn shake_phase_advances_every_cycle_and_drives_the_sine_term() {
        let mut camera = Script::default();
        camera
            .apply(
                &ServerEvent::CameraShake {
                    counter: 0,
                    axis: 1,
                    jitter: 0,
                    amplitude: 50,
                    frequency: 157,
                    phase: 0,
                },
                flat,
            )
            .unwrap();
        let pose = Pose {
            position: [0; 3],
            pitch: 200,
            yaw: 0,
        };
        // Phase 0: sin(0) * 50 plus a jitter of (0 * 2 + 1) * random - 0.
        assert_eq!(camera.shaken(pose, || 0.5).position[2], 0);
        camera.cycle(flat);
        // Phase 1: sin(1.57) * 50 = 49.99 plus 0.5, truncated to 50.
        assert_eq!(camera.shaken(pose, || 0.5).position[2], 50);
        camera.cycle(flat);
        camera.cycle(flat);
        // Phase 3: sin(4.71) * 50 = -49.99 plus 0.5, truncated toward zero.
        assert_eq!(camera.shaken(pose, || 0.5).position[2], -49);
        assert!(camera.apply(&ServerEvent::ResetAnimations, flat).is_none());
    }
}
