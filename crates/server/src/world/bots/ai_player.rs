//! `AIPlayer.java` and `ArtificialSession.java`.
//!
//! The constructor is the original's, in its order: the `PlayerDetails` name
//! `"/aip" + (currentUID + 1) + ":" + name`, the `Player` object (a host
//! node), its location, the `artificial` flag, the session singleton,
//! `Repository.getPlayers().add(this)` (the native `NodeList`: the first
//! free slot from 1, with no entry in the uid or name map), the display
//! name, the uid (`currentUID++`), the virtual `updateRandomValues()` and
//! `init()`. A constructor that fails leaves what it had done: the player in
//! the list, the counter where it was.
//!
//! `clear()` removes the bot from `botMapping` and then is `Player.clear`,
//! the repository port's [`disconnection::player_clear`], which for an
//! artificial player is `finishClear` at once. `deregister` looks the bot up
//! in `botMapping` only: after a `clear()` it finds nothing and the player
//! stays in `Repository.players`.
use super::{
    Dest, Direction, Error, Finder, Gender, Host, Item, Location, NodeKind, Pulse, Result, Statics,
    Which, loc, scanner::Scanner, skill, slot,
};
use crate::definitions::item::Shared;
use crate::world::{disconnection, repository, social::names::format_display_name};

/// `ArtificialSession`: the one `IoSession` every artificial player shares.
/// It writes nothing, queues nothing and never disconnects; its address is
/// the loopback address. `object` is `IoSession.object`, which every
/// `AIPlayer.init` overwrites with its own player.
pub struct ArtificialSession<N>(Shared<Option<N>>);
impl<N> Clone for ArtificialSession<N> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<N: Clone> Default for ArtificialSession<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<N: Clone> ArtificialSession<N> {
    /// `new ArtificialSession()`: `super(null, null)`.
    pub fn new() -> Self {
        Self(Shared::new(None))
    }
    /// `getRemoteAddress()`.
    pub fn remote_address(&self) -> &'static str {
        "127.0.0.1"
    }
    /// `write(context, instant)`: nothing.
    pub fn write(&self) {}
    /// `queue(buffer)`: nothing.
    pub fn queue(&self) {}
    /// `write()`: nothing.
    pub fn flush(&self) {}
    /// `disconnect()`: nothing; the session stays active.
    pub fn disconnect(&self) {}
    /// `getObject()`.
    pub fn object(&self) -> Option<N> {
        self.0.read().clone()
    }
    /// `setObject(object)`.
    pub fn set_object(&self, object: Option<N>) {
        *self.0.write() = object;
    }
}

/// Which engine class an artificial player object is. Content subclasses of
/// `PvMBots` are `Pvm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotClass {
    /// `AIPlayer`.
    Plain,
    /// `CombatBot`.
    Combat,
    /// `PvMBots`.
    Pvm,
}

/// The fields `AIPlayer` and its engine subclasses add to `Player`.
pub struct AiState<H: Host> {
    /// The `Player` object this is.
    pub node: H::Node,
    pub class: BotClass,
    /// `uid`: 0 until the constructor assigns it.
    pub uid: i32,
    /// `startLocation`.
    pub start_location: Option<Location>,
    /// `username`: null until the constructor assigns it.
    pub username: Option<String>,
    /// `highPriority`.
    pub high_priority: bool,
    /// `controler`.
    pub controler: Option<H::Node>,
    /// `CombatBot.tick` / `PvMBots.tick`.
    pub tick: i32,
}

/// An `AIPlayer` object: a shared handle.
pub struct AiPlayer<H: Host>(Shared<AiState<H>>);
impl<H: Host> Clone for AiPlayer<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// `AIPlayer`'s static initialiser: `loadNames("botnames.txt")`, once.
pub fn ensure_initialised<H: Host>(host: &mut H) -> Statics<H> {
    let statics = host.statics();
    if !statics.read(|s| s.initialised) {
        statics.write(|s| s.initialised = true);
        load_names(host, "botnames.txt");
    }
    statics
}

/// `ServerConstants.BOT_DATA_PATH + fileName`.
fn data_path<H: Host>(host: &mut H, file_name: &str) -> String {
    format!(
        "{}{}",
        host.bot_data_path().as_deref().unwrap_or("null"),
        file_name
    )
}

