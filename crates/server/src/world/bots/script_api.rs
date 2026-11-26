//! `ScriptAPI.kt`: the helpers bot scripts call. Each is the original's
//! sequence of calls; what they call (pathfinding and movement pulses,
//! interaction listeners and plugins, containers, the combat pulse, ground
//! items, the Grand Exchange, consumables) is a [`Host`] service.
//!
//! Kept as the original has it: `useWith` casts its node to `Scenery`
//! (anything else fails after the inventory lookup); `getNearestGameObject`
//! never updates its minimum and answers the last object with the id;
//! `evaluateViability` refuses a node farther than the best so far *or* the
//! path-finding limit; `getNearestNode(name, isObject)` ignores `isObject`;
//! the news of `sellAllOnGeAdv` skips the bank removal for six log ids;
//! `buyFromGE` waits on a latch its own pulse releases.
use super::{
    BotOffer, Error, Finder, Food, Handler, Host, IntType, Item, Location, NodeKind, Pulse, Result,
    Used, Vector, Which, loc, repository, skill,
};
use crate::definitions::item::Shared;
use crate::world::content_api::{api_files::colorize, java_double_to_int};
use rs09_game_core::{java_json::Json, java_lower::lowercase_root, java_string};

/// `GRAPHICSUP`, `ANIMATIONUP`, `GRAPHICSDOWN`, `ANIMATIONDOWN`.
pub const GRAPHICS_UP: i32 = 1576;
pub const ANIMATION_UP: i32 = 8939;
pub const GRAPHICS_DOWN: i32 = 1577;
pub const ANIMATION_DOWN: i32 = 8941;

/// A node test of `getNearestObjectByPredicate`.
pub type Predicate<'a, H> = &'a mut dyn FnMut(&mut H, &<H as Host>::Node) -> Result<bool>;

/// What `evaluateViability` and `processEvaluationList` accept: the three
/// optional parameters of the original. The id always applies; a predicate
/// replaces the names.
pub struct Accept<'a, H: Host> {
    /// `acceptedNames`: any of the names, ignoring case; `None` accepts all.
    pub names: Option<&'a [String]>,
    /// `acceptedId`; -1 accepts all.
    pub id: i32,
    /// `acceptedPredicate`.
    pub predicate: Option<Predicate<'a, H>>,
}
impl<'a, H: Host> Accept<'a, H> {
    pub fn names(names: &'a [String]) -> Self {
        Self {
            names: Some(names),
            id: -1,
            predicate: None,
        }
    }
    pub fn id(id: i32) -> Self {
        Self {
            names: None,
            id,
            predicate: None,
        }
    }
    pub fn predicate(predicate: Predicate<'a, H>) -> Self {
        Self {
            names: None,
            id: -1,
            predicate: Some(predicate),
        }
    }
}

/// `Item(id).noteChange`: the definition's note id unless it is negative.
pub fn note_change<H: Host>(host: &mut H, id: i32) -> Result<i32> {
    let note_id = host.item_note_id(id)?;
    Ok(if note_id > -1 {
        note_id
    } else {
        Item::one(id).id
    })
}

/// A `ScriptAPI` object: its bot.
pub struct ScriptApi<H: Host> {
    pub bot: H::Node,
}
impl<H: Host> Clone for ScriptApi<H> {
    fn clone(&self) -> Self {
        Self {
            bot: self.bot.clone(),
        }
    }
}

/// `distance(n1, n2)` over the two nodes' locations.
pub fn distance(first: Location, second: Location) -> f64 {
    let dx = f64::from(first.x()) - f64::from(second.x());
    let dy = f64::from(second.y()) - f64::from(first.y());
    (dx * dx + dy * dy).sqrt()
}

fn int_type(kind: NodeKind) -> Option<IntType> {
    match kind {
        NodeKind::Scenery => Some(IntType::Scenery),
        NodeKind::Npc => Some(IntType::Npc),
        NodeKind::Item => Some(IntType::Item),
        NodeKind::Player | NodeKind::Other => None,
    }
}

impl<H: Host> ScriptApi<H> {
    /// `ScriptAPI(bot)`.
    pub fn new(bot: H::Node) -> Self {
        Self { bot }
    }

    /// `distance(n1, n2)`.
    pub fn distance(&self, host: &mut H, n1: &H::Node, n2: &H::Node) -> Result<f64> {
        Ok(distance(loc(host, n1)?, loc(host, n2)?))
    }

