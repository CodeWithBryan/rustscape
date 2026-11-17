//! What the development frame dump (`RS09_FRAME_DUMP`, a file `take`)
//! writes: the window's pixels as the window shows them.
//!
//! The original blends translucent faces with one `glBlendFunc(GL_SRC_ALPHA,
//! GL_ONE_MINUS_SRC_ALPHA)` for colour and alpha alike (`GlRenderer.java:
//! 295-296`). A face of opacity `a` drawn over an opaque pixel leaves the
//! colour blended as it should be and the alpha at `a * a + 1 - a`: 0.78
//! under the innermost ring of a character shadow (`ShadowModelList`,
//! opacity 82/255), 0.83 and 0.90 under the other two. Nobody sees that
//! alpha: a desktop window's client area is opaque (the original's AWT
//! canvas and this client's window alike), and the browser page asks WebGL
//! for a context without alpha (`web/visual/graphics_lifecycle.js`).
//!
//! This client's persistent static meshes already keep the destination
//! alpha (`static_render`); its streaming world material (`render.rs`:
//! actors, their shadows, hint arrows, ground items, interface models) blended
//! it like the original until 2026-11-17, so a PNG of the framebuffer as read
//! carried it, and an image tool that composited the PNG (the frame
//! comparison's crop, over black) showed every such face darker than the
//! window did. That is where the comparison's "darker character shadows"
//! (0.60 / 0.70 / 0.83 of the floor instead of the original's 0.77 / 0.85 /
//! 0.92) and the dark patch on a wall in front of a shadow came from: the
//! wall, drawn later by `static_render`, kept the alpha the shadow had left
//! on the floor behind it. The window showed neither. Now the world
//! material keeps the alpha too, and the dump is written opaque in any case
//! (interface sprites still blend alpha).

/// Makes every pixel of an RGBA buffer opaque, leaving its colour as is.
pub fn opaque(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
}

/// The alpha that one `GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA` blend of a face
/// of opacity `source` leaves over a pixel of alpha `destination` (both in
/// 0..=255, as the framebuffer stores them; rounded to the nearest, as an
/// unsigned normalized conversion does).
pub fn blended_alpha(source: u8, destination: u8) -> u8 {
    let a = f64::from(source) / 255.0;
    let d = f64::from(destination) / 255.0;
    ((a * a + d * (1.0 - a)) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_is_kept_and_alpha_made_opaque() {
        let mut pixels = [10, 20, 30, 199, 1, 2, 3, 255, 4, 5, 6, 0];
        opaque(&mut pixels);
        assert_eq!(pixels, [10, 20, 30, 255, 1, 2, 3, 255, 4, 5, 6, 255]);
    }

    /// The alpha values found under the three shadow rings in the frame
    /// dumps of the comparison (`refcompare-evidence`, `rust-v3-*` and
    /// `rust-v4-probe*`): one draw of each ring over the opaque floor. The
    /// ring opacities are `255 - transparency` of
    /// `ShadowModelList.getOrCreateShadow(160, ..., 240)`, whose rings have
    /// the transparencies 173, 200 and 226 (`ShadowModelList.java`, `alpha`
    /// per ring); an NPC's defaults -96 and -16 give the same bytes.
    #[test]
    fn one_shadow_ring_over_the_floor_leaves_the_alpha_seen_in_the_dumps() {
        let rings: Vec<u8> = [173u8, 200, 226]
            .iter()
            .map(|t| blended_alpha(255 - t, 255))
            .collect();
        assert_eq!(rings, [199, 212, 229]);
        // A ring over another (two shadows on one spot): lower again.
        assert_eq!(blended_alpha(82, 199), 161);
    }
}
