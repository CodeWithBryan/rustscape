//! RT4 music: the song file decoder and the MIDI event reader.
//!
//! Transcribed from the rev 530 client: `rt4.Song` (`client!rf`) expands the
//! cache's compact song format into standard MIDI bytes and collects, per
//! program, the keys a song plays; `rt4.MidiDecoder` (`client!ki`) reads the
//! events of those bytes track by track. [`Programs`] reproduces the part of
//! `rt4.HashTable` (`client!sc`) holding `rt4.ByteArrayNode`s (`client!ea`)
//! that `Song.programs` exposes, including its iteration order.
//!
//! Valid input behaves exactly as the original, with Java `int`/`long`
//! wrapping. Where the original would throw on malformed input
//! (`ArrayIndexOutOfBoundsException`, `RuntimeException`) this module returns
//! `Err` from [`Song::decode`] and [`MidiDecoder::init`], and the per-event
//! decoder methods end the affected track instead of panicking.

/// Bucket count of the `new HashTable(16)` that `Song.createPrograms` allocates.
const PROGRAM_BUCKETS: usize = 16;

/// One `ByteArrayNode` of `Song.programs`: `key` and the 128-entry `value`
/// (1 where the song sounds that key on the program, else 0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub id: i32,
    pub keys: [u8; 128],
}

/// `Song.programs`: the original's `HashTable(16)` keyed by program id.
///
/// `HashTable.put` appends to the bucket `key & 15` and `head()`/`next()` walk
/// bucket 0 to 15, oldest node first, so that is the order of [`Programs::iter`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Programs {
    buckets: [Vec<Program>; PROGRAM_BUCKETS],
}

impl Default for Programs {
    fn default() -> Self {
        Self::new()
    }
}

impl Programs {
    pub fn new() -> Self {
        Self {
            buckets: std::array::from_fn(|_| Vec::new()),
        }
    }

    fn bucket(id: i32) -> usize {
        (i64::from(id) & (PROGRAM_BUCKETS as i64 - 1)) as usize
    }

    /// `HashTable.get(program)`.
    pub fn get(&self, id: i32) -> Option<&[u8; 128]> {
        self.buckets[Self::bucket(id)]
            .iter()
            .find(|program| program.id == id)
            .map(|program| &program.keys)
    }

    /// `HashTable.get`, then `put(new ByteArrayNode(new byte[128]), program)` when absent.
    fn get_or_put(&mut self, id: i32) -> &mut [u8; 128] {
        let bucket = &mut self.buckets[Self::bucket(id)];
        let index = match bucket.iter().position(|program| program.id == id) {
            Some(index) => index,
            None => {
                bucket.push(Program { id, keys: [0; 128] });
                bucket.len() - 1
            }
        };
        &mut bucket[index].keys
    }

    /// The programs in the order of the original's `head()`/`next()` walk,
    /// which is the order `MidiPcmStream.isSongReady` loads instruments in.
    pub fn iter(&self) -> impl Iterator<Item = &Program> {
        self.buckets.iter().flatten()
    }

    /// `HashTable.size()`.
    pub fn len(&self) -> usize {
        self.buckets.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.iter().all(Vec::is_empty)
    }
}

/// `Song`: the expanded standard-MIDI bytes and the program -> used-keys table.
#[derive(Clone, Debug)]
pub struct Song {
    pub midi_bytes: Vec<u8>,
    /// `null` until [`Song::create_programs`], and again after [`Song::release_programs`].
    pub programs: Option<Programs>,
}

fn byte(data: &[u8], position: usize) -> Result<u8, String> {
    data.get(position)
        .copied()
        .ok_or_else(|| format!("song: read at {position} past the end ({})", data.len()))
}

/// `in.data[position]` as the signed Java `byte`, widened.
fn signed(data: &[u8], position: &mut usize) -> Result<i32, String> {
    let value = byte(data, *position)? as i8;
    *position += 1;
    Ok(i32::from(value))
}

/// `Buffer.gVarInt`.
fn g_var_int(data: &[u8], offset: &mut usize) -> Result<i32, String> {
    let mut b = byte(data, *offset)? as i8;
    *offset += 1;
    let mut value = 0i32;
    while b < 0 {
        value = (i32::from(b) & 0x7F | value) << 7;
        b = byte(data, *offset)? as i8;
        *offset += 1;
    }
    Ok(i32::from(b) | value)
}

/// The output `Buffer` wrapped around `midiBytes`.
struct Out {
    data: Vec<u8>,
    offset: usize,
}

impl Out {
    fn p1(&mut self, value: i32) -> Result<(), String> {
        let length = self.data.len();
        let slot = self
            .data
            .get_mut(self.offset)
            .ok_or_else(|| format!("song: expanded MIDI exceeds its computed length {length}"))?;
        *slot = value as u8;
        self.offset += 1;
        Ok(())
    }

