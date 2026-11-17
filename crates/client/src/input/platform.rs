//! Window-system events, turned into the calls AWT made on the original
//! `Keyboard` and `Mouse` listeners.
//!
//! macroquad records every miniquad event in arrival order for a registered
//! subscriber: key down (with the OS repeat flag), key up, character, mouse
//! motion and buttons, focus lost and gained. [`Events::pump`] replays one
//! frame's worth through [`Translator`]. Native and browser builds share
//! this path.
//!
//! What AWT did and miniquad does not is an explicit policy here, not
//! something a JVM run has checked:
//!
//! - Keys. miniquad names keys by position (US layout); AWT named them by
//!   the layout's virtual key. [`virtual_key`] maps one to the other, so on
//!   other layouts letter and punctuation key codes follow the position.
//!   Number-pad keys are reported as with NumLock on. In a browser the page
//!   reports the quote, slash and back-quote keys itself, since miniquad's
//!   page script and its Rust side disagree about them (see
//!   [`Translator::key_down`]).
//! - Characters. AWT typed `\n` for Enter and 127 for Delete; Windows sends
//!   `\r` and nothing. In a browser Backspace, Tab, Escape and Delete send
//!   no character and Enter sends `\r`. The missing ones are supplied and
//!   `\r` becomes `\n`. Ctrl+letter control characters, which AWT typed, do
//!   not arrive in a browser.
//! - Leaving the window. miniquad reports it on X11 only, and macroquad
//!   does not pass it on. Windows is polled once per frame; the browser page
//!   calls `rs09_mouse_exit`. Other systems never report it.
//! - Extra mouse buttons are ignored (Java 7 and later saw them as left
//!   clicks).
use super::{Input, keyboard::vk, mouse::Held};
use macroquad::miniquad::{EventHandler, KeyCode, KeyMods, MouseButton, TouchPhase};

/// The AWT virtual-key code for a miniquad key; `vk::UNDEFINED` for none.
pub fn virtual_key(key: KeyCode) -> i32 {
    use KeyCode::*;
    let offset = |base: i32, first: KeyCode| base + (key as i32 - first as i32);
    match key {
        Space => vk::SPACE,
        Apostrophe => vk::QUOTE,
        Comma => vk::COMMA,
        Minus => vk::MINUS,
        Period => vk::PERIOD,
        Slash => vk::SLASH,
        Key0 | Key1 | Key2 | Key3 | Key4 | Key5 | Key6 | Key7 | Key8 | Key9 => {
            offset(vk::KEY_0, Key0)
        }
        Semicolon => vk::SEMICOLON,
        Equal => vk::EQUALS,
        A | B | C | D | E | F | G | H | I | J | K | L | M | N | O | P | Q | R | S | T | U | V
        | W | X | Y | Z => offset(vk::A, A),
        LeftBracket => vk::OPEN_BRACKET,
        Backslash => vk::BACK_SLASH,
        RightBracket => vk::CLOSE_BRACKET,
        GraveAccent => vk::BACK_QUOTE,
        Escape => vk::ESCAPE,
        Enter | KpEnter => vk::ENTER,
        Tab => vk::TAB,
        Backspace => vk::BACK_SPACE,
        Insert => vk::INSERT,
        Delete => vk::DELETE,
        Right => vk::RIGHT,
        Left => vk::LEFT,
        Down => vk::DOWN,
        Up => vk::UP,
        PageUp => vk::PAGE_UP,
        PageDown => vk::PAGE_DOWN,
        Home => vk::HOME,
        End => vk::END,
        CapsLock => vk::CAPS_LOCK,
        ScrollLock => vk::SCROLL_LOCK,
        NumLock => vk::NUM_LOCK,
        PrintScreen => vk::PRINTSCREEN,
        Pause => vk::PAUSE,
        F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 => offset(vk::F1, F1),
        F13 | F14 | F15 | F16 | F17 | F18 | F19 | F20 | F21 | F22 | F23 | F24 => {
            offset(vk::F13, F13)
        }
        Kp0 | Kp1 | Kp2 | Kp3 | Kp4 | Kp5 | Kp6 | Kp7 | Kp8 | Kp9 => offset(vk::NUMPAD0, Kp0),
        KpDecimal => vk::DECIMAL,
        KpDivide => vk::DIVIDE,
        KpMultiply => vk::MULTIPLY,
        KpSubtract => vk::SUBTRACT,
        KpAdd => vk::ADD,
        LeftShift | RightShift => vk::SHIFT,
        LeftControl | RightControl => vk::CONTROL,
        LeftAlt | RightAlt => vk::ALT,
        LeftSuper | RightSuper => vk::WINDOWS,
        Menu => vk::CONTEXT_MENU,
        F25 | KpEqual | World1 | World2 | Back | Unknown => vk::UNDEFINED,
    }
}

