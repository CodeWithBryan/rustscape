//! Native bodies of the command sets `MiscCommandSet`, `SpawnCommandSet` and
//! `StatsCommandSet` (`core.game.system.command.sets`).
//!
//! | Original | Here |
//! | --- | --- |
//! | `MiscCommandSet` (all 49 commands and its helpers) | [`misc`] |
//! | `SpawnCommandSet` | [`spawn`] |
//! | `StatsCommandSet` with its book `display` callback | [`stats`] |
//!
//! The registry dispatches a [`SetBody`] through `commands::run`, which
//! hands this module the same world as [`Host`] (`commands::Host::sets`).
//! Every call into a system these sets do not own is a required method of
//! [`Host`] with no default: attributes, interfaces, packets, varps,
//! containers, definitions, regions and scenery, NPCs, combat
//! calculations, the Grand Exchange queries, books, timers and every
//! content call (cutscene, diaries, permadeath, trawler loot, farming and
//! brewing, slayer/clue/stat counters). Values the original computes
//! itself (strings, chunking, the command registry walks, page indices,
//! HashMap orders, the hit-chance arithmetic and its BigDecimal rounding,
//! Location arithmetic, Scenery.toString) are computed here. See
//! docs/command-sets.md.
pub mod misc;
pub mod spawn;
pub mod stats;

use super::{Error, Loc, Registry, Result};
use crate::player::details::Rights;
use std::sync::{Arc, Mutex};

/// A ported body of these sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SetBody {
    Misc(misc::Command),
    Spawn(spawn::Command),
    Stats,
}

/// A `java.util.ArrayList<String>` held in an attribute: shared, so a
/// list read back is the same object the store holds.
pub type ListRef = Arc<Mutex<Vec<String>>>;

/// An attribute value as these sets store and read them.
#[derive(Clone, Debug)]
pub enum Value<P> {
    Int(i32),
    Long(i64),
    Bool(bool),
    Text(String),
    Rights(Rights),
    List(ListRef),
    Player(P),
}
impl<P: PartialEq> PartialEq for Value<P> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Long(a), Self::Long(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Rights(a), Self::Rights(b)) => a == b,
            (Self::List(a), Self::List(b)) => Arc::ptr_eq(a, b),
            (Self::Player(a), Self::Player(b)) => a == b,
            _ => false,
        }
    }
}

/// `Direction`'s constants in the original's order (`ordinal`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    NorthWest,
    North,
    NorthEast,
    West,
    East,
    SouthWest,
    South,
    SouthEast,
}
impl Direction {
    pub const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::West,
        Self::East,
        Self::SouthWest,
        Self::South,
        Self::SouthEast,
    ];
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    pub fn name(self) -> &'static str {
        [
            "NORTH_WEST",
            "NORTH",
            "NORTH_EAST",
            "WEST",
            "EAST",
            "SOUTH_WEST",
            "SOUTH",
            "SOUTH_EAST",
        ][self as usize]
    }
}

/// The player's locks the sets query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    Movement,
    Teleport,
    Interaction,
}

/// `IronmanMode`'s constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IronmanMode {
    None,
    Standard,
    Hardcore,
    Ultimate,
}
impl IronmanMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Standard => "STANDARD",
            Self::Hardcore => "HARDCORE",
            Self::Ultimate => "ULTIMATE",
        }
    }
}

/// The swing handler classes the sets reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handler {
    Melee,
    Range,
    Magic,
    MagicalMelee,
    MagicalRanged,
    RangedMelee,
}
impl Handler {
    /// The class's simple name.
    pub fn simple_name(self) -> &'static str {
        match self {
            Self::Melee => "MeleeSwingHandler",
            Self::Range => "RangeSwingHandler",
            Self::Magic => "MagicSwingHandler",
            Self::MagicalMelee => "MagicalMeleeSwingHandler",
            Self::MagicalRanged => "MagicalRangedSwingHandler",
            Self::RangedMelee => "RangedMeleeSwingHandler",
        }
    }
    /// `getClass().getName()`.
    pub fn class_name(self) -> String {
        format!("core.game.node.entity.combat.{}", self.simple_name())
    }
    /// `handler.type` (`CombatStyle`).
    pub fn style(self) -> &'static str {
        match self {
            Self::Melee => "MELEE",
            Self::Range | Self::RangedMelee => "RANGE",
            Self::Magic | Self::MagicalMelee | Self::MagicalRanged => "MAGIC",
        }
    }
}

