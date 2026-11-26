//! Files written beside their destinations and moved into place together,
//! for the pack builders (`examples/build_packs.rs`): a command that fails
//! part way, or whose publication fails part way, leaves the files it would
//! have replaced as they were.
use std::path::{Path, PathBuf};

/// `path` with `suffix` appended to its file name.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// The staged files of one command: (staged, destination), in staging order.
#[derive(Default)]
pub struct Staging {
    files: Vec<(PathBuf, PathBuf)>,
}

impl Staging {
    /// Writes `bytes` beside `destination` (as `NAME.partial`) for a later
    /// [`publish`](Self::publish).
    pub fn stage(&mut self, destination: &Path, bytes: &[u8]) -> Result<(), String> {
        let staged = with_suffix(destination, ".partial");
        std::fs::write(&staged, bytes).map_err(|e| format!("{}: {e}", staged.display()))?;
        self.files.push((staged, destination.to_path_buf()));
        Ok(())
    }

    /// Moves every staged file into place. Each existing destination is
    /// first set aside (as `NAME.previous`); if any move fails, the files
    /// already moved are removed, the set-aside files are put back and the
    /// staged files are discarded, so the destinations end as they began.
    pub fn publish(mut self) -> Result<(), String> {
        let files = std::mem::take(&mut self.files);
        let mut kept: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut placed: Vec<PathBuf> = Vec::new();
        let mut result = Ok(());
        for (_, destination) in &files {
            if destination.exists() {
                let previous = with_suffix(destination, ".previous");
                if let Err(e) = std::fs::rename(destination, &previous) {
                    result = Err(format!("{}: {e}", destination.display()));
                    break;
                }
                kept.push((previous, destination.clone()));
            }
        }
        if result.is_ok() {
            for (staged, destination) in &files {
                if let Err(e) = std::fs::rename(staged, destination) {
                    result = Err(format!("{}: {e}", destination.display()));
                    break;
                }
                placed.push(destination.clone());
            }
        }
        match result {
            Ok(()) => {
                for (previous, _) in kept {
                    let _ = std::fs::remove_file(previous);
                }
                Ok(())
            }
            Err(error) => {
                for destination in placed {
                    let _ = std::fs::remove_file(destination);
                }
                for (previous, destination) in kept {
                    let _ = std::fs::rename(previous, destination);
                }
                for (staged, _) in files {
                    let _ = std::fs::remove_file(staged);
                }
                Err(error)
            }
        }
    }
}

impl Drop for Staging {
    /// Staged files that were never published are removed.
    fn drop(&mut self) {
        for (staged, _) in self.files.drain(..) {
            let _ = std::fs::remove_file(staged);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rs09-staging-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn publishing_replaces_every_destination_and_leaves_nothing_beside_them() {
        let dir = directory("ok");
        std::fs::write(dir.join("a.pack"), b"old a").unwrap();
        let mut staging = Staging::default();
        staging.stage(&dir.join("a.pack"), b"new a").unwrap();
        staging.stage(&dir.join("b.pack"), b"new b").unwrap();
        staging.publish().unwrap();
        assert_eq!(std::fs::read(dir.join("a.pack")).unwrap(), b"new a");
        assert_eq!(std::fs::read(dir.join("b.pack")).unwrap(), b"new b");
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["a.pack", "b.pack"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_later_move_restores_every_earlier_destination() {
        let dir = directory("fail");
        std::fs::write(dir.join("a.pack"), b"old a").unwrap();
        std::fs::write(dir.join("a.provenance.json"), b"old sidecar").unwrap();
        let mut staging = Staging::default();
        staging.stage(&dir.join("a.pack"), b"new a").unwrap();
        staging
            .stage(&dir.join("a.provenance.json"), b"new sidecar")
            .unwrap();
        staging.stage(&dir.join("c.pack"), b"new c").unwrap();
        // The last move fails: its staged file is gone.
        std::fs::remove_file(dir.join("c.pack.partial")).unwrap();
        assert!(staging.publish().is_err());
        assert_eq!(std::fs::read(dir.join("a.pack")).unwrap(), b"old a");
        assert_eq!(
            std::fs::read(dir.join("a.provenance.json")).unwrap(),
            b"old sidecar"
        );
        assert!(!dir.join("c.pack").exists());
        for leftover in [
            "a.pack.partial",
            "a.pack.previous",
            "a.provenance.json.previous",
        ] {
            assert!(!dir.join(leftover).exists(), "{leftover} was left behind");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dropping_without_publishing_discards_the_staged_files() {
        let dir = directory("drop");
        std::fs::write(dir.join("a.pack"), b"old a").unwrap();
        {
            let mut staging = Staging::default();
            staging.stage(&dir.join("a.pack"), b"new a").unwrap();
        }
        assert_eq!(std::fs::read(dir.join("a.pack")).unwrap(), b"old a");
        assert!(!dir.join("a.pack.partial").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
