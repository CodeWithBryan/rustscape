//! `MapList`, `Map`, `MapChunk`, `MapElementList` and `MapElement`.
use super::{Assets, raster::Sprite};
use crate::ui::sprite::Sheet;
use rs09_protocol::buffer::Reader;

/// JagString bytes as text: one char per byte (ISO-8859-1), as `charAt` reads them.
pub fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// The inverse of [`latin1`]; chars above U+00FF have no JagString byte and become `?`.
pub fn jag_bytes(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

/// JagString's case fold: A-Z and 0xC0..=0xDE except 0xD7 gain 32.
fn fold(b: u8) -> u8 {
    if b.is_ascii_uppercase() || ((0xc0..=0xde).contains(&b) && b != 0xd7) {
        b + 32
    } else {
        b
    }
}

/// `JagString.equalsIgnoreCase`.
pub fn equals_ignore_case(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| fold(x) == fold(y))
}

/// `JagString.startsWithIgnoreCase`.
pub fn starts_with_ignore_case(text: &[u8], prefix: &[u8]) -> bool {
    text.len() >= prefix.len() && text.iter().zip(prefix).all(|(&x, &y)| fold(x) == fold(y))
}

/// `name.replaceAll(SPACE, LINE_BREAK)`: every `<br>` becomes one space.
pub fn breaks_to_spaces(name: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(name.len());
    let mut at = 0;
    while at < name.len() {
        if name[at..].starts_with(b"<br>") {
            out.push(b' ');
            at += 4;
        } else {
            out.push(name[at]);
            at += 1;
        }
    }
    out
}

/// `MapChunk`: one inclusive display rectangle of a map.
///
/// The Java names are misleading; each field names the original it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapChunk {
    /// `displayMinX`
    pub min_x: i32,
    /// `displayMaxX` (the smallest y)
    pub min_y: i32,
    /// `displayMaxZ` (the largest x)
    pub max_x: i32,
    /// `displayMinZ` (the largest y)
    pub max_y: i32,
}
impl MapChunk {
    /// `MapChunk.containsDisplay(x = display y, z = display x)`.
    pub fn contains_display(&self, y: i32, x: i32) -> bool {
        x >= self.min_x && self.max_x >= x && self.min_y <= y && y <= self.max_y
    }
}

/// `Map`: one entry of the `details` group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map {
    /// Archive 23 group holding the map files.
    pub group: String,
    /// Display name.
    pub name: String,
    /// Default centre, in 64-tile squares.
    pub origin_x: i32,
    pub origin_y: i32,
    /// RGB drawn where the map has no data, or -1 for the water chequer.
    pub background_color: i32,
    pub valid: bool,
    /// 37, 50, 75, 100 or 200; 255 in the file reads as 0.
    pub default_zoom: i32,
    pub chunks: Vec<MapChunk>,
    /// `displayMinX`
    pub min_x: i32,
    /// `displayMaxX` (the smallest y)
    pub min_y: i32,
    /// `displayMaxZ` (the largest x)
    pub max_x: i32,
    /// `displayMinZ` (the largest y)
    pub max_y: i32,
}
impl Map {
    /// `Map.create`.
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        let mut r = Reader::new(data);
        let mut read = || -> rs09_protocol::Result<Self> {
            let group = latin1(r.gjstr()?);
            let name = latin1(r.gjstr()?);
            let origin_x = r.g2()?;
            let origin_y = r.g2()?;
            let background_color = r.g4()?;
            let valid = r.g1()? == 1;
            let mut default_zoom = r.g1()?;
            if default_zoom == 255 {
                default_zoom = 0;
            }
            let count = r.g1()?;
            let mut chunks = Vec::with_capacity(count as usize);
            for _ in 0..count {
                // new MapChunk(minX, maxX, maxZ, minZ), arguments read left to right.
                let min_x = r.g2()?;
                let min_y = r.g2()?;
                let max_x = r.g2()?;
                let max_y = r.g2()?;
                chunks.push(MapChunk {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                });
            }
            Ok(Self {
                group,
                name,
                origin_x,
                origin_y,
                background_color,
                valid,
                default_zoom,
                chunks,
                min_x: 12800,
                min_y: 12800,
                max_x: 0,
                max_y: 0,
            })
        };
        let mut map = read().map_err(|e| format!("world map details: {e}"))?;
        map.compute_bounds();
        Ok(map)
    }

    /// `Map.computeBounds`.
    pub fn compute_bounds(&mut self) {
        self.min_y = 12800;
        self.max_x = 0;
        self.max_y = 0;
        self.min_x = 12800;
        for chunk in &self.chunks {
            if chunk.min_y < self.min_y {
                self.min_y = chunk.min_y;
            }
            if chunk.min_x < self.min_x {
                self.min_x = chunk.min_x;
            }
            if chunk.max_x > self.max_x {
                self.max_x = chunk.max_x;
            }
            if self.max_y < chunk.max_y {
                self.max_y = chunk.max_y;
            }
        }
    }

    /// `Map.containsDisplayCoordinate(displayY, displayX)`.
    pub fn contains_display_coordinate(&self, y: i32, x: i32) -> bool {
        if self.min_x > x || x > self.max_x || y < self.min_y || y > self.max_y {
            return false;
        }
        self.chunks.iter().any(|chunk| chunk.contains_display(y, x))
    }
}

