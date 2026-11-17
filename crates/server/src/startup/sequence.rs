//! The original start-up, `GameWorld.prompt(run = false)`, as an ordered list
//! of steps, each either run natively or refused as not ported.
//!
//! [`ORDER`] is the order of the statements of `prompt` (with
//! `ConfigParser.prePlugin`/`postPlugin` expanded loader by loader), which is
//! also the phase order of `fixtures/startup-loaders`. A refused step is never
//! skipped silently and never reported as done: with
//! [`Policy::StopAtRefusal`] the start-up stops there with
//! [`StartupError::NotPorted`]; with [`Policy::ContinuePastRefusals`] the
//! refusal is logged, recorded in the report and the result is marked
//! incomplete. A failure inside a native step is the original's uncaught
//! exception: the start-up stops with [`StartupError::Failed`].
//!
//! The [`World`] this produces holds what the native steps loaded. It is not a
//! playable world: content (class scan, plugins, start-up listeners), NPC
//! spawning, ground spawns and the rest listed in docs/startup.md are refused.
use super::generators::{Positions, Seeds, WorldGenerators};
use super::loaders::{
    self, LoadError, Log, clue_rewards::ClueRewards, music, ranged::RangedTables, xtea::RegionKeys,
};
use super::regions::Regions;
use crate::config::Configuration;
use crate::definitions::{item, item_config, npc, scenery, varbit};
use crate::world::drop_tables::{LogLevel, NpcDropTable, parser as drop_parser};
use crate::world::{doors, interfaces};
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
    /// Why a step is refused; `None` for a step this sequence runs natively.
    pub const fn refusal(self) -> Option<&'static str> {
        Some(match self {
            Self::ChunkDirtyListener => {
                "RegionChunk.dirtyListener = ChunkUpdateTracker: world::chunk_sync is not joined to this World"
            }
            Self::AuthConfigure => {
                "Auth.configure: auth::factory is ported (startup::configure installs it for the endpoint) but its world-owner lookup is not joined to this World"
            }
            Self::ScanClasspath => {
                "ClassScanner.scanClasspath: content class scanning is not ported"
            }
            Self::LoadPureInterfaces => {
                "ClassScanner.loadPureInterfaces: content interfaces/listeners are not ported"
            }
            Self::LoadTimers => {
                "ClassScanner.loadTimers: timers::registry exists but the scanned timer classes are content"
            }
            Self::ServerStore => "ServerStore.parse is not ported",
            Self::WorldPersists => {
                "worldPersists.parse (GraveController) is content and not ported"
            }
            Self::LoadSideEffectfulPlugins => {
                "ClassScanner.loadSideEffectfulPlugins: legacy plugins are content and not ported"
            }
            Self::NpcSpawner => {
                "NPCSpawner: world::npc_spawns parses the rows, but NPC.create/init, Repository registration and region membership are not joined to this World"
            }
            Self::GroundSpawns => {
                "GroundSpawnLoader: world::ground_spawn_loader needs a ground-item manager and world context this World does not own"
            }
            Self::StartupListeners => {
                "startup listeners: the list comes from the class scan (content); none is run"
            }
            Self::SceneryExamine => {
                "SceneryDefinition.getExamine over every definition is not ported"
            }
            _ => return None,
        })
    }
}

/// What a refused step does to the start-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    StopAtRefusal,
    ContinuePastRefusals,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Native,
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
    fn new(seeds: Seeds) -> Self {
        Self {
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

    fn run(&mut self, inputs: &Inputs, step: Step, lines: &mut Lines) -> Result<(), StartupError> {
        let failed = fail(step);
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
            _ => unreachable!("refused steps are handled by start()"),
        }
        Ok(())
    }
}

/// Runs the start-up in the original order.
pub fn start(inputs: &Inputs, policy: Policy) -> Result<Startup, StartupError> {
    let mut world = World::new(inputs.seeds);
    let mut steps = Vec::new();
    let mut complete = true;
    for step in ORDER {
        let mut lines = Lines::default();
        let outcome = if let Some(reason) = step.refusal() {
            if policy == Policy::StopAtRefusal {
                return Err(StartupError::NotPorted { step, reason });
            }
            complete = false;
            lines.0.push(LogLine {
                origin: "startup",
                level: LogLevel::Err,
                message: format!("not ported: {reason}"),
            });
            Outcome::NotPorted(reason)
        } else {
            world.run(inputs, step, &mut lines)?;
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
