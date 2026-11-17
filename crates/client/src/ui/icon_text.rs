//! The stack count the original draws into an item's 36 by 32 sprite
//! (`Inv.renderObjectSprite` 252-254, `Inv.formatObjAmount`), with the
//! software font path it goes through (`Font.render` 228-323,
//! `SoftwareFont.renderGlyph` 185-224 and `blit`), and the brightness the
//! software rasteriser has when it paints item models.
use super::{font::Metrics, small_text::parse_radix, sprite::Sheet};

/// `Rasteriser.setBrightness` as the HD client leaves it: 0.7 while high
/// detail lighting is on (`DisplayMode` 235-237, `ScriptRunner` 5138-5139),
/// otherwise by the brightness preference 1 to 4 (`client` 1585-1596,
/// `ScriptRunner` 5034-5047). Item sprites are painted by the software
/// rasteriser with this palette and texture brightness even in HD.
///
/// The original then adds a random offset of up to 0.015 either way
/// (`Rasteriser.randBrightness`); this is the value it is centred on.
pub fn raster_brightness(preference: i32, high_detail_lighting: bool) -> f32 {
    if high_detail_lighting {
        return 0.7;
    }
    match preference {
        1 => 0.9,
        2 => 0.8,
        4 => 0.6,
        // 3 is the default; the setters clamp the preference to 1..=4.
        _ => 0.7,
    }
}

/// `Inv.formatObjAmount`.
pub fn format_amount(amount: i32) -> String {
    if amount < 100000 {
        format!("<col=ffff00>{amount}</col>")
    } else if amount >= 10000000 {
        format!("<col=00ff80>{}M</col>", amount / 1000000)
    } else {
        format!("<col=ffffff>{}K</col>", amount / 1000)
    }
}

/// Whether `renderObjectSprite` draws the count: the caller asked for text,
/// and the item (after its count variant is chosen) is stackable or the
/// count is not one; a count of -1 never shows.
pub fn shows_count(draw_text: bool, stackable: i32, count: i32) -> bool {
    draw_text && (stackable == 1 || count != 1) && count != -1
}

#[derive(Clone, Debug)]
pub struct Glyph {
    pub offset: [i32; 2],
    pub size: [i32; 2],
    /// Row-major; a non-zero byte is a set pixel.
    pub mask: Vec<u8>,
}

/// A `SoftwareFont`: metrics plus one bitmap per character.
#[derive(Clone, Debug)]
pub struct SoftFont {
    pub advances: [i32; 256],
    pub line_height: i32,
    pub kerning: Option<Vec<i8>>,
    pub glyphs: Vec<Glyph>,
}

/// The mutable drawing state `Font.setColors` resets and tags change.
struct Style {
    color: i32,
    color_override: i32,
    shadow: i32,
    shadow_override: i32,
    alpha_override: i32,
    strike: i32,
    underline: i32,
}

impl SoftFont {
    pub fn new(metrics: &Metrics, sheet: &Sheet) -> Result<Self, String> {
        if sheet.frames.len() < 256 {
            return Err("Font has fewer than 256 glyphs".into());
        }
        Ok(Self {
            advances: metrics.advances,
            line_height: metrics.line_height,
            kerning: metrics.kerning.clone(),
            glyphs: sheet
                .frames
                .iter()
                .take(256)
                .map(|f| Glyph {
                    offset: [f.offset[0] as i32, f.offset[1] as i32],
                    size: [f.size[0] as i32, f.size[1] as i32],
                    mask: f.indices.clone(),
                })
                .collect(),
        })
    }

