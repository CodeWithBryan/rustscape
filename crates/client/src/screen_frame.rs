//! The frame on screen, copied into a texture: what the original gets from
//! `GlRenderer.draw()` when it enters a game state that stops redrawing the
//! game (25, 28, 40) and goes on showing the frame it had.
//!
//! Two things about the copy are the platform's:
//!
//! - The browser page makes its drawing buffer without an alpha channel
//!   (`web/visual/graphics_lifecycle.js`), and WebGL refuses to copy such a
//!   buffer into a texture that has one (`glCopyTexImage2D: Invalid
//!   format`). There the texture is RGB, made through miniquad because
//!   macroquad only makes RGBA ones; on the desktop it is the RGBA texture
//!   macroquad makes, as before.
//! - macroquad's `grab_screen` binds the texture on whichever unit is active
//!   without telling miniquad, which keeps its own record of what is bound
//!   where. Its next draw with the texture it believes bound on that unit
//!   then samples the copied frame instead. The letterbox's black bands,
//!   drawn right after the copy with macroquad's plain white texture, came
//!   out black with the copied frame's alpha, and a browser canvas with an
//!   alpha channel showed the page through them. [`Frame::grab`] goes
//!   through miniquad's record before and after the copy.
use macroquad::prelude::*;

pub struct Frame {
    texture: Texture2D,
    /// The texture, which macroquad does not own and will not delete.
    #[cfg(target_arch = "wasm32")]
    raw: miniquad::TextureId,
}

impl Frame {
    /// A texture of `size` pixels for [`Self::grab`], blank until then.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(size: [u16; 2]) -> Self {
        let blank = vec![0; usize::from(size[0]) * usize::from(size[1]) * 4];
        let texture = Texture2D::from_rgba8(size[0], size[1], &blank);
        texture.set_filter(FilterMode::Nearest);
        Self { texture }
    }

    /// A texture of `size` pixels for [`Self::grab`], undefined until then.
    #[cfg(target_arch = "wasm32")]
    pub fn new(size: [u16; 2]) -> Self {
        // SAFETY: the macroquad context exists whenever a frame is drawn.
        let raw = unsafe { get_internal_gl() }
            .quad_context
            .new_render_texture(miniquad::TextureParams {
                format: miniquad::TextureFormat::RGB8,
                width: u32::from(size[0]),
                height: u32::from(size[1]),
                ..Default::default()
            });
        let texture = Texture2D::from_miniquad_texture(raw);
        texture.set_filter(FilterMode::Nearest);
        Self { texture, raw }
    }

    /// The whole surface as drawn so far, in a texture of its own.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn snapshot() -> Self {
        let texture = Texture2D::from_image(&get_screen_data());
        texture.set_filter(FilterMode::Nearest);
        Self { texture }
    }

    /// The whole surface as drawn so far, in a texture of its own.
    #[cfg(target_arch = "wasm32")]
    pub fn snapshot() -> Self {
        let frame = Self::new(surface());
        frame.grab();
        frame
    }

    pub fn size(&self) -> [u16; 2] {
        [self.texture.width() as u16, self.texture.height() as u16]
    }

    /// Copies the frame drawn so far, without reading it back. The copy is
    /// stored bottom row first.
    pub fn grab(&self) {
        // SAFETY: as in `new`.
        unsafe { get_internal_gl().flush() };
        // Setting the filter goes through miniquad's record of unit 0:
        // before the copy it leaves unit 0 the active one, and after it puts
        // back what miniquad believes is bound there.
        self.texture.set_filter(FilterMode::Nearest);
        self.texture.grab_screen();
        self.texture.set_filter(FilterMode::Nearest);
    }

    /// Draws the copy over the whole surface, the right way up, with
    /// macroquad's default camera. The caller sets its own camera again.
    pub fn draw(&self) {
        set_default_camera();
        draw_texture_ex(
            &self.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(screen_width(), screen_height())),
                flip_y: true,
                ..Default::default()
            },
        );
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for Frame {
    fn drop(&mut self) {
        // SAFETY: as in `new`. A frame is replaced only when nothing queued
        // draws it (see the callers).
        unsafe { get_internal_gl() }
            .quad_context
            .delete_texture(self.raw);
    }
}

/// The surface's size in pixels; zero while the window is minimised.
pub fn surface() -> [u16; 2] {
    let scale = screen_dpi_scale();
    [screen_width() * scale, screen_height() * scale].map(|n| n as u16)
}
