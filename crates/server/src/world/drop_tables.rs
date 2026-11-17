//! NPC drop tables: the original `WeightBasedTable`, `WeightedItem` and
//! `NPCDropTable` structures, the table holder of `NPCDropTables`, and the roll
//! with its original draw order.
//!
//! A roll here ends with the list of items the original would construct. What
//! the original then does with that list (`NPCDropTables.drop` and
//! `createDrop`: bone crusher, item plugins, loot share, drop messages, ground
//! items) is in [`drop`]; the death that leads there is in
//! `world::npc_death`. Systems the roll consults but this crate does not have
//! yet are inputs: [`Looter`] for the receiving entity and [`Host`] for the
//! clue registry and the log.
pub mod definitions;
pub mod drop;
pub mod parser;
pub mod random;
pub mod shared;

use random::{Source, random_double, random_range};
use rs09_game_core::java_double;
use shared::SharedTables;
use std::sync::Arc;

/// Item ids the original reuses as table slots (`WeightBasedTable.SLOT_*`).
pub const SLOT_RDT: i32 = 31;
pub const SLOT_CLUE_EASY: i32 = 1;
pub const SLOT_CLUE_MEDIUM: i32 = 5733;
pub const SLOT_CLUE_HARD: i32 = 12070;
pub const CLUE_SCROLL_SLOTS: [i32; 3] = [SLOT_CLUE_EASY, SLOT_CLUE_MEDIUM, SLOT_CLUE_HARD];
pub const SLOT_CELEDT: i32 = 799;
pub const SLOT_USDT: i32 = 14422;
pub const SLOT_HDT: i32 = 14424;
pub const SLOT_GDT: i32 = 14426;
pub const SLOT_RSDT: i32 = 14428;
pub const SLOT_ASDT: i32 = 14430;
/// `Items.DWARF_REMAINS_0`: an entry that drops nothing.
pub const NOTHING: i32 = 0;

/// Fist of Guthix gloves and the grimy herb each one turns a herb-slot drop
/// into, in the order the original asks about them.
pub const HERB_GLOVES: [(i32, i32); 4] = [(12856, 209), (12857, 211), (12858, 213), (12859, 215)];

/// Native bound on the weighted items of one roll. The original has none: an
/// amount range on a herb or seed slot can ask for two billion repeats.
pub const MAX_WEIGHTED_ITEMS: usize = 1 << 16;
/// Native bound on a rare drop table handing off to itself. The original
/// recurses until the JVM stack overflows.
pub const MAX_RARE_DEPTH: u32 = 64;

/// `content.global.activity.ttrail.ClueLevel`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ClueLevel {
    Easy,
    Medium,
    Hard,
    Unknown,
}
impl ClueLevel {
    /// The enum constant's name, as the original prints it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Easy => "EASY",
            Self::Medium => "MEDIUM",
            Self::Hard => "HARD",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// `core.tools.Log`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Fine,
    Info,
    Warn,
    Err,
    Debug,
}

/// Services of systems that are not ported.
pub trait Host {
    /// `ClueScrollPlugin.ORGANIZED.get(level)`: the clue item ids registered for
    /// the level, in registration order. `None` when none was registered.
    fn clue_scrolls(&self, level: ClueLevel) -> Option<&[i32]>;
    /// `core.api.log(origin, type, message)`; `origin` is the simple class name.
    fn log(&mut self, origin: &'static str, level: LogLevel, message: &str);
    /// A shared-table file stopped loading; the original prints the caught
    /// exception's stack trace to standard error here.
    fn stack_trace(&mut self, origin: &'static str, failure: shared::ParseFailure);
}

/// What the roll asks of a looting player. Each call is one original call, in
/// the original order; none of them is answered from native equipment state.
pub trait PlayerLooter {
    /// `core.api.shouldRemoveNothings(player)` (a ring of wealth is worn).
    fn removes_nothings(&mut self) -> bool;
    /// `core.api.inEquipment(player, item_id)`.
    fn in_equipment(&mut self, item_id: i32) -> bool;
    /// `FOGGlovesManager.updateCharges(player)`.
    fn update_glove_charges(&mut self);
}

/// The `receiver` argument of the original roll.
pub enum Looter<'a> {
    /// `null`.
    None,
    /// An entity that is not a player.
    Entity,
    Player(&'a mut dyn PlayerLooter),
}
impl Looter<'_> {
    pub fn reborrow(&mut self) -> Looter<'_> {
        match self {
            Self::None => Looter::None,
            Self::Entity => Looter::Entity,
            Self::Player(player) => Looter::Player(&mut **player),
        }
    }
}

