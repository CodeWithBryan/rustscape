//! The original start-up, `GameWorld.prompt(run = false)`, as an ordered list
//! of steps, each run natively or, where content is missing, in part.
//!
//! [`ORDER`] is the order of the statements of `prompt` (with
//! `ConfigParser.prePlugin`/`postPlugin` expanded loader by loader), which is
//! also the phase order of `fixtures/startup-loaders`. A step that is only
//! partly ported ([`Step::refusal`] says what it leaves out) is never reported
//! as done: with [`Policy::StopAtRefusal`] the start-up stops before it with
//! [`StartupError::NotPorted`]; with [`Policy::ContinuePastRefusals`] its
//! native part runs, the rest is reported by name in [`Outcome::Partial`], a
//! line is logged and the result is marked incomplete. A failure inside a
//! step is the original's uncaught exception: the start-up stops with
//! [`StartupError::Failed`].
//!
//! The [`World`] this produces holds what the native steps loaded. It is not a
//! playable world: content (most of the class scan, plugins, start-up
//! listeners, content NPC classes) and the rest listed in docs/startup.md are
//! not ported.
use super::generators::{Positions, Seeds, WorldGenerators};
use super::ground::{self, Chunks, GroundItems};
use super::loaders::{
    self, LoadError, Log, clue_rewards::ClueRewards, music, ranged::RangedTables, xtea::RegionKeys,
};
use super::npcs::{self, Npcs};
use super::regions::Regions;
use super::scan::{self, Manifest, Native};
use super::store::{self, ServerStore, WorldWrites};
use super::timers::{Entity as TimerEntity, Natives};
use crate::auth::{factory::Registry as AuthRegistry, password::Passwords};
use crate::config::Configuration;
use crate::definitions::{item, item_config, npc, property::Property, scenery, varbit};
use crate::timers::{Manager as TimerManager, registry::Registry as TimerRegistry};
use crate::world::drop_tables::{LogLevel, NpcDropTable, parser as drop_parser};
use crate::world::{
    chunk_sync::DirtyTracker, doors, ground_options, ground_spawn_loader, interfaces, npc_spawns,
    player_walk::State,
};
use rs09_cache::{
    CacheError, Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use std::{path::PathBuf, sync::Arc};

/// One statement of `GameWorld.prompt` (or of what it calls).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    ServerConfig,
    PromptLog,
    CacheInit,
    ChunkDirtyListener,
    AuthConfigure,
    NewConfigParser,
    NpcConfig,
    ItemConfig,
    ObjectConfig,
    Xtea,
    ObjectOverride,
    InterfaceConfig,
    ScanClasspath,
    LoadPureInterfaces,
    LoadTimers,
    ServerStore,
    WorldPersists,
    LoadSideEffectfulPlugins,
    Shops,
    DropTables,
    NpcSpawner,
    Doors,
    GroundSpawns,
    Music,
    Ranged,
    CustomVarbits,
    ClueRewards,
    StartupListeners,
    SceneryExamine,
    PreloadMap,
    Gc,
}

/// The original's order.
pub const ORDER: [Step; 31] = [
    Step::ServerConfig,
    Step::PromptLog,
    Step::CacheInit,
    Step::ChunkDirtyListener,
    Step::AuthConfigure,
    Step::NewConfigParser,
    Step::NpcConfig,
    Step::ItemConfig,
    Step::ObjectConfig,
    Step::Xtea,
    Step::ObjectOverride,
    Step::InterfaceConfig,
    Step::ScanClasspath,
    Step::LoadPureInterfaces,
    Step::LoadTimers,
    Step::ServerStore,
    Step::WorldPersists,
    Step::LoadSideEffectfulPlugins,
    Step::Shops,
    Step::DropTables,
    Step::NpcSpawner,
    Step::Doors,
    Step::GroundSpawns,
    Step::Music,
    Step::Ranged,
    Step::CustomVarbits,
    Step::ClueRewards,
    Step::StartupListeners,
    Step::SceneryExamine,
    Step::PreloadMap,
    Step::Gc,
];

