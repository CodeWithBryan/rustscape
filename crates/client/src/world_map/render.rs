//! `WorldMap.render` and the routines under it, plus the overview of
//! `Cs1ScriptRunner.renderWorldMapOverview`.
use super::{
    Assets, Client, MapElement, Rendered, WorldMap,
    raster::{Raster, Sprite, render_scaled},
};
use crate::{
    minimap::{Indexed, MapScene},
    ui::sprite::Sheet,
};

/// `Fonts.p11Full.splitParagraph(name, null, labelLines)`: with no widths the
/// font splits at `<br>` only, each line but the last keeping its tag. Other
/// tags are copied through and an unclosed `<` swallows the rest. The
/// original has room for five lines; further ones are dropped here.
pub fn split_label(name: &[u8]) -> Vec<Vec<u8>> {
    let mut lines = Vec::new();
    let mut buffer = Vec::new();
    let mut line_start = 0;
    let mut tag_start = None;
    for (i, &byte) in name.iter().enumerate() {
        let mut ch = byte;
        if ch == b'<' {
            tag_start = Some(i);
            continue;
        }
        if ch == b'>' {
            if let Some(start) = tag_start.take() {
                let tag = &name[start + 1..i];
                buffer.push(b'<');
                buffer.extend_from_slice(tag);
                buffer.push(b'>');
                if tag == b"br" {
                    if lines.len() == 5 {
                        return lines;
                    }
                    lines.push(buffer[line_start..].to_vec());
                    line_start = buffer.len();
                }
                ch = 0;
            }
        }
        if tag_start.is_none() && ch != 0 {
            buffer.push(ch);
        }
    }
    if buffer.len() > line_start && lines.len() < 5 {
        lines.push(buffer[line_start..].to_vec());
    }
    lines
}

impl WorldMap {
    /// `MsiTypeList.get(id)`.
    fn map_scene(&mut self, assets: &dyn Assets, id: i32) -> MapScene {
        if let Some(scene) = self.map_scenes.get(&id) {
            return scene.clone();
        }
        let scene = assets
            .map_scene(id)
            .and_then(|data| MapScene::decode(&data).ok())
            .unwrap_or_default();
        self.map_scenes.insert(id, scene.clone());
        scene
    }

    /// `MsiType.getSprite(rotation)`: the first frame with the map-scene
    /// palette jitter, turned a quarter `rotation` times. A sheet that is
    /// missing or does not decode is asked for again next time.
    fn ensure_map_scene_sprite(&mut self, assets: &dyn Assets, sprite: i32, rotation: i32) -> bool {
        let key = rotation << 16 | sprite;
        if self.map_scene_sprites.contains_key(&key) {
            return true;
        }
        let Some(sheet) = assets
            .sprite(sprite)
            .and_then(|data| Sheet::decode(&data).ok())
            .filter(|sheet| !sheet.frames.is_empty())
        else {
            return false;
        };
        let mut indexed = Indexed::first(&sheet);
        indexed.adjust_palette(self.config.map_scene_jitter);
        indexed.inner = indexed.size;
        for _ in 0..rotation {
            indexed.rotate();
        }
        self.map_scene_sprites.insert(key, indexed);
        true
    }

    /// The colour of a tile outside the map's data.
    fn background(&self, background: i32, x: i32, y: i32) -> i32 {
        if background != -1 {
            background
        } else if (y & 4) == (x & 4) {
            self.overlay_color(self.water_overlay_id + 1)
        } else {
            0x4b5368
        }
    }

    /// `WorldMap.renderMapViewport`: the map tiles `src` (left, top, right,
    /// bottom in map tiles, top being the smaller y) scaled onto the pixels
    /// `dst` (left, top, right, bottom).
    pub(crate) fn render_map_viewport(
        &mut self,
        assets: &dyn Assets,
        raster: &mut Raster<'_>,
        dst: [i32; 4],
        src: [i32; 4],
    ) -> Result<(), String> {
        let range_x = src[2].wrapping_sub(src[0]);
        let range_y = src[3].wrapping_sub(src[1]);
        if range_x == 0 || range_y == 0 {
            return Err("world map: the viewport spans no tiles".into());
        }
        let scale_x = dst[2]
            .wrapping_sub(dst[0])
            .wrapping_shl(16)
            .wrapping_div(range_x);
        let scale_y = dst[3]
            .wrapping_sub(dst[1])
            .wrapping_shl(16)
            .wrapping_div(range_y);
        self.render_map_tiles(
            assets, raster, dst[1], src[3], src[1], src[2], dst[0], src[0], scale_y, scale_x,
        );
        Ok(())
    }

