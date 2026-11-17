// The statements below were written when these fields were locals of the
// main loop and are kept as they were: a field bound to a local of its own
// name is a reference, so `&mut name` borrows where the compiler would not
// need it to.
#![allow(clippy::needless_borrow)]
use super::*;

impl Game {
    pub(super) fn cycle_devices(&mut self) {
        let ui_vm = &mut self.ui_vm;
        let tooltip_seed = &mut self.tooltip_seed;
        let input = &mut self.input;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let mut music = &mut self.music;
        let capture_route = self.capture_route;
        let capture_route_run = self.capture_route_run;
        let muted = self.muted;
        let ui_cycle = self.ui_cycle;
        // client.mainLoop (client.java:1697-1718), in every game state: the
        // loop counter, the hover text's seed, the music player, then
        // Keyboard.loop and Mouse.loop: one snapshot of the devices per cycle.
        let ui_time = f64::from(ui_cycle);
        ui_vm.cycle = ui_cycle;
        tooltip_seed.tick(client_ui::local_time);
        // Songs and jingles from the server and from scripts, then one
        // cycle of the player. The M key silences music like effects.
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        if let Some(music) = &mut music {
            use rs09_client::{music::Request, ui::vm::MusicRequest};
            for request in ui_vm.music.drain(..) {
                music.request(match request {
                    MusicRequest::Song(id) => Request::Song(id),
                    MusicRequest::Jingle { id, .. } => Request::Jingle(id),
                });
            }
            music.set_volume(if muted {
                0
            } else {
                ui_vm.preferences.music_volume
            });
            if let Some(song) = music.cycle() {
                // SOUND_SONGEND: the area's song has played through.
                let mut ended = rs09_protocol::buffer::Writer::new();
                ended.p4(song);
                ui_vm.outgoing.push(rs09_protocol::game::Frame {
                    opcode: 137,
                    payload: ended.into_vec(),
                });
            }
        }
        ui_vm.music.clear();
        input.cycle();
        ClickCross::clicked([input.mouse.click_x, input.mouse.click_y]);
        // ClientProt.sendMovePacket sends Keyboard.KEY_CTRL, either Ctrl key.
        let running = input.keyboard.pressed_keys[rs09_client::input::keyboard::KEY_CTRL]
            || capture_route && capture_route_run;
        self.ui_time = ui_time;
        self.running = running;
    }

    pub(super) fn cycle_timers(&mut self) {
        let mut ui = &mut self.ui;
        let ui_vm = &mut self.ui_vm;
        // Protocol.java:2400-2406.
        rs09_client::ui::legacy_buttons::advance_logout_timer(
            &mut ui_vm.logout_on_disconnect_timer,
        );
        if let Some(ui) = &mut ui {
            ui_vm.advance_reboot_timer(ui);
        }
    }

    pub(super) fn cycle_read(&mut self) -> Vec<rs09_protocol::game::Frame> {
        let ui_vm = &mut self.ui_vm;
        let mut connection = &mut self.connection;
        let cycle_driver = &mut self.cycle_driver;
        let mut input = &mut self.input;
        let mut frames = Vec::new();
        if let Some(connection) = &mut connection {
            #[allow(unused_mut)]
            let mut incoming = match connection.poll() {
                Ok(frames) => frames,
                Err(e) => {
                    connection.stop(e);
                    vec![]
                }
            };
            // A development aid: packets written to `RS09_FRAME_DUMP/inject`.
            #[cfg(not(target_arch = "wasm32"))]
            if connection.in_game() {
                incoming.extend(super::inject::take());
            }
            frames = cycle_driver.read(
                incoming,
                connection,
                ui_vm.window_mode,
                &ui_vm.preferences,
                &mut ui_vm.logout_on_disconnect_timer,
                &mut input,
            );
        }
        frames
    }

    pub(super) fn cycle_rebuild(&mut self) -> bool {
        let workbench = &mut self.workbench;
        let outbox = &mut self.outbox;
        let session_life = &mut self.session_life;
        let connection = &mut self.connection;
        let cycle_driver = &mut self.cycle_driver;
        // The scene of the last rebuild is built: client.gameState is 30, not 25.
        macro_rules! game_state_30 {
            () => {
                session_life.as_ref().is_none_or(|life| {
                    life.life.state() == rs09_client::lifecycle::GameState::InGame
                })
            };
        }
        // LoginManager.rebuildMap: ping(true) follows each blocking stage,
        // four of them, seven with an underwater map.
        let keep_alives = rs09_client::underwater::rebuild_keep_alives(
            workbench
                .regions
                .iter()
                .any(|region| region.underwater.layer().is_some()),
        );
        cycle_driver.rebuild(
            connection.as_mut(),
            &mut outbox.pending,
            game_state_30!(),
            keep_alives,
        )
    }

    pub(super) fn cycle_own_packets(&mut self) {
        let ui = &mut self.ui;
        let ui_vm = &mut self.ui_vm;
        let follow = &mut self.follow;
        let session_life = &mut self.session_life;
        let cycle_driver = &mut self.cycle_driver;
        let mut input = &mut self.input;
        // The scene of the last rebuild is built: client.gameState is 30, not 25.
        macro_rules! game_state_30 {
            () => {
                session_life.as_ref().is_none_or(|life| {
                    life.life.state() == rs09_client::lifecycle::GameState::InGame
                })
            };
        }
        // What one 20 ms cycle of the loop's own packets reads and writes.
        macro_rules! cycle_state {
            ($verify_id:expr) => {
                client_cycle::CycleState {
                    // ClientProt 21 reports `(int)` of the camera targets.
                    camera: [follow.pitch_target as i32, follow.yaw_target as i32],
                    preferences: &mut ui_vm.preferences,
                    verify_id: $verify_id,
                    logout_timer: &mut ui_vm.logout_on_disconnect_timer,
                    window_mode: ui_vm.window_mode,
                    scene_ready: game_state_30!(),
                }
            };
        }
        let verify_id = ui.as_ref().map_or(0, |ui| ui.verify_id);
        cycle_driver.cycle_begin(&mut input, cycle_state!(verify_id));
    }

    pub(super) fn advance_ticks(&mut self) {
        let live_mode = self.live_mode;
        let capture_realtime = self.capture_realtime;
        let capture = self.capture;
        let paused = self.paused;
        let mut ticks = self.ticks;
        let mut world_ticks = self.world_ticks;
        let frame = self.frame;
        if !paused {
            ticks += 1.0;
            world_ticks += 1.0;
        }
        if capture && !live_mode && !capture_realtime {
            ticks = frame as f64;
            world_ticks = frame as f64;
        }
        self.ticks = ticks;
        self.world_ticks = world_ticks;
    }

