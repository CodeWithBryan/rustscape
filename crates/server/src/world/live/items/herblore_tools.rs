//! Grinding, herb tar and barbarian mixes in the live world
//! (`world::herblore_tools`, whose library has the JVM corpus
//! `fixtures/herblore-tools`; this binding has none): GrindItemPlugin's and
//! HerbTarPlugin's `UseWithHandler` registrations, reached through
//! `UseWithHandler.run`'s item-on-item pulse (super::use_with), their
//! skill dialogues (`dialogue_dsl::skill`) with the `RepositionChild` each
//! sends, their two SkillPulses on the player's pulse manager and
//! BarbarianMixListener's `onUseWith` registrations.
//!
//! Joined: ContentAPI's `amountInInventory`, `removeItem` and `addItem`
//! over the canonical inventory, the inventory's own `containsItem`,
//! `remove`, `add` and `getAmount`, the animation and its reset, the
//! Druidic Ritual stage the save holds (nothing in this world advances
//! it), both levels, both kinds of message, `sendDialogue`, both kinds of
//! experience, item names, the barbarian training attribute (set by Otto's
//! dialogue, super::otto) and the pulses' delays (the grinding pulse's 2,
//! the tar pulse's own change to 4).
//!
//! Not ported: SwampToadPlugin (inventory item option handlers are refused
//! by name in this world).
use super::{
    super::host::Live,
    Ctx, Node, P,
    everyday::Everyday,
    woodcutting::{Bound, bind},
};
use crate::{
    definitions::item::Shared,
    pulses,
    world::{
        animation::{Animation, Priority},
        combat_pulse, combat_swing,
        dialogue::{self as d, NodeHandle},
        dialogue_dsl::skill::{Handler, HandlerBody, SkillDialogue},
        herblore::Quest,
        herblore_tools::{self as tools, Dialogue, Error, Host, Pulse, Result},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        packet_handlers::use_with::{Event, HandlerData, ITEM_TYPE, UseHandler},
    },
};

/// GrindItemPlugin's and HerbTarPlugin's handler ids in this world
/// (`UseWithHandler.handle`, super::milking's `use_handler_handle`).
pub const GRIND_HANDLER: usize = 4;
pub const TAR_HANDLER: usize = 5;

/// The two `onUseWith` lambdas of BarbarianMixListener.
pub struct Mix;
impl crate::world::component_objects::NativeObject for Mix {}

