//! The graphics preferences the scene and its textures are built with:
//! `Preferences.highDetailLighting` (with the brightness, which is baked into
//! colours while it is off), `Preferences.showGroundDecorations` and
//! `Preferences.highDetailTextures`.
//!
//! The original reads them as statics wherever it packs a model, places a
//! location, builds the floor or generates a texture, and has what is
//! already built made again when a script changes them: CS2 6012 and 6005
//! always, and 6001 while high detail lighting is off, rebuild the scene
//! (`ScriptRunner.refreshGameStateAfterSettingsChange`, game state 25, whose
//! `client.unload` empties every model cache); CS2 6006 empties the texture
//! cache (`Js5GlTextureProvider.setLowDetail`). [`Detail::apply`] moves the
//! preferences into `rs09_client::sd_light` and `rs09_client::texture`,
//! where the builders read them, and says what has to be made again.
//!
//! Capture runs read no saved preferences, so three launch switches stand
//! over them: `--sd-lighting`, `--low-detail-textures` and `--brightness N`.
//! Two more turn a preference over while the client runs, as a script would,
//! so that a capture can show what a change at run time leaves on screen:
//! `--lighting-switch FRAME` and `--texture-switch FRAME`.
use super::{Geometry, MultiLoc, PreparedChunk, Scene, Workbench, render, scene_floors};
use rs09_client::ui::vm::Preferences;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Detail {
    sd_lighting: bool,
    low_detail_textures: bool,
    brightness: Option<i32>,
    /// The frame from which high detail lighting is the other way round.
    lighting_switch: Option<i32>,
    /// The same for high detail textures.
    texture_switch: Option<i32>,
}

/// What a change of the preferences leaves to be made again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// The scene: colours are packed differently, or ground decoration is
    /// placed differently.
    pub scene: bool,
    /// The material textures: their size changed.
    pub textures: bool,
}

impl Detail {
    pub fn from_arguments(arguments: impl IntoIterator<Item = String>) -> Self {
        let mut detail = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--sd-lighting" => detail.sd_lighting = true,
                "--low-detail-textures" => detail.low_detail_textures = true,
                "--brightness" => {
                    detail.brightness = arguments
                        .next()
                        .and_then(|value| value.parse().ok())
                        .map(|value: i32| value.clamp(1, 4));
                }
                "--lighting-switch" => {
                    detail.lighting_switch = arguments.next().and_then(|value| value.parse().ok());
                }
                "--texture-switch" => {
                    detail.texture_switch = arguments.next().and_then(|value| value.parse().ok());
                }
                _ => {}
            }
        }
        detail
    }

    /// `--brightness N`, which stands until the preference changes.
    pub fn brightness(&self) -> Option<i32> {
        self.brightness
    }

    /// The preferences as the builders are to read them from now on.
    /// `brightness` is the frame's (the preference, or the workbench's key)
    /// and `frame` the number of frames drawn, None before the first.
    pub fn apply(&self, preferences: &Preferences, brightness: i32, frame: Option<i32>) -> Change {
        let switched =
            |switch: Option<i32>| switch.is_some_and(|at| frame.is_some_and(|frame| frame >= at));
        // CS2 6005 rebuilds the scene as well: SceneGraph.addLoc reads the
        // ground decoration preference when it places a location.
        let decorations =
            rs09_client::loc_placement::ground_decorations() != preferences.show_ground_decorations;
        rs09_client::loc_placement::set_ground_decorations(preferences.show_ground_decorations);
        let lighting = rs09_client::sd_light::set(
            (preferences.high_detail_lighting && !self.sd_lighting)
                != switched(self.lighting_switch),
            brightness,
        );
        Change {
            scene: lighting || decorations,
            textures: rs09_client::texture::set_low_detail(
                (!preferences.high_detail_textures || self.low_detail_textures)
                    != switched(self.texture_switch),
            ),
        }
    }
}

impl Workbench {
    /// `client.setGameState(25)` after a graphics preference changed, where
    /// no map streaming builds the scene again: the static geometry is made
    /// again from the prepared pack the scene was loaded from, and the
    /// location changes of the session are put back as a rebuild's are.
    pub(super) fn rebuild_static(
        &mut self,
        ui: Option<&mut rs09_client::ui::state::State>,
    ) -> Result<(), String> {
        let stored: Scene =
            serde_json::from_slice(self.pack.get("scene.json")?).map_err(|e| e.to_string())?;
        // The definitions the load completed (the underwater scene adds its own).
        let scene = Scene {
            definitions: self.definitions.clone(),
            ..stored
        };
        if !scene
            .regions
            .iter()
            .all(|r| self.regions.iter().any(|loaded| [loaded.x, loaded.z] == *r))
        {
            return Err("The scene shown is not the prepared pack's".into());
        }
        let (underlays, overlays) = scene_floors(&self.pack)?;
        let geometry = Geometry::build(
            &scene,
            &self.regions,
            &self.models,
            &self.sequences,
            &self.textures,
            &self.location_types,
            &underlays,
            &overlays,
        )?;
        let mode = self.shadows.mode;
        self.chunks = geometry
            .chunks
            .into_iter()
            .map(PreparedChunk::activate)
            .collect();
        // A Loc keeps its clock through the rebuild.
        let mut animated = geometry.animated_locations;
        for loc in &mut animated {
            if let Some(old) = self.animated_locations.iter().find(|old| {
                old.target.plane == loc.target.plane
                    && old.target.tile == loc.target.tile
                    && old.target.shape == loc.target.shape
                    && old.target.id == loc.target.id
                    && old.sequence == loc.sequence
                    && old.rotation == loc.rotation
            }) {
                loc.clock = old.clock.clone();
            }
        }
        self.animated_locations = animated;
        self.faces = geometry.faces;
        self.objects = geometry.objects;
        self.shadows = render::Shadows::from_map(geometry.shadows);
        self.shadows.mode = mode;
        self.shadow_roofs = true;
        self.static_shadows = geometry.static_shadows;
        self.pick_objects = geometry.pick_objects;
        self.multilocs = geometry
            .multilocs
            .into_iter()
            .map(|(slot, loc)| (slot, MultiLoc::new(loc, None)))
            .collect();
        let retained: Vec<_> = self
            .world
            .changes
            .iter()
            .filter(|change| change.set_loops == -1)
            .map(|change| (change.slot, change.replacement))
            .collect();
        for (slot, replacement) in retained {
            self.dynamic_location(slot, replacement, None)?;
        }
        self.map_revision = self.map_revision.wrapping_add(1);
        self.scene_revision = self.scene_revision.wrapping_add(1);
        self.refresh_ground(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_switches_are_read_from_the_arguments() {
        let arguments =
            |list: &[&str]| Detail::from_arguments(list.iter().map(|s| (*s).to_owned()));
        assert_eq!(arguments(&["client", "--capture"]), Detail::default());
        assert_eq!(
            arguments(&[
                "--sd-lighting",
                "--brightness",
                "9",
                "--low-detail-textures"
            ]),
            Detail {
                sd_lighting: true,
                low_detail_textures: true,
                brightness: Some(4),
                ..Detail::default()
            }
        );
        assert_eq!(arguments(&["--brightness"]).brightness(), None);
        assert_eq!(arguments(&["--brightness", "x"]).brightness(), None);
    }
}
