//! The window behind `rs09_client::display`: where the canvas lies in it,
//! and what the window system is asked to do.
//!
//! The original's canvas is a component of its own. Here one GL surface
//! covers the window, so a canvas smaller than the window (the fixed modes
//! in a larger frame) is a place on that surface: everything drawn and every
//! mouse position is moved by its margins, and `GameShell.paintFrameLetterbox`
//! blacks out the rest at the end of each frame.
//!
//! The modules that draw import [`screen_width`], [`screen_height`] and
//! [`mouse_position`] from here over macroquad's of the same names, so that
//! "the screen" is the canvas for them, and reach the surface through
//! [`camera`], [`scissor`] and the two offsets. While the canvas covers the
//! window all of these are macroquad's own values and calls.
//!
//! What differs from the original:
//! - Full screen is the window without its frame over the primary screen at
//!   the desktop's size (miniquad's `set_fullscreen`, Windows only). The
//!   modes on offer are the screen's own, as the original lists them
//!   (`rs09_client::full_screen::screen_modes`), but the screen's mode is
//!   never changed: a mode other than the desktop's is drawn the way the
//!   monitor would show it, the canvas at the mode's size scaled into the
//!   centre of the window with black bars around it, and mouse positions
//!   are mapped back to the mode's pixels (`rs09_client::full_screen::Fit`).
//!   The game sees the mode's size. In the browser the canvas element is put
//!   in full screen through the Fullscreen API at the screen's own size,
//!   the one size listed there (a page cannot list or choose modes); the
//!   browser grants it only just after a click or key press and can end it
//!   by itself (see [`full_screen_lost`]). Other native systems offer no
//!   mode at all.
//! - The renderer is never restarted. The sample count is fixed when the
//!   window is made (see [`samples`]).
use macroquad::{camera::Camera, prelude as mq};
use rs09_client::display::{Display, Effect, Mode, System};
use rs09_client::full_screen::Fit;
use std::cell::Cell;

/// The canvas as last placed: frame, canvas and margins, in the interface's
/// pixels.
#[derive(Clone, Copy)]
struct Placement {
    frame: [i32; 2],
    canvas: [i32; 2],
    left: i32,
    top: i32,
    /// The frame is the full-screen one: the screen mode's size.
    full_screen: bool,
}

impl Placement {
    /// The canvas is not the whole window.
    fn offset(&self) -> bool {
        self.left != 0 || self.top != 0 || self.canvas != self.frame
    }

    /// A full-screen mode other than the window's size, as the screen would
    /// show it in the window as it is now.
    fn fit(&self) -> Option<Fit> {
        self.full_screen
            .then(|| Fit::new(self.frame, window()))
            .flatten()
    }
}

thread_local! {
    static PLACEMENT: Cell<Option<Placement>> = const { Cell::new(None) };
    /// The window as it was before full screen: outer position and client
    /// size in physical pixels.
    static WINDOWED: Cell<Option<([u32; 2], [u32; 2])>> = const { Cell::new(None) };
    /// Full screen was just left: the size the window still has until the
    /// window system has put it back, and the frames that may take.
    static LEAVING: Cell<Option<([i32; 2], u32)>> = const { Cell::new(None) };
    /// The window was last asked to be full screen.
    static FULL_SCREEN: Cell<bool> = const { Cell::new(false) };
}

fn placement() -> Option<Placement> {
    PLACEMENT
        .with(Cell::get)
        .filter(|placement| placement.offset() || placement.fit().is_some())
}

/// The full-screen mode's picture in the window while it is not the
/// window's size; None otherwise.
pub fn fit() -> Option<Fit> {
    PLACEMENT
        .with(Cell::get)
        .and_then(|placement| placement.fit())
}

/// The window's client area in the interface's pixels: the container
/// `client.mainRedraw` measures.
pub fn window() -> [i32; 2] {
    [mq::screen_width() as i32, mq::screen_height() as i32]
}

