//! Bounded native CS2 execution. Unsupported opcodes produce explicit diagnostics.
//! Client variables and component edits are presentation state; containers come from the server.
use super::{
    Component, HookArg, latin1,
    script::Script,
    state::{Hook, Key, State},
};
use rs09_protocol::buffer::Reader;
use std::{collections::BTreeMap, rc::Rc};
type Result<T> = std::result::Result<T, String>;
#[cfg(test)]
#[path = "vm_event_tests.rs"]
mod event_tests;
#[path = "vm_login.rs"]
mod login;
#[path = "vm_ops.rs"]
mod ops;
#[path = "vm_semantics.rs"]
mod semantics;
#[path = "vm_session.rs"]
mod session;
#[path = "vm_world_map.rs"]
mod world_map_ops;
pub use ops::{CameraRequest, MusicRequest, Preferences, Viewport};
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct ChatLine {
    pub kind: i32,
    pub text: String,
    pub name: String,
    pub clan: String,
    pub phrase: i32,
}
#[derive(Default)]
struct Stack {
    ints: Vec<i32>,
    strings: Vec<Option<String>>,
}
impl Stack {
    fn pop(&mut self) -> Result<i32> {
        self.ints
            .pop()
            .ok_or_else(|| "CS2 integer stack underflow".into())
    }
    fn take<const N: usize>(&mut self) -> Result<[i32; N]> {
        let start = self
            .ints
            .len()
            .checked_sub(N)
            .ok_or("CS2 integer stack underflow")?;
        let values = self.ints[start..].try_into().unwrap();
        self.ints.truncate(start);
        Ok(values)
    }
    fn string(&mut self) -> Result<Option<String>> {
        self.strings
            .pop()
            .ok_or_else(|| "CS2 string stack underflow".into())
    }
    fn text(&mut self) -> Result<String> {
        self.string()?.ok_or_else(|| "CS2 null string".into())
    }
}
struct Frame {
    id: i32,
    script: Rc<Script>,
    pc: i32,
    ints: Vec<i32>,
    strings: Vec<Option<String>>,
}
#[derive(Default)]
pub struct Vm {
    pub player_options: crate::player_interaction::Options,
    pub minimap: crate::minimap::State,
    scripts: BTreeMap<i32, Rc<Script>>,
    pub varps: BTreeMap<i32, i32>,
    /// Ordered original refreshMagicVarp/updateMulti observations. The PCM
    /// ambient binding drains these; intermediate morphs can stop an old voice
    /// even when a later update restores the same final type.
    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
    pub ambient_updates: Vec<BTreeMap<i32, i32>>,
    pub pending_varps: super::varps::Pending,
    pub varcs: BTreeMap<i32, i32>,
    /// What an unset varc reads: 0 until `reset_session_varcs`, then -1.
    pub varc_default: i32,
    pub varcstrs: BTreeMap<i32, String>,
    active: [Option<super::component_handles::Handle>; 2],
    arrays: BTreeMap<i32, Vec<i32>>,
    pub cycle: i32,
    pub members: bool,
    pub site_members: bool,
    pub world: i32,
    pub rights: i32,
    pub executed: usize,
    pub last_op: i32,
    pub last_pc: i32,
    pub last_script: i32,
    pub window_mode: i32,
    pub outgoing: Vec<rs09_protocol::game::Frame>,
    pub effects: Vec<[i32; 3]>,
    pub sound_volume: Option<i32>,
    pub inserting: i32,
    pub mouse_buttons: i32,
    pub chat_effects_disabled: i32,
    pub location: [i32; 3],
    pub combat_level: i32,
    pub boosted_levels: [i32; 25],
    pub base_levels: [i32; 25],
    pub experience: [i32; 25],
    /// Friends, ignores, the clan channel and Grand Exchange offers.
    pub social: super::social::Social,
    pub player_name: String,
    /// Original appearance hash sampled when IF_SETPLAYERHEAD arrives.
    pub player_head_id: i32,
    pub run_energy: i32,
    pub reboot_timer: i32,
    pub weight: i32,
    pub chat_lines: i32,
    pub chat_restricted: bool,
    pub messages: Vec<ChatLine>,
    pub public_chat: i32,
    pub private_chat: i32,
    pub trade_chat: i32,
    pub quick_chat: bool,
    /// Seed and state of the script random source; zero seeds from the cycle.
    pub random_state: u64,
    /// `LoginManager.blackmarks`, reported to scripts only in the band 5..=9.
    pub blackmarks: i32,
    /// The local player's appearance gender.
    pub female: bool,
    pub login_type: i32,
    /// `client.language`: 0 English, 1 German, 2 French, 3 Portuguese.
    pub language: i32,
    pub affiliate: i32,
    /// Character-design changes for the local player's appearance.
    pub design_edits: Vec<rs09_protocol::game::players::DesignEdit>,
    /// `[sequence, delay]` requests from scripts for the local player.
    pub self_animations: Vec<[i32; 2]>,
    /// Song and jingle requests from scripts and the server, oldest first.
    pub music: Vec<MusicRequest>,
    /// CS2 3107: `(option, name)` player-option clicks for the client to send.
    pub player_option_requests: Vec<(i32, String)>,
    /// CS2 3109: start dragging the active component, pressed at this offset
    /// inside it.
    pub drag_requests: Vec<(Key, [i32; 2])>,
    /// `LoginManager.reply`, `CreateManager.reply` and `disallowResult`; None
    /// is the original's starting value (-2, -2 and -1).
    pub login_reply: Option<i32>,
    pub create_reply: Option<i32>,
    pub disallow_result: Option<i32>,
    /// The login state machine behind the title screen's commands (CS2 5600
    /// to 5611 and 6500 to 6507); None leaves them with the three fields
    /// above and an empty world list.
    pub login_host: Option<Rc<std::cell::RefCell<dyn crate::login_flow::Commands>>>,
    /// CS2 5058: the quick chat phrase being composed, with its values.
    pub active_phrase: Option<(i32, super::quickchat::PhraseType, Vec<i32>)>,
    /// `Find.results`, `Find.index` and `Find.size` for the item search.
    pub find_results: Option<Vec<i32>>,
    pub find_index: i32,
    pub find_size: i32,
    pub preferences: Preferences,
    /// `Cheat`'s switches and the effects of `::` commands not yet applied.
    pub cheat: crate::cheat::State,
    /// CS2 6001 and 6015 ask the atmosphere to jump to its target instead of
    /// fading (`FogManager.setInstantFade`); the renderer clears this.
    pub instant_fade: bool,
    pub viewport: Viewport,
    /// Wall-clock milliseconds since 1970 for the date commands.
    pub now_ms: i64,
    /// `DisplayMode` and the canvas. CS2 5300 to 5307 act on it at once, as
    /// the original's do, and leave what the window has to do in its
    /// `effects`; `window_mode` above is kept as its `getWindowMode()`.
    pub display: crate::display::Display,
    /// CS2 5405/5406 camera path points, `[x, height, y, speed]` in fine units.
    pub camera_paths: [Option<Vec<[i32; 4]>>; 2],
    /// CS2 5401 script recolour palette, read by NPC and object `recol_p`.
    pub recolor: crate::recolor::Palette,
    pub camera_requests: Vec<CameraRequest>,
    /// The follow camera's `[pitch, yaw]` targets, kept current by the client.
    pub camera_angles: [i32; 2],
    /// CS2 5424 styled context menu colours and sprites; None is the plain menu.
    pub menu_style: Option<[i32; 11]>,
    pub script_cursor: crate::cursor::Id,
    /// CS2 5427 `[use, examine]` cursors for the minimap.
    pub minimap_cursors: [crate::cursor::Id; 2],
    /// SET_WALK_TEXT; None is the localized "Walk here".
    pub walk_text: Option<String>,
    /// SET_SETTINGS_STRING: the opaque cookie value the original echoes at login.
    pub settings: Option<String>,
    /// URL_OPEN requests. A native client does not open server-supplied URLs
    /// on its own; the caller decides what to show.
    pub requested_urls: Vec<String>,
    /// `GameShell.frame != null`: the client runs as an application in its
    /// own frame rather than as an applet in a page (the browser).
    pub application: bool,
    /// CS2 5411 in an application: `System.exit(0)`, which the host carries
    /// out by ending its run.
    pub exit_requested: bool,
    pub logout_on_disconnect_timer: i32,
    /// Counts `ObjTypeList.clearSprites()` calls (CS2 6001 and 5401); the
    /// presenter drops its cached item sprites when this moves.
    pub item_sprite_resets: u32,
    /// Whether Alt, Ctrl and Shift are held (`Keyboard.pressedKeys` 86, 82
    /// and 81), for CS2 5100 to 5102. The client sets it each cycle.
    pub modifier_keys: [bool; 3],
    pub(crate) last_legacy_varp: Option<i32>,
    pub(crate) legacy_varp_writes: i32,
}
impl Vm {
    /// ClientProt captures its component before the hook, which can close or
    /// replace it. Callback packets precede the outer button's wire phase.
    pub fn click_button(
        &mut self,
        state: &mut State,
        key: Key,
        op: i32,
        op_base: &str,
    ) -> Result<()> {
        let packet = {
            let vm = std::cell::RefCell::new(&mut *self);
            state.button_with_hook(
                key,
                op,
                |state, source, arguments| {
                    let mut hook = Hook::new(source.key(), arguments);
                    hook.op = op;
                    hook.op_base = op_base.to_owned();
                    if let Err(e) = vm.borrow_mut().run(state, &hook, 200000) {
                        state.diagnostic(e);
                    }
                    Ok(())
                },
                |state, source| {
                    Ok(super::legacy_buttons::special(
                        state.component_ref(source).client_code,
                        &mut vm.borrow_mut().logout_on_disconnect_timer,
                    ))
                },
            )?
        };
        if let Some(packet) = packet {
            self.outgoing.push(packet);
        }
        Ok(())
    }
    pub fn close_modal(&mut self, state: &mut State) {
        self.outgoing.push(rs09_protocol::game::Frame {
            opcode: 184,
            payload: Vec::new(),
        });
        let pointers = state
            .subinterfaces
            .iter()
            .filter(|(_, v)| v.1 == 0)
            .map(|(&key, _)| key)
            .collect::<Vec<_>>();
        for pointer in pointers {
            state.close_sub(pointer, true);
            self.drain(state);
        }
        state.clear_waiting();
    }
    /// ScriptRunner catches ordinary script exceptions. Report the completed
    /// prefix, then allow MiniMenu's outer targeting transition to continue.
    fn target_hook(&mut self, state: &mut State, key: Key, arguments: Option<Vec<HookArg>>) {
        if let Some(arguments) = arguments {
            if let Err(error) = self.run(state, &Hook::new(key, arguments), 200000) {
                state.diagnostic(error);
            }
        }
    }
    pub fn cancel_targeting(&mut self, state: &mut State) {
        if !state.targeting.active {
            return;
        }
        let key = state.targeting.source;
        let arguments = state.get(key).ok().and_then(|c| c.on_use_with.clone());
        self.target_hook(state, key, arguments);
        state.targeting.active = false;
        state.targeting.default_cursor = -1;
    }
    pub fn start_targeting(
        &mut self,
        state: &mut State,
        key: Key,
        properties: super::Properties,
        cursors: [i32; 2],
    ) {
        let arguments = state.get(key).ok().and_then(|c| c.on_use.clone());
        self.target_hook(state, key, arguments);
        state.targeting.source = key;
        state.targeting.param = properties.target_param;
        state.targeting.mask = properties.target_mask();
        state.targeting.active = true;
        state.targeting.cursor = cursors[0];
        state.targeting.default_cursor = cursors[1];
    }
    pub fn select_target(&mut self, state: &mut State, key: Key) -> Result<()> {
        if state.get(key).is_err() {
            return Ok(());
        }
        let component = state.retain_component(key)?;
        self.cancel_targeting(state);
        let properties = state.reference_properties(&component);
        let c = state.component_ref(&component);
        let cursors = [c.target_cursor, c.default_target_cursor];
        self.start_targeting(state, key, properties, cursors);
        state.selected_item = None;
        let c = state.component_ref(&component);
        let verb = super::targeting::target_verb(c, state.reference_properties(&component))
            .unwrap_or("Null")
            .to_string();
        let text = if c.if3 {
            format!("{}<col=ffffff>", c.option_base)
        } else {
            format!("<col=00ff00>{}<col=ffffff>", c.option_suffix)
        };
        state.targeting.verb = verb;
        state.targeting.text = text;
        Ok(())
    }
    pub fn update_skill(
        &mut self,
        state: &mut State,
        skill: i32,
        boosted: i32,
        experience: i32,
    ) -> Result<()> {
        if !(0..25).contains(&skill) {
            return Err(format!("Invalid skill {skill}"));
        }
        let i = skill as usize;
        self.experience[i] = experience;
        self.boosted_levels[i] = boosted;
        self.base_levels[i] = super::skills::base_level(experience);
        state.updated_stats[(state.stat_writer & 31) as usize] = skill;
        state.stat_writer = state.stat_writer.wrapping_add(1);
        Ok(())
    }
    pub fn add_message(&mut self, state: &mut State, line: ChatLine) {
        self.messages.insert(0, line);
        self.messages.truncate(100);
        self.chat_lines = self.chat_lines.wrapping_add(1);
        state.chat_transmit_at = state.transmit_timer;
    }
    /// Run script packets immediately, as Protocol.readPacket does. Delayed state
    /// changes are applied separately at the original per-cycle polling boundary.
    pub fn apply_frame(
        &mut self,
        state: &mut State,
        frame: &rs09_protocol::game::Frame,
    ) -> Result<bool> {
        use rs09_protocol::game::updates::{ScriptArg, ServerUpdate};
        self.drain(state);
        if self
            .player_options
            .apply(frame)
            .map_err(|e| e.to_string())?
        {
            return Ok(true);
        }
        if self.minimap.apply(frame).map_err(|e| e.to_string())? {
            return Ok(true);
        }
        if let Some([id, loops, delay]) =
            crate::sound_queue::synth_frame(frame).map_err(|e| e.to_string())?
        {
            if self.sound_volume.unwrap_or(127) != 0
                && loops != 0
                && id != -1
                && self.effects.len() < 50
            {
                self.effects.push([id, loops, delay]);
            }
            return Ok(true);
        }
        let update = ServerUpdate::decode(frame).map_err(|e| e.to_string())?;
        match update {
            Some(ServerUpdate::ChatSettings(filters)) => {
                [self.public_chat, self.private_chat, self.trade_chat] = filters.map(i32::from);
                return Ok(true);
            }
            Some(ServerUpdate::GameMessage(text)) => {
                self.game_message(state, &text)?;
                return Ok(true);
            }
            Some(ServerUpdate::CloseInterface { counter, pointer }) => {
                state.verify_id = (counter + 1) & 65535;
                state.close_sub(pointer, true);
                self.drain(state);
                state.clear_waiting();
                return Ok(true);
            }
            Some(ServerUpdate::InterfacePlayerHead { pointer, counter }) => {
                state.verify_id = (counter + 1) & 65535;
                state
                    .delayed
                    .server_values(4, pointer, [3, -1, self.player_head_id], None);
                self.drain(state);
                return Ok(true);
            }
            Some(ServerUpdate::Skill {
                skill,
                boosted,
                experience,
            }) => {
                self.update_skill(state, skill, boosted, experience)?;
                return Ok(true);
            }
            Some(ServerUpdate::RunEnergy { energy }) => {
                self.run_energy = energy;
                state.misc_transmit_at = state.transmit_timer;
                return Ok(true);
            }
            Some(ServerUpdate::RunWeight { weight }) => {
                self.weight = weight;
                state.misc_transmit_at = state.transmit_timer;
                return Ok(true);
            }
            _ => {}
        }
        if let Some(ServerUpdate::OpenInterface {
            pointer, interface, ..
        }) = &update
        {
            if let Some(&(old, _)) = state.subinterfaces.get(pointer) {
                state.close_sub(*pointer, old != *interface);
                self.drain(state);
            }
        }
        if let Some(ServerUpdate::Varp { id, value }) = update {
            self.pending_varps
                .server(id, value, i64::from(self.cycle) * 20);
            return Ok(true);
        }
        if let Some(ServerUpdate::Varbit { id, mut value }) = update {
            let (base, start, mask) = self.varbit_parts(state, id)?;
            if value < 0 || value > mask {
                value = 0;
            }
            let shifted = mask.wrapping_shl(start as u32);
            let old = self.pending_varps.values.get(&base).copied().unwrap_or(0);
            self.pending_varps.server(
                base,
                (old & !shifted) | (value.wrapping_shl(start as u32) & shifted),
                i64::from(self.cycle) * 20,
            );
            return Ok(true);
        }
        if let Some(ServerUpdate::RunClientScript {
            counter,
            id,
            arguments,
            ..
        }) = update
        {
            state.verify_id = (counter + 1) & 65535;
            let mut args = vec![HookArg::Int(id)];
            args.extend(arguments.into_iter().map(|arg| match arg {
                ScriptArg::Int(n) => HookArg::Int(n),
                ScriptArg::Text(bytes) => HookArg::Text(latin1(&bytes)),
            }));
            self.run(state, &Hook::new(Key::component(-1), args), 200_000)?;
            Ok(true)
        } else {
            let mut consumed = state.apply_frame(frame)?;
            if !consumed {
                if let Some(event) = rs09_protocol::game::events::ServerEvent::decode(frame)
                    .map_err(|e| e.to_string())?
                {
                    consumed = self.apply_event(state, &event)?;
                }
            }
            self.drain(state);
            Ok(consumed)
        }
    }
    pub fn poll_updates(&mut self, state: &mut State, now_ms: i64) -> Result<()> {
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        let changed = {
            let updates = &mut self.ambient_updates;
            self.pending_varps
                .poll_with(&mut self.varps, now_ms, |_, values| {
                    updates.push(values.clone())
                })
        };
        #[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
        let changed = self.pending_varps.poll(&mut self.varps, now_ms);
        for id in changed {
            self.refresh_varp(state, id)?;
            state.updated_varps[(state.varp_writer & 31) as usize] = id;
            state.varp_writer = state.varp_writer.wrapping_add(1);
        }
        while let Some(change) = state.delayed.poll(now_ms) {
            match change.kind {
                1 => {
                    self.varcs.insert(change.id, change.value);
                    state.updated_varcs[(state.varc_writer & 31) as usize] = change.id;
                    state.varc_writer = state.varc_writer.wrapping_add(1);
                }
                2 => {
                    if let Some(text) = change.text {
                        self.varcstrs.insert(change.id, text);
                    }
                    state.updated_varcstrs[(state.varcstr_writer & 31) as usize] = change.id;
                    state.varcstr_writer = state.varcstr_writer.wrapping_add(1);
                }
                3 => {
                    state.get_mut(Key::component(change.id))?.text = change.text.unwrap_or_default()
                }
                4 => {
                    let c = state.get_mut(Key::component(change.id))?;
                    if c.model_type != change.value
                        || c.model_frame_cycle != change.extra[0]
                        || c.model_id != change.extra[1]
                    {
                        c.model_type = change.value;
                        c.model_frame_cycle = change.extra[0];
                        c.model_id = change.extra[1];
                        state.model_redraws = state.model_redraws.wrapping_add(1);
                    }
                }
                5 => {
                    let c = state.get_mut(Key::component(change.id))?;
                    if c.model_seq_id != change.value || change.value == -1 {
                        c.seq_next_frame = 1;
                        c.seq_cycle = 0;
                        c.model_seq_id = change.value;
                        c.seq_frame = 0;
                        state.model_redraws = state.model_redraws.wrapping_add(1);
                    }
                }
                8 => {
                    let c = state.get_mut(Key::component(change.id))?;
                    if c.model_x_angle != change.value
                        || c.model_y_angle != change.extra[1]
                        || c.model_zoom != change.extra[0]
                    {
                        c.model_x_angle = change.value;
                        c.model_zoom = change.extra[0];
                        c.model_y_angle = change.extra[1];
                        if c.obj_id != -1 {
                            if c.model_viewport_width > 0 {
                                c.model_zoom = c
                                    .model_zoom
                                    .wrapping_mul(32)
                                    .wrapping_div(c.model_viewport_width);
                            } else if c.base_width > 0 {
                                c.model_zoom =
                                    c.model_zoom.wrapping_mul(32).wrapping_div(c.base_width);
                            }
                        }
                        state.model_redraws = state.model_redraws.wrapping_add(1);
                    }
                }
                9 => {
                    let c = state.get_mut(Key::component(change.id))?;
                    if c.obj_id != change.value || c.obj_count != change.extra[1] {
                        c.obj_id = change.value;
                        c.obj_count = change.extra[1];
                        state.model_redraws = state.model_redraws.wrapping_add(1);
                    }
                }
                10 => {
                    let c = state.get_mut(Key::component(change.id))?;
                    if c.model_x_offset != change.value
                        || c.model_y_offset != change.extra[0]
                        || c.model_z_offset != change.extra[1]
                    {
                        c.model_x_offset = change.value;
                        c.model_y_offset = change.extra[0];
                        c.model_z_offset = change.extra[1];
                        state.model_redraws = state.model_redraws.wrapping_add(1);
                    }
                }
                11 => {
                    let c = state.get_mut(Key::component(change.id))?;
                    c.x = change.value;
                    c.base_x = change.value;
                    c.y_mode = 0;
                    c.x_mode = 0;
                    c.y = change.extra[1];
                    c.base_y = change.extra[1];
                    state.model_redraws = state.model_redraws.wrapping_add(1);
                }
                7 => state.get_mut(Key::component(change.id))?.hidden = change.value == 1,
                6 => {
                    // A 15-bit colour widened to 24 bits, five bits per channel.
                    let v = change.value;
                    let colour = ((v & 31) << 3) + ((v >> 5 & 31) << 11) + ((v >> 10 & 31) << 19);
                    let c = state.get_mut(Key::component(change.id))?;
                    if c.color != colour {
                        c.color = colour;
                        state.model_redraws = state.model_redraws.wrapping_add(1);
                    }
                }
                12 => {
                    // A missing or non-layer component is ignored, not an error.
                    if let Ok(c) = state.get_mut(Key::component(change.id)) {
                        if c.kind == 0 {
                            let mut scroll = change.value;
                            if scroll > c.scroll_max_v - c.height {
                                scroll = c.scroll_max_v - c.height;
                            }
                            if scroll < 0 {
                                scroll = 0;
                            }
                            if scroll != c.scroll_y {
                                c.scroll_y = scroll;
                                state.model_redraws = state.model_redraws.wrapping_add(1);
                            }
                        }
                    }
                }
                13 => {
                    state
                        .get_mut(Key::component(change.id))?
                        .model_rotation_speed = change.value
                }
                _ => return Err(format!("Unsupported delayed UI change {}", change.kind)),
            }
        }
        Ok(())
    }
    fn script(&mut self, state: &State, id: i32) -> Result<Rc<Script>> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.scripts.entry(id) {
            let script = Script::decode(state.pack.get(&format!("script/{id}"))?)
                .map_err(|e| e.to_string())?;
            entry.insert(Rc::new(script));
        }
        Ok(self.scripts[&id].clone())
    }
    fn frame(&mut self, state: &State, id: i32) -> Result<Frame> {
        let script = self.script(state, id)?;
        Ok(Frame {
            id,
            pc: -1,
            ints: vec![0; script.int_locals as usize],
            strings: vec![None; script.string_locals as usize],
            script,
        })
    }
    pub fn drain(&mut self, state: &mut State) {
        self.world_map_requests(state);
        let mut count = 0;
        while let Some(hook) = state.hooks.pop_front() {
            count += 1;
            if count > 10000 {
                state.diagnostic("Too many queued interface hooks".into());
                break;
            }
            // Protocol.loop 2994-3017: a request for a created component
            // that has since been replaced or deleted is discarded.
            if !state.hook_is_current(&hook) {
                continue;
            }
            if let Err(e) = self.run(state, &hook, 2_000_000) {
                state.diagnostic(format!(
                    "CS2 {}:{} opcode {}: {e}",
                    self.last_script, self.last_pc, self.last_op
                ));
            }
        }
    }
    pub fn run(&mut self, state: &mut State, hook: &Hook, max_cycles: usize) -> Result<()> {
        let Some(HookArg::Int(id)) = hook.arguments.first() else {
            return Err("Missing hook script ID".into());
        };
        let mut current = self.frame(state, *id)?;
        let mut ni = 0;
        let mut ns = 0;
        for arg in hook.arguments.iter().skip(1) {
            match arg {
                HookArg::Int(value) => {
                    let value = match *value {
                        n if n == i32::MIN + 3 => hook.source.id,
                        n if n == i32::MIN + 5 => hook.source.child,
                        n if n == i32::MIN + 4 => hook.op,
                        n if n == i32::MIN + 6 => hook.target.map_or(-1, |k| k.id),
                        n if n == i32::MIN + 7 => hook.target.map_or(-1, |k| k.child),
                        n if n == i32::MIN + 1 => hook.mouse[0],
                        n if n == i32::MIN + 2 => hook.mouse[1],
                        n if n == i32::MIN + 8 => hook.key_code,
                        n if n == i32::MIN + 9 => hook.key_char,
                        n => n,
                    };
                    *current
                        .ints
                        .get_mut(ni)
                        .ok_or("Too many hook integer arguments")? = value;
                    ni += 1;
                }
                HookArg::Text(value) => {
                    *current
                        .strings
                        .get_mut(ns)
                        .ok_or("Too many hook string arguments")? =
                        Some(if value == "event_opbase" {
                            hook.op_base.clone()
                        } else {
                            value.clone()
                        });
                    ns += 1;
                }
                HookArg::Null => {}
            }
        }
        let mut calls = Vec::<Frame>::new();
        let mut stack = Stack::default();
        for _ in 0..max_cycles {
            current.pc = current.pc.wrapping_add(1);
            let pc = current.pc as usize;
            let op = *current
                .script
                .opcodes
                .get(pc)
                .ok_or("CS2 program counter outside script")?;
            let operand = current.script.int_operands[pc];
            self.executed += 1;
            self.last_script = current.id;
            self.last_pc = current.pc;
            self.last_op = op;
            match op {
                0 => stack.ints.push(operand),
                1 => stack.ints.push(*self.varps.get(&operand).unwrap_or(&0)),
                2 => {
                    self.varps.insert(operand, stack.pop()?);
                    self.pending_varps
                        .client(operand, i64::from(self.cycle) * 20);
                    #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                    self.ambient_updates.push(self.varps.clone());
                    self.refresh_varp(state, operand)?;
                }
                3 => stack
                    .strings
                    .push(current.script.string_operands[pc].clone()),
                6 => current.pc = current.pc.wrapping_add(operand),
                7 | 8 | 9 | 10 | 31 | 32 => {
                    let [a, b] = stack.take()?;
                    let yes = match op {
                        7 => a != b,
                        8 => a == b,
                        9 => a < b,
                        10 => a > b,
                        31 => a <= b,
                        _ => a >= b,
                    };
                    if yes {
                        current.pc = current.pc.wrapping_add(operand);
                    }
                }
                21 => {
                    if let Some(caller) = calls.pop() {
                        current = caller;
                    } else {
                        return Ok(());
                    }
                }
                25 => stack.ints.push(self.varbit(state, operand)?),
                27 => {
                    let value = stack.pop()?;
                    self.set_varbit(state, operand, value)?;
                }
                33 => stack.ints.push(
                    *current
                        .ints
                        .get(operand as usize)
                        .ok_or("CS2 local integer index")?,
                ),
                34 => {
                    *current
                        .ints
                        .get_mut(operand as usize)
                        .ok_or("CS2 local integer index")? = stack.pop()?;
                }
                35 => stack.strings.push(
                    current
                        .strings
                        .get(operand as usize)
                        .ok_or("CS2 local string index")?
                        .clone(),
                ),
                36 => {
                    *current
                        .strings
                        .get_mut(operand as usize)
                        .ok_or("CS2 local string index")? = stack.string()?;
                }
                37 => {
                    let start = stack
                        .strings
                        .len()
                        .checked_sub(operand as usize)
                        .ok_or("CS2 concat stack underflow")?;
                    let value = stack.strings[start..]
                        .iter()
                        .map(|s| s.as_deref().unwrap_or("null"))
                        .collect::<String>();
                    stack.strings.truncate(start);
                    stack.strings.push(Some(value));
                }
                38 => {
                    stack.pop()?;
                }
                39 => {
                    stack.string()?;
                }
                40 => {
                    if calls.len() >= 50 {
                        return Err("CS2 call depth exceeded".into());
                    }
                    let mut next = self.frame(state, operand)?;
                    let ni = next.script.int_args as usize;
                    let ns = next.script.string_args as usize;
                    let start = stack
                        .ints
                        .len()
                        .checked_sub(ni)
                        .ok_or("CS2 call integer arguments")?;
                    next.ints[..ni].copy_from_slice(&stack.ints[start..]);
                    stack.ints.truncate(start);
                    let start = stack
                        .strings
                        .len()
                        .checked_sub(ns)
                        .ok_or("CS2 call string arguments")?;
                    next.strings[..ns].clone_from_slice(&stack.strings[start..]);
                    stack.strings.truncate(start);
                    calls.push(current);
                    current = next;
                }
                42 => stack.ints.push(self.varc(operand)),
                43 => {
                    self.varcs.insert(operand, stack.pop()?);
                    state.delayed.client(1, operand, i64::from(self.cycle) * 20);
                }
                44 => {
                    let n = stack.pop()?;
                    if !(0..=5000).contains(&n) {
                        return Err("Invalid CS2 array length".into());
                    }
                    self.arrays.insert(
                        operand >> 16,
                        vec![if operand & 65535 == 105 { 0 } else { -1 }; n as usize],
                    );
                }
                45 => {
                    let i = stack.pop()?;
                    stack.ints.push(
                        *self
                            .arrays
                            .get(&operand)
                            .and_then(|a| a.get(i as usize))
                            .ok_or("CS2 array index")?,
                    );
                }
                46 => {
                    let [i, v] = stack.take()?;
                    *self
                        .arrays
                        .get_mut(&operand)
                        .and_then(|a| a.get_mut(i as usize))
                        .ok_or("CS2 array index")? = v;
                }
                47 => stack.strings.push(Some(
                    self.varcstrs
                        .get(&operand)
                        .cloned()
                        .unwrap_or_else(|| "null".into()),
                )),
                48 => {
                    self.varcstrs.insert(operand, stack.text()?);
                    state.delayed.client(2, operand, i64::from(self.cycle) * 20);
                }
                51 => {
                    let key = stack.pop()?;
                    if let Some(offset) = current
                        .script
                        .switch_tables
                        .as_ref()
                        .and_then(|t| t.get(operand as usize))
                        .and_then(|t| t.get(&key))
                    {
                        current.pc = current.pc.wrapping_add(*offset);
                    }
                }
                _ => self.operation(state, op, operand == 1, &mut stack)?,
            }
            if stack.ints.len() > 1000 || stack.strings.len() > 1000 {
                return Err("CS2 stack capacity exceeded".into());
            }
        }
        Err("CS2 instruction budget exceeded".into())
    }
    fn varbit_parts(&self, state: &State, id: i32) -> Result<(i32, i32, i32)> {
        let mut r = Reader::new(state.pack.get(&format!("varbit/{id}"))?);
        let (mut base, mut start, mut end) = (0, 0, 0);
        loop {
            match r.g1().map_err(|e| e.to_string())? {
                0 => break,
                1 => {
                    base = r.g2().map_err(|e| e.to_string())?;
                    start = r.g1().map_err(|e| e.to_string())?;
                    end = r.g1().map_err(|e| e.to_string())?;
                }
                _ => {}
            }
        }
        if end < start || end - start > 31 {
            return Err("Invalid varbit range".into());
        }
        let mask = if end - start == 31 {
            -1
        } else {
            (1i32 << (end - start + 1)).wrapping_sub(1)
        };
        Ok((base, start, mask))
    }
    pub(super) fn varp_client_code(state: &State, id: i32) -> Result<i32> {
        let Ok(raw) = state.pack.get(&format!("varptype/{id}")) else {
            return Ok(0);
        };
        let mut r = Reader::new(raw);
        let mut code = 0;
        loop {
            match r.g1().map_err(|e| e.to_string())? {
                0 => break,
                5 => code = r.g2().map_err(|e| e.to_string())?,
                _ => {}
            }
        }
        Ok(code)
    }
    fn refresh_varp(&mut self, state: &State, id: i32) -> Result<()> {
        let code = Self::varp_client_code(state, id)?;
        let value = self.varps.get(&id).copied().unwrap_or(0);
        match code {
            5 => self.mouse_buttons = value,
            6 => self.chat_effects_disabled = value,
            9 => self.inserting = value,
            _ => {}
        }
        Ok(())
    }
    pub fn varbit(&self, state: &State, id: i32) -> Result<i32> {
        self.varbit_from(state, id, &self.varps)
    }
    pub fn varbit_from(&self, state: &State, id: i32, values: &BTreeMap<i32, i32>) -> Result<i32> {
        let (base, start, mask) = self.varbit_parts(state, id)?;
        Ok(values
            .get(&base)
            .copied()
            .unwrap_or(0)
            .wrapping_shr(start as u32)
            & mask)
    }
    pub fn set_varbit(&mut self, state: &State, id: i32, mut value: i32) -> Result<()> {
        let (base, start, mask) = self.varbit_parts(state, id)?;
        if value < 0 || value > mask {
            value = 0;
        }
        let m = mask.wrapping_shl(start as u32);
        let old = self.varps.get(&base).copied().unwrap_or(0);
        self.varps
            .insert(base, (old & !m) | (value.wrapping_shl(start as u32) & m));
        self.pending_varps.client(base, i64::from(self.cycle) * 20);
        Ok(())
    }
    /// `Player.setTutorialIsland`, evaluated from the current absolute tile.
    fn in_tutorial_island(&self) -> bool {
        let [x, y, _] = self.location;
        let island = ((3053..=3156).contains(&x) && (3056..=3136).contains(&y))
            || ((3072..=3118).contains(&x) && (9492..=9535).contains(&y));
        island && !((3139..=3199).contains(&x) && (3008..=3062).contains(&y))
    }
    /// `name` is passed separately so the result does not borrow the VM.
    fn local<'a>(&self, name: &'a str) -> super::social::Local<'a> {
        super::social::Local {
            key: super::social::encode(name),
            name,
            world: self.world,
            members: self.members,
            tutorial: self.in_tutorial_island(),
            restricted: self.chat_restricted || self.quick_chat,
        }
    }
    /// Server messages the script VM owns: social lists, offers and the
    /// interface changes that go through DelayedStateChange.
    pub fn apply_event(
        &mut self,
        state: &mut State,
        event: &rs09_protocol::game::events::ServerEvent,
    ) -> Result<bool> {
        use rs09_protocol::game::events::ServerEvent;
        let name = self.player_name.clone();
        let local = self.local(&name);
        // Quick chat needs the phrase definition to produce its text.
        let quick = match event {
            ServerEvent::PrivateQuickChat { phrase, body, .. }
            | ServerEvent::PrivateQuickChatEcho { phrase, body, .. }
            | ServerEvent::ClanQuickChat { phrase, body, .. } => Some((*phrase, body)),
            _ => None,
        };
        if let Some((phrase, body)) = quick {
            if self.social.quick_chat_wanted(event, local) {
                let text = super::quickchat::message(state, phrase, body)?;
                if let Some(line) = self.social.quick_chat(event, text) {
                    self.add_message(state, line);
                }
            }
            return Ok(true);
        }
        let applied = self.social.apply(event, local);
        if let Some((lines, transmit)) = applied {
            for line in lines {
                self.add_message(state, line);
            }
            if transmit.friends {
                state.friend_transmit_at = state.transmit_timer;
            }
            if transmit.clan {
                state.clan_transmit_at = state.transmit_timer;
            }
            if transmit.stock {
                state.stock_transmit_at = state.transmit_timer;
            }
            return Ok(true);
        }
        match event {
            ServerEvent::MidiSong { id } => self.music.push(MusicRequest::Song(*id)),
            ServerEvent::MidiJingle { id, delay } => self.music.push(MusicRequest::Jingle {
                id: *id,
                delay: *delay,
            }),
            ServerEvent::WalkText(text) => {
                self.walk_text = text.as_deref().map(latin1);
            }
            ServerEvent::SettingsString(text) => self.settings = Some(latin1(text)),
            ServerEvent::OpenUrl(url) => {
                // Protocol.java:1830-1832.
                self.display.leave_full_screen(&mut self.preferences);
                self.window_mode = self.display.window_mode();
                if self.requested_urls.len() < 16 {
                    self.requested_urls.push(latin1(url));
                }
            }
            ServerEvent::LastLogin { address } => {
                // The reverse-DNS lookup is a caller service; none runs here.
                state.last_login_address = Some((*address, None));
            }
            ServerEvent::RebootTimer { cycles } => {
                self.reboot_timer = *cycles;
                state.misc_transmit_at = state.transmit_timer;
            }
            ServerEvent::ResetVarCache => self.reset_varp_cache(state)?,
            ServerEvent::RefreshVarps => self.force_varp_refresh(state)?,
            _ => return state.apply_event(event),
        }
        Ok(true)
    }
    fn social_operation(&mut self, state: &mut State, op: i32, s: &mut Stack) -> Result<()> {
        use super::social::{self, title_case};
        let name = self.player_name.clone();
        let me = self.local(&name);
        match op {
            3600 => s.ints.push(match self.social.friend_state {
                0 => -2,
                1 => -1,
                _ => self.social.friends.len() as i32,
            }),
            3601 | 3610 => {
                let friend = self.social.friend(s.pop()?);
                s.strings.push(Some(friend.map_or_else(String::new, |f| {
                    if op == 3601 {
                        f.name.clone()
                    } else {
                        f.world_name.clone()
                    }
                })));
            }
            3602 | 3603 | 3627 => {
                let friend = self.social.friend(s.pop()?);
                s.ints.push(friend.map_or(0, |f| match op {
                    3602 => f.world,
                    3603 => f.rank,
                    _ => i32::from(f.same_game),
                }));
            }
            3604 => {
                let rank = s.pop()?;
                let name = s.text()?;
                self.outgoing.push(social::set_rank(&name, rank));
            }
            3605..=3608 => {
                let key = social::encode(&s.text()?);
                let (line, frame) = match op {
                    3605 => self.social.add_friend(key, me),
                    3606 => (None, self.social.remove_friend(key)),
                    3607 => self.social.add_ignore(key, me),
                    _ => (None, self.social.remove_ignore(key)),
                };
                if let Some(line) = line {
                    self.add_message(state, line);
                }
                if let Some(frame) = frame {
                    state.friend_transmit_at = state.transmit_timer;
                    self.outgoing.push(frame);
                }
            }
            3609 => {
                let name = s.text()?;
                s.ints.push(i32::from(self.social.is_friend(&name, me)));
            }
            3611 => s.strings.push(Some(
                self.social
                    .clan_name
                    .as_deref()
                    .map_or_else(String::new, title_case),
            )),
            3612 => s.ints.push(if self.social.clan_name.is_some() {
                self.social.clan_members.len() as i32
            } else {
                0
            }),
            3613 | 3626 => {
                let member = self.social.clan_member(s.pop()?);
                s.strings.push(Some(member.map_or_else(String::new, |m| {
                    if op == 3613 {
                        title_case(&m.name)
                    } else {
                        m.world_name.clone()
                    }
                })));
            }
            3614 | 3615 => {
                let member = self.social.clan_member(s.pop()?);
                s.ints
                    .push(member.map_or(0, |m| if op == 3614 { m.world } else { m.rank }));
            }
            3616 => s.ints.push(self.social.clan_min_kick),
            3617 => {
                let name = s.text()?;
                if let Some(frame) = self.social.kick(&name) {
                    self.outgoing.push(frame);
                }
            }
            3618 => s.ints.push(self.social.clan_rank),
            3619 => {
                let name = s.text()?;
                if let Some(frame) = social::join_clan(&name) {
                    self.outgoing.push(frame);
                }
            }
            3620 => self.outgoing.push(social::leave_clan()),
            3621 => s.ints.push(if self.social.friend_state == 0 {
                -1
            } else {
                self.social.ignores.len() as i32
            }),
            3622 => {
                let index = s.pop()?;
                let name = (self.social.friend_state != 0)
                    .then(|| usize::try_from(index).ok())
                    .flatten()
                    .and_then(|i| self.social.ignores.get(i))
                    .map_or_else(String::new, |i| title_case(&i.name));
                s.strings.push(Some(name));
            }
            3623 => {
                let name = s.text()?;
                s.ints.push(i32::from(self.social.is_ignored(&name)));
            }
            3624 => {
                let index = s.pop()?;
                s.ints
                    .push(i32::from(self.social.clan_member_is(index, me.name)));
            }
            3625 => s.strings.push(Some(
                self.social
                    .clan_owner
                    .as_deref()
                    .map_or_else(String::new, title_case),
            )),
            3628 => {
                let name = s.text()?;
                s.ints.push(self.social.friend_index(&name));
            }
            // client.country is a launch parameter; the pinned launcher sends 0.
            3629 => s.ints.push(0),
            3903..=3908 | 3910..=3913 => {
                let slot = s.pop()?;
                let offer = usize::try_from(slot)
                    .ok()
                    .and_then(|slot| self.social.offers.get(slot))
                    .ok_or_else(|| format!("Invalid Grand Exchange slot {slot}"))?;
                let status = offer.status & 7;
                s.ints.push(match op {
                    3903 => i32::from(offer.status & 8 == 8),
                    3904 => offer.item,
                    3905 => offer.price,
                    3906 => offer.count,
                    3907 => offer.completed_count,
                    3908 => offer.completed_gold,
                    3910 => i32::from(status == 0),
                    3911 => i32::from(status == 2),
                    3912 => i32::from(status == 5),
                    _ => i32::from(status == 1),
                });
            }
            _ => return Err(format!("Unsupported CS2 opcode {op}")),
        }
        Ok(())
    }
    fn operation(&mut self, state: &mut State, op: i32, first: bool, s: &mut Stack) -> Result<()> {
        let active = usize::from(!first);
        match op {
            3109 => {
                let offset = s.take::<2>()?;
                let handle = self.active[active].clone().ok_or("No active widget")?;
                self.drag_requests.push((handle.key(), offset));
                return Ok(());
            }
            100 => {
                let [parent, kind, index] = s.take()?;
                if kind == 0 || !(0..10000).contains(&index) {
                    return Err("Invalid created widget".into());
                }
                state.load_group((parent as u32 >> 16) as i32)?;
                if index > 0
                    && !state.components.contains_key(&Key {
                        id: parent,
                        child: index - 1,
                    })
                {
                    return Err("Gap before created widget".into());
                }
                let key = Key {
                    id: parent,
                    child: index,
                };
                state.replace_component(
                    key,
                    Component {
                        if3: true,
                        created_component_id: index,
                        overlayer: parent,
                        id: parent,
                        kind,
                        ..Component::default()
                    },
                );
                let length = state.created_lengths.entry(parent).or_default();
                *length = (*length).max(index as usize + 1);
                self.active[active] = Some(state.retain_component(key)?);
                return Ok(());
            }
            101 => {
                let key = self.active[active]
                    .as_ref()
                    .ok_or("No active widget")?
                    .key();
                if key.child < 0 {
                    return Err("Deleting static widget".into());
                }
                state.remove_component(key);
                return Ok(());
            }
            102 => {
                let id = s.pop()?;
                state.created_lengths.remove(&id);
                let keys: Vec<_> = state
                    .components
                    .keys()
                    .copied()
                    .filter(|key| key.id == id && key.child != -1)
                    .collect();
                for key in keys {
                    state.remove_component(key);
                }
                return Ok(());
            }
            200 => {
                let [id, child] = s.take()?;
                state.load_group((id as u32 >> 16) as i32)?;
                let key = Key { id, child };
                let found = child != -1 && state.components.contains_key(&key);
                s.ints.push(i32::from(found));
                if found {
                    self.active[active] = Some(state.retain_component(key)?);
                }
                return Ok(());
            }
            201 => {
                let key = Key::component(s.pop()?);
                state.load_group((key.id as u32 >> 16) as i32)?;
                let found = state.components.contains_key(&key);
                s.ints.push(i32::from(found));
                if found {
                    self.active[active] = Some(state.retain_component(key)?);
                }
                return Ok(());
            }
            _ => {}
        }
        if (1000..1900).contains(&op) || (2000..2900).contains(&op) {
            let explicit = op >= 2000;
            let code = if explicit { op - 1000 } else { op };
            if matches!(op, 2702..=2705) {
                let pointer = s.pop()?;
                match op {
                    2702 => s
                        .ints
                        .push(i32::from(state.subinterfaces.contains_key(&pointer))),
                    2703 => {
                        let length = state.created_lengths.get(&pointer).copied().unwrap_or(0);
                        let n = (0..length)
                            .find(|&i| {
                                !state.components.contains_key(&Key {
                                    id: pointer,
                                    child: i as i32,
                                })
                            })
                            .unwrap_or(length) as i32;
                        s.ints.push(n);
                    }
                    2704 | 2705 => {
                        let group = pointer;
                        let pointer = s.pop()?;
                        s.ints.push(i32::from(
                            state
                                .subinterfaces
                                .get(&pointer)
                                .is_some_and(|&(g, _)| g == group),
                        ));
                    }
                    _ => {}
                }
                return Ok(());
            }
            let handle = if explicit {
                let key = Key::component(s.pop()?);
                state.load_group((key.id as u32 >> 16) as i32)?;
                state.retain_component(key)?
            } else {
                self.active[active].clone().ok_or("No active widget")?
            };
            let key = handle.key();
            if (1400..1500).contains(&code) {
                let mut signature = s.text()?;
                let triggers = if signature.ends_with('Y') {
                    signature.pop();
                    let count = s.pop()?;
                    if count > 0 {
                        let start = s
                            .ints
                            .len()
                            .checked_sub(count as usize)
                            .ok_or("Hook trigger stack")?;
                        let v = s.ints[start..].to_vec();
                        s.ints.truncate(start);
                        Some(v)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let mut args = vec![HookArg::Null; signature.len() + 1];
                for (i, c) in signature.bytes().enumerate().rev() {
                    args[i + 1] = if c == b's' {
                        HookArg::Text(s.text()?)
                    } else {
                        HookArg::Int(s.pop()?)
                    };
                }
                let id = s.pop()?;
                args[0] = HookArg::Int(id);
                let args = (id != -1).then_some(args);
                let c = state.component_ref_mut(&handle);
                match code {
                    1400 => c.on_click_repeat = args,
                    1401 => c.on_hold = args,
                    1402 => c.on_release = args,
                    1403 => c.on_mouse_over = args,
                    1404 => c.on_mouse_leave = args,
                    1405 => c.on_drag_start = args,
                    1406 => c.on_use_with = args,
                    1407 => {
                        c.on_varp_transmit = args;
                        c.varp_triggers = triggers
                    }
                    1408 => c.on_timer = args,
                    1409 => c.on_option_click = args,
                    1410 => c.on_drag_release = args,
                    1411 => c.on_drag = args,
                    1412 => c.on_mouse_repeat = args,
                    1414 => {
                        c.on_inv_transmit = args;
                        c.inventory_triggers = triggers
                    }
                    1415 => {
                        c.on_stat_transmit = args;
                        c.stat_triggers = triggers
                    }
                    1416 => c.on_use = args,
                    1417 => c.on_scroll = args,
                    1418 => c.on_msg = args,
                    1419 => c.on_key = args,
                    1420 => c.on_friend_transmit = args,
                    1421 => c.on_clan_transmit = args,
                    1422 => c.on_misc_transmit = args,
                    1423 => c.on_dialog_abort = args,
                    1424 => c.on_widgets_open_close = args,
                    1425 => c.on_stock_transmit = args,
                    1426 => c.on_minimap_unlock = args,
                    1427 => c.on_resize = args,
                    1428 => {
                        c.on_varc_transmit = args;
                        c.varc_triggers = triggers
                    }
                    1429 => {
                        c.on_varcstr_transmit = args;
                        c.varcstr_triggers = triggers
                    }
                    _ => return Err(format!("Unsupported hook opcode {op}")),
                }
                c.has_event_handlers = true;
                return Ok(());
            }
            let item = if matches!(code, 1200 | 1205) {
                let id = *s
                    .ints
                    .get(s.ints.len().checked_sub(2).ok_or("Item argument stack")?)
                    .ok_or("Item argument stack")?;
                if id != -1 {
                    Some(state.items.get(&state.pack, id)?)
                } else {
                    None
                }
            } else {
                None
            };
            let properties = state.reference_properties(&handle);
            if code == 1301 {
                let [id, child] = s.take()?;
                let parent = Key { id, child };
                if state.components.contains_key(&parent) {
                    state.drag_parents.insert(key, parent);
                } else {
                    state.drag_parents.remove(&key);
                }
                return Ok(());
            }
            let c = state.component_ref_mut(&handle);
            match code {
                1000 => {
                    let [x, y, x_mode, y_mode] = s.take()?;
                    c.base_x = x;
                    c.base_y = y;
                    c.y_mode = x_mode.clamp(0, 5);
                    c.x_mode = y_mode.clamp(0, 5);
                }
                1001 => {
                    let [w, h, wm, hm] = s.take()?;
                    c.base_width = w;
                    c.base_height = h;
                    c.model_viewport_width = 0;
                    c.model_viewport_height = 0;
                    c.dynamic_width_value = wm.clamp(0, 4);
                    c.dynamic_height_value = hm.clamp(0, 4);
                }
                1003 => c.hidden = s.pop()? == 1,
                1004 => {
                    let [w, h] = s.take()?;
                    c.aspect_width = w;
                    c.aspect_height = h;
                }
                1005 => c.no_click_through = s.pop()? == 1,
                1100 => {
                    let [x, y] = s.take()?;
                    c.scroll_x = x.min(c.scroll_max_h - c.width).max(0);
                    c.scroll_y = y.min(c.scroll_max_v - c.height).max(0);
                }
                1101 => c.color = s.pop()?,
                1102 => c.filled = s.pop()? == 1,
                1103 => c.alpha = s.pop()?,
                1104 => c.line_width = s.pop()?,
                1105 => c.sprite_id = s.pop()?,
                1106 => c.angle2d = s.pop()?,
                1107 => c.sprite_tiling = s.pop()? == 1,
                1108 => {
                    c.model_type = 1;
                    c.model_id = s.pop()?;
                }
                1109 => {
                    let [a, b, d, e, f, g] = s.take()?;
                    c.model_x_offset = a;
                    c.model_z_offset = b;
                    c.model_x_angle = d;
                    c.model_y_angle = e;
                    c.model_y_offset = f;
                    c.model_zoom = g;
                }
                1110 => {
                    let id = s.pop()?;
                    if c.model_seq_id != id {
                        c.model_seq_id = id;
                        c.seq_frame = 0;
                        c.seq_cycle = 0;
                        c.seq_next_frame = 1;
                    }
                }
                1111 => c.model_ortho = s.pop()? == 1,
                1112 => c.text = s.text()?,
                1113 => c.font = s.pop()?,
                1114 => {
                    let [a, b, d] = s.take()?;
                    c.halign = a;
                    c.valign = b;
                    c.vpadding = d;
                }
                1115 => c.shadowed = s.pop()? == 1,
                1116 => c.outline_thickness = s.pop()?,
                1117 => c.shadow_color = s.pop()?,
                1118 => c.v_flip = s.pop()? == 1,
                1119 => c.h_flip = s.pop()? == 1,
                1120 => {
                    let [w, h] = s.take()?;
                    c.scroll_max_h = w;
                    c.scroll_max_v = h;
                }
                1121 => {
                    let [a, b] = s.take()?;
                    c.model_view_angle = a as i16 as i32;
                    c.model_near_clip = b as i16 as i32;
                }
                1122 => c.has_alpha = s.pop()? == 1,
                1123 => c.model_zoom = s.pop()?,
                1200 | 1205 => {
                    let [id, count] = s.take()?;
                    c.obj_id = id;
                    if id == -1 {
                        c.model_type = 1;
                        c.model_id = -1;
                    } else {
                        c.obj_count = count;
                        c.obj_draw_text = code != 1205;
                        if let Some(d) = &item {
                            c.model_y_offset = d.zAngle2D;
                            c.model_x_offset = d.xOffset2D;
                            c.model_x_angle = d.xAngle2D;
                            c.model_z_offset = d.yOffset2D;
                            c.model_y_angle = d.yAngle2D;
                            c.model_zoom = d.zoom2d;
                            if c.model_viewport_width > 0 {
                                c.model_zoom =
                                    c.model_zoom.wrapping_mul(32) / c.model_viewport_width;
                            } else if c.base_width > 0 {
                                c.model_zoom = c.model_zoom.wrapping_mul(32) / c.base_width;
                            }
                        }
                    }
                }
                1201 => {
                    c.model_type = 2;
                    c.model_id = s.pop()?;
                }
                1202 => {
                    c.model_type = 3;
                    c.model_id = self.player_head_id;
                }
                1203 => {
                    c.model_type = 6;
                    c.model_id = s.pop()?;
                }
                1204 => {
                    c.model_type = 5;
                    c.model_id = s.pop()?;
                }
                1300 => {
                    let i = s.pop()? - 1;
                    let text = s.text()?;
                    if (0..10).contains(&i) {
                        let ops = c.ops.get_or_insert_with(Vec::new);
                        if ops.len() <= i as usize {
                            ops.resize(i as usize + 1, None);
                        }
                        ops[i as usize] = Some(text);
                    }
                }
                1302 => c.drag_render_behavior = s.pop()? == 1,
                1303 => c.drag_deadzone = s.pop()?,
                1304 => c.drag_deadtime = s.pop()?,
                1305 => c.option_base = s.text()?,
                1306 => c.option_circumfix = s.text()?,
                1307 => c.ops = None,
                1308 => {
                    c.default_target_cursor = s.pop()?;
                    c.target_cursor = s.pop()?;
                }
                1309 => {
                    let [index, target] = s.take()?;
                    if (1..=10).contains(&index) {
                        let targets = c
                            .drag_targets
                            .get_or_insert_with(|| vec![0; index as usize]);
                        targets.resize(targets.len().max(index as usize), -1);
                        targets[index as usize - 1] = target;
                    }
                }
                1500 => s.ints.push(c.x),
                1501 => s.ints.push(c.y),
                1502 => s.ints.push(c.width),
                1503 => s.ints.push(c.height),
                1504 => s.ints.push(i32::from(c.hidden)),
                1505 => s.ints.push(c.overlayer),
                1600 => s.ints.push(c.scroll_x),
                1601 => s.ints.push(c.scroll_y),
                1602 => s.strings.push(Some(c.text.clone())),
                1603 => s.ints.push(c.scroll_max_h),
                1604 => s.ints.push(c.scroll_max_v),
                1605 => s.ints.push(c.model_zoom),
                1606 => s.ints.push(c.model_x_angle),
                1607 => s.ints.push(c.model_y_offset),
                1608 => s.ints.push(c.model_y_angle),
                1609 => s.ints.push(c.alpha),
                1610 => s.ints.push(c.model_x_offset),
                1611 => s.ints.push(c.model_z_offset),
                1612 => s.ints.push(c.sprite_id),
                1700 => s.ints.push(c.obj_id),
                1701 => s.ints.push(if c.obj_id == -1 { 0 } else { c.obj_count }),
                1702 => s.ints.push(c.created_component_id),
                1800 => s.ints.push(properties.target_mask()),
                1801 => {
                    let i = s.pop()? - 1;
                    s.strings.push(Some(
                        c.ops
                            .as_ref()
                            .and_then(|ops| ops.get(i as usize))
                            .and_then(|v| v.clone())
                            .unwrap_or_default(),
                    ));
                }
                1802 => s.strings.push(Some(c.option_base.clone())),
                _ => return Err(format!("Unsupported component opcode {op}")),
            }
            if matches!(code, 1000 | 1001 | 1004) {
                state.refresh_reference(&handle)?;
            }
            if key.child == -1 {
                for kind in match code {
                    1000 => &[11][..],
                    1003 => &[7],
                    1100 => &[12],
                    1101 => &[6],
                    1108 => &[4],
                    1109 => &[8, 10],
                    1110 => &[5],
                    1112 => &[3],
                    1123 => &[8],
                    1200 | 1205 => &[9, 8, 10],
                    1201..=1204 => &[4],
                    _ => &[],
                } {
                    state
                        .delayed
                        .client(*kind, key.id, i64::from(self.cycle) * 20);
                }
            }
            return Ok(());
        }
        match op {
            3100 => {
                let text = s.text()?;
                self.add_message(
                    state,
                    ChatLine {
                        text,
                        phrase: -1,
                        ..ChatLine::default()
                    },
                );
            }
            3103 => {
                self.close_modal(state);
            }
            3104..=3106 => {
                let text = s.text()?;
                self.outgoing.push(super::chat::dialog(op, &text)?);
            }
            3200 => {
                let [id, loops, delay] = s.take()?;
                if self.sound_volume.unwrap_or(127) != 0
                    && loops != 0
                    && id != -1
                    && self.effects.len() < 50
                {
                    self.effects.push([id, loops, delay]);
                }
            }
            3300 => s.ints.push(self.cycle),
            3305..=3307 => {
                let index = s.pop()? as usize;
                let values = match op {
                    3305 => &self.boosted_levels,
                    3306 => &self.base_levels,
                    _ => &self.experience,
                };
                s.ints
                    .push(*values.get(index).ok_or("Invalid skill index")?);
            }
            3301 | 3302 => {
                let [inv, slot] = s.take()?;
                let item = state
                    .containers
                    .get(&inv)
                    .and_then(|v| v.get(slot as usize));
                s.ints.push(if op == 3301 {
                    item.map_or(-1, |i| i.id)
                } else {
                    item.map_or(0, |i| i.amount)
                });
            }
            3303 => {
                let [inv, id] = s.take()?;
                s.ints.push(if id == -1 {
                    0
                } else {
                    state.containers.get(&inv).map_or(0, |v| {
                        v.iter()
                            .filter(|i| i.id == id)
                            .fold(0i32, |n, i| n.wrapping_add(i.amount))
                    })
                });
            }
            3304 => {
                let id = s.pop()?;
                let mut r = Reader::new(state.pack.get(&format!("invtype/{id}"))?);
                let mut size = 0;
                loop {
                    match r.g1().map_err(|e| e.to_string())? {
                        0 => break,
                        2 => size = r.g2().map_err(|e| e.to_string())?,
                        _ => {}
                    }
                }
                s.ints.push(size);
            }
            3308 => s.ints.push(
                self.location[2]
                    .wrapping_shl(28)
                    .wrapping_add(self.location[0].wrapping_shl(14))
                    .wrapping_add(self.location[1]),
            ),
            3309 => {
                let p = s.pop()?;
                s.ints.push(p >> 14 & 16383);
            }
            3310 => {
                let p = s.pop()?;
                s.ints.push(p >> 28);
            }
            3311 => {
                let p = s.pop()?;
                s.ints.push(p & 16383);
            }
            3312 => s.ints.push(i32::from(self.members)),
            3321 => s.ints.push(self.run_energy),
            3322 => s.ints.push(self.weight),
            3328 => s.ints.push(i32::from(self.chat_restricted)),
            3329 => s.ints.push(i32::from(self.quick_chat)),
            3325 => s.ints.push(i32::from(self.site_members)),
            3326 => s.ints.push(self.combat_level),
            3316 => s.ints.push(if self.rights >= 2 { self.rights } else { 0 }),
            3318 => s.ints.push(self.world),
            3317 => s.ints.push(self.reboot_timer),
            3600..=3629 | 3903..=3913 => self.social_operation(state, op, s)?,
            5015 => s.strings.push(Some(self.player_name.clone())),
            5017 => s.ints.push(self.chat_lines),
            5008 => {
                let text = s.text()?;
                if let Some(command) = text.strip_prefix("::") {
                    // Cheat.execute runs before the packet; a command that
                    // throws ends the script without sending it.
                    self.cheat
                        .run(&text, self.window_mode != 0, &mut self.preferences)
                        .map_err(|failure| failure.to_string())?;
                    self.outgoing.push(super::chat::command(command)?);
                    return Ok(());
                }
                let codec = rs09_protocol::huffman::Huffman::new(state.pack.get("huffman")?)
                    .map_err(|e| e.to_string())?;
                if let Some(frame) = super::chat::public(
                    &text,
                    &codec,
                    self.rights,
                    self.chat_restricted,
                    self.quick_chat,
                )? {
                    self.outgoing.push(frame);
                }
            }
            5000 => s.ints.push(self.public_chat),
            5001 => {
                let filters: [i32; 3] = s.take()?;
                [self.public_chat, self.private_chat, self.trade_chat] = filters;
                self.outgoing.push(rs09_protocol::game::Frame {
                    opcode: 157,
                    payload: filters.map(|value| value as u8).to_vec(),
                });
            }
            5005 => s.ints.push(self.private_chat),
            5016 => s.ints.push(self.trade_chat),
            5003 | 5004 | 5010 | 5011 | 5012 => {
                let index = s.pop()?;
                if index < 0 {
                    return Err("Negative chat line index".into());
                }
                let line = (index < 100)
                    .then(|| self.messages.get(index as usize))
                    .flatten();
                match op {
                    5004 => s.ints.push(line.map_or(-1, |l| l.kind)),
                    // Chat.phraseIds is a plain int[100]: a slot no message
                    // has reached reads 0, only an index past it gives -1.
                    5012 => s
                        .ints
                        .push(super::small_text::phrase_id(index, line.map(|l| l.phrase))),
                    _ => s
                        .strings
                        .push(Some(line.map_or_else(String::new, |l| match op {
                            5003 => l.text.clone(),
                            5010 => l.name.clone(),
                            _ => l.clan.clone(),
                        }))),
                }
            }
            4107 => {
                let b = s.text()?;
                let a = s.text()?;
                s.ints.push(super::text::compare(&a, &b));
            }
            4113..=4116 | 4122 | 4123 => {
                let c = s.pop()?;
                s.ints.push(super::text::character(op, c));
            }
            4108 | 4109 => {
                let [width, font] = s.take()?;
                let text = s.text()?;
                let mut metrics =
                    super::font::Metrics::decode(state.pack.get(&format!("font/{font}"))?)
                        .map_err(|e| e.to_string())?;
                if let Ok(raw) = state.pack.get("sprite/name_icons") {
                    let icons = super::sprite::Sheet::decode(raw).map_err(|e| e.to_string())?;
                    metrics.name_icons = vec![icons.size[0] as i32; icons.frames.len()];
                }
                let lines = metrics.split(&text, Some(&[width]));
                s.ints.push(if op == 4108 {
                    lines.len() as i32
                } else {
                    lines.iter().map(|l| metrics.width(l)).max().unwrap_or(0)
                });
            }
            3400 | 3408 => {
                let (kt, vt, id, key) = if op == 3408 {
                    let [a, b, c, d] = s.take()?;
                    (a, b, c, d)
                } else {
                    let [a, b] = s.take()?;
                    (0, 115, a, b)
                };
                let value = enum_value(state, id, key, kt, vt, op == 3408)?;
                match value {
                    HookArg::Int(v) => s.ints.push(v),
                    HookArg::Text(v) => s.strings.push(Some(v)),
                    HookArg::Null => return Err("Null enum result".into()),
                }
            }
            4000 | 4001 | 4002 | 4003 | 4007..=4011 | 4014..=4017 => {
                let [a, b] = s.take()?;
                let value = match op {
                    4000 => a.wrapping_add(b),
                    4001 => a.wrapping_sub(b),
                    4002 => a.wrapping_mul(b),
                    4003 => {
                        if b == 0 {
                            return Err("CS2 division by zero".into());
                        }
                        a.wrapping_div(b)
                    }
                    4007 => a.wrapping_add(a.wrapping_mul(b) / 100),
                    4008 => a | 1i32.wrapping_shl(b as u32),
                    4009 => a & !1i32.wrapping_shl(b as u32),
                    4010 => i32::from(a & 1i32.wrapping_shl(b as u32) != 0),
                    4011 => {
                        if b == 0 {
                            return Err("CS2 modulo zero".into());
                        }
                        a.wrapping_rem(b)
                    }
                    4014 => a & b,
                    4015 => a | b,
                    4016 => a.min(b),
                    _ => a.max(b),
                };
                s.ints.push(value);
            }
            4100 => {
                let b = s.pop()?;
                let a = s.text()?;
                s.strings.push(Some(format!("{a}{b}")));
            }
            4101 => {
                let b = s.text()?;
                let a = s.text()?;
                s.strings.push(Some(format!("{a}{b}")));
            }
            4106 => {
                let n = s.pop()?;
                s.strings.push(Some(n.to_string()));
            }
            4117 => {
                let text = s.string()?;
                s.ints
                    .push(text.map_or(0, |s| s.encode_utf16().count() as i32));
            }
            4112 => {
                let character = s.pop()?;
                let text = s.text()?;
                s.strings
                    .push(Some(super::text::concat_character(&text, character)?));
            }
            4118 => {
                let [from, to] = s.take()?;
                let text = s.text()?;
                s.strings
                    .push(Some(super::text::substring(&text, from, to)?));
            }
            4111 => {
                let text = s.text()?;
                let escaped = text
                    .chars()
                    .map(|c| match c {
                        '<' => "<lt>".to_string(),
                        '>' => "<gt>".to_string(),
                        c => c.to_string(),
                    })
                    .collect::<String>();
                s.strings.push(Some(escaped));
            }
            4103 => {
                let text = s.text()?;
                s.strings.push(Some(
                    text.chars()
                        .map(|c| {
                            let code = c as u32;
                            if (65..=90).contains(&code)
                                || ((192..=222).contains(&code) && code != 215)
                            {
                                char::from_u32(code + 32).unwrap()
                            } else {
                                c
                            }
                        })
                        .collect(),
                ));
            }
            4121 => {
                let from = s.pop()?;
                let needle = s.text()?.chars().collect::<Vec<_>>();
                let text = s.text()?.chars().collect::<Vec<_>>();
                let start = from.max(0) as usize;
                let result = if needle.is_empty() {
                    start.min(text.len()) as i32
                } else if needle.len() > text.len() {
                    -1
                } else {
                    (start..=text.len() - needle.len())
                        .find(|&i| text[i..i + needle.len()] == needle)
                        .map_or(-1, |i| i as i32)
                };
                s.ints.push(result);
            }
            4200 => {
                let id = s.pop()?;
                s.strings.push(Some(state.items.get(&state.pack, id)?.name));
            }
            4201 | 4202 => {
                let [id, option] = s.take()?;
                let item = state.items.get(&state.pack, id)?;
                let options = if op == 4201 { item.ops } else { item.iops };
                s.strings.push(Some(if (1..=5).contains(&option) {
                    options[option as usize - 1].clone().unwrap_or_default()
                } else {
                    String::new()
                }));
            }
            4203..=4207 => {
                let id = s.pop()?;
                let item = state.items.get(&state.pack, id)?;
                s.ints.push(match op {
                    4203 => item.cost,
                    4204 => i32::from(item.stackable == 1),
                    4205 if item.certtemplate == -1 && item.certlink >= 0 => item.certlink,
                    4206 if item.certtemplate >= 0 && item.certlink >= 0 => item.certlink,
                    4207 => i32::from(item.members),
                    _ => id,
                });
            }
            5422 => {
                let index = s.pop()?;
                let suffix = s.string()?;
                let prefix = s.string()?;
                self.player_options.titles(0, index, prefix, suffix)?;
            }
            5306 => s.ints.push(self.window_mode),
            6203 => {
                let size = state
                    .components
                    .values()
                    .find(|c| c.client_code == 1337 && c.id >> 16 == state.root)
                    .map_or(state.viewport, |c| [c.width, c.height]);
                s.ints.extend(size.map(|n| n.max(1) as i16 as i32));
            }
            6506 if self.login_operation(state, op, s)? => {}
            6506 => {
                s.pop()?;
                s.ints.extend([-1, 0, 0]);
                s.strings.extend([Some(String::new()), Some(String::new())]);
            }
            _ => return self.extended_operation(state, op, s),
        }
        Ok(())
    }
}
pub(super) fn enum_value(
    state: &State,
    id: i32,
    key: i32,
    expected_key: i32,
    expected_value: i32,
    check: bool,
) -> Result<HookArg> {
    let mut r = Reader::new(state.pack.get(&format!("enum/{id}"))?);
    let (mut kt, mut vt) = (0, 0);
    let mut default_int = 0;
    let mut default_text = "null".to_string();
    let mut found = None;
    loop {
        match r.g1().map_err(|e| e.to_string())? {
            0 => break,
            1 => kt = r.g1().map_err(|e| e.to_string())?,
            2 => vt = r.g1().map_err(|e| e.to_string())?,
            3 => default_text = latin1(r.gjstr().map_err(|e| e.to_string())?),
            4 => default_int = r.g4().map_err(|e| e.to_string())?,
            op @ (5 | 6) => {
                found = None;
                for _ in 0..r.g2().map_err(|e| e.to_string())? {
                    let k = r.g4().map_err(|e| e.to_string())?;
                    let value = if op == 5 {
                        HookArg::Text(latin1(r.gjstr().map_err(|e| e.to_string())?))
                    } else {
                        HookArg::Int(r.g4().map_err(|e| e.to_string())?)
                    };
                    if k == key && found.is_none() {
                        found = Some(value);
                    }
                }
            }
            _ => {}
        }
    }
    if check && (kt != expected_key || vt != expected_value) {
        return Err(format!("CS2 enum type mismatch {id}"));
    }
    Ok(found.unwrap_or({
        if vt == 115 {
            HookArg::Text(default_text)
        } else {
            HookArg::Int(default_int)
        }
    }))
}
