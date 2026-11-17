//! `cargo xtask menus-oracle [--check]`: the unchanged RT4 `Font` wavy-text
//! renderers and `CursorType.decode` run over seeded inputs; their
//! observations become `fixtures/menus`.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "Font",
    "SecondaryNode",
    "Node",
    "JagString",
    "StringInterface",
    "StringNode",
    "HashTable",
    "Base37",
    "BrowserControl",
    "IndexedSprite",
    "Buffer",
    "CursorType",
];
/// Adapters under `oracle/menus/adapters/rt4`, and the probe.
const ADAPTERS: &[&str] = &[
    "GlRenderer",
    "GlRaster",
    "SoftwareRaster",
    "GlFont",
    "SoftwareFont",
    "SpriteLoader",
    "Js5",
    "SoftLruHashTable",
    "CursorTypeList",
    "SoftwareSprite",
];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("reference/client/client/src/main/java/rt4/{name}.java"))
        .chain(
            ADAPTERS
                .iter()
                .map(|name| format!("oracle/menus/adapters/rt4/{name}.java")),
        )
        .chain(["oracle/menus/MenusOracle.java".to_owned()])
        .collect();
    sources.sort();
    let mut hashes = BTreeMap::new();
    for source in &sources {
        hashes.insert(
            source.clone(),
            format!(
                "{:x}",
                Sha256::digest(normalized(&fs::read(root.join(source))?))
            ),
        );
    }
    let classes = root.join("runs/menus-oracle/classes");
    if classes.exists() {
        fs::remove_dir_all(&classes)?;
    }
    fs::create_dir_all(&classes)?;
    run(Command::new("javac")
        .current_dir(root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .args(&sources))?;
    let output = run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(&classes)
        .arg("MenusOracle"))?;
    let fixture = normalized(&output.stdout);
    let mut counts = BTreeMap::<String, usize>::new();
    for line in fixture.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let value: Value = serde_json::from_slice(line)?;
        *counts
            .entry(value["case"].as_str().unwrap_or("?").to_owned())
            .or_default() += 1;
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "scope": "Unchanged Font.renderWavyText, renderWavyTextAligned and renderOffset (with setColors, parseTag, getStringWidth and the metrics constructor) over four seeded unkerned fonts, with every glyph and name-icon draw recorded instead of rasterised; unchanged CursorType.decode over seeded and truncated definitions, its private sprite id read through getSprite's cache lookup; java.util.Random after setSeed. The renderer switch, line drawing, sprite loading and the cursor cache are adapters that draw and load nothing. The clock cases repeat client.mainLoop's seed expression in the probe over java.util.Calendar; that method is not compiled. MiniMenu, the menu state and the kerned font profile are not observed."
    });
    let directory = root.join("fixtures/menus");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "menus oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("menus oracle provenance is stale; review source changes".into());
        }
        println!("{total} JVM menu observations reproduced with matching sources.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM menu observations: {counts:?}");
    }
    Ok(())
}
