//! Ordinary tree/bronze axe gameplay on authoritative scene and inventory.
use super::{Location, Node, Probe, combat_services, ui_services as ui};
use rs09_game_core::random::JavaRandom;
use rs09_protocol::game::{actions::ClientAction, updates};
use rs09_server::{
    definitions::item::Shared,
    inventory::item::Item,
    world::{
        action_locks::Kind,
        doors::Handle,
        scenery::{self, ReplaceMode},
        scenery_node,
    },
};
use std::collections::BTreeMap;

const WOODCUTTING: usize = 8;
const BRONZE_AXE: i32 = 1351;

fn stump(id: i32) -> Option<i32> {
    Some(match id {
        1276 | 1278 | 2409 | 14308 | 14309 | 16264 | 16265 | 30132 | 30133 | 37477 => 1342,
        1277 | 1280 => 1343,
        1279 | 3033 | 3034 => 1345,
        1330..=1332 => 1341,
        3035 => 1347,
        3036 => 1351,
        3879 | 3881 | 3882 => 3880,
        3883 => 3884,
        37478 | 37652 => 37653,
        _ => return None,
    })
}

struct Chop {
    tree: Handle,
    until: i32,
    started: bool,
}

pub(super) struct Woodcutting {
    jobs: BTreeMap<Node, Chop>,
    rng: JavaRandom,
}

impl Woodcutting {
    pub(super) fn new(seed: i64) -> Self {
        Self {
            jobs: BTreeMap::new(),
            rng: JavaRandom::new(seed),
        }
    }
}

pub(super) fn cancel(p: &mut Probe, player: Node) {
    if let Some(s) = &mut p.woodcutting {
        s.jobs.remove(&player);
    }
}

pub(super) fn process(p: &mut Probe, player: Node, action: &ClientAction) -> Result<bool, String> {
    if p.woodcutting.is_none() {
        return Ok(false);
    }
    if matches!(
        action,
        ClientAction::WorldspaceWalk { .. } | ClientAction::MinimapWalk { .. }
    ) {
        cancel(p, player);
        return Ok(false);
    }
    let ClientAction::SceneryAction {
        opt_index,
        id,
        x,
        y,
    } = *action
    else {
        return Ok(false);
    };
    if stump(id).is_none() {
        return Ok(false);
    }
    let Some(from) = p.state.node_location(player).map_err(|e| e.to_string())? else {
        return Ok(true);
    };
    let [px, py, z] = from.coordinates();
    if (x - px).abs() > 20 || (y - py).abs() > 20 {
        return Ok(true);
    }
    let position = Location::new(x, y, z);
    let tree = p
        .host
        .scene
        .context(&mut p.host.maps)
        .object(position, id, 10)
        .map_err(|e| e.to_string())?;
    let Some(tree) = tree else {
        return Ok(true);
    };
    let options = tree
        .read()
        .definition
        .as_ref()
        .and_then(|d| d.read().options.clone());
    let is_chop = options.is_some_and(|a| {
        a.read()
            .get(opt_index as usize)
            .and_then(Option::as_deref)
            .is_some_and(|option| {
                matches!(
                    option.to_ascii_lowercase().as_str(),
                    "chop down" | "chop-down" | "chop" | "cut down"
                )
            })
    });
    if !is_chop {
        return Ok(false);
    }
    if super::script_services::locked(&mut p.state, player, Kind::Interaction)
        .map_err(|e| e.to_string())?
    {
        return Ok(true);
    }
    let owner = p.state.retain_node(player).map_err(|e| e.to_string())?;
    let player_fields = p.state.player_fields(&owner).map_err(|e| e.to_string())?;
    if !(player_fields.inventory().contain_items(&[BRONZE_AXE])
        || player_fields.equipment().contain_items(&[BRONZE_AXE]))
    {
        combat_services::message(p, player, "You do not have an axe to use.");
        return Ok(true);
    }
    combat_services::cancel(p, player)?;
    p.woodcutting
        .as_mut()
        .unwrap()
        .jobs
        .entry(player)
        .and_modify(|job| {
            if !job.tree.same(&tree) {
                *job = Chop {
                    tree: tree.clone(),
                    until: p.state.ticks,
                    started: false,
                };
            }
        })
        .or_insert_with(|| Chop {
            tree,
            until: p.state.ticks,
            started: false,
        });
    Ok(true)
}

