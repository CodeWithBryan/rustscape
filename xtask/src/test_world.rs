//! `cargo xtask test-world`: a disposable `rs09-server --world` for tests run
//! without a person present (docs/test-world.md).
//!
//! `start` picks a free loopback port, makes `runs/test-world/<name>/` with an
//! empty saves folder, starts the release server there with its output in
//! `server.log`, waits for its "listening" line and writes `world.json`
//! (address, pid, saves, data, log). `stop` kills only a server this command
//! started: the pid in `world.json`, and only while that process is still
//! `rs09-server` with this world's saves folder on its command line. Port 43594
//! is never chosen or accepted.
use crate::Result;
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const DEFAULT_PORT: u16 = 43594;
const LISTENING: &str = "Native world listening on TCP ";
const USAGE: &str = "cargo xtask test-world <start [--name N] [--port P] [--data DIR] [--cache DIR] [--no-build] [--timeout SECS] | stop --name N [--clean] | stop --all [--clean] | list>";

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    match args.get(1).map(String::as_str) {
        Some("start") => start(root, &args[2..]),
        Some("stop") => stop(root, &args[2..]),
        Some("list") => list(root),
        _ => Err(USAGE.into()),
    }
}

fn worlds(root: &Path) -> PathBuf {
    root.join("runs/test-world")
}

fn absolute(root: &Path, path: impl Into<PathBuf>) -> PathBuf {
    let path = path.into();
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn valid_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("world name {name:?}: use letters, digits, - and _").into());
    }
    Ok(())
}

