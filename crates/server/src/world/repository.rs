//! `Repository.kt`: the player and NPC slot lists, the name and uid maps, the
//! renderable NPC and lobby lists and the helpers around them.
//!
//! A library: nothing installs it. The lists and maps are native; everything
//! a player, NPC, session or other system does is a required service of
//! [`Host`], called in the original order and never answered here. The queue
//! of leaving players is [`super::disconnection`].
//!
//! `E` is the handle of a player or NPC. Equality on it must be identity, as
//! it is for the canonical node leases of [`super::player_walk`]. A slot index
//! says where a node sits now; a retained handle is never looked up again by
//! the index it once had.
pub mod hash_map;
pub mod node_list;

use super::{disconnection, disconnection::DisconnectionQueue, update::List};
use crate::definitions::item::Shared;
pub use hash_map::JavaHashMap;
pub use node_list::{NodeList, NodeListIter};

/// `ServerConstants.MAX_PLAYERS`.
pub const MAX_PLAYERS: i32 = 2000;
/// `ServerConstants.MAX_NPCS`.
pub const MAX_NPCS: i32 = 32000;
/// `GE_NEWS_MUTE_ATTRIBUTE`.
pub const GE_NEWS_MUTE_ATTRIBUTE: &str = "/save:ge:news-mute";
/// The default icon of `sendNews` and `sendGrandExchangeNews`.
pub const NEWS_ICON: i32 = 12;
/// Their default colour.
pub const NEWS_COLOR: &str = "CC6600";
/// Native bound on the capacity of a [`NodeList`]; the original has none.
pub const MAX_CAPACITY: i32 = 1 << 20;

/// What stops a call. The first six are the exceptions the ported code raises
/// itself; `Io`, `Exception` and `Throwable` are what a service raised, kept
/// apart because the original catches them differently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `IndexOutOfBoundsException` from `NodeList.get`.
    IndexOutOfBounds,
    /// `ArrayIndexOutOfBoundsException`.
    ArrayIndexOutOfBounds,
    /// `NegativeArraySizeException`.
    NegativeArraySize,
    /// `NullPointerException`.
    NullPointer,
    /// `NoSuchElementException`.
    NoSuchElement,
    /// `IllegalStateException`.
    IllegalState,
    /// `IllegalArgumentException`: a negative `HashMap` capacity.
    IllegalArgument,
    /// A service failed with an `IOException`.
    Io(String),
    /// A service failed with any other `Exception`.
    Exception(String),
    /// A service failed with a `Throwable` that is not an `Exception`.
    Throwable(String),
    /// Native bound, see [`MAX_CAPACITY`]. Nothing in the original catches it.
    Capacity,
}

impl Error {
    /// Whether `catch (Throwable)` stops it.
    pub fn is_throwable(&self) -> bool {
        !matches!(self, Self::Capacity)
    }
    /// Whether `catch (Exception)` stops it.
    pub fn is_exception(&self) -> bool {
        !matches!(self, Self::Throwable(_) | Self::Capacity)
    }
    /// Whether `catch (IOException)` stops it.
    pub fn is_io(&self) -> bool {
        matches!(self, Self::Io(_))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// `IoSession.object`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionObject<E> {
    Null,
    Player(E),
    /// Anything that is not a `Player`.
    Other,
}

/// The two levels the queue logs at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
}

/// What the repository, the queue and the parts of `Player`, `PacketDispatch`
/// and `IoSession` ported with them need from systems that are not ported.
///
/// There are no defaults. Methods without a `Result` are field reads and
/// writes of the original that cannot fail; every other method is a call
/// into another system and may fail with what that system threw.
pub trait Host<E> {
    /// `IoSession`.
    type Session;
    /// `ClanRepository`.
    type Clan;
    /// `Location`.
    type Location;

