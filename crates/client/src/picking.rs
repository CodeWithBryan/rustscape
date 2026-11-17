//! RT4's mouse picking rules, without a GPU.
//!
//! The original decides what is under the mouse while it draws. Every model
//! runs `GlModel.render`'s hover test in the integer camera space of the
//! frame: a cylinder cull, a screen rectangle around the cylinder, the screen
//! rectangle of the eight projected bounding-box corners, and then either
//! nothing more (models flagged `pickable`: players, size-1 NPCs, ground
//! items) or a per-triangle bounding-box test. Ground tiles run
//! `SceneGraph.drawPlainTile` / `drawShapedTile`'s point-in-triangle test to
//! find the walk destination. `scene_order` supplies the order in which the
//! original would have drawn them; this module holds the tests themselves.
//!
//! Coordinates are the original's: scene-local fine units (128 per tile),
//! `[x, height, y]` with height negative upward, and screen units in which
//! one unit is `1/512` of the view depth (`(x << 9) / depth`).

/// `GlobalConfig.VIEW_DISTANCE` (`TILE_DISTANCE * 128`).
pub const VIEW_DISTANCE: i32 = 56 * 128;
/// `SceneGraph.visibility` in HD (`GlobalConfig.TILE_DISTANCE`).
pub const TILE_DISTANCE: i32 = 56;

/// `MathUtils.sin`, a 2048-step table scaled by 65536.
pub fn sin(angle: i32) -> i32 {
    ((f64::from(angle & 2047) * 0.0030679615).sin() * 65536.0) as i32
}

/// `MathUtils.cos`.
pub fn cos(angle: i32) -> i32 {
    ((f64::from(angle & 2047) * 0.0030679615).cos() * 65536.0) as i32
}

/// The camera of one frame as the hover tests see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    /// `Camera.renderX`, `renderZ` (height) and `renderY`.
    pub eye: [i32; 3],
    pub sin_pitch: i32,
    pub cos_pitch: i32,
    pub sin_yaw: i32,
    pub cos_yaw: i32,
    /// `Rasteriser.screenLowerX` and `screenLowerY`.
    pub lower: [i32; 2],
    /// `Rasteriser.screenUpperX` and `screenUpperY`.
    pub upper: [i32; 2],
    /// `GlModel.pickScreenX` and `RawModel.pickScreenY`; None while
    /// `RawModel.allowInput` is false (mouse outside the viewport).
    pub pick: Option<[i32; 2]>,
}

impl View {
    /// A view at `eye` with the camera angles in 2048 units. The screen is
    /// unbounded and nothing is picked until `lower`, `upper` and `pick` are
    /// filled in.
    pub fn new(eye: [i32; 3], pitch: i32, yaw: i32) -> Self {
        Self {
            eye,
            sin_pitch: sin(pitch),
            cos_pitch: cos(pitch),
            sin_yaw: sin(yaw),
            cos_yaw: cos(yaw),
            lower: [-0x4000_0000; 2],
            upper: [0x4000_0000; 2],
            pick: None,
        }
    }

    /// The original's vertex rotation: an offset from the eye becomes
    /// `[screen x numerator, screen y numerator, depth]`.
    pub fn rotate(&self, [x, y, z]: [i32; 3]) -> [i32; 3] {
        let across = mix(z, self.sin_yaw, x, self.cos_yaw);
        let ahead = mix(z, self.cos_yaw, x.wrapping_neg(), self.sin_yaw);
        [
            across,
            mix(y, self.cos_pitch, ahead.wrapping_neg(), self.sin_pitch),
            mix(y, self.sin_pitch, ahead, self.cos_pitch),
        ]
    }

    /// The screen position of a view direction `[east, down, north]`, rounded
    /// down as the original's integer mouse scaling rounds. None for a
    /// direction that does not point into the view.
    pub fn direction(&self, [x, y, z]: [f64; 3]) -> Option<[i32; 2]> {
        let [sin_pitch, cos_pitch, sin_yaw, cos_yaw] =
            [self.sin_pitch, self.cos_pitch, self.sin_yaw, self.cos_yaw]
                .map(|n| f64::from(n) / 65536.0);
        let across = z * sin_yaw + x * cos_yaw;
        let ahead = z * cos_yaw - x * sin_yaw;
        let down = y * cos_pitch - ahead * sin_pitch;
        let depth = y * sin_pitch + ahead * cos_pitch;
        (depth > 1e-9).then(|| [across, down].map(|n| (n * 512.0 / depth).floor() as i32))
    }

