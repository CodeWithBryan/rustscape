//! `cargo xtask ui-fidelity-oracle [--check]`: the unchanged RT4 interface
//! walk, layout, item count text, font kerning and number parsing run over
//! seeded inputs; their observations become `fixtures/ui-fidelity`.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "InterfaceList",
    "HookRequest",
    "LinkedList",
    "HashTable",
    "HashTableIterator",
    "Node",
    "SecondaryNode",
    "ComponentPointer",
    "ServerActiveProperties",
    "Inv",
    "Font",
    "SoftwareFont",
    "SoftwareRaster",
    "IndexedSprite",
    "JagString",
    "StringNode",
    "StringInterface",
    "Base37",
];
const OTHER: &[&str] = &[
    "oracle/ui-fidelity/adapters/rt4/Adapters.java",
    "oracle/ui-fidelity/rt4/UiFidelityOracle.java",
];
/// `UiFidelityOracle` arguments: seed, walk scenarios, kerned fonts, count
/// text cases.
const ARGUMENTS: [&str; 4] = ["20261116", "260", "6", "160"];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let mut sources: Vec<String> = CLIENT
        .iter()
        .map(|name| format!("reference/client/client/src/main/java/rt4/{name}.java"))
        .chain(OTHER.iter().map(|path| (*path).to_owned()))
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
    let build = root.join("runs/ui-fidelity-oracle");
    let classes = build.join("classes");
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
        .arg("rt4.UiFidelityOracle")
        .args(ARGUMENTS))?;
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
        "arguments": ARGUMENTS,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": counts,
        "scope": "Unchanged InterfaceList (layoutTopLevel, calculateSize, calculatePosition, processComponents, handleScrollbar), Inv.renderObjectSprite and formatObjAmount, Font and SoftwareFont (decode, calculateKerning, getStringWidth, render, renderGlyph), SoftwareRaster and JagString.parseHexString over seeded inputs. Component is an adapter holding the fields InterfaceList touches; every other class InterfaceList and Inv name is an adapter without behaviour, except Cs1ScriptRunner.startComponentDrag and getDragParent (the original's bodies) and SoftwareModel.setCamera (copies a staged picture in place of rendering a model). The walk driver stages what Protocol.loop does around processSubInterface (clears the hovered and tooltip components and the drag, fills the key queue, increments transmitTimer) and reads the three request lists without running scripts. Not observed: component key bindings (all keyCodes null), the world map component 1400, a click while an inventory item is held, an IF1 tooltip or scrollbar in a walk that also starts a drag, Protocol.loop's discard of requests for replaced created components, item models, outlines, shadows, notes and count variants, and any GL drawing."
    });
    let directory = root.join("fixtures/ui-fidelity");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "interface fidelity oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err(
                "interface fidelity oracle provenance is stale; review source changes".into(),
            );
        }
        println!("{total} JVM interface observations reproduced with matching sources.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM interface observations: {counts:?}");
    }
    Ok(())
}
