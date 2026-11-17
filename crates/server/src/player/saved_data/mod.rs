//! Activity/quest/global character records and their original JSON save sections.
mod json;
mod records;
pub use records::{ActivityData, GlobalData, QuestData};
use serde::Serialize;
use serde_json::Value;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
/// The ID/amount fields consumed by these save sections. Definitions and item
/// interaction are separate pending systems; no item gameplay is implied.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemData {
    id_hash: i32,
    pub amount: i32,
}
impl ItemData {
    pub fn new(id: i32, amount: i32) -> Self {
        Self {
            id_hash: id.wrapping_shl(16) | 1000,
            amount,
        }
    }
    pub fn id(&self) -> i32 {
        (self.id_hash >> 16) & 65535
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct SavedData {
    pub activity: ActivityData,
    pub quest: QuestData,
    pub global: GlobalData,
}
impl SavedData {
    /// PlayerSaveParser.parseSavedData first obtains all three JSONObject values,
    /// then mutates activity, quest and global in that order.
    pub fn parse_sections(&mut self, root: &Value) -> Result<()> {
        let root = json::object(root)?;
        let activity = json::section(root.get("activityData"))?;
        let quest = json::section(root.get("questData"))?;
        let global = json::section(root.get("globalData"))?;
        self.activity.parse(activity)?;
        self.quest.parse(quest)?;
        self.global.parse(global)
    }
    /// PlayerSaver.saveSavedData publishes each successful section before the next.
    pub fn save_sections(&self, root: &mut serde_json::Map<String, Value>) -> Result<()> {
        root.insert("activityData".into(), self.activity.save()?);
        root.insert("questData".into(), self.quest.save()?);
        root.insert("globalData".into(), self.global.save()?);
        Ok(())
    }
}
impl ActivityData {
    pub fn increase_pest_points(&mut self, amount: i32) {
        self.pest_points = self.pest_points.wrapping_add(amount).min(500);
    }
    pub fn decrease_pest_points(&mut self, amount: i32) {
        self.pest_points = self.pest_points.wrapping_sub(amount);
    }
    pub fn update_warrior_tokens(&mut self, amount: i32) {
        self.warrior_guild_tokens = self.warrior_guild_tokens.wrapping_add(amount);
    }
    pub fn update_bounty_hunter_rate(&mut self, amount: i32) {
        self.bounty_hunter_rate = self.bounty_hunter_rate.wrapping_add(amount);
    }
    pub fn update_bounty_rogue_rate(&mut self, amount: i32) {
        self.bounty_rogue_rate = self.bounty_rogue_rate.wrapping_add(amount);
    }
    pub fn set_barrow_kills(&mut self, value: i32) {
        self.barrow_kills = value.min(10000);
    }
    pub fn has_started_kolodion(&self) -> bool {
        self.kolodion_stage == 1
    }
    pub fn has_killed_kolodion(&self) -> bool {
        self.kolodion_stage >= 2
    }
    pub fn has_received_kolodion_reward(&self) -> bool {
        self.kolodion_stage == 3
    }
    pub fn has_killed_bork(&self) -> bool {
        self.last_bork_battle > 0
    }
    pub fn increment_pizazz(&mut self, index: i32, amount: i32) -> Result<()> {
        let value = self
            .pizazz_points
            .get_mut(index as usize)
            .ok_or(Error::Index)?;
        *value = value.wrapping_add(amount);
        Ok(())
    }
    pub fn decrement_pizazz(&mut self, index: i32, amount: i32) -> Result<()> {
        let value = self
            .pizazz_points
            .get_mut(index as usize)
            .ok_or(Error::Index)?;
        *value = value.wrapping_sub(amount);
        Ok(())
    }
}
/// Original quest helpers compare Java string identity to literals. Ordinary Rust
/// callers should use `Literal`; external reconstructed strings use `Text`.
pub enum QuestName<'a> {
    Literal(&'a str),
    Text(&'a str),
}
impl QuestName<'_> {
    fn literal(&self) -> Option<&str> {
        match self {
            Self::Literal(name) => Some(name),
            Self::Text(_) => None,
        }
    }
}
impl QuestData {
    pub fn dragon_slayer_item(&self, name: QuestName<'_>) -> bool {
        self.dragon_slayer[match name.literal() {
            Some("wizard") => 3,
            Some("silk") => 2,
            Some("bowl") => 1,
            _ => 0,
        }]
    }
    pub fn dragon_slayer_attribute(&self, name: QuestName<'_>) -> bool {
        self.dragon_slayer[Self::dragon_attribute_index(name)]
    }
    fn dragon_attribute_index(name: QuestName<'_>) -> usize {
        match name.literal() {
            Some("ship") => 4,
            Some("memorized") => 5,
            Some("repaired") => 6,
            Some("ned") => 7,
            _ => 8,
        }
    }
    pub fn set_dragon_slayer_attribute(&mut self, name: QuestName<'_>, value: bool) {
        self.dragon_slayer[Self::dragon_attribute_index(name)] = value;
    }
    fn cook_index(name: QuestName<'_>) -> usize {
        match name.literal() {
            Some("milk") => 0,
            Some("egg") => 1,
            Some("flour") => 2,
            _ => 3,
        }
    }
    pub fn cook_assist(&self, name: QuestName<'_>) -> bool {
        self.cooks_assistant[Self::cook_index(name)]
    }
    pub fn set_cooks_assistant(&mut self, name: QuestName<'_>, value: bool) {
        self.cooks_assistant[Self::cook_index(name)] = value;
    }
    pub fn desert_treasure_item(&self, index: i32) -> Result<Option<&ItemData>> {
        self.desert_treasure
            .get(index as usize)
            .map(Option::as_ref)
            .ok_or(Error::Index)
    }
    pub fn set_desert_treasure_item(&mut self, index: i32, item: Option<ItemData>) -> Result<()> {
        *self
            .desert_treasure
            .get_mut(index as usize)
            .ok_or(Error::Index)? = item;
        Ok(())
    }
}
impl GlobalData {
    pub fn set_saved_location(&mut self, x: i32, y: i32, z: i32) {
        self.saved_x = x;
        self.saved_y = y;
        self.saved_h = z;
    }
    pub fn has_stronghold_reward(&self, reward: i32) -> Result<bool> {
        self.strong_hold_rewards
            .get(reward.wrapping_sub(1) as usize)
            .copied()
            .ok_or(Error::Index)
    }
    pub fn has_read_plaques(&self) -> bool {
        self.read_plaques.iter().all(|&read| read)
    }
    pub fn reset_abyss(&mut self) {
        self.abyss_data.fill(false);
    }
    pub fn rc_decay(&self, index: i32) -> Result<i32> {
        if index < 0 {
            Ok(0)
        } else {
            self.rc_decays
                .get(index as usize)
                .copied()
                .ok_or(Error::Index)
        }
    }
    pub fn has_double_exp(&self, now: i64) -> bool {
        self.double_exp > now
    }
}