pub(super) fn tick(p: &mut Probe, player: Node) -> Result<(), String> {
    let Some(job) = p.woodcutting.as_ref().and_then(|s| s.jobs.get(&player)) else {
        return Ok(());
    };
    let tree = job.tree.clone();
    let (destination, id, shape) = {
        let tree = tree.read();
        (tree.location.clone(), tree.id, tree.shape)
    };
    let current = p
        .host
        .scene
        .context(&mut p.host.maps)
        .object(destination.clone(), id, shape)
        .map_err(|e| e.to_string())?;
    if !current.is_some_and(|h| h.same(&tree)) {
        cancel(p, player);
        return Ok(());
    }
    let [x, y, z] = p
        .state
        .node_location(player)
        .map_err(|e| e.to_string())?
        .ok_or("missing player tile")?
        .coordinates();
    let [a, b, c] = destination.coordinates();
    if z != c || (x - a).abs() > 20 || (y - b).abs() > 20 {
        cancel(p, player);
        return Ok(());
    }
    if p.state.ticks.wrapping_sub(job.until) < 0 {
        return Ok(());
    }
    if super::script_services::locked(&mut p.state, player, Kind::Interaction)
        .map_err(|e| e.to_string())?
    {
        return Ok(());
    }
    let shape = tree.read().shape;
    let definition = tree.read().definition.clone();
    let (mut width, mut length) = definition.as_ref().map_or((1, 1), |d| {
        let d = d.read();
        (d.size_x.max(1), d.size_y.max(1))
    });
    if tree.read().rotation & 1 != 0 {
        std::mem::swap(&mut width, &mut length);
    }
    let edge = [x.clamp(a, a + width - 1), y.clamp(b, b + length - 1)];
    if (x - edge[0]).abs() + (y - edge[1]).abs() != 1 {
        // Search to a walkable tile on the edge. A route to the object's
        // rectangle may legally end at a diagonal or a different side.
        let mut sides = Vec::new();
        for dx in 0..width {
            sides.extend([[a + dx, b - 1], [a + dx, b + length]]);
        }
        for dy in 0..length {
            sides.extend([[a - 1, b + dy], [a + width, b + dy]]);
        }
        let mut scene = p.host.scene.context(&mut p.host.maps);
        let reachable = perimeter_route(&mut scene, [x, y, z], &sides)?;
        if let Some(points) = reachable {
            p.state
                .context(&mut p.host)
                .walk_path(
                    player,
                    rs09_server::world::pathfinding::Path {
                        successful: true,
                        move_near: false,
                        points,
                    },
                )
                .map_err(|e| e.to_string())?;
        } else {
            combat_services::message(p, player, "I can't reach that tree.");
            cancel(p, player);
            return Ok(());
        }
        p.woodcutting
            .as_mut()
            .unwrap()
            .jobs
            .get_mut(&player)
            .unwrap()
            .until = p.state.ticks.wrapping_add(2);
        return Ok(());
    }
    // Ordinary trees occupy a clipped tile. Check the directional wall bit
    // rather than treating the tree's own occupancy as a wall.
    let direction = match [edge[0] - x, edge[1] - y] {
        [1, 0] => 0x80,
        [-1, 0] => 0x08,
        [0, 1] => 0x20,
        _ => 0x02,
    };
    if p.host
        .scene
        .context(&mut p.host.maps)
        .clipping(Location::new(edge[0], edge[1], z), false)
        .map_err(|e| e.to_string())?
        & direction
        != 0
    {
        combat_services::message(p, player, "There is an obstacle in the way.");
        cancel(p, player);
        return Ok(());
    }
    let owner = p.state.retain_node(player).map_err(|e| e.to_string())?;
    let fields = p.state.player_fields(&owner).map_err(|e| e.to_string())?;
    if !fields.inventory().contain_items(&[BRONZE_AXE])
        && !fields.equipment().contain_items(&[BRONZE_AXE])
    {
        cancel(p, player);
        return Ok(());
    }
    if fields
        .inventory()
        .items()
        .read()
        .iter()
        .all(Option::is_some)
    {
        combat_services::message(
            p,
            player,
            "Your inventory is too full to hold any more logs.",
        );
        cancel(p, player);
        return Ok(());
    }
    let started = p.woodcutting.as_ref().unwrap().jobs[&player].started;
    if !started {
        combat_services::message(p, player, "You swing your axe at the tree...");
        p.woodcutting
            .as_mut()
            .unwrap()
            .jobs
            .get_mut(&player)
            .unwrap()
            .started = true;
    }
    p.state
        .actor(player)
        .map_err(|e| e.to_string())?
        .queue
        .points()
        .write()
        .clear();
    p.host.masks[&player]
        .face_location(Some(destination))
        .map_err(|e| e.to_string())?;
    combat_services::animate(p, player, 879)?;
    let level = f64::from(ui::skill_level(p, player, WOODCUTTING).unwrap_or(1));
    let chance =
        ((64.0 * (99.0 - level) / 98.0).floor() + (200.0 * (level - 1.0) / 98.0).floor() + 1.0)
            / 256.0;
    if p.woodcutting.as_mut().unwrap().rng.next_double() >= chance.clamp(0.0, 1.0) {
        p.woodcutting
            .as_mut()
            .unwrap()
            .jobs
            .get_mut(&player)
            .unwrap()
            .until = p.state.ticks.wrapping_add(3);
        return Ok(());
    }
    let replacement = scenery_node::transform(
        &mut p.host.scene.definitions,
        &tree,
        stump(id).unwrap(),
        None,
        None,
        Some(shape),
    )
    .map_err(|e| format!("{e:?}"))?;
    let regrow = 50 + (p.nodes.len() as i32 * 50 / 2000);
    let mut scene = p.host.scene.context(&mut p.host.maps);
    if !scenery::replace(
        &mut scene,
        Some(&tree),
        Some(&replacement),
        ReplaceMode::Temporary {
            clip: true,
            ticks: regrow,
        },
    )
    .map_err(|e| format!("{e:?}"))?
    {
        cancel(p, player);
        return Ok(());
    }
    let inserted = fields
        .apply(
            &mut p.state,
            &mut p.host.frames,
            super::now,
            |inventory, definitions, ctx| {
                let logs = Shared::new(Item::new(1511, 1, 1000, definitions));
                inventory.add(Some(&logs), true, -1, definitions, ctx)
            },
        )
        .map_err(|e| format!("log insertion failed: {e:?}"))?;
    if !inserted {
        return Err("eligible inventory failed to store one log".into());
    }
    ui::experience(p, player, WOODCUTTING as i32, 25.0).map_err(|e| e.to_string())?;
    combat_services::message(p, player, "You get some logs.");
    p.host.send(player, updates::synth_sound(2734, 1, 0));
    cancel(p, player);
    Ok(())
}