/// Everything a roll draws from or changes.
pub struct Context<'a> {
    pub random: &'a mut dyn Source,
    /// The shared tables; rolling a weighted chance table reorders it.
    pub shared: &'a mut SharedTables,
    pub host: &'a mut dyn Host,
    rare_depth: u32,
}
impl<'a> Context<'a> {
    pub fn new(
        random: &'a mut dyn Source,
        shared: &'a mut SharedTables,
        host: &'a mut dyn Host,
    ) -> Self {
        Self {
            random,
            shared,
            host,
            rare_depth: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollError {
    /// `IllegalArgumentException`: a table with weighted entries has a total
    /// weight that is zero, negative or NaN.
    BadWeight,
    /// `IndexOutOfBoundsException`: a clue level is registered with no clues.
    NoClue,
    /// `NullPointerException`: `NPCDropTables.roll` without a looter, raised
    /// when the list reaches the behaviour callback's non-null parameter.
    NullLooter,
    /// The behaviour callback failed.
    Behavior,
    /// Native bound, see [`MAX_WEIGHTED_ITEMS`].
    TooManyItems,
    /// Native bound, see [`MAX_RARE_DEPTH`].
    RareDepth,
}
impl std::fmt::Display for RollError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RollError {}

/// `core.api.utils.WeightedItem`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedItem {
    pub id: i32,
    pub min_amt: i32,
    pub max_amt: i32,
    pub weight: f64,
    pub guaranteed: bool,
}
impl WeightedItem {
    pub const fn new(id: i32, min_amt: i32, max_amt: i32, weight: f64, guaranteed: bool) -> Self {
        Self {
            id,
            min_amt,
            max_amt,
            weight,
            guaranteed,
        }
    }
    /// `getItem()`: one amount draw unless both ends are equal.
    pub fn item(&self, random: &mut dyn Source) -> RolledItem {
        RolledItem {
            id: self.id,
            amount: random_range(random, self.min_amt, self.max_amt),
        }
    }
}

/// The arguments of one `Item(id, amount)` the roll constructs. Building the
/// item itself (definition lookup, the 16-bit id the original keeps) is the
/// caller's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RolledItem {
    pub id: i32,
    pub amount: i32,
}

/// `WeightBasedTable.canRoll` asks the player's inventory at most these two
/// questions, in this order.
pub trait Inventory {
    /// `inventory.hasSpaceFor(*items)`.
    fn has_space_for(&mut self, items: &[RolledItem]) -> bool;
    /// `inventory.isFull`.
    fn is_full(&mut self) -> bool;
}

/// `core.api.utils.WeightBasedTable`: the weighted entries in their stored
/// order, their summed weight and the guaranteed entries.
///
/// The original is an `ArrayList` with public fields, so the three are public
/// here too. `total_weight` is the sum in insertion order, which is not the
/// stored order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeightBasedTable {
    pub items: Vec<WeightedItem>,
    pub total_weight: f64,
    pub guaranteed_items: Vec<WeightedItem>,
}
impl WeightBasedTable {
    pub fn new() -> Self {
        Self::default()
    }
    /// `WeightBasedTable.create(vararg items)`.
    pub fn create(random: &mut dyn Source, items: impl IntoIterator<Item = WeightedItem>) -> Self {
        let mut table = Self::new();
        for item in items {
            table.add(random, item);
        }
        table
    }
    /// `add(element)`. A weighted entry goes to a random position: it swaps
    /// with the entry there, which moves to the end. The first one draws
    /// nothing.
    pub fn add(&mut self, random: &mut dyn Source, element: WeightedItem) -> bool {
        if element.guaranteed {
            self.guaranteed_items.push(element);
        } else {
            self.total_weight += element.weight;
            let size = self.items.len();
            let index = random_range(random, 0, size as i32) as usize;
            self.items.push(element);
            self.items.swap(index, size);
        }
        true
    }
    /// `roll(receiver, times)`.
    pub fn roll(
        &self,
        ctx: &mut Context<'_>,
        receiver: Looter<'_>,
        times: i32,
    ) -> Result<Vec<RolledItem>, RollError> {
        let mut selected = Vec::new();
        self.select(ctx.random, times, &mut selected)?;
        convert_weighted_items(ctx, &selected, receiver)
    }
    /// The first half of `roll`: per time, every guaranteed entry and then one
    /// weighted entry. All of it is drawn before any item is made.
    fn select(
        &self,
        random: &mut dyn Source,
        times: i32,
        out: &mut Vec<WeightedItem>,
    ) -> Result<(), RollError> {
        if self.items.is_empty() && self.guaranteed_items.is_empty() {
            // Nothing is selected or drawn, however many times are asked for.
            return Ok(());
        }
        for _ in 0..times.max(0) {
            for item in &self.guaranteed_items {
                push_slot(random, item, out)?;
            }
            if !self.items.is_empty() {
                let mut rng_weight =
                    random_double(random, self.total_weight).map_err(|_| RollError::BadWeight)?;
                for item in &self.items {
                    rng_weight -= item.weight;
                    if rng_weight <= 0.0 {
                        push_slot(random, item, out)?;
                        break;
                    }
                }
            }
        }
        Ok(())
    }
    /// `canRoll(player)`. Every guaranteed entry draws its amount first.
    pub fn can_roll(&self, random: &mut dyn Source, inventory: &mut dyn Inventory) -> bool {
        let guaranteed: Vec<_> = self
            .guaranteed_items
            .iter()
            .map(|item| item.item(random))
            .collect();
        (!guaranteed.is_empty() && inventory.has_space_for(&guaranteed)) || !inventory.is_full()
    }
    /// `excluding(vararg ids)`: a new table, re-added entry by entry (so it
    /// draws new positions), weighted entries first.
    pub fn excluding(&self, random: &mut dyn Source, ids: &[i32]) -> Self {
        let mut filtered = Self::new();
        for item in self.items.iter().chain(&self.guaranteed_items) {
            if !ids.contains(&item.id) {
                filtered.add(random, *item);
            }
        }
        filtered
    }
    fn insert(&mut self, random: &mut dyn Source, slot: i32, weight: f64) -> &mut Self {
        self.add(random, WeightedItem::new(slot, 1, 1, weight, false));
        self
    }
    pub fn insert_easy_clue(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_CLUE_EASY, weight)
    }
    pub fn insert_medium_clue(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_CLUE_MEDIUM, weight)
    }
    pub fn insert_hard_clue(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_CLUE_HARD, weight)
    }
    pub fn insert_rdt_roll(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_RDT, weight)
    }
    pub fn insert_celedt_roll(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_CELEDT, weight)
    }
    pub fn insert_seeddt_roll(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_USDT, weight)
    }
    pub fn insert_herbdt_roll(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_HDT, weight)
    }
    pub fn insert_gdt_roll(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_GDT, weight)
    }
    pub fn insert_rsdt_roll(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_RSDT, weight)
    }
    pub fn insert_asdt_roll(&mut self, random: &mut dyn Source, weight: f64) -> &mut Self {
        self.insert(random, SLOT_ASDT, weight)
    }
    /// `toString()`: one line per weighted entry. `name` answers
    /// `ItemDefinition.forId(id).name`; `None` prints as `null`.
    pub fn describe(&self, name: &mut dyn FnMut(i32) -> Option<String>) -> String {
        let mut out = String::new();
        for item in &self.items {
            out.push_str(name(item.id).as_deref().unwrap_or("null"));
            out.push_str(" || Weight: ");
            out.push_str(&java_double::format(item.weight));
            out.push_str(&format!(
                " || MinAmt: {} || maxAmt: {}\n",
                item.min_amt, item.max_amt
            ));
        }
        out
    }
}

