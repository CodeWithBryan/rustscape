//! Composition regressions: real manager lookup, option tables, canonical target
//! captures and legacy scheduler. The arrival records a typed handler invocation;
//! full Pickup/profile/container services and live Take are separate bindings.
#[allow(dead_code)]
#[path = "../../../oracle/interaction-runtime/native.rs"]
mod legacy_fixture;
#[allow(dead_code)]
#[path = "../../../oracle/ground-options/native.rs"]
mod table_fixture;
use rs09_protocol::{buffer::Reader, game::actions::ClientAction};
use rs09_server::{
    definitions::item::Shared,
    inventory::item,
    world::{
        chunk::{HandleChunk, RegionChunk},
        chunk_sync,
        doors::Location,
        ground_action::{Listener, Request},
        ground_item as ground,
        ground_manager::{self as gm, Result},
        ground_options,
        ground_runtime::{self, Bindings},
        interaction::{Approach, OptionHandle},
        interaction_options as options, legacy_interaction as legacy, legacy_runtime,
        movement::DestinationService,
        pickup,
        player_walk::{self as walk, Event, Node, NodeHandle, Query, Scripts, State},
        walk_request::Interfaces,
    },
};
use serde_json::json;

struct Lookup {
    chunk: HandleChunk,
    calls: usize,
}
impl ground::Context for Lookup {
    fn copy_item(&mut self, _: i32, _: i32, _: i32) -> ground::Result<item::Handle> {
        unreachable!()
    }
    fn default_interaction(&mut self, _: &ground::Handle) -> ground::Result<()> {
        unreachable!()
    }
    fn uid(&mut self, p: &NodeHandle) -> ground::Result<i32> {
        Ok(p.node as i32)
    }
    fn username(&mut self, _: &NodeHandle) -> ground::Result<Option<String>> {
        unreachable!()
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(0)
    }
}
impl gm::Context for Lookup {
    type Plugin = ();
    fn plugin(&mut self, _: &rs09_server::definitions::item::Handle) -> Result<Option<()>> {
        unreachable!()
    }
    fn plugin_remove(
        &mut self,
        _: (),
        _: Option<NodeHandle>,
        _: &ground::Handle,
        _: i32,
    ) -> Result<()> {
        unreachable!()
    }
    fn chunk(&mut self, _: Location) -> Result<HandleChunk> {
        self.calls += 1;
        Ok(self.chunk.clone())
    }
    fn flagged(&mut self, _: &HandleChunk) -> Result<()> {
        unreachable!()
    }
    fn player_location(&mut self, _: &NodeHandle) -> Result<Option<Location>> {
        unreachable!()
    }
    fn playing(&mut self, _: &NodeHandle) -> Result<bool> {
        unreachable!()
    }
    fn artificial(&mut self, _: &NodeHandle) -> Result<bool> {
        unreachable!()
    }
    fn ai_items(&mut self, _: &NodeHandle) -> Result<Option<chunk_sync::Items>> {
        unreachable!()
    }
    fn packet(
        &mut self,
        _: gm::Packet,
        _: Option<NodeHandle>,
        _: &ground::Handle,
        _: i32,
    ) -> Result<()> {
        unreachable!()
    }
}
struct Host {
    legacy: legacy_fixture::Model,
    table: table_fixture::Harness,
    lookup: Lookup,
    events: Vec<String>,
    arrivals: Vec<(usize, ground::Handle, String)>,
    zones: Vec<OptionHandle>,
    plugin_handles: bool,
    listener_handles: Option<Listener>,
    replace_table: bool,
    foreign_capture: Option<ground_options::Captured>,
}
fn error(e: impl std::fmt::Display) -> gm::Error {
    gm::Error::Binding(e.to_string())
}
impl Bindings for Host {
    fn ground_lookup(
        &mut self,
        _: &mut State,
        id: i32,
        l: Option<Location>,
        p: Option<NodeHandle>,
    ) -> Result<Option<ground::Handle>> {
        self.events.push("lookup".into());
        gm::get(&mut self.lookup, id, l, p)
    }
    fn ground_interaction(
        &mut self,
        s: &mut State,
        h: &ground::Handle,
    ) -> Result<ground_options::Captured> {
        self.events.push("interaction".into());
        if let Some(c) = &self.foreign_capture {
            return Ok(c.clone());
        }
        self.table
            .registry
            .capture(s, &self.table.spawns, h)
            .map_err(error)
    }
    fn ground_plugin(
        &mut self,
        _: &mut State,
        _: &NodeHandle,
        _: &ground::Handle,
        _: &OptionHandle,
    ) -> Result<bool> {
        self.events.push("plugin".into());
        Ok(self.plugin_handles)
    }
    fn ground_listeners(
        &mut self,
        s: &mut State,
        _: i32,
        kind: Listener,
        _: Option<String>,
        _: &NodeHandle,
        h: &ground::Handle,
    ) -> Result<bool> {
        self.events.push(format!("{kind:?}"));
        if self.replace_table && kind == Listener::GroundItem {
            let capture = self
                .table
                .registry
                .capture(s, &self.table.spawns, h)
                .map_err(error)?;
            let replacement = Shared::new(walk_option("Replacement", 0));
            *capture.table.read().options().write() =
                vec![Some(replacement), None, None, None, None];
        }
        Ok(self.listener_handles == Some(kind))
    }
}
fn walk_option(name: &str, index: i32) -> rs09_server::world::interaction::OptionData {
    rs09_server::world::interaction::OptionData::new(Some(name.into()), index)
}
impl walk::Services for Host {
    fn query(&mut self, s: &mut State, q: Query<'_>) -> walk::Result<bool> {
        walk::Services::query(&mut self.legacy, s, q)
    }
    fn event(&mut self, s: &mut State, e: Event<'_>) -> walk::Result<()> {
        match &e {
            Event::Scripts(_, Scripts::Reset) => self.events.push("reset".into()),
            Event::Minimap(_) => self.events.push("minimap".into()),
            _ => {}
        }
        walk::Services::event(&mut self.legacy, s, e)
    }
    fn interfaces(&self, s: &State, n: Node) -> walk::Result<Interfaces> {
        walk::Services::interfaces(&self.legacy, s, n)
    }
    fn region(&mut self, s: &mut State, l: &Location) -> walk::Result<Option<bool>> {
        walk::Services::region(&mut self.legacy, s, l)
    }
    fn clipping(&mut self, s: &mut State, z: i32, x: i32, y: i32) -> walk::Result<i32> {
        walk::Services::clipping(&mut self.legacy, s, z, x, y)
    }
    fn now_millis(&mut self) -> i64 {
        0
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
    fn destination(
        &mut self,
        _: &mut State,
        _: Option<Node>,
        _: Option<Node>,
        _: DestinationService,
    ) -> walk::Result<Option<Location>> {
        Ok(pickup::plugin_destination())
    }
    fn teleport_permitted(&mut self, _: &mut State, _: Option<Location>) -> walk::Result<bool> {
        Ok(true)
    }
    fn interaction_arrival(&mut self, s: &mut State, a: &Approach) -> walk::Result<bool> {
        ground_runtime::arrival(s, self, a)
    }
    fn delayed_interaction(&mut self, s: &mut State, t: &legacy::Task) -> walk::Result<bool> {
        legacy_runtime::arrival(s, self, t).map_err(|e| walk::Error::Binding(e.to_string()))
    }
}
macro_rules! legacy_delegate {
    ($name:ident($($arg:ident:$ty:ty),*)->$ret:ty)=>{
        fn $name(&mut self,s:&mut State,$($arg:$ty),*)->legacy::Result<$ret>{legacy_runtime::Bindings::$name(&mut self.legacy,s,$($arg),*)}
    }
}
impl legacy_runtime::Bindings for Host {
    legacy_delegate!(interaction_locked(p: Option<Node>) -> bool);
    fn handler_walk(&mut self, _: &mut State, h: usize) -> legacy::Result<bool> {
        self.table
            .registry
            .handler(h)
            .map_err(|e| legacy::Error::Binding(Some(e.to_string())))?;
        Ok(true)
    }
    fn handler_walk_for(
        &mut self,
        _: &mut State,
        _: usize,
        _: Option<Node>,
        _: Option<Node>,
    ) -> legacy::Result<bool> {
        unreachable!()
    }
    fn handler_delayed(
        &mut self,
        _: &mut State,
        _: usize,
        _: Option<Node>,
    ) -> legacy::Result<bool> {
        unreachable!()
    }
    fn handler_name(&mut self, _: &mut State, _: usize) -> legacy::Result<String> {
        unreachable!()
    }
    fn handler_handle(
        &mut self,
        s: &mut State,
        h: usize,
        _: Option<Node>,
        t: Option<Node>,
        o: &str,
    ) -> legacy::Result<bool> {
        let handler = self
            .table
            .registry
            .handler(h)
            .map_err(|e| legacy::Error::Binding(Some(e.to_string())))?;
        assert!(handler.read().value::<pickup::Program>().is_some());
        self.arrivals
            .push((h, s.ground(t.unwrap()).unwrap(), o.into()));
        Ok(true)
    }
    fn lowercase(&self, s: &str) -> String {
        legacy_runtime::Bindings::lowercase(&self.legacy, s)
    }
    fn zone(
        &mut self,
        s: &mut State,
        p: Option<Node>,
        t: Option<Node>,
        o: Option<&OptionHandle>,
    ) -> legacy::Result<bool> {
        if let Some(o) = o {
            self.zones.push(o.clone());
        }
        legacy_runtime::Bindings::zone(&mut self.legacy, s, p, t, o)
    }
    legacy_delegate!(combat_stop(p: Option<Node>) -> ());
    legacy_delegate!(face_player(p: Option<Node>, l: Option<Location>) -> ());
    legacy_delegate!(dispatch(p: Option<Node>, t: Option<Node>, o: &str) -> ());
    legacy_delegate!(exception(e: &legacy::Error) -> ());
    legacy_delegate!(log(c: &str, m: &str) -> ());
}
fn fixture() -> (State, Host, Request) {
    let (mut state, legacy) = legacy_fixture::create(&json!({}));
    let table = table_fixture::Harness::new(&json!({}));
    // The table-only constructor starts with the original 100-tick private
    // interval, even without a dropper. This composition uses a public item.
    table.ground.write().force_visible = true;
    table
        .registry
        .initialize(
            &mut state,
            &table.spawns,
            &table.definitions,
            &table.ground,
            |s| Ok(s.to_lowercase()),
        )
        .unwrap();
    let lookup = Lookup {
        chunk: Shared::new(RegionChunk::new(Location::new(3200, 3200, 0), 0)),
        calls: 0,
    };
    chunk_sync::get_items(&lookup.chunk)
        .write()
        .push(Some(table.ground.clone()));
    let player = state.retain_node(legacy.base.player).unwrap();
    let action = ClientAction::decode(66, &mut Reader::new(&[0x87, 0x0c, 0x03, 0xe3, 0x10, 0x0c]))
        .unwrap()
        .unwrap();
    let request = Request::from_action(player, &action).unwrap();
    let host = Host {
        legacy,
        table,
        lookup,
        events: vec![],
        arrivals: vec![],
        zones: vec![],
        plugin_handles: false,
        listener_handles: None,
        replace_table: false,
        foreign_capture: None,
    };
    (state, host, request)
}
#[test]
fn real_take_frame_selects_native_handler_and_waits_for_canonical_arrival() {
    let (mut s, mut h, r) = fixture();
    let expected = h.table.ground.clone();
    ground_runtime::process(&mut s, &mut h, &r).unwrap();
    assert_eq!(
        h.events,
        [
            "lookup",
            "reset",
            "interaction",
            "plugin",
            "GroundItem",
            "Item",
            "interaction"
        ]
    );
    assert!(h.arrivals.is_empty());
    assert!(
        s.actor(r.player.as_ref().unwrap().node)
            .unwrap()
            .manager
            .current()
            .is_some()
    );
    s.context(&mut h).pulses().unwrap();
    assert_eq!(h.arrivals.len(), 1);
    assert!(h.arrivals[0].1.same(&expected));
    assert_eq!(h.arrivals[0].2, "take");
    // Movement completion clears its minimap after the actual arrival.
    assert_eq!(h.events.iter().filter(|e| *e == "minimap").count(), 1);
}
#[test]
fn listener_table_replacement_retains_the_original_selected_option() {
    let (mut s, mut h, r) = fixture();
    h.replace_table = true;
    ground_runtime::process(&mut s, &mut h, &r).unwrap();
    s.context(&mut h).pulses().unwrap();
    assert_eq!(h.arrivals.len(), 1);
    assert_eq!(h.arrivals[0].2, "take");
    let c = h
        .table
        .registry
        .capture(&mut s, &h.table.spawns, &h.table.ground)
        .unwrap();
    assert!(options::get(&c.table, 2).unwrap().is_none());
}
#[test]
fn lookup_miss_resets_scripts_and_clears_minimap_without_interaction() {
    let (mut s, mut h, mut r) = fixture();
    r.id = 996;
    ground_runtime::process(&mut s, &mut h, &r).unwrap();
    assert_eq!(h.events, ["lookup", "reset", "minimap"]);
    assert!(s.pending().is_empty());
}

#[test]
fn a_partial_table_preserves_index_failure_and_the_completed_request_prefix() {
    let (mut s, mut h, r) = fixture();
    let c = h
        .table
        .registry
        .capture(&mut s, &h.table.spawns, &h.table.ground)
        .unwrap();
    c.table.read().options().write().truncate(2);
    assert!(matches!(
        ground_runtime::process(&mut s, &mut h, &r),
        Err(gm::Error::Index)
    ));
    assert_eq!(h.events, ["lookup", "reset", "interaction"]);
    assert!(s.pending().is_empty());
    assert!(h.arrivals.is_empty());
}
#[test]
fn plugin_and_ordered_listener_short_circuits_do_not_schedule_pickup() {
    for stage in 0..3 {
        let (mut s, mut h, r) = fixture();
        h.plugin_handles = stage == 0;
        h.listener_handles = match stage {
            1 => Some(Listener::GroundItem),
            2 => Some(Listener::Item),
            _ => None,
        };
        ground_runtime::process(&mut s, &mut h, &r).unwrap();
        assert_eq!(h.events.len(), 4 + stage);
        assert!(s.pending().is_empty());
        assert!(h.arrivals.is_empty());
    }
}
#[test]
fn invalid_option_uses_the_world_null_object_without_a_second_interaction_read() {
    let (mut s, mut h, mut r) = fixture();
    r.option = 1;
    let c = h
        .table
        .registry
        .capture(&mut s, &h.table.spawns, &h.table.ground)
        .unwrap();
    let null = c.null_option();
    ground_runtime::process(&mut s, &mut h, &r).unwrap();
    assert_eq!(h.events, ["lookup", "reset", "interaction", "minimap"]);
    // The canonical movement's arrival capture must preserve Option.NULL's identity.
    let pulse = s.current(r.player.as_ref().unwrap().node).unwrap().unwrap();
    assert!(pulse.base().read().running);
    drop(pulse);
    s.context(&mut h).pulses().unwrap();
    assert!(h.arrivals.is_empty());
    assert_eq!(h.zones.len(), 1);
    assert!(h.zones[0].same(&null));
    assert!(c.null_option().same(&null));
}
#[test]
fn foreign_player_is_rejected_before_lookup_or_script_mutation() {
    let (mut s, mut h, mut r) = fixture();
    let mut other = State::default();
    let node = other.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    r.player = Some(other.retain_node(node).unwrap());
    assert!(ground_runtime::process(&mut s, &mut h, &r).is_err());
    assert!(h.events.is_empty());
    assert_eq!(h.lookup.calls, 0);
}

#[test]
fn a_ground_target_cannot_be_used_as_the_request_player() {
    let (mut s, mut h, mut r) = fixture();
    r.player = Some(
        h.table
            .registry
            .capture(&mut s, &h.table.spawns, &h.table.ground)
            .unwrap()
            .target,
    );
    assert!(ground_runtime::process(&mut s, &mut h, &r).is_err());
    assert!(h.events.is_empty());
    assert_eq!(h.lookup.calls, 0);
}
#[test]
fn foreign_captured_target_is_rejected_before_legacy_dispatch() {
    let (mut s, mut h, r) = fixture();
    let mut foreign = table_fixture::Harness::new(&json!({}));
    foreign.initialize().unwrap();
    h.foreign_capture = Some(foreign.captured());
    assert!(ground_runtime::process(&mut s, &mut h, &r).is_err());
    assert!(h.arrivals.is_empty());
    assert!(s.pending().is_empty());
}

#[test]
fn arrival_retains_the_original_ground_after_a_same_id_tile_replacement() {
    let (mut s, mut h, r) = fixture();
    let original = h.table.ground.downgrade();
    ground_runtime::process(&mut s, &mut h, &r).unwrap();
    let input = ground::as_item(&h.table.ground);
    let replacement = ground::construct(
        &mut table_fixture::Construction(&h.table.definitions),
        &input,
        Some(Location::new(3207, 3216, 0)),
        200,
        None,
    )
    .unwrap();
    replacement.write().force_visible = true;
    h.table
        .registry
        .initialize(
            &mut s,
            &h.table.spawns,
            &h.table.definitions,
            &replacement,
            |s| Ok(s.to_lowercase()),
        )
        .unwrap();
    h.table.ground = replacement.clone();
    let items = chunk_sync::get_items(&h.lookup.chunk);
    items.write().clear();
    items.write().push(Some(replacement.clone()));
    s.collect();
    assert!(original.upgrade().is_some());
    s.context(&mut h).pulses().unwrap();
    assert_eq!(h.arrivals.len(), 1);
    assert!(h.arrivals[0].1.same(&original.upgrade().unwrap()));
    assert!(!h.arrivals[0].1.same(&replacement));
}
