//! The start-up hook for [`rs09_client::cache_update`]: fetches or repairs
//! the cache from the game server before anything is loaded from it, behind
//! the original loading bar.
//!
//! It runs only when a server is named and either `--update-cache` is given
//! or the cache directory holds no cache (`cache_update::requested`).
//! `--update-original-archives` keeps to what the original downloads on its
//! own (archives 6 and 14 are then only verified).
//!
//! The bar has `LoadingBarAwt`'s geometry and colour; the font is whatever
//! macroquad draws by default, not the original's Helvetica. As the original
//! lets the player in while the rest downloads in the background, the bar
//! goes once the start-up sequence has finished and the archives this
//! client still reads whole are in (`cache_update::start_archives`); maps,
//! models and animation frames (`DEFERRED_AT_START`) keep arriving behind
//! the title, read through `rs09_client::cache_live`, and a group asked for
//! before it has arrived is requested at once. `--update-whole-cache` keeps
//! the bar until everything is in, showing verified out of listed groups.
use macroquad::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rs09_client::cache_update::{self, Background, Options};
use rs09_client::cache_update::{PRIMARY_TEXT, Snapshot};

fn centred(text: &str, baseline: f32) {
    let size = measure_text(text, None, 16, 1.0);
    draw_text(
        text,
        (screen_width() / 2.0 - size.width / 2.0).floor(),
        baseline,
        16.0,
        WHITE,
    );
}

/// The bar, with the start-up sequence's own percentage and text until the
/// login screen would open and the share of verified groups after that. The
/// browser build draws the same bar while its cache arrives (`cache_host`).
pub fn draw(snapshot: &Snapshot, note: Option<&str>) {
    let (percentage, text) = if snapshot.loaded && snapshot.groups > 0 {
        let share = snapshot.groups_verified * 100 / snapshot.groups;
        (
            share as i32,
            format!(
                "Downloading cache - {share}% ({} of {} groups)",
                snapshot.groups_verified, snapshot.groups
            ),
        )
    } else {
        (snapshot.percentage, snapshot.text.clone())
    };
    let red = Color::from_rgba(140, 17, 17, 255);
    let x = (screen_width() / 2.0).floor() - 152.0;
    let y = (screen_height() / 2.0).floor() - 18.0;
    let filled = percentage.clamp(0, 100) as f32 * 3.0;
    clear_background(BLACK);
    draw_rectangle_lines(x, y, 304.0, 34.0, 2.0, red);
    draw_rectangle(x + 2.0, y + 2.0, filled, 30.0, red);
    centred(PRIMARY_TEXT, y - 8.0);
    centred(&text, y + 22.0);
    if let Some(note) = note {
        centred(note, y + 60.0);
    }
}

/// Runs `work` again while it reads groups the update has not delivered yet
/// (each one is asked for as it is met), for at most two minutes: for start-up
/// work that cannot answer "not yet" itself, such as building the starting
/// scene from the cache. Without an update running it runs once.
#[cfg(not(target_arch = "wasm32"))]
pub fn when_present<T>(mut work: impl FnMut() -> T) -> T {
    let started = std::time::Instant::now();
    loop {
        let before = rs09_cache::store::misses_on_this_thread();
        let result = work();
        if rs09_cache::store::misses_on_this_thread() == before
            || started.elapsed() > std::time::Duration::from_secs(120)
        {
            return result;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Runs the update if the command line asks for one. Returns when it has
/// finished or failed; a failure is shown until a key is pressed (not when
/// capturing) and the client then carries on with whatever is on disk.
#[cfg(not(target_arch = "wasm32"))]
pub async fn run_if_requested() {
    let args: Vec<String> = std::env::args().collect();
    let request = match cache_update::requested(&args) {
        Ok(Some(request)) => request,
        Ok(None) => return,
        Err(error) => {
            eprintln!("Cache update not started: {error}");
            return;
        }
    };
    // `--update-whole-cache`: wait for every archive before going on, as
    // this client did before it could start with archives still arriving.
    let whole = args.iter().any(|a| a == "--update-whole-cache");
    // `--update-original-pace`: one network pass per 20 ms cycle, as the
    // original reads, instead of reading whatever has arrived in between.
    let options = Options {
        complete: !args.iter().any(|a| a == "--update-original-archives"),
        fast: !args.iter().any(|a| a == "--update-original-pace"),
        start_archives: cache_update::start_archives(),
        ..Options::default()
    };
    eprintln!(
        "Updating the cache in {} from {}:{}",
        request.directory.display(),
        request.host,
        request.port
    );
    let directory = request.directory.clone();
    let background = match Background::start(request.host, request.port, request.directory, options)
    {
        Ok(background) => background,
        Err(error) => {
            eprintln!("Cache update not started: {error}");
            return;
        }
    };
    let mut snapshot = background.snapshot();
    while !background.finished() {
        snapshot = background.snapshot();
        if snapshot.start_ready && !whole {
            break;
        }
        draw(&snapshot, None);
        next_frame().await;
    }
    if !background.finished() {
        // The original's login point: the rest arrives in the background,
        // and from here on the cache directory is read as it fills.
        eprintln!(
            "Cache start gate open at {:.2} s: {} of {} groups verified, {} bytes received; \
             archives {:?} continue in the background",
            get_time(),
            snapshot.groups_verified,
            snapshot.groups,
            snapshot.bytes_received,
            cache_update::DEFERRED_AT_START
        );
        rs09_client::cache_live::install(&directory, background.store());
        std::thread::spawn(move || match background.join() {
            Ok(report) => eprintln!(
                "Cache update finished in the background in {:.1}s: {} of {} groups verified, {} bytes received",
                report.elapsed.as_secs_f64(),
                report.groups_verified,
                report.groups,
                report.bytes_received
            ),
            Err(error) => eprintln!("Cache update stopped in the background: {error}"),
        });
        return;
    }
    match background.join() {
        Ok(report) => eprintln!(
            "Cache update finished in {:.1}s: {} of {} groups verified, {} bytes received",
            report.elapsed.as_secs_f64(),
            report.groups_verified,
            report.groups,
            report.bytes_received
        ),
        Err(error) => {
            eprintln!("Cache update failed: {error}");
            let unattended = args.iter().any(|a| a.starts_with("--capture"));
            let message = format!("Error connecting to server: {error}");
            while !unattended && get_last_key_pressed().is_none() {
                draw(&snapshot, Some(&message));
                next_frame().await;
            }
        }
    }
}
