//! The text labels `MiniMap.render` draws over the map: which label list a
//! scene uses, where each label lands and every glyph `Font` draws for it.
//!
//! The list is `LoginManager.mapElementList`: the `<group>_labels` files of
//! the world map that `MapList.getContainingSource` finds at the scene centre.
//! `MapList`, `Map` and `MapElementList` are the world map module's.
use super::View;
use crate::{
    animation::sin_cos,
    pack::Pack,
    ui::{
        font::{Metrics, tokens},
        sprite::Sheet,
    },
    world_map::{Map, MapElementList, MapList, latin1},
};

/// One of `Fonts.p11Full`, `p12Full` and `b12Full` as `Font` holds it.
#[derive(Clone, Debug)]
pub struct LabelFont {
    pub metrics: Metrics,
    /// `spriteXOffsets`, `spriteYOffsets` of the 256 glyphs.
    pub offsets: Vec<[i32; 2]>,
    /// `spriteInnerWidths`, `spriteInnerHeights`.
    pub sizes: Vec<[i32; 2]>,
    /// `paragraphTopPadding`, `paragraphBottomPadding`.
    pub padding: [i32; 2],
}
impl LabelFont {
    /// The `Font` constructor: the paddings come from the highest glyph that
    /// has a height and the lowest glyph bottom of all 256.
    pub fn new(
        metrics: Metrics,
        offsets: Vec<[i32; 2]>,
        sizes: Vec<[i32; 2]>,
    ) -> Result<Self, String> {
        if offsets.len() < 256 || sizes.len() < 256 {
            return Err("Font has fewer than 256 glyphs".into());
        }
        let (mut min_top, mut max_bottom) = (i32::MAX, i32::MIN);
        for i in 0..256 {
            if offsets[i][1] < min_top && sizes[i][1] != 0 {
                min_top = offsets[i][1];
            }
            max_bottom = max_bottom.max(offsets[i][1].wrapping_add(sizes[i][1]));
        }
        let padding = [
            metrics.line_height.wrapping_sub(min_top),
            max_bottom.wrapping_sub(metrics.line_height),
        ];
        Ok(Self {
            metrics,
            offsets,
            sizes,
            padding,
        })
    }

    /// `Font.load`: metrics from the font archive, glyph boxes from the sprite group.
    pub fn decode(font: &[u8], sprite: &[u8]) -> Result<(Self, Sheet), String> {
        let metrics = Metrics::decode(font).map_err(|e| e.to_string())?;
        let sheet = Sheet::decode(sprite).map_err(|e| e.to_string())?;
        let offsets = sheet
            .frames
            .iter()
            .map(|f| f.offset.map(|v| v as i32))
            .collect();
        let sizes = sheet
            .frames
            .iter()
            .map(|f| f.size.map(|v| v as i32))
            .collect();
        Ok((Self::new(metrics, offsets, sizes)?, sheet))
    }

    /// `Font.getMaxLineWidth`.
    pub fn max_line_width(&self, text: &str, width: i32) -> i32 {
        self.metrics
            .split(text, Some(&[width]))
            .iter()
            .map(|line| self.metrics.width(line))
            .fold(0, i32::max)
    }
}

/// One raster call of `Font.render`, relative to the minimap component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    /// `renderGlyph`, or `renderGlyphTransparent` when `alpha` is not 256.
    Glyph {
        /// Index into the three label fonts.
        font: usize,
        glyph: u8,
        at: [i32; 2],
        size: [i32; 2],
        color: i32,
        alpha: i32,
    },
    /// `drawHorizontalLine(x, y, width, color)` of a strike-through or underline.
    Line {
        at: [i32; 2],
        width: i32,
        color: i32,
    },
}

