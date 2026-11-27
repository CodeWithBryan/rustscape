//! `cargo xtask client-smoke`: drive the real visual client against a
//! disposable `rs09-server --world` and check, from the client's structured
//! capture files, that it logs in, loads the scene around Lumbridge with
//! NPCs, can point at a goblin, opens a menu with Attack for it and sends
//! the attack (docs/client-smoke.md).
//!
//! It opens a window: run it only when that is wanted. The client is always
//! started with `--mute`; on Windows its window starts minimized and is not
//! activated (`--show` shows it, still without taking focus). The client and
//! the world it started are stopped when it ends, on failure too. Port 43594
//! is never used (the test world never chooses it).
use crate::Result;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const USAGE: &str = "cargo xtask client-smoke [--data DIR] [--cache DIR] [--no-build] [--show] [--npc NAME] [--timeout SECS] [--keep-world] [--skip-welcome]";
/// Next to the goblins east of Lumbridge castle (the test-world kit's
/// goblin scenario starts here too).
const LOCATION: [i32; 3] = [3250, 3235, 0];
const CHARACTER: &str = "smokegoblin";
/// The NPC option packets, options 1 to 5 (`ClientAction::NpcAction` in
/// protocol/game/actions.rs).
const NPC_OPTION_OPCODES: [u64; 5] = [78, 3, 148, 30, 218];
/// The capture files the checks read, from the client's `runs/visual`.
const CAPTURE_FILES: [&str; 5] = [
    "npc-menu-capture.json",
    "pick-state.json",
    "live-state.json",
    "loaded.json",
    "menu-state.json",
];

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let mut data = None;
    let mut cache = None;
    let mut build = true;
    let mut show = false;
    let mut keep_world = false;
    let mut skip_welcome = false;
    let mut npc = "Goblin".to_owned();
    let mut timeout = Duration::from_secs(300);
    let mut options = args.iter().skip(1);
    while let Some(option) = options.next() {
        let mut value = || options.next().ok_or(format!("missing value for {option}"));
        match option.as_str() {
            "--data" => data = Some(value()?.clone()),
            "--cache" => cache = Some(value()?.clone()),
            "--no-build" => build = false,
            "--show" => show = true,
            "--keep-world" => keep_world = true,
            "--skip-welcome" => skip_welcome = true,
            "--npc" => npc = value()?.clone(),
            "--timeout" => timeout = Duration::from_secs(value()?.parse()?),
            other => return Err(format!("unknown client-smoke option {other}\n{USAGE}").into()),
        }
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let name = format!("client-smoke-{secs}");
    let out = root.join("runs/client-smoke").join(&name);
    fs::create_dir_all(&out)?;

    if build {
        for args in [
            &["build", "--release", "-p", "rs09-server"][..],
            &[
                "build",
                "--release",
                "-p",
                "rs09-client",
                "--bin",
                "rs09-client",
            ],
            &[
                "build",
                "--release",
                "-p",
                "rs09-headless",
                "--example",
                "agent_run",
            ],
        ] {
            let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .current_dir(root)
                .args(args)
                .status()?;
            if !status.success() {
                return Err(format!("cargo {} failed ({status})", args.join(" ")).into());
            }
        }
    }

    let mut world_args = vec!["--name".to_owned(), name.clone(), "--no-build".to_owned()];
    if let Some(data) = &data {
        world_args.extend(["--data".to_owned(), data.clone()]);
    }
    if let Some(cache) = &cache {
        world_args.extend(["--cache".to_owned(), cache.clone()]);
    }
    let world = crate::test_world::start_world(root, &world_args)?;
    // From here on the world (and later the client) is stopped on every
    // path, a panic included (`Run`'s drop).
    let mut run = Run {
        root: root.to_owned(),
        name: name.clone(),
        client: None,
        keep_world,
    };
    let outcome = smoke(&mut run, &world, &out, &npc, show, skip_welcome, timeout);
    let stopped = run.stop();
    let report = match outcome {
        Ok(report) => report,
        Err(error) => json!({
            "pass": false,
            "error": error.to_string(),
            "checks": check(&out, &npc)["checks"],
        }),
    };
    let report = finish_report(report, &world, &out, &npc)?;
    stopped?;
    if report["pass"].as_bool() == Some(true) {
        Ok(())
    } else {
        Err(format!(
            "client smoke test failed; see {}",
            out.join("report.md").display()
        )
        .into())
    }
}

