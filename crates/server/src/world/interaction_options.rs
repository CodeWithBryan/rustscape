//! InteractPlugin's retained option table and Option.defaultHandler.
//!
//! Node IDs are opaque owning-world identities: hosts must retain their binding
//! while a table is exposed. Handler IDs likewise belong to a native registry.
//! No lock is held across a definition, registry, locale or packet callback.
use super::{
    interaction::{OptionData, OptionHandle},
    player_walk::Node,
};
use crate::definitions::item::Shared;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    Unsupported(String),
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub type Options = Shared<Vec<Option<OptionHandle>>>;
pub type Names = Shared<Vec<Option<String>>>;
pub type Handle = Shared<Table>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Npc,
    Scenery,
    Item,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamesKind {
    Options,
    Ground,
    Inventory,
}

/// Allocate once per world, then share the exact handles among all players.
pub struct Constants {
    pub attack: OptionHandle,
    pub follow: OptionHandle,
    pub trade: OptionHandle,
    pub give_to: OptionHandle,
    pub pickpocket: OptionHandle,
    pub examine: OptionHandle,
    pub assist: OptionHandle,
    pub null: OptionHandle,
}
impl Default for Constants {
    fn default() -> Self {
        fn option(name: &str, index: i32) -> OptionHandle {
            Shared::new(OptionData::new(Some(name.into()), index))
        }
        Self {
            attack: option("Attack", 0),
            follow: option("Follow", 2),
            trade: option("Trade with", 3),
            give_to: option("Give-to", 3),
            pickpocket: option("Pickpocket", 4),
            examine: option("Examine", 7),
            assist: option("Req Assist", 6),
            null: option("null", 0),
        }
    }
}

pub struct Table {
    node: Option<Node>,
    options: Options,
    pub initialized: bool,
}
impl Table {
    pub fn new(node: Option<Node>) -> Handle {
        Shared::new(Self {
            node,
            options: Shared::new(vec![None; 8]),
            initialized: false,
        })
    }
    /// The exposed array aliases the current table until init replaces it.
    pub fn options(&self) -> Options {
        self.options.clone()
    }
    pub fn node(&self) -> Option<Node> {
        self.node
    }
}

pub trait Context {
    type Definition: Clone;
    /// Runtime class identity, not a mutable gameplay classification.
    fn kind(&self, node: Option<Node>) -> Kind;
    fn node_id(&mut self, node: Node) -> Result<i32>;
    fn item_has_location(&mut self, node: Node) -> Result<bool>;
    fn definition(&mut self, node: Node) -> Result<Option<Self::Definition>>;
    fn names(&mut self, definition: &Self::Definition, kind: NamesKind) -> Result<Option<Names>>;
    fn lowercase(&mut self, name: &str) -> Result<String>;
    fn option_handler(&mut self, kind: Kind, id: i32, name: &str) -> Result<Option<usize>>;
    fn send_option(&mut self, player: Node, index: i32, name: Option<String>) -> Result<()>;
    fn node_string(&mut self, node: Option<Node>) -> Result<String>;
    fn constants(&self) -> &Constants;
    /// Native lifetime bookkeeping only: exposed old arrays/options must keep
    /// their executable handler bindings alive. This is not a game callback.
    fn retain_option(&self, option: OptionHandle) -> OptionHandle {
        option
    }
}

pub fn default_handler(
    ctx: &mut impl Context,
    node: Option<Node>,
    id: i32,
    name: Option<&str>,
) -> Result<Option<usize>> {
    // Even an unsupported/null node lowercases first; null names fail first.
    let name = ctx.lowercase(name.ok_or(Error::Null)?)?;
    let kind = ctx.kind(node);
    match kind {
        Kind::Npc | Kind::Scenery | Kind::Item => ctx.option_handler(kind, id, &name),
        Kind::Player | Kind::Other => Ok(None),
    }
}

