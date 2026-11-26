//! Per-frame actor decisions the mesh builders consult: which actors enter
//! the scene (`rs09_client::crowd`), which idle players are low detail, each
//! actor's body lean (`rs09_client::actor_turning`), the hint arrow at the
//! own player's feet (`rs09_client::hint_arrow`) and NPC bodies rebuilt when
//! the script recolour palette changes (`rs09_client::recolor`).
use super::{Workbench, render, tile};
use macroquad::prelude::*;
use rs09_client::{
    actor_turning::{Axis, Lean},
    crowd, hint_arrow,
    minimap::Hint,
    model::Model,
    pack::Pack,
    recolor::Palette,
    walking::{Movement, PlayerBas},
};
use std::collections::{BTreeMap, BTreeSet};

/// A player as the frame plan reads it; fine coordinates are absolute.
pub(super) struct PlayerView<'a> {
    pub index: usize,
    pub motion: &'a Movement,
    pub bas: Option<&'a PlayerBas>,
    /// An appearance has been decoded (`Player.isVisible`).
    pub visible: bool,
    pub size: i32,
}

#[derive(Default)]
pub(super) struct ActorScene {
    /// Players and NPCs left out of the scene this frame by crowd thinning.
    pub thinned_players: BTreeSet<usize>,
    pub thinned_npcs: BTreeSet<usize>,
    pub own_thinned: bool,
    /// Players whose `lowDetail` flag is set this frame (2047 is the own one).
    pub low_detail: BTreeSet<usize>,
    /// The frame's `SceneGraph.add` calls in order, as (is an NPC, index):
    /// the own player, top-priority NPCs, the listed players, other NPCs.
    /// World picking inserts its actors in this order.
    pub inserted: Vec<(bool, usize)>,
    /// Model height of each player when last drawn, which the overheads of a
    /// thinned player keep using (`PathingEntity.minY`).
    heights: BTreeMap<usize, i32>,
    leans: BTreeMap<(bool, usize), Lean>,
    axes: BTreeMap<(bool, usize), [Axis; 2]>,
    /// `[[minX, maxX], [minZ, maxZ]]` of the own body when last posed.
    pub own_extent: std::cell::Cell<Option<[[i32; 2]; 2]>>,
    pub palette: Palette,
    /// Palette revision each palette-recoloured NPC body was built with.
    palette_built: BTreeMap<u32, u64>,
    arrow_models: BTreeMap<i32, Model>,
}

impl ActorScene {
    /// This frame's `applyBodyLean` result for a player (false) or NPC (true).
    pub fn lean(&self, actor: (bool, usize)) -> Lean {
        self.leans.get(&actor).copied().unwrap_or_default()
    }

    /// One `applyBodyLean` call. The roll and pitch state is kept here, since
    /// the scene's NPC list is rebuilt from the update state every frame.
    fn advance_lean(&mut self, actor: (bool, usize), motion: &Movement, bas: &PlayerBas) {
        if !bas.turning.leans() {
            return;
        }
        let mut state = motion.turning;
        [state.roll, state.pitch] = self.axes.get(&actor).copied().unwrap_or_default();
        // A lean record without an acceleration makes the original divide by
        // zero while drawing; the body is drawn upright here.
        let lean = state.lean(&bas.turning).unwrap_or_default();
        self.axes.insert(actor, [state.roll, state.pitch]);
        self.leans.insert(actor, lean);
    }

    /// Record the heights of the players drawn this frame and supply the
    /// remembered height (200 before a first draw) for thinned ones.
    pub fn remember_heights(&mut self, drawn: &mut BTreeMap<usize, i32>) {
        for (&index, &height) in drawn.iter() {
            self.heights.insert(index, height);
        }
        for &index in &self.thinned_players {
            drawn
                .entry(index)
                .or_insert_with(|| self.heights.get(&index).copied().unwrap_or(200));
        }
    }
}

