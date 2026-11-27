//! The world tick: `MajorUpdateWorker.handleTickActions` with the verified
//! libraries installed in its phases, in the original order.
//!
//! | Phase | Original | Here |
//! | --- | --- | --- |
//! | packets | `PacketProcessor.processQueue` | [`super::packet_queue::Queue`] with the [`dispatch`] table |
//! | disconnections | `Repository.disconnectionQueue.update()` | [`super::disconnection::DisconnectionQueue::update`] on the driver's [`Repository`] |
//! | pulses | `GameWorld.Pulser.updateAll()` | [`crate::pulses::Runner::update_all`]; pulse bodies are [`Host::pulse_body`] |
//! | tick listeners | `GameWorld.tickListeners.forEach { it.tick() }` | [`Host::tick_listeners`] |
//! | update sequence | `UpdateSequence.start/run/end` | [`super::update::Sequence`] over the repository's lobby and NPC lists and the driver's renderable players; `Player.update` and `Player.reset` are ported here, their parts are services |
//! | world pulse | `GameWorld.pulse()` | the tick counter ([`Ticks`]) and the 50-tick [`Autosave`] hand-off, here |
//! | managers | `Managers.tick()` | [`Host::managers_tick`] |
//! | flush | `PacketWriteQueue.flush()` in `finally` | [`write_queue::WriteQueue::flush`] |
//!
//! The phase order, the `catch (Exception)` around them and the flush in
//! `finally` are [`super::tick::handle`]'s. The read event that frames and
//! decodes incoming bytes is [`Driver::read`]; a closed connection is
//! [`Driver::close`] (`IoSession.disconnect`, then `Player.clear`); the
//! disconnection queue then finishes the player at a tick that is a multiple
//! of three, as [`super::disconnection`] ports.
//!
//! A library and a test: nothing installs it in a server. Everything the
//! driver does not port is a required method of [`Host`], with no default.
//! Not ported here: the 600 ms worker thread and its sleep, `Server.heartbeat`,
//! the 20-second ping timeout, the inactive-player failsafe, the daily
//! restart and store reset (all in the worker loop around
//! `handleTickActions`), `tickOffline`, and the bodies of `Player.tick`,
//! `NPC.tick`, `NPC.reset`, the renderers, chunk reset and the ground item
//! pulse, which are services. See docs/tick-driver.md.
pub mod dispatch;
pub mod write_queue;

use super::{
    disconnection, packet_queue,
    repository::{self, Repository},
    tick, update,
};
use crate::pulses;
use dispatch::{Body, Buffer, Decoded, Handler, Packet, Route, Stop};
use std::{
    marker::PhantomData,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    },
};
use write_queue::WriteQueue;

/// `GameWorld.pulse`: the autosave runs when the counter is a multiple of this.
pub const AUTOSAVE_INTERVAL: i32 = 50;
/// `IoReadEvent.queueBuffer` allocates this much room beyond what it keeps.
pub const READ_QUEUE_SLACK: usize = 100_000;

/// What stops a call, in the categories the original's `catch` clauses use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// An `Exception`.
    Exception(String),
    /// A `Throwable` that is not an `Exception`.
    Throwable(String),
    /// A native bound with no counterpart in the original; nothing catches it.
    Native(String),
}
impl Error {
    /// Whether `catch (Exception)` stops it.
    pub fn is_exception(&self) -> bool {
        matches!(self, Self::Exception(_))
    }
    /// Whether `catch (Throwable)` stops it.
    pub fn is_throwable(&self) -> bool {
        !matches!(self, Self::Native(_))
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

impl From<repository::Error> for Error {
    fn from(error: repository::Error) -> Self {
        match error {
            repository::Error::Throwable(text) => Self::Throwable(text),
            repository::Error::Capacity => Self::Native("node list capacity".into()),
            other => Self::Exception(format!("{other:?}")),
        }
    }
}
/// For a repository service that fails in the driver: the categories carry
/// over; a native bound becomes the repository's own native bound
/// (`Capacity`), the one error nothing in the original catches.
impl From<Error> for repository::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::Exception(text) => Self::Exception(text),
            Error::Throwable(text) => Self::Throwable(text),
            Error::Native(_) => Self::Capacity,
        }
    }
}

/// The levels of `core.tools.Log`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Fine,
    Info,
    Warn,
    Err,
}

/// `Grafana`'s fields the tick writes, and the pulse runner's telemetry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Metric {
    PacketProcess,
    NpcTick,
    PlayerTick,
    PlayerRender,
    PulseLength(String),
    PulseCount(String),
    BotPulses,
    OtherPulses,
}

