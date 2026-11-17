//! Joined loader bindings: actual native subtype, manager, parser and readers.
use super::*;
use rs09_game_core::java_json::Json;
use rs09_server::world::{ground_manager as gm, ground_spawn as gs, ground_spawn_loader as gl};
use std::io::Read;

#[derive(Default)]
pub(super) struct Extra {
    loader: gl::Loader,
}
fn category(error: &gl::Error) -> String {
    match error {
        gl::Error::Null => "Null".into(),
        gl::Error::Type => "Type".into(),
        gl::Error::Number(_) => "Number".into(),
        gl::Error::Index => "Index".into(),
        gl::Error::Json(_) => "Json".into(),
        gl::Error::Io(_) => "Io".into(),
        gl::Error::Binding(s) => s.clone(),
        gl::Error::Ground(ground::Error::Null) => "Null".into(),
        gl::Error::Ground(ground::Error::Binding(s)) => s.clone(),
        gl::Error::Manager(e) => match e {
            gm::Error::Null => "Null".into(),
            gm::Error::Type => "Type".into(),
            gm::Error::Index => "Index".into(),
            gm::Error::Binding(s) => s.clone(),
            gm::Error::ConcurrentModification => "ConcurrentModificationException".into(),
            gm::Error::Ground(ground::Error::Null) => "Null".into(),
            gm::Error::Ground(ground::Error::Binding(s)) => s.clone(),
        },
    }
}
fn error(e: gl::Error) -> Error {
    Error::Binding(category(&e))
}
pub(super) fn act(m: &mut Model, a: &Value) -> Result<Value> {
    let loader = m.loaders.loader.clone();
    let registry = m.spawns.registry.clone();
    let manager = m.extra.manager.clone();
    match a["op"].as_str().unwrap() {
        "loaderLoad" => {
            if a["missing"] == true {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                    "../../runs/ground-spawn-loader-oracle/config/missing-ground_spawns.json",
                );
                assert!(!path.exists(), "Test missing path must remain absent");
                loader
                    .load_file(&path, m, &registry, &manager)
                    .map_err(error)?;
            } else {
                let text = if a["pinned"] == true {
                    include_str!("../../fixtures/ground-spawn-loader/ground_spawns.json")
                } else {
                    a["text"].as_str().unwrap()
                };
                loader
                    .load_text(text, m, &registry, &manager)
                    .map_err(error)?;
            }
        }
        "loaderRead" => {
            return match loader.reader() {
                None => Ok(Value::Null),
                Some(reader) => {
                    let mut byte = [0];
                    let n = reader
                        .write()
                        .read(&mut byte)
                        .map_err(|e| Error::Binding(e.to_string()))?;
                    Ok(json!(if n == 0 { -1 } else { i32::from(byte[0]) }))
                }
            };
        }
        "loaderStrings" => {
            return Ok(json!(
                m.grounds
                    .iter()
                    .filter_map(|g| registry.get(g))
                    .map(|s| gs::java_string(&s))
                    .collect::<Vec<_>>()
            ));
        }
        _ => return super::pickup::act(m, a),
    }
    Ok(Value::Null)
}
pub(super) fn snapshot(m: &Model, state: &mut Value) {
    state["loaderReader"] = json!(m.loaders.loader.reader().is_some());
    state["loadedSpawns"] = json!(
        m.grounds
            .iter()
            .filter_map(|g| m.spawns.registry.get(g))
            .map(|s| json!([
                m.gid(&gs::as_ground(&s)),
                s.read().respawn_rate,
                gs::java_string(&s)
            ]))
            .collect::<Vec<_>>()
    );
}
impl gl::Context for Model {
    fn entry_error(&mut self, entry: &Json, error: &gl::Error) -> gl::Result<()> {
        self.emit(
            "loaderError",
            vec![json!(entry.text()), json!(category(error))],
        )
        .map_err(|e| gl::Error::Binding(err(e)))
    }
    fn initialized(&mut self, count: i32) -> gl::Result<()> {
        self.emit("loaderCount", vec![json!(count)])
            .map_err(|e| gl::Error::Binding(err(e)))
    }
}

#[test]
fn real_files_publish_readers_after_open_preserve_failed_open_and_keep_partial_state() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let dir = root.join("runs").join(format!(
        "ground-loader-native-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    assert!(dir.canonicalize().unwrap().starts_with(root.join("runs")));
    struct Files(std::path::PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("ground_spawns.json"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let _files = Files(dir.clone());
    let path = dir.join("ground_spawns.json");
    std::fs::write(&path, r#"[{"item_id":1,"loc_data":"1,3207,3216,0,3"}]"#).unwrap();
    let mut model = super::model(&json!({"manager":true,"spawns":true,"loader":true,"actions":[]}));
    let loader = gl::Loader::default();
    let alias = loader.clone();
    let registry = model.spawns.registry.clone();
    let manager = model.extra.manager.clone();
    let prefix = format!("{}{}", dir.display(), std::path::MAIN_SEPARATOR);
    assert_eq!(
        loader
            .load_config(Some(&prefix), &mut model, &registry, &manager)
            .unwrap(),
        1
    );
    let first = loader.reader().unwrap();
    assert!(alias.reader().unwrap().same(&first));
    assert!(matches!(
        loader.load_file(&dir.join("missing.json"), &mut model, &registry, &manager),
        Err(gl::Error::Io(_))
    ));
    assert!(loader.reader().unwrap().same(&first));
    assert!(matches!(
        loader.load_file(&dir, &mut model, &registry, &manager),
        Err(gl::Error::Io(_))
    ));
    assert!(loader.reader().unwrap().same(&first));
    std::fs::write(&path, "[").unwrap();
    assert!(matches!(
        loader.load_file(&path, &mut model, &registry, &manager),
        Err(gl::Error::Json(_))
    ));
    let failed = loader.reader().unwrap();
    assert!(!failed.same(&first));
    assert!(alias.reader().unwrap().same(&failed));
    std::fs::write(&path, "[]").unwrap();
    assert_eq!(
        alias
            .load_file(&path, &mut model, &registry, &manager)
            .unwrap(),
        0
    );
    assert_eq!(
        manager.get_items().read().len(),
        1,
        "new loads do not reset existing initialized items"
    );
}
