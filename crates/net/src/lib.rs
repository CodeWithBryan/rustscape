#![forbid(unsafe_code)]
pub mod js5_client;
pub mod js5_provider;
pub mod js5_stream;
#[cfg(not(target_arch = "wasm32"))]
pub mod js5_tcp;
#[cfg(not(target_arch = "wasm32"))]
pub mod login;
#[cfg(not(target_arch = "wasm32"))]
pub mod peer;
#[cfg(not(target_arch = "wasm32"))]
use bytes::{Bytes, BytesMut};
#[cfg(not(target_arch = "wasm32"))]
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
#[cfg(not(target_arch = "wasm32"))]
use std::{io, net::SocketAddr};
#[cfg(not(target_arch = "wasm32"))]
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};
#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        Message,
        protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
    },
};

#[cfg(not(target_arch = "wasm32"))]
type Ws = WebSocketStream<TcpStream>;
#[cfg(not(target_arch = "wasm32"))]
pub enum Reader {
    Tcp {
        socket: OwnedReadHalf,
        buffer: BytesMut,
    },
    WebSocket(Box<SplitStream<Ws>>),
}
#[cfg(not(target_arch = "wasm32"))]
pub enum Writer {
    Tcp(OwnedWriteHalf),
    WebSocket(Box<SplitSink<Ws, Message>>),
}
#[cfg(not(target_arch = "wasm32"))]
pub enum Chunk {
    Binary(Bytes),
    Text,
    Closed,
}

#[cfg(not(target_arch = "wasm32"))]
fn ws_config(max_message: usize) -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(max_message))
        .max_frame_size(Some(max_message))
}
#[cfg(not(target_arch = "wasm32"))]
pub async fn accept(
    socket: TcpStream,
    websocket: bool,
    max_message: usize,
) -> io::Result<(Reader, Writer)> {
    socket.set_nodelay(true)?;
    if websocket {
        let ws = tokio_tungstenite::accept_async_with_config(socket, Some(ws_config(max_message)))
            .await
            .map_err(io::Error::other)?;
        let (writer, reader) = ws.split();
        Ok((
            Reader::WebSocket(Box::new(reader)),
            Writer::WebSocket(Box::new(writer)),
        ))
    } else {
        let (reader, writer) = socket.into_split();
        Ok((
            Reader::Tcp {
                socket: reader,
                buffer: BytesMut::with_capacity(4096),
            },
            Writer::Tcp(writer),
        ))
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub async fn connect(
    address: SocketAddr,
    websocket: bool,
    max_message: usize,
) -> io::Result<(Reader, Writer)> {
    let socket = TcpStream::connect(address).await?;
    socket.set_nodelay(true)?;
    if websocket {
        let request = format!("ws://{address}/");
        let (ws, _) = tokio_tungstenite::client_async_with_config(
            request,
            socket,
            Some(ws_config(max_message)),
        )
        .await
        .map_err(io::Error::other)?;
        let (writer, reader) = ws.split();
        Ok((
            Reader::WebSocket(Box::new(reader)),
            Writer::WebSocket(Box::new(writer)),
        ))
    } else {
        let (reader, writer) = socket.into_split();
        Ok((
            Reader::Tcp {
                socket: reader,
                buffer: BytesMut::with_capacity(4096),
            },
            Writer::Tcp(writer),
        ))
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Reader {
    pub async fn read(&mut self) -> io::Result<Chunk> {
        match self {
            Self::Tcp { socket, buffer } => {
                buffer.reserve(4096);
                let count = socket.read_buf(buffer).await?;
                if count == 0 {
                    return Ok(Chunk::Closed);
                }
                Ok(Chunk::Binary(buffer.split().freeze()))
            }
            Self::WebSocket(socket) => loop {
                match socket.next().await {
                    Some(Ok(Message::Binary(bytes))) => return Ok(Chunk::Binary(bytes)),
                    Some(Ok(Message::Text(_))) => return Ok(Chunk::Text),
                    Some(Ok(Message::Close(_))) | None => return Ok(Chunk::Closed),
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => return Err(io::Error::other(error)),
                }
            },
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Writer {
    pub async fn write(&mut self, bytes: Bytes) -> io::Result<()> {
        match self {
            Self::Tcp(socket) => socket.write_all(&bytes).await,
            Self::WebSocket(socket) => socket
                .send(Message::Binary(bytes))
                .await
                .map_err(io::Error::other),
        }
    }
    pub async fn close(&mut self, code: u16, reason: &str) -> io::Result<()> {
        match self {
            Self::Tcp(socket) => socket.shutdown().await,
            Self::WebSocket(socket) => socket
                .send(Message::Close(Some(CloseFrame {
                    code: CloseCode::from(code),
                    reason: reason.to_owned().into(),
                })))
                .await
                .map_err(io::Error::other),
        }
    }
}
