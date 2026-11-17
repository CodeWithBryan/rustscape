//! Window modes: `DisplayMode` and the canvas half of `GameShell` (rev 530).
//!
//! The original draws on a canvas inside a container: its own frame, the
//! applet, or a full-screen frame. `DisplayMode.getWindowMode()` names four
//! modes:
//!
//! | mode | renderer | canvas |
//! |---|---|---|
//! | 0 | software | 765 by 503 |
//! | 1 | hardware | 765 by 503 |
//! | 2 | hardware | the container |
//! | 3 | hardware | the full-screen frame |
//!
//! In modes 0 and 1 the canvas sits at the top of the container, centred
//! across it: `leftMargin = (frameWidth - 765) / 2` (Java division, so it
//! truncates toward zero and goes negative in a narrower container) and
//! `topMargin = 0`; `GameShell.paintFrameLetterbox` fills the rest black.
//! Interfaces are laid out to the canvas and mouse positions are the
//! canvas's own.
//!
//! [`Display`] is that state and [`Display::set_window_mode`] is
//! `DisplayMode.setWindowMode` statement for statement, with what it does to
//! the rest of the client recorded as [`Effect`]s in the original's order.
//! The window system's answers are in [`System`], so the whole machine runs
//! without a window; `tests/display.rs` replays it against the unchanged
//! Java class.
//!
//! This client has one renderer, the hardware one. [`System::software`]
//! false states that: a request for mode 0 is then carried out as mode 1,
//! the same canvas with the renderer it has, and `getWindowMode()` answers 1.
use crate::ui::vm::Preferences;

/// The fixed canvas of modes 0 and 1 (DisplayMode.java:129-130).
pub const FIXED: [i32; 2] = [765, 503];

/// The frame `GameShell.startApplication` opens (GameShell.java:686-687).
pub const APPLICATION_FRAME: [i32; 2] = [1024, 768];

/// One `rt4.DisplayMode`: a mode of the screen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Mode {
    pub width: i32,
    pub height: i32,
    pub bit_depth: i32,
    pub refresh_rate: i32,
}

/// How `SignLink.enterFullScreen` ends (DisplayMode.java:362-374).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Enter {
    /// A frame, status 1.
    #[default]
    Frame,
    /// No frame.
    NoFrame,
    /// A frame with status 2: it is given back with `exitFullScreen`.
    Refused,
}

/// What the window system and the renderers answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct System {
    /// `SignLink.isFullScreenSupported()`.
    pub full_screen_supported: bool,
    /// `SignLink.getDisplayModes()`; None is a failed request (status 2).
    pub modes: Option<Vec<Mode>>,
    pub enter: Enter,
    /// `GlRenderer.init` leaves the renderer enabled.
    pub renderer_starts: bool,
    /// The software renderer of mode 0 exists. Without it mode 0 is carried
    /// out as mode 1 and the hardware renderer is taken to start.
    pub software: bool,
}

impl System {
    /// The original's own: both renderers, and a screen that does what it
    /// is asked.
    pub fn original(modes: Vec<Mode>) -> Self {
        Self {
            full_screen_supported: true,
            modes: Some(modes),
            enter: Enter::Frame,
            renderer_starts: true,
            software: true,
        }
    }

    /// This client: the hardware renderer only, and full screen at the
    /// modes given (none where the window system offers none).
    pub fn client(modes: Vec<Mode>) -> Self {
        Self {
            full_screen_supported: !modes.is_empty(),
            software: false,
            ..Self::original(modes)
        }
    }
}

/// What `setWindowMode` and `updateCanvasSize` do outside this state, in the
/// order they do it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    /// `GlRenderer.quit()` (DisplayMode.java:82-84).
    QuitRenderer,
    /// `DisplayMode.exitFullScreen`: the screen returns to its own mode and
    /// the full-screen frame is disposed of.
    ExitFullScreen,
    /// `SignLink.enterFullScreen(bitDepth, height, width)`.
    EnterFullScreen {
        width: i32,
        height: i32,
        bit_depth: i32,
    },
    /// `Preferences.write` (DisplayMode.java:94).
    WritePreferences,
    /// The canvas is replaced: `client.addCanvas()` between stopping and
    /// starting the input listeners (DisplayMode.java:137-148).
    ReplaceCanvas,
    /// The canvas is given this size and moved to these margins; `renderer`
    /// when `GlRenderer.setCanvasSize` went first (DisplayMode.java:149-159,
    /// GameShell.java:212-221).
    PlaceCanvas {
        renderer: bool,
        canvas: [i32; 2],
        left: i32,
        top: i32,
    },
    /// `GlRenderer.createAndDestroyContext` (DisplayMode.java:164-166).
    ResetContext,
    /// Before the hardware renderer's first start the software frame buffer
    /// is cleared, the loading text is drawn on it and the letterbox is
    /// painted (DisplayMode.java:172-217).
    LoadingText,
    /// `GlRenderer.init(canvas, antiAliasingMode * 2)`.
    StartRenderer { samples: i32 },
    /// The block that follows the hardware renderer's first start
    /// (DisplayMode.java:230-238).
    HighDetail,
    /// The block that follows a return to the software renderer
    /// (DisplayMode.java:239-264).
    StandardDetail,
    /// `client.reloadResourcesOnDisplayModeChange()`.
    ReloadResources,
    /// `InterfaceList.layoutTopLevel(true)`, which the original does when a
    /// top-level interface is open (DisplayMode.java:270-272,
    /// GameShell.java:222-224).
    Layout,
    /// `ClientProt.sendWindowDetails()`, which the original does with the
    /// game socket open in game state 30 or 25 (DisplayMode.java:273-275).
    WindowStatus,
    /// Every rectangle marked dirty and a full redraw asked for
    /// (DisplayMode.java:276-279).
    Redraw,
    /// `GameShell.paintFrameLetterbox()`: at the end of `updateCanvasSize`,
    /// and after the loading text of the first hardware start.
    Letterbox,
}

