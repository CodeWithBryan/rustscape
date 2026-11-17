//! Retained Option fields and captures used by InteractPlugin movement pulses.
//! Table/default control flow is in interaction_options. Concrete registry and
//! owning-world arrival bindings remain separate. Handler IDs retain their
//! native registry identity.
use super::player_walk::Node;
use crate::definitions::item::Shared;

pub struct OptionData {
    name: Option<String>,
    index: i32,
    pub handler: Option<usize>,
}
impl OptionData {
    pub fn new(name: Option<String>, index: i32) -> Self {
        Self {
            name,
            index,
            handler: None,
        }
    }
    pub fn name(&self) -> Option<String> {
        self.name.clone()
    }
    pub fn index(&self) -> i32 {
        self.index
    }
}
pub type OptionHandle = Shared<OptionData>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApproachKind {
    Option,
    Invalid,
}

/// These are the anonymous pulse's final captures, independently of mutable
/// MovementPulse.mover/destination and the option's mutable handler field.
#[derive(Clone)]
pub struct Approach {
    pub player: Node,
    pub target: Option<Node>,
    pub option: Option<OptionHandle>,
    pub kind: ApproachKind,
}
