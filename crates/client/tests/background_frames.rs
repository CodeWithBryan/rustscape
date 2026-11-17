//! Frames that no display paces: a hidden browser page, whose frames
//! `web/visual/graphics_lifecycle.js` runs from a worker's 20 ms timer, or
//! from the page's own timer (once a second when hidden) where no worker can
//! be made.
//!
//! The original's `GameShell.run` is a thread of its own: it asks
//! `NanoTimer.count` for the cycles due (GameShell.java:634-636), and each
//! cycle's `Protocol.loop` counts `ticksSinceWrite` (Protocol.java:2920),
//! adds the keep-alive 93 when more than 50 cycles passed since the last
//! write (2975-2977) and writes, which zeroes the count (2982-2986). The
//! server drops a connection that is silent for 20 seconds.
//!
//! Transcription tests: the cycle counts come from `rs09_client::clock`
//! (`NanoTimer.count`, verified on its own); the keep-alive rule above is
//! restated here from the Java lines named. No original class runs.
use rs09_client::clock::{CYCLE_NANOS, Clock, MAX_CYCLES};

const MS: i64 = 1_000_000;

/// Frames every `period` ms for `seconds`: the longest time in ms between
/// two writes of a client that writes nothing but keep-alives, and the
/// cycles run.
fn longest_silence(period: i64, seconds: i64) -> (i64, u64) {
    let mut clock = Clock::new(0);
    let mut ticks_since_write = 0;
    let mut last_write = 0;
    let mut longest = 0;
    let mut cycles = 0;
    let mut now = 0;
    while now <= seconds * 1000 * MS {
        for _ in 0..clock.poll(now) {
            cycles += 1;
            // Protocol.java:2920, 2975-2977, 2982-2986.
            ticks_since_write += 1;
            if ticks_since_write > 50 {
                ticks_since_write = 0;
                longest = longest.max(now - last_write);
                last_write = now;
            }
        }
        now += period * MS;
    }
    (longest / MS, cycles)
}

#[test]
fn a_worker_timer_keeps_the_original_rate() {
    // 20 ms frames: one cycle each, 50 a second, a keep-alive every 51
    // cycles (1.02 s), as at a normal frame rate.
    let (silence, cycles) = longest_silence(20, 30);
    assert_eq!(cycles, 30 * 50 + 1);
    assert_eq!(silence, 51 * CYCLE_NANOS / MS);
}

#[test]
fn a_hidden_page_timer_still_writes_well_inside_the_server_time_out() {
    // One frame a second: NanoTimer.count grants at most ten cycles and
    // forgets the rest (NanoTimer.java:44-59), so the 51st silent cycle
    // comes in the sixth frame: a keep-alive every six seconds, not 20.
    let (silence, cycles) = longest_silence(1000, 60);
    assert_eq!(cycles, 61 * u64::from(MAX_CYCLES) - 9);
    assert_eq!(silence, 6000);
    assert!(silence < 20_000);
}

#[test]
fn no_frames_at_all_is_what_the_server_drops() {
    // The case the page timer removes: a hidden page without it runs no
    // frame, so nothing is written however long it stays hidden.
    let mut clock = Clock::new(0);
    assert_eq!(clock.poll(0), 1);
    // The first frame after 25 s of none runs ten cycles, not 1250.
    assert_eq!(clock.poll(25_000 * MS), MAX_CYCLES);
}
