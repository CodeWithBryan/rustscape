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
use sha2::{Digest, Sha256};
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
    fn write(
        &self,
        output: &Path,
        bytes: &[u8],
        mut record: serde_json::Value,
    ) -> Result<(), Failure> {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(output, bytes)?;
        let fields = record.as_object_mut().expect("a record is an object");
        fields.insert("builder".into(), "rs09-client build_packs".into());
        fields.insert("server".into(), self.lock["server"].clone());
        fields.insert("client".into(), self.lock["client"].clone());
        fields.insert("bytes".into(), bytes.len().into());
        fields.insert("sha256".into(), sha(bytes).into());
        std::fs::write(
            output.with_extension("provenance.json"),
            serde_json::to_string_pretty(&record)? + "\n",
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
    let read = |key: &str, default: &str| {
        let path = tool.option(key).unwrap_or_else(|| default.into());
        std::fs::read(&path).map_err(|e| format!("{path}: {e}"))
    };
    match kind.as_str() {
        "ui" => {
            tool.ui(&output("assets/visual/ui.pack"))?;
        }
        "world" => {
            tool.world(&output("runs/visual/world.pack"), false)?;
        }
        "ui-models" => {
            let ui = read("--ui", "assets/visual/ui.pack")?;
            let world = read("--world", "assets/visual/demo.pack")?;
            tool.ui_models(&output("assets/visual/ui-models.pack"), &ui, &world)?;
        }
        "ui-players" => {
            let ui = read("--ui", "assets/visual/ui-models.pack")?;
            tool.ui_players(&output("assets/visual/ui-players.pack"), &ui)?;
        }
        "all" => {
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
