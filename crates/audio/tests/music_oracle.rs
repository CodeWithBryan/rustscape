//! The music port against `fixtures/music/oracle.jsonl`: observations of the
//! unchanged RT4 classes over the pinned cache (`cargo xtask music-oracle`).
//!
//! Covered: every song and jingle expanded to MIDI, every instrument, every
//! sound an instrument uses, scripted playback through the player, the
//! sequencer and the block reader, and the voice limiter's choice over
//! seeded sets of voices (real songs seldom reach its limit). The fixture holds
//! hashes, so the cache must be present; without it the test reports that it
//! was skipped. Device output and real-time pacing are outside this test.
use rs09_audio::{
    limiter::{self, Candidate},
    midi::Song,
    music::{
        Archive as SongArchive, Channel, Instrument, MidiPcmStream, Player, SoundBank, Source,
    },
    sound_stream::{Output, PcmSound, SoundPcmStream},
};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Arc};

struct Cache {
    songs: Archive,
    jingles: Archive,
    synth: Archive,
    vorbis: Archive,
    instruments: Archive,
}

/// `Js5.fetchFile(int)`: the ID is a file of the only group, or a group of
/// one file. Anything else throws in the original.
fn single(archive: &Archive, id: i32) -> Result<Option<Vec<u8>>, String> {
    let id = u32::try_from(id).map_err(|_| "negative file id")?;
    let (group, file) = if archive.index.capacity == 1 {
        (0, id)
    } else if archive.index.group(id).is_some_and(|g| g.capacity == 1) {
        (id, 0)
    } else {
        return Err(format!("{id} is not a single file"));
    };
    read(archive, group, file)
}
fn read(archive: &Archive, group: u32, file: u32) -> Result<Option<Vec<u8>>, String> {
    if archive.index.group(group).is_none() {
        return Ok(None);
    }
    archive
        .file(group, file, [0; 4])
        .map(|bytes| bytes.map(|bytes| bytes.to_vec()))
        .map_err(|e| e.to_string())
}

