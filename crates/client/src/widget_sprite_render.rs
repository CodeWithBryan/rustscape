//! Prepared original widget images on Macroquad's existing graphics backend.
use crate::display_host::{screen_height, screen_width};
use macroquad::prelude::*;
use rs09_client::ui::{
    Component,
    widget_sprite::{self, Image, Options},
};
use std::cell::Cell;

pub fn material() -> Result<Material, String> {
    // The original glColor4f uses alpha / 256. Carry that float separately
    // from Macroquad's byte color so half opacity stays exactly 0.5.
    load_material(
        ShaderSource::Glsl {
            vertex: r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 normal;
uniform mat4 Model;
uniform mat4 Projection;
varying mediump vec2 uv;
varying mediump float opacity;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    uv = texcoord;
    opacity = normal.x;
}"#,
            fragment: r#"#version 100
precision mediump float;
uniform sampler2D Texture;
varying mediump vec2 uv;
varying mediump float opacity;
void main() {
    vec4 pixel = texture2D(Texture, uv);
    pixel.a *= opacity;
    if (pixel.a <= 0.0) discard;
    gl_FragColor = pixel;
}"#,
        },
        MaterialParams {
            pipeline_params: miniquad::PipelineParams {
                color_blend: Some(miniquad::BlendState::new(
                    miniquad::Equation::Add,
                    miniquad::BlendFactor::Value(miniquad::BlendValue::SourceAlpha),
                    miniquad::BlendFactor::OneMinusValue(miniquad::BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}
pub struct Sprite {
    material: Material,
    texture: Option<Texture2D>,
    filter: Cell<FilterMode>,
    inner: [usize; 2],
    offset: [usize; 2],
    size: [usize; 2],
    #[cfg(not(target_arch = "wasm32"))]
    id: i32,
    #[cfg(not(target_arch = "wasm32"))]
    options: Options,
    #[cfg(not(target_arch = "wasm32"))]
    alpha: bool,
    #[cfg(not(target_arch = "wasm32"))]
    expected_pixels: String,
}
#[cfg(not(target_arch = "wasm32"))]
fn checksum(bytes: &[u8]) -> String {
    format!(
        "{:016x}",
        bytes
            .iter()
            .fold(0xcbf29ce484222325u64, |h, &b| (h ^ u64::from(b))
                .wrapping_mul(0x100000001b3))
    )
}
impl Sprite {
    pub fn new(
        _id: i32,
        _options: Options,
        image: Image,
        material: Material,
    ) -> Result<Self, String> {
        let (backing, rgba) = image.backing_rgba()?;
        #[cfg(not(target_arch = "wasm32"))]
        let expected_pixels = checksum(&rgba);
        let texture = if image.size.contains(&0) {
            None
        } else {
            let width = u16::try_from(backing[0])
                .map_err(|_| "Widget texture width exceeds native bound")?;
            let height = u16::try_from(backing[1])
                .map_err(|_| "Widget texture height exceeds native bound")?;
            let texture = Texture2D::from_rgba8(width, height, &rgba);
            texture.set_filter(FilterMode::Nearest);
            let id = texture.raw_miniquad_id();
            // Original GL textures retain the default repeat wrap. The original
            // power-of-two backing also makes this valid in WebGL1.
            unsafe {
                get_internal_gl().quad_context.texture_set_wrap(
                    id,
                    miniquad::TextureWrap::Repeat,
                    miniquad::TextureWrap::Repeat,
                );
            }
            Some(texture)
        };
        Ok(Self {
            material,
            texture,
            filter: Cell::new(FilterMode::Nearest),
            inner: image.inner,
            offset: image.offset,
            size: image.size,
            #[cfg(not(target_arch = "wasm32"))]
            id: _id,
            #[cfg(not(target_arch = "wasm32"))]
            options: _options,
            #[cfg(not(target_arch = "wasm32"))]
            alpha: image.alpha,
            #[cfg(not(target_arch = "wasm32"))]
            expected_pixels,
        })
    }
    fn filter(&self, filter: FilterMode) {
        if self.filter.get() != filter {
            // Preserve the filter used by already queued geometry before
            // mutating a shared texture, as the original immediate GL path does.
            unsafe {
                get_internal_gl().flush();
            }
            if let Some(texture) = &self.texture {
                texture.set_filter(filter);
            }
            self.filter.set(filter);
        }
    }
    fn quad(&self, at: Vec2, size: Vec2, repeats: Vec2, alpha: f32, rotation: f32, pivot: Vec2) {
        let Some(texture) = &self.texture else {
            return;
        };
        let uv = vec2(
            self.size[0] as f32 / texture.width(),
            self.size[1] as f32 / texture.height(),
        ) * repeats;
        let (sin, cos) = rotation.sin_cos();
        let vertices = std::array::from_fn::<_, 4, _>(|i| {
            let bit = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]][i];
            let mut point = at + size * vec2(bit[0], bit[1]);
            if rotation != 0.0 {
                let d = point - pivot;
                point = pivot + vec2(d.x * cos - d.y * sin, d.x * sin + d.y * cos);
            }
            let mut vertex =
                Vertex::new(point.x, point.y, 0.0, uv.x * bit[0], uv.y * bit[1], WHITE);
            vertex.normal.x = alpha;
            vertex
        });
        // Same stack-array geometry submission as draw_texture_ex, without an
        // atlas redirect: repeated UVs must address this sprite's own backing.
        unsafe {
            let gl = get_internal_gl();
            gl.quad_gl.texture(Some(texture));
            gl.quad_gl.draw_mode(DrawMode::Triangles);
            gl.quad_gl.geometry(&vertices, &[0, 1, 2, 0, 2, 3]);
        }
    }
    pub fn natural(&self, at: Vec2) {
        self.filter(FilterMode::Nearest);
        self.quad(
            at + vec2(self.offset[0] as f32, self.offset[1] as f32),
            vec2(self.size[0] as f32, self.size[1] as f32),
            vec2(1.0, 1.0),
            1.0,
            0.0,
            Vec2::ZERO,
        );
    }
    /// `Sprite.render(x, y)`, or `renderAlpha(x, y, alpha)` when `alpha` (out
    /// of 256) is not full: an item sprite in an inventory slot.
    pub fn icon(&self, at: Vec2, alpha: i32) {
        if self.texture.is_none() {
            return;
        }
        gl_use_material(&self.material);
        if alpha == 256 {
            self.natural(at);
        } else {
            self.natural_alpha(at, alpha as f32 / 256.0);
        }
        gl_use_default_material();
    }
    fn natural_alpha(&self, at: Vec2, alpha: f32) {
        self.filter(FilterMode::Nearest);
        self.quad(
            at + vec2(self.offset[0] as f32, self.offset[1] as f32),
            vec2(self.size[0] as f32, self.size[1] as f32),
            vec2(1.0, 1.0),
            alpha,
            0.0,
            Vec2::ZERO,
        );
    }
    fn resized(&self, rect: [i32; 4], alpha: Option<i32>) -> Result<(), String> {
        if let Some([x, y, w, h]) =
            widget_sprite::resized_rect(self.inner, self.offset, self.size, rect)?
        {
            self.filter(if alpha.is_none() {
                FilterMode::Linear
            } else {
                FilterMode::Nearest
            });
            self.quad(
                vec2(x as f32, y as f32),
                vec2(w as f32, h as f32),
                vec2(1.0, 1.0),
                alpha.map_or(1.0, |a| a as f32 / 256.0),
                0.0,
                Vec2::ZERO,
            );
        }
        Ok(())
    }
    fn tiled(&self, at: [i32; 2], tiles: [i32; 2], alpha: i32) {
        self.filter(FilterMode::Nearest);
        let repeat = vec2(tiles[0] as f32, tiles[1] as f32);
        let at = vec2(
            at[0].wrapping_add(self.offset[0] as i32) as f32,
            at[1].wrapping_add(self.offset[1] as i32) as f32,
        );
        self.quad(
            at,
            vec2(self.size[0] as f32, self.size[1] as f32) * repeat,
            repeat,
            alpha as f32 / 256.0,
            0.0,
            Vec2::ZERO,
        );
    }
    fn rotated(&self, [y, angle, zoom, x]: [i32; 4]) {
        self.filter(FilterMode::Nearest);
        let px = (self.inner[0] as i32).wrapping_shl(3);
        let py = (self.inner[1] as i32).wrapping_shl(3);
        let pivot = vec2(
            x.wrapping_shl(4).wrapping_add(px & 15) as f32 / 16.0,
            y.wrapping_shl(4).wrapping_add(py & 15) as f32 / 16.0,
        );
        let origin = vec2(
            px.wrapping_sub((self.offset[0] as i32).wrapping_shl(4)) as f32 / 16.0,
            py.wrapping_sub((self.offset[1] as i32).wrapping_shl(4)) as f32 / 16.0,
        );
        let scale = zoom as f32 / 4096.0;
        self.quad(
            pivot - origin * scale,
            vec2(self.size[0] as f32, self.size[1] as f32) * scale,
            vec2(1.0, 1.0),
            1.0,
            -(angle as f32) * std::f32::consts::TAU / 65536.0,
            pivot,
        );
    }
    pub fn widget(
        &self,
        c: &Component,
        rect: [i32; 4],
        alpha: i32,
        clip: [i32; 4],
        active: bool,
    ) -> Result<(), String> {
        use widget_sprite::{Draw, DrawInput};
        if self.texture.is_none() {
            return Ok(());
        }
        gl_use_material(&self.material);
        let mut error = None;
        let result = widget_sprite::draw_commands(
            DrawInput {
                if3: c.if3,
                tiling: c.sprite_tiling,
                angle: c.angle2d,
                inner: self.inner.map(|n| n as i32),
                size: self.size.map(|n| n as i32),
                rect,
                alpha,
                active,
                clip,
            },
            |command| {
                match command {
                    Draw::Sprite(_) | Draw::Plugin(_) => {} // Selection already resolved; plugin host is separate.
                    Draw::Clip([left, top, right, bottom]) => {
                        let w = screen_width() as i32;
                        let h = screen_height() as i32;
                        let x = left.clamp(0, w);
                        let y = top.clamp(0, h);
                        crate::display_host::scissor(Some((
                            x,
                            y,
                            (right.clamp(0, w) - x).max(0),
                            (bottom.clamp(0, h) - y).max(0),
                        )));
                    }
                    Draw::Natural([x, y]) => self.natural(vec2(x as f32, y as f32)),
                    Draw::AlphaNatural([x, y, a]) => {
                        self.natural_alpha(vec2(x as f32, y as f32), a as f32 / 256.0)
                    }
                    Draw::Resized(rect) => {
                        if let Err(e) = self.resized(rect, None) {
                            error = Some(e);
                        }
                    }
                    Draw::AlphaResize([x, y, w, h, a]) => {
                        if let Err(e) = self.resized([x, y, w, h], Some(a)) {
                            error = Some(e);
                        }
                    }
                    Draw::Rotated(args) => self.rotated(args),
                    Draw::Tiled([x, y, nx, ny]) => self.tiled([x, y], [nx, ny], 256),
                    Draw::AlphaTile([x, y, a, nx, ny]) => self.tiled([x, y], [nx, ny], a),
                }
            },
        );
        gl_use_default_material();
        result?;
        error.map_or(Ok(()), Err)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn snapshot(&self) -> serde_json::Value {
        let actual = self
            .texture
            .as_ref()
            .map(|t| t.get_texture_data().bytes)
            .unwrap_or_default();
        serde_json::json!({"id":self.id,"options":self.options,"inner":self.inner,"offset":self.offset,"size":self.size,"alpha":self.alpha,"expected_backing_pixels":self.expected_pixels,"actual_backing_pixels":checksum(&actual),"actual_backing_bytes":actual.len()})
    }
}
