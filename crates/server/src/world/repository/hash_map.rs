//! `java.util.HashMap` as the pinned JDK 11 builds and walks it, for the maps
//! whose iteration order the repository and the disconnection queue expose:
//! put, get, remove, clear and the table walk, through lists, tree bins,
//! resizes and splits.
//!
//! The order of a tree bin is its `next` chain, which insertion, removal and
//! every move of the root to the front change, so the red-black tree is kept
//! exactly as `HashMap.TreeNode` keeps it. Lookups walk the chain instead of
//! the tree: they find the same entry and change nothing.
//!
//! Not ported: iterators with their modification check, views, `LinkedHashMap`
//! hooks, load factors other than 0.75 and a table at `MAXIMUM_CAPACITY`.
use std::cmp::Ordering;

const DEFAULT_INITIAL_CAPACITY: i32 = 16;
const MAXIMUM_CAPACITY: i32 = 1 << 30;
const TREEIFY_THRESHOLD: i32 = 8;
const UNTREEIFY_THRESHOLD: i32 = 6;
const MIN_TREEIFY_CAPACITY: usize = 64;

/// A key type of one of the original's maps.
pub trait Key: Clone + Eq {
    /// `hashCode()`, or 0 for the null key, before `HashMap.hash` spreads it.
    fn hash_code(&self) -> i32;
    /// The order of two unequal keys with equal spread hashes in a tree bin:
    /// `compareComparables`, then `tieBreakOrder` where a key is null.
    fn tie(&self, other: &Self) -> Ordering;
}

impl Key for i32 {
    fn hash_code(&self) -> i32 {
        *self
    }
    fn tie(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }
}

/// A `String` key; `None` is the null key.
impl Key for Option<String> {
    fn hash_code(&self) -> i32 {
        self.as_ref().map_or(0, |text| {
            text.encode_utf16().fold(0i32, |hash, unit| {
                hash.wrapping_mul(31).wrapping_add(i32::from(unit))
            })
        })
    }
    /// `String.compareTo` compares UTF-16 units. A null key has no comparable
    /// class, so `tieBreakOrder` decides by identity hash, and null's is 0:
    /// null goes before any other key.
    fn tie(&self, other: &Self) -> Ordering {
        match (self, other) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(a), Some(b)) => a.encode_utf16().cmp(b.encode_utf16()),
        }
    }
}

fn spread(hash_code: i32) -> i32 {
    hash_code ^ ((hash_code as u32) >> 16) as i32
}

/// `HashMap.tableSizeFor`.
fn table_size_for(capacity: i32) -> i32 {
    let n = (-1i32 as u32)
        .checked_shr((capacity.wrapping_sub(1) as u32).leading_zeros())
        .unwrap_or(u32::MAX) as i32;
    if n < 0 {
        1
    } else if n >= MAXIMUM_CAPACITY {
        MAXIMUM_CAPACITY
    } else {
        n + 1
    }
}

#[derive(Clone, Copy, Default)]
struct Bin {
    head: Option<usize>,
    tree: bool,
}

struct Node<K, V> {
    key: K,
    value: V,
    hash: i32,
    next: Option<usize>,
    prev: Option<usize>,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
    red: bool,
}

pub struct JavaHashMap<K, V> {
    table: Vec<Bin>,
    nodes: Vec<Option<Node<K, V>>>,
    free: Vec<usize>,
    size: i32,
    threshold: i32,
}

impl<K: Key, V> Default for JavaHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Key, V> JavaHashMap<K, V> {
    /// `new HashMap<>()`.
    pub fn new() -> Self {
        Self {
            table: Vec::new(),
            nodes: Vec::new(),
            free: Vec::new(),
            size: 0,
            threshold: 0,
        }
    }

    /// `new HashMap<>(initialCapacity)`; `None` for a negative capacity, which
    /// the original refuses with an `IllegalArgumentException`.
    pub fn with_capacity(initial_capacity: i32) -> Option<Self> {
        (initial_capacity >= 0).then(|| Self {
            threshold: table_size_for(initial_capacity.min(MAXIMUM_CAPACITY)),
            ..Self::new()
        })
    }

