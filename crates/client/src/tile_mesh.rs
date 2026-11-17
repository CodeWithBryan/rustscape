//! RT4 ShapedTile geometry, before scene lighting and texture projection.
pub struct TileMesh {
    pub vertices: Vec<[i32; 3]>,
    pub faces: Vec<(bool, [usize; 3])>,
}
const VERTICES: [&[usize]; 13] = [
    &[1, 3, 5, 7],
    &[1, 3, 5, 7],
    &[1, 3, 5, 7],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 2, 6],
    &[1, 3, 5, 7, 2, 8],
    &[1, 3, 5, 7, 2, 8],
    &[1, 3, 5, 7, 11, 12],
    &[1, 3, 5, 7, 11, 12],
    &[1, 3, 5, 7, 13, 14],
];
const FACES: [&[usize]; 13] = [
    &[0, 1, 2, 3, 0, 0, 1, 3],
    &[1, 1, 2, 3, 1, 0, 1, 3],
    &[0, 1, 2, 3, 1, 0, 1, 3],
    &[0, 0, 1, 2, 0, 0, 2, 4, 1, 0, 4, 3],
    &[0, 0, 1, 4, 0, 0, 4, 3, 1, 1, 2, 4],
    &[0, 0, 4, 3, 1, 0, 1, 2, 1, 0, 2, 4],
    &[0, 1, 2, 4, 1, 0, 1, 4, 1, 0, 4, 3],
    &[0, 4, 1, 2, 0, 4, 2, 5, 1, 0, 4, 5, 1, 0, 5, 3],
    &[0, 4, 1, 2, 0, 4, 2, 3, 0, 4, 3, 5, 1, 0, 4, 5],
    &[0, 0, 4, 5, 1, 4, 1, 2, 1, 4, 2, 3, 1, 4, 3, 5],
    &[
        0, 0, 1, 5, 0, 1, 4, 5, 0, 1, 2, 4, 1, 0, 5, 3, 1, 5, 4, 3, 1, 4, 2, 3,
    ],
    &[
        1, 0, 1, 5, 1, 1, 4, 5, 1, 1, 2, 4, 0, 0, 5, 3, 0, 5, 4, 3, 0, 4, 2, 3,
    ],
    &[
        1, 0, 5, 4, 1, 0, 1, 5, 0, 0, 4, 3, 0, 4, 5, 3, 0, 5, 2, 3, 0, 1, 2, 5,
    ],
];
pub fn build(shape: usize, rotation: usize, heights: [i32; 4]) -> Option<TileMesh> {
    let indices = VERTICES.get(shape)?;
    let mut vertices = Vec::new();
    let [sw, se, ne, nw] = heights;
    let positions = [
        [0, sw, 0],
        [64, (sw + se) >> 1, 0],
        [128, se, 0],
        [128, (se + ne) >> 1, 64],
        [128, ne, 128],
        [64, (ne + nw) >> 1, 128],
        [0, nw, 128],
        [0, (nw + sw) >> 1, 64],
        [64, (sw + se) >> 1, 32],
        [96, (se + ne) >> 1, 64],
        [64, (ne + nw) >> 1, 96],
        [32, (nw + sw) >> 1, 64],
        [32, sw, 32],
        [96, se, 32],
        [96, ne, 96],
        [32, nw, 96],
    ];
    for &code in indices.iter() {
        let code = if code <= 8 && code & 1 == 0 {
            ((code.wrapping_sub(rotation * 2 + 1)) & 7) + 1
        } else if code > 8 && code <= 12 {
            ((code.wrapping_sub(rotation + 9)) & 3) + 9
        } else if code > 12 {
            ((code.wrapping_sub(rotation + 13)) & 3) + 13
        } else {
            code
        };
        vertices.push(positions[code - 1]);
    }
    let faces = FACES[shape]
        .chunks_exact(4)
        .map(|f| {
            (
                f[0] != 0,
                [f[1], f[2], f[3]].map(|i| {
                    if i < 4 {
                        (i.wrapping_sub(rotation)) & 3
                    } else {
                        i
                    }
                }),
            )
        })
        .collect();
    Some(TileMesh { vertices, faces })
}
