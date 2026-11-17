//! `NodeList.java` and `NodeListIterator.java`: the slot array behind
//! `Repository.players` and `Repository.npcs`.
//!
//! Slot 0 is never used; a list of capacity `n` has slots `1..=n`. A node's
//! index is the slot `add` put it in, and it is the node's own field: `add`
//! returns the slot and the caller writes it (`Node.setIndex`), removal does
//! not reset it, and `NodeList.indexOf(node)` is only `node.getIndex()`, so
//! it is not a method here. Nodes are never null in this port; the calls
//! that accept null in the original take an `Option`.
use super::{Error, MAX_CAPACITY, Result};
use crate::definitions::item::Shared;

struct Slots<E> {
    nodes: Vec<Option<E>>,
    size: i32,
}

/// A shared handle: clones are the same list, as `Repository.players` is one
/// object for every caller. No method calls out, so no lock is ever held
/// across a service.
pub struct NodeList<E> {
    slots: Shared<Slots<E>>,
}

impl<E> Clone for NodeList<E> {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
        }
    }
}

impl<E: Clone + PartialEq> NodeList<E> {
    /// `new NodeList(capacity)`: `capacity + 1` slots. Capacity -1 makes a
    /// list with no slot at all; below that the original fails with
    /// `NegativeArraySizeException`. Above [`MAX_CAPACITY`] this port refuses.
    pub fn new(capacity: i32) -> Result<Self> {
        if capacity < -1 {
            return Err(Error::NegativeArraySize);
        }
        if capacity > MAX_CAPACITY {
            return Err(Error::Capacity);
        }
        Ok(Self {
            slots: Shared::new(Slots {
                nodes: vec![None; (capacity + 1) as usize],
                size: 0,
            }),
        })
    }

    pub fn same(&self, other: &Self) -> bool {
        self.slots.same(&other.slots)
    }

    /// `get(index)`.
    pub fn get(&self, index: i32) -> Result<Option<E>> {
        let slots = self.slots.read();
        if index <= 0 || index as usize >= slots.nodes.len() {
            return Err(Error::IndexOutOfBounds);
        }
        Ok(slots.nodes[index as usize].clone())
    }

    /// `add(node)`: the first free slot from 1 up, or `None` (`false`) when
    /// there is none. The same node can be added twice.
    pub fn add(&self, node: E) -> Option<i32> {
        let mut slots = self.slots.write();
        let id = (1..slots.nodes.len()).find(|&i| slots.nodes[i].is_none())?;
        slots.nodes[id] = Some(node);
        slots.size = slots.size.wrapping_add(1);
        Some(id as i32)
    }

    /// `addAll`: one `add` per node; the result is true if any was added.
    pub fn add_all(&self, nodes: impl IntoIterator<Item = E>) -> Vec<Option<i32>> {
        nodes.into_iter().map(|node| self.add(node)).collect()
    }

    /// `clear()`: every slot emptied and the size set to 0.
    pub fn clear(&self) {
        let mut slots = self.slots.write();
        slots.nodes.fill(None);
        slots.size = 0;
    }

    /// `contains(node)`. For null it is true while any slot from 1 up is free.
    pub fn contains(&self, node: Option<&E>) -> bool {
        self.slots
            .read()
            .nodes
            .iter()
            .skip(1)
            .any(|slot| slot.as_ref() == node)
    }