impl Step {
    /// The phase name `fixtures/startup-loaders` records for this step. The
    /// start-up listeners are one step here and one phase per listener there
    /// (`startup:` followed by the class name).
    pub const fn phase(self) -> &'static str {
        match self {
            Self::ServerConfig => "ServerConfigParser.parse",
            Self::PromptLog => "prompt.log",
            Self::CacheInit => "Cache.init",
            Self::ChunkDirtyListener => "RegionChunk.dirtyListener",
            Self::AuthConfigure => "Auth.configure",
            Self::NewConfigParser => "ConfigParser()",
            Self::NpcConfig => "NPCConfigParser",
            Self::ItemConfig => "ItemConfigParser",
            Self::ObjectConfig => "ObjectConfigParser",
            Self::Xtea => "XteaParser",
            Self::ObjectOverride => "ObjectOverrideParser",
            Self::InterfaceConfig => "InterfaceConfigParser",
            Self::ScanClasspath => "ClassScanner.scanClasspath",
            Self::LoadPureInterfaces => "ClassScanner.loadPureInterfaces",
            Self::LoadTimers => "ClassScanner.loadTimers",
            Self::ServerStore => "ServerStore.parse",
            Self::WorldPersists => "worldPersists.parse",
            Self::LoadSideEffectfulPlugins => "ClassScanner.loadSideEffectfulPlugins",
            Self::Shops => "ShopParser",
            Self::DropTables => "DropTableParser",
            Self::NpcSpawner => "NPCSpawner",
            Self::Doors => "DoorConfigLoader",
            Self::GroundSpawns => "GroundSpawnLoader",
            Self::Music => "MusicConfigLoader",
            Self::Ranged => "RangedConfigLoader",
            Self::CustomVarbits => "CustomVarbitParser",
            Self::ClueRewards => "ClueRewardParser",
            Self::StartupListeners => "startup:",
            Self::SceneryExamine => "SceneryDefinition.examine",
            Self::PreloadMap => "PRELOAD_MAP",
            Self::Gc => "System.gc",
        }
    }
    /// What a step leaves out; `None` for a step this sequence runs entirely
    /// natively. Every such step is partly native: its native part runs and
    /// the rest is reported by name ([`Outcome::Partial`]).
    pub const fn refusal(self) -> Option<&'static str> {
        Some(match self {
            Self::LoadPureInterfaces => {
                "ClassScanner.loadPureInterfaces: only ServerStore and GraveController (world persists) are native; every other content interface (listeners, interactions, interfaces, commands, behaviours, map areas, world events) is content not ported"
            }
            Self::LoadTimers => {
                "ClassScanner.loadTimers: only GrandExchangeTimer, AntiMacro and HolidayRandoms have native prototypes; the other scanned timers are not ported"
            }
            Self::LoadSideEffectfulPlugins => {
                "ClassScanner.loadSideEffectfulPlugins: only PickupPlugin is native; every other legacy plugin is content not ported"
            }
            Self::NpcSpawner => {
                "NPCSpawner: every NPC is a base NPC (content NPC classes are not ported); zone updates, NPC interaction defaults, slayer tasks, magic autocast spells and the SkillRestore timer are not ported"
            }
            Self::GroundSpawns => {
                "GroundSpawnLoader: ground items' option tables hold only the natively registered item option handler (take); content registers the others"
            }
            Self::StartupListeners => {
                "startup listeners: every start-up listener the class scan registers is content not ported"
            }
            _ => return None,
        })
    }
}

/// A scanned class or a member the original runs that this port does not have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gap {
    pub what: String,
    pub cause: &'static str,
}
const CONTENT: &str = "content not ported";

/// What a refused step does to the start-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    StopAtRefusal,
    ContinuePastRefusals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Native,
    /// The native part ran; the rest is listed.
    Partial(Vec<Gap>),
    NotPorted(&'static str),
}

/// One step as it ran: its outcome, the log lines it produced (as the
/// original prints them, `[Class] message`) and the generator positions after it.
#[derive(Clone, Debug)]
pub struct StepReport {
    pub step: Step,
    pub outcome: Outcome,
    pub log: Vec<String>,
    pub positions: Positions,
}