fn start(root: &Path, args: &[String]) -> Result<()> {
    let mut name = None;
    let mut port = None;
    let mut data = None;
    let mut cache = root.join("assets/cache");
    let mut build = true;
    let mut timeout = Duration::from_secs(300);
    let mut options = args.iter();
    while let Some(option) = options.next() {
        let mut value = || options.next().ok_or(format!("missing value for {option}"));
        match option.as_str() {
            "--name" => name = Some(value()?.clone()),
            "--port" => port = Some(value()?.parse::<u16>()?),
            "--data" => data = Some(absolute(root, value()?)),
            "--cache" => cache = absolute(root, value()?),
            "--no-build" => build = false,
            "--timeout" => timeout = Duration::from_secs(value()?.parse()?),
            other => return Err(format!("unknown test-world start option {other}\n{USAGE}").into()),
        }
    }
    let name = name.unwrap_or_else(|| {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        format!("world-{secs}")
    });
    valid_name(&name)?;
    if port == Some(DEFAULT_PORT) {
        return Err(
            format!("port {DEFAULT_PORT} is reserved for play; test worlds never use it").into(),
        );
    }
    let dir = worlds(root).join(&name);
    if let Some(world) = read_world(&dir)?
        && alive(&world)?
    {
        return Err(format!(
            "test world {name} is running (pid {}); stop it first",
            world["pid"]
        )
        .into());
    }
    if !cache.join("main_file_cache.dat2").is_file() {
        return Err(format!("no cache in {} (cargo xtask cache-fetch)", cache.display()).into());
    }
    let data = match data {
        Some(data) => data,
        None => {
            // The world folder of `cargo xtask play-world` (kept when it exists).
            crate::play_world(root, &["play-world".to_owned()]).map_err(|e| {
                format!(
                    "cargo xtask play-world could not write runs/play/world-data ({e}); run cargo xtask bootstrap first, or pass --data WORLD_DIR"
                )
            })?;
            root.join("runs/play/world-data")
        }
    };
    if !data.join("worldprops/default.conf").is_file() {
        return Err(format!("{} has no worldprops/default.conf", data.display()).into());
    }

    if dir.exists() {
        // Only a folder this command made (it holds world.json) is replaced.
        if !dir.join("world.json").is_file() && fs::read_dir(&dir)?.next().is_some() {
            return Err(format!("{} exists and is not a test world", dir.display()).into());
        }
        fs::remove_dir_all(&dir)?;
    }
    let saves = dir.join("saves");
    fs::create_dir_all(&saves)?;

    if build {
        let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .current_dir(root)
            .args(["build", "--release", "-p", "rs09-server"])
            .status()?;
        if !status.success() {
            return Err(format!("cargo build --release -p rs09-server failed ({status})").into());
        }
    }
    let server = root.join(format!(
        "target/release/rs09-server{}",
        std::env::consts::EXE_SUFFIX
    ));
    if !server.is_file() {
        return Err(format!("missing {}; run without --no-build", server.display()).into());
    }

    let port = match port {
        Some(port) => port,
        None => free_port()?,
    };
    let address = format!("127.0.0.1:{port}");
    let log = dir.join("server.log");
    let error_log = dir.join("server.err.log");
    let mut command = Command::new(&server);
    command
        .current_dir(root)
        .arg("--world")
        .arg("--cache")
        .arg(&cache)
        .arg("--saves")
        .arg(&saves)
        .arg("--data")
        .arg(&data)
        .args(["--tcp", &address, "--verbose"])
        .stdin(Stdio::null())
        .stdout(fs::File::create(&log)?)
        .stderr(fs::File::create(&error_log)?);
    let mut child = spawn_detached(&mut command)?;
    let pid = child.id();
    let world = json!({
        "name": name,
        "address": address,
        "pid": pid,
        "dir": dir,
        "saves": saves,
        "data": data,
        "cache": cache,
        "log": log,
        "errorLog": error_log,
    });
    fs::write(
        dir.join("world.json"),
        serde_json::to_string_pretty(&world)?,
    )?;

    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!(
                "the server exited ({status}) before listening; see {} and {}",
                log.display(),
                error_log.display()
            )
            .into());
        }
        let mut text = String::new();
        fs::File::open(&log)?.read_to_string(&mut text)?;
        if let Some(line) = text.lines().find(|line| line.contains(LISTENING)) {
            if !line.trim_end().ends_with(&address) {
                kill(pid)?;
                return Err(format!("the server listens elsewhere: {line}").into());
            }
            break;
        }
        if Instant::now() > deadline {
            kill(pid)?;
            return Err(format!(
                "the server did not listen within {}s; see {}",
                timeout.as_secs(),
                log.display()
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    eprintln!(
        "Test world {name} is up on {address} (pid {pid}; log {}).\nStop it with: cargo xtask test-world stop --name {name}",
        log.display()
    );
    println!("{}", serde_json::to_string_pretty(&world)?);
    Ok(())
}

fn free_port() -> Result<u16> {
    for _ in 0..16 {
        let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        if port != DEFAULT_PORT {
            return Ok(port);
        }
    }
    Err("no free loopback port".into())
}

#[cfg(windows)]
fn spawn_detached(command: &mut Command) -> Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    const HANDLE_FLAG_INHERIT: u32 = 1;
    unsafe extern "system" {
        fn SetHandleInformation(handle: *mut std::ffi::c_void, mask: u32, flags: u32) -> i32;
    }
    // CreateProcess hands the child every inheritable handle this process
    // holds, and `cargo xtask` gives us inheritable copies of the caller's
    // stdout pipe beyond our standard handles: a caller reading that pipe to
    // its end would then wait for the server to exit. Clear the flag on every
    // handle we hold; std makes the child's own stdio handles inheritable
    // during the spawn.
    for value in (4..0x1_0000usize).step_by(4) {
        // SAFETY: a plain Win32 call on a handle value of this process; a
        // value that is not an open handle only makes it fail.
        unsafe {
            SetHandleInformation(value as *mut std::ffi::c_void, HANDLE_FLAG_INHERIT, 0);
        }
    }
    // Outlive this command (and a job object a caller's shell may close).
    let flags = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
    match command
        .creation_flags(flags | CREATE_BREAKAWAY_FROM_JOB)
        .spawn()
    {
        Ok(child) => Ok(child),
        // The job does not allow breakaway.
        Err(_) => Ok(command.creation_flags(flags).spawn()?),
    }
}

#[cfg(not(windows))]
fn spawn_detached(command: &mut Command) -> Result<std::process::Child> {
    use std::os::unix::process::CommandExt;
    Ok(command.process_group(0).spawn()?)
}

fn read_world(dir: &Path) -> Result<Option<Value>> {
    match fs::read_to_string(dir.join("world.json")) {
        Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The command line of a live process, or None when there is none.
fn command_line(pid: u32) -> Result<Option<String>> {
    #[cfg(windows)]
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("(Get-CimInstance Win32_Process -Filter 'ProcessId={pid}').CommandLine"),
        ])
        .output()?;
    #[cfg(not(windows))]
    let output = Command::new("ps")
        .args(["-o", "args=", "-p", &pid.to_string()])
        .output()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok((!text.is_empty()).then_some(text))
}

/// Whether `world.json`'s pid is still this world's server: an `rs09-server`
/// whose command line names this world's saves folder.
fn alive(world: &Value) -> Result<bool> {
    let (Some(pid), Some(saves)) = (world["pid"].as_u64(), world["saves"].as_str()) else {
        return Ok(false);
    };
    let Some(line) = command_line(pid as u32)? else {
        return Ok(false);
    };
    let normalize = |s: &str| s.replace('\\', "/").to_lowercase();
    let line = normalize(&line);
    Ok(line.contains("rs09-server") && line.contains(&normalize(saves)))
}

fn kill(pid: u32) -> Result<()> {
    #[cfg(windows)]
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .stdout(Stdio::null())
        .status()?;
    #[cfg(not(windows))]
    let status = Command::new("kill").arg(pid.to_string()).status()?;
    if !status.success() {
        return Err(format!("could not stop pid {pid} ({status})").into());
    }
    Ok(())
}

fn stop(root: &Path, args: &[String]) -> Result<()> {
    let mut names = Vec::new();
    let mut all = false;
    let mut clean = false;
    let mut options = args.iter();
    while let Some(option) = options.next() {
        match option.as_str() {
            "--name" => names.push(options.next().ok_or("missing --name N")?.clone()),
            "--all" => all = true,
            "--clean" => clean = true,
            other => return Err(format!("unknown test-world stop option {other}\n{USAGE}").into()),
        }
    }
    if all {
        if let Ok(entries) = fs::read_dir(worlds(root)) {
            for entry in entries.flatten() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    } else if names.is_empty() {
        return Err(USAGE.into());
    }
    for name in names {
        valid_name(&name)?;
        let dir = worlds(root).join(&name);
        let Some(world) = read_world(&dir)? else {
            eprintln!("{name}: not a test world (no world.json); left alone");
            continue;
        };
        if alive(&world)? {
            let pid = world["pid"].as_u64().unwrap() as u32;
            kill(pid)?;
            let end = Instant::now() + Duration::from_secs(10);
            while alive(&world)? && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(200));
            }
            println!("{name}: stopped pid {pid}");
        } else {
            println!("{name}: not running");
        }
        if clean {
            fs::remove_dir_all(&dir)?;
            println!("{name}: removed {}", dir.display());
        }
    }
    Ok(())
}

fn list(root: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(worlds(root)) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if let Some(world) = read_world(&entry.path())? {
            let state = if alive(&world)? { "running" } else { "stopped" };
            println!(
                "{} {} {state} pid {}",
                world["name"].as_str().unwrap_or("?"),
                world["address"].as_str().unwrap_or("?"),
                world["pid"]
            );
        }
    }
    Ok(())
}
