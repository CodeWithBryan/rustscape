//! Ground-pile terrain interpolation and support elevation from RT4 SceneGraph.

/// The callback returns the original signed height and render flags for a tile.
/// The scene supplies a nonnull height grid; unavailable grids return0 upstream.
pub fn tile_height(
    plane: i32,
    fine: [i32; 2],
    mut tile: impl FnMut(i32, i32, i32) -> (i32, u8),
) -> i32 {
    let [x, y] = fine.map(|n| n >> 7);
    if !(0..104).contains(&x) || !(0..104).contains(&y) || !(0..4).contains(&plane) {
        return 0;
    }
    let [dx, dy] = fine.map(|n| n & 127);
    let p = if plane < 3 && tile(1, x, y).1 & 2 == 2 {
        plane + 1
    } else {
        plane
    };
    let z0 = dx
        .wrapping_mul(tile(p, x + 1, y + 1).0)
        .wrapping_add(tile(p, x, y + 1).0.wrapping_mul(128 - dx))
        >> 7;
    let z1 = dx
        .wrapping_mul(tile(p, x + 1, y).0)
        .wrapping_add((128 - dx).wrapping_mul(tile(p, x, y).0))
        >> 7;
    dy.wrapping_mul(z0)
        .wrapping_add((128 - dy).wrapping_mul(z1))
        >> 7
}

/// Only flagged centrepieces support a pile. The original -32768 sentinel does
/// not raise items; positive minY also leaves the stack on the ground.
pub fn support_height(scenery: impl IntoIterator<Item = (bool, i32)>) -> i32 {
    let mut min_y = 0;
    for (supports, y) in scenery {
        if supports && y != -32768 && y < min_y {
            min_y = y;
        }
    }
    min_y.wrapping_neg()
}
