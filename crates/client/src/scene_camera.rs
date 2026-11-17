use macroquad::{camera::Camera, prelude::*};
use rs09_client::{
    camera::{Pose, Script},
    follow_camera::{Follow, Occlusion, RegionScene},
    projection::{self, Bounds},
    ui::{
        state::{DrawNode, State},
        vm::Viewport,
    },
    world::Ray,
};

/// The scene's rectangle this frame: the interface component with client code
/// 1337 (`Cs1ScriptRunner.renderComponent` hands `renderGameScene` its whole
/// rectangle, not the clipped part), or the whole window when no interface
/// with such a component is shown, as in the workbench with its interface
/// hidden. `calculateViewportBounds` then gives the zoom.
pub(crate) fn view(interface: Option<(&State, &[DrawNode])>, limits: &Viewport) -> Bounds {
    let rect = interface
        .and_then(|(state, nodes)| rs09_client::ui::viewport::find(state, nodes))
        .map_or(
            [
                0,
                0,
                crate::display_host::screen_width() as i32,
                crate::display_host::screen_height() as i32,
            ],
            |(rect, _)| [rect.x, rect.y, rect.w, rect.h],
        );
    projection::bounds(rect[0], rect[1], rect[2], rect[3], limits)
}

/// The occlusion heights of the workbench's current scene, rebuilt when the
/// scene base or the loaded regions change.
#[derive(Default)]
pub(crate) struct SceneOcclusion {
    key: Option<([i32; 2], Vec<[i32; 2]>)>,
    heights: Occlusion,
}

impl SceneOcclusion {
    pub(crate) fn refresh(&mut self, world: &super::Workbench) {
        let origin = world.navigation.origin;
        let loaded = world.regions.iter().map(|r| [r.x, r.z]).collect();
        let key = Some((origin, loaded));
        if self.key != key {
            self.heights = Occlusion::scene(origin, &world.regions);
            self.key = key;
        }
    }

    /// `SceneGraph` as the camera reads it.
    pub(crate) fn scene<'a>(&'a self, world: &'a super::Workbench) -> RegionScene<'a> {
        RegionScene {
            regions: &world.regions,
            origin: world.navigation.origin,
            occlusion: &self.heights,
        }
    }
}

/// The local player's fine position within the scene (`PlayerList.self`).
pub(crate) fn local(fine: [i32; 2], origin: [i32; 2]) -> [i32; 2] {
    [0, 1].map(|axis| fine[axis].wrapping_sub(origin[axis].wrapping_mul(128)))
}

/// What `ScriptRunner.renderGameScene` aims the camera with this frame.
pub(crate) struct Viewer<'a> {
    pub(crate) follow: &'a Follow,
    pub(crate) view: &'a Bounds,
    /// `ScriptRunner.minViewDistance` and `maxViewDistance`.
    pub(crate) view_distance: [i32; 2],
    pub(crate) world: &'a super::Workbench,
    pub(crate) occlusion: &'a SceneOcclusion,
    /// The local player's absolute fine position.
    pub(crate) player: [i32; 2],
}

impl Viewer<'_> {
    /// The camera and its pose. The follow camera is in charge unless the
    /// server or a script has locked the view (RT4 camera types 2 and 3); it
    /// reports its pose so a later lock eases from the current view. Shake
    /// channels then displace the pose for drawing only; picking passes
    /// `shake: false`.
    pub(crate) fn camera(&self, script: &mut Script, shake: bool) -> (SceneCamera, Pose) {
        let origin = self.world.navigation.origin;
        let mut pose = if script.locked || script.spline.is_some() {
            script.pose()
        } else {
            let pose = self.follow.pose(
                self.view.height,
                self.view_distance,
                local(self.player, origin),
                self.world.ground.plane,
                script.pitch_floor(),
                &self.occlusion.scene(self.world),
            );
            script.follow(pose);
            pose
        };
        if shake {
            pose = script.shaken(pose, || f64::from(macroquad::rand::gen_range(0.0f32, 1.0)));
        }
        // ScriptRunner.renderGameScene (278-281): a spline camera (type 3)
        // is aimed by `Camera.pitchRadians` and `yawRadians`, not by the
        // 2048-step angles derived from them.
        let camera = match &script.spline {
            Some(spline) => SceneCamera::aimed(
                pose,
                [spline.pitch_radians(), spline.yaw_radians()],
                origin,
                self.view,
            ),
            None => SceneCamera::posed(pose, origin, self.view),
        };
        (camera, pose)
    }
}

