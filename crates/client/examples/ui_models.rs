//! Audit inventory model dependencies through the native decoder.
use rs09_client::{model::Model, pack::Pack};
fn main() {
    let pack = Pack::decode(std::fs::read("assets/visual/ui.pack").unwrap()).unwrap();
    let (mut count, mut textured, mut faces) = (0, 0, 0);
    for name in pack
        .manifest
        .files
        .keys()
        .filter(|s| s.starts_with("model/"))
    {
        let m = Model::decode(pack.get(name).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"));
        count += 1;
        faces += m.faces.len();
        if m.textures.iter().any(|&v| v >= 0) {
            textured += 1;
        }
    }
    println!("{count} inventory models, {faces} faces, {textured} textured models");
}
