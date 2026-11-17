//! PlayerSaveParser.kt. Every section keeps the original cast, conversion and
//! mutation order, so a failure leaves the same earlier changes in place.
use super::{
    EMOTES, Env, Error, GRAVE_TYPES, Host, Level, Profile, Result, SPELL_BOOK_INTERFACES,
    SPELL_BOOKS, order,
};
use crate::{
    player::{
        attributes::LegacyLoad,
        core_data::CoreData,
        ironman,
        skills::{self, Effect, EntityKind, Skills},
    },
    timers,
};
use rs09_game_core::{
    java_json::{self, Json},
    java_string::parse_i32,
};
use serde_json::Value;

/// What a completed parseData did besides mutating the profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Loaded {
    /// The legacy attribute file is consulted only when the save has no
    /// `attributes` key. The original prints and swallows its failures.
    pub legacy_attributes: LegacyLoad,
}
/// The fields of one PlayerSaveParser instance.
#[derive(Clone, Debug)]
pub struct Parser {
    pub read: bool,
    pub save_file: Option<Json>,
}
impl Default for Parser {
    fn default() -> Self {
        Self {
            read: true,
            save_file: None,
        }
    }
}

fn get<'a>(root: &'a Json, key: &str) -> Option<&'a Json> {
    root.get(key)
}
/// `value as JSONObject`.
fn object(value: Option<&Json>) -> Result<&Json> {
    match value {
        None | Some(Json::Null) => Err(Error::Null),
        Some(value @ Json::Object(_)) => Ok(value),
        Some(_) => Err(Error::Type),
    }
}
/// `value as JSONArray`.
fn array(value: Option<&Json>) -> Result<&[Json]> {
    match value {
        None | Some(Json::Null) => Err(Error::Null),
        Some(Json::Array(values)) => Ok(values),
        Some(_) => Err(Error::Type),
    }
}
/// `value as String`.
fn string(value: Option<&Json>) -> Result<&str> {
    match value {
        None | Some(Json::Null) => Err(Error::Null),
        Some(Json::Text(value)) => Ok(value),
        Some(_) => Err(Error::Type),
    }
}
/// Kotlin `Any?.toString()`: a missing or null value reads as "null".
fn display(value: Option<&Json>) -> String {
    value.map_or_else(|| "null".into(), Json::text)
}
fn int(text: &str) -> Result<i32> {
    parse_i32(text).ok_or(Error::Number)
}
fn subset(root: &Json, keys: &[&str]) -> Value {
    let mut map = serde_json::Map::new();
    for key in keys {
        if let Some(value) = get(root, key) {
            map.insert((*key).to_owned(), order::value(value));
        }
    }
    Value::Object(map)
}
/// `saveFile!!`.
fn required(save_file: Option<&Json>) -> Result<&Json> {
    match save_file {
        Some(root @ Json::Object(_)) => Ok(root),
        None | Some(Json::Null) => Err(Error::Null),
        Some(_) => Err(Error::Type),
    }
}

/// Skills reads the save version and the `permadeath` attribute from the
/// player itself; every other skill service stays with the caller's context.
struct SkillBinding<'a, H> {
    host: &'a mut H,
    version: i32,
    permadeath: bool,
}
impl<H: skills::Context> skills::Context for SkillBinding<'_, H> {
    fn kind(&self) -> EntityKind {
        self.host.kind()
    }
    fn version(&self) -> i32 {
        self.version
    }
    fn tick(&mut self) -> i32 {
        self.host.tick()
    }
    fn world_tick(&mut self) -> i32 {
        self.host.world_tick()
    }
    fn combat_level(&self) -> i32 {
        self.host.combat_level()
    }
    fn summoning_combat_level(&self) -> i32 {
        self.host.summoning_combat_level()
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        self.host.npc_combat_level()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        self.host.damage_spong_active()
    }
    fn assisted_level(&mut self, skill: i32) -> skills::Result<Option<i32>> {
        self.host.assisted_level(skill)
    }
    fn translate_experience(
        &mut self,
        state: &mut Skills,
        skill: i32,
        amount: f64,
        multiplier: f64,
    ) -> skills::Result<bool> {
        self.host
            .translate_experience(state, skill, amount, multiplier)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        self.host.brawling_present()
    }
    fn brawling_for_skill(&mut self, skill: i32) -> skills::Result<Option<i32>> {
        self.host.brawling_for_skill(skill)
    }
    fn equipment_contains(&mut self, id: i32) -> skills::Result<bool> {
        self.host.equipment_contains(id)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        self.host.brawling_bonus()
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        self.host.tutorial_complete()
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        self.host.artificial()
    }
    fn username(&mut self) -> skills::Result<String> {
        self.host.username()
    }
    fn permadeath_present(&self) -> bool {
        self.permadeath
    }
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        self.host.effect(state, effect)
    }
}