/// An NPC the host made: its handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Npc(pub u32);

/// An entity the combat calculations take.
#[derive(Clone, Debug, PartialEq)]
pub enum Actor<P> {
    Player(P),
    Npc(Npc),
}

/// A scenery value as the sets read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenery {
    /// The host's handle.
    pub handle: u32,
    pub id: i32,
    pub location: Option<Loc>,
    pub kind: i32,
    pub rotation: i32,
    /// `object is Constructed`.
    pub constructed: bool,
    pub renderable: bool,
    pub active: bool,
}
impl Scenery {
    /// `Scenery.toString()`.
    pub fn java_string(&self) -> String {
        format!(
            "[Scenery {}, {}, type={}, rot={}]",
            self.id,
            location_string(self.location),
            self.kind,
            self.rotation
        )
    }
}

/// A region chunk's handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chunk(pub u32);

/// An NPC definition as getnpcparent reads it: its id and child ids.
pub type NpcDefinition = (i32, Option<Vec<i32>>);

/// A Grand Exchange offer row as the queries answer it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Offer {
    pub item: i32,
    pub amount: i32,
    pub offered_value: i32,
    pub sell: bool,
    pub amount_left: i32,
}

/// The timers `getOrStartTimer` starts here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timer {
    CropGrowth,
    Compost,
    BrewGrowth,
}
impl Timer {
    pub fn simple_name(self) -> &'static str {
        match self {
            Self::CropGrowth => "CropGrowth",
            Self::Compost => "Compost",
            Self::BrewGrowth => "BrewGrowth",
        }
    }
}

/// `core.tools.Log` levels the sets log at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Fine,
    Info,
}
impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fine => "FINE",
            Self::Info => "INFO",
        }
    }
}

/// A book line: `BookLine(message, child)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookLine {
    pub message: String,
    pub child: i32,
}
/// A `PageSet`: its pages, each a list of lines.
pub type PageSet = Vec<Vec<BookLine>>;

/// A book's display callback, as the opened book keeps it.
#[derive(Clone, Debug, PartialEq)]
pub enum Book {
    /// `StatsCommandSet.display`.
    Stats,
    /// `MiscCommandSet.showGeBook`'s local `display`: the title and pages
    /// it captured.
    Ge {
        title: String,
        contents: Vec<PageSet>,
    },
}

/// An input dialogue's handler.
#[derive(Clone, Debug, PartialEq)]
pub enum Input<P> {
    /// `reply`: the private message to `reply_to`.
    Reply { player: P, reply_to: String },
    /// `ge search` without a term.
    GeSearch { player: P },
    /// `ge botsearch` without a term.
    GeBotSearch { player: P },
}

/// A launched coroutine.
#[derive(Clone, Debug, PartialEq)]
pub enum Job<P> {
    /// `getnpcparent`'s definition walk.
    NpcParent { player: P, npc: i32 },
}

/// What these sets need from the world. No defaults: methods without a
/// `Result` are field or constant reads of the original; every other one
/// is a call into a system these sets do not port and may fail with what
/// it threw.
pub trait Host: super::Host {
    // Constants.
    /// `ServerConstants.BANK_SIZE`.
    fn bank_size(&mut self) -> i32;
    /// `ServerConstants.NEW_PLAYER_ANNOUNCEMENT`.
    fn new_player_announcement(&mut self) -> bool;