/// Which characters the window system leaves out (see the module text).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Host {
    Native,
    Browser,
}

impl Host {
    pub fn current() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::Browser
        } else {
            Self::Native
        }
    }
}

/// Stateful translation of window-system events into listener calls.
/// Positions are in the interface's pixels.
#[derive(Clone, Copy, Debug)]
pub struct Translator {
    pub host: Host,
    /// Mouse buttons currently down, for `getModifiersEx`.
    pub held: Held,
}

impl Translator {
    pub fn new(host: Host) -> Self {
        Self {
            host,
            held: Held::default(),
        }
    }

    /// The key the window system meant. miniquad's browser side has no
    /// number for the back-quote key (it reads the number its page script
    /// sends for that key as the quote key), so the client's page reports the
    /// back-quote key as keypad-equals, a key AWT has no code for, and a real
    /// keypad-equals key as unknown (`web/visual/game_transport.js`).
    fn key(&self, key: KeyCode) -> KeyCode {
        match (self.host, key) {
            (Host::Browser, KeyCode::KpEqual) => KeyCode::GraveAccent,
            _ => key,
        }
    }

    /// A key went down, or repeats while held.
    pub fn key_down(&mut self, input: &mut Input, key: KeyCode) {
        let key = self.key(key);
        input.keyboard.key_pressed(virtual_key(key));
        let typed = match (self.host, key) {
            (_, KeyCode::Delete) => 127,
            (Host::Browser, KeyCode::Backspace) => 8,
            (Host::Browser, KeyCode::Tab) => 9,
            (Host::Browser, KeyCode::Escape) => 27,
            _ => return,
        };
        input.keyboard.key_typed(typed);
    }

    pub fn key_up(&mut self, input: &mut Input, key: KeyCode) {
        let key = self.key(key);
        input.keyboard.key_released(virtual_key(key));
    }

    pub fn character(&mut self, input: &mut Input, character: char) {
        let c = match character {
            '\r' => '\n',
            // Already supplied by `key_down`, should a browser send it too.
            '\u{7f}' => return,
            '\u{8}' | '\t' | '\u{1b}' if self.host == Host::Browser => return,
            c => c,
        };
        input.keyboard.key_typed(c as u32 as i32);
    }

    pub fn mouse_motion(&mut self, input: &mut Input, x: i32, y: i32) {
        if self.held == Held::default() {
            input.mouse.mouse_moved(x, y);
        } else {
            input.mouse.mouse_dragged(x, y, self.held);
        }
    }

    pub fn mouse_down(&mut self, input: &mut Input, button: MouseButton, x: i32, y: i32, now: i64) {
        match button {
            MouseButton::Left => self.held.left = true,
            MouseButton::Middle => self.held.middle = true,
            MouseButton::Right => self.held.right = true,
            MouseButton::Unknown => return,
        }
        input
            .mouse
            .mouse_pressed(x, y, button == MouseButton::Middle, self.held, now);
    }

    pub fn mouse_up(&mut self, input: &mut Input, button: MouseButton) {
        match button {
            MouseButton::Left => self.held.left = false,
            MouseButton::Middle => self.held.middle = false,
            MouseButton::Right => self.held.right = false,
            MouseButton::Unknown => return,
        }
        input.mouse.mouse_released();
    }

    /// The window lost or gained keyboard focus. Button releases do not
    /// arrive while another window has it.
    pub fn focus(&mut self, input: &mut Input, focused: bool) {
        input.focus = focused;
        if !focused {
            self.held = Held::default();
            input.keyboard.focus_lost();
            input.mouse.focus_lost();
        }
    }

