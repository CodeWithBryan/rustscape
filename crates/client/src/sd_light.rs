//! Standard-detail lighting: the hardware renderer with
//! `Preferences.highDetailLighting` off.
//!
//! With the preference on, colours are packed at the model's ambient lightness
//! and GL lights them: the sun, its opposite, the ambient term and up to four
//! point lights per object. With it off `GlRenderer.restoreLighting` disables
//! GL lighting and the normal array, `LightingManager` neither selects lights
//! nor draws light meshes, and what is drawn is the vertex colour alone:
//!
//! - `GlModel.packInterleavedVertexData` bakes a lightness into every packed
//!   vertex from its normal and `FogManager.light0Position` ([`ModelLight`]).
//! - `SceneGraph.buildScene` derives `tileLightness` from the terrain slope
//!   and the same direction (`tile_shade::ShadowMap::lightness_sd`).
//! - `Rasteriser.palette` keeps the brightness preference (0.9 to 0.6) where
//!   high detail lighting pins it to 0.7 ([`palette_brightness`]).
//!
//! The light direction is the scene's: `LoginManager` reads it for the
//! player's chunk when a scene is built, every model cache is emptied by the
//! same rebuild (`client.unload`), and a model's colours are packed when it is
//! first drawn. Interface models are drawn after `FogManager.init`, which
//! sets (-50, -60, -50).
//!
//! The switches are process-wide like the original's statics; the direction
//! is per thread, because scenes are built off the render thread.
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

static HIGH_DETAIL: AtomicBool = AtomicBool::new(true);
static BRIGHTNESS: AtomicI32 = AtomicI32::new(3);
static REVISION: AtomicU32 = AtomicU32::new(0);

/// `FogManager.setLightPosition`'s default, which `FogManager.init` restores
/// before an interface model is drawn.
pub const DEFAULT_SUN: [i32; 3] = [-50, -60, -50];

thread_local! {
    static SUN: Cell<[i32; 3]> = const { Cell::new(DEFAULT_SUN) };
}

/// `Preferences.highDetailLighting` and `Preferences.brightness` as the
/// renderer reads them. Returns whether packed colours change with the call:
/// CS2 6012 always rebuilds the scene, and CS2 6001 does while high detail
/// lighting is off (`ScriptRunner` 5048-5053 and 5154).
pub fn set(high_detail_lighting: bool, brightness: i32) -> bool {
    let was = HIGH_DETAIL.swap(high_detail_lighting, Ordering::Relaxed);
    let old = BRIGHTNESS.swap(brightness, Ordering::Relaxed);
    let changed = was != high_detail_lighting || (!high_detail_lighting && old != brightness);
    if changed {
        REVISION.fetch_add(1, Ordering::Relaxed);
    }
    changed
}

/// Whether colours are baked: high detail lighting is off.
pub fn enabled() -> bool {
    !HIGH_DETAIL.load(Ordering::Relaxed)
}

/// Counts the calls of [`set`] that changed packed colours.
pub fn revision() -> u32 {
    REVISION.load(Ordering::Relaxed)
}

/// `Rasteriser.setBrightness` as the hardware renderer leaves it: 0.7 with
/// high detail lighting (`DisplayMode` 235-237, `ScriptRunner` 5138-5139),
/// otherwise by the brightness preference (`client` 1585-1596, `ScriptRunner`
/// 5034-5047 and 5141-5152). GL textures are generated at 0.7 either way
/// (`GlTexture.bind`).
pub fn palette_brightness(brightness: i32, high_detail_lighting: bool) -> f32 {
    crate::ui::icon_text::raster_brightness(brightness, high_detail_lighting)
}

/// `Rasteriser.palette` for the current switches.
pub fn palette() -> Arc<Vec<u32>> {
    static CACHE: Mutex<Option<(u32, Arc<Vec<u32>>)>> = Mutex::new(None);
    let brightness = palette_brightness(
        BRIGHTNESS.load(Ordering::Relaxed),
        HIGH_DETAIL.load(Ordering::Relaxed),
    );
    let mut cache = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match &*cache {
        Some((key, palette)) if *key == brightness.to_bits() => palette.clone(),
        _ => {
            let palette = Arc::new(crate::surface::palette(brightness));
            *cache = Some((brightness.to_bits(), palette.clone()));
            palette
        }
    }
}

/// `FogManager.light0Position` for what this thread packs next.
pub fn set_sun(direction: [i32; 3]) {
    SUN.with(|sun| sun.set(direction));
}

pub fn sun() -> [i32; 3] {
    SUN.with(Cell::get)
}

/// Runs `pack` with another light direction, as the original packs an
/// interface model between `FogManager.init` and the next scene frame.
pub fn with_sun<T>(direction: [i32; 3], pack: impl FnOnce() -> T) -> T {
    let previous = sun();
    set_sun(direction);
    let result = pack();
    set_sun(previous);
    result
}

/// `(int) Math.sqrt(x * x + y * y + z * z)` over the truncated light
/// position, with the original's 32-bit sums.
fn length(sun: [i32; 3]) -> i32 {
    let squared = sun
        .iter()
        .fold(0i32, |sum, &n| sum.wrapping_add(n.wrapping_mul(n)));
    f64::from(squared).sqrt() as i32
}

/// The baked lightness of a model's packed vertices
/// (`GlModel.packInterleavedVertexData` 2413-2473).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelLight {
    sun: [i32; 3],
    /// `(int) ((float) ambient * 1.3F)`.
    base: i32,
    /// `contrast * length >> 8`.
    divisor: i32,
}

