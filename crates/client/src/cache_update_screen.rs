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
//! macroquad draws by default, not the original's Helvetica. The original
//! lets the player in while the rest downloads in the background; this
//! client reads a finished store, so the bar stays until everything is in,
//! showing verified groups out of listed groups for that part.
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
    let options = Options {
        complete: !args.iter().any(|a| a == "--update-original-archives"),
        ..Options::default()
    };
    eprintln!(
        "Updating the cache in {} from {}:{}",
        request.directory.display(),
        request.host,
        request.port
    );
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
        draw(&snapshot, None);
        next_frame().await;
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