/// One selected entry. Only the herb and uncommon-seed slots repeat: their
/// amount range is the number of hand-offs, not an item amount.
fn push_slot(
    random: &mut dyn Source,
    item: &WeightedItem,
    out: &mut Vec<WeightedItem>,
) -> Result<(), RollError> {
    let count = if item.id == SLOT_HDT || item.id == SLOT_USDT {
        random_range(random, item.min_amt, item.max_amt).max(0) as usize
    } else {
        1
    };
    if count > MAX_WEIGHTED_ITEMS - out.len().min(MAX_WEIGHTED_ITEMS) {
        return Err(RollError::TooManyItems);
    }
    out.extend(std::iter::repeat_n(*item, count));
    Ok(())
}

/// `convertWeightedItems(weightedItems, receiver)`: each selected entry becomes
/// an item, a hand-off to a shared table, or nothing.
pub fn convert_weighted_items(
    ctx: &mut Context<'_>,
    weighted_items: &[WeightedItem],
    mut receiver: Looter<'_>,
) -> Result<Vec<RolledItem>, RollError> {
    let mut safe_items = Vec::new();
    for e in weighted_items {
        let item = match e.id {
            SLOT_CLUE_EASY => clue(ctx, ClueLevel::Easy)?,
            SLOT_CLUE_MEDIUM => clue(ctx, ClueLevel::Medium)?,
            SLOT_CLUE_HARD => clue(ctx, ClueLevel::Hard)?,
            SLOT_RDT => shared::retrieve_rare(ctx, receiver.reborrow())?,
            SLOT_CELEDT => shared::roll_weighted_chance_table(ctx.random, &mut ctx.shared.cele),
            SLOT_USDT => {
                shared::roll_weighted_chance_table(ctx.random, &mut ctx.shared.uncommon_seed)
            }
            SLOT_HDT => herb(ctx, &mut receiver),
            SLOT_GDT => shared::roll_weighted_chance_table(ctx.random, &mut ctx.shared.gem),
            SLOT_RSDT => shared::roll_weighted_chance_table(ctx.random, &mut ctx.shared.rare_seed),
            SLOT_ASDT => {
                shared::roll_weighted_chance_table(ctx.random, &mut ctx.shared.allotment_seed)
            }
            NOTHING => continue,
            _ => Some(e.item(ctx.random)),
        };
        safe_items.extend(item);
    }
    Ok(safe_items)
}

