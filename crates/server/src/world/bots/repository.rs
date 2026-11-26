//! `AIRepository.kt`: what bots share. `groundItems` and `GEOffers` are
//! keyed by player identity and never iterated, so they are association
//! lists here; `PulseRepository` is walked by `clearAllBots` and keeps the
//! `HashMap` order (the native `JavaHashMap`).
use super::{AiPlayer, Error, Host, Result, ScriptPulse, ai_player, username, username_key};
use crate::definitions::item::Shared;
use crate::world::repository;

/// The `GrandExchangeOffer` fields `ScriptAPI.buyFromGE` sets and reads.
#[derive(Clone, Debug, PartialEq)]
pub struct BotOfferState<N> {
    pub item_id: i32,
    pub sell: bool,
    pub offered_value: i32,
    pub amount: i32,
    pub player: Option<N>,
    pub completed_amount: i32,
}

/// A `GrandExchangeOffer` object a bot made: a shared handle, because the
/// exchange completes it while the bot's pulse waits.
pub struct BotOffer<N>(Shared<BotOfferState<N>>);
impl<N> Clone for BotOffer<N> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<N: Clone> BotOffer<N> {
    /// `GrandExchangeOffer()`.
    pub fn new() -> Self {
        Self(Shared::new(BotOfferState {
            item_id: 0,
            sell: false,
            offered_value: 0,
            amount: 0,
            player: None,
            completed_amount: 0,
        }))
    }
    pub fn get(&self) -> BotOfferState<N> {
        self.0.read().clone()
    }
    pub fn update(&self, f: impl FnOnce(&mut BotOfferState<N>)) {
        f(&mut self.0.write());
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
}
impl<N: Clone> Default for BotOffer<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// `AIRepository.addItem(item)`: under the item's dropper. The map's key
/// type is not nullable, so an item without a dropper fails.
pub fn add_item<H: Host>(host: &mut H, item: &H::Node) -> Result<()> {
    let dropper = host.dropper(item).ok_or(Error::Null)?;
    host.statics().write(
        |s| match s.ground_items.iter_mut().find(|(key, _)| *key == dropper) {
            Some((_, list)) => list.push(item.clone()),
            None => s.ground_items.push((dropper, vec![item.clone()])),
        },
    );
    Ok(())
}

/// `AIRepository.getItems(player)`.
pub fn get_items<H: Host>(host: &mut H, player: &H::Node) -> Option<Vec<H::Node>> {
    host.statics().read(|s| {
        s.ground_items
            .iter()
            .find(|(key, _)| key == player)
            .map(|(_, list)| list.clone())
    })
}

/// `AIRepository.getItems(player)?.remove(item)`: the first equal element.
/// Removing null from a list of items removes nothing.
pub(crate) fn remove_item<H: Host>(host: &mut H, player: &H::Node, item: Option<&H::Node>) {
    let Some(item) = item else {
        return;
    };
    host.statics().write(|s| {
        if let Some((_, list)) = s.ground_items.iter_mut().find(|(key, _)| key == player)
            && let Some(at) = list.iter().position(|held| held == item)
        {
            list.remove(at);
        }
    });
}

/// `AIRepository.addOffer(player, offer)`.
pub fn add_offer<H: Host>(host: &mut H, player: &H::Node, offer: &super::BotOffer<H::Node>) {
    host.statics().write(
        |s| match s.ge_offers.iter_mut().find(|(key, _)| key == player) {
            Some(entry) => entry.1 = offer.clone(),
            None => s.ge_offers.push((player.clone(), offer.clone())),
        },
    );
}

/// `AIRepository.getOffer(player)`.
pub fn get_offer<H: Host>(host: &mut H, player: &H::Node) -> Option<super::BotOffer<H::Node>> {
    host.statics().read(|s| {
        s.ge_offers
            .iter()
            .find(|(key, _)| key == player)
            .map(|(_, offer)| offer.clone())
    })
}

/// `AIRepository.clearAllBots()`: over a copy of `PulseRepository` in its
/// iteration order, each pulse is stopped, its bot cleared (the virtual
/// `clear()`), and the bot deregistered by its uid. `AIPlayer.clear` has
/// already taken the bot out of `botMapping` by then, so the deregistration
/// finds nothing and the bot keeps its slot in `Repository.players`. A
/// script whose bot is not an `AIPlayer` fails the cast after its clear.
pub fn clear_all_bots<H: Host + repository::Host<<H as Host>::Node>>(host: &mut H) -> Result<()> {
    let pulses: Vec<ScriptPulse<H>> = host
        .statics()
        .read(|s| s.pulse_repository.iter().map(|(_, p)| p.clone()).collect());
    for pulse in pulses {
        pulse.stop(host)?;
        let bot = pulse.script().bot().ok_or(Error::Null)?;
        ai_player::virtual_clear(host, &bot)?;
        let bot = pulse.script().bot().ok_or(Error::Null)?;
        let ai = host.ai_player(&bot).ok_or(Error::Cast)?;
        ai_player::deregister(host, ai.uid())?;
    }
    Ok(())
}

/// `AIRepository.sendBotInfo(player, bot)`: nothing for a bot without a
/// script pulse under its lower-case user name.
pub fn send_bot_info<H: Host>(host: &mut H, player: &H::Node, bot: &AiPlayer<H>) -> Result<()> {
    let node = bot.node();
    let key = username_key(host, &node)?;
    let Some(pulse) = host
        .statics()
        .read(|s| s.pulse_repository.get(&Some(key)).cloned())
    else {
        return Ok(());
    };
    host.set_attribute_true(&node, "tracked")?;
    let script = pulse.script();
    let name = username(host, &node)?;
    let class = host.script_simple_name(&script.class());
    host.debug(player, &format!("[Bot: {name}][{class}]================="))?;
    let running = if host.pulse_manager_has_pulse_running(&node)? {
        let current = host.pulse_manager_current(&node)?.ok_or(Error::Null)?;
        host.pulse_class_name(&current)?
    } else {
        "No".to_owned()
    };
    host.debug(player, &format!("PM Pulse Running? {running}"))?;
    let active = host.active_script(&node)?.is_some();
    let execution = match host.active_script(&node)? {
        Some(script) => format!(
            " : {}",
            host.active_script_execution_class(&script)?
                .ok_or(Error::Null)?
        ),
        None => String::new(),
    };
    host.debug(
        player,
        &format!("Interaction Running? {active} {execution}"),
    )?;
    host.debug(player, &format!("Botscript Running? {}", script.running()))?;
    host.debug(player, &format!("Random Delay? {}", pulse.random_delay()))?;
    let delay = host.scripts_delay(&node)?;
    host.debug(player, &format!("Delayed? {delay}"))?;
    if let Some(state) = host.adventurer_state(&script) {
        host.debug(player, &format!("State: {state}"))?;
    }
    host.debug(player, "==========================================")
}
