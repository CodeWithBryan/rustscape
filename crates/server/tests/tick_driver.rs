//! world::driver: the parts the world-trace scenarios do not reach, checked
//! against the pinned sources' text and bodies (no JVM corpus covers them):
//! the `Decoders530` table, `PacketProcessor`'s routes, `GameReadEvent.read`
//! framing and its per-tick limit, `PacketWriteQueue`, the phase order with
//! its catch and finally, and `GameWorld.pulse`'s counter. The world-trace
//! comparison is `world_login::tick_driver`.
use rs09_protocol::game::ClientAction;
use rs09_server::{
    pulses,
    world::{
        driver::{
            self, Autosave, Driver, Level, Metric,
            dispatch::{self, Decoded, Handler, Packet, Route, Stop},
            write_queue::{self, WriteQueue},
        },
        repository::{self, LogLevel, Repository, SessionObject},
        social::Decoded as Social,
    },
};
use std::collections::HashMap;

const REFERENCE: &str = "../../reference/server/Server/src/main/core/net/packet";

fn reference(file: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(REFERENCE)
        .join(file);
    std::fs::read_to_string(path).ok()
}

/// `NAME(opcode) {` in declaration order, from the pinned Decoders530.kt.
#[test]
fn the_decoder_table_is_the_pinned_enum() {
    let Some(text) = reference("in/Decoders530.kt") else {
        eprintln!("SKIPPED: the reference checkout is absent");
        return;
    };
    let mut found = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(open) = line.find('(') else { continue };
        let (name, rest) = line.split_at(open);
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        let Some(close) = rest.find(')') else {
            continue;
        };
        let Ok(opcode) = rest[1..close].parse::<u8>() else {
            continue;
        };
        if rest[close + 1..].trim() != "{" {
            continue;
        }
        found.push((opcode, name.to_owned()));
    }
    let table: Vec<(u8, String)> = dispatch::DECODERS
        .iter()
        .map(|(op, name)| (*op, name.to_string()))
        .collect();
    assert_eq!(found, table);
    // Every decoder's opcode has a size in GameReadEvent.PACKET_SIZES.
    for (opcode, name) in dispatch::DECODERS {
        assert_ne!(dispatch::PACKET_SIZES[usize::from(opcode)], -3, "{name}");
    }
}