    pub(super) fn cycle_world(&mut self) {
        let workbench = &mut self.workbench;
        let drift_random = &mut self.drift_random;
        let title_world = &mut self.title_world;
        let motion = &mut self.motion;
        let connection = &mut self.connection;
        let player_state = &mut self.player_state;
        let live_actors = &mut self.live_actors;
        let live_npcs = &mut self.live_npcs;
        let live_bas = &mut self.live_bas;
        let live_forms = &mut self.live_forms;
        let actor_states = &mut self.actor_states;
        let actor_sounds = &mut self.actor_sounds;
        let world_sounds = &mut self.world_sounds;
        let live_mode = self.live_mode;
        let npc_packet_mode = self.npc_packet_mode;
        let animation_origin = self.animation_origin;
        let movement_tick = self.movement_tick;
        let route_run = self.route_run;
        let mut manual = self.manual;
        let live_base = self.live_base;
        let direction = self.direction;
        let running = self.running;
        let self_size = self.self_size;
        let plane = self.plane;
        let mut route_status = std::mem::take(&mut self.route_status);
        let mut route_steps = std::mem::take(&mut self.route_steps);
        if let Err(error) = workbench.advance_world() {
            route_status = error;
        }
        if !npc_packet_mode {
            world_sounds.extend(workbench.advance_npcs(movement_tick));
        }
        if !live_mode && motion.queue.is_empty() {
            if direction != [0, 0] {
                route_steps.clear();
                let source = motion.head;
                workbench.refresh_navigation(source);
                let origin = workbench.navigation.origin;
                if walking::can_step_sized(
                    &workbench.navigation.maps[plane],
                    [source[0] - origin[0], source[1] - origin[1]],
                    direction,
                    self_size,
                ) {
                    motion.enqueue(
                        [source[0] + direction[0], source[1] + direction[1]],
                        if running { 2 } else { 1 },
                    );
                    manual = 808;
                }
            } else if let Some(step) = route_steps.pop_front() {
                motion.enqueue(step, if route_run { 2 } else { 1 });
            }
        }
        let cycle = animation_origin.wrapping_add(movement_tick as i32);
        let origin = live_base.unwrap_or(workbench.navigation.origin);
        let self_index = connection
            .as_ref()
            .and_then(|c| c.world.as_ref())
            .map(|w| usize::from(w.player_index));
        let face_target =
            |subject: usize, target: i32, motion: &Movement, others: &BTreeMap<usize, Movement>| {
                if (0..32768).contains(&target) {
                    return live_npcs
                        .actors
                        .get(&(target as usize))
                        .map(|n| n.motion.fine);
                }
                let index = target
                    .checked_sub(32768)
                    .and_then(|n| usize::try_from(n).ok())?;
                let index = if Some(index) == self_index {
                    rs09_protocol::game::players::SELF
                } else {
                    index
                };
                if index == subject {
                    return None;
                }
                if index == rs09_protocol::game::players::SELF {
                    Some(motion.fine)
                } else {
                    others.get(&index).map(|a| a.fine)
                }
            };
        let target = face_target(
            rs09_protocol::game::players::SELF,
            motion.face_entity,
            &motion,
            &live_actors,
        );
        let state = actor_states
            .entry(rs09_protocol::game::players::SELF)
            .or_default();
        if let Err(error) = motion.advance_actor(
            state,
            rs09_client::actor_motion::Context {
                bas: &workbench.bas,
                size: live_forms
                    .get(&rs09_protocol::game::players::SELF)
                    .map_or(1, |f| f.size),
                origin,
                is_self: true,
                cycle,
                face_target: target,
                sequences: &workbench.sequences,
            },
        ) {
            route_status = format!("Actor motion: {error}");
        }
        state.movement.id = motion.sequence;
        match state.tick(cycle, &workbench.sequences, &workbench.spot_sequences) {
            Ok(events) => actor_sounds.extend(
                events
                    .into_iter()
                    .map(|s| (rs09_protocol::game::players::SELF, s, cycle)),
            ),
            Err(error) => {
                route_status = format!("Actor animation: {error}");
                state.action.id = -1;
            }
        }
        motion.frame = state.movement.frame.max(0) as usize;
        motion.next_frame = state.movement.next.max(0) as usize;
        motion.progress = state.movement.clock;
        for index in &player_state.local_ids {
            let Some(actor) = live_actors.get(index) else {
                continue;
            };
            let target = face_target(*index, actor.face_entity, &motion, &live_actors);
            let actor = live_actors.get_mut(index).unwrap();
            let state = actor_states.entry(*index).or_default();
            if let Some(bas) = live_bas.get(index) {
                if let Err(error) = actor.advance_actor(
                    state,
                    rs09_client::actor_motion::Context {
                        bas,
                        size: live_forms.get(index).map_or(1, |f| f.size),
                        origin,
                        is_self: false,
                        cycle,
                        face_target: target,
                        sequences: &workbench.sequences,
                    },
                ) {
                    route_status = format!("Actor motion: {error}");
                }
            }
            state.movement.id = actor.sequence;
            if let Ok(events) = state.tick(cycle, &workbench.sequences, &workbench.spot_sequences) {
                actor_sounds.extend(events.into_iter().map(|s| (*index, s, cycle)));
            }
            actor.frame = state.movement.frame.max(0) as usize;
            actor.next_frame = state.movement.next.max(0) as usize;
            actor.progress = state.movement.clock;
        }
        for &index in &live_npcs.local_ids {
            let npc = &live_npcs.actors[&index];
            let target = npc.motion.face_entity;
            let target = if target >= 32768 {
                let id = (target - 32768) as usize;
                if Some(id) == self_index || id == rs09_protocol::game::players::SELF {
                    Some(motion.fine)
                } else {
                    live_actors.get(&id).map(|a| a.fine)
                }
            } else if target >= 0 && target as usize != index {
                live_npcs
                    .actors
                    .get(&(target as usize))
                    .map(|n| n.motion.fine)
            } else {
                None
            };
            let npc = live_npcs.actors.get_mut(&index).unwrap();
            // client.mainUpdate: a title-screen NPC may start a step.
            if title_world.npc_begin(index, npc, &mut workbench.navigation, || {
                drift_random.next_double()
            }) {
                continue;
            }
            if let Some(bas) = workbench.npc_bases.get(&npc.metadata.bas) {
                if let Err(error) = npc.motion.advance_actor(
                    &mut npc.animation,
                    rs09_client::actor_motion::Context {
                        bas: &bas.movement,
                        size: npc.metadata.size,
                        origin,
                        is_self: false,
                        cycle,
                        face_target: target,
                        sequences: &workbench.sequences,
                    },
                ) {
                    route_status = format!("NPC motion: {error}");
                }
                npc.animation.movement.id = npc.motion.sequence;
                match npc
                    .animation
                    .tick(cycle, &workbench.sequences, &workbench.spot_sequences)
                {
                    Ok(events) => world_sounds.extend(events.into_iter().map(|s| WorldSound {
                        sequence: s.sequence as u32,
                        frame: s.frame,
                        fine: npc.motion.fine,
                        cycle: movement_tick as i32,
                    })),
                    Err(error) => {
                        route_status = format!("NPC animation: {error}");
                        npc.animation.action.id = -1;
                        npc.animation.spot.id = -1;
                    }
                }
                npc.motion.frame = npc.animation.movement.frame.max(0) as usize;
                npc.motion.next_frame = npc.animation.movement.next.max(0) as usize;
                npc.motion.progress = npc.animation.movement.clock;
            }
            title_world.npc_end(index, npc, &mut workbench.navigation);
        }
        for state in workbench.overheads.values_mut() {
            state.tick_chat();
        }
        self.manual = manual;
        self.route_status = route_status;
        self.route_steps = route_steps;
    }

