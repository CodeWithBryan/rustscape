//! Runs one recorded drop-table case against the native library. The services
//! here answer from the case's inputs exactly as oracle/drop-tables/Support.java
//! does for the original classes.
use rs09_game_core::random::JavaRandom;
use rs09_server::world::drop_tables::{
    self as drops, ClueLevel, Context, DropFrequency, Host, Inventory, LogLevel, Looter,
    NpcDropTable, NpcDropTables, PlayerLooter, RollError, RolledItem, WeightBasedTable,
    WeightedItem,
    parser::{self, Definitions, LoadError},
    random::{self, JavaSource, Source, ThreadLocalRandom},
    shared::{self, ParseFailure, SharedKind, SharedTables, WeightedChanceItem},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
    sync::Arc,
};

const PINNED_CONFIG: &str = include_str!("../../fixtures/drop-tables/drop_tables.json");
const FOLDER: &str = "runs/drop-tables-oracle/config";

type Events = Rc<RefCell<Vec<Value>>>;

fn pinned_shared(kind: SharedKind) -> &'static str {
    match kind {
        SharedKind::Rare => include_str!("../../fixtures/drop-tables/shared_tables/RDT.xml"),
        SharedKind::Cele => include_str!("../../fixtures/drop-tables/shared_tables/CELEDT.xml"),
        SharedKind::UncommonSeed => {
            include_str!("../../fixtures/drop-tables/shared_tables/USDT.xml")
        }
        SharedKind::Herb => include_str!("../../fixtures/drop-tables/shared_tables/HDT.xml"),
        SharedKind::Gem => include_str!("../../fixtures/drop-tables/shared_tables/GDT.xml"),
        SharedKind::RareSeed => include_str!("../../fixtures/drop-tables/shared_tables/RSDT.xml"),
        SharedKind::AllotmentSeed => {
            include_str!("../../fixtures/drop-tables/shared_tables/ASDT.xml")
        }
    }
}

/// `Double.doubleToLongBits`: every NaN is the one canonical NaN.
fn bits(value: f64) -> i64 {
    if value.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        value.to_bits() as i64
    }
}
fn double(value: &Value) -> f64 {
    f64::from_bits(value.as_i64().unwrap() as u64)
}
fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn items(list: &[RolledItem]) -> Value {
    list.iter()
        .map(|item| json!([item.id, item.amount]))
        .collect()
}
fn weighted(list: &[WeightedItem]) -> Value {
    list.iter()
        .map(|item| {
            json!([
                item.id,
                item.min_amt,
                item.max_amt,
                bits(item.weight),
                item.guaranteed
            ])
        })
        .collect()
}
fn weighted_item(row: &[Value]) -> WeightedItem {
    WeightedItem::new(
        int(&row[0]),
        int(&row[1]),
        int(&row[2]),
        double(&row[3]),
        row[4].as_bool().unwrap(),
    )
}
fn snapshot(table: &WeightBasedTable) -> Value {
    json!({
        "items": weighted(&table.items),
        "total": bits(table.total_weight),
        "guaranteed": weighted(&table.guaranteed_items),
    })
}
fn npc_snapshot(table: &NpcDropTable) -> Value {
    let mut out = snapshot(&table.main);
    out["charms"] = snapshot(table.charm_drops());
    out["tertiary"] = snapshot(table.tertiary_drops());
    out
}
fn shared_kind(name: &str) -> SharedKind {
    SharedKind::ALL
        .into_iter()
        .find(|kind| kind.abbreviation() == name)
        .unwrap()
}
fn clue_level(name: &str) -> ClueLevel {
    [
        ClueLevel::Easy,
        ClueLevel::Medium,
        ClueLevel::Hard,
        ClueLevel::Unknown,
    ]
    .into_iter()
    .find(|level| level.name() == name)
    .unwrap()
}
fn roll_error(error: RollError) -> &'static str {
    match error {
        RollError::BadWeight => "Argument",
        RollError::NoClue => "Index",
        RollError::NullLooter => "Null",
        RollError::Behavior => "Binding",
        RollError::TooManyItems => "native:TooManyItems",
        RollError::RareDepth => "native:RareDepth",
    }
}
fn load_error(error: &LoadError) -> &'static str {
    match error {
        LoadError::File(_) => "File",
        LoadError::Json(_) => "Json",
        LoadError::Null => "Null",
        LoadError::Type => "Type",
        LoadError::Number => "Number",
        LoadError::Definition(_) => "Binding",
    }
}

