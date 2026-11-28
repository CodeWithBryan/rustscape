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
    /// Hit marks in NPC update mask blocks: (NPC index, damage, type).
    npc_hits: Vec<(usize, i32, i32)>,
    /// Projectiles (zone update 16 inside 230): (frame number, projectile).
    projectiles: Vec<(usize, updates::Projectile)>,
    /// Interfaces opened (server opcode 155): (frame number, interface id).
    opened: Vec<(usize, i32)>,
    /// Interface texts (171): (frame number, component pointer, text).
    texts: Vec<(usize, i32, String)>,
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
        Self::login_as(address, name, false).await
    }

    /// A login with the reconnect operation (18) when `reconnect` is set.
    async fn login_as(
        address: std::net::SocketAddr,
        name: &str,
        reconnect: bool,
    ) -> (i32, Option<Client>) {
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
                    reconnect,
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
                npc_hits: Vec::new(),
                projectiles: Vec::new(),
                opened: Vec::new(),
                texts: Vec::new(),
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
                    // Only this update's mask blocks count as hits.
                    for npc in self.npcs.actors.values_mut() {
                        npc.last_mask = Default::default();
                    }
                    self.npcs
                        .decode(&frame, tile, n as i32)
                        .unwrap_or_else(|e| panic!("{}: NPC update {n}: {e:?}", self.name));
                    self.npc_updates += 1;
                    for (index, npc) in &self.npcs.actors {
                        if npc.last_seen != n as i32 {
                            continue;
                        }
                        for hit in [&npc.last_mask.primary, &npc.last_mask.secondary]
                            .into_iter()
                            .flatten()
                        {
                            self.npc_hits.push((*index, hit.damage, hit.kind));
                        }
                    }
                }
                _ => {
                    if frame.opcode == 155 && frame.payload.len() == 9 {
                        let id =
                            i32::from(u16::from_be_bytes([frame.payload[7], frame.payload[8]]));
                        self.opened.push((n, id));
                    }
                    if frame.opcode == 230
                        && let Ok(Some(updates::ServerUpdate::ZoneEnclosed { frames, .. })) =
                            updates::ServerUpdate::decode(&frame)
                    {
                        for child in frames.iter().filter(|f| f.opcode == 16) {
                            let projectile = updates::Projectile::decode(child)
                                .unwrap_or_else(|e| panic!("{}: projectile {n}: {e:?}", self.name));
                            self.projectiles.push((n, projectile));
                        }
                    }
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
                        Ok(Some(updates::ServerUpdate::InterfaceText {
                            pointer, text, ..
                        })) => {
                            self.texts.push((
                                n,
                                pointer,
                                String::from_utf8_lossy(&text).into_owned(),
                            ));
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

    /// The last appearance block received for the player at `index`.
    fn appearance_of(&self, index: i32) -> Option<&players::RawAppearance> {
        self.updates
            .iter()
            .flat_map(|(_, u)| &u.extended)
            .filter(|e| e.index == index as usize)
            .flat_map(|e| &e.fields)
            .filter_map(|f| match f {
                players::Field::Appearance { appearance, .. } => Some(appearance),
                _ => None,
            })
            .next_back()
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
    let accounts = running.accounts.clone();
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
        drive(address, &saves, Some(&accounts)).await;
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
async fn drive(
    address: std::net::SocketAddr,
    saves: &std::path::Path,
    accounts: Option<&Arc<dyn rs09_server::auth::storage::AccountStorage>>,
) {
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
    for (n, ((original, stack), native)) in kept[..reinit].iter().zip(&mine).enumerate() {
        if original == native {
            exact += 1;
            continue;
        }
        // LoginValidationPlugin is native: the new character's debug mode
        // (Player.toggleDebug) gives MusicPlayer.play's track name its " (id)"
        // as the original's has it, so no entry differs by text.
        assert!(
            counted(original) && original[..2] == native[..2] && original.len() == native.len(),
            "login tick entry {n}: native {native}, original {original} from {:?}",
            stack.as_ref().map(|s| s.iter().take(3).collect::<Vec<_>>())
        );
        counter_only += 1;
    }

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
    // An NPC's first option (processNpcAction). "Talk-to" goes through the
    // native InteractionListeners (NPCTalkListener's catch-all): the
    // movement pulse to the NPC, the arrival, and
    // dialogueInterpreter.open(npc.id, npc), which finds no content dialogue
    // registered for the NPC (dialogue content is not ported) after stopping
    // it; the player ends beside the NPC. Any other first option is the
    // NPC's InteractPlugin with the legacy arrival: no content handler
    // answers, so "Nothing interesting happens.".
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
        let nothing = c.messages[from..]
            .iter()
            .any(|(_, m)| m.contains("Nothing interesting happens."));
        let talk = c.messages[from..]
            .iter()
            .any(|(_, m)| m.contains("Option: Talk-To"));
        let beside = c.npcs.actors.get(&index).is_some_and(|n| {
            let (base, here) = (c.base.unwrap(), c.position());
            (base[0] + n.tile[0] - here[0]).abs() <= 1 && (base[1] + n.tile[1] - here[1]).abs() <= 1
        });
        nothing || (talk && beside)
    })
    .await;
    let nothing = one.messages[from..]
        .iter()
        .any(|(_, m)| m.contains("Nothing interesting happens."));
    eprintln!(
        "traceone: NPC option 1 on NPC {index} ({id}): {}",
        if nothing {
            "Nothing interesting happens."
        } else {
            "Talk-to walked to it (no content dialogue registered)"
        }
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

    // RS09_LIVE_SCENARIOS=a,b runs only the named scenarios after the first
    // character's save (a shortcut while working on one of them).
    if let Some(list) = std::env::var_os("RS09_LIVE_SCENARIOS") {
        for name in list.to_string_lossy().split(',') {
            match name.trim() {
                "items" => items_scenario(address, saves).await,
                "wilderness" => wilderness_scenario(address, saves).await,
                "scenery" => scenery_scenario(address, saves).await,
                "combat" => combat_scenario(address, saves).await,
                "aggression" => aggression_scenario(address, saves).await,
                "death" => death_scenario(address, saves).await,
                "grave" => grave_scenario(address, saves, accounts).await,
                "subclass" => subclass_scenario(address, saves).await,
                "dragon" => dragon_scenario(address, saves).await,
                "ranged" => ranged_scenario(address, saves).await,
                "special" => special_scenario(address, saves).await,
                "bank" => bank_scenario(address, saves).await,
                "shop" => shop_scenario(address, saves).await,
                "trade" => trade_scenario(address, saves).await,
                "follow" => follow_scenario(address, saves).await,
                "eat" => eat_scenario(address, saves).await,
                "usewith" => use_with_scenario(address, saves).await,
                "gather" => gather_scenario(address, saves).await,
                "magic" => magic_scenario(address, saves).await,
                "teleport" => teleport_scenario(address, saves).await,
                "effects" => effects_scenario(address, saves).await,
                "appearance" => appearance_scenario(address, saves).await,
                "reconnect" => reconnect_scenario(address, saves).await,
                other => panic!("RS09_LIVE_SCENARIOS: no scenario {other}"),
            }
        }
        return;
    }
    if std::env::var_os("RS09_LIVE_COMBAT_ONLY").is_some() {
        combat_scenario(address, saves).await;
        aggression_scenario(address, saves).await;
        death_scenario(address, saves).await;
        grave_scenario(address, saves, accounts).await;
        subclass_scenario(address, saves).await;
        dragon_scenario(address, saves).await;
        ranged_scenario(address, saves).await;
        magic_scenario(address, saves).await;
        special_scenario(address, saves).await;
        return;
    }
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
    combat_scenario(address, saves).await;
    aggression_scenario(address, saves).await;
    death_scenario(address, saves).await;
    grave_scenario(address, saves, accounts).await;
    subclass_scenario(address, saves).await;
    dragon_scenario(address, saves).await;
    ranged_scenario(address, saves).await;
    special_scenario(address, saves).await;
    bank_scenario(address, saves).await;
    shop_scenario(address, saves).await;
    trade_scenario(address, saves).await;
    follow_scenario(address, saves).await;
    eat_scenario(address, saves).await;
    use_with_scenario(address, saves).await;
    gather_scenario(address, saves).await;
    magic_scenario(address, saves).await;
    teleport_scenario(address, saves).await;
    effects_scenario(address, saves).await;
    appearance_scenario(address, saves).await;
    reconnect_scenario(address, saves).await;
    healer.await.expect("the heal scenario");
}

/// A player's death: a saved character with one hitpoint, the tutorial
/// complete (so `handleImpact` keeps the damage) and `allow_admin_aggression`,
/// next to the Lumbridge swamp's giant rats. A rat's hit leaves it without
/// lifepoints: `Entity.startDeath`, the death task (`DeathTask`, a strong
/// pulse), `Player.commenceDeath`, then `Player.finalizeDeath`
/// (`world/player_death`): "Oh dear, you are dead!", the skills restored,
/// the death screen (153) opened and the respawn location. The test
/// accounts are administrators: nothing is lost (no grave, no drop).
async fn death_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3196, 3206, 0];
    write_save_with(
        &players,
        "traceone",
        "deathone",
        at,
        &[(0, 1205, 1)],
        |save| {
            save["skills"][3]["lifepoints"] = Value::String("1".into());
            let list = save
                .as_object_mut()
                .unwrap()
                .entry("attributes")
                .or_insert_with(|| Value::Array(vec![]));
            for key in ["allow_admin_aggression", "tutorial:complete"] {
                list.as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"key": key, "type": "bool", "value": true}));
            }
        },
    );
    let (code, one) = Client::login(address, "deathone").await;
    assert_eq!(code, 2, "deathone's login");
    let mut one = one.unwrap();
    let started = Instant::now();
    // tutorial:complete: LoginConfiguration.sendLobbyScreen (the welcome
    // screen 378 with LastLoginInfo); its play button (140) runs
    // configureGameWorld a tick later.
    one.until(Duration::from_secs(10), "deathone's lobby", |c| {
        c.base.is_some() && c.frames.iter().any(|f| f.opcode == 164)
    })
    .await;
    one.send(button(378, 140)).await;
    one.until(Duration::from_secs(10), "deathone in the game world", |c| {
        !c.updates.is_empty()
    })
    .await;
    eprintln!(
        "deathone logged in at {:?}: hitpoints {:?}",
        one.position(),
        one.skills.get(&3)
    );
    one.until_pinging(Duration::from_secs(120), "Oh dear, you are dead!", |c| {
        c.messages
            .iter()
            .any(|(_, m)| m == "Oh dear, you are dead!")
    })
    .await;
    let died = started.elapsed();
    one.until(
        Duration::from_secs(10),
        "the death screen and the restored hitpoints",
        |c| c.opened.iter().any(|(_, id)| *id == 153) && c.skills.get(&3).map(|s| s.0) == Some(10),
    )
    .await;
    let position = one.position();
    eprintln!(
        "deathone: killed by a giant rat after {:.1} s; death screen 153 opened, hitpoints {:?}, respawned at {position:?}; inventory {:?}",
        died.as_secs_f64(),
        one.skills.get(&3),
        one.containers.get(&93)
    );
    assert_ne!(position, at, "deathone respawned away from the rats");
    assert_eq!(
        one.messages
            .iter()
            .filter(|(_, m)| m == "Oh dear, you are dead!")
            .count(),
        1,
        "finalizeDeath ran once (the death task did not fail and stop again)"
    );
    assert_eq!(
        one.containers.get(&93).and_then(|c| c.get(&0)).map(|i| i.0),
        Some(1205),
        "an administrator keeps its items"
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// A regular player's death leaves a grave: `graveone` (an account
/// registered as a regular player before its first login, one hitpoint,
/// `tutorial:complete`, six items) is killed by a giant rat. The three most
/// valuable items are kept; the other three go into a grave
/// (`GraveController.allowGenerate`, `produceGrave`, `Grave.initialize`):
/// a grave NPC made at runtime (`new Grave()`, `transform`, `init`) stands
/// where it died, the lost items lie there as ground items private to it
/// (with the bones), it is told how long the grave lasts and gets the hint
/// icon from `Grave.tick`; `gravewatch`, standing nearby, sees the grave NPC
/// (id 6565, MEM_PLAQUE) added and none of the private items. Needs the
/// in-process world's account storage (`None` against a running server).
async fn grave_scenario(
    address: std::net::SocketAddr,
    saves: &std::path::Path,
    accounts: Option<&Arc<dyn rs09_server::auth::storage::AccountStorage>>,
) {
    let Some(accounts) = accounts else {
        eprintln!("SKIPPED: the grave scenario registers a regular player's account in-process");
        return;
    };
    let players = saves.join("players");
    let at = [3196, 3206, 0];
    // Bronze dagger, sword, axe, pickaxe, wooden shield, bowl.
    let items = [1205, 1277, 1351, 1265, 1171, 1923];
    let inventory: Vec<(i32, i32, i32)> = items
        .iter()
        .enumerate()
        .map(|(slot, id)| (slot as i32, *id, 1))
        .collect();
    write_save_with(&players, "traceone", "graveone", at, &inventory, |save| {
        save["skills"][3]["lifepoints"] = Value::String("1".into());
        let list = save
            .as_object_mut()
            .unwrap()
            .entry("attributes")
            .or_insert_with(|| Value::Array(vec![]));
        list.as_array_mut()
            .unwrap()
            .push(serde_json::json!({"key": "tutorial:complete", "type": "bool", "value": true}));
    });
    write_save(&players, "traceone", "gravewatch", [3199, 3209, 0], &[]);
    {
        let mut account = rs09_server::auth::account::UserAccountInfo::create_default(0);
        account.username = "graveone".into();
        account.rights = 0;
        accounts
            .store(account.shared())
            .expect("graveone's account");
    }
    let (code, watch) = Client::login(address, "gravewatch").await;
    assert_eq!(code, 2, "gravewatch's login");
    let mut watch = watch.unwrap();
    watch
        .until(Duration::from_secs(10), "gravewatch in the world", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let (code, one) = Client::login(address, "graveone").await;
    assert_eq!(code, 2, "graveone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "graveone's lobby", |c| {
        c.base.is_some() && c.frames.iter().any(|f| f.opcode == 164)
    })
    .await;
    one.send(button(378, 140)).await;
    one.until(Duration::from_secs(10), "graveone in the game world", |c| {
        !c.updates.is_empty()
    })
    .await;
    let started = Instant::now();
    let mut ping = Instant::now();
    let mut died_at = one.position();
    while !one
        .messages
        .iter()
        .any(|(_, m)| m == "Oh dear, you are dead!")
    {
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "graveone was not killed"
        );
        died_at = one.position();
        if ping.elapsed() > Duration::from_secs(5) {
            one.send(frame(93, &[])).await;
            watch.send(frame(93, &[])).await;
            ping = Instant::now();
        }
        one.pump(Duration::from_millis(100)).await;
        watch.pump(Duration::from_millis(100)).await;
    }
    let notice = "<col=ff0000>Because of your current gravestone, you have 2 minutes to get your items back.</col> ";
    one.until(Duration::from_secs(10), "the grave's notice", |c| {
        c.messages.iter().any(|(_, m)| m == notice)
    })
    .await;
    // The kept items are the three most valuable; the other three and the
    // bones lie on the grave's tile, private to graveone.
    one.until(Duration::from_secs(10), "the grave's items", |c| {
        let ground = c.ground_ids(died_at);
        ground.contains(&526) && ground.len() >= 4
    })
    .await;
    let kept: Vec<i32> = one
        .containers
        .get(&93)
        .map(|c| c.values().map(|(id, _)| *id).collect())
        .unwrap_or_default();
    let ground = one.ground_ids(died_at);
    assert_eq!(kept.len(), 3, "three items kept: {kept:?}");
    for id in items {
        assert!(
            kept.contains(&id) || ground.contains(&id),
            "item {id} is neither kept nor in the grave (kept {kept:?}, ground {ground:?})"
        );
    }
    // Grave.tick: the owner's hint icon on the grave NPC (HintIcon 217,
    // slot 0, an NPC target).
    one.until(Duration::from_secs(10), "the grave's hint icon", |c| {
        c.frames
            .iter()
            .any(|f| f.opcode == 217 && f.payload.first() == Some(&1))
    })
    .await;
    // gravewatch sees the grave NPC (MEM_PLAQUE, 6565) and not the items.
    watch
        .until(Duration::from_secs(10), "the grave NPC", |c| {
            c.npcs.actors.values().any(|n| n.type_id == 6565)
        })
        .await;
    assert!(
        watch
            .ground_ids(died_at)
            .iter()
            .all(|id| !items.contains(id)),
        "gravewatch sees graveone's private grave items"
    );
    eprintln!(
        "graveone died at {died_at:?} after {:.1} s: kept {kept:?}, grave items {ground:?}, the grave NPC 6565 seen by gravewatch",
        started.elapsed().as_secs_f64()
    );
    one.writer.close(1000, "logout").await.unwrap();
    watch.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The varps (opcode 60/226 through `updates::ServerUpdate`) a client saw,
/// in order: (frame number, id, value).
fn varps(c: &Client) -> Vec<(usize, i32, i32)> {
    c.frames
        .iter()
        .enumerate()
        .filter_map(|(n, f)| match updates::ServerUpdate::decode(f) {
            Ok(Some(updates::ServerUpdate::Varp { id, value })) => Some((n, id, value)),
            _ => None,
        })
        .collect()
}

/// A weapon special attack: a saved character in the Lumbridge chicken pen
/// wielding a dragon dagger (1215) turns the special bar on (the combat
/// tab's special bar button: CombatTabInterface's pulse, `toggleSpecialBar`,
/// varp 301 = 1), then attacks a chicken: `Player.getSwingHandler` finds
/// the registered Puncture handler (`CombatSwingHandler.register` at boot),
/// its swing drains 25 energy (varp 300 from 1000 to 750) and turns the bar
/// off, and the hits land.
async fn special_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3232, 3294, 0];
    write_save_with(&players, "traceone", "specone", at, &[], |save| {
        save["core_data"]["equipment"] =
            serde_json::json!([{"slot": "3", "id": "1215", "amount": "1", "charge": "1000"}]);
        for skill in [0, 2] {
            save["skills"][skill]["static"] = Value::String("60".into());
            save["skills"][skill]["dynamic"] = Value::String("60".into());
            save["skills"][skill]["experience"] = Value::String("273742.0".into());
        }
    });
    let (code, one) = Client::login(address, "specone").await;
    assert_eq!(code, 2, "specone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "specone's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty() && !c.npcs.actors.is_empty()
    })
    .await;
    let tab = one
        .opened
        .iter()
        .rev()
        .map(|(_, id)| *id)
        .find(|id| (75..=93).contains(id))
        .expect("the weapon tab's interface");
    let from = one.frames.len();
    one.send(button(tab, 10)).await;
    one.until(
        Duration::from_secs(10),
        "the special bar on (varp 301 = 1)",
        |c| {
            varps(c)
                .iter()
                .any(|(n, id, v)| *n >= from && *id == 301 && *v == 1)
        },
    )
    .await;
    eprintln!("specone: the special bar of weapon tab {tab} is on");
    let base = one.base.unwrap();
    let here = one.position();
    let mut chickens: Vec<(usize, [i32; 3])> = one
        .npcs
        .actors
        .iter()
        .filter(|(_, n)| n.type_id == 41 || n.type_id == 1017)
        .map(|(i, n)| (*i, [base[0] + n.tile[0], base[1] + n.tile[1], base[2]]))
        .collect();
    chickens.sort_by_key(|(_, t)| (t[0] - here[0]).abs() + (t[1] - here[1]).abs());
    let from = one.frames.len();
    let mut drained = false;
    for (index, tile) in chickens.into_iter().take(4) {
        eprintln!("specone at {here:?} attacks chicken {index} at {tile:?}");
        one.send(frame(3, &[&la16(index as i32)])).await;
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end
            && !varps(&one)
                .iter()
                .any(|(n, id, v)| *n >= from && *id == 300 && *v == 750)
        {
            one.pump(Duration::from_millis(200)).await;
        }
        if varps(&one)
            .iter()
            .any(|(n, id, v)| *n >= from && *id == 300 && *v == 750)
        {
            drained = true;
            break;
        }
        one.send(frame(93, &[])).await;
    }
    assert!(
        drained,
        "Puncture drained 25 special energy (varp 300 = 750)"
    );
    let bar_off = varps(&one)
        .iter()
        .any(|(n, id, v)| *n >= from && *id == 301 && *v == 0);
    eprintln!(
        "specone: Puncture swung: special energy 750/1000, bar turned off: {bar_off}; varps {:?}",
        varps(&one)
            .iter()
            .filter(|(n, id, _)| *n >= from && (*id == 300 || *id == 301))
            .collect::<Vec<_>>()
    );
    assert!(bar_off, "drainSpecial turned the bar off (varp 301 = 0)");
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// A ranged attack: a saved character in the Lumbridge chicken pen with a
/// shortbow (841) worn and bronze arrows (882) in the ammunition slot uses
/// Attack on a chicken: the range swing handler, the arrow's projectile
/// (`Projectile.send`: zone update 16 in the chunk update), `useAmmo`
/// (`equipment.replace`: one arrow fewer) and a hit mark on the chicken.
async fn ranged_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3232, 3294, 0];
    write_save_with(&players, "traceone", "rangeone", at, &[], |save| {
        save["core_data"]["equipment"] = serde_json::json!([
            {"slot": "3", "id": "841", "amount": "1", "charge": "1000"},
            {"slot": "13", "id": "882", "amount": "100", "charge": "1000"}
        ]);
        save["skills"][4]["static"] = Value::String("40".into());
        save["skills"][4]["dynamic"] = Value::String("40".into());
        save["skills"][4]["experience"] = Value::String("37224.0".into());
    });
    let (code, one) = Client::login(address, "rangeone").await;
    assert_eq!(code, 2, "rangeone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "rangeone's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty() && !c.npcs.actors.is_empty()
    })
    .await;
    let base = one.base.unwrap();
    let here = one.position();
    let mut chickens: Vec<(usize, [i32; 3])> = one
        .npcs
        .actors
        .iter()
        .filter(|(_, n)| n.type_id == 41 || n.type_id == 1017)
        .map(|(i, n)| (*i, [base[0] + n.tile[0], base[1] + n.tile[1], base[2]]))
        .collect();
    chickens.sort_by_key(|(_, t)| (t[0] - here[0]).abs() + (t[1] - here[1]).abs());
    let started = Instant::now();
    let hits_from = one.npc_hits.len();
    let shots_from = one.projectiles.len();
    let mut chosen = None;
    for (index, tile) in chickens.into_iter().take(4) {
        eprintln!("rangeone at {here:?} shoots at chicken {index} at {tile:?}");
        one.send(frame(3, &[&la16(index as i32)])).await;
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end
            && !one.npc_hits[hits_from..]
                .iter()
                .any(|(i, _, _)| *i == index)
        {
            one.pump(Duration::from_millis(200)).await;
        }
        if one.npc_hits[hits_from..]
            .iter()
            .any(|(i, _, _)| *i == index)
        {
            chosen = Some(index);
            break;
        }
        one.send(frame(93, &[])).await;
    }
    let index = chosen.expect("a ranged hit mark on a chicken");
    let shots: Vec<_> = one.projectiles[shots_from..]
        .iter()
        .map(|(_, p)| (p.spot, p.target, p.start_height, p.end_height))
        .collect();
    let arrows = one
        .containers
        .get(&94)
        .and_then(|c| c.get(&13))
        .map(|i| i.1);
    eprintln!(
        "rangeone: hit mark on chicken {index} after {:.1} s; projectiles {shots:?}; arrows left {arrows:?}",
        started.elapsed().as_secs_f64()
    );
    assert!(
        one.projectiles[shots_from..]
            .iter()
            .any(|(_, p)| p.target == index as i32 + 1),
        "an arrow's projectile toward the chicken"
    );
    assert!(
        arrows.is_some_and(|a| a < 100),
        "useAmmo took arrows from the quiver"
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// A combat spell on an NPC: a saved character in the Lumbridge chicken pen
/// with magic 40 and air and mind runes casts Wind Strike (standard book
/// 192, button 1) on the nearest chicken with the spell-on-NPC packet (239):
/// processComponentUseWith, MagicSpell.castSpell, CombatSpell.cast (the
/// spell set on the properties, the combat pulse started), the magic swing
/// handler (meetsRequirements removes one air and one mind rune), the
/// spell's projectile (91) toward the chicken, its hit mark and the magic
/// experience.
async fn magic_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3232, 3294, 0];
    write_save_with(
        &players,
        "traceone",
        "mageone",
        at,
        &[(0, 556, 100), (1, 558, 100)],
        |save| {
            save["skills"][6]["static"] = Value::String("40".into());
            save["skills"][6]["dynamic"] = Value::String("40".into());
            save["skills"][6]["experience"] = Value::String("37224.0".into());
            // Before the tutorial is complete addExperience caps a skill
            // at 175 experience.
            save.as_object_mut()
                .unwrap()
                .entry("attributes")
                .or_insert_with(|| Value::Array(vec![]))
                .as_array_mut()
                .unwrap()
                .push(
                    serde_json::json!({"key": "tutorial:complete", "type": "bool", "value": true}),
                );
        },
    );
    let (code, one) = Client::login(address, "mageone").await;
    assert_eq!(code, 2, "mageone's login");
    let mut one = one.unwrap();
    // tutorial:complete: the welcome screen (378) first; its play button.
    one.until(Duration::from_secs(10), "mageone's lobby", |c| {
        c.base.is_some() && c.frames.iter().any(|f| f.opcode == 164)
    })
    .await;
    one.send(button(378, 140)).await;
    one.until(Duration::from_secs(10), "mageone's rebuild", |c| {
        c.base.is_some()
            && !c.updates.is_empty()
            && !c.npcs.actors.is_empty()
            && c.containers.get(&93).is_some_and(|c| c.len() == 2)
    })
    .await;
    let base = one.base.unwrap();
    let here = one.position();
    let mut chickens: Vec<(usize, [i32; 3])> = one
        .npcs
        .actors
        .iter()
        .filter(|(_, n)| n.type_id == 41 || n.type_id == 1017)
        .map(|(i, n)| (*i, [base[0] + n.tile[0], base[1] + n.tile[1], base[2]]))
        .collect();
    chickens.sort_by_key(|(_, t)| (t[0] - here[0]).abs() + (t[1] - here[1]).abs());
    let started = Instant::now();
    let hits_from = one.npc_hits.len();
    let shots_from = one.projectiles.len();
    let mut chosen = None;
    for (index, tile) in chickens.into_iter().take(4) {
        eprintln!("mageone at {here:?} casts Wind Strike on chicken {index} at {tile:?}");
        // Spell on NPC (239): child, interface, a short the server skips,
        // the NPC index.
        one.send(frame(
            239,
            &[&le16(1), &le16(192), &sa16(0), &la16(index as i32)],
        ))
        .await;
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end
            && !one.npc_hits[hits_from..]
                .iter()
                .any(|(i, _, _)| *i == index)
        {
            one.pump(Duration::from_millis(200)).await;
        }
        if one.npc_hits[hits_from..]
            .iter()
            .any(|(i, _, _)| *i == index)
        {
            chosen = Some(index);
            break;
        }
        one.send(frame(93, &[])).await;
    }
    let index = chosen.unwrap_or_else(|| {
        panic!(
            "a Wind Strike hit mark on a chicken (messages {:?})",
            one.messages
        )
    });
    let runes = |c: &Client, slot: i32| {
        c.containers
            .get(&93)
            .and_then(|c| c.get(&slot))
            .map_or(0, |i| i.1)
    };
    let shots: Vec<_> = one.projectiles[shots_from..]
        .iter()
        .map(|(_, p)| (p.spot, p.target, p.start_height, p.end_height))
        .collect();
    eprintln!(
        "mageone: hit mark on chicken {index} after {:.1} s; projectiles {shots:?}; air {} mind {}; magic {:?}",
        started.elapsed().as_secs_f64(),
        runes(&one, 0),
        runes(&one, 1),
        one.skills.get(&6)
    );
    assert!(
        one.projectiles[shots_from..]
            .iter()
            .any(|(_, p)| p.spot == 91 && p.target == index as i32 + 1),
        "Wind Strike's projectile (91) toward the chicken"
    );
    assert!(runes(&one, 0) < 100, "the cast took air runes");
    assert!(runes(&one, 1) < 100, "the cast took mind runes");
    one.until(Duration::from_secs(5), "magic experience", |c| {
        c.skills.get(&6).is_some_and(|s| s.1 > 37224)
    })
    .await;
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// A saved mage for the effect spells: magic 99, the tutorial complete (so
/// experience is not capped), those runes and a further edit of the save;
/// logged in past the welcome screen with its NPCs in view.
async fn mage(
    address: std::net::SocketAddr,
    saves: &std::path::Path,
    name: &str,
    at: [i32; 3],
    runes: &[(i32, i32, i32)],
    edit: impl FnOnce(&mut Value),
) -> Client {
    write_save_with(
        &saves.join("players"),
        "traceone",
        name,
        at,
        runes,
        |save| {
            save["skills"][6]["static"] = Value::String("99".into());
            save["skills"][6]["dynamic"] = Value::String("99".into());
            save["skills"][6]["experience"] = Value::String("13034431.0".into());
            save.as_object_mut()
                .unwrap()
                .entry("attributes")
                .or_insert_with(|| Value::Array(vec![]))
                .as_array_mut()
                .unwrap()
                .push(
                    serde_json::json!({"key": "tutorial:complete", "type": "bool", "value": true}),
                );
            edit(save);
        },
    );
    let (code, one) = Client::login(address, name).await;
    assert_eq!(code, 2, "{name}'s login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "the mage's lobby", |c| {
        c.base.is_some() && c.frames.iter().any(|f| f.opcode == 164)
    })
    .await;
    one.send(button(378, 140)).await;
    let count = runes.len();
    one.until(Duration::from_secs(10), "the mage's rebuild", |c| {
        c.base.is_some()
            && !c.updates.is_empty()
            && !c.npcs.actors.is_empty()
            && c.containers.get(&93).is_some_and(|c| c.len() == count)
    })
    .await;
    one
}

