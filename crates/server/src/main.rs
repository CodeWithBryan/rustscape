use rs09_cache::{
    Limits,
    store::{StorageMode, Store},
};
use rs09_server::{
    catalog::Catalog,
    config::Configuration,
    runtime::{Config, Service},
    startup,
};
use std::{
    error::Error,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut cache = PathBuf::from("assets/cache");
    let mut tcp: SocketAddr = "127.0.0.1:43594".parse()?;
    let mut websocket = None;
    let mut settings_file = None::<PathBuf>;
    let (mut cache_given, mut tcp_given, mut websocket_given) = (false, false, false);
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--cache" => {
                i += 1;
                cache = args.get(i).ok_or("missing --cache path")?.into();
                cache_given = true;
            }
            "--tcp" => {
                i += 1;
                tcp = args.get(i).ok_or("missing --tcp address")?.parse()?;
                tcp_given = true;
            }
            "--websocket" => {
                i += 1;
                websocket = Some(args.get(i).ok_or("missing --websocket address")?.parse()?);
                websocket_given = true;
            }
            "--config" => {
                i += 1;
                settings_file = Some(args.get(i).ok_or("missing --config path")?.into());
            }
            "--help" => {
                println!(
                    "rs09-server [--config FILE] [--cache DIR] [--tcp IP:PORT] [--websocket IP:PORT]\nConfiguration enables native account validation. Playable world entry remains in progress."
                );
                return Ok(());
            }
            argument => return Err(format!("unknown argument {argument}").into()),
        }
        i += 1;
    }
    let mut config = Config::default();
    if let Some(file) = settings_file {
        let configured = startup::configure(Configuration::load(&file)?)?;
        if !cache_given {
            cache = configured.cache;
        }
        if !tcp_given {
            tcp.set_port(configured.tcp_port);
        }
        if !websocket_given {
            websocket = configured
                .websocket_port
                .map(|port| SocketAddr::new(tcp.ip(), port));
        }
        if websocket.is_some() && configured.websocket_tls {
            return Err("Configured TLS WebSockets are not implemented".into());
        }
        config = configured.runtime;
    }
    let limits = Limits::default();
    let catalog = Arc::new(Catalog::new(
        Store::open(&cache, StorageMode::Resident, limits)?,
        limits,
        4096,
        64 * 1024 * 1024,
    )?);
    let seed = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos() as i64;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .max_blocking_threads(
            config
                .compute_jobs
                .checked_add(config.auth_jobs)
                .ok_or("worker count overflow")?,
        )
        .thread_name("rs09-io")
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let service = Service::new(catalog, config, seed)?;
        let server = service.start(tcp, websocket).await?;
        println!(
            "Rust 530 cache/lobby transport listening on TCP {}",
            server.tcp
        );
        if let Some(address) = server.websocket {
            println!("Binary WebSocket listener: ws://{address}/");
        }
        println!(
            "Native account validation: {}. Playable world entry remains unavailable.",
            if service.startup_configuration().is_some() {
                "enabled"
            } else {
                "disabled (cache-only mode)"
            }
        );
        tokio::signal::ctrl_c().await?;
        server.shutdown().await?;
        println!(
            "Stopped: {} requests, {} sent bytes.",
            service.metrics.requests.load(Ordering::Relaxed),
            service.metrics.sent_bytes.load(Ordering::Relaxed)
        );
        Ok::<_, std::io::Error>(())
    })?;
    Ok(())
}
