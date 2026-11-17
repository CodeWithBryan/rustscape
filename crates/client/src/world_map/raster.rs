//! The `SoftwareRaster` routines and sprite blits the world map draws with.
//!
//! Offsets wrap like Java ints. A write outside the buffer is skipped where the
//! original would throw `ArrayIndexOutOfBoundsException`.
use crate::minimap::Indexed;

/// `SoftwareRaster`: a pixel buffer with its clip rectangle.
pub struct Raster<'a> {
    pub pixels: &'a mut [u32],
    pub width: i32,
    pub height: i32,
    pub clip_left: i32,
    pub clip_top: i32,
    pub clip_right: i32,
    pub clip_bottom: i32,
}
impl<'a> Raster<'a> {
    /// `SoftwareRaster.setSize`. The buffer must hold `width * height` pixels.
    pub fn new(pixels: &'a mut [u32], width: i32, height: i32) -> Result<Self, String> {
        let needed = usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .and_then(|(w, h)| w.checked_mul(h))
            .ok_or("world map raster: negative size")?;
        if pixels.len() < needed {
            return Err("world map raster: buffer smaller than width * height".into());
        }
        Ok(Self {
            pixels,
            width,
            height,
            clip_left: 0,
            clip_top: 0,
            clip_right: width,
            clip_bottom: height,
        })
    }

    #[inline]
    pub(crate) fn put(&mut self, offset: i32, color: i32) {
        if let Some(pixel) = usize::try_from(offset)
            .ok()
            .and_then(|i| self.pixels.get_mut(i))
        {
            *pixel = color as u32;
        }
    }

    #[inline]
    pub(crate) fn get(&self, offset: i32) -> i32 {
        usize::try_from(offset)
            .ok()
            .and_then(|i| self.pixels.get(i))
            .map_or(0, |&pixel| pixel as i32)
    }

    /// `SoftwareRaster.setClip`.
    pub fn set_clip(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        self.clip_left = left.max(0);
        self.clip_top = top.max(0);
        self.clip_right = right.min(self.width);
        self.clip_bottom = bottom.min(self.height);
    }

    /// `SoftwareRaster.fillRect`.
    pub fn fill_rect(&mut self, mut x: i32, mut y: i32, mut w: i32, mut h: i32, color: i32) {
        if x < self.clip_left {
            w = w.wrapping_sub(self.clip_left.wrapping_sub(x));
            x = self.clip_left;
        }
        if y < self.clip_top {
            h = h.wrapping_sub(self.clip_top.wrapping_sub(y));
            y = self.clip_top;
        }
        if x.wrapping_add(w) > self.clip_right {
            w = self.clip_right.wrapping_sub(x);
        }
        if y.wrapping_add(h) > self.clip_bottom {
            h = self.clip_bottom.wrapping_sub(y);
        }
        let stride = self.width.wrapping_sub(w);
        let mut offset = x.wrapping_add(y.wrapping_mul(self.width));
        for _ in 0..h.max(0) {
            for _ in 0..w.max(0) {
                self.put(offset, color);
                offset = offset.wrapping_add(1);
            }
            offset = offset.wrapping_add(stride);
        }
    }

    /// `SoftwareRaster.fillRectAlpha`.
    pub fn fill_rect_alpha(
        &mut self,
        mut x: i32,
        mut y: i32,
        mut w: i32,
        mut h: i32,
        color: i32,
        alpha: i32,
    ) {
        if x < self.clip_left {
            w = w.wrapping_sub(self.clip_left.wrapping_sub(x));
            x = self.clip_left;
        }
        if y < self.clip_top {
            h = h.wrapping_sub(self.clip_top.wrapping_sub(y));
            y = self.clip_top;
        }
        if x.wrapping_add(w) > self.clip_right {
            w = self.clip_right.wrapping_sub(x);
        }
        if y.wrapping_add(h) > self.clip_bottom {
            h = self.clip_bottom.wrapping_sub(y);
        }
        let source = (((color & 0xff00ff).wrapping_mul(alpha) >> 8) & 0xff00ff)
            .wrapping_add(((color & 0xff00).wrapping_mul(alpha) >> 8) & 0xff00);
        let inverse = 256i32.wrapping_sub(alpha);
        let stride = self.width.wrapping_sub(w);
        let mut offset = x.wrapping_add(y.wrapping_mul(self.width));
        for _ in 0..h.max(0) {
            for _ in 0..w.max(0) {
                let dst = self.get(offset);
                let blended = (((dst & 0xff00ff).wrapping_mul(inverse) >> 8) & 0xff00ff)
                    .wrapping_add(((dst & 0xff00).wrapping_mul(inverse) >> 8) & 0xff00);
                self.put(offset, source.wrapping_add(blended));
                offset = offset.wrapping_add(1);
            }
            offset = offset.wrapping_add(stride);
        }
    }

