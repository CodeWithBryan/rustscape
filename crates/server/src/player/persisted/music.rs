//! MusicPlayer's unlocked songs: a HashMap from list index to MusicEntry,
//! against the server-wide MusicEntry table. Playing, looping, the music
//! list varps and the air guitar unlock are not ported.
use super::IntHashMap;
use crate::player::save::{Error, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Song {
    pub name: String,
    pub index: i32,
}
/// MusicEntry.SONGS, keyed by music id. Its content is world start-up input.
#[derive(Clone, Debug, Default)]
pub struct Songs {
    pub songs: BTreeMap<i32, Song>,
}
impl Songs {
    pub fn for_id(&self, id: i32) -> Option<&Song> {
        self.songs.get(&id)
    }
}

/// `unlocked`, holding each entry's music id under its list index.
#[derive(Clone, Debug, Default)]
pub struct Player {
    pub unlocked: IntHashMap<i32>,
}
impl Player {
    /// The parser's `unlocked.put(entry.index, entry)`: an unknown id fails on
    /// the null entry, and a second song with the same index replaces the first.
    pub fn put(&mut self, songs: &Songs, id: i32) -> Result<()> {
        let index = songs.for_id(id).ok_or(Error::Null)?.index;
        self.unlocked.put(index, id);
        Ok(())
    }
    pub fn clear_unlocked(&mut self) {
        self.unlocked.clear();
    }
    pub fn has_unlocked(&self, songs: &Songs, id: i32) -> bool {
        songs
            .for_id(id)
            .is_some_and(|song| self.unlocked.contains_key(song.index))
    }
    pub fn has_unlocked_index(&self, index: i32) -> bool {
        self.unlocked.contains_key(index)
    }
    pub fn has_air_guitar(&self, songs: &Songs) -> bool {
        self.unlocked.len() >= 200 || self.unlocked.len() == songs.songs.len()
    }
    /// What saveMusicPlayer writes: the values' ids in the map's order.
    pub fn saved(&self) -> Vec<i32> {
        self.unlocked.iter().map(|(_, id)| *id).collect()
    }
}
