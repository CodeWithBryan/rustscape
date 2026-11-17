//! Presents the cursor the menu selected (`rs09_client::cursor`).
//!
//! The original hands the sprite to the windowing toolkit as a custom cursor.
//! Here the system cursor is hidden and the sprite is drawn at the pointer,
//! its hot spot on the pointer position, on the desktop and in the browser
//! alike. The toolkit may also rescale a cursor image to the size the platform
//! prefers; the sprite is drawn at its own size.
use crate::display_host::mouse_position;
use macroquad::prelude::*;
use rs09_client::{
    cursor::{self, Change, CursorType},
    pack::Pack,
    ui::sprite::Sheet,
};

struct Shown {
    texture: Texture2D,
    hot_spot: [i32; 2],
}

#[derive(Default)]
pub struct View {
    current: cursor::Current,
    shown: Option<Shown>,
    reported: std::collections::BTreeSet<i32>,
}
impl View {
    /// `InterfaceList.setCursor` for this frame's cursor. `packs` are searched
    /// in order for the cursor type and its sprite. Returns a diagnostic the
    /// first time a cursor cannot be shown.
    pub fn set(&mut self, id: i32, enabled: bool, packs: &[&Pack]) -> Option<String> {
        let mut error = None;
        let change = self.current.set(id, enabled, |id| {
            load(id, packs).unwrap_or_else(|e| {
                error = Some(e);
                None
            })
        });
        match change {
            Change::Keep => {}
            Change::Custom(shown) => {
                self.shown = Some(shown);
                show_mouse(false);
            }
            Change::System => {
                self.shown = None;
                show_mouse(true);
            }
        }
        error.filter(|_| self.reported.insert(id))
    }
    /// Last in the frame, over everything else.
    pub fn draw(&self) {
        if let Some(shown) = &self.shown {
            let (x, y) = mouse_position();
            draw_texture(
                &shown.texture,
                (x as i32 - shown.hot_spot[0]) as f32,
                (y as i32 - shown.hot_spot[1]) as f32,
                WHITE,
            );
        }
    }
}

/// `CursorTypeList.get` then `CursorType.getSprite`. None when no pack can
/// say what the cursor is or its sprite is absent: the system cursor stays.
fn load(id: i32, packs: &[&Pack]) -> Result<Option<Shown>, String> {
    let mut kind = None;
    for pack in packs {
        kind = CursorType::load(pack, id)?;
        if kind.is_some() {
            break;
        }
    }
    let Some(kind) = kind else {
        return Ok(None);
    };
    let name = format!("sprite/{}", kind.sprite);
    let Some(pack) = packs
        .iter()
        .find(|pack| pack.manifest.files.contains_key(&name))
    else {
        return Ok(None);
    };
    let sheet = Sheet::decode(pack.get(&name)?).map_err(|e| format!("cursor {id}: {e}"))?;
    let Some(([width, height], rgba)) = cursor::full_image(&sheet) else {
        return Ok(None);
    };
    let (Ok(width), Ok(height)) = (u16::try_from(width), u16::try_from(height)) else {
        return Err(format!("cursor {id}: sprite {} is too large", kind.sprite));
    };
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let texture = Texture2D::from_rgba8(width, height, &rgba);
    texture.set_filter(FilterMode::Nearest);
    Ok(Some(Shown {
        texture,
        hot_spot: kind.hot_spot,
    }))
}
