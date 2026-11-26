//! The headless agent against the live world (`rs09-server --world`, started
//! in-process exactly as `crates/server/tests/live_world.rs` starts it).
//!
//! Every action goes out through `rs09_headless::agent::Action` and every
//! check reads `agent::View`, the state an agent would see:
//!
//! - fight: a character with attack and strength 40 at Lumbridge's chicken
//!   pen attacks the nearest chicken; the view shows hit marks with health
//!   bars on it, its death, and hitpoints experience;
//! - talk: a character in front of Draynor bank talks to the banker, reads
//!   the NPC's line, continues, picks the "access" option by its text,
//!   continues its own line, and the bank interface (762) opens;
//! - use: a dagger used on a bowl answers "Nothing interesting happens."
//!   and an item option (Wield) moves the dagger to the weapon slot.
//!
//! Use on an NPC or scenery is checked byte for byte against the server's
//! decoder in the unit tests only: the live world refuses the content
//! use-with walk (`UseWithHandler`'s MovementPulse), so it shows no outcome.
//!
//! Needs the pinned cache (`assets/cache`, `cargo xtask cache-fetch`) and the
//! world data (`runs/startup-loaders-oracle/template`, or `RS09_STARTUP_WORLD`);
//! without them the test prints SKIPPED and checks nothing.
use rs09_headless::{
    agent::{Action, Agent, DialogueKind, EQUIPMENT, Event, Held, View},
    session::Session,
};
use rs09_server::{
    runtime::{Config, Service},
    world::live::{boot::Options, server},
};
use serde_json::Value;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn inputs() -> Option<(PathBuf, PathBuf)> {
    let cache = std::env::var_os("RS09_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("assets/cache"));
    let data = std::env::var_os("RS09_STARTUP_WORLD")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("runs/startup-loaders-oracle/template"));
    (cache.join("main_file_cache.dat2").exists()
        && data.join("worldprops/default.conf").exists()
        && data.join("data/configs/npc_spawns.json").exists())
    .then_some((cache, data))
}

const PASSWORD: &str = "agenttest";
const LIMIT: Duration = Duration::from_secs(20);

async fn enter(address: SocketAddr, name: &str) -> Agent {
    let session = Session::connect(address, false, name, PASSWORD)
        .await
        .unwrap_or_else(|e| panic!("{name}'s login: {e}"));
    let mut agent = Agent::new(session.into_managed());
    agent
        .until(LIMIT, "the scene and a player update", |v| {
            v.position().is_some()
        })
        .await
        .unwrap();
    agent
}