pub(crate) struct SceneCamera {
    pub(crate) position: Vec3,
    pub(crate) target: Vec3,
    projection: Mat4,
    /// The view's rectangle in window pixels: left, top, width, height.
    rect: [f32; 4],
}

impl SceneCamera {
    /// A camera at an RT4 pose: scene-local fine position with height negative
    /// upward, and pitch/yaw in 2048 units. Yaw 0 faces north and turns toward
    /// west; pitch tilts the view downward. The projection is
    /// `GlRenderer.setupPerspectiveView` for `view`: a frustum through the
    /// viewport's edges about its centre, near plane 50 and far plane 7168
    /// units, here in tiles.
    ///
    /// Game X increases east and Z increases north, with height pointing up
    /// here; RT4's eye space after its 180 degree turn about X has X right, Y
    /// up and the view along -Z. That mapping mirrors, as the original's does,
    /// and both world pipelines use counterclockwise faces with it.
    pub(crate) fn posed(pose: Pose, origin: [i32; 2], view: &Bounds) -> Self {
        // The original turns the model view by `angle * 0.17578125` degrees.
        let unit = std::f32::consts::TAU / 2048.0;
        Self::aimed(
            pose,
            [pose.pitch as f32 * unit, pose.yaw as f32 * unit],
            origin,
            view,
        )
    }

    /// [`Self::posed`] with the pitch and yaw given in radians instead of
    /// taken from the pose.
    pub(crate) fn aimed(
        pose: Pose,
        [pitch, yaw]: [f32; 2],
        origin: [i32; 2],
        view: &Bounds,
    ) -> Self {
        let [x, y, z] = pose.position.map(|n| n as f32 / 128.0);
        let position = vec3(
            x + origin[0] as f32 - 3200.0,
            -z,
            y + origin[1] as f32 - 3200.0,
        );
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let forward = vec3(-sin_yaw * cos_pitch, -sin_pitch, cos_yaw * cos_pitch);
        let rotation = Mat4::from_cols(
            vec4(cos_yaw, -sin_pitch * sin_yaw, cos_pitch * sin_yaw, 0.0),
            vec4(0.0, cos_pitch, sin_pitch, 0.0),
            vec4(sin_yaw, sin_pitch * cos_yaw, -cos_pitch * cos_yaw, 0.0),
            Vec4::W,
        );
        // A zoom of 0 has no frustum (the original divides by zero): look
        // through the default fixed view instead.
        let edges = view.edges().unwrap_or(projection::Edges {
            left: -256,
            right: 256,
            top: -167,
            bottom: 167,
        });
        // setPerspectiveFrustum: edges times NEAR_PLANE_SCALE (50 / 512) on
        // the near plane, Y negated; everything divided by 128 for tiles.
        let scale = 0.097_656_25 / 128.0;
        let (x_min, x_max) = (edges.left as f32 * scale, edges.right as f32 * scale);
        let (y_min, y_max) = (-edges.bottom as f32 * scale, -edges.top as f32 * scale);
        let (near, far) = (
            projection::NEAR as f32 / 128.0,
            projection::FAR as f32 / 128.0,
        );
        let frustum = Mat4::from_cols_array(&[
            near * 2.0 / (x_max - x_min),
            0.0,
            0.0,
            0.0,
            0.0,
            near * 2.0 / (y_max - y_min),
            0.0,
            0.0,
            (x_max + x_min) / (x_max - x_min),
            (y_max + y_min) / (y_max - y_min),
            -(far + near) / (far - near),
            -1.0,
            0.0,
            0.0,
            -(near * 2.0 * far) / (far - near),
            0.0,
        ]);
        Self {
            position,
            target: position + forward,
            projection: frustum * rotation * Mat4::from_translation(-position),
            rect: [view.left, view.top, view.width, view.height].map(|n| n as f32),
        }
    }

