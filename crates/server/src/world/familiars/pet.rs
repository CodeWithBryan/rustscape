//! `Pet`: a familiar without a timer or pouch whose `PetDetails` grow and
//! get hungry every tick, that warns, runs away when starving and grows
//! into its next stage (`growNextStage` morphs it through the manager).
use super::pets::Pet;
use super::{Details, Error, Familiar, Host, Item, Result};

/// `Pet`'s own fields.
#[derive(Clone, Debug)]
pub struct PetState {
    pub item_id: i32,
    pub details: Details,
    pub growth_rate: f64,
    pub pet: &'static Pet,
    /// 0 none, 1 hungry, 2 starving.
    pub has_warned: i32,
}

/// The varp 1175 value: growth in bits 1..9, hunger from bit 9.
fn growth_hunger(details: &Details) -> i32 {
    (details.growth() as i32).wrapping_shl(1) | (details.hunger() as i32).wrapping_shl(9)
}

impl<H: Host> Familiar<H> {
    fn pet(&self) -> Result<PetState> {
        self.pet_state().ok_or(Error::Cast)
    }
    /// `Pet.getItemId()`.
    pub fn pet_item_id(&self) -> Result<i32> {
        Ok(self.pet()?.item_id)
    }
    /// `Pet.getDetails()`.
    pub fn pet_details(&self) -> Result<Details> {
        Ok(self.pet()?.details)
    }
    /// `Pet.getPet()`.
    pub fn pet_kind(&self) -> Result<&'static Pet> {
        Ok(self.pet()?.pet)
    }
    /// `Pet.getHasWarned()`.
    pub fn has_warned(&self) -> Result<i32> {
        Ok(self.pet()?.has_warned)
    }
    /// `Pet.setHasWarned(value)`.
    pub fn set_has_warned(&self, value: i32) -> Result<()> {
        self.pet()?;
        self.pet_set_has_warned(value);
        Ok(())
    }

    /// `Pet.sendConfiguration()`.
    pub(crate) fn pet_send_configuration(&self, host: &mut H) -> Result<()> {
        let pet = self.pet()?;
        let owner = self.owner_or_null()?;
        host.set_varp(&owner, 1175, growth_hunger(&pet.details))?;
        let id = host.npc_id(&self.npc())?;
        host.set_varp(&owner, 1174, id)?;
        host.set_varp(&owner, 448, pet.item_id)
    }

    /// `Pet.handleTickActions()`.
    pub(crate) fn pet_tick_actions(&self, host: &mut H) -> Result<()> {
        let state = self.pet()?;
        let details = state.details.clone();
        let pet = state.pet;
        let item_id = state.item_id;
        let owner = self.owner_or_null()?;
        if !pet.food.is_empty() && !pet.is_grown_cat(item_id) && !host.pet_mastery(&owner)? {
            let mut amount = if item_id == pet.baby_item {
                0.025
            } else {
                0.018
            };
            if host.attribute_int(&owner, "petrate", 1)? == 0 {
                amount = 0.0;
            } else if host.attribute_int(&owner, "petrate", 1)? == 2 {
                amount *= 100.0;
            }
            details.update_hunger(amount);
        }
        let hunger = details.hunger();
        if (75.0..=90.0).contains(&hunger) && self.has_warned()? < 1 {
            host.player_message(&owner, "<col=ff0000>Your pet is getting hungry.</col>")?;
            self.pet_set_has_warned(1);
        } else if hunger >= 90.0 && self.has_warned()? < 2 {
            host.packet_message(
                &owner,
                "<col=ff0000>Your pet is starving, feed it before it runs off.</col>",
            )?;
            self.pet_set_has_warned(2);
        }
        if hunger >= 100.0 && state.growth_rate != 0.0 && !pet.food.is_empty() {
            let manager = host.manager(&owner)?;
            manager.dismiss(host)?;
            let manager = host.manager(&owner)?;
            manager.remove_details(item_id)?;
            host.manager(&owner)?.set_familiar(None);
            host.set_varp(&owner, 1175, 0)?;
            host.player_message(&owner, "<col=ff0000>Your pet has run away.</col>")?;
            return Ok(());
        }
        let growth = details.growth();
        let mut growthrate = pet.growth_rate;
        if growthrate > 0.0 {
            if host.attribute_int(&owner, "petrate", 1)? == 0 {
                growthrate = 0.0;
            } else if host.attribute_int(&owner, "petrate", 1)? == 2 {
                growthrate *= 100.0;
            }
            details.update_growth(growthrate);
            if growth == 100.0 {
                self.grow_next_stage(host)?;
            }
        }
        self.follow_or_recall(host, &owner)?;
        host.set_varp(&owner, 1175, growth_hunger(&details))
    }

    /// `growNextStage()`: the next stage's details list takes this pet's
    /// details, the old list loses its last, and the manager morphs the
    /// pet where it stands with its hunger and no growth.
    pub fn grow_next_stage(&self, host: &mut H) -> Result<()> {
        let state = self.pet()?;
        let new_item_id = state.pet.next_stage_item_id(state.item_id);
        if new_item_id == -1 {
            return Ok(());
        }
        let owner = self.owner_or_null()?;
        if state.pet.is_kitten(state.item_id) {
            host.increment_attribute(&owner, "/save:stats_manager:cats_raised")?;
        }
        host.manager(&owner)?
            .add_details(new_item_id, state.details.clone());
        host.manager(&owner)?.remove_details(state.item_id)?;
        let location = host.location(&self.npc())?;
        let hunger = state.details.hunger();
        let manager = host.manager(&owner)?;
        manager.morph_pet(host, Item::one(new_item_id), false, location, hunger, 0.0)?;
        host.packet_message(&owner, "<col=ff0000>Your pet has grown larger.</col>")
    }
}