    // Node and player fields.
    /// `player.getDetails().getUid()`.
    fn uid(&mut self, player: &E) -> i32;
    /// `node.getName()`; a player's is the name it was constructed with.
    fn name(&mut self, node: &E) -> Option<String>;
    /// `player.getDetails().getUsername()`.
    fn details_username(&mut self, player: &E) -> Option<String>;
    /// `node.setIndex(index)`.
    fn set_index(&mut self, node: &E, index: i32);
    /// `player.isArtificial()`.
    fn is_artificial(&mut self, player: &E) -> bool;
    /// The `isAfkLogout` field, a nullable `Boolean`.
    fn afk_logout(&mut self, player: &E) -> Option<bool>;
    /// `player.setPlaying(playing)`.
    fn set_playing(&mut self, player: &E, playing: bool);
    /// `player.getSession()`.
    fn session(&mut self, player: &E) -> Option<Self::Session>;
    /// `GameWorld.ticks`.
    fn ticks(&mut self) -> i32;
    /// `ServerConstants.ENABLE_GLOBAL_CHAT`.
    fn global_chat_enabled(&mut self) -> bool;
    /// `String.toLowerCase()` in the JVM's default locale.
    fn to_lower_case(&mut self, text: &str) -> String;

    // The session, as far as `IoSession.disconnect` and `getPlayer` use it.
    fn session_active(&mut self, session: &Self::Session) -> bool;
    fn set_session_active(&mut self, session: &Self::Session, active: bool);
    /// `key != null`.
    fn session_has_key(&mut self, session: &Self::Session) -> bool;
    /// `key.cancel()`.
    fn key_cancel(&mut self, session: &Self::Session) -> Result<()>;
    /// `((SocketChannel) key.channel()).socket().close()`.
    fn socket_close(&mut self, session: &Self::Session) -> Result<()>;
    fn session_object(&mut self, session: &Self::Session) -> SessionObject<E>;
    /// `object = player`, or `object = null`.
    fn set_session_object(&mut self, session: &Self::Session, player: Option<E>);
    /// `associatedUsername`.
    fn session_username(&mut self, session: &Self::Session) -> Option<String>;

    // Calls into the player and its systems.
    /// `player.clear()`, a virtual call. For a plain `Player` it is
    /// [`disconnection::player_clear`]; for an `AIPlayer` it is
    /// `bots::AiPlayer::clear`. `bots::ai_player::virtual_clear` is both.
    fn player_clear(&mut self, repository: &Repository<E>, player: &E) -> Result<()>;
    /// `UpdateSequence.renderablePlayers.remove(player)`.
    fn renderable_players_remove(&mut self, player: &E) -> Result<bool>;
    /// `inCombat()`.
    fn in_combat(&mut self, player: &E) -> Result<bool>;
    /// `getSkills().getLifepoints()`.
    fn lifepoints(&mut self, player: &E) -> Result<i32>;
    /// `DeathTask.isDead(player)`.
    fn is_dead(&mut self, player: &E) -> Result<bool>;
    /// `isTeleporting()`.
    fn is_teleporting(&mut self, player: &E) -> Result<bool>;
    /// `scripts.hasTypeInQueue(QueueStrength.SOFT)`.
    fn has_soft_script(&mut self, player: &E) -> Result<bool>;
    /// `PacketRepository.send(Logout.class, context)`.
    fn logout_packet(&mut self, player: &E) -> Result<()>;
    /// `GameWorld.getLogoutListeners().forEach(it -> it.logout(player))`.
    fn world_logout_listeners(&mut self, player: &E) -> Result<()>;
    /// `getWalkingQueue().reset()`.
    fn walking_queue_reset(&mut self, player: &E) -> Result<()>;
    /// The player's own `logoutListeners`, each invoked with the player, if
    /// the map is not empty.
    fn player_logout_listeners(&mut self, player: &E) -> Result<()>;
    /// `familiarManager.hasFamiliar()`.
    fn has_familiar(&mut self, player: &E) -> Result<bool>;
    /// `familiarManager.getFamiliar().clear()`.
    fn familiar_clear(&mut self, player: &E) -> Result<()>;
    /// `interfaceManager.close()`.
    fn interface_close(&mut self, player: &E) -> Result<()>;
    /// `interfaceManager.closeSingleTab()`.
    fn interface_close_single_tab(&mut self, player: &E) -> Result<()>;
    /// `Entity.clear()`.
    fn entity_clear(&mut self, player: &E) -> Result<()>;
    /// `getZoneMonitor().clear()`.
    fn zone_monitor_clear(&mut self, player: &E) -> Result<()>;
    /// `HouseManager.leave(player)`.
    fn house_leave(&mut self, player: &E) -> Result<()>;
    /// `ManagementEvents.publish` of a `PlayerStatusUpdate`.
    fn publish_player_status(
        &mut self,
        username: &str,
        world: i32,
        notify_friends_only: bool,
    ) -> Result<()>;
    /// `getCommunication().getClan()`.
    fn clan(&mut self, player: &E) -> Result<Option<Self::Clan>>;
    /// `GameWorld.getSettings().getWorldId()`.
    fn world_id(&mut self) -> Result<i32>;
    /// `clan.getOwner()`.
    fn clan_owner(&mut self, clan: &Self::Clan) -> Result<Option<String>>;
    /// `ManagementEvents.publish` of a `ClanLeaveNotification`.
    fn publish_clan_leave(&mut self, username: &str, world: i32, clan_name: &str) -> Result<()>;
    /// `checkForWealthUpdate(force)`, whose body is not ported.
    fn wealth_update(&mut self, player: &E, force: bool) -> Result<()>;
    /// `clan.leave(player, remove)`.
    fn clan_leave(&mut self, clan: &Self::Clan, player: &E, remove: bool) -> Result<()>;
    /// `details.save()`.
    fn details_save(&mut self, player: &E) -> Result<()>;

