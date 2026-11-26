//! Test-only driver of the shared production reducer, not a second implementation.
use rs09_protocol::{
    Error, Result,
    game::{
        Frame,
        ground::{Context, Info, Item, ItemRef, Pile, Stack, State, Tile},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Default)]
struct Services {
    info: BTreeMap<i32, Info>,
    items: Vec<ItemRef>,
    events: Vec<Value>,
    hook: Option<(String, usize, String)>,
}
impl Services {
    fn register(&mut self, state: &State) {
        let mut found = BTreeMap::new();
        for tile in state.occupied() {
            for item in state.stack(tile).unwrap().unwrap().items() {
                if !self.items.contains(&item) {
                    found.insert(item.serial(), item);
                }
            }
        }
        self.items.extend(found.into_values());
    }
    fn fire(&mut self, state: &mut State, stage: &str) -> Result<()> {
        if let Some((selected, remaining, _)) = &mut self.hook
            && stage == selected
        {
            *remaining -= 1;
            if *remaining == 0 {
                let (_, _, command) = self.hook.take().unwrap();
                self.command(state, &command)?;
            }
        }
        Ok(())
    }
    fn command(&mut self, state: &mut State, command: &str) -> Result<()> {
        let words: Vec<_> = command.split(' ').collect();
        let n = |i: usize| words[i].parse::<i32>().unwrap();
        let tile = || Tile {
            plane: n(1),
            x: n(2),
            y: n(3),
        };
        match words[0] {
            "M" => {
                self.info.insert(
                    n(1),
                    Info {
                        cost: n(2),
                        stackable: n(3),
                    },
                );
            }
            "P" => {
                state.plane = n(1);
                state.self_id = n(2);
            }
            "K" => {
                let item = state.new_item(Item {
                    id: n(4),
                    amount: n(5),
                });
                self.items.push(item.clone());
                let stack = state.stack(tile())?.unwrap_or_default();
                state.set_stack(tile(), Some(stack.clone()))?;
                stack.add_tail(&item);
            }
            "V" => self.items[n(1) as usize - 1].set(Item {
                id: n(2),
                amount: n(3),
            }),
            "D" => self.items[n(1) as usize - 1].unlink(),
            "T" => {
                let stack = state.stack(tile())?.unwrap_or_default();
                state.set_stack(tile(), Some(stack.clone()))?;
                stack.add_tail(&self.items[n(4) as usize - 1]);
            }
            "L" => state.set_stack(tile(), None)?,
            "A" => state.set_stack(
                tile(),
                state.stack(Tile {
                    plane: n(4),
                    x: n(5),
                    y: n(6),
                })?,
            )?,
            "E" => state.set_stack(tile(), Some(Stack::default()))?,
            "H" | "N" => {
                let stack = state
                    .stack(tile())?
                    .ok_or(Error::new(0, "null ground list iterator"))?;
                let item = if words[0] == "H" {
                    stack.head()?
                } else {
                    stack.next()?
                };
                self.events
                    .push(json!(["cursor", item.map(|i| self.identity(&i))]));
            }
            "F" => self.hook = Some((words[1].to_owned(), n(2) as usize, words[3..].join(" "))),
            "X" => return Err(Error::new(0, "callback failed")),
            "R" => state.refresh(n(1), n(2), self)?,
            "Z" => state.rebase_stacks([n(1), n(2)]),
            "U" => {
                let hex = words.get(2).copied().unwrap_or("");
                let payload = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect();
                state.apply_frame(
                    &Frame {
                        opcode: n(1) as u8,
                        payload,
                    },
                    self,
                )?;
            }
            _ => panic!("unknown oracle command {command}"),
        }
        self.register(state);
        Ok(())
    }
    fn identity(&self, item: &ItemRef) -> usize {
        self.items
            .iter()
            .position(|candidate| candidate == item)
            .expect("registered item")
            + 1
    }
    fn item(&self, item: &ItemRef) -> Value {
        let value = item.get();
        json!([self.identity(item), value.id, value.amount])
    }
    fn snapshot(&self, state: &State, error: Option<&str>) -> Value {
        let stacks: Vec<_> = state
            .occupied()
            .into_iter()
            .map(|tile| {
                json!([
                    tile.plane,
                    tile.x,
                    tile.y,
                    state
                        .stack(tile)
                        .unwrap()
                        .unwrap()
                        .items()
                        .iter()
                        .map(|i| self.item(i))
                        .collect::<Vec<_>>()
                ])
            })
            .collect();
        let piles: Vec<_> = state
            .piles()
            .map(|(tile, p)| {
                json!([
                    tile.plane,
                    tile.x,
                    tile.y,
                    p.height,
                    p.key,
                    self.item(&p.primary),
                    p.secondary.as_ref().map(|i| self.item(i)),
                    p.tertiary.as_ref().map(|i| self.item(i))
                ])
            })
            .collect();
        json!([
            error,
            state.plane,
            state.self_id,
            state.origin[0],
            state.origin[1],
            stacks,
            piles,
            self.events
        ])
    }
}
impl Context for Services {
    fn info(&mut self, state: &mut State, id: i32) -> Result<Info> {
        self.register(state);
        self.events.push(json!(["info", id]));
        self.fire(state, "info")?;
        Ok(*self.info.entry(id).or_insert(Info {
            cost: 1,
            stackable: 0,
        }))
    }
    fn height(&mut self, state: &mut State, plane: i32, x: i32, y: i32) -> Result<i32> {
        self.events.push(json!(["height", plane, x, y]));
        self.fire(state, "height")?;
        Ok(plane.wrapping_mul(1000).wrapping_add(x).wrapping_sub(y))
    }
    fn publish(&mut self, state: &mut State, tile: Tile, pile: Option<Pile>) -> Result<()> {
        self.events
            .push(json!(["publish", tile.plane, tile.x, tile.y]));
        self.fire(state, "publish")?;
        state.publish(tile, pile)
    }
}