    /// `WorldMap.renderMapTiles`: floors, overlays and walls, then map scenes.
    #[allow(clippy::too_many_arguments)]
    fn render_map_tiles(
        &mut self,
        assets: &dyn Assets,
        raster: &mut Raster<'_>,
        dst_y: i32,
        src_bottom: i32,
        src_top: i32,
        src_right: i32,
        dst_x: i32,
        src_left: i32,
        scale_y: i32,
        scale_x: i32,
    ) {
        let mut tiles_x = src_right.wrapping_sub(src_left);
        let mut tiles_y = src_bottom.wrapping_sub(src_top);
        if self.width > src_right {
            tiles_x = tiles_x.wrapping_add(1);
        }
        if self.length > src_bottom {
            tiles_y = tiles_y.wrapping_add(1);
        }
        let background = self.current_map().map_or(-1, |map| map.background_color);
        let (cols, rows) = (
            self.blended_underlay_colors.cols,
            self.blended_underlay_colors.rows,
        );
        for tx in 0..tiles_x {
            let mut pixel_left = tx.wrapping_mul(scale_x) >> 16;
            let mut pixel_right = tx.wrapping_add(1).wrapping_mul(scale_x) >> 16;
            let tile_w = pixel_right.wrapping_sub(pixel_left);
            if tile_w <= 0 {
                continue;
            }
            let chunk_x = tx.wrapping_add(src_left) >> 6;
            pixel_left = pixel_left.wrapping_add(dst_x);
            if chunk_x < 0 || cols - 1 < chunk_x {
                for fy in 0..tiles_y {
                    let mut color = self.background(
                        background,
                        tx.wrapping_add(src_left),
                        fy.wrapping_add(src_top),
                    );
                    if color == 0 {
                        color = 1;
                    }
                    let top = (scale_y.wrapping_mul(fy) >> 16).wrapping_add(dst_y);
                    let bottom = dst_y.wrapping_add(fy.wrapping_add(1).wrapping_mul(scale_y) >> 16);
                    raster.fill_rect(pixel_left, top, tile_w, bottom.wrapping_sub(top), color);
                }
                continue;
            }
            pixel_right = pixel_right.wrapping_add(dst_x);
            for ty in 0..tiles_y {
                let mut top = scale_y.wrapping_mul(ty) >> 16;
                let mut bottom = ty.wrapping_add(1).wrapping_mul(scale_y) >> 16;
                let tile_h = bottom.wrapping_sub(top);
                if tile_h <= 0 {
                    continue;
                }
                bottom = bottom.wrapping_add(dst_y);
                let chunk_y = src_top.wrapping_add(ty) >> 6;
                let local_y = src_top.wrapping_add(ty) & 0x3f;
                top = top.wrapping_add(dst_y);
                let local_x = tx.wrapping_add(src_left) & 0x3f;
                let index = ((local_y << 6) + local_x) as usize;
                let mut color = match self.blended_underlay_colors.chunk(chunk_x, chunk_y) {
                    Some(chunk) => chunk[index],
                    None => {
                        let mut color = self.background(
                            background,
                            src_left.wrapping_add(tx),
                            ty.wrapping_add(src_top),
                        );
                        if chunk_y < 0 || chunk_y > rows - 1 {
                            if color == 0 {
                                color = 1;
                            }
                            raster.fill_rect(pixel_left, top, tile_w, tile_h, color);
                            continue;
                        }
                        color
                    }
                };
                let mut overlay = self
                    .overlay_ids
                    .chunk(chunk_x, chunk_y)
                    .map_or(0, |chunk| self.overlay_color(i32::from(chunk[index])));
                if color == 0 {
                    color = 1;
                }
                let mut secondary = self
                    .secondary_overlay_ids
                    .chunk(chunk_x, chunk_y)
                    .map_or(0, |chunk| self.overlay_color(i32::from(chunk[index])));
                if overlay == 0 && secondary == 0 {
                    raster.fill_rect(pixel_left, top, tile_w, tile_h, color);
                } else {
                    if overlay != 0 {
                        if overlay == -1 {
                            overlay = 1;
                        }
                        let shape_data = self
                            .overlay_shapes
                            .chunk(chunk_x, chunk_y)
                            .map_or(0, |chunk| i32::from(chunk[index] as i8));
                        let shape = shape_data & 0xfc;
                        if shape == 0 || tile_w <= 1 || tile_h <= 1 {
                            raster.fill_rect(pixel_left, top, tile_w, tile_h, overlay);
                        } else {
                            raster.fill_overlay_shape(
                                overlay,
                                pixel_left,
                                shape_data & 3,
                                color,
                                shape >> 2,
                                tile_h,
                                tile_w,
                                top,
                                true,
                            );
                        }
                    }
                    if secondary != 0 {
                        if secondary == -1 {
                            secondary = color;
                        }
                        let shape_data = self
                            .secondary_overlay_shapes
                            .chunk(chunk_x, chunk_y)
                            .map_or(0, |chunk| i32::from(chunk[index] as i8));
                        let shape = shape_data & 0xfc;
                        if shape == 0 || tile_w <= 1 || tile_h <= 1 {
                            raster.fill_rect(pixel_left, top, tile_w, tile_h, secondary);
                        }
                        // Not an else: the shape is filled again, with black
                        // beside it unless a primary overlay was drawn.
                        raster.fill_overlay_shape(
                            secondary,
                            pixel_left,
                            shape_data & 3,
                            0,
                            shape >> 2,
                            tile_h,
                            tile_w,
                            top,
                            overlay == 0,
                        );
                    }
                }
                let Some(walls) = self.wall_data.chunk(chunk_x, chunk_y) else {
                    continue;
                };
                let mut wall = i32::from(walls[index]);
                if wall == 0 {
                    continue;
                }
                let right = if tile_w == 1 {
                    pixel_left
                } else {
                    pixel_right - 1
                };
                let wall_bottom = if tile_h == 1 { top } else { bottom - 1 };
                let mut wall_color = 0xcccccc;
                if (5..=8).contains(&wall)
                    || (13..=16).contains(&wall)
                    || (21..=24).contains(&wall)
                    || wall == 27
                    || wall == 28
                {
                    wall_color = 0xcc0000;
                    wall -= 4;
                }
                match wall {
                    1 => raster.draw_vertical_line(pixel_left, top, tile_h, wall_color),
                    2 => raster.draw_horizontal_line(pixel_left, top, tile_w, wall_color),
                    3 => raster.draw_vertical_line(right, top, tile_h, wall_color),
                    4 => raster.draw_horizontal_line(pixel_left, wall_bottom, tile_w, wall_color),
                    9 => {
                        raster.draw_vertical_line(pixel_left, top, tile_h, 0xffffff);
                        raster.draw_horizontal_line(pixel_left, top, tile_w, wall_color);
                    }
                    10 => {
                        raster.draw_vertical_line(right, top, tile_h, 0xffffff);
                        raster.draw_horizontal_line(pixel_left, top, tile_w, wall_color);
                    }
                    11 => {
                        raster.draw_vertical_line(right, top, tile_h, 0xffffff);
                        raster.draw_horizontal_line(pixel_left, wall_bottom, tile_w, wall_color);
                    }
                    12 => {
                        raster.draw_vertical_line(pixel_left, top, tile_h, 0xffffff);
                        raster.draw_horizontal_line(pixel_left, wall_bottom, tile_w, wall_color);
                    }
                    17 => raster.draw_horizontal_line(pixel_left, top, 1, wall_color),
                    18 => raster.draw_horizontal_line(right, top, 1, wall_color),
                    19 => raster.draw_horizontal_line(right, wall_bottom, 1, wall_color),
                    20 => raster.draw_horizontal_line(pixel_left, wall_bottom, 1, wall_color),
                    25 => {
                        for dy in 0..tile_h {
                            raster.draw_horizontal_line(
                                dy + pixel_left,
                                wall_bottom - dy,
                                1,
                                wall_color,
                            );
                        }
                    }
                    26 => {
                        for dy in 0..tile_h {
                            raster.draw_horizontal_line(dy + pixel_left, top + dy, 1, wall_color);
                        }
                    }
                    _ => {}
                }
            }
        }
        let (cols, rows) = (self.scenery.cols, self.scenery.rows);
        for tx in -2..tiles_x.wrapping_add(2) {
            let mut pixel_left = tx.wrapping_mul(scale_x) >> 16;
            let pixel_right = scale_x.wrapping_mul(tx.wrapping_add(1)) >> 16;
            let tile_w = pixel_right.wrapping_sub(pixel_left);
            if tile_w <= 0 {
                continue;
            }
            pixel_left = pixel_left.wrapping_add(dst_x);
            let chunk_x = src_left.wrapping_add(tx) >> 6;
            if chunk_x < 0 || cols - 1 < chunk_x {
                continue;
            }
            for ty in -2..tiles_y.wrapping_add(2) {
                let mut pixel_top = ty.wrapping_mul(scale_y) >> 16;
                let pixel_bottom = ty.wrapping_add(1).wrapping_mul(scale_y) >> 16;
                let tile_h = pixel_bottom.wrapping_sub(pixel_top);
                if tile_h <= 0 {
                    continue;
                }
                pixel_top = pixel_top.wrapping_add(dst_y);
                let chunk_y = ty.wrapping_add(src_top) >> 6;
                if chunk_y < 0 || chunk_y > rows - 1 {
                    continue;
                }
                let index =
                    ((src_top.wrapping_add(ty) & 0x3f) << 6) + (tx.wrapping_add(src_left) & 0x3f);
                let Some(packed) = self
                    .scenery
                    .chunk(chunk_x, chunk_y)
                    .map(|chunk| chunk[index as usize])
                else {
                    continue;
                };
                let scene_id = packed & 0x3fff;
                if scene_id == 0 {
                    continue;
                }
                let angle = packed >> 14 & 3;
                let scene = self.map_scene(assets, scene_id - 1);
                if !self.ensure_map_scene_sprite(assets, scene.sprite, angle) {
                    continue;
                }
                let sprite = &self.map_scene_sprites[&(angle << 16 | scene.sprite)];
                let mut draw_h = tile_h.wrapping_mul(sprite.size[1]) / 4;
                let mut draw_w = tile_w.wrapping_mul(sprite.size[0]) / 4;
                if scene.stretch {
                    let mut across = packed >> 16 & 0xf;
                    let mut down = packed >> 20 & 0xf;
                    if angle & 1 == 1 {
                        std::mem::swap(&mut across, &mut down);
                    }
                    draw_w = tile_w.wrapping_mul(across);
                    draw_h = tile_h.wrapping_mul(down);
                }
                if draw_w != 0 && draw_h != 0 {
                    render_scaled(
                        sprite,
                        raster,
                        pixel_left,
                        pixel_top.wrapping_add(tile_h).wrapping_sub(draw_h),
                        draw_w,
                        draw_h,
                        scene.tint as i32,
                    );
                }
            }
        }
    }