/// The state of grinding and herb tar in this world.
#[derive(Default)]
pub struct ToolsWorld {
    /// `new GrindItemPlugin()` and `new HerbTarPlugin()`.
    grind: Option<UseHandler>,
    tar: Option<UseHandler>,
    /// The running pulses with their player.
    pulses: Vec<(pulses::Handle, usize, Pulse)>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `herblore_tools::Host` over the live world, for one player.
struct Grinder<'a> {
    live: &'a mut Live,
    p: P,
}
impl Grinder<'_> {
    fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    fn held(&mut self, id: i32) -> i32 {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let player = &self.live.inner.canonical(&self.p).player;
        player.inventory().amount(Some(&item))
    }
    fn take(&mut self, id: i32, amount: i32) -> Result<bool> {
        let node = self.node();
        let mut everyday = Everyday(Ctx(self.live));
        let item = everyday.native(id, amount);
        let removed = everyday.remove_from_inventory(&node, &item);
        self.live.inner.drain(&self.p);
        removed.map_err(fail)
    }
    fn give(&mut self, id: i32, amount: i32) -> Result<()> {
        let node = self.node();
        let added = Everyday(Ctx(self.live)).add_to_inventory(&node, id, amount);
        self.live.inner.drain(&self.p);
        added.map(|_| ()).map_err(fail)
    }
    fn visualize(&mut self, animation: Animation) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(animation), None)
            .map_err(fail)
    }
}
impl Host for Grinder<'_> {
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        Ok(self.held(id))
    }
    fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        self.take(id, amount)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()> {
        self.give(id, amount)
    }
    fn animator_animate(&mut self, id: i32) -> Result<()> {
        // `player.animate(animation)` is `animator.animate(animation)`.
        self.visualize(Animation::new(id))
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.visualize(Animation::new(id))
    }
    fn reset_animation(&mut self) -> Result<()> {
        self.visualize(Animation::with(-1, 0, Some(Priority::High)))
    }
    fn druidic_ritual_stage(&mut self) -> Result<i32> {
        // QuestRepository.getStage: the stage the save holds for the
        // quest's index, 0 without one.
        let index = Quest::DruidicRitual.index() as i32;
        let stage = self
            .live
            .inner
            .with_profile(&self.p, |profile| {
                profile.persisted.quests.stages.get(index).copied()
            })
            .map_err(fail)?;
        Ok(stage.unwrap_or(0))
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, tools::HERBLORE).map_err(fail)
    }
    fn static_level(&mut self) -> Result<i32> {
        combat_swing::Host::static_level(self.live, &self.p, tools::HERBLORE).map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        let node = self.node();
        Everyday(Ctx(self.live))
            .message(&node, message)
            .map_err(fail)
    }
    fn send_dialogue(&mut self, message: &str) -> Result<()> {
        let node = self.node();
        Everyday(Ctx(self.live))
            .dialogue(&node, message)
            .map_err(fail)
    }
    fn contains_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().contains(id, amount))
    }
    fn remove(&mut self, id: i32, amount: i32) -> Result<bool> {
        self.take(id, amount)
    }
    fn add(&mut self, id: i32, amount: i32) -> Result<()> {
        self.give(id, amount)
    }
    fn get_amount(&mut self, id: i32) -> Result<i32> {
        Ok(self.held(id))
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, tools::HERBLORE, xp, true);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, tools::HERBLORE, xp, false);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn item_name(&mut self, id: i32) -> Result<Option<String>> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let name = item.read().name.clone();
        Ok(name)
    }
    fn attribute_int(&mut self, key: &str) -> Result<i32> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        // A value of another type is the Kotlin read's exception.
        attributes.integer(key, 0).or(Err(Error::Cast))
    }
    fn set_attribute_int(&mut self, key: &str, value: i32) -> Result<()> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.set_integer(key, value);
        Ok(())
    }
    fn open_dialogue(&mut self, dialogue: Dialogue) -> Result<()> {
        let items = vec![Everyday(Ctx(self.live)).native(dialogue.item(), 1)];
        let handler = Handler::new(
            self.p.0.clone(),
            Some(SkillDialogue::OneOption),
            items,
            ToolHandler { dialogue },
        );
        let opened = handler.open(self.live);
        self.live.inner.drain(&self.p);
        opened.map_err(fail)
    }
    fn reposition_child(&mut self, interface: i32, child: i32, x: i32, y: i32) -> Result<()> {
        let sent = d::Host::send_reposition_on_interface(
            self.live,
            self.p.clone(),
            interface,
            child,
            x,
            y,
        );
        self.live.inner.drain(&self.p);
        sent.map_err(fail)
    }
    fn run_pulse(&mut self, pulse: Pulse) -> Result<()> {
        let p = self.p.clone();
        self.live.run_tool_pulse(&p, pulse).map_err(fail)
    }
}

/// A plugin's anonymous SkillDialogueHandler: what it captured.
struct ToolHandler {
    dialogue: Dialogue,
}
fn dialogue_error(e: Error) -> d::Error {
    match e {
        Error::Null => d::Error::Null,
        Error::Cast => d::Error::Cast,
        Error::Binding(text) => d::Error::Binding(text),
    }
}
impl ToolHandler {
    fn host<'a>(live: &'a mut Live, player: &NodeHandle) -> d::Result<Grinder<'a>> {
        if !live.inner.players.contains_key(&player.node) {
            return Err(d::Error::Cast);
        }
        Ok(Grinder {
            live,
            p: P(player.clone()),
        })
    }
}
impl HandlerBody<Live> for ToolHandler {
    fn create(
        &self,
        host: &mut Live,
        handler: &Handler<Live>,
        amount: i32,
        _index: i32,
    ) -> d::Result<()> {
        let mut grinder = Self::host(host, handler.player())?;
        tools::create(&mut grinder, self.dialogue, amount).map_err(dialogue_error)
    }
    fn get_all(&self, host: &mut Live, handler: &Handler<Live>, _index: i32) -> d::Result<i32> {
        let mut grinder = Self::host(host, handler.player())?;
        tools::get_all(&mut grinder, self.dialogue).map_err(dialogue_error)
    }
}

impl Live {
    /// `GrindItemPlugin.newInstance(null)` and
    /// `HerbTarPlugin.newInstance(null)`, each if the class scan names it,
    /// in the scan's order (the grinding plugin first).
    pub fn install_herblore_tools(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if scanned.contains(&tools::GRIND_CLASS) {
            let handler = Shared::new(HandlerData {
                name: "GrindItemPlugin".into(),
                allowed_nodes: Some(vec![tools::PESTLE_AND_MORTAR]),
                dynamic: false,
                handler: GRIND_HANDLER,
            });
            for id in tools::grind_handler_ids() {
                self.item_actions
                    .use_handlers
                    .add_handler(id, ITEM_TYPE, &handler);
            }
            self.round.herblore_tools.grind = Some(handler);
        }
        if scanned.contains(&tools::TAR_CLASS) {
            let handler = Shared::new(HandlerData {
                name: "HerbTarPlugin".into(),
                allowed_nodes: Some(vec![tools::SWAMP_TAR]),
                dynamic: false,
                handler: TAR_HANDLER,
            });
            for id in tools::tar_handler_ids() {
                self.item_actions
                    .use_handlers
                    .add_handler(id, ITEM_TYPE, &handler);
            }
            self.round.herblore_tools.tar = Some(handler);
        }
        Ok(())
    }

