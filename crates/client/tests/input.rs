//! Keyboard and mouse input against the original client.
//!
//! `keyboard.txt` and `mouse.txt` under `fixtures/input` are JVM
//! observations: `cargo xtask input-oracle` drives the unchanged
//! `rt4.Keyboard` and `rt4.Mouse` with real AWT event objects and records the
//! complete static state after every call. The first two tests replay those
//! calls here and compare the state line by line.
//!
//! The gameframe test is not a JVM comparison: it checks, on the pinned
//! interface pack, that an F-key press travels from a window-system key
//! event to the button packet of the tab bound to it.
use macroquad::miniquad::KeyCode;
use rs09_client::{
    input::{
        Input,
        keyboard::{Keyboard, Vendor},
        mouse::{Held, Mouse},
        platform::{Host, Translator},
    },
    pack::Pack,
    ui::{
        state::{Key, KeyInput, State},
        vm::Vm,
    },
};
use sha2::{Digest, Sha256};

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(root().join("fixtures/input").join(name))
        .unwrap()
        .replace('\r', "")
}

#[test]
fn fixtures_come_from_the_pinned_sources() {
    let provenance: serde_json::Value = serde_json::from_str(&fixture("provenance.json")).unwrap();
    let sources = provenance["sources"].as_object().unwrap();
    assert_eq!(sources.len(), 5);
    for (path, expected) in sources {
        let text = std::fs::read(root().join(path)).unwrap();
        let text: Vec<u8> = text.into_iter().filter(|&byte| byte != b'\r').collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(&text)),
            expected.as_str().unwrap(),
            "{path} changed: run cargo xtask input-oracle --check"
        );
    }
    for (name, key) in [
        ("keyboard.txt", "keyboardSha256"),
        ("mouse.txt", "mouseSha256"),
    ] {
        assert_eq!(
            format!("{:x}", Sha256::digest(fixture(name).as_bytes())),
            provenance[key].as_str().unwrap(),
            "{name}"
        );
    }
}

fn pressed_hex(keyboard: &Keyboard) -> String {
    (0..28)
        .rev()
        .map(|nibble| {
            let value = (0..4)
                .filter(|bit| keyboard.pressed_keys[nibble * 4 + bit])
                .fold(0, |value, bit| value | 1 << bit);
            char::from_digit(value, 16).unwrap()
        })
        .collect()
}

#[test]
fn keyboard_matches_the_original_class() {
    let text = fixture("keyboard.txt");
    let mut keyboard = Keyboard::default();
    let (mut scenarios, mut compared) = (0, 0);
    let mut vendors = std::collections::BTreeSet::new();
    let mut filled = [false; 2];
    for (number, line) in text.lines().enumerate() {
        let fields: Vec<&str> = line.split(' ').collect();
        let int = |index: usize| fields[index].parse::<i32>().unwrap();
        let context = || format!("line {}: {line}", number + 1);
        match fields[0] {
            "S" => {
                let vendor = [Vendor::Modern, Vendor::Legacy, Vendor::Microsoft][int(1) as usize];
                keyboard = Keyboard::new(vendor);
                vendors.insert(int(1));
                scenarios += 1;
                continue;
            }
            "M" => {
                let map: Vec<i32> = fields[1..].iter().map(|s| s.parse().unwrap()).collect();
                assert_eq!(keyboard.code_map().as_slice(), map, "{}", context());
                continue;
            }
            _ => {}
        }
        let before = keyboard.observation();
        let result = match fields[0] {
            "P" => {
                keyboard.key_pressed(int(1));
                -1
            }
            "R" => {
                keyboard.key_released(int(1));
                -1
            }
            "T" => {
                keyboard.key_typed(int(1));
                -1
            }
            "F" => {
                keyboard.focus_lost();
                -1
            }
            // focusGained does nothing.
            "G" => -1,
            "L" => {
                keyboard.cycle();
                -1
            }
            "N" => i32::from(keyboard.next_key()),
            "X" => {
                keyboard.stop();
                -1
            }
            "Q" => {
                keyboard.quit();
                -1
            }
            other => panic!("unknown command {other}"),
        };
        let [ewi, eri, twi, tri, previous, idle, hash] = keyboard.observation();
        let actual = format!(
            "{} {} {result} {} {ewi} {eri} {twi} {tri} {previous} {idle} {} {} {hash}",
            fields[0],
            fields[1],
            pressed_hex(&keyboard),
            keyboard.key_code,
            keyboard.key_char
        );
        assert_eq!(actual, line, "{}", context());
        // The event ring wrapped onto its reader, or a typed entry was
        // refused because its ring was full.
        filled[0] |= before[0] >= 0 && ewi == -1 && matches!(fields[0], "P" | "R");
        filled[1] |= fields[0] == "T" && (1..256).contains(&int(1)) && before[2] == twi;
        compared += 1;
    }
    assert_eq!(scenarios, 18);
    assert_eq!(vendors.len(), 3);
    assert!(compared > 17_000, "{compared}");
    assert!(filled[0] && filled[1], "both rings overflow in the corpus");
}

