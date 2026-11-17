//! Drives the title interface of the pinned cache without a window: the
//! cache's own components and scripts, a scripted mouse and keyboard, and the
//! per-cycle order of the client's main loop (interface walk, hooks, then the
//! component drag). A development aid for looking at what each control does.
//!
//! `cargo run -p rs09-client --example title_probe -- [modes=N] [size=WxH]
//! [root=GROUP] [sub=COMPONENT:GROUP]... ACTION...`
//!
//! The root interface is the title (744) unless `root=` names another;
//! `sub=11:742` opens group 742 in the root's component 11 (the fixed
//! gameframe's main window), so the in-game settings can be walked too.
//!
//! Actions: `click:N` or `click:G.N` (press and release on component N of
//! the root interface, or of group G),
//! `press:N`, `drag:X,Y`, `release`, `at:X,Y` (click at a canvas point),
//! `key:CODE,CHAR`, `text:STRING`, `wait:CYCLES`, `show`, `nodes`, `comp:N`.
use rs09_client::{
    display::{Display, Mode, System},
    ui::{
        drag::Controller,
        state::{DrawNode, Key, PointerInput, State},
        vm::Vm,
    },
};

struct Probe {
    state: State,
    vm: Vm,
    nodes: Vec<DrawNode>,
    drag: Controller,
    size: [i32; 2],
    point: [i32; 2],
    down: bool,
    interface: i32,
    reported: usize,
}

impl Probe {
    fn layout(&mut self) {
        self.nodes = self
            .state
            .layout(self.size[0], self.size[1])
            .unwrap_or_else(|error| {
                println!("  layout: {error}");
                Vec::new()
            });
    }

    /// One 20 ms cycle and its frame, in the main loop's order.
    fn cycle(&mut self, clicked: bool, keys: &[[i32; 2]]) {
        let (state, vm) = (&mut self.state, &mut self.vm);
        vm.cycle += 1;
        if let Err(error) = vm.poll_updates(state, i64::from(vm.cycle) * 20) {
            println!("  poll: {error}");
        }
        state.key_events.clear();
        state.key_events.extend_from_slice(keys);
        let pointer = PointerInput {
            point: self.point,
            clicked,
            down: self.down,
            any_down: self.down,
            scroll: 0,
            suppressed: self.drag.active(),
            click: None,
        };
        state.queue_cycle_events(Some(pointer), None);
        vm.drain(state);
        state.finish_input_events();
        if !vm.display.effects.is_empty() {
            println!("  display effects: {:?}", vm.display.effects);
            vm.display.effects.clear();
        }
        self.layout();
        let (state, vm) = (&mut self.state, &mut self.vm);
        // The frame's part: the component drag.
        if clicked
            && self
                .drag
                .begin(state, &self.nodes, self.point, vm.cycle, None)
        {
            println!("  drag began");
        }
        if self.drag.active() {
            let (hooks, _, _) =
                self.drag
                    .step(state, &self.nodes, self.point, vm.cycle, self.down, false);
            for hook in hooks {
                if let Err(error) = vm.run(state, &hook, 200_000) {
                    state.diagnostic(error);
                }
            }
            if !self.drag.active() {
                println!("  drag ended");
            }
        }
        self.report();
    }

    fn report(&mut self) {
        let diagnostics: Vec<_> = self.state.diagnostics.iter().cloned().collect();
        for text in diagnostics
            .iter()
            .skip(self.reported.min(diagnostics.len()))
        {
            println!("  DIAGNOSTIC: {text}");
        }
        self.reported = diagnostics.len();
        for url in self.vm.requested_urls.drain(..) {
            println!("  url: {url}");
        }
    }

    /// `N` on the root interface, or `G.N` on group G.
    fn key(&self, name: &str) -> Key {
        match name.split_once('.') {
            Some((group, child)) => {
                Key::component(group.parse::<i32>().unwrap() << 16 | child.parse::<i32>().unwrap())
            }
            None => Key::component(self.interface << 16 | name.parse::<i32>().unwrap()),
        }
    }

    /// The middle of a laid-out component's visible part.
    fn centre(&self, name: &str) -> Option<[i32; 2]> {
        let key = self.key(name);
        self.nodes.iter().find(|node| node.key == key).map(|node| {
            let clip = node.clip;
            [clip.x + clip.w / 2, clip.y + clip.h / 2]
        })
    }

    fn node(&self, component: i32) -> Option<DrawNode> {
        let key = Key::component(self.interface << 16 | component);
        self.nodes.iter().copied().find(|node| node.key == key)
    }

    fn press(&mut self, point: [i32; 2]) {
        self.point = point;
        self.cycle(false, &[]);
        self.down = true;
        self.cycle(true, &[]);
        self.cycle(false, &[]);
    }

    fn release(&mut self) {
        self.down = false;
        self.cycle(false, &[]);
        self.cycle(false, &[]);
    }

