//! `--capture-npc-menu NAME` (native, live `--capture` runs only): a scripted
//! pointer for the client smoke test (`cargo xtask client-smoke`,
//! docs/client-smoke.md). It moves a synthetic mouse through the frame's own
//! input path, so every check below runs the code a person's mouse runs:
//!
//! 1. on the welcome screen (378), click its Play button (child 140);
//! 2. once an NPC named NAME is in the frame's NPC list, take `--pick-probe`
//!    at the frame's mouse pick (`build_menu`) and choose a pixel where that
//!    probe hit the NPC;
//! 3. move the mouse there and keep the menu the frame builds (the hover
//!    menu, whose entries a right-click opens);
//! 4. right-click there and keep the menu that opened, with its bounds;
//! 5. left-click its `Attack` row, keep the packets the client queued, wait
//!    for the server's answer (a hit on the NPC and combat experience, or a
//!    time limit) and end the capture.
//!
//! Each stage is appended to `runs/visual/npc-menu-capture.json` as it ends,
//! so a run that stops early still says how far it got. The capture ends
//! with the usual capture files when the script is done or gives up, or at
//! the capture frame, whichever is first. Nothing here changes what the
//! client does with the events: they enter `Translator` as a window event
//! on the canvas would, and the pick, the menu and the click handling are
//! the frame's own.
use super::*;
use rs09_client::input::platform::Synthetic;
use std::collections::VecDeque;

// Waits are in seconds of `get_time`, not frames: a minimized window may
// draw at any rate, and the cycles that read the mouse run every 20 ms.
/// How long to wait for the game view, for the NPC, or for a menu, before
/// the script gives up and the capture is written as it stands.
const ENTER_SECONDS: f64 = 60.0;
const SEARCH_SECONDS: f64 = 30.0;
const STEP_SECONDS: f64 = 5.0;
/// Between a pointer event and reading what it did: several of the 20 ms
/// cycles that copy the listener's state into `Mouse`.
const SETTLE_SECONDS: f64 = 0.15;
/// After the Attack click: how long to wait for the server's answer: a hit
/// on the NPC (its health bar) and combat experience (attack, defence,
/// strength or hitpoints) rising in the stats it sends. A miss gives no
/// experience, and a character without `tutorial:complete` none at all, so
/// the wait covers several swings and ends early only when both are seen.
const AFTER_CLICK_SECONDS: f64 = 30.0;
/// Attack, defence, strength, hitpoints.
const COMBAT_SKILLS: [usize; 4] = [0, 1, 2, 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Waiting for the game: the welcome screen's Play, then the world.
    Enter,
    /// In the world; waiting for an NPC named NAME and its probe.
    Search,
    Hover,
    Open,
    Choose,
    After,
    Done,
}

pub(crate) struct Driver {
    name: String,
    stage: Stage,
    /// When the stage (or the last welcome click, or probe) began.
    since: f64,
    pending: VecDeque<Synthetic>,
    /// Whether `build_menu` takes `--pick-probe` this frame.
    pub(super) want_probe: bool,
    pub(super) done: bool,
    point: [i32; 2],
    welcome_clicks: u32,
    /// When the welcome screen's Play button was first laid out.
    welcome_seen: Option<f64>,
    /// A probe was asked for and has not come back yet.
    probe_asked: bool,
    last_probe: f64,
    outbox_mark: usize,
    experience: [i32; 4],
    npc_index: Option<usize>,
    hit_bar_until: Option<i32>,
    report: serde_json::Map<String, serde_json::Value>,
}

impl Driver {
    pub(crate) fn from_arguments(capture: bool, live: bool) -> Option<Self> {
        let name = option("--capture-npc-menu")?;
        assert!(
            capture && live,
            "--capture-npc-menu requires a live capture (--connect ... --capture)"
        );
        let mut report = serde_json::Map::new();
        report.insert("npc_name".into(), name.clone().into());
        report.insert("stages".into(), serde_json::json!([]));
        Some(Self {
            name,
            stage: Stage::Enter,
            since: get_time(),
            pending: VecDeque::new(),
            want_probe: false,
            done: false,
            point: [0, 0],
            welcome_clicks: 0,
            welcome_seen: None,
            probe_asked: false,
            last_probe: 0.0,
            outbox_mark: 0,
            experience: [0; 4],
            npc_index: None,
            hit_bar_until: None,
            report,
        })
    }

