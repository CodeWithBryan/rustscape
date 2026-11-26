//! The static social operations of `CommunicationInfo`: friends, ignores,
//! contact ranks, who is shown online, private messages and loot share. The
//! state they change is the native `player::communication::CommunicationInfo`.
use super::management::Event;
use super::names::format_display_name;
use super::packets::{ContactKind, Queued};
use super::{Host, Level, PulseId, Result, Social, rights};
use crate::player::communication::{ClanRank, Contact, MAX_LIST_SIZE};
use std::hash::Hash;

const NO_PLAYER: &str = "Unable to find a player by that name. Please try again.";

impl<P: Clone + Eq + Hash> Social<P> {
    fn friend_packet<H: Host<P>>(&mut self, host: &mut H, player: P, name: String, world_id: i32) {
        self.send_packet(
            host,
            Queued::Contact {
                player,
                kind: ContactKind::UpdateFriend,
                name: Some(name),
                world_id,
            },
        );
    }

    /// `CommunicationInfo.add(player, contact)`: an empty name is refused; with
    /// the management server available the update goes there; a full list
    /// (200) or a duplicate key is refused; the player's own clan ranks the
    /// name FRIEND; the contact is put; an online target is shown to the
    /// player, and the player to the target when the player's private chat
    /// is "friends".
    pub fn add<H: Host<P>>(&mut self, host: &mut H, player: &P, contact: &str) -> Result<()> {
        if contact.is_empty() {
            return host.player_message(player, NO_PLAYER);
        }
        if host.ms_enabled() {
            let name = host.name(player);
            return self.send_contact_update(host, &name, contact, false, false, None);
        }
        let (size, present) = host.with_details(player, |d| {
            (
                d.communication.contacts.len(),
                d.communication.contacts.contains_key(contact),
            )
        });
        if size >= MAX_LIST_SIZE {
            return host.dispatch_message(player, "Your friend list is full.");
        }
        if present {
            return host.dispatch_message(
                player,
                &format!(
                    "{} is already on your friend list.",
                    format_display_name(contact)
                ),
            );
        }
        let name = host.name(player);
        if let Some(clan) = self.clan_get(host, &name, false)? {
            self.clan_rank(host, clan, contact, Some(ClanRank::Friend))?;
        }
        host.with_details(player, |d| {
            d.communication
                .contacts
                .insert(contact.to_owned(), Contact::new(Some(contact.to_owned())));
        });
        if let Some(target) = host.player_by_name(contact) {
            if self.show_active(host, player, &target) {
                let world = host.world_id();
                self.friend_packet(host, player.clone(), contact.to_owned(), world);
            }
            if host.private_chat_setting(player) == 1 && self.show_active(host, &target, player) {
                let name = host.name(player);
                let world = host.world_id();
                self.friend_packet(host, target, name, world);
            }
        }
        Ok(())
    }

    /// `CommunicationInfo.remove(player, contact, block)`. From the ignore
    /// list: the first equal entry goes, and an online target who has the
    /// player as a contact is told the player's world (0 when hidden). From
    /// the friend list: the key goes, the player's own clan ranks the name
    /// NONE, and with private chat on "friends" an online target sees the
    /// player offline.
    pub fn remove<H: Host<P>>(
        &mut self,
        host: &mut H,
        player: &P,
        contact: &str,
        block: bool,
    ) -> Result<()> {
        if contact.is_empty() {
            return host.player_message(player, NO_PLAYER);
        }
        if host.ms_enabled() {
            let name = host.name(player);
            return self.send_contact_update(host, &name, contact, true, block, None);
        }
        if block {
            host.with_details(player, |d| {
                let blocked = &mut d.communication.blocked;
                if let Some(index) = blocked
                    .iter()
                    .position(|entry| entry.as_deref() == Some(contact))
                {
                    blocked.remove(index);
                }
            });
            if let Some(target) = host.player_by_name(contact) {
                let name = host.name(player);
                if self.has_contact(host, &target, &name) {
                    let mut world = 0;
                    if self.show_active(host, &target, player) {
                        world = host.world_id();
                    }
                    let name = host.name(player);
                    self.friend_packet(host, target, name, world);
                }
            }
        } else {
            host.with_details(player, |d| d.communication.contacts.remove(contact));
            let name = host.name(player);
            if let Some(clan) = self.clan_get(host, &name, false)? {
                self.clan_rank(host, clan, contact, Some(ClanRank::None))?;
            }
            if host.private_chat_setting(player) == 1
                && let Some(target) = host.player_by_name(contact)
            {
                let name = host.name(player);
                self.friend_packet(host, target, name, 0);
            }
        }
        Ok(())
    }