    pub(super) fn cycle_camera(&mut self) {
        let mut workbench = &mut self.workbench;
        let drift_random = &mut self.drift_random;
        let mut drift = &mut self.drift;
        let ui_vm = &mut self.ui_vm;
        let world_map = &mut self.world_map;
        let title_world = &mut self.title_world;
        let mut motion = &mut self.motion;
        let mut atmosphere = &mut self.atmosphere;
        let mut follow = &mut self.follow;
        let scene_occlusion = &mut self.scene_occlusion;
        let mut camera_keys = std::mem::take(&mut self.camera_keys);
        let session_life = &mut self.session_life;
        let mut live_npcs = &mut self.live_npcs;
        let mut camera_script = &mut self.camera_script;
        let live_mode = self.live_mode;
        let mut spline_finished = self.spline_finished;
        // client.mainUpdate: the title screen's camera stands still in
        // game state 28 and while a login is under way.
        let camera_held = (!title_world
            .camera_runs(session_life.as_ref().is_some_and(|life| life.login_busy())))
        .then(|| (camera_script.clone(), spline_finished));
        {
            let (regions, origin) = (&workbench.regions, workbench.navigation.origin);
            let plane = workbench.ground.plane;
            // Protocol.loop 2895-2901: a spline camera (type 3) advances
            // every cycle, in game as on the login screen. The next
            // cycle's interface walk reads `Camera.splineJustFinished`
            // for the onMinimapUnlock hooks.
            camera_script.spline_cycle(origin);
            spline_finished.updated(&camera_script);
            // Camera.updatePlayerCamera, while the follow camera is in
            // charge (cameraType 1).
            if !camera_script.locked && camera_script.spline.is_none() {
                scene_occlusion.refresh(&workbench);
                let typed = ui_vm
                    .preferences
                    .keyboard_camera
                    .then(|| std::mem::take(&mut camera_keys));
                follow.cycle(
                    scene_camera::local(motion.fine, origin),
                    plane,
                    typed.as_deref(),
                    &scene_occlusion.scene(&workbench),
                );
            }
            camera_keys.clear();
            camera_script.cycle(|fine| {
                rs09_client::terrain::scene_height(
                    regions,
                    origin,
                    plane,
                    [fine[0] + origin[0] * 128, fine[1] + origin[1] * 128],
                )
            });
            // The drift of the original's in-game loop, one step for the
            // minimap and the camera (Protocol.java:2920-2975). The
            // offline workbench has no login and keeps a steady view.
            rs09_client::session_drift::cycle(&mut drift, live_mode.then_some(&mut follow), || {
                drift_random.next_double()
            });
        }
        match camera_held {
            Some(held) => (*camera_script, spline_finished) = held,
            None => title_world.camera_cycle(title_world::Scene {
                world: &mut workbench,
                map: world_map.as_mut(),
                camera: &mut camera_script,
                npcs: &mut live_npcs,
                motion: &mut motion,
                atmosphere: &mut atmosphere,
            }),
        }
        self.camera_keys = camera_keys;
        self.spline_finished = spline_finished;
    }

