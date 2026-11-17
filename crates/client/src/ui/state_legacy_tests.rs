use super::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) fn empty() -> State {
    let m = br#"{"source":"controlled original legacy UI bindings","files":{}}"#;
    let mut b = b"RS09VIS1".to_vec();
    b.extend((m.len() as u32).to_le_bytes());
    b.extend(m);
    let mut state = State::new(crate::pack::Pack::decode(b).unwrap());
    state.root = 1;
    state.loaded.insert(1);
    state
}

#[test]
fn legacy_scrollbar_uses_updated_child_origin_and_original_wheel_boundaries() {
    let mut state = empty();
    let parent = Key::component(65536);
    let child = Key::component(65537);
    state.replace_component(
        parent,
        super::super::Component {
            id: parent.id,
            kind: 0,
            overlayer: -1,
            base_x: 10,
            base_y: 20,
            base_width: 200,
            base_height: 100,
            scroll_max_v: 300,
            ..Default::default()
        },
    );
    state.replace_component(
        child,
        super::super::Component {
            id: child.id,
            kind: 8,
            overlayer: parent.id,
            base_y: 80,
            base_width: 200,
            base_height: 30,
            ..Default::default()
        },
    );
    let nodes = state.layout(800, 600).unwrap();
    state.queue_input_events(
        &nodes,
        Some(PointerInput {
            point: [20, 101],
            scroll: 1,
            ..Default::default()
        }),
    );
    assert_eq!(state.components[&parent].scroll_y, 45);
    assert_eq!(
        state.tooltip_key(),
        None,
        "child input must use the newly scrolled origin"
    );
    let nodes = state.layout(800, 600).unwrap();
    assert_eq!(nodes[1].rect.y, 55);
    state.queue_input_events(
        &nodes,
        Some(PointerInput {
            point: [20, 120],
            scroll: 1,
            ..Default::default()
        }),
    );
    assert_eq!(
        state.components[&parent].scroll_y, 90,
        "wheel includes the bottom edge outside the hit clip"
    );
    state.queue_input_events(
        &nodes,
        Some(PointerInput {
            point: [226, 100],
            scroll: 1,
            ..Default::default()
        }),
    );
    assert_eq!(
        state.components[&parent].scroll_y, 90,
        "right edge of bar is exclusive"
    );
    state.queue_input_events(
        &nodes,
        Some(PointerInput {
            point: [20, 100],
            scroll: 1,
            suppressed: true,
            ..Default::default()
        }),
    );
    assert_eq!(state.components[&parent].scroll_y, 90);
}

#[test]
fn legacy_scrollbar_accepts_any_button_on_noncontainers_and_clamps_later() {
    let mut state = empty();
    let key = Key::component(65536);
    state.replace_component(
        key,
        super::super::Component {
            id: key.id,
            kind: 3,
            overlayer: -1,
            base_x: 10,
            base_y: 20,
            base_width: 200,
            base_height: 100,
            scroll_max_v: 300,
            ..Default::default()
        },
    );
    let nodes = state.layout(800, 600).unwrap();
    state.queue_input_events(
        &nodes,
        Some(PointerInput {
            point: [211, 21],
            down: false,
            any_down: true,
            ..Default::default()
        }),
    );
    assert_eq!(state.components[&key].scroll_y, -4);
    state.layout(800, 600).unwrap();
    assert_eq!(state.components[&key].scroll_y, 0);
    state.get_mut(key).unwrap().if3 = true;
    state.queue_input_events(
        &nodes,
        Some(PointerInput {
            point: [211, 21],
            any_down: true,
            scroll: 1,
            ..Default::default()
        }),
    );
    assert_eq!(
        state.components[&key].scroll_y, 0,
        "IF3 uses script hooks rather than the legacy handler"
    );
}

