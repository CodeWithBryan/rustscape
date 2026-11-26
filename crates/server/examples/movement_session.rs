//! Controlled live player-walk probe. Real cache collision and native original
//! walk/request/pulse/queue/tick/render code, with empty content services. This
//! does not replace complete Player/profile/region/content initialization.
#[path = "common/combat.rs"]
mod combat_services;
#[path = "common/ground.rs"]
mod ground_services;
#[path = "common/npc.rs"]
mod npc_services;
#[path = "common/profiles.rs"]
mod profiles;
#[path = "common/scenery.rs"]
mod scenery_services;
#[path = "common/scripts.rs"]
mod script_services;
#[path = "common/skilling.rs"]
mod skilling;
#[path = "common/spatial.rs"]
mod spatial_services;
#[path = "common/specials.rs"]
mod specials;
#[path = "common/ui.rs"]
mod ui_services;
#[allow(dead_code)]
#[path = "player_session.rs"]
mod visual;
#[path = "common/weapons.rs"]
mod weapons;
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_protocol::{
    buffer::Reader,
    game::{Frame, actions::ClientAction, updates},
    login::AuthResponse,
};
use rs09_server::{
    auth::{
        Authenticator, Mode, account::UserAccountInfo, password::Passwords,
        storage::InMemoryStorage,
    },
    catalog::Catalog,
    definitions::{animation as animation_defs, item::Shared, scenery as defs},
    pulses,
    runtime::{Config, Service},
    world::{
        animation::Animator,
        appearance,
        doors::{self, Location},
        packet_queue::{self, Packet, Queue as PacketQueue},
        player_render::Player,
        player_walk::{self, Event, Node, Query, State},
        prepared_animation::{self, Owner},
        prepared_masks::PreparedMasks,
        scene::{self, CacheAssets, Scene},
        session::Joined,
        tick, update,
        walk_request::Interfaces,
    },
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc},
    task::JoinHandle,
};
use visual::{Actor, Fixture, Join, render};
type Map = Scene<CacheAssets>;
enum AnimationAssets {
    Cache(animation_defs::CacheSource),
    #[cfg(test)]
    Unbound,
}
impl animation_defs::Source for AnimationAssets {
    fn file(
        &mut self,
        r: &animation_defs::Registry,
        g: i32,
        f: i32,
    ) -> animation_defs::Result<Option<Vec<u8>>> {
        match self {
            Self::Cache(source) => source.file(r, g, f),
            #[cfg(test)]
            Self::Unbound => Err(animation_defs::Error::Binding(
                "portable test has no animation archive".into(),
            )),
        }
    }
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
#[derive(Default)]
struct MapServices {
    players: BTreeSet<Node>,
    names: BTreeMap<Node, String>,
}
macro_rules! unbound {
    ($name:ident($($arg:ident:$ty:ty),* $(,)?) -> $ret:ty) => {fn $name(&mut self,$($arg:$ty),*)->scene::Result<$ret>{Err(scene::Error(concat!("movement probe has no scene service: ",stringify!($name)).into()))}};
}
impl scene::Services<CacheAssets> for MapServices {
    fn is_player(&self, n: usize) -> bool {
        self.players.contains(&n)
    }
    unbound!(location(_scene: &mut Map, _entity: usize) -> Location);
    unbound!(lock(_scene: &mut Map, _entity: usize, _ticks: i32) -> ());
    unbound!(audio(_scene: &mut Map, _entity: usize, _id: i32, _delay: Option<i32>) -> ());
    unbound!(message(_scene: &mut Map, _entity: usize, _text: &str) -> ());
    unbound!(logout_listener(_scene: &mut Map, _entity: usize, _location: Option<Location>) -> ());
    unbound!(reset_path(_scene: &mut Map, _entity: usize) -> ());
    unbound!(add_path(_scene: &mut Map, _entity: usize, _x: i32, _y: i32) -> ());
    unbound!(move_near(_scene: &mut Map, _entity: usize, _location: Location) -> bool);
    unbound!(
        diary(
            _scene: &mut Map,
            _entity: usize,
            _diary: doors::Diary,
            _level: i32,
            _task: i32,
        ) -> ()
    );
    unbound!(ground_create(_scene: &mut Map, _item: Option<usize>) -> ());
    unbound!(pulse(_scene: &mut Map, _pulse: &pulses::Handle) -> bool);
    unbound!(stop(_scene: &mut Map, _pulse: &pulses::Handle) -> ());
    fn now_millis(&mut self) -> i64 {
        now()
    }
}
struct Host {
    animations: prepared_animation::World<AnimationAssets>,
    animators: BTreeMap<Node, Animator<Owner>>,
    scene: Map,
    maps: MapServices,
    active: BTreeSet<i32>,
    frames: BTreeMap<Node, Vec<Frame>>,
    errors: Vec<String>,
    npc: Option<npc_services::Npcs>,
    ui: Option<ui_services::Ui>,
    ground: Option<ground_services::Ground>,
    scenery: Option<rs09_server::world::scenery_options::Registry>,
    scenery_trace: Option<std::fs::File>,
    masks: BTreeMap<Node, PreparedMasks>,
}
impl Host {
    fn activate(&mut self, l: &Location) -> player_walk::Result<()> {
        let [x, y, _] = l.coordinates();
        let id = ((x >> 6) << 8) | (y >> 6);
        self.scene
            .context(&mut self.maps)
            .load(id, None)
            .map_err(|e| player_walk::Error::Binding(e.0))?;
        self.active.insert(id);
        Ok(())
    }
    fn send(&mut self, n: Node, f: Frame) {
        self.frames.entry(n).or_default().push(f);
    }
}
impl player_walk::Services for Host {
    fn external_pulse(&mut self, state: &mut State, p: &pulses::Handle) -> pulses::Result<bool> {
        if ground_services::owns(self, p) {
            return ground_services::pulse(state, self, p);
        }
        self.scene.context(&mut self.maps).pulse_body(p)
    }
    fn external_inactive(&mut self, state: &mut State, p: &pulses::Handle) -> pulses::Result<bool> {
        if ground_services::owns(self, p) {
            return ground_services::inactive(state, self, p);
        }
        self.scene.context(&mut self.maps).pulse_inactive(p)
    }
    fn external_stop(&mut self, state: &mut State, p: &pulses::Handle) -> pulses::Result<()> {
        if ground_services::owns(self, p) {
            return ground_services::stop(state, self, p);
        }
        self.scene.context(&mut self.maps).stop_pulse(p)
    }
    fn query(&mut self, state: &mut State, q: Query<'_>) -> player_walk::Result<bool> {
        Ok(match q {
            Query::Locked(n) => {
                script_services::locked(state, n, rs09_server::world::action_locks::Kind::Movement)
                    .map_err(|e| player_walk::Error::Binding(e.to_string()))?
            }
            Query::Timer(_, _) | Query::Marathon(_) => false,
            Query::CloseDialogue(_) | Query::ZoneMove(_, _, _) => true,
            Query::CloseInterface(_) => {
                return Err(player_walk::Error::Unsupported(
                    "no interfaces in movement probe",
                ));
            }
        })
    }
    fn event(&mut self, state: &mut State, e: Event<'_>) -> player_walk::Result<()> {
        match e {
            Event::Minimap(n) => self.send(
                n,
                Frame {
                    opcode: 153,
                    payload: vec![],
                },
            ),
            Event::Energy(n, _) => self.send(
                n,
                updates::run_energy(state.actor(n)?.queue.entity().energy),
            ),
            Event::RunToggled(n, b) => self.send(n, updates::varp(173, i32::from(b))),
            Event::Message(n, s) => {
                let mut payload = s.as_bytes().to_vec();
                payload.push(0);
                self.send(
                    n,
                    Frame {
                        opcode: 70,
                        payload,
                    },
                );
            }
            Event::RegionMove(n, from, Some(l)) => {
                let [x, y, _] = l.coordinates();
                spatial_services::move_entity(state, self, n, from, l)?;
                // Keep a visited region open when a wandering NPC crosses its edge.
                self.active.insert(((x >> 6) << 8) | (y >> 6));
            }
            Event::Exception(s) => {
                if self.errors.len() < 16 {
                    eprintln!("movement probe: {s}");
                }
                self.errors.push(s.into());
            }
            Event::Log(pulses::Level::Error, s) => {
                if self.errors.len() < 16 {
                    eprintln!("movement probe: {s}");
                }
                self.errors.push(s.into());
            }
            Event::Debug(_, s) => eprintln!("movement probe action: {s}"),
            Event::Scripts(n, phase) => script_services::phase(state, self, n, phase)
                .map_err(|e| player_walk::Error::Binding(format!("script phase: {e:?}")))?,
            // This controlled fixture has no game event hooks, timers or skill
            // actions installed. Script phases above use actual actor fields.
            Event::TickEvent(_, _)
            | Event::Skills(_)
            | Event::Timers(_)
            | Event::PulseLength(_, _)
            | Event::CountPulse(_)
            | Event::Log(pulses::Level::Warn, _)
            | Event::BotTotal(_)
            | Event::OtherTotal(_) => {}
            Event::Masks(n) => {
                if let Some(masks) = self.masks.get(&n) {
                    masks
                        .prepare()
                        .map_err(|e| player_walk::Error::Binding(e.to_string()))?;
                }
            }
            Event::FaceLocationNone(n) => {
                if let Some(masks) = self.masks.get(&n) {
                    masks
                        .face_location(None)
                        .map_err(|e| player_walk::Error::Binding(e.to_string()))?;
                }
            }
            Event::Face(n, None) => {
                if let Some(masks) = self.masks.get(&n) {
                    masks
                        .clear_face_entity()
                        .map_err(|e| player_walk::Error::Binding(e.to_string()))?;
                }
            }
            Event::Face(n, Some(target)) => {
                if let Some(masks) = self.masks.get(&n).cloned() {
                    use rs09_server::world::prepared_masks::FaceTarget;
                    let node = state.retain_node(target)?;
                    let target = if let Some(target) = self.masks.get(&target) {
                        FaceTarget::Player(node, target.owner())
                    } else {
                        FaceTarget::Npc(
                            node,
                            npc_services::view(self, target).ok_or(player_walk::Error::Null)?,
                        )
                    };
                    masks
                        .face_entity(Some(target))
                        .map_err(|e| player_walk::Error::Binding(e.to_string()))?;
                }
            }
            Event::CloseChatbox(_) => {
                return Err(player_walk::Error::Unsupported(
                    "no chatboxes in movement probe",
                ));
            }
            Event::RegionMove(_, _, None) => return Err(player_walk::Error::Null),
        }
        Ok(())
    }
    fn interfaces(&self, _: &State, _: Node) -> player_walk::Result<Interfaces> {
        Ok(Interfaces::default())
    }
    fn region(&mut self, _: &mut State, l: &Location) -> player_walk::Result<Option<bool>> {
        let [x, y, _] = l.coordinates();
        let id = ((x >> 6) << 8) | (y >> 6);
        self.scene.region(id);
        Ok(Some(self.active.contains(&id)))
    }
    fn clipping(&mut self, _: &mut State, z: i32, x: i32, y: i32) -> player_walk::Result<i32> {
        self.scene
            .context(&mut self.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| player_walk::Error::Binding(e.0))
    }
    fn water_clipping(
        &mut self,
        _: &mut State,
        z: i32,
        x: i32,
        y: i32,
    ) -> player_walk::Result<i32> {
        self.scene
            .context(&mut self.maps)
            .water_clipping(Some(&Location::new(x, y, z)))
            .map_err(|e| player_walk::Error::Binding(e.0))
    }
    fn now_millis(&mut self) -> i64 {
        now()
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
    fn teleport_permitted(
        &mut self,
        _: &mut State,
        location: Option<Location>,
    ) -> player_walk::Result<bool> {
        self.scene
            .context(&mut self.maps)
            .teleport_permitted(location.as_ref())
            .map_err(|e| player_walk::Error::Binding(e.0))
    }
    fn npc_finder(&mut self, _: &mut State, _: Node) -> player_walk::Result<Option<usize>> {
        Ok(None)
    }
    fn npc_supplier(
        &mut self,
        _: &mut State,
        _: Node,
    ) -> player_walk::Result<Option<rs09_server::world::pathfinding::dispatch::Supplier>> {
        Ok(Some(
            rs09_server::world::pathfinding::dispatch::Supplier::World,
        ))
    }
    fn npc_arrival(&mut self, state: &mut State, node: Node) -> player_walk::Result<bool> {
        npc_services::arrive(state, self, node)
    }
    fn interaction_arrival(
        &mut self,
        state: &mut State,
        a: &rs09_server::world::interaction::Approach,
    ) -> player_walk::Result<bool> {
        rs09_server::world::scenery_runtime::arrival(state, self, a)
    }
    fn delayed_interaction(
        &mut self,
        state: &mut State,
        t: &rs09_server::world::legacy_interaction::Task,
    ) -> player_walk::Result<bool> {
        rs09_server::world::legacy_runtime::arrival(state, self, t)
            .map_err(|e| player_walk::Error::Binding(e.to_string()))
    }
    fn destination(
        &mut self,
        state: &mut State,
        p: Option<Node>,
        target: Option<Node>,
        service: rs09_server::world::movement::DestinationService,
    ) -> player_walk::Result<Option<Location>> {
        if let rs09_server::world::movement::DestinationService::Option(id) = service
            && ground_services::program(self, id)
                .map_err(|e| player_walk::Error::Binding(e.to_string()))?
                .is_some()
        {
            return Ok(None);
        }
        scenery_services::destination(state, self, p, target, service)
    }
    fn door_destination(
        &mut self,
        state: &mut State,
        p: Option<Node>,
        target: Option<Node>,
    ) -> player_walk::Result<Option<Location>> {
        scenery_services::door_destination(state, self, p, target)
    }
}
enum Ingress {
    Frame(i32, Node, Frame),
    Closed(i32, Node),
}
struct QueuedIngress {
    event: Ingress,
    _permit: OwnedSemaphorePermit,
}
struct Probe {
    state: State,
    host: Host,
    actors: BTreeMap<i32, Actor>,
    order: Vec<i32>,
    nodes: BTreeMap<i32, Node>,
    forwarders: BTreeMap<i32, JoinHandle<()>>,
    keys: HashMap<i32, [i32; 4]>,
    ingress: PacketQueue<QueuedIngress>,
    budget: Arc<Semaphore>,
    renderables: update::List<Node>,
    lobby: update::List<Node>,
    npcs: update::List<Node>,
    world_spawns: Option<npc_services::WorldSpawns>,
    sequence: update::Sequence<Node>,
    metrics: [i32; 3],
    retiring: BTreeSet<Node>,
    leases: BTreeMap<Node, player_walk::NodeHandle>,
    melee: Option<combat_services::Melee>,
    woodcutting: Option<skilling::Woodcutting>,
    profiles: Option<profiles::Store>,
    profile_ready: BTreeSet<Node>,
    spawn: [i32; 3],
    disconnect: Vec<(i32, Node)>,
    ignored: BTreeSet<u8>,
}
impl Drop for Probe {
    fn drop(&mut self) {
        for task in self.forwarders.values() {
            task.abort();
        }
    }
}
impl Probe {
    fn new(mut host: Host, keys: HashMap<i32, [i32; 4]>) -> Self {
        let state = State::with_runner(host.scene.runner.clone());
        let renderables = update::List::default();
        if let Some(ground) = &mut host.ground {
            ground.repository = renderables.clone();
        }
        Self {
            state,
            host,
            actors: BTreeMap::new(),
            order: vec![],
            nodes: BTreeMap::new(),
            forwarders: BTreeMap::new(),
            keys,
            ingress: PacketQueue::default(),
            budget: Arc::new(Semaphore::new(4096)),
            renderables,
            lobby: update::List::plain(),
            npcs: update::List::copy_on_write(),
            world_spawns: None,
            sequence: update::Sequence::default(),
            metrics: [0; 3],
            retiring: BTreeSet::new(),
            leases: BTreeMap::new(),
            melee: None,
            woodcutting: None,
            profiles: None,
            profile_ready: BTreeSet::new(),
            spawn: [3207, 3216, 0],
            disconnect: vec![],
            ignored: BTreeSet::new(),
        }
    }
    fn remove(&mut self, index: i32, node: Node) -> Result<(), String> {
        if self.nodes.get(&index) != Some(&node) {
            return Ok(());
        }
        if !self.retiring.insert(node) {
            return Ok(());
        }
        if self.profile_ready.remove(&node) {
            let name = self.actors[&index].username.clone();
            if let Err(e) = profiles::save_node(self, &name, node) {
                eprintln!("Could not queue profile save for {name}: {e}");
            }
        }
        let combat = combat_services::remove(self, node);
        skilling::cancel(self, node);
        if let Some(task) = self.forwarders.remove(&index) {
            task.abort();
        }
        self.actors[&index].player.write().active = false;
        self.state
            .actor_mut(node)
            .map_err(|e| e.to_string())?
            .view
            .active = false;
        spatial_services::detach(&mut self.state, &mut self.host, node)
            .map_err(|e| e.to_string())?;
        self.renderables.remove(&node);
        self.state
            .context(&mut self.host)
            .disconnect(node)
            .map_err(|e| e.to_string())?;
        combat
    }
    fn reap(&mut self) {
        let removed: Vec<_> = self
            .retiring
            .iter()
            .copied()
            .filter(|n| !self.renderables.contains(n))
            .collect();
        for node in removed {
            if let Some(index) = self
                .nodes
                .iter()
                .find_map(|(i, n)| (*n == node).then_some(*i))
            {
                self.actors.remove(&index);
                self.nodes.remove(&index);
                self.order.retain(|i| *i != index);
            }
            self.retiring.remove(&node);
            self.leases.remove(&node);
            self.host.frames.remove(&node);
            self.host.masks.remove(&node);
            self.host.animators.remove(&node);
        }
    }
    fn join(&mut self, join: Join) {
        let Join { prepared, reply } = join;
        if reply.is_closed() {
            return;
        }
        if self
            .actors
            .values()
            .any(|a| a.username == prepared.login.username)
        {
            let _ = reply.send(Err(AuthResponse::AlreadyOnline));
            return;
        }
        let Some(index) = (1..2000).find(|i| !self.actors.contains_key(i)) else {
            let _ = reply.send(Err(AuthResponse::FullWorld));
            return;
        };
        let name = prepared.login.username;
        let saved = match self
            .profiles
            .as_ref()
            .map(|store| store.get(&name))
            .transpose()
        {
            Ok(value) => value.flatten(),
            Err(e) => {
                eprintln!("Profile load failed for {name}: {e}");
                let _ = reply.send(Err(AuthResponse::ErrorLoadingProfile));
                return;
            }
        };
        let location = saved.as_ref().map_or_else(
            || Location::new(self.spawn[0], self.spawn[1], self.spawn[2]),
            profiles::Snapshot::location,
        );
        if let Err(e) = self.host.activate(&location) {
            eprintln!("movement probe map initialization: {e}");
            let _ = reply.send(Err(AuthResponse::ErrorLoadingProfile));
            return;
        }
        npc_services::open_near(self, &location);
        let node = self.state.add(index, location.clone(), [1, 2, 3, 4]);
        self.state.actor_mut(node).unwrap().view.active = false;
        self.leases
            .insert(node, self.state.retain_node(node).unwrap());
        self.state
            .actor(node)
            .unwrap()
            .queue
            .state
            .write()
            .entity
            .update_scene = true;
        self.host.maps.players.insert(node);
        let rights = prepared.details.rights().map_or(0, |r| r.ordinal());
        self.host.maps.names.insert(node, name.clone());
        if let Err(e) = spatial_services::move_entity(
            &mut self.state,
            &mut self.host,
            node,
            None,
            location.clone(),
        ) {
            eprintln!("movement probe entity membership: {e}");
            let _ = spatial_services::detach(&mut self.state, &mut self.host, node);
            let _ = self.state.context(&mut self.host).disconnect(node);
            self.leases.remove(&node);
            self.host.maps.players.remove(&node);
            self.host.maps.names.remove(&node);
            let _ = reply.send(Err(AuthResponse::ErrorLoadingProfile));
            return;
        }
        let player = Shared::new(Player::new(index, location, 2000));
        {
            let mut p = player.write();
            p.active = false;
            p.update_scene = true;
            p.direction = 6;
        }
        let (incoming, mut input) = mpsc::channel(40);
        let (output, outgoing) = mpsc::channel(2);
        let (_, unused) = mpsc::channel(1);
        let clock_owner = self.state.retain_node(node).expect("admitted entity");
        let clocks = self.state.clocks(&clock_owner).expect("admitted clocks");
        self.actors.insert(
            index,
            Actor::with_clocks(
                player,
                appearance::Data::naked(name.clone(), 0),
                unused,
                output,
                name.clone(),
                &self.host.animations.factory,
                clocks,
            ),
        );
        let actor = self.actors.get_mut(&index).expect("just admitted player");
        actor.chat_rights = rights as u8;
        actor.muted = prepared.details.is_muted();
        self.host
            .masks
            .insert(node, self.actors[&index].masks.clone());
        self.host
            .animators
            .insert(node, self.actors[&index].animator.clone());
        ground_services::admit(
            &mut self.host,
            node,
            prepared.details.account.clone(),
            self.actors[&index].player.clone(),
        );
        self.nodes.insert(index, node);
        self.order.push(index);
        self.renderables.add(node);
        if self.host.ui.is_some()
            && let Err(e) =
                ui_services::admit_with_profile(self, node, prepared.client, saved.as_ref())
        {
            eprintln!("gameframe initialization failed: {e}");
            let _ = self.remove(index, node);
            let _ = reply.send(Err(AuthResponse::ErrorLoadingProfile));
            return;
        }
        if self.profiles.is_some() {
            self.profile_ready.insert(node);
        }
        let queue = self.ingress.clone();
        let budget = self.budget.clone();
        self.forwarders.insert(
            index,
            tokio::spawn(async move {
                while let Some(frame) = input.recv().await {
                    let Ok(permit) = budget.clone().acquire_owned().await else {
                        return;
                    };
                    queue.enqueue(Packet::Dispatch(QueuedIngress {
                        event: Ingress::Frame(index, node, frame),
                        _permit: permit,
                    }));
                }
                if let Ok(permit) = budget.acquire_owned().await {
                    queue.enqueue(Packet::Dispatch(QueuedIngress {
                        event: Ingress::Closed(index, node),
                        _permit: permit,
                    }));
                }
            }),
        );
        if reply
            .send(Ok(Joined {
                rights,
                index,
                incoming,
                outgoing,
            }))
            .is_err()
        {
            let _ = self.remove(index, node);
        } else {
            println!("movement probe joined: {name}, index={index}, node={node}");
        }
    }

