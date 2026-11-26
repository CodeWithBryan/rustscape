//! One 20 ms logic cycle of the original client, in its order, and the state
//! that ties a click to the menu a redraw built.
//!
//! `GameShell.run` (GameShell.java:617-649) runs `logicCycles` whole cycles
//! (`mainLoopWrapper` → `client.mainLoop`) and then, at its own rate, one
//! redraw (`mainInputLoop`, `mainRedrawWrapper` → `client.mainRedraw`).
//! `GameShell.resetTimer()` sets `logicCycles = 0`, so the cycle that calls
//! it is the last of its burst.
//!
//! One cycle, `client.mainLoop` (client.java:1693-1745):
//!
//! 1. `loop++`, the hover text's date seed every 1000th cycle, the cache
//!    connection, `MidiPlayer.loop`, `audioLoop`, `Keyboard.loop`,
//!    `Mouse.loop`, the wheel (1697-1718): [`Step::Devices`], in every state.
//! 2. Game states 25 and 28: `LoginManager.rebuildMap` (1725-1727), which
//!    returns at once while map files are missing and otherwise builds the
//!    scene, writes its keep-alives, and ends in state 30 (from 25) or 10
//!    (from 28): [`Step::Rebuild`]. The second `if` of `mainLoop` then tests
//!    the state again, so the cycle that ends a rebuild goes on as a cycle
//!    of the new state.
//! 3. Game state 10: `mainUpdate` (client.java:1155-1269), then
//!    `CreateManager.loop`, `LoginManager.loopAuto`, `LoginManager.loop`
//!    (1729-1732). `mainUpdate` is the key queue, `sceneDelta++`, the
//!    interface walk and `transmitTimer++` ([`Step::Interfaces`]); the title
//!    scene's NPCs ([`Step::World`]) and camera ([`Step::Camera`]); the
//!    queued hooks ([`Step::Hooks`]); the component drag ([`Step::Drag`]).
//!    No packets, no timers, no mini menu.
//! 4. Game state 30: `Protocol.loop` (Protocol.java:2398-2992):
//!    - 2400-2406 the logout and reboot timers: [`Step::Timers`];
//!    - 2412 at most 100 server packets, each handled as it is read; the
//!      loop does not look at the game state: [`Step::Packets`];
//!    - 2414 return unless the state is still 30 (a rebuild packet made it
//!      25, a logout 10, a lost connection 40); nothing is written;
//!    - 2417-2565 the packets the loop sends by itself from the devices:
//!      mouse recorder 123, click 75, camera 21, focus 22, preferences 98:
//!      [`Step::OwnPackets`];
//!    - 2566-2580 the plane change, pending location changes, the sound
//!      queue, the 750-cycle read time-out, then
//!      `PlayerList.processAllPlayers`, `NpcList.processAllNpcs` and
//!      `OverheadChat.loop`: [`Step::World`];
//!    - 2720-2778 a pressed inventory item: its drag, or on release the swap
//!      (231) or the click it stood for: [`Step::InventoryDrag`];
//!    - 2581-2719 and 2779-2796 the world map's zoom, changed variables,
//!      delayed interface changes, the cross and inventory-press timers, the
//!      key queue, the interface walk, `transmitTimer++`:
//!      [`Step::Interfaces`];
//!    - 2797-3018 the queued hooks: [`Step::Hooks`];
//!    - 2812-2817 the dragged component: [`Step::Drag`];
//!    - 2818-2867 the staff teleports and the tile a walk or a spell was
//!      waiting for ([`Walk::cycle`]): [`Step::Walk`];
//!    - 2868 `processDefaultAction`, the click on the menu the last redraw
//!      built ([`MenuClick`]): [`Step::MenuClick`];
//!    - 2869-2904 hover and tooltip timers, the camera and its shakes:
//!      [`Step::Camera`];
//!    - 2905-2991 the idle logout 245, the drift of the camera and minimap
//!      offsets, the keep-alive 93 after 50 silent cycles, the verification
//!      id 177 and the cycle's one socket write: [`Step::Write`].
//! 5. Game state 40: `LoginManager.loop` and what its reply decides
//!    (1735-1744): [`Step::Login`].
//!
//! One redraw in game state 30, `LoginManager.processInterface`
//! (LoginManager.java:1402-1473): where the menu is built
//! ([`MenuClick::redraw`]); the interface and, inside it, the scene are
//! drawn, which adds the menu's entries and finds the tile under a pending
//! walk ([`Walk::drawn`]); the menu or the hover text; the cursor; the walk
//! states 1 become 2.
use crate::input::mouse::Mouse;

