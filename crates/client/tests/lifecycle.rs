//! Session lifecycle: game state transitions, the login retry rule, the
//! session resets and the status box.
//!
//! These are transcription tests. The expected values are worked out by hand
//! from the pinned RT4 sources (`client.java`, `LoginManager.java`,
//! `Protocol.java`, `InterfaceList.java`, `Fonts.java`, `VarpDomain.java`,
//! `Camera.java`); `server_events.rs` pins the hashes of those files. The
//! original classes are not executed, with one exception:
//! `the_handled_opcode_table_is_the_one_in_the_pinned_source` derives its
//! expectation from the reference files mechanically. The socket test at the
//! end runs this client's own connection code against a scripted peer.
use rs09_client::{
    actor_animation, camera,
    lifecycle::{
        self, Attempt, Dial, Effect, GameState, Jitter, Lifecycle, LoginLoop, LoginPoll, LoginSeam,
        LogoutReason, Session, StatusBox,
    },
    npc_updates, overheads,
    pack::Pack,
    session::WorldLogin,
    ui::{
        Component,
        actions::SelectedItem,
        font::Metrics,
        state::{Key, State},
        vm::{ChatLine, Vm},
    },
    walking::{DestinationFlag, Movement},
    world_spot,
};
use rs09_protocol::game::{Frame, events::ServerEvent, ground, players};
use std::collections::{BTreeMap, VecDeque};

fn world() -> WorldLogin {
    WorldLogin {
        staff_level: 1,
        blackmarks: 0,
        underage: false,
        parental_chat_consent: false,
        parental_ad_consent: false,
        quick_chat: false,
        mouse_recording: true,
        player_index: 77,
        player_member: true,
        map_members: true,
    }
}

/// A scripted `LoginManager`: records `clear()` calls and plays back replies.
#[derive(Default)]
struct Script {
    starts: Vec<bool>,
    replies: VecDeque<LoginPoll>,
    cycles: usize,
}
impl LoginSeam for Script {
    fn start(&mut self, reconnect: bool) {
        self.starts.push(reconnect);
    }
    fn cycle(&mut self) -> LoginPoll {
        self.cycles += 1;
        self.replies.pop_front().unwrap_or(LoginPoll::Pending)
    }
}
fn script(replies: impl IntoIterator<Item = LoginPoll>) -> Script {
    Script {
        replies: replies.into_iter().collect(),
        ..Default::default()
    }
}
fn change(from: GameState, to: GameState) -> Effect {
    Effect::State { from, to }
}
/// A lifecycle that logged in at the title screen and finished its first
/// scene: game state 30.
fn in_game() -> Lifecycle {
    let mut life = Lifecycle::new();
    let mut login = script([LoginPoll::Entered(world())]);
    life.start_login(&mut login);
    life.cycle(&mut login);
    assert!(life.rebuild_packet(400, 400));
    life.rebuild_finished();
    life.take_effects();
    assert_eq!(life.state(), GameState::InGame);
    life
}

#[test]
fn game_state_numbers_are_the_originals() {
    assert_eq!(
        [
            GameState::Title,
            GameState::Loading,
            GameState::InGame,
            GameState::ConnectionLost
        ]
        .map(GameState::code),
        [10, 25, 30, 40]
    );
}

#[test]
fn a_title_login_resets_the_session_then_loads_then_enters_the_game() {
    let mut life = Lifecycle::new();
    assert_eq!((life.state(), life.reply), (GameState::Title, -2));
    let mut login = script([LoginPoll::Pending, LoginPoll::Entered(world())]);
    // Nothing runs until a login is asked for.
    life.cycle(&mut login);
    assert_eq!(login.cycles, 0);
    life.start_login(&mut login);
    assert_eq!((login.starts.as_slice(), life.reply), (&[false][..], -3));
    life.cycle(&mut login);
    assert!(life.take_effects().is_empty() && life.logging_in());
    life.cycle(&mut login);
    // Step 9: reply 2 and resetGameSessionState (which ends by sending the
    // window details). The game state is still 10 until the rebuild packet.
    assert_eq!(
        life.take_effects(),
        [Effect::ResetSession(world()), Effect::WindowStatus]
    );
    assert_eq!((life.state(), life.reply), (GameState::Title, 2));
    assert_eq!((life.self_id, life.mouse_recorder), (77, true));
    assert!(!life.logging_in());
    // centralZoneX was set to -1: even zone 0,0 is a new scene.
    assert!(life.rebuild_packet(0, 0));
    assert_eq!(
        life.take_effects(),
        [change(GameState::Title, GameState::Loading)]
    );
    // rebuildMap's end in state 25: opcode 20 with its constant, state 30,
    // then opcode 110.
    life.rebuild_finished();
    assert_eq!(
        life.take_effects(),
        [
            Effect::Send(Frame {
                opcode: 20,
                payload: 1057001181i32.to_be_bytes().to_vec()
            }),
            change(GameState::Loading, GameState::InGame),
            Effect::Send(Frame {
                opcode: 110,
                payload: vec![]
            }),
        ]
    );
    // Not in state 25: nothing more is sent.
    life.rebuild_finished();
    assert!(life.take_effects().is_empty());
    // No login runs in game.
    life.cycle(&mut login);
    assert_eq!(login.cycles, 2);
}

#[test]
fn a_refused_title_login_stays_at_the_title_with_its_reply() {
    for (poll, reply) in [
        (LoginPoll::Failed(3), 3),
        (LoginPoll::Failed(-4), -4),
        // Reply 15 outside state 40 is just a reply code.
        (LoginPoll::Reconnected, 15),
    ] {
        let mut life = Lifecycle::new();
        let mut login = script([poll]);
        life.start_login(&mut login);
        life.cycle(&mut login);
        assert!(life.take_effects().is_empty());
        assert_eq!((life.state(), life.reply), (GameState::Title, reply));
        assert!(!life.logging_in());
        // The attempt is over: the login is not cycled again.
        life.cycle(&mut login);
        assert_eq!(login.cycles, 1);
    }
}

