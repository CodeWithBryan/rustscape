//! `cargo xtask display-oracle [--check]`: the unchanged RT4 `DisplayMode`
//! class runs over recording adapters; its observations become
//! `fixtures/display`.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged classes compiled into the oracle.
const UNCHANGED: &[&str] = &[
    "reference/client/client/src/main/java/rt4/DisplayMode.java",
    "reference/client/client/src/main/java/rt4/ArrayUtils.java",
    "reference/client/client/src/main/java/rt4/ThreadUtils.java",
    "reference/client/signlink/src/main/java/rt4/PrivilegedRequest.java",
];
const ADAPTERS: &[&str] = &[
    "oracle/display/adapters/rt4/OracleLog.java",
    "oracle/display/adapters/rt4/GameShell.java",
    "oracle/display/adapters/rt4/Adapters.java",
    "oracle/display/adapters/plugin/PluginRepository.java",
    "oracle/display/rt4/DisplayOracle.java",
];
/// Files whose text the adapters and the driver copy by hand (their classes
/// cannot be compiled alone). They are not compiled; their hashes are
/// recorded so that a change to them makes the provenance stale.
const COPIED_FROM: &[&str] = &[
    "reference/client/client/src/main/java/rt4/GameShell.java",
    "reference/client/client/src/main/java/rt4/client.java",
    "reference/client/client/src/main/java/rt4/ClientProt.java",
    "reference/client/client/src/main/java/rt4/ScriptRunner.java",
];
/// `DisplayOracle` arguments: seed and the number of cases.
const ARGUMENTS: [&str; 2] = ["20261116", "600"];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let mut sources: Vec<String> = UNCHANGED
        .iter()
        .chain(ADAPTERS)
        .map(|path| (*path).to_owned())
        .collect();
    sources.sort();
    let hash = |path: &str| -> Result<String> {
        Ok(format!(
            "{:x}",
            Sha256::digest(normalized(&fs::read(root.join(path))?))
        ))
    };
    let mut hashes = BTreeMap::new();
    for source in &sources {
        hashes.insert(source.clone(), hash(source)?);
    }
    let mut copied = BTreeMap::new();
    for source in COPIED_FROM {
        copied.insert((*source).to_owned(), hash(source)?);
    }
    let classes = root.join("runs/display-oracle/classes");
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
        .arg("rt4.DisplayOracle")
        .args(ARGUMENTS))?;
    let fixture = normalized(&output.stdout);
    let (mut cases, mut steps) = (0usize, BTreeMap::<String, usize>::new());
    for line in fixture.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let value: Value = serde_json::from_slice(line)?;
        cases += 1;
        for step in value["steps"].as_array().into_iter().flatten() {
            *steps
                .entry(step["op"][0].as_str().unwrap_or("?").to_owned())
                .or_default() += 1;
        }
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "copiedFrom": copied,
        "arguments": ARGUMENTS,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "cases": cases,
        "steps": steps,
        "scope": "Unchanged DisplayMode (setWindowMode in both forms, getWindowMode, getDisplayModes, getAvailableDisplayModes, createFullScreenFrame, exitFullScreen) with the unchanged ArrayUtils, ThreadUtils and PrivilegedRequest, from the state GameShell.startApplication leaves for a frame of a seeded size, under seeded answers of the window system (full screen supported or not, the screen's modes or a failed request, a full-screen frame made, not made or refused) and of the hardware renderer (starts or does not). Each case is a sequence of calls made the way the client makes them; recorded after each are the calls on the adapters in order, getWindowMode, frame, canvas and margins, the renderer and full-screen flags, the saved full-screen size and write count, and the rectangles paintFrameLetterbox would fill. Copied by hand, not compiled from the reference: GameShell.updateCanvasSize and paintFrameLetterbox (in the GameShell adapter, the latter with its Graphics calls replaced by recording), ClientProt.sendWindowDetails (in the adapters) and the callers' statements in the driver (client.mainRedraw's canvas watch, ScriptRunner opcodes 5300, 5301, 5302, 5303, 5305 and 5307); copiedFrom holds the hashes of the files they were copied from. Everything else DisplayMode touches is a recording adapter: what the real renderer, canvas, listeners, interface layout and plugins do with those calls is not observed. Frames and the canvas are java.awt objects that are sized but never shown, so they have no insets; no window is opened and the screen is not changed."
    });
    let directory = root.join("fixtures/display");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let total: usize = steps.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "display oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("display oracle provenance is stale; review source changes".into());
        }
        println!("{cases} JVM display cases ({total} steps) reproduced with matching sources.");
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {cases} JVM display cases, {total} steps: {steps:?}");
    }
    Ok(())
}
