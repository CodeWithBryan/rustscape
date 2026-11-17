//! `ManagementEvents` in a single world: the event queue, the handlers that
//! deliver statuses, contact lists, private messages and clan traffic, the
//! clan-information round trip, plus the events login (`Login.sendMSEvents`)
//! and logout (`Player.sendLogoutEvents`) publish.
//!
//! The original drains the queue on a coroutine of its own, concurrently with
//! the world; here the host decides when to [`Social::pump`] it, and the
//! order of events is the queue's. Messages the original sends to and from a
//! management server between worlds have no native form.
use super::clan::{Clan, ClanEntry};
use super::packets::{ContactKind, Queued, RECIEVE_MESSAGE, SEND_MESSAGE};
use super::{Error, Host, Result, Social, kotlin_split, rank_at};
use crate::auth::account::UserAccountInfo;
use crate::player::communication::{ClanRank, Contact, parse_clan_requirements, parse_contacts};
use rs09_game_core::java_lower::lowercase_root;
use std::hash::Hash;

/// A contact of `SendContactInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactInfo {
    pub username: String,
    pub world: i32,
    pub rank: i32,
}

/// A member of `SendClanInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberInfo {
    pub username: String,
    pub world: i32,
    pub rank: i32,
}

/// `FriendUpdate.Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendUpdateType {
    Add,
    Remove,
}

/// The `proto.management` messages the social code publishes and handles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    PlayerStatusUpdate {
        username: String,
        world: i32,
        notify_friends_only: bool,
    },
    RequestContactInfo {
        username: String,
        world: i32,
    },
    SendContactInfo {
        username: String,
        contacts: Vec<ContactInfo>,
        blocked: Vec<String>,
    },
    FriendUpdate {
        kind: FriendUpdateType,
        username: String,
        friend: String,
    },
    PrivateMessage {
        sender: String,
        receiver: String,
        message: String,
        rank: i32,
    },
    ClanMessage {
        sender: String,
        clan_name: String,
        message: String,
        rank: i32,
    },
    JoinClanRequest {
        username: String,
        clan_name: String,
    },
    LeaveClanRequest {
        username: String,
        clan_name: String,
    },
    ClanJoinNotification {
        username: String,
        clan_name: String,
        world: i32,
    },
    ClanLeaveNotification {
        username: String,
        clan_name: String,
        world: i32,
    },
    RequestClanInfo {
        world: i32,
        clan_owner: String,
    },
    SendClanInfo {
        clan_owner: String,
        has_info: bool,
        clan_name: String,
        join_requirement: i32,
        kick_requirement: i32,
        message_requirement: i32,
        loot_requirement: i32,
        members: Vec<MemberInfo>,
    },
}

fn parse_failure(error: crate::player::Error) -> Error {
    match error {
        crate::player::Error::Number => Error::NumberFormat("contact rank".into()),
        _ => Error::ArrayIndexOutOfBounds {
            index: -1,
            length: 0,
        },
    }
}

impl<P: Clone + Eq + Hash> Social<P> {
    /// `ManagementEvents.publish(event)`.
    pub fn publish(&mut self, event: Event) {
        self.queue.push_back(event);
    }

    /// The management coroutine until its queue is empty: each event is
    /// taken, handled and then logged; a failing handler skips the log, and
    /// any exception of either is swallowed.
    pub fn pump<H: Host<P>>(&mut self, host: &mut H) {
        while let Some(event) = self.queue.pop_front() {
            host.management_take(&event);
            if self.handle_event(host, &event).is_ok() {
                let _ = log_event(host, &event);
            }
        }
    }

    /// `Login.sendMSEvents(player.details)`: online status, a request for the
    /// contact lists, and a request to rejoin the saved current clan when the
    /// player holds no clan object yet.
    pub fn login_events<H: Host<P>>(&mut self, host: &mut H, player: &P) {
        let (username, current) = host.with_details(player, |details| {
            (
                crate::auth::storage::lock(&details.account)
                    .username
                    .clone(),
                details
                    .communication
                    .current_clan
                    .clone()
                    .unwrap_or_default(),
            )
        });
        let world = host.world_id();
        self.publish(Event::PlayerStatusUpdate {
            username: username.clone(),
            world,
            notify_friends_only: false,
        });
        self.publish(Event::RequestContactInfo {
            username: username.clone(),
            world: host.world_id(),
        });
        if !current.is_empty() && self.clan_of(player).is_none() {
            self.publish(Event::JoinClanRequest {
                username,
                clan_name: current,
            });
        }
    }

