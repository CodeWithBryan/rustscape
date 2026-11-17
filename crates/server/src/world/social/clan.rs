//! `ClanRepository` and `ClanEntry`: the clan registry, entry, leaving,
//! kicking, ranks and requirements, the member list and the packets each
//! sends. `ClanMember` (a rank holder no code uses) has no native form.
use super::management::Event;
use super::names::format_display_name;
use super::packets::{CLAN_MESSAGE, Queued};
use super::{Error, Host, Result, Social, chat_icon, rank_ordinal, rights};
use crate::player::communication::{ClanRank, CommunicationInfo};
use crate::player::details::Rights;
use rs09_game_core::java_lower::lowercase_root;
use std::collections::HashMap;
use std::hash::Hash;

/// The identity of a `ClanRepository` object. Objects replaced in the
/// registry keep their identity for the players and packets that hold them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClanId(pub usize);

/// `ClanEntry`: a member of a clan chat, on this world (with its player) or
/// on another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClanEntry<P> {
    pub name: String,
    pub player: Option<P>,
    pub world_id: i32,
}

/// `ClanEntry.equals`: the names match and both hold the same player (or none).
fn same_entry<P: Eq>(this: &ClanEntry<P>, other: &ClanEntry<P>) -> bool {
    this.name == other.name && this.player == other.player
}

/// A `ClanRepository` object's fields. Requirements are nullable, as the
/// saved requirement parse can leave them.
#[derive(Clone, Debug)]
pub struct Clan<P> {
    pub owner: String,
    pub name: String,
    pub join_requirement: Option<ClanRank>,
    pub message_requirement: Option<ClanRank>,
    pub kick_requirement: Option<ClanRank>,
    pub loot_requirement: Option<ClanRank>,
    /// `ranks`; a contact's username may be null and its rank may be null.
    pub ranks: HashMap<Option<String>, Option<ClanRank>>,
    pub banned: HashMap<String, i64>,
    pub players: Vec<ClanEntry<P>>,
}
impl<P> Clan<P> {
    /// `new ClanRepository(owner)`.
    pub fn new(owner: String) -> Self {
        Self {
            owner,
            name: "Chat disabled".into(),
            join_requirement: Some(ClanRank::Friend),
            message_requirement: Some(ClanRank::None),
            kick_requirement: Some(ClanRank::Owner),
            loot_requirement: Some(ClanRank::Administrator),
            ranks: HashMap::new(),
            banned: HashMap::new(),
            players: Vec::with_capacity(100),
        }
    }
}

const MAX_MEMBERS: usize = 100;

impl<P: Clone + Eq + Hash> Social<P> {
    pub(crate) fn add_clan(&mut self, clan: Clan<P>) -> ClanId {
        self.clans.push(clan);
        ClanId(self.clans.len() - 1)
    }

    /// `ClanRepository.get(owner, create)`. A registered clan is returned as
    /// it is. Otherwise the owner's details are read: an online owner's, or
    /// `new PlayerDetails(owner)`, whose default account reads the clock and
    /// whose communication info is the default one (so an offline owner has
    /// no clan name). Without a name, nothing is created unless `create`, and
    /// then the clan is called "Chat disabled". The new clan takes the
    /// details' contacts as ranks and their four requirements.
    pub fn clan_get<H: Host<P>>(
        &mut self,
        host: &mut H,
        owner: &str,
        create: bool,
    ) -> Result<Option<ClanId>> {
        if let Some(&id) = self.registry.get(owner) {
            return Ok(Some(id));
        }
        let read = |communication: &CommunicationInfo| {
            let ranks: Vec<_> = communication
                .contacts
                .iter()
                .map(|(_, contact)| (contact.username.clone(), contact.rank))
                .collect();
            (
                communication.clan_name.clone(),
                ranks,
                communication.requirements,
            )
        };
        let (clan_name, ranks, requirements) = match host.player_by_name(owner) {
            Some(player) => host.with_details(&player, |details| read(&details.communication)),
            None => {
                let _ = crate::auth::account::UserAccountInfo::create_default(
                    host.current_time_millis(),
                );
                read(&CommunicationInfo::default())
            }
        };
        let mut name = clan_name.ok_or(Error::NullPointer)?;
        if name.is_empty() {
            if !create {
                return Ok(None);
            }
            name = "Chat disabled".into();
        }
        let id = self.add_clan(Clan::new(owner.to_owned()));
        self.registry.insert(owner.to_owned(), id);
        let clan = &mut self.clans[id.0];
        for (username, rank) in ranks {
            clan.ranks.insert(username, rank);
        }
        clan.name = name;
        clan.join_requirement = requirements[0];
        clan.message_requirement = requirements[1];
        clan.kick_requirement = requirements[2];
        clan.loot_requirement = requirements[3];
        Ok(Some(id))
    }

