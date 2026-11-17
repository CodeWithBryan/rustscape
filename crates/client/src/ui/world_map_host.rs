//! The world map as the interface layer drives it: who owns
//! [`crate::world_map::WorldMap`], and the callers the Java spreads over
//! `client`, `Protocol`, `InterfaceList`, `Cs1ScriptRunner` and `LoginManager`.
//!
//! [`Host`] lives in [`State`] so the three users reach it: the script VM
//! (commands 5200..=5220, `vm_world_map.rs`), the interface pass
//! (`hook_walk.rs`) and the renderer (client codes 1400 and
//! 1401). Each method names the Java lines it transcribes.
//!
//! A new `Host` holds the Java statics as they are before `MapList.init`: no
//! maps, nothing selected. Every command then answers as the original does
//! with nothing loaded and the component shows its loading box at 0%. The
//! browser build stays in that state because it has no cache to read
//! archive 23 from; [`Host::open_cache`] is the native start-up step.
//!
//! Differences from the Java, all stated where they occur: effects on the
//! rest of the client are queued instead of performed (`Host::teleports`,
//! `Host::pings`, [`SceneRequests`]), `Rasteriser.palette` is taken at
//! brightness 0.7 like the client's other software colour tables, and the
//! mouse press position stands in for `Mouse.clickX/Y`.
use super::state::State;
use crate::world_map::{Assets, Client, Config, MapList, Rendered, Typeface, WorldMap};

/// An archive 23 with nothing in it: the state before any cache is attached.
struct NoAssets;
impl Assets for NoAssets {
    fn group_ready(&self, _: &str) -> bool {
        false
    }
    fn group_valid(&self, _: &str) -> bool {
        false
    }
    fn group_percentage(&self, _: &str) -> i32 {
        0
    }
    fn map_file(&self, _: &str, _: &str) -> Option<Vec<u8>> {
        None
    }
    fn group_files(&self, _: &str) -> Option<Vec<Vec<u8>>> {
        None
    }
    fn underlay(&self, _: i32) -> Option<Vec<u8>> {
        None
    }
    fn overlay(&self, _: i32) -> Option<Vec<u8>> {
        None
    }
    fn overlay_capacity(&self) -> i32 {
        0
    }
    fn map_scene(&self, _: i32) -> Option<Vec<u8>> {
        None
    }
    fn location(&self, _: i32) -> Option<Vec<u8>> {
        None
    }
    fn sprite(&self, _: i32) -> Option<Vec<u8>> {
        None
    }
    fn texture_repeating(&self, _: i32) -> bool {
        false
    }
    fn texture_average_color(&self, _: i32) -> i32 {
        0
    }
    fn varbit(&self, _: i32) -> i32 {
        0
    }
    fn varp(&self, _: i32) -> i32 {
        0
    }
}

/// The attached assets with the player's current variables, which
/// `VarpDomain` supplies in the Java: `(is_varbit, id) -> value`.
struct Live<'a> {
    inner: &'a dyn Assets,
    variables: &'a dyn Fn(bool, i32) -> i32,
}
impl Assets for Live<'_> {
    fn group_ready(&self, group: &str) -> bool {
        self.inner.group_ready(group)
    }
    fn group_valid(&self, group: &str) -> bool {
        self.inner.group_valid(group)
    }
    fn group_percentage(&self, group: &str) -> i32 {
        self.inner.group_percentage(group)
    }
    fn map_file(&self, group: &str, file: &str) -> Option<Vec<u8>> {
        self.inner.map_file(group, file)
    }
    fn group_files(&self, group: &str) -> Option<Vec<Vec<u8>>> {
        self.inner.group_files(group)
    }
    fn underlay(&self, id: i32) -> Option<Vec<u8>> {
        self.inner.underlay(id)
    }
    fn overlay(&self, id: i32) -> Option<Vec<u8>> {
        self.inner.overlay(id)
    }
    fn overlay_capacity(&self) -> i32 {
        self.inner.overlay_capacity()
    }
    fn map_scene(&self, id: i32) -> Option<Vec<u8>> {
        self.inner.map_scene(id)
    }
    fn location(&self, id: i32) -> Option<Vec<u8>> {
        self.inner.location(id)
    }
    fn sprite(&self, id: i32) -> Option<Vec<u8>> {
        self.inner.sprite(id)
    }
    fn texture_repeating(&self, id: i32) -> bool {
        self.inner.texture_repeating(id)
    }
    fn texture_average_color(&self, id: i32) -> i32 {
        self.inner.texture_average_color(id)
    }
    fn varbit(&self, id: i32) -> i32 {
        (self.variables)(true, id)
    }
    fn varp(&self, id: i32) -> i32 {
        (self.variables)(false, id)
    }
}

/// A software frame for the presenter: Java ints, of which the low 24 bits
/// are the colour. It is drawn opaque.
#[derive(Clone, Debug, Default)]
pub struct Surface {
    pub pixels: Vec<u32>,
    /// Width and height; `pixels` is row-major.
    pub size: [i32; 2],
}
impl Surface {
    /// A zeroed buffer when the size changes, as the Java allocates
    /// `renderBuffer` and `overviewSprite`. Returns whether it was replaced.
    fn fit(&mut self, width: i32, height: i32) -> bool {
        let area = width.max(0) as usize * height.max(0) as usize;
        if self.size == [width, height] && self.pixels.len() == area {
            return false;
        }
        self.pixels = vec![0; area];
        self.size = [width, height];
        true
    }
}

