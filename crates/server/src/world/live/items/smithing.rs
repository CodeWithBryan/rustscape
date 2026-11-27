//! Smithing at anvils in the live world (`world::smithing`, ported without
//! a JVM corpus: docs/smithing.md): SmithingPlugin for the nine anvils in
//! `UseWithHandler.HANDLERS`, its anonymous OptionHandler on 42027's
//! "smith" option, SmithingInterface as the component plugin of 300, and
//! SmithingPulse as a STANDARD pulse of the player's pulse manager whose
//! `SkillPulse.stop()` (the animation reset) also runs when the manager
//! stops it.
//!
//! `Smith` is the library's `Host` over the live world: the canonical
//! inventory, the native Skills, the dialogue interpreter's plain
//! dialogue, the interface manager, `InterfaceContainer.generateItems`
//! (client script 150, the access mask and the item container, with the
//! class's static key counter kept in `SmithingWorld`), the persisted
//! diaries, the animator and the numeric input prompt. The three game
//! attributes ("smith-type", "smith-bar", "smith-item") hold enum constants
//! in the original; here they are kept per player in `SmithingWorld` and
//! dropped at logout (they are not saved attributes). Quests answer false
//! by name (none is ported): Doric's anvil 2782 always refuses and dart
//! tips and claws are never smithed. The skillcape perk answers false by
//! name. The two dialogue plugins the plugin opens (the shields' 82127843
//! and the godsword's 62362) are refused by name.
use super::{super::host::Live, Ctx, Node, P};
use crate::{
    definitions::{item::Shared, option_handler},
    inventory::item::Item as NativeItem,
    pulses,
    world::{
        animation::{Animation, Priority},
        component_objects::{NativeObject, Plugin},
        packet_handlers::{
            Container as Held,
            use_with::{Event, HandlerData, UseHandler},
        },
        player_death as pd,
        player_managers::diary,
        smithing::{self, AmountInput, Diary, Error, Input, Result, SmithingPulse},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates::{self, ScriptArg, WireItem};
use std::collections::HashMap;

/// `SmithingPlugin`'s id among this world's use-with handlers.
pub const SMITH_HANDLER: usize = 3;

/// SmithingPlugin's anonymous OptionHandler on the scenery definitions.
#[derive(Clone, Copy, Debug)]
pub struct AnvilPlugin;
impl option_handler::NativeOptionHandler for AnvilPlugin {}

/// SmithingInterface, the component plugin of 300.
pub struct SmithPlugin;
impl NativeObject for SmithPlugin {}

/// The amount prompt's script: the player and the handler.
#[derive(Clone)]
pub struct SmithScript(pub P, pub AmountInput);

/// A player's three smithing game attributes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Attributes {
    /// "smith-type": absent, or the (possibly null) BarType.
    pub kind: Option<Option<usize>>,
    /// "smith-bar".
    pub bar: Option<usize>,
    /// "smith-item".
    pub item: Option<i32>,
}

/// The plugin's objects, attributes and running pulses.
pub struct SmithingWorld {
    /// The anonymous OptionHandler.
    pub option: Option<option_handler::Handle>,
    /// `new SmithingPlugin()`.
    pub handler: Option<UseHandler>,
    /// The game attributes, by the player's node.
    pub attributes: HashMap<usize, Attributes>,
    /// The smithing pulses, by their pulse object, with the player's node.
    pub pulses: Vec<(pulses::Handle, usize, SmithingPulse<Node>)>,
    /// `InterfaceContainer.index` (the grand exchange's port keeps its own
    /// copy; the exchange is not in the live world).
    pub container_index: i32,
    /// SmithingPlugin and SmithingInterface were registered at boot.
    pub registered: (bool, bool),
}
impl Default for SmithingWorld {
    fn default() -> Self {
        Self {
            option: None,
            handler: None,
            attributes: HashMap::new(),
            pulses: Vec::new(),
            container_index: 600,
            registered: (false, false),
        }
    }
}
impl SmithingWorld {
    /// `InterfaceContainer.increment()`.
    fn increment(&mut self) -> i32 {
        if self.container_index == 6999 {
            self.container_index = 600;
        }
        let key = self.container_index;
        self.container_index += 1;
        key
    }
}

fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}
fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(Some(format!(
        "not ported in the live world: {what}"
    ))))
}