    /// `Repository.players` (the list's walk: no call).
    fn online_players(&mut self) -> Vec<Self::Player>;

    // Player fields (reads of the original's fields: no call).
    fn direction(&mut self, player: &Self::Player) -> Direction;
    fn version(&mut self, player: &Self::Player) -> i32;
    /// `player.playerFlags.lastSceneGraph`.
    fn last_scene_graph(&mut self, player: &Self::Player) -> Option<Loc>;
    /// The diaries' `taskCompleted` sizes: per diary, per level.
    fn diaries(&mut self, player: &Self::Player) -> Vec<Vec<i32>>;
    fn test_stage(&mut self, player: &Self::Player) -> i32;
    fn set_test_stage(&mut self, player: &Self::Player, stage: i32);
    fn read_plaques(&mut self, player: &Self::Player) -> Vec<bool>;
    fn set_read_plaque(&mut self, player: &Self::Player, index: usize, read: bool);
    fn boss_counters(&mut self, player: &Self::Player) -> Vec<i32>;
    fn barrows_loots(&mut self, player: &Self::Player) -> i32;

    // Entity attributes (Entity.getAttribute/setAttribute/removeAttribute;
    // the store's own key handling is the host's).
    /// `entity.getAttribute(key, fail)` (`fail` None is the no-default form).
    fn entity_attribute(
        &mut self,
        player: &Self::Player,
        key: &str,
        fail: Option<Value<Self::Player>>,
    ) -> Result<Option<Value<Self::Player>>>;
    fn entity_set_attribute(
        &mut self,
        player: &Self::Player,
        key: &str,
        value: Option<Value<Self::Player>>,
    ) -> Result<()>;
    fn entity_remove_attribute(&mut self, player: &Self::Player, key: &str) -> Result<()>;
    /// `entity.getAttributes().containsKey(key)`.
    fn has_attribute(&mut self, player: &Self::Player, key: &str) -> Result<bool>;

