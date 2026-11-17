//! ContentAPI's quest functions, `hasRequirement` (both overloads) and the
//! diary task functions.
use super::{Arg, Error, Host, Member as M, Requirement, Result, b, i, n, nd, unit};
use crate::world::content_api::ui::send_message;

/// `QuestRequirements` in declaration order: (constant, `Quests` constant).
/// The requirements each constant carries are evaluated by the
/// requirement classes (a service); only the lookup is ContentAPI's.
pub const QUEST_REQUIREMENTS: [(&str, &str); 140] = [
    ("COOK_ASSIST", "COOKS_ASSISTANT"),
    ("DEMON_SLAYER", "DEMON_SLAYER"),
    ("DORIC_QUEST", "DORICS_QUEST"),
    ("DRAGON_SLAYER", "DRAGON_SLAYER"),
    ("ERNEST", "ERNEST_THE_CHICKEN"),
    ("GOBLIN_DIP", "GOBLIN_DIPLOMACY"),
    ("IMP_CATCHER", "IMP_CATCHER"),
    ("KNIGHT_SWORD", "THE_KNIGHTS_SWORD"),
    ("PIRATE_T", "PIRATES_TREASURE"),
    ("ALI_RESCUE", "PRINCE_ALI_RESCUE"),
    ("RESTLESS_GHOST", "THE_RESTLESS_GHOST"),
    ("ROMEO", "ROMEO_JULIET"),
    ("RUNE_MYST", "RUNE_MYSTERIES"),
    ("SHEEP", "SHEEP_SHEARER"),
    ("ARRAV", "SHIELD_OF_ARRAV"),
    ("VAMPIRE", "VAMPIRE_SLAYER"),
    ("DORIC", "DORICS_QUEST"),
    ("RUNE_MYSTERIES", "RUNE_MYSTERIES"),
    ("BLACK_KNIGHT", "BLACK_KNIGHTS_FORTRESS"),
    ("WITCH_POTION", "WITCHS_POTION"),
    ("DRUIDIC_RITUAL", "DRUIDIC_RITUAL"),
    ("LOST_CITY", "LOST_CITY"),
    ("WITCH_HOUSE", "WITCHS_HOUSE"),
    ("MERLIN", "MERLINS_CRYSTAL"),
    ("HERO", "HEROES_QUEST"),
    ("SCORP_CATCHER", "SCORPION_CATCHER"),
    ("FAMILY_CREST", "FAMILY_CREST"),
    ("FISHING_CONTEST", "FISHING_CONTEST"),
    ("TOTEM", "TRIBAL_TOTEM"),
    ("MONK", "MONKS_FRIEND"),
    ("IKOV", "TEMPLE_OF_IKOV"),
    ("CLOCK_TOWER", "CLOCK_TOWER"),
    ("GRAIL", "HOLY_GRAIL"),
    ("GNOME_VILLAGE", "TREE_GNOME_VILLAGE"),
    ("FIGHT_ARENA", "FIGHT_ARENA"),
    ("HAZEEL", "HAZEEL_CULT"),
    ("SHEEP_HERDER", "SHEEP_HERDER"),
    ("PLAGUE_CITY", "PLAGUE_CITY"),
    ("SEA_SLUG", "SEA_SLUG"),
    ("WATERFALL", "WATERFALL_QUEST"),
    ("POTION", "JUNGLE_POTION"),
    ("GRAND_TREE", "THE_GRAND_TREE"),
    ("BIOHAZARD", "BIOHAZARD"),
    ("UNDERGROUND_PASS", "UNDERGROUND_PASS"),
    ("OBSERVATORY", "OBSERVATORY_QUEST"),
    ("TOURIST", "THE_TOURIST_TRAP"),
    ("WATCHTOWER", "WATCHTOWER"),
    ("DWARF_CANNON", "DWARF_CANNON"),
    ("MURDER_MYS", "MURDER_MYSTERY"),
    ("DIG_SITE", "THE_DIG_SITE"),
    ("GERTRUDE", "GERTRUDES_CAT"),
    ("SHILO", "SHILO_VILLAGE"),
    ("LEGEND", "LEGENDS_QUEST"),
    ("DEATH_PLATEAU", "DEATH_PLATEAU"),
    ("TROLL_STRONGHOLD", "TROLL_STRONGHOLD"),
    ("EADGAR", "EADGARS_RUSE"),
    ("CHOMPY", "BIG_CHOMPY_BIRD_HUNTING"),
    ("ELEMENTAL_W1", "ELEMENTAL_WORKSHOP_I"),
    ("PRIEST", "PRIEST_IN_PERIL"),
    ("NATURE_SPIRIT", "NATURE_SPIRIT"),
    ("REGICIDE", "REGICIDE"),
    ("TAI_BWO", "TAI_BWO_WANNAI_TRIO"),
    ("SHADES", "SHADES_OF_MORTTON"),
    ("FREM_TRIALS", "THE_FREMENNIK_TRIALS"),
    ("HORROR_DEEP", "HORROR_FROM_THE_DEEP"),
    ("THRONE", "THRONE_OF_MISCELLANIA"),
    ("MONKEY", "MONKEY_MADNESS"),
    ("MINE", "HAUNTED_MINE"),
    ("TROLL_ROMANCE", "TROLL_ROMANCE"),
    ("SEARCH_MYREQUE", "IN_SEARCH_OF_THE_MYREQUE"),
    ("FENKENSTRAIN", "CREATURE_OF_FENKENSTRAIN"),
    ("ROVING_ELVES", "ROVING_ELVES"),
    ("GHOSTS_AHOY", "GHOSTS_AHOY"),
    ("FAVOR", "ONE_SMALL_FAVOUR"),
    ("MOUNTAIN_DAUGHTER", "MOUNTAIN_DAUGHTER"),
    ("BETWEEN_ROCK", "BETWEEN_A_ROCK"),
    ("FEUD", "THE_FEUD"),
    ("GOLEM", "THE_GOLEM"),
    ("DESERT", "DESERT_TREASURE"),
    ("ICTHLARIN", "ICTHLARINS_LITTLE_HELPER"),
    ("TEARS_OF_GUTHIX", "TEARS_OF_GUTHIX"),
    ("LOST_TRIBE", "THE_LOST_TRIBE"),
    ("GIANT_DWARF", "THE_GIANT_DWARF"),
    ("RECRUITMENT_DRIVE", "RECRUITMENT_DRIVE"),
    ("MEP_1", "MOURNINGS_END_PART_I"),
    ("FORGETTABLE", "FORGETTABLE_TALE"),
    ("GARDEN", "GARDEN_OF_TRANQUILITY"),
    ("TWO_CATS", "A_TAIL_OF_TWO_CATS"),
    ("WANTED", "WANTED"),
    ("MEP_2", "MOURNINGS_END_PART_II"),
    ("ZOGRE", "ZOGRE_FLESH_EATERS"),
    ("RUM_DEAL", "RUM_DEAL"),
    ("SHADOW", "SHADOW_OF_THE_STORM"),
    ("HISTORY", "MAKING_HISTORY"),
    ("RATCATCHERS", "RATCATCHERS"),
    ("SPIRITS_ELID", "SPIRITS_OF_THE_ELID"),
    ("DEVIOUS", "DEVIOUS_MINDS"),
    ("SAND", "THE_HAND_IN_THE_SAND"),
    ("ENAKHRA", "ENAKHRAS_LAMENT"),
    ("CABIN_FEVER", "CABIN_FEVER"),
    ("FAIRYTALE_1", "FAIRYTALE_I_GROWING_PAINS"),
    ("RFD", "RECIPE_FOR_DISASTER"),
    ("AID_MYREQUE", "IN_AID_OF_THE_MYREQUE"),
    ("SOUL_BANE", "A_SOULS_BANE"),
    ("BONE_MAN_1", "RAG_AND_BONE_MAN"),
    ("SWAN", "SWAN_SONG"),
    ("ROYAL_TROUBLE", "ROYAL_TROUBLE"),
    ("DEATH_DORGESHUUN", "DEATH_TO_THE_DORGESHUUN"),
    ("FAIRYTALE_2", "FAIRYTALE_II_CURE_A_QUEEN"),
    ("LUNAR_DIPLOMACY", "LUNAR_DIPLOMACY"),
    ("GLOUPHRIE", "THE_EYES_OF_GLOUPHRIE"),
    ("HALLOWVALE", "DARKNESS_OF_HALLOWVALE"),
    ("SLUG_MENACE", "THE_SLUG_MENACE"),
    ("ELEMENTAL_W2", "ELEMENTAL_WORKSHOP_II"),
    ("ARM_ADVENTURE", "MY_ARMS_BIG_ADVENTURE"),
    ("ENL_JOURNEY", "ENLIGHTENED_JOURNEY"),
    ("EAGLE", "EAGLES_PEAK"),
    ("ANMA", "ANIMAL_MAGNETISM"),
    ("CONTACT", "CONTACT"),
    ("COLD_WAR", "COLD_WAR"),
    ("FREM_ISLES", "THE_FREMENNIK_ISLES"),
    ("BRAIN_ROBBERY", "THE_GREAT_BRAIN_ROBBERY"),
    ("WHAT_LIES_BELOW", "WHAT_LIES_BELOW"),
    ("OLAF", "OLAFS_QUEST"),
    ("ANOTHER_SLICE", "ANOTHER_SLICE_OF_HAM"),
    ("DREAM_MENTOR", "DREAM_MENTOR"),
    ("GRIM_TALES", "GRIM_TALES"),
    ("KINGS_RANSOM", "KINGS_RANSOM"),
    ("TOWER_OF_LIFE", "TOWER_OF_LIFE"),
    ("BONE_MAN_2", "RAG_AND_BONE_MAN"),
    ("LAND_GOBLINS", "LAND_OF_THE_GOBLINS"),
    ("PATH_GLOUPHRIE", "THE_PATH_OF_GLOUPHRIE"),
    ("DEFENDER_VARROCK", "DEFENDER_OF_VARROCK"),
    ("SPIRIT_OF_SUMMER", "SPIRIT_OF_SUMMER"),
    ("SUMMERS_END", "SUMMERS_END"),
    ("SEERGAZE", "LEGACY_OF_SEERGAZE"),
    ("SMOKING_KILLS", "SMOKING_KILLS"),
    ("WHILE_GUTHIX_SLEEPS", "WHILE_GUTHIX_SLEEPS"),
    ("ALL_FIRED_UP", "ALL_FIRED_UP"),
    ("KENNITHS_CONCERNS", "KENNITHS_CONCERNS"),
];