    /// `BarbarianMixListener.defineListeners`.
    pub fn define_barbarian_mix_listener(&mut self) -> std::result::Result<(), String> {
        for (lambda, used, with) in tools::mix_registrations() {
            self.item_actions
                .listeners
                .clone()
                .add_use_ids(
                    listeners::Kind::Item.ordinal(),
                    used,
                    &[with],
                    Function::new(FunctionKind::UseWith, lambda, Mix),
                )
                .map_err(|e| format!("{}: {e:?}", tools::MIX_CLASS))?;
        }
        Ok(())
    }

    /// `GrindItemPlugin.handle(event)` or `HerbTarPlugin.handle(event)`.
    pub fn herblore_tool_handle(
        &mut self,
        handler: usize,
        player: &P,
        event: &Event<Node>,
    ) -> Bound<bool> {
        // `event.usedItem.id` and `event.baseItem`: both are items, the
        // handlers are keyed by ITEM_TYPE.
        let (Node::Item(used), Node::Item(base)) = (&event.used, &event.with) else {
            return Err(bind(Error::Null));
        };
        let (used, base) = (used.read().id(), base.read().id());
        let mut host = Grinder {
            live: self,
            p: player.clone(),
        };
        let result = if handler == GRIND_HANDLER {
            tools::grind_handle(&mut host, used, base)
        } else {
            tools::tar_handle(&mut host, used, base)
        };
        self.inner.drain(player);
        result.map_err(bind)
    }

    /// One of BarbarianMixListener's lambdas: `handle(player, used, with)`.
    pub fn barbarian_mix_use(&mut self, player: &Node, used: &Node, with: &Node) -> Bound<bool> {
        let Node::Player(player) = player else {
            return Err(bind("a barbarian mix by a node that is not a player"));
        };
        let p = P(player.clone());
        // `inputPotion.getId()` and `egg.getId()`: the registrations are
        // of items.
        let (Node::Item(used), Node::Item(with)) = (used, with) else {
            return Err(bind("a barbarian mix of a node that is not an item"));
        };
        let (used, with) = (used.read().id(), with.read().id());
        let mut host = Grinder {
            live: self,
            p: p.clone(),
        };
        let result = tools::mix(&mut host, used, with);
        self.inner.drain(&p);
        result.map_err(bind)
    }

    /// `player.pulseManager.run(pulse)`: the constructor leaves the pulse
    /// stopped; `start()` runs it when the requirements hold.
    fn run_tool_pulse(&mut self, p: &P, mut pulse: Pulse) -> Bound<()> {
        self.round
            .herblore_tools
            .pulses
            .retain(|(handle, ..)| handle.read().running);
        // `Pulse(1, player, node)`: the node is an Item or null, neither of
        // which stops the pulse, and the player leaving does. The grinding
        // pulse's `setDelay(2)` is the delay it is made with here.
        let handle = Shared::new(pulses::Pulse::new(
            pulse.delay(),
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            pulse.class_name().into(),
        ));
        let mut started = Ok(());
        let submitted = self
            .run_content_pulse(p, handle.clone(), |live| {
                let mut host = Grinder { live, p: p.clone() };
                started = pulse.start(&mut host);
                started.is_ok() && pulse.running()
            })
            .map_err(bind)?;
        if let Err(error) = started {
            pulses::stop_base(&handle);
            return Err(bind(error));
        }
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            self.round
                .herblore_tools
                .pulses
                .push((handle, p.node(), pulse));
        }
        Ok(())
    }

    /// A grinding or herb tar pulse by identity: `Some` with whether the
    /// runner drops it.
    pub fn herblore_tool_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.herblore_tools.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (handle, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let mut host = Grinder { live: self, p };
        let result = task.pulse(&mut host);
        // `setDelay`: the pulse's own change of its delay.
        handle.write().delay = task.delay();
        match result {
            Ok(false) => {
                self.round.herblore_tools.pulses.push((handle, owner, task));
                Some(Ok(false))
            }
            finished => {
                // `stop()`: the animation reset and the base flag.
                let stopped = task.stop(&mut host);
                pulses::stop_base(&handle);
                Some(finished.and_then(|_| stopped.map(|()| true)).map_err(bind))
            }
        }
    }

    /// A grinding or herb tar pulse a pulse manager stopped: its `stop()`.
    pub fn herblore_tool_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Bound<()>> {
        let pulses = &mut self.round.herblore_tools.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (_, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let mut host = Grinder { live: self, p };
        Some(task.stop(&mut host).map_err(bind))
    }
}
