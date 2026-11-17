//! Full screen at a screen mode of the player's choice, without changing the
//! screen's mode.
//!
//! The original lists the screen's modes (`FullScreenManager.getDisplayModes`
//! over `GraphicsDevice.getDisplayModes`, filtered and sorted by
//! `DisplayMode.getDisplayModes`) and, for full screen, switches the screen to
//! the chosen one (`FullScreenManager.enter`, `GraphicsDevice.setDisplayMode`).
//! Its frame, its canvas, the interface layout and packet 243 then all have
//! the mode's size, and the monitor shows that picture on its panel.
//!
//! This client lists the same modes ([`screen_modes`]) but never switches:
//! its full-screen window keeps the desktop's size and the canvas, which has
//! the chosen mode's size everywhere the game can see, is drawn into the part
//! of the window the monitor would show it in ([`Fit`]).
//!
//! What a mode switch looks like depends on who scales: a panel driven at a
//! mode of its own aspect ratio is filled; for another aspect ratio, the
//! graphics driver's scaling as Windows sets it up by default keeps the
//! picture's proportions and puts black bars beside or above and below it
//! (some monitors and driver settings stretch instead). [`Fit`] is that
//! default: the largest picture of the mode's proportions that fits, centred,
//! the rest black. It is scaled by the same factor across and down, up to the
//! rounding of its size to whole pixels.
use crate::display::{Mode, letterbox};

/// How a screen of `screen` pixels shows a screen mode of `mode` pixels: the
/// picture, centred and as large as the mode's proportions allow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fit {
    /// The mode: `GameShell.canvasWidth`/`canvasHeight` in full screen.
    pub mode: [i32; 2],
    /// The window over the whole screen, in the interface's pixels.
    pub screen: [i32; 2],
    /// Where the mode's picture lies on the screen: `[x, y, width, height]`.
    pub picture: [i32; 4],
}

/// `numerator / denominator` rounded half up, for positive values.
fn rounded(numerator: i64, denominator: i64) -> i32 {
    ((2 * numerator + denominator) / (2 * denominator)) as i32
}

impl Fit {
    /// The picture of `mode` on `screen`, or None when the two are the same
    /// size (the mode is the desktop's: nothing to scale) or either is empty.
    pub fn new(mode: [i32; 2], screen: [i32; 2]) -> Option<Self> {
        let [mode_width, mode_height] = mode;
        let [screen_width, screen_height] = screen;
        if mode == screen
            || mode_width <= 0
            || mode_height <= 0
            || screen_width <= 0
            || screen_height <= 0
        {
            return None;
        }
        let (mw, mh, sw, sh) = (
            i64::from(mode_width),
            i64::from(mode_height),
            i64::from(screen_width),
            i64::from(screen_height),
        );
        // The mode is no wider than the screen for its height: full height,
        // bars left and right. Otherwise full width, bars above and below.
        // At least one pixel, however narrow the window is for a moment.
        let (width, height) = if mw * sh <= mh * sw {
            (rounded(mw * sh, mh).max(1), screen_height)
        } else {
            (screen_width, rounded(mh * sw, mw).max(1))
        };
        Some(Self {
            mode,
            screen,
            picture: [
                (screen_width - width) / 2,
                (screen_height - height) / 2,
                width,
                height,
            ],
        })
    }

    /// The pixel of the mode under the screen pixel `point`: the one whose
    /// scaled area holds that pixel's centre. Outside the picture the answer
    /// lies outside the mode (negative, or at least its size).
    pub fn to_mode(&self, point: [i32; 2]) -> [i32; 2] {
        [0, 1].map(|axis| {
            let offset = i64::from(point[axis]) - i64::from(self.picture[axis]);
            let size = i64::from(self.picture[axis + 2]);
            let mode = i64::from(self.mode[axis]);
            ((2 * offset + 1) * mode).div_euclid(2 * size) as i32
        })
    }

    /// Where the edge at `edge` mode pixels along `axis` (0 across, 1 down)
    /// falls on the screen, in screen pixels (fractional).
    pub fn edge(&self, axis: usize, edge: f32) -> f32 {
        self.picture[axis] as f32 + edge * self.picture[axis + 2] as f32 / self.mode[axis] as f32
    }

