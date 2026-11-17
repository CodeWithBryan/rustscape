//! The canonical state the live hosts answer from, and the contexts of the
//! libraries a player owns: the interface manager's context, social's
//! services, Skills, Settings, the equipment listener, interaction options
//! and the save file. The shape is the test binding of
//! `tests/world_login.rs`; where that binding replays an original packet
//! segment, this one answers by name ([`super::not_ported`]).
use super::{P, SessionId, not_ported::NotPorted, now_millis};
use crate::{
    auth::{Clock, account::UserAccountInfo, storage::AccountStorage},
    definitions::item::{Registry as Items, Shared},
    inventory::{
        self,
        container::{Container, Event, Factory},
        container_listener::Handle as ListenerHandle,
        equipment,
        item::Handle as ItemHandle,
    },
    player::{
        attributes::AttributeValue,
        core_data::CoreData,
        details::{ClientInfo, PlayerDetails},
        owned::{
            Player,
            profile::{Environment, Profile},
        },
        persisted::host::{Remaining, World},
        save::{self, Level as SaveLevel},
        settings::{self, Settings},
        skills::{self, Effect, EntityKind, Skills},
        variables,
    },
    startup::loaders::xtea::RegionKeys,
    timers,
    world::{
        doors::Location,
        driver::dispatch::OP_COUNT_SLOTS,
        interaction_options, interfaces,
        login::Constants,
        player_render,
        player_walk::{Node, NodeHandle, State},
        repository::Repository,
        session::Joined,
        social,
    },
};
use rs09_cache::archive::Archive;
use rs09_protocol::{
    buffer::Writer,
    game::{Frame, updates},
    login::AuthResponse,
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{mpsc, oneshot};

/// The canonical player of one slot and the objects login binds to it.
pub struct Canonical {
    pub player: Player,
    pub owner: NodeHandle,
    pub render: player_render::Handle,
    pub client: ClientInfo,
    pub weapon: Option<interfaces::Handle>,
    pub friends: Option<interfaces::Handle>,
    pub options: interaction_options::Handle,
    /// Entity.animator over the player's prepared masks and clocks.
    pub animator: crate::world::animation::Animator<crate::world::prepared_animation::Owner>,
}

/// The fields of `Player`/`Node` the hosts read and write, by node.
pub struct Slot {
    pub name: String,
    pub uid: i32,
    pub session: Option<SessionId>,
    pub index: i32,
    pub active: bool,
    pub playing: bool,
    pub renderable: bool,
    /// `Entity.location` as the save and login read it; the walking queue's
    /// entity holds the live value and is projected here every tick.
    pub location: Location,
    pub op_counts: [i8; OP_COUNT_SLOTS],
    pub invalid_packets: i32,
    pub afk_logout: Option<bool>,
    pub canonical: Canonical,
}

/// The transport side of a session: the two channels of
/// [`crate::world::session::Joined`] and the admission waiting for its reply.
#[derive(Default)]
pub struct Link {
    /// Frames to the client, one batch per tick.
    pub output: Option<mpsc::Sender<Vec<Frame>>>,
    /// Frames from the client.
    pub input: Option<mpsc::Receiver<Frame>>,
    /// The admission waiting for the login reply, with the transport ends
    /// handed over when it is a success.
    pub pending: Option<Pending>,
}
pub struct Pending {
    pub reply: oneshot::Sender<Result<Joined, AuthResponse>>,
    pub incoming: mpsc::Sender<Frame>,
    pub outgoing: mpsc::Receiver<Vec<Frame>>,
}

/// `IoSession` as the hosts see it.
pub struct Session {
    pub active: bool,
    pub object: Option<P>,
    pub username: Option<String>,
    pub address: Option<String>,
    pub client: ClientInfo,
    /// Written at once during this tick (sent before the flushed queue).
    pub direct: Vec<Frame>,
    pub reading_queue: Option<Vec<u8>>,
    pub last_ping: i64,
    pub game: bool,
    /// `socket.close()` was asked: the session's output ends after this
    /// tick's frames.
    pub closing: bool,
    pub link: Link,
}

/// World settings the social code and the worker read, from the configuration.
#[derive(Clone, Debug)]
pub struct WorldSettings {
    pub world_name: String,
    pub dev_mode: bool,
    pub enable_default_clan: bool,
    pub global_chat: bool,
    pub ms_secret_key: String,
}

/// What the live hosts answer from.
pub struct Inner {
    pub state: State,
    pub frames: BTreeMap<Node, Vec<Frame>>,
    pub items: Arc<Items>,
    pub factory: Factory,
    pub registry: interfaces::Registry,
    pub options: interaction_options::Constants,
    pub keys: RegionKeys,
    pub constants: Constants,
    pub settings: WorldSettings,
    pub world: World,
    /// `PlayerSaveParser.contentHooks.size`: the persist-player listeners by
    /// class name; none is ported.
    pub hooks: usize,
    pub players: BTreeMap<Node, Slot>,
    pub sessions: BTreeMap<SessionId, Session>,
    /// `PacketWriteQueue`'s queued packets of this tick, in order.
    pub queue: Vec<(SessionId, Frame)>,
    pub current: Option<P>,
    /// `ServerConstants.PLAYER_SAVE_PATH`, ending in a separator.
    pub save_path: String,
    pub attribute_path: String,
    pub clock: Arc<Clock>,
    /// `ServerConstants.START_LOCATION`.
    pub start: Location,
    pub ticks: i32,
    pub not_ported: NotPorted,
    pub varbits: Arc<Archive>,
    pub repository: Repository<P>,
    pub storage: Arc<dyn AccountStorage>,
    /// Log lines (`core.tools.Log`), echoed when verbose.
    pub log: Vec<String>,
    pub verbose: bool,
}

pub fn native<E: std::fmt::Debug>(e: E) -> crate::world::login::Error {
    crate::world::login::Error::Exception(format!("{e:?}"))
}

impl Inner {
    pub fn log_line(&mut self, line: String) {
        if self.verbose {
            eprintln!("[live] {line}");
        }
        if self.log.len() >= 4096 {
            self.log.drain(..2048);
        }
        self.log.push(line);
    }
    pub fn slot(&self, p: &P) -> &Slot {
        self.players.get(&p.node()).expect("a live player slot")
    }
    pub fn slot_mut(&mut self, p: &P) -> &mut Slot {
        self.players.get_mut(&p.node()).expect("a live player slot")
    }
    pub fn canonical(&self, p: &P) -> &Canonical {
        &self.slot(p).canonical
    }
    pub fn session_of(&self, p: &P) -> Option<SessionId> {
        self.slot(p).session
    }
    /// Queued for the session (`PacketWriteQueue.push`).
    pub fn queue_native(&mut self, p: &P, frame: Frame) {
        if let Some(session) = self.session_of(p) {
            self.queue.push((session, frame));
        }
    }
    /// Written at once.
    pub fn direct_native(&mut self, session: SessionId, frame: Frame) {
        if let Some(s) = self.sessions.get_mut(&session) {
            s.direct.push(frame);
        }
    }
    /// Moves what the canonical services queued for the player's node into
    /// the session queue.
    pub fn drain(&mut self, p: &P) {
        let node = p.node();
        if let Some(frames) = self.frames.remove(&node) {
            for frame in frames {
                self.queue_native(p, frame);
            }
        }
    }
    pub fn variables<T>(
        &mut self,
        p: &P,
        operation: impl FnOnce(
            &mut crate::player::owned::VariableServices<'_, Arc<Archive>>,
            &NodeHandle,
        ) -> variables::Result<T>,
    ) -> crate::world::login::Result<T> {
        let player = self.canonical(p).player.clone();
        let owner = self.canonical(p).owner.clone();
        let mut assets = self.varbits.clone();
        let result = player
            .apply_variables(&mut self.state, &mut self.frames, &mut assets, |ctx| {
                operation(ctx, &owner)
            })
            .map_err(native);
        self.drain(p);
        result
    }
    pub fn text(
        &mut self,
        p: &P,
        text: &str,
        interface: i32,
        child: i32,
    ) -> crate::world::login::Result<()> {
        let counter = self.canonical(p).player.manager().packet_count(1);
        let frame =
            updates::interface_text(interface.wrapping_shl(16) | child, text.as_bytes(), counter)
                .map_err(native)?;
        self.queue_native(p, frame);
        Ok(())
    }
    /// `PacketDispatch.sendMessage`: the chat filter, then GameMessage (70).
    pub fn message(&mut self, p: &P, text: &str) -> crate::world::login::Result<()> {
        let filtered = self
            .canonical(p)
            .player
            .attributes()
            .get("chat_filter")
            .is_some();
        if filtered && !text.contains("<col=CC6600>") && !text.contains("<col=FFFF00>") {
            return Ok(());
        }
        // The original truncates longer messages; nothing in this world sends one.
        if text.encode_utf16().count() > 255 {
            return Err(crate::world::login::Error::Native(
                "message truncation is not ported".into(),
            ));
        }
        let mut w = Writer::new();
        w.pjstr(text.as_bytes()).map_err(native)?;
        self.queue_native(
            p,
            Frame {
                opcode: 70,
                payload: w.into_vec(),
            },
        );
        Ok(())
    }
    pub fn with_profile<T>(
        &self,
        p: &P,
        f: impl FnOnce(&mut Profile) -> T,
    ) -> crate::world::login::Result<T> {
        self.canonical(p).player.with_profile(f).map_err(native)
    }
    /// A save-file operation with the services that stay outside the
    /// canonical player.
    pub fn environment_run<T>(
        &mut self,
        p: &P,
        f: impl FnOnce(&Player, &State, &mut Environment<'_, LiveTimers, Rest>) -> T,
    ) -> T {
        let player = self.canonical(p).player.clone();
        let mut timers = LiveTimers::default();
        let mut rest = Rest {
            hooks: self.hooks,
            ticks: self.ticks,
            skipped: Vec::new(),
        };
        let clock = self.clock.clone();
        let path = self.attribute_path.clone();
        let mut env = Environment {
            clock: &*clock,
            timers: &mut timers,
            services: &mut rest,
            world: &mut self.world,
            attribute_path: &path,
        };
        let result = f(&player, &self.state, &mut env);
        for what in rest.skipped.into_iter().chain(timers.skipped) {
            self.not_ported.skip(what);
        }
        result
    }
    /// The live player whose render handle this is.
    pub fn by_render(&self, handle: &player_render::Handle) -> Option<P> {
        self.players
            .values()
            .find(|s| s.canonical.render.same(handle))
            .map(|s| P(s.canonical.owner.clone()))
    }
}

/// Config (60) and its large form (226) as the original writes them.
pub fn interaction_option(index: i32, name: &str) -> Frame {
    let mut w = Writer::new();
    // putLEShortA(-1): the low byte plus 128, then the high byte.
    let value: i32 = -1;
    w.p1(value.wrapping_add(128));
    w.p1(value >> 8);
    let remove = name == "null";
    w.p1(if remove {
        0
    } else if index == 0 {
        1
    } else {
        0
    });
    w.p1(index + 1);
    w.pjstr(name.as_bytes()).expect("an option name");
    Frame {
        opcode: 44,
        payload: w.into_vec(),
    }
}

// ---------------------------------------------------- interfaces::Context

fn ui_error(e: impl std::fmt::Display) -> interfaces::Error {
    interfaces::Error::Binding(e.to_string())
}

impl interfaces::Context for Inner {
    fn validate_manager(&mut self, manager: &interfaces::Manager) -> interfaces::Result<()> {
        let p = self.current.clone().ok_or(interfaces::Error::Binding(
            "no current player for the interface context".into(),
        ))?;
        let owner = self.canonical(&p).owner.clone();
        if manager.player().as_ref() != Some(&owner) {
            return Err(interfaces::Error::Binding(
                "another player's interface manager".into(),
            ));
        }
        Ok(())
    }
    fn component(&mut self, id: i32) -> interfaces::Result<interfaces::Handle> {
        Ok(self.registry.component(id))
    }
    fn resizable(&mut self) -> interfaces::Result<bool> {
        let p = self.current()?;
        Ok(self.canonical(&p).client.is_resizable())
    }
    fn send(&mut self, frame: Frame) -> interfaces::Result<()> {
        let p = self.current()?;
        self.queue_native(&p, frame);
        Ok(())
    }
    fn log(&mut self, message: String) -> interfaces::Result<()> {
        self.log_line(format!("InterfaceManager {message}"));
        Ok(())
    }
    fn listener_open(&mut self, _: &interfaces::Handle) -> interfaces::Result<()> {
        self.not_ported
            .skip("InterfaceListeners.runOpen (content interface listeners)");
        Ok(())
    }
    fn listener_close(&mut self, _: &interfaces::Handle) -> interfaces::Result<bool> {
        // InterfaceListeners.runClose answers true when no listener is
        // registered for the interface; none is (content).
        self.not_ported
            .default_answer("InterfaceListeners.runClose (no content listener: true)");
        Ok(true)
    }
    fn plugin_open(
        &mut self,
        _: interfaces::Plugin,
        _: &interfaces::Handle,
    ) -> interfaces::Result<()> {
        self.not_ported
            .skip("ComponentPlugin.open (content component plugins)");
        Ok(())
    }
    fn close_event(
        &mut self,
        _: interfaces::CloseEvent,
        _: &interfaces::Handle,
    ) -> interfaces::Result<bool> {
        Err(interfaces::Error::Binding(
            "not ported: CloseEvent.close of content in the live world".into(),
        ))
    }
    fn open_component(
        &mut self,
        m: &interfaces::Manager,
        c: &interfaces::Handle,
    ) -> interfaces::Result<()> {
        let p = self.current()?;
        let weapon = self
            .canonical(&p)
            .weapon
            .as_ref()
            .is_some_and(|w| w.same(c));
        let friends = self
            .canonical(&p)
            .friends
            .as_ref()
            .is_some_and(|f| f.same(c));
        if weapon {
            self.not_ported
                .skip("WeaponInterface.open (core, not ported: attack tab not sent)");
            return Ok(());
        }
        if friends {
            self.not_ported
                .skip("FriendsListInterface.open (content: friends tab not sent)");
            return Ok(());
        }
        interfaces::component_open(self, m, c)
    }
    fn dispatch_open(&mut self, _: interfaces::Handle) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(
            "not ported: interface open dispatch to content".into(),
        ))
    }
    fn set_varp(&mut self, id: i32, value: i32) -> interfaces::Result<()> {
        let p = self.current()?;
        self.variables(&p, |ctx, owner| {
            variables::set_varp(ctx, Some(owner), id, value, false)
        })
        .map_err(ui_error)
    }
    fn weapon_extension(&mut self) -> interfaces::Result<Option<interfaces::Handle>> {
        let p = self.current()?;
        Ok(self.canonical(&p).weapon.clone())
    }
    fn create_weapon(&mut self) -> interfaces::Result<interfaces::Handle> {
        let handle = self.registry.component(92);
        handle.write().weapon = true;
        Ok(handle)
    }
    fn add_weapon(&mut self, component: interfaces::Handle) -> interfaces::Result<()> {
        let p = self.current()?;
        self.slot_mut(&p).canonical.weapon = Some(component);
        Ok(())
    }
    fn spellbook(&mut self) -> interfaces::Result<i32> {
        let p = self.current()?;
        self.with_profile(&p, |profile| profile.spell_book)
            .map_err(ui_error)
    }
    fn has_familiar(&mut self) -> interfaces::Result<bool> {
        self.not_ported
            .default_answer("FamiliarManager.hasFamiliar (not ported: false)");
        Ok(false)
    }
    fn friends_component(&mut self) -> interfaces::Result<interfaces::Handle> {
        let p = self.current()?;
        let handle = self.registry.component(550);
        self.slot_mut(&p).canonical.friends = Some(handle.clone());
        Ok(handle)
    }
    fn autocast(&mut self) -> interfaces::Result<Option<usize>> {
        let p = self.current()?;
        let autocast = self
            .with_profile(&p, |profile| profile.persisted.spells.autocast.is_some())
            .map_err(ui_error)?;
        if autocast {
            return Err(interfaces::Error::Binding(
                "not ported: autocast spell selection in the live world".into(),
            ));
        }
        Ok(None)
    }
    fn auto_spell_id(&mut self, _: &interfaces::Handle, _: usize) -> interfaces::Result<i32> {
        Err(interfaces::Error::Binding(
            "not ported: autocast spell ids".into(),
        ))
    }
    fn select_auto_spell(&mut self, _: &interfaces::Handle, _: i32) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(
            "not ported: autocast selection".into(),
        ))
    }
    fn window_mode(&mut self) -> interfaces::Result<i32> {
        let p = self.current()?;
        Ok(self.canonical(&p).client.window_mode)
    }
    fn set_window_mode(&mut self, mode: i32) -> interfaces::Result<()> {
        let p = self.current()?;
        self.slot_mut(&p).canonical.client.window_mode = mode;
        Ok(())
    }
    fn tutorial_complete(&mut self) -> interfaces::Result<bool> {
        Err(interfaces::Error::Binding(
            "not ported: tutorial island content".into(),
        ))
    }
    fn tutorial_tabs(&mut self) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(
            "not ported: tutorial island content".into(),
        ))
    }
    fn has_runscript(&mut self) -> interfaces::Result<bool> {
        let p = self.current()?;
        Ok(self
            .canonical(&p)
            .player
            .attributes()
            .get("runscript")
            .is_some())
    }
}