/// Every `Packet` subclass `PacketProcessor.process` can receive has a route.
#[test]
fn every_packet_type_has_a_route() {
    let classes = [
        ("ItemAction", Route::Native(Handler::ItemAction)),
        ("NpcAction", Route::Native(Handler::NpcAction)),
        ("SceneryAction", Route::Native(Handler::SceneryAction)),
        ("PlayerAction", Route::Native(Handler::PlayerAction)),
        ("GroundItemAction", Route::Native(Handler::GroundItemAction)),
        ("ItemExamine", Route::Native(Handler::Examine)),
        ("SceneryExamine", Route::Native(Handler::Examine)),
        ("NpcExamine", Route::Native(Handler::Examine)),
        ("UseWithNpc", Route::Native(Handler::UseWith)),
        ("UseWithPlayer", Route::Native(Handler::UseWith)),
        ("UseWithItem", Route::Native(Handler::UseWith)),
        ("UseWithScenery", Route::Native(Handler::UseWith)),
        ("UseWithGroundItem", Route::Native(Handler::UseWith)),
        ("IfAction", Route::Native(Handler::IfAction)),
        ("ContinueOption", Route::Native(Handler::ContinueOption)),
        ("CloseIface", Route::Native(Handler::CloseIface)),
        ("JoinClan", Route::Native(Handler::Social)),
        ("SetClanRank", Route::Native(Handler::Social)),
        ("KickFromClan", Route::Native(Handler::Social)),
        ("AddFriend", Route::Native(Handler::Social)),
        ("RemoveFriend", Route::Native(Handler::Social)),
        ("AddIgnore", Route::Native(Handler::Social)),
        ("RemoveIgnore", Route::Native(Handler::Social)),
        ("TrackingFocus", Route::Driver(dispatch::Body::Nothing)),
        ("TrackingCameraPos", Route::Driver(dispatch::Body::Nothing)),
        (
            "TrackingDisplayUpdate",
            Route::Native(Handler::DisplayUpdate),
        ),
        (
            "TrackingAfkTimeout",
            Route::Driver(dispatch::Body::AfkTimeout),
        ),
        ("TrackingMouseClick", Route::Driver(dispatch::Body::Nothing)),
        (
            "ComponentPlayerAction",
            Route::Native(Handler::ComponentUseWith),
        ),
        (
            "ComponentItemAction",
            Route::Native(Handler::ComponentUseWith),
        ),
        (
            "ComponentNpcAction",
            Route::Native(Handler::ComponentUseWith),
        ),
        (
            "ComponentSceneryAction",
            Route::Native(Handler::ComponentUseWith),
        ),
        (
            "ComponentGroundItemAction",
            Route::Native(Handler::ComponentUseWith),
        ),
        (
            "SlotSwitchMultiComponent",
            Route::Native(Handler::SlotSwitch),
        ),
        (
            "SlotSwitchSingleComponent",
            Route::Native(Handler::SlotSwitch),
        ),
        (
            "PacketCountUpdate",
            Route::Driver(dispatch::Body::PacketCount),
        ),
        ("PrivateMessage", Route::Native(Handler::Social)),
        ("ChatMessage", Route::Native(Handler::Social)),
        ("ChatSetting", Route::Native(Handler::Social)),
        ("Command", Route::Native(Handler::Command)),
        ("GESetOfferItem", Route::Native(Handler::GrandExchange)),
        ("TrackFinished", Route::Native(Handler::TrackFinished)),
        ("ReportAbuse", Route::Native(Handler::ReportAbuse)),
        ("WorldspaceWalk", Route::Native(Handler::Walk)),
        ("MinimapWalk", Route::Native(Handler::Walk)),
        ("InteractWalk", Route::Native(Handler::Walk)),
        ("Ping", Route::Driver(dispatch::Body::Ping)),
        ("QuickChat", Route::Native(Handler::QuickChat)),
        ("InputPromptResponse", Route::Native(Handler::InputPrompt)),
        ("PlayerPrefsUpdate", Route::Driver(dispatch::Body::Nothing)),
    ];
    if let Some(text) = reference("in/Packet.kt") {
        // The subclasses of the pinned Packet.kt, less the three the read
        // event never queues.
        let mut declared: Vec<&str> = text
            .lines()
            .filter_map(|l| l.find("class ").map(|i| &l[i + 6..]))
            .map(|l| l.split(['(', ' ', ':', '{']).next().unwrap())
            .filter(|c| !matches!(*c, "Packet" | "NoProcess" | "UnhandledOp" | "DecodingError"))
            .collect();
        declared.sort();
        let mut ours: Vec<&str> = classes.iter().map(|(c, _)| *c).collect();
        ours.sort();
        assert_eq!(declared, ours);
    }
    for (class, expected) in classes {
        let route = dispatch::route(class);
        match expected {
            Route::NotPorted(_) => assert!(
                matches!(route, Route::NotPorted(m) if !m.contains("else branch")),
                "{class}: {route:?}"
            ),
            other => assert_eq!(route, other, "{class}"),
        }
    }
    assert!(matches!(dispatch::route("Unknown"), Route::NotPorted(m) if m.contains("else branch")));
}

