//! Shared RT4 ground-item lists and `Protocol.spawnGroundObject` selection.
//!
//! Coordinates are local scene tiles, in conventional x/y order. Metadata,
//! terrain height and scene publication are explicit client services. No locks
//! are held while calling them. Item and list handles retain Java reference
//! identity, including captured lists replaced during a metadata callback.
//! Structural updates belong to the client's serialized packet/update phase;
//! the handles are shareable, but concurrent list edits are not a game tick.
use super::{Frame, updates::ServerUpdate};
use crate::{Error, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, MutexGuard, Weak},
};

pub const PLANES: i32 = 4;
pub const SIZE: i32 = 104;

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|error| error.into_inner())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Tile {
    pub plane: i32,
    pub x: i32,
    pub y: i32,
}
impl Tile {
    fn index(self) -> Result<usize> {
        if !(0..PLANES).contains(&self.plane)
            || !(0..SIZE).contains(&self.x)
            || !(0..SIZE).contains(&self.y)
        {
            return Err(Error::new(0, "ground tile outside scene"));
        }
        Ok(((self.plane * SIZE + self.x) * SIZE + self.y) as usize)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Item {
    pub id: i32,
    pub amount: i32,
}

struct Node {
    serial: u64,
    item: Option<Item>,
    previous: Weak<Mutex<Node>>,
    next: Weak<Mutex<Node>>,
    owner: Weak<Mutex<List>>,
    handle: Weak<Retained>,
}

// Lists retain bare nodes; externally retained handles lease their current
// owning list. Nodes refer back to handles weakly, so this creates no cycles.
// Updating one shared lease on transfer also updates all exposed handle clones.
struct Retained {
    node: Arc<Mutex<Node>>,
    owner: Mutex<Option<Arc<Mutex<List>>>>,
}

/// A retained ObjStackNode/ObjStack pair. Equality is pointer identity, not ID.
#[derive(Clone)]
pub struct ItemRef(Arc<Retained>);
impl std::fmt::Debug for ItemRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ItemRef")
            .field(&self.serial())
            .field(&lock(&self.0.node).item)
            .finish()
    }
}
impl PartialEq for ItemRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.node, &other.0.node)
    }
}
impl Eq for ItemRef {}
impl ItemRef {
    fn new(serial: u64, item: Option<Item>) -> Self {
        Self::retain(Arc::new(Mutex::new(Node {
            serial,
            item,
            previous: Weak::new(),
            next: Weak::new(),
            owner: Weak::new(),
            handle: Weak::new(),
        })))
    }
    fn retain(node: Arc<Mutex<Node>>) -> Self {
        let mut data = lock(&node);
        if let Some(handle) = data.handle.upgrade() {
            return Self(handle);
        }
        let handle = Arc::new(Retained {
            node: node.clone(),
            owner: Mutex::new(data.owner.upgrade()),
        });
        data.handle = Arc::downgrade(&handle);
        Self(handle)
    }
    pub fn serial(&self) -> u64 {
        lock(&self.0.node).serial
    }
    pub fn get(&self) -> Item {
        lock(&self.0.node)
            .item
            .expect("item node, not list sentinel")
    }
    pub fn set(&self, item: Item) {
        lock(&self.0.node).item = Some(item);
    }
    pub fn set_amount(&self, amount: i32) {
        lock(&self.0.node).item.as_mut().expect("item node").amount = amount;
    }
    pub fn is_linked(&self) -> bool {
        lock(&self.0.node).previous.strong_count() != 0
    }
    /// Removing the iterator's captured next node does not advance its cursor.
    pub fn unlink(&self) {
        let (previous, next, owner, serial) = {
            let node = lock(&self.0.node);
            (
                node.previous.upgrade(),
                node.next.upgrade(),
                node.owner.upgrade(),
                node.serial,
            )
        };
        if let Some(previous) = previous {
            let next = next.expect("linked node has successor");
            lock(&previous).next = Arc::downgrade(&next);
            lock(&next).previous = Arc::downgrade(&previous);
            {
                let mut node = lock(&self.0.node);
                node.previous = Weak::new();
                node.next = Weak::new();
                node.owner = Weak::new();
            }
            if let Some(owner) = owner {
                lock(&owner).members.remove(&serial);
            }
            *lock(&self.0.owner) = None;
        }
    }
}