#[test]
fn a_rebuild_for_the_loaded_zone_is_skipped_and_a_new_zone_reloads() {
    let mut life = in_game();
    // loadRegion returns at once for the zone already loaded.
    assert!(!life.rebuild_packet(400, 400));
    assert!(life.take_effects().is_empty());
    assert_eq!(life.state(), GameState::InGame);
    assert!(life.rebuild_packet(401, 400));
    assert_eq!(
        life.take_effects(),
        [change(GameState::InGame, GameState::Loading)]
    );
    // A second rebuild while loading changes the zone, not the state.
    assert!(life.rebuild_packet(402, 400));
    assert!(life.take_effects().is_empty());
}

#[test]
fn closing_the_world_map_rebuilds_the_scene_without_a_packet() {
    // WorldMap.reset (WorldMap.java:977-981, from Protocol.java:1284) and
    // `::rebuild` (Cheat.java:168-169) call client.setGameState(25);
    // rebuildMap then ends as after a rebuild packet
    // (LoginManager.java:1106-1130). loadRegion is not involved, so the
    // central zone stays.
    let mut life = in_game();
    assert!(life.rebuild_requested());
    assert_eq!(
        life.take_effects(),
        [change(GameState::InGame, GameState::Loading)]
    );
    // setGameState does nothing when the state is the same.
    assert!(!life.rebuild_requested());
    assert!(life.take_effects().is_empty());
    life.rebuild_finished();
    assert_eq!(
        life.take_effects(),
        [
            Effect::Send(Frame {
                opcode: 20,
                payload: 1057001181i32.to_be_bytes().to_vec()
            }),
            change(GameState::Loading, GameState::InGame),
            Effect::Send(Frame {
                opcode: 110,
                payload: vec![]
            }),
        ]
    );
    assert!(!life.rebuild_packet(400, 400));
    // Only a running game has a scene to rebuild.
    let mut title = Lifecycle::new();
    assert!(!title.rebuild_requested());
    assert_eq!(title.state(), GameState::Title);
}

#[test]
fn the_world_maps_requests_reach_the_lifecycle_and_the_wire() {
    use rs09_client::{
        client_events::{Client, ClientEvents, Cycle, Shared, Window},
        input::Input,
        ui::world_map_host::{Host, SceneRequests},
    };
    // The whole path of a closing world map: the top-level interface opened
    // with reset type 2 (Protocol.java:1284) asks for the rebuild, the
    // lifecycle shows the loading state, and the main loop's own packets end
    // the rebuild with opcodes 20 and 110 in the first cycle after the
    // lifecycle is back in game (LoginManager.java:1106-1130).
    let window = Window {
        mode: 2,
        frame: [765, 503],
        anti_aliasing: 0,
    };
    let mut devices = Input::default();
    let mut events = ClientEvents::default();
    let (mut timer, mut sent) = (0, true);
    let mut cycle = |events: &mut ClientEvents, devices: &mut Input, scene_ready: bool| {
        devices.cycle();
        events.cycle(
            devices,
            &Client {
                scene_ready,
                ..Client::default()
            },
            &window,
            &mut Shared {
                verify_id: 0,
                preferences_sent: &mut sent,
                logout_timer: &mut timer,
            },
        )
    };
    let opcodes = |cycle: Cycle| -> Vec<u8> {
        let (frames, verify) = cycle.written([]);
        frames.iter().chain(&verify).map(|f| f.opcode).collect()
    };
    let mut life = in_game();
    events.session_start(false, &window, &mut 0, &mut devices);
    assert_eq!(opcodes(cycle(&mut events, &mut devices, true)), [243]);
    assert_eq!(
        opcodes(cycle(&mut events, &mut devices, true)),
        [20, 110, 21]
    );

    let mut map = Host::default();
    // Any other reset type asks for nothing.
    map.top_level_opened(0);
    assert!(!lifecycle::world_map_requests(
        map.take_scene_requests(),
        &mut life,
        &mut events
    ));
    assert_eq!(life.state(), GameState::InGame);
    assert!(!events.rebuilding());

    map.top_level_opened(2);
    let requests = map.take_scene_requests();
    assert_eq!(requests.rebuild_scene, 1);
    assert!(!lifecycle::world_map_requests(
        requests,
        &mut life,
        &mut events
    ));
    assert_eq!(
        life.take_effects(),
        [change(GameState::InGame, GameState::Loading)]
    );
    assert!(events.rebuilding());
    // Nothing is written, and no packet read, while the state is 25.
    let in_game_now = |life: &Lifecycle| life.state() == GameState::InGame;
    assert_eq!(events.reading_cycles(1, in_game_now(&life)), 0);
    assert_eq!(
        cycle(&mut events, &mut devices, in_game_now(&life)),
        Cycle::default()
    );
    life.rebuild_finished();
    assert_eq!(events.reading_cycles(1, in_game_now(&life)), 1);
    assert_eq!(
        opcodes(cycle(&mut events, &mut devices, in_game_now(&life))),
        [20, 110]
    );
    assert_eq!(
        opcodes(cycle(&mut events, &mut devices, true)),
        [] as [u8; 0]
    );

    // A slow load step only asks for the timer; the scene clear the
    // original does while the map is open is not carried out.
    let slow = SceneRequests {
        clear_scene: 1,
        rebuild_scene: 0,
        reset_timer: 3,
    };
    life.take_effects();
    assert!(lifecycle::world_map_requests(slow, &mut life, &mut events));
    assert!(life.take_effects().is_empty() && !events.rebuilding());
}

