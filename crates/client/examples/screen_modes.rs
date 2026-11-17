//! `cargo run -p rs09-client --example screen_modes [-- --record]`: this
//! machine's screen modes as the original gets them and as this client gets
//! them, compared.
//!
//! The original's list comes from the unchanged signlink class
//! `rt4.FullScreenManager` run in a JVM (`oracle/display/screen`), which asks
//! `GraphicsDevice.getDisplayModes()` of the default screen. This client's
//! comes from `rs09_client::full_screen::screen_modes`. Both are queries: no
//! window is shown and the screen's mode is not changed. The two lists must
//! be equal, entry for entry and in order. The list `DisplayMode.getDisplayModes`
//! makes of it (as `rs09_client::display` computes it) is printed.
//!
//! `--record` also writes `fixtures/display/screen-modes.json`, from which
//! `cargo xtask display-oracle` runs the unchanged `DisplayMode` over this
//! list (`fixtures/display/screen.jsonl`, replayed by `tests/display.rs`).
//! Windows only: elsewhere this client lists no modes.
use rs09_client::display::{Display, Mode, System};
use std::{path::Path, process::Command};

fn run(command: &mut Command) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|error| format!("{command:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn flat(modes: &[Mode]) -> Vec<i32> {
    modes
        .iter()
        .flat_map(|mode| [mode.width, mode.height, mode.bit_depth, mode.refresh_rate])
        .collect()
}

fn main() -> Result<(), String> {
    let record = std::env::args().any(|arg| arg == "--record");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let classes = root.join("runs/screen-modes/classes");
    std::fs::create_dir_all(&classes).map_err(|error| error.to_string())?;
    run(Command::new("javac")
        .current_dir(&root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .arg("reference/client/signlink/src/main/java/rt4/FullScreenManager.java")
        .arg("oracle/display/screen/ScreenModes.java"))?;
    let printed = run(Command::new("java")
        .current_dir(&root)
        .arg("-cp")
        .arg(&classes)
        .arg("screen.ScreenModes"))?;
    let jvm: Vec<i32> = serde_json::from_str(printed.trim()).map_err(|error| error.to_string())?;
    let ours = rs09_client::full_screen::screen_modes().ok_or("this system lists no modes")?;
    let rust = flat(&ours);
    println!(
        "JVM (FullScreenManager.getDisplayModes): {} modes; this client: {} modes",
        jvm.len() / 4,
        ours.len()
    );
    let mut listed = Display::new([1024, 768], System::original(ours.clone()));
    let sizes: Vec<String> = listed
        .display_modes()
        .iter()
        .map(|mode| format!("{}x{}", mode.width, mode.height))
        .collect();
    println!(
        "DisplayMode.getDisplayModes ({}): {}",
        sizes.len(),
        sizes.join(" ")
    );
    if jvm != rust {
        let first = jvm
            .chunks(4)
            .zip(rust.chunks(4))
            .position(|(a, b)| a != b)
            .unwrap_or(jvm.len().min(rust.len()) / 4);
        return Err(format!("the lists differ, first at mode {first}"));
    }
    println!("Equal, entry for entry.");
    if record {
        let path = root.join("fixtures/display/screen-modes.json");
        let text = serde_json::to_string_pretty(&serde_json::json!({
            "format": 1,
            "about": "One monitor's modes, recorded by `cargo run -p rs09-client --example screen_modes -- --record`: \"jvm\" is what the unchanged rt4.FullScreenManager.getDisplayModes returned in a JVM (GraphicsDevice.getDisplayModes of the default screen), \"rust\" what rs09_client::full_screen::screen_modes returned (EnumDisplaySettings of the primary monitor); width, height, bit depth and refresh rate of each mode in order. Both are queries; the screen's mode was not changed.",
            "jvm": jvm,
            "rust": rust,
        }))
        .map_err(|error| error.to_string())?;
        std::fs::write(&path, format!("{text}\n")).map_err(|error| error.to_string())?;
        println!("Wrote {}", path.display());
    }
    Ok(())
}
