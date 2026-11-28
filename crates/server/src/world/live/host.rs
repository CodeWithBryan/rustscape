//! The production hosts of the live world: [`repository::Host`],
//! [`login::Host`] and [`driver::Host`] over one [`Live`] world.
//!
//! The shape is the test bindings' (`tests/world_login.rs`,
//! `tests/world_login/tick_driver.rs`): the canonical player's libraries
//! answer what they own; walking, masks and the scene are [`super::walk`];
//! every call into a system without a port is a named answer of
//! [`super::not_ported`].
use super::{
    P, SessionId,
    inner::{
        Canonical, EquipmentContext, Inner, OptionsContext, SettingsContext, SkillsContext, Slot,
        native, placeholder_skills,
    },
    now_millis,
    tabs::Tabs,
    walk::{self, Deferred, Walk, WalkHost},
};
use crate::world::player_tabs::emotes as tab_emotes;
use crate::{
    definitions::item::Shared,
    inventory::{
        container::{ContainerType, SortType},
        container_listener::Handle as ListenerHandle,
        equipment,
        listener::Listener as InventoryListener,
        player_binding::Capture,
    },
    player::{
        attributes::AttributeValue,
        details::PlayerDetails,
        owned::{
            Player,
            profile::{Fresh, Profile},
        },
        save::Parser,
        settings::Settings,
        variables,
    },
    pulses,
    world::{
        appearance,
        disconnection::{self, SaveTask},
        doors::Location,
        driver::{self, Autosave, Driver, Level as DriverLevel, Metric, dispatch},
        interaction_options, interfaces,
        login::{self, Constants, Level, Write, pulse::LoginPulse},
        player_render,
        player_walk::{self, Node},
        prepared_masks::PreparedMasks,
        rebuild,
        repository::{self, LogLevel, Repository, SessionObject},
        social::{self, ClanId},
    },
};
use rs09_protocol::{
    game::{Frame, updates},
    login::AuthResponse,
};
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

/// An outgoing packet of the driver's write queue: its session, class and
/// frame, and the session-queue position it was pushed at.
#[derive(Clone, Debug)]
pub struct Sent {
    pub session: SessionId,
    pub class: &'static str,
    pub frame: Frame,
    pub at: usize,
}

/// `TaskExecutor`'s hand-offs, run after the tick.
pub enum Task {
    Save(SaveTask<P>),
    Autosave(Autosave),
}

/// A save file for the saver thread.
pub struct SaveJob {
    pub path: std::path::PathBuf,
    pub text: String,
}

/// The live world.
pub struct Live {
    pub inner: Inner,
    pub social: social::Social<P>,
    pub driver: Driver<P, Sent>,
    pub repository: Repository<P>,
    pub walk: Walk,
    pub login_listeners: Vec<String>,
    pub logout_listeners: Vec<String>,
    pub login_plugins: Vec<String>,
    pub logins: Vec<(pulses::Handle, LoginPulse<P, Parser, SessionId>)>,
    pub tasks: VecDeque<Task>,
    pub saves: std::sync::mpsc::Sender<SaveJob>,
    pub inserted: usize,
    pub deferred: Vec<Deferred>,
    pub online: Arc<Mutex<HashSet<String>>>,
    pub saved: u64,
    /// Entity regions, zone monitors and viewports ([`super::regions`]).
    pub regions: super::regions::Regions,
    /// The NPC renderer's view of each NPC, by node ([`super::npcs`]).
    pub npc_views: std::collections::HashMap<Node, crate::world::npc_render::Handle>,
    /// The global handlers' state: their submitted pulses, login plugins,
    /// configuration and ladders ([`super::global`]).
    pub global: super::global::GlobalState,
    /// `InterfaceListeners` of the installed content ([`super::buttons`]).
    pub listeners: crate::world::interface_listeners::Registry,
    /// The routed item, NPC and player packet handlers ([`super::items`]).
    pub item_actions: super::items::ItemActions,
    /// Region zones and the players' SkullManagers ([`super::region_zones`]).
    pub zones: super::region_zones::RegionZones,
    /// Runtime NPCs, graves, hint icons and the systems installed after
    /// combat ([`super::content`]).
    pub content: super::content::ContentWorld,
    /// The install round after the dragons: dialogue, input prompts, the
    /// bank, shops, trade, follow ([super::round]).
    pub round: super::round::Round,
    /// NPCs whose tick is failing ([`super::faults`]).
    pub npc_faults: super::faults::EntityFaults,
    /// NPCs whose reset is failing, tracked apart from their ticks so that a
    /// reset that succeeds does not end a tick's run of failures.
    pub npc_reset_faults: super::faults::EntityFaults,
    /// NPCs whose `Entity.tick` reached `timers.processTimers()` in the
    /// walking call that just returned ([`super::timers`]).
    pub npc_timers_due: Vec<Node>,
    /// The stand-in the world holds while a player's tick has the canonical
    /// state ([`super::scripts`]).
    pub spare_state: Option<player_walk::State>,
}

pub fn fail(e: impl std::fmt::Debug) -> driver::Error {
    driver::Error::Exception(format!("{e:?}"))
}
fn rfail(e: impl std::fmt::Debug) -> repository::Error {
    repository::Error::Exception(format!("{e:?}"))
}

