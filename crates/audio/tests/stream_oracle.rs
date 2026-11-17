//! `sound_stream` against `fixtures/music/stream.txt`: seeded scenarios that
//! `oracle/music/StreamProbe.java` ran through the unchanged `SoundPcmStream`,
//! `PcmSound` and `PcmResampler` (`cargo xtask music-oracle`). Every step's
//! state and output buffer is compared; where the original throws, the port
//! must panic at the same step. Set `STREAM_SCENARIOS` to replay a larger
//! file from the probe.

use rs09_audio::sound_stream::{Output, PcmResampler, PcmSound, SoundPcmStream};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

const DEFAULT_SCENARIOS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/music/stream.txt"
);

#[derive(Default, Debug)]
pub struct Totals {
    pub lines: u64,
    pub resampler_cases: u64,
    pub resampler_observations: u64,
    pub sound_resample_cases: u64,
    pub scenarios: u64,
    pub step_observations: u64,
    pub read_steps: u64,
    pub exceptions: u64,
    pub null_creations: u64,
    pub mismatches: u64,
}

fn unhex(s: &str) -> Vec<i8> {
    if s == "-" {
        return Vec::new();
    }
    let b = s.as_bytes();
    (0..b.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap() as i8)
        .collect()
}

fn hex_u8(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".to_string();
    }
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        write!(s, "{x:02x}").unwrap();
    }
    s
}

fn bytes(b: &[i8]) -> Vec<u8> {
    b.iter().map(|&x| x as u8).collect()
}

fn big(b: &[i8]) -> String {
    let b = bytes(b);
    if b.len() <= 48 {
        hex_u8(&b)
    } else {
        format!("h:{}:{}", b.len(), hex_u8(&Sha256::digest(&b)))
    }
}

const EXTREMES: [i32; 8] = [
    i32::MIN,
    i32::MAX,
    -1,
    0,
    1,
    0x7fff0000,
    i32::MIN + 1,
    0x80008000u32 as i32,
];

fn fill(buf: &mut [i32], mode: i32, seed: i32) {
    let mut x = seed;
    for slot in buf.iter_mut() {
        x = x.wrapping_mul(1103515245).wrapping_add(12345);
        *slot = match mode {
            0 => 0,
            1 => x ^ ((x as u32) >> 13) as i32,
            2 => x >> 14,
            _ => EXTREMES[(((x as u32) >> 9) % EXTREMES.len() as u32) as usize],
        };
    }
}

fn buf_token(buf: &[i32]) -> String {
    if buf.len() <= 10 {
        let mut s = String::from("v");
        for v in buf {
            write!(s, ":{v}").unwrap();
        }
        return s;
    }
    let mut b = Vec::with_capacity(buf.len() * 4);
    for v in buf {
        b.extend_from_slice(&v.to_be_bytes());
    }
    format!("h:{}", hex_u8(&Sha256::digest(&b)))
}

fn state(s: &SoundPcmStream) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        s.is_linked() as i32,
        s.sample_position(),
        s.loop_count(),
        s.get_volume(),
        s.get_pan(),
        s.get_absolute_rate(),
        s.sample_rate(),
        s.target_volume(),
        s.target_pan(),
        s.current_volume(),
        s.left_volume(),
        s.right_volume(),
        s.volume_delta(),
        s.left_volume_delta(),
        s.right_volume_delta(),
        s.fade_remaining(),
        s.is_transitioning() as i32,
        s.is_out_of_bounds() as i32,
        s.get_effective_volume(),
        s.get_active_channel_count(),
    )
}

fn int(s: &str) -> i32 {
    s.parse().unwrap_or_else(|_| panic!("bad int {s:?}"))
}

fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f)).ok()
}

fn or_exc(v: Option<String>) -> String {
    v.unwrap_or_else(|| "EXC".to_string())
}

struct Reporter {
    totals: Totals,
    messages: Vec<String>,
}

