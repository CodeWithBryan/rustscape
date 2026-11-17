//! `GlRaster.drawRect` and `drawRectAlpha` (78-109): an unfilled rectangle is
//! a GL line loop through four corners nudged by 0.3, which lights exactly
//! the one-pixel border of the rectangle.
use super::widget_legacy::Geometry;

/// The loop's corners in the original's GL canvas space (y up), computed with
/// the same single-precision steps.
pub fn rect([x, y, width, height]: [i32; 4], canvas_height: i32) -> Geometry {
    let left = x as f32 + 0.3;
    let right = left + width.wrapping_sub(1) as f32;
    let top = canvas_height as f32 - y as f32 - 0.3;
    let bottom = top - height.wrapping_sub(1) as f32;
    Geometry {
        thin: true,
        vertices: vec![[left, top], [left, bottom], [right, bottom], [right, top]],
    }
}

/// The pixels that loop lights, as up to four one-pixel strips `[x, y, w, h]`
/// that do not overlap: top row, bottom row, then the left and right columns
/// between them. A multisampled presenter blends a GL line at about half
/// strength, so it fills these strips instead; each pixel is covered once,
/// as with the original's loop, which matters for a translucent outline.
///
/// A 1 by 1 rectangle gives nothing: its four corners coincide and GL draws
/// no fragment for a zero-length segment. Rectangles with a non-positive
/// side also give nothing here; the original would draw a loop running
/// backwards, which no cached component asks for.
pub fn strips([x, y, width, height]: [i32; 4]) -> Vec<[i32; 4]> {
    if width < 1 || height < 1 || (width == 1 && height == 1) {
        return Vec::new();
    }
    let mut out = vec![[x, y, width, 1]];
    if height > 1 {
        out.push([x, y.wrapping_add(height - 1), width, 1]);
    }
    if height > 2 {
        out.push([x, y.wrapping_add(1), 1, height - 2]);
        if width > 1 {
            out.push([x.wrapping_add(width - 1), y.wrapping_add(1), 1, height - 2]);
        }
    }
    out
}

/// `glColor4ub`'s alpha argument: `alpha > 255 ? -1 : (byte) alpha`.
pub fn alpha_byte(alpha: i32) -> u8 {
    if alpha > 255 { 255 } else { alpha as u8 }
}

