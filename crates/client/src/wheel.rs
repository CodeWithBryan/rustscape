//! Original JavaMouseWheel integer accumulation plus explicit platform adapters.
//! DOM pixel/line values do not encode portable physical detents. Prefer legacy
//! detents when supplied; fallback policy is 100 pixels / 3 lines / 1 page.
use rs09_protocol::{Error, Result};
use std::cell::RefCell;

#[derive(Clone, Debug, Default)]
pub struct Wheel {
    pub current: i32,
    pub previous: i32,
    pub fraction: f64,
}
impl Wheel {
    pub fn add(&mut self, rotation: i32) {
        self.previous = self.current;
        self.current = self.current.wrapping_add(rotation);
    }
    pub fn poll(&mut self) -> i32 {
        let rotation = self.current;
        self.current = 0;
        rotation
    }
    fn detents(&mut self, rotation: f64) -> Result<()> {
        if !rotation.is_finite() || rotation.abs() > 16384.0 {
            return Err(Error::new(0, "invalid wheel rotation"));
        }
        let total = self.fraction + rotation;
        let rounded = total.round();
        let total = if (rounded - total).abs() < 1e-9 {
            rounded
        } else {
            total
        };
        let whole = total.trunc() as i32;
        self.fraction = total - f64::from(whole);
        self.add(whole);
        Ok(())
    }
    /// Miniquad's native wheel is positive upwards; RT4 is positive downwards.
    pub fn native(&mut self, up: f64) -> Result<()> {
        if up == 0.0 {
            return Ok(());
        }
        self.detents(-up)
    }
    pub fn browser(&mut self, delta_y: f64, mode: i32, legacy_y: f64) -> Result<()> {
        if !delta_y.is_finite() || !(0..=2).contains(&mode) {
            return Err(Error::new(0, "invalid browser wheel sample"));
        }
        let rotation = if legacy_y.is_finite() && legacy_y != 0.0 {
            -legacy_y / 120.0
        } else {
            delta_y
                / match mode {
                    0 => 100.0,
                    1 => 3.0,
                    _ => 1.0,
                }
        };
        self.detents(rotation)
    }
}
thread_local! {static INPUT:RefCell<Wheel>=RefCell::new(Wheel::default());}
pub fn sample_native(up: f32) {
    #[cfg(not(target_arch = "wasm32"))]
    INPUT.with(|input| {
        let _ = input.borrow_mut().native(f64::from(up));
    });
    #[cfg(target_arch = "wasm32")]
    let _ = up;
}
pub fn poll() -> i32 {
    INPUT.with(|input| input.borrow_mut().poll())
}
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn rs09_wheel_event(delta_y: f64, mode: i32, legacy_y: f64) -> i32 {
    INPUT.with(|input| i32::from(input.borrow_mut().browser(delta_y, mode, legacy_y).is_ok()))
}
