//! GPU presentation of the software world map: the components with client
//! code 1400 (the map, `WorldMap.render`) and 1401 (its overview,
//! `Cs1ScriptRunner.renderWorldMapOverview`).
//!
//! The map is rendered in software by `rs09_client::world_map`, as the
//! original does in HD mode before `GlRaster.drawPixels`; this uploads the
//! buffer and draws it opaque at the component's position. The caller sets
//! the scissor to the component's rectangle, as the original's `setClip` does.
use macroquad::prelude::*;
use rs09_client::ui::{
    state::{DrawNode, State},
    vm::Vm,
    world_map_host::{self, Surface},
};

/// One uploaded software frame.
#[derive(Default)]
struct Layer {
    texture: Option<Texture2D>,
    size: [i32; 2],
    rgba: Vec<u8>,
}
impl Layer {
    /// `GlRaster.drawPixels(pixels, x, y, width, height)`: every pixel
    /// opaque, zero included.
    fn present(&mut self, surface: &Surface, at: [i32; 2]) {
        let [width, height] = surface.size;
        if !(1..=i32::from(u16::MAX)).contains(&width)
            || !(1..=i32::from(u16::MAX)).contains(&height)
            || surface.pixels.len() != width as usize * height as usize
        {
            return;
        }
        self.rgba.clear();
        self.rgba.extend(
            surface
                .pixels
                .iter()
                .flat_map(|&pixel| [(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8, 255]),
        );
        // Geometry queued with the texture's previous contents (or with a
        // texture about to be dropped) is drawn first.
        unsafe {
            get_internal_gl().flush();
        }
        match &self.texture {
            Some(texture) if self.size == surface.size => {
                texture.update_from_bytes(width as u32, height as u32, &self.rgba);
            }
            _ => {
                let texture = Texture2D::from_rgba8(width as u16, height as u16, &self.rgba);
                texture.set_filter(FilterMode::Nearest);
                self.texture = Some(texture);
                self.size = surface.size;
            }
        }
        if let Some(texture) = &self.texture {
            draw_texture(texture, at[0] as f32, at[1] as f32, WHITE);
        }
    }
}

#[derive(Default)]
pub struct Renderer {
    map: Layer,
    overview: Layer,
}
impl Renderer {
    /// Draws the node of a client code 1400 or 1401 component. For the map
    /// this runs `WorldMap.render`, which is also what advances loading: one
    /// step per drawn frame.
    ///
    /// Returns where the caller draws "Loading..." (centre x and baseline in
    /// screen pixels, b12 in white) while the map is loading.
    pub fn draw(
        &mut self,
        state: &mut State,
        vm: &Vm,
        node: &DrawNode,
        client_code: i32,
    ) -> Result<Option<[i32; 2]>, String> {
        let at = [node.rect.x, node.rect.y];
        if client_code == 1400 {
            let rendered = world_map_host::render_map(state, vm, node.rect.w, node.rect.h)?;
            self.map.present(&state.world_map.map_surface, at);
            Ok(rendered
                .loading_text
                .map(|[x, baseline]| [at[0].wrapping_add(x), at[1].wrapping_add(baseline)]))
        } else {
            world_map_host::render_overview(state, vm, node.rect.w, node.rect.h)?;
            self.overview.present(&state.world_map.overview_surface, at);
            Ok(None)
        }
    }
}

/// Start-up (`client` main load state 130): `MapList.init` over the cache
/// the client was pointed at, with the sprite jitters the minimap drew, and
/// a system face for the labels (`--world-map-font PATH.ttf` names one).
/// Without a readable cache the map stays empty and its commands answer as
/// they do before any map is loaded.
#[cfg(not(target_arch = "wasm32"))]
pub fn install(ui: &mut State, minimap: Option<&crate::minimap_render::Renderer>, capture: bool) {
    let directory = crate::option("--cache").unwrap_or_else(|| "assets/cache".into());
    let font = crate::option("--world-map-font");
    let typeface = world_map_host::system_typeface(font.as_deref().map(std::path::Path::new));
    if typeface.is_none() {
        eprintln!(
            "World map labels unavailable: no label font found; pass --world-map-font PATH.ttf"
        );
    }
    let startup = world_map_host::Startup {
        jitters: minimap.map(crate::minimap_render::Renderer::sprite_jitters),
        seed: if capture {
            0
        } else {
            (miniquad::date::now() * 1000.0) as u64
        },
        typeface,
    };
    if let Err(error) = ui.world_map.open_cache(directory.as_ref(), startup) {
        eprintln!("World map unavailable: {error}");
    }
}

/// The lifecycle calls the client makes at the points named in
/// `world_map_host`. Each is a no-op without an interface state.
pub fn scene_loading(ui: Option<&mut State>) {
    if let Some(ui) = ui {
        ui.world_map.scene_loading();
    }
}
pub fn scene_rebuilt(ui: Option<&mut State>) {
    if let Some(ui) = ui {
        ui.world_map.scene_rebuilt();
    }
}
pub fn logged_out(ui: Option<&mut State>) {
    if let Some(ui) = ui {
        ui.world_map.logged_out();
    }
}