    /// `WorldMap.renderMapIcons` and `renderMapIconSprites`: the map-function
    /// icons, and the pulsing circles behind the highlighted ones.
    fn render_map_icons(&mut self, raster: &mut Raster<'_>, dst: [i32; 4], src: [i32; 4]) {
        let range_x = src[2].wrapping_sub(src[0]);
        let range_y = src[3].wrapping_sub(src[1]);
        if range_x == 0 || range_y == 0 {
            return;
        }
        let scale_x = dst[2]
            .wrapping_sub(dst[0])
            .wrapping_shl(16)
            .wrapping_div(range_x);
        let scale_y = dst[3]
            .wrapping_sub(dst[1])
            .wrapping_shl(16)
            .wrapping_div(range_y);
        if scale_x == 0 || scale_y == 0 {
            return;
        }
        let (dst_left, dst_top, src_left, src_top) = (dst[0], dst[1], src[0], src[1]);
        let mut highlight_alpha = -1;
        if self.highlight_pulse_count > 0 {
            highlight_alpha = if self.highlight_pulse_counter <= 10 {
                self.highlight_pulse_counter.wrapping_mul(5)
            } else {
                50 - (self.highlight_pulse_counter - 10).wrapping_mul(5)
            };
        }
        let margin_x = 983040 / scale_x;
        let margin_y = 983040 / scale_y;
        for tx in margin_x.wrapping_neg()..range_x.wrapping_add(margin_x) {
            let mut pixel_left = tx.wrapping_mul(scale_x) >> 16;
            let pixel_right = scale_x.wrapping_mul(tx.wrapping_add(1)) >> 16;
            if pixel_right.wrapping_sub(pixel_left) <= 0 {
                continue;
            }
            let chunk_x = src_left.wrapping_add(tx) >> 6;
            pixel_left = pixel_left.wrapping_add(dst_left);
            for ty in margin_y.wrapping_neg()..range_y.wrapping_add(margin_y) {
                let pixel_bottom = scale_y.wrapping_mul(ty.wrapping_add(1)) >> 16;
                let mut pixel_top = ty.wrapping_mul(scale_y) >> 16;
                if pixel_bottom.wrapping_sub(pixel_top) <= 0 {
                    continue;
                }
                pixel_top = pixel_top.wrapping_add(dst_top);
                let chunk_y = src_top.wrapping_add(ty) >> 6;
                let Some(chunk) = self.map_function_locs.chunk(chunk_x, chunk_y) else {
                    continue;
                };
                let index =
                    (tx.wrapping_add(src_left) & 0x3f) + ((src_top.wrapping_add(ty) & 0x3f) << 6);
                let loc_id = chunk[index as usize];
                if loc_id == 0 {
                    continue;
                }
                let Some(function) = self.locs.get(&(loc_id - 1)).map(|loc| loc.map_function)
                else {
                    continue;
                };
                let Some(sprite) = usize::try_from(function)
                    .ok()
                    .and_then(|i| self.maps.sprites.get(i))
                else {
                    continue;
                };
                if self.maps.is_map_visible(function) {
                    continue;
                }
                if highlight_alpha != -1 && function == self.highlighted_map_function {
                    self.highlighted_elements.push(MapElement {
                        id: function,
                        map_x: pixel_left,
                        map_y: pixel_top,
                    });
                } else {
                    sprite.render(raster, pixel_left - 7, pixel_top - 7);
                }
            }
        }
        for element in std::mem::take(&mut self.highlighted_elements) {
            for radius in [15, 13, 11, 9] {
                raster.fill_circle_alpha(element.map_x, element.map_y, radius, highlight_alpha);
            }
            self.maps.sprites[element.id as usize].render(
                raster,
                element.map_x - 7,
                element.map_y - 7,
            );
        }
    }