/// `DiaryType`'s ordinal.
fn kind(diary: Diary) -> usize {
    match diary {
        Diary::Varrock => 1,
        Diary::Lumbridge => 2,
        Diary::Falador => 3,
    }
}

/// `world::smithing`'s host over the live world.
pub struct Smith<'a>(pub Ctx<'a>);

impl Smith<'_> {
    fn player(&self, node: &Node) -> Result<P> {
        self.0.player_of(node).map_err(|_| Error::Cast)
    }
    fn native(&self, id: i32, amount: i32) -> crate::inventory::item::Handle {
        Shared::new(NativeItem::new(id, amount, 1000, &self.0.0.inner.items))
    }
    fn attributes(&mut self, player: &Node) -> Result<&mut Attributes> {
        let p = self.player(player)?;
        Ok(self
            .0
            .0
            .round
            .smithing
            .attributes
            .entry(p.node())
            .or_default())
    }
}

impl smithing::Host for Smith<'_> {
    type Node = Node;

    fn quest_complete(&mut self, _: &Node, _: &str) -> Result<bool> {
        self.0.0.inner.not_ported.default_answer(
            "QuestRepository.isComplete for smithing (quests not ported: false, no Doric's anvil or dart tips)",
        );
        Ok(false)
    }
    fn has_requirement(&mut self, _: &Node, _: &str) -> Result<bool> {
        self.0.0.inner.not_ported.default_answer(
            "hasRequirement for smithing (quest requirements not ported: false, no claws)",
        );
        Ok(false)
    }
    fn barefisted(&mut self, _: &Node) -> Result<bool> {
        self.0.0.inner.not_ported.default_answer(
            "SkillcapePerks.isActive(BAREFISTED_SMITHING) (skillcape perks off: false)",
        );
        Ok(false)
    }
    fn send_dialogue(&mut self, player: &Node, text: &str) -> Result<()> {
        // `sendDialogue(String...)` with one line.
        let p = self.player(player)?;
        let line: Vec<u16> = text.encode_utf16().collect();
        let interpreter = self.0.0.interpreter_of(&p);
        let sent = interpreter
            .send_dialogue(self.0.0, Some(&crate::world::dialogue_dsl::lines(&[line])))
            .map(|_| ())
            .map_err(bind);
        self.0.0.inner.drain(&p);
        sent
    }
    fn open_dialogue(&mut self, _: &Node, key: i32, _: &[i32]) -> Result<()> {
        refuse(&format!(
            "the smithing dialogue plugin {key} (the shields and the godsword)"
        ))
    }
    fn level(&mut self, player: &Node, skill: i32) -> Result<i32> {
        let p = self.player(player)?;
        crate::world::combat_swing::Host::level(self.0.0, &p, skill).map_err(bind)
    }
    fn inventory_contains(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        let inventory = self.0.container(player, Held::Inventory).map_err(bind)?;
        Ok(inventory.contains(id, amount))
    }
    fn inventory_amount(&mut self, player: &Node, id: i32) -> Result<i32> {
        let inventory = self.0.container(player, Held::Inventory).map_err(bind)?;
        Ok(inventory.amount_id(id, &self.0.0.inner.items))
    }
    fn inventory_ids(&mut self, player: &Node) -> Result<Vec<Option<i32>>> {
        let inventory = self.0.container(player, Held::Inventory).map_err(bind)?;
        let items = inventory.items();
        let ids = items
            .read()
            .iter()
            .map(|slot| slot.as_ref().map(|item| item.read().id()))
            .collect();
        Ok(ids)
    }
    fn inventory_remove(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        let item = self.native(id, amount);
        self.0
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.remove(Some(&item), None, true, defs, ctx)
            })
            .map_err(bind)
    }
    fn inventory_add(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        let item = self.native(id, amount);
        self.0
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.add(Some(&item), true, -1, defs, ctx)
            })
            .map_err(bind)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        let name = self.0.0.inner.items.for_id(id).read().name.clone();
        name.ok_or(Error::Null)
    }
    fn lowercase(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn remove_smith_type(&mut self, player: &Node) -> Result<()> {
        self.attributes(player)?.kind = None;
        Ok(())
    }
    fn set_smith_type(&mut self, player: &Node, kind: Option<usize>) -> Result<()> {
        self.attributes(player)?.kind = Some(kind);
        Ok(())
    }
    fn smith_type(&mut self, player: &Node) -> Result<Option<usize>> {
        Ok(self.attributes(player)?.kind.flatten())
    }
    fn set_smith_bar(&mut self, player: &Node, bar: usize) -> Result<()> {
        self.attributes(player)?.bar = Some(bar);
        Ok(())
    }
    fn smith_bar(&mut self, player: &Node) -> Result<Option<usize>> {
        Ok(self.attributes(player)?.bar)
    }
    fn set_smith_item(&mut self, player: &Node, item: i32) -> Result<()> {
        self.attributes(player)?.item = Some(item);
        Ok(())
    }
    fn smith_item(&mut self, player: &Node) -> Result<i32> {
        // `(int) null`: a NullPointerException.
        self.attributes(player)?.item.ok_or(Error::Null)
    }
    fn send_interface_config(
        &mut self,
        player: &Node,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()> {
        let p = self.player(player)?;
        crate::world::dialogue::Host::send_interface_config(self.0.0, p, interface, child, hidden)
            .map_err(bind)
    }
    fn send_string(&mut self, player: &Node, text: &str, interface: i32, child: i32) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .inner
            .text(&p, text, interface, child)
            .map_err(bind)
    }
    fn generate_item(
        &mut self,
        player: &Node,
        id: i32,
        amount: i32,
        interface: i32,
        child: i32,
    ) -> Result<()> {
        // `generateItems(player, items, options, interface, child)`: the
        // key is taken first (x 7, y 3), `generate` sends the script, the
        // settings and the container and takes the next key.
        let p = self.player(player)?;
        let live = &mut *self.0.0;
        let key = live.round.smithing.increment();
        let pointer = interface.wrapping_shl(16) | child;
        let manager = live.inner.canonical(&p).player.manager();
        let script = updates::run_client_script(
            150,
            b"IviiiIs",
            &[
                ScriptArg::Text(Vec::new()),
                ScriptArg::Int(-1),
                ScriptArg::Int(0),
                ScriptArg::Int(7),
                ScriptArg::Int(3),
                ScriptArg::Int(key),
                ScriptArg::Int(pointer),
            ],
            manager.packet_count(1),
        )
        .map_err(bind)?;
        live.inner.queue_native(&p, script);
        // `IfaceSettingsBuilder().enableAllOptions().build()`: 2046.
        let settings = updates::interface_events(pointer, 0, 1, 2046, manager.packet_count(1));
        live.inner.queue_native(&p, settings);
        let items = updates::inventory_full(
            (-1i32).wrapping_shl(16) | (-2 & 0xFFFF),
            key & 0xFFFF,
            &[Some(WireItem { id, amount })],
        )
        .map_err(bind)?;
        live.inner.queue_native(&p, items);
        live.round.smithing.increment();
        Ok(())
    }
    fn open_interface(&mut self, player: &Node, id: i32) -> Result<()> {
        let p = self.player(player)?;
        pd::Host::open_component(self.0.0, &p.0, id).map_err(bind)
    }
    fn close_interface(&mut self, player: &Node) -> Result<()> {
        let p = self.player(player)?;
        pd::Host::close_interface(self.0.0, &p.0).map_err(bind)
    }
    fn animate(&mut self, player: &Node, id: i32) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .entity_visualize(&p, Some(Animation::new(id)), None)
            .map_err(bind)
    }
    fn reset_animation(&mut self, player: &Node) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .entity_visualize(&p, Some(Animation::with(-1, 0, Some(Priority::High))), None)
            .map_err(bind)
    }
    fn resource_produced(&mut self, _: &Node, _: i32, _: i32) -> Result<()> {
        self.0
            .0
            .inner
            .not_ported
            .skip("ResourceProducedEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn add_experience(&mut self, player: &Node, skill: i32, experience: f64) -> Result<()> {
        let p = self.player(player)?;
        let added =
            crate::world::combat_pulse::Host::add_experience(self.0.0, &p, skill, experience, true)
                .map_err(bind);
        self.0.0.inner.drain(&p);
        added
    }
    fn send_message(&mut self, player: &Node, text: &str) -> Result<()> {
        let p = self.player(player)?;
        self.0.0.inner.message(&p, text).map_err(bind)
    }
    fn location(&mut self, player: &Node) -> Result<[i32; 3]> {
        let p = self.player(player)?;
        self.0
            .0
            .inner
            .state
            .node_location(p.node())
            .map_err(bind)?
            .map(|l| l.coordinates())
            .ok_or(Error::Null)
    }
    fn diary_finish_task(
        &mut self,
        player: &Node,
        which: Diary,
        level: i32,
        index: i32,
    ) -> Result<()> {
        let p = self.player(player)?;
        super::super::subclasses::with_diaries(self.0.0, &p, |h, owner| {
            diary::finish_task(h, owner, owner, Some(kind(which)), level, index)
        })
        .map_err(bind)
    }
    fn send_input(&mut self, player: &Node, prompt: &str, handler: AmountInput) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .smith_prompt(&p, prompt, SmithScript(p.clone(), handler))
            .map_err(bind)
    }
    fn run_pulse(&mut self, player: &Node, mut pulse: SmithingPulse<Node>) -> Result<()> {
        let p = self.player(player)?;
        // `SkillPulse(player, node)`: `Pulse(1, player, node)`. The player
        // leaving stops the pulse (super::super::round).
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            smithing::PULSE_CLASS.into(),
        ));
        let live = &mut *self.0.0;
        let mut started = Ok(());
        let submitted = live
            .run_content_pulse(&p, handle.clone(), |live| {
                started = pulse.start(&mut Smith(Ctx(live)));
                started.is_ok() && pulse.running
            })
            .map_err(bind)?;
        if let Err(error) = started {
            // `start()` threw inside PulseManager.run: nothing is submitted.
            pulses::stop_base(&handle);
            return Err(error);
        }
        if submitted {
            live.walk.stopping_pulses.push(handle.clone());
            live.round.smithing.pulses.push((handle, p.node(), pulse));
        }
        Ok(())
    }
}