    pub fn len(&self) -> i32 {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// The length of the table, 0 before the first put.
    pub fn table_len(&self) -> usize {
        self.table.len()
    }

    fn at(&self, node: usize) -> &Node<K, V> {
        self.nodes[node].as_ref().expect("live map node")
    }

    fn at_mut(&mut self, node: usize) -> &mut Node<K, V> {
        self.nodes[node].as_mut().expect("live map node")
    }

    fn find(&self, hash: i32, key: &K) -> Option<usize> {
        if self.table.is_empty() {
            return None;
        }
        let mut cursor = self.table[(self.table.len() - 1) & hash as u32 as usize].head;
        while let Some(node) = cursor {
            let entry = self.at(node);
            if entry.hash == hash && entry.key == *key {
                return Some(node);
            }
            cursor = entry.next;
        }
        None
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.find(spread(key.hash_code()), key)
            .map(|node| &self.at(node).value)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.find(spread(key.hash_code()), key).is_some()
    }

    /// The entries in the order `entrySet()`, `keySet()` and `values()` walk.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        let (mut bin, mut cursor) = (0, None);
        std::iter::from_fn(move || {
            while cursor.is_none() {
                cursor = self.table.get(bin)?.head;
                bin += 1;
            }
            let node = self.at(cursor?);
            cursor = node.next;
            Some((&node.key, &node.value))
        })
    }

    fn allocate(&mut self, key: K, value: V, hash: i32, next: Option<usize>) -> usize {
        let node = Node {
            key,
            value,
            hash,
            next,
            prev: None,
            parent: None,
            left: None,
            right: None,
            red: false,
        };
        match self.free.pop() {
            Some(index) => {
                self.nodes[index] = Some(node);
                index
            }
            None => {
                self.nodes.push(Some(node));
                self.nodes.len() - 1
            }
        }
    }