async fn leave(agent: Agent) {
    agent.close().await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// A save made from the seed character's: location, inventory and an edit.
fn write_save(
    saves: &Path,
    name: &str,
    at: [i32; 3],
    inventory: &[(i32, i32, i32)],
    edit: impl FnOnce(&mut Value),
) {
    let players = saves.join("players");
    let mut save: Value =
        serde_json::from_str(&std::fs::read_to_string(players.join("agentseed.json")).unwrap())
            .unwrap();
    save["core_data"]["location"] = Value::String(format!("{},{},{}", at[0], at[1], at[2]));
    save["core_data"]["inventory"] = inventory
        .iter()
        .map(|(slot, id, amount)| {
            serde_json::json!({"slot": slot.to_string(), "id": id.to_string(),
                "amount": amount.to_string(), "charge": "1000"})
        })
        .collect();
    edit(&mut save);
    std::fs::write(
        players.join(format!("{name}.json")),
        serde_json::to_string(&save).unwrap(),
    )
    .unwrap();
}

fn hit_on(v: &View, index: i32) -> bool {
    v.events()
        .any(|e| matches!(e, Event::NpcHit { index: i, .. } if *i == index))
}

async fn fight(address: SocketAddr, saves: &Path) {
    write_save(saves, "agentfight", [3232, 3294, 0], &[], |save| {
        for skill in [0, 2] {
            save["skills"][skill]["static"] = Value::String("40".into());
            save["skills"][skill]["dynamic"] = Value::String("40".into());
            save["skills"][skill]["experience"] = Value::String("37224.0".into());
        }
    });
    let mut a = enter(address, "agentfight").await;
    a.until(LIMIT, "chickens", |v| {
        v.nearest_npc(41).or(v.nearest_npc(1017)).is_some()
    })
    .await
    .unwrap();
    let hitpoints_xp = a.view.skill(3).unwrap().experience;
    let here = a.view.position().unwrap();
    let mut chickens: Vec<_> = a
        .view
        .npcs()
        .into_iter()
        .filter(|n| n.type_id == 41 || n.type_id == 1017)
        .collect();
    chickens.sort_by_key(|n| (n.tile[0] - here[0]).abs() + (n.tile[1] - here[1]).abs());
    a.view.drain_events();
    // A chicken the pen's fence keeps out of reach is skipped for the next.
    let mut target = None;
    for chicken in chickens.into_iter().take(4) {
        a.act(&Action::Attack {
            index: chicken.index,
        })
        .await
        .unwrap();
        if a.until(LIMIT, "a hit mark", |v| hit_on(v, chicken.index))
            .await
            .is_ok()
        {
            target = Some(chicken);
            break;
        }
    }
    let chicken = target.expect("a hit mark on one of the four nearest chickens");
    a.until(Duration::from_secs(90), "the chicken's death", |v| {
        v.events().any(
            |e| matches!(e, Event::NpcRemoved { index, died: true, .. } if *index == chicken.index),
        )
    })
    .await
    .unwrap();
    let (events, _) = a.view.drain_events();
    let hits: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::NpcHit {
                index,
                damage,
                health,
                ..
            } if *index == chicken.index => Some((*damage, *health)),
            _ => None,
        })
        .collect();
    eprintln!(
        "agentfight: attacked chicken {} ({}) at {:?}; hits (damage, health bar) {hits:?}",
        chicken.index, chicken.type_id, chicken.tile
    );
    assert!(hits.iter().map(|h| h.0).sum::<i32>() > 0, "damage dealt");
    assert_eq!(hits.last().unwrap().1, Some(0), "the last bar is empty");
    assert!(
        a.view.skill(3).unwrap().experience > hitpoints_xp,
        "hitpoints experience from the hits"
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Experience {
            skill: 3,
            gained,
            ..
        } if *gained > 0
    )));
    leave(a).await;
}

async fn talk(address: SocketAddr, saves: &Path) {
    write_save(saves, "agenttalk", [3092, 3243, 0], &[], |_| {});
    let mut a = enter(address, "agenttalk").await;
    let banker = |v: &View| {
        v.npcs()
            .into_iter()
            .find(|n| n.type_id == 494 && n.tile == [3090, 3243, 0])
    };
    a.until(LIMIT, "the banker behind the booth", |v| {
        banker(v).is_some()
    })
    .await
    .unwrap();
    let index = banker(&a.view).unwrap().index;
    a.act(&Action::TalkTo { index }).await.unwrap();
    a.until(LIMIT, "the banker's greeting", |v| {
        v.dialogue()
            .is_some_and(|d| matches!(d.kind, DialogueKind::Npc { .. }))
    })
    .await
    .unwrap();
    let d = a.view.dialogue().unwrap();
    eprintln!("agenttalk: {:?} says {:?}", d.title, d.lines);
    assert_eq!(d.kind, DialogueKind::Npc { npc: Some(494) });
    assert!(d.lines[0].contains("Good day, would you like to access your bank account?"));
    a.act(&d.resume().unwrap()).await.unwrap();
    a.until(LIMIT, "the banker's options", |v| {
        v.dialogue()
            .is_some_and(|d| d.kind == DialogueKind::Options)
    })
    .await
    .unwrap();
    let d = a.view.dialogue().unwrap();
    eprintln!("agenttalk: options {:?}", d.lines);
    a.act(&d.choose_text("access my bank account").unwrap())
        .await
        .unwrap();
    a.until(LIMIT, "the player's spoken option", |v| {
        v.dialogue().is_some_and(|d| d.kind == DialogueKind::Player)
    })
    .await
    .unwrap();
    let d = a.view.dialogue().unwrap();
    assert!(d.lines[0].contains("I'd like to access my bank account please."));
    assert_eq!(d.title.as_deref(), Some("agenttalk"), "the player's name");
    a.act(&d.resume().unwrap()).await.unwrap();
    a.until(LIMIT, "the bank interface", |v| {
        v.open_interfaces().any(|i| i == 762)
    })
    .await
    .unwrap();
    // The chatbox component is closed right after the bank's own packets.
    a.until(LIMIT, "the chatbox closed", |v| v.dialogue().is_none())
        .await
        .unwrap();
    eprintln!("agenttalk: the conversation opened the bank");
    a.act(&Action::CloseInterface).await.unwrap();
    leave(a).await;
}

