//! The live world (`rs09-server --world`, world::live) end to end.
//!
//! The world is started in-process exactly as `--world` starts it (the
//! native start-up over the pinned world data, the 600 ms worker, the saver
//! thread) behind the native transport (`runtime::Service`). Headless
//! clients use the native protocol code (`rs09_net::login::LoginPeer`, the
//! game frame decoder and the client-side player update decoder) over TCP:
//! a new character logs in, ticks, walks; a second character logs in and
//! each sees the other; both log out; their saves are in the saves directory
//! and the first character logs in again from its save.
//!
//! The login tick of the first character is compared, byte for byte and in
//! order, with the world-trace fixture's new-character login tick after the
//! entries of the calls this world answers as not ported (pinned below by
//! marker and count) are taken out of the original's list.
//!
//! Needs the pinned cache (`assets/cache`) and the world data
//! (`runs/startup-loaders-oracle/template`, or `RS09_STARTUP_WORLD`);
//! without them the test prints SKIPPED and checks nothing.
use rs09_net::{Chunk, Reader, Writer, login::LoginPeer};
use rs09_protocol::{
    game::{self, Direction, Frame, actions, npcs, players, updates},
    login::{LoginRequest, RsaKey},
    profile::RS09_REVISION_530,
};
use rs09_server::{
    runtime::{Config, Service},
    world::live::{boot::Options, server},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tokio::time::timeout;

const TRACE: &str = include_str!("../../../fixtures/world-trace/trace.jsonl");
const ORIGINS: &str = include_str!("../../../fixtures/world-login/origins.jsonl");

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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A headless client: the native login peer, then game frames.
struct Client {
    name: String,
    index: i32,
    reader: Reader,
    writer: Writer,
    decoder: game::Decoder,
    frames: Vec<Frame>,
    players: players::State,
    /// (frame number, update) of every player update decoded.
    updates: Vec<(usize, players::Update)>,
    /// Absolute coordinates of the last rebuild's scene base.
    base: Option<[i32; 3]>,
    closed: bool,
    /// The client-side NPC list from the NPC updates (32).
    npcs: npcs::State,
    npc_updates: usize,
}

impl Client {
    async fn login(address: std::net::SocketAddr, name: &str) -> (i32, Option<Client>) {
        let mut peer = LoginPeer::connect(address, false, name.as_bytes())
            .await
            .expect("login handshake");
        let reply = peer
            .submit(
                LoginRequest {
                    username: name.as_bytes(),
                    password: b"livetest",
                    window_mode: 2,
                    screen_width: 765,
                    screen_height: 503,
                    ..LoginRequest::default()
                },
                RS09_REVISION_530,
                &RsaKey::reference_public(),
            )
            .await
            .expect("login reply");
        if reply.code != 2 {
            return (reply.code as i32, None);
        }
        let index = reply.world.as_ref().expect("a world reply").player_index as i32;
        let (reader, writer, pending) = peer.into_transport();
        let mut decoder = game::Decoder::new(Direction::ServerToClient, 1 << 20, 65535);
        decoder.push(&pending).unwrap();
        (
            2,
            Some(Client {
                name: name.into(),
                index,
                reader,
                writer,
                decoder,
                frames: Vec::new(),
                players: players::State::new(0, 0),
                updates: Vec::new(),
                base: None,
                closed: false,
                npcs: npcs::State::default(),
                npc_updates: 0,
            }),
        )
    }

    fn take_frames(&mut self) {
        while let Some(frame) = self.decoder.next_frame().expect("a server frame") {
            let n = self.frames.len();
            match frame.opcode {
                162 => {
                    let (base, local) = rebuild_base(&frame.payload);
                    self.base = Some(base);
                    self.players = players::State::new(local[0], local[1]);
                }
                225 => {
                    let update = self
                        .players
                        .decode_frame(&frame, n as i32)
                        .unwrap_or_else(|e| panic!("{}: player update {n}: {e:?}", self.name));
                    self.updates.push((n, update));
                }
                32 => {
                    let tile = self.players.self_player().tile();
                    self.npcs
                        .decode(&frame, tile, n as i32)
                        .unwrap_or_else(|e| panic!("{}: NPC update {n}: {e:?}", self.name));
                    self.npc_updates += 1;
                }
                _ => {}
            }
            self.frames.push(frame);
        }
    }

    /// Reads for `duration`.
    async fn pump(&mut self, duration: Duration) {
        let end = Instant::now() + duration;
        self.take_frames();
        while !self.closed {
            let Some(left) = end.checked_duration_since(Instant::now()) else {
                break;
            };
            match timeout(left, self.reader.read()).await {
                Err(_) => break,
                Ok(Ok(Chunk::Binary(bytes))) => {
                    self.decoder.push(&bytes).unwrap();
                    self.take_frames();
                }
                Ok(_) => self.closed = true,
            }
        }
    }

    /// Reads until `done` holds, or panics after `limit`.
    async fn until(&mut self, limit: Duration, what: &str, done: impl Fn(&Client) -> bool) {
        let end = Instant::now() + limit;
        while !done(self) {
            assert!(
                Instant::now() < end,
                "{}: timed out waiting for {what}",
                self.name
            );
            self.pump(Duration::from_millis(200)).await;
        }
    }

    async fn send(&mut self, frame: Frame) {
        let mut wire = vec![];
        frame
            .write(Direction::ClientToServer, None, &mut wire)
            .unwrap();
        self.writer.write(bytes::Bytes::from(wire)).await.unwrap();
    }

    /// The local tile the client's own player stands on, as the absolute
    /// world coordinates the rebuild's base gives.
    fn position(&self) -> [i32; 3] {
        let base = self.base.expect("a rebuild");
        let [x, y] = self.players.self_player().tile();
        [base[0] + x, base[1] + y, base[2]]
    }
}

/// UpdateSceneGraph (162): the absolute scene base and the local position.
fn rebuild_base(payload: &[u8]) -> ([i32; 3], [i32; 2]) {
    let p2add = |hi: u8, lo: u8| (i32::from(hi) << 8) | i32::from(lo.wrapping_sub(128));
    let local_x = p2add(payload[0], payload[1]);
    let tail = &payload[payload.len() - 7..];
    let z = i32::from(128u8.wrapping_sub(tail[0]));
    let chunk_x = (i32::from(tail[1]) << 8) | i32::from(tail[2]);
    let chunk_y = p2add(tail[3], tail[4]);
    let local_y = p2add(tail[5], tail[6]);
    (
        [(chunk_x - 6) * 8, (chunk_y - 6) * 8, z],
        [local_x, local_y],
    )
}

/// The original's entries of one session at the first tick it received
/// any, each with its call stack (None for entries written directly).
fn original_login_tick(scenario: &str, session: &str) -> Vec<(String, Option<Vec<String>>)> {
    let lines: Vec<Value> = ORIGINS
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let stacks: Vec<Vec<String>> =
        serde_json::from_value(lines.last().unwrap()["stacks"].clone()).unwrap();
    let mut origins: HashMap<i64, Value> = HashMap::new();
    let mut current = String::new();
    for line in &lines {
        match line["t"].as_str().unwrap() {
            "scenario" => current = line["name"].as_str().unwrap().to_owned(),
            "record" if current == scenario => {
                origins.insert(line["i"].as_i64().unwrap(), line["origins"].clone());
            }
            _ => {}
        }
    }
    let mut inside = false;
    for line in TRACE.lines() {
        let value: Value = serde_json::from_str(line).unwrap();
        if value["t"] == "scenario" {
            inside = value["name"] == scenario;
            continue;
        }
        if !inside || value["op"] != "tick" {
            continue;
        }
        let Some(entries) = value["out"][session].as_array() else {
            continue;
        };
        let origin = origins.get(&value["i"].as_i64().unwrap());
        return entries
            .iter()
            .enumerate()
            .map(|(n, e)| {
                (
                    e["hex"].as_str().unwrap().to_owned(),
                    origin
                        .and_then(|o| o[session][n].as_u64())
                        .map(|i| stacks[i as usize].clone()),
                )
            })
            .collect();
    }
    panic!("no login tick for {scenario} {session}")
}

/// The calls the live world answers as not ported whose original entries
/// are taken out before the comparison, with the number of entries each
/// removes from new-character's login tick.
/// Opcodes whose writers take `InterfaceManager.getPacketCount(1)`.
const COUNTER_OPCODES: [u8; 16] = [
    145, 155, 149, 21, 171, 165, 115, 36, 130, 73, 66, 132, 50, 119, 65, 69,
];

const NOT_PORTED_SEGMENTS: &[(&str, usize)] = &[
    ("CropCircleController.login", 1),
    ("FriendsListInterface.open", 2),
    ("FamiliarManager.login", 3),
    // The first entry of syncronizeTab (the quest points varp 101) is
    // native; the other 103 are the registered quests' configs and
    // updateVarps (content; the live registry is empty).
    ("QuestRepository.syncronizeTab", 103),
    ("LoginValidationPlugin", 1),
];

#[test]
fn live_world_logs_in_walks_shows_two_players_logs_out_and_saves() {
    let Some((cache, data)) = inputs() else {
        eprintln!(
            "SKIPPED: the live world needs assets/cache and runs/startup-loaders-oracle/template (or RS09_CACHE / RS09_STARTUP_WORLD)"
        );
        return;
    };
    let saves = std::env::temp_dir().join(format!("rs09-live-world-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&saves);
    let booted = Instant::now();
    let running = server::start(Options {
        data,
        cache: cache.clone(),
        saves: saves.clone(),
        verbose: std::env::var_os("RS09_LIVE_VERBOSE").is_some(),
    })
    .expect("the live world boots");
    eprintln!(
        "booted in {} ms (start-up complete: {}, partial steps {:?}, {} NPCs)",
        booted.elapsed().as_millis(),
        running.boot.complete,
        running.boot.partial_steps,
        running.boot.npcs
    );
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
    let stats = running.stats.clone();
    let tokio = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
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
    tokio.block_on(async {
        let server = service
            .start("127.0.0.1:0".parse().unwrap(), None)
            .await
            .unwrap();
        let address = server.tcp;
        drive(address, &saves).await;
        server.shutdown().await.unwrap();
    });
    let ticks = stats.ticks.load(Ordering::Relaxed);
    eprintln!(
        "ticks {ticks}: mean work {:.2} ms, max {:.2} ms, mean interval {:.1} ms, saves written {}",
        stats.mean_work_millis(),
        stats.max_work_micros.load(Ordering::Relaxed) as f64 / 1000.0,
        stats.mean_interval_millis(),
        stats.saves_written.load(Ordering::Relaxed)
    );
    assert!(ticks > 10);
    assert!(stats.mean_interval_millis() > 590.0 && stats.mean_interval_millis() < 700.0);
    running.stop();
    let _ = std::fs::remove_dir_all(&saves);
}

/// The scenario a headless client drives against a running live world at
/// `address` whose saves directory is `saves`: a new character, its login
/// tick against the world trace, a walk, a second character, both logouts,
/// the saves and a returning login.
async fn drive(address: std::net::SocketAddr, saves: &std::path::Path) {
    // A new character.
    let (code, one) = Client::login(address, "traceone").await;
    assert_eq!(code, 2, "login reply");
    let mut one = one.unwrap();
    assert_eq!(one.index, 1, "first player's index");
    one.until(
        Duration::from_secs(10),
        "the rebuild and a player update",
        |c| c.base.is_some() && c.updates.len() >= 2,
    )
    .await;
    let rebuild = one.frames.iter().position(|f| f.opcode == 162).unwrap();
    let first_update = one.frames.iter().position(|f| f.opcode == 225).unwrap();
    eprintln!(
        "traceone: rebuild at frame {rebuild}, first player update at frame {first_update}, base {:?}",
        one.base
    );

    // The login tick against the world-trace's new-character login tick.
    // The login tick against the world-trace's new-character login tick.
    let original = original_login_tick("new-character", "traceone");
    let marker_of = |stack: &Option<Vec<String>>| {
        NOT_PORTED_SEGMENTS.iter().find(|(m, _)| {
            stack
                .as_ref()
                .is_some_and(|frames| frames.iter().any(|f| f.contains(m)))
        })
    };
    let mut removed: HashMap<&str, usize> = HashMap::new();
    // Native leading entries of a segment (the quest tab's points varp).
    let native_first = |m: &str| usize::from(m == "QuestRepository.syncronizeTab");
    let mut seen: HashMap<&str, usize> = HashMap::new();
    // The login reply is the transport's (checked above by code and index).
    let kept: Vec<(String, Option<Vec<String>>)> = original
        .iter()
        .skip(1)
        .filter(|(_, stack)| match marker_of(stack) {
            Some((m, _)) => {
                let n = seen.entry(m).or_insert(0);
                *n += 1;
                if *n <= native_first(m) {
                    return true;
                }
                *removed.entry(m).or_insert(0) += 1;
                false
            }
            None => true,
        })
        .cloned()
        .collect();
    for (marker, n) in NOT_PORTED_SEGMENTS {
        assert_eq!(removed.get(marker).copied().unwrap_or(0), *n, "{marker}");
    }
    // reinitVarps writes the player's varp map in HashMap order: the
    // varps the skipped calls set are missing from the native map, so
    // that segment is compared as a set (every native varp is one of the
    // original's, with its value); everything before it in order.
    let reinit = kept
        .iter()
        .position(|(_, s)| {
            s.as_ref()
                .is_some_and(|f| f.iter().any(|f| f.contains("reinitVarps")))
        })
        .expect("reinitVarps in the login tick");
    let wire = |f: &Frame| {
        let mut wire = vec![];
        f.write(Direction::ServerToClient, None, &mut wire).unwrap();
        hex(&wire)
    };
    let mine: Vec<String> = one.frames.iter().map(wire).collect();
    // Packets that carry InterfaceManager's packet counter: the skipped
    // calls' own counter-carrying packets advance the original's counter,
    // so these are compared by opcode and length, the rest byte for byte.
    let counted =
        |hex: &str| u8::from_str_radix(&hex[..2], 16).is_ok_and(|op| COUNTER_OPCODES.contains(&op));
    let mut exact = 0;
    let mut counter_only = 0;
    let mut debug_suffix = 0;
    for (n, ((original, stack), native)) in kept[..reinit].iter().zip(&mine).enumerate() {
        if original == native {
            exact += 1;
            continue;
        }
        // MusicPlayer.play's track name: the original's new character is in
        // debug mode (LoginValidationPlugin's Player.toggleDebug, content
        // not ported here), so its name carries " (id)". The rest of the
        // String packet is the same except its size and the counter.
        let played = stack
            .as_ref()
            .is_some_and(|s| s.iter().any(|f| f.contains("MusicPlayer.play")));
        let suffix = hex(b" (62)");
        if played && original.contains(&suffix) {
            let stripped = original.replacen(&suffix, "", 1);
            if stripped.len() == native.len()
                && stripped[..2] == native[..2]
                && stripped[6..stripped.len() - 2] == native[6..native.len() - 2]
            {
                debug_suffix += 1;
                continue;
            }
        }
        assert!(
            counted(original) && original[..2] == native[..2] && original.len() == native.len(),
            "login tick entry {n}: native {native}, original {original} from {:?}",
            stack.as_ref().map(|s| s.iter().take(3).collect::<Vec<_>>())
        );
        counter_only += 1;
    }
    assert_eq!(
        debug_suffix, 1,
        "the one track name that differs by the debug suffix"
    );
    let original_varps: Vec<&String> = kept[reinit..]
        .iter()
        .take_while(|(h, _)| h.starts_with("3c") || h.starts_with("e2"))
        .map(|(h, _)| h)
        .collect();
    let native_varps: Vec<&String> = mine[reinit..]
        .iter()
        .take_while(|h| h.starts_with("3c") || h.starts_with("e2"))
        .collect();
    for varp in &native_varps {
        assert!(
            original_varps.contains(varp),
            "reinitVarps: native {varp} is not the original's"
        );
    }
    // After reinitVarps the login tick has nothing the native world writes.
    assert_eq!(
        kept.len(),
        reinit + original_varps.len(),
        "entries after reinitVarps"
    );
    eprintln!(
        "login tick: {} original entries; {} answered as not ported; {reinit} before reinitVarps compared in order ({exact} byte for byte, {counter_only} counter-carrying by opcode and length); reinitVarps: {} native varps, all among the original's {}",
        original.len() - 1,
        removed.values().sum::<usize>(),
        native_varps.len(),
        original_varps.len()
    );

    // A walk: three tiles east.
    let start = one.position();
    let target = [start[0] + 3, start[1], start[2]];
    one.send(actions::worldspace_walk(target[0], target[1], false))
        .await;
    one.until(Duration::from_secs(10), "the walk to finish", |c| {
        c.position() == target
    })
    .await;
    let steps = one
        .updates
        .iter()
        .flat_map(|(_, u)| &u.movements)
        .filter(|m| matches!(m, players::Movement::Step { .. }))
        .count();
    assert!(steps >= 3, "walking steps in player updates: {steps}");
    eprintln!("traceone walked from {start:?} to {target:?} in {steps} steps");

    // NPCs: NPC updates (32) arrive and add NPCs near the player.
    one.until(
        Duration::from_secs(10),
        "an NPC added near the player",
        |c| {
            let base = c.base.unwrap();
            let here = c.position();
            c.npcs.actors.values().any(|n| {
                let [x, y] = [base[0] + n.tile[0], base[1] + n.tile[1]];
                (x - here[0]).abs() <= 15 && (y - here[1]).abs() <= 15
            })
        },
    )
    .await;
    let near: Vec<i32> = one.npcs.actors.values().map(|n| n.type_id).collect();
    eprintln!(
        "traceone: {} NPC updates, {} local NPCs (ids {:?})",
        one.npc_updates,
        one.npcs.local_ids.len(),
        &near[..near.len().min(8)]
    );

    // The tabs' buttons (IfAction, opcode 155: hash, slot).
    let button = |iface: i32, child: i32| {
        let mut payload = ((iface << 16) | child).to_be_bytes().to_vec();
        payload.extend((-1i16).to_be_bytes());
        Frame {
            opcode: 155,
            payload,
        }
    };
    let wire = |f: &Frame| {
        let mut wire = vec![];
        f.write(Direction::ServerToClient, None, &mut wire).unwrap();
        wire
    };
    // The weapon tab (CombatTabInterface, interface 92): button 3 selects
    // slot 1 through WeaponInterface.setAttackStyle; Settings sends varp 43.
    let style = wire(&updates::varp(43, 1));
    let from = one.frames.len();
    one.send(button(92, 3)).await;
    one.until(Duration::from_secs(10), "the attack style varp", |c| {
        c.frames[from..].iter().any(|f| wire(f) == style)
    })
    .await;
    // The music tab (MusicTabInterface, 187): button 11 toggles looping, varp 19.
    let looping = wire(&updates::varp(19, 1));
    let from = one.frames.len();
    one.send(button(187, 11)).await;
    one.until(Duration::from_secs(10), "the music looping varp", |c| {
        c.frames[from..].iter().any(|f| wire(f) == looping)
    })
    .await;
    // The emote tab (EmoteTabInterface, 464): button 2 is YES, animation 855.
    let from = one.updates.len();
    one.send(button(464, 2)).await;
    one.until(Duration::from_secs(10), "the YES emote's animation", |c| {
        c.updates[from..].iter().any(|(_, u)| {
            u.extended.iter().any(|e| {
                e.fields
                    .iter()
                    .any(|f| matches!(f, players::Field::Animation { id: 855, .. }))
            })
        })
    })
    .await;
    eprintln!("traceone: weapon tab style, music looping and the YES emote answered natively");

    // A second character sees the first, and the first sees it.
    let (code, two) = Client::login(address, "tracetwo").await;
    assert_eq!(code, 2);
    let mut two = two.unwrap();
    assert_eq!(two.index, 2);
    let (index_one, index_two) = (one.index as usize, two.index as usize);
    two.until(Duration::from_secs(10), "traceone added", |c| {
        c.updates.iter().any(|(_, u)| {
            u.movements
                .iter()
                .any(|m| matches!(m, players::Movement::Added { index, .. } if *index == index_one))
        })
    })
    .await;
    one.until(
        Duration::from_secs(10),
        "tracetwo added with its appearance",
        |c| {
            c.updates.iter().any(|(_, u)| {
                u.movements.iter().any(
                    |m| matches!(m, players::Movement::Added { index, .. } if *index == index_two),
                ) && u.extended.iter().any(|e| {
                    e.index == index_two
                        && e.fields
                            .iter()
                            .any(|f| matches!(f, players::Field::Appearance { .. }))
                })
            })
        },
    )
    .await;
    let appearance = two.updates.iter().flat_map(|(_, u)| &u.extended).any(|e| {
        e.index == index_one
            && e.fields
                .iter()
                .any(|f| matches!(f, players::Field::Appearance { .. }))
    });
    assert!(appearance, "tracetwo received traceone's appearance");
    eprintln!("traceone and tracetwo see each other");

    // tracetwo logs out (closes its connection); traceone sees it go.
    two.writer.close(1000, "logout").await.unwrap();
    one.until(Duration::from_secs(15), "tracetwo removed", |c| {
        c.updates
            .iter()
            .any(|(_, u)| u.removed.contains(&index_two))
            || c.players.players[index_two].is_none()
    })
    .await;
    one.writer.close(1000, "logout").await.unwrap();

    // Both saves are written by the saver thread.
    let players = saves.join("players");
    let end = Instant::now() + Duration::from_secs(15);
    while !(players.join("traceone.json").exists() && players.join("tracetwo.json").exists()) {
        assert!(Instant::now() < end, "saves were not written");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    // Give the queue the ticks it needs to remove both characters.
    tokio::time::sleep(Duration::from_secs(3)).await;
    for name in ["traceone", "tracetwo"] {
        let text = std::fs::read_to_string(players.join(format!("{name}.json"))).unwrap();
        let json = rs09_server::player::save::parser::parse_text(&text)
            .unwrap_or_else(|e| panic!("{name}.json: {e:?}"));
        assert!(json.get("core_data").is_some(), "{name}.json has core_data");
    }
    let saved = std::fs::read_to_string(players.join("traceone.json")).unwrap();
    let saved: Value = serde_json::from_str(&saved).unwrap();
    let location = saved["core_data"]["location"].as_str().unwrap().to_owned();
    assert_eq!(
        location,
        format!("{},{},{}", target[0], target[1], target[2]),
        "traceone's saved location"
    );

    // traceone returns: the native parser loads the save at login.
    let (code, again) = Client::login(address, "traceone").await;
    assert_eq!(code, 2, "returning login");
    let mut again = again.unwrap();
    again
        .until(Duration::from_secs(10), "the returning rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    assert_eq!(again.position(), target, "returning character's location");
    again.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The same scenario against an `rs09-server --world` started outside the
/// test: `RS09_LIVE_ADDRESS=127.0.0.1:PORT RS09_LIVE_SAVES=DIR cargo test
/// -p rs09-server --test live_world -- --ignored`. The world must be fresh
/// (no character logged in yet) and its saves directory DIR.
#[test]
#[ignore = "needs a running rs09-server --world (RS09_LIVE_ADDRESS, RS09_LIVE_SAVES)"]
fn running_server_scenario() {
    let (Some(address), Some(saves)) = (
        std::env::var_os("RS09_LIVE_ADDRESS"),
        std::env::var_os("RS09_LIVE_SAVES"),
    ) else {
        eprintln!("SKIPPED: set RS09_LIVE_ADDRESS and RS09_LIVE_SAVES");
        return;
    };
    let address = address.to_string_lossy().parse().expect("an address");
    let started = Instant::now();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(drive(address, std::path::Path::new(&saves)));
    eprintln!("scenario finished in {} ms", started.elapsed().as_millis());
}
