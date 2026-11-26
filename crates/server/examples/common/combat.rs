//! Ordinary, unmorphed NPCs and weapon combat. The world tick owns
//! targeting, rolls, impact and cooldown.
use super::weapons::{self, Stance, Style};
use super::{Frame, Location, Node, Probe, npc_services as npc, ui_services as ui};
use rs09_game_core::{direction::Direction, random::JavaRandom};
use rs09_protocol::game::actions::ClientAction;
use rs09_protocol::game::updates::{self, Projectile};
use rs09_server::inventory::equipment;
use rs09_server::world::{
    action_locks::Kind,
    animation::{Animation, Graphics, Priority},
    movement,
    pathfinding::{
        Path,
        projectile::{self, Footprint},
    },
    prepared_masks::FaceTarget,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct Engagement {
    npc: Node,
    route: Option<([i32; 3], i32)>,
}

#[derive(Clone, Copy)]
struct PlayerImpact {
    due: i32,
    damage: i32,
    style: Stance,
}

struct Retaliation {
    victim: Node,
    next_swing: i32,
    impact: Option<(i32, i32)>,
}

pub(super) struct Melee {
    players: BTreeMap<Node, Engagement>,
    cooldowns: BTreeMap<Node, i32>,
    impacts: BTreeMap<Node, Vec<PlayerImpact>>,
    projectiles: Vec<([i32; 3], Projectile)>,
    retaliation: BTreeMap<Node, Retaliation>,
    rng: JavaRandom,
}

impl Melee {
    pub(super) fn new(seed: i64) -> Self {
        Self {
            players: BTreeMap::new(),
            cooldowns: BTreeMap::new(),
            impacts: BTreeMap::new(),
            projectiles: Vec::new(),
            retaliation: BTreeMap::new(),
            rng: JavaRandom::new(seed),
        }
    }
}

fn projectile(
    p: &mut Probe,
    from: &Location,
    to: &Location,
    npc: Node,
    missile: weapons::Missile,
    speed_scale: f64,
) -> Result<(), String> {
    let [x, y, plane] = from.coordinates();
    let [tx, ty, _] = to.coordinates();
    let dx = tx - x;
    let dy = ty - y;
    let target = p.state.actor(npc).map_err(|e| e.to_string())?.wire_index + 1;
    let end = missile.speed + (f64::from(dx).hypot(f64::from(dy)) * speed_scale) as i32;
    p.melee.as_mut().unwrap().projectiles.push((
        [x, y, plane],
        Projectile {
            offset: [x & 7, y & 7],
            delta: [dx, dy],
            target,
            spot: missile.spot,
            start_height: missile.heights[0],
            end_height: missile.heights[1],
            start: missile.start,
            end,
            angle: missile.angle,
            distance: missile.distance,
        },
    ));
    Ok(())
}

pub(super) fn render_projectiles(
    p: &mut Probe,
    node: Node,
    player: &rs09_server::world::player_render::Handle,
) -> Result<(), String> {
    let Some(melee) = &p.melee else { return Ok(()) };
    if melee.projectiles.is_empty() {
        return Ok(());
    }
    let player = player.read();
    let Some(scene) = player.last_scene.as_ref().or(player.location.as_ref()) else {
        return Ok(());
    };
    let [x, y, plane] = scene.coordinates();
    let [bx, by] = [((x >> 3) - 6) << 3, ((y >> 3) - 6) << 3];
    for &([sx, sy, z], packet) in &melee.projectiles {
        if z != plane || !(0..104).contains(&(sx - bx)) || !(0..104).contains(&(sy - by)) {
            continue;
        }
        let originated = updates::zone_enclosed((sx & !7) - bx, (sy & !7) - by, &[packet.frame()])
            .map_err(|e| e.to_string())?;
        p.host.send(node, originated);
    }
    Ok(())
}

pub(super) fn clear_projectiles(p: &mut Probe) {
    if let Some(melee) = &mut p.melee {
        melee.projectiles.clear();
    }
}

pub(super) fn remove(p: &mut Probe, node: Node) -> Result<(), String> {
    cancel(p, node)?;
    if let Some(melee) = &mut p.melee {
        melee.cooldowns.remove(&node);
    }
    Ok(())
}

pub(super) fn cancel(p: &mut Probe, node: Node) -> Result<(), String> {
    let Some(melee) = &mut p.melee else {
        return Ok(());
    };
    melee.impacts.remove(&node);
    let Some(engagement) = melee.players.remove(&node) else {
        return Ok(());
    };
    if melee
        .retaliation
        .get(&engagement.npc)
        .is_some_and(|r| r.victim == node)
    {
        melee.retaliation.remove(&engagement.npc);
        if let Ok(actor) = p.state.actor_mut(engagement.npc) {
            actor.view.combat_running = false;
            actor.queue.points().write().clear();
        }
        npc::face_player(p, engagement.npc, -1);
    }
    if let Ok(actor) = p.state.actor_mut(node) {
        if engagement.route.is_some() {
            actor.queue.points().write().clear();
        }
        actor.view.combat_running = false;
        actor.view.victim = None;
    }
    if let Some(masks) = p.host.masks.get(&node) {
        masks.clear_face_entity().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(super) fn process(
    p: &mut Probe,
    player_index: i32,
    player: Node,
    action: &ClientAction,
) -> Result<bool, String> {
    if p.melee.is_none() {
        return Ok(false);
    }
    if matches!(
        action,
        ClientAction::WorldspaceWalk { .. } | ClientAction::MinimapWalk { .. }
    ) {
        cancel(p, player)?;
        return Ok(false);
    }
    let ClientAction::NpcAction {
        opt_index,
        npc_index,
    } = *action
    else {
        return Ok(false);
    };
    // An NPC index is only valid for this player if it came from the player's
    // last local update. Resolve that retained view, not a recycled index.
    let Some(npc) = npc::visible(p, player_index, npc_index) else {
        return Ok(false);
    };
    if !npc::attack_option(p, npc, opt_index) {
        return Ok(false);
    }
    if npc::fighter(p, npc).is_none()
        || locked(p, player)?
        || ui::melee_stats(p, player).is_none_or(|(_, hp, _)| hp <= 0)
    {
        return Ok(true);
    }
    if p.melee
        .as_ref()
        .unwrap()
        .players
        .get(&player)
        .is_some_and(|e| e.npc == npc)
    {
        return Ok(true); // repeated clicks cannot reset the attack timer
    }
    cancel(p, player)?;
    let node = p.state.retain_node(npc).map_err(|e| e.to_string())?;
    let view = npc::view(&p.host, npc).ok_or("missing visible NPC")?;
    p.host.masks[&player]
        .face_entity(Some(FaceTarget::Npc(node, view)))
        .map_err(|e| e.to_string())?;
    let actor = p.state.actor(player).map_err(|e| e.to_string())?;
    let manager = actor.manager.clone();
    actor.queue.points().write().clear();
    p.state
        .context(&mut p.host)
        .clear_standard(player, manager)
        .map_err(|e| e.to_string())?;
    if !super::script_services::locked(&mut p.state, npc, Kind::Movement)
        .map_err(|e| e.to_string())?
    {
        p.state
            .actor(npc)
            .map_err(|e| e.to_string())?
            .queue
            .points()
            .write()
            .clear();
    }
    p.melee
        .as_mut()
        .unwrap()
        .players
        .insert(player, Engagement { npc, route: None });
    let actor = p.state.actor_mut(player).map_err(|e| e.to_string())?;
    actor.view.combat_running = true;
    actor.view.victim = Some(npc);
    super::skilling::cancel(p, player);
    Ok(true)
}

fn ready(tick: i32, due: i32) -> bool {
    tick.wrapping_sub(due) >= 0
}

fn locked(p: &mut Probe, player: Node) -> Result<bool, String> {
    super::script_services::locked(&mut p.state, player, Kind::Interaction)
        .map_err(|e| e.to_string())
}

pub(super) fn message(p: &mut Probe, player: Node, message: &str) {
    let mut payload = message.as_bytes().to_vec();
    payload.push(0);
    p.host.send(
        player,
        Frame {
            opcode: 70,
            payload,
        },
    );
}

pub(super) fn animate(p: &mut Probe, player: Node, id: i32) -> Result<(), String> {
    animate_player(p, player, Animation::with(id, 0, Some(Priority::High)))
}

fn animate_player(p: &mut Probe, player: Node, animation: Animation) -> Result<(), String> {
    visualize_attack(p, player, animation, None)
}

fn visualize_attack(
    p: &mut Probe,
    player: Node,
    animation: Animation,
    graphic: Option<Graphics>,
) -> Result<(), String> {
    let animator = p
        .host
        .animators
        .get(&player)
        .ok_or("missing combat animator")?
        .clone();
    p.host.animations.ticks = p.state.ticks;
    animator
        .animate(&mut p.host.animations, Some(animation), graphic)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

struct Target<'a>(&'a Location, i32);
impl Footprint for Target<'_> {
    fn size(&self) -> rs09_server::world::pathfinding::Result<i32> {
        Ok(self.1)
    }
    fn location(&self) -> rs09_server::world::pathfinding::Result<Option<Location>> {
        Ok(Some(self.0.clone()))
    }
}

pub(super) fn in_reach(
    p: &mut Probe,
    from: &Location,
    target: &Location,
    size: i32,
    reach: i32,
) -> Result<bool, String> {
    let [x, y, z] = from.coordinates();
    let [tx, ty, tz] = target.coordinates();
    // For this size-one player, the cardinal ring of the victim's footprint
    // is the stationary distance-one branch of canMelee.
    let [cx, cy] = [x.clamp(tx, tx + size - 1), y.clamp(ty, ty + size - 1)];
    let dx = (cx - x).abs();
    let dy = (cy - y).abs();
    let too_far = if reach == 1 {
        dx + dy > 1
    } else {
        let half = size / 2;
        let distance = f64::from(x - (tx + half)).hypot(f64::from(y - (ty + half)));
        distance > f64::from(reach + half)
    };
    if z != tz || dx + dy == 0 || too_far {
        return Ok(false);
    }
    let mut world = p.host.scene.context(&mut p.host.maps);
    if reach > 1 {
        return projectile::can_reach(&mut world, from, Some(&Target(target, size)), false)
            .map_err(|e| e.to_string());
    }
    let direction = Direction::from_delta(cx - x, cy - y);
    for &(dx, dy, mask) in direction.traversal() {
        if world
            .clipping(Location::new(x + dx, y + dy, z), false)
            .map_err(|e| e.to_string())?
            & mask
            != 0
        {
            return Ok(false);
        }
    }
    projectile::can_reach(&mut world, from, Some(&Target(target, size)), true)
        .map_err(|e| e.to_string())
}

/// Base melee rolls from MeleeSwingHandler and CombatSwingHandler.
fn roll(rng: &mut JavaRandom, accuracy: i32, defence: i32, strength: i32) -> i32 {
    let (a, d) = (f64::from(accuracy.max(0)), f64::from(defence.max(0)));
    let chance = if a > d {
        1.0 - (d + 2.0) / (2.0 * (a + 1.0))
    } else {
        a / (2.0 * (d + 1.0))
    };
    if rng.next_double() >= chance {
        return 0;
    }
    let max = (0.5 + f64::from(strength) / 640.0).floor() as i32;
    rng.next_bound(max.max(0) + 1).unwrap_or(0)
}

pub(super) fn approach(p: &mut Probe, player: Node) -> Result<(), String> {
    let Some(engagement) = p
        .melee
        .as_ref()
        .and_then(|m| m.players.get(&player))
        .copied()
    else {
        return Ok(());
    };
    let (Some(from), Some(to)) = (
        p.state.node_location(player).map_err(|e| e.to_string())?,
        p.state
            .node_location(engagement.npc)
            .map_err(|e| e.to_string())?,
    ) else {
        return Ok(());
    };
    if !within_combat_range(&from, &to)
        || npc::fighter(p, engagement.npc).is_none()
        || locked(p, player)?
    {
        return Ok(());
    }
    let queue = p
        .state
        .actor(player)
        .map_err(|e| e.to_string())?
        .queue
        .clone();
    let now = p.state.ticks;
    let target = to.coordinates();
    if engagement
        .route
        .is_some_and(|(tile, retry)| tile == target && (queue.has_path() || !ready(now, retry)))
    {
        return Ok(());
    }
    let size = p
        .state
        .node_size(engagement.npc)
        .map_err(|e| e.to_string())?
        .max(1);
    let weapon = ui::equipment(p, player).and_then(|e| e.get(equipment::WEAPON));
    let slot = ui::melee_stats(p, player).map_or(0, |(_, _, style)| style);
    let (ordinal, _, style) = selected_style(weapon.as_ref(), slot);
    let reach = attack_reach(weapon.as_ref(), ordinal, style);
    let path = if in_reach(p, &from, &to, size, reach)? {
        Path::default()
    } else {
        approach_path(p, player, engagement.npc, &from, &to, size, reach)?
    };
    queue.points().write().clear();
    if !path.points.is_empty() {
        p.state
            .context(&mut p.host)
            .walk_path(player, path)
            .map_err(|e| e.to_string())?;
    }
    p.melee
        .as_mut()
        .unwrap()
        .players
        .get_mut(&player)
        .unwrap()
        .route = Some((target, now.wrapping_add(2)));
    Ok(())
}

pub(super) fn approach_path(
    p: &mut Probe,
    player: Node,
    npc: Node,
    from: &Location,
    target: &Location,
    size: i32,
    reach: i32,
) -> Result<Path, String> {
    let [tx, ty, z] = target.coordinates();
    let [px, py, _] = from.coordinates();
    let direct = p
        .state
        .context(&mut p.host)
        .find_node_path(player, npc)
        .map_err(|e| e.to_string())?;
    if let Some(&[x, y]) = direct.points.last()
        && in_reach(p, &Location::new(x, y, z), target, size, reach)?
    {
        return Ok(direct);
    }

    // Generic entity routes can stop inside a large NPC or against a wall
    // that permits approaching the footprint but blocks the melee swing.
    let mut perimeter = Vec::new();
    for i in 0..size {
        perimeter.extend([
            [tx - 1, ty + i],
            [tx + size, ty + i],
            [tx + i, ty - 1],
            [tx + i, ty + size],
        ]);
    }
    perimeter.sort_unstable_by_key(|[x, y]| ((x - px).abs() + (y - py).abs(), *x, *y));
    for [x, y] in perimeter {
        let tile = Location::new(x, y, z);
        if !in_reach(p, &tile, target, size, reach)? {
            continue;
        }
        let path = p
            .state
            .context(&mut p.host)
            .find_path(player, tile, movement::SMART, false)
            .map_err(|e| e.to_string())?;
        if path.successful && path.points.last() == Some(&[x, y]) {
            return Ok(path);
        }
    }
    Ok(
        if direct.move_near && direct.points.last() != Some(&[px, py]) {
            direct
        } else {
            Path::default()
        },
    )
}

fn selected_style(
    weapon: Option<&rs09_server::inventory::item::Handle>,
    slot: i32,
) -> (usize, usize, Style) {
    let ordinal = weapons::ordinal(weapon);
    let styles = weapons::INTERFACES[ordinal].styles;
    let slot = (slot as usize).min(styles.len() - 1);
    (ordinal, slot, styles[slot])
}

fn attack_reach(
    weapon: Option<&rs09_server::inventory::item::Handle>,
    ordinal: usize,
    style: Style,
) -> i32 {
    if style.bonus != 4 {
        return if ordinal == 15 { 2 } else { 1 };
    }
    if weapons::ranged_weapon(weapon).is_some_and(|w| w.double_shot || w.degrading) {
        return 10;
    }
    let base = if weapons::INTERFACES[ordinal].id == 91 {
        5
    } else {
        7
    };
    base + 2 * i32::from(style.stance == Stance::Longrange)
}

fn within_combat_range(from: &Location, to: &Location) -> bool {
    let [x, y, z] = from.coordinates();
    let [tx, ty, tz] = to.coordinates();
    z == tz && (tx - x).abs() <= 15 && (ty - y).abs() <= 15
}

pub(super) fn tick(p: &mut Probe, player: Node) -> Result<(), String> {
    let Some(engagement) = p
        .melee
        .as_ref()
        .and_then(|m| m.players.get(&player))
        .copied()
    else {
        return Ok(());
    };
    let now = p.state.ticks;
    let Some(monster) = npc::fighter(p, engagement.npc) else {
        return cancel(p, player);
    };
    let Some(([attack, defence, strength, ranged], hp, style)) = ui::melee_stats(p, player) else {
        return cancel(p, player);
    };
    if hp <= 0 {
        return cancel(p, player);
    }
    let equipment = ui::equipment(p, player).ok_or("missing combat equipment")?;
    let weapon = equipment.get(equipment::WEAPON);
    let (ordinal, slot, style) = selected_style(weapon.as_ref(), style);
    let (Some(from), Some(to)) = (
        p.state.node_location(player).map_err(|e| e.to_string())?,
        p.state
            .node_location(engagement.npc)
            .map_err(|e| e.to_string())?,
    ) else {
        return cancel(p, player);
    };
    if !within_combat_range(&from, &to) {
        return cancel(p, player);
    }
    if let Some(retaliation) = p
        .melee
        .as_mut()
        .unwrap()
        .retaliation
        .get_mut(&engagement.npc)
        .filter(|r| r.victim == player)
        && let Some((_, damage)) = retaliation.impact.filter(|(due, _)| ready(now, *due))
    {
        retaliation.impact = None;
        if receive_hit(
            p,
            player,
            damage,
            hp,
            weapons::defence_animation(&equipment),
        )? {
            return Ok(());
        }
    }
    let mut impacts = Vec::new();
    if let Some(pending) = p.melee.as_mut().unwrap().impacts.get_mut(&player) {
        pending.retain(|impact| {
            if ready(now, impact.due) {
                impacts.push(*impact);
                false
            } else {
                true
            }
        });
    }
    let mut remaining = monster.life;
    for impact in impacts {
        let dealt = npc::damage(p, engagement.npc, impact.damage)?;
        ui::melee_experience(p, player, impact.style, dealt).map_err(|e| e.to_string())?;
        remaining -= dealt;
        if remaining == 0 {
            return cancel(p, player);
        }
    }
    if locked(p, player)? {
        return Ok(());
    }
    let size = p
        .state
        .node_size(engagement.npc)
        .map_err(|e| e.to_string())?
        .max(1);
    if in_reach(
        p,
        &from,
        &to,
        size,
        attack_reach(weapon.as_ref(), ordinal, style),
    )? && p
        .melee
        .as_ref()
        .unwrap()
        .cooldowns
        .get(&player)
        .is_none_or(|&due| ready(now, due))
    {
        p.state
            .actor(player)
            .map_err(|e| e.to_string())?
            .queue
            .points()
            .write()
            .clear();
        if style.bonus == 3 {
            message(p, player, "Select a melee stance to fight with this staff.");
            return cancel(p, player);
        }
        let range_weapon = (style.bonus == 4)
            .then(|| weapons::ranged_weapon(weapon.as_ref()))
            .flatten();
        if style.bonus == 4 && range_weapon.is_none() {
            message(
                p,
                player,
                "This weapon requires its own ranged attack handler.",
            );
            return cancel(p, player);
        }
        let mut fired = None;
        let mut animation = weapons::attack_animation(weapon.as_ref(), slot);
        let mut bonuses = weapons::bonuses(&equipment);
        let mut speed = weapon
            .as_ref()
            .map_or(4, |item| weapons::number(item, "attack_speed", 4));
        let (attack, strength, impact_delay) = if let Some(ranged_weapon) = range_weapon {
            if matches!(animation.id(), 422 | 423) {
                animation = Animation::new(ranged_weapon.animation);
            }
            if !ranged_weapon.degrading {
                let Some(ammo) = equipment.get(ranged_weapon.ammo_slot) else {
                    message(p, player, "You do not have enough ammo left.");
                    return cancel(p, player);
                };
                let ammo = ammo.read();
                if !ranged_weapon.ammunition.contains(&ammo.id()) {
                    message(
                        p,
                        player,
                        "You can't use this type of ammunition with this weapon.",
                    );
                    return cancel(p, player);
                }
                let count = if ranged_weapon.double_shot && ammo.amount() >= 2 {
                    2
                } else {
                    1
                };
                fired = Some((
                    ammo.id(),
                    ranged_weapon.ammo_slot,
                    count,
                    ranged_weapon.drop_ammo,
                ));
            }
            if ranged_weapon.ammo_slot != equipment::ARROWS
                && let Some(arrows) = equipment.get(equipment::ARROWS)
                && let Some(rs09_server::definitions::property::Property::IntArray(values)) =
                    weapons::property(&arrows, "bonuses")
            {
                bonuses[14] -= values.read().get(14).copied().unwrap_or(0);
            }
            speed -= i32::from(style.stance == Stance::Rapid);
            let [x, y, _] = from.coordinates();
            let [tx, ty, _] = to.coordinates();
            let travel = (f64::from(x - tx).hypot(f64::from(y - ty)) * 0.3).ceil() as i32;
            (ranged, ranged, travel.max(1))
        } else {
            (attack, strength, 1)
        };
        let graphic = fired.and_then(|(id, _, _, _)| weapons::ammo_graphic(id));
        visualize_attack(p, player, animation, graphic)?;
        let melee = p.melee.as_mut().unwrap();
        let accuracy = (attack + 8 + style.attack_level()) * (bonuses[style.bonus] + 64);
        let defence = (monster.stats[1] + 9) * (monster.bonuses[style.bonus + 5] + 64);
        let strength_bonus = if range_weapon.is_some() { 14 } else { 11 };
        let power = (strength + 8 + style.strength_level()) * (bonuses[strength_bonus] + 64);
        for offset in 0..fired.map_or(1, |(_, _, amount, _)| amount) {
            let damage = roll(&mut melee.rng, accuracy, defence, power);
            melee.impacts.entry(player).or_default().push(PlayerImpact {
                due: now.wrapping_add(impact_delay + offset),
                damage,
                style: style.stance,
            });
        }
        melee
            .cooldowns
            .insert(player, now.wrapping_add(speed.max(1)));
        let recover = fired.is_some_and(|(_, _, _, drop)| drop)
            && melee
                .rng
                .next_bound((5.0 * (1.0 + f64::from(ranged) * 0.01)) as i32)
                .unwrap_or(0)
                != 0;
        if let Some((id, ammo_slot, amount, _)) = fired {
            if let Some(missile) = weapons::ammo_missile(id) {
                projectile(p, &from, &to, engagement.npc, missile, 5.0)?;
                if amount == 2 {
                    let other = weapons::Missile {
                        heights: [40, 36],
                        start: 41,
                        speed: 55,
                        angle: 25,
                        distance: 32,
                        ..missile
                    };
                    projectile(p, &from, &to, engagement.npc, other, 10.0)?;
                }
            }
            ui::consume_ammo(p, player, ammo_slot, amount).map_err(|e| e.to_string())?;
            if recover {
                let owner = p.state.retain_node(player).map_err(|e| e.to_string())?;
                let near_water = p
                    .host
                    .scene
                    .context(&mut p.host.maps)
                    .clipping(to.clone(), false)
                    .map_err(|e| e.to_string())?
                    & 0x200000
                    != 0;
                if !near_water {
                    super::ground_services::recover_ammunition(
                        &mut p.state,
                        &mut p.host,
                        &owner,
                        id,
                        amount,
                        to.clone(),
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
        let melee = p.melee.as_mut().unwrap();
        if monster.can_retaliate && !melee.retaliation.contains_key(&engagement.npc) {
            melee.retaliation.insert(
                engagement.npc,
                Retaliation {
                    victim: player,
                    next_swing: now.wrapping_add(2),
                    impact: None,
                },
            );
            let index = p.state.actor(player).map_err(|e| e.to_string())?.wire_index;
            p.state
                .actor_mut(engagement.npc)
                .map_err(|e| e.to_string())?
                .view
                .combat_running = true;
            npc::face_player(p, engagement.npc, index);
        }
    }
    let can_counter = in_reach(p, &from, &to, size, 1)?;
    if !can_counter
        && monster.can_retaliate
        && p.melee
            .as_ref()
            .unwrap()
            .retaliation
            .get(&engagement.npc)
            .is_some_and(|r| r.victim == player)
    {
        let queue = &p
            .state
            .actor(engagement.npc)
            .map_err(|e| e.to_string())?
            .queue;
        if !queue.has_path() {
            let mut world = p.state.context(&mut p.host);
            let path = world
                .find_node_path(engagement.npc, player)
                .map_err(|e| e.to_string())?;
            if !path.points.is_empty() {
                world
                    .walk_path(engagement.npc, path)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    let melee = p.melee.as_mut().unwrap();
    if monster.can_retaliate
        && can_counter
        && let Some(retaliation) = melee
            .retaliation
            .get_mut(&engagement.npc)
            .filter(|r| r.victim == player && ready(now, r.next_swing))
    {
        let damage = roll(
            &mut melee.rng,
            (monster.stats[0] + 9) * (monster.bonuses[2] + 64),
            (defence + 8 + style.defence_level()) * (weapons::bonuses(&equipment)[7] + 64),
            (monster.stats[2] + 9) * (monster.bonuses[11] + 64),
        );
        retaliation.next_swing = now.wrapping_add(monster.speed);
        retaliation.impact = Some((now.wrapping_add(1), damage));
        npc::attack_animation(p, engagement.npc);
    }
    Ok(())
}

fn receive_hit(
    p: &mut Probe,
    player: Node,
    rolled: i32,
    hp: i32,
    animation: Animation,
) -> Result<bool, String> {
    animate_player(p, player, animation)?;
    let (damage, ratio) = ui::melee_damage(p, player, rolled).map_err(|e| e.to_string())?;
    p.host.masks[&player]
        .melee_hit(damage, ratio)
        .map_err(|e| e.to_string())?;
    if damage < hp {
        return Ok(false);
    }
    cancel(p, player)?;
    let player_state = p.state.actor(player).map_err(|e| e.to_string())?;
    player_state.queue.points().write().clear();
    player_state.queue.state.write().entity.teleport_location =
        Some(Location::new(p.spawn[0], p.spawn[1], p.spawn[2]));
    ui::revive_melee_player(p, player).map_err(|e| e.to_string())?;
    message(p, player, "Oh dear, you are dead!");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::{
        buffer::{Reader, Writer},
        game::{npcs, players, updates::ServerUpdate},
    };
    use rs09_server::{
        definitions::{item, item_config},
        inventory::item::Item,
        player::details::ClientInfo,
        world::update::Context as _,
    };

    #[test]
    fn actual_attack_option_sends_both_swings_and_hits_and_awards_xp() {
        for (style, animation, skill) in [(0, 422, 0), (1, 423, 2), (2, 422, 1)] {
            exchange(None, 92, style + 2, style, animation, 4, &[(skill, 8)]);
        }
    }

    #[test]
    fn equipped_weapons_use_pinned_buttons_animations_cooldowns_and_experience() {
        exchange(Some(1351), 75, 4, 2, 401, 5, &[(2, 8)]);
        exchange(Some(1307), 82, 4, 2, 7048, 7, &[(2, 8)]);
        exchange(Some(1321), 81, 4, 2, 381, 4, &[(0, 2), (1, 2), (2, 2)]);
        exchange(Some(841), 77, 4, 1, 426, 3, &[(4, 8)]);
    }

    fn equipment_definitions() -> std::sync::Arc<item::Registry> {
        // The applicable configuration fields from the pinned item rows.
        let mut definitions = item::Registry::default();
        for id in [1351, 1307, 1321, 841, 884, 1171] {
            definitions.insert(
                id,
                item::Shared::new(item::ItemDefinition {
                    id,
                    ..Default::default()
                }),
            );
        }
        struct Log;
        impl item_config::Logger for Log {
            fn parsed(&mut self, _: i32) -> item_config::Result<()> {
                Ok(())
            }
        }
        item_config::Parser::default().load_text(r#"[
          {"id":"1351","name":"Bronze axe","equipment_slot":"3","weapon_interface":"2","attack_speed":"5","attack_anims":"395,395,401,395","defence_anim":"397","bonuses":"-2,4,2,0,0,0,1,0,0,0,0,5,0,0,0"},
          {"id":"1307","name":"Bronze 2h sword","equipment_slot":"3","weapon_interface":"7","two_handed":"true","attack_speed":"7","attack_anims":"7041,7041,7048,7049","defence_anim":"7050","bonuses":"-4,9,8,-4,0,0,0,0,0,-1,0,10,0,0,0"},
          {"id":"1321","name":"Bronze scimitar","equipment_slot":"3","weapon_interface":"6","attack_speed":"4","attack_anims":"390,390,381,390","defence_anim":"397","bonuses":"1,7,-2,0,0,0,1,0,0,0,0,6,0,0,0"},
          {"id":"841","name":"Shortbow","equipment_slot":"3","weapon_interface":"16","two_handed":"true","attack_speed":"4","attack_anims":"426,426,426,426","defence_anim":"424","bonuses":"0,0,0,0,8,0,0,0,0,0,0,0,0,0,0"},
          {"id":"884","name":"Iron arrow","equipment_slot":"13","bonuses":"0,0,0,0,0,0,0,0,0,0,0,0,0,0,10"},
          {"id":"1171","name":"Wooden shield","equipment_slot":"5"}
        ]"#, &definitions, &mut Log).unwrap();
        for id in [1351, 1307, 1321, 841, 884, 1171] {
            definitions.for_id(id).write().options =
                Some(item::Shared::new(vec![None, Some("Wield".into())]));
        }
        definitions.for_id(884).write().stackable = true;
        std::sync::Arc::new(definitions)
    }

    fn equip(p: &mut Probe, player: Node, slot: i32, id: i32, amount: i32) {
        let owner = p.state.retain_node(player).unwrap();
        let fields = p.state.player_fields(&owner).unwrap();
        let mut item = Item::new(id, amount, 1000, &fields.definitions());
        item.index = slot;
        let slot_index = slot as usize;
        fields.inventory().items().write()[slot_index] = Some(item::Shared::new(item));
        let mut wire = Writer::new();
        wire.ip2(id);
        wire.p2add(slot);
        wire.imp4(149 << 16);
        let click = ClientAction::decode(55, &mut Reader::new(&wire.into_vec()))
            .unwrap()
            .unwrap();
        assert!(
            ui::process(p, player, &click).unwrap(),
            "equip {id}: {click:?}"
        );
    }

    fn exchange(
        weapon: Option<i32>,
        group: i32,
        child: i32,
        style: i32,
        animation: i32,
        speed: i32,
        awards: &[(i32, i32)],
    ) {
        let (mut p, player, _) = super::super::tests::probe();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        p.host.ui =
            Some(ui::Ui::load(&root.join("fixtures/interfaces/interface_configs.json")).unwrap());
        if weapon.is_some() {
            p.host
                .ui
                .as_mut()
                .unwrap()
                .use_definitions(equipment_definitions())
                .unwrap();
        }
        ui::admit(
            &mut p,
            player,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1024,
                screen_height: 768,
            },
        )
        .unwrap();
        let [rat, civilian] = npc::combat_test_targets(&mut p);
        p.host
            .scene
            .context(&mut p.host.maps)
            .load((50 << 8) | 50, None)
            .unwrap()
            .write()
            .has_flags = true;
        for x in 3207..=3209 {
            for y in 3215..=3217 {
                let tile = Location::new(x, y, 0);
                let chunk = p.host.scene.chunk(&tile).unwrap();
                let (walking, projectile) = {
                    let c = chunk.read();
                    (c.flags.clone(), c.projectile_flags.clone())
                };
                let mut world = p.host.scene.context(&mut p.host.maps);
                walking.clear(&mut world, x & 7, y & 7).unwrap();
                projectile.clear(&mut world, x & 7, y & 7).unwrap();
            }
        }
        p.state.ticks = 10;
        p.state.actor_mut(rat).unwrap().view.active = true;
        p.state.actor_mut(civilian).unwrap().view.active = true;
        p.state.actor_mut(player).unwrap().view.active = true;
        p.actors[&1].player.write().active = true;
        p.melee = Some(Melee::new(530));
        if let Some(id) = weapon {
            ui::experience(&mut p, player, 3, 30_000.0).unwrap();
            ui::revive_melee_player(&mut p, player).unwrap();
            if id == 1307 {
                equip(&mut p, player, 1, 1171, 1);
            }
            equip(&mut p, player, 0, id, 1);
            let gear = ui::equipment(&p, player).unwrap();
            assert_eq!(gear.get(equipment::WEAPON).unwrap().read().id(), id);
            assert!(gear.get(equipment::SHIELD).is_none());
            if id == 1307 {
                let owner = p.state.retain_node(player).unwrap();
                assert!(
                    p.state
                        .player_fields(&owner)
                        .unwrap()
                        .inventory()
                        .contains(1171, 1)
                );
            }
            assert!(p.host.frames[&player].iter().any(|frame| matches!(
                ServerUpdate::decode(frame),
                Ok(Some(ServerUpdate::OpenInterface { interface, .. })) if interface == group
            )));
            if id == 841 {
                equip(&mut p, player, 1, 884, 32);
            }
        }
        let mut button = Writer::new();
        button.p4((group << 16) | child);
        button.p2(-1);
        let action = ClientAction::decode(155, &mut Reader::new(&button.into_vec()))
            .unwrap()
            .unwrap();
        assert!(ui::process(&mut p, player, &action).unwrap());
        assert!(p.host.frames[&player].contains(&rs09_protocol::game::updates::varp(43, style)));

        let start = if weapon == Some(841) {
            Location::new(3207, 3216, 0)
        } else if style == 2 {
            Location::new(3208, 3216, 0)
        } else {
            Location::new(3207, 3215, 0)
        };
        let [start_x, start_y, _] = start.coordinates();
        {
            let mut state = p.state.actor(player).unwrap().queue.state.write();
            state.entity.location = Some(start.clone());
            state.entity.update_scene = false;
        }
        let viewer = p.actors[&1].player.clone();
        viewer.write().location = Some(start);
        npc::render(&mut p, player, &viewer).unwrap();
        let first = p.host.frames[&player].last().unwrap().clone();
        let mut wire = npcs::State::default();
        wire.decode(&first, [start_x, start_y], 10).unwrap();
        assert_eq!(wire.actors[&1].type_id, 41);
        assert_eq!(wire.actors[&2].type_id, 278);

        // Normal option two packet, including its little-endian Add encoding.
        let receive = |index: u8| {
            ClientAction::decode(3, &mut Reader::new(&[index + 128, 0]))
                .unwrap()
                .unwrap()
        };
        assert!(!process(&mut p, 1, player, &receive(2)).unwrap());
        assert!(p.melee.as_ref().unwrap().players.is_empty());
        assert!(process(&mut p, 1, player, &receive(1)).unwrap());
        let mut player_wire = players::State::new(start_x - 3152, start_y - 3168);
        let mut first_swing = None;
        let mut npc_swing = None;
        let mut incoming = None;
        let mut seen_hit = false;
        let mut dead = false;
        for time in 10..220 {
            p.state.ticks = time;
            p.player_tick(player).unwrap();
            if time == 10 {
                let [x, y, _] = p
                    .state
                    .node_location(player)
                    .unwrap()
                    .unwrap()
                    .coordinates();
                assert_eq!(
                    (x - 3208).abs() + (y - 3216).abs(),
                    1,
                    "first movement tick"
                );
                assert_eq!(p.melee.as_ref().unwrap().cooldowns[&player], 10 + speed);
                if weapon == Some(841) {
                    assert_eq!(
                        ui::equipment(&p, player)
                            .unwrap()
                            .get(equipment::ARROWS)
                            .unwrap()
                            .read()
                            .amount(),
                        31
                    );
                }
            }
            let prior = p.host.frames[&player].len();
            p.player_update(player).unwrap();
            for frame in &p.host.frames[&player][prior..] {
                if frame.opcode == 225 {
                    let update = player_wire.decode_frame(frame, time).unwrap();
                    for mask in update.extended.iter().filter(|e| e.index == players::SELF) {
                        for field in &mask.fields {
                            match field {
                                players::Field::Animation { id, .. } if *id == animation => {
                                    first_swing.get_or_insert(time);
                                }
                                players::Field::Hit { bar: Some(_), .. } => {
                                    incoming.get_or_insert(time);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            let frames = &p.host.frames[&player][prior..];
            wire.decode(
                frames.iter().find(|frame| frame.opcode == 32).unwrap(),
                [3207, 3216],
                time,
            )
            .unwrap();
            if weapon == Some(841) && time == 10 {
                assert!(frames.iter().any(|frame| matches!(
                    ServerUpdate::decode(frame),
                    Ok(Some(ServerUpdate::ZoneEnclosed { frames, .. }))
                        if frames.iter().any(|frame| Projectile::decode(frame).is_ok())
                )));
            }
            if wire.actors[&1].last_mask.animation == Some([5387, 0]) {
                npc_swing.get_or_insert(time);
            }
            if let Some(hit) = &wire.actors[&1].last_mask.primary {
                if hit.damage > 0 {
                    seen_hit = true;
                }
                if hit.ratio == Some(0) {
                    dead = true;
                    break;
                }
            }
            p.player_reset(player).unwrap();
            p.npc_reset(rat).unwrap();
        }
        assert_eq!(first_swing, Some(10));
        assert_eq!(npc_swing, Some(12));
        assert_eq!(incoming, Some(13));
        assert!(
            seen_hit && dead,
            "weapon {weapon:?}: saw hit {seen_hit}, dead {dead}, monster HP {:?}, player {:?}",
            npc::fighter(&p, rat).map(|f| f.life),
            ui::melee_stats(&p, player)
        );
        assert!(npc::fighter(&p, rat).is_none());
        assert!(p.melee.as_ref().unwrap().players.is_empty());
        for &(skill, experience) in awards {
            assert!(
                p.host.frames[&player].iter().any(|frame| matches!(
                    ServerUpdate::decode(frame),
                    Ok(Some(ServerUpdate::Skill { skill: trained, experience: xp, .. }))
                        if trained == skill && xp == experience
                )),
                "weapon {weapon:?}, skill {skill}, XP {experience}"
            );
        }
        if let Some(id) = weapon {
            let mut packet = Writer::new();
            packet.p2add(equipment::WEAPON);
            packet.p2(id);
            packet.imp4((387 << 16) | 28);
            let remove = ClientAction::decode(81, &mut Reader::new(&packet.into_vec()))
                .unwrap()
                .unwrap();
            assert!(ui::process(&mut p, player, &remove).unwrap());
            let owner = p.state.retain_node(player).unwrap();
            let fields = p.state.player_fields(&owner).unwrap();
            assert!(fields.equipment().get(equipment::WEAPON).is_none());
            assert!(fields.inventory().contains(id, 1));
        }
        assert!(p.host.errors.is_empty(), "{:?}", p.host.errors);
    }
}
