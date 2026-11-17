// Authored from pinned ActivityData, QuestData and GlobalData; see tools/port_saved_data.py.
use super::{Error, ItemData, Result, json};
use serde::Serialize;
use serde_json::{Map, Value};
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityData {
    pub pest_points: i32,
    pub warrior_guild_tokens: i32,
    pub bounty_hunter_rate: i32,
    pub bounty_rogue_rate: i32,
    pub barrow_brothers: Vec<bool>,
    pub barrow_kills: i32,
    pub barrow_tunnel_index: i32,
    pub kolodion_stage: i32,
    pub god_casts: Vec<i32>,
    pub kolodion_boss: i32,
    pub elnock_supplies: bool,
    pub last_bork_battle: i64,
    pub bork_kills: i8,
    pub lost_cannon: bool,
    pub started_mta: bool,
    pub pizazz_points: Vec<i32>,
    pub bones_to_peaches: bool,
    pub solved_mazes: i32,
    pub fog_rating: i32,
    pub hardcore_death: bool,
    pub top_grabbed: bool,
}
impl Default for ActivityData {
    fn default() -> Self {
        Self {
            pest_points: 0,
            warrior_guild_tokens: 0,
            bounty_hunter_rate: 0,
            bounty_rogue_rate: 0,
            barrow_brothers: vec![false; 6],
            barrow_kills: 0,
            barrow_tunnel_index: 0,
            kolodion_stage: 0,
            god_casts: vec![0; 3],
            kolodion_boss: 0,
            elnock_supplies: false,
            last_bork_battle: 0,
            bork_kills: 0,
            lost_cannon: false,
            started_mta: false,
            pizazz_points: vec![0; 4],
            bones_to_peaches: false,
            solved_mazes: 0,
            fog_rating: 0,
            hardcore_death: false,
            top_grabbed: false,
        }
    }
}
impl ActivityData {
    pub fn parse(&mut self, data: &Value) -> Result<()> {
        let data = json::object(data)?;
        self.pest_points = json::int(json::required(data, "pestPoints")?)?;
        self.warrior_guild_tokens = json::int(json::required(data, "warriorGuildTokens")?)?;
        self.bounty_hunter_rate = json::int(json::required(data, "bountyHunterRate")?)?;
        self.bounty_rogue_rate = json::int(json::required(data, "bountyRogueRate")?)?;
        self.barrow_kills = json::int(json::required(data, "barrowKills")?)?;
        json::array(
            data,
            "barrowBrothers",
            &mut self.barrow_brothers,
            json::boolean,
        )?;
        self.barrow_tunnel_index = json::int(json::required(data, "barrowTunnelIndex")?)?;
        self.kolodion_stage = json::int(json::required(data, "kolodionStage")?)?;
        json::array(data, "godCasts", &mut self.god_casts, json::int)?;
        self.kolodion_boss = json::int(json::required(data, "kolodionBoss")?)?;
        self.elnock_supplies = json::boolean(json::required(data, "elnockSupplies")?)?;
        self.last_bork_battle = json::long(json::required(data, "lastBorkBattle")?)?;
        self.started_mta = json::boolean(json::required(data, "startedMta")?)?;
        self.lost_cannon = json::boolean(json::required(data, "lostCannon")?)?;
        json::array(data, "pizazzPoints", &mut self.pizazz_points, json::int)?;
        self.bones_to_peaches = json::boolean(json::required(data, "bonesToPeaches")?)?;
        self.solved_mazes = json::int(json::required(data, "solvedMazes")?)?;
        self.fog_rating = json::int(json::required(data, "fogRating")?)?;
        self.bork_kills = json::byte(json::required(data, "borkKills")?)?;
        self.hardcore_death = json::boolean(json::required(data, "hardcoreDeath")?)?;
        self.top_grabbed = json::boolean(json::required(data, "topGrabbed")?)?;
        Ok(())
    }
    pub fn save(&self) -> Result<Value> {
        let mut data = Map::new();
        data.insert(
            "pestPoints".into(),
            Value::String(self.pest_points.to_string()),
        );
        data.insert(
            "warriorGuildTokens".into(),
            Value::String(self.warrior_guild_tokens.to_string()),
        );
        data.insert(
            "bountyHunterRate".into(),
            Value::String(self.bounty_hunter_rate.to_string()),
        );
        data.insert(
            "bountyRogueRate".into(),
            Value::String(self.bounty_rogue_rate.to_string()),
        );
        data.insert(
            "barrowKills".into(),
            Value::String(self.barrow_kills.to_string()),
        );
        data.insert(
            "barrowBrothers".into(),
            serde_json::json!(self.barrow_brothers),
        );
        data.insert(
            "barrowTunnelIndex".into(),
            Value::String(self.barrow_tunnel_index.to_string()),
        );
        data.insert(
            "kolodionStage".into(),
            Value::String(self.kolodion_stage.to_string()),
        );
        data.insert(
            "godCasts".into(),
            Value::Array(
                self.god_casts
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        );
        data.insert(
            "kolodionBoss".into(),
            Value::String(self.kolodion_boss.to_string()),
        );
        data.insert("elnockSupplies".into(), Value::Bool(self.elnock_supplies));
        data.insert(
            "lastBorkBattle".into(),
            Value::String(self.last_bork_battle.to_string()),
        );
        data.insert("startedMta".into(), Value::Bool(self.started_mta));
        data.insert("lostCannon".into(), Value::Bool(self.lost_cannon));
        data.insert(
            "pizazzPoints".into(),
            Value::Array(
                self.pizazz_points
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        );
        data.insert("bonesToPeaches".into(), Value::Bool(self.bones_to_peaches));
        data.insert(
            "solvedMazes".into(),
            Value::String(self.solved_mazes.to_string()),
        );
        data.insert(
            "fogRating".into(),
            Value::String(self.fog_rating.to_string()),
        );
        data.insert(
            "borkKills".into(),
            Value::String(self.bork_kills.to_string()),
        );
        data.insert("hardcoreDeath".into(), Value::Bool(self.hardcore_death));
        data.insert("topGrabbed".into(), Value::Bool(self.top_grabbed));
        Ok(Value::Object(data))
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestData {
    pub cooks_assistant: Vec<bool>,
    pub demon_slayer: Vec<bool>,
    pub draynor_lever: Vec<bool>,
    pub dragon_slayer: Vec<bool>,
    pub desert_treasure: Vec<Option<ItemData>>,
    pub dragon_slayer_planks: i32,
    pub gardener_attack: bool,
    pub talked_drezel: bool,
    pub witchs_experiment_stage: i32,
    pub witchs_experiment_killed: bool,
}
impl Default for QuestData {
    fn default() -> Self {
        Self {
            cooks_assistant: vec![false; 4],
            demon_slayer: vec![false; 2],
            draynor_lever: vec![true; 6],
            dragon_slayer: vec![false; 9],
            desert_treasure: vec![
                Some(ItemData::new(1513, 12)),
                Some(ItemData::new(592, 10)),
                Some(ItemData::new(1775, 6)),
                Some(ItemData::new(2353, 6)),
                Some(ItemData::new(526, 2)),
                Some(ItemData::new(973, 2)),
                Some(ItemData::new(565, 1)),
            ],
            dragon_slayer_planks: 0,
            gardener_attack: false,
            talked_drezel: false,
            witchs_experiment_stage: 0,
            witchs_experiment_killed: false,
        }
    }
}
impl QuestData {
    pub fn parse(&mut self, data: &Value) -> Result<()> {
        let data = json::object(data)?;
        json::array(data, "draynorLever", &mut self.draynor_lever, json::boolean)?;
        json::array(data, "dragonSlayer", &mut self.dragon_slayer, json::boolean)?;
        self.dragon_slayer_planks = json::int(json::required(data, "dragonSlayerPlanks")?)?;
        json::array(data, "demonSlayer", &mut self.demon_slayer, json::boolean)?;
        json::array(
            data,
            "cooksAssistant",
            &mut self.cooks_assistant,
            json::boolean,
        )?;
        self.gardener_attack = json::boolean(json::required(data, "gardenerAttack")?)?;
        self.talked_drezel = json::boolean(json::required(data, "talkedDrezel")?)?;
        json::items(data, "desertTreasureNode", &mut self.desert_treasure)?;
        self.witchs_experiment_killed =
            json::boolean(json::required(data, "witchsExperimentKilled")?)?;
        self.witchs_experiment_stage = json::int(json::required(data, "witchsExperimentStage")?)?;
        Ok(())
    }
    pub fn save(&self) -> Result<Value> {
        let mut data = Map::new();
        data.insert("draynorLever".into(), serde_json::json!(self.draynor_lever));
        data.insert("dragonSlayer".into(), serde_json::json!(self.dragon_slayer));
        data.insert(
            "dragonSlayerPlanks".into(),
            Value::String(self.dragon_slayer_planks.to_string()),
        );
        data.insert("demonSlayer".into(), serde_json::json!(self.demon_slayer));
        data.insert(
            "cooksAssistant".into(),
            serde_json::json!(self.cooks_assistant),
        );
        data.insert("gardenerAttack".into(), Value::Bool(self.gardener_attack));
        data.insert("talkedDrezel".into(), Value::Bool(self.talked_drezel));
        data.insert("desertTreasureNode".into(),Value::Array(self.desert_treasure.iter().map(|item|{let item=item.as_ref().ok_or(Error::Null)?;Ok(serde_json::json!({"id":item.id().to_string(),"amount":item.amount.to_string()}))}).collect::<Result<Vec<_>>>()?));
        data.insert(
            "witchsExperimentKilled".into(),
            Value::Bool(self.witchs_experiment_killed),
        );
        data.insert(
            "witchsExperimentStage".into(),
            Value::String(self.witchs_experiment_stage.to_string()),
        );
        Ok(Value::Object(data))
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalData {
    pub home_teleport_delay: i64,
    pub assist_time: i64,
    pub chat_ping: i64,
    pub tutor_claim: i64,
    pub luthas_task: bool,
    pub karamja_bananas: i32,
    pub saved_x: i32,
    pub saved_y: i32,
    pub saved_h: i32,
    pub task_amount: i32,
    pub task_points: i32,
    pub silk_steal: i64,
    pub assist_experience: Vec<f64>,
    pub strong_hold_rewards: Vec<bool>,
    pub lumbridge_rope: bool,
    pub apprentice: bool,
    pub fritz_glass: bool,
    pub zaf_amount: i32,
    pub zaf_time: i64,
    pub wydin_employee: bool,
    pub ge_tutorial: bool,
    pub essence_teleporter: i32,
    pub recoil_damage: i32,
    pub double_exp_delay: i64,
    pub drop_delay: i64,
    pub joined_monastery: bool,
    pub read_plaques: Vec<bool>,
    pub forging_uses: i32,
    pub ecto_charges: i32,
    pub abyss_data: Vec<bool>,
    pub rc_decays: Vec<i32>,
    pub disable_death_screen: bool,
    pub player_test_stage: i32,
    pub charming_delay: i64,
    pub travel_logs: Vec<bool>,
    pub god_books: Vec<bool>,
    pub god_book: i32,
    pub disable_news: bool,
    pub god_pages: Vec<bool>,
    pub over_charge_delay: i64,
    pub boss_counters: Vec<i32>,
    pub barrows_loots: i32,
    pub loot_share_points: i32,
    pub loot_share_delay: i64,
    pub double_exp: i64,
    pub global_teleporter_delay: i64,
    pub star_sprite_delay: i64,
    pub run_replenish_delay: i64,
    pub run_replenish_charges: i32,
    pub low_alchemy_charges: i32,
    pub low_alchemy_delay: i64,
    pub enable_bone_crusher: bool,
    pub enable_coin_machine: bool,
    pub enable_charm_collector: bool,
    pub magic_skill_cape_delay: i64,
    pub hunter_cape_delay: i64,
    pub hunter_cape_charges: i32,
    pub minigame_teleport_delay: i64,
    pub macro_disabled: bool,
}
impl Default for GlobalData {
    fn default() -> Self {
        Self {
            home_teleport_delay: 0,
            assist_time: 0,
            chat_ping: 0,
            tutor_claim: 0,
            luthas_task: false,
            karamja_bananas: 0,
            saved_x: 0,
            saved_y: 0,
            saved_h: 0,
            task_amount: 0,
            task_points: 0,
            silk_steal: 0,
            assist_experience: vec![0.0; 9],
            strong_hold_rewards: vec![false; 4],
            lumbridge_rope: false,
            apprentice: false,
            fritz_glass: false,
            zaf_amount: 8,
            zaf_time: 0,
            wydin_employee: false,
            ge_tutorial: false,
            essence_teleporter: 0,
            recoil_damage: 40,
            double_exp_delay: 0,
            drop_delay: 0,
            joined_monastery: false,
            read_plaques: vec![false; 7],
            forging_uses: 0,
            ecto_charges: 0,
            abyss_data: vec![false; 4],
            rc_decays: vec![0; 3],
            disable_death_screen: false,
            player_test_stage: 0,
            charming_delay: 0,
            travel_logs: vec![false; 45],
            god_books: vec![false; 3],
            god_book: -1,
            disable_news: false,
            god_pages: vec![false; 4],
            over_charge_delay: 0,
            boss_counters: vec![0; 25],
            barrows_loots: 0,
            loot_share_points: 0,
            loot_share_delay: 0,
            double_exp: 0,
            global_teleporter_delay: 0,
            star_sprite_delay: 0,
            run_replenish_delay: 0,
            run_replenish_charges: 0,
            low_alchemy_charges: 0,
            low_alchemy_delay: 0,
            enable_bone_crusher: false,
            enable_coin_machine: false,
            enable_charm_collector: false,
            magic_skill_cape_delay: 0,
            hunter_cape_delay: 0,
            hunter_cape_charges: 0,
            minigame_teleport_delay: 0,
            macro_disabled: false,
        }
    }
}
impl GlobalData {
    pub fn parse(&mut self, data: &Value) -> Result<()> {
        let data = json::object(data)?;
        self.home_teleport_delay = json::long(json::required(data, "homeTeleportDelay")?)?;
        self.lumbridge_rope = json::boolean(json::required(data, "lumbridgeRope")?)?;
        self.apprentice = json::boolean(json::required(data, "apprentice")?)?;
        self.assist_time = json::long(json::required(data, "assistTime")?)?;
        json::array(
            data,
            "assistExperience",
            &mut self.assist_experience,
            json::double,
        )?;
        json::array(
            data,
            "strongHoldRewards",
            &mut self.strong_hold_rewards,
            json::boolean,
        )?;
        self.chat_ping = json::long(json::required(data, "chatPing")?)?;
        self.tutor_claim = json::long(json::required(data, "tutorClaim")?)?;
        self.luthas_task = json::boolean(json::required(data, "luthasTask")?)?;
        self.karamja_bananas = json::int(json::required(data, "karamjaBananas")?)?;
        self.silk_steal = json::long(json::required(data, "silkSteal")?)?;
        self.zaf_amount = json::int(json::required(data, "zafAmount")?)?;
        self.zaf_time = json::long(json::required(data, "zafTime")?)?;
        self.fritz_glass = json::boolean(json::required(data, "fritzGlass")?)?;
        self.wydin_employee = json::boolean(json::required(data, "wydinEmployee")?)?;
        self.ge_tutorial = json::boolean(json::required(data, "geTutorial")?)?;
        self.essence_teleporter = json::int(json::required(data, "essenceTeleporter")?)?;
        self.recoil_damage = json::int(json::required(data, "recoilDamage")?)?;
        self.double_exp_delay = json::long(json::required(data, "doubleExpDelay")?)?;
        self.joined_monastery = json::boolean(json::required(data, "joinedMonastery")?)?;
        json::array(data, "readPlaques", &mut self.read_plaques, json::boolean)?;
        self.forging_uses = json::int(json::required(data, "forgingUses")?)?;
        self.ecto_charges = json::int(json::required(data, "ectoCharges")?)?;
        self.drop_delay = json::long(json::required(data, "dropDelay")?)?;
        json::array(data, "abyssData", &mut self.abyss_data, json::boolean)?;
        json::array(data, "rcDecays", &mut self.rc_decays, json::int)?;
        self.disable_death_screen = json::boolean(json::required(data, "disableDeathScreen")?)?;
        self.player_test_stage = json::int(json::required(data, "playerTestStage")?)?;
        self.charming_delay = json::long(json::required(data, "charmingDelay")?)?;
        json::array(data, "travelLogs", &mut self.travel_logs, json::boolean)?;
        json::array(data, "godBooks", &mut self.god_books, json::boolean)?;
        self.disable_news = json::boolean(json::required(data, "disableNews")?)?;
        json::array(data, "godPages", &mut self.god_pages, json::boolean)?;
        self.over_charge_delay = json::long(json::required(data, "overChargeDelay")?)?;
        json::array(data, "bossCounters", &mut self.boss_counters, json::int)?;
        self.barrows_loots = json::int(json::required(data, "barrowsLoots")?)?;
        self.loot_share_delay = json::long(json::required(data, "lootShareDelay")?)?;
        self.loot_share_points = json::int(json::required(data, "lootSharePoints")?)?;
        self.double_exp = json::long(json::required(data, "doubleExp")?)?;
        self.global_teleporter_delay = json::long(json::required(data, "globalTeleporterDelay")?)?;
        self.star_sprite_delay = json::long(json::required(data, "starSpriteDelay")?)?;
        self.run_replenish_delay = json::long(json::required(data, "runReplenishDelay")?)?;
        self.run_replenish_charges = json::int(json::required(data, "runReplenishCharges")?)?;
        self.low_alchemy_charges = json::int(json::required(data, "lowAlchemyCharges")?)?;
        self.low_alchemy_delay = json::long(json::required(data, "lowAlchemyDelay")?)?;
        self.magic_skill_cape_delay = json::long(json::required(data, "magicSkillCapeDelay")?)?;
        self.hunter_cape_delay = json::long(json::required(data, "hunterCapeDelay")?)?;
        self.hunter_cape_charges = json::int(json::required(data, "hunterCapeCharges")?)?;
        self.task_amount = json::int(json::required(data, "taskAmount")?)?;
        self.task_points = json::int(json::required(data, "taskPoints")?)?;
        self.macro_disabled = json::boolean(json::required(data, "macroDisabled")?)?;
        Ok(())
    }
    pub fn save(&self) -> Result<Value> {
        let mut data = Map::new();
        data.insert(
            "homeTeleportDelay".into(),
            Value::String(self.home_teleport_delay.to_string()),
        );
        data.insert("lumbridgeRope".into(), Value::Bool(self.lumbridge_rope));
        data.insert("apprentice".into(), Value::Bool(self.apprentice));
        data.insert(
            "assistTime".into(),
            Value::String(self.assist_time.to_string()),
        );
        data.insert(
            "assistExperience".into(),
            Value::Array(
                self.assist_experience
                    .iter()
                    .map(|value| Value::String(rs09_game_core::java_double::format(*value)))
                    .collect(),
            ),
        );
        data.insert(
            "strongHoldRewards".into(),
            serde_json::json!(self.strong_hold_rewards),
        );
        data.insert("chatPing".into(), Value::String(self.chat_ping.to_string()));
        data.insert(
            "tutorClaim".into(),
            Value::String(self.tutor_claim.to_string()),
        );
        data.insert("luthasTask".into(), Value::Bool(self.luthas_task));
        data.insert(
            "karamjaBananas".into(),
            Value::String(self.karamja_bananas.to_string()),
        );
        data.insert(
            "silkSteal".into(),
            Value::String(self.silk_steal.to_string()),
        );
        data.insert(
            "zafAmount".into(),
            Value::String(self.zaf_amount.to_string()),
        );
        data.insert("zafTime".into(), Value::String(self.zaf_time.to_string()));
        data.insert("fritzGlass".into(), Value::Bool(self.fritz_glass));
        data.insert("wydinEmployee".into(), Value::Bool(self.wydin_employee));
        data.insert("geTutorial".into(), Value::Bool(self.ge_tutorial));
        data.insert(
            "essenceTeleporter".into(),
            Value::String(self.essence_teleporter.to_string()),
        );
        data.insert(
            "recoilDamage".into(),
            Value::String(self.recoil_damage.to_string()),
        );
        data.insert(
            "doubleExpDelay".into(),
            Value::String(self.double_exp_delay.to_string()),
        );
        data.insert("joinedMonastery".into(), Value::Bool(self.joined_monastery));
        data.insert("readPlaques".into(), serde_json::json!(self.read_plaques));
        data.insert(
            "forgingUses".into(),
            Value::String(self.forging_uses.to_string()),
        );
        data.insert(
            "ectoCharges".into(),
            Value::String(self.ecto_charges.to_string()),
        );
        data.insert(
            "dropDelay".into(),
            Value::String(self.drop_delay.to_string()),
        );
        data.insert("abyssData".into(), serde_json::json!(self.abyss_data));
        data.insert(
            "rcDecays".into(),
            Value::Array(
                self.rc_decays
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        );
        data.insert(
            "disableDeathScreen".into(),
            Value::Bool(self.disable_death_screen),
        );
        data.insert(
            "playerTestStage".into(),
            Value::String(self.player_test_stage.to_string()),
        );
        data.insert(
            "charmingDelay".into(),
            Value::String(self.charming_delay.to_string()),
        );
        data.insert("travelLogs".into(), serde_json::json!(self.travel_logs));
        data.insert("godBooks".into(), serde_json::json!(self.god_books));
        data.insert("disableNews".into(), Value::Bool(self.disable_news));
        data.insert("godPages".into(), serde_json::json!(self.god_pages));
        data.insert(
            "overChargeDelay".into(),
            Value::String(self.over_charge_delay.to_string()),
        );
        data.insert(
            "bossCounters".into(),
            Value::Array(
                self.boss_counters
                    .iter()
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        );
        data.insert(
            "barrowsLoots".into(),
            Value::String(self.barrows_loots.to_string()),
        );
        data.insert(
            "lootShareDelay".into(),
            Value::String(self.loot_share_delay.to_string()),
        );
        data.insert(
            "lootSharePoints".into(),
            Value::String(self.loot_share_points.to_string()),
        );
        data.insert(
            "doubleExp".into(),
            Value::String(self.double_exp.to_string()),
        );
        data.insert(
            "globalTeleporterDelay".into(),
            Value::String(self.global_teleporter_delay.to_string()),
        );
        data.insert(
            "starSpriteDelay".into(),
            Value::String(self.star_sprite_delay.to_string()),
        );
        data.insert(
            "runReplenishDelay".into(),
            Value::String(self.run_replenish_delay.to_string()),
        );
        data.insert(
            "runReplenishCharges".into(),
            Value::String(self.run_replenish_charges.to_string()),
        );
        data.insert(
            "lowAlchemyCharges".into(),
            Value::String(self.low_alchemy_charges.to_string()),
        );
        data.insert(
            "lowAlchemyDelay".into(),
            Value::String(self.low_alchemy_delay.to_string()),
        );
        data.insert(
            "magicSkillCapeDelay".into(),
            Value::String(self.magic_skill_cape_delay.to_string()),
        );
        data.insert(
            "hunterCapeDelay".into(),
            Value::String(self.hunter_cape_delay.to_string()),
        );
        data.insert(
            "hunterCapeCharges".into(),
            Value::String(self.hunter_cape_charges.to_string()),
        );
        data.insert(
            "taskAmount".into(),
            Value::String(self.task_amount.to_string()),
        );
        data.insert(
            "taskPoints".into(),
            Value::String(self.task_points.to_string()),
        );
        data.insert("macroDisabled".into(), Value::Bool(self.macro_disabled));
        Ok(Value::Object(data))
    }
}