    /// `interact(bot, node, option)`.
    pub fn interact(
        &self,
        host: &mut H,
        bot: &H::Node,
        node: Option<&H::Node>,
        option: &str,
    ) -> Result<()> {
        let Some(node) = node else {
            return Ok(());
        };
        let Some(kind) = int_type(host.kind(node)) else {
            return Ok(());
        };
        if !host.interaction(node)? {
            return Err(Error::Null);
        }
        let mut chosen = None;
        for candidate in host.interaction_options(node)?.into_iter().flatten() {
            let named = host
                .option_name(&candidate)
                .is_some_and(|name| java_string::equals_ignore_case(&name, option));
            if named && chosen.is_none() {
                chosen = Some(candidate);
            }
        }
        let Some(chosen) = chosen else {
            return host.log_warn(&format!("Invalid option name provided: {option}"));
        };
        let id = host.id(node);
        if !host.listeners_run(id, kind, option, bot, node)? {
            if !host.interaction(node)? {
                return Err(Error::Null);
            }
            host.interaction_handle(node, bot, Some(&chosen))?;
        }
        Ok(())
    }

    /// `useWith(bot, itemId, node)`.
    pub fn use_with(
        &self,
        host: &mut H,
        bot: &H::Node,
        item_id: i32,
        node: Option<&H::Node>,
    ) -> Result<()> {
        let Some(node) = node else {
            return Ok(());
        };
        let node_kind = host.kind(node);
        let Some(kind) = int_type(node_kind) else {
            return Ok(());
        };
        let item = host.container_get_item(bot, Which::Inventory, Item::one(item_id))?;
        // `node.asScenery()` is a cast.
        if node_kind != NodeKind::Scenery {
            return Err(Error::Cast);
        }
        let child = host.scenery_child(node, bot)?;
        // The listeners' `used` parameter is not nullable.
        if item.is_none() {
            return Err(Error::Null);
        }
        if host.listeners_run_use(item, node, kind, bot)? {
            return Ok(());
        }
        if let Some(child) = &child
            && host.id(child) != host.id(node)
            && host.listeners_run_use(item, child, kind, bot)?
        {
            return Ok(());
        }
        let flipped = kind == IntType::Item && item.ok_or(Error::Null)?.id < host.id(node);
        let (used, with) = if flipped {
            (Used::Node(node.clone()), Used::Item(item))
        } else {
            (
                Used::Item(item),
                Used::Node(child.unwrap_or_else(|| node.clone())),
            )
        };
        if host.plugin_interaction_handle(bot, &used, &with)? {
            return Ok(());
        }
        host.use_with_run(bot, &used, &with)
    }

    /// `sendChat(message)`.
    pub fn send_chat(&self, host: &mut H, message: &str) -> Result<()> {
        host.send_chat(&self.bot, message)?;
        host.register_chat_mask(&self.bot, message)
    }

    /// The node list of the bot's region and plane.
    fn region_list(&self, host: &mut H, objects: bool) -> Result<Vec<Option<H::Node>>> {
        let region_id = loc(host, &self.bot)?.region_id();
        let region = host.region_for_id(region_id)?;
        let z = loc(host, &self.bot)?.z();
        if objects {
            host.region_object_list(&region, z)
        } else {
            host.region_node_list(&region, z)
        }
    }

    /// `getNearestNodeFromList(acceptedNames, isObject)`.
    pub fn get_nearest_node_from_list(
        &self,
        host: &mut H,
        accepted_names: &[String],
        is_object: bool,
    ) -> Result<Option<H::Node>> {
        let list = self.region_list(host, is_object)?;
        self.process_evaluation_list(host, &list, Accept::names(accepted_names))
    }

    /// `getNearestNode(id, isObject)`.
    pub fn get_nearest_node_by_id(
        &self,
        host: &mut H,
        id: i32,
        is_object: bool,
    ) -> Result<Option<H::Node>> {
        let list = self.region_list(host, is_object)?;
        self.process_evaluation_list(host, &list, Accept::id(id))
    }

    /// `getNearestNode(entityName)` and `getNearestNode(name, isObject)`:
    /// both search the node list, whatever `isObject` says.
    pub fn get_nearest_node(&self, host: &mut H, name: &str) -> Result<Option<H::Node>> {
        let list = self.region_list(host, false)?;
        self.process_evaluation_list(host, &list, Accept::names(&[name.to_owned()]))
    }

    /// `getNearestObjectByPredicate(predicate)`.
    pub fn get_nearest_object_by_predicate(
        &self,
        host: &mut H,
        predicate: Predicate<'_, H>,
    ) -> Result<Option<H::Node>> {
        let list = self.region_list(host, true)?;
        self.process_evaluation_list(host, &list, Accept::predicate(predicate))
    }

    /// `evaluateViability(e, minDistance, maxDistance, ..)`.
    pub fn evaluate_viability(
        &self,
        host: &mut H,
        e: Option<&H::Node>,
        min_distance: f64,
        max_distance: f64,
        accept: &mut Accept<'_, H>,
    ) -> Result<bool> {
        let Some(e) = e else {
            return Ok(false);
        };
        if !host.is_active(e) {
            return Ok(false);
        }
        if accept.id != -1 && host.id(e) != accept.id {
            return Ok(false);
        }
        let dist = self.distance(host, &self.bot.clone(), e)?;
        if dist > max_distance || dist > min_distance {
            return Ok(false);
        }
        let accepted = match (&mut accept.predicate, accept.names) {
            (Some(predicate), _) => predicate(host, e)?,
            (None, Some(names)) => {
                let name = host.name(e);
                names.iter().any(|accepted| {
                    name.as_deref()
                        .is_some_and(|name| java_string::equals_ignore_case(accepted, name))
                })
            }
            (None, None) => true,
        };
        if !accepted {
            return Ok(false);
        }
        let path =
            host.pathfinder_find(&self.bot, &super::Dest::Node(e.clone()), Finder::Default)?;
        Ok(!host.path_is_move_near(&path))
    }