#[test]
fn decoding_follows_decoders530() {
    // No decoder: UnhandledOp.
    assert_eq!(dispatch::decode(1usize, 0, &[]), Decoded::UnhandledOp);
    // MAP_REBUILD_STARTED and _FINISHED: NoProcess.
    assert_eq!(dispatch::decode(1usize, 20, &[]), Decoded::NoProcess);
    assert_eq!(dispatch::decode(1usize, 110, &[]), Decoded::NoProcess);
    // PING.
    assert_eq!(
        dispatch::decode(1usize, 93, &[]),
        Decoded::Packet(Packet::Action {
            player: 1,
            opcode: 93,
            action: ClientAction::Ping
        })
    );
    // A social decoder: ADD_FRIEND (a base-37 name).
    let Decoded::Packet(packet) = dispatch::decode(1usize, 120, &[0, 0, 0, 0, 0, 0, 0, 1]) else {
        panic!()
    };
    assert!(matches!(
        packet,
        Packet::Social {
            packet: Social::AddFriend { .. },
            ..
        }
    ));
    assert_eq!(packet.class(), "AddFriend");
    // COMMAND stays undecoded here: world::commands decodes it when the
    // packet is processed.
    let Decoded::Packet(packet) = dispatch::decode(1usize, 44, b"x\0") else {
        panic!()
    };
    assert_eq!(packet.class(), "Command");
    assert_eq!(
        dispatch::route(packet.class()),
        Route::Native(Handler::Command)
    );
    // A short ITEM_ACTION_1: DecodingError.
    assert!(matches!(
        dispatch::decode(1usize, 156, &[1]),
        Decoded::DecodingError(m) if m.starts_with("Error decoding opcode 156/ITEM_ACTION_1: ")
    ));
}

// ---------------------------------------------------------------- a host

