//! Definitions the client reads when the server names them.
//!
//! The original never meets a type it "does not have": `NpcTypeList.get`
//! (`NpcTypeList.java` 25-38), `SeqTypeList.get`, `LocTypeList.get` and the
//! other type lists read the file from the cache the first time an id is
//! asked for, and when the archive has no file for the id they keep a type
//! with every field at its default (`new NpcType()`, then `postDecode`). A
//! file whose group has not arrived yet is fetched (`Js5.fetchFile` asks the
//! update server for the group and returns null meanwhile); the client's
//! start-up sequence waits for every config archive before the title opens
//! (`client.mainLoad` state 70), so a type list never answers from a group
//! that is still on its way.
//!
//! This client may run from prepared packs, which hold only some of the
//! types. [`find`] is the one rule every on-demand lookup follows:
//!
//! 1. a definition already held (prepared pack, or read before) is used;
//! 2. with a cache open, the file is read from it: its decoded contents, the
//!    type's defaults when the archive has no such file (as the original),
//!    or "not yet" when its group is still being downloaded;
//! 3. without a cache, a prepared catalog that says it is complete stands
//!    for the archive: an id it lacks gets the defaults;
//! 4. otherwise the data is simply not in hand ([`Found::Unknown`]): the
//!    caller keeps the entity in its state, draws nothing for it and reports
//!    it once ([`Reported`]); it never ends the session over it.
use std::collections::BTreeSet;

/// What the cache answered for one definition file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read<T> {
    /// The file, decoded.
    File(T),
    /// The archive's index lists no such file: `Js5.fetchFile` returns null
    /// for it and the type keeps its defaults.
    NoFile,
    /// The index lists the group but the store does not have it yet.
    NotYet,
}

/// Where a definition asked for comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found<T> {
    /// Already held.
    Held,
    /// Read now from the cache.
    Read(T),
    /// The type's defaults: the cache, or a complete catalog, has no file.
    Defaults,
    /// The cache has the group listed but not yet in its store: ask again on
    /// a later cycle.
    NotYet,
    /// No cache is open and the prepared data lacks it.
    Unknown,
}

/// The lookup rule (see the module documentation). `cache` is called only
/// when the definition is not held; an error from it (an undecodable file,
/// an I/O failure) is passed on for the caller to report.
pub fn find<T>(
    held: bool,
    cache: Option<impl FnOnce() -> Result<Read<T>, String>>,
    catalog_complete: bool,
) -> Result<Found<T>, String> {
    if held {
        return Ok(Found::Held);
    }
    if let Some(read) = cache {
        return Ok(match read()? {
            Read::File(definition) => Found::Read(definition),
            Read::NoFile => Found::Defaults,
            Read::NotYet => Found::NotYet,
        });
    }
    Ok(if catalog_complete {
        Found::Defaults
    } else {
        Found::Unknown
    })
}

/// Turns a cache read into a [`Read`]: `Ok(None)` (the index has no such
/// file) is [`Read::NoFile`], a group the index lists but the store lacks
/// is [`Read::NotYet`], any other failure is an error.
pub fn classify<T>(
    read: rs09_cache::CacheResult<Option<T>>,
) -> Result<Read<T>, rs09_cache::CacheError> {
    match read {
        Ok(Some(file)) => Ok(Read::File(file)),
        Ok(None) => Ok(Read::NoFile),
        Err(rs09_cache::CacheError::Missing { .. }) => Ok(Read::NotYet),
        Err(error) => Err(error),
    }
}

/// A read that met a group a live store does not have yet (it has been
/// recorded and will be asked for, `Store::live_recording`) answers "absent"
/// instead of failing, so that a piece of work goes on and meets every such
/// group in one pass; the caller learns from
/// `rs09_cache::store::misses_on_this_thread` that it has to run again once
/// they are in. Any other failure, and a group missing from a finished
/// store, is passed on.
pub fn arriving<T>(
    read: impl FnOnce() -> rs09_cache::CacheResult<Option<T>>,
) -> rs09_cache::CacheResult<Option<T>> {
    let before = rs09_cache::store::misses_on_this_thread();
    match read() {
        Err(rs09_cache::CacheError::Missing { .. })
            if rs09_cache::store::misses_on_this_thread() != before =>
        {
            Ok(None)
        }
        other => other,
    }
}

/// What [`gather`] found for a type and the types its morph table names.
#[derive(Debug, PartialEq, Eq)]
pub struct Gathered<T> {
    /// Definitions to keep: read from the cache, or the defaults.
    pub add: Vec<(u32, T)>,
    /// Not in hand and no cache to read them from.
    pub unknown: Vec<u32>,
    /// Still being downloaded.
    pub not_yet: Vec<u32>,
    /// Unreadable files.
    pub failed: Vec<(u32, String)>,
}

