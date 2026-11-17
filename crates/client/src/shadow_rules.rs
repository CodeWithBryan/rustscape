//! Shadows beyond the static scenery masks: the floor shadows upper-level
//! tiles cast (`ShadowManager.addFloorShadow`, called from
//! `SceneGraph.buildScene`), the test that defers a location's first shadow
//! (`ShadowManager.isObjectInShadow`), and when `Loc.getOrBuildEntity`
//! refreshes an animated location's shadow.
use crate::shadow::{Map, Sprite};

/// The `floorshadows` sprite group after `SoftwareIndexedSprite.trim`: one
/// full-canvas mask of palette indices per tile shape.
#[derive(Clone, Debug, Default)]
pub struct FloorSprites {
    pub size: [usize; 2],
    pub masks: Vec<Vec<u8>>,
}
impl FloorSprites {
    pub fn decode(data: &[u8]) -> rs09_protocol::Result<Self> {
        let sheet = crate::ui::sprite::Sheet::decode(data)?;
        let [width, height] = sheet.size;
        let masks = sheet
            .frames
            .iter()
            .map(|frame| {
                let mut mask = vec![0; width * height];
                for row in 0..frame.size[1] {
                    for column in 0..frame.size[0] {
                        let (x, y) = (column + frame.offset[0], row + frame.offset[1]);
                        if x < width && y < height {
                            mask[y * width + x] = frame.indices[row * frame.size[0] + column];
                        }
                    }
                }
                mask
            })
            .collect();
        Ok(Self {
            size: sheet.size,
            masks,
        })
    }
}

/// One tile of `buildScene`'s floor pass, as far as shadows are concerned.
#[derive(Clone, Copy, Debug)]
pub struct FloorTile {
    /// `tileShapes + 1` for a tile with an overlay, 0 without.
    pub shape: usize,
    pub rotation: i32,
    /// The overlay casts nothing: it is invisible or its type says so. True
    /// for a tile without an overlay.
    pub overlay_open: bool,
    /// The underlay casts nothing: there is none or its type says so.
    pub underlay_open: bool,
    /// Scene tile, and the corner heights (south-west, south-east,
    /// north-east, north-west) less the ground level's heights there.
    pub tile: [i32; 2],
    pub heights: [i32; 4],
}

/// `addFloorShadow`: the mask to add to the shadow map and where, as
/// [`Map::stamp`] takes it. `light` is `FogManager.lightX`/`lightZ` and `map`
/// the shadow map's pixel size. None when the tile casts nothing, when a
/// shaped mask would not lie wholly inside the map, or when the sprite group
/// is not the 16 x 16 set the original assumes.
pub fn floor(
    sprites: &FloorSprites,
    light: [i32; 2],
    map: [usize; 2],
    tile: FloorTile,
) -> Option<(Sprite, [i32; 2])> {
    let FloorTile {
        shape,
        overlay_open,
        underlay_open,
        ..
    } = tile;
    if overlay_open && underlay_open || overlay_open && shape == 1 || underlay_open && shape == 0 {
        return None;
    }
    if sprites.size != [16, 16] {
        return None;
    }
    let average = tile
        .heights
        .iter()
        .fold(0i32, |sum, &h| sum.wrapping_add(h))
        >> 2;
    let position: [i32; 2] = std::array::from_fn(|i| {
        (tile.tile[i] << 7).wrapping_sub(average.wrapping_mul(light[i]) >> 8) >> 3
    });
    let sprite = |pixels: Vec<u8>| Sprite {
        offset: [0, 0],
        width: 16,
        height: 16,
        pixels,
    };
    if shape != 0 && shape != 1 && (overlay_open || underlay_open) {
        // `addRotatedFloorShadow`.
        let [x, y] = position.map(|n| n + 1);
        if x <= 0 || y <= 0 || x + 16 >= map[0] as i32 || y + 16 >= map[1] as i32 {
            return None;
        }
        let source = sprites.masks.get(shape)?;
        let (mut index, stride, row_pad): (i32, i32, i32) = match tile.rotation {
            0 => (240, 1, -32),
            1 => (255, -16, 255),
            2 => (15, -1, 32),
            _ => (0, 16, -255),
        };
        let mut pixels = Vec::with_capacity(256);
        for _ in 0..16 {
            for _ in 0..16 {
                let value = *source.get(usize::try_from(index).ok()?)?;
                // The overlay's part, or everything but it (`1 - mask`).
                pixels.push(if overlay_open {
                    1u8.wrapping_sub(value)
                } else {
                    value
                });
                index += stride;
            }
            index += row_pad;
        }
        Some((sprite(pixels), position))
    } else {
        Some((sprite(sprites.masks.get(1)?.clone()), position))
    }
}

