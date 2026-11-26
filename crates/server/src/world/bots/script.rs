//! `Script.java`: the base of every bot script. The script classes
//! themselves (their `tick()` and `newInstance()`) are content, reached
//! through [`Host::script_tick`] and [`Host::script_new_instance`].
use super::{Error, Host, Item, JavaHashMap, Result, ScriptApi, Which, loc};
use crate::definitions::item::Shared;

/// The fields of a `Script` object.
pub struct ScriptState<H: Host> {
    /// The content class of the object.
    pub class: H::ScriptClass,
    /// `scriptAPI`, set by `init`.
    pub script_api: Option<ScriptApi<H>>,
    /// `inventory`.
    pub inventory: Vec<Item>,
    /// `equipment`.
    pub equipment: Vec<Item>,
    /// `skills`: a `HashMap<Integer, Integer>`, walked in its order.
    pub skills: JavaHashMap<i32, i32>,
    /// `quests`.
    pub quests: Vec<H::Quest>,
    /// `bot`.
    pub bot: Option<H::Node>,
    /// `running`.
    pub running: bool,
    /// `endDialogue`.
    pub end_dialogue: bool,
}

/// A `Script` object: a shared handle.
pub struct Script<H: Host>(Shared<ScriptState<H>>);
impl<H: Host> Clone for Script<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<H: Host> Script<H> {
    /// A new script object of a content class, with the field initialisers
    /// of `Script`.
    pub fn new(class: H::ScriptClass) -> Self {
        Self(Shared::new(ScriptState {
            class,
            script_api: None,
            inventory: Vec::new(),
            equipment: Vec::new(),
            skills: JavaHashMap::new(),
            quests: Vec::new(),
            bot: None,
            running: true,
            end_dialogue: true,
        }))
    }
    pub fn read<T>(&self, f: impl FnOnce(&ScriptState<H>) -> T) -> T {
        f(&self.0.read())
    }
    pub fn write<T>(&self, f: impl FnOnce(&mut ScriptState<H>) -> T) -> T {
        f(&mut self.0.write())
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn class(&self) -> H::ScriptClass {
        self.0.read().class.clone()
    }
    pub fn bot(&self) -> Option<H::Node> {
        self.0.read().bot.clone()
    }
    pub fn running(&self) -> bool {
        self.0.read().running
    }
    pub fn end_dialogue(&self) -> bool {
        self.0.read().end_dialogue
    }
    pub fn script_api(&self) -> Option<ScriptApi<H>> {
        self.0.read().script_api.clone()
    }

    /// `init(isPlayer)`: a new `ScriptAPI(bot)` (a null bot fails there);
    /// for a bot, levels in the map's order, quests at stage 100, the
    /// equipment, then the inventory emptied and filled.
    pub fn init(&self, host: &mut H, is_player: bool) -> Result<()> {
        let bot = self.bot().ok_or(Error::Null)?;
        self.write(|s| s.script_api = Some(ScriptApi::new(bot.clone())));
        if !is_player {
            let skills: Vec<(i32, i32)> =
                self.read(|s| s.skills.iter().map(|(k, v)| (*k, *v)).collect());
            for (skill, level) in skills {
                self.set_level(host, skill, level)?;
            }
            for quest in self.read(|s| s.quests.clone()) {
                let bot = self.bot().ok_or(Error::Null)?;
                let object = host.quest(&bot, &quest)?;
                host.quest_set_stage(&bot, object.as_ref(), 100)?;
            }
            for item in self.read(|s| s.equipment.clone()) {
                let bot = self.bot().ok_or(Error::Null)?;
                host.equipment_add(&bot, item, true, false)?;
            }
            let bot = self.bot().ok_or(Error::Null)?;
            host.container_clear(&bot, Which::Inventory)?;
            for item in self.read(|s| s.inventory.clone()) {
                let bot = self.bot().ok_or(Error::Null)?;
                host.container_add(&bot, Which::Inventory, item)?;
            }
        }
        Ok(())
    }

    /// `toString()`.
    pub fn text(&self, host: &mut H) -> Result<String> {
        let bot = self.bot().ok_or(Error::Null)?;
        let name = host.name(&bot).unwrap_or_else(|| "null".to_owned());
        let class = host.script_simple_name(&self.class());
        let location = loc(host, &bot)?;
        let current = host.pulse_manager_current(&bot)?;
        let pulse = host.pulse_text(current.as_ref())?;
        Ok(format!(
            "{name} is a {class} at location {location} Current pulse: {pulse}"
        ))
    }

    /// `setLevel(skill, level)`.
    pub fn set_level(&self, host: &mut H, skill: i32, level: i32) -> Result<()> {
        let bot = self.bot().ok_or(Error::Null)?;
        host.skills_set_level(&bot, skill, level)?;
        host.skills_set_static_level(&bot, skill, level)?;
        host.skills_update_combat_level(&bot)?;
        host.appearance_sync(&bot)
    }
}