impl Inner {
    fn current(&self) -> interfaces::Result<P> {
        self.current.clone().ok_or(interfaces::Error::Binding(
            "no current player for the interface context".into(),
        ))
    }
}

// ---------------------------------------------------- social::Host

fn host_error(e: impl std::fmt::Debug) -> social::Error {
    social::Error::Host(format!("{e:?}"))
}

impl social::Host<P> for Inner {
    fn name(&mut self, p: &P) -> String {
        self.slot(p).name.clone()
    }
    fn with_details<R>(&mut self, p: &P, f: impl FnOnce(&mut PlayerDetails) -> R) -> R {
        self.canonical(p)
            .player
            .with_profile(|profile| f(&mut profile.details))
            .expect("a bound canonical profile")
    }
    fn private_chat_setting(&mut self, p: &P) -> i32 {
        self.canonical(p)
            .player
            .settings()
            .fields
            .read()
            .private_chat_setting
    }
    fn is_ai(&mut self, _: &P) -> bool {
        // Bots are not spawned by this world (enable_bots is content).
        false
    }
    fn is_artificial(&mut self, _: &P) -> bool {
        false
    }
    fn has_session(&mut self, p: &P) -> bool {
        self.slot(p).session.is_some()
    }
    fn is_resizable(&mut self, p: &P) -> bool {
        self.canonical(p).client.is_resizable()
    }
    fn world_id(&mut self) -> i32 {
        self.constants.world_id
    }
    fn world_name(&mut self) -> String {
        self.settings.world_name.clone()
    }
    fn dev_mode(&mut self) -> bool {
        self.settings.dev_mode
    }
    fn enable_default_clan(&mut self) -> bool {
        self.settings.enable_default_clan
    }
    fn server_name(&mut self) -> String {
        self.constants.server_name.clone()
    }
    fn ms_secret_key(&mut self) -> String {
        self.settings.ms_secret_key.clone()
    }
    fn global_chat_enabled(&mut self) -> bool {
        self.settings.global_chat
    }
    fn ms_enabled(&mut self) -> bool {
        // The pinned Server.kt never connects to a management server.
        false
    }
    fn ms_session(&mut self) -> bool {
        false
    }
    fn player_by_name(&mut self, name: &str) -> Option<P> {
        let wanted = rs09_game_core::java_lower::lowercase_root(name).replace(' ', "_");
        self.repository
            .name_entries()
            .into_iter()
            .find(|(n, _)| n.as_deref() == Some(wanted.as_str()))
            .map(|(_, p)| p)
    }
    fn player_names(&mut self) -> Vec<String> {
        self.repository
            .name_entries()
            .into_iter()
            .filter_map(|(n, _)| n)
            .collect()
    }
    fn players(&mut self) -> Vec<P> {
        self.repository
            .players()
            .occupied()
            .into_iter()
            .map(|(_, p)| p)
            .collect()
    }
    fn current_time_millis(&mut self) -> i64 {
        (self.clock)()
    }
    fn random_seed(&mut self) -> i64 {
        // `new java.util.Random()`'s seed: the host's entropy.
        now_millis() ^ 0x0005_DEEC_E66D
    }
    fn player_message(&mut self, p: &P, text: &str) -> social::Result<()> {
        self.message(p, text).map_err(host_error)
    }
    fn dispatch_message(&mut self, p: &P, text: &str) -> social::Result<()> {
        self.message(p, text).map_err(host_error)
    }
    fn api_message(&mut self, p: &P, text: &str) -> social::Result<()> {
        self.message(p, text).map_err(host_error)
    }
    fn send_string(&mut self, p: &P, text: &str, interface: i32, child: i32) -> social::Result<()> {
        self.text(p, text, interface, child).map_err(host_error)
    }
    fn open_component(&mut self, p: &P, id: i32) -> social::Result<()> {
        self.current = Some(p.clone());
        let manager = self.canonical(p).player.manager();
        interfaces::open_id(self, &manager, id)
            .map(|_| ())
            .map_err(host_error)
    }
    fn get_attribute(&mut self, p: &P, key: &str) -> social::Result<Option<social::Value>> {
        Ok(match self.canonical(p).player.attributes().get(key) {
            None => None,
            Some(AttributeValue::Int(v)) => Some(social::Value::Int(v)),
            Some(AttributeValue::Bool(v)) => Some(social::Value::Bool(v)),
            Some(AttributeValue::Text(v)) => Some(social::Value::Text(v)),
            Some(other) => {
                return Err(social::Error::Host(format!(
                    "attribute {key} holds {}",
                    other.java_string()
                )));
            }
        })
    }
    fn set_attribute(&mut self, p: &P, key: &str, value: social::Value) -> social::Result<()> {
        let attributes = self.canonical(p).player.attributes();
        match value {
            social::Value::Int(v) => attributes.set_value(key, Some(AttributeValue::Int(v))),
            social::Value::Bool(v) => attributes.set_value(key, Some(AttributeValue::Bool(v))),
            social::Value::Text(v) => attributes.set_value(key, Some(AttributeValue::Text(v))),
            social::Value::Rights(_) => {
                return Err(social::Error::Host(
                    "not ported: a Rights-valued attribute".into(),
                ));
            }
        }
        Ok(())
    }
    fn is_locked(&mut self, _: &P, key: &str) -> social::Result<bool> {
        // Player.getLocks() keyed locks: nothing in this world sets one.
        self.not_ported
            .default_answer("ActionLocks keyed lock query (no content sets keyed locks)");
        let _ = key;
        Ok(false)
    }
    fn lock(&mut self, _: &P, _: &str, _: i32) -> social::Result<()> {
        Err(social::Error::Host(
            "not ported: keyed action locks in the live world".into(),
        ))
    }
    fn register_chat(
        &mut self,
        p: &P,
        message: &social::ChatMessage<P>,
        _: bool,
    ) -> social::Result<bool> {
        let masks = self.canonical(p).player.masks();
        masks
            .chat(
                message.effects as u16,
                message.chat_icon as u8,
                chat_payload(message).map_err(host_error)?,
            )
            .map_err(host_error)
    }
    fn cipher(&mut self, _: &P) -> social::Result<i32> {
        // 2009scape's 530 transport runs without ISAAC opcode ciphering.
        Ok(0)
    }
    fn write(&mut self, p: &P, packet: social::Written) -> social::Result<()> {
        let frame = Frame {
            opcode: packet.opcode as u8,
            payload: packet.payload,
        };
        self.queue_native(p, frame);
        Ok(())
    }
    fn ms_write(&mut self, _: social::Written) -> social::Result<()> {
        Err(social::Error::Host("no management server session".into()))
    }
    fn varp(&mut self, p: &P, index: i32, value: i32, save: bool) -> social::Result<()> {
        self.variables(p, |ctx, owner| {
            variables::set_varp(ctx, Some(owner), index, value, save)
        })
        .map_err(host_error)
    }
    fn submit_pulse(&mut self, _: social::PulseId, _: i32) -> social::Result<()> {
        Err(social::Error::Host(
            "not ported: the loot-share pulse in the live world".into(),
        ))
    }
    fn stop_pulse(&mut self, _: social::PulseId) -> social::Result<()> {
        Err(social::Error::Host(
            "not ported: the loot-share pulse in the live world".into(),
        ))
    }
    fn log(&mut self, origin: &str, level: social::Level, message: &str) -> social::Result<()> {
        self.log_line(format!("{origin} {} {message}", level.name()));
        Ok(())
    }
    fn log_ms(&mut self, text: &str) -> social::Result<()> {
        self.log_line(format!("MS {text}"));
        Ok(())
    }
    fn log_chat(&mut self, _: &P, _: &str, _: &str) -> social::Result<()> {
        self.not_ported
            .skip("PlayerMonitor.logChat (player monitor not ported)");
        Ok(())
    }
    fn log_private_chat(&mut self, _: &P, _: &str, _: &str) -> social::Result<()> {
        self.not_ported
            .skip("PlayerMonitor.logPrivateChat (player monitor not ported)");
        Ok(())
    }
    fn account_info(&mut self, username: &str) -> social::Result<UserAccountInfo> {
        let account = self
            .storage
            .get_account_info(username, (self.clock)())
            .map_err(host_error)?;
        Ok(copy_account(&crate::auth::storage::lock(&account)))
    }
    fn online_friends(&mut self, username: &str) -> social::Result<Vec<String>> {
        self.storage.online_friends(username).map_err(host_error)
    }
    fn account_update(&mut self, info: &UserAccountInfo) -> social::Result<()> {
        self.storage
            .update(copy_account(info).shared())
            .map_err(host_error)
    }
    fn create_account(&mut self, _: &UserAccountInfo) -> social::Result<bool> {
        Err(social::Error::Host(
            "not ported: account creation from the social code".into(),
        ))
    }
    fn define_command(&mut self, _: &str, _: &str, _: &str, _: &str) -> social::Result<()> {
        self.not_ported
            .skip("Commands.define (the command system is not ported)");
        Ok(())
    }
    fn update_chat_settings(
        &mut self,
        p: &P,
        public: i32,
        private: i32,
        trade: i32,
    ) -> social::Result<()> {
        let settings = self.canonical(p).player.settings();
        let p = p.clone();
        settings
            .update_chat_values(
                &mut SettingsContext { inner: self, p },
                public,
                private,
                trade,
            )
            .map_err(host_error)
    }
    fn ai_incoming_chat(&mut self, _: &P, _: i32, _: &str, _: &str) -> social::Result<()> {
        Err(social::Error::Host("no artificial players".into()))
    }
    fn management_take(&mut self, event: &social::Event) {
        let text = format!("{event:?}");
        let name = text.split([' ', '{', '(']).next().unwrap_or_default();
        self.log_line(format!("ManagementEvents take {name}"));
    }
}

