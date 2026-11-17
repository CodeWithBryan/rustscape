//! Original RT4 minimap tile raster, fixed-point projection and HD texture geometry.
//! Scene/actor bindings and GPU presentation are supplied by the visual runtime.
use crate::{
    animation::sin_cos,
    minimap_tables::{ROTATIONS, SHAPE_MASKS},
};
mod scene;
pub use scene::{Indexed, MapScene, floor_rgb, scenery, stroke};
mod state;
pub use state::{Hint, HintPosition, State, hints_visible};
mod dots;
mod drift;
pub mod labels;
pub mod source;
pub use dots::{player_dot, username_key};
pub use drift::{Drift, Redraw, redraw};
pub use labels::{LabelFont, Labels};

pub const SIZE: usize = 512;
pub const TILES: usize = 104;

/// Equipment.init's ascending real-ID membership predicate, after item resolution.
pub fn equipment_ids(types: impl IntoIterator<Item = (i32, i32, i32)>) -> Vec<i32> {
    types
        .into_iter()
        .filter_map(|(id, man, woman)| (man >= 0 || woman >= 0).then_some(id))
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct Tile {
    /// Plain underlay0, plain overlay1, or original shaped tile2..12.
    pub shape: usize,
    pub rotation: usize,
    pub underlay: u32,
    pub overlay: u32,
}
impl Tile {
    pub fn draw(self, pixels: &mut [u32], index: usize) {
        if self.shape <= 1 {
            let color = if self.shape == 0 {
                self.underlay
            } else {
                self.overlay
            };
            if color != 0 {
                for y in 0..4 {
                    pixels[index + y * SIZE..index + y * SIZE + 4].fill(color);
                }
            }
        } else {
            let mask = &SHAPE_MASKS[self.shape];
            let rotation = &ROTATIONS[self.rotation];
            for i in 0..16 {
                if self.underlay != 0 || mask[rotation[i]] != 0 {
                    pixels[index + i / 4 * SIZE + i % 4] = if mask[rotation[i]] == 0 {
                        self.underlay
                    } else {
                        self.overlay
                    };
                }
            }
        }
    }
}

/// First minimap construction pass, including bridge render flags and original borders.
pub fn background(
    plane: usize,
    mut tile: impl FnMut(usize, usize, usize) -> Option<Tile>,
    mut flags: impl FnMut(usize, usize, usize) -> u8,
) -> Vec<u32> {
    let mut pixels = vec![1; SIZE * SIZE];
    for y in 1..103 {
        for x in 1..103 {
            let index = 4 * SIZE * (103 - y) + 24628 + (x - 1) * 4;
            if flags(plane, x, y) & 0x18 == 0 {
                if let Some(t) = tile(plane, x, y) {
                    t.draw(&mut pixels, index);
                }
            }
            if plane < 3 && flags(plane + 1, x, y) & 8 != 0 {
                if let Some(t) = tile(plane + 1, x, y) {
                    t.draw(&mut pixels, index);
                }
            }
        }
    }
    pixels
}

/// Component.buildClickMask scans the first nonzero run, including its empty-row quirk.
pub fn click_mask(width: usize, height: usize, indices: &[u8]) -> Vec<[i32; 2]> {
    (0..height)
        .map(|y| {
            let row = &indices[y * width..(y + 1) * width];
            let start = row.iter().position(|&v| v != 0).unwrap_or(0);
            let end = (start..width).find(|&x| row[x] == 0).unwrap_or(width);
            [start as i32, (end - start) as i32]
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default)]
pub struct View {
    pub yaw: i32,
    pub angle_offset: i32,
    pub zoom_offset: i32,
}
/// Original GlSprite.renderRotatedFixed transform for the trimmed edge-arrow
/// sheet. Its fixed fifteen-pixel pivot and negative GL angle are intentional.
pub fn edge_transform(center: [i32; 2], angle: i32) -> [f32; 5] {
    [
        center[0] as f32 / 16.,
        center[1] as f32 / 16.,
        angle.wrapping_neg() as f32 * 0.005493164,
        15.,
        15.,
    ]
}
impl View {
    /// Original minimap39 movement prefix and MiniMenu's fourteen-byte click trailer.
    pub fn walk_packet(
        self,
        route: &crate::walking::Route,
        origin: [i32; 2],
        run: bool,
        click: [i32; 2],
        fine: [i32; 2],
    ) -> Vec<u8> {
        use rs09_protocol::buffer::Writer;
        let mut bytes = route.packet(origin, run);
        bytes[0] = 39;
        bytes[1] = bytes[1].wrapping_add(14);
        let mut w = Writer::new();
        w.p1(click[0]);
        w.p1(click[1]);
        w.p2(self.yaw);
        w.p1(57);
        w.p1(self.angle_offset);
        w.p1(self.zoom_offset);
        w.p1(89);
        w.p2(fine[0]);
        w.p2(fine[1]);
        w.p1(i32::from(route.approximate));
        w.p1(63);
        bytes.extend(w.into_vec());
        bytes
    }
    pub fn angle(self) -> i32 {
        self.yaw.wrapping_add(self.angle_offset) & 2047
    }
    /// Returns the original sprite's inner top-left, before cropped-sprite offsets.
    pub fn icon(self, size: [i32; 2], sprite: [i32; 2], delta: [i32; 2]) -> Option<[i32; 2]> {
        let [x, y] = delta;
        let distance = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y));
        let radius = (size[0] / 2).max(size[1] / 2) + 10;
        if radius.wrapping_mul(radius) < distance {
            return None;
        }
        let (sin, cos) = sin_cos(self.angle());
        let zoom = self.zoom_offset.wrapping_add(256);
        let sin = sin.wrapping_mul(256).wrapping_div(zoom);
        let cos = cos.wrapping_mul(256).wrapping_div(zoom);
        let sx = sin.wrapping_mul(y).wrapping_add(x.wrapping_mul(cos)) >> 16;
        let sy = cos.wrapping_mul(y).wrapping_sub(x.wrapping_mul(sin)) >> 16;
        Some([
            size[0] / 2 + sx - sprite[0] / 2,
            size[1] / 2 - (sy + sprite[1] / 2),
        ])
    }
    /// Cs1ScriptRunner's inclusive right mask boundary is intentional.
    pub fn click(
        self,
        size: [i32; 2],
        mask: &[[i32; 2]],
        point: [i32; 2],
        fine: [i32; 2],
    ) -> Option<[i32; 2]> {
        let [px, py] = point;
        if px < 0 || py < 0 || px >= size[0] || py >= size[1] {
            return None;
        }
        let &[start, width] = mask.get(py as usize)?;
        if px < start || px > start + width {
            return None;
        }
        let x = px - size[0] / 2;
        let y = py - size[1] / 2;
        let (sin, cos) = sin_cos(self.angle());
        let sin = self.zoom_offset.wrapping_add(256).wrapping_mul(sin) >> 8;
        let cos = self.zoom_offset.wrapping_add(256).wrapping_mul(cos) >> 8;
        let dy = cos.wrapping_mul(y).wrapping_sub(sin.wrapping_mul(x)) >> 11;
        let dx = y.wrapping_mul(sin).wrapping_add(x.wrapping_mul(cos)) >> 11;
        Some([fine[0].wrapping_add(dx) >> 7, fine[1].wrapping_sub(dy) >> 7])
    }
}

