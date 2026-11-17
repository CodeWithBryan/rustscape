//! The tick driver (world::driver) against the world-trace scenarios.
//!
//! The scripted steps of new-character, two-characters and
//! returning-character are driven through the native login (the binding of
//! the parent module, unchanged) and the native tick driver: every `tick`
//! step is one `Driver::tick` (packet queue, disconnection queue, the pulse
//! runner with the login pulse in it, tick listeners, the update sequence
//! with `Player.update`/`Player.reset` and the native player renderer,
//! `GameWorld.pulse`, managers, the write queue's flush), every `close` is
//! `Driver::close` (`IoSession.disconnect`, `Player.clear`), and the
//! disconnection queue finishes the player as the original does, with the
//! native save written to the scenario directory.
//!
//! Compared per step: every entry each session received (bytes and the
//! dropped flag, in order), the repository's characters (slot, index, name,
//! location, active, playing, renderable, queued for disconnection), the
//! lobby, the tick counter, the management events published, and the files
//! written. Where the original reaches a system this world does not run,
//! the stand-in replays the original's entries for that call (queued
//! entries by call stack, as the parent module does; direct entries by
//! opcode at the call's position) or records the call. Every replayed
//! segment, every remaining difference and every stand-in kind is pinned
//! below; a gap that closes or a new difference fails the test.
use super::*;
use rs09_server::{
    auth::storage::AccountStorage,
    pulses,
    world::{
        disconnection,
        driver::{self, Autosave, Driver, Level, Metric, dispatch},
        social::Event,
    },
};
use std::collections::VecDeque;

/// An outgoing packet: the session it goes to, its class and its bytes.
#[derive(Clone, Debug)]
struct Sent {
    session: usize,
    class: String,
    bytes: Vec<u8>,
    /// The queue position it was pushed at, for queued packets.
    at: usize,
}

/// `TaskExecutor`'s hand-offs, run when the harness pumps after a step.
enum Task {
    Save(disconnection::SaveTask<P>),
    Autosave(Autosave),
}

struct World {
    binding: Binding,
    driver: Driver<P, Sent>,
    repository: Repository<P>,
    storage: Arc<InMemoryStorage>,
    login_listeners: Vec<String>,
    logout_listeners: Vec<String>,
    logins: Vec<(pulses::Handle, login::pulse::LoginPulse<P, Parser, usize>)>,
    renderable: HashMap<usize, bool>,
    op_counts: HashMap<usize, [i8; dispatch::OP_COUNT_SLOTS]>,
    invalid: HashMap<usize, i32>,
    tasks: VecDeque<Task>,
    /// The original's directly written entries of the tick, per session.
    expected_direct: HashMap<usize, Vec<Original>>,
    /// (session, marker, entries) of every direct replay.
    direct_replays: Vec<(usize, String, usize)>,
    /// Calls to systems not run here, by kind.
    stand_ins: BTreeMap<String, usize>,
    logs: Vec<String>,
    written: Vec<String>,
    /// Queued packets inserted into the session queue at the flush so far.
    inserted: usize,
}

fn fail(e: impl std::fmt::Debug) -> driver::Error {
    driver::Error::Exception(format!("{e:?}"))
}

impl World {
    fn stand_in(&mut self, kind: &str) {
        *self.stand_ins.entry(kind.to_owned()).or_insert(0) += 1;
    }
    fn session_of(&self, p: &P) -> usize {
        self.binding.inner.session_of(p)
    }
    fn render(&self, p: &P) -> player_render::Handle {
        self.binding.inner.canonical(p).render.clone()
    }
    fn is_character(&self, p: &P) -> bool {
        !self.binding.inner.slot(p).artificial
    }
    /// Replays the original's direct entries at the session's current
    /// direct position with this opcode (one, or every consecutive one).
    fn replay_direct(&mut self, p: &P, opcode: u8, marker: &str, many: bool) -> usize {
        let session = self.session_of(p);
        let position = self.binding.inner.sessions[session].direct.len();
        let expected = self
            .expected_direct
            .get(&session)
            .cloned()
            .unwrap_or_default();
        let mut n = 0;
        while let Some(entry) = expected.get(position + n) {
            if entry.op != Some(opcode) {
                break;
            }
            self.binding.inner.sessions[session].direct.push(Out {
                bytes: unhex(&entry.hex),
                replay: Some(marker.to_owned()),
            });
            n += 1;
            if !many {
                break;
            }
        }
        self.direct_replays.push((session, marker.to_owned(), n));
        n
    }
    /// Runs what `TaskExecutor` was handed during the step, in order.
    fn pump(&mut self) -> usize {
        let mut blocks = 0;
        while let Some(task) = self.tasks.pop_front() {
            blocks += 1;
            let queue = self.repository.disconnection_queue();
            match task {
                Task::Save(task) => {
                    task.run(self, &queue).unwrap();
                }
                Task::Autosave(task) => {
                    let repository = self.repository.clone();
                    task.run(self, &repository).unwrap();
                }
            }
        }
        blocks
    }
}

// ------------------------------------------------ repository::Host

