//! Joined native regression: differential-tested NPC, DUMB/SMART, pulse, queue,
//! NPC encoder and shared graphics-free reader. Content/regions are explicit.
use rs09_game_core::{direction::Direction, random::JavaRandom};
use rs09_protocol::game::{Frame, npcs};
use rs09_server::{
    definitions::item::Shared,
    world::{
        doors::Location,
        movement, npc, npc_render, npc_walk, player_render,
        player_walk::{self, Event, Node, Query, State},
        walk_request::Interfaces,
    },
};
use std::collections::BTreeMap;
struct Host {
    brains: BTreeMap<Node, npc::State>,
    locks: BTreeMap<Node, i32>,
    rng: JavaRandom,
    wall: bool,
    queries: usize,
    events: Vec<(Node, npc::Event)>,
}
impl Default for Host {
    fn default() -> Self {
        Self {
            brains: BTreeMap::new(),
            locks: BTreeMap::new(),
            rng: JavaRandom::new(530),
            wall: false,
            queries: 0,
            events: vec![],
        }
    }
}
impl player_walk::Services for Host {
    fn query(&mut self, s: &mut State, q: Query<'_>) -> player_walk::Result<bool> {
        Ok(match q {
            Query::Locked(n) => self.locks.get(&n).is_some_and(|t| *t > s.ticks),
            Query::CloseDialogue(_) | Query::ZoneMove(..) => true,
            _ => false,
        })
    }
    fn event(&mut self, _: &mut State, _: Event<'_>) -> player_walk::Result<()> {
        Ok(())
    }
    fn interfaces(&self, _: &State, _: Node) -> player_walk::Result<Interfaces> {
        Ok(Interfaces::default())
    }
    fn region(&mut self, _: &mut State, _: &Location) -> player_walk::Result<Option<bool>> {
        Ok(Some(true))
    }
    fn clipping(&mut self, _: &mut State, _: i32, x: i32, _: i32) -> player_walk::Result<i32> {
        self.queries += 1;
        Ok(if self.wall && x == 3209 { 0x12c01ff } else { 0 })
    }
    fn now_millis(&mut self) -> i64 {
        0
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
    fn npc_finder(&mut self, _: &mut State, _: Node) -> player_walk::Result<Option<usize>> {
        Ok(None)
    }
    fn npc_supplier(
        &mut self,
        _: &mut State,
        _: Node,
    ) -> player_walk::Result<Option<rs09_server::world::pathfinding::dispatch::Supplier>> {
        Ok(None)
    }
    fn npc_arrival(&mut self, s: &mut State, n: Node) -> player_walk::Result<bool> {
        npc_walk::arrive(s, self, n).map_err(|e| player_walk::Error::Binding(e.to_string()))
    }
}
impl npc_walk::Services for Host {
    fn brain(&self, n: Node) -> npc::Result<npc::State> {
        self.brains.get(&n).cloned().ok_or(npc::Error::Null)
    }
    fn npc_region_active(&mut self, _: &mut State, _: Node, _: Location) -> npc::Result<bool> {
        Ok(true)
    }
    fn npc_query(&mut self, s: &mut State, n: Node, q: npc::Query) -> npc::Result<bool> {
        Ok(match q {
            npc::Query::Behavior => true,
            npc::Query::MovementLocked => self.locks.get(&n).is_some_and(|t| *t > s.ticks),
            _ => false,
        })
    }
    fn npc_event(&mut self, s: &mut State, n: Node, e: npc::Event) -> npc::Result<()> {
        match e {
            npc::Event::LockMovement(t) => {
                self.locks.insert(n, s.ticks.wrapping_add(t));
            }
            npc::Event::UnlockMovement => {
                self.locks.remove(&n);
            }
            _ => {}
        }
        self.events.push((n, e));
        Ok(())
    }
    fn dialogue_active(&mut self, _: &mut State, _: usize) -> npc::Result<bool> {
        Ok(false)
    }
    fn dialogue_chatbox(&mut self, _: &mut State, _: usize) -> npc::Result<bool> {
        Ok(false)
    }
    fn random_bool(&mut self) -> npc::Result<bool> {
        Ok(self.rng.next_bool())
    }
    fn randomize(&mut self, b: i32) -> npc::Result<i32> {
        Ok(self.rng.next_bound(b).unwrap_or(0))
    }
    fn random_range(&mut self, a: i32, b: i32) -> npc::Result<i32> {
        let n = b.wrapping_sub(a).wrapping_abs();
        Ok(a.min(b).wrapping_add(self.rng.next_bound(n).unwrap_or(0)))
    }
    fn npc_can_move(&mut self, _: &mut State, _: Node, _: Option<Location>) -> npc::Result<bool> {
        Ok(true)
    }
}
fn add(s: &mut State, h: &mut Host, index: i32, l: Location, walks: bool) -> Node {
    let n = s.add_npc(index, l.clone(), [1, 2, 3, 4]);
    let brain = npc::State::new(l);
    brain.fields.write().walks = walks;
    h.brains.insert(n, brain);
    n
}
struct Render {
    handles: Vec<npc_render::Handle>,
    frames: Vec<Frame>,
}
impl npc_render::Context for Render {
    fn hidden(
        &mut self,
        _: &player_render::Handle,
        _: &npc_render::Handle,
    ) -> player_render::Result<bool> {
        Ok(false)
    }
    fn candidates(
        &mut self,
        _: Location,
        _: i32,
    ) -> player_render::Result<Vec<Option<npc_render::Handle>>> {
        Ok(self.handles.iter().cloned().map(Some).collect())
    }
    fn masks(
        &mut self,
        _: &player_render::Handle,
        _: &npc_render::Handle,
        _: &mut Vec<u8>,
        _: bool,
    ) -> player_render::Result<()> {
        panic!("no masks installed in this joined movement case")
    }
    fn remove_tolerance(&mut self, _: &npc_render::Handle, _: i32) -> player_render::Result<()> {
        panic!("no aggression installed")
    }
    fn add_tolerance(&mut self, _: &npc_render::Handle, _: i32) -> player_render::Result<()> {
        panic!("no aggression installed")
    }
    fn send(&mut self, _: &player_render::Handle, f: Frame) -> player_render::Result<()> {
        self.frames.push(f);
        Ok(())
    }
}
#[test]
fn seeded_npc_wander_collision_queue_encoder_and_headless_reader_stay_in_sync() {
    let (mut s, mut h) = (
        State::default(),
        Host {
            wall: true,
            ..Host::default()
        },
    );
    let start = Location::new(3207, 3216, 0);
    let n = add(&mut s, &mut h, 7, start.clone(), true);
    let local = Shared::new(player_render::Player::new(1, start.clone(), 2000));
    let view = Shared::new(npc_render::Npc::new(7, 47, start));
    let mut render = Render {
        handles: vec![view.clone()],
        frames: vec![],
    };
    let mut consumer = npcs::State::default();
    let mut moving = 0;
    for t in 1..=180 {
        s.ticks = t;
        s.context(&mut h).pulses().unwrap();
        npc_walk::tick(&mut s, &mut h, n).unwrap();
        let q = &s.actor(n).unwrap().queue;
        let e = q.entity();
        let coordinates = e.location.as_ref().unwrap().coordinates();
        assert!(!e.player);
        assert!(coordinates[0] < 3209, "crossed a fully clipped wall");
        {
            let mut v = view.write();
            v.location = e.location;
            v.direction = Direction::ALL
                .iter()
                .position(|d| Some(*d) == e.direction)
                .unwrap() as i32;
            v.walk_dir = q.walk_dir();
            v.run_dir = q.run_dir();
            assert_eq!(v.run_dir, -1);
            moving += usize::from(v.walk_dir != -1);
        }
        npc_render::render(&mut render, &local).unwrap();
        let f = render.frames.pop().unwrap();
        consumer.decode(&f, [3207, 3216], t).unwrap();
        assert_eq!(
            consumer.actors[&7].tile,
            [coordinates[0], coordinates[1]],
            "tick{t}"
        );
    }
    assert!(moving > 20, "wander never produced sustained movement");
    assert!(h.queries > 100);
}
#[test]
fn original_return_pulse_reaches_spawn_and_releases_lock_and_attribute_edge() {
    let (mut s, mut h) = (State::default(), Host::default());
    let spawn = Location::new(3207, 3216, 0);
    let n = add(&mut s, &mut h, 7, spawn.clone(), false);
    s.actor(n).unwrap().queue.state.write().entity.location = Some(Location::new(3224, 3216, 0));
    s.ticks = 1;
    npc_walk::tick(&mut s, &mut h, n).unwrap();
    let brain = h.brains[&n].clone();
    let id = brain.fields.read().return_pulse.unwrap();
    assert!(brain.fields.read().returning);
    assert_eq!(s.pending().len(), 1);
    assert!(h.locks.contains_key(&n));
    assert_eq!(s.movement(id).unwrap().finder(), movement::SMART);
    for t in 2..=45 {
        s.ticks = t;
        s.context(&mut h).pulses().unwrap();
        npc_walk::tick(&mut s, &mut h, n).unwrap();
        s.collect();
    }
    assert_eq!(
        s.actor(n)
            .unwrap()
            .queue
            .entity()
            .location
            .unwrap()
            .coordinates(),
        spawn.coordinates()
    );
    assert!(!brain.fields.read().returning);
    assert!(brain.fields.read().return_pulse.is_none());
    assert!(!h.locks.contains_key(&n));
    s.context(&mut h).disconnect(n).unwrap();
    s.context(&mut h).pulses().unwrap();
    s.collect();
    assert_eq!(s.retained_counts(), [0, 0, 0]);
}
#[test]
fn detached_return_attribute_retains_its_motion_only_while_owner_is_reachable() {
    let (mut s, mut h) = (State::default(), Host::default());
    let n = add(&mut s, &mut h, 7, Location::new(3207, 3216, 0), false);
    let first = s
        .context(&mut h)
        .return_to_spawn(n, Some(Location::new(3208, 3216, 0)))
        .unwrap();
    s.attribute_motion(n, Some(first)).unwrap();
    let second = s
        .context(&mut h)
        .return_to_spawn(n, Some(Location::new(3209, 3216, 0)))
        .unwrap();
    s.collect();
    assert_ne!(first, second);
    assert!(s.movement(first).is_ok());
    s.attribute_motion(n, None).unwrap();
    s.collect();
    assert!(s.movement(first).is_err());
    s.attribute_motion(n, Some(second)).unwrap();
    s.context(&mut h).disconnect(n).unwrap();
    s.collect();
    assert_eq!(s.retained_counts(), [0, 0, 0]);
}
#[test]
fn npc_arrival_keeps_the_original_owner_when_base_mover_changes() {
    let (mut s, mut h) = (State::default(), Host::default());
    let n = add(&mut s, &mut h, 7, Location::new(3207, 3216, 0), false);
    let other = add(&mut s, &mut h, 8, Location::new(3210, 3216, 0), false);
    let id = s
        .context(&mut h)
        .return_to_spawn(n, Some(Location::new(3208, 3216, 0)))
        .unwrap();
    let mut p = s.movement(id).unwrap();
    p.state.write().mover = Some(other);
    movement::Context::pulse(&mut s.context(&mut h), &mut p).unwrap();
    assert!(
        h.events
            .iter()
            .any(|(node, e)| *node == n && matches!(e, npc::Event::StopCombat))
    );
    assert!(!h.events.iter().any(|(node, _)| *node == other));
}

#[test]
fn pinned_server_run_bits_and_rt4_selector_discrepancy_remain_explicit() {
    let start = Location::new(3207, 3216, 0);
    let local = Shared::new(player_render::Player::new(1, start.clone(), 2000));
    let view = Shared::new(npc_render::Npc::new(7, 47, start));
    let mut render = Render {
        handles: vec![view.clone()],
        frames: vec![],
    };
    let mut reader = npcs::State::default();
    npc_render::render(&mut render, &local).unwrap();
    reader
        .decode(&render.frames.pop().unwrap(), [3207, 3216], 1)
        .unwrap();
    {
        let mut v = view.write();
        v.walk_dir = 4;
        v.run_dir = 4;
        v.location = Some(Location::new(3209, 3216, 0));
    }
    npc_render::render(&mut render, &local).unwrap();
    reader
        .decode(&render.frames.pop().unwrap(), [3207, 3216], 2)
        .unwrap();
    // NPCRenderer omits the crawl/run selector read by Protocol.readNpcPacket.
    // This records the pinned mismatch; neither separately verified algorithm
    // should be silently changed to hide it. Normal NPC wandering only walks.
    assert_eq!(reader.actors[&7].tile, [3206, 3218]);
    assert_ne!(reader.actors[&7].tile, [3209, 3216]);
}