/// The display state: `GameShell`'s frame and canvas, `DisplayMode`'s flags
/// and `GlRenderer.enabled`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Display {
    /// `GameShell.frameWidth`/`frameHeight`: the container as last measured.
    pub frame: [i32; 2],
    /// `GameShell.canvasWidth`/`canvasHeight`.
    pub canvas: [i32; 2],
    /// `GameShell.leftMargin`.
    pub left_margin: i32,
    /// `GameShell.topMargin`; the original only ever stores 0.
    pub top_margin: i32,
    /// The window's client area now: the size of `GameShell.frame` less its
    /// insets, or of the applet. The owner of the window keeps it current.
    pub window: [i32; 2],
    /// `GameShell.fullScreenFrame`, by its size.
    full_screen: Option<[i32; 2]>,
    /// `DisplayMode.resizable`.
    resizable: bool,
    /// `GlRenderer.enabled`.
    renderer: bool,
    /// `DisplayMode.start_GLRenderer`: the hardware renderer has been
    /// started before.
    renderer_started: bool,
    /// `DisplayMode.cachedDisplayModes`.
    cached_modes: Option<Vec<Mode>>,
    /// `GameShell.replaceCanvas`: the canvas is to be replaced at the next
    /// redraw. CS2 6016 sets it with the hardware renderer on
    /// (ScriptRunner.java:5182-5184), as `::replacecanvas` does; a replaced
    /// canvas clears it (GameShell.java:353).
    pub replace_canvas: bool,
    pub system: System,
    /// What the calls so far did, oldest first, for the owner to carry out
    /// and clear.
    pub effects: Vec<Effect>,
}

impl Default for Display {
    /// The standalone client's frame with no screen modes known.
    fn default() -> Self {
        Self::new(APPLICATION_FRAME, System::client(Vec::new()))
    }
}

impl Display {
    /// `GameShell.startApplication` (GameShell.java:684-690) for a frame
    /// whose client area is `window`: the canvas is the whole frame and
    /// neither renderer is on, so `getWindowMode()` is 0 until the first
    /// `setWindowMode`. (`startApplet` has the same shape at 765 by 503.)
    pub fn new(window: [i32; 2], system: System) -> Self {
        Self {
            frame: window,
            canvas: window,
            left_margin: 0,
            top_margin: 0,
            window,
            full_screen: None,
            resizable: false,
            renderer: false,
            renderer_started: false,
            cached_modes: None,
            replace_canvas: false,
            system,
            effects: Vec::new(),
        }
    }

    /// `DisplayMode.getWindowMode()` (DisplayMode.java:68-78). The
    /// reference checkout's own `resizableSD` flag is never set and is left
    /// out.
    pub fn window_mode(&self) -> i32 {
        if self.full_screen.is_some() {
            3
        } else if self.renderer && self.resizable {
            2
        } else if self.renderer {
            1
        } else {
            0
        }
    }

    /// `GameShell.fullScreenFrame != null`.
    pub fn full_screen(&self) -> bool {
        self.full_screen.is_some()
    }

    /// `GlRenderer.enabled`.
    pub fn renderer(&self) -> bool {
        self.renderer
    }

    /// Everything the differential test compares, in the fixture's order:
    /// the window mode, frame, canvas and margins, whether a full-screen
    /// frame exists, `GlRenderer.enabled`, `DisplayMode.resizable`, the
    /// saved full-screen size and the count of preference writes,
    /// `GameShell.replaceCanvas` and `DisplayMode.start_GLRenderer`.
    pub fn observation(&self, preferences: &Preferences) -> [i32; 15] {
        [
            self.window_mode(),
            self.frame[0],
            self.frame[1],
            self.canvas[0],
            self.canvas[1],
            self.left_margin,
            self.top_margin,
            i32::from(self.full_screen.is_some()),
            i32::from(self.renderer),
            i32::from(self.resizable),
            preferences.full_screen_width,
            preferences.full_screen_height,
            preferences.writes as i32,
            i32::from(self.replace_canvas),
            i32::from(self.renderer_started),
        ]
    }

    /// Mode 0 as this system carries it out.
    fn carried(&self, mode: i32) -> i32 {
        if mode == 0 && !self.system.software {
            1
        } else {
            mode
        }
    }

    /// The container: the full-screen frame if there is one, else the
    /// window (DisplayMode.java:105-120, GameShell.java:185-200).
    fn measure(&mut self) {
        self.frame = self.full_screen.unwrap_or(self.window);
    }

