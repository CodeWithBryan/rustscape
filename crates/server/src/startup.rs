//! Connect parsed startup settings to the native endpoint; gameplay loading is pending.
//! The original start-up order (`GameWorld.prompt`) is [`sequence`]; see docs/startup.md.
pub mod generators;
pub mod loaders;
pub mod regions;
pub mod sequence;
use crate::{
    auth::{factory::Registry, password::Passwords},
    config::Configuration,
    runtime::Config,
};
use std::{error::Error, path::PathBuf, sync::Arc};
pub struct Configured {
    pub cache: PathBuf,
    pub tcp_port: u16,
    pub websocket_port: Option<u16>,
    pub websocket_tls: bool,
    pub runtime: Config,
}
pub fn configure(settings: Configuration) -> Result<Configured, Box<dyn Error>> {
    let (tcp, websocket) = settings
        .listener_ports()
        .ok_or("configuration has no world settings")?;
    let tcp_port = u16::try_from(tcp).map_err(|_| "configured TCP port is outside 0..65535")?;
    let websocket_port = websocket
        .map(u16::try_from)
        .transpose()
        .map_err(|_| "configured WebSocket port is outside 0..65535")?;
    let cache = PathBuf::from(
        settings.constants()["CACHE_PATH"]
            .as_str()
            .ok_or("configuration has no cache path")?,
    );
    let websocket_tls = settings.constants()["WEBSOCKET_TLS_ENABLED"]
        .as_bool()
        .unwrap();
    let mut registry = Registry::default();
    // There are no admitted world characters yet. World-owner lookup replaces
    // this empty callback when that boundary is implemented.
    registry.configure(
        &settings.auth_settings(),
        Arc::new(Passwords::default()),
        |_| false,
    )?;
    let runtime = Config {
        authenticator: registry.authenticator().cloned(),
        startup: Some(Arc::new(settings)),
        ..Default::default()
    };
    Ok(Configured {
        cache,
        tcp_port,
        websocket_port,
        websocket_tls,
        runtime,
    })
}