/// `GameWorld.ticks`. A shared handle: the host answers
/// [`repository::Host::ticks`] from a clone.
#[derive(Clone, Default)]
pub struct Ticks(Arc<AtomicI32>);
impl Ticks {
    pub fn get(&self) -> i32 {
        self.0.load(Ordering::SeqCst)
    }
    pub fn set(&self, ticks: i32) {
        self.0.store(ticks, Ordering::SeqCst);
    }
}

/// A packet `PacketProcessor` refused because its handler is not ported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub opcode: u8,
    pub class: &'static str,
    pub missing: &'static str,
}

/// The block `GameWorld.pulse` hands to `TaskExecutor.execute` every
/// [`AUTOSAVE_INTERVAL`] ticks. Whoever takes it runs [`Autosave::run`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Autosave {
    /// The tick counter when it was handed off.
    pub ticks: i32,
}

impl Autosave {
    /// The body: every player of `Repository.players` (a copy of the list
    /// made when the stream starts) that is neither artificial nor
    /// playing-false is saved through `disconnectionQueue.save(p, false)`.
    /// A `Throwable` stops the walk and is printed.
    pub fn run<P: Clone + PartialEq, H: Host<P>>(
        &self,
        host: &mut H,
        repository: &Repository<P>,
    ) -> repository::Result<()> {
        let walked = (|| -> repository::Result<()> {
            let mut players = repository.players().iter()?;
            while players.has_next() {
                let player = players.next_node()?;
                if repository::Host::is_artificial(host, &player) || !host.is_playing(&player) {
                    continue;
                }
                repository
                    .disconnection_queue()
                    .save(host, &player, false)?;
            }
            Ok(())
        })();
        match walked {
            Err(error) if error.is_throwable() => {
                host.print_stack_trace(&error);
                Ok(())
            }
            other => other,
        }
    }
}

/// What the tick needs from the world. There are no defaults: a method
/// without a `Result` is a field read or write of the original; every other
/// is a call into a system this module does not port, and may fail with
/// what that system threw.
pub trait Host<P>: repository::Host<P> {
    /// An `OutgoingPacket` with its context.
    type Outgoing: Clone;

    // Clock, telemetry, logging.
    /// `System.currentTimeMillis()`.
    fn clock_millis(&mut self) -> i64;
    /// A `Grafana` or pulse telemetry write.
    fn telemetry(&mut self, metric: Metric, millis: i32);
    /// `ServerConstants.GRAFANA_LOGGING` as the pulse runner reads it.
    fn metrics_enabled(&mut self) -> bool;
    /// `player.isPlaying()`.
    fn is_playing(&mut self, player: &P) -> bool;
    /// `log(origin, level, message)`.
    fn driver_log(&mut self, origin: &str, level: Level, message: &str);
    /// `e.printStackTrace(pw)` into a `StringWriter`: the text it adds.
    fn stack_trace(&mut self, error: &Error) -> String;
    /// `e.printStackTrace()`.
    fn driver_print_stack_trace(&mut self, error: &Error);

    // The read event.
    /// `IoSession.readingQueue`, if set.
    fn reading_queue(&mut self, session: &Self::Session) -> Option<Vec<u8>>;
    fn set_reading_queue(&mut self, session: &Self::Session, queue: Option<Vec<u8>>);
    /// `session.setLastPing(millis)`.
    fn set_last_ping(&mut self, session: &Self::Session, millis: i64);
    /// `invalidPacketCount++`, answering the new count.
    fn add_invalid_packet(&mut self, player: &P) -> i32;
    /// `Player.opCounts`.
    fn op_counts(&mut self, player: &P) -> &mut [i8; dispatch::OP_COUNT_SLOTS];

    // PacketProcessor.
    /// A packet type bound to a verified native handler library: the world
    /// calls that library. Every [`Handler`] must be bound.
    fn handle(&mut self, handler: Handler, packet: &Packet<P>) -> Result<()>;
    /// `player.details.rights == Rights.ADMINISTRATOR`.
    fn is_administrator(&mut self, player: &P) -> Result<bool>;
    /// `player.isAfkLogout = value`.
    fn set_afk_logout(&mut self, player: &P, value: bool);
    /// `interfaceManager.getPacketCount(increment)`.
    fn packet_count(&mut self, player: &P, increment: i32) -> Result<i32>;

    // The world pulse runner.
    /// `Pulse.pulse()` of a world pulse: true when it is finished.
    fn pulse_body(&mut self, pulse: &pulses::Handle) -> Result<bool>;
    /// `Pulse.stop()` of a world pulse: the base `running = false` unless
    /// the pulse's class overrides `stop` (a world answers its own).
    fn pulse_stop(&mut self, pulse: &pulses::Handle) -> Result<()> {
        pulses::stop_base(pulse);
        Ok(())
    }
    /// `node.isActive()` for a pulse's checked node.
    fn pulse_node_active(&mut self, node: pulses::Node) -> Result<bool>;