    /// `put`: the previous value of the key, if it had one.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        let hash = spread(key.hash_code());
        if let Some(existing) = self.find(hash, &key) {
            return Some(std::mem::replace(&mut self.at_mut(existing).value, value));
        }
        if self.table.is_empty() {
            self.resize();
        }
        let index = (self.table.len() - 1) & hash as u32 as usize;
        match self.table[index] {
            Bin { head: None, .. } => {
                let node = self.allocate(key, value, hash, None);
                self.table[index] = Bin {
                    head: Some(node),
                    tree: false,
                };
            }
            Bin { tree: true, .. } => self.put_tree_val(index, hash, key, value),
            Bin {
                head: Some(mut tail),
                ..
            } => {
                let mut bin_count = 0;
                while let Some(next) = self.at(tail).next {
                    tail = next;
                    bin_count += 1;
                }
                let node = self.allocate(key, value, hash, None);
                self.at_mut(tail).next = Some(node);
                if bin_count >= TREEIFY_THRESHOLD - 1 {
                    self.treeify_bin(hash);
                }
            }
        }
        self.size += 1;
        if self.size > self.threshold {
            self.resize();
        }
        None
    }

    /// `remove(key)`.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let hash = spread(key.hash_code());
        let node = self.find(hash, key)?;
        let index = (self.table.len() - 1) & hash as u32 as usize;
        if self.table[index].tree {
            self.remove_tree_node(node, index);
        } else {
            let next = self.at(node).next;
            if self.table[index].head == Some(node) {
                self.table[index].head = next;
            } else {
                let mut previous = self.table[index].head.expect("bin of a found node");
                while self.at(previous).next != Some(node) {
                    previous = self
                        .at(previous)
                        .next
                        .expect("chain reaches the found node");
                }
                self.at_mut(previous).next = next;
            }
        }
        self.size -= 1;
        self.free.push(node);
        self.nodes[node].take().map(|entry| entry.value)
    }

    /// `clear()`: the entries go, the table keeps its length.
    pub fn clear(&mut self) {
        if !self.table.is_empty() && self.size > 0 {
            self.size = 0;
            self.table.fill(Bin::default());
            self.nodes.clear();
            self.free.clear();
        }
    }

    fn resize(&mut self) {
        let old_capacity = self.table.len() as i32;
        let old_threshold = self.threshold;
        let (new_capacity, mut new_threshold);
        if old_capacity > 0 {
            if old_capacity >= MAXIMUM_CAPACITY {
                self.threshold = i32::MAX;
                return;
            }
            new_capacity = old_capacity << 1;
            new_threshold =
                if new_capacity < MAXIMUM_CAPACITY && old_capacity >= DEFAULT_INITIAL_CAPACITY {
                    old_threshold << 1
                } else {
                    0
                };
        } else if old_threshold > 0 {
            new_capacity = old_threshold;
            new_threshold = 0;
        } else {
            new_capacity = DEFAULT_INITIAL_CAPACITY;
            new_threshold = 12;
        }
        if new_threshold == 0 {
            let float = new_capacity as f32 * 0.75f32;
            new_threshold = if new_capacity < MAXIMUM_CAPACITY && float < MAXIMUM_CAPACITY as f32 {
                float as i32
            } else {
                i32::MAX
            };
        }
        self.threshold = new_threshold;
        let old = std::mem::replace(&mut self.table, vec![Bin::default(); new_capacity as usize]);
        let bit = old_capacity as usize;
        for (index, bin) in old.into_iter().enumerate() {
            let Some(head) = bin.head else {
                continue;
            };
            if self.at(head).next.is_none() {
                let target = self.at(head).hash as u32 as usize & (new_capacity as usize - 1);
                self.table[target] = bin;
                continue;
            }
            let (mut low, mut high) = (Vec::new(), Vec::new());
            let mut cursor = Some(head);
            while let Some(node) = cursor {
                cursor = self.at(node).next;
                if self.at(node).hash as u32 as usize & bit == 0 {
                    low.push(node);
                } else {
                    high.push(node);
                }
            }
            let both = !low.is_empty() && !high.is_empty();
            for (target, nodes) in [(index, low), (index + bit, high)] {
                let Some(&first) = nodes.first() else {
                    continue;
                };
                for (position, &node) in nodes.iter().enumerate() {
                    let entry = self.at_mut(node);
                    entry.next = nodes.get(position + 1).copied();
                    if bin.tree {
                        entry.prev = position.checked_sub(1).map(|before| nodes[before]);
                    }
                }
                self.table[target].head = Some(first);
                if bin.tree {
                    // TreeNode.split.
                    if nodes.len() as i32 <= UNTREEIFY_THRESHOLD {
                        self.untreeify(target);
                    } else {
                        self.table[target].tree = true;
                        if both {
                            self.treeify(target);
                        }
                    }
                }
            }
        }
    }

    /// `treeifyBin`: a short table grows instead.
    fn treeify_bin(&mut self, hash: i32) {
        if self.table.len() < MIN_TREEIFY_CAPACITY {
            self.resize();
            return;
        }
        let index = (self.table.len() - 1) & hash as u32 as usize;
        let mut previous = None;
        let mut cursor = self.table[index].head;
        while let Some(node) = cursor {
            self.at_mut(node).prev = previous;
            previous = Some(node);
            cursor = self.at(node).next;
        }
        if self.table[index].head.is_some() {
            self.table[index].tree = true;
            self.treeify(index);
        }
    }

    /// `untreeify`: the chain stays as it is, as plain nodes.
    fn untreeify(&mut self, index: usize) {
        self.table[index].tree = false;
        let mut cursor = self.table[index].head;
        while let Some(node) = cursor {
            let entry = self.at_mut(node);
            entry.prev = None;
            entry.parent = None;
            entry.left = None;
            entry.right = None;
            entry.red = false;
            cursor = entry.next;
        }
    }

    /// The side a new key goes to below `parent`: -1 or 1, never 0.
    fn direction(&self, hash: i32, key: &K, parent: usize) -> i32 {
        let entry = self.at(parent);
        if entry.hash > hash {
            -1
        } else if entry.hash < hash || key.tie(&entry.key) == Ordering::Greater {
            1
        } else {
            -1
        }
    }

    /// `TreeNode.treeify` over the chain of a bin.
    fn treeify(&mut self, index: usize) {
        let mut root: Option<usize> = None;
        let mut cursor = self.table[index].head;
        while let Some(x) = cursor {
            cursor = self.at(x).next;
            self.at_mut(x).left = None;
            self.at_mut(x).right = None;
            let Some(current_root) = root else {
                self.at_mut(x).parent = None;
                self.at_mut(x).red = false;
                root = Some(x);
                continue;
            };
            let hash = self.at(x).hash;
            let mut parent = current_root;
            loop {
                let direction = {
                    let key = &self.at(x).key;
                    self.direction(hash, key, parent)
                };
                let child = if direction <= 0 {
                    self.at(parent).left
                } else {
                    self.at(parent).right
                };
                match child {
                    Some(child) => parent = child,
                    None => {
                        self.at_mut(x).parent = Some(parent);
                        if direction <= 0 {
                            self.at_mut(parent).left = Some(x);
                        } else {
                            self.at_mut(parent).right = Some(x);
                        }
                        root = Some(self.balance_insertion(current_root, x));
                        break;
                    }
                }
            }
        }
        if let Some(root) = root {
            self.move_root_to_front(index, root);
        }
    }

    /// `putTreeVal` for a key the bin does not hold.
    fn put_tree_val(&mut self, index: usize, hash: i32, key: K, value: V) {
        let mut root = self.table[index].head.expect("tree bin has a head");
        while let Some(parent) = self.at(root).parent {
            root = parent;
        }
        let mut parent = root;
        loop {
            let direction = self.direction(hash, &key, parent);
            let child = if direction <= 0 {
                self.at(parent).left
            } else {
                self.at(parent).right
            };
            if let Some(child) = child {
                parent = child;
                continue;
            }
            let after = self.at(parent).next;
            let x = self.allocate(key, value, hash, after);
            if direction <= 0 {
                self.at_mut(parent).left = Some(x);
            } else {
                self.at_mut(parent).right = Some(x);
            }
            self.at_mut(parent).next = Some(x);
            self.at_mut(x).parent = Some(parent);
            self.at_mut(x).prev = Some(parent);
            if let Some(after) = after {
                self.at_mut(after).prev = Some(x);
            }
            let root = self.balance_insertion(root, x);
            self.move_root_to_front(index, root);
            return;
        }
    }

    fn move_root_to_front(&mut self, index: usize, root: usize) {
        let first = self.table[index].head;
        if first == Some(root) {
            return;
        }
        self.table[index].head = Some(root);
        let before = self.at(root).prev;
        let after = self.at(root).next;
        if let Some(after) = after {
            self.at_mut(after).prev = before;
        }
        if let Some(before) = before {
            self.at_mut(before).next = after;
        }
        if let Some(first) = first {
            self.at_mut(first).prev = Some(root);
        }
        self.at_mut(root).next = first;
        self.at_mut(root).prev = None;
    }

    /// `removeTreeNode` with `movable` true, as `remove(key)` calls it.
    fn remove_tree_node(&mut self, p: usize, index: usize) {
        let mut first = self.table[index].head;
        let mut root = first;
        let successor = self.at(p).next;
        let predecessor = self.at(p).prev;
        match predecessor {
            None => {
                first = successor;
                self.table[index].head = successor;
            }
            Some(predecessor) => self.at_mut(predecessor).next = successor,
        }
        if let Some(successor) = successor {
            self.at_mut(successor).prev = predecessor;
        }
        if first.is_none() {
            self.table[index].tree = false;
            return;
        }
        if let Some(mut top) = root {
            while let Some(parent) = self.at(top).parent {
                top = parent;
            }
            root = Some(top);
        }
        let too_small = match root {
            None => true,
            Some(root) => {
                self.at(root).right.is_none()
                    || self
                        .at(root)
                        .left
                        .is_none_or(|left| self.at(left).left.is_none())
            }
        };
        if too_small {
            self.untreeify(index);
            return;
        }
        let mut root = root.expect("checked above");
        let left = self.at(p).left;
        let right = self.at(p).right;
        let replacement = match (left, right) {
            (Some(pl), Some(pr)) => {
                let mut s = pr;
                while let Some(further) = self.at(s).left {
                    s = further;
                }
                let colour = self.at(s).red;
                self.at_mut(s).red = self.at(p).red;
                self.at_mut(p).red = colour;
                let sr = self.at(s).right;
                let pp = self.at(p).parent;
                if s == pr {
                    self.at_mut(p).parent = Some(s);
                    self.at_mut(s).right = Some(p);
                } else {
                    let sp = self.at(s).parent;
                    self.at_mut(p).parent = sp;
                    if let Some(sp) = sp {
                        if self.at(sp).left == Some(s) {
                            self.at_mut(sp).left = Some(p);
                        } else {
                            self.at_mut(sp).right = Some(p);
                        }
                    }
                    self.at_mut(s).right = Some(pr);
                    self.at_mut(pr).parent = Some(s);
                }
                self.at_mut(p).left = None;
                self.at_mut(p).right = sr;
                if let Some(sr) = sr {
                    self.at_mut(sr).parent = Some(p);
                }
                self.at_mut(s).left = Some(pl);
                self.at_mut(pl).parent = Some(s);
                self.at_mut(s).parent = pp;
                match pp {
                    None => root = s,
                    Some(pp) => {
                        if self.at(pp).left == Some(p) {
                            self.at_mut(pp).left = Some(s);
                        } else {
                            self.at_mut(pp).right = Some(s);
                        }
                    }
                }
                sr.unwrap_or(p)
            }
            (Some(pl), None) => pl,
            (None, Some(pr)) => pr,
            (None, None) => p,
        };
        if replacement != p {
            let pp = self.at(p).parent;
            self.at_mut(replacement).parent = pp;
            match pp {
                None => {
                    root = replacement;
                    self.at_mut(root).red = false;
                }
                Some(pp) => {
                    if self.at(pp).left == Some(p) {
                        self.at_mut(pp).left = Some(replacement);
                    } else {
                        self.at_mut(pp).right = Some(replacement);
                    }
                }
            }
            let entry = self.at_mut(p);
            entry.left = None;
            entry.right = None;
            entry.parent = None;
        }
        let r = if self.at(p).red {
            root
        } else {
            self.balance_deletion(root, replacement)
        };
        if replacement == p {
            let pp = self.at(p).parent;
            self.at_mut(p).parent = None;
            if let Some(pp) = pp {
                if self.at(pp).left == Some(p) {
                    self.at_mut(pp).left = None;
                } else if self.at(pp).right == Some(p) {
                    self.at_mut(pp).right = None;
                }
            }
        }
        self.move_root_to_front(index, r);
    }

    fn red(&self, node: Option<usize>) -> bool {
        node.is_some_and(|node| self.at(node).red)
    }

    fn rotate_left(&mut self, mut root: usize, p: Option<usize>) -> usize {
        let Some(p) = p else {
            return root;
        };
        let Some(r) = self.at(p).right else {
            return root;
        };
        let rl = self.at(r).left;
        self.at_mut(p).right = rl;
        if let Some(rl) = rl {
            self.at_mut(rl).parent = Some(p);
        }
        let pp = self.at(p).parent;
        self.at_mut(r).parent = pp;
        match pp {
            None => {
                root = r;
                self.at_mut(r).red = false;
            }
            Some(pp) => {
                if self.at(pp).left == Some(p) {
                    self.at_mut(pp).left = Some(r);
                } else {
                    self.at_mut(pp).right = Some(r);
                }
            }
        }
        self.at_mut(r).left = Some(p);
        self.at_mut(p).parent = Some(r);
        root
    }

    fn rotate_right(&mut self, mut root: usize, p: Option<usize>) -> usize {
        let Some(p) = p else {
            return root;
        };
        let Some(l) = self.at(p).left else {
            return root;
        };
        let lr = self.at(l).right;
        self.at_mut(p).left = lr;
        if let Some(lr) = lr {
            self.at_mut(lr).parent = Some(p);
        }
        let pp = self.at(p).parent;
        self.at_mut(l).parent = pp;
        match pp {
            None => {
                root = l;
                self.at_mut(l).red = false;
            }
            Some(pp) => {
                if self.at(pp).right == Some(p) {
                    self.at_mut(pp).right = Some(l);
                } else {
                    self.at_mut(pp).left = Some(l);
                }
            }
        }
        self.at_mut(l).right = Some(p);
        self.at_mut(p).parent = Some(l);
        root
    }

    fn balance_insertion(&mut self, mut root: usize, mut x: usize) -> usize {
        self.at_mut(x).red = true;
        loop {
            let Some(xp) = self.at(x).parent else {
                self.at_mut(x).red = false;
                return x;
            };
            if !self.at(xp).red {
                return root;
            }
            let Some(xpp) = self.at(xp).parent else {
                return root;
            };
            let xppl = self.at(xpp).left;
            if Some(xp) == xppl {
                let xppr = self.at(xpp).right;
                if self.red(xppr) {
                    self.at_mut(xppr.expect("red node")).red = false;
                    self.at_mut(xp).red = false;
                    self.at_mut(xpp).red = true;
                    x = xpp;
                    continue;
                }
                let mut grand = Some(xpp);
                let mut parent = Some(xp);
                if self.at(xp).right == Some(x) {
                    x = xp;
                    root = self.rotate_left(root, Some(x));
                    parent = self.at(x).parent;
                    grand = parent.and_then(|parent| self.at(parent).parent);
                }
                if let Some(parent) = parent {
                    self.at_mut(parent).red = false;
                    if let Some(grand) = grand {
                        self.at_mut(grand).red = true;
                        root = self.rotate_right(root, Some(grand));
                    }
                }
            } else {
                if self.red(xppl) {
                    self.at_mut(xppl.expect("red node")).red = false;
                    self.at_mut(xp).red = false;
                    self.at_mut(xpp).red = true;
                    x = xpp;
                    continue;
                }
                let mut grand = Some(xpp);
                let mut parent = Some(xp);
                if self.at(xp).left == Some(x) {
                    x = xp;
                    root = self.rotate_right(root, Some(x));
                    parent = self.at(x).parent;
                    grand = parent.and_then(|parent| self.at(parent).parent);
                }
                if let Some(parent) = parent {
                    self.at_mut(parent).red = false;
                    if let Some(grand) = grand {
                        self.at_mut(grand).red = true;
                        root = self.rotate_left(root, Some(grand));
                    }
                }
            }
        }
    }

    fn balance_deletion(&mut self, mut root: usize, start: usize) -> usize {
        let mut x = Some(start);
        loop {
            let Some(current) = x else {
                return root;
            };
            if current == root {
                return root;
            }
            let Some(xp) = self.at(current).parent else {
                self.at_mut(current).red = false;
                return current;
            };
            if self.at(current).red {
                self.at_mut(current).red = false;
                return root;
            }
            if self.at(xp).left == Some(current) {
                let mut parent = Some(xp);
                let mut xpr = self.at(xp).right;
                if self.red(xpr) {
                    self.at_mut(xpr.expect("red node")).red = false;
                    self.at_mut(xp).red = true;
                    root = self.rotate_left(root, Some(xp));
                    parent = self.at(current).parent;
                    xpr = parent.and_then(|parent| self.at(parent).right);
                }
                let Some(sibling) = xpr else {
                    x = parent;
                    continue;
                };
                let sl = self.at(sibling).left;
                let sr = self.at(sibling).right;
                if !self.red(sr) && !self.red(sl) {
                    self.at_mut(sibling).red = true;
                    x = parent;
                    continue;
                }
                let mut sibling = Some(sibling);
                if !self.red(sr) {
                    if let Some(sl) = sl {
                        self.at_mut(sl).red = false;
                    }
                    self.at_mut(sibling.expect("set above")).red = true;
                    root = self.rotate_right(root, sibling);
                    parent = self.at(current).parent;
                    sibling = parent.and_then(|parent| self.at(parent).right);
                }
                if let Some(sibling) = sibling {
                    self.at_mut(sibling).red = parent.is_some_and(|parent| self.at(parent).red);
                    if let Some(sr) = self.at(sibling).right {
                        self.at_mut(sr).red = false;
                    }
                }
                if let Some(parent) = parent {
                    self.at_mut(parent).red = false;
                    root = self.rotate_left(root, Some(parent));
                }
                x = Some(root);
            } else {
                let mut parent = Some(xp);
                let mut xpl = self.at(xp).left;
                if self.red(xpl) {
                    self.at_mut(xpl.expect("red node")).red = false;
                    self.at_mut(xp).red = true;
                    root = self.rotate_right(root, Some(xp));
                    parent = self.at(current).parent;
                    xpl = parent.and_then(|parent| self.at(parent).left);
                }
                let Some(sibling) = xpl else {
                    x = parent;
                    continue;
                };
                let sl = self.at(sibling).left;
                let sr = self.at(sibling).right;
                if !self.red(sl) && !self.red(sr) {
                    self.at_mut(sibling).red = true;
                    x = parent;
                    continue;
                }
                let mut sibling = Some(sibling);
                if !self.red(sl) {
                    if let Some(sr) = sr {
                        self.at_mut(sr).red = false;
                    }
                    self.at_mut(sibling.expect("set above")).red = true;
                    root = self.rotate_left(root, sibling);
                    parent = self.at(current).parent;
                    sibling = parent.and_then(|parent| self.at(parent).left);
                }
                if let Some(sibling) = sibling {
                    self.at_mut(sibling).red = parent.is_some_and(|parent| self.at(parent).red);
                    if let Some(sl) = self.at(sibling).left {
                        self.at_mut(sl).red = false;
                    }
                }
                if let Some(parent) = parent {
                    self.at_mut(parent).red = false;
                    root = self.rotate_right(root, Some(parent));
                }
                x = Some(root);
            }
        }
    }
}