/// `client.gameState` as far as the cycle depends on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    /// 10: the title screen.
    Title,
    /// 25: the game's scene is being built.
    Loading,
    /// 28: the title screen's scene is being built.
    TitleLoading,
    /// 30: in game.
    InGame,
    /// 40: the connection is lost and a reconnect is under way.
    ConnectionLost,
}

/// A part of one cycle; see the module documentation for the lines each
/// stands for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Devices,
    Rebuild,
    Login,
    Timers,
    Packets,
    OwnPackets,
    World,
    InventoryDrag,
    Interfaces,
    Hooks,
    Drag,
    Walk,
    MenuClick,
    Camera,
    Write,
}

/// What runs the steps.
pub trait Host {
    /// `client.gameState` now; a step may have changed it.
    fn state(&self) -> State;
    fn step(&mut self, step: Step);
}

/// One cycle of `client.mainLoop`.
pub fn cycle(host: &mut impl Host) {
    host.step(Step::Devices);
    // client.java:1725-1727
    if matches!(host.state(), State::Loading | State::TitleLoading) {
        host.step(Step::Rebuild);
    }
    // client.java:1728-1744, on the state the rebuild may have left.
    match host.state() {
        State::Title => {
            // client.mainUpdate
            host.step(Step::Interfaces);
            host.step(Step::World);
            host.step(Step::Camera);
            host.step(Step::Hooks);
            host.step(Step::Drag);
            host.step(Step::Login);
        }
        State::InGame => protocol_loop(host),
        State::ConnectionLost => host.step(Step::Login),
        State::Loading | State::TitleLoading => {}
    }
}

/// Whether a cycle that starts in `state` reads the devices
/// (`Keyboard.loop`, `Mouse.loop` in [`Step::Devices`]). With the map files
/// at hand the original builds the title's scene in one blocking
/// `LoginManager.rebuildMap` call of the cycle that finds game state 28
/// (client.java:1725-1727), so whatever is clicked or typed during the build
/// is taken by the cycle after it. This client builds that scene on the
/// streaming thread while cycles go on; reading the devices in them would
/// spend a click (`Mouse.clickButton` lasts one cycle) on cycles that run no
/// interface walk. The first cycle of game state 10 reads them instead.
pub fn reads_devices(state: State) -> bool {
    state != State::TitleLoading
}

/// `Protocol.loop`.
fn protocol_loop(host: &mut impl Host) {
    host.step(Step::Timers);
    host.step(Step::Packets);
    // Protocol.java:2414
    if host.state() != State::InGame {
        return;
    }
    host.step(Step::OwnPackets);
    host.step(Step::World);
    // Protocol.java:2567 and 2574-2577: a plane change in the software
    // renderer, or the read time-out.
    if host.state() != State::InGame {
        return;
    }
    host.step(Step::InventoryDrag);
    host.step(Step::Interfaces);
    host.step(Step::Hooks);
    host.step(Step::Drag);
    host.step(Step::Walk);
    host.step(Step::MenuClick);
    host.step(Step::Camera);
    host.step(Step::Write);
}

/// `MiniMenu.clickProcessingState` and `ScriptRunner.interfaceMouseX/Y`.
///
/// The mini menu is built by a redraw for one point of the canvas. A click
/// that a cycle latches somewhere else is not acted on at once: the point is
/// remembered (`Mouse.lastHandledClickX/Y`), the next redraw builds the menu
/// there, and the first cycle after that redraw acts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MenuClick {
    /// 0; 1 while a menu waits to open; 2 while a click waits to act.
    pub state: i32,
    /// Where the last redraw built the menu.
    pub interface_mouse: [i32; 2],
}

