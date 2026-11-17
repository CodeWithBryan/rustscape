//! JVM differential for the window modes: every expected value in
//! `fixtures/display/oracle.jsonl` was produced by the unchanged RT4
//! `DisplayMode` class (`cargo xtask display-oracle`; `provenance.json` lists
//! the sources, what was copied by hand and what the adapters leave
//! unobserved).
//!
//! Each case starts from the state `GameShell.startApplication` leaves and
//! runs a sequence of calls. After every call the test compares what
//! `rs09_client::display::Display` did, in order, with the calls the Java
//! made on its adapters, then the state and the letterbox.
use rs09_client::{
    client_events::Window,
    display::{Display, Effect, Enter, Mode, System},
    ui::vm::Preferences,
};
use serde_json::Value;

fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn ints(value: &Value) -> Vec<i32> {
    value.as_array().unwrap().iter().map(int).collect()
}

/// The adapter calls `effects` stand for. The window status is the packet
/// `client_events::Window` writes for the display as it is now, which is
/// where every `setWindowMode` that reports has ended.
fn calls(effects: &[Effect], display: &Display, preferences: &Preferences) -> Vec<String> {
    let mut out = Vec::new();
    for effect in effects {
        match *effect {
            Effect::QuitRenderer => out.push("quit".to_owned()),
            Effect::ExitFullScreen => out.push("exitFullScreen".to_owned()),
            Effect::EnterFullScreen {
                width,
                height,
                bit_depth,
            } => out.push(format!("enterFullScreen {bit_depth} {height} {width}")),
            Effect::WritePreferences => out.push("write".to_owned()),
            Effect::ReplaceCanvas => out.push("addCanvas".to_owned()),
            Effect::PlaceCanvas {
                renderer,
                canvas: [width, height],
                left,
                top,
            } => {
                if renderer {
                    out.push(format!("setCanvasSize {width} {height}"));
                }
                out.push(format!("setSize {width} {height}"));
                out.push(format!("setLocation {left} {top}"));
            }
            Effect::ResetContext => out.push("createAndDestroyContext".to_owned()),
            Effect::LoadingText => out.push("loading".to_owned()),
            Effect::StartRenderer { samples } => out.push(format!("init {samples}")),
            Effect::HighDetail => out.push("hd".to_owned()),
            Effect::StandardDetail => out.push("sd".to_owned()),
            Effect::ReloadResources => out.push("reload".to_owned()),
            Effect::Layout => out.push("layout true".to_owned()),
            Effect::WindowStatus => {
                let window = Window {
                    mode: display.window_mode(),
                    frame: display.frame,
                    anti_aliasing: preferences.anti_aliasing,
                };
                // The canvas the packet names is the canvas the display has.
                assert_eq!(window.canvas(), display.canvas);
                let frame = window.status();
                let mut text = format!("status {}", frame.opcode);
                for byte in frame.payload {
                    text.push_str(&format!(" {byte}"));
                }
                out.push(text);
            }
            Effect::Redraw => out.push("reloadPlugins".to_owned()),
            Effect::Letterbox => out.push("letterbox".to_owned()),
        }
    }
    out
}

