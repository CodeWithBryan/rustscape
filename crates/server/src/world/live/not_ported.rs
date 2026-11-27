//! Named answers for systems the live world reaches without a native port.
//!
//! Every answer is one of two kinds, and both are visible:
//!
//! - [`Kind::Skipped`]: the original would run content or a system that is
//!   not ported; the live world does nothing in its place (it sends nothing,
//!   changes nothing) and records the call. This is never reported as the
//!   original's behaviour: it is the gap.
//! - [`Kind::Default`]: the original's own answer when no content is
//!   registered (an empty list, `false` for a system with nothing in it),
//!   recorded the same way.
//!
//! The first call of each name is logged (with `--verbose`); every call is
//! counted. With the environment variable `RS09_NOT_PORTED_LOG=every` every
//! call is logged, with its running count (`[#n]`), for the refusal census
//! (docs/refusal-census.md); it changes nothing but the log. The full list,
//! with the reason for each, is in docs/live-world.md.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Skipped,
    Default,
}

#[derive(Default)]
pub struct NotPorted {
    calls: BTreeMap<&'static str, (Kind, u64)>,
    /// Print the first call of each name to standard error.
    pub verbose: bool,
    /// Print every call (`RS09_NOT_PORTED_LOG=every`), with its count.
    pub every: bool,
}

/// The environment variable that logs every call: `RS09_NOT_PORTED_LOG=every`.
pub const LOG_EVERY_VAR: &str = "RS09_NOT_PORTED_LOG";

impl NotPorted {
    pub fn new(verbose: bool) -> Self {
        Self {
            calls: BTreeMap::new(),
            verbose,
            every: std::env::var(LOG_EVERY_VAR).is_ok_and(|v| v.eq_ignore_ascii_case("every")),
        }
    }
    fn record(&mut self, kind: Kind, what: &'static str) {
        let entry = self.calls.entry(what).or_insert((kind, 0));
        entry.1 += 1;
        let label = match kind {
            Kind::Skipped => "not ported, skipped",
            Kind::Default => "not ported, original's empty answer",
        };
        if self.every {
            eprintln!("[live] {label}: {what} [#{}]", entry.1);
        } else if entry.1 == 1 && self.verbose {
            eprintln!("[live] {label}: {what}");
        }
    }
    /// A call whose system or content is not ported: nothing is done.
    pub fn skip(&mut self, what: &'static str) {
        self.record(Kind::Skipped, what);
    }
    /// A query answered with what the original answers with no content.
    pub fn default_answer(&mut self, what: &'static str) {
        self.record(Kind::Default, what);
    }
    /// Every name reached so far with its kind and count.
    pub fn calls(&self) -> Vec<(&'static str, Kind, u64)> {
        self.calls
            .iter()
            .map(|(name, (kind, count))| (*name, *kind, *count))
            .collect()
    }
    pub fn count(&self, what: &str) -> u64 {
        self.calls.get(what).map_or(0, |(_, n)| *n)
    }
}
