//! Reader for the world-trace fixture (`cargo xtask world-trace-oracle`).
//!
//! The fixture is what the whole original server did, step by step, in four
//! scripted runs. These tests parse it, check its pins and assert the
//! properties a native replayer will depend on. Nothing here runs a native
//! world: no Rust server reproduces any of these traces yet, and a pass says
//! nothing about the Rust server. See docs/world-trace.md.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const TRACE: &str = include_str!("../../../fixtures/world-trace/trace.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/world-trace/provenance.json");
const ARTIFACTS: &str = include_str!("../../../oracle/world-trace/artifacts.lock.json");
const UPSTREAM: &str = include_str!("../../../upstream.lock.json");
const SCHEMA: u64 = 1;

fn sha256(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hex(text: &str) -> Vec<u8> {
    assert!(text.len() % 2 == 0 && text.is_ascii(), "odd hex");
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex digit"))
        .collect()
}

struct Scenario {
    header: Value,
    records: Vec<Value>,
}

fn scenarios() -> Vec<Scenario> {
    let mut out: Vec<Scenario> = Vec::new();
    for line in TRACE.lines() {
        let record: Value = serde_json::from_str(line).expect("trace line is JSON");
        if record["t"] == "scenario" {
            out.push(Scenario {
                header: record,
                records: Vec::new(),
            });
        } else {
            out.last_mut()
                .expect("a scenario record comes first")
                .records
                .push(record);
        }
    }
    out
}

fn counter(generators: &Value, name: &str) -> u64 {
    match &generators[name] {
        Value::Array(pair) => pair[0].as_u64().unwrap(),
        value => value.as_u64().unwrap(),
    }
}

#[test]
fn fixture_and_its_inputs_are_pinned() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["format"], 1);
    assert_eq!(provenance["schema"], SCHEMA);
    assert_eq!(provenance["fixtureSha256"], sha256(TRACE));
    assert_eq!(
        provenance["upstream"],
        serde_json::from_str::<Value>(UPSTREAM).unwrap()
    );
    // Two fresh JVM runs agreed byte for byte when the fixture was written.
    assert_eq!(provenance["determinism"]["freshJvmRuns"], 2);
    assert_eq!(provenance["determinism"]["identical"], true);
    assert!(
        provenance["javaVersion"]
            .as_str()
            .unwrap()
            .contains("version \"11.")
    );
    assert_eq!(provenance["kotlinCompiler"], "1.8.20");

    // Every source file of the original server went into the build.
    let tree = &provenance["sourceTree"];
    assert_eq!(tree["files"], 3487);
    assert_eq!(tree["kotlin"], 1863);
    assert_eq!(tree["java"], 1624);
    assert_eq!(tree["manifestSha256"].as_str().unwrap().len(), 64);
    assert_eq!(tree["gitTree"].as_str().unwrap().len(), 40);

    // Every jar on the original's class path is the one the lock file names.
    let lock: Value = serde_json::from_str(ARTIFACTS).unwrap();
    let lock = lock.as_object().unwrap();
    let artifacts = provenance["artifacts"].as_object().unwrap();
    assert_eq!(artifacts.len(), lock.len());
    for (name, digest) in artifacts {
        assert_eq!(digest, &lock[name]["sha256"], "{name}");
        assert!(lock[name]["url"].as_str().unwrap().starts_with("https://"));
    }
    assert!(
        !provenance["compilerArtifacts"]
            .as_object()
            .unwrap()
            .is_empty()
    );

    // The one configuration key that differs from the shipped default.conf.
    assert_eq!(
        provenance["config"]["overrides"],
        serde_json::json!({"enable_bots": {"shipped": "true", "used": "false"}})
    );
    // The wall clock and unseeded generators were redirected, and the count is on record.
    let sites = provenance["redirectedCallSites"]["sites"]
        .as_object()
        .unwrap();
    assert!(sites["System.currentTimeMillis"].as_u64().unwrap() > 0);
    assert!(sites["new Random()"].as_u64().unwrap() > 0);
    assert!(
        provenance["scope"]
            .as_str()
            .unwrap()
            .contains("No native world replays this trace")
    );

    // Harness, tool, generator and studied original sources still hash as recorded.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut checked = 0;
    for (path, hash) in provenance["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|&b| b != b'\r')
                .collect();
            assert_eq!(hash, &sha256(bytes), "{path:?}");
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn boot_record_describes_one_loaded_world() {
    let all = scenarios();
    let names: Vec<_> = all
        .iter()
        .map(|s| s.header["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "boot",
            "new-character",
            "two-characters",
            "returning-character"
        ]
    );
    let boot = &all[0].records[0];
    assert_eq!(boot["t"], "boot");
    assert_eq!(boot["tick"], 0);
    assert_eq!(boot["systemState"], "PRIVATE");
    assert!(boot["players"].as_array().unwrap().is_empty());
    assert_eq!(boot["stderrLines"], 0);

    // The full NPC list is recorded once; its digests are the ones every later record repeats the form of.
    let npcs = &boot["npcs"];
    let list = npcs["list"].as_array().unwrap();
    assert_eq!(npcs["count"], list.len());
    let (mut index, mut state, mut seen) = (String::new(), String::new(), BTreeSet::new());
    let mut rows = BTreeMap::new();
    for row in list {
        let row: Vec<i64> = row
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();
        // [slot, reported index, id, x, y, z]. Slots are distinct and ascending.
        assert!(row[0] >= 1 && seen.insert(row[0]), "NPC slot {}", row[0]);
        assert!(row[1] >= row[0], "NPC slot {}", row[0]);
        index += &format!("{}:{}:{};", row[0], row[1], row[2]);
        state += &format!(
            "{}:{}:{}:{},{},{};",
            row[0], row[1], row[2], row[3], row[4], row[5]
        );
        rows.insert(row[0], row);
    }
    // The original can hold one NPC in two slots; it then reports the later slot from both.
    for (slot, row) in &rows {
        if row[1] != *slot {
            assert_eq!(rows[&row[1]][1..], row[1..], "NPC slot {slot}");
        }
    }
    assert!(
        rows.iter().any(|(slot, row)| row[1] != *slot),
        "the pinned world has an NPC held in two slots"
    );
    assert_eq!(npcs["indexSha256"], sha256(index));
    assert_eq!(npcs["stateSha256"], sha256(state));

    // Registration order is part of the record: lists, not sets.
    for kind in [
        "login",
        "loginPlugins",
        "logout",
        "persistPlayer",
        "persistWorld",
        "shutdown",
        "startup",
        "tick",
        "timers",
    ] {
        let list = boot["listeners"][kind]
            .as_array()
            .unwrap_or_else(|| panic!("{kind}"));
        assert!(list.iter().all(Value::is_string), "{kind}");
    }
    assert_eq!(boot["listeners"]["persistWorld"][0], "core.ServerStore");
    for table in [
        "contentInterfaces",
        "itemDefinitions",
        "legacyPlugins",
        "npcDefinitions",
        "sceneryDefinitions",
    ] {
        assert!(boot["tables"][table].as_u64().unwrap() > 0, "{table}");
    }
    assert!(!boot["registries"].as_object().unwrap().is_empty());
    // The class scan's order, which is the order the original instantiated content in.
    for kind in ["contentInterfaces", "initializable", "timers"] {
        let names = boot["scan"][kind]
            .as_array()
            .unwrap_or_else(|| panic!("{kind}"));
        assert!(
            !names.is_empty() && names.iter().all(Value::is_string),
            "{kind}"
        );
    }

    // Every scenario starts from the same world: the others carry only the digest of this record.
    let mut body = boot.clone();
    let fields = body.as_object_mut().unwrap();
    fields.remove("i");
    fields.remove("sha256");
    assert_eq!(
        boot["sha256"],
        sha256(serde_json::to_string(&body).unwrap())
    );
    for scenario in &all[1..] {
        let other = &scenario.records[0];
        assert_eq!(other["t"], "boot");
        assert_eq!(other["sameAsFirstScenario"], true);
        assert_eq!(other["sha256"], boot["sha256"]);
    }
}

#[test]
fn every_step_is_attributed_and_consistent() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    let mut census = Vec::new();
    for scenario in scenarios() {
        let name = scenario.header["name"].as_str().unwrap().to_string();
        let start = scenario.header["clockStart"].as_u64().unwrap();
        let tick_millis = scenario.header["tickMillis"].as_u64().unwrap();
        assert_eq!(tick_millis, 600);
        // One record per scripted step, between the boot and end records.
        assert_eq!(
            scenario.records.len() as u64,
            scenario.header["steps"].as_u64().unwrap() + 2,
            "{name}"
        );
        assert_eq!(scenario.records.last().unwrap()["t"], "end");

        let mut connected = BTreeSet::new();
        let mut closed = BTreeSet::new();
        let mut logged_in = BTreeSet::new();
        let mut saves = BTreeMap::new();
        let (mut tick, mut previous): (u64, Option<Value>) = (0, None);
        let (mut ticks, mut packets, mut replies, mut bytes_out, mut dropped) = (0, 0, 0, 0, 0);
        for (i, record) in scenario.records.iter().enumerate() {
            let at = format!("{name} record {i}");
            assert_eq!(record["i"], i, "{at}");
            let kind = record["t"].as_str().unwrap();
            if kind == "boot" {
                continue;
            }
            // The tick counter moves by exactly one per tick record and not otherwise.
            if kind == "tick" {
                tick += 1;
                ticks += 1;
            } else {
                assert!(kind == "io" || kind == "end", "{at}: {kind}");
            }
            assert_eq!(record["tick"], tick, "{at}");
            let generators = &record["generators"];
            assert_eq!(generators["clock"], start + tick * tick_millis, "{at}");
            // Generator positions only move forward.
            if let Some(previous) = &previous {
                for generator in ["clockReads", "seeded", "math", "shared", "shuffle"] {
                    assert!(
                        counter(generators, generator) >= counter(previous, generator),
                        "{at}: {generator}"
                    );
                }
            }
            previous = Some(generators.clone());

            if kind == "io" {
                let session = record["session"].as_str().unwrap().to_string();
                match record["op"].as_str().unwrap() {
                    "connect" => assert!(connected.insert(session), "{at}"),
                    "recv" => {
                        assert!(connected.contains(&session) && !closed.contains(&session));
                        assert!(!hex(record["hex"].as_str().unwrap()).is_empty(), "{at}");
                    }
                    "close" => {
                        assert!(connected.contains(&session) && closed.insert(session));
                    }
                    op => panic!("{at}: unknown op {op}"),
                }
            }
            let sessions: BTreeSet<_> = record["sessions"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            assert_eq!(sessions, connected, "{at}");
            for (id, state) in record["sessions"].as_object().unwrap() {
                assert_eq!(state["active"], !closed.contains(id), "{at}: {id}");
            }

            // Player indices and names are unique, and the lobby holds only listed players.
            let players = record["players"].as_array().unwrap();
            let indices: BTreeSet<_> = players
                .iter()
                .map(|p| p["index"].as_u64().unwrap())
                .collect();
            let player_names: BTreeSet<_> = players
                .iter()
                .map(|p| p["name"].as_str().unwrap())
                .collect();
            assert_eq!(indices.len(), players.len(), "{at}");
            assert_eq!(player_names.len(), players.len(), "{at}");
            assert!(indices.iter().all(|&index| index >= 1), "{at}");
            // A player reports the repository slot it occupies.
            assert!(players.iter().all(|p| p["slot"] == p["index"]), "{at}");
            for name in record["lobby"].as_array().unwrap() {
                assert!(player_names.contains(name.as_str().unwrap()), "{at}");
            }

            // Every byte sent is attributed to a session that exists, and frames parse.
            for (id, entries) in record["out"].as_object().unwrap() {
                assert!(
                    connected.contains(id),
                    "{at}: bytes for unknown session {id}"
                );
                for entry in entries.as_array().unwrap() {
                    let bytes = hex(entry["hex"].as_str().unwrap());
                    assert!(!bytes.is_empty(), "{at}");
                    // After its connection closed the original still queues, and sends nothing.
                    let is_dropped = entry.get("dropped").is_some();
                    assert_eq!(is_dropped, closed.contains(id), "{at}: {id}");
                    if is_dropped {
                        dropped += 1;
                    } else {
                        bytes_out += bytes.len();
                    }
                    match entry["k"].as_str().unwrap() {
                        "handshake" => {
                            assert_eq!((bytes.len(), bytes[0]), (9, 0), "{at}");
                            assert!(!logged_in.contains(id), "{at}");
                        }
                        "login" => {
                            replies += 1;
                            assert_eq!(entry["response"], "Success", "{at}");
                            assert_eq!((bytes.len(), bytes[0]), (12, 2), "{at}");
                            // The reply carries the index the repository lists for that player.
                            let index = u64::from(u16::from_be_bytes([bytes[8], bytes[9]]));
                            let listed = players
                                .iter()
                                .find(|p| p["name"] == id.as_str())
                                .unwrap_or_else(|| panic!("{at}: no player for {id}"));
                            assert_eq!(listed["index"], index, "{at}");
                            assert!(logged_in.insert(id.clone()), "{at}: second reply");
                        }
                        "packet" => {
                            // Game packets only follow that session's login reply.
                            assert!(logged_in.contains(id), "{at}: packet before login");
                            let opcode = entry["op"].as_u64().unwrap();
                            assert!(opcode <= 255 && u64::from(bytes[0]) == opcode, "{at}");
                            match entry["size"].as_str().unwrap() {
                                "NORMAL" => {}
                                "BYTE" => {
                                    assert_eq!(usize::from(bytes[1]), bytes.len() - 2, "{at}")
                                }
                                "SHORT" => assert_eq!(
                                    usize::from(u16::from_be_bytes([bytes[1], bytes[2]])),
                                    bytes.len() - 3,
                                    "{at}"
                                ),
                                size => panic!("{at}: size {size}"),
                            }
                            if !is_dropped {
                                packets += 1;
                            }
                        }
                        kind => panic!("{at}: unknown entry kind {kind}"),
                    }
                }
            }

            // Nothing left the run's directory, opened a socket or arrived from an unpinned thread.
            assert!(record["refused"].as_array().unwrap().is_empty(), "{at}");
            assert_eq!(record["unpinned"], 0, "{at}");
            assert!(record["handoffs"]["foreign"].as_array().unwrap().is_empty());
            assert_eq!(record["stderrLines"], 0, "{at}");

            for file in record["files"].as_array().unwrap() {
                let path = file["path"].as_str().unwrap();
                if let Some(text) = file.get("hex") {
                    let bytes = hex(text.as_str().unwrap());
                    assert!(path.starts_with("data/players/"), "{at}: {path}");
                    assert_eq!(file["size"], bytes.len(), "{at}");
                    assert_eq!(file["sha256"], sha256(&bytes), "{at}");
                    // A character save is one JSON document.
                    serde_json::from_slice::<Value>(&bytes).expect("save is JSON");
                    saves.insert(path.to_string(), bytes);
                } else {
                    assert!(!path.starts_with("data/players/"), "{at}: {path}");
                }
            }
        }

        // Every character that logged in was saved once its connection closed, under its own name.
        let expected: BTreeSet<_> = logged_in
            .iter()
            .map(|id| format!("data/players/{id}.json"))
            .collect();
        assert_eq!(
            saves.keys().cloned().collect::<BTreeSet<_>>(),
            expected,
            "{name}"
        );
        assert_eq!(closed, connected, "{name}");
        assert!(
            scenario.records.last().unwrap()["players"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p["artificial"] == true)
        );
        census.push(serde_json::json!({
            "name": name, "records": scenario.records.len(), "ticks": ticks, "sessions": connected.len(),
            "packets": packets, "loginReplies": replies, "bytesOut": bytes_out,
            "droppedAfterClose": dropped, "savesWritten": saves.len(),
        }));
    }
    // The counts the provenance quotes are the counts in the fixture.
    assert_eq!(provenance["scenarios"], Value::Array(census));
}

#[test]
fn returning_character_starts_from_the_recorded_save() {
    let all = scenarios();
    let returning = all
        .iter()
        .find(|s| s.header["name"] == "returning-character")
        .unwrap();
    assert_eq!(returning.header["filesFrom"], "new-character");
    // Same script as the new character, different world: the first tick's output differs.
    let fresh = all
        .iter()
        .find(|s| s.header["name"] == "new-character")
        .unwrap();
    assert_eq!(fresh.records.len(), returning.records.len());
    let first_tick = |s: &Scenario| {
        s.records
            .iter()
            .find(|r| r["t"] == "tick")
            .map(|r| r["out"].clone())
            .unwrap()
    };
    assert_ne!(first_tick(fresh), first_tick(returning));
}
