use rs09_client::session::{Config, Event, Session};
use rs09_protocol::{
    game::{Direction, Frame},
    isaac::OpcodeCipher,
    login::{LoginRequest, RsaKey},
    profile::RS09_REVISION_530,
};
fn start(isaac: bool) -> (Session, [i32; 4]) {
    let mut c = Config::new(
        b"transporttest".to_vec(),
        b"fixturepassword".to_vec(),
        [123, 456],
    );
    c.profile.use_isaac = isaac;
    let mut s = Session::new(c).unwrap();
    assert_eq!(
        s.take_output().unwrap(),
        rs09_protocol::login::handshake(b"transporttest")
    );
    let seed: [i32; 4] = [123, 456, -1234567, 7654321];
    let mut challenge = vec![0];
    challenge.extend_from_slice(&seed[2].to_be_bytes());
    challenge.extend_from_slice(&seed[3].to_be_bytes());
    for b in challenge {
        assert!(s.feed(&[b]).unwrap().is_empty());
    }
    let expected = LoginRequest {
        username: b"transporttest",
        password: b"fixturepassword",
        isaac_seed: seed,
        window_mode: 2,
        ..Default::default()
    };
    let mut profile = RS09_REVISION_530;
    profile.use_isaac = isaac;
    assert_eq!(
        s.take_output().unwrap(),
        expected
            .encode(profile, &RsaKey::reference_public())
            .unwrap()
    );
    (s, seed)
}
fn success(seed: [i32; 4], isaac: bool) -> (Vec<u8>, Vec<Frame>) {
    let frames = vec![
        Frame {
            opcode: 162,
            payload: vec![4; 25],
        },
        Frame {
            opcode: 225,
            payload: vec![1, 2, 3],
        },
        Frame {
            opcode: 86,
            payload: vec![],
        },
    ];
    let mut wire = vec![2, 2, 3, 1, 0, 1, 0, 1, 0, 7, 1, 0];
    let mut cipher = OpcodeCipher::new(&seed.map(|n| n.wrapping_add(50) as u32), isaac).unwrap();
    for f in &frames {
        f.write(Direction::ServerToClient, Some(&mut cipher), &mut wire)
            .unwrap();
    }
    (wire, frames)
}
#[test]
fn success_fragmentation_and_coalescing_preserve_original_first_rebuild_gate_and_cipher() {
    for isaac in [false, true] {
        for split in 0..50 {
            let (mut s, seed) = start(isaac);
            let (wire, frames) = success(seed, isaac);
            let split = split.min(wire.len());
            let mut events = s.feed(&wire[..split]).unwrap();
            if split < 40 {
                assert!(!s.in_game());
            }
            events.extend(s.feed(&wire[split..]).unwrap());
            let Event::World(world) = events.remove(0) else {
                panic!("world must precede frames")
            };
            assert_eq!(world.player_index, 7);
            assert_eq!(world.staff_level, 2);
            assert!(world.underage);
            assert!(!world.parental_chat_consent);
            assert_eq!(
                events,
                frames.into_iter().map(Event::Frame).collect::<Vec<_>>()
            );
            assert!(s.in_game());
            s.send(&Frame {
                opcode: 93,
                payload: vec![],
            })
            .unwrap();
            let mut outgoing = OpcodeCipher::new(&seed.map(|n| n as u32), isaac).unwrap();
            assert_eq!(s.take_output(), Some(vec![outgoing.encode(93)]));
        }
    }
}
#[test]
fn confirmation_rejections_timeouts_and_keepalives() {
    let (mut s, seed) = start(true);
    assert_eq!(s.feed(&[1]).unwrap(), vec![Event::ConfirmationRequired]);
    for _ in 0..2100 {
        assert_eq!(s.tick().unwrap(), None);
    }
    s.confirm().unwrap();
    let mut cipher = OpcodeCipher::new(&seed.map(|n| n as u32), true).unwrap();
    assert_eq!(s.take_output(), Some(vec![cipher.encode(17)]));
    assert!(s.feed(&[29]).unwrap().is_empty());
    assert_eq!(
        s.feed(&[64]).unwrap(),
        vec![Event::Rejected {
            code: 29,
            detail: Some(64)
        }]
    );
    assert!(s.closed());
    let (mut s, _) = start(false);
    for _ in 0..2000 {
        assert_eq!(s.tick().unwrap(), None);
    }
    assert_eq!(s.tick().unwrap(), Some(Event::Timeout));
    let (mut s, seed) = start(false);
    let (wire, _) = success(seed, false);
    s.feed(&wire).unwrap();
    for _ in 0..50 {
        s.tick().unwrap();
    }
    assert!(s.take_output().is_none());
    s.tick().unwrap();
    assert_eq!(s.take_output(), Some(vec![93]));
    for _ in 51..750 {
        assert_eq!(s.tick().unwrap(), None);
    }
    assert_eq!(s.tick().unwrap(), Some(Event::Timeout));
}
/// Transcription test of the end of `Protocol.loop` (Protocol.java:2573-2577,
/// 2920, 2976-2988), traced by hand; the original was not run.
#[test]
fn a_cycle_writes_its_frames_then_the_keep_alive_then_the_verify_id() {
    fn written(s: &mut Session) -> Vec<u8> {
        let mut bytes = vec![];
        while let Some(chunk) = s.take_output() {
            bytes.extend(chunk);
        }
        bytes
    }
    let click = Frame {
        opcode: 75,
        payload: vec![1, 2, 3, 4, 5, 6],
    };
    let verify = Frame {
        opcode: 177,
        payload: vec![0, 9],
    };
    let (mut s, seed) = start(false);
    let (wire, _) = success(seed, false);
    s.feed(&wire).unwrap();
    // A cycle with something to write: no keep-alive, the id last.
    assert_eq!(
        s.cycle(std::slice::from_ref(&click), Some(&verify))
            .unwrap(),
        None
    );
    assert_eq!(written(&mut s), [75, 1, 2, 3, 4, 5, 6, 177, 0, 9]);
    // Fifty cycles with nothing to write stay silent (`ticksSinceWrite > 50`).
    for _ in 0..50 {
        assert_eq!(s.cycle(&[], None).unwrap(), None);
        assert!(s.take_output().is_none());
    }
    // The 51st adds the keep-alive after what the cycle wrote itself and
    // before the id, although the cycle is not silent.
    s.cycle(std::slice::from_ref(&click), Some(&verify))
        .unwrap();
    assert_eq!(written(&mut s), [75, 1, 2, 3, 4, 5, 6, 93, 177, 0, 9]);
    // That write restarted the count.
    for _ in 0..50 {
        s.cycle(&[], None).unwrap();
    }
    assert!(s.take_output().is_none());
    s.cycle(&[], None).unwrap();
    assert_eq!(written(&mut s), [93]);
    // 103 cycles so far without a server packet: the 751st closes the
    // session before writing anything.
    for _ in 103..750 {
        assert_eq!(s.cycle(&[], None).unwrap(), None);
    }
    written(&mut s);
    assert_eq!(
        s.cycle(std::slice::from_ref(&click), Some(&verify))
            .unwrap(),
        Some(Event::Timeout)
    );
    assert!(s.closed() && s.take_output().is_none());

    // Before the game the frames are dropped and the login timeout runs.
    let (mut s, _) = start(false);
    for _ in 0..2000 {
        assert_eq!(s.cycle(std::slice::from_ref(&click), None).unwrap(), None);
        assert!(s.take_output().is_none());
    }
    assert_eq!(s.cycle(&[], None).unwrap(), Some(Event::Timeout));
}
/// Transcription test of game state 25 as the connection sees it
/// (client.java:1725-1734, LoginManager.java:886, ClientProt.java:199-219),
/// traced by hand; the original was not run.
#[test]
fn a_ping_writes_the_waiting_buffer_and_does_not_count_towards_the_read_timeout() {
    fn written(s: &mut Session) -> Vec<u8> {
        let mut bytes = vec![];
        while let Some(chunk) = s.take_output() {
            bytes.extend(chunk);
        }
        bytes
    }
    let status = Frame {
        opcode: 243,
        payload: vec![2, 3, 0, 2, 0xf7, 1],
    };
    let (mut s, seed) = start(false);
    let (wire, _) = success(seed, false);
    s.feed(&wire).unwrap();
    // `ping(false)`: 49 cycles return at `ticksSinceWrite < 50` and leave
    // the buffer waiting; the 50th writes it with the keep-alive behind it
    // (ClientProt.java:204-213).
    for _ in 0..49 {
        assert!(!s.ping(false, std::slice::from_ref(&status)).unwrap());
        assert!(s.take_output().is_none());
    }
    assert!(s.ping(false, std::slice::from_ref(&status)).unwrap());
    assert_eq!(written(&mut s), [243, 2, 3, 0, 2, 0xf7, 1, 93]);
    // The count starts again.
    for _ in 0..49 {
        assert!(!s.ping(false, &[]).unwrap());
    }
    assert!(s.ping(false, &[]).unwrap());
    assert_eq!(written(&mut s), [93]);
    // However long the scene takes, `LoginManager.ticksSinceLastPacket`
    // stands still: Protocol.loop, which counts it, does not run.
    for _ in 0..2000 {
        s.ping(false, &[]).unwrap();
    }
    assert!(!s.closed());
    written(&mut s);
    for _ in 0..750 {
        assert_eq!(s.cycle(&[], None).unwrap(), None);
    }
    assert_eq!(s.cycle(&[], None).unwrap(), Some(Event::Timeout));
    // Before the game there is no such state.
    let (mut s, _) = start(false);
    assert!(!s.ping(true, std::slice::from_ref(&status)).unwrap());
    assert!(s.take_output().is_none());

    // `ping(true)` writes at once, each call a write of its own: the four
    // of a rebuild (LoginManager.java:1025, 1029, 1051, 1056), the first of
    // them carrying what was waiting. The cycle of `Protocol.loop` that
    // follows in the same `mainLoop` is one cycle after the last of them.
    let (mut s, seed) = start(false);
    let (wire, _) = success(seed, false);
    s.feed(&wire).unwrap();
    assert!(s.ping(true, std::slice::from_ref(&status)).unwrap());
    assert_eq!(written(&mut s), [243, 2, 3, 0, 2, 0xf7, 1, 93]);
    for _ in 0..3 {
        assert!(s.ping(true, &[]).unwrap());
        assert_eq!(written(&mut s), [93]);
    }
    for _ in 0..50 {
        s.cycle(&[], None).unwrap();
        assert!(s.take_output().is_none());
    }
    s.cycle(&[], None).unwrap();
    assert_eq!(written(&mut s), [93]);
}
#[test]
fn oversized_input_fails_closed_and_does_not_retain_login_output() {
    let (mut s, _) = start(false);
    assert!(s.feed(&vec![0; 1024 * 1024 + 1]).is_err());
    assert!(s.closed());
    assert!(s.take_output().is_none());
    assert!(s.feed(&[2]).is_err());
}