/// The `Quests` constant of a `QuestRequirements` constant.
fn quest_of(requirement: &str) -> Result<&'static str> {
    QUEST_REQUIREMENTS
        .iter()
        .find(|(r, _)| *r == requirement)
        .map(|(_, q)| *q)
        .ok_or_else(|| Error::Argument(format!("No enum constant QuestRequirements.{requirement}")))
}

/// `getQuestPoints(player)`.
pub fn get_quest_points<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<i32> {
    i(h, M::QuestPoints, vec![nd(p)])
}

/// `getQuestStage(player, quest)`.
pub fn get_quest_stage<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    quest: &'static str,
) -> Result<i32> {
    i(h, M::QuestStage, vec![nd(p), Arg::Enum(quest)])
}

/// `setQuestStage(player, quest, stage)`: `QuestRepository.getQuests()[quest]!!`.
pub fn set_quest_stage<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    quest: &'static str,
    stage: i32,
) -> Result<()> {
    let quests = h.call(M::QuestAll, vec![])?.quest_map()?;
    let q = quests
        .into_iter()
        .find(|(k, _)| k == quest)
        .map(|(_, q)| q)
        .ok_or(Error::Null)?;
    unit(h, M::QuestSetStage, vec![nd(p), nd(&q), Arg::Int(stage)])?;
    unit(h, M::QuestSyncTab, vec![nd(p), nd(p)])
}

