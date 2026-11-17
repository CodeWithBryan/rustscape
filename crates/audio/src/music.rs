//! RT4 music synthesizer: `MidiInstrument`, `SoundBank`, `MidiNote`,
//! `MidiNoteStream`, the sub-stream half of `MixerPcmStream`, `MidiPcmStream`
//! and the `MidiPlayer`/`MusicPlayer` state machine.
//!
//! The original objects point at each other (notes at their instrument, the
//! note stream at its parent). Here the sequencer owns the notes and released
//! voices directly, and a note is named by an ID where the original compares
//! references. [`Channel`] is `AudioChannel`'s block reader with its voice
//! limiter; the device side of `AudioChannel` belongs to the host.
use crate::{
    audio::SoundEffect,
    limiter::{self, Candidate},
    midi::{MidiDecoder, Song},
    sound_stream::{Output, PcmSound, SoundPcmStream},
    vorbis,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

/// `GlobalConfig.AUDIO_SAMPLE_RATE`: the rate synthesized effects are made at,
/// and the per-call loading budget of `isSongReady`.
pub const RATE: i32 = 22050;

/// The cache files the music classes read.
pub trait Source {
    /// Songs archive (6) or jingles archive (11): file 0 of `group`.
    fn song(&mut self, jingle: bool, group: i32) -> Result<Option<Vec<u8>>, String>;
    /// Instruments archive (15), `Js5.fetchFile(id)`.
    fn instrument(&mut self, id: i32) -> Result<Option<Vec<u8>>, String>;
    /// Synthesized effects archive (4), addressed as `SoundBank` does.
    fn synth(&mut self, id: i32) -> Result<Option<Vec<u8>>, String>;
    /// Vorbis archive (14), addressed as `SoundBank` does.
    fn vorbis(&mut self, id: i32) -> Result<Option<Vec<u8>>, String>;
    /// Vorbis archive group 0 file 0: the shared codebooks.
    fn vorbis_setup(&mut self) -> Result<Option<Vec<u8>>, String>;
}

struct Cursor<'a> {
    data: &'a [u8],
    offset: usize,
}
impl Cursor<'_> {
    fn at(&self, index: usize) -> Result<i32, String> {
        self.data
            .get(index)
            .map(|&b| i32::from(b as i8))
            .ok_or_else(|| "Truncated instrument".to_owned())
    }
    /// The signed byte `ahead` of the read position.
    fn peek(&self, ahead: usize) -> Result<i32, String> {
        self.at(self.offset + ahead)
    }
    fn g1b(&mut self) -> Result<i32, String> {
        let value = self.at(self.offset)?;
        self.offset += 1;
        Ok(value)
    }
    fn g1(&mut self) -> Result<i32, String> {
        Ok(self.g1b()? & 255)
    }
    fn g_var_int(&mut self) -> Result<i32, String> {
        let mut b = self.g1b()?;
        let mut value = 0i32;
        while b < 0 {
            value = ((b & 0x7f) | value).wrapping_shl(7);
            b = self.g1b()?;
        }
        Ok(b | value)
    }
    /// A zero-terminated run of signed bytes; the terminator is not consumed.
    fn run(&mut self) -> Result<Vec<i32>, String> {
        let mut length = 0;
        while self.peek(length)? != 0 {
            length += 1;
        }
        (0..length).map(|_| self.g1b()).collect()
    }
}

/// `MidiTrackState`: the envelope and vibrato settings a group of keys shares.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrackState {
    pub sustain_rate: i32,
    pub attack_envelope: Option<Vec<i8>>,
    pub vibrato_depth: i32,
    pub attack_rate: i32,
    pub vibrato_delay: i32,
    pub release_rate: i32,
    pub release_envelope: Option<Vec<i8>>,
    pub vibrato_speed: i32,
    pub decay_rate: i32,
}

/// `MidiInstrument`.
#[derive(Clone, Debug)]
pub struct Instrument {
    pub note_pan: [i8; 128],
    /// Sound IDs still to resolve; `freeSoundTable` drops the table.
    sound_lookup: Option<[i32; 128]>,
    pub volume_multiplier: i32,
    pub note_exclusive_group: [i8; 128],
    pub note_volume: [i8; 128],
    pub sounds: Vec<Option<Arc<PcmSound>>>,
    pub note_pitch_offset: Arc<[i16; 128]>,
    pub track_states: Vec<Option<Arc<TrackState>>>,
}

fn floor_div(numerator: i32, denominator: i32) -> i32 {
    let sign = ((numerator as u32) >> 31) as i32;
    (numerator + sign) / denominator - sign
}

fn key_index(key: i32) -> Result<usize, String> {
    usize::try_from(key)
        .ok()
        .filter(|&key| key < 128)
        .ok_or_else(|| "Instrument key modifier out of range".to_owned())
}

impl Instrument {
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        let mut b = Cursor { data, offset: 0 };
        let next_run = |runs: &[i32], index: &mut usize| {
            if *index < runs.len() {
                *index += 1;
                runs[*index - 1]
            } else {
                -1
            }
        };

        let exclusive_runs = b.run()?;
        b.offset += 1;
        let mut exclusive_at = b.offset;
        b.offset += exclusive_runs.len() + 1;
        let pan_runs = b.run()?;
        b.offset += 1;
        let mut pan_at = b.offset;
        b.offset += pan_runs.len() + 1;
        let track_map_runs = b.run()?;
        b.offset += 1;
        let track_map_length = track_map_runs.len() + 1;
        let mut track_mapping = vec![0usize; track_map_length];
        let mut state_count = track_map_length;
        if track_map_length > 1 {
            state_count = 2;
            track_mapping[1] = 1;
            let mut group = 1i32;
            for mapping in track_mapping.iter_mut().skip(2) {
                let mut value = b.g1()?;
                if value == 0 {
                    group = state_count as i32;
                    state_count += 1;
                } else {
                    if value <= group {
                        value -= 1;
                    }
                    group = value;
                }
                // The original stores the group in a byte.
                *mapping = usize::try_from(i32::from(group as i8))
                    .map_err(|_| "Instrument track group out of range")?;
            }
        }
        let mut states = vec![TrackState::default(); state_count];
        for state in &mut states {
            let size = b.g1()? as usize;
            if size > 0 {
                state.attack_envelope = Some(vec![0; size * 2]);
            }
            let size = b.g1()? as usize;
            if size > 0 {
                let mut envelope = vec![0; size * 2 + 2];
                envelope[1] = 64;
                state.release_envelope = Some(envelope);
            }
        }
        let size = b.g1()? as usize;
        let mut volume_modifiers = (size > 0).then(|| vec![0i32; size * 2]);
        let size = b.g1()? as usize;
        let mut pan_modifiers = (size > 0).then(|| vec![0i32; size * 2]);
        let volume_runs = b.run()?;
        b.offset += 1;

        let mut pitch_offset = [0i16; 128];
        let mut accumulator = 0i32;
        for offset in &mut pitch_offset {
            accumulator += b.g1()?;
            *offset = accumulator as i16;
        }
        accumulator = 0;
        for offset in &mut pitch_offset {
            accumulator += b.g1()?;
            *offset = (i32::from(*offset) + (accumulator << 8)) as i16;
        }
        let mut sound_lookup = [0i32; 128];
        let (mut remaining, mut run_index, mut sound_id) = (0i32, 0usize, 0i32);
        for note in 0..128 {
            if remaining == 0 {
                remaining = next_run(&volume_runs, &mut run_index);
                sound_id = b.g_var_int()?;
            }
            pitch_offset[note] =
                (i32::from(pitch_offset[note]) + ((sound_id.wrapping_sub(1) & 2) << 14)) as i16;
            sound_lookup[note] = sound_id;
            remaining -= 1;
        }

