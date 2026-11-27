//! The state of the systems installed in the live world after the NPC class
//! overrides and the dragons: the dialogue interpreter per player and its
//! content (super::talk), the content listeners' walk to an NPC
//! ([`Approaches`]), input prompts, the bank (super::bank), shops, trade and
//! requests, following (super::follow). Each part lives in its own module; this
//! is only the world's holder of their state.
use crate::world::{
    doors::Location,
    interaction::OptionHandle,
    interaction_listeners::{Arrival, Function},
};
use std::collections::HashMap;

/// The content listeners' movement pulses (`InteractionListeners.run`'s
/// `MovementPulse(player, node, flag, destOverride)`): each is a movement of
/// the canonical state whose arrival is an [`crate::world::interaction::Approach`]
/// carrying a fresh option handle; the walk host finds the arrival here by
/// that handle's identity and the world runs it right after the walking call
/// (super::talk).
#[derive(Default)]
pub struct Approaches {
    /// The pending arrivals.
    pub arrivals: Vec<PendingArrival>,
    /// A destination override's answer, by the token the movement carries
    /// (`DestinationService::Override(token)`), with its function.
    pub destinations: HashMap<usize, (Function, Option<Location>)>,
    /// The token of each player's latest override (its earlier one is
    /// dropped when a new movement is made).
    pub player_tokens: HashMap<usize, usize>,
    pub next_token: usize,
}

/// What runs when an approach arrives.
pub enum Pending {
    /// `InteractionListeners.run`'s movement pulse body.
    Listener(Arrival<super::items::Node, super::items::Node>),
    /// `UseWithHandler.run`'s movement pulse body (`UseWithHandler$1`).
    UseWith(crate::world::packet_handlers::use_with::Body<super::items::Node>),
}

/// A pending arrival: its option handle (the identity the walk host finds
/// it by), the player node, the body, and the lease of a scene object
/// target's canonical node.
pub struct PendingArrival {
    pub option: OptionHandle,
    pub player: usize,
    pub pending: Pending,
    pub lease: Option<crate::world::player_walk::NodeHandle>,
}

/// The holder of this install round's state ([`super::host::Live::round`]).
#[derive(Default)]
pub struct Round {
    pub talk: super::talk::TalkWorld,
    pub trade: super::trading::TradeWorld,
    pub follow: super::follow::FollowWorld,
    pub eat: super::eating::EatWorld,
    pub magic: super::magic::MagicWorld,
    /// The content use-with handlers (super::items::everyday).
    pub everyday: super::items::everyday::EverydayWorld,
    /// Cooking (super::items::cooking).
    pub cooking: super::items::cooking::CookingWorld,
    /// `ServerConstants.SECOND_BANK`.
    pub second_bank: bool,
}

impl super::host::Live {
    /// A pulse this round submitted to `GameWorld.Pulser`, by identity:
    /// `Some` with whether it finished.
    pub fn round_pulse_body(
        &mut self,
        pulse: &crate::pulses::Handle,
    ) -> Option<Result<bool, String>> {
        if let Some(result) = self.trade_pulse(pulse) {
            return Some(result.map_err(|e| format!("{e:?}")));
        }
        None
    }
    /// A player left the world: what this round keeps for it goes.
    pub fn round_logout(&mut self, node: usize) {
        self.magic_logout(node);
        let talk = &mut self.round.talk;
        talk.interpreters.remove(&node);
        talk.close_hooks.remove(&node);
        talk.scripts.remove(&node);
        self.inner.shops.open.remove(&node);
        let trade = &mut self.round.trade;
        trade.modules.remove(&node);
        trade.targets.remove(&node);
        trade.last.remove(&node);
        trade.alerts.retain(|(_, player, _)| player.node != node);
        self.round
            .follow
            .movements
            .retain(|(_, _, _, player, target)| player.node != node && target.node != node);
        self.walk.follows.retain(|(_, mover)| *mover != node);
        self.round.eat.doses.retain(|(_, owner, _)| *owner != node);
        self.round.cooking.pulses.retain(|(handle, owner, _)| {
            if *owner == node {
                crate::pulses::stop_base(handle);
            }
            *owner != node
        });
        // The player's content pulses stop with it (the runner drops a
        // pulse that is not running).
        self.round.everyday.pulses.retain(|(handle, owner, _)| {
            if *owner == node {
                crate::pulses::stop_base(handle);
            }
            *owner != node
        });
        self.walk.approaches.arrivals.retain(|a| a.player != node);
        if let Some(token) = self.walk.approaches.player_tokens.remove(&node) {
            self.walk.approaches.destinations.remove(&token);
        }
    }
}
