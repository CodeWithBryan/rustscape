//! Downloads, verifies or repairs a cache directory from a game server's JS5
//! update service, the way the original client does at start-up.
//!
//! `cargo run --release -p rs09-client --example fetch_cache -- HOST:PORT DIR
//!     [--original-archives] [--original-pace] [--alternate-port PORT]
//!     [--stall SECONDS]`
//!
//! DIR is created if needed. Groups already stored there are kept when their
//! CRC and version match the server's indexes, so running it again on a
//! complete directory downloads nothing and running it on a damaged one
//! fetches only what is wrong.
//!
//! By default every archive the server lists is filled, which is more than
//! the original client does on its own: it leaves archives 6 and 14 to be
//! fetched on demand and knows nothing of archives past 27.
//! `--original-archives` keeps to that (and, on a fresh directory, ends with
//! the read-back reporting those two archives' groups missing). `--original-pace` keeps to the
//! original's 100 socket reads per 20 ms cycle instead of reading as fast as
//! data arrives.
//!
//! Prints progress to stderr and one JSON report to stdout: what the updater
//! did, then the result of reading the whole directory back through the
//! read-only cache reader and re-checking every group against its index.
//! Exits non-zero if the update failed or that re-check found a group
//! missing or wrong.
use rs09_client::cache_update::{self, Options, Snapshot};
use serde_json::json;
use std::{error::Error, path::PathBuf, time::Duration, time::Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|argument| argument == name);
    let option = |name: &str| {
        args.iter()
            .position(|argument| argument == name)
            .and_then(|position| args.get(position + 1))
            .cloned()
    };
    let usage = "usage: fetch_cache HOST:PORT DIR [--original-archives] [--original-pace] \
                 [--alternate-port PORT] [--stall SECONDS]";
    let (Some(server), Some(directory)) = (args.first(), args.get(1)) else {
        return Err(usage.into());
    };
    if server.starts_with("--") || directory.starts_with("--") {
        return Err(usage.into());
    }
    let (host, port) = server.rsplit_once(':').ok_or(usage)?;
    let host = host.trim_matches(['[', ']']);
    let port: u16 = port.parse()?;
    let directory = PathBuf::from(directory);
    let mut options = Options {
        complete: !flag("--original-archives"),
        fast: !flag("--original-pace"),
        ..Options::default()
    };
    if let Some(alternate) = option("--alternate-port") {
        options.alternate_port = Some(alternate.parse()?);
    }
    if let Some(stall) = option("--stall") {
        options.stall = Duration::from_secs(stall.parse()?);
    }

    let started = Instant::now();
    let mut last_report = Instant::now();
    let mut last_text = String::new();
    let mut progress = |snapshot: &Snapshot| {
        let text_changed = snapshot.text != last_text && !snapshot.loaded;
        if text_changed || last_report.elapsed() >= Duration::from_secs(2) {
            eprintln!(
                "{:7.1}s  {:3}%  {:<34} groups {}/{}  {:.1} MB  errors {}",
                started.elapsed().as_secs_f64(),
                snapshot.percentage,
                if snapshot.loaded {
                    "Background download"
                } else {
                    &snapshot.text
                },
                snapshot.groups_verified,
                snapshot.groups,
                snapshot.bytes_received as f64 / 1_048_576.0,
                snapshot.errors,
            );
            last_report = Instant::now();
            last_text.clone_from(&snapshot.text);
        }
    };
    let outcome = cache_update::run(host, port, &directory, &options, &mut progress);
    let update = match &outcome {
        Ok(report) => json!({
            "ok": true,
            "cycles": report.cycles,
            "seconds": report.elapsed.as_secs_f64(),
            "secondsToLoginScreen": report.loaded_after.as_secs_f64(),
            "bytesReceived": report.bytes_received,
            "repliesReceived": report.replies_received,
            "groupsListed": report.groups,
            "groupsVerifiedByUpdater": report.groups_verified,
            "dataFileBytes": report.data_length,
            "loginChecksums": report.checksums.to_vec(),
        }),
        Err(error) => json!({"ok": false, "error": error.to_string()}),
    };
    // Independent of the updater: the directory as the read-only reader sees it.
    let check_started = Instant::now();
    let archives = cache_update::verify_directory(&directory)?;
    let (mut groups, mut verified, mut bytes) = (0usize, 0usize, 0u64);
    let mut detail = Vec::new();
    for archive in &archives {
        groups += archive.groups;
        verified += archive.verified;
        bytes += archive.stored_bytes;
        detail.push(json!({
            "archive": archive.archive,
            "indexCrc": archive.index_crc,
            "indexVersion": archive.index_version,
            "groups": archive.groups,
            "verified": archive.verified,
            "missing": archive.missing.len(),
            "crcMismatch": archive.crc_mismatch.len(),
            "versionMismatch": archive.version_mismatch.len(),
            "firstProblems": archive.missing.iter().chain(&archive.crc_mismatch)
                .chain(&archive.version_mismatch).take(8).collect::<Vec<_>>(),
        }));
    }
    let complete = !archives.is_empty() && archives.iter().all(|archive| archive.complete());
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "server": server,
            "directory": directory,
            "options": {
                "everyListedArchive": options.complete,
                "originalPace": !options.fast,
                "alternatePort": options.alternate_port,
                "stallSeconds": options.stall.as_secs(),
            },
            "update": update,
            "readBack": {
                "what": "every group of every stored index, CRC and version trailer, through rs09_cache::store::Store",
                "seconds": check_started.elapsed().as_secs_f64(),
                "archives": archives.len(),
                "groups": groups,
                "verified": verified,
                "storedBytes": bytes,
                "complete": complete,
                "detail": detail,
            },
        }))?
    );
    outcome?;
    if !complete {
        return Err(format!(
            "{} of {groups} groups are missing or wrong",
            groups - verified
        )
        .into());
    }
    Ok(())
}