        let mut note_exclusive_group = [0i8; 128];
        let (mut remaining, mut run_index, mut value) = (0i32, 0usize, 0i32);
        for note in 0..128 {
            if sound_lookup[note] != 0 {
                if remaining == 0 {
                    value = b.at(exclusive_at)? - 1;
                    exclusive_at += 1;
                    remaining = next_run(&exclusive_runs, &mut run_index);
                }
                remaining -= 1;
                note_exclusive_group[note] = value as i8;
            }
        }
        let mut note_pan = [0i8; 128];
        let (mut remaining, mut run_index, mut value) = (0i32, 0usize, 0i32);
        for note in 0..128 {
            if sound_lookup[note] != 0 {
                if remaining == 0 {
                    value = (b.at(pan_at)? + 16) << 2;
                    pan_at += 1;
                    remaining = next_run(&pan_runs, &mut run_index);
                }
                remaining -= 1;
                note_pan[note] = value as i8;
            }
        }
        let mut state_of = [None; 128];
        let (mut remaining, mut run_index, mut current) = (0i32, 0usize, None);
        for note in 0..128 {
            if sound_lookup[note] != 0 {
                if remaining == 0 {
                    let index = *track_mapping
                        .get(run_index)
                        .ok_or("Instrument track mapping out of range")?;
                    if index >= states.len() {
                        return Err("Instrument track state out of range".into());
                    }
                    current = Some(index);
                    remaining = next_run(&track_map_runs, &mut run_index);
                }
                state_of[note] = current;
                remaining -= 1;
            }
        }
        let mut note_volume = [0i8; 128];
        let (mut remaining, mut run_index, mut value) = (0i32, 0usize, 0i32);
        for note in 0..128 {
            if remaining == 0 {
                remaining = next_run(&volume_runs, &mut run_index);
                if sound_lookup[note] > 0 {
                    value = b.g1()? + 1;
                }
            }
            remaining -= 1;
            note_volume[note] = value as i8;
        }
        let volume_multiplier = b.g1()? + 1;

        for state in &mut states {
            if let Some(envelope) = &mut state.attack_envelope {
                for index in (1..envelope.len()).step_by(2) {
                    envelope[index] = b.g1b()? as i8;
                }
            }
            if let Some(envelope) = &mut state.release_envelope {
                for index in (3..envelope.len() - 2).step_by(2) {
                    envelope[index] = b.g1b()? as i8;
                }
            }
        }
        for modifiers in [&mut volume_modifiers, &mut pan_modifiers]
            .into_iter()
            .flatten()
        {
            for index in (1..modifiers.len()).step_by(2) {
                modifiers[index] = b.g1b()?;
            }
        }
        for state in &mut states {
            if let Some(envelope) = &mut state.release_envelope {
                let mut accumulator = 0i32;
                for index in (2..envelope.len()).step_by(2) {
                    accumulator = accumulator + b.g1()? + 1;
                    envelope[index] = accumulator as i8;
                }
            }
        }
        for state in &mut states {
            if let Some(envelope) = &mut state.attack_envelope {
                let mut accumulator = 0i32;
                for index in (2..envelope.len()).step_by(2) {
                    accumulator = accumulator + b.g1()? + 1;
                    envelope[index] = accumulator as i8;
                }
            }
        }

        // Both modifier tables are (key, value) byte pairs with delta-coded keys.
        let keyed = |modifiers: &mut Vec<i32>, b: &mut Cursor<'_>| -> Result<(), String> {
            let mut accumulator = b.g1()?;
            modifiers[0] = i32::from(accumulator as i8);
            for index in (2..modifiers.len()).step_by(2) {
                accumulator = accumulator + b.g1()? + 1;
                modifiers[index] = i32::from(accumulator as i8);
            }
            Ok(())
        };
        if let Some(modifiers) = &mut volume_modifiers {
            keyed(modifiers, &mut b)?;
            let scale = |volume: &mut i8, value: i32| {
                *volume = ((value * i32::from(*volume) + 32) >> 6) as i8;
            };
            let (mut prev_note, mut prev_value) = (modifiers[0], modifiers[1]);
            for key in 0..prev_note {
                scale(&mut note_volume[key_index(key)?], prev_value);
            }
            for pair in modifiers[2..].chunks_exact(2) {
                let (next_note, next_value) = (pair[0], pair[1]);
                let mut interpolation =
                    (next_note - prev_note) * prev_value + (next_note - prev_note) / 2;
                for key in prev_note..next_note {
                    let scaled = floor_div(interpolation, next_note - prev_note);
                    interpolation += next_value - prev_value;
                    scale(&mut note_volume[key_index(key)?], scaled);
                }
                prev_value = next_value;
                prev_note = next_note;
            }
            for key in prev_note..128 {
                scale(&mut note_volume[key_index(key)?], prev_value);
            }
        }
        if let Some(modifiers) = &mut pan_modifiers {
            keyed(modifiers, &mut b)?;
            let shift = |pan: &mut i8, value: i32| {
                *pan = (value + (i32::from(*pan) & 255)).clamp(0, 128) as i8;
            };
            let (mut prev_note, mut prev_pan) = (modifiers[0], modifiers[1] << 1);
            for key in 0..prev_note {
                shift(&mut note_pan[key_index(key)?], prev_pan);
            }
            for pair in modifiers[2..].chunks_exact(2) {
                let (next_note, next_pan) = (pair[0], pair[1] << 1);
                let mut interpolation =
                    (next_note - prev_note) * prev_pan + (next_note - prev_note) / 2;
                for key in prev_note..next_note {
                    let scaled = floor_div(interpolation, next_note - prev_note);
                    interpolation += next_pan - prev_pan;
                    shift(&mut note_pan[key_index(key)?], scaled);
                }
                prev_note = next_note;
                prev_pan = next_pan;
            }
            for key in prev_note..128 {
                shift(&mut note_pan[key_index(key)?], prev_pan);
            }
        }

        for state in &mut states {
            state.decay_rate = b.g1()?;
        }
        for state in &mut states {
            if state.attack_envelope.is_some() {
                state.attack_rate = b.g1()?;
            }
            if state.release_envelope.is_some() {
                state.release_rate = b.g1()?;
            }
            if state.decay_rate > 0 {
                state.sustain_rate = b.g1()?;
            }
        }
        for state in &mut states {
            state.vibrato_speed = b.g1()?;
        }
        for state in &mut states {
            if state.vibrato_speed > 0 {
                state.vibrato_depth = b.g1()?;
            }
        }
        for state in &mut states {
            if state.vibrato_depth > 0 {
                state.vibrato_delay = b.g1()?;
            }
        }

        let states: Vec<_> = states.into_iter().map(Arc::new).collect();
        Ok(Self {
            note_pan,
            sound_lookup: Some(sound_lookup),
            volume_multiplier,
            note_exclusive_group,
            note_volume,
            sounds: vec![None; 128],
            note_pitch_offset: Arc::new(pitch_offset),
            track_states: state_of
                .iter()
                .map(|index| index.map(|index| states[index].clone()))
                .collect(),
        })
    }

    /// `freeSoundTable`.
    pub fn free_sound_table(&mut self) {
        self.sound_lookup = None;
    }

    /// `soundLookup`: for each key, one more than the sound it still has to
    /// resolve, or 0. None after `free_sound_table`.
    pub fn sound_lookup(&self) -> Option<&[i32; 128]> {
        self.sound_lookup.as_ref()
    }

    /// `resolveSounds`: load the sounds of the keys `filter` marks, within the
    /// remaining `budget` of sample bytes. False while any is still missing.
    pub fn resolve_sounds(
        &mut self,
        mut budget: Option<&mut i32>,
        bank: &mut SoundBank,
        source: &mut dyn Source,
        filter: Option<&[u8; 128]>,
    ) -> Result<bool, String> {
        let lookup = self
            .sound_lookup
            .as_mut()
            .ok_or("Instrument sound table was released")?;
        let mut last = 0;
        let mut sound = None;
        let mut all = true;
        for note in 0..128 {
            if filter.is_some_and(|filter| filter[note] == 0) {
                continue;
            }
            let id = lookup[note];
            if id == 0 {
                continue;
            }
            if last != id {
                last = id;
                let id = id - 1;
                sound = if id & 1 == 0 {
                    bank.synth(id >> 2, budget.as_deref_mut(), source)?
                } else {
                    bank.vorbis(id >> 2, budget.as_deref_mut(), source)?
                };
                if sound.is_none() {
                    all = false;
                }
            }
            if let Some(sound) = &sound {
                self.sounds[note] = Some(sound.clone());
                lookup[note] = 0;
            }
        }
        Ok(all)
    }
}

