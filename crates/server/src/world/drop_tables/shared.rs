//! The shared tables a drop-table slot hands off to: the rare drop table
//! (`RareDropTable.kt`) and the six weighted chance tables (`CELEMinorTable`,
//! `UncommonSeedDropTable`, `HerbDropTable`, `GemDropTable`, `RareSeedDropTable`,
//! `AllotmentSeedDropTable`), each loaded from an XML file of `item` elements.
use super::{
    Context, Host, LogLevel, Looter, MAX_RARE_DEPTH, NOTHING, RollError, RolledItem,
    WeightBasedTable, WeightedItem, convert_weighted_items,
    random::{self, Source, random_double, random_range},
};
use rs09_game_core::java_string::parse_i32;

/// `core.game.node.item.WeightedChanceItem`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeightedChanceItem {
    pub id: i32,
    pub minimum_amount: i32,
    pub maximum_amount: i32,
    pub weight: i32,
}
impl WeightedChanceItem {
    /// `getItem()`.
    pub fn item(&self, random: &mut dyn Source) -> RolledItem {
        RolledItem {
            id: self.id,
            amount: random_range(random, self.minimum_amount, self.maximum_amount),
        }
    }
}

/// `RandomFunction.rollWeightedChanceTable(table)`. The table is shuffled in
/// place on every call, after the pick is drawn, and stays in that order.
///
/// The comparison is `rand <= weight`, as in the original: with a pick in
/// `0..sum`, the first entry after the shuffle wins one draw more often than
/// its weight.
pub fn roll_weighted_chance_table(
    random: &mut dyn Source,
    table: &mut [WeightedChanceItem],
) -> Option<RolledItem> {
    let sum_of_weights = table
        .iter()
        .fold(0i32, |sum, item| sum.wrapping_add(item.weight));
    let mut rand = random::random(random, sum_of_weights);
    random::shuffle(random, table);
    for item in table.iter() {
        if rand <= item.weight {
            return Some(item.item(random));
        }
        rand = rand.wrapping_sub(item.weight);
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SharedKind {
    Rare,
    Cele,
    UncommonSeed,
    Herb,
    Gem,
    RareSeed,
    AllotmentSeed,
}
impl SharedKind {
    pub const ALL: [Self; 7] = [
        Self::Rare,
        Self::Cele,
        Self::UncommonSeed,
        Self::Herb,
        Self::Gem,
        Self::RareSeed,
        Self::AllotmentSeed,
    ];
    /// The simple name of the original class.
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Rare => "RareDropTable",
            Self::Cele => "CELEMinorTable",
            Self::UncommonSeed => "UncommonSeedDropTable",
            Self::Herb => "HerbDropTable",
            Self::Gem => "GemDropTable",
            Self::RareSeed => "RareSeedDropTable",
            Self::AllotmentSeed => "AllotmentSeedDropTable",
        }
    }
    /// The abbreviation in the original's "Can't locate" message, which is
    /// also the pinned file's name.
    pub const fn abbreviation(self) -> &'static str {
        match self {
            Self::Rare => "RDT",
            Self::Cele => "CELEDT",
            Self::UncommonSeed => "USDT",
            Self::Herb => "HDT",
            Self::Gem => "GDT",
            Self::RareSeed => "RSDT",
            Self::AllotmentSeed => "ASDT",
        }
    }
    /// The original's message after `startup` parsed its file.
    const fn loaded_message(self) -> &'static str {
        match self {
            Self::Rare => "Initialized Rare Drop Table from ",
            Self::Cele => "Loaded up Chaos Elemental drop table from ",
            Self::UncommonSeed => "Initialized Uncommon Seed Drop Table from ",
            Self::Herb => "Initialized Herb Drop Table from ",
            Self::Gem => "Initialized Gem Drop Table from ",
            Self::RareSeed => "Initialized Rare Seed Drop Table from ",
            Self::AllotmentSeed => "Initialized Allotment Seed Drop Table from ",
        }
    }
}

/// Why a shared-table file stopped loading. The original catches the exception
/// and prints its stack trace; entries added before it stay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseFailure {
    /// The document is not well-formed XML (`SAXParseException`).
    Xml,
    /// An `item` attribute is missing or not a Java `int` (`NumberFormatException`).
    Number,
    /// `startup` ran with a null path (`IllegalArgumentException`).
    NullPath,
}