    // Tick listeners and managers.
    /// `GameWorld.tickListeners.forEach { it.tick() }`.
    fn tick_listeners(&mut self) -> Result<()>;
    /// `CorePluginTypes.Managers.tick()`.
    fn managers_tick(&mut self) -> Result<()>;

    // The update sequence.
    /// `node.setActive(active)` (the renderable flag is
    /// [`repository::Host::set_renderable`]).
    fn set_active(&mut self, node: &P, active: bool) -> Result<()>;
    /// `ClearMinimapFlag` with `PlayerContext(player)`, or null.
    fn clear_minimap_packet(&mut self, player: &P) -> Option<Self::Outgoing>;
    /// `NPC.tick()`.
    fn npc_tick(&mut self, npc: &P) -> Result<()>;
    /// `NPC.reset()`.
    fn npc_reset(&mut self, npc: &P) -> Result<()>;
    /// `Player.tick()`: `Entity.tick` (scripts, tick event, skills, walking,
    /// timers, masks), the music player, potion timers, prayer drain,
    /// wealth and the off-map check.
    fn player_tick(&mut self, player: &P) -> Result<()>;
    /// `playerFlags.isUpdateSceneGraph()`.
    fn update_scene_graph_flag(&mut self, player: &P) -> Result<bool>;
    /// `playerFlags.setUpdateSceneGraph(value)`.
    fn set_update_scene_graph_flag(&mut self, player: &P, value: bool) -> Result<()>;
    /// `Player.updateSceneGraph(false)`.
    fn update_scene_graph(&mut self, player: &P) -> Result<()>;
    /// `PlayerRenderer.render(player)`.
    fn render_players(&mut self, player: &P) -> Result<()>;
    /// `NPCRenderer.render(player)`.
    fn render_npcs(&mut self, player: &P) -> Result<()>;
    /// `MapChunkRenderer.render(player)`.
    fn render_map_chunks(&mut self, player: &P) -> Result<()>;
    /// `Entity.reset()`: `updateMasks.reset()`, `properties.setTeleporting(false)`.
    fn entity_reset(&mut self, player: &P) -> Result<()>;
    /// `renderInfo.updateInformation()`.
    fn update_render_information(&mut self, player: &P) -> Result<()>;
    /// `getSkills().isLifepointsUpdate()`.
    fn lifepoints_update(&mut self, player: &P) -> Result<bool>;
    fn set_lifepoints_update(&mut self, player: &P, value: bool) -> Result<()>;
    /// `SkillLevel` with `SkillContext(player, HITPOINTS)`, or null.
    fn hitpoints_packet(&mut self, player: &P) -> Option<Self::Outgoing>;
    /// `getAttribute(key, default)` of a boolean attribute.
    fn attribute_bool(&mut self, player: &P, key: &str, default: bool) -> Result<bool>;
    /// `removeAttribute(key)`.
    fn remove_attribute(&mut self, player: &P, key: &str) -> Result<()>;
    /// `ChunkUpdateTracker.resetAll()`.
    fn reset_chunks(&mut self) -> Result<()>;
    /// `GroundItemManager.pulse()`.
    fn ground_items_pulse(&mut self) -> Result<()>;

    // GameWorld.pulse.
    /// `TaskExecutor.execute { .. }` with the autosave block.
    fn execute_autosave(&mut self, task: Autosave) -> Result<()>;

    // PacketWriteQueue.
    /// `OutgoingPacket.send(context)`.
    fn send_packet(&mut self, packet: &Self::Outgoing) -> Result<()>;
}

/// The world's tick state: the repository, `UpdateSequence.renderablePlayers`,
/// the update sequence, `GameWorld.Pulser`, `PacketProcessor`'s queue,
/// `PacketWriteQueue` and `GameWorld.ticks`. Clones are the same world.
pub struct Driver<P, O> {
    repository: Repository<P>,
    renderable: update::List<P>,
    sequence: update::Sequence<P>,
    pulser: pulses::Runner,
    packets: packet_queue::Queue<Packet<P>>,
    writes: WriteQueue<O>,
    ticks: Ticks,
    refusals: Arc<Mutex<Vec<Refusal>>>,
}