#[test]
fn original_rt4_ground_item_observations() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/ground-items/oracle.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut state = State::default();
        let mut services = Services::default();
        for (index, command) in case["commands"].as_array().unwrap().iter().enumerate() {
            services.events.clear();
            let result = services.command(&mut state, command.as_str().unwrap());
            services.register(&state);
            let error = result.err().map(|e| match e.reason {
                "ground tile outside scene" | "truncated buffer" => "bounds",
                "null ground list iterator" => "null",
                "callback failed" => "callback",
                other => panic!("unexpected {other}"),
            });
            assert_eq!(
                services.snapshot(&state, error),
                case["expected"][index],
                "{} command {index}: {command}",
                case["name"]
            );
        }
    }
}

#[test]
fn ground_writers_match_original_reader_inputs_through_fragmented_frames() {
    use rs09_protocol::game::{
        Decoder, Direction,
        updates::{self, ServerUpdate},
    };
    let fixture: Value =
        serde_json::from_str(include_str!("../../fixtures/ground-items/oracle.json")).unwrap();
    let mut compared = 0;
    for case in fixture["cases"].as_array().unwrap() {
        for command in case["commands"].as_array().unwrap() {
            let words: Vec<_> = command.as_str().unwrap().split(' ').collect();
            if words[0] != "U" {
                continue;
            }
            let opcode = words[1].parse::<u8>().unwrap();
            let hex = words.get(2).copied().unwrap_or("");
            let payload: Vec<_> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            let coordinate = match opcode {
                33 if payload.len() == 5 => payload[2],
                135 if payload.len() == 7 => payload[2].wrapping_neg(),
                240 if payload.len() == 3 => 128u8.wrapping_sub(payload[0]),
                14 if payload.len() == 7 => payload[0],
                _ => continue,
            };
            // The packet reader ignores these two coordinate bits. The writer
            // deliberately emits the canonical, actual local 0..7 offsets.
            if coordinate & 0x88 != 0 {
                continue;
            }
            let frame = Frame { opcode, payload };
            let writer = match ServerUpdate::decode(&frame).unwrap().unwrap() {
                ServerUpdate::GroundAdd {
                    x,
                    y,
                    id,
                    amount,
                    exclude_player: None,
                } => updates::ground_add(x, y, id, amount),
                ServerUpdate::GroundAdd {
                    x,
                    y,
                    id,
                    amount,
                    exclude_player: Some(owner),
                } => updates::ground_add_excluding(x, y, id, amount, owner),
                ServerUpdate::GroundRemove { x, y, id } => updates::ground_remove(x, y, id),
                ServerUpdate::GroundAmount {
                    x,
                    y,
                    id,
                    old_amount,
                    amount,
                } => updates::ground_amount(x, y, id, old_amount, amount),
                _ => unreachable!(),
            };
            assert_eq!(writer, frame);
            let mut wire = vec![];
            writer
                .write(Direction::ServerToClient, None, &mut wire)
                .unwrap();
            for split in 0..=wire.len() {
                let mut decoder = Decoder::new(Direction::ServerToClient, 128, 64);
                decoder.push(&wire[..split]).unwrap();
                if split < wire.len() {
                    assert!(decoder.next_frame().unwrap().is_none());
                    decoder.push(&wire[split..]).unwrap();
                }
                assert_eq!(decoder.next_frame().unwrap(), Some(frame.clone()));
                assert!(decoder.next_frame().unwrap().is_none());
            }
            compared += 1;
        }
    }
    assert!(compared > 3500, "only {compared} canonical original inputs");
}

#[test]
fn malformed_envelope_is_rejected_before_shared_state_changes() {
    // Safety difference from the original streaming global reader: validate an
    // entire enclosed frame first, so a truncated later child has no prefix.
    let frame = Frame {
        opcode: 230,
        payload: vec![136, 120, 33, 1, 0, 0, 0, 129, 14, 0],
    };
    let mut state = State::default();
    let mut services = Services::default();
    assert!(state.apply_frame(&frame, &mut services).is_err());
    assert_eq!(state.origin, [0, 0]);
    assert!(state.occupied().is_empty());
    assert!(state.take_dirty().is_empty());
    assert!(services.events.is_empty());
}