#[test]
fn container_chrome_boundary_follows_static_created_and_nested_children() {
    let mut state = empty();
    let root = Key::component(65536);
    let nested = Key::component(65537);
    let text = Key::component(65538);
    let created = Key {
        id: root.id,
        child: 0,
    };
    for (key, parent, kind) in [
        (root, -1, 0),
        (nested, root.id, 0),
        (text, nested.id, 4),
        (created, root.id, 3),
    ] {
        state.replace_component(
            key,
            super::super::Component {
                id: key.id,
                overlayer: parent,
                kind,
                base_width: 100,
                base_height: 100,
                ..Default::default()
            },
        );
    }
    let nodes = state.layout(800, 600).unwrap();
    assert_eq!(
        nodes.iter().map(|n| n.key).collect::<Vec<_>>(),
        vec![root, nested, text, created]
    );
    assert_eq!(
        nodes.iter().map(|n| n.subtree_end).collect::<Vec<_>>(),
        vec![4, 3, 3, 4]
    );
}
#[test]
fn legacy_tooltip_timer_carries_between_widgets_and_keeps_captured_identity() {
    let mut state = empty();
    let a = Key::component(65536);
    let b = Key::component(65537);
    for (key, x) in [(a, 10), (b, 100)] {
        state.replace_component(
            key,
            super::super::Component {
                id: key.id,
                kind: 8,
                overlayer: -1,
                base_x: x,
                base_y: 10,
                base_width: 50,
                base_height: 20,
                ..Default::default()
            },
        );
    }
    let input = |x| {
        Some(PointerInput {
            point: [x, 15],
            ..Default::default()
        })
    };
    for _ in 0..49 {
        let nodes = state.layout(400, 300).unwrap();
        state.queue_input_events(&nodes, input(15));
        state.finish_input_events();
    }
    assert_eq!(state.tooltip_timer(), 49);
    assert!(!state.is_tooltip(a));
    let nodes = state.layout(400, 300).unwrap();
    state.queue_input_events(&nodes, input(105));
    state.finish_input_events();
    assert_eq!(state.tooltip_timer(), 50);
    assert!(state.is_tooltip(b));
    state.queue_input_events(&nodes, input(15));
    state.finish_input_events();
    assert!(state.is_tooltip(a));
    let old = state.components[&a].clone();
    state.replace_component(a, old);
    assert_eq!(state.tooltip_key(), None);
    assert!(!state.is_tooltip(a));
    state.finish_input_events();
    assert_eq!(state.tooltip_timer(), 50);
    state.queue_input_events(
        &nodes,
        Some(PointerInput {
            suppressed: true,
            ..Default::default()
        }),
    );
    state.finish_input_events();
    assert_eq!(state.tooltip_timer(), 49);
    assert_eq!(state.tooltip_key(), None);
    state.queue_input_events(&nodes, input(105));
    state.finish_input_events();
    assert!(state.is_tooltip(b));
    for _ in 0..60 {
        state.queue_input_events(&nodes, None);
        state.finish_input_events();
    }
    assert_eq!(state.tooltip_timer(), 0);
}
#[test]
fn original_static_legacy_hover_clips_hidden_branches_and_reference_replacement_match() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/ui/legacy-widgets.json")).unwrap(),
    )
    .unwrap();
    for (name, hash) in f["sources"].as_object().unwrap() {
        let text = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 1024);
    for (index, row) in f["cases"].as_array().unwrap().iter().enumerate() {
        let mut state = empty();
        for (i, raw) in row["nodes"].as_array().unwrap().iter().enumerate() {
            let a = raw
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_i64().unwrap() as i32)
                .collect::<Vec<_>>();
            let id = 65536 + i as i32;
            state.components.insert(
                Key::component(id),
                super::super::Component {
                    id,
                    overlayer: if a[0] < 0 { -1 } else { 65536 + a[0] },
                    base_x: a[1],
                    base_y: a[2],
                    base_width: a[3],
                    base_height: a[4],
                    kind: a[5],
                    if3: a[6] != 0,
                    hidden: a[7] != 0,
                    hover_overlayer: a[8],
                    over_color: a[9],
                    ..Default::default()
                },
            );
        }
        let nodes = state.layout(400, 300).unwrap();
        state.queue_input_events(
            &nodes,
            Some(PointerInput {
                point: serde_json::from_value(row["point"].clone()).unwrap(),
                suppressed: row["dragged"].as_bool().unwrap()
                    || row["inventoryClicked"].as_bool().unwrap()
                    || row["menuOpen"].as_bool().unwrap(),
                ..Default::default()
            }),
        );
        let replaced = row["replaceAfterInput"].as_i64().unwrap() as i32;
        if replaced >= 0 {
            let k = Key::component(65536 + replaced);
            let c = state.components[&k].clone();
            state.replace_component(k, c);
        }
        let hovered = state.hovered_key().map_or(-1, |k| k.id - 65536);
        let visible = state
            .layout(400, 300)
            .unwrap()
            .iter()
            .map(|n| n.key.id)
            .collect::<Vec<_>>();
        assert_eq!(
            json!([hovered, visible]),
            row["expected"],
            "legacy scenario {index}"
        );
    }
}