    /// `processEvaluationList(list, ..)`: the viable node nearest the bot.
    pub fn process_evaluation_list(
        &self,
        host: &mut H,
        list: &[Option<H::Node>],
        mut accept: Accept<'_, H>,
    ) -> Result<Option<H::Node>> {
        let mut entity = None;
        let mut min_distance = f64::MAX;
        let max_distance = f64::from(host.max_pathfind_distance());
        for e in list {
            if self.evaluate_viability(host, e.as_ref(), min_distance, max_distance, &mut accept)? {
                let e = e.as_ref().ok_or(Error::Null)?;
                entity = Some(e.clone());
                min_distance = self.distance(host, &self.bot.clone(), e)?;
            }
        }
        Ok(entity)
    }

    /// The private `getNearestGroundItem(id)`.
    fn nearest_ground_item(&self, host: &mut H, id: i32) -> Result<Option<H::Node>> {
        let mut closest: Option<H::Node> = None;
        if repository::get_items(host, &self.bot).is_some() {
            let items = repository::get_items(host, &self.bot).ok_or(Error::Null)?;
            let mut near = Vec::new();
            for item in items {
                if loc(host, &item)?.distance(loc(host, &self.bot)?) < 10.0 {
                    near.push(item);
                }
            }
            for item in near {
                if host.id(&item) == id {
                    closest = Some(item);
                }
            }
            if !host.ground_items_contain(closest.as_ref())? {
                repository::remove_item(host, &self.bot, closest.as_ref());
                return Ok(None);
            }
        } else if let Some(mut items) = host.botting_drops(&self.bot)? {
            let mut near = Vec::new();
            for item in &items {
                if loc(host, item)?.distance(loc(host, &self.bot)?) < 10.0 {
                    near.push(item.clone());
                }
            }
            for item in near {
                if host.id(&item) == id {
                    if let Some(at) = items.iter().position(|held| *held == item) {
                        items.remove(at);
                    }
                    host.set_botting_drops(&self.bot, &items)?;
                    return Ok(Some(item));
                }
            }
        }
        Ok(closest)
    }

    /// `takeNearestGroundItem(id)`.
    pub fn take_nearest_ground_item(&self, host: &mut H, id: i32) -> Result<bool> {
        let Some(item) = self.nearest_ground_item(host, id)? else {
            return Ok(false);
        };
        // `item.interaction?.handle(bot, item.interaction[2])`
        if host.interaction(&item)? {
            if !host.interaction(&item)? {
                return Err(Error::Null);
            }
            let option = host.interaction_get(&item, 2)?;
            host.interaction_handle(&item, &self.bot, option.as_ref())?;
        }
        Ok(true)
    }

    /// `getNearestGameObject(loc, objectId)`: `minDistance` is never
    /// lowered, so this is the last listed object with the id.
    pub fn get_nearest_game_object(
        &self,
        host: &mut H,
        location: Location,
        object_id: i32,
    ) -> Result<Option<H::Node>> {
        let mut nearest = None;
        let min_distance = f64::MAX;
        let region = host.region_for_id(location.region_id())?;
        for object in host
            .region_object_list(&region, location.z())?
            .into_iter()
            .flatten()
        {
            if distance(location, loc(host, &object)?) < min_distance
                && host.id(&object) == object_id
            {
                nearest = Some(object);
            }
        }
        Ok(nearest)
    }

    /// The private `findTargets(entity, radius, name)`: of the first five
    /// local NPCs, the valid ones; `None` when there is none.
    fn find_targets(
        &self,
        host: &mut H,
        entity: &H::Node,
        radius: i32,
        name: Option<&str>,
    ) -> Result<Option<Vec<H::Node>>> {
        let mut targets = Vec::new();
        let location = host.location(entity);
        let local = host.local_npcs(location, radius)?;
        for npc in local.iter().take(5) {
            if host.kind(npc) != NodeKind::Npc {
                return Err(Error::Cast);
            }
            if self.check_valid_targets(host, npc, name)? {
                targets.push(npc.clone());
            }
        }
        Ok(if targets.is_empty() {
            None
        } else {
            Some(targets)
        })
    }

