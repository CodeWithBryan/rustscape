//! Shared native logical scenery with local presentation picking helpers.
pub use rs09_client_data::scenery::*;

/// Float ray for the workbench camera. Exact RT4 viewport/picking is a separate
/// presentation boundary; intersections select the geometry actually displayed.
#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
}
impl Ray {
    pub fn bounds(&self, min: [f32; 3], max: [f32; 3]) -> bool {
        let mut near = 0.0f32;
        let mut far = f32::INFINITY;
        for axis in 0..3 {
            if self.direction[axis].abs() < 1e-8 {
                if self.origin[axis] < min[axis] || self.origin[axis] > max[axis] {
                    return false;
                }
            } else {
                let a = (min[axis] - self.origin[axis]) / self.direction[axis];
                let b = (max[axis] - self.origin[axis]) / self.direction[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far {
                    return false;
                }
            }
        }
        true
    }
    pub fn triangle(&self, a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<f32> {
        let sub = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| a[i] - b[i]);
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let dot = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
        let e1 = sub(b, a);
        let e2 = sub(c, a);
        let p = cross(self.direction, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-7 {
            return None;
        }
        let t = sub(self.origin, a);
        let u = dot(t, p) / det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = cross(t, e1);
        let v = dot(self.direction, q) / det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let distance = dot(e2, q) / det;
        (distance >= 0.0).then_some(distance)
    }
}
