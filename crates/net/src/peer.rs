//! Headless cache transport used by probes and future game-facing client state.
use crate::{Chunk, Reader, Writer};
use bytes::Bytes;
use rs09_protocol::js5::{self, Request, Response, ResponseDecoder};
use std::{collections::VecDeque, io, net::SocketAddr};

pub struct CachePeer {
    reader: Reader,
    writer: Writer,
    decoder: ResponseDecoder,
    ready: VecDeque<Response>,
    pending: Vec<Request>,
}
impl CachePeer {
    pub async fn connect(
        address: SocketAddr,
        websocket: bool,
        key: u8,
        max_payload: usize,
    ) -> io::Result<Self> {
        let (mut reader, mut writer) =
            crate::connect(address, websocket, max_payload.saturating_add(64 * 1024)).await?;
        writer
            .write(Bytes::copy_from_slice(&js5::handshake(530)))
            .await?;
        match reader.read().await? {
            Chunk::Binary(bytes) if bytes.as_ref() == [0] => {}
            Chunk::Binary(_) => {
                return Err(io::Error::other("cache handshake rejected or malformed"));
            }
            Chunk::Closed => return Err(io::ErrorKind::UnexpectedEof.into()),
            Chunk::Text => return Err(io::Error::other("binary cache protocol required")),
        }
        let mut setup = vec![6, 0, 0, 3, 3, 0, 0, 0];
        if key != 0 {
            setup.extend([4, key, 0, 0]);
        }
        writer.write(setup.into()).await?;
        Ok(Self {
            reader,
            writer,
            decoder: ResponseDecoder::new(key, max_payload),
            ready: VecDeque::new(),
            pending: Vec::with_capacity(40),
        })
    }
    pub async fn request(&mut self, request: Request) -> io::Result<()> {
        if self
            .pending
            .iter()
            .filter(|pending| pending.urgent == request.urgent)
            .count()
            >= 20
        {
            return Err(io::Error::other("cache request queue full"));
        }
        self.writer
            .write(Bytes::copy_from_slice(&request.encode()))
            .await?;
        self.pending.push(request);
        Ok(())
    }
    pub async fn next_response(&mut self) -> io::Result<Response> {
        loop {
            if let Some(response) = self.ready.pop_front() {
                return Ok(response);
            }
            match self.reader.read().await? {
                Chunk::Binary(bytes) => {
                    for response in self.decoder.feed(&bytes).map_err(io::Error::other)? {
                        let index = self
                            .pending
                            .iter()
                            .position(|request| *request == response.request)
                            .ok_or_else(|| {
                                io::Error::other(
                                    "cache reply does not match an outstanding request",
                                )
                            })?;
                        self.pending.remove(index);
                        self.ready.push_back(response);
                    }
                }
                Chunk::Closed => return Err(io::ErrorKind::UnexpectedEof.into()),
                Chunk::Text => return Err(io::Error::other("binary cache reply required")),
            }
        }
    }
    pub async fn fetch(&mut self, request: Request) -> io::Result<Vec<u8>> {
        self.request(request).await?;
        let response = self.next_response().await?;
        if response.request != request {
            return Err(io::Error::other("unexpected cache reply"));
        }
        Ok(response.container)
    }
    pub async fn close(&mut self) -> io::Result<()> {
        self.writer.write(Bytes::from_static(&[7, 0, 0, 0])).await?;
        self.writer.close(1000, "done").await
    }
}
