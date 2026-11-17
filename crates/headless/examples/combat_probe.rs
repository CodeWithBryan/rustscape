//! Two-session loopback smoke: attack an ordinary NPC and observe the same
//! death from both game streams, with XP delivered only to the attacker.
use rs09_headless::{
    ground,
    protocol::{
        buffer::Writer,
        game::{
            Frame, actions, npcs, players, rebuild,
            updates::{ServerUpdate, WireItem},
        },
    },
    session::{ManagedSession, Session},
};
use std::{io, net::SocketAddr, time::Duration};

struct View {
    base: Option<[i32; 2]>,
    players: players::State,
    npcs: npcs::State,
    tick: i32,
    xp: i32,
    wood_xp: i32,
    inventory: Vec<WireItem>,
    dead: bool,
    lost: bool,
    respawn: bool,
    last_ratio: Option<i32>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            base: None,
            players: players::State::new(0, 0),
            npcs: npcs::State::default(),
            tick: 0,
            xp: 0,
            wood_xp: 0,
            inventory: vec![],
            dead: false,
            lost: false,
            respawn: false,
            last_ratio: None,
        }
    }
}

impl View {
    fn update(&mut self, f: &Frame, target: Option<usize>) -> io::Result<()> {
        match f.opcode {
            162 => {
                let next = rebuild::decode_normal(f).map_err(io::Error::other)?;
                let new = next.base();
                if let Some(old) = self.base {
                    self.players.rebase([new[0] - old[0], new[1] - old[1]]);
                }
                self.base = Some(new);
                self.players.plane = next.plane;
                self.players
                    .self_player_mut()
                    .teleport(next.local_x, next.local_y, false);
            }
            225 => {
                self.tick += 1;
                self.players
                    .decode_frame(f, self.tick)
                    .map_err(io::Error::other)?;
            }
            32 => {
                let [x, y] = self
                    .base
                    .ok_or_else(|| io::Error::other("NPC update before scene"))?;
                let [px, py] = self.players.self_player().tile();
                self.npcs
                    .decode(f, [x + px, y + py], self.tick)
                    .map_err(io::Error::other)?;
                if let Some(target) = target {
                    if let Some(npc) = self.npcs.actors.get(&target) {
                        if self.lost {
                            self.respawn = true;
                        }
                        if let Some(hit) = &npc.last_mask.primary {
                            self.last_ratio = hit.ratio;
                            self.dead |= hit.ratio == Some(0);
                        }
                    } else if self.dead {
                        self.lost = true;
                    }
                }
            }
            _ => match ServerUpdate::decode(f).map_err(io::Error::other)? {
                Some(ServerUpdate::Skill {
                    skill: 0,
                    experience,
                    ..
                }) => self.xp = experience,
                Some(ServerUpdate::Skill {
                    skill: 8,
                    experience,
                    ..
                }) => self.wood_xp = experience,
                Some(ServerUpdate::InventoryFull {
                    container: 93,
                    items,
                    ..
                }) => self.inventory = items,
                Some(ServerUpdate::InventoryPartial {
                    container: 93,
                    slots,
                    ..
                }) => {
                    for slot in slots {
                        let index = usize::try_from(slot.slot).map_err(io::Error::other)?;
                        if index >= 28 {
                            return Err(io::Error::other("bad inventory slot"));
                        }
                        if index >= self.inventory.len() {
                            self.inventory
                                .resize(index + 1, WireItem { id: -1, amount: 0 });
                        }
                        self.inventory[index] = slot.item;
                    }
                }
                _ => {}
            },
        }
        Ok(())
    }
}

