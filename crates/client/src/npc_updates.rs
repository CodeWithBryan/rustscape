//! Original NPC_INFO (32) local membership, commands and complete mask reads.
//! Cache lookup, ambient emitters, hit/chat graphics and particles are bindings.
use crate::{
    actor_animation::{Sound, State as Animation},
    animation::Sequence,
    walking::Movement,
};
use rs09_protocol::{
    Error, Result,
    buffer::Reader,
    game::{Frame, npcs as wire},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct Metadata {
    pub size: i32,
    pub bas: i32,
    pub turn_speed: i32,
    pub rotates: bool,
    pub area_sound: bool,
}
#[derive(Clone, Debug)]
pub struct Actor {
    pub type_id: i32,
    pub metadata: Metadata,
    pub last_seen: i32,
    pub motion: Movement,
    pub animation: Animation,
    pub chat: Vec<u8>,
    pub chat_loops: i32,
    pub hit_bar: i32,
    pub hit_bar_until: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Type {
        index: usize,
        id: Option<i32>,
    },
    AreaRemove(usize),
    AreaAdd {
        index: usize,
        tile: [i32; 2],
    },
    Hit {
        index: usize,
        damage: i32,
        kind: i32,
        cycle: i32,
    },
    KillingBlow {
        definition: i32,
        tile: [i32; 2],
    },
    Sound {
        index: usize,
        sound: Sound,
    },
}
#[derive(Default, Debug)]
pub struct Update {
    pub events: Vec<Event>,
    pub removed_candidates: Vec<usize>,
    pub removed: Vec<usize>,
    pub masks: Vec<[i32; 2]>,
    pub diagnostics: Vec<String>,
}
#[derive(Default)]
pub struct State {
    pub actors: BTreeMap<usize, Actor>,
    pub local_ids: Vec<usize>,
}
pub struct Context<'a> {
    pub cycle: i32,
    /// World queue-head tile of PlayerList.self, including scene origin.
    pub self_tile: [i32; 2],
    pub origin: [i32; 2],
    pub sequences: &'a BTreeMap<u32, Sequence>,
    pub spots: &'a BTreeMap<i32, i32>,
}
fn sounds(out: &mut Update, index: usize, result: Result<Vec<Sound>>) {
    match result {
        Ok(events) => out.events.extend(
            events
                .into_iter()
                .map(|sound| Event::Sound { index, sound }),
        ),
        Err(error) => out.diagnostics.push(error.to_string()),
    }
}
impl State {
    pub fn decode(
        &mut self,
        frame: &Frame,
        ctx: Context<'_>,
        mut lookup: impl FnMut(i32) -> Result<Metadata>,
    ) -> Result<Update> {
        if frame.opcode != 32 {
            return Err(Error::new(0, "not NPC_INFO"));
        }
        let mut r = Reader::new(&frame.payload);
        let mut out = Update::default();
        let mut extended = Vec::new();
        let mut bits = r.bits();
        let count = bits.read(8)? as usize;
        if count > self.local_ids.len() {
            return Err(Error::new(0, "NPC local count exceeds previous list"));
        }
        out.removed_candidates
            .extend_from_slice(&self.local_ids[count..]);
        let old = std::mem::take(&mut self.local_ids);
        for &index in &old[..count] {
            let npc = self
                .actors
                .get_mut(&index)
                .ok_or(Error::new(0, "missing local NPC"))?;
            let command = wire::local(&mut bits)?;
            if command == wire::Local::Remove {
                out.removed_candidates.push(index);
                continue;
            }
            self.local_ids.push(index);
            npc.last_seen = ctx.cycle;
            match command {
                wire::Local::Mask => extended.push(index),
                wire::Local::Steps {
                    directions,
                    count,
                    speed,
                    masked,
                } => {
                    for direction in directions.into_iter().take(count as usize) {
                        npc.motion
                            .cancel_on_move(&mut npc.animation, ctx.sequences)?;
                        let delta = [
                            [-1, 1],
                            [0, 1],
                            [1, 1],
                            [-1, 0],
                            [1, 0],
                            [-1, -1],
                            [0, -1],
                            [1, -1],
                        ][direction];
                        npc.motion.enqueue(
                            [npc.motion.head[0] + delta[0], npc.motion.head[1] + delta[1]],
                            speed,
                        );
                    }
                    if masked {
                        extended.push(index);
                    }
                }
                _ => {}
            }
        }
        while let Some(addition) = wire::addition(&mut bits)? {
            let index = addition.index;
            let is_new = !self.actors.contains_key(&index);
            let npc = self.actors.entry(index).or_insert_with(|| {
                let mut motion = Movement::new(ctx.origin);
                motion.fine = ctx.origin.map(|n| n * 128);
                Actor {
                    type_id: -1,
                    metadata: Metadata {
                        size: 1,
                        bas: -1,
                        turn_speed: 32,
                        rotates: true,
                        area_sound: false,
                    },
                    last_seen: 0,
                    motion,
                    animation: Animation::default(),
                    chat: Vec::new(),
                    chat_loops: 100,
                    hit_bar: 0,
                    hit_bar_until: -1000,
                }
            });
            self.local_ids.push(index);
            npc.last_seen = ctx.cycle;
            if npc.metadata.area_sound {
                out.events.push(Event::AreaRemove(index));
            }
            let discard = addition.discard;
            let angle = [768, 1024, 1280, 512, 1536, 256, 0, 1792][addition.direction];
            if is_new {
                npc.motion.angle = angle;
                npc.motion.target_angle = angle;
            }
            if addition.masked {
                extended.push(index);
            }
            let dy = addition.offset[1];
            let id = addition.type_id;
            let dx = addition.offset[0];
            npc.type_id = id;
            npc.metadata = lookup(id)?;
            npc.motion.rotates_on_move = npc.metadata.rotates;
            out.events.push(Event::Type {
                index,
                id: Some(id),
            });
            npc.motion.turn_speed = npc.metadata.turn_speed;
            if npc.motion.turn_speed == 0 {
                npc.motion.angle = 0;
            }
            npc.motion
                .cancel_on_move(&mut npc.animation, ctx.sequences)?;
            npc.motion.teleport_actor(
                [ctx.self_tile[0] + dx, ctx.self_tile[1] + dy],
                discard,
                npc.metadata.size,
                &mut npc.animation,
            );
            if npc.metadata.area_sound {
                out.events.push(Event::AreaAdd {
                    index,
                    tile: npc.motion.head,
                });
            }
        }
        bits.finish();
        for index in extended {
            let npc = self
                .actors
                .get_mut(&index)
                .ok_or(Error::new(r.position(), "missing extended NPC"))?;
            let mask = wire::mask(&mut r)?;
            out.masks.push([index as i32, mask.flags]);
            if let Some(hit) = mask.primary {
                out.events.push(Event::Hit {
                    index,
                    damage: hit.damage,
                    kind: hit.kind,
                    cycle: ctx.cycle,
                });
                npc.hit_bar_until = ctx.cycle.wrapping_add(300);
                npc.hit_bar = hit.ratio.unwrap();
            }
            if let Some(hit) = mask.secondary {
                out.events.push(Event::Hit {
                    index,
                    damage: hit.damage,
                    kind: hit.kind,
                    cycle: ctx.cycle,
                });
            }
            if let Some([id, delay]) = mask.animation {
                sounds(
                    &mut out,
                    index,
                    npc.animation
                        .request(id, delay, npc.motion.queue.len() as i32, ctx.sequences),
                );
            }
            if let Some(entity) = mask.face_entity {
                npc.motion.face_entity = entity;
            }
            if let Some([id, packed]) = mask.spot {
                out.events.push(Event::KillingBlow {
                    definition: npc.type_id,
                    tile: npc.motion.head,
                });
                sounds(
                    &mut out,
                    index,
                    npc.animation
                        .request_spot(id, packed, ctx.cycle, ctx.spots, ctx.sequences),
                );
            }
            if let Some(id) = mask.type_id {
                if npc.metadata.area_sound {
                    out.events.push(Event::AreaRemove(index));
                }
                npc.type_id = id;
                npc.metadata = lookup(id)?;
                npc.motion.rotates_on_move = npc.metadata.rotates;
                out.events.push(Event::Type {
                    index,
                    id: Some(id),
                });
                // Original type change retains turn speed.
                if npc.metadata.area_sound {
                    out.events.push(Event::AreaAdd {
                        index,
                        tile: npc.motion.head,
                    });
                }
            }
            if let Some(chat) = mask.chat {
                npc.chat = chat;
                npc.chat_loops = 100;
            }
            if let Some(slots) = mask.slots {
                sounds(
                    &mut out,
                    index,
                    npc.animation.request_slots(&slots, ctx.sequences),
                );
            }
            if let Some(tile) = mask.face_tile {
                npc.motion.face_tile = tile;
            }
        }
        for &index in &out.removed_candidates {
            if let Some(npc) = self.actors.get(&index) {
                if npc.last_seen != ctx.cycle {
                    if npc.metadata.area_sound {
                        out.events.push(Event::AreaRemove(index));
                    }
                    out.events.push(Event::Type { index, id: None });
                    self.actors.remove(&index);
                    out.removed.push(index);
                }
            }
        }
        if r.remaining() != 0 {
            return Err(Error::new(r.position(), "NPC_INFO trailing bytes"));
        }
        for index in &self.local_ids {
            if !self.actors.contains_key(index) {
                return Err(Error::new(r.position(), "missing NPC local membership"));
            }
        }
        Ok(out)
    }
}