/// `JagString.parseHexString(radix)`; None where it throws.
fn parse_number(text: &str, radix: i32) -> Option<i32> {
    let (mut negative, mut seen, mut result) = (false, false, 0i32);
    for (i, c) in text.chars().enumerate() {
        let mut digit = c as i32 & 0xff;
        if i == 0 {
            if digit == 45 {
                negative = true;
                continue;
            }
            if digit == 43 {
                continue;
            }
        }
        digit = match digit {
            48..=57 => digit - 48,
            65..=90 => digit - 55,
            97..=122 => digit - 87,
            _ => return None,
        };
        if radix <= digit {
            return None;
        }
        if negative {
            digit = -digit;
        }
        let next = digit.wrapping_add(result.wrapping_mul(radix));
        if next.wrapping_div(radix) != result {
            return None;
        }
        result = next;
        seen = true;
    }
    seen.then_some(result)
}

/// The static colour state `Font.setColors` and `Font.parseTag` keep.
struct Style {
    color: i32,
    shadow: i32,
    alpha: i32,
    color_override: i32,
    shadow_override: i32,
    alpha_override: i32,
    strike: i32,
    underline: i32,
}
impl Style {
    fn new(color: i32, shadow: i32, alpha: i32) -> Self {
        Self {
            color,
            shadow,
            alpha,
            color_override: color,
            shadow_override: shadow,
            alpha_override: alpha,
            strike: -1,
            underline: -1,
        }
    }
    /// `Font.parseTag`; a number that does not parse leaves the state alone.
    fn tag(&mut self, tag: &str) {
        if let Some(value) = tag.strip_prefix("col=") {
            if let Some(v) = parse_number(value, 16) {
                self.color_override = v;
            }
        } else if tag == "/col" {
            self.color_override = self.color;
        } else if let Some(value) = tag.strip_prefix("trans=") {
            if let Some(v) = parse_number(value, 10) {
                self.alpha_override = v;
            }
        } else if tag == "/trans" {
            self.alpha_override = self.alpha;
        } else if let Some(value) = tag.strip_prefix("str=") {
            if let Some(v) = parse_number(value, 16) {
                self.strike = v;
            }
        } else if tag == "str" {
            self.strike = 0x800000;
        } else if tag == "/str" {
            self.strike = -1;
        } else if let Some(value) = tag.strip_prefix("u=") {
            if let Some(v) = parse_number(value, 16) {
                self.underline = v;
            }
        } else if tag == "u" {
            self.underline = 0;
        } else if tag == "/u" {
            self.underline = -1;
        } else if let Some(value) = tag.strip_prefix("shad=") {
            if let Some(v) = parse_number(value, 16) {
                self.shadow_override = v;
            }
        } else if tag == "shad" {
            self.shadow_override = 0;
        } else if tag == "/shad" {
            self.shadow_override = self.shadow;
        } else if tag == "br" {
            *self = Self::new(self.color, self.shadow, self.alpha);
        }
    }
}

/// `Font.render` of one line with no justification. `<img=N>` draws nothing:
/// the label fonts here carry no name icons.
fn render(
    index: usize,
    font: &LabelFont,
    text: &str,
    mut x: i32,
    y: i32,
    style: &mut Style,
    out: &mut Vec<Draw>,
) {
    let base = y.wrapping_sub(font.metrics.line_height);
    let mut previous = 0usize;
    for (raw, glyph, tag) in tokens(text) {
        if tag && glyph.is_none() {
            let name = &raw[1..raw.len() - 1];
            if !name.starts_with("img=") {
                style.tag(name);
            }
            continue;
        }
        let Some(glyph) = glyph else {
            continue;
        };
        let ch = usize::from(glyph);
        if previous != 0
            && let Some(kerning) = &font.metrics.kerning
        {
            x = x.wrapping_add(i32::from(kerning[previous * 256 + ch]));
        }
        if ch != 32 {
            let [ox, oy] = font.offsets[ch];
            let mut put = |dx: i32, color: i32| {
                out.push(Draw::Glyph {
                    font: index,
                    glyph,
                    at: [
                        x.wrapping_add(ox).wrapping_add(dx),
                        base.wrapping_add(oy).wrapping_add(dx),
                    ],
                    size: font.sizes[ch],
                    color,
                    alpha: style.alpha_override,
                });
            };
            if style.shadow_override != -1 {
                put(1, style.shadow_override);
            }
            put(0, style.color_override);
        }
        let advance = font.metrics.advances[ch];
        if style.strike != -1 {
            out.push(Draw::Line {
                at: [
                    x,
                    base.wrapping_add((f64::from(font.metrics.line_height) * 0.7) as i32),
                ],
                width: advance,
                color: style.strike,
            });
        }
        if style.underline != -1 {
            out.push(Draw::Line {
                at: [
                    x,
                    base.wrapping_add(font.metrics.line_height).wrapping_add(1),
                ],
                width: advance,
                color: style.underline,
            });
        }
        x = x.wrapping_add(advance);
        previous = ch;
    }
}

