//! `FamiliarManager`: a player's current familiar or pet, its pet details
//! (the persisted [`PetStore`]), summoning, renewing, pets (summon, morph,
//! feed, pick up), login, the save's parse (with the v0/v1 pet migrations)
//! and the static `FAMILIARS` registry ([`Registry`]).
use super::pets::Pet;
use super::{
    Details, Error, Familiar, Host, Item, Loc, LogLevel, Origin, PetDetails, PetStore, Result,
    SUMMONING,
};
use crate::player::persisted::{java_array, java_object, java_text, parse_int};
use crate::player::save::ActiveFamiliar;
use rs09_game_core::java_json::{Json, key_order};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// The varbits packed into varp 1160 that `login` writes.
pub const VARBIT_SUMMONING_ORB_VISIBILITY: i32 = 4280;
pub const VARBIT_SUMMONING_UNKNOWN1: i32 = 4281;
pub const VARBIT_SUMMONING_UNKNOWN2: i32 = 4282;
/// `Items.BUCKET_OF_MILK_1927`, `Items.BUCKET_1925`.
pub const BUCKET_OF_MILK: i32 = 1927;
pub const BUCKET: i32 = 1925;

/// `FamiliarManager.FAMILIARS`: each NPC id's prototype familiar. Clones
/// share the one map; it is only read with get/containsKey/put, so its
/// iteration order is never observed.
pub struct Registry<H: Host>(Arc<Mutex<HashMap<i32, Familiar<H>>>>);

impl<H: Host> Clone for Registry<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> Default for Registry<H> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

impl<H: Host> Registry<H> {
    fn lock(&self) -> MutexGuard<'_, HashMap<i32, Familiar<H>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// `FAMILIARS.get(id)`.
    pub fn get(&self, id: i32) -> Option<Familiar<H>> {
        self.lock().get(&id).cloned()
    }
    /// The registered ids, sorted (the map is never iterated).
    pub fn ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.lock().keys().copied().collect();
        ids.sort_unstable();
        ids
    }
    /// `Familiar.newInstance(arg)` on `prototype`: each of its ids in turn
    /// (`getIds()` read once) is put with a `configureFamiliar()` after
    /// each; the first id already registered logs and answers null, leaving
    /// the earlier ids registered.
    pub fn new_instance(
        &self,
        host: &mut H,
        prototype: &Familiar<H>,
    ) -> Result<Option<Familiar<H>>> {
        for id in host.ids(prototype)? {
            if self.lock().contains_key(&id) {
                host.log(
                    Origin::Familiar(prototype),
                    LogLevel::Err,
                    &format!("Familiar {id} was already registered!"),
                )?;
                return Ok(None);
            }
            self.lock().insert(id, prototype.clone());
            host.configure(prototype)?;
        }
        Ok(Some(prototype.clone()))
    }
}

/// The fields of one `FamiliarManager`.
pub struct ManagerState<H: Host> {
    pub player: H::Node,
    pub pets: PetStore,
    pub familiar: Option<Familiar<H>>,
    pub summoning_combat_level: i32,
    pub has_pouch: bool,
}

/// One player's `FamiliarManager`; clones are the same manager.
pub struct Manager<H: Host>(Arc<Mutex<ManagerState<H>>>);

impl<H: Host> Clone for Manager<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> std::fmt::Debug for Manager<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FamiliarManager({:?})", self.player())
    }
}

/// The pieces of `parse` that fail the way the original's casts and
/// conversions do.
fn object(value: Option<&Json>) -> Result<Option<&Json>> {
    java_object(value).map_err(save_error)
}
fn array(value: Option<&Json>) -> Result<Option<&[Json]>> {
    java_array(value).map_err(save_error)
}
fn int_of(value: Option<&Json>) -> Result<i32> {
    parse_int(&java_text(value).map_err(save_error)?).map_err(save_error)
}
fn double_of(value: Option<&Json>) -> Result<f64> {
    let text = java_text(value).map_err(save_error)?;
    rs09_game_core::java_double::parse(&text).ok_or(Error::Number)
}
fn save_error(e: crate::player::save::Error) -> Error {
    use crate::player::save::Error as S;
    match e {
        S::Null => Error::Null,
        S::Type => Error::Cast,
        S::Number => Error::Number,
        other => Error::Binding(format!("{other:?}")),
    }
}
fn contains_key(object: &Json, key: &str) -> bool {
    matches!(object, Json::Object(entries) if entries.iter().any(|(k, _)| k == key))
}
/// `JSONObject.keySet()` in the HashMap's order.
fn keys(object: &Json) -> Vec<String> {
    match object {
        Json::Object(entries) => key_order(entries)
            .into_iter()
            .map(|i| entries[i].0.clone())
            .collect(),
        _ => Vec::new(),
    }
}

