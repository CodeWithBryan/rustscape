//! `ObjectOverrideParser`: `object_overrides.json` into the `addSceneries` /
//! `removeSceneries` lists of the regions it names.
use super::{
    Log, Result, array, int, kotlin_trim, list_get, object, parse, read_config, remove_surrounding,
    text,
};
use crate::definitions::scenery::Registry;
use crate::startup::regions::{Regions, SceneryOverride};
use crate::world::drop_tables::LogLevel;
use rs09_game_core::java_json::Json;

/// `Direction.X.ordinal` for the parser's direction names. The rotation the
/// original stores is this ordinal, not the direction's `value`.
pub fn rotation(direction: &str) -> i32 {
    match direction {
        "ne" => 2, // NORTH_EAST
        "nw" => 0, // NORTH_WEST
        "w" => 3,  // WEST
        "e" => 4,  // EAST
        "sw" => 5, // SOUTH_WEST
        "se" => 7, // SOUTH_EAST
        "s" => 6,  // SOUTH
        _ => 1,    // NORTH
    }
}

/// `Location.fromString`: `removeSurrounding("[", "]")`, split on commas,
/// `trim().toInt()` each part (all parts are parsed), then the first three.
pub fn location_from_string(source: &str) -> Result<[i32; 3]> {
    let parts = remove_surrounding(source, "[", "]")
        .split(',')
        .map(|part| int(kotlin_trim(part)))
        .collect::<Result<Vec<i32>>>()?;
    Ok([
        *list_get(&parts, 0)?,
        *list_get(&parts, 1)?,
        *list_get(&parts, 2)?,
    ])
}

/// `load()` with `CONFIG_PATH`.
pub fn load_config(
    prefix: Option<&str>,
    definitions: &mut Registry,
    regions: &mut Regions,
    log: &mut dyn Log,
) -> Result<i32> {
    let text = read_config(prefix, "object_overrides.json")?;
    load(&text, definitions, regions, log)
}

/// `load()` over the file's text. Returns the count it logs: every row,
/// including those with an unknown mode and duplicates that were not added.
pub fn load(
    source: &str,
    definitions: &mut Registry,
    regions: &mut Regions,
    log: &mut dyn Log,
) -> Result<i32> {
    let mut count = 0i32;
    let data = parse(source)?;
    let overridelist = array(Some(&data))?;
    for entry in overridelist {
        let e = object(Some(entry))?;
        let mode = text(e, "mode");
        let id = int(&text(e, "id"))?;
        let [x, y, z] = location_from_string(&text(e, "location"))?;
        // `e["type"]?.toString()?.toInt() ?: 10`: absent or JSON null is 10.
        let kind = match e.get("type") {
            None | Some(Json::Null) => 10,
            Some(value) => int(&value.text())?,
        };
        let direction = match e.get("direction") {
            None | Some(Json::Null) => "n".to_string(),
            Some(value) => value.text(),
        };
        let rotation = rotation(&direction);
        // The Scenery constructor looks its definition up (creating it if the
        // cache has none) for the node's name.
        definitions.for_id(id);
        let obj = SceneryOverride::new(id, x, y, z, kind, rotation);
        let region = regions.for_id(obj.region_id());
        match mode.as_str() {
            "remove" => {
                if !region.remove_sceneries.contains(&obj) {
                    region.remove_sceneries.push(obj);
                }
            }
            "add" => {
                if !region.add_sceneries.contains(&obj) {
                    region.add_sceneries.push(obj);
                }
            }
            _ => log.log(
                "ObjectOverrideParser",
                LogLevel::Err,
                &format!("Ignored unknown ObjectOverride mode {mode}!"),
            ),
        }
        count = count.wrapping_add(1);
    }
    log.log(
        "ObjectOverrideParser",
        LogLevel::Fine,
        &format!("Parsed {count} object overrides."),
    );
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::startup::loaders::LoadError;

    #[test]
    fn locations_need_both_brackets_to_lose_them() {
        assert_eq!(location_from_string("[1, 2,3]").unwrap(), [1, 2, 3]);
        assert!(matches!(
            location_from_string("[1,2,3"),
            Err(LoadError::Number)
        ));
        assert!(matches!(
            location_from_string("1,2"),
            Err(LoadError::ListIndex)
        ));
    }
}