/// The chat mask's body as the original's Chat encoder builds it: the
/// character count, the wordpack-compressed text and one final zero (the
/// mask writer reverses it).
fn chat_payload(message: &social::ChatMessage<P>) -> Result<Vec<u8>, String> {
    let count = u8::try_from(message.num_chars).map_err(|_| "chat count".to_owned())?;
    if count > 127 {
        return Err("chat messages beyond 127 characters are not ported".into());
    }
    let mut payload = vec![count];
    payload.extend(
        rs09_protocol::game::chat::WORDPACK
            .encode(message.text.as_bytes())
            .map_err(|e| e.to_string())?,
    );
    payload.push(0);
    Ok(payload)
}

/// A copy of an account's fields (the social code reads and updates copies).
fn copy_account(info: &UserAccountInfo) -> UserAccountInfo {
    UserAccountInfo {
        fields: info.fields.clone(),
        initial_values: info.initial_values.clone(),
    }
}

// ---------------------------------------------------- Skills

/// skills::Context for Skills.refresh/configure and the tick.
pub struct SkillsContext<'a> {
    pub inner: &'a mut Inner,
    pub p: P,
}
impl skills::Context for SkillsContext<'_> {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        6
    }
    fn tick(&mut self) -> i32 {
        self.inner.ticks
    }
    fn world_tick(&mut self) -> i32 {
        self.inner.ticks
    }
    fn combat_level(&self) -> i32 {
        self.inner
            .with_profile(&self.p, |p| p.properties.combat_level)
            .unwrap_or(3)
    }
    fn summoning_combat_level(&self) -> i32 {
        self.inner
            .with_profile(&self.p, |p| p.summoning_combat_level)
            .unwrap_or(0)
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        Err(skills::Error::Binding)
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        // AssistSession: nobody assists in this world (not ported).
        self.inner
            .not_ported
            .default_answer("AssistSession (no assist sessions: none)");
        Ok(None)
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        Err(skills::Error::Binding)
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        Ok(false)
    }
    fn username(&mut self) -> skills::Result<String> {
        Ok(self.inner.slot(&self.p).name.clone())
    }
    fn permadeath_present(&self) -> bool {
        false
    }
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        match effect {
            Effect::Packet(skill) => {
                let level = match skill {
                    5 => state.prayer_points.ceil() as i32,
                    3 => state.lifepoints,
                    _ => state.level(skill as i32, true, self)?,
                };
                let frame = updates::skill_level(skill as i32, level, state.experience[skill]);
                self.inner.queue_native(&self.p, frame);
                Ok(())
            }
            Effect::FlashingIcons(_) => {
                self.inner
                    .not_ported
                    .skip("LevelUp.sendFlashingIcons (core, not ported)");
                Ok(())
            }
            Effect::Combat(level) => {
                self.inner
                    .with_profile(&self.p, |p| p.properties.combat_level = level)
                    .map_err(|_| skills::Error::Binding)?;
                Ok(())
            }
            Effect::SummoningCombat(level) => {
                self.inner
                    .with_profile(&self.p, |p| p.summoning_combat_level = level)
                    .map_err(|_| skills::Error::Binding)?;
                Ok(())
            }
            other => {
                self.inner.log_line(format!(
                    "not ported: skills effect {other:?} in the live world"
                ));
                Err(skills::Error::Binding)
            }
        }
    }
}