/// The label loop of `MiniMap.render`, in list order.
///
/// `fonts` are p11, p12 and b12; `size` the component's width and height;
/// `origin` is `Camera.originX/Y` and `fine` the local player's scene-local
/// fine position. Coordinates in the result have the component's top left at
/// the origin. Labels are clipped by the component and its mask when drawn.
pub fn layout(
    list: &MapElementList,
    fonts: &[LabelFont; 3],
    view: View,
    size: [i32; 2],
    origin: [i32; 2],
    fine: [i32; 2],
) -> Vec<Draw> {
    let mut out = Vec::new();
    let zoom = view.zoom_offset.wrapping_add(256);
    if zoom == 0 {
        // The original divides by zero here; its zoom offset stays in -21..=11.
        return out;
    }
    let (sin, cos) = sin_cos(view.angle());
    let sin = sin.wrapping_mul(256).wrapping_div(zoom);
    let cos = cos.wrapping_mul(256).wrapping_div(zoom);
    for k in 0..list.count() {
        if !list.is_minimap_label_visible(k) {
            continue;
        }
        let i = k as usize;
        let delta = |coordinate: i16, axis: usize| {
            (i32::from(coordinate).wrapping_sub(origin[axis]))
                .wrapping_mul(4)
                .wrapping_add(2)
                .wrapping_sub(fine[axis] / 32)
        };
        let dx = delta(list.coord_x[i], 0);
        let dy = delta(list.coord_y[i], 1);
        let screen_dy = dy.wrapping_mul(cos).wrapping_sub(dx.wrapping_mul(sin)) >> 16;
        let index = match list.get_label_size(k) {
            1 => 1,
            2 => 2,
            _ => 0,
        };
        let font = &fonts[index];
        let screen_dx = sin.wrapping_mul(dy).wrapping_add(cos.wrapping_mul(dx)) >> 16;
        let text = latin1(&list.names[i]);
        let width = font.max_line_width(&text, 100);
        let x = screen_dx.wrapping_sub(width / 2);
        if x < -size[0] || x > size[0] || screen_dy < -size[1] || screen_dy > size[1] {
            continue;
        }
        let color = if list.colors[i] != -1 {
            list.colors[i]
        } else {
            0xffffff
        };
        // renderParagraphAlpha(name, x, y, width, 50, colour, shadow 0,
        // centred, top aligned, the font's own line spacing).
        let mut style = Style::new(color, 0, 256);
        for line in font.metrics.paragraph(
            &text,
            [
                x.wrapping_add(size[0] / 2),
                (size[1] / 2).wrapping_sub(screen_dy),
                width,
                50,
            ],
            [1, 0],
            0,
            font.padding,
        ) {
            render(
                index,
                font,
                &line.text,
                line.position[0],
                line.position[1],
                &mut style,
                &mut out,
            );
        }
    }
    out
}

