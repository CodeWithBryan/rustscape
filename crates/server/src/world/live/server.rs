//! The running live world: the transport gate, the 600 ms worker loop
//! (`MajorUpdateWorker`), the routed handlers and the saver thread.
use super::{
    P, SessionId,
    boot::{self, Booted, Options},
    host::{Live, SaveJob, Task},
    inner::{Link, Pending, Session},
    now_millis,
};
use crate::{
    player::Prepared,
    world::{
        driver::{self, dispatch},
        interfaces, login,
        session::{Admission, Joined, World},
    },
};
use rs09_protocol::{
    game::{Direction, Frame},
    login::AuthResponse,
};
use std::{
    collections::HashSet,
    net::IpAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc as std_mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};

/// `MajorUpdateWorker`'s sleep target.
pub const TICK: Duration = Duration::from_millis(600);
/// The worker's ping timeout.
pub const PING_TIMEOUT_MILLIS: i64 = 20_000;
/// Frames waiting per session before it is treated as stalled.
const OUTPUT_BATCHES: usize = 64;
const INPUT_FRAMES: usize = 256;

/// One admission from the transport.
pub struct JoinRequest {
    pub prepared: Prepared,
    pub address: Option<IpAddr>,
    pub reply: oneshot::Sender<Result<Joined, AuthResponse>>,
}

/// The transport's view of the world ([`World`]): admissions go to the
/// worker thread; online names are the repository's.
pub struct Gate {
    joins: Mutex<std_mpsc::Sender<JoinRequest>>,
    online: Arc<Mutex<HashSet<String>>>,
}
impl World for Gate {
    fn is_online(&self, username: &str) -> bool {
        self.online
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(username)
    }
    fn enter(&self, player: Prepared) -> Admission<'_> {
        self.enter_from(player, None)
    }
    fn enter_from(&self, player: Prepared, address: Option<IpAddr>) -> Admission<'_> {
        let (reply, answer) = oneshot::channel();
        let sent = self
            .joins
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send(JoinRequest {
                prepared: player,
                address,
                reply,
            });
        Box::pin(async move {
            if sent.is_err() {
                return Err(AuthResponse::LoginServerOffline);
            }
            answer.await.unwrap_or(Err(AuthResponse::UnexpectedError))
        })
    }
}

/// Tick statistics, shared with whoever started the world.
#[derive(Default)]
pub struct Stats {
    pub ticks: AtomicU64,
    /// Sum and maximum of the work per tick, in microseconds.
    pub work_micros: AtomicU64,
    pub max_work_micros: AtomicU64,
    /// Sum of the interval between tick starts, in microseconds.
    pub interval_micros: AtomicU64,
    pub players: AtomicU64,
    pub saves_written: AtomicU64,
}
impl Stats {
    pub fn mean_work_millis(&self) -> f64 {
        let n = self.ticks.load(Ordering::Relaxed).max(1);
        self.work_micros.load(Ordering::Relaxed) as f64 / n as f64 / 1000.0
    }
    pub fn mean_interval_millis(&self) -> f64 {
        let n = self.ticks.load(Ordering::Relaxed).saturating_sub(1).max(1);
        self.interval_micros.load(Ordering::Relaxed) as f64 / n as f64 / 1000.0
    }
}

/// A started live world.
pub struct Running {
    pub gate: Arc<Gate>,
    pub authenticator: Arc<crate::auth::Authenticator>,
    pub configuration: crate::config::Configuration,
    pub store: rs09_cache::store::Store,
    pub stats: Arc<Stats>,
    pub boot: boot::Report,
    /// The account storage `Auth.configure` made (a test registers an
    /// account in it before its first login, as `AccountRegister` would).
    pub accounts: Arc<dyn crate::auth::storage::AccountStorage>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    saver: Option<JoinHandle<()>>,
}
impl Running {
    /// Stops the worker after its current tick (saving every playing
    /// character first) and waits for the saver to write everything.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(saver) = self.saver.take() {
            let _ = saver.join();
        }
    }
}