    /// `ScriptRunner.renderGameScene`'s mouse scaling: the pick position of a
    /// mouse at `mouse` inside the viewport rectangle `[left, top, width,
    /// height]`, or None outside it.
    pub fn mouse(&self, mouse: [i32; 2], [left, top, width, height]: [i32; 4]) -> Option<[i32; 2]> {
        if mouse[0] < left
            || mouse[0] >= width.wrapping_add(left)
            || top > mouse[1]
            || height.wrapping_add(top) <= mouse[1]
            || width == 0
            || height == 0
        {
            return None;
        }
        Some([
            self.lower[0].wrapping_add(
                self.upper[0]
                    .wrapping_sub(self.lower[0])
                    .wrapping_mul(mouse[0].wrapping_sub(left))
                    .wrapping_div(width),
            ),
            self.upper[1]
                .wrapping_sub(self.lower[1])
                .wrapping_mul(mouse[1].wrapping_sub(top))
                .wrapping_div(height)
                .wrapping_add(self.lower[1]),
        ])
    }
}

/// `a * b + c * d >> 16` in Java's wrapping 32-bit arithmetic.
fn mix(a: i32, b: i32, c: i32, d: i32) -> i32 {
    a.wrapping_mul(b).wrapping_add(c.wrapping_mul(d)) >> 16
}

/// `GlBoundingBox` as `GlModel.calculateBounds` fills it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    pub min: [i16; 3],
    pub max: [i16; 3],
    pub radius: i16,
}

impl Bounds {
    pub fn of(vertices: &[[i32; 3]]) -> Self {
        let (mut min, mut max) = ([32767i32; 3], [-32768i32; 3]);
        let mut radius = 0i32;
        for v in vertices {
            for axis in 0..3 {
                min[axis] = min[axis].min(v[axis]);
                max[axis] = max[axis].max(v[axis]);
            }
            radius = radius.max(
                v[0].wrapping_mul(v[0])
                    .wrapping_add(v[2].wrapping_mul(v[2])),
            );
        }
        Self {
            min: min.map(|n| n as i16),
            max: max.map(|n| n as i16),
            // `(short) (Math.sqrt(max) + 0.99D)`: double to int, then to short.
            radius: (f64::from(radius).sqrt() + 0.99) as i32 as i16,
        }
    }
}

/// Which test follows the bounding-box test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// `Model.pickable`: the projected box is the whole test.
    Box,
    /// Every triangle's screen bounding box, with the original's lenient
    /// right edge and no alpha filter.
    Triangles,
}