    fn placed(&self) -> Effect {
        Effect::PlaceCanvas {
            renderer: self.renderer,
            canvas: self.canvas,
            left: self.left_margin,
            top: self.top_margin,
        }
    }

    fn fixed_canvas(&mut self) {
        self.top_margin = 0;
        // Java's `/` truncates toward zero, as Rust's does.
        self.left_margin = self.frame[0].wrapping_sub(FIXED[0]) / 2;
        self.canvas = FIXED;
    }

    fn whole_canvas(&mut self) {
        self.canvas = self.frame;
        self.left_margin = 0;
        self.top_margin = 0;
    }

    /// `DisplayMode.setWindowMode(replaceCanvas, newMode, width, height)`
    /// (DisplayMode.java:54-65). `width` and `height` matter in mode 3 only;
    /// everywhere else the original passes -1.
    ///
    /// The caller zeroes `DisplayMode.canvasReplaceTime` (line 55), which
    /// lives with the window-status timer.
    pub fn set_window_mode(
        &mut self,
        mut replace_canvas: bool,
        new_mode: i32,
        width: i32,
        height: i32,
        preferences: &mut Preferences,
    ) {
        let new_mode = self.carried(new_mode);
        let current = self.window_mode();
        if new_mode == 3 || current == 3 {
            replace_canvas = true;
        }
        let mut use_hd = (current > 0) != (new_mode > 0);
        if replace_canvas && new_mode > 0 {
            use_hd = true;
        }
        self.apply(
            replace_canvas,
            new_mode,
            use_hd,
            current,
            width,
            height,
            preferences,
        );
    }

    /// The six-argument `DisplayMode.setWindowMode` (DisplayMode.java:81-281).
    #[allow(clippy::too_many_arguments)]
    fn apply(
        &mut self,
        replace_canvas: bool,
        new_mode: i32,
        use_hd: bool,
        current: i32,
        width: i32,
        height: i32,
        preferences: &mut Preferences,
    ) {
        let new_mode = self.carried(new_mode);
        if use_hd {
            self.renderer = false;
            self.effects.push(Effect::QuitRenderer);
        }
        if self.full_screen.is_some()
            && (new_mode != 3
                || width != preferences.full_screen_width
                || height != preferences.full_screen_height)
        {
            self.effects.push(Effect::ExitFullScreen);
            self.full_screen = None;
        }
        if new_mode == 3 && self.full_screen.is_none() {
            self.full_screen = self.create_full_screen_frame(0, height, width);
            if self.full_screen.is_some() {
                preferences.full_screen_height = height;
                preferences.full_screen_width = width;
                preferences.writes = preferences.writes.wrapping_add(1);
                self.effects.push(Effect::WritePreferences);
            }
        }
        // No full-screen frame could be made: the remembered mode instead.
        if new_mode == 3 && self.full_screen.is_none() {
            let remembered = preferences.remembered_mode;
            self.apply(true, remembered, true, current, -1, -1, preferences);
            return;
        }
        self.measure();
        if new_mode >= 2 {
            self.whole_canvas();
        } else {
            self.fixed_canvas();
        }
        if replace_canvas {
            self.effects.push(Effect::ReplaceCanvas);
            self.replace_canvas = false;
        } else {
            self.effects.push(self.placed());
        }
        if new_mode == 0 && current > 0 {
            self.effects.push(Effect::ResetContext);
        }
        if use_hd && new_mode > 0 {
            if !self.renderer_started {
                self.effects.push(Effect::LoadingText);
                self.effects.push(Effect::Letterbox);
                self.renderer_started = true;
            }
            self.effects.push(Effect::StartRenderer {
                samples: preferences.anti_aliasing.wrapping_mul(2),
            });
            self.renderer = self.system.renderer_starts || !self.system.software;
        }
        // The hardware renderer did not start: the software one instead.
        if !self.renderer && new_mode > 0 {
            self.apply(true, 0, true, current, -1, -1, preferences);
            return;
        }
        if new_mode > 0 && current == 0 {
            self.effects.push(Effect::HighDetail);
        } else if new_mode == 0 && current > 0 {
            self.effects.push(Effect::StandardDetail);
        }
        if use_hd {
            self.effects.push(Effect::ReloadResources);
        }
        // Resizable in mode 2 only, not in full screen.
        self.resizable = new_mode == 2;
        self.effects.push(Effect::Layout);
        self.effects.push(Effect::WindowStatus);
        self.effects.push(Effect::Redraw);
    }

    /// The first half of `client.mainRedraw`'s canvas watch
    /// (client.java:817-819), in game state 30 or 10: a canvas asked to be
    /// replaced gets `setWindowMode(true, getWindowMode(), fullScreenWidth,
    /// fullScreenHeight)`, which restarts the renderer with the samples the
    /// anti-aliasing preference now names and reports the window. Returns
    /// whether it ran. (The other trigger, the window-size timer, calls it
    /// with `replaceCanvas` false; that half is
    /// `ClientEvents::redraw`.)
    pub fn redraw(&mut self, preferences: &mut Preferences) -> bool {
        if !self.replace_canvas {
            return false;
        }
        let mode = self.window_mode();
        let (width, height) = (
            preferences.full_screen_width,
            preferences.full_screen_height,
        );
        // Replacing the canvas (`addCanvas`) clears the flag.
        self.set_window_mode(true, mode, width, height, preferences);
        true
    }

