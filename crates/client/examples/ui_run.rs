//! Run real cached interface hooks without a GPU, with explicit empty/offline inputs.
use rs09_client::{
    pack::Pack,
    ui::{state::State, vm::Vm},
};
fn main() {
    let pack = Pack::decode(std::fs::read("assets/visual/ui.pack").unwrap()).unwrap();
    let mut state = State::new(pack);
    let mut vm = Vm::default();
    vm.window_mode = 2;
    vm.cycle = 100;
    vm.location = [3222, 3218, 0];
    state.open_root(746).unwrap();
    state.layout(1138, 1244).unwrap();
    vm.drain(&mut state);
    for arg in std::env::args().skip(1) {
        if arg.starts_with("--") {
            continue;
        }
        if arg.ends_with(".bin") {
            let bytes = std::fs::read(arg).unwrap();
            let mut decoder = rs09_protocol::game::Decoder::new(
                rs09_protocol::game::Direction::ServerToClient,
                1_000_000,
                65535,
            );
            decoder.push(&bytes).unwrap();
            while let Some(frame) = decoder.next_frame().unwrap() {
                vm.apply_frame(&mut state, &frame).unwrap();
            }
            state.layout(1138, 1244).unwrap();
            vm.drain(&mut state);
            continue;
        }
        let group = arg.parse::<i32>().unwrap();
        state.open_sub(746 << 16 | 6, group, 0).unwrap();
        state.layout(1138, 1244).unwrap();
        vm.drain(&mut state);
    }
    for cycle in 100..120 {
        vm.cycle = cycle;
        vm.poll_updates(&mut state, i64::from(cycle) * 20).unwrap();
        let nodes = state.layout(1138, 1244).unwrap();
        state.queue_events(&nodes);
        vm.drain(&mut state);
    }
    let nodes = state.layout(1138, 1244).unwrap();
    if std::env::args().any(|s| s == "--nodes") {
        for node in &nodes {
            if node.key.id >> 16 == 746 && (55..=69).contains(&(node.key.id & 65535)) {
                println!("{node:?}");
            }
        }
    }
    if std::env::args().any(|s| s == "--drag") {
        use rs09_client::{
            interaction::Menu,
            ui::{drag::Controller, state::Key},
        };
        let from = nodes
            .iter()
            .find(|n| {
                n.key
                    == Key {
                        id: 762 << 16 | 73,
                        child: 0,
                    }
            })
            .unwrap();
        let to = nodes
            .iter()
            .find(|n| {
                n.key
                    == Key {
                        id: 762 << 16 | 73,
                        child: 1,
                    }
            })
            .unwrap();
        let point = |n: &rs09_client::ui::state::DrawNode| [n.rect.x + 16, n.rect.y + 16];
        let mut menu = Menu::default();
        state.menu_at(&nodes, point(from), &mut menu);
        menu.sort();
        let mut drag = Controller::default();
        println!(
            "Begin drag: {}",
            drag.begin(
                &state,
                &nodes,
                point(from),
                120,
                menu.entries.last().cloned()
            )
        );
        for (cycle, down) in [(126, true), (127, false)] {
            let (hooks, click, packet) =
                drag.step(&mut state, &nodes, point(to), cycle, down, false);
            println!(
                "Drag hooks={} click={click:?} packet={packet:?}",
                hooks.len()
            );
            for hook in hooks {
                println!("Hook {:?} target {:?}", hook.arguments, hook.target);
                if let Err(e) = vm.run(&mut state, &hook, 200000) {
                    state.diagnostic(e);
                }
            }
            println!(
                "Resolved drag packet: {:?}",
                packet.and_then(|release| release.resolve(&state))
            );
        }
        println!("Queued UI action packets: {:?}", vm.outgoing);
    }
    println!(
        "Executed {} instructions; {} components; {} visible nodes",
        vm.executed,
        state.components.len(),
        nodes.len()
    );
    for diagnostic in &state.diagnostics {
        println!("{diagnostic}");
    }
    if let Some(path) = std::env::var_os("RS09_UI_DUMP") {
        let dump: Vec<_> = state
            .components
            .iter()
            .map(|(key, c)| serde_json::json!({"id":key.id,"child":key.child,"component":c}))
            .collect();
        std::fs::write(path, serde_json::to_vec(&dump).unwrap()).unwrap();
    }
}