impl Reporter {
    fn check(&mut self, line_no: u64, what: &str, expected: &str, actual: &str) -> bool {
        // Java records EXC:<exception class>; a Rust panic is plain EXC.
        if expected == actual || (actual == "EXC" && expected.starts_with("EXC:")) {
            return true;
        }
        self.totals.mismatches += 1;
        if self.messages.len() < 25 {
            self.messages.push(format!(
                "line {line_no}: {what}\n   java: {expected}\n   rust: {actual}"
            ));
        }
        false
    }
}

fn resampler_case(rep: &mut Reporter, line_no: u64, line: &str) {
    rep.totals.resampler_cases += 1;
    let (head, tail) = line.split_once(" > ").unwrap();
    let h: Vec<&str> = head.split(' ').collect();
    let (g, src, dest) = (int(h[1]), int(h[2]), int(h[3]));
    let input = unhex(h[4]);
    let (rate, pos) = (int(h[5]), int(h[6]));
    let exp: Vec<&str> = tail.split(' ').collect();
    let resampler = guarded(|| PcmResampler::with_audio_sample_rate(src, dest, g));
    rep.totals.resampler_observations += 1;
    let Some(resampler) = resampler else {
        rep.check(line_no, "resampler constructor", exp[0], "EXC");
        return;
    };
    if !rep.check(line_no, "resampler constructor", exp[0], "OK") {
        return;
    }
    if g == 22050 {
        // The public constructor is the same thing with the default global rate.
        assert_eq!(PcmResampler::new(src, dest), resampler);
    }
    let out = or_exc(guarded(|| big(&resampler.resample(input.clone()))));
    rep.check(line_no, &format!("resample {head}"), exp[1], &out);
    let out = or_exc(guarded(|| resampler.scale_rate(rate).to_string()));
    rep.check(line_no, &format!("scale_rate {head}"), exp[2], &out);
    let out = or_exc(guarded(|| resampler.scale_position(pos).to_string()));
    rep.check(line_no, &format!("scale_position {head}"), exp[3], &out);
    rep.totals.resampler_observations += 3;
}

fn sound_resample_case(rep: &mut Reporter, line_no: u64, line: &str) {
    rep.totals.sound_resample_cases += 1;
    let (head, tail) = line.split_once(" > ").unwrap();
    let h: Vec<&str> = head.split(' ').collect();
    let (g, src, dest) = (int(h[1]), int(h[2]), int(h[3]));
    let mut sound =
        PcmSound::with_ping_pong(int(h[5]), unhex(h[4]), int(h[6]), int(h[7]), h[8] == "1");
    let out = or_exc(guarded(|| {
        let resampler = PcmResampler::with_audio_sample_rate(src, dest, g);
        sound.resample(&resampler);
        format!(
            "{} {} {} {}",
            big(&sound.samples),
            sound.rate,
            sound.start,
            sound.end
        )
    }));
    rep.check(line_no, &format!("PcmSound.resample {head}"), tail, &out);
}

