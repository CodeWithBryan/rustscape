//! The world map through the client's own interface path: interface 755 of
//! the pinned cache with its real components and scripts, opened by the
//! server's top-level packet, driven by the interface pass with a scripted
//! mouse and drawn into the software frames the client presents.
//!
//! Nothing here ran in a JVM. The expected values are worked out by hand
//! from the Java callers (`InterfaceList` 750-789, `Protocol` 1284, 2581,
//! 2792 and 2812, `WorldMap.updateZoomAndScroll`) and from the map facts
//! `tests/world_map.rs` holds from the JVM. What it establishes beyond the
//! unit tests is that the cache's own scripts (1369, 1371, 1373, 1381, 2045
//! and the ones they call) run to completion against commands 5200..=5220.
//! The GPU presentation is not covered here.
#![cfg(not(target_arch = "wasm32"))]
use rs09_client::ui::{
    state::{DrawNode, Key, PointerInput, State},
    vm::Vm,
    world_map_host::{self, SceneRequests, Startup},
};
use rs09_protocol::game::Frame;

struct Session {
    state: State,
    vm: Vm,
    nodes: Vec<DrawNode>,
}
impl Session {
    const SIZE: [i32; 2] = [765, 503];

    fn node(&self, client_code: i32) -> Option<DrawNode> {
        self.nodes
            .iter()
            .copied()
            .find(|node| self.state.components[&node.key].client_code == client_code)
    }

    /// One 20 ms logic cycle and one drawn frame, in the client's order.
    fn frame(&mut self, input: PointerInput) {
        let (state, vm) = (&mut self.state, &mut self.vm);
        vm.cycle += 1;
        vm.poll_updates(state, i64::from(vm.cycle) * 20).unwrap();
        self.nodes = state.layout(Self::SIZE[0], Self::SIZE[1]).unwrap();
        state.queue_input_events(&self.nodes, Some(input));
        vm.drain(state);
        state.finish_input_events();
        self.nodes = state.layout(Self::SIZE[0], Self::SIZE[1]).unwrap();
        for node in self.nodes.clone() {
            match state.components[&node.key].client_code {
                1400 => {
                    world_map_host::render_map(state, vm, node.rect.w, node.rect.h).unwrap();
                }
                1401 => {
                    world_map_host::render_overview(state, vm, node.rect.w, node.rect.h).unwrap()
                }
                _ => {}
            }
        }
    }

    /// IF_OPENSUB (145), the top-level open.
    fn open_top_level(&mut self, interface: i32, reset: i32) {
        let mut w = rs09_protocol::buffer::Writer::new();
        w.ip2add(interface);
        w.p1add(reset);
        w.ip2add(0);
        let frame = Frame {
            opcode: 145,
            payload: w.into_vec(),
        };
        assert!(self.vm.apply_frame(&mut self.state, &frame).unwrap());
    }

    fn click(&mut self, component: i32, child: i32) {
        let key = Key {
            id: 755 << 16 | component,
            child,
        };
        self.vm.click_button(&mut self.state, key, 1, "").unwrap();
    }

    fn view(&self) -> [i32; 2] {
        [
            self.state.world_map.map.view_x,
            self.state.world_map.map.view_y,
        ]
    }
}

/// The mouse at `point` with no button down.
fn hover(point: [i32; 2]) -> PointerInput {
    PointerInput {
        point,
        ..Default::default()
    }
}
fn held(point: [i32; 2], clicked: bool) -> PointerInput {
    PointerInput {
        clicked,
        down: true,
        any_down: true,
        ..hover(point)
    }
}