/// Boots the world and starts its worker and saver threads.
pub fn start(options: Options) -> Result<Running, String> {
    let (saves, jobs) = std_mpsc::channel::<SaveJob>();
    let stats = Arc::new(Stats::default());
    let saver_stats = stats.clone();
    let saver = std::thread::Builder::new()
        .name("PlayerSave".into())
        .spawn(move || {
            for job in jobs {
                let written = (|| {
                    if let Some(parent) = job.path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&job.path, job.text.as_bytes())
                })();
                match written {
                    Ok(()) => {
                        saver_stats.saves_written.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(e) => eprintln!("[live] save {} failed: {e}", job.path.display()),
                }
            }
        })
        .map_err(|e| e.to_string())?;
    let Booted {
        live,
        configuration,
        authenticator,
        store,
        report,
    } = boot::boot(&options, saves)?;
    let accounts = live.inner.storage.clone();
    let (joins, admissions) = std_mpsc::channel();
    let gate = Arc::new(Gate {
        joins: Mutex::new(joins),
        online: live.online.clone(),
    });
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker_stats = stats.clone();
    let worker = std::thread::Builder::new()
        .name("Major Update Worker".into())
        .spawn(move || worker_loop(live, admissions, worker_stop, worker_stats))
        .map_err(|e| e.to_string())?;
    Ok(Running {
        gate,
        authenticator,
        configuration,
        store,
        stats,
        boot: report,
        accounts,
        stop,
        worker: Some(worker),
        saver: Some(saver),
    })
}

fn worker_loop(
    mut live: Live,
    admissions: std_mpsc::Receiver<JoinRequest>,
    stop: Arc<AtomicBool>,
    stats: Arc<Stats>,
) {
    let mut next_session: SessionId = 1;
    let mut last_start: Option<Instant> = None;
    std::thread::sleep(TICK);
    while !stop.load(Ordering::SeqCst) {
        let start = Instant::now();
        if let Some(last) = last_start {
            stats.interval_micros.fetch_add(
                start.duration_since(last).as_micros() as u64,
                Ordering::Relaxed,
            );
        }
        last_start = Some(start);
        // What an admission queues (the management events' packets) waits
        // in the write queue for this tick's flush, as the original's does.
        while let Ok(join) = admissions.try_recv() {
            admit(&mut live, join, next_session);
            next_session += 1;
        }
        read_input(&mut live);
        tick(&mut live);
        worker_checks(&mut live);
        stats.ticks.fetch_add(1, Ordering::Relaxed);
        stats
            .players
            .store(live.inner.players.len() as u64, Ordering::Relaxed);
        let work = start.elapsed();
        stats
            .work_micros
            .fetch_add(work.as_micros() as u64, Ordering::Relaxed);
        stats
            .max_work_micros
            .fetch_max(work.as_micros() as u64, Ordering::Relaxed);
        if let Some(rest) = TICK.checked_sub(work) {
            std::thread::sleep(rest);
        }
    }
    // Shutdown: every playing character is saved as the autosave does.
    let repository = live.repository.clone();
    if let Err(error) = (driver::Autosave {
        ticks: live.driver.ticks().get(),
    })
    .run(&mut live, &repository)
    {
        eprintln!("[live] shutdown save failed: {error:?}");
    }
    run_tasks(&mut live);
}

/// The admission of one prepared login (`Login.proceedWith` and the rest of
/// the network-thread half).
fn admit(live: &mut Live, join: JoinRequest, id: SessionId) {
    let JoinRequest {
        prepared,
        address,
        reply,
    } = join;
    if reply.is_closed() {
        return;
    }
    let (incoming, input) = mpsc::channel(INPUT_FRAMES);
    let (output, outgoing) = mpsc::channel(OUTPUT_BATCHES);
    let username = prepared.login.username.clone();
    live.inner.sessions.insert(
        id,
        Session {
            active: true,
            object: None,
            username: Some(username),
            address: address.map(|a| a.to_string()),
            client: prepared.client,
            direct: Vec::new(),
            reading_queue: None,
            last_ping: now_millis(),
            game: false,
            closing: false,
            link: Link {
                output: Some(output),
                input: Some(input),
                pending: Some(Pending {
                    reply,
                    incoming,
                    outgoing,
                }),
            },
        },
    );
    let repository = live.repository.clone();
    let opcode = prepared.login.opcode;
    let admitted = login::admission::read(live, &repository, &id, prepared.details, opcode);
    match admitted {
        Ok(login::admission::Admitted::Submitted(_)) => {}
        Ok(other) => {
            live.inner.log_line(format!("admission ended: {other:?}"));
            refuse_pending(live, id, AuthResponse::UnexpectedError);
        }
        Err(error) => {
            live.inner.log_line(format!("admission failed: {error:?}"));
            refuse_pending(live, id, AuthResponse::UnexpectedError);
        }
    }
    social_end_of_step(live);
}

