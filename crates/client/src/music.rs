//! Music: the cache files the synthesizer reads, and the calls the original
//! client makes on `MidiPlayer`/`MusicPlayer` from its packet handlers,
//! scripts and main loop.
//!
//! Nothing here knows the audio device. A device with its own thread
//! (Windows) reads the sequencer in 256-frame blocks through [`Device`]; one
//! without (the browser, whose audio thread cannot share this memory) is
//! handed the device after every client cycle and takes its blocks then
//! ([`Music::set_feed`]).
use rs09_audio::{
    music::{Archive as SongArchive, Channel, MidiPcmStream, Player, Source},
    sound_stream::Output,
};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

/// `Js5.isGroupReady` for a store that is still filling: asked with an
/// archive and a group its index lists, before the group is read.
pub type Ready = Box<dyn FnMut(u8, u32) -> bool + Send>;

/// The five archives music reads: songs (6), jingles (11), synthesized
/// sounds (4), Vorbis samples (14) and instruments (15).
pub struct CacheSource {
    songs: Archive,
    jingles: Archive,
    synth: Archive,
    vorbis: Archive,
    instruments: Archive,
    ready: Option<Ready>,
}

impl CacheSource {
    pub fn open(directory: &Path) -> Result<Self, String> {
        if let Some(store) = crate::cache_live::store_for(directory) {
            return Self::from_store(store);
        }
        let limits = Limits::default();
        let store = Store::open(directory, StorageMode::File, limits).map_err(|e| e.to_string())?;
        Self::from_store(store)
    }

    /// The same over an opened store. The five indexes have to be in it.
    pub fn from_store(store: Store) -> Result<Self, String> {
        let limits = Limits::default();
        let open = |id| {
            Archive::open(store.clone(), id, None, limits)
                .map_err(|e| format!("music archive {id}: {e}"))
        };
        Ok(Self {
            songs: open(6)?,
            jingles: open(11)?,
            synth: open(4)?,
            vorbis: open(14)?,
            instruments: open(15)?,
            ready: None,
        })
    }

    /// For a store the update connection is still filling: a group is read
    /// only once `ready` says it has arrived, and until then the file is
    /// reported as not there yet, which the player takes as "ask again next
    /// cycle". `ready` is what requests the group from the server. The
    /// original fetches songs (6) and Vorbis samples (14) this way.
    pub fn on_demand(mut self, ready: Ready) -> Self {
        self.ready = Some(ready);
        self
    }

    /// `js5Archive6.getGroupId(name)`: a song's group by its name.
    pub fn song_named(&self, name: &str) -> Option<i32> {
        self.songs
            .index
            .group_by_ascii_name(name)
            .and_then(|group| i32::try_from(group).ok())
    }

    fn read(&mut self, id: u8, group: u32, file: u32) -> Result<Option<Vec<u8>>, String> {
        let archive = match id {
            6 => &self.songs,
            11 => &self.jingles,
            4 => &self.synth,
            14 => &self.vorbis,
            _ => &self.instruments,
        };
        if archive.index.group(group).is_none() {
            return Ok(None);
        }
        if let Some(ready) = &mut self.ready
            && !ready(id, group)
        {
            return Ok(None);
        }
        // A group an update has not delivered yet is "not there yet" too
        // (it is asked for; `crate::on_demand::arriving`).
        crate::on_demand::arriving(|| archive.file(group, file, [0; 4]))
            .map(|bytes| bytes.map(|bytes| bytes.to_vec()))
            .map_err(|e| e.to_string())
    }

    /// `Js5.fetchFile(int)`: a file of the archive's only group, or a group
    /// of one file. The original throws for anything else.
    fn single(&mut self, archive: u8, id: i32) -> Result<Option<Vec<u8>>, String> {
        let index = match archive {
            4 => &self.synth.index,
            14 => &self.vorbis.index,
            _ => &self.instruments.index,
        };
        let id = u32::try_from(id).map_err(|_| "negative music file".to_owned())?;
        if index.capacity == 1 {
            self.read(archive, 0, id)
        } else if index.group(id).is_some_and(|g| g.capacity == 1) {
            self.read(archive, id, 0)
        } else {
            Err(format!("music file {id} is not a single file"))
        }
    }
}