/// `SoundBank`: decoded sounds for one song load. With a budget (the
/// original's one-element `samplingRates` array), a sound is skipped and
/// asked for again on the next call once the budget is used up.
#[derive(Default)]
pub struct SoundBank {
    cache: HashMap<(bool, i32), Arc<PcmSound>>,
    vorbis: HashMap<i32, vorbis::VorbisSound>,
    setup: Option<vorbis::Setup>,
}
impl SoundBank {
    pub fn synth(
        &mut self,
        id: i32,
        budget: Option<&mut i32>,
        source: &mut dyn Source,
    ) -> Result<Option<Arc<PcmSound>>, String> {
        if let Some(sound) = self.cache.get(&(false, id)) {
            return Ok(Some(sound.clone()));
        }
        if budget.as_deref().is_some_and(|budget| *budget <= 0) {
            return Ok(None);
        }
        let Some(data) = source.synth(id)? else {
            return Ok(None);
        };
        let effect = SoundEffect::decode(&data).map_err(|e| format!("synth/{id}: {e}"))?;
        if let Some(budget) = budget {
            *budget = budget.wrapping_sub(effect.samples.len() as i32);
        }
        let sound = Arc::new(PcmSound {
            rate: RATE,
            start: effect.loop_start as i32,
            end: effect.loop_end as i32,
            samples: effect.samples,
            ping_pong_loop: false,
        });
        self.cache.insert((false, id), sound.clone());
        Ok(Some(sound))
    }

    pub fn vorbis(
        &mut self,
        id: i32,
        budget: Option<&mut i32>,
        source: &mut dyn Source,
    ) -> Result<Option<Arc<PcmSound>>, String> {
        if let Some(sound) = self.cache.get(&(true, id)) {
            return Ok(Some(sound.clone()));
        }
        if budget.as_deref().is_some_and(|budget| *budget <= 0) {
            return Ok(None);
        }
        if self.setup.is_none() {
            let Some(header) = source.vorbis_setup()? else {
                return Ok(None);
            };
            self.setup = Some(vorbis::Setup::decode(&header)?);
        }
        let decoding = match self.vorbis.entry(id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let Some(data) = source.vorbis(id)? else {
                    return Ok(None);
                };
                entry.insert(
                    vorbis::VorbisSound::decode(&data).map_err(|e| format!("vorbis/{id}: {e}"))?,
                )
            }
        };
        let setup = self.setup.as_ref().expect("loaded above");
        // A packet the original would throw on is an error here too.
        let Some(pcm) = decoding
            .try_to_pcm(setup, budget)
            .map_err(|e| format!("vorbis/{id}: {e}"))?
        else {
            return Ok(None);
        };
        self.vorbis.remove(&id);
        let sound = Arc::new(PcmSound {
            rate: pcm.rate,
            samples: pcm.samples,
            start: pcm.start,
            end: pcm.end,
            ping_pong_loop: pcm.ping_pong,
        });
        self.cache.insert((true, id), sound.clone());
        Ok(Some(sound))
    }
}

/// `MidiNote`.
struct Note {
    id: u64,
    attack_envelope_index: i32,
    pitch_modifier: i32,
    pan: i32,
    track_state: Option<Arc<TrackState>>,
    release_timer: i32,
    stream: Option<SoundPcmStream>,
    vibrato_phase: i32,
    sound: Option<Arc<PcmSound>>,
    base_pitch: i32,
    attack_envelope_pos: i32,
    retrigger_delay: i32,
    volume_gain: i32,
    channel: usize,
    tick_count: i32,
    portamento_pos: i32,
    exclusive_group: i32,
    release_envelope_index: i32,
    note_key: i32,
    pitch_delta: i32,
    sustain_timer: i32,
    /// `instrument.notePitchOffset`, read by the note's current key.
    pitch_offsets: Arc<[i16; 128]>,
    /// `PcmStream.active` of the note's voice, as the limiter last left it.
    active: bool,
}
impl Note {
    /// `MidiNote.release`.
    fn release(&mut self) {
        self.track_state = None;
        self.sound = None;
        self.stream = None;
    }
    fn loops_forever(&self) -> bool {
        self.pitch_offsets[(self.note_key & 127) as usize] < 0
    }
}

/// `MidiPcmStream` with its `MidiNoteStream`.
pub struct MidiPcmStream {
    output: Output,
    looping: bool,
    current_time_micros: i64,
    current_tick: i32,
    current_track: i32,
    elapsed_time_micros: i64,
    channel_pan: [i32; 16],
    default_program: [i32; 16],
    channel_modulation: [i32; 16],
    channel_pitch_bend: [i32; 16],
    /// `exclusiveGroupNotes`, by note ID; 0 is null.
    exclusive: Box<[[u64; 128]; 16]>,
    channel_program: [i32; 16],
    channel_rpn_nrpn: [i32; 16],
    channel_detune_ratio: [i32; 16],
    channel_volume: [i32; 16],
    channel_expression: [i32; 16],
    channel_flags: [i32; 16],
    channel_volume_scalar: [i32; 16],
    channel_detune_value: [i32; 16],
    channel_bank_select: [i32; 16],
    channel_sample_offset: [i32; 16],
    /// `activeNotes`, by note ID; 0 is null.
    active: Box<[[u64; 128]; 16]>,
    volume: i32,
    channel_pitch_bend_range: [i32; 16],
    channel_portamento_time: [i32; 16],
    decoder: MidiDecoder,
    /// `noteStream.notes`, head first.
    notes: Vec<Note>,
    /// `noteStream.mixer`: voices fading out after their note let go of them,
    /// each with its `PcmStream.active` flag. The limiter no longer reaches a
    /// voice here, so the flag stays as it was when the note released it.
    mixer: Vec<(SoundPcmStream, bool)>,
    /// `(release timer, key)` of notes that left the list while an exclusive
    /// group slot still named them. The original keeps reading such a note.
    detached: HashMap<u64, (i32, i32)>,
    next_note: u64,
    instruments: BTreeMap<i32, Instrument>,
}

const TEMPO_SCALE: i32 = 1_000_000;

impl MidiPcmStream {
    pub fn new(output: Output) -> Self {
        let mut stream = Self {
            output,
            looping: false,
            current_time_micros: 0,
            current_tick: 0,
            current_track: 0,
            elapsed_time_micros: 0,
            channel_pan: [0; 16],
            default_program: [0; 16],
            channel_modulation: [0; 16],
            channel_pitch_bend: [0; 16],
            exclusive: Box::new([[0; 128]; 16]),
            channel_program: [0; 16],
            channel_rpn_nrpn: [0; 16],
            channel_detune_ratio: [0; 16],
            channel_volume: [0; 16],
            channel_expression: [0; 16],
            channel_flags: [0; 16],
            channel_volume_scalar: [256; 16],
            channel_detune_value: [0; 16],
            channel_bank_select: [0; 16],
            channel_sample_offset: [0; 16],
            active: Box::new([[0; 128]; 16]),
            volume: 256,
            channel_pitch_bend_range: [0; 16],
            channel_portamento_time: [0; 16],
            decoder: MidiDecoder::new(),
            notes: vec![],
            mixer: vec![],
            detached: HashMap::new(),
            next_note: 1,
            instruments: BTreeMap::new(),
        };
        stream.stop_all_notes(true);
        stream
    }

