//! `cargo xtask input-oracle [--check]`: the unchanged RT4 `Keyboard` and
//! `Mouse` listeners, driven with real AWT event objects on a headless
//! canvas; their observations become `fixtures/input`.
use crate::{Result, normalized, run};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// The two unchanged client classes, then the adapters and the driver.
const SOURCES: &[&str] = &[
    "reference/client/client/src/main/java/rt4/Keyboard.java",
    "reference/client/client/src/main/java/rt4/Mouse.java",
    "oracle/input/adapters/rt4/SignLink.java",
    "oracle/input/adapters/rt4/MonotonicClock.java",
    "oracle/input/InputOracle.java",
];
/// `InputOracle` arguments: seed, keyboard scenarios, mouse scenarios.
const ARGUMENTS: [&str; 3] = ["20261116", "18", "12"];

pub fn generate(root: &Path, check: bool) -> Result<()> {
    let upstream = crate::verify_reference(root)?;
    let mut hashes = BTreeMap::new();
    for source in SOURCES {
        hashes.insert(
            *source,
            format!(
                "{:x}",
                Sha256::digest(normalized(&fs::read(root.join(source))?))
            ),
        );
    }
    let build = root.join("runs/input-oracle");
    let classes = build.join("classes");
    if classes.exists() {
        fs::remove_dir_all(&classes)?;
    }
    fs::create_dir_all(&classes)?;
    run(Command::new("javac")
        .current_dir(root)
        .args(["--release", "8", "-encoding", "UTF-8", "-nowarn"])
        .arg("-sourcepath")
        .arg("reference/client/deob-annotations/src/main/java")
        .arg("-d")
        .arg(&classes)
        .args(SOURCES))?;
    let outputs = [build.join("keyboard.txt"), build.join("mouse.txt")];
    run(Command::new("java")
        .current_dir(root)
        .arg("-Djava.awt.headless=true")
        .arg("-cp")
        .arg(&classes)
        .arg("InputOracle")
        .args(ARGUMENTS)
        .args(&outputs))?;
    // Printed, not recorded: the fixture does not depend on the patch level.
    let version = run(Command::new("java").arg("-version"))?;
    let runtime = String::from_utf8_lossy(&version.stderr)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let keyboard = normalized(&fs::read(&outputs[0])?);
    let mouse = normalized(&fs::read(&outputs[1])?);
    let lines = |bytes: &[u8]| bytes.iter().filter(|&&byte| byte == b'\n').count();
    let provenance = json!({
        "format": 1,
        "upstream": upstream,
        "sourceHashAlgorithm": "sha256, CR removed for consistent LF/CRLF checkouts",
        "sources": hashes,
        "arguments": ARGUMENTS,
        "keyboardSha256": format!("{:x}", Sha256::digest(&keyboard)),
        "keyboardLines": lines(&keyboard),
        "mouseSha256": format!("{:x}", Sha256::digest(&mouse)),
        "mouseLines": lines(&mouse),
        "scope": "Unchanged rt4.Keyboard and rt4.Mouse called as AWT calls them, with real java.awt.event KeyEvent, MouseEvent and FocusEvent objects on a headless Canvas, in seeded scenarios: every listener method, loop, nextKey, setIdleLoops, stop and quit, all three Keyboard.init branches, both rings filled past their 128 entries, and javax.swing.SwingUtilities.isMiddleMouseButton as the running JDK implements it (JDK 9 and later test the extended button-2 mask). SignLink supplies the two values Keyboard.init reads; MonotonicClock is a scripted clock. Static state is reset between scenarios to the field initializers. No window, native peer, operating-system key translation or auto-repeat is observed, and event consumption is not recorded."
    });
    let directory = root.join("fixtures/input");
    let paths = [
        directory.join("keyboard.txt"),
        directory.join("mouse.txt"),
        directory.join("provenance.json"),
    ];
    if check {
        if normalized(&fs::read(&paths[0])?) != keyboard
            || normalized(&fs::read(&paths[1])?) != mouse
        {
            return Err(
                "input oracle output differs from the committed fixture; review before regenerating"
                    .into(),
            );
        }
        if serde_json::from_slice::<Value>(&fs::read(&paths[2])?)? != provenance {
            return Err("input oracle provenance is stale; review source changes".into());
        }
        println!(
            "{} keyboard and {} mouse JVM observations reproduced with matching sources ({runtime}).",
            lines(&keyboard),
            lines(&mouse)
        );
    } else {
        fs::create_dir_all(&directory)?;
        fs::write(&paths[0], &keyboard)?;
        fs::write(&paths[1], &mouse)?;
        fs::write(
            &paths[2],
            format!("{}\n", serde_json::to_string_pretty(&provenance)?),
        )?;
        println!(
            "Generated {} keyboard and {} mouse JVM observations ({runtime}).",
            lines(&keyboard),
            lines(&mouse)
        );
    }
    Ok(())
}