/// `AIPlayer.loadNames(fileName)`: every line of the file is added to
/// `botNames`; a missing file is printed and nothing is added.
pub fn load_names<H: Host>(host: &mut H, file_name: &str) {
    let statics = host.statics();
    let path = data_path(host, file_name);
    match host.read_file(&path) {
        Some(text) => {
            let mut scanner = Scanner::new(&text);
            while scanner.has_next_line() {
                let Some(line) = scanner.next_line() else {
                    break;
                };
                statics.write(|s| s.bot_names.push(line));
            }
        }
        None => host.print_stack_trace("FileNotFoundException"),
    }
}

/// `AIPlayer.getRandomName()`: a name is taken out of the list.
pub fn get_random_name<H: Host>(host: &mut H) -> Result<String> {
    let statics = ensure_initialised(host);
    let size = statics.read(|s| s.bot_names.len()) as i32;
    let index = host.random(size)?;
    statics.write(|s| {
        if index < 0 || index as usize >= s.bot_names.len() {
            return Err(Error::Index);
        }
        Ok(s.bot_names.remove(index as usize))
    })
}

/// `AIPlayer.updateRandomOSRScopyLine(fileName)`: reservoir sampling over
/// the lines the scanner still has a token for. The n-th line replaces the
/// kept one when the draw of n is 0, unless it is shorter than three
/// characters or starts with `#`. A missing file is printed.
pub fn update_random_osrs_copy_line<H: Host>(host: &mut H, file_name: &str) -> Result<()> {
    let statics = ensure_initialised(host);
    let mut n: i32 = 0;
    let path = data_path(host, file_name);
    let Some(text) = host.read_file(&path) else {
        host.print_stack_trace("FileNotFoundException");
        return Ok(());
    };
    let mut scanner = Scanner::new(&text);
    while scanner.has_next() {
        n = n.wrapping_add(1);
        let line = scanner.next_line().ok_or(Error::NoSuchElement)?;
        if host.new_random_next_int(n)? == 0 {
            if line.encode_utf16().count() < 3 || line.starts_with('#') {
                continue;
            }
            statics.write(|s| s.osrs_copy_line = Some(line));
        }
    }
    Ok(())
}

/// `AIPlayer.retrieveRandomName(fileName)`: lines are drawn until the kept
/// one is not a comment and holds neither `_` nor a space; its first field.
pub fn retrieve_random_name<H: Host>(host: &mut H, file_name: &str) -> Result<String> {
    let statics = ensure_initialised(host);
    loop {
        update_random_osrs_copy_line(host, file_name)?;
        let line = statics
            .read(|s| s.osrs_copy_line.clone())
            .ok_or(Error::Null)?;
        if !(line.starts_with('#') || line.contains('_') || line.contains(' ')) {
            break;
        }
    }
    let line = statics
        .read(|s| s.osrs_copy_line.clone())
        .ok_or(Error::Null)?;
    // `split(":")[0]`: a line of colons only splits into nothing.
    java_split_colon(&line).first().cloned().ok_or(Error::Index)
}

