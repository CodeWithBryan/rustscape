//! Booting the live world: a world directory assembled from the data
//! directory, the cache and the saves directory; the native start-up over it;
//! the canonical world built from what the start-up loaded.
use super::{
    P,
    host::{Live, SaveJob},
    inner::{Inner, WorldSettings},
    not_ported::NotPorted,
    walk::{Maps, Walk},
};
use crate::{
    auth::{Authenticator, Clock, factory::Registry as AuthRegistry, password::Passwords},
    config::Configuration,
    player::persisted::{host::World as PersistedWorld, music},
    startup::{
        self,
        generators::Seeds,
        sequence::{self, Inputs, Policy},
        store::WorldWrites,
    },
    world::{
        doors::Location,
        driver::Driver,
        interaction_options,
        login::Constants,
        repository::{self, Repository},
        scene::{CacheAssets, Scene},
        social,
    },
};
use rs09_cache::{Limits, archive::Archive, store::Store};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

/// What `--world` is given.
#[derive(Clone, Debug)]
pub struct Options {
    /// The original's world directory with `worldprops/default.conf` and
    /// `data/configs` (the world-trace template, for example).
    pub data: PathBuf,
    /// The cache directory (`CACHE_PATH`).
    pub cache: PathBuf,
    /// Where the live world writes: player saves (`<saves>/players`), the
    /// server store and the assembled world directory.
    pub saves: PathBuf,
    /// Print log lines and the first call of each not-ported answer.
    pub verbose: bool,
}

/// The start-up's report, for the caller.
pub struct Report {
    pub millis: u128,
    pub complete: bool,
    pub partial_steps: Vec<String>,
    pub npcs: usize,
}

/// A booted world and what the transport needs from it.
pub struct Booted {
    pub live: Live,
    pub configuration: Configuration,
    pub authenticator: Arc<Authenticator>,
    pub store: Store,
    pub report: Report,
}

/// Start-up writes go only under the saves directory.
struct SavesOnly(PathBuf);
impl WorldWrites for SavesOnly {
    fn create_dirs(&self, path: &Path) -> std::io::Result<()> {
        if !path.starts_with(&self.0) {
            return Err(std::io::Error::other(format!(
                "start-up write outside the saves directory refused: {}",
                path.display()
            )));
        }
        std::fs::create_dir_all(path)
    }
}

