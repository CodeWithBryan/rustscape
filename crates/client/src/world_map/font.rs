//! `WorldMapFont`: label glyph tables rasterised from a system font.
//!
//! The original asks AWT for "Helvetica" bold and grabs each glyph from an
//! off-screen image. Here [`GlyphSource`] stands in for `java.awt.Font`,
//! `FontMetrics` and `Graphics.drawString`, and [`Typeface`] implements it
//! with `fontdue` over caller-supplied TrueType data. Glyph pixels and metrics
//! therefore differ from any particular JVM; the table layout, the
//! bold/plain/double-strike fallback, the string measurement and the blits are
//! the original's. [`WorldMapFont::from_data`] accepts a table produced
//! elsewhere, such as one dumped from the Java class.
use super::raster::Raster;

/// `WorldMapFont.ALPHABET`.
pub const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!\"\u{a3}$%^&*()-_=+[{]};:'@#~,<.>/?\\| \u{c4}\u{cb}\u{cf}\u{d6}\u{dc}\u{e4}\u{eb}\u{ef}\u{f6}\u{fc}\u{ff}\u{df}\u{c1}\u{c0}\u{c9}\u{c8}\u{cd}\u{cc}\u{d3}\u{d2}\u{da}\u{d9}\u{e1}\u{e0}\u{e9}\u{e8}\u{ed}\u{ec}\u{f3}\u{f2}\u{fa}\u{f9}\u{c2}\u{ca}\u{ce}\u{d4}\u{db}\u{e2}\u{ea}\u{ee}\u{f4}\u{fb}\u{c6}\u{e6}";

/// The original's fixed glyph table size; a font needing more fails there.
const DATA_LIMIT: usize = 100_000;

/// `CHAR_INDEXES[byte]`: the header offset of a character, `=` for unknown ones.
fn char_index(byte: u8) -> usize {
    ALPHABET
        .chars()
        .position(|c| u32::from(c) == u32::from(byte))
        .unwrap_or(74)
        * 9
}

/// What `WorldMapFont` asks of AWT. `bold` selects `Font.BOLD` over `Font.PLAIN`.
pub trait GlyphSource {
    /// `FontMetrics.charWidth`.
    fn char_width(&self, bold: bool, ch: char) -> i32;
    /// `FontMetrics.getMaxAscent`.
    fn max_ascent(&self, bold: bool) -> i32;
    /// `FontMetrics.getMaxDescent`.
    fn max_descent(&self, bold: bool) -> i32;
    /// `FontMetrics.getHeight`.
    fn height(&self, bold: bool) -> i32;
    /// `Graphics.drawString` in white: raise `canvas` (row-major, `width`
    /// columns) to the glyph's intensity with its origin at `(x, baseline)`.
    fn draw(&self, bold: bool, ch: char, x: i32, baseline: i32, canvas: &mut [u8], width: usize);
}

/// A TrueType face pair read by `fontdue`.
pub struct Typeface {
    bold: fontdue::Font,
    plain: Option<fontdue::Font>,
    /// False (the default) thresholds coverage at one half, like AWT text
    /// drawn without anti-aliasing; true keeps the coverage.
    pub antialias: bool,
}
impl Typeface {
    /// `bold` stands in for Helvetica bold. `plain` is used only if the bold
    /// glyphs come out anti-aliased; without it the bold face serves both.
    pub fn new(bold: &[u8], plain: Option<&[u8]>) -> Result<Self, String> {
        let load = |data: &[u8]| {
            fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
                .map_err(|e| format!("world map font: {e}"))
        };
        Ok(Self {
            bold: load(bold)?,
            plain: plain.map(load).transpose()?,
            antialias: false,
        })
    }

    /// The glyph source for one pixel size.
    pub fn sized(&self, size: i32) -> SizedTypeface<'_> {
        SizedTypeface {
            face: self,
            size: size as f32,
        }
    }

    fn font(&self, bold: bool) -> &fontdue::Font {
        if bold {
            &self.bold
        } else {
            self.plain.as_ref().unwrap_or(&self.bold)
        }
    }
}

