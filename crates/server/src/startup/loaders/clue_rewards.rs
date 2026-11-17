//! `ClueRewardParser`: `clue_rewards.json` into the four clue reward tables.
//!
//! Every weighted entry goes into its `WeightBasedTable` at a random position
//! (`RandomFunction.random(0, size)` on the shared generator), so the stored
//! order depends on the world's generator state when loading starts. A table
//! replaces the old one only once it is complete: a failure keeps the tables
//! assigned before it, and the draws of the table it was building.
use super::{Result, array, double, int, object, parse, read_config, text};
use crate::world::drop_tables::{WeightBasedTable, WeightedItem, random::Source};

/// `ClueRewardParser.easyTable`, `medTable`, `hardTable` and `rareTable`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClueRewards {
    pub easy: WeightBasedTable,
    pub medium: WeightBasedTable,
    pub hard: WeightBasedTable,
    pub rare: WeightBasedTable,
}

/// `load()` with `CONFIG_PATH`.
pub fn load_config(
    prefix: Option<&str>,
    tables: &mut ClueRewards,
    random: &mut dyn Source,
) -> Result<()> {
    let text = read_config(prefix, "clue_rewards.json")?;
    load(&text, tables, random)
}

/// `load()` over the file's text. The original logs nothing.
pub fn load(source: &str, tables: &mut ClueRewards, random: &mut dyn Source) -> Result<()> {
    let data = parse(source)?;
    let raw = object(Some(&data))?;
    tables.easy = parse_clue_table(array(raw.get("easy"))?, random)?;
    tables.medium = parse_clue_table(array(raw.get("medium"))?, random)?;
    tables.hard = parse_clue_table(array(raw.get("hard"))?, random)?;
    tables.rare = parse_clue_table(array(raw.get("rare"))?, random)?;
    Ok(())
}

fn parse_clue_table(
    data: &[rs09_game_core::java_json::Json],
    random: &mut dyn Source,
) -> Result<WeightBasedTable> {
    let mut table = WeightBasedTable::new();
    for element in data {
        let item = object(Some(element))?;
        let id = int(&text(item, "id"))?;
        let min = int(&text(item, "minAmount"))?;
        let max = int(&text(item, "maxAmount"))?;
        let weight = double(&text(item, "weight"))?;
        table.add(random, WeightedItem::new(id, min, max, weight, false));
    }
    Ok(table)
}
