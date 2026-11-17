//! Admission/stream boundary for a native owning world. The world must finish
//! profile/map/player initialization before returning a successful admission.
use crate::player::Prepared;
use rs09_protocol::{game::Frame, login::AuthResponse};
use std::{future::Future, pin::Pin};
use tokio::sync::mpsc;
pub struct Joined {
    pub rights: i32,
    pub index: i32,
    pub incoming: mpsc::Sender<Frame>,
    pub outgoing: mpsc::Receiver<Vec<Frame>>,
}
pub type Admission<'a> =
    Pin<Box<dyn Future<Output = std::result::Result<Joined, AuthResponse>> + Send + 'a>>;
pub trait World: Send + Sync {
    /// Original Login.decode asks this before account/profile initialization.
    fn is_online(&self, _username: &str) -> bool {
        false
    }
    fn enter(&self, player: Prepared) -> Admission<'_>;
    /// `enter` with the connection's remote address (`IoSession.getAddress()`),
    /// which the transport knows and `Prepared` does not carry. Worlds that do
    /// not read the address keep `enter`.
    fn enter_from(&self, player: Prepared, _address: Option<std::net::IpAddr>) -> Admission<'_> {
        self.enter(player)
    }
}