    // Saving and logging.
    /// `TaskExecutor.executeSQL { .. }`: the hand-off of a save. Whoever takes
    /// it runs [`disconnection::SaveTask::run`] later, off the tick.
    fn execute_sql(&mut self, task: disconnection::SaveTask<E>) -> Result<()>;
    /// `Thread.currentThread().name = name`, in the thread running the task.
    fn set_thread_name(&mut self, name: &str);
    /// `PlayerParser.saveImmediately(player)`.
    fn save_immediately(&mut self, player: &E) -> Result<()>;
    /// `log(DisconnectionQueue::class.java, level, message)`.
    fn log(&mut self, level: LogLevel, message: &str) -> Result<()>;
    /// `throwable.printStackTrace()`.
    fn print_stack_trace(&mut self, error: &Error);

    // NPCs and news.
    /// `npc is RevenantNPC`.
    fn is_revenant(&mut self, npc: &E) -> bool;
    /// `npc.setRenderable(renderable)`.
    fn set_renderable(&mut self, npc: &E, renderable: bool);
    /// `npc.getId()`.
    fn npc_id(&mut self, npc: &E) -> i32;
    /// `RegionManager.getRegionChunk(location).npcs`, as it is when asked.
    fn region_chunk_npcs(&mut self, location: &Self::Location) -> Result<Vec<E>>;
    /// `npc.location == location`.
    fn npc_at(&mut self, npc: &E, location: &Self::Location) -> bool;
    /// `player.getAttribute(GE_NEWS_MUTE_ATTRIBUTE, false)`.
    fn ge_news_muted(&mut self, player: &E) -> Result<bool>;
    /// `sendMessage(player, message)`.
    fn send_message(&mut self, player: &E, message: &str) -> Result<()>;
}

struct Maps<E> {
    uids: JavaHashMap<i32, E>,
    names: JavaHashMap<Option<String>, E>,
}

/// The `Repository` object. A shared handle: clones are the same repository,
/// and no lock is held while a service runs, so a service may call back in.
pub struct Repository<E> {
    players: NodeList<E>,
    npcs: NodeList<E>,
    maps: Shared<Maps<E>>,
    logged_in_players: Shared<Vec<String>>,
    renderable_npcs: List<E>,
    lobby_players: List<E>,
    disconnection_queue: DisconnectionQueue<E>,
}

