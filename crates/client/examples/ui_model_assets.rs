//! Discover raw model dependencies using the already-ported cache decoders.
use rs09_client::{pack::Pack, ui_packs::ModelDependencies};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let ui = Pack::decode(std::fs::read(&args[1]).unwrap()).unwrap();
    let world = Pack::decode(std::fs::read(&args[2]).unwrap()).unwrap();
    println!(
        "{}",
        ModelDependencies::discover(&ui, &world).unwrap().json()
    );
}
