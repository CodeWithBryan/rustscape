//! `ServerStore.parse` and the other world persist's `parse`
//! (`GraveController.deserializeFromServerStore`), the two `PersistWorld`
//! objects the class scan registers (`listeners.persistWorld` in the boot
//! record, in that order).
//!
//! `ServerStore` keeps one JSON object per file of `STORE_PATH`
//! (`fileMap`, keyed by the file name without its extension) and counts the
//! files it parsed. A missing directory is created and nothing is read. A file
//! that is not a JSON object is logged at ERR and skipped (the original catches
//! `Exception` around the parse and the cast); a file that cannot be opened is
//! the original's uncaught `FileNotFoundException` (it is opened before the
//! `try`). Files are read in the order the operating system lists them, as
//! `File.listFiles` does; that order is not pinned by the original either.
//!
//! `GraveController.parse` reads the archive `active-graves`
//! (`ServerStore.getArchive`, which stores an empty object under that name
//! when there is none). A grave is a content object (`Grave`, `GraveType`)
//! this port does not have: an archive with entries is refused, never
//! skipped. Verified against the original by `fixtures/startup-draws`
//! (`serverStore` and `activeGraves` after each phase).
use super::loaders::{LoadError, Log, Result};
use crate::world::drop_tables::LogLevel;
use rs09_game_core::java_json::{self, Json};
use std::path::Path;

/// The writes the start-up makes in the world directory. The original creates
/// `STORE_PATH` when it is missing; a host decides where that happens (the
/// whole-start-up test records it instead of writing into the pinned data).
pub trait WorldWrites: Send + Sync {
    /// `File.mkdirs()`.
    fn create_dirs(&self, path: &Path) -> std::io::Result<()>;
}

/// The disk: `std::fs::create_dir_all`.
pub struct Disk;
impl WorldWrites for Disk {
    fn create_dirs(&self, path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }
}

/// `ServerStore.fileMap` and `counter`. Entries keep their first insertion
/// position and a later `put` replaces the value, as a map does; the map's
/// own iteration order (a `HashMap<String, JSONObject>`) is not modelled
/// because nothing in the start-up walks it.
#[derive(Clone, Debug, Default)]
pub struct ServerStore {
    files: Vec<(String, Json)>,
    pub counter: i32,
}
impl ServerStore {
    pub fn keys(&self) -> Vec<&str> {
        self.files.iter().map(|(k, _)| k.as_str()).collect()
    }
    pub fn get(&self, name: &str) -> Option<&Json> {
        self.files.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
    fn put(&mut self, name: String, data: Json) {
        match self.files.iter_mut().find(|(k, _)| *k == name) {
            Some(entry) => entry.1 = data,
            None => self.files.push((name, data)),
        }
    }
    /// `ServerStore.getArchive(name)`: stores an empty object first when the
    /// name has none.
    pub fn archive(&mut self, name: &str) -> &Json {
        if self.get(name).is_none() {
            self.files.push((name.to_string(), Json::object()));
        }
        self.get(name).expect("stored above")
    }
}

/// Kotlin `File.nameWithoutExtension`: the name up to its last dot.
fn name_without_extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) => &name[..dot],
        None => name,
    }
}

/// `ServerStore.parse()` with `STORE_PATH` = `dir`.
pub fn parse(
    store: &mut ServerStore,
    dir: &Path,
    writes: &dyn WorldWrites,
    log: &mut impl Log,
) -> Result<()> {
    log.log(
        "SystemLogger",
        LogLevel::Fine,
        "[STARTUP] Parsing server store...",
    );
    if !dir.exists() {
        writes.create_dirs(dir).map_err(LoadError::File)?;
        return Ok(());
    }
    // `dir.listFiles()` is null for a path that is not a directory.
    let Ok(listing) = std::fs::read_dir(dir) else {
        log.log(
            "SystemLogger",
            LogLevel::Fine,
            &format!("[STARTUP] Initialized {} store files.", store.counter),
        );
        return Ok(());
    };
    for entry in listing {
        let entry = entry.map_err(LoadError::File)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let key = name_without_extension(&name).to_string();
        // FileReader(storeFile) is outside the try: a directory or an
        // unreadable file leaves parse.
        if entry.path().is_dir() {
            return Err(LoadError::File(std::io::ErrorKind::IsADirectory.into()));
        }
        let bytes = std::fs::read(entry.path()).map_err(LoadError::File)?;
        let parsed = String::from_utf8(bytes)
            .map_err(|_| LoadError::Binding("store file is not UTF-8".into()))
            .map(|text| java_json::parse(&text));
        match parsed? {
            Ok(data @ Json::Object(_)) => {
                store.put(key, data);
                store.counter = store.counter.wrapping_add(1);
            }
            _ => log.log(
                "ServerStore",
                LogLevel::Err,
                &format!("Failed parsing {name} - stack trace below."),
            ),
        }
    }
    log.log(
        "SystemLogger",
        LogLevel::Fine,
        &format!("[STARTUP] Initialized {} store files.", store.counter),
    );
    Ok(())
}

/// `GraveController.parse()`: the graves of the `active-graves` archive.
pub fn parse_graves(store: &mut ServerStore) -> Result<usize> {
    match store.archive("active-graves") {
        Json::Object(entries) if entries.is_empty() => Ok(0),
        Json::Object(entries) => Err(LoadError::Binding(format!(
            "GraveController: {} saved graves; Grave/GraveType are content and not ported",
            entries.len()
        ))),
        _ => Err(LoadError::Type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Lines(Vec<String>);
    impl Log for Lines {
        fn log(&mut self, origin: &'static str, level: LogLevel, message: &str) {
            if level != LogLevel::Err {
                self.0.push(format!("[{origin}] {message}"));
            }
        }
    }
    struct Recorded(std::sync::Mutex<Vec<std::path::PathBuf>>);
    impl WorldWrites for Recorded {
        fn create_dirs(&self, path: &Path) -> std::io::Result<()> {
            self.0.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }
    }

    #[test]
    fn a_missing_store_is_created_and_not_counted() {
        let dir = std::env::temp_dir().join(format!("rs09-store-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let writes = Recorded(Default::default());
        let mut store = ServerStore::default();
        let mut log = Lines(vec![]);
        parse(&mut store, &dir, &writes, &mut log).unwrap();
        assert_eq!(*writes.0.lock().unwrap(), vec![dir]);
        assert_eq!(log.0, ["[SystemLogger] [STARTUP] Parsing server store..."]);
        assert_eq!(store.counter, 0);
        assert_eq!(parse_graves(&mut store).unwrap(), 0);
        assert_eq!(store.keys(), ["active-graves"]);
    }

    #[test]
    fn objects_are_kept_and_other_files_skipped() {
        let dir = std::env::temp_dir().join(format!("rs09-store-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.json"), "{\"x\":1}").unwrap();
        std::fs::write(dir.join("b.json"), "[1]").unwrap();
        std::fs::write(dir.join("c.v.json"), "{}").unwrap();
        let mut store = ServerStore::default();
        let mut log = Lines(vec![]);
        parse(&mut store, &dir, &Disk, &mut log).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(store.counter, 2);
        assert_eq!(store.keys(), ["a", "c.v"]);
        assert_eq!(
            log.0[1],
            "[SystemLogger] [STARTUP] Initialized 2 store files."
        );
    }
}