struct Recorder {
    events: Events,
    clues: HashMap<ClueLevel, Vec<i32>>,
}
impl Host for Recorder {
    fn clue_scrolls(&self, level: ClueLevel) -> Option<&[i32]> {
        self.clues.get(&level).map(Vec::as_slice)
    }
    fn log(&mut self, origin: &'static str, level: LogLevel, message: &str) {
        let level = match level {
            LogLevel::Fine => "FINE",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Err => "ERR",
            LogLevel::Debug => "DEBUG",
        };
        self.events
            .borrow_mut()
            .push(json!(["log", origin, level, message]));
    }
    fn stack_trace(&mut self, _origin: &'static str, failure: ParseFailure) {
        let category = match failure {
            ParseFailure::Xml => "Xml",
            ParseFailure::Number => "Number",
            ParseFailure::NullPath => "Argument",
        };
        self.events
            .borrow_mut()
            .push(json!(["stackTrace", category]));
    }
}

struct Player {
    events: Events,
    wealth: bool,
    equipped: Vec<i64>,
    has_space: bool,
    full: bool,
}
impl PlayerLooter for Player {
    fn removes_nothings(&mut self) -> bool {
        self.events.borrow_mut().push(json!(["removesNothings"]));
        self.wealth
    }
    fn in_equipment(&mut self, item_id: i32) -> bool {
        self.events
            .borrow_mut()
            .push(json!(["inEquipment", item_id, 1]));
        self.equipped.contains(&i64::from(item_id))
    }
    fn update_glove_charges(&mut self) {
        self.events.borrow_mut().push(json!(["fogCharges", 1]));
    }
}
impl Inventory for Player {
    fn has_space_for(&mut self, list: &[RolledItem]) -> bool {
        self.events
            .borrow_mut()
            .push(json!(["hasSpaceFor", items(list)]));
        self.has_space
    }
    fn is_full(&mut self) -> bool {
        self.events.borrow_mut().push(json!(["isFull"]));
        self.full
    }
}

enum Receiver {
    None,
    Entity,
    Player(Player),
}
impl Receiver {
    fn new(spec: &Value, events: &Events) -> Self {
        if spec.is_null() {
            Self::None
        } else if spec["kind"] == "player" {
            Self::Player(Player {
                events: events.clone(),
                wealth: spec["wealth"] == true,
                equipped: spec["equipped"]
                    .as_array()
                    .map(|ids| ids.iter().map(|id| id.as_i64().unwrap()).collect())
                    .unwrap_or_default(),
                has_space: spec["hasSpace"] == true,
                full: spec["full"] == true,
            })
        } else {
            Self::Entity
        }
    }
    fn looter(&mut self) -> Looter<'_> {
        match self {
            Self::None => Looter::None,
            Self::Entity => Looter::Entity,
            Self::Player(player) => Looter::Player(player),
        }
    }
}

/// The definitions the oracle's NPCDefinition adapter serves: created on first
/// lookup, each with its own empty table.
struct Defs<'a> {
    options: &'a Value,
    events: Events,
    /// `None` is a definition whose drop tables are null.
    known: BTreeMap<i32, Option<NpcDropTables>>,
}
impl Defs<'_> {
    fn listed(&self, key: &str, id: i32) -> bool {
        self.options[key]
            .as_array()
            .is_some_and(|ids| ids.contains(&json!(id)))
    }
    fn lookup(&mut self, id: i32) -> Result<&mut Option<NpcDropTables>, LoadError> {
        if self.listed("throwing", id) {
            return Err(LoadError::Definition(format!("forId({id})")));
        }
        if self.listed("missing", id) {
            return Err(LoadError::Null);
        }
        let null_tables = self.listed("nullTables", id);
        Ok(self
            .known
            .entry(id)
            .or_insert_with(|| (!null_tables).then(NpcDropTables::new)))
    }
    fn for_id(&mut self, id: i32) -> Result<&mut Option<NpcDropTables>, LoadError> {
        self.events.borrow_mut().push(json!(["forId", id]));
        self.lookup(id)
    }
}
impl Definitions for Defs<'_> {
    fn assign(&mut self, id: i32, table: &Arc<NpcDropTable>) -> Result<bool, LoadError> {
        Ok(match self.for_id(id)? {
            Some(holder) => {
                holder.table = table.clone();
                true
            }
            None => false,
        })
    }
    fn name(&mut self, id: i32) -> Result<Option<String>, LoadError> {
        self.for_id(id)?;
        Ok(match self.options["names"].get(id.to_string()) {
            Some(name) => name.as_str().map(str::to_owned),
            None => Some(format!("NPC {id}")),
        })
    }
    fn log(&mut self, level: LogLevel, message: &str) {
        let level = if level == LogLevel::Err {
            "ERR"
        } else {
            "FINE"
        };
        self.events
            .borrow_mut()
            .push(json!(["log", "DropTableParser", level, message]));
    }
}

