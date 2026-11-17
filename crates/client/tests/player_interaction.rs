use rs09_client::{
    interaction::{Menu, MenuEntry},
    player_interaction::{self, Context, Options, Player},
    ui::{actions::SelectedItem, targeting::Targeting},
};
use rs09_protocol::game::{Frame, players::RawAppearance};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn corpus() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/ui/player-interaction.json")).unwrap()
}
fn hex(bytes: impl IntoIterator<Item = u8>) -> String {
    bytes.into_iter().map(|n| format!("{n:02x}")).collect()
}
fn text_hex(s: &str) -> String {
    hex(s.chars().map(|c| c as u8))
}
fn bytes(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn labels(values: Option<&[Option<String>]>) -> String {
    values
        .map(|values| {
            values
                .iter()
                .map(|s| s.as_deref().map(text_hex).unwrap_or_else(|| "-".into()))
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_else(|| "~".into())
}
fn appearance(name37: i64, title: i32) -> RawAppearance {
    RawAppearance {
        flags: title << 6,
        skull_icon: -1,
        prayer_icon: -1,
        slots: [0; 12],
        npc_id: -1,
        npc_team: 0,
        colors: [0; 5],
        bas: 1426,
        name37,
        combat: 3,
        skill: 0,
        combat_with_summoning: 3,
        combat_range: -1,
        sound_radius: 0,
        sounds: None,
        consumed: 0,
    }
}

#[test]
fn original_player_sources_remain_pinned() {
    let f = corpus();
    for (name, expected) in f["sources"].as_object().unwrap() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../reference/client/client/src/main/java/rt4/{name}.java"
        ));
        let data = std::fs::read(path)
            .unwrap()
            .into_iter()
            .filter(|&b| b != b'\r')
            .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            expected.as_str().unwrap(),
            "{name}"
        );
    }
}
#[test]
fn original_player_priority_target_masks_and_walk_labels_match() {
    for c in corpus()["menus"].as_array().unwrap() {
        let a: Vec<i32> = serde_json::from_value(c["args"].clone()).unwrap();
        let actor = |offset: usize| Player {
            name: "Sir Test 7",
            combat: a[offset],
            summoning: a[offset + 1],
            range: a[offset + 2],
            skill: a[offset + 3],
            team: a[offset + 4],
        };
        let mut menu = Menu::default();
        for i in 0..a[16] {
            menu.add(MenuEntry {
                option: if i == 0 { "Walk here" } else { "seed" }.into(),
                target: String::new(),
                action: if i == 0 { 60 } else { 1005 },
                key: 0,
                args: [0; 2],
                cursor: -1,
            });
        }
        let options = Options {
            labels: std::array::from_fn(|i| {
                (a[11] >> i & 1 != 0).then(|| {
                    if i == 0 {
                        "aTtAcK".into()
                    } else {
                        format!("Option{i}")
                    }
                })
            }),
            cursors: std::array::from_fn(|i| i as i32 * 13 - 1),
            secondary: std::array::from_fn(|i| a[12] >> i & 1 != 0),
            ..Options::default()
        };
        let selected = SelectedItem {
            id: 1277,
            slot: 27,
            component: 149 << 16,
            text: "Seed".into(),
        };
        let targeting = Targeting {
            active: a[14] != 0,
            mask: a[15],
            param: 999,
            cursor: 37,
            verb: "Cast".into(),
            text: "Spell".into(),
            ..Targeting::default()
        };
        player_interaction::entries(
            &mut menu,
            &options,
            Context {
                game: a[0],
                local: actor(1),
                selected: (a[13] == 1).then_some(&selected),
                targeting: &targeting,
                use_cursor: 29,
            },
            &actor(6),
            321,
            [27, 43],
            a[17] != 0,
        );
        let actual = menu
            .entries
            .iter()
            .filter(|e| e.option != "seed")
            .map(|e| {
                format!(
                    "{}:{}:{}:{}:{}:{}:{}",
                    e.action,
                    e.key,
                    e.args[0],
                    e.args[1],
                    e.cursor,
                    text_hex(&e.option),
                    text_hex(&e.target)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(actual, c["expected"].as_str().unwrap(), "{c}");
    }
}
#[test]
fn all_player_actions_item_and_spell_bytes_match_original() {
    let selected = SelectedItem {
        id: 1277,
        slot: 27,
        component: 149 << 16,
        text: String::new(),
    };
    let targeting = Targeting {
        source: rs09_client::ui::state::Key {
            id: 0x12345678,
            child: -1,
        },
        ..Targeting::default()
    };
    for c in corpus()["wires"].as_array().unwrap() {
        let action = c["action"].as_i64().unwrap() as i32;
        let index = c["index"].as_i64().unwrap() as i32;
        let packet = match action {
            1 => selected.on_player(index),
            15 => targeting.packet(15, index, [0; 2]).unwrap(),
            _ => player_interaction::packet(action, index).unwrap(),
        };
        assert_eq!(
            hex(std::iter::once(packet.opcode).chain(packet.payload)),
            c["expected"].as_str().unwrap(),
            "{c}"
        );
        if player_interaction::ACTIONS.contains(&action) {
            assert_eq!(
                player_interaction::packet(action, index),
                player_interaction::packet(action + 2000, index)
            );
        }
    }
}
#[test]
fn original_server_option_updates_keep_null_slots_cursors_and_priorities() {
    for c in corpus()["updates"].as_array().unwrap() {
        let mut options = Options::default();
        let mut states = Vec::new();
        for raw in c["frames"].as_array().unwrap() {
            assert!(
                options
                    .apply(&Frame {
                        opcode: 44,
                        payload: bytes(raw.as_str().unwrap())
                    })
                    .unwrap()
            );
            let mut state = labels(Some(&options.labels));
            for i in 0..8 {
                state.push_str(&format!(
                    ":{}:{}",
                    options.cursors[i],
                    i32::from(options.secondary[i])
                ));
            }
            states.push(state);
        }
        assert_eq!(states.join(";"), c["expected"].as_str().unwrap(), "{c}");
    }
}
#[test]
fn original_base37_title_arrays_and_failed_prefixes_match() {
    for c in corpus()["names"].as_array().unwrap() {
        let mut options = Options::default();
        let mut states = Vec::new();
        for event in c["events"].as_array().unwrap() {
            let v: Vec<_> = event.as_str().unwrap().split(',').collect();
            let output = if v[0] == "T" {
                let text = |s: &str| (s != "~").then(|| rs09_client::ui::latin1(&bytes(s)));
                if options
                    .titles(
                        c["game"].as_i64().unwrap() as i32,
                        v[1].parse().unwrap(),
                        text(v[2]),
                        text(v[3]),
                    )
                    .is_ok()
                {
                    "ok".into()
                } else {
                    "error".into()
                }
            } else {
                options
                    .name(&appearance(v[1].parse().unwrap(), v[2].parse().unwrap()))
                    .map(|s| text_hex(&s))
                    .unwrap_or_else(|_| "error".into())
            };
            states.push(format!(
                "{output}:{}:{}",
                labels(options.prefixes.as_deref()),
                labels(options.suffixes.as_deref())
            ));
        }
        assert_eq!(states.join(";"), c["expected"].as_str().unwrap(), "{c}");
    }
}

#[test]
fn real_cs2_title_assignment_and_server_options_reach_player_menus() {
    use rs09_client::{
        pack::Pack,
        ui::{
            HookArg,
            state::{Hook, Key, State},
            vm::Vm,
        },
    };
    for null_suffix in [false, true] {
        let ops = [
            (0u16, 0i32),
            (3, 0),
            (if null_suffix { 35 } else { 3 }, 0),
            (5422, 0),
            (21, 0),
        ];
        let mut script = vec![0];
        let mut strings = ["Sir ", " the Brave"].into_iter();
        for (op, arg) in ops {
            script.extend(op.to_be_bytes());
            if op == 3 {
                script.extend(strings.next().unwrap().as_bytes());
                script.push(0);
            } else if op >= 100 || op == 21 {
                script.push(arg as u8);
            } else {
                script.extend(arg.to_be_bytes());
            }
        }
        script.extend(5i32.to_be_bytes());
        script.extend(0u16.to_be_bytes());
        script.extend(1u16.to_be_bytes());
        script.extend([0; 4]);
        script.push(0);
        script.extend(1u16.to_be_bytes());
        let manifest=serde_json::to_vec(&serde_json::json!({"source":"Explicit CS2 title callback input","files":{"script/901":[0,script.len()]}})).unwrap();
        let mut raw = b"RS09VIS1".to_vec();
        raw.extend((manifest.len() as u32).to_le_bytes());
        raw.extend(manifest);
        raw.extend(script);
        let mut state = State::new(Pack::decode(raw).unwrap());
        let mut vm = Vm::default();
        let result = vm.run(
            &mut state,
            &Hook::new(Key::component(0), vec![HookArg::Int(901)]),
            1000,
        );
        assert_eq!(result.is_err(), null_suffix);
        assert_eq!(vm.player_options.prefixes, Some(vec![Some("Sir ".into())]));
        assert_eq!(
            vm.player_options.suffixes,
            (!null_suffix).then(|| vec![Some(" the Brave".into())])
        );
        let name = rs09_protocol::login::encode_name(b"test_player");
        assert_eq!(
            vm.player_options.name(&appearance(name, 0)).unwrap(),
            if null_suffix {
                "Sir Test Player"
            } else {
                "Sir Test Player the Brave"
            }
        );
        let frame = Frame {
            opcode: 44,
            payload: bytes("7fff0103466f6c6c6f7700"),
        };
        assert!(vm.apply_frame(&mut state, &frame).unwrap());
        assert_eq!(vm.player_options.labels[2].as_deref(), Some("Follow"));
        assert_eq!(vm.player_options.cursors[2], -1);
        assert!(!vm.player_options.secondary[2]);
    }
}