    /// The events to hand the frame's input after the window's own.
    pub(super) fn events(&mut self) -> Vec<Synthetic> {
        self.pending.drain(..).collect()
    }

    fn click(&mut self, button: MouseButton, point: [i32; 2]) {
        self.pending.push_back(Synthetic::Move(point));
        self.pending.push_back(Synthetic::Down(button, point));
        self.pending.push_back(Synthetic::Up(button));
    }

    /// End a stage: record it, write the report so far, move on.
    fn stage_done(&mut self, frame: i32, next: Stage, detail: serde_json::Value) {
        let name = format!("{:?}", self.stage).to_lowercase();
        if let Some(stages) = self.report.get_mut("stages").and_then(|s| s.as_array_mut()) {
            stages.push(serde_json::json!({"stage": name, "frame": frame, "detail": detail}));
        }
        self.stage = next;
        self.since = get_time();
        self.write();
    }

    fn fail(&mut self, frame: i32, reason: String) {
        self.report.insert(
            "failed_at".into(),
            format!("{:?}", self.stage).to_lowercase().into(),
        );
        self.report.insert("reason".into(), reason.into());
        self.report.insert("failed_frame".into(), frame.into());
        self.done = true;
        // Not a finished stage: `stages` lists only those.
        self.stage = Stage::Done;
        self.write();
    }

    fn write(&self) {
        let mut report = self.report.clone();
        report.insert(
            "stage".into(),
            format!("{:?}", self.stage).to_lowercase().into(),
        );
        let _ = std::fs::create_dir_all("runs/visual");
        let _ = std::fs::write(
            "runs/visual/npc-menu-capture.json",
            serde_json::to_vec_pretty(&serde_json::Value::Object(report)).unwrap_or_default(),
        );
    }
}

/// A pixel where the probe's mouse hit the NPC `index` with the player on
/// `plane`.
fn probe_point(probe: &serde_json::Value, plane: usize, index: u64) -> Option<[i32; 2]> {
    let level = probe["levels"]
        .as_array()?
        .iter()
        .find(|level| level["player_level"].as_u64() == Some(plane as u64))?;
    let actor = level["actors"].as_array()?.iter().find(|actor| {
        actor["npc"].as_bool() == Some(true) && actor["index"].as_u64() == Some(index)
    })?;
    let pixel = actor["first_pixel"].as_array()?;
    Some([
        pixel.first()?.as_i64()? as i32,
        pixel.get(1)?.as_i64()? as i32,
    ])
}

fn entries(menu: &Menu) -> serde_json::Value {
    serde_json::json!(
        menu.entries
            .iter()
            .map(|e| serde_json::json!({
                "option": e.option,
                "target": e.target,
                "action": e.action,
                "key": e.key,
                "args": e.args,
            }))
            .collect::<Vec<_>>()
    )
}