/// A placeholder Skills while the profile's own are taken out for a call.
pub fn placeholder_skills() -> Skills {
    struct Empty;
    impl skills::Context for Empty {
        fn kind(&self) -> EntityKind {
            EntityKind::Player
        }
        fn version(&self) -> i32 {
            6
        }
        fn tick(&mut self) -> i32 {
            0
        }
        fn world_tick(&mut self) -> i32 {
            0
        }
        fn combat_level(&self) -> i32 {
            3
        }
        fn summoning_combat_level(&self) -> i32 {
            0
        }
        fn npc_combat_level(&mut self) -> skills::Result<i32> {
            Err(skills::Error::Binding)
        }
        fn damage_spong_active(&mut self) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
            Err(skills::Error::Binding)
        }
        fn translate_experience(
            &mut self,
            _: &mut Skills,
            _: i32,
            _: f64,
            _: f64,
        ) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn brawling_present(&mut self) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
            Err(skills::Error::Binding)
        }
        fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn brawling_bonus(&mut self) -> skills::Result<f64> {
            Err(skills::Error::Binding)
        }
        fn tutorial_complete(&mut self) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn artificial(&mut self) -> skills::Result<bool> {
            Err(skills::Error::Binding)
        }
        fn username(&mut self) -> skills::Result<String> {
            Err(skills::Error::Binding)
        }
        fn permadeath_present(&self) -> bool {
            false
        }
        fn effect(&mut self, _: &mut Skills, _: Effect) -> skills::Result<()> {
            Ok(())
        }
    }
    Skills::new(&mut Empty).expect("default skills")
}

