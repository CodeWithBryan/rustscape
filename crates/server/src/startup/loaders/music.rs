//! `MusicConfigLoader`: the song list from the cache's data maps 1351 (button
//! index to song id) and 1345 (button index to name), then
//! `music_regions.json` (a track per region) and `music_tiles.json` (music
//! zones by tile borders).
use super::{LoadError, Log, Result, array, array_get, int, object, parse, read_config, text};
use crate::startup::regions::{MusicZone, Regions, ZoneBorders};
use crate::world::drop_tables::LogLevel;
use rs09_game_core::java_map::int_key_order;
use std::{collections::HashMap, sync::Arc};

/// Native bound on the regions one music zone may cover. The original walks
/// any border box and caches a region for each id; a malformed box can ask for
/// billions.
pub const MAX_ZONE_REGIONS: usize = 1 << 16;

/// A `DataMap` value: `opcode 5` strings or `opcode 6` integers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataValue {
    Text(String),
    Int(i32),
}

/// `core.cache.def.impl.DataMap` (an enum config of cache index 17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataMap {
    pub id: i32,
    /// `keyType`/`valueType` as UTF-16 code units; `'?'` until set.
    pub key_type: u16,
    pub value_type: u16,
    pub default_string: Option<String>,
    pub default_int: i32,
    /// `dataStore` in first-insertion order of its keys (a later `put` of the
    /// same key replaces the value in place).
    entries: Vec<(i32, DataValue)>,
}
impl DataMap {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            key_type: u16::from(b'?'),
            value_type: u16::from(b'?'),
            default_string: None,
            default_int: 0,
            entries: Vec::new(),
        }
    }
    /// `DataMap.parse(id, data)`; `None` is a file the cache does not have.
    /// Reading past the end is Java's `BufferUnderflowException` and a zero
    /// type byte its `IllegalArgumentException`; both are refused here.
    pub fn parse(id: i32, data: Option<&[u8]>) -> Result<Self> {
        let mut def = Self::new(id);
        let Some(data) = data else {
            return Ok(def);
        };
        let underflow = || LoadError::Binding("DataMap: BufferUnderflowException".into());
        let mut at = 0usize;
        let mut byte = |at: &mut usize| -> Result<u8> {
            let b = *data.get(*at).ok_or_else(underflow)?;
            *at += 1;
            Ok(b)
        };
        loop {
            let opcode = byte(&mut at)?;
            if opcode == 0 {
                break;
            }
            match opcode {
                1 => def.key_type = cp1252(byte(&mut at)?)?,
                2 => def.value_type = cp1252(byte(&mut at)?)?,
                3 => def.default_string = Some(string(&mut byte, &mut at)?),
                4 => def.default_int = int32(&mut byte, &mut at)?,
                5 | 6 => {
                    let size = (u16::from(byte(&mut at)?) << 8) | u16::from(byte(&mut at)?);
                    for _ in 0..size {
                        let key = int32(&mut byte, &mut at)?;
                        let value = if opcode == 5 {
                            DataValue::Text(string(&mut byte, &mut at)?)
                        } else {
                            DataValue::Int(int32(&mut byte, &mut at)?)
                        };
                        def.put(key, value);
                    }
                }
                _ => {}
            }
        }
        Ok(def)
    }
    fn put(&mut self, key: i32, value: DataValue) {
        if let Some(slot) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }
    pub fn get(&self, key: i32) -> Option<&DataValue> {
        self.entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// The entries in `HashMap<Integer, Object>` iteration order.
    pub fn iter(&self) -> Vec<(i32, &DataValue)> {
        int_key_order(self.entries.iter().map(|(k, _)| *k))
            .into_iter()
            .map(|key| (key, self.get(key).expect("key from this map")))
            .collect()
    }
    /// `getString(key)`: `(String) dataStore.get(key)`.
    pub fn get_string(&self, key: i32) -> Result<Option<String>> {
        match self.get(key) {
            None => Ok(None),
            Some(DataValue::Text(text)) => Ok(Some(text.clone())),
            Some(DataValue::Int(_)) => Err(LoadError::Type),
        }
    }
}

/// `CP1252.getFromByte`.
fn cp1252(value: u8) -> Result<u16> {
    const MAP: [u16; 32] = [
        0x20ac, 0, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039,
        0x0152, 0, 0x017d, 0, 0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc,
        0x2122, 0x0161, 0x203a, 0x0153, 0, 0x017e, 0x0178,
    ];
    if value == 0 {
        return Err(LoadError::Binding(
            "CP1252: IllegalArgumentException (Non cp1252 character 0x0)".into(),
        ));
    }
    if (128..160).contains(&value) {
        let mapped = MAP[usize::from(value - 128)];
        return Ok(if mapped == 0 { 63 } else { mapped });
    }
    Ok(u16::from(value))
}

/// `ByteBufferUtils.getString`: bytes up to a zero, each widened as a signed
/// byte (`Char(b.toUShort())`), so bytes above 127 become U+FF80..U+FFFF.
fn string(byte: &mut impl FnMut(&mut usize) -> Result<u8>, at: &mut usize) -> Result<String> {
    let mut units = Vec::new();
    loop {
        let b = byte(at)?;
        if b == 0 {
            break;
        }
        units.push(b as i8 as i16 as u16);
    }
    Ok(String::from_utf16(&units).expect("no surrogates are produced"))
}

fn int32(byte: &mut impl FnMut(&mut usize) -> Result<u8>, at: &mut usize) -> Result<i32> {
    let mut value = 0u32;
    for _ in 0..4 {
        value = (value << 8) | u32::from(byte(at)?);
    }
    Ok(value as i32)
}