impl Live {
    /// Runs a walking operation on the canonical state, then what it
    /// deferred to the interface context.
    pub fn with_walk<T>(
        &mut self,
        f: impl FnOnce(&mut player_walk::World<'_, WalkHost<'_>>) -> T,
    ) -> T {
        self.with_walk_host(|state, host| f(&mut state.context(host)))
    }
    /// The canonical state and the walk services, then what they deferred.
    pub fn with_walk_host<T>(
        &mut self,
        f: impl FnOnce(&mut player_walk::State, &mut WalkHost<'_>) -> T,
    ) -> T {
        let Inner {
            state,
            frames,
            players,
            not_ported,
            npcs,
            generators,
            combat,
            ..
        } = &mut self.inner;
        let mut lines = Vec::new();
        let mut host = WalkHost {
            walk: &mut self.walk,
            frames,
            players,
            not_ported,
            deferred: &mut self.deferred,
            log: &mut lines,
            npcs,
            random: generators,
            npc_views: Some(&self.npc_views),
            combat,
        };
        let result = f(state, &mut host);
        self.after_walk(lines);
        result
    }
    /// After a walking call: its log lines, the frames it queued and what
    /// its services deferred.
    pub(super) fn after_walk(&mut self, lines: Vec<String>) {
        for line in lines {
            self.inner.log_line(line);
        }
        for node in self.inner.frames.keys().copied().collect::<Vec<_>>() {
            if let Some(p) = self.player_of(node) {
                self.inner.drain(&p);
            }
        }
        self.run_deferred();
    }
    fn run_deferred(&mut self) {
        for deferred in std::mem::take(&mut self.deferred) {
            let result = match deferred {
                Deferred::CloseInterfaces(node) => self
                    .player_of(node)
                    .map(|p| self.close_interfaces_of(&p).map(|_| ())),
                Deferred::PlayerHandler(call) => {
                    self.run_player_handler(call);
                    None
                }
                Deferred::Follow(option) => {
                    self.follow_arrival(&option);
                    None
                }
                Deferred::CloseChatbox(node) => self.player_of(node).map(|p| {
                    let (ui, manager) = self.ui_of(&p);
                    interfaces::close_chatbox(ui, &manager)
                }),
                Deferred::RunToggled(node, value) => self.player_of(node).map(|p| {
                    self.inner
                        .canonical(&p)
                        .player
                        .settings()
                        .fields
                        .write()
                        .run_toggled = value;
                    self.inner
                        .variables(&p, |ctx, owner| {
                            variables::set_varp(ctx, Some(owner), 173, i32::from(value), false)
                        })
                        .map_err(|e| interfaces::Error::Binding(e.to_string()))
                }),
                Deferred::RemoveRenderable(node) => {
                    let repository = self.repository.clone();
                    repository.remove_renderable_npc(self, &P(node));
                    None
                }
                Deferred::Handler(call) => {
                    self.run_handler_calls(vec![call]);
                    None
                }
                Deferred::RegionMove(node, from, to) => Some(
                    self.region_move(&node, from, to)
                        .map_err(|e| interfaces::Error::Binding(format!("{e:?}"))),
                ),
                Deferred::SelectTarget(node) => self.combat_entity(node).map(|npc| {
                    self.npc_select_target(&npc)
                        .map(|_| ())
                        .map_err(|e| interfaces::Error::Binding(format!("{e:?}")))
                }),
                Deferred::ImpactDisabled(node, ticks) => self.combat_entity(node).map(|npc| {
                    let result =
                        crate::world::combat_pulse::impact::set_disabled_ticks(self, &npc, ticks)
                            .map_err(|e| interfaces::Error::Binding(format!("{e:?}")));
                    self.sync_combat();
                    result
                }),
                Deferred::Listener(option) => Some(
                    self.listener_arrival(&option)
                        .map_err(|e| interfaces::Error::Binding(format!("{e:?}"))),
                ),
                Deferred::SceneryListener {
                    player,
                    id,
                    option,
                    object,
                } => Some(
                    self.scenery_listener_run(player, id, &option, object)
                        .map_err(|e| interfaces::Error::Binding(format!("{e:?}"))),
                ),
                Deferred::CloseDialogue(node) => self.player_of(node).map(|p| {
                    self.close_dialogue_of(&p)
                        .map(|_| ())
                        .map_err(|e| interfaces::Error::Binding(format!("{e:?}")))
                }),
                Deferred::NpcRestoreTimers(node) => self.combat_entity(node).map(|npc| {
                    self.remove_npc_timer(&npc, "poison")
                        .and_then(|_| self.remove_npc_timer(&npc, "poison:immunity"))
                        .map_err(|e| interfaces::Error::Binding(format!("{e:?}")))
                }),
                Deferred::NpcTimers(node) => {
                    self.npc_timers_due.push(node);
                    None
                }
                Deferred::ContentStop(pulse) => Some(
                    self.content_pulse_stopped(&pulse)
                        .map_err(|e| interfaces::Error::Binding(format!("{e:?}"))),
                ),
                Deferred::CombatStop(node) => Some(
                    self.combat_stop_of(node)
                        .map_err(|e| interfaces::Error::Binding(format!("{e:?}"))),
                ),
            };
            if let Some(Err(error)) = result {
                self.inner
                    .log_line(format!("deferred walk service failed: {error:?}"));
            }
        }
    }
    /// `NPC.tick()` with a subclass's override, as the original runs it.
    pub fn npc_tick_unisolated(&mut self, npc: &P) -> driver::Result<()> {
        // A subclass whose tick replaces NPC.tick (the grave's Grave.tick).
        if let Some(result) = self.grave_npc_tick(npc) {
            return result.map_err(fail);
        }
        // NPC.tick() with a subclass's override (super::subclasses).
        self.npc_class_tick(npc).map_err(fail)
    }
    /// Records an NPC step's outcome against the NPC and logs a failure
    /// (rate limited, super::faults); the failure is not passed on.
    fn isolate_npc(&mut self, npc: &P, step: &str, result: driver::Result<()>) {
        let node = npc.node();
        let tick = self.inner.ticks;
        let id = self.inner.npcs.rows.get(&node).map(|row| row.id);
        let location = self.inner.state.node_location(node).ok().flatten();
        let Self {
            inner,
            npc_faults,
            npc_reset_faults,
            ..
        } = self;
        let faults = if step == "reset" {
            npc_reset_faults
        } else {
            npc_faults
        };
        super::faults::isolate(
            faults,
            node,
            tick,
            || {
                let id = id.map_or_else(|| "?".to_owned(), |id| id.to_string());
                let at = location.map_or_else(
                    || "nowhere".to_owned(),
                    |l| format!("{:?}", l.coordinates()),
                );
                format!("NPC {id} (node {node}) at {at}: {step}")
            },
            result,
            |line| {
                eprintln!("[live] {line}");
                inner.log_line(line);
            },
        );
    }
    pub fn player_of(&self, node: Node) -> Option<P> {
        self.inner
            .players
            .get(&node)
            .map(|s| P(s.canonical.owner.clone()))
    }
    fn ui_of(&mut self, p: &P) -> (&mut Inner, interfaces::Manager) {
        self.inner.current = Some(p.clone());
        let manager = self.inner.canonical(p).player.manager();
        (&mut self.inner, manager)
    }
    fn render(&self, p: &P) -> player_render::Handle {
        self.inner.canonical(p).render.clone()
    }
}

// ---------------------------------------------------- repository::Host

impl repository::Host<P> for Live {
    type Session = SessionId;
    type Clan = ClanId;
    type Location = ();
    fn uid(&mut self, p: &P) -> i32 {
        self.inner.slot(p).uid
    }
    fn name(&mut self, p: &P) -> Option<String> {
        Some(self.inner.slot(p).name.clone())
    }
    fn details_username(&mut self, p: &P) -> Option<String> {
        Some(self.inner.slot(p).name.clone())
    }
    fn set_index(&mut self, p: &P, index: i32) {
        let node = p.node();
        let slot = self.inner.slot_mut(p);
        slot.index = index;
        slot.canonical.render.write().index = index;
        if let Ok(actor) = self.inner.state.actor_mut(node) {
            actor.wire_index = index;
        }
    }
    fn is_artificial(&mut self, _: &P) -> bool {
        false
    }
    fn afk_logout(&mut self, p: &P) -> Option<bool> {
        self.inner.slot(p).afk_logout
    }
    fn set_playing(&mut self, p: &P, playing: bool) {
        self.inner.slot_mut(p).playing = playing;
    }
    fn session(&mut self, p: &P) -> Option<SessionId> {
        self.inner.slot(p).session
    }
    fn ticks(&mut self) -> i32 {
        self.driver.ticks().get()
    }
    fn global_chat_enabled(&mut self) -> bool {
        self.inner.settings.global_chat
    }
    fn to_lower_case(&mut self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn session_active(&mut self, s: &SessionId) -> bool {
        self.inner.sessions.get(s).is_some_and(|s| s.active)
    }
    fn set_session_active(&mut self, s: &SessionId, active: bool) {
        if let Some(s) = self.inner.sessions.get_mut(s) {
            s.active = active;
        }
    }
    fn session_has_key(&mut self, _: &SessionId) -> bool {
        // The transport's sessions are not NIO selection keys.
        false
    }
    fn key_cancel(&mut self, _: &SessionId) -> repository::Result<()> {
        Ok(())
    }
    fn socket_close(&mut self, s: &SessionId) -> repository::Result<()> {
        if let Some(s) = self.inner.sessions.get_mut(s) {
            s.closing = true;
        }
        Ok(())
    }
    fn session_object(&mut self, s: &SessionId) -> SessionObject<P> {
        self.inner
            .sessions
            .get(s)
            .and_then(|s| s.object.clone())
            .map_or(SessionObject::Null, SessionObject::Player)
    }
    fn set_session_object(&mut self, s: &SessionId, p: Option<P>) {
        if let Some(s) = self.inner.sessions.get_mut(s) {
            s.object = p;
        }
    }
    fn session_username(&mut self, s: &SessionId) -> Option<String> {
        self.inner.sessions.get(s).and_then(|s| s.username.clone())
    }
    fn player_clear(&mut self, repository: &Repository<P>, p: &P) -> repository::Result<()> {
        disconnection::player_clear(self, repository, p)
    }
    fn renderable_players_remove(&mut self, p: &P) -> repository::Result<bool> {
        Ok(self.driver.renderable_players().remove(p))
    }
    fn in_combat(&mut self, p: &P) -> repository::Result<bool> {
        let time = match self
            .inner
            .canonical(p)
            .player
            .attributes()
            .get("combat-time")
        {
            None => 0,
            Some(AttributeValue::Long(v)) => v,
            Some(other) => {
                return Err(repository::Error::Exception(format!(
                    "ClassCastException: combat-time is {}",
                    other.java_string()
                )));
            }
        };
        Ok(time > now_millis())
    }
    fn lifepoints(&mut self, p: &P) -> repository::Result<i32> {
        self.inner
            .with_profile(p, |profile| profile.skills.lifepoints)
            .map_err(rfail)
    }
    fn is_dead(&mut self, _: &P) -> repository::Result<bool> {
        // DeathTask.isDead: no death is ever started (combat is not bound).
        self.inner
            .not_ported
            .default_answer("DeathTask.isDead (no death started in this world: false)");
        Ok(false)
    }
    fn is_teleporting(&mut self, p: &P) -> repository::Result<bool> {
        Ok(self.render(p).read().teleporting)
    }
    fn has_soft_script(&mut self, p: &P) -> repository::Result<bool> {
        Ok(self
            .inner
            .canonical(p)
            .player
            .scripts()
            .has_type(crate::scripts::Strength::Soft))
    }
    fn logout_packet(&mut self, p: &P) -> repository::Result<()> {
        let Some(session) = self.inner.session_of(p) else {
            return Err(repository::Error::Exception(
                "NullPointerException: session".into(),
            ));
        };
        let packet = Sent {
            session,
            class: "Logout",
            frame: Frame {
                opcode: 86,
                payload: vec![],
            },
            at: 0,
        };
        let driver = self.driver.clone();
        driver
            .send(self, "Logout", Some(packet))
            .map(|_| ())
            .map_err(repository::Error::from)
    }
    fn world_logout_listeners(&mut self, _: &P) -> repository::Result<()> {
        for _ in 0..self.logout_listeners.len() {
            self.inner
                .not_ported
                .skip("GameWorld.logoutListeners (content listeners by class name)");
        }
        Ok(())
    }
    fn walking_queue_reset(&mut self, _: &P) -> repository::Result<()> {
        // WalkingQueue.reset at logout: the queue's reset needs player_walk's
        // walking context, which it does not expose; the player is removed
        // and its movement pulses are cleared by Entity.clear below.
        self.inner
            .not_ported
            .skip("WalkingQueue.reset at logout (player_walk does not expose it)");
        Ok(())
    }
    fn player_logout_listeners(&mut self, _: &P) -> repository::Result<()> {
        self.inner
            .not_ported
            .default_answer("Player.logoutListeners (content adds none: empty)");
        Ok(())
    }
    fn has_familiar(&mut self, _: &P) -> repository::Result<bool> {
        self.inner
            .not_ported
            .default_answer("FamiliarManager.hasFamiliar (not ported: false)");
        Ok(false)
    }
    fn familiar_clear(&mut self, _: &P) -> repository::Result<()> {
        Err(repository::Error::Exception("not ported: familiars".into()))
    }
    fn interface_close(&mut self, p: &P) -> repository::Result<()> {
        let (ui, manager) = self.ui_of(p);
        interfaces::close(ui, &manager).map(|_| ()).map_err(rfail)
    }
    fn interface_close_single_tab(&mut self, p: &P) -> repository::Result<()> {
        let (ui, manager) = self.ui_of(p);
        interfaces::close_single_tab(ui, &manager)
            .map(|_| ())
            .map_err(rfail)
    }
    fn entity_clear(&mut self, p: &P) -> repository::Result<()> {
        // Entity.clear: active = false, the chunk's player list and the
        // region's tolerance, then the pulse manager (the canonical actor's
        // movement pulses).
        self.inner.slot_mut(p).active = false;
        self.render(p).write().active = false;
        self.region_clear(&p.0).map_err(rfail)?;
        let node = p.node();
        self.with_walk(|world| world.disconnect(node))
            .map_err(rfail)?;
        self.walk.maps.players.remove(&node);
        Ok(())
    }
    fn zone_monitor_clear(&mut self, p: &P) -> repository::Result<()> {
        // ZoneMonitor.clear: the player's zones and music zones.
        let monitor = self.monitor(&p.0);
        monitor.clear(self).map(|_| ()).map_err(rfail)
    }
    fn house_leave(&mut self, _: &P) -> repository::Result<()> {
        self.inner
            .not_ported
            .skip("HouseManager.leave (construction not ported)");
        Ok(())
    }
    fn publish_player_status(
        &mut self,
        username: &str,
        world: i32,
        notify_friends_only: bool,
    ) -> repository::Result<()> {
        self.social.publish(social::Event::PlayerStatusUpdate {
            username: username.to_owned(),
            world,
            notify_friends_only,
        });
        Ok(())
    }
    fn clan(&mut self, p: &P) -> repository::Result<Option<ClanId>> {
        Ok(self.social.clan_of(p))
    }
    fn world_id(&mut self) -> repository::Result<i32> {
        Ok(self.inner.constants.world_id)
    }
    fn clan_owner(&mut self, clan: &ClanId) -> repository::Result<Option<String>> {
        Ok(Some(self.social.clan(*clan).owner.clone()))
    }
    fn publish_clan_leave(
        &mut self,
        username: &str,
        world: i32,
        clan_name: &str,
    ) -> repository::Result<()> {
        self.social.publish(social::Event::ClanLeaveNotification {
            username: username.to_owned(),
            clan_name: clan_name.to_owned(),
            world,
        });
        Ok(())
    }
    fn wealth_update(&mut self, _: &P, _: bool) -> repository::Result<()> {
        self.inner
            .not_ported
            .skip("checkForWealthUpdate (wealth tracking not ported)");
        Ok(())
    }
    fn clan_leave(&mut self, clan: &ClanId, p: &P, remove: bool) -> repository::Result<()> {
        self.social
            .clan_leave_default(&mut self.inner, *clan, p, remove)
            .map_err(rfail)
    }
    fn details_save(&mut self, p: &P) -> repository::Result<()> {
        let storage = self.inner.storage.clone();
        self.inner
            .with_profile(p, |profile| profile.details.save(&*storage))
            .map_err(rfail)?
            .map_err(rfail)
    }
    fn execute_sql(&mut self, task: SaveTask<P>) -> repository::Result<()> {
        self.tasks.push_back(Task::Save(task));
        Ok(())
    }
    fn set_thread_name(&mut self, _: &str) {}
    fn save_immediately(&mut self, p: &P) -> repository::Result<()> {
        let location = self.inner.slot(p).location.clone();
        let (parsed, name) = self
            .inner
            .with_profile(p, |profile| {
                (profile.details.save_parsed, profile.name.clone())
            })
            .map_err(rfail)?;
        if !parsed {
            return Ok(());
        }
        let text = self
            .inner
            .environment_run(p, |player, state, env| {
                player.save_text(state, Some(&location), env)
            })
            .map_err(rfail)?;
        let path = std::path::PathBuf::from(format!("{}{name}.json", self.inner.save_path));
        self.saves
            .send(SaveJob { path, text })
            .map_err(|_| repository::Error::Exception("the saver thread stopped".into()))?;
        self.saved += 1;
        Ok(())
    }
    fn log(&mut self, level: LogLevel, message: &str) -> repository::Result<()> {
        self.inner.log_line(format!("{level:?} {message}"));
        Ok(())
    }
    fn print_stack_trace(&mut self, error: &repository::Error) {
        self.inner.log_line(format!("printStackTrace {error:?}"));
    }
    fn is_revenant(&mut self, _: &P) -> bool {
        false
    }
    fn set_renderable(&mut self, p: &P, renderable: bool) {
        if let Some(slot) = self.inner.players.get_mut(&p.node()) {
            slot.renderable = renderable;
        }
    }
    fn npc_id(&mut self, npc: &P) -> i32 {
        self.inner
            .npcs
            .rows
            .get(&npc.node())
            .map_or(-1, |row| row.id)
    }
    fn region_chunk_npcs(&mut self, _: &()) -> repository::Result<Vec<P>> {
        Err(repository::Error::Exception(
            "not ported: NPCs are not joined to the live repository".into(),
        ))
    }
    fn npc_at(&mut self, _: &P, _: &()) -> bool {
        false
    }
    fn ge_news_muted(&mut self, p: &P) -> repository::Result<bool> {
        self.inner
            .canonical(p)
            .player
            .attributes()
            .boolean(repository::GE_NEWS_MUTE_ATTRIBUTE, false)
            .map_err(rfail)
    }
    fn send_message(&mut self, p: &P, message: &str) -> repository::Result<()> {
        self.inner.message(p, message).map_err(rfail)
    }
}

// ---------------------------------------------------- login::Host

impl login::Host<P> for Live {
    type Ui = Inner;
    type Parser = Parser;
    type Social = Inner;
    fn now_millis(&mut self) -> i64 {
        (self.inner.clock)()
    }
    fn constants(&mut self) -> Constants {
        self.inner.constants.clone()
    }
    fn random_seed(&mut self) -> i64 {
        now_millis() ^ 0x0005_DEEC_E66D
    }
    fn session_address(&mut self, s: &SessionId) -> Option<String> {
        self.inner.sessions.get(s).and_then(|s| s.address.clone())
    }
    fn server_key(&mut self, _: &SessionId) -> i64 {
        // The transport writes its own reply with the handshake's key.
        0
    }
    fn write_bytes(&mut self, s: &SessionId, bytes: Vec<u8>, _: Write) -> login::Result<()> {
        let Some(session) = self.inner.sessions.get_mut(s) else {
            return Ok(());
        };
        let Some(pending) = session.link.pending.take() else {
            // A reply after the admission: the transport has already
            // answered this connection; the original writes it into the
            // game stream, which the transport cannot frame. Logged.
            self.inner
                .log_line(format!("login reply {bytes:?} after admission: dropped"));
            return Ok(());
        };
        let response = AuthResponse::from_byte(bytes[0]).unwrap_or(AuthResponse::UnexpectedError);
        let result = if response == AuthResponse::Success {
            let rights = i32::from(bytes[1]);
            let index = (i32::from(bytes[8]) << 8) | i32::from(bytes[9]);
            // The transport writes the same reply from these two fields.
            debug_assert_eq!(
                rs09_protocol::login::reply(AuthResponse::Success, rights, index, 0),
                bytes
            );
            Ok(crate::world::session::Joined {
                rights,
                index,
                incoming: pending.incoming,
                outgoing: pending.outgoing,
            })
        } else {
            Err(response)
        };
        let _ = pending.reply.send(result);
        Ok(())
    }
    fn set_game_producer(&mut self, s: &SessionId) {
        if let Some(s) = self.inner.sessions.get_mut(s) {
            s.game = true;
        }
    }
    fn set_last_ping(&mut self, s: &SessionId, millis: i64) {
        if let Some(s) = self.inner.sessions.get_mut(s) {
            s.last_ping = millis;
        }
    }
    fn flagged_ips(&mut self) -> login::Result<Vec<String>> {
        self.inner
            .not_ported
            .default_answer("ServerStore flagged-ips (no archive: empty list)");
        Ok(Vec::new())
    }
    fn post_player_alert(&mut self, _: &str, _: &str) -> login::Result<()> {
        self.inner
            .not_ported
            .skip("Discord.postPlayerAlert (no Discord integration)");
        Ok(())
    }
    fn create_player(&mut self, details: PlayerDetails) -> login::Result<P> {
        let inner = &mut self.inner;
        let (name, uid) = {
            let account = crate::auth::storage::lock(&details.account);
            (account.username.clone(), account.uid)
        };
        let start = inner.start.clone();
        let node = inner.state.add(0, start.clone(), [1, 2, 3, 4]);
        inner.state.actor_mut(node).map_err(native)?.view.active = false;
        let owner = inner.state.retain_node(node).map_err(native)?;
        let mut inventory = inner
            .factory
            .create(28, ContainerType::Default, SortType::Id)
            .map_err(native)?;
        inventory.register(Some(ListenerHandle::new(InventoryListener::new(Some(
            owner.clone(),
        )))));
        // EquipmentContainer: bound to its player, with its EquipmentListener.
        let equipment = equipment::create(&inner.factory, Some(owner.clone())).map_err(native)?;
        let core = Capture::new(
            owner.clone(),
            inventory,
            equipment,
            Settings::default(),
            inner.factory.null_item.clone(),
        );
        let render = Shared::new(player_render::Player::new(0, start.clone(), 2000));
        render.write().active = false;
        let masks = PreparedMasks::new(
            render.clone(),
            Shared::new(appearance::Data::naked(name.clone(), 0)),
        );
        let clocks = inner.state.clocks(&owner).map_err(native)?;
        let animator = self
            .walk
            .animations
            .factory
            .actor_with_clocks(masks.clone(), clocks);
        let player = Player::new(
            inner.state.identity(),
            core,
            inner.items.clone(),
            interfaces::Manager::default(),
            Shared::new(false),
            masks,
        )
        .map_err(native)?;
        inner
            .state
            .bind_player_fields(player.clone())
            .map_err(native)?;
        let profile = Profile::new(
            details,
            Fresh {
                world: &inner.world,
                ticks: inner.ticks,
            },
            &inner.factory,
        )
        .map_err(native)?;
        player.bind_profile(profile).map_err(native)?;
        let session = inner
            .sessions
            .iter()
            .find(|(_, s)| s.username.as_deref() == Some(name.as_str()) && s.object.is_none())
            .map(|(id, _)| *id);
        let client = session
            .and_then(|s| inner.sessions.get(&s))
            .map(|s| s.client)
            .ok_or_else(|| native("a session for the new player"))?;
        inner.players.insert(
            node,
            Slot {
                name,
                uid,
                session,
                index: 0,
                active: false,
                playing: false,
                renderable: false,
                location: start,
                op_counts: [0; dispatch::OP_COUNT_SLOTS],
                invalid_packets: 0,
                afk_logout: None,
                canonical: Canonical {
                    player,
                    owner: owner.clone(),
                    render,
                    client,
                    weapon: None,
                    weapon_tab: None,
                    music: Default::default(),
                    friends: None,
                    options: interaction_options::Table::new(Some(node)),
                    animator,
                },
            },
        );
        self.walk.maps.players.insert(node);
        let slot = self.inner.players.get(&node).expect("the slot just made");
        self.walk
            .ground
            .accounts
            .insert(node, (slot.uid, slot.name.clone()));
        Ok(P(owner))
    }
    fn with_details<R>(
        &mut self,
        p: &P,
        f: impl FnOnce(&mut PlayerDetails) -> R,
    ) -> login::Result<R> {
        self.inner
            .with_profile(p, |profile| f(&mut profile.details))
    }
    fn log_ip(&mut self, _: &P, _: Option<&str>) -> login::Result<()> {
        self.inner
            .not_ported
            .skip("PlayerMonitor.log IP_LOG (player monitor not ported)");
        Ok(())
    }
    fn account_limit_exception(
        &mut self,
        path: &str,
        ip: Option<&str>,
    ) -> login::Result<Option<i64>> {
        // account_limit_exceptions.conf: `exceptions."<ip>"`, read as TOML.
        let Ok(text) = std::fs::read_to_string(path) else {
            return Ok(None);
        };
        let table: toml::Table = text
            .parse()
            .map_err(|e| login::Error::Exception(format!("{e}")))?;
        let key = ip.unwrap_or("null");
        Ok(table
            .get("exceptions")
            .and_then(|t| t.get(key))
            .and_then(|v| v.as_integer()))
    }
    fn daily_accounts(&mut self, _: Option<&str>) -> login::Result<Vec<String>> {
        // ServerStore's "daily-accounts" archive: the store is read at
        // start-up but its archive writes are not ported; every login starts
        // from an empty list (no limit is enforced across logins).
        self.inner
            .not_ported
            .default_answer("ServerStore daily-accounts (archive writes not ported: empty)");
        Ok(Vec::new())
    }
    fn add_daily_account(&mut self, _: Option<&str>, _: &str) -> login::Result<()> {
        self.inner
            .not_ported
            .skip("ServerStore daily-accounts addToList (archive writes not ported)");
        Ok(())
    }
    fn print_stack(&mut self, error: &login::Error) {
        self.inner.log_line(format!("printStackTrace {error:?}"));
    }
    fn is_updating(&mut self) -> bool {
        // SystemManager: this world never schedules an update.
        false
    }
    fn parse_save(&mut self, p: &P) -> login::Result<Option<Parser>> {
        let location = self.inner.slot(p).location.clone();
        let save_path = self.inner.save_path.clone();
        let (outcome, parser) = self.inner.environment_run(p, |player, state, env| {
            let mut parser = Parser::default();
            let outcome = player.load_file(state, Some(&location), &mut parser, &save_path, env);
            (outcome, parser)
        });
        // The messages and skull calls of the timers the parse registered.
        self.run_timer_calls();
        let outcome = outcome.map_err(native)?;
        if let Some(location) = outcome.location {
            let node = p.node();
            let Inner { state, players, .. } = &mut self.inner;
            let slot = players.get_mut(&node).expect("a live slot");
            walk::place(state, node, slot, &location).map_err(native)?;
        }
        match outcome.result {
            Ok(_) => Ok(Some(parser)),
            Err(error) => {
                self.inner
                    .log_line(format!("PlayerParser.parse printed {error:?}"));
                Ok(None)
            }
        }
    }
    fn submit_login_pulse(&mut self, pulse: LoginPulse<P, Parser, SessionId>) -> login::Result<()> {
        let handle = Shared::new(pulses::Pulse::new(
            login::pulse::DELAY,
            None,
            pulses::Class::Other,
            "core.game.node.entity.player.info.login.LoginParser$initialize$1".into(),
        ));
        self.driver
            .pulser()
            .submit(Some(handle.clone()))
            .map_err(native)?;
        self.logins.push((handle, pulse));
        Ok(())
    }
    fn social(&mut self) -> (&mut social::Social<P>, &mut Inner) {
        (&mut self.social, &mut self.inner)
    }
    fn saved_spawn_location(
        &mut self,
        p: &P,
        default: Option<Location>,
    ) -> login::Result<Option<Location>> {
        match self
            .inner
            .canonical(p)
            .player
            .attributes()
            .get("/save:spawnLocation")
        {
            None => Ok(default),
            Some(other) => Err(login::Error::Exception(format!(
                "not ported: a saved spawn location attribute ({})",
                other.java_string()
            ))),
        }
    }
    fn set_spawn_location(&mut self, p: &P, location: Option<Location>) -> login::Result<()> {
        self.inner
            .with_profile(p, |profile| profile.properties.spawn_location = location)
    }
    fn login_listener(&mut self, _: &str, _: &P) -> login::Result<()> {
        self.inner
            .not_ported
            .skip("GameWorld.loginListeners (content listeners by class name)");
        Ok(())
    }
    fn run_content_hooks(&mut self, p: &P, parser: &Parser) -> login::Result<()> {
        let location = self.inner.slot(p).location.clone();
        let result = self.inner.environment_run(p, |player, state, env| {
            player.run_content_hooks(state, Some(&location), parser, env)
        });
        result.map_err(native)?.map_err(native)
    }
    fn init_reconnect(&mut self, p: &P) -> login::Result<()> {
        // Player.initReconnect(), in its order.
        let node = p.node();
        // getInterfaceManager().setChatbox(null)
        self.inner
            .canonical(p)
            .player
            .manager()
            .fields
            .write()
            .chatbox = None;
        // getPulseManager().clear()
        self.with_walk(|world| world.clear_pulses(node))
            .map_err(native)?;
        // getZoneMonitor().getZones().clear(): the list, no zone callback.
        self.monitor(&p.0).zones().clear();
        // playerFlags: lastSceneGraph, updateSceneGraph, lastViewport.
        let render = self.render(p);
        {
            let mut view = render.write();
            view.last_scene = None;
            view.update_scene = false;
        }
        {
            let actor = self.inner.state.actor(node).map_err(native)?;
            let mut state = actor.queue.state.write();
            state.entity.last_scene = None;
            state.entity.update_scene = false;
        }
        self.regions
            .viewports
            .insert(node, crate::world::map_chunk_render::empty_viewport());
        // renderInfo: the local lists, lastLocation, onFirstCycle and the
        // appearance stamps.
        let info = render.read().info.clone();
        let mut info = info.write();
        info.local_npcs.write().clear();
        info.local_players.write().clear();
        info.last_location = None;
        info.on_first_cycle = true;
        info.appearance_stamps.write().fill(0);
        Ok(())
    }
    fn set_active(&mut self, p: &P, active: bool) {
        let node = p.node();
        self.inner.slot_mut(p).active = active;
        self.render(p).write().active = active;
        if let Ok(actor) = self.inner.state.actor_mut(node) {
            actor.view.active = active;
        }
    }
    fn add_auto_timers(&mut self, p: &P) -> login::Result<()> {
        // TimerRegistry.addAutoTimers (super::timers): AntiMacro, SkillRestore.
        self.add_auto_timers_of(p).map_err(native)
    }
    fn update_scene_graph(&mut self, p: &P, _: bool) -> login::Result<()> {
        struct Keys<'a> {
            keys: &'a crate::startup::loaders::xtea::RegionKeys,
            frames: Vec<Frame>,
        }
        impl rebuild::Context for Keys<'_> {
            fn keys(&mut self, id: i32) -> player_render::Result<[i32; 4]> {
                Ok(self.keys.get(id))
            }
            fn send(
                &mut self,
                _: &player_render::Handle,
                frame: Frame,
            ) -> player_render::Result<()> {
                self.frames.push(frame);
                Ok(())
            }
        }
        let render = self.render(p);
        if let Some(location) = render.read().location.clone() {
            self.walk.activate(&location).map_err(native)?;
        }
        let mut keys = Keys {
            keys: &self.inner.keys,
            frames: Vec::new(),
        };
        rebuild::send(&mut keys, &render).map_err(native)?;
        let last = render.read().last_scene.clone();
        self.inner
            .state
            .actor(p.node())
            .map_err(native)?
            .queue
            .state
            .write()
            .entity
            .last_scene = last;
        if let Some(session) = self.inner.session_of(p) {
            for frame in keys.frames {
                self.inner.direct_native(session, frame);
            }
        }
        Ok(())
    }
    fn reinit_varps(&mut self, p: &P) -> login::Result<()> {
        self.inner
            .variables(p, |ctx, owner| variables::reinit(ctx, Some(owner)))
    }
    fn attribute_bool(&mut self, p: &P, key: &str, default: bool) -> login::Result<bool> {
        self.inner
            .canonical(p)
            .player
            .attributes()
            .boolean(key, default)
            .map_err(native)
    }
    fn login_type_not_reconnect(&mut self, p: &P) -> login::Result<bool> {
        match self
            .inner
            .canonical(p)
            .player
            .attributes()
            .get("login_type")
        {
            None => Ok(true),
            Some(other) => Err(login::Error::Exception(format!(
                "not ported: a login_type attribute ({})",
                other.java_string()
            ))),
        }
    }
    fn set_attribute_bool(&mut self, p: &P, key: &str, value: bool) -> login::Result<()> {
        self.inner
            .canonical(p)
            .player
            .attributes()
            .set_boolean(key, value);
        Ok(())
    }
    fn log_line(&mut self, origin: &str, level: Level, message: &str) -> login::Result<()> {
        self.inner.log_line(format!("{origin} {level:?} {message}"));
        Ok(())
    }
    fn index(&mut self, p: &P) -> i32 {
        self.inner.slot(p).index
    }
    fn ui(&mut self, p: &P) -> login::Result<(&mut Inner, interfaces::Manager)> {
        Ok(self.ui_of(p))
    }
    fn dispatch_message(&mut self, p: &P, text: &str) -> login::Result<()> {
        self.inner.message(p, text)
    }
    fn set_interface_text(
        &mut self,
        p: &P,
        text: &str,
        interface: i32,
        child: i32,
    ) -> login::Result<()> {
        self.inner.text(p, text, interface, child)
    }
    fn send_last_login_info(&mut self, p: &P) -> login::Result<()> {
        // LastLoginInfo (164): imp4 of Inet4Address.getByName(details
        // .ipAddress).hashCode() (the address as a big-endian int), written
        // at once (session.write). A host name is not resolved here.
        let ip = self
            .inner
            .with_profile(p, |profile| profile.details.ip_address().map(str::to_owned))
            .map_err(native)?;
        let address: std::net::Ipv4Addr = ip
            .as_deref()
            .ok_or_else(|| login::Error::Exception("NullPointerException: ipAddress".into()))?
            .parse()
            .map_err(|_| {
                login::Error::Exception(
                    "not ported: sendLastLoginInfo of a host name (no resolver)".into(),
                )
            })?;
        let mut w = rs09_protocol::buffer::Writer::new();
        w.imp4(u32::from(address) as i32);
        if let Some(session) = self.inner.session_of(p) {
            self.inner.direct_native(
                session,
                Frame {
                    opcode: 164,
                    payload: w.into_vec(),
                },
            );
        }
        Ok(())
    }
    fn inventory_refresh(&mut self, p: &P) -> login::Result<()> {
        let inner = &mut self.inner;
        let player = inner.canonical(p).player.clone();
        player
            .apply(
                &mut inner.state,
                &mut inner.frames,
                now_millis,
                |inventory, definitions, ctx| inventory.refresh(None, definitions, ctx),
            )
            .map_err(native)?;
        inner.drain(p);
        Ok(())
    }
    fn equipment_refresh(&mut self, p: &P) -> login::Result<()> {
        let owner = self.inner.canonical(p).owner.clone();
        let mut container = self.inner.canonical(p).player.equipment();
        let items = self.inner.items.clone();
        let mut ctx = EquipmentContext {
            inner: &mut self.inner,
            p: p.clone(),
            listeners: Some(self.item_actions.listeners.clone()),
        };
        equipment::Listener::new(Some(owner))
            .refresh(&mut ctx, &mut container, &items)
            .map_err(native)
    }
    fn skills_refresh(&mut self, p: &P) -> login::Result<()> {
        let mut skills = self.inner.with_profile(p, |profile| {
            std::mem::replace(&mut profile.skills, placeholder_skills())
        })?;
        let result = skills.refresh(&mut SkillsContext {
            inner: &mut self.inner,
            p: p.clone(),
        });
        self.inner
            .with_profile(p, |profile| profile.skills = skills)?;
        result.map_err(native)
    }
    fn skills_configure(&mut self, p: &P) -> login::Result<()> {
        let mut skills = self.inner.with_profile(p, |profile| {
            std::mem::replace(&mut profile.skills, placeholder_skills())
        })?;
        let result = skills.update_combat_level(&mut SkillsContext {
            inner: &mut self.inner,
            p: p.clone(),
        });
        self.inner
            .with_profile(p, |profile| profile.skills = skills)?;
        result.map(|_| ()).map_err(native)
    }
    fn settings_update(&mut self, p: &P) -> login::Result<()> {
        let settings = self.inner.canonical(p).player.settings();
        settings
            .update(&mut SettingsContext {
                inner: &mut self.inner,
                p: p.clone(),
            })
            .map_err(native)
    }
    fn interaction_set_default(&mut self, p: &P) -> login::Result<()> {
        let table = self.inner.canonical(p).options.clone();
        interaction_options::set_default(
            &mut OptionsContext {
                inner: &mut self.inner,
                p: p.clone(),
            },
            &table,
        )
        .map_err(native)
    }
    fn send_run_energy(&mut self, p: &P) -> login::Result<()> {
        let (energy, toggled) = {
            let settings = self.inner.canonical(p).player.settings();
            let fields = settings.fields.read();
            (fields.run_energy, fields.run_toggled)
        };
        // The walking queue's entity keeps Settings' run state.
        {
            let actor = self.inner.state.actor(p.node()).map_err(native)?;
            let mut state = actor.queue.state.write();
            state.entity.energy = energy;
            state.entity.run_toggled = toggled;
        }
        self.inner.queue_native(p, updates::run_energy(energy));
        Ok(())
    }
    fn familiar_login(&mut self, _: &P) -> login::Result<()> {
        self.inner
            .not_ported
            .skip("FamiliarManager.login (summoning not ported)");
        Ok(())
    }
    fn quest_synchronize_tab(&mut self, p: &P) -> login::Result<()> {
        // QuestRepository.syncronizeTab, native over the profile's stages
        // and the world's registry (empty: quest classes are content).
        Tabs::new(&mut self.inner, p.clone())
            .quest_tab()
            .map_err(native)
    }
    fn emote_refresh(&mut self, p: &P) -> login::Result<()> {
        // EmoteManager.refresh, native over the profile's persisted emotes.
        Tabs::new(&mut self.inner, p.clone())
            .emotes(|tabs, manager| tab_emotes::refresh(tabs, manager))
            .map_err(native)
    }
    fn login_plugins(&mut self) -> Vec<String> {
        self.login_plugins.clone()
    }
    fn login_plugin(&mut self, class: &str, p: &P) -> login::Result<()> {
        // The global handlers' LoginValidationPlugin (super::global).
        if let Some(result) = self.global_login_plugin(class, p) {
            return result.map_err(|e| login::Error::Exception(format!("{class}: {e:?}")));
        }
        self.inner
            .not_ported
            .skip("LoginConfiguration.LOGIN_PLUGINS (content plugins by class name other than LoginValidationPlugin)");
        Ok(())
    }
    fn appearance_sync(&mut self, p: &P) -> login::Result<()> {
        // Appearance.sync: the body prepared from the canonical appearance
        // and the worn equipment, then the appearance mask registered.
        self.inner.appearance_sync(p).map(|_| ()).map_err(native)
    }
    fn renderable_players_add(&mut self, p: &P) -> login::Result<()> {
        self.driver.renderable_players().add(p.clone());
        Ok(())
    }
    fn region_move(&mut self, p: &P) -> login::Result<()> {
        // RegionManager.move(player, null, player.getLocation()).
        let location = self.inner.slot(p).location.clone();
        Live::region_move(self, &p.0, None, location).map_err(native)
    }
    fn music_init(&mut self, p: &P) -> login::Result<()> {
        // MusicPlayer.init, native over the profile's persisted songs.
        Tabs::new(&mut self.inner, p.clone())
            .music(|tabs, player| player.init(tabs))
            .map_err(native)
    }
    fn update_appearance(&mut self, p: &P) -> login::Result<()> {
        self.inner
            .canonical(p)
            .player
            .masks()
            .sync_appearance(now_millis)
            .map(|_| ())
            .map_err(native)
    }
    fn set_update_scene_graph_flag(&mut self, p: &P, value: bool) -> login::Result<()> {
        let node = p.node();
        let Inner { state, players, .. } = &self.inner;
        walk::set_scene_flag(state, node, players.get(&node).expect("a slot"), value)
            .map_err(native)
    }
    fn test_stage(&mut self, p: &P) -> login::Result<i32> {
        self.inner
            .with_profile(p, |profile| profile.saved.global.player_test_stage)
    }
    fn safety_first_unlocked(&mut self, p: &P) -> login::Result<bool> {
        self.inner.with_profile(p, |profile| {
            profile
                .persisted
                .emotes
                .is_unlocked(tab_emotes::Emote::SafetyFirst.ordinal())
        })
    }
    fn unlock_safety_first(&mut self, p: &P) -> login::Result<()> {
        Tabs::new(&mut self.inner, p.clone())
            .emotes(|tabs, manager| {
                tab_emotes::unlock(tabs, manager, tab_emotes::Emote::SafetyFirst)
            })
            .map(|_| ())
            .map_err(native)
    }
    fn attack_style_index(&mut self, p: &P) -> login::Result<i32> {
        Ok(self
            .inner
            .canonical(p)
            .player
            .settings()
            .fields
            .read()
            .attack_style_index)
    }
    fn equipment_items(&mut self, p: &P) -> login::Result<Vec<Option<i32>>> {
        let items = self.inner.canonical(p).player.equipment().items();
        let items = items.read();
        Ok(items
            .iter()
            .map(|item| item.as_ref().map(|i| i.read().id()))
            .collect())
    }
    fn rerun_equip_hooks(&mut self, _: &P, _: usize, _: i32) -> login::Result<()> {
        self.inner.not_ported.skip(
            "LoginConfiguration equipment hooks per equipped item (equip content not ported)",
        );
        Ok(())
    }
    fn spell_book(&mut self, p: &P) -> login::Result<i32> {
        self.inner.with_profile(p, |profile| profile.spell_book)
    }
    fn player_message(&mut self, p: &P, text: &str) -> login::Result<()> {
        self.inner.message(p, text)
    }
    fn has_requirement(&mut self, _: &P, _: &str, _: bool) -> login::Result<bool> {
        // QuestRequirements is content: no quest is registered in this
        // world, so no requirement is met.
        self.inner
            .not_ported
            .default_answer("QuestRequirements.hasRequirement (no quests registered: false)");
        Ok(false)
    }
    fn set_spell_book(&mut self, p: &P, book: i32) -> login::Result<()> {
        self.inner
            .with_profile(p, |profile| profile.spell_book = book)
    }
    fn set_varbit(&mut self, p: &P, id: i32, value: i32, save: bool) -> login::Result<()> {
        self.inner.variables(p, |ctx, owner| {
            variables::set_varbit(ctx, Some(owner), id, value, save)
        })
    }
    fn rules_and_info(&mut self, _: &P) -> login::Result<()> {
        self.inner.not_ported.skip("RulesAndInfo.openFor (content)");
        Ok(())
    }
    fn set_last_login(&mut self, p: &P, millis: i64) -> login::Result<()> {
        self.inner.with_profile(p, |profile| {
            crate::auth::storage::lock(&profile.details.account).last_login = millis;
        })
    }
}