    /// `SoftwareRaster.drawRect`.
    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: i32) {
        self.draw_horizontal_line(x, y, w, color);
        self.draw_horizontal_line(x, y.wrapping_add(h).wrapping_sub(1), w, color);
        self.draw_vertical_line(x, y, h, color);
        self.draw_vertical_line(x.wrapping_add(w).wrapping_sub(1), y, h, color);
    }

    /// `SoftwareRaster.drawHorizontalLine`.
    pub fn draw_horizontal_line(&mut self, mut x: i32, y: i32, mut length: i32, color: i32) {
        if y < self.clip_top || y >= self.clip_bottom {
            return;
        }
        if x < self.clip_left {
            length = length.wrapping_sub(self.clip_left.wrapping_sub(x));
            x = self.clip_left;
        }
        if x.wrapping_add(length) > self.clip_right {
            length = self.clip_right.wrapping_sub(x);
        }
        let offset = x.wrapping_add(y.wrapping_mul(self.width));
        for i in 0..length.max(0) {
            self.put(offset.wrapping_add(i), color);
        }
    }

    /// `SoftwareRaster.drawVerticalLine`.
    pub fn draw_vertical_line(&mut self, x: i32, mut y: i32, mut length: i32, color: i32) {
        if x < self.clip_left || x >= self.clip_right {
            return;
        }
        if y < self.clip_top {
            length = length.wrapping_sub(self.clip_top.wrapping_sub(y));
            y = self.clip_top;
        }
        if y.wrapping_add(length) > self.clip_bottom {
            length = self.clip_bottom.wrapping_sub(y);
        }
        let offset = x.wrapping_add(y.wrapping_mul(self.width));
        for i in 0..length.max(0) {
            self.put(offset.wrapping_add(i.wrapping_mul(self.width)), color);
        }
    }

    fn draw_dot(&mut self, x: i32, y: i32) {
        if x >= self.clip_left && y >= self.clip_top && x < self.clip_right && y < self.clip_bottom
        {
            self.put(x.wrapping_add(y.wrapping_mul(self.width)), 0xffff00);
        }
    }

    /// `SoftwareRaster.fillCircleAlpha`: a yellow disc. An alpha of 256 takes
    /// the original's opaque `fillCircle` path, whose lower half differs.
    pub fn fill_circle_alpha(&mut self, x: i32, mut y: i32, mut radius: i32, alpha: i32) {
        if alpha == 0 {
            return;
        }
        let opaque = alpha == 256;
        if opaque {
            if radius == 0 {
                self.draw_dot(x, y);
                return;
            }
            if radius < 0 {
                radius = -radius;
            }
        }
        let inverse = 256 - alpha;
        let source = [alpha.wrapping_mul(255), alpha.wrapping_mul(255), 0];
        let mut start_y = y.wrapping_sub(radius);
        if start_y < self.clip_top {
            start_y = self.clip_top;
        }
        let mut end_y = y.wrapping_add(radius).wrapping_add(1);
        if end_y > self.clip_bottom {
            end_y = self.clip_bottom;
        }
        let mut cur_y = start_y;
        let radius_sq = radius.wrapping_mul(radius);
        let mut half_width = 0i32;
        let mut dy = y.wrapping_sub(start_y);
        let mut dy_sq = dy.wrapping_mul(dy);
        let mut inner_dy_sq = dy_sq.wrapping_sub(dy);
        if y > end_y {
            y = end_y;
        }
        let span = |raster: &mut Self, left: i32, right: i32, row: i32, inclusive: bool| {
            let mut offset = left.wrapping_add(row.wrapping_mul(raster.width));
            let mut px = left;
            while px < right || (inclusive && px == right) {
                if opaque {
                    raster.put(offset, 0xffff00);
                } else {
                    let dst = raster.get(offset);
                    let r = (dst >> 16 & 0xff).wrapping_mul(inverse);
                    let g = (dst >> 8 & 0xff).wrapping_mul(inverse);
                    let b = (dst & 0xff).wrapping_mul(inverse);
                    raster.put(
                        offset,
                        ((source[0].wrapping_add(r) >> 8) << 16)
                            .wrapping_add((source[1].wrapping_add(g) >> 8) << 8)
                            .wrapping_add(source[2].wrapping_add(b) >> 8),
                    );
                }
                offset = offset.wrapping_add(1);
                px += 1;
            }
        };
        while cur_y < y {
            while inner_dy_sq <= radius_sq || dy_sq <= radius_sq {
                dy_sq = dy_sq.wrapping_add(half_width.wrapping_add(half_width));
                // innerDySq += halfWidth++ + halfWidth
                inner_dy_sq = inner_dy_sq.wrapping_add(half_width.wrapping_mul(2).wrapping_add(1));
                half_width = half_width.wrapping_add(1);
            }
            let mut left = x.wrapping_add(1).wrapping_sub(half_width);
            if left < self.clip_left {
                left = self.clip_left;
            }
            let mut right = x.wrapping_add(half_width);
            if right > self.clip_right {
                right = self.clip_right;
            }
            span(self, left, right, cur_y, false);
            cur_y += 1;
            // dySq -= dy-- + dy; innerDySq -= dy + dy
            dy_sq = dy_sq.wrapping_sub(dy.wrapping_mul(2).wrapping_sub(1));
            dy = dy.wrapping_sub(1);
            inner_dy_sq = inner_dy_sq.wrapping_sub(dy.wrapping_add(dy));
        }
        half_width = radius;
        // fillCircle recomputes the row distance; fillCircleAlpha negates it.
        dy = if opaque {
            cur_y.wrapping_sub(y)
        } else {
            dy.wrapping_neg()
        };
        inner_dy_sq = dy.wrapping_mul(dy).wrapping_add(radius_sq);
        dy_sq = inner_dy_sq.wrapping_sub(radius);
        inner_dy_sq = inner_dy_sq.wrapping_sub(dy);
        while cur_y < end_y {
            while inner_dy_sq > radius_sq && dy_sq > radius_sq {
                // innerDySq -= halfWidth-- + halfWidth
                inner_dy_sq = inner_dy_sq.wrapping_sub(half_width.wrapping_mul(2).wrapping_sub(1));
                half_width = half_width.wrapping_sub(1);
                dy_sq = dy_sq.wrapping_sub(half_width.wrapping_add(half_width));
            }
            let mut left = x.wrapping_sub(half_width);
            if left < self.clip_left {
                left = self.clip_left;
            }
            let mut right = x.wrapping_add(half_width);
            if right > self.clip_right - 1 {
                right = self.clip_right - 1;
            }
            span(self, left, right, cur_y, true);
            cur_y += 1;
            inner_dy_sq = inner_dy_sq.wrapping_add(dy.wrapping_add(dy));
            // dySq += dy++ + dy
            dy_sq = dy_sq.wrapping_add(dy.wrapping_mul(2).wrapping_add(1));
            dy = dy.wrapping_add(1);
        }
    }

    /// `WorldMap.fillOverlayShape`: an overlay tile of `shape` 1..=11 at
    /// `rotation`. Pixels outside the overlay take `underlay_color` when
    /// `fill_underlay` is set and are left alone otherwise.
    ///
    /// The original spells out every shape and rotation as its own pair of
    /// loops. Each pair walks the clipped rectangle left to right and top to
    /// bottom while counting its row or column backwards for mirrored cases,
    /// so one loop over the same positions with the mirrored coordinates
    /// writes the same pixels in the same order.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_overlay_shape(
        &mut self,
        overlay_color: i32,
        x: i32,
        mut rotation: i32,
        underlay_color: i32,
        mut shape: i32,
        h: i32,
        w: i32,
        y: i32,
        fill_underlay: bool,
    ) {
        let mut clipped_x = x;
        if self.clip_right <= x {
            return;
        }
        if x < self.clip_left {
            clipped_x = self.clip_left;
        }
        let mut right = w.wrapping_add(x);
        if self.clip_left >= right {
            return;
        }
        if self.clip_right < right {
            right = self.clip_right;
        }
        let mut clipped_y = y;
        if self.clip_bottom <= y {
            return;
        }
        let mut bottom = y.wrapping_add(h);
        if y < self.clip_top {
            clipped_y = self.clip_top;
        }
        if bottom <= self.clip_top {
            return;
        }
        let mut offset = clipped_x.wrapping_add(self.width.wrapping_mul(clipped_y));
        if shape == 9 {
            rotation = (rotation + 1) & 3;
            shape = 1;
        }
        let stride = clipped_x.wrapping_add(self.width).wrapping_sub(right);
        clipped_y = clipped_y.wrapping_sub(y);
        if self.clip_bottom < bottom {
            bottom = self.clip_bottom;
        }
        if shape == 10 {
            rotation = (rotation + 3) & 3;
            shape = 1;
        }
        clipped_x = clipped_x.wrapping_sub(x);
        if shape == 11 {
            rotation = (rotation + 3) & 3;
            shape = 8;
        }
        right = right.wrapping_sub(x);
        bottom = bottom.wrapping_sub(y);
        // (rows counted backwards, columns counted backwards)
        let (mirror_rows, mirror_cols) = match (shape, rotation) {
            (1, 1 | 3) | (2, 0) | (3, 1) | (4, 0) | (5, 1) | (7, 1) | (8, 1) => (true, false),
            (2, 2) | (3, 3) | (4, 2) | (5, 3) | (7, 3) | (8, 3) => (false, true),
            (2, 3) | (3, 0) | (4, 3) | (5, 0) | (7, 2) | (8, 2) => (true, true),
            (1..=8, 0..=3) => (false, false),
            _ => return,
        };
        let half_w = w / 2;
        let half_h = h / 2;
        for line in clipped_y..bottom {
            let row = if mirror_rows { h - 1 - line } else { line };
            for position in clipped_x..right {
                let col = if mirror_cols {
                    w - 1 - position
                } else {
                    position
                };
                let overlay = match (shape, rotation) {
                    (1, 0) => col <= row,
                    (1, 1) => row >= col,
                    (1, 2) => col >= row,
                    (1, 3) => col >= row,
                    (2, 0) => row >> 1 >= col,
                    (2, 1) => row << 1 <= col,
                    (2, 2) => row >> 1 >= col,
                    (2, 3) => row << 1 <= col,
                    (3, 0) => row >> 1 >= col,
                    (3, 1) => col >= row << 1,
                    (3, 2) => col <= row >> 1,
                    (3, 3) => row << 1 <= col,
                    (4, 0) => row >> 1 <= col,
                    (4, 1) => row << 1 >= col,
                    (4, 2) => col >= row >> 1,
                    (4, 3) => col <= row << 1,
                    (5, 0) => row >> 1 <= col,
                    (5, 1) => col <= row << 1,
                    (5, 2) => col >= row >> 1,
                    (5, 3) => row << 1 >= col,
                    (6, 0) => col <= half_w,
                    (6, 1) => row <= half_h,
                    (6, 2) => col >= half_w,
                    (6, 3) => row >= half_h,
                    (7, 0) => col <= row - half_h,
                    (7, 1) => row - half_h >= col,
                    (7, 2) => col <= row - half_h,
                    (7, 3) => row - half_h >= col,
                    (8, 0) => row - half_h <= col,
                    (8, 1) => row - half_h <= col,
                    (8, 2) => col >= row - half_h,
                    _ => row - half_h <= col,
                };
                if overlay {
                    self.put(offset, overlay_color);
                } else if fill_underlay {
                    self.put(offset, underlay_color);
                }
                offset = offset.wrapping_add(1);
            }
            offset = offset.wrapping_add(stride);
        }
    }
}