    /// `GameShell.updateCanvasSize()` (GameShell.java:184-226): the
    /// container is measured again and the canvas follows the mode
    /// `getWindowMode()` names now. Unlike `setWindowMode` it asks the mode
    /// in force, so before the first `setWindowMode` (mode 0) it shrinks the
    /// start-up canvas to the fixed one.
    pub fn update_canvas_size(&mut self) {
        self.measure();
        if self.window_mode() >= 2 {
            self.whole_canvas();
        } else {
            self.fixed_canvas();
        }
        self.effects.push(self.placed());
        self.effects.push(Effect::Layout);
        self.effects.push(Effect::Letterbox);
    }

    /// The size watch of `client.mainRedraw` (client.java:822-842), once a
    /// drawn frame: a window whose client area is no longer the frame last
    /// measured gets `updateCanvasSize()`. Returns whether it did, in which
    /// case the original also starts the 500 ms window-status timer. With a
    /// full-screen frame nothing is watched.
    pub fn watch(&mut self, window: [i32; 2]) -> bool {
        self.window = window;
        if self.full_screen.is_some() || window == self.frame {
            return false;
        }
        self.update_canvas_size();
        true
    }

    /// The four bands `GameShell.paintFrameLetterbox` fills black
    /// (GameShell.java:229-269), as `[x, y, width, height]` within the
    /// container: left, top, right, bottom. A band is absent when it has no
    /// thickness, and all are when the canvas covers the container.
    pub fn letterbox(&self) -> [Option<[i32; 4]>; 4] {
        letterbox(self.frame, self.canvas, self.left_margin, self.top_margin)
    }

    /// `DisplayMode.getAvailableDisplayModes` (DisplayMode.java:319-341).
    fn available(&self) -> Vec<Mode> {
        if !self.system.full_screen_supported {
            return Vec::new();
        }
        self.system.modes.clone().unwrap_or_default()
    }

    /// `DisplayMode.getDisplayModes()` (DisplayMode.java:284-316): the modes
    /// of at least 800 by 600 and 24 bits (or of unstated depth), one for
    /// each size (the deepest), in the order `ArrayUtils.sort` leaves them
    /// by area. Computed once.
    pub fn display_modes(&mut self) -> &[Mode] {
        if self.cached_modes.is_none() {
            let mut filtered: Vec<Mode> = Vec::new();
            'next: for mode in self.available() {
                if (mode.bit_depth <= 0 || mode.bit_depth >= 24)
                    && mode.width >= 800
                    && mode.height >= 600
                {
                    for existing in &mut filtered {
                        if mode.width == existing.width && existing.height == mode.height {
                            if mode.bit_depth > existing.bit_depth {
                                *existing = mode;
                            }
                            continue 'next;
                        }
                    }
                    filtered.push(mode);
                }
            }
            let mut keys: Vec<i32> = filtered
                .iter()
                .map(|mode| mode.height.wrapping_mul(mode.width))
                .collect();
            let last = keys.len() as i32 - 1;
            sort(&mut filtered, last, &mut keys, 0);
            self.cached_modes = Some(filtered);
        }
        self.cached_modes.as_deref().unwrap_or_default()
    }

    /// `DisplayMode.createFullScreenFrame(0, height, width, signLink)`
    /// (DisplayMode.java:344-375): the frame's size, if the screen has a
    /// mode of exactly this size and enters it.
    fn create_full_screen_frame(
        &mut self,
        mut bit_depth: i32,
        height: i32,
        width: i32,
    ) -> Option<[i32; 2]> {
        if !self.system.full_screen_supported {
            return None;
        }
        let mut found = false;
        for mode in self.available() {
            if width == mode.width
                && height == mode.height
                && (!found || mode.bit_depth > bit_depth)
            {
                bit_depth = mode.bit_depth;
                found = true;
            }
        }
        if !found {
            return None;
        }
        self.effects.push(Effect::EnterFullScreen {
            width,
            height,
            bit_depth,
        });
        match self.system.enter {
            Enter::Frame => Some([width, height]),
            Enter::NoFrame => None,
            Enter::Refused => {
                self.effects.push(Effect::ExitFullScreen);
                None
            }
        }
    }

    /// CS2 5300 (ScriptRunner.java:4649-4656): full screen at this size;
    /// pushes whether a full-screen frame exists afterwards.
    pub fn script_full_screen(
        &mut self,
        width: i32,
        height: i32,
        preferences: &mut Preferences,
    ) -> i32 {
        self.set_window_mode(false, 3, width, height, preferences);
        i32::from(self.full_screen.is_some())
    }

    /// `if (GameShell.fullScreenFrame != null) setWindowMode(false,
    /// Preferences.favoriteWorlds, -1, -1)`: CS2 5301, 5411 and 5421, the
    /// URL_OPEN packet (Protocol.java:1830-1832) and a full-screen frame
    /// that lost the focus (client.java:849-851). Returns whether it left.
    pub fn leave_full_screen(&mut self, preferences: &mut Preferences) -> bool {
        if self.full_screen.is_none() {
            return false;
        }
        let remembered = preferences.remembered_mode;
        self.set_window_mode(false, remembered, -1, -1, preferences);
        true
    }

    /// CS2 5303 (ScriptRunner.java:4668-4675): a mode's width and height.
    /// The original indexes the array unchecked; None is its exception.
    pub fn mode_size(&mut self, index: i32) -> Option<[i32; 2]> {
        let modes = self.display_modes();
        usize::try_from(index)
            .ok()
            .and_then(|index| modes.get(index))
            .map(|mode| [mode.width, mode.height])
    }

    /// CS2 5305 (ScriptRunner.java:4676-4690): the index of the mode last
    /// used for full screen, or -1.
    pub fn preferred_mode(&mut self, preferences: &Preferences) -> i32 {
        let (width, height) = (
            preferences.full_screen_width,
            preferences.full_screen_height,
        );
        self.display_modes()
            .iter()
            .position(|mode| width == mode.width && mode.height == height)
            .map_or(-1, |index| index as i32)
    }

    /// CS2 5307 (ScriptRunner.java:4695-4704): a mode outside 0 to 2 is 0.
    pub fn script_window_mode(&mut self, mode: i32, preferences: &mut Preferences) {
        let mode = if (0..=2).contains(&mode) { mode } else { 0 };
        self.set_window_mode(false, mode, -1, -1, preferences);
    }

    /// A position in the container as the canvas sees it: mouse events are
    /// the canvas's own (Mouse.java), so they are relative to its corner.
    pub fn to_canvas(&self, point: [i32; 2]) -> [i32; 2] {
        [
            point[0].wrapping_sub(self.left_margin),
            point[1].wrapping_sub(self.top_margin),
        ]
    }
}