impl<P, O> Clone for Driver<P, O> {
    fn clone(&self) -> Self {
        Self {
            repository: self.repository.clone(),
            renderable: self.renderable.clone(),
            sequence: self.sequence.clone(),
            pulser: self.pulser.clone(),
            packets: self.packets.clone(),
            writes: self.writes.clone(),
            ticks: self.ticks.clone(),
            refusals: self.refusals.clone(),
        }
    }
}

impl<P: Clone + PartialEq, O: Clone> Driver<P, O> {
    /// A world over `repository` with tick counter 0, an empty renderable
    /// list, pulse runner, packet queue and write queue.
    pub fn new(repository: Repository<P>) -> Self {
        Self {
            repository,
            renderable: update::List::default(),
            sequence: update::Sequence::default(),
            pulser: pulses::Runner::default(),
            packets: packet_queue::Queue::default(),
            writes: WriteQueue::default(),
            ticks: Ticks::default(),
            refusals: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn repository(&self) -> &Repository<P> {
        &self.repository
    }
    /// `UpdateSequence.renderablePlayers`.
    pub fn renderable_players(&self) -> &update::List<P> {
        &self.renderable
    }
    /// `GameWorld.Pulser`.
    pub fn pulser(&self) -> &pulses::Runner {
        &self.pulser
    }
    /// `PacketProcessor`'s queue.
    pub fn packets(&self) -> &packet_queue::Queue<Packet<P>> {
        &self.packets
    }
    /// `PacketWriteQueue`.
    pub fn writes(&self) -> &WriteQueue<O> {
        &self.writes
    }
    pub fn ticks(&self) -> &Ticks {
        &self.ticks
    }
    /// The packets refused so far because their handler is not ported.
    pub fn refusals(&self) -> Vec<Refusal> {
        self.refusals
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// `MajorUpdateWorker.handleTickActions(skipPulseUpdate)`. An
    /// `Exception` from a phase is printed and ends the phases; the flush
    /// runs in any case. A `Throwable` that is not an `Exception` escapes
    /// after the flush, as does one from the flush itself.
    pub fn tick<H: Host<P, Outgoing = O>>(
        &self,
        host: &mut H,
        skip_pulse_update: bool,
    ) -> Result<()> {
        let mut phases = Phases { driver: self, host };
        tick::handle(&mut phases, skip_pulse_update)
    }

    /// `IoReadEvent.run` with `GameReadEvent.read`: bytes from the session's
    /// socket, after what an earlier read queued. A `Throwable` is printed
    /// and disconnects the session.
    pub fn read<H: Host<P, Outgoing = O>>(
        &self,
        host: &mut H,
        session: &H::Session,
        bytes: &[u8],
    ) -> Result<Stop> {
        let read = (|| {
            let bytes = match host.reading_queue(session) {
                Some(mut queued) => {
                    // `queue.put(buffer)` into room for 100,000 more bytes.
                    if bytes.len() > READ_QUEUE_SLACK {
                        return Err(Error::Exception("java.nio.BufferOverflowException".into()));
                    }
                    queued.extend_from_slice(bytes);
                    host.set_reading_queue(session, None);
                    queued
                }
                None => bytes.to_vec(),
            };
            self.read_buffer(host, session, Buffer { bytes, position: 0 })
        })();
        match read {
            Err(error) if error.is_throwable() => {
                host.driver_print_stack_trace(&error);
                disconnection::session_disconnect(host, &self.repository, session)?;
                Ok(Stop::End)
            }
            other => other,
        }
    }

    fn read_buffer<H: Host<P, Outgoing = O>>(
        &self,
        host: &mut H,
        session: &H::Session,
        mut buffer: Buffer,
    ) -> Result<Stop> {
        while buffer.has_remaining() {
            let opcode = buffer.get();
            if disconnection::session_player(host, &self.repository, session).is_none() {
                continue;
            }
            let header = dispatch::PACKET_SIZES[usize::from(opcode)];
            let mut size = i32::from(header);
            if header < 0 {
                let (found, queued) = dispatch::packet_size(&mut buffer, opcode, header);
                if let Some(queued) = queued {
                    host.set_reading_queue(session, Some(queued));
                }
                size = found;
            }
            if size == -1 {
                let player = self.session_player(host, session)?;
                self.invalid_packet(host, &player)?;
                return Ok(Stop::InvalidSize { opcode });
            }
            if buffer.remaining() < size as usize {
                let op = i32::from(opcode);
                let queued = match header {
                    -2 => buffer.queue(&[op, size >> 8, size]),
                    -1 => buffer.queue(&[op, size]),
                    _ => buffer.queue(&[op]),
                };
                host.set_reading_queue(session, Some(queued.clone()));
                return Ok(Stop::Incomplete { queued });
            }
            let data = buffer.take(size as usize);
            let now = host.clock_millis();
            host.set_last_ping(session, now);
            let player = self.session_player(host, session)?;
            let slot = usize::from(opcode);
            if slot >= dispatch::OP_COUNT_SLOTS {
                return Err(Error::Exception(format!(
                    "java.lang.ArrayIndexOutOfBoundsException: Index {slot} out of bounds for length {}",
                    dispatch::OP_COUNT_SLOTS
                )));
            }
            let counts = host.op_counts(&player);
            let count = counts[slot];
            counts[slot] = count.wrapping_add(1);
            if count >= dispatch::PER_TICK_LIMIT {
                host.driver_log(
                    "GameReadEvent",
                    Level::Fine,
                    &format!("Skipping packet {opcode} because already received more than 10!"),
                );
                return Ok(Stop::Limit { opcode });
            }
            let player = self.session_player(host, session)?;
            match dispatch::decode(player, opcode, &data) {
                Decoded::UnhandledOp => host.driver_log(
                    "GameReadEvent",
                    Level::Warn,
                    &format!("Unhandled opcode: {opcode}"),
                ),
                Decoded::DecodingError(message) => {
                    host.driver_log("GameReadEvent", Level::Err, &message)
                }
                Decoded::NoProcess => {}
                Decoded::Packet(packet) => {
                    self.packets.enqueue(packet_queue::Packet::Dispatch(packet))
                }
            }
        }
        Ok(Stop::End)
    }

    /// `session.getPlayer()` where the original dereferences it.
    fn session_player<H: Host<P, Outgoing = O>>(
        &self,
        host: &mut H,
        session: &H::Session,
    ) -> Result<P> {
        disconnection::session_player(host, &self.repository, session)
            .ok_or_else(|| Error::Exception("java.lang.NullPointerException".into()))
    }

    /// `Player.incrementInvalidPacketCount()`.
    fn invalid_packet<H: Host<P, Outgoing = O>>(&self, host: &mut H, player: &P) -> Result<()> {
        let count = host.add_invalid_packet(player);
        if count >= dispatch::INVALID_PACKET_LIMIT {
            host.player_clear(&self.repository, player)?;
            let name = repository::Host::name(host, player);
            host.driver_log(
                "Player",
                Level::Err,
                &format!(
                    "Disconnecting {} for having a high rate of invalid packets. Potential packet bot misbehaving, or simply really bad connection.",
                    name.as_deref().unwrap_or("null")
                ),
            );
        }
        Ok(())
    }

    /// `IoSession.disconnect()` for a closed connection, as
    /// `IoEventHandler.read` calls it at the end of the stream.
    pub fn close<H: Host<P, Outgoing = O>>(
        &self,
        host: &mut H,
        session: &H::Session,
    ) -> Result<()> {
        disconnection::session_disconnect(host, &self.repository, session).map_err(Error::from)
    }

    /// `PacketRepository.send(class, context)` through
    /// `PacketWriteQueue.handle`: true when it was written at once.
    pub fn send<H: Host<P, Outgoing = O>>(
        &self,
        host: &mut H,
        class: &str,
        packet: Option<O>,
    ) -> Result<bool> {
        self.writes
            .handle(&mut Writes(host, PhantomData), class, packet)
            .map_err(|e| {
                e.unwrap_or_else(|| Error::Exception("java.lang.NullPointerException".into()))
            })
    }

    /// `PacketProcessor.process(pkt)`.
    fn dispatch<H: Host<P, Outgoing = O>>(&self, host: &mut H, packet: &Packet<P>) -> Result<()> {
        let class = packet.class();
        match dispatch::route(class) {
            Route::Native(handler) => host.handle(handler, packet),
            Route::NotPorted(missing) => {
                host.driver_log(
                    "PacketProcessor",
                    Level::Warn,
                    &format!(
                        "Not ported: {class} (opcode {}) needs {missing}; the packet is dropped",
                        packet.opcode()
                    ),
                );
                self.refusals
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(Refusal {
                        opcode: packet.opcode(),
                        class,
                        missing,
                    });
                Ok(())
            }
            Route::Driver(Body::Nothing) => Ok(()),
            Route::Driver(Body::Ping) => {
                let player = packet.player();
                let session = repository::Host::session(host, player)
                    .ok_or_else(|| Error::Exception("java.lang.NullPointerException".into()))?;
                let now = host.clock_millis();
                host.set_last_ping(&session, now);
                Ok(())
            }
            Route::Driver(Body::PacketCount) => {
                let Packet::Action {
                    player,
                    action: rs09_protocol::game::ClientAction::PacketCountUpdate { count },
                    ..
                } = packet
                else {
                    return Err(Error::Native("PacketCountUpdate without its count".into()));
                };
                let current = host.packet_count(player, 0)?;
                host.packet_count(player, count.wrapping_sub(current))?;
                Ok(())
            }
            Route::Driver(Body::AfkTimeout) => {
                let player = packet.player();
                if !host.is_administrator(player)? {
                    host.set_afk_logout(player, true);
                    disconnection::send_logout(host, &self.repository, player)?;
                }
                Ok(())
            }
        }
    }

    /// `GameWorld.pulse()`.
    fn pulse_world<H: Host<P, Outgoing = O>>(&self, host: &mut H) -> Result<()> {
        let ticks = self.ticks.get().wrapping_add(1);
        self.ticks.set(ticks);
        if ticks % AUTOSAVE_INTERVAL == 0 {
            host.execute_autosave(Autosave { ticks })?;
        }
        Ok(())
    }

    /// `Player.update()`: `Entity.update` (empty), the scene graph when
    /// flagged, then the player, NPC and map chunk renderers.
    fn player_update<H: Host<P, Outgoing = O>>(&self, host: &mut H, player: &P) -> Result<()> {
        if host.update_scene_graph_flag(player)? {
            host.update_scene_graph(player)?;
        }
        host.render_players(player)?;
        host.render_npcs(player)?;
        host.render_map_chunks(player)
    }

    /// `Player.reset()`.
    fn player_reset<H: Host<P, Outgoing = O>>(&self, host: &mut H, player: &P) -> Result<()> {
        host.entity_reset(player)?;
        host.set_update_scene_graph_flag(player, false)?;
        host.update_render_information(player)?;
        if host.lifepoints_update(player)? {
            let packet = host.hitpoints_packet(player);
            self.send(host, "SkillLevel", packet)?;
            host.set_lifepoints_update(player, false)?;
        }
        if host.attribute_bool(player, "flagged-for-save", false)? {
            host.save_immediately(player)?;
            host.remove_attribute(player, "flagged-for-save")?;
        }
        host.op_counts(player).fill(0);
        Ok(())
    }
}

/// The write queue's services, from a driver host.
pub struct Writes<'a, P, H>(pub &'a mut H, pub PhantomData<P>);

impl<P, H: Host<P>> write_queue::Host<H::Outgoing> for Writes<'_, P, H> {
    type Error = Error;
    fn send(&mut self, packet: &H::Outgoing) -> Result<()> {
        self.0.send_packet(packet)
    }
    fn is_exception(&mut self, error: &Error) -> bool {
        error.is_exception()
    }
    fn stack_trace(&mut self, error: &Error) -> String {
        self.0.stack_trace(error)
    }
    fn log_error(&mut self, message: &str) {
        self.0.driver_log("PacketWriteQueue", Level::Err, message);
    }
}

/// `handleTickActions`' phases over the driver and its host.
struct Phases<'a, P, O, H> {
    driver: &'a Driver<P, O>,
    host: &'a mut H,
}