#[derive(Default)]
struct Host {
    calls: Vec<String>,
    queue: Option<Vec<u8>>,
    counts: HashMap<usize, [i8; dispatch::OP_COUNT_SLOTS]>,
    invalid: i32,
    ticks: driver::Ticks,
    sent: Vec<String>,
    /// A phase that fails, and how.
    fail: Option<(&'static str, driver::Error)>,
    handled: Vec<Handler>,
}

impl Host {
    fn call(&mut self, name: &str) -> driver::Result<()> {
        self.calls.push(name.to_owned());
        match &self.fail {
            Some((phase, error)) if *phase == name => Err(error.clone()),
            _ => Ok(()),
        }
    }
}

impl repository::Host<usize> for Host {
    type Session = usize;
    type Clan = ();
    type Location = ();
    fn uid(&mut self, p: &usize) -> i32 {
        *p as i32
    }
    fn name(&mut self, p: &usize) -> Option<String> {
        Some(format!("p{p}"))
    }
    fn details_username(&mut self, p: &usize) -> Option<String> {
        Some(format!("p{p}"))
    }
    fn set_index(&mut self, _: &usize, _: i32) {}
    fn is_artificial(&mut self, _: &usize) -> bool {
        false
    }
    fn afk_logout(&mut self, _: &usize) -> Option<bool> {
        None
    }
    fn set_playing(&mut self, _: &usize, _: bool) {}
    fn session(&mut self, p: &usize) -> Option<usize> {
        Some(*p)
    }
    fn ticks(&mut self) -> i32 {
        self.ticks.get()
    }
    fn global_chat_enabled(&mut self) -> bool {
        false
    }
    fn to_lower_case(&mut self, text: &str) -> String {
        text.to_lowercase()
    }
    fn session_active(&mut self, _: &usize) -> bool {
        true
    }
    fn set_session_active(&mut self, _: &usize, _: bool) {}
    fn session_has_key(&mut self, _: &usize) -> bool {
        false
    }
    fn key_cancel(&mut self, _: &usize) -> repository::Result<()> {
        Ok(())
    }
    fn socket_close(&mut self, _: &usize) -> repository::Result<()> {
        Ok(())
    }
    fn session_object(&mut self, s: &usize) -> SessionObject<usize> {
        if *s == 0 {
            SessionObject::Other
        } else {
            SessionObject::Player(*s)
        }
    }
    fn set_session_object(&mut self, _: &usize, _: Option<usize>) {}
    fn session_username(&mut self, _: &usize) -> Option<String> {
        None
    }
    fn player_clear(&mut self, _: &Repository<usize>, p: &usize) -> repository::Result<()> {
        self.calls.push(format!("clear {p}"));
        Ok(())
    }
    fn renderable_players_remove(&mut self, _: &usize) -> repository::Result<bool> {
        unreachable!()
    }
    fn in_combat(&mut self, _: &usize) -> repository::Result<bool> {
        unreachable!()
    }
    fn lifepoints(&mut self, _: &usize) -> repository::Result<i32> {
        unreachable!()
    }
    fn is_dead(&mut self, _: &usize) -> repository::Result<bool> {
        unreachable!()
    }
    fn is_teleporting(&mut self, _: &usize) -> repository::Result<bool> {
        unreachable!()
    }
    fn has_soft_script(&mut self, _: &usize) -> repository::Result<bool> {
        unreachable!()
    }
    fn logout_packet(&mut self, p: &usize) -> repository::Result<()> {
        self.calls.push(format!("logout packet {p}"));
        Ok(())
    }
    fn world_logout_listeners(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn walking_queue_reset(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn player_logout_listeners(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn has_familiar(&mut self, _: &usize) -> repository::Result<bool> {
        unreachable!()
    }
    fn familiar_clear(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn interface_close(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn interface_close_single_tab(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn entity_clear(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn zone_monitor_clear(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn house_leave(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn publish_player_status(&mut self, _: &str, _: i32, _: bool) -> repository::Result<()> {
        unreachable!()
    }
    fn clan(&mut self, _: &usize) -> repository::Result<Option<()>> {
        unreachable!()
    }
    fn world_id(&mut self) -> repository::Result<i32> {
        unreachable!()
    }
    fn clan_owner(&mut self, _: &()) -> repository::Result<Option<String>> {
        unreachable!()
    }
    fn publish_clan_leave(&mut self, _: &str, _: i32, _: &str) -> repository::Result<()> {
        unreachable!()
    }
    fn wealth_update(&mut self, _: &usize, _: bool) -> repository::Result<()> {
        unreachable!()
    }
    fn clan_leave(&mut self, _: &(), _: &usize, _: bool) -> repository::Result<()> {
        unreachable!()
    }
    fn details_save(&mut self, _: &usize) -> repository::Result<()> {
        unreachable!()
    }
    fn execute_sql(
        &mut self,
        _: rs09_server::world::disconnection::SaveTask<usize>,
    ) -> repository::Result<()> {
        unreachable!()
    }
    fn set_thread_name(&mut self, _: &str) {}
    fn save_immediately(&mut self, p: &usize) -> repository::Result<()> {
        self.calls.push(format!("save {p}"));
        Ok(())
    }
    fn log(&mut self, _: LogLevel, message: &str) -> repository::Result<()> {
        self.calls.push(message.to_owned());
        Ok(())
    }
    fn print_stack_trace(&mut self, error: &repository::Error) {
        self.calls.push(format!("print {error:?}"));
    }
    fn is_revenant(&mut self, _: &usize) -> bool {
        false
    }
    fn set_renderable(&mut self, p: &usize, value: bool) {
        self.calls.push(format!("renderable {p} {value}"));
    }
    fn npc_id(&mut self, _: &usize) -> i32 {
        unreachable!()
    }
    fn region_chunk_npcs(&mut self, _: &()) -> repository::Result<Vec<usize>> {
        unreachable!()
    }
    fn npc_at(&mut self, _: &usize, _: &()) -> bool {
        unreachable!()
    }
    fn ge_news_muted(&mut self, _: &usize) -> repository::Result<bool> {
        unreachable!()
    }
    fn send_message(&mut self, _: &usize, _: &str) -> repository::Result<()> {
        unreachable!()
    }
}

impl driver::Host<usize> for Host {
    type Outgoing = String;
    fn clock_millis(&mut self) -> i64 {
        0
    }
    fn telemetry(&mut self, _: Metric, _: i32) {}
    fn metrics_enabled(&mut self) -> bool {
        false
    }
    fn is_playing(&mut self, p: &usize) -> bool {
        *p != 3
    }
    fn driver_log(&mut self, origin: &str, level: Level, message: &str) {
        self.calls.push(format!("{origin} {level:?} {message}"));
    }
    fn stack_trace(&mut self, error: &driver::Error) -> String {
        format!("[{error:?}]")
    }
    fn driver_print_stack_trace(&mut self, error: &driver::Error) {
        self.calls.push(format!("print {error:?}"));
    }
    fn reading_queue(&mut self, _: &usize) -> Option<Vec<u8>> {
        self.queue.clone()
    }
    fn set_reading_queue(&mut self, _: &usize, queue: Option<Vec<u8>>) {
        self.queue = queue;
    }
    fn set_last_ping(&mut self, _: &usize, _: i64) {
        self.calls.push("ping".into());
    }
    fn add_invalid_packet(&mut self, _: &usize) -> i32 {
        self.invalid += 1;
        self.invalid
    }
    fn op_counts(&mut self, p: &usize) -> &mut [i8; dispatch::OP_COUNT_SLOTS] {
        self.counts
            .entry(*p)
            .or_insert([0; dispatch::OP_COUNT_SLOTS])
    }
    fn handle(&mut self, handler: Handler, _: &Packet<usize>) -> driver::Result<()> {
        self.handled.push(handler);
        self.call("handle")
    }
    fn is_administrator(&mut self, _: &usize) -> driver::Result<bool> {
        Ok(false)
    }
    fn set_afk_logout(&mut self, p: &usize, value: bool) {
        self.calls.push(format!("afk {p} {value}"));
    }
    fn packet_count(&mut self, _: &usize, increment: i32) -> driver::Result<i32> {
        self.calls.push(format!("packet count {increment}"));
        Ok(7)
    }
    fn pulse_body(&mut self, _: &pulses::Handle) -> driver::Result<bool> {
        self.call("pulse body")?;
        Ok(true)
    }
    fn pulse_node_active(&mut self, _: pulses::Node) -> driver::Result<bool> {
        Ok(true)
    }
    fn tick_listeners(&mut self) -> driver::Result<()> {
        self.call("listeners")
    }
    fn managers_tick(&mut self) -> driver::Result<()> {
        self.call("managers")
    }
    fn set_active(&mut self, p: &usize, value: bool) -> driver::Result<()> {
        self.calls.push(format!("active {p} {value}"));
        Ok(())
    }
    fn clear_minimap_packet(&mut self, p: &usize) -> Option<String> {
        Some(format!("ClearMinimapFlag {p}"))
    }
    fn npc_tick(&mut self, _: &usize) -> driver::Result<()> {
        unreachable!()
    }
    fn npc_reset(&mut self, _: &usize) -> driver::Result<()> {
        unreachable!()
    }
    fn player_tick(&mut self, p: &usize) -> driver::Result<()> {
        self.call(&format!("tick {p}"))
    }
    fn update_scene_graph_flag(&mut self, _: &usize) -> driver::Result<bool> {
        Ok(true)
    }
    fn set_update_scene_graph_flag(&mut self, p: &usize, value: bool) -> driver::Result<()> {
        self.calls.push(format!("scene flag {p} {value}"));
        Ok(())
    }
    fn update_scene_graph(&mut self, p: &usize) -> driver::Result<()> {
        self.call(&format!("scene {p}"))
    }
    fn render_players(&mut self, p: &usize) -> driver::Result<()> {
        self.call(&format!("players {p}"))
    }
    fn render_npcs(&mut self, p: &usize) -> driver::Result<()> {
        self.call(&format!("npcs {p}"))
    }
    fn render_map_chunks(&mut self, p: &usize) -> driver::Result<()> {
        self.call(&format!("chunks {p}"))
    }
    fn entity_reset(&mut self, p: &usize) -> driver::Result<()> {
        self.call(&format!("entity reset {p}"))
    }
    fn update_render_information(&mut self, p: &usize) -> driver::Result<()> {
        self.call(&format!("render info {p}"))
    }
    fn lifepoints_update(&mut self, _: &usize) -> driver::Result<bool> {
        Ok(true)
    }
    fn set_lifepoints_update(&mut self, p: &usize, value: bool) -> driver::Result<()> {
        self.calls.push(format!("lifepoints update {p} {value}"));
        Ok(())
    }
    fn hitpoints_packet(&mut self, p: &usize) -> Option<String> {
        Some(format!("SkillLevel {p}"))
    }
    fn attribute_bool(&mut self, _: &usize, _: &str, _: bool) -> driver::Result<bool> {
        Ok(true)
    }
    fn remove_attribute(&mut self, p: &usize, key: &str) -> driver::Result<()> {
        self.calls.push(format!("remove {p} {key}"));
        Ok(())
    }
    fn reset_chunks(&mut self) -> driver::Result<()> {
        self.call("reset chunks")
    }
    fn ground_items_pulse(&mut self) -> driver::Result<()> {
        self.call("ground items")
    }
    fn execute_autosave(&mut self, task: Autosave) -> driver::Result<()> {
        self.calls.push(format!("autosave {}", task.ticks));
        Ok(())
    }
    fn send_packet(&mut self, packet: &String) -> driver::Result<()> {
        self.sent.push(packet.clone());
        self.call("send")
    }
}

fn world() -> (Driver<usize, String>, Host) {
    let repository = Repository::new(10, 10).unwrap();
    let driver = Driver::new(repository);
    let host = Host {
        ticks: driver.ticks().clone(),
        ..Host::default()
    };
    (driver, host)
}

#[test]
fn reading_frames_limits_and_queues_as_game_read_event() {
    let (driver, mut host) = world();
    // PING (size 0) twice, then an incomplete ITEM_ACTION_1 (size 8).
    let stop = driver.read(&mut host, &1, &[93, 93, 156, 1, 2]).unwrap();
    assert_eq!(
        stop,
        Stop::Incomplete {
            queued: vec![156, 1, 2]
        }
    );
    assert_eq!(host.queue, Some(vec![156, 1, 2]));
    assert_eq!(driver.packets().lengths(), (2, 0));
    // The next read starts with what was queued.
    driver.read(&mut host, &1, &[3, 4, 5, 6, 7, 8]).unwrap();
    assert_eq!(host.queue, None);
    assert_eq!(driver.packets().lengths(), (3, 0));
    // MAP_REBUILD_STARTED decodes to NoProcess and is not queued.
    driver.read(&mut host, &1, &[20, 0, 0, 0, 0]).unwrap();
    assert_eq!(driver.packets().lengths(), (3, 0));
    // An opcode of unknown size: the invalid count, and the rest is dropped.
    assert_eq!(
        driver.read(&mut host, &1, &[0, 93]).unwrap(),
        Stop::InvalidSize { opcode: 0 }
    );
    assert_eq!(host.invalid, 1);
    // A missing size byte queues the opcode alone and counts as invalid.
    assert_eq!(
        driver.read(&mut host, &1, &[44]).unwrap(),
        Stop::InvalidSize { opcode: 44 }
    );
    assert_eq!(host.queue, Some(vec![44]));
    assert_eq!(host.invalid, 2);
    host.queue = None;
    // A session without a player: every byte is skipped.
    assert_eq!(driver.read(&mut host, &0, &[93, 93]).unwrap(), Stop::End);
    assert_eq!(driver.packets().lengths(), (3, 0));
}

#[test]
fn the_per_tick_limit_is_ten_per_opcode_in_a_wrapping_byte() {
    let (driver, mut host) = world();
    let pings = [93u8; 12];
    assert_eq!(
        driver.read(&mut host, &1, &pings).unwrap(),
        Stop::Limit { opcode: 93 }
    );
    // Ten queued; the eleventh stopped the read and the twelfth was dropped.
    assert_eq!(driver.packets().lengths(), (10, 0));
    assert_eq!(host.counts[&1][93], 11);
    // The byte wraps: at 127 the next packet is refused and leaves -128,
    // which is below the limit again.
    host.counts.get_mut(&1).unwrap()[93] = 127;
    assert_eq!(
        driver.read(&mut host, &1, &[93]).unwrap(),
        Stop::Limit { opcode: 93 }
    );
    assert_eq!(host.counts[&1][93], -128);
    assert_eq!(driver.read(&mut host, &1, &[93]).unwrap(), Stop::End);
    assert_eq!(driver.packets().lengths(), (11, 0));
}

#[test]
fn the_write_queue_sends_five_classes_at_once_and_flushes_the_rest_in_order() {
    struct Writer(Vec<String>, Vec<String>);
    impl write_queue::Host<&'static str> for Writer {
        type Error = driver::Error;
        fn send(&mut self, packet: &&'static str) -> driver::Result<()> {
            self.0.push(packet.to_string());
            match *packet {
                "bad" => Err(driver::Error::Exception("e".into())),
                "worse" => Err(driver::Error::Exception("f".into())),
                "fatal" => Err(driver::Error::Throwable("t".into())),
                _ => Ok(()),
            }
        }
        fn is_exception(&mut self, error: &driver::Error) -> bool {
            error.is_exception()
        }
        fn stack_trace(&mut self, error: &driver::Error) -> String {
            format!("[{error:?}]")
        }
        fn log_error(&mut self, message: &str) {
            self.1.push(message.to_owned());
        }
    }
    let queue = WriteQueue::default();
    let mut w = Writer(Vec::new(), Vec::new());
    assert!(queue.handle(&mut w, "Logout", Some("logout")).unwrap());
    assert!(!queue.handle(&mut w, "SkillLevel", Some("a")).unwrap());
    assert!(!queue.handle(&mut w, "SkillLevel", None).unwrap());
    queue.push(&mut w, "X", Some("bad"));
    queue.push(&mut w, "Y", Some("worse"));
    queue.push(&mut w, "Z", Some("b"));
    assert_eq!(w.0, ["logout"]);
    assert_eq!(w.1, ["SkillLevel tried to queue with a null context!"]);
    queue.flush(&mut w).unwrap();
    assert_eq!(w.0, ["logout", "a", "bad", "worse", "b"]);
    // One writer for the whole flush: the second log carries both traces.
    assert_eq!(
        &w.1[1..],
        [
            "Error flushing packet class core.net.packet.out.X: [Exception(\"e\")]",
            "Error flushing packet class core.net.packet.out.Y: [Exception(\"e\")][Exception(\"f\")]"
        ]
    );
    // A Throwable leaves the flush; the rest stays queued.
    queue.push(&mut w, "A", Some("fatal"));
    queue.push(&mut w, "B", Some("c"));
    assert!(queue.flush(&mut w).is_err());
    assert_eq!(queue.snapshot()[1].len(), 1);
    // Nothing queued: nothing swapped.
    queue.flush(&mut w).unwrap();
    assert_eq!(queue.snapshot()[1].len(), 1);
}

#[test]
fn a_tick_runs_the_phases_in_order_and_flushes_in_finally() {
    let (driver, mut host) = world();
    driver.renderable_players().add(4);
    driver
        .pulser()
        .submit(Some(rs09_server::definitions::item::Shared::new(
            pulses::Pulse::new(1, None, pulses::Class::Other, "P".into()),
        )))
        .unwrap();
    driver.tick(&mut host, false).unwrap();
    assert_eq!(
        host.calls,
        [
            "pulse body",
            "listeners",
            "active 4 true",
            "renderable 4 true",
            "reset chunks",
            "ground items",
            "managers",
        ]
    );
    assert_eq!(driver.ticks().get(), 1);
    // The second tick: the player is renderable now.
    host.calls.clear();
    driver.tick(&mut host, false).unwrap();
    assert_eq!(
        host.calls,
        [
            "listeners",
            "tick 4",
            "scene 4",
            "players 4",
            "npcs 4",
            "chunks 4",
            "entity reset 4",
            "scene flag 4 false",
            "render info 4",
            "lifepoints update 4 false",
            "save 4",
            "remove 4 flagged-for-save",
            "reset chunks",
            "ground items",
            "managers",
            "send",
        ]
    );
    assert_eq!(host.sent, ["SkillLevel 4"]);
    // An Exception in a phase is printed, the rest of the phases skipped
    // (the counter too), and the flush still runs.
    host.calls.clear();
    host.sent.clear();
    host.fail = Some(("players 4", driver::Error::Exception("x".into())));
    driver
        .send(&mut host, "SkillLevel", Some("q".into()))
        .unwrap();
    driver.tick(&mut host, false).unwrap();
    assert!(host.calls.iter().any(|c| c.starts_with("print Exception")));
    assert!(!host.calls.iter().any(|c| c == "managers"));
    assert_eq!(host.calls.last().unwrap(), "send");
    assert_eq!(driver.ticks().get(), 2);
    // A Throwable escapes, after the flush.
    host.calls.clear();
    host.fail = Some(("listeners", driver::Error::Throwable("t".into())));
    driver
        .send(&mut host, "SkillLevel", Some("r".into()))
        .unwrap();
    assert_eq!(
        driver.tick(&mut host, false),
        Err(driver::Error::Throwable("t".into()))
    );
    assert_eq!(host.calls.last().unwrap(), "send");
}

#[test]
fn game_world_pulse_counts_ticks_and_hands_off_the_autosave_every_fifty() {
    let (driver, mut host) = world();
    driver.ticks().set(48);
    driver.tick(&mut host, true).unwrap();
    assert!(!host.calls.iter().any(|c| c.starts_with("autosave")));
    driver.tick(&mut host, true).unwrap();
    assert!(host.calls.iter().any(|c| c == "autosave 50"));
    // The counter wraps as Kotlin's Int does; -2147483648 % 50 != 0.
    host.calls.clear();
    driver.ticks().set(i32::MAX);
    driver.tick(&mut host, true).unwrap();
    assert_eq!(driver.ticks().get(), i32::MIN);
    assert!(!host.calls.iter().any(|c| c.starts_with("autosave")));
    // The block saves every playing, non-artificial player in slot order.
    let repository = driver.repository().clone();
    for p in [1, 3, 2] {
        repository.players().add(p);
    }
    host.calls.clear();
    Autosave { ticks: 50 }.run(&mut host, &repository).unwrap();
    assert_eq!(host.calls, ["save 1", "save 2"]);
}

#[test]
fn packets_dispatch_by_route_and_refusals_are_logged_not_run() {
    let (driver, mut host) = world();
    // Ticks 1 and 2: the disconnection queue the AFK logout fills is not
    // looked at (only when the counter is a multiple of three).
    driver.ticks().set(1);
    // WORLDSPACE_WALK (size -1), COMMAND (undecoded, routed to the command
    // handler), PACKET_COUNT, AFK_TIMEOUT.
    let mut bytes = vec![215, 5, 0, 0, 0, 0, 0];
    bytes.extend([44, 2, b'x', 0]);
    bytes.extend([177, 0, 9]);
    bytes.extend([245]);
    driver.read(&mut host, &1, &bytes).unwrap();
    host.calls.clear();
    driver.tick(&mut host, true).unwrap();
    assert_eq!(host.handled, [Handler::Walk, Handler::Command]);
    assert!(driver.refusals().is_empty());
    assert!(!host.calls.iter().any(|c| c.contains("Not ported")));
    // PacketCountUpdate: getPacketCount(count - getPacketCount(0)).
    assert!(host.calls.iter().any(|c| c == "packet count 0"));
    assert!(host.calls.iter().any(|c| c == "packet count 2"));
    // AFK: the flag, then sendLogout (the queue, then the packet).
    let afk = host.calls.iter().position(|c| c == "afk 1 true").unwrap();
    assert_eq!(host.calls[afk + 1], "Queueing p1 for disconnection.");
    assert_eq!(host.calls[afk + 2], "logout packet 1");
    // A handler's Exception is logged with the accumulated trace and the
    // queue goes on.
    host.fail = Some(("handle", driver::Error::Exception("w".into())));
    driver
        .read(
            &mut host,
            &1,
            &[215, 5, 0, 0, 0, 0, 0, 215, 5, 0, 0, 0, 0, 0],
        )
        .unwrap();
    host.calls.clear();
    driver.tick(&mut host, true).unwrap();
    let logged: Vec<&String> = host
        .calls
        .iter()
        .filter(|c| c.starts_with("PacketProcessor Err"))
        .collect();
    assert_eq!(
        logged,
        [
            "PacketProcessor Err Error Processing WorldspaceWalk: [Exception(\"w\")]",
            "PacketProcessor Err Error Processing WorldspaceWalk: [Exception(\"w\")][Exception(\"w\")]"
        ]
    );
}
