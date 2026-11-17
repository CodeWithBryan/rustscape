//! Explicit empty-content NPC fixture bindings. Uses pinned metadata/spawns,
//! native initialization/tick/search/queue/render kernels and real scene collision.
//! Concrete subclasses/combat/skills/timers/region-viewer startup remain separate.
use super::{Frame, Host, Location, Node, Probe, Shared, State};
use rs09_cache::archive::Archive;
use rs09_game_core::{direction::Direction, random::JavaRandom};
use rs09_server::{
    definitions::{
        npc::{self as defs, Graphics, Registry, config},
        property::{Animation, Property},
    },
    world::{
        npc,
        npc_init::{self, Change, Context, Skill, Slot},
        npc_render, npc_spawns, npc_walk, player_render, player_walk,
    },
};
use std::{collections::BTreeMap, io::Write, path::Path};
pub(super) struct Npcs {
    defs: Registry,
    archive: Archive,
    rows: BTreeMap<Node, Row>,
    view_owners: BTreeMap<usize, Node>,
    next_index: i32,
    rng: JavaRandom,
    trace: Option<Trace>,
}
struct Trace {
    file: std::fs::File,
    updates: BTreeMap<Node, usize>,
}
struct Row {
    brain: npc::State,
    def: defs::Handle,
    view: npc_render::Handle,
    props: Props,
    statics: [i32; 24],
    levels: [i32; 24],
    chat: Option<Vec<u16>>,
    tolerance: BTreeMap<i32, i32>,
    attributes: BTreeMap<&'static str, bool>,
    melee_animation: Option<(i32, i32)>,
    melee_hit: Option<(i32, i32)>,
    secondary_hit: Option<(i32, i32)>,
    combat_spot: Option<(i32, i32)>,
    poison: Option<(i32, i32)>,
    defence_drain: i32,
    melee_face: Option<i32>,
    death_end: Option<i32>,
}
#[derive(Default)]
struct Props {
    spawn: Option<Location>,
    combat: i32,
    speed: i32,
    bonuses: Option<Shared<Vec<i32>>>,
    animations: [Option<Shared<Animation>>; 5],
    death: Option<Shared<Graphics>>,
}
struct Files<'a>(&'a Archive);
impl defs::Files for Files<'_> {
    fn file(&mut self, _: &mut Registry, id: i32) -> defs::Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .file((id as u32) >> 7, (id & 127) as u32, [0; 4])?
            .map(|b| b.as_ref().clone()))
    }
}
impl config::Context for Files<'_> {
    fn log(&mut self, _: &mut Registry, _: config::Level, _: String) -> defs::Result<()> {
        Ok(())
    }
}
impl Npcs {
    pub(super) fn load(
        archive: Archive,
        path: &Path,
        trace: Option<&Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut defs = Registry::default();
        defs.load_archive(&archive, 1_000_000)?;
        config::Parser::default().load_file(path, &mut defs, &mut Files(&archive))?;
        Ok(Self {
            defs,
            archive,
            rows: BTreeMap::new(),
            view_owners: BTreeMap::new(),
            next_index: 1,
            rng: JavaRandom::new(530),
            trace: trace
                .map(|p| {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(p)
                        .map(|file| Trace {
                            file,
                            updates: BTreeMap::new(),
                        })
                })
                .transpose()?,
        })
    }
    fn definition(&mut self, id: i32) -> defs::Result<defs::Handle> {
        self.defs.for_id(&mut Files(&self.archive), id)
    }
}
fn error(e: impl std::fmt::Display) -> npc_init::Error {
    npc_init::Error::Binding(e.to_string())
}
fn slot(s: Slot) -> usize {
    match s {
        Slot::Attack => 0,
        Slot::Defence => 1,
        Slot::Death => 2,
        Slot::Magic => 3,
        Slot::Range => 4,
    }
}
struct Init<'a> {
    p: &'a mut Probe,
    roots: Vec<player_walk::NodeHandle>,
}
impl Init<'_> {
    fn store(&self) -> &Npcs {
        self.p.host.npc.as_ref().unwrap()
    }
    fn store_mut(&mut self) -> &mut Npcs {
        self.p.host.npc.as_mut().unwrap()
    }
    fn row(&self, n: Node) -> npc_init::Result<&Row> {
        self.store().rows.get(&n).ok_or(npc_init::Error::Null)
    }
    fn row_mut(&mut self, n: Node) -> npc_init::Result<&mut Row> {
        self.store_mut()
            .rows
            .get_mut(&n)
            .ok_or(npc_init::Error::Null)
    }
}
impl Context for Init<'_> {
    type Arguments = ();
    fn prototype(&mut self, _: i32) -> npc_init::Result<Option<Node>> {
        Ok(None)
    }
    fn construct(
        &mut self,
        _: Node,
        _: i32,
        _: Option<Location>,
        _: &(),
    ) -> npc_init::Result<Option<Node>> {
        Err(error("content prototypes are not installed in NPC fixture"))
    }
    fn base(
        &mut self,
        id: i32,
        l: Option<Location>,
        d: Option<Direction>,
    ) -> npc_init::Result<Node> {
        let def = self.store_mut().definition(id)?;
        let size = def.read().size;
        let node = self.p.state.add_npc_at(0, l.clone(), [1, 2, 3, 4]);
        {
            let a = self.p.state.actor_mut(node).map_err(error)?;
            a.connected = false;
            a.view.active = false;
            a.view.size = size;
            a.queue.state.write().entity.direction = d;
        }
        self.roots
            .push(self.p.state.retain_node(node).map_err(error)?);
        let brain = npc::State::new(l.clone().unwrap_or_else(|| Location::new(0, 0, 0)));
        {
            let mut f = brain.fields.write();
            f.location = l.clone();
            f.spawn = None;
        }
        let mut view =
            npc_render::Npc::new(0, id, l.clone().unwrap_or_else(|| Location::new(0, 0, 0)));
        view.location = l;
        let view = Shared::new(view);
        let store = self.store_mut();
        store.view_owners.insert(view.identity(), node);
        store.rows.insert(
            node,
            Row {
                brain,
                def,
                view,
                props: Props::default(),
                statics: [0; 24],
                levels: [0; 24],
                chat: None,
                tolerance: BTreeMap::new(),
                attributes: BTreeMap::new(),
                melee_animation: None,
                melee_hit: None,
                secondary_hit: None,
                combat_spot: None,
                poison: None,
                defence_drain: 0,
                melee_face: None,
                death_end: None,
            },
        );
        Ok(node)
    }
    fn definition(&self, n: Node) -> npc_init::Result<Option<defs::Handle>> {
        Ok(Some(self.row(n)?.def.clone()))
    }
    fn brain(&self, n: Node) -> npc_init::Result<npc::State> {
        Ok(self.row(n)?.brain.clone())
    }
    fn location(&self, n: Node) -> npc_init::Result<Option<Location>> {
        Ok(self
            .p
            .state
            .actor(n)
            .map_err(error)?
            .queue
            .entity()
            .location)
    }
    fn index(&self, n: Node) -> npc_init::Result<i32> {
        Ok(self.p.state.actor(n).map_err(error)?.wire_index)
    }
    fn size(&self, n: Node) -> npc_init::Result<i32> {
        Ok(self.p.state.actor(n).map_err(error)?.view.size)
    }
    fn set_index(&mut self, n: Node, i: i32) -> npc_init::Result<()> {
        self.p.state.actor_mut(n).map_err(error)?.wire_index = i;
        self.row_mut(n)?.view.write().index = i;
        Ok(())
    }
    fn set_size(&mut self, n: Node, s: i32) -> npc_init::Result<()> {
        self.p.state.actor_mut(n).map_err(error)?.view.size = s;
        Ok(())
    }
    fn children(&self, n: Node) -> npc_init::Result<Option<Shared<Vec<Option<Node>>>>> {
        self.p.state.children(n).map_err(error)
    }
    fn set_children(&mut self, n: Node, c: Shared<Vec<Option<Node>>>) -> npc_init::Result<()> {
        self.p.state.set_children(n, c).map_err(error)
    }
    fn entity_init(&mut self, n: Node) -> npc_init::Result<()> {
        self.p.state.actor_mut(n).map_err(error)?.view.active = true;
        Ok(())
    }
    fn properties(&mut self, n: Node) -> npc_init::Result<Option<usize>> {
        self.row(n)?;
        Ok(Some(n))
    }
    fn skills(&mut self, n: Node) -> npc_init::Result<Option<usize>> {
        self.row(n)?;
        Ok(Some(n))
    }
    fn set_property(&mut self, n: Option<usize>, v: Change) -> npc_init::Result<()> {
        let p = &mut self.row_mut(n.ok_or(npc_init::Error::Null)?)?.props;
        match v {
            Change::CombatLevel(v) => p.combat = v,
            Change::Animation(s, a) => p.animations[slot(s)] = a,
            Change::Bonuses(a) => p.bonuses = Some(a),
            Change::AttackSpeed(v) => p.speed = v,
            Change::DeathGraphic(g) => p.death = Some(g),
        }
        Ok(())
    }
    fn get_animation(
        &mut self,
        n: Option<usize>,
        s: Slot,
    ) -> npc_init::Result<Option<Shared<Animation>>> {
        Ok(self.row(n.ok_or(npc_init::Error::Null)?)?.props.animations[slot(s)].clone())
    }
    fn set_skill(&mut self, n: Option<usize>, s: Skill, id: i32, v: i32) -> npc_init::Result<()> {
        let n = self.row_mut(n.ok_or(npc_init::Error::Null)?)?;
        match s {
            Skill::Static => n.statics[id as usize] = v,
            Skill::Current => n.levels[id as usize] = v,
        }
        Ok(())
    }
    fn maximum_life_points(&mut self, n: Option<usize>) -> npc_init::Result<i32> {
        Ok(self.row(n.ok_or(npc_init::Error::Null)?)?.statics[3])
    }
    fn familiar(&self, _: Node) -> npc_init::Result<bool> {
        Ok(false)
    }
    fn attribute_hp(&mut self, _: Node, _: i32) -> npc_init::Result<i32> {
        Err(error("no familiar prototypes installed"))
    }
    fn spawn_location(&mut self, n: Option<usize>, l: Option<Location>) -> npc_init::Result<()> {
        let row = self.row_mut(n.ok_or(npc_init::Error::Null)?)?;
        row.props.spawn = l.clone();
        row.brain.fields.write().spawn = l;
        Ok(())
    }
    fn add_repository(&mut self, n: Node) -> npc_init::Result<()> {
        let index = self.store().next_index;
        if index >= 32767 {
            return Err(error("NPC wire index exhausted"));
        }
        self.store_mut().next_index += 1;
        self.set_index(n, index)?;
        self.p.state.actor_mut(n).map_err(error)?.connected = true;
        Ok(())
    }
    fn region_move(
        &mut self,
        node: Node,
        from: Option<Location>,
        to: Option<Location>,
    ) -> npc_init::Result<()> {
        super::spatial_services::move_entity(
            &mut self.p.state,
            &mut self.p.host,
            node,
            from,
            to.ok_or(npc_init::Error::Null)?,
        )
        .map_err(error)
    }
    fn region_active(&mut self, l: Location) -> npc_init::Result<bool> {
        let [x, y, _] = l.coordinates();
        Ok(self.p.host.active.contains(&(((x >> 6) << 8) | (y >> 6))))
    }
    fn add_renderable(&mut self, n: Node) -> npc_init::Result<()> {
        self.p.npcs.add(n);
        Ok(())
    }
    fn interact_default(&mut self, _: Node) -> npc_init::Result<()> {
        Ok(())
    }
    fn configure(&mut self, _: Node) -> npc_init::Result<()> {
        Ok(())
    }
    fn default_behavior(&mut self, n: Node) -> npc_init::Result<()> {
        let row = self.row_mut(n)?;
        let aggressive = row.brain.fields.read().aggressive;
        if aggressive && row.def.read().combat_level > 0 {
            row.brain.fields.write().has_handler = true;
            row.view.write().aggressive = true;
        }
        Ok(())
    }
    fn behavior_created(&mut self, _: Node) -> npc_init::Result<()> {
        Ok(())
    }
}
struct Spawn<'a> {
    init: Init<'a>,
    lumbridge_only: bool,
    count: usize,
}