/// `SmithingPlugin.newInstance`'s registrations over the live world.
struct Install<'a>(&'a mut Live);
impl smithing::Registrar for Install<'_> {
    fn new_use_handler(&mut self, allowed: Vec<i32>) -> Result<()> {
        self.0.round.smithing.handler = Some(Shared::new(HandlerData {
            name: "SmithingPlugin".into(),
            allowed_nodes: Some(allowed),
            dynamic: false,
            handler: SMITH_HANDLER,
        }));
        Ok(())
    }
    fn add_use_handler(&mut self, id: i32, kind: i32) -> Result<()> {
        let handler = self.0.round.smithing.handler.clone().ok_or(Error::Null)?;
        self.0
            .item_actions
            .use_handlers
            .add_handler(id, kind, &handler);
        Ok(())
    }
    fn define_option_plugin(&mut self, id: i32, key: &str) -> Result<()> {
        let plugin = option_handler::Handler::new(smithing::OPTION_CLASS, AnvilPlugin);
        self.0.round.smithing.option = Some(plugin.clone());
        let definition = self.0.walk.scene.definitions.for_id(id);
        let handlers = definition.read().handlers.clone();
        handlers.write().insert(
            key.to_owned(),
            crate::definitions::property::Property::OptionHandler(plugin),
        );
        Ok(())
    }
}

