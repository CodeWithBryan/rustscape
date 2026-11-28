//! Content use-with listeners in the live world: the item-on-object basics
//! of `world::global_items2` (docs/global-items2.md: WaterSourceListener,
//! SandSourceListener, SackOnHay and HatStand) and the item-on-NPC basics of
//! `world::item_on_npc` (docs/item-on-npc.md: FeedOnStrayDog and
//! FeedFaladorSheepdog), registered in the native InteractionListeners at
//! boot and run by `InteractionListeners.run`'s use-with path (the walk to
//! the scene object or NPC, the arrival, then the lambda).
//!
//! [`Everyday`] is both libraries' `Host` over the live world. The services
//! the installed lambdas reach answer through the native systems (the
//! canonical inventory with its listeners, the animators, messages, the
//! audio packet, the ground manager, the dialogue interpreter, the Falador
//! diary, the NPC masks' force chat); every other service of
//! `global_items2::Host` is a refusal by name ("not ported in the live
//! world: everyday item handler service NAME"), listed in
//! docs/live-world.md. The fill pulses (`pulseManager.run(pulse)`) are
//! STANDARD pulses of the player's canonical pulse manager submitted to
//! `GameWorld.Pulser` and dispatched by identity ([`EverydayWorld`]): a walk
//! stops them, as it does the original's.
use super::{Ctx, Node};
use crate::{
    definitions::item::Shared,
    inventory::item::Item as NativeItem,
    pulses,
    world::{
        global_handlers::{Kind, LogLevel, Store},
        global_items::{Anim, Gfx, ItemArg, QueueStrength},
        global_items2::{
            self as gi, Container, Definition, Dialogue, Diary, Error, Expression, Result, Speaker,
            Statics, Task, Value, random, registry,
        },
        interaction_listeners as listeners, item_on_npc, item_on_object,
        packet_handlers::{Base, Container as Held},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates;

/// A content callback of the dig registries: no content registers one here.
#[derive(Clone, Debug, PartialEq)]
pub enum NoCallback {}

/// The installed content use-with handlers' state.
#[derive(Default)]
pub struct EverydayWorld {
    /// The static fields of the `global_items2` classes.
    pub statics: Statics<NoCallback>,
    /// The pulses the lambdas ran in a player's pulse manager, with the
    /// player's node and the body.
    pub pulses: Vec<(pulses::Handle, usize, Task<Node, NoCallback>)>,
    /// The `global_items2` classes registered at boot, in scan order.
    pub registered: Vec<registry::Class>,
    /// The `item_on_npc` classes registered at boot, in scan order.
    pub npc_registered: Vec<item_on_npc::Class>,
}

/// The `global_items2` classes this world installs: the use-with listeners
/// whose every service is joined (ChestKeyListener's `replaceScenery` is
/// `SceneryBuilder.replace` over the scene).
pub const INSTALLED: [registry::Class; 5] = [
    registry::Class::ChestKey,
    registry::Class::HatStand,
    registry::Class::SackOnHay,
    registry::Class::SandSource,
    registry::Class::WaterSource,
];

fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(Some(format!(
        "not ported in the live world: everyday item handler service {what}"
    ))))
}
fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Player(h) => write!(f, "Player(node {})", h.node),
            Self::Npc(h) => write!(f, "Npc(node {})", h.node),
            Self::Item(i) => write!(f, "Item({})", i.read().id()),
            Self::Ground(g) => write!(f, "GroundItem({})", g.read().item.read().id()),
            Self::Scenery(s) => write!(f, "Scenery({})", s.read().id),
        }
    }
}

/// `global_items2::random::Random` over the world's generators.
impl random::Random for crate::startup::generators::WorldGenerators {
    fn next_int(&mut self, bound: i32) -> i32 {
        // Callers pass a positive bound; a draw Java would refuse answers 0
        // without drawing, as the drop tables' source does.
        self.shared_next_int(bound).unwrap_or(0)
    }
    fn next_boolean(&mut self) -> bool {
        self.shared.next_bool()
    }
    fn shuffle_int(&mut self, bound: i32) -> i32 {
        self.shuffle_next_int(bound).unwrap_or(0)
    }
    fn thread_int(&mut self, bound: i32) -> i32 {
        // ThreadLocalRandom.nextInt(bound) on the world thread's seed.
        let mut thread = random::ThreadRandom::new(self.thread_local.seed());
        let value = thread.next_int(bound);
        self.thread_local = crate::world::drop_tables::random::ThreadLocalRandom::new(thread.seed);
        value
    }
    fn thread_double(
        &mut self,
        origin: f64,
        bound: f64,
    ) -> std::result::Result<f64, crate::world::drop_tables::random::BadRange> {
        self.thread_local_next_double(origin, bound)
    }
}

/// Both libraries' `Host` over the live world.
pub struct Everyday<'a>(pub Ctx<'a>);