// ---------------------------------------------------- driver::Host

/// The native player renderer's services: candidates are the live players
/// within 15 tiles in repository order (entity regions are not joined);
/// masks are the canonical players' prepared masks.
struct Render<'a> {
    inner: &'a Inner,
    /// The SkullManagers and `wild_pvp_enabled` the Appearance flag reads.
    zones: &'a super::region_zones::RegionZones,
    candidates: Vec<Option<player_render::Handle>>,
    frames: Vec<Frame>,
}
impl player_render::Context for Render<'_> {
    fn candidates(
        &mut self,
        _: Location,
        _: i32,
    ) -> player_render::Result<Vec<Option<player_render::Handle>>> {
        Ok(std::mem::take(&mut self.candidates))
    }
    fn masks(
        &mut self,
        local: &player_render::Handle,
        other: &player_render::Handle,
        output: &mut Vec<u8>,
        appearance: bool,
        sync: bool,
    ) -> player_render::Result<()> {
        let p = self
            .inner
            .by_render(other)
            .ok_or(player_render::Error::Binding(
                "a mask of an unknown player".into(),
            ))?;
        // PlayerFlags530.Appearance runs here, at the write, over the live
        // receivers (super::appearance).
        let mut receivers = super::appearance::Receivers {
            inner: self.inner,
            skulls: &self.zones.skulls,
            wild_pvp: self.zones.wild_pvp,
        };
        self.inner
            .canonical(&p)
            .player
            .masks()
            .write_late(local.clone(), output, appearance, sync, &mut |buffer| {
                receivers.encode(&p.0, buffer)
            })
            .map_err(|e| player_render::Error::Binding(e.to_string()))
    }
    fn send(&mut self, _: &player_render::Handle, frame: Frame) -> player_render::Result<()> {
        self.frames.push(frame);
        Ok(())
    }
}