/// `GameShell.paintFrameLetterbox` (GameShell.java:229-269) for a canvas at
/// these margins in a container of size `frame`; see [`Display::letterbox`].
pub fn letterbox(frame: [i32; 2], canvas: [i32; 2], left: i32, top: i32) -> [Option<[i32; 4]>; 4] {
    let [frame_width, frame_height] = frame;
    let bottom = frame_height.wrapping_sub(canvas[1]).wrapping_sub(top);
    let right = frame_width.wrapping_sub(left).wrapping_sub(canvas[0]);
    [
        (left > 0).then_some([0, 0, left, frame_height]),
        (top > 0).then_some([0, 0, frame_width, top]),
        (right > 0).then_some([frame_width.wrapping_sub(right), 0, right, frame_height]),
        (bottom > 0).then_some([0, frame_height.wrapping_sub(bottom), frame_width, bottom]),
    ]
}

/// `ArrayUtils.sort(Object[] values, int hi, int[] keys, int lo)`
/// (ArrayUtils.java:356-384): the original's quicksort, whose order among
/// equal keys is its own.
fn sort<T>(values: &mut [T], hi: i32, keys: &mut [i32], lo: i32) {
    if lo >= hi {
        return;
    }
    let (low, high) = (lo as usize, hi as usize);
    let mid = ((lo + hi) / 2) as usize;
    let pivot = keys[mid];
    let mut i = low;
    keys[mid] = keys[high];
    keys[high] = pivot;
    values.swap(mid, high);
    for j in low..high {
        if ((j as i32) & 1).wrapping_add(pivot) > keys[j] {
            keys.swap(j, i);
            values.swap(j, i);
            i += 1;
        }
    }
    keys[high] = keys[i];
    keys[i] = pivot;
    values.swap(high, i);
    sort(values, i as i32 - 1, keys, lo);
    sort(values, hi, keys, i as i32 + 1);
}

#[cfg(test)]
mod tests {
    //! Transcription tests: the expected values are worked by hand from the
    //! Java lines cited. `tests/display.rs` holds the JVM differential.
    use super::*;

    fn mode(width: i32, height: i32, bit_depth: i32) -> Mode {
        Mode {
            width,
            height,
            bit_depth,
            refresh_rate: 60,
        }
    }

    fn original(window: [i32; 2]) -> Display {
        Display::new(
            window,
            System::original(vec![mode(1024, 768, 32), mode(1920, 1080, 32)]),
        )
    }

    fn placed(renderer: bool, canvas: [i32; 2], left: i32) -> Effect {
        Effect::PlaceCanvas {
            renderer,
            canvas,
            left,
            top: 0,
        }
    }

    fn client(window: [i32; 2]) -> Display {
        Display::new(window, System::client(vec![mode(1920, 1080, 32)]))
    }

    #[test]
    fn the_application_starts_with_the_canvas_over_its_whole_frame() {
        // GameShell.java:684-690; GlRenderer.enabled and
        // DisplayMode.resizable start false, so getWindowMode() is 0.
        let display = Display::new(APPLICATION_FRAME, System::original(Vec::new()));
        assert_eq!(display.frame, [1024, 768]);
        assert_eq!(display.canvas, [1024, 768]);
        assert_eq!((display.left_margin, display.top_margin), (0, 0));
        assert_eq!(display.window_mode(), 0);
        assert_eq!(display.letterbox(), [None; 4]);
    }