impl Live {
    /// `SmithingPlugin.newInstance(null)` and
    /// `SmithingInterface.newInstance(null)` (`ComponentDefinition.put(300,
    /// plugin)`), each if the class scan names it.
    pub fn install_smithing(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if scanned.contains(&smithing::INTERFACE_CLASS) {
            self.inner
                .registry
                .for_id(smithing::INTERFACE)
                .write()
                .plugin = Some(Plugin::new(SmithPlugin));
            self.round.smithing.registered.1 = true;
        }
        if scanned.contains(&smithing::PLUGIN_CLASS) {
            smithing::new_instance(&mut Install(self))
                .map_err(|e| format!("{}: {e:?}", smithing::PLUGIN_CLASS))?;
            self.round.smithing.registered.0 = true;
        }
        Ok(())
    }

    /// `SmithingPlugin.handle(event)`.
    pub fn anvil_use(&mut self, player: &P, event: &Event<Node>) -> Result<bool> {
        let mut ctx = Ctx(self);
        let used = crate::world::packet_handlers::Base::id(&mut ctx, &event.used).map_err(bind)?;
        let anvil = crate::world::packet_handlers::Base::id(&mut ctx, &event.with).map_err(bind)?;
        let node = Node::Player(player.0.clone());
        let handled = smithing::handle(&mut Smith(ctx), &node, used, anvil);
        self.inner.drain(player);
        handled
    }

