//! `cargo xtask preferences-oracle [--check]`: the unchanged RT4 `Preferences`
//! and `Cheat` classes run over small adapters; their observations become
//! `fixtures/preferences`.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Unchanged client classes compiled into the oracle.
const CLIENT: &[&str] = &[
    "Preferences",
    "Cheat",
    "Buffer",
    "Node",
    "JagString",
    "StringInterface",
    "StringNode",
    "HashTable",
    "BrowserControl",
    "Base37",
    "GlobalConfig",
    "ThreadUtils",
];
const OTHER: &[&str] = &[
    "reference/client/signlink/src/main/java/rt4/FileOnDisk.java",
    "reference/client/signlink/src/main/java/rt4/PrivilegedRequest.java",
    "oracle/preferences/adapters/rt4/OracleLog.java",
    "oracle/preferences/adapters/rt4/GameShell.java",
    "oracle/preferences/adapters/rt4/SceneGraph.java",
    "oracle/preferences/adapters/rt4/SignLink.java",
    "oracle/preferences/adapters/rt4/GlRenderer.java",
    "oracle/preferences/adapters/rt4/GlCleaner.java",
    "oracle/preferences/adapters/rt4/client.java",
    "oracle/preferences/adapters/rt4/Chat.java",
    "oracle/preferences/adapters/rt4/Player.java",
    "oracle/preferences/adapters/rt4/PlayerAppearance.java",
    "oracle/preferences/adapters/rt4/Protocol.java",
    "oracle/preferences/adapters/rt4/ClientProt.java",
    "oracle/preferences/adapters/rt4/DisplayMode.java",
    "oracle/preferences/adapters/rt4/CollisionMap.java",
    "oracle/preferences/adapters/rt4/PathFinder.java",
    "oracle/preferences/adapters/rt4/SeqType.java",
    "oracle/preferences/adapters/plugin/PluginRepository.java",
    "oracle/preferences/PreferencesOracle.java",
];
/// `PreferencesOracle` arguments: seed and the number of decode cases.
const ARGUMENTS: [&str; 2] = ["20261116", "1000"];

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
    let build = root.join("runs/preferences-oracle");
    let classes = build.join("classes");
    let files = build.join("files");
    for directory in [&classes, &files] {
        if directory.exists() {
            fs::remove_dir_all(directory)?;
        }
        fs::create_dir_all(directory)?;
    }
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
        .arg("PreferencesOracle")
        .args(ARGUMENTS)
        .arg(&files))?;
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
        "scope": "Unchanged Preferences (read, decode, encode, write, toInt) over real files opened through the unchanged FileOnDisk, and unchanged Cheat (execute, teleport) with the unchanged JagString and Buffer. read: files of every version, short, long, absent and unopenable, from a scrambled state, with GameShell.maxMemory on both sides of 96. decode: seeded bytes of versions 0 to 255 over default and scrambled states. write: over absent, shorter and longer files. cheat: every command of Cheat.execute and near misses, under four combinations of client.modeWhere, GlRenderer.enabled, SeqType.applyTweening and Cheat.shiftClick; recorded are the calls on the adapters in order, the exception, the switches, the collision arrays and the preferences file. SignLink (one directory instead of the original's eight cache locations, no request thread), GameShell, SceneGraph, GlRenderer, GlCleaner, client, Chat, Player, PlayerAppearance, Protocol, ClientProt, DisplayMode, CollisionMap, PathFinder, SeqType and PluginRepository are recording adapters: what the real classes do with those calls is not observed. Chat lines have their digits replaced because the memory figures vary by run."
    });
    let directory = root.join("fixtures/preferences");
    let (fixture_path, provenance_path) = (
        directory.join("oracle.jsonl"),
        directory.join("provenance.json"),
    );
    let total: usize = counts.values().sum();
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "preferences oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&provenance_path)?)? != provenance {
            return Err("preferences oracle provenance is stale; review source changes".into());
        }
        println!(
            "{total} JVM preference and command observations reproduced with matching sources."
        );
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&fixture_path, &fixture)?;
        fs::write(
            &provenance_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {total} JVM preference and command observations: {counts:?}");
    }
    Ok(())
}
