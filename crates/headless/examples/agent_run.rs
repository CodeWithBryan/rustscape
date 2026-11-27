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
//! when a wait or assertion failed, 2 on an error (bad script, no server).
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
                }
                "--address" => {
                    address = Some(
                        args.next()
                            .ok_or("missing --address IP:PORT")?
                            .parse()
                            .map_err(|e| format!("{e}"))?,
                    )
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
    match runtime.block_on(kit::run(address, saves.as_deref(), &script, seed)) {
        Ok(outcome) => {
            let text = serde_json::to_string_pretty(&outcome.report).unwrap();
            println!("{text}");
            if let Some(path) = output
                && let Err(e) = std::fs::write(&path, &text)
            {
                eprintln!("{}: {e}", path.display());
            }
            if outcome.ok {
                ExitCode::SUCCESS
            } else {
                eprintln!("FAILED: a wait or assertion did not hold");
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