    /// `isSongReady`: load the song's instruments and sounds, up to one
    /// budget's worth per call. True once everything is present.
    pub fn is_song_ready(
        &mut self,
        song: &mut Song,
        bank: &mut SoundBank,
        source: &mut dyn Source,
    ) -> Result<bool, String> {
        song.create_programs();
        let mut ready = true;
        let mut budget = RATE;
        for (program, keys) in song.programs_in_order() {
            let instrument = match self.instruments.entry(program) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let Some(data) = source.instrument(program)? else {
                        ready = false;
                        continue;
                    };
                    entry.insert(
                        Instrument::decode(&data)
                            .map_err(|e| format!("instrument/{program}: {e}"))?,
                    )
                }
            };
            if !instrument.resolve_sounds(Some(&mut budget), bank, source, Some(&keys))? {
                ready = false;
            }
        }
        if ready {
            song.release_programs();
        }
        Ok(ready)
    }

    /// `releaseInstruments`.
    pub fn release_instruments(&mut self) {
        for instrument in self.instruments.values_mut() {
            instrument.free_sound_table();
        }
    }

    /// `clearInstruments`.
    pub fn clear_instruments(&mut self) {
        self.instruments.clear();
    }

    /// `init`: channel 10 plays the percussion bank.
    pub fn init(&mut self) {
        self.default_program[9] = 128;
        self.channel_bank_select[9] = 128;
        self.program_change(128, 9);
    }

    pub fn is_valid(&self) -> bool {
        self.decoder.is_valid()
    }
    pub fn volume(&self) -> i32 {
        self.volume
    }
    pub fn set_volume(&mut self, volume: i32) {
        self.volume = volume;
    }
    /// `reset`.
    pub fn reset(&mut self) {
        self.stop_playback(true);
    }
    /// `playSong`.
    pub fn play_song(&mut self, looping: bool, song: &Song) {
        self.load_song(looping, song, true);
    }
    /// Voices currently sounding: held notes and released ones fading out.
    pub fn voices(&self) -> usize {
        self.notes.iter().filter(|n| n.stream.is_some()).count() + self.mixer.len()
    }

    fn stop_playback(&mut self, reset: bool) {
        self.decoder.release();
        self.stop_all_notes(reset);
    }

    fn load_song(&mut self, looping: bool, song: &Song, reset: bool) {
        self.stop_playback(reset);
        self.decoder.init(&song.midi_bytes);
        self.looping = looping;
        self.elapsed_time_micros = 0;
        let tracks = self.decoder.get_track_count();
        for track in 0..tracks {
            self.decoder.load_track_position(track);
            self.decoder.add_delta_time(track);
            self.decoder.save_track_position(track);
        }
        self.current_track = self.decoder.get_next_track();
        if self.current_track < 0 {
            // A song without a live track; the original throws here and
            // `MidiPlayer` resets the stream.
            self.decoder.release();
            return;
        }
        self.current_tick = self.decoder.times[self.current_track as usize];
        self.current_time_micros = self.decoder.get_time_millis(self.current_tick);
    }

    fn reset_all_controllers(&mut self, channel: i32) {
        if channel < 0 {
            for channel in 0..16 {
                self.reset_all_controllers(channel);
            }
            return;
        }
        let c = channel as usize;
        self.channel_volume[c] = 12800;
        self.channel_pan[c] = 8192;
        self.channel_expression[c] = 16383;
        self.channel_pitch_bend[c] = 8192;
        self.channel_modulation[c] = 0;
        self.channel_portamento_time[c] = 8192;
        self.release_orphaned_portamento_notes(c);
        self.release_custom_hold_pedal(c);
        self.channel_flags[c] = 0;
        self.channel_rpn_nrpn[c] = 32767;
        self.channel_pitch_bend_range[c] = 256;
        self.channel_sample_offset[c] = 0;
        self.set_detune(c, 8192);
    }

    /// `Node.unlink` for the note at `index`.
    fn unlink(&mut self, index: usize) {
        let note = self.notes.remove(index);
        let named = usize::try_from(note.exclusive_group)
            .is_ok_and(|group| self.exclusive[note.channel][group & 127] == note.id);
        if named {
            self.detached
                .insert(note.id, (note.release_timer, note.note_key));
        }
    }

    /// The voice goes to the mixer when it is still fading, as the original
    /// does after `fadeOutAndRelease`.
    fn hand_to_mixer(&mut self, mut stream: SoundPcmStream, active: bool) {
        if stream.is_transitioning() {
            stream.set_linked(true);
            self.mixer.insert(0, (stream, active));
        }
    }

    fn all_sound_off(&mut self, channel: i32) {
        let mut index = 0;
        while index < self.notes.len() {
            if channel >= 0 && self.notes[index].channel as i32 != channel {
                index += 1;
                continue;
            }
            if let Some(mut stream) = self.notes[index].stream.take() {
                stream.fade_out_and_release(self.output.sample_rate / 100);
                self.hand_to_mixer(stream, self.notes[index].active);
                self.notes[index].release();
            }
            let note = &self.notes[index];
            if note.release_timer < 0 {
                self.active[note.channel][(note.note_key & 127) as usize] = 0;
            }
            self.unlink(index);
        }
    }

    fn set_detune(&mut self, channel: usize, value: i32) {
        self.channel_detune_value[channel] = value;
        self.channel_detune_ratio[channel] =
            (2f64.powf(f64::from(value) * 5.4931640625E-4) * 2097152.0 + 0.5) as i32;
    }

    fn program_change(&mut self, program: i32, channel: usize) {
        if self.channel_program[channel] != program {
            self.channel_program[channel] = program;
            self.exclusive[channel] = [0; 128];
        }
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        (id != 0)
            .then(|| self.notes.iter().position(|note| note.id == id))
            .flatten()
    }

    fn note_on(&mut self, velocity: i32, channel: usize, key: i32) {
        self.note_off(key, 64, channel);
        let k = (key & 127) as usize;
        if self.channel_flags[channel] & 2 != 0 {
            if let Some(index) = self
                .notes
                .iter()
                .rposition(|note| note.channel == channel && note.release_timer < 0)
            {
                let note = &mut self.notes[index];
                self.active[channel][(note.note_key & 127) as usize] = 0;
                self.active[channel][k] = note.id;
                let current_pitch =
                    note.base_pitch + (note.pitch_modifier.wrapping_mul(note.pitch_delta) >> 12);
                note.pitch_modifier = 4096;
                note.base_pitch += (key - note.note_key) << 8;
                note.pitch_delta = current_pitch - note.base_pitch;
                note.note_key = key;
                return;
            }
        }
        let Some(instrument) = self.instruments.get(&self.channel_program[channel]) else {
            return;
        };
        let Some(sound) = instrument.sounds[k].clone() else {
            return;
        };
        let pitch_offset = instrument.note_pitch_offset[k];
        let mut note = Note {
            id: self.next_note,
            sound: Some(sound.clone()),
            channel,
            track_state: instrument.track_states[k].clone(),
            exclusive_group: i32::from(instrument.note_exclusive_group[k]),
            note_key: key,
            volume_gain: (i32::from(instrument.note_volume[k])
                .wrapping_mul(velocity)
                .wrapping_mul(velocity)
                .wrapping_mul(instrument.volume_multiplier)
                .wrapping_add(1024))
                >> 11,
            pan: i32::from(instrument.note_pan[k]) & 255,
            base_pitch: (key << 8) - (i32::from(pitch_offset) & 0x7fff),
            release_timer: -1,
            attack_envelope_pos: 0,
            release_envelope_index: 0,
            sustain_timer: 0,
            attack_envelope_index: 0,
            pitch_modifier: 0,
            stream: None,
            vibrato_phase: 0,
            retrigger_delay: 0,
            tick_count: 0,
            portamento_pos: 0,
            pitch_delta: 0,
            pitch_offsets: instrument.note_pitch_offset.clone(),
            active: true,
        };
        self.next_note += 1;
        let (pitch, pan) = (self.calculate_pitch(&note), self.calculate_pan(&note));
        if self.channel_sample_offset[channel] == 0 {
            let volume = self.calculate_volume(&note);
            note.stream = SoundPcmStream::create_with_rate(sound, pitch, volume, pan, self.output);
        } else {
            note.stream = SoundPcmStream::create_with_rate(sound, pitch, 0, pan, self.output);
            self.apply_loop_start_offset(&mut note, pitch_offset < 0);
        }
        if pitch_offset < 0 {
            if let Some(stream) = &mut note.stream {
                stream.set_loops(-1);
            }
        }
        if let Ok(group) = usize::try_from(note.exclusive_group) {
            let previous = self.exclusive[channel][group];
            if previous != 0 {
                if let Some(index) = self.index_of(previous) {
                    let previous = &mut self.notes[index];
                    if previous.release_timer < 0 {
                        self.active[channel][(previous.note_key & 127) as usize] = 0;
                        previous.release_timer = 0;
                    }
                } else if let Some((timer, key)) = self.detached.get_mut(&previous) {
                    if *timer < 0 {
                        self.active[channel][(*key & 127) as usize] = 0;
                        *timer = 0;
                    }
                }
                self.detached.remove(&previous);
            }
            self.exclusive[channel][group] = note.id;
        }
        self.active[channel][k] = note.id;
        self.notes.push(note);
    }

    fn dispatch_midi_event(&mut self, event: i32) {
        let status = event & 0xf0;
        let channel = (event & 0xf) as usize;
        let data1 = event >> 8 & 0x7f;
        let data2 = event >> 16 & 0x7f;
        match status {
            128 => self.note_off(data1, data2, channel),
            144 => {
                if data2 > 0 {
                    self.note_on(data2, channel, data1);
                } else {
                    self.note_off(data1, 64, channel);
                }
            }
            // Polyphonic aftertouch and channel pressure do nothing.
            160 | 208 => {}
            176 => self.control_change(channel, data1, data2),
            192 => self.program_change(self.channel_bank_select[channel] + data1, channel),
            224 => {
                self.channel_pitch_bend[channel] = (event >> 9 & 0x3f80) + ((event & 0x7fbe) >> 8);
            }
            _ => {
                if event & 0xff == 255 {
                    self.stop_all_notes(true);
                }
            }
        }
    }

    fn control_change(&mut self, channel: usize, data1: i32, data2: i32) {
        // 14-bit controllers: the coarse byte replaces bits 7..13, the fine
        // byte bits 0..6.
        let coarse = |value: i32| (value & !0x3f80) + (data2 << 7);
        let fine = |value: i32| (value & !0x7f) + data2;
        match data1 {
            0 => {
                self.channel_bank_select[channel] =
                    (data2 << 14) + (self.channel_bank_select[channel] & 0xffe03fffu32 as i32);
            }
            32 => self.channel_bank_select[channel] = coarse(self.channel_bank_select[channel]),
            1 => self.channel_modulation[channel] = coarse(self.channel_modulation[channel]),
            33 => self.channel_modulation[channel] = fine(self.channel_modulation[channel]),
            5 => {
                self.channel_portamento_time[channel] =
                    coarse(self.channel_portamento_time[channel])
            }
            37 => {
                self.channel_portamento_time[channel] = fine(self.channel_portamento_time[channel])
            }
            7 => self.channel_volume[channel] = coarse(self.channel_volume[channel]),
            39 => self.channel_volume[channel] = fine(self.channel_volume[channel]),
            10 => self.channel_pan[channel] = coarse(self.channel_pan[channel]),
            42 => self.channel_pan[channel] = fine(self.channel_pan[channel]),
            11 => self.channel_expression[channel] = coarse(self.channel_expression[channel]),
            43 => self.channel_expression[channel] = fine(self.channel_expression[channel]),
            64 => {
                if data2 >= 64 {
                    self.channel_flags[channel] |= 1;
                } else {
                    self.channel_flags[channel] &= !1;
                }
            }
            65 => {
                if data2 < 64 {
                    self.release_orphaned_portamento_notes(channel);
                    self.channel_flags[channel] &= !2;
                } else {
                    self.channel_flags[channel] |= 2;
                }
            }
            99 => {
                self.channel_rpn_nrpn[channel] =
                    (data2 << 7) + (self.channel_rpn_nrpn[channel] & 0x7f);
            }
            98 => {
                self.channel_rpn_nrpn[channel] = (self.channel_rpn_nrpn[channel] & 0x3f80) + data2;
            }
            101 => {
                self.channel_rpn_nrpn[channel] =
                    (data2 << 7) + (self.channel_rpn_nrpn[channel] & 0x7f) + 16384;
            }
            100 => {
                self.channel_rpn_nrpn[channel] =
                    data2 + (self.channel_rpn_nrpn[channel] & 0x3f80) + 16384;
            }
            120 => self.all_sound_off(channel as i32),
            121 => self.reset_all_controllers(channel as i32),
            123 => self.all_notes_off(channel as i32),
            6 => {
                if self.channel_rpn_nrpn[channel] == 16384 {
                    self.channel_pitch_bend_range[channel] =
                        coarse(self.channel_pitch_bend_range[channel]);
                }
            }
            38 => {
                if self.channel_rpn_nrpn[channel] == 16384 {
                    self.channel_pitch_bend_range[channel] =
                        fine(self.channel_pitch_bend_range[channel]);
                }
            }
            16 => self.channel_sample_offset[channel] = coarse(self.channel_sample_offset[channel]),
            48 => self.channel_sample_offset[channel] = fine(self.channel_sample_offset[channel]),
            81 => {
                if data2 >= 64 {
                    self.channel_flags[channel] |= 4;
                } else {
                    self.release_custom_hold_pedal(channel);
                    self.channel_flags[channel] &= !4;
                }
            }
            17 => self.set_detune(channel, coarse(self.channel_detune_value[channel])),
            49 => self.set_detune(channel, fine(self.channel_detune_value[channel])),
            _ => {}
        }
    }

    fn all_notes_off(&mut self, channel: i32) {
        for note in &mut self.notes {
            if (channel < 0 || channel == note.channel as i32) && note.release_timer < 0 {
                self.active[note.channel][(note.note_key & 127) as usize] = 0;
                note.release_timer = 0;
            }
        }
    }

    /// `PcmStream.read`: mix `length` frames into `buffer` from frame `offset`.
    /// The buffer is interleaved when the output is stereo.
    pub fn read(&mut self, buffer: &mut [i32], mut offset: usize, mut length: i32) {
        if self.decoder.is_valid() {
            let tick_length =
                self.decoder.division.wrapping_mul(TEMPO_SCALE) / self.output.sample_rate;
            loop {
                let end = self.elapsed_time_micros + i64::from(length) * i64::from(tick_length);
                if self.current_time_micros - end >= 0 {
                    self.elapsed_time_micros = end;
                    break;
                }
                let advance = ((self.current_time_micros + i64::from(tick_length)
                    - self.elapsed_time_micros
                    - 1)
                    / i64::from(tick_length)) as i32;
                self.elapsed_time_micros += i64::from(tick_length) * i64::from(advance);
                self.notes_read(Some(&mut *buffer), offset, advance);
                length -= advance;
                offset += advance as usize;
                self.advance_sequencer();
                if !self.decoder.is_valid() {
                    break;
                }
            }
        }
        self.notes_read(Some(buffer), offset, length);
    }

    /// `PcmStream.skip`: advance `length` frames without producing sound.
    pub fn skip(&mut self, mut length: i32) {
        if self.decoder.is_valid() {
            let tick_length =
                self.decoder.division.wrapping_mul(TEMPO_SCALE) / self.output.sample_rate;
            loop {
                let end = self.elapsed_time_micros + i64::from(length) * i64::from(tick_length);
                if self.current_time_micros - end >= 0 {
                    self.elapsed_time_micros = end;
                    break;
                }
                let advance = ((i64::from(tick_length) + self.current_time_micros
                    - self.elapsed_time_micros
                    - 1)
                    / i64::from(tick_length)) as i32;
                length -= advance;
                self.elapsed_time_micros += i64::from(advance) * i64::from(tick_length);
                self.notes_read(None, 0, advance);
                self.advance_sequencer();
                if !self.decoder.is_valid() {
                    break;
                }
            }
        }
        self.notes_read(None, 0, length);
    }

    /// `MidiNoteStream.read` and `skip`; `buffer` is None when skipping.
    fn notes_read(&mut self, mut buffer: Option<&mut [i32]>, offset: usize, length: i32) {
        for (stream, active) in &mut self.mixer {
            // `PcmStream.readIfActive`.
            match buffer.as_deref_mut().filter(|_| *active) {
                Some(buffer) => stream.read(buffer, offset, length.max(0) as usize),
                None => stream.skip(length),
            }
        }
        self.mixer.retain(|(stream, _)| stream.is_linked());
        let mut index = 0;
        while index < self.notes.len() {
            let id = self.notes[index].id;
            if !self.is_note_finished(index) {
                let mut remaining = length;
                let mut position = offset;
                loop {
                    let delay = self.notes[index].retrigger_delay;
                    if remaining <= delay {
                        self.note_read(buffer.as_deref_mut(), index, position, remaining);
                        self.notes[index].retrigger_delay -= remaining;
                        break;
                    }
                    self.note_read(buffer.as_deref_mut(), index, position, delay);
                    remaining -= delay;
                    position += delay.max(0) as usize;
                    if self.advance_note_envelope(remaining, position, index, buffer.as_deref_mut())
                    {
                        break;
                    }
                }
            }
            // The note may have left the list while it was processed.
            if self.notes.get(index).is_some_and(|note| note.id == id) {
                index += 1;
            }
        }
    }

    /// A fresh voice for a note whose custom hold pedal retriggers it.
    fn retrigger(&mut self, index: usize, previous: &SoundPcmStream) {
        let note = &self.notes[index];
        let Some(sound) = note.sound.clone() else {
            return;
        };
        let (rate, volume, pan) = (
            previous.get_absolute_rate(),
            previous.get_volume(),
            previous.get_pan(),
        );
        let forever = note.loops_forever();
        // A new stream starts active.
        self.notes[index].active = true;
        let note = &self.notes[index];
        if self.channel_sample_offset[note.channel] == 0 {
            self.notes[index].stream =
                SoundPcmStream::create_with_rate(sound, rate, volume, pan, self.output);
        } else {
            let stream = SoundPcmStream::create_with_rate(sound, rate, 0, pan, self.output);
            let mut note = self.notes.remove(index);
            note.stream = stream;
            self.apply_loop_start_offset(&mut note, forever);
            self.notes.insert(index, note);
        }
        if forever {
            if let Some(stream) = &mut self.notes[index].stream {
                stream.set_loops(-1);
            }
        }
    }

    /// `MidiNoteStream.readNoteWithRetrigger` and the per-note `skip`.
    fn note_read(
        &mut self,
        mut buffer: Option<&mut [i32]>,
        index: usize,
        mut offset: usize,
        mut length: i32,
    ) {
        let end = offset + length.max(0) as usize;
        let channel = self.notes[index].channel;
        if self.channel_flags[channel] & 4 != 0 && self.notes[index].release_timer < 0 {
            let step = self.channel_detune_ratio[channel] / self.output.sample_rate;
            if step != 0 {
                match buffer.as_deref_mut() {
                    Some(buffer) => loop {
                        let until = (step + 1048575 - self.notes[index].portamento_pos) / step;
                        if length < until {
                            self.notes[index].portamento_pos += length * step;
                            break;
                        }
                        length -= until;
                        let Some(mut previous) = self.notes[index].stream.take() else {
                            return;
                        };
                        let was_active = self.notes[index].active;
                        previous.read(buffer, offset, until.max(0) as usize);
                        let fade = (self.output.sample_rate / 100).min(262144 / step);
                        self.notes[index].portamento_pos += step * until - 1048576;
                        self.retrigger(index, &previous);
                        if self.channel_sample_offset[channel] != 0 {
                            if let Some(stream) = &mut self.notes[index].stream {
                                stream.fade_to_volume(fade, previous.get_volume());
                            }
                        }
                        offset += until.max(0) as usize;
                        previous.fade_out_and_release(fade);
                        previous.read(buffer, offset, end.saturating_sub(offset));
                        self.hand_to_mixer(previous, was_active);
                    },
                    None => {
                        let until = (step + 1048575 - self.notes[index].portamento_pos) / step;
                        let note = &mut self.notes[index];
                        note.portamento_pos =
                            step.wrapping_mul(length).wrapping_add(note.portamento_pos) & 0xfffff;
                        if length >= until {
                            if let Some(previous) = self.notes[index].stream.take() {
                                self.retrigger(index, &previous);
                            }
                            length = self.notes[index].portamento_pos / step;
                        }
                    }
                }
            }
        }
        if let Some(stream) = &mut self.notes[index].stream {
            match buffer {
                Some(buffer) => stream.read(buffer, offset, length.max(0) as usize),
                None => stream.skip(length),
            }
        }
    }

    /// `advanceNoteEnvelope`: true when the note has let go of its voice.
    fn advance_note_envelope(
        &mut self,
        samples: i32,
        offset: usize,
        index: usize,
        buffer: Option<&mut [i32]>,
    ) -> bool {
        let rate = self.output.sample_rate;
        self.notes[index].retrigger_delay = rate / 100;
        let note = &self.notes[index];
        if note.release_timer >= 0
            && note
                .stream
                .as_ref()
                .is_none_or(SoundPcmStream::is_out_of_bounds)
        {
            self.finish(index);
            return true;
        }
        let channel = note.channel;
        let mut portamento = note.pitch_modifier;
        if portamento > 0 {
            portamento -= (2f64
                .powf(f64::from(self.channel_portamento_time[channel]) * 4.921259842519685E-4)
                * 16.0
                + 0.5) as i32;
            self.notes[index].pitch_modifier = portamento.max(0);
        }
        let pitch = self.calculate_pitch(&self.notes[index]);
        let held = self.channel_flags[channel] & 1 != 0;
        let named = usize::try_from(self.notes[index].exclusive_group)
            .is_ok_and(|group| self.exclusive[channel][group] == self.notes[index].id);
        let note = &mut self.notes[index];
        if let Some(stream) = &mut note.stream {
            stream.set_rate(pitch);
        }
        let Some(state) = note.track_state.clone() else {
            return true;
        };
        note.vibrato_phase = note.vibrato_phase.wrapping_add(state.vibrato_speed);
        note.tick_count = note.tick_count.wrapping_add(1);
        let pitch_factor = f64::from(
            ((note.note_key - 60) << 8)
                + (note.pitch_modifier.wrapping_mul(note.pitch_delta) >> 12),
        ) * 5.086263020833333E-6;
        let step = |rate: i32| {
            if rate > 0 {
                (2f64.powf(pitch_factor * f64::from(rate)) * 128.0 + 0.5) as i32
            } else {
                128
            }
        };
        let mut release = false;
        if state.decay_rate > 0 {
            note.sustain_timer = note.sustain_timer.wrapping_add(step(state.sustain_rate));
            if note.sustain_timer.wrapping_mul(state.decay_rate) >= 819200 {
                release = true;
            }
        }
        if let Some(envelope) = &state.attack_envelope {
            note.attack_envelope_pos = note
                .attack_envelope_pos
                .wrapping_add(step(state.attack_rate));
            let last = envelope.len() as i32 - 2;
            while note.attack_envelope_index < last
                && note.attack_envelope_pos
                    > (i32::from(envelope[note.attack_envelope_index as usize + 2]) & 255) << 8
            {
                note.attack_envelope_index += 2;
            }
            if last == note.attack_envelope_index
                && envelope[note.attack_envelope_index as usize + 1] == 0
            {
                release = true;
            }
        }
        if note.release_timer >= 0 && !held && !named {
            if let Some(envelope) = &state.release_envelope {
                note.release_timer = note.release_timer.wrapping_add(step(state.release_rate));
                let last = envelope.len() as i32 - 2;
                while last > note.release_envelope_index
                    && (i32::from(envelope[note.release_envelope_index as usize + 2]) & 255) << 8
                        < note.release_timer
                {
                    note.release_envelope_index += 2;
                }
                if note.release_envelope_index == last {
                    release = true;
                }
            }
        }
        let delay = note.retrigger_delay;
        if !release {
            let (volume, pan) = (
                self.calculate_volume(&self.notes[index]),
                self.calculate_pan(&self.notes[index]),
            );
            if let Some(stream) = &mut self.notes[index].stream {
                stream.fade_to_volume_and_pan(delay, volume, pan);
            }
            return false;
        }
        if let Some(mut stream) = self.notes[index].stream.take() {
            stream.fade_out_and_release(delay);
            match buffer {
                Some(buffer) => stream.read(buffer, offset, samples.max(0) as usize),
                None => stream.skip(samples),
            }
            self.hand_to_mixer(stream, self.notes[index].active);
        }
        self.notes[index].release();
        if self.notes[index].release_timer >= 0 {
            self.finish(index);
        }
        true
    }

    /// Release and unlink a note, freeing its exclusive group slot. As in the
    /// original, group 0 is not freed.
    fn finish(&mut self, index: usize) {
        let note = &mut self.notes[index];
        note.release();
        let (channel, group, id) = (note.channel, note.exclusive_group, note.id);
        if group > 0 && self.exclusive[channel][group as usize] == id {
            self.exclusive[channel][group as usize] = 0;
        }
        self.unlink(index);
    }

    fn note_off(&mut self, key: i32, _velocity: i32, channel: usize) {
        let k = (key & 127) as usize;
        let id = self.active[channel][k];
        let Some(index) = self.index_of(id) else {
            self.active[channel][k] = 0;
            return;
        };
        self.active[channel][k] = 0;
        if self.channel_flags[channel] & 2 == 0 {
            self.notes[index].release_timer = 0;
            return;
        }
        let note_channel = self.notes[index].channel;
        if self
            .notes
            .iter()
            .any(|other| other.channel == note_channel && other.release_timer < 0 && other.id != id)
        {
            self.notes[index].release_timer = 0;
        }
    }

    fn advance_sequencer(&mut self) {
        let mut track = self.current_track;
        let mut tick = self.current_tick;
        let mut time = self.current_time_micros;
        while self.current_tick == tick {
            while tick == self.decoder.times[track as usize] {
                self.decoder.load_track_position(track);
                let event = self.decoder.get_next_event(track);
                if event == 1 {
                    self.decoder.load_end_of_track_position();
                    self.decoder.save_track_position(track);
                    if self.decoder.has_next_track() {
                        if !self.looping || tick == 0 {
                            self.stop_all_notes(true);
                            self.decoder.release();
                            return;
                        }
                        self.decoder.set_start_millis(time);
                    }
                    break;
                }
                if event & 0x80 != 0 {
                    self.dispatch_midi_event(event);
                }
                self.decoder.add_delta_time(track);
                self.decoder.save_track_position(track);
            }
            track = self.decoder.get_next_track();
            if track < 0 {
                self.stop_all_notes(true);
                self.decoder.release();
                return;
            }
            tick = self.decoder.times[track as usize];
            time = self.decoder.get_time_millis(tick);
        }
        self.current_track = track;
        self.current_time_micros = time;
        self.current_tick = tick;
    }

    fn calculate_pan(&self, note: &Note) -> i32 {
        let pan = self.channel_pan[note.channel];
        if pan < 8192 {
            (note.pan * pan + 32) >> 6
        } else {
            16384 - (((128 - note.pan) * (-pan + 16384) + 32) >> 6)
        }
    }

    fn release_custom_hold_pedal(&mut self, channel: usize) {
        if self.channel_flags[channel] & 4 == 0 {
            return;
        }
        for note in &mut self.notes {
            if note.channel == channel {
                note.portamento_pos = 0;
            }
        }
    }

    fn calculate_pitch(&self, note: &Note) -> i32 {
        let (Some(state), Some(sound)) = (&note.track_state, &note.sound) else {
            return 1;
        };
        let c = note.channel;
        let mut pitch =
            (note.pitch_delta.wrapping_mul(note.pitch_modifier) >> 12) + note.base_pitch;
        pitch +=
            self.channel_pitch_bend_range[c].wrapping_mul(self.channel_pitch_bend[c] - 8192) >> 12;
        if state.vibrato_speed > 0 && (state.vibrato_depth > 0 || self.channel_modulation[c] > 0) {
            let mut depth = state.vibrato_depth << 2;
            let delay = state.vibrato_delay << 1;
            if delay > note.tick_count {
                depth = note.tick_count.wrapping_mul(depth) / delay;
            }
            depth += self.channel_modulation[c] >> 7;
            let sine = (f64::from(note.vibrato_phase & 0x1ff) * 0.01227184630308513).sin();
            pitch += (f64::from(depth) * sine) as i32;
        }
        let rate = (f64::from(sound.rate.wrapping_mul(256))
            * 2f64.powf(f64::from(pitch) * 3.255208333333333E-4)
            / f64::from(self.output.sample_rate)
            + 0.5) as i32;
        rate.max(1)
    }

    fn stop_all_notes(&mut self, force: bool) {
        if force {
            self.all_sound_off(-1);
        } else {
            self.all_notes_off(-1);
        }
        self.reset_all_controllers(-1);
        self.channel_program = self.default_program;
        for channel in 0..16 {
            self.channel_bank_select[channel] = self.default_program[channel] & !0x7f;
        }
    }

    fn apply_loop_start_offset(&self, note: &mut Note, ping_pong: bool) {
        let (Some(sound), Some(stream)) = (&note.sound, &mut note.stream) else {
            return;
        };
        let mut length = sound.samples.len() as i32;
        let shift = i64::from(self.channel_sample_offset[note.channel]);
        let offset = if ping_pong && sound.ping_pong_loop {
            let mirrored = length + length - sound.start;
            length <<= 8;
            let mut position = ((i64::from(mirrored) * shift) >> 6) as i32;
            if position >= length {
                stream.reverse_direction();
                position = length + length - position - 1;
            }
            position
        } else {
            ((i64::from(length) * shift) >> 6) as i32
        };
        stream.set_sample_position(offset);
    }

    fn release_orphaned_portamento_notes(&mut self, channel: usize) {
        if self.channel_flags[channel] & 2 == 0 {
            return;
        }
        for note in &mut self.notes {
            if note.channel == channel
                && self.active[channel][(note.note_key & 127) as usize] == 0
                && note.release_timer < 0
            {
                note.release_timer = 0;
            }
        }
    }

    /// `isNoteFinished`: a note without a voice; it leaves the list once its
    /// key was released.
    fn is_note_finished(&mut self, index: usize) -> bool {
        let note = &self.notes[index];
        if note.stream.is_some() {
            return false;
        }
        if note.release_timer >= 0 {
            self.finish(index);
        }
        true
    }

    fn calculate_volume(&self, note: &Note) -> i32 {
        let c = note.channel;
        if self.channel_volume_scalar[c] == 0 {
            return 0;
        }
        let Some(state) = &note.track_state else {
            return 0;
        };
        let mut volume =
            (self.channel_volume[c].wrapping_mul(self.channel_expression[c]) + 4096) >> 13;
        let squared = (volume.wrapping_mul(volume) + 16384) >> 15;
        let gain = (note.volume_gain.wrapping_mul(squared).wrapping_add(16384)) >> 15;
        let master = (gain.wrapping_mul(self.volume).wrapping_add(128)) >> 8;
        volume = (self.channel_volume_scalar[c]
            .wrapping_mul(master)
            .wrapping_add(128))
            >> 8;
        if state.decay_rate > 0 {
            volume = (0.5f64
                .powf(f64::from(note.sustain_timer) * 1.953125E-5 * f64::from(state.decay_rate))
                * f64::from(volume)
                + 0.5) as i32;
        }
        // Envelopes are (time, level) byte pairs; the level is interpolated
        // between the current pair and the next.
        let level = |envelope: &[i8], index: i32, position: i32| {
            let i = index as usize;
            let mut value = i32::from(envelope[i + 1]);
            if envelope.len() as i32 - 2 > index {
                let previous = (i32::from(envelope[i]) & 255) << 8;
                let next = (i32::from(envelope[i + 2]) & 255) << 8;
                value += (i32::from(envelope[i + 3]) - value) * (position - previous)
                    / (next - previous);
            }
            value
        };
        if let Some(envelope) = &state.attack_envelope {
            let value = level(
                envelope,
                note.attack_envelope_index,
                note.attack_envelope_pos,
            );
            volume = (value.wrapping_mul(volume) + 32) >> 6;
        }
        if note.release_timer > 0 {
            if let Some(envelope) = &state.release_envelope {
                let value = level(envelope, note.release_envelope_index, note.release_timer);
                volume = (volume.wrapping_mul(value) + 32) >> 6;
            }
        }
        volume
    }
}