    /// The private `checkValidTargets(target, name)`.
    fn check_valid_targets(
        &self,
        host: &mut H,
        target: &H::Node,
        name: Option<&str>,
    ) -> Result<bool> {
        if !host.is_active(target) {
            return Ok(false);
        }
        if !host.properties_is_multi_zone(target)? && host.in_combat(target)? {
            return Ok(false);
        }
        if let Some(name) = name
            && host.name(target).as_deref() != Some(name)
        {
            return Ok(false);
        }
        host.npc_has_action(target, "attack")
    }

    /// `attackNpcsInRadius(bot, radius)` (`name` none) and
    /// `attackNpcInRadius(bot, name, radius)`.
    pub fn attack_npcs_in_radius(
        &self,
        host: &mut H,
        bot: &H::Node,
        name: Option<&str>,
        radius: i32,
    ) -> Result<bool> {
        if host.in_combat(bot)? {
            return Ok(true);
        }
        let Some(creatures) = self.find_targets(host, bot, radius, name)? else {
            return Ok(false);
        };
        let index = host.get_random(creatures.len() as i32 - 1)?;
        let target = usize::try_from(index)
            .ok()
            .and_then(|i| creatures.get(i))
            .ok_or(Error::Index)?;
        host.attack(bot, target)?;
        // The list is never empty here; the original's second search is
        // unreachable.
        Ok(true)
    }

    /// `walkTo(loc)`.
    pub fn walk_to(&self, host: &mut H, location: Location) -> Result<()> {
        if !host.walking_queue_is_moving(&self.bot)? {
            self.walk_to_iterator(host, location)?;
        }
        Ok(())
    }

    /// `walkArray(steps)`.
    pub fn walk_array(&self, host: &mut H, steps: &[Location]) -> Result<()> {
        host.pulse_manager_run(
            &self.bot,
            Pulse::WalkArray(WalkArray {
                api: self.clone(),
                steps: steps.to_vec(),
                step_index: Shared::new(0),
            }),
            false,
        )
    }

    /// `randomWalkTo(loc, radius)`.
    pub fn random_walk_to(&self, host: &mut H, location: Location, radius: i32) -> Result<()> {
        if !host.walking_queue_is_moving(&self.bot)? {
            let dx = host.random_between(radius, radius.wrapping_neg())?;
            let dy = host.random_between(radius, radius.wrapping_neg())?;
            self.walk_to_iterator(host, location.transform(dx, dy, 0))?;
        }
        Ok(())
    }

    /// `randomizeLocationInRanges(location, xMin, xMax, yMin, yMax, staticZ)`:
    /// `kotlin.random.Random.nextInt(from, until)` refuses an empty range.
    #[allow(clippy::too_many_arguments)]
    pub fn randomize_location_in_ranges(
        &self,
        host: &mut H,
        location: Location,
        x_min: i32,
        x_max: i32,
        y_min: i32,
        y_max: i32,
        static_z: i32,
    ) -> Result<Location> {
        if x_max <= x_min {
            return Err(Error::Argument);
        }
        let new_x = location
            .x()
            .wrapping_add(host.kotlin_next_int(x_min, x_max)?);
        if y_max <= y_min {
            return Err(Error::Argument);
        }
        let new_y = location
            .y()
            .wrapping_add(host.kotlin_next_int(y_min, y_max)?);
        Ok(Location::new(new_x, new_y, static_z))
    }

    /// The private `walkToIterator(loc)`: a movement pulse towards the
    /// tile at most `MAX_PATHFIND_DISTANCE - 1` tiles along the line.
    fn walk_to_iterator(&self, host: &mut H, location: Location) -> Result<()> {
        // `loc.x - bot.location.x` and `loc.y - bot.location.y` are read
        // and dropped.
        loc(host, &self.bot)?;
        loc(host, &self.bot)?;
        let vec = Vector::between_locs(loc(host, &self.bot)?, location);
        let norm = vec.normalized();
        let tiles = java_double_to_int(vec.magnitude().floor())
            .min(host.max_pathfind_distance().wrapping_sub(1));
        let destination = loc(host, &self.bot)?.transform_vector(norm.times_int(tiles));
        host.pulse_manager_run(
            &self.bot,
            Pulse::Walk {
                bot: self.bot.clone(),
                destination,
            },
            false,
        )
    }

    /// `teleportToGE()`.
    pub fn teleport_to_ge(&self, host: &mut H) -> Result<bool> {
        self.teleport_with(host, Location::new(3165, 3482, 0), true)
    }

    /// `teleport(loc)`.
    pub fn teleport(&self, host: &mut H, location: Location) -> Result<bool> {
        self.teleport_with(host, location, false)
    }

    /// The two teleports differ in where the pulse goes: the bot's pulse
    /// manager for the Grand Exchange, the world pulser otherwise.
    fn teleport_with(&self, host: &mut H, location: Location, to_ge: bool) -> Result<bool> {
        let bot = &self.bot;
        if host.is_tele_blocked(bot)? {
            return Ok(false);
        }
        host.lock(bot)?;
        host.visualize(bot, ANIMATION_UP, GRAPHICS_UP)?;
        host.impact_set_disabled_ticks(bot, 4)?;
        let pulse = Pulse::Teleport {
            bot: bot.clone(),
            location,
            to_ge,
        };
        if to_ge {
            host.pulse_manager_run(bot, pulse, false)?;
        } else {
            host.pulser_submit(pulse)?;
        }
        Ok(true)
    }

