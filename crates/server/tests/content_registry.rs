//! The live registration of ported content (docs/content-porting.md, step 6):
//! every class of `content::LIVE_DIALOGUES` is a scanned class with a
//! `dialogue_plugin` arm, and the module that declares it keeps no stub the
//! scaffold wrote ("... is not ported yet"). A freshly scaffolded port is in
//! the replay table only; listing it live before its bodies exist fails here.
use rs09_server::world::content;
use std::path::{Path, PathBuf};

/// Every `.rs` file under `dir`.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn live_dialogues_are_scanned_ported_and_free_of_stubs() {
    let scanned = rs09_server::startup::scan::section("scan.initializable");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/world/content");
    let mut files = Vec::new();
    sources(&root, &mut files);
    let texts: Vec<(PathBuf, String)> = files
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            (p, text)
        })
        .collect();
    let squeeze = |s: &str| s.replace(char::is_whitespace, "");
    let table = squeeze(&std::fs::read_to_string(root.join("mod.rs")).unwrap());
    for class in content::LIVE_DIALOGUES {
        assert!(
            scanned.contains(class),
            "{class} is in LIVE_DIALOGUES but not in scan.initializable"
        );
        assert!(
            table.contains(&format!("\"{class}\"=>")),
            "{class} has no content::dialogue_plugin arm"
        );
        let simple = class.rsplit('.').next().unwrap();
        let declaration = format!("pub struct {simple};");
        let (path, text) = texts
            .iter()
            .find(|(_, t)| t.contains(&declaration))
            .unwrap_or_else(|| panic!("no module under world/content declares {simple}"));
        assert!(
            !text.contains("is not ported yet"),
            "{class} is registered live but {} still has a scaffold stub (\"is not ported yet\")",
            path.display()
        );
    }
}