// ---------------------------------------------------- Settings

/// settings::Context: varps through the canonical variable store, chat
/// settings written at once, run energy queued.
pub struct SettingsContext<'a> {
    pub inner: &'a mut Inner,
    pub p: P,
}
impl settings::Context for SettingsContext<'_> {
    fn varp(&mut self, _: &Settings, id: i32, value: i32) -> settings::Result<()> {
        self.inner
            .variables(&self.p, |ctx, owner| {
                variables::set_varp(ctx, Some(owner), id, value, false)
            })
            .map_err(|_| settings::Error::Binding)
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(self.inner.slot(&self.p).session.map(|s| s as usize))
    }
    fn send(&mut self, _: &Settings, session: Option<usize>, frame: Frame) -> settings::Result<()> {
        let session = session.ok_or(settings::Error::Null)?;
        self.inner.direct_native(session as SessionId, frame);
        Ok(())
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn run_energy(&mut self, state: &Settings, _: Option<usize>) -> settings::Result<()> {
        let energy = state.fields.read().run_energy;
        self.inner
            .queue_native(&self.p, updates::run_energy(energy));
        Ok(())
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, text: &str) -> settings::Result<()> {
        self.inner
            .message(&self.p, text)
            .map_err(|_| settings::Error::Binding)
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> settings::Result<()> {
        self.inner
            .text(&self.p, &text, interface, child)
            .map_err(|_| settings::Error::Binding)
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        self.inner
            .with_profile(&self.p, |p| p.properties.retaliating)
            .map_err(|_| settings::Error::Binding)
    }
    fn set_retaliating(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        value: bool,
    ) -> settings::Result<()> {
        self.inner
            .with_profile(&self.p, |p| p.properties.retaliating = value)
            .map_err(|_| settings::Error::Binding)
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        Err(settings::Error::Binding)
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        Err(settings::Error::Binding)
    }
}

/// settings::Context for Settings.updateWeight over the canonical containers.
pub struct WeightContext<'a> {
    pub inner: &'a mut Inner,
    pub p: P,
    pub item: Option<ItemHandle>,
}
impl settings::Context for WeightContext<'_> {
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn send(&mut self, _: &Settings, _: Option<usize>, _: Frame) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> settings::Result<()> {
        self.inner
            .text(&self.p, &text, interface, child)
            .map_err(|_| settings::Error::Binding)
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn container(
        &mut self,
        _: &Settings,
        kind: settings::ContainerKind,
    ) -> settings::Result<usize> {
        Ok(match kind {
            settings::ContainerKind::Inventory => 0,
            settings::ContainerKind::Equipment => 1,
        })
    }
    fn item(
        &mut self,
        _: &Settings,
        container: usize,
        slot: i32,
    ) -> settings::Result<Option<usize>> {
        let player = &self.inner.canonical(&self.p).player;
        self.item = match container {
            0 => player.inventory().get(slot),
            1 => player.equipment().get(slot),
            _ => return Err(settings::Error::Binding),
        };
        Ok(self.item.as_ref().map(|_| 0))
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        Err(settings::Error::Binding)
    }
}