    /// The ray through a window pixel, which need not lie inside the view.
    pub(crate) fn ray(&self, pixel: Vec2) -> Ray {
        let inverse = self.projection.inverse();
        let [left, top, width, height] = self.rect;
        let ndc = vec2(
            2.0 * (pixel.x - left) / width - 1.0,
            1.0 - 2.0 * (pixel.y - top) / height,
        );
        let near = inverse.project_point3(ndc.extend(-1.0));
        let far = inverse.project_point3(ndc.extend(1.0));
        Ray {
            origin: near.to_array(),
            direction: (far - near).normalize().to_array(),
        }
    }

    /// The window pixel a world point is drawn at, if it is in the view.
    pub(crate) fn pixel(&self, point: Vec3) -> Option<Vec2> {
        let projected = self.projection.project_point3(point);
        let [left, top, width, height] = self.rect;
        (projected.is_finite() && projected.abs().max_element() < 1.0).then(|| {
            vec2(
                left + (projected.x + 1.0) * width * 0.5,
                top + (1.0 - projected.y) * height * 0.5,
            )
        })
    }

    /// The view's rectangle in physical pixels from the window's top-left
    /// corner, for a scissor. The view is given on the canvas, which the
    /// fixed window modes place inside a larger window.
    pub(crate) fn scissor(&self) -> (i32, i32, i32, i32) {
        if let Some(scaled) = crate::display_host::scaled_view_px(self.rect) {
            return scaled;
        }
        let scale = screen_dpi_scale();
        let [left, top, width, height] = self.rect.map(|n| (n * scale) as i32);
        let (canvas_left, canvas_top) = crate::display_host::corner_px();
        (left + canvas_left, top + canvas_top, width, height)
    }

    /// Confine 2D drawing to the view (`GlRaster.setClip` in
    /// `renderGameScene`), or stop confining it.
    pub(crate) fn clip(&self, on: bool) {
        // SAFETY: the macroquad context exists whenever a frame is drawn.
        unsafe {
            get_internal_gl()
                .quad_gl
                .scissor(on.then(|| self.scissor()));
        }
    }
}

impl Camera for SceneCamera {
    fn matrix(&self) -> Mat4 {
        self.projection
    }

    fn depth_enabled(&self) -> bool {
        true
    }

    fn render_pass(&self) -> Option<RenderPass> {
        None
    }

    /// `GlRenderer.setViewportBounds(x, canvasHeight - y - height, width,
    /// height)`, in physical pixels from the bottom-left corner.
    fn viewport(&self) -> Option<(i32, i32, i32, i32)> {
        let (left, top, width, height) = self.scissor();
        let window = (screen_height() * screen_dpi_scale()) as i32;
        Some((left, window - top - height, width, height))
    }
}

