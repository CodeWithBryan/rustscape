//! RT4 `Keyboard`: the AWT key listener and its per-cycle `loop`.
//!
//! The listener half ([`Keyboard::key_pressed`], [`Keyboard::key_typed`],
//! [`Keyboard::key_released`], [`Keyboard::focus_lost`]) takes what AWT hands
//! the original: a `KeyEvent` virtual-key code or a typed character. The
//! client half ([`Keyboard::cycle`], [`Keyboard::next_key`]) runs once per
//! 20 ms logic cycle. Both halves keep the original's two rings:
//!
//! - 128 press/release events feeding [`Keyboard::pressed_keys`]. When the
//!   ring fills, or focus is lost, the next cycle releases every key and drops
//!   what was queued.
//! - 128 typed entries, `(code, -1)` for a press and `(-1, char)` for a typed
//!   character, in arrival order. A held key arrives again for every OS
//!   auto-repeat. Entries not read during a cycle are dropped by the next.
//!
//! `fixtures/input/keyboard.txt` holds what the unchanged class did with real
//! `KeyEvent` objects; `tests/input.rs` replays it here.

pub const KEY_SHIFT: usize = 81;
pub const KEY_CTRL: usize = 82;
pub const KEY_ENTER: usize = 84;
pub const KEY_BACK_SPACE: usize = 85;
pub const KEY_ALT: usize = 86;
pub const KEY_LEFT: usize = 96;
pub const KEY_RIGHT: usize = 97;
pub const KEY_UP: usize = 98;
pub const KEY_DOWN: usize = 99;

/// `java.awt.event.KeyEvent` virtual-key codes the original names.
pub mod vk {
    pub const BACK_SPACE: i32 = 8;
    pub const TAB: i32 = 9;
    pub const ENTER: i32 = 10;
    pub const CLEAR: i32 = 12;
    pub const SHIFT: i32 = 16;
    pub const CONTROL: i32 = 17;
    pub const ALT: i32 = 18;
    pub const PAUSE: i32 = 19;
    pub const CAPS_LOCK: i32 = 20;
    pub const ESCAPE: i32 = 27;
    pub const SPACE: i32 = 32;
    pub const PAGE_UP: i32 = 33;
    pub const PAGE_DOWN: i32 = 34;
    pub const END: i32 = 35;
    pub const HOME: i32 = 36;
    pub const LEFT: i32 = 37;
    pub const UP: i32 = 38;
    pub const RIGHT: i32 = 39;
    pub const DOWN: i32 = 40;
    pub const COMMA: i32 = 44;
    pub const MINUS: i32 = 45;
    pub const PERIOD: i32 = 46;
    pub const SLASH: i32 = 47;
    pub const KEY_0: i32 = 48;
    pub const SEMICOLON: i32 = 59;
    pub const EQUALS: i32 = 61;
    pub const A: i32 = 65;
    pub const OPEN_BRACKET: i32 = 91;
    pub const BACK_SLASH: i32 = 92;
    pub const CLOSE_BRACKET: i32 = 93;
    pub const NUMPAD0: i32 = 96;
    pub const MULTIPLY: i32 = 106;
    pub const ADD: i32 = 107;
    pub const SUBTRACT: i32 = 109;
    pub const DECIMAL: i32 = 110;
    pub const DIVIDE: i32 = 111;
    pub const F1: i32 = 112;
    pub const DELETE: i32 = 127;
    pub const NUM_LOCK: i32 = 144;
    pub const SCROLL_LOCK: i32 = 145;
    pub const PRINTSCREEN: i32 = 154;
    pub const INSERT: i32 = 155;
    pub const BACK_QUOTE: i32 = 192;
    pub const QUOTE: i32 = 222;
    pub const NUMBER_SIGN: i32 = 520;
    pub const WINDOWS: i32 = 524;
    pub const CONTEXT_MENU: i32 = 525;
    pub const F13: i32 = 0xF000;
    pub const UNDEFINED: i32 = 0;
}

