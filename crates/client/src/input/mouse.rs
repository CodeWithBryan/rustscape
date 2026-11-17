//! RT4 `Mouse`: the AWT mouse listener and its per-cycle `loop`.
//!
//! The listener half records what arrives between cycles in the `pending_*`
//! and `current_*` fields. [`Mouse::cycle`] copies them once per 20 ms logic
//! cycle into the fields the client reads: one click (button, position and
//! time) is seen for exactly one cycle, and the pointer position and held
//! button stay fixed for the whole cycle.
//!
//! Original behaviour kept as it is, including the 2009scape changes:
//!
//! - The pointer is at (-1, -1) after it leaves the window.
//! - The middle button never clicks, and while it is held drags are not
//!   recorded, so the pointer position freezes (as `SwingUtilities
//!   .isMiddleMouseButton` decides on JDK 9 and later).
//! - A press is button 2 when the right button is down at that moment, else
//!   button 1; releasing any button, the middle one too, clears the held
//!   button.
//!
//! `fixtures/input/mouse.txt` holds what the unchanged class did with real
//! `MouseEvent` objects; `tests/input.rs` replays it here.

/// Buttons down after an event, as `MouseEvent.getModifiersEx()` reports
/// them (`BUTTON1_DOWN_MASK`, `BUTTON2_DOWN_MASK`, `BUTTON3_DOWN_MASK`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Held {
    pub left: bool,
    pub middle: bool,
    pub right: bool,
}

#[derive(Clone, Debug)]
pub struct Mouse {
    /// Position of the click taken by the last cycle.
    pub click_x: i32,
    pub click_y: i32,
    /// Pointer position at the last cycle; (-1, -1) outside the window.
    pub last_mouse_x: i32,
    pub last_mouse_y: i32,
    /// 1 or 2 during the one cycle that takes a click, else 0.
    pub click_button: i32,
    /// The held button at the last cycle: 0, 1 or 2.
    pub pressed_button: i32,
    /// Clock value of the click taken by the last cycle, in milliseconds.
    pub click_time: i64,
    pub pending_click_time: i64,
    pub pending_click_x: i32,
    pub pending_click_y: i32,
    pub pending_pressed_button: i32,
    pub pending_click_button: i32,
    /// Pointer position as of the latest event, before any cycle sees it.
    /// The original's mouse recorder samples these.
    pub current_mouse_x: i32,
    pub current_mouse_y: i32,
    /// `lastHandledClickX`, `lastHandledClickY` and `prevClickTime`: the
    /// original keeps them here but only `Protocol.loop` writes them.
    pub last_handled_click_x: i32,
    pub last_handled_click_y: i32,
    pub prev_click_time: i64,
    idle_loops: i32,
    /// False after [`Mouse::quit`]: the listener then ignores every event.
    instance: bool,
}

impl Default for Mouse {
    fn default() -> Self {
        Self {
            click_x: 0,
            click_y: 0,
            last_mouse_x: 0,
            last_mouse_y: 0,
            click_button: 0,
            pressed_button: 0,
            click_time: 0,
            pending_click_time: 0,
            pending_click_x: 0,
            pending_click_y: 0,
            pending_pressed_button: 0,
            pending_click_button: 0,
            current_mouse_x: -1,
            current_mouse_y: -1,
            last_handled_click_x: 0,
            last_handled_click_y: 0,
            prev_click_time: 0,
            idle_loops: 0,
            instance: true,
        }
    }
}

impl Mouse {
    /// `Mouse.loop`, once per logic cycle.
    pub fn cycle(&mut self) {
        self.pressed_button = self.pending_pressed_button;
        self.last_mouse_x = self.current_mouse_x;
        self.last_mouse_y = self.current_mouse_y;
        self.click_button = self.pending_click_button;
        self.click_x = self.pending_click_x;
        self.idle_loops = self.idle_loops.wrapping_add(1);
        self.click_y = self.pending_click_y;
        self.click_time = self.pending_click_time;
        self.pending_click_button = 0;
    }

    /// `mouseMoved`: the pointer moved with no button down.
    pub fn mouse_moved(&mut self, x: i32, y: i32) {
        if self.instance {
            self.idle_loops = 0;
            self.current_mouse_x = x;
            self.current_mouse_y = y;
        }
    }

    /// `mouseDragged`: the pointer moved with `held` buttons down.
    pub fn mouse_dragged(&mut self, x: i32, y: i32, held: Held) {
        // SwingUtilities.isMiddleMouseButton: for a drag, the held buttons
        // decide.
        if held.middle {
            return;
        }
        if self.instance {
            self.idle_loops = 0;
            self.current_mouse_x = x;
            self.current_mouse_y = y;
        }
    }

    /// `mousePressed`. `middle` says the event's own button is the middle
    /// one; `held` is what is down once it has been pressed. `now` is
    /// `MonotonicClock.currentTimeMillis()`.
    pub fn mouse_pressed(&mut self, x: i32, y: i32, middle: bool, held: Held, now: i64) {
        // SwingUtilities.isMiddleMouseButton: for a press, the event's own
        // button decides; a held middle button does not hide other presses.
        if middle {
            return;
        }
        if self.instance {
            self.idle_loops = 0;
            self.pending_click_x = x;
            self.pending_click_y = y;
            self.pending_click_time = now;
            if held.right {
                self.pending_click_button = 2;
                self.pending_pressed_button = 2;
            } else {
                self.pending_click_button = 1;
                self.pending_pressed_button = 1;
            }
        }
    }

    /// `mouseReleased`, for any button.
    pub fn mouse_released(&mut self) {
        if self.instance {
            self.idle_loops = 0;
            self.pending_pressed_button = 0;
        }
    }

    /// `mouseExited`: the pointer left the window.
    pub fn mouse_exited(&mut self) {
        if self.instance {
            self.idle_loops = 0;
            self.current_mouse_x = -1;
            self.current_mouse_y = -1;
        }
    }

    /// `mouseEntered`.
    pub fn mouse_entered(&mut self, x: i32, y: i32) {
        if self.instance {
            self.idle_loops = 0;
            self.current_mouse_x = x;
            self.current_mouse_y = y;
        }
    }

    /// `focusLost`: no button is held any more.
    pub fn focus_lost(&mut self) {
        if self.instance {
            self.pending_pressed_button = 0;
        }
    }

    /// `stop`: the listener is detached.
    pub fn stop(&mut self) {
        self.pending_pressed_button = 0;
    }

    /// `quit`: every later event is ignored.
    pub fn quit(&mut self) {
        self.instance = false;
    }

    /// `Mouse.getIdleLoops`: cycles since the last mouse event. 1 right after
    /// a cycle means an event arrived since the cycle before.
    pub fn idle_loops(&self) -> i32 {
        self.idle_loops
    }

    /// `Mouse.setIdleLoops`: the idle logout sets 14500, a login sets 0.
    pub fn set_idle_loops(&mut self, value: i32) {
        self.idle_loops = value;
    }

    /// Everything the differential test compares, in the fixture's order.
    pub fn observation(&self) -> [i64; 15] {
        [
            i64::from(self.click_button),
            i64::from(self.click_x),
            i64::from(self.click_y),
            self.click_time,
            i64::from(self.pressed_button),
            i64::from(self.last_mouse_x),
            i64::from(self.last_mouse_y),
            i64::from(self.pending_click_button),
            i64::from(self.pending_click_x),
            i64::from(self.pending_click_y),
            self.pending_click_time,
            i64::from(self.pending_pressed_button),
            i64::from(self.current_mouse_x),
            i64::from(self.current_mouse_y),
            i64::from(self.idle_loops),
        ]
    }
}