/// The size of the window to hand `Display::watch` this frame, or None
/// while it is not the windowed frame's: with a full-screen frame the
/// original's own frame keeps its size out of sight (and is not watched,
/// client.java:822), and for the frame or two after full screen is left the
/// window is still on its way back to it.
pub fn windowed(display: &Display) -> Option<[i32; 2]> {
    let window = window();
    if display.full_screen() {
        return None;
    }
    if let Some((full, frames)) = LEAVING.with(Cell::get) {
        if window == full && frames > 0 {
            LEAVING.with(|leaving| leaving.set(Some((full, frames - 1))));
            return None;
        }
        LEAVING.with(|leaving| leaving.set(None));
    }
    Some(window)
}

/// Note where `display` has the canvas. Once per drawn frame, after the
/// size watch, and after anything that set the window mode.
pub fn place(display: &Display) {
    PLACEMENT.with(|placement| {
        placement.set(Some(Placement {
            frame: display.frame,
            canvas: display.canvas,
            left: display.left_margin,
            top: display.top_margin,
            full_screen: display.full_screen(),
        }))
    });
}

/// `GameShell.frameWidth`/`frameHeight` as last placed; the window itself
/// before that.
pub fn frame() -> [i32; 2] {
    PLACEMENT
        .with(Cell::get)
        .map_or_else(window, |placement| placement.frame)
}

/// `GameShell.canvasWidth`.
pub fn screen_width() -> f32 {
    placement().map_or_else(mq::screen_width, |placement| placement.canvas[0] as f32)
}

/// `GameShell.canvasHeight`.
pub fn screen_height() -> f32 {
    placement().map_or_else(mq::screen_height, |placement| placement.canvas[1] as f32)
}

/// The pointer relative to the canvas, as the original's listeners on the
/// canvas report it.
pub fn mouse_position() -> (f32, f32) {
    let (x, y) = mq::mouse_position();
    if let Some(fit) = fit() {
        // The mode's pixel under the pointer, as the input events map it.
        let [x, y] = fit.to_mode([x.floor() as i32, y.floor() as i32]);
        return (x as f32, y as f32);
    }
    match placement() {
        Some(placement) => (x - placement.left as f32, y - placement.top as f32),
        None => (x, y),
    }
}

/// The canvas within the window for the input events: `[left, top, width,
/// height]`, or None while it is the whole window.
pub fn canvas_rect() -> Option<[i32; 4]> {
    placement().map(|placement| {
        [
            placement.left,
            placement.top,
            placement.canvas[0],
            placement.canvas[1],
        ]
    })
}

/// The canvas's corner in the units the callers of `QuadGl::scissor` in the
/// interface renderer use (the interface's pixels).
fn corner() -> (i32, i32) {
    placement().map_or((0, 0), |placement| (placement.left, placement.top))
}

/// The canvas's corner in physical pixels from the window's top-left.
pub fn corner_px() -> (i32, i32) {
    let scale = mq::screen_dpi_scale();
    if let Some(fit) = fit() {
        let (x, y, ..) = fit.surface_rect((0, 0, 0, 0), scale);
        return (x, y);
    }
    let (left, top) = corner();
    (
        (left as f32 * scale).round() as i32,
        (top as f32 * scale).round() as i32,
    )
}

/// macroquad's default pixel camera, moved to the canvas.
struct Canvas2D {
    left: i32,
    top: i32,
}

impl Camera for Canvas2D {
    /// `Context::pixel_perfect_projection_matrix`.
    fn matrix(&self) -> mq::Mat4 {
        let (width, height) = mq::miniquad::window::screen_size();
        let dpi = mq::miniquad::window::dpi_scale();
        mq::Mat4::orthographic_rh_gl(0., width / dpi, height / dpi, 0., -1., 1.)
    }

    fn depth_enabled(&self) -> bool {
        false
    }

    fn render_pass(&self) -> Option<mq::RenderPass> {
        None
    }

    /// The whole surface's viewport shifted by the margins: GL clips what
    /// leaves the window, and a negative margin (a window narrower than the
    /// canvas) shifts the other way.
    fn viewport(&self) -> Option<(i32, i32, i32, i32)> {
        let (width, height) = mq::miniquad::window::screen_size();
        Some((self.left, -self.top, width as i32, height as i32))
    }
}

/// The pixel camera of a full-screen mode shown scaled: the mode's pixels
/// over its picture in the window, and nothing drawn outside it.
struct Scaled {
    mode: [i32; 2],
    /// The picture in the surface's pixels from its bottom-left corner.
    viewport: (i32, i32, i32, i32),
}