    /// `WorldMap.renderMapLabels`.
    fn render_map_labels(&self, raster: &mut Raster<'_>, dst: [i32; 4], src: [i32; 4]) {
        let Some(labels) = &self.labels else {
            return;
        };
        let [dst_left, dst_top, dst_right, dst_bottom] = dst;
        let [src_left, src_top, src_right, src_bottom] = src;
        if src_right == src_left || src_bottom == src_top {
            return;
        }
        let zoom = f64::from(self.zoom);
        for i in 0..labels.count() {
            if !labels.is_text_label(i) {
                continue;
            }
            let at = i as usize;
            let map_x = i32::from(labels.coord_x[at]).wrapping_sub(self.origin_x);
            let map_y = self
                .origin_y
                .wrapping_add(self.length)
                .wrapping_sub(i32::from(labels.coord_y[at]))
                .wrapping_sub(1);
            let screen_x = dst_left.wrapping_add(
                dst_right
                    .wrapping_sub(dst_left)
                    .wrapping_mul(map_x.wrapping_sub(src_left))
                    .wrapping_div(src_right.wrapping_sub(src_left)),
            );
            let size = labels.get_label_size(i);
            let mut screen_y = dst_bottom
                .wrapping_sub(dst_top)
                .wrapping_mul(map_y.wrapping_sub(src_top))
                .wrapping_div(src_bottom.wrapping_sub(src_top))
                .wrapping_add(dst_top);
            let mut color = 0xffffff;
            // Font slots: 11, 12, 14, 17, 19, 22, 26, 30.
            let mut font = None;
            let mut pick = |sizes: [usize; 4]| {
                if zoom == 3.0 {
                    font = Some(sizes[0]);
                }
                if zoom == 4.0 {
                    font = Some(sizes[1]);
                }
                if zoom == 6.0 {
                    font = Some(sizes[2]);
                }
                if zoom >= 8.0 {
                    font = Some(sizes[3]);
                }
            };
            match size {
                0 => pick([0, 1, 2, 3]),
                1 => pick([2, 3, 4, 5]),
                2 => {
                    pick([4, 5, 6, 7]);
                    color = 0xffaa00;
                }
                _ => {}
            }
            if labels.colors[at] != -1 {
                color = labels.colors[at];
            }
            let Some(font) = font.and_then(|slot| self.fonts[slot].as_ref()) else {
                continue;
            };
            let lines = split_label(&labels.names[at]);
            let count = lines.len() as i32;
            screen_y = screen_y.wrapping_sub(font.get_line_height().wrapping_mul(count - 1) / 2);
            screen_y = screen_y.wrapping_add(font.get_ascent() / 2);
            for (line, text) in lines.iter().enumerate() {
                let mut text = text.as_slice();
                if count - 1 > line as i32 {
                    text = &text[..text.len().saturating_sub(4)];
                }
                font.render_string_center(raster, text, screen_x, screen_y, color);
                screen_y = screen_y.wrapping_add(font.get_line_height());
            }
        }
    }

