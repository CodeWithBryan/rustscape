//! `Fonts.drawTextOnScreen` (Fonts.java:61-71): the black box with a white
//! outline at the top left of the canvas that carries "Loading - please
//! wait." (game state 25) and the connection-lost message (game state 40).
use super::{Renderer, TextStyle};
use macroquad::prelude::*;
use rs09_client::{lifecycle::StatusBox, pack::Pack};
use std::collections::BTreeMap;

impl Renderer {
    /// Draws `text` (with `<br>` line breaks) in `p12_full`, as the software
    /// and the hardware renderer both do: `fillRect(6, 6, w + 8, h + 8, 0)`,
    /// `drawRect` of the same rectangle in white, then the paragraph centred
    /// at (10, 10) in white with no shadow.
    pub fn status_box(&mut self, ui: &Pack, visual: &Pack, text: &str) -> Result<(), String> {
        let ids: BTreeMap<String, i32> =
            serde_json::from_slice(ui.get("overhead/assets")?).map_err(|e| e.to_string())?;
        let id = *ids
            .get("p12_full")
            .ok_or("Status box p12_full font asset absent")?;
        let font = self.font(ui, visual, id)?;
        let layout = StatusBox::layout(&font.metrics, [font.top, font.bottom], text);
        let [x, y, w, h] = layout.rect.map(|n| n as f32);
        draw_rectangle(x, y, w, h, Color::from_hex(StatusBox::FILL));
        let outline = Color::from_hex(StatusBox::OUTLINE);
        draw_rectangle(x, y, w, 1.0, outline);
        draw_rectangle(x, y + h - 1.0, w, 1.0, outline);
        draw_rectangle(x, y, 1.0, h, outline);
        draw_rectangle(x + w - 1.0, y, 1.0, h, outline);
        let mut style = TextStyle::new(Color::from_hex(StatusBox::TEXT), false);
        for (line, position) in &layout.lines {
            font.styled_line(line, position[0] as f32, position[1] as f32, &mut style);
        }
        Ok(())
    }
}
