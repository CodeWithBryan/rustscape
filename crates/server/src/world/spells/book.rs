//! `SpellBookManager`, its `SpellBook` registry and `SpellBlocks`.
use super::{Error, Host, Result};
use std::collections::HashMap;

/// `SpellBookManager.SpellBook`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpellBook {
    Modern,
    Ancient,
    Lunar,
}

impl SpellBook {
    pub const ALL: [Self; 3] = [Self::Modern, Self::Ancient, Self::Lunar];

    pub fn ordinal(self) -> i32 {
        match self {
            Self::Modern => 0,
            Self::Ancient => 1,
            Self::Lunar => 2,
        }
    }

    /// `getInterfaceId()`.
    pub fn interface_id(self) -> i32 {
        match self {
            Self::Modern => 192,
            Self::Ancient => 193,
            Self::Lunar => 430,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Modern => "MODERN",
            Self::Ancient => "ANCIENT",
            Self::Lunar => "LUNAR",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|b| b.name() == name)
    }

    /// `forInterface(id)`.
    pub fn for_interface(id: i32) -> Option<Self> {
        Self::ALL.iter().copied().find(|b| b.interface_id() == id)
    }
}

/// `SpellBookManager.SpellbookChangeSource`: the ways a book can change.
/// Declared by the original but read by nothing ported here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellbookChangeSource {
    Altar,
    MagicCapePerk,
    SpellbookSwapCast,
    SpellbookSwapRestore,
}

impl SpellbookChangeSource {
    pub const ALL: [Self; 4] = [
        Self::Altar,
        Self::MagicCapePerk,
        Self::SpellbookSwapCast,
        Self::SpellbookSwapRestore,
    ];
}

/// The three books' `spells` maps. Only `put` and `get` happen, so the map's
/// iteration order is never observed.
pub struct Registry<S> {
    spells: HashMap<(SpellBook, i32), S>,
}

impl<S> Default for Registry<S> {
    fn default() -> Self {
        Self {
            spells: HashMap::new(),
        }
    }
}

impl<S: Clone> Registry<S> {
    /// `book.register(buttonId, spell)` after the caller has set the spell's
    /// id to the button (`spell.setSpellId(buttonId)`): a later registration
    /// at the same button replaces the earlier one.
    pub fn put(&mut self, book: SpellBook, button: i32, spell: S) {
        self.spells.insert((book, button), spell);
    }

    /// `book.getSpell(buttonId)`.
    pub fn get(&self, book: SpellBook, button: i32) -> Option<S> {
        self.spells.get(&(book, button)).cloned()
    }

    pub fn len(&self) -> usize {
        self.spells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spells.is_empty()
    }

    /// Every registration, in book and button order.
    pub fn entries(&self) -> Vec<(SpellBook, i32, S)> {
        let mut out: Vec<_> = self
            .spells
            .iter()
            .map(|((b, i), s)| (*b, *i, s.clone()))
            .collect();
        out.sort_by_key(|(b, i, _)| (b.ordinal(), *i));
        out
    }
}

/// `SpellBookManager`: the player's current spell book as an interface id.
/// The canonical player keeps this field (`profile.spell_book`); these are
/// the manager's members over it.
pub struct SpellBookManager;

impl SpellBookManager {
    /// The field's initial value.
    pub const INITIAL: i32 = 192;

    /// `setSpellBook(book)`.
    pub fn set_spell_book(field: &mut i32, book: SpellBook) {
        *field = book.interface_id();
    }

    /// `update(player)`: opens the book's tab, then sets varbit 357 to the
    /// book's ordinal; a field naming no book fails after the tab opens.
    pub fn update<H: Host>(h: &mut H, player: &H::Node) -> Result<()> {
        let book = *h.spell_book(player);
        h.open_tab(player, book)?;
        let ordinal = SpellBook::for_interface(*h.spell_book(player))
            .ok_or(Error::Null)?
            .ordinal();
        h.set_varbit(player, 357, ordinal, false)
    }
}

/// `SpellBlocks`: a static map from spell id to blocked nodes. Only `get` and
/// `put` happen, so its iteration order is never observed; each list keeps
/// insertion order.
pub struct SpellBlocks<N> {
    blocks: HashMap<i32, Vec<N>>,
}

impl<N> Default for SpellBlocks<N> {
    fn default() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }
}

impl<N: Clone> SpellBlocks<N> {
    /// `register(spellId, toBlock)`.
    pub fn register(&mut self, spell_id: i32, node: N) {
        self.blocks.entry(spell_id).or_default().push(node);
    }

    /// `isBlocked(spellId, node)`: compares `node.getName()` with every
    /// listed node's name, reading both names for each element and never
    /// stopping early. A null name of the queried node fails at the first
    /// element.
    pub fn is_blocked<H: Host<Node = N>>(
        &self,
        h: &mut H,
        spell_id: i32,
        node: &N,
    ) -> Result<bool> {
        let Some(list) = self.blocks.get(&spell_id) else {
            return Ok(false);
        };
        let list = list.clone();
        let mut blocked = false;
        for other in &list {
            // The receiver and the argument are both evaluated before the
            // receiver's null check.
            let name = h.name(node)?;
            let other = h.name(other)?;
            let name = name.ok_or(Error::Null)?;
            if other.as_deref() == Some(name.as_str()) {
                blocked = true;
            }
        }
        Ok(blocked)
    }
}