/// `ClueScrollPlugin.getClue(level)`.
pub fn clue(ctx: &mut Context<'_>, level: ClueLevel) -> Result<Option<RolledItem>, RollError> {
    let Some(count) = ctx.host.clue_scrolls(level).map(<[i32]>::len) else {
        ctx.host.log(
            "ClueScrollPlugin",
            LogLevel::Err,
            &format!("Error! There are no clues for level {}!", level.name()),
        );
        return Ok(None);
    };
    let index = random::random(ctx.random, count.min(i32::MAX as usize) as i32) as usize;
    let id = ctx
        .host
        .clue_scrolls(level)
        .and_then(|clues| clues.get(index).copied())
        .ok_or(RollError::NoClue)?;
    Ok(Some(RolledItem { id, amount: 1 }))
}

/// The herb slot. The coin is flipped before the receiver is looked at; a
/// player who wins it and wears one of the gloves gets that glove's herb and
/// loses a charge, everyone else rolls the herb table.
fn herb(ctx: &mut Context<'_>, receiver: &mut Looter<'_>) -> Option<RolledItem> {
    if ctx.random.next_boolean() {
        if let Looter::Player(player) = receiver {
            for (gloves, herb) in HERB_GLOVES {
                if player.in_equipment(gloves) {
                    player.update_glove_charges();
                    return Some(RolledItem {
                        id: herb,
                        amount: 1,
                    });
                }
            }
        }
    }
    shared::roll_weighted_chance_table(ctx.random, &mut ctx.shared.herb)
}