    /// `sellOnGE(id)`.
    pub fn sell_on_ge(&self, host: &mut H, id: i32) -> Result<()> {
        host.pulse_manager_run(
            &self.bot,
            Pulse::SellOnGe {
                api: self.clone(),
                id,
            },
            false,
        )
    }

    /// `sellAllOnGe()`.
    pub fn sell_all_on_ge(&self, host: &mut H) -> Result<()> {
        host.pulse_manager_run(&self.bot, Pulse::SellAll { api: self.clone() }, false)
    }

    /// `sellAllOnGeAdv()`: nothing without a "Desk" nearby.
    pub fn sell_all_on_ge_adv(&self, host: &mut H) -> Result<()> {
        let Some(desk) = self.nearest_scenery(host, "Desk")? else {
            return Ok(());
        };
        host.pulse_manager_run(
            &self.bot,
            Pulse::SellAllAdv {
                api: self.clone(),
                desk,
            },
            false,
        )
    }

    /// `depositAtBank()`: nothing without a "Bank booth" nearby.
    pub fn deposit_at_bank(&self, host: &mut H) -> Result<()> {
        let Some(bank) = self.nearest_scenery(host, "Bank booth")? else {
            return Ok(());
        };
        host.pulse_manager_run(
            &self.bot,
            Pulse::Deposit {
                api: self.clone(),
                bank,
            },
            false,
        )
    }

    /// `getNearestNode(name, true) as Scenery?`.
    fn nearest_scenery(&self, host: &mut H, name: &str) -> Result<Option<H::Node>> {
        let node = self.get_nearest_node(host, name)?;
        if let Some(node) = &node
            && host.kind(node) != NodeKind::Scenery
        {
            return Err(Error::Cast);
        }
        Ok(node)
    }

    /// The news line of a big sale.
    fn offer_news(&self, host: &mut H, id: i32, amount: i32) -> Result<()> {
        let name = host.item_name(id).ok_or(Error::Null)?;
        let text = format!(
            "{} just offered {} {} on the GE.",
            host.server_ge_name(),
            amount,
            lowercase_root(&name)
        );
        host.send_grand_exchange_news(&text)
    }

    /// `toCounterPulse.pulse()` of `sellOnGE`.
    pub(crate) fn sell_on_ge_pulse(&self, host: &mut H, id: i32) -> Result<bool> {
        let bot = &self.bot;
        let mut actual_id = id;
        let amount = host.container_amount(bot, Which::Bank, id)?;
        if host.item_note_id(id)? == id {
            actual_id = note_change(host, id)?;
        }
        let can_sell = host.ge_add_bot_offer(actual_id, amount)?;
        if can_sell && self.sale_is_big_news(host, actual_id, amount)? {
            self.offer_news(host, actual_id, amount)?;
        }
        host.container_remove(bot, Which::Bank, Item::new(id, amount))?;
        host.container_refresh(bot, Which::Bank)?;
        Ok(true)
    }

    /// `toCounterPulseAll.pulse()` of `sellAllOnGe` and, with `adv`, of
    /// `sellAllOnGeAdv`.
    pub(crate) fn sell_all_pulse(&self, host: &mut H, adv: bool) -> Result<bool> {
        let bot = &self.bot;
        for item in host
            .container_to_array(bot, Which::Bank)?
            .into_iter()
            .flatten()
        {
            // Lobster, swordfish and shark stay in the plain form.
            if !adv && matches!(item.id, 379 | 373 | 385) {
                continue;
            }
            if !host.item_is_tradeable(item.definition)? {
                continue;
            }
            let amount = item.amount;
            let mut actual_id = item.id;
            if host.item_note_id(actual_id)? == actual_id {
                actual_id = note_change(host, actual_id)?;
            }
            let can_sell = host.ge_add_bot_offer(actual_id, amount)?;
            if can_sell && self.sale_is_big_news(host, actual_id, amount)? {
                // The six log ids leave the loop body here: no news, and
                // the item is not taken out of the bank either.
                if adv && matches!(actual_id, 1511 | 1513 | 1515 | 1517 | 1519 | 1521) {
                    continue;
                }
                self.offer_news(host, actual_id, amount)?;
            }
            host.container_remove(bot, Which::Bank, item)?;
            host.container_refresh(bot, Which::Bank)?;
        }
        Ok(true)
    }

