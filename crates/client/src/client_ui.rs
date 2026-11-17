//! Window input for the context menu, whose state and rules live in the
//! GPU-independent client library (`rs09_client::context_menu`), and the plain
//! text presentation used when no interface pack is loaded.
use crate::display_host::{mouse_position, screen_height, screen_width};
use macroquad::prelude::*;
use rs09_client::{
    context_menu::{self, Button},
    interaction::{Menu, MenuEntry},
};

#[derive(Default)]
pub struct ContextMenu(context_menu::ContextMenu);
impl std::ops::Deref for ContextMenu {
    type Target = context_menu::ContextMenu;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for ContextMenu {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
fn pointer() -> [i32; 2] {
    let (x, y) = mouse_position();
    [x as i32, y as i32]
}
impl ContextMenu {
    /// `ScriptRunner.layoutMiniMenu`: the menu opens around `point`, where
    /// it was built. `width` is the widest entry in the menu font plus
    /// eight; without cache fonts it is measured in the built-in font.
    pub fn open_at(&mut self, mut menu: Menu, width: Option<i32>, point: [i32; 2]) {
        let width = width.unwrap_or_else(|| {
            context_menu::width(&menu.entries, |text| {
                measure_text(strip_tags(text), None, 16, 1.0).width as i32
            })
        });
        menu.sort();
        self.0.open(
            menu,
            width,
            point,
            [screen_width() as i32, screen_height() as i32],
        );
    }
    /// One cycle of the open menu: the entry a left click chose, if any.
    /// `pointer` is `Mouse.lastMouseX/Y`, `click` is `Mouse.clickX/Y`.
    pub fn step_at(
        &mut self,
        button: Button,
        pointer: [i32; 2],
        click: [i32; 2],
    ) -> Option<MenuEntry> {
        self.0.step(button, pointer, click)
    }
    pub fn draw(&self) {
        if !self.menu.open {
            return;
        }
        let b = self.bounds;
        let (x, y, w, h) = (b.x as f32, b.y as f32, b.width as f32, b.height as f32);
        let frame = Color::from_hex(0x5d5447);
        draw_rectangle(x, y, w, h, frame);
        draw_rectangle(x + 1.0, y + 1.0, w - 2.0, 16.0, BLACK);
        crate::ui_render::legacy_border([b.x + 1, b.y + 18, b.width - 2, b.height - 19], BLACK);
        draw_text(context_menu::CHOOSE_OPTION, x + 3.0, y + 14.0, 16.0, frame);
        let size = self.menu.entries.len();
        let hovered = b.row_at(size, pointer());
        for (index, entry) in self.menu.entries.iter().enumerate() {
            draw_text(
                label(entry),
                x + 3.0,
                b.row_baseline(size, index) as f32,
                16.0,
                if hovered == Some(index) {
                    YELLOW
                } else {
                    WHITE
                },
            );
        }
    }
}
fn strip_tags(text: &str) -> String {
    let mut plain = String::new();
    let mut tag = false;
    for c in text.chars() {
        if c == '<' {
            tag = true;
        } else if c == '>' {
            tag = false;
        } else if !tag {
            plain.push(c);
        }
    }
    plain
}
pub fn label(entry: &MenuEntry) -> String {
    strip_tags(&context_menu::op_text(entry))
}

/// The local wall clock's hour, minute and second, which seed the hover
/// text's jitter (`GregorianCalendar` in `client.mainLoop`).
#[cfg(target_os = "windows")]
pub fn local_time() -> [i32; 3] {
    // SYSTEMTIME is eight WORDs: year, month, day of week, day, hour, minute,
    // second, milliseconds.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(time: *mut [u16; 8]);
    }
    let mut time = [0u16; 8];
    // SAFETY: GetLocalTime fills the SYSTEMTIME it is given and cannot fail.
    unsafe { GetLocalTime(&mut time) };
    [i32::from(time[4]), i32::from(time[5]), i32::from(time[6])]
}
/// No dependency-free local-time call exists on the other targets, so this is
/// the UTC time of day: the jitter still changes with the clock, but not on
/// the original's local-time schedule.
#[cfg(not(target_os = "windows"))]
pub fn local_time() -> [i32; 3] {
    let seconds = (miniquad::date::now() as i64).rem_euclid(86_400) as i32;
    [seconds / 3600, seconds / 60 % 60, seconds % 60]
}
