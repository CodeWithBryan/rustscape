//! InitializingNodeList/InitializationEntry and UpdateSequence. Concrete Player,
//! NPC, chunk, ground-item and telemetry behavior remains an owning-world service.
use crate::definitions::item::Shared;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Index,
    ConcurrentModification,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
struct Values<N> {
    nodes: Vec<N>,
    version: u32,
}
struct Entry<N> {
    node: N,
    removal: bool,
}
#[derive(Clone, Copy)]
enum Kind {
    Initializing,
    Array,
    CopyOnWrite,
}
pub struct List<N> {
    values: Shared<Values<N>>,
    pending: Arc<Mutex<VecDeque<Entry<N>>>>,
    kind: Kind,
}
impl<N> Clone for List<N> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            pending: self.pending.clone(),
            kind: self.kind,
        }
    }
}
impl<N> Default for List<N> {
    fn default() -> Self {
        Self {
            values: Shared::new(Values {
                nodes: vec![],
                version: 0,
            }),
            pending: Arc::new(Mutex::new(VecDeque::new())),
            kind: Kind::Initializing,
        }
    }
}
pub trait Initialization<N> {
    fn active(&mut self, node: &N, active: bool) -> Result<()>;
    fn renderable(&mut self, node: &N, renderable: bool) -> Result<()>;
}
impl<N: Clone + PartialEq> List<N> {
    pub fn plain() -> Self {
        Self {
            kind: Kind::Array,
            ..Self::default()
        }
    }
    /// Repository.RENDERABLE_NPCS uses CopyOnWriteArrayList. Each iterator keeps
    /// its original array; later iterations still see mutations to this handle.
    pub fn copy_on_write() -> Self {
        Self {
            kind: Kind::CopyOnWrite,
            ..Self::default()
        }
    }
    pub fn same(&self, other: &Self) -> bool {
        self.values.same(&other.values)
    }
    pub fn values(&self) -> Vec<N> {
        self.values.read().nodes.clone()
    }
    pub fn pending(&self) -> Vec<(N, bool)> {
        self.pending
            .lock()
            .expect("initialization queue poisoned")
            .iter()
            .map(|e| (e.node.clone(), e.removal))
            .collect()
    }
    pub fn contains(&self, node: &N) -> bool {
        self.values.read().nodes.contains(node)
    }
    pub fn add(&self, node: N) -> bool {
        if !matches!(self.kind, Kind::Initializing) {
            self.push_direct(node);
            return true;
        }
        if self.contains(&node) {
            return false;
        }
        self.pending
            .lock()
            .expect("initialization queue poisoned")
            .push_back(Entry {
                node,
                removal: false,
            });
        true
    }
    pub fn remove(&self, node: &N) -> bool {
        if !matches!(self.kind, Kind::Initializing) {
            let mut v = self.values.write();
            let Some(index) = v.nodes.iter().position(|n| n == node) else {
                return false;
            };
            v.nodes.remove(index);
            v.version = v.version.wrapping_add(1);
            return true;
        }
        if !self.contains(node) {
            return false;
        }
        self.pending
            .lock()
            .expect("initialization queue poisoned")
            .push_back(Entry {
                node: node.clone(),
                removal: true,
            });
        true
    }
    /// Inherited indexed add/addAll operations bypass InitializingNodeList.add.
    pub fn insert(&self, index: usize, node: N) -> Result<()> {
        let mut v = self.values.write();
        if index > v.nodes.len() {
            return Err(Error::Index);
        }
        v.nodes.insert(index, node);
        v.version = v.version.wrapping_add(1);
        Ok(())
    }
    pub fn push_direct(&self, node: N) {
        let mut v = self.values.write();
        v.nodes.push(node);
        v.version = v.version.wrapping_add(1);
    }
    pub fn set(&self, index: usize, node: N) -> Result<N> {
        let mut v = self.values.write();
        Ok(std::mem::replace(
            v.nodes.get_mut(index).ok_or(Error::Index)?,
            node,
        ))
    }
    pub fn remove_at(&self, index: usize) -> Result<N> {
        let mut v = self.values.write();
        if index >= v.nodes.len() {
            return Err(Error::Index);
        }
        v.version = v.version.wrapping_add(1);
        Ok(v.nodes.remove(index))
    }
    pub fn clear(&self) {
        let mut v = self.values.write();
        v.nodes.clear();
        v.version = v.version.wrapping_add(1);
    }
    pub fn sync<C: Initialization<N>>(&self, ctx: &mut C) -> Result<()> {
        loop {
            let e = self
                .pending
                .lock()
                .expect("initialization queue poisoned")
                .pop_front();
            let Some(e) = e else {
                return Ok(());
            };
            if e.removal {
                {
                    let mut v = self.values.write();
                    if let Some(i) = v.nodes.iter().position(|n| *n == e.node) {
                        v.nodes.remove(i);
                        v.version = v.version.wrapping_add(1);
                    }
                }
                ctx.renderable(&e.node, false)?;
            } else {
                ctx.active(&e.node, true)?;
                self.push_direct(e.node.clone());
                ctx.renderable(&e.node, true)?;
            }
        }
    }
    /// Kotlin Iterable.forEach: hasNext reads the live size before next checks
    /// modCount. Removing the last remaining element can terminate silently.
    pub fn each(&self, mut f: impl FnMut(N) -> Result<()>) -> Result<()> {
        if matches!(self.kind, Kind::CopyOnWrite) {
            let nodes = self.values.read().nodes.clone();
            for node in nodes {
                f(node)?;
            }
            return Ok(());
        }
        let version = self.values.read().version;
        let mut cursor = 0;
        loop {
            let node = {
                let v = self.values.read();
                if cursor == v.nodes.len() {
                    return Ok(());
                }
                if version != v.version {
                    return Err(Error::ConcurrentModification);
                }
                v.nodes.get(cursor).cloned().ok_or(Error::Index)?
            };
            cursor += 1;
            f(node)?;
        }
    }
}
pub struct Fields<N> {
    pub lobby: Option<List<N>>,
    pub players: Option<List<N>>,
    pub npcs: Option<List<N>>,
}
pub struct Sequence<N> {
    pub fields: Shared<Fields<N>>,
}
impl<N> Clone for Sequence<N> {
    fn clone(&self) -> Self {
        Self {
            fields: self.fields.clone(),
        }
    }
}
impl<N> Default for Sequence<N> {
    fn default() -> Self {
        Self {
            fields: Shared::new(Fields {
                lobby: None,
                players: None,
                npcs: None,
            }),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    NpcTick,
    PlayerTick,
    PlayerRender,
}
pub trait Context<N>: Initialization<N> {
    fn lobby(&mut self) -> Result<List<N>>;
    fn players(&mut self) -> Result<List<N>>;
    fn npcs(&mut self) -> Result<List<N>>;
    fn clear_minimap(&mut self, node: N) -> Result<()>;
    fn npc_tick(&mut self, node: N) -> Result<()>;
    fn player_tick(&mut self, node: N) -> Result<()>;
    fn player_update(&mut self, node: N) -> Result<()>;
    fn player_reset(&mut self, node: N) -> Result<()>;
    fn npc_reset(&mut self, node: N) -> Result<()>;
    fn now_millis(&mut self) -> Result<i64>;
    fn metric(&mut self, metric: Metric, millis: i32) -> Result<()>;
    fn reset_chunks(&mut self) -> Result<()>;
    fn ground_items(&mut self) -> Result<()>;
}
impl<N: Clone + PartialEq> Sequence<N> {
    pub fn start<C: Context<N>>(&self, ctx: &mut C) -> Result<()> {
        let lobby = ctx.lobby()?;
        self.fields.write().lobby = Some(lobby);
        let players = ctx.players()?;
        self.fields.write().players = Some(players);
        let npcs = ctx.npcs()?;
        self.fields.write().npcs = Some(npcs);
        let lobby = self.fields.read().lobby.clone().ok_or(Error::Null)?;
        lobby.each(|n| ctx.clear_minimap(n))?;
        let start = ctx.now_millis()?;
        let npcs = self.fields.read().npcs.clone().ok_or(Error::Null)?;
        npcs.each(|n| ctx.npc_tick(n))?;
        let elapsed = ctx.now_millis()?.wrapping_sub(start) as i32;
        ctx.metric(Metric::NpcTick, elapsed)?;
        let start = ctx.now_millis()?;
        ctx.players()?.each(|n| ctx.player_tick(n))?;
        let elapsed = ctx.now_millis()?.wrapping_sub(start) as i32;
        ctx.metric(Metric::PlayerTick, elapsed)
    }
    pub fn run<C: Context<N>>(&self, ctx: &mut C) -> Result<()> {
        let start = ctx.now_millis()?;
        ctx.players()?.each(|n| ctx.player_update(n))?;
        let elapsed = ctx.now_millis()?.wrapping_sub(start) as i32;
        ctx.metric(Metric::PlayerRender, elapsed)
    }
    pub fn end<C: Context<N>>(&self, ctx: &mut C) -> Result<()> {
        let players = self.fields.read().players.clone().ok_or(Error::Null)?;
        players.each(|n| ctx.player_reset(n))?;
        let npcs = self.fields.read().npcs.clone().ok_or(Error::Null)?;
        npcs.each(|n| ctx.npc_reset(n))?;
        ctx.players()?.sync(ctx)?;
        ctx.reset_chunks()?;
        ctx.ground_items()
    }
    pub fn terminate(&self) {}
}