    fn p2(&mut self, value: i32) -> Result<(), String> {
        self.p1(value >> 8)?;
        self.p1(value)
    }

    fn p4(&mut self, value: i32) -> Result<(), String> {
        self.p1(value >> 24)?;
        self.p1(value >> 16)?;
        self.p1(value >> 8)?;
        self.p1(value)
    }

    /// `Buffer.pVarInt`: the shifts are logical (`>>>`).
    fn p_var_int(&mut self, value: i32) -> Result<(), String> {
        let bits = value as u32;
        if bits & 0xFFFF_FF80 != 0 {
            if bits & 0xFFFF_C000 != 0 {
                if bits & 0xFFE0_0000 != 0 {
                    if bits & 0xF000_0000 != 0 {
                        self.p1((bits >> 28 | 0x80) as i32)?;
                    }
                    self.p1((bits >> 21 | 0x80) as i32)?;
                }
                self.p1((bits >> 14 | 0x80) as i32)?;
            }
            self.p1((bits >> 7 | 0x80) as i32)?;
        }
        self.p1(value & 0x7F)
    }
}

impl Song {
    /// The `Song(Buffer)` constructor.
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        if data.len() < 3 {
            return Err(format!("song: {} bytes is too short", data.len()));
        }
        let trailer = data.len() - 3;
        let tracks = usize::from(data[trailer]);
        let division = i32::from(data[trailer + 1]) << 8 | i32::from(data[trailer + 2]);
        let mut midi_len = tracks * 10 + 14;
        let mut offset = 0usize;
        let mut tempo_changes = 0usize;
        let mut controller_events = 0usize;
        let mut note_on_events = 0usize;
        let mut note_off_events = 0usize;
        let mut pitch_wheel_events = 0usize;
        let mut channel_pressure_events = 0usize;
        let mut key_pressure_events = 0usize;
        let mut bank_select_events = 0usize;
        for _ in 0..tracks {
            let mut status = -1i32;
            loop {
                let status_and_channel = i32::from(byte(data, offset)?);
                offset += 1;
                // The whole event byte is compared with the previous low nibble.
                if status_and_channel != status {
                    midi_len += 1;
                }
                status = status_and_channel & 0xF;
                if status_and_channel == 7 {
                    break;
                }
                if status_and_channel == 23 {
                    tempo_changes += 1;
                } else {
                    match status {
                        0 => note_on_events += 1,
                        1 => note_off_events += 1,
                        2 => controller_events += 1,
                        3 => pitch_wheel_events += 1,
                        4 => channel_pressure_events += 1,
                        5 => key_pressure_events += 1,
                        6 => bank_select_events += 1,
                        _ => {
                            return Err(format!(
                                "song: unknown event byte {status_and_channel} at {}",
                                offset - 1
                            ));
                        }
                    }
                }
            }
        }
        midi_len += tempo_changes * 5;
        midi_len += (note_on_events
            + note_off_events
            + controller_events
            + pitch_wheel_events
            + key_pressure_events)
            * 2;
        midi_len += channel_pressure_events + bank_select_events;
        let delta_time_pos = offset;
        let events = tracks
            + tempo_changes
            + controller_events
            + note_on_events
            + note_off_events
            + pitch_wheel_events
            + channel_pressure_events
            + key_pressure_events
            + bank_select_events;
        for _ in 0..events {
            g_var_int(data, &mut offset)?;
        }
        midi_len += offset - delta_time_pos;
        let mut controller_pos = offset;
        let mut modulation_wheel_msb_events = 0usize;
        let mut modulation_wheel_lsb_events = 0usize;
        let mut channel_volume_msb_events = 0usize;
        let mut channel_volume_lsb_events = 0usize;
        let mut pan_msb_events = 0usize;
        let mut pan_lsb_events = 0usize;
        let mut non_registered_msb_events = 0usize;
        let mut non_registered_lsb_events = 0usize;
        let mut registered_msb_events = 0usize;
        let mut registered_lsb_events = 0usize;
        let mut other_known_controller_events = 0usize;
        let mut unknown_controller_events = 0usize;
        let mut controller = 0i32;
        for _ in 0..controller_events {
            controller = (controller + i32::from(byte(data, offset)?)) & 0x7F;
            offset += 1;
            match controller {
                // Program changes share the bank select value array.
                0 | 32 => bank_select_events += 1,
                1 => modulation_wheel_msb_events += 1,
                33 => modulation_wheel_lsb_events += 1,
                7 => channel_volume_msb_events += 1,
                39 => channel_volume_lsb_events += 1,
                10 => pan_msb_events += 1,
                42 => pan_lsb_events += 1,
                99 => non_registered_msb_events += 1,
                98 => non_registered_lsb_events += 1,
                101 => registered_msb_events += 1,
                100 => registered_lsb_events += 1,
                64 | 65 | 120 | 121 | 123 => other_known_controller_events += 1,
                _ => unknown_controller_events += 1,
            }
        }
        let mut event_pos = 0usize;
        let mut other_known_controller_pos = offset;
        offset += other_known_controller_events;
        let mut key_pressure_pos = offset;
        offset += key_pressure_events;
        let mut channel_pressure_pos = offset;
        offset += channel_pressure_events;
        let mut pitch_wheel_msb_pos = offset;
        offset += pitch_wheel_events;
        let mut modulation_wheel_msb_pos = offset;
        offset += modulation_wheel_msb_events;
        let mut channel_volume_msb_pos = offset;
        offset += channel_volume_msb_events;
        let mut pan_msb_pos = offset;
        offset += pan_msb_events;
        let mut key_pos = offset;
        offset += note_on_events + note_off_events + key_pressure_events;
        let mut on_velocity_pos = offset;
        offset += note_on_events;
        let mut unknown_controller_pos = offset;
        offset += unknown_controller_events;
        let mut off_velocity_pos = offset;
        offset += note_off_events;
        let mut modulation_wheel_lsb_pos = offset;
        offset += modulation_wheel_lsb_events;
        let mut channel_volume_lsb_pos = offset;
        offset += channel_volume_lsb_events;
        let mut pan_lsb_pos = offset;
        offset += pan_lsb_events;
        let mut bank_select_pos = offset;
        offset += bank_select_events;
        let mut pitch_wheel_lsb_pos = offset;
        offset += pitch_wheel_events;
        let mut non_registered_msb_pos = offset;
        offset += non_registered_msb_events;
        let mut non_registered_lsb_pos = offset;
        offset += non_registered_lsb_events;
        let mut registered_msb_pos = offset;
        offset += registered_msb_events;
        let mut registered_lsb_pos = offset;
        offset += registered_lsb_events;
        let mut tempo_pos = offset;