#[test]
fn the_logout_packet_runs_process_logout_and_returns_to_the_title() {
    let mut life = in_game();
    life.logout_packet();
    assert!(life.stopping());
    assert_eq!(
        life.take_effects(),
        [
            Effect::ProcessLogout(LogoutReason::Server),
            change(GameState::InGame, GameState::Title)
        ]
    );
    assert_eq!(life.logout_reason, Some(LogoutReason::Server));
    assert!(!life.stopping() && !life.logging_in());
    // processLogout zeroes the central zone: zone 0,0 is "already loaded".
    assert!(!life.rebuild_packet(0, 0));
}

#[test]
fn a_lost_connection_goes_to_state_40_unless_the_logout_timer_runs() {
    // Timer 0: previousSocket = socket, setGameState(40) with clear().
    let mut life = in_game();
    let mut login = Script::default();
    life.connection_lost(0, &mut login);
    assert_eq!(
        life.take_effects(),
        [
            Effect::SetAsideConnection,
            change(GameState::InGame, GameState::ConnectionLost)
        ]
    );
    assert_eq!(login.starts, [true], "the login block is a reconnect");
    assert_eq!(life.reply, -3);
    // Already lost: a second report changes nothing.
    life.connection_lost(0, &mut login);
    assert!(life.take_effects().is_empty());
    assert_eq!(login.starts.len(), 1);

    // Timer running (logout button or idle logout sent): log out at once.
    for timer in [1, 250] {
        let mut life = in_game();
        let mut login = Script::default();
        life.connection_lost(timer, &mut login);
        assert_eq!(
            life.take_effects(),
            [
                Effect::ProcessLogout(LogoutReason::LostWhileLoggingOut),
                change(GameState::InGame, GameState::Title)
            ]
        );
        assert!(login.starts.is_empty());
    }

    // This client reads packets while its scene loads, so it can meet the
    // loss in state 25 (the original meets it on its first cycle back in
    // state 30, with the same outcome). At the title screen nothing happens.
    let mut life = in_game();
    life.rebuild_packet(1, 1);
    life.take_effects();
    let mut login = Script::default();
    life.connection_lost(0, &mut login);
    assert_eq!(life.state(), GameState::ConnectionLost);
    let mut life = Lifecycle::new();
    life.connection_lost(0, &mut login);
    assert!(life.take_effects().is_empty());
}

#[test]
fn state_40_resumes_on_reply_15() {
    let mut life = in_game();
    life.packet_read(225);
    let mut login = script([
        LoginPoll::Pending,
        LoginPoll::Pending,
        LoginPoll::Reconnected,
    ]);
    life.connection_lost(0, &mut login);
    life.take_effects();
    for _ in 0..2 {
        life.cycle(&mut login);
        assert!(life.take_effects().is_empty());
        assert_eq!(life.state(), GameState::ConnectionLost);
    }
    life.cycle(&mut login);
    // LoginManager.reconnect(): its resets, setGameState(30), window details.
    assert_eq!(
        life.take_effects(),
        [
            Effect::Reconnect,
            change(GameState::ConnectionLost, GameState::InGame),
            Effect::WindowStatus
        ]
    );
    assert_eq!(life.reply, 15);
    // The scene is kept: the same zone is still the loaded one.
    assert!(!life.rebuild_packet(400, 400));
    // reconnect() forgot the previous opcodes.
    life.packet_read(7);
    life.unknown_opcode(0);
    assert_eq!(
        life.take_effects()[0],
        Effect::Report("T1 - 7,-1,-1 - 0".into())
    );
}

#[test]
fn state_40_logs_out_on_any_other_reply() {
    for reply in [-5, -4, 1, 3, 5, 21] {
        let mut life = in_game();
        let mut login = script([LoginPoll::Failed(reply)]);
        life.connection_lost(0, &mut login);
        life.take_effects();
        life.cycle(&mut login);
        assert_eq!(
            life.take_effects(),
            [
                Effect::ProcessLogout(LogoutReason::ReconnectFailed(reply)),
                change(GameState::ConnectionLost, GameState::Title)
            ]
        );
        assert_eq!(life.reply, reply);
        // Back at the title screen nothing retries by itself.
        life.cycle(&mut login);
        assert_eq!(login.cycles, 1);
    }
}

#[test]
fn state_40_with_reply_2_is_a_whole_new_login() {
    let mut life = in_game();
    let mut login = script([LoginPoll::Entered(world())]);
    life.connection_lost(0, &mut login);
    life.take_effects();
    life.cycle(&mut login);
    // Step 9 ran inside LoginManager.loop; mainLoop does nothing for reply 2.
    assert_eq!(
        life.take_effects(),
        [Effect::ResetSession(world()), Effect::WindowStatus]
    );
    assert_eq!(life.state(), GameState::ConnectionLost);
    // The rebuild packet read by step 9 is never skipped (centralZoneX -1).
    assert!(life.rebuild_packet(400, 400));
    assert_eq!(
        life.take_effects(),
        [change(GameState::ConnectionLost, GameState::Loading)]
    );
}

#[test]
fn unhandled_packets_and_handler_errors_report_and_log_out() {
    // T1: "T1 - " + opcode + "," + opcode3 + "," + opcode4 + " - " + length
    let mut life = in_game();
    for opcode in [225, 32, 1] {
        life.packet_read(opcode);
    }
    life.unknown_opcode(0);
    assert_eq!(
        life.take_effects(),
        [
            Effect::Report("T1 - 1,32,225 - 0".into()),
            Effect::ProcessLogout(LogoutReason::UnknownOpcode(1)),
            change(GameState::InGame, GameState::Title)
        ]
    );

    // T2 adds the local player's absolute tile and at most 50 payload bytes
    // as signed decimals, each followed by a comma.
    let mut life = in_game();
    life.packet_read(225);
    life.packet_read(70);
    life.handler_error(&[0, 127, 128, 255], [3222, 3218]);
    assert_eq!(
        life.take_effects(),
        [
            Effect::Report("T2 - 70,225,-1 - 4,3222,3218 - 0,127,-128,-1,".into()),
            Effect::ProcessLogout(LogoutReason::HandlerError),
            change(GameState::InGame, GameState::Title)
        ]
    );
    let mut life = in_game();
    life.packet_read(70);
    life.handler_error(&[1; 60], [1, 2]);
    let Effect::Report(text) = &life.take_effects()[0] else {
        panic!("report first")
    };
    assert_eq!(
        *text,
        format!("T2 - 70,-1,-1 - 60,1,2 - {}", "1,".repeat(50))
    );
}

