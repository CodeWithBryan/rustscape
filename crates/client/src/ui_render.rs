//! GPU presentation of cached interface components and bitmap fonts.
use crate::display_host::{mouse_position, screen_height};
use macroquad::prelude::*;
use rs09_client::{
    pack::Pack,
    ui::{
        font::{Metrics, tokens},
        sprite::Sheet,
        state::{DrawNode, State},
    },
};
use std::collections::{BTreeMap, VecDeque};
#[path = "status_box.rs"]
mod status_box;
#[path = "title_screen.rs"]
mod title_screen;

fn legacy_geometry(geometry: rs09_client::ui::widget_legacy::Geometry, color: Color) {
    if geometry.vertices.is_empty() {
        return;
    }
    let height = screen_height() as i32 as f32;
    let vertices: Vec<_> = geometry
        .vertices
        .iter()
        .map(|&[x, y]| Vertex::new(x, height - y, 0.0, 0.0, 0.0, color))
        .collect();
    let indices: &[u16] = if geometry.thin {
        if vertices.len() == 2 {
            &[0, 1, 1, 0]
        } else {
            &[0, 1, 1, 2, 2, 3, 3, 0]
        }
    } else {
        &[0, 1, 2, 0, 2, 3]
    };
    gl_use_default_material();
    unsafe {
        let gl = get_internal_gl();
        gl.quad_gl.texture(None);
        gl.quad_gl.draw_mode(if geometry.thin {
            DrawMode::Lines
        } else {
            DrawMode::Triangles
        });
        gl.quad_gl.geometry(&vertices, indices);
    }
}
/// GlRaster.drawRect and drawRectAlpha: a one-pixel outline. macroquad's
/// `draw_rectangle_lines` insets by half the thickness, which at thickness 1
/// leaves bands half a pixel wide, and this window is multisampled, so a GL
/// line loop also lands at about half strength. Whole-pixel strips cover
/// exactly the pixels the original's loop lights.
pub(crate) fn legacy_border(rect: [i32; 4], color: Color) {
    for [x, y, w, h] in rs09_client::ui::outline::strips(rect) {
        draw_rectangle(x as f32, y as f32, w as f32, h as f32, color);
    }
}
struct TextStyle {
    base: Color,
    tint: Color,
    base_shadow: Option<Color>,
    shadow: Option<Color>,
    underline: Option<Color>,
    strike: Option<Color>,
    spacing: i32,
    fraction: i32,
}
impl TextStyle {
    fn new(color: Color, shadow: bool) -> Self {
        let shade = shadow.then_some(Color::new(0.0, 0.0, 0.0, color.a));
        Self {
            base: color,
            tint: color,
            base_shadow: shade,
            shadow: shade,
            underline: None,
            strike: None,
            spacing: 0,
            fraction: 0,
        }
    }
    fn tag(&mut self, tag: &str) {
        // Font.parseTag: a number that does not parse throws, and the catch
        // leaves the previous value; -1 is the original's "none".
        let hex = |s: &str| {
            rs09_client::ui::small_text::parse_radix(s, 16)
                .map(|v| (v != -1).then(|| Color::from_hex(v as u32)))
        };
        if let Some(value) = tag.strip_prefix("col=") {
            if let Some(color) = rs09_client::ui::small_text::parse_radix(value, 16) {
                self.tint = Color {
                    a: self.tint.a,
                    ..Color::from_hex(color as u32)
                };
            }
        } else if tag == "/col" {
            self.tint = Color {
                a: self.tint.a,
                ..self.base
            };
        } else if let Some(value) = tag.strip_prefix("trans=") {
            if let Some(alpha) = rs09_client::ui::small_text::parse_radix(value, 10) {
                self.tint.a = alpha as f32 / 256.0;
            }
        } else if tag == "/trans" {
            self.tint.a = self.base.a;
        } else if let Some(value) = tag.strip_prefix("shad=") {
            if let Some(color) = hex(value) {
                self.shadow = color;
            }
        } else if tag == "shad" {
            self.shadow = Some(BLACK);
        } else if tag == "/shad" {
            self.shadow = self.base_shadow;
        } else if let Some(value) = tag.strip_prefix("u=") {
            if let Some(color) = hex(value) {
                self.underline = color;
            }
        } else if tag == "u" {
            self.underline = Some(BLACK);
        } else if tag == "/u" {
            self.underline = None;
        } else if let Some(value) = tag.strip_prefix("str=") {
            if let Some(color) = hex(value) {
                self.strike = color;
            }
        } else if tag == "str" {
            self.strike = Some(Color::from_hex(0x800000));
        } else if tag == "/str" {
            self.strike = None;
        } else if tag == "br" {
            self.tint = self.base;
            self.shadow = self.base_shadow;
            self.underline = None;
            self.strike = None;
            self.spacing = 0;
            self.fraction = 0;
        }
    }
}
struct BitmapFont {
    texture: Texture2D,
    metrics: Metrics,
    glyphs: Vec<(Rect, Vec2)>,
    top: i32,
    bottom: i32,
    icons: Vec<(Texture2D, Vec2, [usize; 2])>,
}
impl BitmapFont {
    fn legacy_line(&self, line: &rs09_client::ui::widget_legacy::Line) {
        let width = self.metrics.width(&line.text);
        let offset = match line.align {
            0 => 0,
            1 => width / 2,
            _ => width,
        };
        self.line(
            &line.text,
            line.position[0].wrapping_sub(offset) as f32,
            line.position[1] as f32,
            Color::from_hex(line.color as u32),
            line.shadow,
        );
    }
    fn load(data: &[u8], sprite: &[u8], name_icons: Option<&[u8]>) -> Result<Self, String> {
        let mut metrics = Metrics::decode(data).map_err(|e| e.to_string())?;
        let mut icons = Vec::new();
        if let Some(raw) = name_icons {
            let sheet = Sheet::decode(raw).map_err(|e| e.to_string())?;
            for (index, frame) in sheet.frames.iter().enumerate() {
                let rgba = sheet.rgba(index).map_err(|e| e.to_string())?;
                let texture =
                    Texture2D::from_rgba8(frame.size[0] as u16, frame.size[1] as u16, &rgba);
                texture.set_filter(FilterMode::Nearest);
                icons.push((
                    texture,
                    vec2(frame.offset[0] as f32, frame.offset[1] as f32),
                    sheet.size,
                ));
                metrics.name_icons.push(sheet.size[0] as i32);
            }
        }
        let sheet = Sheet::decode(sprite).map_err(|e| e.to_string())?;
        if sheet.frames.len() < 256 {
            return Err("Font has fewer than 256 glyphs".into());
        }
        let cell_w = sheet
            .frames
            .iter()
            .map(|f| f.size[0])
            .max()
            .unwrap_or(0)
            .max(1)
            + 2;
        let cell_h = sheet
            .frames
            .iter()
            .map(|f| f.size[1])
            .max()
            .unwrap_or(0)
            .max(1)
            + 2;
        let (w, h) = (cell_w * 16, cell_h * 16);
        let mut pixels = vec![0; w * h * 4];
        let mut glyphs = Vec::new();
        let (mut min, mut max) = (i32::MAX, i32::MIN);
        for (i, f) in sheet.frames.iter().take(256).enumerate() {
            let ox = i % 16 * cell_w;
            let oy = i / 16 * cell_h;
            for y in 0..f.size[1] {
                for x in 0..f.size[0] {
                    let a = if f.indices[y * f.size[0] + x] != 0 {
                        255
                    } else {
                        0
                    };
                    let off = ((y + oy) * w + x + ox) * 4;
                    pixels[off..off + 4].copy_from_slice(&[255, 255, 255, a]);
                }
            }
            glyphs.push((
                Rect::new(ox as f32, oy as f32, f.size[0] as f32, f.size[1] as f32),
                vec2(f.offset[0] as f32, f.offset[1] as f32),
            ));
            if f.size[1] != 0 {
                min = min.min(f.offset[1] as i32);
                max = max.max((f.offset[1] + f.size[1]) as i32);
            }
        }
        let texture = Texture2D::from_rgba8(w as u16, h as u16, &pixels);
        texture.set_filter(FilterMode::Nearest);
        let top = metrics.line_height - min;
        let bottom = max - metrics.line_height;
        Ok(Self {
            texture,
            metrics,
            glyphs,
            top,
            bottom,
            icons,
        })
    }
    fn line(&self, text: &str, x: f32, baseline: f32, color: Color, shadow: bool) {
        self.styled_line(text, x, baseline, &mut TextStyle::new(color, shadow));
    }
    fn styled_line(&self, text: &str, x: f32, baseline: f32, style: &mut TextStyle) {
        self.offset_line(text, x, baseline, style, &[]);
    }
    fn offset_line(
        &self,
        text: &str,
        mut x: f32,
        baseline: f32,
        style: &mut TextStyle,
        offsets: &[[i32; 2]],
    ) {
        let mut previous = 0;
        let mut glyph_index = 0;
        for (raw, glyph, tag) in tokens(text) {
            if tag {
                let tag = raw.trim_start_matches('<').trim_end_matches('>');
                if let Some(index) = tag
                    .strip_prefix("img=")
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    if let Some((texture, offset, size)) = self.icons.get(index) {
                        let delta = offsets.get(glyph_index).copied().unwrap_or([0, 0]);
                        glyph_index += 1;
                        draw_texture(
                            texture,
                            x + offset.x + delta[0] as f32,
                            baseline - size[1] as f32 + offset.y + delta[1] as f32,
                            // Font.render 263-267: renderAlpha with the text alpha.
                            Color::new(1.0, 1.0, 1.0, style.tint.a),
                        );
                        x += size[0] as f32;
                        previous = 0;
                    }
                    continue;
                }
                style.tag(tag);
            }
            let Some(glyph) = glyph else {
                continue;
            };
            let id = usize::from(glyph);
            if previous != 0
                && let Some(kerning) = &self.metrics.kerning
            {
                x += f32::from(kerning[previous * 256 + id]);
            }
            let (src, offset) = self.glyphs[id];
            let delta = offsets.get(glyph_index).copied().unwrap_or([0, 0]);
            glyph_index += 1;
            let p = vec2(x, baseline - self.metrics.line_height as f32)
                + offset
                + vec2(delta[0] as f32, delta[1] as f32);
            if glyph == 32 && style.spacing > 0 {
                style.fraction += style.spacing;
                x += (style.fraction >> 8) as f32;
                style.fraction &= 255;
            }
            if src.w > 0.0 && src.h > 0.0 {
                if let Some(color) = style.shadow {
                    draw_texture_ex(
                        &self.texture,
                        p.x + 1.0,
                        p.y + 1.0,
                        Color {
                            a: style.tint.a,
                            ..color
                        },
                        DrawTextureParams {
                            source: Some(src),
                            ..Default::default()
                        },
                    );
                }
                draw_texture_ex(
                    &self.texture,
                    p.x,
                    p.y,
                    style.tint,
                    DrawTextureParams {
                        source: Some(src),
                        ..Default::default()
                    },
                );
            }
            let width = self.metrics.advances[id] as f32;
            if let Some(color) = style.strike {
                draw_rectangle(
                    x,
                    baseline - self.metrics.line_height as f32
                        + (self.metrics.line_height as f32 * 0.7) as i32 as f32,
                    width,
                    1.0,
                    color,
                );
            }
            if let Some(color) = style.underline {
                draw_rectangle(x, baseline + 1.0, width, 1.0, color);
            }
            x += width;
            previous = id;
        }
    }
    fn paragraph(
        &self,
        text: &str,
        r: Rect,
        color: Color,
        shadow: bool,
        alignment: [i32; 2],
        padding: i32,
    ) {
        let lines = self.metrics.paragraph(
            text,
            [r.x as i32, r.y as i32, r.w as i32, r.h as i32],
            alignment,
            padding,
            [self.top, self.bottom],
        );
        let mut style = TextStyle::new(color, shadow);
        for line in lines {
            style.spacing = line.spacing;
            self.styled_line(
                &line.text,
                line.position[0] as f32,
                line.position[1] as f32,
                &mut style,
            );
            style.spacing = 0;
        }
    }
}
#[derive(Default)]
pub struct Renderer {
    sprite_material: Option<Material>,
    sprites: BTreeMap<u64, crate::widget_sprite_render::Sprite>,
    sprite_lru: VecDeque<u64>,
    fonts: BTreeMap<i32, BitmapFont>,
    overhead_ids: Option<BTreeMap<String, i32>>,
    overhead_sheets: BTreeMap<i32, Sheet>,
    overhead_textures: BTreeMap<(i32, usize), Texture2D>,
    scrollbar_sprites: Option<Vec<(Texture2D, [i32; 2])>>,
    icons: Option<rs09_client::ui::icon::Icons>,
    /// `ObjTypeList.objectSpriteCache`: 100 sprites, least recently used out
    /// first, keyed by id, count, outline, shadow and whether text is drawn.
    item_sprites: BTreeMap<[i32; 5], crate::widget_sprite_render::Sprite>,
    item_lru: VecDeque<[i32; 5]>,
    /// `ObjTypeList.font`, the software p11 font the count is drawn with.
    item_font: Option<rs09_client::ui::icon_text::SoftFont>,
    /// The rasteriser brightness and sprite-reset count the cache was built for.
    item_state: Option<(u32, u32)>,
    /// `client.objRecolorPalette` as of the last reset (CS2 5401 resets).
    item_palette: rs09_client::recolor::Palette,
    pub inventory_drag: Option<rs09_client::ui::drag::InventoryVisual>,
    /// Bounded data-only record of a pressed item actually submitted this frame.
    pub inventory_press_presented: Option<serde_json::Value>,
    pub scene_delta: i32,
    pub excluded: Option<rs09_client::ui::state::Key>,
    pub alpha_override: Option<i32>,
    world_map: crate::world_map_render::Renderer,
    /// `Cheat.displayFps`: the frame rate to show in the debug component
    /// (client code 1405), or None while the display is off.
    pub display_fps: Option<i32>,
    /// What `applyClientCode` reads from the rest of the client this frame.
    pub client_codes: rs09_client::ui::client_code::Inputs,
    gender_sprites: rs09_client::ui::client_code::GenderSprites,
    /// The title screen's 3D scene has been drawn in the title interface's
    /// scene component (client code 1403), which is then left as it is.
    pub title_scene_drawn: bool,
}
struct ItemDraw {
    id: i32,
    count: i32,
    position: Vec2,
    outline: i32,
    shadow: u32,
    draw_text: bool,
    /// Out of 256, as `Sprite.renderAlpha` takes it.
    alpha: i32,
}
impl Renderer {
    fn scrollbar(&mut self, state: &State, node: &DrawNode) -> Result<(), String> {
        use rs09_client::ui::scrollbar::{self, Part};
        let Some(c) = state.components.get(&node.key) else {
            return Ok(());
        };
        let clip = node.paint_clip;
        if clip.w <= 0 || clip.h <= 0 {
            return Ok(());
        }
        crate::display_host::scissor(Some((clip.x, clip.y, clip.w, clip.h)));
        scrollbar::paint(
            c.scroll_y,
            c.scroll_max_v,
            [node.rect.x.wrapping_add(c.width), node.rect.y],
            c.height,
            |part| {
                if let Part::Sprite(frame, [x, y]) = part {
                    if self.scrollbar_sprites.is_none() {
                        let sheet = Sheet::decode(state.pack.get("sprite/scrollbar")?)
                            .map_err(|e| e.to_string())?;
                        if sheet.frames.len() != 2 {
                            return Err("Scrollbar requires two indexed sprite frames".into());
                        }
                        let mut sprites = Vec::new();
                        for (index, f) in sheet.frames.iter().enumerate() {
                            let rgba = sheet.rgba(index).map_err(|e| e.to_string())?;
                            let texture =
                                Texture2D::from_rgba8(f.size[0] as u16, f.size[1] as u16, &rgba);
                            texture.set_filter(FilterMode::Nearest);
                            sprites.push((texture, f.offset.map(|n| n as i32)));
                        }
                        self.scrollbar_sprites = Some(sprites);
                    }
                    let (texture, offset) = &self.scrollbar_sprites.as_ref().unwrap()[frame];
                    draw_texture(
                        texture,
                        x.wrapping_add(offset[0]) as f32,
                        y.wrapping_add(offset[1]) as f32,
                        WHITE,
                    );
                } else if let Some((geometry, color)) =
                    scrollbar::geometry(part, screen_height() as i32)
                {
                    legacy_geometry(geometry, Color::from_hex(color as u32));
                }
                Ok(())
            },
        )
    }
    fn overhead_sheet(&mut self, visual: &Pack, id: i32) -> Result<&Sheet, String> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.overhead_sheets.entry(id) {
            entry.insert(
                Sheet::decode(visual.get(&format!("sprite/{id}"))?).map_err(|e| e.to_string())?,
            );
        }
        Ok(&self.overhead_sheets[&id])
    }
    pub fn draw_sheet_frame(
        &mut self,
        visual: &Pack,
        group: i32,
        frame: usize,
        at: [i32; 2],
    ) -> Result<(), String> {
        self.overhead_sheet(visual, group)?;
        let sheet = &self.overhead_sheets[&group];
        let sprite = sheet
            .frames
            .get(frame)
            .ok_or_else(|| format!("Sprite {group} frame {frame} absent"))?;
        let texture = self.overhead_textures.entry((group, frame));
        let texture = match texture {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let rgba = sheet.rgba(frame).map_err(|e| e.to_string())?;
                let texture =
                    Texture2D::from_rgba8(sprite.size[0] as u16, sprite.size[1] as u16, &rgba);
                texture.set_filter(FilterMode::Nearest);
                entry.insert(texture)
            }
        };
        draw_texture(
            texture,
            (at[0] + sprite.offset[0] as i32) as f32,
            (at[1] + sprite.offset[1] as i32) as f32,
            WHITE,
        );
        Ok(())
    }
    /// Tile hint sprites (`MiniMap.renderHeadHints`): frames of the head-hint
    /// sheet at their top-left corners. Call after [`Self::overheads`], which
    /// resolves the sheet.
    pub fn tile_hints(
        &mut self,
        visual: &Pack,
        sprites: &[rs09_client::hint_arrow::TileSprite],
    ) -> Result<(), String> {
        let Some(sheet) = self
            .overhead_ids
            .as_ref()
            .and_then(|ids| ids.get("hint_headicons").copied())
        else {
            return Ok(());
        };
        for &(frame, at) in sprites {
            self.draw_sheet_frame(visual, sheet, frame, at)?;
        }
        Ok(())
    }
    /// Original overhead commands use cache fonts/sprites; camera projection is a service.
    pub fn overheads(
        &mut self,
        ui: &Pack,
        visual: &Pack,
        actors: &[rs09_client::overheads::Actor],
        config: &mut rs09_client::overheads::Config,
        project: impl FnMut(usize, i32) -> [i32; 2],
    ) -> Result<usize, String> {
        use rs09_client::overheads::{self, Draw};
        if self.overhead_ids.is_none() {
            self.overhead_ids = Some(
                serde_json::from_slice(ui.get("overhead/assets")?).map_err(|e| e.to_string())?,
            );
        }
        let ids = self.overhead_ids.as_ref().unwrap().clone();
        let id = |name: &str| {
            ids.get(name)
                .copied()
                .ok_or_else(|| format!("Overhead asset name absent: {name}"))
        };
        let bold = id("b12_full")?;
        let small = id("p11_full")?;
        config.font_height = self.font(ui, visual, bold)?.metrics.line_height;
        let bar = id("hitbar_default")?;
        let sheet = self.overhead_sheet(visual, bar)?;
        if sheet.frames.len() != 2 {
            return Err("Default hit bar requires two frames".into());
        }
        config.default_bar = sheet.frames[0].size.map(|n| n as i32);
        config.prayer_count = self
            .overhead_sheet(visual, id("headicons_prayer")?)?
            .frames
            .len() as i32;
        for a in actors {
            if let Some(n) = a.npc
                && n.hitbar >= 0
                && visual.get(&format!("sprite/{}", n.hitbar)).is_ok()
            {
                let s = self.overhead_sheet(visual, n.hitbar)?;
                if s.frames.len() == 2 {
                    config
                        .custom_bars
                        .insert(n.hitbar, s.frames[0].size.map(|n| n as i32));
                }
            }
        }
        let commands = {
            let font = self.font(ui, visual, bold)?;
            overheads::layout(actors, config, |text| font.metrics.width(text), project)
        };
        let [left, top, width, height] = config.viewport;
        let result = (|| {
            for cmd in &commands {
                match cmd {
                    Draw::Clip {
                        rect: [x0, y0, x1, y1],
                    } => {
                        let x = (*x0).max(left);
                        let y = (*y0).max(top);
                        let w = (*x1).min(left + width).saturating_sub(x).max(0);
                        let h = (*y1).min(top + height).saturating_sub(y).max(0);
                        crate::display_host::scissor(Some((x, y, w, h)));
                    }
                    Draw::Sprite { group, frame, at } => {
                        let gid = match *group {
                            overheads::HITMARKS => id("hitmarks")?,
                            overheads::HITBARS => bar,
                            overheads::PRAYERS => id("headicons_prayer")?,
                            overheads::SKULLS => id("headicons_pk")?,
                            overheads::HINTS => id("hint_headicons")?,
                            n => n,
                        };
                        self.draw_sheet_frame(visual, gid, *frame, *at)?;
                    }
                    Draw::Text {
                        small: is_small,
                        mode,
                        text,
                        at,
                        color,
                        tick,
                        age,
                    } => {
                        let font = self.font(ui, visual, if *is_small { small } else { bold })?;
                        let x = at[0]
                            - if *mode == 4 {
                                0
                            } else {
                                font.metrics.width(text) / 2
                            };
                        let offsets =
                            overheads::glyph_offsets(*mode, text.chars().count(), *tick, *age);
                        font.offset_line(
                            text,
                            x as f32,
                            at[1] as f32,
                            &mut TextStyle::new(Color::from_hex(*color as u32), true),
                            &offsets,
                        );
                    }
                }
            }
            Ok(commands.len())
        })();
        crate::display_host::scissor(None);
        result
    }
    fn scroll_inventory_parent(
        &mut self,
        state: &mut State,
        layer: i32,
        y: i32,
        drag_y: i32,
        clip: rs09_client::ui::state::Rect,
    ) -> Result<(), String> {
        if layer == -1 {
            return Ok(());
        }
        let parent = state
            .components
            .get_mut(&rs09_client::ui::state::Key::component(layer));
        let mut values = parent
            .as_ref()
            .map(|p| rs09_client::ui::inventory_scroll::Parent {
                scroll: p.scroll_y,
                range: p.scroll_max_v,
                height: p.height,
            });
        let drag = self
            .inventory_drag
            .as_mut()
            .ok_or("Missing inventory press")?;
        let before = values.as_ref().map_or(0, |p| p.scroll);
        rs09_client::ui::inventory_scroll::edge_scroll(
            values.as_mut(),
            &mut drag.press[1],
            y,
            drag_y,
            [clip.y, clip.y.wrapping_add(clip.h)],
            self.scene_delta,
            |_, _| {}, // The native presenter redraws every frame.
        )?;
        if let (Some(parent), Some(values)) = (parent, values) {
            drag.scroll_delta = values.scroll.wrapping_sub(before);
            parent.scroll_y = values.scroll;
        }
        Ok(())
    }
    /// Client code 1405 while `::fpson` is in effect. Only the frame rate has
    /// a native source; the heap, graphics-card and cache-verification lines
    /// of the original have none and are left out.
    fn debug_overlay(
        &mut self,
        state: &State,
        visual: &Pack,
        node: &DrawNode,
        fps: i32,
    ) -> Result<(), String> {
        use rs09_client::ui::client_code::{Overlay, overlay};
        let ids: BTreeMap<String, i32> = serde_json::from_slice(state.pack.get("overhead/assets")?)
            .map_err(|e| e.to_string())?;
        let paint = node.paint_clip;
        crate::display_host::scissor(Some((paint.x, paint.y, paint.w, paint.h)));
        let values = Overlay {
            fps,
            ..Overlay::default()
        };
        for line in overlay([node.rect.x, node.rect.y], node.rect.w, values) {
            let name = if line.small { "p11_full" } else { "p12_full" };
            let id = *ids
                .get(name)
                .ok_or_else(|| format!("Overlay font {name} asset absent"))?;
            let font = self.font(&state.pack, visual, id)?;
            font.line(
                &line.text,
                (line.at[0] - font.metrics.width(&line.text)) as f32,
                line.at[1] as f32,
                Color::from_hex(line.color as u32),
                true,
            );
        }
        Ok(())
    }
    /// Item sprites depend on the software rasteriser's brightness and are
    /// thrown away whenever the original clears `objectSpriteCache`.
    fn sync_items(&mut self, vm: &rs09_client::ui::vm::Vm) {
        let brightness = rs09_client::ui::icon_text::raster_brightness(
            vm.preferences.brightness,
            vm.preferences.high_detail_lighting,
        );
        let state = (brightness.to_bits(), vm.item_sprite_resets);
        if self.item_state != Some(state) {
            if !self.item_sprites.is_empty() {
                // Queued geometry must finish before its textures drop.
                unsafe {
                    get_internal_gl().flush();
                }
            }
            self.item_sprites.clear();
            self.item_lru.clear();
            self.icons = None;
            self.item_state = Some(state);
            self.item_palette = vm.recolor.clone();
        }
    }
    /// `Inv.getObjectSprite`: the 36 by 32 sprite with its count drawn in.
    fn item_sprite(
        &mut self,
        catalog: &mut rs09_client::ui::item::Catalog,
        ui: &Pack,
        visual: &Pack,
        request: &ItemDraw,
    ) -> Result<&crate::widget_sprite_render::Sprite, String> {
        use rs09_client::ui::icon_text;
        let key = [
            request.id,
            request.count,
            request.outline,
            request.shadow as i32,
            i32::from(request.draw_text),
        ];
        if !self.item_sprites.contains_key(&key) {
            if self.icons.is_none() {
                let brightness = self
                    .item_state
                    .map_or(0.7, |(bits, _)| f32::from_bits(bits));
                let mut icons = rs09_client::ui::icon::Icons::new(visual, brightness)?;
                icons.recolor = self.item_palette.colors;
                self.icons = Some(icons);
            }
            let mut pixels = self.icons.as_mut().unwrap().render(
                catalog,
                ui,
                visual,
                request.id,
                request.count,
                request.outline,
                request.shadow,
            )?;
            // The count is judged on the definition the sprite was drawn
            // from, which a large stack replaces with its count variant.
            let mut definition = catalog.get(ui, request.id)?;
            let variant = definition.count_variant(request.count);
            if variant != definition.id {
                definition = catalog.get(ui, variant)?;
            }
            if icon_text::shows_count(request.draw_text, definition.stackable, request.count) {
                if self.item_font.is_none() {
                    let metrics =
                        Metrics::decode(ui.get("font/494")?).map_err(|e| e.to_string())?;
                    let sheet =
                        Sheet::decode(visual.get("sprite/494")?).map_err(|e| e.to_string())?;
                    self.item_font = Some(icon_text::SoftFont::new(&metrics, &sheet)?);
                }
                icon_text::bake(
                    &mut pixels,
                    self.item_font.as_ref().unwrap(),
                    request.draw_text,
                    definition.stackable,
                    request.count,
                );
            }
            if self.sprite_material.is_none() {
                self.sprite_material = Some(crate::widget_sprite_render::material()?);
            }
            let material = self
                .sprite_material
                .as_ref()
                .expect("sprite material")
                .clone();
            let image = rs09_client::ui::widget_sprite::Image {
                inner: [36, 32],
                offset: [0, 0],
                size: [36, 32],
                pixels,
                alpha: false,
            };
            let sprite = crate::widget_sprite_render::Sprite::new(
                request.id,
                rs09_client::ui::widget_sprite::Options {
                    outline: request.outline,
                    shadow: request.shadow as i32,
                    ..Default::default()
                },
                image,
                material,
            )?;
            if self.item_sprites.len() >= 100 {
                unsafe {
                    get_internal_gl().flush();
                }
                if let Some(old) = self.item_lru.pop_front() {
                    self.item_sprites.remove(&old);
                }
            }
            self.item_sprites.insert(key, sprite);
        }
        self.item_lru.retain(|&k| k != key);
        self.item_lru.push_back(key);
        Ok(&self.item_sprites[&key])
    }
    fn item(
        &mut self,
        catalog: &mut rs09_client::ui::item::Catalog,
        ui: &Pack,
        visual: &Pack,
        request: ItemDraw,
    ) -> Result<(), String> {
        let (position, alpha) = (request.position, request.alpha);
        self.item_sprite(catalog, ui, visual, &request)?
            .icon(position, alpha);
        Ok(())
    }
    fn sprite(
        &mut self,
        pack: &Pack,
        id: i32,
    ) -> Result<&crate::widget_sprite_render::Sprite, String> {
        self.styled_sprite(pack, id, rs09_client::ui::widget_sprite::Options::default())
    }
    fn styled_sprite(
        &mut self,
        pack: &Pack,
        id: i32,
        options: rs09_client::ui::widget_sprite::Options,
    ) -> Result<&crate::widget_sprite_render::Sprite, String> {
        let key = options.key(id);
        if !self.sprites.contains_key(&key) {
            let sheet =
                Sheet::decode(pack.get(&format!("sprite/{id}"))?).map_err(|e| e.to_string())?;
            let image = rs09_client::ui::widget_sprite::Image::prepare(&sheet, options)?;
            if self.sprite_material.is_none() {
                self.sprite_material = Some(crate::widget_sprite_render::material()?);
            }
            let material = self
                .sprite_material
                .as_ref()
                .expect("sprite material")
                .clone();
            let sprite = crate::widget_sprite_render::Sprite::new(id, options, image, material)?;
            if self.sprites.len() >= 200 {
                // Queued geometry must finish before its last texture owner drops.
                unsafe {
                    get_internal_gl().flush();
                }
                if let Some(old) = self.sprite_lru.pop_front() {
                    self.sprites.remove(&old);
                }
            }
            self.sprites.insert(key, sprite);
        }
        self.sprite_lru.retain(|&k| k != key);
        self.sprite_lru.push_back(key);
        Ok(&self.sprites[&key])
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn sprite_snapshot(&self) -> serde_json::Value {
        serde_json::json!(
            self.sprites
                .iter()
                .map(|(key, s)| serde_json::json!({"key":key.to_string(),"image":s.snapshot()}))
                .collect::<Vec<_>>()
        )
    }
    fn font(&mut self, ui: &Pack, visual: &Pack, id: i32) -> Result<&BitmapFont, String> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.fonts.entry(id) {
            entry.insert(BitmapFont::load(
                ui.get(&format!("font/{id}"))?,
                visual.get(&format!("sprite/{id}"))?,
                ui.get("sprite/name_icons").ok(),
            )?);
        }
        Ok(&self.fonts[&id])
    }
    /// `Fonts.b12Full.renderCenter(LOADINGDOTDOTDOT, x, baseline, 0xFFFFFF, -1)`
    /// under the world map's loading box.
    fn world_map_loading(&mut self, ui: &Pack, visual: &Pack, at: [i32; 2]) -> Result<(), String> {
        let ids: BTreeMap<String, i32> =
            serde_json::from_slice(ui.get("overhead/assets")?).map_err(|e| e.to_string())?;
        let id = *ids.get("b12_full").ok_or("b12_full font asset absent")?;
        let font = self.font(ui, visual, id)?;
        let text = "Loading...";
        font.line(
            text,
            at[0].wrapping_sub(font.metrics.width(text) / 2) as f32,
            at[1] as f32,
            WHITE,
            false,
        );
        Ok(())
    }
    pub fn draw(
        &mut self,
        state: &mut State,
        visual: &Pack,
        nodes: &[DrawNode],
        mut minimap: Option<&mut crate::minimap_render::Renderer>,
        models: &mut crate::ui_model_render::Renderer,
        model_context: &crate::ui_model_render::Context,
    ) {
        self.sync_items(model_context.vm);
        let mut scrollbars: Vec<&DrawNode> = Vec::new();
        // The world map (1400, 1401) and the debug overlay (1405) end their
        // part of the walk: nothing below them is drawn.
        let mut skip_until = 0;
        for (index, node) in nodes.iter().enumerate() {
            while scrollbars
                .last()
                .is_some_and(|parent| parent.subtree_end <= index)
            {
                if let Err(e) = self.scrollbar(state, scrollbars.pop().unwrap()) {
                    state.diagnostic(e);
                }
            }
            if index < skip_until {
                continue;
            }
            // Cs1ScriptRunner.renderComponent 315-317.
            if let Some(component) = state.components.get_mut(&node.key)
                && component.client_code > 0
            {
                rs09_client::ui::client_code::apply(
                    component,
                    &mut self.gender_sprites,
                    &self.client_codes,
                );
            }
            if self.excluded == Some(node.key) {
                continue;
            }
            let Some(c) = state.components.get(&node.key).cloned() else {
                continue;
            };
            if c.kind == 0 && !c.if3 && c.scroll_max_v > c.height {
                scrollbars.push(node);
            }
            let r = Rect::new(
                node.rect.x as f32,
                node.rect.y as f32,
                node.rect.w as f32,
                node.rect.h as f32,
            );
            if c.if3 && (node.clip.w <= 0 || node.clip.h <= 0) {
                continue;
            }
            let clip = if matches!(c.kind, 4 | 7 | 8 | 9) || c.kind == 5 && !c.if3 {
                node.paint_clip
            } else {
                node.clip
            };
            if (clip.w <= 0 || clip.h <= 0) && c.kind != 2 {
                continue;
            }
            crate::display_host::scissor(Some((clip.x, clip.y, clip.w, clip.h)));
            if matches!(c.client_code, 1338 | 1339) {
                if let Some(map) = minimap.as_deref_mut() {
                    match map.draw(state, visual, node) {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(e) => state.diagnostic(format!("Minimap: {e}")),
                    }
                } else {
                    state.diagnostic("Minimap assets unavailable in this visual pack".into());
                }
            }
            // The hover action text's host is not drawn itself (`action_tooltip`).
            if c.client_code == rs09_client::ui::action_tooltip::HOST_CLIENT_CODE {
                continue;
            }
            if matches!(c.client_code, 1400 | 1401) {
                // Cs1ScriptRunner 456-478: the world map and its overview
                // replace the component, children included, and are clipped
                // to its rectangle alone.
                skip_until = node.subtree_end;
                crate::display_host::scissor(Some((
                    node.rect.x,
                    node.rect.y,
                    node.rect.w,
                    node.rect.h,
                )));
                let drawn = self
                    .world_map
                    .draw(state, model_context.vm, node, c.client_code)
                    .and_then(|loading| match loading {
                        Some(at) => self.world_map_loading(&state.pack, visual, at),
                        None => Ok(()),
                    });
                if let Err(e) = drawn {
                    state.diagnostic(format!("World map: {e}"));
                }
                continue;
            }
            if c.client_code == 1403 {
                // Cs1ScriptRunner.java:388-398: with the hardware renderer
                // the title interface's scene component is the game scene.
                // Where no scene can be built the title background stands
                // in for it.
                if self.title_scene_drawn {
                    continue;
                }
                if let Err(e) = self.title_scene(&state.pack, visual, node.rect) {
                    state.diagnostic(format!("Title scene: {e}"));
                }
                continue;
            }
            if c.client_code == 1402 {
                // Cs1ScriptRunner.java:478-485: the title flames belong to
                // the software renderer; the hardware one draws nothing.
                continue;
            }
            if c.client_code == 1405 {
                // Cs1ScriptRunner.java:486-533: the debug component draws
                // nothing of its own or below it, only the `::fpson` lines.
                skip_until = node.subtree_end;
                if let Some(fps) = self.display_fps
                    && let Err(e) = self.debug_overlay(state, visual, node, fps)
                {
                    state.diagnostic(format!("Frame rate display: {e}"));
                }
                continue;
            }
            if matches!(c.kind, 4 | 7) && c.font == -1 {
                continue;
            }
            let alpha = self.alpha_override.unwrap_or(c.alpha);
            match c.kind {
                6 => {
                    if let Err(e) = models.draw(state, node, model_context) {
                        state.diagnostic(format!("UI model: {e}"));
                    }
                }
                2 => {
                    use rs09_client::ui::inventory_render::{self, Context, Request};
                    let context = Context {
                        origin: [node.rect.x, node.rect.y],
                        clip: [
                            node.clip.x,
                            node.clip.y,
                            node.clip.x.wrapping_add(node.clip.w),
                            node.clip.y.wrapping_add(node.clip.h),
                        ],
                        dragged_slot: self
                            .inventory_drag
                            .filter(|d| d.key == node.key)
                            .map(|d| d.slot),
                        selected_slot: state
                            .selected_item
                            .as_ref()
                            .filter(|s| s.component == c.id)
                            .map(|s| s.slot),
                        pressed_slot: state.inventory_pressed_slot(node.key),
                    };
                    let result = inventory_render::visit(&c, context, |request| match request {
                        Request::Item {
                            slot,
                            at,
                            id,
                            count,
                            outline,
                            shadow,
                            draw_text,
                            alpha,
                            dragged,
                        } => {
                            let selected = self
                                .inventory_drag
                                .filter(|d| dragged && d.key == node.key && d.slot == slot);
                            let delta = selected.map_or([0, 0], |d| {
                                rs09_client::ui::inventory_scroll::delta(d.point, d.press, d.cycles)
                            });
                            let drawn =
                                [at[0].wrapping_add(delta[0]), at[1].wrapping_add(delta[1])];
                            if let Err(error) = self.item(
                                &mut state.items,
                                &state.pack,
                                visual,
                                ItemDraw {
                                    id,
                                    count,
                                    position: vec2(drawn[0] as f32, drawn[1] as f32),
                                    outline,
                                    shadow,
                                    draw_text,
                                    alpha,
                                },
                            ) {
                                state.diagnostic(error);
                            } else {
                                if context.pressed_slot == Some(slot) && !dragged {
                                    self.inventory_press_presented = Some(
                                        serde_json::json!({"component":node.key,"slot":slot,"alpha":alpha,"at":drawn,"tick":state.inventory_press_snapshot()["tick"]}),
                                    );
                                }
                                if dragged {
                                    if let Some(drag) = self.inventory_drag.as_mut() {
                                        drag.source_at = Some(at);
                                        drag.drawn_at = Some(drawn);
                                    }
                                    if let Err(error) = self.scroll_inventory_parent(
                                        state,
                                        c.overlayer,
                                        at[1],
                                        delta[1],
                                        node.clip,
                                    ) {
                                        state.diagnostic(error);
                                    }
                                }
                            }
                        }
                        Request::Background { slot, at } => {
                            if let Some(&sprite) =
                                c.inv_sprite.as_ref().and_then(|a| a.get(slot as usize))
                                && sprite >= 0
                                && let Ok(sprite) = self.sprite(visual, sprite)
                            {
                                sprite.natural(vec2(at[0] as f32, at[1] as f32));
                            }
                        }
                    });
                    if let Err(error) = result {
                        state.diagnostic(error.to_string());
                    }
                }
                3 => {
                    let active = match rs09_client::ui::cs1::is_true(state, model_context.vm, &c) {
                        Ok(v) => v,
                        Err(e) => {
                            state.diagnostic(e);
                            false
                        }
                    };
                    let rgb = rs09_client::ui::widget_style::color(
                        &c,
                        active,
                        state.is_hovered(node.key),
                    );
                    let mut color = Color::from_hex(rgb as u32);
                    // glColor4ub takes the alpha as a byte out of 255.
                    color.a = f32::from(rs09_client::ui::outline::component_alpha(alpha)) / 255.0;
                    if c.filled {
                        draw_rectangle(r.x, r.y, r.w, r.h, color);
                    } else {
                        legacy_border([node.rect.x, node.rect.y, node.rect.w, node.rect.h], color);
                    }
                }
                4 => match self.font(&state.pack, visual, c.font) {
                    Ok(font) => {
                        let active =
                            match rs09_client::ui::cs1::is_true(state, model_context.vm, &c) {
                                Ok(v) => v,
                                Err(e) => {
                                    state.diagnostic(e);
                                    false
                                }
                            };
                        let item = if c.if3 && c.obj_id != -1 {
                            match state.items.get(&state.pack, c.obj_id) {
                                Ok(item) => Some((item.name.clone(), item.stackable)),
                                Err(e) => {
                                    state.diagnostic(e);
                                    continue;
                                }
                            }
                        } else {
                            None
                        };
                        let dns = state.last_login_address.clone();
                        let text = rs09_client::ui::widget_style::text(
                            &c,
                            active,
                            state.is_hovered(node.key),
                            state.is_waiting(node.key),
                            item.as_ref().map(|(name, stackable)| {
                                rs09_client::ui::widget_style::Item {
                                    name: Some(name),
                                    stackable: *stackable,
                                }
                            }),
                            |index| {
                                rs09_client::ui::cs1::run(state, model_context.vm, index as i32, &c)
                            },
                            dns.as_ref()
                                .map(|(ip, result)| rs09_client::ui::widget_style::Dns {
                                    ip: *ip,
                                    result: result.as_deref(),
                                }),
                            "Please wait...",
                        );
                        match text {
                            Ok(text) => font.paragraph(
                                &text.content,
                                r,
                                Color::from_hex(text.color as u32),
                                c.shadowed,
                                [c.halign, c.valign],
                                c.vpadding,
                            ),
                            Err(e) => state.diagnostic(e),
                        }
                    }
                    Err(e) => state.diagnostic(e),
                },
                5 if c.if3 && c.obj_id >= 0 => {
                    // Cs1ScriptRunner 785-878: the item sprite takes the
                    // same tiling, rotation, alpha and resize route as any
                    // other IF3 sprite.
                    let paint = node.paint_clip;
                    crate::display_host::scissor(Some((paint.x, paint.y, paint.w, paint.h)));
                    let drawn = self
                        .item_sprite(
                            &mut state.items,
                            &state.pack,
                            visual,
                            &ItemDraw {
                                id: c.obj_id,
                                count: c.obj_count,
                                position: vec2(r.x, r.y),
                                outline: c.outline_thickness,
                                shadow: c.shadow_color as u32,
                                draw_text: c.obj_draw_text,
                                alpha: 256,
                            },
                        )
                        .and_then(|sprite| {
                            sprite.widget(
                                &c,
                                [node.rect.x, node.rect.y, node.rect.w, node.rect.h],
                                alpha,
                                [
                                    paint.x,
                                    paint.y,
                                    paint.x.wrapping_add(paint.w),
                                    paint.y.wrapping_add(paint.h),
                                ],
                                false,
                            )
                        });
                    if let Err(error) = drawn {
                        state.diagnostic(error);
                    }
                }
                5 => {
                    let active = if c.if3 {
                        false
                    } else {
                        match rs09_client::ui::cs1::is_true(state, model_context.vm, &c) {
                            Ok(v) => v,
                            Err(e) => {
                                state.diagnostic(e);
                                false
                            }
                        }
                    };
                    let id = if active {
                        c.active_sprite_id
                    } else {
                        c.sprite_id
                    };
                    if id == -1 {
                        continue;
                    }
                    let options = rs09_client::ui::widget_sprite::Options {
                        has_alpha: c.has_alpha,
                        h_flip: c.h_flip,
                        v_flip: c.v_flip,
                        outline: c.outline_thickness,
                        shadow: c.shadow_color,
                    };
                    let paint = node.paint_clip;
                    crate::display_host::scissor(Some((paint.x, paint.y, paint.w, paint.h)));
                    match self.styled_sprite(visual, id, options) {
                        Ok(sprite) => {
                            if let Err(e) = sprite.widget(
                                &c,
                                [node.rect.x, node.rect.y, node.rect.w, node.rect.h],
                                alpha,
                                [
                                    paint.x,
                                    paint.y,
                                    paint.x.wrapping_add(paint.w),
                                    paint.y.wrapping_add(paint.h),
                                ],
                                active,
                            ) {
                                state.diagnostic(e);
                            }
                        }
                        Err(e) => state.diagnostic(e),
                    }
                }
                7 => match self.font(&state.pack, visual, c.font) {
                    Ok(font) => {
                        if let Err(e) = rs09_client::ui::widget_legacy::item_list(
                            &c,
                            [node.rect.x, node.rect.y],
                            |id| {
                                state
                                    .items
                                    .get(&state.pack, id)
                                    .map(|item| (item.name.clone(), item.stackable))
                            },
                            |line| font.legacy_line(&line),
                        ) {
                            state.diagnostic(e);
                        }
                    }
                    Err(e) => state.diagnostic(e),
                },
                8 if state.is_tooltip(node.key) => {
                    let result = (|| {
                        let ids: BTreeMap<String, i32> =
                            serde_json::from_slice(state.pack.get("overhead/assets")?)
                                .map_err(|e| e.to_string())?;
                        let id = *ids
                            .get("p12_full")
                            .ok_or("Tooltip p12_full font asset absent")?;
                        let font = self.font(&state.pack, visual, id)?;
                        let dns = state.last_login_address.clone();
                        rs09_client::ui::widget_legacy::tooltip(
                            &c,
                            [node.rect.x, node.rect.y],
                            [clip.x.wrapping_add(clip.w), clip.y.wrapping_add(clip.h)],
                            font.metrics.line_height,
                            || {
                                rs09_client::ui::widget_style::interpolate(
                                    &c.text,
                                    |i| {
                                        rs09_client::ui::cs1::run(
                                            state,
                                            model_context.vm,
                                            i as i32,
                                            &c,
                                        )
                                    },
                                    dns.as_ref().map(|(ip, result)| {
                                        rs09_client::ui::widget_style::Dns {
                                            ip: *ip,
                                            result: result.as_deref(),
                                        }
                                    }),
                                )
                            },
                            |s| font.metrics.width(s),
                            |part| match part {
                                rs09_client::ui::widget_legacy::TooltipPart::Box([x, y, w, h]) => {
                                    draw_rectangle(
                                        x as f32,
                                        y as f32,
                                        w as f32,
                                        h as f32,
                                        Color::from_hex(16777120),
                                    );
                                    legacy_border([x, y, w, h], BLACK);
                                }
                                rs09_client::ui::widget_legacy::TooltipPart::Text(line) => {
                                    font.legacy_line(&line)
                                }
                            },
                        )
                    })();
                    if let Err(e) = result {
                        state.diagnostic(e);
                    }
                }
                9 => legacy_geometry(
                    rs09_client::ui::widget_legacy::line(
                        &c,
                        [node.rect.x, node.rect.y],
                        screen_height() as i32,
                    ),
                    Color::from_hex(c.color as u32),
                ),
                _ => {}
            }
        }
        while let Some(node) = scrollbars.pop() {
            if let Err(e) = self.scrollbar(state, node) {
                state.diagnostic(e);
            }
        }
        crate::display_host::scissor(None);
    }
    pub fn menu_width(
        &mut self,
        ui: &Pack,
        visual: &Pack,
        menu: &rs09_client::interaction::Menu,
    ) -> Result<i32, String> {
        let measure = self.menu_measure(ui, visual)?;
        Ok(rs09_client::context_menu::width(&menu.entries, measure))
    }
    /// Text width in the menu font.
    pub fn menu_measure(
        &mut self,
        ui: &Pack,
        visual: &Pack,
    ) -> Result<impl Fn(&str) -> i32 + '_, String> {
        let font = self.font(ui, visual, 496)?;
        Ok(move |text: &str| font.metrics.width(text))
    }
    pub fn menu(
        &mut self,
        ui: &Pack,
        visual: &Pack,
        menu: &crate::client_ui::ContextMenu,
    ) -> Result<(), String> {
        if !menu.menu.open {
            return Ok(());
        }
        let bounds = menu.bounds;
        let r = Rect::new(
            bounds.x as f32,
            bounds.y as f32,
            bounds.width as f32,
            bounds.height as f32,
        );
        let color = Color::from_hex(0x5d5447);
        draw_rectangle(r.x, r.y, r.w, r.h, color);
        draw_rectangle(r.x + 1.0, r.y + 1.0, r.w - 2.0, 16.0, BLACK);
        // MiniMenu.drawA: drawRect(menuX + 1, menuY + 18, menuWidth - 2, menuHeight - 19, 0).
        legacy_border(
            [
                bounds.x + 1,
                bounds.y + 18,
                bounds.width - 2,
                bounds.height - 19,
            ],
            BLACK,
        );
        let font = self.font(ui, visual, 496)?;
        font.line(
            rs09_client::context_menu::CHOOSE_OPTION,
            r.x + 3.0,
            r.y + 14.0,
            color,
            false,
        );
        let (mx, my) = mouse_position();
        let size = menu.menu.entries.len();
        let hovered = bounds.row_at(size, [mx as i32, my as i32]);
        for (index, entry) in menu.menu.entries.iter().enumerate() {
            font.line(
                &rs09_client::context_menu::op_text(entry),
                r.x + 3.0,
                bounds.row_baseline(size, index) as f32,
                if hovered == Some(index) {
                    YELLOW
                } else {
                    WHITE
                },
                true,
            );
        }
        Ok(())
    }
    /// The hover action text: inside the client-code-1406 component when the
    /// open interfaces have one, else at the game view's top-left corner.
    /// `fixed_view` replaces both with a given corner while the interface is
    /// not shown.
    pub fn action_tooltip(
        &mut self,
        state: &State,
        visual: &Pack,
        nodes: &[DrawNode],
        fixed_view: Option<[i32; 2]>,
        text: String,
        seed: i32,
    ) -> Result<(), String> {
        use rs09_client::ui::{action_tooltip, viewport};
        let (font, draw) = if let Some(view) = fixed_view {
            let font = self.font(&state.pack, visual, action_tooltip::FONT)?;
            let width = font.metrics.width(&text);
            (font, action_tooltip::in_view(text, view, width, seed))
        } else if let Some(host) = action_tooltip::host(state, nodes) {
            // Component.getFont, falling back to b12 as renderTooltip does.
            let id = if host.font != -1 && self.font(&state.pack, visual, host.font).is_ok() {
                host.font
            } else {
                action_tooltip::FONT
            };
            let font = self.font(&state.pack, visual, id)?;
            let padding = [font.top, font.bottom];
            let draw = action_tooltip::hosted(text, &host, &font.metrics, padding, seed);
            (font, draw)
        } else if let Some((view, _)) = viewport::find(state, nodes) {
            let font = self.font(&state.pack, visual, action_tooltip::FONT)?;
            let width = font.metrics.width(&text);
            (
                font,
                action_tooltip::in_view(text, [view.x, view.y], width, seed),
            )
        } else {
            return Ok(());
        };
        let alpha = draw.alpha as f32 / 256.0;
        let rgb = |color: i32| Color {
            a: alpha,
            ..Color::from_hex(color as u32 & 0xffffff)
        };
        let mut style = TextStyle::new(rgb(draw.color), false);
        if draw.shadow != -1 {
            style.shadow = Some(rgb(draw.shadow));
            style.base_shadow = style.shadow;
        }
        let steps =
            action_tooltip::steps(&draw.text, &font.metrics, draw.position[0], &draw.offsets);
        let baseline = draw.position[1] as f32;
        let top = baseline - font.metrics.line_height as f32;
        for step in steps {
            match step {
                action_tooltip::Step::Tag(tag) => style.tag(&tag),
                action_tooltip::Step::Icon { index, x } => {
                    if let Some((texture, offset, size)) = font.icons.get(index) {
                        draw_texture(
                            texture,
                            x as f32 + offset.x,
                            baseline - size[1] as f32 + offset.y,
                            Color::new(1.0, 1.0, 1.0, style.tint.a),
                        );
                    }
                }
                action_tooltip::Step::Glyph {
                    glyph,
                    x,
                    pen,
                    advance,
                } => {
                    let (src, offset) = font.glyphs[usize::from(glyph)];
                    if glyph != 32 && src.w > 0.0 && src.h > 0.0 {
                        let params = || DrawTextureParams {
                            source: Some(src),
                            ..Default::default()
                        };
                        let p = vec2(x as f32, top) + offset;
                        if let Some(shadow) = style.shadow {
                            let shadow = Color {
                                a: style.tint.a,
                                ..shadow
                            };
                            draw_texture_ex(&font.texture, p.x + 1.0, p.y + 1.0, shadow, params());
                        }
                        draw_texture_ex(&font.texture, p.x, p.y, style.tint, params());
                    }
                    let (pen, advance) = (pen as f32, advance as f32);
                    if let Some(color) = style.strike {
                        let y = top + (font.metrics.line_height as f32 * 0.7) as i32 as f32;
                        draw_rectangle(pen, y, advance, 1.0, color);
                    }
                    if let Some(color) = style.underline {
                        draw_rectangle(pen, baseline, advance, 1.0, color);
                    }
                }
            }
        }
        Ok(())
    }
}