        let mut out = Out {
            data: vec![0; midi_len],
            offset: 0,
        };
        out.p4(0x4D54_6864)?; // "MThd"
        out.p4(6)?;
        out.p2(i32::from(tracks > 1))?;
        out.p2(tracks as i32)?;
        out.p2(division)?;
        offset = delta_time_pos;
        let mut channel = 0i32;
        let mut key_delta = 0i32;
        let mut on_velocity_delta = 0i32;
        let mut off_velocity_delta = 0i32;
        let mut pitch_wheel_delta = 0i32;
        let mut channel_pressure_delta = 0i32;
        let mut key_pressure_delta = 0i32;
        let mut values = [0i32; 128];
        controller = 0;
        for _ in 0..tracks {
            out.p4(0x4D54_726B)?; // "MTrk"
            out.offset += 4;
            let track_start = out.offset;
            let mut status_type = -1i32;
            loop {
                let delta_time = g_var_int(data, &mut offset)?;
                out.p_var_int(delta_time)?;
                let event_byte = i32::from(byte(data, event_pos)?);
                event_pos += 1;
                let status_changed = event_byte != status_type;
                status_type = event_byte & 0xF;
                if event_byte == 7 {
                    if status_changed {
                        out.p1(255)?;
                    }
                    out.p1(47)?;
                    out.p1(0)?;
                    // psize4(out.offset - trackStart)
                    let length = out.offset - track_start;
                    let size = out
                        .data
                        .get_mut(track_start - 4..track_start)
                        .ok_or("song: expanded MIDI exceeds its computed length")?;
                    size.copy_from_slice(&(length as u32).to_be_bytes());
                    break;
                }
                if event_byte == 23 {
                    if status_changed {
                        out.p1(255)?;
                    }
                    out.p1(81)?;
                    out.p1(3)?;
                    out.p1(signed(data, &mut tempo_pos)?)?;
                    out.p1(signed(data, &mut tempo_pos)?)?;
                    out.p1(signed(data, &mut tempo_pos)?)?;
                    continue;
                }
                channel ^= event_byte >> 4;
                match status_type {
                    0 => {
                        if status_changed {
                            out.p1(channel + 144)?;
                        }
                        key_delta = key_delta.wrapping_add(signed(data, &mut key_pos)?);
                        on_velocity_delta =
                            on_velocity_delta.wrapping_add(signed(data, &mut on_velocity_pos)?);
                        out.p1(key_delta & 0x7F)?;
                        out.p1(on_velocity_delta & 0x7F)?;
                    }
                    1 => {
                        if status_changed {
                            out.p1(channel + 128)?;
                        }
                        key_delta = key_delta.wrapping_add(signed(data, &mut key_pos)?);
                        off_velocity_delta =
                            off_velocity_delta.wrapping_add(signed(data, &mut off_velocity_pos)?);
                        out.p1(key_delta & 0x7F)?;
                        out.p1(off_velocity_delta & 0x7F)?;
                    }
                    2 => {
                        if status_changed {
                            out.p1(channel + 176)?;
                        }
                        controller = (controller + signed(data, &mut controller_pos)?) & 0x7F;
                        out.p1(controller)?;
                        let position = match controller {
                            0 | 32 => &mut bank_select_pos,
                            1 => &mut modulation_wheel_msb_pos,
                            33 => &mut modulation_wheel_lsb_pos,
                            7 => &mut channel_volume_msb_pos,
                            39 => &mut channel_volume_lsb_pos,
                            10 => &mut pan_msb_pos,
                            42 => &mut pan_lsb_pos,
                            99 => &mut non_registered_msb_pos,
                            98 => &mut non_registered_lsb_pos,
                            101 => &mut registered_msb_pos,
                            100 => &mut registered_lsb_pos,
                            64 | 65 | 120 | 121 | 123 => &mut other_known_controller_pos,
                            _ => &mut unknown_controller_pos,
                        };
                        let value_delta = signed(data, position)?;
                        let slot = &mut values[controller as usize];
                        *slot = value_delta.wrapping_add(*slot);
                        out.p1(*slot & 0x7F)?;
                    }
                    3 => {
                        if status_changed {
                            out.p1(channel + 224)?;
                        }
                        pitch_wheel_delta =
                            pitch_wheel_delta.wrapping_add(signed(data, &mut pitch_wheel_lsb_pos)?);
                        pitch_wheel_delta = pitch_wheel_delta
                            .wrapping_add(signed(data, &mut pitch_wheel_msb_pos)? << 7);
                        out.p1(pitch_wheel_delta & 0x7F)?;
                        out.p1(pitch_wheel_delta >> 7 & 0x7F)?;
                    }
                    4 => {
                        if status_changed {
                            out.p1(channel + 208)?;
                        }
                        channel_pressure_delta = channel_pressure_delta
                            .wrapping_add(signed(data, &mut channel_pressure_pos)?);
                        out.p1(channel_pressure_delta & 0x7F)?;
                    }
                    5 => {
                        if status_changed {
                            out.p1(channel + 160)?;
                        }
                        key_delta = key_delta.wrapping_add(signed(data, &mut key_pos)?);
                        key_pressure_delta =
                            key_pressure_delta.wrapping_add(signed(data, &mut key_pressure_pos)?);
                        out.p1(key_delta & 0x7F)?;
                        out.p1(key_pressure_delta & 0x7F)?;
                    }
                    6 => {
                        if status_changed {
                            out.p1(channel + 192)?;
                        }
                        // The program number is written unmasked.
                        out.p1(signed(data, &mut bank_select_pos)?)?;
                    }
                    _ => {
                        return Err(format!(
                            "song: unknown event byte {event_byte} at {}",
                            event_pos - 1
                        ));
                    }
                }
            }
        }
        Ok(Self {
            midi_bytes: out.data,
            programs: None,
        })
    }

    /// `releasePrograms()`.
    pub fn release_programs(&mut self) {
        self.programs = None;
    }

    /// The `(key, value)` of every program node in the order
    /// `MidiPcmStream.isSongReady` meets them with `programs.head()`/`next()`.
    /// Empty while `programs` is `null`, where the original would throw.
    pub fn programs_in_order(&self) -> Vec<(i32, [u8; 128])> {
        self.programs
            .iter()
            .flat_map(Programs::iter)
            .map(|program| (program.id, program.keys))
            .collect()
    }

    /// `createPrograms()`: replays the song once and records, for each program
    /// (bank select << 7 or << 14 plus the program number; channel 9 starts in
    /// bank 128), the keys sounded with a non-zero velocity.
    pub fn create_programs(&mut self) {
        if self.programs.is_some() {
            return;
        }
        let table = self.programs.insert(Programs::new());
        let mut banks = [0i32; 16];
        let mut programs = [0i32; 16];
        banks[9] = 128;
        programs[9] = 128;
        let mut song = MidiDecoder::new();
        // Cannot fail for bytes `decode` produced; the original would throw.
        if song.try_init(&self.midi_bytes).is_err() {
            return;
        }
        let track_count = song.get_track_count();
        for track in 0..track_count {
            song.load_track_position(track);
            song.add_delta_time(track);
            song.save_track_position(track);
        }
        loop {
            let track = song.get_next_track();
            // A song without tracks: the original throws here.
            let Some(&time) = song.times.get(track as usize) else {
                return;
            };
            let mut ended = false;
            while song.times[track as usize] == time {
                song.load_track_position(track);
                let event = song.get_next_event(track);
                if event == 1 {
                    song.load_end_of_track_position();
                    song.save_track_position(track);
                    ended = true;
                    break;
                }
                let event_type = event & 0xF0;
                if event_type == 176 {
                    let channel = (event & 0xF) as usize;
                    let data1 = event >> 8 & 0x7F;
                    let data2 = event >> 16 & 0x7F;
                    if data1 == 0 {
                        banks[channel] = (banks[channel] & !0x001F_C000) + (data2 << 14);
                    }
                    if data1 == 32 {
                        banks[channel] = (banks[channel] & !0x0000_3F80) + (data2 << 7);
                    }
                }
                if event_type == 192 {
                    let channel = (event & 0xF) as usize;
                    let data1 = event >> 8 & 0x7F;
                    programs[channel] = banks[channel] + data1;
                }
                if event_type == 144 {
                    let channel = (event & 0xF) as usize;
                    let data1 = event >> 8 & 0x7F;
                    let data2 = event >> 16 & 0x7F;
                    if data2 > 0 {
                        table.get_or_put(programs[channel])[data1 as usize] = 1;
                    }
                }
                song.add_delta_time(track);
                song.save_track_position(track);
            }
            // `continue nextTrack` re-tests `!song.hasNextTrack()`, which
            // despite its name is true once every track has ended.
            if ended && song.has_next_track() {
                return;
            }
        }
    }
}

