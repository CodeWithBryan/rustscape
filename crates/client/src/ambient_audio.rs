//! Shared location/NPC emitters. Packet lifetimes, varp refreshes and redraw use
//! their original phases; cache readiness and the global RNG are services.
use rs09_client::{
    ambient::{Definition, Emitter, Host},
    ambient_npc::{self, Actor as NpcActor, Collection as NpcSounds, Source as NpcSource},
    audio::SoundEffect,
    interaction::LocationType,
    npc,
    npc_updates::{self, Event as NpcEvent},
    pack::Pack,
    pcm::Handle,
    ui::{state::State, vm::Vm},
    walking::PlayerBas,
    world::{self, Location, Slot},
};
use rs09_game_core::random::JavaRandom;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

struct Entry {
    tile: [i32; 2],
    plane: usize,
    id: i32,
    emitter: Emitter,
}
pub struct Locations {
    definitions: BTreeMap<i32, LocationType>,
    initial: Vec<[i32; 6]>,
    current: BTreeMap<Slot, Location>,
    entries: Vec<Entry>,
    cache: BTreeMap<i32, Arc<SoundEffect>>,
    failed: BTreeSet<i32>,
    handles: BTreeMap<u64, Handle>,
    random: JavaRandom,
    cycle: Option<i32>,
    reported: BTreeSet<String>,
    npcs: NpcSounds,
    npc_actors: BTreeMap<usize, NpcActor>,
    npc_bases: BTreeMap<i32, PlayerBas>,
    npc_varp_cursor: usize,
    pub submitted: usize,
}
pub struct Listener {
    pub cycle: i32,
    pub fine: [i32; 2],
    pub volume: i32,
}
pub struct Updates {
    pub locations: Vec<(Slot, Location)>,
    pub varps: Vec<BTreeMap<i32, i32>>,
}
#[derive(Clone, Copy)]
pub struct Services<'a> {
    pub ui: &'a State,
    pub vm: &'a Vm,
    pub npcs: &'a npc_updates::State,
    pub definitions: &'a BTreeMap<u32, npc::Definition>,
    pub location_types: &'a BTreeMap<u32, LocationType>,
    pub world_pack: &'a Pack,
    pub plane: i32,
}
struct AudioHost<'a> {
    cache: &'a mut BTreeMap<i32, Arc<SoundEffect>>,
    failed: &'a mut BTreeSet<i32>,
    handles: &'a mut BTreeMap<u64, Handle>,
    effects: &'a mut crate::ui_audio::Effects,
    pack: &'a Pack,
    errors: &'a mut Vec<String>,
    submitted: &'a mut usize,
}
impl Host for AudioHost<'_> {
    fn start(&mut self, id: i32, volume: i32, loops: i32) -> Option<u64> {
        if self.failed.contains(&id) {
            return None;
        }
        if let std::collections::btree_map::Entry::Vacant(entry) = self.cache.entry(id) {
            match self
                .pack
                .get(&format!("sound/{id}"))
                .and_then(|b| SoundEffect::decode(b).map_err(|e| e.to_string()))
            {
                Ok(pcm) => {
                    entry.insert(Arc::new(pcm));
                }
                Err(error) => {
                    self.failed.insert(id);
                    self.errors.push(format!("Ambient sound{id}: {error}"));
                    return None;
                }
            }
        }
        let pcm = self.cache.get(&id)?.clone();
        let handle = self.effects.start_pcm(pcm, loops, volume)?;
        let key = handle.id;
        self.handles.insert(key, handle);
        *self.submitted += 1;
        Some(key)
    }
    fn volume(&mut self, id: u64, volume: i32) {
        self.effects.pcm_volume(id, volume);
    }
    fn stop(&mut self, id: u64) {
        self.effects.pcm_stop(id);
    }
    fn linked(&self, id: u64) -> bool {
        self.handles.get(&id).is_some_and(Handle::linked)
    }
}
fn selected(
    definitions: &BTreeMap<i32, LocationType>,
    id: i32,
    vm: &Vm,
    ui: &State,
    values: &BTreeMap<i32, i32>,
) -> Result<Option<Option<Definition>>, String> {
    let d = definitions
        .get(&id)
        .ok_or_else(|| format!("Ambient location metadata{id} unavailable"))?;
    if d.children.is_none() {
        return Ok(None);
    }
    let mut error = None;
    let child = d.select_morph(|bit, key| {
        if bit {
            vm.varbit_from(ui, key, values).unwrap_or_else(|e| {
                error = Some(e);
                0
            })
        } else {
            values.get(&key).copied().unwrap_or(0)
        }
    });
    if let Some(error) = error {
        return Err(error);
    }
    Ok(Some(if let Some(child) = child {
        Some(
            definitions
                .get(&(child as i32))
                .ok_or_else(|| format!("Ambient location child{child} unavailable"))?
                .ambient
                .clone(),
        )
    } else {
        None
    }))
}
fn npc_resolved(
    services: Services<'_>,
    id: i32,
    values: &BTreeMap<i32, i32>,
) -> Result<Option<ambient_npc::Definition>, String> {
    let raw = services
        .definitions
        .get(&(id as u32))
        .ok_or_else(|| format!("NPC ambient definition{id} unavailable"))?;
    let mut error = None;
    let id = if raw.children.is_none() {
        Some(id as u32)
    } else {
        raw.select_morph(|bit, key| {
            if bit {
                services
                    .vm
                    .varbit_from(services.ui, key, values)
                    .unwrap_or_else(|e| {
                        error = Some(e);
                        0
                    })
            } else {
                values.get(&key).copied().unwrap_or(0)
            }
        })
    };
    if let Some(error) = error {
        return Err(error);
    }
    id.map(|id| {
        let d = services
            .definitions
            .get(&id)
            .ok_or_else(|| format!("NPC ambient child{id} unavailable"))?;
        Ok(ambient_npc::Definition {
            size: d.size,
            radius: d.sound_radius.wrapping_mul(128),
            sounds: d.sounds,
        })
    })
    .transpose()
}
impl Locations {
    pub fn new(pack: &Pack, seed: i64) -> Result<Self, String> {
        let mut definitions = BTreeMap::new();
        for key in pack.manifest.files.keys() {
            if let Some(id) = key.strip_prefix("loc/") {
                definitions.insert(
                    id.parse().map_err(|_| "Ambient location id")?,
                    LocationType::decode(pack.get(key)?).map_err(|e| e.to_string())?,
                );
            }
        }
        #[derive(serde::Deserialize)]
        struct Scene {
            locations: Vec<[i32; 6]>,
        }
        let initial = serde_json::from_slice::<Scene>(pack.get("scene.json")?)
            .map_err(|e| e.to_string())?
            .locations;
        let mut current = BTreeMap::new();
        for &[id, x, y, plane, shape, rotation] in &initial {
            if let Some(layer) = world::layer(shape as u8) {
                current.insert(
                    Slot {
                        plane: plane as usize,
                        tile: [x, y],
                        layer,
                    },
                    Location {
                        id,
                        shape: shape as u8,
                        rotation: rotation as u8,
                    },
                );
            }
        }
        Ok(Self {
            definitions,
            initial,
            current,
            entries: vec![],
            cache: BTreeMap::new(),
            failed: BTreeSet::new(),
            handles: BTreeMap::new(),
            random: JavaRandom::new(seed),
            cycle: None,
            reported: BTreeSet::new(),
            npcs: NpcSounds::default(),
            npc_actors: BTreeMap::new(),
            npc_bases: BTreeMap::new(),
            npc_varp_cursor: 0,
            submitted: 0,
        })
    }
    fn npc_host(
        &mut self,
        effects: &mut crate::ui_audio::Effects,
        pack: &Pack,
        errors: &mut Vec<String>,
        run: impl FnOnce(&mut NpcSounds, &mut AudioHost<'_>),
    ) {
        let mut host = AudioHost {
            cache: &mut self.cache,
            failed: &mut self.failed,
            handles: &mut self.handles,
            effects,
            pack,
            errors,
            submitted: &mut self.submitted,
        };
        run(&mut self.npcs, &mut host);
    }
    fn npc_source(
        &mut self,
        index: usize,
        id: i32,
        services: Services<'_>,
    ) -> Result<Option<NpcSource>, String> {
        let Some(d) = services.definitions.get(&(id as u32)) else {
            return Ok(None);
        };
        let audible = |d: &npc::Definition| [0, 2, 3].into_iter().any(|i| d.sounds[i] != -1);
        let registered = d.children.as_ref().map_or_else(
            || audible(d),
            |ids| {
                ids.iter()
                    .filter_map(|id| services.definitions.get(&(*id as u32)))
                    .any(audible)
            },
        );
        if !registered && !self.npc_actors.contains_key(&index) {
            return Ok(None);
        }
        let actor = services
            .npcs
            .actors
            .get(&index)
            .ok_or_else(|| format!("NPC ambient actor{index} unavailable"))?;
        if let std::collections::btree_map::Entry::Vacant(entry) = self.npc_bases.entry(d.bas) {
            let bas = if d.bas == -1 {
                PlayerBas::default()
            } else {
                let name = format!("bas/{}", d.bas);
                PlayerBas::decode(
                    services
                        .ui
                        .pack
                        .get(&name)
                        .or_else(|_| services.world_pack.get(&name))?,
                )
                .map_err(|e| e.to_string())?
            };
            entry.insert(bas);
        }
        Ok(Some(NpcSource {
            type_id: id,
            fine: actor.motion.fine,
            size: d.size,
            sequence: actor.motion.sequence,
            bas: self.npc_bases[&d.bas].clone(),
            multi: d.children.is_some(),
            resolved: npc_resolved(services, id, &services.vm.varps)?,
        }))
    }
    pub fn npc_events(
        &mut self,
        events: &[NpcEvent],
        services: Services<'_>,
        effects: &mut crate::ui_audio::Effects,
    ) -> Vec<String> {
        let mut errors = vec![];
        for event in events {
            match event {
                NpcEvent::Type {
                    index,
                    id: Some(id),
                } => match self.npc_source(*index, *id, services) {
                    Ok(Some(source)) => {
                        if let Some(actor) = self.npc_actors.get(index) {
                            *actor.borrow_mut() = source;
                        } else {
                            self.npc_actors
                                .insert(*index, Rc::new(RefCell::new(source)));
                        }
                    }
                    Ok(None) => {}
                    Err(e) => errors.push(e),
                },
                NpcEvent::Type { index, id: None } => {
                    self.npc_actors.remove(index);
                }
                NpcEvent::AreaRemove(index) => {
                    if let Some(actor) = self.npc_actors.get(index).cloned() {
                        self.npc_host(effects, &services.ui.pack, &mut errors, |npcs, host| {
                            npcs.remove(&actor, host)
                        });
                    }
                }
                NpcEvent::AreaAdd { index, tile } => {
                    if let Some(actor) = self.npc_actors.get(index) {
                        self.npcs.add(actor.clone(), *tile, services.plane);
                    }
                }
                _ => {}
            }
        }
        self.report(errors)
    }
    fn refresh_npcs(
        &mut self,
        services: Services<'_>,
        values: &BTreeMap<i32, i32>,
        errors: &mut Vec<String>,
    ) {
        for (index, source) in &self.npc_actors {
            if let Some(actor) = services.npcs.actors.get(index) {
                let mut source = source.borrow_mut();
                if source.type_id == actor.type_id {
                    source.fine = actor.motion.fine;
                    source.size = actor.metadata.size;
                    source.sequence = actor.motion.sequence;
                }
            }
        }
        let mut seen = BTreeSet::new();
        for source in self
            .npcs
            .entries
            .iter()
            .map(|e| &e.actor)
            .chain(self.npc_actors.values())
        {
            if seen.insert(Rc::as_ptr(source) as usize) {
                let mut source = source.borrow_mut();
                match npc_resolved(services, source.type_id, values) {
                    Ok(d) => source.resolved = d,
                    Err(e) => errors.push(e),
                }
            }
        }
    }
    /// Flush before NPC packets and before actor motion, so a later animation
    /// category cannot change the sound chosen by an earlier varp poll.
    pub fn npc_update_multi(
        &mut self,
        services: Services<'_>,
        effects: &mut crate::ui_audio::Effects,
    ) -> Vec<String> {
        let mut errors = vec![];
        for values in services
            .vm
            .ambient_updates
            .iter()
            .skip(self.npc_varp_cursor)
        {
            self.refresh_npcs(services, values, &mut errors);
            self.npc_host(effects, &services.ui.pack, &mut errors, |npcs, host| {
                npcs.update_multi(host)
            });
        }
        self.npc_varp_cursor = services.vm.ambient_updates.len();
        self.report(errors)
    }
    fn report(&mut self, errors: Vec<String>) -> Vec<String> {
        errors
            .into_iter()
            .filter(|e| self.reported.insert(e.clone()))
            .collect()
    }
    fn add(&mut self, slot: Slot, loc: Location, vm: &Vm, ui: &State) -> Result<(), String> {
        let d = self
            .definitions
            .get(&loc.id)
            .ok_or_else(|| format!("Ambient location metadata{} unavailable", loc.id))?;
        let mut missing = None;
        let audible = d.has_area_sound(|id| {
            self.definitions.get(&id).map_or_else(
                || {
                    missing = Some(id);
                    false
                },
                |d| d.ambient.audible(),
            )
        });
        if let Some(id) = missing {
            return Err(format!("Ambient location child{id} unavailable"));
        }
        if !audible {
            return Ok(());
        }
        let morph = selected(&self.definitions, loc.id, vm, ui, &vm.varps)?;
        let emitter = Emitter::location(
            slot.tile,
            d.size,
            i32::from(loc.rotation),
            slot.plane as i32,
            d.ambient.clone(),
            morph,
            || self.random.next_double(),
        );
        self.entries.push(Entry {
            tile: slot.tile,
            plane: slot.plane,
            id: loc.id,
            emitter,
        });
        Ok(())
    }
    pub fn advance(
        &mut self,
        updates: Updates,
        effects: &mut crate::ui_audio::Effects,
        listener: Listener,
        services: Services<'_>,
    ) -> Vec<String> {
        let (ui, vm) = (services.ui, services.vm);
        let mut errors = vec![];
        self.refresh_npcs(services, &vm.varps, &mut errors);
        self.npc_varp_cursor = 0;
        for [id, x, y, plane, shape, rotation] in std::mem::take(&mut self.initial) {
            if let Some(layer) = world::layer(shape as u8) {
                if let Err(error) = self.add(
                    Slot {
                        plane: plane as usize,
                        tile: [x, y],
                        layer,
                    },
                    Location {
                        id,
                        shape: shape as u8,
                        rotation: rotation as u8,
                    },
                    vm,
                    ui,
                ) {
                    errors.push(error);
                }
            }
        }
        for (slot, loc) in updates.locations {
            if let Some(old) = self.current.remove(&slot) {
                if let Some(index) = self
                    .entries
                    .iter()
                    .position(|e| e.plane == slot.plane && e.tile == slot.tile && e.id == old.id)
                {
                    let mut entry = self.entries.remove(index);
                    entry.emitter.remove(&mut AudioHost {
                        cache: &mut self.cache,
                        failed: &mut self.failed,
                        handles: &mut self.handles,
                        effects,
                        pack: &ui.pack,
                        errors: &mut errors,
                        submitted: &mut self.submitted,
                    });
                }
            }
            if loc.id >= 0 {
                if !self.definitions.contains_key(&loc.id) {
                    if let Some(definition) = services.location_types.get(&(loc.id as u32)) {
                        if let Some(children) = &definition.children {
                            for &id in children {
                                if let Some(child) = services.location_types.get(&(id as u32)) {
                                    self.definitions.entry(id).or_insert_with(|| child.clone());
                                }
                            }
                        }
                        self.definitions.insert(loc.id, definition.clone());
                    }
                }
                self.current.insert(slot, loc);
                if let Err(error) = self.add(slot, loc, vm, ui) {
                    errors.push(error);
                }
            }
        }
        let elapsed = self
            .cycle
            .map_or(0, |c| listener.cycle.wrapping_sub(c).clamp(0, 1000));
        self.cycle = Some(listener.cycle);
        let mut host = AudioHost {
            cache: &mut self.cache,
            failed: &mut self.failed,
            handles: &mut self.handles,
            effects,
            pack: &ui.pack,
            errors: &mut errors,
            submitted: &mut self.submitted,
        };
        for values in &updates.varps {
            for entry in self.entries.iter_mut().filter(|e| e.emitter.multi) {
                match selected(&self.definitions, entry.id, vm, ui, values) {
                    Ok(Some(d)) => {
                        if d.as_ref().unwrap_or(&Definition::default()) != &entry.emitter.definition
                        {
                            entry.emitter.update(d, &mut host);
                        }
                    }
                    Err(error) => host.errors.push(error),
                    _ => {}
                }
            }
        }
        for entry in &mut self.entries {
            entry.emitter.tick(
                listener.fine,
                vm.location[2],
                elapsed,
                listener.volume,
                &mut host,
                || self.random.next_double(),
            );
        }
        self.npcs.redraw(
            listener.fine,
            services.plane,
            elapsed,
            listener.volume,
            &mut host,
        );
        self.handles.retain(|id, h| {
            h.linked()
                || self
                    .entries
                    .iter()
                    .any(|e| e.emitter.primary == Some(*id) || e.emitter.secondary == Some(*id))
                || self
                    .npcs
                    .entries
                    .iter()
                    .any(|e| e.emitter.primary == Some(*id))
        });
        errors
            .into_iter()
            .filter(|e| self.reported.insert(e.clone()))
            .collect()
    }
    #[cfg(target_os = "windows")]
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!({"emitters":self.entries.len(),"cached_sounds":self.cache.len(),"submitted":self.submitted,"active_voices":self.handles.values().filter(|h|h.linked()).count(),"entries":self.entries.iter().map(|e|serde_json::json!({"id":e.id,"tile":e.tile,"plane":e.plane,"state":e.emitter})).collect::<Vec<_>>(),"npc_entries":self.npcs.entries.iter().map(|e|{let n=e.actor.borrow();serde_json::json!({"type":n.type_id,"fine":n.fine,"size":n.size,"sequence":n.sequence,"movement_speed":e.movement_speed,"state":e.emitter})}).collect::<Vec<_>>()})
    }
    pub fn suspend(&mut self, effects: &mut crate::ui_audio::Effects, pack: &Pack) {
        self.npc_varp_cursor = 0;
        self.npc_host(effects, pack, &mut vec![], |npcs, host| npcs.suspend(host));
        for entry in &mut self.entries {
            for id in [entry.emitter.primary.take(), entry.emitter.secondary.take()]
                .into_iter()
                .flatten()
            {
                effects.pcm_stop(id);
            }
        }
    }
}