/// `SoftwareSprite`: a map-function icon. Zero pixels are transparent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sprite {
    pub x_offset: i32,
    pub y_offset: i32,
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<i32>,
}
impl Sprite {
    /// `SoftwareSprite.adjustRgb(red, green, blue)`.
    pub fn adjust_rgb(&mut self, [red, green, blue]: [i32; 3]) {
        for pixel in &mut self.pixels {
            if *pixel != 0 {
                let r = ((*pixel >> 16 & 0xff) + red).clamp(1, 255);
                let g = ((*pixel >> 8 & 0xff) + green).clamp(1, 255);
                let b = ((*pixel & 0xff) + blue).clamp(1, 255);
                *pixel = (r << 16) + (g << 8) + b;
            }
        }
    }

    /// `SoftwareSprite.render` (`blitOpaque`).
    pub fn render(&self, raster: &mut Raster<'_>, x: i32, y: i32) {
        self.blit(raster, x, y, false);
    }

    /// `SoftwareSprite.drawPixels` (`blitAllPixels`): zero pixels are copied too.
    pub fn draw_pixels(&self, raster: &mut Raster<'_>, x: i32, y: i32) {
        self.blit(raster, x, y, true);
    }

    fn blit(&self, raster: &mut Raster<'_>, mut x: i32, mut y: i32, all: bool) {
        x = x.wrapping_add(self.x_offset);
        y = y.wrapping_add(self.y_offset);
        let mut dst = x.wrapping_add(y.wrapping_mul(raster.width));
        let mut src = 0i32;
        let mut h = self.height;
        let mut w = self.width;
        let mut dst_step = raster.width.wrapping_sub(w);
        let mut src_step = 0i32;
        if y < raster.clip_top {
            let clip = raster.clip_top.wrapping_sub(y);
            h = h.wrapping_sub(clip);
            y = raster.clip_top;
            src = clip.wrapping_mul(w);
            dst = dst.wrapping_add(clip.wrapping_mul(raster.width));
        }
        if y.wrapping_add(h) > raster.clip_bottom {
            h = h.wrapping_sub(y.wrapping_add(h).wrapping_sub(raster.clip_bottom));
        }
        if x < raster.clip_left {
            let clip = raster.clip_left.wrapping_sub(x);
            w = w.wrapping_sub(clip);
            x = raster.clip_left;
            src = src.wrapping_add(clip);
            dst = dst.wrapping_add(clip);
            src_step = clip;
            dst_step = dst_step.wrapping_add(clip);
        }
        if x.wrapping_add(w) > raster.clip_right {
            let clip = x.wrapping_add(w).wrapping_sub(raster.clip_right);
            w = w.wrapping_sub(clip);
            src_step = src_step.wrapping_add(clip);
            dst_step = dst_step.wrapping_add(clip);
        }
        if w <= 0 || h <= 0 {
            return;
        }
        for _ in 0..h {
            for _ in 0..w {
                let pixel = usize::try_from(src)
                    .ok()
                    .and_then(|i| self.pixels.get(i))
                    .copied()
                    .unwrap_or(0);
                if all || pixel != 0 {
                    raster.put(dst, pixel);
                }
                src = src.wrapping_add(1);
                dst = dst.wrapping_add(1);
            }
            dst = dst.wrapping_add(dst_step);
            src = src.wrapping_add(src_step);
        }
    }
}

