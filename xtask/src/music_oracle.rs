//! `cargo xtask music-oracle [--check]`: the unchanged RT4 music classes run
//! over the pinned cache; their observations become `fixtures/music`.
use crate::{Result, cache_assets, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "AudioChannel",
    "ThreadUtils",
    "Song",
    "MidiDecoder",
    "MidiTrackState",
    "MidiInstrument",
    "MidiNote",
    "MidiNoteStream",
    "MidiPcmStream",
    "MidiPlayer",
    "MixerPcmStream",
    "MixerListener",
    "PcmStream",
    "PcmSound",
    "PcmResampler",
    "Sound",
    "SoundPcmStream",
    "SoundBank",
    "SynthSound",
    "SynthInstrument",
    "SynthEnvelope",
    "SynthFilter",
    "VorbisSound",
    "VorbisCodebook",
    "VorbisFloor",
    "VorbisResidue",
    "VorbisMapping",
    "Node",
    "LinkedList",
    "HashTable",
    "Buffer",
    "IntUtils",
    "ByteArrayNode",
    "ArrayUtils",
    "GlobalConfig",
    "Packet",
    "IsaacRandom",
    "RandomUtils",
    "Js5Compression",
    "GzipDecompressor",
    "Bzip2Decompressor",
    "Bzip2DState",
    "Js5Index",
    "IntHashTable",
    "Js5",
    "Js5ResourceProvider",
    "ByteArray",
    "DirectByteArray",
    "Cache",
    "BufferedFile",
];
const OTHER: &[&str] = &[
    "reference/client/signlink/src/main/java/rt4/MonotonicClock.java",
    "reference/client/signlink/src/main/java/rt4/FileOnDisk.java",
    "oracle/adapters/rt4/JagString.java",
    "oracle/music/adapters/rt4/SignLink.java",
    "oracle/music/adapters/rt4/AudioThread.java",
    "oracle/music/adapters/rt4/JavaAudioChannel.java",
    "oracle/music/adapters/rt4/SignLinkAudioChannel.java",
    "oracle/music/MusicOracle.java",
    "oracle/music/MusicFixtures.java",
    "oracle/music/StreamProbe.java",
];
/// `StreamProbe` arguments: seed, stream scenarios, resampler cases.
const STREAM: [&str; 3] = ["20261116", "4000", "600"];
/// Cache files the observations depend on.
const INPUTS: &[&str] = &[
    "main_file_cache.dat2",
    "main_file_cache.idx4",
    "main_file_cache.idx6",
    "main_file_cache.idx11",
    "main_file_cache.idx14",
    "main_file_cache.idx15",
    "main_file_cache.idx255",
];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    cache_assets::verify(root)?;
    let upstream = crate::verify_reference(root)?;
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("reference/client/client/src/main/java/rt4/{name}.java"))
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
        .collect();
    sources.sort();
    let mut hashes = BTreeMap::new();
    for source in &sources {
        hashes.insert(
            source.clone(),
            format!(
                "{:x}",
                Sha256::digest(normalized(&fs::read(root.join(source))?))
            ),
        );
    }
    let build = root.join("runs/music-oracle");
    let classes = build.join("classes");
    if classes.exists() {
        fs::remove_dir_all(&classes)?;
    }
    fs::create_dir_all(&classes)?;
    run(Command::new("javac")
        .current_dir(root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .args(&sources))?;
    let cache = cache_assets::directory(root);
    let output = run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(&classes)
        .arg("MusicFixtures")
        .arg(&cache))?;
    let fixture = normalized(&output.stdout);
    let stream_path = build.join("stream.txt");
    run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(&classes)
        .arg("StreamProbe")
        .args(STREAM)
        .arg(&stream_path))?;
    let stream = normalized(&fs::read(&stream_path)?);
    let mut counts = BTreeMap::<String, usize>::new();
    for line in fixture.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let value: Value = serde_json::from_slice(line)?;
        *counts
            .entry(value["case"].as_str().unwrap_or("?").to_owned())
            .or_default() += 1;
    }
    let mut inputs = BTreeMap::new();
    for name in INPUTS {
        let mut hasher = Sha256::new();
        std::io::copy(&mut fs::File::open(cache.join(name))?, &mut hasher)?;
        inputs.insert(*name, format!("{:x}", hasher.finalize()));
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "inputs": inputs,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "streamSha256": format!("{:x}", Sha256::digest(&stream)),
        "streamArguments": STREAM,
        "streamScope": "Seeded scenarios through the unchanged SoundPcmStream, PcmSound and PcmResampler: every public mutator interleaved with read and skip, mono and stereo, forward and backward, looped and ping-pong sounds, and inputs on which the original throws.",
        "scope": "Unchanged Song, MidiDecoder, MidiInstrument, SoundBank, SynthSound, VorbisSound, MidiPcmStream, MidiNoteStream, MixerPcmStream, SoundPcmStream, MidiPlayer and AudioChannel block reader over the pinned cache, and the AudioChannel voice limiter over seeded voices. SignLink, AudioThread and the two device channel classes are empty adapters; JagString is the byte-copy adapter. Playback is scripted in whole client cycles of two 256-frame blocks. No audio device, real-time pacing or hardware output is observed."
    });
    let directory = root.join("fixtures/music");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let stream_fixture = directory.join("stream.txt");
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&stream_fixture)?) != stream {
            return Err("stream probe output differs from the committed fixture".into());
        }
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "music oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("music oracle provenance is stale; review source or input changes".into());
        }
        println!("{total} JVM music observations reproduced with matching sources and inputs.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(&stream_fixture, &stream)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM music observations: {counts:?}");
    }
    Ok(())
}