#[test]
fn original_success_header_and_complete_rebuild_gate_match_over_fragmented_streams() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/players/session.json")).unwrap();
    for (case_index, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let (mut s, _) = start(false);
        assert!(s.feed(&[2]).unwrap().is_empty());
        let decode = |hex: &str| {
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>()
        };
        let wire = decode(case["wire"].as_str().unwrap());
        let mut offset = 0;
        let mut events = vec![];
        for (i, stop) in case["stops"].as_array().unwrap().iter().enumerate() {
            let stop = stop.as_u64().unwrap() as usize;
            events.extend(s.feed(&wire[offset..stop]).unwrap());
            offset = stop;
            assert_eq!(
                s.in_game(),
                case["expected"]["states"][i][1] == 2,
                "case{case_index} chunk{i}"
            );
        }
        let Event::World(w) = events.remove(0) else {
            panic!("world header required")
        };
        assert_eq!(
            serde_json::json!([
                w.staff_level,
                w.blackmarks,
                w.underage,
                w.parental_chat_consent,
                w.parental_ad_consent,
                w.quick_chat,
                w.mouse_recording,
                w.player_index,
                w.player_member,
                w.map_members
            ]),
            case["expected"]["fields"],
            "case{case_index}"
        );
        assert_eq!(
            events,
            vec![Event::Frame(Frame {
                opcode: 162,
                payload: decode(case["expected"]["rebuild"].as_str().unwrap())
            })]
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_transport_exchanges_fragmented_bytes_in_order_without_render_thread_io() {
    use rs09_client::transport::{Event as Net, Transport};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        time::{Duration, Instant},
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let expected: Vec<u8> = (0..200_000).map(|n| (n * 37) as u8).collect();
    let server_bytes = expected.clone();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = vec![0; 60000];
        socket.read_exact(&mut request).unwrap();
        assert_eq!(request, vec![42; 60000]);
        for chunk in server_bytes.chunks(997) {
            socket.write_all(chunk).unwrap();
        }
    });
    let mut t = Transport::connect(&address.to_string()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut received = vec![];
    let mut connected = false;
    let mut closed = false;
    while Instant::now() < deadline {
        match t.poll() {
            Some(Net::Connected) => {
                assert!(!connected);
                connected = true;
                t.send(vec![42; 60000]).unwrap();
            }
            Some(Net::Data(bytes)) => received.extend(bytes),
            Some(Net::Closed) => {
                closed = true;
                break;
            }
            Some(Net::Error(error)) => panic!("{error}"),
            None => std::thread::sleep(Duration::from_millis(1)),
        }
    }
    server.join().unwrap();
    assert!(connected && closed);
    assert_eq!(received, expected);
}

/// Transcription test of a scene rebuild on the wire: `client.mainLoop`
/// in game state 25 (client.java:1725-1734), `LoginManager.rebuildMap`
/// (LoginManager.java:885-1135), `ClientProt.ping` (ClientProt.java:199-219)
/// and the `Protocol.loop` that follows in the cycle that ends the rebuild.
/// Traced by hand; the original was not run. The session and its encoders
/// are the real ones.
#[test]
fn a_rebuild_writes_its_keep_alives_and_the_same_cycle_then_writes_the_marker() {
    use rs09_client::{
        client_events::{Client, ClientEvents, Ping, Shared, Window},
        input::Input,
    };
    use std::collections::VecDeque;
    struct Wire {
        session: Session,
        writes: Vec<Vec<u8>>,
    }
    impl Wire {
        fn new() -> Self {
            let (mut session, seed) = start(false);
            let (wire, _) = success(seed, false);
            session.feed(&wire).unwrap();
            Self {
                session,
                writes: vec![],
            }
        }
        /// What the session has ready, as one socket write.
        fn flush(&mut self) {
            let mut bytes = vec![];
            while let Some(chunk) = self.session.take_output() {
                bytes.extend(chunk);
            }
            if !bytes.is_empty() {
                self.writes.push(bytes);
            }
        }
        fn opcodes(&self) -> Vec<Vec<u8>> {
            self.writes
                .iter()
                .map(|write| {
                    let mut decoder =
                        rs09_protocol::game::Decoder::new(Direction::ClientToServer, 65536, 65535);
                    decoder.push(write).unwrap();
                    let mut opcodes = vec![];
                    while let Some(frame) = decoder.next_frame().unwrap() {
                        opcodes.push(frame.opcode);
                    }
                    opcodes
                })
                .collect()
        }
    }
    impl Ping for Wire {
        fn ping(&mut self, force: bool, waiting: &[Frame]) -> bool {
            let wrote = self.session.ping(force, waiting).unwrap();
            self.flush();
            wrote
        }
    }
    const WINDOW: Window = Window {
        mode: 2,
        frame: [765, 503],
        anti_aliasing: 0,
    };
    const CLIENT: Client = Client {
        keyboard_camera: false,
        camera: [128, 0],
        preferences: 0,
        scene_ready: true,
    };
    /// One cycle of `Protocol.loop` with nothing to do but its own packets.
    fn protocol_loop(events: &mut ClientEvents, input: &mut Input, wire: &mut Wire) {
        let (mut sent, mut timer) = (true, 0);
        let cycle = events.cycle(
            input,
            &CLIENT,
            &WINDOW,
            &mut Shared {
                verify_id: 0,
                preferences_sent: &mut sent,
                logout_timer: &mut timer,
            },
        );
        assert!(cycle.playing);
        let (written, verify_id) = cycle.written([]);
        wire.session.cycle(&written, verify_id.as_ref()).unwrap();
        wire.flush();
    }
    /// A login: `resetGameSessionState` ends with the window status, and the
    /// rebuild packet the login reads makes the state 25.
    fn login(keep_alives: u32, waiting_cycles: u32) -> (Wire, ClientEvents, Input) {
        let mut wire = Wire::new();
        let mut events = ClientEvents::default();
        let mut input = Input {
            focus: true,
            ..Default::default()
        };
        let mut timer = 0;
        events.session_start(false, &WINDOW, &mut timer, &mut input);
        assert!(events.rebuild_packet([400, 400], 0, true));
        let mut pending = VecDeque::new();
        // The map files are not all there yet: `rebuildMap` returns after
        // its `ping(false)` (LoginManager.java:886, 945).
        for _ in 0..waiting_cycles {
            input.cycle();
            assert!(!events.rebuild_cycle(Some(&mut wire), &mut pending, false, keep_alives));
        }
        // They are: the scene is built, with a `ping(true)` after each
        // blocking stage, and `Protocol.loop` runs in the same cycle.
        input.cycle();
        assert!(events.rebuild_cycle(Some(&mut wire), &mut pending, true, keep_alives));
        assert!(!events.rebuilding());
        protocol_loop(&mut events, &mut input, &mut wire);
        (wire, events, input)
    }

    // A rebuild that takes a few cycles: nothing is written while it waits.
    // Its first forced keep-alive carries the window status that was
    // waiting; three more follow, each a write of its own; then the write of
    // the cycle's `Protocol.loop`: the marker (20), MAP_REBUILD_COMPLETE
    // (110) and the camera report (21) the session start asked for.
    let (wire, _, _) = login(4, 3);
    assert_eq!(
        wire.opcodes(),
        [
            vec![243, 93],
            vec![93],
            vec![93],
            vec![93],
            vec![20, 110, 21]
        ]
    );

    // With an underwater map there are seven (LoginManager.java:1074-1085).
    let (wire, _, _) = login(7, 0);
    let opcodes = wire.opcodes();
    assert_eq!(opcodes.len(), 8);
    assert_eq!(opcodes[0], [243, 93]);
    assert!(opcodes[1..7].iter().all(|write| write == &[93]));
    assert_eq!(opcodes[7], [20, 110, 21]);

    // A rebuild that waits for more than a second: the fiftieth
    // `ping(false)` writes what was waiting, and the forced ones are alone.
    let (wire, mut events, mut input) = login(4, 60);
    assert_eq!(
        wire.opcodes(),
        [
            vec![243, 93],
            vec![93],
            vec![93],
            vec![93],
            vec![93],
            vec![20, 110, 21]
        ]
    );

    // Afterwards one write per cycle that has something to say, and the
    // keep-alive in the 51st silent cycle (Protocol.java:2976-2978).
    let mut wire = wire;
    let before = wire.writes.len();
    for _ in 0..50 {
        input.cycle();
        protocol_loop(&mut events, &mut input, &mut wire);
    }
    assert_eq!(wire.writes.len(), before);
    input.cycle();
    protocol_loop(&mut events, &mut input, &mut wire);
    assert_eq!(wire.opcodes().last().unwrap(), &[93]);
}
