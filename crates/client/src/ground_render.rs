//! Visible item-pile geometry. Mutation, duplicate handling and selected item
//! order are supplied by protocol::game::ground, shared with the headless client.
use macroquad::prelude::*;
use rs09_client::ui::state::State;
use rs09_protocol::game::ground;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pile {
    pub plane: usize,
    pub tile: [i32; 2],
    /// Primary, secondary, tertiary: item ID and amount, without merging IDs.
    pub items: [Option<[i32; 2]>; 3],
    /// Original SceneGraph.setObjStack support height, in fine units.
    pub elevation: i32,
    pub height: i32,
}
pub struct Context<'a> {
    pub ui: Option<&'a mut State>,
    pub world: &'a crate::Workbench,
    pub supports: &'a mut BTreeMap<ground::Tile, i32>,
}
impl ground::Context for Context<'_> {
    fn info(&mut self, _state: &mut ground::State, id: i32) -> rs09_protocol::Result<ground::Info> {
        let ui = self.ui.as_mut().ok_or(rs09_protocol::Error::new(
            0,
            "Ground item metadata assets unavailable",
        ))?;
        let item = ui
            .items
            .get(&ui.pack, id)
            .map_err(|_| rs09_protocol::Error::new(0, "Ground item definition unavailable"))?;
        if ui.items.fallbacks.contains(&id) {
            ui.diagnostic(format!(
                "Ground item{id}: using the original default item definition"
            ));
        }
        Ok(ground::Info {
            cost: item.cost,
            stackable: item.stackable,
        })
    }
    fn height(
        &mut self,
        _state: &mut ground::State,
        plane: i32,
        x: i32,
        y: i32,
    ) -> rs09_protocol::Result<i32> {
        let origin = self.world.navigation.origin;
        Ok(rs09_client::ground_geometry::tile_height(
            plane,
            [x, y],
            |p, x, y| {
                let tile = crate::tile(
                    &self.world.regions,
                    p as usize,
                    origin[0] + x,
                    origin[1] + y,
                );
                (tile.height, tile.flags)
            },
        ))
    }
    fn publish(
        &mut self,
        state: &mut ground::State,
        tile: ground::Tile,
        pile: Option<ground::Pile>,
    ) -> rs09_protocol::Result<()> {
        // setObjStack captures scenery support height at publication. It is not
        // recomputed every frame when scenery later changes or animates.
        let elevation = pile.as_ref().map(|_| {
            self.world.ground_support(
                tile.plane as usize,
                [
                    self.world.navigation.origin[0] + tile.x,
                    self.world.navigation.origin[1] + tile.y,
                ],
            )
        });
        state.publish(tile, pile)?;
        if let Some(height) = elevation {
            self.supports.insert(tile, height);
        } else {
            self.supports.remove(&tile);
        }
        Ok(())
    }
}
pub fn piles(world: &crate::Workbench) -> Vec<Pile> {
    let origin = world.navigation.origin;
    world
        .ground
        .piles()
        .map(|(tile, pile)| {
            let position = [origin[0] + tile.x, origin[1] + tile.y];
            Pile {
                plane: tile.plane as usize,
                tile: position,
                items: [
                    Some(&pile.primary),
                    pile.secondary.as_ref(),
                    pile.tertiary.as_ref(),
                ]
                .map(|r| {
                    r.map(|r| {
                        let item = r.get();
                        [item.id, item.amount]
                    })
                }),
                height: pile.height,
                elevation: world.ground_supports.get(&tile).copied().unwrap_or(0),
            }
        })
        .collect()
}
struct Geometry {
    source: Pile,
    revision: u64,
    mesh: Mesh,
    bounds: [Vec3; 2],
}
#[derive(Default)]
pub struct Renderer {
    tiles: BTreeMap<(usize, [i32; 2]), Geometry>,
}
impl Renderer {
    pub fn clear(&mut self) {
        self.tiles.clear();
    }
    pub fn refresh(
        &mut self,
        piles: impl IntoIterator<Item = Pile>,
        state: &mut State,
        models: &mut crate::ui_model_render::Renderer,
        world: &crate::Workbench,
    ) -> Vec<String> {
        let mut keep = BTreeSet::new();
        let mut diagnostics = Vec::new();
        for pile in piles {
            let key = (pile.plane, pile.tile);
            keep.insert(key);
            if self
                .tiles
                .get(&key)
                .is_some_and(|g| g.source == pile && g.revision == world.map_revision)
            {
                continue;
            }
            let mut mesh = world.textures.mesh();
            let position = vec2(pile.tile[0] as f32 + 0.5, pile.tile[1] as f32 + 0.5);
            // SceneGraph draws secondary, tertiary, then primary at the same
            // tile centre; the item amount selects count-variant geometry.
            for index in [1, 2, 0] {
                let Some([id, amount]) = pile.items[index] else {
                    continue;
                };
                match models.item_model(state, world, id, amount) {
                    Ok(model) => {
                        let first = mesh.vertices.len();
                        crate::location_mesh(
                            &mut mesh,
                            &model,
                            &world.textures,
                            &world.regions,
                            position,
                            pile.plane,
                            false,
                        );
                        let surface =
                            crate::height(&world.regions, pile.plane, position.x, position.y);
                        for v in &mut mesh.vertices[first..] {
                            v.position.y +=
                                (pile.elevation as f32 - pile.height as f32) / 128.0 - surface;
                        }
                    }
                    Err(e) => diagnostics.push(format!(
                        "Ground item{id} at{},{} unavailable: {e}",
                        pile.tile[0], pile.tile[1]
                    )),
                }
            }
            let bounds = crate::geometry_bounds(&mesh, 0..mesh.vertices.len());
            self.tiles.insert(
                key,
                Geometry {
                    source: pile,
                    revision: world.map_revision,
                    mesh,
                    bounds,
                },
            );
        }
        self.tiles.retain(|key, _| keep.contains(key));
        diagnostics
    }
    pub fn draw(&self, plane: usize, position: Vec2) {
        for geometry in self.tiles.values() {
            let p = geometry.source;
            if p.plane == plane
                && vec2(p.tile[0] as f32, p.tile[1] as f32).distance(position) < 55.0
            {
                draw_mesh(&geometry.mesh);
            }
        }
    }
    /// The drawn pile on a tile: its vertices, three to a triangle.
    pub fn pile(&self, plane: usize, tile: [i32; 2]) -> Option<&[Vertex]> {
        self.tiles
            .get(&(plane, tile))
            .map(|geometry| geometry.mesh.vertices.as_slice())
            .filter(|vertices| !vertices.is_empty())
    }
    pub fn snapshot(&self) -> serde_json::Value {
        serde_json::json!(self.tiles.values().map(|g| serde_json::json!({
            "plane":g.source.plane,"tile":g.source.tile,"items":g.source.items,
            "elevation":g.source.elevation,"height":g.source.height,"triangles":g.mesh.indices.len()/3,
            "bounds":if g.mesh.vertices.is_empty(){None}else{Some(g.bounds.map(|v|v.to_array()))},
        })).collect::<Vec<_>>())
    }
}
