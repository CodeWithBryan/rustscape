//! Finite real-world liveness proof: keep the connection active while the AI
//! consumer pauses, then continue observing past the server's idle read timeout.
use rs09_headless::session::{Session, Status};
use serde_json::json;
use std::{collections::BTreeMap, io, time::Duration};
use tokio::time::{Instant, sleep, timeout};

#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let option = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|i| args.get(i + 1))
    };
    let address = option("--address")
        .map_or("127.0.0.1:43640", String::as_str)
        .parse()?;
    let websocket = args.iter().any(|arg| arg == "--websocket");
    let username = option("--username").map_or("idleagent", String::as_str);
    let seconds: u64 = option("--seconds").map_or("70", String::as_str).parse()?;
    if !(5..=3600).contains(&seconds) {
        return Err("--seconds must be5..3600".into());
    }
    let password = std::env::var("RS09_CLIENT_PASSWORD").unwrap_or_else(|_| "testing".into());
    let mut session = timeout(
        Duration::from_secs(15),
        Session::connect(address, websocket, username, &password),
    )
    .await??
    .into_managed();
    let started = Instant::now();
    // Deliberately no observer is polling during this thinking interval.
    sleep(Duration::from_secs(2)).await;
    let after_pause = session.stats();
    if after_pause.keepalives_written == 0 || session.status() != Status::Connected {
        return Err("connection did not stay active while consumer paused".into());
    }
    let deadline = started + Duration::from_secs(seconds);
    let mut counts = BTreeMap::<u8, u64>::new();
    loop {
        let frame = match tokio::time::timeout_at(deadline, session.next_frame()).await {
            Ok(frame) => frame?,
            Err(_) => break,
        };
        if frame.opcode == 86 {
            return Err(io::Error::other("server logged out agent").into());
        }
        *counts.entry(frame.opcode).or_default() += 1;
    }
    let stats = session.stats();
    let connected = session.status() == Status::Connected;
    let player_updates = counts.get(&225).copied().unwrap_or(0);
    if !connected
        || player_updates < 2
        || stats.keepalives_written < 2
        || stats.commands_written != 0
    {
        return Err("liveness proof lacked sustained authoritative updates/idle keepalives".into());
    }
    let elapsed = started.elapsed().as_secs_f64();
    session.close().await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope":"finite data-only connection liveness, not player capacity or complete gameplay",
            "address":address.to_string(), "websocket":websocket, "username":username,
            "seconds":elapsed, "consumerPauseSeconds":2, "connectedAtEnd":connected,
            "keepalivesDuringPause":after_pause.keepalives_written,
            "keepalivesWritten":stats.keepalives_written, "commandsWritten":stats.commands_written,
            "framesReceived":stats.frames_received, "playerUpdates":player_updates, "frameCounts":counts
        }))?
    );
    Ok(())
}