#[test]
fn the_world_map_interface_of_the_pinned_cache_opens_drags_and_closes() {
    let cache = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cache");
    if !cache.join("main_file_cache.dat2").exists() {
        eprintln!(
            "skipped: {} is absent, so the world map interface was not exercised",
            cache.display()
        );
        return;
    }
    let pack = rs09_client::cache_pack::cache_pack(&cache, "test").unwrap();
    let mut state = State::new(pack);
    state
        .world_map
        .open_cache(
            &cache,
            Startup {
                jitters: None,
                seed: 7,
                typeface: None,
            },
        )
        .unwrap();
    assert!(state.world_map.has_maps());
    let mut vm = Vm::default();
    vm.window_mode = 2;
    vm.cycle = 100;
    vm.location = [3222, 3218, 0];
    let mut session = Session {
        state,
        vm,
        nodes: Vec::new(),
    };

    // The server opens the map as the top-level interface. Its load script
    // selects the map containing the player.
    session.open_top_level(755, 0);
    assert_eq!(session.state.root, 755);
    assert_eq!(
        session
            .state
            .world_map
            .map
            .current_map()
            .map(|map| map.group.as_str()),
        Some("main")
    );
    // One load step per drawn frame: the JVM's 14 steps for this map.
    let mut frames = 0;
    while !session.state.world_map.map.is_loaded() {
        session.frame(hover([-1, -1]));
        frames += 1;
        assert!(frames <= 14, "the map did not load");
    }
    assert_eq!(frames, 14);
    for _ in 0..3 {
        session.frame(hover([-1, -1]));
    }
    let map = session.node(1400).expect("the map component");
    assert_eq!(
        session.state.world_map.map.component,
        Some([map.rect.w, map.rect.h])
    );
    assert_eq!(
        session.state.world_map.map_surface.size,
        [map.rect.w, map.rect.h]
    );
    assert!(session.node(1401).is_none(), "the overview starts hidden");
    // The load asked for the scene to be dropped once and pinged ten times.
    assert_eq!(session.state.world_map.scene_requests().clear_scene, 1);
    let pings = |session: &Session| {
        session
            .vm
            .outgoing
            .iter()
            .filter(|frame| frame.opcode == 93)
            .count()
    };
    assert_eq!(pings(&session), 10);
    // Default zoom 75 is six pixels per two tiles; the view starts within
    // five tiles of the player at (3222, 3218).
    let world = &session.state.world_map.map;
    assert_eq!([world.zoom, world.target_zoom], [6.0, 6.0]);
    let [x, y] = world.view_coord();
    assert!((x - 3222).abs() <= 5 && (y - 3218).abs() <= 5, "{x},{y}");
    assert_eq!(
        world.viewport_size(),
        [map.rect.w * 2 / 6, map.rect.h * 2 / 6]
    );

    // Press in the middle and drag 90 pixels left and 60 down: the view
    // moves (90 * 2 / 6, -60 * 2 / 6) tiles, measured from the press.
    let centre = [map.rect.x + map.rect.w / 2, map.rect.y + map.rect.h / 2];
    let before = session.view();
    session.frame(held(centre, true));
    assert_eq!(session.state.world_map.drag_state, 1);
    assert_eq!(session.view(), before);
    for step in 1..=3 {
        session.frame(held([centre[0] - 30 * step, centre[1] + 20 * step], false));
        assert_eq!(session.state.world_map.drag_state, 2);
        assert_eq!(
            session.view(),
            [before[0] + 10 * step, before[1] - 20 * step / 3]
        );
    }
    session.frame(hover([centre[0] - 90, centre[1] + 60]));
    assert_eq!(session.state.world_map.drag_state, 0);
    assert_eq!(session.view(), [before[0] + 30, before[1] - 20]);

    // The interface's buttons. "Show overview" reveals the 1401 component.
    session.click(18, -1);
    session.frame(hover([-1, -1]));
    let overview = session.node(1401).expect("the overview component");
    assert_eq!(
        session.state.world_map.overview_surface.size,
        [overview.rect.w, overview.rect.h]
    );
    // Not black: the overview of the loaded map with its viewport frame.
    let frame_red = session
        .state
        .world_map
        .overview_surface
        .pixels
        .iter()
        .filter(|&&pixel| pixel & 0xffffff == 0xff0000)
        .count();
    assert!(frame_red > 0);
    // The key's "Highlight" for banks (row 11) starts three pulses on map
    // function 5 through command 5202.
    session.click(12, 11);
    let world = &session.state.world_map.map;
    assert_eq!(
        [world.highlighted_map_function, world.highlight_pulse_count],
        [5, 3]
    );
    // "100% zoom" sets the target through command 5200; the zoom then eases
    // by a thirtieth of itself per cycle: 6 * (31 / 30)^9 first passes 8.
    session.click(39, -1);
    assert_eq!(session.state.world_map.map.target_zoom, 8.0);
    let mut cycles = 0;
    while session.state.world_map.map.zoom != 8.0 {
        session.frame(hover([-1, -1]));
        cycles += 1;
        assert!(cycles <= 9, "the zoom did not settle");
    }
    assert_eq!(cycles, 9);

    // A press on the overview recentres the view under the mouse.
    let at = [overview.rect.x + 20, overview.rect.y + 30];
    session.frame(held(at, true));
    assert_eq!(
        session.view(),
        [1856 * 20 / overview.rect.w, 1664 * 30 / overview.rect.h]
    );
    assert_eq!(session.state.world_map.drag_state, 0);
    session.frame(hover(at));

    // A scene rebuild while the map is open drops it and loads it again.
    session.state.world_map.scene_loading();
    assert!(session.state.world_map.map.current_map().is_none());
    session.frame(hover([-1, -1]));
    assert!(!session.state.world_map.map.is_loaded());
    session.state.world_map.scene_rebuilt();
    for _ in 0..14 {
        session.frame(hover([-1, -1]));
    }
    assert!(session.state.world_map.map.is_loaded());
    // The last step's ping is sent by the cycle after the frame that ran it.
    assert_eq!(pings(&session) + session.state.world_map.pings as usize, 20);
    session.frame(hover([-1, -1]));
    assert_eq!(pings(&session), 20);
    // Every script of the interface ran to its end.
    let diagnostics: Vec<_> = session.state.diagnostics.iter().collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    // Closing: the server reopens the game frame with reset 2. The map is
    // dropped and the scene it cleared is asked for again.
    session.open_top_level(746, 2);
    assert_eq!(session.state.root, 746);
    assert!(session.state.world_map.map.current_map().is_none());
    assert!(session.state.world_map.map.previous_map_name.is_none());
    session.frame(hover([-1, -1]));
    assert!(session.node(1400).is_none());
    assert_eq!(session.state.world_map.map.component, None);
    assert_eq!(
        session.state.world_map.take_scene_requests(),
        SceneRequests {
            clear_scene: 2,
            rebuild_scene: 1,
            reset_timer: 26,
        }
    );
}