    /// The anonymous OptionHandler's `handle(player, node, option)`.
    pub fn anvil_option(&mut self, player: &P, anvil: i32) -> Result<bool> {
        let node = Node::Player(player.0.clone());
        let handled = smithing::option_handle(&mut Smith(Ctx(self)), &node, anvil);
        self.inner.drain(player);
        handled
    }

    /// `SmithingInterface.handle`: a button of 300.
    pub fn smithing_button(&mut self, player: &P, button: i32) -> Result<bool> {
        let node = Node::Player(player.0.clone());
        let handled = smithing::interface_button(&mut Smith(Ctx(self)), &node, button);
        self.inner.drain(player);
        handled
    }

    /// The amount prompt answered.
    pub fn smithing_amount(&mut self, script: &SmithScript, value: &Input) -> Result<()> {
        let node = Node::Player(script.0.0.clone());
        script.1.run(&mut Smith(Ctx(self)), &node, value)
    }

    /// A smithing pulse by identity: `Some` with whether the runner drops
    /// it.
    pub fn smithing_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let pulses = &mut self.round.smithing.pulses;
        let at = pulses.iter().position(|(h, _, _)| h.same(pulse))?;
        let (handle, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let result = task.update(&mut Smith(Ctx(self)));
        if matches!(result, Ok(false)) {
            // `setDelay` in `reward`.
            handle.write().delay = task.delay;
            self.round.smithing.pulses.push((handle, owner, task));
        }
        Some(result)
    }

    /// A smithing pulse a pulse manager stopped: `SkillPulse.stop()`.
    pub fn smithing_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Result<()>> {
        let pulses = &mut self.round.smithing.pulses;
        let at = pulses.iter().position(|(h, _, _)| h.same(pulse))?;
        let (_, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        Some(task.stop(&mut Smith(Ctx(self))))
    }

    /// A player who left: its smithing pulses stop and its game attributes
    /// go with it.
    pub fn smithing_logout(&mut self, node: usize) {
        self.round.smithing.attributes.remove(&node);
        self.round.smithing.pulses.retain(|(handle, owner, _)| {
            if *owner == node {
                pulses::stop_base(handle);
            }
            *owner != node
        });
    }
}