    /// `CommunicationInfo.block(player, contact)`: refused when empty, full
    /// (200) or already listed; added; an online target who has the player
    /// as a contact sees the player offline.
    pub fn block<H: Host<P>>(&mut self, host: &mut H, player: &P, contact: &str) -> Result<()> {
        if contact.is_empty() {
            return host.player_message(player, NO_PLAYER);
        }
        if host.ms_enabled() {
            let name = host.name(player);
            return self.send_contact_update(host, &name, contact, false, true, None);
        }
        let (size, present) = host.with_details(player, |d| {
            let blocked = &d.communication.blocked;
            (
                blocked.len(),
                blocked
                    .iter()
                    .any(|entry| entry.as_deref() == Some(contact)),
            )
        });
        if size >= MAX_LIST_SIZE {
            return host.dispatch_message(player, "Your ignore list is full.");
        }
        if present {
            return host.dispatch_message(
                player,
                &format!(
                    "{} is already on your ignore list.",
                    format_display_name(contact)
                ),
            );
        }
        host.with_details(player, |d| {
            d.communication.blocked.push(Some(contact.to_owned()))
        });
        if let Some(target) = host.player_by_name(contact) {
            let name = host.name(player);
            if self.has_contact(host, &target, &name) {
                self.friend_packet(host, target, name, 0);
            }
        }
        Ok(())
    }

    /// `CommunicationInfo.updateClanRank(player, contact, rank)`: a missing
    /// contact is logged; otherwise its rank is set, the player's own clan
    /// ranks the name, and the player is sent the contact with its stored
    /// world when it is shown online, 0 otherwise.
    pub fn update_clan_rank<H: Host<P>>(
        &mut self,
        host: &mut H,
        player: &P,
        contact: &str,
        rank: Option<ClanRank>,
    ) -> Result<()> {
        if contact.is_empty() {
            return host.player_message(player, NO_PLAYER);
        }
        let found = host.with_details(player, |d| {
            match d.communication.contacts.get_mut(contact) {
                Some(entry) => {
                    entry.rank = rank;
                    true
                }
                None => false,
            }
        });
        if !found {
            return host.log(
                "CommunicationInfo",
                Level::Err,
                &format!("Could not find contact {contact} to update clan rank!"),
            );
        }
        let name = host.name(player);
        if let Some(clan) = self.clan_get(host, &name, false)? {
            self.clan_rank(host, clan, contact, rank)?;
        }
        let mut world = 0;
        if self.show_active_by_name(host, player, contact) {
            world = host.with_details(player, |d| {
                d.communication
                    .contacts
                    .get(contact)
                    .map_or(0, |c| c.world_id)
            });
        }
        self.friend_packet(host, player.clone(), contact.to_owned(), world);
        Ok(())
    }

    /// `CommunicationInfo.hasContact(player, contact)`.
    pub fn has_contact<H: Host<P>>(&mut self, host: &mut H, player: &P, contact: &str) -> bool {
        host.with_details(player, |d| d.communication.contacts.contains_key(contact))
    }

    /// `CommunicationInfo.showActive(player, name)`: false unless the name is online.
    pub fn show_active_by_name<H: Host<P>>(
        &mut self,
        host: &mut H,
        player: &P,
        name: &str,
    ) -> bool {
        match host.player_by_name(name) {
            Some(target) => self.show_active(host, player, &target),
            None => false,
        }
    }

    /// `CommunicationInfo.showActive(player, target)`: whether the target is
    /// shown online to the player: never to oneself or to someone the target
    /// ignores; with the target's private chat on "friends" only to its
    /// contacts, on "off" to nobody, otherwise to everyone.
    pub fn show_active<H: Host<P>>(&mut self, host: &mut H, player: &P, target: &P) -> bool {
        let name = host.name(player);
        if host.name(target) == name {
            return false;
        }
        if host.with_details(target, |d| {
            d.communication.blocked.contains(&Some(name.clone()))
        }) {
            return false;
        }
        match host.private_chat_setting(target) {
            1 => self.has_contact(host, target, &name),
            2 => false,
            _ => true,
        }
    }

    /// `CommunicationInfo.sendMessage(player, target, message)`: published for
    /// the management queue with the sender's rights. Neither privacy nor
    /// ignore lists are consulted on delivery.
    pub fn send_message<H: Host<P>>(
        &mut self,
        host: &mut H,
        player: &P,
        target: &str,
        message: &str,
    ) -> Result<()> {
        let sender = host.name(player);
        let rank = rights(host, player)?.ordinal();
        self.publish(Event::PrivateMessage {
            sender,
            rank,
            message: message.to_owned(),
            receiver: target.to_owned(),
        });
        Ok(())
    }

    /// `toggleLootshare(player)`: on turns it off; a pending timer is
    /// stopped; otherwise varp 1083 shows 2 and a timer of 200 ticks (5 in
    /// development) is created, stored and submitted.
    pub fn toggle_lootshare<H: Host<P>>(&mut self, host: &mut H, player: &P) -> Result<()> {
        if host.with_details(player, |d| d.communication.loot_share) {
            host.with_details(player, |d| d.communication.loot_share = false);
            return host.varp(player, 1083, 0, false);
        }
        if let Some(pulse) = self.loot_share_pulse(player) {
            host.stop_pulse(pulse)?;
            self.links
                .entry(player.clone())
                .or_default()
                .loot_share_pulse = None;
            return host.varp(player, 1083, 0, false);
        }
        host.varp(player, 1083, 2, false)?;
        let delay = if host.dev_mode() { 5 } else { 200 };
        let pulse = PulseId(self.pulses.len());
        self.pulses.push(player.clone());
        self.links
            .entry(player.clone())
            .or_default()
            .loot_share_pulse = Some(pulse);
        host.submit_pulse(pulse, delay)
    }
}