/// `MidiDecoder.STATUS_LENGTHS`: data byte count for status `0x80 + index`.
pub const STATUS_LENGTHS: [u8; 128] = {
    let mut lengths = [2u8; 128];
    let mut index = 64;
    while index < 96 {
        lengths[index] = 1; // 0xC0..=0xDF
        index += 1;
    }
    let tail = [0u8, 1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]; // 0xF0..=0xFF
    let mut index = 0;
    while index < 16 {
        lengths[112 + index] = tail[index];
        index += 1;
    }
    lengths
};

/// `MidiDecoder`: a cursor over standard MIDI bytes with a saved position,
/// tick time and running status per track.
///
/// Track arguments are Java `int`s. Positions are offsets into the MIDI bytes;
/// `-1` (any negative value) marks a track that has ended.
#[derive(Clone, Debug, Default)]
pub struct MidiDecoder {
    /// `times`: the tick of each track's next event.
    pub times: Vec<i32>,
    positions: Vec<i32>,
    /// `division`: ticks per quarter note.
    pub division: i32,
    start_positions: Vec<i32>,
    /// Named `startMillis`, but in the unit of `tempo`: microseconds.
    start_millis: i64,
    statuses: Vec<i32>,
    /// Microseconds per quarter note.
    tempo: i32,
    /// `buffer.data`; `None` is the original's `null`.
    data: Option<Vec<u8>>,
    /// `buffer.offset`.
    offset: i32,
}