impl Everyday<'_> {
    pub(super) fn player(&self, node: &Node) -> Result<super::P> {
        self.0.player_of(node).map_err(|_| Error::Cast)
    }
    pub(super) fn native(&self, id: i32, amount: i32) -> crate::inventory::item::Handle {
        Shared::new(NativeItem::new(id, amount, 1000, &self.0.0.inner.items))
    }
    fn node_name(&mut self, node: &Node) -> Result<Option<String>> {
        match node {
            Node::Item(i) => Ok(i.read().name.clone()),
            Node::Ground(g) => Ok(g.read().item.read().name.clone()),
            // Scenery's Node name is its definition's.
            Node::Scenery(s) => {
                let definition = s.read().definition.clone().ok_or(Error::Null)?;
                let name = definition.read().name.clone();
                Ok(name)
            }
            Node::Npc(h) => {
                let rows = &self.0.0.inner.npcs.rows;
                let row = rows.get(&h.node).ok_or(Error::Null)?;
                Ok(row.definition.read().name.clone())
            }
            Node::Player(h) => Ok(self.0.0.inner.players.get(&h.node).map(|s| s.name.clone())),
        }
    }
    /// ContentAPI `sendMessage`: `player.sendMessages(*splitLines(message, 86))`.
    pub(super) fn message(&mut self, player: &Node, text: &str) -> Result<()> {
        let p = self.player(player)?;
        let units: Vec<u16> = text.encode_utf16().collect();
        let lines = crate::world::plain_dialogue::split_lines(&units, 86).map_err(bind)?;
        for line in lines {
            self.0
                .0
                .inner
                .message(&p, &String::from_utf16_lossy(&line))
                .map_err(bind)?;
        }
        Ok(())
    }
    /// ContentAPI `sendDialogue`: `dialogueInterpreter.sendDialogue(*splitLines(message))`.
    pub(super) fn dialogue(&mut self, player: &Node, text: &str) -> Result<()> {
        let p = self.player(player)?;
        let units: Vec<u16> = text.encode_utf16().collect();
        let lines = crate::world::plain_dialogue::split_lines(&units, 54).map_err(bind)?;
        let interpreter = self.0.0.interpreter_of(&p);
        let sent = interpreter
            .send_dialogue(self.0.0, Some(&crate::world::dialogue_dsl::lines(&lines)))
            .map(|_| ())
            .map_err(bind);
        self.0.0.inner.drain(&p);
        sent
    }
    /// ContentAPI `animate(entity, animation)`: `animator.animate(animation)`
    /// of a player or an NPC.
    pub(super) fn animate_entity(&mut self, entity: &Node, animation: Anim) -> Result<()> {
        let (Node::Player(h) | Node::Npc(h)) = entity else {
            return Err(Error::Cast);
        };
        if animation.delay != 0 || animation.priority != crate::world::global_items::Priority::Mid {
            return refuse("animate with a delay or a priority other than MID");
        }
        self.0
            .0
            .entity_visualize(
                &super::P(h.clone()),
                Some(crate::world::animation::Animation::new(animation.id)),
                None,
            )
            .map_err(bind)
    }
    /// `entity.sendChat(text)` of an NPC (its ForceChat mask).
    fn npc_chat(&mut self, entity: &Node, text: &str) -> Result<()> {
        match entity {
            Node::Npc(h) => self
                .0
                .0
                .npc_chat(h.node, text.to_owned())
                .map(|_| ())
                .map_err(bind),
            Node::Player(_) => refuse("Player.sendChat (force chat on a player)"),
            _ => Err(Error::Cast),
        }
    }
    /// ContentAPI `removeItem(player, item)` in the inventory:
    /// `player.inventory.remove(item)`.
    pub(super) fn remove_from_inventory(
        &mut self,
        player: &Node,
        item: &crate::inventory::item::Handle,
    ) -> Result<bool> {
        self.0
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.remove(Some(item), None, true, defs, ctx)
            })
            .map_err(bind)
    }
    /// `player.inventory.add(Item(id, amount))`.
    pub(super) fn add_to_inventory(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        let item = self.native(id, amount);
        self.0
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.add(Some(&item), true, -1, defs, ctx)
            })
            .map_err(bind)
    }
}

