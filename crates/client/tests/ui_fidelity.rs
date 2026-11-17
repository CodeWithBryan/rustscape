//! JVM differential for the interface-fidelity work: every expected value in
//! `fixtures/ui-fidelity/oracle.jsonl` was produced by the unchanged RT4
//! classes (`cargo xtask ui-fidelity-oracle`; see `provenance.json` for the
//! sources and the scope of the adapters).
use rs09_client::{
    pack::Pack,
    ui::{
        Component, HookArg, Properties,
        drag::Controller,
        font::Metrics,
        icon_text::{self, Glyph, SoftFont},
        small_text,
        state::{Key, PointerInput, State},
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn corpus(case: &str) -> Vec<Value> {
    let needle = format!("{{\"case\":\"{case}\"");
    include_str!("../../../fixtures/ui-fidelity/oracle.jsonl")
        .lines()
        .filter(|line| line.starts_with(&needle))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn ints(value: &Value) -> Vec<i32> {
    value.as_array().unwrap().iter().map(int).collect()
}
fn unhex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

/// `JagString.parseHexString(16)` and `parseInt()` over 435 strings.
#[test]
fn numbers_parse_as_the_original_does() {
    let cases = corpus("number");
    assert_eq!(cases.len(), 435);
    let mut failures = 0;
    for case in &cases {
        let text = case["text"].as_str().unwrap();
        for (radix, field) in [(16, "hex"), (10, "decimal")] {
            let expected = case[field].as_i64().map(|v| v as i32);
            failures += usize::from(expected.is_none());
            assert_eq!(
                small_text::parse_radix(text, radix),
                expected,
                "{text:?} radix {radix}"
            );
        }
    }
    // Both outcomes are well represented.
    assert!(failures > 100 && failures < 700, "{failures}");
}

/// `Font.decode` on kerned font files: the line height and the widths of
/// 3,600 short strings, which expose the kerning table through
/// `getStringWidth`.
#[test]
fn kerned_fonts_decode_as_the_original_does() {
    let fonts = corpus("kerning");
    assert_eq!(fonts.len(), 6);
    let mut kerned = 0;
    for font in &fonts {
        let metrics = Metrics::decode(&unhex(font["data"].as_str().unwrap())).unwrap();
        assert_eq!(metrics.line_height, int(&font["lineHeight"]));
        kerned += metrics
            .kerning
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&k| k != 0)
            .count();
        for entry in font["widths"].as_array().unwrap() {
            let text: String = unhex(entry[0].as_str().unwrap())
                .into_iter()
                .map(char::from)
                .collect();
            assert_eq!(metrics.width(&text), int(&entry[1]), "{:?}", entry[0]);
        }
    }
    assert!(kerned > 10000, "{kerned}");
}

/// The stack count `Inv.renderObjectSprite` draws into the 36 by 32 sprite,
/// through the unchanged `Font.render` and `SoftwareFont.renderGlyph`, over
/// a staged picture in place of the item model.
#[test]
fn count_text_is_drawn_into_the_sprite_as_the_original_does() {
    let mut fonts = BTreeMap::new();
    for font in corpus("font") {
        let metrics = Metrics::decode(&unhex(font["data"].as_str().unwrap())).unwrap();
        let glyphs = font["glyphs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| Glyph {
                offset: [int(&g[0]), int(&g[1])],
                size: [int(&g[2]), int(&g[3])],
                mask: unhex(g[4].as_str().unwrap()),
            })
            .collect::<Vec<_>>();
        assert_eq!(glyphs.len(), 256);
        fonts.insert(
            int(&font["id"]),
            SoftFont {
                advances: metrics.advances,
                line_height: metrics.line_height,
                kerning: None,
                glyphs,
            },
        );
    }
    let cases = corpus("countText");
    assert_eq!(cases.len(), 160);
    let mut drawn = 0;
    for case in &cases {
        let [a, b, c] = <[i32; 3]>::try_from(ints(&case["picture"])).unwrap();
        // UiFidelityOracle.picture.
        let mut pixels: Vec<u32> = (0..36 * 32)
            .map(|i: i32| {
                if (i * 7 + c) % 5 == 0 {
                    0
                } else {
                    ((i.wrapping_mul(a).wrapping_add(b)) & 0xffffff) as u32
                }
            })
            .collect();
        let before = pixels.clone();
        let stack = int(&case["stack"]);
        assert_eq!(
            icon_text::format_amount(stack),
            case["text"].as_str().unwrap()
        );
        icon_text::bake(
            &mut pixels,
            &fonts[&int(&case["font"])],
            case["drawText"].as_bool().unwrap(),
            int(&case["stackable"]),
            stack,
        );
        drawn += usize::from(pixels != before);
        let mut digest = Sha256::new();
        for pixel in &pixels {
            digest.update(pixel.to_be_bytes());
        }
        assert_eq!(
            (
                pixels.iter().filter(|&&p| p != 0).count() as i64,
                format!("{:x}", digest.finalize())
            ),
            (
                case["lit"].as_i64().unwrap(),
                case["sha256"].as_str().unwrap().to_owned()
            ),
            "stack {stack} stackable {} text {}",
            case["stackable"],
            case["drawText"]
        );
    }
    assert!(drawn > 80, "{drawn}");
}

const HOOKS: usize = 22;
fn hook(c: &mut Component, index: usize) -> &mut Option<Vec<HookArg>> {
    match index {
        0 => &mut c.on_scroll,
        1 => &mut c.on_click_repeat,
        2 => &mut c.on_drag,
        3 => &mut c.on_release,
        4 => &mut c.on_hold,
        5 => &mut c.on_mouse_over,
        6 => &mut c.on_mouse_repeat,
        7 => &mut c.on_mouse_leave,
        8 => &mut c.on_timer,
        9 => &mut c.on_varc_transmit,
        10 => &mut c.on_varcstr_transmit,
        11 => &mut c.on_varp_transmit,
        12 => &mut c.on_inv_transmit,
        13 => &mut c.on_stat_transmit,
        14 => &mut c.on_msg,
        15 => &mut c.on_friend_transmit,
        16 => &mut c.on_clan_transmit,
        17 => &mut c.on_stock_transmit,
        18 => &mut c.on_misc_transmit,
        19 => &mut c.on_key,
        20 => &mut c.on_minimap_unlock,
        _ => &mut c.on_resize,
    }
}
fn triggers(value: &Value) -> Option<Vec<i32>> {
    (!value.is_null()).then(|| ints(value))
}
fn ring(target: &mut [i32; 32], writer: &mut i32, ids: &Value) {
    for id in ints(ids) {
        target[(*writer & 31) as usize] = id;
        *writer = writer.wrapping_add(1);
    }
}

/// `InterfaceList.layoutTopLevel` and `processComponents` over 260 staged
/// interface trees and 1,100-odd cycles: the three request lists in order,
/// each component's geometry, scroll, mouse flags and transmit readers, the
/// hovered and tooltip components, and the component a click starts
/// dragging.
#[test]
fn the_hook_walk_and_layout_match_the_original() {
    let manifest = br#"{"source":"Interface fidelity differential","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((manifest.len() as u32).to_le_bytes());
    raw.extend(manifest);
    let scenarios = corpus("walk");
    assert_eq!(scenarios.len(), 260);
    let (mut cycles, mut requests, mut drags, mut resizes, mut cancelled) = (0, 0, 0, 0, 0);
    let mut kinds = [0usize; HOOKS + 1];
    for scenario in &scenarios {
        let number = int(&scenario["number"]);
        let mut state = State::new(Pack::decode(raw.clone()).unwrap());
        let canvas = ints(&scenario["canvas"]);
        // The interface opens at this canvas size: no resize is pending.
        state.viewport = [canvas[0], canvas[1]];
        state.root = 100;
        state.transmit_timer = int(&scenario["timer"]);
        let stamps = ints(&scenario["stamps"]);
        state.chat_transmit_at = stamps[0];
        state.friend_transmit_at = stamps[1];
        state.clan_transmit_at = stamps[2];
        state.stock_transmit_at = stamps[3];
        state.misc_transmit_at = stamps[4];
        let writers = ints(&scenario["writers"]);
        state.varc_writer = writers[0];
        state.varcstr_writer = writers[1];
        state.varp_writer = writers[2];
        state.inventory_writer = writers[3];
        state.stat_writer = writers[4];
        state.updated_varcs = ints(&scenario["varcRing"]).try_into().unwrap();
        state.updated_varcstrs = ints(&scenario["varcstrRing"]).try_into().unwrap();
        state.updated_varps = ints(&scenario["varpRing"]).try_into().unwrap();
        state.updated_inventories = ints(&scenario["invRing"]).try_into().unwrap();
        state.updated_stats = ints(&scenario["statRing"]).try_into().unwrap();
        let components = scenario["components"].as_array().unwrap();
        let keys: Vec<Key> = components
            .iter()
            .map(|c| Key {
                id: int(&c["id"]),
                child: int(&c["slot"]),
            })
            .collect();
        for (c, &key) in components.iter().zip(&keys) {
            let (base, modes) = (ints(&c["base"]), ints(&c["modes"]));
            let (aspect, margin) = (ints(&c["aspect"]), ints(&c["margin"]));
            let (scroll, readers) = (ints(&c["scroll"]), ints(&c["readers"]));
            let flags = int(&c["flags"]);
            let mut component = Component {
                id: key.id,
                created_component_id: key.child,
                overlayer: int(&c["layer"]),
                kind: int(&c["type"]),
                if3: c["if3"].as_bool().unwrap(),
                hidden: c["hidden"].as_bool().unwrap(),
                no_click_through: c["noClickThrough"].as_bool().unwrap(),
                client_code: int(&c["code"]),
                base_x: base[0],
                base_y: base[1],
                base_width: base[2],
                base_height: base[3],
                dynamic_width_value: modes[0],
                dynamic_height_value: modes[1],
                x_mode: modes[2],
                y_mode: modes[3],
                aspect_width: aspect[0],
                aspect_height: aspect[1],
                inv_margin_x: margin[0],
                inv_margin_y: margin[1],
                scroll_x: scroll[0],
                scroll_y: scroll[1],
                scroll_max_h: scroll[2],
                scroll_max_v: scroll[3],
                has_event_handlers: c["handlers"].as_bool().unwrap(),
                varc_triggers: triggers(&c["varcTriggers"]),
                varcstr_triggers: triggers(&c["varcstrTriggers"]),
                varp_triggers: triggers(&c["varpTriggers"]),
                inventory_triggers: triggers(&c["invTriggers"]),
                stat_triggers: triggers(&c["statTriggers"]),
                updated_varcs_reader_index: readers[0],
                updated_varcstrs_reader_index: readers[1],
                updated_varps_reader_index: readers[2],
                updated_inventories_reader_index: readers[3],
                updated_stats_reader_index: readers[4],
                last_transmit_timer: readers[5],
                mouse_over: flags & 1 != 0,
                dragging: flags & 2 != 0,
                properties: Properties {
                    events: int(&c["events"]),
                    target_param: -1,
                },
                hover_overlayer: int(&c["hover"]),
                over_color: int(&c["overColor"]),
                ..Component::default()
            };
            for index in 0..HOOKS {
                if int(&c["hooks"]) >> index & 1 != 0 {
                    *hook(&mut component, index) = Some(vec![HookArg::Int(index as i32 + 1)]);
                }
            }
            state.components.insert(key, component);
            state.loaded.insert(key.id >> 16);
            if key.child >= 0 {
                let length = state.created_lengths.entry(key.id).or_default();
                *length = (*length).max(key.child as usize + 1);
            }
            if let Some(events) = c["serverEvents"].as_i64() {
                state.properties.insert(
                    key,
                    Properties {
                        events: events as i32,
                        target_param: -1,
                    },
                );
            }
            if int(&c["dragParent"]) >= 0 {
                state
                    .drag_parents
                    .insert(key, keys[int(&c["dragParent"]) as usize]);
            }
        }
        for open in scenario["open"].as_array().unwrap() {
            state
                .subinterfaces
                .insert(int(&open[0]), (int(&open[1]), int(&open[2])));
        }
        let ordinal = |key: Option<Key>| {
            key.map_or(-1, |key| {
                keys.iter().position(|&k| k == key).unwrap() as i32
            })
        };
        state.layout(canvas[0], canvas[1]).unwrap();
        assert!(state.pending_resize_hooks().is_empty());
        for (cycle, step) in scenario["cycles"].as_array().unwrap().iter().enumerate() {
            let at = format!("scenario {number} cycle {cycle}");
            let change = ints(&step["change"]);
            if change[0] >= 0 {
                let c = state.components.get_mut(&keys[change[0] as usize]).unwrap();
                match change[1] {
                    0 => c.hidden = change[2] != 0,
                    1 => c.scroll_y = change[2],
                    _ => c.base_x = change[2],
                }
            }
            let canvas = ints(&step["canvas"]);
            let nodes = state.layout(canvas[0], canvas[1]).unwrap();
            let forced = step["forced"].as_bool().unwrap();
            if !forced {
                assert!(state.pending_resize_hooks().is_empty(), "{at}");
            }
            resizes += state.pending_resize_hooks().len();
            let mouse = ints(&step["mouse"]);
            for key in step["keys"].as_array().unwrap() {
                state.key_events.push([int(&key[0]), int(&key[1])]);
            }
            ring(
                &mut state.updated_varcs,
                &mut state.varc_writer,
                &step["varc"],
            );
            ring(
                &mut state.updated_varcstrs,
                &mut state.varcstr_writer,
                &step["varcstr"],
            );
            ring(
                &mut state.updated_varps,
                &mut state.varp_writer,
                &step["varp"],
            );
            ring(
                &mut state.updated_inventories,
                &mut state.inventory_writer,
                &step["inv"],
            );
            ring(
                &mut state.updated_stats,
                &mut state.stat_writer,
                &step["stat"],
            );
            let stamped = ints(&step["stamped"]);
            let timer = state.transmit_timer;
            for (flag, stamp) in stamped.iter().zip([
                &mut state.chat_transmit_at,
                &mut state.friend_transmit_at,
                &mut state.clan_transmit_at,
                &mut state.stock_transmit_at,
                &mut state.misc_transmit_at,
            ]) {
                if *flag == 1 {
                    *stamp = timer;
                }
            }
            state.spline_just_finished = mouse[7] != 0;
            let point = [mouse[0], mouse[1]];
            let (menu, inventory) = (mouse[5] != 0, mouse[6] != 0);
            state.queue_input_events(
                &nodes,
                Some(PointerInput {
                    point,
                    clicked: mouse[3] == 1,
                    down: mouse[2] == 1,
                    any_down: mouse[2] != 0,
                    scroll: mouse[4],
                    suppressed: menu || inventory,
                }),
            );
            let actual: Vec<Vec<i32>> = state
                .hooks
                .drain(..)
                .map(|hook| {
                    let Some(HookArg::Int(kind)) = hook.arguments.first() else {
                        panic!("{at}: hook without its marker");
                    };
                    kinds[*kind as usize] += 1;
                    vec![
                        ordinal(Some(hook.source)),
                        *kind,
                        hook.mouse[0],
                        hook.mouse[1],
                        hook.key_code,
                        hook.key_char,
                    ]
                })
                .collect();
            // Timers, then releases and mouse-leaves, then everything else.
            let expected: Vec<Vec<i32>> = ["high", "medium", "low"]
                .into_iter()
                .flat_map(|queue| step[queue].as_array().unwrap().iter().map(ints))
                .collect();
            assert_eq!(actual, expected, "{at} requests");
            requests += actual.len();
            cycles += 1;
            assert_eq!(
                ordinal(state.hovered_key()),
                int(&step["hover"]),
                "{at} hover"
            );
            assert_eq!(
                ordinal(state.tooltip_key()),
                int(&step["tooltip"]),
                "{at} tooltip"
            );
            // The component the click starts dragging, as the walk tracked it
            // and as the drag controller decides it from the draw list.
            let dragged = int(&step["dragged"]);
            assert_eq!(ordinal(state.drag_candidate), dragged, "{at} drag");
            if mouse[3] == 1 && !menu && !inventory {
                let mut controller = Controller::default();
                assert_eq!(
                    controller.begin(&state, &nodes, point, 0, None),
                    dragged >= 0,
                    "{at} drag controller"
                );
                drags += usize::from(dragged >= 0);
            }
            for ((key, expected), described) in keys
                .iter()
                .zip(step["state"].as_array().unwrap())
                .zip(components)
            {
                let c = &state.components[key];
                let actual = vec![
                    i32::from(c.mouse_over) | i32::from(c.dragging) << 1,
                    c.x,
                    c.y,
                    c.width,
                    c.height,
                    c.scroll_x,
                    c.scroll_y,
                    c.updated_varcs_reader_index,
                    c.updated_varcstrs_reader_index,
                    c.updated_varps_reader_index,
                    c.updated_inventories_reader_index,
                    c.updated_stats_reader_index,
                    c.last_transmit_timer,
                ];
                assert_eq!(actual, ints(expected), "{at} component {described}");
            }
            // A cancelled mouse-over is the visible trace of noClickThrough.
            cancelled += usize::from(
                step["low"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|request| int(&request[1]) != 6)
                    && keys.iter().zip(components).any(|(key, c)| {
                        c["noClickThrough"].as_bool().unwrap()
                            && nodes
                                .iter()
                                .any(|n| n.key == *key && n.clip.contains(point))
                    }),
            );
        }
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
    }
    // The corpus exercises every hook, drags, resizes and click blockers.
    assert!(cycles > 1000 && requests > 5000, "{cycles} {requests}");
    assert!(kinds[1..].iter().all(|&count| count > 20), "{kinds:?}");
    assert!(
        drags > 15 && resizes > 30 && cancelled > 40,
        "{drags} {resizes} {cancelled}"
    );
}