struct List {
    sentinel: Arc<Mutex<Node>>,
    members: BTreeMap<u64, Arc<Mutex<Node>>>,
    cursor: Option<Arc<Mutex<Node>>>,
}

/// Original intrusive list and its single shared iterator cursor. Links are
/// weak; the member table and captured iterator retain nodes without cycles.
#[derive(Clone)]
pub struct Stack(Arc<Mutex<List>>);
impl Default for Stack {
    fn default() -> Self {
        let sentinel = ItemRef::new(0, None).0.node.clone();
        {
            let mut node = lock(&sentinel);
            node.previous = Arc::downgrade(&sentinel);
            node.next = Arc::downgrade(&sentinel);
        }
        Self(Arc::new(Mutex::new(List {
            sentinel,
            members: BTreeMap::new(),
            cursor: None,
        })))
    }
}
impl Stack {
    fn insert(&self, item: &ItemRef, head: bool) {
        item.unlink();
        let mut list = lock(&self.0);
        let sentinel = &list.sentinel;
        let (previous, next) = if head {
            (sentinel.clone(), lock(sentinel).next.upgrade().unwrap())
        } else {
            (lock(sentinel).previous.upgrade().unwrap(), sentinel.clone())
        };
        {
            let mut node = lock(&item.0.node);
            node.previous = Arc::downgrade(&previous);
            node.next = Arc::downgrade(&next);
            node.owner = Arc::downgrade(&self.0);
        }
        lock(&previous).next = Arc::downgrade(&item.0.node);
        lock(&next).previous = Arc::downgrade(&item.0.node);
        *lock(&item.0.owner) = Some(self.0.clone());
        list.members.insert(item.serial(), item.0.node.clone());
    }
    pub fn add_tail(&self, item: &ItemRef) {
        self.insert(item, false);
    }
    pub fn add_head(&self, item: &ItemRef) {
        self.insert(item, true);
    }
    fn advance(&self, first: bool) -> Result<Option<ItemRef>> {
        let mut list = lock(&self.0);
        let node = if first {
            lock(&list.sentinel).next.upgrade()
        } else {
            list.cursor.clone()
        }
        .ok_or(Error::new(0, "null ground list iterator"))?;
        if Arc::ptr_eq(&node, &list.sentinel) {
            list.cursor = None;
            Ok(None)
        } else {
            list.cursor = lock(&node).next.upgrade();
            Ok(Some(ItemRef::retain(node)))
        }
    }
    pub fn head(&self) -> Result<Option<ItemRef>> {
        self.advance(true)
    }
    pub fn next(&self) -> Result<Option<ItemRef>> {
        self.advance(false)
    }
    /// Observation without changing the original list's iterator cursor.
    pub fn items(&self) -> Vec<ItemRef> {
        let list = lock(&self.0);
        let mut result = Vec::with_capacity(list.members.len());
        let mut node = lock(&list.sentinel).next.upgrade();
        while let Some(current) = node {
            if Arc::ptr_eq(&current, &list.sentinel) {
                break;
            }
            node = lock(&current).next.upgrade();
            result.push(ItemRef::retain(current));
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Info {
    pub cost: i32,
    /// The reference multiplies the value only when this is exactly one.
    pub stackable: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pile {
    pub height: i32,
    pub key: i64,
    pub primary: ItemRef,
    pub secondary: Option<ItemRef>,
    pub tertiary: Option<ItemRef>,
}

pub trait Context {
    fn info(&mut self, state: &mut State, id: i32) -> Result<Info>;
    fn height(&mut self, state: &mut State, plane: i32, fine_x: i32, fine_y: i32) -> Result<i32>;
    fn publish(&mut self, state: &mut State, tile: Tile, pile: Option<Pile>) -> Result<()> {
        state.publish(tile, pile)
    }
}

#[derive(Default)]
pub struct State {
    pub plane: i32,
    /// Actual network player slot, not players::SELF (2047) or a profile UID.
    pub self_id: i32,
    pub origin: [i32; 2],
    stacks: BTreeMap<usize, Stack>,
    rendered: BTreeMap<Tile, Pile>,
    dirty: BTreeSet<Tile>,
}
impl State {
    pub fn new_item(&mut self, item: Item) -> ItemRef {
        // Global IDs prevent collisions when explicitly transferring a retained
        // node between lists/worlds. Equality still uses the actual reference.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        // `try_update`, its newer name, does not exist at the pinned 1.86.
        #[allow(deprecated)]
        let serial = NEXT
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |next| next.checked_add(1),
            )
            .expect("ground node identity exhausted");
        ItemRef::new(serial, Some(item))
    }
    pub fn stack(&self, tile: Tile) -> Result<Option<Stack>> {
        Ok(self.stacks.get(&tile.index()?).cloned())
    }
    /// Direct field assignment, without an implicit selection/render callback.
    pub fn set_stack(&mut self, tile: Tile, stack: Option<Stack>) -> Result<()> {
        let index = tile.index()?;
        match stack {
            Some(stack) => {
                self.stacks.insert(index, stack);
            }
            None => {
                self.stacks.remove(&index);
            }
        }
        Ok(())
    }
    pub fn pile(&self, tile: Tile) -> Option<&Pile> {
        self.rendered.get(&tile)
    }
    pub fn piles(&self) -> impl Iterator<Item = (Tile, &Pile)> {
        self.rendered.iter().map(|(tile, pile)| (*tile, pile))
    }
    pub fn publish(&mut self, tile: Tile, pile: Option<Pile>) -> Result<()> {
        tile.index()?;
        match pile {
            Some(pile) => {
                self.rendered.insert(tile, pile);
            }
            None => {
                self.rendered.remove(&tile);
            }
        }
        self.dirty.insert(tile);
        Ok(())
    }
    pub fn take_dirty(&mut self) -> Vec<Tile> {
        std::mem::take(&mut self.dirty).into_iter().collect()
    }
    pub fn occupied(&self) -> Vec<Tile> {
        self.stacks
            .keys()
            .map(|index| Tile {
                plane: *index as i32 / (SIZE * SIZE),
                x: *index as i32 / SIZE % SIZE,
                y: *index as i32 % SIZE,
            })
            .collect()
    }
    /// LoginManager's logical array shift, retaining list and node references.
    /// Scene teardown/rebuild and repopulating rendered piles are separate phases.
    pub fn rebase_stacks(&mut self, delta: [i32; 2]) {
        // No callbacks occur in the original array-shift loop. Moving only
        // nonnull cells is equivalent to its direction-aware in-place copy.
        // This also avoids allocating 43,264 empty slots in every AI client.
        for (index, stack) in std::mem::take(&mut self.stacks) {
            let destination = Tile {
                plane: index as i32 / (SIZE * SIZE),
                x: (index as i32 / SIZE % SIZE).wrapping_sub(delta[0]),
                y: (index as i32 % SIZE).wrapping_sub(delta[1]),
            };
            if let Ok(index) = destination.index() {
                self.stacks.insert(index, stack);
            }
        }
    }
    pub fn clear_rendered(&mut self) {
        self.dirty.extend(self.rendered.keys().copied());
        self.rendered.clear();
    }
    pub fn refresh(&mut self, x: i32, y: i32, context: &mut impl Context) -> Result<()> {
        let Some(stack) = self.stack(Tile {
            plane: self.plane,
            x,
            y,
        })?
        else {
            return context.publish(
                self,
                Tile {
                    plane: self.plane,
                    x,
                    y,
                },
                None,
            );
        };
        let mut best_value = -99_999_999;
        let mut best = None;
        let mut node = stack.head()?;
        while let Some(item) = node {
            let info = context.info(self, item.get().id)?;
            let value = if info.stackable == 1 {
                info.cost.wrapping_mul(item.get().amount.wrapping_add(1))
            } else {
                info.cost
            };
            if best_value < value {
                best_value = value;
                best = Some(item);
            }
            node = stack.next()?;
        }
        let Some(primary) = best else {
            return context.publish(
                self,
                Tile {
                    plane: self.plane,
                    x,
                    y,
                },
                None,
            );
        };
        stack.add_head(&primary);
        let mut secondary: Option<ItemRef> = None;
        let mut tertiary = None;
        let mut node = stack.head()?;
        while let Some(item) = node {
            if item.get().id != primary.get().id {
                if secondary.is_none() {
                    secondary = Some(item.clone());
                }
                if item.get().id != secondary.as_ref().unwrap().get().id && tertiary.is_none() {
                    tertiary = Some(item);
                }
            }
            node = stack.next()?;
        }
        let key = y
            .wrapping_shl(7)
            .wrapping_add(x)
            .wrapping_add(1_610_612_736) as i64;
        let tile = Tile {
            plane: self.plane,
            x,
            y,
        };
        let height = context.height(
            self,
            self.plane,
            x.wrapping_mul(128).wrapping_add(64),
            y.wrapping_mul(128).wrapping_add(64),
        )?;
        context.publish(
            self,
            tile,
            Some(Pile {
                height,
                key,
                primary,
                secondary,
                tertiary,
            }),
        )
    }
    /// Apply each top-level frame once, including enclosed updates. Consumers
    /// must not feed the children a second time after applying their envelope.
    pub fn apply_frame(&mut self, frame: &Frame, context: &mut impl Context) -> Result<bool> {
        match ServerUpdate::decode(frame)? {
            Some(update) => self.apply(&update, context),
            None => Ok(false),
        }
    }
    pub fn apply(&mut self, update: &ServerUpdate, context: &mut impl Context) -> Result<bool> {
        let (x, y) = match update {
            ServerUpdate::ZoneOrigin { full, x, y } => {
                self.origin = [*x, *y];
                if *full {
                    let mut x = self.origin[0];
                    while x < self.origin[0].wrapping_add(8) {
                        let mut y = self.origin[1];
                        while y < self.origin[1].wrapping_add(8) {
                            let tile = Tile {
                                plane: self.plane,
                                x,
                                y,
                            };
                            if self.stack(tile)?.is_some() {
                                self.set_stack(tile, None)?;
                                self.refresh(x, y, context)?;
                            }
                            y = y.wrapping_add(1);
                        }
                        x = x.wrapping_add(1);
                    }
                }
                return Ok(true);
            }
            ServerUpdate::ZoneEnclosed { x, y, frames } => {
                self.origin = [*x, *y];
                for frame in frames {
                    self.apply_frame(frame, context)?;
                }
                return Ok(true);
            }
            ServerUpdate::GroundAdd { x, y, .. }
            | ServerUpdate::GroundRemove { x, y, .. }
            | ServerUpdate::GroundAmount { x, y, .. } => (
                self.origin[0].wrapping_add(*x),
                self.origin[1].wrapping_add(*y),
            ),
            _ => return Ok(false),
        };
        if !(0..SIZE).contains(&x) || !(0..SIZE).contains(&y) {
            return Ok(true);
        }
        let tile = Tile {
            plane: self.plane,
            x,
            y,
        };
        match update {
            ServerUpdate::GroundAdd {
                id,
                amount,
                exclude_player,
                ..
            } => {
                if *exclude_player == Some(self.self_id) {
                    return Ok(true);
                }
                let item = self.new_item(Item {
                    id: *id,
                    amount: *amount,
                });
                let stack = self.stack(tile)?.unwrap_or_default();
                self.set_stack(tile, Some(stack.clone()))?;
                stack.add_tail(&item);
            }
            ServerUpdate::GroundRemove { id, .. } => {
                let Some(stack) = self.stack(tile)? else {
                    return Ok(true);
                };
                let mut node = stack.head()?;
                while let Some(item) = node {
                    if item.get().id == id & 0x7fff {
                        item.unlink();
                        break;
                    }
                    node = stack.next()?;
                }
                if stack.head()?.is_none() {
                    self.set_stack(tile, None)?;
                }
            }
            ServerUpdate::GroundAmount {
                id,
                old_amount,
                amount,
                ..
            } => {
                let Some(stack) = self.stack(tile)? else {
                    return Ok(true);
                };
                let mut node = stack.head()?;
                while let Some(item) = node {
                    let value = item.get();
                    if value.id == id & 0x7fff && value.amount == *old_amount {
                        item.set_amount(*amount);
                        break;
                    }
                    node = stack.next()?;
                }
            }
            _ => unreachable!(),
        }
        self.refresh(x, y, context)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposed_nodes_follow_owner_transfers_and_release_without_cycles() {
        let mut state = State::default();
        let first = Stack::default();
        let second = Stack::default();
        let a = state.new_item(Item { id: 1, amount: 2 });
        let b = state.new_item(Item { id: 1, amount: 2 });
        let old_owner = Arc::downgrade(&first.0);
        let new_owner = Arc::downgrade(&second.0);
        let weak_a = Arc::downgrade(&a.0.node);
        let weak_b = Arc::downgrade(&b.0.node);
        first.add_tail(&a);
        first.add_tail(&b);
        assert_ne!(a, b);
        assert_eq!(first.head().unwrap(), Some(a.clone()));
        drop(first);
        assert!(old_owner.upgrade().is_some());
        second.add_tail(&a);
        second.add_tail(&b);
        assert!(old_owner.upgrade().is_none());
        drop(second);
        assert!(new_owner.upgrade().is_some());
        a.unlink();
        assert!(!a.is_linked());
        assert!(new_owner.upgrade().is_some());
        drop(a);
        assert!(weak_a.upgrade().is_none());
        drop(b);
        assert!(new_owner.upgrade().is_none());
        assert!(weak_b.upgrade().is_none());
    }

    #[test]
    fn published_values_remain_live_and_failed_height_keeps_prior_pile() {
        struct Services {
            fail: bool,
        }
        impl Context for Services {
            fn info(&mut self, _: &mut State, id: i32) -> Result<Info> {
                Ok(Info {
                    cost: id,
                    stackable: 0,
                })
            }
            fn height(&mut self, _: &mut State, _: i32, _: i32, _: i32) -> Result<i32> {
                if self.fail {
                    Err(Error::new(0, "height unavailable"))
                } else {
                    Ok(42)
                }
            }
        }
        let mut state = State::default();
        let mut services = Services { fail: false };
        let tile = Tile {
            plane: 0,
            x: 3,
            y: 4,
        };
        state
            .apply_frame(
                &super::super::updates::ground_add(3, 4, 2, 5),
                &mut services,
            )
            .unwrap();
        let retained = state.pile(tile).unwrap().primary.clone();
        assert_eq!(state.take_dirty(), [tile]);
        retained.set_amount(10);
        assert_eq!(state.pile(tile).unwrap().primary.get().amount, 10);
        services.fail = true;
        assert!(
            state
                .apply_frame(
                    &super::super::updates::ground_add(3, 4, 3, 7),
                    &mut services
                )
                .is_err()
        );
        let stack = state.stack(tile).unwrap().unwrap();
        assert_eq!(stack.items()[0].get().id, 3);
        assert_eq!(state.pile(tile).unwrap().primary, retained);
        assert!(state.take_dirty().is_empty());
        let owner = Arc::downgrade(&stack.0);
        drop(stack);
        state.set_stack(tile, None).unwrap();
        state.clear_rendered();
        assert_eq!(state.take_dirty(), [tile]);
        assert!(owner.upgrade().is_some());
        drop(retained);
        assert!(owner.upgrade().is_none());
    }
}