impl<E> Clone for Repository<E> {
    fn clone(&self) -> Self {
        Self {
            players: self.players.clone(),
            npcs: self.npcs.clone(),
            maps: self.maps.clone(),
            logged_in_players: self.logged_in_players.clone(),
            renderable_npcs: self.renderable_npcs.clone(),
            lobby_players: self.lobby_players.clone(),
            disconnection_queue: self.disconnection_queue.clone(),
        }
    }
}

impl<E: Clone + PartialEq> Repository<E> {
    /// The object as its initialiser builds it from `MAX_PLAYERS` and
    /// `MAX_NPCS`: the uid map starts with room for `max_players`.
    pub fn new(max_players: i32, max_npcs: i32) -> Result<Self> {
        Ok(Self {
            players: NodeList::new(max_players)?,
            maps: Shared::new(Maps {
                uids: JavaHashMap::with_capacity(max_players).ok_or(Error::IllegalArgument)?,
                names: JavaHashMap::new(),
            }),
            npcs: NodeList::new(max_npcs)?,
            logged_in_players: Shared::new(Vec::new()),
            renderable_npcs: List::copy_on_write(),
            lobby_players: List::plain(),
            disconnection_queue: DisconnectionQueue::default(),
        })
    }

    /// `Repository.players`.
    pub fn players(&self) -> NodeList<E> {
        self.players.clone()
    }
    /// `Repository.npcs`.
    pub fn npcs(&self) -> NodeList<E> {
        self.npcs.clone()
    }
    /// `LOGGED_IN_PLAYERS`, which the pinned sources declare and never use.
    pub fn logged_in_players(&self) -> Shared<Vec<String>> {
        self.logged_in_players.clone()
    }
    /// `RENDERABLE_NPCS` and `renderableNpcs`: one copy-on-write list.
    pub fn renderable_npcs(&self) -> List<E> {
        self.renderable_npcs.clone()
    }
    /// `lobbyPlayers`, a plain list.
    pub fn lobby_players(&self) -> List<E> {
        self.lobby_players.clone()
    }
    /// `disconnectionQueue`.
    pub fn disconnection_queue(&self) -> DisconnectionQueue<E> {
        self.disconnection_queue.clone()
    }

    /// `uid_map` in the order the map is walked.
    pub fn uid_entries(&self) -> Vec<(i32, E)> {
        let maps = self.maps.read();
        maps.uids
            .iter()
            .map(|(uid, player)| (*uid, player.clone()))
            .collect()
    }
    /// `uid_map.contains(uid)`.
    pub fn contains_uid(&self, uid: i32) -> bool {
        self.maps.read().uids.contains_key(&uid)
    }
    /// `playerNames` in the order the map is walked.
    pub fn name_entries(&self) -> Vec<(Option<String>, E)> {
        let maps = self.maps.read();
        maps.names
            .iter()
            .map(|(name, player)| (name.clone(), player.clone()))
            .collect()
    }

    /// `addPlayer`. A player with the same uid is looked for only in slots
    /// `1 .. size - 1`; the first one found is taken out of the list and
    /// cleared. The new player then takes the first free slot if there is
    /// one, and is entered in both maps either way. A player whose name is
    /// null fails with a NullPointerException once the uid map is written.
    pub fn add_player<H: Host<E>>(&self, host: &mut H, player: &E) -> Result<()> {
        if !self.players.is_empty() {
            let size = self.players.size();
            let mut index = 1;
            while index < size {
                if let Some(current) = self.players.get(index)? {
                    if host.uid(&current) == host.uid(player) {
                        self.players.remove(Some(&current));
                        host.player_clear(self, &current)?;
                        break;
                    }
                }
                index += 1;
            }
        }
        if let Some(index) = self.players.add(player.clone()) {
            host.set_index(player, index);
        }
        let uid = host.uid(player);
        self.maps.write().uids.put(uid, player.clone());
        // Kotlin checks the platform-typed name before it becomes a key of a
        // Map<String, Player>: a null name fails here, after the list and the
        // uid map were written.
        let name = host.name(player).ok_or(Error::NullPointer)?;
        self.maps.write().names.put(Some(name), player.clone());
        Ok(())
    }