    /// `Font.renderLeft(text, x, y, color, shadow)` onto a software raster
    /// `size[0]` wide whose clip is the whole raster. `shadow` -1 is none.
    /// Name icons (`<img=N>`) are skipped: the item font has none, and the
    /// original swallows the resulting exception.
    #[allow(clippy::too_many_arguments)] // The original call's arguments.
    pub fn render_left(
        &self,
        pixels: &mut [u32],
        size: [i32; 2],
        text: &str,
        mut x: i32,
        y: i32,
        color: i32,
        shadow: i32,
    ) {
        let mut style = Style {
            color,
            color_override: color,
            shadow,
            shadow_override: shadow,
            alpha_override: 256,
            strike: -1,
            underline: -1,
        };
        let base_y = y.wrapping_sub(self.line_height);
        let chars: Vec<char> = text.chars().collect();
        let mut tag_start = None;
        let mut previous = 0usize;
        for (index, &raw) in chars.iter().enumerate() {
            if raw == '<' {
                tag_start = Some(index);
                continue;
            }
            // The original's strings are Latin-1 bytes.
            let mut ch = (u32::from(raw) & 0xff) as usize;
            if raw == '>' {
                if let Some(start) = tag_start.take() {
                    let tag: String = chars[start + 1..index].iter().collect();
                    ch = match tag.as_str() {
                        "lt" => 60,
                        "gt" => 62,
                        "nbsp" => 160,
                        "shy" => 173,
                        "times" => 215,
                        "euro" => 128,
                        "copy" => 169,
                        "reg" => 174,
                        _ => {
                            if !tag.starts_with("img=") {
                                style.tag(&tag);
                            }
                            continue;
                        }
                    };
                }
            }
            if tag_start.is_some() {
                continue;
            }
            if let Some(kerning) = &self.kerning {
                if previous != 0 {
                    x = x.wrapping_add(i32::from(kerning[(previous << 8) + ch]));
                }
            }
            let glyph = &self.glyphs[ch];
            // A space draws nothing; justified spacing does not apply to
            // renderLeft, which leaves the space width at zero.
            if ch != 32 {
                let at = [
                    x.wrapping_add(glyph.offset[0]),
                    base_y.wrapping_add(glyph.offset[1]),
                ];
                if style.shadow_override != -1 {
                    blit(
                        pixels,
                        size,
                        glyph,
                        [at[0].wrapping_add(1), at[1].wrapping_add(1)],
                        style.shadow_override,
                        style.alpha_override,
                    );
                }
                blit(
                    pixels,
                    size,
                    glyph,
                    at,
                    style.color_override,
                    style.alpha_override,
                );
            }
            let advance = self.advances[ch];
            if style.strike != -1 {
                let row = base_y.wrapping_add((f64::from(self.line_height) * 0.7) as i32);
                horizontal_line(pixels, size, x, row, advance, style.strike);
            }
            if style.underline != -1 {
                let row = base_y.wrapping_add(self.line_height).wrapping_add(1);
                horizontal_line(pixels, size, x, row, advance, style.underline);
            }
            x = x.wrapping_add(advance);
            previous = ch;
        }
    }
}

impl Style {
    /// `Font.parseTag`: a number that fails to parse leaves the value alone.
    fn tag(&mut self, tag: &str) {
        if let Some(value) = tag.strip_prefix("col=") {
            if let Some(color) = parse_radix(value, 16) {
                self.color_override = color;
            }
        } else if tag == "/col" {
            self.color_override = self.color;
        } else if let Some(value) = tag.strip_prefix("trans=") {
            if let Some(alpha) = parse_radix(value, 10) {
                self.alpha_override = alpha;
            }
        } else if tag == "/trans" {
            self.alpha_override = 256;
        } else if let Some(value) = tag.strip_prefix("str=") {
            if let Some(color) = parse_radix(value, 16) {
                self.strike = color;
            }
        } else if tag == "str" {
            self.strike = 0x800000;
        } else if tag == "/str" {
            self.strike = -1;
        } else if let Some(value) = tag.strip_prefix("u=") {
            if let Some(color) = parse_radix(value, 16) {
                self.underline = color;
            }
        } else if tag == "u" {
            self.underline = 0;
        } else if tag == "/u" {
            self.underline = -1;
        } else if let Some(value) = tag.strip_prefix("shad=") {
            if let Some(color) = parse_radix(value, 16) {
                self.shadow_override = color;
            }
        } else if tag == "shad" {
            self.shadow_override = 0;
        } else if tag == "/shad" {
            self.shadow_override = self.shadow;
        } else if tag == "br" {
            self.color_override = self.color;
            self.shadow_override = self.shadow;
            self.alpha_override = 256;
            self.strike = -1;
            self.underline = -1;
        }
    }
}

