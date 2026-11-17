//! [`LoginFlow`] behind the lifecycle's [`LoginSeam`], and the hand-over of
//! its socket to the game connection.
//!
//! `client.mainLoop` calls `CreateManager.loop`, `LoginManager.loopAuto` and
//! `LoginManager.loop` once per cycle in game state 10 and `LoginManager.loop`
//! alone in state 40 (client.java:1729-1745). [`FlowSeam::cycle_game_state`]
//! is that call for one cycle: it runs the flow and lets the [`Lifecycle`]
//! act on what came of it. The lifecycle owns the game state; the flow keeps
//! its own copy because its script guards and its login block read it, and
//! [`FlowSeam::mirror`] keeps the copy equal.
//!
//! The title screen's scripts start a login themselves (CS2 5600 calls
//! `LoginManager.startLogin` on the flow), so [`LoginSeam::start`] has
//! nothing to arm in that case; it arms one itself only for a caller that
//! has no script to do it.
use crate::{
    lifecycle::{GameState, Lifecycle, LoginPoll, LoginSeam},
    live::Connection,
    login_flow::{
        Dialer, Entered, Environment, Event, LoginFlow, Reconnected, TransportDialer,
        TransportSocket,
    },
    session::{self, Session},
};
use rs09_protocol::game::{Decoder, Direction, Frame};
use std::{cell::RefCell, rc::Rc};

/// The inbound buffer and payload limits of a game connection, as
/// [`crate::session`] has them.
const BUFFER_LIMIT: usize = 1024 * 1024;
const PAYLOAD_LIMIT: usize = 65535;

/// [`Environment`] with its settings owned, kept current by the application.
#[derive(Clone, Debug, Default)]
pub struct Env {
    /// `DisplayMode.getWindowMode()`.
    pub window_mode: i32,
    /// `GameShell.canvasWidth` and `canvasHeight`.
    pub canvas: [i32; 2],
    /// `Preferences.antiAliasingMode`.
    pub anti_aliasing: i32,
    /// `client.writeUid`.
    pub uid: [u8; 24],
    /// `client.settings`.
    pub settings: Vec<u8>,
    /// `client.affiliate`.
    pub affiliate: i32,
    /// `Preferences.toInt()`.
    pub preferences: i32,
    /// `Protocol.verifyId`.
    pub verify_id: i32,
    /// `js5Archive0..27.getChecksum()`.
    pub checksums: [i32; 28],
}
impl Env {
    fn borrow(&self) -> Environment<'_> {
        Environment {
            // The standalone client has no advert frame to suppress.
            advert_suppressed: false,
            window_mode: self.window_mode,
            canvas_width: self.canvas[0],
            canvas_height: self.canvas[1],
            anti_aliasing: self.anti_aliasing,
            uid: self.uid,
            settings: &self.settings,
            affiliate: self.affiliate,
            preferences: self.preferences,
            verify_id: self.verify_id,
            checksums: self.checksums,
        }
    }
}

/// The flow as the lifecycle's login code.
pub struct FlowSeam<D: Dialer> {
    pub flow: Rc<RefCell<LoginFlow<D>>>,
    pub env: Env,
    /// The login block went out with `Preferences.toInt()` in it: the
    /// application sets `Preferences.sentToServer` and clears this.
    pub preferences_sent: bool,
    entered: Option<Box<Entered>>,
    reconnected: Option<Reconnected>,
}
impl<D: Dialer> FlowSeam<D> {
    pub fn new(flow: Rc<RefCell<LoginFlow<D>>>) -> Self {
        Self {
            flow,
            env: Env {
                canvas: [765, 503],
                ..Env::default()
            },
            preferences_sent: false,
            entered: None,
            reconnected: None,
        }
    }

    /// The login that produced the lifecycle's `Effect::ResetSession`.
    pub fn take_entered(&mut self) -> Option<Box<Entered>> {
        self.entered.take()
    }
    /// The login that produced the lifecycle's `Effect::Reconnect`.
    pub fn take_reconnected(&mut self) -> Option<Reconnected> {
        self.reconnected.take()
    }

    /// `client.gameState` as the lifecycle now has it, for the flow's copy
    /// (`client.setGameState`: entering 40 arms the reconnect login, any
    /// other change closes the set-aside socket).
    pub fn mirror(&mut self, state: GameState) {
        self.flow.borrow_mut().set_game_state(state.code());
    }