    /// `BankingPulse.pulse()` of `depositAtBank`.
    pub(crate) fn deposit_pulse(&self, host: &mut H, bank: &H::Node) -> Result<bool> {
        let bot = &self.bot;
        let location = host.location(bank);
        host.face_location(bot, location)?;
        for item in host
            .container_to_array(bot, Which::Inventory)?
            .into_iter()
            .flatten()
        {
            // Rune axe, tinderbox, adamant pickaxe and coins stay.
            if matches!(item.id, 1359 | 590 | 1271 | 995) {
                continue;
            }
            host.container_add(bot, Which::Bank, item)?;
            host.container_remove(bot, Which::Inventory, item)?;
        }
        Ok(true)
    }

    /// `saleIsBigNews(itemID, amount)`: the int product wraps.
    pub fn sale_is_big_news(&self, host: &mut H, item_id: i32, amount: i32) -> Result<bool> {
        let value = host.item_alchemy_value(item_id)?.wrapping_mul(amount);
        Ok(value >= host.ge_announcement_limit().unwrap_or(500))
    }

    /// `bankItem(item)`.
    pub fn bank_item(&self, host: &mut H, item: i32) -> Result<()> {
        host.pulse_manager_run(
            &self.bot,
            Pulse::BankItem {
                api: self.clone(),
                item,
            },
            false,
        )
    }

    /// `BankingPulse.pulse()` of `bankItem`.
    pub(crate) fn bank_item_pulse(&self, host: &mut H, item: i32) -> Result<bool> {
        let bot = &self.bot;
        let logs = host.container_amount(bot, Which::Inventory, item)?;
        host.container_remove(bot, Which::Inventory, Item::new(item, logs))?;
        host.container_add(bot, Which::Bank, Item::new(item, logs))?;
        Ok(true)
    }

    /// `bankAll(onComplete)`.
    pub fn bank_all(&self, host: &mut H, on_complete: Option<OnComplete<H>>) -> Result<()> {
        host.pulse_manager_run(
            &self.bot,
            Pulse::BankAll(BankAll {
                api: self.clone(),
                on_complete,
            }),
            false,
        )
    }

    /// `eat(foodId)`: below half to three quarters of the hitpoints level.
    pub fn eat(&self, host: &mut H, food_id: i32) -> Result<()> {
        let bot = &self.bot;
        let food_item = Item::one(food_id);
        let level = host.skills_static_level(bot, skill::HITPOINTS)?;
        let share = host.random_double(0.5, 0.75)?;
        if f64::from(level) * share >= f64::from(host.skills_lifepoints(bot)?)
            && host.container_contains_item(bot, Which::Inventory, food_item)?
        {
            self.consume(host, food_id, food_item, false)?;
        }
        Ok(())
    }

    /// `forceEat(foodId)`.
    pub fn force_eat(&self, host: &mut H, food_id: i32) -> Result<()> {
        let food_item = Item::one(food_id);
        if host.container_contains_item(&self.bot, Which::Inventory, food_item)? {
            self.consume(host, food_id, food_item, true)?;
        }
        Ok(())
    }

    /// The shared tail of `eat` and `forceEat`. An item that is not a known
    /// consumable is eaten as `Food(ids, HealingEffect(1))`: `eat` names the
    /// found item's id (a missing item fails there), `forceEat` the id it
    /// was given.
    fn consume(&self, host: &mut H, food_id: i32, food_item: Item, forced: bool) -> Result<()> {
        let bot = &self.bot;
        host.lock_for(bot, 3)?;
        let food = host.container_get_item(bot, Which::Inventory, food_item)?;
        let consumable = match host.consumable(food_id)? {
            Some(known) => Food::Known(known),
            None => Food::Plain {
                ids: vec![if forced {
                    food_id
                } else {
                    food.ok_or(Error::Null)?.id
                }],
                heal: 1,
            },
        };
        host.consume(consumable, food, bot)?;
        host.combat_delay_next_attack(bot, 3)
    }

    /// `buyFromGE(bot, itemID, amount)`: the work is launched off the tick.
    pub fn buy_from_ge(
        &self,
        host: &mut H,
        bot: &H::Node,
        item_id: i32,
        amount: i32,
    ) -> Result<()> {
        host.launch(BuyTask(Shared::new(BuyState {
            api: self.clone(),
            bot: bot.clone(),
            item_id,
            amount,
            offer: None,
            bought: false,
            released: false,
        })))
    }

    /// `withdraw(itemID, amount)`: the amount asked for when the bank has
    /// it, else what the bank has; cut to what the inventory can take.
    pub fn withdraw(&self, host: &mut H, item_id: i32, amount: i32) -> Result<()> {
        let bot = &self.bot;
        let mut item =
            if host.container_contains_item(bot, Which::Bank, Item::new(item_id, amount))? {
                Item::new(item_id, amount)
            } else {
                Item::new(item_id, host.container_amount(bot, Which::Bank, item_id)?)
            };
        if item.amount == 0 {
            return Ok(());
        }
        if !host.container_has_space_for(bot, Which::Inventory, item)? {
            let most = host.container_maximum_add(bot, Which::Inventory, item)?;
            item.set_amount(most);
        }
        host.container_remove(bot, Which::Bank, item)?;
        host.container_add(bot, Which::Inventory, item)?;
        Ok(())
    }

