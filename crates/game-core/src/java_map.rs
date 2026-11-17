//! JDK HashMap insertion/resize/tree-bin key iteration for String/Integer keys.
//! This narrow helper preserves order consumed by original game routines; it is
//! not a replacement for all Java collection APIs.
use std::cmp::Ordering;

#[derive(Clone, Default)]
struct Bucket {
    head: Option<usize>,
    tree: bool,
}
struct Node {
    key: String,
    hash: u32,
    next: Option<usize>,
    prev: Option<usize>,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
    red: bool,
}
struct Map {
    buckets: Vec<Bucket>,
    nodes: Vec<Node>,
}
fn hash(key: &str) -> u32 {
    let value = key.encode_utf16().fold(0u32, |hash, ch| {
        hash.wrapping_mul(31).wrapping_add(u32::from(ch))
    });
    value ^ (value >> 16)
}
impl Map {
    fn new() -> Self {
        Self {
            buckets: vec![Bucket::default(); 16],
            nodes: Vec::new(),
        }
    }
    fn compare(&self, a: usize, b: usize) -> Ordering {
        (self.nodes[a].hash as i32)
            .cmp(&(self.nodes[b].hash as i32))
            .then_with(|| {
                self.nodes[a]
                    .key
                    .encode_utf16()
                    .cmp(self.nodes[b].key.encode_utf16())
            })
    }
    fn red(&self, node: Option<usize>) -> bool {
        node.is_some_and(|node| self.nodes[node].red)
    }
    fn rotate_left(&mut self, mut root: usize, p: usize) -> usize {
        let Some(r) = self.nodes[p].right else {
            return root;
        };
        let child = self.nodes[r].left;
        self.nodes[p].right = child;
        if let Some(child) = child {
            self.nodes[child].parent = Some(p);
        }
        let parent = self.nodes[p].parent;
        self.nodes[r].parent = parent;
        if let Some(parent) = parent {
            if self.nodes[parent].left == Some(p) {
                self.nodes[parent].left = Some(r);
            } else {
                self.nodes[parent].right = Some(r);
            }
        } else {
            root = r;
            self.nodes[r].red = false;
        }
        self.nodes[r].left = Some(p);
        self.nodes[p].parent = Some(r);
        root
    }
    fn rotate_right(&mut self, mut root: usize, p: usize) -> usize {
        let Some(l) = self.nodes[p].left else {
            return root;
        };
        let child = self.nodes[l].right;
        self.nodes[p].left = child;
        if let Some(child) = child {
            self.nodes[child].parent = Some(p);
        }
        let parent = self.nodes[p].parent;
        self.nodes[l].parent = parent;
        if let Some(parent) = parent {
            if self.nodes[parent].right == Some(p) {
                self.nodes[parent].right = Some(l);
            } else {
                self.nodes[parent].left = Some(l);
            }
        } else {
            root = l;
            self.nodes[l].red = false;
        }
        self.nodes[l].right = Some(p);
        self.nodes[p].parent = Some(l);
        root
    }
    fn balance(&mut self, mut root: usize, mut x: usize) -> usize {
        self.nodes[x].red = true;
        loop {
            let Some(parent) = self.nodes[x].parent else {
                self.nodes[x].red = false;
                return x;
            };
            if !self.nodes[parent].red {
                return root;
            }
            let Some(grand) = self.nodes[parent].parent else {
                return root;
            };
            if self.nodes[grand].left == Some(parent) {
                let uncle = self.nodes[grand].right;
                if self.red(uncle) {
                    self.nodes[parent].red = false;
                    self.nodes[uncle.unwrap()].red = false;
                    self.nodes[grand].red = true;
                    x = grand;
                    continue;
                }
                if self.nodes[parent].right == Some(x) {
                    x = parent;
                    root = self.rotate_left(root, x);
                }
                let parent = self.nodes[x].parent.unwrap();
                let grand = self.nodes[parent].parent;
                self.nodes[parent].red = false;
                if let Some(grand) = grand {
                    self.nodes[grand].red = true;
                    root = self.rotate_right(root, grand);
                }
                return root;
            } else {
                let uncle = self.nodes[grand].left;
                if self.red(uncle) {
                    self.nodes[parent].red = false;
                    self.nodes[uncle.unwrap()].red = false;
                    self.nodes[grand].red = true;
                    x = grand;
                    continue;
                }
                if self.nodes[parent].left == Some(x) {
                    x = parent;
                    root = self.rotate_right(root, x);
                }
                let parent = self.nodes[x].parent.unwrap();
                let grand = self.nodes[parent].parent;
                self.nodes[parent].red = false;
                if let Some(grand) = grand {
                    self.nodes[grand].red = true;
                    root = self.rotate_left(root, grand);
                }
                return root;
            }
        }
    }
    fn root_to_front(&mut self, bucket: usize, root: usize) {
        let first = self.buckets[bucket].head.unwrap();
        if first == root {
            return;
        }
        let prev = self.nodes[root].prev;
        let next = self.nodes[root].next;
        if let Some(prev) = prev {
            self.nodes[prev].next = next;
        }
        if let Some(next) = next {
            self.nodes[next].prev = prev;
        }
        self.nodes[first].prev = Some(root);
        self.nodes[root].next = Some(first);
        self.nodes[root].prev = None;
        self.buckets[bucket].head = Some(root);
    }
    fn treeify(&mut self, bucket: usize) {
        let mut cursor = self.buckets[bucket].head;
        let mut root = None;
        while let Some(x) = cursor {
            cursor = self.nodes[x].next;
            self.nodes[x].parent = None;
            self.nodes[x].left = None;
            self.nodes[x].right = None;
            if let Some(r) = root {
                let mut parent = r;
                loop {
                    let left = self.compare(x, parent) == Ordering::Less;
                    let child = if left {
                        self.nodes[parent].left
                    } else {
                        self.nodes[parent].right
                    };
                    if let Some(child) = child {
                        parent = child;
                    } else {
                        self.nodes[x].parent = Some(parent);
                        if left {
                            self.nodes[parent].left = Some(x);
                        } else {
                            self.nodes[parent].right = Some(x);
                        }
                        break;
                    }
                }
                root = Some(self.balance(r, x));
            } else {
                self.nodes[x].red = false;
                root = Some(x);
            }
        }
        self.buckets[bucket].tree = true;
        self.root_to_front(bucket, root.unwrap());
    }
    fn resize(&mut self) {
        let old = std::mem::take(&mut self.buckets);
        let count = old.len();
        self.buckets = vec![Bucket::default(); count * 2];
        for (bucket, old) in old.into_iter().enumerate() {
            let mut lo = Vec::new();
            let mut hi = Vec::new();
            let mut cursor = old.head;
            while let Some(node) = cursor {
                cursor = self.nodes[node].next;
                if self.nodes[node].hash as usize & count == 0 {
                    lo.push(node);
                } else {
                    hi.push(node);
                }
            }
            let split = !lo.is_empty() && !hi.is_empty();
            for (target, nodes) in [(bucket, lo), (bucket + count, hi)] {
                if nodes.is_empty() {
                    continue;
                }
                self.buckets[target].head = Some(nodes[0]);
                for (i, &node) in nodes.iter().enumerate() {
                    self.nodes[node].prev = i.checked_sub(1).map(|i| nodes[i]);
                    self.nodes[node].next = nodes.get(i + 1).copied();
                }
                if old.tree && nodes.len() > 6 {
                    self.buckets[target].tree = true;
                    if split {
                        self.treeify(target);
                    }
                }
            }
        }
    }
    fn insert(&mut self, key: String) -> usize {
        let hash = hash(&key);
        self.insert_hashed(key, hash)
    }
    fn insert_hashed(&mut self, key: String, hash: u32) -> usize {
        let bucket = hash as usize & (self.buckets.len() - 1);
        let mut cursor = self.buckets[bucket].head;
        let mut tail = None;
        let mut count = 0;
        while let Some(node) = cursor {
            if self.nodes[node].key == key {
                return node;
            }
            tail = Some(node);
            count += 1;
            cursor = self.nodes[node].next;
        }
        let node = self.nodes.len();
        self.nodes.push(Node {
            key,
            hash,
            next: None,
            prev: tail,
            parent: None,
            left: None,
            right: None,
            red: false,
        });
        if let Some(head) = self.buckets[bucket].head {
            if self.buckets[bucket].tree {
                let mut parent = head;
                loop {
                    let left = self.compare(node, parent) == Ordering::Less;
                    let child = if left {
                        self.nodes[parent].left
                    } else {
                        self.nodes[parent].right
                    };
                    if let Some(child) = child {
                        parent = child;
                    } else {
                        let next = self.nodes[parent].next;
                        self.nodes[node].next = next;
                        self.nodes[node].prev = Some(parent);
                        self.nodes[node].parent = Some(parent);
                        self.nodes[parent].next = Some(node);
                        if let Some(next) = next {
                            self.nodes[next].prev = Some(node);
                        }
                        if left {
                            self.nodes[parent].left = Some(node);
                        } else {
                            self.nodes[parent].right = Some(node);
                        }
                        break;
                    }
                }
                let root = self.balance(head, node);
                self.root_to_front(bucket, root);
            } else {
                self.nodes[tail.unwrap()].next = Some(node);
                if count >= 8 {
                    if self.buckets.len() < 64 {
                        self.resize();
                    } else {
                        self.treeify(bucket);
                    }
                }
            }
        } else {
            self.buckets[bucket].head = Some(node);
        }
        if self.nodes.len() > self.buckets.len() * 3 / 4 {
            self.resize();
        }
        node
    }
}
/// Insertion, lookup and ordered iteration only. Node indices survive resize.
#[derive(Default)]
pub struct StringKeyOrder {
    map: Option<Map>,
}
impl StringKeyOrder {
    pub fn insert(&mut self, key: String) -> usize {
        self.map.get_or_insert_with(Map::new).insert(key)
    }
    pub fn get(&self, key: &str) -> Option<usize> {
        let map = self.map.as_ref()?;
        let mut cursor = map.buckets[hash(key) as usize & (map.buckets.len() - 1)].head;
        while let Some(node) = cursor {
            if map.nodes[node].key == key {
                return Some(node);
            }
            cursor = map.nodes[node].next;
        }
        None
    }
    pub fn iter(&self) -> impl Iterator<Item = (usize, &str)> {
        self.map.iter().flat_map(|map| {
            let (mut bucket, mut cursor) = (0, None);
            std::iter::from_fn(move || {
                while cursor.is_none() {
                    cursor = map.buckets.get(bucket)?.head;
                    bucket += 1;
                }
                let node = cursor?;
                cursor = map.nodes[node].next;
                Some((node, map.nodes[node].key.as_str()))
            })
        })
    }
}
pub fn string_key_order(keys: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut map = Map::new();
    for key in keys {
        map.insert(key);
    }
    let mut result = Vec::with_capacity(map.nodes.len());
    for bucket in &map.buckets {
        let mut cursor = bucket.head;
        while let Some(node) = cursor {
            result.push(map.nodes[node].key.clone());
            cursor = map.nodes[node].next;
        }
    }
    result
}