#[derive(Debug)]
pub enum StartupError {
    NotPorted { step: Step, reason: &'static str },
    Failed { step: Step, error: String },
}
impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StartupError {}

/// The host inputs of a start-up.
pub struct Inputs {
    /// The world directory (the original's working directory): holds
    /// `worldprops/default.conf`, and relative data/cache paths resolve here.
    pub world: PathBuf,
    /// The generator seeds; the original takes them from its host.
    pub seeds: Seeds,
    /// Bound on decoded cache definitions per archive.
    pub max_definitions: usize,
    /// Where the start-up's own writes in the world directory go (the
    /// original creates `STORE_PATH` when it is missing).
    pub writes: Arc<dyn WorldWrites>,
    /// The JVM's default-locale `String.toLowerCase()` (en-US in the
    /// world-trace oracle, which lowercases as the ROOT locale does).
    pub lower_case: fn(&str) -> String,
}

/// A log line: origin (simple class name), level, message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    pub origin: &'static str,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Default)]
struct Lines(Vec<LogLine>);
impl Log for Lines {
    fn log(&mut self, origin: &'static str, level: LogLevel, message: &str) {
        self.0.push(LogLine {
            origin,
            level,
            message: message.into(),
        });
    }
}

/// What the native steps loaded.
pub struct World {
    pub generators: WorldGenerators,
    pub settings: Option<Configuration>,
    pub config_path: Option<String>,
    pub cache: Option<Store>,
    pub items: item::Registry,
    pub scenery: scenery::Registry,
    pub npcs: npc::Registry,
    npc_archive: Option<Arc<Archive>>,
    pub interfaces: interfaces::Registry,
    pub region_keys: RegionKeys,
    pub regions: Regions,
    pub doors: doors::Registry,
    pub songs: music::Songs,
    pub ranged: RangedTables,
    pub varbits: varbit::Registry,
    pub clue_rewards: ClueRewards,
    pub log: Vec<LogLine>,
    /// `RegionChunk.dirtyListener`: `None` is the original's initial no-op
    /// listener; the `RegionChunk.dirtyListener` step installs the
    /// `ChunkUpdateTracker` port.
    pub dirty_listener: Option<DirtyTracker>,
    /// `Auth.authenticator` / `Auth.storageProvider`.
    pub auth: Option<AuthRegistry>,
    /// `ClassScanner.scanResults`: the native manifest.
    pub scan: Option<Manifest>,
    /// `GameWorld.worldPersists`, in registration order.
    pub world_persists: Vec<Native>,
    /// `ServerStore.fileMap` / `counter`.
    pub store: ServerStore,
    /// `TimerRegistry`.
    pub timers: TimerRegistry,
    /// `registerTimer`'s log lines while NPCs get their automatic timers.
    pub timer_log: Vec<(crate::timers::registry::Level, String)>,
    /// `ItemDefinition.OPTION_HANDLERS` (and the arena binding them to ground
    /// items).
    pub item_options: ground_options::Registry,
    /// `ClassScanner.amountLoaded` and `numPlugins`.
    pub content_interfaces_loaded: i32,
    pub legacy_plugins: i32,
    /// Canonical actors: NPCs and the nodes ground items are bound to.
    pub actors: State,
    /// The NPCs Repository.npcs holds.
    pub entities: Npcs,
    pub ground: GroundItems,
    /// Region chunks `RegionManager.getRegionChunk` created.
    pub chunks: Chunks,
    /// `GameWorld.ticks`.
    pub ticks: i32,
}

/// The result of a start-up that did not stop.
pub struct Startup {
    pub world: World,
    pub steps: Vec<StepReport>,
    /// True only when every step ran natively.
    pub complete: bool,
}

struct NpcFiles<'a> {
    archive: Option<&'a Archive>,
    lines: &'a mut Lines,
}
impl npc::Files for NpcFiles<'_> {
    fn file(&mut self, _: &mut npc::Registry, id: i32) -> npc::Result<Option<Vec<u8>>> {
        let archive = self
            .archive
            .ok_or_else(|| npc::Error::Binding("cache not initialised".into()))?;
        Ok(archive
            .file((id as u32) >> 7, (id & 127) as u32, [0; 4])?
            .map(|b| b.as_ref().clone()))
    }
}
impl npc::config::Context for NpcFiles<'_> {
    fn log(
        &mut self,
        _: &mut npc::Registry,
        level: npc::config::Level,
        text: String,
    ) -> npc::Result<()> {
        let level = match level {
            npc::config::Level::Fine => LogLevel::Fine,
            npc::config::Level::Warn => LogLevel::Warn,
        };
        self.lines.log("NPCConfigParser", level, &text);
        Ok(())
    }
}

struct SceneryLog<'a>(&'a mut Lines);
impl scenery::LogContext for SceneryLog<'_> {
    fn log(&mut self, _: &mut scenery::Registry, message: String) -> scenery::Result<()> {
        self.0.log("SceneryDefinition", LogLevel::Err, &message);
        Ok(())
    }
}

struct ItemConfigLog<'a>(&'a mut Lines);
impl item_config::Logger for ItemConfigLog<'_> {
    fn parsed(&mut self, count: i32) -> item_config::Result<()> {
        self.0.log(
            "ItemConfigParser",
            LogLevel::Fine,
            &format!("Parsed {count} item configs."),
        );
        Ok(())
    }
}

