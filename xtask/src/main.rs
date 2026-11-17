use serde_json::{Value, json};
use sha2::{Digest, Sha256};
mod actors_oracle;
mod animator_oracle;
mod attributes_oracle;
mod auth_oracle;
mod benchmark;
mod cache_assets;
mod cache_oracle;
mod camera_oracle;
mod config_oracle;
mod display_oracle;
mod dynamic_regions_oracle;
mod input_oracle;
mod inventory_oracle;
mod item_config_oracle;
mod item_oracle;
mod lighting_oracle;
mod loc_oracle;
mod login_oracle;
mod materials_oracle;
mod menus_oracle;
mod minimap_oracle;
mod music_oracle;
mod network_oracle;
mod npc_definition_oracle;
mod occlusion_oracle;
mod picking_oracle;
mod player_oracle;
mod preferences_oracle;
mod profile_oracle;
mod roof_oracle;
mod saved_data_oracle;
mod scene_oracle;
mod scenery_definition_oracle;
mod skills_oracle;
mod sql_oracle;
mod title_oracle;
mod ui_fidelity_oracle;
mod underwater_material_oracle;
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const CLIENT_SOURCES: &[&str] = &[
    "Buffer",
    "Node",
    "Packet",
    "IsaacRandom",
    "GlobalConfig",
    "HuffmanCodec",
    "IntUtils",
    "RandomUtils",
    "Js5Compression",
    "GzipDecompressor",
    "Bzip2Decompressor",
    "Bzip2DState",
    "CollisionMap",
    "Js5Index",
    "IntHashTable",
    "Js5",
    "Js5ResourceProvider",
    "ByteArray",
    "DirectByteArray",
    "Cache",
    "BufferedFile",
];

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bootstrap") => bootstrap(root)?,
        Some("oracle") => oracle(root, args.iter().any(|arg| arg == "--check"))?,
        Some("inventory") => inventory(root)?,
        Some("bench") => benchmark::run_benchmarks(root)?,
        Some("cache-oracle") => {
            cache_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("cache-audit") => cache_oracle::audit(root, args.iter().any(|arg| arg == "--jvm"))?,
        Some("cache-fetch") => cache_assets::fetch(root)?,
        Some("actors-oracle") => {
            actors_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("camera-oracle") => {
            camera_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("display-oracle") => {
            display_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("lighting-oracle") => {
            lighting_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("loc-oracle") => loc_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?,
        Some("occlusion-oracle") => {
            occlusion_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("music-oracle") => {
            music_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("menus-oracle") => {
            menus_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("minimap-oracle") => {
            minimap_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("roof-oracle") => {
            roof_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("materials-oracle") => {
            let check = args.iter().any(|arg| arg == "--check");
            // `--underwater` runs the underwater material's part alone.
            if !args.iter().any(|arg| arg == "--underwater") {
                materials_oracle::generate(root, check)?;
            }
            underwater_material_oracle::generate(root, check, &verify_reference(root)?)?
        }
        Some("picking-oracle") => {
            picking_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("title-oracle") => {
            title_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("preferences-oracle") => {
            preferences_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("ui-fidelity-oracle") => {
            ui_fidelity_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("network-oracle") => {
            network_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("input-oracle") => {
            input_oracle::generate(root, args.iter().any(|arg| arg == "--check"))?
        }
        Some("network-interop") => network_oracle::interop(root)?,
        Some("headless-lifecycle-oracle") => {
            profile_oracle::generate(root, &args, "headless-lifecycle")?
        }
        Some("network-smoke") => network_oracle::smoke(root, &args)?,
        Some("login-oracle") => login_oracle::generate(root, &args)?,
        Some("login-interop") => login_oracle::interop(root, &args)?,
        Some("auth-oracle") => auth_oracle::generate(root, &args)?,
        Some("auth-interop") => auth_oracle::interop(root, &args)?,
        Some("sql-oracle") => sql_oracle::generate(root, &args)?,
        Some("sql-native") => sql_oracle::native(root, &args)?,
        Some("config-oracle") => config_oracle::generate(root, &args)?,
        Some("player-oracle") => player_oracle::generate(root, &args)?,
        Some("saved-data-oracle") => saved_data_oracle::generate(root, &args)?,
        Some("attributes-oracle") => attributes_oracle::generate(root, &args)?,
        Some("ironman-oracle") => profile_oracle::generate(root, &args, "ironman")?,
        Some("bank-oracle") => profile_oracle::generate(root, &args, "bank")?,
        Some("bank-interface-oracle") => profile_oracle::generate(root, &args, "bank-interface")?,
        Some("input-prompt-oracle") => profile_oracle::generate(root, &args, "input-prompt")?,
        Some("plain-dialogue-oracle") => profile_oracle::generate(root, &args, "plain-dialogue")?,
        Some("dialogue-oracle") => profile_oracle::generate(root, &args, "dialogue")?,
        Some("interaction-listeners-oracle") => {
            profile_oracle::generate(root, &args, "interaction-listeners")?
        }
        Some("variables-oracle") => profile_oracle::generate(root, &args, "variables")?,
        Some("bank-pin-oracle") => profile_oracle::generate(root, &args, "bank-pin")?,
        Some("skills-oracle") => skills_oracle::generate(root, &args)?,
        Some("skills-bench") => skills_oracle::benchmark(root, &args)?,
        Some("item-oracle") => item_oracle::generate(root, &args)?,
        Some("item-samples") => item_oracle::samples(root)?,
        Some("item-audit") => item_oracle::audit(root, &args)?,
        Some("inventory-oracle") => inventory_oracle::generate(root, &args)?,
        Some("container-ownership-oracle") => {
            profile_oracle::generate(root, &args, "container-ownership")?
        }
        Some("container-listeners-oracle") => {
            profile_oracle::generate(root, &args, "container-listeners")?
        }
        Some("owned-player-oracle") => profile_oracle::generate(root, &args, "owned-player")?,
        Some("equipment-oracle") => profile_oracle::generate(root, &args, "equipment")?,
        Some("player-appearance-oracle") => {
            profile_oracle::generate(root, &args, "player-appearance")?
        }
        Some("appearance-writer-oracle") => {
            profile_oracle::generate(root, &args, "appearance-writer")?
        }
        Some("inventory-listener-oracle") => {
            profile_oracle::generate(root, &args, "inventory-listener")?
        }
        Some("player-inventory-oracle") => {
            profile_oracle::generate(root, &args, "player-inventory")?
        }
        Some("item-config-oracle") => item_config_oracle::generate(root, &args)?,
        Some("item-config-audit") => item_config_oracle::audit(root, &args)?,
        Some("timers-oracle") => profile_oracle::generate(root, &args, "timers")?,
        Some("core-profile-oracle") => profile_oracle::generate(root, &args, "core-profile")?,
        Some("player-save-oracle") => profile_oracle::generate(root, &args, "player-save")?,
        Some("timer-registry-oracle") => profile_oracle::generate(root, &args, "timer-registry")?,
        Some("string-case-oracle") => profile_oracle::generate(root, &args, "string-case")?,
        Some("pulses-oracle") => profile_oracle::generate(root, &args, "pulses")?,
        Some("scripts-oracle") => profile_oracle::generate(root, &args, "scripts")?,
        Some("script-runtime-oracle") => profile_oracle::generate(root, &args, "script-runtime")?,
        Some("action-locks-oracle") => profile_oracle::generate(root, &args, "action-locks")?,
        Some("projectile-oracle") => profile_oracle::generate(root, &args, "projectile")?,
        Some("combat-swing-oracle") => profile_oracle::generate(root, &args, "combat-swing")?,
        Some("combat-pulse-oracle") => profile_oracle::generate(root, &args, "combat-pulse")?,
        Some("game-packets-oracle") => profile_oracle::generate(root, &args, "game-packets")?,
        Some("doors-oracle") => profile_oracle::generate(root, &args, "doors")?,
        Some("gathering-oracle") => profile_oracle::generate(root, &args, "gathering")?,
        Some("scenery-oracle") => profile_oracle::generate(root, &args, "scenery")?,
        Some("clipping-oracle") => profile_oracle::generate(root, &args, "clipping")?,
        Some("chunk-oracle") => profile_oracle::generate(root, &args, "chunk")?,
        Some("landscape-oracle") => profile_oracle::generate(root, &args, "landscape")?,
        Some("scenery-definition-oracle") => {
            profile_oracle::generate(root, &args, "scenery-definitions")?
        }
        Some("scenery-definition-samples") => scenery_definition_oracle::samples(root)?,
        Some("scenery-definition-audit") => scenery_definition_oracle::audit(root, &args)?,
        Some("mapscape-oracle") => profile_oracle::generate(root, &args, "mapscape")?,
        Some("scenery-node-oracle") => profile_oracle::generate(root, &args, "scenery-node")?,
        Some("region-load-oracle") => profile_oracle::generate(root, &args, "region-load")?,
        Some("region-queries-oracle") => profile_oracle::generate(root, &args, "region-queries")?,
        Some("update-masks-oracle") => profile_oracle::generate(root, &args, "update-masks")?,
        Some("npc-masks-oracle") => profile_oracle::generate(root, &args, "npc-masks")?,
        Some("prepared-masks-oracle") => profile_oracle::generate(root, &args, "prepared-masks")?,
        Some("animator-oracle") => profile_oracle::generate(root, &args, "animator")?,
        Some("animator-audit") => animator_oracle::audit(root, &args)?,
        Some("scene-oracle") => profile_oracle::generate(root, &args, "scene")?,
        Some("player-render-oracle") => profile_oracle::generate(root, &args, "player-render")?,
        Some("appearance-oracle") => profile_oracle::generate(root, &args, "appearance")?,
        Some("walking-oracle") => profile_oracle::generate(root, &args, "walking")?,
        Some("pathfinding-oracle") => profile_oracle::generate(root, &args, "pathfinding")?,
        Some("movement-oracle") => profile_oracle::generate(root, &args, "movement")?,
        Some("path-dispatch-oracle") => profile_oracle::generate(root, &args, "path-dispatch")?,
        Some("navigation-oracle") => profile_oracle::generate(root, &args, "navigation")?,
        Some("walk-request-oracle") => profile_oracle::generate(root, &args, "walk-request")?,
        Some("scenery-action-oracle") => profile_oracle::generate(root, &args, "scenery-action")?,
        Some("legacy-interaction-oracle") => {
            profile_oracle::generate(root, &args, "legacy-interaction")?
        }
        Some("interaction-options-oracle") => {
            profile_oracle::generate(root, &args, "interaction-options")?
        }
        Some("ground-action-oracle") => profile_oracle::generate(root, &args, "ground-action")?,
        Some("ground-options-oracle") => profile_oracle::generate(root, &args, "ground-options")?,
        Some("scenery-options-oracle") => profile_oracle::generate(root, &args, "scenery-options")?,
        Some("ground-target-oracle") => profile_oracle::generate(root, &args, "ground-target")?,
        Some("item-option-registry-oracle") => {
            profile_oracle::generate(root, &args, "item-option-registry")?
        }
        Some("interaction-runtime-oracle") => {
            profile_oracle::generate(root, &args, "interaction-runtime")?
        }
        Some("world-pulses-oracle") => profile_oracle::generate(root, &args, "world-pulses")?,
        Some("map-chunk-render-oracle") => {
            profile_oracle::generate(root, &args, "map-chunk-render")?
        }
        Some("chunk-sync-oracle") => profile_oracle::generate(root, &args, "chunk-sync")?,
        Some("clear-chunk-oracle") => profile_oracle::generate(root, &args, "clear-chunk")?,
        Some("pickup-oracle") => profile_oracle::generate(root, &args, "pickup")?,
        Some("ground-items-oracle") => profile_oracle::generate(root, &args, "ground-items")?,
        Some("ground-manager-oracle") => profile_oracle::generate(root, &args, "ground-manager")?,
        Some("ground-spawn-oracle") => profile_oracle::generate(root, &args, "ground-spawn")?,
        Some("ground-spawn-loader-oracle") => {
            profile_oracle::generate(root, &args, "ground-spawn-loader")?
        }
        Some("tick-oracle") => profile_oracle::generate(root, &args, "tick")?,
        Some("player-walk-oracle") => profile_oracle::generate(root, &args, "player-walk")?,
        Some("world-update-oracle") => profile_oracle::generate(root, &args, "world-update")?,
        Some("npc-render-oracle") => profile_oracle::generate(root, &args, "npc-render")?,
        Some("npc-definition-oracle") => profile_oracle::generate(root, &args, "npc-definitions")?,
        Some("npc-init-oracle") => profile_oracle::generate(root, &args, "npc-init")?,
        Some("entity-regions-oracle") => profile_oracle::generate(root, &args, "entity-regions")?,
        Some("interfaces-oracle") => profile_oracle::generate(root, &args, "interfaces")?,
        Some("settings-oracle") => profile_oracle::generate(root, &args, "settings")?,
        Some("npc-definition-audit") => npc_definition_oracle::audit(root, &args)?,
        Some("npc-tick-oracle") => profile_oracle::generate(root, &args, "npc-tick")?,
        Some("npc-spawns-oracle") => profile_oracle::generate(root, &args, "npc-spawns")?,
        Some("drop-tables-oracle") => profile_oracle::generate(root, &args, "drop-tables")?,
        Some("npc-death-oracle") => profile_oracle::generate(root, &args, "npc-death")?,
        Some("player-death-oracle") => profile_oracle::generate(root, &args, "player-death")?,
        Some("npc-behaviour-oracle") => profile_oracle::generate(root, &args, "npc-behaviour")?,
        Some("zones-oracle") => profile_oracle::generate(root, &args, "zones")?,
        Some("repository-oracle") => profile_oracle::generate(root, &args, "repository")?,
        Some("social-oracle") => profile_oracle::generate(root, &args, "social")?,
        Some("world-trace-oracle") => profile_oracle::generate(root, &args, "world-trace")?,
        Some("startup-loaders-oracle") => profile_oracle::generate(root, &args, "startup-loaders")?,
        Some("startup-draws-oracle") => profile_oracle::generate(root, &args, "startup-draws")?,
        Some("canonical-player-oracle") => {
            profile_oracle::generate(root, &args, "canonical-player")?
        }
        Some("world-login-oracle") => profile_oracle::generate(root, &args, "world-login")?,
        Some("dumb-path-oracle") => profile_oracle::generate(root, &args, "dumb-path")?,
        Some("trade-oracle") => profile_oracle::generate(root, &args, "trade")?,
        Some("shops-oracle") => profile_oracle::generate(root, &args, "shops")?,
        Some("prayer-oracle") => profile_oracle::generate(root, &args, "prayer")?,
        Some("teleport-oracle") => profile_oracle::generate(root, &args, "teleport")?,
        Some("wilderness-oracle") => profile_oracle::generate(root, &args, "wilderness")?,
        Some("spells-oracle") => profile_oracle::generate(root, &args, "spells")?,
        Some("content-timers-oracle") => profile_oracle::generate(root, &args, "content-timers")?,
        Some("player-tabs-oracle") => profile_oracle::generate(root, &args, "player-tabs")?,
        Some("grand-exchange-oracle") => profile_oracle::generate(root, &args, "grand-exchange")?,
        Some("player-managers-oracle") => profile_oracle::generate(root, &args, "player-managers")?,
        Some("packet-handlers-oracle") => profile_oracle::generate(root, &args, "packet-handlers")?,
        Some("commands-oracle") => profile_oracle::generate(root, &args, "commands")?,
        Some("command-sets-oracle") => profile_oracle::generate(root, &args, "command-sets")?,
        Some("global-handlers-oracle") => profile_oracle::generate(root, &args, "global-handlers")?,
        Some("global-items-oracle") => profile_oracle::generate(root, &args, "global-items")?,
        Some("global-items2-oracle") => profile_oracle::generate(root, &args, "global-items2")?,
        Some("content-api-oracle") => profile_oracle::generate(root, &args, "content-api")?,
        Some("consumables-oracle") => profile_oracle::generate(root, &args, "consumables")?,
        Some("force-movement-oracle") => profile_oracle::generate(root, &args, "force-movement")?,
        Some("npc-classes-oracle") => profile_oracle::generate(root, &args, "npc-classes")?,
        Some("dialogue-dsl-oracle") => profile_oracle::generate(root, &args, "dialogue-dsl")?,
        Some("special-attacks-oracle") => profile_oracle::generate(root, &args, "special-attacks")?,
        Some("dragonfire-oracle") => profile_oracle::generate(root, &args, "dragonfire")?,
        Some("equipment-registers-oracle") => {
            profile_oracle::generate(root, &args, "equipment-registers")?
        }
        Some("wield-requirements-oracle") => profile_oracle::generate(root, &args, "wield-requirements")?,
        Some("dynamic-regions-oracle") => dynamic_regions_oracle::generate(root, &args)?,
        Some("familiars-oracle") => profile_oracle::generate(root, &args, "familiars")?,
        Some("scene-audit") => scene_oracle::audit(root, &args)?,
        _ => println!(
            "cargo xtask <bootstrap | oracle [--check] | cache-oracle [--check] | cache-fetch | cache-audit [--jvm] | network-oracle [--check] | input-oracle [--check] | headless-lifecycle-oracle [--check] | network-interop | network-smoke [--clients N] [--output DIR] | login-oracle [--check] | login-interop | auth-oracle [--check] | auth-interop | sql-oracle [--check] | config-oracle [--check] | player-oracle [--check] | saved-data-oracle [--check] | attributes-oracle [--check] | ironman-oracle [--check] | bank-oracle [--check] | bank-interface-oracle [--check] | input-prompt-oracle [--check] | plain-dialogue-oracle [--check] | dialogue-oracle [--check] | interaction-listeners-oracle [--check] | variables-oracle [--check] | bank-pin-oracle [--check] | skills-oracle [--check] | skills-bench | item-oracle [--check] | item-audit | item-samples | item-config-oracle [--check] | item-config-audit [--output DIR] | inventory-oracle [--check] | container-ownership-oracle [--check] | container-listeners-oracle [--check] | owned-player-oracle [--check] | equipment-oracle [--check] | player-appearance-oracle [--check] | appearance-writer-oracle [--check] | inventory-listener-oracle [--check] | player-inventory-oracle [--check] | timers-oracle [--check] | core-profile-oracle [--check] | player-save-oracle [--check] | timer-registry-oracle [--check] | string-case-oracle [--check] | pulses-oracle [--check] | scripts-oracle [--check] | script-runtime-oracle [--check] | action-locks-oracle [--check] | projectile-oracle [--check] | combat-swing-oracle [--check] | combat-pulse-oracle [--check] | game-packets-oracle [--check] | doors-oracle [--check] | scenery-oracle [--check] | clipping-oracle [--check] | chunk-oracle [--check] | landscape-oracle [--check] | mapscape-oracle [--check] | scenery-definition-oracle [--check] | scenery-definition-samples | scenery-definition-audit | scenery-node-oracle [--check] | region-load-oracle [--check] | region-queries-oracle [--check] | update-masks-oracle [--check] | npc-masks-oracle [--check] | prepared-masks-oracle [--check] | animator-oracle [--check] | animator-audit [--output DIR] | scene-oracle [--check] | scene-audit [--output DIR] | player-render-oracle [--check] | appearance-oracle [--check] | walking-oracle [--check] | pathfinding-oracle [--check] | movement-oracle [--check] | path-dispatch-oracle [--check] | navigation-oracle [--check] | walk-request-oracle [--check] | scenery-action-oracle [--check] | legacy-interaction-oracle [--check] | interaction-options-oracle [--check] | ground-action-oracle [--check] | ground-options-oracle [--check] | scenery-options-oracle [--check] | ground-target-oracle [--check] | item-option-registry-oracle [--check] | interaction-runtime-oracle [--check] | world-pulses-oracle [--check] | map-chunk-render-oracle [--check] | chunk-sync-oracle [--check] | clear-chunk-oracle [--check] | pickup-oracle [--check] | ground-items-oracle [--check] | ground-manager-oracle [--check] | ground-spawn-oracle [--check] | ground-spawn-loader-oracle [--check] | tick-oracle [--check] | player-walk-oracle [--check] | world-update-oracle [--check] | npc-render-oracle [--check] | npc-definition-oracle [--check] | npc-definition-audit | npc-init-oracle [--check] | entity-regions-oracle [--check] | interfaces-oracle [--check] | settings-oracle [--check] | npc-tick-oracle [--check] | npc-spawns-oracle [--check] | drop-tables-oracle [--check] | world-trace-oracle [--check] | startup-loaders-oracle [--check] | startup-draws-oracle [--check] | canonical-player-oracle [--check] | world-login-oracle [--check] | npc-death-oracle [--check] | player-death-oracle [--check] | npc-behaviour-oracle [--check] | zones-oracle [--check] | repository-oracle [--check] | social-oracle [--check] | dumb-path-oracle [--check] | trade-oracle [--check] | shops-oracle [--check] | prayer-oracle [--check] | teleport-oracle [--check] | wilderness-oracle [--check] | spells-oracle [--check] | content-timers-oracle [--check] | player-tabs-oracle [--check] | grand-exchange-oracle [--check] | player-managers-oracle [--check] | packet-handlers-oracle [--check] | commands-oracle [--check] | command-sets-oracle [--check] | global-handlers-oracle [--check] | global-items-oracle [--check] | global-items2-oracle [--check] | consumables-oracle [--check] | force-movement-oracle [--check] | npc-classes-oracle [--check] | dialogue-dsl-oracle [--check] | special-attacks-oracle [--check] | dragonfire-oracle [--check] | equipment-registers-oracle [--check] | wield-requirements-oracle [--check] | dynamic-regions-oracle [--check] | familiars-oracle [--check] | content-api-oracle [--check] | inventory | bench>\nPython-based oracle commands accept --python PATH and --java-home PATH."
        ),
    }
    Ok(())
}

fn run(command: &mut Command) -> Result<Output> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "command {:?} failed with {}:\n{}{}",
            command.get_program(),
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = run(Command::new("git").current_dir(root).args(args))?;
    Ok(String::from_utf8(output.stdout)?.trim_end().to_owned())
}

fn lock(root: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(
        root.join("upstream.lock.json"),
    )?)?)
}

fn verify_reference(root: &Path) -> Result<Value> {
    let upstream = lock(root)?;
    for component in ["client", "server"] {
        let path = root.join("reference").join(component);
        let actual = git(&path, &["rev-parse", "HEAD"])?;
        let expected = upstream[component]["commit"]
            .as_str()
            .ok_or("missing commit")?;
        if actual != expected {
            return Err(format!("{component} revision differs: {actual} != {expected}").into());
        }
        git(&path, &["diff", "--quiet", "HEAD", "--"])?;
    }
    Ok(upstream)
}

fn bootstrap(root: &Path) -> Result<()> {
    let upstream = lock(root)?;
    fs::create_dir_all(root.join("reference"))?;
    for component in ["server", "client"] {
        let path = root.join("reference").join(component);
        let expected = upstream[component]["commit"]
            .as_str()
            .ok_or("missing commit")?;
        if path.exists() {
            let actual = git(&path, &["rev-parse", "HEAD"])?;
            if actual != expected {
                return Err(
                    format!("existing {component} is not pinned; refusing to reset it").into(),
                );
            }
            continue;
        }
        let url = upstream[component]["url"]
            .as_str()
            .ok_or("missing repository URL")?;
        run(Command::new("git")
            .env("GIT_LFS_SKIP_SMUDGE", "1")
            .args([
                "clone",
                "--depth=1",
                "--filter=blob:none",
                "--no-checkout",
                url,
            ])
            .arg(&path))?;
        git(&path, &["sparse-checkout", "init", "--cone"])?;
        if component == "server" {
            git(
                &path,
                &[
                    "sparse-checkout",
                    "set",
                    "Server/src",
                    "Server/worldprops",
                    "Server/.mvn",
                ],
            )?;
        } else {
            git(
                &path,
                &[
                    "sparse-checkout",
                    "set",
                    "client/src",
                    "deob-annotations/src",
                    "signlink/src",
                ],
            )?;
        }
        git(&path, &["fetch", "--depth=1", "origin", expected])?;
        run(Command::new("git")
            .current_dir(&path)
            .env("GIT_LFS_SKIP_SMUDGE", "1")
            .args(["checkout", "--detach", expected]))?;
    }
    verify_reference(root)?;
    println!("Pinned server and client references are ready.");
    Ok(())
}

fn normalized(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .copied()
        .filter(|byte| *byte != b'\r')
        .collect()
}

fn source_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let mut sources: Vec<_> = CLIENT_SOURCES
        .iter()
        .map(|name| {
            root.join(format!(
                "reference/client/client/src/main/java/rt4/{name}.java"
            ))
        })
        .collect();
    let annotation_dir =
        root.join("reference/client/deob-annotations/src/main/java/org/openrs2/deob/annotation");
    for entry in fs::read_dir(annotation_dir)? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "java")
        {
            sources.push(path);
        }
    }
    sources.push(root.join("oracle/adapters/rt4/JagString.java"));
    for name in ["Direction.java", "Point.java", "path/ClipMaskSupplier.java"] {
        sources.push(
            root.join("reference/server/Server/src/main/core/game/world/map")
                .join(name),
        );
    }
    for name in ["Location", "RegionManager"] {
        sources.push(root.join(format!("oracle/adapters/core/game/world/map/{name}.java")));
    }
    sources.push(root.join("oracle/OracleMain.java"));
    sources.push(root.join("oracle/OracleBenchmark.java"));
    sources.push(root.join("oracle/CacheOracle.java"));
    sources.push(root.join("reference/client/signlink/src/main/java/rt4/FileOnDisk.java"));
    sources.sort();
    Ok(sources)
}

fn oracle(root: &Path, check: bool) -> Result<()> {
    let upstream = verify_reference(root)?;
    let sources = source_paths(root)?;
    let mut hashes = BTreeMap::new();
    for source in &sources {
        let path = source
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        hashes.insert(
            path,
            format!("{:x}", Sha256::digest(normalized(&fs::read(source)?))),
        );
    }
    let build = root.join("runs/oracle");
    fs::create_dir_all(build.join("classes"))?;
    // Generate a valid headerless BZip2 container. Only decoding is under test.
    let payload: Vec<u8> = (0..300_001)
        .map(|i| ((i * 17 + i / 127) & 255) as u8)
        .collect();
    let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
    encoder.write_all(&payload)?;
    let compressed = encoder.finish()?;
    let mut container = rs09_protocol::buffer::Writer::new();
    container.p1(1);
    container.p4(i32::try_from(compressed.len() - 4)?);
    container.p4(i32::try_from(payload.len())?);
    container.pdata(&compressed[4..]);
    let bzip_path = build.join("bzip.js5");
    fs::write(&bzip_path, container.as_slice())?;
    run(Command::new("javac")
        .current_dir(root)
        .args(["--release", "8", "-encoding", "UTF-8", "-d"])
        .arg(build.join("classes"))
        .args(&sources))?;
    let output = run(Command::new("java")
        .current_dir(root)
        .arg("-cp")
        .arg(build.join("classes"))
        .arg("OracleMain")
        .arg(bzip_path))?;
    let fixture = normalized(&output.stdout);
    let count = fixture
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .count();
    for line in fixture
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let _: Value = serde_json::from_slice(line)?;
    }
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "fixtureSha256": format!("{:x}", Sha256::digest(&fixture)),
        "caseCount": count,
        "scope": "Selected RT4 primitives/collision and server Direction. JagString/Location are isolated test adapters. RegionManager throws."
    });
    let fixture_path = root.join("fixtures/rt4-v1.jsonl");
    let metadata_path = root.join("fixtures/rt4-v1.provenance.json");
    if check {
        if normalized(&fs::read(&fixture_path)?) != fixture {
            return Err(
                "JVM outputs differ from committed fixtures; review changes before regeneration"
                    .into(),
            );
        }
        let committed: Value = serde_json::from_slice(&fs::read(metadata_path)?)?;
        if committed != provenance {
            return Err(
                "fixture provenance is stale; review source changes before regeneration".into(),
            );
        }
        println!("{count} JVM oracle cases reproduced with matching pinned source hashes.");
    } else {
        fs::create_dir_all(root.join("fixtures"))?;
        fs::write(fixture_path, fixture)?;
        fs::write(
            metadata_path,
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!("Generated {count} JVM oracle cases. Run cargo test --workspace to compare Rust.");
    }
    Ok(())
}

fn inventory(root: &Path) -> Result<()> {
    let upstream = verify_reference(root)?;
    let mut entries = Vec::new();
    for component in ["server", "client"] {
        let path = root.join("reference").join(component);
        let files = git(&path, &["ls-files", "-s", "--", "*.java", "*.kt"])?;
        for line in files.lines() {
            let (header, source) = line.split_once('\t').ok_or("invalid git inventory")?;
            let blob = header.split_whitespace().nth(1).ok_or("missing blob ID")?;
            entries.push(json!({
                "component": component, "path": source, "gitBlob": blob, "status": "not_replaced"
            }));
        }
    }
    fs::create_dir_all(root.join("parity"))?;
    fs::write(
        root.join("parity/source-inventory.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "format": 1, "upstream": upstream,
                "note": "File inventory, not a completion percentage. Partial function ports are tracked separately.",
                "sourceFileCount": entries.len(), "files": entries
            }))?
        ),
    )?;
    println!(
        "Inventoried {} Java/Kotlin source files; no full-file replacement claimed.",
        entries.len()
    );
    Ok(())
}
