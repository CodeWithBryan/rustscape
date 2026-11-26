//! Where the tests find the prepared packs.
//!
//! The packs the client loads by default (`assets/visual/*.pack`) were
//! rebuilt from the pinned cache on 2026-11-26 with the native builders. The
//! packs the Python scripts wrote, which the JVM fixtures were recorded from
//! and name by hash, are kept beside them under `assets/visual/recorded/`.
#![allow(dead_code)]
use std::path::{Path, PathBuf};

pub fn visual() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/visual")
}

/// A pack (or its provenance file) as the Python scripts wrote it. Before
/// the rebuild those were the default files themselves.
pub fn recorded(name: &str) -> PathBuf {
    let kept = visual().join("recorded").join(name);
    if kept.is_file() {
        kept
    } else {
        visual().join(name)
    }
}

/// Whether the default packs are the native builders' (their provenance
/// names the builder), not the Python scripts'.
pub fn rebuilt() -> bool {
    std::fs::read(visual().join("demo.provenance.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .is_some_and(|record| record["builder"].is_string())
}