    #[test]
    fn the_fixed_canvas_is_centred_across_the_frame_and_at_its_top() {
        // DisplayMode.java:121-131 with the frame GameShell opens.
        let preferences = &mut Preferences::default();
        let mut display = original(APPLICATION_FRAME);
        display.set_window_mode(false, 1, -1, -1, preferences);
        assert_eq!(display.window_mode(), 1);
        assert_eq!(display.canvas, FIXED);
        // (1024 - 765) / 2 = 129 (259 / 2 truncates).
        assert_eq!((display.left_margin, display.top_margin), (129, 0));
        // paintFrameLetterbox: left 129, right 1024 - 129 - 765 = 130,
        // bottom 768 - 503 = 265, no top band.
        assert_eq!(
            display.letterbox(),
            [
                Some([0, 0, 129, 768]),
                None,
                Some([894, 0, 130, 768]),
                Some([0, 503, 1024, 265]),
            ]
        );
        assert_eq!(display.to_canvas([129, 0]), [0, 0]);
        assert_eq!(display.to_canvas([500, 300]), [371, 300]);
    }

    #[test]
    fn a_frame_narrower_or_shorter_than_the_canvas_cuts_it() {
        // (700 - 765) / 2 is -32 in Java (-65 / 2 truncates toward zero).
        let preferences = &mut Preferences::default();
        let mut display = original([700, 400]);
        display.set_window_mode(false, 1, -1, -1, preferences);
        assert_eq!(display.left_margin, -32);
        assert_eq!(display.canvas, FIXED);
        // left -32, right 700 + 32 - 765 = -33, bottom 400 - 503 = -103:
        // nothing is painted.
        assert_eq!(display.letterbox(), [None; 4]);
        assert_eq!(display.to_canvas([0, 0]), [32, 0]);
        // An exact fit has no margin either.
        display.watch([765, 503]);
        assert_eq!(display.left_margin, 0);
        assert_eq!(display.letterbox(), [None; 4]);
        // One pixel wider: 1 / 2 = 0 on the left, the odd pixel on the right.
        display.watch([766, 503]);
        assert_eq!(display.left_margin, 0);
        assert_eq!(
            display.letterbox(),
            [None, None, Some([765, 0, 1, 503]), None]
        );
    }

    #[test]
    fn the_resizable_canvas_is_the_frame_whatever_its_size() {
        // DisplayMode.java:121-125. Nothing gives the frame a minimum size.
        let preferences = &mut Preferences::default();
        let mut display = original([1280, 800]);
        display.set_window_mode(false, 2, -1, -1, preferences);
        assert_eq!(display.window_mode(), 2);
        assert_eq!(display.canvas, [1280, 800]);
        assert_eq!(display.left_margin, 0);
        assert!(display.watch([300, 200]));
        assert_eq!(display.canvas, [300, 200]);
        assert_eq!(display.letterbox(), [None; 4]);
        // The same size again is not a change.
        assert!(!display.watch([300, 200]));
    }

    #[test]
    fn a_resize_before_the_first_mode_shrinks_the_start_up_canvas() {
        // GameShell.updateCanvasSize asks getWindowMode(), which is 0 while
        // the client loads (GameShell.java:201-211).
        let mut display = original(APPLICATION_FRAME);
        assert_eq!(display.canvas, [1024, 768]);
        assert!(display.watch([1100, 800]));
        assert_eq!(display.canvas, FIXED);
        assert_eq!(display.left_margin, (1100 - 765) / 2);
        assert_eq!(
            display.effects,
            [placed(false, FIXED, 167), Effect::Layout, Effect::Letterbox]
        );
    }

    #[test]
    fn the_first_hardware_mode_starts_the_renderer_and_reports() {
        // From mode 0: useHD (0 > 0) != (2 > 0), DisplayMode.java:60.
        let preferences = &mut Preferences {
            anti_aliasing: 2,
            ..Preferences::default()
        };
        let mut display = original([1280, 800]);
        display.set_window_mode(false, 2, -1, -1, preferences);
        assert_eq!(
            display.effects,
            [
                Effect::QuitRenderer,
                placed(false, [1280, 800], 0),
                Effect::LoadingText,
                Effect::Letterbox,
                Effect::StartRenderer { samples: 4 },
                Effect::HighDetail,
                Effect::ReloadResources,
                Effect::Layout,
                Effect::WindowStatus,
                Effect::Redraw,
            ]
        );
        // Between the hardware modes nothing restarts.
        display.effects.clear();
        display.set_window_mode(false, 1, -1, -1, preferences);
        assert_eq!(
            display.effects,
            [
                placed(true, FIXED, 257),
                Effect::Layout,
                Effect::WindowStatus,
                Effect::Redraw,
            ]
        );
        // The same mode again still lays out and reports (line 273).
        display.effects.clear();
        display.set_window_mode(false, 1, -1, -1, preferences);
        assert_eq!(display.effects.len(), 4);
        assert_eq!(display.window_mode(), 1);
    }