/// The label lists of every world map, chosen as `LoginManager.loadRegion` does.
#[derive(Clone, Debug, Default)]
pub struct Labels {
    maps: MapList,
    /// By map; None where the map has no `<group>_labels` group.
    lists: Vec<Option<MapElementList>>,
}
impl Labels {
    /// `group_files` returns the files of a named archive 23 group in
    /// ascending order, or None if there is no such group.
    pub fn load(group_files: impl Fn(&str) -> Option<Vec<Vec<u8>>>) -> Result<Self, String> {
        let details = group_files("details").ok_or("world map details are missing")?;
        let maps = MapList::decode(details.iter().map(Vec::as_slice), Vec::new())?;
        let lists = maps
            .maps
            .iter()
            .map(|map| {
                group_files(&format!("{}_labels", map.group))
                    .map(|files| MapElementList::decode(files.iter().map(Vec::as_slice)))
                    .transpose()
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { maps, lists })
    }

    /// The same files as pack entries: `worldmap/details/<n>` and, for map
    /// `m` of the details, `worldmap/labels/<m>/<n>`.
    pub fn pack_entries(
        group_files: impl Fn(&str) -> Option<Vec<Vec<u8>>>,
    ) -> Result<Vec<(String, Vec<u8>)>, String> {
        let details = group_files("details").ok_or("world map details are missing")?;
        let maps = MapList::decode(details.iter().map(Vec::as_slice), Vec::new())?;
        let mut entries = Vec::new();
        for (m, map) in maps.maps.iter().enumerate() {
            for (n, file) in group_files(&format!("{}_labels", map.group))
                .into_iter()
                .flatten()
                .enumerate()
            {
                entries.push((format!("worldmap/labels/{m}/{n}"), file));
            }
        }
        entries.extend(
            details
                .into_iter()
                .enumerate()
                .map(|(n, file)| (format!("worldmap/details/{n}"), file)),
        );
        Ok(entries)
    }

    /// Reads the entries `pack_entries` wrote; None if the pack has none.
    pub fn from_pack(pack: &Pack) -> Result<Option<Self>, String> {
        fn files<'a>(pack: &'a Pack, prefix: &str) -> Vec<&'a [u8]> {
            (0..)
                .map_while(|n| pack.get(&format!("{prefix}/{n}")).ok())
                .collect()
        }
        let details = files(pack, "worldmap/details");
        if details.is_empty() {
            return Ok(None);
        }
        let maps = MapList::decode(details, Vec::new())?;
        let lists = (0..maps.maps.len())
            .map(|m| {
                let labels = files(pack, &format!("worldmap/labels/{m}"));
                (!labels.is_empty())
                    .then(|| MapElementList::decode(labels))
                    .transpose()
            })
            .collect::<Result<_, _>>()?;
        Ok(Some(Self { maps, lists }))
    }

    /// Straight from archive 23 of a cache directory.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_cache(directory: &std::path::Path) -> Result<Self, String> {
        let archive = crate::cache_pack::Source::open(directory)?.archive(23)?;
        Self::load(cache_group(&archive))
    }

    /// A diagnostic with no original counterpart: marks every label for the
    /// minimap. The pinned cache marks none, so nothing else shows the text.
    pub fn show_all(&mut self) {
        for list in self.lists.iter_mut().flatten() {
            for flags in &mut list.flags {
                *flags |= 4;
            }
        }
    }

    /// Every map of the details with its label list, in file order.
    pub fn lists(&self) -> impl Iterator<Item = (&Map, Option<&MapElementList>)> {
        self.maps
            .maps
            .iter()
            .zip(self.lists.iter().map(Option::as_ref))
    }

    /// `LoginManager.mapElementList` for a scene whose origin is `origin`:
    /// the first valid map containing the centre tile, 48 tiles in. None
    /// stands for the original's empty list.
    pub fn for_origin(&self, origin: [i32; 2]) -> Option<&MapElementList> {
        let (x, y) = (origin[0].wrapping_add(48), origin[1].wrapping_add(48));
        let index = self
            .maps
            .maps
            .iter()
            .position(|map| map.valid && map.contains_display_coordinate(y, x))?;
        self.lists.get(index)?.as_ref()
    }
}

/// The files of a named group of an opened archive 23.
pub fn cache_group(
    archive: &rs09_cache::archive::Archive,
) -> impl Fn(&str) -> Option<Vec<Vec<u8>>> + '_ {
    move |group| {
        let id = archive.index.group_by_ascii_name(group)?;
        let files = archive.group(id, [0; 4]).ok().flatten()?;
        Some(files.iter().map(|(_, data)| data.to_vec()).collect())
    }
}