pub(crate) fn minimap_yaw(orbit_yaw: f32) -> i32 {
    // The workbench's `--yaw` measures an orbit from the camera's position
    // southward from north; RT4 measures the direction it faces, turning
    // toward west.
    ((std::f32::consts::PI - orbit_yaw) * 2048.0 / std::f32::consts::TAU).round() as i32 & 2047
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_client::{
        follow_camera::{Scene, render_position},
        minimap::View,
    };

    struct Flat;
    impl Scene for Flat {
        fn ground(&self, _: i32, _: [i32; 2]) -> i32 {
            -64
        }
        fn flags(&self, _: i32, _: i32, _: i32) -> u8 {
            0
        }
        fn height(&self, _: i32, _: i32, _: i32) -> i32 {
            -64
        }
        fn occlusion(&self, _: i32, _: i32, _: i32) -> u8 {
            0
        }
    }

    fn limits() -> Viewport {
        Viewport::default()
    }

    #[test]
    fn the_matrix_draws_points_where_project_to_screen_puts_them() {
        // `Bounds::project` is ScriptRunner.projectToScreen, which the JVM
        // differential in tests/camera_reference.rs covers; the GL matrix must
        // agree with it to the pixel its integers truncate to, for views with
        // an odd width (whose centre is off the middle) and an offset.
        let origin = [3200 - 48, 3200 - 48];
        for (rect, pitch, yaw) in [
            ([4, 4, 512, 334], 128, 0),
            ([0, 0, 765, 503], 228, 144),
            ([13, 57, 1037, 611], 383, 1900),
            ([0, 0, 1280, 800], 300, 1024),
        ] {
            let view = projection::bounds(rect[0], rect[1], rect[2], rect[3], &limits());
            let target = [48 * 128 + 64, 50 * 128 + 64];
            let pose = render_position(
                target,
                view.height,
                -64 - 50,
                600 + pitch * 3,
                yaw,
                pitch,
                [256, 320],
            );
            let camera = SceneCamera::posed(pose, origin, &view);
            let mut seen = 0;
            for dx in (-1500..=1500).step_by(300) {
                for dy in (-1500..=1500).step_by(300) {
                    for above in [0, 200] {
                        let fine = [target[0] + dx, target[1] + dy];
                        let [x, y] = view.project(&pose, fine, above, 0, &Flat);
                        // Points in the view: off it, and so close to the
                        // camera plane, the truncated depth moves the
                        // integer result by many pixels.
                        if !(0..view.width).contains(&x) || !(0..view.height).contains(&y) {
                            continue;
                        }
                        let world = vec3(
                            fine[0] as f32 / 128.0 + origin[0] as f32 - 3200.0,
                            (64 + above) as f32 / 128.0,
                            fine[1] as f32 / 128.0 + origin[1] as f32 - 3200.0,
                        );
                        let clip = camera.matrix().project_point3(world);
                        let pixel = vec2(
                            (clip.x + 1.0) * view.width as f32 * 0.5,
                            (1.0 - clip.y) * view.height as f32 * 0.5,
                        );
                        // The original's two paths do not agree exactly
                        // either: projectToScreen truncates its rotation and
                        // quotient, and the frustum's edges are truncated
                        // (-208 for -208.6 at 765 x 503), which scales the
                        // drawn scene by up to a quarter of a percent.
                        assert!(
                            (pixel - vec2(x as f32, y as f32)).abs().max_element() < 2.5,
                            "{rect:?} {fine:?}+{above}: {pixel} != {x},{y}"
                        );
                        seen += 1;
                    }
                }
            }
            assert!(seen > 40, "{rect:?}: {seen} points in front of the camera");
            // The player, 50 units up as the camera aims, is drawn at the
            // view's centre: the middle pixel, left of it for an odd width.
            let centre = view.project(&pose, target, 50, 0, &Flat);
            assert!(
                (centre[0] - view.width / 2).abs() <= 1,
                "{rect:?} {centre:?}"
            );
            assert!(
                (centre[1] - view.height / 2).abs() <= 1,
                "{rect:?} {centre:?}"
            );
        }
    }

    #[test]
    fn field_of_view_is_the_originals_not_45_radians() {
        // tan(half the vertical field) is 334 / (4 * zoom): 36.1 degrees for
        // the fixed 334-pixel view, 44.3 from 434 pixels up.
        for (height, degrees) in [(334, 36.1f32), (503, 44.3), (800, 44.3)] {
            let view = projection::bounds(0, 0, 765, height, &limits());
            let pose = Pose {
                position: [6400, 6400, -500],
                pitch: 0,
                yaw: 0,
            };
            let camera = SceneCamera::posed(pose, [3200, 3200], &view);
            let top = camera.ray(vec2(382.0, 0.0)).direction;
            let bottom = camera.ray(vec2(382.0, height as f32)).direction;
            let angle = Vec3::from_array(top)
                .angle_between(Vec3::from_array(bottom))
                .to_degrees();
            assert!((angle - degrees).abs() < 0.3, "{height}: {angle}");
        }
    }

    #[test]
    fn poses_face_north_at_yaw_zero_and_look_down_with_pitch() {
        let view = projection::bounds(0, 0, 512, 334, &limits());
        let facing = |pitch, yaw| {
            let camera = SceneCamera::posed(
                Pose {
                    position: [0; 3],
                    pitch,
                    yaw,
                },
                [3200, 3200],
                &view,
            );
            camera.target - camera.position
        };
        // Yaw 0 faces north (+z here); 512 faces west; a positive pitch looks down.
        assert!((facing(0, 0) - vec3(0.0, 0.0, 1.0)).length() < 1e-5);
        assert!((facing(0, 512) - vec3(-1.0, 0.0, 0.0)).length() < 1e-5);
        assert!(facing(256, 0).y < -0.7);
        // The position is the pose's, in tiles with height up.
        let camera = SceneCamera::posed(
            Pose {
                position: [640, 1280, -256],
                pitch: 200,
                yaw: 300,
            },
            [3210, 3190],
            &view,
        );
        assert!((camera.position - vec3(15.0, 2.0, 0.0)).length() < 1e-5);
    }

    fn orbiting(yaw: i32, view: &Bounds) -> SceneCamera {
        // The follow camera around the world origin of these tests.
        let pose = render_position([6400, 6400], view.height, -50, 1500, yaw, 256, [256, 320]);
        SceneCamera::posed(pose, [3150, 3150], view)
    }

    #[test]
    fn world_and_minimap_agree_on_cardinal_and_diagonal_directions() {
        let size = [256, 256];
        let masks = [[0, 256]; 256];
        let view = projection::bounds(0, 0, 1280, 800, &limits());
        // Each case points at the target from a different compass heading.
        for step in 0..8 {
            let yaw = std::f32::consts::PI - step as f32 * std::f32::consts::PI / 4.0;
            let map = View {
                yaw: minimap_yaw(yaw),
                angle_offset: 0,
                zoom_offset: 0,
            };
            assert_eq!(map.yaw, step * 256);
            let camera = orbiting(map.yaw, &view);
            for [east, north] in [[2, 1], [-2, 1], [1, -2], [-1, -2]] {
                let delta = [east * 4, north * 4];
                let icon = map.icon(size, [0; 2], delta).unwrap();
                let point = camera
                    .matrix()
                    .project_point3(vec3(east as f32, 0.0, north as f32));
                assert_eq!(point.x.is_sign_positive(), icon[0] > size[0] / 2);
                assert_eq!(point.y.is_sign_positive(), icon[1] < size[1] / 2);
                let tile = map.click(size, &masks, icon, [64, 64]).unwrap();
                assert_eq!(tile, [east, north]);
            }
        }
    }

    #[test]
    fn north_up_east_and_west_picks_hit_their_visible_geometry() {
        // A view that does not start at the window's corner: clicks are
        // measured from the view's own rectangle.
        let view = projection::bounds(40, 30, 1037, 611, &limits());
        let camera = orbiting(0, &view);
        for east in [-2.0, 2.0] {
            let tile = vec3(east, 0.0, 1.0);
            let projected = camera.matrix().project_point3(tile);
            assert_eq!(projected.x.is_sign_positive(), east > 0.0);
            let pixel = camera.pixel(tile).expect("the tile is in view");
            assert!(pixel.x > 40.0 && pixel.y > 30.0);
            let ray = camera.ray(pixel);
            let distance = ray
                .triangle(
                    (tile + vec3(-0.4, 0.0, -0.4)).to_array(),
                    (tile + vec3(0.4, 0.0, -0.4)).to_array(),
                    (tile + vec3(0.0, 0.0, 0.4)).to_array(),
                )
                .expect("click hits the visible tile");
            let hit = Vec3::from_array(ray.origin) + Vec3::from_array(ray.direction) * distance;
            assert!(hit.abs_diff_eq(tile, 0.001), "{hit:?} != {tile:?}");
        }
    }
}