impl driver::Host<P> for Live {
    type Outgoing = Sent;
    fn clock_millis(&mut self) -> i64 {
        now_millis()
    }
    fn telemetry(&mut self, _: Metric, _: i32) {}
    fn metrics_enabled(&mut self) -> bool {
        false
    }
    fn is_playing(&mut self, p: &P) -> bool {
        self.inner.slot(p).playing
    }
    fn driver_log(&mut self, origin: &str, level: DriverLevel, message: &str) {
        self.inner.log_line(format!("{origin} {level:?} {message}"));
    }
    fn stack_trace(&mut self, error: &driver::Error) -> String {
        format!("{error:?}\n")
    }
    fn driver_print_stack_trace(&mut self, error: &driver::Error) {
        eprintln!("[live] printStackTrace {error:?}");
        self.inner.log_line(format!("printStackTrace {error:?}"));
    }
    fn reading_queue(&mut self, s: &SessionId) -> Option<Vec<u8>> {
        self.inner
            .sessions
            .get_mut(s)
            .and_then(|s| s.reading_queue.take())
    }
    fn set_reading_queue(&mut self, s: &SessionId, queue: Option<Vec<u8>>) {
        if let Some(s) = self.inner.sessions.get_mut(s) {
            s.reading_queue = queue;
        }
    }
    fn set_last_ping(&mut self, s: &SessionId, millis: i64) {
        if let Some(s) = self.inner.sessions.get_mut(s) {
            s.last_ping = millis;
        }
    }
    fn add_invalid_packet(&mut self, p: &P) -> i32 {
        let slot = self.inner.slot_mut(p);
        slot.invalid_packets = slot.invalid_packets.wrapping_add(1);
        slot.invalid_packets
    }
    fn op_counts(&mut self, p: &P) -> &mut [i8; dispatch::OP_COUNT_SLOTS] {
        &mut self.inner.slot_mut(p).op_counts
    }
    fn handle(
        &mut self,
        handler: dispatch::Handler,
        packet: &dispatch::Packet<P>,
    ) -> driver::Result<()> {
        super::server::handle(self, handler, packet)
    }
    fn is_administrator(&mut self, p: &P) -> driver::Result<bool> {
        let rights = self
            .inner
            .with_profile(p, |profile| profile.details.rights())
            .map_err(fail)?
            .map_err(fail)?;
        Ok(rights == crate::player::details::Rights::Administrator)
    }
    fn set_afk_logout(&mut self, p: &P, value: bool) {
        self.inner.slot_mut(p).afk_logout = Some(value);
    }
    fn packet_count(&mut self, p: &P, increment: i32) -> driver::Result<i32> {
        Ok(self
            .inner
            .canonical(p)
            .player
            .manager()
            .packet_count(increment))
    }
    fn pulse_stop(&mut self, pulse: &pulses::Handle) -> driver::Result<()> {
        // TeleportPulse.stop (super::magic); every other world pulse here
        // keeps the base stop.
        if let Some(result) = self.magic_pulse_stop(pulse) {
            return result.map_err(fail);
        }
        pulses::stop_base(pulse);
        Ok(())
    }
    fn pulse_body(&mut self, pulse: &pulses::Handle) -> driver::Result<bool> {
        // Combat pulses, impact pulses and death tasks (super::combat).
        if let Some(result) = self.combat_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // The global handlers' submitted bodies (super::global).
        if let Some(result) = self.global_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // Teleport pulses (super::magic).
        if let Some(result) = self.magic_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // The pulses of the install round after the dragons (super::round).
        if let Some(result) = self.round_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // The content use-with listeners' pulses (super::items::everyday).
        if let Some(result) = self.everyday_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // The small item-on-object listeners' pulses
        // (super::items::withobject).
        if let Some(result) = self.with_object_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // The cooking pulses (super::items::cooking).
        if let Some(result) = self.cooking_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // SceneryBuilder's pulses (the scene submitted them to this runner).
        if self.walk.scene.owns_pulse(pulse) {
            let walk = &mut self.walk;
            let done = crate::world::scenery::Context::pulse_direct(
                &mut walk.scene.context(&mut walk.maps),
                pulse,
            );
            walk.scene.collect_pulses();
            return done.map_err(fail);
        }
        // CowMilkingPlugin's milking pulses (super::items::milking).
        if let Some(result) = self.milking_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // SmeltingPulse (super::items::smelting).
        if let Some(result) = self.smelting_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // StallThiefPulse (super::items::stalls).
        if let Some(result) = self.stall_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // SmithingPulse (super::items::smithing).
        if let Some(result) = self.smithing_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // WoodcuttingSkillPulse (super::items::woodcutting).
        if let Some(result) = self.woodcutting_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // BarbFishingPulse and FishCuttingPulse (super::items::barbarian).
        if let Some(result) = self.barbarian_pulse_body(pulse) {
            return result.map_err(fail);
        }
        if let Some(result) = self.fish_cutting_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // FireMakingPulse and BarbFirePulse (super::items::firemaking).
        if let Some(result) = self.firemaking_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // ProspectListener's message pulse (super::items::mining).
        if let Some(result) = self.prospect_pulse_body(pulse) {
            return result.map_err(fail);
        }
        // PulseManager.runUnhandledAction's pulse of a use-with.
        if let Some(result) = self.use_pulse(pulse) {
            return result.map_err(fail);
        }
        // InteractPlugin.handleItemOption's pulse (InteractPlugin$1).
        if let Some(result) = self.item_pulse(pulse) {
            return result.map_err(fail);
        }
        // GroundSpawn.respawn's pulse (GroundSpawnLoader$GroundSpawn$respawn$1).
        if let Some(result) = self.ground_respawn_pulse(pulse) {
            return result.map_err(fail);
        }
        let Some(at) = self.logins.iter().position(|(h, _)| h.same(pulse)) else {
            return Err(driver::Error::Native(
                "a world pulse this world did not submit".into(),
            ));
        };
        let login = self.logins.remove(at).1;
        let repository = self.repository.clone();
        let listeners = self.login_listeners.clone();
        let done = login::pulse::run(self, &repository, &listeners, &login).map_err(fail)?;
        if self.inner.players.contains_key(&login.player.node())
            && self.inner.slot(&login.player).active
        {
            let name = self.inner.slot(&login.player).name.clone();
            self.online
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(name.clone());
            self.inner.log_line(format!("{name} entered the world"));
        }
        if !done {
            self.logins.insert(at, (pulse.clone(), login));
        }
        Ok(done)
    }
    fn pulse_node_active(&mut self, node: pulses::Node) -> driver::Result<bool> {
        // Node.isActive() of a pulse's checked node (the tab pulses' player).
        match self.inner.players.get(&node) {
            Some(slot) => Ok(slot.active),
            // An NPC checked by a combat or death pulse.
            None if self.inner.npcs.rows.contains_key(&node) => Ok(self.inner.node_active(node)),
            None => match self.walk.scene.checked_active(node) {
                // The scenery a SceneryBuilder pulse checks (a fire).
                Some(active) => Ok(active),
                None => Err(driver::Error::Native(format!(
                    "a world pulse checks node {node}, which is not a live player"
                ))),
            },
        }
    }
    fn tick_listeners(&mut self) -> driver::Result<()> {
        // The canonical state's runner (movement pulses) is the rest of
        // GameWorld.Pulser.updateAll: run here, right after the world runner.
        self.with_walk(|world| world.pulses()).map_err(fail)?;
        // GameWorld.tickListeners: the installed ones (super::content).
        self.content_tick_listeners().map_err(fail)?;
        // Shops.tick() (after GraveController in the scan's order).
        self.shops_tick().map_err(fail)?;
        self.inner.not_ported.skip(
            "GameWorld.tickListeners (the content tick listeners other than those installed)",
        );
        Ok(())
    }
    fn managers_tick(&mut self) -> driver::Result<()> {
        self.inner
            .not_ported
            .skip("Managers.tick (manager plugins: Grand Exchange and others)");
        Ok(())
    }
    fn set_active(&mut self, p: &P, active: bool) -> driver::Result<()> {
        login::Host::set_active(self, p, active);
        Ok(())
    }
    fn clear_minimap_packet(&mut self, p: &P) -> Option<Sent> {
        // A lobby entry whose player already left has no slot (and no session).
        let session = self.inner.players.get(&p.node())?.session?;
        Some(Sent {
            session,
            class: "ClearMinimapFlag",
            frame: Frame {
                opcode: 153,
                payload: vec![],
            },
            at: self.inner.queue.len(),
        })
    }
    fn npc_tick(&mut self, npc: &P) -> driver::Result<()> {
        // One NPC's failure is isolated (super::faults): the sequence goes
        // on with the next NPC instead of ending the tick for everyone.
        let result = self.npc_tick_unisolated(npc);
        self.isolate_npc(npc, "tick", result);
        Ok(())
    }
    fn npc_reset(&mut self, npc: &P) -> driver::Result<()> {
        let result = self.npc_reset_of(npc).map_err(fail);
        self.isolate_npc(npc, "reset", result);
        Ok(())
    }
    fn player_tick(&mut self, p: &P) -> driver::Result<()> {
        // DropListener's soft queued scripts that are due (super::items).
        self.run_drop_scripts(p).map_err(fail)?;
        // The potions' dose messages that are due (super::eating).
        self.run_dose_scripts(p).map_err(fail)?;
        let node = p.node();
        // Entity.tick: scripts, skills.pulse(), the walking queue (and the
        // region move it asks for), then timers.processTimers() (its event
        // is the last of entity_before_masks), then updateMasks.prepare.
        // The script phases run with the whole world behind their bodies
        // (super::scripts).
        self.with_script_host(|world| world.entity_before_masks(node))
            .map_err(fail)?;
        self.process_timers_of(p).map_err(fail)?;
        // updateMasks.prepare polls the ImpactHandler's queue: the impacts
        // the prepared masks will poll this tick, with their health ratio.
        self.hand_impacts_to_masks(p);
        self.with_walk(|world| world.prepare_masks(node))
            .map_err(fail)?;
        let Inner { state, players, .. } = &mut self.inner;
        if let Some(slot) = players.get_mut(&node) {
            walk::project(state, node, slot).map_err(fail)?;
        }
        // musicPlayer.tick(), native.
        Tabs::new(&mut self.inner, p.clone())
            .music(|tabs, player| player.tick(tabs))
            .map_err(fail)?;
        self.inner
            .not_ported
            .skip("Player.tick potion timers, wealth, off-map check (not ported)");
        // getPrayer().tick(): the prayer drain (world::prayer).
        self.prayer_tick(p).map_err(fail)?;
        Ok(())
    }
    fn update_scene_graph_flag(&mut self, p: &P) -> driver::Result<bool> {
        Ok(self.render(p).read().update_scene)
    }
    fn set_update_scene_graph_flag(&mut self, p: &P, value: bool) -> driver::Result<()> {
        login::Host::set_update_scene_graph_flag(self, p, value).map_err(fail)
    }
    fn update_scene_graph(&mut self, p: &P) -> driver::Result<()> {
        login::Host::update_scene_graph(self, p, false).map_err(fail)
    }
    fn render_players(&mut self, p: &P) -> driver::Result<()> {
        let local = self.render(p);
        let here = local
            .read()
            .location
            .clone()
            .ok_or_else(|| fail("no location"))?;
        // RegionManager.getLocalPlayers(player, 15): the chunks around the
        // player, x then y, each chunk's players in list order.
        let mut candidates = Vec::new();
        let nodes = self
            .local(here, 15, crate::world::entity_regions::Select::Players)
            .map_err(fail)?;
        for other in nodes {
            let Some(slot) = self.inner.players.get(&other.node) else {
                continue;
            };
            candidates.push(Some(slot.canonical.render.clone()));
        }
        let mut ctx = Render {
            inner: &self.inner,
            zones: &self.zones,
            candidates,
            frames: Vec::new(),
        };
        player_render::render(&mut ctx, &local).map_err(fail)?;
        let frames = ctx.frames;
        if let Some(session) = self.inner.session_of(p) {
            for frame in frames {
                self.inner.direct_native(session, frame);
            }
        }
        Ok(())
    }
    fn render_npcs(&mut self, p: &P) -> driver::Result<()> {
        self.render_npcs_of(p).map_err(fail)
    }
    fn render_map_chunks(&mut self, p: &P) -> driver::Result<()> {
        self.render_map_chunks_of(p).map_err(fail)
    }
    fn entity_reset(&mut self, p: &P) -> driver::Result<()> {
        // Entity.reset: updateMasks.reset() and setTeleporting(false).
        self.inner.canonical(p).player.masks().reset();
        self.render(p).write().teleporting = false;
        let actor = self.inner.state.actor(p.node()).map_err(fail)?;
        actor.queue.state.write().entity.teleporting = false;
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
        self.inner
            .with_profile(p, |profile| profile.skills.lifepoints_update)
            .map_err(fail)
    }
    fn set_lifepoints_update(&mut self, p: &P, value: bool) -> driver::Result<()> {
        self.inner
            .with_profile(p, |profile| profile.skills.lifepoints_update = value)
            .map_err(fail)
    }
    fn hitpoints_packet(&mut self, p: &P) -> Option<Sent> {
        let session = self.inner.session_of(p)?;
        let (level, experience) = self
            .inner
            .with_profile(p, |profile| {
                (profile.skills.lifepoints, profile.skills.experience[3])
            })
            .ok()?;
        Some(Sent {
            session,
            class: "SkillLevel",
            frame: updates::skill_level(3, level, experience),
            at: self.inner.queue.len(),
        })
    }
    fn attribute_bool(&mut self, p: &P, key: &str, default: bool) -> driver::Result<bool> {
        self.inner
            .canonical(p)
            .player
            .attributes()
            .boolean(key, default)
            .map_err(fail)
    }
    fn remove_attribute(&mut self, p: &P, key: &str) -> driver::Result<()> {
        self.inner.canonical(p).player.attributes().remove(key);
        Ok(())
    }
    fn reset_chunks(&mut self) -> driver::Result<()> {
        // ChunkUpdateTracker.resetAll: every flagged chunk's update list.
        self.walk.scene.clear_updates();
        self.inner.combat.projectiles.clear();
        Ok(())
    }
    fn ground_items_pulse(&mut self) -> driver::Result<()> {
        // GroundItemManager.pulse(), native (super::ground_items).
        self.ground_pulse().map_err(fail)
    }
    fn execute_autosave(&mut self, task: Autosave) -> driver::Result<()> {
        self.tasks.push_back(Task::Autosave(task));
        Ok(())
    }
    fn send_packet(&mut self, packet: &Sent) -> driver::Result<()> {
        if driver::write_queue::IMMEDIATE.contains(&packet.class) {
            self.inner
                .direct_native(packet.session, packet.frame.clone());
            return Ok(());
        }
        let at = (packet.at + self.inserted).min(self.inner.queue.len());
        self.inner
            .queue
            .insert(at, (packet.session, packet.frame.clone()));
        self.inserted += 1;
        Ok(())
    }
}