    /// The pointer left the window.
    pub fn exit(&mut self, input: &mut Input) {
        input.mouse.mouse_exited();
    }
}

/// The canvas inside a larger window (`GameShell.canvas` at its margins in
/// the fixed window modes). The original's listeners are the canvas's own:
/// positions are relative to its corner, the pointer has left when it is
/// over the letterbox, and a press there never reaches them, while a drag
/// begun on the canvas goes on being reported beyond its edge.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Canvas {
    /// `[left, top, width, height]` within the window; None when the canvas
    /// is the whole window.
    pub rect: Option<[i32; 4]>,
    /// Buttons whose press fell outside the canvas; their release is not
    /// the canvas's either.
    elsewhere: Held,
}

impl Canvas {
    /// A window position as the canvas sees it, and whether it is on it.
    pub fn point(&self, x: i32, y: i32) -> ([i32; 2], bool) {
        match self.rect {
            None => ([x, y], true),
            Some([left, top, width, height]) => {
                let point = [x.wrapping_sub(left), y.wrapping_sub(top)];
                let on = point[0] >= 0 && point[1] >= 0 && point[0] < width && point[1] < height;
                (point, on)
            }
        }
    }

    /// A motion event at a window position. `inside` is whether the pointer
    /// was on the canvas before, if known.
    pub fn motion(
        &mut self,
        translator: &mut Translator,
        input: &mut Input,
        inside: &mut Option<bool>,
        x: i32,
        y: i32,
    ) {
        let ([x, y], on) = self.point(x, y);
        if self.rect.is_none() {
            translator.mouse_motion(input, x, y);
            return;
        }
        if on {
            if *inside == Some(false) {
                input.mouse.mouse_entered(x, y);
            }
            *inside = Some(true);
        } else {
            if *inside != Some(false) {
                translator.exit(input);
            }
            *inside = Some(false);
            if translator.held == Held::default() {
                // mouseMoved over the letterbox belongs to the container.
                return;
            }
        }
        translator.mouse_motion(input, x, y);
    }

    /// A button went down at a window position.
    pub fn down(
        &mut self,
        translator: &mut Translator,
        input: &mut Input,
        button: MouseButton,
        x: i32,
        y: i32,
        now: i64,
    ) {
        let ([x, y], on) = self.point(x, y);
        if on {
            translator.mouse_down(input, button, x, y, now);
        } else {
            match button {
                MouseButton::Left => self.elsewhere.left = true,
                MouseButton::Middle => self.elsewhere.middle = true,
                MouseButton::Right => self.elsewhere.right = true,
                MouseButton::Unknown => {}
            }
        }
    }

    /// A button went up.
    pub fn up(&mut self, translator: &mut Translator, input: &mut Input, button: MouseButton) {
        let elsewhere = match button {
            MouseButton::Left => &mut self.elsewhere.left,
            MouseButton::Middle => &mut self.elsewhere.middle,
            MouseButton::Right => &mut self.elsewhere.right,
            MouseButton::Unknown => return,
        };
        if std::mem::take(elsewhere) {
            return;
        }
        translator.mouse_up(input, button);
    }
}

struct Replay<'a> {
    translator: &'a mut Translator,
    input: &'a mut Input,
    now: i64,
    scale: f32,
    canvas: &'a mut Canvas,
    inside: &'a mut Option<bool>,
}

impl Replay<'_> {
    fn point(&self, x: f32, y: f32) -> (i32, i32) {
        ((x / self.scale) as i32, (y / self.scale) as i32)
    }
}

impl EventHandler for Replay<'_> {
    fn update(&mut self) {}
    fn draw(&mut self) {}
    fn mouse_motion_event(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.canvas
            .motion(self.translator, self.input, self.inside, x, y);
    }
    fn mouse_button_down_event(&mut self, button: MouseButton, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.canvas
            .down(self.translator, self.input, button, x, y, self.now);
    }
    fn mouse_button_up_event(&mut self, button: MouseButton, _x: f32, _y: f32) {
        self.canvas.up(self.translator, self.input, button);
    }
    fn char_event(&mut self, character: char, _modifiers: KeyMods, _repeat: bool) {
        self.translator.character(self.input, character);
    }
    fn key_down_event(&mut self, key: KeyCode, _modifiers: KeyMods, _repeat: bool) {
        self.translator.key_down(self.input, key);
    }
    fn key_up_event(&mut self, key: KeyCode, _modifiers: KeyMods) {
        self.translator.key_up(self.input, key);
    }
    // macroquad already replays a touch as mouse events; the trait's default
    // would add them a second time.
    fn touch_event(&mut self, _phase: TouchPhase, _id: u64, _x: f32, _y: f32) {}
    fn window_minimized_event(&mut self) {
        self.translator.focus(self.input, false);
    }
    fn window_restored_event(&mut self) {
        self.translator.focus(self.input, true);
    }
}