fn refuse_pending(live: &mut Live, id: SessionId, response: AuthResponse) {
    if let Some(session) = live.inner.sessions.get_mut(&id) {
        session.active = false;
        if let Some(pending) = session.link.pending.take() {
            let _ = pending.reply.send(Err(response));
        }
        session.closing = true;
    }
}

/// `IoReadEvent` for every session: the transport's frames as the bytes the
/// read event frames again; an ended stream is `IoSession.disconnect`.
fn read_input(live: &mut Live) {
    let ids: Vec<SessionId> = live.inner.sessions.keys().copied().collect();
    for id in ids {
        let mut bytes = Vec::new();
        let mut ended = false;
        {
            let Some(session) = live.inner.sessions.get_mut(&id) else {
                continue;
            };
            if let Some(pending) = &session.link.pending {
                // The client left while its admission waited.
                if pending.reply.is_closed() {
                    session.link.pending = None;
                    ended = true;
                }
            } else if let Some(input) = session.link.input.as_mut() {
                loop {
                    match input.try_recv() {
                        Ok(frame) => {
                            if frame
                                .write(Direction::ClientToServer, None, &mut bytes)
                                .is_err()
                            {
                                ended = true;
                                break;
                            }
                        }
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => {
                            ended = true;
                            break;
                        }
                    }
                }
            }
            if ended {
                session.link.input = None;
            }
        }
        let driver = live.driver.clone();
        if !bytes.is_empty() {
            match driver.read(live, &id, &bytes) {
                Ok(_) => {}
                Err(error) => live.inner.log_line(format!("read event failed: {error:?}")),
            }
        }
        if ended {
            let was_active = live.inner.sessions.get(&id).is_some_and(|s| s.active);
            if was_active {
                if let Err(error) = driver.close(live, &id) {
                    live.inner.log_line(format!("disconnect failed: {error:?}"));
                }
            }
        }
    }
}

/// One tick: `handleTickActions`, then the hand-offs, the management
/// events and the delivery of every session's frames.
fn tick(live: &mut Live) {
    live.inner.ticks = live.driver.ticks().get();
    live.walk.animations.ticks = live.inner.ticks;
    live.inner.state.ticks = live.inner.ticks;
    live.inserted = 0;
    let driver = live.driver.clone();
    if let Err(error) = driver.tick(live, false) {
        eprintln!("[live] tick failed: {error:?}");
        live.inner.log_line(format!("tick failed: {error:?}"));
    }
    run_tasks(live);
    social_end_of_step(live);
    deliver(live);
    collect(live);
}

/// The management-event pump, then the social packet queue's flush.
fn social_end_of_step(live: &mut Live) {
    live.social.pump(&mut live.inner);
    if let Err(error) = live.social.flush(&mut live.inner) {
        live.inner
            .log_line(format!("social flush failed: {error:?}"));
    }
}

/// `TaskExecutor`'s blocks: saves and the autosave. The save text is built
/// here, on the world thread; the saver thread writes the file.
fn run_tasks(live: &mut Live) {
    while let Some(task) = live.tasks.pop_front() {
        let result = match task {
            Task::Save(task) => {
                let queue = live.repository.disconnection_queue();
                task.run(live, &queue).map(|_| ())
            }
            Task::Autosave(task) => {
                let repository = live.repository.clone();
                task.run(live, &repository)
            }
        };
        if let Err(error) = result {
            live.inner.log_line(format!("save task failed: {error:?}"));
        }
    }
}