/// `MapList`: the maps of the `details` group, the map-function sprites and
/// their visibility toggles.
#[derive(Clone, Debug, Default)]
pub struct MapList {
    pub maps: Vec<Map>,
    /// `MapList.sprites` (`Sprites.mapfunctions`).
    pub sprites: Vec<Sprite>,
    /// `MapList.visibility`: true hides that map function.
    pub visibility: Vec<bool>,
}
impl MapList {
    /// The `details` files in ascending file order, with already loaded sprites.
    pub fn decode<'a>(
        details: impl IntoIterator<Item = &'a [u8]>,
        sprites: Vec<Sprite>,
    ) -> Result<Self, String> {
        let maps = details
            .into_iter()
            .map(Map::decode)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            maps,
            visibility: vec![false; sprites.len()],
            sprites,
        })
    }

    /// `MapList.init(Sprites.mapfunctions, js5Archive23)`.
    ///
    /// `sprite` is the `mapfunction` sprite group and `jitter` the red, green
    /// and blue offsets `Sprites.load` passes to `adjustRgb`.
    pub fn init(assets: &dyn Assets, sprite: i32, jitter: [i32; 3]) -> Result<Self, String> {
        let sheet = assets
            .sprite(sprite)
            .ok_or("world map: mapfunction sprites are missing")?;
        let sprites = map_functions(&sheet, jitter)?;
        let details = assets
            .group_files("details")
            .ok_or("world map: the details group is missing")?;
        Self::decode(details.iter().map(Vec::as_slice), sprites)
    }

    /// `MapList.get`.
    pub fn get(&self, group: &str) -> Option<&Map> {
        self.maps.iter().find(|map| map.group == group)
    }

    /// `MapList.getContainingSource(displayX, displayY)`.
    pub fn get_containing_source(&self, x: i32, y: i32) -> Option<&Map> {
        self.maps
            .iter()
            .find(|map| map.valid && map.contains_display_coordinate(y, x))
    }

    /// `ScriptRunner.findMapsAtCoordinate(displayX, displayY)`.
    pub fn find_maps_at_coordinate(&self, x: i32, y: i32) -> Vec<&Map> {
        self.maps
            .iter()
            .filter(|map| map.valid && map.contains_display_coordinate(y, x))
            .collect()
    }

    /// Script command 5215: whether a map at the coordinate has this group
    /// name, compared with `JagString.equalsIgnoreCase`.
    pub fn contains_at(&self, x: i32, y: i32, group: &str) -> bool {
        let group = jag_bytes(group);
        self.find_maps_at_coordinate(x, y)
            .iter()
            .any(|map| equals_ignore_case(&jag_bytes(&map.group), &group))
    }

    /// `MapList.isMapVisible`.
    pub fn is_map_visible(&self, index: i32) -> bool {
        index >= 0
            && self
                .visibility
                .get(index as usize)
                .copied()
                .unwrap_or(false)
    }

    /// `MapList.toggleMapVisibility`.
    pub fn toggle_map_visibility(&mut self, index: i32) {
        if index >= 0
            && let Some(hidden) = self.visibility.get_mut(index as usize)
        {
            *hidden = !*hidden;
        }
    }
}

