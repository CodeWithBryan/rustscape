//! Desktop TCP socket for [`crate::js5_client`]: `rt4.BufferedSocket` over a
//! standard-library stream, opened off the calling thread like
//! `SignLink.openSocket`.
use crate::js5_client::{Js5Socket, OpenStatus, SocketOpener, SocketRequest};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::Duration,
};

/// `BufferedSocket` sets a 30 second read timeout.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// `BufferedSocket.write` fails once this much is waiting to be sent.
const WRITE_BACKLOG: usize = 4900;
/// Received bytes held here until the queue asks for them.
const READ_AHEAD: usize = 64 * 1024;

pub struct TcpSocket {
    stream: TcpStream,
    incoming: VecDeque<u8>,
    outgoing: VecDeque<u8>,
    write_error: bool,
    end_of_stream: bool,
    closed: bool,
}
impl TcpSocket {
    pub fn new(stream: TcpStream) -> io::Result<Self> {
        stream.set_nodelay(true)?;
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            incoming: VecDeque::with_capacity(READ_AHEAD),
            outgoing: VecDeque::new(),
            write_error: false,
            end_of_stream: false,
            closed: false,
        })
    }
    /// Sends what the kernel accepts now. The Java does this on its own
    /// thread and reports a failure through the next `write`/`checkError`.
    fn flush(&mut self) {
        while !self.outgoing.is_empty() {
            let (front, _) = self.outgoing.as_slices();
            match self.stream.write(front) {
                Ok(0) => {
                    self.write_error = true;
                    return;
                }
                Ok(count) => {
                    self.outgoing.drain(..count);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => {
                    self.write_error = true;
                    self.outgoing.clear();
                    return;
                }
            }
        }
    }
    fn fill(&mut self) -> io::Result<()> {
        let mut buffer = [0u8; 16 * 1024];
        while !self.end_of_stream && self.incoming.len() < READ_AHEAD {
            let room = (READ_AHEAD - self.incoming.len()).min(buffer.len());
            match self.stream.read(&mut buffer[..room]) {
                // Like InputStream.available, the end of the stream is not an
                // error here: no more bytes ever become available.
                Ok(0) => self.end_of_stream = true,
                Ok(count) => self.incoming.extend(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}
impl Js5Socket for TcpSocket {
    fn check_error(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.flush();
        if std::mem::take(&mut self.write_error) {
            return Err(io::Error::other("update server write failed"));
        }
        Ok(())
    }
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        if std::mem::take(&mut self.write_error) {
            return Err(io::Error::other("update server write failed"));
        }
        if self.outgoing.len() + bytes.len() >= WRITE_BACKLOG {
            return Err(io::Error::other("update server write backlog"));
        }
        self.outgoing.extend(bytes);
        self.flush();
        Ok(())
    }
    fn available(&mut self) -> io::Result<usize> {
        if self.closed {
            return Ok(0);
        }
        self.flush();
        self.fill()?;
        Ok(self.incoming.len())
    }
    fn read(&mut self, destination: &mut [u8]) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        if self.incoming.len() < destination.len() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let count = destination.len();
        for (byte, value) in destination.iter_mut().zip(self.incoming.drain(..count)) {
            *byte = value;
        }
        Ok(())
    }
    fn read_blocking(&mut self) -> io::Result<i32> {
        if self.closed {
            return Ok(0);
        }
        if let Some(byte) = self.incoming.pop_front() {
            return Ok(i32::from(byte));
        }
        if self.end_of_stream {
            return Ok(-1);
        }
        // The handshake bytes have to be on their way before waiting.
        self.stream.set_nonblocking(false)?;
        self.stream.set_write_timeout(Some(READ_TIMEOUT))?;
        let pending: Vec<u8> = self.outgoing.drain(..).collect();
        self.stream.write_all(&pending)?;
        self.stream.set_read_timeout(Some(READ_TIMEOUT))?;
        let mut byte = [0u8];
        let result = loop {
            match self.stream.read(&mut byte) {
                Ok(0) => {
                    self.end_of_stream = true;
                    break Ok(-1);
                }
                Ok(_) => break Ok(i32::from(byte[0])),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => break Err(error),
            }
        };
        self.stream.set_nonblocking(true)?;
        result
    }
    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.flush();
            let _ = self.stream.shutdown(std::net::Shutdown::Both);
        }
    }
}