    /// `equipAndSetStats(items)`.
    pub fn equip_and_set_stats_all(&self, host: &mut H, items: Option<&[Item]>) -> Result<()> {
        for item in items.unwrap_or_default() {
            self.equip_and_set_stats(host, *item)?;
        }
        Ok(())
    }

    /// `equipAndSetStats(item)`: an item without an equipment slot does
    /// nothing; otherwise it is equipped and every requirement becomes the
    /// bot's static level.
    pub fn equip_and_set_stats(&self, host: &mut H, item: Item) -> Result<()> {
        let bot = &self.bot;
        let Some(slot) = host.item_handler(item.definition, "equipment_slot")? else {
            return Ok(());
        };
        let Handler::Int(slot) = slot else {
            return Err(Error::Cast);
        };
        host.equipment_add_slot(bot, item, slot, false, false)?;
        if host
            .item_handler(item.definition, "requirements")?
            .is_some()
        {
            match host.item_handler(item.definition, "requirements")? {
                Some(Handler::Requirements(requirements)) => {
                    for (skill, level) in requirements {
                        host.skills_set_static_level(bot, skill, level)?;
                    }
                }
                Some(_) => return Err(Error::Cast),
                None => return Err(Error::Null),
            }
        }
        host.skills_update_combat_level(bot)
    }

    /// `loadAppearanceAndEquipment(json)`.
    pub fn load_appearance_and_equipment(&self, host: &mut H, json: Option<&Json>) -> Result<()> {
        let Some(json) = json else {
            return Ok(());
        };
        let bot = &self.bot;
        host.container_clear(bot, Which::Equipment)?;
        match json.get("appearance") {
            None | Some(Json::Null) => return Err(Error::Null),
            Some(appearance @ Json::Object(_)) => host.appearance_parse(bot, appearance)?,
            Some(_) => return Err(Error::Cast),
        }
        match json.get("equipment") {
            None | Some(Json::Null) => return Err(Error::Null),
            Some(equipment @ Json::Array(_)) => {
                host.container_parse(bot, Which::Equipment, equipment)?;
            }
            Some(_) => return Err(Error::Cast),
        }
        host.appearance_sync(bot)?;
        let capacity = host.container_capacity(bot, Which::Equipment)?;
        for i in 0..capacity {
            if let Some(item) = host.container_get(bot, Which::Equipment, i)? {
                self.equip_and_set_stats(host, item)?;
            }
        }
        // Every combat skill is at least three quarters of the highest one.
        let highest_id = host.skills_highest_combat_skill(bot)?;
        let highest = host.skills_static_level(bot, highest_id)?;
        for i in 0..7 {
            let floor = java_double_to_int(f64::from(highest) * 0.75);
            let level = floor.max(host.skills_static_level(bot, i)?);
            host.skills_set_static_level(bot, i, level)?;
        }
        host.skills_update_combat_level(bot)
    }

    /// `getOverlay()`.
    pub fn get_overlay(&self) -> BottingOverlay<H> {
        BottingOverlay {
            player: self.bot.clone(),
        }
    }

    /// `checkPriceOverrides(id)`.
    pub fn check_price_overrides(&self, host: &mut H, id: i32) -> Result<Option<i32>> {
        host.item_ge_price(id)
    }
}

/// The callback of `bankAll`.
pub type OnComplete<H> = std::sync::Arc<dyn Fn(&mut H) -> Result<()> + Send + Sync>;

/// `walkArray`'s pulse: the step index is its own state.
pub struct WalkArray<H: Host> {
    pub api: ScriptApi<H>,
    pub steps: Vec<Location>,
    pub step_index: Shared<i32>,
}
impl<H: Host> Clone for WalkArray<H> {
    fn clone(&self) -> Self {
        Self {
            api: self.api.clone(),
            steps: self.steps.clone(),
            step_index: self.step_index.clone(),
        }
    }
}
impl<H: Host> WalkArray<H> {
    /// `pulse()`: done past the last step or within two tiles of it; a walk
    /// towards the current step while it is more than five tiles away;
    /// otherwise the next step.
    pub fn pulse(&self, host: &mut H) -> Result<bool> {
        let index = *self.step_index.read();
        let steps = &self.steps;
        if index < 0 || index as usize >= steps.len() {
            return Ok(true);
        }
        let here = loc(host, &self.api.bot)?;
        if here.within_distance(steps[steps.len() - 1], 2) {
            return Ok(true);
        }
        let here = loc(host, &self.api.bot)?;
        if !here.within_distance(steps[index as usize], 5) {
            self.api.walk_to(host, steps[index as usize])?;
            return Ok(false);
        }
        *self.step_index.write() = index + 1;
        Ok(false)
    }
}