/// [`Typeface`] at one size. Metrics round the way `FontDesignMetrics` does:
/// advances to nearest, ascent, descent and leading up from 0.05.
pub struct SizedTypeface<'a> {
    face: &'a Typeface,
    size: f32,
}
impl SizedTypeface<'_> {
    fn line(&self, bold: bool) -> [i32; 3] {
        let up = |v: f32| (v + 0.95) as i32;
        self.face
            .font(bold)
            .horizontal_line_metrics(self.size)
            .map_or([self.size as i32, 0, 0], |m| {
                [up(m.ascent), up(-m.descent), up(m.line_gap)]
            })
    }
}
impl GlyphSource for SizedTypeface<'_> {
    fn char_width(&self, bold: bool, ch: char) -> i32 {
        (self.face.font(bold).metrics(ch, self.size).advance_width + 0.5) as i32
    }
    fn max_ascent(&self, bold: bool) -> i32 {
        self.line(bold)[0]
    }
    fn max_descent(&self, bold: bool) -> i32 {
        self.line(bold)[1]
    }
    fn height(&self, bold: bool) -> i32 {
        self.line(bold).iter().sum()
    }
    fn draw(&self, bold: bool, ch: char, x: i32, baseline: i32, canvas: &mut [u8], width: usize) {
        let (metrics, coverage) = self.face.font(bold).rasterize(ch, self.size);
        if width == 0 {
            return;
        }
        let height = canvas.len() / width;
        let left = x + metrics.xmin;
        let top = baseline - metrics.ymin - metrics.height as i32;
        for row in 0..metrics.height {
            let cy = top + row as i32;
            if cy < 0 || cy as usize >= height {
                continue;
            }
            for col in 0..metrics.width {
                let cx = left + col as i32;
                if cx < 0 || cx as usize >= width {
                    continue;
                }
                let mut value = coverage[row * metrics.width + col];
                if !self.face.antialias {
                    value = if value >= 128 { 255 } else { 0 };
                }
                let pixel = &mut canvas[cy as usize * width + cx as usize];
                *pixel = (*pixel).max(value);
            }
        }
    }
}

/// `WorldMapFont`: nine header bytes per alphabet glyph (three base-128 data
/// offset digits, width, height, left offset, rise above the baseline,
/// advance, line height) followed by the glyph intensities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldMapFont {
    data: Vec<u8>,
    grayscale: bool,
}
impl WorldMapFont {
    /// `new WorldMapFont(size, bold, component)` over TrueType bytes.
    pub fn new(size: i32, bold: bool, ttf: &[u8]) -> Result<Self, String> {
        Self::from_source(bold, &Typeface::new(ttf, None)?.sized(size))
    }

    /// The constructor proper. As in the original, `bold` is ignored: the
    /// bold face is tried first, the plain face if that came out
    /// anti-aliased, and a double-struck plain face if the plain one did not.
    pub fn from_source(_bold: bool, source: &dyn GlyphSource) -> Result<Self, String> {
        let mut font = Self {
            data: vec![0; ALPHABET.chars().count() * 9],
            grayscale: false,
        };
        let header = font.data.len();
        let pass = |font: &mut Self, bold: bool, double: bool| -> Result<(), String> {
            font.data.truncate(header);
            font.grayscale = false;
            for (index, ch) in ALPHABET.chars().enumerate() {
                font.pre_render_glyph(source, bold, ch, index, double)?;
            }
            Ok(())
        };
        pass(&mut font, true, false)?;
        if font.grayscale {
            pass(&mut font, false, false)?;
            if !font.grayscale {
                pass(&mut font, false, true)?;
            }
        }
        Ok(font)
    }

    /// A finished glyph table, for example one dumped from the Java class.
    pub fn from_data(data: Vec<u8>, grayscale: bool) -> Result<Self, String> {
        if data.len() < ALPHABET.chars().count() * 9 {
            return Err("world map font: table shorter than its glyph headers".into());
        }
        Ok(Self { data, grayscale })
    }

    /// The glyph table.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Whether glyphs are blended by intensity instead of stamped with a shadow.
    pub fn grayscale(&self) -> bool {
        self.grayscale
    }