/// `isObjectInShadow`/`testSpriteOverlap`/`hasShadowPixel`: whether the area
/// a shadow sprite would cover still has an unshadowed pixel among those the
/// original samples (every eighth pixel of every eighth row, and the pixel
/// before each sampled row's end). `position` is as [`Map::stamp`] takes it.
/// The original also marks the area's texture tiles for refresh; refreshing an
/// unchanged tile uploads nothing here, so that is not repeated.
pub fn open_to_light(map: &Map, sprite: &Sprite, position: [i32; 2]) -> bool {
    let mut x = position[0] + 1 + sprite.offset[0];
    let mut y = position[1] + 1 + sprite.offset[1];
    let (map_width, map_height) = (map.width as i32, map.height as i32);
    let mut offset = x + y * map_width;
    let (mut width, mut height) = (sprite.width as i32, sprite.height as i32);
    let mut row_pad = map_width - width;
    if y <= 0 {
        let clip = 1 - y;
        height -= clip;
        offset += clip * map_width;
        y = 1;
    }
    if y + height >= map_height {
        height -= y + height + 1 - map_height;
    }
    if x <= 0 {
        let clip = 1 - x;
        width -= clip;
        offset += clip;
        row_pad += clip;
        x = 1;
    }
    if x + width >= map_width {
        let clip = x + width + 1 - map_width;
        width -= clip;
        row_pad += clip;
    }
    if width <= 0 || height <= 0 {
        return false;
    }
    row_pad += map_width * 7;
    let align = width % 8;
    let align_pad = if align == 0 { 0 } else { 8 - align };
    // Reads past the map (possible in the original only as an exception at
    // the map's last rows) count as shadowed.
    let empty = |offset: i32| {
        usize::try_from(offset)
            .ok()
            .and_then(|i| map.pixels.get(i))
            .is_some_and(|&pixel| pixel == 0)
    };
    for _ in 0..(height + 7) / 8 {
        for _ in 0..(width + 7) / 8 {
            if empty(offset) {
                return true;
            }
            offset += 8;
        }
        offset -= align_pad;
        if empty(offset - 1) {
            return true;
        }
        offset += row_pad;
    }
    false
}

