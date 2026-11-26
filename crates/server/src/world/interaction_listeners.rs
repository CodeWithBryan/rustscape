//! InteractionListeners' registries and modern/legacy listener orchestration.
//! Native function objects retain identity; scheduling and entity services are
//! explicit. No registry/list guard spans a gameplay callback.
use super::component_objects::{NativeObject, Plugin};
use crate::definitions::item::Shared;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};
pub type Text = Vec<u16>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    Type,
    Concurrent,
    State(Text),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Item,
    Scenery,
    Npc,
    GroundItem,
    Player,
}
impl Kind {
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    fn name(n: i32) -> Result<&'static str> {
        Ok(match n {
            0 => "ITEM",
            1 => "SCENERY",
            2 => "NPC",
            3 => "GROUNDITEM",
            4 => "PLAYER",
            _ => return Err(Error::Index),
        })
    }
    fn flag(self) -> Option<Flag> {
        match self {
            Self::Npc | Self::Player => Some(Flag::Entity),
            Self::GroundItem => Some(Flag::Item),
            Self::Scenery => None,
            Self::Item => Some(Flag::Object),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Entity,
    Item,
    Object,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionKind {
    On,
    UseWith,
    Predicate,
    Destination,
    Interaction,
    UseInteraction,
    Void,
}
#[derive(Clone)]
pub struct Function {
    header: Plugin,
    kind: FunctionKind,
    class: String,
}
impl Function {
    pub fn new<T: NativeObject>(kind: FunctionKind, class: impl Into<String>, value: T) -> Self {
        Self {
            header: Plugin::new(value),
            kind,
            class: class.into(),
        }
    }
    pub fn value<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.header.value()
    }
    pub fn header(&self) -> Plugin {
        self.header.clone()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.header.same(&other.header)
    }
    pub fn kind(&self) -> FunctionKind {
        self.kind
    }
    fn check(&self, kind: FunctionKind) -> Result<()> {
        if self.kind == kind {
            Ok(())
        } else {
            Err(Error::Type)
        }
    }
}
#[derive(Clone)]
pub struct Metadata {
    pub function: Function,
    pub distance: i32,
    pub persist: bool,
}
pub type Meta = Shared<Metadata>;
#[derive(Clone)]
pub struct Pair {
    pub predicate: Function,
    pub handler: Function,
}
#[derive(Default)]
struct Wildcards {
    pairs: Vec<Pair>,
    changes: i32,
}
#[derive(Default)]
struct Fields {
    listeners: HashMap<Text, Function>,
    use_with: HashMap<Text, Function>,
    use_any: HashMap<Text, Function>,
    wildcard: HashMap<i32, Shared<Wildcards>>,
    destinations: HashMap<Text, Function>,
    equip: HashMap<Text, Function>,
    interactions: HashMap<Text, Meta>,
    use_interactions: HashMap<Text, Meta>,
}
#[derive(Clone)]
pub struct Registry {
    fields: Shared<Fields>,
    pub instant_classes: Shared<HashSet<String>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            fields: Shared::new(Fields::default()),
            instant_classes: Shared::new(HashSet::new()),
        }
    }
}
fn text(s: &str) -> Text {
    s.encode_utf16().collect()
}
fn key(prefix: impl std::fmt::Display, suffix: &[u16]) -> Text {
    let mut out = text(&prefix.to_string());
    out.extend_from_slice(suffix);
    out
}
pub trait Casing {
    fn lowercase_default(&self, s: &[u16]) -> Text;
    fn lowercase_root(&self, s: &[u16]) -> Text;
}
/// Detached diagnostic views of actual objects. This never establishes original
/// HashMap iteration order or installs observer identities as runtime handlers.
pub enum Entry {
    Function(Function),
    Metadata(Meta),
    Wildcards(Vec<Pair>),
}
impl Registry {
    pub fn inspect(&self) -> Vec<(&'static str, Vec<(Text, Entry)>)> {
        let f = self.fields.read();
        let functions = |m: &HashMap<Text, Function>| {
            m.iter()
                .map(|(k, v)| (k.clone(), Entry::Function(v.clone())))
                .collect()
        };
        let metadata = |m: &HashMap<Text, Meta>| {
            m.iter()
                .map(|(k, v)| (k.clone(), Entry::Metadata(v.clone())))
                .collect()
        };
        vec![
            ("listeners", functions(&f.listeners)),
            ("useWithListeners", functions(&f.use_with)),
            ("useAnyWithListeners", functions(&f.use_any)),
            ("destinationOverrides", functions(&f.destinations)),
            ("equipListeners", functions(&f.equip)),
            ("interactions", metadata(&f.interactions)),
            ("useWithInteractions", metadata(&f.use_interactions)),
            (
                "useWithWildcardListeners",
                f.wildcard
                    .iter()
                    .map(|(k, v)| {
                        (
                            text(&k.to_string()),
                            Entry::Wildcards(v.read().pairs.clone()),
                        )
                    })
                    .collect(),
            ),
        ]
    }
    fn validate(&self, k: &Text) -> bool {
        let f = self.fields.read();
        !f.listeners.contains_key(k) && !f.use_with.contains_key(k)
    }
    pub fn add(
        &self,
        c: &impl Casing,
        id: i32,
        kind: i32,
        options: &[Text],
        method: Function,
    ) -> Result<()> {
        method.check(FunctionKind::On)?;
        for opt in options {
            let k = key(format!("{id}:{kind}:"), &c.lowercase_default(opt));
            if !self.validate(&k) {
                let ty = Kind::name(kind)?;
                let old = self
                    .fields
                    .read()
                    .listeners
                    .get(&k)
                    .cloned()
                    .ok_or(Error::Null)?;
                let mut m = opt.clone();
                m.extend(text(&format!(
                    " for {id} with type {ty} already defined! Existing use: [class {}]",
                    old.class
                )));
                return Err(Error::State(m));
            }
            self.fields.write().listeners.insert(k, method.clone());
        }
        Ok(())
    }
    pub fn add_ids(
        &self,
        c: &impl Casing,
        ids: &[i32],
        kind: i32,
        options: &[Text],
        method: Function,
    ) -> Result<()> {
        for id in ids {
            self.add(c, *id, kind, options, method.clone())?;
        }
        Ok(())
    }
    pub fn add_generic(
        &self,
        c: &impl Casing,
        option: &[u16],
        kind: i32,
        method: Function,
    ) -> Result<()> {
        method.check(FunctionKind::On)?;
        let k = key(format!("{kind}:"), &c.lowercase_default(option));
        if !self.validate(&k) {
            let ty = Kind::name(kind)?;
            let old = self
                .fields
                .read()
                .listeners
                .get(&k)
                .cloned()
                .ok_or(Error::Null)?;
            let mut m = text("Catchall listener for ");
            m.extend(option);
            m.extend(text(&format!(
                " on type {ty} already in use: class {}",
                old.class
            )));
            return Err(Error::State(m));
        }
        self.fields.write().listeners.insert(k, method);
        Ok(())
    }
    pub fn add_generics(
        &self,
        c: &impl Casing,
        options: &[Text],
        kind: i32,
        method: Function,
    ) -> Result<()> {
        for option in options {
            self.add_generic(c, &c.lowercase_default(option), kind, method.clone())?;
        }
        Ok(())
    }
    pub fn add_use(&self, used: i32, with: i32, kind: i32, method: Function) -> Result<()> {
        method.check(FunctionKind::UseWith)?;
        let k = text(&format!("{used}:{with}:{kind}"));
        let alt = text(&format!("{with}:{used}:{kind}"));
        if !self.validate(&k) || !self.validate(&alt) {
            let ty = Kind::name(kind)?;
            let f = self.fields.read();
            let old = f
                .use_with
                .get(&k)
                .or_else(|| f.use_with.get(&alt))
                .ok_or(Error::Null)?;
            return Err(Error::State(text(&format!(
                "Usewith using {used} with {with} for type {ty} already in use: [class {}",
                old.class
            ))));
        }
        self.fields.write().use_with.insert(k, method);
        Ok(())
    }
    pub fn add_use_ids(&self, kind: i32, used: i32, with: &[i32], method: Function) -> Result<()> {
        for id in with {
            self.add_use(used, *id, kind, method.clone())?;
        }
        Ok(())
    }
    // Unlike the scalar overload, this overload deliberately overwrites without
    // validation. Preserve the original overload's distinct behavior.
    pub fn add_use_arrays(
        &self,
        kind: i32,
        used: &[i32],
        with: &[i32],
        method: Function,
    ) -> Result<()> {
        method.check(FunctionKind::UseWith)?;
        for u in used {
            for w in with {
                self.fields
                    .write()
                    .use_with
                    .insert(text(&format!("{u}:{w}:{kind}")), method.clone());
            }
        }
        Ok(())
    }
    pub fn add_any(&self, kind: i32, with: &[i32], method: Function) -> Result<()> {
        method.check(FunctionKind::UseWith)?;
        for w in with {
            self.fields
                .write()
                .use_any
                .insert(text(&format!("{w}:{kind}")), method.clone());
        }
        Ok(())
    }
    pub fn add_wildcard(&self, kind: i32, predicate: Function, handler: Function) -> Result<()> {
        predicate.check(FunctionKind::Predicate)?;
        handler.check(FunctionKind::UseWith)?;
        let list = self
            .fields
            .write()
            .wildcard
            .entry(kind)
            .or_insert_with(|| Shared::new(Wildcards::default()))
            .clone();
        let mut list = list.write();
        list.pairs.push(Pair { predicate, handler });
        list.changes = list.changes.wrapping_add(1);
        Ok(())
    }
    pub fn add_equip(&self, id: i32, method: Function, equip: bool) -> Result<()> {
        method.check(FunctionKind::On)?;
        self.fields.write().equip.insert(
            text(&format!("{}:{id}", if equip { "equip" } else { "unequip" })),
            method,
        );
        Ok(())
    }
    pub fn get_equip(&self, id: i32, equip: bool) -> Option<Function> {
        self.fields
            .read()
            .equip
            .get(&text(&format!(
                "{}:{id}",
                if equip { "equip" } else { "unequip" }
            )))
            .cloned()
    }
    pub fn get(&self, c: &impl Casing, id: i32, kind: i32, option: &[u16]) -> Option<Function> {
        self.fields
            .read()
            .listeners
            .get(&key(format!("{id}:{kind}:"), &c.lowercase_default(option)))
            .cloned()
    }
    pub fn get_generic(&self, c: &impl Casing, option: &[u16], kind: i32) -> Option<Function> {
        self.fields
            .read()
            .listeners
            .get(&key(format!("{kind}:"), &c.lowercase_default(option)))
            .cloned()
    }
    pub fn get_use(
        &self,
        c: &mut impl Context,
        used: i32,
        with: i32,
        kind: i32,
    ) -> Result<Option<Function>> {
        let (method, list) = {
            let f = self.fields.read();
            (
                f.use_with
                    .get(&text(&format!("{used}:{with}:{kind}")))
                    .or_else(|| f.use_any.get(&text(&format!("{with}:{kind}"))))
                    .cloned(),
                f.wildcard.get(&kind).cloned(),
            )
        };
        if method.is_some() {
            return Ok(method);
        }
        let Some(list) = list else { return Ok(None) };
        let expected = list.read().changes;
        let mut cursor = 0;
        loop {
            // ArrayList.hasNext does not inspect modCount. A predicate that adds
            // a row and returns true can still return its captured handler.
            let pair = {
                let f = list.read();
                if cursor == f.pairs.len() {
                    return Ok(None);
                }
                if f.changes != expected {
                    return Err(Error::Concurrent);
                }
                f.pairs.get(cursor).cloned().ok_or(Error::Index)?
            };
            cursor += 1;
            if c.predicate(pair.predicate, used, with)? {
                return Ok(Some(pair.handler));
            }
        }
    }
    pub fn add_destination(&self, kind: i32, id: i32, method: Function) -> Result<()> {
        method.check(FunctionKind::Destination)?;
        self.fields
            .write()
            .destinations
            .insert(text(&format!("{kind}:{id}")), method);
        Ok(())
    }
    pub fn add_destinations(
        &self,
        c: &impl Casing,
        kind: i32,
        ids: Option<&[i32]>,
        options: &[Text],
        method: Function,
    ) -> Result<()> {
        method.check(FunctionKind::Destination)?;
        match ids {
            Some(ids) => {
                for id in ids {
                    for opt in options {
                        self.fields.write().destinations.insert(
                            key(format!("{kind}:{id}:"), &c.lowercase_default(opt)),
                            method.clone(),
                        );
                    }
                }
            }
            None => {
                for opt in options {
                    self.fields.write().destinations.insert(
                        key(format!("{kind}:"), &c.lowercase_default(opt)),
                        method.clone(),
                    );
                }
            }
        }
        Ok(())
    }
    pub fn destination(
        &self,
        c: &impl Casing,
        kind: i32,
        id: i32,
        option: &[u16],
    ) -> Option<Function> {
        self.fields
            .read()
            .destinations
            .get(&key(format!("{kind}:{id}:"), &c.lowercase_default(option)))
            .cloned()
    }
    pub fn destination_id(&self, kind: i32, id: i32) -> Option<Function> {
        self.fields
            .read()
            .destinations
            .get(&text(&format!("{kind}:{id}")))
            .cloned()
    }
    /// This overload does not lowercase. Callers sometimes lowercase first.
    pub fn destination_generic(&self, kind: i32, option: &[u16]) -> Option<Function> {
        self.fields
            .read()
            .destinations
            .get(&key(format!("{kind}:"), option))
            .cloned()
    }
    pub fn add_metadata(
        &self,
        c: &impl Casing,
        ids: &[i32],
        kind: Kind,
        options: &[Text],
        meta: Meta,
    ) {
        for id in ids {
            for opt in options {
                self.fields.write().interactions.insert(
                    key(format!("{}:{id}:", kind.ordinal()), &c.lowercase_root(opt)),
                    meta.clone(),
                );
            }
        }
    }
    /// Generic metadata registration retains the original option casing.
    pub fn add_generic_metadata(&self, kind: Kind, options: &[Text], meta: Meta) {
        for opt in options {
            self.fields
                .write()
                .interactions
                .insert(key(format!("{}:", kind.ordinal()), opt), meta.clone());
        }
    }
    pub fn add_use_metadata(&self, used: i32, with: i32, kind: Kind, meta: Meta) {
        self.fields
            .write()
            .use_interactions
            .insert(text(&format!("{}:{used}:{with}", kind.ordinal())), meta);
    }
    /// Kotlin interpolates the IntArray object, not its current loop element.
    /// Its VM identity hash is explicit because it is platform-specific input.
    pub fn add_use_metadata_array(
        &self,
        used: i32,
        with: &[i32],
        array_identity_hash: u32,
        kind: Kind,
        meta: Meta,
    ) {
        for _ in with {
            self.fields.write().use_interactions.insert(
                text(&format!(
                    "{}:{used}:[I@{array_identity_hash:x}",
                    kind.ordinal()
                )),
                meta.clone(),
            );
        }
    }
    /// `defineInteraction`'s metadata for an id and option, then the generic one.
    pub fn metadata(&self, c: &impl Casing, id: i32, kind: Kind, option: &[u16]) -> Option<Meta> {
        let specific = key(
            format!("{}:{id}:", kind.ordinal()),
            &c.lowercase_root(option),
        );
        if let Some(m) = self.fields.read().interactions.get(&specific).cloned() {
            return Some(m);
        }
        let generic = key(format!("{}:", kind.ordinal()), &c.lowercase_root(option));
        self.fields.read().interactions.get(&generic).cloned()
    }
    pub fn is_instant(&self, f: &Function) -> bool {
        let class = f.class.split('$').next().unwrap();
        self.instant_classes.read().contains(class)
    }
    pub fn run_equip<C: Context>(
        &self,
        c: &mut C,
        id: i32,
        p: C::Player,
        node: C::Node,
        equip: bool,
    ) -> Result<bool> {
        let scripts = c.scripts(&p)?;
        c.remove_weak(scripts)?;
        c.dispatch_on(&p, &node, text(if equip { "equip" } else { "unequip" }))?;
        match self.get_equip(id, equip) {
            Some(f) => c.invoke_on(f, &p, &node),
            None => Ok(true),
        }
    }
    pub fn run<C: Context>(
        &self,
        c: &mut C,
        id: i32,
        kind: Kind,
        option: &[u16],
        p: C::Player,
        node: C::Node,
    ) -> Result<bool> {
        let scripts = c.scripts(&p)?;
        c.remove_weak(scripts)?;
        let locks = c.locks(&p)?;
        if c.interaction_locked(locks)? {
            return Ok(false);
        }
        let method = self
            .get(c, id, kind.ordinal(), option)
            .or_else(|| self.get_generic(c, option, kind.ordinal()));
        let lower = c.lowercase_root(option);
        c.set_option(&p, lower)?;
        let lower = c.lowercase_default(option);
        c.dispatch_on(&p, &node, lower)?;
        let Some(method) = method else {
            let Some(meta) = self.metadata(c, id, kind, option) else {
                return Ok(false);
            };
            let m = meta.read().clone();
            let script = c.interaction(m.function, m.distance, m.persist)?;
            let scripts = c.scripts(&p)?;
            c.install(scripts, &node, script)?;
            let manager = c.manager(&p)?;
            let pulse = c.movement(
                Arrival::Empty {
                    player: p.clone(),
                    target: node,
                },
                kind.flag(),
                None,
            )?;
            c.run(manager, pulse)?;
            return Ok(true);
        };
        let mut dest = self.destination(c, kind.ordinal(), id, option);
        if dest.is_none() {
            let node_id = c.id(&node)?;
            dest = self.destination_id(kind.ordinal(), node_id);
        }
        if dest.is_none() {
            dest = self.destination_generic(kind.ordinal(), &c.lowercase_default(option));
        }
        if kind != Kind::Item && !self.is_instant(&method) {
            let locks = c.locks(&p)?;
            if c.movement_locked(locks)? {
                return Ok(false);
            }
            let manager = c.manager(&p)?;
            let pulse = c.movement(
                Arrival::On {
                    player: p,
                    target: node,
                    method,
                    option: option.to_vec(),
                },
                kind.flag(),
                dest,
            )?;
            c.run(manager, pulse)?;
        } else {
            c.invoke_on(method, &p, &node)?;
        }
        Ok(true)
    }
    pub fn run_use<C: Context>(
        &self,
        c: &mut C,
        used: C::Node,
        with: C::Node,
        kind: Kind,
        p: C::Player,
    ) -> Result<bool> {
        let scripts = c.scripts(&p)?;
        c.remove_weak(scripts)?;
        let flag = match kind {
            Kind::Npc | Kind::Player => Flag::Entity,
            Kind::GroundItem => Flag::Item,
            _ => Flag::Object,
        };
        let locks = c.locks(&p)?;
        if c.interaction_locked(locks)? {
            return Ok(false);
        }
        let mut flipped = false;
        let method = if c.is_player(&with) {
            let id = c.id(&used)?;
            self.get_use(c, -1, id, 4)?
        } else {
            let u = c.id(&used)?;
            let w = c.id(&with)?;
            let first = self.get_use(c, u, w, kind.ordinal())?;
            match first {
                Some(f) => Some(f),
                None if kind == Kind::Item => {
                    let w = c.id(&with)?;
                    let u = c.id(&used)?;
                    let f = self.get_use(c, w, u, kind.ordinal())?;
                    flipped = true;
                    f
                }
                None => None,
            }
        };
        let Some(method) = method else {
            return Ok(false);
        };
        let first = if flipped { c.id(&used)? } else { c.id(&with)? };
        let mut dest = self.destination(c, kind.ordinal(), first, &text("use"));
        if dest.is_none() {
            let id = if flipped { c.id(&with)? } else { c.id(&used)? };
            dest = self.destination_id(kind.ordinal(), id);
        }
        if dest.is_none() {
            dest = self.destination_generic(kind.ordinal(), &text("use"));
        }
        if kind != Kind::Item && !self.is_instant(&method) {
            let locks = c.locks(&p)?;
            if c.movement_locked(locks)? {
                return Ok(false);
            }
            let manager = c.manager(&p)?;
            let pulse = c.movement(
                Arrival::Use {
                    player: p,
                    used,
                    with,
                    flipped,
                    method,
                },
                Some(flag),
                dest,
            )?;
            c.run(manager, pulse)?;
            Ok(true)
        } else {
            let (a, b) = if flipped {
                (&with, &used)
            } else {
                (&used, &with)
            };
            let ia = c.id(a)?;
            let ib = c.id(b)?;
            c.dispatch_use(&p, ia, ib)?;
            c.invoke_use(method, &p, a, b)
        }
    }
}
#[derive(Clone)]
pub enum Arrival<P, N> {
    Empty {
        player: P,
        target: N,
    },
    On {
        player: P,
        target: N,
        method: Function,
        option: Text,
    },
    Use {
        player: P,
        used: N,
        with: N,
        flipped: bool,
        method: Function,
    },
}
pub trait Context: Casing {
    type Player: Clone;
    type Node: Clone;
    type Scripts;
    type Locks;
    type Script;
    type Manager;
    type Pulse;
    type Zone;
    type Location;
    fn scripts(&mut self, p: &Self::Player) -> Result<Self::Scripts>;
    fn remove_weak(&mut self, s: Self::Scripts) -> Result<()>;
    fn locks(&mut self, p: &Self::Player) -> Result<Self::Locks>;
    fn interaction_locked(&mut self, l: Self::Locks) -> Result<bool>;
    fn movement_locked(&mut self, l: Self::Locks) -> Result<bool>;
    fn set_option(&mut self, p: &Self::Player, option: Text) -> Result<()>;
    fn dispatch_on(&mut self, p: &Self::Player, n: &Self::Node, option: Text) -> Result<()>;
    fn dispatch_use(&mut self, p: &Self::Player, used: i32, with: i32) -> Result<()>;
    fn interaction(&mut self, f: Function, distance: i32, persist: bool) -> Result<Self::Script>;
    fn install(&mut self, s: Self::Scripts, n: &Self::Node, script: Self::Script) -> Result<()>;
    fn manager(&mut self, p: &Self::Player) -> Result<Self::Manager>;
    fn movement(
        &mut self,
        a: Arrival<Self::Player, Self::Node>,
        flag: Option<Flag>,
        dest: Option<Function>,
    ) -> Result<Self::Pulse>;
    fn run(&mut self, m: Self::Manager, p: Self::Pulse) -> Result<()>;
    fn invoke_on(&mut self, f: Function, p: &Self::Player, n: &Self::Node) -> Result<bool>;
    fn invoke_use(
        &mut self,
        f: Function,
        p: &Self::Player,
        u: &Self::Node,
        w: &Self::Node,
    ) -> Result<bool>;
    fn predicate(&mut self, f: Function, u: i32, w: i32) -> Result<bool>;
    fn id(&mut self, n: &Self::Node) -> Result<i32>;
    fn is_player(&self, n: &Self::Node) -> bool;
    fn zone(&mut self, p: &Self::Player) -> Result<Self::Zone>;
    fn zone_on(&mut self, z: Self::Zone, n: &Self::Node, option: Text, index: i32) -> Result<bool>;
    fn as_item(&mut self, n: &Self::Node) -> Result<Self::Node>;
    fn zone_use(&mut self, z: Self::Zone, u: &Self::Node, w: &Self::Node) -> Result<bool>;
    fn location(&mut self, n: &Self::Node) -> Result<Self::Location>;
    fn face(&mut self, p: &Self::Player, l: Self::Location) -> Result<()>;
}
pub fn arrival<C: Context>(c: &mut C, a: Arrival<C::Player, C::Node>) -> Result<bool> {
    match a {
        Arrival::Empty { .. } => {}
        Arrival::On {
            player,
            target,
            method,
            option,
        } => {
            let z = c.zone(&player)?;
            if c.zone_on(z, &target, option, 0)? {
                return Ok(true);
            }
            let l = c.location(&target)?;
            c.face(&player, l)?;
            c.invoke_on(method, &player, &target)?;
        }
        Arrival::Use {
            player,
            used,
            with,
            flipped,
            method,
        } => {
            let z = c.zone(&player)?;
            let item = c.as_item(&used)?;
            if c.zone_use(z, &item, &with)? {
                return Ok(true);
            }
            let l = c.location(&with)?;
            c.face(&player, l)?;
            let (a, b) = if flipped {
                (&with, &used)
            } else {
                (&used, &with)
            };
            let ia = c.id(a)?;
            let ib = c.id(b)?;
            c.dispatch_use(&player, ia, ib)?;
            c.invoke_use(method, &player, a, b)?;
        }
    }
    Ok(true)
}