async fn use_items(address: SocketAddr, saves: &Path) {
    write_save(
        saves,
        "agentuse",
        [3208, 3214, 0],
        &[(0, 1205, 1), (5, 1923, 1)],
        |_| {},
    );
    let mut a = enter(address, "agentuse").await;
    a.until(LIMIT, "the inventory", |v| v.held(1923).is_some())
        .await
        .unwrap();
    let dagger = a.view.held(1205).unwrap();
    let bowl = a.view.held(1923).unwrap();
    assert_eq!(
        (dagger, bowl),
        (Held { id: 1205, slot: 0 }, Held { id: 1923, slot: 5 })
    );
    a.view.drain_events();
    a.act(&Action::UseOnItem {
        used: dagger,
        target: bowl,
    })
    .await
    .unwrap();
    a.until(LIMIT, "the unhandled use-with message", |v| {
        v.events()
            .any(|e| *e == Event::Message("Nothing interesting happens.".into()))
    })
    .await
    .unwrap();
    eprintln!("agentuse: dagger on bowl: \"Nothing interesting happens.\"");
    a.act(&Action::ItemOption {
        option: 1,
        item: dagger,
    })
    .await
    .unwrap();
    a.until(LIMIT, "the dagger in the weapon slot", |v| {
        v.container(EQUIPMENT)
            .get(3)
            .copied()
            .flatten()
            .is_some_and(|i| i.id == 1205)
            && v.held(1205).is_none()
    })
    .await
    .unwrap();
    eprintln!(
        "agentuse: wielded the dagger: {}",
        a.view.snapshot()["equipment"]
    );
    leave(a).await;
}

#[test]
fn headless_agent_fights_talks_and_uses_items_in_the_live_world() {
    let Some((cache, data)) = inputs() else {
        eprintln!(
            "SKIPPED: the live world needs assets/cache and runs/startup-loaders-oracle/template (or RS09_CACHE / RS09_STARTUP_WORLD)"
        );
        return;
    };
    // The config's parsePath drops a path's leading separator (as the original
    // does), so on Unix the world's absolute paths resolve against the working
    // directory: run from the root, where they mean what they say.
    #[cfg(unix)]
    std::env::set_current_dir("/").unwrap();
    let cache = root().join(cache).canonicalize().unwrap();
    let data = root().join(data).canonicalize().unwrap();
    let saves = std::env::temp_dir().join(format!("rs09-live-agent-{}", std::process::id()));
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
    let tokio = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    tokio.block_on(async {
        let server = service
            .start("127.0.0.1:0".parse().unwrap(), None)
            .await
            .unwrap();
        let address = server.tcp;
        // A new character, whose save the scenarios' characters are made from.
        let seed = enter(address, "agentseed").await;
        let snapshot = seed.view.snapshot();
        eprintln!("agentseed: {snapshot}");
        assert!(snapshot["hitpoints"].as_i64().is_some_and(|h| h > 0));
        leave(seed).await;
        let saved = saves.join("players/agentseed.json");
        for _ in 0..50 {
            if saved.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert!(saved.exists(), "the seed character's save");
        fight(address, &saves).await;
        talk(address, &saves).await;
        use_items(address, &saves).await;
        server.shutdown().await.unwrap();
    });
    running.stop();
    let _ = std::fs::remove_dir_all(&saves);
}