/// `core.game.node.entity.player.link.music.MusicEntry`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicEntry {
    pub id: i32,
    pub name: Option<String>,
    pub index: i32,
}

/// `MusicEntry.SONGS`.
#[derive(Clone, Debug, Default)]
pub struct Songs {
    entries: HashMap<i32, MusicEntry>,
}
impl Songs {
    pub fn get(&self, id: i32) -> Option<&MusicEntry> {
        self.entries.get(&id)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&i32, &MusicEntry)> {
        self.entries.iter()
    }
}

/// The first part of `load()`: `MusicEntry.getSongs().putIfAbsent` for every
/// entry of `DataMap.get(1351)` in its iteration order, named from 1345.
pub fn register_songs(song_ids: &DataMap, names: &DataMap, songs: &mut Songs) -> Result<()> {
    for (index, value) in song_ids.iter() {
        let DataValue::Int(song_id) = value else {
            return Err(LoadError::Type);
        };
        let entry = MusicEntry {
            id: *song_id,
            name: names.get_string(index)?,
            index,
        };
        songs.entries.entry(*song_id).or_insert(entry);
    }
    Ok(())
}

/// The data maps `load()` reads from the cache.
pub struct SongMaps<'a> {
    /// `DataMap.get(1351)`.
    pub song_ids: &'a DataMap,
    /// `DataMap.get(1345)`.
    pub names: &'a DataMap,
}

/// `load()` with `CONFIG_PATH`: songs, then the region file, then the tile
/// file. A missing tile file fails after the songs and region tracks are in.
pub fn load_config(
    prefix: Option<&str>,
    maps: SongMaps<'_>,
    songs: &mut Songs,
    regions: &mut Regions,
    log: &mut dyn Log,
) -> Result<()> {
    register_songs(maps.song_ids, maps.names, songs)?;
    load_files(prefix, regions, log)
}

/// The part of `load()` after the songs: both configuration files.
pub fn load_files(prefix: Option<&str>, regions: &mut Regions, log: &mut dyn Log) -> Result<()> {
    let text = read_config(prefix, "music_regions.json")?;
    load_regions(&text, regions, log)?;
    let text = read_config(prefix, "music_tiles.json")?;
    load_tiles(&text, regions, log)?;
    Ok(())
}

/// `music_regions.json`: `RegionManager.forId(region).music = id`, last row wins.
pub fn load_regions(source: &str, regions: &mut Regions, log: &mut dyn Log) -> Result<i32> {
    let mut count = 0i32;
    let data = parse(source)?;
    let configs = array(Some(&data))?;
    for config in configs {
        let e = object(Some(config))?;
        let region = int(&text(e, "region"))?;
        let id = int(&text(e, "id"))?;
        regions.for_id(region).music = id;
        count = count.wrapping_add(1);
    }
    log.log(
        "MusicConfigLoader",
        LogLevel::Fine,
        &format!("Parsed {count} region music configs."),
    );
    Ok(count)
}

/// `music_tiles.json`: each `-`-separated `{x1,y1,x2,y2[,exceptions]}` box
/// becomes one zone, added to every region the box touches.
pub fn load_tiles(source: &str, regions: &mut Regions, log: &mut dyn Log) -> Result<i32> {
    let mut count = 0i32;
    let data = parse(source)?;
    let configs = array(Some(&data))?;
    for config in configs {
        let e = object(Some(config))?;
        let music_id = int(&text(e, "id"))?;
        let string = text(e, "borders");
        for border in string.split('-') {
            let cleaned = border.replace(['{', '}'], "");
            let tokens: Vec<&str> = cleaned.split(',').collect();
            let token = |i: usize| -> Result<i32> { int(array_get(&tokens, i)?) };
            let mut borders = ZoneBorders::new(token(0)?, token(1)?, token(2)?, token(3)?);
            if border.contains('[') {
                let mut exceptions = String::new();
                for t in tokens.iter().skip(4) {
                    exceptions.push_str(t);
                    if !t.contains("]~") || t.contains('[') {
                        exceptions.push(',');
                    }
                }
                for exception in exceptions.split('~') {
                    let cleaned = exception.replace(['[', ']'], "");
                    let e: Vec<&str> = cleaned.split(',').collect();
                    let value = |i: usize| -> Result<i32> { int(array_get(&e, i)?) };
                    borders.add_exception(ZoneBorders::new(
                        value(0)?,
                        value(1)?,
                        value(2)?,
                        value(3)?,
                    ));
                }
            }
            let ids = bounded_region_ids(&borders)?;
            let zone = Arc::new(MusicZone { music_id, borders });
            for id in ids {
                regions.for_id(id).music_zones.push(Arc::clone(&zone));
            }
        }
        count = count.wrapping_add(1);
    }
    log.log(
        "MusicConfigLoader",
        LogLevel::Fine,
        &format!("Parsed {count} tile music configs."),
    );
    Ok(count)
}

fn bounded_region_ids(borders: &ZoneBorders) -> Result<Vec<i32>> {
    let width = i64::from(borders.north_east_x >> 6) - i64::from(borders.south_west_x >> 6) + 1;
    let height = i64::from(borders.north_east_y >> 6) - i64::from(borders.south_west_y >> 6) + 1;
    if width.saturating_mul(height) > MAX_ZONE_REGIONS as i64 {
        return Err(LoadError::Binding(
            "music zone covers more than 65,536 regions".into(),
        ));
    }
    Ok(borders.region_ids())
}
