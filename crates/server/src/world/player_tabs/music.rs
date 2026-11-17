//! `MusicPlayer` and `MusicZone.enter`: the music tab's list varps and access
//! mask, unlocking and playing tracks, looping, the 20-tick replay and the
//! Air Guitar emote that follows the unlocked count.
//!
//! The unlocked songs are the player's persisted
//! [`music::Player`](crate::player::persisted::music::Player) (list index to
//! music id, in the JDK HashMap's order) against the world's
//! [`Songs`](crate::player::persisted::music::Songs). The current track,
//! playing and looping flags are [`Playback`]; the original does not save
//! them. A region's music zones reach [`zone_enter`] through the zones
//! library's music service (`world::zones`), which the world binding answers
//! by calling it.
use super::{Error, Result, emotes::Emote};
use crate::player::persisted::music::{Player as Unlocked, Song, Songs};

/// `TUTORIAL_MUSIC`.
pub const TUTORIAL_MUSIC: i32 = 62;
/// `DEFAULT_MUSIC_ID`.
pub const DEFAULT_MUSIC_ID: i32 = 76;
/// `CONFIG_IDS`: the music list varps, 32 songs each.
pub const CONFIG_IDS: [i32; 20] = [
    20, 21, 22, 23, 24, 25, 298, 311, 346, 414, 464, 598, 662, 721, 906, 1009, 1104, 1136, 1180,
    1202,
];
/// The music tab.
pub const MUSIC_TAB: i32 = 187;

/// The MusicPlayer's fields besides the unlocked songs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Playback {
    /// `currentMusicId`.
    pub current_music_id: i32,
    /// `playing`.
    pub playing: bool,
    /// `looping`.
    pub looping: bool,
}

/// Every call `MusicPlayer` makes outside its class, for the owning player.
pub trait Host {
    /// `ContentAPI.setVarp(player, id, value, save)`.
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()>;
    /// `packetDispatch.sendIfaceSettings(hash, child, interface, offset, length)`.
    fn access_mask(
        &mut self,
        hash: i32,
        child: i32,
        interface: i32,
        offset: i32,
        length: i32,
    ) -> Result<()>;
    /// `packetDispatch.sendMessage(text)`.
    fn message(&mut self, text: &str) -> Result<()>;
    /// The MusicPacket: `MusicContext(player, id)` (`secondary` false).
    fn music(&mut self, id: i32, secondary: bool) -> Result<()>;
    /// The String packet.
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> Result<()>;
    /// `player.isDebug()`.
    fn is_debug(&mut self) -> Result<bool>;
    /// `GameWorld.getTicks()`.
    fn ticks(&mut self) -> Result<i32>;
    /// `player.getEmoteManager().isUnlocked(emote)`.
    fn emote_unlocked(&mut self, emote: Emote) -> Result<bool>;
    /// `player.getEmoteManager().unlock(emote)`.
    fn emote_unlock(&mut self, emote: Emote) -> Result<bool>;
    /// `player.getEmoteManager().lock(emote)`.
    fn emote_lock(&mut self, emote: Emote) -> Result<bool>;
}

/// The MusicPlayer object: its unlocked songs, its playback fields and the
/// world's song table.
pub struct MusicPlayer<'a> {
    pub unlocked: &'a mut Unlocked,
    pub playback: &'a mut Playback,
    pub songs: &'a Songs,
}