impl Camera for Scaled {
    fn matrix(&self) -> mq::Mat4 {
        mq::Mat4::orthographic_rh_gl(0., self.mode[0] as f32, self.mode[1] as f32, 0., -1., 1.)
    }

    fn depth_enabled(&self) -> bool {
        false
    }

    fn render_pass(&self) -> Option<mq::RenderPass> {
        None
    }

    fn viewport(&self) -> Option<(i32, i32, i32, i32)> {
        Some(self.viewport)
    }
}

/// A rectangle given on the scaled full-screen canvas, as a GL viewport: in
/// the surface's pixels from its bottom-left corner.
fn scaled_viewport(fit: &Fit, rect: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    let (x, y, width, height) = fit.surface_rect(rect, mq::screen_dpi_scale());
    let surface = mq::miniquad::window::screen_size().1 as i32;
    (x, surface - y - height, width, height)
}

/// The scene view's rectangle `[left, top, width, height]` on the canvas,
/// in the surface's pixels from its top-left corner, while a full-screen
/// mode is shown scaled (the scene camera's scissor and viewport); None
/// otherwise.
pub fn scaled_view_px(rect: [f32; 4]) -> Option<(i32, i32, i32, i32)> {
    let fit = fit()?;
    let [left, top, width, height] = rect.map(|n| n as i32);
    Some(fit.surface_rect((left, top, width, height), mq::screen_dpi_scale()))
}

/// The 2D camera of the canvas: `set_default_camera()` wherever the canvas
/// is the window.
pub fn camera() {
    if let Some(fit) = fit() {
        mq::set_camera(&Scaled {
            mode: fit.mode,
            viewport: scaled_viewport(&fit, (0, 0, fit.mode[0], fit.mode[1])),
        });
    } else if placement().is_some() {
        let (left, top) = corner_px();
        mq::set_camera(&Canvas2D { left, top });
    } else {
        mq::set_default_camera();
    }
}

/// A rectangle `(x, y, width, height)` given in the interface's pixels, in
/// the surface's. The two differ by the display's scale (a browser's device
/// pixel ratio, a desktop's scaling), which is 1 on an unscaled display.
/// Each edge is scaled and rounded by itself, so that rectangles sharing an
/// edge go on sharing it.
fn surface_rect((x, y, width, height): (i32, i32, i32, i32), scale: f32) -> (i32, i32, i32, i32) {
    let px = |edge: i32| (edge as f32 * scale).round() as i32;
    let (x0, y0, x1, y1) = (px(x), px(y), px(x + width), px(y + height));
    (x0, y0, x1 - x0, y1 - y0)
}

/// `QuadGl::scissor` for a rectangle given on the canvas in the interface's
/// pixels. `QuadGl` takes the surface's pixels.
pub fn scissor(clip: Option<(i32, i32, i32, i32)>) {
    let (left, top) = corner();
    let scale = mq::screen_dpi_scale();
    let fit = fit();
    // SAFETY: the macroquad context exists whenever a frame is drawn.
    unsafe {
        mq::get_internal_gl()
            .quad_gl
            .scissor(clip.map(|(x, y, width, height)| match &fit {
                Some(fit) => fit.surface_rect((x, y, width, height), scale),
                None => surface_rect((x + left, y + top, width, height), scale),
            }));
    }
}

/// A GL viewport for the whole canvas of this `size`, as the interface's
/// model renderer sets it (`GlRenderer.setViewportBounds` over the canvas):
/// in the surface's pixels from its bottom-left corner.
pub fn canvas_viewport(size: [i32; 2]) -> (i32, i32, i32, i32) {
    if let Some(fit) = fit() {
        return scaled_viewport(&fit, (0, 0, size[0], size[1]));
    }
    let (left, bottom) = placement().map_or((0, 0), |placement| {
        (placement.left, placement.frame[1] - placement.top - size[1])
    });
    surface_rect((left, bottom, size[0], size[1]), mq::screen_dpi_scale())
}