#[test]
fn the_unchanged_display_mode_class_is_reproduced() {
    let cases: Vec<Value> = include_str!("../../../fixtures/display/oracle.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(cases.len(), 600);
    let (mut steps, mut full_screens, mut fallbacks, mut negative_margins) = (0, 0, 0, 0);
    let mut modes_seen = [0usize; 4];
    for case in &cases {
        let id = &case["id"];
        let modes = case["modes"].as_array().map(|_| {
            ints(&case["modes"])
                .chunks_exact(4)
                .map(|mode| Mode {
                    width: mode[0],
                    height: mode[1],
                    bit_depth: mode[2],
                    refresh_rate: mode[3],
                })
                .collect::<Vec<_>>()
        });
        let system = System {
            full_screen_supported: case["supported"].as_bool().unwrap(),
            modes,
            enter: match int(&case["enter"]) {
                0 => Enter::Frame,
                1 => Enter::NoFrame,
                _ => Enter::Refused,
            },
            renderer_starts: case["starts"].as_bool().unwrap(),
            software: true,
        };
        let window = ints(&case["window"]);
        let mut display = Display::new([window[0], window[1]], system);
        let saved = ints(&case["fullScreen"]);
        let mut preferences = Preferences {
            remembered_mode: int(&case["remembered"]),
            anti_aliasing: int(&case["antiAliasing"]),
            full_screen_width: saved[0],
            full_screen_height: saved[1],
            ..Preferences::default()
        };
        let observe = |display: &Display, preferences: &Preferences, step: &Value, at: &str| {
            assert_eq!(
                display.observation(preferences).to_vec(),
                ints(&step["state"]),
                "case {id} {at}: state"
            );
            let bands: Vec<Vec<i32>> = display
                .letterbox()
                .into_iter()
                .flatten()
                .map(|band| band.to_vec())
                .collect();
            let expected: Vec<Vec<i32>> =
                step["bands"].as_array().unwrap().iter().map(ints).collect();
            assert_eq!(bands, expected, "case {id} {at}: letterbox");
        };
        observe(&display, &preferences, &case["start"], "start");
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let op = step["op"].as_array().unwrap();
            let name = op[0].as_str().unwrap();
            let arguments: Vec<i32> = op[1..].iter().map(int).collect();
            let at = format!("step {index} {name} {arguments:?}");
            let before = display.window_mode();
            let result = match name {
                "set" => {
                    display.set_window_mode(
                        arguments[0] == 1,
                        arguments[1],
                        arguments[2],
                        arguments[3],
                        &mut preferences,
                    );
                    None
                }
                "redraw" => {
                    display.redraw(&mut preferences);
                    None
                }
                "resize" => Some(Value::from(i32::from(
                    display.watch([arguments[0], arguments[1]]),
                ))),
                "leave" => {
                    display.leave_full_screen(&mut preferences);
                    None
                }
                "5300" => Some(Value::from(display.script_full_screen(
                    arguments[0],
                    arguments[1],
                    &mut preferences,
                ))),
                "5307" => {
                    display.script_window_mode(arguments[0], &mut preferences);
                    None
                }
                "modes" => {
                    let listed: Vec<Vec<i32>> = display
                        .display_modes()
                        .iter()
                        .map(|mode| {
                            vec![mode.width, mode.height, mode.bit_depth, mode.refresh_rate]
                        })
                        .collect();
                    // CS2 5303 answers from the same list.
                    for (index, mode) in listed.iter().enumerate() {
                        assert_eq!(
                            display.mode_size(index as i32),
                            Some([mode[0], mode[1]]),
                            "case {id} {at}"
                        );
                    }
                    assert_eq!(display.mode_size(listed.len() as i32), None);
                    Some(serde_json::json!({
                        "modes": listed,
                        "preferred": display.preferred_mode(&preferences),
                    }))
                }
                "flag" => {
                    display.replace_canvas = true;
                    None
                }
                "remember" => {
                    preferences.remembered_mode = arguments[0];
                    None
                }
                "aa" => {
                    preferences.anti_aliasing = arguments[0];
                    None
                }
                other => panic!("unknown step {other}"),
            };
            let effects = std::mem::take(&mut display.effects);
            let log: Vec<String> = step["log"]
                .as_array()
                .unwrap()
                .iter()
                .map(|event| event.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(
                calls(&effects, &display, &preferences),
                log,
                "case {id} {at}: calls"
            );
            assert_eq!(
                result.as_ref(),
                step.get("result"),
                "case {id} {at}: result"
            );
            observe(&display, &preferences, step, &at);
            steps += 1;
            modes_seen[display.window_mode() as usize] += 1;
            full_screens += usize::from(before != 3 && display.window_mode() == 3);
            fallbacks += usize::from(
                effects
                    .iter()
                    .filter(|effect| **effect == Effect::QuitRenderer)
                    .count()
                    > 1,
            );
            negative_margins += usize::from(display.left_margin < 0);
        }
    }
    // The corpus reaches every mode, full screen and both retries.
    assert!(steps > 5000, "{steps}");
    assert!(
        modes_seen.iter().all(|&count| count > 300),
        "{modes_seen:?}"
    );
    assert!(full_screens > 100, "{full_screens}");
    assert!(fallbacks > 100, "{fallbacks}");
    assert!(negative_margins > 100, "{negative_margins}");
}

#[test]
fn this_client_differs_from_the_original_only_in_mode_zero() {
    // The same corpus under `System::software` false: wherever the original
    // ends in a hardware mode the client is in the same state, and wherever
    // it ends in mode 0 the client is in mode 1 on the same canvas. Cases
    // whose hardware renderer fails to start are left out: this client's
    // always starts.
    let mut compared = 0;
    for line in include_str!("../../../fixtures/display/oracle.jsonl").lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        if !case["starts"].as_bool().unwrap() {
            continue;
        }
        let modes = case["modes"].as_array().map(|_| {
            ints(&case["modes"])
                .chunks_exact(4)
                .map(|mode| Mode {
                    width: mode[0],
                    height: mode[1],
                    bit_depth: mode[2],
                    refresh_rate: mode[3],
                })
                .collect::<Vec<_>>()
        });
        let window = ints(&case["window"]);
        let saved = ints(&case["fullScreen"]);
        let mut preferences = Preferences {
            remembered_mode: int(&case["remembered"]),
            anti_aliasing: int(&case["antiAliasing"]),
            full_screen_width: saved[0],
            full_screen_height: saved[1],
            ..Preferences::default()
        };
        let mut display = Display::new(
            [window[0], window[1]],
            System {
                full_screen_supported: case["supported"].as_bool().unwrap(),
                modes,
                enter: match int(&case["enter"]) {
                    0 => Enter::Frame,
                    1 => Enter::NoFrame,
                    _ => Enter::Refused,
                },
                renderer_starts: true,
                software: false,
            },
        );
        // Until the first setWindowMode both are in the start-up state.
        let mut set = false;
        for step in case["steps"].as_array().unwrap() {
            let op = step["op"].as_array().unwrap();
            let a: Vec<i32> = op[1..].iter().map(int).collect();
            match op[0].as_str().unwrap() {
                "set" => display.set_window_mode(a[0] == 1, a[1], a[2], a[3], &mut preferences),
                "redraw" => {
                    display.redraw(&mut preferences);
                }
                "resize" => {
                    display.watch([a[0], a[1]]);
                }
                "leave" => {
                    display.leave_full_screen(&mut preferences);
                }
                "5300" => {
                    display.script_full_screen(a[0], a[1], &mut preferences);
                }
                "5307" => display.script_window_mode(a[0], &mut preferences),
                "flag" => display.replace_canvas = true,
                "remember" => preferences.remembered_mode = a[0],
                "aa" => preferences.anti_aliasing = a[0],
                _ => {}
            }
            set |= display.effects.contains(&Effect::WindowStatus);
            display.effects.clear();
            let expected = ints(&step["state"]);
            let ours = display.observation(&preferences);
            let original_mode = expected[0];
            if set {
                assert_eq!(ours[0], original_mode.max(1), "case {}", case["id"]);
            } else {
                assert_eq!(ours[0], 0);
            }
            // Frame, canvas, margins, full screen, saved size and writes.
            for field in [1, 2, 3, 4, 5, 6, 7, 10, 11, 12] {
                assert_eq!(
                    ours[field], expected[field],
                    "case {} field {field}",
                    case["id"]
                );
            }
            compared += 1;
        }
    }
    assert!(compared > 4000, "{compared}");
}