fn index(index: i32, length: usize) -> Result<usize> {
    let index = usize::try_from(index).map_err(|_| Error::Index)?;
    if index < length {
        Ok(index)
    } else {
        Err(Error::Index)
    }
}
pub fn get(table: &Handle, slot: i32) -> Result<Option<OptionHandle>> {
    let array = table.read().options();
    let array = array.read();
    Ok(array[index(slot, array.len())?].clone())
}
pub fn send_option(
    ctx: &mut impl Context,
    node: Option<Node>,
    slot: i32,
    name: Option<String>,
) -> Result<()> {
    if ctx.kind(node) == Kind::Player {
        ctx.send_option(node.ok_or(Error::Null)?, slot, name)?;
    }
    Ok(())
}
pub fn set(ctx: &mut impl Context, table: &Handle, option: Option<&OptionHandle>) -> Result<()> {
    // Java evaluates the array receiver before the index expression.
    let array = table.read().options();
    let option = option.ok_or(Error::Null)?;
    let slot = option.read().index();
    {
        let mut values = array.write();
        let index = index(slot, values.len())?;
        values[index] = Some(option.clone());
    }
    let node = table.read().node;
    let name = option.read().name();
    send_option(ctx, node, slot, name)
}
pub fn remove_index(ctx: &mut impl Context, table: &Handle, slot: i32) -> Result<()> {
    let array = table.read().options();
    {
        let mut values = array.write();
        let index = index(slot, values.len())?;
        if values[index].is_none() {
            return Ok(());
        }
        values[index] = None;
    }
    let node = table.read().node;
    send_option(ctx, node, slot, Some("null".into()))
}
pub fn remove(
    ctx: &mut impl Context,
    table: &Handle,
    option: Option<&OptionHandle>,
) -> Result<bool> {
    let option = option.ok_or(Error::Null)?;
    let slot = option.read().index();
    if get(table, slot)?
        .as_ref()
        .is_some_and(|current| current.same(option))
    {
        remove_index(ctx, table, slot)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
pub fn init(ctx: &mut impl Context, table: &Handle, id: i32, names: Option<Names>) -> Result<()> {
    let names = names.ok_or(Error::Null)?;
    let length = names.read().len();
    table.write().options = Shared::new(vec![None; length]);
    let mut i = 0;
    // A callback can replace the table or mutate the retained input names.
    while i < table.read().options.read().len() {
        let name = names.read().get(i).ok_or(Error::Index)?.clone();
        if let Some(name) = name.filter(|name| name != "null") {
            let node = table.read().node;
            let handler = default_handler(ctx, node, id, Some(&name))?;
            let option = Shared::new(OptionData::new(Some(name), i as i32));
            option.write().handler = handler;
            let option = ctx.retain_option(option);
            set(ctx, table, Some(&option))?;
        }
        i += 1;
    }
    Ok(())
}
pub fn set_default(ctx: &mut impl Context, table: &Handle) -> Result<()> {
    if table.read().initialized {
        return Ok(());
    }
    let node = table.read().node;
    match ctx.kind(node) {
        Kind::Player => {
            let mut i = 0;
            while i < table.read().options.read().len() {
                remove_index(ctx, table, i as i32)?;
                i += 1;
            }
            for option in [
                ctx.constants().follow.clone(),
                ctx.constants().trade.clone(),
                ctx.constants().assist.clone(),
            ] {
                set(ctx, table, Some(&option))?;
            }
        }
        Kind::Npc | Kind::Scenery | Kind::Item => {
            let node = node.ok_or(Error::Null)?;
            let names_kind = if ctx.kind(Some(node)) == Kind::Item {
                if ctx.item_has_location(node)? {
                    NamesKind::Ground
                } else {
                    NamesKind::Inventory
                }
            } else {
                NamesKind::Options
            };
            let id = ctx.node_id(node)?;
            let definition = ctx.definition(node)?.ok_or(Error::Null)?;
            let names = ctx.names(&definition, names_kind)?;
            init(ctx, table, id, names)?;
        }
        Kind::Other => {
            let name = ctx.node_string(node)?;
            return Err(Error::Unsupported(format!(
                "Unsupported node type - {name}"
            )));
        }
    }
    table.write().initialized = true;
    Ok(())
}