async fn train(mut session: ManagedSession) -> io::Result<()> {
    const AXE: [i32; 2] = [3196, 3209];
    let mut v = View::default();
    let mut ground_watch = ground::Observer::new(std::sync::Arc::new(ground::Definitions::load(
        std::path::Path::new("assets/visual/ui-players.pack"),
    )?));
    let self_id = i32::from(session.metadata().player_index);
    let sender = session.sender();
    let mut took = false;
    let mut tried_chop = false;
    let mut target = 0;
    let mut messages = vec![];
    tokio::time::timeout(Duration::from_secs(75), async {
        loop {
            let frame = session.next_frame().await?;
            if frame.opcode == 70 && messages.len() < 20 {
                messages.push(String::from_utf8_lossy(&frame.payload).into_owned());
            }
            if frame.opcode == 70 && (frame.payload.starts_with(b"I can't reach") || frame.payload.starts_with(b"There is an obstacle")) {
                target += 1;
                tried_chop = false;
            }
            v.update(&frame, None)?;
            ground_watch
                .frame(&frame, v.players.plane, self_id)
                .map_err(io::Error::other)?;
            if !took
                && v.base.is_some_and(|base| {
                    ground_watch.snapshot(base)["tiles"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|tile| {
                            tile["location"] == serde_json::json!([AXE[0], AXE[1], 0])
                                && tile["items"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .any(|item| item["id"] == 1351)
                        })
                })
            {
                let mut w = Writer::new();
                w.ip2(AXE[0]);
                w.p2(1351);
                w.ip2add(AXE[1]);
                sender
                    .send_frame(&Frame {
                        opcode: 66,
                        payload: w.into_vec(),
                    })
                    .await?;
                took = true;
            }
            if !tried_chop && v.inventory.iter().any(|i| i.id == 1351 && i.amount == 1) {
                let (id,tile) = [(1276,[3197,3209]), (1278,[3212,3232]), (1276,[3196,3226]), (1276,[3190,3217])]
                    .get(target).copied().ok_or_else(||io::Error::other(format!("none of the selected trees were reachable: {messages:?}")))?;
                let wire = actions::location_packet(42, id, tile).unwrap();
                sender
                    .send_frame(&Frame {
                        opcode: wire[0],
                        payload: wire[1..].to_vec(),
                    })
                    .await?;
                tried_chop = true;
            }
            if v.wood_xp == 25 && v.inventory.iter().any(|i| i.id == 1511 && i.amount == 1) {
                break;
            }
        }
        Ok::<(), io::Error>(())
    })
    .await
    .map_err(|_| {
        io::Error::other(format!(
            "woodcut timeout: took={took}, chop={tried_chop}, xp={}, tile={:?}, messages={messages:?}",
            v.wood_xp,
            v.players.self_player().tile()
        ))
    })??;
    session.close().await?;
    let [x, y] = v.base.unwrap();
    let [a, b] = v.players.self_player().tile();
    println!(
        "{}",
        serde_json::json!({"passed":true,"woodcuttingXp":v.wood_xp,"log":1511,"location":[x+a,y+b,v.players.plane]})
    );
    Ok(())
}

async fn next(session: &mut ManagedSession) -> io::Result<Frame> {
    session.next_frame().await
}

#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let tcp: SocketAddr = args.next().ok_or("TCP address required")?.parse()?;
    let ws: SocketAddr = args.next().ok_or("WebSocket address required")?.parse()?;
    let option: usize = args.next().ok_or("Attack option required")?.parse()?;
    let mode = args.next();
    let (target_id, earned_xp, limit) = match mode.as_deref() {
        Some("kill") => (
            args.next().ok_or("NPC definition ID required")?.parse()?,
            args.next().ok_or("Attack XP gain required")?.parse()?,
            Duration::from_secs(240),
        ),
        _ => (47, 8, Duration::from_secs(70)),
    };
    assert!(
        (tcp.ip().is_loopback() && ws.ip().is_loopback())
            || (mode.as_deref() == Some("verify-auth")
                && tcp.ip() == ws.ip()
                && matches!(tcp.ip(), std::net::IpAddr::V4(ip) if ip.is_private())),
        "use owned loopback probes or a configured private IPv4 playtest host"
    );
    if mode.as_deref() == Some("verify-auth") {
        // One negative check and two positive admissions using accounts that
        // were provisioned for this test; do not probe unowned endpoints.
        let bad = Session::connect(tcp, false, "meleetest", "wrongpass").await;
        assert!(matches!(bad, Err(e) if e.to_string().contains("code 3")));
        let mut a = Session::connect(tcp, false, "meleetest", "alpha-test-1").await?;
        let mut b = Session::connect(ws, true, "meleeobserve", "alpha-test-2").await?;
        assert_ne!(a.metadata().player_index, b.metadata().player_index);
        a.close().await?;
        b.close().await?;
        println!(
            "{}",
            serde_json::json!({"passed":true,"wrongPasswordRejected":true,"configuredPlayers":2})
        );
        return Ok(());
    }
    let username =
        if mode.as_deref() == Some("train") || mode.as_deref() == Some("verify-wood-save") {
            "woodcuttest"
        } else {
            "meleetest"
        };
    let mut actor = Session::connect(tcp, false, username, "testing")
        .await?
        .into_managed();
    if mode.as_deref() == Some("train") {
        return train(actor).await.map_err(Into::into);
    }
    if mode.as_deref() == Some("verify-wood-save") {
        let mut view = View::default();
        tokio::time::timeout(Duration::from_secs(10), async {
            while view.wood_xp != 25
                || !view.inventory.iter().any(|i| i.id == 1511 && i.amount == 1)
            {
                view.update(&actor.next_frame().await?, None)?;
            }
            Ok::<(), io::Error>(())
        })
        .await??;
        actor.close().await?;
        let [x, y] = view.base.unwrap();
        let [a, b] = view.players.self_player().tile();
        println!(
            "{}",
            serde_json::json!({"passed":true,"restoredWoodcuttingXp":view.wood_xp,"restoredLog":1511,"location":[x+a,y+b,view.players.plane]})
        );
        return Ok(());
    }
    if mode.as_deref() == Some("verify-save") {
        let mut view = View::default();
        tokio::time::timeout(Duration::from_secs(10), async {
            while view.xp != 8 {
                view.update(&actor.next_frame().await?, None)?;
            }
            Ok::<(), io::Error>(())
        })
        .await??;
        actor.close().await?;
        println!(
            "{}",
            serde_json::json!({"passed":true,"restoredAttackXp":view.xp})
        );
        return Ok(());
    }
    let mut observer = Session::connect(ws, true, "meleeobserve", "testing")
        .await?
        .into_managed();
    let sender = actor.sender();
    let a_index = actor.metadata().player_index as usize;
    let b_index = observer.metadata().player_index as usize;
    let (mut a, mut b) = (View::default(), View::default());
    let mut selected = None;
    let mut start_xp = 0;
    let mut wire = vec![];
    tokio::time::timeout(limit, async {
        loop {
            tokio::select! {
                f = next(&mut actor) => a.update(&f?, selected)?,
                f = next(&mut observer) => b.update(&f?, selected)?,
            }
            if selected.is_none()
                && a.players.local_ids.contains(&b_index)
                && b.players.local_ids.contains(&a_index)
            {
                let [base_x, base_y] = a.base.unwrap();
                let [local_x, local_y] = a.players.self_player().tile();
                let near = [base_x + local_x, base_y + local_y];
                let target = a
                    .npcs
                    .actors
                    .iter()
                    .filter(|(i, n)| n.type_id == target_id && b.npcs.actors.contains_key(i))
                    .min_by_key(|(_, npc)| {
                        (npc.tile[0] - near[0]).abs() + (npc.tile[1] - near[1]).abs()
                    });
                if let Some((&index, _)) = target {
                    start_xp = a.xp;
                    let mut w = Writer::new();
                    let index = index as i32;
                    let opcode = match option {
                        0 => {
                            w.ip2(index);
                            78
                        }
                        1 => {
                            w.ip2add(index);
                            3
                        }
                        2 => {
                            w.p2add(index);
                            148
                        }
                        3 => {
                            w.p2(index);
                            30
                        }
                        4 => {
                            w.ip2(index);
                            218
                        }
                        _ => return Err(io::Error::other("unknown NPC option")),
                    };
                    wire = sender
                        .send_frame(&Frame {
                            opcode,
                            payload: w.into_vec(),
                        })
                        .await?;
                    selected = Some(index as usize);
                }
            }
            if a.dead && b.dead && a.respawn && b.respawn && a.xp - start_xp == earned_xp {
                return Ok::<(), io::Error>(());
            }
        }
    })
    .await
    .map_err(|_| {
        format!(
            "melee proof timed out: target={selected:?} actor={:?} observer={:?}",
            (a.tick, a.xp, a.dead, a.lost, a.respawn, a.last_ratio),
            (b.tick, b.xp, b.dead, b.lost, b.respawn, b.last_ratio)
        )
    })??;
    assert_eq!(b.xp, 0, "spectator received attack XP");
    actor.close().await?;
    observer.close().await?;
    println!(
        "{}",
        serde_json::json!({"passed":true,"attacker":a_index,"observer":b_index,"npc":selected,"npcType":target_id,"attackWire":wire,"attackerXp":a.xp,"attackXpGain":a.xp-start_xp,"observerXp":b.xp,"bothSawDeathAndRespawn":a.respawn && b.respawn})
    );
    Ok(())
}