/// What the map asked of the scene and has not been carried out yet. The
/// client takes these with [`Host::take_scene_requests`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneRequests {
    /// `client.clearScene()` from `WorldMap.load`: drop the scene graph and
    /// the collision maps while the map is open.
    pub clear_scene: u32,
    /// `client.setGameState(25)` from `WorldMap.reset`: rebuild the scene the
    /// map cleared, then send MAP_REBUILD_COMPLETE (opcode 110) and call
    /// [`Host::scene_rebuilt`], as `LoginManager` does on reaching state 30.
    pub rebuild_scene: u32,
    /// `GameShell.resetTimer()` after a slow load step.
    pub reset_timer: u32,
}

/// What start-up hands over besides the cache (see [`Host::open_cache`]).
pub struct Startup {
    /// `Sprites.load`'s red, green and blue offsets for the map-function
    /// sprites and `MsiTypeList`'s for map scenes, when the minimap already
    /// drew them; None draws both here in the original's order.
    pub jitters: Option<([i32; 3], [i32; 3])>,
    /// Seeds this host's `Math.random`.
    pub seed: u64,
    /// The label face standing in for AWT's "Helvetica"; None draws no labels.
    pub typeface: Option<Typeface>,
}

pub struct Host {
    /// The `WorldMap` and `MapList` statics.
    pub map: WorldMap,
    assets: Box<dyn Assets>,
    /// `InterfaceList.worldMapDragState`: 0 idle, 1 pressed, 2 dragging.
    pub drag_state: i32,
    /// `Cs1ScriptRunner.dragStartMouseX/Y` as the press on the map left them.
    pub drag_start_mouse: [i32; 2],
    /// `InterfaceList.worldMapDragStartX/Y`: the view when the mouse first moved.
    pub drag_start_view: [i32; 2],
    /// `Keyboard.pressedKeys[KEY_CTRL]`, kept current by the client.
    pub ctrl_held: bool,
    /// `LoginManager.staffModLevel`, kept current by the client.
    pub staff_level: i32,
    /// `SceneGraph.hueJitter` and `lightnessJitter`, kept current by the client.
    pub scene_jitter: [i32; 2],
    /// `client.game == 1`: the overview frame is white instead of red.
    pub white_frame: bool,
    /// Absolute tiles of staff Ctrl-clicks on the map. For each the Java runs
    /// `Cheat.teleport(x, y, 0)` and `ClientProt.closeWidget()`;
    /// `Vm::world_map_requests` does that.
    pub teleports: Vec<[i32; 2]>,
    /// `ClientProt.ping(true)` calls of the load steps, not yet sent.
    pub pings: u32,
    scene: SceneRequests,
    random_state: u64,
    /// `renderBuffer`: the last [`Host::render_map`] frame.
    pub map_surface: Surface,
    /// The last [`Host::render_overview`] frame.
    pub overview_surface: Surface,
}

impl Default for Host {
    fn default() -> Self {
        let config = Config {
            // Never indexed: nothing can be selected from an empty list.
            palette: vec![0; 65536],
            random_light_offset: 0,
            random_hue_offset: 0,
            map_scene_jitter: [0; 3],
            typeface: None,
        };
        Self {
            map: WorldMap::new(MapList::default(), config).expect("empty world map"),
            assets: Box::new(NoAssets),
            drag_state: 0,
            drag_start_mouse: [0; 2],
            drag_start_view: [0; 2],
            ctrl_held: false,
            staff_level: 0,
            scene_jitter: [0; 2],
            white_frame: false,
            teleports: Vec::new(),
            pings: 0,
            scene: SceneRequests::default(),
            random_state: 0,
            map_surface: Surface::default(),
            overview_surface: Surface::default(),
        }
    }
}

impl Host {
    /// `Math.random()`: the fixed-algorithm generator the script VM uses, so
    /// captures repeat.
    fn random(state: &mut u64) -> f64 {
        let mut x = if *state == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            *state
        };
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        *state = x;
        (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `MapList.init` has run: these are the maps, with what reads them.
    /// The Java does this once, in main load state 130.
    pub fn install(
        &mut self,
        maps: MapList,
        config: Config,
        assets: Box<dyn Assets>,
    ) -> Result<(), String> {
        self.map = WorldMap::new(maps, config)?;
        self.assets = assets;
        self.drag_state = 0;
        self.map_surface = Surface::default();
        self.overview_surface = Surface::default();
        Ok(())
    }

    /// Whether [`Host::install`] supplied any map.
    pub fn has_maps(&self) -> bool {
        !self.map.maps.maps.is_empty()
    }

    /// Start-up over a JS5 cache directory (`client` main load state 130):
    /// `MapList.init(Sprites.mapfunctions, js5Archive23)`, plus the statics
    /// the map reads from other classes.
    ///
    /// - `Rasteriser.palette` is built at brightness 0.7. The Java uses 0.7
    ///   only in HD with high-detail lighting, otherwise 0.9 to 0.6 by the
    ///   brightness preference, and always adds up to 0.015 at random; this
    ///   client's software tables are all the plain 0.7 one.
    /// - `WorldMap.randomLightOffset` and `randomHueOffset` are drawn here as
    ///   `(int) (Math.random() * 33.0) - 16` and `(int) (Math.random() * 17.0) - 8`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_cache(
        &mut self,
        directory: &std::path::Path,
        startup: Startup,
    ) -> Result<(), String> {
        use crate::world_map::cache::CacheAssets;
        let mut state = startup.seed;
        let mut draw = || Self::random(&mut state);
        let (functions, scenes) = startup.jitters.unwrap_or_else(|| {
            // Sprites.load, then MsiTypeList.init: four draws each.
            let a = (draw() * 21.0) as i32 - 10;
            let b = (draw() * 21.0) as i32 - 10;
            let c = (draw() * 21.0) as i32 - 10;
            let base = (draw() * 41.0) as i32 - 20;
            let functions = [b + base, a + base, c + base];
            let scenes = crate::minimap::jitter(std::array::from_fn(|_| draw()));
            (functions, scenes)
        });
        let random_light_offset = (draw() * 33.0) as i32 - 16;
        let random_hue_offset = (draw() * 17.0) as i32 - 8;
        let assets = CacheAssets::open_on_demand(directory)?;
        let sprite = assets
            .map_function_sprite()
            .ok_or("world map: the cache has no mapfunction sprites")?;
        let maps = MapList::init(&assets, sprite, functions)?;
        self.install(
            maps,
            Config {
                palette: crate::surface::palette(0.7),
                random_light_offset,
                random_hue_offset,
                map_scene_jitter: scenes,
                typeface: startup.typeface,
            },
            Box::new(assets),
        )?;
        self.random_state = state;
        Ok(())
    }

