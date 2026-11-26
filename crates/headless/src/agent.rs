//! Agent-facing actions and a world view reduced from authoritative frames.
//!
//! [`Action`] names what an agent wants to do (attack or talk to an NPC, use an
//! item on an item, scenery, NPC or player, continue or answer a dialogue) and
//! builds the original client bytes through the writers the visual client also
//! uses (`protocol::game::menu`, `location_packet`, `worldspace_walk`).
//! [`View`] folds received frames into what an agent reads back: its position,
//! skills, inventory and equipment, the NPCs around it with the hits and health
//! bars the server showed, its own hits, game messages and the open dialogue.
//! [`Agent`] couples a managed session with a view.
//!
//! Scope: these are the original client's packets and the server's own update
//! stream. Sending an action proves only that it was published; its outcome is
//! whatever the server sends next. The view holds no cache definitions: NPC
//! names, option labels and item names are not known here. [`Action::Attack`]
//! and [`Action::TalkTo`] are the definition's conventional slots (Talk-to is
//! the first option, Attack the second, as on ordinary monsters and humans);
//! for any other NPC use [`Action::NpcOption`] with the slot the cache shows.
use crate::session::{ManagedSession, SceneryAction};
use rs09_protocol::game::{
    Frame, actions,
    menu::{self, UsedItem},
    npcs, players, rebuild,
    updates::{ServerUpdate, WireItem},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    time::Duration,
};
use tokio::time::{Instant, timeout_at};

/// The inventory's component (interface 149, child 0) and container ids.
pub const INVENTORY_COMPONENT: i32 = 149 << 16;
pub const INVENTORY: i32 = 93;
pub const EQUIPMENT: i32 = 94;
const EVENT_LIMIT: usize = 512;

