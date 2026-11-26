//! Structured shared player-state observations over a real native game stream.
use crate::{
    ground, scenery,
    session::{SceneryAction, Session},
};
use rs09_protocol::game::{self, npcs, players, rebuild, updates::ServerUpdate};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::time::timeout;
#[derive(Clone, Copy)]
pub struct Walk {
    pub destination: [i32; 2],
    pub running: bool,
    pub after_update: usize,
}
pub struct Options {
    pub updates: usize,
    pub walk: Option<Walk>,
    pub npc_stream: bool,
    pub ground: Option<Arc<ground::Definitions>>,
    pub scenery: Vec<ScheduledScenery>,
}
#[derive(Clone, Copy, Debug)]
pub struct ScheduledScenery {
    pub action: SceneryAction,
    pub after_update: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            updates: 10,
            walk: None,
            npc_stream: false,
            ground: None,
            scenery: vec![],
        }
    }
}
#[derive(Default)]
struct NpcHistory {
    previous: BTreeMap<usize, (i32, [i32; 3])>,
    changes: VecDeque<Value>,
    omitted: usize,
}
#[derive(Default)]
struct PlayerSpots {
    updates: VecDeque<Value>,
    omitted: usize,
}
#[derive(Default)]
struct PlayerAnimations {
    updates: VecDeque<Value>,
    omitted: usize,
}
#[derive(Default)]
struct InventoryHistory {
    events: VecDeque<Value>,
    omitted: usize,
}
impl InventoryHistory {
    fn record(&mut self, frame: &game::Frame, player_update: usize) -> rs09_protocol::Result<()> {
        let event = match ServerUpdate::decode(frame)? {
            Some(ServerUpdate::InventoryFull {
                pointer,
                container,
                items,
            }) => {
                let items: Vec<_> = items
                    .into_iter()
                    .map(|i| json!({"id":i.id,"amount":i.amount}))
                    .collect();
                json!({"kind":"full","pointer":pointer,"container":container,"items":items})
            }
            Some(ServerUpdate::InventoryPartial {
                pointer,
                container,
                slots,
            }) => {
                let slots: Vec<_> = slots
                    .into_iter()
                    .map(|s| json!({"slot":s.slot,"id":s.item.id,"amount":s.item.amount}))
                    .collect();
                json!({"kind":"partial","pointer":pointer,"container":container,"slots":slots})
            }
            Some(ServerUpdate::InventoryClear { pointer }) => {
                json!({"kind":"clear","pointer":pointer})
            }
            Some(ServerUpdate::InterfaceText {
                pointer,
                text,
                counter,
            }) if pointer == (667 << 16 | 32) => {
                // Preserve wire bytes. The label is the server's displayed,
                // integer-truncated value, not a measured precise weight.
                json!({"kind":"weightLabel","pointer":pointer,"bytes":text,"counter":counter})
            }
            _ => return Ok(()),
        };
        let mut event = event;
        event["playerUpdate"] = json!(player_update);
        event["opcode"] = json!(frame.opcode);
        self.events.push_back(event);
        if self.events.len() > 256 {
            self.events.pop_front();
            self.omitted += 1;
        }
        Ok(())
    }
}
impl PlayerSpots {
    fn record(&mut self, number: usize, update: &players::Update, frame: &game::Frame) {
        let fields: Vec<_> = update
            .extended
            .iter()
            .flat_map(|extended| {
                extended.fields.iter().filter_map(|field| {
                    if let players::Field::SpotAnimation { id, packed } = field {
                        Some(json!({"slot":extended.index,"id":id,"packed":packed}))
                    } else {
                        None
                    }
                })
            })
            .collect();
        if fields.is_empty() {
            return;
        }
        let payload: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.updates.push_back(json!({"playerUpdate":number,"opcode":frame.opcode,"fields":fields,"payloadHex":payload}));
        if self.updates.len() > 256 {
            self.updates.pop_front();
            self.omitted += 1;
        }
    }
}
impl PlayerAnimations {
    fn record(&mut self, number: usize, update: &players::Update, frame: &game::Frame) {
        let fields: Vec<_> = update
            .extended
            .iter()
            .flat_map(|extended| {
                extended.fields.iter().filter_map(|field| {
                    if let players::Field::Animation { id, delay } = field {
                        Some(json!({"slot":extended.index,"id":id,"delay":delay}))
                    } else {
                        None
                    }
                })
            })
            .collect();
        if fields.is_empty() {
            return;
        }
        let payload: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.updates.push_back(json!({"playerUpdate":number,"opcode":frame.opcode,"fields":fields,"payloadHex":payload}));
        if self.updates.len() > 256 {
            self.updates.pop_front();
            self.omitted += 1;
        }
    }
}
impl NpcHistory {
    fn record(&mut self, state: &npcs::State, update: usize, plane: i32) {
        let next: BTreeMap<_, _> = state
            .local_ids
            .iter()
            .filter_map(|id| {
                state
                    .actors
                    .get(id)
                    .map(|n| (*id, (n.type_id, [n.tile[0], n.tile[1], plane])))
            })
            .collect();
        let changed: Vec<_> = next
            .iter()
            .filter(|(id, value)| self.previous.get(id) != Some(value))
            .map(|(id, (kind, tile))| json!({"slot":id,"type":kind,"location":tile}))
            .collect();
        let removed: Vec<_> = self
            .previous
            .keys()
            .filter(|id| !next.contains_key(id))
            .copied()
            .collect();
        if !changed.is_empty() || !removed.is_empty() {
            self.changes
                .push_back(json!({"update":update,"changed":changed,"removed":removed}));
            if self.changes.len() > 256 {
                self.changes.pop_front();
                self.omitted += 1;
            }
        }
        self.previous = next;
    }
}
pub async fn observe(
    address: SocketAddr,
    websocket: bool,
    username: String,
    password: String,
    options: Options,
) -> io::Result<Value> {
    let Options {
        updates,
        walk,
        npc_stream,
        ground,
        scenery: scenery_actions,
    } = options;
    if updates == 0
        || walk.is_some_and(|w| w.after_update == 0 || w.after_update >= updates)
        || scenery_actions
            .iter()
            .any(|s| s.after_update == 0 || s.after_update >= updates)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scheduled actions must leave observation updates and use positive update counts",
        ));
    }
    for s in &scenery_actions {
        s.action.frame()?;
    }
    let mut session = Session::connect(address, websocket, &username, &password)
        .await?
        .into_managed();
    let metadata = session.metadata().clone();
    let sender = session.sender();
    let mut state = players::State::new(0, 0);
    let mut npc_state = npcs::State::default();
    let mut ground = ground.map(ground::Observer::new);
    let mut npc_observed = 0usize;
    let mut npc_history = NpcHistory::default();
    let mut player_spots = PlayerSpots::default();
    let mut player_animations = PlayerAnimations::default();
    let mut inventories = InventoryHistory::default();
    let mut rebuild = None;
    let mut observed = 0;
    let mut opcodes = BTreeMap::<u8, usize>::new();
    let mut sent = None;
    let mut trajectory = VecDeque::new();
    let mut energy = None;
    let mut run_toggled = None;
    let mut skills: [Option<Value>; 24] = std::array::from_fn(|_| None);
    let mut scenery = scenery::Observer::default();
    let mut sent_scenery = vec![];
    while observed < updates || (npc_stream && npc_observed < updates) {
        let frame = timeout(Duration::from_secs(10), session.next_frame())
            .await
            .map_err(|_| io::Error::other("world read timeout"))??;
        *opcodes.entry(frame.opcode).or_default() += 1;
        scenery
            .frame(
                &frame,
                state.plane,
                rebuild.as_ref().map(rebuild::NormalRebuild::base),
                observed,
            )
            .map_err(io::Error::other)?;
        match frame.opcode {
            162 => {
                let next = rebuild::decode_normal(&frame).map_err(io::Error::other)?;
                let mut delta = [0, 0];
                if let Some(previous) = &rebuild {
                    let previous: &rebuild::NormalRebuild = previous;
                    let a = previous.base();
                    let b = next.base();
                    delta = [b[0] - a[0], b[1] - a[1]];
                    state.rebase(delta);
                }
                state.plane = next.plane;
                state
                    .self_player_mut()
                    .teleport(next.local_x, next.local_y, false);
                if let Some(ground) = &mut ground {
                    ground
                        .rebuild(delta, next.plane, i32::from(metadata.player_index))
                        .map_err(io::Error::other)?;
                }
                rebuild = Some(next);
            }
            225 => {
                let update = state
                    .decode_frame(&frame, observed as i32)
                    .map_err(io::Error::other)?;
                player_spots.record(observed + 1, &update, &frame);
                player_animations.record(observed + 1, &update, &frame);
                observed += 1;
                if walk.is_some()
                    && let Some(r) = &rebuild
                {
                    let base = r.base();
                    let p = state.self_player();
                    trajectory.push_back(json!({"update":observed,"location":[base[0]+p.queue_x[0],base[1]+p.queue_y[0],state.plane],"energy":energy}));
                    if trajectory.len() > 256 {
                        trajectory.pop_front();
                    }
                }
                if let Some(walk) = walk
                    && sent.is_none()
                    && observed == walk.after_update
                {
                    if rebuild.is_none() {
                        return Err(io::Error::other(
                            "walking requires a received scene rebuild",
                        ));
                    }
                    let frame = game::actions::worldspace_walk(
                        walk.destination[0],
                        walk.destination[1],
                        walk.running,
                    );
                    let bytes = sender.send_frame(&frame).await?;
                    sent = Some(
                        json!({"destination":walk.destination,"running":walk.running,"afterUpdate":observed,"opcode":frame.opcode,"wire":bytes}),
                    );
                }
                for scheduled in &scenery_actions {
                    if observed == scheduled.after_update {
                        if rebuild.is_none() {
                            return Err(io::Error::other(
                                "scenery action requires a received scene rebuild",
                            ));
                        }
                        let wire = sender.scenery(scheduled.action).await?;
                        sent_scenery.push(json!({"id":scheduled.action.id,"tile":scheduled.action.tile,"option":scheduled.action.option,"afterUpdate":observed,"wire":wire}));
                    }
                }
            }
            32 => {
                let base = rebuild
                    .as_ref()
                    .ok_or_else(|| io::Error::other("NPC update before scene rebuild"))?
                    .base();
                let p = state.self_player();
                npc_observed += 1;
                npc_state
                    .decode(
                        &frame,
                        [base[0] + p.queue_x[0], base[1] + p.queue_y[0]],
                        npc_observed as i32,
                    )
                    .map_err(io::Error::other)?;
                if npc_stream {
                    npc_history.record(&npc_state, npc_observed, state.plane);
                }
            }
            86 => return Err(io::Error::other("server logged out")),
            14 | 26 | 33 | 112 | 135 | 230 | 240 => {
                if let Some(ground) = &mut ground {
                    ground
                        .frame(&frame, state.plane, i32::from(metadata.player_index))
                        .map_err(io::Error::other)?;
                }
            }
            105 | 22 | 144 | 171 => inventories
                .record(&frame, observed)
                .map_err(io::Error::other)?,
            38 | 234 | 60 | 226 => match ServerUpdate::decode(&frame).map_err(io::Error::other)? {
                Some(ServerUpdate::Skill {
                    skill,
                    boosted,
                    experience,
                }) => {
                    let entry = skills.get_mut(skill as usize).ok_or_else(|| {
                        io::Error::other("skill index exceeds original24-slot player state")
                    })?;
                    *entry = Some(json!({"boosted":boosted,"experience":experience}));
                }
                Some(ServerUpdate::RunEnergy { energy: value }) => energy = Some(value),
                Some(ServerUpdate::Varp { id: 173, value }) => run_toggled = Some(value != 0),
                _ => {}
            },
            _ => {}
        }
        if observed >= updates && (!npc_stream || npc_observed >= updates) {
            break;
        }
    }
    let rebuild = rebuild.ok_or_else(|| io::Error::other("missing scene rebuild"))?;
    let base = rebuild.base();
    let observation = |id: usize, p: &players::Player| json!({"slot":id,"location":[base[0]+p.queue_x[0],base[1]+p.queue_y[0],state.plane],"queuedMoves":p.queue_size,"appearance":p.appearance.as_ref().map(|a|json!({"name37":a.name37,"npc":a.npc_id,"body":a.slots,"colors":a.colors,"bas":a.bas,"combat":a.combat}))});
    let nearby: Vec<_> = state
        .local_ids
        .iter()
        .map(|&id| observation(id, state.players[id].as_ref().unwrap()))
        .collect();
    let mut result = json!({"scope":"live login/scene/player stream observations; complete gameplay/actions pending","username":username,"networkPlayerIndex":metadata.player_index,"selfSlot":players::SELF,"playerUpdates":observed,"self":observation(players::SELF,state.self_player()),"nearby":nearby,"frameCounts":opcodes,"regions":rebuild.regions(),"websocket":websocket});
    result["npcUpdates"] = json!(npc_observed);
    if !inventories.events.is_empty() {
        result["inventoryEvents"] =
            json!({"events":inventories.events,"eventsOmitted":inventories.omitted});
    }
    if !player_spots.updates.is_empty() {
        result["playerSpots"] =
            json!({"updates":player_spots.updates,"updatesOmitted":player_spots.omitted});
    }
    if !player_animations.updates.is_empty() {
        result["playerAnimations"] =
            json!({"updates":player_animations.updates,"updatesOmitted":player_animations.omitted});
    }
    if let Some(ground) = &ground {
        result["groundItems"] = ground.snapshot(base);
    }
    result["skills"] = json!(skills);
    if npc_stream {
        result["npcTrajectory"] =
            json!({"changes":npc_history.changes,"changesOmitted":npc_history.omitted});
    }
    result["npcs"]=json!(npc_state.local_ids.iter().filter_map(|id|npc_state.actors.get(id).map(|n|{
        let m=&n.last_mask;json!({"slot":id,"type":n.type_id,"location":[n.tile[0],n.tile[1],state.plane],"lastMask":{"flags":m.flags,"primaryHit":m.primary.as_ref().map(|h|json!({"damage":h.damage,"kind":h.kind,"ratio":h.ratio})),"secondaryHit":m.secondary.as_ref().map(|h|json!({"damage":h.damage,"kind":h.kind})),"animation":m.animation,"faceEntity":m.face_entity,"spot":m.spot,"chatBytes":m.chat,"slots":m.slots,"faceTile":m.face_tile}})
    })).collect::<Vec<_>>());
    if walk.is_some() {
        result["scope"] = json!(
            "live absolute walk request and authoritative player observations; full gameplay remains pending"
        );
        result["walk"] = json!({"sent":sent,"trajectory":trajectory,"trajectoryOmitted":observed.saturating_sub(trajectory.len()),"energy":energy,"runToggled":run_toggled});
    }
    result["scenery"] = scenery.snapshot();
    if !scenery_actions.is_empty() {
        result["sceneryRequests"] = json!(sent_scenery);
    }
    session.close().await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_history_preserves_shared_decoded_events_labels_and_bounded_history() {
        use rs09_protocol::game::updates::{self, WireItem};
        let mut history = InventoryHistory::default();
        let full = updates::inventory_full(
            149 << 16,
            93,
            &[
                Some(WireItem {
                    id: 995,
                    amount: 300,
                }),
                None,
            ],
        )
        .unwrap();
        history.record(&full, 0).unwrap();
        let partial = updates::inventory_partial(
            149 << 16,
            93,
            &[
                (
                    1,
                    Some(WireItem {
                        id: 12047,
                        amount: 1,
                    }),
                ),
                (0, None),
            ],
        )
        .unwrap();
        history.record(&partial, 2).unwrap();
        let label = updates::interface_text(667 << 16 | 32, b"2 kg", -1).unwrap();
        history.record(&label, 2).unwrap();
        history
            .record(&updates::inventory_clear(149 << 16), 3)
            .unwrap();
        assert_eq!(history.events.len(), 4);
        assert_eq!(
            history.events[0]["items"],
            json!([{"id":995,"amount":300},{"id":-1,"amount":0}])
        );
        assert_eq!(
            history.events[1]["slots"],
            json!([{"slot":1,"id":12047,"amount":1},{"slot":0,"id":-1,"amount":0}])
        );
        assert_eq!(history.events[2]["bytes"], json!(b"2 kg".to_vec()));
        assert_eq!(history.events[2]["counter"], 65535);
        assert_eq!(history.events[3]["kind"], "clear");
        assert!(
            history.events[2].get("weight").is_none(),
            "a label is not a measured exact weight"
        );
        for _ in 0..256 {
            history.record(&full, 4).unwrap();
        }
        assert_eq!(history.events.len(), 256);
        assert_eq!(history.omitted, 4);
    }
    #[test]
    fn player_spot_observations_keep_decoded_slots_clears_and_exact_payload_with_a_bound() {
        let frame = game::Frame {
            opcode: 225,
            payload: vec![0x10, 1, 0xff, 0xff, 0, 0, 0, 0],
        };
        let mut history = PlayerSpots::default();
        let mut update = players::Update::default();
        update.extended.push(players::Extended {
            index: players::SELF,
            flags: 0x110,
            fields: vec![players::Field::SpotAnimation { id: -1, packed: 0 }],
        });
        for n in 1..=257 {
            history.record(n, &update, &frame);
        }
        assert_eq!(history.updates.len(), 256);
        assert_eq!(history.omitted, 1);
        assert_eq!(
            history.updates.back().unwrap()["fields"][0],
            json!({"slot":players::SELF,"id":-1,"packed":0})
        );
        assert_eq!(
            history.updates.back().unwrap()["payloadHex"],
            "1001ffff00000000"
        );
    }
    #[test]
    fn animation_observations_preserve_shared_decoder_ids_delays_and_wire_clear() {
        let mut history = PlayerAnimations::default();
        let mut state = players::State::new(0, 0);
        for (number, id, delay) in [(1, 535, 7), (2, -1, 255)] {
            // Self has a mask, zero locals, then the2047 addition sentinel.
            let mut payload = vec![0x80, 0x1f, 0xfc, 8];
            payload.extend([((id as u16) >> 8) as u8, id as u8, delay]);
            let frame = game::Frame {
                opcode: 225,
                payload,
            };
            let update = state.decode_frame(&frame, number as i32).unwrap();
            history.record(number, &update, &frame);
        }
        assert_eq!(
            history.updates[0]["fields"],
            json!([{"slot":players::SELF,"id":535,"delay":7}])
        );
        assert_eq!(
            history.updates[1]["fields"],
            json!([{"slot":players::SELF,"id":-1,"delay":255}])
        );
        let empty = players::Update::default();
        history.record(
            3,
            &empty,
            &game::Frame {
                opcode: 225,
                payload: vec![],
            },
        );
        assert_eq!(history.updates.len(), 2);
        for n in 3..=258 {
            let update = players::Update {
                extended: vec![players::Extended {
                    index: 7,
                    flags: 8,
                    fields: vec![players::Field::Animation { id: 863, delay: 0 }],
                }],
                ..players::Update::default()
            };
            history.record(
                n,
                &update,
                &game::Frame {
                    opcode: 225,
                    payload: vec![0xaa],
                },
            );
        }
        assert_eq!(history.omitted, 2);
        assert_eq!(history.updates.len(), 256);
        assert_eq!(history.updates.back().unwrap()["fields"][0]["slot"], 7);
        assert_eq!(history.updates.back().unwrap()["payloadHex"], "aa");
    }
    #[test]
    fn npc_history_preserves_reuse_removal_plane_changes_and_bounds() {
        let mut state = npcs::State::default();
        let mut history = NpcHistory::default();
        state.local_ids.push(7);
        state.actors.insert(
            7,
            npcs::Npc {
                type_id: 1,
                tile: [3207, 3216],
                direction: 0,
                last_seen: 1,
                last_mask: npcs::Mask::default(),
            },
        );
        history.record(&state, 1, 0);
        history.record(&state, 2, 0);
        assert_eq!(history.changes.len(), 1);
        history.record(&state, 3, 1);
        assert_eq!(
            history.changes[1]["changed"][0]["location"],
            json!([3207, 3216, 1])
        );
        state.actors.get_mut(&7).unwrap().type_id = 2;
        history.record(&state, 4, 1);
        assert_eq!(history.changes[2]["changed"][0]["type"], 2);
        state.local_ids.clear();
        history.record(&state, 5, 1);
        assert_eq!(history.changes[3]["removed"], json!([7]));
        state.local_ids.push(7);
        for update in 6..306 {
            state.actors.get_mut(&7).unwrap().tile[0] += 1;
            history.record(&state, update, 1);
        }
        assert_eq!(history.changes.len(), 256);
        assert_eq!(history.omitted, 48);
        assert_eq!(history.changes.back().unwrap()["update"], 305);
    }
}