    /// `Protocol` 2581 and 2792, before the interface pass of a logic cycle:
    /// ease zoom and pan if the previous cycle met the map component, then
    /// forget that component.
    pub fn begin_cycle(&mut self) {
        if self.map.component.is_some() {
            self.map.update_zoom_and_scroll();
        }
        self.map.component = None;
    }

    /// `Protocol` 2812, once the cycle's hook requests have run: a map
    /// component that was not met (or that a script's `clear` dropped) ends
    /// the drag.
    pub fn end_cycle(&mut self) {
        if self.map.component.is_none() {
            self.drag_state = 0;
        }
    }

    /// `InterfaceList` 731: while the map is pressed or dragged, every other
    /// component sees no click, press or hover.
    pub fn dragging(&self) -> bool {
        self.drag_state > 0
    }

    /// `InterfaceList` 677: a click-through-blocking layer under the mouse
    /// ends the drag when the pass reaches it.
    pub fn blocked_by_layer(&mut self) {
        self.drag_state = 0;
    }

    /// `InterfaceList` 750-783 for the client code 1400 component of `size`
    /// at `origin`. `clicked` and `pressed` are the Java's `isClicked` and
    /// `isPressed` after line 731; `mouse` is `Mouse.lastMouseX/Y`.
    pub fn map_input(
        &mut self,
        origin: [i32; 2],
        size: [i32; 2],
        mouse: [i32; 2],
        clicked: bool,
        pressed: bool,
    ) {
        self.map.component = Some(size);
        if clicked {
            if self.ctrl_held && self.staff_level > 0 {
                let point = [
                    mouse[0].wrapping_sub(origin[0]),
                    mouse[1].wrapping_sub(origin[1]),
                ];
                self.teleports.push(self.map.tile_at(size, point));
                return;
            }
            self.drag_state = 1;
            self.drag_start_mouse = mouse;
            return;
        }
        if pressed && self.drag_state > 0 {
            if self.drag_state == 1 && self.drag_start_mouse != mouse {
                self.drag_start_view = [self.map.view_x, self.map.view_y];
                self.drag_state = 2;
            }
            if self.drag_state == 2 {
                self.map
                    .drag(self.drag_start_view, self.drag_start_mouse, mouse);
            }
            return;
        }
        self.drag_state = 0;
    }

    /// `InterfaceList` 784-789 for the client code 1401 component: a press
    /// recentres the view under the mouse.
    pub fn overview_input(
        &mut self,
        origin: [i32; 2],
        size: [i32; 2],
        mouse: [i32; 2],
        pressed: bool,
    ) {
        if pressed {
            self.map.set_view_from_mouse_position(
                size[0],
                mouse[1].wrapping_sub(origin[1]),
                mouse[0].wrapping_sub(origin[0]),
                size[1],
            );
        }
    }

    /// `Protocol` 1284 (IF_OPENSUB, the top-level open): `reset == 2` runs
    /// `WorldMap.reset()`, which is `clear(false)` then
    /// `client.setGameState(25)`. The state change is left as a request; its
    /// own `clear(true)` finds nothing selected.
    pub fn top_level_opened(&mut self, reset: i32) {
        if reset == 2 {
            self.map.reset();
            self.scene.rebuild_scene += 1;
        }
    }

    /// `client.setGameState(25 | 28)`: a scene load begins, so the map is
    /// dropped and its name kept for [`Host::scene_rebuilt`].
    pub fn scene_loading(&mut self) {
        self.map.clear(true);
    }

    /// `LoginManager` 1132, once a rebuilt scene is ready:
    /// `WorldMap.restorePreviousMap()`.
    pub fn scene_rebuilt(&mut self) {
        self.map.restore_previous_map();
    }

    /// `LoginManager` 660 (logout) and `client.unload`: `WorldMap.clear(false)`.
    pub fn logged_out(&mut self) {
        self.map.clear(false);
    }