impl MusicPlayer<'_> {
    /// `init()`.
    pub fn init<H: Host>(&mut self, h: &mut H) -> Result<()> {
        self.refresh_list(h)?;
        h.varp(19, i32::from(self.playback.looping), false)?;
        let mut value = 0i32;
        for i in 0..CONFIG_IDS.len() {
            value |= 2i32.wrapping_shl(i as u32);
        }
        h.access_mask(value, 1, MUSIC_TAB, 0, CONFIG_IDS.len() as i32 * 64)?;
        // The key is a list index: a song whose index is 62 counts, not music 62.
        if !self.unlocked.has_unlocked_index(TUTORIAL_MUSIC) {
            self.unlock(h, TUTORIAL_MUSIC, false)?;
        }
        if !self.is_music_playing() {
            self.play_default(h)?;
        }
        if !self.has_air_guitar() && h.emote_unlocked(Emote::AirGuitar)? {
            h.message(
                "As you no longer have all music unlocked, the Air Guitar emote is locked again.",
            )?;
            h.emote_lock(Emote::AirGuitar)?;
        }
        Ok(())
    }

    /// `hasAirGuitar()`.
    pub fn has_air_guitar(&self) -> bool {
        self.unlocked.has_air_guitar(self.songs)
    }

    /// `hasUnlocked(musicId)`.
    pub fn has_unlocked(&self, id: i32) -> bool {
        self.unlocked.has_unlocked(self.songs, id)
    }

    /// `refreshList()`: one varp per config, each the OR of its 32 songs' bits.
    pub fn refresh_list<H: Host>(&mut self, h: &mut H) -> Result<()> {
        let mut values = [0i32; CONFIG_IDS.len()];
        for index in self.unlocked.unlocked.keys() {
            let config = index / 32;
            if config >= CONFIG_IDS.len() as i32 {
                continue;
            }
            // A negative index fails as the original's array store does.
            let slot = usize::try_from(config).map_err(|_| Error::Index)?;
            values[slot] |= 1i32.wrapping_shl((index & 31) as u32);
        }
        for (id, value) in CONFIG_IDS.iter().zip(values) {
            h.varp(*id, value, false)?;
        }
        Ok(())
    }

    /// `playDefault()`.
    pub fn play_default<H: Host>(&mut self, h: &mut H) -> Result<()> {
        if self.songs.for_id(DEFAULT_MUSIC_ID).is_some() {
            self.play(h, DEFAULT_MUSIC_ID)?;
        }
        Ok(())
    }

    /// `replay()`.
    pub fn replay<H: Host>(&mut self, h: &mut H) -> Result<()> {
        let id = self.playback.current_music_id;
        if self.songs.for_id(id).is_some() {
            self.play(h, id)?;
        }
        Ok(())
    }

    /// `play(MusicEntry.forId(id))`: an unknown id is a null entry.
    pub fn play<H: Host>(&mut self, h: &mut H, id: i32) -> Result<()> {
        let entry: Option<Song> = self.songs.for_id(id).cloned();
        // `!looping || currentMusicId == entry.getId()`: a looping player reads
        // the null entry's id at once, any other at the packet.
        let matches = !self.playback.looping || {
            entry.as_ref().ok_or(Error::Null)?;
            self.playback.current_music_id == id
        };
        if matches {
            let entry = entry.ok_or(Error::Null)?;
            h.music(id, false)?;
            let debug = h.is_debug()?;
            let name = if debug {
                format!("{} ({id})", entry.name)
            } else {
                entry.name.clone()
            };
            h.send_string(&name, MUSIC_TAB, 14)?;
            self.playback.current_music_id = id;
            self.playback.playing = true;
        }
        Ok(())
    }

    /// `unlock(id, play)`; `unlock(id)` plays.
    pub fn unlock<H: Host>(&mut self, h: &mut H, id: i32, play: bool) -> Result<()> {
        let Some(entry) = self.songs.for_id(id).cloned() else {
            return Ok(());
        };
        if entry.name != " " && !self.unlocked.has_unlocked_index(entry.index) {
            self.unlocked.unlocked.put(entry.index, id);
            h.message(&format!(
                "<col=FF0000>You have unlocked a new music track: {}.</col>",
                entry.name
            ))?;
            self.refresh_list(h)?;
            if !h.emote_unlocked(Emote::AirGuitar)? && self.has_air_guitar() {
                h.emote_unlock(Emote::AirGuitar)?;
                if self.unlocked.unlocked.len() >= 200 {
                    h.message("You've unlocked 200 music tracks and can use a new emote!")?;
                } else {
                    h.message("You've unlocked all music tracks and can use a new emote!")?;
                }
            }
        }
        if play {
            self.play(h, id)?;
        }
        Ok(())
    }

    /// `tick()`: every 20 ticks a stopped track is sent again. The send is in
    /// a `try` whose `catch (Exception)` drops any failure.
    pub fn tick<H: Host>(&mut self, h: &mut H) -> Result<()> {
        if h.ticks()? % 20 == 0 && !self.playback.playing {
            let _ = h.music(self.playback.current_music_id, false);
        }
        Ok(())
    }

    /// `toggleLooping()`.
    pub fn toggle_looping<H: Host>(&mut self, h: &mut H) -> Result<()> {
        self.playback.looping = !self.playback.looping;
        h.varp(19, i32::from(self.playback.looping), false)
    }

    /// `setLooping(looping)`.
    pub fn set_looping<H: Host>(&mut self, h: &mut H, looping: bool) -> Result<()> {
        self.playback.looping = looping;
        h.varp(19, i32::from(looping), false)
    }

    /// `isMusicPlaying()`.
    fn is_music_playing(&self) -> bool {
        self.playback.current_music_id > 0 && self.playback.playing
    }
}

/// `MusicZone.enter(entity)`: a player unlocks (and plays) the zone's track;
/// any other entity is refused.
pub fn zone_enter<H: Host>(
    player: Option<&mut MusicPlayer<'_>>,
    h: &mut H,
    music_id: i32,
) -> Result<bool> {
    let Some(player) = player else {
        return Err(Error::IllegalState("Music is for players only!".into()));
    };
    player.unlock(h, music_id, true)?;
    Ok(true)
}