#[test]
fn the_handled_opcode_table_is_the_one_in_the_pinned_source() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/client/client/src/main/java/rt4");
    let read = |name: &str| String::from_utf8(std::fs::read(root.join(name)).unwrap()).unwrap();
    let mut constants = BTreeMap::new();
    for line in read("ServerProt.java").lines() {
        // `public static final int NAME = 123; // comment`
        if let Some((name, value)) = line
            .trim()
            .strip_prefix("public static final int ")
            .and_then(|rest| rest.split_once(';'))
            .and_then(|(declaration, _)| declaration.split_once(" = "))
        {
            constants.insert(name.to_owned(), value.parse::<u8>().unwrap());
        }
    }
    let protocol = read("Protocol.java");
    let from = protocol.find("static boolean readPacketInternal").unwrap();
    let to = protocol.find("\"T1 - \"").unwrap();
    let mut handled = std::collections::BTreeSet::new();
    for piece in protocol[from..to].split("opcode == ServerProt.").skip(1) {
        let name: String = piece
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        handled.insert(constants[&name]);
    }
    assert_eq!(
        handled.into_iter().collect::<Vec<_>>(),
        lifecycle::ORIGINAL_SERVER_OPCODES
    );
    assert!(lifecycle::original_handles(86) && !lifecycle::original_handles(1));
}

/// A scripted connection for `LoginLoop`.
#[derive(Default)]
struct Line {
    opens: Vec<bool>,
    open_fails: bool,
    closes: usize,
    polls: VecDeque<Attempt>,
}
impl Dial for Line {
    fn open(&mut self, reconnect: bool) -> Result<(), String> {
        self.opens.push(reconnect);
        if self.open_fails {
            Err("refused".into())
        } else {
            Ok(())
        }
    }
    fn poll(&mut self) -> Attempt {
        self.polls.pop_front().unwrap_or(Attempt::Pending)
    }
    fn close(&mut self) {
        self.closes += 1;
    }
}
fn line(polls: impl IntoIterator<Item = Attempt>) -> LoginLoop<Line> {
    LoginLoop::new(Line {
        polls: polls.into_iter().collect(),
        ..Default::default()
    })
}

#[test]
fn the_login_loop_times_out_after_2000_cycles_twice() {
    let mut login = line([]);
    // Idle until started.
    assert_eq!(login.cycle(), LoginPoll::Pending);
    assert!(login.dial.opens.is_empty());
    login.start(true);
    // `++loops > 2000`: cycles 1 to 2000 belong to the first attempt.
    for _ in 0..2000 {
        assert_eq!(login.cycle(), LoginPoll::Pending);
    }
    assert_eq!(login.dial.opens, [true]);
    // Cycle 2001 closes it and opens the second attempt in the same call.
    assert_eq!(login.cycle(), LoginPoll::Pending);
    assert_eq!(login.dial.opens, [true, true]);
    for _ in 0..2000 {
        assert_eq!(login.cycle(), LoginPoll::Pending);
    }
    // The second time-out is reply -5.
    assert_eq!(login.cycle(), LoginPoll::Failed(-5));
    assert!(!login.active());
    assert_eq!(login.cycle(), LoginPoll::Pending);
    assert_eq!(login.dial.opens.len(), 2);
}

#[test]
fn the_login_loop_retries_one_failed_connection_then_gives_reply_minus_4() {
    let mut login = line([]);
    login.dial.open_fails = true;
    login.start(false);
    // The IOException handler: errors 0 -> step 1 again, on the next cycle.
    assert_eq!(login.cycle(), LoginPoll::Pending);
    assert_eq!(login.cycle(), LoginPoll::Failed(-4));
    assert_eq!(login.dial.opens, [false, false]);

    // A connection that fails after it opened counts the same way, and a
    // retry gets its full 2000 cycles.
    let mut login = line([Attempt::Pending, Attempt::Io]);
    login.start(true);
    assert_eq!(login.cycle(), LoginPoll::Pending);
    assert_eq!(login.cycle(), LoginPoll::Pending);
    assert_eq!(login.dial.opens, [true]);
    for _ in 0..2000 {
        assert_eq!(login.cycle(), LoginPoll::Pending);
    }
    assert_eq!(login.dial.opens, [true, true]);
    assert_eq!(login.cycle(), LoginPoll::Failed(-5));
}

#[test]
fn the_login_loop_passes_replies_through_and_retries_reply_23_once() {
    let mut login = line([Attempt::Reply(23), Attempt::Reply(23)]);
    login.start(false);
    assert_eq!(login.cycle(), LoginPoll::Pending);
    assert_eq!(login.cycle(), LoginPoll::Failed(23));
    assert_eq!(login.dial.opens.len(), 2);

    for (attempt, poll) in [
        (Attempt::Reply(5), LoginPoll::Failed(5)),
        (Attempt::Reply(1), LoginPoll::Failed(1)),
        (Attempt::Reconnected, LoginPoll::Reconnected),
        (Attempt::Entered(world()), LoginPoll::Entered(world())),
    ] {
        let mut login = line([attempt]);
        login.start(true);
        assert_eq!(login.cycle(), poll);
        assert!(!login.active());
        assert_eq!(login.dial.opens, [true]);
    }
    // clear() restarts the counters: a new start gets two attempts again.
    let mut login = line([Attempt::Io, Attempt::Io, Attempt::Io]);
    login.start(false);
    login.cycle();
    login.start(false);
    assert_eq!(login.cycle(), LoginPoll::Pending);
    assert_eq!(login.cycle(), LoginPoll::Failed(-4));
}