impl npc_spawns::Context for Spawn<'_> {
    type Npc = Option<Node>;
    fn create(&mut self, id: i32, l: Location) -> npc_spawns::Result<Option<Node>> {
        if self.lumbridge_only {
            let [x, y, _] = l.coordinates();
            if (x - 3207).abs() > 25 || (y - 3216).abs() > 25 {
                return Ok(None);
            }
            self.init
                .p
                .host
                .activate(&l)
                .map_err(|e| npc_spawns::Error::Binding(e.to_string()))?;
        }
        npc_init::create(&mut self.init, id, Some(l), Some(Direction::South), &())
            .map(Some)
            .map_err(|e| npc_spawns::Error::Binding(e.to_string()))
    }
    fn walks(&mut self, n: &Option<Node>, v: bool) -> npc_spawns::Result<()> {
        if let Some(n) = n {
            self.init
                .store_mut()
                .rows
                .get_mut(n)
                .unwrap()
                .brain
                .fields
                .write()
                .walks = v;
        }
        Ok(())
    }
    fn direction(&mut self, n: &Option<Node>, d: Direction) -> npc_spawns::Result<()> {
        if let Some(n) = n {
            self.init
                .p
                .state
                .actor(*n)
                .unwrap()
                .queue
                .state
                .write()
                .entity
                .direction = Some(d);
        }
        Ok(())
    }
    fn attribute(
        &mut self,
        n: &Option<Node>,
        key: &'static str,
        value: bool,
    ) -> npc_spawns::Result<()> {
        if let Some(n) = n {
            self.init
                .store_mut()
                .rows
                .get_mut(n)
                .unwrap()
                .attributes
                .insert(key, value);
        }
        Ok(())
    }
    fn facing_booth(&mut self, n: &Option<Node>) -> npc_spawns::Result<bool> {
        let Some(n) = n else {
            return Ok(false);
        };
        let handlers = self.init.store().rows[n].def.read().handlers.clone();
        match handlers.read().get("facing_booth") {
            None | Some(Property::Null) => Ok(false),
            Some(Property::Bool(value)) => Ok(*value),
            _ => Err(npc_spawns::Error::Type),
        }
    }
    fn init(&mut self, n: &Option<Node>) -> npc_spawns::Result<()> {
        if let Some(n) = n {
            npc_init::init(&mut self.init, *n)
                .map_err(|e| npc_spawns::Error::Binding(e.to_string()))?;
            self.count += 1;
        }
        Ok(())
    }
    fn log(&mut self, _: i32) -> npc_spawns::Result<()> {
        Ok(())
    }
}
pub(super) fn spawn(p: &mut Probe, path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let mut ctx = Spawn {
        init: Init { p, roots: vec![] },
        lumbridge_only: true,
        count: 0,
    };
    npc_spawns::load(&mut ctx, &std::fs::read_to_string(path)?)?;
    Ok(ctx.count)
}

