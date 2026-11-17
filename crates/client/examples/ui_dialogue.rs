//! Offline cached dialogue/model review packets. This runs no NPC/content handler.
use rs09_client::{pack::Pack, ui::item::Catalog};
use rs09_protocol::game::{Direction, Frame, updates as u};

fn save(name: &str, frames: impl IntoIterator<Item = Frame>) {
    let mut wire =
        std::fs::read("runs/visual/ui-gameframe.bin").expect("generate ui_preview fixtures first");
    for f in frames {
        f.write(Direction::ServerToClient, None, &mut wire).unwrap();
    }
    std::fs::write(format!("runs/visual/{name}.bin"), wire).unwrap();
}
fn main() {
    let p = 241 << 16;
    save(
        "ui-dialogue",
        [
            u::interface_hidden(true, 101, p | 1),
            u::interface_hidden(false, 102, p | 2),
            u::interface_animation(p | 2, 9760, 103),
            u::interface_object(p | 1, -1, 1, 104),
            u::interface_object(p | 2, -1, 1, 105),
            u::interface_npc_head(p | 2, 278, 106),
            u::interface_position(p | 2, 45, 45, 107),
            u::interface_angle(p | 2, 40, 1882, 2150, 108),
            u::interface_text(p | 3, b"Cook", 109).unwrap(),
            u::interface_text(p | 4, b"Welcome to Lumbridge!", 110).unwrap(),
            u::open_interface(0, 752 << 16 | 12, 111, 241),
        ],
    );
    let p = 64 << 16;
    save(
        "ui-player-dialogue",
        [
            u::interface_hidden(true, 101, p | 1),
            u::interface_hidden(false, 102, p | 2),
            u::interface_animation(p | 2, 9760, 103),
            u::interface_position(p | 2, 426, 45, 104),
            u::interface_player_head(p | 2, 105),
            u::interface_text(p | 3, b"Player preview", 106).unwrap(),
            u::interface_text(p | 4, b"Hello there!", 107).unwrap(),
            u::open_interface(0, 752 << 16 | 12, 108, 64),
        ],
    );
    let pack = Pack::decode(std::fs::read("assets/visual/ui-models.pack").unwrap()).unwrap();
    let item = Catalog::default().get(&pack, 4151).unwrap();
    let p = 241 << 16;
    save(
        "ui-item-message",
        [
            u::interface_hidden(true, 101, p | 1),
            u::interface_hidden(false, 102, p | 2),
            u::interface_text(p | 3, b"", 103).unwrap(),
            u::interface_text(p | 4, b"An abyssal whip.", 104).unwrap(),
            u::interface_object(p | 2, 4151, 1, 105),
            u::interface_angle(p | 2, item.xAngle2D, item.yAngle2D, item.zoom2d / 2, 106),
            u::interface_position(p | 2, 45, 45, 107),
            u::open_interface(0, 752 << 16 | 12, 108, 241),
        ],
    );
    println!(
        "Offline NPC/player dialogue and item-message replays generated; stock remains synthetic."
    );
}