fn pack(files: &[(&str, Vec<u8>)]) -> Pack {
    let mut body = Vec::new();
    let mut entries = serde_json::Map::new();
    for (name, bytes) in files {
        entries.insert(
            (*name).to_owned(),
            serde_json::json!([body.len(), bytes.len()]),
        );
        body.extend_from_slice(bytes);
    }
    let manifest =
        serde_json::to_vec(&serde_json::json!({"source":"lifecycle tests","files":entries}))
            .unwrap();
    let mut bytes = b"RS09VIS1".to_vec();
    bytes.extend((manifest.len() as u32).to_le_bytes());
    bytes.extend(manifest);
    bytes.extend(body);
    Pack::decode(bytes).unwrap()
}

/// Every piece of session state, filled in.
struct World {
    vm: Vm,
    ui: State,
    players: players::State,
    npcs: npc_updates::State,
    ground: ground::State,
    world_spots: world_spot::State,
    camera: camera::Script,
    map_flag: DestinationFlag,
    motion: Movement,
    actors: BTreeMap<usize, Movement>,
    overheads: BTreeMap<(bool, usize), overheads::State>,
}
impl World {
    fn populated() -> Self {
        // Variable 1 has client code 5 (mouse buttons); variable 2 has none.
        let mut ui = State::new(pack(&[
            ("varptype/1", vec![5, 0, 5, 0]),
            ("varptype/2", vec![0]),
        ]));
        for (group, child) in [(548, 0), (548, 1), (149, 0), (200, 3)] {
            let key = Key::component(group << 16 | child);
            ui.loaded.insert(group);
            ui.components.insert(
                key,
                Component {
                    id: key.id,
                    ..Default::default()
                },
            );
        }
        ui.root = 548;
        ui.subinterfaces.insert(548 << 16 | 1, (149, 1));
        ui.subinterfaces.insert(548 << 16, (200, 0));
        ui.containers.insert(93, vec![]);
        ui.verify_id = 41;
        ui.selected_item = Some(SelectedItem {
            id: 995,
            slot: 0,
            component: 149 << 16,
            text: "Coins".into(),
        });
        ui.targeting.active = true;
        ui.targeting.default_cursor = 4;
        ui.delayed.server(1, 5, 9, None);

        let mut vm = Vm::default();
        vm.rights = 2;
        vm.members = true;
        vm.site_members = true;
        vm.world = 2;
        vm.blackmarks = 6;
        vm.quick_chat = true;
        vm.chat_restricted = true;
        vm.player_name = "Old Name".into();
        vm.combat_level = 99;
        vm.logout_on_disconnect_timer = 100;
        vm.reboot_timer = 600;
        vm.run_energy = 55;
        vm.weight = 12;
        [vm.public_chat, vm.private_chat, vm.trade_chat] = [1, 2, 3];
        vm.varps.extend([(1, 4), (2, 9)]);
        vm.pending_varps.server(1, 4, 0);
        vm.pending_varps.server(2, 9, 0);
        vm.varcs.insert(5, 123);
        vm.varcstrs.insert(1, "kept".into());
        vm.minimap.state = 2;
        vm.minimap.hints[1] = Some(Default::default());
        vm.messages.push(ChatLine::default());
        vm.chat_lines = 7;
        vm.effects.push([1, 1, 0]);
        vm.outgoing.push(Frame {
            opcode: 93,
            payload: vec![],
        });
        vm.player_options.labels[0] = Some("Attack".into());
        vm.player_options.secondary[0] = true;
        vm.player_options.cursors[0] = 3;
        vm.boosted_levels[3] = 10;
        vm.base_levels[3] = 10;
        vm.experience[3] = 1154;
        vm.walk_text = Some("Dive here".into());
        vm.preferences.never_remove_roofs = true;
        vm.menu_style = Some([0; 11]);
        vm.social.friend_state = 2;
        vm.social.clan_name = Some("clan".into());
        vm.social.clan_owner = Some("owner".into());
        vm.social.clan_rank = 3;
        vm.social.clan_min_kick = 4;
        vm.social.offers[2].status = 5;
        vm.social.ignores.push(rs09_client::ui::social::Ignore {
            key: 1,
            name: "a".into(),
        });

        let mut players = players::State::new(10, 20);
        players.local_ids.push(5);
        players.plane = 1;
        players.appearance_cache[5] = Some(vec![1]);
        let mut npcs = npc_updates::State::default();
        npcs.local_ids.push(9);
        let mut npc_motion = Movement::new([3200, 3200]);
        npc_motion.face_entity = 5;
        npcs.actors.insert(
            9,
            npc_updates::Actor {
                type_id: 1,
                metadata: npc_updates::Metadata {
                    size: 1,
                    bas: -1,
                    turn_speed: 32,
                    rotates: true,
                    area_sound: false,
                },
                last_seen: 0,
                motion: npc_motion,
                animation: actor_animation::State::default(),
                chat: vec![],
                chat_loops: 0,
                hit_bar: 0,
                hit_bar_until: 0,
            },
        );
        let mut ground = ground::State::default();
        ground
            .set_stack(
                ground::Tile {
                    plane: 0,
                    x: 3,
                    y: 4,
                },
                Some(ground::Stack::default()),
            )
            .unwrap();
        let mut world_spots = world_spot::State {
            zone: [8, 16],
            ..Default::default()
        };
        world_spots.effects.push(world_spot::Effect {
            id: 1,
            plane: 0,
            fine: [0, 0],
            z: 0,
            start: 0,
            sequence: -1,
            frame: 0,
            clock: 0,
            finished: false,
        });
        let mut camera = camera::Script::default();
        camera.apply(
            &ServerEvent::CameraShake {
                counter: 0,
                axis: 1,
                jitter: 2,
                amplitude: 3,
                frequency: 4,
                phase: 0,
            },
            |_| 0,
        );
        camera.apply(
            &ServerEvent::CameraLookAt {
                counter: 0,
                x: 50,
                y: 60,
                height: 700,
                min_step: 8,
                speed: 9,
            },
            |_| 0,
        );
        camera.locked = true;
        camera.render = [1, 2, 3];
        let mut motion = Movement::new([3222, 3218]);
        motion.face_entity = 9;
        let mut other = Movement::new([3223, 3218]);
        other.face_entity = 32768 + 77;
        Self {
            vm,
            ui,
            players,
            npcs,
            ground,
            world_spots,
            camera,
            map_flag: DestinationFlag { tile: [30, 40] },
            motion,
            actors: [(5, other)].into(),
            overheads: [((false, 5), overheads::State::default())].into(),
        }
    }
    fn session(&mut self) -> Session<'_> {
        Session {
            vm: &mut self.vm,
            ui: Some(&mut self.ui),
            players: &mut self.players,
            npcs: &mut self.npcs,
            ground: &mut self.ground,
            world_spots: &mut self.world_spots,
            camera: &mut self.camera,
            map_flag: &mut self.map_flag,
            motion: &mut self.motion,
            actors: &mut self.actors,
            overheads: &mut self.overheads,
            username: "new_name",
        }
    }
    fn assert_actors_gone(&self) {
        assert!(self.actors.is_empty() && self.overheads.is_empty());
        assert!(self.npcs.actors.is_empty() && self.npcs.local_ids.is_empty());
        assert!(self.players.local_ids.is_empty());
        assert!(self.players.appearance_cache.iter().all(Option::is_none));
        assert_eq!(self.players.players.iter().flatten().count(), 1);
        assert_eq!(self.players.self_player().tile(), [0, 0]);
        assert!(self.players.self_player().appearance.is_none());
        assert!(self.ground.occupied().is_empty());
    }
    fn assert_interfaces_closed(&self) {
        assert_eq!(self.ui.root, -1);
        assert!(self.ui.subinterfaces.is_empty());
        assert!(self.ui.components.is_empty() && self.ui.loaded.is_empty());
        assert!(self.ui.hooks.is_empty());
        assert_eq!(self.ui.verify_id, 0);
    }
}

