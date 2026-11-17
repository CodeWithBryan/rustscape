//! Keyboard and mouse as the original client sees them.
//!
//! [`keyboard`] and [`mouse`] are the RT4 `Keyboard` and `Mouse` classes.
//! [`platform`] turns the window system's events into the calls AWT would
//! have made on them. [`bindings`] is the part of the interface walk that
//! fires a component's key bindings.
//!
//! The client calls [`platform::Events::pump`] once per rendered frame and
//! [`Input::cycle`] once per 20 ms logic cycle. Everything the game reads
//! (held keys, typed keys, pointer, click) changes only in `cycle`:
//!
//! ```text
//! frame:  events.pump(&mut input, now_ms)
//! cycle:  input.cycle()
//!         input.keyboard.pressed_keys[KEY_CTRL]     held keys
//!         input.keys                                this cycle's (code, char) pairs
//!         input.mouse.click_button / click_x / ...  this cycle's click
//!         input.mouse.last_mouse_x / pressed_button pointer and held button
//! ```
pub mod bindings;
pub mod keyboard;
pub mod mouse;
pub mod platform;

use crate::ui::state::PointerInput;
use keyboard::{KEY_ALT, KEY_CTRL, KEY_SHIFT, Keyboard};
use mouse::Mouse;

pub struct Input {
    pub keyboard: Keyboard,
    pub mouse: Mouse,
    /// `InterfaceList.keyCodes` and `keyChars` for the current cycle: up to
    /// 128 `[code, char]` pairs in arrival order, one of the two being -1.
    pub keys: Vec<[i32; 2]>,
    /// `GameShell.focusIn`: whether the window has keyboard focus.
    pub focus: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            keyboard: Keyboard::default(),
            mouse: Mouse::default(),
            keys: Vec::new(),
            focus: true,
        }
    }
}

impl Input {
    /// One logic cycle: `Keyboard.loop()`, `Mouse.loop()`, then the key queue
    /// as `Protocol.loop` fills it.
    pub fn cycle(&mut self) {
        self.keyboard.cycle();
        self.mouse.cycle();
        self.keys.clear();
        while self.keyboard.next_key() && self.keys.len() < 128 {
            self.keys
                .push([self.keyboard.key_code, self.keyboard.key_char]);
        }
    }

    /// Whether Alt, Ctrl and Shift are held, in the order of CS2 5100 to 5102.
    pub fn modifiers(&self) -> [bool; 3] {
        let pressed = &self.keyboard.pressed_keys;
        [pressed[KEY_ALT], pressed[KEY_CTRL], pressed[KEY_SHIFT]]
    }

    /// Cycles since the last mouse event and since the last key press or
    /// release, as the idle logout reads them. Each is 1 right after a cycle
    /// when such an event arrived since the cycle before.
    pub fn idle_loops(&self) -> [i32; 2] {
        [self.mouse.idle_loops(), self.keyboard.idle_loops()]
    }

    /// Fill the interface walk's pointer from this cycle's mouse snapshot.
    /// `scroll` and `suppressed` are left alone. With `shown` false the
    /// interface is hidden and receives no buttons.
    ///
    /// The original tests a click against its own position (`clickX`,
    /// `clickY`, InterfaceList.java:684), not where the pointer is by the
    /// cycle that takes it.
    pub fn pointer(&self, pointer: &mut PointerInput, shown: bool) {
        let mouse = &self.mouse;
        pointer.point = [mouse.last_mouse_x, mouse.last_mouse_y];
        pointer.click = Some([mouse.click_x, mouse.click_y]);
        pointer.clicked = shown && mouse.click_button == 1;
        pointer.down = shown && mouse.pressed_button == 1;
        pointer.any_down = shown && mouse.pressed_button != 0;
    }
}