    fn pre_render_glyph(
        &mut self,
        source: &dyn GlyphSource,
        bold: bool,
        ch: char,
        glyph: usize,
        mut double: bool,
    ) -> Result<(), String> {
        let mut char_width = source.char_width(bold, ch);
        let advance = char_width;
        if double {
            if ch == '/' {
                double = false;
            }
            if "ftwvkxyAVW".contains(ch) {
                char_width += 1;
            }
        }
        let max_ascent = source.max_ascent(bold);
        let total_height = max_ascent + source.max_descent(bold);
        let line_height = source.height(bold);
        if char_width <= 0 || total_height <= 0 {
            return Err(format!("world map font: glyph {ch:?} has no image area"));
        }
        let (w, h) = (char_width as usize, total_height as usize);
        let mut pixels = vec![0u8; w * h];
        source.draw(bold, ch, 0, max_ascent, &mut pixels, w);
        if double {
            source.draw(bold, ch, 1, max_ascent, &mut pixels, w);
        }
        let lit = |col: usize, row: usize| pixels[col + row * w] != 0;
        let min_y = (0..h)
            .find(|&row| (0..w).any(|col| lit(col, row)))
            .unwrap_or(0);
        let min_x = (0..w)
            .find(|&col| (0..h).any(|row| lit(col, row)))
            .unwrap_or(0);
        let max_y = (0..h)
            .rev()
            .find(|&row| (0..w).any(|col| lit(col, row)))
            .map_or(h, |row| row + 1);
        let max_x = (0..w)
            .rev()
            .find(|&col| (0..h).any(|row| lit(col, row)))
            .map_or(w, |col| col + 1);
        let at = self.data.len();
        if at + (max_x - min_x) * (max_y - min_y) > DATA_LIMIT {
            return Err("world map font: glyph table exceeds the original 100000 bytes".into());
        }
        let head = &mut self.data[glyph * 9..glyph * 9 + 9];
        head[0] = (at / 16384) as u8;
        head[1] = ((at / 128) & 0x7f) as u8;
        head[2] = (at & 0x7f) as u8;
        head[3] = (max_x - min_x) as u8;
        head[4] = (max_y - min_y) as u8;
        head[5] = min_x as u8;
        head[6] = (max_ascent - min_y as i32) as u8;
        head[7] = advance as u8;
        head[8] = line_height as u8;
        for row in min_y..max_y {
            for col in min_x..max_x {
                let pixel = pixels[col + row * w];
                if pixel > 30 && pixel < 230 {
                    self.grayscale = true;
                }
                self.data.push(pixel);
            }
        }
        Ok(())
    }

    /// A header field as the signed Java byte it is.
    fn field(&self, index: usize) -> i32 {
        self.data.get(index).map_or(0, |&b| i32::from(b as i8))
    }

    /// `getLineHeight`.
    pub fn get_line_height(&self) -> i32 {
        self.field(8) - 1
    }

    /// `getAscent`: the rise of `A`.
    pub fn get_ascent(&self) -> i32 {
        self.field(6)
    }

    /// `getStringWidth`. `@xxx@` and `~xxx~` sequences are not measured,
    /// although `renderString` draws them.
    pub fn get_string_width(&self, text: &[u8]) -> i32 {
        let mut width = 0i32;
        let mut i = 0;
        while i < text.len() {
            if (text[i] == b'@' && i + 4 < text.len() && text[i + 4] == b'@')
                || (text[i] == b'~' && i + 4 < text.len() && text[i + 4] == b'~')
            {
                i += 4;
            } else {
                width = width.wrapping_add(self.field(char_index(text[i]) + 7));
            }
            i += 1;
        }
        width
    }

    /// `renderStringCenter`: `y` is the baseline.
    pub fn render_string_center(
        &self,
        raster: &mut Raster<'_>,
        text: &[u8],
        x: i32,
        y: i32,
        color: i32,
    ) {
        let half = self.get_string_width(text) / 2;
        let ascent = self.get_ascent();
        if x - half <= raster.clip_right
            && x + half >= raster.clip_left
            && y - ascent <= raster.clip_bottom
            && y >= 0
        {
            self.render_string(raster, text, x - half, y, color, true);
        }
    }

    fn render_string(
        &self,
        raster: &mut Raster<'_>,
        text: &[u8],
        mut x: i32,
        y: i32,
        color: i32,
        mut shadow: bool,
    ) {
        if self.grayscale || color == 0 {
            shadow = false;
        }
        for &ch in text {
            let index = char_index(ch);
            if shadow {
                self.render_glyph(raster, index, x + 1, y, 1);
                self.render_glyph(raster, index, x, y + 1, 1);
            }
            self.render_glyph(raster, index, x, y, color);
            x = x.wrapping_add(self.field(index + 7));
        }
    }