impl gi::Host for Everyday<'_> {
    type Node = Node;
    type Callback = NoCallback;

    fn kind(&self, node: &Node) -> Kind {
        match node {
            Node::Player(_) => Kind::Player,
            Node::Npc(_) => Kind::Npc,
            Node::Scenery(_) => Kind::Scenery,
            Node::Item(_) | Node::Ground(_) => Kind::Item,
        }
    }
    fn id(&mut self, node: &Node) -> Result<i32> {
        Base::id(&mut self.0, node).map_err(bind)
    }
    fn name(&mut self, node: &Node) -> Result<Option<String>> {
        self.node_name(node)
    }
    fn amount(&mut self, item: &Node) -> Result<i32> {
        Ok(Ctx::item(item).map_err(|_| Error::Cast)?.read().amount())
    }
    fn slot(&mut self, item: &Node) -> Result<i32> {
        Ok(Ctx::item(item).map_err(|_| Error::Cast)?.read().index)
    }
    fn location(&mut self, node: &Node) -> Result<Option<[i32; 3]>> {
        match node {
            Node::Player(h) | Node::Npc(h) => Ok(self
                .0
                .0
                .inner
                .state
                .node_location(h.node)
                .map_err(bind)?
                .map(|l| l.coordinates())),
            Node::Scenery(s) => Ok(Some(s.read().location.coordinates())),
            Node::Item(_) | Node::Ground(_) => refuse("location of an item"),
        }
    }
    fn lowercase_default(&self, text: &str) -> String {
        // The default locale of this world is the root's (as the listeners'
        // casing answers).
        lowercase_root(text)
    }
    fn lowercase_root(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn item_name(&mut self, id: i32) -> Option<String> {
        self.0.0.inner.items.for_id(id).read().name.clone()
    }
    fn definitions(&mut self) -> Vec<Definition> {
        definitions(&self.0.0.inner.items)
    }
    fn new_item(&mut self, id: i32, amount: i32) -> Result<Node> {
        Ok(Node::Item(self.native(id, amount)))
    }
    fn constant_item(&mut self, id: i32, amount: i32) -> Node {
        Node::Item(self.native(id, amount))
    }
    fn random(&mut self) -> &mut dyn random::Random {
        &mut self.0.0.inner.generators
    }
    fn statics(&mut self) -> &mut Statics<NoCallback> {
        &mut self.0.0.round.everyday.statics
    }
    fn send_message(&mut self, player: &Node, text: &str) -> Result<()> {
        self.message(player, text)
    }
    fn send_dialogue(&mut self, player: &Node, text: &str) -> Result<()> {
        self.dialogue(player, text)
    }
    fn pulse_run(
        &mut self,
        entity: &Node,
        pulse: Task<Node, NoCallback>,
        delay: i32,
    ) -> Result<()> {
        let p = self.player(entity)?;
        let class = match &pulse {
            Task::WaterFill { .. } => {
                "content.global.handlers.item.withobject.WaterSourceListener$defineListeners$1$1"
            }
            Task::RunTask { .. } => "core.api.ContentAPIKt$runTask$1",
            _ => return refuse("pulse_run of a pulse other than the water fill and runTask"),
        };
        // `Pulse(delay)`: no checked nodes.
        let handle = Shared::new(pulses::Pulse::new(
            delay,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            class.into(),
        ));
        if self
            .0
            .0
            .run_standard_pulse(&p, handle.clone())
            .map_err(bind)?
        {
            let world = &mut self.0.0.round.everyday;
            world.pulses.retain(|(h, _, _)| h.read().running);
            world.pulses.push((handle, p.node(), pulse));
        }
        Ok(())
    }
    fn animate(&mut self, entity: &Node, animation: Anim) -> Result<bool> {
        self.animate_entity(entity, animation)?;
        Ok(true)
    }
    fn chat(&mut self, entity: &Node, text: &str) -> Result<()> {
        self.npc_chat(entity, text)
    }
    fn play_audio(
        &mut self,
        player: &Node,
        id: i32,
        delay: i32,
        loops: i32,
        location: Option<[i32; 3]>,
        _radius: i32,
    ) -> Result<()> {
        if location.is_some() {
            return refuse("playAudio at a location");
        }
        // AudioPacket with no location: the synth sound.
        let p = self.player(player)?;
        self.0
            .0
            .inner
            .queue_native(&p, updates::synth_sound(id, loops, delay));
        Ok(())
    }
    fn remove_item(&mut self, player: &Node, item: ItemArg<Node>, store: Store) -> Result<bool> {
        if store != Store::Inventory {
            return refuse("removeItem from a container other than the inventory");
        }
        let item = match item {
            ItemArg::Id(id) => self.native(id, 1),
            ItemArg::Item(node) => Ctx::item(&node).map_err(|_| Error::State)?,
        };
        self.remove_from_inventory(player, &item)
    }
    fn add_item(&mut self, player: &Node, id: i32, amount: i32, store: Store) -> Result<bool> {
        if store != Store::Inventory {
            return refuse("addItem to a container other than the inventory");
        }
        self.add_to_inventory(player, id, amount)
    }
    fn add_item_or_drop(&mut self, player: &Node, id: i32, amount: i32) -> Result<()> {
        // One stack, or one item at a time for an unstackable amount; what
        // does not fit is dropped for the player at its location.
        let stackable = self.0.0.inner.items.for_id(id).read().is_stackable();
        let (count, each) = if amount == 1 || stackable {
            (1, amount)
        } else {
            (amount.max(0), 1)
        };
        for _ in 0..count {
            if !self.add_to_inventory(player, id, each)? {
                let p = self.player(player)?;
                let at = self.0.0.inner.state.node_location(p.node()).map_err(bind)?;
                let item = self.native(id, each);
                self.0
                    .0
                    .ground_create(item, at, Some(p.0.clone()))
                    .map_err(bind)?;
            }
        }
        Ok(())
    }
    fn amount_in_inventory(&mut self, player: &Node, id: i32) -> Result<i32> {
        let inventory = self.0.container(player, Held::Inventory).map_err(bind)?;
        Ok(inventory.amount_id(id, &self.0.0.inner.items))
    }
    fn get_diary(&mut self, player: &Node, _: Diary) -> Result<()> {
        self.player(player)?;
        Ok(())
    }
    fn diary_complete(
        &mut self,
        player: &Node,
        diary: Diary,
        level: i32,
        task: i32,
    ) -> Result<bool> {
        use crate::world::npc_classes::{self as nc, Host as _};
        let Diary::Falador = diary;
        let p = self.player(player)?;
        let mut classes = super::super::subclasses::Classes::new(self.0.0);
        classes.get_diary(&p, nc::Diary::Falador).map_err(bind)?;
        classes
            .diary_complete(nc::Diary::Falador, level, task)
            .map_err(bind)
    }
    fn diary_update(
        &mut self,
        player: &Node,
        diary: Diary,
        level: i32,
        task: i32,
        complete: bool,
    ) -> Result<()> {
        use crate::world::npc_classes::{self as nc, Host as _};
        let Diary::Falador = diary;
        let p = self.player(player)?;
        let mut classes = super::super::subclasses::Classes::new(self.0.0);
        classes.get_diary(&p, nc::Diary::Falador).map_err(bind)?;
        let updated = classes
            .diary_update(nc::Diary::Falador, &p, level, task, complete)
            .map_err(bind);
        self.0.0.inner.drain(&p);
        updated
    }
    fn log(&mut self, class: &str, level: LogLevel, message: &str) -> Result<()> {
        self.0
            .0
            .inner
            .log_line(format!("[{class}] {} {message}", level.name()));
        Ok(())
    }
    fn stack_trace(&mut self, exception: &str) {
        self.0
            .0
            .inner
            .log_line(format!("an everyday item handler printed {exception}"));
    }
    fn item_examine(&mut self, id: i32) -> Option<String> {
        let definition = self.0.0.inner.items.for_id(id);
        crate::world::packet_handlers::examine::item_examine_text(&definition, lowercase_root).ok()
    }

    // Every other service: named refusals.
    fn charge(&mut self, _: &Self::Node) -> Result<i32> {
        refuse("charge")
    }
    fn set_charge(&mut self, _: &Self::Node, _: i32) -> Result<()> {
        refuse("set_charge")
    }
    fn get_attribute(&mut self, _: &Self::Node, _: &str) -> Result<Option<Value<Self::Node>>> {
        refuse("get_attribute")
    }
    fn set_attribute(&mut self, _: &Self::Node, _: &str, _: Value<Self::Node>) -> Result<()> {
        refuse("set_attribute")
    }
    fn remove_attribute(&mut self, _: &Self::Node, _: &str) -> Result<()> {
        refuse("remove_attribute")
    }
    fn debug(&mut self, _: &Self::Node, _: &str) -> Result<()> {
        refuse("debug")
    }
    fn item_message(&mut self, _: &Self::Node, _: &Self::Node, _: &[&str]) -> Result<()> {
        refuse("item_message")
    }
    fn item_dialogue(&mut self, _: &Self::Node, _: ItemArg<Self::Node>, _: &str) -> Result<()> {
        refuse("item_dialogue")
    }
    fn player_dialogue(&mut self, _: &Self::Node, _: &str, _: Expression, _: bool) -> Result<()> {
        refuse("player_dialogue")
    }
    fn dialogue_lines(&mut self, _: &Self::Node, _: &[&str]) -> Result<()> {
        refuse("dialogue_lines")
    }
    fn open_interface(&mut self, _: &Self::Node, _: i32) -> Result<()> {
        refuse("open_interface")
    }
    fn interface_text(&mut self, _: &Self::Node, _: &str, _: i32, _: i32) -> Result<()> {
        refuse("interface_text")
    }
    fn close_all_interfaces(&mut self, _: &Self::Node) -> Result<()> {
        refuse("close_all_interfaces")
    }
    fn open_component(
        &mut self,
        _: &Self::Node,
        _: i32,
        _: Option<Task<Self::Node, Self::Callback>>,
    ) -> Result<()> {
        refuse("open_component")
    }
    fn open_default_tabs(&mut self, _: &Self::Node) -> Result<()> {
        refuse("open_default_tabs")
    }
    fn remove_tabs(&mut self, _: &Self::Node, _: &[i32]) -> Result<()> {
        refuse("remove_tabs")
    }
    fn has_requirement(&mut self, _: &Self::Node, _: &str, _: bool) -> Result<bool> {
        refuse("has_requirement")
    }
    fn quests(&mut self) -> Result<Vec<i32>> {
        refuse("quests")
    }
    fn quest_points(&mut self, _: &Self::Node) -> Result<i32> {
        refuse("quest_points")
    }
    fn lock(&mut self, _: &Self::Node, _: i32) -> Result<()> {
        refuse("lock")
    }
    fn unlock(&mut self, _: &Self::Node) -> Result<()> {
        refuse("unlock")
    }
    fn lock_interactions(&mut self, _: &Self::Node, _: i32) -> Result<()> {
        refuse("lock_interactions")
    }
    fn stop_walk(&mut self, _: &Self::Node) -> Result<()> {
        refuse("stop_walk")
    }
    fn submit_world(&mut self, _: Task<Self::Node, Self::Callback>, _: i32) -> Result<()> {
        refuse("submit_world")
    }
    fn visualize(&mut self, _: &Self::Node, _: Anim, _: Gfx) -> Result<bool> {
        refuse("visualize")
    }
    fn transform_npc(&mut self, _: &Self::Node, _: i32) -> Result<()> {
        refuse("transform_npc")
    }
    fn play_jingle(&mut self, _: &Self::Node, _: i32) -> Result<()> {
        refuse("play_jingle")
    }
    fn container_add(&mut self, _: &Self::Node, _: Container, _: &Self::Node) -> Result<bool> {
        refuse("container_add")
    }
    fn container_add_all(
        &mut self,
        _: &Self::Node,
        _: Container,
        _: &[Option<Self::Node>],
    ) -> Result<bool> {
        refuse("container_add_all")
    }
    fn container_remove(
        &mut self,
        _: &Self::Node,
        _: Container,
        _: Option<&Self::Node>,
    ) -> Result<bool> {
        refuse("container_remove")
    }
    fn container_remove_slot(
        &mut self,
        _: &Self::Node,
        _: Container,
        _: &Self::Node,
        _: i32,
        _: bool,
    ) -> Result<bool> {
        refuse("container_remove_slot")
    }
    fn container_free_slots(&mut self, _: &Self::Node, _: Container) -> Result<i32> {
        refuse("container_free_slots")
    }
    fn container_replace(
        &mut self,
        _: &Self::Node,
        _: Container,
        _: &Self::Node,
        _: i32,
    ) -> Result<Option<Self::Node>> {
        refuse("container_replace")
    }
    fn container_get(
        &mut self,
        _: &Self::Node,
        _: Container,
        _: i32,
    ) -> Result<Option<Self::Node>> {
        refuse("container_get")
    }
    fn container_to_array(
        &mut self,
        _: &Self::Node,
        _: Container,
    ) -> Result<Vec<Option<Self::Node>>> {
        refuse("container_to_array")
    }
    fn replace_slot(
        &mut self,
        _: &Self::Node,
        _: i32,
        _: &Self::Node,
        _: Option<&Self::Node>,
        _: Store,
    ) -> Result<Option<Self::Node>> {
        refuse("replace_slot")
    }
    fn produce_ground_item(&mut self, _: &Self::Node, _: i32) -> Result<()> {
        refuse("produce_ground_item")
    }
    fn ground_item(&mut self, _: &Self::Node, _: &Self::Node) -> Result<()> {
        refuse("ground_item")
    }
    fn replace_scenery(
        &mut self,
        to: &Self::Node,
        with: i32,
        ticks: i32,
        at: Option<[i32; 3]>,
    ) -> Result<()> {
        // ContentAPI replaceScenery(toReplace, with, forTicks, loc):
        // SceneryBuilder.replace(toReplace, toReplace.transform(with,
        // toReplace.rotation, newLoc)[, forTicks]) over the scene, then
        // `toReplace.isActive = false`.
        use crate::world::scenery::{self as builder, ReplaceMode};
        let Node::Scenery(object) = to else {
            return Err(Error::Cast);
        };
        let (location, shape, rotation) = {
            let object = object.read();
            (object.location.clone(), object.shape, object.rotation)
        };
        let location = at.map_or(location, |[x, y, z]| {
            crate::world::doors::Location::new(x, y, z)
        });
        let walk = &mut self.0.0.walk;
        let construct = crate::world::scenery_node::construct(
            &mut walk.scene.definitions,
            with,
            location,
            shape,
            rotation,
            false,
        )
        .map_err(bind)?;
        let mode = if ticks == -1 {
            ReplaceMode::default()
        } else {
            ReplaceMode::Temporary { clip: true, ticks }
        };
        let mut world = walk.scene.context(&mut walk.maps);
        builder::replace(&mut world, Some(object), Some(&construct), mode).map_err(bind)?;
        builder::set_active(&mut world, object, false).map_err(bind)
    }
    fn static_level(&mut self, _: &Self::Node, _: i32) -> Result<i32> {
        refuse("static_level")
    }
    fn add_experience(&mut self, _: &Self::Node, _: i32, _: f64) -> Result<()> {
        refuse("add_experience")
    }
    fn wilderness_level(&mut self, _: &Self::Node) -> Result<i32> {
        refuse("wilderness_level")
    }
    fn run_spade_listener(&mut self, _: &Self::Node, _: &Self::Callback) -> Result<()> {
        refuse("run_spade_listener")
    }
    fn run_dig_action(&mut self, _: &Self::Node, _: &Self::Callback) -> Result<()> {
        refuse("run_dig_action")
    }
    fn username(&mut self, _: &Self::Node) -> Result<String> {
        refuse("username")
    }
    fn open_dialogue(&mut self, _: &Self::Node, _: Dialogue<Self::Node>) -> Result<()> {
        refuse("open_dialogue")
    }
    fn send_dialogues(
        &mut self,
        _: &Self::Node,
        _: Speaker<Self::Node>,
        _: Option<Expression>,
        _: &[String],
    ) -> Result<()> {
        refuse("send_dialogues")
    }
    fn add_topic(&mut self, _: &Self::Node, _: Expression, _: &str, _: i32) -> Result<()> {
        refuse("add_topic")
    }
    fn clear_topics(&mut self, _: &Self::Node) -> Result<()> {
        refuse("clear_topics")
    }
    fn send_options(&mut self, _: &Self::Node, _: &str, _: &[&str]) -> Result<()> {
        refuse("send_options")
    }
    fn interpreter_close(&mut self, _: &Self::Node) -> Result<bool> {
        refuse("interpreter_close")
    }
    fn slayer_master(&mut self, _: &Self::Node) -> Result<Option<Self::Node>> {
        refuse("slayer_master")
    }
    fn slayer_tip(&mut self, _: &Self::Node) -> Result<Vec<String>> {
        refuse("slayer_tip")
    }
    fn has_slayer_task(&mut self, _: &Self::Node) -> Result<bool> {
        refuse("has_slayer_task")
    }
    fn slayer_task_name(&mut self, _: &Self::Node) -> Result<String> {
        refuse("slayer_task_name")
    }
    fn slayer_kills_remaining(&mut self, _: &Self::Node) -> Result<i32> {
        refuse("slayer_kills_remaining")
    }
    fn slayer_task_flags(&mut self, _: &Self::Node) -> Result<i32> {
        refuse("slayer_task_flags")
    }
    fn set_varp(&mut self, _: &Self::Node, _: i32, _: i32, _: bool) -> Result<()> {
        refuse("set_varp")
    }
    fn queue_script(
        &mut self,
        _: &Self::Node,
        _: i32,
        _: QueueStrength,
        _: bool,
        _: Task<Self::Node, Self::Callback>,
    ) -> Result<()> {
        refuse("queue_script")
    }
    fn stop_executing(&mut self, _: &Self::Node) -> Result<bool> {
        refuse("stop_executing")
    }
    fn animation_duration(&mut self, _: i32) -> Result<i32> {
        refuse("animation_duration")
    }
    fn projectile_location(&mut self, _: &Self::Node) -> Result<Option<[i32; 3]>> {
        refuse("projectile_location")
    }
    fn random_location(&mut self, _: [i32; 3], _: i32, _: bool) -> Result<Option<[i32; 3]>> {
        refuse("random_location")
    }
    fn face_location(&mut self, _: &Self::Node, _: [i32; 3]) -> Result<()> {
        refuse("face_location")
    }
    #[allow(clippy::too_many_arguments)]
    fn spawn_projectile(
        &mut self,
        _: [i32; 3],
        _: [i32; 3],
        _: i32,
        _: i32,
        _: i32,
        _: i32,
        _: i32,
        _: i32,
    ) -> Result<()> {
        refuse("spawn_projectile")
    }
    fn opened_component(&mut self, _: &Self::Node) -> Result<Option<i32>> {
        refuse("opened_component")
    }
    fn component_close(&mut self, _: &Self::Node, _: i32) -> Result<()> {
        refuse("component_close")
    }
    fn close_interface(&mut self, _: &Self::Node) -> Result<bool> {
        refuse("close_interface")
    }
    fn container_contains(&mut self, _: &Self::Node, _: Container, _: &Self::Node) -> Result<bool> {
        refuse("container_contains")
    }
    fn storage_contains(&mut self, _: &Self::Node, _: &str, _: i32) -> Result<bool> {
        refuse("storage_contains")
    }
    fn in_inventory(&mut self, _: &Self::Node, _: i32, _: i32) -> Result<bool> {
        refuse("in_inventory")
    }
    fn in_equipment(&mut self, _: &Self::Node, _: i32, _: i32) -> Result<bool> {
        refuse("in_equipment")
    }
    #[allow(clippy::too_many_arguments)]
    fn generate_items(
        &mut self,
        _: &Self::Node,
        _: &[Option<Self::Node>],
        _: &[&str],
        _: i32,
        _: i32,
        _: i32,
        _: i32,
    ) -> Result<i32> {
        refuse("generate_items")
    }
}

