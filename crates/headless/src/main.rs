//! Native cache probes and structured live player-state observations.
use rs09_headless::{ground, world};
use rs09_net::peer::CachePeer;
use rs09_protocol::js5::Request;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time::timeout;

#[tokio::main(worker_threads = 4)]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut address: SocketAddr = "127.0.0.1:43594".parse()?;
    let mut clients = 1usize;
    let mut rounds = 10usize;
    let mut websocket = false;
    let mut observe_world = false;
    let mut npc_stream = false;
    let mut ground_pack = None;
    let mut walk_destination = None;
    let mut walk_running = false;
    let mut walk_after = 1usize;
    let mut scenery = Vec::<world::ScheduledScenery>::new();
    let mut username = String::from("agent");
    let mut password = String::from("testing");
    let mut request = Request {
        archive: 255,
        group: 255,
        urgent: true,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--address" => {
                i += 1;
                address = args.get(i).ok_or("missing address")?.parse()?;
            }
            "--clients" => {
                i += 1;
                clients = args.get(i).ok_or("missing client count")?.parse()?;
            }
            "--rounds" => {
                i += 1;
                rounds = args.get(i).ok_or("missing round count")?.parse()?;
            }
            "--websocket" => websocket = true,
            "--world" => observe_world = true,
            "--npcs" => {
                npc_stream = true;
                observe_world = true;
            }
            "--ground-pack" => {
                i += 1;
                ground_pack = Some(PathBuf::from(
                    args.get(i).ok_or("missing ground item pack path")?,
                ));
                observe_world = true;
            }
            "--walk" => {
                i += 1;
                let coordinates: Vec<i32> = args
                    .get(i)
                    .ok_or("missing walk X,Y")?
                    .split(',')
                    .map(str::parse)
                    .collect::<Result<_, _>>()?;
                let [x, y]: [i32; 2] = coordinates.try_into().map_err(|_| "walk needs X,Y")?;
                i16::try_from(x).map_err(|_| "walk X exceeds original signed-short field")?;
                u16::try_from(y).map_err(|_| "walk Y exceeds original unsigned-short field")?;
                walk_destination = Some([x, y]);
                observe_world = true;
            }
            "--run" => walk_running = true,
            "--scenery" => {
                i += 1;
                let fields = args
                    .get(i)
                    .ok_or("missing scenery ID,X,Y,OPTION")?
                    .split(',')
                    .map(str::parse::<i32>)
                    .collect::<Result<Vec<_>, _>>()?;
                let [id, x, y, option]: [i32; 4] = fields
                    .try_into()
                    .map_err(|_| "scenery needs ID,X,Y,OPTION")?;
                let option = u8::try_from(option)
                    .ok()
                    .filter(|o| *o < 5)
                    .ok_or("scenery option must be0..4")?;
                scenery.push(world::ScheduledScenery {
                    action: rs09_headless::session::SceneryAction {
                        id,
                        tile: [x, y],
                        option,
                    },
                    after_update: 1,
                });
                observe_world = true;
            }
            "--scenery-after" => {
                i += 1;
                let after = args.get(i).ok_or("missing scenery update count")?.parse()?;
                scenery
                    .last_mut()
                    .ok_or("--scenery-after requires preceding --scenery")?
                    .after_update = after;
            }
            "--walk-after" => {
                i += 1;
                walk_after = args.get(i).ok_or("missing walk update count")?.parse()?;
            }
            "--username" => {
                i += 1;
                username = args.get(i).ok_or("missing username")?.clone();
            }
            "--password" => {
                i += 1;
                password = args.get(i).ok_or("missing password")?.clone();
            }
            "--archive" => {
                i += 1;
                request.archive = args.get(i).ok_or("missing archive")?.parse()?;
            }
            "--group" => {
                i += 1;
                request.group = args.get(i).ok_or("missing group")?.parse()?;
            }
            "--help" => {
                println!(
                    "--scenery ID,X,Y,OPTION [--scenery-after N] sends an original scenery option0..4 after N player updates (default1). Repeat for a sequence. JSON scenery observations report received object/chunk/sound events; requests are not success acknowledgements."
                );
                println!(
                    "rs09-headless [--address IP:PORT] [--clients N] [--rounds N] [--websocket] [--archive ID --group ID]\n--world --username NAME --password VALUE observes live scene/player updates. --walk X,Y [--run] [--walk-after N] sends one original absolute walk after N player updates (default 1). --npcs includes NPC observations and waits for both NPC and player updates. --ground-pack PATH loads the same prepared item metadata as the visual client and includes logical ground item observations, with no graphics engine. Complete gameplay actions remain pending."
                );
                return Ok(());
            }
            argument => return Err(format!("unknown argument {argument}").into()),
        }
        i += 1;
    }
    if clients == 0 || clients > 10_000 || rounds == 0 || rounds > 100_000 {
        return Err("probe bounds exceeded".into());
    }
    if walk_running && walk_destination.is_none() {
        return Err("--run requires --walk X,Y".into());
    }
    if walk_destination.is_some() && (walk_after == 0 || walk_after >= rounds) {
        return Err(
            "walk update count must be positive and leave observation updates after the request"
                .into(),
        );
    }
    let walk = walk_destination.map(|destination| world::Walk {
        destination,
        running: walk_running,
        after_update: walk_after,
    });
    if scenery
        .iter()
        .any(|s| s.after_update == 0 || s.after_update >= rounds)
    {
        return Err(
            "scenery update count must be positive and leave observation updates after the request"
                .into(),
        );
    }
    if observe_world {
        let definitions = ground_pack
            .as_deref()
            .map(ground::Definitions::load)
            .transpose()?
            .map(Arc::new);
        let mut tasks = tokio::task::JoinSet::new();
        for index in 0..clients {
            let name = if clients == 1 {
                username.clone()
            } else {
                format!("{username}{index}")
            };
            if name.len() > 12 {
                return Err(
                    "world probe names must fit the original 12-byte login encoding".into(),
                );
            }
            let password = password.clone();
            let options = world::Options {
                updates: rounds,
                walk,
                npc_stream,
                ground: definitions.clone(),
                scenery: scenery.clone(),
            };
            tasks.spawn(async move {
                world::observe(address, websocket, name, password, options).await
            });
        }
        let mut observations = vec![];
        while let Some(result) = tasks.join_next().await {
            observations.push(result??);
        }
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"scope":"headless live player-state observers, not gameplay capacity","clients":observations})
            )?
        );
        return Ok(());
    }
    let started = Instant::now();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..clients {
        tasks.spawn(async move {
            timeout(Duration::from_secs(60), async move {
                let mut peer = CachePeer::connect(address, websocket, 0, 1_000_000).await?;
                let mut hash = Sha256::new();
                let mut bytes = 0usize;
                for _ in 0..rounds {
                    let data = peer.fetch(request).await?;
                    hash.update(&data);
                    bytes += data.len();
                }
                peer.close().await?;
                Ok::<_, std::io::Error>((bytes, format!("{:x}", hash.finalize())))
            })
            .await
            .map_err(std::io::Error::other)?
        });
    }
    let mut bytes = 0usize;
    let mut expected = None;
    let mut completed = 0;
    while let Some(result) = tasks.join_next().await {
        let (count, checksum) = result??;
        if let Some(expected) = &expected {
            if expected != &checksum {
                return Err("clients received differing bytes".into());
            }
        } else {
            expected = Some(checksum);
        }
        bytes += count;
        completed += 1;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"scope":"cache transport, not active game players",
        "clients":completed,"requests":clients*rounds,"bytes":bytes,"seconds":started.elapsed().as_secs_f64(),
        "checksum":expected,"websocket":websocket,"archive":request.archive,"group":request.group})
        )?
    );
    Ok(())
}