fn slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// `worldprops/default.conf` of the data directory with the four paths this
/// world owns replaced by absolute ones: the data, the cache, the server
/// store and the player saves. Nothing else changes.
pub fn assemble_world(options: &Options) -> Result<PathBuf, String> {
    let data = std::path::absolute(&options.data).map_err(|e| e.to_string())?;
    let cache = std::path::absolute(&options.cache).map_err(|e| e.to_string())?;
    let saves = std::path::absolute(&options.saves).map_err(|e| e.to_string())?;
    let source = data.join("worldprops").join("default.conf");
    let text =
        std::fs::read_to_string(&source).map_err(|e| format!("{}: {e}", source.display()))?;
    let replacements = [
        ("data_path", slashes(&data.join("data"))),
        ("cache_path", slashes(&cache)),
        ("store_path", slashes(&saves.join("serverstore"))),
        ("save_path", format!("{}/", slashes(&saves.join("players")))),
    ];
    let mut seen = BTreeSet::new();
    let mut out = String::new();
    for line in text.lines() {
        let key = line.split('=').next().unwrap_or_default().trim();
        if let Some((name, value)) = replacements.iter().find(|(k, _)| *k == key) {
            seen.insert(*name);
            out.push_str(&format!("{name} = \"{value}\"\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for (name, _) in &replacements {
        if !seen.contains(name) {
            return Err(format!("{} has no {name} line", source.display()));
        }
    }
    let world = saves.join("world");
    std::fs::create_dir_all(world.join("worldprops")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(saves.join("players")).map_err(|e| e.to_string())?;
    std::fs::write(world.join("worldprops").join("default.conf"), out)
        .map_err(|e| e.to_string())?;
    Ok(world)
}

fn constant_location(config: &Configuration, key: &str) -> Result<Location, String> {
    let v: Vec<i32> = serde_json::from_value(config.constants()[key].clone())
        .map_err(|e| format!("{key}: {e}"))?;
    if v.len() != 3 {
        return Err(format!("{key} is not a location"));
    }
    Ok(Location::new(v[0], v[1], v[2]))
}

fn text_constant(config: &Configuration, key: &str) -> Result<String, String> {
    config.constants()[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("configuration has no {key}"))
}

/// Boots the world. Every refused or partial start-up step is reported; the
/// start-up runs with `Policy::ContinuePastRefusals`.
pub fn boot(options: &Options, saves: std::sync::mpsc::Sender<SaveJob>) -> Result<Booted, String> {
    let started = Instant::now();
    let world_dir = assemble_world(options)?;
    let saves_dir = std::path::absolute(&options.saves).map_err(|e| e.to_string())?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos() as i64;
    let inputs = Inputs {
        world: world_dir,
        seeds: Seeds {
            shared: nanos,
            math: nanos.rotate_left(17),
            shuffle: nanos.rotate_left(31),
            thread_local: nanos.rotate_left(47),
        },
        max_definitions: 1_000_000,
        writes: Arc::new(SavesOnly(saves_dir.clone())),
        lower_case: rs09_game_core::java_lower::lowercase_root,
    };
    let startup =
        sequence::start(&inputs, Policy::ContinuePastRefusals).map_err(|e| e.to_string())?;
    let partial_steps = startup
        .steps
        .iter()
        .filter(|s| !matches!(s.outcome, sequence::Outcome::Native))
        .map(|s| format!("{:?}", s.step))
        .collect();
    let complete = startup.complete;
    let mut world = startup.world;
    let configuration = world
        .settings
        .take()
        .ok_or("the start-up loaded no configuration")?;
    let store = world.cache.clone().ok_or("the start-up opened no cache")?;
    let config_path = world
        .config_path
        .clone()
        .ok_or("configuration has no CONFIG_PATH")?;

    // ServerConstants and GameWorld.settings the login and social code read.
    let settings = configuration
        .world()
        .ok_or("configuration has no [world]")?;
    let values = settings.values();
    let value_str = |key: &str| -> Result<String, String> {
        values
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or_else(|| format!("[world] has no {key}"))
    };
    let value_bool = |key: &str| -> Result<bool, String> {
        values
            .get(key)
            .and_then(|v| v.as_bool())
            .ok_or_else(|| format!("[world] has no {key}"))
    };
    let constants = Constants {
        server_name: text_constant(&configuration, "SERVER_NAME")?,
        world_id: settings.world_id(),
        home_location: Some(constant_location(&configuration, "HOME_LOCATION")?),
        daily_account_limit: configuration.constants()["DAILY_ACCOUNT_LIMIT"]
            .as_i64()
            .ok_or("configuration has no DAILY_ACCOUNT_LIMIT")? as i32,
        config_path: Some(config_path.clone()),
        rules_and_info_enabled: configuration.constants()["RULES_AND_INFO_ENABLED"]
            .as_bool()
            .ok_or("configuration has no RULES_AND_INFO_ENABLED")?,
        message_model: values
            .get("message_model")
            .and_then(|v| v.as_i64())
            .ok_or("[world] has no message_model")? as i32,
        message_string: value_str("message_string")?,
    };
    let world_settings = WorldSettings {
        world_name: value_str("name")?,
        dev_mode: value_bool("isDevMode")?,
        enable_default_clan: value_bool("enable_default_clan")?,
        global_chat: configuration.constants()["ENABLE_GLOBAL_CHAT"]
            .as_bool()
            .ok_or("configuration has no ENABLE_GLOBAL_CHAT")?,
        ms_secret_key: text_constant(&configuration, "MS_SECRET_KEY")?,
    };
    let start = constant_location(&configuration, "START_LOCATION")?;
    let save_path = {
        let path = text_constant(&configuration, "PLAYER_SAVE_PATH")?.replace('\\', "/");
        if path.ends_with('/') {
            path
        } else {
            format!("{path}/")
        }
    };

    // The canonical actor state the start-up filled with its NPCs.
    let mut state = std::mem::take(&mut world.actors);
    let varbits = Arc::new(
        Archive::open(store.clone(), 22, None, Limits::default()).map_err(|e| format!("{e:?}"))?,
    );
    state
        .bind_varbit_assets(varbits.clone())
        .map_err(|e| e.to_string())?;
    let custom = std::fs::read_to_string(format!("{config_path}varbit_definitions.json"))
        .map_err(|e| format!("varbit_definitions.json: {e}"))?;
    crate::definitions::varbit::Loader::load_text(&state.identity().varbits(), &custom)
        .map_err(|e| format!("custom varbits: {e:?}"))?;

    // MusicEntry.SONGS from the start-up's music loader.
    let mut persisted = PersistedWorld::default();
    for (id, entry) in world.songs.iter() {
        persisted.songs.songs.insert(
            *id,
            music::Song {
                name: entry.name.clone().unwrap_or_else(|| "null".into()),
                index: entry.index,
            },
        );
    }

    let repository: Repository<P> = Repository::new(repository::MAX_PLAYERS, repository::MAX_NPCS)
        .map_err(|e| format!("{e:?}"))?;
    // Auth.configure with the live repository's name map as the online
    // lookup (the start-up's registry looks at the start-up's repository,
    // which holds only NPCs).
    let online_names = repository.clone();
    let mut auth = AuthRegistry::default();
    auth.configure(
        &configuration.auth_settings(),
        Arc::new(Passwords::default()),
        move |name: &str| {
            let key = Some(rs09_game_core::java_lower::lowercase_root(name).replace(' ', "_"));
            online_names.name_entries().iter().any(|(n, _)| *n == key)
        },
    )
    .map_err(|e| e.to_string())?;
    let authenticator = auth
        .authenticator()
        .cloned()
        .ok_or("Auth.configure made no authenticator")?;
    let storage = auth
        .provider()
        .ok_or("Auth.configure made no provider")?
        .storage();

    let items = Arc::new(std::mem::take(&mut world.items));
    let keys = std::mem::take(&mut world.region_keys);
    let scene = Scene::new(
        CacheAssets {
            archive: Archive::open(store.clone(), 5, None, Limits::default())
                .map_err(|e| format!("{e:?}"))?,
            keys: keys
                .iter()
                .map(|(id, k)| (id, k.map(|n| n as u32)))
                .collect(),
        },
        std::mem::take(&mut world.scenery),
        std::mem::take(&mut world.doors),
    );
    let clock: Arc<Clock> = Arc::new(crate::runtime::wall_clock);
    let npc_store = std::mem::replace(&mut world.entities, crate::startup::npcs::Npcs::new()?);
    let generators = std::mem::replace(
        &mut world.generators,
        crate::startup::generators::WorldGenerators::new(Seeds {
            shared: 0,
            math: 0,
            shuffle: 0,
            thread_local: 0,
        }),
    );
    let loaded_regions = std::mem::take(&mut world.regions);
    let inner = Inner {
        state,
        frames: BTreeMap::new(),
        factory: crate::inventory::container::Factory::new(&items),
        items,
        registry: std::mem::take(&mut world.interfaces),
        options: interaction_options::Constants::default(),
        keys,
        constants,
        settings: world_settings,
        world: persisted,
        hooks: startup::scan::section("listeners.persistPlayer").len(),
        players: BTreeMap::new(),
        sessions: BTreeMap::new(),
        queue: Vec::new(),
        current: None,
        attribute_path: format!("{save_path}attributes/"),
        save_path,
        clock,
        start,
        ticks: 0,
        not_ported: NotPorted::new(options.verbose),
        varbits,
        repository: repository.clone(),
        storage,
        log: Vec::new(),
        verbose: options.verbose,
        npcs: npc_store,
        generators,
    };
    let names = |section: &str| -> Vec<String> {
        startup::scan::section(section)
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    let mut live = Live {
        inner,
        social: social::Social::new(),
        driver: Driver::new(repository.clone()),
        repository,
        walk: Walk {
            scene,
            animations: crate::world::prepared_animation::World::new(
                crate::definitions::animation::CacheSource(
                    Archive::open(store.clone(), 20, None, Limits::default())
                        .map_err(|e| format!("{e:?}"))?,
                ),
            ),
            maps: Maps::default(),
            active: BTreeSet::new(),
        },
        login_listeners: names("listeners.login"),
        logout_listeners: names("listeners.logout"),
        login_plugins: names("listeners.loginPlugins"),
        logins: Vec::new(),
        tasks: VecDeque::new(),
        saves,
        inserted: 0,
        deferred: Vec::new(),
        online: Arc::new(Mutex::new(HashSet::new())),
        saved: 0,
        regions: super::regions::Regions::new(loaded_regions),
        npc_views: Default::default(),
        tab_pulses: Vec::new(),
        listeners: Default::default(),
    };
    // The start-up's NPCs in the live repository and the scene's chunks;
    // the tab content's component plugins and interface listeners.
    let npcs = super::npcs::join(&mut live)?;
    super::buttons::register(&mut live)?;
    Ok(Booted {
        live,
        configuration,
        authenticator,
        store,
        report: Report {
            millis: started.elapsed().as_millis(),
            complete,
            partial_steps,
            npcs,
        },
    })
}
