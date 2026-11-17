//! A bounded, data-only connection driver. No renderer, inference or gameplay
//! callbacks execute in the transport task. Read and write futures run separately.
use super::{SceneryAction, Session};
use rs09_net::{Chunk, Reader, Writer, login::WorldLogin};
use rs09_protocol::game::{Decoder, Direction, Frame};
use std::{
    future::Future,
    io,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch},
    task::JoinHandle,
    time::{Instant, MissedTickBehavior, interval_at, timeout},
};

const CYCLE: Duration = Duration::from_millis(20);
const BYTE_LIMIT: usize = 1024 * 1024;
const FRAME_LIMIT: usize = 256;
const COMMAND_LIMIT: usize = 64;
const WRITE_TIMEOUT: Duration = Duration::from_secs(15);

// Static dispatch keeps the real Writer in production and lets fault tests hold
// a write future pending without relying on OS loopback buffering behavior.
trait Output: Send {
    fn publish(&mut self, wire: Vec<u8>) -> impl Future<Output = io::Result<()>> + Send;
    fn close(&mut self) -> impl Future<Output = io::Result<()>> + Send;
}
impl Output for Writer {
    async fn publish(&mut self, wire: Vec<u8>) -> io::Result<()> {
        self.write(wire.into()).await
    }
    async fn close(&mut self) -> io::Result<()> {
        self.close(1000, "session closed").await
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CloseReason {
    Requested,
    PeerClosed,
    ServerLogout,
    ReadTimeout,
    WriteTimeout,
    ObservationOverflow,
    Protocol(String),
    Transport(String),
}
impl CloseReason {
    fn error(&self) -> io::Error {
        let kind = match self {
            Self::ReadTimeout | Self::WriteTimeout => io::ErrorKind::TimedOut,
            Self::PeerClosed | Self::ServerLogout => io::ErrorKind::UnexpectedEof,
            Self::Requested => io::ErrorKind::NotConnected,
            Self::Protocol(_) => io::ErrorKind::InvalidData,
            _ => io::ErrorKind::Other,
        };
        io::Error::new(kind, format!("headless session closed: {self:?}"))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Connected,
    Closed(CloseReason),
}

/// The original game-cycle counters. Complete packets reset reads; only a
/// successful socket write resets writes. Timeout precedes the keepalive branch.
#[derive(Default)]
struct Cycles {
    read: i32,
    write: i32,
}
impl Cycles {
    fn received(&mut self) {
        self.read = 0;
    }
    fn written(&mut self) {
        self.write = 0;
    }
    fn tick(&mut self) -> Result<bool, CloseReason> {
        self.read = self.read.wrapping_add(1);
        if self.read > 750 {
            return Err(CloseReason::ReadTimeout);
        }
        self.write = self.write.wrapping_add(1);
        Ok(self.write > 50)
    }
}

struct Observation {
    frame: Frame,
    _budget: OwnedSemaphorePermit,
}
struct Write {
    wire: Vec<u8>,
    reply: Option<oneshot::Sender<io::Result<Vec<u8>>>>,
    _budget: Option<OwnedSemaphorePermit>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub frames_received: u64,
    pub commands_written: u64,
    pub keepalives_written: u64,
}
#[derive(Default)]
struct Metrics {
    frames: AtomicU64,
    commands: AtomicU64,
    keepalives: AtomicU64,
}
impl Metrics {
    fn snapshot(&self) -> Stats {
        Stats {
            frames_received: self.frames.load(Ordering::Relaxed),
            commands_written: self.commands.load(Ordering::Relaxed),
            keepalives_written: self.keepalives.load(Ordering::Relaxed),
        }
    }
}

/// A cloneable command sender. A successful send confirms transport publication,
/// not gameplay acceptance. Commands already admitted survive caller cancellation.
#[derive(Clone)]
pub struct Sender {
    commands: mpsc::Sender<Write>,
    budget: Arc<Semaphore>,
    status: watch::Receiver<Status>,
    metrics: Arc<Metrics>,
}
impl Sender {
    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }
    /// Independently sampled counters, incremented only after acceptance/write.
    pub fn stats(&self) -> Stats {
        self.metrics.snapshot()
    }
    fn closed_error(&self) -> io::Error {
        match self.status() {
            Status::Closed(reason) => reason.error(),
            Status::Connected => {
                io::Error::new(io::ErrorKind::NotConnected, "session driver stopped")
            }
        }
    }
    pub async fn send_frame(&self, frame: &Frame) -> io::Result<Vec<u8>> {
        // Validation must precede queue admission or any network writes.
        let mut wire = vec![];
        frame
            .write(Direction::ClientToServer, None, &mut wire)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if let Status::Closed(reason) = self.status() {
            return Err(reason.error());
        }
        let budget = self
            .budget
            .clone()
            .acquire_many_owned(wire.len() as u32)
            .await
            .map_err(|_| self.closed_error())?;
        let (reply, received) = oneshot::channel();
        self.commands
            .send(Write {
                wire,
                reply: Some(reply),
                _budget: Some(budget),
            })
            .await
            .map_err(|_| self.closed_error())?;
        received.await.map_err(|_| self.closed_error())?
    }
    pub async fn walk(&self, tile: [i32; 2], running: bool) -> io::Result<Vec<u8>> {
        self.send_frame(&rs09_protocol::game::actions::worldspace_walk(
            tile[0], tile[1], running,
        ))
        .await
    }
    pub async fn scenery(&self, action: SceneryAction) -> io::Result<Vec<u8>> {
        self.send_frame(&action.frame()?).await
    }
}

/// Owns one background connection task. The observation queue is bounded to256
/// frames and1MiB of payload/header storage; overflow closes explicitly instead
/// of silently losing game data. Draining queued frames preserves failure prefixes.
pub struct ManagedSession {
    metadata: WorldLogin,
    sender: Sender,
    observations: mpsc::Receiver<Observation>,
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
}
impl ManagedSession {
    pub(super) fn start(session: Session) -> Self {
        let Session {
            metadata,
            reader,
            writer,
            decoder,
        } = session;
        Self::drive(metadata, reader, writer, decoder)
    }
    fn drive<W: Output + 'static>(
        metadata: WorldLogin,
        mut reader: Reader,
        mut writer: W,
        decoder: Decoder,
    ) -> Self {
        let (commands, mut requests) = mpsc::channel(COMMAND_LIMIT);
        let (observations, frames) = mpsc::channel(FRAME_LIMIT);
        let (state, status) = watch::channel(Status::Connected);
        let (stop, mut stopping) = watch::channel(false);
        let budget = Arc::new(Semaphore::new(BYTE_LIMIT));
        let metrics = Arc::new(Metrics::default());
        let sender = Sender {
            commands: commands.clone(),
            budget: budget.clone(),
            status,
            metrics: metrics.clone(),
        };
        let task = tokio::spawn(async move {
            let (written, writes) = watch::channel(0_u64);
            let reason = tokio::select! {
                reason = read_loop(&mut reader, decoder, observations, commands, writes, &metrics) => reason,
                reason = write_loop(&mut writer, &mut requests, written, &metrics) => reason,
                _ = stopping.changed() => CloseReason::Requested,
            };
            // Publish the reason before dropping requests/acks and waking waiters.
            state.send_replace(Status::Closed(reason));
            budget.close();
            requests.close();
            drop(requests);
            let _ = timeout(Duration::from_secs(1), writer.close()).await;
        });
        Self {
            metadata,
            sender,
            observations: frames,
            stop,
            task: Some(task),
        }
    }
    pub fn metadata(&self) -> &WorldLogin {
        &self.metadata
    }
    pub fn sender(&self) -> Sender {
        self.sender.clone()
    }
    pub fn status(&self) -> Status {
        self.sender.status()
    }
    pub fn stats(&self) -> Stats {
        self.sender.stats()
    }
    /// Returns already queued authoritative frames before the terminal reason.
    pub async fn next_frame(&mut self) -> io::Result<Frame> {
        if let Some(observation) = self.observations.recv().await {
            return Ok(observation.frame);
        }
        // The read future drops its channel before its supervisor publishes
        // the terminal reason. Another runtime thread can observe that prefix.
        while self.status() == Status::Connected {
            if self.sender.status.changed().await.is_err() {
                break;
            }
        }
        Err(self.sender.closed_error())
    }
    pub async fn close(mut self) -> io::Result<()> {
        self.stop.send_replace(true);
        if let Some(task) = self.task.take() {
            task.await.map_err(io::Error::other)?;
        }
        match self.status() {
            Status::Closed(CloseReason::Requested) => Ok(()),
            Status::Closed(reason) => Err(reason.error()),
            Status::Connected => Err(self.sender.closed_error()),
        }
    }
}
impl Drop for ManagedSession {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}

async fn read_loop(
    reader: &mut Reader,
    mut decoder: Decoder,
    observations: mpsc::Sender<Observation>,
    commands: mpsc::Sender<Write>,
    mut writes: watch::Receiver<u64>,
    metrics: &Metrics,
) -> CloseReason {
    let budget = Arc::new(Semaphore::new(BYTE_LIMIT));
    let mut cycles = Cycles::default();
    let mut heartbeat_pending = false;
    let mut clock = interval_at(Instant::now() + CYCLE, CYCLE);
    // Stalls do not produce a burst of historical keepalive packets.
    clock.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        loop {
            let frame = match decoder.next_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(e) => return CloseReason::Protocol(e.to_string()),
            };
            cycles.received();
            let logout = frame.opcode == 86;
            let Ok(permit) = budget
                .clone()
                .try_acquire_many_owned((frame.payload.len() + 3) as u32)
            else {
                return CloseReason::ObservationOverflow;
            };
            if let Err(e) = observations.try_send(Observation {
                frame,
                _budget: permit,
            }) {
                return match e {
                    mpsc::error::TrySendError::Full(_) => CloseReason::ObservationOverflow,
                    mpsc::error::TrySendError::Closed(_) => CloseReason::Requested,
                };
            }
            metrics.frames.fetch_add(1, Ordering::Relaxed);
            if logout {
                return CloseReason::ServerLogout;
            }
        }
        tokio::select! {
            _ = clock.tick() => {
                if writes.has_changed().unwrap_or(false) {
                    writes.borrow_and_update();
                    cycles.written();
                    heartbeat_pending = false;
                }
                match cycles.tick() {
                    Err(reason) => return reason,
                    Ok(true) if !heartbeat_pending => {
                        match commands.try_send(Write { wire: vec![93], reply: None, _budget: None }) {
                            Ok(()) => heartbeat_pending = true,
                            Err(mpsc::error::TrySendError::Full(_)) => {},
                            Err(mpsc::error::TrySendError::Closed(_)) => return CloseReason::Requested,
                        }
                    }
                    _ => {},
                }
            }
            chunk = reader.read() => match chunk {
                Ok(Chunk::Binary(bytes)) => if let Err(e) = decoder.push(&bytes) {
                    return CloseReason::Protocol(e.to_string());
                },
                Ok(Chunk::Closed | Chunk::Text) => return CloseReason::PeerClosed,
                Err(e) => return CloseReason::Transport(e.to_string()),
            },
        }
    }
}

async fn write_loop<W: Output>(
    writer: &mut W,
    requests: &mut mpsc::Receiver<Write>,
    written: watch::Sender<u64>,
    metrics: &Metrics,
) -> CloseReason {
    while let Some(request) = requests.recv().await {
        match timeout(WRITE_TIMEOUT, writer.publish(request.wire.clone())).await {
            Ok(Ok(())) => {
                written.send_modify(|count| *count = count.wrapping_add(1));
                if let Some(reply) = request.reply {
                    metrics.commands.fetch_add(1, Ordering::Relaxed);
                    let _ = reply.send(Ok(request.wire));
                } else {
                    metrics.keepalives.fetch_add(1, Ordering::Relaxed);
                }
            }
            result => {
                let reason = match result {
                    Err(_) => CloseReason::WriteTimeout,
                    Ok(Err(e)) => CloseReason::Transport(e.to_string()),
                    Ok(Ok(())) => unreachable!(),
                };
                if let Some(reply) = request.reply {
                    let _ = reply.send(Err(reason.error()));
                }
                return reason;
            }
        }
    }
    CloseReason::Requested
}

#[cfg(test)]
mod tests;