/// Every session's frames of this tick: written at once, then the queue.
fn deliver(live: &mut Live) {
    let mut queued: std::collections::BTreeMap<SessionId, Vec<Frame>> = Default::default();
    for (session, frame) in std::mem::take(&mut live.inner.queue) {
        queued.entry(session).or_default().push(frame);
    }
    let mut stalled = Vec::new();
    for (id, session) in live.inner.sessions.iter_mut() {
        let mut frames = std::mem::take(&mut session.direct);
        frames.extend(queued.remove(id).unwrap_or_default());
        if !frames.is_empty() {
            if let Some(output) = &session.link.output {
                match output.try_send(frames) {
                    Ok(()) => {}
                    Err(mpsc::error::TrySendError::Full(_)) => stalled.push(*id),
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        session.link.output = None;
                    }
                }
            }
        }
        if session.closing {
            session.link.output = None;
            session.link.input = None;
        }
    }
    for id in stalled {
        live.inner.log_line(format!(
            "session {id} stopped reading its frames: disconnected"
        ));
        let driver = live.driver.clone();
        if live.inner.sessions.get(&id).is_some_and(|s| s.active) {
            let _ = driver.close(live, &id);
        }
        if let Some(session) = live.inner.sessions.get_mut(&id) {
            session.link.output = None;
        }
    }
}

/// Forgets players the repository no longer holds and sessions nothing
/// refers to.
fn collect(live: &mut Live) {
    let in_repository: HashSet<usize> = live
        .repository
        .players()
        .occupied()
        .into_iter()
        .map(|(_, p)| p.node())
        .collect();
    let pending_logins: HashSet<usize> = live.logins.iter().map(|(_, l)| l.player.node()).collect();
    let gone: Vec<usize> = live
        .inner
        .players
        .keys()
        .copied()
        .filter(|n| !in_repository.contains(n) && !pending_logins.contains(n))
        .filter(|n| {
            let p = live.player_of(*n);
            p.is_none_or(|p| !live.driver.renderable_players().contains(&p))
        })
        // A player still in the disconnection queue is read by its next
        // update (allowRemoval: combat, death, teleport): kept until it
        // leaves the queue.
        .filter(|n| {
            let name = live.inner.players.get(n).map(|s| s.name.clone());
            !live
                .repository
                .disconnection_queue()
                .contains(name.as_deref())
        })
        .collect();
    for node in gone {
        if let Some(slot) = live.inner.players.remove(&node) {
            live.online
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&slot.name);
            live.walk.maps.players.remove(&node);
            live.inner.frames.remove(&node);
            live.regions.monitors.remove(&node);
            live.regions.viewports.remove(&node);
            live.round_logout(node);
            live.inner.log_line(format!("{} left the world", slot.name));
        }
    }
    let referenced: HashSet<SessionId> = live
        .inner
        .players
        .values()
        .filter_map(|s| s.session)
        .collect();
    live.inner.sessions.retain(|id, s| {
        referenced.contains(id)
            || s.link.pending.is_some()
            || (s.active && s.object.is_none() && s.link.output.is_some())
    });
    // The canonical state's collection (the native stand-in for the JVM's
    // garbage collector) traces every retained node, the ~22k joined NPCs
    // included: it runs every 100 ticks (a departed player's nodes are
    // released then).
    if live.inner.ticks % 100 == 0 {
        live.inner.state.collect();
    }
}

/// The worker loop's checks after `handleTickActions`: the 20-second ping
/// timeout and the inactive-player failsafe. The daily restart is off in the
/// configuration (`daily_restart = false`) and its ServerStore clears are
/// store archive writes that are not ported: not run.
fn worker_checks(live: &mut Live) {
    let now = now_millis();
    let players: Vec<P> = live
        .repository
        .players()
        .occupied()
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    for p in players {
        let Some(slot) = live.inner.players.get(&p.node()) else {
            continue;
        };
        let (name, active) = (slot.name.clone(), slot.active);
        let Some(id) = slot.session else { continue };
        let last_ping = live
            .inner
            .sessions
            .get(&id)
            .map_or(i64::MAX, |s| s.last_ping);
        let driver = live.driver.clone();
        if now.wrapping_sub(last_ping) > PING_TIMEOUT_MILLIS {
            if let Some(s) = live.inner.sessions.get_mut(&id) {
                s.last_ping = i64::MAX;
            }
            if let Err(error) = driver.close(live, &id) {
                live.inner
                    .log_line(format!("ping timeout disconnect failed: {error:?}"));
            }
        }
        let queued = live.repository.disconnection_queue().contains(Some(&name));
        let fully = live
            .inner
            .players
            .get(&p.node())
            .and_then(|s| {
                s.canonical
                    .player
                    .attributes()
                    .boolean("logged-in-fully", false)
                    .ok()
            })
            .unwrap_or(false);
        if !active && !queued && fully {
            let _ = driver.close(live, &id);
            live.inner.log_line(format!(
                "Manually disconnecting {name} because they were set as inactive without being disconnected. This is bad."
            ));
        }
    }
}