impl Workbench {
    /// `ScriptRunner.renderGameScene`'s insertion passes for this frame, plus
    /// one `applyBodyLean` per actor. `others` are the local players in list
    /// order. `flag` is the minimap destination flag in scene tiles, whose x
    /// the original zeroes when the own player stands on it.
    /// `many_idle_animations` is `Preferences.manyIdleAnimations`.
    pub(super) fn plan_actors(
        &mut self,
        own: PlayerView<'_>,
        others: &[PlayerView<'_>],
        flag: &mut [i32; 2],
        many_idle_animations: bool,
    ) {
        let origin = self.navigation.origin;
        let local = |fine: [i32; 2]| [fine[0] - origin[0] * 128, fine[1] - origin[1] * 128];
        let default_bas = PlayerBas::default();
        let scene = &mut self.actor_scene;
        scene.leans.clear();
        let mut seen = BTreeSet::new();
        let player = |view: &PlayerView<'_>| crowd::Player {
            index: view.index as i32,
            visible: view.visible,
            size: view.size,
            fine: local(view.motion.fine),
            idle: view.motion.sequence == view.bas.unwrap_or(&default_bas).idle,
            // Attached models (the original's `attachment`) are not ported.
            attached: false,
        };
        // `PlayerList.ids` never holds the own player (`crowd::live_frame`).
        let players: Vec<_> = others.iter().map(|view| Some(player(view))).collect();
        for view in std::iter::once(&own).chain(others) {
            if let Some(bas) = view.bas {
                scene.advance_lean((false, view.index), view.motion, bas);
                seen.insert((false, view.index));
            }
        }
        let mut npcs = Vec::with_capacity(self.npcs.len());
        for npc in &self.npcs {
            let d = &self.npc_definitions[&npc.id];
            npcs.push(Some(crowd::Npc {
                index: npc.index as i32,
                // `render_id` is None when the morph resolves to nothing.
                visible: npc.render_id.is_some(),
                top: d.top_render_priority,
                size: d.size,
                fine: local(npc.motion.fine),
                interactive: d.interactive,
            }));
            if let Some(bas) = self.npc_bases.get(&d.bas) {
                scene.advance_lean((true, npc.index), &npc.motion, &bas.movement);
                seen.insert((true, npc.index));
            }
        }
        scene.axes.retain(|actor, _| seen.contains(actor));
        let frame = crowd::live_frame(player(&own), &players, &npcs, many_idle_animations, *flag);
        flag[0] = frame.flag_x;
        scene.own_thinned = own.visible && !frame.inserted.iter().any(|i| i.who == crowd::Who::Own);
        scene.thinned_players = others
            .iter()
            .enumerate()
            .filter(|(position, view)| view.visible && !frame.draws_player(*position))
            .map(|(_, view)| view.index)
            .collect();
        scene.thinned_npcs = self
            .npcs
            .iter()
            .enumerate()
            .filter(|(position, npc)| npc.render_id.is_some() && !frame.draws_npc(*position))
            .map(|(_, npc)| npc.index)
            .collect();
        scene.inserted = frame
            .inserted
            .iter()
            .map(|inserted| match inserted.who {
                crowd::Who::Own => (false, own.index),
                crowd::Who::Player(position) => (false, others[position].index),
                crowd::Who::Npc(position) => (true, self.npcs[position].index),
            })
            .collect();
        scene.low_detail = others
            .iter()
            .zip(&frame.low_detail)
            .filter(|(_, low)| **low)
            .map(|(view, _)| view.index)
            .chain(frame.own_low_detail.then_some(own.index))
            .collect();
        scene
            .heights
            .retain(|index, _| others.iter().any(|v| v.index == *index));
    }

    /// Rebuild NPC bodies whose recolours read the script palette when CS2
    /// 5401 has changed it since they were built. The original clears its
    /// whole NPC model cache on every 5401; bodies without `recol_p` would be
    /// rebuilt identical.
    pub(super) fn sync_npc_palette(&mut self, palette: &Palette) -> Vec<String> {
        if self.actor_scene.palette.revision != palette.revision {
            self.actor_scene.palette = palette.clone();
        }
        let revision = palette.revision;
        let stale: Vec<u32> = self
            .npc_types
            .iter()
            .filter(|(id, body)| {
                !body.definition.palette.is_empty()
                    && self.actor_scene.palette_built.get(id).copied().unwrap_or(0) != revision
            })
            .map(|(&id, _)| id)
            .collect();
        let mut errors = Vec::new();
        for id in stale {
            let previous = self.npc_types.remove(&id);
            if let Err(error) = self.ensure_npc_body(id) {
                errors.push(error);
                // Keep drawing the body built with the old palette.
                if let Some(previous) = previous {
                    self.npc_types.insert(id, previous);
                }
            }
            self.actor_scene.palette_built.insert(id, revision);
        }
        errors
    }