/// The running client's event source. Needs macroquad's window.
pub struct Events {
    subscriber: usize,
    translator: Translator,
    /// Whether the pointer was over the window (the canvas, when that is
    /// smaller) at the last poll.
    inside: Option<bool>,
    /// Where the canvas is within the window; the owner of the window keeps
    /// `canvas.rect` current.
    pub canvas: Canvas,
    /// A development aid: the pointer is where the window's own events put
    /// it and the system cursor is not consulted (see [`posted_input`]).
    posted: bool,
}

/// Whether `RS09_POSTED_INPUT` is set: a test harness then drives the
/// window with posted mouse and key messages while the system cursor is
/// elsewhere, so the once-a-frame question "is the cursor over the window"
/// (which would answer no and take the pointer away again) is not asked. The
/// events themselves take the same path as a person's.
fn posted_input() -> bool {
    cfg!(not(target_arch = "wasm32")) && std::env::var_os("RS09_POSTED_INPUT").is_some()
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

impl Events {
    pub fn new() -> Self {
        Self {
            subscriber: macroquad::input::utils::register_input_subscriber(),
            translator: Translator::new(Host::current()),
            inside: None,
            canvas: Canvas::default(),
            posted: posted_input(),
        }
    }

    /// Hand `input` every event since the last call, in arrival order. Call
    /// once per rendered frame, before the frame's logic cycles. `now` is the
    /// clock clicks are stamped with, in milliseconds; all of a frame's
    /// clicks get the same value.
    pub fn pump(&mut self, input: &mut Input, now: i64) {
        let scale = macroquad::miniquad::window::dpi_scale();
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let mut replay = Replay {
            translator: &mut self.translator,
            input,
            now,
            scale,
            canvas: &mut self.canvas,
            inside: &mut self.inside,
        };
        macroquad::input::utils::repeat_all_miniquad_input(&mut replay, self.subscriber);
        // Over the window but beside the canvas is outside too.
        let pointer = match pointer_in_window(scale) {
            _ if self.posted => Pointer::Unknown,
            Pointer::Inside(point) => match self.canvas.point(point[0], point[1]) {
                (point, true) => Pointer::Inside(point),
                (_, false) => Pointer::Outside,
            },
            other => other,
        };
        match pointer {
            Pointer::Outside => {
                if self.inside != Some(false) {
                    self.translator.exit(input);
                }
                self.inside = Some(false);
            }
            Pointer::Inside(point) => {
                if self.inside == Some(false) {
                    input.mouse.mouse_entered(point[0], point[1]);
                }
                self.inside = Some(true);
            }
            Pointer::Unknown => {}
        }
    }
}

/// What the window system says about the pointer, apart from its events.
enum Pointer {
    /// Over the window's client area, at this interface pixel.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    Inside([i32; 2]),
    #[cfg_attr(
        not(any(target_os = "windows", target_arch = "wasm32")),
        allow(dead_code)
    )]
    Outside,
    /// This system cannot say, or nothing happened.
    Unknown,
}

