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
    /// If that rollback fails too, the error says so and names each
    /// set-aside file left for recovery.
    pub fn publish(self) -> Result<(), String> {
        self.publish_with(|from, to| std::fs::rename(from, to))
    }

    fn publish_with(
        mut self,
        mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<(), String> {
        let files = std::mem::take(&mut self.files);
        let mut kept: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut placed: Vec<PathBuf> = Vec::new();
        let mut failure = None;
        for (_, destination) in &files {
            if destination.exists() {
                let previous = with_suffix(destination, ".previous");
                if let Err(e) = rename(destination, &previous) {
                    failure = Some(format!("{}: {e}", destination.display()));
                    break;
                }
                kept.push((previous, destination.clone()));
            }
        }
        if failure.is_none() {
            for (staged, destination) in &files {
                if let Err(e) = rename(staged, destination) {
                    failure = Some(format!("{}: {e}", destination.display()));
                    break;
                }
                placed.push(destination.clone());
            }
        }
        let Some(error) = failure else {
            for (previous, _) in kept {
                let _ = std::fs::remove_file(previous);
            }
            return Ok(());
        };
        let mut unrestored = Vec::new();
        for destination in placed {
            if let Err(e) = std::fs::remove_file(&destination) {
                unrestored.push(format!("{} (not removed: {e})", destination.display()));
            }
        }
        for (previous, destination) in kept {
            if let Err(e) = rename(&previous, &destination) {
                unrestored.push(format!(
                    "{} is kept as {} ({e})",
                    destination.display(),
                    previous.display()
                ));
            }
        }
        for (staged, _) in files {
            let _ = std::fs::remove_file(staged);
        }
        if unrestored.is_empty() {
            Err(error)
        } else {
            Err(format!(
                "{error}; the previous files could not all be put back: {}",
                unrestored.join("; ")
            ))
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

    #[test]
    fn a_failed_rollback_is_reported_and_keeps_the_set_aside_file() {
        let dir = directory("rollback");
        std::fs::write(dir.join("a.pack"), b"old a").unwrap();
        let mut staging = Staging::default();
        staging.stage(&dir.join("a.pack"), b"new a").unwrap();
        staging.stage(&dir.join("b.pack"), b"new b").unwrap();
        // Set aside a.pack, place a.pack, then fail placing b.pack and fail
        // putting the set-aside a.pack back.
        let mut calls = 0;
        let error = staging
            .publish_with(|from, to| {
                calls += 1;
                match calls {
                    3 | 4 => Err(std::io::Error::other("injected")),
                    _ => std::fs::rename(from, to),
                }
            })
            .unwrap_err();
        assert!(error.contains("could not all be put back"), "{error}");
        assert!(error.contains("a.pack.previous"), "{error}");
        assert_eq!(
            std::fs::read(dir.join("a.pack.previous")).unwrap(),
            b"old a"
        );
        assert!(!dir.join("a.pack").exists());
        assert!(!dir.join("b.pack.partial").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_sidecar_move_puts_the_previous_pack_and_sidecar_back() {
        let dir = directory("sidecar");
        std::fs::write(dir.join("a.pack"), b"old a").unwrap();
        std::fs::write(dir.join("a.provenance.json"), b"old sidecar").unwrap();
        let mut staging = Staging::default();
        staging.stage(&dir.join("a.pack"), b"new a").unwrap();
        staging
            .stage(&dir.join("a.provenance.json"), b"new sidecar")
            .unwrap();
        // Set aside both, place the pack, then fail placing the sidecar.
        let mut calls = 0;
        let error = staging
            .publish_with(|from, to| {
                calls += 1;
                if calls == 4 {
                    Err(std::io::Error::other("injected"))
                } else {
                    std::fs::rename(from, to)
                }
            })
            .unwrap_err();
        assert!(!error.contains("could not all be put back"), "{error}");
        assert_eq!(std::fs::read(dir.join("a.pack")).unwrap(), b"old a");
        assert_eq!(
            std::fs::read(dir.join("a.provenance.json")).unwrap(),
            b"old sidecar"
        );
        for leftover in [
            "a.pack.partial",
            "a.pack.previous",
            "a.provenance.json.partial",
            "a.provenance.json.previous",
        ] {
            assert!(!dir.join(leftover).exists(), "{leftover} was left behind");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
