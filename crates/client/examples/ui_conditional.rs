//! Explicit offline states of the original cached casket combination interface.
use rs09_protocol::game::{Direction, updates};
fn main() {
    for value in [0, 1] {
        let mut wire =
            std::fs::read("runs/visual/ui-gameframe.bin").expect("generate ui_preview first");
        for f in [
            updates::varp(261, value),
            updates::open_interface(0, 746 << 16 | 6, 101, 289),
        ] {
            f.write(Direction::ServerToClient, None, &mut wire).unwrap();
        }
        std::fs::write(format!("runs/visual/ui-casket{value}.bin"), wire).unwrap();
    }
    println!("Two explicit casket-interface states generated; no clue/content handler runs.");
}