/// Which branch of `Keyboard.init` fills the punctuation keys.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Vendor {
    /// A Java 1.4+ runtime (`setFocusTraversalKeysEnabled` exists): what the
    /// reference client runs on.
    Modern,
    /// A runtime without `setFocusTraversalKeysEnabled`.
    Legacy,
    /// The Microsoft VM, which reports Windows virtual keys.
    Microsoft,
}

pub const CODE_MAP_LENGTH: usize = 521;

fn code_map(vendor: Vendor) -> [i32; CODE_MAP_LENGTH] {
    let mut map = [-1; CODE_MAP_LENGTH];
    let mut set = |key: i32, code: i32| map[key as usize] = code;
    // The static initializer, in its order.
    set(vk::BACK_SPACE, 85);
    set(vk::TAB, 80);
    set(vk::ENTER, 84);
    set(vk::CLEAR, 91);
    set(vk::SHIFT, 81);
    set(vk::CONTROL, 82);
    set(vk::ALT, 86);
    set(vk::ESCAPE, 13);
    set(vk::SPACE, 83);
    set(vk::PAGE_UP, 104);
    set(vk::PAGE_DOWN, 105);
    set(vk::END, 103);
    set(vk::HOME, 102);
    set(vk::LEFT, 96);
    set(vk::UP, 98);
    set(vk::RIGHT, 97);
    set(vk::DOWN, 99);
    // VK_0 is 25, VK_1 to VK_9 are 16 to 24.
    set(vk::KEY_0, 25);
    for digit in 1..=9 {
        set(vk::KEY_0 + digit, 15 + digit);
    }
    // VK_A to VK_Z.
    for (letter, code) in [
        48, 68, 66, 50, 34, 51, 52, 53, 39, 54, 55, 56, 70, 69, 40, 41, 32, 35, 49, 36, 38, 67, 33,
        65, 37, 64,
    ]
    .into_iter()
    .enumerate()
    {
        set(vk::A + letter as i32, code);
    }
    // VK_NUMPAD0 to VK_NUMPAD9: bit 7 marks keys whose press is ignored.
    for (digit, code) in [228, 231, 227, 233, 224, 219, 225, 230, 226, 232]
        .into_iter()
        .enumerate()
    {
        set(vk::NUMPAD0 + digit as i32, code);
    }
    set(vk::MULTIPLY, 89);
    set(vk::ADD, 87);
    set(vk::SUBTRACT, 88);
    set(vk::DECIMAL, 229);
    set(vk::DIVIDE, 90);
    for function in 0..12 {
        set(vk::F1 + function, 1 + function);
    }
    set(vk::DELETE, 101);
    set(vk::INSERT, 100);
    // `init`.
    if vendor == Vendor::Microsoft {
        for (key, code) in [
            (187, 27),
            (223, 28),
            (221, 43),
            (188, 71),
            (222, 59),
            (192, 58),
            (191, 73),
            (219, 42),
            (190, 72),
            (186, 57),
            (220, 74),
            (189, 26),
        ] {
            set(key, code);
        }
        return map;
    }
    if vendor == Vendor::Legacy {
        set(vk::BACK_QUOTE, 58);
        set(vk::QUOTE, 59);
    } else {
        set(vk::QUOTE, 58);
        set(vk::BACK_QUOTE, 28);
        set(vk::NUMBER_SIGN, 59);
    }
    set(vk::MINUS, 26);
    set(vk::EQUALS, 27);
    set(vk::OPEN_BRACKET, 42);
    set(vk::SEMICOLON, 57);
    set(vk::CLOSE_BRACKET, 43);
    set(vk::COMMA, 71);
    set(vk::BACK_SLASH, 74);
    set(vk::PERIOD, 72);
    set(vk::SLASH, 73);
    map
}

/// `Keyboard.getKeyChar`: the euro sign is 128, anything else outside 1..=255
/// is not a character the client accepts.
pub fn key_char(c: i32) -> i32 {
    if c == 8364 {
        128
    } else if c <= 0 || c >= 256 {
        -1
    } else {
        c
    }
}