#[test]
fn a_login_resets_the_session_state_and_keeps_what_the_original_keeps() {
    let mut w = World::populated();
    // Math.random() returning 0.5 every time.
    let jitter = lifecycle::reset_session(w.session(), || 0.5);
    assert_eq!(
        jitter,
        Jitter {
            camera_offset_x: 0,
            yaw_target: 0,
            camera_offset_y: 0,
            zoom_offset: -5,
            compass_angle_offset: 0,
            yaw_drift: 0,
        }
    );
    w.assert_actors_gone();
    w.assert_interfaces_closed();
    assert!(w.ui.containers.is_empty(), "Inv.clear()");
    assert!(w.ui.selected_item.is_none() && !w.ui.targeting.active);
    assert_eq!(w.ui.waiting_key(), None);
    assert!(w.ui.delayed.poll(i64::MAX).is_none(), "DelayedStateChange");
    // The default cursor is only reset by a logout.
    assert_eq!(w.ui.targeting.default_cursor, 4);

    let vm = &w.vm;
    assert_eq!((vm.logout_on_disconnect_timer, vm.reboot_timer), (0, 0));
    assert!(vm.outgoing.is_empty() && vm.effects.is_empty());
    assert!(vm.messages.is_empty());
    assert_eq!(vm.chat_lines, 0);
    assert_eq!(vm.minimap.state, 0);
    assert!(vm.minimap.hints.iter().all(Option::is_none));
    // VarpDomain.reset: client-code variables survive, the rest read 0.
    assert_eq!(vm.varps, [(1, 4)].into());
    assert_eq!(vm.pending_varps.values, [(1, 4)].into());
    // All 2000 client variables are -1; the strings are untouched.
    assert_eq!(vm.varcs.len(), 2000);
    assert!(vm.varcs.values().all(|&value| value == -1));
    assert_eq!(vm.varcstrs[&1], "kept");
    assert!(vm.player_options.labels.iter().all(Option::is_none));
    assert_eq!(
        (vm.player_options.secondary, vm.player_options.cursors),
        ([false; 8], [-1; 8])
    );
    assert_eq!(
        (vm.boosted_levels, vm.base_levels, vm.experience),
        ([0; 25], [0; 25], [0; 25])
    );
    assert_eq!(vm.walk_text, None);
    assert!(!vm.preferences.never_remove_roofs);
    assert_eq!(vm.menu_style, None);
    // PlayerList.self is a new Player: name from the login input, combat 0.
    assert_eq!((vm.player_name.as_str(), vm.combat_level), ("new_name", 0));
    // Friends, clan members and name, and offers go; the ignore list and
    // the clan's owner, rank and kick threshold stay.
    assert_eq!(vm.social.friend_state, 0);
    assert_eq!(vm.social.clan_name, None);
    assert_eq!(vm.social.offers[2].status, 0);
    assert_eq!(vm.social.ignores.len(), 1);
    assert_eq!(vm.social.clan_owner.as_deref(), Some("owner"));
    assert_eq!((vm.social.clan_rank, vm.social.clan_min_kick), (3, 4));
    // Not session state in the original: energy, weight, chat filters, and
    // the login facts themselves (set by step 8 just before).
    assert_eq!((vm.run_energy, vm.weight), (55, 12));
    assert_eq!([vm.public_chat, vm.private_chat, vm.trade_chat], [1, 2, 3]);
    assert_eq!((vm.rights, vm.world, vm.blackmarks), (2, 2, 6));
    assert!(vm.members && vm.site_members && vm.quick_chat && vm.chat_restricted);

    assert_eq!(w.map_flag.tile, [0, 0]);
    assert!(w.world_spots.effects.is_empty() && w.world_spots.projectiles.is_empty());
    assert_eq!(w.world_spots.zone, [8, 16], "currentChunkX/Y are kept");
    assert_eq!((w.motion.head, w.motion.face_entity), ([0, 0], -1));
    // The locked camera's targets are zeroed, but its mode and shakes stay:
    // only a logout calls resetCameraEffects.
    assert!(w.camera.locked && w.camera.shakes[1].active);
    assert_eq!(w.camera.render, [1, 2, 3]);
    let mut zeroed = camera::Script::default();
    zeroed.locked = true;
    zeroed.render = [1, 2, 3];
    zeroed.shakes = w.camera.shakes;
    assert_eq!(w.camera, zeroed);
}