    /// `WorldMap.render` for a client code 1400 component of `width` by
    /// `height`, into `target` (`width * height` 0xRRGGBB pixels, row-major).
    ///
    /// `target` is the Java's `renderBuffer`: it is not cleared here, and the
    /// original allocates it zeroed whenever the component's size changes.
    /// While the map is loading this runs one [`WorldMap::load`] step and
    /// draws the loading box instead.
    pub fn render(
        &mut self,
        assets: &dyn Assets,
        client: &mut Client<'_>,
        target: &mut [u32],
        width: i32,
        height: i32,
    ) -> Result<Rendered, String> {
        let mut rendered = Rendered::default();
        if self.load_percentage < 100 {
            rendered.step.merge(self.load(assets, client)?);
        }
        let mut raster = Raster::new(target, width, height)?;
        if self.load_percentage < 100 {
            let center_x = width / 2;
            let center_y = height / 2 - 18 - 20;
            let filled = self.load_percentage.wrapping_mul(3);
            raster.fill_rect(0, 0, width, height, 0);
            raster.draw_rect(center_x - 152, center_y, 304, 34, 0x8c1111);
            raster.draw_rect(center_x - 151, center_y + 1, 302, 32, 0);
            raster.fill_rect(center_x - 150, center_y + 2, filled, 30, 0x8c1111);
            raster.fill_rect(center_x + filled - 150, center_y + 2, 300 - filled, 30, 0);
            rendered.loading_text = Some([center_x, center_y + 20]);
            return Ok(rendered);
        }
        let across = (width as f32 / self.zoom) as i32;
        let down = (height as f32 / self.zoom) as i32;
        self.viewport_height = (height.wrapping_mul(2) as f32 / self.zoom) as i32;
        self.viewport_x = self.view_x.wrapping_sub(across);
        let src_left = self.view_x.wrapping_sub(across);
        let src_top = self.view_y.wrapping_sub(down);
        self.viewport_y = self.view_y.wrapping_sub(down);
        let src_bottom = self.view_y.wrapping_add(down);
        let src_right = across.wrapping_add(self.view_x);
        self.viewport_width = (width.wrapping_mul(2) as f32 / self.zoom) as i32;
        let dst = [0, 0, width, height];
        let src = [src_left, src_top, src_right, src_bottom];
        self.render_map_viewport(assets, &mut raster, dst, src)?;
        self.render_map_icons(&mut raster, dst, src);
        self.render_map_labels(&mut raster, dst, src);
        if self.highlight_pulse_count > 0 {
            self.highlight_pulse_counter = self.highlight_pulse_counter.wrapping_sub(1);
            if self.highlight_pulse_counter == 0 {
                self.highlight_pulse_counter = 20;
                self.highlight_pulse_count -= 1;
            }
        }
        Ok(rendered)
    }