/// What a location remembers about its shadow between frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocShadow {
    /// `Loc.shadowDirty`: no shadow has been added yet.
    pub pending: bool,
    /// The location type the shadow was last built for (`lastLocTypeId`).
    pub last_type: i32,
    /// `lastSeqFrame`.
    pub last_frame: i32,
}
impl Default for LocShadow {
    fn default() -> Self {
        Self {
            pending: true,
            last_type: -1,
            last_frame: -1,
        }
    }
}
impl LocShadow {
    /// `needsShadowUpdate` for a location type that casts a shadow: always
    /// for a new type; otherwise only with the scenery-shadow preference at 2
    /// and the frame changed, or a tweened or shadow-updating sequence between
    /// two frames. `frames` is the current and next frame of a running
    /// sequence (`seqFrame`, `seqNextFrame`; -1 for no next frame).
    pub fn needs_update(
        &self,
        type_id: i32,
        frames: Option<[i32; 2]>,
        between_frames: bool,
        preference: i32,
    ) -> bool {
        // A location without a sequence keeps `seqFrame` 0.
        let [frame, next] = frames.unwrap_or([0, 0]);
        type_id != self.last_type
            || (frame != self.last_frame || frames.is_some() && between_frames && frame != next)
                && preference >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprites() -> FloorSprites {
        // Mask 1 is the whole tile; mask 2 numbers its pixels by position so a
        // rotation shows in the result (values are added as bytes).
        let numbered: Vec<u8> = (0..=255).collect();
        FloorSprites {
            size: [16, 16],
            masks: vec![vec![0; 256], vec![1; 256], numbered],
        }
    }
    fn tile(shape: usize, rotation: i32, overlay_open: bool, underlay_open: bool) -> FloorTile {
        FloorTile {
            shape,
            rotation,
            overlay_open,
            underlay_open,
            tile: [20, 30],
            heights: [-240, -240, -248, -236],
        }
    }

    // Transcription tests; the JVM differential is `tests/lighting_oracle.rs`.
    #[test]
    fn a_tile_that_blocks_light_adds_the_whole_tile_mask_displaced_by_its_height() {
        let (sprite, position) = floor(
            &sprites(),
            [213, 213],
            [1666, 1666],
            tile(0, 0, true, false),
        )
        .unwrap();
        assert_eq!(sprite.pixels, vec![1; 256]);
        // Average height -241; 2560 - (-241 * 213 >> 8) = 2761, >> 3 = 345.
        assert_eq!(position, [345, 505]);
        // Neither layer casts, an underlay-only tile that does not block, and
        // an overlay that covers the whole tile but casts nothing.
        assert!(floor(&sprites(), [0, 0], [1666; 2], tile(3, 0, true, true)).is_none());
        assert!(floor(&sprites(), [0, 0], [1666; 2], tile(0, 0, false, true)).is_none());
        assert!(floor(&sprites(), [0, 0], [1666; 2], tile(1, 0, true, false)).is_none());
    }

    #[test]
    fn a_shaped_tile_adds_its_rotated_mask_or_the_rest_of_the_tile() {
        let pixels = |rotation, overlay_open| {
            floor(
                &sprites(),
                [0, 0],
                [1666; 2],
                tile(2, rotation, overlay_open, !overlay_open),
            )
            .unwrap()
            .0
            .pixels
        };
        // Rotation 0 reads the mask bottom row first.
        assert_eq!(pixels(0, false)[..3], [240, 241, 242]);
        assert_eq!(pixels(0, false)[16], 224);
        assert_eq!(pixels(1, false)[..3], [255, 239, 223]);
        assert_eq!(pixels(1, false)[16], 254);
        assert_eq!(pixels(2, false)[..3], [15, 14, 13]);
        assert_eq!(pixels(2, false)[16], 31);
        assert_eq!(pixels(3, false)[..3], [0, 16, 32]);
        assert_eq!(pixels(3, false)[16], 1);
        // An overlay that casts nothing leaves the underlay's part: 1 - mask.
        assert_eq!(pixels(3, true)[..3], [1, 241, 225]);
        // Both layers cast: the whole tile.
        let whole = floor(&sprites(), [0, 0], [1666; 2], tile(2, 1, false, false)).unwrap();
        assert_eq!(whole.0.pixels, vec![1; 256]);
        // A shaped mask that would touch the map's edge is not added at all.
        let mut edge = tile(2, 0, false, true);
        edge.tile = [0, 0];
        edge.heights = [0; 4];
        assert!(floor(&sprites(), [0, 0], [1666; 2], edge).is_some());
        // The last tile fits exactly; displaced by one pixel it does not.
        edge.tile = [103, 0];
        assert!(floor(&sprites(), [0, 0], [104 * 16 + 2; 2], edge).is_some());
        edge.heights = [-8; 4];
        assert!(floor(&sprites(), [256, 0], [104 * 16 + 2; 2], edge).is_none());
    }

    #[test]
    fn a_shadow_is_open_to_light_until_every_sampled_pixel_is_covered() {
        let mut map = Map::new(2, 2);
        let sprite = Sprite {
            offset: [0, 0],
            width: 16,
            height: 16,
            pixels: vec![1; 256],
        };
        assert!(open_to_light(&map, &sprite, [40, 40]));
        map.stamp(&sprite, [40, 40], false);
        assert!(!open_to_light(&map, &sprite, [40, 40]));
        // A smaller sprite inside the covered area samples its first pixel
        // and the pixel before its row's end: both covered.
        let small = Sprite {
            offset: [0, 0],
            width: 8,
            height: 8,
            pixels: vec![1; 64],
        };
        assert!(!open_to_light(&map, &small, [44, 44]));
        // One pixel to the west the first sampled pixel is uncovered.
        assert!(open_to_light(&map, &sprite, [39, 40]));
        assert!(open_to_light(&map, &sprite, [50, 40]));
        // Wholly outside the map: nothing to test.
        assert!(!open_to_light(&map, &sprite, [-40, -40]));
    }

    #[test]
    fn a_location_shadow_is_rebuilt_per_frame_only_at_the_highest_setting() {
        let mut state = LocShadow::default();
        assert!(state.needs_update(7, Some([0, 1]), false, 0));
        state.last_type = 7;
        state.last_frame = 0;
        assert!(!state.needs_update(7, Some([0, 1]), false, 2));
        assert!(state.needs_update(7, Some([1, 2]), false, 2));
        assert!(!state.needs_update(7, Some([1, 2]), false, 1));
        // Between two frames of a tweened sequence it is rebuilt every time.
        assert!(state.needs_update(7, Some([0, 1]), true, 2));
        assert!(!state.needs_update(7, Some([0, 0]), true, 2));
        assert!(state.needs_update(8, None, false, 0));
    }
}