    // The player's systems.
    /// `player.getPacketDispatch().sendMessage(text)`.
    fn dispatch_message(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    fn send_string(
        &mut self,
        player: &Self::Player,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn send_varc(&mut self, player: &Self::Player, index: i16, value: i32) -> Result<()>;
    fn send_interface_config(
        &mut self,
        player: &Self::Player,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()>;
    fn interface_opened(&mut self, player: &Self::Player) -> Result<bool>;
    /// `interfaceManager.getOpened()`: its id, or `None` when null.
    fn opened_component(&mut self, player: &Self::Player) -> Result<Option<i32>>;
    fn open_component(&mut self, player: &Self::Player, id: i32) -> Result<()>;
    /// `interfaceManager.close()`.
    fn close_interfaces(&mut self, player: &Self::Player) -> Result<bool>;
    fn close_dialogue(&mut self, player: &Self::Player) -> Result<bool>;
    fn locked(&mut self, player: &Self::Player, lock: Lock) -> Result<bool>;
    fn is_artificial(&mut self, player: &Self::Player) -> Result<bool>;
    fn swing_handler(&mut self, player: &Self::Player, swing: bool) -> Result<Handler>;
    fn level(&mut self, actor: &Actor<Self::Player>, skill: i32) -> Result<i32>;
    fn set_version(&mut self, player: &Self::Player, version: i32) -> Result<()>;
    fn unlock(&mut self, player: &Self::Player) -> Result<()>;
    fn ip_address(&mut self, player: &Self::Player) -> Result<String>;
    fn comp_name(&mut self, player: &Self::Player) -> Result<String>;
    fn finish_task(
        &mut self,
        player: &Self::Player,
        diary: i32,
        level: i32,
        task: i32,
    ) -> Result<()>;
    fn quest_points(&mut self, player: &Self::Player) -> Result<i32>;
    fn ironman_mode(&mut self, player: &Self::Player) -> Result<IronmanMode>;
    fn slayer_completed_tasks(&mut self, player: &Self::Player) -> Result<i32>;
    fn completed_clues(&mut self, player: &Self::Player) -> Result<Vec<i32>>;
    fn stat_kills(&mut self, player: &Self::Player, npcs: &[i32]) -> Result<i64>;
    fn stat_rare_drops(&mut self, player: &Self::Player, item: i32) -> Result<i64>;

    // Containers beyond commands::Host's.
    fn container_clear(&mut self, player: &Self::Player, container: super::Container)
    -> Result<()>;
    fn container_refresh(
        &mut self,
        player: &Self::Player,
        container: super::Container,
    ) -> Result<()>;
    fn container_update(
        &mut self,
        player: &Self::Player,
        container: super::Container,
    ) -> Result<()>;
    fn maximum_add(
        &mut self,
        player: &Self::Player,
        container: super::Container,
        id: i32,
        amount: i32,
    ) -> Result<i32>;

    // core.api members that are other systems.
    fn send_news(&mut self, text: &str, icon: i32, color: &str) -> Result<()>;
    /// `getItemName(id)`: a definition-table read (no call).
    fn item_definition_name(&mut self, id: i32) -> String;
    fn add_item_or_drop(&mut self, player: &Self::Player, id: i32, amount: i32) -> Result<()>;
    fn set_varp(&mut self, player: &Self::Player, index: i32, value: i32, save: bool)
    -> Result<()>;
    fn set_varbit(&mut self, player: &Self::Player, id: i32, value: i32, save: bool) -> Result<()>;
    fn get_varbit(&mut self, player: &Self::Player, id: i32) -> Result<i32>;
    fn start_timer(&mut self, player: &Self::Player, timer: Timer) -> Result<()>;
    fn patches(&mut self, player: &Self::Player) -> Result<i32>;
    fn set_next_growth(&mut self, player: &Self::Player, patch: i32, value: i64) -> Result<()>;
    fn set_disease_mod(&mut self, player: &Self::Player, patch: i32, value: i32) -> Result<()>;
    fn crop_run(&mut self, player: &Self::Player) -> Result<bool>;
    fn bins(&mut self, player: &Self::Player) -> Result<i32>;
    fn bin_finished(&mut self, player: &Self::Player, bin: i32) -> Result<bool>;
    fn bin_closed(&mut self, player: &Self::Player, bin: i32) -> Result<bool>;
    fn bin_finish(&mut self, player: &Self::Player, bin: i32) -> Result<()>;
    fn bin_reset(&mut self, player: &Self::Player, bin: i32) -> Result<()>;
    fn vats(&mut self, player: &Self::Player) -> Result<i32>;
    fn brew(&mut self, player: &Self::Player, vat: i32, force: [bool; 4]) -> Result<()>;
    fn log(&mut self, origin: &str, level: Level, message: &str) -> Result<()>;
    fn input(
        &mut self,
        player: &Self::Player,
        kind: super::InputType,
        prompt: &str,
        handler: Input<Self::Player>,
    ) -> Result<()>;
    fn launch_job(&mut self, job: Job<Self::Player>) -> Result<()>;

    // Content.
    fn start_cutscene(&mut self, player: &Self::Player, name: &str) -> Result<()>;
    fn permadeath(&mut self, player: &Self::Player) -> Result<()>;
    fn trawler_loot(
        &mut self,
        player: &Self::Player,
        level: i32,
        rolls: i32,
        skip_junk: bool,
    ) -> Result<()>;
    /// `RulesAndInfo.setBaseRulesAndInfo(player)`.
    fn rules_and_info(&mut self, player: &Self::Player) -> Result<i32>;
    fn open_book(&mut self, player: &Self::Player, book: i32, callback: Book) -> Result<()>;
    fn page_setup(
        &mut self,
        player: &Self::Player,
        book: i32,
        title: &str,
        contents: &[PageSet],
        pagination: bool,
    ) -> Result<()>;
    /// `CommunicationInfo.sendMessage(player, target, message)`.
    fn private_message(&mut self, player: &Self::Player, target: &str, message: &str)
    -> Result<()>;
    /// `AIRepository.PulseRepository[name]?.botScript.toString()`.
    fn bot_script(&mut self, name: &str) -> Result<Option<String>>;
    fn valid_offers(&mut self) -> Result<Vec<Offer>>;
    fn bot_offers(&mut self) -> Result<Vec<Offer>>;

    // Definitions.
    /// `Cache.getItemDefinitionsSize()`.
    fn cache_item_definitions_size(&mut self) -> Result<i32>;
    /// `SceneryDefinition.forId(id)` (the definition is named by its id).
    fn scenery_definition(&mut self, id: i32) -> Result<()>;
    fn scenery_varbit_id(&mut self, id: i32) -> Result<i32>;
    /// `getConfigFile()`: the varbit's id, or `None` when null.
    fn scenery_config_file(&mut self, id: i32) -> Result<Option<i32>>;
    fn varbit_for_object(&mut self, id: i32) -> Result<Option<i32>>;
    fn varbit_for_id(&mut self, id: i32) -> Result<Option<i32>>;
    fn varbit_varp(&mut self, varbit: i32) -> Result<i32>;
    fn varbit_start_bit(&mut self, varbit: i32) -> Result<i32>;
    fn varbit_string(&mut self, varbit: i32) -> Result<String>;
    /// `NPCDefinition.getDefinitions().values`, in the map's order: each
    /// definition's id and child ids (`None` for a null value).
    fn npc_definitions(&mut self) -> Result<Vec<Option<NpcDefinition>>>;

    // Regions and scenery.
    /// `RegionManager.forId(regionId)`: whether it is a `DynamicRegion`.
    fn region(&mut self, region: i32) -> Result<bool>;
    fn region_active(&mut self, region: i32) -> Result<bool>;
    fn teleport_permitted(&mut self, location: Loc) -> Result<bool>;
    fn clipping_flag(&mut self, location: Loc) -> Result<i32>;
    fn is_landscape(&mut self, location: Loc) -> Result<bool>;
    fn region_object(&mut self, location: Loc) -> Result<Option<Scenery>>;
    fn region_chunk(&mut self, location: Loc) -> Result<Chunk>;
    /// `chunk.statObjects[x][y]` (a field read).
    fn chunk_static_objects(&mut self, chunk: Chunk, x: i32, y: i32) -> Vec<Option<Scenery>>;
    fn chunk_objects(&mut self, chunk: Chunk, x: i32, y: i32) -> Result<Vec<Option<Scenery>>>;
    /// `Scenery(id, location, type, rotation)`.
    fn new_scenery(
        &mut self,
        id: i32,
        location: Option<Loc>,
        kind: i32,
        rotation: i32,
    ) -> Result<Scenery>;
    fn scenery_add(&mut self, scenery: &Scenery) -> Result<()>;
    fn scenery_remove(&mut self, scenery: &Scenery) -> Result<bool>;

    // NPCs.
    /// `NPC(id)` (location `None`) or `NPC.create(id, location)`.
    fn new_npc(&mut self, id: i32, location: Option<Loc>) -> Result<Npc>;
    fn npc_init_config(&mut self, npc: Npc) -> Result<()>;
    fn npc_name(&mut self, npc: Npc) -> Result<String>;
    fn npc_bonuses(&mut self, npc: Npc) -> Result<Vec<i32>>;
    fn npc_set_attack_style(&mut self, npc: Npc, style: i32, bonus: i32) -> Result<()>;
    fn npc_set_attribute(&mut self, npc: Npc, key: &str, value: bool) -> Result<()>;
    fn npc_set_respawn(&mut self, npc: Npc, respawn: bool) -> Result<()>;
    fn npc_set_direction(&mut self, npc: Npc, direction: Direction) -> Result<()>;
    fn npc_init(&mut self, npc: Npc) -> Result<()>;
    fn npc_set_walks(&mut self, npc: Npc, walks: bool) -> Result<()>;
    /// `npc.location`, `npc.direction`, `npc.isWalks` (field reads).
    fn npc_location(&mut self, npc: Npc) -> Option<Loc>;
    fn npc_direction(&mut self, npc: Npc) -> Direction;
    fn npc_walks(&mut self, npc: Npc) -> bool;

    // Combat calculations.
    fn calculate_accuracy(&mut self, handler: Handler, entity: &Actor<Self::Player>)
    -> Result<i32>;
    fn calculate_defence(
        &mut self,
        handler: Handler,
        victim: &Actor<Self::Player>,
        attacker: &Actor<Self::Player>,
    ) -> Result<i32>;
    fn calculate_hit(
        &mut self,
        handler: Handler,
        entity: &Actor<Self::Player>,
        victim: &Actor<Self::Player>,
        modifier: f64,
    ) -> Result<i32>;

    // Host outputs.
    /// `println(text)` on standard output.
    fn println(&mut self, text: &str);
    /// The system clipboard: a headless host answers `HeadlessException`
    /// ([`HEADLESS`]).
    fn set_clipboard(&mut self, text: &str) -> Result<()>;
    /// `System.currentTimeMillis()`.
    fn clock(&mut self) -> i64;
}

/// The error a headless host's clipboard throws.
pub const HEADLESS: &str = "java.awt.HeadlessException";

/// `HeadlessException` as an [`Error`].
pub fn headless() -> Error {
    Error::Binding(Some(HEADLESS.into()))
}

/// Whether `error` is the clipboard's `HeadlessException`.
pub fn is_headless(error: &Error) -> bool {
    matches!(error, Error::Binding(Some(m)) if m == HEADLESS)
}

/// A body run with the registry `CommandMapping` is.
pub fn run<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    body: SetBody,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    match body {
        SetBody::Misc(command) => misc::run(host, registry, command, player, args),
        SetBody::Spawn(command) => spawn::run(host, command, player, args),
        SetBody::Stats => stats::stats(host, player, args),
    }
}

/// `Location.toString()`, or "null".
pub fn location_string(location: Option<Loc>) -> String {
    location.map_or("null".into(), |[x, y, z]| format!("[{x}, {y}, {z}]"))
}

/// `args.slice(1 until args.size).joinToString(sep)`.
pub fn join_from(args: &[String], from: usize, separator: &str) -> String {
    args.get(from..).unwrap_or(&[]).join(separator)
}

/// `String.length` (UTF-16 units).
pub fn java_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `Character.toUpperCase(char)` for the characters a command line holds
/// (U+0001..U+007F and U+FF80..U+FFFF): a single-character mapping.
fn upper(c: char) -> char {
    let mut it = c.to_uppercase();
    match (it.next(), it.next()) {
        (Some(u), None) => u,
        _ => c,
    }
}
fn lower(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}
fn char_eq_ignore_case(a: char, b: char) -> bool {
    a == b || upper(a) == upper(b) || lower(upper(a)) == lower(upper(b))
}

/// Kotlin `equals(other, ignoreCase = true)`.
pub fn equals_ignore_case(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| char_eq_ignore_case(*x, *y))
}

/// Kotlin `contains(other, ignoreCase = true)`.
pub fn contains_ignore_case(text: &str, other: &str) -> bool {
    let t: Vec<char> = text.chars().collect();
    let o: Vec<char> = other.chars().collect();
    if o.is_empty() {
        return true;
    }
    t.windows(o.len())
        .any(|w| w.iter().zip(&o).all(|(x, y)| char_eq_ignore_case(*x, *y)))
}

/// `Rights.getChatIcon(player)`: the rights (read first), then the
/// `visible_rank` attribute with them as its default.
pub fn chat_icon<H: Host + ?Sized>(host: &mut H, player: &H::Player) -> Result<i32> {
    let rights = super::rights(host, player)?;
    match host.entity_attribute(player, "visible_rank", Some(Value::Rights(rights)))? {
        None => Ok(0),
        Some(Value::Rights(Rights::RegularPlayer)) => Ok(0),
        Some(Value::Rights(r)) => Ok(r.ordinal()),
        Some(_) => Err(Error::Cast),
    }
}

/// `Command.attemptHandling`'s player-facing helper: the bool an attribute
/// holds, `ClassCastException` for any other value.
pub fn as_bool<P>(value: Option<Value<P>>) -> Result<Option<bool>> {
    match value {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(b)),
        Some(_) => Err(Error::Cast),
    }
}