/// `GameShell.paintFrameLetterbox`: black over what the canvas leaves of the
/// window. The original paints it on the container, beside the canvas; here
/// it goes over the frame just drawn, which also cuts off whatever was drawn
/// past the canvas's edge. Leaves the canvas camera set.
pub fn letterbox() {
    let Some(placement) = placement() else {
        return;
    };
    // A scaled full-screen mode: the screen around its picture is black, as
    // a monitor shows it.
    let bands = match fit() {
        Some(fit) => fit.bars(),
        None => rs09_client::display::letterbox(
            placement.frame,
            placement.canvas,
            placement.left,
            placement.top,
        ),
    };
    if bands.iter().all(Option::is_none) {
        return;
    }
    mq::set_default_camera();
    // SAFETY: as in `scissor`.
    unsafe {
        mq::get_internal_gl().quad_gl.scissor(None);
    }
    for [x, y, width, height] in bands.into_iter().flatten() {
        mq::draw_rectangle(x as f32, y as f32, width as f32, height as f32, mq::BLACK);
    }
    camera();
}

/// The screen's size in the interface's pixels.
#[cfg(target_os = "windows")]
fn desktop() -> Option<[i32; 2]> {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetSystemMetrics(index: i32) -> i32;
    }
    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;
    // SAFETY: a plain user32 query. These are the metrics miniquad sizes
    // its full-screen window with.
    let (width, height) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
    let scale = mq::screen_dpi_scale();
    let scale = if scale > 0.0 { scale } else { 1.0 };
    (width > 0 && height > 0).then(|| {
        [
            (width as f32 / scale) as i32,
            (height as f32 / scale) as i32,
        ]
    })
}

/// The page glue (`web/visual/display_glue.js`): the screen and the
/// browser's Fullscreen API.
#[cfg(target_arch = "wasm32")]
mod page {
    unsafe extern "C" {
        pub fn rs09_screen_width() -> i32;
        pub fn rs09_screen_height() -> i32;
        pub fn rs09_full_screen(on: i32);
        pub fn rs09_full_screen_left() -> i32;
    }
}

/// The screen in the page's pixels. A browser offers full screen at the
/// screen's own size only (it never changes the screen's mode), so that is
/// the one mode listed, as on Windows.
#[cfg(target_arch = "wasm32")]
fn desktop() -> Option<[i32; 2]> {
    // SAFETY: plain calls into the page glue.
    let (width, height) = unsafe { (page::rs09_screen_width(), page::rs09_screen_height()) };
    (width > 0 && height > 0).then_some([width, height])
}

#[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
fn desktop() -> Option<[i32; 2]> {
    None
}

/// What this window system offers `DisplayMode`: the hardware renderer
/// only, and the screen's modes. On Windows those are the monitor's own, in
/// the order the original's `GraphicsDevice.getDisplayModes()` gets them
/// (`DisplayMode.getDisplayModes` filters and sorts them as the original
/// does); elsewhere, or if the monitor gives no list, the one mode is the
/// desktop as it is.
pub fn system() -> System {
    #[cfg(target_os = "windows")]
    if let Some(modes) = rs09_client::full_screen::screen_modes().filter(|modes| !modes.is_empty())
    {
        return System::client(modes);
    }
    System::client(
        desktop()
            .map(|[width, height]| Mode {
                width,
                height,
                // The depth and rate are whatever the desktop runs at; the
                // original only compares depths against each other.
                bit_depth: 32,
                refresh_rate: 0,
            })
            .into_iter()
            .collect(),
    )
}

#[cfg(target_os = "windows")]
fn enter_full_screen() {
    use mq::miniquad::window;
    let (x, y) = window::get_window_position();
    let (width, height) = window::screen_size();
    WINDOWED.with(|windowed| windowed.set(Some(([x, y], [width as u32, height as u32]))));
    window::set_fullscreen(true);
}

#[cfg(target_os = "windows")]
fn exit_full_screen() {
    use mq::miniquad::window;
    LEAVING.with(|leaving| leaving.set(Some((self::window(), 30))));
    window::set_fullscreen(false);
    // miniquad leaves the window at the screen's corner with the screen's
    // size; the three requests are carried out together before the next
    // frame, so the window is back as it was when that frame measures it.
    if let Some(([x, y], [width, height])) = WINDOWED.with(Cell::take) {
        window::set_window_size(width, height);
        window::set_window_position(x, y);
    }
}