impl Source for Cache {
    fn song(&mut self, jingle: bool, group: i32) -> Result<Option<Vec<u8>>, String> {
        let archive = if jingle { &self.jingles } else { &self.songs };
        read(
            archive,
            u32::try_from(group).map_err(|_| "negative song")?,
            0,
        )
    }
    fn instrument(&mut self, id: i32) -> Result<Option<Vec<u8>>, String> {
        single(&self.instruments, id)
    }
    fn synth(&mut self, id: i32) -> Result<Option<Vec<u8>>, String> {
        single(&self.synth, id)
    }
    fn vorbis(&mut self, id: i32) -> Result<Option<Vec<u8>>, String> {
        single(&self.vorbis, id)
    }
    fn vorbis_setup(&mut self) -> Result<Option<Vec<u8>>, String> {
        read(&self.vorbis, 0, 0)
    }
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The byte layout `MusicFixtures.instrumentBytes` hashes.
fn instrument_bytes(instrument: &Instrument) -> Vec<u8> {
    let mut out = vec![];
    let signed = |values: &[i8]| values.iter().map(|&v| v as u8).collect::<Vec<_>>();
    out.extend(signed(&instrument.note_pan));
    out.extend(signed(&instrument.note_volume));
    out.extend(signed(&instrument.note_exclusive_group));
    for offset in instrument.note_pitch_offset.iter() {
        out.extend(offset.to_be_bytes());
    }
    for id in instrument.sound_lookup().unwrap() {
        out.extend(id.to_be_bytes());
    }
    out.extend(instrument.volume_multiplier.to_be_bytes());
    for state in &instrument.track_states {
        let Some(state) = state else {
            out.push(0);
            continue;
        };
        out.push(1);
        for value in [
            state.sustain_rate,
            state.vibrato_depth,
            state.attack_rate,
            state.vibrato_delay,
            state.release_rate,
            state.vibrato_speed,
            state.decay_rate,
        ] {
            out.extend(value.to_be_bytes());
        }
        for envelope in [&state.attack_envelope, &state.release_envelope] {
            match envelope {
                None => out.extend((-1i32).to_be_bytes()),
                Some(envelope) => {
                    out.extend((envelope.len() as i32).to_be_bytes());
                    out.extend(signed(envelope));
                }
            }
        }
    }
    out
}

/// One scripted playback scenario; returns each `run` step's observation in
/// the fixture's shape.
fn render(cache: &mut Cache, stereo: bool, script: &str) -> Vec<Value> {
    let output = Output {
        sample_rate: 22050,
        stereo,
    };
    let mut stream = MidiPcmStream::new(output);
    stream.init();
    let mut player = Player::new();
    let mut channel = Channel::default();
    let mut block = vec![0i32; if stereo { 512 } else { 256 }];
    let mut runs = vec![];
    let mut cycle = 0;
    let archive = |id: &str| {
        if id == "11" {
            SongArchive::Jingles
        } else {
            SongArchive::Songs
        }
    };
    for op in script.split(' ') {
        let p: Vec<&str> = op.split(':').collect();
        let int = |index: usize| p[index].parse::<i32>().unwrap();
        match p[0] {
            "play" => player.play_immediate(archive(p[1]), int(2), int(3)),
            "fade" => player.play_fade_out(int(2), archive(p[1]), int(3)),
            "fadeout" => player.fade_out(),
            "stop" => player.stop(&mut stream),
            "volume" => player.set_volume(&mut stream, int(1)),
            "run" => {
                let mut digest = Sha256::new();
                let mut started = vec![];
                for _ in 0..int(1) {
                    player.cycle(&mut stream);
                    if player.try_start_loaded_song(&mut stream, cache) {
                        started.push(cycle);
                    }
                    for _ in 0..2 {
                        channel.read(&mut stream, &mut block);
                        for sample in &block {
                            digest.update(sample.to_be_bytes());
                        }
                    }
                    cycle += 1;
                }
                runs.push(serde_json::json!({
                    "sha256": format!("{:x}", digest.finalize()),
                    "started": started,
                    "valid": stream.is_valid(),
                    "playing": player.is_playing(&stream),
                    "state": player.state(),
                    "volume": stream.volume(),
                }));
            }
            other => panic!("unknown script operation {other}"),
        }
    }
    runs
}

#[test]
fn music_matches_the_original_classes_over_the_pinned_cache() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = std::fs::read_to_string(root.join("fixtures/music/oracle.jsonl")).unwrap();
    let limits = Limits::default();
    let Ok(store) = Store::open(&root.join("assets/cache"), StorageMode::File, limits) else {
        eprintln!("skipped: the pinned cache is not present");
        return;
    };
    let open = |id| Archive::open(store.clone(), id, None, limits).unwrap();
    let mut cache = Cache {
        songs: open(6),
        jingles: open(11),
        synth: open(4),
        vorbis: open(14),
        instruments: open(15),
    };
    let mut bank = SoundBank::default();
    let mut counts = [0usize; 5];
    let mut limited = 0;
    let mut failures = vec![];
    for line in fixture.lines().filter(|line| !line.is_empty()) {
        let case: Value = serde_json::from_str(line).unwrap();
        let missing = case["missing"] == true;
        match case["case"].as_str().unwrap() {
            "song" => {
                counts[0] += 1;
                let (archive, group) = (case["archive"].as_i64().unwrap(), &case["group"]);
                let data = cache
                    .song(archive == 11, group.as_i64().unwrap() as i32)
                    .unwrap();
                let got = data.map(|data| Song::decode(&data).unwrap().midi_bytes);
                let same = match &got {
                    None => missing,
                    Some(bytes) => {
                        !missing
                            && case["length"] == bytes.len()
                            && case["sha256"] == sha(bytes).as_str()
                    }
                };
                if !same {
                    failures.push(format!("song {archive}/{group}"));
                }
            }
            "instrument" => {
                counts[1] += 1;
                let id = case["id"].as_i64().unwrap() as i32;
                // A group that is not a single file throws in the original.
                let got = cache
                    .instrument(id)
                    .ok()
                    .flatten()
                    .map(|data| instrument_bytes(&Instrument::decode(&data).unwrap()));
                let same = match &got {
                    None => missing,
                    Some(bytes) => !missing && case["sha256"] == sha(bytes).as_str(),
                };
                if !same {
                    failures.push(format!("instrument {id}"));
                }
            }
            "sound" => {
                counts[2] += 1;
                let id = case["id"].as_i64().unwrap() as i32;
                let got = if id & 1 == 0 {
                    bank.synth(id >> 2, None, &mut cache)
                } else {
                    bank.vorbis(id >> 2, None, &mut cache)
                }
                .ok()
                .flatten();
                let same = match &got {
                    None => missing,
                    Some(sound) => {
                        let bytes: Vec<u8> = sound.samples.iter().map(|&s| s as u8).collect();
                        !missing
                            && case["rate"] == sound.rate
                            && case["start"] == sound.start
                            && case["end"] == sound.end
                            && case["pingPong"] == sound.ping_pong_loop
                            && case["length"] == bytes.len()
                            && case["sha256"] == sha(&bytes).as_str()
                    }
                };
                if !same {
                    failures.push(format!("sound {id}"));
                }
            }
            "render" => {
                counts[3] += 1;
                let runs = render(
                    &mut cache,
                    case["stereo"].as_bool().unwrap(),
                    case["script"].as_str().unwrap(),
                );
                let expected = case["runs"].as_array().unwrap();
                if let Some(at) = (0..expected.len()).find(|&i| runs.get(i) != expected.get(i)) {
                    failures.push(format!(
                        "render {} run {at}: {} != {}",
                        case["name"], runs[at], expected[at]
                    ));
                }
            }
            "limiter" => {
                counts[4] += 1;
                let output = Output {
                    sample_rate: 22050,
                    stereo: true,
                };
                let mut sounds = std::collections::BTreeMap::new();
                let rows: Vec<Vec<i32>> = serde_json::from_value(case["voices"].clone()).unwrap();
                let candidates: Vec<Candidate> = rows
                    .iter()
                    .map(|row| {
                        let &[sound, length, start, end, volume, loops, skipped] = &row[..] else {
                            panic!("limiter voice row");
                        };
                        let sound = sounds
                            .entry(sound)
                            .or_insert_with(|| {
                                Arc::new(PcmSound::new(22050, vec![0; length as usize], start, end))
                            })
                            .clone();
                        let identity = Arc::as_ptr(&sound) as usize;
                        let mut stream = SoundPcmStream::create(sound, volume, output).unwrap();
                        stream.set_loops(loops);
                        stream.skip(skipped);
                        Candidate {
                            sound: identity,
                            volume: stream.get_effective_volume(),
                            channels: stream.get_active_channel_count(),
                        }
                    })
                    .collect();
                let parents = case["parents"].as_u64().unwrap() as u32;
                let active = limiter::admit(&candidates, parents);
                limited += active.iter().filter(|&&active| !active).count();
                if serde_json::json!(active) != case["active"] {
                    failures.push(format!("limiter case {}", counts[4] - 1));
                }
            }
            other => panic!("unknown fixture case {other}"),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        counts.iter().sum::<usize>(),
        failures
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    // The fixture's case counts, so an emptied fixture cannot pass.
    assert_eq!(counts, [1058, 266, 479, 83, 240]);
    // The limiter cases must include voices that were turned away.
    assert!(limited > 500, "{limited}");
}