impl<P: Clone + PartialEq, O: Clone, H: Host<P, Outgoing = O>> tick::Context
    for Phases<'_, P, O, H>
{
    type Error = Error;
    fn now_millis(&mut self) -> Result<i64> {
        Ok(self.host.clock_millis())
    }
    fn packets(&mut self) -> Result<()> {
        let mut ctx = Processing {
            driver: self.driver,
            host: &mut *self.host,
            failure: None,
        };
        let result = self.driver.packets.process(&mut ctx);
        match (result, ctx.failure) {
            (Ok(()), _) => Ok(()),
            (Err(_), Some(failure)) => Err(failure),
            (Err(error), None) => Err(error),
        }
    }
    fn packet_time(&mut self, millis: i32) -> Result<()> {
        self.host.telemetry(Metric::PacketProcess, millis);
        Ok(())
    }
    fn disconnects(&mut self) -> Result<()> {
        let repository = &self.driver.repository;
        repository
            .disconnection_queue()
            .update(self.host, repository)
            .map_err(Error::from)
    }
    fn pulses(&mut self) -> Result<()> {
        let metrics = self.host.metrics_enabled();
        let mut ctx = PulseContext {
            host: &mut *self.host,
            failure: None,
            metrics,
            player: PhantomData,
        };
        let result = self.driver.pulser.update_all(&mut (), &mut ctx);
        let failure = ctx.failure.take();
        result.map_err(|error| failure.unwrap_or_else(|| pulse_error(error)))
    }
    fn listeners(&mut self) -> Result<()> {
        self.host.tick_listeners()
    }
    fn start(&mut self) -> Result<()> {
        self.sequence(|sequence, ctx| sequence.start(ctx))
    }
    fn run(&mut self) -> Result<()> {
        self.sequence(|sequence, ctx| sequence.run(ctx))
    }
    fn end(&mut self) -> Result<()> {
        self.sequence(|sequence, ctx| sequence.end(ctx))
    }
    fn pulse_world(&mut self) -> Result<()> {
        self.driver.pulse_world(self.host)
    }
    fn managers(&mut self) -> Result<()> {
        self.host.managers_tick()
    }
    fn flush(&mut self) -> Result<()> {
        self.driver
            .writes
            .flush(&mut Writes(&mut *self.host, PhantomData))
    }
    fn exception(&mut self, error: Error) -> Result<()> {
        if error.is_exception() {
            self.host.driver_print_stack_trace(&error);
            return Ok(());
        }
        Err(error)
    }
}

