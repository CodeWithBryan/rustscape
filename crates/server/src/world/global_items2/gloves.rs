//! The brawling gloves (`BrawlingGloves`, `BrawlingGlovesManager`) and the
//! Fist of Guthix gloves (`FOGGlovesListener`, `FOGGlovesManager`).
use super::tables;
use super::{
    Container, Error, Host, ItemArg, Object, Result, Store, Value, as_item, colorize,
    item_from_equipment, parse_int,
};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------- FOGGlovesListener, FOGGlovesManager

/// FOGGlovesListener's `inspect`: the name and the charge capped at the
/// glove's maximum.
pub fn fog_inspect<H: Host + ?Sized>(h: &mut H, player: &H::Node, node: &H::Node) -> Result<bool> {
    let name = h.name(node)?;
    let charge = h.charge(as_item(h, node)?)?;
    let max = fog_max(h.id(node)?)?;
    h.send_message(
        player,
        &format!(
            "{}: {} charge left.",
            name.as_deref().unwrap_or("null"),
            charge.min(max)
        ),
    )?;
    Ok(true)
}

fn fog_max(id: i32) -> Result<i32> {
    usize::try_from(id.wrapping_sub(tables::FOG_FIRST))
        .ok()
        .and_then(|i| tables::FOG_MAX_CHARGES.get(i).copied())
        .ok_or(Error::Index)
}

/// `FOGGlovesManager.updateCharges(player, charges)`: the hands item's
/// charge is first capped at its maximum; gloves that run out crumble and
/// answer the charge they had. The charges used.
pub fn fog_update<H: Host + ?Sized>(h: &mut H, player: &H::Node, charges: i32) -> Result<i32> {
    let Some(gloves) = item_from_equipment(h, player, tables::HANDS)? else {
        return Ok(0);
    };
    let charge = h.charge(&gloves)?;
    let max = fog_max(h.id(&gloves)?)?;
    h.set_charge(&gloves, charge.min(max))?;
    if h.charge(&gloves)?.wrapping_sub(charges) <= 0 {
        h.remove_item(player, ItemArg::Item(gloves.clone()), Store::Equipment)?;
        h.send_message(
            player,
            &colorize("%RThe charges in your gloves have been used up and they crumble to dust."),
        )?;
        return h.charge(&gloves);
    }
    let charge = h.charge(&gloves)?;
    h.set_charge(&gloves, charge.wrapping_sub(charges))?;
    Ok(charges)
}

// ---------------------------------------------------------------------- BrawlingGloves

/// `BrawlingGloves`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrawlingGloves {
    Melee,
    Ranged,
    Magic,
    Prayer,
    Agility,
    Woodcutting,
    Firemaking,
    Mining,
    Hunter,
    Thieving,
    Smithing,
    Fishing,
    Cooking,
}
impl BrawlingGloves {
    pub const ALL: [Self; 13] = [
        Self::Melee,
        Self::Ranged,
        Self::Magic,
        Self::Prayer,
        Self::Agility,
        Self::Woodcutting,
        Self::Firemaking,
        Self::Mining,
        Self::Hunter,
        Self::Thieving,
        Self::Smithing,
        Self::Fishing,
        Self::Cooking,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Melee => "MELEE",
            Self::Ranged => "RANGED",
            Self::Magic => "MAGIC",
            Self::Prayer => "PRAYER",
            Self::Agility => "AGILITY",
            Self::Woodcutting => "WOODCUTTING",
            Self::Firemaking => "FIREMAKING",
            Self::Mining => "MINING",
            Self::Hunter => "HUNTER",
            Self::Thieving => "THIEVING",
            Self::Smithing => "SMITHING",
            Self::Fishing => "FISHING",
            Self::Cooking => "COOKING",
        }
    }
    fn index(self) -> i32 {
        Self::ALL.iter().position(|g| *g == self).unwrap_or(0) as i32
    }
    /// The gloves' item id (13845 onwards).
    pub fn id(self) -> i32 {
        13845 + self.index()
    }
    /// Their charges when first registered.
    pub fn charges(self) -> i32 {
        if self.index() < 3 { 500 } else { 450 }
    }
    /// The indicator saved for them (1 onwards).
    pub fn indicator(self) -> i32 {
        self.index() + 1
    }
    /// The skill they train (`Skills` ids).
    pub fn skill_slot(self) -> i32 {
        match self {
            Self::Melee => 0,
            Self::Ranged => 4,
            Self::Magic => 6,
            Self::Prayer => 5,
            Self::Agility => 16,
            Self::Woodcutting => 8,
            Self::Firemaking => 11,
            Self::Mining => 14,
            Self::Hunter => 21,
            Self::Thieving => 17,
            Self::Smithing => 13,
            Self::Fishing => 10,
            Self::Cooking => 7,
        }
    }
    /// `forId(id)`.
    pub fn for_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.id() == id)
    }
    /// `forIndicator(indicator)`.
    pub fn for_indicator(indicator: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.indicator() == indicator)
    }
    /// `forSkill(skill)`.
    pub fn for_skill(skill: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.skill_slot() == skill)
    }
}

