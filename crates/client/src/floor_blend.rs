//! Edge feathering for adjacent terrain materials. The RT4 face shapes still
//! determine which material covers each tile and where its boundary lies.
use crate::tile_mesh;
use std::sync::OnceLock;

type Point = [f32; 2];

fn lerp(a: Point, b: Point, t: f32) -> Point {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn cross(a: Point, b: Point, p: Point) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

pub fn overlay_at(shape: usize, rotation: usize, point: Point) -> bool {
    type Face = [Point; 3];
    type Shapes = [[Vec<Face>; 4]; 13];
    static SHAPES: OnceLock<Shapes> = OnceLock::new();
    if shape == 0 {
        return false;
    }
    if shape == 1 {
        return true;
    }
    let shapes = SHAPES.get_or_init(|| {
        std::array::from_fn(|shape| {
            std::array::from_fn(|rotation| {
                let mesh = tile_mesh::build(shape, rotation, [0; 4]).unwrap();
                mesh.faces
                    .into_iter()
                    .filter(|&(overlay, _)| overlay)
                    .map(|(_, corners)| {
                        corners.map(|i| {
                            let [x, _, y] = mesh.vertices[i];
                            [x as f32, y as f32]
                        })
                    })
                    .collect()
            })
        })
    });
    shapes.get(shape).is_some_and(|turns| {
        turns[rotation & 3].iter().any(|&[a, b, c]| {
            let spans = [cross(a, b, point), cross(b, c, point), cross(c, a, point)];
            spans.iter().all(|&v| v >= 0.0) || spans.iter().all(|&v| v <= 0.0)
        })
    })
}

#[derive(Debug, Clone, Copy)]
pub struct Strip {
    pub edge: [f32; 2],
    pub inset: f32,
}

/// Sample adjacent faces directly rather than treating a shaped overlay as a
/// full tile. A tile boundary can meet two different faces of its neighbour.
/// Each emitted strip stays on the source triangle, even on sloped terrain.
pub fn strips<T: Copy + Eq>(
    [a, b, c]: [Point; 3],
    mut neighbor: impl FnMut(Point) -> Option<T>,
    mut emit: impl FnMut(T, Strip),
) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx.hypot(dy);
    let altitude = cross(a, b, c) / length;
    if !altitude.is_finite() || altitude.abs() < 1.0 {
        return;
    }
    let sign = 0.5 * altitude.signum() / length;
    let sample = |t| {
        let p = lerp(a, b, t);
        [p[0] + dy * sign, p[1] - dx * sign]
    };
    let inset = (40.0 / altitude.abs()).min(0.48);
    let crosses_tile = (a[0] == b[0] && (a[0] == 0.0 || a[0] == 128.0))
        || (a[1] == b[1] && (a[1] == 0.0 || a[1] == 128.0));
    let halves = crosses_tile && length == 128.0;
    let first = neighbor(sample(if halves { 0.25 } else { 0.5 }));
    let second = if halves {
        neighbor(sample(0.75))
    } else {
        first
    };
    if first == second {
        if let Some(material) = first {
            emit(
                material,
                Strip {
                    edge: [0.0, 1.0],
                    inset,
                },
            );
        }
    } else {
        for (material, edge) in [(first, [0.0, 0.5]), (second, [0.5, 1.0])] {
            if let Some(material) = material {
                emit(material, Strip { edge, inset });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_edge_splits_at_the_neighbours_half_tile_boundary() {
        let mut halves = Vec::new();
        strips(
            [[0.0, 0.0], [128.0, 0.0], [128.0, 128.0]],
            |[x, y]| {
                assert!(y < 0.0);
                (x > 64.0).then_some(8)
            },
            |material, strip| halves.push((material, strip)),
        );
        let [(8, strip)] = halves.as_slice() else {
            panic!("unexpected strips {halves:?}")
        };
        assert_eq!(strip.edge, [0.5, 1.0]);
        assert_eq!(strip.inset, 40.0 / 128.0);
    }

    #[test]
    fn shaped_overlay_lookup_covers_rotated_faces() {
        for shape in 0..13 {
            for rotation in 0..4 {
                let mesh = tile_mesh::build(shape, rotation, [0; 4]).unwrap();
                for (overlay, vertices) in mesh.faces {
                    let [a, b, c] = vertices.map(|i| mesh.vertices[i]);
                    assert_eq!(
                        overlay_at(
                            shape,
                            rotation,
                            [
                                (a[0] + b[0] + c[0]) as f32 / 3.0,
                                (a[2] + b[2] + c[2]) as f32 / 3.0
                            ]
                        ),
                        overlay
                    );
                }
            }
        }
    }
}