/// `isQuestInProgress(player, quest, startStage, endStage)`: inclusive.
pub fn is_quest_in_progress<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    quest: &'static str,
    start: i32,
    end: i32,
) -> Result<bool> {
    let stage = get_quest_stage(h, p, quest)?;
    Ok((start..=end).contains(&stage))
}

/// `isQuestStarted(player, quest)`.
pub fn is_quest_started<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    quest: &'static str,
) -> Result<bool> {
    Ok(get_quest_stage(h, p, quest)? > 0)
}

/// `isQuestComplete(player, quest)`.
pub fn is_quest_complete<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    quest: &'static str,
) -> Result<bool> {
    Ok(get_quest_stage(h, p, quest)? == 100)
}

/// `getQuest(player, quest)` (Kotlin's non-null return).
pub fn get_quest<H: Host + ?Sized>(h: &mut H, p: &H::Node, quest: &'static str) -> Result<H::Node> {
    n(h, M::QuestGet, vec![nd(p), Arg::Enum(quest)])?.ok_or(Error::Null)
}

/// `startQuest(player, quest)`.
pub fn start_quest<H: Host + ?Sized>(h: &mut H, p: &H::Node, quest: &'static str) -> Result<bool> {
    let q = get_quest(h, p, quest)?;
    if !b(h, M::QuestHasRequirements, vec![nd(&q), nd(p)])? {
        return Ok(false);
    }
    unit(h, M::QuestStart, vec![nd(&q), nd(p)])?;
    Ok(true)
}