/// `SoftwareIndexedSprite.renderScaled` and, with a nonzero `tint`,
/// `renderScaledTinted`. A zero step, which the original divides by, draws nothing.
pub fn render_scaled(
    sprite: &Indexed,
    raster: &mut Raster<'_>,
    mut x: i32,
    mut y: i32,
    mut scaled_width: i32,
    mut scaled_height: i32,
    tint: i32,
) {
    if scaled_width == 0 || scaled_height == 0 {
        return;
    }
    let [sprite_width, sprite_height] = sprite.size;
    let mut src_x = 0i32;
    let mut src_y = 0i32;
    let [inner_w, inner_h] = sprite.inner;
    let x_step = inner_w.wrapping_shl(16).wrapping_div(scaled_width);
    let y_step = inner_h.wrapping_shl(16).wrapping_div(scaled_height);
    if x_step == 0 || y_step == 0 {
        return;
    }
    let [x_offset, y_offset] = sprite.offset;
    if x_offset > 0 {
        let clip = (x_offset
            .wrapping_shl(16)
            .wrapping_add(x_step)
            .wrapping_sub(1))
        .wrapping_div(x_step);
        x = x.wrapping_add(clip);
        src_x = clip
            .wrapping_mul(x_step)
            .wrapping_sub(x_offset.wrapping_shl(16));
    }
    if y_offset > 0 {
        let clip = (y_offset
            .wrapping_shl(16)
            .wrapping_add(y_step)
            .wrapping_sub(1))
        .wrapping_div(y_step);
        y = y.wrapping_add(clip);
        src_y = clip
            .wrapping_mul(y_step)
            .wrapping_sub(y_offset.wrapping_shl(16));
    }
    if sprite_width < inner_w {
        scaled_width = (sprite_width << 16)
            .wrapping_add(x_step)
            .wrapping_sub(src_x)
            .wrapping_sub(1)
            / x_step;
    }
    if sprite_height < inner_h {
        scaled_height = (sprite_height << 16)
            .wrapping_add(y_step)
            .wrapping_sub(src_y)
            .wrapping_sub(1)
            / y_step;
    }
    let mut dst = x.wrapping_add(y.wrapping_mul(raster.width));
    let mut row_pad = raster.width.wrapping_sub(scaled_width);
    if y.wrapping_add(scaled_height) > raster.clip_bottom {
        scaled_height = scaled_height.wrapping_sub(
            y.wrapping_add(scaled_height)
                .wrapping_sub(raster.clip_bottom),
        );
    }
    if y < raster.clip_top {
        let clipped = raster.clip_top.wrapping_sub(y);
        scaled_height = scaled_height.wrapping_sub(clipped);
        dst = dst.wrapping_add(clipped.wrapping_mul(raster.width));
        src_y = src_y.wrapping_add(y_step.wrapping_mul(clipped));
    }
    if x.wrapping_add(scaled_width) > raster.clip_right {
        let clipped = x.wrapping_add(scaled_width).wrapping_sub(raster.clip_right);
        scaled_width = scaled_width.wrapping_sub(clipped);
        row_pad = row_pad.wrapping_add(clipped);
    }
    if x < raster.clip_left {
        let clipped = raster.clip_left.wrapping_sub(x);
        scaled_width = scaled_width.wrapping_sub(clipped);
        dst = dst.wrapping_add(clipped);
        src_x = src_x.wrapping_add(x_step.wrapping_mul(clipped));
        row_pad = row_pad.wrapping_add(clipped);
    }
    let reset = src_x;
    let tint_r = tint >> 16 & 0xff;
    let tint_g = tint >> 8 & 0xff;
    let tint_b = tint & 0xff;
    for _ in 0..scaled_height.max(0) {
        let row = (src_y >> 16).wrapping_mul(sprite_width);
        for _ in 0..scaled_width.max(0) {
            let index = usize::try_from((src_x >> 16).wrapping_add(row))
                .ok()
                .and_then(|i| sprite.indices.get(i))
                .copied()
                .unwrap_or(0);
            if index != 0 {
                let color = sprite.palette.get(index as usize).copied().unwrap_or(0) as i32;
                if tint == 0 {
                    raster.put(dst, color);
                } else {
                    let r = color >> 16 & 0xff;
                    let g = color >> 8 & 0xff;
                    let b = color & 0xff;
                    raster.put(
                        dst,
                        (((r * tint_r) >> 8) << 16)
                            + (((g * tint_g) >> 8) << 8)
                            + ((b * tint_b) >> 8),
                    );
                }
            }
            dst = dst.wrapping_add(1);
            src_x = src_x.wrapping_add(x_step);
        }
        src_y = src_y.wrapping_add(y_step);
        src_x = reset;
        dst = dst.wrapping_add(row_pad);
    }
}
