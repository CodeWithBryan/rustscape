//! Ordered authoritative wire observations. These are changes, not a fabricated
//! complete map: static cache scenery, collision and asset readiness are separate.
use rs09_client_data::scenery::{Location, Slot, layer};
use rs09_protocol::{
    Error, Result,
    game::{Frame, updates::ServerUpdate},
};
use serde_json::{Value, json};
use std::collections::VecDeque;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Location {
        slot: Slot,
        location: Option<Location>,
        shape: i32,
        rotation: i32,
    },
    ResetChunk {
        plane: i32,
        origin: [i32; 2],
    },
    Sound {
        id: i32,
        loops: i32,
        delay: i32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub player_update: usize,
    pub frame: Frame,
    pub event: Event,
}
#[derive(Default)]
pub struct Observer {
    zone: [i32; 2],
    events: VecDeque<Observation>,
    omitted: usize,
}
impl Observer {
    pub fn zone(&self) -> [i32; 2] {
        self.zone
    }
    pub fn events(&self) -> impl Iterator<Item = &Observation> {
        self.events.iter()
    }
    pub fn drain(&mut self) -> impl Iterator<Item = Observation> + '_ {
        self.events.drain(..)
    }
    pub fn omitted(&self) -> usize {
        self.omitted
    }
    fn record(&mut self, player_update: usize, frame: &Frame, event: Event) {
        self.events.push_back(Observation {
            player_update,
            frame: frame.clone(),
            event,
        });
        if self.events.len() > 256 {
            self.events.pop_front();
            self.omitted += 1;
        }
    }
    /// The top-level envelope is applied once. A malformed later subframe leaves
    /// the already observed prefix. `base` is the last received rebuild base.
    pub fn frame(
        &mut self,
        frame: &Frame,
        plane: i32,
        base: Option<[i32; 2]>,
        player_update: usize,
    ) -> Result<()> {
        let Some(update) = ServerUpdate::decode(frame)? else {
            return Ok(());
        };
        match update {
            ServerUpdate::ZoneOrigin { full, x, y } => {
                self.zone = [x, y];
                if full {
                    let b = base.ok_or(Error::new(0, "scenery reset before scene rebuild"))?;
                    self.record(
                        player_update,
                        frame,
                        Event::ResetChunk {
                            plane,
                            origin: [b[0] + x, b[1] + y],
                        },
                    );
                }
            }
            ServerUpdate::ZoneEnclosed { x, y, frames } => {
                self.zone = [x, y];
                for frame in frames {
                    self.frame(&frame, plane, base, player_update)?;
                }
            }
            ServerUpdate::LocAdd {
                x,
                y,
                shape,
                rotation,
                id,
            } => self.location(
                frame,
                plane,
                base,
                player_update,
                [x, y],
                shape,
                rotation,
                Some(id),
            )?,
            ServerUpdate::LocDel {
                x,
                y,
                shape,
                rotation,
            } => self.location(
                frame,
                plane,
                base,
                player_update,
                [x, y],
                shape,
                rotation,
                None,
            )?,
            ServerUpdate::SynthSound { id, loops, delay } => {
                self.record(player_update, frame, Event::Sound { id, loops, delay })
            }
            _ => {}
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn location(
        &mut self,
        frame: &Frame,
        plane: i32,
        base: Option<[i32; 2]>,
        player_update: usize,
        tile: [i32; 2],
        shape: i32,
        rotation: i32,
        id: Option<i32>,
    ) -> Result<()> {
        if !(0..4).contains(&plane) {
            return Err(Error::new(0, "invalid scenery plane"));
        }
        let base = base.ok_or(Error::new(0, "scenery update before scene rebuild"))?;
        let layer = layer(shape as u8).ok_or(Error::new(0, "invalid scenery shape"))?;
        let slot = Slot {
            plane: plane as usize,
            tile: [
                base[0] + self.zone[0] + tile[0],
                base[1] + self.zone[1] + tile[1],
            ],
            layer,
        };
        let location = id.map(|id| Location {
            id,
            shape: shape as u8,
            rotation: rotation as u8,
        });
        self.record(
            player_update,
            frame,
            Event::Location {
                slot,
                location,
                shape,
                rotation,
            },
        );
        Ok(())
    }
    pub fn snapshot(&self) -> Value {
        let events:Vec<_>=self.events.iter().map(|o|{
            let event=match &o.event{
                Event::Location{slot,location,shape,rotation}=>json!({"kind":if location.is_some(){"add"}else{"remove"},"plane":slot.plane,"tile":slot.tile,"layer":slot.layer,"id":location.map(|l|l.id),"shape":shape,"rotation":rotation}),
                Event::ResetChunk{plane,origin}=>json!({"kind":"resetChunk","plane":plane,"origin":origin}),
                Event::Sound{id,loops,delay}=>json!({"kind":"sound","id":id,"loops":loops,"delay":delay}),
            };
            json!({"playerUpdate":o.player_update,"opcode":o.frame.opcode,"payloadHex":o.frame.payload.iter().map(|b|format!("{b:02x}")).collect::<String>(),"event":event})
        }).collect();
        json!({"scope":"authoritative scenery changes, chunk resets and private sound wire events; static map/collision/assets are separate","events":events,"eventsOmitted":self.omitted})
    }
}
