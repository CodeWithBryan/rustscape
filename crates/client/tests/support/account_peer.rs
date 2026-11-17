//! A scripted account-creation server for tests and captures (not the game
//! server): it speaks the handshake opcodes the title screen's
//! `CreateManager` sends, as the reference server reads and answers them
//! (`core/net/event/HSReadEvent.java`, `core/net/registry/AccountRegister.java`
//! and `RegistryResponse.java`):
//!
//! - 147, the date-of-birth check: `p1 day, p1 month, p2 year, p2 country`;
//! - 186, the name check: the name as a zero-terminated string;
//! - 36, the creation: `p1 size` and the RSA block (`p1 length` and the
//!   encrypted bytes) holding `10, p2 noise, p2 revision, name string,
//!   p4 noise, password string, p4 noise, p2 affiliate, p1 day, p1 month,
//!   p4 noise, p2 year, p2 country, p4 noise`.
//!
//! Each is answered with one reply byte (and, for 21, a count and that many
//! base-37 names, which `CreateManager.loop` reads; the reference server never
//! sends 21). Which answer is given is up to the `answer` function: the
//! reference server's own rules ([`Registry::reference`]), a fixed code, a
//! closed connection or none at all.
//!
//! Any other first byte (JS5 15, world list 255, login 14) is either relayed
//! byte for byte to a real server (`forward`), or answered here: the world
//! list with the shared encoder, a login with the shared decoder and a scripted
//! reply (2 with player details and a map rebuild, as `tests/login_seam.rs`
//! sends it, or a refusal code).
#![allow(dead_code)]
use rs09_protocol::{
    buffer::Reader,
    lobby,
    login::{self, RsaKey},
};
use std::{
    collections::BTreeSet,
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

/// `RegistryResponse` ids.
pub const SUCCESS: u8 = 2;
pub const CONTACT_ERROR: u8 = 3;
pub const SERVER_BUSY: u8 = 7;
pub const CANNOT_CREATE: u8 = 9;
pub const INVALID_BIRTH: u8 = 10;
pub const FUTURE_BIRTH: u8 = 11;
pub const BIRTH_THIS_YEAR: u8 = 12;
pub const BIRTH_LAST_YEAR: u8 = 13;
pub const INVALID_COUNTRY: u8 = 14;
pub const NOT_AVAILABLE_USER: u8 = 20;
pub const INVALID_USERNAME: u8 = 22;
pub const INVALID_PASS_LENGTH: u8 = 30;
pub const INVALID_PASS: u8 = 31;
pub const WEAK_PASS: u8 = 32;
pub const PASS_SIMILAR_TO_USER: u8 = 33;

/// One request as the peer read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// 147.
    Info {
        day: i32,
        month: i32,
        year: i32,
        country: i32,
    },
    /// 186: the name as `ByteBufferUtils.getString` reads it.
    Name(String),
    /// 36, decrypted with the reference private key. `header` is the first
    /// byte of the block (10 when it decrypted properly).
    Create {
        header: i32,
        revision: i32,
        name: String,
        password: String,
        affiliate: i32,
        day: i32,
        month: i32,
        year: i32,
        country: i32,
    },
    /// 255 and the client's update stamp.
    WorldList(i32),
    /// 14: a login, with the name and password of its block.
    Login {
        opcode: i32,
        username: String,
        password: String,
    },
    /// A connection relayed to the forward server, by its first byte.
    Forwarded(u8),
}

/// What the peer does with a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// This reply byte; for a login, 2 accepts (details and a rebuild follow)
    /// and anything else refuses.
    Code(u8),
    /// 21, then the names (as base-37 longs).
    Suggest(Vec<String>),
    /// What the reference server answers.
    Reference,
    /// Close the connection without a reply (the client reads end of stream).
    Close,
    /// Keep the connection open and never answer.
    Silent,
}

/// The reference server's account store and rules.
#[derive(Default)]
pub struct Registry {
    /// Names that exist (`checkUsernameTaken`), lower case with underscores.
    pub accounts: BTreeSet<String>,
}

/// `name.replace(" ", "_").toLowerCase().replace("|", "")`.
pub fn normalise(name: &str) -> String {
    name.replace(' ', "_").to_lowercase().replace('|', "")
}

