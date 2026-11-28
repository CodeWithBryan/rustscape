//! Log a seeded character in to a running `rs09-server --world`, run a JSON
//! script of agent steps and print the result as JSON (docs/test-world.md).
//!
//! ```text
//! cargo run --release -p rs09-headless --example agent_run -- --world runs/test-world/NAME SCRIPT.json
//! cargo run --release -p rs09-headless --example agent_run -- --address 127.0.0.1:PORT --saves DIR SCRIPT.json
//! ```
//!
//! `--world DIR` reads the address and saves folder from the `world.json` of
//! `cargo xtask test-world start`. `--no-seed` logs in an existing character
//! without writing its save. Progress goes to stderr, the report to stdout
//! (`--output FILE` writes it there too). Exit 0 when every step passed, 1
//! when a wait or assertion failed, 2 on an error (bad script, no server, a
//! step's error, the logout save not written); the report is printed whenever
//! the character logged in. On a failure with `--world`, the last 40 lines of
//! the world's `server.log` follow on stderr.
//!
//! Port 43594 (the play world) is refused, and seeding is refused unless the
//! saves folder is a test world's (under `runs/test-world/`, or beside a
//! `world.json`): seeding replaces the character's save.
#[path = "common/kit.rs"]
mod kit;

use serde_json::Value;
use std::{net::SocketAddr, path::PathBuf, process::ExitCode};

fn usage() -> ExitCode {
    eprintln!(
        "agent_run (--world DIR | --address IP:PORT [--saves DIR]) [--no-seed] [--output FILE] SCRIPT.json"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut address: Option<SocketAddr> = None;
    let mut saves: Option<PathBuf> = None;
    let mut script: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut seed = true;
    let mut server_log: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let result: Result<(), String> = (|| {
            match arg.as_str() {
                "--world" => {
                    let dir = PathBuf::from(args.next().ok_or("missing --world DIR")?);
                    let text = std::fs::read_to_string(dir.join("world.json"))
                        .map_err(|e| format!("{}: {e}", dir.join("world.json").display()))?;
                    let world: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
                    address = Some(
                        world["address"]
                            .as_str()
                            .ok_or("world.json has no address")?
                            .parse()
                            .map_err(|e| format!("{e}"))?,
                    );
                    saves = world["saves"].as_str().map(PathBuf::from);
                    server_log = world["log"].as_str().map(PathBuf::from);
                }
                "--address" => {
                    let parsed: SocketAddr = args
                        .next()
                        .ok_or("missing --address IP:PORT")?
                        .parse()
                        .map_err(|e| format!("{e}"))?;
                    if parsed.port() == kit::PLAY_PORT {
                        return Err(format!(
                            "port {} is the play world's; drive a test world (cargo xtask test-world start)",
                            kit::PLAY_PORT
                        ));
                    }
                    address = Some(parsed);
                }
                "--saves" => saves = Some(args.next().ok_or("missing --saves DIR")?.into()),
                "--output" => output = Some(args.next().ok_or("missing --output FILE")?.into()),
                "--no-seed" => seed = false,
                other if other.starts_with("--") => return Err(format!("unknown option {other}")),
                other => script = Some(other.into()),
            }
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("{e}");
            return usage();
        }
    }
    let (Some(address), Some(script)) = (address, script) else {
        return usage();
    };
    let text = match std::fs::read_to_string(&script) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("{}: {e}", script.display());
            return ExitCode::from(2);
        }
    };
    let script: Value = match serde_json::from_str(&text) {
        Ok(script) => script,
        Err(e) => {
            eprintln!("{}: {e}", script.display());
            return ExitCode::from(2);
        }
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    let result = runtime.block_on(kit::run(address, saves.as_deref(), &script, seed));
    let code = match result {
        Ok(outcome) => {
            let text = serde_json::to_string_pretty(&outcome.report).unwrap();
            println!("{text}");
            if let Some(path) = output
                && let Err(e) = std::fs::write(&path, &text)
            {
                eprintln!("{}: {e}", path.display());
            }
            match (&outcome.error, outcome.ok) {
                (Some(error), _) => {
                    eprintln!("error: {error}");
                    ExitCode::from(2)
                }
                (None, true) => return ExitCode::SUCCESS,
                (None, false) => {
                    eprintln!("FAILED: a wait or assertion did not hold");
                    ExitCode::from(1)
                }
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    };
    if let Some(log) = server_log {
        tail(&log, 40);
    }
    code
}

/// The last `lines` lines of the world's server log, to stderr.
fn tail(log: &std::path::Path, lines: usize) {
    match std::fs::read(log) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let all: Vec<&str> = text.lines().collect();
            eprintln!(
                "--- last {} lines of {} ---",
                lines.min(all.len()),
                log.display()
            );
            for line in &all[all.len().saturating_sub(lines)..] {
                eprintln!("{line}");
            }
        }
        Err(e) => eprintln!("{}: {e}", log.display()),
    }
}
