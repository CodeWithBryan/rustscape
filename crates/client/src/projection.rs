//! Where the scene is drawn and how it is projected, in the original's
//! integers: `ScriptRunner.calculateViewportBounds`, the frustum edges of
//! `GlRenderer.setupPerspectiveView` and `ScriptRunner.projectToScreen`, all
//! on their HD (`GlRenderer.enabled`) paths.
use crate::{animation::sin_cos, camera::Pose, follow_camera::Scene, ui::vm::Viewport};

/// `GlRenderer.setupPerspectiveView`'s near plane, in fine units.
pub const NEAR: i32 = 50;
/// `GlobalConfig.VIEW_DISTANCE`: `TILE_DISTANCE * 128`, the far plane.
pub const FAR: i32 = 56 * 128;

/// Java's `int / int` for a divisor that may be zero or -1. The original
/// throws on zero; this gives 0.
fn div(a: i32, b: i32) -> i32 {
    if b == 0 { 0 } else { a.wrapping_div(b) }
}

/// `ScriptRunner.viewportLeft`, `viewportTop`, `viewportWidth`,
/// `viewportHeight` and `viewportZoom` after `calculateViewportBounds`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub zoom: i32,
    /// The rectangles `[x, y, width, height]` the original fills black beside
    /// a view narrowed or shortened by the zoom limits.
    pub bars: Vec<[i32; 4]>,
}

/// `ScriptRunner.calculateViewportBounds(width, top, height, left, true)` for
/// a viewport component's rectangle. The zoom falls from `near_zoom` at 334
/// pixels high to `far_zoom` at 434; the limits scripts set (CS2 6202) can
/// then narrow or shorten the view.
pub fn bounds(
    mut left: i32,
    mut top: i32,
    mut width: i32,
    mut height: i32,
    v: &Viewport,
) -> Bounds {
    if width < 1 {
        width = 1;
    }
    if height < 1 {
        height = 1;
    }
    let mut bars = Vec::new();
    let delta = height.wrapping_sub(334).clamp(0, 100);
    let mut zoom = delta.wrapping_mul(v.far_zoom.wrapping_sub(v.near_zoom)) / 100 + v.near_zoom;
    if v.min_zoom > zoom {
        zoom = v.min_zoom;
    } else if v.max_zoom < zoom {
        zoom = v.max_zoom;
    }
    let fov = div(
        zoom.wrapping_mul(height).wrapping_mul(512),
        width.wrapping_mul(334),
    );
    if fov < v.min_fov {
        let limit = v.min_fov;
        zoom = div(
            width.wrapping_mul(334).wrapping_mul(limit),
            height.wrapping_mul(512),
        );
        if v.max_zoom < zoom {
            zoom = v.max_zoom;
            let adjusted = div(
                height.wrapping_mul(512).wrapping_mul(zoom),
                limit.wrapping_mul(334),
            );
            let padding = width.wrapping_sub(adjusted) / 2;
            bars.push([left, top, padding, height]);
            bars.push([
                width.wrapping_add(left).wrapping_sub(padding),
                top,
                padding,
                height,
            ]);
            left = left.wrapping_add(padding);
            width = width.wrapping_sub(padding.wrapping_mul(2));
        }
    } else if v.max_fov < fov {
        let limit = v.max_fov;
        zoom = div(
            limit.wrapping_mul(width).wrapping_mul(334),
            height.wrapping_mul(512),
        );
        if v.min_zoom > zoom {
            zoom = v.min_zoom;
            let adjusted = div(
                limit.wrapping_mul(width).wrapping_mul(334),
                zoom.wrapping_mul(512),
            );
            let padding = height.wrapping_sub(adjusted) / 2;
            bars.push([left, top, width, padding]);
            bars.push([
                left,
                top.wrapping_add(height).wrapping_sub(padding),
                width,
                padding,
            ]);
            height = height.wrapping_sub(padding.wrapping_mul(2));
            top = top.wrapping_add(padding);
        }
    }
    Bounds {
        left,
        top,
        // The original stores both in `short` fields and reads them back.
        width: i32::from(width as i16),
        height: i32::from(height as i16),
        zoom: zoom.wrapping_mul(height) / 334,
        bars,
    }
}