    /// The login part of one 20 ms `client.mainLoop` cycle in game states 10
    /// and 40. In every other state the caller reads game packets instead.
    pub fn cycle_game_state(&mut self, life: &mut Lifecycle) {
        match life.state() {
            GameState::Title => {
                // A script (CS2 5600) or a finished world hop armed a login.
                if !life.logging_in() && self.flow.borrow().login.step != 0 {
                    life.start_login(self);
                }
                if life.logging_in() {
                    life.cycle(self);
                } else {
                    // CreateManager.loop and LoginManager.loopAuto;
                    // LoginManager.loop returns at once with nothing armed.
                    let env = self.env.borrow();
                    let _ = self.flow.borrow_mut().cycle(&env);
                }
            }
            GameState::ConnectionLost => life.cycle(self),
            GameState::Loading | GameState::InGame => {}
        }
    }
}
impl<D: Dialer> LoginSeam for FlowSeam<D> {
    fn start(&mut self, reconnect: bool) {
        let mut flow = self.flow.borrow_mut();
        if reconnect {
            // Protocol.handleConnectionLost: setGameState(40) runs
            // LoginManager.clear().
            flow.set_game_state(40);
            if flow.login.step == 0 {
                // The flow's copy of the state was 40 already.
                flow.clear();
            }
        } else if flow.login.step == 0 {
            flow.clear();
        }
        self.entered = None;
        self.reconnected = None;
    }
    fn cycle(&mut self) -> LoginPoll {
        let env = self.env.borrow();
        let mut flow = self.flow.borrow_mut();
        let mut poll = None;
        for event in flow.cycle(&env) {
            match event {
                Event::PreferencesSent => self.preferences_sent = true,
                Event::Entered(entered) => {
                    poll = Some(LoginPoll::Entered(entered.world.clone()));
                    self.entered = Some(entered);
                }
                Event::Reconnected(reconnected) => {
                    poll = Some(LoginPoll::Reconnected);
                    self.reconnected = Some(reconnected);
                }
                Event::LoggedOut { reply, .. } => poll = Some(LoginPoll::Failed(reply)),
            }
        }
        if let Some(poll) = poll {
            return poll;
        }
        // At the title screen a refused or failed attempt only leaves its
        // reply code behind. Reply 1 (step 5) waits for the script's
        // continue command and is still in progress.
        let login = &flow.login;
        if flow.game_state == 10 && login.step == 0 && login.hop_step == 0 && login.reply != -3 {
            LoginPoll::Failed(login.reply)
        } else {
            LoginPoll::Pending
        }
    }
}

/// The frames among the bytes that arrived behind the login reply.
fn feed(session: &mut Session, pending: &[u8], frames: &mut Vec<Frame>) -> Result<(), String> {
    for event in session.feed(pending).map_err(|e| e.to_string())? {
        if let session::Event::Frame(frame) = event {
            frames.push(frame);
        }
    }
    Ok(())
}

/// `Protocol.socket` after `LoginManager.loop` step 9 as a game connection:
/// the flow's socket, ciphers where the login left them, the first packet
/// (the scene rebuild) and whatever arrived behind it as unread frames.
pub fn game_connection(
    flow: &mut LoginFlow<TransportDialer>,
    entered: &Entered,
) -> Result<Connection, String> {
    let socket: TransportSocket = flow.net.socket.take().ok_or("Login left no socket")?;
    let (transport, pending) = socket.into_parts();
    let profile = flow.profile();
    let mut session = Session::established(
        profile,
        entered.outbound_cipher(profile),
        entered.decoder(profile, BUFFER_LIMIT, PAYLOAD_LIMIT),
    );
    let mut frames = vec![entered.frame()];
    feed(&mut session, &pending, &mut frames)?;
    Ok(Connection::established(
        transport,
        session,
        Some(entered.world.clone()),
        frames,
    ))
}

/// `Protocol.socket` after reply 15 in game state 40: the flow's new socket
/// with fresh ciphers. The player details are those of the session it
/// resumes, which the caller copies from the connection it set aside.
pub fn resumed_connection(
    flow: &mut LoginFlow<TransportDialer>,
    reconnected: &Reconnected,
) -> Result<Connection, String> {
    let socket: TransportSocket = flow.net.socket.take().ok_or("Login left no socket")?;
    let (transport, pending) = socket.into_parts();
    let profile = flow.profile();
    let (outbound, inbound) = reconnected.ciphers(profile);
    let decoder =
        Decoder::new(Direction::ServerToClient, BUFFER_LIMIT, PAYLOAD_LIMIT).with_cipher(inbound);
    let mut session = Session::established(profile, outbound, decoder);
    let mut frames = Vec::new();
    feed(&mut session, &pending, &mut frames)?;
    Ok(Connection::established(transport, session, None, frames))
}