struct DropDefinitions<'a> {
    npcs: &'a mut npc::Registry,
    files: NpcFiles<'a>,
}
impl drop_parser::Definitions for DropDefinitions<'_> {
    fn assign(
        &mut self,
        id: i32,
        table: &Arc<NpcDropTable>,
    ) -> Result<bool, drop_parser::LoadError> {
        let def = self
            .npcs
            .for_id(&mut self.files, id)
            .map_err(|e| drop_parser::LoadError::Definition(format!("{e:?}")))?;
        let mut def = def.write();
        match def.drop_tables.as_mut() {
            None => Ok(false),
            Some(tables) => {
                tables.table = Arc::clone(table);
                Ok(true)
            }
        }
    }
    fn name(&mut self, id: i32) -> Result<Option<String>, drop_parser::LoadError> {
        let def = self
            .npcs
            .for_id(&mut self.files, id)
            .map_err(|e| drop_parser::LoadError::Definition(format!("{e:?}")))?;
        let name = def.read().name.clone();
        Ok(name)
    }
    fn log(&mut self, level: LogLevel, message: &str) {
        self.files.lines.log("DropTableParser", level, message);
    }
}

fn fail(step: Step) -> impl Fn(String) -> StartupError {
    move |error| StartupError::Failed { step, error }
}

fn debug<E: std::fmt::Debug>(error: E) -> String {
    format!("{error:?}")
}