/// `SoftwareFont.renderGlyph` (or `renderGlyphTransparent` when `alpha` is
/// not 256) with the clip at the raster's edges.
fn blit(pixels: &mut [u32], size: [i32; 2], glyph: &Glyph, at: [i32; 2], color: i32, alpha: i32) {
    let [mut x, mut y] = at;
    let [mut width, mut height] = glyph.size;
    let stride = glyph.size[0];
    let (mut source_x, mut source_y) = (0, 0);
    if y < 0 {
        source_y = -y;
        height -= source_y;
        y = 0;
    }
    if y.wrapping_add(height) > size[1] {
        height -= y.wrapping_add(height) - size[1];
    }
    if x < 0 {
        source_x = -x;
        width -= source_x;
        x = 0;
    }
    if x.wrapping_add(width) > size[0] {
        width -= x.wrapping_add(width) - size[0];
    }
    if width <= 0 || height <= 0 {
        return;
    }
    let color = color as u32;
    let source_color = ((color & 0xff00ff).wrapping_mul(alpha as u32) & 0xff00ff00)
        .wrapping_add((color & 0xff00).wrapping_mul(alpha as u32) & 0xff0000)
        >> 8;
    let inverse = 256i32.wrapping_sub(alpha) as u32;
    for row in 0..height {
        for column in 0..width {
            let source = ((source_y + row) * stride + source_x + column) as usize;
            if glyph.mask.get(source).is_none_or(|&bit| bit == 0) {
                continue;
            }
            let Some(target) = pixels.get_mut(((y + row) * size[0] + x + column) as usize) else {
                continue;
            };
            *target = if alpha == 256 {
                color
            } else {
                (((*target & 0xff00ff).wrapping_mul(inverse) & 0xff00ff00)
                    .wrapping_add((*target & 0xff00).wrapping_mul(inverse) & 0xff0000)
                    >> 8)
                    .wrapping_add(source_color)
            };
        }
    }
}

/// `SoftwareRaster.drawHorizontalLine` with the clip at the raster's edges.
fn horizontal_line(
    pixels: &mut [u32],
    size: [i32; 2],
    mut x: i32,
    y: i32,
    mut length: i32,
    color: i32,
) {
    if y < 0 || y >= size[1] {
        return;
    }
    if x < 0 {
        length = length.wrapping_add(x);
        x = 0;
    }
    if x.wrapping_add(length) > size[0] {
        length = size[0] - x;
    }
    for i in 0..length.max(0) {
        if let Some(target) = pixels.get_mut((y * size[0] + x + i) as usize) {
            *target = color as u32;
        }
    }
}

/// The count as `renderObjectSprite` adds it to a finished 36 by 32 sprite:
/// `font.renderLeft(formatObjAmount(count), 0, 9, 0xFFFF00, 1)`.
pub fn bake(pixels: &mut [u32], font: &SoftFont, draw_text: bool, stackable: i32, count: i32) {
    if shows_count(draw_text, stackable, count) {
        font.render_left(pixels, [36, 32], &format_amount(count), 0, 9, 0xffff00, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests of the brightness rule and the count thresholds;
    // the pixels are compared with the JVM in tests/ui_fidelity.rs.
    #[test]
    fn brightness_follows_the_hd_rule() {
        for preference in 1..=4 {
            assert_eq!(raster_brightness(preference, true), 0.7);
        }
        assert_eq!(raster_brightness(1, false), 0.9);
        assert_eq!(raster_brightness(2, false), 0.8);
        assert_eq!(raster_brightness(3, false), 0.7);
        assert_eq!(raster_brightness(4, false), 0.6);
    }

    #[test]
    fn amounts_use_the_original_bands() {
        assert_eq!(format_amount(99999), "<col=ffff00>99999</col>");
        assert_eq!(format_amount(100000), "<col=ffffff>100K</col>");
        assert_eq!(format_amount(9999999), "<col=ffffff>9999K</col>");
        assert_eq!(format_amount(10000000), "<col=00ff80>10M</col>");
        assert_eq!(format_amount(-5), "<col=ffff00>-5</col>");
        assert!(shows_count(true, 0, 2));
        assert!(shows_count(true, 1, 1));
        assert!(!shows_count(true, 0, 1));
        assert!(!shows_count(true, 1, -1));
        assert!(!shows_count(false, 1, 5));
    }
}
