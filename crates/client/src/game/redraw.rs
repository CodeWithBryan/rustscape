// The browser build has no capture files or travel captures: some of the
// fields bound below are only read by the desktop build.
#![cfg_attr(target_arch = "wasm32", allow(unused_variables, unused_mut))]
// The statements below were written when these fields were locals of the
// main loop and are kept as they were: a field bound to a local of its own
// name is a reference, so `&mut name` borrows where the compiler would not
// need it to.
#![allow(clippy::needless_borrow)]
use super::*;

impl Game {
    pub(super) async fn redraw(&mut self) -> bool {
        let mut workbench = &mut self.workbench;
        let npc_replay = &mut self.npc_replay;
        let replay = &mut self.replay;
        let mut ui = &mut self.ui;
        let mut ui_renderer = &mut self.ui_renderer;
        let mut ui_models = &mut self.ui_models;
        let ground_renderer = &mut self.ground_renderer;
        let mut world_picker = &mut self.world_picker;
        let mut minimap = &mut self.minimap;
        let drift = &mut self.drift;
        let mut ui_audio = &mut self.ui_audio;
        let ui_vm = &mut self.ui_vm;
        let sounds = &mut self.sounds;
        let material = &mut self.material;
        let static_renderer = &mut self.static_renderer;
        let character_shadow_material = &mut self.character_shadow_material;
        let title_world = &mut self.title_world;
        let motion = &mut self.motion;
        let destination_flag = &mut self.destination_flag;
        let atmosphere = &mut self.atmosphere;
        let light_meshes = &mut self.light_meshes;
        let follow = &mut self.follow;
        let scene_occlusion = &mut self.scene_occlusion;
        let scene_view = &mut self.scene_view;
        let context_menu = &mut self.context_menu;
        let cursor_view = &mut self.cursor_view;
        let tooltip_seed = &mut self.tooltip_seed;
        let outbox = &mut self.outbox;
        let mut session_life = &mut self.session_life;
        let connection = &mut self.connection;
        let player_state = &mut self.player_state;
        let live_actors = &mut self.live_actors;
        let live_npcs = &mut self.live_npcs;
        let live_appearances = &mut self.live_appearances;
        let live_bas = &mut self.live_bas;
        let live_decoded = std::mem::take(&mut self.live_decoded);
        let live_forms = &mut self.live_forms;
        let player_picks = &mut self.player_picks;
        let body_overrides = &mut self.body_overrides;
        let actor_states = &mut self.actor_states;
        let mut camera_script = &mut self.camera_script;
        let mut world_sounds = &mut self.world_sounds;
        let live_diagnostics = &mut self.live_diagnostics;
        let input = &mut self.input;
        let ui_drag = &mut self.ui_drag;
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let ambient_audio = &mut self.ambient_audio;
        let live_mode = self.live_mode;
        let capture_npcs = self.capture_npcs;
        let capture_animation = self.capture_animation;
        let capture_models = self.capture_models;
        let capture = self.capture;
        let capture_frame = self.capture_frame;
        let portrait = self.portrait;
        let capture_route = self.capture_route;
        let frustum_culling = self.frustum_culling;
        let fog_allowed = self.fog_allowed;
        let flicker_allowed = self.flicker_allowed;
        let loc_varbit = self.loc_varbit;
        let animation_origin = self.animation_origin;
        let capture_route_issued = self.capture_route_issued;
        let show_ui = self.show_ui;
        let sound_index = self.sound_index;
        let muted = self.muted;
        let audio_played = self.audio_played;
        let position = self.position;
        let npc_showcase = self.npc_showcase;
        let movement_tick = self.movement_tick;
        let mut atmosphere_tick = self.atmosphere_tick;
        let mut light_direction = self.light_direction;
        let mut lit_scene = self.lit_scene;
        let tween = self.tween;
        let roofs = self.roofs_frame;
        let shadows_enabled = self.shadows_enabled;
        let character_shadows = self.character_shadows;
        let brightness = self.brightness;
        let corrected_lights = self.corrected_lights;
        let ticks = self.ticks;
        let world_ticks = self.world_ticks;
        let manual = self.manual;
        let frame = self.frame;
        let live_base = self.live_base;
        let mut render_profile_started = self.render_profile_started;
        let mut static_submission_seconds = self.static_submission_seconds;
        let ui_time = self.ui_time;
        let ui_cycle = self.ui_cycle;
        let mut ui_scene_delta = self.ui_scene_delta;
        let mut published_inventory_press_active = self.published_inventory_press_active;
        let mut model_cycle = self.model_cycle;
        let mut world_spot_tick = self.world_spot_tick;
        let ui_debug = self.ui_debug;
        let view_distance = self.view_distance;
        let plane = self.plane;
        let at_title = self.at_title;
        let viewport = self.viewport;
        let captured_journey_finished = self.captured_journey_finished;
        let mut route_status = std::mem::take(&mut self.route_status);
        let mut click_cross = std::mem::take(&mut self.click_cross);
        let mut last_render_metrics = std::mem::take(&mut self.last_render_metrics);
        let hover_menu = std::mem::take(&mut self.hover_menu);
        let ui_nodes = std::mem::take(&mut self.ui_nodes);
        let input_camera = self.input_camera.take().expect("input_camera");
        let origin = workbench.navigation.origin;
        // `Cheat.shiftClick && Keyboard.pressedKeys[KEY_SHIFT]`: the left
        // click, the hover text and the cursor then all take the entry below
        // the top one (`context_menu::top_entry`).
        let shift_click = ui_vm.cheat.shift_click
            && input.keyboard.pressed_keys[rs09_client::input::keyboard::KEY_SHIFT];
        let heading = motion.angle as f32 * std::f32::consts::TAU / 2048.0;
        let sequence = if manual == 808 && motion.sequence >= 0 {
            motion.sequence as u32
        } else {
            manual
        };
        let movement_cursor = if manual == 808 {
            workbench.sequences.get(&sequence).and_then(|s| {
                s.delays.get(motion.frame).map(|&length| {
                    (
                        motion.frame,
                        motion.next_frame,
                        motion.progress - 1,
                        i32::from(length),
                    )
                })
            })
        } else {
            None
        };
        scene_occlusion.refresh(&workbench);
        let (camera, camera_pose) = scene_camera::Viewer {
            follow: &follow,
            view: &scene_view,
            view_distance,
            world: &workbench,
            occlusion: &scene_occlusion,
            player: motion.fine,
        }
        .camera(&mut camera_script, true);
        #[cfg(not(target_arch = "wasm32"))]
        super::pose_dump::answer(super::pose_dump::Frame {
            pose: &camera_pose,
            follow: &follow,
            view: &scene_view,
            view_distance,
            player_local: scene_camera::local(motion.fine, origin),
            origin,
            plane: workbench.ground.plane as usize,
            locked: camera_script.locked,
            spline: camera_script.spline.as_ref(),
        });
        // A newly built scene: `LoginManager` takes the sun from the chunk
        // the player stands in.
        if lit_scene != workbench.point_lights.revision {
            lit_scene = workbench.point_lights.revision;
            light_direction = workbench.environment(position).direction;
        }
        // The title scene's sun is that of the chunk its unseen local
        // player was put in, not of the camera's.
        if let Some(sun) = title_world.sun() {
            light_direction = workbench.environment(sun).direction;
        }
        // FogManager.light0Position, which colours are baked with when a
        // model is packed without high detail lighting.
        rs09_client::sd_light::set_sun(light_direction);
        let atmosphere_now = world_ticks as u64;
        // CS2 6001 and 6015 call FogManager.setInstantFade in HD.
        if std::mem::take(&mut ui_vm.instant_fade) {
            atmosphere.instant = true;
        }
        let params = atmosphere.update(
            workbench.environment(position),
            atmosphere_now.wrapping_sub(atmosphere_tick) as i32,
            brightness,
            fog_allowed && ui_vm.preferences.fog,
        );
        atmosphere_tick = atmosphere_now;
        // Preferences.sceneryShadowsType: 0 hides the map, 1 keeps the static
        // shadows, 2 also follows animated scenery (Loc.java:328).
        workbench.shadows.mode = if shadows_enabled {
            ui_vm.preferences.scenery_shadows.max(1)
        } else {
            0
        };
        // The scene build reads the preference as the original's static; a
        // Loc picks its multiloc child from the variables every frame. A
        // varbit without a definition to read it by counts as 0.
        rs09_client::loc_placement::set_ground_decorations(
            ui_vm.preferences.show_ground_decorations,
        );
        if let Err(error) = workbench.sync_multilocs(|bit, key| {
            if bit {
                loc_varbit
                    .filter(|&(id, _)| id == key)
                    .map(|(_, value)| value)
                    .or_else(|| ui.as_ref().and_then(|ui| ui_vm.varbit(ui, key).ok()))
                    .unwrap_or(0)
            } else {
                ui_vm.varps.get(&key).copied().unwrap_or(0)
            }
        }) {
            route_status = error;
        }
        // R shows every level whole; otherwise the two roof preferences decide.
        workbench.roof_frame(
            if roofs {
                rs09_client::roof::Mode::Never
            } else {
                rs09_client::roof::Mode::new(
                    ui_vm.preferences.never_remove_roofs,
                    ui_vm.preferences.remove_roofs_selectively,
                )
            },
            motion.fine,
            &camera,
            !(camera_script.locked || camera_script.spline.is_some()),
        );
        // SceneGraph.buildActiveOccluders and the walk's occlusion queries:
        // what the original leaves undrawn behind walls and upper floors.
        workbench.cull_frame(
            &mut world_picker,
            &camera,
            [
                scene_view.left as f32,
                scene_view.top as f32,
                scene_view.width as f32,
                scene_view.height as f32,
            ],
        );
        let locations = workbench.prepare_locations(
            world_ticks as u64 as i32,
            tween,
            position,
            true,
            &mut world_sounds,
        );
        for sound in world_sounds.drain(..) {
            if let Some(seq) = workbench.sequences.get(&sound.sequence) {
                ui_audio.sequence(
                    seq,
                    sound.frame,
                    [
                        sound.fine[0] - origin[0] * 128,
                        sound.fine[1] - origin[1] * 128,
                    ],
                    false,
                    animation_origin.wrapping_add(sound.cycle),
                );
            }
        }
        if let Some(ui) = &mut ui {
            for error in ui_audio
                .advance(
                    &ui.pack,
                    ui_time as i32,
                    [
                        motion.fine[0] - origin[0] * 128,
                        motion.fine[1] - origin[1] * 128,
                    ],
                    if muted {
                        [0; 2]
                    } else {
                        [
                            ui_vm.preferences.sound_volume,
                            ui_vm.preferences.ambient_volume,
                        ]
                    },
                )
                .await
            {
                ui.diagnostic(error);
            }
        }
        workbench.point_lights.update(
            world_ticks as i32,
            !(flicker_allowed && ui_vm.preferences.flickering_effects),
        );
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        if let Some(ui) = &mut ui {
            let changes = std::mem::take(&mut workbench.audio_changes);
            let updates = ambient_audio::Updates {
                locations: changes,
                varps: std::mem::take(&mut ui_vm.ambient_updates),
            };
            if !live_mode || connection.as_ref().is_some_and(|c| c.in_game()) {
                for error in ambient_audio.advance(
                    updates,
                    &mut ui_audio,
                    ambient_audio::Listener {
                        cycle: ui_time as i32,
                        fine: motion.fine,
                        // AreaSoundManager: Preferences.ambientSoundsVolume.
                        volume: if muted {
                            0
                        } else {
                            ui_vm.preferences.ambient_volume
                        },
                    },
                    workbench.audio_services(ui, &ui_vm, &live_npcs),
                ) {
                    ui.diagnostic(error);
                }
            } else {
                ambient_audio.suspend(&mut ui_audio, &ui.pack);
            }
        }
        let [r, g, b] = rs09_client::atmosphere::rgb(params.fog_color);
        if let Some(ui) = &mut ui {
            // Rebuild only when the primary sequence changes equipment; pose the
            // resulting raw body through the same movement/action blend below.
            for (&index, raw) in &live_decoded {
                let overrides = actor_states
                    .get(&index)
                    .filter(|a| a.action.id != -1 && a.action.delay == 0)
                    .and_then(|a| workbench.sequences.get(&(a.action.id as u32)))
                    .map_or([-1; 2], |s| [s.mainhand, s.offhand]);
                let form = match ui_models.player_form(ui, &workbench, &ui_vm, raw) {
                    Ok(form) => form,
                    Err(e) => {
                        if index == rs09_protocol::game::players::SELF {
                            workbench.avatar.clear();
                        } else {
                            live_appearances.remove(&index);
                        }
                        live_forms.remove(&index);
                        ui.diagnostic(format!("Player {index} form: {e}"));
                        continue;
                    }
                };
                if body_overrides.get(&index) == Some(&overrides)
                    && live_forms.get(&index) == Some(&form)
                {
                    continue;
                }
                live_forms.insert(index, form);
                if !form.visible() {
                    if index == rs09_protocol::game::players::SELF {
                        workbench.avatar.clear();
                    } else {
                        live_appearances.remove(&index);
                    }
                    body_overrides.insert(index, overrides);
                    continue;
                }
                match ui_models.player_body(ui, &workbench, raw, overrides, form) {
                    Ok(model) => {
                        if index == rs09_protocol::game::players::SELF {
                            workbench.avatar = vec![model];
                        } else {
                            live_appearances.insert(index, model);
                        }
                    }
                    Err(e) => {
                        // The body built before stays (see the appearance update).
                        ui.diagnostic(format!("Player {index} equipment override: {e}"));
                    }
                }
                body_overrides.insert(index, overrides);
            }
        }
        if let Some(ui) = &mut ui {
            let delta = ui_cycle.wrapping_sub(model_cycle);
            model_cycle = ui_cycle;
            if let Err(e) = ui_models.advance(ui, &workbench.pack, &ui_vm, delta) {
                ui.diagnostic(format!("UI animation: {e}"));
            }
        }
        // GlRenderer.clearColorAndDepthBuffers clears the scene's rectangle
        // to the fog colour (its scissor is the viewport); around it is the
        // interface, on black.
        clear_background(BLACK);
        draw_rectangle(
            scene_view.left as f32,
            scene_view.top as f32,
            scene_view.width as f32,
            scene_view.height as f32,
            Color::new(r, g, b, 1.0),
        );
        set_camera(&camera);
        material.set_uniform("TextureTime", world_ticks as f32);
        material.set_uniform("ShadowsEnabled", if shadows_enabled { 1.0f32 } else { 0.0 });
        render::atmosphere(
            &material,
            params,
            light_direction,
            camera.position,
            camera.target,
        );
        gl_use_material(&material);
        let frustum = rs09_client::frustum::Frustum::new(camera.matrix().to_cols_array());
        let submission_start = get_time();
        let (
            mut eligible_chunks,
            mut visible_chunks,
            mut eligible_triangles,
            mut submitted_triangles,
        ) = (0usize, 0usize, 0usize, 0usize);
        let mut visible_static_chunks = Vec::new();
        for (index, chunk) in workbench.chunks.iter().enumerate() {
            if !workbench.roofs.hides_chunk(index) && chunk.center.distance(position) < 70.0 {
                eligible_chunks += 1;
                eligible_triangles += chunk.mesh.indices.len() / 3;
                if chunk.mesh.indices.is_empty()
                    || (frustum_culling && !frustum.intersects(chunk.bounds))
                {
                    continue;
                }
                visible_chunks += 1;
                submitted_triangles += chunk.mesh.indices.len() / 3;
                visible_static_chunks.push(index);
            }
        }
        // SceneGraph.setPlainTile: the underwater scene comes first.
        let underwater_chunks = underwater_mesh::split(&workbench, &mut visible_static_chunks);
        // SceneGraph.renderScene: every level's floors, then the light
        // meshes, then the scenery. Floor chunks come first in the list.
        let (floor_chunks, scenery_chunks): (Vec<usize>, Vec<usize>) = visible_static_chunks
            .into_iter()
            .partition(|&index| workbench.chunks[index].floor);
        if static_renderer.is_none() {
            // Blended floor faces need the nearer depth layer and no depth
            // writes: the character shadow material has both.
            character_shadow_material.set_uniform("TextureTime", world_ticks as f32);
            character_shadow_material
                .set_uniform("ShadowsEnabled", if shadows_enabled { 1.0f32 } else { 0.0 });
            render::atmosphere(
                &character_shadow_material,
                params,
                light_direction,
                camera.position,
                camera.target,
            );
        }
        underwater_mesh::draw(
            &mut workbench,
            &underwater_chunks,
            underwater_mesh::Frame {
                renderer: static_renderer.as_ref(),
                materials: [&material, &character_shadow_material],
                camera: &camera,
                atmosphere: params,
                direction: light_direction,
                time: world_ticks as f32,
                shadows: shadows_enabled,
            },
        );
        // The tile walk that follows the floors draws each actor's shadow
        // with the actor (`Player.java:436-448`, `Npc.java:73-76`), so the
        // scenery drawn after it covers a shadow it stands in front of: the
        // scenery pass waits until the shadows are drawn (`deferred_scenery`).
        let mut deferred_scenery = Vec::new();
        let scenery_plane = plane;
        for (pass, chunks) in [floor_chunks, scenery_chunks].into_iter().enumerate() {
            if pass == 1 {
                deferred_scenery = chunks;
                continue;
            }
            if let Some(renderer) = &static_renderer {
                renderer.draw(
                    &mut workbench,
                    &chunks,
                    &[],
                    static_render::View {
                        camera: &camera,
                        atmosphere: params,
                        direction: light_direction,
                        time: world_ticks as f32,
                        shadows: shadows_enabled,
                    },
                );
            } else {
                for index in chunks {
                    workbench.draw_chunk(index, [&material, &character_shadow_material]);
                }
            }
            gl_use_material(&material);
            // LightingManager.renderLightMeshes: none without high-detail
            // lighting.
            if pass == 0 && !rs09_client::sd_light::enabled() {
                if light_meshes.sync(&workbench) {
                    println!(
                        "Light meshes: {} of {} placed lights, {} triangles; floor shadow sprites {}",
                        light_meshes.lights,
                        workbench.point_lights.lights.len(),
                        light_meshes.triangles,
                        if workbench.textures.floor_shadows.is_some() {
                            "loaded"
                        } else {
                            "absent from the pack"
                        }
                    );
                }
                light_meshes.draw(&workbench, params, &camera, position);
                gl_use_material(&material);
            }
        }
        let floor_seconds = get_time() - submission_start;
        workbench.slot_views.borrow_mut().clear();
        for error in workbench.sync_npc_palette(&ui_vm.recolor) {
            if live_diagnostics.len() < 32 && !live_diagnostics.contains(&error) {
                live_diagnostics.push(error);
            }
        }
        ui_models.set_palette(&ui_vm.recolor);
        {
            // renderGameScene: which actors enter the scene this frame.
            let self_id = rs09_protocol::game::players::SELF;
            let others: Vec<_> = player_state
                .local_ids
                .iter()
                .filter_map(|index| {
                    Some(actor_scene::PlayerView {
                        index: *index,
                        motion: live_actors.get(index)?,
                        bas: live_bas.get(index),
                        visible: rs09_client::crowd::visible(player_state, *index),
                        size: live_forms.get(index).map_or(1, |form| form.size),
                    })
                })
                .collect();
            workbench.plan_actors(
                actor_scene::PlayerView {
                    index: self_id,
                    motion: &motion,
                    bas: Some(&workbench.bas.clone()),
                    visible: !workbench.avatar.is_empty() || live_decoded.contains_key(&self_id),
                    size: live_forms.get(&self_id).map_or(1, |form| form.size),
                },
                &others,
                &mut destination_flag.tile,
                ui_vm.preferences.many_idle_animations,
            );
        }
        let (avatar, character_shadow, self_height) = workbench.avatar_mesh(
            &workbench.avatar,
            ActorPose {
                actor: (false, rs09_protocol::game::players::SELF),
                // Player.render: a low-detail player has no movement sequence.
                sequence: if workbench
                    .actor_scene
                    .low_detail
                    .contains(&rs09_protocol::game::players::SELF)
                {
                    u32::MAX
                } else {
                    sequence
                },
                tick: ticks as u64,
                tween,
                cursor: movement_cursor,
                state: actor_states.get(&rs09_protocol::game::players::SELF),
                idle: workbench.bas.idle,
            },
            position,
            heading,
            live_forms
                .get(&rs09_protocol::game::players::SELF)
                .copied()
                .unwrap_or_default(),
            workbench.bas.terrain,
        );
        let mut spot_actors = Vec::new();
        let mut add_spot =
            |npc, index, fine, yaw, terrain, state: &rs09_client::actor_animation::State| {
                if state.spot.id != -1 && state.spot.frame >= 0 {
                    spot_actors.push(SpotActor {
                        pitch: None,
                        npc,
                        index,
                        fine,
                        yaw,
                        terrain,
                        track: state.spot.clone(),
                        height: state.spot_height,
                        world: None,
                    });
                }
            };
        if !workbench.avatar.is_empty()
            && let Some(state) = actor_states.get(&rs09_protocol::game::players::SELF)
        {
            add_spot(
                false,
                rs09_protocol::game::players::SELF,
                motion.fine,
                motion.angle,
                workbench.bas.terrain,
                state,
            );
        }
        for index in &player_state.local_ids {
            if let (Some(actor), Some(state)) = (live_actors.get(index), actor_states.get(index))
                && live_appearances.contains_key(index)
            {
                add_spot(
                    false,
                    *index,
                    actor.fine,
                    actor.angle,
                    live_bas.get(index).map_or([0; 2], |b| b.terrain),
                    state,
                );
            }
        }
        for npc in &workbench.npcs {
            if npc.render_id.is_some()
                && let Some(state) = &npc.animation
            {
                let bas = workbench.npc_definitions[&npc.id].bas;
                add_spot(
                    true,
                    npc.index,
                    npc.motion.fine,
                    npc.motion.angle,
                    workbench.npc_bases.get(&bas).map_or([0; 2], |b| b.terrain),
                    state,
                );
            }
        }
        let spot_tick = world_ticks as u64;
        let spot_cycle = animation_origin.wrapping_add(spot_tick as i32);
        let spot_delta = spot_tick.wrapping_sub(world_spot_tick) as i32;
        if let Err(error) = workbench.world_spots.update(
            workbench.ground.plane,
            spot_cycle,
            spot_delta,
            &workbench.sequences,
        ) {
            let message = error.to_string();
            if live_diagnostics.len() < 32 && !live_diagnostics.contains(&message) {
                live_diagnostics.push(message);
            }
        }
        world_spot_tick = spot_tick;
        workbench.spot_views.clear();
        let mut spot_meshes = BTreeMap::new();
        for actor in spot_actors {
            let p = vec2(actor.fine[0] as f32 / 128.0, actor.fine[1] as f32 / 128.0);
            if p.distance(position) > 55.0 {
                continue;
            }
            match workbench.spot_mesh(&actor, tween) {
                Ok(Some(mesh)) => {
                    spot_meshes.insert((actor.npc, actor.index), mesh);
                }
                Ok(None) => {}
                Err(error) => {
                    let message = format!("Spot{}: {error}", actor.track.id);
                    if !live_diagnostics.contains(&message) && live_diagnostics.len() < 32 {
                        live_diagnostics.push(message);
                    }
                }
            }
        }
        let mut world_spot_meshes = Vec::new();
        for (index, effect) in workbench
            .world_spots
            .visible(spot_cycle)
            .cloned()
            .enumerate()
            .collect::<Vec<_>>()
        {
            let actor = SpotActor {
                pitch: None,
                npc: false,
                index,
                fine: std::array::from_fn(|i| {
                    effect.fine[i] + workbench.navigation.origin[i] * 128
                }),
                yaw: 0,
                terrain: [0; 2],
                height: 0,
                track: rs09_client::actor_animation::Track {
                    id: effect.id,
                    frame: effect.frame as i32,
                    next: -1,
                    clock: effect.clock,
                    delay: 0,
                    repeats: 0,
                },
                world: Some((effect.plane, effect.z)),
            };
            match workbench.spot_mesh(&actor, tween) {
                Ok(Some(mesh)) => world_spot_meshes.push(mesh),
                Ok(None) => {}
                Err(error) => {
                    if live_diagnostics.len() < 32 && !live_diagnostics.contains(&error) {
                        live_diagnostics.push(error);
                    }
                }
            }
        }
        let plane = workbench.ground.plane;
        let origin = workbench.navigation.origin;
        let player_index = connection
            .as_ref()
            .and_then(|c| c.world.as_ref())
            .map(|w| usize::from(w.player_index));
        workbench.world_spots.projectiles.retain_mut(|shot| {
            if shot.plane != plane || shot.end.wrapping_sub(spot_cycle) < 0 {
                return false;
            }
            let target = shot.packet.target;
            // updateSceneProjectiles re-aims only at an actor that exists and
            // stands inside the scene; otherwise the flight keeps the aim
            // Protocol gave it at the packet's destination.
            let entity = if target > 0 {
                live_npcs
                    .actors
                    .get(&((target - 1) as usize))
                    .map(|n| n.motion.fine)
            } else if target < 0 {
                let index = (-target - 1) as usize;
                if Some(index) == player_index {
                    Some(motion.fine)
                } else {
                    live_actors.get(&index).map(|p| p.fine)
                }
            } else {
                None
            }
            .filter(|fine| (0..2).all(|i| (0..13312).contains(&(fine[i] - origin[i] * 128))));
            let aim = |fine: [i32; 2]| {
                rs09_client::ground_geometry::tile_height(
                    plane,
                    std::array::from_fn(|i| fine[i] - origin[i] * 128),
                    |p, x, y| {
                        let t = tile(&workbench.regions, p as usize, x + origin[0], y + origin[1]);
                        (t.height, t.flags)
                    },
                ) - shot.packet.end_height * 4
            };
            let sequence = workbench
                .spot_sequences
                .get(&shot.packet.spot)
                .and_then(|&id| workbench.sequences.get(&(id as u32)));
            shot.step(
                spot_cycle,
                spot_delta,
                aim(shot.destination),
                entity.map(|fine| [fine[0], fine[1], aim(fine)]),
                sequence,
            );
            true
        });
        for (index, shot) in workbench
            .world_spots
            .projectiles
            .iter()
            .enumerate()
            .filter(|(_, shot)| shot.visible(spot_cycle))
            .map(|(i, shot)| (i, shot.clone()))
            .collect::<Vec<_>>()
        {
            let [x, y, z] = shot.position.map(|v| v as i32);
            let actor = SpotActor {
                pitch: Some(shot.pitch),
                npc: false,
                index,
                fine: [x, y],
                yaw: shot.yaw,
                terrain: [0; 2],
                height: 0,
                track: rs09_client::actor_animation::Track {
                    id: shot.packet.spot,
                    frame: shot.frame as i32,
                    next: shot.next_frame,
                    clock: shot.clock,
                    delay: 0,
                    repeats: 0,
                },
                world: Some((plane, z)),
            };
            match workbench.spot_mesh(&actor, tween) {
                Ok(Some(mesh)) => world_spot_meshes.push(mesh),
                Ok(None) => {}
                Err(error) if live_diagnostics.len() < 32 && !live_diagnostics.contains(&error) => {
                    live_diagnostics.push(error)
                }
                Err(_) => {}
            }
        }
        let (npc_meshes, npc_shadows) = workbench.npc_meshes(position, tween, &spot_meshes);
        let mut player_heights = BTreeMap::new();
        let player_meshes: Vec<_> = player_state
            .local_ids
            .iter()
            .filter_map(|index| {
                let actor = live_actors.get(index)?;
                let model = live_appearances.get(index)?;
                if workbench.actor_scene.thinned_players.contains(index) {
                    return None;
                }
                let low_detail = workbench.actor_scene.low_detail.contains(index);
                let cursor = workbench
                    .sequences
                    .get(&(actor.sequence as u32))
                    .and_then(|s| {
                        s.delays.get(actor.frame).map(|&n| {
                            (
                                actor.frame,
                                actor.next_frame,
                                actor.progress - 1,
                                i32::from(n),
                            )
                        })
                    });
                let meshes = workbench.avatar_mesh(
                    std::slice::from_ref(model),
                    ActorPose {
                        actor: (false, *index),
                        sequence: if low_detail {
                            u32::MAX
                        } else {
                            actor.sequence as u32
                        },
                        tick: world_ticks as u64,
                        tween,
                        cursor,
                        state: actor_states.get(index),
                        idle: live_bas.get(index).map_or(-1, |b| b.idle),
                    },
                    vec2(actor.fine[0] as f32 / 128.0, actor.fine[1] as f32 / 128.0),
                    actor.angle as f32 * std::f32::consts::TAU / 2048.0,
                    live_forms.get(index).copied().unwrap_or_default(),
                    live_bas.get(index).map_or([0; 2], |b| b.terrain),
                );
                player_heights.insert(*index, meshes.2);
                Some((*index, meshes))
            })
            .collect();
        workbench.actor_scene.remember_heights(&mut player_heights);
        let foot_arrows = workbench
            .foot_arrow_mesh(
                &ui_vm.minimap.hints,
                motion.fine,
                |index| {
                    if index == rs09_protocol::game::players::SELF as i32 {
                        Some(motion.fine)
                    } else {
                        live_actors.get(&(index as usize)).map(|a| a.fine)
                    }
                },
                ui.as_ref().map(|ui| &ui.pack),
            )
            .unwrap_or_else(|error| {
                if live_diagnostics.len() < 32 && !live_diagnostics.contains(&error) {
                    live_diagnostics.push(error);
                }
                workbench.textures.mesh()
            });
        player_picks.clear();
        for (index, (mesh, _, _)) in &player_meshes {
            if mesh.vertices.is_empty() {
                continue;
            }
            let Some(actor) = live_actors.get(index) else {
                continue;
            };
            player_picks.push(PlayerPick {
                index: *index,
                fine: actor.fine,
                angle: actor.angle,
                size: live_forms.get(index).map_or(1, |form| form.size),
                triangles: mesh
                    .indices
                    .chunks_exact(3)
                    .map(|f| {
                        [
                            mesh.vertices[f[0] as usize].position,
                            mesh.vertices[f[1] as usize].position,
                            mesh.vertices[f[2] as usize].position,
                        ]
                    })
                    .collect(),
            });
        }
        // The own body is in the scene with key 2047 << 32 and can be picked
        // like any player; its menu lists the actors on its tile.
        if !workbench.actor_scene.own_thinned && !avatar.vertices.is_empty() {
            let index = rs09_protocol::game::players::SELF;
            player_picks.push(PlayerPick {
                index,
                fine: motion.fine,
                angle: motion.angle,
                size: live_forms.get(&index).map_or(1, |form| form.size),
                triangles: avatar
                    .indices
                    .chunks_exact(3)
                    .map(|f| [0, 1, 2].map(|i| avatar.vertices[f[i] as usize].position))
                    .collect(),
            });
        }
        if character_shadows {
            character_shadow_material.set_uniform("TextureTime", world_ticks as f32);
            character_shadow_material.set_uniform("ShadowsEnabled", 0.0f32);
            render::atmosphere(
                &character_shadow_material,
                params,
                light_direction,
                camera.position,
                camera.target,
            );
            gl_use_material(&character_shadow_material);
            // Player.render draws the shadow only for a player in the scene:
            // not the title screen's unseen local player (drawn below
            // under the same condition as its body).
            if !workbench.actor_scene.own_thinned {
                draw_mesh(&character_shadow);
            }
            draw_mesh(&npc_shadows);
            for (_, (_, shadow, _)) in &player_meshes {
                draw_mesh(shadow);
            }
            gl_use_material(&material);
        }
        if !foot_arrows.vertices.is_empty() && !workbench.actor_scene.own_thinned {
            // Player.renderHintArrow draws like the shadow: no depth writes.
            character_shadow_material.set_uniform("TextureTime", world_ticks as f32);
            character_shadow_material.set_uniform("ShadowsEnabled", 0.0f32);
            render::atmosphere(
                &character_shadow_material,
                params,
                light_direction,
                camera.position,
                camera.target,
            );
            gl_use_material(&character_shadow_material);
            draw_mesh(&foot_arrows);
            gl_use_material(&material);
        }
        // The scenery after the shadows (see `deferred_scenery`).
        let scenery_start = get_time();
        if let Some(renderer) = &static_renderer {
            renderer.draw(
                &mut workbench,
                &deferred_scenery,
                &[],
                static_render::View {
                    camera: &camera,
                    atmosphere: params,
                    direction: light_direction,
                    time: world_ticks as f32,
                    shadows: shadows_enabled,
                },
            );
        } else {
            for index in deferred_scenery {
                workbench.draw_chunk(index, [&material, &character_shadow_material]);
            }
        }
        gl_use_material(&material);
        if frame >= 20 {
            render_profile_started.get_or_insert(get_time());
            static_submission_seconds += floor_seconds + (get_time() - scenery_start);
        }
        if connection.is_some() || npc_replay.is_some() {
            last_render_metrics = serde_json::json!({"frame":frame,"frustum_culling":frustum_culling,"static_gpu":static_renderer.as_ref().map(static_render::Renderer::statistics),"eligible_chunks":eligible_chunks,"visible_chunks":visible_chunks,"eligible_static_triangles":eligible_triangles,"submitted_static_triangles":submitted_triangles,"frames_after_warmup":frame.saturating_sub(20),"seconds_after_warmup":render_profile_started.map(|start|get_time()-start),"static_submission_seconds":static_submission_seconds});
        }
        for (index, mesh) in locations.iter().enumerate() {
            let layer = workbench.roofs.animated_layers.get(index).copied();
            material.set_uniform("DepthBias", Vec2::from(layer.unwrap_or_default()));
            draw_mesh(mesh);
        }
        material.set_uniform(
            "DepthBias",
            Vec2::from(rs09_client::depth_layers::offset::NONE),
        );
        ground_renderer.draw(scenery_plane, position);
        for mesh in &world_spot_meshes {
            draw_mesh(mesh);
        }
        let low_detail = |index: &usize| workbench.actor_scene.low_detail.contains(index);
        if !workbench.actor_scene.own_thinned {
            draw_mesh(&avatar);
            if let Some(spot) = spot_meshes
                .get(&(false, rs09_protocol::game::players::SELF))
                .filter(|_| !low_detail(&rs09_protocol::game::players::SELF))
            {
                draw_mesh(spot);
            }
        }
        for (index, (mesh, _, _)) in &player_meshes {
            draw_mesh(mesh);
            if let Some(spot) = spot_meshes
                .get(&(false, *index))
                .filter(|_| !low_detail(index))
            {
                draw_mesh(spot);
            }
        }
        for (index, mesh) in npc_meshes {
            draw_mesh(&mesh);
            if let Some(spot) = spot_meshes.get(&(true, index)) {
                draw_mesh(spot);
            }
        }
        // The translucent faces of the map's locations, after everything
        // opaque: in the original's drawing order and, as there, with depth
        // writes.
        let glass = workbench.roofs.glass(&workbench.chunks, position);
        if !glass.is_empty() {
            if let Some(renderer) = &static_renderer {
                renderer.draw(
                    &mut workbench,
                    &[],
                    &glass,
                    static_render::View {
                        camera: &camera,
                        atmosphere: params,
                        direction: light_direction,
                        time: world_ticks as f32,
                        shadows: shadows_enabled,
                    },
                );
            } else {
                workbench.draw_glass(&glass, &material);
            }
        }
        gl_use_default_material();
        display_host::camera();
        let mut overhead_draw_count = 0;
        if let Some(ui) = &mut ui {
            use rs09_client::overheads::{Actor, Config, NpcStyle};
            let mut actors = Vec::new();
            let mut fines = Vec::new();
            let mut add = |npc: bool,
                           index: usize,
                           fine: [i32; 2],
                           model_height: i32,
                           styles: (Option<NpcStyle>, Option<NpcStyle>),
                           icons: [i32; 2],
                           friend: bool| {
                fines.push(fine);
                actors.push(Actor {
                    index: index as i32,
                    visible: true,
                    npc: styles.0,
                    npc_head: styles.1,
                    model_height,
                    icons,
                    friend,
                    state: workbench
                        .overheads
                        .get(&(npc, index))
                        .cloned()
                        .unwrap_or_default(),
                });
            };
            let self_id = rs09_protocol::game::players::SELF;
            let icons = live_decoded
                .get(&self_id)
                .map_or([-1, -1], |a| [a.skull_icon, a.prayer_icon]);
            add(
                false,
                self_id,
                motion.fine,
                self_height,
                (None, None),
                icons,
                true,
            );
            for &index in &player_state.local_ids {
                if let (Some(actor), Some(height), Some(appearance)) = (
                    live_actors.get(&index),
                    player_heights.get(&index),
                    live_decoded.get(&index),
                ) {
                    let friend = ui_vm
                        .social
                        .friends
                        .iter()
                        .any(|f| f.key == appearance.name37);
                    add(
                        false,
                        index,
                        actor.fine,
                        *height,
                        (None, None),
                        [appearance.skull_icon, appearance.prayer_icon],
                        friend,
                    );
                }
            }
            for npc in &workbench.npcs {
                // drawOverheads does not ask whether an actor is in the scene:
                // one left out by crowd thinning keeps its overheads.
                if npc.pick_triangles.is_empty()
                    && !workbench.actor_scene.thinned_npcs.contains(&npc.index)
                {
                    continue;
                }
                let d = &workbench.npc_definitions[&npc.id];
                let Ok(Some(head_id)) = workbench.npc_choice(npc.id, &ui_vm, Some(ui)) else {
                    continue;
                };
                let h = &workbench.npc_definitions[&head_id];
                add(
                    true,
                    npc.index,
                    npc.motion.fine,
                    npc.model_height,
                    (
                        Some(NpcStyle {
                            headicon: d.headicon,
                            icon_height: d.icon_height,
                            hitbar: d.hit_bar_id,
                        }),
                        Some(NpcStyle {
                            headicon: h.headicon,
                            icon_height: h.icon_height,
                            hitbar: h.hit_bar_id,
                        }),
                    ),
                    [-1, -1],
                    false,
                );
            }
            actors[0].visible = !workbench.avatar.is_empty();
            let mut config = Config {
                cycle: animation_origin.wrapping_add(movement_tick as i32),
                render_cycle: frame,
                viewport: [
                    scene_view.left,
                    scene_view.top,
                    scene_view.width,
                    scene_view.height,
                ],
                public_filter: ui_vm.public_chat,
                effects_disabled: ui_vm.chat_effects_disabled,
                font_height: 0,
                prayer_count: 0,
                default_bar: [0, 0],
                custom_bars: BTreeMap::new(),
                hints: ui_vm
                    .minimap
                    .hints
                    .iter()
                    .flatten()
                    .filter(|h| matches!(h.kind, 1 | 10))
                    .map(|h| rs09_client::overheads::Hint {
                        npc: h.kind == 1,
                        target: h.actor,
                        sprite: h.sprite,
                    })
                    .collect(),
            };
            // ScriptRunner.projectToScreen, with the pose the scene was drawn
            // from (shakes included) and relative to the viewport.
            let overhead_scene = scene_occlusion.scene(&workbench);
            let project = |i: usize, offset: i32| {
                scene_view.project(
                    &camera_pose,
                    scene_camera::local(fines[i], workbench.navigation.origin),
                    offset,
                    workbench.ground.plane,
                    &overhead_scene,
                )
            };
            match ui_renderer.overheads(&ui.pack, &workbench.pack, &actors, &mut config, project) {
                Ok(count) => overhead_draw_count = count,
                Err(e) => ui.diagnostic(format!("Overheads: {e}")),
            }
            // MiniMap.renderHeadHints, drawn after the overheads through the
            // same ScriptRunner.projectToScreen.
            let tile_hints = rs09_client::hint_arrow::tile_sprites(
                &ui_vm.minimap.hints,
                workbench.navigation.origin,
                config.cycle,
                [config.viewport[0], config.viewport[1]],
                |fine, above| {
                    scene_view.project(
                        &camera_pose,
                        fine,
                        above,
                        workbench.ground.plane,
                        &overhead_scene,
                    )
                },
            );
            if let Err(e) = ui_renderer.tile_hints(&workbench.pack, &tile_hints) {
                ui.diagnostic(format!("Tile hints: {e}"));
            }
        }
        // Player.renderCross: the click cross is part of the scene, drawn at
        // the click's window position inside the viewport's clip and under
        // the interface drawn after it.
        if let Some(cross) = &click_cross {
            if let Some(sprite_frame) = cross.frame(get_time()) {
                camera.clip(true);
                // Cache archive 8's named "cross" sprite group in the visual pack.
                let drawn =
                    ui_renderer.draw_sheet_frame(&workbench.pack, 299, sprite_frame, cross.at);
                camera.clip(false);
                if let Err(e) = drawn {
                    if let Some(ui) = &mut ui {
                        ui.diagnostic(e);
                    }
                    click_cross = None;
                }
            } else {
                click_cross = None;
            }
        }
        // The workbench's status line is no part of the original's frame: in
        // a live session it is drawn only with the workbench bindings
        // (`--workbench-keys`), where it would otherwise cover the hover
        // text, and never over the title scene. The same text still goes to
        // the page's status badge in the browser.
        if live_mode && self.workbench_keys && !(at_title && title_world.active()) {
            draw_rectangle(8.0, 8.0, 440.0, 32.0, Color::new(0.05, 0.065, 0.08, 0.92));
            draw_text(&route_status, 18.0, 30.0, 20.0, Color::from_hex(0xe9ce96));
        }
        if npc_showcase {
            for npc in &workbench.npcs {
                let p = vec2(
                    npc.motion.fine[0] as f32 / 128.0,
                    npc.motion.fine[1] as f32 / 128.0,
                );
                let Some(render_id) = npc.render_id else {
                    continue;
                };
                let body = &workbench.npc_types[&render_id];
                let top = body
                    .model
                    .vertices
                    .iter()
                    .map(|v| -v[1])
                    .max()
                    .unwrap_or(128) as f32
                    / 128.0
                    * body.definition.resize[1] as f32
                    / 128.0;
                let projected = camera.pixel(vec3(
                    p.x - 3200.0,
                    actor_height(
                        &workbench.regions,
                        workbench.navigation.origin,
                        workbench.ground.plane as usize,
                        p.x,
                        p.y,
                    ) + top
                        + 0.3,
                    p.y - 3200.0,
                ));
                if let Some(projected) = projected {
                    let label = &body.definition.name;
                    let width = measure_text(label, None, 16, 1.0).width;
                    let x = projected.x - width * 0.5;
                    let y = projected.y;
                    draw_text(label, x + 1.0, y + 1.0, 16.0, BLACK);
                    draw_text(label, x, y, 16.0, Color::from_hex(0xf7e8bf));
                }
            }
        }
        if !show_ui {
            draw_rectangle(
                0.0,
                0.0,
                screen_width(),
                99.0,
                Color::new(0.05, 0.065, 0.08, 0.92),
            );
            draw_text(
                "2009scape / Rust visual workbench",
                22.0,
                29.0,
                25.0,
                Color::from_hex(0xe9ce96),
            );
            draw_text(
                format!(
                    "{} | {}, {} | zoom {} | seq {} | tick {} | {} FPS | tween {} | roofs {} | test sound {} | shadows {} | light {}",
                    workbench.name,
                    position.x as i32,
                    position.y as i32,
                    follow.zoom,
                    sequence,
                    ticks as u64,
                    get_fps(),
                    tween,
                    roofs,
                    audio_played,
                    // Scenery and character shadows are two preferences.
                    match (shadows_enabled, character_shadows) {
                        (true, true) => "true",
                        (false, false) => "false",
                        (true, false) => "scenery",
                        (false, true) => "characters",
                    },
                    brightness
                ),
                22.0,
                53.0,
                19.0,
                WHITE,
            );
            draw_text(
                "Click objects / Right click options / WASD walk / Ctrl run   Arrows or middle drag orbit / Wheel zoom   R roofs   Space pause",
                22.0,
                78.0,
                16.0,
                Color::from_hex(0xaebcc8),
            );
            draw_rectangle(
                0.0,
                screen_height() - 34.0,
                screen_width(),
                34.0,
                Color::new(0.05, 0.065, 0.08, 0.9),
            );
            draw_text(
                format!(
                    "LOCAL | N NPCs: {} ({}) | {} | H shadows / B light / P {} | F sound {} / G next / M mute {}",
                    workbench.npcs.len(),
                    if live_mode {
                        "server packets"
                    } else if npc_replay.is_some() {
                        "packet replay"
                    } else if npc_showcase {
                        "showcase"
                    } else {
                        "pinned spawns"
                    },
                    route_status,
                    if corrected_lights {
                        "corrected"
                    } else {
                        "pinned"
                    },
                    sounds.get(sound_index).map_or(0, |s| s.0),
                    muted
                ),
                18.0,
                screen_height() - 12.0,
                16.0,
                Color::from_hex(0xd9c393),
            );
        }
        ui_renderer.inventory_press_presented = None;
        if show_ui && let Some(ui) = &mut ui {
            if let Some(map) = &mut minimap {
                map.fine = motion.fine;
                map.state = ui_vm.minimap.state;
                map.flag = (destination_flag.tile[0] != 0).then_some([
                    destination_flag.tile[0] + workbench.navigation.origin[0],
                    destination_flag.tile[1] + workbench.navigation.origin[1],
                ]);
                map.markers.clear();
                map.hints.clear();
                if rs09_client::minimap::hints_visible(ui_vm.cycle) {
                    for hint in ui_vm.minimap.hints.iter().flatten() {
                        let fine = match hint.kind {
                            1 => workbench
                                .npcs
                                .iter()
                                .find(|n| n.index as i32 == hint.actor)
                                .map(|n| n.motion.fine),
                            2 => Some(hint.tile.map(|n| n * 128 + 64)),
                            10 if hint.actor == rs09_protocol::game::players::SELF as i32 => {
                                Some(motion.fine)
                            }
                            10 => live_actors.get(&(hint.actor as usize)).map(|a| a.fine),
                            _ => None,
                        };
                        if let Some(fine) = fine {
                            map.hints.push((hint.sprite, fine));
                        }
                    }
                }
                let dots = map.asset("mapdots").expect("minimap mapdots");
                let functions = map.asset("mapfunction").expect("minimap mapfunction");
                for tile in workbench
                    .ground
                    .occupied()
                    .into_iter()
                    .filter(|t| t.plane == workbench.ground.plane)
                {
                    map.markers.push(minimap_render::Marker {
                        group: dots,
                        frame: 0,
                        fine: [
                            (tile.x + workbench.navigation.origin[0]) * 128 + 64,
                            (tile.y + workbench.navigation.origin[1]) * 128 + 64,
                        ],
                    });
                }
                for npc in &workbench.npcs {
                    if let Ok(Some(id)) = workbench.npc_choice(npc.id, &ui_vm, Some(ui)) {
                        // A morph child the scene did not load (the
                        // title scene loads base types only) has no dot.
                        let Some(d) = workbench.npc_definitions.get(&id) else {
                            continue;
                        };
                        if d.minimap_display && d.interactive {
                            map.markers.push(minimap_render::Marker {
                                group: if d.minimap_marker == -1 {
                                    dots
                                } else {
                                    functions
                                },
                                frame: if d.minimap_marker == -1 {
                                    1
                                } else {
                                    d.minimap_marker as usize
                                },
                                fine: npc.motion.fine,
                            });
                        }
                    }
                }
                let team = live_decoded
                    .get(&rs09_protocol::game::players::SELF)
                    .map_or(Ok(0), |d| map.player_team(ui, d))
                    .unwrap_or_else(|e| {
                        ui.diagnostic(e);
                        0
                    });
                for index in &player_state.local_ids {
                    if let (Some(actor), Some(d)) =
                        (live_actors.get(index), live_decoded.get(index))
                    {
                        let peer_team = map.player_team(ui, d).unwrap_or_else(|e| {
                            ui.diagnostic(e);
                            0
                        });
                        // A key decode37 rejects has no player in the original.
                        let Some(name) = rs09_client::minimap::username_key(d.name37) else {
                            continue;
                        };
                        map.markers.push(minimap_render::Marker {
                            group: dots,
                            frame: rs09_client::minimap::player_dot(
                                name,
                                ui_vm.social.friends.iter().map(|f| (f.key, f.world)),
                                ui_vm.social.clan_members.iter().map(|m| m.key),
                                team,
                                peer_team,
                            ),
                            fine: actor.fine,
                        });
                    }
                }
                if let Err(e) = map.prepare(&workbench, &ui_vm, ui) {
                    ui.diagnostic(format!("Minimap: {e}"));
                }
                ui.world_map.scene_jitter = map.scene_jitter();
            }
            ui_models.set_brightness(brightness);
            ui_models.begin();
            let model_context = ui_model_render::Context {
                world: &workbench,
                vm: &ui_vm,
                players: &live_decoded,
                self_index: connection
                    .as_ref()
                    .and_then(|s| s.world.as_ref())
                    .map_or(0, |w| w.player_index as usize),
                live: live_mode,
            };
            ui_renderer.inventory_drag = ui_drag.inventory_visual;
            // applyClientCode's inputs: the client loop, and the local
            // player's name and movement animation for the body on the
            // character design screens.
            ui_renderer.client_codes = rs09_client::ui::client_code::Inputs {
                cycle: ui_vm.cycle,
                default_female: false,
                self_name37: live_decoded
                    .get(&rs09_protocol::game::players::SELF)
                    .map(|raw| raw.name37),
                movement: actor_states
                    .get(&rs09_protocol::game::players::SELF)
                    .map_or([-1, 0, 0], |state| {
                        [state.movement.id, state.movement.frame, state.movement.next]
                    }),
            };
            ui_renderer.scene_delta = ui_scene_delta;
            ui_renderer.title_scene_drawn = title_world.active();
            ui_renderer.excluded = ui_drag.visual.map(|n| n.key);
            ui_renderer.draw(
                ui,
                &workbench.pack,
                &ui_nodes,
                minimap.as_mut(),
                &mut ui_models,
                &model_context,
            );
            ui_renderer.excluded = None;
            ui_drag.inventory_presented(ui_renderer.inventory_drag);
            if let Some(node) = ui_drag.visual {
                ui_renderer.alpha_override =
                    (!ui.components[&node.key].drag_render_behavior).then_some(128);
                ui_renderer.draw(
                    ui,
                    &workbench.pack,
                    &[node],
                    minimap.as_mut(),
                    &mut ui_models,
                    &model_context,
                );
                ui_renderer.alpha_override = None;
            }
        }
        ui_scene_delta = 0;
        if let Some(ui) = &mut ui {
            if let Err(e) = ui_renderer.menu(&ui.pack, &workbench.pack, &context_menu) {
                ui.diagnostic(e);
            }
            // LoginManager.processInterface: the menu, or else the hover text.
            if !context_menu.menu.open {
                use rs09_client::ui::action_tooltip;
                // With the interface hidden the workbench header covers the
                // top 99 pixels, so the text goes under it instead.
                let workbench_view = (!show_ui).then_some([0, 99]);
                let selection = action_tooltip::Selection {
                    item: ui.selected_item.as_ref().map(|item| item.text.as_str()),
                    target: ui
                        .targeting
                        .active
                        .then_some((ui.targeting.verb.as_str(), ui.targeting.text.as_str())),
                };
                if action_tooltip::visible(hover_menu.entries.len(), selection) {
                    let text = action_tooltip::text(&hover_menu.entries, selection, shift_click);
                    let drawn = ui_renderer.action_tooltip(
                        ui,
                        &workbench.pack,
                        &ui_nodes,
                        workbench_view,
                        text,
                        tooltip_seed.seed,
                    );
                    if let Err(e) = drawn {
                        ui.diagnostic(e);
                    }
                }
            }
        } else {
            context_menu.draw();
        }
        // The cursor of the top entry, else the script's; none under the menu.
        let default_cursor = ui
            .as_ref()
            .filter(|ui| ui.targeting.active)
            .map_or(-1, |ui| ui.targeting.default_cursor);
        let wanted_cursor = rs09_client::cursor::select(
            &hover_menu.entries,
            shift_click,
            context_menu.menu.open,
            default_cursor,
            ui_vm.script_cursor.0,
        );
        let cursor_packs: Vec<&Pack> = ui
            .iter()
            .map(|ui| &ui.pack)
            .chain([&workbench.pack])
            .collect();
        let cursor_error = cursor_view.set(wanted_cursor, ui_vm.preferences.cursors, &cursor_packs);
        if let (Some(ui), Some(error)) = (&mut ui, cursor_error) {
            ui.diagnostic(error);
        }
        cursor_view.draw();
        let inventory_press_active = ui.as_ref().is_some_and(|u| u.inventory_press_active());
        if !live_mode
            && !live_decoded.is_empty()
            && (frame % 25 == 0 || inventory_press_active || published_inventory_press_active)
        {
            rs09_client::launch::publish(&serde_json::json!({
                "status":"Offline appearance preview", "in_game":false, "preview":true,
                "player_forms":live_forms, "rendered_body_parts":workbench.avatar.len(),
                "rendered_body_triangles":avatar.indices.len()/3, "model_height":self_height,
                "movement":{"fine":motion.fine,"sequence":motion.sequence,"frame":motion.frame},
                "npc_views":workbench.npc_views(),"slot_poses":*workbench.slot_views.borrow(), "menu":menu_snapshot(&context_menu.menu),
                "last_action":outbox.last().map(|f|serde_json::json!({"opcode":f.opcode,"payload":f.payload})),
                "cycle":world_ticks as i32, "diagnostics":ui.as_ref().map(|u|&u.diagnostics),
                "inventory":ui.as_ref().map(inventory_snapshot), "weight":ui_vm.weight,
                "hovered_widget":ui.as_ref().and_then(|u|u.hovered_key()),"waiting_widget":ui.as_ref().and_then(|u|u.waiting_key()),
                "tooltip_widget":ui.as_ref().and_then(|u|u.tooltip_key()),"tooltip_timer":ui.as_ref().map(|u|u.tooltip_timer()),
                "scrollbars":ui.as_ref().map(|u|u.scrollbar_snapshot()),
                "inventory_drag":ui_drag.snapshot(ui.as_ref()),
                "inventory_press":ui.as_ref().map(|u|u.inventory_press_snapshot()),
                "inventory_press_draw":&ui_renderer.inventory_press_presented,
                "latest_chat":ui_vm.messages.first(),"legacy_buttons":ui.as_ref().map(|u|ui_vm.legacy_button_snapshot(u)),
            }));
        }
        published_inventory_press_active = inventory_press_active;
        if ui_debug {
            draw_rectangle(8.0, 8.0, 700.0, 128.0, Color::new(0.0, 0.0, 0.0, 0.85));
            draw_text(
                format!(
                    "{} actions: {} | {} | overhead draws {}",
                    if live_mode { "Live" } else { "Offline" },
                    outbox.len(),
                    route_status,
                    overhead_draw_count
                ),
                16.0,
                26.0,
                16.0,
                WHITE,
            );
            if let Some(last) = outbox.last() {
                draw_text(
                    format!("Last opcode {}: {:02x?}", last.opcode, last.payload),
                    16.0,
                    46.0,
                    16.0,
                    WHITE,
                );
            }
            if let Some(ui) = &ui {
                for (i, d) in ui.diagnostics.iter().rev().take(4).enumerate() {
                    draw_text(d, 16.0, 68.0 + i as f32 * 17.0, 15.0, ORANGE);
                }
            }
        }
        // GameShell.paintFrameLetterbox: black around a canvas smaller than
        // the window, before anything looks at the finished frame.
        display_host::letterbox();
        if let Some(life) = &mut session_life {
            life.end_frame(&mut ui_renderer, ui.as_ref(), &workbench.pack, &title_world);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if capture_models && frame <= capture_frame.unwrap_or(100) && frame % 6 == 0 {
            std::fs::create_dir_all("runs/visual/ui-model-frames").unwrap();
            get_screen_data()
                .export_png(&format!("runs/visual/ui-model-frames/{:03}.png", frame / 6));
            use std::io::Write;
            let mut trace = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(frame != 0)
                .truncate(frame == 0)
                .open("runs/visual/ui-model-frames/trace.jsonl")
                .unwrap();
            writeln!(trace,"{}",serde_json::json!({"frame":frame,"cycle":ui_vm.cycle,"draws":ui_models.draws,"models":ui_models.last})).unwrap();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if capture_animation && frame <= capture_frame.unwrap_or(180) && frame % 6 == 0 {
            std::fs::create_dir_all("runs/visual/actor-frames").unwrap();
            get_screen_data().export_png(&format!("runs/visual/actor-frames/{:03}.png", frame / 6));
            use std::io::Write;
            let mut trace = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(frame != 0)
                .truncate(frame == 0)
                .open("runs/visual/actor-frames/frames.jsonl")
                .unwrap();
            let action = actor_states
                .get(&rs09_protocol::game::players::SELF)
                .map(|s| &s.action);
            #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
            let ambient_snapshot = ambient_audio.snapshot();
            #[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
            let ambient_snapshot = serde_json::Value::Null;
            writeln!(trace,"{}",serde_json::json!({"file":format!("{:03}.png",frame/6),"time":get_time(),"cycle":animation_origin.wrapping_add(movement_tick as i32),"action":action.map(|t|[t.id,t.frame,t.clock,t.delay]),"destination_flag":destination_flag.tile,"movement":{"fine":motion.fine,"head":motion.head,"sequence":motion.sequence,"frame":motion.frame,"queue":motion.queue,"server_tile":player_state.self_player().tile(),"base":live_base},"players":player_state.local_ids.iter().filter_map(|id|live_actors.get(id).map(|m|serde_json::json!({"index":id,"fine":m.fine,"sequence":m.sequence,"frame":m.frame}))).collect::<Vec<_>>(),"npcs":npc_snapshot(&live_npcs),"npc_views":workbench.npc_views(),"slot_poses":*workbench.slot_views.borrow(),"spots":workbench.spot_views,"ambient_audio":ambient_snapshot,"sounds_submitted":ui_audio.submitted,"overhead_commands":overhead_draw_count,"overheads":workbench.overheads.iter().map(|(&(npc,index),state)|serde_json::json!({"npc":npc,"index":index,"state":state})).collect::<Vec<_>>(),"live":live_mode})).unwrap();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if capture_npcs && (100..=395).contains(&frame) && frame % 5 == 0 {
            std::fs::create_dir_all("runs/visual/npc-frames").unwrap();
            get_screen_data().export_png(&format!(
                "runs/visual/npc-frames/{:03}.png",
                (frame - 100) / 5
            ));
        }
        if capture
            && (captured_journey_finished
                || frame
                    == capture_frame.unwrap_or(if capture_npcs {
                        400
                    } else if capture_animation {
                        180
                    } else if capture_route {
                        300
                    } else {
                        100
                    }))
        {
            #[cfg(not(target_arch = "wasm32"))]
            {
                std::fs::create_dir_all("runs/visual").unwrap();
                std::fs::write("runs/visual/render-state.json",serde_json::json!({"frustum_culling":frustum_culling,"static_gpu":static_renderer.as_ref().map(static_render::Renderer::statistics),"eligible_chunks":eligible_chunks,"visible_chunks":visible_chunks,"eligible_static_triangles":eligible_triangles,"submitted_static_triangles":submitted_triangles,"frames_after_warmup":frame.saturating_sub(20),"seconds_after_warmup":render_profile_started.map(|start|get_time()-start),"static_submission_seconds":static_submission_seconds,"scope":"Static-world CPU submission for the selected persistent/streaming path and elapsed render frames, after20 warmup frames; excludes loading. GPU wait, other app work and presentation affect elapsed time. No JVM performance comparison."}).to_string()).unwrap();
                #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                {
                    let mut audio = ui_audio
                        .capture_pcm("runs/visual/client-audio.wav")
                        .expect("client PCM capture");
                    audio["location_ambient"] = ambient_audio.snapshot();
                    std::fs::write("runs/visual/audio-state.json", audio.to_string())
                        .expect("client audio state");
                }
                std::fs::write(
                    "runs/visual/widget-sprite-state.json",
                    ui_renderer.sprite_snapshot().to_string(),
                )
                .expect("widget sprite readback");
                let actor = actor_states.get(&rs09_protocol::game::players::SELF);
                std::fs::write(
                    "runs/visual/ui-model-state.json",
                    serde_json::to_vec_pretty(
                        &serde_json::json!({"draws":ui_models.draws,"last":ui_models.last}),
                    )
                    .unwrap(),
                )
                .unwrap();
                let models: Vec<_> = workbench.avatar.iter().map(|m| {
                    let checksum=m.vertices.iter().flatten().fold(0xcbf29ce484222325u64,|h,&n|(h^(n as u32 as u64)).wrapping_mul(0x100000001b3));
                    serde_json::json!({"vertices":m.vertices.len(),"faces":m.faces.len(),"vertex_checksum":format!("{checksum:016x}")})
                }).collect();
                std::fs::write("runs/visual/player-model-state.json",serde_json::json!({"models":models,"overrides":body_overrides,"forms":live_forms,"appearances":live_decoded.iter().map(|(&i,a)|serde_json::json!({"index":i,"slots":a.slots,"female":a.female(),"bas":a.bas})).collect::<Vec<_>>(),"scope":"Raw assembled bodies submitted to the existing GPU pose/render path; explicit offline appearances do not exercise equipment transactions."}).to_string()).unwrap();
                std::fs::write("runs/visual/spot-state.json",serde_json::json!({"views":workbench.spot_views,"world_effects":workbench.world_spots.effects,"cached_models":workbench.spot_models.len(),"definitions":workbench.spot_definitions.len(),"tracks":actor_states.iter().map(|(index,s)|serde_json::json!({"index":index,"id":s.spot.id,"frame":s.spot.frame,"next":s.spot.next,"clock":s.spot.clock,"height":s.spot_height,"start":s.spot_start})).collect::<Vec<_>>()}).to_string()).unwrap();
                std::fs::write("runs/visual/animation-state.json",serde_json::json!({"cycle":animation_origin.wrapping_add(movement_tick as i32),"primary":actor.map(|s|serde_json::json!({"id":s.action.id,"frame":s.action.frame,"next":s.action.next,"clock":s.action.clock,"delay":s.action.delay,"repeats":s.action.repeats})),"movement_sequence":motion.sequence,"movement_frame":motion.frame,"fine":motion.fine,"head":motion.head,"capture_route_issued":capture_route_issued,"destination_flag":destination_flag.tile,"sound_sequence_triggers":ui_audio.sequence_triggers,"sound_backend_submissions":ui_audio.submitted,"overhead_commands":overhead_draw_count,"overheads":workbench.overheads.iter().map(|(&(npc,index),state)|serde_json::json!({"npc":npc,"index":index,"state":state})).collect::<Vec<_>>(),"live":live_mode,"capture_frame":frame}).to_string()).unwrap();
                std::fs::write(
                    "runs/visual/menu-state.json",
                    menu_snapshot(&context_menu.menu).to_string(),
                )
                .unwrap();
                if std::env::args().any(|arg| arg == "--pick-probe") {
                    let rect = viewport.map_or(
                        [0.0, 0.0, screen_width(), screen_height()],
                        |(whole, _)| {
                            [
                                whole.x as f32,
                                whole.y as f32,
                                whole.w as f32,
                                whole.h as f32,
                            ]
                        },
                    );
                    let picks = world_pick::probe(
                        &mut world_picker,
                        &world_pick::Sources {
                            world: &workbench,
                            players: &player_picks,
                            piles: &ground_renderer,
                        },
                        &input_camera,
                        rect,
                        roofs,
                    );
                    std::fs::write(
                        "runs/visual/pick-state.json",
                        serde_json::to_vec_pretty(&picks).unwrap(),
                    )
                    .unwrap();
                }
                std::fs::write("runs/visual/npc-state.json",serde_json::json!({"npcs":npc_snapshot(&live_npcs),"npc_views":workbench.npc_views(),"slot_poses":*workbench.slot_views.borrow(),"diagnostics":live_diagnostics,"fixture_remaining":npc_replay.as_ref().map(|r|r.frames.len()),"sounds_submitted":ui_audio.submitted}).to_string()).unwrap();
                if let Some(map) = &minimap {
                    let background = map
                        .capture("runs/visual/minimap.png")
                        .expect("minimap capture");
                    std::fs::write("runs/visual/minimap-state.json",serde_json::json!({"background_captured":background,"server":ui_vm.minimap,"yaw":map.view.yaw,"angle_offset":map.view.angle_offset,"zoom_offset":map.view.zoom_offset,"fine":map.fine,"origin":map.origin,"flag":map.flag,"markers":map.markers.len(),"marker_frames":map.markers.iter().map(|m|(m.group,m.frame)).collect::<Vec<_>>(),"label_draws":map.label_draws,"rasters":map.rasters,"drift":drift,"visible_hints":map.hints,"last_click_offset":map.click_offset}).to_string()).expect("minimap state capture");
                }
                let output = option("--output");
                if let Some(ui) = &ui {
                    std::fs::write("runs/visual/ui-state.json",serde_json::to_vec(&serde_json::json!({"hovered_widget":ui.hovered_key(),"waiting_widget":ui.waiting_key(),"scrollbars":ui.scrollbar_snapshot(),"inventory_drag":ui_drag.snapshot(Some(ui)),"inventory_press":ui.inventory_press_snapshot(),"inventory_press_draw":&ui_renderer.inventory_press_presented,"latest_chat":ui_vm.messages.first(),"legacy_buttons":ui_vm.legacy_button_snapshot(ui),"targeting":&ui.targeting,"root":ui.root,"subinterfaces":ui.subinterfaces,"components":ui.components.len(),"visible_nodes":ui_nodes.len(),"inventory":inventory_snapshot(ui),"varcs":ui_vm.varcs,"diagnostics":ui.diagnostics,"scripts_executed":ui_vm.executed,"boosted_levels":ui_vm.boosted_levels.to_vec(),"base_levels":ui_vm.base_levels.to_vec(),"experience":ui_vm.experience.to_vec(),"run_energy":ui_vm.run_energy,"weight":ui_vm.weight,"item_fallbacks":ui.items.fallbacks})).unwrap()).unwrap();
                }
                let player_screen = scene_view.project(
                    &camera_pose,
                    scene_camera::local(motion.fine, workbench.navigation.origin),
                    50,
                    workbench.ground.plane,
                    &scene_occlusion.scene(&workbench),
                );
                std::fs::write("runs/visual/camera-state.json",serde_json::to_vec(&serde_json::json!({"viewport":[scene_view.left,scene_view.top,scene_view.width,scene_view.height],"viewport_zoom":scene_view.zoom,"frustum_edges":scene_view.edges().map(|e|[e.left,e.right,e.top,e.bottom]),"pose":{"position":camera_pose.position,"pitch":camera_pose.pitch,"yaw":camera_pose.yaw},"pitch_target":follow.pitch_target,"yaw_target":follow.yaw_target,"orbit_point":follow.position,"camera_zoom":follow.zoom,"max_pitch_distance":follow.max_pitch_distance,"locked":camera_script.locked,"player_fine":scene_camera::local(motion.fine, workbench.navigation.origin),"player_in_viewport":player_screen,"scope":"The follow camera's state and the scene viewport at capture time; player_in_viewport is ScriptRunner.projectToScreen of the point 50 units above the local player, relative to the viewport."})).unwrap()).unwrap();
                let center = workbench.start;
                let locations: Vec<_> = workbench
                    .world
                    .locations
                    .iter()
                    .filter(|(slot, _)| {
                        (slot.tile[0] - center[0]).abs() <= 3
                            && (slot.tile[1] - center[1]).abs() <= 3
                    })
                    .map(|(slot, loc)| {
                        serde_json::json!([
                            slot.plane,
                            slot.tile[0],
                            slot.tile[1],
                            slot.layer,
                            loc.id,
                            loc.shape,
                            loc.rotation
                        ])
                    })
                    .collect();
                let mut flags = Vec::new();
                for plane in 0..4 {
                    for x in center[0] - 3..=center[0] + 3 {
                        for y in center[1] - 3..=center[1] + 3 {
                            flags.push(serde_json::json!([
                                plane,
                                x,
                                y,
                                workbench.navigation.maps[plane].get(
                                    x - workbench.navigation.origin[0],
                                    y - workbench.navigation.origin[1]
                                )
                            ]));
                        }
                    }
                }
                std::fs::write("runs/visual/scene-state.json",serde_json::to_vec(&serde_json::json!({"center":center,"origin":workbench.navigation.origin,"capture_frame":frame,"world_tick":world_ticks as i32,"replay_frames_remaining":replay.len(),"locations":locations,"collision_flags":flags})).unwrap()).unwrap();
                std::fs::write("runs/visual/navigation-state.json",serde_json::to_vec(&serde_json::json!({"origin":workbench.navigation.origin,"planes":workbench.navigation.maps.iter().map(|map|map.flags()).collect::<Vec<_>>(),"scope":"Complete bounded client navigation input at capture time; scene construction and accepted server movement are separate contracts."})).unwrap()).unwrap();
                std::fs::write("runs/visual/ground-state.json",serde_json::to_vec(&serde_json::json!({"piles":ground_renderer.snapshot(),"stacks":workbench.ground.occupied().iter().map(|&tile|serde_json::json!({"plane":tile.plane,"tile":[tile.x+workbench.navigation.origin[0],tile.y+workbench.navigation.origin[1]],"items":workbench.ground.stack(tile).unwrap().unwrap().items().iter().map(|r| {let i=r.get();serde_json::json!({"serial":r.serial(),"id":i.id,"amount":i.amount})}).collect::<Vec<_>>()})).collect::<Vec<_>>(),"scope":"Shared original ground-state reducer, cached item geometry and visible pile/minimap/picking. Offline actions emit packets; live pickup/inventory transactions require server handlers."})).unwrap()).unwrap();
                get_screen_data().export_png(if let Some(path) = &output {
                    path
                } else if capture_npcs {
                    "runs/visual/npcs.png"
                } else if capture_route {
                    "runs/visual/route.png"
                } else if portrait {
                    "runs/visual/wave.png"
                } else if !shadows_enabled {
                    "runs/visual/lumbridge-unshadowed.png"
                } else {
                    "runs/visual/lumbridge.png"
                });
                let environments: Vec<_> = workbench.regions.iter().map(|r| serde_json::json!({"region":[r.x,r.z],"light_color":r.environment.color,"ambient":r.environment.ambient,"diffuse":r.environment.diffuse,"direction":r.environment.direction,"fog_color":r.environment.fog_color,"fog_depth":r.environment.fog_depth,"point_light_records":r.point_lights.len()})).collect();
                std::fs::write(
                    "runs/visual/actions.json",
                    serde_json::json!(
                        outbox
                            .iter()
                            .map(|f| serde_json::json!({"opcode":f.opcode,"payload":f.payload}))
                            .collect::<Vec<_>>()
                    )
                    .to_string(),
                )
                .unwrap();
                std::fs::write("runs/visual/loaded.json",serde_json::json!({"models":workbench.models.len(),"placed_meshes":workbench.objects,"animated_locations":workbench.animated_locations.len(),"triangles":workbench.faces,"frames":workbench.frames.len(),"native_texture_graphs":workbench.textures.generated,"sound_effects":sounds.len(),"audio_playback_submitted":audio_played,"environments":environments,"position":[position.x,position.y],"move_packet":motion.pending_move_packet,"route_status":route_status,"shadows_enabled":shadows_enabled,"brightness":brightness,"static_shadow_casters":workbench.static_shadows.len(),"shadow_projection":workbench.shadows.light,"point_lights":workbench.point_lights.lights.len(),"corrected_light_placement":corrected_lights,"source":workbench.pack.manifest.source}).to_string()).unwrap();
            }
            return false;
        }
        self.atmosphere_tick = atmosphere_tick;
        self.light_direction = light_direction;
        self.lit_scene = lit_scene;
        self.static_submission_seconds = static_submission_seconds;
        self.ui_scene_delta = ui_scene_delta;
        self.published_inventory_press_active = published_inventory_press_active;
        self.model_cycle = model_cycle;
        self.world_spot_tick = world_spot_tick;
        self.route_status = route_status;
        self.click_cross = click_cross;
        self.last_render_metrics = last_render_metrics;
        self.hover_menu = hover_menu;
        self.ui_nodes = ui_nodes;
        // The decoded appearances outlive the frame: the packets, the menu and
        // the next frame read them again.
        self.live_decoded = live_decoded;
        self.input_camera = Some(input_camera);
        self.render_profile_started = render_profile_started;
        true
    }
}
