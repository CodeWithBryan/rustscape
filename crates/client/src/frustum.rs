//! Conservative submission culling against the GPU's four side clip planes.
//! Near/far clipping stays on the GPU because the shader applies a depth bias.

#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    planes: [[f32; 4]; 4],
    finite: bool,
}
impl Frustum {
    /// The same column-major projection/view matrix uploaded to the world shader.
    pub fn new(matrix: [f32; 16]) -> Self {
        let row = |n| [matrix[n], matrix[n + 4], matrix[n + 8], matrix[n + 12]];
        let w = row(3);
        let x = row(0);
        let y = row(1);
        let planes = [
            std::array::from_fn(|i| w[i] + x[i]),
            std::array::from_fn(|i| w[i] - x[i]),
            std::array::from_fn(|i| w[i] + y[i]),
            std::array::from_fn(|i| w[i] - y[i]),
        ];
        Self {
            planes,
            finite: matrix.into_iter().all(f32::is_finite),
        }
    }
    /// Retain uncertain/invalid bounds. Only a box wholly beyond one plane can
    /// be omitted; vertices remaining after scenery removal may enlarge the box.
    pub fn intersects(&self, bounds: [[f32; 3]; 2]) -> bool {
        if !self.finite
            || bounds.iter().flatten().any(|n| !n.is_finite())
            || (0..3).any(|i| bounds[0][i] > bounds[1][i])
        {
            return true;
        }
        for p in self.planes {
            let mut distance = p[3];
            let mut magnitude = p[3].abs();
            for i in 0..3 {
                let vertex = bounds[usize::from(p[i] >= 0.0)][i];
                distance += p[i] * vertex;
                magnitude += p[i].abs() * bounds[0][i].abs().max(bounds[1][i].abs());
            }
            // Account for separate GPU matrix products versus combined CPU
            // planes, including cancellation near an edge and float rounding.
            let tolerance = magnitude * (32.0 * f32::EPSILON) + 0.001;
            if distance < -tolerance {
                return false;
            }
        }
        true
    }
}