    /// The scene effects asked for since the last call.
    pub fn take_scene_requests(&mut self) -> SceneRequests {
        std::mem::take(&mut self.scene)
    }

    /// The scene effects asked for and not yet taken.
    pub fn scene_requests(&self) -> SceneRequests {
        self.scene
    }

    /// `Cs1ScriptRunner` 456: `WorldMap.render` for a client code 1400
    /// component of `width` by `height`, into [`Host::map_surface`]. This is
    /// the only place a load step runs: one per drawn frame.
    ///
    /// `player` is the local player's absolute tile and `variables` reads a
    /// player variable, `(is_varbit, id) -> value`.
    pub fn render_map(
        &mut self,
        player: [i32; 2],
        variables: &dyn Fn(bool, i32) -> i32,
        width: i32,
        height: i32,
    ) -> Result<Rendered, String> {
        // `clear` drops `renderBuffer`, so the first frame of a map starts
        // from a zeroed buffer; so does every frame of the loading box.
        if !self.map_surface.fit(width, height) && self.map.load_percentage < 100 {
            self.map_surface.pixels.fill(0);
        }
        let assets = Live {
            inner: self.assets.as_ref(),
            variables,
        };
        let state = &mut self.random_state;
        let mut random = || Self::random(state);
        let mut client = Client {
            player,
            random: &mut random,
            jitter: self.scene_jitter,
        };
        let rendered = self.map.render(
            &assets,
            &mut client,
            &mut self.map_surface.pixels,
            width,
            height,
        )?;
        self.scene.clear_scene += u32::from(rendered.step.clear_scene);
        self.scene.reset_timer += u32::from(rendered.step.reset_timer);
        self.pings += u32::from(rendered.step.ping);
        Ok(rendered)
    }

    /// `Cs1ScriptRunner` 467: `renderWorldMapOverview` for a client code
    /// 1401 component, into [`Host::overview_surface`].
    pub fn render_overview(
        &mut self,
        variables: &dyn Fn(bool, i32) -> i32,
        width: i32,
        height: i32,
    ) -> Result<(), String> {
        self.overview_surface.fit(width, height);
        let assets = Live {
            inner: self.assets.as_ref(),
            variables,
        };
        self.map.render_overview(
            &assets,
            &mut self.overview_surface.pixels,
            width,
            height,
            self.white_frame,
        )
    }
}

/// A player variable as `VarpDomain` answers it; a varbit without a
/// definition reads 0.
fn variable(state: &State, vm: &super::vm::Vm, varbit: bool, id: i32) -> i32 {
    if varbit {
        vm.varbit(state, id).unwrap_or(0)
    } else {
        vm.varps.get(&id).copied().unwrap_or(0)
    }
}

/// Draw-time entry for client code 1400: [`Host::render_map`] with the
/// player's tile (`vm.location`) and variables.
pub fn render_map(
    state: &mut State,
    vm: &super::vm::Vm,
    width: i32,
    height: i32,
) -> Result<Rendered, String> {
    let mut host = std::mem::take(&mut state.world_map);
    let result = host.render_map(
        [vm.location[0], vm.location[1]],
        &|varbit, id| variable(state, vm, varbit, id),
        width,
        height,
    );
    state.world_map = host;
    result
}

/// Draw-time entry for client code 1401: [`Host::render_overview`].
pub fn render_overview(
    state: &mut State,
    vm: &super::vm::Vm,
    width: i32,
    height: i32,
) -> Result<(), String> {
    let mut host = std::mem::take(&mut state.world_map);
    let result = host.render_overview(&|varbit, id| variable(state, vm, varbit, id), width, height);
    state.world_map = host;
    result
}

