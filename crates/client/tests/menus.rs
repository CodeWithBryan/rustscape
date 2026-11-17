//! JVM differential for the hover action text and for cursor definitions.
//!
//! `fixtures/menus/oracle.jsonl` is written by `cargo xtask menus-oracle`. It
//! holds what the unchanged `rt4.Font.renderWavyText` and
//! `renderWavyTextAligned` drew (every glyph and name-icon call, recorded
//! instead of rasterised) over four seeded unkerned fonts, what the unchanged
//! `rt4.CursorType.decode` read, and `java.util.Random` after `setSeed`.
//!
//! The `clock` cases are the exception: `client.mainLoop` cannot be compiled
//! alone, so the probe repeats its seed expression over `java.util.Calendar`.
//! Icon heights and the paragraph padding below are worked out in this file
//! the way `Font`'s constructor and `renderOffset` do, to place the recorded
//! calls; the renderer supplies them at run time.
use rs09_client::{
    cursor::CursorType,
    ui::{
        action_tooltip::{self, Draw, Host, Step},
        font::Metrics,
    },
};
use rs09_game_core::random::JavaRandom;
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../fixtures/menus/oracle.jsonl");

fn cases(kind: &str) -> Vec<Value> {
    FIXTURE
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["case"] == kind)
        .collect()
}
fn int(value: &Value) -> i32 {
    i32::try_from(value.as_i64().unwrap()).unwrap()
}
fn ints(value: &Value) -> Vec<i32> {
    value.as_array().unwrap().iter().map(int).collect()
}
fn bytes(value: &Value) -> Vec<u8> {
    let hex = value.as_str().unwrap();
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}
fn latin1(value: &Value) -> String {
    bytes(value).into_iter().map(char::from).collect()
}

struct Font {
    metrics: Metrics,
    glyph_tops: Vec<i32>,
    icon_heights: Vec<i32>,
    /// `paragraphTopPadding` and `paragraphBottomPadding`.
    padding: [i32; 2],
}
fn fonts() -> Vec<Font> {
    cases("font")
        .iter()
        .map(|font| {
            let mut metrics = Metrics::decode(&bytes(&font["metrics"])).unwrap();
            let icons: Vec<Vec<i32>> = font["icons"].as_array().unwrap().iter().map(ints).collect();
            metrics.name_icons = icons.iter().map(|icon| icon[0]).collect();
            let (tops, heights) = (ints(&font["y"]), ints(&font["heights"]));
            let (mut min_top, mut max_bottom) = (i32::MAX, i32::MIN);
            for (&top, &height) in tops.iter().zip(&heights) {
                if top < min_top && height != 0 {
                    min_top = top;
                }
                max_bottom = max_bottom.max(top + height);
            }
            Font {
                padding: [
                    metrics.line_height - min_top,
                    max_bottom - metrics.line_height,
                ],
                metrics,
                glyph_tops: tops,
                icon_heights: if font["icon_heights"].is_null() {
                    icons.iter().map(|icon| icon[1]).collect()
                } else {
                    ints(&font["icon_heights"])
                },
            }
        })
        .collect()
}

/// The calls `Font.renderOffset` makes for `draw`: for each visible glyph its
/// shadow (when there is one) and then the glyph, both with the line's alpha.
fn expected_calls(font: &Font, draw: &Draw) -> Vec<Vec<i32>> {
    let top = draw.position[1] - font.metrics.line_height;
    let mut color = draw.color;
    let mut calls = Vec::new();
    for step in action_tooltip::steps(&draw.text, &font.metrics, draw.position[0], &draw.offsets) {
        match step {
            // `Font.parseTag` for the tags the probe writes.
            Step::Tag(tag) => {
                if let Some(hex) = tag.strip_prefix("col=") {
                    color = i32::from_str_radix(hex, 16).unwrap();
                } else if tag == "/col" || tag == "br" {
                    color = draw.color;
                }
            }
            Step::Icon { index, x } => calls.push(vec![
                2,
                index as i32,
                x,
                draw.position[1] - font.icon_heights[index],
                0,
                draw.alpha,
            ]),
            Step::Glyph { glyph: 32, .. } => {}
            Step::Glyph { glyph, x, .. } => {
                let y = top + font.glyph_tops[usize::from(glyph)];
                if draw.shadow != -1 {
                    calls.push(vec![
                        1,
                        i32::from(glyph),
                        x + 1,
                        y + 1,
                        draw.shadow,
                        draw.alpha,
                    ]);
                }
                calls.push(vec![1, i32::from(glyph), x, y, color, draw.alpha]);
            }
        }
    }
    calls
}
fn recorded(case: &Value) -> Vec<Vec<i32>> {
    case["calls"].as_array().unwrap().iter().map(ints).collect()
}

