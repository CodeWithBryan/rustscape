//! Opens the world map interface (755) the way the server does, without a
//! GPU: the real components and scripts from the cache, the interface pass
//! with a scripted mouse, and the software frames the client presents for
//! the components with client code 1400 and 1401.
//!
//! `cargo run -p rs09-client --example world_map_ui -- [--cache DIR]
//! [--size WxH] [--font PATH.ttf] [--output DIR]`
//!
//! Writes `map.png`, `map-dragged.png`, `map-zoomed.png`, `overview.png` and
//! `map-recentred.png` into the output directory (default
//! `runs/visual/world-map-ui`) and prints what happened.
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use rs09_client::ui::{
        state::{DrawNode, Key, PointerInput, State},
        vm::Vm,
        world_map_host::{self, Startup, Surface},
    };
    use std::path::Path;

    fn png(path: &Path, surface: &Surface) {
        let mut bytes = Vec::with_capacity(surface.pixels.len() * 4);
        // export_png writes its rows bottom-up, as for a framebuffer capture.
        for row in surface.pixels.chunks_exact(surface.size[0] as usize).rev() {
            for &pixel in row {
                bytes.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8, 255]);
            }
        }
        macroquad::texture::Image {
            bytes,
            width: surface.size[0] as u16,
            height: surface.size[1] as u16,
        }
        .export_png(&path.to_string_lossy());
        println!("wrote {}", path.display());
    }

    struct Session {
        state: State,
        vm: Vm,
        size: [i32; 2],
        nodes: Vec<DrawNode>,
    }
    impl Session {
        fn node(&self, client_code: i32) -> Option<DrawNode> {
            self.nodes
                .iter()
                .copied()
                .find(|node| self.state.components[&node.key].client_code == client_code)
        }
        /// One 20 ms logic cycle and one drawn frame, in the client's order.
        fn frame(&mut self, input: PointerInput) -> Result<(), String> {
            let (state, vm) = (&mut self.state, &mut self.vm);
            vm.cycle += 1;
            vm.poll_updates(state, i64::from(vm.cycle) * 20)?;
            self.nodes = state.layout(self.size[0], self.size[1])?;
            state.queue_input_events(&self.nodes, Some(input));
            vm.drain(state);
            state.finish_input_events();
            self.nodes = state.layout(self.size[0], self.size[1])?;
            for node in self.nodes.clone() {
                match state.components[&node.key].client_code {
                    1400 => {
                        world_map_host::render_map(state, vm, node.rect.w, node.rect.h)?;
                    }
                    1401 => world_map_host::render_overview(state, vm, node.rect.w, node.rect.h)?,
                    _ => {}
                }
            }
            Ok(())
        }
    }

    pub fn run() -> Result<(), String> {
        let mut options = std::collections::BTreeMap::new();
        let mut args = std::env::args().skip(1);
        while let Some(name) = args.next() {
            let value = args.next().ok_or_else(|| format!("{name} needs a value"))?;
            options.insert(name, value);
        }
        let cache = options
            .get("--cache")
            .map_or("assets/cache", String::as_str);
        let output = options
            .get("--output")
            .map_or("runs/visual/world-map-ui", String::as_str);
        let size = match options.get("--size") {
            Some(text) => {
                let (w, h) = text.split_once('x').ok_or("--size takes WxH")?;
                [
                    w.parse().map_err(|_| "--size takes WxH")?,
                    h.parse().map_err(|_| "--size takes WxH")?,
                ]
            }
            None => [765, 503],
        };
        std::fs::create_dir_all(output).map_err(|e| format!("{output}: {e}"))?;
        let pack = rs09_client::cache_pack::cache_pack(Path::new(cache), "example")?;
        let mut state = State::new(pack);
        let typeface = world_map_host::system_typeface(options.get("--font").map(Path::new));
        if typeface.is_none() {
            println!("no label font found: the map is drawn without labels");
        }
        state.world_map.open_cache(
            Path::new(cache),
            Startup {
                jitters: None,
                seed: 0,
                typeface,
            },
        )?;
        let mut vm = Vm::default();
        vm.window_mode = 2;
        vm.cycle = 100;
        vm.location = [3222, 3218, 0];
        // IF_OPENSUB (145), the top-level open: interface 755, reset 0.
        let mut open = rs09_protocol::buffer::Writer::new();
        open.ip2add(755);
        open.p1add(0);
        open.ip2add(0);
        vm.apply_frame(
            &mut state,
            &rs09_protocol::game::Frame {
                opcode: 145,
                payload: open.into_vec(),
            },
        )?;
        let mut session = Session {
            state,
            vm,
            size,
            nodes: Vec::new(),
        };
        let idle = PointerInput {
            point: [-1, -1],
            ..Default::default()
        };
        let mut frames = 0;
        while !session.state.world_map.map.is_loaded() {
            session.frame(idle)?;
            frames += 1;
            if frames > 40 {
                return Err("the map did not load".into());
            }
        }
        // A few more cycles for the interface's timer scripts.
        for _ in 0..5 {
            session.frame(idle)?;
        }
        let map = session
            .node(1400)
            .ok_or("no component with client code 1400")?;
        println!(
            "interface 755 open: {} nodes, map component {:?}, loaded after {frames} frames",
            session.nodes.len(),
            map.rect
        );
        let world = &session.state.world_map.map;
        println!(
            "view ({}, {}) of {}x{} tiles at zoom {}, {} labels, {} pings queued",
            world.view_x,
            world.view_y,
            world.width,
            world.length,
            world.zoom,
            world.labels.as_ref().map_or(0, |labels| labels.count()),
            session
                .vm
                .outgoing
                .iter()
                .filter(|frame| frame.opcode == 93)
                .count()
        );
        png(
            &Path::new(output).join("map.png"),
            &session.state.world_map.map_surface,
        );

        // Press in the middle of the map and drag 90 pixels left and 60 down.
        let centre = [map.rect.x + map.rect.w / 2, map.rect.y + map.rect.h / 2];
        let before = [world.view_x, world.view_y];
        let held = |point| PointerInput {
            point,
            down: true,
            any_down: true,
            ..Default::default()
        };
        session.frame(PointerInput {
            clicked: true,
            ..held(centre)
        })?;
        for step in 1..=3 {
            session.frame(held([centre[0] - 30 * step, centre[1] + 20 * step]))?;
        }
        session.frame(PointerInput {
            point: [centre[0] - 90, centre[1] + 60],
            ..Default::default()
        })?;
        let world = &session.state.world_map.map;
        println!(
            "dragged the view from {before:?} to {:?}",
            [world.view_x, world.view_y]
        );
        png(
            &Path::new(output).join("map-dragged.png"),
            &session.state.world_map.map_surface,
        );

        // The interface's own buttons: "Show overview" (component 18), the
        // key's "Highlight" for banks (component 12, row 11) and "100% zoom"
        // (component 39). Their scripts use commands 5202 and 5200.
        let component = |id: i32, child: i32| Key {
            id: 755 << 16 | id,
            child,
        };
        for key in [component(18, -1), component(12, 11), component(39, -1)] {
            session.vm.click_button(&mut session.state, key, 1, "")?;
            session.frame(idle)?;
        }
        // Zoom eases by a thirtieth per cycle from 6 to 8.
        let mut cycles = 1;
        while session.state.world_map.map.zoom != session.state.world_map.map.target_zoom {
            session.frame(idle)?;
            cycles += 1;
            if cycles > 100 {
                return Err("the zoom did not settle".into());
            }
        }
        let world = &session.state.world_map.map;
        println!(
            "zoom {} after {cycles} cycles; highlighting map function {} ({} pulses left)",
            world.zoom, world.highlighted_map_function, world.highlight_pulse_count
        );
        png(
            &Path::new(output).join("map-zoomed.png"),
            &session.state.world_map.map_surface,
        );
        let overview = session
            .node(1401)
            .ok_or("the overview component (client code 1401) is not shown")?;
        println!("overview component {:?}", overview.rect);
        png(
            &Path::new(output).join("overview.png"),
            &session.state.world_map.overview_surface,
        );
        // A press on the overview recentres the map there.
        let corner = [overview.rect.x + 20, overview.rect.y + 30];
        session.frame(PointerInput {
            clicked: true,
            ..held(corner)
        })?;
        session.frame(PointerInput {
            point: corner,
            ..Default::default()
        })?;
        let world = &session.state.world_map.map;
        println!(
            "pressed the overview at (20, 30): the view is now {:?}",
            [world.view_x, world.view_y]
        );
        png(
            &Path::new(output).join("map-recentred.png"),
            &session.state.world_map.map_surface,
        );
        for diagnostic in &session.state.diagnostics {
            println!("diagnostic: {diagnostic}");
        }
        Ok(())
    }
}