struct SpawnPlacement {
    id: i32,
    location: Location,
    walks: bool,
    direction: Direction,
}

#[derive(Default)]
pub(super) struct WorldSpawns {
    regions: BTreeMap<(i32, i32), Vec<SpawnPlacement>>,
}

impl npc_spawns::Context for WorldSpawns {
    type Npc = ((i32, i32), usize);

    fn create(&mut self, id: i32, location: Location) -> npc_spawns::Result<Self::Npc> {
        let [x, y, _] = location.coordinates();
        let region = (x >> 6, y >> 6);
        let entries = self.regions.entry(region).or_default();
        let index = entries.len();
        entries.push(SpawnPlacement {
            id,
            location,
            walks: false,
            direction: Direction::South,
        });
        Ok((region, index))
    }
    fn walks(&mut self, (r, i): &Self::Npc, walks: bool) -> npc_spawns::Result<()> {
        self.regions.get_mut(r).unwrap()[*i].walks = walks;
        Ok(())
    }
    fn direction(&mut self, (r, i): &Self::Npc, direction: Direction) -> npc_spawns::Result<()> {
        self.regions.get_mut(r).unwrap()[*i].direction = direction;
        Ok(())
    }
    fn attribute(&mut self, _: &Self::Npc, _: &'static str, _: bool) -> npc_spawns::Result<()> {
        Ok(())
    }
    // Apply definition-dependent attributes and NPC.init when the region opens.
    fn facing_booth(&mut self, _: &Self::Npc) -> npc_spawns::Result<bool> {
        Ok(false)
    }
    fn init(&mut self, _: &Self::Npc) -> npc_spawns::Result<()> {
        Ok(())
    }
    fn log(&mut self, _: i32) -> npc_spawns::Result<()> {
        Ok(())
    }
}

impl WorldSpawns {
    pub(super) fn load(text: &str) -> npc_spawns::Result<(Self, i32)> {
        let mut world = Self::default();
        let count = npc_spawns::load(&mut world, text)?;
        Ok((world, count))
    }
}