/// A label face from the system, standing in for the AWT "Helvetica" bold
/// the Java asks for (which Windows maps to Arial): `path` if given, else
/// the first of the usual Arial or metric-compatible bold faces that loads.
/// None when there is none, and then the map has no labels.
#[cfg(not(target_arch = "wasm32"))]
pub fn system_typeface(path: Option<&std::path::Path>) -> Option<Typeface> {
    let mut candidates = Vec::new();
    if let Some(path) = path {
        candidates.push(path.to_path_buf());
    } else {
        if let Some(windows) = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot"))
        {
            candidates.push(
                std::path::Path::new(&windows)
                    .join("Fonts")
                    .join("arialbd.ttf"),
            );
        }
        candidates.extend(
            [
                "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
                "/Library/Fonts/Arial Bold.ttf",
                "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
                "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
                "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            ]
            .map(std::path::PathBuf::from),
        );
    }
    candidates.into_iter().find_map(|path| {
        let data = std::fs::read(path).ok()?;
        Typeface::new(&data, None).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pack::Pack,
        ui::{
            Component, HookArg,
            state::{Key, PointerInput},
        },
        world_map::Map,
    };

    /// A host whose map has the `main` map's geometry without its data:
    /// 1856 by 1664 tiles from (2048, 2496), zoom 6 (75%), view at (1000, 900).
    fn host() -> Host {
        let mut host = Host::default();
        let map = &mut host.map;
        (map.origin_x, map.origin_y, map.width, map.length) = (2048, 2496, 1856, 1664);
        (map.zoom, map.target_zoom) = (6.0, 6.0);
        (map.view_x, map.view_y) = (1000, 900);
        host
    }
    const ORIGIN: [i32; 2] = [10, 20];
    const SIZE: [i32; 2] = [512, 334];

    /// Transcription test of `InterfaceList` 750-783, by hand from the Java.
    #[test]
    fn a_press_arms_the_drag_and_movement_pans_from_where_the_view_was() {
        let mut host = host();
        let view = |host: &Host| [host.map.view_x, host.map.view_y];
        // Pressed without a click (the press began elsewhere): nothing.
        host.map_input(ORIGIN, SIZE, [100, 100], false, true);
        assert_eq!(host.drag_state, 0);
        assert_eq!(host.map.component, Some(SIZE));
        // The click arms the drag and remembers the mouse.
        host.map_input(ORIGIN, SIZE, [100, 100], true, true);
        assert_eq!((host.drag_state, host.drag_start_mouse), (1, [100, 100]));
        // Held still: still armed, the view untouched.
        host.map_input(ORIGIN, SIZE, [100, 100], false, true);
        assert_eq!((host.drag_state, view(&host)), (1, [1000, 900]));
        // First movement: the view is remembered, then moved by
        // (press - mouse) * 2 / targetZoom, truncated: 30 * 2 / 6 = 10.
        host.map.pan_target_x = 5;
        host.map.pan_target_y = 6;
        host.map_input(ORIGIN, SIZE, [70, 130], false, true);
        assert_eq!((host.drag_state, host.drag_start_view), (2, [1000, 900]));
        assert_eq!(view(&host), [1010, 890]);
        // setViewX and setViewY end any pan.
        assert_eq!([host.map.pan_target_x, host.map.pan_target_y], [-1, -1]);
        // Later movement is measured from the press, not from the last cycle.
        host.map_input(ORIGIN, SIZE, [40, 100], false, true);
        assert_eq!(view(&host), [1020, 900]);
        // (int) truncates toward zero in both directions.
        host.map_input(ORIGIN, SIZE, [102, 98], false, true);
        assert_eq!(view(&host), [1000, 900]);
        host.map_input(ORIGIN, SIZE, [103, 97], false, true);
        assert_eq!(view(&host), [999, 901]);
        // The divisor is targetZoom, whatever zoom has eased to.
        host.map.target_zoom = 16.0;
        host.map_input(ORIGIN, SIZE, [84, 100], false, true);
        assert_eq!(view(&host), [1002, 900]);
        // The view is clamped to the map.
        host.map.target_zoom = 3.0;
        host.map_input(ORIGIN, SIZE, [3000, -3000], false, true);
        assert_eq!(view(&host), [0, 1664]);
        // Release ends it; a later press without a click does not resume.
        host.map_input(ORIGIN, SIZE, [84, 100], false, false);
        assert_eq!(host.drag_state, 0);
        host.map_input(ORIGIN, SIZE, [60, 100], false, true);
        assert_eq!((host.drag_state, view(&host)), (0, [0, 1664]));
        assert!(host.teleports.is_empty());
    }

    /// Transcription test of `InterfaceList` 753-763: Ctrl and a staff level
    /// turn the click into `Cheat.teleport` at the tile under the mouse.
    #[test]
    fn a_staff_ctrl_click_asks_for_a_teleport_instead_of_a_drag() {
        let mut host = host();
        // 60 right and 30 above the centre of the 512 by 334 component.
        let mouse = [ORIGIN[0] + 256 + 60, ORIGIN[1] + 167 - 30];
        host.ctrl_held = true;
        host.map_input(ORIGIN, SIZE, mouse, true, true);
        assert_eq!((host.drag_state, host.teleports.len()), (1, 0));
        host.map_input(ORIGIN, SIZE, mouse, false, false);
        (host.ctrl_held, host.staff_level) = (false, 2);
        host.map_input(ORIGIN, SIZE, mouse, true, true);
        assert_eq!((host.drag_state, host.teleports.len()), (1, 0));
        host.map_input(ORIGIN, SIZE, mouse, false, false);
        host.ctrl_held = true;
        host.map_input(ORIGIN, SIZE, mouse, true, true);
        // 60 * 2 / 6 = 20 tiles east and 30 * 2 / 6 = 10 tiles north of the
        // view centre (1000, 900), which is tile (3048, 3259).
        assert_eq!(host.teleports, [[3068, 3269]]);
        assert_eq!(host.drag_state, 0);
        assert_eq!(host.map.component, Some(SIZE));
    }

    /// Transcription test of `Protocol` 2581, 2792 and 2812.
    #[test]
    fn a_cycle_eases_only_if_the_previous_one_met_the_component() {
        let mut host = host();
        host.map.target_zoom = 8.0;
        host.begin_cycle();
        assert_eq!(host.map.zoom, 6.0);
        host.map.component = Some(SIZE);
        host.drag_state = 2;
        host.end_cycle();
        assert_eq!(host.drag_state, 2);
        host.begin_cycle();
        // zoom + zoom / 30.0 in double, stored as float.
        assert_eq!(host.map.zoom, (6.0f64 + 6.0 / 30.0) as f32);
        assert_eq!(host.map.component, None);
        // The pass did not meet the component this cycle.
        host.end_cycle();
        assert_eq!(host.drag_state, 0);
        host.begin_cycle();
        assert_eq!(host.map.zoom, (6.0f64 + 6.0 / 30.0) as f32);
    }

    /// Transcription test of `InterfaceList` 784-789.
    #[test]
    fn a_press_on_the_overview_recentres_the_view() {
        let mut host = host();
        host.map.pan_target_x = 4;
        host.overview_input([600, 40], [116, 104], [629, 66], false);
        assert_eq!([host.map.view_x, host.map.view_y], [1000, 900]);
        host.overview_input([600, 40], [116, 104], [629, 66], true);
        // width * 29 / 116 and length * 26 / 104.
        assert_eq!([host.map.view_x, host.map.view_y], [464, 416]);
        assert_eq!(host.map.pan_target_x, -1);
    }

    /// The staged interface: group 7, open as the top level.
    const BEFORE: i32 = 7 << 16 | 1;
    const MAP: i32 = 7 << 16 | 2;
    const AFTER: i32 = 7 << 16 | 3;
    const BLOCKER: i32 = 7 << 16 | 4;
    const OVERVIEW: i32 = 7 << 16 | 5;
    /// A button over the map before it in the walk, the map, a button after
    /// it, a click-through-blocking layer over the map's right part, and the
    /// overview inside that layer. The geometry is what a layout would store;
    /// the walk (`hook_walk.rs`) follows the component tree.
    fn state() -> State {
        let manifest = br#"{"source":"world map host","files":{}}"#;
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        let mut state = State::new(Pack::decode(bytes).unwrap());
        state.world_map = host();
        let placed = |id, parent, rect: [i32; 4]| Component {
            id,
            created_component_id: -1,
            if3: true,
            overlayer: parent,
            x: rect[0],
            y: rect[1],
            width: rect[2],
            height: rect[3],
            ..Default::default()
        };
        let button = |id, rect| Component {
            kind: 5,
            has_event_handlers: true,
            on_mouse_over: Some(vec![HookArg::Int(id)]),
            on_click_repeat: Some(vec![HookArg::Int(100 + id)]),
            ..placed(id, -1, rect)
        };
        let layer = |id, parent, rect, code, blocking| Component {
            kind: 0,
            client_code: code,
            no_click_through: blocking,
            ..placed(id, parent, rect)
        };
        // The overview sits at (404, 40) on screen, inside the layer.
        let mut overview = layer(OVERVIEW, BLOCKER, [4, 20, 116, 104], 1401, false);
        overview.has_event_handlers = true;
        overview.on_scroll = Some(vec![HookArg::Int(1370)]);
        for component in [
            button(BEFORE, [20, 30, 40, 40]),
            layer(MAP, -1, [10, 20, 512, 334], 1400, false),
            button(AFTER, [100, 30, 40, 40]),
            layer(BLOCKER, -1, [400, 20, 122, 334], 0, true),
            overview,
        ] {
            state.replace_component(Key::component(component.id), component);
        }
        state.loaded.insert(7);
        state.root = 7;
        state
    }
    fn pointer(point: [i32; 2], clicked: bool, down: bool) -> Option<PointerInput> {
        Some(PointerInput {
            point,
            clicked,
            down,
            any_down: down,
            ..Default::default()
        })
    }
    /// One logic cycle; returns the first argument of each queued hook.
    fn cycle(state: &mut State, input: Option<PointerInput>) -> Vec<i32> {
        state.queue_input_events(&[], input);
        let hooks = state
            .hooks
            .drain(..)
            .map(|hook| match hook.arguments[0] {
                HookArg::Int(id) => id,
                _ => unreachable!(),
            })
            .collect();
        state.finish_input_events();
        hooks
    }
    fn component(state: &mut State, id: i32) -> &mut Component {
        state.components.get_mut(&Key::component(id)).unwrap()
    }

    /// Transcription test of the interface pass around the map:
    /// `InterfaceList` 632, 661, 666-678, 723-735 and 750-789, with
    /// `Protocol`'s cycle order.
    #[test]
    fn the_pass_drags_the_map_and_silences_other_components_meanwhile() {
        let mut state = state();
        let view = |state: &State| [state.world_map.map.view_x, state.world_map.map.view_y];
        // Hovering the button after the map: its mouse-over hook, no drag.
        assert_eq!(cycle(&mut state, pointer([110, 40], false, false)), [AFTER]);
        assert_eq!(state.world_map.map.component, Some([512, 334]));
        assert_eq!(state.world_map.drag_state, 0);
        cycle(&mut state, pointer([300, 300], false, false));
        // A click on bare map arms the drag.
        assert!(cycle(&mut state, pointer([300, 300], true, true)).is_empty());
        assert_eq!(state.world_map.drag_state, 1);
        // Dragging across both buttons: neither hears of the mouse, and the
        // view follows. (300 - 110) * 2 / 6 = 63, (300 - 40) * 2 / 6 = 86.
        assert!(cycle(&mut state, pointer([110, 40], false, true)).is_empty());
        assert_eq!((state.world_map.drag_state, view(&state)), (2, [1063, 986]));
        assert!(cycle(&mut state, pointer([30, 40], false, true)).is_empty());
        assert_eq!(view(&state), [1090, 986]);
        // Released over the first button: the map, visited after it, ends
        // the drag, so the button hears of the mouse a cycle later.
        assert!(cycle(&mut state, pointer([30, 40], false, false)).is_empty());
        assert_eq!(state.world_map.drag_state, 0);
        assert_eq!(cycle(&mut state, pointer([30, 40], false, false)), [BEFORE]);
        // A click on a button over the map arms the drag too (the map does
        // not see what is drawn over it) and the button gets its click, as
        // the button is visited first.
        cycle(&mut state, pointer([300, 300], false, false));
        assert_eq!(
            cycle(&mut state, pointer([30, 40], true, true)),
            [100 + BEFORE, BEFORE]
        );
        assert_eq!(state.world_map.drag_state, 1);
        cycle(&mut state, pointer([30, 40], false, false));

        // Under the blocking layer a click arms nothing: the layer, visited
        // after the map, resets the state within the same pass.
        cycle(&mut state, pointer([450, 300], true, true));
        assert_eq!(state.world_map.drag_state, 0);
        cycle(&mut state, pointer([450, 300], false, false));
        // A drag that reaches the layer moves the view once more and ends.
        let start = view(&state);
        cycle(&mut state, pointer([300, 300], true, true));
        cycle(&mut state, pointer([450, 300], false, true));
        assert_eq!(state.world_map.drag_state, 0);
        assert_eq!(view(&state), [start[0] - 50, start[1]]);
        cycle(&mut state, pointer([460, 300], false, true));
        assert_eq!(view(&state), [start[0] - 50, start[1]]);
        cycle(&mut state, pointer([460, 300], false, false));

        // A press on the overview recentres: 1856 * 29 / 116, 1664 * 26 / 104.
        cycle(&mut state, pointer([433, 66], true, true));
        assert_eq!((state.world_map.drag_state, view(&state)), (0, [464, 416]));
        // The wheel over the overview queues its scroll hook; the map has none.
        let mut wheel = pointer([433, 66], false, false).unwrap();
        wheel.scroll = -1;
        assert_eq!(cycle(&mut state, Some(wheel)), [1370]);
        wheel.point = [300, 300];
        assert!(cycle(&mut state, Some(wheel)).is_empty());

        // With a menu open (or a component being dragged) the map sees no click.
        let mut blocked = pointer([300, 300], true, true).unwrap();
        blocked.suppressed = true;
        cycle(&mut state, Some(blocked));
        assert_eq!(state.world_map.drag_state, 0);
        // An armed drag ends when the overview is pressed while dragging is
        // impossible, and the overview ignores a press during a drag. (The
        // layer lets clicks through for these two cycles.)
        cycle(&mut state, pointer([300, 300], true, true));
        state.world_map.map.view_x = 500;
        component(&mut state, BLOCKER).no_click_through = false;
        cycle(&mut state, pointer([433, 66], false, true));
        assert_eq!(state.world_map.map.view_x, 500 + (300 - 433) * 2 / 6);
        assert_eq!(state.world_map.drag_state, 2);
        cycle(&mut state, pointer([433, 66], false, false));
        component(&mut state, BLOCKER).no_click_through = true;

        // No pointer at all: the component is still met.
        state.queue_events(&[]);
        assert_eq!(state.world_map.map.component, Some([512, 334]));
        state.hooks.clear();
        // The map component is hidden: the drag state is reset after the
        // cycle's hooks and the next cycle does not ease.
        cycle(&mut state, pointer([300, 300], true, true));
        assert_eq!(state.world_map.drag_state, 1);
        component(&mut state, MAP).hidden = true;
        cycle(&mut state, pointer([300, 300], false, true));
        assert_eq!(state.world_map.drag_state, 0);
        assert_eq!(state.world_map.map.component, None);
        state.world_map.map.target_zoom = 8.0;
        cycle(&mut state, None);
        assert_eq!(state.world_map.map.zoom, 6.0);
        component(&mut state, MAP).hidden = false;
        cycle(&mut state, None);
        assert_eq!(state.world_map.map.zoom, 6.0);
        cycle(&mut state, None);
        assert_eq!(state.world_map.map.zoom, (6.0f64 + 6.0 / 30.0) as f32);
    }

    /// Transcription test of which components the pass treats as the map.
    #[test]
    fn only_a_visited_if3_component_is_the_map() {
        let mut state = state();
        let map = Key::component(MAP);
        // IF1 components have no client-code handling in the pass.
        component(&mut state, MAP).if3 = false;
        cycle(&mut state, pointer([300, 300], true, true));
        assert_eq!(
            (state.world_map.map.component, state.world_map.drag_state),
            (None, 0)
        );
        // An IF3 component that is neither a layer nor has handlers or
        // server-set events is not visited at all.
        let c = component(&mut state, MAP);
        (c.if3, c.kind) = (true, 5);
        cycle(&mut state, pointer([300, 300], true, true));
        assert_eq!(state.world_map.map.component, None);
        state.properties.insert(
            map,
            crate::ui::Properties {
                events: 2,
                ..Default::default()
            },
        );
        cycle(&mut state, pointer([300, 300], true, true));
        assert_eq!(
            (state.world_map.map.component, state.world_map.drag_state),
            (Some([512, 334]), 1)
        );
        // Clipped away entirely: skipped.
        component(&mut state, MAP).width = 0;
        cycle(&mut state, pointer([300, 300], true, true));
        assert_eq!(
            (state.world_map.map.component, state.world_map.drag_state),
            (None, 0)
        );
    }

    fn one_map() -> MapList {
        MapList {
            maps: vec![Map {
                group: "main".into(),
                name: "RuneScape surface".into(),
                origin_x: 50,
                origin_y: 50,
                background_color: -1,
                valid: true,
                default_zoom: 75,
                chunks: Vec::new(),
                min_x: 2048,
                min_y: 2496,
                max_x: 3903,
                max_y: 4159,
            }],
            sprites: Vec::new(),
            visibility: Vec::new(),
        }
    }
    fn config() -> Config {
        Config {
            palette: vec![0; 65536],
            random_light_offset: 0,
            random_hue_offset: 0,
            map_scene_jitter: [0; 3],
            typeface: None,
        }
    }

    /// Transcription test of the lifecycle calls: `Protocol` 1284,
    /// `client.setGameState`, `LoginManager` 660 and 1132.
    #[test]
    fn the_lifecycle_calls_clear_and_restore_like_the_original() {
        let mut host = Host::default();
        assert!(!host.has_maps());
        host.install(one_map(), config(), Box::new(NoAssets))
            .unwrap();
        assert!(host.has_maps());
        let current = |host: &Host| host.map.current_map().map(|map| map.group.clone());
        host.map.switch_map("main");
        assert_eq!(current(&host).as_deref(), Some("main"));
        // A scene load drops the map and brings it back afterwards.
        host.scene_loading();
        assert_eq!(current(&host), None);
        assert_eq!(host.map.previous_map_name.as_deref(), Some("main"));
        host.scene_rebuilt();
        assert_eq!(current(&host).as_deref(), Some("main"));
        assert_eq!(host.map.previous_map_name, None);
        host.scene_rebuilt();
        assert_eq!(current(&host).as_deref(), Some("main"));
        // Any top-level open but reset 2 leaves the map alone.
        for reset in [0, 1, 3] {
            host.top_level_opened(reset);
        }
        assert_eq!(current(&host).as_deref(), Some("main"));
        assert_eq!(host.scene_requests(), SceneRequests::default());
        // Reset 2: the map is dropped without a name to restore, and the
        // scene it cleared is asked for again (game state 25).
        host.top_level_opened(2);
        assert_eq!(current(&host), None);
        host.scene_loading();
        assert_eq!(host.map.previous_map_name, None);
        host.scene_rebuilt();
        assert_eq!(current(&host), None);
        assert_eq!(
            host.take_scene_requests(),
            SceneRequests {
                rebuild_scene: 1,
                ..Default::default()
            }
        );
        assert_eq!(host.scene_requests(), SceneRequests::default());
        // Logout forgets the map and the name.
        host.map.switch_map("main");
        host.scene_loading();
        host.logged_out();
        host.scene_rebuilt();
        assert_eq!(current(&host), None);
    }

    /// The server's top-level open reaches the map before the interface changes.
    #[test]
    fn a_top_level_open_with_reset_two_resets_the_map() {
        use rs09_protocol::game::updates::ServerUpdate;
        let mut state = state();
        state
            .world_map
            .install(one_map(), config(), Box::new(NoAssets))
            .unwrap();
        for (reset, expected) in [(0, Some("main")), (2, None)] {
            state.world_map.map.switch_map("main");
            // This pack has no interface 746, so opening it fails afterwards.
            let _ = state.apply(ServerUpdate::WindowsPane {
                interface: 746,
                reset,
                counter: 0,
            });
            assert_eq!(
                state
                    .world_map
                    .map
                    .current_map()
                    .map(|map| map.group.as_str()),
                expected
            );
        }
        assert_eq!(state.world_map.scene_requests().rebuild_scene, 1);
    }

    /// Transcription test of `WorldMap.render` with nothing selected, which
    /// is all the browser build can show: black, the empty loading box 38
    /// pixels above the centre, and the text anchor on its baseline.
    #[test]
    fn without_a_map_the_component_shows_the_empty_loading_box() {
        let mut host = Host::default();
        for _ in 0..3 {
            let rendered = host.render_map([3222, 3218], &|_, _| 0, 400, 300).unwrap();
            assert_eq!(rendered.loading_text, Some([200, 300 / 2 - 18 - 20 + 20]));
            assert_eq!(rendered.step, Default::default());
        }
        assert_eq!(host.map.load_percentage, 0);
        let surface = &host.map_surface;
        assert_eq!((surface.size, surface.pixels.len()), ([400, 300], 120000));
        let at = |x: i32, y: i32| surface.pixels[(y * 400 + x) as usize];
        // The 304 by 34 frame at (48, 112), its black inner frame, no fill.
        assert_eq!([at(48, 112), at(351, 145), at(200, 112)], [0x8c1111; 3]);
        assert_eq!(
            [at(49, 113), at(50, 114), at(200, 128), at(47, 112)],
            [0; 4]
        );
        assert_eq!(
            surface.pixels.iter().filter(|&&pixel| pixel != 0).count(),
            2 * 304 + 2 * 32
        );
        assert_eq!(host.pings, 0);
        assert_eq!(host.scene_requests(), SceneRequests::default());
        // A new size is a new buffer.
        host.render_map([0, 0], &|_, _| 0, 320, 200).unwrap();
        assert_eq!(host.map_surface.size, [320, 200]);
        // The overview is black until a map is loaded.
        host.render_overview(&|_, _| 0, 116, 104).unwrap();
        assert_eq!(host.overview_surface.size, [116, 104]);
        assert!(host.overview_surface.pixels.iter().all(|&pixel| pixel == 0));
        // A component without area draws nothing and does not panic.
        host.render_map([0, 0], &|_, _| 0, 0, 0).unwrap();
        assert!(host.map_surface.pixels.is_empty());
        host.render_overview(&|_, _| 0, 0, 0).unwrap();
    }
}