    /// The cached `overviewSprite`: the whole map scaled to `width` by
    /// `height`, redrawn when the size changes or after `clear`. None until
    /// the map is loaded.
    pub fn overview(
        &mut self,
        assets: &dyn Assets,
        width: i32,
        height: i32,
    ) -> Result<Option<&Sprite>, String> {
        if self.load_percentage < 100 {
            return Ok(None);
        }
        if self
            .overview
            .as_ref()
            .is_none_or(|sprite| sprite.width != width || sprite.height != height)
        {
            let mut pixels = vec![0u32; width.max(0) as usize * height.max(0) as usize];
            let mut raster = Raster::new(&mut pixels, width, height)?;
            self.render_map_viewport(
                assets,
                &mut raster,
                [0, 0, width, height],
                [0, 0, self.width, self.length],
            )?;
            self.overview = Some(Sprite {
                x_offset: 0,
                y_offset: 0,
                width,
                height,
                pixels: pixels.into_iter().map(|pixel| pixel as i32).collect(),
            });
        }
        Ok(self.overview.as_ref())
    }

    /// The viewport rectangle on a `width` by `height` overview: x, y, width
    /// and height in overview pixels, from the last `render`.
    pub fn overview_viewport(&self, width: i32, height: i32) -> [i32; 4] {
        if self.width == 0 || self.length == 0 {
            return [0; 4];
        }
        [
            width.wrapping_mul(self.viewport_x).wrapping_div(self.width),
            height
                .wrapping_mul(self.viewport_y)
                .wrapping_div(self.length),
            width
                .wrapping_mul(self.viewport_width)
                .wrapping_div(self.width),
            self.viewport_height
                .wrapping_mul(height)
                .wrapping_div(self.length),
        ]
    }