/// The JDK 11 UTF-8 decoder with malformed input replaced, which is what a
/// FileReader does when the default charset is UTF-8. It differs from
/// `String::from_utf8_lossy` for encoded surrogates: the JDK replaces a
/// well-formed three-byte surrogate once, and a truncated one once.
fn decode_utf8(bytes: &[u8]) -> String {
    fn continuation(byte: u8) -> bool {
        byte & 0xc0 == 0x80
    }
    fn bad_second_of_three(b1: u8, b2: u8) -> bool {
        (b1 == 0xe0 && b2 & 0xe0 == 0x80) || !continuation(b2)
    }
    fn bad_second_of_four(b1: u8, b2: u8) -> bool {
        (b1 == 0xf0 && !(0x90..=0xbf).contains(&b2))
            || (b1 == 0xf4 && b2 & 0xf0 != 0x80)
            || !continuation(b2)
    }
    let mut out = String::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let rest = &bytes[at..];
        let b1 = rest[0];
        // The number of bytes consumed and the character they decode to, if any.
        let (length, decoded) = if b1 < 0x80 {
            (1, Some(b1 as char))
        } else if (0xc2..=0xdf).contains(&b1) {
            match rest.get(1) {
                Some(&b2) if continuation(b2) => (
                    2,
                    char::from_u32((u32::from(b1 & 0x1f) << 6) | u32::from(b2 & 0x3f)),
                ),
                _ => (1, None),
            }
        } else if b1 & 0xf0 == 0xe0 {
            if rest.len() < 3 {
                if rest.len() > 1 && bad_second_of_three(b1, rest[1]) {
                    (1, None)
                } else {
                    (rest.len(), None)
                }
            } else if bad_second_of_three(b1, rest[1]) {
                (1, None)
            } else if !continuation(rest[2]) {
                (2, None)
            } else {
                // A surrogate decodes to no character and is replaced as one sequence.
                (
                    3,
                    char::from_u32(
                        (u32::from(b1 & 0x0f) << 12)
                            | (u32::from(rest[1] & 0x3f) << 6)
                            | u32::from(rest[2] & 0x3f),
                    ),
                )
            }
        } else if b1 & 0xf8 == 0xf0 {
            if b1 > 0xf4 || (rest.len() > 1 && bad_second_of_four(b1, rest[1])) {
                (1, None)
            } else if rest.len() > 2 && !continuation(rest[2]) {
                (2, None)
            } else if rest.len() < 4 {
                (rest.len(), None)
            } else if !continuation(rest[3]) {
                (3, None)
            } else {
                (
                    4,
                    char::from_u32(
                        (u32::from(b1 & 0x07) << 18)
                            | (u32::from(rest[1] & 0x3f) << 12)
                            | (u32::from(rest[2] & 0x3f) << 6)
                            | u32::from(rest[3] & 0x3f),
                    ),
                )
            }
        } else {
            (1, None)
        };
        out.push(decoded.unwrap_or('\u{fffd}'));
        at += length;
    }
    out
}
/// JSONParser.parse followed by `as JSONObject`.
pub fn parse_text(text: &str) -> Result<Json> {
    match java_json::parse(text)? {
        root @ Json::Object(_) => Ok(root),
        Json::Null => Err(Error::Null),
        _ => Err(Error::Type),
    }
}
pub fn parse_version(profile: &mut Profile<'_>, save_file: Option<&Json>) -> Result<()> {
    let Some(root) = save_file else {
        return Ok(());
    };
    *profile.version = 0;
    if get(root, "version").is_some() {
        *profile.version = int(&display(get(root, "version")))?;
    }
    Ok(())
}
pub fn parse_core<T: timers::Context<CoreData>, H>(
    profile: &mut Profile<'_>,
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    Ok(profile
        .core
        .parse(save_file, env.definitions, profile.timers, env.timers)?)
}
pub fn parse_attributes<T, H: Host>(
    profile: &mut Profile<'_>,
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<LegacyLoad> {
    let root = required(save_file)?;
    let host = &mut *env.host;
    Ok(profile.attributes.parse_json_with_log(
        &subset(root, &["attributes"]),
        env.clock,
        env.attribute_path,
        profile.name,
        &mut |message| host.log(Level::Warn, message),
    )?)
}
pub fn parse_skills<T, H: skills::Context>(
    profile: &mut Profile<'_>,
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let Some(root) = save_file else {
        return Ok(());
    };
    let mut binding = SkillBinding {
        host: &mut *env.host,
        version: *profile.version,
        permadeath: profile.attributes.values.contains_key("permadeath"),
    };
    Ok(profile.skills.parse_profile(
        &subset(root, &["skills", "totalEXP", "exp_multiplier", "milestone"]),
        &mut binding,
    )?)
}
pub fn parse_settings<T, H: crate::player::settings::Context>(
    profile: &mut Profile<'_>,
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let Some(root) = save_file else {
        return Ok(());
    };
    let data = object(get(root, "settings"))?;
    Ok(profile.settings.parse(env.host, data)?)
}
pub fn parse_quests<T, H: Host>(save_file: Option<&Json>, env: &mut Env<'_, T, H>) -> Result<()> {
    let Some(root) = save_file else {
        return Ok(());
    };
    env.host.parse_quests(object(get(root, "quests"))?)
}
pub fn parse_appearance(profile: &mut Profile<'_>, save_file: Option<&Json>) -> Result<()> {
    let Some(root) = save_file else {
        return Ok(());
    };
    Ok(profile.appearance.parse(object(get(root, "appearance"))?)?)
}
pub fn parse_grave<T, H: Host>(
    profile: &mut Profile<'_>,
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let Some(root) = save_file else {
        return Ok(());
    };
    // `as? String`: anything but a string leaves the grave type alone.
    let Some(Json::Text(grave)) = get(root, "grave_type") else {
        return Ok(());
    };
    let ordinal = int(grave)?;
    if !(0..GRAVE_TYPES).contains(&ordinal) {
        return Err(Error::Index);
    }
    env.host.update_grave_type(profile.attributes, ordinal)
}
/// The legacy `varps` key and VarpManager.parse. Each completed row is
/// written to the same varp and save-flag maps the core section uses.
pub fn parse_varps(profile: &mut Profile<'_>, save_file: Option<&Json>) -> Result<()> {
    let root = required(save_file)?;
    if get(root, "varps").is_none() {
        return Ok(());
    }
    for row in array(get(root, "varps"))? {
        let row = object(Some(row))?;
        let index = int(&display(get(row, "index")))?;
        let mut total = 0i32;
        for bit in array(get(row, "bitArray"))? {
            let bit = object(Some(bit))?;
            let offset = int(&display(get(bit, "offset")))?;
            let value = int(&display(get(bit, "value")))?;
            total |= value.wrapping_shl(offset as u32);
        }
        profile.core.varps.insert(index, total);
        profile.core.save_varps.insert(index, Some(true));
    }
    Ok(())
}
pub fn parse_spellbook(profile: &mut Profile<'_>, save_file: Option<&Json>) -> Result<()> {
    let root = required(save_file)?;
    let interface = int(string(get(root, "spellbook"))?)?;
    // SpellBook.forInterface returns null for an unknown id and setSpellBook then dereferences it.
    if !SPELL_BOOK_INTERFACES.contains(&interface) {
        return Err(Error::Null);
    }
    *profile.spell_book = interface;
    Ok(())
}
pub fn parse_saved_data(profile: &mut Profile<'_>, save_file: Option<&Json>) -> Result<()> {
    let root = required(save_file)?;
    Ok(profile
        .saved
        .parse_sections(&subset(root, &["activityData", "questData", "globalData"]))?)
}
pub fn parse_autocast_spell<T, H: Host>(
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let root = required(save_file)?;
    let autocast = match get(root, "autocastSpell") {
        None | Some(Json::Null) => return Ok(()),
        raw => object(raw)?,
    };
    let book = int(string(get(autocast, "book"))?)?;
    let spell = int(string(get(autocast, "spellId"))?)?;
    if !(0..SPELL_BOOKS).contains(&book) {
        return Err(Error::Index);
    }
    env.host.set_autocast(book, spell)
}
/// The legacy `configs` key writes varps without setting their save flags.
pub fn parse_configs(profile: &mut Profile<'_>, save_file: Option<&Json>) -> Result<()> {
    let root = required(save_file)?;
    let Some(Json::Array(configs)) = get(root, "configs") else {
        return Ok(());
    };
    for config in configs {
        let config = object(Some(config))?;
        let index = int(string(get(config, "index"))?)?;
        if index == 1048 {
            continue;
        }
        let value = int(string(get(config, "value"))?)?;
        profile.core.varps.insert(index, value);
    }
    Ok(())
}
pub fn parse_music<T, H: Host>(save_file: Option<&Json>, env: &mut Env<'_, T, H>) -> Result<()> {
    let root = required(save_file)?;
    for song in array(get(root, "unlockedMusic"))? {
        env.host.unlock_music(int(string(Some(song))?)?)?;
    }
    Ok(())
}
pub fn parse_familiars<T, H: Host>(
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let root = required(save_file)?;
    env.host
        .parse_familiars(object(get(root, "familiarManager"))?)
}
pub fn parse_bank_pin(profile: &mut Profile<'_>, save_file: Option<&Json>) -> Result<()> {
    let root = required(save_file)?;
    Ok(profile.bank_pin.parse(get(root, "bankPinManager"))?)
}
pub fn parse_house<T, H: Host>(save_file: Option<&Json>, env: &mut Env<'_, T, H>) -> Result<()> {
    let root = required(save_file)?;
    env.host.parse_house(object(get(root, "houseData"))?)
}
pub fn parse_ironman(profile: &mut Profile<'_>, save_file: Option<&Json>) -> Result<()> {
    let root = required(save_file)?;
    if get(root, "ironManMode").is_none() {
        return Ok(());
    }
    let mode = match int(string(get(root, "ironManMode"))?)? {
        0 => ironman::Mode::None,
        1 => ironman::Mode::Standard,
        2 => ironman::Mode::Hardcore,
        3 => ironman::Mode::Ultimate,
        _ => return Err(Error::Index),
    };
    profile.ironman.set_mode(Some(mode));
    Ok(())
}
pub fn parse_emote_manager<T, H: Host>(
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let root = required(save_file)?;
    if get(root, "emoteData").is_none() {
        return Ok(());
    }
    for emote in array(get(root, "emoteData"))? {
        let ordinal = int(string(Some(emote))?)?;
        if !(0..EMOTES).contains(&ordinal) {
            return Err(Error::Index);
        }
        if !env.host.emote_unlocked(ordinal)? {
            env.host.unlock_emote(ordinal)?;
        }
    }
    Ok(())
}
/// The pinned method converts each row and stores nothing.
pub fn parse_statistics(save_file: Option<&Json>) -> Result<()> {
    let root = required(save_file)?;
    if get(root, "statistics").is_none() {
        return Ok(());
    }
    for stat in array(get(root, "statistics"))? {
        let stat = object(Some(stat))?;
        int(string(get(stat, "index"))?)?;
        int(string(get(stat, "value"))?)?;
    }
    Ok(())
}
pub fn parse_achievements<T, H: Host>(
    profile: &mut Profile<'_>,
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let root = required(save_file)?;
    if get(root, "achievementDiaries").is_none() {
        return env.host.reset_diary_rewards(profile.core, env.definitions);
    }
    let data = get(root, "achievementDiaries");
    array(data)?;
    env.host.parse_diaries(data.unwrap())
}
pub fn parse_pouches<T, H: Host>(save_file: Option<&Json>, env: &mut Env<'_, T, H>) -> Result<()> {
    let root = required(save_file)?;
    if get(root, "pouches").is_none() {
        return Ok(());
    }
    let data = get(root, "pouches");
    array(data)?;
    env.host.parse_pouches(data.unwrap())
}
pub fn parse_poh_storable<T, H: Host>(
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<()> {
    let Some(root) = save_file else {
        return Ok(());
    };
    match get(root, "pohstorage") {
        None | Some(Json::Null) => Ok(()),
        raw => env.host.parse_poh_storage(object(raw)?),
    }
}
/// PlayerSaveParser.parseData.
pub fn parse_data<T, H>(
    profile: &mut Profile<'_>,
    save_file: Option<&Json>,
    env: &mut Env<'_, T, H>,
) -> Result<Loaded>
where
    T: timers::Context<CoreData>,
    H: Host + skills::Context + crate::player::settings::Context,
{
    parse_version(profile, save_file)?;
    parse_core(profile, save_file, env)?;
    let legacy_attributes = parse_attributes(profile, save_file, env)?;
    parse_skills(profile, save_file, env)?;
    parse_settings(profile, save_file, env)?;
    parse_quests(save_file, env)?;
    parse_appearance(profile, save_file)?;
    parse_grave(profile, save_file, env)?;
    parse_varps(profile, save_file)?;
    parse_spellbook(profile, save_file)?;
    parse_saved_data(profile, save_file)?;
    parse_autocast_spell(save_file, env)?;
    // parseFarming and parseMonitor have empty bodies.
    parse_configs(profile, save_file)?;
    parse_music(save_file, env)?;
    parse_familiars(save_file, env)?;
    parse_bank_pin(profile, save_file)?;
    parse_house(save_file, env)?;
    parse_ironman(profile, save_file)?;
    parse_emote_manager(save_file, env)?;
    parse_statistics(save_file)?;
    parse_achievements(profile, save_file, env)?;
    parse_pouches(save_file, env)?;
    parse_poh_storable(save_file, env)?;
    Ok(Loaded { legacy_attributes })
}
impl Parser {
    /// PlayerSaveParser.parse. `save_path` is ServerConstants.PLAYER_SAVE_PATH,
    /// used as a plain string prefix. Bytes are read as the JDK reads UTF-8,
    /// replacing malformed input. The parsed object stays in `save_file` when a later
    /// section fails, as it does on the original instance.
    pub fn parse<T, H>(
        &mut self,
        profile: &mut Profile<'_>,
        env: &mut Env<'_, T, H>,
        save_path: &str,
    ) -> Result<Option<Loaded>>
    where
        T: timers::Context<CoreData>,
        H: Host + skills::Context + crate::player::settings::Context,
    {
        let path = format!("{save_path}{}.json", profile.name);
        if !std::path::Path::new(&path).exists() {
            env.host.log(
                Level::Warn,
                &format!(
                    "Couldn't find save file for {}, or save is corrupted.",
                    profile.name
                ),
            );
            self.read = false;
            return Ok(None);
        }
        let bytes = std::fs::read(&path).map_err(|error| Error::Io(error.kind()))?;
        self.parse_text(profile, env, &decode_utf8(&bytes))
            .map(Some)
    }
    /// The part of parse after the reader exists: parse, cast, parseData.
    pub fn parse_text<T, H>(
        &mut self,
        profile: &mut Profile<'_>,
        env: &mut Env<'_, T, H>,
        text: &str,
    ) -> Result<Loaded>
    where
        T: timers::Context<CoreData>,
        H: Host + skills::Context + crate::player::settings::Context,
    {
        self.save_file = Some(parse_text(text)?);
        parse_data(profile, self.save_file.as_ref(), env)
    }
    /// PlayerSaveParser.runContentHooks. `saveFile!!` is evaluated per hook,
    /// so a parser that never loaded a file only fails when hooks exist.
    pub fn run_content_hooks<T, H: Host>(
        &self,
        profile: &mut Profile<'_>,
        env: &mut Env<'_, T, H>,
    ) -> Result<()> {
        if self.read {
            for hook in 0..env.host.content_hooks() {
                let root = self.save_file.as_ref().ok_or(Error::Null)?;
                env.host.parse_content(hook, root)?;
            }
        }
        *profile.save_parsed = true;
        Ok(())
    }
}