    #[test]
    fn mode_zero_returns_to_the_software_renderer_where_there_is_one() {
        let preferences = &mut Preferences::default();
        let mut display = original([1024, 768]);
        display.set_window_mode(false, 2, -1, -1, preferences);
        display.effects.clear();
        display.set_window_mode(false, 0, -1, -1, preferences);
        assert_eq!(display.window_mode(), 0);
        assert_eq!(display.canvas, FIXED);
        assert_eq!(
            display.effects,
            [
                Effect::QuitRenderer,
                placed(false, FIXED, 129),
                Effect::ResetContext,
                Effect::StandardDetail,
                Effect::ReloadResources,
                Effect::Layout,
                Effect::WindowStatus,
                Effect::Redraw,
            ]
        );
    }

    #[test]
    fn without_a_software_renderer_mode_zero_is_carried_out_as_mode_one() {
        let preferences = &mut Preferences::default();
        let mut display = client([1024, 768]);
        assert_eq!(display.window_mode(), 0);
        // client.mainLoad state 150 with the default remembered mode 0.
        display.set_window_mode(false, 0, -1, -1, preferences);
        assert_eq!(display.window_mode(), 1);
        assert_eq!(display.canvas, FIXED);
        assert_eq!(display.left_margin, 129);
        display.set_window_mode(false, 2, -1, -1, preferences);
        assert_eq!(display.window_mode(), 2);
        display.effects.clear();
        display.script_window_mode(0, preferences);
        assert_eq!(display.window_mode(), 1);
        // No renderer change between hardware modes.
        assert_eq!(
            display.effects,
            [
                placed(true, FIXED, 129),
                Effect::Layout,
                Effect::WindowStatus,
                Effect::Redraw,
            ]
        );
        // CS2 5307 takes anything outside 0..=2 as 0.
        display.script_window_mode(2, preferences);
        display.script_window_mode(7, preferences);
        assert_eq!(display.window_mode(), 1);
        display.script_window_mode(-1, preferences);
        assert_eq!(display.window_mode(), 1);
    }

    #[test]
    fn full_screen_takes_a_mode_the_screen_has_and_remembers_it() {
        let preferences = &mut Preferences {
            remembered_mode: 2,
            ..Preferences::default()
        };
        let mut display = original([1024, 768]);
        display.set_window_mode(false, 2, -1, -1, preferences);
        display.effects.clear();
        // CS2 5300.
        assert_eq!(display.script_full_screen(1920, 1080, preferences), 1);
        assert_eq!(display.window_mode(), 3);
        assert!(display.full_screen());
        assert_eq!(display.frame, [1920, 1080]);
        assert_eq!(display.canvas, [1920, 1080]);
        assert_eq!(
            (
                preferences.full_screen_width,
                preferences.full_screen_height
            ),
            (1920, 1080)
        );
        assert_eq!(preferences.writes, 1);
        assert_eq!(
            display.effects,
            [
                Effect::QuitRenderer,
                Effect::EnterFullScreen {
                    width: 1920,
                    height: 1080,
                    bit_depth: 32
                },
                Effect::WritePreferences,
                Effect::ReplaceCanvas,
                Effect::StartRenderer { samples: 0 },
                Effect::ReloadResources,
                Effect::Layout,
                Effect::WindowStatus,
                Effect::Redraw,
            ]
        );
        // The window is not watched while the full-screen frame exists.
        assert!(!display.watch([640, 480]));
        assert_eq!(display.canvas, [1920, 1080]);
        // The same mode again keeps the frame (DisplayMode.java:85).
        display.effects.clear();
        assert_eq!(display.script_full_screen(1920, 1080, preferences), 1);
        assert!(!display.effects.contains(&Effect::ExitFullScreen));
        assert_eq!(preferences.writes, 1);
        // CS2 5301 returns to the remembered mode, in the window as it is
        // now.
        display.effects.clear();
        assert!(display.leave_full_screen(preferences));
        assert_eq!(display.window_mode(), 2);
        assert_eq!(display.canvas, [640, 480]);
        assert_eq!(
            display.effects[..2],
            [Effect::QuitRenderer, Effect::ExitFullScreen]
        );
        assert!(!display.leave_full_screen(preferences));
    }

    #[test]
    fn full_screen_that_cannot_be_had_ends_in_the_remembered_mode() {
        // DisplayMode.java:101-103, reached four ways.
        for (system, entered) in [
            // No mode of that size.
            (System::original(vec![mode(1280, 1024, 32)]), false),
            // Full screen is not supported at all.
            (
                System {
                    full_screen_supported: false,
                    ..System::original(vec![mode(1024, 768, 32)])
                },
                false,
            ),
            // The mode list request failed.
            (
                System {
                    modes: None,
                    ..System::original(Vec::new())
                },
                false,
            ),
            // The frame was not made.
            (
                System {
                    enter: Enter::NoFrame,
                    ..System::original(vec![mode(1024, 768, 32)])
                },
                true,
            ),
            (
                System {
                    enter: Enter::Refused,
                    ..System::original(vec![mode(1024, 768, 32)])
                },
                true,
            ),
        ] {
            let refused = system.enter == Enter::Refused;
            let preferences = &mut Preferences {
                remembered_mode: 1,
                ..Preferences::default()
            };
            let mut display = Display::new([900, 600], system);
            display.set_window_mode(false, 2, -1, -1, preferences);
            display.effects.clear();
            // `::wm3`.
            display.set_window_mode(false, 3, 1024, 768, preferences);
            assert_eq!(display.window_mode(), 1);
            assert_eq!(display.canvas, FIXED);
            assert_eq!(preferences.writes, 0);
            assert_eq!(preferences.full_screen_width, 0);
            let attempt = Effect::EnterFullScreen {
                width: 1024,
                height: 768,
                bit_depth: 32,
            };
            assert_eq!(display.effects.contains(&attempt), entered);
            assert_eq!(display.effects.contains(&Effect::ExitFullScreen), refused);
            // The renderer was stopped twice and started once: the retry
            // passes useHD again.
            let quits = display
                .effects
                .iter()
                .filter(|effect| **effect == Effect::QuitRenderer)
                .count();
            assert_eq!(quits, 2);
            assert_eq!(*display.effects.last().unwrap(), Effect::Redraw);
        }
    }