/// The NPCs of those types in view, nearest first: (index, tile).
fn nearest_npcs(one: &Client, ids: &[i32]) -> Vec<(usize, [i32; 3])> {
    let base = one.base.unwrap();
    let here = one.position();
    let mut found: Vec<(usize, [i32; 3])> = one
        .npcs
        .actors
        .iter()
        .filter(|(_, n)| ids.contains(&n.type_id))
        .map(|(i, n)| (*i, [base[0] + n.tile[0], base[1] + n.tile[1], base[2]]))
        .collect();
    found.sort_by_key(|(_, t)| (t[0] - here[0]).abs() + (t[1] - here[1]).abs());
    found
}

/// The spell-on-NPC packet (239): the spell's button, the spell book's
/// interface, a short the server skips, the NPC index.
fn spell_on_npc(interface: i32, child: i32, index: usize) -> Frame {
    frame(
        239,
        &[
            &le16(child),
            &le16(interface),
            &sa16(0),
            &la16(index as i32),
        ],
    )
}

/// Casts a spell on the nearest NPCs of those types in turn until its
/// projectile flies toward one; that NPC's index.
async fn cast_until_projectile(
    one: &mut Client,
    ids: &[i32],
    interface: i32,
    child: i32,
    spot: i32,
    what: &str,
) -> usize {
    for (index, tile) in nearest_npcs(one, ids).into_iter().take(5) {
        eprintln!("{}: {what} on NPC {index} at {tile:?}", one.name);
        let from = one.projectiles.len();
        // Only a PING (93) refreshes the 20-second timeout.
        one.send(frame(93, &[])).await;
        one.send(spell_on_npc(interface, child, index)).await;
        let flown = |c: &Client| {
            c.projectiles[from..]
                .iter()
                .any(|(_, p)| p.spot == spot && p.target == index as i32 + 1)
        };
        let end = Instant::now() + Duration::from_secs(12);
        while Instant::now() < end && !flown(one) {
            one.pump(Duration::from_millis(200)).await;
        }
        if flown(one) {
            return index;
        }
        eprintln!(
            "{}: no projectile in 12 s (messages {:?})",
            one.name,
            one.messages.iter().rev().take(3).collect::<Vec<_>>()
        );
    }
    panic!(
        "{what}'s projectile ({spot}) toward an NPC (messages {:?})",
        one.messages
    );
}

/// The effect spells (`spells::effects`, world/live/magic.rs), each cast
/// with the spell-on-NPC packet by a saved mage with magic 99 beside
/// Lumbridge's chickens:
///
/// - Confuse (button 2): its projectile (103), its runes gone and exactly
///   its 13 experience (`CurseSpells` gives none for the hit, which its
///   `fireEffect` hides).
/// - Bind (button 12): its projectile (178); `fireEffect` writes
///   `entangleDelay`, so another Bind in the next ticks is answered "You
///   have recently cast a binding spell." and takes no runes.
/// - Claws of Guthix (button 42) with a Guthix staff and a hundred saved
///   casts: the swing runs through `GodSpells.fireEffect` to its hit mark,
///   with its runes and experience.
///
/// What the effects leave in an NPC's levels and timers is checked in
/// process by `live_spell_effects`. The ancient spells are not cast here: a
/// login puts a character without Desert Treasure back on the standard
/// book, and no quest is installed in this world. Teleblock needs a player
/// target.
async fn effects_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    const CHICKENS: [i32; 2] = [41, 1017];
    let coop = [3232, 3294, 0];
    let amount = |c: &Client, slot: i32| {
        c.containers
            .get(&93)
            .and_then(|c| c.get(&slot))
            .map_or(0, |i| i.1)
    };
    let said =
        |c: &Client, from: usize, text: &str| c.messages[from..].iter().any(|(_, m)| m == text);

    // Water, earth, body and nature runes.
    let mut one = mage(
        address,
        saves,
        "hexone",
        coop,
        &[(0, 555, 100), (1, 557, 100), (2, 559, 100), (3, 561, 100)],
        |_| {},
    )
    .await;
    cast_until_projectile(&mut one, &CHICKENS, 192, 2, 103, "Confuse").await;
    one.until(
        Duration::from_secs(5),
        "Confuse's runes and experience",
        |c| amount(c, 2) == 99 && c.skills.get(&6).is_some_and(|s| s.1 == 13_034_431 + 13),
    )
    .await;
    assert_eq!(
        (amount(&one, 0), amount(&one, 1)),
        (97, 98),
        "Confuse's water and earth runes"
    );
    // Bind, then Bind again while entangleDelay runs (tick + 9 + 2). A
    // splash writes no delay, so the next cast is a cast: tried a few times.
    let binding = "You have recently cast a binding spell.";
    let mut refused = false;
    for attempt in 0..5 {
        // The last cast's magic delay, and a binding delay still running.
        one.send(frame(93, &[])).await;
        one.pump(Duration::from_secs(8)).await;
        let index = cast_until_projectile(&mut one, &CHICKENS, 192, 12, 178, "Bind").await;
        let nature = amount(&one, 3);
        let from = one.messages.len();
        // Past the cast's magic delay, inside the binding delay.
        one.send(frame(93, &[])).await;
        one.pump(Duration::from_millis(3600)).await;
        one.send(spell_on_npc(192, 12, index)).await;
        let end = Instant::now() + Duration::from_secs(3);
        while Instant::now() < end && !said(&one, from, binding) {
            one.pump(Duration::from_millis(200)).await;
        }
        if said(&one, from, binding) {
            assert_eq!(amount(&one, 3), nature, "the refused Bind took no runes");
            refused = true;
            eprintln!("hexone: the binding delay answered on attempt {attempt}");
            break;
        }
    }
    assert!(
        refused,
        "a Bind inside entangleDelay is refused (messages {:?})",
        one.messages
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Claws of Guthix: blood, fire and air runes, the Guthix staff worn
    // and the hundred casts that let it be cast outside the Mage Arena.
    let mut one = mage(
        address,
        saves,
        "godone",
        coop,
        &[(0, 565, 100), (1, 554, 100), (2, 556, 100)],
        |save| {
            save["core_data"]["equipment"] = serde_json::json!([
                {"slot": "3", "id": "2416", "amount": "1", "charge": "1000"}
            ]);
            save["activityData"]["godCasts"] = serde_json::json!(["100", "100", "100"]);
        },
    )
    .await;
    // A chicken can still be in someone else's fight ("Someone else is
    // already fighting this."): the cast that went out is the one that
    // took its runes. A splash shows no hit mark, so a cast is repeated
    // until one lands; the hit mark comes after adjustBattleState, where
    // fireEffect runs.
    let mut casts = 0;
    let mut marked = false;
    for _ in 0..4 {
        let blood = amount(&one, 0);
        let mut cast = None;
        for (index, tile) in nearest_npcs(&one, &CHICKENS).into_iter().take(6) {
            eprintln!("godone: Claws of Guthix on chicken {index} at {tile:?}");
            let hits_from = one.npc_hits.len();
            one.send(frame(93, &[])).await;
            one.send(spell_on_npc(192, 42, index)).await;
            let end = Instant::now() + Duration::from_secs(8);
            while Instant::now() < end && amount(&one, 0) == blood {
                one.pump(Duration::from_millis(200)).await;
            }
            if amount(&one, 0) < blood {
                cast = Some((index, hits_from));
                break;
            }
        }
        let (index, hits_from) = cast.unwrap_or_else(|| {
            panic!(
                "a Claws of Guthix cast on a chicken (messages {:?})",
                one.messages
            )
        });
        casts += 1;
        assert_eq!(
            (amount(&one, 0), amount(&one, 1), amount(&one, 2)),
            (100 - 2 * casts, 100 - casts, 100 - 4 * casts),
            "Claws of Guthix's blood, fire and air runes"
        );
        let hit = |c: &Client| c.npc_hits[hits_from..].iter().any(|(i, _, _)| *i == index);
        let end = Instant::now() + Duration::from_secs(6);
        while Instant::now() < end && !hit(&one) {
            one.pump(Duration::from_millis(200)).await;
        }
        if hit(&one) {
            marked = true;
            break;
        }
    }
    assert!(
        marked,
        "a Claws of Guthix hit mark in {casts} casts (messages {:?})",
        one.messages
    );
    one.until(
        Duration::from_secs(5),
        "Claws of Guthix's experience",
        |c| c.skills.get(&6).is_some_and(|s| s.1 >= 13_034_431 + 35),
    )
    .await;
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// A standard teleport: a saved character in Lumbridge with magic 40 and
/// the runes for Varrock Teleport presses it on the magic tab (192, button
/// 15): the magic tab's handler, SpellListeners.run with ModernListeners'
/// Varrock teleport (requires, sendTeleport), TeleportManager.send with the
/// NORMAL teleport pulse (sounds, the animations, the move at its fourth
/// tick); the character arrives at Varrock's square (3213,3424), the runes
/// are gone and 35 magic experience is added.
async fn teleport_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3222, 3218, 0];
    write_save_with(
        &players,
        "traceone",
        "teleone",
        at,
        &[(0, 554, 1), (1, 556, 3), (2, 563, 1)],
        |save| {
            save["skills"][6]["static"] = Value::String("40".into());
            save["skills"][6]["dynamic"] = Value::String("40".into());
            save["skills"][6]["experience"] = Value::String("37224.0".into());
            // Before the tutorial is complete addExperience caps a skill
            // at 175 experience.
            save.as_object_mut()
                .unwrap()
                .entry("attributes")
                .or_insert_with(|| Value::Array(vec![]))
                .as_array_mut()
                .unwrap()
                .push(
                    serde_json::json!({"key": "tutorial:complete", "type": "bool", "value": true}),
                );
        },
    );
    let (code, one) = Client::login(address, "teleone").await;
    assert_eq!(code, 2, "teleone's login");
    let mut one = one.unwrap();
    // tutorial:complete: the welcome screen (378) first; its play button.
    one.until(Duration::from_secs(10), "teleone's lobby", |c| {
        c.base.is_some() && c.frames.iter().any(|f| f.opcode == 164)
    })
    .await;
    one.send(button(378, 140)).await;
    one.until(Duration::from_secs(10), "teleone's rebuild", |c| {
        c.base.is_some()
            && !c.updates.is_empty()
            && c.containers.get(&93).is_some_and(|c| c.len() == 3)
    })
    .await;
    assert_eq!(one.position(), at, "teleone's saved location");
    let started = Instant::now();
    one.send(button(192, 15)).await;
    one.until(Duration::from_secs(10), "the arrival in Varrock", |c| {
        c.position() == [3213, 3424, 0]
    })
    .await;
    eprintln!(
        "teleone: in Varrock after {:.1} s; magic {:?}",
        started.elapsed().as_secs_f64(),
        one.skills.get(&6)
    );
    one.until(
        Duration::from_secs(5),
        "the runes gone and the experience",
        |c| {
            c.containers.get(&93).is_some_and(|c| c.is_empty())
                && c.skills.get(&6).is_some_and(|s| s.1 == 37224 + 35)
        },
    )
    .await;
    // A second press with no runes, after the magic delay (five ticks):
    // the rune message, no teleport. SpellUtils.hasRunes walks its cost
    // HashMap in bucket order (563 before 554 and 556): the law rune.
    one.pump(Duration::from_secs(4)).await;
    let from = one.messages.len();
    one.send(button(192, 15)).await;
    one.until(Duration::from_secs(5), "the missing rune message", |c| {
        c.messages[from..]
            .iter()
            .any(|(_, m)| m == "You don't have enough law runes to cast this spell.")
    })
    .await;
    assert_eq!(one.position(), [3213, 3424, 0], "no second teleport");
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    // A teleport cut short by a disconnection: the pulse is stopped at the
    // logout and never runs against the removed player; the world goes on
    // (a later login is served).
    write_save_with(
        &players,
        "teleone",
        "teletwo",
        at,
        &[(0, 554, 1), (1, 556, 3), (2, 563, 1)],
        |_| {},
    );
    let (code, two) = Client::login(address, "teletwo").await;
    assert_eq!(code, 2, "teletwo's login");
    let mut two = two.unwrap();
    two.until(Duration::from_secs(10), "teletwo's lobby", |c| {
        c.base.is_some() && c.frames.iter().any(|f| f.opcode == 164)
    })
    .await;
    two.send(button(378, 140)).await;
    two.until(Duration::from_secs(10), "teletwo's rebuild", |c| {
        !c.updates.is_empty() && c.containers.get(&93).is_some_and(|c| c.len() == 3)
    })
    .await;
    two.send(button(192, 15)).await;
    two.pump(Duration::from_millis(300)).await;
    two.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(4)).await;
    let (code, three) = Client::login(address, "teleone").await;
    assert_eq!(code, 2, "a login after the cut-short teleport");
    three.unwrap().writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// Combat against an NPC: a saved character in the
/// Lumbridge chicken pen toggles Thick Skin on the prayer tab (interface
/// 271, button 5: varp 83 = 1), then uses Attack (NPC option 2, opcode 3)
/// on the nearest chicken: AttackListener, CombatPulse with the melee swing
/// handler, the impact pulse and ImpactHandler; the hit marks arrive in the
/// NPC update's mask blocks, the chicken dies (DeathTask) and its drop table
/// leaves bones (526) on the ground where it died.
async fn combat_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3232, 3294, 0];
    // Attack and strength 40 (the kill takes a few swings, not minutes).
    write_save_with(&players, "traceone", "fightone", at, &[], |save| {
        for skill in [0, 2] {
            save["skills"][skill]["static"] = Value::String("40".into());
            save["skills"][skill]["dynamic"] = Value::String("40".into());
            save["skills"][skill]["experience"] = Value::String("37224.0".into());
        }
    });
    let (code, one) = Client::login(address, "fightone").await;
    assert_eq!(code, 2, "fightone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "fightone's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty() && !c.npcs.actors.is_empty()
    })
    .await;
    // The prayer tab: Thick Skin.
    let mut payload = ((271i32 << 16) | 5).to_be_bytes().to_vec();
    payload.extend((-1i16).to_be_bytes());
    let thick_skin = {
        let mut wire = vec![];
        updates::varp(83, 1)
            .write(Direction::ServerToClient, None, &mut wire)
            .unwrap();
        wire
    };
    let from = one.frames.len();
    one.send(Frame {
        opcode: 155,
        payload,
    })
    .await;
    one.until(Duration::from_secs(10), "Thick Skin's varp", |c| {
        c.frames[from..].iter().any(|f| {
            let mut wire = vec![];
            f.write(Direction::ServerToClient, None, &mut wire).unwrap();
            wire == thick_skin
        })
    })
    .await;
    eprintln!("fightone: Thick Skin toggled on (varp 83 = 1)");
    // The nearest chickens, in order: a chicken the pen's fence keeps out of
    // reach times the combat pulse out, and the next one is tried.
    let base = one.base.unwrap();
    let here = one.position();
    let mut chickens: Vec<(usize, i32, [i32; 3])> = one
        .npcs
        .actors
        .iter()
        .filter(|(_, n)| n.type_id == 41 || n.type_id == 1017)
        .map(|(i, n)| {
            (
                *i,
                n.type_id,
                [base[0] + n.tile[0], base[1] + n.tile[1], base[2]],
            )
        })
        .collect();
    chickens.sort_by_key(|(_, _, t)| (t[0] - here[0]).abs() + (t[1] - here[1]).abs());
    let started = Instant::now();
    let hits_from = one.npc_hits.len();
    let mut chosen = None;
    for (index, id, tile) in chickens.into_iter().take(4) {
        eprintln!("fightone at {here:?} attacks chicken {index} ({id}) at {tile:?}");
        one.send(frame(3, &[&la16(index as i32)])).await;
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end
            && !one.npc_hits[hits_from..]
                .iter()
                .any(|(i, _, _)| *i == index)
        {
            one.pump(Duration::from_millis(200)).await;
        }
        if one.npc_hits[hits_from..]
            .iter()
            .any(|(i, _, _)| *i == index)
        {
            chosen = Some((index, id));
            break;
        }
        one.send(frame(93, &[])).await;
    }
    let (index, id) = chosen.expect("a hit mark on a chicken");
    let first = one.npc_hits[hits_from..]
        .iter()
        .find(|(i, _, _)| *i == index)
        .copied()
        .unwrap();
    eprintln!(
        "fightone: hit mark on chicken {index} ({id}) after {:.1} s: damage {} type {}",
        started.elapsed().as_secs_f64(),
        first.1,
        first.2
    );
    // The chicken dies (the NPC update removes it while it waits to
    // respawn); the last tile it stood on holds the drop (bones always).
    let mut last = [0, 0, 0];
    let end = Instant::now() + Duration::from_secs(90);
    let mut ping = Instant::now();
    loop {
        if let Some(n) = one.npcs.actors.get(&index) {
            last = [base[0] + n.tile[0], base[1] + n.tile[1], base[2]];
        }
        if one.ground_ids(last).contains(&526) {
            break;
        }
        assert!(
            Instant::now() < end,
            "fightone: no bones where the chicken died"
        );
        if ping.elapsed() > Duration::from_secs(5) {
            one.send(frame(93, &[])).await;
            ping = Instant::now();
        }
        one.pump(Duration::from_millis(200)).await;
    }
    let hits: Vec<_> = one.npc_hits[hits_from..]
        .iter()
        .filter(|(i, _, _)| *i == index)
        .map(|(_, d, k)| (*d, *k))
        .collect();
    eprintln!(
        "fightone: the chicken died after {:.1} s; hit marks {hits:?}; bones (526) on the ground at {last:?}: {:?}",
        started.elapsed().as_secs_f64(),
        one.ground_ids(last)
    );
    // Hitpoints experience (hit * 1.33) above a new character's 1154.
    let hitpoints_xp = one.skills.get(&3).map(|s| s.1).unwrap_or(0);
    eprintln!("fightone: hitpoints experience {hitpoints_xp} after the kill");
    assert!(hitpoints_xp > 1154, "hitpoints experience from the hits");
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// Attacks the nearest NPC of `ids` (option 2, Attack) until one takes a
/// hit, trying up to four; answers its index.
async fn attack_nearest(one: &mut Client, ids: &[i32], what: &str) -> usize {
    let base = one.base.unwrap();
    let here = one.position();
    let mut found: Vec<(usize, [i32; 3])> = one
        .npcs
        .actors
        .iter()
        .filter(|(_, n)| ids.contains(&n.type_id))
        .map(|(i, n)| (*i, [base[0] + n.tile[0], base[1] + n.tile[1], base[2]]))
        .collect();
    found.sort_by_key(|(_, t)| (t[0] - here[0]).abs() + (t[1] - here[1]).abs());
    let hits_from = one.npc_hits.len();
    for (index, tile) in found.into_iter().take(4) {
        eprintln!("{}: attacks {what} {index} at {tile:?}", one.name);
        one.send(frame(3, &[&la16(index as i32)])).await;
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end
            && !one.npc_hits[hits_from..]
                .iter()
                .any(|(i, _, _)| *i == index)
        {
            one.pump(Duration::from_millis(200)).await;
        }
        if one.npc_hits[hits_from..]
            .iter()
            .any(|(i, _, _)| *i == index)
        {
            return index;
        }
        one.send(frame(93, &[])).await;
    }
    panic!("{}: no hit mark on a {what}", one.name);
}