/// `core.api.utils.NPCDropTable`: a main table with a charm and a tertiary
/// table beside it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NpcDropTable {
    pub main: WeightBasedTable,
    charm_drops: WeightBasedTable,
    tertiary_drops: WeightBasedTable,
}
impl NpcDropTable {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, random: &mut dyn Source, element: WeightedItem) -> bool {
        self.main.add(random, element)
    }
    pub fn add_to_charms(&mut self, random: &mut dyn Source, element: WeightedItem) -> bool {
        self.charm_drops.add(random, element)
    }
    pub fn add_to_tertiary(&mut self, random: &mut dyn Source, element: WeightedItem) -> bool {
        self.tertiary_drops.add(random, element)
    }
    pub fn charm_drops(&self) -> &WeightBasedTable {
        &self.charm_drops
    }
    pub fn tertiary_drops(&self) -> &WeightBasedTable {
        &self.tertiary_drops
    }
    /// `roll(receiver, times)`: the charm table for every time, then the
    /// tertiary table, then the main table. Each is selected and converted
    /// before the next starts.
    pub fn roll(
        &self,
        ctx: &mut Context<'_>,
        mut receiver: Looter<'_>,
        times: i32,
    ) -> Result<Vec<RolledItem>, RollError> {
        let mut items = self.charm_drops.roll(ctx, receiver.reborrow(), times)?;
        items.extend(self.tertiary_drops.roll(ctx, receiver.reborrow(), times)?);
        items.extend(self.main.roll(ctx, receiver, times)?);
        Ok(items)
    }
}

/// `NPCDropTables.getStabilizerRatio()` divided by zero (combat level -1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DivideByZero;

/// The part of `core.game.node.entity.npc.drop.NPCDropTables` that holds and
/// rolls a table. `drop`, `createDrop`, `getLooter` and the drop messages are
/// in [`drop`].
#[derive(Clone, Debug, Default)]
pub struct NpcDropTables {
    /// Rows of the configuration that name several NPCs give them one table.
    pub table: Arc<NpcDropTable>,
    mod_rate: f64,
}
impl NpcDropTables {
    /// The drop rates (0 = common, 1 = uncommon, 2 = rare, 3 = very rare).
    pub const DROP_RATES: [i32; 4] = [750, 150, 15, 5];
    /// The NPCs whose drops the original announces to nearby players.
    pub const MESSAGE_NPCS: [i32; 29] = [
        50, 7133, 7134, 2881, 2882, 2883, 3200, 3340, 6247, 6203, 6260, 6222, 2745, 1160, 8133,
        8610, 8611, 8612, 8613, 8614, 6204, 6206, 6208, 6261, 6263, 6265, 6223, 6225, 6227,
    ];

    pub fn new() -> Self {
        Self::default()
    }
    /// `roll(npc, looter, times)`. `on_rolled` is `npc.behavior.onDropTableRolled`:
    /// it receives the list that is returned and may change it.
    pub fn roll(
        &self,
        ctx: &mut Context<'_>,
        mut looter: Looter<'_>,
        times: i32,
        on_rolled: &mut dyn FnMut(&mut Vec<RolledItem>) -> Result<(), RollError>,
    ) -> Result<Vec<RolledItem>, RollError> {
        let mut drops = self.table.roll(ctx, looter.reborrow(), times)?;
        if matches!(looter, Looter::None) {
            return Err(RollError::NullLooter);
        }
        on_rolled(&mut drops)?;
        Ok(drops)
    }
    /// `getStabilizerRatio()` for a definition's combat level. The original
    /// divides integers: the result is 10 at level 0, -10 at level -2 and 0
    /// everywhere else.
    pub fn stabilizer_ratio(combat_level: i32) -> Result<f64, DivideByZero> {
        let ratio = 1i32
            .checked_div(1i32.wrapping_add(combat_level))
            .ok_or(DivideByZero)?;
        Ok(f64::from(ratio.wrapping_mul(10)))
    }
    pub fn mod_rate(&self) -> f64 {
        self.mod_rate
    }
    pub fn set_mod_rate(&mut self, mod_rate: f64) {
        self.mod_rate = mod_rate;
    }
}

/// `core.game.node.entity.npc.drop.DropFrequency`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DropFrequency {
    Always,
    Common,
    Uncommon,
    Rare,
    VeryRare,
}
impl DropFrequency {
    pub const ALL: [Self; 5] = [
        Self::Always,
        Self::Common,
        Self::Uncommon,
        Self::Rare,
        Self::VeryRare,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Always => "ALWAYS",
            Self::Common => "COMMON",
            Self::Uncommon => "UNCOMMON",
            Self::Rare => "RARE",
            Self::VeryRare => "VERY_RARE",
        }
    }
    /// `DropFrequency.rate(frequency)`.
    pub const fn rate(self) -> i32 {
        [1, 5, 15, 30, 60][self as usize]
    }
}