    /// `ClanRepository.getDefault()`: the clan of the world's name in lower case.
    pub fn clan_default<H: Host<P>>(&mut self, host: &mut H) -> Result<Option<ClanId>> {
        let owner = lowercase_root(&host.world_name());
        self.clan_get(host, &owner, false)
    }

    /// `isDefault()`.
    pub fn clan_is_default<H: Host<P>>(&mut self, host: &mut H, clan: ClanId) -> bool {
        self.clans[clan.0].owner == lowercase_root(&host.world_name())
    }

    /// `enter(player)`. A full channel (100 entries, except the server's own)
    /// refuses everyone; the owner and administrators skip the ban, the
    /// owner's ignore list and the join requirement. The entry is added once,
    /// the join is published and every member on this world gets the list.
    pub fn clan_enter<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        player: &P,
    ) -> Result<bool> {
        let owner = self.clans[clan.0].owner.clone();
        if owner != lowercase_root(&host.server_name())
            && self.clans[clan.0].players.len() >= MAX_MEMBERS
        {
            host.dispatch_message(player, "The channel you tried to join is full.:clan:")?;
            return Ok(false);
        }
        let name = host.name(player);
        if name != owner && rights(host, player)? != Rights::Administrator {
            if self.clan_is_banned(host, clan, &name) {
                host.dispatch_message(
                    player,
                    "You are temporarily banned from this clan channel.:clan:",
                )?;
                return Ok(false);
            }
            if let Some(o) = host.player_by_name(&owner) {
                let blocked = host.with_details(&o, |details| {
                    details.communication.blocked.contains(&Some(name.clone()))
                });
                if blocked {
                    host.dispatch_message(
                        player,
                        "You do not have a high enough rank to join this clan channel.:clan:",
                    )?;
                    return Ok(false);
                }
            }
            let rank = self.clan_rank_of(host, clan, player)?;
            if rank.ordinal() < rank_ordinal(self.clans[clan.0].join_requirement)? {
                host.dispatch_message(
                    player,
                    "You do not have a high enough rank to join this clan channel.:clan:",
                )?;
                return Ok(false);
            }
        }
        let entry = ClanEntry {
            name: host.name(player),
            player: Some(player.clone()),
            world_id: host.world_id(),
        };
        if !self.clans[clan.0]
            .players
            .iter()
            .any(|other| same_entry(&entry, other))
        {
            self.clans[clan.0].players.push(entry);
        }
        let event = Event::ClanJoinNotification {
            username: host.name(player),
            world: host.world_id(),
            clan_name: owner,
        };
        self.publish(event);
        let channel = self.clans[clan.0].name.clone();
        host.dispatch_message(
            player,
            &format!("Now talking in clan channel {channel}:clan:"),
        )?;
        host.dispatch_message(
            player,
            "To talk, start each line of chat with the / symbol.:clan:",
        )?;
        self.clan_update(host, clan);
        Ok(true)
    }

    /// `clean(disable)`: nothing while the management server is available.
    /// Otherwise every entry (all of them, or those below the join
    /// requirement) leaves without removal, loses its clan and is taken out
    /// by the iterator. An entry of another world has no player: ranking it
    /// fails with a NullPointerException, as does making it leave.
    pub fn clan_clean<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        disable: bool,
    ) -> Result<()> {
        if host.ms_enabled() {
            return Ok(());
        }
        let mut cursor = 0;
        while cursor < self.clans[clan.0].players.len() {
            let player = self.clans[clan.0].players[cursor].player.clone();
            let mut remove = disable;
            if !remove {
                let member = player.as_ref().ok_or(Error::NullPointer)?;
                let rank = self.clan_rank_of(host, clan, member)?;
                remove = rank.ordinal() < rank_ordinal(self.clans[clan.0].join_requirement)?;
            }
            if remove {
                let member = player.ok_or(Error::NullPointer)?;
                self.clan_leave(
                    host,
                    clan,
                    &member,
                    false,
                    "You have left the channel.:clan:",
                )?;
                self.set_clan(host, &member, None);
                self.clans[clan.0].players.remove(cursor);
                continue;
            }
            cursor += 1;
        }
        if self.clans[clan.0].players.is_empty() {
            self.clans[clan.0].banned.clear();
        }
        self.clan_update(host, clan);
        Ok(())
    }

    /// `isBanned(name)`: a ban still in the future (the clock is read only
    /// for a banned name); an expired ban is removed.
    pub fn clan_is_banned<H: Host<P>>(&mut self, host: &mut H, clan: ClanId, name: &str) -> bool {
        if let Some(&time) = self.clans[clan.0].banned.get(name) {
            if time > host.current_time_millis() {
                return true;
            }
            self.clans[clan.0].banned.remove(name);
        }
        false
    }

    /// `message(player, text)`: the one-tick `cc_message` lock and the ban
    /// stop it silently; below the message requirement (unless owner or
    /// administrator) it is refused after the lock. Each member with a player
    /// gets a clan message packet, its chat icon read for each one.
    pub fn clan_message<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        player: &P,
        text: &str,
    ) -> Result<()> {
        if host.is_locked(player, "cc_message")? {
            return Ok(());
        }
        let name = host.name(player);
        if self.clan_is_banned(host, clan, &name) {
            return Ok(());
        }
        host.lock(player, "cc_message", 1)?;
        let owner = self.clans[clan.0].owner.clone();
        if host.name(player) != owner && rights(host, player)? != Rights::Administrator {
            let rank = self.clan_rank_of(host, clan, player)?;
            if rank.ordinal() < rank_ordinal(self.clans[clan.0].message_requirement)? {
                host.dispatch_message(
                    player,
                    "You do not have a high enough rank to talk in this clan channel.:clan:",
                )?;
                return Ok(());
            }
        }
        host.log_chat(player, "clan", text)?;
        let mut index = 0;
        while index < self.clans[clan.0].players.len() {
            if let Some(recipient) = self.clans[clan.0].players[index].player.clone() {
                let other = host.name(player);
                let icon = chat_icon(host, player)?;
                self.send_packet(
                    host,
                    Queued::Message {
                        player: recipient,
                        opcode: CLAN_MESSAGE,
                        other,
                        chat_icon: icon,
                        message: text.to_owned(),
                    },
                );
            }
            index += 1;
        }
        Ok(())
    }

    /// `kick(player, target)`: administrators cannot be kicked, nor oneself,
    /// nor the owner; a kicker below the kick requirement (unless an
    /// administrator) is refused. Every entry is sent the notice (an entry
    /// of another world fails with a NullPointerException there), then the
    /// target leaves with removal, loses its clan and is banned for an hour.
    pub fn clan_kick<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        player: &P,
        target: &P,
    ) -> Result<()> {
        let rank = self.clan_rank_of(host, clan, player)?;
        if rights(host, target)? == Rights::Administrator {
            host.player_message(player, "You can't kick an administrator.:clan:")?;
            return Ok(());
        }
        if host.name(target) == host.name(player) {
            host.player_message(player, "You can't kick yourself.:clan:")?;
            return Ok(());
        }
        if rights(host, player)? != Rights::Administrator
            && rank.ordinal() < rank_ordinal(self.clans[clan.0].kick_requirement)?
        {
            host.dispatch_message(
                player,
                "You do not have a high enough rank to kick in this clan channel.:clan:",
            )?;
            return Ok(());
        }
        if host.name(target) == self.clans[clan.0].owner {
            host.dispatch_message(
                player,
                "You can't kick the owner of this clan channel.:clan:",
            )?;
            return Ok(());
        }
        let mut index = 0;
        while index < self.clans[clan.0].players.len() {
            let recipient = self.clans[clan.0].players[index].player.clone();
            let notice = format!(
                "[Attempting to kick/ban {} from this Clan Chat.]",
                format_display_name(&host.name(target))
            );
            let other = host.name(player);
            let icon = chat_icon(host, player)?;
            let recipient = recipient.ok_or(Error::NullPointer)?;
            self.send_packet(
                host,
                Queued::Message {
                    player: recipient,
                    opcode: CLAN_MESSAGE,
                    other,
                    chat_icon: icon,
                    message: notice,
                },
            );
            index += 1;
        }
        self.clan_leave(
            host,
            clan,
            target,
            true,
            "You have been kicked from the channel.:clan:",
        )?;
        self.set_clan(host, target, None);
        let name = host.name(target);
        let until = host.current_time_millis().wrapping_add(3_600_000);
        self.clans[clan.0].banned.insert(name, until);
        Ok(())
    }

    /// `leave(player, remove)`, with its default message.
    pub fn clan_leave_default<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        player: &P,
        remove: bool,
    ) -> Result<()> {
        self.clan_leave(
            host,
            clan,
            player,
            remove,
            "You have left the channel.:clan:",
        )
    }

    /// `leave(player, remove, message)`: with removal, the first equal entry
    /// goes, the members are updated and an empty channel forgets its bans.
    /// The player is sent the leave packet and the message, and the leave is
    /// published. The clan-wars hook is not reached: nothing sets a clan war.
    pub fn clan_leave<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        player: &P,
        remove: bool,
        message: &str,
    ) -> Result<()> {
        if remove {
            let entry = ClanEntry {
                name: host.name(player),
                player: Some(player.clone()),
                world_id: host.world_id(),
            };
            if let Some(index) = self.clans[clan.0]
                .players
                .iter()
                .position(|other| same_entry(&entry, other))
            {
                self.clans[clan.0].players.remove(index);
            }
            self.clan_update(host, clan);
            if self.clans[clan.0].players.is_empty() {
                self.clans[clan.0].banned.clear();
            }
        }
        self.send_packet(
            host,
            Queued::Clan {
                player: player.clone(),
                clan,
                leave: true,
            },
        );
        host.dispatch_message(player, message)?;
        let event = Event::ClanLeaveNotification {
            clan_name: self.clans[clan.0].owner.clone(),
            username: host.name(player),
            world: host.world_id(),
        };
        self.publish(event);
        Ok(())
    }

    /// `rank(name, rank)`: NONE removes the name, anything else replaces its
    /// rank; when that changed the map's answer, the channel is cleaned.
    pub fn clan_rank<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        name: &str,
        rank: Option<ClanRank>,
    ) -> Result<()> {
        let ranks = &mut self.clans[clan.0].ranks;
        let update = if rank == Some(ClanRank::None) {
            matches!(ranks.remove(&Some(name.to_owned())), Some(Some(_)))
        } else {
            ranks.insert(Some(name.to_owned()), rank).flatten() != rank
        };
        if update {
            self.clan_clean(host, clan, false)?;
        }
        Ok(())
    }

    /// `update()`: every entry of this world with a player gets the list.
    pub fn clan_update<H: Host<P>>(&mut self, host: &mut H, clan: ClanId) {
        let mut index = 0;
        while index < self.clans[clan.0].players.len() {
            let entry = &self.clans[clan.0].players[index];
            if entry.world_id == host.world_id() {
                if let Some(player) = entry.player.clone() {
                    self.send_packet(
                        host,
                        Queued::Clan {
                            player,
                            clan,
                            leave: false,
                        },
                    );
                }
            }
            index += 1;
        }
    }

    /// `getRank(entry)`: the player's rank, or for another world's entry the
    /// rank its name holds (NONE when none).
    pub fn clan_rank_of_entry<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        index: usize,
    ) -> Result<ClanRank> {
        let (name, player) = {
            let entry = &self.clans[clan.0].players[index];
            (entry.name.clone(), entry.player.clone())
        };
        if let Some(player) = player {
            return self.clan_rank_of(host, clan, &player);
        }
        Ok(self.clans[clan.0]
            .ranks
            .get(&Some(name))
            .copied()
            .flatten()
            .unwrap_or(ClanRank::None))
    }

    pub(crate) fn rank_of_entry<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        index: usize,
    ) -> Result<ClanRank> {
        self.clan_rank_of_entry(host, clan, index)
    }

    /// `getRank(player)`: an administrator who is not the owner is
    /// ADMINISTRATOR; otherwise the held rank, or OWNER for the owner and
    /// NONE for anyone else.
    pub fn clan_rank_of<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        player: &P,
    ) -> Result<ClanRank> {
        let name = host.name(player);
        let rank = self.clans[clan.0]
            .ranks
            .get(&Some(name.clone()))
            .copied()
            .flatten();
        if rights(host, player)? == Rights::Administrator && name != self.clans[clan.0].owner {
            return Ok(ClanRank::Administrator);
        }
        Ok(match rank {
            Some(rank) => rank,
            None if name == self.clans[clan.0].owner => ClanRank::Owner,
            None => ClanRank::None,
        })
    }

    /// `ClanRepository.openSettings(player)`: interface 590, then the
    /// player's own clan's settings if it has one.
    pub fn clan_open_settings<H: Host<P>>(&mut self, host: &mut H, player: &P) -> Result<()> {
        host.open_component(player, 590)?;
        let owner = host.name(player);
        if let Some(clan) = self.clan_get(host, &owner, false)? {
            self.clan_update_settings(host, clan, player)?;
        }
        Ok(())
    }

    /// `updateSettings(player)`: the name and the four requirement texts.
    pub fn clan_update_settings<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        player: &P,
    ) -> Result<()> {
        let name = self.clans[clan.0].name.clone();
        host.send_string(player, &name, 590, 22)?;
        let info = |rank: Option<ClanRank>| rank.map(ClanRank::info).ok_or(Error::NullPointer);
        let text = info(self.clans[clan.0].join_requirement)?;
        host.send_string(player, text, 590, 23)?;
        let text = info(self.clans[clan.0].message_requirement)?;
        host.send_string(player, text, 590, 24)?;
        let text = info(self.clans[clan.0].kick_requirement)?;
        host.send_string(player, text, 590, 25)?;
        let text = info(self.clans[clan.0].loot_requirement)?;
        host.send_string(player, text, 590, 26)
    }

    /// `delete()`: the owner's registry entry goes, then everyone leaves.
    pub fn clan_delete<H: Host<P>>(&mut self, host: &mut H, clan: ClanId) -> Result<()> {
        let owner = self.clans[clan.0].owner.clone();
        self.registry.remove(&owner);
        self.clan_clean(host, clan, true)
    }

    /// `setJoinRequirement`: then the channel is cleaned.
    pub fn clan_set_join_requirement<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        rank: Option<ClanRank>,
    ) -> Result<()> {
        self.clans[clan.0].join_requirement = rank;
        self.clan_clean(host, clan, false)
    }
    /// `setMessageRequirement`.
    pub fn clan_set_message_requirement(&mut self, clan: ClanId, rank: Option<ClanRank>) {
        self.clans[clan.0].message_requirement = rank;
    }
    /// `setKickRequirement`: then the members are updated.
    pub fn clan_set_kick_requirement<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan: ClanId,
        rank: Option<ClanRank>,
    ) {
        self.clans[clan.0].kick_requirement = rank;
        self.clan_update(host, clan);
    }
    /// `setLootRequirement`.
    pub fn clan_set_loot_requirement(&mut self, clan: ClanId, rank: Option<ClanRank>) {
        self.clans[clan.0].loot_requirement = rank;
    }
    /// `setName`.
    pub fn clan_set_name(&mut self, clan: ClanId, name: String) {
        self.clans[clan.0].name = name;
    }
}