fn perimeter_route(
    scene: &mut impl rs09_server::world::pathfinding::Context,
    [x, y, z]: [i32; 3],
    sides: &[[i32; 2]],
) -> Result<Option<Vec<[i32; 2]>>, String> {
    use rs09_game_core::direction::Direction;
    use std::collections::VecDeque;
    const LIMIT: i32 = 24;
    const WIDTH: usize = (LIMIT as usize) * 2 + 1;
    let index = |a: i32, b: i32| ((a - x + LIMIT) as usize) * WIDTH + (b - y + LIMIT) as usize;
    let mut visited = [false; WIDTH * WIDTH];
    let mut parent = [None; WIDTH * WIDTH];
    let mut open = VecDeque::from([[x, y]]);
    visited[index(x, y)] = true;
    while let Some([a, b]) = open.pop_front() {
        if sides.contains(&[a, b]) {
            let mut points = vec![];
            let mut at = [a, b];
            while let Some(from) = parent[index(at[0], at[1])] {
                points.push(at);
                at = from;
            }
            points.reverse();
            return Ok(Some(points));
        }
        for direction in Direction::ALL {
            let (dx, dy) = direction.step();
            let (ax, by) = (a + dx, b + dy);
            if (x - ax).abs() > LIMIT || (y - by).abs() > LIMIT || visited[index(ax, by)] {
                continue;
            }
            let mut clear = true;
            for &(dx, dy, mask) in direction.traversal() {
                if scene
                    .clipping(z, a + dx, b + dy)
                    .map_err(|e| e.to_string())?
                    & mask
                    != 0
                {
                    clear = false;
                    break;
                }
            }
            if clear {
                visited[index(ax, by)] = true;
                parent[index(ax, by)] = Some([a, b]);
                open.push_back([ax, by]);
            }
        }
    }
    Ok(None)
}