/// `PacketProcessor.process`'s branches bound to native handler libraries.
pub fn handle(
    live: &mut Live,
    handler: dispatch::Handler,
    packet: &dispatch::Packet<P>,
) -> driver::Result<()> {
    use dispatch::Handler;
    match (handler, packet) {
        (Handler::Walk, dispatch::Packet::Action { player, action, .. }) => {
            let node = player.node();
            live.with_walk(|world| world.process(node, action).map(|_| ()))
                .map_err(super::host::fail)
        }
        (Handler::Social, dispatch::Packet::Social { player, packet, .. }) => live
            .social
            .process(&mut live.inner, player, packet)
            .map_err(super::host::fail),
        (
            Handler::DisplayUpdate,
            dispatch::Packet::Action {
                player,
                action:
                    rs09_protocol::game::ClientAction::TrackingDisplayUpdate {
                        window_mode,
                        screen_width,
                        screen_height,
                        display_mode,
                    },
                ..
            },
        ) => {
            {
                let client = &mut live.inner.slot_mut(player).canonical.client;
                client.screen_width = *screen_width;
                client.screen_height = *screen_height;
                client.display_mode = *display_mode;
            }
            live.inner.current = Some(player.clone());
            let manager = live.inner.canonical(player).player.manager();
            interfaces::switch_window_mode(&mut live.inner, &manager, *window_mode)
                .map_err(super::host::fail)
        }
        (
            Handler::IfAction,
            dispatch::Packet::Action {
                player,
                action:
                    rs09_protocol::game::ClientAction::IfAction {
                        opcode,
                        opt_index,
                        iface,
                        child,
                        slot,
                        item_id,
                    },
                ..
            },
        ) => live.if_action(
            player,
            crate::world::interface_packets::Action {
                iface: *iface,
                child: *child,
                slot: *slot,
                item: *item_id,
                opcode: *opcode,
                opt_index: *opt_index,
            },
        ),
        (Handler::CloseIface, dispatch::Packet::Action { player, .. }) => live.close_iface(player),
        (
            Handler::ContinueOption,
            dispatch::Packet::Action {
                player,
                action:
                    rs09_protocol::game::ClientAction::ContinueOption {
                        iface,
                        child,
                        slot,
                        opcode,
                    },
                ..
            },
        ) => live
            .continue_option(
                player,
                crate::world::interface_packets::Action {
                    iface: *iface,
                    child: *child,
                    slot: *slot,
                    item: -1,
                    opcode: *opcode,
                    opt_index: -1,
                },
            )
            .map_err(super::host::fail),
        (Handler::InputPrompt, dispatch::Packet::Action { player, action, .. }) => {
            let value = match action {
                rs09_protocol::game::ClientAction::InputIntResponse { response } => {
                    crate::world::input_prompt::Response::Int(*response)
                }
                rs09_protocol::game::ClientAction::InputStringResponse { response } => {
                    crate::world::input_prompt::Response::Text(response.encode_utf16().collect())
                }
                _ => {
                    return Err(super::host::fail(
                        "an input response without its decoded form",
                    ));
                }
            };
            live.input_response(player, value)
                .map_err(super::host::fail)
        }
        (
            Handler::GroundItemAction,
            dispatch::Packet::Action {
                player,
                action: action @ rs09_protocol::game::ClientAction::GroundItemAction { .. },
                ..
            },
        ) => live
            .ground_action(player.node(), action)
            .map_err(super::host::fail),
        (
            Handler::SceneryAction,
            dispatch::Packet::Action {
                player,
                action: action @ rs09_protocol::game::ClientAction::SceneryAction { .. },
                ..
            },
        ) => live
            .scenery_action(player.node(), action)
            .map_err(super::host::fail),
        (Handler::UseWith, dispatch::Packet::Action { player, action, .. }) => {
            match crate::world::packet_handlers::use_with::UseWith::from_action(action) {
                Some(request) => live.use_with(player, request).map_err(super::host::fail),
                None => Err(super::host::fail(
                    "a use-with packet without its decoded form",
                )),
            }
        }
        (Handler::SlotSwitch, dispatch::Packet::Action { player, action, .. }) => {
            match crate::world::packet_handlers::slot_switch::SlotSwitch::from_action(action) {
                Some(request) => live.slot_switch(player, request).map_err(super::host::fail),
                None => Err(super::host::fail(
                    "a slot switch packet without its decoded form",
                )),
            }
        }
        (
            Handler::ItemAction
            | Handler::NpcAction
            | Handler::PlayerAction
            | Handler::Examine
            | Handler::TrackFinished,
            dispatch::Packet::Action { player, action, .. },
        ) => live.item_packet(player, action).map_err(super::host::fail),
        (handler, packet) => {
            // A routed handler whose library is ported but not installed in
            // this world: refused, logged and recorded, never a success.
            live.inner.log_line(format!(
                "Not installed in the live world: {handler:?} for {} (opcode {}); the packet is dropped",
                packet.class(),
                packet.opcode()
            ));
            live.inner.not_ported.skip(match handler {
                Handler::SceneryAction => "SceneryAction handler (scenery_action not installed)",
                Handler::GroundItemAction => {
                    "GroundItemAction handler (ground_action not installed)"
                }
                Handler::IfAction => "IfAction handler (interface_packets not installed)",
                Handler::CloseIface => "CloseIface handler (interface_packets not installed)",
                Handler::ContinueOption => {
                    "ContinueOption handler (interface_packets not installed)"
                }
                Handler::InputPrompt => "InputPromptResponse handler (input_prompt not installed)",
                _ => "a routed packet without its expected decoded form",
            });
            Ok(())
        }
    }
}

