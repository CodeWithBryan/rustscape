//! Bounded byte transport. Native TCP runs off the render thread; browsers use WebSocket.
#[derive(Debug)]
pub enum Event {
    Connected,
    Data(Vec<u8>),
    Closed,
    Error(String),
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::Event;
    use std::{
        collections::VecDeque,
        io::{Read, Write},
        net::{SocketAddr, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, SyncSender, TrySendError},
        },
        thread,
        time::Duration,
    };
    const LIMIT: usize = 1024 * 1024;
    pub struct Transport {
        commands: SyncSender<Vec<u8>>,
        events: Receiver<Event>,
        stop: Arc<AtomicBool>,
    }
    impl Transport {
        pub fn connect(endpoint: &str) -> Result<Self, String> {
            let address: SocketAddr = endpoint
                .parse()
                .map_err(|_| "TCP endpoint must be IP:port")?;
            let (commands, receive) = mpsc::sync_channel::<Vec<u8>>(64);
            let (send, events) = mpsc::sync_channel(64);
            let stop = Arc::new(AtomicBool::new(false));
            let stopped = Arc::clone(&stop);
            thread::Builder::new()
                .name("game-tcp".into())
                .spawn(move || {
                    let result = run(address, receive, &send, &stopped);
                    let event = match result {
                        Ok(()) => Event::Closed,
                        Err(e) => Event::Error(e.to_string()),
                    };
                    let mut pending = event;
                    while !stopped.load(Ordering::Relaxed) {
                        match send.try_send(pending) {
                            Ok(()) | Err(TrySendError::Disconnected(_)) => break,
                            Err(TrySendError::Full(e)) => pending = e,
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                })
                .map_err(|e| e.to_string())?;
            Ok(Self {
                commands,
                events,
                stop,
            })
        }
        pub fn poll(&mut self) -> Option<Event> {
            self.events.try_recv().ok()
        }
        pub fn send(&mut self, bytes: Vec<u8>) -> Result<(), String> {
            if self.stop.load(Ordering::Relaxed) {
                return Err("transport closed".into());
            }
            if bytes.len() > 65538 {
                return Err("transport write exceeds packet limit".into());
            }
            self.commands
                .try_send(bytes)
                .map_err(|_| "transport closed or egress queue full".into())
        }
        pub fn close(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }
    impl Drop for Transport {
        fn drop(&mut self) {
            self.close();
        }
    }
    fn run(
        address: SocketAddr,
        commands: Receiver<Vec<u8>>,
        events: &SyncSender<Event>,
        stop: &AtomicBool,
    ) -> std::io::Result<()> {
        let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
        socket.set_nonblocking(true)?;
        socket.set_nodelay(true)?;
        let mut event = Some(Event::Connected);
        let mut writes = VecDeque::<Vec<u8>>::new();
        let mut position = 0;
        let mut queued = 0usize;
        let mut buffer = [0; 16384];
        while !stop.load(Ordering::Relaxed) {
            for bytes in commands.try_iter().take(64) {
                queued += bytes.len();
                if queued > LIMIT {
                    return Err(std::io::Error::other("TCP egress exceeds limit"));
                }
                if !bytes.is_empty() {
                    writes.push_back(bytes);
                }
            }
            while let Some(bytes) = writes.front() {
                match socket.write(&bytes[position..]) {
                    Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
                    Ok(n) => {
                        queued -= n;
                        position += n;
                        if position == bytes.len() {
                            writes.pop_front();
                            position = 0;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
            if let Some(pending) = event.take() {
                match events.try_send(pending) {
                    Ok(()) => {}
                    Err(TrySendError::Full(pending)) => event = Some(pending),
                    Err(TrySendError::Disconnected(_)) => return Ok(()),
                }
            }
            if event.is_none() {
                match socket.read(&mut buffer) {
                    Ok(0) => return Ok(()),
                    Ok(n) => event = Some(Event::Data(buffer[..n].to_vec())),
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(e),
                }
            }
            thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::Event;
    unsafe extern "C" {
        fn rs09_ws_open(ptr: *const u8, len: usize) -> i32;
        fn rs09_ws_status(handle: i32) -> i32;
        fn rs09_ws_read(handle: i32, ptr: *mut u8, capacity: usize) -> i32;
        fn rs09_ws_send(handle: i32, ptr: *const u8, len: usize) -> i32;
        fn rs09_ws_close(handle: i32);
    }
    pub struct Transport {
        handle: i32,
        connected: bool,
        ended: bool,
    }
    impl Transport {
        pub fn connect(endpoint: &str) -> Result<Self, String> {
            let handle = unsafe { rs09_ws_open(endpoint.as_ptr(), endpoint.len()) };
            if handle < 0 {
                return Err("invalid WebSocket endpoint".into());
            }
            Ok(Self {
                handle,
                connected: false,
                ended: false,
            })
        }
        pub fn poll(&mut self) -> Option<Event> {
            if self.ended {
                return None;
            }
            let status = unsafe { rs09_ws_status(self.handle) };
            if !self.connected && status == 1 {
                self.connected = true;
                return Some(Event::Connected);
            }
            let mut bytes = vec![0; 16384];
            let n = unsafe { rs09_ws_read(self.handle, bytes.as_mut_ptr(), bytes.len()) };
            if n > 0 {
                bytes.truncate(n as usize);
                return Some(Event::Data(bytes));
            }
            if status >= 2 {
                self.ended = true;
                return Some(if status == 2 {
                    Event::Closed
                } else {
                    Event::Error("WebSocket connection failed".into())
                });
            }
            None
        }
        pub fn send(&mut self, bytes: Vec<u8>) -> Result<(), String> {
            let status = unsafe { rs09_ws_send(self.handle, bytes.as_ptr(), bytes.len()) };
            if status == 0 {
                Ok(())
            } else {
                Err("WebSocket closed or egress queue full".into())
            }
        }
        pub fn close(&mut self) {
            unsafe {
                rs09_ws_close(self.handle);
            }
            self.ended = true;
        }
    }
    impl Drop for Transport {
        fn drop(&mut self) {
            self.close();
        }
    }
}
pub use platform::Transport;