    fn save_connected(&mut self) {
        let saves: Vec<_> = self
            .nodes
            .iter()
            .filter(|(_, n)| self.profile_ready.contains(n))
            .map(|(i, &n)| (self.actors[i].username.clone(), n))
            .collect();
        for (name, node) in saves {
            if let Err(e) = profiles::save_node(self, &name, node) {
                eprintln!("Could not queue profile save for {name}: {e}");
            }
        }
    }
}
fn project_player(state: &State, node: Node, actor: &Actor) -> Result<(), String> {
    let q = &state.actor(node).map_err(|e| e.to_string())?.queue;
    let e = q.entity();
    let mut p = actor.player.write();
    p.location = e.location;
    p.update_scene = e.update_scene;
    p.teleporting = e.teleporting;
    p.direction = e.direction.map_or(6, |d| d.code());
    p.walk_dir = q.walk_dir();
    p.run_dir = q.run_dir();
    Ok(())
}
#[cfg(test)]
fn project_players(
    state: &State,
    actors: &BTreeMap<i32, Actor>,
    order: &[i32],
    nodes: &BTreeMap<i32, Node>,
    keys: &HashMap<i32, [i32; 4]>,
) -> Result<BTreeMap<Node, Vec<Frame>>, String> {
    let mut frames_by_node: BTreeMap<Node, Vec<Frame>> = BTreeMap::new();
    for (&index, &node) in nodes {
        project_player(state, node, &actors[&index])?;
    }
    for &index in order {
        let node = nodes[&index];
        let player = &actors[&index].player;
        if player.read().update_scene {
            let frames = render(actors, order, keys, player, true).map_err(|e| e.to_string())?;
            frames_by_node.entry(node).or_default().extend(frames);
        }
        let frames = render(actors, order, keys, player, false).map_err(|e| e.to_string())?;
        frames_by_node.entry(node).or_default().extend(frames);
    }
    Ok(frames_by_node)
}
fn reset_player(state: &State, node: Node, actor: &Actor) -> Result<(), String> {
    let mut p = actor.player.write();
    let q = &state.actor(node).map_err(|e| e.to_string())?.queue;
    let mut state = q.state.write();
    state.entity.last_scene = p.last_scene.clone();
    state.entity.update_scene = false;
    state.entity.teleporting = false;
    p.update_scene = false;
    p.teleporting = false;
    let l = p.location.clone();
    let info = p.info.clone();
    drop(state);
    drop(p);
    actor.masks.reset();
    info.write().update_information(l);
    Ok(())
}
#[cfg(test)]
fn reset_players(
    state: &State,
    actors: &BTreeMap<i32, Actor>,
    nodes: &BTreeMap<i32, Node>,
) -> Result<(), String> {
    for (&index, &node) in nodes {
        reset_player(state, node, &actors[&index])?;
    }
    Ok(())
}
impl packet_queue::Context<QueuedIngress> for Probe {
    type Error = String;
    fn process(&mut self, packet: &QueuedIngress) -> Result<(), String> {
        match &packet.event {
            Ingress::Closed(index, node) => self.disconnect.push((*index, *node)),
            Ingress::Frame(index, node, frame) => {
                if self.nodes.get(index) != Some(node) || self.retiring.contains(node) {
                    return Ok(());
                }
                if frame.opcode == rs09_protocol::game::chat::PUBLIC {
                    let actor = &self.actors[index];
                    if actor.muted {
                        combat_services::message(
                            self,
                            *node,
                            "You have been muted due to breaking a rule.",
                        );
                    } else if let Ok(Some((effects, text))) =
                        rs09_protocol::game::chat::public_mask(frame)
                    {
                        actor
                            .masks
                            .chat(effects, actor.chat_rights, text)
                            .map_err(|e| e.to_string())?;
                    }
                    return Ok(());
                }
                let action = ClientAction::decode(frame.opcode, &mut Reader::new(&frame.payload))
                    .map_err(|e| e.to_string())?;
                let handled = if let Some(action) = action {
                    let handled = ui_services::shops::process(self, *index, *node, &action)?
                        || combat_services::process(self, *index, *node, &action)?
                        || skilling::process(self, *node, &action)?
                        || ui_services::process(self, *node, &action).map_err(|e| e.to_string())?
                        || scenery_services::process(
                            &mut self.state,
                            &mut self.host,
                            *node,
                            &action,
                        )
                        .map_err(|e| e.to_string())?
                        || ground_services::process(
                            &mut self.state,
                            &mut self.host,
                            *node,
                            &action,
                        )
                        .map_err(|e| e.to_string())?
                        || self
                            .state
                            .context(&mut self.host)
                            .process(*node, &action)
                            .map_err(|e| e.to_string())?;
                    if handled {
                        ui_services::record_action(self, *node, frame.opcode, &action);
                    }
                    handled
                } else {
                    false
                };
                if !handled && self.ignored.insert(frame.opcode) {
                    eprintln!("movement probe has no handler for opcode {}", frame.opcode);
                }
            }
        }
        Ok(())
    }
    fn exception_text(&mut self, error: String) -> Result<String, String> {
        Ok(format!("{error}\n"))
    }
    fn log(&mut self, _: &QueuedIngress, trace: &str) -> Result<(), String> {
        eprintln!("movement packet batch failed: {trace}");
        Ok(())
    }
}
impl update::Initialization<Node> for Probe {
    fn active(&mut self, node: &Node, active: bool) -> update::Result<()> {
        let a = self
            .state
            .actor_mut(*node)
            .map_err(|e| update::Error::Binding(e.to_string()))?;
        a.view.active = active;
        self.actors[&a.wire_index].player.write().active = active;
        Ok(())
    }
    fn renderable(&mut self, node: &Node, renderable: bool) -> update::Result<()> {
        // An admission whose transport disappeared before its first sync must
        // retire after initialization. List.remove itself deliberately does not
        // cancel a pending add, matching the original list contract.
        if renderable && self.retiring.contains(node) {
            self.renderables.remove(node);
        }
        Ok(())
    }
}
impl update::Context<Node> for Probe {
    fn lobby(&mut self) -> update::Result<update::List<Node>> {
        Ok(self.lobby.clone())
    }
    fn players(&mut self) -> update::Result<update::List<Node>> {
        Ok(self.renderables.clone())
    }
    fn npcs(&mut self) -> update::Result<update::List<Node>> {
        Ok(self.npcs.clone())
    }
    fn clear_minimap(&mut self, node: Node) -> update::Result<()> {
        self.host.send(
            node,
            Frame {
                opcode: 153,
                payload: vec![],
            },
        );
        Ok(())
    }
    fn npc_tick(&mut self, node: Node) -> update::Result<()> {
        npc_services::tick(self, node).map_err(update::Error::Binding)
    }
    fn npc_reset(&mut self, node: Node) -> update::Result<()> {
        npc_services::reset(self, node).map_err(update::Error::Binding)
    }
    fn player_tick(&mut self, node: Node) -> update::Result<()> {
        ui_services::restore_special(self, node)
            .map_err(|e| update::Error::Binding(e.to_string()))?;
        ui_services::shops::approach(self, node).map_err(update::Error::Binding)?;
        combat_services::approach(self, node).map_err(update::Error::Binding)?;
        self.state
            .context(&mut self.host)
            .entity_before_masks(node)
            .map_err(|e| update::Error::Binding(e.to_string()))?;
        ui_services::shops::arrive(self, node).map_err(update::Error::Binding)?;
        combat_services::tick(self, node).map_err(update::Error::Binding)?;
        skilling::tick(self, node).map_err(update::Error::Binding)?;
        self.state
            .context(&mut self.host)
            .prepare_masks(node)
            .map_err(|e| update::Error::Binding(e.to_string()))?;
        let index = self
            .state
            .actor(node)
            .map_err(|e| update::Error::Binding(e.to_string()))?
            .wire_index;
        project_player(&self.state, node, &self.actors[&index]).map_err(update::Error::Binding)
    }
    fn player_update(&mut self, node: Node) -> update::Result<()> {
        let index = self
            .state
            .actor(node)
            .map_err(|e| update::Error::Binding(e.to_string()))?
            .wire_index;
        let player = self.actors[&index].player.clone();
        let candidates = spatial_services::local(
            &mut self.state,
            &mut self.host,
            player.read().location.clone().ok_or(update::Error::Null)?,
            15,
            rs09_server::world::entity_regions::Select::Players,
        )
        .map_err(|e| update::Error::Binding(e.to_string()))?;
        let order: Vec<_> = candidates
            .iter()
            .map(|h| {
                self.state
                    .actor(h.node)
                    .map(|a| a.wire_index)
                    .map_err(|e| update::Error::Binding(e.to_string()))
            })
            .collect::<update::Result<_>>()?;
        let frames = self.host.frames.entry(node).or_default();
        if player.read().update_scene {
            frames.extend(
                render(&self.actors, &order, &self.keys, &player, true)
                    .map_err(|e| update::Error::Binding(e.to_string()))?,
            );
        }
        frames.extend(
            render(&self.actors, &order, &self.keys, &player, false)
                .map_err(|e| update::Error::Binding(e.to_string()))?,
        );
        if self.host.npc.is_some() {
            npc_services::render(self, node, &player).map_err(update::Error::Binding)?;
        }
        if self.host.ground.is_some() {
            let handle = self
                .state
                .retain_node(node)
                .map_err(|e| update::Error::Binding(e.to_string()))?;
            ground_services::render(&mut self.state, &mut self.host, &handle)
                .map_err(|e| update::Error::Binding(e.to_string()))?;
        }
        combat_services::render_projectiles(self, node, &player).map_err(update::Error::Binding)?;
        Ok(())
    }
    fn player_reset(&mut self, node: Node) -> update::Result<()> {
        let index = self
            .state
            .actor(node)
            .map_err(|e| update::Error::Binding(e.to_string()))?
            .wire_index;
        reset_player(&self.state, node, &self.actors[&index]).map_err(update::Error::Binding)
    }
    fn now_millis(&mut self) -> update::Result<i64> {
        Ok(now())
    }
    fn metric(&mut self, metric: update::Metric, millis: i32) -> update::Result<()> {
        let i = match metric {
            update::Metric::NpcTick => 0,
            update::Metric::PlayerTick => 1,
            update::Metric::PlayerRender => 2,
        };
        self.metrics[i] = millis;
        Ok(())
    }
    fn reset_chunks(&mut self) -> update::Result<()> {
        combat_services::clear_projectiles(self);
        if self.host.ground.is_some() {
            self.host.scene.clear_updates();
        }
        Ok(())
    }
    fn ground_items(&mut self) -> update::Result<()> {
        ui_services::shops::restock(self).map_err(|e| update::Error::Binding(e.to_string()))?;
        if self.host.ground.is_some() {
            ground_services::manager_pulse(&mut self.state, &mut self.host)
                .map_err(|e| update::Error::Binding(e.to_string()))?;
        }
        Ok(())
    }
}
impl tick::Context for Probe {
    type Error = String;
    fn now_millis(&mut self) -> Result<i64, String> {
        Ok(now())
    }
    fn packets(&mut self) -> Result<(), String> {
        self.ingress.clone().process(self)
    }
    fn packet_time(&mut self, _: i32) -> Result<(), String> {
        Ok(())
    }
    fn disconnects(&mut self) -> Result<(), String> {
        let pending = std::mem::take(&mut self.disconnect);
        for (index, node) in pending {
            self.remove(index, node)?;
        }
        Ok(())
    }
    fn pulses(&mut self) -> Result<(), String> {
        self.state
            .context(&mut self.host)
            .pulses()
            .map_err(|e| e.to_string())?;
        self.host.scene.collect_pulses();
        Ok(())
    }
    fn listeners(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn start(&mut self) -> Result<(), String> {
        if self.world_spawns.is_some() {
            let locations: Vec<_> = self
                .nodes
                .values()
                .filter(|node| !self.retiring.contains(node))
                .filter_map(|&node| self.state.node_location(node).ok().flatten())
                .collect();
            for location in locations {
                npc_services::open_near(self, &location);
            }
        }
        self.sequence.clone().start(self).map_err(|e| e.to_string())
    }
    fn run(&mut self) -> Result<(), String> {
        self.sequence.clone().run(self).map_err(|e| e.to_string())
    }
    fn end(&mut self) -> Result<(), String> {
        self.sequence.clone().end(self).map_err(|e| e.to_string())?;
        self.reap();
        Ok(())
    }
    fn pulse_world(&mut self) -> Result<(), String> {
        self.state.ticks = self.state.ticks.wrapping_add(1);
        self.host.animations.ticks = self.state.ticks;
        Ok(())
    }
    fn managers(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn flush(&mut self) -> Result<(), String> {
        let frames = std::mem::take(&mut self.host.frames);
        let mut dead = vec![];
        for (node, frames) in frames {
            if self.retiring.contains(&node) {
                continue;
            }
            if let Ok(a) = self.state.actor(node) {
                if self.nodes.get(&a.wire_index) != Some(&node) {
                    continue;
                }
                if let Some(actor) = self.actors.get(&a.wire_index)
                    && actor.output.try_send(frames).is_err()
                {
                    dead.push((a.wire_index, node));
                }
            }
        }
        for (index, node) in dead {
            self.remove(index, node)?;
        }
        Ok(())
    }
    fn exception(&mut self, s: String) -> Result<(), String> {
        eprintln!("movement tick failed: {s}");
        self.host.errors.push(s);
        Ok(())
    }
}
struct DefinitionLog;
impl defs::LogContext for DefinitionLog {
    fn log(&mut self, _: &mut defs::Registry, s: String) -> defs::Result<()> {
        Err(defs::Error::Binding(s))
    }
}
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn Error>> {
    run(false).await
}
pub(crate) async fn run(npc_mode: bool) -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let gameframe = args.iter().any(|a| a == "--gameframe");
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let melee = args.iter().any(|a| a == "--npc-combat");
    let woodcutting = args.iter().any(|a| a == "--woodcutting");
    let world_npcs = args.iter().any(|a| a == "--world-npcs");
    if world_npcs && !npc_mode {
        return Err("--world-npcs requires npc_session".into());
    }
    if melee && (!npc_mode || !gameframe) {
        return Err("--npc-combat requires npc_session and --gameframe".into());
    }
    if args.iter().any(|a| a == "--profiles") && option("--profiles").is_none() {
        return Err("--profiles requires a directory".into());
    }
    if option("--profiles").is_some() && !gameframe {
        return Err("--profiles requires --gameframe".into());
    }
    if woodcutting && !gameframe {
        return Err("--woodcutting requires --gameframe".into());
    }
    let tcp: std::net::SocketAddr = option("--tcp")
        .map_or(
            if npc_mode {
                "127.0.0.1:43602"
            } else {
                "127.0.0.1:43600"
            },
            String::as_str,
        )
        .parse()?;
    let websocket: std::net::SocketAddr = option("--websocket")
        .map_or(
            if npc_mode {
                "127.0.0.1:43603"
            } else {
                "127.0.0.1:43601"
            },
            String::as_str,
        )
        .parse()?;
    let account_file = option("--accounts");
    if account_file.is_none() && (!tcp.ip().is_loopback() || !websocket.ip().is_loopback()) {
        return Err(
            "network hosting requires --accounts FILE; development authentication is loopback-only"
                .into(),
        );
    }
    let authenticator = Authenticator::new(
        if account_file.is_some() {
            Mode::Production
        } else {
            Mode::Development {
                default_admin: false,
            }
        },
        Arc::new(InMemoryStorage::default()),
        Arc::new(Passwords::default()),
    );
    if let Some(path) = account_file {
        let names: BTreeMap<String, String> = serde_json::from_slice(&std::fs::read(path)?)?;
        if names.is_empty() {
            return Err("--accounts requires at least one account".into());
        }
        for (name, password) in names {
            if name
                != rs09_protocol::login::decode_name(rs09_protocol::login::encode_name(
                    name.as_bytes(),
                ))
                || name.is_empty()
                || password.len() < 8
                || password.len() > 32
                || password.as_bytes().contains(&0)
            {
                return Err("accounts require canonical names and passwords of 8–32 bytes".into());
            }
            let account = UserAccountInfo::create_default(now()).shared();
            {
                let mut fields = account
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                fields.username = name;
                fields.password = password;
            }
            if !authenticator.create_account(&account, now())? {
                return Err("could not provision an alpha account".into());
            }
        }
    }
    let data: Value = serde_json::from_slice(&std::fs::read(
        option("--keys").ok_or("--keys FILE is required")?,
    )?)?;
    let mut keys: HashMap<i32, [i32; 4]> = HashMap::new();
    for row in data["xteas"].as_array().ok_or("missing xteas")? {
        let id = row["regionId"]
            .as_str()
            .ok_or("missing regionId")?
            .parse()?;
        let values: Vec<i32> = row["keys"]
            .as_str()
            .ok_or("missing keys")?
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        keys.insert(id, values.try_into().map_err(|_| "invalid key count")?);
    }
    let limits = Limits::default();
    let store = Store::open(
        option("--cache")
            .map_or("assets/cache", String::as_str)
            .as_ref(),
        StorageMode::Resident,
        limits,
    )?;
    let catalog = Arc::new(Catalog::new(store.clone(), limits, 4096, 64 * 1024 * 1024)?);
    let variable_archive = if gameframe {
        Some(Arc::new(Archive::open(store.clone(), 22, None, limits)?))
    } else {
        None
    };
    let archive = Archive::open(store.clone(), 5, None, limits)?;
    let definition_archive = Archive::open(store.clone(), 16, None, limits)?;
    let mut definitions = defs::Registry::default();
    let count = definitions.load_archive(&mut DefinitionLog, &definition_archive, 1_000_000)?;
    let door_config = doors::Registry::default();
    let scenery = if let Some(path) = option("--door-config") {
        doors::Loader::default()
            .load_file(std::path::Path::new(path), &door_config, &mut |count| {
                println!("Loaded {count} original door configuration rows");
                Ok(())
            })
            .map_err(|e| format!("door configuration: {e:?}"))?;
        let options = rs09_server::world::scenery_options::Registry::default();
        options.register_doors()?;
        Some(options)
    } else {
        None
    };
    let mut host = Host {
        animations: prepared_animation::World::new(AnimationAssets::Cache(
            animation_defs::CacheSource(Archive::open(store.clone(), 20, None, limits)?),
        )),
        animators: BTreeMap::new(),
        masks: BTreeMap::new(),
        scene: Scene::new(
            CacheAssets {
                archive,
                keys: keys
                    .iter()
                    .map(|(&id, key)| (id, key.map(|n| n as u32)))
                    .collect(),
            },
            definitions,
            door_config,
        ),
        maps: MapServices::default(),
        active: BTreeSet::new(),
        frames: BTreeMap::new(),
        errors: vec![],
        ground: if option("--ground-spawns").is_some()
            || scenery.is_some()
            || woodcutting
            || melee
            || option("--profiles").is_some()
        {
            Some(ground_services::Ground::load(
                &Archive::open(store.clone(), 19, None, limits)?,
                std::path::Path::new(
                    option("--item-config")
                        .map_or("runs/item-config-oracle/item_configs.json", String::as_str),
                ),
            )?)
        } else {
            None
        },
        scenery,
        scenery_trace: option("--scenery-trace")
            .map(std::fs::File::create)
            .transpose()?,
        ui: if gameframe {
            Some(ui_services::Ui::load(std::path::Path::new(
                option("--interface-config")
                    .map_or("fixtures/interfaces/interface_configs.json", String::as_str),
            ))?)
        } else {
            None
        },
        npc: if npc_mode {
            Some(npc_services::Npcs::load(
                Archive::open(store, 18, None, limits)?,
                std::path::Path::new(
                    option("--npc-config")
                        .map_or("fixtures/npc-definitions/npc_configs.json", String::as_str),
                ),
                option("--npc-trace").map(std::path::Path::new),
            )?)
        } else {
            None
        },
    };
    if let (Some(ui), Some(ground)) = (&mut host.ui, &host.ground) {
        ui.use_definitions(ground.items.definitions.clone())?;
    }
    if npc_mode && let Some(ui) = &mut host.ui {
        ui.enable_shops(std::path::Path::new(
            option("--shop-config").map_or("fixtures/shops/shops.json", String::as_str),
        ))?;
    }
    if let (Some(ui), Some(archive)) = (&mut host.ui, variable_archive) {
        ui.use_variable_assets(archive)?;
    }
    if args.iter().any(|arg| arg == "--bank-probe") {
        host.ui
            .as_mut()
            .ok_or("--bank-probe requires --gameframe")?
            .enable_bank_probe()?;
        println!(
            "Bank probe controller enabled: open the actual bank once after a real1351 pickup; no item seeding or banker/PIN/dialogue admission claim."
        );
    }
    host.activate(&Location::new(3207, 3216, 0))?;
    println!(
        "Movement probe cache ready: {count} definitions, {} loaded regions",
        host.scene.regions.len()
    );
    let (joins, mut receiver) = mpsc::channel(40);
    let mut probe = Probe::new(host, keys);
    if let Some(tile) = option("--spawn") {
        let tile = tile
            .split(',')
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()?;
        probe.spawn = tile.try_into().map_err(|_| "--spawn expects X,Y,PLANE")?;
        let [x, y, z] = probe.spawn;
        if !(0..16384).contains(&x) || !(0..16384).contains(&y) || !(0..4).contains(&z) {
            return Err("--spawn is outside the world coordinate range".into());
        }
        probe.host.activate(&Location::new(x, y, z))?;
    }
    probe.profiles = option("--profiles")
        .map(|p| profiles::Store::open(p.into()))
        .transpose()?;
    if gameframe {
        let count = rs09_server::definitions::varbit::Loader::load_text(
            &probe.state.identity().varbits(),
            include_str!("../../../fixtures/variables/varbit_definitions.json"),
        )
        .map_err(|e| format!("custom variable definitions: {e:?}"))?;
        println!(
            "Loaded {count} original custom variable definitions; native UI variable writes retain canonical player state."
        );
    }
    if let Some(path) = option("--pickup-trace") {
        probe
            .host
            .ground
            .as_mut()
            .ok_or("pickup trace requires ground or door configuration")?
            .pickup_trace = Some(std::fs::File::create(path)?);
    }
    if let Some(path) = option("--ground-spawns") {
        let text = std::fs::read_to_string(path)?;
        let count = ground_services::load(&mut probe.state, &mut probe.host, &text)?;
        println!(
            "Ground fixture enabled: {count} configured native spawns, original13x13 viewport synchronization and shared manager/scheduler. Native Take uses canonical player inventory/attributes; full bank/dialogue/item plugins/audit/content, Drop, profiles and region inactivity remain open."
        );
    }
    if gameframe {
        println!(
            "Gameframe fixture enabled: native original interface ordering/metadata/counters, post-tutorial default tabs, empty inventory and content services. Full player/login/plugins/skills and tutorial configuration remain pending."
        );
    }
    if npc_mode {
        let path =
            option("--npc-spawns").map_or("fixtures/npc-spawns/npc_spawns.json", String::as_str);
        if world_npcs {
            let (spawns, count) = npc_services::WorldSpawns::load(&std::fs::read_to_string(path)?)?;
            probe.world_spawns = Some(spawns);
            println!("Indexed {count} NPC spawns from {path} for regional activation.");
        } else {
            let count = npc_services::spawn(&mut probe, std::path::Path::new(path))?;
            println!(
                "Initialized {count} NPC spawns in the 25-tile Lumbridge probe area from {path}."
            );
        }
    }
    if melee {
        probe.melee = Some(combat_services::Melee::new(now()));
        println!("NPC combat enabled: original Attack options, equipped melee and ranged attacks.");
    }
    if woodcutting {
        probe.woodcutting = Some(skilling::Woodcutting::new(now()));
        println!("Basic woodcutting enabled: ordinary trees and bronze axes.");
    }
    let (shutdown, mut stop) = tokio::sync::oneshot::channel::<()>();
    let owner = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(600));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = &mut stop => break,
                join = receiver.recv() => {
                    let Some(join) = join else { break; };
                    probe.join(join);
                },
                _ = ticker.tick() => {
                    let _ = tick::handle(&mut probe, false);
                    if let Err(error) = ui_services::bank_probe(&mut probe) { eprintln!("bank probe failed: {error}"); }
                    if probe.profiles.is_some() && probe.state.ticks % 50 == 0 { probe.save_connected(); }
                    probe.state.collect();
                    npc_services::reap(&mut probe);
                    ui_services::reap(&mut probe.host, &probe.state);
                    ground_services::reap(&mut probe.host, &probe.state);
                    probe.host.maps.players.retain(|n| probe.state.actor(*n).is_ok());
                    probe.host.maps.names.retain(|n, _| probe.state.actor(*n).is_ok());
                }
            }
        }
        probe.save_connected();
        if let Some(store) = probe.profiles.take() {
            tokio::task::spawn_blocking(move || store.finish()).await??;
        }
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    });
    let service = Service::new(
        catalog,
        Config {
            authenticator: Some(Arc::new(authenticator)),
            world: Some(Arc::new(Fixture { joins })),
            ..Config::default()
        },
        530,
    )?;
    let server = service.start(tcp, Some(websocket)).await?;
    println!(
        "Controlled movement probe TCP={} WS={:?}. Auth: {}. Full game content and capacity remain unverified.",
        server.tcp,
        server.websocket,
        if account_file.is_some() {
            "configured accounts"
        } else {
            "local development"
        },
    );
    tokio::signal::ctrl_c().await?;
    let stopped = server.shutdown().await;
    let _ = shutdown.send(());
    owner.await?.map_err(|e| e.to_string())?;
    stopped?;
    Ok(())
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../oracle/player-walk/native.rs"]
mod test_services;
#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::game::{players, rebuild};
    fn actor(animations: &prepared_animation::Factory) -> Actor {
        let (_, input) = mpsc::channel(1);
        let (output, _) = mpsc::channel(1);
        let player = Shared::new(Player::new(1, Location::new(3207, 3216, 0), 2000));
        Actor::new(
            player,
            appearance::Data::naked("walker".into(), 0),
            input,
            output,
            "walker".into(),
            animations,
        )
    }
    pub(super) fn probe() -> (Probe, Node, mpsc::Receiver<Vec<Frame>>) {
        // A portable archive is only an unused scene service here: these cases
        // test admission/removal and empty walking, never cache-map decoding.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let limits = Limits::default();
        let store =
            Store::open(&root.join("fixtures/cache"), StorageMode::Resident, limits).unwrap();
        let archive = Archive::open(store, 3, None, limits).unwrap();
        let host = Host {
            animations: prepared_animation::World::new(AnimationAssets::Unbound),
            animators: BTreeMap::new(),
            masks: BTreeMap::new(),
            scene: Scene::new(
                CacheAssets {
                    archive,
                    keys: HashMap::new(),
                },
                defs::Registry::default(),
                doors::Registry::default(),
            ),
            maps: MapServices::default(),
            active: BTreeSet::new(),
            frames: BTreeMap::new(),
            errors: vec![],
            ui: None,
            npc: None,
            ground: None,
            scenery: None,
            scenery_trace: None,
        };
        let mut p = Probe::new(host, HashMap::new());
        let location = Location::new(3207, 3216, 0);
        let node = p.state.add(1, location.clone(), [1, 2, 3, 4]);
        p.state.actor_mut(node).unwrap().view.active = false;
        {
            let q = &p.state.actor(node).unwrap().queue;
            let mut fields = q.state.write();
            fields.entity.last_scene = Some(location);
            fields.entity.update_scene = true;
        }
        p.leases.insert(node, p.state.retain_node(node).unwrap());
        let mut a = actor(&p.host.animations.factory);
        let owner = p.state.retain_node(node).unwrap();
        a.animator = p
            .host
            .animations
            .factory
            .actor_with_clocks(a.masks.clone(), p.state.clocks(&owner).unwrap());
        a.player.write().active = false;
        let (output, outgoing) = mpsc::channel(2);
        a.output = output;
        p.host.masks.insert(node, a.masks.clone());
        p.host.animators.insert(node, a.animator.clone());
        p.actors.insert(1, a);
        p.nodes.insert(1, node);
        p.order.push(1);
        p.host.maps.names.insert(node, "walker".into());
        p.host.active.insert((50 << 8) | 50);
        spatial_services::move_entity(
            &mut p.state,
            &mut p.host,
            node,
            None,
            Location::new(3207, 3216, 0),
        )
        .unwrap();
        p.renderables.add(node);
        (p, node, outgoing)
    }
    #[test]
    fn deferred_admission_renders_next_tick_and_disconnect_releases_world_owners() {
        let (mut p, node, mut output) = probe();
        tick::handle(&mut p, false).unwrap();
        assert_eq!(p.state.ticks, 1);
        assert!(output.try_recv().is_err());
        assert_eq!(p.renderables.values(), [node]);
        assert!(p.actors[&1].player.read().active);
        tick::handle(&mut p, false).unwrap();
        let frames = output.try_recv().unwrap();
        let mut receiver = Receiver::new();
        receiver.receive(&frames, 2);
        assert_eq!(receiver.position(), [3207, 3216, 0]);
        p.remove(1, node).unwrap();
        p.state.collect();
        assert_eq!(p.state.retained_counts()[0], 1);
        tick::handle(&mut p, false).unwrap();
        p.state.collect();
        assert!(p.actors.is_empty());
        assert!(p.nodes.is_empty());
        assert!(p.leases.is_empty());
        assert!(p.retiring.is_empty());
        assert!(p.renderables.values().is_empty());
        assert_eq!(p.state.retained_counts(), [0, 0, 0]);
        assert!(p.host.errors.is_empty());
    }
    #[test]
    fn owned_tick_renders_and_resets_masks_and_actual_host_callbacks_preserve_synced_state() {
        use rs09_protocol::game::players::{Field, State as Wire};
        let (mut p, node, mut output) = probe();
        tick::handle(&mut p, false).unwrap();
        let masks = p.actors[&1].masks.clone();
        masks
            .face_location(Some(Location::new(3210, 3216, 0)))
            .unwrap();
        masks.animate(535, 0).unwrap();
        tick::handle(&mut p, false).unwrap();
        let frames = output.try_recv().unwrap();
        let own = frames.iter().find(|f| f.opcode == 225).unwrap();
        let update = Wire::new(55, 48).decode_frame(own, 1).unwrap();
        assert!(
            update
                .extended
                .iter()
                .flat_map(|e| &e.fields)
                .any(|f| matches!(f, Field::Animation { id: 535, delay: 0 }))
        );
        assert!(
            update
                .extended
                .iter()
                .flat_map(|e| &e.fields)
                .any(|f| matches!(f, Field::FaceTile([6421, 6433])))
        );
        assert!(!masks.status().updating);
        assert!(!masks.status().update_required());
        assert!(masks.status().has_synced());
        player_walk::Services::event(&mut p.host, &mut p.state, Event::Masks(node)).unwrap();
        assert!(masks.status().updating);
        assert!(!masks.animate(-1, 0).unwrap());
        player_walk::Services::event(&mut p.host, &mut p.state, Event::FaceLocationNone(node))
            .unwrap();
        assert_eq!(masks.status().synced_presence, 4);
        reset_player(&p.state, node, &p.actors[&1]).unwrap();
        assert!(!masks.status().updating);
        assert_eq!(masks.status().synced_presence, 4);
    }
    #[test]
    fn self_facing_capture_and_host_mask_view_release_on_actual_disconnect() {
        let (mut p, node, mut output) = probe();
        let weak = p.actors[&1].player.downgrade();
        tick::handle(&mut p, false).unwrap();
        player_walk::Services::event(&mut p.host, &mut p.state, Event::Face(node, Some(node)))
            .unwrap();
        assert_eq!(p.actors[&1].masks.status().synced_presence, 6);
        tick::handle(&mut p, false).unwrap();
        let _ = output.try_recv().unwrap();
        p.remove(1, node).unwrap();
        p.state.collect();
        assert_eq!(p.state.retained_counts()[0], 1);
        tick::handle(&mut p, false).unwrap();
        p.state.collect();
        assert!(p.actors.is_empty());
        assert!(p.host.masks.is_empty());
        assert!(p.host.animators.is_empty());
        assert_eq!(p.state.retained_counts(), [0, 0, 0]);
        assert!(weak.upgrade().is_none());
        assert!(p.host.errors.is_empty());
    }
    #[test]
    fn actual_actor_animator_keeps_world_timing_masks_and_retirement_on_the_same_owner() {
        use rs09_protocol::game::players::{Field, State as Wire};
        use rs09_server::world::animation::{Animation, Graphics, Priority};
        let (mut p, node, mut output) = probe();
        tick::handle(&mut p, false).unwrap();
        tick::handle(&mut p, false).unwrap();
        let mut wire = Wire::new(0, 0);
        for frame in output.try_recv().unwrap() {
            if frame.opcode == 225 {
                wire.decode_frame(&frame, 1).unwrap();
            }
        }
        let d = Shared::new(animation_defs::Definition {
            durations: Some(Shared::new(vec![90])),
            ..animation_defs::Definition::default()
        });
        p.host
            .animations
            .definitions
            .entries
            .write()
            .insert(535, Some(d));
        let a = p.host.animators[&node].clone();
        assert!(a.same(&p.actors[&1].animator));
        assert_eq!(p.host.animations.ticks, p.state.ticks);
        let current = p.state.ticks;
        a.animate(
            &mut p.host.animations,
            Some(Animation::with(535, 7, Some(Priority::High))),
            Some(Graphics::new(111, 128, 60)),
        )
        .unwrap();
        assert_eq!(
            a.owner().unwrap().clocks().read()[1],
            current.wrapping_add(3)
        );
        assert!(
            !a.animate(&mut p.host.animations, Some(Animation::new(863)), None)
                .unwrap()
        );
        tick::handle(&mut p, false).unwrap();
        let mut found = false;
        for frame in output.try_recv().unwrap() {
            if frame.opcode == 225 {
                let update = wire.decode_frame(&frame, 2).unwrap();
                found = update
                    .extended
                    .iter()
                    .flat_map(|e| &e.fields)
                    .any(|f| matches!(f, Field::Animation { id: 535, delay: 7 }));
            }
        }
        assert!(found);
        assert!(!p.actors[&1].masks.status().updating);
        let weak = p.actors[&1].player.downgrade();
        p.remove(1, node).unwrap();
        tick::handle(&mut p, false).unwrap();
        p.state.collect();
        assert!(p.host.animators.is_empty());
        assert!(weak.upgrade().is_some());
        drop(a);
        assert!(weak.upgrade().is_none());
        assert!(p.host.errors.is_empty());
    }
    #[test]
    fn scene_membership_filters_players_on_addition_and_preserves_chunk_crossings() {
        let (mut p, _, mut out) = probe();
        let far = Location::new(3230, 3216, 0);
        let other = p.state.add(2, far.clone(), [1, 2, 3, 4]);
        p.leases.insert(other, p.state.retain_node(other).unwrap());
        p.host.maps.names.insert(other, "other".into());
        spatial_services::move_entity(&mut p.state, &mut p.host, other, None, far.clone()).unwrap();
        let mut a = actor(&p.host.animations.factory);
        a.username = "other".into();
        {
            let mut player = a.player.write();
            player.index = 2;
            player.location = Some(far.clone());
            player.active = false;
        }
        let (output, mut other_output) = mpsc::channel(2);
        a.output = output;
        p.host.masks.insert(other, a.masks.clone());
        p.host.animators.insert(other, a.animator.clone());
        p.actors.insert(2, a);
        p.nodes.insert(2, other);
        p.order.push(2);
        p.renderables.add(other);
        tick::handle(&mut p, false).unwrap();
        tick::handle(&mut p, false).unwrap();
        let mut state = players::State::new(0, 0);
        for f in out.try_recv().unwrap() {
            if f.opcode == 225 {
                state.decode_frame(&f, 1).unwrap();
            }
        }
        assert!(
            state.local_ids.is_empty(),
            "distant candidates must not wrap into the local scene"
        );
        other_output.try_recv().unwrap();
        let near = Location::new(3222, 3216, 0);
        p.state
            .actor(other)
            .unwrap()
            .queue
            .state
            .write()
            .entity
            .location = Some(near.clone());
        spatial_services::move_entity(&mut p.state, &mut p.host, other, Some(far), near.clone())
            .unwrap();
        tick::handle(&mut p, false).unwrap();
        for f in out.try_recv().unwrap() {
            if f.opcode == 225 {
                state.decode_frame(&f, 2).unwrap();
            }
        }
        assert_eq!(state.local_ids, vec![2]);
        other_output.try_recv().unwrap();
        let above = Location::new(3222, 3216, 1);
        p.state
            .actor(other)
            .unwrap()
            .queue
            .state
            .write()
            .entity
            .location = Some(above.clone());
        spatial_services::move_entity(&mut p.state, &mut p.host, other, Some(near), above).unwrap();
        tick::handle(&mut p, false).unwrap();
        for f in out.try_recv().unwrap() {
            if f.opcode == 225 {
                state.decode_frame(&f, 3).unwrap();
            }
        }
        assert!(
            state.local_ids.is_empty(),
            "other planes are not local candidates"
        );
        assert!(p.host.errors.is_empty(), "{:?}", p.host.errors);
    }
    #[test]
    fn cancelled_admission_before_first_sync_does_not_leave_a_ghost_player() {
        let (mut p, node, mut output) = probe();
        p.remove(1, node).unwrap();
        assert_eq!(p.renderables.pending(), [(node, false)]);
        p.state.collect();
        assert_eq!(p.state.retained_counts()[0], 1);
        tick::handle(&mut p, false).unwrap();
        p.state.collect();
        assert!(p.renderables.values().is_empty());
        assert!(p.renderables.pending().is_empty());
        assert!(p.actors.is_empty());
        assert!(p.leases.is_empty());
        assert_eq!(p.state.retained_counts(), [0, 0, 0]);
        assert!(output.try_recv().is_err());
        assert!(p.host.errors.is_empty());
    }
    #[test]
    fn late_ingress_cannot_move_a_new_player_reusing_the_wire_index() {
        let (mut p, old, _) = probe();
        p.remove(1, old).unwrap();
        tick::handle(&mut p, false).unwrap();
        p.state.collect();
        let new = p.state.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
        assert_ne!(old, new);
        p.nodes.insert(1, new);
        let frame = rs09_protocol::game::actions::worldspace_walk(3211, 3215, true);
        p.ingress.enqueue(Packet::Dispatch(QueuedIngress {
            event: Ingress::Frame(1, old, frame),
            _permit: p.budget.clone().try_acquire_owned().unwrap(),
        }));
        p.ingress.enqueue(Packet::Dispatch(QueuedIngress {
            event: Ingress::Closed(1, old),
            _permit: p.budget.clone().try_acquire_owned().unwrap(),
        }));
        tick::Context::packets(&mut p).unwrap();
        tick::Context::disconnects(&mut p).unwrap();
        assert!(p.state.current(new).unwrap().is_none());
        assert!(p.state.actor(new).unwrap().connected);
        assert_eq!(p.budget.available_permits(), 4096);
        assert!(p.host.errors.is_empty());
    }
    struct Receiver {
        state: players::State,
        rebuild: Option<rebuild::NormalRebuild>,
        rebuilds: usize,
    }
    #[test]
    fn scene_restore_uses_the_probe_global_pulse_phase_once_per_tick() {
        use rs09_server::world::scenery;
        let (mut probe, _, _) = probe();
        let original = Shared::new(
            doors::Scenery::new(
                1,
                Location::new(3207, 3217, 0),
                0,
                1,
                Some("closed".into()),
                false,
            )
            .unwrap(),
        );
        let replacement = Shared::new(
            doors::Scenery::new(
                2,
                Location::new(3207, 3217, 0),
                0,
                2,
                Some("opened".into()),
                false,
            )
            .unwrap(),
        );
        scenery::replace(
            &mut probe.host.scene.context(&mut probe.host.maps),
            Some(&original),
            Some(&replacement),
            scenery::ReplaceMode::Temporary {
                clip: false,
                ticks: 2,
            },
        )
        .unwrap();
        assert_eq!(probe.state.pending().len(), 1);
        probe.host.scene.clear_updates();
        tick::Context::pulses(&mut probe).unwrap();
        assert_eq!(probe.state.pending()[0].read().ticks_passed, 1);
        assert!(probe.host.scene.updates.is_empty());
        tick::Context::pulses(&mut probe).unwrap();
        assert!(probe.state.pending().is_empty());
        assert_eq!(probe.host.scene.retained_task_count(), 0);
        let records = probe
            .host
            .scene
            .updates
            .values()
            .flatten()
            .map(|u| (u.object.read().id, u.remove))
            .collect::<Vec<_>>();
        assert_eq!(records, [(2, true), (1, false)]);
        assert!(probe.host.errors.is_empty());
    }
    impl Receiver {
        fn new() -> Self {
            Self {
                state: players::State::new(0, 0),
                rebuild: None,
                rebuilds: 0,
            }
        }
        fn receive(&mut self, frames: &[Frame], tick: i32) {
            for f in frames {
                match f.opcode {
                    162 => {
                        let next = rebuild::decode_normal(f).unwrap();
                        if let Some(previous) = &self.rebuild {
                            let a = previous.base();
                            let b = next.base();
                            self.state.rebase([b[0] - a[0], b[1] - a[1]]);
                        }
                        self.state.plane = next.plane;
                        self.rebuild = Some(next);
                        self.rebuilds += 1;
                    }
                    225 => {
                        self.state.decode_frame(f, tick).unwrap();
                    }
                    _ => panic!("unexpected render frame {}", f.opcode),
                }
            }
        }
        fn position(&self) -> [i32; 3] {
            let base = self.rebuild.as_ref().unwrap().base();
            let p = self.state.self_player();
            [
                base[0] + p.queue_x[0],
                base[1] + p.queue_y[0],
                self.state.plane,
            ]
        }
    }
    fn publish(state: &State, node: Node, actors: &BTreeMap<i32, Actor>, receiver: &mut Receiver) {
        let nodes = BTreeMap::from([(1, node)]);
        let frames = project_players(state, actors, &[1], &nodes, &HashMap::new()).unwrap();
        receiver.receive(&frames[&node], state.ticks);
        reset_players(state, actors, &nodes).unwrap();
    }
    #[test]
    fn native_walk_and_run_updates_reach_the_shared_wire_consumer() {
        for running in [false, true] {
            let (mut state, mut host) = test_services::create(&serde_json::json!({}));
            let node = host.player;
            let actors = BTreeMap::from([(1, actor(&prepared_animation::Factory::default()))]);
            let mut receiver = Receiver::new();
            state
                .actor(node)
                .unwrap()
                .queue
                .state
                .write()
                .entity
                .update_scene = true;
            publish(&state, node, &actors, &mut receiver);
            assert_eq!(receiver.position(), [3207, 3216, 0]);
            host.act(
                &mut state,
                &serde_json::json!({"op":"request","x":3209,"y":3216,"running":running}),
            )
            .unwrap();
            host.act(&mut state, &serde_json::json!({"op":"tick"}))
                .unwrap();
            publish(&state, node, &actors, &mut receiver);
            assert_eq!(
                receiver.position(),
                [if running { 3209 } else { 3208 }, 3216, 0]
            );
            host.act(&mut state, &serde_json::json!({"op":"tick"}))
                .unwrap();
            publish(&state, node, &actors, &mut receiver);
            assert_eq!(receiver.position(), [3209, 3216, 0]);
        }
    }
    #[test]
    fn continued_walking_rebuilds_the_scene_and_preserves_absolute_wire_positions() {
        let (mut state, mut host) = test_services::create(&serde_json::json!({}));
        let node = host.player;
        let actors = BTreeMap::from([(1, actor(&prepared_animation::Factory::default()))]);
        let mut receiver = Receiver::new();
        state
            .actor(node)
            .unwrap()
            .queue
            .state
            .write()
            .entity
            .update_scene = true;
        publish(&state, node, &actors, &mut receiver);
        for x in [3231, 3255, 3279] {
            host.act(
                &mut state,
                &serde_json::json!({"op":"request","x":x,"y":3216,"running":true}),
            )
            .unwrap();
            for _ in 0..64 {
                host.act(&mut state, &serde_json::json!({"op":"tick"}))
                    .unwrap();
                publish(&state, node, &actors, &mut receiver);
                assert_eq!(
                    receiver.position(),
                    state
                        .actor(node)
                        .unwrap()
                        .queue
                        .entity()
                        .location
                        .unwrap()
                        .coordinates()
                );
                if receiver.position() == [x, 3216, 0] {
                    break;
                }
            }
            assert_eq!(receiver.position(), [x, 3216, 0]);
        }
        assert!(receiver.rebuilds >= 3);
    }
}