/// `String.split(":")`: trailing empty strings are dropped; a text without
/// a colon, the empty text included, is its own single element.
pub fn java_split_colon(text: &str) -> Vec<String> {
    if !text.contains(':') {
        return vec![text.to_owned()];
    }
    let mut parts: Vec<String> = text.split(':').map(str::to_owned).collect();
    while parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

/// `Integer.parseInt(text)`.
pub fn java_parse_int(text: &str) -> Result<i32> {
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() {
        return Err(Error::Number);
    }
    let mut value: i64 = 0;
    for c in digits.chars() {
        let digit = c.to_digit(10).ok_or(Error::Number)?;
        value = value * 10 + i64::from(digit);
        if value > i64::from(i32::MAX) + 1 {
            return Err(Error::Number);
        }
    }
    let value = if negative { -value } else { value };
    i32::try_from(value).map_err(|_| Error::Number)
}

impl<H: Host> AiPlayer<H> {
    /// `new AIPlayer(l)`: a name out of `botNames`.
    pub fn new(host: &mut H, l: Option<Location>) -> Result<Self> {
        Self::with_class(host, BotClass::Plain, l)
    }

    /// `new AIPlayer(fileName, l)`: a name out of a names-and-armour file.
    pub fn from_file(host: &mut H, file_name: &str, l: Option<Location>) -> Result<Self> {
        Self::with_class_from_file(host, BotClass::Plain, file_name, l)
    }

    /// `super(l)` of an engine subclass.
    pub fn with_class(host: &mut H, class: BotClass, l: Option<Location>) -> Result<Self> {
        let name = get_random_name(host)?;
        Self::construct(host, class, &name, l)
    }

    /// `super(fileName, l)` of an engine subclass.
    pub fn with_class_from_file(
        host: &mut H,
        class: BotClass,
        file_name: &str,
        l: Option<Location>,
    ) -> Result<Self> {
        let name = retrieve_random_name(host, file_name)?;
        Self::construct(host, class, &name, l)
    }

    /// The private `AIPlayer(name, l, ignored)`.
    fn construct(host: &mut H, class: BotClass, name: &str, l: Option<Location>) -> Result<Self> {
        let statics = ensure_initialised(host);
        let current = statics.read(|s| s.current_uid);
        let node = host.new_player(&super::canonical::details_username(current, name))?;
        let ai = Self(Shared::new(AiState {
            node: node.clone(),
            class,
            uid: 0,
            start_location: None,
            username: None,
            high_priority: false,
            controler: None,
            tick: 0,
        }));
        host.attach_ai_player(&node, &ai);
        ai.0.write().start_location = l;
        host.set_location(&node, l)?;
        host.set_artificial(&node, true);
        host.details_set_session(&node)?;
        // NodeList.add: the first free slot, or nothing when the list is full.
        if let Some(index) = host.repository().players().add(node.clone()) {
            host.set_index(&node, index);
        }
        let current = statics.read(|s| s.current_uid);
        let username = format_display_name(&format!("{name}{}", current.wrapping_add(1)));
        {
            let mut state = ai.0.write();
            state.username = Some(username);
            state.uid = current;
        }
        statics.write(|s| s.current_uid = current.wrapping_add(1));
        ai.update_random_values(host)?;
        ai.init(host)?;
        Ok(ai)
    }

    pub fn node(&self) -> H::Node {
        self.0.read().node.clone()
    }
    pub fn class(&self) -> BotClass {
        self.0.read().class
    }
    /// `getUid()`.
    pub fn uid(&self) -> i32 {
        self.0.read().uid
    }
    /// `getUsername()`.
    pub fn username(&self) -> Option<String> {
        self.0.read().username.clone()
    }
    /// `getStartLocation()`.
    pub fn start_location(&self) -> Option<Location> {
        self.0.read().start_location
    }
    /// `isHighPriority()`.
    pub fn is_high_priority(&self) -> bool {
        self.0.read().high_priority
    }
    /// `setHighPriority(highPriority)`.
    pub fn set_high_priority(&self, high_priority: bool) {
        self.0.write().high_priority = high_priority;
    }
    /// `getControler()`.
    pub fn controler(&self) -> Option<H::Node> {
        self.0.read().controler.clone()
    }
    /// `setControler(controler)`.
    pub fn set_controler(&self, controler: Option<H::Node>) {
        self.0.write().controler = controler;
    }
    /// `CombatBot.tick` / `PvMBots.tick`, the counter.
    pub fn tick_counter(&self) -> i32 {
        self.0.read().tick
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }

    /// `updateRandomValues()`, the virtual call: `CombatBot` overrides it.
    pub fn update_random_values(&self, host: &mut H) -> Result<()> {
        let node = self.node();
        match self.class() {
            BotClass::Combat => {
                let female = host.random(5)? == 1;
                host.appearance_set_gender(&node, gender(female))?;
            }
            BotClass::Plain | BotClass::Pvm => {
                let female = host.random(3)? == 1;
                host.appearance_set_gender(&node, gender(female))?;
                // `int setTo = RandomFunction.random(0, 10)`, never read.
                host.random_between(0, 10)?;
                host.character_design_randomize(&node, true)?;
            }
        }
        let values = Direction::ALL;
        let index = host.new_random_next_int(values.len() as i32)?;
        let direction = usize::try_from(index)
            .ok()
            .and_then(|i| values.get(i).copied())
            .ok_or(Error::Index)?;
        host.set_direction(&node, direction)?;
        host.skills_update_combat_level(&node)?;
        host.appearance_sync(&node)
    }

    /// `init()`.
    pub fn init(&self, host: &mut H) -> Result<()> {
        let node = self.node();
        let statics = host.statics();
        host.properties_set_spawn_location(&node, self.start_location())?;
        host.interface_open_default_tabs(&node)?;
        // getSession().setObject(this): the singleton every bot shares.
        statics
            .read(|s| s.session.clone())
            .set_object(Some(node.clone()));
        statics.write(|s| s.bot_mapping.insert(self.uid(), self.clone()));
        host.player_init(&node)?;
        host.settings_set_run_toggled(&node, true)?;
        host.character_design_randomize(&node, false)?;
        let location = host.location(&node);
        host.flags_set_last_scene_graph(&node, location)
    }

    /// `update()`: nothing.
    pub fn update(&self) {}

    /// `follow(e)`.
    pub fn follow(&self, host: &mut H, e: &H::Node) -> Result<()> {
        let bot = self.node();
        host.pulse_manager_run(
            &bot.clone(),
            Pulse::Follow {
                bot,
                entity: e.clone(),
            },
            true,
        )
    }

    /// `randomWalkAroundPoint(point, radius)`: both offsets are drawn
    /// before a null point fails.
    pub fn random_walk_around_point(
        &self,
        host: &mut H,
        point: Option<Location>,
        radius: i32,
    ) -> Result<()> {
        let node = self.node();
        let dx = host.random_between(radius, radius.wrapping_neg())?;
        let dy = host.random_between(radius, radius.wrapping_neg())?;
        let destination = Dest::Location(point.ok_or(Error::Null)?.transform(dx, dy, 0));
        let path = host.pathfinder_find(&node, &destination, Finder::Smart { near: true })?;
        host.path_walk(&path, &node)
    }

    /// `randomWalk(radiusX, radiusY)`: the location is read first, but a
    /// bot without one fails only after both offsets were drawn.
    pub fn random_walk(&self, host: &mut H, radius_x: i32, radius_y: i32) -> Result<()> {
        let node = self.node();
        let here = host.location(&node);
        let dx = host.random_between(radius_x, radius_x.wrapping_neg())?;
        let dy = host.random_between(radius_y, radius_y.wrapping_neg())?;
        let destination = Dest::Location(here.ok_or(Error::Null)?.transform(dx, dy, 0));
        let path = host.pathfinder_find(&node, &destination, Finder::Smart { near: false })?;
        host.path_walk(&path, &node)
    }

    /// `walkToPosSmart(x, y)`.
    pub fn walk_to_pos_smart_xy(&self, host: &mut H, x: i32, y: i32) -> Result<()> {
        self.walk_to_pos_smart(host, Location::new(x, y, 0))
    }

    /// `walkToPosSmart(loc)`.
    pub fn walk_to_pos_smart(&self, host: &mut H, location: Location) -> Result<()> {
        let node = self.node();
        let path = host.pathfinder_find(
            &node,
            &Dest::Location(location),
            Finder::Smart { near: true },
        )?;
        host.path_walk(&path, &node)
    }

    /// `walkPos(x, y)`: the path is found and dropped.
    pub fn walk_pos(&self, host: &mut H, x: i32, y: i32) -> Result<()> {
        let node = self.node();
        host.pathfinder_find(
            &node,
            &Dest::Location(Location::new(x, y, 0)),
            Finder::Default,
        )?;
        Ok(())
    }

    /// `checkVictimIsPlayer()`: the victim is read once for the null test
    /// and again for `isPlayer()`.
    pub fn check_victim_is_player(&self, host: &mut H) -> Result<bool> {
        let node = self.node();
        if host.combat_victim(&node)?.is_some() {
            let victim = host.combat_victim(&node)?.ok_or(Error::Null)?;
            if host.kind(&victim) == NodeKind::Player {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `AIPlayer.tick()`.
    fn base_tick(&self, host: &mut H) -> Result<()> {
        let node = self.node();
        host.player_tick(&node)?;
        if host.skull_is_wilderness(&node)? {
            let level = host.wilderness_level(&node)?;
            host.skull_set_level(&node, level)?;
        }
        // `if (getSkills().getLifepoints() <= 0) {}`: read, nothing done.
        host.skills_lifepoints(&node)?;
        Ok(())
    }

    /// `tick()`, the virtual call: `CombatBot` deregisters itself at zero
    /// lifepoints; `PvMBots` restores 20 at five or fewer and wraps its
    /// counter at 100.
    pub fn tick(&self, host: &mut H) -> Result<()>
    where
        H: repository::Host<H::Node>,
    {
        self.base_tick(host)?;
        let node = self.node();
        match self.class() {
            BotClass::Plain => {}
            BotClass::Combat => {
                self.bump_tick();
                if host.skills_lifepoints(&node)? == 0 {
                    deregister(host, self.uid())?;
                }
            }
            BotClass::Pvm => {
                self.bump_tick();
                if host.skills_lifepoints(&node)? <= 5 {
                    host.skills_set_lifepoints(&node, 20)?;
                }
                let mut state = self.0.write();
                if state.tick == 100 {
                    state.tick = 0;
                }
            }
        }
        Ok(())
    }

    fn bump_tick(&self) {
        let mut state = self.0.write();
        state.tick = state.tick.wrapping_add(1);
    }

    /// `getItemById(id)`: the first of the 28 inventory slots holding it.
    pub fn get_item_by_id(&self, host: &mut H, id: i32) -> Result<Option<Item>> {
        let node = self.node();
        for i in 0..28 {
            if let Some(item) = host.container_get(&node, Which::Inventory, i)?
                && item.id == id
            {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    /// `handleIncomingChat(ctx)`: nothing.
    pub fn handle_incoming_chat(&self) {}

    /// `interact(n)`: nothing.
    pub fn interact(&self) {}

    /// The private `getNodeInRange`: the local NPCs passing `wanted`, then
    /// the objects of a triangle of tiles around the bot on plane 0, each
    /// tile asked four times (once per quadrant, so a tile on an axis is
    /// asked more than once and its object added more than once).
    fn nodes_in_range(
        &self,
        host: &mut H,
        range: i32,
        wanted: &dyn Fn(i32) -> bool,
    ) -> Result<Vec<H::Node>> {
        let node = self.node();
        let here = loc(host, &node)?;
        let (me_x, me_y) = (here.x(), here.y());
        let mut nodes = Vec::new();
        // `RegionManager.getLocalNPCs(location, range)` reads the field again.
        let location = host.location(&node);
        for npc in host.local_npcs(location, range)? {
            if wanted(host.id(&npc)) {
                nodes.push(npc);
            }
        }
        let mut x = 0;
        while x < range {
            let mut y = 0;
            while y < range.wrapping_sub(x) {
                for (ox, oy) in [(x, y), (x, -y), (-x, y), (-x, -y)] {
                    let found =
                        host.region_object(me_x.wrapping_add(ox), me_y.wrapping_add(oy), 0)?;
                    if let Some(object) = found
                        && wanted(host.id(&object))
                    {
                        nodes.push(object);
                    }
                }
                y += 1;
            }
            x += 1;
        }
        Ok(nodes)
    }

    /// `getClosestNodeinNodeList` (with `direction`, the `WithDirection`
    /// form): the first node, then any strictly closer one.
    fn closest_in(
        &self,
        host: &mut H,
        nodes: &[H::Node],
        direction: Option<Direction>,
    ) -> Result<Option<H::Node>> {
        let me = self.node();
        let mut distance = 0.0;
        let mut found: Option<H::Node> = None;
        for node in nodes {
            let here = loc(host, &me)?;
            let node_distance = here.distance(loc(host, node)?);
            let matches = match direction {
                Some(direction) => host.direction(node) == Some(direction),
                None => true,
            };
            if (found.is_none() || node_distance < distance) && matches {
                distance = node_distance;
                found = Some(node.clone());
            }
        }
        Ok(found)
    }

    /// `getClosestNodeWithEntryAndDirection(range, entry, direction)`.
    pub fn get_closest_node_with_entry_and_direction(
        &self,
        host: &mut H,
        range: i32,
        entry: i32,
        direction: Direction,
    ) -> Result<Option<H::Node>> {
        let nodes = self.nodes_in_range(host, range, &|id| id == entry)?;
        self.closest_in(host, &nodes, Some(direction))
    }

    /// `getClosestNodeWithEntry(range, entry)`.
    pub fn get_closest_node_with_entry(
        &self,
        host: &mut H,
        range: i32,
        entry: i32,
    ) -> Result<Option<H::Node>> {
        let nodes = self.nodes_in_range(host, range, &|id| id == entry)?;
        self.closest_in(host, &nodes, None)
    }

    /// `getClosestNodeWithEntry(range, entrys)`.
    pub fn get_closest_node_with_entries(
        &self,
        host: &mut H,
        range: i32,
        entries: &[i32],
    ) -> Result<Option<H::Node>> {
        let nodes = self.nodes_in_range(host, range, &|id| entries.contains(&id))?;
        self.closest_in(host, &nodes, None)
    }

    /// The three `getClosesCreature` forms: the closest local NPC passing
    /// `wanted`, the distance kept as a truncated int starting at
    /// `radius + 1`.
    fn closest_creature(
        &self,
        host: &mut H,
        radius: i32,
        wanted: &dyn Fn(i32) -> bool,
    ) -> Result<Option<H::Node>> {
        let me = self.node();
        let mut distance = radius.wrapping_add(1);
        let mut found = None;
        // A bot without a location only fails once an NPC is measured.
        let location = host.location(&me);
        for npc in host.local_npcs(location, radius)? {
            let to_npc = loc(host, &npc)?.distance(loc(host, &me)?);
            if wanted(host.id(&npc)) && to_npc < f64::from(distance) {
                distance = super::super::content_api::java_double_to_int(to_npc);
                found = Some(npc);
            }
        }
        Ok(found)
    }

    /// `getClosesCreature(radius)`.
    pub fn get_closes_creature(&self, host: &mut H, radius: i32) -> Result<Option<H::Node>> {
        self.closest_creature(host, radius, &|_| true)
    }

    /// `getClosesCreature(radius, entry)`.
    pub fn get_closes_creature_with_entry(
        &self,
        host: &mut H,
        radius: i32,
        entry: i32,
    ) -> Result<Option<H::Node>> {
        self.closest_creature(host, radius, &|id| id == entry)
    }

    /// `getClosesCreature(radius, entrys)`.
    pub fn get_closes_creature_with_entries(
        &self,
        host: &mut H,
        radius: i32,
        entries: &[i32],
    ) -> Result<Option<H::Node>> {
        self.closest_creature(host, radius, &|id| entries.contains(&id))
    }

    /// `clear()`: out of `botMapping`, then `Player.clear()`.
    pub fn clear(&self, host: &mut H) -> Result<()>
    where
        H: repository::Host<H::Node>,
    {
        let statics = host.statics();
        let uid = self.uid();
        statics.write(|s| s.bot_mapping.remove(&uid));
        let repository = Host::repository(host);
        disconnection::player_clear(host, &repository, &self.node()).map_err(Error::from)
    }

    /// `reset()`.
    pub fn reset(&self, host: &mut H) -> Result<()> {
        let node = self.node();
        if host.flags_is_update_scene_graph(&node)? {
            let location = host.location(&node);
            host.flags_set_last_scene_graph(&node, location)?;
        }
        host.player_reset(&node)
    }

    /// `finalizeDeath(killer)`.
    pub fn finalize_death(&self, host: &mut H, killer: Option<&H::Node>) -> Result<()> {
        let node = self.node();
        host.player_finalize_death(&node, killer)?;
        host.full_restore(&node)
    }

    /// The private `parseOSRS(index)`: a field of `OSRScopyLine`.
    fn parse_osrs(&self, host: &mut H, index: usize) -> Result<i32> {
        let line = host
            .statics()
            .read(|s| s.osrs_copy_line.clone())
            .ok_or(Error::Null)?;
        let fields = java_split_colon(&line);
        java_parse_int(fields.get(index).ok_or(Error::Index)?)
    }

    /// The private `setLevels()`, which nothing in the pinned sources calls.
    /// `combatLevelsLeft -= 0.25 * hitpoints` is an int compound assignment:
    /// the difference is computed as a double and truncated.
    pub fn set_levels(&self, host: &mut H) -> Result<()> {
        use super::super::content_api::java_double_to_int as int;
        let node = self.node();
        let cap = self.parse_osrs(host, 1)?.min(99);
        let max_level = host.random_between(1, cap)?;
        for i in 0..skill::NUM_SKILLS {
            let level = host.linear_decrease_rand(max_level)?;
            host.skills_set_static_level(&node, i, level)?;
        }
        let mut left = self.parse_osrs(host, 1)?;
        let hitpoints = host
            .random_between(10, max_level.min(left.wrapping_mul(4)))?
            .max(10);
        left = int(f64::from(left) - 0.25 * f64::from(hitpoints));
        let prayer = if left > 0 {
            host.random(max_level.min(left.wrapping_mul(8)))?
        } else {
            1
        };
        left = int(f64::from(left) - 0.125 * f64::from(prayer));
        let defence = if left > 0 {
            host.random(max_level.min(left.wrapping_mul(4)))?
        } else {
            1
        };
        left = int(f64::from(left) - 0.25 * f64::from(defence));
        left = left.min(199);
        let attack = if left > 0 {
            host.normal_rand_dist(max_level.min(left.wrapping_mul(3)))?
        } else {
            1
        };
        let strength = if left > 0 {
            left.wrapping_mul(3).wrapping_sub(attack)
        } else {
            1
        };
        host.skills_set_static_level(&node, skill::HITPOINTS, hitpoints)?;
        host.skills_set_static_level(&node, skill::PRAYER, prayer)?;
        host.skills_set_static_level(&node, skill::DEFENCE, defence)?;
        host.skills_set_static_level(&node, skill::ATTACK, attack)?;
        host.skills_set_static_level(&node, skill::STRENGTH, strength)?;
        host.skills_set_static_level(&node, skill::RANGE, left / 2)?;
        host.skills_set_static_level(&node, skill::MAGIC, left / 2)
    }

    /// The private `giveArmor()`, which nothing in the pinned sources calls:
    /// nine fields of `OSRScopyLine` into their equipment slots.
    pub fn give_armor(&self, host: &mut H) -> Result<()> {
        for (index, slot) in [
            (2, slot::HAT),
            (3, slot::CAPE),
            (4, slot::AMULET),
            (5, slot::WEAPON),
            (6, slot::CHEST),
            (7, slot::SHIELD),
            (9, slot::LEGS),
            (11, slot::HANDS),
            (12, slot::FEET),
        ] {
            let id = self.parse_osrs(host, index)?;
            self.equip_if_exists(host, id, slot)?;
        }
        Ok(())
    }

    /// The private `equipIfExists(e, slot)` for `e = new Item(id)`: the
    /// item's name is its definition's, looked up by the id as given; its
    /// own id is that id's low 16 bits.
    fn equip_if_exists(&self, host: &mut H, id: i32, slot: i32) -> Result<()> {
        let e = Item::one(id);
        let name = host.item_name(e.definition).ok_or(Error::Null)?;
        if rs09_game_core::java_string::equals_ignore_case(&name, "null") {
            return Ok(());
        }
        if e.id != 0 {
            host.container_replace(&self.node(), Which::Equipment, e, slot)?;
        }
        Ok(())
    }
}

fn gender(female: bool) -> Gender {
    if female { Gender::Female } else { Gender::Male }
}

/// `AIPlayer.get(uid)`.
pub fn get<H: Host>(host: &mut H, uid: i32) -> Option<AiPlayer<H>> {
    ensure_initialised(host).read(|s| s.bot_mapping.get(&uid).cloned())
}

/// `AIPlayer.deregister(uid)`: a mapped bot is cleared and taken out of
/// `Repository.players`; an unknown uid does nothing.
pub fn deregister<H: Host + repository::Host<<H as Host>::Node>>(
    host: &mut H,
    uid: i32,
) -> Result<()> {
    if let Some(player) = get(host, uid) {
        player.clear(host)?;
        Host::repository(host)
            .players()
            .remove(Some(&player.node()));
    }
    Ok(())
}

/// `player.clear()`, the virtual call: `AIPlayer.clear` for an artificial
/// player object, `Player.clear` for any other. A binding's
/// `repository::Host::player_clear` is this.
pub fn virtual_clear<H: Host + repository::Host<<H as Host>::Node>>(
    host: &mut H,
    player: &<H as Host>::Node,
) -> Result<()> {
    match host.ai_player(player) {
        Some(ai) => ai.clear(host),
        None => {
            let repository = Host::repository(host);
            disconnection::player_clear(host, &repository, player).map_err(Error::from)
        }
    }
}
