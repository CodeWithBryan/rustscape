//! Finite data-only Take actor and spectator. Both observe the real item before
//! sending; success requires authoritative ground/inventory/label/sound changes.
use rs09_headless::{
    ground::{Definitions, Observer},
    protocol::{
        buffer::Writer,
        game::{
            Frame, players, rebuild,
            updates::{ServerUpdate, WireItem},
        },
    },
    session::Session,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    net::SocketAddr,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{sync::Barrier, time::timeout};
#[path = "common/bank_input.rs"]
mod bank_input;
struct Options {
    address: SocketAddr,
    websocket: bool,
    id: i32,
    tile: [i32; 2],
    actor: String,
    spectator: String,
    banking: bool,
    banking_x: bool,
}
fn contains(snapshot: &Value, tile: [i32; 2], id: i32) -> bool {
    snapshot["tiles"].as_array().unwrap().iter().any(|t| {
        t["location"] == json!([tile[0], tile[1], 0])
            && t["items"].as_array().unwrap().iter().any(|i| i["id"] == id)
    })
}
async fn observe(
    options: &Options,
    actor: bool,
    definitions: Arc<Definitions>,
    barrier: Arc<Barrier>,
    bank_finished: Arc<AtomicBool>,
) -> io::Result<Value> {
    let name = if actor {
        &options.actor
    } else {
        &options.spectator
    };
    let mut session = Session::connect(options.address, options.websocket, name, "testing")
        .await?
        .into_managed();
    let sender = session.sender();
    let self_id = i32::from(session.metadata().player_index);
    let mut observer = Observer::new(definitions);
    let mut state = players::State::new(0, 0);
    let mut base: Option<[i32; 2]> = None;
    let mut updates = 0;
    let mut seen = false;
    let mut removed = false;
    let mut sent = None;
    let mut inventory: Vec<WireItem> = vec![];
    let mut initial_inventory = None;
    let mut labels = vec![];
    let mut sounds = vec![];
    let mut counts = BTreeMap::<u8, usize>::new();
    let mut initial_ground = None;
    let mut bank_items: Vec<WireItem> = vec![];
    let mut bank_full = false;
    let mut main_open = false;
    let mut side_open = false;
    let mut bank_seen = false;
    let mut root = 548;
    let mut bank_stage = 0;
    let mut single_closed = false;
    let mut main_closed = false;
    let mut closed_tick = None;
    let mut bank_steps = vec![];
    let mut bank_wires = vec![];
    let mut input = bank_input::Probe::default();
    loop {
        let frame = session.next_frame().await?;
        *counts.entry(frame.opcode).or_default() += 1;
        if frame.opcode == 162 {
            let next = rebuild::decode_normal(&frame).map_err(io::Error::other)?;
            let new = next.base();
            let delta = base.map_or([0, 0], |old| [new[0] - old[0], new[1] - old[1]]);
            state.rebase(delta);
            state.plane = next.plane;
            state
                .self_player_mut()
                .teleport(next.local_x, next.local_y, false);
            observer
                .rebuild(delta, next.plane, self_id)
                .map_err(io::Error::other)?;
            base = Some(new);
        } else if frame.opcode == 225 {
            state
                .decode_frame(&frame, updates)
                .map_err(io::Error::other)?;
            updates += 1;
        }
        let update = ServerUpdate::decode(&frame).map_err(io::Error::other)?;
        if options.banking_x {
            input.observe(update.as_ref())?;
            if !actor && input.private_seen() {
                return Err(io::Error::other("spectator received private amount input"));
            }
        }
        match update {
            Some(ServerUpdate::WindowsPane { interface, .. }) => root = interface,
            Some(ServerUpdate::OpenInterface { interface, .. }) => {
                if interface == 762 {
                    if options.banking
                        && actor
                        && ((!options.banking_x && bank_stage >= 3)
                            || (options.banking_x && input.closing()))
                    {
                        return Err(io::Error::other("bank reopened after Close/late button"));
                    }
                    main_open = true;
                    bank_seen = true;
                }
                if interface == 763 {
                    side_open = true;
                    bank_seen = true;
                }
            }
            Some(ServerUpdate::CloseInterface { pointer, .. }) => {
                if pointer == (root << 16) | if root == 746 { 76 } else { 80 } {
                    single_closed = true;
                    side_open = false;
                }
                if pointer == (root << 16) | if root == 746 { 6 } else { 11 } {
                    main_closed = true;
                    main_open = false;
                }
            }
            Some(ServerUpdate::InventoryFull {
                container: 95,
                items,
                ..
            }) => {
                bank_items = items;
                bank_full = true;
                bank_seen = true;
            }
            Some(ServerUpdate::InventoryPartial {
                container: 95,
                slots,
                ..
            }) => {
                bank_seen = true;
                for s in slots {
                    let index = usize::try_from(s.slot).map_err(io::Error::other)?;
                    if index >= 4096 {
                        return Err(io::Error::other("bank probe slot exceeds limit"));
                    }
                    if bank_items.len() <= index {
                        bank_items.resize(index + 1, WireItem { id: -1, amount: 0 });
                    }
                    bank_items[index] = s.item;
                }
            }
            Some(ServerUpdate::InventoryFull {
                container: 93,
                items,
                ..
            }) => inventory = items,
            Some(ServerUpdate::InventoryPartial {
                container: 93,
                slots,
                ..
            }) => {
                for s in slots {
                    let index = usize::try_from(s.slot).map_err(io::Error::other)?;
                    if index >= 4096 {
                        return Err(io::Error::other(
                            "inventory slot exceeds finite probe limit",
                        ));
                    }
                    if inventory.len() <= index {
                        inventory.resize(index + 1, WireItem { id: -1, amount: 0 });
                    }
                    inventory[index] = s.item;
                }
            }
            Some(ServerUpdate::InterfaceText {
                pointer,
                text,
                counter,
            }) if pointer == (667 << 16 | 32) => {
                labels.push(json!({"bytes":text,"counter":counter,"afterTake":sent.is_some()}))
            }
            Some(ServerUpdate::SynthSound { id, loops, delay }) => {
                sounds.push(json!({"id":id,"loops":loops,"delay":delay,"afterTake":sent.is_some()}))
            }
            _ => {}
        }
        if let Some(base) = base {
            observer
                .frame(&frame, state.plane, self_id)
                .map_err(io::Error::other)?;
            let snapshot = observer.snapshot(base);
            let present = contains(&snapshot, options.tile, options.id);
            if present && !seen {
                seen = true;
                initial_ground = Some(snapshot);
                initial_inventory = Some(
                    inventory
                        .iter()
                        .map(|i| json!({"id":i.id,"amount":i.amount}))
                        .collect::<Vec<_>>(),
                );
                barrier.wait().await;
                if actor {
                    let mut w = Writer::new();
                    w.ip2(options.tile[0]);
                    w.p2(options.id);
                    w.ip2add(options.tile[1]);
                    let frame = Frame {
                        opcode: 66,
                        payload: w.into_vec(),
                    };
                    sent = Some(sender.send_frame(&frame).await?);
                }
            } else if seen && !present {
                removed = true;
            }
        }
        let has_item = inventory.iter().any(|i| i.id == options.id && i.amount > 0);
        let sound = sounds
            .iter()
            .any(|s| s["id"] == 2582 && s["afterTake"] == true);
        let label = labels.iter().any(|l| l["afterTake"] == true);
        if options.banking && actor && !options.banking_x {
            let quantity = |items: &[WireItem]| {
                items
                    .iter()
                    .filter(|i| i.id == options.id && i.amount > 0)
                    .map(|i| i64::from(i.amount))
                    .sum::<i64>()
            };
            let carried = quantity(&inventory);
            let banked = quantity(&bank_items);
            let request = match bank_stage {
                0 if main_open && side_open && bank_full && removed && has_item => {
                    if carried != 1 || banked != 0 {
                        return Err(io::Error::other(
                            "finite bank probe requires one picked-up item and an empty matching bank slot",
                        ));
                    }
                    let slot = inventory
                        .iter()
                        .position(|i| i.id == options.id && i.amount > 0)
                        .unwrap();
                    bank_steps.push(
                        json!({"stage":"open","inventoryQuantity":carried,"bankQuantity":banked}),
                    );
                    bank_stage = 1;
                    Some((763, 0, slot as i32))
                }
                1 if banked == 1 && carried == 0 => {
                    let slot = bank_items
                        .iter()
                        .position(|i| i.id == options.id && i.amount > 0)
                        .unwrap();
                    bank_steps.push(json!({"stage":"deposit-confirmed","inventoryQuantity":carried,"bankQuantity":banked}));
                    bank_stage = 2;
                    Some((762, 73, slot as i32))
                }
                2 if banked == 0 && carried == 1 => {
                    bank_steps.push(json!({"stage":"withdraw-confirmed","inventoryQuantity":carried,"bankQuantity":banked}));
                    bank_wires.push(
                        sender
                            .send_frame(&Frame {
                                opcode: 184,
                                payload: vec![],
                            })
                            .await?,
                    );
                    bank_stage = 3;
                    Some((762, 22, -1))
                }
                _ => None,
            };
            if let Some((iface, child, slot)) = request {
                let mut w = Writer::new();
                w.p4((iface << 16) | child);
                w.p2(slot);
                bank_wires.push(
                    sender
                        .send_frame(&Frame {
                            opcode: 155,
                            payload: w.into_vec(),
                        })
                        .await?,
                );
            }
            if bank_stage == 3 && single_closed && main_closed {
                let first = *closed_tick.get_or_insert(updates);
                if frame.opcode == 225 && updates - first >= 2 {
                    bank_steps.push(json!({"stage":"close-confirmed","singleClosed":true,"mainClosed":true,"continuedPlayerUpdates":updates-first}));
                    bank_stage = 4;
                    bank_finished.store(true, Ordering::Release);
                }
            }
        }
        if options.banking_x && actor {
            let quantity = |items: &[WireItem]| {
                items
                    .iter()
                    .filter(|i| i.id == options.id && i.amount > 0)
                    .map(|i| i64::from(i.amount))
                    .sum()
            };
            let frames = input.advance(bank_input::Snapshot {
                carried: quantity(&inventory),
                banked: quantity(&bank_items),
                inventory_slot: inventory
                    .iter()
                    .position(|i| i.id == options.id && i.amount > 0)
                    .map(|i| i as i32),
                bank_slot: bank_items
                    .iter()
                    .position(|i| i.id == options.id && i.amount > 0)
                    .map(|i| i as i32),
                main_open,
                side_open,
                bank_full,
                removed,
                single_closed,
                main_closed,
                updates,
                opcode: frame.opcode,
            })?;
            for next in frames {
                input.wires.push(sender.send_frame(&next).await?);
            }
            if input.done() {
                bank_finished.store(true, Ordering::Release);
            }
        }
        let bank_done = !options.banking
            || if actor {
                if options.banking_x {
                    input.done()
                } else {
                    bank_stage == 4
                }
            } else {
                bank_finished.load(Ordering::Acquire) && frame.opcode == 225
            };
        if removed && (!actor || (has_item && sound && label)) && bank_done {
            break;
        }
    }
    let final_ground = observer.snapshot(base.ok_or_else(|| io::Error::other("missing rebuild"))?);
    session.close().await?;
    let mut bank = if options.banking_x {
        input.report()
    } else {
        json!({"stage":bank_stage,"steps":bank_steps,"requests":bank_wires})
    };
    if options.banking {
        bank["seen"] = json!(bank_seen);
        bank["open"] = json!(main_open);
        bank["sideOpen"] = json!(side_open);
        bank["remainingItems"] = json!(
            bank_items
                .iter()
                .filter(|i| i.id >= 0 && i.amount > 0)
                .map(|i| json!({"id":i.id,"amount":i.amount}))
                .collect::<Vec<_>>()
        );
    }
    Ok(
        json!({"role":if actor{"actor"}else{"spectator"},"username":name,"groundSeen":seen,"groundRemoved":removed,"initialGround":initial_ground,"finalGround":final_ground,"initialInventory":initial_inventory,"inventory":inventory.iter().map(|i|json!({"id":i.id,"amount":i.amount})).collect::<Vec<_>>(),"weightLabels":labels,"sounds":sounds,"takeWire":sent,"playerUpdates":updates,"frameCounts":counts,"bank":if options.banking{bank}else{Value::Null}}),
    )
}
#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|v| v == name)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    let tile: Vec<i32> = arg("--tile")
        .unwrap_or("3207,3216")
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let options = Options {
        address: arg("--address").unwrap_or("127.0.0.1:43640").parse()?,
        websocket: args.iter().any(|v| v == "--websocket"),
        id: arg("--id").unwrap_or("1351").parse()?,
        tile: tile.try_into().map_err(|_| "--tile needs X,Y")?,
        actor: arg("--username").unwrap_or("pickupagent").into(),
        spectator: arg("--spectator").unwrap_or("pickupspec").into(),
        banking: args.iter().any(|v| v == "--banking" || v == "--banking-x"),
        banking_x: args.iter().any(|v| v == "--banking-x"),
    };
    if options.banking_x && options.id != 1351 {
        return Err("--banking-x requires the controlled 1351 pickup".into());
    }
    let definitions = Arc::new(Definitions::load(Path::new(
        arg("--pack").unwrap_or("runs/windows-client-bootstrap/assets/visual/ui-players.pack"),
    ))?);
    let barrier = Arc::new(Barrier::new(2));
    let bank_finished = Arc::new(AtomicBool::new(false));
    let (actor, spectator) = timeout(Duration::from_secs(25), async {
        tokio::try_join!(
            observe(
                &options,
                true,
                definitions.clone(),
                barrier.clone(),
                bank_finished.clone()
            ),
            observe(
                &options,
                false,
                definitions.clone(),
                barrier.clone(),
                bank_finished.clone()
            )
        )
    })
    .await
    .map_err(|_| "Take actor/spectator did not observe all outcomes within25seconds")??;
    if spectator["inventory"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["id"] == options.id)
    {
        return Err("spectator incorrectly received actor inventory".into());
    }
    if spectator["sounds"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == 2582)
    {
        return Err("spectator incorrectly received private pickup sound".into());
    }
    if options.banking
        && (actor["bank"]["stage"] != if options.banking_x { 12 } else { 4 }
            || spectator["bank"]["seen"] != false
            || options.banking_x && spectator["bank"]["privateInputSeen"] != false)
    {
        return Err("bank round-trip failed or private bank packets reached spectator".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"scope":if options.banking_x{"finite real Take, Deposit-X/Withdraw-X, invalid/overflow amount errors, Continue and Close with a continuously-reading spectator; test-bank admission after actual pickup, original input/listener paths with explicit fresh-player service boundaries; no natural banker/PIN, capacity or full-game claim"}else if options.banking{"finite real Take/bank deposit/withdraw/Close actor and continuously-reading spectator; bank opens through an explicit server test controller after actual pickup, transactions use original IfAction/listener paths; no banker/PIN/X/dialogue, capacity or full-game claim"}else{"finite real data-only Take actor/spectator; shared original ground/packet reducers, no renderer, fixed-sleep success, capacity or full-game claim"},"passed":true,"banking":options.banking,"bankingX":options.banking_x,"websocket":options.websocket,"itemId":options.id,"tile":options.tile,"actor":actor,"spectator":spectator})
        )?
    );
    Ok(())
}
