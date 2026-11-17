//! PacketProcessor's two shared buffers. Producers never hold the lock while a
//! handler runs; nested processing observes the same buffers as the original.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub enum Packet<P> {
    Dispatch(P),
    NoProcess,
}
struct Buffers<P> {
    queued: VecDeque<Packet<P>>,
    processing: VecDeque<Packet<P>>,
}
pub struct Queue<P>(Arc<Mutex<Buffers<P>>>);
impl<P> Clone for Queue<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<P> Default for Queue<P> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Buffers {
            queued: VecDeque::new(),
            processing: VecDeque::new(),
        })))
    }
}
pub trait Context<P> {
    type Error;
    fn process(&mut self, packet: &P) -> Result<(), Self::Error>;
    /// The host supplies the exception text (JVM stack traces are test services).
    fn exception_text(&mut self, error: Self::Error) -> Result<String, Self::Error>;
    fn log(&mut self, packet: &P, accumulated_trace: &str) -> Result<(), Self::Error>;
}
impl<P> Queue<P> {
    pub fn enqueue(&self, packet: Packet<P>) {
        self.0
            .lock()
            .expect("packet queue poisoned")
            .queued
            .push_back(packet);
    }
    pub fn lengths(&self) -> (usize, usize) {
        let b = self.0.lock().expect("packet queue poisoned");
        (b.queued.len(), b.processing.len())
    }
    pub fn process<C: Context<P>>(&self, ctx: &mut C) -> Result<(), C::Error> {
        {
            let mut b = self.0.lock().expect("packet queue poisoned");
            if b.queued.is_empty() {
                return Ok(());
            }
            let Buffers { queued, processing } = &mut *b;
            std::mem::swap(queued, processing);
        }
        // StringWriter is allocated once per invocation, not once per failure.
        let mut trace = String::new();
        loop {
            let packet = self
                .0
                .lock()
                .expect("packet queue poisoned")
                .processing
                .pop_front();
            match packet {
                None => return Ok(()),
                Some(Packet::NoProcess) => {
                    let mut b = self.0.lock().expect("packet queue poisoned");
                    b.queued.clear();
                    b.processing.clear();
                    return Ok(());
                }
                Some(Packet::Dispatch(packet)) => {
                    if let Err(error) = ctx.process(&packet) {
                        trace.push_str(&ctx.exception_text(error)?);
                        ctx.log(&packet, &trace)?;
                    }
                }
            }
        }
    }
}
impl<P: Clone> Queue<P> {
    pub fn snapshot(&self) -> [Vec<Packet<P>>; 2] {
        let b = self.0.lock().expect("packet queue poisoned");
        [
            b.queued.iter().cloned().collect(),
            b.processing.iter().cloned().collect(),
        ]
    }
}