/// `SpriteLoader.loadSoftwareSprites` followed by `SoftwareSprite.adjustRgb`.
pub fn map_functions(data: &[u8], jitter: [i32; 3]) -> Result<Vec<Sprite>, String> {
    let sheet = Sheet::decode(data).map_err(|e| format!("mapfunction sprites: {e}"))?;
    Ok(sheet
        .frames
        .iter()
        .map(|frame| {
            let mut sprite = Sprite {
                x_offset: frame.offset[0] as i32,
                y_offset: frame.offset[1] as i32,
                width: frame.size[0] as i32,
                height: frame.size[1] as i32,
                pixels: frame
                    .indices
                    .iter()
                    .map(|&index| sheet.palette[index as usize] as i32)
                    .collect(),
            };
            sprite.adjust_rgb(jitter);
            sprite
        })
        .collect())
}

/// `MapElementList`: the labels of one map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapElementList {
    /// JagString bytes; lines are separated by `<br>`.
    pub names: Vec<Vec<u8>>,
    pub flags: Vec<i8>,
    /// Absolute tile coordinates, stored as Java shorts.
    pub coord_x: Vec<i16>,
    pub coord_y: Vec<i16>,
    /// RGB, or -1 for the size's default colour.
    pub colors: Vec<i32>,
}
impl MapElementList {
    /// `MapElementList.create`: the files of `<group>_labels` in ascending order.
    pub fn decode<'a>(files: impl IntoIterator<Item = &'a [u8]>) -> Result<Self, String> {
        let mut list = Self::default();
        for data in files {
            let mut r = Reader::new(data);
            let mut read = || -> rs09_protocol::Result<()> {
                list.names.push(r.gjstr()?.to_vec());
                list.flags.push(r.g1b()? as i8);
                list.coord_x.push(r.g2()? as i16);
                list.coord_y.push(r.g2()? as i16);
                list.colors.push(r.g4()?);
                Ok(())
            };
            read().map_err(|e| format!("world map label: {e}"))?;
        }
        Ok(list)
    }

    /// `MapElementList.count`.
    pub fn count(&self) -> i32 {
        self.names.len() as i32
    }

    fn flag(&self, index: i32) -> i32 {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.flags.get(i))
            .map_or(0, |&flags| i32::from(flags))
    }

    /// `isTextLabel`: drawn as text on the map.
    pub fn is_text_label(&self, index: i32) -> bool {
        self.flag(index) & 8 != 0
    }

    /// `isMinimapLabelVisible`.
    pub fn is_minimap_label_visible(&self, index: i32) -> bool {
        self.flag(index) & 4 != 0
    }

    /// `getLabelSize`: 0, 1 or 2 (3 selects no font).
    pub fn get_label_size(&self, index: i32) -> i32 {
        self.flag(index) & 3
    }

    /// `isVisible`: listed by the label iteration of commands 5212 and 5213.
    pub fn is_visible(&self, index: i32) -> bool {
        self.flag(index) & 16 == 0
    }
}

/// `MapElement`: a map function found while loading, or one queued for a highlight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapElement {
    pub id: i32,
    pub map_x: i32,
    pub map_y: i32,
}
