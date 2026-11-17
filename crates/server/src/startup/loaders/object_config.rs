//! `ObjectConfigParser`: `object_configs.json` onto scenery definitions (the
//! `examine` handler and `animationId`).
use super::{Log, Result, array, int, object, parse, read_config, text};
use crate::definitions::{property::Property, scenery::Registry};
use crate::world::drop_tables::LogLevel;
use rs09_game_core::java_json::{Json, key_order};

/// `load()` with `CONFIG_PATH`.
pub fn load_config(
    prefix: Option<&str>,
    definitions: &mut Registry,
    log: &mut dyn Log,
) -> Result<i32> {
    let text = read_config(prefix, "object_configs.json")?;
    load(&text, definitions, log)
}

/// `load()` over the file's text. Returns the count it logs (one per id).
///
/// `SceneryDefinition.forId` stores a new definition for an id the cache does
/// not have, so every listed id ends up in the registry. A row's fields are
/// visited in `JSONObject` (`HashMap`) order, so a bad `render_anim` can fail
/// before or after that row's `examine` was stored.
pub fn load(source: &str, definitions: &mut Registry, log: &mut dyn Log) -> Result<i32> {
    let mut count = 0i32;
    let data = parse(source)?;
    let configlist = array(Some(&data))?;
    for config in configlist {
        let e = object(Some(config))?;
        let ids = text(e, "ids")
            .split(',')
            .map(int)
            .collect::<Result<Vec<i32>>>()?;
        let Json::Object(entries) = e else {
            unreachable!("object() checked the type")
        };
        let order = key_order(entries);
        for id in ids {
            let def = definitions.for_id(id);
            for &index in &order {
                let (key, value) = &entries[index];
                let value = value.text();
                if value.is_empty() || value == "null" {
                    continue;
                }
                match key.as_str() {
                    "examine" => {
                        let handlers = def.read().handlers.clone();
                        handlers.write().insert(key.clone(), Property::Text(value));
                    }
                    "render_anim" => {
                        let animation = int(&value)?;
                        def.write().animation_id = animation;
                    }
                    _ => {}
                }
            }
            count = count.wrapping_add(1);
        }
    }
    log.log(
        "ObjectConfigParser",
        LogLevel::Fine,
        &format!("Parsed {count} object configs."),
    );
    Ok(count)
}
