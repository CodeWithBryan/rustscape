//! Long computations cut into bounded slices on a thread that may not block.
//!
//! The original builds its scene in one blocking call
//! (`LoginManager.rebuildMap`) and keeps its connection alive from inside it
//! (`ClientProt.ping(true)`), while its audio runs on a thread of its own. The
//! desktop client builds streamed scenes on a worker thread. A browser page
//! has one thread for the game, the drawing, the connection and the feed of
//! the audio worklet, so a scene build there holds every one of them up.
//!
//! A computation written as an `async fn` with [`yield_point`]s in its loops
//! runs either way, with the same result:
//!
//! - [`block_on`] runs it to the end at once: no yield point ever waits.
//!   The worker thread and every synchronous caller use this.
//! - [`Sliced::run_until`] runs it until the first yield point met after a
//!   deadline, and on from there at the next call. The page calls it once a
//!   frame with a few milliseconds, so the build is spread over frames and
//!   everything else the frame does goes on.
//!
//! Nothing but the deadline decides where a slice ends; the work done and its
//! order are the same, so the result is the same.
use std::{
    cell::Cell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

/// A clock in milliseconds.
pub type Clock = fn() -> f64;

thread_local! {
    /// The deadline of the slice running on this thread, and its clock.
    static DEADLINE: Cell<Option<(f64, Clock)>> = const { Cell::new(None) };
}

/// Sets the slice deadline for the duration of `run`, and puts back the one
/// in force before (a [`block_on`] inside a slice clears it).
fn with_deadline<T>(deadline: Option<(f64, Clock)>, run: impl FnOnce() -> T) -> T {
    let before = DEADLINE.with(|cell| cell.replace(deadline));
    let result = run();
    DEADLINE.with(|cell| cell.set(before));
    result
}

fn past_deadline() -> bool {
    DEADLINE
        .with(Cell::get)
        .is_some_and(|(deadline, clock)| clock() >= deadline)
}

/// A place where a slice may end: ready at once, unless a slice is running
/// on this thread and its deadline has passed, in which case the computation
/// stops here once and goes on at the next poll.
pub fn yield_point() -> YieldPoint {
    YieldPoint { waited: false }
}

pub struct YieldPoint {
    waited: bool,
}

impl Future for YieldPoint {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.waited || !past_deadline() {
            return Poll::Ready(());
        }
        self.waited = true;
        context.waker().wake_by_ref();
        Poll::Pending
    }
}

/// Runs `future` to its end on this thread, with no slice deadline: the
/// synchronous form of a sliced computation.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    with_deadline(None, || {
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
        }
    })
}

/// Runs `future` to its end with a slice ending at every yield point (a
/// deadline that has always passed): the most a sliced run can be cut up.
/// Returns the output and the number of slices beyond the first. For tests
/// that a sliced computation gives what the whole one gives.
pub fn stopping_everywhere<F: Future>(future: F) -> (F::Output, u32) {
    fn passed() -> f64 {
        0.0
    }
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    let mut stops = 0;
    loop {
        match with_deadline(Some((0.0, passed)), || future.as_mut().poll(&mut context)) {
            Poll::Ready(output) => return (output, stops),
            Poll::Pending => stops += 1,
        }
    }
}

/// A computation carried out a slice at a time.
pub struct Sliced<T> {
    future: Pin<Box<dyn Future<Output = T>>>,
    /// Slices run so far, and the longest in milliseconds.
    pub slices: u32,
    pub longest_ms: f64,
    pub total_ms: f64,
}

impl<T> Sliced<T> {
    pub fn new(future: impl Future<Output = T> + 'static) -> Self {
        Self {
            future: Box::pin(future),
            slices: 0,
            longest_ms: 0.0,
            total_ms: 0.0,
        }
    }

    /// Runs on until the first yield point met at or after `deadline` (in
    /// `clock`'s milliseconds); the output once the computation has ended.
    pub fn run_until(&mut self, deadline: f64, clock: Clock) -> Option<T> {
        let start = clock();
        let mut context = Context::from_waker(Waker::noop());
        let poll = with_deadline(Some((deadline, clock)), || {
            self.future.as_mut().poll(&mut context)
        });
        let spent = clock() - start;
        self.slices += 1;
        self.longest_ms = self.longest_ms.max(spent);
        self.total_ms += spent;
        match poll {
            Poll::Ready(output) => Some(output),
            Poll::Pending => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    thread_local! {
        static NOW: Cell<f64> = const { Cell::new(0.0) };
    }
    fn now() -> f64 {
        NOW.with(Cell::get)
    }
    fn advance(ms: f64) {
        NOW.with(|now| now.set(now.get() + ms));
    }

    /// Ten steps of 3 ms each with a yield point after each step, recording
    /// the order; one inner synchronous run that must never stop part way.
    async fn work(log: std::rc::Rc<std::cell::RefCell<Vec<u32>>>) -> u32 {
        let mut sum = 0;
        for step in 0..10 {
            advance(3.0);
            sum += step * step;
            log.borrow_mut().push(step);
            yield_point().await;
        }
        sum += block_on(async {
            let mut inner = 0;
            for step in 0..5 {
                advance(3.0);
                inner += step;
                yield_point().await;
            }
            inner
        });
        sum
    }

    #[test]
    fn a_sliced_run_gives_what_a_blocking_run_gives() {
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let whole = block_on(work(log.clone()));
        assert_eq!(whole, 285 + 10);
        let blocking_order = std::mem::take(&mut *log.borrow_mut());

        let mut sliced = Sliced::new(work(log.clone()));
        let mut ends = Vec::new();
        let output = loop {
            let deadline = now() + 7.0;
            if let Some(output) = sliced.run_until(deadline, now) {
                break output;
            }
            ends.push(log.borrow().len());
            // Time passes between frames too.
            advance(16.0);
        };
        assert_eq!(output, whole);
        assert_eq!(*log.borrow(), blocking_order);
        // 7 ms of budget with 3 ms steps: the deadline is met after the
        // third step of each slice, which then ends at that step's yield.
        assert_eq!(ends, [3, 6, 9]);
        assert_eq!(sliced.slices, 4);
        // The last slice holds the tenth step and the whole inner run (no
        // yield inside a block_on): 3 + 15 ms.
        assert_eq!(sliced.longest_ms, 18.0);
        assert_eq!(sliced.total_ms, 45.0);
    }

    #[test]
    fn without_a_deadline_nothing_waits_and_deadlines_do_not_leak() {
        // A yield point outside any slice is ready at once.
        let mut context = Context::from_waker(Waker::noop());
        let mut point = std::pin::pin!(yield_point());
        assert_eq!(point.as_mut().poll(&mut context), Poll::Ready(()));
        // A slice whose deadline has already passed stops at the first
        // yield point and leaves no deadline behind it.
        let mut sliced = Sliced::new(async {
            yield_point().await;
            7
        });
        assert_eq!(sliced.run_until(now() - 1.0, now), None);
        assert!(DEADLINE.with(Cell::get).is_none());
        assert_eq!(sliced.run_until(now() - 1.0, now), Some(7));
    }
}