// ---------------------------------------------------- Equipment

/// equipment::Context for EquipmentListener.refresh: the container packet and
/// the weight are native; WeaponInterface is not ported.
pub struct EquipmentContext<'a> {
    pub inner: &'a mut Inner,
    pub p: P,
}
fn binding<E: std::fmt::Debug>(e: E) -> inventory::Error {
    inventory::Error::Binding(format!("{e:?}"))
}
fn unbound(what: &str) -> inventory::Error {
    inventory::Error::Binding(format!("not ported in the live world: {what}"))
}
impl inventory::container::Context for EquipmentContext<'_> {
    fn update(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: Event,
        _: &Items,
    ) -> inventory::Result<()> {
        Err(unbound("container update from the equipment refresh"))
    }
    fn refresh(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: &Items,
    ) -> inventory::Result<()> {
        Err(unbound("container refresh from the equipment refresh"))
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<ItemHandle>,
        _: &Items,
    ) -> inventory::Result<()> {
        Err(unbound("ground drop from the equipment refresh"))
    }
}
impl equipment::Context for EquipmentContext<'_> {
    type Dispatch = ();
    type Properties = ();
    type Weapon = ();
    type CombatPulse = ();
    type Settings = Settings;
    type Appearance = ();
    type Interfaces = interfaces::Manager;
    fn inventory(&mut self, _: &NodeHandle) -> inventory::Result<Container> {
        Ok(self.inner.canonical(&self.p).player.inventory())
    }
    fn equipment(&mut self, _: &NodeHandle) -> inventory::Result<Container> {
        Ok(self.inner.canonical(&self.p).player.equipment())
    }
    fn requirements(
        &mut self,
        _: crate::definitions::item::Handle,
        _: Option<&NodeHandle>,
        _: bool,
        _: bool,
    ) -> inventory::Result<bool> {
        Err(unbound("equipment requirements"))
    }
    fn dispatch(&mut self, _: &NodeHandle) -> inventory::Result<()> {
        Ok(())
    }
    fn message(&mut self, _: (), text: &str) -> inventory::Result<()> {
        self.inner.message(&self.p, text).map_err(binding)
    }
    fn text(
        &mut self,
        _: (),
        text: Option<String>,
        interface: i32,
        child: i32,
    ) -> inventory::Result<()> {
        self.inner
            .text(
                &self.p,
                &text.unwrap_or_else(|| "null".into()),
                interface,
                child,
            )
            .map_err(binding)
    }
    fn unequip_plugin(
        &mut self,
        _: crate::definitions::plugin::Handle,
        _: &NodeHandle,
        _: ItemHandle,
    ) -> inventory::Result<crate::definitions::property::Property> {
        Err(unbound("unequip plugins (content)"))
    }
    fn interaction(
        &mut self,
        _: i32,
        _: &NodeHandle,
        _: ItemHandle,
        _: bool,
    ) -> inventory::Result<bool> {
        Err(unbound("equipment interaction listeners (content)"))
    }
    fn send(&mut self, packet: equipment::Packet, _: &mut Container) -> inventory::Result<()> {
        let null = self.inner.factory.null_item.clone();
        let frame = packet.frame(&null)?;
        self.inner.queue_native(&self.p, frame);
        Ok(())
    }
    fn properties(&mut self, _: &NodeHandle) -> inventory::Result<()> {
        Ok(())
    }
    fn attack_speed(&mut self, _: (), speed: i32) -> inventory::Result<()> {
        self.inner
            .with_profile(&self.p, |p| p.properties.attack_speed = speed)
            .map_err(binding)
    }
    fn extension(&mut self, _: &NodeHandle) -> inventory::Result<Option<()>> {
        Ok(self.inner.canonical(&self.p).weapon.as_ref().map(|_| ()))
    }
    fn update_weapon(&mut self, _: ()) -> inventory::Result<()> {
        self.inner
            .not_ported
            .skip("WeaponInterface.updateInterface (core, not ported)");
        Ok(())
    }
    fn defence_animation(&mut self, _: ()) -> inventory::Result<()> {
        self.inner
            .not_ported
            .skip("Properties.updateDefenceAnimation (not ported)");
        Ok(())
    }
    fn cape_perks(&mut self, _: &NodeHandle, _: &mut Container, _: usize) -> inventory::Result<()> {
        Err(unbound("skillcape perks (content)"))
    }
    fn attribute_bool(
        &mut self,
        _: &NodeHandle,
        key: &str,
        default: bool,
    ) -> inventory::Result<bool> {
        self.inner
            .canonical(&self.p)
            .player
            .attributes()
            .boolean(key, default)
            .map_err(binding)
    }
    fn remove_attribute(&mut self, _: &NodeHandle, key: &str) -> inventory::Result<()> {
        self.inner
            .canonical(&self.p)
            .player
            .attributes()
            .remove(key);
        Ok(())
    }
    fn combat_pulse(&mut self, _: ()) -> inventory::Result<()> {
        Err(unbound("the combat pulse"))
    }
    fn clear_combat_handler(&mut self, _: ()) -> inventory::Result<()> {
        Err(unbound("the combat handler"))
    }
    fn settings(&mut self, _: &NodeHandle) -> inventory::Result<Settings> {
        Ok(self.inner.canonical(&self.p).player.settings())
    }
    fn special_toggled(&mut self, settings: Settings) -> inventory::Result<bool> {
        Ok(settings.fields.read().special_toggled)
    }
    fn set_varp(&mut self, _: &NodeHandle, id: i32, value: i32) -> inventory::Result<()> {
        self.inner
            .variables(&self.p, |ctx, owner| {
                variables::set_varp(ctx, Some(owner), id, value, false)
            })
            .map_err(binding)
    }
    fn appearance(&mut self, _: &NodeHandle) -> inventory::Result<()> {
        Ok(())
    }
    fn set_animations(&mut self, _: ()) -> inventory::Result<()> {
        self.inner
            .not_ported
            .skip("Appearance.setAnimations (equipment render animations not joined)");
        Ok(())
    }
    fn update_appearance(&mut self, _: &NodeHandle) -> inventory::Result<()> {
        let masks = self.inner.canonical(&self.p).player.masks();
        masks
            .sync_appearance(now_millis)
            .map(|_| ())
            .map_err(binding)
    }
    fn update_weight(&mut self, settings: Settings) -> inventory::Result<()> {
        let mut ctx = WeightContext {
            inner: &mut *self.inner,
            p: self.p.clone(),
            item: None,
        };
        settings.update_weight(&mut ctx).map_err(binding)
    }
    fn perk(&mut self, _: &NodeHandle, _: equipment::Perk) -> inventory::Result<bool> {
        self.inner
            .not_ported
            .default_answer("skillcape perks (skillcape_perks off: false)");
        Ok(false)
    }
    fn set_bonuses(&mut self, _: (), bonuses: Shared<Vec<i32>>) -> inventory::Result<()> {
        let bonuses = bonuses.read().clone();
        self.inner
            .with_profile(&self.p, |p| p.properties.bonuses = bonuses)
            .map_err(binding)
    }
    fn bonuses(&mut self, _: ()) -> inventory::Result<Shared<Vec<i32>>> {
        let bonuses = self
            .inner
            .with_profile(&self.p, |p| p.properties.bonuses.clone())
            .map_err(binding)?;
        Ok(Shared::new(bonuses))
    }
    fn interfaces(&mut self, _: &NodeHandle) -> inventory::Result<interfaces::Manager> {
        Ok(self.inner.canonical(&self.p).player.manager())
    }
    fn has_main(&mut self, manager: interfaces::Manager, id: i32) -> inventory::Result<bool> {
        Ok(interfaces::has_main_component(&manager, id))
    }
}

