//! Play or render one song from the cache through the music synthesizer.
//!
//! `cargo run --release -p rs09-client --example play_song -- 76`
//! plays song 76 on the default audio device for 20 seconds. Options:
//! `--cache DIR` (default `assets/cache`), `--jingle` (the ID is a jingle),
//! `--seconds N`, `--volume 0..255`, and `--wav PATH` to write the audio to a
//! file instead of playing it (no device needed).
use rs09_client::music::{CacheSource, Music, Request};
use std::time::Duration;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let option = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|at| args.get(at + 1).cloned())
    };
    let number = |name: &str, default: i32| {
        option(name).map_or(Ok(default), |value| {
            value.parse::<i32>().map_err(|e| format!("{name}: {e}"))
        })
    };
    let id = args
        .iter()
        .enumerate()
        .find(|(at, arg)| {
            !arg.starts_with("--")
                && (*at == 0 || !args[at - 1].starts_with("--") || args[at - 1] == "--jingle")
        })
        .ok_or(
            "usage: play_song [--cache DIR] [--jingle] [--seconds N] [--volume N] [--wav PATH] ID",
        )?
        .1
        .parse::<i32>()
        .map_err(|e| format!("song id: {e}"))?;
    let seconds = number("--seconds", 20)?;
    let cache = option("--cache").unwrap_or_else(|| "assets/cache".into());
    let source = CacheSource::open(cache.as_ref())?;
    let mut music = Music::new(Box::new(source), number("--volume", 255)?);
    music.request(if args.iter().any(|arg| arg == "--jingle") {
        Request::Jingle(id)
    } else {
        Request::Song(id)
    });
    // 50 client cycles a second.
    let cycles = seconds * 50;
    if let Some(path) = option("--wav") {
        let device = music.device();
        let mut pcm = Vec::<i16>::new();
        let mut block = [0i32; 512];
        // 22050 frames a second over 50 cycles: 441 frames a cycle.
        let mut owed = 0;
        for _ in 0..cycles {
            music.cycle();
            owed += 441;
            while owed >= 256 {
                owed -= 256;
                device.lock().unwrap().read(&mut block);
                pcm.extend(block.iter().map(|&sample| rs09_client::pcm::clip(sample)));
            }
        }
        std::fs::write(&path, rs09_client::pcm::wav(&pcm)).map_err(|e| e.to_string())?;
        let loud = pcm.iter().filter(|&&sample| sample != 0).count();
        println!(
            "{} frames written to {path}; {loud} nonzero samples; {}",
            pcm.len() / 2,
            music.status()
        );
        return Ok(());
    }
    play(&mut music, cycles)
}

#[cfg(target_os = "windows")]
fn play(music: &mut Music, cycles: i32) -> Result<(), String> {
    let output = rs09_client::pcm::Output::new(false)?;
    output.music(music.device());
    for cycle in 0..cycles {
        music.cycle();
        if cycle % 50 == 0 {
            println!("{}", music.status());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    match output.error() {
        Some(error) => Err(error),
        None => {
            let capture = output.snapshot();
            println!(
                "{} blocks submitted to the device; {}",
                capture.buffers,
                music.status()
            );
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn play(_: &mut Music, _: i32) -> Result<(), String> {
    let _ = Duration::ZERO;
    Err("device playback is implemented for Windows; use --wav PATH".into())
}