impl Source for CacheSource {
    fn song(&mut self, jingle: bool, group: i32) -> Result<Option<Vec<u8>>, String> {
        let group = u32::try_from(group).map_err(|_| "negative song".to_owned())?;
        self.read(if jingle { 11 } else { 6 }, group, 0)
    }
    fn instrument(&mut self, id: i32) -> Result<Option<Vec<u8>>, String> {
        self.single(15, id)
    }
    fn synth(&mut self, id: i32) -> Result<Option<Vec<u8>>, String> {
        self.single(4, id)
    }
    fn vorbis(&mut self, id: i32) -> Result<Option<Vec<u8>>, String> {
        self.single(14, id)
    }
    fn vorbis_setup(&mut self) -> Result<Option<Vec<u8>>, String> {
        self.read(14, 0, 0)
    }
}
/// What the audio device thread reads: the sequencer and the block reader
/// (`musicStream` and `musicChannel` in the original).
pub struct Device {
    pub stream: MidiPcmStream,
    pub channel: Channel,
}
impl Device {
    /// The next 256 stereo frames, as the original's signed 24-bit mix.
    pub fn read(&mut self, block: &mut [i32; 512]) {
        self.channel.read(&mut self.stream, block);
    }
}

pub type Shared = Arc<Mutex<Device>>;

/// A song or jingle asked for by the server or a script.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Request {
    Song(i32),
    Jingle(i32),
}

pub struct Music {
    player: Player,
    source: Box<dyn Source + Send>,
    device: Shared,
    /// `Preferences.musicVolume` as last applied.
    volume: i32,
    feed: Option<Feed>,
}

/// Called with the device at the end of every [`Music::cycle`].
pub type Feed = Box<dyn FnMut(&mut Device) + Send>;

impl Music {
    /// A silent player at `volume` (`Preferences.musicVolume`, 0 to 255).
    pub fn new(source: Box<dyn Source + Send>, volume: i32) -> Self {
        let mut stream = MidiPcmStream::new(Output {
            sample_rate: 22050,
            stereo: true,
        });
        stream.init();
        Self {
            player: Player::new(),
            source,
            device: Arc::new(Mutex::new(Device {
                stream,
                channel: Channel::default(),
            })),
            volume: volume.clamp(0, 255),
            feed: None,
        }
    }

    /// For an audio device that cannot read [`Self::device`] from its own
    /// thread: `feed` is called once per client cycle, after the player has
    /// run, and reads however many blocks the device is short of.
    pub fn set_feed(&mut self, feed: Feed) {
        self.feed = Some(feed);
    }

    /// The handle the audio device thread reads from.
    pub fn device(&self) -> Shared {
        self.device.clone()
    }

    fn lock(device: &Shared) -> MutexGuard<'_, Device> {
        // A panic elsewhere must not silence or kill the audio thread.
        device.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// MIDI_SONG / MIDI_JINGLE and their script commands.
    pub fn request(&mut self, request: Request) {
        match request {
            Request::Song(id) => {
                let mut device = Self::lock(&self.device);
                self.player.play_song(&mut device.stream, id, self.volume);
            }
            Request::Jingle(id) => self.player.play_jingle(id, self.volume),
        }
    }

    /// Script command 6019: a change of `Preferences.musicVolume`.
    pub fn set_volume(&mut self, volume: i32) {
        let volume = volume.clamp(0, 255);
        if volume == self.volume {
            return;
        }
        let mut device = Self::lock(&self.device);
        if self.volume == 0 && self.player.group_id != -1 {
            self.player
                .play_immediate(SongArchive::Songs, self.player.group_id, volume);
            self.player.jingle = false;
        } else if volume == 0 {
            self.player.stop(&mut device.stream);
            self.player.jingle = false;
        } else {
            self.player.set_volume(&mut device.stream, volume);
        }
        self.volume = volume;
    }

    /// `MidiPlayer.playFadeOut()` alone, as `client.setGameState` calls it
    /// when the title screen is left: the song fades, `MusicPlayer.groupId`
    /// and the jingle flag stay.
    pub fn fade_out(&mut self) {
        self.player.fade_out();
    }

    /// The title screen's song (`MusicPlayer.titleSong`), or silence when
    /// music is off.
    pub fn title(&mut self, song: i32) {
        self.player.group_id = song;
        if self.volume == 0 {
            self.player.fade_out();
        } else {
            self.player
                .play_fade_out(song, SongArchive::Songs, self.volume);
        }
    }

