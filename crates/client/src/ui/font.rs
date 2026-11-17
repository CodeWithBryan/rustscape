//! Bitmap font metrics from the original font archive.
use rs09_protocol::{Error, Result, buffer::Reader};
#[derive(Clone, Debug)]
pub struct Metrics {
    pub advances: [i32; 256],
    pub line_height: i32,
    pub kerning: Option<Vec<i8>>,
    pub name_icons: Vec<i32>,
}
#[derive(Clone, Debug)]
pub struct Line {
    pub text: String,
    pub position: [i32; 2],
    pub spacing: i32,
}
impl Metrics {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut advances = [0; 256];
        for v in &mut advances {
            *v = r.g1()?;
        }
        if data.len() == 257 {
            return Ok(Self {
                advances,
                line_height: r.g1()?,
                kerning: None,
                name_icons: Vec::new(),
            });
        }
        let mut heights = [0usize; 256];
        let mut tops = [0usize; 256];
        for v in &mut heights {
            *v = r.g1()? as usize;
        }
        for v in &mut tops {
            *v = r.g1()? as usize;
        }
        let mut profiles = Vec::new();
        for _ in 0..2 {
            let mut glyphs = Vec::new();
            for &height in &heights {
                let mut acc = 0i8;
                let mut glyph = Vec::new();
                for _ in 0..height {
                    acc = acc.wrapping_add(r.g1b()? as i8);
                    glyph.push(acc);
                }
                glyphs.push(glyph);
            }
            profiles.push(glyphs);
        }
        let mut kerning = vec![0; 65536];
        for left in 0..256 {
            if left == 32 || left == 160 {
                continue;
            }
            for right in 0..256 {
                if right == 32 || right == 160 {
                    continue;
                }
                let top = tops[left].max(tops[right]);
                let bottom = (tops[left] + heights[left]).min(tops[right] + heights[right]);
                let mut min = advances[left].min(advances[right]);
                for y in top..bottom {
                    // Font.decode passes its two tables to calculateKerning in
                    // an order that pairs the second one with the left glyph.
                    let sum = i32::from(profiles[1][left][y - tops[left]])
                        + i32::from(profiles[0][right][y - tops[right]]);
                    min = min.min(sum);
                }
                kerning[left * 256 + right] = (-min) as i8;
            }
        }
        Ok(Self {
            advances,
            line_height: (tops[32] + heights[32]) as i32,
            kerning: Some(kerning),
            name_icons: Vec::new(),
        })
    }
    pub fn width(&self, text: &str) -> i32 {
        let mut n = 0;
        let mut last = 0;
        for (raw, c, _) in tokens(text) {
            if let Some(width) = self.icon_width(&raw) {
                n += width;
                last = 0;
                continue;
            }
            let Some(c) = c else {
                continue;
            };
            let c = c as usize;
            if let Some(k) = &self.kerning {
                if last != 0 {
                    n += i32::from(k[last * 256 + c]);
                }
            }
            n += self.advances[c];
            last = c;
        }
        n
    }
    fn icon_width(&self, tag: &str) -> Option<i32> {
        let index = tag
            .strip_prefix("<img=")?
            .strip_suffix('>')?
            .parse::<usize>()
            .ok()?;
        self.name_icons.get(index).copied()
    }
    /// Original paragraph breaks and optional inline name-icon advances.
    pub fn split(&self, text: &str, widths: Option<&[i32]>) -> Vec<String> {
        let mut lines = Vec::new();
        let mut buffer = String::new();
        let (mut width, mut start, mut break_pos, mut break_width, mut trim, mut previous) =
            (0, 0, None, 0, 0, 0);
        for (raw, glyph, tag) in tokens(text) {
            buffer.push_str(&raw);
            if tag && raw == "<br>" {
                lines.push(buffer[start..].into());
                start = buffer.len();
                width = 0;
                break_pos = None;
                previous = 0;
            }
            if let Some(ch) = glyph {
                width += self.advances[ch as usize];
                if previous != 0 {
                    if let Some(k) = &self.kerning {
                        width += i32::from(k[previous * 256 + ch as usize]);
                    }
                }
                previous = ch as usize;
            } else if let Some(icon_width) = self.icon_width(&raw) {
                width += icon_width;
                previous = 0;
            }
            let literal = if tag { None } else { glyph };
            if literal == Some(32) {
                break_pos = Some(buffer.len());
                break_width = width;
                trim = 1;
            }
            if let (Some(widths), Some(pos)) = (widths, break_pos) {
                if !widths.is_empty() && width > widths[lines.len().min(widths.len() - 1)] {
                    lines.push(buffer[start..pos - trim].into());
                    start = pos;
                    break_pos = None;
                    width -= break_width;
                    previous = 0;
                }
            }
            if literal == Some(45) {
                break_pos = Some(buffer.len());
                break_width = width;
                trim = 0;
            }
        }
        if buffer.len() > start {
            lines.push(buffer[start..].into());
        }
        lines
    }
    pub fn paragraph(
        &self,
        text: &str,
        bounds: [i32; 4],
        align: [i32; 2],
        line_spacing: i32,
        padding: [i32; 2],
    ) -> Vec<Line> {
        let [x, y, w, h] = bounds;
        let [top, bottom] = padding;
        let mut step = if line_spacing == 0 {
            self.line_height
        } else {
            line_spacing
        };
        let widths = [w];
        let wrap = !(h < top + bottom + step && h < step + step);
        let lines = self.split(text, wrap.then_some(&widths[..]));
        let count = lines.len() as i32;
        let valign = if align[1] == 3 && count == 1 {
            1
        } else {
            align[1]
        };
        let space = (count - 1) * step;
        let baseline = match valign {
            0 => y + top,
            1 => y + top + (h - top - bottom - space) / 2,
            2 => y + h - bottom - space,
            _ => {
                let extra = ((h - top - bottom - space) / (count + 1)).max(0);
                step += extra;
                y + top + extra
            }
        };
        lines
            .into_iter()
            .enumerate()
            .map(|(i, text)| {
                let width = self.width(&text);
                let x = match align[0] {
                    1 => x + (w - width) / 2,
                    2 => x + w - width,
                    _ => x,
                };
                let mut spacing = 0;
                if align[0] >= 3 && i + 1 < count as usize {
                    let spaces = tokens(&text)
                        .iter()
                        .filter(|(_, c, tag)| !*tag && *c == Some(32))
                        .count() as i32;
                    if spaces > 0 {
                        spacing = ((w - width) << 8) / spaces;
                    }
                }
                Line {
                    text,
                    position: [x, baseline + i as i32 * step],
                    spacing,
                }
            })
            .collect()
    }
}
pub fn tokens(text: &str) -> Vec<(String, Option<u8>, bool)> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut start = None;
    let mut out = Vec::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == '<' {
            start = Some(i);
        } else if c == '>' && start.is_some() {
            let tag = chars[start.take().unwrap() + 1..i]
                .iter()
                .collect::<String>();
            let glyph = match tag.as_str() {
                "lt" => Some(60),
                "gt" => Some(62),
                "nbsp" => Some(160),
                "shy" => Some(173),
                "times" => Some(215),
                "euro" => Some(128),
                "copy" => Some(169),
                "reg" => Some(174),
                _ => None,
            };
            out.push((format!("<{tag}>"), glyph, true));
        } else if start.is_none() {
            out.push((c.to_string(), Some(c as u8), false));
        }
    }
    out
}
/// Formatting tags carry no advance. The renderer consumes color tags separately.
pub fn glyphs(text: &str) -> Vec<char> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for c in chars.by_ref() {
                if c == '>' {
                    break;
                }
                tag.push(c);
            }
            if let Some(c) = match tag.as_str() {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "nbsp" => Some('\u{a0}'),
                "shy" => Some('\u{ad}'),
                "times" => Some('\u{d7}'),
                "euro" => Some('\u{80}'),
                "copy" => Some('\u{a9}'),
                "reg" => Some('\u{ae}'),
                "br" => Some('\n'),
                _ => None,
            } {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
}
pub fn simple_metrics(data: &[u8]) -> Result<Metrics> {
    if data.len() != 257 {
        return Err(Error::new(0, "font needs kerning metrics"));
    }
    Metrics::decode(data)
}
