//! Properties.autocastSpell as the save file holds it: the spell's own book
//! and spell id. The spell registry is world start-up content and the
//! caller's input; spell casting is not ported.
use crate::player::save::{Autocast, Error, Result};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct Spell {
    /// MagicSpell.getBook: the book the spell was built for, which need not be
    /// the book it was registered in.
    pub book: i32,
    pub combat: bool,
}
/// Each SpellBook's `spells` map, keyed by book ordinal and button id.
/// SpellBook.register sets the spell's id to the button id.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub spells: BTreeMap<(i32, i32), Spell>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    pub autocast: Option<Autocast>,
}
impl State {
    /// `SpellBook.values()[book].getSpell(id) as CombatSpell`, assigned:
    /// a missing spell fails as null, another kind of spell as a cast.
    pub fn set(&mut self, registry: &Registry, book: i32, id: i32) -> Result<()> {
        let spell = registry.spells.get(&(book, id)).ok_or(Error::Null)?;
        if !spell.combat {
            return Err(Error::Type);
        }
        self.autocast = Some(Autocast {
            book: spell.book,
            spell_id: id,
        });
        Ok(())
    }
}