    pub(super) fn cycle_interfaces(&mut self) {
        let mut ui = &mut self.ui;
        let ui_vm = &mut self.ui_vm;
        let camera_keys = &mut self.camera_keys;
        let context_menu = &mut self.context_menu;
        let input = &mut self.input;
        let ui_drag = &mut self.ui_drag;
        let show_ui = self.show_ui;
        let muted = self.muted;
        let mut spline_finished = self.spline_finished;
        let ui_cycle = self.ui_cycle;
        let wheel_rotation = self.wheel_rotation;
        if let Some(ui) = &mut ui {
            let mut pointer = rs09_client::ui::state::PointerInput {
                scroll: if show_ui { wheel_rotation } else { 0 },
                suppressed: context_menu.menu.open || ui_drag.active() || !show_ui,
                ..Default::default()
            };
            // The world map's staff Ctrl-click reads these during the pass.
            ui.world_map.ctrl_held = show_ui
                && (is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl));
            ui.world_map.staff_level = ui_vm.rights;
            input.pointer(&mut pointer, show_ui);
            ui.key_events.clear();
            if show_ui {
                ui.key_events.extend_from_slice(&input.keys);
            }
            ui_vm.modifier_keys = input.modifiers();
            ui_vm.cycle = ui_cycle;
            ui_vm.sound_volume = Some(if muted {
                0
            } else {
                ui_vm.preferences.sound_volume
            });
            if let Err(e) = ui_vm.poll_updates(ui, i64::from(ui_cycle) * 20) {
                ui.diagnostic(e);
            }
            ui.advance_inventory_press();
            ui.spline_just_finished = spline_finished.read();
            // InterfaceList.keyCodes, for Camera.updatePlayerCamera.
            camera_keys.extend(ui.key_events.iter().map(|event| event[0]));
            // Component key bindings click during the interface walk,
            // before the hooks it queues. A hidden interface gets no keys.
            let mut click = |ui: &mut rs09_client::ui::state::State, key, op| {
                if let Err(e) = ui_vm.click_button(ui, key, op, "") {
                    ui.diagnostic(e);
                }
            };
            ui.queue_cycle_events(
                Some(pointer),
                show_ui.then_some(rs09_client::ui::state::KeyInput {
                    pressed: &input.keyboard.pressed_keys,
                    cycle: ui_cycle,
                    click: &mut click,
                }),
            );
        }
        self.spline_finished = spline_finished;
    }

    pub(super) fn cycle_hooks(&mut self) {
        let workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let ui_audio = &mut self.ui_audio;
        let ui_vm = &mut self.ui_vm;
        let motion = &mut self.motion;
        let destination_flag = &mut self.destination_flag;
        let follow = &mut self.follow;
        let scene_occlusion = &mut self.scene_occlusion;
        let context_menu = &mut self.context_menu;
        let mut outbox = &mut self.outbox;
        let player_state = &mut self.player_state;
        let actor_states = &mut self.actor_states;
        let camera_script = &mut self.camera_script;
        let actor_sounds = &mut self.actor_sounds;
        let cycle_driver = &mut self.cycle_driver;
        let input_events = &mut self.input_events;
        let ui_drag = &mut self.ui_drag;
        let capture = self.capture;
        let mut route_run = self.route_run;
        let mut manual = self.manual;
        let ui_cycle = self.ui_cycle;
        let mut ui_scene_delta = self.ui_scene_delta;
        let mut display_shown = self.display_shown;
        let running = self.running;
        let self_size = self.self_size;
        let plane = self.plane;
        let mut route_steps = std::mem::take(&mut self.route_steps);
        let ui_nodes = std::mem::take(&mut self.ui_nodes);
        macro_rules! display_effects {
            () => {
                display_effects(
                    ui_vm,
                    cycle_driver,
                    input_events,
                    capture,
                    &mut display_shown,
                )
            };
        }
        if let Some(ui) = &mut ui {
            ui_vm.drain(ui);
            ui.finish_input_events();
            ui_audio.enqueue(&mut ui_vm.effects, ui_cycle);
            // CS2 3101: scripts animate the local player through the same
            // priority rule as a server animation mask.
            for [sequence, delay] in ui_vm.self_animations.drain(..) {
                let own = rs09_protocol::game::players::SELF;
                let result = workbench
                    .player_sequence(&ui.pack, sequence)
                    .and_then(|()| {
                        actor_states
                            .entry(own)
                            .or_default()
                            .request(
                                sequence,
                                delay,
                                motion.queue.len() as i32,
                                &workbench.sequences,
                            )
                            .map_err(|e| e.to_string())
                    });
                match result {
                    Ok(events) => {
                        actor_sounds.extend(events.into_iter().map(|sound| (own, sound, ui_cycle)))
                    }
                    Err(error) => ui.diagnostic(format!("Script animation {sequence}: {error}")),
                }
            }
            // CS2 3109: a script starts dragging its component.
            for (key, offset) in ui_vm.drag_requests.drain(..) {
                ui_drag.begin_scripted(
                    ui,
                    &ui_nodes,
                    key,
                    offset,
                    ui_cycle,
                    context_menu.menu.open,
                );
            }
            // CS2 3107: a scripted click on a named player's option.
            for (option, name) in std::mem::take(&mut ui_vm.player_option_requests) {
                let key = rs09_client::ui::social::encode(&name);
                let target = player_state.local_ids.iter().copied().find(|&index| {
                    player_state.players[index]
                        .as_ref()
                        .and_then(|player| player.appearance.as_ref())
                        .is_some_and(|appearance| appearance.name37 == key)
                });
                match target {
                    Some(index) => {
                        // ClientProt.clickPlayerOption asks for a route
                        // to the player before it writes the option.
                        let origin = workbench.navigation.origin;
                        let source = motion.head;
                        let route = player_state.players[index].as_ref().and_then(|actor| {
                            rs09_client::interaction::Approach {
                                tile: actor.tile(),
                                size: [1; 2],
                                shape: 0,
                                angle: 0,
                                blocked_sides: 0,
                            }
                            .route_sized(
                                &workbench.navigation.maps[plane],
                                [source[0] - origin[0], source[1] - origin[1]],
                                self_size,
                            )
                        });
                        if let Some(route) = route {
                            motion.pending_move_packet = route.move_packet(origin, running, true);
                            destination_flag.route(&route);
                            queue_packet(&mut outbox, &motion.pending_move_packet);
                            route_steps = route
                                .sent_steps()
                                .iter()
                                .map(|p| [p[0] + origin[0], p[1] + origin[1]])
                                .collect();
                            route_run = running;
                            manual = 808;
                        }
                        ui_vm
                            .outgoing
                            .extend(rs09_client::player_interaction::script_option(
                                option,
                                index as i32,
                            ))
                    }
                    None => {
                        let shown =
                            rs09_client::player_interaction::username(key).unwrap_or_default();
                        ui_vm.add_message(
                            ui,
                            rs09_client::ui::vm::ChatLine {
                                text: format!("Unable to find {shown}"),
                                phrase: -1,
                                ..Default::default()
                            },
                        );
                    }
                }
            }
            // Character design (CS2 403/404/410) edits the local player's
            // appearance; the next player update rebuilds the body from it.
            for edit in ui_vm.design_edits.drain(..) {
                let pack = &ui.pack;
                if let Some(appearance) = &mut player_state.self_player_mut().appearance {
                    let result = appearance.design(edit, |kit| {
                        pack.manifest.files.contains_key(&format!("kit/{kit}"))
                    });
                    if let Err(error) = result {
                        ui.diagnostic(format!("Character design: {error}"));
                    }
                }
            }
            // Script camera commands, and the facts scripts read back.
            for request in ui_vm.camera_requests.drain(..) {
                let (regions, origin) = (&workbench.regions, workbench.navigation.origin);
                let plane = workbench.ground.plane;
                let angle = camera_script.script(request, origin, |fine| {
                    rs09_client::terrain::scene_height(
                        regions,
                        origin,
                        plane,
                        [fine[0] + origin[0] * 128, fine[1] + origin[1] * 128],
                    )
                });
                if let Some([units_yaw, units_pitch]) = angle {
                    follow.set_angles(
                        f64::from(units_pitch),
                        f64::from(units_yaw),
                        workbench.ground.plane,
                        &scene_occlusion.scene(&workbench),
                    );
                }
            }
            // CS2 5505 and 5506 push `(int) Camera.pitchTarget` and
            // `(int) Camera.yawTarget`.
            ui_vm.camera_angles = [follow.pitch_target as i32, follow.yaw_target as i32];
            ui_vm.now_ms = (miniquad::date::now() * 1000.0) as i64;
            // A script of this cycle set the window mode (CS2 5300,
            // 5301, 5307): the canvas the cycle's layout below uses and
            // the window status this cycle writes.
            if !ui_vm.display.effects.is_empty() {
                display_effects!();
            }
            ui_scene_delta = ui_scene_delta.wrapping_add(1);
            // The end of this cycle of Protocol.loop: the packets its
            // scripts made are its own, written before the idle logout
            // and the verification id.
            for packet in ui_vm.outgoing.drain(..) {
                if outbox.len() >= 512 {
                    outbox.remove(0);
                }
                outbox.push(packet);
            }
        }
        self.route_run = route_run;
        self.manual = manual;
        self.ui_scene_delta = ui_scene_delta;
        self.display_shown = display_shown;
        self.route_steps = route_steps;
        self.ui_nodes = ui_nodes;
    }

    pub(super) fn cycle_drag(&mut self, inventory: bool) -> Option<MenuEntry> {
        let mut ui = &mut self.ui;
        let ui_vm = &mut self.ui_vm;
        let outbox = &mut self.outbox;
        let input = &mut self.input;
        let ui_drag = &mut self.ui_drag;
        let ui_nodes = std::mem::take(&mut self.ui_nodes);
        // A drag under way: an inventory item (Protocol.java:2720-2778) or a
        // component (2815-2817). The click a press stood for comes back when
        // the button is let go without a drag.
        let mut chosen = None;
        if let Some(ui) = &mut ui {
            if ui_drag.active() && ui_drag.inventory() == inventory {
                let point = [input.mouse.last_mouse_x, input.mouse.last_mouse_y];
                let (hooks, click, packet) = ui_drag.step(
                    ui,
                    &ui_nodes,
                    point,
                    ui_vm.cycle,
                    input.mouse.pressed_button != 0,
                    ui_vm.inserting == 1,
                );
                for hook in hooks {
                    if let Err(e) = ui_vm.run(ui, &hook, 200_000) {
                        ui.diagnostic(e);
                    }
                }
                if let Some(packet) = packet.and_then(|release| release.resolve(ui)) {
                    if outbox.len() >= 512 {
                        outbox.remove(0);
                    }
                    outbox.push(packet);
                }
                if click.is_some() {
                    chosen = click;
                }
            }
        }
        self.ui_nodes = ui_nodes;
        chosen
    }

    pub(super) fn cycle_menu_click(&mut self) -> Option<MenuEntry> {
        let workbench = &mut self.workbench;
        let ui = &mut self.ui;
        let ui_renderer = &mut self.ui_renderer;
        let ui_vm = &mut self.ui_vm;
        let context_menu = &mut self.context_menu;
        let input = &mut self.input;
        let ui_drag = &mut self.ui_drag;
        let show_ui = self.show_ui;
        let paused = self.paused;
        let mut menu_trims_seen = self.menu_trims_seen;
        let mut hover_menu = std::mem::take(&mut self.hover_menu);
        let ui_nodes = std::mem::take(&mut self.ui_nodes);
        let menu_click = &mut self.menu_click;
        // Each sub-interface opened or closed since the last frame trims the
        // open menu, before this frame's click is looked at.
        if let Some(ui) = &ui {
            let changes = ui.subinterface_changes.wrapping_sub(menu_trims_seen);
            menu_trims_seen = ui.subinterface_changes;
            for _ in 0..changes {
                rs09_client::context_menu::remove_interface_actions(&mut hover_menu.entries);
            }
            if context_menu.menu.open && changes != 0 {
                match ui_renderer.menu_measure(&ui.pack, &workbench.pack) {
                    Ok(measure) => {
                        for _ in 0..changes {
                            context_menu.interface_changed(&measure);
                        }
                    }
                    Err(_) => context_menu.close(),
                }
            }
        }
        let menu_was_open = context_menu.menu.open;
        // `Cheat.shiftClick && Keyboard.pressedKeys[KEY_SHIFT]`: the left
        // click, the hover text and the cursor then all take the entry below
        // the top one (`context_menu::top_entry`).
        let shift_click = ui_vm.cheat.shift_click
            && input.keyboard.pressed_keys[rs09_client::input::keyboard::KEY_SHIFT];
        // Protocol.processDefaultAction (Protocol.java:3534-3609), on the menu
        // the last redraw built: nothing during a drag; an open menu takes
        // the click; otherwise a press may start a drag, a right click opens
        // the menu and a left click runs its top entry, each once the menu
        // in hand is the one built where the click was.
        let mouse = &mut input.mouse;
        let click = [mouse.click_x, mouse.click_y];
        // The paused workbench with its interface hidden takes no left click.
        let button = if mouse.click_button == 1 && paused && !show_ui {
            0
        } else {
            mouse.click_button
        };
        let mut chosen = None;
        if ui_drag.active() {
        } else if menu_was_open {
            chosen = context_menu.step_at(
                match button {
                    1 => rs09_client::context_menu::Button::Left,
                    2 => rs09_client::context_menu::Button::Right,
                    _ => rs09_client::context_menu::Button::None,
                },
                [mouse.last_mouse_x, mouse.last_mouse_y],
                click,
            );
        } else {
            let size = hover_menu.entries.len();
            // MiniMenu.processClick: with Cheat.shiftClick, Shift runs the
            // option below the top one.
            let top = rs09_client::context_menu::top_entry(size, shift_click)
                .map(|index| hover_menu.entries[index].clone());
            let drag = button == 1
                && show_ui
                && ui.as_ref().is_some_and(|ui| {
                    ui_drag.begin(ui, &ui_nodes, click, ui_vm.cycle, top.clone())
                });
            if !drag {
                let menu_instead = top.as_ref().is_some_and(|e| {
                    (ui_vm.mouse_buttons == 1 && size > 2) || e.action == 1003 || e.action == 3003
                });
                let decision = menu_click.default_action(mouse, button, size, menu_instead);
                if decision.open {
                    let width = ui.as_ref().map(|ui| {
                        ui_renderer
                            .menu_width(&ui.pack, &workbench.pack, &hover_menu)
                            .unwrap_or(160)
                    });
                    context_menu.open_at(hover_menu.clone(), width, menu_click.interface_mouse);
                }
                if decision.act {
                    chosen = top;
                }
            }
        }
        self.menu_trims_seen = menu_trims_seen;
        self.hover_menu = hover_menu;
        self.ui_nodes = ui_nodes;
        chosen
    }

    pub(super) fn release_opens_menu(&mut self, entry: &MenuEntry) -> bool {
        let workbench = &mut self.workbench;
        let ui = &mut self.ui;
        let ui_renderer = &mut self.ui_renderer;
        let ui_vm = &mut self.ui_vm;
        let context_menu = &mut self.context_menu;
        let hover_menu = std::mem::take(&mut self.hover_menu);
        let menu_click = &mut self.menu_click;
        // Protocol.java:2769-2771: the click an inventory press stood for
        // opens the menu instead under the rule of a plain left click.
        let opens = !context_menu.menu.open
            && ((ui_vm.mouse_buttons == 1 && hover_menu.entries.len() > 2)
                || entry.action == 1003
                || entry.action == 3003);
        if opens {
            let width = ui.as_ref().map(|ui| {
                ui_renderer
                    .menu_width(&ui.pack, &workbench.pack, &hover_menu)
                    .unwrap_or(160)
            });
            context_menu.open_at(hover_menu.clone(), width, menu_click.interface_mouse);
        }
        self.hover_menu = hover_menu;
        opens
    }

    pub(super) fn do_action(&mut self, chosen: Option<MenuEntry>) -> Option<[i32; 2]> {
        let walk_spell = &mut self.walk_spell;
        let workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let ui_vm = &mut self.ui_vm;
        let motion = &mut self.motion;
        let destination_flag = &mut self.destination_flag;
        let mut outbox = &mut self.outbox;
        let player_state = &mut self.player_state;
        let live_npcs = &mut self.live_npcs;
        let ui_drag = &mut self.ui_drag;
        let live_mode = self.live_mode;
        let mut route_run = self.route_run;
        let mut manual = self.manual;
        let running = self.running;
        let self_size = self.self_size;
        let plane = self.plane;
        let mut route_status = std::mem::take(&mut self.route_status);
        let mut route_steps = std::mem::take(&mut self.route_steps);
        let mut click_cross = std::mem::take(&mut self.click_cross);
        let mut ui_nodes = std::mem::take(&mut self.ui_nodes);
        let walk = &mut self.walk;
        let origin = workbench.navigation.origin;
        let mut menu_action_failed = false;
        if let Some(entry) = &chosen {
            let mut normalized = entry.clone();
            if normalized.action >= 2000 {
                normalized.action -= 2000;
            }
            let entry = &normalized;
            if entry.action == 32 {
                if let Some(ui) = &mut ui {
                    if let Err(error) = ui_vm.select_target(
                        ui,
                        rs09_client::ui::state::Key {
                            id: entry.args[1],
                            child: entry.args[0],
                        },
                    ) {
                        ui.diagnostic(error);
                    }
                }
            }
            let mut ground_packet = rs09_client::ground_interaction::packet(
                entry.action,
                entry.key as i32,
                [entry.args[0] + origin[0], entry.args[1] + origin[1]],
                ui.as_ref().and_then(|u| u.selected_item.as_ref()),
            );
            if entry.action == 39 {
                ground_packet = ui.as_ref().and_then(|u| {
                    u.targeting.packet(
                        39,
                        entry.key as i32,
                        [entry.args[0] + origin[0], entry.args[1] + origin[1]],
                    )
                });
            }
            if let Some(packet) = ground_packet {
                click_cross = Some(ClickCross::new(ClickKind::Interact));
                if entry.action != 1002 {
                    let source = motion.head;
                    if let Some(route) = rs09_client::ground_interaction::route_sized(
                        &workbench.navigation.maps[plane],
                        [source[0] - origin[0], source[1] - origin[1]],
                        entry.args,
                        self_size,
                    ) {
                        motion.pending_move_packet = route.move_packet(origin, running, true);
                        destination_flag.route(&route);
                        queue_packet(&mut outbox, &motion.pending_move_packet);
                        route_steps = route
                            .sent_steps()
                            .iter()
                            .map(|p| [p[0] + origin[0], p[1] + origin[1]])
                            .collect();
                        route_run = running;
                        manual = 808;
                    }
                }
                if outbox.len() >= 512 {
                    outbox.remove(0);
                }
                outbox.push(packet);
                route_status = format!(
                    "{}: {}",
                    if live_mode { "Sent" } else { "Offline" },
                    client_ui::label(entry)
                );
            }
            if let Some(npc) = workbench.npcs.iter().find(|n| {
                n.index == entry.key as usize
                    && (!live_mode || live_npcs.local_ids.contains(&n.index))
            }) {
                // Examine names the definition the NPC currently shows
                // (NpcType.getMultiNpc), and sends nothing when it shows none.
                let definition = if entry.action == 1007 {
                    npc.render_id
                } else {
                    Some(npc.id)
                };
                let mut packet = definition.and_then(|definition| {
                    rs09_client::npc_interaction::packet(
                        entry.action,
                        entry.key as i32,
                        definition as i32,
                    )
                });
                if entry.action == 26 {
                    packet = ui
                        .as_ref()
                        .and_then(|u| u.selected_item.as_ref())
                        .map(|selected| selected.on_npc(entry.key as i32));
                }
                if entry.action == 45 {
                    packet = ui
                        .as_ref()
                        .and_then(|u| u.targeting.packet(45, entry.key as i32, entry.args));
                }
                if let Some(packet) = packet {
                    click_cross = Some(ClickCross::new(ClickKind::Interact));
                    if entry.action != 1007 {
                        let tile = npc.motion.head;
                        let source = motion.head;
                        let approach = rs09_client::interaction::Approach {
                            tile: [tile[0] - origin[0], tile[1] - origin[1]],
                            size: [workbench.npc_definitions[&npc.id].size; 2],
                            shape: 0,
                            angle: 0,
                            blocked_sides: 0,
                        };
                        if let Some(route) = approach.route_sized(
                            &workbench.navigation.maps[plane],
                            [source[0] - origin[0], source[1] - origin[1]],
                            self_size,
                        ) {
                            motion.pending_move_packet = route.move_packet(origin, running, true);
                            destination_flag.route(&route);
                            queue_packet(&mut outbox, &motion.pending_move_packet);
                            route_steps = route
                                .sent_steps()
                                .iter()
                                .map(|p| [p[0] + origin[0], p[1] + origin[1]])
                                .collect();
                            route_run = running;
                            manual = 808;
                        }
                    }
                    if outbox.len() >= 512 {
                        outbox.remove(0);
                    }
                    outbox.push(packet);
                    route_status = format!("Offline: {}", client_ui::label(entry));
                }
            }
            if let Some(actor) = player_state
                .players
                .get(entry.key as usize)
                .and_then(|p| p.as_ref())
                .filter(|_| player_state.local_ids.contains(&(entry.key as usize)))
            {
                let packet = match entry.action {
                    1 => ui
                        .as_ref()
                        .and_then(|u| u.selected_item.as_ref())
                        .map(|s| s.on_player(entry.key as i32)),
                    15 => ui
                        .as_ref()
                        .and_then(|u| u.targeting.packet(15, entry.key as i32, entry.args)),
                    _ => rs09_client::player_interaction::packet(entry.action, entry.key as i32),
                };
                if let Some(packet) = packet {
                    click_cross = Some(ClickCross::new(ClickKind::Interact));
                    let source = motion.head;
                    let approach = rs09_client::interaction::Approach {
                        tile: actor.tile(),
                        size: [1; 2],
                        shape: 0,
                        angle: 0,
                        blocked_sides: 0,
                    };
                    if let Some(route) = approach.route_sized(
                        &workbench.navigation.maps[plane],
                        [source[0] - origin[0], source[1] - origin[1]],
                        self_size,
                    ) {
                        motion.pending_move_packet = route.move_packet(origin, running, true);
                        destination_flag.route(&route);
                        queue_packet(&mut outbox, &motion.pending_move_packet);
                        route_steps = route
                            .sent_steps()
                            .iter()
                            .map(|p| [p[0] + origin[0], p[1] + origin[1]])
                            .collect();
                        route_run = running;
                        manual = 808;
                    }
                    if outbox.len() >= 512 {
                        outbox.remove(0);
                    }
                    outbox.push(packet);
                    route_status = format!(
                        "{}: {}",
                        if live_mode { "Sent" } else { "Offline" },
                        client_ui::label(entry)
                    );
                }
            }
            if let Some(ui) = &mut ui {
                if entry.action == 28 {
                    ui_vm.close_modal(ui);
                }
                if entry.action == 22 {
                    ui_vm.cancel_targeting(ui);
                    ui.selected_item = Some(rs09_client::ui::actions::SelectedItem {
                        id: entry.key as i32,
                        slot: entry.args[0],
                        component: entry.args[1],
                        text: format!("{}<col=ffffff>", entry.target),
                    });
                } else if entry.action == 40 {
                    if let Some(selected) = &ui.selected_item {
                        if outbox.len() >= 512 {
                            outbox.remove(0);
                        }
                        outbox.push(selected.on_item(
                            entry.key as i32,
                            entry.args[0],
                            entry.args[1],
                        ));
                    }
                }
                // A spell cast on the world (key 0) needs the mouse on a
                // drawn tile and shows the yellow cross (Protocol.loop,
                // minimapWalkState 2); one cast on the minimap (key 1) does
                // neither.
                // MiniMenu.doAction, action 11 with key 0 (MiniMenu.java:923-
                // 926): the spell waits, as a walk does, for the scene draw
                // that finds the tile under the click.
                if entry.action == 11 && entry.key == 0 {
                    walk.minimap_state = 1;
                    walk.set_destination(entry.args);
                    *walk_spell = Some(ui.targeting.clone());
                }
                if matches!(entry.action, 3 | 12 | 11) && !(entry.action == 11 && entry.key == 0) {
                    let args = if entry.action == 11 {
                        [entry.args[0] + origin[0], entry.args[1] + origin[1]]
                    } else {
                        entry.args
                    };
                    if let Some(packet) = ui.targeting.packet(entry.action, entry.key as i32, args)
                    {
                        if outbox.len() >= 512 {
                            outbox.remove(0);
                        }
                        outbox.push(packet);
                        if entry.action == 11 && entry.key == 0 {
                            click_cross = Some(ClickCross::new(ClickKind::Walk));
                        }
                    }
                }
            }
            let item_packet = if matches!(entry.action, 8 | 51 | 59) {
                if let Some(ui) = &mut ui {
                    if let Err(error) =
                        ui_vm.legacy_button(ui, entry.action, entry.args[1], |packet| {
                            if outbox.len() >= 512 {
                                outbox.remove(0);
                            }
                            outbox.push(packet);
                            Ok(())
                        })
                    {
                        ui.diagnostic(error);
                        menu_action_failed = true;
                    }
                    None
                } else {
                    rs09_client::ui::actions::item_packet(
                        entry.action,
                        entry.key as i32,
                        entry.args[0],
                        entry.args[1],
                    )
                }
            } else if entry.action == 1006 {
                if let Some(ui) = &mut ui {
                    if let Err(error) = ui_vm.examine_inventory(
                        ui,
                        entry.key as i32,
                        entry.args[0],
                        entry.args[1],
                        |packet| {
                            if outbox.len() >= 512 {
                                outbox.remove(0);
                            }
                            outbox.push(packet);
                            Ok(())
                        },
                    ) {
                        ui.diagnostic(error);
                        menu_action_failed = true;
                    }
                    route_status = format!("Examine: {}", entry.target);
                    None
                } else {
                    rs09_client::ui::actions::item_packet(
                        1006,
                        entry.key as i32,
                        entry.args[0],
                        entry.args[1],
                    )
                }
            } else if entry.action == 41 {
                ui.as_mut().and_then(|ui| {
                    ui.request_continue(rs09_client::ui::state::Key {
                        id: entry.args[1],
                        child: entry.args[0],
                    })
                })
            } else {
                rs09_client::ui::actions::item_packet(
                    entry.action,
                    entry.key as i32,
                    entry.args[0],
                    entry.args[1],
                )
            };
            if let Some(packet) = item_packet {
                if outbox.len() >= 512 {
                    outbox.remove(0);
                }
                outbox.push(packet);
                route_status = format!("Offline: {}", client_ui::label(entry));
            }
            if matches!(entry.action, 9 | 1003) {
                if let Some(ui) = &mut ui {
                    let key = rs09_client::ui::state::Key {
                        id: entry.args[1],
                        child: entry.args[0],
                    };
                    if let Err(e) = ui_vm.click_button(ui, key, entry.key as i32, &entry.target) {
                        ui.diagnostic(e);
                    }
                    route_status = format!("Offline: {}", client_ui::label(entry));
                }
            }
            let mut location_packet = rs09_client::interaction::location_packet(
                entry.action,
                if entry.action == 1004 {
                    entry.key as i32
                } else {
                    (entry.key >> 32) as i32 & i32::MAX
                },
                [entry.args[0] + origin[0], entry.args[1] + origin[1]],
            );
            if entry.action == 14 {
                if let Some(selected) = ui.as_ref().and_then(|u| u.selected_item.as_ref()) {
                    let frame = selected.on_location(
                        (entry.key >> 32) as i32 & i32::MAX,
                        [entry.args[0] + origin[0], entry.args[1] + origin[1]],
                    );
                    location_packet =
                        Some(std::iter::once(frame.opcode).chain(frame.payload).collect());
                }
            }
            if entry.action == 38 {
                location_packet = ui
                    .as_ref()
                    .and_then(|u| {
                        u.targeting.packet(
                            38,
                            (entry.key >> 32) as i32 & i32::MAX,
                            [entry.args[0] + origin[0], entry.args[1] + origin[1]],
                        )
                    })
                    .map(|frame| std::iter::once(frame.opcode).chain(frame.payload).collect());
            }
            if let Some(packet) = location_packet {
                click_cross = Some(ClickCross::new(ClickKind::Interact));
                if entry.action != 1004 {
                    let id = (entry.key >> 32) as u32 & i32::MAX as u32;
                    if let Some(d) = workbench.location_types.get(&id) {
                        let target = rs09_client::interaction::LocationTarget {
                            id,
                            tile: [entry.args[0] + origin[0], entry.args[1] + origin[1]],
                            plane: 0,
                            shape: (entry.key >> 14 & 31) as u8,
                            rotation: (entry.key >> 20 & 3) as u8,
                            size: d.size,
                            blocked_sides: d.blocked_sides,
                        };
                        let source = motion.head;
                        if let Some(route) = target.approach(origin).route_sized(
                            &workbench.navigation.maps[plane],
                            [source[0] - origin[0], source[1] - origin[1]],
                            self_size,
                        ) {
                            motion.pending_move_packet = route.move_packet(origin, running, true);
                            destination_flag.route(&route);
                            queue_packet(&mut outbox, &motion.pending_move_packet);
                            route_steps = route
                                .sent_steps()
                                .iter()
                                .map(|p| [p[0] + origin[0], p[1] + origin[1]])
                                .collect();
                            route_run = running;
                            manual = 808;
                        }
                    }
                }
                queue_packet(&mut outbox, &packet);
                route_status = format!("Offline: {}", client_ui::label(entry));
            }
            if let Some(ui) = ui
                .as_mut()
                .filter(|_| entry.action != 1006 && !menu_action_failed)
            {
                if let Err(error) =
                    ui.capture_inventory_press(entry.action, entry.args[1], entry.args[0])
                {
                    ui.diagnostic(error);
                    menu_action_failed = true;
                }
            }
            if !menu_action_failed && !matches!(entry.action, 22 | 32) {
                if let Some(ui) = &mut ui {
                    ui.selected_item = None;
                    ui_vm.cancel_targeting(ui);
                    ui.finish_inventory_action();
                }
            }
        }
        if ui_drag.take_inventory_release() && !menu_action_failed {
            if let Some(ui) = &mut ui {
                ui.released_inventory();
            }
        }
        if chosen.is_some() {
            if let Some(ui) = &mut ui {
                ui_nodes = ui
                    .layout(screen_width() as i32, screen_height() as i32)
                    .unwrap_or_else(|e| {
                        ui.diagnostic(e);
                        Vec::new()
                    });
            }
        }
        for packet in ui_vm.outgoing.drain(..) {
            if outbox.len() >= 512 {
                outbox.remove(0);
            }
            outbox.push(packet);
        }
        // MiniMenu.doAction, "Walk here" (MiniMenu.java:607-640): in the scene
        // (key 0) the click's point is remembered until the next scene draw
        // has found the tile under it; on the minimap (key 1) the tile is
        // known and the walk starts now.
        let mut minimap_walk = None;
        if let Some(entry) = &chosen {
            if entry.action == 60 || entry.action == 2060 {
                if entry.key == 0 {
                    walk.set_destination(entry.args);
                } else {
                    minimap_walk = Some([entry.args[0] + origin[0], entry.args[1] + origin[1]]);
                }
            }
        }
        self.route_run = route_run;
        self.manual = manual;
        self.route_status = route_status;
        self.route_steps = route_steps;
        self.click_cross = click_cross;
        self.ui_nodes = ui_nodes;
        minimap_walk
    }

    pub(super) fn staff_keys(&mut self) -> bool {
        let ui_vm = &mut self.ui_vm;
        let input = &mut self.input;
        // Protocol.java:2818-2833: a staff member holding Ctrl and Shift.
        let pressed = &input.keyboard.pressed_keys;
        rs09_client::cheat::staff_teleport(
            ui_vm.rights,
            pressed[rs09_client::input::keyboard::KEY_CTRL],
            pressed[rs09_client::input::keyboard::KEY_SHIFT],
        )
    }

    pub(super) fn wheel_plane(&mut self) -> Option<[i32; 3]> {
        let motion = &mut self.motion;
        let wheel_rotation = self.wheel_rotation;
        let plane = self.plane;
        // Protocol.java:2818-2827: the wheel moves staff a plane down or up.
        (wheel_rotation != 0).then(|| {
            let level = rs09_client::cheat::wheel_plane(plane as i32, wheel_rotation);
            [motion.head[0], motion.head[1], level]
        })
    }

    pub(super) fn staff_teleport(&mut self, tiles: &[[i32; 3]]) {
        let ui_vm = &mut self.ui_vm;
        for &[x, y, level] in tiles {
            let context = ui_vm.cheat.context(ui_vm.window_mode != 0);
            let (command, effects) = rs09_client::cheat::teleport(x, y, level, &context);
            if ui_vm.cheat.apply(effects, &mut ui_vm.preferences).is_ok() {
                if let Ok(packet) = rs09_client::ui::chat::command(&command[2..]) {
                    ui_vm.outgoing.push(packet);
                }
            }
        }
    }

    pub(super) fn cast_on_tile(&mut self, tile: [i32; 2]) {
        let outbox = &mut self.outbox;
        let mut click_cross = std::mem::take(&mut self.click_cross);
        let walk_spell = &mut self.walk_spell;
        // Protocol.java:2834-2846: the spell cast on the world reaches its
        // tile, with the yellow cross.
        if let Some(packet) = walk_spell
            .take()
            .and_then(|targeting| targeting.packet(11, 0, tile))
        {
            if outbox.len() >= 512 {
                outbox.remove(0);
            }
            outbox.push(packet);
            click_cross = Some(ClickCross::new(ClickKind::Walk));
        }
        self.click_cross = click_cross;
    }

    pub(super) fn walk_to(&mut self, destination: [i32; 2], kind: WalkKind) {
        let workbench = &mut self.workbench;
        let minimap = &mut self.minimap;
        let motion = &mut self.motion;
        let destination_flag = &mut self.destination_flag;
        let mut outbox = &mut self.outbox;
        let mut route_run = self.route_run;
        let mut manual = self.manual;
        let running = self.running;
        let self_size = self.self_size;
        let plane = self.plane;
        let route_status;
        let mut route_steps = std::mem::take(&mut self.route_steps);
        let mut click_cross = std::mem::take(&mut self.click_cross);
        let source = motion.head;
        workbench.refresh_navigation(source);
        let origin = workbench.navigation.origin;
        if let Some(route) = walking::find_path_sized(
            &workbench.navigation.maps[plane],
            [source[0] - origin[0], source[1] - origin[1]],
            [destination[0] - origin[0], destination[1] - origin[1]],
            true,
            self_size,
        ) {
            if kind == WalkKind::World {
                click_cross = Some(ClickCross::new(ClickKind::Walk));
            }
            motion.pending_move_packet =
                if let Some(map) = minimap.as_ref().filter(|_| kind == WalkKind::Minimap) {
                    map.view.walk_packet(
                        &route,
                        origin,
                        running,
                        map.click_offset,
                        [
                            motion.fine[0] - origin[0] * 128,
                            motion.fine[1] - origin[1] * 128,
                        ],
                    )
                } else {
                    route.packet(origin, running)
                };
            destination_flag.route(&route);
            queue_packet(&mut outbox, &motion.pending_move_packet);
            route_steps = route
                .sent_steps()
                .iter()
                .map(|p| [p[0] + origin[0], p[1] + origin[1]])
                .collect();
            route_run = running;
            route_status = format!(
                "Walk {}, {}{}",
                destination[0],
                destination[1],
                if route.approximate {
                    " (nearest reachable)"
                } else {
                    ""
                }
            );
            manual = 808;
        } else {
            route_status = String::from("No reachable route");
        }
        self.route_run = route_run;
        self.manual = manual;
        self.route_status = route_status;
        self.route_steps = route_steps;
        self.click_cross = click_cross;
    }

    pub(super) fn cycle_write(&mut self) {
        let ui = &mut self.ui;
        let ui_vm = &mut self.ui_vm;
        let follow = &mut self.follow;
        let outbox = &mut self.outbox;
        let session_life = &mut self.session_life;
        let connection = &mut self.connection;
        let cycle_driver = &mut self.cycle_driver;
        let mut input = &mut self.input;
        // The scene of the last rebuild is built: client.gameState is 30, not 25.
        macro_rules! game_state_30 {
            () => {
                session_life.as_ref().is_none_or(|life| {
                    life.life.state() == rs09_client::lifecycle::GameState::InGame
                })
            };
        }
        // What one 20 ms cycle of the loop's own packets reads and writes.
        macro_rules! cycle_state {
            ($verify_id:expr) => {
                client_cycle::CycleState {
                    // ClientProt 21 reports `(int)` of the camera targets.
                    camera: [follow.pitch_target as i32, follow.yaw_target as i32],
                    preferences: &mut ui_vm.preferences,
                    verify_id: $verify_id,
                    logout_timer: &mut ui_vm.logout_on_disconnect_timer,
                    window_mode: ui_vm.window_mode,
                    scene_ready: game_state_30!(),
                }
            };
        }
        // The end of Protocol.loop: the cycle's packets are its own, written
        // once, the idle logout, the keep-alive and the verification id last.
        let verify_id = ui.as_ref().map_or(0, |ui| ui.verify_id);
        cycle_driver.cycle_end(
            &mut input,
            cycle_state!(verify_id),
            connection.as_mut(),
            &mut outbox.pending,
        );
    }

    pub(super) fn cycle_finish(&mut self, interfaces: bool) {
        let mut ui = &mut self.ui;
        let mut ui_cycle = self.ui_cycle;
        let mut ui_nodes = std::mem::take(&mut self.ui_nodes);
        ui_cycle = ui_cycle.wrapping_add(1);
        if interfaces {
            if let Some(ui) = &mut ui {
                ui_nodes = ui
                    .layout(screen_width() as i32, screen_height() as i32)
                    .unwrap_or_default();
            }
        }
        self.ui_cycle = ui_cycle;
        self.ui_nodes = ui_nodes;
    }

    pub(super) fn prepare_sequences(&mut self) {
        let workbench = &mut self.workbench;
        let ui = &mut self.ui;
        let live_npcs = &mut self.live_npcs;
        let actor_states = &mut self.actor_states;
        let live_diagnostics = &mut self.live_diagnostics;
        if let Some(ui) = &ui {
            let ids: std::collections::BTreeSet<_> = actor_states
                .values()
                .chain(live_npcs.actors.values().map(|n| &n.animation))
                .flat_map(|s| {
                    std::iter::once(s.action.id).chain(s.slots.iter().flatten().map(|t| t.id))
                })
                .filter(|&id| id != -1)
                .collect();
            for id in ids {
                if workbench.prepared_actor_sequences.insert(id) {
                    if let Err(error) = workbench.player_sequence(&ui.pack, id) {
                        let message = format!("Actor sequence{id} assets: {error}");
                        if live_diagnostics.len() < 32 && !live_diagnostics.contains(&message) {
                            live_diagnostics.push(message);
                        }
                    }
                }
            }
        }
    }
}