/// `[a-z0-9_]{1,12}` does not match.
pub fn invalid_username(name: &str) -> bool {
    let count = name.chars().count();
    !(1..=12).contains(&count)
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl Registry {
    /// `AccountRegister.read`'s reply, creating the account on success.
    pub fn reference(&mut self, request: &Request) -> u8 {
        match request {
            Request::Info { .. } => SUCCESS,
            Request::Name(name) => {
                let name = normalise(name);
                let length = name.chars().count();
                if length == 0 || length > 12 || invalid_username(&name) {
                    INVALID_USERNAME
                } else if self.accounts.contains(&name) {
                    NOT_AVAILABLE_USER
                } else {
                    SUCCESS
                }
            }
            Request::Create {
                header,
                revision,
                name,
                password,
                ..
            } => {
                if *header != 10 || *revision != 530 {
                    return CANNOT_CREATE;
                }
                let name = normalise(name);
                let length = password.chars().count();
                if !(5..=20).contains(&length) {
                    INVALID_PASS_LENGTH
                } else if *password == name {
                    PASS_SIMILAR_TO_USER
                } else if invalid_username(&name) {
                    INVALID_USERNAME
                } else if self.accounts.contains(&name) {
                    CANNOT_CREATE
                } else {
                    self.accounts.insert(name);
                    SUCCESS
                }
            }
            _ => SUCCESS,
        }
    }
}

/// The function that picks each answer, given the registry.
pub type AnswerFn = dyn FnMut(&Request, &mut Registry) -> Answer + Send;

pub struct Config {
    /// Relay every connection that is not an account request to this server.
    pub forward: Option<SocketAddr>,
    pub answer: Box<AnswerFn>,
    pub registry: Registry,
}

/// A running peer: what it saw, in order.
pub struct Peer {
    pub address: SocketAddr,
    pub seen: mpsc::Receiver<Request>,
}

/// Starts the peer on `listener`; it runs until the process ends.
pub fn start(listener: TcpListener, config: Config) -> Peer {
    let address = listener.local_addr().unwrap();
    let (seen, reports) = mpsc::channel();
    let shared = Arc::new(Mutex::new(config));
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let shared = Arc::clone(&shared);
            let seen = seen.clone();
            thread::spawn(move || {
                if let Err(error) = serve(stream, &shared, &seen) {
                    eprintln!("account peer: {error}");
                }
            });
        }
    });
    Peer {
        address,
        seen: reports,
    }
}

fn read_string(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        if byte[0] == 0 {
            return Ok(bytes);
        }
        bytes.push(byte[0]);
    }
}

fn read_exact(stream: &mut TcpStream, count: usize) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; count];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn invalid(error: rs09_protocol::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}"))
}

/// The 36 request's block, read in `AccountRegister.read` order.
fn decode_create(block: &[u8]) -> Result<Request, rs09_protocol::Error> {
    let mut reader = Reader::new(block);
    reader.g1()?; // the size the reference server skips
    let secret = RsaKey::reference_private().unframe(&mut reader)?;
    let mut r = Reader::new(&secret);
    let header = r.g1b()?;
    if header != 10 {
        return Ok(Request::Create {
            header,
            revision: 0,
            name: String::new(),
            password: String::new(),
            affiliate: 0,
            day: 0,
            month: 0,
            year: 0,
            country: 0,
        });
    }
    r.g2()?;
    let revision = r.g2b()?;
    let name = login::server_string(r.gjstr()?);
    r.g4()?;
    let password = login::server_string(r.gjstr()?);
    r.g4()?;
    let affiliate = r.g2b()?;
    let day = r.g1b()?;
    let month = r.g1b()?;
    r.g4()?;
    let year = r.g2b()?;
    let country = r.g2b()?;
    r.g4()?;
    Ok(Request::Create {
        header,
        revision,
        name,
        password,
        affiliate,
        day,
        month,
        year,
        country,
    })
}

fn serve(
    mut stream: TcpStream,
    shared: &Mutex<Config>,
    seen: &mpsc::Sender<Request>,
) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let mut first = [0];
    stream.read_exact(&mut first)?;
    let forward = shared.lock().unwrap().forward;
    let request = match first[0] {
        147 => {
            let bytes = read_exact(&mut stream, 6)?;
            let mut r = Reader::new(&bytes);
            Request::Info {
                day: r.g1b().map_err(invalid)?,
                month: r.g1b().map_err(invalid)?,
                year: r.g2b().map_err(invalid)?,
                country: r.g2b().map_err(invalid)?,
            }
        }
        186 => Request::Name(login::server_string(&read_string(&mut stream)?)),
        36 => {
            let size = read_exact(&mut stream, 1)?[0];
            let mut block = vec![size];
            block.extend(read_exact(&mut stream, usize::from(size))?);
            decode_create(&block).map_err(invalid)?
        }
        opcode => {
            if let Some(address) = forward {
                let _ = seen.send(Request::Forwarded(opcode));
                return relay(stream, opcode, address);
            }
            return match opcode {
                255 => world_list(stream, seen),
                14 => login(stream, shared, seen),
                _ => Ok(()),
            };
        }
    };
    let answer = {
        let mut config = shared.lock().unwrap();
        let Config {
            answer, registry, ..
        } = &mut *config;
        let answer = answer(&request, registry);
        if answer == Answer::Reference {
            Answer::Code(registry.reference(&request))
        } else {
            answer
        }
    };
    let _ = seen.send(request);
    reply(stream, answer)
}