pub(super) fn open_near(p: &mut Probe, location: &Location) {
    use npc_spawns::Context as _;

    if p.world_spawns.is_none() {
        return;
    }
    // The renderer looks 15 tiles out. Include this tick's two running steps.
    let [x, y, _] = location.coordinates();
    for rx in ((x - 17) >> 6).max(0)..=((x + 17) >> 6).min(255) {
        for ry in ((y - 17) >> 6).max(0)..=((y + 17) >> 6).min(255) {
            let Some(rows) = p.world_spawns.as_mut().unwrap().regions.remove(&(rx, ry)) else {
                continue;
            };
            if let Err(error) = p.host.activate(&Location::new(rx << 6, ry << 6, 0)) {
                eprintln!("NPC region {rx},{ry} could not be loaded: {error}");
                continue;
            }
            let mut spawn = Spawn {
                init: Init { p, roots: vec![] },
                lumbridge_only: false,
                count: 0,
            };
            for row in rows {
                let result = (|| {
                    let npc = spawn.create(row.id, row.location)?;
                    spawn.walks(&npc, row.walks)?;
                    spawn.direction(&npc, row.direction)?;
                    spawn.attribute(&npc, "spawned:npc", true)?;
                    if spawn.facing_booth(&npc)? {
                        spawn.attribute(&npc, "facing_booth", true)?;
                    }
                    spawn.init(&npc)
                })();
                if let Err(error) = result {
                    eprintln!(
                        "NPC {} in region {rx},{ry} could not be spawned: {error}",
                        row.id
                    );
                }
            }
        }
    }
}
pub(super) fn view(host: &Host, n: Node) -> Option<npc_render::Handle> {
    host.npc.as_ref()?.rows.get(&n).map(|row| row.view.clone())
}

pub(super) fn visible(p: &Probe, player: i32, index: i32) -> Option<Node> {
    if !(1..32767).contains(&index) {
        return None;
    }
    let local = p
        .actors
        .get(&player)?
        .player
        .read()
        .info
        .read()
        .local_npcs
        .clone();
    let local = local.read();
    let handle = local
        .values()
        .iter()
        .flatten()
        .find(|n| n.read().index == index)?;
    p.host
        .npc
        .as_ref()?
        .view_owners
        .get(&handle.identity())
        .copied()
}

#[cfg(test)]
mod combat_fixture {
    use super::*;
    pub(super) fn populate(p: &mut Probe) -> [Node; 2] {
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cache");
        let limits = rs09_cache::Limits::default();
        let store = rs09_cache::store::Store::open(
            &cache,
            rs09_cache::store::StorageMode::Resident,
            limits,
        )
        .unwrap();
        let archive = Archive::open(store, 3, None, limits).unwrap();
        let mut defs = Registry::default();
        for (id, option) in [(41, "Attack"), (278, "Talk-to")] {
            let def = defs::NpcDefinition {
                id,
                combat_level: 2,
                options: Some(Shared::new(vec![
                    None,
                    Some(option.into()),
                    None,
                    None,
                    None,
                ])),
                ..Default::default()
            };
            for (key, val) in [("lifepoints", 2), ("attack_speed", 4), ("respawn_delay", 4)] {
                def.handlers.write().insert(key.into(), Property::Int(val));
            }
            if id == 41 {
                use rs09_server::world::animation::Animation;
                for (key, id) in [("melee_animation", 5387), ("defence_animation", 5388)] {
                    def.handlers
                        .write()
                        .insert(key.into(), Property::Animation(Animation::new(id).shared()));
                }
            }
            defs.definitions.insert(id, Shared::new(def));
        }
        p.host.npc = Some(Npcs {
            defs,
            archive,
            rows: BTreeMap::new(),
            view_owners: BTreeMap::new(),
            next_index: 1,
            rng: JavaRandom::new(530),
            trace: None,
        });
        let mut init = Init { p, roots: vec![] };
        [(41, 3208), (278, 3206)].map(|(id, x)| {
            let node = npc_init::create(
                &mut init,
                id,
                Some(Location::new(x, 3216, 0)),
                Some(Direction::South),
                &(),
            )
            .unwrap();
            npc_init::init(&mut init, node).unwrap();
            node
        })
    }
}
#[cfg(test)]
pub(super) fn combat_test_targets(p: &mut Probe) -> [Node; 2] {
    combat_fixture::populate(p)
}

pub(super) struct Fighter {
    pub life: i32,
    pub stats: [i32; 3],
    pub bonuses: [i32; 15],
    pub speed: i32,
    pub can_retaliate: bool,
    pub magic: i32,
    pub type_id: i32,
}

pub(super) fn attack_option(p: &Probe, n: Node, option: i32) -> bool {
    option_type(p, n, option, &["attack"]).is_some()
}

pub(super) fn option_type(p: &Probe, n: Node, option: i32, names: &[&str]) -> Option<i32> {
    let row = p.host.npc.as_ref()?.rows.get(&n)?;
    let def = row.def.read();
    let options = def.options.as_ref()?.read();
    options
        .get(option as usize)
        .and_then(Option::as_deref)
        .filter(|name| names.iter().any(|other| name.eq_ignore_ascii_case(other)))
        .map(|_| def.id)
}

pub(super) fn fighter(p: &Probe, n: Node) -> Option<Fighter> {
    let row = p.host.npc.as_ref()?.rows.get(&n)?;
    let def = row.def.read();
    let fields = row.brain.fields.read();
    if row.levels[3] <= 0
        || row.death_end.is_some()
        || fields.respawning
        || fields.returning
        || def.child_npc_ids.is_some()
        || !def.has_attack_option().ok()?
        || !p.state.actor(n).ok()?.view.active
    {
        return None;
    }
    let mut bonuses = [0; 15];
    if let Some(values) = &row.props.bonuses {
        for (bonus, &value) in bonuses.iter_mut().zip(values.read().iter()) {
            *bonus = value;
        }
    }
    let handlers = def.handlers.read();
    let can_retaliate = !matches!(handlers.get("combat_style"),
        Some(Property::CombatStyle(defs_combat_style)) if *defs_combat_style != rs09_server::definitions::property::CombatStyle::Melee);
    Some(Fighter {
        life: row.levels[3],
        stats: [
            row.statics[0],
            (row.statics[1] - row.defence_drain).max(0),
            row.statics[2],
        ],
        bonuses,
        speed: row.props.speed.max(1),
        can_retaliate,
        magic: row.statics[6],
        type_id: def.id,
    })
}

pub(super) fn face_player(p: &mut Probe, n: Node, index: i32) {
    if let Some(row) = p.host.npc.as_mut().and_then(|s| s.rows.get_mut(&n)) {
        row.melee_face = Some(if index < 0 { -1 } else { index | 0x8000 });
        row.view.write().masks_required = true;
    }
}

