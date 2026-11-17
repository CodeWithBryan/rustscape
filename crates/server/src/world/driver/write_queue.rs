//! `PacketWriteQueue.kt`: outgoing packets that are written at once, and the
//! two lists of queued packets swapped and written at the end of the tick.
//!
//! The packet itself (an `OutgoingPacket` with its context) is the caller's
//! type `O`; writing it (`OutgoingPacket.send(context)`) is a service of
//! [`Host`]. A clone of a [`WriteQueue`] is the same queue, as the original's
//! lists are one pair per world.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

/// The outgoing packet classes `handle` sends at once instead of queueing:
/// `UpdateSceneGraph`, `BuildDynamicScene`, `InstancedLocationUpdate`,
/// `Logout` and `ClearRegionChunk`, by their simple class names.
pub const IMMEDIATE: [&str; 5] = [
    "UpdateSceneGraph",
    "BuildDynamicScene",
    "InstancedLocationUpdate",
    "Logout",
    "ClearRegionChunk",
];

/// One `QueuedPacket`: the packet's simple class name and the packet with
/// its context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued<O> {
    pub class: String,
    pub packet: O,
}

/// What writing needs from the world.
pub trait Host<O> {
    type Error;
    /// `OutgoingPacket.send(context)` (through `write`'s casts, which
    /// cannot fail for a packet queued with its own context).
    fn send(&mut self, packet: &O) -> Result<(), Self::Error>;
    /// Whether `catch (e: Exception)` stops this error.
    fn is_exception(&mut self, error: &Self::Error) -> bool;
    /// `e.printStackTrace(pw)`: the text the error adds to the writer.
    fn stack_trace(&mut self, error: &Self::Error) -> String;
    /// `log(PacketWriteQueue::class.java, Log.ERR, message)`.
    fn log_error(&mut self, message: &str);
}

struct Lists<O> {
    to_write: VecDeque<Queued<O>>,
    to_flush: VecDeque<Queued<O>>,
}

/// `PacketWriteQueue`'s companion: `packetsToWrite` and `packetsToFlush`
/// under `queueLock`.
pub struct WriteQueue<O>(Arc<Mutex<Lists<O>>>);

impl<O> Clone for WriteQueue<O> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<O> Default for WriteQueue<O> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Lists {
            to_write: VecDeque::new(),
            to_flush: VecDeque::new(),
        })))
    }
}

impl<O: Clone> WriteQueue<O> {
    fn lists(&self) -> MutexGuard<'_, Lists<O>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether two handles are the same queue.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// `handle(packet, context)`: the five immediate classes are sent now
    /// (their failure goes to the caller, as in the original); everything
    /// else is pushed. Returns whether the packet was sent at once.
    pub fn handle<H: Host<O>>(
        &self,
        host: &mut H,
        class: &str,
        packet: Option<O>,
    ) -> Result<bool, Option<H::Error>> {
        if IMMEDIATE.contains(&class) {
            // `packet.send(context)` with a null context fails inside the
            // writer; that is the caller's NullPointerException.
            let packet = packet.ok_or(None)?;
            host.send(&packet).map_err(Some)?;
            return Ok(true);
        }
        self.push(host, class, packet);
        Ok(false)
    }

    /// `push(packet, context)`: a null context is logged and dropped.
    pub fn push<H: Host<O>>(&self, host: &mut H, class: &str, packet: Option<O>) {
        let Some(packet) = packet else {
            host.log_error(&format!("{class} tried to queue with a null context!"));
            return;
        };
        self.lists().to_write.push_back(Queued {
            class: class.to_owned(),
            packet,
        });
    }

    /// `flush()`: nothing if nothing is queued; otherwise the lists are
    /// swapped under the lock and every queued packet is written in order
    /// outside it. A failing write is printed into one writer shared by the
    /// whole flush and logged with everything printed so far, and the flush
    /// goes on. Packets pushed while flushing wait for the next flush. An
    /// error `catch (Exception)` does not stop leaves the flush with the
    /// rest of the list still queued, as the original's would.
    pub fn flush<H: Host<O>>(&self, host: &mut H) -> Result<(), H::Error> {
        {
            let mut lists = self.lists();
            if lists.to_write.is_empty() {
                return Ok(());
            }
            let Lists { to_write, to_flush } = &mut *lists;
            std::mem::swap(to_write, to_flush);
        }
        let mut trace = String::new();
        loop {
            let next = self.lists().to_flush.pop_front();
            let Some(queued) = next else {
                return Ok(());
            };
            if let Err(error) = host.send(&queued.packet) {
                if !host.is_exception(&error) {
                    return Err(error);
                }
                trace.push_str(&host.stack_trace(&error));
                host.log_error(&format!(
                    "Error flushing packet class core.net.packet.out.{}: {trace}",
                    queued.class
                ));
            }
        }
    }

    /// The queued packets: `packetsToWrite`, then `packetsToFlush`.
    pub fn snapshot(&self) -> [Vec<Queued<O>>; 2] {
        let lists = self.lists();
        [
            lists.to_write.iter().cloned().collect(),
            lists.to_flush.iter().cloned().collect(),
        ]
    }
}