impl MidiPcmStream {
    /// The priority pass of `AudioChannel.read` over the sequencer, its note
    /// stream and each note's voice. A voice already handed to the mixer is
    /// out of the limiter's reach and keeps the flag it had.
    fn limit_voices(&mut self) {
        let owners: Vec<usize> = (0..self.notes.len())
            .filter(|&index| self.notes[index].stream.is_some())
            .collect();
        let voices: Vec<Candidate> = owners
            .iter()
            .filter_map(|&index| {
                let note = &self.notes[index];
                let stream = note.stream.as_ref()?;
                Some(Candidate {
                    sound: note
                        .sound
                        .as_ref()
                        .map_or(0, |sound| Arc::as_ptr(sound) as usize),
                    volume: stream.get_effective_volume(),
                    channels: stream.get_active_channel_count(),
                })
            })
            .collect();
        for note in &mut self.notes {
            note.active = false;
        }
        for (index, active) in owners.into_iter().zip(limiter::admit(&voices, 2)) {
            self.notes[index].active = active;
        }
    }
}
/// `AudioChannel`'s block reader: 256 frames at a time, with the voice
/// limiter run every sixteenth of a second.
#[derive(Default)]
pub struct Channel {
    countdown: i32,
}
impl Channel {
    pub const FRAMES: usize = 256;

