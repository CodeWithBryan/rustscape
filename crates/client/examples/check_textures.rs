//! Developer differential audit; JVM output is never loaded by the visual client.
use rs09_client::{pack::Pack, texture};
fn main() {
    let pack = Pack::decode(std::fs::read("assets/visual/demo.pack").unwrap()).unwrap();
    let mut exact = 0;
    let mut different = 0;
    let mut missing = 0;
    let mut report = Vec::new();
    std::fs::create_dir_all("runs/native-textures").unwrap();
    for id in 0..texture::material_colors(pack.get("materials").unwrap())
        .unwrap()
        .len()
    {
        if pack.get(&format!("texture/{id}")).is_err() {
            continue;
        }
        match texture::generate(&pack, id as i32, 64) {
            Ok(pixels) => {
                std::fs::write(format!("runs/native-textures/{id}.rgba"), &pixels).unwrap();
                let reference =
                    std::fs::read(format!("runs/texture-oracle/rgba/{id}.rgba")).unwrap();
                let mismatches = pixels
                    .iter()
                    .zip(reference)
                    .filter(|(a, b)| **a != *b)
                    .count();
                if mismatches == 0 {
                    exact += 1;
                } else {
                    different += 1;
                }
                report.push(serde_json::json!({"id":id,"mismatches":mismatches}));
            }
            Err(error) => {
                missing += 1;
                report.push(serde_json::json!({"id":id,"error":error}));
            }
        }
    }
    std::fs::write(
        "runs/native-textures/report.json",
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{exact} exact, {different} differ, {missing} unsupported");
}