impl World {
    fn new(seeds: Seeds) -> Result<Self, StartupError> {
        Ok(Self {
            generators: WorldGenerators::new(seeds),
            settings: None,
            config_path: None,
            cache: None,
            items: item::Registry::default(),
            scenery: scenery::Registry::default(),
            npcs: npc::Registry::default(),
            npc_archive: None,
            interfaces: interfaces::Registry::default(),
            region_keys: RegionKeys::default(),
            regions: Regions::default(),
            doors: doors::Registry::default(),
            songs: music::Songs::default(),
            ranged: RangedTables::default(),
            varbits: varbit::Registry::default(),
            clue_rewards: ClueRewards::default(),
            log: Vec::new(),
            dirty_listener: None,
            auth: None,
            scan: None,
            world_persists: Vec::new(),
            store: ServerStore::default(),
            timers: TimerRegistry::default(),
            timer_log: Vec::new(),
            item_options: ground_options::Registry::default(),
            content_interfaces_loaded: 0,
            legacy_plugins: 0,
            actors: State::default(),
            entities: Npcs::new().map_err(|error| StartupError::Failed {
                step: Step::ServerConfig,
                error,
            })?,
            ground: GroundItems::default(),
            chunks: Chunks::default(),
            ticks: 0,
        })
    }
    fn constant_bool(&self, key: &str) -> Option<bool> {
        self.settings
            .as_ref()
            .and_then(|s| s.constants().get(key))
            .and_then(|v| v.as_bool())
    }
    fn manifest(&self, step: Step) -> Result<&Manifest, StartupError> {
        // `lateinit var scanResults`: read before scanClasspath it throws.
        self.scan.as_ref().ok_or(StartupError::Failed {
            step,
            error: "UninitializedPropertyAccessException: scanResults".into(),
        })
    }
    fn timer_entity(&mut self) -> TimerEntity<'_> {
        TimerEntity {
            player: false,
            artificial: false,
            ticks: self.ticks,
            holiday_event_randoms: false,
            generators: &mut self.generators,
            log: &mut self.timer_log,
        }
    }
    fn archive(&self, id: u8) -> Result<Archive, String> {
        let store = self.cache.clone().ok_or("cache not initialised")?;
        Archive::open(store, id, None, Limits::default()).map_err(debug)
    }
    fn prefix(&self) -> Option<&str> {
        self.config_path.as_deref()
    }
    fn config_text(&self, name: &str) -> Result<String, LoadError> {
        loaders::read_config(self.prefix(), name)
    }
    /// `NPCDefinition.forId(id)` against this world's cache.
    pub fn npc(&mut self, id: i32) -> npc::Result<npc::Handle> {
        let mut lines = Lines::default();
        let mut files = NpcFiles {
            archive: self.npc_archive.as_deref(),
            lines: &mut lines,
        };
        self.npcs.for_id(&mut files, id)
    }

    fn run(
        &mut self,
        inputs: &Inputs,
        step: Step,
        lines: &mut Lines,
    ) -> Result<Vec<Gap>, StartupError> {
        let failed = fail(step);
        let mut gaps = Vec::new();
        match step {
            Step::ServerConfig => {
                let path = inputs.world.join("worldprops").join("default.conf");
                let settings = Configuration::load(&path).map_err(|e| failed(debug(e)))?;
                let resolve = |key: &str| -> Option<String> {
                    let value = settings.constants().get(key)?.as_str()?.to_string();
                    Some(if std::path::Path::new(&value).is_absolute() {
                        value
                    } else {
                        format!(
                            "{}{}{value}",
                            inputs.world.display(),
                            std::path::MAIN_SEPARATOR
                        )
                    })
                };
                self.config_path = resolve("CONFIG_PATH");
                let cache = resolve("CACHE_PATH").ok_or_else(|| failed("no CACHE_PATH".into()))?;
                self.settings = Some(settings);
                lines.0.push(LogLine {
                    origin: "startup",
                    level: LogLevel::Fine,
                    message: format!("cache {cache}"),
                });
            }
            Step::PromptLog => {
                let name = self
                    .settings
                    .as_ref()
                    .and_then(|s| s.world())
                    .and_then(|w| {
                        w.values()
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                    });
                lines.log(
                    "GameWorld",
                    LogLevel::Fine,
                    &format!(
                        "Prompting {} Game World...",
                        name.as_deref().unwrap_or("null")
                    ),
                );
            }
            Step::CacheInit => {
                lines.log("Cache", LogLevel::Fine, "Initializing cache...");
                let settings = self
                    .settings
                    .as_ref()
                    .ok_or_else(|| failed("no settings".into()))?;
                let cache = settings
                    .constants()
                    .get("CACHE_PATH")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| failed("no CACHE_PATH".into()))?;
                let cache = if std::path::Path::new(cache).is_absolute() {
                    PathBuf::from(cache)
                } else {
                    inputs.world.join(cache)
                };
                let store = Store::open(&cache, StorageMode::Resident, Limits::default())
                    .map_err(|e| failed(debug(e)))?;
                self.cache = Some(store);
                let items = self.archive(19).map_err(&failed)?;
                self.items
                    .load_archive(&items, inputs.max_definitions)
                    .map_err(|e: CacheError| failed(debug(e)))?;
                let objects = self.archive(16).map_err(&failed)?;
                self.scenery
                    .load_archive(&mut SceneryLog(lines), &objects, inputs.max_definitions)
                    .map_err(|e| failed(debug(e)))?;
                self.npc_archive = Some(Arc::new(self.archive(18).map_err(&failed)?));
            }
            Step::NewConfigParser | Step::Gc => {}
            Step::NpcConfig => {
                let text = self
                    .config_text("npc_configs.json")
                    .map_err(|e| failed(debug(e)))?;
                let mut files = NpcFiles {
                    archive: self.npc_archive.as_deref(),
                    lines,
                };
                npc::config::Parser::default()
                    .load_text(&text, &mut self.npcs, &mut files)
                    .map_err(|e| failed(debug(e)))?;
            }
            Step::ItemConfig => {
                let text = self
                    .config_text("item_configs.json")
                    .map_err(|e| failed(debug(e)))?;
                item_config::Parser::default()
                    .load_text(&text, &self.items, &mut ItemConfigLog(lines))
                    .map_err(|e| failed(debug(e)))?;
            }
            Step::ObjectConfig => {
                loaders::object_config::load_config(
                    self.config_path.as_deref(),
                    &mut self.scenery,
                    lines,
                )
                .map_err(|e| failed(debug(e)))?;
            }
            Step::Xtea => {
                loaders::xtea::load_config(
                    self.config_path.as_deref(),
                    &mut self.region_keys,
                    lines,
                )
                .map_err(|e| failed(debug(e)))?;
            }
            Step::ObjectOverride => {
                loaders::object_override::load_config(
                    self.config_path.as_deref(),
                    &mut self.scenery,
                    &mut self.regions,
                    lines,
                )
                .map_err(|e| failed(debug(e)))?;
            }
            Step::InterfaceConfig => {
                let text = self
                    .config_text("interface_configs.json")
                    .map_err(|e| failed(debug(e)))?;
                let count = self.interfaces.load(&text).map_err(|e| failed(debug(e)))?;
                lines.log(
                    "InterfaceConfigParser",
                    LogLevel::Fine,
                    &format!("Parsed {count} interface configs."),
                );
            }
            Step::Shops => {
                loaders::shops::load_config(self.config_path.as_deref(), lines)
                    .map_err(|e| failed(debug(e)))?;
            }
            Step::DropTables => {
                let text = self
                    .config_text("drop_tables.json")
                    .map_err(|e| failed(debug(e)))?;
                let mut definitions = DropDefinitions {
                    npcs: &mut self.npcs,
                    files: NpcFiles {
                        archive: self.npc_archive.as_deref(),
                        lines,
                    },
                };
                drop_parser::load(&text, &mut self.generators, &mut definitions)
                    .map_err(|e| failed(debug(e)))?;
            }
            Step::Doors => {
                let text = self
                    .config_text("door_configs.json")
                    .map_err(|e| failed(debug(e)))?;
                doors::Loader::default()
                    .load_text(&text, &self.doors, &mut |count| {
                        lines.log(
                            "DoorConfigLoader",
                            LogLevel::Fine,
                            &format!("Parsed {count} door configs."),
                        );
                        Ok(())
                    })
                    .map_err(|e| failed(debug(e)))?;
            }
            Step::Music => {
                let maps = self.archive(17).map_err(&failed)?;
                let read = |id: i32| -> Result<music::DataMap, String> {
                    let data = maps
                        .file((id as u32) >> 8, (id & 255) as u32, [0; 4])
                        .map_err(debug)?;
                    music::DataMap::parse(id, data.as_deref().map(Vec::as_slice)).map_err(debug)
                };
                let song_ids = read(1351).map_err(&failed)?;
                let names = read(1345).map_err(&failed)?;
                music::load_config(
                    self.config_path.as_deref(),
                    music::SongMaps {
                        song_ids: &song_ids,
                        names: &names,
                    },
                    &mut self.songs,
                    &mut self.regions,
                    lines,
                )
                .map_err(|e| failed(debug(e)))?;
            }
            Step::Ranged => {
                loaders::ranged::load_config(
                    self.config_path.as_deref(),
                    &self.items,
                    &mut self.ranged,
                    lines,
                )
                .map_err(|e| failed(debug(e)))?;
            }
            Step::CustomVarbits => {
                let text = self
                    .config_text("varbit_definitions.json")
                    .map_err(|e| failed(debug(e)))?;
                let count = varbit::Loader::load_text(&self.varbits, &text)
                    .map_err(|e| failed(debug(e)))?;
                lines.log(
                    "CustomVarbitParser",
                    LogLevel::Fine,
                    &format!("Parsed {count} custom varbit definitions."),
                );
            }
            Step::ClueRewards => {
                loaders::clue_rewards::load_config(
                    self.config_path.as_deref(),
                    &mut self.clue_rewards,
                    &mut self.generators,
                )
                .map_err(|e| failed(debug(e)))?;
            }
            Step::PreloadMap => {
                let preload = self
                    .settings
                    .as_ref()
                    .and_then(|s| s.constants().get("PRELOAD_MAP"))
                    .and_then(|v| v.as_bool())
                    .ok_or_else(|| failed("no PRELOAD_MAP".into()))?;
                if preload {
                    return Err(StartupError::NotPorted {
                        step,
                        reason: "PRELOAD_MAP: loading regions 7483..15420 is not ported",
                    });
                }
            }
            Step::ChunkDirtyListener => {
                // RegionChunk.dirtyListener = ChunkUpdateTracker.
                self.dirty_listener = Some(DirtyTracker::default());
            }
            Step::AuthConfigure => {
                let settings = self
                    .settings
                    .as_ref()
                    .ok_or_else(|| failed("no settings".into()))?;
                // SQLStorageProvider's friend lookup is Repository.getPlayerByName:
                // the world's player name map, keyed by the lower-cased name
                // with spaces as underscores.
                let repository = self.entities.repository.clone();
                let lower = inputs.lower_case;
                let online = move |name: &str| {
                    let key = Some(lower(name).replace(' ', "_"));
                    repository.name_entries().iter().any(|(n, _)| *n == key)
                };
                let mut registry = AuthRegistry::default();
                registry
                    .configure(
                        &settings.auth_settings(),
                        Arc::new(Passwords::default()),
                        online,
                    )
                    .map_err(|e| failed(e.to_string()))?;
                self.auth = Some(registry);
            }
            Step::ScanClasspath => {
                self.scan = Some(Manifest::load());
            }
            Step::LoadPureInterfaces => {
                let classes = self.manifest(step)?.content_interfaces.clone();
                for class in classes {
                    match scan::native(class) {
                        Some(native @ (Native::ServerStore | Native::GraveController)) => {
                            // `if (clazz is PersistWorld) GameWorld.worldPersists.add(clazz)`
                            self.world_persists.push(native);
                            self.content_interfaces_loaded += 1;
                        }
                        _ => gaps.push(Gap {
                            what: class.into(),
                            cause: CONTENT,
                        }),
                    }
                }
                lines.log(
                    "SystemLogger",
                    LogLevel::Fine,
                    &format!(
                        "[STARTUP] Loaded {} content interfaces.",
                        self.content_interfaces_loaded
                    ),
                );
            }
            Step::LoadTimers => {
                let classes = self.manifest(step)?.timers.clone();
                for class in classes {
                    if scan::is_abstract(class) {
                        continue;
                    }
                    match scan::native(class) {
                        Some(Native::Timer(kind)) => {
                            let prototype = kind.instance();
                            let timers = self.timers.clone();
                            let mut manager = TimerManager::default();
                            let mut entity = self.timer_entity();
                            timers
                                .register(Some(&prototype), &mut manager, &mut entity, &mut Natives)
                                .map_err(|e| failed(format!("{e:?}")))?;
                        }
                        _ => gaps.push(Gap {
                            what: class.into(),
                            cause: CONTENT,
                        }),
                    }
                }
                for (level, message) in self.timer_log.drain(..) {
                    let level = match level {
                        crate::timers::registry::Level::Warn => LogLevel::Warn,
                        crate::timers::registry::Level::Error => LogLevel::Err,
                    };
                    lines.log("TimerRegistry", level, &message);
                }
            }
            Step::ServerStore => {
                // worldPersists.first { it is ServerStore }.parse()
                if !self.world_persists.contains(&Native::ServerStore) {
                    return Err(failed(
                        "NoSuchElementException: List contains no element matching the predicate."
                            .into(),
                    ));
                }
                let path = self
                    .settings
                    .as_ref()
                    .and_then(|s| s.constants().get("STORE_PATH"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| failed("NullPointerException: STORE_PATH".into()))?;
                let dir = inputs.world.join(path);
                store::parse(&mut self.store, &dir, inputs.writes.as_ref(), lines)
                    .map_err(|e| failed(debug(e)))?;
            }
            Step::WorldPersists => {
                for persist in self.world_persists.clone() {
                    match persist {
                        Native::ServerStore => {}
                        Native::GraveController => {
                            store::parse_graves(&mut self.store).map_err(|e| failed(debug(e)))?;
                        }
                        other => return Err(failed(format!("{other:?} is not a world persist"))),
                    }
                }
            }
            Step::LoadSideEffectfulPlugins => {
                let classes = self.manifest(step)?.initializable.clone();
                for class in classes {
                    match scan::native(class) {
                        Some(Native::PickupPlugin) => {
                            // definePlugin -> plugin.newInstance(null):
                            // ItemDefinition.setOptionHandler("take", this).
                            self.item_options
                                .register_pickup()
                                .map_err(|e| failed(debug(e)))?;
                            self.legacy_plugins += 1;
                        }
                        _ => gaps.push(Gap {
                            what: class.into(),
                            cause: CONTENT,
                        }),
                    }
                }
                lines.log(
                    "SystemLogger",
                    LogLevel::Fine,
                    &format!(
                        "[STARTUP] We still have {} legacy plugins being loaded.",
                        self.legacy_plugins
                    ),
                );
            }
            Step::NpcSpawner => {
                let text = self
                    .config_text("npc_spawns.json")
                    .map_err(|e| failed(debug(e)))?;
                let holiday = self
                    .constant_bool("HOLIDAY_EVENT_RANDOMS")
                    .ok_or_else(|| failed("no HOLIDAY_EVENT_RANDOMS".into()))?;
                let before = self.entities.unported.clone();
                let archive = self.npc_archive.clone();
                let mut init = npcs::Init {
                    actors: &mut self.actors,
                    npcs: &mut self.entities,
                    definitions: &mut self.npcs,
                    archive: archive.as_deref(),
                    timers: &self.timers,
                    generators: &mut self.generators,
                    chunks: &mut self.chunks,
                    regions: &mut self.regions,
                    holiday_event_randoms: holiday,
                    ticks: self.ticks,
                    timer_log: &mut self.timer_log,
                    spawned: None,
                };
                npc_spawns::load(&mut init, &text).map_err(|e| failed(debug(e)))?;
                let count = init
                    .spawned
                    .ok_or_else(|| failed("the spawner did not finish".into()))?;
                lines.log(
                    "NPCSpawner",
                    LogLevel::Fine,
                    &format!("Spawned {count} NPCs"),
                );
                let after = &self.entities.unported;
                for (n, what) in [
                    (
                        after.zone_updates - before.zone_updates,
                        "ZoneMonitor.updateLocation after RegionManager.move (region zones are registered by class-scan content)",
                    ),
                    (
                        after.interact_defaults - before.interact_defaults,
                        "InteractPlugin.setDefault for NPCs (NPC option handlers are class-scan content)",
                    ),
                    (
                        after.slayer_tasks - before.slayer_tasks,
                        "NPC.configure: Tasks.forId (slayer content)",
                    ),
                    (
                        after.magic_autocasts - before.magic_autocasts,
                        "NPC.configure: DefaultCombatSpell / SpellBook autocast for magic NPCs",
                    ),
                    (
                        after.skill_restore_timers - before.skill_restore_timers,
                        "Entity.init: the automatic SkillRestore timer (Entity.hook has no native port)",
                    ),
                ] {
                    if n > 0 {
                        gaps.push(Gap {
                            what: format!("{n} x {what}"),
                            cause: "not ported",
                        });
                    }
                }
            }
            Step::GroundSpawns => {
                let mut ctx = ground::Context {
                    actors: &mut self.actors,
                    definitions: &self.items,
                    options: &self.item_options,
                    spawns: &self.ground.spawns,
                    chunks: &mut self.chunks,
                    regions: &mut self.regions,
                    dirty: &mut self.dirty_listener,
                    ticks: self.ticks,
                    lowercase: inputs.lower_case,
                    log: lines,
                };
                ground_spawn_loader::Loader::default()
                    .load_config(
                        self.config_path.as_deref(),
                        &mut ctx,
                        &self.ground.spawns,
                        &self.ground.manager,
                    )
                    .map_err(|e| failed(debug(e)))?;
                gaps.push(Gap {
                    what: "ItemDefinition.OPTION_HANDLERS entries other than take (registered by content plugins) in the ground items' option tables".into(),
                    cause: CONTENT,
                });
            }
            Step::StartupListeners => {
                for class in scan::section("listeners.startup") {
                    gaps.push(Gap {
                        what: class.into(),
                        cause: CONTENT,
                    });
                }
            }
            Step::SceneryExamine => {
                let lower = inputs.lower_case;
                for definition in self.scenery.definitions.values() {
                    let mut d = definition.write();
                    d.examine = Some(examine(&d, lower).map_err(&failed)?);
                }
            }
        }
        Ok(gaps)
    }
}

