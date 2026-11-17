//! `LoadingBar.render` (LoadingBar.java:10-33): the title background and
//! logo with the loading bar and its two lines of text, which the original
//! draws in game state 5 while `client.mainLoad` runs. The title screen
//! proper is the `loginscreen` interface and is drawn like any other.
use super::Renderer;
use crate::display_host::{screen_height, screen_width};
use macroquad::prelude::*;
use rs09_client::{
    pack::Pack,
    title::{Assets, LOADING_TITLE, LoadingBar},
};
use std::collections::BTreeMap;

fn outline([x, y, w, h]: [i32; 4], color: Color) {
    let [x, y, w, h] = [x, y, w, h].map(|n| n as f32);
    draw_rectangle(x, y, w, 1.0, color);
    draw_rectangle(x, y + h - 1.0, w, 1.0, color);
    draw_rectangle(x, y, 1.0, h, color);
    draw_rectangle(x + w - 1.0, y, 1.0, h, color);
}
fn fill([x, y, w, h]: [i32; 4], color: Color) {
    draw_rectangle(x as f32, y as f32, w as f32, h as f32, color);
}

impl Renderer {
    /// `titleBg.renderResized(x, y, width, height)`: the whole sprite,
    /// margins included, stretched to the rectangle.
    ///
    /// The stretch is linear-filtered; which filter the original's
    /// `GlSprite.renderResized` ends up with was not established.
    fn title_background(&mut self, visual: &Pack, id: i32, to: [i32; 4]) -> Result<(), String> {
        self.overhead_sheet(visual, id)?;
        let sheet = &self.overhead_sheets[&id];
        let frame = sheet.frames.first().ok_or("Title background is empty")?;
        let key = (id, 0);
        if let std::collections::btree_map::Entry::Vacant(entry) = self.overhead_textures.entry(key)
        {
            let rgba = sheet.rgba(0).map_err(|e| e.to_string())?;
            let texture = Texture2D::from_rgba8(frame.size[0] as u16, frame.size[1] as u16, &rgba);
            texture.set_filter(FilterMode::Linear);
            entry.insert(texture);
        }
        let scale = [
            to[2] as f32 / sheet.size[0].max(1) as f32,
            to[3] as f32 / sheet.size[1].max(1) as f32,
        ];
        draw_texture_ex(
            &self.overhead_textures[&key],
            to[0] as f32 + frame.offset[0] as f32 * scale[0],
            to[1] as f32 + frame.offset[1] as f32 * scale[1],
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(
                    frame.size[0] as f32 * scale[0],
                    frame.size[1] as f32 * scale[1],
                )),
                ..Default::default()
            },
        );
        Ok(())
    }

    /// The title interface's scene component (client code 1403). With the
    /// hardware renderer the original draws the title screen's 3D scene in
    /// it (`ScriptRunner.renderGameScene`): a region around a scripted
    /// camera path, built in game state 28 (`title_world.rs`). That needs
    /// maps streamed from a cache. Where there is none, this stand-in fills
    /// the component with the title background instead, placed as
    /// `TitleScreen.load` places it, so that the screen keeps the picture it
    /// loads behind once the interface fades its own copy out.
    pub(super) fn title_scene(
        &mut self,
        ui: &Pack,
        visual: &Pack,
        rect: rs09_client::ui::state::Rect,
    ) -> Result<(), String> {
        draw_rectangle(
            rect.x as f32,
            rect.y as f32,
            rect.w as f32,
            rect.h as f32,
            BLACK,
        );
        let width = rect.h * 956 / 503;
        let background = Assets::read(ui)?.background;
        self.title_background(
            visual,
            background,
            [rect.x + (rect.w - width) / 2, rect.y, width, rect.h],
        )
    }
}
impl Renderer {
    /// The whole frame of game state 5 at `percent` with the stage `text`.
    /// The canvas is cleared first, as `TitleScreen.load` clears it before
    /// the background goes on.
    pub fn loading_bar(
        &mut self,
        ui: &Pack,
        visual: &Pack,
        assets: &Assets,
        percent: i32,
        text: &str,
    ) -> Result<(), String> {
        clear_background(BLACK);
        let canvas = [screen_width() as i32, screen_height() as i32];
        self.overhead_sheet(visual, assets.logo)?;
        let logo_width = self.overhead_sheets[&assets.logo].size[0] as i32;
        let bar = LoadingBar::layout(canvas, percent, logo_width);

        self.title_background(visual, assets.background, bar.background)?;
        self.draw_sheet_frame(visual, assets.logo, 0, bar.logo)?;

        let ids: BTreeMap<String, i32> =
            serde_json::from_slice(ui.get("overhead/assets")?).map_err(|e| e.to_string())?;
        let id = *ids.get("b12_full").ok_or("b12_full font asset absent")?;
        let font = self.font(ui, visual, id)?;
        let white = Color::from_hex(LoadingBar::TEXT);
        let centred = |text: &str, [x, baseline]: [i32; 2]| {
            font.line(
                text,
                x.wrapping_sub(font.metrics.width(text) / 2) as f32,
                baseline as f32,
                white,
                false,
            );
        };
        centred(LOADING_TITLE, bar.title);
        let red = Color::from_hex(LoadingBar::RED);
        outline(bar.outline, red);
        outline(bar.inner_outline, BLACK);
        fill(bar.filled, red);
        fill(bar.empty, BLACK);
        centred(text, bar.text);
        Ok(())
    }
}