    /// Leaving the title screen, and the scene reset on login or logout:
    /// fade out and forget the area's song.
    pub fn reset(&mut self) {
        self.fade_out();
        self.player.jingle = false;
        self.player.group_id = -1;
    }

    /// One client cycle: the fade (`MidiPlayer.loop`), one loading step
    /// (`tryStartLoadedSong`) and the end of `SoundPlayer.loop`. Returns the
    /// song to report in SOUND_SONGEND when the area's song has finished.
    pub fn cycle(&mut self) -> Option<i32> {
        let mut device = Self::lock(&self.device);
        let device = &mut *device;
        self.player.cycle(&mut device.stream);
        self.player
            .try_start_loaded_song(&mut device.stream, self.source.as_mut());
        let ended = if self.player.after_sounds(&device.stream, self.volume) {
            let ended = self.player.group_id;
            self.player.group_id = -1;
            Some(ended)
        } else {
            None
        };
        if let Some(feed) = &mut self.feed {
            feed(device);
        }
        ended
    }

    pub fn status(&self) -> serde_json::Value {
        let device = Self::lock(&self.device);
        serde_json::json!({
            "song": self.player.group_id,
            "jingle": self.player.jingle,
            "state": self.player.state(),
            "playing": self.player.is_playing(&device.stream),
            "voices": device.stream.voices(),
            "volume": device.stream.volume(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source with no files: nothing can load, so nothing starts.
    struct Empty;
    impl Source for Empty {
        fn song(&mut self, _: bool, _: i32) -> Result<Option<Vec<u8>>, String> {
            Ok(None)
        }
        fn instrument(&mut self, _: i32) -> Result<Option<Vec<u8>>, String> {
            Ok(None)
        }
        fn synth(&mut self, _: i32) -> Result<Option<Vec<u8>>, String> {
            Ok(None)
        }
        fn vorbis(&mut self, _: i32) -> Result<Option<Vec<u8>>, String> {
            Ok(None)
        }
        fn vorbis_setup(&mut self) -> Result<Option<Vec<u8>>, String> {
            Ok(None)
        }
    }

    #[test]
    fn requests_follow_the_original_player_rules() {
        let mut music = Music::new(Box::new(Empty), 255);
        // A song request fades the (silent) stream out and moves to loading.
        music.request(Request::Song(76));
        assert_eq!(music.status()["song"], 76);
        assert_eq!(music.status()["state"], 1);
        assert_eq!(music.cycle(), None);
        assert_eq!(music.status()["state"], 2);
        // The file never arrives, so the player keeps waiting and never
        // reports the song as ended.
        for _ in 0..10 {
            assert_eq!(music.cycle(), None);
        }
        assert_eq!(music.status()["playing"], true);
        // A jingle interrupts; song requests during it only record the song.
        music.request(Request::Jingle(5));
        assert_eq!(music.status()["jingle"], true);
        music.request(Request::Song(12));
        assert_eq!(music.status()["song"], 12);
        assert_eq!(music.status()["jingle"], true);
        // Turning music off stops it and clears the jingle flag.
        music.set_volume(0);
        assert_eq!(music.status()["jingle"], false);
        assert_eq!(music.cycle(), None);
        assert_eq!(music.status()["playing"], false);
        // With music off nothing is requested and no song end is reported.
        music.request(Request::Song(30));
        assert_eq!(music.status()["state"], 0);
        assert_eq!(music.cycle(), None);
        // Turning it back on restarts the area's song at once.
        music.set_volume(128);
        assert_eq!(music.status()["state"], 1);
        // Song -1 stops the music.
        music.request(Request::Song(-1));
        assert_eq!(music.status()["song"], -1);
        music.reset();
        assert_eq!(music.status()["song"], -1);
    }

    #[test]
    fn a_finished_song_is_reported_once() {
        let mut music = Music::new(Box::new(Empty), 255);
        // Nothing is loading and nothing plays, but an area song is recorded:
        // the original reports it as ended and forgets it.
        music.player.group_id = 9;
        assert_eq!(music.cycle(), Some(9));
        assert_eq!(music.cycle(), None);
    }
}