/// `GlModel.render`'s hover test for a model drawn at `position` with model
/// yaw `yaw`. `faces` indexes `vertices`; None reads them three at a time.
/// False also covers a model the original would not have drawn at all.
pub fn hover(
    view: &View,
    bounds: &Bounds,
    vertices: &[[i32; 3]],
    faces: Option<&[[usize; 3]]>,
    yaw: i32,
    position: [i32; 3],
    rule: Rule,
) -> bool {
    if vertices.is_empty() {
        return false;
    }
    let scene = offset(view, position);
    let Some(pick) = frame(view, bounds, scene, false) else {
        return false;
    };
    let (sin_yaw, cos_yaw) = if yaw != 0 {
        (sin(yaw), cos(yaw))
    } else {
        (0, 0)
    };
    let project = |[mut x, y, mut z]: [i32; 3]| {
        if yaw != 0 {
            let turned = mix(z, sin_yaw, x, cos_yaw);
            z = mix(z, cos_yaw, x.wrapping_neg(), sin_yaw);
            x = turned;
        }
        view.rotate([
            x.wrapping_add(scene[0]),
            y.wrapping_add(scene[1]),
            z.wrapping_add(scene[2]),
        ])
    };
    let (mut low, mut high) = ([999999; 2], [-999999; 2]);
    for corner in 0..8 {
        let [x, y, depth] = project([
            i32::from(if corner & 1 == 0 {
                bounds.min[0]
            } else {
                bounds.max[0]
            }),
            i32::from(if corner & 4 == 0 {
                bounds.min[1]
            } else {
                bounds.max[1]
            }),
            i32::from(if corner & 2 == 0 {
                bounds.min[2]
            } else {
                bounds.max[2]
            }),
        ]);
        if depth > 0 {
            let screen = [x.wrapping_shl(9) / depth, y.wrapping_shl(9) / depth];
            for axis in 0..2 {
                low[axis] = low[axis].min(screen[axis]);
                high[axis] = high[axis].max(screen[axis]);
            }
        }
    }
    if (0..2).any(|axis| pick[axis] < low[axis] || pick[axis] > high[axis]) {
        return false;
    }
    if rule == Rule::Box {
        return true;
    }
    // One vertex nearer than 50 abandons the whole model.
    let mut screen = Vec::with_capacity(vertices.len());
    for &vertex in vertices {
        let [x, y, depth] = project(vertex);
        if depth < 50 {
            return false;
        }
        screen.push([x.wrapping_shl(9) / depth, y.wrapping_shl(9) / depth]);
    }
    let inside = |[a, b, c]: [usize; 3]| match (screen.get(a), screen.get(b), screen.get(c)) {
        (Some(a), Some(b), Some(c)) => within_triangle(pick, [*a, *b, *c]),
        _ => false,
    };
    match faces {
        Some(faces) => faces.iter().any(|&face| inside(face)),
        None => (0..vertices.len() / 3).any(|i| inside([i * 3, i * 3 + 1, i * 3 + 2])),
    }
}

/// The part of the hover test that needs only the bounds: true when the
/// model is drawn and the mouse is inside the rectangle around its bounding
/// cylinder. With `lenient`, bounds that are no smaller than the model's
/// never reject a model the exact bounds would accept.
pub fn near(view: &View, bounds: &Bounds, position: [i32; 3], lenient: bool) -> bool {
    frame(view, bounds, offset(view, position), lenient).is_some()
}

fn offset(view: &View, position: [i32; 3]) -> [i32; 3] {
    std::array::from_fn(|axis| position[axis].wrapping_sub(view.eye[axis]))
}

/// The cylinder culls and the first screen rectangle. Returns the pick point.
fn frame(view: &View, bounds: &Bounds, [x, y, z]: [i32; 3], lenient: bool) -> Option<[i32; 2]> {
    let (radius, min_y, max_y) = (
        i32::from(bounds.radius),
        i32::from(bounds.min[1]),
        i32::from(bounds.max[1]),
    );
    let (sin_pitch, cos_pitch) = (view.sin_pitch, view.cos_pitch);
    let ahead = mix(z, view.cos_yaw, x.wrapping_neg(), view.sin_yaw);
    let depth = mix(y, sin_pitch, ahead, cos_pitch);
    let near = depth.wrapping_add(mix(radius, cos_pitch, max_y, sin_pitch));
    if near <= 50 {
        return None;
    }
    let far = depth.wrapping_add(mix(-radius, cos_pitch, min_y, sin_pitch));
    if far >= VIEW_DISTANCE {
        return None;
    }
    let across = mix(z, view.sin_yaw, x, view.cos_yaw);
    let right = across.wrapping_add(radius).wrapping_shl(9);
    if right / near <= view.lower[0] {
        return None;
    }
    let left = across.wrapping_sub(radius).wrapping_shl(9);
    if left / near >= view.upper[0] {
        return None;
    }
    let down = mix(y, cos_pitch, ahead.wrapping_neg(), sin_pitch);
    let top = down
        .wrapping_add(mix(radius, sin_pitch, max_y, cos_pitch))
        .wrapping_shl(9);
    if top / near <= view.lower[1] {
        return None;
    }
    let bottom = down
        .wrapping_add(mix(-radius, sin_pitch, min_y, cos_pitch))
        .wrapping_shl(9);
    if bottom / near >= view.upper[1] {
        return None;
    }
    let pick = view.pick?;
    if far <= 0 {
        return lenient.then_some(pick);
    }
    let (min_x, max_x) = if across > 0 {
        (left / near, right / far)
    } else {
        (left / far, right / near)
    };
    let (min_y, max_y) = if down > 0 {
        (bottom / near, top / far)
    } else {
        (bottom / far, top / near)
    };
    (pick[0] >= min_x && pick[0] <= max_x && pick[1] >= min_y && pick[1] <= max_y).then_some(pick)
}

