use rs09_client::{audio::SoundEffect, pcm};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[test]
fn original_real_location_effect_pcm_before_and_after_queue_trim_match() {
    check_synthesis(include_str!("../../../fixtures/audio/synthesis.json"), 90);
}
#[test]
fn original_real_npc_ambient_effect_pcm_before_and_after_queue_trim_match() {
    check_synthesis(
        include_str!("../../../fixtures/audio/npc-synthesis.json"),
        19,
    );
}
#[test]
fn original_real_spot_effect_pcm_before_and_after_queue_trim_match() {
    check_synthesis(
        include_str!("../../../fixtures/audio/spot-synthesis.json"),
        1,
    );
}
fn check_synthesis(fixture: &str, count: usize) {
    let f: Value = serde_json::from_str(fixture).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in f["sources"].as_object().unwrap() {
        let bytes = std::fs::read(root.join(format!(
            "reference/client/client/src/main/java/rt4/{name}.java"
        )))
        .unwrap()
        .into_iter()
        .filter(|&v| v != b'\r')
        .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            hash.as_str().unwrap()
        );
    }
    let hash = |s: &SoundEffect| {
        format!(
            "{:x}",
            Sha256::digest(s.samples.iter().map(|&v| v as u8).collect::<Vec<_>>())
        )
    };
    for c in f["cases"].as_array().unwrap() {
        let raw = c["raw"].as_str().unwrap();
        let bytes = (0..raw.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let original = SoundEffect::decode(&bytes).unwrap();
        let (trimmed, ticks) = SoundEffect::decode_for_queue(&bytes).unwrap();
        assert_eq!(
            json!([
                original.samples.len(),
                original.loop_start,
                original.loop_end,
                hash(&original),
                ticks,
                trimmed.samples.len(),
                trimmed.loop_start,
                trimmed.loop_end,
                hash(&trimmed)
            ]),
            c["expected"],
            "sound{}",
            c["id"]
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), count);
    assert!(f["missing"].as_array().unwrap().is_empty());
}

#[test]
fn original_synth_pcm_stream_loops_gain_and_output_clipping_match() {
    let f: Value = serde_json::from_str(include_str!("../../../fixtures/audio/pcm.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in f["sources"].as_object().unwrap() {
        let bytes = std::fs::read(root.join(format!(
            "reference/client/client/src/main/java/rt4/{name}.java"
        )))
        .unwrap()
        .into_iter()
        .filter(|&v| v != b'\r')
        .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            hash.as_str().unwrap()
        );
    }
    for (i, c) in f["streams"].as_array().unwrap().iter().enumerate() {
        let args: Vec<i32> = serde_json::from_value(c["args"].clone()).unwrap();
        let raw = c["pcm"].as_str().unwrap();
        let samples = (0..raw.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap() as i8)
            .collect();
        let mut voice = pcm::Voice::new(
            Arc::new(SoundEffect {
                samples,
                loop_start: args[0] as usize,
                loop_end: args[1] as usize,
            }),
            args[2],
            args[3],
        );
        for (j, s) in c["steps"].as_array().unwrap().iter().enumerate() {
            let frames = s["frames"].as_u64().unwrap() as usize;
            let offset = s["offset"].as_u64().unwrap() as usize;
            let stereo = args[4] != 0;
            let mut out = vec![
                s["fill"].as_i64().unwrap() as i32;
                (frames + offset + 3) * if stereo { 2 } else { 1 }
            ];
            if let Some(v) = &mut voice {
                v.volume = s["volume"].as_i64().unwrap() as i32;
                v.read(&mut out, offset, frames, stereo);
            }
            let hash = format!(
                "{:x}",
                Sha256::digest(
                    out.into_iter()
                        .flat_map(i32::to_be_bytes)
                        .collect::<Vec<_>>()
                )
            );
            let state = voice
                .as_ref()
                .map(|v| [v.position as i32 * 256, v.loops, i32::from(v.linked)]);
            assert_eq!(json!([hash, state]), s["expected"], "stream{i} step{j}");
        }
    }
    for c in f["clips"].as_array().unwrap() {
        let samples: Vec<i32> = serde_json::from_value(c["samples"].clone()).unwrap();
        let bytes = samples
            .into_iter()
            .flat_map(|v| pcm::clip(v).to_le_bytes())
            .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            c["expected"].as_str().unwrap()
        );
    }
    assert_eq!(f["streams"].as_array().unwrap().len() * 16, 3072);
    assert_eq!(f["clips"].as_array().unwrap().len(), 128);
}
