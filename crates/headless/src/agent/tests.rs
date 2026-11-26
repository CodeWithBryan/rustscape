use super::*;
use rs09_protocol::{
    buffer::Reader,
    game::{ClientAction, updates},
};

fn decode(f: &Frame) -> ClientAction {
    let mut r = Reader::new(&f.payload);
    ClientAction::decode(f.opcode, &mut r).unwrap().unwrap()
}
fn chat(view: &mut View, interface: i32, texts: &[(i32, &str)]) {
    for (child, t) in texts {
        view.apply(&updates::interface_text(interface << 16 | child, t.as_bytes(), 0).unwrap())
            .unwrap();
    }
    view.apply(&updates::open_interface(1, 752 << 16 | 12, 0, interface))
        .unwrap();
}
const DAGGER: Held = Held { id: 1205, slot: 0 };
const LOGS: Held = Held { id: 1511, slot: 4 };

#[test]
fn actions_are_the_packets_the_server_decodes() {
    assert_eq!(
        decode(&Action::Attack { index: 41 }.frame().unwrap()),
        ClientAction::NpcAction {
            opt_index: 1,
            npc_index: 41
        }
    );
    assert_eq!(
        decode(&Action::TalkTo { index: 7 }.frame().unwrap()),
        ClientAction::NpcAction {
            opt_index: 0,
            npc_index: 7
        }
    );
    assert_eq!(
        decode(
            &Action::NpcOption {
                option: 2,
                index: 300
            }
            .frame()
            .unwrap()
        ),
        ClientAction::NpcAction {
            opt_index: 2,
            npc_index: 300
        }
    );
    assert!(
        Action::NpcOption {
            option: 5,
            index: 1
        }
        .frame()
        .is_err()
    );
    assert_eq!(
        decode(
            &Action::UseOnItem {
                used: DAGGER,
                target: LOGS
            }
            .frame()
            .unwrap()
        ),
        ClientAction::UseWithItem {
            used_id: 1205,
            used_with_id: 1511,
            used_slot: 0,
            used_with_slot: 4,
            used_iface: 149,
            used_with_iface: 149,
            used_child: 0,
            used_with_child: 0
        }
    );
    assert_eq!(
        decode(
            &Action::UseOnNpc {
                used: LOGS,
                index: 12
            }
            .frame()
            .unwrap()
        ),
        ClientAction::UseWithNpc {
            item_id: 1511,
            npc_index: 12,
            iface: 149,
            slot: 4
        }
    );
    assert_eq!(
        decode(
            &Action::UseOnScenery {
                used: LOGS,
                id: 2732,
                tile: [3222, 3218]
            }
            .frame()
            .unwrap()
        ),
        ClientAction::UseWithScenery {
            item_id: 1511,
            slot: 4,
            scenery_id: 2732,
            x: 3222,
            y: 3218
        }
    );
    assert_eq!(
        decode(
            &Action::UseOnPlayer {
                used: LOGS,
                index: 3
            }
            .frame()
            .unwrap()
        ),
        ClientAction::UseWithPlayer {
            item_id: 1511,
            other_index: 3,
            iface: 149,
            slot: 4
        }
    );
    assert_eq!(
        decode(
            &Action::ItemOption {
                option: 4,
                item: DAGGER
            }
            .frame()
            .unwrap()
        ),
        ClientAction::ItemAction {
            opt_index: 4,
            item_id: 1205,
            slot: 0,
            iface: 149,
            child: 0
        }
    );
    assert_eq!(
        decode(&Action::CloseInterface.frame().unwrap()),
        ClientAction::CloseIface
    );
    // The same bytes the live world test hand-writes for Continue on 241:5.
    let resume = Action::Resume {
        component: 241 << 16 | 5,
    }
    .frame()
    .unwrap();
    assert_eq!(resume.opcode, 132);
    assert_eq!(resume.payload, [0, 5, 0, 241, 255, 255]);
}