/// The most types one walk visits; a morph table naming more is malformed.
pub const GATHER_LIMIT: usize = 128;

/// A type and, for a morph (`NpcType.multinpc`), every type it may become:
/// `Npc.getType` and `NpcType.getMultiNpc` ask `NpcTypeList.get` for the
/// chosen child when the NPC is drawn, so all of them are read up front.
/// `held` gives the child ids of a definition already held, `lookup` is
/// [`find`] for one id that is not held, `children` the child ids of a
/// definition just read (ids below zero name no type).
pub fn gather<T>(
    root: u32,
    held: impl Fn(u32) -> Option<Vec<i32>>,
    mut lookup: impl FnMut(u32) -> Result<Found<T>, String>,
    defaults: impl Fn() -> Result<T, String>,
    children: impl Fn(&T) -> Vec<i32>,
) -> Gathered<T> {
    let mut out = Gathered {
        add: Vec::new(),
        unknown: Vec::new(),
        not_yet: Vec::new(),
        failed: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if seen.len() >= GATHER_LIMIT || !seen.insert(id) {
            continue;
        }
        let next = if let Some(next) = held(id) {
            next
        } else {
            let found = lookup(id).and_then(|found| match found {
                Found::Defaults => defaults().map(Found::Read),
                other => Ok(other),
            });
            match found {
                Ok(Found::Read(definition)) => {
                    let next = children(&definition);
                    out.add.push((id, definition));
                    next
                }
                Ok(Found::Held | Found::Defaults) => Vec::new(),
                Ok(Found::NotYet) => {
                    out.not_yet.push(id);
                    Vec::new()
                }
                Ok(Found::Unknown) => {
                    out.unknown.push(id);
                    Vec::new()
                }
                Err(error) => {
                    out.failed.push((id, error));
                    Vec::new()
                }
            }
        };
        pending.extend(
            next.into_iter()
                .rev()
                .filter_map(|child| u32::try_from(child).ok()),
        );
    }
    out
}