fn stream_scenario(rep: &mut Reporter, line_no: u64, line: &str) {
    rep.totals.scenarios += 1;
    let mut segments = line.split(" | ");
    let head: Vec<&str> = segments.next().unwrap().split(' ').collect();
    let samples = unhex(head[1]);
    let sound = Arc::new(PcmSound::with_ping_pong(
        int(head[2]),
        samples,
        int(head[3]),
        int(head[4]),
        head[5] == "1",
    ));
    let output = Output {
        sample_rate: int(head[6]),
        stereo: head[7] == "1",
    };
    let linked = head[8] == "1";
    let (a, b, c) = (int(head[10]), int(head[11]), int(head[12]));
    let created = guarded(|| {
        if head[9] == "0" {
            SoundPcmStream::create(Arc::clone(&sound), a, output)
        } else {
            SoundPcmStream::create_with_rate(Arc::clone(&sound), a, b, c, output)
        }
    });
    let context = head[2..13].join(" ");
    let mut stream = match created {
        None => {
            rep.totals.exceptions += 1;
            rep.check(line_no, &format!("create [{context}]"), head[13], "EXC");
            return;
        }
        Some(None) => {
            rep.totals.null_creations += 1;
            rep.check(line_no, &format!("create [{context}]"), head[13], "NULL");
            return;
        }
        Some(Some(stream)) => stream,
    };
    if !rep.check(line_no, &format!("create [{context}]"), head[13], "OK") {
        return;
    }
    if linked {
        stream.set_linked(true);
    }
    if !rep.check(
        line_no,
        &format!("initial state [{context}]"),
        head[14],
        &state(&stream),
    ) {
        return;
    }
    let channels = if output.stereo { 2 } else { 1 };
    let mut history = String::new();
    for (index, segment) in segments.enumerate() {
        rep.totals.step_observations += 1;
        let (op, expected) = segment.split_once(" > ").unwrap();
        let t: Vec<&str> = op.split(' ').collect();
        let mut buf: Option<Vec<i32>> = None;
        let ok = guarded(|| match t[0] {
            "R" => {
                let (offset, length, slack) =
                    (int(t[1]) as usize, int(t[2]) as usize, int(t[3]) as usize);
                let mut b = vec![0i32; (offset + length) * channels + slack];
                fill(&mut b, int(t[4]), int(t[5]));
                let b = buf.insert(b);
                stream.read(b, offset, length);
            }
            "K" => stream.skip(int(t[1])),
            "LP" => stream.set_loops(int(t[1])),
            "V" => stream.set_volume(int(t[1])),
            "RT" => stream.set_rate(int(t[1])),
            "FV" => stream.fade_to_volume(int(t[1]), int(t[2])),
            "FP" => stream.fade_to_volume_and_pan(int(t[1]), int(t[2]), int(t[3])),
            "FO" => stream.fade_out_and_release(int(t[1])),
            "SP" => stream.set_sample_position(int(t[1])),
            "RV" => stream.reverse_direction(),
            "LK" => stream.set_linked(true),
            other => panic!("unknown op {other}"),
        })
        .is_some();
        if t[0] == "R" {
            rep.totals.read_steps += 1;
        }
        let actual = if ok {
            let mut s = state(&stream);
            if let Some(b) = &buf {
                s.push(' ');
                s.push_str(&buf_token(b));
            }
            s
        } else {
            rep.totals.exceptions += 1;
            "EXC".to_string()
        };
        let what = format!("step {index} `{op}` after [{context}]{history}");
        if !rep.check(line_no, &what, expected, &actual) || !ok {
            return;
        }
        write!(history, " | {op}").unwrap();
    }
}

pub fn run() -> Totals {
    let path = std::env::var("STREAM_SCENARIOS").unwrap_or_else(|_| DEFAULT_SCENARIOS.to_string());
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let mut rep = Reporter {
        totals: Totals::default(),
        messages: Vec::new(),
    };
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    for line in BufReader::with_capacity(1 << 20, file).lines() {
        let line = line.unwrap();
        rep.totals.lines += 1;
        let line_no = rep.totals.lines;
        match line.as_bytes().first() {
            Some(b'P') => resampler_case(&mut rep, line_no, &line),
            Some(b'Q') => sound_resample_case(&mut rep, line_no, &line),
            Some(b'S') => stream_scenario(&mut rep, line_no, &line),
            _ => panic!("unknown line {line_no}"),
        }
    }
    std::panic::set_hook(hook);
    println!("{path}\n{:#?}", rep.totals);
    for message in &rep.messages {
        println!("MISMATCH {message}");
    }
    assert_eq!(rep.totals.mismatches, 0, "observations differ from the JVM");
    rep.totals
}

#[test]
fn sound_streams_match_the_original() {
    let totals = run();
    // The committed fixture's size, so an emptied file cannot pass.
    if std::env::var_os("STREAM_SCENARIOS").is_none() {
        assert_eq!(
            [
                totals.scenarios,
                totals.resampler_cases + totals.sound_resample_cases
            ],
            [4000, 600]
        );
        assert!(totals.read_steps > 5000 && totals.exceptions > 0);
    }
}