    /// `containsAll`.
    pub fn contains_all<'a>(&self, nodes: impl IntoIterator<Item = Option<&'a E>>) -> bool
    where
        E: 'a,
    {
        let mut failed = false;
        for node in nodes {
            if !self.contains(node) {
                failed = true;
            }
        }
        !failed
    }

    /// `isEmpty()`: the size counter is 0, whatever the slots hold.
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }

    /// `remove(int index)`. It has no range check of its own: slot 0 answers
    /// false, anything outside the array is an `ArrayIndexOutOfBoundsException`.
    pub fn remove_at(&self, index: i32) -> Result<bool> {
        let mut slots = self.slots.write();
        if index < 0 || index as usize >= slots.nodes.len() {
            return Err(Error::ArrayIndexOutOfBounds);
        }
        let removed = slots.nodes[index as usize].take().is_some();
        if removed {
            slots.size = slots.size.wrapping_sub(1);
        }
        Ok(removed)
    }

    /// `remove(Object)`: the first slot holding the node. For null that is
    /// the first free slot, and the size still goes down by one.
    pub fn remove(&self, node: Option<&E>) -> bool {
        let mut slots = self.slots.write();
        let Some(index) = (1..slots.nodes.len()).find(|&i| slots.nodes[i].as_ref() == node) else {
            return false;
        };
        slots.nodes[index] = None;
        slots.size = slots.size.wrapping_sub(1);
        true
    }

    /// `removeAll`.
    pub fn remove_all<'a>(&self, nodes: impl IntoIterator<Item = Option<&'a E>>) -> bool
    where
        E: 'a,
    {
        let mut changed = false;
        for node in nodes {
            if self.remove(node) {
                changed = true;
            }
        }
        changed
    }

    /// `retainAll(collection)`; `keep` is the collection's `contains`.
    pub fn retain_all(&self, keep: impl Fn(&E) -> bool) -> bool {
        let mut slots = self.slots.write();
        let mut changed = false;
        for index in 1..slots.nodes.len() {
            if slots.nodes[index].as_ref().is_some_and(|node| !keep(node)) {
                slots.nodes[index] = None;
                slots.size = slots.size.wrapping_sub(1);
                changed = true;
            }
        }
        changed
    }

    /// `size()`: the counter, which `remove(null)` can push below the number
    /// of held nodes and below zero.
    pub fn size(&self) -> i32 {
        self.slots.read().size
    }

    /// `length()`: the capacity plus one.
    pub fn length(&self) -> i32 {
        self.slots.read().nodes.len() as i32
    }

    /// `toArray()`: an array of `size()` elements filled from the occupied
    /// slots in slot order. A counter that is too small makes it fail.
    pub fn to_array(&self) -> Result<Vec<Option<E>>> {
        let slots = self.slots.read();
        if slots.size < 0 {
            return Err(Error::NegativeArraySize);
        }
        let mut array = vec![None; slots.size as usize];
        for (pointer, node) in slots.nodes.iter().skip(1).flatten().enumerate() {
            *array.get_mut(pointer).ok_or(Error::ArrayIndexOutOfBounds)? = Some(node.clone());
        }
        Ok(array)
    }

    /// The occupied slots with their indices, for diagnostics.
    pub fn occupied(&self) -> Vec<(i32, E)> {
        self.slots
            .read()
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| Some((index as i32, node.clone()?)))
            .collect()
    }

    /// `iterator()`: a copy of the list as it is now.
    pub fn iter(&self) -> Result<NodeListIter<E>> {
        Ok(NodeListIter {
            list: self.clone(),
            nodes: self.to_array()?,
            last_index: None,
            cursor: 0,
        })
    }
}

/// `NodeListIterator`: it walks the array `toArray` made when it was created,
/// so later changes to the list are not seen, and `remove` goes to the list.
pub struct NodeListIter<E> {
    list: NodeList<E>,
    nodes: Vec<Option<E>>,
    last_index: Option<usize>,
    cursor: usize,
}

impl<E: Clone + PartialEq> NodeListIter<E> {
    pub fn has_next(&mut self) -> bool {
        while self.cursor < self.nodes.len() {
            if self.nodes[self.cursor].is_some() {
                return true;
            }
            self.cursor += 1;
        }
        false
    }

    /// `next()`, failing with `NoSuchElementException` at the end. Named as
    /// the original names it; this is not `Iterator::next`.
    pub fn next_node(&mut self) -> Result<E> {
        if !self.has_next() {
            return Err(Error::NoSuchElement);
        }
        self.last_index = Some(self.cursor);
        self.cursor += 1;
        self.nodes[self.cursor - 1]
            .clone()
            .ok_or(Error::NoSuchElement)
    }

    /// `remove()`: `IllegalStateException` before the first `next`; otherwise
    /// the last returned node is removed from the list if it still holds it.
    pub fn remove(&mut self) -> Result<()> {
        let last = self.last_index.ok_or(Error::IllegalState)?;
        self.list.remove(self.nodes[last].as_ref());
        Ok(())
    }
}
