//! The fixed 20 ms logic step of the original client.
//!
//! `GameShell.run` (GameShell.java:612-649) runs the game logic in whole
//! cycles of 20 ms (`FIXED_UPDATE_RATE`) and draws at its own rate. Once at
//! least 20 ms have passed since the previous update it asks
//! `NanoTimer.count` (NanoTimer.java:44-59) how many cycles are due and runs
//! `mainLoopWrapper` that many times in a row. `count` keeps the time at
//! which the next cycle is due and
//!
//! - always grants one cycle,
//! - grants further cycles while that time is still in the past,
//! - never grants more than ten,
//! - and, when ten were not enough to catch up, forgets the rest by moving
//!   the due time to now.
//!
//! So a stall of any length (a dragged window, a long load) costs at most ten
//! cycles of catching up, 200 ms of game time; the time beyond that is lost.
//! [`Clock::count`] is that method. [`Clock::poll`] is the form for a caller
//! that is itself called once per drawn frame.

/// `GameShell.FIXED_UPDATE_RATE`, 20 ms, in nanoseconds.
pub const CYCLE_NANOS: i64 = 20_000_000;

/// The most cycles `NanoTimer.count` grants in one call.
pub const MAX_CYCLES: u32 = 10;

/// `NanoTimer`: the time the next cycle is due, in nanoseconds on any
/// monotonic clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Clock {
    next_tick: i64,
    lost: i64,
}

impl Clock {
    /// `new NanoTimer()`: the first cycle is due at once.
    pub fn new(now: i64) -> Self {
        Self {
            next_tick: now,
            lost: 0,
        }
    }

    /// `NanoTimer.reset`, which `GameShell.resetTimer` calls after the
    /// loading stages and at the end of every scene rebuild: time spent
    /// there is not caught up.
    pub fn reset(&mut self, now: i64) {
        self.next_tick = now;
        self.lost = 0;
    }

    /// When the next cycle is due.
    pub fn next_tick(&self) -> i64 {
        self.next_tick
    }

    /// The time the last [`Clock::count`] or [`Clock::poll`] forgot because
    /// more than [`MAX_CYCLES`] cycles were due; 0 when it caught up. The
    /// original does not expose this. Non-zero means the caller was held up
    /// for more than 200 ms, the situation in which the original's blocking
    /// stages send their own keep-alives.
    pub fn lost(&self) -> i64 {
        self.lost
    }

    /// `NanoTimer.count`: the cycles to run now, 1 to [`MAX_CYCLES`]. The
    /// original's `minSleep` argument has no effect on the result and is
    /// left out.
    pub fn count(&mut self, now: i64) -> u32 {
        let mut ticks = 0;
        while ticks < MAX_CYCLES && (ticks < 1 || self.next_tick < now) {
            ticks += 1;
            self.next_tick = self.next_tick.wrapping_add(CYCLE_NANOS);
        }
        self.lost = 0;
        if now > self.next_tick {
            self.lost = now - self.next_tick;
            self.next_tick = now;
        }
        ticks
    }

    /// The cycles to run in a drawn frame that starts at `now`: none while
    /// the next cycle is not yet due, otherwise [`Clock::count`].
    ///
    /// `GameShell.run` sleeps until 20 ms have passed since its last update
    /// and only then calls `count`. A caller driven by the display has no
    /// such sleep, so the wait is expressed with the due time `count`
    /// already keeps. With updates 20 ms apart both forms grant one cycle
    /// per update; the cap and the forgotten backlog are `count`'s. The
    /// difference: where `count` leaves a cycle due at once (it had just
    /// caught up exactly, or had dropped a backlog), the original still
    /// waits 20 ms for its next update and this runs that cycle in the next
    /// frame. The rate is the same, 50 cycles a second.
    pub fn poll(&mut self, now: i64) -> u32 {
        if now < self.next_tick {
            self.lost = 0;
            return 0;
        }
        self.count(now)
    }
}

