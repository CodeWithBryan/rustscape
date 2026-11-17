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
    game::{self, Direction, Frame, actions, ground, npcs, players, updates},
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
    /// The client's ground item lists (the shared RT4 reducer), reset by
    /// each rebuild.
    ground: ground::State,
    /// Containers by id (93 inventory, 94 equipment): slot -> (id, amount).
    containers: HashMap<i32, std::collections::BTreeMap<i32, (i32, i32)>>,
    /// Skill packets: skill -> (level, experience), the last of each.
    skills: HashMap<i32, (i32, i32)>,
    /// Every skill packet in order: (frame number, skill, level).
    skill_log: Vec<(usize, i32, i32)>,
    /// Game messages (70) as text, with their frame numbers.
    messages: Vec<(usize, String)>,
}

/// Item metadata for the ground reducer: the test reads lists, not piles.
struct NoMeta;
impl ground::Context for NoMeta {
    fn info(&mut self, _: &mut ground::State, _: i32) -> rs09_protocol::Result<ground::Info> {
        Ok(ground::Info {
            cost: 1,
            stackable: 0,
        })
    }
    fn height(
        &mut self,
        _: &mut ground::State,
        _: i32,
        _: i32,
        _: i32,
    ) -> rs09_protocol::Result<i32> {
        Ok(0)
    }
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
                ground: ground::State::default(),
                containers: HashMap::new(),
                skills: HashMap::new(),
                skill_log: Vec::new(),
                messages: Vec::new(),
            }),
        )
    }

    /// The ground items the client holds on an absolute tile.
    fn ground_ids(&self, at: [i32; 3]) -> Vec<i32> {
        let Some(base) = self.base else {
            return vec![];
        };
        let tile = ground::Tile {
            plane: at[2],
            x: at[0] - base[0],
            y: at[1] - base[1],
        };
        self.ground
            .stack(tile)
            .ok()
            .flatten()
            .map(|s| s.items().iter().map(|i| i.get().id).collect())
            .unwrap_or_default()
    }
    fn slot_of(&self, container: i32, id: i32) -> Option<i32> {
        self.containers
            .get(&container)?
            .iter()
            .find(|(_, (i, _))| *i == id)
            .map(|(s, _)| *s)
    }

    fn take_frames(&mut self) {
        while let Some(frame) = self.decoder.next_frame().expect("a server frame") {
            let n = self.frames.len();
            match frame.opcode {
                162 => {
                    let (base, local) = rebuild_base(&frame.payload);
                    self.base = Some(base);
                    self.players = players::State::new(local[0], local[1]);
                    self.ground = ground::State::default();
                    self.ground.plane = base[2];
                    self.ground.self_id = self.index;
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
                _ => {
                    if self.base.is_some() {
                        self.ground
                            .apply_frame(&frame, &mut NoMeta)
                            .unwrap_or_else(|e| panic!("{}: ground frame {n}: {e:?}", self.name));
                    }
                    match updates::ServerUpdate::decode(&frame) {
                        Ok(Some(updates::ServerUpdate::InventoryFull {
                            container, items, ..
                        })) => {
                            let slots = self.containers.entry(container).or_default();
                            slots.clear();
                            for (slot, item) in items.iter().enumerate() {
                                if item.id >= 0 && item.amount > 0 {
                                    slots.insert(slot as i32, (item.id, item.amount));
                                }
                            }
                        }
                        Ok(Some(updates::ServerUpdate::InventoryPartial {
                            container,
                            slots,
                            ..
                        })) => {
                            let held = self.containers.entry(container).or_default();
                            for update in slots {
                                if update.item.id >= 0 && update.item.amount > 0 {
                                    held.insert(update.slot, (update.item.id, update.item.amount));
                                } else {
                                    held.remove(&update.slot);
                                }
                            }
                        }
                        Ok(Some(updates::ServerUpdate::Skill {
                            skill,
                            boosted,
                            experience,
                        })) => {
                            self.skills.insert(skill, (boosted, experience));
                            self.skill_log.push((n, skill, boosted));
                        }
                        Ok(Some(updates::ServerUpdate::GameMessage(text))) => {
                            self.messages
                                .push((n, String::from_utf8_lossy(&text).into_owned()));
                        }
                        _ => {}
                    }
                }
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
                "{}: timed out waiting for {what} (inventory {:?}, last messages {:?})",
                self.name,
                self.containers.get(&93),
                &self.messages[self.messages.len().saturating_sub(5)..]
            );
            self.pump(Duration::from_millis(200)).await;
        }
    }

    /// [`Client::until`] for long waits: a PING (93) every five seconds, as
    /// the client sends one, so the 20-second ping timeout does not log it out.
    async fn until_pinging(&mut self, limit: Duration, what: &str, done: impl Fn(&Client) -> bool) {
        let end = Instant::now() + limit;
        let mut ping = Instant::now();
        while !done(self) {
            assert!(
                Instant::now() < end && !self.closed,
                "{}: timed out waiting for {what} (closed: {})",
                self.name,
                self.closed
            );
            if ping.elapsed() > Duration::from_secs(5) {
                self.send(frame(93, &[])).await;
                ping = Instant::now();
            }
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
    // An NPC's first option (processNpcAction): the NPC's InteractPlugin
    // over its definition's options, the walk to it and the legacy
    // arrival; no content handler answers, so "Nothing interesting happens.".
    let base = one.base.unwrap();
    let here = one.position();
    let (index, id) = one
        .npcs
        .actors
        .iter()
        .min_by_key(|(_, n)| {
            (base[0] + n.tile[0] - here[0]).abs() + (base[1] + n.tile[1] - here[1]).abs()
        })
        .map(|(i, n)| (*i, n.type_id))
        .unwrap();
    let from = one.messages.len();
    one.send(npc_option1(index as i32)).await;
    one.until(Duration::from_secs(20), "the NPC option's answer", |c| {
        c.messages[from..]
            .iter()
            .any(|(_, m)| m.contains("Nothing interesting happens."))
    })
    .await;
    eprintln!("traceone: NPC option 1 on NPC {index} ({id}): Nothing interesting happens.");

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
    // Where traceone is when it leaves (it walked, then walked to the NPC).
    let target = one.position();
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

    // Hitpoints restoration (SkillRestore, installed by addAutoTimers) runs
    // beside the other scenarios: it needs about a hundred ticks.
    let healer = tokio::spawn(heal_scenario(address, saves.to_path_buf()));
    items_scenario(address, saves).await;
    wilderness_scenario(address, saves).await;
    scenery_scenario(address, saves).await;
    healer.await.expect("the heal scenario");
}

/// A scenery option: a saved character next to the ladder (36768) east of
/// Lumbridge castle at 3229,3213 uses Climb-up (opcode 254):
/// processSceneryAction, the legacy walk and arrival, the global handlers'
/// LadderManagingPlugin and ClimbActionHandler (lock, animation 828, the
/// one-tick pulse and the teleport location): the client's position reaches
/// the floor above.
async fn scenery_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3229, 3212, 0];
    write_save(&players, "traceone", "climbone", at, &[]);
    let (code, one) = Client::login(address, "climbone").await;
    assert_eq!(code, 2, "climbone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "climbone's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    assert_eq!(one.position(), at, "climbone's saved location");
    let from = one.updates.len();
    one.send(scenery_option1(36768, 3229, 3213)).await;
    one.until(Duration::from_secs(10), "the floor above the ladder", |c| {
        c.players.plane == 1
    })
    .await;
    let climbed = one.updates[from..].iter().any(|(_, u)| {
        u.extended.iter().any(|e| {
            e.index == one.index as usize
                && e.fields
                    .iter()
                    .any(|f| matches!(f, players::Field::Animation { id: 828, .. }))
        })
    });
    eprintln!(
        "climbone climbed the ladder 36768 at [3229, 3213, 0]: now on plane {} at {:?} (climb animation 828 seen: {climbed})",
        one.players.plane,
        one.players.self_player().tile()
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The engine timers: a saved character with 5 of its 10 hitpoints logs in;
/// `TimerRegistry.addAutoTimers` installs SkillRestore, which heals one
/// hitpoint once its counter passes 100 ticks (the client's skill packet for
/// hitpoints goes from 5 to 6, about a minute after login).
async fn heal_scenario(address: std::net::SocketAddr, saves: PathBuf) {
    let players = saves.join("players");
    let at = [3221, 3219, 0];
    write_save_with(&players, "traceone", "healone", at, &[], |save| {
        save["skills"][3]["lifepoints"] = Value::String("5".into());
    });
    let (code, one) = Client::login(address, "healone").await;
    assert_eq!(code, 2, "healone's login");
    let mut one = one.unwrap();
    let logged_in = Instant::now();
    one.until(Duration::from_secs(10), "healone's hitpoints", |c| {
        c.skills.get(&3).map(|s| s.0) == Some(5)
    })
    .await;
    one.until_pinging(Duration::from_secs(120), "a hitpoint restored", |c| {
        c.skills.get(&3).map(|s| s.0) == Some(6)
    })
    .await;
    let after = logged_in.elapsed();
    eprintln!(
        "healone's hitpoints 5 -> 6 after {:.1} s (SkillRestore: 100 ticks)",
        after.as_secs_f64()
    );
    assert!(
        after > Duration::from_secs(50) && after < Duration::from_secs(90),
        "one hitpoint about 100 ticks after login, not {after:?}"
    );
    one.writer.close(1000, "logout").await.unwrap();
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

// ---------------------------------------------------- packets the client sends

fn le16(v: i32) -> [u8; 2] {
    [v as u8, (v >> 8) as u8]
}
fn la16(v: i32) -> [u8; 2] {
    [(v as u8).wrapping_add(128), (v >> 8) as u8]
}
fn sa16(v: i32) -> [u8; 2] {
    [(v >> 8) as u8, (v as u8).wrapping_add(128)]
}
fn be16(v: i32) -> [u8; 2] {
    [(v >> 8) as u8, v as u8]
}
fn ib32(v: i32) -> [u8; 4] {
    [(v >> 16) as u8, (v >> 24) as u8, v as u8, (v >> 8) as u8]
}
fn ia32(v: i32) -> [u8; 4] {
    [(v >> 8) as u8, v as u8, (v >> 24) as u8, (v >> 16) as u8]
}
fn frame(opcode: u8, parts: &[&[u8]]) -> Frame {
    Frame {
        opcode,
        payload: parts.concat(),
    }
}
/// OBJSTACK_ACTION_1 (66): a ground item's third option, Take.
fn take(id: i32, x: i32, y: i32) -> Frame {
    frame(66, &[&le16(x), &be16(id), &la16(y)])
}
/// ITEM_ACTION_5 (135): an inventory item's fifth option (Drop).
fn item_option5(id: i32, slot: i32) -> Frame {
    frame(135, &[&sa16(id), &sa16(slot), &ib32(149 << 16)])
}
/// ITEM_ACTION_2 (55): an inventory item's second option (Wield).
fn item_option2(id: i32, slot: i32) -> Frame {
    frame(55, &[&le16(id), &sa16(slot), &ia32(149 << 16)])
}
/// EXAMINE_ITEM (92) and EXAMINE_NPC (72).
fn item_examine(id: i32) -> Frame {
    frame(92, &[&la16(id)])
}
fn npc_examine(id: i32) -> Frame {
    frame(72, &[&be16(id)])
}
fn li32(v: i32) -> [u8; 4] {
    v.to_le_bytes()
}
/// SLOT_SWITCH (231): an inventory item dragged to another slot (no insert).
fn slot_switch(source: i32, dest: i32) -> Frame {
    frame(231, &[&be16(source), &li32(149 << 16), &sa16(dest), &[0]])
}
/// USE_ITEM_ON_ITEM (27): one inventory item on another.
fn use_item_on_item(used: i32, used_slot: i32, with: i32, with_slot: i32) -> Frame {
    frame(
        27,
        &[
            &be16(used_slot),
            &li32(149 << 16),
            &le16(with_slot),
            &li32(149 << 16),
            &la16(used),
            &la16(with),
        ],
    )
}
/// OBJECT_ACTION_1 (254): a scenery's first option.
fn scenery_option1(id: i32, x: i32, y: i32) -> Frame {
    frame(254, &[&le16(x), &sa16(id), &be16(y)])
}
/// NPC_ACTION_1 (78): an NPC's first option.
fn npc_option1(index: i32) -> Frame {
    frame(78, &[&le16(index)])
}

/// A saved character made from another's save: its location and inventory
/// replaced (the native parser loads it at login like any returning one).
fn write_save(
    players: &std::path::Path,
    from: &str,
    name: &str,
    location: [i32; 3],
    inventory: &[(i32, i32, i32)],
) {
    write_save_with(players, from, name, location, inventory, |_| {});
}

/// [`write_save`] with a further edit of the save's JSON.
fn write_save_with(
    players: &std::path::Path,
    from: &str,
    name: &str,
    location: [i32; 3],
    inventory: &[(i32, i32, i32)],
    edit: impl FnOnce(&mut Value),
) {
    let text = std::fs::read_to_string(players.join(format!("{from}.json"))).unwrap();
    let mut save: Value = serde_json::from_str(&text).unwrap();
    save["core_data"]["location"] =
        Value::String(format!("{},{},{}", location[0], location[1], location[2]));
    save["core_data"]["inventory"] = Value::Array(
        inventory
            .iter()
            .map(|(slot, id, amount)| {
                serde_json::json!({"slot": slot.to_string(), "id": id.to_string(),
                    "amount": amount.to_string(), "charge": "1000"})
            })
            .collect(),
    );
    edit(&mut save);
    std::fs::write(
        players.join(format!("{name}.json")),
        serde_json::to_string(&save).unwrap(),
    )
    .unwrap();
}

/// Ground items, the item handlers and the equipment: a saved character in
/// Lumbridge castle's kitchen, on the start-up's ground spawn of a bowl
/// (1923 at 3208,3214), with a bronze dagger (1205) in its inventory; a
/// second character watches.
async fn items_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let spawn = [3208, 3214, 0];
    write_save(&players, "traceone", "groundone", spawn, &[(0, 1205, 1)]);
    write_save(&players, "traceone", "groundtwo", [3209, 3215, 0], &[]);
    let (code, one) = Client::login(address, "groundone").await;
    assert_eq!(code, 2, "groundone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "groundone's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    assert_eq!(one.position(), spawn, "groundone's saved location");
    // The start-up's GroundSpawn is in the chunk synchronisation.
    one.until(Duration::from_secs(10), "the bowl spawn", |c| {
        c.ground_ids(spawn).contains(&1923)
    })
    .await;
    one.until(Duration::from_secs(10), "the inventory", |c| {
        c.slot_of(93, 1205) == Some(0)
    })
    .await;
    eprintln!("groundone sees the ground spawn 1923 at {spawn:?}; dagger 1205 in slot 0");

    // Take: the ground item packet, the legacy movement and arrival,
    // PickupPlugin and PickupHandler.take.
    let from = one.frames.len();
    one.send(take(1923, spawn[0], spawn[1])).await;
    one.until(Duration::from_secs(10), "the bowl in the inventory", |c| {
        c.slot_of(93, 1923).is_some() && !c.ground_ids(spawn).contains(&1923)
    })
    .await;
    let pickup_sound = updates::synth_sound(2582, 1, 0);
    assert!(
        one.frames[from..].contains(&pickup_sound),
        "the pickup sound (2582)"
    );
    let slot = one.slot_of(93, 1923).unwrap();
    eprintln!("groundone took 1923 into slot {slot}; the spawn left the ground");

    // A second character next to it.
    let (code, two) = Client::login(address, "groundtwo").await;
    assert_eq!(code, 2, "groundtwo's login");
    let mut two = two.unwrap();
    two.until(Duration::from_secs(10), "groundtwo's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    assert!(
        !two.ground_ids(spawn).contains(&1923),
        "the taken spawn is gone for a new viewer"
    );

    // Drop: processItemAction, InteractPlugin.handleItemOption and its
    // pulse, DropListener through InteractionListeners, the soft script.
    let from = one.frames.len();
    one.send(item_option5(1923, slot)).await;
    one.until(Duration::from_secs(10), "the dropped bowl", |c| {
        c.slot_of(93, 1923).is_none() && c.ground_ids(spawn).contains(&1923)
    })
    .await;
    let drop_sound = updates::synth_sound(2739, 1, 0);
    assert!(
        one.frames[from..].contains(&drop_sound),
        "the drop sound (2739)"
    );
    // A dropped item is private to its dropper for its first 100 ticks.
    two.pump(Duration::from_millis(1500)).await;
    assert!(
        !two.ground_ids(spawn).contains(&1923),
        "the dropped bowl is private to groundone"
    );
    eprintln!("groundone dropped 1923 at {spawn:?} (private: groundtwo does not see it)");

    // Take it again (a dropped item, the dropper's own), after the
    // `droppedItem:1923` deadline (two ticks after the drop).
    one.pump(Duration::from_millis(1500)).await;
    let from = one.messages.len();
    one.send(take(1923, spawn[0], spawn[1])).await;
    one.pump(Duration::from_millis(3000)).await;
    assert!(
        one.slot_of(93, 1923).is_some(),
        "the dropped bowl was not taken back; messages since: {:?}",
        &one.messages[from..]
    );
    one.until(Duration::from_secs(10), "the bowl taken back", |c| {
        c.slot_of(93, 1923).is_some() && !c.ground_ids(spawn).contains(&1923)
    })
    .await;
    eprintln!("groundone took the dropped bowl back: drop/pickup round trip over TCP");

    // Slot switch (opcode 231): the bowl moves to inventory slot 5
    // (processSlotSwitch, switchItem over the canonical inventory).
    let bowl = one.slot_of(93, 1923).unwrap();
    one.send(slot_switch(bowl, 5)).await;
    one.until(Duration::from_secs(10), "the bowl in slot 5", |c| {
        c.slot_of(93, 1923) == Some(5) && c.slot_of(93, 1205) == Some(0)
    })
    .await;
    eprintln!("groundone switched the bowl from slot {bowl} to slot 5");
    // Use-with (opcode 27): the dagger on the bowl; no listener, plugin or
    // UseWithHandler takes it, so runUnhandledAction answers.
    let from = one.messages.len();
    one.send(use_item_on_item(1205, 0, 1923, 5)).await;
    one.until(
        Duration::from_secs(10),
        "the unhandled use-with message",
        |c| {
            c.messages[from..]
                .iter()
                .any(|(_, m)| m == "Nothing interesting happens.")
        },
    )
    .await;
    eprintln!("groundone used 1205 on 1923: \"Nothing interesting happens.\"");

    // Wield: EquipHandler over the canonical EquipmentContainer.
    one.send(item_option2(1205, 0)).await;
    one.until(
        Duration::from_secs(10),
        "the dagger in the weapon slot",
        |c| {
            c.containers.get(&94).and_then(|e| e.get(&3)) == Some(&(1205, 1))
                && c.slot_of(93, 1205).is_none()
        },
    )
    .await;
    eprintln!("groundone wields 1205: equipment slot 3, inventory slot 0 empty");
    // The second player sees the weapon in groundone's appearance block.
    let index_one = one.index as usize;
    two.until(
        Duration::from_secs(10),
        "groundone's appearance with the dagger",
        |c| {
            c.updates.iter().flat_map(|(_, u)| &u.extended).any(|e| {
                e.index == index_one
                    && e.fields.iter().any(|f| match f {
                        // The weapon slot (3) carries a worn item: 0x8000 plus
                        // the item's equipment index.
                        players::Field::Appearance { appearance, .. } => {
                            appearance.slots[3] >= 0x8000
                        }
                        _ => false,
                    })
            })
        },
    )
    .await;
    eprintln!("groundtwo sees groundone's appearance with the dagger (1205)");

    // Examine: an item and an NPC.
    let from = one.messages.len();
    one.send(item_examine(1205)).await;
    one.until(Duration::from_secs(10), "the item examine", |c| {
        c.messages.len() > from
    })
    .await;
    eprintln!("item examine of 1205: {:?}", one.messages[from].1);
    let near = one.npcs.actors.iter().map(|(i, n)| (*i, n.type_id)).next();
    if let Some((index, id)) = near {
        let from = one.messages.len();
        one.send(npc_examine(id)).await;
        one.until(Duration::from_secs(10), "the NPC examine", |c| {
            c.messages.len() > from
        })
        .await;
        eprintln!(
            "NPC examine of {id} (index {index}): {:?}",
            one.messages[from].1
        );
    }
    two.writer.close(1000, "logout").await.unwrap();
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The region zones (ZoneBuilder.startup): a saved character in the
/// wilderness north of Edgeville's ditch enters WildernessZone at login:
/// overlay 381 and its level line through SkullManager.setLevel.
async fn wilderness_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3093, 3525, 0];
    write_save(&players, "traceone", "wildone", at, &[]);
    let (code, one) = Client::login(address, "wildone").await;
    assert_eq!(code, 2, "wildone's login");
    let mut one = one.unwrap();
    let pointer = (381 << 16) | 1;
    one.until(
        Duration::from_secs(10),
        "the wilderness overlay and level",
        |c| {
            let opened = c.frames.iter().any(|f| {
                matches!(
                    updates::ServerUpdate::decode(f),
                    Ok(Some(updates::ServerUpdate::OpenInterface {
                        interface: 381,
                        ..
                    }))
                )
            });
            let level = c.frames.iter().any(|f| {
                matches!(updates::ServerUpdate::decode(f),
                Ok(Some(updates::ServerUpdate::InterfaceText { pointer: p, ref text, .. }))
                    if p == pointer && text.as_slice() == b"Level: 1")
            });
            opened && level
        },
    )
    .await;
    assert_eq!(one.position(), at, "wildone's saved location");
    eprintln!("wildone entered WildernessZone at {at:?}: overlay 381, \"Level: 1\"");
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}