/// NPC subclasses: `ratone` kills a rat (47) west of Draynor, a `RatNPC`
/// the start-up constructed, with Witch's Potion started in its save (stage
/// 10): the death task's stop reaches RatNPC's `finalizeDeath`
/// (world/npc_classes through the live world's virtual dispatch: the base
/// death with its drop, then the quest check against the persisted quest
/// stage) and the rat tail (300) lies where the rat died.
async fn subclass_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3077, 3281, 0];
    write_save_with(&players, "traceone", "ratone", at, &[], |save| {
        for skill in [0, 2] {
            save["skills"][skill]["static"] = Value::String("40".into());
            save["skills"][skill]["dynamic"] = Value::String("40".into());
            save["skills"][skill]["experience"] = Value::String("37224.0".into());
        }
        // Witch's Potion (quest index 31) started: stage 10.
        save["quests"]["questStages"] = serde_json::json!([{"questId": "31", "questStage": "10"}]);
    });
    let (code, one) = Client::login(address, "ratone").await;
    assert_eq!(code, 2, "ratone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "ratone's rats", |c| {
        c.base.is_some() && c.npcs.actors.values().any(|n| n.type_id == 47)
    })
    .await;
    let started = Instant::now();
    let index = attack_nearest(&mut one, &[47], "rat").await;
    let base = one.base.unwrap();
    let mut last = [0, 0, 0];
    let end = Instant::now() + Duration::from_secs(90);
    let mut ping = Instant::now();
    loop {
        if let Some(n) = one.npcs.actors.get(&index) {
            last = [base[0] + n.tile[0], base[1] + n.tile[1], base[2]];
        }
        if one.ground_ids(last).contains(&300) {
            break;
        }
        assert!(
            Instant::now() < end,
            "ratone: no rat tail where the rat died"
        );
        if ping.elapsed() > Duration::from_secs(5) {
            one.send(frame(93, &[])).await;
            ping = Instant::now();
        }
        one.pump(Duration::from_millis(200)).await;
    }
    eprintln!(
        "ratone: the rat (RatNPC) died after {:.1} s; RatNPC's rat tail (300) with Witch's Potion at stage 10, ground at {last:?}: {:?}",
        started.elapsed().as_secs_f64(),
        one.ground_ids(last)
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// Dragons: `dragonone` (Hitpoints, Defence, Attack and Strength 99) attacks
/// a green dragon of the west wilderness (2982,3618; a ChromaticDragonNPC
/// the start-up constructed). The dragon's swing handler is its
/// `combatAction` (MultiSwingHandler, world/dragonfire); when it picks the
/// dragonfire attack the breath's maximum is `calculateDragonfireMaxHit`
/// and the player reads "You are horribly burnt by the fiery breath."
async fn dragon_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [2980, 3615, 0];
    write_save_with(&players, "traceone", "dragonone", at, &[], |save| {
        for skill in [0, 1, 2, 3] {
            save["skills"][skill]["static"] = Value::String("99".into());
            save["skills"][skill]["dynamic"] = Value::String("99".into());
            save["skills"][skill]["experience"] = Value::String("13034431.0".into());
        }
        save["skills"][3]["lifepoints"] = Value::String("99".into());
    });
    let (code, one) = Client::login(address, "dragonone").await;
    assert_eq!(code, 2, "dragonone's login");
    let mut one = one.unwrap();
    let green = [941, 4677, 4678, 4679, 4680];
    one.until(Duration::from_secs(10), "dragonone's green dragons", |c| {
        c.base.is_some() && c.npcs.actors.values().any(|n| green.contains(&n.type_id))
    })
    .await;
    let started = Instant::now();
    attack_nearest(&mut one, &green, "green dragon").await;
    let burnt = "You are horribly burnt by the fiery breath.";
    one.until_pinging(Duration::from_secs(120), "the dragon's breath", |c| {
        c.messages.iter().any(|(_, m)| m == burnt)
    })
    .await;
    eprintln!(
        "dragonone: the green dragon breathed fire after {:.1} s (hitpoints {:?})",
        started.elapsed().as_secs_f64(),
        one.skills.get(&3)
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// NPC aggression: a saved character (combat level 3) next to the giant
/// rats (86, aggressive, combat 3) of the Lumbridge swamp:
/// `AggressiveHandler.selectTarget` picks it and the rat's combat pulse
/// hits it; the hit mark arrives in the player's own update (a miss: the
/// character has not completed the tutorial, so `handleImpact` queues MISS).
async fn aggression_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3196, 3206, 0];
    // The test accounts are administrators (the development configuration):
    // AggressiveHandler passes an administrator over unless the
    // allow_admin_aggression attribute is set.
    write_save_with(&players, "traceone", "ratbait", at, &[], |save| {
        let list = save
            .as_object_mut()
            .unwrap()
            .entry("attributes")
            .or_insert_with(|| Value::Array(vec![]));
        list.as_array_mut().unwrap().push(
            serde_json::json!({"key": "allow_admin_aggression", "type": "bool", "value": true}),
        );
    });
    let (code, one) = Client::login(address, "ratbait").await;
    assert_eq!(code, 2, "ratbait's login");
    let mut one = one.unwrap();
    let started = Instant::now();
    one.until(Duration::from_secs(10), "ratbait's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    // The local player's own mask block (the decoder's SELF entry).
    let me = players::SELF;
    one.until_pinging(Duration::from_secs(90), "a hit mark on ratbait", |c| {
        c.updates.iter().any(|(_, u)| {
            u.extended.iter().any(|e| {
                e.index == me
                    && e.fields
                        .iter()
                        .any(|f| matches!(f, players::Field::Hit { .. }))
            })
        })
    })
    .await;
    let hit = one
        .updates
        .iter()
        .flat_map(|(_, u)| &u.extended)
        .filter(|e| e.index == me)
        .flat_map(|e| &e.fields)
        .find_map(|f| match f {
            players::Field::Hit { damage, kind, bar } => Some((*damage, *kind, *bar)),
            _ => None,
        })
        .unwrap();
    eprintln!(
        "ratbait: attacked by an aggressive NPC after {:.1} s: hit mark {hit:?}",
        started.elapsed().as_secs_f64()
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
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

/// Item on an NPC and on scenery (`processUseWith`): no listener, plugin or
/// UseWithHandler takes a bronze dagger on Lumbridge's shop keeper or on the
/// ladder 36768, so `UseWithHandler.run` runs its `MovementPulse` toward the
/// node (`UseWithHandler$1`): the player walks next to it, then "Nothing
/// interesting happens." on arrival, not before.
async fn use_with_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let nothing = "Nothing interesting happens.";
    // On an NPC: the shop keeper (520) a few tiles north.
    let start = [3214, 3241, 0];
    write_save(&players, "traceone", "useone", start, &[(0, 1205, 1)]);
    let (code, one) = Client::login(address, "useone").await;
    assert_eq!(code, 2, "useone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "useone's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let keeper = |c: &Client| {
        c.npcs
            .actors
            .iter()
            .find(|(_, n)| n.type_id == 520)
            .map(|(i, _)| *i as i32)
    };
    one.until(Duration::from_secs(15), "the shop keeper", |c| {
        keeper(c).is_some()
    })
    .await;
    let index = keeper(&one).unwrap();
    let from = one.messages.len();
    one.send(use_with_npc(1205, 0, index)).await;
    one.until(Duration::from_secs(20), "nothing interesting (NPC)", |c| {
        c.messages[from..].iter().any(|(_, m)| m == nothing)
    })
    .await;
    // Where both stand when the message came (the shop keeper wanders: it
    // may take its own step in the arrival's tick).
    let at = one.position();
    let base = one.base.unwrap();
    let tile = one.npcs.actors.get(&(index as usize)).unwrap().tile;
    let keeper_at = [base[0] + tile[0], base[1] + tile[1], base[2]];
    eprintln!(
        "useone used the dagger on the shop keeper (index {index}) at {keeper_at:?}: walked {start:?} -> {at:?}"
    );
    assert_ne!(at, start, "useone walked toward the shop keeper");
    let with = one.messages[from..]
        .iter()
        .find(|(_, m)| m.starts_with("With: "))
        .map(|(_, m)| m.clone());
    eprintln!("useone's debug line: {with:?}");
    // The debug line carries the shop keeper's tile when the walk arrived:
    // useone is never on it (DestinationFlag.ENTITY). The shop keeper
    // wanders, and MovementPulse leads a walking target
    // (checkForEntityPathInterrupt) and reaches it from a tile around the
    // led point, so the arrival is one to three tiles from it.
    let with = with.expect("useone's debug line for the shop keeper");
    let tile: Vec<i32> = with
        .strip_prefix("With: NPC 520, [")
        .and_then(|rest| rest.strip_suffix(&format!("], {index}")))
        .unwrap_or_else(|| panic!("useone's debug line: {with}"))
        .split(", ")
        .map(|n| n.parse().unwrap())
        .collect();
    let arrival = (at[0] - tile[0]).abs().max((at[1] - tile[1]).abs());
    assert!(
        (1..=3).contains(&arrival),
        "useone near the shop keeper at arrival, not {arrival} tiles from {tile:?}"
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // On another player: usethree walks to usefour, three tiles east.
    let start = [3222, 3219, 0];
    write_save(&players, "traceone", "usethree", start, &[(0, 1205, 1)]);
    write_save(&players, "traceone", "usefour", [3225, 3219, 0], &[]);
    let (code, four) = Client::login(address, "usefour").await;
    assert_eq!(code, 2, "usefour's login");
    let mut four = four.unwrap();
    four.until(Duration::from_secs(10), "usefour's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let (code, three) = Client::login(address, "usethree").await;
    assert_eq!(code, 2, "usethree's login");
    let mut three = three.unwrap();
    three
        .until(Duration::from_secs(10), "usethree's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = three.messages.len();
    three.send(use_with_player(1205, 0, four.index)).await;
    three
        .until(
            Duration::from_secs(20),
            "nothing interesting (player)",
            |c| c.messages[from..].iter().any(|(_, m)| m == nothing),
        )
        .await;
    let at = three.position();
    eprintln!("usethree used the dagger on usefour at [3225, 3219, 0]: walked {start:?} -> {at:?}");
    assert_ne!(at, start, "usethree walked toward usefour");
    assert!(
        (at[0] - 3225).abs().max((at[1] - 3219).abs()) <= 1,
        "usethree beside usefour"
    );
    assert!(
        three.messages[from..]
            .iter()
            .any(|(_, m)| m == "With: Player [name=usefour, getRights()=ADMINISTRATOR]"),
        "usethree's debug line for usefour: {:?}",
        &three.messages[from..]
    );
    three.writer.close(1000, "logout").await.unwrap();
    four.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // On scenery: the ladder 36768 east of the castle, from two tiles west
    // and one south (the walk goes round to the ladder's north side).
    let start = [3227, 3212, 0];
    write_save(&players, "traceone", "usetwo", start, &[(0, 1205, 1)]);
    let (code, two) = Client::login(address, "usetwo").await;
    assert_eq!(code, 2, "usetwo's login");
    let mut two = two.unwrap();
    two.until(Duration::from_secs(10), "usetwo's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = two.messages.len();
    two.send(use_with_scenery(1205, 0, 36768, 3229, 3213)).await;
    two.until(
        Duration::from_secs(20),
        "nothing interesting (scenery)",
        |c| c.messages[from..].iter().any(|(_, m)| m == nothing),
    )
    .await;
    let at = two.position();
    eprintln!(
        "usetwo used the dagger on the ladder 36768 at [3229, 3213, 0]: walked {start:?} -> {at:?}"
    );
    assert_eq!(at, [3229, 3214, 0], "usetwo beside the ladder, on plane 0");
    // The test accounts have debug mode on: UseWithHandler$1's debug lines
    // (Item.toString and Scenery.toString) come before the message.
    let lines: Vec<&str> = two.messages[from..]
        .iter()
        .map(|(_, m)| m.as_str())
        .collect();
    assert_eq!(
        lines,
        [
            "Unhandled use with interaction:",
            "Used: Item id=1205, name=Bronze dagger, amount=1",
            "With: [Scenery 36768, [3229, 3213, 0], type=10, rot=0]",
            nothing,
        ],
        "usetwo's messages"
    );
    two.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    use_content_scenario(address, saves).await;
    use_cook_scenario(address, saves).await;
    use_milk_scenario(address, saves).await;
    use_chest_scenario(address, saves).await;
    use_smelt_scenario(address, saves).await;
    use_smith_scenario(address, saves).await;
    use_object_scenario(address, saves).await;
    use_chop_scenario(address, saves).await;
}

/// Cooking (`world::cooking`, docs/cooking.md) at the fire 5631 west of
/// Lumbridge: a bowl of water and one cooked meat (no roll: hot water and
/// burnt meat), then three raw shrimps through the "How many would you like
/// to cook?" chatbox (307) with "All" (each shrimp is cooked or burnt by
/// its roll at cooking level 1).
/// ChestKeyListener: the chest key on the pirate chest 2079 upstairs in the
/// Blue Moon Inn (`SceneryBuilder.replace` for three ticks).
async fn use_chest_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    let start = [3218, 3395, 1];
    write_save(&players, "traceone", "usechest", start, &[(0, 432, 1)]);
    let (code, chest) = Client::login(address, "usechest").await;
    assert_eq!(code, 2, "usechest's login");
    let mut chest = chest.unwrap();
    chest
        .until(Duration::from_secs(10), "usechest's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = chest.messages.len();
    let frames = chest.frames.len();
    chest.send(use_with_scenery(432, 0, 2079, 3219, 3396)).await;
    chest
        .until_pinging(Duration::from_secs(20), "the pirate message", |c| {
            held(c, 433) == 1 && held(c, 432) == 0
        })
        .await;
    let lines: Vec<&str> = chest.messages[from..]
        .iter()
        .map(|(_, m)| m.as_str())
        .filter(|m| m.contains("chest"))
        .collect();
    assert_eq!(
        lines,
        [
            "You unlock the chest.",
            "All that's in the chest is a message...",
            "You take the message from the chest."
        ],
        "usechest's messages"
    );
    // The open chest 2080 for three ticks, then the closed one again: two
    // chunk update batches (230), the replacement and the restoration.
    chest
        .until_pinging(Duration::from_secs(20), "the chest closes again", |c| {
            c.frames[frames..]
                .iter()
                .filter(|f| f.opcode == 230)
                .count()
                >= 2
        })
        .await;
    // Each batch deletes the object (195) and adds one (179), whose id is
    // the last two bytes (`p2add`): 2080, then 2079 again.
    let added: Vec<i32> = chest.frames[frames..]
        .iter()
        .filter(|f| f.opcode == 230)
        .map(|f| {
            let n = f.payload.len();
            assert!(f.payload.contains(&195) && f.payload.contains(&179));
            (i32::from(f.payload[n - 2]) << 8) | i32::from(f.payload[n - 1].wrapping_sub(128))
        })
        .collect();
    assert_eq!(added[..2], [2080, 2079], "usechest's object updates");
    eprintln!("usechest opened the chest 2079 (2080 for three ticks) and took the message");
    chest.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// FurnaceOptionPlugin, SmeltingInterface and SmeltingPulse at the Al
/// Kharid furnace 11666: an ore used on it (SmeltUseWithHandler through
/// `UseWithHandler.HANDLERS`), then its "Smelt" option, each followed by a
/// bronze button of the chatbox 311.
async fn use_smelt_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    let start = [3274, 3187, 0];
    write_save(
        &players,
        "traceone",
        "usesmelt",
        start,
        &[(0, 436, 1), (1, 438, 1), (2, 436, 1), (3, 438, 1)],
    );
    let (code, smelt) = Client::login(address, "usesmelt").await;
    assert_eq!(code, 2, "usesmelt's login");
    let mut smelt = smelt.unwrap();
    smelt
        .until(Duration::from_secs(10), "usesmelt's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = smelt.messages.len();
    // Tin ore on the furnace: the chatbox, then bronze's "1" (button 16).
    let opened = smelt.opened.len();
    smelt
        .send(use_with_scenery(438, 1, 11666, 3272, 3185))
        .await;
    smelt
        .until(Duration::from_secs(20), "the smelting chatbox", |c| {
            c.opened[opened..].iter().any(|(_, id)| *id == 311)
        })
        .await;
    smelt.send(button(311, 16)).await;
    smelt
        .until_pinging(Duration::from_secs(30), "the first bronze bar", |c| {
            held(c, 2349) == 1
        })
        .await;
    assert_eq!(
        (held(&smelt, 436), held(&smelt, 438)),
        (1, 1),
        "one bar's ores are gone"
    );
    // The option, then bronze's "5" (button 15): the pulse smelts the one
    // bar the ores are for and stops.
    let opened = smelt.opened.len();
    smelt.send(scenery_option2(11666, 3272, 3185)).await;
    smelt
        .until(Duration::from_secs(20), "the smelting chatbox again", |c| {
            c.opened[opened..].iter().any(|(_, id)| *id == 311)
        })
        .await;
    smelt.send(button(311, 15)).await;
    smelt
        .until_pinging(Duration::from_secs(30), "the second bronze bar", |c| {
            held(c, 2349) == 2 && held(c, 436) == 0 && held(c, 438) == 0
        })
        .await;
    smelt
        .until_pinging(Duration::from_secs(30), "the ores ran out", |c| {
            c.messages[from..]
                .iter()
                .any(|(_, m)| m == "You do not have the required ores to make this bar.")
        })
        .await;
    let lines: Vec<&str> = smelt.messages[from..]
        .iter()
        .map(|(_, m)| m.as_str())
        .collect();
    for (line, count) in [
        (
            "You place the required ores and attempt to create a bar of Bronze.",
            2,
        ),
        ("You retrieve a bar of bronze.", 2),
    ] {
        assert_eq!(
            lines.iter().filter(|m| **m == line).count(),
            count,
            "usesmelt's messages: {lines:?}"
        );
    }
    assert!(
        lines
            .iter()
            .any(|m| m.starts_with("Handler=SmeltUseWithHandler, used item=")),
        "the smelt handler's debug line: {lines:?}"
    );
    eprintln!(
        "usesmelt smelted two bronze bars at the furnace 11666 from {:?}",
        smelt.position()
    );
    smelt.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// SmithingPlugin, SmithingBuilder, SmithingInterface and SmithingPulse at
/// a Varrock anvil 2783: a bronze bar used on it (SmithingPlugin through
/// `UseWithHandler.HANDLERS`) opens the interface 300, whose dagger buttons
/// smith one dagger, then the rest of the bars.
async fn use_smith_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    let start = [3186, 3424, 0];
    write_save(
        &players,
        "traceone",
        "usesmith",
        start,
        &[(0, 2347, 1), (1, 2349, 1), (2, 2349, 1), (3, 2349, 1)],
    );
    let (code, smith) = Client::login(address, "usesmith").await;
    assert_eq!(code, 2, "usesmith's login");
    let mut smith = smith.unwrap();
    smith
        .until(Duration::from_secs(10), "usesmith's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = smith.messages.len();
    // A bar on the anvil: the interface, then the dagger's "1" (button 24).
    let opened = smith.opened.len();
    smith
        .send(use_with_scenery(2349, 1, 2783, 3188, 3424))
        .await;
    smith
        .until(Duration::from_secs(20), "the smithing interface", |c| {
            c.opened[opened..].iter().any(|(_, id)| *id == 300)
        })
        .await;
    assert!(
        smith
            .texts
            .iter()
            .any(|(_, _, text)| text == "Bronze Smithing"),
        "the interface names the metal"
    );
    assert!(
        smith
            .texts
            .iter()
            .any(|(_, _, text)| text == "<col=FFFFFF>Dagger"),
        "the dagger's name is white at level 1"
    );
    smith.send(button(300, 24)).await;
    smith
        .until_pinging(Duration::from_secs(30), "the first dagger", |c| {
            held(c, 1205) == 1
        })
        .await;
    assert_eq!(held(&smith, 2349), 2, "one bar is gone");
    // Again, with the dagger's "5" (button 23): five daggers are not
    // covered, so the amount becomes the two bars left.
    let opened = smith.opened.len();
    let slot = smith.containers[&93]
        .iter()
        .find(|(_, (item, _))| *item == 2349)
        .map(|(slot, _)| *slot)
        .expect("a bar is left");
    smith
        .send(use_with_scenery(2349, slot, 2783, 3188, 3424))
        .await;
    smith
        .until(
            Duration::from_secs(20),
            "the smithing interface again",
            |c| c.opened[opened..].iter().any(|(_, id)| *id == 300),
        )
        .await;
    smith.send(button(300, 23)).await;
    smith
        .until_pinging(Duration::from_secs(40), "three daggers", |c| {
            held(c, 1205) == 3 && held(c, 2349) == 0
        })
        .await;
    let lines: Vec<&str> = smith.messages[from..]
        .iter()
        .map(|(_, m)| m.as_str())
        .collect();
    assert_eq!(
        lines
            .iter()
            .filter(|m| **m == "You hammer the bronze and make a bronze dagger.")
            .count(),
        3,
        "usesmith's messages: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|m| m.starts_with("Handler=SmithingPlugin, used item=")),
        "the smithing handler's debug line: {lines:?}"
    );
    eprintln!(
        "usesmith smithed three bronze daggers at the anvil 2783 at [3188, 3424, 0]: walked {start:?} -> {:?}",
        smith.position()
    );
    smith.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// Five small item-on-object listeners (`world::item_on_object`,
/// docs/item-on-object.md): `usesap` by the tree 1278 east of Lumbridge
/// castle (a herring on it, the knife for a bucket of sap, the sap's
/// "empty"), `usepoison` at the Draynor Manor fountain 153 (the poisoned
/// fish food once, then "dead already") and `usecheese` at the Falador
/// hairdresser's treadmill 11677.
async fn use_object_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let slot_of = |c: &Client, id: i32| {
        c.containers[&93]
            .iter()
            .find(|(_, (item, _))| *item == id)
            .map(|(slot, _)| *slot)
            .expect("the item is held")
    };

    let start = [3223, 3232, 0];
    write_save(
        &players,
        "traceone",
        "usesap",
        start,
        &[(0, 946, 1), (1, 1925, 1), (2, 347, 1)],
    );
    let (code, sap) = Client::login(address, "usesap").await;
    assert_eq!(code, 2, "usesap's login");
    let mut sap = sap.unwrap();
    sap.until(Duration::from_secs(10), "usesap's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = sap.messages.len();
    sap.send(use_with_scenery(347, 2, 1278, 3225, 3232)).await;
    sap.until(Duration::from_secs(20), "the herring joke", |c| {
        said(c, from, "This is not the mightiest tree in the forest.")
    })
    .await;
    sap.send(use_with_scenery(946, 0, 1278, 3225, 3232)).await;
    sap.until_pinging(Duration::from_secs(20), "the bucket of sap", |c| {
        held(c, 4687) == 1 && held(c, 1925) == 0
    })
    .await;
    assert!(
        said(
            &sap,
            from,
            "You cut the tree and allow its sap to drip down into your bucket."
        ),
        "usesap's messages: {:?}",
        &sap.messages[from..]
    );
    let slot = slot_of(&sap, 4687);
    sap.send(item_option4(4687, slot)).await;
    sap.until(Duration::from_secs(10), "the emptied bucket", |c| {
        held(c, 1925) == 1 && held(c, 4687) == 0
    })
    .await;
    assert_eq!(slot_of(&sap, 1925), slot, "the bucket is in the sap's slot");
    assert!(
        said(
            &sap,
            from,
            "You empty the contents of the bucket on the floor."
        ),
        "usesap's messages: {:?}",
        &sap.messages[from..]
    );
    assert!(
        !said(&sap, from, "You need a bucket to do that."),
        "a bucket was held"
    );
    eprintln!(
        "usesap collected sap from the tree 1278 at [3225, 3232, 0] and emptied it: walked {start:?} -> {:?}",
        sap.position()
    );
    sap.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let start = [3085, 3332, 0];
    write_save(
        &players,
        "traceone",
        "usepoison",
        start,
        &[(0, 274, 1), (1, 274, 1)],
    );
    let (code, poison) = Client::login(address, "usepoison").await;
    assert_eq!(code, 2, "usepoison's login");
    let mut poison = poison.unwrap();
    poison
        .until(Duration::from_secs(10), "usepoison's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = poison.messages.len();
    poison.send(use_with_scenery(274, 0, 153, 3087, 3334)).await;
    poison
        .until_pinging(Duration::from_secs(30), "the dead piranhas", |c| {
            said(c, from, "... then die and float to the surface.")
        })
        .await;
    let lines: Vec<&str> = poison.messages[from..]
        .iter()
        .map(|(_, m)| m.as_str())
        .filter(|m| !m.starts_with("Handler="))
        .collect();
    assert_eq!(
        lines,
        [
            "You pour the poisoned fish food into the fountain.",
            "The piranhas start eating the food...",
            "... then die and float to the surface.",
        ],
        "usepoison's messages"
    );
    assert_eq!(held(&poison, 274), 1, "one fish food is gone");
    let from = poison.messages.len();
    poison.send(use_with_scenery(274, 1, 153, 3087, 3334)).await;
    poison
        .until_pinging(Duration::from_secs(20), "the second pour's answer", |c| {
            said(c, from, "The piranhas are dead already.")
        })
        .await;
    assert_eq!(held(&poison, 274), 1, "the second fish food stays");
    eprintln!(
        "usepoison poisoned the fountain 153 at [3087, 3334, 0] once: walked {start:?} -> {:?}",
        poison.position()
    );
    poison.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    // `/save:piranhas-killed` went into the save.
    let saved = std::fs::read_to_string(players.join("usepoison.json")).expect("usepoison's save");
    assert!(
        saved.contains("piranhas-killed"),
        "the saved attribute is in usepoison's save"
    );

    let start = [2946, 3379, 0];
    write_save(&players, "traceone", "usecheese", start, &[(0, 1985, 1)]);
    let (code, cheese) = Client::login(address, "usecheese").await;
    assert_eq!(code, 2, "usecheese's login");
    let mut cheese = cheese.unwrap();
    cheese
        .until(Duration::from_secs(10), "usecheese's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = cheese.messages.len();
    cheese
        .send(use_with_scenery(1985, 0, 11677, 2948, 3380))
        .await;
    cheese
        .until_pinging(Duration::from_secs(30), "the thrown cheese", |c| {
            said(
                c,
                from,
                "You throw the cheese to Ridgeley, for which he appears grateful.",
            )
        })
        .await;
    assert_eq!(held(&cheese, 1985), 0, "the cheese is gone");
    // The lambda answers false before its task runs; the scenery path of
    // `InteractionListeners.run` ignores the answer, so nothing else is said.
    assert!(
        !said(&cheese, from, "Nothing interesting happens."),
        "usecheese's messages: {:?}",
        &cheese.messages[from..]
    );
    assert!(
        said(&cheese, from, "Achievement Diary has been updated."),
        "the Falador task is finished: {:?}",
        &cheese.messages[from..]
    );
    eprintln!(
        "usecheese threw cheese at the treadmill 11677 at [2948, 3380, 0]: walked {start:?} -> {:?}",
        cheese.position()
    );
    cheese.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // CoalTruckListener: the Seers' Village truck 2114 at [2696, 3504, 0],
    // its two scenery options and coal used on it.
    let read = |c: &Client, from: usize, what: &str| {
        c.texts[from..].iter().any(|(_, _, t)| t.contains(what))
    };
    let start = [2694, 3505, 0];
    write_save(
        &players,
        "traceone",
        "usecoal",
        start,
        &[(0, 453, 1), (1, 453, 1), (2, 453, 1)],
    );
    let (code, coal) = Client::login(address, "usecoal").await;
    assert_eq!(code, 2, "usecoal's login");
    let mut coal = coal.unwrap();
    coal.until(Duration::from_secs(10), "usecoal's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = coal.texts.len();
    coal.send(scenery_option2(2114, 2696, 3504)).await;
    coal.until_pinging(Duration::from_secs(20), "the empty truck's load", |c| {
        read(c, from, "There is currently 0 coal in the truck.")
    })
    .await;
    // Every coal held goes in, not only the one used.
    coal.send(use_with_scenery(453, 0, 2114, 2696, 3504)).await;
    coal.until_pinging(Duration::from_secs(20), "the deposited coal", |c| {
        held(c, 453) == 0
    })
    .await;
    let from = coal.texts.len();
    coal.send(scenery_option2(2114, 2696, 3504)).await;
    coal.until_pinging(Duration::from_secs(20), "the truck's load", |c| {
        read(c, from, "There is currently 3 coal in the truck.")
    })
    .await;
    coal.send(scenery_option1(2114, 2696, 3504)).await;
    coal.until_pinging(Duration::from_secs(20), "the removed coal", |c| {
        held(c, 453) == 3
    })
    .await;
    let from = coal.texts.len();
    coal.send(scenery_option1(2114, 2696, 3504)).await;
    coal.until_pinging(Duration::from_secs(20), "the empty truck", |c| {
        read(c, from, "The coal truck is empty.")
    })
    .await;
    let slot = slot_of(&coal, 453);
    coal.send(use_with_scenery(453, slot, 2114, 2696, 3504))
        .await;
    coal.until_pinging(Duration::from_secs(20), "the second deposit", |c| {
        held(c, 453) == 0
    })
    .await;
    eprintln!(
        "usecoal filled and emptied the coal truck 2114 at [2696, 3504, 0]: walked {start:?} -> {:?}",
        coal.position()
    );
    coal.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    // `/save:coal-truck-inventory` went into the save.
    let saved = std::fs::read_to_string(players.join("usecoal.json")).expect("usecoal's save");
    assert!(
        saved.contains("coal-truck-inventory"),
        "the saved attribute is in usecoal's save"
    );

    // KaramjaBananaCrateListener: Luthas' crate 2072 at [2943, 3151, 0]
    // with the task given in the save's GlobalData.
    let start = [2941, 3150, 0];
    write_save_with(
        &players,
        "traceone",
        "usecrate",
        start,
        &[(0, 1963, 1), (1, 1963, 1), (2, 1963, 1), (3, 431, 1)],
        |save| save["globalData"]["luthasTask"] = Value::Bool(true),
    );
    let (code, banana) = Client::login(address, "usecrate").await;
    assert_eq!(code, 2, "usecrate's login");
    let mut banana = banana.unwrap();
    banana
        .until(Duration::from_secs(10), "usecrate's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = banana.messages.len();
    banana.send(scenery_option1(2072, 2943, 3151)).await;
    banana
        .until_pinging(Duration::from_secs(20), "the empty crate", |c| {
            said(c, from, "The crate is completely empty.")
        })
        .await;
    let texts = banana.texts.len();
    banana
        .send(use_with_scenery(1963, 0, 2072, 2943, 3151))
        .await;
    banana
        .until_pinging(Duration::from_secs(20), "the packed banana", |c| {
            held(c, 1963) == 2 && read(c, texts, "You pack a banana into the crate.")
        })
        .await;
    // The two-tick lock of the pack ends before the next option.
    tokio::time::sleep(Duration::from_secs(2)).await;
    banana.send(scenery_option1(2072, 2943, 3151)).await;
    banana
        .until_pinging(Duration::from_secs(20), "the crate's count", |c| {
            said(c, from, "The crate has 1 banana inside.")
        })
        .await;
    banana.send(scenery_option2(2072, 2943, 3151)).await;
    banana
        .until_pinging(Duration::from_secs(20), "the filled crate", |c| {
            held(c, 1963) == 0 && said(c, from, "You pack all your bananas into the crate.")
        })
        .await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let texts = banana.texts.len();
    banana
        .send(use_with_scenery(431, 3, 2072, 2943, 3151))
        .await;
    banana
        .until_pinging(Duration::from_secs(20), "the stashed rum", |c| {
            held(c, 431) == 0 && read(c, texts, "You stash the rum in the crate.")
        })
        .await;
    eprintln!(
        "usecrate packed three bananas and the rum into the crate 2072 at [2943, 3151, 0]: walked {start:?} -> {:?}",
        banana.position()
    );
    banana.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let saved: Value = serde_json::from_str(
        &std::fs::read_to_string(players.join("usecrate.json")).expect("usecrate's save"),
    )
    .unwrap();
    assert_eq!(
        saved["globalData"]["karamjaBananas"], "3",
        "the bananas are in usecrate's GlobalData"
    );
    assert!(
        saved.to_string().contains("stashed-rum"),
        "the saved attribute is in usecrate's save"
    );

    // SwampHoleRopeListener: the dark hole 5947 at [3169, 3172, 0].
    let start = [3167, 3172, 0];
    write_save(
        &players,
        "traceone",
        "userope",
        start,
        &[(0, 954, 1), (1, 954, 1)],
    );
    let (code, rope) = Client::login(address, "userope").await;
    assert_eq!(code, 2, "userope's login");
    let mut rope = rope.unwrap();
    rope.until(Duration::from_secs(10), "userope's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let texts = rope.texts.len();
    rope.send(use_with_scenery(954, 0, 5947, 3169, 3172)).await;
    rope.until_pinging(Duration::from_secs(20), "the tied rope", |c| {
        held(c, 954) == 1 && read(c, texts, "You tie the rope to the top of the entrance")
    })
    .await;
    let texts = rope.texts.len();
    rope.send(use_with_scenery(954, 1, 5947, 3169, 3172)).await;
    rope.until_pinging(Duration::from_secs(20), "the second rope's answer", |c| {
        read(c, texts, "There is already a rope tied to the entrance.")
    })
    .await;
    assert_eq!(held(&rope, 954), 1, "the second rope stays");
    eprintln!(
        "userope tied a rope to the dark hole 5947 at [3169, 3172, 0]: walked {start:?} -> {:?}",
        rope.position()
    );
    rope.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let saved: Value = serde_json::from_str(
        &std::fs::read_to_string(players.join("userope.json")).expect("userope's save"),
    )
    .unwrap();
    assert_eq!(
        saved["globalData"]["lumbridgeRope"], true,
        "the rope is in userope's GlobalData"
    );
}

/// AxeOnTree and WoodcuttingSkillPulse (`world::gathering::woodcutting`
/// over the live world): `usechop` uses a bronze axe on the tree 1276 north
/// of Lumbridge castle until it has logs, then stands still (the tree fell).
async fn use_chop_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let start = [3224, 3248, 0];
    write_save(&players, "traceone", "usechop", start, &[(0, 1351, 1)]);
    let (code, chop) = Client::login(address, "usechop").await;
    assert_eq!(code, 2, "usechop's login");
    let mut chop = chop.unwrap();
    chop.until(Duration::from_secs(10), "usechop's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = chop.messages.len();
    let before = chop.skills.get(&8).map_or(0, |s| s.1);
    chop.send(use_with_scenery(1351, 0, 1276, 3226, 3248)).await;
    chop.until_pinging(Duration::from_secs(20), "the first swing", |c| {
        said(c, from, "You swing your axe at the tree...")
    })
    .await;
    // Level 1 with a bronze axe: a roll every four ticks.
    chop.until_pinging(Duration::from_secs(120), "the logs", |c| held(c, 1511) == 1)
        .await;
    assert!(
        said(&chop, from, "You get some logs."),
        "usechop's messages: {:?}",
        &chop.messages[from..]
    );
    let after = chop.skills.get(&8).map_or(0, |s| s.1);
    assert!(
        after > before,
        "woodcutting experience rose ({before} -> {after})"
    );
    // A normal tree always falls: the pulse ended, so no second log comes.
    for _ in 0..40 {
        chop.pump(Duration::from_millis(200)).await;
    }
    assert_eq!(held(&chop, 1511), 1, "one log from one tree");
    eprintln!(
        "usechop chopped the tree 1276 at [3226, 3248, 0] with a bronze axe: walked {start:?} -> {:?}, woodcutting experience {before} -> {after}",
        chop.position()
    );
    chop.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The gathering listeners' interaction scripts, run by the player's
/// ScriptProcessor (`world::live::scripts`): `chopdown` takes the "Chop
/// down" option of the tree 1278 east of Lumbridge castle
/// (WoodcuttingListener) with a bronze axe in the inventory.
async fn gather_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let start = [3223, 3232, 0];
    write_save(&players, "traceone", "chopdown", start, &[(0, 1351, 1)]);
    let (code, chop) = Client::login(address, "chopdown").await;
    assert_eq!(code, 2, "chopdown's login");
    let mut chop = chop.unwrap();
    chop.until(Duration::from_secs(10), "chopdown's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = chop.messages.len();
    let before = chop.skills.get(&8).map_or(0, |s| s.1);
    chop.send(scenery_option1(1278, 3225, 3232)).await;
    chop.until_pinging(Duration::from_secs(20), "the first swing", |c| {
        said(c, from, "You swing your axe at the tree...")
    })
    .await;
    assert_ne!(chop.position(), start, "chopdown walked to the tree");
    // Level 1 with a bronze axe: a roll every four ticks.
    chop.until_pinging(Duration::from_secs(120), "the logs", |c| held(c, 1511) == 1)
        .await;
    assert!(
        said(&chop, from, "You get some logs."),
        "chopdown's messages: {:?}",
        &chop.messages[from..]
    );
    let after = chop.skills.get(&8).map_or(0, |s| s.1);
    assert!(
        after > before,
        "woodcutting experience rose ({before} -> {after})"
    );
    // A normal tree always falls: the script cleared itself, so no second
    // log comes and nothing more is said. (The opening line itself can
    // come twice: the script first runs within its distance of the walk's
    // end, and restarts once the player has stepped there.)
    let lines = chop.messages.len();
    for _ in 0..40 {
        chop.pump(Duration::from_millis(200)).await;
    }
    assert_eq!(held(&chop, 1511), 1, "one log from one tree");
    assert_eq!(
        chop.messages.len(),
        lines,
        "chopdown's messages after the tree fell: {:?}",
        &chop.messages[from..]
    );
    eprintln!(
        "chopdown chopped down the tree 1278 at [3225, 3232, 0] with a bronze axe: walked {start:?} -> {:?}, woodcutting experience {before} -> {after}",
        chop.position()
    );
    chop.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // ProspectListener and MiningListener: the tin rock 11933 in the
    // Lumbridge swamp mine, prospected and then mined with a bronze pickaxe.
    let start = [3226, 3146, 0];
    write_save(&players, "traceone", "minerock", start, &[(0, 1265, 1)]);
    let (code, mine) = Client::login(address, "minerock").await;
    assert_eq!(code, 2, "minerock's login");
    let mut mine = mine.unwrap();
    mine.until(Duration::from_secs(10), "minerock's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = mine.messages.len();
    mine.send(scenery_option2(11933, 3224, 3146)).await;
    mine.until_pinging(Duration::from_secs(20), "the prospect's answer", |c| {
        said(c, from, "This rock contains tin ore.")
    })
    .await;
    assert!(
        said(&mine, from, "You examine the rock for ores..."),
        "minerock's messages: {:?}",
        &mine.messages[from..]
    );
    assert_ne!(mine.position(), start, "minerock walked to the rock");
    let before = mine.skills.get(&14).map_or(0, |s| s.1);
    mine.send(scenery_option1(11933, 3224, 3146)).await;
    mine.until_pinging(Duration::from_secs(20), "the first swing", |c| {
        said(c, from, "You swing your pickaxe at the rock.")
    })
    .await;
    // Level 1 with a bronze pickaxe: a roll every four ticks.
    mine.until_pinging(Duration::from_secs(300), "the tin ore", |c| {
        held(c, 438) == 1
    })
    .await;
    assert!(
        said(&mine, from, "You manage to mine some tin."),
        "minerock's messages: {:?}",
        &mine.messages[from..]
    );
    let after = mine.skills.get(&14).map_or(0, |s| s.1);
    assert!(
        after > before,
        "mining experience rose ({before} -> {after})"
    );
    // The rock is depleted: the script ended, so no second ore comes and
    // nothing more is said.
    let lines = mine.messages.len();
    for _ in 0..40 {
        mine.pump(Duration::from_millis(200)).await;
    }
    assert_eq!(held(&mine, 438), 1, "one ore from one rock");
    assert_eq!(
        mine.messages.len(),
        lines,
        "minerock's messages after the rock was mined: {:?}",
        &mine.messages[from..]
    );
    eprintln!(
        "minerock prospected and mined the tin rock 11933 at [3224, 3146, 0] with a bronze pickaxe: walked {start:?} -> {:?}, mining experience {before} -> {after}",
        mine.position()
    );
    mine.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // FishingListener: a net fishing spot (316) at Draynor, with a small
    // fishing net.
    let start = [3088, 3228, 0];
    write_save(&players, "traceone", "netfish", start, &[(0, 303, 1)]);
    let (code, fish) = Client::login(address, "netfish").await;
    assert_eq!(code, 2, "netfish's login");
    let mut fish = fish.unwrap();
    fish.until(Duration::from_secs(10), "netfish's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let spot = |c: &Client| {
        c.npcs
            .actors
            .iter()
            .find(|(_, n)| n.type_id == 316)
            .map(|(i, _)| *i as i32)
    };
    fish.until(Duration::from_secs(15), "a net fishing spot", |c| {
        spot(c).is_some()
    })
    .await;
    let index = spot(&fish).unwrap();
    let from = fish.messages.len();
    let before = fish.skills.get(&10).map_or(0, |s| s.1);
    fish.send(npc_option1(index)).await;
    fish.until_pinging(Duration::from_secs(20), "the cast", |c| {
        said(c, from, "You cast out your net...")
    })
    .await;
    // Level 1: a roll every five ticks.
    fish.until_pinging(Duration::from_secs(300), "the shrimps", |c| {
        held(c, 317) >= 1
    })
    .await;
    assert!(
        said(&fish, from, "You catch some shrimps."),
        "netfish's messages: {:?}",
        &fish.messages[from..]
    );
    let after = fish.skills.get(&10).map_or(0, |s| s.1);
    assert!(
        after > before,
        "fishing experience rose ({before} -> {after})"
    );
    // The script keeps running: a second catch comes without another click.
    fish.until_pinging(Duration::from_secs(300), "a second catch", |c| {
        held(c, 317) >= 2
    })
    .await;
    eprintln!(
        "netfish netted shrimps at the fishing spot 316 with a small fishing net: walked {start:?} -> {:?}, fishing experience {before} -> {after}",
        fish.position()
    );
    fish.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // FishingNPC: a fishing spot moves to a random tile of its area on the
    // first tick its region is active (its switch delay starts at 0) unless
    // an NPC stands there. The Fishing Guild lists 36 tiles for the nine
    // spots it owns, so some spot stands off every spawn tile right away.
    let spawns = [
        [2607, 3416],
        [2604, 3417],
        [2602, 3419],
        [2602, 3411],
        [2605, 3413],
        [2609, 3416],
        [2610, 3413],
        [2605, 3421],
        [2602, 3423],
        [2603, 3426],
        [2612, 3411],
    ];
    let start = [2598, 3418, 0];
    write_save(&players, "traceone", "movespot", start, &[]);
    let (code, guild) = Client::login(address, "movespot").await;
    assert_eq!(code, 2, "movespot's login");
    let mut guild = guild.unwrap();
    guild
        .until(Duration::from_secs(10), "movespot's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let moved = |c: &Client| {
        let base = c.base?;
        c.npcs
            .actors
            .values()
            .filter(|n| [326, 333, 334].contains(&n.type_id))
            .map(|n| [base[0] + n.tile[0], base[1] + n.tile[1]])
            .find(|tile| !spawns.contains(tile))
    };
    guild
        .until_pinging(Duration::from_secs(30), "a moved fishing spot", |c| {
            moved(c).is_some()
        })
        .await;
    let tile = moved(&guild).unwrap();
    assert!(
        (2599..=2612).contains(&tile[0]) && (3410..=3426).contains(&tile[1]),
        "the moved spot stands on a Fishing Guild tile: {tile:?}"
    );
    eprintln!("movespot saw a Fishing Guild fishing spot off its spawn tile, at {tile:?}");
    guild.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Barbarian fishing: SpotManager's five fishing spots (2722) below the
    // Baxtorian Falls, "Use-rod" with a barbarian rod and feathers at
    // Fishing 48, Agility 15 and Strength 15, then the knife on the catch.
    let start = [2505, 3495, 0];
    let inventory = [(0, 11323, 1), (1, 314, 50), (2, 946, 1)];
    write_save_with(
        &players,
        "traceone",
        "barbfish",
        start,
        &inventory,
        |save| {
            for (skill, level, experience) in [
                (10, "48", "83014.0"),
                (16, "15", "2411.0"),
                (2, "15", "2411.0"),
            ] {
                save["skills"][skill]["static"] = Value::String(level.into());
                save["skills"][skill]["dynamic"] = Value::String(level.into());
                save["skills"][skill]["experience"] = Value::String(experience.into());
            }
        },
    );
    let (code, barb) = Client::login(address, "barbfish").await;
    assert_eq!(code, 2, "barbfish's login");
    let mut barb = barb.unwrap();
    barb.until(Duration::from_secs(10), "barbfish's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let spots = |c: &Client| {
        let Some(base) = c.base else {
            return Vec::new();
        };
        c.npcs
            .actors
            .iter()
            .filter(|(_, n)| n.type_id == 2722)
            .map(|(i, n)| (*i as i32, [base[0] + n.tile[0], base[1] + n.tile[1]]))
            .collect::<Vec<_>>()
    };
    barb.until_pinging(
        Duration::from_secs(15),
        "the barbarian fishing spots",
        |c| !spots(c).is_empty(),
    )
    .await;
    let seen = spots(&barb);
    let listed = [
        [2506, 3494],
        [2504, 3497],
        [2500, 3506],
        [2500, 3509],
        [2500, 3512],
        [2504, 3516],
    ];
    assert!(
        seen.iter().all(|(_, tile)| listed.contains(tile)),
        "every barbarian fishing spot stands on a SpotManager location: {seen:?}"
    );
    // The nearest spot.
    let (index, tile) = *seen
        .iter()
        .min_by_key(|(_, t)| (t[0] - start[0]).abs().max((t[1] - start[1]).abs()))
        .unwrap();
    let amount = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| {
                slots
                    .values()
                    .filter(|(item, _)| *item == id)
                    .map(|(_, n)| *n)
                    .sum::<i32>()
            })
            .unwrap_or(0)
    };
    let from = barb.messages.len();
    let before = [10, 16, 2].map(|skill| barb.skills.get(&skill).map_or(0, |s| s.1));
    barb.send(npc_option1(index)).await;
    barb.until_pinging(Duration::from_secs(30), "the cast", |c| {
        said(c, from, "You attempt to catch a fish.")
    })
    .await;
    assert!(
        said(&barb, from, "You cast out your line..."),
        "barbfish's messages: {:?}",
        &barb.messages[from..]
    );
    // The first pulse only sets the delay; then a roll every five ticks.
    barb.until_pinging(Duration::from_secs(300), "a leaping trout", |c| {
        held(c, 11328) >= 1
    })
    .await;
    assert!(
        said(&barb, from, "You catch a leaping trout."),
        "barbfish's messages: {:?}",
        &barb.messages[from..]
    );
    assert_eq!(amount(&barb, 314), 49, "one feather was used as bait");
    // The catch adds experience to the three skills; this save has not
    // completed the tutorial, so `Skills.addExperience` caps each at 175.
    let after = [10, 16, 2].map(|skill| barb.skills.get(&skill).map_or(0, |s| s.1));
    assert_eq!(
        after,
        [175, 175, 175],
        "fishing, agility and strength experience after the catch (before: {before:?})"
    );
    // KnifeWithFish: FishCuttingPulse replaces the fishing pulse.
    let slot = barb.slot_of(93, 11328).unwrap();
    let trout = held(&barb, 11328);
    let from = barb.messages.len();
    let cooking = barb.skills.get(&7).map_or(0, |s| s.1);
    barb.send(use_item_on_item(946, 2, 11328, slot)).await;
    barb.until_pinging(Duration::from_secs(20), "the cut fish", |c| {
        held(c, 11328) < trout
            && (said(c, from, "usable as bait.")
                || said(c, from, "useless fragments, which you discard.")
                || said(
                    c,
                    from,
                    "You cut open the fish and extract some fish cuts and roe.",
                ))
    })
    .await;
    let roe = amount(&barb, 11324);
    let offcuts = amount(&barb, 11334);
    let cooked = barb.skills.get(&7).map_or(0, |s| s.1);
    assert_eq!(
        roe > 0,
        cooked > cooking,
        "roe comes with cooking experience"
    );
    assert!(offcuts == 0 || roe > 0, "offcuts only come with roe");
    eprintln!(
        "barbfish used the rod on the spot 2722 at {tile:?} from {start:?} (stood at {:?}), caught a leaping trout (experience {before:?} -> {after:?}) and cut it: roe {roe}, offcuts {offcuts}",
        barb.position()
    );
    barb.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Otto Godblessed (2725): OttoGodblessedDialogue's "talk-to" opens the
    // DialogueLabeller conversation. A save with no barbarian training
    // attribute takes the first-time branch; "You think so?" leads to the
    // training menu, the rod topic sets `barbtraining:rod` to 1, and
    // leaving hands out the Barbarian skills book (11340). The bed (25268)
    // then gives the barbarian rod.
    let start = [2501, 3488, 0];
    write_save(&players, "traceone", "ottotalk", start, &[]);
    let (code, otto) = Client::login(address, "ottotalk").await;
    assert_eq!(code, 2, "ottotalk's login");
    let mut otto = otto.unwrap();
    otto.until(Duration::from_secs(10), "ottotalk's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let ottos = |c: &Client| {
        c.npcs
            .actors
            .iter()
            .find(|(_, n)| n.type_id == 2725)
            .map(|(i, _)| *i as i32)
    };
    otto.until_pinging(Duration::from_secs(15), "Otto Godblessed", |c| {
        ottos(c).is_some()
    })
    .await;
    let index = ottos(&otto).unwrap();
    let picks = [
        "You think so?",
        "Are there any ways to use a fishing rod which I might learn?",
        "I have no more questions at this time.",
    ];
    // The chat interfaces (the player's 64-67, an NPC's 241-244) and the
    // option lists; other texts (the music tab's track name) are not pages.
    let dialogue = |component: i32| {
        let interface = component >> 16;
        (64..=67).contains(&interface) || (224..=244).contains(&interface)
    };
    let mut picked = Vec::new();
    let mut transcript = Vec::new();
    let mut from = otto.texts.len();
    otto.send(npc_option1(index)).await;
    for step in 0..40 {
        otto.until_pinging(Duration::from_secs(20), "Otto's next page", |c| {
            c.texts[from..].iter().any(|(_, p, _)| dialogue(*p))
                || held(c, 11340) >= 1
                || c.messages.iter().any(|(_, m)| m == "I can't reach that.")
        })
        .await;
        assert!(
            !otto
                .messages
                .iter()
                .any(|(_, m)| m == "I can't reach that."),
            "ottotalk could not reach Otto from {:?}",
            otto.position()
        );
        if !otto.texts[from..].iter().any(|(_, p, _)| dialogue(*p)) {
            break;
        }
        // The rest of the page's texts.
        otto.pump(Duration::from_millis(300)).await;
        let page: Vec<(i32, String)> = otto.texts[from..]
            .iter()
            .filter(|(_, p, _)| dialogue(*p))
            .map(|(_, p, t)| (*p, t.clone()))
            .collect();
        from = otto.texts.len();
        let interface = page.last().unwrap().0 >> 16;
        let chat = (64..=67).contains(&interface) || (241..=244).contains(&interface);
        let line = format!(
            "{interface}: {}",
            page.iter()
                .map(|(_, t)| t.as_str())
                .collect::<Vec<_>>()
                .join(" | ")
        );
        if std::env::var_os("RS09_LIVE_VERBOSE").is_some() {
            eprintln!("ottotalk step {step}: {line}");
        }
        transcript.push(line);
        if chat {
            // An NPC's or the player's lines: Continue is the child after
            // the lines.
            let lines = if interface >= 241 {
                interface - 241
            } else {
                interface - 64
            };
            otto.send(continue_option(interface, lines + 5)).await;
        } else {
            let (component, text) = page
                .iter()
                .find(|(_, t)| picks.contains(&t.as_str()))
                .unwrap_or_else(|| panic!("step {step}: no option to pick in {page:?}"));
            picked.push(text.clone());
            otto.send(continue_option(component >> 16, component & 0xffff))
                .await;
        }
    }
    eprintln!("ottotalk's conversation: {transcript:#?}");
    assert_eq!(picked, picks, "the options chosen: {transcript:#?}");
    assert!(
        transcript
            .iter()
            .any(|page| page.contains("Take the rod from under the bed in my")),
        "Otto taught the rod: {transcript:#?}"
    );
    assert_eq!(held(&otto, 11340), 1, "the Barbarian skills book");
    assert_eq!(held(&otto, 11323), 0, "no rod before the bed");
    let from = otto.messages.len();
    otto.send(scenery_option1(25268, OTTO_BED[0], OTTO_BED[1]))
        .await;
    otto.until_pinging(Duration::from_secs(20), "the rod from the bed", |c| {
        held(c, 11323) >= 1
    })
    .await;
    assert!(
        said(
            &otto,
            from,
            "You find a heavy fishing rod under the bed and take it."
        ),
        "ottotalk's messages: {:?}",
        &otto.messages[from..]
    );
    eprintln!(
        "ottotalk learned barbarian fishing from Otto in {} pages, took the book and found the rod under the bed",
        transcript.len()
    );
    otto.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Firemaking: a tinderbox used with logs. The log goes to the ground,
    // FireMakingPulse rolls every third tick (at level 1 normal logs always
    // light), the fire replaces the log, the player steps off it and gains
    // 40 experience; the fire burns 180 ticks and leaves ashes.
    let start = [3226, 3146, 0];
    let inventory = [(0, 590, 1), (1, 1511, 1), (2, 1511, 1)];
    write_save(&players, "traceone", "firelog", start, &inventory);
    let (code, fire) = Client::login(address, "firelog").await;
    assert_eq!(code, 2, "firelog's login");
    let mut fire = fire.unwrap();
    fire.until(Duration::from_secs(10), "firelog's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = fire.messages.len();
    fire.send(use_item_on_item(590, 0, 1511, 1)).await;
    fire.until_pinging(Duration::from_secs(20), "the log on the ground", |c| {
        said(c, from, "You attempt to light the logs..")
    })
    .await;
    assert_eq!(held(&fire, 1511), 1, "one log left the inventory");
    fire.until_pinging(Duration::from_secs(30), "the fire", |c| {
        said(c, from, "The fire catches and the logs begin to burn.")
            && c.skills.get(&11).is_some_and(|s| s.1 == 40)
    })
    .await;
    fire.until_pinging(Duration::from_secs(10), "the step off the fire", |c| {
        c.position() != start
    })
    .await;
    let stood = fire.position();
    assert_eq!(
        (stood[0] - start[0]).abs() + (stood[1] - start[1]).abs(),
        1,
        "firelog stepped one tile off the fire: {stood:?}"
    );
    assert!(
        !fire.ground_ids(start).contains(&1511),
        "the log on the ground was burnt"
    );
    // Fires cannot be stacked: the fire is an object on its tile. The
    // second log is lit from the new tile at once (`last-firemake`) or on a
    // later roll.
    let from = fire.messages.len();
    fire.send(use_item_on_item(590, 0, 1511, 2)).await;
    fire.until_pinging(Duration::from_secs(30), "the second fire", |c| {
        said(c, from, "The fire catches and the logs begin to burn.")
            && c.skills.get(&11).is_some_and(|s| s.1 == 80)
    })
    .await;
    assert_eq!(held(&fire, 1511), 0, "both logs were burnt");
    // 180 ticks later the first fire goes out and leaves its ashes.
    fire.until_pinging(Duration::from_secs(150), "the ashes", |c| {
        c.ground_ids(start).contains(&592)
    })
    .await;
    eprintln!(
        "firelog lit two logs with a tinderbox from {start:?} (stepped to {stood:?}), firemaking experience 80, and the first fire left ashes"
    );
    fire.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // BoneBuryListener: the bones' "Bury" option digs at once and the
    // STRONG script a tick later empties the slot and rewards prayer
    // experience (4.5 for bones, 15 for big bones). The SKILLING clock
    // holds a second click for two ticks.
    let inventory = [(0, 526, 1), (1, 526, 1), (2, 532, 1)];
    write_save(&players, "traceone", "burybones", start, &inventory);
    let (code, bury) = Client::login(address, "burybones").await;
    assert_eq!(code, 2, "burybones' login");
    let mut bury = bury.unwrap();
    bury.until(Duration::from_secs(10), "burybones' rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = bury.messages.len();
    let count = |c: &Client, from: usize, line: &str| {
        c.messages[from..].iter().filter(|(_, m)| m == line).count()
    };
    // Two clicks in one tick: the second finds the clock set and is dropped.
    bury.send(item_option1(526, 0)).await;
    bury.send(item_option1(526, 1)).await;
    bury.until_pinging(Duration::from_secs(20), "the first bones buried", |c| {
        said(c, from, "You bury the bones.") && c.skills.get(&5).is_some_and(|s| s.1 == 4)
    })
    .await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    bury.until_pinging(Duration::from_secs(5), "burybones' quiet", |_| true)
        .await;
    assert_eq!(
        (held(&bury, 526), held(&bury, 532)),
        (1, 1),
        "one click buried one set of bones"
    );
    assert_eq!(
        count(&bury, from, "You dig a hole in the ground."),
        1,
        "the second click waited for the skilling clock"
    );
    for (id, slot, experience) in [(526, 1, 9), (532, 2, 24)] {
        let from = bury.messages.len();
        bury.send(item_option1(id, slot)).await;
        bury.until_pinging(Duration::from_secs(20), "more bones buried", |c| {
            said(c, from, "You dig a hole in the ground.")
                && said(c, from, "You bury the bones.")
                && c.skills.get(&5).is_some_and(|s| s.1 == experience)
        })
        .await;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    assert_eq!(
        (held(&bury, 526), held(&bury, 532)),
        (0, 0),
        "every set of bones was buried"
    );
    assert_eq!(bury.position(), start, "burying does not move the player");
    eprintln!("burybones buried two bones and big bones at {start:?}, prayer experience 24");
    bury.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // PrayerAltarListener: Lumbridge church's altar 36972 recharges a drained
    // player (level 43, 3 points left), and says so when the points are full.
    let church = [ALTAR_STAND[0], ALTAR_STAND[1], 0];
    write_save_with(&players, "traceone", "prayaltar", church, &[], |save| {
        save["skills"][5]["static"] = Value::String("43".into());
        save["skills"][5]["dynamic"] = Value::String("43".into());
        save["skills"][5]["experience"] = Value::String("51000.0".into());
        save["skills"][5]["prayerPoints"] = Value::String("3.0".into());
    });
    let (code, pray) = Client::login(address, "prayaltar").await;
    assert_eq!(code, 2, "prayaltar's login");
    let mut pray = pray.unwrap();
    pray.until(Duration::from_secs(10), "prayaltar's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    assert_eq!(
        pray.skills.get(&5).map(|s| s.0),
        Some(3),
        "prayaltar starts with three prayer points"
    );
    let from = pray.messages.len();
    pray.send(scenery_option1(36972, ALTAR[0], ALTAR[1])).await;
    pray.until_pinging(Duration::from_secs(30), "the recharge", |c| {
        said(c, from, "You recharge your prayer points.")
            && c.skills.get(&5).is_some_and(|s| s.0 == 43)
    })
    .await;
    // The lock of three ticks runs out, then the altar has nothing to give.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let from = pray.messages.len();
    pray.send(scenery_option1(36972, ALTAR[0], ALTAR[1])).await;
    pray.until_pinging(Duration::from_secs(30), "the full points", |c| {
        said(c, from, "You already have full prayer points.")
    })
    .await;
    assert!(
        !said(&pray, from, "You recharge your prayer points."),
        "a full player is not recharged"
    );
    eprintln!("prayaltar recharged 3 to 43 prayer points at the altar 36972 at {ALTAR:?}");
    pray.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // LogCraftableListeners: a knife on logs opens the four-option make-X
    // dialogue (305). A shortbow needs level 5 and is refused with a
    // dialogue; "Make 10" arrow shafts cuts the three logs, one every three
    // ticks, 15 shafts and 5 experience each.
    let inventory = [(0, 946, 1), (1, 1511, 1), (2, 1511, 1), (3, 1511, 1)];
    write_save(&players, "traceone", "fletchlog", start, &inventory);
    let (code, fletch) = Client::login(address, "fletchlog").await;
    assert_eq!(code, 2, "fletchlog's login");
    let mut fletch = fletch.unwrap();
    fletch
        .until(Duration::from_secs(10), "fletchlog's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let shown = |c: &Client, from: usize, pointer: i32, what: &str| {
        c.texts[from..]
            .iter()
            .any(|(_, at, t)| *at == pointer && t.contains(what))
    };
    let read = |c: &Client, from: usize, what: &str| {
        c.texts[from..].iter().any(|(_, _, t)| t.contains(what))
    };
    let texts = fletch.texts.len();
    fletch.send(use_item_on_item(946, 0, 1511, 1)).await;
    fletch
        .until_pinging(Duration::from_secs(20), "the make-X dialogue", |c| {
            shown(c, texts, (305 << 16) | 9, "Arrow shaft")
                && shown(c, texts, (305 << 16) | 13, "Shortbow")
                && shown(c, texts, (305 << 16) | 17, "Longbow")
                && shown(c, texts, (305 << 16) | 21, "Wooden stock")
        })
        .await;
    // "Make 1" of the second item (the interpreter hands its plugin the
    // component less one).
    let texts = fletch.texts.len();
    fletch.send(continue_option(305, 13)).await;
    fletch
        .until_pinging(Duration::from_secs(20), "the level refusal", |c| {
            read(c, texts, "You need a Fletching skill of 5 or above to")
        })
        .await;
    assert_eq!(held(&fletch, 1511), 3, "no log was cut for the shortbow");
    // "Make 10" of the first item.
    let texts = fletch.texts.len();
    let from = fletch.messages.len();
    fletch.send(use_item_on_item(946, 0, 1511, 1)).await;
    fletch
        .until_pinging(Duration::from_secs(20), "the second make-X dialogue", |c| {
            shown(c, texts, (305 << 16) | 9, "Arrow shaft")
        })
        .await;
    fletch.send(continue_option(305, 7)).await;
    fletch
        .until_pinging(Duration::from_secs(40), "the three logs cut", |c| {
            held(c, 1511) == 0 && c.skills.get(&9).is_some_and(|s| s.1 == 15)
        })
        .await;
    let cuts = fletch.messages[from..]
        .iter()
        .filter(|(_, m)| m == "You carefully cut the wood into 15 arrow shafts.")
        .count();
    assert_eq!(cuts, 3, "one message a log");
    let shafts = fletch.containers[&93]
        .values()
        .find(|(item, _)| *item == 52)
        .map(|(_, amount)| *amount);
    assert_eq!(shafts, Some(45), "the shafts stack");
    eprintln!("fletchlog cut three logs into 45 arrow shafts, fletching experience 15");
    fletch.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // StringingListeners and ArrowListeners: the wrong string is refused
    // with a message; a bow string on a shortbow (u) opens the one-option
    // dialogue (309) and "Make 1" strings it; a shaft on feathers opens the
    // make-sets dialogue (582) and one set is 15 headless arrows; five sets
    // of bronze arrows stop after the first when the headless arrows run
    // out.
    let inventory = [
        (0, 50, 1),
        (1, 1777, 1),
        (2, 9438, 1),
        (3, 52, 20),
        (4, 314, 20),
        (5, 39, 20),
    ];
    write_save_with(
        &players,
        "traceone",
        "fletcharrow",
        start,
        &inventory,
        |save| {
            save["skills"][9]["static"] = Value::String("5".into());
            save["skills"][9]["dynamic"] = Value::String("5".into());
        },
    );
    let (code, arrow) = Client::login(address, "fletcharrow").await;
    assert_eq!(code, 2, "fletcharrow's login");
    let mut arrow = arrow.unwrap();
    arrow
        .until(Duration::from_secs(10), "fletcharrow's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let amount = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .and_then(|slots| slots.values().find(|(item, _)| *item == id))
            .map(|(_, amount)| *amount)
            .unwrap_or(0)
    };
    let from = arrow.messages.len();
    arrow.send(use_item_on_item(9438, 2, 50, 0)).await;
    arrow
        .until_pinging(Duration::from_secs(20), "the wrong string", |c| {
            said(c, from, "That's not the right kind of string for this.")
        })
        .await;
    let texts = arrow.texts.len();
    arrow.send(use_item_on_item(1777, 1, 50, 0)).await;
    arrow
        .until_pinging(Duration::from_secs(20), "the stringing dialogue", |c| {
            shown(c, texts, (309 << 16) | 6, "Shortbow")
        })
        .await;
    arrow.send(continue_option(309, 6)).await;
    arrow
        .until_pinging(Duration::from_secs(20), "the strung shortbow", |c| {
            amount(c, 841) == 1 && said(c, from, "You add a string to the bow.")
        })
        .await;
    assert_eq!(
        (amount(&arrow, 50), amount(&arrow, 1777)),
        (0, 0),
        "the bow and the string are used"
    );
    let texts = arrow.texts.len();
    arrow.send(use_item_on_item(52, 3, 314, 4)).await;
    arrow
        .until_pinging(Duration::from_secs(20), "the headless dialogue", |c| {
            shown(c, texts, (582 << 16) | 5, "Headless arrow")
        })
        .await;
    arrow.send(continue_option(582, 6)).await;
    arrow
        .until_pinging(Duration::from_secs(20), "one set of headless arrows", |c| {
            amount(c, 53) == 15 && said(c, from, "You attach feathers to 15 arrow shafts.")
        })
        .await;
    assert_eq!(
        (amount(&arrow, 52), amount(&arrow, 314)),
        (5, 5),
        "one set takes 15 shafts and 15 feathers"
    );
    let texts = arrow.texts.len();
    let slot = arrow.containers[&93]
        .iter()
        .find(|(_, (item, _))| *item == 53)
        .map(|(slot, _)| *slot)
        .expect("the headless arrows' slot");
    arrow.send(use_item_on_item(53, slot, 39, 5)).await;
    arrow
        .until_pinging(Duration::from_secs(20), "the arrow dialogue", |c| {
            shown(c, texts, (582 << 16) | 5, "Bronze arrow")
        })
        .await;
    // Five sets: the second finds no headless arrow and stops.
    arrow.send(continue_option(582, 4)).await;
    arrow
        .until_pinging(Duration::from_secs(30), "the bronze arrows", |c| {
            amount(c, 882) == 15 && said(c, from, "You attach arrow heads to 15 arrow shafts.")
        })
        .await;
    arrow.pump(Duration::from_secs(4)).await;
    assert_eq!(
        (amount(&arrow, 882), amount(&arrow, 53), amount(&arrow, 39)),
        (15, 0, 5),
        "one set of bronze arrows"
    );
    // 5 for the bow, 15 for the headless arrows, 19.5 for the tips.
    let experience = arrow.skills.get(&9).map(|s| s.1);
    assert!(
        matches!(experience, Some(39 | 40)),
        "fletcharrow's experience: {experience:?}"
    );
    eprintln!("fletcharrow strung a shortbow and made 15 headless then 15 bronze arrows");
    arrow.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Bolt, Dart, Limbing and GemBolt listeners: feathers on unfinished
    // bolts make one set of ten; darts are refused without Tourist Trap;
    // the wrong limbs are refused with a message and the right ones make
    // the crossbow; a chisel cuts an opal into twelve tips, and the tips on
    // the bolts make ten opal bolts.
    let inventory = [
        (0, 314, 20),
        (1, 9375, 20),
        (2, 819, 5),
        (3, 9440, 1),
        (4, 9420, 1),
        (5, 9422, 1),
        (6, 1755, 1),
        (7, 1609, 1),
    ];
    write_save_with(
        &players,
        "traceone",
        "fletchbolt",
        start,
        &inventory,
        |save| {
            save["skills"][9]["static"] = Value::String("11".into());
            save["skills"][9]["dynamic"] = Value::String("11".into());
        },
    );
    let (code, bolt) = Client::login(address, "fletchbolt").await;
    assert_eq!(code, 2, "fletchbolt's login");
    let mut bolt = bolt.unwrap();
    bolt.until(Duration::from_secs(10), "fletchbolt's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = bolt.messages.len();
    let texts = bolt.texts.len();
    bolt.send(use_item_on_item(314, 0, 9375, 1)).await;
    bolt.until_pinging(Duration::from_secs(20), "the bolt dialogue", |c| {
        shown(c, texts, (582 << 16) | 5, "Bronze bolts")
    })
    .await;
    bolt.send(continue_option(582, 6)).await;
    bolt.until_pinging(Duration::from_secs(20), "one set of bolts", |c| {
        amount(c, 877) == 10 && said(c, from, "You fletch 10 bolts")
    })
    .await;
    assert_eq!(
        (amount(&bolt, 314), amount(&bolt, 9375)),
        (10, 10),
        "one set takes ten feathers and ten unfinished bolts"
    );
    let texts = bolt.texts.len();
    bolt.send(use_item_on_item(314, 0, 819, 2)).await;
    bolt.until_pinging(Duration::from_secs(20), "the dart dialogue", |c| {
        shown(c, texts, (582 << 16) | 5, "Bronze dart")
    })
    .await;
    let texts = bolt.texts.len();
    bolt.send(continue_option(582, 6)).await;
    bolt.until_pinging(Duration::from_secs(20), "the Tourist Trap refusal", |c| {
        read(c, texts, "You need to have completed Tourist Trap")
    })
    .await;
    assert_eq!(
        (amount(&bolt, 806), amount(&bolt, 819)),
        (0, 5),
        "no dart is made without the quest"
    );
    bolt.send(use_item_on_item(9422, 5, 9440, 3)).await;
    bolt.until_pinging(Duration::from_secs(20), "the wrong limbs", |c| {
        said(
            c,
            from,
            "That's not the right limb to attach to that stock.",
        )
    })
    .await;
    let texts = bolt.texts.len();
    bolt.send(use_item_on_item(9420, 4, 9440, 3)).await;
    bolt.until_pinging(Duration::from_secs(20), "the limbing dialogue", |c| {
        shown(c, texts, (309 << 16) | 6, "Bronze c'bow (u)")
    })
    .await;
    bolt.send(continue_option(309, 6)).await;
    bolt.until_pinging(Duration::from_secs(20), "the unstrung crossbow", |c| {
        amount(c, 9454) == 1 && said(c, from, "You attach the metal limbs to the stock.")
    })
    .await;
    assert_eq!(
        (
            amount(&bolt, 9440),
            amount(&bolt, 9420),
            amount(&bolt, 9422)
        ),
        (0, 0, 1),
        "the stock and its limbs are used"
    );
    let texts = bolt.texts.len();
    bolt.send(use_item_on_item(1755, 6, 1609, 7)).await;
    bolt.until_pinging(Duration::from_secs(20), "the gem dialogue", |c| {
        shown(c, texts, (309 << 16) | 6, "Opal")
    })
    .await;
    bolt.send(continue_option(309, 6)).await;
    bolt.until_pinging(Duration::from_secs(30), "the opal bolt tips", |c| {
        amount(c, 45) == 12 && amount(c, 1609) == 0
    })
    .await;
    // The crafting loop's delay runs out before the next script is taken.
    bolt.pump(Duration::from_secs(5)).await;
    let texts = bolt.texts.len();
    let slot = |id: i32| bolt.slot_of(93, id).expect("the item is held");
    let (bolts, tips) = (slot(877), slot(45));
    bolt.send(use_item_on_item(877, bolts, 45, tips)).await;
    bolt.until_pinging(Duration::from_secs(20), "the tipping dialogue", |c| {
        shown(c, texts, (582 << 16) | 5, "Opal bolts")
    })
    .await;
    // Ten sets: the second finds no bronze bolt and stops.
    bolt.send(continue_option(582, 3)).await;
    bolt.until_pinging(Duration::from_secs(30), "the opal bolts", |c| {
        amount(c, 879) == 10 && said(c, from, "You fletch 10 bolts.")
    })
    .await;
    bolt.pump(Duration::from_secs(4)).await;
    assert_eq!(
        (amount(&bolt, 879), amount(&bolt, 877), amount(&bolt, 45)),
        (10, 0, 2),
        "one set of opal bolts"
    );
    // 5 for the bolts, 12 for the crossbow, 1.6 for the gem, 16 for the tips.
    let experience = bolt.skills.get(&9).map(|s| s.1);
    assert!(
        matches!(experience, Some(34 | 35)),
        "fletchbolt's experience: {experience:?}"
    );
    eprintln!(
        "fletchbolt made 10 bronze bolts, a bronze crossbow (u), 12 opal tips and 10 opal bolts"
    );
    bolt.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // ThievingListeners: a man's "Pickpocket" option. A success gives the
    // MAN table's three coins and 8 experience; a failure stuns the player
    // and hits for 1. The chance at level 1 is 181/256, so the attempts
    // are repeated until one succeeds. The man is the one who spawns in
    // the castle courtyard (3222,3221); others in view stand behind walls.
    let start = [3222, 3223, 0];
    write_save(&players, "traceone", "pickman", start, &[]);
    let (code, thief) = Client::login(address, "pickman").await;
    assert_eq!(code, 2, "pickman's login");
    let mut thief = thief.unwrap();
    thief
        .until(Duration::from_secs(10), "pickman's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let man = |c: &Client| {
        let base = c.base?;
        c.npcs
            .actors
            .iter()
            .filter(|(_, n)| (1..=3).contains(&n.type_id))
            .min_by_key(|(_, n)| {
                let (dx, dy) = (base[0] + n.tile[0] - 3222, base[1] + n.tile[1] - 3221);
                dx.abs().max(dy.abs())
            })
            .map(|(i, _)| *i as i32)
    };
    thief
        .until_pinging(Duration::from_secs(15), "a man in view", |c| {
            man(c).is_some()
        })
        .await;
    // A click is repeated when nothing comes of it within twelve seconds:
    // the man wanders, and a walk towards him can end short of him or at a
    // wall ("I can't reach that.").
    let (mut successes, mut failures, mut unanswered) = (0, 0, 0);
    for _ in 0..16 {
        let from = thief.messages.len();
        let coins = amount(&thief, 995);
        let index = man(&thief).expect("a man is still in view");
        thief.send(npc_option3(index)).await;
        let mut stunned = false;
        for _ in 0..12 {
            thief.pump(Duration::from_secs(1)).await;
            stunned = said(&thief, from, "You have been stunned!");
            if stunned || amount(&thief, 995) > coins {
                break;
            }
        }
        if amount(&thief, 995) > coins {
            successes += 1;
            assert_eq!(amount(&thief, 995), coins + 3, "a man's three coins");
            break;
        }
        if stunned {
            failures += 1;
            // The stun's five ticks.
            thief.pump(Duration::from_secs(4)).await;
        } else {
            unanswered += 1;
        }
    }
    assert_eq!(successes, 1, "pickman never succeeded in sixteen clicks");
    thief.pump(Duration::from_secs(2)).await;
    let experience = thief.skills.get(&17).map(|s| s.1);
    assert_eq!(experience, Some(8), "pickman's thieving experience");
    eprintln!(
        "pickman stole 3 coins from a man after {failures} failed attempts and {unanswered} unanswered clicks"
    );
    thief.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    stall_part(address, &players).await;
    chest_part(address, &players).await;
    door_part(address, &players).await;
    pick_part(address, &players).await;
    herb_part(address, &players).await;
    grind_part(address, &players).await;
    gem_part(address, &players).await;
    wares_part(address, &players).await;
}

/// The chest part of the gather scenario: ThievableChestPlugin on the
/// ten-coin chest 2566 in East Ardougne. "Search for traps" at Thieving 40
/// disarms and opens it over six ticks for 10 coins and 7.8 experience;
/// the constant then respawns for eleven ticks, during which "Open" says
/// it has been looted, and after which "Open" springs the trap.
async fn chest_part(address: std::net::SocketAddr, players: &std::path::Path) {
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let amount = |c: &Client, id: i32| -> i32 {
        c.containers
            .get(&93)
            .map(|slots| {
                slots
                    .values()
                    .filter(|(item, _)| *item == id)
                    .map(|(_, n)| *n)
                    .sum()
            })
            .unwrap_or(0)
    };
    let chest = [2676, 3310];
    let start = [chest[0] - 1, chest[1], 0];
    write_save_with(players, "traceone", "chestman", start, &[], |save| {
        // The experience stays 0: this character has not finished the
        // tutorial, which caps a skill's experience at 175.
        save["skills"][17]["static"] = Value::String("40".into());
        save["skills"][17]["dynamic"] = Value::String("40".into());
    });
    let (code, thief) = Client::login(address, "chestman").await;
    assert_eq!(code, 2, "chestman's login");
    let mut thief = thief.unwrap();
    thief
        .until(Duration::from_secs(10), "chestman's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = thief.messages.len();
    thief.send(scenery_option2(2566, chest[0], chest[1])).await;
    thief
        .until_pinging(Duration::from_secs(15), "the trap found", |c| {
            said(c, from, "You find a trap on the chest...")
        })
        .await;
    thief
        .until_pinging(Duration::from_secs(10), "the treasure", |c| {
            said(c, from, "You find treasure inside!") && amount(c, 995) == 10
        })
        .await;
    assert!(said(&thief, from, "You disable the trap."));
    assert!(said(&thief, from, "You open the chest."));
    thief.pump(Duration::from_secs(1)).await;
    let experience = thief.skills.get(&17).map(|s| s.1);
    assert_eq!(experience, Some(7), "chestman's thieving experience");
    // The chest is the looted id for three ticks, then itself again while
    // the constant still respawns.
    thief.pump(Duration::from_secs(2)).await;
    let from = thief.messages.len();
    thief.send(scenery_option1(2566, chest[0], chest[1])).await;
    thief
        .until_pinging(Duration::from_secs(5), "the looted chest", |c| {
            said(c, from, "It looks like this chest has already been looted.")
        })
        .await;
    // Eleven ticks after the treasure the trap is set again.
    thief.pump(Duration::from_secs(5)).await;
    let from = thief.messages.len();
    thief.send(scenery_option1(2566, chest[0], chest[1])).await;
    thief
        .until_pinging(Duration::from_secs(5), "the sprung trap", |c| {
            said(c, from, "You have activated a trap on the chest.")
        })
        .await;
    assert_eq!(
        amount(&thief, 995),
        10,
        "an opened chest hands over nothing"
    );
    eprintln!(
        "chestman searched the ten-coin chest for traps, found it looted, then sprang its trap"
    );
    thief.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The door part of the gather scenario.
async fn door_part(address: std::net::SocketAddr, players: &std::path::Path) {
    // DoorManagingPlugin and DoorActionHandler: a closed door of a
    // Lumbridge house (36846, on the west edge of its tile) is opened,
    // walked through, closed from inside and opened again.
    let door = [3234, 3207];
    let open = [door[0] - 1, door[1]];
    let start = [door[0] - 2, door[1], 0];
    write_save(players, "traceone", "doorman", start, &[]);
    let (code, walker) = Client::login(address, "doorman").await;
    assert_eq!(code, 2, "doorman's login");
    let mut walker = walker.unwrap();
    walker
        .until(Duration::from_secs(10), "doorman's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    // The closed door is a wall: a walk to the tile behind it ends outside.
    walker
        .send(actions::worldspace_walk(door[0] + 1, door[1], false))
        .await;
    walker.pump(Duration::from_secs(5)).await;
    let outside = walker.position();
    eprintln!("doorman stands at {outside:?} with the door closed");
    assert!(outside[0] < door[0], "doorman walked through a closed door");
    walker.send(scenery_option1(36846, door[0], door[1])).await;
    walker.pump(Duration::from_secs(4)).await;
    walker
        .send(actions::worldspace_walk(door[0] + 1, door[1], false))
        .await;
    walker
        .until_pinging(
            Duration::from_secs(10),
            "the walk through the open door",
            |c| c.position() == [door[0] + 1, door[1], 0],
        )
        .await;
    // "Close" on the open door (36848, swung onto the tile outside).
    walker.send(scenery_option1(36848, open[0], open[1])).await;
    walker.pump(Duration::from_secs(4)).await;
    // The house has a second, open door on its south side: with this one
    // closed the two tiles to the tile outside are a walk around the
    // house, which four ticks do not finish.
    walker
        .send(actions::worldspace_walk(open[0], open[1], false))
        .await;
    walker.pump(Duration::from_millis(2400)).await;
    let detour = walker.position();
    eprintln!("doorman is at {detour:?} four ticks after the door closed again");
    assert_ne!(
        detour,
        [open[0], open[1], 0],
        "the closed door did not block"
    );
    walker
        .send(actions::worldspace_walk(door[0] + 1, door[1], false))
        .await;
    walker
        .until_pinging(Duration::from_secs(15), "the walk back inside", |c| {
            c.position() == [door[0] + 1, door[1], 0]
        })
        .await;
    walker.send(scenery_option1(36846, door[0], door[1])).await;
    walker.pump(Duration::from_secs(4)).await;
    walker
        .send(actions::worldspace_walk(start[0], start[1], false))
        .await;
    walker
        .until_pinging(Duration::from_millis(2500), "the walk back out", |c| {
            c.position() == start
        })
        .await;
    eprintln!("doorman opened a Lumbridge door, walked in, closed it, opened it and walked out");
    walker.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The lock part of the gather scenario.
async fn pick_part(address: std::net::SocketAddr, players: &std::path::Path) {
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    // PickableDoorHandler: the level 1 door of East Ardougne (34812, on
    // the east edge of its tile). West of it is the locked side.
    let door = [2672, 3308];
    let locked = [door[0], door[1], 0];
    let unlocked = [door[0] + 1, door[1], 0];
    write_save(players, "traceone", "lockman", locked, &[]);
    let (code, thief) = Client::login(address, "lockman").await;
    assert_eq!(code, 2, "lockman's login");
    let mut thief = thief.unwrap();
    thief
        .until(Duration::from_secs(10), "lockman's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = thief.messages.len();
    thief.send(scenery_option1(34812, door[0], door[1])).await;
    thief
        .until_pinging(Duration::from_secs(8), "the locked door", |c| {
            said(c, from, "The door is locked.")
        })
        .await;
    assert_eq!(thief.position(), locked);
    // A pick succeeds two times in three: click until one does.
    let mut attempts = 0;
    loop {
        attempts += 1;
        assert!(attempts <= 12, "lockman never picked the lock");
        let from = thief.messages.len();
        thief.send(scenery_option2(34812, door[0], door[1])).await;
        thief
            .until_pinging(Duration::from_secs(8), "an attempt at the lock", |c| {
                said(c, from, "You manage to pick the lock.")
                    || said(c, from, "You fail to pick the lock.")
            })
            .await;
        assert!(said(&thief, from, "You attempt to pick the lock."));
        if said(&thief, from, "You manage to pick the lock.") {
            break;
        }
    }
    thief
        .until_pinging(
            Duration::from_secs(8),
            "the walk through the picked door",
            |c| c.position() == unlocked,
        )
        .await;
    thief.pump(Duration::from_secs(3)).await;
    let experience = thief.skills.get(&17).map(|s| s.1);
    assert_eq!(experience, Some(3), "lockman's thieving experience");
    let from = thief.messages.len();
    thief.send(scenery_option2(34812, door[0], door[1])).await;
    thief
        .until_pinging(Duration::from_secs(8), "the unlocked side", |c| {
            said(c, from, "The door is already unlocked.")
        })
        .await;
    assert_eq!(thief.position(), unlocked);
    thief.send(scenery_option1(34812, door[0], door[1])).await;
    thief
        .until_pinging(Duration::from_secs(8), "the way back", |c| {
            said(c, from, "You go through the door.") && c.position() == locked
        })
        .await;
    eprintln!(
        "lockman found the door locked, picked it in {attempts} attempts, and opened it from the other side"
    );
    thief.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The herblore part of the gather scenario: HerbCleanListener's "Clean"
/// option and PotionMixingListener with HerblorePulse. Without the Druidic
/// Ritual nothing is cleaned. With it, at Herblore 3: a grimy ranarr is
/// refused by its level, two grimy guams are cleaned, "All" in the skill
/// dialogue (309) makes two guam potions (unf) from three vials, "Make 1"
/// mixes one attack potion and, with one of each left, a second use mixes
/// the other without a dialogue.
async fn herb_part(address: std::net::SocketAddr, players: &std::path::Path) {
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let count = |c: &Client, from: usize, line: &str| {
        c.messages[from..].iter().filter(|(_, m)| m == line).count()
    };
    let amount = |c: &Client, id: i32| -> i32 {
        c.containers
            .get(&93)
            .map(|slots| {
                slots
                    .values()
                    .filter(|(item, _)| *item == id)
                    .map(|(_, amount)| *amount)
                    .sum()
            })
            .unwrap_or(0)
    };
    let slot_of = |c: &Client, id: i32| -> i32 {
        c.containers[&93]
            .iter()
            .filter(|(_, (item, _))| *item == id)
            .map(|(slot, _)| *slot)
            .min()
            .expect("the item's slot")
    };
    let shown = |c: &Client, from: usize, pointer: i32, what: &str| {
        c.texts[from..]
            .iter()
            .any(|(_, at, t)| *at == pointer && t.contains(what))
    };
    let start = [3222, 3218, 0];
    write_save(players, "traceone", "herbnovice", start, &[(0, 199, 1)]);
    let (code, novice) = Client::login(address, "herbnovice").await;
    assert_eq!(code, 2, "herbnovice's login");
    let mut novice = novice.unwrap();
    novice
        .until(Duration::from_secs(10), "herbnovice's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = novice.messages.len();
    novice.send(item_option1(199, 0)).await;
    novice
        .until_pinging(Duration::from_secs(20), "the quest's refusal", |c| {
            said(
                c,
                from,
                "You must have completed the Druidic Ritual quest before you can use Herblore.",
            )
        })
        .await;
    assert_eq!(amount(&novice, 199), 1, "the herb stays grimy");
    novice.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let inventory = [
        (0, 199, 1),
        (1, 199, 1),
        (2, 227, 1),
        (3, 227, 1),
        (4, 227, 1),
        (5, 221, 1),
        (6, 221, 1),
        (7, 207, 1),
    ];
    write_save_with(players, "traceone", "herbman", start, &inventory, |save| {
        save["skills"][15]["static"] = Value::String("3".into());
        save["skills"][15]["dynamic"] = Value::String("3".into());
        // The experience stays under the 175 an unfinished tutorial caps it at.
        save["skills"][15]["experience"] = Value::String("0.0".into());
        // Druidic Ritual (quest index 48) complete.
        save["quests"]["questStages"] = serde_json::json!([{"questId": "48", "questStage": "100"}]);
    });
    let (code, herb) = Client::login(address, "herbman").await;
    assert_eq!(code, 2, "herbman's login");
    let mut herb = herb.unwrap();
    herb.until(Duration::from_secs(10), "herbman's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = herb.messages.len();
    herb.send(item_option1(207, 7)).await;
    herb.until_pinging(Duration::from_secs(20), "the ranarr's level", |c| {
        said(
            c,
            from,
            "You cannot clean this herb. You need a Herblore level of 25 to attempt this.",
        )
    })
    .await;
    for (slot, cleaned) in [(0, 1), (1, 2)] {
        herb.send(item_option1(199, slot)).await;
        herb.until_pinging(Duration::from_secs(20), "a cleaned guam", |c| {
            amount(c, 249) == cleaned
                && count(c, from, "You clean the dirt from the guam leaf.") == cleaned as usize
        })
        .await;
    }
    assert_eq!(
        (amount(&herb, 199), amount(&herb, 207)),
        (0, 1),
        "both guams are clean and the ranarr is not"
    );
    let experience = herb.skills.get(&15).map(|s| s.1);
    assert_eq!(experience, Some(5), "two guams are 5 experience");

    // Two herbs and three vials: the dialogue, and "All" asks for three.
    let texts = herb.texts.len();
    herb.send(use_item_on_item(249, 0, 227, 2)).await;
    herb.until_pinging(Duration::from_secs(20), "the unfinished dialogue", |c| {
        shown(c, texts, (309 << 16) | 6, "Guam")
    })
    .await;
    herb.send(continue_option(309, 3)).await;
    herb.until_pinging(Duration::from_secs(30), "two guam potions", |c| {
        amount(c, 91) == 2 && count(c, from, "You put the Guam leaf into the vial of water.") == 2
    })
    .await;
    herb.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&herb, 249), amount(&herb, 227), amount(&herb, 91)),
        (0, 1, 2),
        "the third vial has no herb"
    );
    assert_eq!(
        herb.skills.get(&15).map(|s| s.1),
        Some(5),
        "an unfinished potion is no experience"
    );

    // Two of each: the dialogue and "Make 1". Then one of each: at once.
    let texts = herb.texts.len();
    let (unfinished, eye) = (slot_of(&herb, 91), slot_of(&herb, 221));
    herb.send(use_item_on_item(221, eye, 91, unfinished)).await;
    herb.until_pinging(Duration::from_secs(20), "the potion dialogue", |c| {
        shown(c, texts, (309 << 16) | 6, "Attack")
    })
    .await;
    herb.send(continue_option(309, 6)).await;
    herb.until_pinging(Duration::from_secs(30), "the first attack potion", |c| {
        amount(c, 121) == 1 && count(c, from, "You mix the eye of newt into your potion.") == 1
    })
    .await;
    herb.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&herb, 91), amount(&herb, 221), amount(&herb, 121)),
        (1, 1, 1),
        "Make 1 mixes one potion"
    );
    let texts = herb.texts.len();
    let (unfinished, eye) = (slot_of(&herb, 91), slot_of(&herb, 221));
    herb.send(use_item_on_item(91, unfinished, 221, eye)).await;
    herb.until_pinging(Duration::from_secs(30), "the second attack potion", |c| {
        amount(c, 121) == 2 && count(c, from, "You mix the eye of newt into your potion.") == 2
    })
    .await;
    assert!(
        !shown(&herb, texts, (309 << 16) | 6, "Attack"),
        "one of each mixes without a dialogue"
    );
    herb.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&herb, 91), amount(&herb, 221)),
        (0, 0),
        "both potions are mixed"
    );
    let experience = herb.skills.get(&15).map(|s| s.1);
    assert_eq!(experience, Some(55), "two attack potions are 50 experience");
    eprintln!(
        "herbman cleaned two guams, made two guam potions (unf) and mixed two attack potions"
    );
    herb.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The grinding, herb tar and barbarian mix part of the gather scenario:
/// GrindItemPlugin and HerbTarPlugin through `UseWithHandler.run`'s
/// item-on-item pulse, and BarbarianMixListener. Without the barbarian
/// training attribute roe on an attack potion is nothing interesting. With
/// it at 1, at Herblore 19 with the Druidic Ritual: "All" grinds two
/// unicorn horns, "Make 1" grinds ten of 25 fishing bait, "All" makes two
/// guam tars from 30 swamp tar and the roe makes an attack mix with Otto's
/// hint.
async fn grind_part(address: std::net::SocketAddr, players: &std::path::Path) {
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let count = |c: &Client, from: usize, line: &str| {
        c.messages[from..].iter().filter(|(_, m)| m == line).count()
    };
    let amount = |c: &Client, id: i32| -> i32 {
        c.containers
            .get(&93)
            .map(|slots| {
                slots
                    .values()
                    .filter(|(item, _)| *item == id)
                    .map(|(_, amount)| *amount)
                    .sum()
            })
            .unwrap_or(0)
    };
    let slot_of = |c: &Client, id: i32| -> i32 {
        c.containers[&93]
            .iter()
            .filter(|(_, (item, _))| *item == id)
            .map(|(slot, _)| *slot)
            .min()
            .expect("the item's slot")
    };
    let shown = |c: &Client, from: usize, pointer: i32, what: &str| {
        c.texts[from..]
            .iter()
            .any(|(_, at, t)| *at == pointer && t.contains(what))
    };
    let start = [3222, 3218, 0];
    write_save(
        players,
        "traceone",
        "mixnovice",
        start,
        &[(0, 123, 1), (1, 11324, 1)],
    );
    let (code, novice) = Client::login(address, "mixnovice").await;
    assert_eq!(code, 2, "mixnovice's login");
    let mut novice = novice.unwrap();
    novice
        .until(Duration::from_secs(10), "mixnovice's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = novice.messages.len();
    novice.send(use_item_on_item(11324, 1, 123, 0)).await;
    novice
        .until_pinging(Duration::from_secs(20), "the untrained mix", |c| {
            said(c, from, "Nothing interesting happens.")
        })
        .await;
    assert_eq!(
        (
            amount(&novice, 123),
            amount(&novice, 11324),
            amount(&novice, 11429)
        ),
        (1, 1, 0),
        "nothing is mixed without the barbarian training"
    );
    novice.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let inventory = [
        (0, 233, 1),
        (1, 237, 1),
        (2, 237, 1),
        (3, 1939, 30),
        (4, 249, 1),
        (5, 249, 1),
        (6, 123, 1),
        (7, 11324, 1),
        (8, 313, 25),
    ];
    write_save_with(players, "traceone", "grindman", start, &inventory, |save| {
        save["skills"][15]["static"] = Value::String("19".into());
        save["skills"][15]["dynamic"] = Value::String("19".into());
        // The experience stays under the 175 an unfinished tutorial caps it at.
        save["skills"][15]["experience"] = Value::String("0.0".into());
        // Druidic Ritual (quest index 48) complete.
        save["quests"]["questStages"] = serde_json::json!([{"questId": "48", "questStage": "100"}]);
        // BarbarianTraining.attributeHerblore at 1: Otto has explained the mixes.
        save.as_object_mut()
            .unwrap()
            .entry("attributes")
            .or_insert_with(|| Value::Array(vec![]))
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"key": "barbtraining:herblore", "type": "int", "value": "1"}));
    });
    let (code, grind) = Client::login(address, "grindman").await;
    assert_eq!(code, 2, "grindman's login");
    let mut grind = grind.unwrap();
    grind
        .until(Duration::from_secs(10), "grindman's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = grind.messages.len();

    // Two horns on the pestle and mortar: the dialogue, and "All" asks for two.
    let texts = grind.texts.len();
    grind.send(use_item_on_item(237, 1, 233, 0)).await;
    grind
        .until_pinging(Duration::from_secs(20), "the grinding dialogue", |c| {
            shown(c, texts, (309 << 16) | 6, "nicorn")
        })
        .await;
    grind.send(continue_option(309, 3)).await;
    grind
        .until_pinging(Duration::from_secs(30), "two horns ground", |c| {
            amount(c, 235) == 2
        })
        .await;
    grind.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (
            amount(&grind, 237),
            amount(&grind, 235),
            amount(&grind, 233)
        ),
        (0, 2, 1),
        "both horns are dust and the pestle and mortar stays"
    );

    // 25 fishing bait, "Make 1": one reward grinds ten.
    let texts = grind.texts.len();
    grind.send(use_item_on_item(313, 8, 233, 0)).await;
    grind
        .until_pinging(Duration::from_secs(20), "the bait dialogue", |c| {
            shown(c, texts, (309 << 16) | 6, "bait")
        })
        .await;
    grind.send(continue_option(309, 6)).await;
    grind
        .until_pinging(Duration::from_secs(30), "ten bait ground", |c| {
            amount(c, 12129) == 10
        })
        .await;
    grind.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&grind, 313), amount(&grind, 12129)),
        (15, 10),
        "Make 1 grinds ten of the bait"
    );
    assert_eq!(
        grind.skills.get(&15).map(|s| s.1),
        Some(0),
        "grinding is no experience"
    );

    // Two clean guams on 30 swamp tar: "All" asks for the two guams.
    let texts = grind.texts.len();
    let (guam, tar) = (slot_of(&grind, 249), slot_of(&grind, 1939));
    grind.send(use_item_on_item(249, guam, 1939, tar)).await;
    grind
        .until_pinging(Duration::from_secs(20), "the tar dialogue", |c| {
            shown(c, texts, (309 << 16) | 6, "tar")
        })
        .await;
    grind.send(continue_option(309, 3)).await;
    grind
        .until_pinging(Duration::from_secs(40), "two guam tars", |c| {
            amount(c, 10142) == 30 && count(c, from, "You add the guam to the swamp tar.") == 2
        })
        .await;
    grind.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (
            amount(&grind, 249),
            amount(&grind, 1939),
            amount(&grind, 10142)
        ),
        (0, 0, 30),
        "each guam takes 15 swamp tar and makes 15 guam tar"
    );
    assert_eq!(
        grind.skills.get(&15).map(|s| s.1),
        Some(60),
        "two guam tars are 60 experience"
    );

    // Roe on an attack potion (2) with the training at 1: the mix and Otto's hint.
    let texts = grind.texts.len();
    let (roe, potion) = (slot_of(&grind, 11324), slot_of(&grind, 123));
    grind.send(use_item_on_item(11324, roe, 123, potion)).await;
    grind
        .until_pinging(Duration::from_secs(20), "the attack mix", |c| {
            amount(c, 11429) == 1 && said(c, from, "You combine your potion with the fish eggs.")
        })
        .await;
    grind
        .until_pinging(Duration::from_secs(10), "Otto's hint", |c| {
            c.texts[texts..]
                .iter()
                .any(|(_, _, t)| t.contains("barbarian ways"))
        })
        .await;
    assert_eq!(
        (amount(&grind, 123), amount(&grind, 11324)),
        (0, 0),
        "the potion and the roe are used"
    );
    assert_eq!(
        grind.skills.get(&15).map(|s| s.1),
        Some(68),
        "an attack mix is 8 experience"
    );
    eprintln!(
        "grindman ground two unicorn horns and ten fishing bait, made two guam tars and an attack mix"
    );
    grind.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The crafting tools part of the gather scenario.
async fn gem_part(address: std::net::SocketAddr, players: &std::path::Path) {
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let amount = |c: &Client, id: i32| -> i32 {
        c.containers
            .get(&93)
            .map(|slots| {
                slots
                    .values()
                    .filter(|(item, _)| *item == id)
                    .map(|(_, amount)| *amount)
                    .sum()
            })
            .unwrap_or(0)
    };
    let slot_of = |c: &Client, id: i32| -> i32 {
        c.containers[&93]
            .iter()
            .filter(|(_, (item, _))| *item == id)
            .map(|(slot, _)| *slot)
            .min()
            .expect("the item's slot")
    };
    let shown = |c: &Client, from: usize, pointer: i32, what: &str| {
        c.texts[from..]
            .iter()
            .any(|(_, at, t)| *at == pointer && t.contains(what))
    };
    // GemCutPlugin, ChiselLimestonePlugin, AmuletStringingPlugin and
    // SilverStringingListener: a chisel, two uncut sapphires, an uncut opal,
    // an uncut diamond, a limestone, two balls of wool, a gold amulet and
    // an unstrung symbol, at Crafting 20.
    let start = [3222, 3218, 0];
    let inventory = [
        (0, 1755, 1),
        (1, 1623, 1),
        (2, 1623, 1),
        (3, 1625, 1),
        (4, 1617, 1),
        (5, 3211, 1),
        (6, 1759, 1),
        (7, 1759, 1),
        (8, 1673, 1),
        (9, 1714, 1),
    ];
    write_save_with(players, "traceone", "gemman", start, &inventory, |save| {
        save["skills"][12]["static"] = Value::String("20".into());
        save["skills"][12]["dynamic"] = Value::String("20".into());
        // The experience stays under the 175 an unfinished tutorial caps it at.
        save["skills"][12]["experience"] = Value::String("0.0".into());
    });
    let (code, gem) = Client::login(address, "gemman").await;
    assert_eq!(code, 2, "gemman's login");
    let mut gem = gem.unwrap();
    gem.until(Duration::from_secs(10), "gemman's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = gem.messages.len();

    // The chisel on one of two uncut sapphires: the dialogue, and "All"
    // cuts both.
    let texts = gem.texts.len();
    gem.send(use_item_on_item(1755, 0, 1623, 1)).await;
    gem.until_pinging(Duration::from_secs(20), "the gem dialogue", |c| {
        shown(c, texts, (309 << 16) | 6, "apphire")
    })
    .await;
    gem.send(continue_option(309, 3)).await;
    gem.until_pinging(Duration::from_secs(30), "two sapphires cut", |c| {
        amount(c, 1607) == 2
    })
    .await;
    gem.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&gem, 1623), amount(&gem, 1607), amount(&gem, 1755)),
        (0, 2, 1),
        "both sapphires are cut and the chisel stays"
    );
    assert_eq!(
        gem.skills.get(&12).map(|s| s.1),
        Some(100),
        "two sapphires are 100 experience"
    );

    // The one uncut opal on the chisel: no dialogue, one cut.
    let (opal, chisel) = (slot_of(&gem, 1625), slot_of(&gem, 1755));
    gem.send(use_item_on_item(1625, opal, 1755, chisel)).await;
    gem.until_pinging(Duration::from_secs(30), "the opal cut", |c| {
        amount(c, 1609) == 1
    })
    .await;
    gem.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&gem, 1625), gem.skills.get(&12).map(|s| s.1)),
        (0, Some(110)),
        "an opal is 10 experience"
    );

    // The uncut diamond needs Crafting 43.
    let diamond = slot_of(&gem, 1617);
    gem.send(use_item_on_item(1755, chisel, 1617, diamond))
        .await;
    gem.until_pinging(Duration::from_secs(20), "the diamond's refusal", |c| {
        said(
            c,
            from,
            "You need a crafting level of 43 to craft this gem.",
        )
    })
    .await;
    gem.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&gem, 1617), amount(&gem, 1601)),
        (1, 0),
        "the diamond stays uncut"
    );

    // The chisel on the limestone: a brick on the pulse's second tick.
    let limestone = slot_of(&gem, 3211);
    gem.send(use_item_on_item(1755, chisel, 3211, limestone))
        .await;
    gem.until_pinging(Duration::from_secs(30), "the limestone brick", |c| {
        amount(c, 3420) == 1
    })
    .await;
    gem.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&gem, 3211), gem.skills.get(&12).map(|s| s.1)),
        (0, Some(116)),
        "a limestone brick is 6 experience"
    );

    // A ball of wool on the gold amulet: the strung amulet, no experience.
    let (wool, amulet) = (slot_of(&gem, 1759), slot_of(&gem, 1673));
    gem.send(use_item_on_item(1759, wool, 1673, amulet)).await;
    gem.until_pinging(Duration::from_secs(20), "the strung amulet", |c| {
        amount(c, 1692) == 1
    })
    .await;
    assert_eq!(
        (amount(&gem, 1673), amount(&gem, 1759)),
        (0, 1),
        "the amulet and one ball of wool are used"
    );

    // The other ball of wool on the unstrung symbol: SilverStringingListener.
    let (wool, symbol) = (slot_of(&gem, 1759), slot_of(&gem, 1714));
    gem.send(use_item_on_item(1759, wool, 1714, symbol)).await;
    gem.until_pinging(Duration::from_secs(20), "the strung symbol", |c| {
        amount(c, 1716) == 1
    })
    .await;
    assert_eq!(
        (
            amount(&gem, 1714),
            amount(&gem, 1759),
            gem.skills.get(&12).map(|s| s.1)
        ),
        (0, 0, Some(116)),
        "the symbol and the wool are used and stringing is no experience"
    );
    eprintln!(
        "gemman cut two sapphires and an opal, was refused a diamond, made a limestone brick and strung an amulet and a symbol"
    );
    gem.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The crafting wares part of the gather scenario.
async fn wares_part(address: std::net::SocketAddr, players: &std::path::Path) {
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let count = |c: &Client, from: usize, line: &str| {
        c.messages[from..].iter().filter(|(_, m)| m == line).count()
    };
    let amount = |c: &Client, id: i32| -> i32 {
        c.containers
            .get(&93)
            .map(|slots| {
                slots
                    .values()
                    .filter(|(item, _)| *item == id)
                    .map(|(_, amount)| *amount)
                    .sum()
            })
            .unwrap_or(0)
    };
    let slot_of = |c: &Client, id: i32| -> i32 {
        c.containers[&93]
            .iter()
            .filter(|(_, (item, _))| *item == id)
            .map(|(slot, _)| *slot)
            .min()
            .expect("the item's slot")
    };
    let shown = |c: &Client, from: usize, pointer: i32, what: &str| {
        c.texts[from..]
            .iter()
            .any(|(_, at, t)| *at == pointer && t.contains(what))
    };
    // GlassMakeListener and GlassCraftingListener: two heaps of soda ash and
    // two buckets of sand at the Al Kharid furnace 11666, then the pipe.
    let start = [3274, 3187, 0];
    let inventory = [
        (0, 1781, 1),
        (1, 1781, 1),
        (2, 1783, 1),
        (3, 1783, 1),
        (4, 1785, 1),
    ];
    write_save_with(players, "traceone", "glassman", start, &inventory, |save| {
        save["skills"][12]["static"] = Value::String("10".into());
        save["skills"][12]["dynamic"] = Value::String("10".into());
        // The experience stays under the 175 an unfinished tutorial caps it at.
        save["skills"][12]["experience"] = Value::String("0.0".into());
    });
    let (code, glass) = Client::login(address, "glassman").await;
    assert_eq!(code, 2, "glassman's login");
    let mut glass = glass.unwrap();
    glass
        .until(Duration::from_secs(10), "glassman's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = glass.messages.len();

    // Soda ash on the furnace: the dialogue, and "All" makes two.
    let texts = glass.texts.len();
    glass
        .send(use_with_scenery(1781, 0, 11666, 3272, 3185))
        .await;
    glass
        .until_pinging(Duration::from_secs(20), "the molten glass dialogue", |c| {
            shown(c, texts, (309 << 16) | 6, "olten")
        })
        .await;
    glass.send(continue_option(309, 3)).await;
    glass
        .until_pinging(Duration::from_secs(30), "two molten glass", |c| {
            amount(c, 1775) == 2
        })
        .await;
    glass.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (
            amount(&glass, 1781),
            amount(&glass, 1783),
            amount(&glass, 1925),
            count(
                &glass,
                from,
                "You heat the sand and soda ash in the furnace to make glass."
            )
        ),
        (0, 0, 2, 2),
        "the ash and the sand are used and the buckets are back"
    );
    assert_eq!(
        glass.skills.get(&12).map(|s| s.1),
        Some(40),
        "two molten glass are 40 experience"
    );

    // The pipe on the molten glass: the interface. A vial needs Crafting 33.
    let opened = glass.opened.len();
    let (pipe, molten) = (slot_of(&glass, 1785), slot_of(&glass, 1775));
    glass.send(use_item_on_item(1785, pipe, 1775, molten)).await;
    glass
        .until_pinging(Duration::from_secs(20), "the glassblowing interface", |c| {
            c.opened[opened..].iter().any(|(_, id)| *id == 542)
        })
        .await;
    glass.send(button(542, 38)).await;
    glass
        .until_pinging(Duration::from_secs(20), "the vial's refusal", |c| {
            said(c, from, "You need a Crafting level of 33 to make this.")
        })
        .await;
    // "Make All" (opcode 124) of the beer glass (button 40).
    glass.send(if_button(124, 542, 40, -1)).await;
    glass
        .until_pinging(Duration::from_secs(40), "two beer glasses", |c| {
            amount(c, 1919) == 2
        })
        .await;
    glass.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (
            amount(&glass, 1775),
            amount(&glass, 1785),
            count(&glass, from, "You make a beer glass.")
        ),
        (0, 1, 2),
        "both molten glass are blown and the pipe stays"
    );
    assert_eq!(
        glass.skills.get(&12).map(|s| s.1),
        Some(75),
        "two beer glasses are 35 experience"
    );
    eprintln!("glassman made two molten glass at the furnace and blew two beer glasses");
    glass.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // SpikyVambraces and BattlestaffListener, at Crafting 54.
    let start = [3222, 3218, 0];
    let inventory = [
        (0, 10113, 1),
        (1, 1063, 1),
        (2, 571, 1),
        (3, 571, 1),
        (4, 1391, 1),
        (5, 1391, 1),
    ];
    write_save_with(players, "traceone", "staffman", start, &inventory, |save| {
        save["skills"][12]["static"] = Value::String("54".into());
        save["skills"][12]["dynamic"] = Value::String("54".into());
        save["skills"][12]["experience"] = Value::String("0.0".into());
    });
    let (code, staff) = Client::login(address, "staffman").await;
    assert_eq!(code, 2, "staffman's login");
    let mut staff = staff.unwrap();
    staff
        .until(Duration::from_secs(10), "staffman's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let from = staff.messages.len();
    staff.send(use_item_on_item(10113, 0, 1063, 1)).await;
    staff
        .until_pinging(Duration::from_secs(20), "the spiky vambraces", |c| {
            amount(c, 10077) == 1
                && said(
                    c,
                    from,
                    "You carefully attach the sharp claws to the leather vambraces.",
                )
        })
        .await;
    assert_eq!(
        (
            amount(&staff, 10113),
            amount(&staff, 1063),
            staff.skills.get(&12).map(|s| s.1)
        ),
        (0, 0, Some(6)),
        "the claws and the vambraces are used for 6 experience"
    );

    // A water orb on one of two battlestaves: the dialogue, and "All"
    // makes two, two ticks apart.
    let texts = staff.texts.len();
    let (orb, stick) = (slot_of(&staff, 571), slot_of(&staff, 1391));
    staff.send(use_item_on_item(571, orb, 1391, stick)).await;
    staff
        .until_pinging(Duration::from_secs(20), "the battlestaff dialogue", |c| {
            shown(c, texts, (309 << 16) | 6, "attlestaff")
        })
        .await;
    staff.send(continue_option(309, 3)).await;
    staff
        .until_pinging(Duration::from_secs(30), "two water battlestaves", |c| {
            amount(c, 1395) == 2
        })
        .await;
    staff.pump(Duration::from_secs(3)).await;
    assert_eq!(
        (amount(&staff, 571), amount(&staff, 1391)),
        (0, 0),
        "both orbs and both battlestaves are used"
    );
    // 6 and two times 100, capped at the 175 of an unfinished tutorial.
    assert_eq!(
        staff.skills.get(&12).map(|s| s.1),
        Some(175),
        "the battlestaves' experience up to the tutorial's cap"
    );
    eprintln!("staffman made spiky vambraces and two water battlestaves");
    staff.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The stall part of the gather scenario.
async fn stall_part(address: std::net::SocketAddr, players: &std::path::Path) {
    let said =
        |c: &Client, from: usize, line: &str| c.messages[from..].iter().any(|(_, m)| m == line);
    let amount = |c: &Client, id: i32| -> i32 {
        c.containers
            .get(&93)
            .map(|slots| {
                slots
                    .values()
                    .filter(|(item, _)| *item == id)
                    .map(|(_, n)| *n)
                    .sum()
            })
            .unwrap_or(0)
    };
    // ThievingOptionPlugin and StallThiefPulse: the Varrock tea stall's
    // "Steal-from" option. The third tick of the pulse hands over a cup of
    // tea and 16 experience and replaces the stall with its empty id for
    // twelve ticks; no guard 32 or 2236 stands within eight tiles.
    let stall = [3269, 3410];
    let start = [stall[0] - 1, stall[1] - 1, 0];
    write_save_with(players, "traceone", "stallman", start, &[], |save| {
        save["skills"][17]["static"] = Value::String("40".into());
        save["skills"][17]["dynamic"] = Value::String("40".into());
        // The experience stays 0: this character has not finished the
        // tutorial, which caps a skill's experience at 175.
    });
    let (code, thief) = Client::login(address, "stallman").await;
    assert_eq!(code, 2, "stallman's login");
    let mut thief = thief.unwrap();
    thief
        .until(Duration::from_secs(10), "stallman's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    let before = thief.skills.get(&17).map(|s| s.1);
    let from = thief.messages.len();
    thief.send(scenery_option2(635, stall[0], stall[1])).await;
    thief
        .until_pinging(Duration::from_secs(15), "the attempt's message", |c| {
            said(c, from, "You attempt to steal some tea from the tea stall")
        })
        .await;
    thief
        .until_pinging(Duration::from_secs(10), "a cup of tea", |c| {
            amount(c, 712) == 1 && said(c, from, "You steal a cup of tea from the tea stall.")
        })
        .await;
    thief.pump(Duration::from_secs(1)).await;
    let after = thief.skills.get(&17).map(|s| s.1);
    assert_eq!(
        after.zip(before).map(|(a, b)| a - b),
        Some(16),
        "stallman's thieving experience"
    );
    // The stall is its empty id now: the full id is no longer on the tile,
    // so a second click finds nothing until it is restored.
    let from = thief.messages.len();
    thief.send(scenery_option2(635, stall[0], stall[1])).await;
    thief.pump(Duration::from_secs(3)).await;
    assert!(
        !said(&thief, from, "You attempt to steal"),
        "the emptied stall was stolen from"
    );
    // Twelve ticks after the theft the stall is back.
    thief.pump(Duration::from_secs(6)).await;
    let from = thief.messages.len();
    thief.send(scenery_option2(635, stall[0], stall[1])).await;
    thief
        .until_pinging(Duration::from_secs(10), "a second cup of tea", |c| {
            amount(c, 712) == 2 && said(c, from, "You steal a cup of tea from the tea stall.")
        })
        .await;
    eprintln!(
        "stallman stole two cups of tea from the Varrock tea stall, with the stall empty in between"
    );
    thief.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// CowMilkingPlugin: the dairy cow 8689's "Milk" option, then a bucket used
/// on it (BucketHandler through `UseWithHandler.HANDLERS`).
async fn use_milk_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    let start = [3251, 3273, 0];
    write_save(
        &players,
        "traceone",
        "usemilk",
        start,
        &[(0, 1925, 1), (1, 1925, 1)],
    );
    let (code, milk) = Client::login(address, "usemilk").await;
    assert_eq!(code, 2, "usemilk's login");
    let mut milk = milk.unwrap();
    milk.until(Duration::from_secs(10), "usemilk's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = milk.messages.len();
    // The option: eight ticks later the first bucket is milk.
    milk.send(scenery_option1(8689, 3252, 3275)).await;
    milk.until_pinging(Duration::from_secs(30), "the first bucket of milk", |c| {
        held(c, 1927) == 1
    })
    .await;
    // A bucket on the cow: a new milking pulse takes the old one's place.
    let slot = milk.containers[&93]
        .iter()
        .find(|(_, (item, _))| *item == 1925)
        .map(|(slot, _)| *slot)
        .expect("a bucket is left");
    milk.send(use_with_scenery(1925, slot, 8689, 3252, 3275))
        .await;
    milk.until_pinging(Duration::from_secs(30), "the second bucket of milk", |c| {
        held(c, 1927) == 2 && held(c, 1925) == 0
    })
    .await;
    let lines: Vec<&str> = milk.messages[from..]
        .iter()
        .map(|(_, m)| m.as_str())
        .collect();
    // The save has debug on: UseWithHandler$3 names the handler it ran.
    assert_eq!(
        lines.iter().filter(|m| **m == "You milk the cow.").count(),
        2,
        "usemilk's messages: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|m| m.starts_with("Handler=BucketHandler, used item=")),
        "the bucket handler's debug line: {lines:?}"
    );
    eprintln!(
        "usemilk milked the dairy cow 8689 twice from {:?}",
        milk.position()
    );
    milk.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

async fn use_cook_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    let start = [3152, 3207, 0];
    write_save_with(
        &players,
        "traceone",
        "usecook",
        start,
        &[
            (0, 1921, 1),
            (1, 2142, 1),
            (2, 317, 1),
            (3, 317, 1),
            (4, 317, 1),
        ],
        |save| {
            // Three experience short of cooking level 2: the first cooked
            // shrimp (30) gains the level.
            save["skills"][7]["experience"] = Value::String("80.0".into());
            save.as_object_mut()
                .unwrap()
                .entry("attributes")
                .or_insert_with(|| Value::Array(vec![]))
                .as_array_mut()
                .unwrap()
                .push(
                    serde_json::json!({"key": "tutorial:complete", "type": "bool", "value": true}),
                );
        },
    );
    let (code, cook) = Client::login(address, "usecook").await;
    assert_eq!(code, 2, "usecook's login");
    let mut cook = cook.unwrap();
    // tutorial:complete: the welcome screen (378) and its play button.
    cook.until(Duration::from_secs(10), "usecook's lobby", |c| {
        c.base.is_some() && c.frames.iter().any(|f| f.opcode == 164)
    })
    .await;
    cook.send(button(378, 140)).await;
    cook.until(Duration::from_secs(10), "usecook in the game world", |c| {
        !c.updates.is_empty()
    })
    .await;
    // A bowl of water: no cooking row, no roll.
    cook.send(use_with_scenery(1921, 0, 5631, 3150, 3205)).await;
    cook.until(Duration::from_secs(20), "a bowl of hot water", |c| {
        held(c, 4456) == 1 && held(c, 1921) == 0
    })
    .await;
    let at = cook.position();
    eprintln!(
        "usecook heated a bowl of water at the fire 5631 at [3150, 3205, 0]: walked {start:?} -> {at:?}"
    );
    assert_ne!(at, start, "usecook walked to the fire");
    // Cooked meat: burnt on purpose.
    let from = cook.messages.len();
    cook.send(use_with_scenery(2142, 1, 5631, 3150, 3205)).await;
    cook.until(Duration::from_secs(20), "burnt meat", |c| {
        held(c, 2146) == 1 && held(c, 2142) == 0
    })
    .await;
    assert!(
        cook.messages[from..]
            .iter()
            .any(|(_, m)| m == "You deliberately burn the perfectly good piece of meat."),
        "usecook's burnt meat message: {:?}",
        &cook.messages[from..]
    );
    // Three raw shrimps: the amount chatbox, then "All" (child 3, the file's button 2).
    let from = cook.messages.len();
    let opened = cook.opened.len();
    cook.send(use_with_scenery(317, 2, 5631, 3150, 3205)).await;
    cook.until(Duration::from_secs(20), "the cooking chatbox", |c| {
        c.opened[opened..].iter().any(|(_, id)| *id == 307)
    })
    .await;
    assert!(
        cook.texts
            .iter()
            .any(|(_, _, text)| text == "<br><br><br><br>Raw shrimps"),
        "the chatbox names the food"
    );
    // The interpreter hands the file `child - 1`: "All" is button 2.
    cook.send(continue_option(307, 3)).await;
    cook.until_pinging(Duration::from_secs(40), "three shrimps cooked", |c| {
        held(c, 317) == 0 && held(c, 315) + held(c, 7954) == 3
    })
    .await;
    let lines: Vec<&str> = cook.messages[from..]
        .iter()
        .map(|(_, m)| m.as_str())
        .filter(|m| m.contains("shrimps"))
        .collect();
    assert_eq!(lines.len(), 3, "a message per shrimp: {lines:?}");
    for line in &lines {
        assert!(
            [
                "You manage to cook some shrimps.",
                "You accidentally burn some shrimps."
            ]
            .contains(line),
            "{line}"
        );
    }
    let cooked = held(&cook, 315);
    eprintln!(
        "usecook cooked {cooked} and burnt {} of three shrimps; cooking {:?}",
        held(&cook, 7954),
        cook.skills.get(&7)
    );
    if cooked > 0 {
        // 30 experience a shrimp (the client's skill packet for cooking).
        let experience = cook.skills.get(&7).map(|s| s.1).unwrap_or(0);
        assert!(experience > 80, "cooking experience after {cooked} shrimps");
        // LevelUp.levelup for a tutorial-complete player.
        assert!(
            cook.messages[from..].iter().any(
                |(_, m)| m == "You've just advanced a Cooking level! You have reached level 2."
            ),
            "the level-up message: {:?}",
            &cook.messages[from..]
        );
    }
    cook.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// The content use-with listeners installed from `world::global_items2` and
/// `world::item_on_npc` (docs/live-world.md): two buckets and a jug at the
/// Lumbridge castle fountain (WaterSourceListener's fill pulse, one vessel a
/// tick), a jug at the well west of the castle (only a bucket goes down a
/// well) and bones fed to a Varrock stray dog (FeedOnStrayDog).
async fn use_content_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let held = |c: &Client, id: i32| {
        c.containers
            .get(&93)
            .map(|slots| slots.values().filter(|(item, _)| *item == id).count())
            .unwrap_or(0)
    };
    // The fountain 36781 in the castle courtyard.
    let start = [3222, 3214, 0];
    write_save(
        &players,
        "traceone",
        "usefill",
        start,
        &[(0, 1925, 1), (1, 1925, 1), (2, 1935, 1)],
    );
    let (code, fill) = Client::login(address, "usefill").await;
    assert_eq!(code, 2, "usefill's login");
    let mut fill = fill.unwrap();
    fill.until(Duration::from_secs(10), "usefill's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = fill.messages.len();
    fill.send(use_with_scenery(1925, 0, 36781, 3221, 3210))
        .await;
    fill.until(Duration::from_secs(20), "two buckets of water", |c| {
        held(c, 1929) == 2 && held(c, 1925) == 0
    })
    .await;
    let lines: Vec<&str> = fill.messages[from..]
        .iter()
        .map(|(_, m)| m.as_str())
        .collect();
    assert_eq!(
        lines,
        ["You fill the bucket.", "You fill the bucket."],
        "usefill's bucket messages"
    );
    let at = fill.position();
    eprintln!(
        "usefill filled two buckets at the fountain 36781 at [3221, 3210, 0]: walked {start:?} -> {at:?}"
    );
    assert_ne!(at, start, "usefill walked to the fountain");
    assert_eq!(held(&fill, 1935), 1, "the jug is not the bucket's vessel");
    let from = fill.messages.len();
    fill.send(use_with_scenery(1935, 2, 36781, 3221, 3210))
        .await;
    fill.until(Duration::from_secs(20), "a jug of water", |c| {
        held(c, 1937) == 1 && held(c, 1935) == 0
    })
    .await;
    assert!(
        fill.messages[from..]
            .iter()
            .any(|(_, m)| m == "You fill the jug."),
        "usefill's jug message: {:?}",
        &fill.messages[from..]
    );
    fill.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // The well 884 west of the castle: a jug does not go down a well.
    let start = [3164, 3222, 0];
    write_save(&players, "traceone", "usewell", start, &[(0, 1935, 1)]);
    let (code, well) = Client::login(address, "usewell").await;
    assert_eq!(code, 2, "usewell's login");
    let mut well = well.unwrap();
    well.until(Duration::from_secs(10), "usewell's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let from = well.messages.len();
    well.send(use_with_scenery(1935, 0, 884, 3161, 3220)).await;
    let refusal = "If I drop my jug down there, I don't think I'm likely to get it back.";
    well.until(Duration::from_secs(20), "the well's refusal", |c| {
        c.messages[from..].iter().any(|(_, m)| m == refusal)
    })
    .await;
    assert_eq!(held(&well, 1935), 1, "usewell keeps the jug");
    eprintln!(
        "usewell used a jug on the well 884 at [3161, 3220, 0]: walked {start:?} -> {:?}",
        well.position()
    );
    well.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // A stray dog (5917) in Varrock: bones.
    let start = [3200, 3402, 0];
    write_save(&players, "traceone", "usedog", start, &[(0, 526, 1)]);
    let (code, dog) = Client::login(address, "usedog").await;
    assert_eq!(code, 2, "usedog's login");
    let mut dog = dog.unwrap();
    dog.until(Duration::from_secs(10), "usedog's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let stray = |c: &Client| {
        c.npcs
            .actors
            .iter()
            .find(|(_, n)| n.type_id == 5917)
            .map(|(i, _)| *i as i32)
    };
    dog.until(Duration::from_secs(15), "the stray dog", |c| {
        stray(c).is_some()
    })
    .await;
    let index = stray(&dog).unwrap();
    let from = dog.messages.len();
    dog.send(use_with_npc(526, 0, index)).await;
    dog.until(Duration::from_secs(30), "the dog fed", |c| {
        c.messages[from..]
            .iter()
            .any(|(_, m)| m == "You feed the stray dog your bones.")
    })
    .await;
    dog.until(Duration::from_secs(5), "the bones gone", |c| {
        held(c, 526) == 0
    })
    .await;
    eprintln!(
        "usedog fed bones to the stray dog (index {index}): walked {start:?} -> {:?}",
        dog.position()
    );
    dog.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    // CiderOnForester: five ciders for the people (ids 1 to 5) of the
    // Seers' Village pub, each a two-line dialogue; the fifth finishes the
    // Seers' Village diary's task.
    let start = [2694, 3494, 0];
    write_save(
        &players,
        "traceone",
        "usecider",
        start,
        &[
            (0, 5763, 1),
            (1, 5763, 1),
            (2, 5763, 1),
            (3, 5763, 1),
            (4, 5763, 1),
        ],
    );
    let (code, cider) = Client::login(address, "usecider").await;
    assert_eq!(code, 2, "usecider's login");
    let mut cider = cider.unwrap();
    cider
        .until(Duration::from_secs(10), "usecider's rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    // The patron nearest the middle of the pub: the lambda asks that the
    // player stands inside the pub (2689,3488 to 2700,3498), and the same
    // NPC types walk the village outside it, so the first one in view can
    // lead the player out of the door.
    let patron = |c: &Client| {
        let base = c.base?;
        c.npcs
            .actors
            .iter()
            .filter(|(_, n)| (1..=5).contains(&n.type_id))
            .min_by_key(|(_, n)| {
                let (dx, dy) = (base[0] + n.tile[0] - 2694, base[1] + n.tile[1] - 3493);
                dx.abs().max(dy.abs())
            })
            .map(|(i, _)| *i as i32)
    };
    cider
        .until(Duration::from_secs(15), "a patron of the pub", |c| {
            patron(c).is_some()
        })
        .await;
    let read = |c: &Client, from: usize, what: &str| {
        c.texts[from..].iter().any(|(_, _, t)| t.contains(what))
    };
    let messages = cider.messages.len();
    for slot in 0..5 {
        let index = patron(&cider).unwrap();
        let from = cider.texts.len();
        cider.send(use_with_npc(5763, slot, index)).await;
        cider
            .until_pinging(Duration::from_secs(30), "the patron's first line", |c| {
                read(c, from, "Ah, a glass of cider, Don't mind if I do!")
            })
            .await;
        assert_eq!(
            held(&cider, 5763),
            5 - slot as usize,
            "the cider is still held"
        );
        let from = cider.texts.len();
        cider.send(continue_option(241, 5)).await;
        cider
            .until_pinging(Duration::from_secs(10), "the patron's thanks", |c| {
                read(c, from, "Thanks!") && held(c, 5763) == 4 - slot as usize
            })
            .await;
        cider.send(continue_option(241, 5)).await;
        tokio::time::sleep(Duration::from_millis(1300)).await;
    }
    cider
        .until_pinging(Duration::from_secs(10), "the diary's task", |c| {
            c.messages[messages..]
                .iter()
                .any(|(_, m)| m == "Achievement Diary has been updated.")
        })
        .await;
    eprintln!(
        "usecider gave five ciders to the pub's patrons: walked {start:?} -> {:?}",
        cider.position()
    );
    cider.writer.close(1000, "logout").await.unwrap();
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
        .block_on(drive(address, std::path::Path::new(&saves), None));
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
/// IF_ACTION (155 from the client): a component's first option.
fn button(interface: i32, child: i32) -> Frame {
    let mut payload = ((interface << 16) | child).to_be_bytes().to_vec();
    payload.extend((-1i16).to_be_bytes());
    Frame {
        opcode: 155,
        payload,
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
/// OBJECT_ACTION_2 (194): a scenery's second option.
fn scenery_option2(id: i32, x: i32, y: i32) -> Frame {
    frame(194, &[&la16(y), &le16(x), &be16(id)])
}
/// OPLOCU (115): an inventory item (interface 149) used on an NPC.
fn use_with_npc(item: i32, slot: i32, npc_index: i32) -> Frame {
    frame(
        115,
        &[&ib32(149 << 16), &le16(slot), &le16(npc_index), &la16(item)],
    )
}
/// OPPLAYERU (248): an inventory item used on another player.
fn use_with_player(item: i32, slot: i32, other_index: i32) -> Frame {
    frame(
        248,
        &[
            &la16(other_index),
            &be16(item),
            &be16(slot),
            &ib32(149 << 16),
        ],
    )
}
/// OPLOCU (134): an inventory item used on a scene object.
fn use_with_scenery(item: i32, slot: i32, id: i32, x: i32, y: i32) -> Frame {
    frame(
        134,
        &[
            &sa16(x),
            &be16(item),
            &le16(y),
            &be16(slot),
            &le16(0),
            &be16(149),
            &sa16(id),
        ],
    )
}
/// Lumbridge church's altar 36972 and a tile in front of it.
const ALTAR: [i32; 2] = [3243, 3206];
const ALTAR_STAND: [i32; 2] = [3243, 3208];
/// Otto's bed (scenery 25268) in his grotto.
const OTTO_BED: [i32; 2] = [2500, 3490];
/// NPC_ACTION_1 (78): an NPC's first option.
fn npc_option1(index: i32) -> Frame {
    frame(78, &[&le16(index)])
}
/// NPC_ACTION_3 (148): an NPC's third option.
fn npc_option3(index: i32) -> Frame {
    frame(148, &[&sa16(index)])
}
/// IF_ACTION with a menu opcode (155 first option, 196 second ...), a
/// component and a slot.
fn if_button(opcode: u8, interface: i32, child: i32, slot: i32) -> Frame {
    frame(
        opcode,
        &[&((interface << 16) | child).to_be_bytes(), &be16(slot)],
    )
}
/// RESUME_PAUSEBUTTON (132): Continue, or a dialogue option's component.
fn continue_option(interface: i32, child: i32) -> Frame {
    frame(132, &[&ia32((interface << 16) | child), &le16(-1)])
}
/// CLOSE_MODAL (184).
fn close_interface() -> Frame {
    frame(184, &[])
}

/// Talking and the bank: `bankone` stands in front of Draynor bank's
/// middle booth (3092,3243; the banker 494 is behind it at 3090,3243) with a
/// bronze dagger. Talk-to (NPC option 1) goes through the native
/// InteractionListeners (BankerNPC's "talk-to", its destination override,
/// the walk and the arrival), `DialogueLabeller.open` and the banker
/// conversation: the NPC's line in chatbox 241, Continue (132), the options,
/// the chosen "access" option spoken by the player, Continue, and the bank
/// (762) opens. A deposit (763:0 slot 0) moves the dagger to the bank
/// container (95), a withdraw (762:73 slot 0) brings it back, Close (184)
/// runs the captured BankClose. The banker's Bank option (3) then opens the
/// bank directly (`BankerNPC.attemptBank`).
async fn bank_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let at = [3092, 3243, 0];
    write_save(&players, "traceone", "bankone", at, &[(0, 1205, 1)]);
    let (code, one) = Client::login(address, "bankone").await;
    assert_eq!(code, 2, "bankone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "bankone's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let banker = |c: &Client| {
        let base = c.base?;
        c.npcs
            .actors
            .iter()
            .find(|(_, n)| {
                n.type_id == 494 && [base[0] + n.tile[0], base[1] + n.tile[1]] == [3090, 3243]
            })
            .map(|(i, _)| *i as i32)
    };
    one.until(
        Duration::from_secs(15),
        "the banker behind the booth",
        |c| banker(c).is_some(),
    )
    .await;
    let index = banker(&one).unwrap();
    let said = |c: &Client, from: usize, what: &str| {
        c.texts[from..].iter().any(|(_, _, t)| t.contains(what))
    };

    // Talk-to: the banker's greeting.
    let from = one.texts.len();
    one.send(npc_option1(index)).await;
    one.until(Duration::from_secs(15), "the banker's greeting", |c| {
        said(
            c,
            from,
            "Good day, would you like to access your bank account?",
        )
    })
    .await;
    let greeting = one.texts[from..]
        .iter()
        .find(|(_, _, t)| t.contains("Good day"))
        .map(|(_, p, _)| *p)
        .unwrap();
    assert_eq!(
        greeting >> 16,
        241,
        "a one-line NPC dialogue is chatbox 241"
    );
    // Continue: the options.
    let from = one.texts.len();
    one.send(continue_option(241, 5)).await;
    one.until(Duration::from_secs(10), "the banker's options", |c| {
        said(c, from, "I'd like to access my bank account please.")
    })
    .await;
    let access = one.texts[from..]
        .iter()
        .find(|(_, _, t)| t.contains("I'd like to access my bank account please."))
        .map(|(_, p, _)| *p)
        .unwrap();
    let options: Vec<&str> = one.texts[from..]
        .iter()
        .map(|(_, _, t)| t.as_str())
        .collect();
    eprintln!(
        "bankone: banker options {options:?} (access at {}:{})",
        access >> 16,
        access & 0xffff
    );
    // The chosen option: the player speaks it.
    let from = one.texts.len();
    one.send(continue_option(access >> 16, access & 0xffff))
        .await;
    one.until(Duration::from_secs(10), "the player's spoken option", |c| {
        said(c, from, "I'd like to access my bank account please.")
    })
    .await;
    // Continue: the bank opens.
    let opened = one.opened.len();
    one.send(continue_option(64, 5)).await;
    one.until(Duration::from_secs(10), "the bank interface", |c| {
        c.opened[opened..].iter().any(|(_, id)| *id == 762)
    })
    .await;
    eprintln!("bankone: talked to the banker, the conversation opened the bank");

    // Deposit one dagger (the side inventory 763:0, slot 0).
    one.send(if_button(155, 763, 0, 0)).await;
    one.until(Duration::from_secs(10), "the dagger in the bank", |c| {
        c.slot_of(95, 1205).is_some() && c.slot_of(93, 1205).is_none()
    })
    .await;
    // Withdraw it (the bank's items 762:73).
    let slot = one.slot_of(95, 1205).unwrap();
    one.send(if_button(155, 762, 73, slot)).await;
    one.until(
        Duration::from_secs(10),
        "the dagger back in the inventory",
        |c| c.slot_of(93, 1205).is_some() && c.slot_of(95, 1205).is_none(),
    )
    .await;
    // Close: BankClose (the side tab closed, script 571), then the bank.
    let from = one.frames.len();
    one.send(close_interface()).await;
    one.until(Duration::from_secs(10), "the bank closed", |c| {
        c.frames[from..].iter().any(|f| {
            matches!(
                updates::ServerUpdate::decode(f),
                Ok(Some(updates::ServerUpdate::RunClientScript { id: 571, .. }))
            )
        })
    })
    .await;
    eprintln!("bankone: deposited and withdrew the dagger, closed the bank");

    // The banker's Bank option: BankerNPC.attemptBank opens the bank.
    let opened = one.opened.len();
    one.send(npc_option3(index)).await;
    one.until(
        Duration::from_secs(15),
        "the bank from the Bank option",
        |c| c.opened[opened..].iter().any(|(_, id)| *id == 762),
    )
    .await;
    one.send(close_interface()).await;
    one.pump(Duration::from_secs(1)).await;
    eprintln!("bankone: the banker's Bank option opened the bank");
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
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

/// Shops: `shopone` stands in Lumbridge's general store with 100 coins and
/// a bronze dagger. Trade (NPC option 3) on the shop keeper (520, a ShopNPC of
/// the start-up) goes through the native InteractionListeners (Shops'
/// "trade" listener and destination override, the walk and the arrival) and
/// `Shop.openFor`: the stock interface (620) with its side tab (621).
/// Buy-1 (196 on 620:23, slot 0) takes the price and gives a pot (1931);
/// Value (155 on 621:0) answers with the dagger's price; Sell-1 (196) puts
/// the dagger in the store's player stock and pays for it; Buy-X (234) asks
/// for an amount (script 108/109) and the answer (INPUT_INT 23) buys two more
/// pots through `RunScript.processInput` and `shop::run_amount`; Close
/// (184) runs the shop's onClose(620).
async fn shop_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    write_save(
        &players,
        "traceone",
        "shopone",
        [3214, 3241, 0],
        &[(0, 995, 100), (1, 1205, 1)],
    );
    let (code, one) = Client::login(address, "shopone").await;
    assert_eq!(code, 2, "shopone's login");
    let mut one = one.unwrap();
    one.until(Duration::from_secs(10), "shopone's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    let keeper = |c: &Client| {
        c.npcs
            .actors
            .iter()
            .find(|(_, n)| n.type_id == 520)
            .map(|(i, _)| *i as i32)
    };
    one.until(Duration::from_secs(15), "the shop keeper", |c| {
        keeper(c).is_some()
    })
    .await;
    let index = keeper(&one).unwrap();
    let coins = |c: &Client| {
        c.containers
            .get(&93)
            .and_then(|i| i.values().find(|(id, _)| *id == 995).map(|(_, n)| *n))
            .unwrap_or(0)
    };
    let pots = |c: &Client| {
        c.containers
            .get(&93)
            .map(|i| i.values().filter(|(id, _)| *id == 1931).count())
            .unwrap_or(0)
    };
    let opened = one.opened.len();
    one.send(npc_option3(index)).await;
    one.until(Duration::from_secs(20), "the shop interface", |c| {
        c.opened[opened..].iter().any(|(_, id)| *id == 620)
            && c.opened[opened..].iter().any(|(_, id)| *id == 621)
    })
    .await;
    assert!(
        one.texts
            .iter()
            .any(|(_, _, t)| t == "Lumbridge General Store"),
        "the shop's title"
    );
    // Buy one pot.
    let before = coins(&one);
    one.send(if_button(196, 620, 23, 0)).await;
    one.until(Duration::from_secs(10), "a bought pot", |c| {
        pots(c) == 1 && coins(c) < before
    })
    .await;
    let paid = before - coins(&one);
    // The dagger's value, then its sale.
    let from = one.messages.len();
    one.send(if_button(155, 621, 0, 1)).await;
    one.until(Duration::from_secs(10), "the dagger's value", |c| {
        c.messages[from..]
            .iter()
            .any(|(_, m)| m.contains("This shop will buy this item for"))
    })
    .await;
    let before = coins(&one);
    one.send(if_button(196, 621, 0, 1)).await;
    one.until(Duration::from_secs(10), "the dagger sold", |c| {
        c.slot_of(93, 1205).is_none() && coins(c) > before
    })
    .await;
    let earned = coins(&one) - before;
    // The sale switched the shown tab to the store's player stock: back to
    // the main stock (620:25), then Buy-X: the amount prompt and two more pots.
    one.send(if_button(155, 620, 25, -1)).await;
    one.pump(Duration::from_millis(700)).await;
    let from = one.frames.len();
    one.send(if_button(234, 620, 23, 0)).await;
    one.until(Duration::from_secs(10), "the amount prompt", |c| {
        c.frames[from..].iter().any(|f| {
            matches!(
                updates::ServerUpdate::decode(f),
                Ok(Some(updates::ServerUpdate::RunClientScript {
                    id: 108 | 109,
                    ..
                }))
            )
        })
    })
    .await;
    one.send(frame(23, &[&2i32.to_be_bytes()])).await;
    one.until(Duration::from_secs(10), "two more pots", |c| pots(c) == 3)
        .await;
    one.send(close_interface()).await;
    one.pump(Duration::from_secs(1)).await;
    eprintln!(
        "shopone: bought a pot for {paid} coins, sold the dagger for {earned}, bought two more through Buy-X"
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}
/// PLAYER_ACTION: Follow (71, option index 2) and Trade with (180, index 3).
fn player_follow(index: i32) -> Frame {
    frame(71, &[&la16(index)])
}
fn player_trade(index: i32) -> Frame {
    frame(180, &[&la16(index)])
}

/// Logs a saved character in and waits for its rebuild.
async fn enter(address: std::net::SocketAddr, name: &str) -> Client {
    let (code, client) = Client::login(address, name).await;
    assert_eq!(code, 2, "{name}'s login");
    let mut client = client.unwrap();
    client
        .until(Duration::from_secs(10), "the rebuild", |c| {
            c.base.is_some() && !c.updates.is_empty()
        })
        .await;
    client
}

/// Requests and a trade between two players: `tradeone` (a bronze dagger)
/// uses Trade with on `tradetwo` (the global handlers' RequestOption
/// listener through the native InteractionListeners, the walk to the player,
/// `RequestManager.request`): "Sending a trade offer..." and the other's
/// ":tradereq:" line. `tradetwo` answers with Trade with: the request is
/// accepted and both get the trade screen (335 with the side tab 336).
/// `tradeone` offers the dagger (155 on 336:0, slot 0), both accept
/// (335:16), the second screen (334) opens, both accept (334:20):
/// `TradeContainer` gives the dagger to `tradetwo`.
async fn trade_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    write_save(
        &players,
        "traceone",
        "tradeone",
        [3210, 3222, 0],
        &[(0, 1205, 1)],
    );
    write_save(&players, "traceone", "tradetwo", [3211, 3222, 0], &[]);
    let mut one = enter(address, "tradeone").await;
    let mut two = enter(address, "tradetwo").await;
    let (index_one, index_two) = (one.index, two.index);
    one.until(Duration::from_secs(10), "tradetwo in view", |c| {
        c.players.players[index_two as usize].is_some()
    })
    .await;
    two.until(Duration::from_secs(10), "tradeone in view", |c| {
        c.players.players[index_one as usize].is_some()
    })
    .await;
    let (from_one, from_two) = (one.messages.len(), two.messages.len());
    one.send(player_trade(index_two)).await;
    one.until(Duration::from_secs(15), "the trade offer's message", |c| {
        c.messages[from_one..]
            .iter()
            .any(|(_, m)| m.contains("Sending a trade offer..."))
    })
    .await;
    two.until(Duration::from_secs(15), "the trade request", |c| {
        c.messages[from_two..]
            .iter()
            .any(|(_, m)| m.contains("tradeone:tradereq:"))
    })
    .await;
    let (opened_one, opened_two) = (one.opened.len(), two.opened.len());
    two.send(player_trade(index_one)).await;
    two.until(Duration::from_secs(15), "tradetwo's trade screen", |c| {
        c.opened[opened_two..].iter().any(|(_, id)| *id == 335)
    })
    .await;
    one.until(Duration::from_secs(15), "tradeone's trade screen", |c| {
        c.opened[opened_one..].iter().any(|(_, id)| *id == 335)
            && c.opened[opened_one..].iter().any(|(_, id)| *id == 336)
    })
    .await;
    // The offer, then both accept the first screen.
    one.send(if_button(155, 336, 0, 0)).await;
    one.until(Duration::from_secs(10), "the dagger offered", |c| {
        c.slot_of(93, 1205).is_none()
    })
    .await;
    let (opened_one, opened_two) = (one.opened.len(), two.opened.len());
    one.send(if_button(155, 335, 16, -1)).await;
    one.pump(Duration::from_millis(900)).await;
    two.send(if_button(155, 335, 16, -1)).await;
    one.until(Duration::from_secs(10), "tradeone's second screen", |c| {
        c.opened[opened_one..].iter().any(|(_, id)| *id == 334)
    })
    .await;
    two.until(Duration::from_secs(10), "tradetwo's second screen", |c| {
        c.opened[opened_two..].iter().any(|(_, id)| *id == 334)
    })
    .await;
    one.send(if_button(155, 334, 20, -1)).await;
    one.pump(Duration::from_millis(900)).await;
    two.send(if_button(155, 334, 20, -1)).await;
    two.until(Duration::from_secs(10), "the dagger traded", |c| {
        c.slot_of(93, 1205).is_some()
    })
    .await;
    one.pump(Duration::from_secs(1)).await;
    assert!(
        one.slot_of(93, 1205).is_none(),
        "tradeone gave the dagger away"
    );
    eprintln!("tradeone and tradetwo: request, acceptance, both screens, the dagger changed hands");
    one.writer.close(1000, "logout").await.unwrap();
    two.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// Following: `followone` uses Follow on `followtwo` (the player option's
/// InteractPlugin with the bound FollowOptionPlugin: the walk to the
/// player, then its `MovementPulse(player, target, FOLLOW_ENTITY)`), and
/// when `followtwo` walks six tiles west `followone` ends on a tile
/// next to it.
async fn follow_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    write_save(&players, "traceone", "followone", [3222, 3222, 0], &[]);
    write_save(&players, "traceone", "followtwo", [3219, 3222, 0], &[]);
    let mut one = enter(address, "followone").await;
    let mut two = enter(address, "followtwo").await;
    let index_two = two.index;
    one.until(Duration::from_secs(10), "followtwo in view", |c| {
        c.players.players[index_two as usize].is_some()
    })
    .await;
    one.send(player_follow(index_two)).await;
    one.until(Duration::from_secs(15), "followone beside followtwo", |c| {
        let here = c.position();
        (here[0] - 3219).abs() <= 1 && (here[1] - 3222).abs() <= 1
    })
    .await;
    let target = [3213, 3222, 0];
    two.send(actions::worldspace_walk(target[0], target[1], false))
        .await;
    two.until(Duration::from_secs(15), "followtwo's walk", |c| {
        c.position() == target
    })
    .await;
    let mut ping = Instant::now();
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        let here = one.position();
        if (here[0] - target[0]).abs() <= 1 && (here[1] - target[1]).abs() <= 1 {
            break;
        }
        assert!(
            Instant::now() < end,
            "followone did not follow: at {here:?}"
        );
        if ping.elapsed() > Duration::from_secs(5) {
            one.send(frame(93, &[])).await;
            two.send(frame(93, &[])).await;
            ping = Instant::now();
        }
        one.pump(Duration::from_millis(200)).await;
        two.pump(Duration::from_millis(50)).await;
    }
    eprintln!(
        "followone followed followtwo to {:?} (followtwo at {target:?})",
        one.position()
    );
    one.writer.close(1000, "logout").await.unwrap();
    two.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}
/// ITEM_ACTION_4 (161): an inventory item's fourth option (Empty).
fn item_option4(id: i32, slot: i32) -> Frame {
    frame(161, &[&li32(149 << 16), &la16(id), &la16(slot)])
}
/// ITEM_ACTION_1 (156): an inventory item's first option (Eat, Drink).
fn item_option1(id: i32, slot: i32) -> Frame {
    frame(156, &[&la16(slot), &sa16(id), &li32(149 << 16)])
}

/// Eating and drinking: `eatone` (4 of its 10 hitpoints, shrimps 315, a
/// strength potion (4) 113) eats the shrimps (ITEM option "eat" through the
/// native InteractionListeners to ConsumableListener and `Food.consume`):
/// "You eat the shrimps.", the shrimps leave the inventory, hitpoints go
/// from 4 to 7 with "It heals some health.", animation 829 and sound 2393.
/// Drinking the potion (`Potion.consume`) leaves a three-dose potion (115),
/// raises Strength above 1 and, two ticks later, the SOFT dose script says
/// "You have 3 doses of potion left.".
async fn eat_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    write_save_with(
        &players,
        "traceone",
        "eatone",
        [3220, 3225, 0],
        &[(0, 315, 1), (1, 113, 1)],
        |save| {
            save["skills"][3]["lifepoints"] = Value::String("4".into());
        },
    );
    let mut one = enter(address, "eatone").await;
    one.until(Duration::from_secs(10), "eatone's hitpoints", |c| {
        c.skills.get(&3).map(|s| s.0) == Some(4)
    })
    .await;
    let (from, updates_from) = (one.messages.len(), one.updates.len());
    one.send(item_option1(315, 0)).await;
    one.until(Duration::from_secs(10), "the shrimps eaten", |c| {
        c.slot_of(93, 315).is_none() && c.skills.get(&3).map(|s| s.0) == Some(7)
    })
    .await;
    let said =
        |c: &Client, from: usize, what: &str| c.messages[from..].iter().any(|(_, m)| m == what);
    assert!(
        said(&one, from, "You eat the shrimps."),
        "the eating message"
    );
    assert!(
        said(&one, from, "It heals some health."),
        "the healing message"
    );
    one.until(Duration::from_secs(5), "the eating animation", |c| {
        c.updates[updates_from..].iter().any(|(_, u)| {
            u.extended.iter().any(|e| {
                e.fields
                    .iter()
                    .any(|f| matches!(f, players::Field::Animation { id: 829, .. }))
            })
        })
    })
    .await;
    // The potion, after the eat clock (NEXT_CONSUME is not set by plain food).
    let from = one.messages.len();
    one.send(item_option1(113, 1)).await;
    one.until(Duration::from_secs(10), "the potion drunk", |c| {
        c.slot_of(93, 115) == Some(1) && c.skills.get(&2).is_some_and(|s| s.0 > 1)
    })
    .await;
    one.until(Duration::from_secs(10), "the dose message", |c| {
        said(c, from, "You have 3 doses of potion left.")
    })
    .await;
    assert!(
        said(&one, from, "You drink some of your strength potion."),
        "the drinking message"
    );
    eprintln!(
        "eatone: ate the shrimps (hitpoints 4 -> 7), drank a dose (Strength {} of 1, three doses left)",
        one.skills[&2].0
    );
    one.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// `StringUtils.stringToLong` for a lower-case name.
fn name37(name: &str) -> i64 {
    name.bytes().fold(0i64, |value, c| {
        value * 37
            + match c {
                b'a'..=b'z' => i64::from(c - b'a') + 1,
                b'0'..=b'9' => i64::from(c - b'0') + 27,
                _ => 0,
            }
    })
}

/// The Appearance flag written by the original's flag writer over the live
/// receivers (world::live::appearance): `lookone` is saved as a female
/// character with body colours and 40 Attack, Strength and Defence, holding
/// a bronze two-handed sword (1307). A second character reads its block:
/// gender, the saved looks and colours, the name, the combat level of the
/// saved skills and the default render animation; after `lookone` wields
/// the sword, the weapon slot and the sword's render animation (124, from
/// `Appearance.setAnimations`).
async fn appearance_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    let looks = [45, 1000, 56, 61, 68, 70, 79];
    let colours = [5, 0, 3, 0, 4, 7, 2];
    write_save_with(
        &players,
        "traceone",
        "lookone",
        [3230, 3230, 0],
        &[(0, 1307, 1)],
        |save| {
            save["appearance"]["gender"] = Value::String("1".into());
            save["appearance"]["appearance_cache"] = Value::Array(
                looks
                    .iter()
                    .zip(colours)
                    .map(|(look, colour)| {
                        serde_json::json!({"look": look.to_string(), "color": colour.to_string()})
                    })
                    .collect(),
            );
            for row in save["skills"].as_array_mut().expect("skill rows") {
                if matches!(row["id"].as_str(), Some("0" | "1" | "2")) {
                    row["static"] = Value::String("40".into());
                    row["dynamic"] = Value::String("40".into());
                    row["experience"] = Value::String("37224.0".into());
                }
            }
        },
    );
    write_save(&players, "traceone", "looktwo", [3231, 3230, 0], &[]);
    let mut one = enter(address, "lookone").await;
    let mut two = enter(address, "looktwo").await;
    let index_one = one.index;
    two.until(Duration::from_secs(10), "lookone's appearance", |c| {
        c.appearance_of(index_one).is_some()
    })
    .await;
    let seen = two.appearance_of(index_one).unwrap().clone();
    assert_eq!(seen.flags & 1, 1, "lookone is female: {seen:?}");
    assert_eq!(
        seen.flags & !1,
        0,
        "size 1 outside the wilderness: {seen:?}"
    );
    assert_eq!((seen.skull_icon, seen.prayer_icon), (-1, -1), "{seen:?}");
    // prepareBodyData of a naked character: chest, arms, legs, hair, hands,
    // feet and the beard part from the saved looks (0x100 + look).
    assert_eq!(
        seen.slots,
        [
            0,
            0,
            0,
            0,
            looks[2] + 0x100,
            0,
            looks[3] + 0x100,
            looks[5] + 0x100,
            looks[0] + 0x100,
            looks[4] + 0x100,
            looks[6] + 0x100,
            looks[1] + 0x100
        ],
        "{seen:?}"
    );
    // Hair, torso, legs, feet and skin colours.
    assert_eq!(seen.colors, [5, 3, 7, 2, 4], "{seen:?}");
    assert_eq!(seen.bas, 1426, "the default render animation: {seen:?}");
    assert_eq!(seen.name37, name37("lookone"), "{seen:?}");
    // 0.25 * (40 Defence + 10 Hitpoints) + 0.325 * (40 Attack + 40 Strength).
    assert_eq!(
        (seen.combat, seen.combat_with_summoning, seen.combat_range),
        (38, 38, -1),
        "{seen:?}"
    );
    // lookone's own block is the same field.
    one.until(Duration::from_secs(10), "its own appearance", |c| {
        c.appearance_of(players::SELF as i32).is_some()
    })
    .await;
    assert_eq!(one.appearance_of(players::SELF as i32), Some(&seen));
    eprintln!("looktwo reads lookone's saved gender, looks, colours, name and combat level 38");

    one.send(item_option2(1307, 0)).await;
    two.until(
        Duration::from_secs(10),
        "lookone's appearance with the two-handed sword",
        |c| {
            c.appearance_of(index_one)
                .is_some_and(|a| a.slots[3] >= 0x8000)
        },
    )
    .await;
    let armed = two.appearance_of(index_one).unwrap().clone();
    assert_eq!(armed.bas, 124, "the sword's render animation: {armed:?}");
    // A two-handed weapon leaves the shield slot empty; the body is unchanged.
    assert_eq!(armed.slots[5], 0, "{armed:?}");
    assert_eq!(armed.slots[4], seen.slots[4], "{armed:?}");
    assert_eq!(armed.colors, seen.colors, "{armed:?}");
    eprintln!("looktwo sees lookone wield 1307: weapon slot worn, render animation 124");
    one.writer.close(1000, "logout").await.unwrap();
    two.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

/// Reconnect (login operation 18): `reconone` walks, then its connection
/// drops. The original answers a reconnect AlreadyOnline while the old
/// player is still in the repository (`Login.kt` reads
/// `Repository.getPlayerByName`), so the client retries; once the
/// disconnection queue has saved and removed the old player the reconnect
/// is admitted: `LoginParser.reconnect` with `Player.initReconnect` runs in
/// place of `Player.init`, and the character is back where it stood, sees
/// itself and walks. `recontwo` stays online and sees `reconone` return.
async fn reconnect_scenario(address: std::net::SocketAddr, saves: &std::path::Path) {
    let players = saves.join("players");
    write_save(&players, "traceone", "reconone", [3236, 3222, 0], &[]);
    write_save(&players, "traceone", "recontwo", [3236, 3224, 0], &[]);
    let mut one = enter(address, "reconone").await;
    let mut two = enter(address, "recontwo").await;
    let old_index = one.index;
    two.until(Duration::from_secs(10), "reconone in view", |c| {
        c.players.players[old_index as usize].is_some()
    })
    .await;
    let stood = [3238, 3222, 0];
    one.send(actions::worldspace_walk(stood[0], stood[1], false))
        .await;
    one.until(Duration::from_secs(10), "reconone's walk", |c| {
        c.position() == stood
    })
    .await;
    // The connection is lost: no logout, the socket just goes.
    drop(one);
    let started = Instant::now();
    let mut refused = 0;
    let mut one = loop {
        let (code, client) = Client::login_as(address, "reconone", true).await;
        match (code, client) {
            (2, Some(client)) => break client,
            // AlreadyOnline: the old player has not left the repository yet.
            (5, None) => refused += 1,
            (other, _) => panic!("reconone's reconnect was answered {other}"),
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "reconone's reconnect was refused {refused} times"
        );
        two.pump(Duration::from_millis(300)).await;
    };
    eprintln!(
        "reconone's reconnect was admitted after {} ms ({refused} AlreadyOnline replies)",
        started.elapsed().as_millis()
    );
    one.until(Duration::from_secs(10), "the reconnect's rebuild", |c| {
        c.base.is_some() && !c.updates.is_empty()
    })
    .await;
    assert_eq!(one.position(), stood, "reconone is back where it stood");
    let index = one.index;
    one.until(Duration::from_secs(10), "its own appearance", |c| {
        c.appearance_of(players::SELF as i32).is_some()
    })
    .await;
    assert_eq!(
        one.appearance_of(players::SELF as i32).unwrap().name37,
        name37("reconone"),
        "reconone's own appearance block"
    );
    // The player who stayed sees reconone again, with its appearance, after
    // the reconnect (updates received before it do not count).
    let from = two.updates.len();
    two.until(Duration::from_secs(10), "reconone back in view", |c| {
        c.players.players[index as usize].is_some()
            && c.updates[from.min(c.updates.len())..].iter().any(|(_, u)| {
                u.extended.iter().any(|e| {
                    e.index == index as usize
                        && e.fields.iter().any(|f| {
                            matches!(f, players::Field::Appearance { appearance, .. }
                                if appearance.name37 == name37("reconone"))
                        })
                })
            })
    })
    .await;
    // The reconnected character plays on: it walks and both see it.
    let target = [3238, 3225, 0];
    one.send(actions::worldspace_walk(target[0], target[1], false))
        .await;
    one.until(
        Duration::from_secs(10),
        "reconone's walk after reconnect",
        |c| c.position() == target,
    )
    .await;
    eprintln!("reconone reconnected at {stood:?}, saw itself and walked to {target:?}");
    one.writer.close(1000, "logout").await.unwrap();
    two.writer.close(1000, "logout").await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}
