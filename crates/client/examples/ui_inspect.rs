//! Inspect decoded cache data without a GPU; useful while porting CS2 dependencies.
use rs09_client::{pack::Pack, ui::Component};
fn main() {
    let pack = Pack::decode(std::fs::read("assets/visual/ui.pack").unwrap()).unwrap();
    let groups: Vec<String> = std::env::args().skip(1).collect();
    if groups.first().is_some_and(|s| s == "sprites") {
        let visual = Pack::decode(std::fs::read("assets/visual/demo.pack").unwrap()).unwrap();
        for id in groups.iter().skip(1) {
            let sheet = rs09_client::ui::sprite::Sheet::decode(
                visual.get(&format!("sprite/{id}")).unwrap(),
            )
            .unwrap();
            let f = &sheet.frames[0];
            let rgba = sheet.rgba(0).unwrap();
            println!(
                "{id}: size {:?}, frame {:?}, offset {:?}, visible {} / {}",
                sheet.size,
                f.size,
                f.offset,
                rgba.chunks_exact(4).filter(|p| p[3] != 0).count(),
                f.indices.len()
            );
        }
        return;
    }
    if groups.first().is_some_and(|s| s == "scripts") {
        for name in pack
            .manifest
            .files
            .keys()
            .filter(|s| s.starts_with("script/"))
        {
            let script = rs09_client::ui::script::Script::decode(pack.get(name).unwrap()).unwrap();
            println!("{}\t{}", name, serde_json::to_string(&script).unwrap());
        }
        return;
    }
    for name in pack.manifest.files.keys() {
        let p: Vec<_> = name.split('/').collect();
        if p.len() == 3 && p[0] == "interface" && groups.iter().any(|g| g == p[1]) {
            let id = (p[1].parse::<i32>().unwrap() << 16) | p[2].parse::<i32>().unwrap();
            let (c, _) = Component::decode(id, pack.get(name).unwrap()).unwrap();
            println!("{}", serde_json::to_string(&c).unwrap());
        }
    }
}