    /// `AudioChannel.read`: clear `block` (256 frames, interleaved when
    /// stereo) and mix the stream into it.
    pub fn read(&mut self, stream: &mut MidiPcmStream, block: &mut [i32]) {
        block.fill(0);
        self.countdown -= 256;
        if self.countdown <= 0 {
            self.countdown += stream.output.sample_rate >> 4;
            stream.limit_voices();
        }
        self.countdown = self.countdown.max(0);
        stream.read(block, 0, 256);
    }

    /// `AudioChannel.skip`: the device fell behind; advance without sound.
    pub fn skip(&mut self, stream: &mut MidiPcmStream) {
        self.countdown = (self.countdown - 256).max(0);
        stream.skip(256);
    }
}

/// Which archive a song comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Archive {
    Songs,
    Jingles,
}

/// `MidiPlayer` and `MusicPlayer`: song changes, fades and jingles. The caller
/// owns the stream (the audio device reads it) and passes it to each step.
#[derive(Default)]
pub struct Player {
    pub jingle: bool,
    bank: Option<SoundBank>,
    state: i32,
    volume: i32,
    song_group: i32,
    fade_rate: i32,
    song_loop: bool,
    song: Option<Song>,
    archive: Option<Archive>,
    /// `MusicPlayer.groupId`: the area's song.
    pub group_id: i32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            group_id: -1,
            ..Self::default()
        }
    }

    fn abandon(&mut self, stream: &mut MidiPcmStream) {
        stream.reset();
        self.archive = None;
        self.song = None;
        self.state = 0;
        self.bank = None;
    }

    /// `tryStartLoadedSong`: one loading step. True when the song starts.
    pub fn try_start_loaded_song(
        &mut self,
        stream: &mut MidiPcmStream,
        source: &mut dyn Source,
    ) -> bool {
        if self.state != 2 {
            return false;
        }
        match self.load_step(stream, source) {
            Ok(started) => started,
            Err(_) => {
                self.abandon(stream);
                false
            }
        }
    }

    fn load_step(
        &mut self,
        stream: &mut MidiPcmStream,
        source: &mut dyn Source,
    ) -> Result<bool, String> {
        if self.song.is_none() {
            let jingle = self.archive == Some(Archive::Jingles);
            let Some(data) = source.song(jingle, self.song_group)? else {
                return Ok(false);
            };
            self.song = Some(Song::decode(&data)?);
        }
        let bank = self.bank.get_or_insert_with(SoundBank::default);
        let song = self.song.as_mut().expect("loaded above");
        if !stream.is_song_ready(song, bank, source)? {
            return Ok(false);
        }
        stream.release_instruments();
        stream.set_volume(self.volume);
        stream.play_song(self.song_loop, song);
        self.state = 0;
        self.song = None;
        self.bank = None;
        self.archive = None;
        Ok(true)
    }

    /// `playFadeOut()`: fade the current song out and play nothing.
    pub fn fade_out(&mut self) {
        self.volume = 0;
        self.state = 1;
        self.fade_rate = 2;
        self.song_loop = false;
        self.archive = None;
        self.song_group = -1;
    }

    /// `playFadeOut(group, archive, volume)`: fade out, then start `group`.
    pub fn play_fade_out(&mut self, group: i32, archive: Archive, volume: i32) {
        self.archive = Some(archive);
        self.song_group = group;
        self.song_loop = false;
        self.state = 1;
        self.fade_rate = 2;
        self.volume = volume;
    }

    /// `playImmediate`: cut the current song and start `group`.
    pub fn play_immediate(&mut self, archive: Archive, group: i32, volume: i32) {
        self.archive = Some(archive);
        self.state = 1;
        self.volume = volume;
        self.song_group = group;
        self.song_loop = false;
        self.fade_rate = 10000;
    }

    /// `loop`: one client cycle of the fade.
    pub fn cycle(&mut self, stream: &mut MidiPcmStream) {
        if self.state != 1 {
            return;
        }
        let volume = stream.volume();
        if volume > 0 && stream.is_valid() {
            stream.set_volume((volume - self.fade_rate).max(0));
            return;
        }
        stream.reset();
        stream.clear_instruments();
        self.song = None;
        self.bank = None;
        self.state = if self.archive.is_none() { 0 } else { 2 };
    }

    pub fn is_playing(&self, stream: &MidiPcmStream) -> bool {
        self.state != 0 || stream.is_valid()
    }

    /// `MidiPlayer.state`: 0 idle or playing, 1 fading out, 2 loading.
    pub fn state(&self) -> i32 {
        self.state
    }

    /// `stop`.
    pub fn stop(&mut self, stream: &mut MidiPcmStream) {
        stream.reset();
        self.state = 1;
        self.archive = None;
    }

    /// `setVolume`.
    pub fn set_volume(&mut self, stream: &mut MidiPcmStream, volume: i32) {
        if self.state == 0 {
            stream.set_volume(volume);
        } else {
            self.volume = volume;
        }
    }

    /// `MusicPlayer.playSong`.
    pub fn play_song(&mut self, stream: &mut MidiPcmStream, song: i32, music_volume: i32) {
        if song == -1 && !self.jingle {
            self.stop(stream);
        } else if song != -1
            && (self.group_id != song || !self.is_playing(stream))
            && music_volume != 0
            && !self.jingle
        {
            self.play_fade_out(song, Archive::Songs, music_volume);
        }
        self.group_id = song;
    }

    /// `MusicPlayer.playJingle`.
    pub fn play_jingle(&mut self, jingle: i32, music_volume: i32) {
        if music_volume != 0 && jingle != -1 {
            self.play_immediate(Archive::Jingles, jingle, music_volume);
            self.jingle = true;
        }
    }

    /// The end of `SoundPlayer.loop`: after a jingle the area's song resumes.
    /// Returns true when the server should be told the track ended.
    pub fn after_sounds(&mut self, stream: &MidiPcmStream, music_volume: i32) -> bool {
        if self.jingle && !self.is_playing(stream) {
            if music_volume != 0 && self.group_id != -1 {
                self.play_immediate(Archive::Songs, self.group_id, music_volume);
            }
            self.jingle = false;
            false
        } else {
            music_volume != 0 && self.group_id != -1 && !self.is_playing(stream)
        }
    }
}