// ---------------------------------------------------------------------- BrawlingGlovesManager

/// A JDK `HashMap<Integer, Integer>`: its entries in insertion order and the
/// table size it has grown to (a removal never shrinks it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JavaIntMap {
    entries: Vec<(i32, i32)>,
    capacity: usize,
}
impl JavaIntMap {
    pub fn get(&self, key: i32) -> Option<i32> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn put_if_absent(&mut self, key: i32, value: i32) {
        if self.get(key).is_none() {
            self.entries.push((key, value));
            if self.capacity == 0 {
                self.capacity = 16;
            }
            while self.entries.len() > self.capacity * 3 / 4 {
                self.capacity *= 2;
            }
        }
    }
    pub fn replace(&mut self, key: i32, value: i32) {
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        }
    }
    pub fn remove(&mut self, key: i32) {
        self.entries.retain(|(k, _)| *k != key);
    }
    /// The entries in the HashMap's walk order.
    pub fn walk(&self) -> Vec<(i32, i32)> {
        let keys: Vec<i32> = self.entries.iter().map(|(k, _)| *k).collect();
        tables::java_bucket_order(&keys, self.capacity.max(16))
            .into_iter()
            .map(|k| (k, self.get(k).unwrap_or(0)))
            .collect()
    }
}

/// A `BrawlingGlovesManager`: its player (null for the default instance
/// `getInstance` builds) and `GloveCharges`.
#[derive(Clone, Debug, PartialEq)]
pub struct BrawlingGlovesManager<N> {
    pub player: Option<N>,
    pub glove_charges: JavaIntMap,
}

/// A manager object as attributes hold it; `==` is identity.
#[derive(Clone, Debug)]
pub struct ManagerRef<N>(pub Arc<Mutex<BrawlingGlovesManager<N>>>);
impl<N> PartialEq for ManagerRef<N> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<N: Clone> ManagerRef<N> {
    pub fn new(player: Option<N>) -> Self {
        Self(Arc::new(Mutex::new(BrawlingGlovesManager {
            player,
            glove_charges: JavaIntMap::default(),
        })))
    }
    /// The object's identity.
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    fn with<T>(&self, f: impl FnOnce(&mut BrawlingGlovesManager<N>) -> T) -> T {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut guard)
    }
    /// A copy of the state (no lock is held afterwards).
    pub fn snapshot(&self) -> BrawlingGlovesManager<N> {
        self.with(|m| m.clone())
    }
}

/// The attribute the manager lives in.
pub const MANAGER_ATTRIBUTE: &str = "bg-manager";

/// `login(player)`: a new manager for the player goes into its attribute.
pub fn login<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<()> {
    let manager = ManagerRef::new(Some(player.clone()));
    h.set_attribute(
        player,
        MANAGER_ATTRIBUTE,
        Value::Object(Object::GloveManager(manager)),
    )
}

/// `getInstance(player)`: the attribute's manager, or a new default one
/// (built before the attribute is read, never stored).
pub fn instance<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<ManagerRef<H::Node>> {
    let default = ManagerRef::new(None);
    match h.get_attribute(player, MANAGER_ATTRIBUTE)? {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Object(Object::GloveManager(m))) => Ok(m),
        Some(_) => Err(Error::Cast),
    }
}

/// `registerGlove(id, charges)`: `putIfAbsent`.
pub fn register_with<N: Clone>(manager: &ManagerRef<N>, id: i32, charges: i32) {
    manager.with(|m| m.glove_charges.put_if_absent(id, charges));
}

/// `registerGlove(id)`: an unknown id fails `requireNonNull`, which the
/// method catches and prints.
pub fn register<H: Host + ?Sized>(h: &mut H, manager: &ManagerRef<H::Node>, id: i32) {
    match BrawlingGloves::for_id(id) {
        Some(gloves) => register_with(manager, id, gloves.charges()),
        None => h.stack_trace("java.lang.NullPointerException"),
    }
}