impl<P: Clone + PartialEq, O: Clone, H: Host<P, Outgoing = O>> Phases<'_, P, O, H> {
    fn sequence(
        &mut self,
        f: impl FnOnce(&update::Sequence<P>, &mut Sequencing<'_, P, O, H>) -> update::Result<()>,
    ) -> Result<()> {
        let mut ctx = Sequencing {
            driver: self.driver,
            host: &mut *self.host,
            failure: None,
        };
        let result = f(&self.driver.sequence, &mut ctx);
        let failure = ctx.failure.take();
        result.map_err(|error| failure.unwrap_or_else(|| Error::Exception(format!("{error:?}"))))
    }
}

fn pulse_error(error: pulses::Error) -> Error {
    match error {
        pulses::Error::Binding(text) => Error::Native(text),
        other => Error::Exception(format!("{other:?}")),
    }
}

/// `PacketProcessor.processQueue`'s services.
struct Processing<'a, P, O, H> {
    driver: &'a Driver<P, O>,
    host: &'a mut H,
    /// The error that left the loop, with its category.
    failure: Option<Error>,
}

impl<P: Clone + PartialEq, O: Clone, H: Host<P, Outgoing = O>> packet_queue::Context<Packet<P>>
    for Processing<'_, P, O, H>
{
    type Error = Error;
    fn process(&mut self, packet: &Packet<P>) -> Result<()> {
        self.driver.dispatch(self.host, packet)
    }
    /// `catch (e: Exception)`: anything else leaves the loop.
    fn exception_text(&mut self, error: Error) -> Result<String> {
        if !error.is_exception() {
            self.failure = Some(error.clone());
            return Err(error);
        }
        Ok(self.host.stack_trace(&error))
    }
    fn log(&mut self, packet: &Packet<P>, trace: &str) -> Result<()> {
        self.host.driver_log(
            "PacketProcessor",
            Level::Err,
            &format!("Error Processing {}: {trace}", packet.class()),
        );
        Ok(())
    }
}