/// `GlModel.pointWithinTriangle`: the triangle's screen bounding box, except
/// that nothing bounds the point on the right.
fn within_triangle([x, y]: [i32; 2], t: [[i32; 2]; 3]) -> bool {
    if t.iter().all(|p| y < p[1]) || t.iter().all(|p| y > p[1]) || t.iter().all(|p| x < p[0]) {
        return false;
    }
    t.iter().any(|p| x <= p[0])
}

/// `SceneGraph.isRectVisible`: a point strictly inside a screen triangle.
fn inside_triangle([x, y]: [i32; 2], t: [[i32; 2]; 3]) -> bool {
    if t.iter().all(|p| y < p[1])
        || t.iter().all(|p| y > p[1])
        || t.iter().all(|p| x < p[0])
        || t.iter().all(|p| x > p[0])
    {
        return false;
    }
    let cross = |a: [i32; 2], b: [i32; 2]| {
        y.wrapping_sub(a[1])
            .wrapping_mul(b[0].wrapping_sub(a[0]))
            .wrapping_sub(x.wrapping_sub(a[0]).wrapping_mul(b[1].wrapping_sub(a[1])))
    };
    let (first, second, third) = (cross(t[0], t[1]), cross(t[2], t[0]), cross(t[1], t[2]));
    first.wrapping_mul(third) > 0 && third.wrapping_mul(second) > 0
}

fn tile_screen(view: &View, point: [i32; 3]) -> Option<[i32; 2]> {
    let [x, y, depth] = view.rotate(offset(view, point));
    (depth >= 50).then(|| [x.wrapping_shl(9) / depth, y.wrapping_shl(9) / depth])
}

fn facing(a: [i32; 2], b: [i32; 2], c: [i32; 2]) -> i32 {
    a[0].wrapping_sub(b[0])
        .wrapping_mul(c[1].wrapping_sub(b[1]))
        .wrapping_sub(
            a[1].wrapping_sub(b[1])
                .wrapping_mul(c[0].wrapping_sub(b[0])),
        )
}

/// `SceneGraph.drawPlainTile`'s walk test: whether screen point `point` is
/// on the two-triangle tile at scene tile `tile`, whose corner heights are
/// `[south-west, south-east, north-east, north-west]`.
pub fn plain_tile(view: &View, point: [i32; 2], tile: [i32; 2], heights: [i32; 4]) -> bool {
    let (x, y) = (tile[0].wrapping_shl(7), tile[1].wrapping_shl(7));
    let corner = |dx: i32, dy: i32, height: i32| {
        tile_screen(view, [x.wrapping_add(dx), height, y.wrapping_add(dy)])
    };
    let (Some(sw), Some(se), Some(ne), Some(nw)) = (
        corner(0, 0, heights[0]),
        corner(128, 0, heights[1]),
        corner(128, 128, heights[2]),
        corner(0, 128, heights[3]),
    ) else {
        return false;
    };
    let mut hit = false;
    // `(xNE - xNW) * (ySE - yNW) - (yNE - yNW) * (xSE - xNW) > 0`.
    if facing(ne, nw, se) > 0 && inside_triangle(point, [ne, nw, se]) {
        hit = true;
    }
    if facing(sw, se, nw) <= 0 {
        return hit;
    }
    hit || inside_triangle(point, [sw, se, nw])
}