impl ModelLight {
    /// `ambient` and `contrast` are the model's two light intensities, which
    /// `GlModel` holds as shorts.
    pub fn new(sun: [i32; 3], ambient: i32, contrast: i32) -> Self {
        Self {
            sun,
            base: (f32::from(ambient as i16) * 1.3f32) as i32,
            divisor: i32::from(contrast as i16).wrapping_mul(length(sun)) >> 8,
        }
    }

    /// The lightness of a packed vertex whose accumulated normal is `normal`
    /// (x, y, z and the number of faces it sums, zero for a flat face's own
    /// normal). A negative count is a lightness the original has already
    /// stored in place of it.
    ///
    /// A zero divisor (no contrast, or no light direction) throws in the
    /// original; here such a vertex takes the base lightness.
    pub fn lightness(&self, normal: [i32; 4]) -> i32 {
        let count = normal[3] as i16;
        if count < 0 {
            return -i32::from(count) - 1;
        }
        let dot = (0..3).fold(0i32, |sum, axis| {
            sum.wrapping_add(self.sun[axis].wrapping_mul(i32::from(normal[axis] as i16)))
        });
        let divisor = if count == 0 {
            self.divisor.wrapping_add(self.divisor / 2)
        } else {
            self.divisor.wrapping_mul(i32::from(count))
        };
        let lit = if divisor == 0 {
            self.base
        } else {
            self.base.wrapping_add(dot.wrapping_div(divisor))
        };
        lit.clamp(0, 16384)
    }
}

/// What a model's colours are packed with while high detail lighting is off.
#[derive(Clone, Debug)]
pub struct Bake {
    pub light: ModelLight,
    pub palette: Arc<Vec<u32>>,
}

/// None with high detail lighting, when colours are packed at the ambient
/// lightness and lit by the renderer.
pub fn bake(ambient: i32, contrast: i32) -> Option<Bake> {
    enabled().then(|| Bake {
        light: ModelLight::new(sun(), ambient, contrast),
        palette: palette(),
    })
}

/// The world shader's `LightRegion` (the point-light grid's origin and size).
/// `GlRenderer.restoreLighting` leaves GL lighting off without high detail
/// lighting; the shader does the same when the region's size is negative,
/// which also puts every vertex outside the grid. The light colours are left
/// as they are, because the liquid material's vertex program reads
/// `GL_LIGHT_MODEL_AMBIENT` whether or not lighting is enabled.
pub fn light_region(region: [f32; 4]) -> [f32; 4] {
    if enabled() {
        [region[0], region[1], -1.0, -1.0]
    } else {
        region
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests: the expected values are worked by hand from
    // GlModel.packInterleavedVertexData 2413-2436. The JVM differential is
    // `tests/loc_placement.rs` over `fixtures/locs`.
    #[test]
    fn a_vertex_takes_the_ambient_term_plus_the_slope_term() {
        // length = (int) sqrt(2500 + 3600 + 2500) = 92; divisor = 768 * 92 >> 8 = 276.
        let light = ModelLight::new(DEFAULT_SUN, 64, 768);
        assert_eq!(light.base, 83);
        assert_eq!(light.divisor, 276);
        // One face straight up (y is down): dot = -60 * -256 = 15360.
        assert_eq!(light.lightness([0, -256, 0, 1]), 83 + 15360 / 276);
        // Two faces: the count divides.
        assert_eq!(light.lightness([0, -512, 0, 2]), 83 + 30720 / 552);
        // A flat normal divides by one and a half.
        assert_eq!(light.lightness([0, -256, 0, 0]), 83 + 15360 / 414);
        // Facing away: Java's division truncates toward zero, then the clamp.
        assert_eq!(light.lightness([0, 256, 0, 1]), 83 - 55);
        assert_eq!(light.lightness([0, 2560, 0, 1]), 0);
        // A stored lightness comes back.
        assert_eq!(light.lightness([9, 9, 9, -41]), 40);
        assert_eq!(light.lightness([0, -32768, 0, 0]), 83 + 1966080 / 414);
        // The upper clamp: a contrast of 8 divides by 8 * 92 >> 8 = 2.
        let harsh = ModelLight::new(DEFAULT_SUN, 64, 8);
        assert_eq!(harsh.lightness([0, -32768, 0, 0]), 16384);
    }

    #[test]
    fn shorts_and_zero_divisors_follow_the_original_types() {
        // ambient and contrast are stored as shorts.
        let light = ModelLight::new(DEFAULT_SUN, 65536 + 10, 65536 + 768);
        assert_eq!((light.base, light.divisor), (13, 276));
        // No direction: the original divides by zero; the base stays.
        let dark = ModelLight::new([0; 3], 64, 768);
        assert_eq!(dark.lightness([0, -256, 0, 1]), 83);
        // A direction whose squares overflow 32 bits has length 0 as well.
        assert_eq!(length([32767, 32767, 32767]), 0);
        assert_eq!(length([-3, 4, 0]), 5);
    }

    #[test]
    fn the_palette_brightness_follows_the_preference_only_without_hd_lighting() {
        for brightness in 0..6 {
            assert_eq!(palette_brightness(brightness, true), 0.7);
        }
        assert_eq!(
            [1, 2, 3, 4].map(|brightness| palette_brightness(brightness, false)),
            [0.9, 0.8, 0.7, 0.6]
        );
    }

    #[test]
    fn the_light_direction_is_restored_after_an_interface_model() {
        set_sun([1, 2, 3]);
        assert_eq!(with_sun(DEFAULT_SUN, sun), DEFAULT_SUN);
        assert_eq!(sun(), [1, 2, 3]);
        set_sun(DEFAULT_SUN);
    }
}