enum Table {
    Weighted(WeightBasedTable),
    Npc(NpcDropTable),
}
impl Table {
    fn main(&self) -> &WeightBasedTable {
        match self {
            Self::Weighted(table) => table,
            Self::Npc(table) => &table.main,
        }
    }
    fn main_mut(&mut self) -> &mut WeightBasedTable {
        match self {
            Self::Weighted(table) => table,
            Self::Npc(table) => &mut table.main,
        }
    }
    fn snapshot(&self) -> Value {
        match self {
            Self::Weighted(table) => snapshot(table),
            Self::Npc(table) => npc_snapshot(table),
        }
    }
}

struct Model<'a> {
    events: Events,
    source: JavaSource,
    shared: SharedTables,
    host: Recorder,
    defs: Defs<'a>,
    tables: HashMap<String, Table>,
}

type Outcome = Result<Value, &'static str>;

impl Model<'_> {
    fn seed(&mut self, seeds: &Value) {
        if let Some(seed) = seeds["random"].as_i64() {
            self.source.random = JavaRandom::new(seed);
        }
        if let Some(seed) = seeds["shuffle"].as_i64() {
            self.source.shuffle = JavaRandom::new(seed);
        }
        if let Some(seed) = seeds["tlr"].as_i64() {
            self.source.thread_local = ThreadLocalRandom::new(seed);
        }
    }
    fn set_clues(&mut self, level: &str, ids: &Value) {
        let level = clue_level(level);
        match ids.as_array() {
            Some(ids) => self.host.clues.insert(level, ids.iter().map(int).collect()),
            None => self.host.clues.remove(&level),
        };
    }
    fn shared_snapshot(&self, kind: SharedKind) -> Value {
        let chance = |table: &[WeightedChanceItem]| -> Value {
            table
                .iter()
                .map(|item| {
                    json!([
                        item.id,
                        item.minimum_amount,
                        item.maximum_amount,
                        item.weight
                    ])
                })
                .collect()
        };
        match kind {
            SharedKind::Rare => snapshot(&self.shared.rare),
            SharedKind::Cele => chance(&self.shared.cele),
            SharedKind::UncommonSeed => chance(&self.shared.uncommon_seed),
            SharedKind::Herb => chance(&self.shared.herb),
            SharedKind::Gem => chance(&self.shared.gem),
            SharedKind::RareSeed => chance(&self.shared.rare_seed),
            SharedKind::AllotmentSeed => chance(&self.shared.allotment_seed),
        }
    }
    fn text<'t>(a: &'t Value, pinned: &'static str) -> &'t str {
        if a["pinned"] == true {
            pinned
        } else {
            a["text"].as_str().unwrap()
        }
    }
    fn act(&mut self, a: &Value) -> Outcome {
        let op = a["op"].as_str().unwrap();
        Ok(match op {
            "seed" => {
                self.seed(a);
                Value::Null
            }
            "load" => {
                let result = if a["missingFile"] == true {
                    parser::load_config(
                        Some("runs/drop-tables-oracle/config/missing-"),
                        &mut self.source,
                        &mut self.defs,
                    )
                } else {
                    parser::load(
                        Self::text(a, PINNED_CONFIG),
                        &mut self.source,
                        &mut self.defs,
                    )
                };
                result.map_err(|error| load_error(&error))?;
                Value::Null
            }
            "tables" => {
                let mut seen: Vec<&Arc<NpcDropTable>> = Vec::new();
                let mut npcs = Vec::new();
                for (id, holder) in &self.defs.known {
                    let index = holder.as_ref().map(|holder| {
                        seen.iter()
                            .position(|table| Arc::ptr_eq(table, &holder.table))
                            .unwrap_or_else(|| {
                                seen.push(&holder.table);
                                seen.len() - 1
                            })
                    });
                    npcs.push(json!([id, index]));
                }
                let tables: Vec<_> = seen.iter().map(|table| npc_snapshot(table)).collect();
                json!({"tables": tables, "npcs": npcs})
            }
            "roll" => {
                let id = int(&a["npc"]);
                let holder = self
                    .defs
                    .lookup(id)
                    .map_err(|error| load_error(&error))?
                    .clone()
                    .ok_or("Null")?;
                let mut receiver = Receiver::new(&a["receiver"], &self.events);
                let events = self.events.clone();
                let behavior = &a["behavior"];
                let mut on_rolled = |list: &mut Vec<RolledItem>| {
                    events.borrow_mut().push(json!(["rolled", id, items(list)]));
                    if behavior["removeFirst"] == true && !list.is_empty() {
                        list.remove(0);
                    }
                    if let Some(add) = behavior["add"].as_array() {
                        list.push(RolledItem {
                            id: int(&add[0]),
                            amount: int(&add[1]),
                        });
                    }
                    if behavior["fail"] == true {
                        return Err(RollError::Behavior);
                    }
                    Ok(())
                };
                let mut ctx = Context::new(&mut self.source, &mut self.shared, &mut self.host);
                let rolled = holder
                    .roll(
                        &mut ctx,
                        receiver.looter(),
                        int(&a["times"]),
                        &mut on_rolled,
                    )
                    .map_err(roll_error)?;
                items(&rolled)
            }
            "table" => {
                let adds = a["adds"].as_array().unwrap();
                let table = match a["kind"].as_str().unwrap() {
                    "created" => Table::Weighted(WeightBasedTable::create(
                        &mut self.source,
                        adds.iter()
                            .map(|row| weighted_item(&row.as_array().unwrap()[1..])),
                    )),
                    kind => {
                        let mut table = if kind == "npc" {
                            Table::Npc(NpcDropTable::new())
                        } else {
                            Table::Weighted(WeightBasedTable::new())
                        };
                        for row in adds {
                            let row = row.as_array().unwrap();
                            let source = &mut self.source;
                            match (row[0].as_str().unwrap(), &mut table) {
                                ("main", table) => {
                                    table.main_mut().add(source, weighted_item(&row[1..]));
                                }
                                ("charm", Table::Npc(table)) => {
                                    table.add_to_charms(source, weighted_item(&row[1..]));
                                }
                                ("tertiary", Table::Npc(table)) => {
                                    table.add_to_tertiary(source, weighted_item(&row[1..]));
                                }
                                ("insert", table) => {
                                    let weight = double(&row[2]);
                                    let table = table.main_mut();
                                    match row[1].as_str().unwrap() {
                                        "EasyClue" => table.insert_easy_clue(source, weight),
                                        "MediumClue" => table.insert_medium_clue(source, weight),
                                        "HardClue" => table.insert_hard_clue(source, weight),
                                        "RDTRoll" => table.insert_rdt_roll(source, weight),
                                        "CELEDTRoll" => table.insert_celedt_roll(source, weight),
                                        "SEEDDTRoll" => table.insert_seeddt_roll(source, weight),
                                        "HERBDTRoll" => table.insert_herbdt_roll(source, weight),
                                        "GDTRoll" => table.insert_gdt_roll(source, weight),
                                        "RSDTRoll" => table.insert_rsdt_roll(source, weight),
                                        "ASDTRoll" => table.insert_asdt_roll(source, weight),
                                        other => panic!("unknown insert {other}"),
                                    };
                                }
                                (other, _) => panic!("unknown add {other}"),
                            }
                        }
                        table
                    }
                };
                let value = table.snapshot();
                self.tables
                    .insert(a["name"].as_str().unwrap().into(), table);
                value
            }
            "assign" => {
                let Table::Npc(table) = &self.tables[a["table"].as_str().unwrap()] else {
                    panic!("assign needs an NPC table");
                };
                let table = Arc::new(table.clone());
                let holder = self
                    .defs
                    .lookup(int(&a["npc"]))
                    .map_err(|error| load_error(&error))?;
                holder.as_mut().ok_or("Null")?.table = table;
                Value::Null
            }
            "rollTable" => {
                let mut receiver = Receiver::new(&a["receiver"], &self.events);
                // The one-argument overload rolls once.
                let times = a["times"].as_i64().map_or(1, |times| times as i32);
                let mut ctx = Context::new(&mut self.source, &mut self.shared, &mut self.host);
                let rolled = match &self.tables[a["table"].as_str().unwrap()] {
                    Table::Weighted(table) => table.roll(&mut ctx, receiver.looter(), times),
                    Table::Npc(table) => table.roll(&mut ctx, receiver.looter(), times),
                };
                items(&rolled.map_err(roll_error)?)
            }
            "convert" => {
                let list: Vec<_> = a["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| weighted_item(row.as_array().unwrap()))
                    .collect();
                let mut receiver = Receiver::new(&a["receiver"], &self.events);
                let mut ctx = Context::new(&mut self.source, &mut self.shared, &mut self.host);
                items(
                    &drops::convert_weighted_items(&mut ctx, &list, receiver.looter())
                        .map_err(roll_error)?,
                )
            }
            "excluding" => {
                let ids: Vec<_> = a["ids"].as_array().unwrap().iter().map(int).collect();
                let out = self.tables[a["table"].as_str().unwrap()]
                    .main()
                    .excluding(&mut self.source, &ids);
                let value = snapshot(&out);
                self.tables
                    .insert(a["as"].as_str().unwrap().into(), Table::Weighted(out));
                value
            }
            "canRoll" => {
                let Receiver::Player(mut player) = Receiver::new(&a["receiver"], &self.events)
                else {
                    panic!("canRoll needs a player");
                };
                json!(
                    self.tables[a["table"].as_str().unwrap()]
                        .main()
                        .can_roll(&mut self.source, &mut player)
                )
            }
            "describe" => {
                let names = &a["names"];
                json!(
                    self.tables[a["table"].as_str().unwrap()].main().describe(
                        &mut |id| match names.get(id.to_string()) {
                            Some(name) => name.as_str().map(str::to_owned),
                            None => Some(format!("Item {id}")),
                        }
                    )
                )
            }
            "snapshot" => self.tables[a["table"].as_str().unwrap()].snapshot(),
            "shared" => {
                let kind = shared_kind(a["table"].as_str().unwrap());
                let text = Self::text(a, pinned_shared(kind));
                self.shared
                    .parse(kind, text, &mut self.source, &mut self.host);
                self.shared_snapshot(kind)
            }
            "startup" => {
                let name = a["table"].as_str().unwrap();
                let kind = shared_kind(name);
                let (path, file) = match a["path"].as_str().unwrap() {
                    "present" => (
                        Some(format!("{FOLDER}/{name}.xml")),
                        Some(Self::text(a, pinned_shared(kind))),
                    ),
                    "missing" => (Some(format!("{FOLDER}/missing-{name}.xml")), None),
                    _ => (None, None),
                };
                self.shared.startup(
                    kind,
                    path.as_deref(),
                    file,
                    &mut self.source,
                    &mut self.host,
                );
                self.shared_snapshot(kind)
            }
            "rareAdd" => {
                let item = weighted_item(a["item"].as_array().unwrap());
                self.shared.rare.add(&mut self.source, item);
                self.shared_snapshot(SharedKind::Rare)
            }
            "sharedSnapshot" => self.shared_snapshot(shared_kind(a["table"].as_str().unwrap())),
            "retrieve" => {
                let kind = shared_kind(a["table"].as_str().unwrap());
                let item = if kind == SharedKind::Rare {
                    let mut receiver = Receiver::new(&a["receiver"], &self.events);
                    let mut ctx = Context::new(&mut self.source, &mut self.shared, &mut self.host);
                    shared::retrieve_rare(&mut ctx, receiver.looter()).map_err(roll_error)?
                } else {
                    shared::roll_weighted_chance_table(
                        &mut self.source,
                        self.shared.chance(kind).unwrap(),
                    )
                };
                item.map_or(Value::Null, |item| json!([item.id, item.amount]))
            }
            "clues" => {
                self.set_clues(a["level"].as_str().unwrap(), &a["ids"]);
                Value::Null
            }
            "clue" => {
                let level = clue_level(a["level"].as_str().unwrap());
                let mut ctx = Context::new(&mut self.source, &mut self.shared, &mut self.host);
                drops::clue(&mut ctx, level)
                    .map_err(roll_error)?
                    .map_or(Value::Null, |item| json!([item.id, item.amount]))
            }
            "random" => match a["fn"].as_str().unwrap() {
                "range" => json!(random::random_range(
                    &mut self.source,
                    int(&a["a"]),
                    int(&a["b"])
                )),
                "max" => json!(random::random(&mut self.source, int(&a["n"]))),
                "double" => json!(bits(
                    random::random_double(&mut self.source, double(&a["bits"]))
                        .map_err(|_| "Argument")?
                )),
                _ => json!(self.source.next_boolean()),
            },
            "stabilizer" => json!(bits(
                NpcDropTables::stabilizer_ratio(int(&a["combat"])).map_err(|_| "Arithmetic")?
            )),
            "modRate" => {
                let mut holder = NpcDropTables::new();
                let before = bits(holder.mod_rate());
                holder.set_mod_rate(double(&a["bits"]));
                json!([before, bits(holder.mod_rate())])
            }
            "constants" => {
                let frequencies: Vec<_> = DropFrequency::ALL
                    .iter()
                    .enumerate()
                    .map(|(ordinal, frequency)| {
                        json!([frequency.name(), ordinal, frequency.rate()])
                    })
                    .collect();
                json!({
                    "dropRates": NpcDropTables::DROP_RATES,
                    "messageNpcs": NpcDropTables::MESSAGE_NPCS,
                    "frequencies": frequencies,
                    "slots": {
                        "RDT": drops::SLOT_RDT,
                        "CLUE_EASY": drops::SLOT_CLUE_EASY,
                        "CLUE_MEDIUM": drops::SLOT_CLUE_MEDIUM,
                        "CLUE_HARD": drops::SLOT_CLUE_HARD,
                        "CELEDT": drops::SLOT_CELEDT,
                        "USDT": drops::SLOT_USDT,
                        "HDT": drops::SLOT_HDT,
                        "GDT": drops::SLOT_GDT,
                        "RSDT": drops::SLOT_RSDT,
                        "ASDT": drops::SLOT_ASDT,
                    },
                    "clueSlots": drops::CLUE_SCROLL_SLOTS,
                    "sharedSlotIds": [
                        drops::SLOT_HDT,
                        drops::SLOT_GDT,
                        drops::SLOT_CELEDT,
                        drops::SLOT_USDT,
                        drops::SLOT_RSDT,
                        drops::SLOT_ASDT
                    ],
                })
            }
            other => panic!("unknown action {other}"),
        })
    }
}