// ---------------------------------------------------- interaction options

/// interaction_options::Context for InteractPlugin.setDefault on a player.
pub struct OptionsContext<'a> {
    pub inner: &'a mut Inner,
    pub p: P,
}
fn option_error(what: &str) -> interaction_options::Error {
    interaction_options::Error::Binding(format!("not ported in the live world: {what}"))
}
impl interaction_options::Context for OptionsContext<'_> {
    type Definition = ();
    fn kind(&self, node: Option<Node>) -> interaction_options::Kind {
        if node == Some(self.p.node()) {
            interaction_options::Kind::Player
        } else {
            interaction_options::Kind::Other
        }
    }
    fn node_id(&mut self, _: Node) -> interaction_options::Result<i32> {
        Err(option_error("node ids for options"))
    }
    fn item_has_location(&mut self, _: Node) -> interaction_options::Result<bool> {
        Err(option_error("item locations for options"))
    }
    fn definition(&mut self, _: Node) -> interaction_options::Result<Option<()>> {
        Err(option_error("definitions for options"))
    }
    fn names(
        &mut self,
        _: &(),
        _: interaction_options::NamesKind,
    ) -> interaction_options::Result<Option<interaction_options::Names>> {
        Err(option_error("option names"))
    }
    fn lowercase(&mut self, name: &str) -> interaction_options::Result<String> {
        Ok(rs09_game_core::java_lower::lowercase_root(name))
    }
    fn option_handler(
        &mut self,
        _: interaction_options::Kind,
        _: i32,
        _: &str,
    ) -> interaction_options::Result<Option<usize>> {
        Err(option_error("option handlers (content)"))
    }
    fn send_option(
        &mut self,
        _: Node,
        index: i32,
        name: Option<String>,
    ) -> interaction_options::Result<()> {
        let name = name.unwrap_or_else(|| "null".into());
        self.inner
            .queue_native(&self.p, interaction_option(index, &name));
        Ok(())
    }
    fn node_string(&mut self, _: Option<Node>) -> interaction_options::Result<String> {
        Err(option_error("node descriptions"))
    }
    fn constants(&self) -> &interaction_options::Constants {
        &self.inner.options
    }
}