#[cfg(target_os = "windows")]
fn pointer_in_window(scale: f32) -> Pointer {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
        fn WindowFromPoint(point: Point) -> *mut c_void;
        fn GetWindowThreadProcessId(window: *mut c_void, process: *mut u32) -> u32;
        fn ScreenToClient(window: *mut c_void, point: *mut Point) -> i32;
        fn GetClientRect(window: *mut c_void, rect: *mut Rect) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    // The frame comparison with the original (oracle/refcompare) drives the
    // client with posted window messages while the real pointer is elsewhere:
    // with RS09_POSTED_POINTER set, the poll reports nothing.
    static POSTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *POSTED.get_or_init(|| std::env::var_os("RS09_POSTED_POINTER").is_some()) {
        return Pointer::Unknown;
    }
    let mut point = Point::default();
    let mut area = Rect::default();
    // SAFETY: plain user32 queries with valid out-pointers. miniquad creates
    // its one window on the thread that runs the client's frame, so a window
    // of this thread under the cursor is the client's.
    unsafe {
        if GetCursorPos(&mut point) == 0 {
            return Pointer::Unknown;
        }
        let window = WindowFromPoint(point);
        if window.is_null()
            || GetWindowThreadProcessId(window, std::ptr::null_mut()) != GetCurrentThreadId()
        {
            return Pointer::Outside;
        }
        if ScreenToClient(window, &mut point) == 0 || GetClientRect(window, &mut area) == 0 {
            return Pointer::Unknown;
        }
    }
    let inside = point.x >= area.left
        && point.y >= area.top
        && point.x < area.right
        && point.y < area.bottom;
    if inside {
        // With `high_dpi` miniquad reports client pixels; macroquad divides
        // them by the DPI scale, as `Replay::point` does.
        Pointer::Inside([
            (point.x as f32 / scale) as i32,
            (point.y as f32 / scale) as i32,
        ])
    } else {
        Pointer::Outside
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {static EXITED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };}

/// The page's `mouseleave` listener on the canvas calls this.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn rs09_mouse_exit() {
    EXITED.with(|exited| exited.set(true));
}

/// A leave is applied after the frame's other events; the next motion event
/// brings the pointer back.
#[cfg(target_arch = "wasm32")]
fn pointer_in_window(_scale: f32) -> Pointer {
    if EXITED.with(|exited| exited.replace(false)) {
        Pointer::Outside
    } else {
        Pointer::Unknown
    }
}

