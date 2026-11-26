//! Discover resolved original wearable model dependencies.
use rs09_client::pack::Pack;
fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/visual/ui-models.pack".into());
    let pack = Pack::decode(std::fs::read(path).unwrap()).unwrap();
    let (models, items) = rs09_client::ui_packs::player_models(&pack).unwrap();
    println!("{}", serde_json::json!({"models":models,"items":items}));
}
