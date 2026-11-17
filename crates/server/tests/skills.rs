use rs09_game_core::java_double;
use rs09_server::player::skills::{Context, Effect, EntityKind, Error, Result, Skills};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
const CORPUS: &str = include_str!("../../../fixtures/skills/oracle.jsonl");
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
fn number(value: &Value) -> f64 {
    java_double::parse(value.as_str().unwrap()).unwrap()
}
fn snapshot(s: &Skills) -> Value {
    json!({"experienceMultiplier":bits(s.experience_multiplier),"experience":s.experience.map(bits),"staticLevels":s.static_levels,"dynamicLevels":s.dynamic_levels,"prayerPoints":bits(s.prayer_points),"lifepoints":s.lifepoints,"lifepointsIncrease":s.lifepoints_increase,"experienceGained":bits(s.experience_gained),"lifepointsUpdate":s.lifepoints_update,"combatMilestone":s.combat_milestone,"skillMilestone":s.skill_milestone,"lastTrainedSkill":s.last_trained_skill,"lastXpGain":s.last_xp_gain,"lastUpdateXp":s.last_update_xp.map(|xp|xp.map(bits)),"lastUpdate":s.last_update})
}
struct Env {
    config: Value,
    kind: EntityKind,
    tick: i32,
    combat: i32,
    summon: i32,
    trace: Vec<Value>,
    equipment: BTreeSet<i32>,
    failure: String,
}
impl Env {
    fn new(config: &Value) -> Self {
        Self {
            config: config.clone(),
            kind: match config["entity"].as_str() {
                Some("npc") => EntityKind::Npc,
                Some("other") => EntityKind::Other,
                _ => EntityKind::Player,
            },
            tick: config["tick"].as_i64().unwrap_or(0) as i32,
            combat: 0,
            summon: 0,
            trace: Vec::new(),
            equipment: config["equipment"]
                .as_array()
                .map(|ids| ids.iter().map(|id| id.as_i64().unwrap() as i32).collect())
                .unwrap_or_default(),
            failure: String::new(),
        }
    }
    fn flag(&self, key: &str) -> bool {
        self.config[key] == true
    }
    fn call(&mut self, tag: &str, data: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(tag)];
        row.extend(data);
        self.trace.push(json!(row));
        if self.failure == tag {
            Err(Error::Binding)
        } else {
            Ok(())
        }
    }
    fn mutate(&mut self, state: &mut Skills, stage: &str) {
        let mutation = &self.config["mutations"][stage];
        if let Some(slot) = mutation["skill"].as_u64() {
            state.experience[slot as usize] = number(&mutation["xp"]);
        }
        if let Some(tick) = mutation["tick"].as_i64() {
            self.tick = tick as i32;
        }
    }
}
impl Context for Env {
    fn kind(&self) -> EntityKind {
        self.kind
    }
    fn version(&self) -> i32 {
        if self.kind == EntityKind::Player {
            self.config["version"].as_i64().unwrap_or(6) as i32
        } else {
            6
        }
    }
    fn tick(&mut self) -> i32 {
        self.call("tick", vec![json!(self.tick)]).unwrap();
        self.tick
    }
    fn world_tick(&mut self) -> i32 {
        self.call("api_tick", vec![json!(self.tick)]).unwrap();
        self.tick
    }
    fn combat_level(&self) -> i32 {
        self.combat
    }
    fn summoning_combat_level(&self) -> i32 {
        self.summon
    }
    fn npc_combat_level(&mut self) -> Result<i32> {
        self.call("npc_combat", vec![])?;
        Ok(self.config["npcCombat"].as_i64().unwrap_or(42) as i32)
    }
    fn damage_spong_active(&mut self) -> Result<bool> {
        self.call("perk", vec![])?;
        Ok(self.flag("perk"))
    }
    fn assisted_level(&mut self, slot: i32) -> Result<Option<i32>> {
        if self.flag("assist")
            && !self.flag("assistOwn")
            && !self.flag("restricted")
            && self.flag("assistEnabled")
            && [20, 12, 9, 22, 19, 6, 13, 7, 15].contains(&slot)
        {
            Ok(Some(self.config["assistLevel"].as_i64().unwrap_or(1) as i32))
        } else {
            Ok(None)
        }
    }
    fn translate_experience(
        &mut self,
        _state: &mut Skills,
        slot: i32,
        amount: f64,
        multiplier: f64,
    ) -> Result<bool> {
        if self.flag("assist") {
            self.call(
                "assist_translate",
                vec![json!(slot), json!(bits(amount)), json!(bits(multiplier))],
            )?;
            Ok(self.flag("translate"))
        } else {
            Ok(false)
        }
    }
    fn brawling_present(&mut self) -> Result<bool> {
        self.call("brawling", vec![])?;
        if self.kind != EntityKind::Player {
            return Err(Error::Null);
        }
        Ok(self.flag("gloves"))
    }
    fn brawling_for_skill(&mut self, skill: i32) -> Result<Option<i32>> {
        Ok([0, 4, 6, 5, 16, 8, 11, 14, 21, 17, 13, 10, 7]
            .iter()
            .position(|&slot| slot == skill)
            .map(|index| 13845 + index as i32))
    }
    fn equipment_contains(&mut self, id: i32) -> Result<bool> {
        self.call("equipment", vec![json!(id)])?;
        Ok(self.equipment.contains(&id))
    }
    fn brawling_bonus(&mut self) -> Result<f64> {
        self.call("bonus", vec![])?;
        Ok(self.config["bonus"].as_f64().unwrap_or(0.5))
    }
    fn tutorial_complete(&mut self) -> Result<bool> {
        self.call("tutorial", vec![])?;
        Ok(self.flag("tutorial"))
    }
    fn artificial(&mut self) -> Result<bool> {
        self.call("artificial", vec![])?;
        Ok(self.flag("artificial"))
    }
    fn username(&mut self) -> Result<String> {
        self.call("username", vec![])?;
        Ok("player".into())
    }
    fn permadeath_present(&self) -> bool {
        self.config
            .as_object()
            .is_some_and(|config| config.contains_key("permadeath"))
    }
    fn effect(&mut self, s: &mut Skills, effect: Effect) -> Result<()> {
        match effect {
            Effect::Combat(level) => {
                self.call("combat", vec![json!(level)])?;
                self.combat = level;
            }
            Effect::SummoningCombat(level) => {
                self.call("summon", vec![json!(level)])?;
                self.summon = level;
            }
            Effect::Packet(i) => {
                self.call(
                    "packet",
                    vec![
                        json!(i),
                        json!(s.static_levels[i]),
                        json!(s.dynamic_levels[i]),
                        json!(bits(s.experience[i])),
                        json!(s.lifepoints),
                        json!(bits(s.prayer_points)),
                    ],
                )?;
                self.mutate(s, "packet");
            }
            Effect::DynamicLevel {
                skill,
                previous,
                level,
            } => {
                self.call("dynamic", vec![json!(skill), json!(previous), json!(level)])?;
                self.mutate(s, "dynamic");
            }
            Effect::XpEvent { skill, amount } => {
                self.call("xp_event", vec![json!(skill), json!(bits(amount))])?;
                self.mutate(s, "event");
            }
            Effect::XpPlugins { skill, amount } => {
                self.call("xp_plugins", vec![json!(skill), json!(bits(amount))])?;
                self.mutate(s, "plugin");
            }
            Effect::Appearance => {
                self.call("appearance", vec![])?;
                self.mutate(s, "appearance");
            }
            Effect::LevelUp { skill, amount } => {
                self.call("level_up", vec![json!(skill), json!(amount)])?;
                self.mutate(s, "level");
            }
            Effect::FlashingIcons(slot) => {
                self.call("flash", vec![json!(slot)])?;
            }
            Effect::News(message) => {
                self.call("news", vec![json!(message)])?;
                self.mutate(s, "news");
            }
            Effect::XpLog(values) => {
                self.call(
                    "xp_log",
                    vec![json!(
                        values
                            .into_iter()
                            .map(|(i, value)| json!([i, bits(value)]))
                            .collect::<Vec<_>>()
                    )],
                )?;
                self.mutate(s, "log");
            }
            Effect::ConsumeGloves { id, charges } => {
                self.call("consume", vec![json!(id), json!(charges)])?;
                if self.flag("removeGlove") {
                    self.equipment.remove(&id);
                }
                self.mutate(s, "consume");
            }
        }
        Ok(())
    }
}
fn attempt(result: Result<Value>) -> Value {
    match result {
        Ok(value) => json!({"ok":value}),
        Err(error) => {
            json!({"error":match error{Error::Index=>"index",Error::Number=>"number",Error::Type=>"type",Error::Null=>"null",Error::Binding=>"binding"}})
        }
    }
}
fn evaluate(case: &Value) -> Value {
    let mut env = Env::new(&case["config"]);
    let mut s = Skills::new(&mut env).unwrap();
    let mut output = Vec::new();
    for action in case["actions"].as_array().unwrap() {
        env.trace.clear();
        let mut root = serde_json::Map::new();
        let slot = action["skill"].as_i64().unwrap_or(0) as i32;
        let result = (|| -> Result<Value> {
            match action["op"].as_str().unwrap() {
                "configure" => {
                    s.update_combat_level(&mut env)?;
                }
                "combat" => return Ok(json!(s.update_combat_level(&mut env)?)),
                "pulse" => s.pulse(),
                "refresh" => s.refresh(&mut env)?,
                "restore" => s.restore(&mut env)?,
                "setLevel" => {
                    s.set_level(slot, action["value"].as_i64().unwrap() as i32, &mut env)?
                }
                "getLevel" => return Ok(json!(s.level(slot, false, &mut env)?)),
                "getLevelDiscard" => return Ok(json!(s.level(slot, true, &mut env)?)),
                "staticLevel" => {
                    s.set_static_level(slot, action["value"].as_i64().unwrap() as i32, &mut env)?
                }
                "life" => s.set_lifepoints(action["value"].as_i64().unwrap() as i32),
                "maxLife" => return Ok(json!(s.maximum_lifepoints(&mut env)?)),
                "heal" => {
                    return Ok(json!(
                        s.heal(action["value"].as_i64().unwrap() as i32, &mut env)?
                    ));
                }
                "hit" => return Ok(json!(s.hit(action["value"].as_i64().unwrap() as i32))),
                "healRaw" => s.heal_no_restrictions(action["value"].as_i64().unwrap() as i32),
                "prayerSet" => s.set_prayer(number(&action["value"]), &mut env)?,
                "prayerInc" => s.increment_prayer(number(&action["value"]), &mut env)?,
                "prayerDec" => s.decrement_prayer(number(&action["value"]), &mut env)?,
                "recharge" => s.recharge_prayer(&mut env)?,
                "update" => {
                    return Ok(json!(s.update_level(
                        slot,
                        action["amount"].as_i64().unwrap() as i32,
                        action["maximum"].as_i64().unwrap() as i32,
                        &mut env
                    )?));
                }
                "updateDefault" => {
                    return Ok(json!(s.update_level_default(
                        slot,
                        action["amount"].as_i64().unwrap() as i32,
                        &mut env
                    )?));
                }
                "drain" => s.drain_level(
                    slot,
                    number(&action["drain"]),
                    number(&action["maximum"]),
                    &mut env,
                )?,
                "xp" => s.add_experience(
                    slot,
                    number(&action["value"]),
                    action["playerMod"] == true,
                    &mut env,
                )?,
                "rawXp" => {
                    s.experience[slot as usize] = number(&action["value"]);
                }
                "tick" => env.tick = action["value"].as_i64().unwrap() as i32,
                "fail" => env.failure = action["value"].as_str().unwrap().into(),
                "save" => {
                    if env.kind != EntityKind::Player {
                        return Err(Error::Type);
                    }
                    s.save_profile(&mut root);
                }
                "parse" => s.parse(&action["value"], &mut env)?,
                "profile" => {
                    if env.kind != EntityKind::Player {
                        return Err(Error::Type);
                    }
                    s.parse_profile(&action["value"], &mut env)?;
                }
                "correct" => s.correct(number(&action["value"]), &mut env)?,
                "copy" => {
                    let mut other_env = Env::new(&env.config);
                    other_env.kind = EntityKind::Other;
                    other_env.tick = env.tick;
                    let mut source = Skills::new(&mut other_env)?;
                    env.trace.extend(other_env.trace);
                    let input = &action["value"];
                    for i in 0..24 {
                        source.experience[i] = number(&input["experience"][i]);
                        source.static_levels[i] = input["staticLevels"][i].as_i64().unwrap() as i32;
                        source.dynamic_levels[i] =
                            input["dynamicLevels"][i].as_i64().unwrap() as i32;
                    }
                    source.prayer_points = number(&input["prayerPoints"]);
                    source.lifepoints = input["lifepoints"].as_i64().unwrap() as i32;
                    source.lifepoints_increase =
                        input["lifepointsIncrease"].as_i64().unwrap() as i32;
                    source.experience_gained = number(&input["experienceGained"]);
                    source.experience_multiplier = number(&input["experienceMultiplier"]);
                    source.combat_milestone = input["combatMilestone"].as_i64().unwrap() as i32;
                    source.skill_milestone = input["skillMilestone"].as_i64().unwrap() as i32;
                    source.last_trained_skill = input["lastTrainedSkill"].as_i64().unwrap() as i32;
                    source.last_xp_gain = input["lastXpGain"].as_i64().unwrap() as i32;
                    source.lifepoints_update = input["lifepointsUpdate"].as_bool().unwrap();
                    s.copy_from(&source);
                }
                "rawLevels" => {
                    for i in 0..24 {
                        s.static_levels[i] = action["static"][i].as_i64().unwrap() as i32;
                        s.dynamic_levels[i] = action["dynamic"][i].as_i64().unwrap() as i32;
                    }
                }
                op => panic!("unhandled action {op}"),
            };
            Ok(Value::Null)
        })();
        output.push(json!({"result":attempt(result),"state":snapshot(&s),"trace":env.trace,"root":root,"combat":env.combat,"summon":env.summon,"tick":env.tick,"equipment":env.equipment,"totals":[s.total_level(),s.total_xp(),s.mastered_skills(),s.highest_combat_skill()]}));
    }
    json!(output)
}
#[test]
fn skill_state_effect_order_and_profile_sections_match_original() {
    for (index, case) in cases("sequence").iter().enumerate() {
        let actual = evaluate(case);
        if actual != case["expected"] {
            let actual = actual.as_array().unwrap();
            let expected = case["expected"].as_array().unwrap();
            let step = actual
                .iter()
                .zip(expected)
                .position(|(a, b)| a != b)
                .unwrap();
            panic!(
                "skill case {index}, step {step}, action {}, actual {}, expected {}",
                case["actions"][step], actual[step], expected[step]
            );
        }
    }
}
#[test]
fn cached_experience_curve_and_name_lookup_match_original() {
    for case in cases("curve") {
        assert_eq!(
            json!(Skills::experience_by_level(
                case["level"].as_i64().unwrap() as i32
            )),
            case["expected"],
            "level {}",
            case["level"]
        );
    }
    for case in cases("xp_level") {
        assert_eq!(
            json!(Skills::level_from_xp(number(&case["value"]))),
            case["expected"]
        );
    }
    for case in cases("name") {
        assert_eq!(
            json!(Skills::skill_by_name(case["name"].as_str())),
            case["expected"],
            "name {}",
            case["name"]
        );
    }
}
#[test]
fn skills_fixtures_pin_complete_original_class_selected_profile_methods_and_boundaries() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../../fixtures/skills/provenance.json")).unwrap();
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
        ["sequence", "curve", "xp_level", "name"]
            .iter()
            .map(|kind| cases(kind).len())
            .sum::<usize>(),
        CORPUS.lines().count()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, expected) in metadata["sources"].as_object().unwrap() {
        if path.starts_with("oracle/") {
            let bytes = std::fs::read(root.join(path))
                .unwrap()
                .into_iter()
                .filter(|&b| b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(*expected, format!("{:x}", Sha256::digest(bytes)), "{path}");
        }
    }
}
