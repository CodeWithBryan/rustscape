//! Native replacement for `tools/prepare_ui.py`, `tools/prepare_visual.py`,
//! `tools/prepare_ui_models.py` and `tools/prepare_player_models.py`: builds
//! asset packs from the pinned cache without Python.
//!
//! `cargo run --release -p rs09-client --example build_packs -- ui [OUTPUT] [--cache DIR]`
//!
//! `... -- world OUTPUT [--cache DIR] [--keys xteas.json] [--doors door_configs.json]
//! [--spawns npc_spawns.json] [--center X,Y] [--label NAME] [--login-window]
//! [--region-npcs] [--lenient]` builds a starting-scene pack. The three JSON
//! files are the pinned server's; each is optional, but without a region's
//! key its locations cannot be read (`--lenient` then leaves them out).
//!
//! `... -- ui-models [OUTPUT] [--ui ui.pack] [--world demo.pack] [--facial
//! FacialExpression.java]` extends an interface pack for model widgets and
//! chat heads. Without `--facial` the file is read from the reference
//! checkout at the pinned commit with `git show`.
//!
//! `... -- ui-players [OUTPUT] [--ui ui-models.pack]` extends that pack for
//! equipped player bodies.
//!
//! `... -- all [--keys ..] [--doors ..] [--spawns ..]` writes the four packs
//! the client and the browser page load by default, each from the one before:
//! `assets/visual/demo.pack`, `ui.pack`, `ui-models.pack`, `ui-players.pack`.
//! Inputs that are not given are read from the reference checkout.
//!
//! Every pack gets a `NAME.provenance.json` beside it: the pins, the hashes
//! of its inputs, its size and hash, and the builder's counts.
//!
//! Nothing is replaced until every pack of the command is built: each pack
//! and sidecar is written beside its destination first and all of them are
//! moved into place at the end (`rs09_client::staging`), which puts the
//! previous files back if a move fails. As the scripts did, the layered packs refuse
//! a cache whose files differ from the pinned server's Git LFS objects, and
//! an output that is one of their inputs.
use rs09_client::staging::Staging;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

type Failure = Box<dyn std::error::Error>;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Tool {
    args: Vec<String>,
    cache: PathBuf,
    lock: serde_json::Value,
    commit: String,
    /// Written files waiting to be moved into place together.
    staged: RefCell<Staging>,
}

/// The absolute form of a path, following links where it exists.
fn resolved(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    match (absolute.parent(), absolute.file_name()) {
        (Some(parent), Some(name)) => parent
            .canonicalize()
            .map(|parent| parent.join(name))
            .unwrap_or(absolute),
        _ => absolute,
    }
}

/// Refuses an output pack, or its sidecar, that is one of the inputs.
fn separate(output: &Path, inputs: &[&str]) -> Result<(), Failure> {
    let targets = [
        resolved(output),
        resolved(&output.with_extension("provenance.json")),
    ];
    for input in inputs {
        if targets.contains(&resolved(Path::new(input))) {
            return Err(format!(
                "{} would replace its input {input}; use a separate output pack",
                output.display()
            )
            .into());
        }
    }
    Ok(())
}

