//! `ShopParser.load`: the pinned body only counts the rows of `shops.json`
//! (its shop construction is commented out in the original). The shops the
//! world uses are built later by the `core.game.shops.Shops` start-up listener,
//! which is content and is not ported here.
use super::{Log, Result, array, parse, read_config};
use crate::world::drop_tables::LogLevel;

/// `load()` with `CONFIG_PATH`.
pub fn load_config(prefix: Option<&str>, log: &mut dyn Log) -> Result<i32> {
    let text = read_config(prefix, "shops.json")?;
    load(&text, log)
}

/// `load()` over the file's text: the number of array elements, whatever they are.
pub fn load(source: &str, log: &mut dyn Log) -> Result<i32> {
    let data = parse(source)?;
    let configlist = array(Some(&data))?;
    let mut count = 0i32;
    for _ in configlist {
        count = count.wrapping_add(1);
    }
    log.log(
        "ShopParser",
        LogLevel::Fine,
        &format!("Parsed {count} shops."),
    );
    Ok(count)
}