#[test]
fn mouse_matches_the_original_class() {
    let text = fixture("mouse.txt");
    let mut mouse = Mouse::default();
    let (mut scenarios, mut compared, mut swallowed) = (0, 0, 0);
    for (number, line) in text.lines().enumerate() {
        let fields: Vec<i64> = line
            .split(' ')
            .skip(1)
            .map(|field| field.parse().unwrap())
            .collect();
        let [x, y, button, mask] = [0, 1, 2, 3].map(|index| fields[index] as i32);
        let held = Held {
            left: mask & 1 != 0,
            middle: mask & 2 != 0,
            right: mask & 4 != 0,
        };
        let pending = mouse.pending_click_button;
        match &line[..1] {
            "S" => {
                mouse = Mouse::default();
                scenarios += 1;
            }
            "M" => mouse.mouse_moved(x, y),
            "D" => mouse.mouse_dragged(x, y, held),
            "P" => {
                mouse.mouse_pressed(x, y, button == 2, held, fields[4]);
                swallowed += usize::from(mouse.pending_click_button == pending && pending == 0);
            }
            "U" => mouse.mouse_released(),
            // mouseClicked and focusGained do nothing.
            "C" | "G" => {}
            "E" => mouse.mouse_entered(x, y),
            "X" => mouse.mouse_exited(),
            "F" => mouse.focus_lost(),
            "L" => mouse.cycle(),
            "I" => mouse.set_idle_loops(x),
            "T" => mouse.stop(),
            "Q" => mouse.quit(),
            other => panic!("unknown command {other}"),
        }
        assert_eq!(
            mouse.observation().as_slice(),
            &fields[5..],
            "line {}: {line}",
            number + 1
        );
        compared += 1;
    }
    assert_eq!(scenarios, 12);
    assert!(compared > 4_000, "{compared}");
    assert!(swallowed > 50, "middle-button presses are in the corpus");
}