    fn arrow_model(&mut self, id: i32, ui: Option<&Pack>) -> Result<Model, String> {
        if let Some(model) = self.actor_scene.arrow_models.get(&id) {
            return Ok(model.clone());
        }
        let key = u32::try_from(id).map_err(|_| format!("Hint arrow model {id}"))?;
        #[cfg(not(target_arch = "wasm32"))]
        if !self.models.contains_key(&key)
            && let Some(cache) = &self.world_cache
            && let Some(model) = cache.model(key)?
        {
            self.models.insert(key, model);
        }
        if !self.models.contains_key(&key) {
            let name = format!("model/{id}");
            let raw = self.pack.get(&name).or_else(|error| match ui {
                Some(ui) => ui.get(&name),
                None => Err(error),
            });
            if let Ok(raw) = raw {
                let model = Model::decode(raw).map_err(|e| format!("{name}: {e}"))?;
                self.models.insert(key, model);
            }
        }
        // HintArrowManager.getModel: createModel(64, 768, -50, -10, -50).
        let mut model = self
            .models
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("Hint arrow model {id} unavailable"))?;
        model.lighting = [64, 768];
        model.draw_order = rs09_client::surface::face_order(&model, &self.textures.materials);
        model.surfaces = rs09_client::surface::prepare(&model, &self.textures.materials);
        self.actor_scene.arrow_models.insert(id, model.clone());
        Ok(model)
    }

    /// The hint arrow models at the own player's feet (`Player.render` 450-479
    /// with `renderHintArrow`), as one mesh to draw the way character shadows
    /// are drawn: without depth writes, at the player's position, unrotated.
    /// `own` and the positions `player` returns are absolute fine coordinates.
    /// The model comes from the loaded scene models, the cache, the scene
    /// pack or the `ui` pack, whichever has it.
    pub(super) fn foot_arrow_mesh(
        &mut self,
        hints: &[Option<Hint>],
        own: [i32; 2],
        player: impl Fn(i32) -> Option<[i32; 2]>,
        ui: Option<&Pack>,
    ) -> Result<Mesh, String> {
        let mut mesh = self.textures.mesh();
        let Some(extent) = self.actor_scene.own_extent.get() else {
            return Ok(mesh);
        };
        let origin = self.navigation.origin;
        let local = |fine: [i32; 2]| [fine[0] - origin[0] * 128, fine[1] - origin[1] * 128];
        let arrows = hint_arrow::foot_arrows(
            hints,
            local(own),
            origin,
            |index| {
                self.npcs
                    .iter()
                    .find(|n| n.index as i32 == index)
                    .map(|n| local(n.motion.fine))
            },
            |index| player(index).map(local),
        );
        let plane = self.ground.plane;
        for arrow in arrows {
            let mut model = self.arrow_model(arrow.model, ui)?;
            let height = |fine: [i32; 2]| {
                rs09_client::ground_geometry::tile_height(plane, fine, |p, x, y| {
                    let t = tile(&self.regions, p as usize, x + origin[0], y + origin[1]);
                    (t.height, t.flags)
                })
            };
            let base = height(local(own));
            hint_arrow::lay(&mut model, arrow.angle, extent, local(own), base, height);
            let position = own.map(|n| n as f32 / 128.0);
            render::model(
                &mut mesh,
                &model,
                &self.textures,
                |v| {
                    vec3(
                        position[0] - 3200.0 + v[0] as f32 / 128.0,
                        -(base + v[1]) as f32 / 128.0,
                        position[1] - 3200.0 + v[2] as f32 / 128.0,
                    )
                },
                |n| vec3(n[0] as f32, -n[1] as f32, n[2] as f32),
            );
        }
        Ok(mesh)
    }
}