impl Tool {
    fn option(&self, key: &str) -> Option<String> {
        self.args
            .iter()
            .position(|a| a == key)
            .and_then(|i| self.args.get(i + 1))
            .cloned()
    }
    fn flag(&self, key: &str) -> bool {
        self.args.iter().any(|a| a == key)
    }
    /// A file of the pinned server, from the reference checkout.
    fn server_file(&self, path: &str) -> Result<Vec<u8>, Failure> {
        let output = std::process::Command::new("git")
            .args(["-C", "reference/server", "show"])
            .arg(format!("{}:{path}", self.commit))
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "git show {path}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )
            .into());
        }
        Ok(output.stdout)
    }
    /// The file `key` names; with `fallback`, the pinned server's file when
    /// the option is absent.
    fn input(&self, key: &str, fallback: Option<&str>) -> Result<Option<Vec<u8>>, Failure> {
        match (self.option(key), fallback) {
            (Some(path), _) => Ok(Some(std::fs::read(path)?)),
            (None, Some(path)) => Ok(Some(self.server_file(path)?)),
            (None, None) => Ok(None),
        }
    }
    /// The scripts' check: each cache file the builders read must be the
    /// pinned server's Git LFS object.
    fn verify_cache(&self) -> Result<(), Failure> {
        let names = ["main_file_cache.dat2"]
            .into_iter()
            .map(String::from)
            .chain([0, 1, 2, 7, 20].map(|i| format!("main_file_cache.idx{i}")));
        for name in names {
            let pointer = self.server_file(&format!("Server/data/cache/{name}"))?;
            let expected = String::from_utf8_lossy(&pointer)
                .lines()
                .find_map(|line| line.strip_prefix("oid sha256:").map(str::to_owned))
                .ok_or_else(|| format!("{name}: the pinned file is not a Git LFS pointer"))?;
            let actual = sha(&std::fs::read(self.cache.join(&name))
                .map_err(|e| format!("{}: {e}", self.cache.join(&name).display()))?);
            if actual != expected {
                return Err(format!("{name} differs from pin").into());
            }
        }
        Ok(())
    }
    fn stage(&self, destination: &Path, bytes: &[u8]) -> Result<(), Failure> {
        Ok(self.staged.borrow_mut().stage(destination, bytes)?)
    }
    fn write(
        &self,
        output: &Path,
        bytes: &[u8],
        mut record: serde_json::Value,
    ) -> Result<(), Failure> {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.stage(output, bytes)?;
        let fields = record.as_object_mut().expect("a record is an object");
        fields.insert("builder".into(), "rs09-client build_packs".into());
        fields.insert("server".into(), self.lock["server"].clone());
        fields.insert("client".into(), self.lock["client"].clone());
        fields.insert("bytes".into(), bytes.len().into());
        fields.insert("sha256".into(), sha(bytes).into());
        self.stage(
            &output.with_extension("provenance.json"),
            (serde_json::to_string_pretty(&record)? + "\n").as_bytes(),
        )?;
        println!(
            "{} bytes, sha256 {} -> {}",
            bytes.len(),
            sha(bytes),
            output.display()
        );
        Ok(())
    }

    fn ui(&self, output: &Path) -> Result<Vec<u8>, Failure> {
        let bytes = rs09_client::cache_pack::ui_pack(&self.cache, &self.commit)?;
        let pack = rs09_client::pack::Pack::decode(bytes.clone())?;
        let count = |prefix: &str| {
            pack.manifest
                .files
                .keys()
                .filter(|name| name.starts_with(prefix))
                .count()
        };
        let counts = serde_json::json!({
            "interface": count("interface/"),
            "script": count("script/"),
            "font": count("font/"),
            "enum": count("enum/"),
            "item": count("item/"),
            "varbit": count("varbit/"),
            "param": count("param/"),
            "item_models": count("model/"),
            "sounds": count("sound/"),
        });
        self.write(output, &bytes, serde_json::json!({ "counts": counts }))?;
        Ok(bytes)
    }

    /// `defaults` reads the inputs that are not given from the reference
    /// checkout, as the original script did.
    fn world(&self, output: &Path, defaults: bool) -> Result<Vec<u8>, Failure> {
        use rs09_client::world_pack::{Inputs, Options, world_pack};
        let pinned = |name: &'static str| defaults.then_some(name);
        let keys = self.input("--keys", pinned("Server/data/configs/xteas.json"))?;
        let doors = self.input("--doors", pinned("Server/data/configs/door_configs.json"))?;
        let spawns = self.input("--spawns", pinned("Server/data/configs/npc_spawns.json"))?;
        let mut inputs = Inputs::default();
        if let Some(bytes) = &keys {
            inputs.keys = Inputs::parse_keys(bytes)?;
        }
        if let Some(bytes) = &doors {
            inputs.doors = Inputs::parse_doors(bytes)?;
        }
        if let Some(bytes) = &spawns {
            inputs.spawns = Inputs::parse_spawns(bytes)?;
        }
        let mut options = Options {
            login_window: self.flag("--login-window"),
            region_npcs: self.flag("--region-npcs"),
            lenient: self.flag("--lenient"),
            ..Options::default()
        };
        if let Some(center) = self.option("--center") {
            let (x, y) = center.split_once(',').ok_or("--center takes X,Y")?;
            options.center = [x.parse()?, y.parse()?];
        }
        if let Some(label) = self.option("--label") {
            options.label = label;
        }
        let built = world_pack(&self.cache, &inputs, &options, &self.commit)?;
        for note in &built.notes {
            eprintln!("left out: {note}");
        }
        let pack = rs09_client::pack::Pack::decode(built.bytes.clone())?;
        let scene: serde_json::Value = serde_json::from_slice(pack.get("scene.json")?)?;
        let count = |prefix: &str| {
            pack.manifest
                .files
                .keys()
                .filter(|name| name.starts_with(prefix))
                .count()
        };
        let hash = |bytes: &Option<Vec<u8>>| bytes.as_deref().map(sha);
        let record = serde_json::json!({
            "scene": { "name": options.label, "start": options.center, "regions": scene["regions"],
                "login_window": options.login_window, "region_npcs": options.region_npcs },
            "assets": pack.manifest.files.len(),
            "models": count("model/"),
            "locations": scene["locations"].as_array().map_or(0, Vec::len),
            "location_definitions": count("loc/"),
            "missing_models": scene["missing_models"],
            "npc_types": scene["npc_types"],
            "npc_spawns": scene["npc_spawns"].as_array().map_or(0, Vec::len),
            "underwater_files": count("underwater/"),
            "left_out": built.notes,
            "region_key_source_sha256": hash(&keys),
            "door_source_sha256": hash(&doors),
            "npc_spawn_source_sha256": hash(&spawns),
        });
        self.write(output, &built.bytes, record)?;
        Ok(built.bytes)
    }

    fn ui_models(&self, output: &Path, ui: &[u8], world: &[u8]) -> Result<Vec<u8>, Failure> {
        let facial = self
            .input(
                "--facial",
                Some("Server/src/main/core/game/dialogue/FacialExpression.java"),
            )?
            .expect("a fallback was given");
        let source = rs09_client::cache_pack::Source::open(&self.cache)?;
        let built = rs09_client::ui_packs::ui_models_pack(
            &source,
            ui,
            world,
            &rs09_client::ui_packs::facial_sequences(&facial),
        )?;
        let mut record = built.report;
        record["baseUiSha256"] = sha(ui).into();
        record["worldSha256"] = sha(world).into();
        record["facialExpressionSource"] = sha(&facial).into();
        self.write(output, &built.bytes, record)?;
        Ok(built.bytes)
    }

    fn ui_players(&self, output: &Path, ui: &[u8]) -> Result<Vec<u8>, Failure> {
        let source = rs09_client::cache_pack::Source::open(&self.cache)?;
        let built = rs09_client::ui_packs::ui_players_pack(&source, ui)?;
        let mut record = built.report;
        record["baseUiSha256"] = sha(ui).into();
        self.write(output, &built.bytes, record)?;
        Ok(built.bytes)
    }
}