/// GlSprite.renderRotatedTransparent: TR,TL,BL,BR UVs, with original odd-size truncation.
pub fn rotated_uv(
    size: [i32; 2],
    source: [i32; 2],
    angle: i32,
    zoom: i32,
    power: [i32; 2],
) -> [[f32; 2]; 4] {
    let half = [-size[0] / 2, -size[1] / 2];
    let sin = ((f64::from(angle) / 326.11).sin() * 65536.0) as i32;
    let cos = ((f64::from(angle) / 326.11).cos() * 65536.0) as i32;
    let sin = sin.wrapping_mul(zoom) >> 8;
    let cos = cos.wrapping_mul(zoom) >> 8;
    [
        [-half[0], half[1]],
        [half[0], half[1]],
        [half[0], -half[1]],
        [-half[0], -half[1]],
    ]
    .map(|[x, y]| {
        let sx = source[0]
            .wrapping_shl(16)
            .wrapping_add(y.wrapping_mul(sin))
            .wrapping_add(x.wrapping_mul(cos));
        let sy = source[1]
            .wrapping_shl(16)
            .wrapping_add(y.wrapping_mul(cos).wrapping_sub(x.wrapping_mul(sin)));
        [
            sx as f32 / (power[0] as f32 * 65536.0),
            sy as f32 / (power[1].wrapping_mul(65536) as f32),
        ]
    })
}

/// MathUtils and GlSprite use different angle constants; retain both paths above.
pub fn jitter(deltas: [f64; 4]) -> [i32; 3] {
    let a = (deltas[0] * 21.0) as i32 - 10;
    let b = (deltas[1] * 21.0) as i32 - 10;
    let c = (deltas[2] * 21.0) as i32 - 10;
    let base = (deltas[3] * 41.0) as i32 - 20;
    [base + c, base + a, base + b]
}