#[test]
fn the_generator_after_set_seed_is_java_util_random() {
    let cases = cases("random");
    assert_eq!(cases.len(), 72);
    for case in cases {
        let mut random = JavaRandom::new(case["seed"].as_i64().unwrap());
        let values: Vec<i32> = (0..12).map(|_| random.next_i32()).collect();
        assert_eq!(values, ints(&case["ints"]), "seed {}", case["seed"]);
    }
}

#[test]
fn the_clock_seed_expression_matches_the_calendar() {
    let cases = cases("clock");
    assert_eq!(cases.len(), 32);
    for case in cases {
        let time = ints(&case["time"]);
        assert_eq!(
            action_tooltip::date_seed(time[0], time[1], time[2]),
            int(&case["seed"]),
            "{time:?}"
        );
    }
}

#[test]
fn wavy_text_in_the_view_draws_what_the_original_font_draws() {
    let fonts = fonts();
    let cases = cases("wavy");
    assert_eq!(cases.len(), 400);
    let (mut glyphs, mut icons) = (0, 0);
    for (index, case) in cases.iter().enumerate() {
        let font = &fonts[case["font"].as_u64().unwrap() as usize];
        let text = latin1(&case["text"]);
        let at = ints(&case["at"]);
        let width = font.metrics.width(&text);
        // renderTooltip draws at (x + 4, y + 15) of the view's corner.
        let draw = action_tooltip::in_view(
            text.clone(),
            [at[0] - 4, at[1] - 15],
            width,
            int(&case["seed"]),
        );
        assert_eq!(draw.position, [at[0], at[1]], "case {index}");
        assert!((192..224).contains(&draw.alpha));
        assert_eq!((draw.color, draw.shadow), (0xffffff, 0));
        let extra = int(&case["extra"]);
        assert_eq!(
            draw.bounds,
            [at[0], at[1] - 15, width + extra, 15],
            "case {index}"
        );
        assert_eq!(draw.offsets.len(), text.chars().count());
        let calls = recorded(case);
        glyphs += calls.iter().filter(|call| call[0] == 1).count();
        icons += calls.iter().filter(|call| call[0] == 2).count();
        assert_eq!(expected_calls(font, &draw), calls, "case {index}: {text:?}");
    }
    // The corpus is not vacuous.
    assert!(
        glyphs > 8000 && icons > 100,
        "{glyphs} glyphs, {icons} icons"
    );
}

#[test]
fn wavy_text_in_a_host_component_draws_what_the_original_font_draws() {
    let fonts = fonts();
    let cases = cases("aligned");
    assert_eq!(cases.len(), 400);
    let mut seen = std::collections::BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let font = &fonts[case["font"].as_u64().unwrap() as usize];
        let text = latin1(&case["text"]);
        let rect = ints(&case["rect"]);
        let host = Host {
            rect: [rect[0], rect[1], rect[2], rect[3]],
            font: -1,
            color: int(&case["color"]),
            shadow: int(&case["shadow"]),
            halign: int(&case["halign"]),
            valign: int(&case["valign"]),
        };
        seen.insert((host.halign, host.valign, host.shadow == -1));
        let draw = action_tooltip::hosted(
            text.clone(),
            &host,
            &font.metrics,
            font.padding,
            int(&case["seed"]),
        );
        assert_eq!(draw.bounds.to_vec(), ints(&case["bounds"]), "case {index}");
        assert_eq!(
            action_tooltip::wavy(text.chars().count(), int(&case["seed"])).extra,
            int(&case["extra"])
        );
        assert_eq!(
            expected_calls(font, &draw),
            recorded(case),
            "case {index}: {text:?}"
        );
    }
    // Every alignment pair, with and without a shadow.
    assert_eq!(seen.len(), 32);
}

#[test]
fn cursor_definitions_decode_as_the_original_does() {
    let cases = cases("cursor");
    assert_eq!(cases.len(), 300);
    let mut failures = 0;
    for case in cases {
        let data = bytes(&case["data"]);
        match CursorType::decode(&data) {
            Ok(cursor) => {
                assert!(case["error"].is_null(), "{data:?}");
                assert_eq!(cursor.sprite, int(&case["sprite"]), "{data:?}");
                assert_eq!(cursor.hot_spot.to_vec(), ints(&case["hot"]), "{data:?}");
            }
            // The original faults reading past the end; this port reports it.
            Err(_) => {
                assert_eq!(case["error"], true, "{data:?}");
                failures += 1;
            }
        }
    }
    assert!(failures > 20);
}
