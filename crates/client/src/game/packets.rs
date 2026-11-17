// The statements below were written when these fields were locals of the
// main loop and are kept as they were: a field bound to a local of its own
// name is a reference, so `&mut name` borrows where the compiler would not
// need it to.
#![allow(clippy::needless_borrow)]
use super::*;

impl Game {
    pub(super) fn handle_packet(&mut self, server_frame: rs09_protocol::game::Frame) -> bool {
        let workbench = &mut self.workbench;
        let mut ui = &mut self.ui;
        let ui_models = &mut self.ui_models;
        let ground_renderer = &mut self.ground_renderer;
        let mut ui_audio = &mut self.ui_audio;
        let ui_vm = &mut self.ui_vm;
        let mut world_map = &mut self.world_map;
        let mut motion = &mut self.motion;
        let mut destination_flag = &mut self.destination_flag;
        let atmosphere = &mut self.atmosphere;
        let follow = &mut self.follow;
        let scene_occlusion = &mut self.scene_occlusion;
        let context_menu = &mut self.context_menu;
        let mut session_life = &mut self.session_life;
        let mut connection = &mut self.connection;
        let mut player_state = &mut self.player_state;
        let live_actors = &mut self.live_actors;
        let mut live_npcs = &mut self.live_npcs;
        let live_appearances = &mut self.live_appearances;
        let live_bas = &mut self.live_bas;
        let live_decoded = &mut self.live_decoded;
        let motion_appearances = &mut self.motion_appearances;
        let appearance_cache = &mut self.appearance_cache;
        let live_forms = &mut self.live_forms;
        let body_overrides = &mut self.body_overrides;
        let actor_states = &mut self.actor_states;
        let mut camera_script = &mut self.camera_script;
        let actor_sounds = &mut self.actor_sounds;
        let mut world_sounds = &mut self.world_sounds;
        let live_diagnostics = &mut self.live_diagnostics;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let ambient_audio = &mut self.ambient_audio;
        let streaming_map = self.streaming_map;
        let animation_origin = self.animation_origin;
        let mut manual = self.manual;
        let mut live_base = self.live_base;
        let mut missing_scene_regions = self.missing_scene_regions;
        let ui_time = self.ui_time;
        let mut route_status = std::mem::take(&mut self.route_status);
        let mut route_steps = std::mem::take(&mut self.route_steps);
        let mut read_on = false;
        if let Some(connection) = &mut connection {
            for server_frame in std::iter::once(server_frame) {
                workbench.ground.self_id = connection
                    .world
                    .as_ref()
                    .map_or(0, |world| world.player_index as i32);
                if let Some(life) = &mut session_life {
                    life.life.packet_read(server_frame.opcode);
                    if !rs09_client::lifecycle::original_handles(server_frame.opcode) {
                        life.life.unknown_opcode(server_frame.payload.len());
                        break;
                    }
                }
                let result: Result<(), String> = (|| {
                    use rs09_protocol::game::players::{Field, Movement as WireMove, SELF};
                    match server_frame.opcode {
                        162 | 214 => {
                            // REBUILD_REGION (214) carries a chunk table in
                            // place of a region window; the logical rebuild
                            // (base, local tile, plane) is the same for both.
                            let instance = if server_frame.opcode == 214 {
                                match rs09_protocol::game::events::ServerEvent::decode(
                                    &server_frame,
                                )
                                .map_err(|e| e.to_string())?
                                {
                                    Some(
                                        rs09_protocol::game::events::ServerEvent::DynamicRebuild(
                                            instance,
                                        ),
                                    ) => Some(instance),
                                    _ => return Err("Malformed instanced rebuild".into()),
                                }
                            } else {
                                None
                            };
                            let rebuild = match &instance {
                                Some(instance) => rs09_protocol::game::rebuild::NormalRebuild {
                                    local_x: instance.local_x,
                                    local_y: instance.local_y,
                                    plane: instance.plane,
                                    zone_x: instance.zone_x,
                                    zone_y: instance.zone_y,
                                    keys: vec![],
                                },
                                None => rs09_protocol::game::rebuild::decode_normal(&server_frame)
                                    .map_err(|e| e.to_string())?,
                            };
                            // LoginManager.loadRegion returns at once for the
                            // zone the scene is already centred on.
                            if rs09_client::scene_rebuild::unchanged(live_base, &rebuild) {
                                return Ok(());
                            }
                            if instance.is_some() && !streaming_map {
                                return Err(
                                    "Instanced scenes need map streaming (--world-cache)".into()
                                );
                            }
                            if let Some(life) = &mut session_life {
                                life.life.rebuild_packet(rebuild.zone_x, rebuild.zone_y);
                            }
                            let first_scene = live_base.is_none();
                            // LoginManager.loadRegion: a rebuild to another
                            // base releases a locked or spline camera and
                            // moves the destination flag with the base.
                            if rs09_client::follow_camera::rebuild(
                                &mut destination_flag.tile,
                                live_base,
                                rebuild.base(),
                            ) {
                                camera_script.locked = false;
                                camera_script.spline = None;
                            }
                            let previous_base = live_base;
                            if first_scene {
                                // client.resetGameSessionState runs before the
                                // first rebuild of a session is read.
                                ui_vm.reset_session_varcs();
                            }
                            if let Some(map) = &mut world_map {
                                match &instance {
                                    Some(instance) => map.rebuild_instance(instance)?,
                                    None => map.rebuild(&rebuild)?,
                                }
                            }
                            let base = rs09_client::world::rebuild_logical(
                                &rebuild,
                                &mut live_base,
                                &mut player_state,
                                &mut workbench.ground,
                                &mut workbench.ground_supports,
                            );
                            // The rest of loadRegion: pending location changes,
                            // the scripted camera, the map flag, scene spot
                            // animations and projectiles, and the sound queue.
                            rs09_client::scene_rebuild::apply(
                                previous_base,
                                base,
                                rs09_client::scene_rebuild::Parts {
                                    changes: &mut workbench.world,
                                    spots: &mut workbench.world_spots,
                                    camera: &mut camera_script,
                                    flag: &mut destination_flag,
                                },
                            );
                            ui_audio.clear_queue();
                            ground_renderer.clear();
                            context_menu.menu = rs09_client::interaction::Menu::default();
                            workbench.navigation =
                                Navigation::new(&workbench.regions, &workbench.placements, base);
                            workbench.refresh_ground(ui.as_mut())?;
                            // Login, or a rebuild that moves the scene by more
                            // than its own width (LoginManager.loadRegion).
                            if previous_base.is_none_or(|previous| {
                                (0..2).any(|axis| (base[axis] - previous[axis]).abs() > 104)
                            }) {
                                atmosphere.instant = true;
                            }
                            // Rendered coordinates are absolute. Only the first scene
                            // seeds them; packet 225 applies the move on later rebases.
                            if first_scene {
                                *motion = Movement::new([
                                    base[0] + rebuild.local_x,
                                    base[1] + rebuild.local_y,
                                ]);
                                if let Some(raw) = motion_appearances.get(&SELF) {
                                    let size = rs09_client::player_form::size(
                                        raw,
                                        &workbench.npc_definitions,
                                    )?;
                                    motion.fine = motion.head.map(|n| {
                                        n.wrapping_mul(128).wrapping_add(size.wrapping_mul(64))
                                    });
                                }
                            }
                            if live_decoded.is_empty() {
                                workbench.avatar.clear();
                            }
                            route_steps.clear();
                            missing_scene_regions = 0;
                            for id in rebuild.regions().into_iter().filter(|_| !streaming_map) {
                                if !workbench.regions.iter().any(|r| (r.x << 8) + r.z == id) {
                                    missing_scene_regions += 1;
                                    let message = format!(
                                        "Scene region {id} absent from prepared cache pack"
                                    );
                                    if !live_diagnostics.contains(&message)
                                        && live_diagnostics.len() < 32
                                    {
                                        live_diagnostics.push(message);
                                    }
                                }
                            }
                            if rebuild.plane != 0 && !streaming_map {
                                return Err("Live rendering above plane0 is pending; logical scene state updated".into());
                            }
                        }
                        225 => {
                            let base =
                                live_base.ok_or("Player update arrived before scene rebuild")?;
                            let update = player_state
                                .decode_frame(&server_frame, ui_time as i32)
                                .map_err(|e| e.to_string())?;
                            if workbench.ground.plane != player_state.plane {
                                workbench.ground.plane = player_state.plane;
                                workbench.refresh_ground(ui.as_mut())?;
                                if player_state.plane != 0 && !streaming_map {
                                    let message = format!(
                                        "Live rendering above plane0 is pending (player plane{})",
                                        player_state.plane
                                    );
                                    if !live_diagnostics.contains(&message) {
                                        live_diagnostics.push(message);
                                    }
                                }
                            }
                            for movement in update.movements {
                                let index = match &movement {
                                    WireMove::Step { index, .. }
                                    | WireMove::Teleport { index, .. }
                                    | WireMove::Added { index, .. } => *index,
                                };
                                let actor = if index == SELF {
                                    &mut motion
                                } else {
                                    live_actors.entry(index).or_insert_with(|| {
                                        let t =
                                            player_state.players[index].as_ref().unwrap().tile();
                                        Movement::new([t[0] + base[0], t[1] + base[1]])
                                    })
                                };
                                if !matches!(movement, WireMove::Added { .. }) {
                                    let state = actor_states.entry(index).or_default();
                                    if let Err(error) =
                                        actor.cancel_on_move(state, &workbench.sequences)
                                    {
                                        route_status = error.to_string();
                                    }
                                }
                                match movement {
                                    WireMove::Step {
                                        speed, direction, ..
                                    } => {
                                        let delta = [
                                            [-1, 1],
                                            [0, 1],
                                            [1, 1],
                                            [-1, 0],
                                            [1, 0],
                                            [-1, -1],
                                            [0, -1],
                                            [1, -1],
                                        ][direction];
                                        let source = actor.head;
                                        actor.enqueue(
                                            [source[0] + delta[0], source[1] + delta[1]],
                                            speed as u8,
                                        );
                                    }
                                    WireMove::Teleport { tile, discard, .. } => {
                                        let dest = [base[0] + tile[0], base[1] + tile[1]];
                                        let size = motion_appearances.get(&index).map_or(
                                            Ok(1),
                                            |raw| {
                                                rs09_client::player_form::size(
                                                    raw,
                                                    &workbench.npc_definitions,
                                                )
                                            },
                                        )?;
                                        actor.teleport_actor(
                                            dest,
                                            discard,
                                            size,
                                            actor_states.entry(index).or_default(),
                                        );
                                    }
                                    WireMove::Added { angle, is_new, .. } => {
                                        if is_new {
                                            *actor = Movement::new(base);
                                            actor.fine = base.map(|n| n * 128);
                                            actor.angle = angle;
                                            actor.target_angle = angle;
                                            motion_appearances.remove(&index);
                                            if let Some(raw) = appearance_cache.get(&index) {
                                                rs09_client::player_form::appearance_position(
                                                    actor,
                                                    None,
                                                    raw,
                                                    &workbench.npc_definitions,
                                                )?;
                                                motion_appearances.insert(index, raw.clone());
                                            }
                                        }
                                    }
                                }
                            }
                            for index in update.removed {
                                workbench.overheads.remove(&(false, index));
                                live_actors.remove(&index);
                                live_appearances.remove(&index);
                                live_bas.remove(&index);
                                live_decoded.remove(&index);
                                motion_appearances.remove(&index);
                                live_forms.remove(&index);
                                body_overrides.remove(&index);
                                actor_states.remove(&index);
                            }
                            for extended in update.extended {
                                for field in extended.fields {
                                    match field {
                                        Field::PublicChat {
                                            color_effect,
                                            rights,
                                            payload,
                                        } => {
                                            // The chat block is read before this
                                            // frame's appearance block, so a player
                                            // whose name is not yet known says nothing.
                                            if let Some(name37) = motion_appearances
                                                .get(&extended.index)
                                                .map(|raw| raw.name37)
                                                && let Some(name) =
                                                    rs09_client::player_interaction::username(
                                                        name37,
                                                    )
                                            {
                                                match ui_vm.public_chat(
                                                    ui.as_mut(),
                                                    name37,
                                                    &name,
                                                    color_effect,
                                                    rights,
                                                    &payload,
                                                ) {
                                                    Ok(Some(chat)) => {
                                                        workbench
                                                            .overheads
                                                            .entry((false, extended.index))
                                                            .or_default()
                                                            .chat = Some(chat);
                                                    }
                                                    Ok(None) => {}
                                                    Err(error) => {
                                                        let message =
                                                            format!("Public chat: {error}");
                                                        if !live_diagnostics.contains(&message)
                                                            && live_diagnostics.len() < 32
                                                        {
                                                            live_diagnostics.push(message);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Field::Hit { damage, kind, bar } => {
                                            let v = workbench
                                                .overheads
                                                .entry((false, extended.index))
                                                .or_default();
                                            v.hit(kind, ui_time as i32, damage);
                                            if let Some(bar) = bar {
                                                v.bar = bar;
                                                v.bar_until = (ui_time as i32).wrapping_add(300);
                                            }
                                        }
                                        Field::OverheadText(bytes) => {
                                            // Forced chat is also logged when it
                                            // starts with a tilde or is the local
                                            // player's. A player with no appearance
                                            // yet has no name to log it under.
                                            let name = motion_appearances
                                                .get(&extended.index)
                                                .and_then(|raw| {
                                                    rs09_client::player_interaction::username(
                                                        raw.name37,
                                                    )
                                                })
                                                .unwrap_or_default();
                                            workbench
                                                .overheads
                                                .entry((false, extended.index))
                                                .or_default()
                                                .chat = Some(ui_vm.forced_chat(
                                                ui.as_mut(),
                                                &name,
                                                extended.index == SELF,
                                                &bytes,
                                            ));
                                        }
                                        Field::Appearance { appearance, .. } => {
                                            let actor = if extended.index == SELF {
                                                Some(&mut *motion)
                                            } else {
                                                live_actors.get_mut(&extended.index)
                                            };
                                            if let Some(actor) = actor {
                                                rs09_client::player_form::appearance_position(
                                                    actor,
                                                    motion_appearances.get(&extended.index),
                                                    &appearance,
                                                    &workbench.npc_definitions,
                                                )?;
                                            }
                                            motion_appearances
                                                .insert(extended.index, appearance.clone());
                                            appearance_cache.insert(extended.index, appearance);
                                        }
                                        Field::FaceEntity(target) => {
                                            let actor = if extended.index == SELF {
                                                Some(&mut *motion)
                                            } else {
                                                live_actors.get_mut(&extended.index)
                                            };
                                            if let Some(actor) = actor {
                                                actor.face_entity = target;
                                            }
                                        }
                                        Field::FaceTile(tile) => {
                                            let actor = if extended.index == SELF {
                                                Some(&mut *motion)
                                            } else {
                                                live_actors.get_mut(&extended.index)
                                            };
                                            if let Some(actor) = actor {
                                                actor.face_tile = tile;
                                            }
                                        }
                                        Field::ForcedMovement {
                                            start,
                                            destination,
                                            start_cycle,
                                            end_cycle,
                                            direction,
                                        } => {
                                            let actor = if extended.index == SELF {
                                                Some(&mut *motion)
                                            } else {
                                                live_actors.get_mut(&extended.index)
                                            };
                                            if let Some(actor) = actor {
                                                actor.force_actor(
                                                    [start[0] + base[0], start[1] + base[1]],
                                                    [
                                                        destination[0] + base[0],
                                                        destination[1] + base[1],
                                                    ],
                                                    [start_cycle, end_cycle],
                                                    direction,
                                                    actor_states.entry(extended.index).or_default(),
                                                );
                                            }
                                        }
                                        Field::Animation { id, delay } => {
                                            if let Some(ui) = &ui {
                                                if let Err(e) =
                                                    workbench.player_sequence(&ui.pack, id)
                                                {
                                                    let message =
                                                        format!("Player animation {id}: {e}");
                                                    if !live_diagnostics.contains(&message)
                                                        && live_diagnostics.len() < 32
                                                    {
                                                        live_diagnostics.push(message);
                                                    }
                                                }
                                            }
                                            let queue = if extended.index == SELF {
                                                motion.queue.len()
                                            } else {
                                                live_actors
                                                    .get(&extended.index)
                                                    .map_or(0, |a| a.queue.len())
                                            };
                                            match actor_states
                                                .entry(extended.index)
                                                .or_default()
                                                .request(
                                                    id,
                                                    delay,
                                                    queue as i32,
                                                    &workbench.sequences,
                                                ) {
                                                Ok(events) => {
                                                    actor_sounds.extend(events.into_iter().map(
                                                        |s| (extended.index, s, ui_time as i32),
                                                    ))
                                                }
                                                Err(error) => {
                                                    let message =
                                                        format!("Actor animation {id}: {error}");
                                                    if !live_diagnostics.contains(&message)
                                                        && live_diagnostics.len() < 32
                                                    {
                                                        live_diagnostics.push(message);
                                                    }
                                                }
                                            }
                                        }
                                        Field::SpotAnimation { id, packed } => {
                                            match actor_states
                                                .entry(extended.index)
                                                .or_default()
                                                .request_spot(
                                                    id,
                                                    packed,
                                                    ui_time as i32,
                                                    &workbench.spot_sequences,
                                                    &workbench.sequences,
                                                ) {
                                                Ok(events) => actor_sounds.extend(
                                                    events.into_iter().map(|sound| {
                                                        (extended.index, sound, ui_time as i32)
                                                    }),
                                                ),
                                                Err(error) => {
                                                    let message =
                                                        format!("Player spot {id}: {error}");
                                                    if !live_diagnostics.contains(&message)
                                                        && live_diagnostics.len() < 32
                                                    {
                                                        live_diagnostics.push(message);
                                                    }
                                                }
                                            }
                                        }
                                        Field::SlotAnimations(requests) => {
                                            let requests: Vec<_> = requests
                                                .iter()
                                                .map(|s| (s.id, s.delay, s.mask))
                                                .collect();
                                            match actor_states
                                                .entry(extended.index)
                                                .or_default()
                                                .request_slots(&requests, &workbench.sequences)
                                            {
                                                Ok(events) => {
                                                    actor_sounds.extend(events.into_iter().map(
                                                        |s| (extended.index, s, ui_time as i32),
                                                    ))
                                                }
                                                Err(error) => {
                                                    let message =
                                                        format!("Slot animation: {error}");
                                                    if !live_diagnostics.contains(&message)
                                                        && live_diagnostics.len() < 32
                                                    {
                                                        live_diagnostics.push(message);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // New actors may reuse their original cached appearance without a mask.
                            for index in
                                std::iter::once(SELF).chain(player_state.local_ids.iter().copied())
                            {
                                let Some(appearance) = player_state.players[index]
                                    .as_ref()
                                    .and_then(|p| p.appearance.as_ref())
                                else {
                                    continue;
                                };
                                if live_decoded.get(&index) == Some(appearance) {
                                    continue;
                                }
                                if index == rs09_protocol::game::players::SELF {
                                    ui_vm.apply_self_name(appearance.name37);
                                    if let Some(ui) = &mut ui {
                                        match ui_models.sync_self(ui, appearance) {
                                            Ok(hash) => ui_vm.player_head_id = hash,
                                            Err(e) => ui.diagnostic(e),
                                        }
                                    }
                                }
                                let visual: Result<_, String> = (|| {
                                    let name = format!("bas/{}", appearance.bas);
                                    let bas = PlayerBas::decode(
                                        ui.as_ref()
                                            .and_then(|s| s.pack.get(&name).ok())
                                            .map_or_else(|| workbench.pack.get(&name), Ok)?,
                                    )
                                    .map_err(|e| e.to_string())?;
                                    let ui =
                                        ui.as_mut().ok_or("Player UI/model assets unavailable")?;
                                    workbench.player_sequences(&ui.pack, &bas)?;
                                    let form = ui_models
                                        .player_form(ui, &workbench, &ui_vm, appearance)?;
                                    let model = if form.visible() {
                                        Some(ui_models.player_body(
                                            ui, &workbench, appearance, [-1; 2], form,
                                        )?)
                                    } else {
                                        None
                                    };
                                    Ok((model, bas, form))
                                })(
                                );
                                match visual {
                                    Ok((model, bas, form)) => {
                                        body_overrides.insert(index, [-1; 2]);
                                        live_forms.insert(index, form);
                                        if index == SELF {
                                            workbench.avatar = model.into_iter().collect();
                                            workbench.bas = bas;
                                            ui_vm.combat_level = appearance.combat;
                                            ui_vm.female = appearance.female();
                                            manual = 808;
                                        } else {
                                            if let Some(model) = model {
                                                live_appearances.insert(index, model);
                                            } else {
                                                live_appearances.remove(&index);
                                            }
                                            live_bas.insert(index, bas);
                                        }
                                    }
                                    Err(error) => {
                                        // PlayerAppearance.getBodyModel: while the
                                        // models of a new look cannot be had, the
                                        // body built before stays; a player without
                                        // one has no body.
                                        let message = format!(
                                            "Player {index} appearance unavailable: {error}"
                                        );
                                        if live_diagnostics.len() < 32
                                            && !live_diagnostics.contains(&message)
                                        {
                                            live_diagnostics.push(message);
                                        }
                                    }
                                }
                                live_decoded.insert(index, appearance.clone());
                            }
                        }
                        32 => {
                            let base =
                                live_base.ok_or("NPC update arrived before scene rebuild")?;
                            #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                            if let Some(ui) = &mut ui {
                                for error in ambient_audio.npc_update_multi(
                                    workbench.audio_services(ui, &ui_vm, &live_npcs),
                                    &mut ui_audio,
                                ) {
                                    ui.diagnostic(error);
                                }
                            }
                            let update = workbench.receive_npcs(
                                &mut live_npcs,
                                &server_frame,
                                NpcInput {
                                    cycle: ui_time as i32,
                                    self_tile: motion.head,
                                    origin: base,
                                    animation_origin,
                                },
                                &mut world_sounds,
                            )?;
                            let messages = update.diagnostics;
                            #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                            let messages = if let Some(ui) = &ui {
                                messages
                                    .into_iter()
                                    .chain(ambient_audio.npc_events(
                                        &update.ambient,
                                        workbench.audio_services(ui, &ui_vm, &live_npcs),
                                        &mut ui_audio,
                                    ))
                                    .collect::<Vec<_>>()
                            } else {
                                messages
                            };
                            for message in messages {
                                if !live_diagnostics.contains(&message)
                                    && live_diagnostics.len() < 32
                                {
                                    live_diagnostics.push(message);
                                }
                            }
                        }
                        153 => {
                            destination_flag
                                .apply(&server_frame)
                                .map_err(|e| e.to_string())?;
                        }
                        86 => {
                            // LoginManager.processLogout(): the lifecycle
                            // closes the connection, clears the session and
                            // fades the music out once this packet is done.
                            match &mut session_life {
                                Some(life) => life.life.logout_packet(),
                                None => connection.stop("Logged out by server"),
                            }
                        }
                        _ => {
                            let handled = if let Some(ui) = &mut ui {
                                ui_vm.apply_frame(ui, &server_frame)?
                            } else {
                                false
                            };
                            let mut handled = handled
                                || workbench.server_frame(
                                    &server_frame,
                                    ui.as_mut(),
                                    ui_time as i32,
                                )?;
                            // SOUND_AREA and the half-tile projectiles, on their
                            // own or as children of the zone envelope, which
                            // has set the zone origin they are relative to.
                            for event in rs09_client::zone_events::scene_events(
                                &server_frame,
                                &mut workbench.world_spots.zone,
                            )
                            .map_err(|e| e.to_string())?
                            {
                                use rs09_client::zone_events::SceneEvent;
                                handled = true;
                                match event {
                                    SceneEvent::Projectile(packet) => workbench.launch_projectile(
                                        &packet,
                                        ui_time as i32,
                                        Launchers {
                                            npcs: &live_npcs,
                                            bas: &live_bas,
                                            actors: &live_actors,
                                            motion: &motion,
                                            own: connection
                                                .world
                                                .as_ref()
                                                .map(|world| usize::from(world.player_index)),
                                        },
                                    ),
                                    SceneEvent::AreaSound { tile, fields } => ui_audio.area(
                                        tile,
                                        player_state.self_player().tile(),
                                        fields,
                                        ui_time as i32,
                                    ),
                                }
                            }
                            if !handled {
                                use rs09_protocol::game::events::ServerEvent;
                                let own = connection
                                    .world
                                    .as_ref()
                                    .map(|world| usize::from(world.player_index));
                                let mut note = |message: String| {
                                    if live_diagnostics.len() < 32
                                        && !live_diagnostics.contains(&message)
                                    {
                                        live_diagnostics.push(message);
                                    }
                                };
                                handled = true;
                                match ServerEvent::decode(&server_frame)
                                    .map_err(|e| e.to_string())?
                                {
                                    Some(ServerEvent::TeleportSelf { x, y, plane, jump }) => {
                                        let base = live_base
                                            .ok_or("Teleport arrived before scene rebuild")?;
                                        player_state.plane = plane;
                                        player_state.self_player_mut().teleport(x, y, jump);
                                        if workbench.ground.plane != plane {
                                            workbench.ground.plane = plane;
                                            workbench.refresh_ground(ui.as_mut())?;
                                        }
                                        let state = actor_states.entry(SELF).or_default();
                                        if let Err(error) =
                                            motion.cancel_on_move(state, &workbench.sequences)
                                        {
                                            note(error.to_string());
                                        }
                                        let size =
                                            motion_appearances.get(&SELF).map_or(Ok(1), |raw| {
                                                rs09_client::player_form::size(
                                                    raw,
                                                    &workbench.npc_definitions,
                                                )
                                            })?;
                                        // PathingEntity.teleport: placed, not walked.
                                        if jump
                                            || (base[0] + x - motion.head[0]).abs() > 8
                                            || (base[1] + y - motion.head[1]).abs() > 8
                                        {
                                            atmosphere.instant = true;
                                        }
                                        motion.teleport_actor(
                                            [base[0] + x, base[1] + y],
                                            jump,
                                            size,
                                            state,
                                        );
                                    }
                                    Some(ServerEvent::ActorSpot {
                                        npc,
                                        index,
                                        spot,
                                        height,
                                        delay,
                                    }) => {
                                        let index = index as usize;
                                        let cycle = ui_time as i32;
                                        if npc {
                                            if let Some(actor) = live_npcs.actors.get_mut(&index) {
                                                match actor.animation.request_spot_at(
                                                    spot,
                                                    height,
                                                    delay,
                                                    cycle,
                                                    &workbench.spot_sequences,
                                                    &workbench.sequences,
                                                ) {
                                                    Ok(events) => world_sounds.extend(
                                                        events.into_iter().map(|s| WorldSound {
                                                            sequence: s.sequence as u32,
                                                            frame: s.frame,
                                                            fine: actor.motion.fine,
                                                            cycle: cycle
                                                                .wrapping_sub(animation_origin),
                                                        }),
                                                    ),
                                                    Err(error) => {
                                                        note(format!("NPC spot {spot}: {error}"))
                                                    }
                                                }
                                            }
                                        } else {
                                            let key = if Some(index) == own { SELF } else { index };
                                            let present = key == SELF
                                                || player_state
                                                    .players
                                                    .get(key)
                                                    .is_some_and(Option::is_some);
                                            if present {
                                                match actor_states
                                                    .entry(key)
                                                    .or_default()
                                                    .request_spot_at(
                                                        spot,
                                                        height,
                                                        delay,
                                                        cycle,
                                                        &workbench.spot_sequences,
                                                        &workbench.sequences,
                                                    ) {
                                                    Ok(events) => actor_sounds.extend(
                                                        events
                                                            .into_iter()
                                                            .map(|sound| (key, sound, cycle)),
                                                    ),
                                                    Err(error) => {
                                                        note(format!("Player spot {spot}: {error}"))
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Some(ServerEvent::NpcAnimation {
                                        npc,
                                        delay,
                                        sequence,
                                    }) => {
                                        let cycle = ui_time as i32;
                                        if let Some(actor) =
                                            live_npcs.actors.get_mut(&(npc as usize))
                                        {
                                            match actor.animation.request(
                                                sequence,
                                                delay,
                                                actor.motion.queue.len() as i32,
                                                &workbench.sequences,
                                            ) {
                                                Ok(events) => world_sounds.extend(
                                                    events.into_iter().map(|s| WorldSound {
                                                        sequence: s.sequence as u32,
                                                        frame: s.frame,
                                                        fine: actor.motion.fine,
                                                        cycle: cycle.wrapping_sub(animation_origin),
                                                    }),
                                                ),
                                                Err(error) => note(format!(
                                                    "NPC animation {sequence}: {error}"
                                                )),
                                            }
                                        }
                                    }
                                    Some(ServerEvent::ResetAnimations) => {
                                        for state in actor_states.values_mut() {
                                            state.reset_action();
                                        }
                                        for actor in live_npcs.actors.values_mut() {
                                            actor.animation.reset_action();
                                        }
                                    }
                                    Some(ServerEvent::LocAnimAt {
                                        x,
                                        y,
                                        plane,
                                        shape,
                                        sequence,
                                        ..
                                    }) => {
                                        if let Some(layer) = rs09_client::world::layer(shape as u8)
                                        {
                                            let slot = rs09_client::world::Slot {
                                                plane: plane as usize,
                                                tile: [x, y],
                                                layer,
                                            };
                                            if let Some(&loc) = workbench.world.locations.get(&slot)
                                            {
                                                workbench.dynamic_location(
                                                    slot,
                                                    loc,
                                                    Some(sequence),
                                                )?;
                                            }
                                        }
                                    }
                                    Some(
                                        event @ (ServerEvent::CameraReset { .. }
                                        | ServerEvent::CameraShake { .. }
                                        | ServerEvent::CameraLookAt { .. }
                                        | ServerEvent::CameraMoveTo { .. }
                                        | ServerEvent::CameraAngle { .. }),
                                    ) => {
                                        let (regions, origin) =
                                            (&workbench.regions, workbench.navigation.origin);
                                        let plane = workbench.ground.plane;
                                        let applied = camera_script.apply(&event, |fine| {
                                            rs09_client::terrain::scene_height(
                                                regions,
                                                origin,
                                                plane,
                                                [
                                                    fine[0] + origin[0] * 128,
                                                    fine[1] + origin[1] * 128,
                                                ],
                                            )
                                        });
                                        if let Some(applied) = applied {
                                            if let Some(ui) = &mut ui {
                                                ui.verify_id = (applied.counter + 1) & 65535;
                                            }
                                            if let Some([units_yaw, units_pitch]) = applied.angle {
                                                scene_occlusion.refresh(&workbench);
                                                follow.set_angles(
                                                    f64::from(units_pitch),
                                                    f64::from(units_yaw),
                                                    workbench.ground.plane,
                                                    &scene_occlusion.scene(&workbench),
                                                );
                                            }
                                        }
                                    }
                                    // The pinned HD renderer reads LOC_ADD_CHANGE and discards it.
                                    Some(ServerEvent::AttachLocation { .. }) => {}
                                    // Reflection probes and the random.dat identifier address
                                    // the original JVM client and have no native effect.
                                    Some(ServerEvent::ReflectionCheck(_) | ServerEvent::Uid(_)) => {
                                    }
                                    _ => handled = false,
                                }
                            }
                            if !handled {
                                let message =
                                    format!("Unhandled server opcode {}", server_frame.opcode);
                                if live_diagnostics.len() < 32
                                    && !live_diagnostics.contains(&message)
                                {
                                    live_diagnostics.push(message);
                                }
                            }
                        }
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    match &mut session_life {
                        Some(life) => life.handler_error(e, &server_frame.payload, motion.head),
                        None => connection.stop(e),
                    }
                    break;
                }
                // After LOGOUT the original reads no further packet.
                if session_life.as_ref().is_some_and(|l| l.life.stopping()) {
                    break;
                }
                read_on = true;
            }
        }
        self.manual = manual;
        self.live_base = live_base;
        self.missing_scene_regions = missing_scene_regions;
        self.route_status = route_status;
        self.route_steps = route_steps;
        read_on
    }
}
