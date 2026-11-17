//! The order in which PlayerSaver.kt puts each key. A JSONObject is a HashMap:
//! keys sharing a bucket keep insertion order, so that order is part of the
//! written bytes. The record libraries build sorted serde maps, which lose it.
use rs09_game_core::java_json::Json;
use serde_json::Value;

pub const SKILL_ROW: &[&str] = &[
    "id",
    "static",
    "dynamic",
    "lifepoints",
    "prayerPoints",
    "experience",
];
pub const MILESTONE: &[&str] = &["combatMilestone", "skillMilestone"];
pub const ATTRIBUTE: &[&str] = &["type", "key", "value", "expirable", "expiration-time"];
pub const ITEM: &[&str] = &["id", "amount"];
pub const ACTIVITY: &[&str] = &[
    "pestPoints",
    "warriorGuildTokens",
    "bountyHunterRate",
    "bountyRogueRate",
    "barrowKills",
    "barrowBrothers",
    "barrowTunnelIndex",
    "kolodionStage",
    "godCasts",
    "kolodionBoss",
    "elnockSupplies",
    "lastBorkBattle",
    "startedMta",
    "lostCannon",
    "pizazzPoints",
    "bonesToPeaches",
    "solvedMazes",
    "fogRating",
    "borkKills",
    "hardcoreDeath",
    "topGrabbed",
];
pub const QUEST: &[&str] = &[
    "draynorLever",
    "dragonSlayer",
    "dragonSlayerPlanks",
    "demonSlayer",
    "cooksAssistant",
    "gardenerAttack",
    "talkedDrezel",
    "desertTreasureNode",
    "witchsExperimentStage",
    "witchsExperimentKilled",
];
pub const GLOBAL: &[&str] = &[
    "homeTeleportDelay",
    "lumbridgeRope",
    "apprentice",
    "assistTime",
    "assistExperience",
    "strongHoldRewards",
    "chatPing",
    "tutorClaim",
    "luthasTask",
    "karamjaBananas",
    "silkSteal",
    "zafAmount",
    "zafTime",
    "fritzGlass",
    "wydinEmployee",
    "geTutorial",
    "essenceTeleporter",
    "recoilDamage",
    "doubleExpDelay",
    "joinedMonastery",
    "readPlaques",
    "forgingUses",
    "ectoCharges",
    "dropDelay",
    "abyssData",
    "rcDecays",
    "disableDeathScreen",
    "playerTestStage",
    "charmingDelay",
    "travelLogs",
    "godBooks",
    "disableNews",
    "godPages",
    "overChargeDelay",
    "bossCounters",
    "barrowsLoots",
    "lootSharePoints",
    "lootShareDelay",
    "doubleExp",
    "globalTeleporterDelay",
    "starSpriteDelay",
    "runReplenishDelay",
    "runReplenishCharges",
    "lowAlchemyCharges",
    "lowAlchemyDelay",
    "magicSkillCapeDelay",
    "hunterCapeDelay",
    "hunterCapeCharges",
    "taskAmount",
    "taskPoints",
    "macroDisabled",
];

/// A value without nested objects whose key order matters.
pub fn plain(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Bool(value) => Json::Bool(*value),
        Value::Number(number) => match number.as_i64() {
            Some(value) => Json::Int(value),
            None => Json::Double(number.as_f64().unwrap_or(f64::NAN)),
        },
        Value::String(value) => Json::Text(value.clone()),
        Value::Array(values) => Json::Array(values.iter().map(plain).collect()),
        Value::Object(_) => object(value, &[], &plain_child),
    }
}
fn plain_child(_: &str, value: &Value) -> Json {
    plain(value)
}
/// Rebuilds an object with the original put order. Keys outside the table
/// follow in sorted order; the save sections never produce any.
pub fn object(value: &Value, order: &[&str], child: &dyn Fn(&str, &Value) -> Json) -> Json {
    let Value::Object(map) = value else {
        return plain(value);
    };
    let mut out = Json::object();
    for key in order {
        if let Some(value) = map.get(*key) {
            out.put(*key, child(key, value)).unwrap();
        }
    }
    for (key, value) in map {
        if !order.contains(&key.as_str()) {
            out.put(key.clone(), child(key, value)).unwrap();
        }
    }
    out
}
pub fn rows(value: &Value, order: &[&str]) -> Json {
    match value {
        Value::Array(values) => Json::Array(
            values
                .iter()
                .map(|value| object(value, order, &plain_child))
                .collect(),
        ),
        other => plain(other),
    }
}
/// The parser side: what the serde-based record libraries read. JSON-simple
/// can hold nonfinite doubles, serde_json cannot; they stay as their Java text,
/// which is what every consumer of these values converts them to first.
pub fn value(json: &Json) -> Value {
    match json {
        Json::Null => Value::Null,
        Json::Bool(value) => Value::Bool(*value),
        Json::Int(value) => (*value).into(),
        Json::Double(value) => serde_json::Number::from_f64(*value).map_or_else(
            || Value::String(rs09_game_core::java_double::format(*value)),
            Value::Number,
        ),
        Json::Text(value) => Value::String(value.clone()),
        Json::Array(values) => Value::Array(values.iter().map(value_ref).collect()),
        Json::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|(key, entry)| (key.clone(), value(entry)))
                .collect(),
        ),
    }
}
fn value_ref(json: &Json) -> Value {
    value(json)
}