impl Game {
    /// Run after `build_menu`: read what this frame's pick and menus made of
    /// the script's last events and queue the next ones.
    pub(super) fn npc_menu_step(&mut self) {
        let Some(mut driver) = self.npc_menu.take() else {
            return;
        };
        let frame = self.frame;
        let elapsed = get_time() - driver.since;
        let probe = if driver.want_probe {
            self.pick_probe.take()
        } else {
            None
        };
        driver.want_probe = false;
        let in_game = self.connection.as_ref().is_some_and(|c| c.in_game());
        if in_game && !driver.report.contains_key("in_game_at") {
            let at = serde_json::json!({"frame": frame, "time": get_time()});
            driver.report.insert("in_game_at".into(), at);
        }
        let welcome = self
            .ui_nodes
            .iter()
            .find(|node| node.key.id == (378 << 16 | 140) && node.rect.w > 0 && node.rect.h > 0)
            .map(|node| [node.rect.x + node.rect.w / 2, node.rect.y + node.rect.h / 2]);
        match driver.stage {
            Stage::Enter => {
                let recent = |outbox: &ActionOutbox| -> Vec<serde_json::Value> {
                    let skip = outbox.len().saturating_sub(12);
                    outbox
                        .iter()
                        .skip(skip)
                        .map(|f| serde_json::json!([f.opcode, f.payload.len()]))
                        .collect()
                };
                if let Some(point) = welcome {
                    // As a person would: a moment after the screen shows,
                    // then again every few seconds while it stays.
                    if driver.welcome_seen.is_none() {
                        let at = serde_json::json!({"frame": frame, "time": get_time()});
                        driver.report.insert("welcome_seen_at".into(), at);
                    }
                    let first_seen = *driver.welcome_seen.get_or_insert(get_time());
                    let due = if driver.welcome_clicks == 0 {
                        get_time() - first_seen >= 1.0
                    } else {
                        elapsed >= 5.0
                    };
                    if due && driver.welcome_clicks >= 4 {
                        driver
                            .report
                            .insert("welcome_outbox_tail".into(), recent(&self.outbox).into());
                        driver.fail(
                            frame,
                            "the welcome screen stayed after four clicks on Play".into(),
                        );
                    } else if due {
                        driver.click(MouseButton::Left, point);
                        driver.welcome_clicks += 1;
                        driver.since = get_time();
                        let click = serde_json::json!({
                            "frame": frame, "time": get_time(), "point": point,
                            "outbox_tail_before": recent(&self.outbox),
                        });
                        if let Some(clicks) = driver
                            .report
                            .entry("welcome_clicks")
                            .or_insert_with(|| serde_json::json!([]))
                            .as_array_mut()
                        {
                            clicks.push(click);
                        }
                        driver.write();
                    }
                } else if in_game && !self.at_title && self.viewport.is_some() {
                    let detail = serde_json::json!({"welcome_clicks": driver.welcome_clicks});
                    driver.stage_done(frame, Stage::Search, detail);
                } else if elapsed > ENTER_SECONDS {
                    driver.fail(frame, "the game view never appeared".into());
                }
            }
            Stage::Search => {
                let name = driver.name.clone();
                let candidates: Vec<u64> = self
                    .workbench
                    .npcs
                    .iter()
                    .filter(|npc| {
                        self.workbench
                            .npc_definitions
                            .get(&npc.id)
                            .is_some_and(|d| d.name.eq_ignore_ascii_case(&name))
                    })
                    .map(|npc| npc.index as u64)
                    .collect();
                if let Some(probe) = probe {
                    driver.probe_asked = false;
                    let plane = self.plane;
                    let hit = candidates.iter().find_map(|&index| {
                        probe_point(&probe, plane, index).map(|point| (index, point))
                    });
                    // The probe is the smoke test's evidence of what the
                    // pick saw, whether or not it found the NPC.
                    let _ = std::fs::write(
                        "runs/visual/pick-state.json",
                        serde_json::to_vec_pretty(&probe).unwrap_or_default(),
                    );
                    match hit {
                        Some((index, point)) => {
                            driver.point = point;
                            driver.npc_index = Some(index as usize);
                            driver.pending.push_back(Synthetic::Move(point));
                            let detail = serde_json::json!({
                                "npc_index": index, "point": point,
                                "candidates": candidates, "plane": plane,
                            });
                            driver.stage_done(frame, Stage::Hover, detail);
                        }
                        None if elapsed > SEARCH_SECONDS => {
                            driver
                                .report
                                .insert("last_candidates".into(), serde_json::json!(candidates));
                            driver.fail(
                                frame,
                                format!(
                                    "the pick probe never hit an NPC named {name:?} ({} in the frame's NPC list)",
                                    candidates.len()
                                ),
                            );
                        }
                        // Ask again later: the NPC may be off the view.
                        None => {}
                    }
                } else {
                    // A probe costs a grid of picks: at most two a second.
                    if !candidates.is_empty()
                        && !driver.probe_asked
                        && get_time() - driver.last_probe >= 0.5
                    {
                        driver.want_probe = true;
                        driver.probe_asked = true;
                        driver.last_probe = get_time();
                    }
                    if elapsed > SEARCH_SECONDS + 5.0 {
                        let reason = if candidates.is_empty() {
                            format!("no NPC named {name:?} in the frame's NPC list")
                        } else {
                            "the pick probe was never taken".into()
                        };
                        driver.fail(frame, reason);
                    }
                }
            }
            Stage::Hover => {
                let name = driver.name.to_lowercase();
                let names_npc = self
                    .hover_menu
                    .entries
                    .iter()
                    .any(|e| e.target.to_lowercase().contains(&name));
                if elapsed >= SETTLE_SECONDS && names_npc {
                    let detail = serde_json::json!({
                        "point": driver.point,
                        "entries": entries(&self.hover_menu),
                    });
                    let point = driver.point;
                    driver.click(MouseButton::Right, point);
                    driver.stage_done(frame, Stage::Open, detail);
                } else if elapsed > STEP_SECONDS {
                    driver
                        .report
                        .insert("hover_entries".into(), entries(&self.hover_menu));
                    let point = driver.point;
                    driver.fail(
                        frame,
                        format!("the menu built at {point:?} names no {:?}", driver.name),
                    );
                }
            }
            Stage::Open => {
                let menu = &self.context_menu;
                if menu.menu.open {
                    let bounds = menu.bounds;
                    let size = menu.menu.entries.len();
                    let attack = menu.menu.entries.iter().position(|e| e.option == "Attack");
                    let detail = serde_json::json!({
                        "bounds": [bounds.x, bounds.y, bounds.width, bounds.height],
                        "entries": entries(&menu.menu),
                    });
                    match attack {
                        Some(index) => {
                            let row = [
                                bounds.x + bounds.width / 2,
                                bounds.row_baseline(size, index) - 5,
                            ];
                            driver.outbox_mark = self.outbox.len();
                            driver.experience = COMBAT_SKILLS.map(|s| self.ui_vm.experience[s]);
                            driver.hit_bar_until = driver
                                .npc_index
                                .and_then(|i| self.live_npcs.actors.get(&i))
                                .map(|n| n.hit_bar_until);
                            driver.click(MouseButton::Left, row);
                            driver
                                .report
                                .insert("attack_row".into(), serde_json::json!(row));
                            driver.stage_done(frame, Stage::Choose, detail);
                        }
                        None => {
                            driver.report.insert("open_menu".into(), detail);
                            driver.fail(frame, "the right-click menu has no Attack".into());
                        }
                    }
                } else if elapsed > STEP_SECONDS {
                    driver.fail(frame, "no menu opened on the right-click".into());
                }
            }
            Stage::Choose => {
                if !self.context_menu.menu.open && elapsed >= SETTLE_SECONDS {
                    let sent: Vec<_> = self
                        .outbox
                        .iter()
                        .skip(driver.outbox_mark.min(self.outbox.len()))
                        .map(|f| serde_json::json!({"opcode": f.opcode, "payload": f.payload}))
                        .collect();
                    driver.stage_done(frame, Stage::After, serde_json::json!({"sent": sent}));
                } else if elapsed > STEP_SECONDS {
                    driver.fail(frame, "the menu stayed open after the Attack click".into());
                }
            }
            Stage::After => {
                let now = COMBAT_SKILLS.map(|s| self.ui_vm.experience[s]);
                let gained = now.iter().zip(driver.experience).any(|(&n, b)| n > b);
                // A hit on the goblin (a miss too) sets its health bar's
                // time: the server's swing reached the client.
                let hit_bar_until = driver
                    .npc_index
                    .and_then(|i| self.live_npcs.actors.get(&i))
                    .map(|n| n.hit_bar_until);
                let npc_hit = hit_bar_until.is_some() && hit_bar_until != driver.hit_bar_until;
                if (gained && npc_hit) || elapsed > AFTER_CLICK_SECONDS {
                    let detail = serde_json::json!({
                        "combat_experience_before": driver.experience,
                        "combat_experience_after": now,
                        "gained": gained,
                        "npc_index": driver.npc_index,
                        "npc_hit_bar_until_before": driver.hit_bar_until,
                        "npc_hit_bar_until_after": hit_bar_until,
                        "npc_hit": npc_hit,
                    });
                    driver.done = true;
                    driver.stage_done(frame, Stage::Done, detail);
                }
            }
            Stage::Done => {}
        }
        self.npc_menu = Some(driver);
    }
}