/// `Definition.getExamine()` on a scenery definition: the cached text, else
/// the `examine` handler's `toString()`, else "It's a(n) <name>." or "null"
/// for an empty name. A null name is the original's NullPointerException.
fn examine(d: &scenery::SceneryDefinition, lower: fn(&str) -> String) -> Result<String, String> {
    if let Some(text) = &d.examine {
        return Ok(text.clone());
    }
    match d.handlers.read().get("examine") {
        None | Some(Property::Null) => {}
        Some(Property::Text(text)) => return Ok(text.clone()),
        Some(Property::Int(value)) => return Ok(value.to_string()),
        Some(other) => return Err(format!("examine handler {other:?} has no native toString")),
    }
    let name = d
        .name
        .as_deref()
        .ok_or("NullPointerException: name.length()")?;
    if name.is_empty() {
        return Ok("null".into());
    }
    // StringUtils.isPlusN: the first characters of name.toLowerCase().
    let s: Vec<u16> = lower(name).encode_utf16().collect();
    let c = |i: usize| s.get(i).copied().map(u32::from).and_then(char::from_u32);
    let plus_n = matches!(c(0), Some('a' | 'e' | 'i' | 'o' | 'u'))
        || (c(0) == Some('h') && s.len() > 1 && c(1) != Some('e'));
    Ok(format!("It's a{}{name}.", if plus_n { "n " } else { " " }))
}