impl MidiDecoder {
    /// `new MidiDecoder()`: no song; [`MidiDecoder::is_valid`] is false.
    pub fn new() -> Self {
        Self::default()
    }

    /// `new MidiDecoder(byte[])`.
    pub fn with_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut decoder = Self::new();
        decoder.try_init(bytes)?;
        Ok(decoder)
    }

    /// `getTimeMillis(time)`: the clock value of a tick under the current
    /// tempo. Microseconds scaled by `division`, despite the name.
    pub fn get_time_millis(&self, time: i32) -> i64 {
        self.start_millis
            .wrapping_add(i64::from(time).wrapping_mul(i64::from(self.tempo)))
    }

    /// `hasNextTrack()`: true when every track has ended, despite the name.
    pub fn has_next_track(&self) -> bool {
        self.positions.iter().all(|&position| position < 0)
    }

    /// `setStartMillis(millis)`: rewinds every track for a loop. The tempo is
    /// left as the song last set it.
    pub fn set_start_millis(&mut self, millis: i64) {
        self.start_millis = millis;
        for track in 0..self.positions.len() {
            self.times[track] = 0;
            self.statuses[track] = 0;
            self.offset = self.start_positions[track];
            self.add_delta_time(track as i32);
            self.positions[track] = self.offset;
        }
    }

    /// `isValid()`: a song is loaded.
    pub fn is_valid(&self) -> bool {
        self.data.is_some()
    }

    /// `getTrackCount()`.
    pub fn get_track_count(&self) -> i32 {
        self.positions.len() as i32
    }

    /// `getNextEvent(track)`: reads the event at the cursor. Returns the
    /// channel message packed as `status | data1 << 8 | data2 << 16`, or 0 for
    /// a skipped system exclusive, 1 for end of track, 2 for a tempo change
    /// (applied here) and 3 for any other meta event.
    ///
    /// Where the original would throw (a read outside the bytes, no running
    /// status, a negative length) this returns 1 and moves the cursor to -1.
    pub fn get_next_event(&mut self, track: i32) -> i32 {
        match self.get_next_event_internal(track) {
            Some(event) => event,
            None => {
                self.offset = -1;
                1
            }
        }
    }

    /// `loadTrackPosition(track)`.
    pub fn load_track_position(&mut self, track: i32) {
        if let Some(&position) = self.positions.get(track as usize) {
            self.offset = position;
        }
    }

    /// `addDeltaTime(track)`: reads the delta time at the cursor into `times`.
    ///
    /// A read outside the bytes (the original throws) leaves the time
    /// unchanged and moves the cursor to -1, so saving it ends the track.
    pub fn add_delta_time(&mut self, track: i32) {
        match self.g_var_int() {
            Some(delta_time) => {
                if let Some(time) = self.times.get_mut(track as usize) {
                    *time = time.wrapping_add(delta_time);
                }
            }
            None => self.offset = -1,
        }
    }

    /// The private `getNextEvent(track, status)`.
    fn get_next_event_with_status(&mut self, track: i32, status: i32) -> Option<i32> {
        if status != 255 {
            let length = *STATUS_LENGTHS.get(usize::try_from(status - 128).ok()?)?;
            let mut event = status;
            if length >= 1 {
                event |= self.g1()? << 8;
            }
            if length >= 2 {
                event |= self.g1()? << 16;
            }
            return Some(event);
        }
        let kind = self.g1()?;
        let mut skip_bytes = self.g_var_int()?;
        if skip_bytes < 0 {
            return None;
        }
        if kind == 47 {
            self.offset = self.offset.wrapping_add(skip_bytes);
            Some(1)
        } else if kind == 81 {
            let tempo = self.g3()?;
            skip_bytes -= 3;
            let time = *self.times.get(track as usize)?;
            self.start_millis = self.start_millis.wrapping_add(
                i64::from(time).wrapping_mul(i64::from(self.tempo.wrapping_sub(tempo))),
            );
            self.tempo = tempo;
            self.offset = self.offset.wrapping_add(skip_bytes);
            Some(2)
        } else {
            self.offset = self.offset.wrapping_add(skip_bytes);
            Some(3)
        }
    }

    /// `getNextEventInternal(track)`.
    fn get_next_event_internal(&mut self, track: i32) -> Option<i32> {
        let status_byte = self.peek()?;
        let status = if status_byte >= 0x80 {
            *self.statuses.get_mut(track as usize)? = i32::from(status_byte);
            self.offset += 1;
            i32::from(status_byte)
        } else {
            *self.statuses.get(track as usize)?
        };
        if status != 240 && status != 247 {
            return self.get_next_event_with_status(track, status);
        }
        let length = self.g_var_int()?;
        if length < 0 {
            return None;
        }
        if status == 247 && length > 0 {
            let status2 = i32::from(self.peek()?);
            if (241..=243).contains(&status2)
                || status2 == 246
                || status2 == 248
                || (250..=252).contains(&status2)
                || status2 == 254
            {
                self.offset += 1;
                self.statuses[track as usize] = status2;
                return self.get_next_event_with_status(track, status2);
            }
        }
        self.offset = self.offset.wrapping_add(length);
        Some(0)
    }

    /// `init(bytes)`: loads standard MIDI bytes and places every track at its
    /// start with tempo 500000. The header chunk is not inspected beyond the
    /// track count and division at offsets 10 and 12.
    ///
    /// Malformed bytes (the original throws mid-way) leave the decoder
    /// released: [`MidiDecoder::is_valid`] is false and there are no tracks.
    /// [`MidiDecoder::try_init`] reports why.
    pub fn init(&mut self, bytes: &[u8]) {
        let _ = self.try_init(bytes);
    }

    /// [`MidiDecoder::init`], returning the reason when the bytes are malformed.
    pub fn try_init(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.release();
        let g2 = |offset: usize| -> Result<i32, String> {
            match bytes.get(offset..offset + 2) {
                Some(pair) => Ok(i32::from(pair[0]) << 8 | i32::from(pair[1])),
                None => Err(format!("midi: header ends at {}", bytes.len())),
            }
        };
        let tracks = g2(10)? as usize;
        let division = g2(12)?;
        let mut start_positions = vec![0i32; tracks];
        let mut offset = 14usize;
        let mut track = 0;
        while track < tracks {
            let header = bytes
                .get(offset..)
                .and_then(|rest| rest.get(..8))
                .ok_or_else(|| format!("midi: track {track} of {tracks} is missing"))?;
            let id = i32::from_be_bytes([header[0], header[1], header[2], header[3]]);
            let length = i32::from_be_bytes([header[4], header[5], header[6], header[7]]);
            offset += 8;
            if id == 0x4D54_726B {
                start_positions[track] = i32::try_from(offset)
                    .map_err(|_| "midi: track offset exceeds a Java int".to_string())?;
                track += 1;
            }
            // The original adds a negative length as is; it cannot be valid.
            let length = usize::try_from(length)
                .map_err(|_| format!("midi: negative chunk length {length}"))?;
            offset += length;
        }
        self.division = division;
        self.tempo = 500000;
        self.start_millis = 0;
        self.positions = start_positions.clone();
        self.start_positions = start_positions;
        self.times = vec![0; tracks];
        self.statuses = vec![0; tracks];
        self.data = Some(bytes.to_vec());
        self.offset = offset.min(i32::MAX as usize) as i32;
        Ok(())
    }

    /// `saveTrackPosition(track)`.
    pub fn save_track_position(&mut self, track: i32) {
        if let Some(position) = self.positions.get_mut(track as usize) {
            *position = self.offset;
        }
    }

    /// `getNextTrack()`: the live track with the earliest next event (the
    /// lowest index on a tie), or -1 when none is live.
    pub fn get_next_track(&self) -> i32 {
        let mut track = -1;
        let mut min_time = i32::MAX;
        for (index, &position) in self.positions.iter().enumerate() {
            if position >= 0 && self.times[index] < min_time {
                track = index as i32;
                min_time = self.times[index];
            }
        }
        track
    }

    /// `release()`: drops the song; the track tables become empty where the
    /// original's become `null`.
    pub fn release(&mut self) {
        self.data = None;
        self.start_positions = Vec::new();
        self.positions = Vec::new();
        self.times = Vec::new();
        self.statuses = Vec::new();
    }

    /// `loadEndOfTrackPosition()`: the cursor value that marks a track ended.
    pub fn load_end_of_track_position(&mut self) {
        self.offset = -1;
    }

    /// The private `positions[track]`. Not an original method.
    pub fn position(&self, track: i32) -> i32 {
        self.positions[track as usize]
    }

    /// The private `statuses[track]` (running status). Not an original method.
    pub fn status(&self, track: i32) -> i32 {
        self.statuses[track as usize]
    }

    /// The private `startMillis`. Not an original method.
    pub fn start_millis(&self) -> i64 {
        self.start_millis
    }

    /// The private `tempo`. Not an original method.
    pub fn tempo(&self) -> i32 {
        self.tempo
    }

    /// `buffer.data[buffer.offset]`, unsigned.
    fn peek(&self) -> Option<u8> {
        let offset = usize::try_from(self.offset).ok()?;
        self.data.as_ref()?.get(offset).copied()
    }

    fn g1(&mut self) -> Option<i32> {
        let value = self.peek()?;
        self.offset += 1;
        Some(i32::from(value))
    }

    fn g3(&mut self) -> Option<i32> {
        Some(self.g1()? << 16 | self.g1()? << 8 | self.g1()?)
    }

    /// `Buffer.gVarInt`.
    fn g_var_int(&mut self) -> Option<i32> {
        let mut b = self.g1()?;
        let mut value = 0i32;
        while b >= 0x80 {
            value = (b & 0x7F | value) << 7;
            b = self.g1()?;
        }
        Some(b | value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One track: tempo 500000, program 5, key 60 on (velocity 100) and off
    /// (velocity 64) 96 ticks later, end of track; division 96.
    const COMPACT: [u8; 21] = [
        23, 6, 0, 1, 7, // events
        0, 0, 0, 96, 0, // delta times
        60, 0,   // key deltas
        100, // on velocity
        64,  // off velocity
        5,   // program
        0x07, 0xA1, 0x20, // tempo
        1, 0, 96, // tracks, division
    ];

    const EXPANDED: [u8; 44] = [
        b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 0, 96, //
        b'M', b'T', b'r', b'k', 0, 0, 0, 22, //
        0, 0xFF, 0x51, 3, 0x07, 0xA1, 0x20, //
        0, 0xC0, 5, //
        0, 0x90, 60, 100, //
        96, 0x80, 60, 64, //
        0, 0xFF, 0x2F, 0,
    ];

    #[test]
    fn song_expands_to_standard_midi() {
        let song = Song::decode(&COMPACT).unwrap();
        assert_eq!(song.midi_bytes, EXPANDED);
        assert!(song.programs.is_none());
    }

    #[test]
    fn song_rejects_malformed_input() {
        assert!(Song::decode(&[]).is_err());
        assert!(Song::decode(&COMPACT[..COMPACT.len() - 4]).is_err());
        let mut unknown = COMPACT;
        unknown[1] = 8;
        assert!(Song::decode(&unknown).is_err());
        // No tracks is accepted, as in the original.
        assert_eq!(Song::decode(&[0, 0, 96]).unwrap().midi_bytes.len(), 14);
    }

    #[test]
    fn programs_record_sounded_keys() {
        let mut song = Song::decode(&COMPACT).unwrap();
        song.create_programs();
        let programs = song.programs.as_ref().unwrap();
        assert_eq!(programs.len(), 1);
        let program = programs.iter().next().unwrap();
        assert_eq!(program.id, 5);
        let mut keys = [0u8; 128];
        keys[60] = 1;
        assert_eq!(program.keys, keys);
        assert_eq!(programs.get(5), Some(&keys));
        assert_eq!(programs.get(21), None);
        assert_eq!(song.programs_in_order(), [(5, keys)]);
        song.release_programs();
        assert!(song.programs.is_none());
        assert!(song.programs_in_order().is_empty());

        let mut empty = Song::decode(&[0, 0, 96]).unwrap();
        empty.create_programs();
        assert!(empty.programs.unwrap().is_empty());
    }

    #[test]
    fn programs_iterate_in_hash_table_order() {
        let mut programs = Programs::new();
        for id in [17, 1, 128, 16, 33, 17, 15] {
            programs.get_or_put(id)[0] = 1;
        }
        let order: Vec<i32> = programs.iter().map(|program| program.id).collect();
        assert_eq!(order, [128, 16, 17, 1, 33, 15]);
        assert_eq!(programs.len(), 6);
    }

    #[test]
    fn decoder_reads_events_and_keeps_the_clock() {
        let mut decoder = MidiDecoder::new();
        assert!(!decoder.is_valid());
        decoder.init(&EXPANDED[..20]);
        assert!(!decoder.is_valid());
        assert_eq!(decoder.get_track_count(), 0);
        decoder.init(&EXPANDED);
        assert!(decoder.is_valid());
        assert_eq!(decoder.get_track_count(), 1);
        assert_eq!(decoder.division, 96);
        decoder.load_track_position(0);
        decoder.add_delta_time(0);
        decoder.save_track_position(0);
        let mut events = Vec::new();
        loop {
            let track = decoder.get_next_track();
            assert_eq!(track, 0);
            decoder.load_track_position(track);
            let event = decoder.get_next_event(track);
            events.push((decoder.times[0], event));
            if event == 1 {
                decoder.load_end_of_track_position();
                decoder.save_track_position(track);
                break;
            }
            decoder.add_delta_time(track);
            decoder.save_track_position(track);
        }
        assert_eq!(
            events,
            [
                (0, 2),
                (0, 0x05C0),
                (0, 0x64_3C90),
                (96, 0x40_3C80),
                (96, 1)
            ]
        );
        assert!(decoder.has_next_track());
        assert_eq!(decoder.get_next_track(), -1);
        assert_eq!(decoder.get_time_millis(96), 48_000_000);
        decoder.set_start_millis(48_000_000);
        assert!(!decoder.has_next_track());
        assert_eq!(decoder.times, [0]);
        assert_eq!(decoder.get_time_millis(2), 49_000_000);
        decoder.release();
        assert!(!decoder.is_valid());
    }

    #[test]
    fn tempo_change_keeps_elapsed_time() {
        // 10 ticks at the default tempo, then tempo 0x030000, then the end.
        let midi = [
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 0, 96, //
            b'M', b'T', b'r', b'k', 0, 0, 0, 11, //
            10, 0xFF, 0x51, 3, 3, 0, 0, //
            0, 0xFF, 0x2F, 0,
        ];
        let mut decoder = MidiDecoder::with_bytes(&midi).unwrap();
        decoder.load_track_position(0);
        decoder.add_delta_time(0);
        assert_eq!(decoder.times, [10]);
        assert_eq!(decoder.get_time_millis(10), 5_000_000);
        assert_eq!(decoder.get_next_event(0), 2);
        assert_eq!(decoder.tempo(), 0x03_0000);
        assert_eq!(decoder.get_time_millis(10), 5_000_000);
        assert_eq!(decoder.get_time_millis(11), 5_000_000 + 0x03_0000);
        decoder.add_delta_time(0);
        // Running status carries the 0xFF over to the end of track.
        assert_eq!(decoder.get_next_event(0), 1);
    }

    #[test]
    fn decoder_ends_malformed_tracks() {
        assert!(MidiDecoder::with_bytes(&EXPANDED[..13]).is_err());
        assert!(MidiDecoder::with_bytes(&EXPANDED[..20]).is_err());
        // The track is cut inside the note on.
        let mut decoder = MidiDecoder::with_bytes(&EXPANDED[..35]).unwrap();
        decoder.load_track_position(0);
        decoder.add_delta_time(0);
        assert_eq!(decoder.get_next_event(0), 2);
        decoder.add_delta_time(0);
        assert_eq!(decoder.get_next_event(0), 0x05C0);
        decoder.add_delta_time(0);
        assert_eq!(decoder.get_next_event(0), 1);
        decoder.add_delta_time(0);
        decoder.save_track_position(0);
        assert!(decoder.has_next_track());
        // A data byte with no running status.
        let mut midi = EXPANDED;
        midi[23] = 0x40;
        let mut decoder = MidiDecoder::with_bytes(&midi).unwrap();
        decoder.load_track_position(0);
        decoder.add_delta_time(0);
        assert_eq!(decoder.get_next_event(0), 1);
    }
}
