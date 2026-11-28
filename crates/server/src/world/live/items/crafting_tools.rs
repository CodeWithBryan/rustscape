//! Gem cutting, limestone bricks, amulet stringing and silver stringing in
//! the live world (`world::crafting_tools`, whose library has the JVM
//! corpus `fixtures/crafting-tools`; this binding has none): GemCutPlugin's,
//! ChiselLimestonePlugin's and AmuletStringingPlugin's `UseWithHandler`
//! registrations, reached through `UseWithHandler.run`'s item-on-item
//! pulse (super::use_with), the gem dialogue (`dialogue_dsl::skill`), the
//! two SkillPulses on the player's pulse manager and
//! SilverStringingListener's `onUseWith` registration.
//!
//! Joined: the canonical inventory (`getAmount`, `containsItem`, `remove`,
//! `add`, ContentAPI's `removeItem` and `addItem`), the dynamic level, the
//! packet dispatch's message, the experience, the synth sound and the
//! animation.
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
        animation::Animation,
        combat_pulse, combat_swing,
        crafting_tools::{self as tools, Error, Gem, Host, Pulse, Result},
        dialogue::{self as d, NodeHandle},
        dialogue_dsl::skill::{Handler, HandlerBody, SkillDialogue},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        packet_handlers::{
            Container,
            use_with::{Event, HandlerData, ITEM_TYPE},
        },
    },
};
use rs09_protocol::game::updates;

/// The three plugins' handler ids in this world (`UseWithHandler.handle`,
/// super::milking's `use_handler_handle`).
pub const AMULET_HANDLER: usize = 6;
pub const LIMESTONE_HANDLER: usize = 7;
pub const GEM_HANDLER: usize = 8;

/// SilverStringingListener's `onUseWith` reference.
pub struct SilverStringing;
impl crate::world::component_objects::NativeObject for SilverStringing {}

/// The state of the two chisel pulses in this world.
#[derive(Default)]
pub struct CraftingToolsWorld {
    /// The running pulses with their player.
    pulses: Vec<(pulses::Handle, usize, Pulse)>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `crafting_tools::Host` over the live world, for one player.
struct Crafter<'a> {
    live: &'a mut Live,
    p: P,
    /// The ids of the two items of the event being handled.
    event: Option<(i32, i32)>,
}
impl Crafter<'_> {
    fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    fn take(&mut self, id: i32) -> Result<bool> {
        let node = self.node();
        let mut everyday = Everyday(Ctx(self.live));
        let item = everyday.native(id, 1);
        let removed = everyday.remove_from_inventory(&node, &item);
        self.live.inner.drain(&self.p);
        removed.map_err(fail)
    }
    fn give(&mut self, id: i32) -> Result<()> {
        let node = self.node();
        let added = Everyday(Ctx(self.live)).add_to_inventory(&node, id, 1);
        self.live.inner.drain(&self.p);
        added.map(|_| ()).map_err(fail)
    }
}
impl Host for Crafter<'_> {
    fn get_amount(&mut self, id: i32) -> Result<i32> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().amount(Some(&item)))
    }
    fn contains_item(&mut self, id: i32) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().contains(id, 1))
    }
    fn remove(&mut self, id: i32) -> Result<bool> {
        self.take(id)
    }
    fn remove_event_items(&mut self) -> Result<bool> {
        // `remove(Item...)`: each of the two, by its id and amount.
        let (used, base) = self.event.ok_or(Error::Null)?;
        let node = self.node();
        let everyday = Everyday(Ctx(self.live));
        let items = Shared::new(vec![
            Some(everyday.native(used, 1)),
            Some(everyday.native(base, 1)),
        ]);
        let removed = Ctx(self.live).with_container(&node, Container::Inventory, |c, defs, ctx| {
            c.remove_many(&items, defs, ctx)
        });
        self.live.inner.drain(&self.p);
        removed.map_err(fail)
    }
    fn add(&mut self, id: i32) -> Result<()> {
        self.give(id)
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, tools::CRAFTING).map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, tools::CRAFTING, xp, true);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        // `playAudio(player, id)`: the synth sound, once, no delay.
        self.live
            .inner
            .queue_native(&self.p, updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn remove_item(&mut self, id: i32) -> Result<bool> {
        self.take(id)
    }
    fn add_item(&mut self, id: i32) -> Result<()> {
        self.give(id)
    }
    fn open_dialogue(&mut self, gem: &'static Gem) -> Result<()> {
        let items = vec![Everyday(Ctx(self.live)).native(gem.gem, 1)];
        let handler = Handler::new(
            self.p.0.clone(),
            Some(SkillDialogue::OneOption),
            items,
            GemHandler { gem },
        );
        let opened = handler.open(self.live);
        self.live.inner.drain(&self.p);
        opened.map_err(fail)
    }
    fn run_pulse(&mut self, pulse: Pulse) -> Result<()> {
        let p = self.p.clone();
        self.live.run_crafting_tool_pulse(&p, pulse).map_err(fail)
    }
}