/// What `Protocol.processDefaultAction` does with a closed menu.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Decision {
    /// `ScriptRunner.layoutMiniMenu` opened the menu, at
    /// [`MenuClick::interface_mouse`].
    pub open: bool,
    /// `MiniMenu.processClick` ran the menu's top entry.
    pub act: bool,
}

impl MenuClick {
    /// `LoginManager.processInterface` with the menu closed
    /// (LoginManager.java:1403-1413): the point this redraw builds the menu
    /// for. The caller does not call it while the menu is open, when the
    /// original leaves the point and the menu alone.
    pub fn redraw(&mut self, mouse: &Mouse) -> [i32; 2] {
        self.interface_mouse = if self.state != 0 {
            [mouse.last_handled_click_x, mouse.last_handled_click_y]
        } else if mouse.click_button == 0 {
            [mouse.last_mouse_x, mouse.last_mouse_y]
        } else {
            [mouse.click_x, mouse.click_y]
        };
        self.interface_mouse
    }

    /// The end of `ScriptRunner.layoutMiniMenu` (ScriptRunner.java:1496-
    /// 1516): whether the menu opens now.
    pub fn layout(&mut self, mouse: &mut Mouse) -> bool {
        if self.state == 1 {
            if self.interface_mouse == [mouse.last_handled_click_x, mouse.last_handled_click_y] {
                self.state = 0;
                return true;
            }
            false
        } else if self.interface_mouse == [mouse.click_x, mouse.click_y] {
            self.state = 0;
            true
        } else {
            mouse.last_handled_click_y = mouse.click_y;
            mouse.last_handled_click_x = mouse.click_x;
            self.state = 1;
            false
        }
    }

    /// `MiniMenu.processClick` (MiniMenu.java:1669-1691): whether the top
    /// entry acts now.
    pub fn process(&mut self, mouse: &mut Mouse) -> bool {
        if self.state == 2 {
            if self.interface_mouse == [mouse.last_handled_click_x, mouse.last_handled_click_y] {
                self.state = 0;
                return true;
            }
            false
        } else if self.interface_mouse == [mouse.click_x, mouse.click_y] {
            self.state = 0;
            true
        } else {
            mouse.last_handled_click_y = mouse.click_y;
            self.state = 2;
            mouse.last_handled_click_x = mouse.click_x;
            false
        }
    }

    /// `Protocol.processDefaultAction` for a closed menu, after its test
    /// for a press that starts an inventory drag (Protocol.java:3564-3573).
    ///
    /// `button` is `Mouse.clickButton`; `size` is `MiniMenu.size`;
    /// `menu_instead` is `VarpDomain.mouseButtons == 1 && MiniMenu.size > 2
    /// || MiniMenu.isComponentOptionAction(MiniMenu.size - 1)`, which makes
    /// a left click open the menu.
    pub fn default_action(
        &mut self,
        mouse: &mut Mouse,
        button: i32,
        size: usize,
        menu_instead: bool,
    ) -> Decision {
        let mut button = button;
        if button == 1 && menu_instead {
            button = 2;
        }
        let mut decision = Decision::default();
        if button == 2 && size > 0 || self.state == 1 {
            decision.open = self.layout(mouse);
        }
        if button == 1 && size > 0 || self.state == 2 {
            decision.act = self.process(mouse);
        }
        decision
    }
}

/// What [`Walk::cycle`] found to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arrived {
    /// Nothing waits, or the redraw found no tile.
    Nothing,
    /// `Cheat.teleport` to the tile (staff holding Ctrl and Shift).
    Teleport([i32; 2]),
    /// The spell that was cast on the world goes to the tile (opcode 131).
    Spell([i32; 2]),
    /// Opcode 179 for the tile; only `client.game == 1` has the menu entry.
    Face([i32; 2]),
    /// `PathFinder.findPath` to the tile, and the walk packet.
    Walk([i32; 2]),
}

