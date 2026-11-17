//! StorageState: the POH costume room and bookcase storage, one container of
//! item ids per StorableFamily. The storage interface and options are not
//! ported.
use crate::player::save::{Error, Result};
use rs09_game_core::java_json::Json;

/// StorableFamily, in declaration order.
pub const FAMILIES: [&str; 10] = [
    "FANCY_DRESS",
    "ARMOUR_CASE",
    "MAGIC_WARDROBE",
    "TREASURE_CHEST_LOW",
    "TREASURE_CHEST_MED",
    "TREASURE_CHEST_HIGH",
    "BOOKCASE",
    "CAPE_RACK",
    "CAPE_RACK_SKILL",
    "TOY_BOX",
];

/// StorageContainer: `stored` is a LinkedHashSet, so ids keep their first
/// insertion position.
#[derive(Clone, Debug, Default)]
pub struct Container {
    pub stored: Vec<i32>,
    pub current_page: i32,
}
impl Container {
    pub fn add_item(&mut self, id: i32) {
        if !self.stored.contains(&id) {
            self.stored.push(id);
        }
    }
    pub fn withdraw(&mut self, id: i32) -> bool {
        match self.stored.iter().position(|v| *v == id) {
            Some(at) => {
                self.stored.remove(at);
                true
            }
            None => false,
        }
    }
    pub fn contains(&self, id: i32) -> bool {
        self.stored.contains(&id)
    }
    pub fn next_page(&mut self, total: i32, page_size: i32) {
        let next = self.current_page.wrapping_add(1);
        if next.wrapping_mul(page_size) < total {
            self.current_page = next;
        }
    }
    pub fn prev_page(&mut self) {
        if self.current_page > 0 {
            self.current_page -= 1;
        }
    }
    pub fn clear(&mut self) {
        self.stored.clear();
        self.current_page = 0;
    }
    /// Ids are written as JSON numbers, not strings.
    pub fn to_json(&self) -> Json {
        let mut json = Json::object();
        let items = self
            .stored
            .iter()
            .map(|id| Json::Int(i64::from(*id)))
            .collect();
        json.put("items", Json::Array(items)).unwrap();
        json
    }
    /// readJson: the old ids go first; without an `items` array the page is
    /// left as it was. Each id is `(id as Number).toInt()`, which wraps a long
    /// and truncates a double.
    pub fn read_json(&mut self, json: &Json) -> Result<()> {
        self.stored.clear();
        let Some(Json::Array(items)) = json.get("items") else {
            return Ok(());
        };
        for id in items {
            let id = match id {
                Json::Null => return Err(Error::Null),
                Json::Int(value) => *value as i32,
                Json::Double(value) => *value as i32,
                _ => return Err(Error::Type),
            };
            self.add_item(id);
        }
        self.current_page = 0;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct State {
    pub containers: Vec<Container>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            containers: vec![Container::default(); FAMILIES.len()],
        }
    }
}
impl State {
    /// toJson: family names lower-cased (ROOT, ASCII) as keys.
    pub fn to_json(&self) -> Json {
        let mut data = Json::object();
        for (family, container) in FAMILIES.iter().zip(&self.containers) {
            data.put(family.to_ascii_lowercase(), container.to_json())
                .unwrap();
        }
        let mut root = Json::object();
        root.put("containers", data).unwrap();
        root
    }
    /// readJson: anything but an object under `containers`, or under a
    /// family's name, is skipped.
    pub fn read_json(&mut self, data: &Json) -> Result<()> {
        let Some(containers @ Json::Object(_)) = data.get("containers") else {
            return Ok(());
        };
        for (family, container) in FAMILIES.iter().zip(&mut self.containers) {
            if let Some(json @ Json::Object(_)) = containers.get(&family.to_ascii_lowercase()) {
                container.read_json(json)?;
            }
        }
        Ok(())
    }
    pub fn clear(&mut self) {
        self.containers.iter_mut().for_each(Container::clear);
    }
}