/// The world pulse runner's services.
struct PulseContext<'a, P, H> {
    host: &'a mut H,
    failure: Option<Error>,
    metrics: bool,
    player: PhantomData<P>,
}

impl<P, H> PulseContext<'_, P, H> {
    fn keep(&mut self, error: Error) -> pulses::Error {
        let text = format!("{error:?}");
        self.failure = Some(error);
        pulses::Error::Binding(text)
    }
}

impl<P, H: Host<P>> pulses::Context<()> for PulseContext<'_, P, H> {
    fn pulse(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        self.host.pulse_body(pulse).map_err(|e| self.keep(e))
    }
    fn node_active(&mut self, node: pulses::Node, _: &mut ()) -> pulses::Result<bool> {
        self.host.pulse_node_active(node).map_err(|e| self.keep(e))
    }
    fn stop(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<()> {
        self.host.pulse_stop(pulse).map_err(|e| self.keep(e))
    }
    fn exception(&mut self, error: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        // Pulse.update's catch prints the body's failure; the category is
        // the one the body raised.
        let error = self
            .failure
            .take()
            .unwrap_or_else(|| pulse_error(error.clone()));
        self.host.driver_print_stack_trace(&error);
        Ok(())
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding(
            "an entity pulse manager service reached from the world runner".into(),
        ))
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding(
            "an entity pulse manager service reached from the world runner".into(),
        ))
    }
    fn is_dead(&mut self, _: &mut ()) -> pulses::Result<bool> {
        Err(pulses::Error::Binding(
            "an entity pulse manager service reached from the world runner".into(),
        ))
    }
    fn send_message(&mut self, _: pulses::Node, _: &str, _: &mut ()) -> pulses::Result<()> {
        Err(pulses::Error::Binding(
            "an entity pulse manager service reached from the world runner".into(),
        ))
    }
    fn now_millis(&mut self, _: &mut ()) -> pulses::Result<i64> {
        Ok(self.host.clock_millis())
    }
    fn log(&mut self, level: pulses::Level, message: String, _: &mut ()) -> pulses::Result<()> {
        let level = match level {
            pulses::Level::Warn => Level::Warn,
            _ => Level::Err,
        };
        self.host.driver_log("PulseRunner", level, &message);
        Ok(())
    }
    fn pulse_length(&mut self, class: &str, millis: i32, _: &mut ()) -> pulses::Result<()> {
        self.host
            .telemetry(Metric::PulseLength(class.to_owned()), millis);
        Ok(())
    }
    fn count_pulse(&mut self, class: &str, _: &mut ()) -> pulses::Result<()> {
        self.host.telemetry(Metric::PulseCount(class.to_owned()), 1);
        Ok(())
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        // Read from the host when the runner starts; the runner asks once,
        // after the last pulse.
        self.metrics
    }
    fn set_bot_total(&mut self, millis: i32, _: &mut ()) -> pulses::Result<()> {
        self.host.telemetry(Metric::BotPulses, millis);
        Ok(())
    }
    fn set_other_total(&mut self, millis: i32, _: &mut ()) -> pulses::Result<()> {
        self.host.telemetry(Metric::OtherPulses, millis);
        Ok(())
    }
}

