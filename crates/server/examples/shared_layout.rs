//! Maintainer probe for handle storage, excluding pointees/allocator overhead.
use rs09_server::definitions::item::{Shared, WeakShared};
fn main() {
    println!(
        "{}",
        serde_json::json!({
            "format":1,"pointerBytes":std::mem::size_of::<usize>(),
            "sharedBytes":std::mem::size_of::<Shared<()>>(),
            "weakBytes":std::mem::size_of::<WeakShared<()>>(),
            "optionalSharedBytes":std::mem::size_of::<Option<Shared<()>>>(),
            "scope":"Inline handle storage only; excludes objects, owner allocations and allocator overhead. No total RAM, speed or player-capacity claim."
        })
    );
}