#[derive(Clone, Debug)]
pub struct Keyboard {
    code_map: [i32; CODE_MAP_LENGTH],
    event_queue: [i32; 128],
    typed_code_queue: [i32; 128],
    typed_char_queue: [i32; 128],
    /// `Keyboard.pressedKeys`, indexed by the client's key codes
    /// ([`KEY_SHIFT`] and the like). Changes only in [`Keyboard::cycle`].
    pub pressed_keys: [bool; 112],
    event_queue_writer_index: i32,
    event_queue_reader_index: i32,
    typed_queue_writer_index: i32,
    typed_queue_reader_index: i32,
    prev_typed_queue_writer_index: i32,
    idle_loops: i32,
    /// `Keyboard.keyCode` and `Keyboard.keyChar`, set by [`Keyboard::next_key`].
    pub key_code: i32,
    pub key_char: i32,
    /// False after [`Keyboard::quit`]: the listener then ignores every event.
    instance: bool,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self::new(Vendor::Modern)
    }
}

impl Keyboard {
    pub fn new(vendor: Vendor) -> Self {
        Self {
            code_map: code_map(vendor),
            event_queue: [0; 128],
            typed_code_queue: [0; 128],
            typed_char_queue: [0; 128],
            pressed_keys: [false; 112],
            event_queue_writer_index: 0,
            event_queue_reader_index: 0,
            typed_queue_writer_index: 0,
            typed_queue_reader_index: 0,
            prev_typed_queue_writer_index: 0,
            idle_loops: 0,
            key_code: 0,
            key_char: 0,
            instance: true,
        }
    }

    /// `CODE_MAP`: client key code by AWT virtual key, -1 where unmapped.
    pub fn code_map(&self) -> &[i32; CODE_MAP_LENGTH] {
        &self.code_map
    }

    /// `Keyboard.getKey`: whether the key with this AWT virtual-key code is
    /// held. The original indexes without checks; unmapped keys are not held.
    pub fn key(&self, vk: i32) -> bool {
        usize::try_from(vk)
            .ok()
            .and_then(|vk| self.code_map.get(vk))
            .and_then(|&code| usize::try_from(code).ok())
            .and_then(|code| self.pressed_keys.get(code))
            .copied()
            .unwrap_or(false)
    }

    /// Whether the key with this client key code is held. Codes outside the
    /// array (an exception in the original) are not held.
    pub fn held(&self, code: i32) -> bool {
        usize::try_from(code)
            .ok()
            .and_then(|code| self.pressed_keys.get(code))
            .copied()
            .unwrap_or(false)
    }

    /// `keyPressed` with `KeyEvent.getKeyCode()`. AWT calls this again for
    /// every auto-repeat of a held key.
    pub fn key_pressed(&mut self, vk: i32) {
        if !self.instance {
            return;
        }
        self.idle_loops = 0;
        let mut code = match usize::try_from(vk)
            .ok()
            .and_then(|vk| self.code_map.get(vk))
        {
            Some(&code) => code,
            None => -1,
        };
        if code & 0x80 != 0 {
            code = -1;
        }
        if self.event_queue_writer_index >= 0 && code >= 0 {
            self.event_queue[self.event_queue_writer_index as usize] = code;
            self.event_queue_writer_index = (self.event_queue_writer_index + 1) & 0x7F;
            if self.event_queue_writer_index == self.event_queue_reader_index {
                self.event_queue_writer_index = -1;
            }
        }
        if code >= 0 {
            let index = (self.typed_queue_writer_index + 1) & 0x7F;
            if index != self.typed_queue_reader_index {
                self.typed_code_queue[self.typed_queue_writer_index as usize] = code;
                self.typed_char_queue[self.typed_queue_writer_index as usize] = -1;
                self.typed_queue_writer_index = index;
            }
        }
    }

    /// `keyTyped` with `KeyEvent.getKeyChar()` as an integer (a UTF-16 unit
    /// in the original; any larger scalar is rejected the same way).
    pub fn key_typed(&mut self, c: i32) {
        if !self.instance {
            return;
        }
        let c = key_char(c);
        if c >= 0 {
            let index = (self.typed_queue_writer_index + 1) & 0x7F;
            if self.typed_queue_reader_index != index {
                self.typed_code_queue[self.typed_queue_writer_index as usize] = -1;
                self.typed_char_queue[self.typed_queue_writer_index as usize] = c;
                self.typed_queue_writer_index = index;
            }
        }
    }