fn main() -> Result<(), Failure> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lock: serde_json::Value = serde_json::from_slice(&std::fs::read("upstream.lock.json")?)?;
    let commit = lock["server"]["commit"]
        .as_str()
        .ok_or("upstream.lock.json has no server commit")?
        .to_owned();
    let mut tool = Tool {
        cache: PathBuf::new(),
        args,
        lock,
        commit,
        staged: RefCell::new(Staging::default()),
    };
    tool.cache = PathBuf::from(
        tool.option("--cache")
            .unwrap_or_else(|| "assets/cache".into()),
    );
    let kind = tool.args.first().cloned().unwrap_or_else(|| "ui".into());
    let output = |default: &str| {
        PathBuf::from(
            tool.args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .cloned()
                .unwrap_or_else(|| default.into()),
        )
    };
    let path = |key: &str, default: &str| tool.option(key).unwrap_or_else(|| default.into());
    let read = |path: &str| std::fs::read(path).map_err(|e| format!("{path}: {e}"));
    // A failed build drops the staging, which removes what it wrote; a
    // failed publication puts back what it had replaced.
    build(&tool, &kind, &output, &path, &read)?;
    Ok(tool.staged.into_inner().publish()?)
}

fn build(
    tool: &Tool,
    kind: &str,
    output: &dyn Fn(&str) -> PathBuf,
    path: &dyn Fn(&str, &str) -> String,
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<(), Failure> {
    match kind {
        "ui" => {
            tool.ui(&output("assets/visual/ui.pack"))?;
        }
        "world" => {
            tool.world(&output("runs/visual/world.pack"), false)?;
        }
        "ui-models" => {
            let target = output("assets/visual/ui-models.pack");
            let (ui, world) = (
                path("--ui", "assets/visual/ui.pack"),
                path("--world", "assets/visual/demo.pack"),
            );
            separate(&target, &[&ui, &world])?;
            tool.verify_cache()?;
            tool.ui_models(&target, &read(&ui)?, &read(&world)?)?;
        }
        "ui-players" => {
            let target = output("assets/visual/ui-players.pack");
            let ui = path("--ui", "assets/visual/ui-models.pack");
            separate(&target, &[&ui])?;
            tool.verify_cache()?;
            tool.ui_players(&target, &read(&ui)?)?;
        }
        "all" => {
            tool.verify_cache()?;
            let directory = output("assets/visual");
            let world = tool.world(&directory.join("demo.pack"), true)?;
            let ui = tool.ui(&directory.join("ui.pack"))?;
            let models = tool.ui_models(&directory.join("ui-models.pack"), &ui, &world)?;
            tool.ui_players(&directory.join("ui-players.pack"), &models)?;
        }
        other => {
            return Err(format!(
                "unknown pack kind {other}; expected ui, world, ui-models, ui-players or all"
            )
            .into());
        }
    }
    Ok(())
}