/// The tile a click on the world waits for.
///
/// "Walk here" in the scene (`MiniMenu.doAction`, action 60 with key 0) does
/// not walk: it remembers the click's point (`setWalkDestination`). The next
/// scene draw finds the tile drawn there (`MiniMenu.clickTileX/Y`), and the
/// first cycle after it walks. A spell cast on the world (action 11, key 0)
/// waits the same way.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Walk {
    /// `MiniMenu.walkPending` with `walkDestTileX/Y`, a point of the canvas.
    pub pending: Option<[i32; 2]>,
    /// `MiniMenu.clickTileX/Y`; `None` is -1.
    pub click_tile: Option<[i32; 2]>,
    /// `MiniMenu.minimapWalkState`: 1 from the spell's action, 2 from the
    /// redraw after it.
    pub minimap_state: i32,
    /// `Protocol.viewportWalkState`, the same for action 36.
    pub viewport_state: i32,
}

impl Walk {
    /// `MiniMenu.setWalkDestination` (MiniMenu.java:1582-1589).
    pub fn set_destination(&mut self, point: [i32; 2]) {
        self.pending = Some(point);
        self.click_tile = None;
    }

    /// The scene was drawn: `tile_at` is the tile drawn under a point of the
    /// canvas, if any (SceneGraph.java:3923, 3949, 4282), and the wait ends
    /// either way (SceneGraph.java:3144, 3191, 3199).
    pub fn drawn(&mut self, tile_at: impl FnOnce([i32; 2]) -> Option<[i32; 2]>) {
        if let Some(point) = self.pending.take()
            && let Some(tile) = tile_at(point)
        {
            self.click_tile = Some(tile);
        }
    }

    /// The end of `LoginManager.processInterface` (LoginManager.java:1468-
    /// 1473).
    pub fn redrawn(&mut self) {
        if self.minimap_state == 1 {
            self.minimap_state = 2;
        }
        if self.viewport_state == 1 {
            self.viewport_state = 2;
        }
    }