    /// A rectangle `(x, y, width, height)` given in the mode's pixels, on a
    /// surface of `scale` surface pixels per screen pixel, each edge rounded
    /// by itself so that neighbours keep sharing their edge.
    pub fn surface_rect(
        &self,
        (x, y, width, height): (i32, i32, i32, i32),
        scale: f32,
    ) -> (i32, i32, i32, i32) {
        let px = |axis: usize, edge: i32| (self.edge(axis, edge as f32) * scale).round() as i32;
        let (x0, y0) = (px(0, x), px(1, y));
        let (x1, y1) = (px(0, x + width), px(1, y + height));
        (x0, y0, x1 - x0, y1 - y0)
    }

    /// The screen around the picture, black: left, top, right and bottom
    /// bands as `GameShell.paintFrameLetterbox` names them, absent where
    /// the picture reaches the edge.
    pub fn bars(&self) -> [Option<[i32; 4]>; 4] {
        let [x, y, width, height] = self.picture;
        letterbox(self.screen, [width, height], x, y)
    }
}

/// `GraphicsDevice.getDisplayModes()` of the screen `FullScreenManager`
/// uses (the default one, which is the primary monitor): the modes
/// `EnumDisplaySettings` gives for index 0, 1, ... until it fails, of at
/// least 8 bits, in the system's order, each width, height, depth and
/// refresh rate once (the system repeats a mode for each of its scaling and
/// output variants, which the JVM does not). A query only: no setting of
/// the screen is changed. None when the monitor cannot be named.
///
/// The JVM's list comes from its Direct3D pipeline
/// (`sun.java2d.d3d.D3DGraphicsDevice.enumDisplayModes`, native) or, without
/// it, from `Win32GraphicsDevice`'s; that native code is not in the JDK's
/// sources here, so the rule above is the one observed: on the machine this
/// was written on the JVM listed 177 modes and this function the same 177
/// in the same order, out of 471 entries (`examples/screen_modes.rs`
/// compares the two live).
#[cfg(target_os = "windows")]
pub fn screen_modes() -> Option<Vec<Mode>> {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    #[allow(dead_code)]
    struct MonitorInfoEx {
        size: u32,
        monitor: [i32; 4],
        work: [i32; 4],
        flags: u32,
        device: [u16; 32],
    }
    #[repr(C)]
    #[allow(dead_code)]
    struct DevMode {
        device_name: [u16; 32],
        spec_version: u16,
        driver_version: u16,
        size: u16,
        driver_extra: u16,
        fields: u32,
        /// The printer or display union.
        union: [u32; 4],
        color: i16,
        duplex: i16,
        y_resolution: i16,
        tt_option: i16,
        collate: i16,
        form_name: [u16; 32],
        log_pixels: u16,
        bits_per_pel: u32,
        pels_width: u32,
        pels_height: u32,
        display_flags: u32,
        display_frequency: u32,
        rest: [u32; 8],
    }
    const _: () = assert!(std::mem::size_of::<DevMode>() == 220);
    const _: () = assert!(std::mem::size_of::<MonitorInfoEx>() == 104);
    const MONITOR_DEFAULTTOPRIMARY: u32 = 1;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MonitorFromPoint(point: Point, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfoEx) -> i32;
        fn EnumDisplaySettingsW(device: *const u16, index: u32, mode: *mut DevMode) -> i32;
    }
    // SAFETY: plain user32 queries with valid, sized out-pointers; the
    // device name is NUL-terminated by the system.
    unsafe {
        let monitor = MonitorFromPoint(Point::default(), MONITOR_DEFAULTTOPRIMARY);
        if monitor.is_null() {
            return None;
        }
        let mut info: MonitorInfoEx = std::mem::zeroed();
        info.size = std::mem::size_of::<MonitorInfoEx>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let mut modes = Vec::new();
        let mut entry: DevMode = std::mem::zeroed();
        for index in 0u32.. {
            entry.size = std::mem::size_of::<DevMode>() as u16;
            entry.driver_extra = 0;
            if EnumDisplaySettingsW(info.device.as_ptr(), index, &mut entry) == 0 {
                break;
            }
            if entry.bits_per_pel >= 8 {
                modes.push(Mode {
                    width: entry.pels_width as i32,
                    height: entry.pels_height as i32,
                    bit_depth: entry.bits_per_pel as i32,
                    refresh_rate: entry.display_frequency as i32,
                });
            }
        }
        Some(distinct(modes))
    }
}