/// The update sequence's services.
struct Sequencing<'a, P, O, H> {
    driver: &'a Driver<P, O>,
    host: &'a mut H,
    failure: Option<Error>,
}

impl<P, O, H> Sequencing<'_, P, O, H> {
    fn keep(&mut self, error: Error) -> update::Error {
        let text = format!("{error:?}");
        self.failure = Some(error);
        update::Error::Binding(text)
    }
}

impl<P: Clone + PartialEq, O: Clone, H: Host<P, Outgoing = O>> update::Initialization<P>
    for Sequencing<'_, P, O, H>
{
    fn active(&mut self, node: &P, active: bool) -> update::Result<()> {
        self.host.set_active(node, active).map_err(|e| self.keep(e))
    }
    fn renderable(&mut self, node: &P, renderable: bool) -> update::Result<()> {
        repository::Host::set_renderable(self.host, node, renderable);
        Ok(())
    }
}

impl<P: Clone + PartialEq, O: Clone, H: Host<P, Outgoing = O>> update::Context<P>
    for Sequencing<'_, P, O, H>
{
    fn lobby(&mut self) -> update::Result<update::List<P>> {
        Ok(self.driver.repository.lobby_players())
    }
    fn players(&mut self) -> update::Result<update::List<P>> {
        Ok(self.driver.renderable.clone())
    }
    fn npcs(&mut self) -> update::Result<update::List<P>> {
        Ok(self.driver.repository.renderable_npcs())
    }
    fn clear_minimap(&mut self, node: P) -> update::Result<()> {
        let packet = self.host.clear_minimap_packet(&node);
        self.driver
            .send(self.host, "ClearMinimapFlag", packet)
            .map(|_| ())
            .map_err(|e| self.keep(e))
    }
    fn npc_tick(&mut self, node: P) -> update::Result<()> {
        self.host.npc_tick(&node).map_err(|e| self.keep(e))
    }
    fn player_tick(&mut self, node: P) -> update::Result<()> {
        self.host.player_tick(&node).map_err(|e| self.keep(e))
    }
    fn player_update(&mut self, node: P) -> update::Result<()> {
        self.driver
            .player_update(self.host, &node)
            .map_err(|e| self.keep(e))
    }
    fn player_reset(&mut self, node: P) -> update::Result<()> {
        self.driver
            .player_reset(self.host, &node)
            .map_err(|e| self.keep(e))
    }
    fn npc_reset(&mut self, node: P) -> update::Result<()> {
        self.host.npc_reset(&node).map_err(|e| self.keep(e))
    }
    fn now_millis(&mut self) -> update::Result<i64> {
        Ok(self.host.clock_millis())
    }
    fn metric(&mut self, metric: update::Metric, millis: i32) -> update::Result<()> {
        let metric = match metric {
            update::Metric::NpcTick => Metric::NpcTick,
            update::Metric::PlayerTick => Metric::PlayerTick,
            update::Metric::PlayerRender => Metric::PlayerRender,
        };
        self.host.telemetry(metric, millis);
        Ok(())
    }
    fn reset_chunks(&mut self) -> update::Result<()> {
        self.host.reset_chunks().map_err(|e| self.keep(e))
    }
    fn ground_items(&mut self) -> update::Result<()> {
        self.host.ground_items_pulse().map_err(|e| self.keep(e))
    }
}