/// `rs09-server --world`: the world, the transport, until Ctrl-C.
pub fn run(
    options: Options,
    tcp: std::net::SocketAddr,
    websocket: Option<std::net::SocketAddr>,
    cache: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Booting the native world (start-up sequence over {})...",
        options.data.display()
    );
    let running = start(options)?;
    println!(
        "World booted in {} ms: start-up complete = {}, {} steps partly ported or refused, {} NPCs joined to the live world.",
        running.boot.millis,
        running.boot.complete,
        running.boot.partial_steps.len(),
        running.boot.npcs
    );
    let limits = rs09_cache::Limits::default();
    let catalog = Arc::new(crate::catalog::Catalog::new(
        rs09_cache::store::Store::open(cache, rs09_cache::store::StorageMode::Resident, limits)?,
        limits,
        4096,
        64 * 1024 * 1024,
    )?);
    let config = crate::runtime::Config {
        authenticator: Some(running.authenticator.clone()),
        world: Some(running.gate.clone()),
        ..crate::runtime::Config::default()
    };
    let seed = now_millis();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("rs09-io")
        .enable_all()
        .build()?;
    let stats = running.stats.clone();
    let final_stats = stats.clone();
    runtime.block_on(async move {
        let service = crate::runtime::Service::new(catalog, config, seed)?;
        let server = service.start(tcp, websocket).await?;
        println!("Native world listening on TCP {}", server.tcp);
        if let Some(address) = server.websocket {
            println!("Binary WebSocket listener: ws://{address}/");
        }
        let report = {
            let stats = stats.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(60));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    println!(
                        "ticks {}: mean work {:.2} ms, max {:.2} ms, mean interval {:.1} ms, {} players",
                        stats.ticks.load(Ordering::Relaxed),
                        stats.mean_work_millis(),
                        stats.max_work_micros.load(Ordering::Relaxed) as f64 / 1000.0,
                        stats.mean_interval_millis(),
                        stats.players.load(Ordering::Relaxed),
                    );
                }
            })
        };
        tokio::signal::ctrl_c().await?;
        report.abort();
        server.shutdown().await?;
        Ok::<_, std::io::Error>(())
    })?;
    println!(
        "Stopping: {} ticks, mean work {:.2} ms, mean interval {:.1} ms.",
        final_stats.ticks.load(Ordering::Relaxed),
        final_stats.mean_work_millis(),
        final_stats.mean_interval_millis()
    );
    running.stop();
    Ok(())
}