    /// `keyReleased` with `KeyEvent.getKeyCode()`. Bit 7 of the mapped code is
    /// cleared rather than rejected, so releasing a number-pad key releases
    /// the navigation key it doubles as.
    pub fn key_released(&mut self, vk: i32) {
        if !self.instance {
            return;
        }
        self.idle_loops = 0;
        let c = match usize::try_from(vk)
            .ok()
            .and_then(|vk| self.code_map.get(vk))
        {
            Some(&code) => code & !0x80,
            None => -1,
        };
        if self.event_queue_writer_index >= 0 && c >= 0 {
            self.event_queue[self.event_queue_writer_index as usize] = !c;
            self.event_queue_writer_index = (self.event_queue_writer_index + 1) & 0x7F;
            if self.event_queue_reader_index == self.event_queue_writer_index {
                self.event_queue_writer_index = -1;
            }
        }
    }

    /// `focusLost`: the next cycle releases every key.
    pub fn focus_lost(&mut self) {
        if self.instance {
            self.event_queue_writer_index = -1;
        }
    }

    /// `stop`: the listener is detached and the next cycle releases every key.
    pub fn stop(&mut self) {
        self.event_queue_writer_index = -1;
    }

    /// `quit`: every later event is ignored. The original's `loop` and
    /// `nextKey` cannot run after it; here they keep working on what is left.
    pub fn quit(&mut self) {
        self.instance = false;
    }

    /// `Keyboard.loop`, once per logic cycle: apply queued presses and
    /// releases to [`Keyboard::pressed_keys`] and open this cycle's typed
    /// entries to [`Keyboard::next_key`].
    pub fn cycle(&mut self) {
        self.typed_queue_reader_index = self.prev_typed_queue_writer_index;
        self.idle_loops = self.idle_loops.wrapping_add(1);
        if self.event_queue_writer_index < 0 {
            self.pressed_keys = [false; 112];
            self.event_queue_writer_index = self.event_queue_reader_index;
        } else {
            while self.event_queue_writer_index != self.event_queue_reader_index {
                let key = self.event_queue[self.event_queue_reader_index as usize];
                self.event_queue_reader_index = (self.event_queue_reader_index + 1) & 0x7F;
                // Queued codes come from the code map with bit 7 clear; none
                // of its values reaches the end of the array.
                let (index, pressed) = if key >= 0 { (key, true) } else { (!key, false) };
                if let Some(slot) = self.pressed_keys.get_mut(index as usize) {
                    *slot = pressed;
                }
            }
        }
        self.prev_typed_queue_writer_index = self.typed_queue_writer_index;
    }

    /// `Keyboard.nextKey`: move the next typed entry of this cycle into
    /// [`Keyboard::key_code`] and [`Keyboard::key_char`].
    pub fn next_key(&mut self) -> bool {
        if self.prev_typed_queue_writer_index == self.typed_queue_reader_index {
            return false;
        }
        self.key_code = self.typed_code_queue[self.typed_queue_reader_index as usize];
        self.key_char = self.typed_char_queue[self.typed_queue_reader_index as usize];
        self.typed_queue_reader_index = (self.typed_queue_reader_index + 1) & 0x7F;
        true
    }

    /// `Keyboard.getIdleLoops`: cycles since the last press or release (typed
    /// characters do not count). 1 right after a cycle means a key event
    /// arrived since the cycle before.
    pub fn idle_loops(&self) -> i32 {
        self.idle_loops
    }

    /// Everything the differential test compares: the five ring indices, the
    /// idle counter and a running hash of the three rings.
    pub fn observation(&self) -> [i32; 7] {
        let mut hash = 0i32;
        for queue in [
            &self.event_queue,
            &self.typed_code_queue,
            &self.typed_char_queue,
        ] {
            for &value in queue {
                hash = hash.wrapping_mul(31).wrapping_add(value);
            }
        }
        [
            self.event_queue_writer_index,
            self.event_queue_reader_index,
            self.typed_queue_writer_index,
            self.typed_queue_reader_index,
            self.prev_typed_queue_writer_index,
            self.idle_loops,
            hash,
        ]
    }
}
