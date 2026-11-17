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
//! The first call of each name is logged; every call is counted. The full
//! list, with the reason for each, is in docs/live-world.md.
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
}

impl NotPorted {
    pub fn new(verbose: bool) -> Self {
        Self {
            calls: BTreeMap::new(),
            verbose,
        }
    }
    fn record(&mut self, kind: Kind, what: &'static str) {
        let entry = self.calls.entry(what).or_insert((kind, 0));
        entry.1 += 1;
        if entry.1 == 1 && self.verbose {
            let label = match kind {
                Kind::Skipped => "not ported, skipped",
                Kind::Default => "not ported, original's empty answer",
            };
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
