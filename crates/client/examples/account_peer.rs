//! The scripted account-creation peer of `tests/account_creation.rs` as a
//! program, for clicking account creation through in a running client (this
//! one or the original): it answers the create requests (147, 186, 36) and
//! relays everything else (JS5, world list, login) to a real server.
//!
//! `cargo run -p rs09-client --example account_peer -- --listen 127.0.0.1:P
//! [--forward 127.0.0.1:Q] [--script FILE] [--taken NAME,NAME]`
//!
//! FILE holds lines `OPCODE ANSWER` (`#` starts a comment). The n-th request
//! with an opcode takes the n-th line for that opcode; the file is read again
//! for every request, so lines can be appended while the peer runs. ANSWER is
//! a reply code, `close` (no reply, end of stream), `silent` (no reply, the
//! connection stays open), `reference` (the reference server's rules, also
//! what a request without a line gets) or `suggest:a,b,...` (21 and those
//! names). Every request and answer is printed on standard output.
#[cfg(not(target_arch = "wasm32"))]
#[path = "../tests/support/account_peer.rs"]
mod account_peer;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use account_peer::{Answer, Config, Registry, Request};
    use std::{collections::BTreeMap, io::Write, net::TcpListener};

    let mut listen = None;
    let mut forward = None;
    let mut script = None;
    let mut registry = Registry::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args.next().expect("a value after each option");
        match arg.as_str() {
            "--listen" => listen = Some(value),
            "--forward" => forward = Some(value.parse().expect("--forward IP:PORT")),
            "--script" => script = Some(value),
            "--taken" => registry
                .accounts
                .extend(value.split(',').map(account_peer::normalise)),
            other => panic!("unknown option {other}"),
        }
    }
    let listen = listen.expect("--listen IP:PORT");
    let mut counts = BTreeMap::<i32, usize>::new();
    let answer = move |request: &Request, registry: &mut Registry| {
        let opcode = match request {
            Request::Info { .. } => 147,
            Request::Name(_) => 186,
            Request::Create { .. } => 36,
            Request::Login { .. } => 14,
            Request::WorldList(_) => 255,
            Request::Forwarded(opcode) => i32::from(*opcode),
        };
        let index = *counts.entry(opcode).and_modify(|n| *n += 1).or_insert(0);
        let line = script.as_ref().and_then(|path| {
            std::fs::read_to_string(path)
                .unwrap_or_default()
                .lines()
                .map(|line| line.split('#').next().unwrap_or("").trim().to_owned())
                .filter(|line| line.split_whitespace().next() == Some(&opcode.to_string()))
                .nth(index)
        });
        let answer = match line.as_deref().and_then(|l| l.split_whitespace().nth(1)) {
            None | Some("reference") if opcode == 14 || opcode == 255 => Answer::Reference,
            None | Some("reference") => Answer::Code(registry.reference(request)),
            Some("close") => Answer::Close,
            Some("silent") => Answer::Silent,
            Some(other) => match other.strip_prefix("suggest:") {
                Some(names) => Answer::Suggest(
                    names
                        .split(',')
                        .filter(|n| !n.is_empty())
                        .map(str::to_owned)
                        .collect(),
                ),
                None => Answer::Code(other.parse().expect("a reply code")),
            },
        };
        println!("{request:?} #{index} -> {answer:?}");
        let _ = std::io::stdout().flush();
        answer
    };
    let peer = account_peer::start(
        TcpListener::bind(&listen).expect("--listen address"),
        Config {
            forward,
            answer: Box::new(answer),
            registry,
        },
    );
    println!("PEER={}", peer.address);
    let _ = std::io::stdout().flush();
    for request in peer.seen {
        if let Request::Forwarded(opcode) = request {
            println!("relayed a connection opening with {opcode}");
            let _ = std::io::stdout().flush();
        }
    }
}