/// The seven shared tables. The original keeps each in a static field.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SharedTables {
    pub rare: WeightBasedTable,
    pub cele: Vec<WeightedChanceItem>,
    pub uncommon_seed: Vec<WeightedChanceItem>,
    pub herb: Vec<WeightedChanceItem>,
    pub gem: Vec<WeightedChanceItem>,
    pub rare_seed: Vec<WeightedChanceItem>,
    pub allotment_seed: Vec<WeightedChanceItem>,
}
impl SharedTables {
    pub fn new() -> Self {
        Self::default()
    }
    /// The weighted chance table of a kind; `None` for the rare drop table.
    pub fn chance(&mut self, kind: SharedKind) -> Option<&mut Vec<WeightedChanceItem>> {
        match kind {
            SharedKind::Rare => None,
            SharedKind::Cele => Some(&mut self.cele),
            SharedKind::UncommonSeed => Some(&mut self.uncommon_seed),
            SharedKind::Herb => Some(&mut self.herb),
            SharedKind::Gem => Some(&mut self.gem),
            SharedKind::RareSeed => Some(&mut self.rare_seed),
            SharedKind::AllotmentSeed => Some(&mut self.allotment_seed),
        }
    }
    /// `parse(file)` over the file's text: every `item` element of the
    /// document, in document order, is appended. Loading twice appends twice.
    /// The rare drop table draws an insertion position per entry. A failure
    /// is also reported to `host`, at the point the original prints it.
    pub fn parse(
        &mut self,
        kind: SharedKind,
        xml: &str,
        random: &mut dyn Source,
        host: &mut dyn Host,
    ) -> Option<ParseFailure> {
        let failure = self.parse_items(kind, xml, random);
        if let Some(failure) = failure {
            host.stack_trace(kind.class_name(), failure);
        }
        failure
    }
    fn parse_items(
        &mut self,
        kind: SharedKind,
        xml: &str,
        random: &mut dyn Source,
    ) -> Option<ParseFailure> {
        let options = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        };
        let Ok(document) = roxmltree::Document::parse_with_options(xml, options) else {
            return Some(ParseFailure::Xml);
        };
        for node in document.descendants().filter(|node| is_item(xml, node)) {
            let mut values = [0i32; 4];
            for (value, name) in values.iter_mut().zip(["id", "minAmt", "maxAmt", "weight"]) {
                // getAttribute gives "" for a missing attribute.
                match parse_i32(node.attribute(name).unwrap_or("")) {
                    Some(parsed) => *value = parsed,
                    None => return Some(ParseFailure::Number),
                }
            }
            let [id, minimum_amount, maximum_amount, weight] = values;
            match self.chance(kind) {
                Some(table) => table.push(WeightedChanceItem {
                    id,
                    minimum_amount,
                    maximum_amount,
                    weight,
                }),
                None => {
                    let item = WeightedItem::new(
                        id,
                        minimum_amount,
                        maximum_amount,
                        f64::from(weight),
                        false,
                    );
                    self.rare.add(random, item);
                }
            }
        }
        None
    }
    /// `startup()`. `path` is the configured path and `file` the text found
    /// there, `None` when no file exists. A null path is not checked for
    /// existence: the original parses it, fails and still reports success.
    pub fn startup(
        &mut self,
        kind: SharedKind,
        path: Option<&str>,
        file: Option<&str>,
        random: &mut dyn Source,
        host: &mut dyn Host,
    ) -> Option<ParseFailure> {
        let failure = match (path, file) {
            (Some(path), None) => {
                host.log(
                    kind.class_name(),
                    LogLevel::Err,
                    &format!("Can't locate {} file at {path}", kind.abbreviation()),
                );
                return None;
            }
            (Some(_), Some(text)) => self.parse(kind, text, random, host),
            (None, _) => {
                host.stack_trace(kind.class_name(), ParseFailure::NullPath);
                Some(ParseFailure::NullPath)
            }
        };
        host.log(
            kind.class_name(),
            LogLevel::Fine,
            &format!("{}{}", kind.loaded_message(), path.unwrap_or("null")),
        );
        failure
    }
}

/// `getElementsByTagName("item")` on the original's parser, which is not
/// namespace aware: the tag as written must be exactly `item`.
fn is_item(xml: &str, node: &roxmltree::Node<'_, '_>) -> bool {
    node.is_element()
        && xml[node.range()].strip_prefix("<item").is_some_and(|rest| {
            rest.starts_with(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
        })
}

/// `RareDropTable.retrieve(receiver)`: the first item of the rare table's own
/// roll, or nothing.
pub fn retrieve_rare(
    ctx: &mut Context<'_>,
    receiver: Looter<'_>,
) -> Result<Option<RolledItem>, RollError> {
    Ok(roll_rare(ctx, receiver)?.first().copied())
}

/// The rare table's `roll(receiver)` override.
///
/// The total weight keeps the "nothing" entries unless the receiver is a
/// player wearing a ring of wealth, but the walk over the shuffled copy always
/// passes them by: a draw that lands beyond the other entries drops nothing.
/// A table with a single entry gives it without drawing.
pub fn roll_rare(
    ctx: &mut Context<'_>,
    mut receiver: Looter<'_>,
) -> Result<Vec<RolledItem>, RollError> {
    if ctx.rare_depth >= MAX_RARE_DEPTH {
        return Err(RollError::RareDepth);
    }
    let table = &ctx.shared.rare;
    let mut items = table.guaranteed_items.clone();
    let mut effective_weight = table.total_weight;
    if let Looter::Player(player) = &mut receiver
        && player.removes_nothings()
    {
        let nothing_weight = table
            .items
            .iter()
            .filter(|item| item.id == NOTHING)
            .fold(0.0, |sum, item| sum + item.weight);
        effective_weight -= nothing_weight;
    }
    if table.items.len() == 1 {
        items.push(table.items[0]);
    } else if !table.items.is_empty() {
        let mut rng_weight =
            random_double(ctx.random, effective_weight).map_err(|_| RollError::BadWeight)?;
        let mut shuffled = table.items.clone();
        random::shuffle(ctx.random, &mut shuffled);
        for item in shuffled {
            if item.id == NOTHING {
                continue;
            }
            rng_weight -= item.weight;
            if rng_weight <= 0.0 {
                items.push(item);
                break;
            }
        }
    }
    ctx.rare_depth += 1;
    let converted = convert_weighted_items(ctx, &items, receiver);
    ctx.rare_depth -= 1;
    converted
}