/// GemCutPlugin's anonymous SkillDialogueHandler: its gem.
struct GemHandler {
    gem: &'static Gem,
}
fn dialogue_error(e: Error) -> d::Error {
    match e {
        Error::Null => d::Error::Null,
        Error::Cast => d::Error::Cast,
        Error::Binding(text) => d::Error::Binding(text),
    }
}
impl GemHandler {
    fn host<'a>(live: &'a mut Live, player: &NodeHandle) -> d::Result<Crafter<'a>> {
        if !live.inner.players.contains_key(&player.node) {
            return Err(d::Error::Cast);
        }
        Ok(Crafter {
            live,
            p: P(player.clone()),
            event: None,
        })
    }
}
impl HandlerBody<Live> for GemHandler {
    fn create(
        &self,
        host: &mut Live,
        handler: &Handler<Live>,
        amount: i32,
        _index: i32,
    ) -> d::Result<()> {
        let mut crafter = Self::host(host, handler.player())?;
        tools::gem_create(&mut crafter, self.gem, amount).map_err(dialogue_error)
    }
    fn get_all(&self, host: &mut Live, handler: &Handler<Live>, _index: i32) -> d::Result<i32> {
        let mut crafter = Self::host(host, handler.player())?;
        tools::gem_get_all(&mut crafter, self.gem).map_err(dialogue_error)
    }
}

impl Live {
    /// `AmuletStringingPlugin.newInstance(null)`,
    /// `ChiselLimestonePlugin.newInstance(null)` and
    /// `GemCutPlugin.newInstance(null)`, each if the class scan names it,
    /// in the scan's order.
    pub fn install_crafting_tools(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        let plugins: [(&str, &str, &[i32], i32, usize); 3] = [
            (
                tools::AMULET_CLASS,
                "AmuletStringingPlugin",
                &tools::AMULET_ALLOWED,
                tools::BALL_OF_WOOL,
                AMULET_HANDLER,
            ),
            (
                tools::LIMESTONE_CLASS,
                "ChiselLimestonePlugin",
                &tools::LIMESTONE_ALLOWED,
                tools::CHISEL,
                LIMESTONE_HANDLER,
            ),
            (
                tools::GEM_CLASS,
                "GemCutPlugin",
                &tools::GEM_ALLOWED,
                tools::CHISEL,
                GEM_HANDLER,
            ),
        ];
        for (class, name, allowed, key, handler) in plugins {
            if !scanned.contains(&class) {
                continue;
            }
            let handler = Shared::new(HandlerData {
                name: name.into(),
                allowed_nodes: Some(allowed.to_vec()),
                dynamic: false,
                handler,
            });
            self.item_actions
                .use_handlers
                .add_handler(key, ITEM_TYPE, &handler);
        }
        Ok(())
    }