impl repository::Host<P> for World {
    type Session = usize;
    type Clan = ();
    type Location = ();
    fn uid(&mut self, p: &P) -> i32 {
        repository::Host::uid(&mut self.binding, p)
    }
    fn name(&mut self, p: &P) -> Option<String> {
        repository::Host::name(&mut self.binding, p)
    }
    fn details_username(&mut self, p: &P) -> Option<String> {
        repository::Host::details_username(&mut self.binding, p)
    }
    fn set_index(&mut self, p: &P, index: i32) {
        repository::Host::set_index(&mut self.binding, p, index)
    }
    fn is_artificial(&mut self, p: &P) -> bool {
        repository::Host::is_artificial(&mut self.binding, p)
    }
    fn afk_logout(&mut self, p: &P) -> Option<bool> {
        repository::Host::afk_logout(&mut self.binding, p)
    }
    fn set_playing(&mut self, p: &P, playing: bool) {
        repository::Host::set_playing(&mut self.binding, p, playing)
    }
    fn session(&mut self, p: &P) -> Option<usize> {
        repository::Host::session(&mut self.binding, p)
    }
    fn ticks(&mut self) -> i32 {
        self.driver.ticks().get()
    }
    fn global_chat_enabled(&mut self) -> bool {
        repository::Host::global_chat_enabled(&mut self.binding)
    }
    fn to_lower_case(&mut self, text: &str) -> String {
        repository::Host::to_lower_case(&mut self.binding, text)
    }
    fn session_active(&mut self, s: &usize) -> bool {
        repository::Host::session_active(&mut self.binding, s)
    }
    fn set_session_active(&mut self, s: &usize, active: bool) {
        repository::Host::set_session_active(&mut self.binding, s, active)
    }
    fn session_has_key(&mut self, s: &usize) -> bool {
        repository::Host::session_has_key(&mut self.binding, s)
    }
    fn key_cancel(&mut self, s: &usize) -> repository::Result<()> {
        repository::Host::key_cancel(&mut self.binding, s)
    }
    fn socket_close(&mut self, s: &usize) -> repository::Result<()> {
        repository::Host::socket_close(&mut self.binding, s)
    }
    fn session_object(&mut self, s: &usize) -> SessionObject<P> {
        repository::Host::session_object(&mut self.binding, s)
    }
    fn set_session_object(&mut self, s: &usize, p: Option<P>) {
        repository::Host::set_session_object(&mut self.binding, s, p)
    }
    fn session_username(&mut self, s: &usize) -> Option<String> {
        repository::Host::session_username(&mut self.binding, s)
    }
    /// `Player.clear()` of a plain player: native.
    fn player_clear(&mut self, repository: &Repository<P>, p: &P) -> repository::Result<()> {
        disconnection::player_clear(self, repository, p)
    }
    /// `UpdateSequence.renderablePlayers.remove(player)`: the driver's list.
    fn renderable_players_remove(&mut self, p: &P) -> repository::Result<bool> {
        Ok(self.driver.renderable_players().remove(p))
    }
    /// `getAttribute("combat-time", 0L) > System.currentTimeMillis()`.
    fn in_combat(&mut self, p: &P) -> repository::Result<bool> {
        let value = self
            .binding
            .inner
            .canonical(p)
            .player
            .attributes()
            .get("combat-time");
        let time = match value {
            None => 0,
            Some(AttributeValue::Long(v)) => v,
            Some(other) => panic!("combat-time is {}", other.java_string()),
        };
        Ok(time > now())
    }
    fn lifepoints(&mut self, p: &P) -> repository::Result<i32> {
        self.binding
            .inner
            .with_profile(p, |profile| profile.skills.lifepoints)
            .map_err(|e| repository::Error::Exception(format!("{e:?}")))
    }
    fn is_dead(&mut self, _: &P) -> repository::Result<bool> {
        // DeathTask is not joined here; no death was started.
        self.stand_in("DeathTask.isDead (not joined: false)");
        Ok(false)
    }
    fn is_teleporting(&mut self, p: &P) -> repository::Result<bool> {
        Ok(self.render(p).read().teleporting)
    }
    fn has_soft_script(&mut self, _: &P) -> repository::Result<bool> {
        // The canonical script queue is not joined here; nothing queued one.
        self.stand_in("scripts.hasTypeInQueue(SOFT) (not joined: false)");
        Ok(false)
    }
    /// `PacketRepository.send(Logout.class, context)`: written at once.
    fn logout_packet(&mut self, p: &P) -> repository::Result<()> {
        let session = self.session_of(p);
        let packet = Sent {
            session,
            class: "Logout".into(),
            bytes: vec![86],
            at: 0,
        };
        let driver = self.driver.clone();
        driver
            .send(self, "Logout", Some(packet))
            .map(|_| ())
            .map_err(repository::Error::from)
    }
    fn world_logout_listeners(&mut self, _: &P) -> repository::Result<()> {
        // Content: the logout listeners by class name, not ported.
        for listener in self.logout_listeners.clone() {
            self.stand_in(&format!("logout listener {listener}"));
        }
        Ok(())
    }
    fn walking_queue_reset(&mut self, _: &P) -> repository::Result<()> {
        self.stand_in("WalkingQueue.reset (walking not joined)");
        Ok(())
    }
    fn player_logout_listeners(&mut self, _: &P) -> repository::Result<()> {
        self.stand_in("Player.logoutListeners (content, none natively)");
        Ok(())
    }
    fn has_familiar(&mut self, _: &P) -> repository::Result<bool> {
        self.stand_in("FamiliarManager.hasFamiliar (not ported: false)");
        Ok(false)
    }
    fn familiar_clear(&mut self, _: &P) -> repository::Result<()> {
        panic!("{NOT_REACHED}")
    }
    /// `interfaceManager.close()`: the native interface manager.
    fn interface_close(&mut self, p: &P) -> repository::Result<()> {
        let (ui, manager) = login::Host::ui(&mut self.binding, p).unwrap();
        interfaces::close(ui, &manager)
            .map(|_| ())
            .map_err(|e| repository::Error::Exception(format!("{e:?}")))
    }
    fn interface_close_single_tab(&mut self, p: &P) -> repository::Result<()> {
        let (ui, manager) = login::Host::ui(&mut self.binding, p).unwrap();
        interfaces::close_single_tab(ui, &manager)
            .map(|_| ())
            .map_err(|e| repository::Error::Exception(format!("{e:?}")))
    }
    /// `Entity.clear()`: `active = false` is the field; the chunk, region
    /// tolerances and the pulse manager are not joined here.
    fn entity_clear(&mut self, p: &P) -> repository::Result<()> {
        self.binding.inner.players[p.0].active = false;
        self.render(p).write().active = false;
        self.stand_in("Entity.clear chunk/tolerances/pulseManager (not joined)");
        Ok(())
    }
    fn zone_monitor_clear(&mut self, _: &P) -> repository::Result<()> {
        self.stand_in("ZoneMonitor.clear (zones not joined)");
        Ok(())
    }
    fn house_leave(&mut self, _: &P) -> repository::Result<()> {
        self.stand_in("HouseManager.leave (not ported)");
        Ok(())
    }
    /// `ManagementEvents.publish`: the native social queue.
    fn publish_player_status(
        &mut self,
        username: &str,
        world: i32,
        notify_friends_only: bool,
    ) -> repository::Result<()> {
        self.binding.social.publish(Event::PlayerStatusUpdate {
            username: username.to_owned(),
            world,
            notify_friends_only,
        });
        Ok(())
    }
    fn clan(&mut self, p: &P) -> repository::Result<Option<()>> {
        Ok(self.binding.social.clan_of(p).map(|_| ()))
    }
    fn world_id(&mut self) -> repository::Result<i32> {
        repository::Host::world_id(&mut self.binding)
    }
    fn clan_owner(&mut self, _: &()) -> repository::Result<Option<String>> {
        panic!("{NOT_REACHED}")
    }
    fn publish_clan_leave(&mut self, _: &str, _: i32, _: &str) -> repository::Result<()> {
        panic!("{NOT_REACHED}")
    }
    fn wealth_update(&mut self, _: &P, _: bool) -> repository::Result<()> {
        self.stand_in("checkForWealthUpdate (not ported)");
        Ok(())
    }
    fn clan_leave(&mut self, _: &(), _: &P, _: bool) -> repository::Result<()> {
        panic!("{NOT_REACHED}")
    }
    /// `details.save()`: the native PlayerDetails into the native storage.
    fn details_save(&mut self, p: &P) -> repository::Result<()> {
        let storage = self.storage.clone();
        self.binding
            .inner
            .with_profile(p, |profile| {
                profile.details.save(&*storage as &dyn AccountStorage)
            })
            .map_err(|e| repository::Error::Exception(format!("{e:?}")))?
            .map_err(|e| repository::Error::Exception(format!("{e:?}")))
    }
    fn execute_sql(&mut self, task: disconnection::SaveTask<P>) -> repository::Result<()> {
        self.tasks.push_back(Task::Save(task));
        Ok(())
    }
    fn set_thread_name(&mut self, name: &str) {
        self.logs.push(format!("thread {name}"));
    }
    /// `PlayerParser.saveImmediately`: the canonical player's save file.
    fn save_immediately(&mut self, p: &P) -> repository::Result<()> {
        let location = self.binding.inner.slot(p).location.clone();
        let path = self.binding.inner.save_path.clone();
        let (outcome, calls) = self.binding.inner.environment_run(p, |player, state, env| {
            player.save_file(state, Some(&location), env, &path)
        });
        self.binding.inner.calls.extend(calls);
        let outcome = outcome.map_err(|e| repository::Error::Exception(format!("{e:?}")))?;
        let name = self.binding.inner.slot(p).name.clone();
        self.written
            .push(format!("data/players/{name}.json {outcome:?}"));
        Ok(())
    }
    fn log(&mut self, level: LogLevel, message: &str) -> repository::Result<()> {
        self.logs.push(format!("{level:?} {message}"));
        Ok(())
    }
    fn print_stack_trace(&mut self, error: &repository::Error) {
        self.logs.push(format!("printStackTrace {error:?}"));
    }
    fn is_revenant(&mut self, _: &P) -> bool {
        panic!("{NOT_REACHED}")
    }
    /// `node.setRenderable(value)`.
    fn set_renderable(&mut self, p: &P, renderable: bool) {
        self.renderable.insert(p.0, renderable);
    }
    fn npc_id(&mut self, _: &P) -> i32 {
        panic!("{NOT_REACHED}")
    }
    fn region_chunk_npcs(&mut self, _: &()) -> repository::Result<Vec<P>> {
        panic!("{NOT_REACHED}")
    }
    fn npc_at(&mut self, _: &P, _: &()) -> bool {
        panic!("{NOT_REACHED}")
    }
    fn ge_news_muted(&mut self, _: &P) -> repository::Result<bool> {
        panic!("{NOT_REACHED}")
    }
    fn send_message(&mut self, _: &P, _: &str) -> repository::Result<()> {
        panic!("{NOT_REACHED}")
    }
}