/// Each mode once, where it first appears.
pub fn distinct(modes: Vec<Mode>) -> Vec<Mode> {
    let mut seen = std::collections::HashSet::new();
    modes
        .into_iter()
        .filter(|mode| seen.insert((mode.width, mode.height, mode.bit_depth, mode.refresh_rate)))
        .collect()
}

/// No mode list on this system.
#[cfg(not(target_os = "windows"))]
pub fn screen_modes() -> Option<Vec<Mode>> {
    None
}

#[cfg(test)]
mod tests {
    //! Transcription tests: not from the Java (which switches the screen's
    //! mode instead); the expected values are worked by hand from the rule in
    //! the module documentation.
    use super::*;

    #[test]
    fn a_narrower_mode_is_shown_full_height_between_two_bars() {
        // 1024 by 768 on 5120 by 2160: 1024 * 2160 / 768 = 2880 wide,
        // (5120 - 2880) / 2 = 1120 each side.
        let fit = Fit::new([1024, 768], [5120, 2160]).unwrap();
        assert_eq!(fit.picture, [1120, 0, 2880, 2160]);
        assert_eq!(
            fit.bars(),
            [
                Some([0, 0, 1120, 2160]),
                None,
                Some([4000, 0, 1120, 2160]),
                None,
            ]
        );
        // 2880 / 1024 = 2.8125 screen pixels a mode pixel.
        assert_eq!(fit.to_mode([1120, 0]), [0, 0]);
        assert_eq!(fit.to_mode([1122, 2]), [0, 0]);
        // Screen pixel 3's centre is at 3.5 / 2.8125 = 1.24.
        assert_eq!(fit.to_mode([1123, 3]), [1, 1]);
        assert_eq!(fit.to_mode([3999, 2159]), [1023, 767]);
        // On the bars: outside the mode.
        assert_eq!(fit.to_mode([1119, 0])[0], -1);
        assert_eq!(fit.to_mode([4000, 0])[0], 1024);
        assert_eq!(fit.to_mode([0, 0])[0], -399);
        assert_eq!(fit.edge(0, 0.0), 1120.0);
        assert_eq!(fit.edge(0, 512.0), 2560.0);
        assert_eq!(fit.edge(1, 768.0), 2160.0);
    }

    #[test]
    fn a_wider_mode_is_shown_full_width_between_two_bars() {
        // 1920 by 1080 on 1280 by 1024: 1080 * 1280 / 1920 = 720 high,
        // (1024 - 720) / 2 = 152 above and below.
        let fit = Fit::new([1920, 1080], [1280, 1024]).unwrap();
        assert_eq!(fit.picture, [0, 152, 1280, 720]);
        assert_eq!(
            fit.bars(),
            [
                None,
                Some([0, 0, 1280, 152]),
                None,
                Some([0, 872, 1280, 152]),
            ]
        );
        // Two thirds of a screen pixel a mode pixel: screen pixel 1's centre
        // is at 1.5 * 1.5 = 2.25.
        assert_eq!(fit.to_mode([1, 153]), [2, 2]);
        assert_eq!(fit.to_mode([1279, 871]), [1919, 1079]);
        assert_eq!(fit.to_mode([0, 151])[1], -1);
    }