    fn show(&self) {
        let hidden = |component: i32| {
            self.state
                .components
                .get(&Key::component(self.interface << 16 | component))
                .map(|c| c.hidden)
        };
        let panels = [
            (226, "main menu"),
            (138, "graphics"),
            (402, "audio"),
            (241, "login"),
            (176, "world select"),
            (403, "create 1"),
            (404, "create 2"),
            (405, "create 3"),
            (406, "create 4"),
            (409, "create 5"),
            (410, "create 6"),
            (411, "message"),
            (237, "confirm"),
            (242, "pvp"),
        ];
        let open: Vec<_> = panels
            .iter()
            .filter(|(component, _)| hidden(*component) == Some(false))
            .map(|(_, name)| *name)
            .collect();
        println!("  panels shown: {open:?}");
        let p = &self.vm.preferences;
        println!(
            "  window mode {} remembered {} canvas {:?}; brightness {} roofs {}/{} decor {} textures {} idle {} flicker {} char shadows {} scenery shadows {} lighting {} water {} fog {} aa {} stereo {} volumes {}/{}/{} writes {}",
            self.vm.display.window_mode(),
            p.remembered_mode,
            self.vm.display.canvas,
            p.brightness,
            p.remove_roofs_selectively,
            p.never_remove_roofs,
            p.show_ground_decorations,
            p.high_detail_textures,
            p.many_idle_animations,
            p.flickering_effects,
            p.character_shadows,
            p.scenery_shadows,
            p.high_detail_lighting,
            p.high_water_detail,
            p.fog,
            p.anti_aliasing,
            p.stereo,
            p.music_volume,
            p.sound_volume,
            p.ambient_volume,
            p.writes
        );
    }
}

fn main() {
    let cache = std::path::Path::new("assets/cache");
    let pack = rs09_client::cache_pack::cache_pack(cache, "probe").expect("assets/cache");
    let mut modes = 1;
    let mut size = [765, 503];
    let mut actions = Vec::new();
    let mut root = 744;
    let mut subs = Vec::new();
    for arg in std::env::args().skip(1) {
        if let Some(value) = arg.strip_prefix("root=") {
            root = value.parse().unwrap();
        } else if let Some(value) = arg.strip_prefix("sub=") {
            let (component, group) = value.split_once(':').unwrap();
            subs.push((
                component.parse::<i32>().unwrap(),
                group.parse::<i32>().unwrap(),
            ));
        } else if let Some(value) = arg.strip_prefix("modes=") {
            modes = value.parse().unwrap();
        } else if let Some(value) = arg.strip_prefix("size=") {
            let (w, h) = value.split_once('x').unwrap();
            size = [w.parse().unwrap(), h.parse().unwrap()];
        } else {
            actions.push(arg);
        }
    }
    let system = System::client(
        (0..modes)
            .map(|index| Mode {
                width: 1024 + index * 256,
                height: 768 + index * 192,
                bit_depth: 32,
                refresh_rate: 60,
            })
            .collect(),
    );
    let mut vm = Vm::default();
    vm.display = Display::new([1024, 768], system);
    vm.cycle = 100;
    vm.now_ms = 1_700_000_000_000;
    let remembered = vm.preferences.remembered_mode;
    vm.display
        .set_window_mode(false, remembered, -1, -1, &mut vm.preferences);
    vm.window_mode = vm.display.window_mode();
    vm.display.effects.clear();
    let interface = root;
    let mut state = State::new(pack);
    state.open_root(interface).unwrap();
    for (component, group) in subs {
        state
            .open_sub(interface << 16 | component, group, 0)
            .unwrap();
    }
    vm.drain(&mut state);
    let mut probe = Probe {
        state,
        vm,
        nodes: Vec::new(),
        drag: Controller::default(),
        size,
        point: [-1, -1],
        down: false,
        interface,
        reported: 0,
    };
    probe.layout();
    for _ in 0..5 {
        probe.cycle(false, &[]);
    }
    println!(
        "opened {interface}: window mode {}",
        probe.vm.display.window_mode()
    );
    probe.show();
    for action in actions {
        println!("> {action}");
        let (name, value) = action.split_once(':').unwrap_or((&action, ""));
        let numbers: Vec<i32> = value.split(',').filter_map(|n| n.parse().ok()).collect();
        match name {
            "click" | "press" => match probe.centre(value) {
                Some(point) => {
                    println!("  at {point:?}");
                    probe.press(point);
                    if name == "click" {
                        probe.release();
                    }
                }
                None => println!("  component {value} is not laid out"),
            },
            "at" => {
                probe.press([numbers[0], numbers[1]]);
                probe.release();
            }
            "drag" => {
                probe.point = [numbers[0], numbers[1]];
                for _ in 0..8 {
                    probe.cycle(false, &[]);
                }
            }
            "release" => probe.release(),
            "key" => probe.cycle(false, &[[numbers[0], numbers[1]]]),
            "text" => {
                for character in value.chars() {
                    probe.cycle(false, &[[-1, character as i32]]);
                }
            }
            "wait" => {
                for _ in 0..numbers[0] {
                    probe.cycle(false, &[]);
                }
            }
            "show" => probe.show(),
            "comp" => {
                let key = Key::component(interface << 16 | numbers[0]);
                match probe.state.components.get(&key) {
                    Some(c) => println!(
                        "  hidden {} text {:?} sprite {} colour {:x} rect [{},{} {}x{}] node {:?}",
                        c.hidden,
                        c.text,
                        c.sprite_id,
                        c.color,
                        c.x,
                        c.y,
                        c.width,
                        c.height,
                        probe.node(numbers[0]).map(|n| n.clip)
                    ),
                    None => println!("  no such component"),
                }
            }
            "nodes" => {
                for node in &probe.nodes {
                    let c = &probe.state.components[&node.key];
                    if c.on_click_repeat.is_some() || c.on_drag_release.is_some() {
                        println!(
                            "  {}.{}:{} clip {:?} text {:?} click {:?}",
                            node.key.id >> 16,
                            node.key.id & 65535,
                            node.key.child,
                            node.clip,
                            c.text,
                            c.on_click_repeat
                        );
                    }
                }
            }
            other => println!("  unknown action {other}"),
        }
        probe.report();
    }
    probe.show();
}