// ------------------------------------------------ driver::Host

/// The native player renderer's services: candidates from the characters
/// in the repository within 15 tiles (entity regions are not joined; this
/// answers what `RegionManager.getLocalPlayers` would for these scenarios),
/// update masks not joined (reaching them fails the test), frames out.
struct Render {
    candidates: Vec<Option<player_render::Handle>>,
    frames: Vec<Frame>,
}
impl player_render::Context for Render {
    fn candidates(
        &mut self,
        _: Location,
        _: i32,
    ) -> player_render::Result<Vec<Option<player_render::Handle>>> {
        Ok(std::mem::take(&mut self.candidates))
    }
    fn masks(
        &mut self,
        _: &player_render::Handle,
        _: &player_render::Handle,
        _: &mut Vec<u8>,
        _: bool,
        _: bool,
    ) -> player_render::Result<()> {
        Err(player_render::Error::Binding(
            "update masks are not joined in this test".into(),
        ))
    }
    fn send(&mut self, _: &player_render::Handle, frame: Frame) -> player_render::Result<()> {
        self.frames.push(frame);
        Ok(())
    }
}

/// `Location.withinDistance(other, 15)`.
fn within(a: &Location, b: &Location) -> bool {
    let [x, y, z] = a.coordinates();
    let [xx, yy, zz] = b.coordinates();
    z == zz && (x - xx).abs() <= 15 && (y - yy).abs() <= 15
}