// ---------------------------------------------------- the save file's services

/// The services that stay outside the canonical player during a save or a
/// load: content hooks and house/familiar parsing are not ported.
pub struct Rest {
    pub hooks: usize,
    pub ticks: i32,
    pub skipped: Vec<&'static str>,
}
impl Remaining for Rest {
    fn log(&mut self, level: SaveLevel, message: &str) {
        eprintln!("[live] PlayerSaveParser {level:?} {message}");
    }
    fn retaliating(&mut self) -> save::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn attribute_set(&mut self, _: &str, _: &AttributeValue) -> save::Result<()> {
        // AttributeSetEvent: Entity event hooks are not ported.
        self.skipped
            .push("AttributeSetEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn content_hooks(&mut self) -> usize {
        self.hooks
    }
    fn save_content(
        &mut self,
        _: usize,
        _: &mut rs09_game_core::java_json::Json,
    ) -> save::Result<()> {
        self.skipped
            .push("PersistPlayer content hook save (content, writes nothing)");
        Ok(())
    }
    fn parse_content(&mut self, _: usize, _: &rs09_game_core::java_json::Json) -> save::Result<()> {
        self.skipped
            .push("PersistPlayer content hook parse (content, reads nothing)");
        Ok(())
    }
    fn parse_familiars(&mut self, _: &rs09_game_core::java_json::Json) -> save::Result<()> {
        self.skipped.push("FamiliarManager.parse (not ported)");
        Ok(())
    }
    fn parse_house(&mut self, _: &rs09_game_core::java_json::Json) -> save::Result<()> {
        self.skipped.push("HouseManager.parse (not ported)");
        Ok(())
    }
}
impl inventory::container::Context for Rest {
    fn update(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: Event,
        _: &Items,
    ) -> inventory::Result<()> {
        // A container listener during a load: nothing is sent before login.
        Ok(())
    }
    fn refresh(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: &Items,
    ) -> inventory::Result<()> {
        Ok(())
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<ItemHandle>,
        _: &Items,
    ) -> inventory::Result<()> {
        Err(unbound("ground drop during a load"))
    }
}
impl skills::Context for Rest {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        6
    }
    fn tick(&mut self) -> i32 {
        self.ticks
    }
    fn world_tick(&mut self) -> i32 {
        self.ticks
    }
    fn combat_level(&self) -> i32 {
        unreachable!("answered by the canonical player")
    }
    fn summoning_combat_level(&self) -> i32 {
        unreachable!("answered by the canonical player")
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        Err(skills::Error::Binding)
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        Err(skills::Error::Binding)
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        Ok(false)
    }
    fn username(&mut self) -> skills::Result<String> {
        Err(skills::Error::Binding)
    }
    fn permadeath_present(&self) -> bool {
        unreachable!("the parser supplies the permadeath attribute")
    }
    fn effect(&mut self, _: &mut Skills, _: Effect) -> skills::Result<()> {
        // Skill packets during a load: nothing is sent before login.
        Ok(())
    }
}
impl settings::Context for Rest {
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn send(&mut self, _: &Settings, _: Option<usize>, _: Frame) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: String,
        _: i32,
        _: i32,
    ) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!("answered by the canonical player")
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        unreachable!("answered by the canonical player")
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        unreachable!("answered by the canonical player")
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        Err(settings::Error::Binding)
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        Err(settings::Error::Binding)
    }
}

/// Timer content in a save: no player timer type is ported (the three native
/// timer prototypes refuse their player branches, docs/startup.md). An
/// unknown identifier is the original's `TimerRegistry` answer for a class it
/// does not hold: null, logged by the timer manager and dropped.
#[derive(Default)]
pub struct LiveTimers {
    pub skipped: Vec<&'static str>,
}
impl timers::Context<CoreData> for LiveTimers {
    fn is_player(&self, _: &CoreData) -> bool {
        true
    }
    fn modal_open(&mut self, _: &mut CoreData) -> timers::Result<bool> {
        Err(timers::Error::Type)
    }
    fn script_delay(&mut self, _: &mut CoreData) -> timers::Result<i32> {
        Err(timers::Error::Type)
    }
    fn ticks(&mut self, _: &mut CoreData) -> timers::Result<i32> {
        Err(timers::Error::Type)
    }
    fn entity_name(&mut self, _: &mut CoreData) -> timers::Result<Option<String>> {
        Ok(None)
    }
    fn before_register(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Err(timers::Error::Type)
    }
    fn on_register(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Err(timers::Error::Type)
    }
    fn on_removal(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        Err(timers::Error::Type)
    }
    fn run(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<bool> {
        Err(timers::Error::Type)
    }
    fn initial_delay(
        &mut self,
        _: &timers::Handle,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<i32> {
        Err(timers::Error::Type)
    }
    fn save(
        &mut self,
        timer: &timers::Handle,
        root: &mut crate::definitions::item_config::json::Json,
        _: &mut timers::Manager,
        state: &mut CoreData,
    ) -> timers::Result<()> {
        timers::save_base(timer, root, state, self)
    }
    fn parse(
        &mut self,
        timer: &timers::Handle,
        data: &crate::definitions::item_config::json::Json,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<()> {
        timers::parse_base(timer, data)
    }
    fn resolve(
        &mut self,
        _: &str,
        _: &mut timers::Manager,
        _: &mut CoreData,
    ) -> timers::Result<Option<timers::Handle>> {
        self.skipped
            .push("TimerRegistry player timer types (content timers not ported: dropped)");
        Ok(None)
    }
    fn run_failure(&mut self, _: &str, _: Option<&str>, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
    fn unknown_timer(&mut self, _: &str, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
    fn exception(&mut self, _: &timers::Error, _: &mut CoreData) -> timers::Result<()> {
        Ok(())
    }
}