/// The canvas element asks the browser for full screen. The answer comes
/// later: a refusal (no click or key press just before) or the player
/// leaving with Escape is reported by [`full_screen_lost`].
#[cfg(target_arch = "wasm32")]
fn enter_full_screen() {
    // SAFETY: a plain call into the page glue.
    unsafe { page::rs09_full_screen(1) }
}

#[cfg(target_arch = "wasm32")]
fn exit_full_screen() {
    LEAVING.with(|leaving| leaving.set(Some((self::window(), 30))));
    // SAFETY: as above.
    unsafe { page::rs09_full_screen(0) }
}

#[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
fn enter_full_screen() {}

#[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
fn exit_full_screen() {}

/// The browser ended full screen without the client asking (Escape, its own
/// controls) or refused to enter it, since the last call. The original's
/// full-screen frame cannot be taken away like that; the nearest thing it
/// knows is that frame losing the focus, which takes it back to the
/// remembered mode (client.java:849-851), and the caller does the same.
pub fn full_screen_lost() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: a plain call into the page glue.
        unsafe { page::rs09_full_screen_left() != 0 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

/// Carries out what `display` did since the last call. Returns whether
/// `DisplayMode.setWindowMode` ran to its end (it reports the window when
/// the game socket is open), which the caller passes on to the status timer.
///
/// Entering and leaving full screen are the only effects with something to
/// do here: the layout runs every frame, the preferences are saved by the
/// main loop when their write count moves, and the single renderer neither
/// stops nor starts.
pub fn carry_out(display: &mut Display) -> bool {
    let mut reported = false;
    // The last of the full-screen requests decides. The window is the same
    // for every mode, so leaving one mode for another (an exit and an entry
    // in one call) keeps it as it is, and the size it had before full
    // screen stays remembered.
    let mut full_screen = None;
    for effect in std::mem::take(&mut display.effects) {
        match effect {
            Effect::EnterFullScreen { .. } => full_screen = Some(true),
            Effect::ExitFullScreen => full_screen = Some(false),
            Effect::WindowStatus => reported = true,
            _ => {}
        }
    }
    match full_screen {
        Some(true) if !FULL_SCREEN.with(Cell::get) => {
            FULL_SCREEN.with(|full| full.set(true));
            enter_full_screen();
        }
        Some(false) if FULL_SCREEN.with(Cell::get) => {
            FULL_SCREEN.with(|full| full.set(false));
            exit_full_screen();
        }
        _ => {}
    }
    reported
}

/// The sample count of the window for `Preferences.antiAliasingMode`: the
/// original starts its renderer with `antiAliasingMode * 2` samples
/// (DisplayMode.java:219) and replaces the canvas when the preference
/// changes (ScriptRunner.java:5179-5190). miniquad fixes the count when it
/// makes the window, so the preference read from the file at start decides
/// and a change takes effect at the next start. 0 and 1 both mean no
/// multisampling.
#[cfg(not(target_arch = "wasm32"))]
pub fn samples(anti_aliasing: i32) -> i32 {
    match anti_aliasing {
        1 => 2,
        2 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::surface_rect;

    #[test]
    fn rectangles_are_scaled_to_the_surface_edge_by_edge() {
        // Not from the Java, which has no display scale: the unit of
        // `QuadGl::scissor` and of a GL viewport is the surface's pixel.
        // On an unscaled display nothing changes.
        for rect in [(0, 0, 765, 503), (129, 0, 765, 503), (-32, 7, 5, 9)] {
            assert_eq!(surface_rect(rect, 1.0), rect);
        }
        // A device pixel ratio of 2: the fixed canvas at its margin in an
        // 1100 by 700 page.
        assert_eq!(surface_rect((167, 0, 765, 503), 2.0), (334, 0, 1530, 1006));
        // A fractional scale: neighbours meet at one surface pixel, with no
        // gap and no overlap, whatever their own widths round to.
        let scale = 1.25;
        let (left, _, width, _) = surface_rect((3, 0, 3, 1), scale);
        let (next, _, next_width, _) = surface_rect((6, 0, 3, 1), scale);
        assert_eq!((left, width, next, next_width), (4, 4, 8, 3));
        assert_eq!(left + width, next);
        // A canvas wider than a narrow window starts left of it.
        assert_eq!(surface_rect((-32, 0, 765, 503), 1.5), (-48, 0, 1148, 755));
    }
}