#[test]
fn a_logout_clears_the_world_but_not_the_chat_or_the_lists() {
    let mut w = World::populated();
    lifecycle::process_logout(w.session());
    w.assert_actors_gone();
    w.assert_interfaces_closed();
    assert_eq!(w.ui.targeting.default_cursor, -1);
    // Camera.resetCameraEffects(): shakes off, camera type 1.
    assert!(!w.camera.locked && w.camera.spline.is_none());
    assert!(w.camera.shakes.iter().all(|shake| !shake.active));
    // PlayerList.self = new Player() at fine 3000, 3000.
    assert_eq!(w.motion.fine, [3000, 3000]);
    let vm = &w.vm;
    assert_eq!((vm.player_name.as_str(), vm.combat_level), ("new_name", 0));
    assert!(vm.minimap.hints.iter().all(Option::is_none));
    assert_eq!(vm.varps, [(1, 4)].into());
    // What only the next login clears.
    assert_eq!(vm.messages.len(), 1);
    assert_eq!(vm.varcs[&5], 123);
    assert_eq!(vm.social.friend_state, 2);
    assert_eq!(vm.social.clan_name.as_deref(), Some("clan"));
    assert_eq!(vm.experience[3], 1154);
    assert_eq!(vm.minimap.state, 2);
    assert_eq!(vm.logout_on_disconnect_timer, 100);
    assert_eq!(w.ui.containers.len(), 1);
    assert_eq!(w.map_flag.tile, [30, 40]);
    assert_eq!(w.world_spots.effects.len(), 1);
}

#[test]
fn a_reconnect_keeps_the_scene_and_clears_what_reconnect_clears() {
    let mut w = World::populated();
    lifecycle::reconnect(w.session());
    // faceEntity = -1 for every player and NPC.
    assert_eq!(w.motion.face_entity, -1);
    assert_eq!(w.actors[&5].face_entity, -1);
    assert_eq!(w.npcs.actors[&9].motion.face_entity, -1);
    // Inv.clear(), MiniMap.state, rebootTimer, the outbound buffer.
    assert!(w.ui.containers.is_empty());
    assert_eq!((w.vm.minimap.state, w.vm.reboot_timer), (0, 0));
    assert!(w.vm.outgoing.is_empty());
    // mapFlagX = 0 hides the flag; mapFlagY is not touched.
    assert_eq!(w.map_flag.tile, [0, 40]);
    // Camera.cameraType = 1; the shakes go on.
    assert!(!w.camera.locked && w.camera.shakes[1].active);
    // Everything else is as it was.
    assert_eq!(w.ui.root, 548);
    assert_eq!(w.ui.subinterfaces.len(), 2);
    assert_eq!(w.ui.verify_id, 41);
    assert_eq!(w.players.local_ids, [5]);
    assert_eq!(w.npcs.local_ids, [9]);
    assert_eq!(w.motion.head, [3222, 3218]);
    assert_eq!(w.ground.occupied().len(), 1);
    assert_eq!(w.vm.messages.len(), 1);
    assert_eq!(w.vm.varcs[&5], 123);
    assert_eq!(w.vm.varps.len(), 2);
    assert_eq!(w.vm.logout_on_disconnect_timer, 100);
    assert_eq!(w.vm.minimap.hints.iter().flatten().count(), 1);
    assert_eq!(w.world_spots.effects.len(), 1);
}

#[test]
fn the_six_random_offsets_are_drawn_in_the_original_order() {
    // Each draw scales a different range, so distinct inputs show the order.
    let mut values = [0.0, 0.25, 0.5, 0.75, 0.99, 0.1].into_iter();
    let jitter = Jitter::draw(|| values.next().unwrap());
    assert_eq!(
        jitter,
        Jitter {
            camera_offset_x: -50,         // (int) (0.0 * 100) - 50
            yaw_target: (5 - 10) & 0x7ff, // (int) (0.25 * 20) - 10 & 0x7FF
            camera_offset_y: 0,           // (int) (0.5 * 110) - 55
            zoom_offset: 2,               // (int) (0.75 * 30) - 20
            compass_angle_offset: 58,     // (int) (0.99 * 120) - 60
            yaw_drift: -32,               // (int) (0.1 * 80) - 40
        }
    );
}