/// Diagnostics said once per kind and id, so an entity whose data is not in
/// hand is reported when first met and not on every update after.
#[derive(Default, Debug)]
pub struct Reported(BTreeSet<(&'static str, i64)>);
impl Reported {
    /// True the first time `kind`/`id` is seen.
    pub fn first(&mut self, kind: &'static str, id: i64) -> bool {
        self.0.insert((kind, id))
    }
    /// Whether `kind`/`id` has been reported.
    pub fn contains(&self, kind: &'static str, id: i64) -> bool {
        self.0.contains(&(kind, id))
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests of `NpcTypeList.get` (`NpcTypeList.java` 25-38)
    //! and `Js5.fetchFile` (`Js5.java` 183-205) against a cache double.
    use super::*;
    use std::cell::Cell;

    /// A cache double: answers one way and counts how often it was asked.
    fn cache(answer: Read<u32>, calls: &Cell<u32>) -> impl FnOnce() -> Result<Read<u32>, String> {
        move || {
            calls.set(calls.get() + 1);
            Ok(answer)
        }
    }

    #[test]
    fn a_held_definition_never_reaches_the_cache() {
        let calls = Cell::new(0);
        let found = find(true, Some(cache(Read::File(7), &calls)), false).unwrap();
        assert_eq!(found, Found::Held);
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn the_cache_answers_a_file_its_absence_or_not_yet() {
        let calls = Cell::new(0);
        assert_eq!(
            find(false, Some(cache(Read::File(7), &calls)), false).unwrap(),
            Found::Read(7)
        );
        // `data == null`: a new type, nothing decoded, postDecode.
        assert_eq!(
            find(false, Some(cache(Read::NoFile, &calls)), false).unwrap(),
            Found::Defaults
        );
        assert_eq!(
            find(false, Some(cache(Read::NotYet, &calls)), true).unwrap(),
            Found::NotYet,
            "an open cache decides even when a prepared catalog is complete"
        );
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn without_a_cache_only_a_complete_catalog_gives_defaults() {
        let none = None::<fn() -> Result<Read<u32>, String>>;
        assert_eq!(find(false, none, true).unwrap(), Found::Defaults);
        assert_eq!(find(false, none, false).unwrap(), Found::Unknown);
    }

    #[test]
    fn a_failing_read_is_passed_on() {
        let failing = || Err::<Read<u32>, _>("npc/9: truncated".to_owned());
        assert_eq!(
            find(false, Some(failing), false).unwrap_err(),
            "npc/9: truncated"
        );
    }

    #[test]
    fn cache_results_are_classified_as_the_original_reads_them() {
        assert_eq!(classify(Ok(Some(3))).unwrap(), Read::File(3));
        assert_eq!(classify::<u32>(Ok(None)).unwrap(), Read::NoFile);
        assert_eq!(
            classify::<u32>(Err(rs09_cache::CacheError::Missing {
                archive: 18,
                group: 4
            }))
            .unwrap(),
            Read::NotYet
        );
        assert!(classify::<u32>(Err(rs09_cache::CacheError::Poisoned)).is_err());
    }

    /// A definition double: its id and the ids its morph table names.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Type(Vec<i32>);

    #[test]
    fn a_morph_and_every_type_it_may_become_are_read_once() {
        // 10 is held and names 11 and -1; 11 is in the cache and names 12
        // and 10 again; 12 has no file; 13, named by nobody, is never read.
        let asked = std::cell::RefCell::new(Vec::new());
        let gathered = gather(
            10,
            |id| (id == 10).then(|| vec![11, -1]),
            |id| {
                asked.borrow_mut().push(id);
                Ok(match id {
                    11 => Found::Read(Type(vec![12, 10])),
                    12 => Found::Defaults,
                    _ => Found::Unknown,
                })
            },
            || Ok(Type(vec![])),
            |t: &Type| t.0.clone(),
        );
        assert_eq!(*asked.borrow(), [11, 12]);
        assert_eq!(gathered.add, [(11, Type(vec![12, 10])), (12, Type(vec![]))]);
        assert!(gathered.unknown.is_empty() && gathered.not_yet.is_empty());
    }

    #[test]
    fn what_cannot_be_had_is_listed_and_the_walk_goes_on() {
        let gathered = gather(
            1,
            |_| None,
            |id| match id {
                1 => Ok(Found::Read(Type(vec![2, 3, 4]))),
                2 => Ok(Found::Unknown),
                3 => Ok(Found::NotYet),
                _ => Err(format!("npc/{id}: truncated")),
            },
            || Ok(Type(vec![])),
            |t: &Type| t.0.clone(),
        );
        assert_eq!(gathered.add, [(1, Type(vec![2, 3, 4]))]);
        assert_eq!(gathered.unknown, [2]);
        assert_eq!(gathered.not_yet, [3]);
        assert_eq!(gathered.failed, [(4, "npc/4: truncated".to_owned())]);
    }

    #[test]
    fn a_morph_table_that_never_ends_is_cut_off() {
        // Every type names the next: the walk stops at the limit.
        let gathered = gather(
            0,
            |_| None,
            |id| Ok(Found::Read(Type(vec![id as i32 + 1]))),
            || Ok(Type(vec![])),
            |t: &Type| t.0.clone(),
        );
        assert_eq!(gathered.add.len(), GATHER_LIMIT);
    }

    #[test]
    fn only_a_group_still_arriving_reads_as_absent() {
        use rs09_cache::{
            CacheError, Limits,
            disk::DiskStore,
            store::{Misses, Store},
        };
        use std::sync::{Arc, Mutex};
        let mut disk = DiskStore::in_memory(Vec::new(), Default::default());
        disk.ensure_archive(7).unwrap();
        let live = Store::live_recording(
            Arc::new(Mutex::new(disk)),
            Limits::default(),
            Misses::default(),
        );
        let missing = || CacheError::Missing {
            archive: 7,
            group: 9,
        };
        // The store did not have it: absent for now, the work goes on.
        let read = arriving(|| {
            live.read(7, 9)?;
            Err::<Option<u8>, _>(missing())
        });
        assert!(matches!(read, Ok(None)));
        // The same error without a live miss (a damaged finished store) is
        // passed on, as is any other failure.
        assert!(matches!(
            arriving(|| Err::<Option<u8>, _>(missing())),
            Err(CacheError::Missing { .. })
        ));
        assert!(matches!(
            arriving(|| Err::<Option<u8>, _>(CacheError::Poisoned)),
            Err(CacheError::Poisoned)
        ));
        assert!(matches!(arriving(|| Ok(Some(1u8))), Ok(Some(1))));
    }

    #[test]
    fn each_kind_and_id_is_reported_once() {
        let mut reported = Reported::default();
        assert!(reported.first("npc", 5));
        assert!(!reported.first("npc", 5));
        assert!(reported.first("npc", 6));
        assert!(reported.first("seq", 5));
        assert!(reported.contains("npc", 5));
        assert!(!reported.contains("loc", 5));
    }
}
