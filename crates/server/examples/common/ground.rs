//! Opt-in ground visibility/scheduling on the movement probe's actual scene,
//! actor repository and packet queues. Real default option tables are installed;
//! packet-to-pickup interactions/plugins, region
//! inactivity and complete player profiles remain explicit fixture boundaries.
use super::{Host, Map, MapServices, Node, State, now};
use rs09_cache::archive::Archive;
use rs09_game_core::java_json::Json;
use rs09_protocol::game::{Direction, Frame};
use rs09_server::{
    auth::account::Account,
    definitions::{item, item_config, scenery as defs},
    pulses,
    world::{
        chunk_sync::{Buffer, Items as ItemList},
        doors::{self, Location},
        ground_item::Handle,
        ground_manager::{Error, Result},
        ground_options, ground_spawn_loader as loader,
        ground_world::{self as gw, Parts},
        map_chunk_render::{self, Viewport},
        player_render,
        player_walk::NodeHandle,
        region, scene, scenery_node, update,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

pub(super) struct Ground {
    pub items: gw::Items,
    pub options: ground_options::Registry,
    pub repository: update::List<Node>,
    profiles: BTreeMap<Node, Profile>,
    viewers: HashMap<usize, Vec<String>>,
    pub pickup_trace: Option<std::fs::File>,
}
struct Profile {
    account: Account,
    render: player_render::Handle,
    viewport: Viewport,
}
impl Ground {
    pub(super) fn new(definitions: item::Registry) -> Self {
        let options = ground_options::Registry::default();
        options
            .register_pickup()
            .expect("native default registration has no fallible services");
        Self {
            items: gw::Items::new(definitions),
            options,
            repository: update::List::default(),
            profiles: BTreeMap::new(),
            viewers: HashMap::new(),
            pickup_trace: None,
        }
    }
    pub(super) fn load(
        archive: &Archive,
        config: &Path,
    ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let mut definitions = item::Registry::default();
        let count = definitions.load_archive(archive, 1_000_000)?;
        struct Log;
        impl item_config::Logger for Log {
            fn parsed(&mut self, count: i32) -> item_config::Result<()> {
                println!("Ground fixture item configuration: {count} rows");
                Ok(())
            }
        }
        item_config::Parser::default()
            .load_file(config, &definitions, &mut Log)
            .map_err(|e| format!("ground item configuration: {e:?}"))?;
        println!("Ground fixture item definitions: {count}");
        Ok(Self::new(definitions))
    }
}
pub(super) struct Services<'a> {
    options: &'a ground_options::Registry,
    maps: &'a mut MapServices,
    active: &'a mut BTreeSet<i32>,
    frames: &'a mut BTreeMap<Node, Vec<Frame>>,
    errors: &'a mut Vec<String>,
    profiles: &'a mut BTreeMap<Node, Profile>,
    viewers: &'a mut HashMap<usize, Vec<String>>,
    repository: &'a update::List<Node>,
    animations: &'a mut rs09_server::world::prepared_animation::World<super::AnimationAssets>,
    animators: &'a mut BTreeMap<
        Node,
        rs09_server::world::animation::Animator<rs09_server::world::prepared_animation::Owner>,
    >,
}
#[path = "ground_pickup.rs"]
mod pickup_bindings;
pub(super) use pickup_bindings::{pickup, process, program};
fn error(e: impl std::fmt::Display) -> Error {
    Error::Binding(e.to_string())
}
fn pulse_error(e: pulses::Error) -> Error {
    Error::Binding(format!("{e:?}"))
}
macro_rules! delegate_scene {
    ($name:ident($($arg:ident:$ty:ty),*)->$ret:ty)=>{fn $name(&mut self,$($arg:$ty),*)->scene::Result<$ret>{scene::Services::$name(self.maps,$($arg),*)}};
}
impl scene::Services<scene::CacheAssets> for Services<'_> {
    fn is_player(&self, n: usize) -> bool {
        scene::Services::is_player(self.maps, n)
    }
    delegate_scene!(location(s: &mut Map, n: usize) -> Location);
    delegate_scene!(lock(s: &mut Map, n: usize, t: i32) -> ());
    delegate_scene!(audio(s: &mut Map, n: usize, id: i32, delay: Option<i32>) -> ());
    delegate_scene!(message(s: &mut Map, n: usize, text: &str) -> ());
    delegate_scene!(logout_listener(s: &mut Map, n: usize, l: Option<Location>) -> ());
    delegate_scene!(reset_path(s: &mut Map, n: usize) -> ());
    delegate_scene!(add_path(s: &mut Map, n: usize, x: i32, y: i32) -> ());
    delegate_scene!(move_near(s: &mut Map, n: usize, l: Location) -> bool);
    delegate_scene!(diary(s: &mut Map, n: usize, d: doors::Diary, level: i32, task: i32) -> ());
    delegate_scene!(ground_create(s: &mut Map, id: Option<usize>) -> ());
    delegate_scene!(pulse(s: &mut Map, p: &pulses::Handle) -> bool);
    delegate_scene!(stop(s: &mut Map, p: &pulses::Handle) -> ());
    fn now_millis(&mut self) -> i64 {
        now()
    }
}
impl gw::Services<scene::CacheAssets> for Services<'_> {
    type Plugin = ();
    fn uid(&mut self, _: &mut Parts<'_, scene::CacheAssets>, p: &NodeHandle) -> Result<i32> {
        Ok(self
            .profiles
            .get(&p.node)
            .ok_or(Error::Null)?
            .account
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .uid)
    }
    fn username(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
    ) -> Result<Option<String>> {
        Ok(Some(
            self.profiles
                .get(&p.node)
                .ok_or(Error::Null)?
                .account
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .username
                .clone(),
        ))
    }
    fn default_interaction(
        &mut self,
        world: &mut Parts<'_, scene::CacheAssets>,
        item: &Handle,
    ) -> Result<()> {
        self.options
            .initialize(
                world.actors,
                &world.items.spawns,
                &world.items.definitions,
                item,
                |name| Ok(rs09_game_core::java_lower::lowercase_root(name)),
            )
            .map_err(error)
    }
    fn plugin(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: &item::Handle,
    ) -> Result<Option<()>> {
        Ok(None)
    }
    fn plugin_remove(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: (),
        _: Option<NodeHandle>,
        _: &Handle,
        _: i32,
    ) -> Result<()> {
        Err(error("fixture has no registered item plugins"))
    }
    fn playing(&mut self, w: &mut Parts<'_, scene::CacheAssets>, p: &NodeHandle) -> Result<bool> {
        let a = w.actors.actor(p.node).map_err(error)?;
        Ok(a.connected && a.view.active)
    }
    fn artificial(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: &NodeHandle,
    ) -> Result<bool> {
        Ok(false)
    }
    fn ai_items(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: &NodeHandle,
    ) -> Result<Option<ItemList>> {
        Err(error("fixture has no artificial players"))
    }
    fn send_frames(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
        frames: &[Frame],
    ) -> Result<()> {
        let mut wire = Vec::new();
        for f in frames {
            f.write(Direction::ServerToClient, None, &mut wire)
                .map_err(error)?;
        }
        self.frames
            .entry(p.node)
            .or_default()
            .extend_from_slice(frames);
        Ok(())
    }
    fn attribute(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: &NodeHandle,
        _: &str,
    ) -> Result<bool> {
        Ok(false)
    }
    fn layer(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        h: &doors::Handle,
    ) -> Result<Option<scenery_node::Layer>> {
        scenery_node::layer(&mut Layers(self.errors), h).map_err(|e| error(format!("{e:?}")))
    }
    fn foreign_flag(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: usize,
        _: &Buffer,
        _: &NodeHandle,
    ) -> Result<()> {
        Err(error("fixture has no registered foreign chunk flag"))
    }
    fn max_players(&mut self, _: &mut Parts<'_, scene::CacheAssets>) -> Result<i32> {
        Ok(2000)
    }
    fn player_count(&mut self, _: &mut Parts<'_, scene::CacheAssets>) -> Result<i32> {
        Ok(self.repository.values().len() as i32)
    }
    fn viewport(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
    ) -> Result<Viewport> {
        Ok(self
            .profiles
            .get(&p.node)
            .ok_or(Error::Null)?
            .viewport
            .clone())
    }
    fn last_scene(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
    ) -> Result<Option<Location>> {
        Ok(self
            .profiles
            .get(&p.node)
            .ok_or(Error::Null)?
            .render
            .read()
            .last_scene
            .clone())
    }
    fn decrement_view(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        r: &region::Handle,
        name: String,
    ) -> Result<()> {
        if let Some(v) = self.viewers.get_mut(&r.identity())
            && let Some(i) = v.iter().position(|n| *n == name)
        {
            v.remove(i);
        }
        Ok(())
    }
    fn increment_view(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        r: &region::Handle,
        name: String,
    ) -> Result<()> {
        let v = self.viewers.entry(r.identity()).or_default();
        if !v.contains(&name) {
            v.push(name);
        }
        Ok(())
    }
    fn region_active(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        r: &region::Handle,
    ) -> Result<bool> {
        Ok(self.active.contains(&r.read().id()))
    }
    fn check_inactive(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: &region::Handle,
    ) -> Result<()> {
        // Existing movement/NPC probes keep activated cache regions resident;
        // original delayed inactivity/NPC deactivation is not claimed here.
        Ok(())
    }
    fn flag_active(
        &mut self,
        w: &mut Parts<'_, scene::CacheAssets>,
        r: &region::Handle,
    ) -> Result<()> {
        let id = r.read().id();
        w.scene.context(self.maps).load(id, None).map_err(error)?;
        self.active.insert(id);
        Ok(())
    }
    fn loader_error(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        entry: &Json,
        e: &loader::Error,
    ) -> loader::Result<()> {
        self.errors
            .push(format!("ground entry {}: {e}", entry.text()));
        Ok(())
    }
    fn loaded(&mut self, _: &mut Parts<'_, scene::CacheAssets>, count: i32) -> loader::Result<()> {
        println!("Ground fixture initialized {count} spawns");
        Ok(())
    }
}
struct Layers<'a>(&'a mut Vec<String>);
impl defs::PlayerContext for Layers<'_> {
    fn varbit(&mut self, _: i32, _: usize) -> defs::Result<Option<i32>> {
        Err(defs::Error::Binding("layer-only context".into()))
    }
    fn varp(&mut self, _: i32, _: usize) -> defs::Result<i32> {
        Err(defs::Error::Binding("layer-only context".into()))
    }
}
impl scenery_node::Context for Layers<'_> {
    fn varbit_exists(&mut self, _: i32) -> scenery_node::Result<bool> {
        Err(scenery_node::Error::Null)
    }
    fn set_varbit(&mut self, _: usize, _: i32, _: i32) -> scenery_node::Result<()> {
        Err(scenery_node::Error::Null)
    }
    fn set_varp(&mut self, _: usize, _: i32, _: i32) -> scenery_node::Result<()> {
        Err(scenery_node::Error::Null)
    }
    fn log_invalid_shape(&mut self, shape: i32, constructed: bool) -> scenery_node::Result<()> {
        self.0.push(format!(
            "invalid scenery layer: {shape}, constructed={constructed}"
        ));
        Ok(())
    }
}
pub(super) fn world<R>(
    actors: &mut State,
    host: &mut Host,
    action: impl FnOnce(&mut gw::World<'_, '_, scene::CacheAssets, Services<'_>>) -> Result<R>,
) -> Result<R> {
    let Host {
        scene,
        maps,
        active,
        frames,
        errors,
        ground,
        animations,
        animators,
        ..
    } = host;
    let Ground {
        items,
        options,
        profiles,
        viewers,
        repository,
        ..
    } = ground
        .as_mut()
        .ok_or_else(|| error("ground fixture is disabled"))?;
    let mut parts = Parts::new(scene, actors, items)?;
    let mut services = Services {
        options,
        maps,
        active,
        frames,
        errors,
        profiles,
        viewers,
        repository,
        animations,
        animators,
    };
    action(&mut parts.context(&mut services))
}
pub(super) fn admit(host: &mut Host, node: Node, account: Account, render: player_render::Handle) {
    if let Some(g) = &mut host.ground {
        g.profiles.insert(
            node,
            Profile {
                account,
                render,
                viewport: map_chunk_render::empty_viewport(),
            },
        );
    }
}
pub(super) fn reap(host: &mut Host, actors: &State) {
    if let Some(g) = &mut host.ground {
        g.profiles.retain(|n, _| actors.actor(*n).is_ok());
        g.items.spawns.collect();
    }
}
pub(super) fn owns(host: &Host, p: &pulses::Handle) -> bool {
    host.ground
        .as_ref()
        .is_some_and(|g| g.items.spawns.owns_pulse(p))
}
pub(super) fn render(actors: &mut State, host: &mut Host, p: &NodeHandle) -> Result<()> {
    world(actors, host, |w| w.render(p).map_err(error))
}
pub(super) fn manager_pulse(actors: &mut State, host: &mut Host) -> Result<()> {
    world(actors, host, |w| w.manager_pulse())
}
pub(super) fn recover_ammunition(
    actors: &mut State,
    host: &mut Host,
    player: &NodeHandle,
    id: i32,
    amount: i32,
    location: Location,
) -> Result<()> {
    if host.ground.is_none() {
        return Ok(());
    }
    world(actors, host, |w| {
        w.recover_ammunition(player, id, amount, location)
    })
}
pub(super) fn load(actors: &mut State, host: &mut Host, text: &str) -> Result<i32> {
    world(actors, host, |w| {
        w.load(&loader::Loader::default(), text).map_err(error)
    })
}
pub(super) fn pulse(
    actors: &mut State,
    host: &mut Host,
    p: &pulses::Handle,
) -> pulses::Result<bool> {
    world(actors, host, |w| w.pulse_body(p).map_err(pulse_error))
        .map_err(|e| pulses::Error::Binding(e.to_string()))
}
pub(super) fn inactive(
    actors: &mut State,
    host: &mut Host,
    p: &pulses::Handle,
) -> pulses::Result<bool> {
    world(actors, host, |w| w.pulse_inactive(p).map_err(pulse_error))
        .map_err(|e| pulses::Error::Binding(e.to_string()))
}
pub(super) fn stop(actors: &mut State, host: &mut Host, p: &pulses::Handle) -> pulses::Result<()> {
    world(actors, host, |w| w.stop_pulse(p).map_err(pulse_error))
        .map_err(|e| pulses::Error::Binding(e.to_string()))
}
#[cfg(test)]
#[path = "ground_tests.rs"]
mod tests;
