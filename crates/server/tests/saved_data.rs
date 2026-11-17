use rs09_game_core::java_double;
use rs09_server::player::saved_data::{Error, ItemData, QuestName, Result, SavedData};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/saved-data/oracle.jsonl");
fn cases(kind: &str) -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["kind"] == kind)
        .collect()
}
fn bits(value: f64) -> String {
    format!(
        "{:016x}",
        if value.is_nan() {
            f64::NAN.to_bits()
        } else {
            value.to_bits()
        }
    )
}
fn snapshot(data: &SavedData) -> Value {
    let mut state = serde_json::to_value(data).unwrap();
    state["global"]["assistExperience"] = json!(
        data.global
            .assist_experience
            .iter()
            .map(|&value| bits(value))
            .collect::<Vec<_>>()
    );
    state
}
fn attempt(result: Result<Value>) -> Value {
    match result {
        Ok(value) => json!({"ok":value}),
        Err(error) => {
            json!({"error":match error{Error::Null=>"null",Error::Type=>"type",Error::Number=>"number",Error::Index=>"index"}})
        }
    }
}
fn call(data: &mut SavedData, action: &Value, now: i64) -> Result<Value> {
    let args = action["args"].as_array().unwrap();
    let int = |i: usize| args[i].as_i64().unwrap() as i32;
    let long = |i: usize| args[i].as_i64().unwrap();
    let name = || {
        if action["literal"] == true {
            QuestName::Literal(args[0].as_str().unwrap())
        } else {
            QuestName::Text(args[0].as_str().unwrap())
        }
    };
    let activity = &mut data.activity;
    let quest = &mut data.quest;
    let global = &mut data.global;
    match action["method"].as_str().unwrap() {
        "setPestPoints" => activity.pest_points = int(0),
        "increasePestPoints" => activity.increase_pest_points(int(0)),
        "decreasePestPoints" => activity.decrease_pest_points(int(0)),
        "setBarrowKills" => activity.set_barrow_kills(int(0)),
        "setKolodionStage" => activity.kolodion_stage = int(0),
        "setLastBorkBattle" => activity.last_bork_battle = long(0),
        "incrementPizazz" => {
            activity.increment_pizazz(int(0), if args.len() == 1 { 1 } else { int(1) })?
        }
        "decrementPizazz" => {
            activity.decrement_pizazz(int(0), if args.len() == 1 { 1 } else { int(1) })?
        }
        "updateWarriorTokens" => activity.update_warrior_tokens(int(0)),
        "updateBountyHunterRate" => activity.update_bounty_hunter_rate(int(0)),
        "updateBountyRogueRate" => activity.update_bounty_rogue_rate(int(0)),
        "setDragonSlayerAttribute" => {
            quest.set_dragon_slayer_attribute(name(), args[1].as_bool().unwrap())
        }
        "getDragonSlayerAttribute" => return Ok(json!(quest.dragon_slayer_attribute(name()))),
        "getDragonSlayerItem" => return Ok(json!(quest.dragon_slayer_item(name()))),
        "setCooksAssistant" => quest.set_cooks_assistant(name(), args[1].as_bool().unwrap()),
        "getCookAssist" => return Ok(json!(quest.cook_assist(name()))),
        "getDesertTreasureItem" => return Ok(json!(quest.desert_treasure_item(int(0))?)),
        "setTravelLog" => {
            *global
                .travel_logs
                .get_mut(int(0) as usize)
                .ok_or(Error::Index)? = true
        }
        "hasTravelLog" => {
            return Ok(json!(
                global
                    .travel_logs
                    .get(int(0) as usize)
                    .ok_or(Error::Index)?
            ));
        }
        "removeTravelLog" => {
            *global
                .travel_logs
                .get_mut(int(0) as usize)
                .ok_or(Error::Index)? = false
        }
        "hasStrongholdReward" => return Ok(json!(global.has_stronghold_reward(int(0))?)),
        "setAbyssCharge" => {
            *global
                .abyss_data
                .get_mut(int(0) as usize)
                .ok_or(Error::Index)? = true
        }
        "hasAbyssCharge" => {
            return Ok(json!(
                global.abyss_data.get(int(0) as usize).ok_or(Error::Index)?
            ));
        }
        "getRcDecay" => return Ok(json!(global.rc_decay(int(0))?)),
        "resetAbyss" => global.reset_abyss(),
        "setDoubleExp" => global.double_exp = long(0),
        "hasDoubleExp" => return Ok(json!(global.has_double_exp(now))),
        "setGodBook" => {
            *global
                .god_books
                .get_mut(int(0) as usize)
                .ok_or(Error::Index)? = true
        }
        "hasCompletedGodBook" => {
            return Ok(json!(
                global.god_books.get(int(0) as usize).ok_or(Error::Index)?
            ));
        }
        "setSavedLocation" => global.set_saved_location(int(0), int(1), int(2)),
        "hasReadPlaques" => return Ok(json!(global.has_read_plaques())),
        method => panic!("unhandled original method {method}"),
    }
    Ok(Value::Null)
}
fn evaluate(case: &Value) -> Value {
    let mut data = SavedData::default();
    let mut now = 0;
    json!(case["actions"].as_array().unwrap().iter().map(|action|{
    let mut root=serde_json::Map::new();let result=match action["op"].as_str().unwrap(){
        "parse"=>match action["target"].as_str().unwrap(){"activity"=>data.activity.parse(&action["value"]),"quest"=>data.quest.parse(&action["value"]),"global"=>data.global.parse(&action["value"]),_=>panic!()}.map(|()|Value::Null),
        "sections"=>data.parse_sections(&action["value"]).map(|()|Value::Null),"save"=>data.save_sections(&mut root).map(|()|Value::Null),"call"=>call(&mut data,action,now),
        "item"=>data.quest.set_desert_treasure_item(action["index"].as_i64().unwrap()as i32,action["id"].as_i64().map(|id|ItemData::new(id as i32,action["amount"].as_i64().unwrap()as i32))).map(|()|Value::Null),"clock"=>{now=action["value"].as_i64().unwrap();Ok(Value::Null)},_=>panic!()};
    json!({"result":attempt(result),"state":snapshot(&data),"root":root,"flags":[data.activity.has_started_kolodion(),data.activity.has_killed_kolodion(),data.activity.has_received_kolodion_reward(),data.activity.has_killed_bork(),data.global.has_read_plaques()]})
}).collect::<Vec<_>>())
}
#[test]
fn save_records_and_partial_load_save_mutations_match_original() {
    for (index, case) in cases("saved").iter().enumerate() {
        assert_eq!(
            evaluate(case),
            case["expected"],
            "saved case {index}, actions {}",
            case["actions"]
        );
    }
}
#[test]
fn legacy_jdk_double_strings_match_exponent_boundaries_and_random_bit_patterns() {
    for (index, case) in cases("double_format").iter().enumerate() {
        let value =
            f64::from_bits(u64::from_str_radix(case["bits"].as_str().unwrap(), 16).unwrap());
        assert_eq!(
            json!(java_double::format(value)),
            case["expected"],
            "double case {index}, bits {}",
            case["bits"]
        );
    }
}
#[test]
fn saved_double_text_acceptance_and_rounding_match_jdk11() {
    for (index, case) in cases("double_parse").iter().enumerate() {
        let actual = match java_double::parse(case["text"].as_str().unwrap()) {
            Some(value) => json!({"ok":bits(value)}),
            None => json!({"error":"number"}),
        };
        assert_eq!(
            actual, case["expected"],
            "double parse case {index}, {}",
            case["text"]
        );
    }
}
#[test]
fn saved_data_fixtures_pin_original_records_selected_save_methods_and_adapters() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../../fixtures/saved-data/provenance.json")).unwrap();
    assert_eq!(
        metadata["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS.as_bytes()))
    );
    assert_eq!(
        metadata["caseCount"].as_u64().unwrap() as usize,
        CORPUS.lines().count()
    );
    assert_eq!(
        metadata["upstream"],
        serde_json::from_str::<Value>(include_str!("../../../upstream.lock.json")).unwrap()
    );
    assert_eq!(
        ["saved", "double_format", "double_parse"]
            .iter()
            .map(|kind| cases(kind).len())
            .sum::<usize>(),
        CORPUS.lines().count()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, expected) in metadata["sources"].as_object().unwrap() {
        if path.starts_with("oracle/") {
            let data = std::fs::read(root.join(path))
                .unwrap()
                .into_iter()
                .filter(|&byte| byte != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(*expected, format!("{:x}", Sha256::digest(data)), "{path}");
        }
    }
}