/// `BigDecimal(value.toString()).setScale(4, RoundingMode.UP).toDouble()`.
pub fn round_up_4(value: f64) -> Result<f64> {
    let text = rs09_game_core::java_double::format(value);
    let Some((negative, digits, point)) = decimal_digits(&text) else {
        return Err(Error::NumberFormat(format!(
            "Character {} is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark.",
            text.trim_start_matches('-').chars().next().unwrap_or('?')
        )));
    };
    // value = 0.digits * 10^point; keep 4 fraction digits, rounding away
    // from zero when anything non-zero is dropped.
    let keep = point + 4;
    let mut kept: Vec<u8> = if keep <= 0 {
        Vec::new()
    } else {
        digits
            .iter()
            .copied()
            .chain(std::iter::repeat(0))
            .take(keep as usize)
            .collect()
    };
    let dropped = digits.iter().skip(keep.max(0) as usize).any(|d| *d != 0);
    if dropped {
        // Add one unit in the last kept place.
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, 1);
                break;
            }
            i -= 1;
            if kept[i] == 9 {
                kept[i] = 0;
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    // kept holds the digits of value * 10^4 (an integer).
    let mut integer: String = kept.iter().map(|d| char::from(b'0' + d)).collect();
    if integer.is_empty() {
        integer.push('0');
    }
    let text = format!("{}{}e-4", if negative { "-" } else { "" }, integer);
    text.parse::<f64>()
        .map_err(|_| Error::NumberFormat(text.clone()))
}