impl driver::Host<P> for World {
    type Outgoing = Sent;
    fn clock_millis(&mut self) -> i64 {
        now()
    }
    fn telemetry(&mut self, _: Metric, _: i32) {}
    fn metrics_enabled(&mut self) -> bool {
        false
    }
    fn is_playing(&mut self, p: &P) -> bool {
        self.binding.inner.slot(p).playing
    }
    fn driver_log(&mut self, origin: &str, level: Level, message: &str) {
        self.logs.push(format!("{origin} {level:?} {message}"));
    }
    fn stack_trace(&mut self, error: &driver::Error) -> String {
        format!("{error:?}\n")
    }
    fn driver_print_stack_trace(&mut self, error: &driver::Error) {
        self.logs.push(format!("printStackTrace {error:?}"));
    }
    fn reading_queue(&mut self, _: &usize) -> Option<Vec<u8>> {
        panic!("{NOT_REACHED}: client input")
    }
    fn set_reading_queue(&mut self, _: &usize, _: Option<Vec<u8>>) {
        panic!("{NOT_REACHED}: client input")
    }
    fn set_last_ping(&mut self, _: &usize, _: i64) {
        panic!("{NOT_REACHED}: client input")
    }
    fn add_invalid_packet(&mut self, p: &P) -> i32 {
        let count = self.invalid.entry(p.0).or_insert(0);
        *count = count.wrapping_add(1);
        *count
    }
    fn op_counts(&mut self, p: &P) -> &mut [i8; dispatch::OP_COUNT_SLOTS] {
        self.op_counts
            .entry(p.0)
            .or_insert([0; dispatch::OP_COUNT_SLOTS])
    }
    fn handle(
        &mut self,
        handler: dispatch::Handler,
        _: &dispatch::Packet<P>,
    ) -> driver::Result<()> {
        panic!("{NOT_REACHED}: client input ({handler:?})")
    }
    fn is_administrator(&mut self, _: &P) -> driver::Result<bool> {
        panic!("{NOT_REACHED}: client input")
    }
    fn set_afk_logout(&mut self, _: &P, _: bool) {
        panic!("{NOT_REACHED}: client input")
    }
    fn packet_count(&mut self, _: &P, _: i32) -> driver::Result<i32> {
        panic!("{NOT_REACHED}: client input")
    }
    /// The login pulse: `world::login::pulse::run`. No other world pulse
    /// is in the native runner.
    fn pulse_body(&mut self, pulse: &pulses::Handle) -> driver::Result<bool> {
        let at = self
            .logins
            .iter()
            .position(|(h, _)| h.same(pulse))
            .expect("only login pulses are submitted");
        let login = self.logins.remove(at).1;
        let repository = self.repository.clone();
        let listeners = self.login_listeners.clone();
        let done =
            login::pulse::run(&mut self.binding, &repository, &listeners, &login).map_err(fail)?;
        // The login library asked for UpdateSequence.renderablePlayers.add
        // (the parent binding records the call); it is done on the driver's
        // list here.
        let name = self.binding.inner.slot(&login.player).name.clone();
        let wanted = format!("UpdateSequence.renderablePlayers.add {name}");
        if self.binding.inner.calls.contains(&wanted) {
            self.binding.inner.calls.retain(|c| *c != wanted);
            self.driver.renderable_players().add(login.player);
        }
        if !done {
            self.logins.insert(at, (pulse.clone(), login));
        }
        Ok(done)
    }
    fn pulse_node_active(&mut self, _: pulses::Node) -> driver::Result<bool> {
        panic!("{NOT_REACHED}: checked nodes")
    }
    fn tick_listeners(&mut self) -> driver::Result<()> {
        // The phase before this one ran the native pulse runner, which holds
        // only the login pulses: the content world pulses the original ran
        // there (submitted by login listeners that are not ported) are
        // replayed here, at the same queue position.
        let characters: Vec<P> = (0..self.binding.inner.players.len())
            .map(P)
            .filter(|p| self.binding.inner.slot(p).session.is_some())
            .collect();
        for p in characters {
            self.binding
                .inner
                .replay(&p, "core.game.world.PulseRunner.updateAll:");
        }
        self.stand_in("GameWorld.tickListeners (content, not ported)");
        Ok(())
    }
    fn managers_tick(&mut self) -> driver::Result<()> {
        self.stand_in("Managers.tick (manager plugins, not ported)");
        Ok(())
    }
    fn set_active(&mut self, p: &P, active: bool) -> driver::Result<()> {
        self.binding.inner.players[p.0].active = active;
        Ok(())
    }
    fn clear_minimap_packet(&mut self, _: &P) -> Option<Sent> {
        panic!("{NOT_REACHED}: the lobby")
    }
    fn npc_tick(&mut self, _: &P) -> driver::Result<()> {
        panic!("{NOT_REACHED}: no NPC is in this world")
    }
    fn npc_reset(&mut self, _: &P) -> driver::Result<()> {
        panic!("{NOT_REACHED}: no NPC is in this world")
    }
    /// `Player.tick()` is not joined: what the original queued from it
    /// (walking's region move and the music it plays) is replayed.
    fn player_tick(&mut self, p: &P) -> driver::Result<()> {
        assert!(self.is_character(p));
        self.binding
            .inner
            .replay(p, "core.game.node.entity.player.Player.tick:");
        self.stand_in("Player.tick (Entity.tick, music, prayer, wealth: not joined)");
        Ok(())
    }
    fn update_scene_graph_flag(&mut self, p: &P) -> driver::Result<bool> {
        Ok(self.render(p).read().update_scene)
    }
    fn set_update_scene_graph_flag(&mut self, p: &P, value: bool) -> driver::Result<()> {
        self.render(p).write().update_scene = value;
        Ok(())
    }
    /// `Player.updateSceneGraph(false)`: the native rebuild writer.
    fn update_scene_graph(&mut self, p: &P) -> driver::Result<()> {
        login::Host::update_scene_graph(&mut self.binding, p, false).map_err(fail)
    }
    /// `PlayerRenderer.render`: native.
    fn render_players(&mut self, p: &P) -> driver::Result<()> {
        let local = self.render(p);
        let here = local.read().location.clone().expect("a location");
        let mut candidates = Vec::new();
        for (i, slot) in self.binding.inner.players.iter().enumerate() {
            let Some(c) = &slot.canonical else { continue };
            if !self.repository.players().contains(Some(&P(i))) {
                continue;
            }
            let there = c.render.read().location.clone().expect("a location");
            if within(&here, &there) {
                candidates.push(Some(c.render.clone()));
            }
        }
        let mut ctx = Render {
            candidates,
            frames: Vec::new(),
        };
        player_render::render(&mut ctx, &local).map_err(fail)?;
        let session = self.session_of(p);
        for frame in ctx.frames {
            self.binding.inner.direct_native(session, frame);
        }
        Ok(())
    }
    /// `NPCRenderer.render`: the NPC world is not joined; the original's
    /// NPC update (32) is replayed.
    fn render_npcs(&mut self, p: &P) -> driver::Result<()> {
        self.replay_direct(p, 32, "NPCRenderer.render", false);
        Ok(())
    }
    /// `MapChunkRenderer.render`: chunk synchronisation is not joined; the
    /// original's chunk updates (230) are replayed.
    fn render_map_chunks(&mut self, p: &P) -> driver::Result<()> {
        self.replay_direct(p, 230, "MapChunkRenderer.render", true);
        Ok(())
    }
    /// `Entity.reset()`: `updateMasks.reset()` and `setTeleporting(false)`
    /// on the render state (prepared masks are not joined here).
    fn entity_reset(&mut self, p: &P) -> driver::Result<()> {
        let render = self.render(p);
        let mut render = render.write();
        render.masks.required = false;
        render.teleporting = false;
        Ok(())
    }
    fn update_render_information(&mut self, p: &P) -> driver::Result<()> {
        let render = self.render(p);
        let location = render.read().location.clone();
        let info = render.read().info.clone();
        info.write().update_information(location);
        Ok(())
    }
    fn lifepoints_update(&mut self, p: &P) -> driver::Result<bool> {
        self.binding
            .inner
            .with_profile(p, |profile| profile.skills.lifepoints_update)
            .map_err(fail)
    }
    fn set_lifepoints_update(&mut self, _: &P, _: bool) -> driver::Result<()> {
        panic!("{NOT_REACHED}: a lifepoints update")
    }
    fn hitpoints_packet(&mut self, _: &P) -> Option<Sent> {
        panic!("{NOT_REACHED}: a lifepoints update")
    }
    fn attribute_bool(&mut self, p: &P, key: &str, default: bool) -> driver::Result<bool> {
        self.binding
            .inner
            .canonical(p)
            .player
            .attributes()
            .boolean(key, default)
            .map_err(fail)
    }
    fn remove_attribute(&mut self, p: &P, key: &str) -> driver::Result<()> {
        self.binding
            .inner
            .canonical(p)
            .player
            .attributes()
            .remove(key);
        Ok(())
    }
    fn reset_chunks(&mut self) -> driver::Result<()> {
        self.stand_in("ChunkUpdateTracker.resetAll (chunk sync not joined)");
        Ok(())
    }
    fn ground_items_pulse(&mut self) -> driver::Result<()> {
        self.stand_in("GroundItemManager.pulse (ground items not joined)");
        Ok(())
    }
    fn execute_autosave(&mut self, task: Autosave) -> driver::Result<()> {
        self.tasks.push_back(Task::Autosave(task));
        Ok(())
    }
    /// Written at once, or at the flush into the session's queue at the
    /// position it was pushed at.
    fn send_packet(&mut self, packet: &Sent) -> driver::Result<()> {
        let out = Out {
            bytes: packet.bytes.clone(),
            replay: None,
        };
        if driver::write_queue::IMMEDIATE.contains(&packet.class.as_str()) {
            self.binding.inner.sessions[packet.session].direct.push(out);
            return Ok(());
        }
        let at = (packet.at + self.inserted).min(self.binding.inner.queue.len());
        self.binding.inner.queue.insert(at, (packet.session, out));
        self.inserted += 1;
        Ok(())
    }
}

