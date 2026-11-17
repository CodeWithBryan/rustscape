use macroquad::prelude::{Mat4, Vec3};
use rs09_client::frustum::Frustum;

// Independent double-precision polygon clipping models what the unchanged GPU
// side planes accept; the production optimization uses support points of boxes.
fn clipped_triangle(m: [f32; 16], triangle: [[f32; 3]; 3]) -> bool {
    let mut polygon: Vec<[f64; 4]> = triangle
        .into_iter()
        .map(|v| {
            std::array::from_fn(|r| {
                (0..3).fold(f64::from(m[12 + r]), |sum, c| {
                    sum + f64::from(m[c * 4 + r]) * f64::from(v[c])
                })
            })
        })
        .collect();
    for plane in 0..4 {
        let distance = |v: [f64; 4]| match plane {
            0 => v[3] + v[0],
            1 => v[3] - v[0],
            2 => v[3] + v[1],
            _ => v[3] - v[1],
        };
        let input = std::mem::take(&mut polygon);
        for i in 0..input.len() {
            let a = input[i];
            let b = input[(i + 1) % input.len()];
            let da = distance(a);
            let db = distance(b);
            if da >= 0.0 {
                polygon.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                let t = da / (da - db);
                polygon.push(std::array::from_fn(|j| a[j] + t * (b[j] - a[j])));
            }
        }
        if polygon.is_empty() {
            return false;
        }
    }
    polygon.iter().any(|v| v[3] > 0.0000001)
}

#[test]
fn rejected_boxes_cannot_contain_triangles_accepted_by_gpu_side_clipping() {
    let mut seed = 530u64;
    let mut random = |lo: f32, hi: f32| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        lo + (hi - lo) * ((seed >> 32) as u32 as f64 / u32::MAX as f64) as f32
    };
    let faces = [
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
    ];
    let mut rejected = 0;
    for _ in 0..256 {
        let eye = Vec3::new(random(-80., 80.), random(0.1, 60.), random(-80., 80.));
        let target = Vec3::new(random(-30., 30.), random(-10., 10.), random(-30., 30.));
        let matrix = (Mat4::perspective_rh_gl(random(0.3, 1.6), random(0.5, 2.5), 0.1, 300.)
            * Mat4::look_at_rh(eye, target, Vec3::Y))
        .to_cols_array();
        let frustum = Frustum::new(matrix);
        for _ in 0..32 {
            let min = [random(-150., 150.), random(-40., 40.), random(-150., 150.)];
            let max = std::array::from_fn(|i| min[i] + random(0.01, 40.));
            if frustum.intersects([min, max]) {
                continue;
            }
            rejected += 1;
            let vertices: Vec<[f32; 3]> = (0..8)
                .map(|n| std::array::from_fn(|i| if n & (1 << i) == 0 { min[i] } else { max[i] }))
                .collect();
            for face in faces {
                assert!(
                    !clipped_triangle(matrix, face.map(|i| vertices[i])),
                    "camera{eye:?} target{target:?} box{min:?}/{max:?}"
                );
            }
        }
    }
    assert!(rejected > 1000);
}

#[test]
fn clipping_retains_boundaries_uncertain_inputs_and_depth_biased_geometry() {
    let identity = Frustum::new(Mat4::IDENTITY.to_cols_array());
    assert!(identity.intersects([[1., -0.1, -1.0e20], [2., 0.1, 1.0e20]]));
    assert!(!identity.intersects([[1.01, -0.1, -1.], [2., 0.1, 1.]]));
    assert!(identity.intersects([[-0.1, -0.1, 1.0e20], [0.1, 0.1, 2.0e20]]));
    assert!(identity.intersects([[f32::NEG_INFINITY; 3], [f32::INFINITY; 3]]));
    assert!(identity.intersects([[f32::NAN; 3], [0.; 3]]));
    assert!(identity.intersects([[1.; 3], [-1.; 3]]));
    assert!(Frustum::new([f32::NAN; 16]).intersects([[10.; 3], [20.; 3]]));
}