/// Integer.hashCode is the integer itself. Its spread is bijective, so distinct
/// Integer keys never reach the same-hash String comparison/tie-break branch.
pub fn int_key_order(keys: impl IntoIterator<Item = i32>) -> Vec<i32> {
    int_key_layout(keys, 16).0
}

/// Insertion-only Integer HashMap layout, seeded with a retained table capacity.
/// Java clear() empties buckets without shrinking the table. The caller supplies
/// a previously returned capacity (or16); this is not an arbitrary input parser.
pub fn int_key_layout(keys: impl IntoIterator<Item = i32>, capacity: usize) -> (Vec<i32>, usize) {
    assert!(capacity >= 16 && capacity.is_power_of_two());
    let mut map = Map {
        buckets: vec![Bucket::default(); capacity],
        nodes: Vec::new(),
    };
    for key in keys {
        let bits = key as u32;
        map.insert_hashed(key.to_string(), bits ^ (bits >> 16));
    }
    let mut result = Vec::with_capacity(map.nodes.len());
    for bucket in &map.buckets {
        let mut cursor = bucket.head;
        while let Some(node) = cursor {
            result.push(map.nodes[node].key.parse().unwrap());
            cursor = map.nodes[node].next;
        }
    }
    (result, map.buckets.len())
}