/// The decimal digits of a `Double.toString` result: sign, digits (no
/// leading zeros needed) and the position of the decimal point (value =
/// 0.d1d2.. * 10^point). `None` for "NaN"/"Infinity".
fn decimal_digits(text: &str) -> Option<(bool, Vec<u8>, i32)> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (mantissa, exponent) = match body.split_once('E') {
        Some((m, e)) => (m, e.parse::<i32>().ok()?),
        None => (body, 0),
    };
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !frac_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits: Vec<u8> = int_part
        .bytes()
        .chain(frac_part.bytes())
        .map(|b| b - b'0')
        .collect();
    Some((negative, digits, int_part.len() as i32 + exponent))
}

/// `Pulse`-free `chunkText(text, maxLength)` of MiscCommandSet.
pub fn chunk_text(text: &str, max_length: usize) -> Vec<String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() <= max_length {
        return vec![text.to_owned()];
    }
    let mut chunks = Vec::new();
    let mut remaining = units;
    while remaining.len() > max_length {
        let mut break_point = max_length;
        for i in (1..=max_length).rev() {
            if remaining[i - 1] == u16::from(b' ') {
                break_point = i;
                break;
            }
        }
        let head = String::from_utf16_lossy(&remaining[..break_point]);
        chunks.push(super::kotlin::kotlin_trim(&head).to_owned());
        let tail = String::from_utf16_lossy(&remaining[break_point..]);
        remaining = super::kotlin::kotlin_trim(&tail).encode_utf16().collect();
    }
    if !remaining.is_empty() {
        chunks.push(String::from_utf16_lossy(&remaining));
    }
    chunks
}
