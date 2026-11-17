//! Original NPC AreaSound construction, identity, morph updates and redraw.
//! Definitions are resolved by the caller; voices use the shared ambient Host.
use crate::{
    ambient::{Definition as Sound, Emitter, Host},
    walking::PlayerBas,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug)]
pub struct Definition {
    pub size: i32,
    /// Fine-coordinate radius (NpcType.soundRadius multiplied by 128).
    pub radius: i32,
    /// Original idle, crawl, walk and run order.
    pub sounds: [i32; 4],
}

#[derive(Clone, Debug)]
pub struct Source {
    /// Caller-owned raw NPC type identity, retained for late morph resolution.
    pub type_id: i32,
    pub fine: [i32; 2],
    /// Npc.getSize, which can differ from the resolved child type's size.
    pub size: i32,
    pub sequence: i32,
    pub bas: PlayerBas,
    pub multi: bool,
    pub resolved: Option<Definition>,
}
impl Source {
    pub fn movement_speed(&self) -> i32 {
        if self.sequence == self.bas.idle {
            0
        } else if std::iter::once(self.bas.run)
            .chain(self.bas.run_turns)
            .any(|id| id == self.sequence)
        {
            2
        } else if std::iter::once(self.bas.slow)
            .chain(self.bas.slow_turns)
            .any(|id| id == self.sequence)
        {
            3
        } else {
            1
        }
    }
    pub fn sound(&self) -> i32 {
        self.resolved.as_ref().map_or(-1, |d| {
            d.sounds[[0, 2, 3, 1][self.movement_speed() as usize]]
        })
    }
}

pub type Actor = Rc<RefCell<Source>>;

pub struct Entry {
    pub actor: Actor,
    pub emitter: Emitter,
    pub movement_speed: i32,
}
impl Entry {
    fn new(actor: Actor, tile: [i32; 2], plane: i32) -> Self {
        let n = actor.borrow();
        // A null morph leaves AreaSound's primitive defaults intact, including
        // sound=0 and max fine coordinates=0. It is not the location-null case.
        let mut emitter = Emitter {
            min: tile.map(|v| v.wrapping_mul(128)),
            max: [0; 2],
            plane,
            definition: Sound {
                sound: 0,
                ..Sound::default()
            },
            remaining: 0,
            multi: n.multi,
            primary: None,
            secondary: None,
        };
        if let Some(d) = &n.resolved {
            emitter.max = tile.map(|v| v.wrapping_add(d.size).wrapping_mul(128));
            emitter.definition.sound = n.sound();
            emitter.definition.radius = d.radius;
        }
        drop(n);
        Self {
            actor,
            emitter,
            movement_speed: 0,
        }
    }
    fn stop_primary(&mut self, host: &mut impl Host) {
        if let Some(id) = self.emitter.primary.take() {
            host.stop(id);
        }
    }
    fn update(&mut self, host: &mut impl Host) {
        let n = self.actor.borrow();
        let sound = n.sound();
        if sound == self.emitter.definition.sound {
            return;
        }
        self.emitter.definition.sound = sound;
        self.emitter.definition.radius = n.resolved.as_ref().map_or(0, |d| d.radius);
        drop(n);
        self.stop_primary(host);
    }
    fn redraw(
        &mut self,
        listener: [i32; 2],
        plane: i32,
        elapsed: i32,
        volume: i32,
        host: &mut impl Host,
    ) {
        let n = self.actor.borrow();
        let speed = n.movement_speed();
        if speed != self.movement_speed {
            let sound = n.sound();
            if self.emitter.definition.sound != sound {
                self.emitter.definition.sound = sound;
                if let Some(id) = self.emitter.primary.take() {
                    host.stop(id);
                }
            }
            self.movement_speed = speed;
        }
        self.emitter.min = n.fine;
        self.emitter.max = n.fine.map(|v| v.wrapping_add(n.size.wrapping_mul(64)));
        drop(n);
        self.emitter
            .tick(listener, plane, elapsed, volume, host, || 0.0);
    }
}

#[derive(Default)]
pub struct Collection {
    pub entries: Vec<Entry>,
}
impl Collection {
    pub fn add(&mut self, actor: Actor, tile: [i32; 2], plane: i32) {
        self.entries.push(Entry::new(actor, tile, plane));
    }
    pub fn remove(&mut self, actor: &Actor, host: &mut impl Host) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|e| Rc::ptr_eq(&e.actor, actor))
        {
            self.entries.remove(index).stop_primary(host);
        }
    }
    pub fn update_multi(&mut self, host: &mut impl Host) {
        for entry in self.entries.iter_mut().filter(|e| e.emitter.multi) {
            entry.update(host);
        }
    }
    pub fn redraw(
        &mut self,
        listener: [i32; 2],
        plane: i32,
        elapsed: i32,
        volume: i32,
        host: &mut impl Host,
    ) {
        for entry in &mut self.entries {
            entry.redraw(listener, plane, elapsed, volume, host);
        }
    }
    pub fn clear(&mut self, host: &mut impl Host) {
        for entry in &mut self.entries {
            entry.stop_primary(host);
        }
        self.entries.clear();
    }
    pub fn suspend(&mut self, host: &mut impl Host) {
        for entry in &mut self.entries {
            entry.stop_primary(host);
        }
    }
}