    #[test]
    fn a_hardware_renderer_that_does_not_start_ends_in_mode_zero() {
        // DisplayMode.java:222-226.
        let preferences = &mut Preferences::default();
        let mut display = Display::new(
            [1024, 768],
            System {
                renderer_starts: false,
                ..System::original(Vec::new())
            },
        );
        display.set_window_mode(false, 2, -1, -1, preferences);
        assert_eq!(display.window_mode(), 0);
        assert_eq!(display.canvas, FIXED);
        assert!(!display.renderer());
    }

    #[test]
    fn a_canvas_asked_to_be_replaced_restarts_the_renderer_and_reports() {
        // ScriptRunner.java:5179-5190 (CS2 6016), then client.java:817-819.
        let preferences = &mut Preferences::default();
        let mut display = client([1024, 768]);
        assert!(!display.redraw(preferences));
        display.set_window_mode(false, 2, -1, -1, preferences);
        display.effects.clear();
        display.replace_canvas = true;
        preferences.anti_aliasing = 2;
        assert!(display.redraw(preferences));
        assert_eq!(display.window_mode(), 2);
        assert_eq!(
            display.effects,
            [
                Effect::QuitRenderer,
                Effect::ReplaceCanvas,
                Effect::StartRenderer { samples: 4 },
                Effect::ReloadResources,
                Effect::Layout,
                Effect::WindowStatus,
                Effect::Redraw,
            ]
        );
        // Once: the new canvas cleared the request.
        assert!(!display.replace_canvas);
        assert!(!display.redraw(preferences));
    }

    #[test]
    fn the_screen_modes_are_filtered_merged_and_sorted_as_the_original_does() {
        // DisplayMode.java:292: at least 800 by 600, 24 bits or unstated.
        let mut display = Display::new(
            [1024, 768],
            System::original(vec![
                mode(1920, 1080, 32),
                mode(640, 480, 32),
                mode(800, 600, 16),
                mode(800, 600, 24),
                mode(1024, 768, 24),
                mode(1024, 768, 32),
                mode(1280, 599, 32),
                mode(799, 1000, 32),
                mode(1024, 768, 24),
                mode(1280, 1024, -1),
            ]),
        );
        let sizes: Vec<_> = display
            .display_modes()
            .iter()
            .map(|mode| (mode.width, mode.height, mode.bit_depth))
            .collect();
        assert_eq!(
            sizes,
            [
                (800, 600, 24),
                (1024, 768, 32),
                (1280, 1024, -1),
                (1920, 1080, 32)
            ]
        );
        // CS2 5302, 5303 and 5305.
        assert_eq!(display.display_modes().len(), 4);
        assert_eq!(display.mode_size(1), Some([1024, 768]));
        assert_eq!(display.mode_size(4), None);
        assert_eq!(display.mode_size(-1), None);
        let mut preferences = Preferences::default();
        assert_eq!(display.preferred_mode(&preferences), -1);
        preferences.full_screen_width = 1920;
        preferences.full_screen_height = 1080;
        assert_eq!(display.preferred_mode(&preferences), 3);
        // The list is computed once (cachedDisplayModes).
        display.system.modes = Some(Vec::new());
        assert_eq!(display.display_modes().len(), 4);
        // Unsupported: none.
        let mut none = Display::new(
            [1024, 768],
            System {
                full_screen_supported: false,
                ..System::original(vec![mode(1024, 768, 32)])
            },
        );
        assert!(none.display_modes().is_empty());
    }

    #[test]
    fn the_original_sort_orders_by_area() {
        // ArrayUtils.java:356-384 over a hand-run case: keys 5 3 9 1.
        // mid = 1, pivot 3: swap with hi -> 5 1 9 3; j=0: (0&1)+3 > 5 no;
        // j=1: 1+3 > 1 yes, swap(1,0) -> 1 5 9 3, i=1; j=2: 0+3 > 9 no;
        // place pivot: 1 3 9 5; right part (2,3): mid 2, pivot 9 -> 5 9;
        // j=2: 0+9 > 5 yes, i=3; -> 1 3 5 9.
        let mut keys = vec![5, 3, 9, 1];
        let mut values = vec!['a', 'b', 'c', 'd'];
        sort(&mut values, 3, &mut keys, 0);
        assert_eq!(keys, [1, 3, 5, 9]);
        assert_eq!(values, ['d', 'b', 'a', 'c']);
    }
}
