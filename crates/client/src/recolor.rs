//! The script recolour palette: `client.scriptRecolorPalette`, which CS2 5401
//! writes and which NPC, object and location types read through their
//! `recol_p` indices. The original keeps one 256-entry array under four names
//! (`scriptRecolorPalette`, `npcRecolorPalette`, `objRecolorPalette`,
//! `locRecolorPalette`), all zero until a script sets an entry.

/// `ColorUtils.rgbToHsl`: 24-bit RGB to the packed 16-bit HSL of model colours.
pub fn rgb_to_hsl(rgb: i32) -> i32 {
    let r = f64::from(rgb >> 16 & 0xff) / 256.0;
    let g = f64::from(rgb >> 8 & 0xff) / 256.0;
    let b = f64::from(rgb & 0xff) / 256.0;
    let (mut min, mut max) = (r, r);
    if r > g {
        min = g;
    }
    if b < min {
        min = b;
    }
    if g > r {
        max = g;
    }
    if b > max {
        max = b;
    }
    let lightness = (min + max) / 2.0;
    let (mut hue, mut saturation) = (0.0, 0.0);
    if min != max {
        if lightness < 0.5 {
            saturation = (max - min) / (min + max);
        }
        if lightness >= 0.5 {
            saturation = (max - min) / (2.0 - max - min);
        }
        if max == r {
            hue = (g - b) / (max - min);
        } else if g == max {
            hue = (b - r) / (max - min) + 2.0;
        } else if max == b {
            hue = (r - g) / (-min + max) + 4.0;
        }
    }
    let mut saturation = (saturation * 256.0) as i32;
    let light = ((lightness * 256.0) as i32).clamp(0, 255);
    hue /= 6.0;
    let hue = (hue * 256.0) as i32;
    saturation = saturation.clamp(0, 255);
    if light > 243 {
        saturation >>= 4;
    } else if light > 217 {
        saturation >>= 3;
    } else if light > 192 {
        saturation >>= 2;
    } else if light > 179 {
        saturation >>= 1;
    }
    ((hue >> 2) << 10) + ((saturation >> 5) << 7) + (light >> 1)
}

/// The palette and a counter that changes whenever an entry is written, so
/// that holders of models built from it know to rebuild them: the original
/// clears its NPC body and head model caches and its object model and sprite
/// caches on every 5401.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub colors: [u16; 256],
    pub revision: u64,
}
impl Default for Palette {
    fn default() -> Self {
        Self {
            colors: [0; 256],
            revision: 0,
        }
    }
}
impl Palette {
    /// CS2 5401: `palette[index] = (short) rgbToHsl(rgb)`. An index outside
    /// the array ends the original script with an exception; here it is an
    /// error and nothing changes.
    pub fn set(&mut self, index: i32, rgb: i32) -> Result<(), String> {
        let slot = usize::try_from(index)
            .ok()
            .and_then(|i| self.colors.get_mut(i))
            .ok_or_else(|| format!("CS2 5401 palette index {index} outside 0..256"))?;
        *slot = rgb_to_hsl(rgb) as u16;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }
}