    /// `removePlayer`: the list, both maps by the player's current uid and
    /// name, the renderable players, then the session.
    pub fn remove_player<H: Host<E>>(&self, host: &mut H, player: &E) -> Result<()> {
        self.players.remove(Some(player));
        let uid = host.uid(player);
        self.maps.write().uids.remove(&uid);
        let name = host.name(player);
        self.maps.write().names.remove(&name);
        host.renderable_players_remove(player)?;
        let session = host.session(player).ok_or(Error::NullPointer)?;
        disconnection::session_disconnect(host, self, &session)
    }

    /// `getPlayerByName`: lower case, spaces to underscores, then the map.
    pub fn get_player_by_name<H: Host<E>>(&self, host: &mut H, name: Option<&str>) -> Option<E> {
        let key = host.to_lower_case(name?).replace(' ', "_");
        self.maps.read().names.get(&Some(key)).cloned()
    }

    /// `getPlayerByUid`.
    pub fn get_player_by_uid(&self, uid: i32) -> Option<E> {
        self.maps.read().uids.get(&uid).cloned()
    }

    /// `sendNews(string, icon, color, isGeNews)`: one message per value of
    /// the name map, in the order the map is walked when the call starts.
    pub fn send_news<H: Host<E>>(
        &self,
        host: &mut H,
        text: &str,
        icon: i32,
        color: &str,
        ge_news: bool,
    ) -> Result<()> {
        if !host.global_chat_enabled() {
            return Ok(());
        }
        let players: Vec<E> = {
            let maps = self.maps.read();
            maps.names
                .iter()
                .map(|(_, player)| player.clone())
                .collect()
        };
        for player in players {
            if ge_news && host.ge_news_muted(&player)? {
                continue;
            }
            host.send_message(&player, &format!("<img={icon}><col={color}>News: {text}"))?;
        }
        Ok(())
    }

    /// `sendNews(string)` and `sendNews(string, icon, color)` with defaults.
    pub fn send_default_news<H: Host<E>>(&self, host: &mut H, text: &str) -> Result<()> {
        self.send_news(host, text, NEWS_ICON, NEWS_COLOR, false)
    }

    /// `sendGrandExchangeNews` with its defaults.
    pub fn send_grand_exchange_news<H: Host<E>>(&self, host: &mut H, text: &str) -> Result<()> {
        self.send_news(host, text, NEWS_ICON, NEWS_COLOR, true)
    }

    /// `addRenderableNPC`.
    pub fn add_renderable_npc<H: Host<E>>(&self, host: &mut H, npc: &E) {
        if host.is_revenant(npc) {
            return;
        }
        if !self.renderable_npcs.contains(npc) {
            self.renderable_npcs.add(npc.clone());
            host.set_renderable(npc, true);
        }
    }

    /// `removeRenderableNPC`: the flag is cleared whether or not the list
    /// held the NPC.
    pub fn remove_renderable_npc<H: Host<E>>(&self, host: &mut H, npc: &E) {
        if host.is_revenant(npc) {
            return;
        }
        self.renderable_npcs.remove(npc);
        host.set_renderable(npc, false);
    }

    /// `findNPC(location)`: the first NPC of the location's chunk standing
    /// on it.
    pub fn find_npc_at<H: Host<E>>(
        &self,
        host: &mut H,
        location: &H::Location,
    ) -> Result<Option<E>> {
        for npc in host.region_chunk_npcs(location)? {
            if host.npc_at(&npc, location) {
                return Ok(Some(npc));
            }
        }
        Ok(None)
    }

    /// `findNPC(npcId)`: the first NPC with the id, in slot order, over a
    /// copy of the list made when the call starts.
    pub fn find_npc<H: Host<E>>(&self, host: &mut H, npc_id: i32) -> Result<Option<E>> {
        let mut nodes = self.npcs.iter()?;
        while nodes.has_next() {
            let npc = nodes.next_node()?;
            if host.npc_id(&npc) == npc_id {
                return Ok(Some(npc));
            }
        }
        Ok(None)
    }
}