/// An inventory item named by id and slot, as the original Use cursor holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub id: i32,
    pub slot: i32,
}
impl Held {
    fn used(self) -> UsedItem {
        UsedItem {
            id: self.id,
            slot: self.slot,
            component: INVENTORY_COMPONENT,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Walk {
        tile: [i32; 2],
        running: bool,
    },
    Scenery(SceneryAction),
    /// The NPC definition's option at zero-based slot 0..4 on a local NPC index.
    NpcOption {
        option: u8,
        index: i32,
    },
    /// Option slot 1 ("Attack" on ordinary monsters).
    Attack {
        index: i32,
    },
    /// Option slot 0 ("Talk-to" on ordinary humans).
    TalkTo {
        index: i32,
    },
    /// An inventory item's option at zero-based slot 0..4 (Eat/Wield, ..., Drop).
    ItemOption {
        option: u8,
        item: Held,
    },
    UseOnItem {
        used: Held,
        target: Held,
    },
    UseOnNpc {
        used: Held,
        index: i32,
    },
    UseOnScenery {
        used: Held,
        id: i32,
        tile: [i32; 2],
    },
    UseOnPlayer {
        used: Held,
        index: i32,
    },
    /// Continue, or a dialogue option, as a chatbox component (`iface << 16 | child`).
    Resume {
        component: i32,
    },
    CloseInterface,
}
impl Action {
    pub fn frame(&self) -> io::Result<Frame> {
        let invalid = |what: &str| io::Error::new(io::ErrorKind::InvalidInput, what.to_owned());
        Ok(match *self {
            Self::Walk { tile, running } => actions::worldspace_walk(tile[0], tile[1], running),
            Self::Scenery(action) => action.frame()?,
            Self::NpcOption { option, index } => {
                menu::npc_option(option, index).ok_or_else(|| invalid("NPC option must be 0..4"))?
            }
            Self::Attack { index } => menu::npc_option(1, index).expect("slot 1"),
            Self::TalkTo { index } => menu::npc_option(0, index).expect("slot 0"),
            Self::ItemOption { option, item } => {
                let code = *[47, 5, 43, 35, 58]
                    .get(usize::from(option))
                    .ok_or_else(|| invalid("item option must be 0..4"))?;
                menu::item_packet(code, item.id, item.slot, INVENTORY_COMPONENT)
                    .expect("original item option writer")
            }
            Self::UseOnItem { used, target } => {
                used.used()
                    .on_item(target.id, target.slot, INVENTORY_COMPONENT)
            }
            Self::UseOnNpc { used, index } => used.used().on_npc(index),
            Self::UseOnScenery { used, id, tile } => used.used().on_location(id, tile),
            Self::UseOnPlayer { used, index } => used.used().on_player(index),
            Self::Resume { component } => menu::resume_pause_button(component, -1),
            Self::CloseInterface => menu::close_modal(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogueKind {
    /// Chatbox 241..244 (248 with Continue hidden): an NPC speaks.
    Npc { npc: Option<i32> },
    /// Chatbox 64..67 (71 with Continue hidden): the player speaks.
    Player,
    /// Chatbox 210..213: a plain message.
    Plain,
    /// Chatbox 228, 230, 232, 234: two to five options under a title.
    Options,
}

/// The dialogue in the chatbox as the server drew it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dialogue {
    pub interface: i32,
    pub kind: DialogueKind,
    /// The speaker's name (NPC or player dialogue) or the options' title.
    pub title: Option<String>,
    /// Spoken lines, or the options in order.
    pub lines: Vec<String>,
    continue_child: Option<i32>,
}
impl Dialogue {
    /// Continue, when the chatbox shows it.
    pub fn resume(&self) -> Option<Action> {
        self.continue_child.map(|child| Action::Resume {
            component: self.interface << 16 | child,
        })
    }
    /// The zero-based option of an options dialogue.
    pub fn choose(&self, option: usize) -> Option<Action> {
        (self.kind == DialogueKind::Options && option < self.lines.len()).then(|| Action::Resume {
            component: self.interface << 16 | (option as i32 + 2),
        })
    }
    /// The first option containing `text` (ASCII case-insensitive).
    pub fn choose_text(&self, text: &str) -> Option<Action> {
        let text = text.to_ascii_lowercase();
        let i = self
            .lines
            .iter()
            .position(|l| l.to_ascii_lowercase().contains(&text))?;
        self.choose(i)
    }
    fn json(&self) -> Value {
        let kind = match self.kind {
            DialogueKind::Npc { .. } => "npc",
            DialogueKind::Player => "player",
            DialogueKind::Plain => "plain",
            DialogueKind::Options => "options",
        };
        let npc = match self.kind {
            DialogueKind::Npc { npc } => npc,
            _ => None,
        };
        json!({"interface":self.interface,"kind":kind,"npc":npc,"title":self.title,"lines":self.lines,"canContinue":self.continue_child.is_some()})
    }
}

/// Layout of the chatbox interfaces the pinned DialogueInterpreter opens:
/// (kind, title child, first line child, line count, Continue child).
type Layout = (u8, Option<i32>, i32, i32, Option<i32>);
fn layout(interface: i32) -> Option<Layout> {
    Some(match interface {
        241..=244 => (0, Some(3), 4, interface - 240, Some(interface - 240 + 4)),
        245..=248 => (0, Some(3), 4, interface - 244, None),
        64..=67 => (1, Some(3), 4, interface - 63, Some(interface - 63 + 4)),
        68..=71 => (1, Some(3), 4, interface - 67, None),
        210..=213 => (2, None, 1, interface - 209, Some(interface - 209 + 1)),
        228 | 230 | 232 | 234 => (3, Some(1), 2, (interface - 224) / 2, None),
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skill {
    pub level: i32,
    pub experience: i32,
}

/// One local NPC. `tile` is absolute; `health` is the last health bar ratio the
/// server drew (0..255 of the bar), `None` until the NPC is first hit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcView {
    pub index: i32,
    pub type_id: i32,
    pub tile: [i32; 3],
    pub health: Option<i32>,
    pub interacting: Option<i32>,
}

/// What changed, in the order the server's frames said it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Message(String),
    NpcHit {
        index: i32,
        type_id: i32,
        damage: i32,
        kind: i32,
        health: Option<i32>,
    },
    /// A local NPC left the list (death, despawn or out of view) after it had
    /// been drawn with an empty health bar (`died`) or not.
    NpcRemoved {
        index: i32,
        type_id: i32,
        died: bool,
    },
    SelfHit {
        damage: i32,
        kind: i32,
        health: Option<i32>,
    },
    Experience {
        skill: i32,
        gained: i32,
        experience: i32,
    },
    Level {
        skill: i32,
        level: i32,
    },
    ItemSlot {
        container: i32,
        slot: i32,
        item: Option<WireItem>,
    },
    DialogueOpened(i32),
    DialogueClosed(i32),
    InterfaceOpened(i32),
}
impl Event {
    pub fn json(&self) -> Value {
        match self {
            Self::Message(m) => json!({"event":"message","text":m}),
            Self::NpcHit {
                index,
                type_id,
                damage,
                kind,
                health,
            } => {
                json!({"event":"npcHit","index":index,"type":type_id,"damage":damage,"kind":kind,"health":health})
            }
            Self::NpcRemoved {
                index,
                type_id,
                died,
            } => json!({"event":"npcRemoved","index":index,"type":type_id,"died":died}),
            Self::SelfHit {
                damage,
                kind,
                health,
            } => json!({"event":"selfHit","damage":damage,"kind":kind,"health":health}),
            Self::Experience {
                skill,
                gained,
                experience,
            } => {
                json!({"event":"experience","skill":skill,"gained":gained,"experience":experience})
            }
            Self::Level { skill, level } => json!({"event":"level","skill":skill,"level":level}),
            Self::ItemSlot {
                container,
                slot,
                item,
            } => {
                json!({"event":"itemSlot","container":container,"slot":slot,"item":item.map(|i|json!({"id":i.id,"amount":i.amount}))})
            }
            Self::DialogueOpened(i) => json!({"event":"dialogueOpened","interface":i}),
            Self::DialogueClosed(i) => json!({"event":"dialogueClosed","interface":i}),
            Self::InterfaceOpened(i) => json!({"event":"interfaceOpened","interface":i}),
        }
    }
}

fn text(bytes: &[u8]) -> String {
    // Game strings are single-byte; map them as Latin-1 rather than fail on UTF-8.
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// The agent's world, reduced from the authoritative frames it is given.
pub struct View {
    base: Option<[i32; 2]>,
    players: players::State,
    npcs: npcs::State,
    npc_health: BTreeMap<usize, i32>,
    updates: i32,
    skills: [Option<Skill>; 25],
    containers: BTreeMap<i32, Vec<Option<WireItem>>>,
    /// Open interfaces by the component they sit in.
    open: BTreeMap<i32, i32>,
    texts: BTreeMap<(i32, i32), String>,
    heads: BTreeMap<i32, i32>,
    run_energy: Option<i32>,
    events: VecDeque<Event>,
    omitted: usize,
}
impl Default for View {
    fn default() -> Self {
        Self::new()
    }
}
impl View {
    pub fn new() -> Self {
        Self {
            base: None,
            players: players::State::new(0, 0),
            npcs: npcs::State::default(),
            npc_health: BTreeMap::new(),
            updates: 0,
            skills: [None; 25],
            containers: BTreeMap::new(),
            open: BTreeMap::new(),
            texts: BTreeMap::new(),
            heads: BTreeMap::new(),
            run_energy: None,
            events: VecDeque::new(),
            omitted: 0,
        }
    }
    fn push(&mut self, event: Event) {
        self.events.push_back(event);
        if self.events.len() > EVENT_LIMIT {
            self.events.pop_front();
            self.omitted += 1;
        }
    }
    /// Events since the last drain, oldest first, and how many were dropped
    /// because more than 512 were held.
    pub fn drain_events(&mut self) -> (Vec<Event>, usize) {
        (
            self.events.drain(..).collect(),
            std::mem::take(&mut self.omitted),
        )
    }
    pub fn events(&self) -> impl Iterator<Item = &Event> {
        self.events.iter()
    }
    /// Player updates applied so far (one per server tick once in the world).
    pub fn player_updates(&self) -> i32 {
        self.updates
    }
    fn self_tile(&self) -> Option<[i32; 2]> {
        let [x, y] = self.base?;
        let p = self.players.self_player();
        Some([x + p.queue_x[0], y + p.queue_y[0]])
    }
    /// The absolute tile at the head of the player's walking queue, once a
    /// scene rebuild and a player update have arrived.
    pub fn position(&self) -> Option<[i32; 3]> {
        (self.updates > 0)
            .then(|| self.self_tile())
            .flatten()
            .map(|[x, y]| [x, y, self.players.plane])
    }
    pub fn skill(&self, skill: usize) -> Option<Skill> {
        self.skills.get(skill).copied().flatten()
    }
    /// Current (boosted) hitpoints, skill 3.
    pub fn hitpoints(&self) -> Option<i32> {
        self.skill(3).map(|s| s.level)
    }
    pub fn run_energy(&self) -> Option<i32> {
        self.run_energy
    }
    pub fn container(&self, id: i32) -> &[Option<WireItem>] {
        self.containers.get(&id).map_or(&[], Vec::as_slice)
    }
    pub fn inventory(&self) -> &[Option<WireItem>] {
        self.container(INVENTORY)
    }
    /// The first inventory slot holding `id`.
    pub fn held(&self, id: i32) -> Option<Held> {
        self.inventory()
            .iter()
            .position(|i| i.is_some_and(|i| i.id == id))
            .map(|slot| Held {
                id,
                slot: slot as i32,
            })
    }
    pub fn npcs(&self) -> Vec<NpcView> {
        let plane = self.players.plane;
        self.npcs
            .local_ids
            .iter()
            .filter_map(|&i| {
                let n = self.npcs.actors.get(&i)?;
                Some(NpcView {
                    index: i as i32,
                    type_id: n.type_id,
                    tile: [n.tile[0], n.tile[1], plane],
                    health: self.npc_health.get(&i).copied(),
                    interacting: n.last_mask.face_entity.filter(|e| *e >= 0),
                })
            })
            .collect()
    }
    pub fn npc(&self, index: i32) -> Option<NpcView> {
        self.npcs().into_iter().find(|n| n.index == index)
    }
    /// The nearest local NPC of a definition, by Chebyshev distance.
    pub fn nearest_npc(&self, type_id: i32) -> Option<NpcView> {
        let [x, y] = self.self_tile()?;
        self.npcs()
            .into_iter()
            .filter(|n| n.type_id == type_id)
            .min_by_key(|n| ((n.tile[0] - x).abs().max((n.tile[1] - y).abs()), n.index))
    }
    /// The dialogue the chatbox shows now, if any.
    pub fn dialogue(&self) -> Option<Dialogue> {
        self.open.values().find_map(|&interface| {
            let (kind, title, first, count, resume) = layout(interface)?;
            let line = |child| self.texts.get(&(interface, child)).cloned();
            Some(Dialogue {
                interface,
                kind: match kind {
                    0 => DialogueKind::Npc {
                        npc: self.heads.get(&interface).copied(),
                    },
                    1 => DialogueKind::Player,
                    2 => DialogueKind::Plain,
                    _ => DialogueKind::Options,
                },
                title: title.and_then(line),
                lines: (first..first + count)
                    .map(|c| line(c).unwrap_or_default())
                    .collect(),
                continue_child: resume,
            })
        })
    }
    /// Interfaces open now, by the component that holds them.
    pub fn open_interfaces(&self) -> impl Iterator<Item = i32> + '_ {
        self.open.values().copied()
    }

    /// Fold one frame from the server into the view.
    pub fn apply(&mut self, frame: &Frame) -> io::Result<()> {
        match frame.opcode {
            162 => {
                let next = rebuild::decode_normal(frame).map_err(io::Error::other)?;
                let new = next.base();
                if let Some(old) = self.base {
                    self.players.rebase([new[0] - old[0], new[1] - old[1]]);
                }
                self.base = Some(new);
                self.players.plane = next.plane;
                self.players
                    .self_player_mut()
                    .teleport(next.local_x, next.local_y, false);
            }
            225 => {
                self.updates += 1;
                let update = self
                    .players
                    .decode_frame(frame, self.updates)
                    .map_err(io::Error::other)?;
                for extended in update.extended.iter().filter(|e| e.index == players::SELF) {
                    for field in &extended.fields {
                        if let players::Field::Hit { damage, kind, bar } = *field {
                            self.push(Event::SelfHit {
                                damage,
                                kind,
                                health: bar,
                            });
                        }
                    }
                }
            }
            32 => {
                let at = self
                    .self_tile()
                    .ok_or_else(|| io::Error::other("NPC update before scene rebuild"))?;
                let before: BTreeMap<usize, i32> = self
                    .npcs
                    .local_ids
                    .iter()
                    .filter_map(|i| self.npcs.actors.get(i).map(|n| (*i, n.type_id)))
                    .collect();
                let cycle = self.updates;
                self.npcs
                    .decode(frame, at, cycle)
                    .map_err(io::Error::other)?;
                let mut events = vec![];
                for &i in &self.npcs.local_ids {
                    let n = &self.npcs.actors[&i];
                    if n.last_seen != cycle {
                        continue;
                    }
                    // A mask is current only in the update that carried it.
                    for hit in [&n.last_mask.primary, &n.last_mask.secondary]
                        .into_iter()
                        .flatten()
                    {
                        if let Some(r) = hit.ratio {
                            self.npc_health.insert(i, r);
                        }
                        events.push(Event::NpcHit {
                            index: i as i32,
                            type_id: n.type_id,
                            damage: hit.damage,
                            kind: hit.kind,
                            health: self.npc_health.get(&i).copied(),
                        });
                    }
                }
                // Clear consumed masks so an unmasked later update is not re-read.
                for &i in &self.npcs.local_ids {
                    if let Some(n) = self.npcs.actors.get_mut(&i) {
                        n.last_mask.primary = None;
                        n.last_mask.secondary = None;
                    }
                }
                for (i, type_id) in before {
                    if !self.npcs.local_ids.contains(&i) {
                        let died = self.npc_health.remove(&i) == Some(0);
                        events.push(Event::NpcRemoved {
                            index: i as i32,
                            type_id,
                            died,
                        });
                    }
                }
                for e in events {
                    self.push(e);
                }
            }
            _ => match ServerUpdate::decode(frame).map_err(io::Error::other)? {
                Some(ServerUpdate::GameMessage(m)) => self.push(Event::Message(text(&m))),
                Some(ServerUpdate::Skill {
                    skill,
                    boosted,
                    experience,
                }) => {
                    let slot = self
                        .skills
                        .get_mut(skill as usize)
                        .ok_or_else(|| io::Error::other("skill index out of range"))?;
                    let old = slot.replace(Skill {
                        level: boosted,
                        experience,
                    });
                    if let Some(old) = old {
                        if experience != old.experience {
                            self.push(Event::Experience {
                                skill,
                                gained: experience - old.experience,
                                experience,
                            });
                        }
                        if boosted != old.level {
                            self.push(Event::Level {
                                skill,
                                level: boosted,
                            });
                        }
                    }
                }
                Some(ServerUpdate::RunEnergy { energy }) => self.run_energy = Some(energy),
                Some(ServerUpdate::InventoryFull {
                    container, items, ..
                }) => {
                    let items: Vec<_> = items
                        .into_iter()
                        .map(|i| (i.id >= 0 && i.amount > 0).then_some(i))
                        .collect();
                    let old = self.containers.insert(container, items.clone());
                    for (slot, item) in items.iter().enumerate() {
                        if old.as_ref().and_then(|o| o.get(slot).copied()).flatten() != *item {
                            self.push(Event::ItemSlot {
                                container,
                                slot: slot as i32,
                                item: *item,
                            });
                        }
                    }
                }
                Some(ServerUpdate::InventoryPartial {
                    container, slots, ..
                }) => {
                    for s in slots {
                        let slot = usize::try_from(s.slot)
                            .ok()
                            .filter(|s| *s < 600)
                            .ok_or_else(|| io::Error::other("container slot out of range"))?;
                        let items = self.containers.entry(container).or_default();
                        if items.len() <= slot {
                            items.resize(slot + 1, None);
                        }
                        let item = (s.item.id >= 0 && s.item.amount > 0).then_some(s.item);
                        if items[slot] != item {
                            items[slot] = item;
                            self.push(Event::ItemSlot {
                                container,
                                slot: s.slot,
                                item,
                            });
                        }
                    }
                }
                Some(ServerUpdate::InventoryClear { pointer }) => {
                    // A cleared component container: the inventory and equipment are
                    // identified by their container ids in full/partial updates.
                    let _ = pointer;
                }
                Some(ServerUpdate::OpenInterface {
                    pointer, interface, ..
                }) => {
                    if let Some(old) = self.open.insert(pointer, interface) {
                        if layout(old).is_some() && old != interface {
                            self.push(Event::DialogueClosed(old));
                        }
                    }
                    self.push(if layout(interface).is_some() {
                        Event::DialogueOpened(interface)
                    } else {
                        Event::InterfaceOpened(interface)
                    });
                }
                Some(ServerUpdate::CloseInterface { pointer, .. }) => {
                    if let Some(old) = self.open.remove(&pointer) {
                        if layout(old).is_some() {
                            self.push(Event::DialogueClosed(old));
                        }
                    }
                }
                Some(ServerUpdate::InterfaceText {
                    pointer, text: t, ..
                }) => {
                    self.texts
                        .insert((pointer >> 16, pointer & 0xffff), text(&t));
                }
                Some(ServerUpdate::InterfaceNpcHead { pointer, npc, .. }) => {
                    self.heads.insert(pointer >> 16, npc);
                }
                _ => {}
            },
        }
        Ok(())
    }

    /// A JSON snapshot for agents that read state as data.
    pub fn snapshot(&self) -> Value {
        let items = |id| {
            self.container(id)
                .iter()
                .enumerate()
                .filter_map(|(slot, i)| i.map(|i| json!({"slot":slot,"id":i.id,"amount":i.amount})))
                .collect::<Vec<_>>()
        };
        json!({
            "position": self.position(),
            "playerUpdates": self.updates,
            "hitpoints": self.hitpoints(),
            "runEnergy": self.run_energy,
            "skills": self.skills.iter().map(|s| s.map(|s| json!({"level":s.level,"experience":s.experience}))).collect::<Vec<_>>(),
            "inventory": items(INVENTORY),
            "equipment": items(EQUIPMENT),
            "npcs": self.npcs().iter().map(|n| json!({"index":n.index,"type":n.type_id,"location":n.tile,"health":n.health,"interacting":n.interacting})).collect::<Vec<_>>(),
            "dialogue": self.dialogue().map(|d| d.json()),
            "interfaces": self.open.values().collect::<Vec<_>>(),
            "pendingEvents": self.events.iter().map(Event::json).collect::<Vec<_>>(),
            "eventsOmitted": self.omitted,
        })
    }
}

/// A managed session with its view: every frame received is applied before
/// it is returned, and actions go out through the session's command queue.
pub struct Agent {
    pub session: ManagedSession,
    pub view: View,
}
impl Agent {
    pub fn new(session: ManagedSession) -> Self {
        Self {
            session,
            view: View::new(),
        }
    }
    pub async fn act(&self, action: &Action) -> io::Result<Vec<u8>> {
        self.session.sender().act(action).await
    }
    pub async fn next_frame(&mut self) -> io::Result<Frame> {
        let frame = self.session.next_frame().await?;
        self.view.apply(&frame)?;
        Ok(frame)
    }
    /// Apply frames until `done` holds, or fail with `what` after `limit`.
    pub async fn until(
        &mut self,
        limit: Duration,
        what: &str,
        mut done: impl FnMut(&View) -> bool,
    ) -> io::Result<()> {
        let deadline = Instant::now() + limit;
        while !done(&self.view) {
            timeout_at(deadline, self.next_frame())
                .await
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::TimedOut, format!("waiting for {what}"))
                })??;
        }
        Ok(())
    }
    pub async fn close(self) -> io::Result<()> {
        self.session.close().await
    }
}

#[cfg(test)]
mod tests;