// ------------------------------------------------ the scenario

/// One session's entries at one step, compared.
#[derive(Debug, Default, Clone)]
struct Tally {
    entries: usize,
    native: usize,
    replayed: BTreeMap<String, usize>,
    /// (tick, entry, opcode) of natively written entries that differ.
    differ: Vec<(i64, usize, Option<u8>)>,
}

struct Driven {
    tallies: BTreeMap<String, Tally>,
    /// (step, native, original) for the repository, lobby and counter.
    states: Vec<(String, Value, Value)>,
    /// (step, native events, original events).
    events: Vec<(String, Vec<String>, Vec<String>)>,
    /// (path, native bytes, original bytes).
    saves: Vec<(String, Vec<u8>, Vec<u8>)>,
    stand_ins: BTreeMap<String, usize>,
    pulses: Vec<(i64, usize, i64)>,
    logs: Vec<String>,
    refusals: usize,
    /// The tick the autosave was handed off at and what it wrote.
    autosave: Option<(i32, Vec<String>)>,
}

fn character_rows(world: &World) -> Value {
    let queue = world.repository.disconnection_queue();
    let rows: Vec<Value> = world
        .repository
        .players()
        .occupied()
        .into_iter()
        .filter(|(_, p)| world.is_character(p))
        .map(|(slot, p)| {
            let s = world.binding.inner.slot(&p);
            serde_json::json!({
                "slot": slot, "index": s.index, "name": s.name,
                "location": s.location.coordinates(), "active": s.active,
                "playing": s.playing,
                "renderable": world.renderable.get(&p.0).copied().unwrap_or(false),
                "queuedForDisconnect": queue.contains(Some(&s.name)),
            })
        })
        .collect();
    serde_json::json!({
        "players": rows,
        "lobby": world.repository.lobby_players().values().len(),
        "tick": world.driver.ticks().get(),
    })
}

fn original_rows(v: &Value) -> Value {
    let rows: Vec<Value> = v["players"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| !r["artificial"].as_bool().unwrap())
        .map(|r| {
            serde_json::json!({
                "slot": r["slot"], "index": r["index"], "name": r["name"],
                "location": r["location"], "active": r["active"], "playing": r["playing"],
                "renderable": r["renderable"], "queuedForDisconnect": r["queuedForDisconnect"],
            })
        })
        .collect();
    serde_json::json!({
        "players": rows,
        "lobby": v["lobby"].as_array().unwrap().len(),
        "tick": v["tick"],
    })
}