/// `ItemDefinition.getDefinitions().values` as the registrations read them:
/// the ids put in ascending order, walked in the JDK HashMap's order.
pub fn definitions(items: &crate::definitions::item::Registry) -> Vec<Definition> {
    let table = items.definitions();
    let mut ids: Vec<i32> = table.keys().copied().collect();
    ids.sort_unstable();
    gi::tables::java_hash_order(&ids)
        .into_iter()
        .filter_map(|id| table.get(&id).map(|d| (id, d)))
        .map(|(id, d)| {
            let d = d.read();
            Definition {
                id,
                name: d.name.clone(),
                equipment_slot: match d.handlers.get("equipment_slot") {
                    Some(crate::definitions::property::Property::Int(slot)) => Some(*slot),
                    _ => None,
                },
            }
        })
        .collect()
}

impl item_on_npc::Host for Everyday<'_> {
    type Node = Node;
    fn kind(&self, node: &Node) -> Kind {
        gi::Host::kind(self, node)
    }
    fn id(&mut self, node: &Node) -> Result<i32> {
        gi::Host::id(self, node)
    }
    fn name(&mut self, node: &Node) -> Result<Option<String>> {
        self.node_name(node)
    }
    fn definition_name(&mut self, node: &Node) -> Result<Option<String>> {
        match node {
            Node::Item(i) => {
                let id = i.read().id();
                Ok(self.0.0.inner.items.for_id(id).read().name.clone())
            }
            Node::Npc(_) => self.node_name(node),
            _ => Err(Error::Cast),
        }
    }
    fn lowercase_default(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn lowercase_root(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn remove_item(&mut self, player: &Node, item: &Node) -> Result<bool> {
        let item = Ctx::item(item).map_err(|_| Error::State)?;
        self.remove_from_inventory(player, &item)
    }
    fn send_message(&mut self, player: &Node, text: &str) -> Result<()> {
        self.message(player, text)
    }
    fn send_dialogue(&mut self, player: &Node, text: &str) -> Result<()> {
        self.dialogue(player, text)
    }
    fn animate(&mut self, entity: &Node, animation: i32) -> Result<()> {
        self.animate_entity(entity, Anim::new(animation))
    }
    fn send_chat(&mut self, entity: &Node, text: &str) -> Result<()> {
        self.npc_chat(entity, text)
    }
    fn in_borders(&mut self, entity: &Node, borders: [i32; 4]) -> Result<bool> {
        let p = self.player(entity)?;
        self.0.0.in_borders(&p, borders)
    }
    fn open_cider_dialogue(&mut self, player: &Node, with: &Node) -> Result<()> {
        let p = self.player(player)?;
        let Node::Npc(npc) = with else {
            return Err(Error::Cast);
        };
        // super::withnpc.
        self.0.0.open_cider_dialogue(&p, &super::P(npc.clone()))
    }
}

impl super::super::host::Live {
    /// The content use-with listeners' `defineListeners`, at boot, in the
    /// class scan's order (`scan.contentInterfaces`): FeedFaladorSheepdog
    /// and FeedOnStrayDog, then HatStand, SackOnHay, SandSourceListener and
    /// WaterSourceListener.
    pub fn install_content_use(&mut self) -> std::result::Result<(), String> {
        let registry = self.item_actions.listeners.clone();
        let interfaces = self.listeners.clone();
        let definitions = definitions(&self.inner.items);
        let casing = super::super::global::RootCasing;
        let mut native = registry::Native::new(
            &registry,
            &interfaces,
            &casing,
            definitions,
            &mut self.inner.generators,
        );
        let mut registered = Vec::new();
        let mut npc_registered = Vec::new();
        let mut object_registered = Vec::new();
        let mut cooking = false;
        let mut woodcutting = false;
        let (mut mining, mut prospect, mut fishing) = (false, false, false);
        let (mut barbarian_listeners, mut barbarian_knife, mut barbarian_manager) =
            (false, false, false);
        let mut otto = false;
        let mut pickpocket = false;
        let mut firemaking = false;
        let mut bones = false;
        let mut altar = false;
        let mut herb_clean = false;
        let mut potion_mixing = false;
        let mut barbarian_mix = false;
        let mut silver_stringing = false;
        let mut log_fletching = false;
        let (mut stringing, mut arrows) = (false, false);
        let mut ammo = Vec::new();
        use crate::world::gathering::barbarian;
        for name in crate::startup::scan::section("scan.contentInterfaces") {
            if name == crate::world::cooking::CLASS {
                // CookingRewrite.defineListeners (super::cooking).
                crate::world::cooking::define(&registry).map_err(|e| format!("{name}: {e:?}"))?;
                cooking = true;
            }
            if name == super::woodcutting::LISTENER_CLASS {
                // WoodcuttingListener.defineListeners (super::woodcutting).
                woodcutting = true;
            }
            if name == super::mining::LISTENER_CLASS {
                // MiningListener.defineListeners (super::mining).
                mining = true;
            }
            if name == super::fishing::LISTENER_CLASS {
                // FishingListener.defineListeners (super::fishing).
                fishing = true;
            }
            if name == crate::world::firemaking::LISTENER_CLASS {
                firemaking = true;
            }
            if name == crate::world::fletching_logs::LISTENER_CLASS {
                log_fletching = true;
            }
            if name == crate::world::fletching_assembly::STRINGING_CLASS {
                stringing = true;
            }
            if name == crate::world::fletching_assembly::ARROW_CLASS {
                arrows = true;
            }
            {
                use crate::world::fletching_ammo as a;
                let classes = [
                    a::BOLT_CLASS,
                    a::DART_CLASS,
                    a::LIMBING_CLASS,
                    a::GEM_CLASS,
                    a::GRAPPLE_CLASS,
                ];
                if let Some(class) = classes.into_iter().find(|class| *class == name) {
                    ammo.push(class);
                }
            }
            if name == crate::world::crafting_tools::SILVER_CLASS {
                silver_stringing = true;
            }
            if name == crate::world::herblore_tools::MIX_CLASS {
                barbarian_mix = true;
            }
            if name == crate::world::herblore::CLEAN_CLASS {
                herb_clean = true;
            }
            if name == crate::world::herblore::MIXING_CLASS {
                potion_mixing = true;
            }
            if name == crate::world::prayer_altar::LISTENER_CLASS {
                altar = true;
            }
            if name == crate::world::bone_burying::LISTENER_CLASS {
                bones = true;
            }
            if name == crate::world::dialogue_dsl::otto::CLASS {
                otto = true;
            }
            if name == crate::world::thieving_pickpocket::CLASS {
                pickpocket = true;
            }
            if name == barbarian::LISTENERS_CLASS {
                barbarian_listeners = true;
            }
            if name == barbarian::KNIFE_CLASS {
                barbarian_knife = true;
            }
            if name == barbarian::MANAGER_CLASS {
                barbarian_manager = true;
            }
            if name == super::mining::PROSPECT_CLASS {
                // ProspectListener.defineListeners (super::mining).
                prospect = true;
            }
            if let Some(class) = item_on_npc::Class::named(name) {
                item_on_npc::define(&registry, class).map_err(|e| format!("{name}: {e:?}"))?;
                npc_registered.push(class);
            }
            if let Some(class) = item_on_object::Class::named(name) {
                // super::withobject.
                item_on_object::define(&registry, &casing, class)
                    .map_err(|e| format!("{name}: {e:?}"))?;
                object_registered.push(class);
            }
            if let Some(class) = registry::Class::named(name)
                && INSTALLED.contains(&class)
            {
                registry::define(&mut native, class).map_err(|e| format!("{name}: {e:?}"))?;
                registered.push(class);
            }
        }
        self.round.everyday.registered = registered;
        self.round.everyday.npc_registered = npc_registered;
        self.round.with_object.registered = object_registered;
        self.round.cooking.registered = cooking;
        if woodcutting {
            // Metadata lives in its own map, so its place among the other
            // classes' registrations is not observable.
            self.define_woodcutting_listener();
        }
        if mining {
            self.define_mining_listener();
        }
        if fishing {
            self.define_fishing_listener();
        }
        if prospect {
            // The only generic "prospect" listener, so its place among
            // the other registrations is not observable either.
            self.define_prospect_listener()?;
        }
        // Barbarian fishing (super::barbarian): each registration has its
        // own key, so their place among the others is not observable.
        if barbarian_listeners {
            self.define_barbarian_listeners()?;
        }
        if barbarian_knife {
            self.define_knife_with_fish()?;
        }
        if barbarian_manager {
            // SpotManager.startup().
            self.barbarian_startup()?;
        }
        if firemaking {
            // FiremakingListener.defineListeners (super::firemaking).
            self.define_firemaking_listener()?;
        }
        if crate::startup::scan::section("scan.initializable")
            .contains(&super::fletching::SKILL_DIALOGUE_PLUGIN)
        {
            // SkillDialoguePlugin.init() (super::fletching).
            self.define_skill_dialogue_plugin()?;
        }
        if log_fletching {
            // LogCraftableListeners.defineListeners (super::fletching).
            self.define_log_fletching_listener()?;
        }
        if stringing {
            // StringingListeners.defineListeners (super::fletching_assembly).
            self.define_stringing_listener()?;
        }
        if arrows {
            // ArrowListeners.defineListeners (super::fletching_assembly).
            self.define_arrow_listeners()?;
        }
        for class in ammo {
            // Bolt, Dart, Limbing, GemBolt and Grapple listeners
            // (super::fletching_ammo).
            self.define_fletching_ammo_listeners(class)?;
        }
        if silver_stringing {
            // SilverStringingListener.defineListeners
            // (super::crafting_tools): scanned before the herblore
            // listeners.
            self.define_silver_stringing_listener()?;
        }
        if barbarian_mix {
            // BarbarianMixListener.defineListeners (super::herblore_tools):
            // scanned before the two other herblore listeners.
            self.define_barbarian_mix_listener()?;
        }
        if herb_clean {
            // HerbCleanListener.defineListeners (super::herblore): the only
            // generic "clean" listener, so its place is not observable.
            self.define_herb_clean_listener()?;
        }
        if potion_mixing {
            // PotionMixingListener.defineListeners (super::herblore).
            self.define_potion_mixing_listener()?;
        }
        if altar {
            // PrayerAltarListener.defineListeners (super::altar).
            self.define_prayer_altar_listener()?;
        }
        if bones {
            // BoneBuryListener.defineListeners (super::bones).
            self.define_bone_bury_listener()?;
        }
        if pickpocket {
            // ThievingListeners.defineListeners (super::pickpocket).
            self.define_pickpocket_listener()?;
        }
        if otto {
            // OttoGodblessedDialogue.defineListeners (super::otto).
            self.define_otto_listener()?;
        }
        Ok(())
    }
    /// `player.pulseManager.run(pulse)` (STANDARD) of a content pulse: the
    /// pulses that are not running go, nothing happens while a STRONG pulse
    /// runs, the current STANDARD pulse stops, then the pulse is put,
    /// started (`start` answers `isRunning()` afterwards) and, while it is
    /// running, submitted to `GameWorld.Pulser`. Whether it was submitted.
    pub fn run_content_pulse(
        &mut self,
        player: &super::P,
        handle: pulses::Handle,
        start: impl FnOnce(&mut Self) -> bool,
    ) -> std::result::Result<bool, String> {
        let node = player.node();
        let debug = |e: &dyn std::fmt::Debug| format!("{e:?}");
        let manager = self
            .inner
            .state
            .actor(node)
            .map_err(|e| debug(&e))?
            .manager
            .clone();
        for (kind, pulse) in manager.entries() {
            if let Some(pulse) = pulse
                && !pulse.read().running
            {
                manager.put(kind, None);
            }
        }
        if manager
            .get(Some(pulses::Kind::Strong))
            .is_some_and(|p| p.read().running)
        {
            return Ok(false);
        }
        self.with_walk(|world| world.clear_standard(node, manager.clone()))
            .map_err(|e| debug(&e))?;
        manager.put(Some(pulses::Kind::Standard), Some(handle.clone()));
        self.walk.content_pulses.retain(|h| h.read().running);
        self.walk.content_pulses.push(handle.clone());
        let running = start(self);
        handle.write().running = running;
        if !running {
            return Ok(false);
        }
        self.driver
            .pulser()
            .submit(Some(handle))
            .map_err(|e| debug(&e))?;
        Ok(true)
    }
    /// [`Self::run_content_pulse`] of a base `Pulse` (`start()` only sets
    /// `running`).
    pub fn run_standard_pulse(
        &mut self,
        player: &super::P,
        handle: pulses::Handle,
    ) -> std::result::Result<bool, String> {
        self.run_content_pulse(player, handle, |_| true)
    }
    /// A content use-with lambda (`InteractionListeners`' `invoke_use`), if
    /// the function is one this module installed.
    pub fn content_use(
        &mut self,
        f: &listeners::Function,
        player: &Node,
        used: &Node,
        with: &Node,
    ) -> Option<Result<bool>> {
        if let Some(interaction) = f.value::<registry::Interaction>() {
            let mut host = Everyday(Ctx(self));
            return Some(registry::run_interaction(
                &mut host,
                *interaction,
                player,
                used,
                Some(with),
            ));
        }
        if f.value::<crate::world::cooking::Interaction>().is_some() {
            return Some(self.cooking_use(player, used, with));
        }
        if let Some(ammo) = f.value::<super::fletching_ammo::Ammo>() {
            return Some(
                self.fletching_ammo_use(*ammo, player, used, with)
                    .map_err(bind),
            );
        }
        if let Some(assembly) = f.value::<super::fletching_assembly::Assembly>() {
            return Some(
                self.fletching_assembly_use(*assembly, player, used, with)
                    .map_err(bind),
            );
        }
        if let Some(mixing) = f.value::<super::herblore::Mixing>() {
            return Some(
                self.potion_mixing_use(*mixing, player, used, with)
                    .map_err(bind),
            );
        }
        if f.value::<super::crafting_tools::SilverStringing>()
            .is_some()
        {
            return Some(self.silver_stringing_use(player, used, with).map_err(bind));
        }
        if f.value::<super::herblore_tools::Mix>().is_some() {
            return Some(self.barbarian_mix_use(player, used, with).map_err(bind));
        }
        if f.value::<super::fletching::KnifeOnLogs>().is_some() {
            return Some(self.log_fletching_use(player, with).map_err(bind));
        }
        if f.value::<super::firemaking::TinderboxOnLogs>().is_some() {
            return Some(self.firemaking_use(player, None, with).map_err(bind));
        }
        if f.value::<super::firemaking::BowOnLogs>().is_some() {
            return Some(self.firemaking_use(player, Some(used), with).map_err(bind));
        }
        if f.value::<super::barbarian::KnifeWithFish>().is_some() {
            // KnifeWithFish: `fish.id` of the node the knife was used with.
            let fish = match Base::id(&mut Ctx(self), with) {
                Ok(id) => id,
                Err(e) => return Some(Err(bind(e))),
            };
            return Some(self.barbarian_knife_with_fish(player, fish).map_err(bind));
        }
        if let Some(interaction) = f.value::<item_on_npc::Interaction>() {
            let mut host = Everyday(Ctx(self));
            return Some(item_on_npc::run(
                &mut host,
                *interaction,
                player,
                used,
                with,
            ));
        }
        if let Some(interaction) = f.value::<item_on_object::Interaction>() {
            let mut host = super::withobject::WithObject(Everyday(Ctx(self)));
            return Some(item_on_object::run(
                &mut host,
                *interaction,
                player,
                used,
                with,
            ));
        }
        None
    }
    /// A pulse a content use-with lambda ran in a pulse manager, by
    /// identity: `Some` with whether it finished.
    pub fn everyday_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let pulses = &mut self.round.everyday.pulses;
        let at = pulses.iter().position(|(h, _, _)| h.same(pulse))?;
        let (handle, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let result = registry::run_task(&mut Everyday(Ctx(self)), &mut task);
        if matches!(result, Ok(false)) {
            self.round.everyday.pulses.push((handle, owner, task));
        }
        Some(result)
    }
}
