//! Original item and container state rules, independent of transport/rendering.
pub mod bank;
pub mod container;
pub mod container_listener;
pub mod equipment;
pub mod item;
pub mod listener;
pub mod player_binding;
pub mod pouch_ids;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Bounds,
    ListBounds,
    Type,
    Number,
    ConcurrentModification,
    CyclicNote,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
