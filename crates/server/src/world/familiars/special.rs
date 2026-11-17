//! `FamiliarSpecial`: the node a special move is used on, with the
//! interface, component and item of the click that made it.
use super::{Error, Host, Item, NodeKind, Result};

pub struct FamiliarSpecial<H: Host> {
    pub node: Option<H::Node>,
    pub interface_id: i32,
    pub component: i32,
    pub item: Option<Item>,
}

impl<H: Host> Clone for FamiliarSpecial<H> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            interface_id: self.interface_id,
            component: self.component,
            item: self.item,
        }
    }
}

impl<H: Host> std::fmt::Debug for FamiliarSpecial<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FamiliarSpecial")
            .field("node", &self.node)
            .field("interface_id", &self.interface_id)
            .field("component", &self.component)
            .field("item", &self.item)
            .finish()
    }
}

impl<H: Host> FamiliarSpecial<H> {
    /// `new FamiliarSpecial(node)`: interface and component -1, no item.
    pub fn new(node: Option<H::Node>) -> Self {
        Self::with(node, -1, -1, None)
    }
    /// `new FamiliarSpecial(node, interfaceID, component, item)`.
    pub fn with(
        node: Option<H::Node>,
        interface_id: i32,
        component: i32,
        item: Option<Item>,
    ) -> Self {
        Self {
            node,
            interface_id,
            component,
            item,
        }
    }
    /// `getTarget()`: `(Entity) node`; a node that is not an entity fails
    /// the cast, null passes.
    pub fn target(&self, host: &mut H) -> Result<Option<H::Node>> {
        match &self.node {
            None => Ok(None),
            Some(node) => match host.kind(node)? {
                NodeKind::Other => Err(Error::Cast),
                _ => Ok(Some(node.clone())),
            },
        }
    }
}