/// `SceneGraph.drawShapedTile`'s walk test over a shaped tile's mesh, whose
/// vertices are `[x, height, y]` fine offsets within the tile.
pub fn shaped_tile(
    view: &View,
    point: [i32; 2],
    tile: [i32; 2],
    vertices: &[[i32; 3]],
    faces: impl IntoIterator<Item = [usize; 3]>,
) -> bool {
    let (x, y) = (tile[0].wrapping_shl(7), tile[1].wrapping_shl(7));
    let mut screen = Vec::with_capacity(vertices.len());
    for v in vertices {
        match tile_screen(view, [x.wrapping_add(v[0]), v[1], y.wrapping_add(v[2])]) {
            Some(p) => screen.push(p),
            None => return false,
        }
    }
    faces.into_iter().any(
        |[a, b, c]| match (screen.get(a), screen.get(b), screen.get(c)) {
            (Some(&a), Some(&b), Some(&c)) => {
                facing(a, b, c) > 0 && inside_triangle(point, [a, b, c])
            }
            _ => false,
        },
    )
}

/// `SceneGraph.addLoc`'s scene key for a location at scene tile `tile`.
/// A key that is not positive is never picked (`key > 0L` in the hover
/// test): locations that are not interactable, and — because the original
/// ORs the int `Integer.MIN_VALUE` into the long — every location whose
/// definition has opcode 96.
pub fn location_key(
    tile: [i32; 2],
    shape: i32,
    rotation: i32,
    id: i32,
    interactable: bool,
    support_items: bool,
    has_animation: bool,
) -> i64 {
    let mut key = i64::from(tile[0] | 0x4000_0000)
        | i64::from(tile[1]) << 7
        | i64::from(shape) << 14
        | i64::from(rotation) << 20;
    if !interactable {
        key |= i64::MIN;
    }
    if support_items {
        key |= 0x40_0000;
    }
    if has_animation {
        key |= i64::from(i32::MIN);
    }
    key | i64::from(id) << 32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camera south of the scene point `target`, looking north and down
    /// at 45 degrees from `distance` away.
    fn looking_at(target: [i32; 3], distance: i32) -> View {
        let step = (f64::from(distance) * std::f64::consts::FRAC_1_SQRT_2) as i32;
        let mut view = View::new([target[0], target[1] - step, target[2] - step], 256, 0);
        view.lower = [-256, -167];
        view.upper = [256, 167];
        view
    }

    fn cube(half: i32, height: i32) -> Vec<[i32; 3]> {
        let corner = |i: usize| {
            [
                if i & 1 == 0 { -half } else { half },
                if i & 2 == 0 { 0 } else { -height },
                if i & 4 == 0 { -half } else { half },
            ]
        };
        [
            [0, 1, 3],
            [0, 3, 2],
            [4, 6, 7],
            [4, 7, 5],
            [0, 4, 5],
            [0, 5, 1],
            [2, 3, 7],
            [2, 7, 6],
            [0, 2, 6],
            [0, 6, 4],
            [1, 5, 7],
            [1, 7, 3],
        ]
        .into_iter()
        .flatten()
        .map(corner)
        .collect()
    }

    #[test]
    fn a_view_looks_north_and_down_with_east_to_the_right() {
        let view = View::new([0, 0, 0], 0, 0);
        // Yaw 0 looks north: depth grows with y; x is screen right; a larger
        // height value (lower ground) is further down the screen.
        assert_eq!(view.rotate([0, 0, 1000]), [0, 0, 1000]);
        assert_eq!(view.rotate([100, 0, 0])[0], 100);
        assert!(view.rotate([0, 100, 1000])[1] > 0);
        // Yaw 512 faces west.
        assert!(View::new([0, 0, 0], 0, 512).rotate([-1000, 0, 0])[2] > 990);
        // A direction projects to where the integer rotation puts a far point.
        let view = View::new([0, 0, 0], 300, 700);
        let d = [0.3, 0.5, -0.6];
        let far = view.rotate(d.map(|n| (n * 4000.0) as i32));
        let screen = view.direction(d).unwrap();
        assert!((screen[0] - (far[0] << 9) / far[2]).abs() <= 1);
        assert!((screen[1] - (far[1] << 9) / far[2]).abs() <= 1);
        assert_eq!(view.direction([-0.3, -0.5, 0.6]), None);
    }

    #[test]
    fn mouse_scaling_is_the_identity_on_the_fixed_viewport() {
        let mut view = View::new([0; 3], 128, 0);
        view.lower = [-256, -167];
        view.upper = [256, 167];
        let rect = [4, 4, 512, 334];
        assert_eq!(view.mouse([4, 4], rect), Some([-256, -167]));
        assert_eq!(view.mouse([260, 171], rect), Some([0, 0]));
        assert_eq!(view.mouse([515, 337], rect), Some([255, 166]));
        for outside in [[3, 100], [516, 100], [100, 3], [100, 338]] {
            assert_eq!(view.mouse(outside, rect), None);
        }
    }

    #[test]
    fn a_boxed_model_is_hovered_anywhere_inside_its_projected_box() {
        let vertices = cube(40, 200);
        let bounds = Bounds::of(&vertices);
        assert_eq!(bounds.min, [-40, -200, -40]);
        assert_eq!(bounds.max, [40, 0, 40]);
        assert_eq!(bounds.radius, 57);
        let position = [6400, -500, 6400];
        let mut view = looking_at([6400, -600, 6400], 1200);
        let hovered = |view: &View, rule| hover(view, &bounds, &vertices, None, 0, position, rule);
        // Not picked while the mouse is outside the viewport.
        assert!(!hovered(&view, Rule::Box));
        view.pick = Some([0, 0]);
        assert!(hovered(&view, Rule::Box) && hovered(&view, Rule::Triangles));
        assert!(near(&view, &bounds, position, false));
        // Beside the model, inside the cylinder rectangle but outside the box.
        view.pick = Some([40, 0]);
        assert!(!hovered(&view, Rule::Box));
        // The corner of the projected box, where no triangle's own box is.
        let top_right = view
            .rotate(offset(&view, [6440, -700, 6440]))
            .map(|n| n << 9);
        let top_left = view
            .rotate(offset(&view, [6360, -700, 6360]))
            .map(|n| n << 9);
        view.pick = Some([
            top_right[0] / top_right[2] - 1,
            top_left[1] / top_left[2] + 1,
        ]);
        assert!(hovered(&view, Rule::Box));
    }

    #[test]
    fn the_triangle_test_uses_bounding_boxes_and_ignores_the_right_edge() {
        // One upright triangle, facing the camera; the point to the left of
        // its slanted edge is inside its bounding box, so it counts.
        let vertices = vec![[-100, 0, 0], [100, 0, 0], [100, -200, 0]];
        let bounds = Bounds::of(&vertices);
        let position = [6400, 0, 6400];
        let mut view = View::new([6400, -100, 5400], 0, 0);
        view.lower = [-256, -167];
        view.upper = [256, 167];
        let corner = view.rotate(offset(&view, [6300, -200, 6400]));
        // The empty upper-left corner of the triangle's bounding box. (The
        // very corner is outside the first rectangle, which divides the far
        // edge by the near depth.)
        view.pick = Some([
            (corner[0] << 9) / corner[2] + 6,
            (corner[1] << 9) / corner[2] + 6,
        ]);
        assert!(hover(
            &view,
            &bounds,
            &vertices,
            None,
            0,
            position,
            Rule::Triangles
        ));
        assert!(hover(
            &view,
            &bounds,
            &vertices,
            Some(&[[0, 1, 2]]),
            0,
            position,
            Rule::Triangles
        ));
        // Model yaw turns the model, not the camera: a quarter turn puts the
        // triangle edge-on, and its box collapses to a line at the centre.
        assert!(!hover(
            &view,
            &bounds,
            &vertices,
            None,
            512,
            position,
            Rule::Triangles
        ));
    }

    #[test]
    fn a_vertex_behind_the_near_plane_abandons_the_triangle_test_only() {
        // A small model right in front of the camera, one vertex 40 deep: the
        // box test still works but no triangle is tested.
        let vertices = vec![[-10, 0, -20], [10, 0, -20], [0, -30, 20]];
        let bounds = Bounds::of(&vertices);
        let mut view = View::new([6400, -100, 6340], 0, 0);
        view.lower = [-256, -167];
        view.upper = [256, 167];
        view.pick = Some([0, 0]);
        let position = [6400, -90, 6400];
        assert!(hover(
            &view,
            &bounds,
            &vertices,
            None,
            0,
            position,
            Rule::Box
        ));
        assert!(!hover(
            &view,
            &bounds,
            &vertices,
            None,
            0,
            position,
            Rule::Triangles
        ));
        // A model whose bounding cylinder reaches behind the eye is not
        // picked at all.
        let long = vec![[-50, 0, -3000], [50, 0, -3000], [0, 0, 3000]];
        let position = [6400, 300, 6400];
        assert!(!hover(
            &view,
            &Bounds::of(&long),
            &long,
            None,
            0,
            position,
            Rule::Box
        ));
    }

    #[test]
    fn models_outside_the_view_are_not_drawn() {
        let vertices = cube(40, 200);
        let bounds = Bounds::of(&vertices);
        let mut view = looking_at([6400, -600, 6400], 1200);
        view.pick = Some([0, 0]);
        for position in [
            [6400, -500, 6400 + VIEW_DISTANCE + 2000],
            [6400, -500, 3000],
            [6400 + 4000, -500, 6400],
            [6400 - 4000, -500, 6400],
        ] {
            assert!(!near(&view, &bounds, position, false), "{position:?}");
            assert!(!near(&view, &bounds, position, true), "{position:?}");
        }
        // Larger bounds never reject what the exact bounds accept.
        let loose = Bounds {
            radius: 4000,
            ..bounds
        };
        assert!(near(&view, &loose, [6400, -500, 6400], true));
    }

    #[test]
    fn walk_tests_find_the_tile_under_the_point() {
        let view = looking_at([6400 + 64, 0, 6400 + 64], 1500);
        let centre = tile_screen(&view, [6464, 0, 6464]).unwrap();
        assert!(plain_tile(&view, centre, [50, 50], [0; 4]));
        for other in [[49, 50], [51, 50], [50, 49], [50, 51]] {
            assert!(!plain_tile(&view, centre, other, [0; 4]));
        }
        // Both halves of the tile, and a raised corner moving the edge.
        let north_east = tile_screen(&view, [6400 + 110, 0, 6400 + 110]).unwrap();
        let south_west = tile_screen(&view, [6400 + 18, 0, 6400 + 18]).unwrap();
        assert!(plain_tile(&view, north_east, [50, 50], [0; 4]));
        assert!(plain_tile(&view, south_west, [50, 50], [0; 4]));
        // A tile behind the camera is never hit.
        assert!(!plain_tile(&view, centre, [50, 30], [0; 4]));
        // The shaped test agrees on the mesh the client builds for a flat
        // tile, in the winding the client's tile meshes use.
        for shape in 0..=12 {
            let mesh = crate::tile_mesh::build(shape, 0, [0; 4]).unwrap();
            let faces = mesh.faces.iter().map(|&(_, face)| face);
            assert!(
                shaped_tile(&view, centre, [50, 50], &mesh.vertices, faces.clone())
                    || shape > 1
                        && [north_east, south_west].iter().any(|&p| shaped_tile(
                            &view,
                            p,
                            [50, 50],
                            &mesh.vertices,
                            faces.clone()
                        )),
                "shape {shape}"
            );
            assert!(!shaped_tile(&view, centre, [50, 52], &mesh.vertices, faces));
        }
    }

    #[test]
    fn location_keys_follow_add_loc() {
        // Transcribed from SceneGraph.addLoc.
        let key = location_key([31, 47], 10, 2, 1530, true, false, false);
        assert_eq!(
            key,
            (1530i64 << 32) | 0x4000_0000 | 31 | (47 << 7) | (10 << 14) | (2 << 20)
        );
        assert!(key > 0);
        assert!(location_key([31, 47], 10, 2, 1530, false, false, false) < 0);
        assert_eq!(
            location_key([1, 2], 22, 0, 7, true, true, false) & 0x40_0000,
            0x40_0000
        );
        // Opcode 96 sign-extends: the key is negative and its id bits are lost.
        let animated = location_key([1, 2], 22, 0, 7, true, false, true);
        assert!(animated < 0);
        assert_eq!((animated as u64 >> 32) as u32, u32::MAX);
        assert_ne!(animated & 0x8000_0000, 0);
    }

    #[test]
    fn sine_table_endpoints() {
        assert_eq!([sin(0), cos(0)], [0, 65536]);
        assert_eq!(sin(512), 65535);
        assert_eq!(sin(256), 46340);
        assert_eq!(cos(1024), -65535);
        assert_eq!(sin(2048 + 256), sin(256));
    }
}