/// The main loop's own mouse packets over the JVM's mouse trace.
///
/// `mouse.txt` is replayed into the `Mouse` port as in the test above, and
/// after every recorded `Mouse.loop` one cycle of `client_events` runs on
/// that port, with the mouse recorder sampling before it. The click report
/// (opcode 75) and the recorder packet (opcode 123) it writes must be the
/// ones computed from the statics the JVM recorded on that line
/// (`clickButton`, `clickX`, `clickY`, `clickTime`, `lastMouseX`,
/// `lastMouseY`). So the JVM supplies what `Protocol.loop` reads; that
/// `Protocol.loop` reads those statics, and the encoders, are transcribed
/// (Protocol.java:2426-2529, MouseRecorder.java:33-46).
#[test]
fn the_main_loops_mouse_packets_follow_the_original_mouse_trace() {
    use rs09_client::client_events::{Client, ClientEvents, Shared, Window};
    use rs09_protocol::game::outgoing::{self, MouseRecorder};
    let window = Window {
        mode: 2,
        frame: [765, 503],
        anti_aliasing: 0,
    };
    let client = Client {
        scene_ready: true,
        ..Client::default()
    };
    let text = fixture("mouse.txt");
    let mut devices = Input::default();
    let mut events = ClientEvents::default();
    let mut reference = MouseRecorder::default();
    reference.enabled = true;
    let (mut timer, mut sent, mut now) = (0, true, 0i64);
    let mut reference_prev_click = 0i64;
    let (mut loops, mut clicks, mut recorded, mut outside) = (0, 0, 0, 0);
    for (number, line) in text.lines().enumerate() {
        let fields: Vec<i64> = line
            .split(' ')
            .skip(1)
            .map(|field| field.parse().unwrap())
            .collect();
        let [x, y, button, mask] = [0, 1, 2, 3].map(|index| fields[index] as i32);
        let held = Held {
            left: mask & 1 != 0,
            middle: mask & 2 != 0,
            right: mask & 4 != 0,
        };
        let mut shared = Shared {
            verify_id: 0,
            preferences_sent: &mut sent,
            logout_timer: &mut timer,
        };
        let mouse = &mut devices.mouse;
        match &line[..1] {
            "S" => {
                *mouse = Mouse::default();
                // A new session with the recorder on; its window status,
                // rebuild frames and first camera report go out before the
                // trace begins.
                events = ClientEvents::default();
                events.session_start(true, &window, &mut 0, &mut devices);
                reference = MouseRecorder::default();
                reference.enabled = true;
                reference_prev_click = 0;
                for _ in 0..2 {
                    events.cycle(&mut devices, &client, &window, &mut shared);
                }
                continue;
            }
            "M" => mouse.mouse_moved(x, y),
            "D" => mouse.mouse_dragged(x, y, held),
            "P" => mouse.mouse_pressed(x, y, button == 2, held, fields[4]),
            "U" => mouse.mouse_released(),
            "C" | "G" => {}
            "E" => mouse.mouse_entered(x, y),
            "X" => mouse.mouse_exited(),
            "F" => mouse.focus_lost(),
            "I" => mouse.set_idle_loops(x),
            "T" => mouse.stop(),
            "Q" => mouse.quit(),
            "L" => {
                mouse.cycle();
                // The JVM's statics after this Mouse.loop.
                let jvm = &fields[5..];
                assert_eq!(mouse.observation().as_slice(), jvm, "line {}", number + 1);
                let [click_button, click_x, click_y] = [0, 1, 2].map(|index| jvm[index] as i32);
                let [last_x, last_y] = [5, 6].map(|index| jvm[index] as i32);
                now += 50;
                events.sample_mouse(now, &devices);
                reference.sample(last_x, last_y);
                outside += usize::from([last_x, last_y] == [-1, -1]);
                let expected: Vec<_> = reference
                    .flush(click_button != 0)
                    .into_iter()
                    .chain(outgoing::mouse_click_report(
                        click_button,
                        click_x,
                        click_y,
                        jvm[3],
                        &mut reference_prev_click,
                    ))
                    .collect();
                let cycle = events.cycle(&mut devices, &client, &window, &mut shared);
                assert_eq!(cycle.before, expected, "line {}: {line}", number + 1);
                assert!(cycle.playing && cycle.after.is_empty() && cycle.interface.is_empty());
                loops += 1;
                clicks += usize::from(click_button != 0);
                recorded += expected.iter().filter(|f| f.opcode == 123).count();
            }
            other => panic!("unknown command {other}"),
        }
    }
    assert!(loops > 700, "{loops}");
    assert!(clicks > 100, "{clicks}");
    assert!(recorded > 100, "{recorded}");
    assert!(outside > 20, "{outside}");
}

/// Button clicks as `(component, op)` and packets as `(opcode, payload)`.
type Clicks = Vec<(Key, i32)>;
type Packets = Vec<(i32, Vec<u8>)>;

/// Press a key as the window system reports it, run one cycle and the key
/// bindings, and return the clicks and the packets they produced.
fn press(
    translator: &mut Translator,
    input: &mut Input,
    state: &mut State,
    vm: &mut Vm,
    cycle: i32,
    key: Option<KeyCode>,
) -> (Clicks, Packets) {
    if let Some(key) = key {
        translator.key_down(input, key);
    }
    input.cycle();
    state.layout(765, 503).unwrap();
    let mut clicks = Vec::new();
    vm.outgoing.clear();
    // The interface walk itself: bindings click at their place in it.
    state.queue_cycle_events(
        None,
        Some(KeyInput {
            pressed: &input.keyboard.pressed_keys,
            cycle,
            click: &mut |state, key, op| {
                clicks.push((key, op));
                vm.click_button(state, key, op, "").unwrap();
            },
        }),
    );
    // The requests the walk queued are not this test's subject.
    state.hooks.clear();
    let packets = vm
        .outgoing
        .iter()
        .map(|frame| (i32::from(frame.opcode), frame.payload.clone()))
        .collect();
    (clicks, packets)
}