/// `Rasteriser.screenLowerX`, `screenUpperX`, `screenLowerY` and
/// `screenUpperY` as `setupPerspectiveView` leaves them: the view's edges on
/// the plane 256 units ahead. The GL frustum is these times 50 / 512 at the
/// near plane of 50, with Y negated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edges {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl Bounds {
    /// None when the zoom is 0, where the original divides by zero.
    pub fn edges(&self) -> Option<Edges> {
        if self.zoom == 0 {
            return None;
        }
        let (x, y) = (self.left, self.top);
        let center_x = (self.width / 2).wrapping_add(x);
        let center_y = y.wrapping_add(self.height / 2);
        let edge = |at: i32, center: i32| (at.wrapping_sub(center) << 8).wrapping_div(self.zoom);
        Some(Edges {
            left: edge(x, center_x),
            right: edge(x.wrapping_add(self.width), center_x),
            top: edge(y, center_y),
            bottom: edge(y.wrapping_add(self.height), center_y),
        })
    }

    /// `ScriptRunner.projectToScreen` as `drawOverheads` calls it: where the
    /// point `height` above the ground at scene-local `fine` appears, relative
    /// to the viewport's top-left corner, or `[-1, -1]` when it is outside the
    /// scene or nearer than 50 units to the camera plane.
    pub fn project(
        &self,
        pose: &Pose,
        fine: [i32; 2],
        height: i32,
        plane: i32,
        scene: &impl Scene,
    ) -> [i32; 2] {
        project(
            pose,
            fine,
            height,
            [self.width >> 1, self.height >> 1],
            [self.zoom; 2],
            plane,
            scene,
        )
    }
}