/// The alpha byte a type 3 component is drawn with (`Cs1ScriptRunner`
/// 717-739): opaque when the component's alpha is zero, otherwise
/// `256 - (alpha & 0xFF)` through [`alpha_byte`].
pub fn component_alpha(alpha: i32) -> u8 {
    if alpha == 0 {
        255
    } else {
        alpha_byte(256 - (alpha & 0xff))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// Pixels (x, y from the top) that axis-aligned GL line segments light
    /// under the OpenGL diamond-exit rule: a fragment is produced when the
    /// segment leaves the diamond of radius one half around a pixel centre,
    /// so a segment's last pixel is left to the segment that starts there.
    fn lit(vertices: &[[f32; 2]], canvas_height: i32) -> BTreeSet<(i32, i32)> {
        let inside = |p: [f32; 2], cell: [i32; 2]| {
            (p[0] - (cell[0] as f32 + 0.5)).abs() + (p[1] - (cell[1] as f32 + 0.5)).abs() < 0.5
        };
        let mut out = BTreeSet::new();
        for i in 0..vertices.len() {
            let (a, b) = (vertices[i], vertices[(i + 1) % vertices.len()]);
            if a == b {
                continue;
            }
            let horizontal = a[1] == b[1];
            assert!(horizontal || a[0] == b[0], "axis-aligned segments only");
            let axis = usize::from(!horizontal);
            let (low, high) = (a[axis].min(b[axis]), a[axis].max(b[axis]));
            for step in low.floor() as i32..=high.floor() as i32 {
                let cell = if horizontal {
                    [step, a[1].floor() as i32]
                } else {
                    [a[0].floor() as i32, step]
                };
                // The segment touches this diamond somewhere along its span...
                let centre = cell[axis] as f32 + 0.5;
                let half = 0.5 - (a[1 - axis] - (cell[1 - axis] as f32 + 0.5)).abs();
                if half <= 0.0 || high <= centre - half || low >= centre + half {
                    continue;
                }
                // ...and lights it unless it ends inside.
                if !inside(b, cell) {
                    out.insert((cell[0], canvas_height - 1 - cell[1]));
                }
            }
        }
        out
    }
    fn border([x, y, w, h]: [i32; 4]) -> BTreeSet<(i32, i32)> {
        let mut out = BTreeSet::new();
        for px in x..x + w {
            out.insert((px, y));
            out.insert((px, y + h - 1));
        }
        for py in y..y + h {
            out.insert((x, py));
            out.insert((x + w - 1, py));
        }
        out
    }

    // Transcription test of GlRaster.drawRect's vertex arithmetic.
    #[test]
    fn corners_are_the_original_floats() {
        let g = rect([10, 20, 30, 40], 503);
        assert!(g.thin);
        assert_eq!(
            g.vertices,
            vec![
                [10.0f32 + 0.3, 503.0f32 - 20.0 - 0.3],
                [10.0f32 + 0.3, 503.0f32 - 20.0 - 0.3 - 39.0],
                [10.0f32 + 0.3 + 29.0, 503.0f32 - 20.0 - 0.3 - 39.0],
                [10.0f32 + 0.3 + 29.0, 503.0f32 - 20.0 - 0.3],
            ]
        );
    }

    // The loop lights the rectangle's border and nothing else. This rests on
    // the diamond-exit model above, not on a GL implementation; a capture of
    // interface 320's three outlines was the check against a real rasteriser.
    #[test]
    fn the_loop_lights_exactly_the_border() {
        for r in [
            [10, 20, 30, 40],
            [0, 0, 765, 503],
            [3, 4, 2, 2],
            [5, 6, 1, 9],
            [5, 6, 9, 1],
            [700, 400, 17, 3],
        ] {
            assert_eq!(lit(&rect(r, 503).vertices, 503), border(r), "{r:?}");
            // The strips the presenter fills are those same pixels, each once.
            let mut filled = BTreeSet::new();
            for [sx, sy, sw, sh] in strips(r) {
                for px in sx..sx + sw {
                    for py in sy..sy + sh {
                        assert!(filled.insert((px, py)), "{r:?} covers {px},{py} twice");
                    }
                }
            }
            assert_eq!(filled, border(r), "{r:?}");
        }
        // Four coincident corners: GL draws nothing for zero-length segments.
        assert!(lit(&rect([8, 9, 1, 1], 503).vertices, 503).is_empty());
        assert!(strips([8, 9, 1, 1]).is_empty());
        assert!(strips([8, 9, 0, 5]).is_empty());
        assert!(strips([8, 9, 5, -1]).is_empty());
    }

    #[test]
    fn alpha_bytes_follow_the_original_and_survive_the_colour_round_trip() {
        assert_eq!(alpha_byte(256), 255);
        assert_eq!(alpha_byte(255), 255);
        assert_eq!(alpha_byte(128), 128);
        assert_eq!(alpha_byte(1), 1);
        assert_eq!(component_alpha(0), 255);
        assert_eq!(component_alpha(1), 255);
        assert_eq!(component_alpha(128), 128);
        assert_eq!(component_alpha(255), 1);
        // 256 & 0xFF is 0: a component alpha of 256 draws through the alpha
        // path at full strength.
        assert_eq!(component_alpha(256), 255);
        // The presenter stores colours as f32 and converts back with
        // `(v * 255.) as u8`; every byte must come back unchanged.
        for byte in 0..=255u8 {
            assert_eq!((f32::from(byte) / 255.0 * 255.0) as u8, byte);
        }
    }
}