#[test]
fn dialogues_are_read_back_from_the_chatbox() {
    let mut view = View::new();
    view.apply(&updates::interface_npc_head(241 << 16 | 2, 494, 0))
        .unwrap();
    chat(
        &mut view,
        241,
        &[
            (3, "Banker"),
            (4, "Good day, would you like to access your bank account?"),
        ],
    );
    let d = view.dialogue().unwrap();
    assert_eq!(d.kind, DialogueKind::Npc { npc: Some(494) });
    assert_eq!(d.title.as_deref(), Some("Banker"));
    assert_eq!(
        d.lines,
        ["Good day, would you like to access your bank account?"]
    );
    assert_eq!(
        d.resume(),
        Some(Action::Resume {
            component: 241 << 16 | 5
        })
    );
    assert_eq!(d.choose(0), None);

    // The options replace the NPC's line in the same chatbox component.
    chat(
        &mut view,
        230,
        &[
            (1, "Select an Option"),
            (2, "I'd like to access my bank account please."),
            (3, "I'd like to check my PIN settings."),
            (4, "What is this place?"),
        ],
    );
    let d = view.dialogue().unwrap();
    assert_eq!(d.kind, DialogueKind::Options);
    assert_eq!(d.lines.len(), 3);
    assert_eq!(d.resume(), None);
    assert_eq!(
        d.choose_text("access my bank"),
        Some(Action::Resume {
            component: 230 << 16 | 2
        })
    );
    assert_eq!(
        d.choose(2),
        Some(Action::Resume {
            component: 230 << 16 | 4
        })
    );
    assert_eq!(d.choose(3), None);

    // A plain message, then the chatbox closes.
    chat(&mut view, 211, &[(1, "first"), (2, "second")]);
    let d = view.dialogue().unwrap();
    assert_eq!((&d.kind, d.lines.len()), (&DialogueKind::Plain, 2));
    assert_eq!(
        d.resume(),
        Some(Action::Resume {
            component: 211 << 16 | 3
        })
    );
    view.apply(&updates::close_interface(0, 752 << 16 | 12))
        .unwrap();
    assert_eq!(view.dialogue(), None);
    let (events, omitted) = view.drain_events();
    assert_eq!(omitted, 0);
    assert_eq!(
        events,
        [
            Event::DialogueOpened(241),
            Event::DialogueClosed(241),
            Event::DialogueOpened(230),
            Event::DialogueClosed(230),
            Event::DialogueOpened(211),
            Event::DialogueClosed(211),
        ]
    );
    // The normal chatbox is not a dialogue.
    view.apply(&updates::open_interface(1, 752 << 16 | 12, 0, 137))
        .unwrap();
    assert_eq!(view.dialogue(), None);
}

#[test]
fn inventory_skills_and_messages_are_tracked() {
    let mut view = View::new();
    let item = |id, amount| Some(WireItem { id, amount });
    view.apply(
        &updates::inventory_full(
            INVENTORY_COMPONENT,
            INVENTORY,
            &[item(1205, 1), None, None, None, item(1511, 1)],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(view.held(1511), Some(LOGS));
    assert_eq!(view.held(995), None);
    view.apply(
        &updates::inventory_partial(
            INVENTORY_COMPONENT,
            INVENTORY,
            &[(4, None), (1, item(995, 300))],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(view.held(1511), None);
    assert_eq!(view.held(995), Some(Held { id: 995, slot: 1 }));
    view.apply(&updates::skill_level(0, 1, 0.0)).unwrap();
    view.apply(&updates::skill_level(0, 1, 8.0)).unwrap();
    view.apply(&updates::skill_level(3, 10, 1154.0)).unwrap();
    view.apply(&updates::skill_level(3, 7, 1154.0)).unwrap();
    assert_eq!(view.hitpoints(), Some(7));
    view.apply(&Frame {
        opcode: 70,
        payload: b"Nothing interesting happens.\0".to_vec(),
    })
    .unwrap();
    let (events, _) = view.drain_events();
    assert_eq!(
        &events[3..],
        [
            Event::ItemSlot {
                container: INVENTORY,
                slot: 1,
                item: item(995, 300)
            },
            Event::Experience {
                skill: 0,
                gained: 8,
                experience: 8
            },
            Event::Level { skill: 3, level: 7 },
            Event::Message("Nothing interesting happens.".into()),
        ]
    );
    assert_eq!(
        events[2],
        Event::ItemSlot {
            container: INVENTORY,
            slot: 4,
            item: None
        }
    );
    let snapshot = view.snapshot();
    assert_eq!(snapshot["inventory"][1]["id"], 995);
    assert_eq!(snapshot["hitpoints"], 7);
    assert!(snapshot["position"].is_null());
}