/// `ScriptRunner.projectToScreen(halfWidth, zoomH, yFine, projHeight,
/// halfHeight, xFine, zoomV)` with `Camera.renderX/Y/Z`, `cameraPitch` and
/// `cameraYaw` from `pose`.
pub fn project(
    pose: &Pose,
    fine: [i32; 2],
    height: i32,
    half: [i32; 2],
    zoom: [i32; 2],
    plane: i32,
    scene: &impl Scene,
) -> [i32; 2] {
    let [x, y] = fine;
    if x < 128 || y < 128 || x > 13056 || y > 13056 {
        return [-1, -1];
    }
    let above = scene.ground(plane, fine).wrapping_sub(height);
    let dy = y.wrapping_sub(pose.position[1]);
    let dz = above.wrapping_sub(pose.position[2]);
    let dx = x.wrapping_sub(pose.position[0]);
    let (sin_pitch, cos_pitch) = sin_cos(pose.pitch);
    let (sin_yaw, cos_yaw) = sin_cos(pose.yaw);
    let rotated_x = dx
        .wrapping_mul(cos_yaw)
        .wrapping_add(sin_yaw.wrapping_mul(dy))
        >> 16;
    let rotated_z = dy
        .wrapping_mul(cos_yaw)
        .wrapping_sub(sin_yaw.wrapping_mul(dx))
        >> 16;
    let screen_dy = cos_pitch
        .wrapping_mul(dz)
        .wrapping_sub(rotated_z.wrapping_mul(sin_pitch))
        >> 16;
    let depth = rotated_z
        .wrapping_mul(cos_pitch)
        .wrapping_add(dz.wrapping_mul(sin_pitch))
        >> 16;
    if depth < 50 {
        return [-1, -1];
    }
    let scale = |zoom: i32| zoom.wrapping_mul(512) >> 8;
    [
        scale(zoom[0])
            .wrapping_mul(rotated_x)
            .wrapping_div(depth)
            .wrapping_add(half[0]),
        scale(zoom[1])
            .wrapping_mul(screen_dy)
            .wrapping_div(depth)
            .wrapping_add(half[1]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests, worked by hand from the Java. The JVM differential
    // is tests/camera_reference.rs.

    #[test]
    fn fixed_and_resizable_viewports_get_the_original_zoom_and_edges() {
        let limits = Viewport::default();
        // The fixed game frame's 512 x 334 view at (4, 4): zoom 256.
        let fixed = bounds(4, 4, 512, 334, &limits);
        assert_eq!(
            (fixed.left, fixed.top, fixed.width, fixed.height, fixed.zoom),
            (4, 4, 512, 334, 256)
        );
        assert!(fixed.bars.is_empty());
        // (-256 << 8) / 256 and (-167 << 8) / 256: 334 / 1024 is the tangent
        // of half the vertical field of view, 36.1 degrees in all.
        assert_eq!(
            fixed.edges(),
            Some(Edges {
                left: -256,
                right: 256,
                top: -167,
                bottom: 167
            })
        );
        // 100 pixels taller or more: zoom 205, times height / 334.
        let tall = bounds(0, 0, 1280, 800, &limits);
        assert_eq!(tall.zoom, 205 * 800 / 334);
        // (-640 << 8) / 491 = -333 and (-400 << 8) / 491 = -208: 44.2 degrees.
        assert_eq!(
            tall.edges(),
            Some(Edges {
                left: -333,
                right: 333,
                top: -208,
                bottom: 208
            })
        );
        // Half way: 50 * (205 - 256) / 100 truncates toward zero to -25.
        assert_eq!(bounds(0, 0, 765, 384, &limits).zoom, 231 * 384 / 334);
        // An odd width leaves the centre half a pixel left of the middle.
        let odd = bounds(10, 20, 765, 503, &limits);
        assert_eq!(odd.zoom, 205 * 503 / 334);
        assert_eq!(
            odd.edges(),
            Some(Edges {
                left: (-382 << 8) / 308,
                right: (383 << 8) / 308,
                top: (-251 << 8) / 308,
                bottom: (252 << 8) / 308
            })
        );
        assert_eq!(bounds(0, 0, 0, -5, &limits).width, 1);
    }

    #[test]
    fn zoom_limits_narrow_or_shorten_the_view_with_black_bars() {
        // A wide view whose field of view may not fall under 400: the zoom
        // wanted, 1200 * 334 * 400 / (400 * 512) = 782, is over the maximum
        // of 300, so the view is narrowed to 400 * 512 * 300 / (400 * 334).
        let wide = Viewport {
            min_fov: 400,
            max_zoom: 300,
            ..Viewport::default()
        };
        let view = bounds(10, 20, 1200, 400, &wide);
        let adjusted = 400 * 512 * 300 / (400 * 334);
        let padding = (1200 - adjusted) / 2;
        assert_eq!((adjusted, padding), (459, 370));
        assert_eq!(
            (view.left, view.top, view.width, view.height),
            (380, 20, 460, 400)
        );
        assert_eq!(
            view.bars,
            [[10, 20, 370, 400], [1200 + 10 - 370, 20, 370, 400]]
        );
        assert_eq!(view.zoom, 300 * 400 / 334);
        // A tall view capped at a field of view of 100 with a minimum zoom.
        let tall = Viewport {
            max_fov: 100,
            min_zoom: 220,
            ..Viewport::default()
        };
        let view = bounds(0, 0, 400, 1000, &tall);
        // fov = 220 * 1000 * 512 / (400 * 334) = 843 > 100; the zoom wanted,
        // 100 * 400 * 334 / (1000 * 512) = 26, is under the minimum of 220.
        let adjusted = 100 * 400 * 334 / (220 * 512);
        let padding = (1000 - adjusted) / 2;
        assert_eq!((adjusted, padding), (118, 441));
        assert_eq!(
            (view.left, view.top, view.width, view.height),
            (0, 441, 400, 118)
        );
        assert_eq!(view.bars, [[0, 0, 400, 441], [0, 1000 - 441, 400, 441]]);
        assert_eq!(view.zoom, 220 * 118 / 334);
    }
}