/// One step's native observations, shaped like the fixture's.
pub struct Step {
    pub value: Value,
    pub error: Value,
    pub events: Value,
    /// Whether each generator stands where the recorded draw counts put it:
    /// RandomFunction.RANDOM, the shuffle generator, the thread-local one.
    pub aligned: [bool; 3],
}

pub struct Evaluation {
    pub steps: Vec<Step>,
    /// The next value of each generator after the last step.
    pub probe: Value,
}

/// Runs a case. `draws` gives the recorded per-step advance of each generator
/// (`None` for a step that reseeds); the native generators are compared with
/// independent ones advanced by exactly those counts.
pub fn evaluate(case: &Value, draws: &[Option<[u64; 3]>]) -> Evaluation {
    let events: Events = Rc::default();
    let mut model = Model {
        events: events.clone(),
        source: JavaSource::new(530, 531, 532),
        shared: SharedTables::new(),
        host: Recorder {
            events: events.clone(),
            clues: HashMap::new(),
        },
        defs: Defs {
            options: &case["definitions"],
            events: events.clone(),
            known: BTreeMap::new(),
        },
        tables: HashMap::new(),
    };
    if let Some(clues) = case["clues"].as_object() {
        for (level, ids) in clues {
            model.set_clues(level, ids);
        }
    }
    model.seed(&case["seeds"]);
    let mut shadow = model.source.clone();
    let mut steps = Vec::new();
    for (action, draws) in case["actions"].as_array().unwrap().iter().zip(draws) {
        events.borrow_mut().clear();
        let outcome = model.act(action);
        let aligned = match draws {
            Some(draws) => {
                for _ in 0..draws[0] {
                    shadow.random.next_i32();
                }
                for _ in 0..draws[1] {
                    shadow.shuffle.next_i32();
                }
                for _ in 0..draws[2] {
                    shadow.thread_local.next_long();
                }
                [
                    shadow.random.clone().next_i64() == model.source.random.clone().next_i64(),
                    shadow.shuffle.clone().next_i64() == model.source.shuffle.clone().next_i64(),
                    shadow.thread_local == model.source.thread_local,
                ]
            }
            None => {
                shadow = model.source.clone();
                [true; 3]
            }
        };
        let (value, error) = match outcome {
            Ok(value) => (value, Value::Null),
            Err(error) => (Value::Null, json!(error)),
        };
        steps.push(Step {
            value,
            error,
            events: Value::Array(events.borrow().clone()),
            aligned,
        });
    }
    let probe = json!([
        model.source.random.next_i64(),
        model.source.shuffle.next_i64(),
        model.source.thread_local.next_long()
    ]);
    Evaluation { steps, probe }
}
