//! [`Assets`] read straight from a JS5 cache directory: archives 2 (floor and
//! map-scene definitions), 8 (sprites), 16 (locations), 23 (world map) and
//! 26 (texture metadata). Everything on disk is ready at once; in the
//! browser these archives are among those the title waits for.
use super::Assets;
use crate::{cache_pack::Source, texture::Material};
use rs09_cache::{archive::Archive, index::name_hash};
use std::path::Path;

/// Player variables for morphing locations: `(is_varbit, id) -> value`.
pub type Variables = Box<dyn Fn(bool, i32) -> i32>;

pub struct CacheAssets {
    configs: Archive,
    sprites: Archive,
    locations: Archive,
    maps: Archive,
    materials: Vec<Option<Material>>,
    /// Defaults to every variable reading 0.
    pub variables: Variables,
}
impl CacheAssets {
    pub fn open(directory: &Path) -> Result<Self, String> {
        let source = Source::open(directory)?;
        Self::from_archives(|id| source.archive(id))
    }

    /// The same, reading the data file when a group is asked for instead of
    /// holding the whole cache in memory: for a client that keeps the map's
    /// assets attached for as long as it runs.
    pub fn open_on_demand(directory: &Path) -> Result<Self, String> {
        use rs09_cache::store::{StorageMode, Store};
        let limits = rs09_cache::Limits::default();
        let store = match crate::cache_live::store_for(directory) {
            Some(store) => store,
            None => Store::open(directory, StorageMode::File, limits)
                .map_err(|e| format!("{}: {e}", directory.display()))?,
        };
        Self::from_archives(|id| {
            Archive::open(store.clone(), id, None, limits).map_err(|e| format!("archive {id}: {e}"))
        })
    }

    /// The same over a store that is already open: the browser's downloaded
    /// cache, read as it stands.
    pub fn from_store(store: rs09_cache::store::Store) -> Result<Self, String> {
        let limits = rs09_cache::Limits::default();
        Self::from_archives(|id| {
            Archive::open(store.clone(), id, None, limits).map_err(|e| format!("archive {id}: {e}"))
        })
    }

    fn from_archives(archive: impl Fn(u8) -> Result<Archive, String>) -> Result<Self, String> {
        let textures = archive(26)?;
        let materials = textures
            .file(0, 0, [0; 4])
            .map_err(|e| format!("texture metadata: {e}"))?
            .ok_or("texture metadata is missing")?;
        Ok(Self {
            configs: archive(2)?,
            sprites: archive(8)?,
            locations: archive(16)?,
            maps: archive(23)?,
            materials: crate::texture::materials(&materials).map_err(|e| e.to_string())?,
            variables: Box::new(|_, _| 0),
        })
    }

    /// `Sprites.mapfunctionId`: the sprite group named `mapfunction`.
    pub fn map_function_sprite(&self) -> Option<i32> {
        self.sprites
            .index
            .group_by_ascii_name("mapfunction")
            .map(|id| id as i32)
    }

    /// Every group name of archive 23 cannot be listed (only hashes are
    /// stored); this reports whether one exists.
    fn map_group(&self, group: &str) -> Option<u32> {
        self.maps.index.group_by_ascii_name(group)
    }

    fn file(archive: &Archive, group: u32, file: i32) -> Option<Vec<u8>> {
        let file = u32::try_from(file).ok()?;
        archive
            .file(group, file, [0; 4])
            .ok()
            .flatten()
            .map(|bytes| bytes.as_ref().clone())
    }
}
impl Assets for CacheAssets {
    fn group_ready(&self, group: &str) -> bool {
        self.map_group(group).is_some()
    }
    fn group_valid(&self, group: &str) -> bool {
        self.map_group(group).is_some()
    }
    fn group_percentage(&self, group: &str) -> i32 {
        if self.map_group(group).is_some() {
            100
        } else {
            0
        }
    }
    fn map_file(&self, group: &str, file: &str) -> Option<Vec<u8>> {
        let group = self.map_group(group)?;
        let file = self
            .maps
            .index
            .group(group)?
            .file_by_hash(name_hash(file.to_ascii_lowercase().as_bytes()))?;
        Self::file(&self.maps, group, file as i32)
    }
    fn group_files(&self, group: &str) -> Option<Vec<Vec<u8>>> {
        let group = self.map_group(group)?;
        let files = self.maps.group(group, [0; 4]).ok().flatten()?;
        Some(files.iter().map(|(_, data)| data.to_vec()).collect())
    }
    fn underlay(&self, id: i32) -> Option<Vec<u8>> {
        Self::file(&self.configs, 1, id)
    }
    fn overlay(&self, id: i32) -> Option<Vec<u8>> {
        Self::file(&self.configs, 4, id)
    }
    fn overlay_capacity(&self) -> i32 {
        self.configs
            .index
            .group(4)
            .map_or(0, |group| group.capacity as i32)
    }
    fn map_scene(&self, id: i32) -> Option<Vec<u8>> {
        Self::file(&self.configs, 34, id)
    }
    fn location(&self, id: i32) -> Option<Vec<u8>> {
        Self::file(&self.locations, (id as u32) >> 8, id & 0xff)
    }
    fn sprite(&self, id: i32) -> Option<Vec<u8>> {
        Self::file(&self.sprites, u32::try_from(id).ok()?, 0)
    }
    fn texture_repeating(&self, id: i32) -> bool {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.materials.get(i)?.as_ref())
            .is_some_and(|material| material.repeating)
    }
    fn texture_average_color(&self, id: i32) -> i32 {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.materials.get(i)?.as_ref())
            .map_or(0, |material| i32::from(material.average_color))
    }
    fn varbit(&self, id: i32) -> i32 {
        (self.variables)(true, id)
    }
    fn varp(&self, id: i32) -> i32 {
        (self.variables)(false, id)
    }
}