/// `bankAll`'s pulse.
pub struct BankAll<H: Host> {
    pub api: ScriptApi<H>,
    pub on_complete: Option<OnComplete<H>>,
}
impl<H: Host> Clone for BankAll<H> {
    fn clone(&self) -> Self {
        Self {
            api: self.api.clone(),
            on_complete: self.on_complete.clone(),
        }
    }
}
impl<H: Host> BankAll<H> {
    /// `pulse()`: every inventory item that could be removed goes to the
    /// bank, then the callback runs.
    pub fn pulse(&self, host: &mut H) -> Result<bool> {
        let bot = &self.api.bot;
        for item in host
            .container_to_array(bot, Which::Inventory)?
            .into_iter()
            .flatten()
        {
            // `var itemAmount = bot.inventory.getAmount(item)`, never read.
            host.container_amount_of(bot, Which::Inventory, item)?;
            if host.container_remove(bot, Which::Inventory, item)? {
                host.container_add(bot, Which::Bank, item)?;
            }
        }
        if let Some(on_complete) = &self.on_complete {
            on_complete(host)?;
        }
        Ok(true)
    }
}

/// The state of one `buyFromGE` coroutine.
pub struct BuyState<H: Host> {
    pub api: ScriptApi<H>,
    pub bot: H::Node,
    pub item_id: i32,
    pub amount: i32,
    pub offer: Option<BotOffer<H::Node>>,
    /// `bought`.
    pub bought: bool,
    /// Whether the latch was counted down.
    pub released: bool,
}

/// What `buyFromGE` launches. The original's coroutine makes the offer,
/// runs a five-tick pulse on the bot and blocks on a latch that pulse
/// releases; then, if the offer was complete when the pulse ran, the items
/// go to the bank. Here the block has two halves: [`BuyTask::run`] up to
/// the wait and [`BuyTask::resume`] after it.
pub struct BuyTask<H: Host>(Shared<BuyState<H>>);
impl<H: Host> Clone for BuyTask<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> BuyTask<H> {
    pub fn offer(&self) -> Option<BotOffer<H::Node>> {
        self.0.read().offer.clone()
    }

    /// The launched block up to `latch.await()`.
    pub fn run(&self, host: &mut H) -> Result<()> {
        let (api, bot, item_id, amount) = {
            let state = self.0.read();
            (
                state.api.clone(),
                state.bot.clone(),
                state.item_id,
                state.amount,
            )
        };
        let offer = BotOffer::new();
        offer.update(|o| {
            o.item_id = item_id;
            o.sell = false;
        });
        let value = match api.check_price_overrides(host, item_id)? {
            Some(price) => price,
            None => host.item_value(item_id)?,
        };
        offer.update(|o| {
            o.offered_value = value;
            o.amount = amount;
            o.player = Some(bot.clone());
        });
        repository::add_offer(host, &bot, &offer);
        self.0.write().offer = Some(offer);
        host.pulse_manager_run(&bot, Pulse::Buy(self.clone()), false)
    }

    /// The pulse: whether the offer is complete, and the latch released.
    pub fn pulse(&self) -> Result<bool> {
        let offer = self.offer().ok_or(Error::Null)?.get();
        let mut state = self.0.write();
        state.bought = offer.completed_amount == offer.amount;
        state.released = true;
        Ok(true)
    }

    /// What follows `latch.await()`; `false` while the pulse has not run.
    pub fn resume(&self, host: &mut H) -> Result<bool> {
        let (bot, bought, released) = {
            let state = self.0.read();
            (state.bot.clone(), state.bought, state.released)
        };
        if !released {
            return Ok(false);
        }
        if bought {
            let offer = self.offer().ok_or(Error::Null)?.get();
            host.container_add(
                &bot,
                Which::Bank,
                Item::new(offer.item_id, offer.completed_amount),
            )?;
            host.container_refresh(&bot, Which::Bank)?;
        }
        Ok(true)
    }
}

/// `ScriptAPI.BottingOverlay`.
pub struct BottingOverlay<H: Host> {
    pub player: H::Node,
}
impl<H: Host> BottingOverlay<H> {
    /// `init()`.
    pub fn init(&self, host: &mut H) -> Result<()> {
        host.interface_open_overlay(&self.player, 195)?;
        host.send_interface_config(&self.player, 195, 5, true)
    }
    /// `setTitle(title)`.
    pub fn set_title(&self, host: &mut H, title: &str) -> Result<()> {
        host.send_string(&self.player, &colorize(&format!("%B{title}")), 195, 7)
    }
    /// `setTaskLabel(label)`.
    pub fn set_task_label(&self, host: &mut H, label: &str) -> Result<()> {
        host.send_string(&self.player, &colorize(&format!("%B{label}")), 195, 8)
    }
    /// `setAmount(amount)`.
    pub fn set_amount(&self, host: &mut H, amount: i32) -> Result<()> {
        host.send_string(&self.player, &colorize(&format!("%B{amount}")), 195, 9)
    }
}