    /// `SilverStringingListener.defineListeners`.
    pub fn define_silver_stringing_listener(&mut self) -> std::result::Result<(), String> {
        self.item_actions
            .listeners
            .clone()
            .add_use_ids(
                listeners::Kind::Item.ordinal(),
                tools::BALL_OF_WOOL,
                &tools::STRINGABLE,
                Function::new(FunctionKind::UseWith, tools::SILVER_LAMBDA, SilverStringing),
            )
            .map_err(|e| format!("{}: {e:?}", tools::SILVER_CLASS))
    }

    /// `handle(event)` of one of the three plugins.
    pub fn crafting_tool_handle(
        &mut self,
        handler: usize,
        player: &P,
        event: &Event<Node>,
    ) -> Bound<bool> {
        // `event.getUsedItem()` and `event.getBaseItem()`: both are items,
        // the handlers are keyed by ITEM_TYPE.
        let (Node::Item(used), Node::Item(base)) = (&event.used, &event.with) else {
            return Err(bind(Error::Null));
        };
        let (used, base) = (used.read().id(), base.read().id());
        let mut host = Crafter {
            live: self,
            p: player.clone(),
            event: Some((used, base)),
        };
        let result = match handler {
            GEM_HANDLER => tools::gem_handle(&mut host, used, base),
            LIMESTONE_HANDLER => tools::limestone_handle(&mut host),
            _ => tools::amulet_handle(&mut host, used, base),
        };
        self.inner.drain(player);
        result.map_err(bind)
    }

    /// SilverStringingListener's `stringSilverProduct(player, used, with)`.
    pub fn silver_stringing_use(&mut self, player: &Node, used: &Node, with: &Node) -> Bound<bool> {
        let Node::Player(player) = player else {
            return Err(bind("silver strung by a node that is not a player"));
        };
        // `with.id` and `used.id`: the registration is of items.
        let (Node::Item(used), Node::Item(with)) = (used, with) else {
            return Err(bind("silver strung with a node that is not an item"));
        };
        let p = P(player.clone());
        let (used, with) = (used.read().id(), with.read().id());
        let mut host = Crafter {
            live: self,
            p: p.clone(),
            event: None,
        };
        let result = tools::string_silver(&mut host, used, with);
        self.inner.drain(&p);
        result.map_err(bind)
    }

    /// `player.getPulseManager().run(pulse)`: the constructor leaves the
    /// pulse stopped; `start()` runs it when the requirements hold.
    fn run_crafting_tool_pulse(&mut self, p: &P, mut pulse: Pulse) -> Bound<()> {
        self.round
            .crafting_tools
            .pulses
            .retain(|(handle, ..)| handle.read().running);
        // `Pulse(1, player, node)`: the node is an Item, which is never
        // deactivated, and the player leaving stops the pulse.
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            pulse.class_name().into(),
        ));
        let mut started = Ok(());
        let submitted = self
            .run_content_pulse(p, handle.clone(), |live| {
                let mut host = Crafter {
                    live,
                    p: p.clone(),
                    event: None,
                };
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
                .crafting_tools
                .pulses
                .push((handle, p.node(), pulse));
        }
        Ok(())
    }

    /// A gem or limestone pulse by identity: `Some` with whether the
    /// runner drops it.
    pub fn crafting_tool_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.crafting_tools.pulses;
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
        let mut host = Crafter {
            live: self,
            p,
            event: None,
        };
        match task.pulse(&mut host) {
            Ok(false) => {
                self.round.crafting_tools.pulses.push((handle, owner, task));
                Some(Ok(false))
            }
            finished => {
                // `stop()` with `resetAnimation` false: the base flag only.
                task.stop();
                pulses::stop_base(&handle);
                Some(finished.map(|_| true).map_err(bind))
            }
        }
    }

    /// A gem or limestone pulse a pulse manager stopped: its `stop()`,
    /// which resets nothing.
    pub fn crafting_tool_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Bound<()>> {
        let pulses = &mut self.round.crafting_tools.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        pulses.remove(at);
        Some(Ok(()))
    }
}