/// Seconds on a monotonic clock as the nanoseconds [`Clock`] counts in.
pub fn nanos(seconds: f64) -> i64 {
    (seconds * 1e9) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests: the expected values are traced by hand from
    // NanoTimer.java:44-59. They were not produced by running the class,
    // which reads `System.nanoTime()` itself.

    const MS: i64 = 1_000_000;

    #[test]
    fn count_always_grants_one_cycle_and_then_catches_up() {
        let mut clock = Clock::new(0);
        // Due exactly now: one cycle, the next one 20 ms later.
        assert_eq!(clock.count(0), 1);
        assert_eq!(clock.next_tick(), 20 * MS);
        // Called early (5 ms): `ticks < 1` still grants a cycle and pushes
        // the due time on.
        assert_eq!(clock.count(5 * MS), 1);
        assert_eq!(clock.next_tick(), 40 * MS);
        // 45 ms: the forced cycle moves the due time to 60 ms, which is not
        // in the past, so that is all.
        assert_eq!(clock.count(45 * MS), 1);
        assert_eq!(clock.next_tick(), 60 * MS);
        // 101 ms: 60 -> 80 -> 100 -> 120; 100 is still in the past.
        assert_eq!(clock.count(101 * MS), 3);
        assert_eq!(clock.next_tick(), 120 * MS);
        // The comparison is strict: a due time equal to now ends the loop.
        let mut clock = Clock::new(0);
        assert_eq!(clock.count(20 * MS), 1);
        assert_eq!(clock.next_tick(), 20 * MS);
        assert_eq!(clock.count(40 * MS), 1);
        assert_eq!(clock.next_tick(), 40 * MS);
        assert_eq!(clock.count(61 * MS), 2);
        assert_eq!(clock.next_tick(), 80 * MS);
    }

    #[test]
    fn a_stall_costs_ten_cycles_and_the_rest_is_forgotten() {
        let mut clock = Clock::new(0);
        assert_eq!(clock.count(0), 1);
        // Fifteen seconds without a call: ten cycles, then the due time
        // (220 ms) is still in the past and jumps to now.
        let now = 15_000 * MS;
        assert_eq!(clock.count(now), MAX_CYCLES);
        assert_eq!(clock.next_tick(), now);
        assert_eq!(clock.lost(), now - 220 * MS);
        // The next update 20 ms later is an ordinary one.
        assert_eq!(clock.count(now + 20 * MS), 1);
        assert_eq!(clock.next_tick(), now + 20 * MS);
        assert_eq!(clock.lost(), 0);
        // Exactly ten cycles late: all ten are granted and nothing is lost.
        let mut clock = Clock::new(0);
        assert_eq!(clock.count(199 * MS), MAX_CYCLES);
        assert_eq!(clock.next_tick(), 200 * MS);
        assert_eq!(clock.lost(), 0);
        // Any later and the remainder is dropped.
        let mut clock = Clock::new(0);
        assert_eq!(clock.count(201 * MS), MAX_CYCLES);
        assert_eq!(clock.next_tick(), 201 * MS);
        assert_eq!(clock.lost(), MS);
    }

    #[test]
    fn reset_drops_the_time_spent_loading() {
        let mut clock = Clock::new(0);
        clock.reset(5_000 * MS);
        assert_eq!(clock.count(5_000 * MS), 1);
        assert_eq!(clock.next_tick(), 5_020 * MS);
    }

    /// Total cycles `poll` grants for frames `step` nanoseconds apart over
    /// `frames` frames, and the most granted in one frame.
    fn run(step: i64, frames: i64) -> (u32, u32) {
        let mut clock = Clock::new(0);
        let (mut total, mut most) = (0, 0);
        for frame in 0..frames {
            let cycles = clock.poll(frame * step);
            total += cycles;
            most = most.max(cycles);
        }
        (total, most)
    }

    #[test]
    fn poll_keeps_fifty_cycles_a_second_at_any_frame_rate() {
        // One second of frames plus the frame at t = 1 s: the cycles due at
        // 0, 20, ... 1000 ms, or one fewer when the last frame falls just
        // short of the second.
        // 50 Hz: one cycle every frame.
        assert_eq!(run(20 * MS, 51), (51, 1));
        // 60 Hz (16.666 ms): five cycles in six frames.
        assert_eq!(run(16_666_667, 61), (51, 1));
        // 144 Hz: most frames run no cycle.
        assert_eq!(run(6_944_444, 145), (50, 1));
        // 30 Hz: one or two cycles a frame.
        assert_eq!(run(33_333_333, 31), (50, 2));
        // 4 Hz: every frame is a stall and is capped; game time runs slow.
        assert_eq!(run(250 * MS, 5), (1 + 4 * MAX_CYCLES, MAX_CYCLES));
    }

    #[test]
    fn poll_waits_for_the_due_time_and_bounds_a_stall() {
        let mut clock = Clock::new(0);
        assert_eq!(clock.poll(0), 1);
        assert_eq!(clock.poll(19 * MS), 0);
        assert_eq!(clock.next_tick(), 20 * MS);
        assert_eq!(clock.poll(20 * MS), 1);
        // A 15 s frame: ten cycles, far below the 750 a read timeout takes.
        // `count` leaves the next cycle due at once, so the frame after the
        // stall runs one and the usual spacing follows from there.
        let now = 15_020 * MS;
        assert_eq!(clock.poll(now), MAX_CYCLES);
        assert_eq!(clock.next_tick(), now);
        assert_eq!(clock.lost(), 14_780 * MS);
        assert_eq!(clock.poll(now + MS), 1);
        assert_eq!(clock.lost(), 0);
        assert_eq!(clock.poll(now + 2 * MS), 0);
        assert_eq!(clock.poll(now + 20 * MS), 1);
        assert_eq!(clock.poll(now + 39 * MS), 0);
    }

    #[test]
    fn nanos_converts_seconds() {
        assert_eq!(nanos(0.02), CYCLE_NANOS);
        assert_eq!(nanos(1.5), 1_500_000_000);
    }
}