    /// `Player.sendLogoutEvents()`: offline status, and the clan leave when
    /// the player holds a clan.
    pub fn logout_events<H: Host<P>>(&mut self, host: &mut H, player: &P) {
        self.publish(Event::PlayerStatusUpdate {
            username: host.name(player),
            world: 0,
            notify_friends_only: false,
        });
        if let Some(clan) = self.clan_of(player) {
            let event = Event::ClanLeaveNotification {
                username: host.name(player),
                world: host.world_id(),
                clan_name: self.clans[clan.0].owner.clone(),
            };
            self.publish(event);
        }
    }

    fn should_wait_for_clan_info<H: Host<P>>(
        &mut self,
        host: &mut H,
        clan_name: &str,
    ) -> Result<bool> {
        Ok(self.clan_get(host, clan_name, false)?.is_none()
            && !self.requested.contains_key(clan_name))
    }

    fn queue_until_clan_info<H: Host<P>>(&mut self, host: &mut H, clan_name: &str, event: &Event) {
        self.waiting
            .entry(clan_name.to_owned())
            .or_default()
            .push_back(event.clone());
        if !self.requested.contains_key(clan_name) {
            self.publish(Event::RequestClanInfo {
                clan_owner: clan_name.to_owned(),
                world: host.world_id(),
            });
            self.requested.insert(clan_name.to_owned(), true);
        }
    }