/// `updateCharges(glove, charges)`: the last charges remove the gloves from
/// the equipment; otherwise the count goes down (the message every fifty
/// counts the charges before this use, less one).
pub fn update_charges<H: Host + ?Sized>(
    h: &mut H,
    manager: &ManagerRef<H::Node>,
    glove: i32,
    charges: i32,
) -> Result<()> {
    let Some(current) = manager.with(|m| m.glove_charges.get(glove)) else {
        return Ok(());
    };
    let player = manager.with(|m| m.player.clone());
    if current.wrapping_sub(charges) <= 0 {
        manager.with(|m| m.glove_charges.remove(glove));
        let player = player.ok_or(Error::Null)?;
        let item = h.new_item(glove, 1)?;
        h.container_remove(&player, Container::Equipment, Some(&item))?;
        let name = h.item_name(glove);
        h.send_message(
            &player,
            &format!(
                "<col=ff0000>You use the last charge of your {} and they vanish.</col>",
                name.as_deref().unwrap_or("null")
            ),
        )?;
        return Ok(());
    }
    manager.with(|m| {
        m.glove_charges
            .replace(glove, current.wrapping_sub(charges))
    });
    let player = player.ok_or(Error::Null)?;
    h.debug(
        &player,
        &format!("Glove charges: {}", current.wrapping_sub(1)),
    )?;
    if current.wrapping_sub(1) % 50 == 0 {
        let name = h.item_name(glove);
        let left = manager.with(|m| m.glove_charges.get(glove));
        h.send_message(
            &player,
            &format!(
                "<col=1fbd0d>Your {} have {} charges left.</col>",
                name.as_deref().unwrap_or("null"),
                left.map_or("null".to_owned(), |l| l.to_string())
            ),
        )?;
    }
    Ok(())
}

/// `getExperienceBonus()`: 3.0 in the wilderness, 0.5 elsewhere.
pub fn experience_bonus<H: Host + ?Sized>(h: &mut H, manager: &ManagerRef<H::Node>) -> Result<f64> {
    let player = manager.with(|m| m.player.clone()).ok_or(Error::Null)?;
    Ok(if h.wilderness_level(&player)? > 0 {
        3.0
    } else {
        0.5
    })
}

/// `java.util.Objects.toString` of a json-simple value as the save parser
/// reads it (`glove.get(key).toString()`); a missing key is null.
fn json_text(value: Option<&serde_json::Value>) -> Result<String> {
    match value.ok_or(Error::Null)? {
        serde_json::Value::Null => Err(Error::Null),
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        other => Ok(other.to_string()),
    }
}

/// `parsePlayer(player, data)`: each saved glove is registered (an unknown
/// indicator fails at `getId`).
pub fn parse_player<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    data: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let manager = instance(h, player)?;
    if let Some(saved) = data.get("brawlingGloves") {
        let saved = match saved {
            serde_json::Value::Null => return Err(Error::Null),
            other => other.as_array().ok_or(Error::Cast)?,
        };
        for glove in saved {
            let glove = match glove {
                serde_json::Value::Null => return Err(Error::Null),
                other => other.as_object().ok_or(Error::Cast)?,
            };
            let indicator = parse_int(&json_text(glove.get("gloveId"))?)?;
            let id = BrawlingGloves::for_indicator(indicator)
                .ok_or(Error::Null)?
                .id();
            let charges = parse_int(&json_text(glove.get("charges"))?)?;
            register_with(&manager, id, charges);
        }
    }
    Ok(())
}

/// `savePlayer(player, save)`: the gloves in GloveCharges' walk order, as
/// strings, when there are any.
pub fn save_player<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    save: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    if instance(h, player)?.with(|m| m.glove_charges.is_empty()) {
        return Ok(());
    }
    let entries = instance(h, player)?.with(|m| m.glove_charges.walk());
    let mut gloves = Vec::new();
    for (id, charges) in entries {
        let indicator = BrawlingGloves::for_id(id).ok_or(Error::Null)?.indicator();
        let mut glove = serde_json::Map::new();
        glove.insert(
            "gloveId".into(),
            serde_json::Value::String(indicator.to_string()),
        );
        glove.insert(
            "charges".into(),
            serde_json::Value::String(charges.to_string()),
        );
        gloves.push(serde_json::Value::Object(glove));
    }
    save.insert("brawlingGloves".into(), serde_json::Value::Array(gloves));
    Ok(())
}