pub(super) fn attack_animation(p: &mut Probe, n: Node) {
    let Some(row) = p.host.npc.as_mut().and_then(|s| s.rows.get_mut(&n)) else {
        return;
    };
    if let Some(animation) = row.props.animations[slot(Slot::Attack)].as_ref() {
        let f = animation.read();
        row.melee_animation = Some((f.id, f.delay));
        row.view.write().masks_required = true;
    }
}

pub(super) fn damage(p: &mut Probe, n: Node, amount: i32) -> Result<i32, String> {
    impact(p, n, amount, i32::from(amount > 0), true)
}

pub(super) fn graphic(p: &mut Probe, n: Node, id: i32, height: i32, delay: i32) {
    if let Some(row) = p.host.npc.as_mut().and_then(|store| store.rows.get_mut(&n)) {
        row.combat_spot = Some((id, height.wrapping_shl(16).wrapping_add(delay)));
        row.view.write().masks_required = true;
    }
}

pub(super) fn drain_defence(p: &mut Probe, n: Node, amount: i32) {
    if let Some(row) = p.host.npc.as_mut().and_then(|store| store.rows.get_mut(&n)) {
        row.defence_drain = (row.defence_drain + amount).min(row.statics[1]);
    }
}

pub(super) fn poison(p: &mut Probe, n: Node, severity: i32) {
    if let Some(row) = p.host.npc.as_mut().and_then(|store| store.rows.get_mut(&n))
        && !matches!(
            row.def.read().handlers.read().get("poison_immune"),
            Some(Property::Bool(true))
        )
        && row.poison.is_none_or(|(old, _)| severity > old)
    {
        let due = row
            .poison
            .map_or_else(|| p.state.ticks.wrapping_add(30), |(_, due)| due);
        row.poison = Some((severity, due));
    }
}

pub(super) fn impact(
    p: &mut Probe,
    n: Node,
    amount: i32,
    kind: i32,
    block: bool,
) -> Result<i32, String> {
    let row = p
        .host
        .npc
        .as_mut()
        .and_then(|s| s.rows.get_mut(&n))
        .ok_or("missing NPC")?;
    let amount = amount.min(row.levels[3]);
    row.levels[3] -= amount;
    if row.melee_hit.is_some() {
        row.secondary_hit = row.melee_hit;
    }
    row.melee_hit = Some((amount, kind));
    row.view.write().masks_required = true;
    let dead = row.levels[3] == 0;
    let anim = (dead || block)
        .then(|| row.props.animations[slot(if dead { Slot::Death } else { Slot::Defence })].clone())
        .flatten();
    if let Some(animation) = anim.as_ref() {
        let f = animation.read();
        row.melee_animation = Some((f.id, f.delay));
    }
    if dead {
        row.poison = None;
        let duration = anim
            .map(|a| {
                rs09_server::world::animation::Animation::from_shared(a)
                    .duration(&mut p.host.animations)
            })
            .unwrap_or(6);
        let duration = if (1..=30).contains(&duration) {
            duration
        } else {
            6
        };
        row.death_end = Some(p.state.ticks.wrapping_add(duration));
        row.melee_face = Some(-1);
        p.state
            .actor(n)
            .map_err(|e| e.to_string())?
            .queue
            .points()
            .write()
            .clear();
    }
    Ok(amount)
}