    fn render_glyph(&self, raster: &mut Raster<'_>, index: usize, x: i32, y: i32, color: i32) {
        let mut draw_x = x + self.field(index + 5);
        let mut draw_y = y - self.field(index + 6);
        let mut width = self.field(index + 3);
        let mut height = self.field(index + 4);
        let mut src =
            self.field(index) * 16384 + self.field(index + 1) * 128 + self.field(index + 2);
        let mut dst = draw_x.wrapping_add(draw_y.wrapping_mul(raster.width));
        let mut dst_stride = raster.width - width;
        let mut src_stride = 0;
        if draw_y < raster.clip_top {
            let clip = raster.clip_top - draw_y;
            height -= clip;
            draw_y = raster.clip_top;
            src += clip * width;
            dst = dst.wrapping_add(clip.wrapping_mul(raster.width));
        }
        if draw_y + height >= raster.clip_bottom {
            height -= draw_y + height + 1 - raster.clip_bottom;
        }
        if draw_x < raster.clip_left {
            let clip = raster.clip_left - draw_x;
            width -= clip;
            draw_x = raster.clip_left;
            src += clip;
            dst = dst.wrapping_add(clip);
            src_stride = clip;
            dst_stride += clip;
        }
        if draw_x + width >= raster.clip_right {
            let clip = draw_x + width + 1 - raster.clip_right;
            width -= clip;
            src_stride += clip;
            dst_stride += clip;
        }
        if width <= 0 || height <= 0 {
            return;
        }
        for _ in 0..height {
            for _ in 0..width {
                let intensity = usize::try_from(src)
                    .ok()
                    .and_then(|i| self.data.get(i))
                    .map_or(0, |&b| i32::from(b));
                src += 1;
                if self.grayscale {
                    // renderGlyphGrayscale
                    if intensity >= 230 {
                        raster.put(dst, color);
                    } else if intensity > 30 {
                        let background = raster.get(dst);
                        let mixed = ((color & 0xff00ff)
                            .wrapping_mul(intensity)
                            .wrapping_add((background & 0xff00ff).wrapping_mul(256 - intensity))
                            & 0xff00ff00u32 as i32)
                            .wrapping_add(
                                (color & 0xff00).wrapping_mul(intensity).wrapping_add(
                                    (background & 0xff00).wrapping_mul(256 - intensity),
                                ) & 0xff0000,
                            )
                            >> 8;
                        raster.put(dst, mixed);
                    }
                } else if intensity != 0 {
                    // renderGlyphMono
                    raster.put(dst, color);
                }
                dst = dst.wrapping_add(1);
            }
            dst = dst.wrapping_add(dst_stride);
            src += src_stride;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source whose every glyph is a solid 2x3 block above the baseline.
    struct Blocks(u8);
    impl GlyphSource for Blocks {
        fn char_width(&self, _: bool, ch: char) -> i32 {
            if ch == ' ' { 3 } else { 4 }
        }
        fn max_ascent(&self, _: bool) -> i32 {
            5
        }
        fn max_descent(&self, _: bool) -> i32 {
            2
        }
        fn height(&self, _: bool) -> i32 {
            8
        }
        fn draw(&self, bold: bool, ch: char, x: i32, baseline: i32, canvas: &mut [u8], w: usize) {
            if ch == ' ' {
                return;
            }
            let value = if bold { self.0 } else { 255 };
            for row in baseline - 3..baseline {
                for col in x + 1..x + 3 {
                    if col >= 0 && (col as usize) < w {
                        canvas[row as usize * w + col as usize] = value;
                    }
                }
            }
        }
    }

    #[test]
    fn alphabet_and_unknown_characters_follow_the_original_table() {
        assert_eq!(ALPHABET.chars().count(), 139);
        assert_eq!(char_index(b'A'), 0);
        assert_eq!(char_index(b'='), 74 * 9);
        assert_eq!(char_index(0xa3), 64 * 9);
        assert_eq!(char_index(0xe6), 138 * 9);
        assert_eq!(char_index(1), 74 * 9);
    }

    #[test]
    fn glyph_headers_trim_to_the_inked_box_and_blank_glyphs_keep_the_canvas() {
        let font = WorldMapFont::from_source(true, &Blocks(255)).unwrap();
        assert!(!font.grayscale());
        let head = &font.data()[0..9];
        // 'A' is the first glyph, stored right after the 139 headers.
        assert_eq!(
            head[0] as usize * 16384 + head[1] as usize * 128 + head[2] as usize,
            1251
        );
        assert_eq!(&head[3..], &[2, 3, 1, 3, 4, 8]);
        let space = char_index(b' ');
        assert_eq!(&font.data()[space + 3..space + 9], &[3, 7, 0, 5, 3, 8]);
        assert_eq!(font.get_ascent(), 3);
        assert_eq!(font.get_line_height(), 7);
        assert_eq!(font.get_string_width(b"A A"), 11);
        // Five-character @...@ and ~...~ runs are skipped by the measurement.
        assert_eq!(font.get_string_width(b"@red@A"), 4);
        assert_eq!(font.get_string_width(b"~abc~A"), 4);
        assert_eq!(font.get_string_width(b"@re"), 12);
    }

    #[test]
    fn antialiased_bold_falls_back_to_double_struck_plain() {
        let font = WorldMapFont::from_source(true, &Blocks(128)).unwrap();
        // Plain is crisp, so the third pass strikes each glyph twice: 3 wide.
        assert!(!font.grayscale());
        assert_eq!(&font.data()[3..6], &[3, 3, 1]);
        // '/' is never double struck; 'A' gets one more column of canvas.
        let slash = char_index(b'/');
        assert_eq!(font.data()[slash + 3], 2);
        assert_eq!(font.data()[7], 4);
    }

    #[test]
    fn mono_text_is_stamped_with_a_one_pixel_shadow_and_clipped() {
        let font = WorldMapFont::from_source(true, &Blocks(255)).unwrap();
        let mut pixels = vec![0u32; 12 * 8];
        let mut raster = Raster::new(&mut pixels, 12, 8).unwrap();
        font.render_string_center(&mut raster, b"AA", 6, 5, 0xffffff);
        let row = |y: usize| pixels[y * 12..y * 12 + 12].to_vec();
        let w = 0xffffff;
        assert_eq!(row(1), [0; 12]);
        assert_eq!(row(2), [0, 0, 0, w, w, 1, 0, w, w, 1, 0, 0]);
        assert_eq!(row(4), [0, 0, 0, w, w, 1, 0, w, w, 1, 0, 0]);
        assert_eq!(row(5), [0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0]);
        // The right and bottom clip edges drop one extra column and row.
        let mut pixels = vec![0u32; 12 * 8];
        let mut raster = Raster::new(&mut pixels, 12, 8).unwrap();
        raster.set_clip(0, 0, 9, 5);
        font.render_string_center(&mut raster, b"AA", 6, 5, 0xffffff);
        assert_eq!(pixels[2 * 12..3 * 12], [0, 0, 0, w, w, 1, 0, w, 0, 0, 0, 0]);
        assert_eq!(pixels[4 * 12..5 * 12], [0; 12]);
    }

    #[test]
    fn grayscale_text_blends_and_has_no_shadow() {
        let data = {
            let mut data = vec![0u8; 139 * 9];
            let at = data.len();
            data[0] = (at / 16384) as u8;
            data[1] = ((at / 128) & 0x7f) as u8;
            data[2] = (at & 0x7f) as u8;
            data[3..9].copy_from_slice(&[3, 1, 0, 1, 3, 9]);
            data.extend([20, 128, 240]);
            data
        };
        let font = WorldMapFont::from_data(data, true).unwrap();
        let mut pixels = vec![0x204060u32; 5 * 3];
        let mut raster = Raster::new(&mut pixels, 5, 3).unwrap();
        font.render_string_center(&mut raster, b"A", 2, 2, 0xff8000);
        // Intensity 20 is skipped, 128 mixes each channel half and half and
        // 240 is solid. The red sum 0x8f80 sets the Java int's sign bit, and
        // the arithmetic `>> 8` then fills the unused top byte with ones.
        assert_eq!(
            pixels[5..10],
            [0x204060, 0x204060, 0xff8f6030, 0xff8000, 0x204060]
        );
        assert!(
            pixels[..5]
                .iter()
                .chain(&pixels[10..])
                .all(|&p| p == 0x204060)
        );
    }
}