/// `finishQuest(player, quest)`.
pub fn finish_quest<H: Host + ?Sized>(h: &mut H, p: &H::Node, quest: &'static str) -> Result<()> {
    let q = get_quest(h, p, quest)?;
    unit(h, M::QuestFinish, vec![nd(&q), nd(p)])
}

/// `requireQuest(player, quest, message)`.
pub fn require_quest<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    quest: &'static str,
    message: &str,
) -> Result<bool> {
    if !is_quest_complete(h, p, quest)? {
        let name = h.quest_name(quest);
        send_message(
            h,
            p,
            &format!("You must have completed the {name} quest {message}"),
        )?;
        return Ok(false);
    }
    Ok(true)
}

/// `hasRequirement(player, req: QuestReq, message = true)`.
pub fn has_requirement<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    requirement: &str,
    stage: i32,
    message: bool,
) -> Result<bool> {
    let req = Requirement::Quest {
        requirement: requirement.into(),
        stage,
    };
    let (met, unmet) = h
        .call(M::Evaluate, vec![req.arg(), nd(p)])?
        .evaluation()?
        .ok_or(Error::Null)?;
    let (mut soft, mut hard) = (0i32, 0i32);
    for r in &unmet {
        match r {
            Requirement::QpCumulative(a) => soft = soft.wrapping_add(*a),
            Requirement::QpReq(a) if *a > hard => hard = *a,
            _ => {}
        }
    }
    let available = i(h, M::QuestAvailablePoints, vec![nd(p)])?;
    let needed = soft.max(hard).min(available);
    let met = met && needed <= i(h, M::QuestPoints, vec![nd(p)])?;
    if met {
        return Ok(true);
    }
    let mut lines = Vec::new();
    if let (
        2,
        Some(Requirement::Quest {
            requirement: first, ..
        }),
    ) = (unmet.len(), unmet.first())
    {
        let name = h.quest_name(quest_of(first)?);
        lines.push(format!("This requires completion of {name} to access."));
    } else {
        let name = h.quest_name(quest_of(requirement)?);
        lines.push(format!("You need the pre-reqs for {name} to access this."));
        lines.push("Please check the page in your quest journal for more info.".into());
    }
    if message {
        for line in lines {
            send_message(h, p, &line)?;
        }
    }
    Ok(false)
}

/// `hasRequirement(player, quest: Quests, message = true)`: the first
/// `QuestRequirements` constant for the quest, else false.
pub fn has_quest_requirement<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    quest: &str,
    message: bool,
) -> Result<bool> {
    let Some((requirement, _)) = QUEST_REQUIREMENTS.iter().find(|(_, q)| *q == quest) else {
        return Ok(false);
    };
    has_requirement(h, p, requirement, 100, message)
}

/// `DiaryLevel`.
pub const DIARY_LEVELS: [&str; 4] = ["BEGINNER", "EASY", "MEDIUM", "HARD"];

/// `getDiaryLevelIndex(diary, level)`: the level's capitalised name in the
/// diary's level names.
pub fn get_diary_level_index<H: Host + ?Sized>(h: &H, diary: &str, level: &str) -> Result<i32> {
    let lower = level.to_lowercase();
    let mut chars = lower.chars();
    let name = match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    };
    match h.diary_level_names(diary).iter().position(|n| *n == name) {
        Some(index) => Ok(index as i32),
        None => Err(Error::Argument(format!(
            "'{name}' was not found in diary '{diary}'."
        ))),
    }
}

/// `finishTask(player, diary, level, task)`.
pub fn finish_task<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    diary: &'static str,
    level: &str,
    task: i32,
) -> Result<()> {
    let index = get_diary_level_index(h, diary, level)?;
    unit(
        h,
        M::DiaryFinishTask,
        vec![
            nd(p),
            nd(p),
            Arg::Enum(diary),
            Arg::Int(index),
            Arg::Int(task),
        ],
    )
}
