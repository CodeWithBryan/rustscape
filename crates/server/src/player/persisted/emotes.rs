//! EmoteManager's unlocked list, as `Emotes` ordinals in list order. Playing
//! emotes, locking/unlocking with their varp refresh and the emote tab are not
//! ported.
use crate::player::save::Emotes as Saved;

/// `Emotes.values().length`, the bound the parser checks saved ordinals against.
pub const COUNT: i32 = crate::player::save::EMOTES;

#[derive(Clone, Debug)]
pub struct Manager {
    pub emotes: Vec<i32>,
}
impl Default for Manager {
    /// The constructor unlocks the first 22 constants.
    fn default() -> Self {
        Self {
            emotes: (0..22).collect(),
        }
    }
}
impl Manager {
    pub fn is_unlocked(&self, ordinal: i32) -> bool {
        self.emotes.contains(&ordinal)
    }
    /// isSaveRequired: any constant past the 22nd.
    pub fn is_save_required(&self) -> bool {
        self.emotes.iter().any(|ordinal| *ordinal > 21)
    }
    /// `emotes.remove(emote)`: the first occurrence.
    pub fn remove(&mut self, ordinal: i32) -> bool {
        match self.emotes.iter().position(|e| *e == ordinal) {
            Some(at) => {
                self.emotes.remove(at);
                true
            }
            None => false,
        }
    }
    pub fn saved(&self) -> Saved {
        Saved {
            save_required: self.is_save_required(),
            ordinals: self.emotes.clone(),
        }
    }
}