impl<H: Host> Manager<H> {
    /// `new FamiliarManager(player)`, its pet details the player's
    /// persisted store.
    pub fn new(player: H::Node, pets: PetStore) -> Self {
        Self(Arc::new(Mutex::new(ManagerState {
            player,
            pets,
            familiar: None,
            summoning_combat_level: 0,
            has_pouch: false,
        })))
    }
    fn lock(&self) -> MutexGuard<'_, ManagerState<H>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn player(&self) -> H::Node {
        self.lock().player.clone()
    }
    /// `getPetDetails()`.
    pub fn pet_details(&self) -> PetStore {
        self.lock().pets.clone()
    }
    /// `getFamiliar()`.
    pub fn familiar(&self) -> Option<Familiar<H>> {
        self.lock().familiar.clone()
    }
    /// `setFamiliar(familiar)`.
    pub fn set_familiar(&self, familiar: Option<Familiar<H>>) {
        self.lock().familiar = familiar;
    }
    /// `hasFamiliar()`.
    pub fn has_familiar(&self) -> bool {
        self.lock().familiar.is_some()
    }
    /// `hasPet()`.
    pub fn has_pet(&self) -> bool {
        self.familiar().is_some_and(|f| f.is_pet())
    }
    /// `isUsingSummoning()`.
    pub fn is_using_summoning(&self) -> bool {
        self.has_pouch() || (self.has_familiar() && !self.has_pet())
    }
    pub fn has_pouch(&self) -> bool {
        self.lock().has_pouch
    }
    pub fn set_has_pouch(&self, has_pouch: bool) {
        self.lock().has_pouch = has_pouch;
    }
    pub fn summoning_combat_level(&self) -> i32 {
        self.lock().summoning_combat_level
    }
    pub fn set_summoning_combat_level(&self, level: i32) {
        self.lock().summoning_combat_level = level;
    }

    /// `parse(familiarData)`.
    pub fn parse(&self, host: &mut H, data: &Json) -> Result<()> {
        let player = self.player();
        let store = self.pet_details();
        let mut current_pet = -1;
        if contains_key(data, "currentPet") {
            current_pet = int_of(data.get("currentPet"))?;
        }
        if host.version(&player)? < 2 {
            // Migrate the v1 format: a list for every stage of every pet first.
            store.with(|map| {
                for pet in super::pets::PETS {
                    for id in [pet.baby_item, pet.grown_item, pet.overgrown_item] {
                        if id != -1 {
                            map.put(id, Vec::new());
                        }
                    }
                }
            });
            let list = array(data.get("petDetails"))?.ok_or(Error::Null)?;
            for entry in list {
                let detail = object(Some(entry))?.ok_or(Error::Null)?;
                let details = PetDetails::new(0.0);
                details.update_hunger(double_of(detail.get("hunger"))?);
                details.update_growth(double_of(detail.get("growth"))?);
                let hash = int_of(detail.get("petId"))?;
                let item_id = if contains_key(detail, "stage") {
                    // v0: the baby item id and a stage.
                    let mut item_id = hash;
                    let stage = int_of(detail.get("stage"))?;
                    if stage > 0 {
                        let pets = Pet::for_id(hash);
                        item_id = pets.ok_or(Error::Null)?.next_stage_item_id(item_id);
                        if stage > 1 {
                            item_id = pets.ok_or(Error::Null)?.next_stage_item_id(item_id);
                        }
                    }
                    item_id
                } else {
                    hash >> 16 & 0xFFFF
                };
                store
                    .with(|map| map.get_mut(item_id).map(|l| l.push(details)))
                    .ok_or(Error::Null)?;
            }
            if current_pet > 65536 {
                current_pet = current_pet >> 16 & 0xFFFF;
            }
        } else {
            let details = object(data.get("petDetails"))?.ok_or(Error::Null)?;
            for key in keys(details) {
                let item_id = parse_int(&key).map_err(save_error)?;
                store.with(|map| map.put(item_id, Vec::new()));
                let values = array(details.get(&key))?.ok_or(Error::Null)?;
                for entry in values {
                    let detail = object(Some(entry))?.ok_or(Error::Null)?;
                    let d = PetDetails::new(0.0);
                    d.update_hunger(double_of(detail.get("hunger"))?);
                    d.update_growth(double_of(detail.get("growth"))?);
                    store
                        .with(|map| map.get_mut(item_id).map(|l| l.push(d)))
                        .ok_or(Error::Null)?;
                }
            }
        }
        if current_pet != -1 {
            let last = store
                .with(|map| map.get(current_pet).map(|l| l.last().cloned()))
                .ok_or(Error::Null)?;
            // Missing data in the save from a historical bug (GL !2077).
            let details = last.unwrap_or_else(|| PetDetails::new(0.0));
            let pets = Pet::for_id(current_pet).ok_or(Error::Null)?;
            let npc_id = pets.npc_id(host, current_pet)?;
            let class = host.pet_class();
            let pet = Familiar::new_pet(
                host,
                class,
                Some(player.clone()),
                details,
                current_pet,
                npc_id,
            )?;
            self.set_familiar(Some(pet));
        } else if contains_key(data, "familiar") {
            let current = object(data.get("familiar"))?.ok_or(Error::Null)?;
            let familiar_id = int_of(current.get("originalId"))?;
            let prototype = host.registry()?.get(familiar_id).ok_or(Error::Null)?;
            let familiar = host.construct(&prototype, &player, familiar_id)?;
            self.set_familiar(Some(familiar.clone()));
            familiar.set_ticks(int_of(current.get("ticks"))?);
            familiar.set_special_points(int_of(current.get("specialPoints"))?);
            if let Some(inventory) = current
                .get("inventory")
                .filter(|v| !matches!(v, Json::Null))
            {
                if !matches!(inventory, Json::Array(_)) {
                    return Err(Error::Cast);
                }
                let container = familiar.container()?;
                host.parse_container(&container, inventory)?;
            }
            let hp = int_of(current.get("lifepoints"))?;
            host.set_attribute_int(&familiar.npc(), "hp", hp)?;
        }
        Ok(())
    }

    /// `login()`.
    pub fn login(&self, host: &mut H) -> Result<()> {
        if let Some(familiar) = self.familiar() {
            familiar.init(host)?;
        }
        let player = self.player();
        let unlocked = host.wolf_whistle_complete(&player)?;
        host.set_varbit(
            &player,
            VARBIT_SUMMONING_ORB_VISIBILITY,
            if unlocked { 1 } else { 0 },
        )?;
        host.set_varbit(&player, VARBIT_SUMMONING_UNKNOWN1, 0)?;
        host.set_varbit(&player, VARBIT_SUMMONING_UNKNOWN2, 7)
    }

    /// `summon(item, pet, deleteItem)`.
    pub fn summon(&self, host: &mut H, item: Item, pet: bool, delete_item: bool) -> Result<()> {
        let player = self.player();
        let mut renew = false;
        if let Some(familiar) = self.familiar() {
            if familiar.pouch_id() == item.id {
                renew = true;
            } else {
                host.packet_message(&player, "You already have a follower.")?;
                return Ok(());
            }
        }
        if host.restricted_followers(&player)? && !host.is_locked(&player, "enable_summoning")? {
            host.packet_message(&player, "This is a Summoning-free area.")?;
            return Ok(());
        }
        if pet {
            self.summon_pet(host, item, delete_item, false, None, 0, -1.0, -1.0)?;
            return Ok(());
        }
        let Some(pouch) = host.pouch(item.id)? else {
            return Ok(());
        };
        if host.static_level(&player, SUMMONING)? < pouch.level_required {
            host.packet_message(
                &player,
                &format!(
                    "You need a Summoning level of {} to summon this familiar.",
                    pouch.level_required
                ),
            )?;
            return Ok(());
        }
        if host.skill_level(&player, SUMMONING)? < pouch.summon_cost {
            host.packet_message(
                &player,
                &format!(
                    "You need at least {} Summoning points to summon this familiar.",
                    pouch.summon_cost
                ),
            )?;
            return Ok(());
        }
        let npc_id = pouch.npc_id;
        let fam = if !renew {
            host.registry()?.get(npc_id)
        } else {
            self.familiar()
        };
        let Some(mut fam) = fam else {
            host.packet_message(
                &player,
                &format!("Invalid familiar {npc_id} - report on 2009Scape GitLab"),
            )?;
            return Ok(());
        };
        if !renew {
            fam = host.construct(&fam, &player, npc_id)?;
            if fam.spawn_location(host)?.is_none() {
                host.packet_message(
                    &player,
                    "The spirit in this pouch is too big to summon here. You will need to move to a larger",
                )?;
                host.packet_message(&player, "area.")?;
                return Ok(());
            }
        }
        let inventory = host.inventory(&player)?;
        if !host.remove(&inventory, &item, None)? {
            return Ok(());
        }
        host.update_level(&player, SUMMONING, pouch.summon_cost.wrapping_neg(), 0)?;
        host.add_experience(&player, SUMMONING, pouch.summon_experience, None)?;
        if !renew {
            self.set_familiar(Some(fam));
            self.spawn_familiar(host)?;
        } else {
            self.familiar().ok_or(Error::Null)?.refresh_timer();
        }
        host.appearance_sync(&player)
    }

    /// `morphPet(item, deleteItem, location, hunger, growth)`.
    pub fn morph_pet(
        &self,
        host: &mut H,
        item: Item,
        delete_item: bool,
        location: Option<Loc>,
        hunger: f64,
        growth: f64,
    ) -> Result<()> {
        let familiar = self.familiar().ok_or(Error::Null)?;
        let has_warned = familiar.has_warned()?;
        familiar.dismiss(host)?;
        self.summon_pet(
            host,
            item,
            delete_item,
            true,
            location,
            has_warned,
            hunger,
            growth,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn summon_pet(
        &self,
        host: &mut H,
        item: Item,
        delete_item: bool,
        morph: bool,
        location: Option<Loc>,
        has_warned: i32,
        hunger: f64,
        growth: f64,
    ) -> Result<bool> {
        let item_id = item.id;
        if item_id > 8850 && item_id < 8900 {
            return Ok(false);
        }
        let Some(pets) = Pet::for_id(item_id) else {
            return Ok(false);
        };
        let player = self.player();
        if host.static_level(&player, SUMMONING)? < pets.summoning_level {
            host.send_dialogue(
                &player,
                &format!(
                    "You need a summoning level of {} to summon this.",
                    pets.summoning_level
                ),
            )?;
            return Ok(false);
        }
        let store = self.pet_details();
        let details = store.with(|map| {
            if !map.contains_key(item_id) {
                map.put(item_id, Vec::new());
            }
            let list = map.get_mut(item_id).expect("put above");
            if list.is_empty() {
                // A new pet.
                list.push(PetDetails::new(if pets.growth_rate == 0.0 {
                    100.0
                } else {
                    0.0
                }));
            }
            list.last().cloned().expect("pushed above")
        });
        let npc_id = pets.npc_id(host, item_id)?;
        if npc_id > 0 {
            let class = host.pet_class();
            let pet =
                Familiar::new_pet(host, class, Some(player.clone()), details, item_id, npc_id)?;
            self.set_familiar(Some(pet.clone()));
            pet.set_has_warned(has_warned)?;
            if hunger != -1.0 {
                pet.pet_details()?.set_hunger(hunger);
            }
            if growth != -1.0 {
                pet.pet_details()?.set_growth(growth);
            }
            if delete_item {
                host.animate(&player, 827)?;
                let inventory = host.inventory(&player)?;
                if !host.remove(&inventory, &item, Some(true))? {
                    return Ok(false);
                }
            }
            if morph {
                self.morph_familiar(host, location)?;
            } else {
                self.spawn_familiar(host)?;
            }
            return Ok(true);
        }
        Ok(true)
    }

    /// `morphFamiliar(location)`.
    pub fn morph_familiar(&self, host: &mut H, location: Option<Loc>) -> Result<()> {
        self.familiar()
            .ok_or(Error::Null)?
            .init_at(host, location, false)?;
        let player = self.player();
        host.open_tab(&player, 662)?;
        host.set_viewed_tab(&player, 7)
    }

    /// `spawnFamiliar()`.
    pub fn spawn_familiar(&self, host: &mut H) -> Result<()> {
        self.familiar().ok_or(Error::Null)?.init(host)?;
        let player = self.player();
        host.open_tab(&player, 662)?;
        host.set_viewed_tab(&player, 7)
    }

    /// `eat(foodId, npc)`.
    pub fn eat(&self, host: &mut H, food_id: i32, npc: &Familiar<H>) -> Result<()> {
        let player = self.player();
        if !npc.is_pet() {
            return Err(Error::Cast);
        }
        let current = self.familiar();
        if !current.as_ref().is_some_and(|f| f.same(npc)) {
            host.packet_message(&player, "This isn't your pet!")?;
            return Ok(());
        }
        let pet = current.ok_or(Error::Null)?;
        let Some(pets) = Pet::for_id(pet.pet_item_id()?) else {
            return Ok(());
        };
        for &food in pets.food {
            if food == food_id {
                let inventory = host.inventory(&player)?;
                host.remove(&inventory, &Item::one(food_id), None)?;
                if food_id == BUCKET_OF_MILK {
                    let inventory = host.inventory(&player)?;
                    host.add(&inventory, &Item::one(BUCKET), None)?;
                }
                let name = host.item_definition_name(food)?;
                host.packet_message(&player, &format!("Your pet happily eats the {name}."))?;
                host.animate(&player, 827)?;
                npc.pet_details()?.update_hunger(-15.0);
                return Ok(());
            }
        }
        host.packet_message(&player, "Nothing interesting happens.")
    }

    /// `pickup()`.
    pub fn pickup(&self, host: &mut H) -> Result<()> {
        let player = self.player();
        let inventory = host.inventory(&player)?;
        if host.free_slots(&inventory)? == 0 {
            host.packet_message(&player, "You don't have enough room in your inventory.")?;
            return Ok(());
        }
        let pet = match self.familiar() {
            None => return Err(Error::Null),
            Some(f) if !f.is_pet() => return Err(Error::Cast),
            Some(f) => f,
        };
        let inventory = host.inventory(&player)?;
        if host.add(&inventory, &Item::one(pet.pet_item_id()?), None)? {
            host.animate(&player, 827)?;
            host.manager(&player)?.dismiss(host)?;
        }
        Ok(())
    }

    /// `adjustBattleState(state)`.
    pub fn adjust_battle_state(&self, host: &mut H, state: &mut H::State) -> Result<()> {
        let Some(familiar) = self.familiar() else {
            return Ok(());
        };
        host.adjust_player_battle(&familiar, state)
    }

    /// `getBoost(skill)`.
    pub fn get_boost(&self, skill: i32) -> i32 {
        self.familiar().map_or(0, |f| f.get_boost(skill))
    }

    /// `dismiss()`.
    pub fn dismiss(&self, host: &mut H) -> Result<()> {
        if let Some(familiar) = self.familiar() {
            familiar.dismiss(host)?;
        }
        Ok(())
    }

    /// `addDetails(itemId, details)`.
    pub fn add_details(&self, item_id: i32, details: Details) {
        self.pet_details().with(|map| {
            if map.get(item_id).is_none() {
                map.put(item_id, Vec::new());
            }
            map.get_mut(item_id).expect("put above").push(details);
        });
    }

    /// `removeDetails(itemId)`: the last of the list; a missing list throws.
    pub fn remove_details(&self, item_id: i32) -> Result<()> {
        self.pet_details().with(|map| {
            let list = map.get_mut(item_id).ok_or(Error::Null)?;
            list.pop();
            Ok(())
        })
    }

    /// `isOwner(familiar)`.
    pub fn is_owner(&self, host: &mut H, familiar: &Familiar<H>) -> Result<bool> {
        let Some(current) = self.familiar() else {
            return Ok(false);
        };
        if !current.same(familiar) {
            host.packet_message(&self.player(), "This is not your familiar.")?;
            return Ok(false);
        }
        Ok(true)
    }

    /// What `PlayerSaver.saveFamiliarManager` reads off the current
    /// familiar: a pet's item id, or the familiar's original id, ticks,
    /// special points, a non-empty beast of burden's container and the
    /// NPC's lifepoints. The live save host answers `save::Host::familiars`
    /// with the store's entries and this.
    pub fn saved_active(&self, host: &mut H) -> Result<ActiveFamiliar> {
        let Some(familiar) = self.familiar() else {
            return Ok(ActiveFamiliar::None);
        };
        if familiar.is_pet() {
            return Ok(ActiveFamiliar::Pet {
                item_id: familiar.pet_item_id()?,
            });
        }
        let npc = familiar.npc();
        let original_id = host.original_id(&npc)?;
        let burden = if familiar.is_burden_beast() {
            let container = familiar.container()?;
            if host.is_empty(&container)? {
                None
            } else {
                Some(host.saved_container(&container)?)
            }
        } else {
            None
        };
        Ok(ActiveFamiliar::Familiar {
            original_id,
            ticks: familiar.ticks(),
            special_points: familiar.special_points(),
            burden,
            lifepoints: host.lifepoints(&npc)?,
        })
    }
}