    #[test]
    fn a_mode_of_the_screens_proportions_fills_it_and_odd_sizes_round() {
        // 1280 by 720 on 5120 by 2880: four screen pixels a mode pixel.
        let fit = Fit::new([1280, 720], [5120, 2880]).unwrap();
        assert_eq!(fit.picture, [0, 0, 5120, 2880]);
        assert_eq!(fit.bars(), [None; 4]);
        assert_eq!(fit.to_mode([7, 4]), [1, 1]);
        // 1176 by 664 on 5120 by 2160: 1176 * 2160 / 664 = 3825.54, so 3826
        // wide; (5120 - 3826) / 2 = 647 on the left, 647 on the right.
        let fit = Fit::new([1176, 664], [5120, 2160]).unwrap();
        assert_eq!(fit.picture, [647, 0, 3826, 2160]);
        assert_eq!(fit.bars()[2], Some([4473, 0, 647, 2160]));
        // An odd margin: 800 by 600 on 1281 by 600 is 800 wide at 240.
        let fit = Fit::new([800, 600], [1281, 600]).unwrap();
        assert_eq!(fit.picture, [240, 0, 800, 600]);
        assert_eq!(fit.bars()[2], Some([1040, 0, 241, 600]));
        assert_eq!(fit.to_mode([240, 0]), [0, 0]);
    }

    #[test]
    fn the_desktops_own_size_and_empty_sizes_have_nothing_to_scale() {
        assert_eq!(Fit::new([5120, 2160], [5120, 2160]), None);
        assert_eq!(Fit::new([0, 768], [5120, 2160]), None);
        assert_eq!(Fit::new([1024, 768], [0, 0]), None);
        // A window far too small for the mode's proportions still has a
        // picture of one pixel (800 * 1 / 6000 rounds to 0).
        let fit = Fit::new([800, 6000], [10, 1]).unwrap();
        assert_eq!(fit.picture, [4, 0, 1, 1]);
        assert_eq!(fit.to_mode([4, 0]), [400, 3000]);
        // A mode larger than the screen is shown smaller.
        let fit = Fit::new([3840, 2160], [1920, 1080]).unwrap();
        assert_eq!(fit.picture, [0, 0, 1920, 1080]);
        assert_eq!(fit.to_mode([0, 0]), [1, 1]);
        assert_eq!(fit.to_mode([1919, 1079]), [3839, 2159]);
    }

    #[test]
    fn rectangles_map_to_the_surface_edge_by_edge() {
        let fit = Fit::new([1024, 768], [5120, 2160]).unwrap();
        // The whole mode is the picture.
        assert_eq!(
            fit.surface_rect((0, 0, 1024, 768), 1.0),
            (1120, 0, 2880, 2160)
        );
        // The fixed gameframe's viewport (4, 4, 512, 334): edges at
        // 1120 + 4 * 2.8125 = 1131.25 and 1120 + 516 * 2.8125 = 2571.25.
        assert_eq!(
            fit.surface_rect((4, 4, 512, 334), 1.0),
            (1131, 11, 1440, 940)
        );
        // Neighbours share their edge.
        let (left, _, width, _) = fit.surface_rect((3, 0, 3, 1), 1.0);
        let (next, ..) = fit.surface_rect((6, 0, 3, 1), 1.0);
        assert_eq!(left + width, next);
        // A display scale multiplies the screen's pixels.
        assert_eq!(
            fit.surface_rect((0, 0, 1024, 768), 1.5),
            (1680, 0, 4320, 3240)
        );
    }

    #[test]
    fn a_mode_the_system_repeats_is_listed_once_where_it_first_appears() {
        let mode = |width, height, refresh_rate| Mode {
            width,
            height,
            bit_depth: 32,
            refresh_rate,
        };
        assert_eq!(
            distinct(vec![
                mode(640, 480, 120),
                mode(640, 480, 120),
                mode(640, 480, 60),
                mode(800, 600, 120),
                mode(640, 480, 120),
                mode(640, 480, 60),
            ]),
            [mode(640, 480, 120), mode(640, 480, 60), mode(800, 600, 120)]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn the_screen_answers_with_its_modes() {
        // The machine running the tests has a monitor with at least one
        // mode; the list is the system's, so only its shape is checked.
        let modes = screen_modes().expect("the primary monitor is named");
        assert!(!modes.is_empty());
        assert!(
            modes
                .iter()
                .all(|mode| mode.width > 0 && mode.height > 0 && mode.bit_depth >= 8)
        );
    }
}