struct TcpRequest(Receiver<io::Result<TcpStream>>);
impl SocketRequest for TcpRequest {
    fn poll(&mut self) -> OpenStatus {
        match self.0.try_recv() {
            Ok(Ok(stream)) => match TcpSocket::new(stream) {
                Ok(socket) => OpenStatus::Ready(Box::new(socket)),
                Err(_) => OpenStatus::Failed,
            },
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => OpenStatus::Failed,
            Err(TryRecvError::Empty) => OpenStatus::Pending,
        }
    }
}

/// Opens connections to one host; the port comes from the connect logic.
pub struct TcpOpener {
    host: String,
    connect_timeout: Duration,
}
impl TcpOpener {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            connect_timeout: Duration::from_secs(10),
        }
    }
}
impl SocketOpener for TcpOpener {
    fn open(&mut self, port: u16) -> Box<dyn SocketRequest> {
        let (send, receive) = mpsc::channel();
        let host = self.host.clone();
        let timeout = self.connect_timeout;
        let spawned = thread::Builder::new()
            .name("js5-connect".into())
            .spawn(move || {
                let result = (host.as_str(), port)
                    .to_socket_addrs()
                    .and_then(|addresses| {
                        let mut last = io::Error::other("update server host has no address");
                        for address in addresses {
                            match TcpStream::connect_timeout(&address, timeout) {
                                Ok(stream) => return Ok(stream),
                                Err(error) => last = error,
                            }
                        }
                        Err(last)
                    });
                let _ = send.send(result);
            });
        if let Err(error) = spawned {
            let (send, receive) = mpsc::channel();
            let _ = send.send(Err(error));
            return Box::new(TcpRequest(receive));
        }
        Box::new(TcpRequest(receive))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// One loopback connection: a transport smoke test, not a protocol proof.
    #[test]
    fn a_loopback_socket_carries_bytes_both_ways_and_reports_the_end() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut peer, _) = listener.accept().unwrap();
            let mut handshake = [0u8; 5];
            peer.read_exact(&mut handshake).unwrap();
            peer.write_all(&[0]).unwrap();
            let mut request = [0u8; 4];
            peer.read_exact(&mut request).unwrap();
            peer.write_all(&[9, 8, 7]).unwrap();
            (handshake, request)
        });
        let mut request = TcpOpener::new("127.0.0.1").open(port);
        let mut socket = loop {
            match request.poll() {
                OpenStatus::Ready(socket) => break socket,
                OpenStatus::Failed => panic!("loopback connect failed"),
                OpenStatus::Pending => thread::sleep(Duration::from_millis(1)),
            }
        };
        socket.write(&[15, 0, 0, 2, 18]).unwrap();
        assert_eq!(socket.read_blocking().unwrap(), 0);
        socket.write(&[1, 255, 0, 255]).unwrap();
        let mut received = Vec::new();
        while received.len() < 3 {
            let available = socket.available().unwrap();
            let mut bytes = vec![0; available];
            socket.read(&mut bytes).unwrap();
            received.extend(bytes);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(received, [9, 8, 7]);
        assert_eq!(
            server.join().unwrap(),
            ([15, 0, 0, 2, 18], [1, 255, 0, 255])
        );
        // The peer is gone: nothing more is available, and that is not an error.
        for _ in 0..50 {
            assert_eq!(socket.available().unwrap(), 0);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(socket.read_blocking().unwrap(), -1);
        socket.check_error().unwrap();
        socket.close();
        assert_eq!(socket.available().unwrap(), 0);
    }

    #[test]
    fn a_refused_connection_is_a_failed_request() {
        // Bind then drop: the port is free again and nothing listens on it.
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let mut request = TcpOpener::new("127.0.0.1").open(port);
        loop {
            match request.poll() {
                OpenStatus::Failed => break,
                OpenStatus::Ready(_) => panic!("nothing should be listening"),
                OpenStatus::Pending => thread::sleep(Duration::from_millis(5)),
            }
        }
    }
}