fn reply(mut stream: TcpStream, answer: Answer) -> io::Result<()> {
    match answer {
        Answer::Code(code) => stream.write_all(&[code])?,
        Answer::Suggest(names) => {
            let mut bytes = vec![21, names.len() as u8];
            for name in &names {
                bytes.extend_from_slice(&login::encode_name(name.as_bytes()).to_be_bytes());
            }
            stream.write_all(&bytes)?;
        }
        Answer::Close | Answer::Reference => {
            stream.shutdown(Shutdown::Both)?;
            return Ok(());
        }
        Answer::Silent => {}
    }
    // The reference server leaves the connection open; the client closes it.
    let mut rest = [0; 64];
    stream.set_read_timeout(Some(Duration::from_secs(120)))?;
    while matches!(stream.read(&mut rest), Ok(n) if n > 0) {}
    Ok(())
}

fn relay(mut client: TcpStream, first: u8, address: SocketAddr) -> io::Result<()> {
    let mut server = TcpStream::connect(address)?;
    server.set_nodelay(true)?;
    server.write_all(&[first])?;
    let (mut client_in, mut server_out) = (client.try_clone()?, server.try_clone()?);
    let upstream = thread::spawn(move || {
        let _ = io::copy(&mut client_in, &mut server_out);
        let _ = server_out.shutdown(Shutdown::Write);
    });
    let _ = io::copy(&mut server, &mut client);
    let _ = client.shutdown(Shutdown::Both);
    let _ = upstream.join();
    Ok(())
}

fn world_list(mut stream: TcpStream, seen: &mpsc::Sender<Request>) -> io::Result<()> {
    let stamp = i32::from_be_bytes(read_exact(&mut stream, 4)?.try_into().unwrap());
    let _ = seen.send(Request::WorldList(stamp));
    let worlds = [lobby::World::default()];
    stream.write_all(&lobby::world_list(&worlds, 1, stamp).map_err(invalid)?)?;
    Ok(())
}

fn login(
    mut stream: TcpStream,
    shared: &Mutex<Config>,
    seen: &mpsc::Sender<Request>,
) -> io::Result<()> {
    read_exact(&mut stream, 1)?; // the name hash
    let mut reply = vec![0];
    reply.extend_from_slice(&0x0102_0304_0506_0708i64.to_be_bytes());
    stream.write_all(&reply)?;
    let header = read_exact(&mut stream, 3)?;
    let mut block = header.clone();
    block.extend(read_exact(
        &mut stream,
        usize::from(u16::from_be_bytes([header[1], header[2]])),
    )?);
    let outcome = login::decode(&block, &RsaKey::reference_private(), |_| false);
    let info = outcome.info.unwrap_or_default();
    let request = Request::Login {
        opcode: info.opcode,
        username: info.username,
        password: info.password,
    };
    let answer = {
        let mut config = shared.lock().unwrap();
        let Config {
            answer, registry, ..
        } = &mut *config;
        answer(&request, registry)
    };
    let _ = seen.send(request);
    match answer {
        Answer::Code(2) | Answer::Reference => {
            // staff 2, blackmarks 0, underage, chat consent, ad consent,
            // quick chat, mouse recorder, index 300, member, members' map;
            // then REBUILD_NORMAL with a three-byte payload.
            let mut bytes = vec![2, 2, 0, 0, 0, 0, 0, 1, 1, 44, 1, 1];
            bytes.extend_from_slice(&[162, 0, 3, 9, 9, 9]);
            stream.write_all(&bytes)?;
            let mut rest = [0; 256];
            stream.set_read_timeout(Some(Duration::from_secs(120)))?;
            while matches!(stream.read(&mut rest), Ok(n) if n > 0) {}
        }
        Answer::Code(code) => stream.write_all(&[code])?,
        Answer::Suggest(_) | Answer::Close => stream.shutdown(Shutdown::Both)?,
        Answer::Silent => {
            let mut rest = [0; 256];
            while matches!(stream.read(&mut rest), Ok(n) if n > 0) {}
        }
    }
    Ok(())
}