#[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
fn pointer_in_window(_scale: f32) -> Pointer {
    Pointer::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::keyboard::{KEY_BACK_SPACE, KEY_CTRL, KEY_ENTER, KEY_SHIFT};

    #[test]
    fn every_key_the_original_maps_is_reachable() {
        let input = Input::default();
        let map = input.keyboard.code_map();
        let code = |key| map.get(virtual_key(key) as usize).copied().unwrap_or(-1);
        assert_eq!(
            [KeyCode::F1, KeyCode::F8, KeyCode::F12].map(code),
            [1, 8, 12]
        );
        assert_eq!(code(KeyCode::Escape), 13);
        assert_eq!(
            [KeyCode::Key1, KeyCode::Key9, KeyCode::Key0].map(code),
            [16, 24, 25]
        );
        assert_eq!([KeyCode::Q, KeyCode::A, KeyCode::Z].map(code), [32, 48, 64]);
        assert_eq!(
            [KeyCode::LeftShift, KeyCode::RightShift].map(code),
            [KEY_SHIFT as i32; 2]
        );
        assert_eq!(code(KeyCode::RightControl), KEY_CTRL as i32);
        assert_eq!(
            [KeyCode::Enter, KeyCode::KpEnter].map(code),
            [KEY_ENTER as i32; 2]
        );
        assert_eq!(code(KeyCode::Backspace), KEY_BACK_SPACE as i32);
        assert_eq!(
            [
                KeyCode::Left,
                KeyCode::Right,
                KeyCode::Up,
                KeyCode::Down,
                KeyCode::Insert,
                KeyCode::Delete,
                KeyCode::Home,
                KeyCode::End,
                KeyCode::PageUp,
                KeyCode::PageDown
            ]
            .map(code),
            [96, 97, 98, 99, 100, 101, 102, 103, 104, 105]
        );
        assert_eq!(
            [KeyCode::Kp0, KeyCode::Kp5, KeyCode::Kp9].map(code),
            [228, 219, 232]
        );
        assert_eq!(
            [
                KeyCode::Minus,
                KeyCode::Equal,
                KeyCode::GraveAccent,
                KeyCode::Apostrophe,
                KeyCode::Slash
            ]
            .map(code),
            [26, 27, 28, 58, 73]
        );
        assert_eq!(
            [
                KeyCode::Tab,
                KeyCode::Space,
                KeyCode::LeftAlt,
                KeyCode::KpAdd,
                KeyCode::KpSubtract,
                KeyCode::KpMultiply,
                KeyCode::KpDivide,
                KeyCode::KpDecimal
            ]
            .map(code),
            [80, 83, 86, 87, 88, 89, 90, 229]
        );
        // Keys AWT has no entry for in the table stay unmapped.
        assert_eq!(
            [KeyCode::CapsLock, KeyCode::LeftSuper, KeyCode::Unknown].map(code),
            [-1; 3]
        );
        // Together the keys reach every code the table holds (bit 7 marks
        // the number pad), except Clear and the UK layout's number sign.
        use KeyCode::*;
        let reached: std::collections::BTreeSet<i32> = [
            Space,
            Apostrophe,
            Comma,
            Minus,
            Period,
            Slash,
            Key0,
            Key1,
            Key2,
            Key3,
            Key4,
            Key5,
            Key6,
            Key7,
            Key8,
            Key9,
            Semicolon,
            Equal,
            A,
            B,
            C,
            D,
            E,
            F,
            G,
            H,
            I,
            J,
            K,
            L,
            M,
            N,
            O,
            P,
            Q,
            R,
            S,
            T,
            U,
            V,
            W,
            X,
            Y,
            Z,
            LeftBracket,
            Backslash,
            RightBracket,
            GraveAccent,
            World1,
            World2,
            Escape,
            Enter,
            Tab,
            Backspace,
            Insert,
            Delete,
            Right,
            Left,
            Down,
            Up,
            PageUp,
            PageDown,
            Home,
            End,
            CapsLock,
            ScrollLock,
            NumLock,
            PrintScreen,
            Pause,
            F1,
            F2,
            F3,
            F4,
            F5,
            F6,
            F7,
            F8,
            F9,
            F10,
            F11,
            F12,
            F13,
            F24,
            F25,
            Kp0,
            Kp1,
            Kp2,
            Kp3,
            Kp4,
            Kp5,
            Kp6,
            Kp7,
            Kp8,
            Kp9,
            KpDecimal,
            KpDivide,
            KpMultiply,
            KpSubtract,
            KpAdd,
            KpEnter,
            KpEqual,
            LeftShift,
            LeftControl,
            LeftAlt,
            LeftSuper,
            RightShift,
            RightControl,
            RightAlt,
            RightSuper,
            Menu,
            Back,
            Unknown,
        ]
        .into_iter()
        .map(code)
        .collect();
        let table: std::collections::BTreeSet<i32> = map.iter().copied().collect();
        let missing: Vec<_> = table.difference(&reached).collect();
        assert_eq!(missing, [&59, &91], "{missing:?}");
    }

    #[test]
    fn typed_characters_follow_awt() {
        let typed = |host, events: &[(Option<KeyCode>, Option<char>)]| {
            let mut input = Input::default();
            let mut translator = Translator::new(host);
            for &(key, character) in events {
                if let Some(key) = key {
                    translator.key_down(&mut input, key);
                }
                if let Some(character) = character {
                    translator.character(&mut input, character);
                }
            }
            input.cycle();
            input.keys.clone()
        };
        // Windows: WM_KEYDOWN then WM_CHAR; Enter arrives as a carriage
        // return and Delete has no character.
        assert_eq!(
            typed(
                Host::Native,
                &[
                    (Some(KeyCode::A), Some('a')),
                    (Some(KeyCode::Enter), Some('\r')),
                    (Some(KeyCode::Backspace), Some('\u{8}')),
                    (Some(KeyCode::Delete), None),
                    (Some(KeyCode::F4), None),
                    (None, Some('\u{20ac}')),
                    (None, Some('\u{100}')),
                ]
            ),
            [
                [48, -1],
                [-1, 97],
                [84, -1],
                [-1, 10],
                [85, -1],
                [-1, 8],
                [101, -1],
                [-1, 127],
                [4, -1],
                [-1, 128]
            ]
        );
        // Browser: no character for Backspace, Tab, Escape or Delete.
        assert_eq!(
            typed(
                Host::Browser,
                &[
                    (Some(KeyCode::Backspace), None),
                    (Some(KeyCode::Tab), None),
                    (Some(KeyCode::Escape), None),
                    (Some(KeyCode::Delete), None),
                    (Some(KeyCode::Enter), Some('\r')),
                ]
            ),
            [
                [85, -1],
                [-1, 8],
                [80, -1],
                [-1, 9],
                [13, -1],
                [-1, 27],
                [101, -1],
                [-1, 127],
                [84, -1],
                [-1, 10]
            ]
        );
    }

    #[test]
    fn the_browser_page_reports_the_back_quote_key_as_keypad_equals() {
        // Not from the Java: miniquad 0.4.11's `translate_keycode` has no
        // number that gives `GraveAccent` (96, which its page script sends
        // for "Backquote", gives `Apostrophe`), so the client's page sends
        // 336 for that key. Key codes as in `Keyboard.KEY_CODES`: back quote
        // 28, quote 58, slash 73.
        let events = |host, keys: &[KeyCode]| {
            let mut input = Input::default();
            let mut translator = Translator::new(host);
            for &key in keys {
                translator.key_down(&mut input, key);
            }
            input.cycle();
            let typed = input.keys.clone();
            let held = [28, 58, 73].map(|code: usize| input.keyboard.pressed_keys[code]);
            for &key in keys {
                translator.key_up(&mut input, key);
            }
            input.cycle();
            let released = [28, 58, 73].map(|code: usize| input.keyboard.pressed_keys[code]);
            (typed, held, released)
        };
        // In a browser keypad-equals stands for the back-quote key, pressed
        // and released; the quote and slash keys are themselves.
        assert_eq!(
            events(Host::Browser, &[KeyCode::KpEqual]),
            (vec![[28, -1]], [true, false, false], [false; 3])
        );
        assert_eq!(
            events(
                Host::Browser,
                &[KeyCode::Apostrophe, KeyCode::Slash, KeyCode::GraveAccent]
            ),
            (vec![[58, -1], [73, -1], [28, -1]], [true; 3], [false; 3])
        );
        // On the desktop keypad-equals is a key of its own, with no code.
        assert_eq!(
            events(Host::Native, &[KeyCode::KpEqual]),
            (vec![], [false; 3], [false; 3])
        );
        assert_eq!(
            events(Host::Native, &[KeyCode::GraveAccent]),
            (vec![[28, -1]], [true, false, false], [false; 3])
        );
    }

    #[test]
    fn mouse_events_become_listener_calls() {
        let mut input = Input::default();
        let mut translator = Translator::new(Host::Native);
        translator.mouse_motion(&mut input, 10, 20);
        translator.mouse_down(&mut input, MouseButton::Left, 11, 21, 500);
        translator.mouse_motion(&mut input, 12, 22);
        input.cycle();
        let mouse = &input.mouse;
        assert_eq!(
            (mouse.click_button, mouse.click_x, mouse.click_y),
            (1, 11, 21)
        );
        assert_eq!(
            (mouse.click_time, mouse.pressed_button, mouse.last_mouse_x),
            (500, 1, 12)
        );
        // The middle button does not click, and freezes the pointer while held.
        translator.mouse_down(&mut input, MouseButton::Middle, 12, 22, 600);
        translator.mouse_motion(&mut input, 90, 90);
        input.cycle();
        let mouse = &input.mouse;
        assert_eq!(
            (mouse.click_button, mouse.pressed_button, mouse.last_mouse_x),
            (0, 1, 12)
        );
        // Other buttons still click while it is held.
        translator.mouse_down(&mut input, MouseButton::Right, 90, 91, 700);
        input.cycle();
        let mouse = &input.mouse;
        assert_eq!(
            (mouse.click_button, mouse.click_x, mouse.pressed_button),
            (2, 90, 2)
        );
        assert_eq!(mouse.last_mouse_x, 12);
        translator.mouse_up(&mut input, MouseButton::Middle);
        translator.mouse_up(&mut input, MouseButton::Right);
        translator.mouse_up(&mut input, MouseButton::Left);
        translator.mouse_down(&mut input, MouseButton::Right, 30, 40, 800);
        translator.mouse_down(&mut input, MouseButton::Unknown, 1, 1, 900);
        translator.exit(&mut input);
        input.cycle();
        let mouse = &input.mouse;
        assert_eq!(
            (mouse.click_button, mouse.click_x, mouse.click_time),
            (2, 30, 800)
        );
        assert_eq!((mouse.last_mouse_x, mouse.last_mouse_y), (-1, -1));
        // Losing focus releases the held button and, a cycle later, the keys.
        translator.key_down(&mut input, KeyCode::LeftControl);
        input.cycle();
        assert!(input.keyboard.pressed_keys[KEY_CTRL]);
        assert_eq!(input.modifiers(), [false, true, false]);
        translator.focus(&mut input, false);
        input.cycle();
        assert_eq!(input.mouse.pressed_button, 0);
        assert!(!input.keyboard.pressed_keys[KEY_CTRL] && !input.focus);
        assert_eq!(translator.held, Held::default());
    }

    #[test]
    fn a_canvas_inside_the_window_gets_its_own_events_at_its_own_positions() {
        // Transcription test: the listeners are added to GameShell.canvas
        // (Mouse.start), which the fixed modes place at leftMargin
        // (DisplayMode.java:126-131, 154-158). AWT gives a component the
        // moves and presses over it, relative to its corner, and goes on
        // sending it the drags and the release of a press it received.
        let mut input = Input::default();
        let mut translator = Translator::new(Host::Native);
        let mut inside = None;
        // The fixed canvas in a 1024 by 768 frame.
        let mut canvas = Canvas {
            rect: Some([129, 0, 765, 503]),
            ..Canvas::default()
        };
        assert_eq!(canvas.point(129, 0), ([0, 0], true));
        assert_eq!(canvas.point(893, 502), ([764, 502], true));
        assert_eq!(canvas.point(894, 502), ([765, 502], false));
        assert_eq!(canvas.point(500, 503), ([371, 503], false));
        assert_eq!(canvas.point(128, 10), ([-1, 10], false));

        canvas.motion(&mut translator, &mut input, &mut inside, 500, 300);
        input.cycle();
        assert_eq!(
            (input.mouse.last_mouse_x, input.mouse.last_mouse_y),
            (371, 300)
        );
        // Over the letterbox: mouseExited once, and nothing more.
        canvas.motion(&mut translator, &mut input, &mut inside, 50, 300);
        input.cycle();
        assert_eq!(
            (input.mouse.last_mouse_x, input.mouse.last_mouse_y),
            (-1, -1)
        );
        assert_eq!(input.mouse.idle_loops(), 1);
        canvas.motion(&mut translator, &mut input, &mut inside, 60, 300);
        input.cycle();
        assert_eq!(input.mouse.idle_loops(), 2);
        // A press there is not the canvas's, nor is its release.
        canvas.down(&mut translator, &mut input, MouseButton::Left, 60, 300, 100);
        canvas.up(&mut translator, &mut input, MouseButton::Left);
        input.cycle();
        assert_eq!(input.mouse.click_button, 0);
        assert_eq!(input.mouse.idle_loops(), 3);
        assert_eq!(translator.held, Held::default());
        // Back on the canvas: mouseEntered, then the move.
        canvas.motion(&mut translator, &mut input, &mut inside, 130, 5);
        assert_eq!(inside, Some(true));
        // A press on it, dragged off its edge and released there.
        canvas.down(&mut translator, &mut input, MouseButton::Left, 139, 7, 200);
        canvas.motion(&mut translator, &mut input, &mut inside, 100, 600);
        input.cycle();
        let mouse = &input.mouse;
        assert_eq!(
            (mouse.click_button, mouse.click_x, mouse.click_y),
            (1, 10, 7)
        );
        assert_eq!((mouse.last_mouse_x, mouse.last_mouse_y), (-29, 600));
        assert_eq!(mouse.pressed_button, 1);
        canvas.up(&mut translator, &mut input, MouseButton::Left);
        input.cycle();
        assert_eq!(input.mouse.pressed_button, 0);

        // The whole window as the canvas: positions as they come.
        let mut whole = Canvas::default();
        let mut inside = None;
        whole.motion(&mut translator, &mut input, &mut inside, 5, 6);
        whole.down(&mut translator, &mut input, MouseButton::Right, 5, 6, 300);
        input.cycle();
        let mouse = &input.mouse;
        assert_eq!(
            (mouse.click_button, mouse.click_x, mouse.click_y),
            (2, 5, 6)
        );
        assert_eq!(inside, None);
    }
}