#[test]
fn status_texts_and_the_box_around_them() {
    assert_eq!(
        lifecycle::status_text(GameState::Loading, None).unwrap(),
        "Loading - please wait."
    );
    assert_eq!(
        lifecycle::status_text(GameState::Loading, Some(37)).unwrap(),
        "Loading - please wait.<br>(37%)"
    );
    let lost = lifecycle::status_text(GameState::ConnectionLost, Some(1)).unwrap();
    assert_eq!(
        lost,
        "Connection lost.<br>Please wait - attempting to reestablish."
    );
    assert_eq!(lifecycle::status_text(GameState::InGame, None), None);
    assert_eq!(lifecycle::status_text(GameState::Title, None), None);

    // A font in which every glyph is 5 wide and the line height is 12, with
    // paragraph paddings 10 above and 2 below the baseline.
    let metrics = Metrics {
        advances: [5; 256],
        line_height: 12,
        kerning: None,
        name_icons: vec![],
    };
    // One line of 22 glyphs: maxWidth 110, textHeight 13.
    let one = StatusBox::layout(&metrics, [10, 2], "Loading - please wait.");
    assert_eq!(one.rect, [6, 6, 110 + 8, 13 + 8]);
    // Centred vertically: 10 + top 10 + (13 - 10 - 2 - 0) / 2 = 20.
    assert_eq!(one.lines, [("Loading - please wait.".to_owned(), [10, 20])]);
    // Two lines of 16 and 40 glyphs: maxWidth 200, textHeight 26. The first
    // line is centred in the width of the second.
    let two = StatusBox::layout(&metrics, [10, 2], &lost);
    assert_eq!(two.rect, [6, 6, 208, 34]);
    assert_eq!(two.lines.len(), 2);
    // 10 + 10 + (26 - 10 - 2 - 12) / 2 = 21, then one line height further.
    assert_eq!(two.lines[0].1, [10 + (200 - 80) / 2, 21]);
    assert_eq!(two.lines[1].1, [10, 33]);
    assert_eq!(
        (StatusBox::FILL, StatusBox::OUTLINE, StatusBox::TEXT),
        (0, 0xff_ffff, 0xff_ffff)
    );
}

/// A session that has logged in (reply 2, details, an empty rebuild packet).
fn game_session(reconnect: bool) -> rs09_client::session::Session {
    use rs09_client::session::{Config, Session};
    let mut config = Config::new(b"lifecycle".to_vec(), b"secret".to_vec(), [1, 2]);
    config.reconnect = reconnect;
    config.verify_id = 9;
    let mut session = Session::new(config).unwrap();
    let mut key = vec![0];
    key.extend_from_slice(&7i64.to_be_bytes());
    assert!(session.feed(&key).unwrap().is_empty());
    session
}

#[test]
fn a_reconnect_login_block_is_type_18_and_reply_15_resumes_the_session() {
    use rs09_client::session::Event;
    use rs09_protocol::login::{LoginRequest, RsaKey};
    for reconnect in [false, true] {
        let mut session = game_session(reconnect);
        assert_eq!(
            session.take_output().unwrap(),
            rs09_protocol::login::handshake(b"lifecycle")
        );
        // client.gameState == 40 ? p1(18) : p1(16), and p2(Protocol.verifyId).
        let block = session.take_output().unwrap();
        assert_eq!(block[0], if reconnect { 18 } else { 16 });
        let expected = LoginRequest {
            username: b"lifecycle",
            password: b"secret",
            isaac_seed: [1, 2, 0, 7],
            window_mode: 2,
            reconnect,
            packet_count: 9,
            ..Default::default()
        };
        assert_eq!(
            block,
            expected
                .encode(
                    rs09_protocol::profile::RS09_REVISION_530,
                    &RsaKey::reference_public()
                )
                .unwrap()
        );
        let events = session.feed(&[15]).unwrap();
        if reconnect {
            // No player details and no rebuild packet follow reply 15.
            assert_eq!(events, [Event::Reconnected]);
            assert!(session.in_game());
            assert_eq!(
                session.feed(&[153]).unwrap(),
                [Event::Frame(Frame {
                    opcode: 153,
                    payload: vec![]
                })]
            );
        } else {
            assert_eq!(
                events,
                [Event::Rejected {
                    code: 15,
                    detail: None
                }]
            );
            assert!(session.closed());
        }
    }
}

/// `fixtures/headless-lifecycle/oracle.json` holds what the unchanged
/// `Protocol.loop` read-timeout and keep-alive blocks did in a JVM. The
/// cases that start from fresh counters and only tick or receive apply to
/// this client's session as they are: the tick on which the original calls
/// `handleConnectionLost` is the tick on which the session ends, which is
/// what sends the lifecycle to game state 40.
#[test]
fn the_jvm_recorded_read_timeout_ends_the_session_on_the_same_tick() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/headless-lifecycle/oracle.json");
    let oracle: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut replayed = 0;
    for case in oracle["cases"].as_array().unwrap() {
        let ops = case["ops"].as_array().unwrap();
        if case["read"] != 0
            || case["write"] != 0
            || ops.iter().any(|op| op[0] != "T" && op[0] != "R")
        {
            continue;
        }
        replayed += 1;
        let mut session = game_session(false);
        let mut login = vec![2];
        login.extend_from_slice(&[0; 11]);
        login.extend_from_slice(&[162, 0, 0]);
        assert_eq!(session.feed(&login).unwrap().len(), 2);
        while session.take_output().is_some() {}
        let mut sent = String::new();
        for (op, expected) in ops.iter().zip(case["expected"].as_array().unwrap()) {
            if op[0] == "T" {
                for _ in 0..op[1].as_i64().unwrap() {
                    if session.closed() {
                        break;
                    }
                    session.tick().unwrap();
                    while let Some(bytes) = session.take_output() {
                        sent.extend(bytes.iter().map(|byte| format!("{byte:02x}")));
                    }
                }
            } else {
                session.feed(&[153]).unwrap();
            }
            assert_eq!(
                (session.closed(), sent.as_str()),
                (
                    expected[2].as_bool().unwrap(),
                    expected[3].as_str().unwrap()
                ),
                "case {case}"
            );
        }
    }
    assert!(replayed >= 1, "no applicable oracle case");
}