    /// Protocol.java:2828-2867. `staff` is `LoginManager.staffModLevel > 0`
    /// with Ctrl and Shift held.
    pub fn cycle(&mut self, staff: bool) -> Arrived {
        let tile = self.click_tile.take();
        if staff {
            self.viewport_state = 0;
            self.minimap_state = 0;
            tile.map_or(Arrived::Nothing, Arrived::Teleport)
        } else if self.minimap_state == 2 {
            self.minimap_state = 0;
            tile.map_or(Arrived::Nothing, Arrived::Spell)
        } else if self.viewport_state == 2 {
            self.viewport_state = 0;
            tile.map_or(Arrived::Nothing, Arrived::Face)
        } else if self.minimap_state == 0 && self.viewport_state == 0 {
            tile.map_or(Arrived::Nothing, Arrived::Walk)
        } else {
            Arrived::Nothing
        }
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests: the expected orders and values were worked out
    //! by hand from the Java lines named; no original class runs here.
    use super::*;

    /// A host that records the steps and changes state where told to.
    struct Script {
        state: State,
        ran: Vec<Step>,
        /// After this step, the state becomes this.
        change: Vec<(Step, State)>,
    }
    impl Host for Script {
        fn state(&self) -> State {
            self.state
        }
        fn step(&mut self, step: Step) {
            self.ran.push(step);
            if let Some(at) = self.change.iter().position(|(s, _)| *s == step) {
                self.state = self.change.remove(at).1;
            }
        }
    }
    fn run(state: State, change: &[(Step, State)]) -> Vec<Step> {
        let mut script = Script {
            state,
            ran: Vec::new(),
            change: change.to_vec(),
        };
        cycle(&mut script);
        script.ran
    }

    use Step::{
        Camera, Devices, Drag, Hooks, Interfaces, InventoryDrag, Login, OwnPackets, Packets,
        Rebuild, Timers, World, Write,
    };
    const PROTOCOL_LOOP: [Step; 12] = [
        Timers,
        Packets,
        OwnPackets,
        World,
        InventoryDrag,
        Interfaces,
        Hooks,
        Drag,
        Step::Walk,
        Step::MenuClick,
        Camera,
        Write,
    ];

    #[test]
    fn a_cycle_in_game_is_protocol_loop_in_its_order() {
        // client.java:1710-1711, then Protocol.java:2398-2992: packets are
        // read before anything is written, players and NPCs move before the
        // interfaces run, the menu click follows the hooks, the camera
        // follows the click, and the write is last.
        let mut expected = vec![Devices];
        expected.extend(PROTOCOL_LOOP);
        assert_eq!(run(State::InGame, &[]), expected);
    }

    #[test]
    fn a_rebuild_packet_ends_the_cycle_that_reads_it() {
        // Protocol.java:2412-2416: the packet loop, then `return` unless the
        // state is still 30. Nothing more runs and nothing is written.
        assert_eq!(
            run(State::InGame, &[(Packets, State::Loading)]),
            [Devices, Timers, Packets]
        );
        // A LOGOUT packet (state 10) and a lost connection (40) alike.
        assert_eq!(
            run(State::InGame, &[(Packets, State::Title)]),
            [Devices, Timers, Packets]
        );
        assert_eq!(
            run(State::InGame, &[(Packets, State::ConnectionLost)]),
            [Devices, Timers, Packets]
        );
    }

    #[test]
    fn game_state_25_runs_the_rebuild_and_nothing_else() {
        // client.java:1725-1727: neither `mainUpdate` nor `Protocol.loop`.
        assert_eq!(run(State::Loading, &[]), [Devices, Rebuild]);
        assert_eq!(run(State::TitleLoading, &[]), [Devices, Rebuild]);
    }

    #[test]
    fn the_cycle_that_ends_a_rebuild_goes_on_in_the_new_state() {
        // client.java:1725-1734: `rebuildMap` ends with `setGameState(30)`
        // and the `else if (gameState == 30)` below it then runs
        // `Protocol.loop` in the same cycle.
        let mut expected = vec![Devices, Rebuild];
        expected.extend(PROTOCOL_LOOP);
        assert_eq!(run(State::Loading, &[(Rebuild, State::InGame)]), expected);
        // From 28 it ends in 10, and `mainUpdate` runs.
        assert_eq!(
            run(State::TitleLoading, &[(Rebuild, State::Title)]),
            [
                Devices, Rebuild, Interfaces, World, Camera, Hooks, Drag, Login
            ]
        );
    }

    #[test]
    fn the_title_screen_runs_main_update_and_the_login() {
        // client.mainUpdate (client.java:1155-1269): the walk comes before
        // the NPCs and the camera, the hooks after them; there are no
        // packets, timers or menu clicks. Then the login (1730-1732).
        assert_eq!(
            run(State::Title, &[]),
            [Devices, Interfaces, World, Camera, Hooks, Drag, Login]
        );
    }

    #[test]
    fn the_title_scene_build_leaves_the_devices_to_the_cycle_after_it() {
        // client.java:1725-1732 with the map files at hand: game state 28 is
        // one cycle whose rebuild blocks, so input that arrives meanwhile is
        // read by the next cycle, a cycle of game state 10.
        assert!(!reads_devices(State::TitleLoading));
        // Every other state reads them each cycle (client.java:1697-1718),
        // the game's own rebuild (25) included: it waits for map files
        // cycle by cycle in the original too.
        for state in [
            State::Title,
            State::Loading,
            State::InGame,
            State::ConnectionLost,
        ] {
            assert!(reads_devices(state), "{state:?}");
        }
    }

    #[test]
    fn a_lost_connection_runs_only_the_login() {
        // client.java:1735-1744.
        assert_eq!(run(State::ConnectionLost, &[]), [Devices, Login]);
    }

    #[test]
    fn a_time_out_ends_the_cycle_before_the_interfaces() {
        // Protocol.java:2573-2577: handleConnectionLost(), return.
        assert_eq!(
            run(State::InGame, &[(World, State::ConnectionLost)]),
            [Devices, Timers, Packets, OwnPackets, World]
        );
    }

    fn mouse(click: Option<[i32; 2]>, at: [i32; 2]) -> Mouse {
        let mut mouse = Mouse::default();
        mouse.click_button = i32::from(click.is_some());
        [mouse.click_x, mouse.click_y] = click.unwrap_or([0; 2]);
        [mouse.last_mouse_x, mouse.last_mouse_y] = at;
        mouse
    }

    #[test]
    fn a_click_where_the_menu_was_built_acts_in_its_own_cycle() {
        // Redraw N builds the menu at the pointer (LoginManager.java:1407-
        // 1409). The click latched by the next cycle is at the same point:
        // MiniMenu.java:1679-1685 acts at once.
        let mut click = MenuClick::default();
        let mut m = mouse(None, [300, 200]);
        assert_eq!(click.redraw(&m), [300, 200]);
        m.click_button = 1;
        (m.click_x, m.click_y) = (300, 200);
        assert_eq!(
            click.default_action(&mut m, 1, 2, false),
            Decision {
                open: false,
                act: true
            }
        );
        assert_eq!(click.state, 0);
    }

    #[test]
    fn a_click_elsewhere_waits_for_the_redraw_that_builds_its_menu() {
        // The pointer moved between the redraw and the click: the menu in
        // hand was built for another point. MiniMenu.java:1686-1690
        // remembers the click; LoginManager.java:1404-1406 builds the next
        // menu there; MiniMenu.java:1670-1678 then acts.
        let mut click = MenuClick::default();
        let mut m = mouse(None, [300, 200]);
        click.redraw(&m);
        m = mouse(Some([340, 210]), [340, 210]);
        assert_eq!(
            click.default_action(&mut m, 1, 2, false),
            Decision::default()
        );
        assert_eq!(click.state, 2);
        assert_eq!([m.last_handled_click_x, m.last_handled_click_y], [340, 210]);
        // Cycles without a redraw between them (a slow frame owing several):
        // the click keeps waiting, with no button down any more.
        m.click_button = 0;
        assert_eq!(
            click.default_action(&mut m, 0, 2, false),
            Decision::default()
        );
        assert_eq!(click.state, 2);
        // The redraw builds the menu at the remembered click, wherever the
        // pointer has gone meanwhile.
        m.last_mouse_x = 500;
        assert_eq!(click.redraw(&m), [340, 210]);
        assert_eq!(
            click.default_action(&mut m, 0, 3, false),
            Decision {
                open: false,
                act: true
            }
        );
        assert_eq!(click.state, 0);
    }

    #[test]
    fn a_second_click_while_one_waits_is_judged_by_the_first() {
        // MiniMenu.java:1670-1678: in state 2 only lastHandledClick counts;
        // a new click before the redraw is dropped, and one after it acts on
        // the menu built for the first.
        let mut click = MenuClick::default();
        let mut m = mouse(None, [10, 10]);
        click.redraw(&m);
        m = mouse(Some([50, 50]), [50, 50]);
        click.default_action(&mut m, 1, 2, false);
        (m.click_x, m.click_y) = (90, 90);
        assert_eq!(
            click.default_action(&mut m, 1, 2, false),
            Decision::default()
        );
        assert_eq!([m.last_handled_click_x, m.last_handled_click_y], [50, 50]);
        assert_eq!(click.redraw(&m), [50, 50]);
        assert!(click.default_action(&mut m, 1, 2, false).act);
    }

    #[test]
    fn a_right_click_opens_the_menu_by_the_same_rule() {
        // ScriptRunner.java:1496-1516.
        let mut click = MenuClick::default();
        let mut m = mouse(None, [300, 200]);
        click.redraw(&m);
        m = mouse(Some([300, 200]), [300, 200]);
        m.click_button = 2;
        assert_eq!(
            click.default_action(&mut m, 2, 2, false),
            Decision {
                open: true,
                act: false
            }
        );
        // Elsewhere: state 1, then the redraw, then it opens with no button.
        m = mouse(Some([10, 20]), [10, 20]);
        m.click_button = 2;
        assert_eq!(
            click.default_action(&mut m, 2, 2, false),
            Decision::default()
        );
        assert_eq!(click.state, 1);
        assert_eq!(click.redraw(&m), [10, 20]);
        assert!(click.default_action(&mut m, 0, 2, false).open);
        assert_eq!(click.state, 0);
    }

    #[test]
    fn a_left_click_opens_the_menu_when_the_top_entry_asks_for_it() {
        // Protocol.java:3564-3566: one-button mouse with more than two
        // entries, or a component option on top.
        let mut click = MenuClick::default();
        let mut m = mouse(None, [5, 5]);
        click.redraw(&m);
        m = mouse(Some([5, 5]), [5, 5]);
        assert_eq!(
            click.default_action(&mut m, 1, 3, true),
            Decision {
                open: true,
                act: false
            }
        );
    }

    #[test]
    fn the_redraw_uses_the_click_point_in_the_cycle_that_latched_it() {
        // LoginManager.java:1410-1412: with Mouse.clickButton still set the
        // menu is built at the click, not at the pointer.
        let mut click = MenuClick::default();
        let m = mouse(Some([7, 8]), [70, 80]);
        assert_eq!(click.redraw(&m), [7, 8]);
    }

    #[test]
    fn walk_here_walks_in_the_cycle_after_the_redraw_that_finds_the_tile() {
        let mut walk = Walk::default();
        // Cycle N: the action (MiniMenu.java:607-612) only sets the point.
        walk.set_destination([400, 300]);
        // A cycle before any redraw: Protocol.java:2858 finds clickTileX -1.
        assert_eq!(walk.cycle(false), Arrived::Nothing);
        assert_eq!(walk.pending, Some([400, 300]));
        // The redraw finds the tile under the point and ends the wait.
        walk.drawn(|point| {
            assert_eq!(point, [400, 300]);
            Some([3222, 3218])
        });
        walk.redrawn();
        assert_eq!(walk.pending, None);
        // Cycle N+1 walks, once.
        assert_eq!(walk.cycle(false), Arrived::Walk([3222, 3218]));
        assert_eq!(walk.cycle(false), Arrived::Nothing);
    }

    #[test]
    fn a_click_on_no_tile_walks_nowhere() {
        let mut walk = Walk::default();
        walk.set_destination([1, 1]);
        walk.drawn(|_| None);
        assert_eq!(walk.pending, None);
        assert_eq!(walk.cycle(false), Arrived::Nothing);
    }

    #[test]
    fn a_spell_on_the_world_waits_for_its_tile_and_its_state_2() {
        // MiniMenu.java:923-926 sets minimapWalkState 1; the redraw makes
        // it 2 (LoginManager.java:1468-1470); Protocol.java:2834-2846 sends.
        let mut walk = Walk {
            minimap_state: 1,
            ..Walk::default()
        };
        walk.set_destination([9, 9]);
        // Two cycles in one slow frame: nothing yet, and no plain walk.
        assert_eq!(walk.cycle(false), Arrived::Nothing);
        walk.drawn(|_| Some([3200, 3200]));
        walk.redrawn();
        assert_eq!(walk.cycle(false), Arrived::Spell([3200, 3200]));
        assert_eq!(walk.minimap_state, 0);
    }

    #[test]
    fn staff_holding_ctrl_and_shift_teleport_instead() {
        // Protocol.java:2828-2833.
        let mut walk = Walk {
            minimap_state: 2,
            click_tile: Some([3100, 3100]),
            ..Walk::default()
        };
        assert_eq!(walk.cycle(true), Arrived::Teleport([3100, 3100]));
        assert_eq!((walk.minimap_state, walk.viewport_state), (0, 0));
    }
}