    /// The overview's highlight dots: their alpha and the centre of each
    /// 4x4 yellow square, or None when nothing is highlighted.
    pub fn overview_highlights(&self, width: i32, height: i32) -> Option<(i32, Vec<[i32; 2]>)> {
        if self.highlight_pulse_count <= 0 || self.width == 0 || self.length == 0 {
            return None;
        }
        let alpha = if self.highlight_pulse_counter > 10 {
            (20 - self.highlight_pulse_counter).wrapping_mul(25)
        } else {
            self.highlight_pulse_counter.wrapping_mul(25)
        };
        let dots = self
            .map_elements
            .iter()
            .filter(|element| element.id == self.highlighted_map_function)
            .map(|element| {
                [
                    width.wrapping_mul(element.map_x).wrapping_div(self.width),
                    element.map_y.wrapping_mul(height).wrapping_div(self.length),
                ]
            })
            .collect();
        Some((alpha, dots))
    }

    /// `Cs1ScriptRunner.renderWorldMapOverview` for a client code 1401
    /// component of `width` by `height`, composed in software into `target`:
    /// black, then once loaded the overview, the translucent viewport frame
    /// (white instead of red when `white_frame`, the Java's `client.game == 1`)
    /// and the highlight dots.
    pub fn render_overview(
        &mut self,
        assets: &dyn Assets,
        target: &mut [u32],
        width: i32,
        height: i32,
        white_frame: bool,
    ) -> Result<(), String> {
        let mut raster = Raster::new(target, width, height)?;
        raster.fill_rect(0, 0, width, height, 0);
        let Some(sprite) = self.overview(assets, width, height)? else {
            return Ok(());
        };
        sprite.draw_pixels(&mut raster, 0, 0);
        let [x, y, w, h] = self.overview_viewport(width, height);
        let color = if white_frame { 0xffffff } else { 0xff0000 };
        raster.fill_rect_alpha(x, y, w, h, color, 128);
        raster.draw_rect(x, y, w, h, color);
        if let Some((alpha, dots)) = self.overview_highlights(width, height) {
            for [dot_x, dot_y] in dots {
                raster.fill_rect_alpha(dot_x - 2, dot_y - 2, 4, 4, 0xffff00, alpha);
            }
        }
        Ok(())
    }
}