fn drive(scenario: &Scenario, seed: &[(String, Vec<u8>)], autosave: bool) -> Driven {
    let (config, constants, start) = configuration();
    NOW.with(|n| n.store(1_689_163_200_000, Ordering::SeqCst));
    let directory = std::env::temp_dir().join(format!(
        "rs09-tick-driver-{}-{}-{}",
        std::process::id(),
        RUN.fetch_add(1, Ordering::SeqCst),
        scenario.name
    ));
    std::fs::create_dir_all(directory.join("data/players")).unwrap();
    for (path, bytes) in seed {
        std::fs::write(directory.join(path), bytes).unwrap();
    }
    let save_path = format!(
        "{}/data/players/",
        directory.to_string_lossy().replace('\\', "/")
    );
    let items = Arc::new(Items::default());
    let mut registry = interfaces::Registry::default();
    registry.load(INTERFACE_CONFIGS).unwrap();
    let state = State::default();
    let (varp, id, start_bit, end_bit) = SPELL_BOOK_VARBIT;
    state
        .identity()
        .varbits()
        .create(varp, id, start_bit, end_bit);
    let clock: Arc<Clock> = Arc::new(now);
    let binding = Binding {
        inner: Inner {
            state,
            frames: BTreeMap::new(),
            factory: Factory::new(&items),
            items,
            registry,
            options: interaction_options::Constants::default(),
            xteas: xteas(),
            constants,
            world: world_inputs(),
            hooks: boot()["listeners"]["persistPlayer"]
                .as_array()
                .unwrap()
                .len(),
            players: Vec::new(),
            sessions: Vec::new(),
            queue: Vec::new(),
            expected: HashMap::new(),
            replays: Vec::new(),
            calls: Vec::new(),
            pending: Vec::new(),
            ticks: 0,
            current: P(0),
            save_path,
            clock: clock.clone(),
            start,
        },
        social: social::Social::new(),
    };
    let repository: Repository<P> =
        Repository::new(repository::MAX_PLAYERS, repository::MAX_NPCS).unwrap();
    let storage = Arc::new(InMemoryStorage::default());
    let auth_settings = config.auth_settings();
    let authenticator = Authenticator::new(
        Mode::Development {
            default_admin: auth_settings.default_admin,
        },
        storage.clone(),
        Arc::new(Passwords::with_salt_source(4, || Ok([0; 16])).unwrap()),
    );
    let names = |key: &str| -> Vec<String> {
        boot()["listeners"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect()
    };
    let mut world = World {
        binding,
        driver: Driver::new(repository.clone()),
        repository: repository.clone(),
        storage,
        login_listeners: names("login"),
        logout_listeners: names("logout"),
        logins: Vec::new(),
        renderable: HashMap::new(),
        op_counts: HashMap::new(),
        invalid: HashMap::new(),
        tasks: VecDeque::new(),
        expected_direct: HashMap::new(),
        direct_replays: Vec::new(),
        stand_ins: BTreeMap::new(),
        logs: Vec::new(),
        written: Vec::new(),
        inserted: 0,
    };
    let key = RsaKey::reference_private();
    let mut out = Driven {
        tallies: BTreeMap::new(),
        states: Vec::new(),
        events: Vec::new(),
        saves: Vec::new(),
        stand_ins: BTreeMap::new(),
        pulses: Vec::new(),
        logs: Vec::new(),
        refusals: 0,
        autosave: None,
    };
    for record in &scenario.records {
        let v = &record.value;
        if matches!(v["t"].as_str().unwrap(), "boot" | "end") {
            continue;
        }
        let step = format!(
            "{} {} tick {}",
            v["op"].as_str().unwrap(),
            v["i"],
            v["tick"]
        );
        let events_before = world.binding.social.pending().count();
        world.written.clear();
        if autosave && v["op"] == "close" {
            out.autosave = Some(tick_to_autosave(&mut world));
            break;
        }
        match v["op"].as_str().unwrap() {
            "connect" => {
                world.binding.inner.sessions.push(Session {
                    id: v["session"].as_str().unwrap().to_owned(),
                    active: true,
                    object: None,
                    game: false,
                    username: None,
                    direct: Vec::new(),
                    published: false,
                });
            }
            "recv" => {
                let session = world
                    .binding
                    .inner
                    .sessions
                    .iter()
                    .position(|s| s.id == v["session"])
                    .unwrap();
                let bytes = unhex(v["hex"].as_str().unwrap());
                if bytes[0] == 14 {
                    // The handshake reply is the transport's (see the parent module).
                    continue;
                }
                let parsed = codec::decode(&bytes, &key, |name| {
                    repository
                        .get_player_by_name(&mut world.binding, Some(name))
                        .is_some()
                });
                assert_eq!(parsed.response, AuthResponse::Success);
                let name = parsed.info.as_ref().unwrap().username.clone();
                world.binding.inner.sessions[session].username = Some(name);
                let Decision::Ready(authenticated) =
                    admission::check(parsed, &authenticator, &*clock)
                else {
                    panic!("authentication refused")
                };
                let prepared = players::prepare(authenticated, clock.clone()).unwrap();
                let opcode = prepared.login.opcode;
                let admitted = login::admission::read(
                    &mut world.binding,
                    &repository,
                    &session,
                    prepared.details,
                    opcode,
                )
                .unwrap();
                assert!(matches!(admitted, login::admission::Admitted::Submitted(_)));
                // GameWorld.Pulser.submit(Pulse(1)): the native runner.
                for pulse in std::mem::take(&mut world.binding.inner.pending) {
                    let handle = Shared::new(pulses::Pulse::new(
                        login::pulse::DELAY,
                        None,
                        pulses::Class::Other,
                        "core.game.node.entity.player.info.login.LoginParser$initialize$1".into(),
                    ));
                    world.driver.pulser().submit(Some(handle.clone())).unwrap();
                    world.logins.push((handle, pulse));
                }
                assert!(world.binding.inner.sessions[session].direct.is_empty());
                assert!(
                    !record
                        .out
                        .contains_key(&world.binding.inner.sessions[session].id)
                );
            }
            "tick" => {
                NOW.with(|n| n.fetch_add(600, Ordering::SeqCst));
                world.binding.inner.ticks = world.driver.ticks().get();
                world.binding.inner.queue.clear();
                world.binding.inner.expected.clear();
                world.binding.inner.replays.clear();
                world.expected_direct.clear();
                world.direct_replays.clear();
                world.inserted = 0;
                for s in 0..world.binding.inner.sessions.len() {
                    world.binding.inner.sessions[s].direct.clear();
                    let id = world.binding.inner.sessions[s].id.clone();
                    let originals = record.out.get(&id).cloned().unwrap_or_default();
                    world.binding.inner.expected.insert(
                        s,
                        originals
                            .iter()
                            .filter(|o| o.origin.is_some())
                            .cloned()
                            .collect(),
                    );
                    world.expected_direct.insert(
                        s,
                        originals
                            .into_iter()
                            .filter(|o| o.origin.is_none())
                            .collect(),
                    );
                }
                // The management events handled between the steps (their
                // handlers are not joined): their queued packets first.
                let characters: Vec<P> = (0..world.binding.inner.players.len())
                    .map(P)
                    .filter(|p| world.binding.inner.slot(p).session.is_some())
                    .collect();
                for p in characters {
                    world
                        .binding
                        .inner
                        .replay(&p, "ManagementEvents.handleEvent");
                }
                let driver = world.driver.clone();
                driver.tick(&mut world, false).unwrap();
                // The sessions' output: written at once, then the flush.
                for s in 0..world.binding.inner.sessions.len() {
                    let id = world.binding.inner.sessions[s].id.clone();
                    let originals = record.out.get(&id).cloned().unwrap_or_default();
                    let dropped: Vec<bool> = v["out"][&id]
                        .as_array()
                        .map(|a| a.iter().map(|e| e["dropped"] == true).collect())
                        .unwrap_or_default();
                    let mut native: Vec<Out> =
                        std::mem::take(&mut world.binding.inner.sessions[s].direct);
                    native.extend(
                        world
                            .binding
                            .inner
                            .queue
                            .iter()
                            .filter(|(q, _)| *q == s)
                            .map(|(_, o)| o.clone()),
                    );
                    let active = world.binding.inner.sessions[s].active;
                    let tally = out
                        .tallies
                        .entry(format!("{} {id}", scenario.name))
                        .or_default();
                    assert_eq!(
                        native.len(),
                        originals.len(),
                        "{} {id} {step}: {:?}",
                        scenario.name,
                        first_mismatch(&native, &originals)
                    );
                    for (n, (mine, original)) in native.iter().zip(&originals).enumerate() {
                        // Bytes queued after the connection closed are dropped.
                        assert_eq!(
                            !active, dropped[n],
                            "{} {id} {step}: dropped",
                            scenario.name
                        );
                        tally.entries += 1;
                        match &mine.replay {
                            Some(marker) => {
                                assert_eq!(hex(&mine.bytes), original.hex);
                                *tally.replayed.entry(marker.clone()).or_insert(0) += 1;
                            }
                            None if hex(&mine.bytes) == original.hex => tally.native += 1,
                            None => {
                                tally
                                    .differ
                                    .push((v["tick"].as_i64().unwrap(), n, original.op))
                            }
                        }
                    }
                }
                add_artificial_players(&mut world.binding, &repository, v);
                out.pulses.push((
                    v["tick"].as_i64().unwrap(),
                    world.driver.pulser().current().len(),
                    v["pulses"].as_i64().unwrap(),
                ));
            }
            "close" => {
                let session = world
                    .binding
                    .inner
                    .sessions
                    .iter()
                    .position(|s| s.id == v["session"])
                    .unwrap();
                let driver = world.driver.clone();
                driver.close(&mut world, &session).unwrap();
                assert!(!record.out.contains_key(v["session"].as_str().unwrap()));
            }
            other => panic!("{other}"),
        }
        // The harness pumps after every step: the dispatcher's blocks.
        let blocks = world.pump();
        let originals: Vec<(String, Vec<u8>)> = v["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| Some((f["path"].as_str()?.to_owned(), unhex(f["hex"].as_str()?))))
            .collect();
        assert_eq!(
            world.written.len(),
            originals.len(),
            "{step}: files written {:?}",
            world.written
        );
        for (path, original) in originals {
            let native = std::fs::read(directory.join(&path)).unwrap();
            out.saves
                .push((format!("{} {path}", scenario.name), native, original));
        }
        let events: Vec<String> = world
            .binding
            .social
            .pending()
            .skip(events_before)
            .map(event_name)
            .collect();
        let original: Vec<String> =
            serde_json::from_value(v["handoffs"]["events"].clone()).unwrap();
        out.events.push((step.clone(), events, original));
        if v["op"] != "recv" {
            // At the recv the original also ran the logins' SendContactInfo
            // block (a handler that is not joined); see the parent module.
            assert!(
                blocks as i64 <= v["handoffs"]["blocks"].as_i64().unwrap(),
                "{step}"
            );
        }
        out.states
            .push((step, character_rows(&world), original_rows(v)));
    }
    out.stand_ins = world.stand_ins.clone();
    out.logs = world.logs.clone();
    out.refusals = world.driver.refusals().len();
    let _ = std::fs::remove_dir_all(&directory);
    out
}

/// Instead of the scenario's close: native ticks, without client input and
/// without originals to compare, until `GameWorld.ticks` reaches 50. The
/// world-trace scenarios end before the 50-tick autosave; this checks the
/// native hand-off and what it saves, not the original.
fn tick_to_autosave(world: &mut World) -> (i32, Vec<String>) {
    loop {
        NOW.with(|n| n.fetch_add(600, Ordering::SeqCst));
        world.binding.inner.ticks = world.driver.ticks().get();
        world.binding.inner.queue.clear();
        world.binding.inner.expected.clear();
        world.expected_direct.clear();
        world.inserted = 0;
        for session in &mut world.binding.inner.sessions {
            session.direct.clear();
        }
        let driver = world.driver.clone();
        driver.tick(world, false).unwrap();
        let handed: Vec<i32> = world
            .tasks
            .iter()
            .filter_map(|t| match t {
                Task::Autosave(a) => Some(a.ticks),
                Task::Save(_) => None,
            })
            .collect();
        world.written.clear();
        world.pump();
        let ticks = world.driver.ticks().get();
        if ticks % driver::AUTOSAVE_INTERVAL == 0 {
            assert_eq!(handed, vec![ticks]);
            return (ticks, world.written.clone());
        }
        assert!(
            handed.is_empty() && world.written.is_empty(),
            "tick {ticks}"
        );
    }
}

// ------------------------------------------------ pins

/// The entries replayed for systems this test does not run, by marker,
/// per scenario and session, beyond the login's own segments (pinned by the
/// parent module's `expected_segments`): the NPC updates (one per player
/// update), the chunk updates of the first update after login, what
/// `Player.tick` queued (walking's region move and its music, twice) and
/// the content world pulses (tutorial island's `TutorialLoginCheck` and
/// the stage dialogue a `runTask` opens ten ticks later).
const TICK_SEGMENTS: &[(&str, usize)] = &[
    ("MapChunkRenderer.render", 2),
    ("core.game.node.entity.player.Player.tick:", 4),
    ("core.game.world.PulseRunner.updateAll:", 37),
];

/// (scenario session, entries compared, natively written and equal, NPC
/// updates replayed, login tick, extra login listener). The native count
/// includes the login's player-tab systems (world/player_tabs: 76 entries
/// for a new character, 52 for the returning one).
#[allow(clippy::type_complexity)]
const SESSIONS: &[(&str, usize, usize, usize, i64, bool)] = &[
    ("new-character traceone", 482, 303, 21, 1, false),
    ("two-characters traceone", 482, 300, 21, 1, false),
    ("two-characters tracetwo", 485, 304, 22, 6, true),
    ("returning-character traceone", 458, 279, 21, 1, false),
];

/// Natively written entries that differ from the original's, all player
/// updates (225), by (scenario session, tick, entry):
/// - the first update after the login tick (entry 1, after the rebuild):
///   the original's carries the appearance block `updateAppearance`
///   registered at login; update masks and the appearance writer are not
///   joined in this binding, so the native one has no mask flag;
/// - the update after that: tutorial content (`TutorialStage.load`, not
///   ported) teleported the player, so the original's is a teleport with
///   the appearance block, the native one an idle update;
/// - traceone's updates at ticks 6 to 8 of two-characters: the original
///   adds tracetwo with its appearance and then sends tracetwo's masks
///   (placement, teleport); natively tracetwo is added without masks.
const DIFFER: &[(&str, &[(i64, usize)])] = &[
    ("new-character traceone", &[(2, 1), (3, 0)]),
    (
        "two-characters traceone",
        &[(2, 1), (3, 0), (6, 0), (7, 0), (8, 0)],
    ),
    ("two-characters tracetwo", &[(7, 1), (8, 0)]),
    ("returning-character traceone", &[(2, 1), (3, 0)]),
];

/// Every kind of call to a system this test does not run, besides the
/// replays, with the number of calls per scenario.
const STAND_INS: &[&str] = &[
    "ChunkUpdateTracker.resetAll (chunk sync not joined)",
    "DeathTask.isDead (not joined: false)",
    "Entity.clear chunk/tolerances/pulseManager (not joined)",
    "FamiliarManager.hasFamiliar (not ported: false)",
    "GameWorld.tickListeners (content, not ported)",
    "GroundItemManager.pulse (ground items not joined)",
    "HouseManager.leave (not ported)",
    "Managers.tick (manager plugins, not ported)",
    "Player.logoutListeners (content, none natively)",
    "Player.tick (Entity.tick, music, prayer, wealth: not joined)",
    "WalkingQueue.reset (walking not joined)",
    "ZoneMonitor.clear (zones not joined)",
    "checkForWealthUpdate (not ported)",
    "logout listener content.global.skill.hunter.HunterManager",
    "logout listener content.minigame.greatorbproject.OrbProjMinigame",
    "logout listener content.minigame.pyramidplunder.PyramidPlunderMinigame",
    "logout listener content.region.kandarin.quest.dwarfcannon.dmc.DMCHandler",
    "logout listener rs09.game.content.activity.castlewars.areas.CastleWarsGameArea",
    "logout listener rs09.game.content.activity.castlewars.areas.CastleWarsRespawnArea",
    "logout listener rs09.game.content.activity.castlewars.areas.CastleWarsWaitingArea",
    "scripts.hasTypeInQueue(SOFT) (not joined: false)",
];

fn verbose() -> bool {
    std::env::var_os("RS09_TICK_DRIVER_VERBOSE").is_some()
}

#[test]
fn tick_driver_runs_the_world_trace_scenarios() {
    let scenarios = scenarios();
    let mut sessions = 0;
    for name in ["new-character", "two-characters", "returning-character"] {
        let scenario = scenarios.iter().find(|s| s.name == name).unwrap();
        let seed = if name == "returning-character" {
            seed_for(&scenarios, "new-character")
        } else {
            Vec::new()
        };
        let driven = drive(scenario, &seed, false);
        let ticks = scenario
            .records
            .iter()
            .filter(|r| r.value["t"] == "tick")
            .count();
        // Every session's entries, step by step.
        for (session, tally) in &driven.tallies {
            if verbose() {
                eprintln!(
                    "{session}: {} entries, {} native, replayed {:?}, differ {:?}",
                    tally.entries, tally.native, tally.replayed, tally.differ
                );
            }
            let pin = SESSIONS
                .iter()
                .find(|p| p.0 == session)
                .unwrap_or_else(|| panic!("no pin for {session}"));
            assert_eq!(tally.entries, pin.1, "{session}: entries");
            assert_eq!(tally.native, pin.2, "{session}: natively written and equal");
            let mut expected: BTreeMap<String, usize> = BTreeMap::new();
            for (marker, n) in expected_segments(pin.5) {
                *expected.entry(marker).or_insert(0) += n;
            }
            for (marker, n) in TICK_SEGMENTS {
                expected.insert(marker.to_string(), *n);
            }
            expected.insert("NPCRenderer.render".into(), pin.3);
            assert_eq!(tally.replayed, expected, "{session}: replayed segments");
            let differ: Vec<(i64, usize)> = tally
                .differ
                .iter()
                .map(|(tick, entry, op)| {
                    assert_eq!(*op, Some(225), "{session}: tick {tick} entry {entry}");
                    (*tick, *entry)
                })
                .collect();
            let pinned = DIFFER.iter().find(|d| d.0 == session).unwrap().1;
            assert_eq!(differ, pinned, "{session}: differing entries");
            // The first player update is the tick after the login tick.
            assert_eq!(differ[0], (pin.4 + 1, 1));
            assert_eq!(
                tally.entries,
                tally.native + tally.differ.len() + tally.replayed.values().sum::<usize>()
            );
            sessions += 1;
        }
        // The repository's characters, the lobby and the tick counter after
        // every step but the handshakes.
        let steps = match name {
            "two-characters" => 37,
            _ => 29,
        };
        assert_eq!(driven.states.len(), steps, "{name}");
        for (step, native, original) in &driven.states {
            assert_eq!(native, original, "{name} {step}");
        }
        // The management events published: equal, except that at each
        // login the original's RequestContactInfo handler (not joined) also
        // published SendContactInfo.
        for (step, native, original) in &driven.events {
            if step.starts_with("recv") && !original.is_empty() {
                assert_eq!(native, &["PlayerStatusUpdate", "RequestContactInfo"]);
                assert_eq!(
                    original,
                    &[
                        "PlayerStatusUpdate",
                        "RequestContactInfo",
                        "SendContactInfo"
                    ]
                );
            } else {
                assert_eq!(native, original, "{name} {step}");
            }
        }
        // The save each logout writes, against the original's file of that
        // step: the top-level keys the parent module pins for the login
        // (content hooks, content attributes and the wealth check, replayed
        // emote and music state, the AntiMacro timer, varp 442, the
        // loot-share delay) and nothing else.
        let characters = if name == "two-characters" { 2 } else { 1 };
        assert_eq!(driven.saves.len(), characters);
        for (path, native, original) in &driven.saves {
            let native = String::from_utf8(native.clone()).unwrap();
            let original = String::from_utf8(original.clone()).unwrap();
            let pinned = if name == "returning-character" {
                RETURNING_SAVE
            } else {
                NEW_SAVE
            };
            assert_eq!(differing_keys(&native, &original), pinned, "{path}");
        }
        // The native pulse runner holds only the login pulses: empty after
        // every tick. The original's queue (14 to 50 pulses) is content.
        assert_eq!(driven.pulses.len(), ticks);
        for (tick, native, original) in &driven.pulses {
            assert_eq!(*native, 0, "{name} tick {tick}");
            assert!(*original >= 14, "{name} tick {tick}");
        }
        assert_eq!(
            driven.stand_ins.keys().collect::<Vec<_>>(),
            STAND_INS.iter().collect::<Vec<_>>(),
            "{name}: stand-in kinds"
        );
        for kind in [
            "ChunkUpdateTracker.resetAll (chunk sync not joined)",
            "GameWorld.tickListeners (content, not ported)",
            "GroundItemManager.pulse (ground items not joined)",
            "Managers.tick (manager plugins, not ported)",
        ] {
            assert_eq!(driven.stand_ins[kind], ticks, "{name}: {kind}");
        }
        for (kind, n) in &driven.stand_ins {
            if !kind.contains("Player.tick") {
                assert!(*n == ticks || *n == characters, "{name}: {kind} {n}");
            }
        }
        // Nothing was refused, nothing was printed as an error; the queue
        // logged each logout.
        assert_eq!(driven.refusals, 0);
        let mut logs = Vec::new();
        for character in if characters == 2 {
            vec!["traceone", "tracetwo"]
        } else {
            vec!["traceone"]
        } {
            logs.push(format!("Info Queueing {character} for disconnection."));
            logs.push(format!("Info Player cleared. Removed {character}."));
            logs.push("thread PlayerSave SQL".to_string());
        }
        assert_eq!(driven.logs, logs, "{name}");
    }
    assert_eq!(sessions, 4);
}

/// Not in the world trace (it ends before tick 50): the driver hands the
/// autosave off when the counter reaches 50 and not before, and the block
/// saves the playing character natively.
#[test]
fn tick_driver_hands_off_the_autosave_at_tick_fifty() {
    let scenarios = scenarios();
    let scenario = scenarios
        .iter()
        .find(|s| s.name == "new-character")
        .unwrap();
    let driven = drive(scenario, &[], true);
    let (ticks, written) = driven.autosave.expect("an autosave");
    assert_eq!(ticks, 50);
    assert_eq!(written, ["data/players/traceone.json Written"]);
}