/// Runs the start-up in the original order.
pub fn start(inputs: &Inputs, policy: Policy) -> Result<Startup, StartupError> {
    let mut world = World::new(inputs.seeds)?;
    let mut steps = Vec::new();
    let mut complete = true;
    for step in ORDER {
        let mut lines = Lines::default();
        let outcome = if let Some(reason) = step.refusal() {
            if policy == Policy::StopAtRefusal {
                return Err(StartupError::NotPorted { step, reason });
            }
            complete = false;
            let gaps = world.run(inputs, step, &mut lines)?;
            lines.0.push(LogLine {
                origin: "startup",
                level: LogLevel::Err,
                message: format!("partly ported: {reason} ({} named gaps)", gaps.len()),
            });
            Outcome::Partial(gaps)
        } else {
            let gaps = world.run(inputs, step, &mut lines)?;
            if !gaps.is_empty() {
                return Err(StartupError::Failed {
                    step,
                    error: "a step classified native reported gaps".into(),
                });
            }
            Outcome::Native
        };
        let log = lines
            .0
            .iter()
            .filter(|l| l.origin != "startup" && !matches!(l.level, LogLevel::Err))
            .map(|l| format!("[{}] {}", l.origin, l.message))
            .collect();
        world.log.extend(lines.0);
        steps.push(StepReport {
            step,
            outcome,
            log,
            positions: world.generators.positions(),
        });
    }
    Ok(Startup {
        world,
        steps,
        complete,
    })
}