/// What has to be stopped at the end.
struct Run {
    root: PathBuf,
    name: String,
    client: Option<Client>,
    keep_world: bool,
}

impl Run {
    fn stop(&mut self) -> Result<()> {
        let mut result = Ok(());
        if let Some(mut client) = self.client.take() {
            if let Err(e) = client.stop() {
                result = Err(e);
            }
        }
        if !self.keep_world {
            // Only the world this run started; stopping twice is harmless.
            let args = ["--name".to_owned(), self.name.clone()];
            if let Err(e) = crate::test_world::stop(&self.root, &args) {
                result = Err(e);
            }
            self.keep_world = true;
        }
        result
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn smoke(
    run: &mut Run,
    world: &Value,
    out: &Path,
    npc: &str,
    show: bool,
    skip_welcome: bool,
    timeout: Duration,
) -> Result<Value> {
    let root = run.root.clone();
    let address = world["address"]
        .as_str()
        .ok_or("world.json has no address")?;
    let world_dir = world["dir"].as_str().ok_or("world.json has no dir")?;
    let data = PathBuf::from(world["data"].as_str().ok_or("world.json has no data")?);
    let cache = PathBuf::from(world["cache"].as_str().ok_or("world.json has no cache")?);

    // Seed the character with the test-world kit's runner: it writes the
    // save, logs in and out once and checks the location took.
    let script = out.join("seed.json");
    fs::write(
        &script,
        serde_json::to_string_pretty(&json!({
            "character": {
                "name": CHARACTER,
                "location": LOCATION,
                "skills": { "attack": 20, "strength": 20, "defence": 20, "hitpoints": 20 },
                "equipment": [{ "slot": 3, "id": 1277 }],
                // Without `tutorial:complete` the login skips the welcome
                // screen (LoginConfiguration.configureLobby).
                "tutorialComplete": !skip_welcome,
            },
            "steps": [{ "do": "snapshot" }],
        }))?,
    )?;
    let agent = root.join(format!(
        "target/release/examples/agent_run{}",
        std::env::consts::EXE_SUFFIX
    ));
    let seeded = Command::new(&agent)
        .current_dir(&root)
        .arg("--world")
        .arg(world_dir)
        .arg(&script)
        .stdin(Stdio::null())
        .output()?;
    fs::write(out.join("seed-report.json"), &seeded.stdout)?;
    fs::write(out.join("seed.err.log"), &seeded.stderr)?;
    if !seeded.status.success() {
        return Err(format!(
            "seeding the character failed ({}); see {}",
            seeded.status,
            out.join("seed.err.log").display()
        )
        .into());
    }
    // The disconnection queue removes the agent's character a few ticks
    // after it closed.
    std::thread::sleep(Duration::from_secs(3));

    // The client's capture files go to `runs/visual` under the working
    // directory; clear the ones the checks read so none is left over.
    let visual = root.join("runs/visual");
    for file in CAPTURE_FILES {
        let _ = fs::remove_file(visual.join(file));
    }
    let client = root.join(format!(
        "target/release/rs09-client{}",
        std::env::consts::EXE_SUFFIX
    ));
    let mut arguments: Vec<String> = vec![
        "--connect".into(),
        address.into(),
        "--username".into(),
        CHARACTER.into(),
        "--mute".into(),
        "--no-music".into(),
        "--capture".into(),
        // The script ends the capture; the frame is only a backstop.
        "--capture-frame".into(),
        "1000000".into(),
        "--capture-npc-menu".into(),
        npc.into(),
        "--cache".into(),
        cache.display().to_string(),
    ];
    let keys = data.join("data/configs/xteas.json");
    if keys.is_file() {
        arguments.extend(["--world-keys".into(), keys.display().to_string()]);
    }
    let started = Instant::now();
    let client = run
        .client
        .insert(Client::spawn(&client, &root, &arguments, out, show)?);
    let mut timed_out = false;
    while client.alive()? {
        if started.elapsed() > timeout {
            timed_out = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    let seconds = started.elapsed().as_secs_f64();
    for file in CAPTURE_FILES {
        let from = visual.join(file);
        if from.is_file() {
            fs::copy(&from, out.join(file))?;
        }
    }
    let mut report = check(out, npc);
    report["skip_welcome"] = json!(skip_welcome);
    report["client_seconds"] = json!(seconds);
    report["client_timed_out"] = json!(timed_out);
    if timed_out {
        report["pass"] = json!(false);
    }
    Ok(report)
}

fn read(out: &Path, file: &str) -> Value {
    fs::read_to_string(out.join(file))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}

fn stage<'a>(capture: &'a Value, name: &str) -> Option<&'a Value> {
    capture["stages"]
        .as_array()?
        .iter()
        .find(|s| s["stage"] == name)
}

/// Whether a menu's entries have `option` for an NPC named `npc`.
fn lists(entries: &Value, option: &str, npc: &str) -> bool {
    let npc = npc.to_lowercase();
    entries.as_array().is_some_and(|entries| {
        entries.iter().any(|e| {
            e["option"] == option
                && e["target"]
                    .as_str()
                    .is_some_and(|t| t.to_lowercase().contains(&npc))
        })
    })
}

/// The checks (a) to (e) over the copied capture files.
fn check(out: &Path, npc: &str) -> Value {
    let capture = read(out, "npc-menu-capture.json");
    let picks = read(out, "pick-state.json");
    let live = read(out, "live-state.json");
    let loaded = read(out, "loaded.json");
    let mut checks = Vec::new();
    let mut add = |id: &str, what: &str, pass: bool, evidence: Value| {
        checks.push(json!({"id": id, "what": what, "pass": pass, "evidence": evidence}));
    };

    // (a) login reaches the game.
    let entered = stage(&capture, "enter");
    add(
        "a",
        "login reaches the game state (welcome screen passed, world view shown)",
        entered.is_some() && live["in_game"] == true,
        json!({
            "welcome_clicks": capture["welcome_clicks"],
            "in_game": live["in_game"],
            "status": live["status"],
            "enter_stage": entered,
            "failed_at": capture["failed_at"],
            "reason": capture["reason"],
        }),
    );

    // (b) the scene around Lumbridge with NPCs.
    // `self_tile` is relative to the scene's base.
    let tile = match (live["base"].as_array(), live["self_tile"].as_array()) {
        (Some(base), Some(local)) if base.len() >= 2 && local.len() >= 2 => Some(
            (0..2)
                .map(|i| (base[i].as_i64().unwrap_or(0) + local[i].as_i64().unwrap_or(0)) as i32)
                .collect::<Vec<_>>(),
        ),
        _ => None,
    };
    let near = tile.as_ref().is_some_and(|t| {
        t.len() >= 2 && (t[0] - LOCATION[0]).abs() <= 20 && (t[1] - LOCATION[1]).abs() <= 20
    });
    let npc_rows = picks["npcs"].as_array().cloned().unwrap_or_default();
    let named = npc_rows
        .iter()
        .filter(|row| {
            row["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(npc))
        })
        .count();
    let meshes = loaded["placed_meshes"].as_u64().unwrap_or(0);
    add(
        "b",
        "the scene around Lumbridge loaded, with the NPCs in the frame's list",
        near && meshes > 0 && named > 0,
        json!({
            "self_tile": tile,
            "seeded_at": LOCATION,
            "placed_meshes": meshes,
            "npcs_in_frame_list": npc_rows.len(),
            "named_npcs": named,
        }),
    );

    // (c) the pick probe hits one, and the menu built there has Attack.
    let search = stage(&capture, "search");
    let index = search.and_then(|s| s["detail"]["npc_index"].as_u64());
    let row = index.and_then(|i| npc_rows.iter().find(|r| r["index"].as_u64() == Some(i)));
    let hover = stage(&capture, "hover");
    let hover_entries = hover.map_or(Value::Null, |h| h["detail"]["entries"].clone());
    let probed: Vec<_> = npc_rows
        .iter()
        .map(|r| json!([r["index"], r["name"], r["pick_triangles"], r["enters_pick"]]))
        .collect();
    add(
        "c",
        "the pick probe (taken at the frame's mouse pick) hits a goblin, and the menu built with the mouse there has Attack for it",
        row.is_some_and(|r| r["enters_pick"] == true) && lists(&hover_entries, "Attack", npc),
        json!({
            "npc_index": index,
            "point": search.map(|s| s["detail"]["point"].clone()),
            "probe_npcs_index_name_triangles_enters_pick": probed,
            "hover_menu": hover_entries,
            "failed_at": capture["failed_at"],
            "reason": capture["reason"],
        }),
    );

    // (d) the right-click menu opened there lists Attack.
    let open = stage(&capture, "open");
    let open_entries = open.map_or(Value::Null, |o| o["detail"]["entries"].clone());
    add(
        "d",
        "the right-click menu opened at that point lists Attack for it",
        open.is_some() && lists(&open_entries, "Attack", npc),
        json!({"menu": open.map(|o| o["detail"].clone())}),
    );

    // (e) the Attack click sends an NPC option and the server answers.
    let choose = stage(&capture, "choose");
    let sent = choose.map_or(Value::Null, |c| c["detail"]["sent"].clone());
    let sent_npc_option = sent.as_array().is_some_and(|frames| {
        frames.iter().any(|f| {
            f["opcode"]
                .as_u64()
                .is_some_and(|o| NPC_OPTION_OPCODES.contains(&o))
        })
    });
    let after = stage(&capture, "after");
    let npc_hit = after.is_some_and(|a| a["detail"]["npc_hit"] == true);
    add(
        "e",
        "choosing Attack sends an NPC option packet, and the server's hit on that NPC reaches the client",
        sent_npc_option && npc_hit,
        json!({
            "sent": sent,
            "after": after.map(|a| a["detail"].clone()),
        }),
    );

    let pass = checks.iter().all(|c| c["pass"] == true);
    json!({"pass": pass, "checks": checks})
}

/// Add the run's context, write `report.json` and `report.md`, print the
/// summary.
fn finish_report(mut report: Value, world: &Value, out: &Path, npc: &str) -> Result<Value> {
    for (key, file) in [("log", "server.log"), ("errorLog", "server.err.log")] {
        if let Some(from) = world[key].as_str() {
            let _ = fs::copy(from, out.join(file));
        }
    }
    report["npc"] = json!(npc);
    report["world"] = world.clone();
    fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    let mut md = format!(
        "# Client smoke test: {}\n\nWorld `{}` at `{}`; NPC `{npc}`; output `{}`.\n\n",
        if report["pass"] == true {
            "PASS"
        } else {
            "FAIL"
        },
        world["name"].as_str().unwrap_or("?"),
        world["address"].as_str().unwrap_or("?"),
        out.display()
    );
    if let Some(error) = report["error"].as_str() {
        md.push_str(&format!("Error: {error}\n\n"));
    }
    if report["client_timed_out"] == true {
        md.push_str("The client did not finish within the time limit and was stopped.\n\n");
    }
    md.push_str("| check | result | what |\n| --- | --- | --- |\n");
    for c in report["checks"].as_array().into_iter().flatten() {
        md.push_str(&format!(
            "| {} | {} | {} |\n",
            c["id"].as_str().unwrap_or("?"),
            if c["pass"] == true { "pass" } else { "FAIL" },
            c["what"].as_str().unwrap_or("")
        ));
    }
    md.push_str(
        "\nEvidence for each check is in report.json; the client's capture files are beside it.\n",
    );
    fs::write(out.join("report.md"), &md)?;
    println!("{md}");
    Ok(report)
}

/// The client process this command started, held by its handle so that
/// stopping it can never reach another process that took its id.
#[cfg(windows)]
struct Client(windows::Process);

#[cfg(windows)]
impl Client {
    fn spawn(
        program: &Path,
        dir: &Path,
        arguments: &[String],
        out: &Path,
        show: bool,
    ) -> Result<Self> {
        Ok(Self(windows::spawn(program, dir, arguments, out, show)?))
    }
    fn alive(&mut self) -> Result<bool> {
        Ok(self.0.alive())
    }
    fn stop(&mut self) -> Result<()> {
        self.0.stop()
    }
}

#[cfg(not(windows))]
struct Client(std::process::Child);

#[cfg(not(windows))]
impl Client {
    fn spawn(
        program: &Path,
        dir: &Path,
        arguments: &[String],
        out: &Path,
        _show: bool,
    ) -> Result<Self> {
        Ok(Self(
            Command::new(program)
                .current_dir(dir)
                .args(arguments)
                .env("RS09_CLIENT_PASSWORD", "test")
                .stdin(Stdio::null())
                .stdout(fs::File::create(out.join("client.log"))?)
                .stderr(fs::File::create(out.join("client.err.log"))?)
                .spawn()?,
        ))
    }
    fn alive(&mut self) -> Result<bool> {
        Ok(self.0.try_wait()?.is_none())
    }
    fn stop(&mut self) -> Result<()> {
        if self.alive()? {
            self.0.kill()?;
            self.0.wait()?;
        }
        Ok(())
    }
}

/// The client is started with CreateProcess so that its window can start
/// minimized and inactive: the first `ShowWindow` of a process takes
/// `STARTUPINFO.wShowWindow` in place of its own argument, which std's
/// `Command` cannot set.
#[cfg(windows)]
mod windows {
    use crate::Result;
    use std::{ffi::c_void, fs, os::windows::io::AsRawHandle, path::Path};

    const STARTF_USESHOWWINDOW: u32 = 0x1;
    const STARTF_USESTDHANDLES: u32 = 0x100;
    const SW_SHOWMINNOACTIVE: u16 = 7;
    const SW_SHOWNOACTIVATE: u16 = 4;
    const HANDLE_FLAG_INHERIT: u32 = 1;
    const CREATE_UNICODE_ENVIRONMENT: u32 = 0x400;
    const STILL_ACTIVE: u32 = 259;

    #[repr(C)]
    struct StartupInfo {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        x_size: u32,
        y_size: u32,
        x_count_chars: u32,
        y_count_chars: u32,
        fill_attribute: u32,
        flags: u32,
        show_window: u16,
        reserved2_size: u16,
        reserved2: *mut u8,
        std_input: *mut c_void,
        std_output: *mut c_void,
        std_error: *mut c_void,
    }

    #[repr(C)]
    struct ProcessInformation {
        process: *mut c_void,
        thread: *mut c_void,
        process_id: u32,
        thread_id: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateProcessW(
            application: *const u16,
            command_line: *mut u16,
            process_attributes: *mut c_void,
            thread_attributes: *mut c_void,
            inherit_handles: i32,
            creation_flags: u32,
            environment: *mut c_void,
            current_directory: *const u16,
            startup_info: *mut StartupInfo,
            process_information: *mut ProcessInformation,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn SetHandleInformation(handle: *mut c_void, mask: u32, flags: u32) -> i32;
        fn GetExitCodeProcess(process: *mut c_void, code: *mut u32) -> i32;
        fn TerminateProcess(process: *mut c_void, code: u32) -> i32;
        fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    }

    /// A process handle from CreateProcess, closed on drop.
    pub struct Process {
        handle: *mut c_void,
    }

    impl Process {
        pub fn alive(&self) -> bool {
            let mut code = 0;
            // SAFETY: our open process handle and a local out-value.
            let ok = unsafe { GetExitCodeProcess(self.handle, &mut code) };
            ok != 0 && code == STILL_ACTIVE
        }
        pub fn stop(&mut self) -> Result<()> {
            if !self.alive() {
                return Ok(());
            }
            // SAFETY: our open process handle.
            unsafe {
                if TerminateProcess(self.handle, 1) == 0 && self.alive() {
                    return Err(format!(
                        "could not stop the client: {}",
                        std::io::Error::last_os_error()
                    )
                    .into());
                }
                WaitForSingleObject(self.handle, 10_000);
            }
            Ok(())
        }
    }

    impl Drop for Process {
        fn drop(&mut self) {
            // SAFETY: closed once, here.
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// One argument as `CommandLineToArgvW` reads it back.
    fn quote(argument: &str) -> String {
        if !argument.is_empty() && !argument.contains([' ', '\t', '"']) {
            return argument.to_owned();
        }
        let mut quoted = String::from('"');
        let mut backslashes = 0;
        for c in argument.chars() {
            match c {
                '\\' => backslashes += 1,
                '"' => {
                    quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                    quoted.push('"');
                    backslashes = 0;
                }
                c => {
                    quoted.push_str(&"\\".repeat(backslashes));
                    quoted.push(c);
                    backslashes = 0;
                }
            }
        }
        quoted.push_str(&"\\".repeat(backslashes * 2));
        quoted.push('"');
        quoted
    }

    /// This process's environment with the client's password set, as a
    /// CreateProcess block. The test world keeps accounts in memory: any
    /// password logs in.
    fn environment() -> Vec<u16> {
        let mut block = Vec::new();
        for (key, value) in std::env::vars_os() {
            if key.eq_ignore_ascii_case("RS09_CLIENT_PASSWORD") {
                continue;
            }
            let entry = format!("{}={}", key.to_string_lossy(), value.to_string_lossy());
            block.extend(entry.encode_utf16());
            block.push(0);
        }
        block.extend("RS09_CLIENT_PASSWORD=test".encode_utf16());
        block.push(0);
        block.push(0);
        block
    }

    pub fn spawn(
        program: &Path,
        dir: &Path,
        arguments: &[String],
        out: &Path,
        show: bool,
    ) -> Result<Process> {
        let stdout = fs::File::create(out.join("client.log"))?;
        let stderr = fs::File::create(out.join("client.err.log"))?;
        let stdin = fs::File::open("NUL")?;
        let handles = [
            stdin.as_raw_handle(),
            stdout.as_raw_handle(),
            stderr.as_raw_handle(),
        ];
        for handle in handles {
            // SAFETY: handles of files this function holds open.
            unsafe {
                SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
            }
        }
        let mut command_line = std::iter::once(program.display().to_string())
            .chain(arguments.iter().cloned())
            .map(|a| quote(&a))
            .collect::<Vec<_>>()
            .join(" ");
        command_line.push('\0');
        let mut command_line: Vec<u16> = command_line.encode_utf16().collect();
        let program = wide(&program.display().to_string());
        let dir = wide(&dir.display().to_string());
        let mut environment = environment();
        let mut startup = StartupInfo {
            cb: std::mem::size_of::<StartupInfo>() as u32,
            reserved: std::ptr::null_mut(),
            desktop: std::ptr::null_mut(),
            title: std::ptr::null_mut(),
            x: 0,
            y: 0,
            x_size: 0,
            y_size: 0,
            x_count_chars: 0,
            y_count_chars: 0,
            fill_attribute: 0,
            flags: STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES,
            show_window: if show {
                SW_SHOWNOACTIVATE
            } else {
                SW_SHOWMINNOACTIVE
            },
            reserved2_size: 0,
            reserved2: std::ptr::null_mut(),
            std_input: handles[0],
            std_output: handles[1],
            std_error: handles[2],
        };
        let mut information = ProcessInformation {
            process: std::ptr::null_mut(),
            thread: std::ptr::null_mut(),
            process_id: 0,
            thread_id: 0,
        };
        // SAFETY: every pointer is to a live, NUL-terminated buffer or to a
        // struct of the documented layout; the out-struct is ours.
        let created = unsafe {
            CreateProcessW(
                program.as_ptr(),
                command_line.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                1,
                CREATE_UNICODE_ENVIRONMENT,
                environment.as_mut_ptr().cast(),
                dir.as_ptr(),
                &mut startup,
                &mut information,
            )
        };
        if created == 0 {
            return Err(format!(
                "could not start the client: {}",
                std::io::Error::last_os_error()
            )
            .into());
        }
        // SAFETY: the thread handle CreateProcess returned; the process
        // handle is kept.
        unsafe {
            CloseHandle(information.thread);
        }
        Ok(Process {
            handle: information.process,
        })
    }
}