#[test]
fn function_keys_click_the_gameframe_tabs() {
    let path = root().join("assets/visual/ui.pack");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!(
            "skipped: {} is not present, nothing was checked",
            path.display()
        );
        return;
    };
    // Fixed (548) and resizable (746) gameframes: the tab component bound to
    // each of F1 to F8, read from the pinned pack with the ui_inspect example.
    for (group, tabs) in [
        (548, [38, 39, 40, 41, 42, 43, 44, 21]),
        (746, [41, 42, 43, 44, 45, 46, 47, 49]),
    ] {
        let mut state = State::new(Pack::decode(bytes.clone()).unwrap());
        state.open_root(group).unwrap();
        let mut vm = Vm::default();
        let mut input = Input::default();
        let mut translator = Translator::new(Host::Native);
        let keys = [
            KeyCode::F1,
            KeyCode::F2,
            KeyCode::F3,
            KeyCode::F4,
            KeyCode::F5,
            KeyCode::F6,
            KeyCode::F7,
            KeyCode::F8,
        ];
        let mut cycle = 100;
        let none = (Vec::new(), Vec::new());
        assert_eq!(
            press(
                &mut translator,
                &mut input,
                &mut state,
                &mut vm,
                cycle,
                None
            ),
            none
        );
        for (key, tab) in keys.into_iter().zip(tabs) {
            let id = group << 16 | tab;
            let component = state.get(Key::component(id)).unwrap();
            assert_eq!(
                component.key_codes.as_ref().unwrap()[0],
                input.keyboard.code_map()[rs09_client::input::platform::virtual_key(key) as usize],
                "{group}:{tab}"
            );
            cycle += 1;
            let (clicks, packets) = press(
                &mut translator,
                &mut input,
                &mut state,
                &mut vm,
                cycle,
                Some(key),
            );
            // Op 1 on the tab: IF_BUTTON1 (155) with the component and no slot.
            assert_eq!(clicks, [(Key::component(id), 1)], "{group}:{tab}");
            let mut payload = id.to_be_bytes().to_vec();
            payload.extend([0xff, 0xff]);
            assert_eq!(packets, [(155, payload)], "{group}:{tab}");
            // Held, with the system's auto-repeat: the binding's delay is 0,
            // so nothing more until the key is released and pressed again.
            for _ in 0..60 {
                cycle += 1;
                assert_eq!(
                    press(
                        &mut translator,
                        &mut input,
                        &mut state,
                        &mut vm,
                        cycle,
                        Some(key)
                    ),
                    none
                );
            }
            translator.key_up(&mut input, key);
            cycle += 1;
            assert_eq!(
                press(
                    &mut translator,
                    &mut input,
                    &mut state,
                    &mut vm,
                    cycle,
                    None
                ),
                none
            );
            cycle += 1;
            let (clicks, _) = press(
                &mut translator,
                &mut input,
                &mut state,
                &mut vm,
                cycle,
                Some(key),
            );
            assert_eq!(clicks, [(Key::component(id), 1)]);
            translator.key_up(&mut input, key);
        }
        // F9 and letters are bound to nothing in the gameframe.
        for key in [KeyCode::F9, KeyCode::F10, KeyCode::A, KeyCode::Escape] {
            cycle += 1;
            assert_eq!(
                press(
                    &mut translator,
                    &mut input,
                    &mut state,
                    &mut vm,
                    cycle,
                    Some(key)
                ),
                none
            );
            translator.key_up(&mut input, key);
        }
        let unsupported: Vec<_> = state
            .diagnostics
            .iter()
            .filter(|text| text.contains("Unsupported"))
            .collect();
        assert!(unsupported.is_empty(), "{unsupported:?}");
    }
}
