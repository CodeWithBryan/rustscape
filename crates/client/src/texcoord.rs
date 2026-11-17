//! Texture coordinates as the original's graphics card addressed them.
//!
//! `GlModel`'s constructor (`GlModel.java:454-495`) maps a face of texture
//! type 0 into its texture triangle P, M, N in float arithmetic:
//! `invDet2 = 1.0F / (invRow2 . edgeB)` and `v = (invRow2 . d) * invDet2`.
//! When a face's texture triangle is edge-on to the second axis, as on the
//! flat tops of Lumbridge castle's walls (texture 147 on locations 33858 and
//! 33864, models 36668 and 36673), the dot product is zero, `invDet2` is
//! infinite and `v` is `0 * Infinity`, NaN. `surface::prepare` computes the
//! same NaN. The original hands the NaN to OpenGL unchanged
//! (`GlModel.packInterleavedVertexData` writes `vertexS`/`vertexT` as they
//! are), and the face is drawn: the original client run against the example
//! server shows those wall tops in the texture's colour
//! (`refcompare-evidence` pairs `v2-B1`, `v3-B1`).
//!
//! OpenGL leaves a NaN texture coordinate undefined. Graphics hardware of the
//! Direct3D 10 generation and later converts a NaN coordinate to 0 when it
//! addresses a texel, which is what the original's frames show. This
//! client's shader would instead carry the NaN through its own wrapping and
//! filtering arithmetic, sample nothing and discard the fragment (its alpha
//! test), so the wall tops were missing. [`gpu`] makes the substitution
//! before the coordinate reaches the shader.
//!
//! A sweep of the pinned cache (every location type with models, its first
//! shape at rotation 0, `surface::prepare`) found 714 of 40,211 types with
//! such faces, 25,126 faces in all, every one of them left undrawn before
//! this. The same sweep found no infinite coordinate (a non-zero numerator
//! over the zero determinant), so the hardware rule for those (clamping to
//! the largest float) is not reproduced; they are left as they are. Actor
//! and interface models were not swept.

/// A face corner's texture coordinate with NaN components addressed as 0.
pub fn gpu(uv: [f32; 2]) -> [f32; 2] {
    uv.map(|c| if c.is_nan() { 0.0 } else { c })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nan_components_are_addressed_as_zero_and_others_kept() {
        assert_eq!(gpu([0.578125, f32::NAN]), [0.578125, 0.0]);
        assert_eq!(gpu([f32::NAN, f32::NAN]), [0.0, 0.0]);
        assert_eq!(gpu([-1.25, 3.5]), [-1.25, 3.5]);
        let infinite = gpu([f32::INFINITY, 0.5]);
        assert!(infinite[0].is_infinite() && infinite[1] == 0.5);
    }

    /// GlModel.java:485-495 in float: an edge-on texture axis gives
    /// `0 * (1 / 0)`, which is NaN in Java as here.
    #[test]
    fn an_edge_on_texture_axis_gives_nan_as_in_java_float_arithmetic() {
        let numerator = 0.0f32;
        let inv_det2 = 1.0f32 / 0.0f32;
        let v = numerator * inv_det2;
        assert!(v.is_nan());
        assert_eq!(gpu([0.5, v]), [0.5, 0.0]);
    }
}