    fn handle_event<H: Host<P>>(&mut self, host: &mut H, event: &Event) -> Result<()> {
        match event {
            Event::PlayerStatusUpdate {
                username,
                world,
                notify_friends_only,
            } => {
                let names = if *notify_friends_only {
                    host.online_friends(username)?
                } else {
                    host.player_names()
                };
                let mut notifiable = Vec::new();
                for name in names {
                    let has = match host.player_by_name(&name) {
                        Some(player) => host.with_details(&player, |d| {
                            d.communication.contacts.contains_key(username)
                        }),
                        None => false,
                    };
                    if has {
                        notifiable.push(name);
                    }
                }
                for name in notifiable {
                    let Some(player) = host.player_by_name(&name) else {
                        continue;
                    };
                    host.with_details(&player, |d| {
                        if let Some(contact) = d.communication.contacts.get_mut(username) {
                            contact.world_id = *world;
                        }
                    });
                    self.send_packet(
                        host,
                        Queued::Contact {
                            player,
                            kind: ContactKind::UpdateFriend,
                            name: Some(username.clone()),
                            world_id: *world,
                        },
                    );
                }
            }
            Event::RequestContactInfo { username, .. } => {
                let info = host.account_info(username)?;
                let contacts = parse_contacts(&info.contacts).map_err(parse_failure)?;
                let mut listed = Vec::new();
                for (name, contact) in contacts.iter() {
                    let online = host.player_by_name(name).is_some();
                    let rank = contact.rank.ok_or(Error::NullPointer)?.ordinal();
                    listed.push(ContactInfo {
                        username: name.to_owned(),
                        world: if online { host.world_id() } else { 0 },
                        rank,
                    });
                }
                let blocked = kotlin_split(&info.blocked, ",")
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                self.publish(Event::SendContactInfo {
                    username: username.clone(),
                    contacts: listed,
                    blocked,
                });
            }
            Event::SendContactInfo {
                username,
                contacts,
                blocked,
            } => {
                let Some(player) = host.player_by_name(username) else {
                    return Ok(());
                };
                self.send_packet(
                    host,
                    Queued::Contact {
                        player: player.clone(),
                        kind: ContactKind::UpdateState,
                        name: None,
                        world_id: 0,
                    },
                );
                host.with_details(&player, |d| {
                    d.communication.contacts.clear();
                    d.communication.blocked.clear();
                });
                for info in contacts {
                    let rank = rank_at(i64::from(info.rank));
                    host.with_details(&player, |d| {
                        let mut contact = Contact::new(Some(info.username.clone()));
                        contact.world_id = info.world;
                        if let Ok(rank) = rank {
                            contact.rank = Some(rank);
                        }
                        d.communication
                            .contacts
                            .insert(info.username.clone(), contact);
                    });
                    rank?;
                    self.send_packet(
                        host,
                        Queued::Contact {
                            player: player.clone(),
                            kind: ContactKind::UpdateFriend,
                            name: Some(info.username.clone()),
                            world_id: info.world,
                        },
                    );
                }
                host.with_details(&player, |d| {
                    d.communication
                        .blocked
                        .extend(blocked.iter().cloned().map(Some));
                });
                self.send_packet(
                    host,
                    Queued::Contact {
                        player,
                        kind: ContactKind::IgnoreList,
                        name: None,
                        world_id: 0,
                    },
                );
            }
            Event::FriendUpdate {
                username, friend, ..
            } => {
                // The original computes these and does nothing with them.
                let _ = host.player_by_name(friend);
                let _ = host.player_by_name(username);
            }
            Event::PrivateMessage {
                sender,
                receiver,
                message,
                rank,
            } => {
                let from = host.player_by_name(sender);
                let to = host.player_by_name(receiver);
                if let Some(from) = from {
                    self.send_packet(
                        host,
                        Queued::Message {
                            player: from,
                            opcode: SEND_MESSAGE,
                            other: receiver.clone(),
                            chat_icon: *rank,
                            message: message.clone(),
                        },
                    );
                }
                if let Some(to) = to {
                    self.send_packet(
                        host,
                        Queued::Message {
                            player: to,
                            opcode: RECIEVE_MESSAGE,
                            other: sender.clone(),
                            chat_icon: *rank,
                            message: message.clone(),
                        },
                    );
                }
            }
            Event::JoinClanRequest {
                username,
                clan_name,
            } => {
                let Some(player) = host.player_by_name(username) else {
                    return Ok(());
                };
                if self.should_wait_for_clan_info(host, clan_name)? {
                    self.queue_until_clan_info(host, clan_name, event);
                    return Ok(());
                }
                match self.clan_get(host, clan_name, false)? {
                    None => host.api_message(
                        &player,
                        "The channel you tried to join does not exist.:clan:",
                    )?,
                    Some(clan) => {
                        self.clan_enter(host, clan, &player)?;
                        self.set_clan(host, &player, Some(clan));
                    }
                }
            }
            Event::ClanJoinNotification {
                username,
                world,
                clan_name,
            } => {
                if *world == host.world_id() {
                    return Ok(());
                }
                if self.should_wait_for_clan_info(host, clan_name)? {
                    self.queue_until_clan_info(host, clan_name, event);
                    return Ok(());
                }
                let clan = self
                    .clan_get(host, clan_name, false)?
                    .ok_or(Error::NullPointer)?;
                self.clans[clan.0].players.push(ClanEntry {
                    name: username.clone(),
                    player: None,
                    world_id: *world,
                });
                self.clan_update(host, clan);
            }
            Event::LeaveClanRequest {
                username,
                clan_name,
            } => {
                let Some(player) = host.player_by_name(username) else {
                    return Ok(());
                };
                if self.should_wait_for_clan_info(host, clan_name)? {
                    self.queue_until_clan_info(host, clan_name, event);
                    return Ok(());
                }
                match self.clan_get(host, clan_name, false)? {
                    None => host.api_message(&player, "Error leaving clan. Please relog.")?,
                    Some(clan) => {
                        self.clan_leave_default(host, clan, &player, true)?;
                        self.set_clan(host, &player, None);
                    }
                }
            }
            Event::ClanLeaveNotification {
                username,
                clan_name,
                ..
            } => {
                if self.should_wait_for_clan_info(host, clan_name)? {
                    self.queue_until_clan_info(host, clan_name, event);
                    return Ok(());
                }
                let clan = self
                    .clan_get(host, clan_name, false)?
                    .ok_or(Error::NullPointer)?;
                let Some(index) = self.clans[clan.0]
                    .players
                    .iter()
                    .position(|e| e.name == *username)
                else {
                    return Ok(());
                };
                self.clans[clan.0].players.remove(index);
                self.clan_update(host, clan);
            }
            Event::RequestClanInfo { clan_owner, .. } => {
                let response = match self.clan_get(host, clan_owner, false)? {
                    None => Event::SendClanInfo {
                        clan_owner: clan_owner.clone(),
                        has_info: false,
                        clan_name: String::new(),
                        join_requirement: 0,
                        kick_requirement: 0,
                        message_requirement: 0,
                        loot_requirement: 0,
                        members: Vec::new(),
                    },
                    Some(clan) => {
                        let clan = &self.clans[clan.0];
                        let members = clan
                            .players
                            .iter()
                            .map(|member| MemberInfo {
                                username: member.name.clone(),
                                world: member.world_id,
                                rank: clan
                                    .ranks
                                    .get(&Some(member.name.clone()))
                                    .copied()
                                    .flatten()
                                    .unwrap_or(ClanRank::None)
                                    .ordinal(),
                            })
                            .collect();
                        Event::SendClanInfo {
                            clan_owner: clan_owner.clone(),
                            has_info: true,
                            clan_name: clan.name.clone(),
                            join_requirement: 0,
                            kick_requirement: 0,
                            message_requirement: 0,
                            loot_requirement: 0,
                            members,
                        }
                    }
                };
                self.publish(response);
            }
            Event::SendClanInfo {
                has_info,
                clan_owner,
                ..
            } => {
                if *has_info {
                    self.initialize_clan_from(host, event)?;
                } else {
                    let mut info = host.account_info(clan_owner)?;
                    if !info.clan_name.is_empty() {
                        self.initialize_clan_with(host, &info)?;
                    } else {
                        host.log_ms("Creating default server clan")?;
                        let server = lowercase_root(&host.server_name());
                        if host.enable_default_clan() && *clan_owner == server {
                            if info == UserAccountInfo::create_default(host.current_time_millis()) {
                                info.username = server.clone();
                                info.password = host.ms_secret_key();
                                info.rights = 2;
                                host.create_account(&info)?;
                                info = host.account_info(clan_owner)?;
                            }
                            info.clan_name = "Global".into();
                            info.clan_reqs = "-1,-1,7,7".into();
                            host.account_update(&info)?;
                            self.initialize_clan_with(host, &info)?;
                        }
                    }
                }
                let Some(queued) = self.waiting.get_mut(clan_owner) else {
                    return Ok(());
                };
                let pending: Vec<Event> = queued.drain(..).collect();
                for event in pending {
                    self.publish(event);
                }
            }
            Event::ClanMessage {
                sender,
                clan_name,
                message,
                rank,
            } => {
                if self.should_wait_for_clan_info(host, clan_name)? {
                    self.queue_until_clan_info(host, clan_name, event);
                    return Ok(());
                }
                let clan = self
                    .clan_get(host, clan_name, false)?
                    .ok_or(Error::NullPointer)?;
                let members: Vec<P> = self.clans[clan.0]
                    .players
                    .iter()
                    .filter_map(|m| m.player.clone())
                    .collect();
                for member in members {
                    self.send_packet(
                        host,
                        Queued::Message {
                            player: member,
                            opcode: super::packets::CLAN_MESSAGE,
                            other: sender.clone(),
                            chat_icon: *rank,
                            message: message.clone(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    /// `initializeClanFrom(event)`: the registered clan (or a new one) takes
    /// the name, the four requirements and the members; members on this
    /// world are bound to their players, who take the clan.
    fn initialize_clan_from<H: Host<P>>(&mut self, host: &mut H, event: &Event) -> Result<()> {
        let Event::SendClanInfo {
            clan_owner,
            clan_name,
            join_requirement,
            kick_requirement,
            message_requirement,
            loot_requirement,
            members,
            ..
        } = event
        else {
            unreachable!("only SendClanInfo initializes a clan");
        };
        let clan = match self.registry.get(clan_owner) {
            Some(&clan) => clan,
            None => {
                let clan = self.add_clan(Clan::new(clan_owner.clone()));
                self.registry.insert(clan_owner.clone(), clan);
                clan
            }
        };
        // Kotlin property assignments call the Java setters: the join
        // requirement cleans the channel and the kick requirement updates it.
        self.clan_set_name(clan, clan_name.clone());
        let join = rank_at(i64::from(*join_requirement))?;
        self.clan_set_join_requirement(host, clan, Some(join))?;
        let kick = rank_at(i64::from(*kick_requirement))?;
        self.clan_set_kick_requirement(host, clan, Some(kick));
        let message = rank_at(i64::from(*message_requirement))?;
        self.clan_set_message_requirement(clan, Some(message));
        let loot = rank_at(i64::from(*loot_requirement))?;
        self.clan_set_loot_requirement(clan, Some(loot));
        for member in members {
            let mut entry = ClanEntry {
                name: member.username.clone(),
                player: None,
                world_id: member.world,
            };
            let rank = rank_at(i64::from(member.rank))?;
            self.clans[clan.0]
                .ranks
                .insert(Some(member.username.clone()), Some(rank));
            if member.world == host.world_id() {
                let player = host.player_by_name(&member.username);
                entry.player = player.clone();
                if let Some(player) = player {
                    self.set_clan(host, &player, Some(clan));
                }
            }
            self.clans[clan.0].players.push(entry);
        }
        self.clan_update(host, clan);
        Ok(())
    }

    /// `initializeClanWith(info)`: a new clan from a saved account, replacing
    /// any registered one under that owner.
    fn initialize_clan_with<H: Host<P>>(
        &mut self,
        host: &mut H,
        info: &UserAccountInfo,
    ) -> Result<()> {
        let requirements = parse_clan_requirements(&info.clan_reqs).map_err(parse_failure)?;
        let id = self.add_clan(Clan::new(info.username.clone()));
        let contacts = parse_contacts(&info.contacts).map_err(parse_failure)?;
        // As in initializeClanFrom, these assignments are the Java setters.
        self.clan_set_name(id, info.clan_name.clone());
        self.clan_set_join_requirement(host, id, requirements[0])?;
        self.clan_set_message_requirement(id, requirements[1]);
        self.clan_set_kick_requirement(host, id, requirements[2]);
        self.clan_set_loot_requirement(id, requirements[3]);
        for (name, contact) in contacts.iter() {
            self.clans[id.0]
                .ranks
                .insert(Some(name.to_owned()), contact.rank);
        }
        self.registry.insert(info.username.clone(), id);
        Ok(())
    }

    /// The pulse body of the loot-share timer: loot share on, the pulse
    /// forgotten, varp 1083 set to 1; it finishes.
    pub fn run_pulse<H: Host<P>>(&mut self, host: &mut H, pulse: super::PulseId) -> Result<bool> {
        let player = self.pulses[pulse.0].clone();
        host.with_details(&player, |d| d.communication.loot_share = true);
        self.links
            .entry(player.clone())
            .or_default()
            .loot_share_pulse = None;
        host.varp(&player, 1083, 1, false)?;
        Ok(true)
    }

    /// The pulses this library submitted, by id.
    pub fn pulse_owner(&self, pulse: super::PulseId) -> &P {
        &self.pulses[pulse.0]
    }
}

/// `handleLoggingFor(event)`.
fn log_event<P, H: Host<P>>(host: &mut H, event: &Event) -> Result<()> {
    let text = match event {
        Event::PlayerStatusUpdate {
            username, world, ..
        } => format!("{username} -(WLD)> {world}"),
        Event::RequestContactInfo { username, .. } => format!("{username} -> RQ CONTACT INFO"),
        Event::SendContactInfo { username, .. } => format!("{username} <- SND CONTACT INFO"),
        Event::PrivateMessage {
            sender,
            receiver,
            message,
            ..
        } => format!("[PM] {sender}->{receiver}: {message}"),
        Event::ClanMessage {
            sender,
            clan_name,
            message,
            ..
        } => format!("[CM:{clan_name}] {sender}: {message}"),
        Event::JoinClanRequest {
            username,
            clan_name,
        } => format!("{username} +CL {clan_name}"),
        Event::LeaveClanRequest {
            username,
            clan_name,
        } => format!("{username} -CL {clan_name}"),
        Event::RequestClanInfo { clan_owner, .. } => format!("REQUEST CLAN INFO: {clan_owner}"),
        Event::SendClanInfo {
            clan_owner,
            clan_name,
            ..
        } => format!("RECEIVE CLAN INFO: {clan_owner}->{clan_name}"),
        Event::ClanJoinNotification {
            username,
            clan_name,
            ..
        } => format!("{username} JOINED CLAN {clan_name}"),
        Event::ClanLeaveNotification {
            username,
            clan_name,
            ..
        } => format!("{username} LEFT CLAN {clan_name}"),
        Event::FriendUpdate { .. } => return Ok(()),
    };
    host.log_ms(&text)
}
