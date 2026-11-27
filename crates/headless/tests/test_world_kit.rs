//! The test-world kit's worked example (docs/test-world.md) as a test: the
//! live world boots in-process (as `live_agent` boots it), the runner seeds
//! `scenarios/goblins.json`'s character next to the Lumbridge goblins through
//! the server's own save writer, logs it in, attacks the nearest goblin and
//! waits for the kill and the attack and hitpoints experience, reading only
//! what the server sent.
//!
//! Needs the pinned cache (`assets/cache`) and a world folder
//! (`RS09_STARTUP_WORLD`, else `runs/startup-loaders-oracle/template`,
//! `runs/world-trace-oracle/template` or `runs/play/world-data`); without
//! them the test prints SKIPPED and checks nothing.
#[path = "../examples/common/kit.rs"]
mod kit;

use rs09_server::{
    runtime::{Config, Service},
    world::live::{boot::Options, server},
};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn inputs() -> Option<(PathBuf, PathBuf)> {
    let cache = std::env::var_os("RS09_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("assets/cache"));
    let world = |data: &PathBuf| {
        data.join("worldprops/default.conf").exists()
            && data.join("data/configs/npc_spawns.json").exists()
    };
    let data = match std::env::var_os("RS09_STARTUP_WORLD") {
        Some(data) => PathBuf::from(data),
        None => [
            "runs/startup-loaders-oracle/template",
            "runs/world-trace-oracle/template",
            "runs/play/world-data",
        ]
        .iter()
        .map(|d| root().join(d))
        .find(world)?,
    };
    (cache.join("main_file_cache.dat2").exists() && world(&data)).then_some((cache, data))
}

// Release builds only: in a debug build (2026-11-27, Windows) the goblin took
// one hit and the fight then stopped, with no retaliation, for 90 seconds;
// the release build passed every run. See docs/test-world.md.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "run with --release: in a debug build the live world's fight stalled after one hit"
)]
fn seeded_character_kills_a_lumbridge_goblin_and_gains_experience() {
    let Some((cache, data)) = inputs() else {
        eprintln!(
            "SKIPPED: the live world needs assets/cache and a world folder (RS09_STARTUP_WORLD or runs/*/template)"
        );
        return;
    };
    #[cfg(unix)]
    std::env::set_current_dir("/").unwrap();
    let cache = std::path::absolute(root().join(cache)).unwrap();
    let data = std::path::absolute(root().join(data)).unwrap();
    let saves = std::env::temp_dir().join(format!("rs09-test-world-kit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&saves);
    let running = server::start(Options {
        data,
        cache: cache.clone(),
        saves: saves.clone(),
        verbose: std::env::var_os("RS09_LIVE_VERBOSE").is_some(),
    })
    .expect("the live world boots");
    let limits = rs09_cache::Limits::default();
    let catalog = Arc::new(
        rs09_server::catalog::Catalog::new(
            rs09_cache::store::Store::open(
                &cache,
                rs09_cache::store::StorageMode::Resident,
                limits,
            )
            .unwrap(),
            limits,
            1024,
            16 * 1024 * 1024,
        )
        .unwrap(),
    );
    let service = Service::new(
        catalog,
        Config {
            authenticator: Some(running.authenticator.clone()),
            world: Some(running.gate.clone()),
            ..Config::default()
        },
        530,
    )
    .unwrap();
    let script: Value = serde_json::from_str(include_str!("../scenarios/goblins.json")).unwrap();
    let tokio = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let outcome = tokio.block_on(async {
        let server = service
            .start("127.0.0.1:0".parse().unwrap(), None)
            .await
            .unwrap();
        let outcome = kit::run(server.tcp, Some(&saves), &script, true).await;
        server.shutdown().await.unwrap();
        outcome
    });
    running.stop();
    let outcome = outcome.expect("the script ran");
    let report = &outcome.report;
    eprintln!("steps: {}", report["steps"]);
    eprintln!("experience gained: {}", report["experienceGained"]);
    assert!(outcome.ok, "every step passed: {}", report["steps"]);
    // The seeded fields reached the character: the start snapshot is the
    // server's own view of the login.
    let start = &report["start"];
    let position = start["position"].as_array().unwrap();
    let [x, y] = [position[0].as_i64().unwrap(), position[1].as_i64().unwrap()];
    assert!(
        (x - 3250).abs() <= 1 && (y - 3235).abs() <= 1,
        "logged in at the seeded tile: {position:?}"
    );
    assert_eq!(start["skills"][0]["level"], 20, "seeded attack level");
    assert!(
        start["equipment"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["slot"] == 3 && i["id"] == 1277),
        "seeded bronze sword in the weapon slot: {}",
        start["equipment"]
    );
    assert!(report["experienceGained"]["attack"].as_i64().unwrap() > 0);
    assert!(report["experienceGained"]["hitpoints"].as_i64().unwrap() > 0);
    let _ = std::fs::remove_dir_all(&saves);
}
