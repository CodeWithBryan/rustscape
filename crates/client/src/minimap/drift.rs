//! The slow random drift of the minimap and of the follow camera.
//!
//! `client.resetGameSessionState` picks the starting values and the tail of
//! `Protocol`'s per-cycle update (`MiniMap.jitterTimer++` down to the last
//! bound check) moves them. Both read `Math.random()`; here the random source
//! is the caller's, one call per original draw, in the original order.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Drift {
    /// `Camera.cameraOffsetX`, `Camera.cameraOffsetY`: added to the followed position.
    pub camera_offset: [i32; 2],
    /// `Camera.yawDrift`: added to the rendered camera yaw.
    pub yaw_drift: i32,
    /// `MiniMap.compassAngleOffset`.
    pub angle_offset: i32,
    /// `MiniMap.zoomOffset`.
    pub zoom_offset: i32,
    /// `Camera.offsetXDelta`, `Protocol.cameraYOffsetDelta`.
    pub camera_step: [i32; 2],
    /// `Protocol.yawDriftDelta`.
    pub yaw_step: i32,
    /// `MiniMap.angleJitterStep`.
    pub angle_step: i32,
    /// `MiniMap.zoomJitterStep`.
    pub zoom_step: i32,
    /// `Protocol.cameraJitterTimer`.
    pub camera_timer: i32,
    /// `MiniMap.jitterTimer`.
    pub minimap_timer: i32,
}
/// What a frame owes the minimap raster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Redraw {
    /// `Player.plane == LightingManager.minimapRenderedPlane`: keep the sprite.
    Keep,
    /// `SceneGraph.checkPlaneChange` on a new plane: `MiniMap.renderMap` only.
    /// (With `allLevelsAreVisible()` false the original reloads the region
    /// instead; this client always holds all four levels.)
    Plane,
    /// After a scene rebuild, which reset `minimapRenderedPlane`: `buildScene`
    /// has stepped the floor jitter and `renderMap` runs for the new scene.
    Scene,
}

/// `drawn` is the scene (a rebuild counter and its origin) and plane the
/// raster was last built for. Locations the server adds or removes inside a
/// scene are not an input: the original leaves them out of the sprite until
/// the plane changes or the scene is rebuilt.
pub fn redraw(
    drawn: Option<((u64, [i32; 2]), usize)>,
    scene: (u64, [i32; 2]),
    plane: usize,
) -> Redraw {
    match drawn {
        Some((old, level)) if old == scene && level == plane => Redraw::Keep,
        Some((old, _)) if old == scene => Redraw::Plane,
        _ => Redraw::Scene,
    }
}

impl Default for Drift {
    /// The field initialisers of `Camera`, `Protocol` and `MiniMap`.
    fn default() -> Self {
        Self {
            camera_offset: [0; 2],
            yaw_drift: 0,
            angle_offset: 0,
            zoom_offset: 0,
            camera_step: [2, 2],
            yaw_step: 1,
            angle_step: 2,
            zoom_step: 1,
            camera_timer: 0,
            minimap_timer: 0,
        }
    }
}
impl Drift {
    /// The six `Math.random()` assignments of `client.resetGameSessionState`,
    /// in source order. Steps and timers are not reset there. Returns the new
    /// `Camera.yawTarget`, which belongs to the camera.
    pub fn reset(&mut self, mut random: impl FnMut() -> f64) -> i32 {
        self.camera_offset[0] = (random() * 100.0) as i32 - 50;
        let yaw_target = ((random() * 20.0) as i32 - 10) & 0x7ff;
        self.camera_offset[1] = (random() * 110.0) as i32 - 55;
        self.zoom_offset = (random() * 30.0) as i32 - 20;
        self.angle_offset = (random() * 120.0) as i32 - 60;
        self.yaw_drift = (random() * 80.0) as i32 - 40;
        yaw_target
    }

    /// One 20 ms client cycle. A timer that has passed 500 draws once and
    /// applies its steps by the bits of `(int) (random * 8)`; a value outside
    /// its range then turns its step around.
    pub fn cycle(&mut self, mut random: impl FnMut() -> f64) {
        self.minimap_timer = self.minimap_timer.wrapping_add(1);
        self.camera_timer = self.camera_timer.wrapping_add(1);
        if self.camera_timer > 500 {
            self.camera_timer = 0;
            let bits = (random() * 8.0) as i32;
            if bits & 4 == 4 {
                self.yaw_drift = self.yaw_drift.wrapping_add(self.yaw_step);
            }
            if bits & 2 == 2 {
                self.camera_offset[1] = self.camera_offset[1].wrapping_add(self.camera_step[1]);
            }
            if bits & 1 == 1 {
                self.camera_offset[0] = self.camera_offset[0].wrapping_add(self.camera_step[0]);
            }
        }
        if self.minimap_timer > 500 {
            self.minimap_timer = 0;
            let bits = (random() * 8.0) as i32;
            if bits & 1 == 1 {
                self.angle_offset = self.angle_offset.wrapping_add(self.angle_step);
            }
            if bits & 2 == 2 {
                self.zoom_offset = self.zoom_offset.wrapping_add(self.zoom_step);
            }
        }
        if self.camera_offset[0] < -50 {
            self.camera_step[0] = 2;
        }
        if self.angle_offset < -60 {
            self.angle_step = 2;
        }
        if self.zoom_offset < -20 {
            self.zoom_step = 1;
        }
        if self.camera_offset[1] < -55 {
            self.camera_step[1] = 2;
        }
        if self.camera_offset[1] > 55 {
            self.camera_step[1] = -2;
        }
        if self.yaw_drift < -40 {
            self.yaw_step = 1;
        }
        if self.camera_offset[0] > 50 {
            self.camera_step[0] = -2;
        }
        if self.yaw_drift > 40 {
            self.yaw_step = -1;
        }
        if self.zoom_offset > 10 {
            self.zoom_step = -1;
        }
        if self.angle_offset > 60 {
            self.angle_step = -2;
        }
    }

    /// `SceneGraph`'s static `lightnessJitter` then `hueJitter` initialisers,
    /// returned as `[hueJitter, lightnessJitter]`.
    pub fn initial_floor(mut random: impl FnMut() -> f64) -> [i32; 2] {
        let lightness = (random() * 17.0) as i32 - 8;
        let hue = (random() * 33.0) as i32 - 16;
        [hue, lightness]
    }

    /// `SceneGraph.buildScene`'s per-rebuild step of `[hueJitter, lightnessJitter]`.
    pub fn step_floor(floor: &mut [i32; 2], mut random: impl FnMut() -> f64) {
        floor[0] = floor[0]
            .wrapping_add((random() * 5.0) as i32 - 2)
            .clamp(-16, 16);
        floor[1] = floor[1]
            .wrapping_add((random() * 5.0) as i32 - 2)
            .clamp(-8, 8);
    }
}
