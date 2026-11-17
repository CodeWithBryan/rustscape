//! `AbstractNPC`'s registry of NPC subclasses by id and `NPC.create`.
//!
//! The original keeps one prototype object per id (`mapping`, a
//! `HashMap<Integer, AbstractNPC>` only read with `get`/`containsKey`/`put`,
//! so its iteration order is never observed). `newInstance` puts the
//! prototype under each of its `getIds()`; an id another class already holds
//! is skipped with a warning, while the same class replaces its own entry.
//! The class test is `name != getClass().getSimpleName()` on two `String`
//! objects: JDK 11 caches a class's simple name, so the test is class
//! identity (a class of another package with the same simple name still
//! warns), which is how [`Registry::new_instance`] compares.
use super::{Abstract, Constructor, Direction, Factory, LogLevel, Result};
use std::collections::HashMap;

/// `AbstractNPC.mapping`: each id's class and prototype object.
pub struct Registry<C, P> {
    mapping: HashMap<i32, (C, P)>,
}
impl<C, P> Default for Registry<C, P> {
    fn default() -> Self {
        Self {
            mapping: HashMap::new(),
        }
    }
}
impl<C: Abstract, P: Clone> Registry<C, P> {
    /// `AbstractNPC.newInstance(arg)` on `prototype`, an object of `class`.
    pub fn new_instance<F: Factory + ?Sized>(
        &mut self,
        f: &mut F,
        class: C,
        prototype: P,
    ) -> Result<()> {
        for id in class.ids() {
            if let Some((held, _)) = self.mapping.get(&id) {
                let held = *held;
                if held != class {
                    let message = format!(
                        "[{}] - Warning: Mapping already contained NPC id {}! ({})",
                        class.simple_name(),
                        id,
                        held.simple_name()
                    );
                    f.log(class.name(), LogLevel::Err, &message)?;
                    continue;
                }
            }
            self.mapping.insert(id, (class, prototype.clone()));
        }
        Ok(())
    }
    /// `AbstractNPC.forId(id)`.
    pub fn for_id(&self, id: i32) -> Option<&(C, P)> {
        self.mapping.get(&id)
    }
    /// The ids registered, with their class (sorted by id; the original's
    /// map is never iterated).
    pub fn entries(&self) -> Vec<(i32, C)> {
        let mut out: Vec<(i32, C)> = self.mapping.iter().map(|(k, (c, _))| (*k, *c)).collect();
        out.sort_by_key(|(k, _)| *k);
        out
    }
    pub fn len(&self) -> usize {
        self.mapping.len()
    }
    pub fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }
}

/// A subclass constructor: `NPC(id, location)` (direction south), then
/// `super.setWalks(autowalk)`, then the class's own statements.
pub fn construct_npc<F: Factory + ?Sized>(
    f: &mut F,
    class: F::Class,
    id: i32,
    location: Option<[i32; 3]>,
    constructor: Constructor,
) -> Result<F::Npc> {
    let npc = f.new_npc(Some(class), id, location, Some(Direction::South))?;
    f.set_walks(&npc, constructor.walks)?;
    if let Some(aggressive) = constructor.aggressive {
        f.set_aggressive(&npc, aggressive)?;
    }
    Ok(npc)
}

/// The class scan's `newInstance()` of a class, then `definePlugin`'s
/// `newInstance(null)`: the prototype and its registration.
pub fn define<F: Factory + ?Sized>(
    f: &mut F,
    registry: &mut Registry<F::Class, F::Npc>,
    class: F::Class,
) -> Result<F::Npc> {
    let (id, constructor) = class.prototype();
    let prototype = construct_npc(f, class, id, None, constructor)?;
    registry.new_instance(f, class, prototype.clone())?;
    Ok(prototype)
}

/// `NPC.create(id, location, direction, objects...)`: the registered class's
/// `construct`, or a base NPC when there is none or it returns null.
pub fn create<F: Factory + ?Sized>(
    f: &mut F,
    registry: &Registry<F::Class, F::Npc>,
    id: i32,
    location: Option<[i32; 3]>,
    direction: Option<Direction>,
) -> Result<F::Npc> {
    let mut npc = None;
    if let Some((class, _)) = registry.for_id(id) {
        let class = *class;
        if let Some(constructor) = class.construct(location)? {
            npc = Some(construct_npc(f, class, id, location, constructor)?);
        }
    }
    match npc {
        Some(npc) => Ok(npc),
        None => f.new_npc(None, id, location, direction),
    }
}