fn has_masks(row: &Row) -> bool {
    row.chat.is_some()
        || row.melee_animation.is_some()
        || row.melee_hit.is_some()
        || row.combat_spot.is_some()
        || row.melee_face.is_some()
}
pub(super) fn project(p: &mut Probe, n: Node) -> Result<(), String> {
    let q = &p.state.actor(n).map_err(|e| e.to_string())?.queue;
    let e = q.entity();
    let row = &p.host.npc.as_ref().unwrap().rows[&n];
    let mut view = row.view.write();
    view.location = e.location;
    view.teleporting = e.teleporting;
    view.direction = e.direction.map_or(6, |d| {
        Direction::ALL.iter().position(|v| *v == d).unwrap() as i32
    });
    view.walk_dir = q.walk_dir();
    view.run_dir = q.run_dir();
    view.masks_required = has_masks(row);
    Ok(())
}
pub(super) fn reset(p: &mut Probe, n: Node) -> Result<(), String> {
    p.state
        .actor(n)
        .map_err(|e| e.to_string())?
        .queue
        .state
        .write()
        .entity
        .teleporting = false;
    let row = p
        .host
        .npc
        .as_mut()
        .unwrap()
        .rows
        .get_mut(&n)
        .ok_or("unknown NPC")?;
    row.chat = None;
    row.melee_animation = None;
    row.melee_hit = None;
    row.secondary_hit = None;
    row.combat_spot = None;
    row.melee_face = None;
    let mut v = row.view.write();
    v.teleporting = false;
    v.masks_required = false;
    Ok(())
}
fn npc_error(e: impl std::fmt::Display) -> npc::Error {
    npc::Error::Binding(e.to_string())
}
impl npc_walk::Services for Host {
    fn brain(&self, n: Node) -> npc::Result<npc::State> {
        self.npc
            .as_ref()
            .and_then(|s| s.rows.get(&n))
            .map(|r| r.brain.clone())
            .ok_or(npc::Error::Null)
    }
    fn npc_region_active(&mut self, _: &mut State, _: Node, l: Location) -> npc::Result<bool> {
        let [x, y, _] = l.coordinates();
        Ok(self.active.contains(&(((x >> 6) << 8) | (y >> 6))))
    }
    fn npc_query(&mut self, state: &mut State, n: Node, q: npc::Query) -> npc::Result<bool> {
        let row = &self.npc.as_ref().ok_or(npc::Error::Null)?.rows[&n];
        Ok(match q {
            npc::Query::Behavior => true,
            npc::Query::MovementLocked => super::script_services::locked(
                state,
                n,
                rs09_server::world::action_locks::Kind::Movement,
            )
            .map_err(npc_error)?,
            npc::Query::WaterNpc => match row.def.read().handlers.read().get("water_npc") {
                None | Some(Property::Null) => false,
                Some(Property::Bool(v)) => *v,
                _ => return Err(npc_error("invalid water_npc configuration")),
            },
            npc::Query::InCombat => state.actor(n).map_err(npc_error)?.view.combat_running,
            _ => false,
        })
    }
    fn npc_event(&mut self, state: &mut State, n: Node, e: npc::Event) -> npc::Result<()> {
        let row = self
            .npc
            .as_mut()
            .ok_or(npc::Error::Null)?
            .rows
            .get_mut(&n)
            .ok_or(npc::Error::Null)?;
        match e {
            npc::Event::LockMovement(t) => super::script_services::lock(
                state,
                n,
                rs09_server::world::action_locks::Kind::Movement,
                t,
            )
            .map_err(npc_error)?,
            npc::Event::UnlockMovement => super::script_services::unlock(
                state,
                n,
                rs09_server::world::action_locks::Kind::Movement,
            )
            .map_err(npc_error)?,
            npc::Event::Chat(s) => row.chat = s,
            npc::Event::BehaviorRespawn | npc::Event::Respawn => {}
            npc::Event::Restore => {
                row.levels[3] = row.statics[3];
                row.defence_drain = 0;
                row.poison = None;
            }
            npc::Event::Attribute(_)
            | npc::Event::ImpactDisabled(_)
            | npc::Event::PauseAggression(_)
            | npc::Event::StopCombat => {}
            _ => {
                return Err(npc_error(
                    "NPC fixture has no inactivity/respawn/content service",
                ));
            }
        }
        Ok(())
    }
    fn dialogue_active(&mut self, _: &mut State, _: usize) -> npc::Result<bool> {
        Ok(false)
    }
    fn dialogue_chatbox(&mut self, _: &mut State, _: usize) -> npc::Result<bool> {
        Ok(false)
    }
    fn random_bool(&mut self) -> npc::Result<bool> {
        Ok(self.npc.as_mut().ok_or(npc::Error::Null)?.rng.next_bool())
    }
    fn randomize(&mut self, b: i32) -> npc::Result<i32> {
        Ok(self
            .npc
            .as_mut()
            .ok_or(npc::Error::Null)?
            .rng
            .next_bound(b)
            .unwrap_or(0))
    }
    fn random_range(&mut self, a: i32, b: i32) -> npc::Result<i32> {
        let n = b.wrapping_sub(a).wrapping_abs();
        Ok(a.min(b).wrapping_add(
            self.npc
                .as_mut()
                .ok_or(npc::Error::Null)?
                .rng
                .next_bound(n)
                .unwrap_or(0),
        ))
    }
    fn npc_can_move(&mut self, _: &mut State, _: Node, _: Option<Location>) -> npc::Result<bool> {
        Ok(true)
    }
    fn npc_water_path(
        &mut self,
        state: &mut State,
        node: Node,
        destination: Location,
    ) -> npc::Result<rs09_server::world::pathfinding::Path> {
        state
            .context(self)
            .find_water_path(node, destination)
            .map_err(npc_error)
    }
}
pub(super) fn tick(p: &mut Probe, n: Node) -> Result<(), String> {
    let poisoned = p
        .host
        .npc
        .as_mut()
        .and_then(|store| store.rows.get_mut(&n))
        .and_then(|row| {
            let (severity, due) = row.poison?;
            if p.state.ticks.wrapping_sub(due) < 0 {
                return None;
            }
            row.poison = (severity > 1).then_some((severity - 1, due.wrapping_add(30)));
            Some((severity + 4) / 5)
        });
    if let Some(damage) = poisoned {
        impact(p, n, damage, 2, false)?;
    }
    if let Some(end) = p
        .host
        .npc
        .as_ref()
        .and_then(|s| s.rows.get(&n))
        .and_then(|r| r.death_end)
    {
        if p.state.ticks.wrapping_sub(end) >= 0 {
            let row = p.host.npc.as_mut().unwrap().rows.get_mut(&n).unwrap();
            let delay = match row.def.read().handlers.read().get("respawn_delay") {
                Some(Property::Int(delay)) => (*delay).max(1),
                _ => 17,
            };
            row.death_end = None;
            let mut fields = row.brain.fields.write();
            fields.respawning = true;
            fields.respawn_tick = p.state.ticks.wrapping_add(delay);
            p.state
                .actor(n)
                .map_err(|e| e.to_string())?
                .queue
                .state
                .write()
                .entity
                .teleport_location = fields.spawn.clone();
        }
        return project(p, n);
    }
    npc_walk::tick(&mut p.state, &mut p.host, n).map_err(|e| e.to_string())?;
    project(p, n)
}
pub(super) fn arrive(state: &mut State, host: &mut Host, n: Node) -> player_walk::Result<bool> {
    npc_walk::arrive(state, host, n).map_err(|e| player_walk::Error::Binding(e.to_string()))
}
pub(super) fn reap(p: &mut Probe) {
    if let Some(store) = &mut p.host.npc {
        store.rows.retain(|n, _| p.state.actor(*n).is_ok());
        store
            .view_owners
            .retain(|_, node| store.rows.contains_key(node));
        if let Some(trace) = &mut store.trace {
            trace.updates.retain(|n, _| p.state.actor(*n).is_ok());
        }
    }
}
struct Renderer<'a> {
    p: &'a mut Probe,
    node: Node,
}
impl npc_render::Context for Renderer<'_> {
    fn hidden(
        &mut self,
        _: &player_render::Handle,
        npc: &npc_render::Handle,
    ) -> player_render::Result<bool> {
        let store = self.p.host.npc.as_ref().ok_or(player_render::Error::Null)?;
        let node = *store
            .view_owners
            .get(&npc.identity())
            .ok_or(player_render::Error::Null)?;
        let row = &store.rows[&node];
        let active = self
            .p
            .state
            .actor(node)
            .map_err(|e| player_render::Error::Binding(e.to_string()))?
            .view
            .active;
        Ok(!active || row.brain.fields.read().respawn_tick > self.p.state.ticks)
    }
    fn candidates(
        &mut self,
        location: Location,
        distance: i32,
    ) -> player_render::Result<Vec<Option<npc_render::Handle>>> {
        let nodes = super::spatial_services::local(
            &mut self.p.state,
            &mut self.p.host,
            location,
            distance,
            rs09_server::world::entity_regions::Select::Npcs,
        )
        .map_err(|e| player_render::Error::Binding(e.to_string()))?;
        let mut out = vec![];
        for lease in nodes {
            let n = lease.node;
            let view = self
                .p
                .host
                .npc
                .as_ref()
                .ok_or(player_render::Error::Null)?
                .rows[&n]
                .view
                .clone();
            let lease = Shared::new(lease);
            out.push(Some(view.keeping_from(&lease)));
        }
        Ok(out)
    }
    fn masks(
        &mut self,
        _: &player_render::Handle,
        npc: &npc_render::Handle,
        out: &mut Vec<u8>,
        _: bool,
    ) -> player_render::Result<()> {
        let store = self.p.host.npc.as_ref().ok_or(player_render::Error::Null)?;
        let node = store
            .view_owners
            .get(&npc.identity())
            .ok_or(player_render::Error::Null)?;
        let row = &store.rows[node];
        let mut writer = rs09_protocol::buffer::Writer::default();
        writer.p1((i32::from(row.chat.is_some()) << 5)
            | (i32::from(row.melee_hit.is_some()) << 6)
            | (i32::from(row.melee_animation.is_some()) << 4)
            | (i32::from(row.melee_face.is_some()) << 2)
            | (i32::from(row.combat_spot.is_some()) << 7)
            | (i32::from(row.secondary_hit.is_some()) << 1));
        if let Some((damage, kind)) = row.melee_hit {
            writer.p1(damage);
            writer.p1neg(kind);
            writer.p1sub(row.levels[3] * 255 / row.statics[3].max(1));
        }
        if let Some((damage, kind)) = row.secondary_hit {
            writer.p1neg(damage);
            writer.p1sub(kind);
        }
        if let Some((id, delay)) = row.melee_animation {
            writer.p2(id);
            writer.p1(delay);
        }
        if let Some(face) = row.melee_face {
            writer.p2add(face);
        }
        if let Some((graphic, packed)) = row.combat_spot {
            writer.p2add(graphic);
            writer.ip4(packed);
        }
        out.extend(writer.into_vec());
        if let Some(text) = &row.chat {
            let text = String::from_utf16(text)
                .map_err(|e| player_render::Error::Binding(e.to_string()))?;
            out.extend_from_slice(text.as_bytes());
            out.push(0);
        }
        Ok(())
    }
    fn remove_tolerance(
        &mut self,
        npc: &npc_render::Handle,
        player: i32,
    ) -> player_render::Result<()> {
        let store = self.p.host.npc.as_mut().ok_or(player_render::Error::Null)?;
        let node = store
            .view_owners
            .get(&npc.identity())
            .ok_or(player_render::Error::Null)?;
        let row = store.rows.get_mut(node).unwrap();
        row.tolerance.remove(&player);
        Ok(())
    }
    fn add_tolerance(
        &mut self,
        npc: &npc_render::Handle,
        player: i32,
    ) -> player_render::Result<()> {
        let store = self.p.host.npc.as_mut().ok_or(player_render::Error::Null)?;
        let node = store
            .view_owners
            .get(&npc.identity())
            .ok_or(player_render::Error::Null)?;
        let row = store.rows.get_mut(node).unwrap();
        row.tolerance.insert(player, self.p.state.ticks);
        Ok(())
    }
    fn send(&mut self, _: &player_render::Handle, f: Frame) -> player_render::Result<()> {
        self.p.host.send(self.node, f);
        Ok(())
    }
}
pub(super) fn render(
    p: &mut Probe,
    node: Node,
    player: &player_render::Handle,
) -> Result<(), String> {
    npc_render::render(&mut Renderer { p, node }, player).map_err(|e| e.to_string())?;
    if p.host.npc.as_ref().is_some_and(|s| s.trace.is_some()) {
        let info = player.read().info.clone();
        let locals = info.read().local_npcs.clone();
        let store = p.host.npc.as_ref().unwrap();
        let mut npcs = Vec::new();
        for view in locals.read().values().iter().flatten() {
            let n = *store
                .view_owners
                .get(&view.identity())
                .ok_or("unknown traced NPC")?;
            let row = &store.rows[&n];
            let a = p.state.actor(n).map_err(|e| e.to_string())?;
            npcs.push(serde_json::json!({"slot":a.wire_index,"type":row.def.read().id,"location":a.queue.entity().location.map(|l|l.coordinates())}));
        }
        let index = p.state.actor(node).map_err(|e| e.to_string())?.wire_index;
        let self_location = p
            .state
            .actor(node)
            .map_err(|e| e.to_string())?
            .queue
            .entity()
            .location
            .map(|l| l.coordinates());
        let ui = super::ui_services::trace_state(&p.host, node);
        let trace = p.host.npc.as_mut().unwrap().trace.as_mut().unwrap();
        let update = trace.updates.entry(node).or_default();
        *update += 1;
        let record = serde_json::json!({"username":p.actors[&index].username,"update":update,"tick":p.state.ticks,"npcs":npcs,"self":self_location,"ui":ui});
        serde_json::to_writer(&mut trace.file, &record).map_err(|e| e.to_string())?;
        trace.file.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::game::{npcs as wire, players, rebuild};
    use rs09_server::world::tick;

    #[test]
    fn whole_world_catalog_includes_original_cows_and_mountain_trolls() {
        let (catalog, count) = WorldSpawns::load(include_str!(
            "../../../../fixtures/npc-spawns/npc_spawns.json"
        ))
        .unwrap();
        assert_eq!(count, 13_709);
        assert_eq!(
            catalog.regions.values().map(Vec::len).sum::<usize>(),
            count as usize
        );
        for (id, tile) in [(81, [3255, 3255, 0]), (1106, [2864, 3594, 0])] {
            let entries = &catalog.regions[&(tile[0] >> 6, tile[1] >> 6)];
            let placement = entries
                .iter()
                .find(|entry| entry.id == id && entry.location.coordinates() == tile)
                .unwrap();
            assert!(placement.walks);
        }
    }

    #[test]
    fn newly_visited_regions_spawn_once_and_keep_original_planes() {
        let (mut p, _, _) = super::super::tests::probe();
        combat_test_targets(&mut p);
        let (catalog, _) = WorldSpawns::load(
            r#"[{"npc_id":"41","loc_data":"{3213,3216,0,1,3}-{3265,3216,1,0,0}-"}]"#,
        )
        .unwrap();
        p.world_spawns = Some(catalog);
        let arrival = Location::new(3216, 3216, 0);
        open_near(&mut p, &arrival);
        open_near(&mut p, &arrival);
        assert_eq!(p.host.npc.as_ref().unwrap().rows.len(), 3);
        let crossing = Location::new(3248, 3216, 0);
        open_near(&mut p, &crossing);
        open_near(&mut p, &crossing);
        let store = p.host.npc.as_ref().unwrap();
        assert_eq!(store.rows.len(), 4);
        assert!(store.rows.values().any(|row| {
            row.props
                .spawn
                .as_ref()
                .is_some_and(|tile| tile.coordinates() == [3265, 3216, 1])
        }));
    }

    #[test]
    fn initialized_npc_children_chat_wire_and_retired_local_leases_are_composed() {
        let (mut p, player, mut output) = super::super::tests::probe();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cache");
        let limits = rs09_cache::Limits::default();
        let store =
            rs09_cache::store::Store::open(&root, rs09_cache::store::StorageMode::Resident, limits)
                .unwrap();
        let archive = Archive::open(store, 3, None, limits).unwrap();
        let mut defs = Registry::default();
        for id in [-1, 1, 2] {
            let d = Shared::new(defs::NpcDefinition {
                id,
                ..Default::default()
            });
            if id == 1 {
                let mut d = d.write();
                d.size = 2;
                d.child_npc_ids = Some(Shared::new(vec![2, -1]));
                d.handlers
                    .write()
                    .insert("force_talk".into(), Property::Text("Hello there".into()));
            }
            defs.definitions.insert(id, d);
        }
        p.host.npc = Some(Npcs {
            defs,
            archive,
            rows: BTreeMap::new(),
            view_owners: BTreeMap::new(),
            next_index: 1,
            rng: JavaRandom::new(530),
            trace: None,
        });
        p.host.active.insert((50 << 8) | 50);
        let npc;
        {
            let mut init = Init {
                p: &mut p,
                roots: vec![],
            };
            npc = npc_init::create(
                &mut init,
                1,
                Some(Location::new(3208, 3216, 0)),
                Some(Direction::South),
                &(),
            )
            .unwrap();
            npc_init::init(&mut init, npc).unwrap();
        }
        let children = p.state.children(npc).unwrap().unwrap();
        assert_eq!(children.read().len(), 2);
        for child in children.read().iter().flatten() {
            let a = p.state.actor(*child).unwrap();
            assert_eq!(a.wire_index, 1);
            assert_eq!(a.view.size, 2);
            assert!(a.queue.entity().location.is_none());
        }
        drop(children);
        p.state.collect();
        assert_eq!(p.state.retained_counts(), [4, 0, 0]);
        {
            let mut init = Init {
                p: &mut p,
                roots: vec![],
            };
            for location in [Location::new(3223, 3216, 0), Location::new(3208, 3216, 1)] {
                let n = npc_init::create(&mut init, 2, Some(location), Some(Direction::South), &())
                    .unwrap();
                npc_init::init(&mut init, n).unwrap();
            }
        }
        tick::handle(&mut p, false).unwrap();
        assert!(output.try_recv().is_err());
        p.state.ticks = 10;
        tick::handle(&mut p, false).unwrap();
        assert!(p.host.errors.is_empty(), "{:?}", p.host.errors);
        let frames = output.try_recv().unwrap();
        assert!(
            frames.iter().position(|f| f.opcode == 225).unwrap()
                < frames.iter().position(|f| f.opcode == 32).unwrap()
        );
        let mut players = players::State::new(0, 0);
        let mut base = [0; 2];
        let mut npcs = wire::State::default();
        for frame in frames {
            match frame.opcode {
                162 => base = rebuild::decode_normal(&frame).unwrap().base(),
                225 => {
                    players.decode_frame(&frame, 1).unwrap();
                }
                32 => {
                    let s = players.self_player();
                    npcs.decode(&frame, [base[0] + s.queue_x[0], base[1] + s.queue_y[0]], 1)
                        .unwrap();
                }
                _ => {}
            }
        }
        assert_eq!(npcs.local_ids, vec![1]);
        assert_eq!(npcs.actors[&1].tile, [3208, 3216]);
        assert_eq!(
            npcs.actors[&1].last_mask.chat.as_deref(),
            Some(&b"Hello there"[..])
        );
        let nodes: Vec<_> = p.host.npc.as_ref().unwrap().rows.keys().copied().collect();
        for node in nodes {
            super::super::spatial_services::detach(&mut p.state, &mut p.host, node).unwrap();
            p.npcs.remove(&node);
            p.state.actor_mut(node).unwrap().connected = false;
            p.state.actor_mut(node).unwrap().view.active = false;
        }
        p.state.collect();
        assert!(
            p.state.actor(npc).is_ok(),
            "the player's local view retains the old node"
        );
        tick::handle(&mut p, false).unwrap();
        p.state.collect();
        reap(&mut p);
        assert!(p.state.actor(npc).is_err());
        assert!(p.host.npc.as_ref().unwrap().rows.is_empty());
        assert!(p.state.actor(player).is_ok());
    }
}
