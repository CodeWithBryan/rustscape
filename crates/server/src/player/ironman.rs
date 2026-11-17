//! Original IronmanMode and IronmanManager over one retained player receiver.
use crate::{definitions::item::Shared, world::player_walk::NodeHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Mode {
    None = 0,
    Standard = 1,
    Hardcore = 2,
    Ultimate = 3,
}
impl Mode {
    pub fn icon(self) -> i32 {
        match self {
            Self::None => -1,
            Self::Standard => 5,
            Self::Hardcore => 6,
            Self::Ultimate => 7,
        }
    }
}
struct Fields {
    player: Option<NodeHandle>,
    mode: Option<Mode>,
}
#[derive(Clone)]
pub struct Manager(Shared<Fields>);
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub trait Context {
    fn message(&mut self, player: Option<&NodeHandle>, text: &str) -> Result<()>;
}
impl Manager {
    pub fn new(player: Option<NodeHandle>) -> Self {
        Self(Shared::new(Fields {
            player,
            mode: Some(Mode::None),
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn player(&self) -> Option<NodeHandle> {
        self.0.read().player.clone()
    }
    pub fn mode(&self) -> Option<Mode> {
        self.0.read().mode
    }
    pub fn set_mode(&self, mode: Option<Mode>) {
        self.0.write().mode = mode;
    }
    pub fn is_ironman(&self) -> bool {
        self.mode() != Some(Mode::None)
    }
    pub fn check_restriction(&self, ctx: &mut impl Context) -> Result<bool> {
        self.check(ctx, Some(Mode::Standard))
    }
    pub fn check(&self, ctx: &mut impl Context, minimum: Option<Mode>) -> Result<bool> {
        if self.is_ironman() {
            let mode = self.mode().ok_or(Error::Null)?;
            let minimum = minimum.ok_or(Error::Null)?;
            if mode as i32 >= minimum as i32 {
                let player = self.player();
                ctx.message(player.as_ref(), "You can't do that as an Ironman.")?;
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(crate) fn ownership(&self) -> (usize, usize, Option<NodeHandle>) {
        (self.0.identity(), self.0.strong_count(), self.player())
    }
}
