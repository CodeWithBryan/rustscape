//! Cleaning herbs and mixing potions in the live world (`world::herblore`,
//! whose library has the JVM corpus `fixtures/herblore`; this binding has
//! none): HerbCleanListener's generic "clean" option, PotionMixingListener's
//! two `onUseWith` registrations, the skill dialogue `sendSkillDialogue`
//! opens (`dialogue_dsl::skill`) and HerblorePulse on the player's pulse
//! manager.
//!
//! Joined: the quest stages the save holds (Druidic Ritual, Zogre Flesh
//! Eaters and Heroes' Quest by their repository index; nothing in this
//! world advances them), the dynamic level, both kinds of message, the
//! lock, item names, `replaceSlot` of the herb, the experience, the sound,
//! the canonical inventory (`containsItem`, `remove(Item...)`,
//! `add(Item...)`), the animation and its reset, the skillcape perk's
//! attribute read back, `RandomFunction.RANDOM`, the `Consumables` table
//! and the pulse's delay, which the pulse changes itself.
//!
//! Not ported: QuestRequirements. `hasRequirement` answers false by name
//! (no quest is registered in this world), so the potions that ask it
//! (Serum 207, the fishing explosive, Shrink-me-quick, magic essence and
//! Guthix balance) are always refused with the original's message. Nothing
//! in this world sets the skillcape perk's attribute. `replaceSlot`'s
//! second attempt (the herb gone from its slot: the dupe alert and the
//! removal from the equipment) is refused by name.
use super::{
    super::host::Live,
    Ctx, Node, P,
    everyday::Everyday,
    woodcutting::{Bound, bind},
};
use crate::{
    definitions::item::Shared,
    inventory::item::Handle as ItemHandle,
    pulses,
    world::{
        action_locks,
        animation::{Animation, Priority},
        combat_pulse, combat_swing,
        dialogue::{self as d, NodeHandle},
        dialogue_dsl::skill::{Handler, HandlerBody, SkillDialogue},
        herblore::{self, Error, Host, Potion, Pulse, Quest, Result},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        packet_handlers::Container,
    },
};
use rs09_protocol::game::updates;

/// The `on(IntType.ITEM, "clean")` lambda.
pub struct Clean;
impl crate::world::component_objects::NativeObject for Clean {}
/// The two `onUseWith` lambdas' marker.
pub use herblore::Listener as Mixing;
impl crate::world::component_objects::NativeObject for Mixing {}

/// Herblore's state in this world.
#[derive(Default)]
pub struct HerbloreWorld {
    /// The running pulses with their player.
    pulses: Vec<(pulses::Handle, usize, Pulse)>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `herblore::Host` over the live world, for one player.
struct Herbalist<'a> {
    live: &'a mut Live,
    p: P,
}
impl Herbalist<'_> {
    fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    fn items(&mut self, ids: &[i32]) -> crate::inventory::container::Items {
        let everyday = Everyday(Ctx(self.live));
        Shared::new(ids.iter().map(|id| Some(everyday.native(*id, 1))).collect())
    }
}
impl Host for Herbalist<'_> {
    type Item = ItemHandle;

    fn quest_stage(&mut self, quest: Quest) -> Result<i32> {
        // QuestRepository.getStage: the stage the save holds for the
        // quest's index, 0 without one.
        let index = quest.index() as i32;
        let stage = self
            .live
            .inner
            .with_profile(&self.p, |profile| {
                profile.persisted.quests.stages.get(index).copied()
            })
            .map_err(fail)?;
        Ok(stage.unwrap_or(0))
    }
    fn has_requirement(&mut self, quest: &str) -> Result<bool> {
        // No quest is registered in this world: false, counted by name.
        crate::world::login::Host::has_requirement(self.live, &self.p, quest, false).map_err(fail)
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, herblore::HERBLORE).map_err(fail)
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        let node = self.node();
        Everyday(Ctx(self.live))
            .message(&node, message)
            .map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn lock(&mut self, ticks: i32) -> Result<()> {
        let state = &mut self.live.inner.state;
        let locks = state.locks(&self.p.0).map_err(fail)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(fail)
    }
    fn item_name(&mut self, id: i32) -> Result<Option<String>> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let name = item.read().name.clone();
        Ok(name)
    }
    fn slot(&mut self, item: &ItemHandle) -> Result<i32> {
        Ok(item.read().index)
    }
    fn replace_slot(&mut self, slot: i32, product: i32, item: &ItemHandle) -> Result<()> {
        // `cont.remove(currentItem, slot, true)`, then `cont.replace(item,
        // slot)`.
        let node = self.node();
        let product = Everyday(Ctx(self.live)).native(product, 1);
        let replaced =
            Ctx(self.live).with_container(&node, Container::Inventory, |c, defs, ctx| {
                if !c.remove(Some(item), Some(slot), true, defs, ctx)? {
                    return Ok(false);
                }
                c.replace(Some(&product), slot, true, defs, ctx)?;
                Ok(true)
            });
        self.live.inner.drain(&self.p);
        if !replaced.map_err(fail)? {
            return Err(Error::Binding(
                "not ported in the live world: replaceSlot of an item that left its slot".into(),
            ));
        }
        Ok(())
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, herblore::HERBLORE, xp, false);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn add_experience(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, herblore::HERBLORE, xp, true);
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
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().amount(Some(&item)))
    }
    fn contains_item(&mut self, id: i32) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().contains(id, 1))
    }
    fn remove(&mut self, base: i32, ingredient: i32) -> Result<bool> {
        let node = self.node();
        let items = self.items(&[base, ingredient]);
        let removed = Ctx(self.live).with_container(&node, Container::Inventory, |c, defs, ctx| {
            c.remove_many(&items, defs, ctx)
        });
        self.live.inner.drain(&self.p);
        removed.map_err(fail)
    }
    fn add(&mut self, id: i32) -> Result<()> {
        let node = self.node();
        let items = self.items(&[id]);
        let added = Ctx(self.live).with_container(&node, Container::Inventory, |c, defs, ctx| {
            c.add_many(&items, defs, ctx)
        });
        self.live.inner.drain(&self.p);
        added.map(|_| ()).map_err(fail)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn reset_animation(&mut self) -> Result<()> {
        let animation = Animation::with(-1, 0, Some(Priority::High));
        self.live
            .entity_visualize(&self.p, Some(animation), None)
            .map_err(fail)
    }
    fn attribute(&mut self, key: &str) -> Result<bool> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        // A value that is not a Boolean is the Kotlin read's exception.
        attributes.boolean(key, false).or(Err(Error::Cast))
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        // RandomFunction.random(maxValue): 0 without a draw below 1.
        if max <= 0 {
            return Ok(0);
        }
        self.live
            .inner
            .generators
            .shared_next_int(max)
            .ok_or(Error::Null)
    }
    fn consumable_first_id(&mut self, id: i32) -> Result<Option<i32>> {
        let table = self.live.round.eat.table.clone();
        let Some(row) = table.lookup(id) else {
            return Ok(None);
        };
        // `consum.getConsumable().getIds()[0]`.
        let first = table.rows[row].consumable.ids.first().copied();
        first.map(Some).ok_or(Error::Index)
    }
    fn run_pulse(&mut self, potion: Potion, amount: i32) -> Result<()> {
        let p = self.p.clone();
        self.live
            .run_herblore_pulse(&p, Pulse::new(potion, amount))
            .map_err(fail)
    }
    fn open_dialogue(&mut self, potion: Potion) -> Result<()> {
        // sendSkillDialogue with one item: ONE_OPTION.
        let items = vec![Everyday(Ctx(self.live)).native(potion.product, 1)];
        let handler = Handler::new(
            self.p.0.clone(),
            Some(SkillDialogue::OneOption),
            items,
            PotionHandler { potion },
        );
        let opened = handler.open(self.live);
        self.live.inner.drain(&self.p);
        opened.map_err(fail)
    }
}

/// `sendSkillDialogue`'s anonymous SkillDialogueHandler: its potion.
struct PotionHandler {
    potion: Potion,
}
fn dialogue_error(e: Error) -> d::Error {
    match e {
        Error::Null => d::Error::Null,
        Error::Index => d::Error::Index,
        Error::Cast => d::Error::Cast,
        Error::Binding(text) => d::Error::Binding(text),
    }
}
impl PotionHandler {
    fn host<'a>(live: &'a mut Live, player: &NodeHandle) -> d::Result<Herbalist<'a>> {
        if !live.inner.players.contains_key(&player.node) {
            return Err(d::Error::Cast);
        }
        Ok(Herbalist {
            live,
            p: P(player.clone()),
        })
    }
}
impl HandlerBody<Live> for PotionHandler {
    fn create(
        &self,
        host: &mut Live,
        handler: &Handler<Live>,
        amount: i32,
        index: i32,
    ) -> d::Result<()> {
        let mut herbalist = Self::host(host, handler.player())?;
        herblore::create(&mut herbalist, self.potion, amount, index).map_err(dialogue_error)
    }
    fn get_all(&self, host: &mut Live, handler: &Handler<Live>, index: i32) -> d::Result<i32> {
        let mut herbalist = Self::host(host, handler.player())?;
        herblore::get_all(&mut herbalist, &self.potion, index).map_err(dialogue_error)
    }
}

impl Live {
    /// `HerbCleanListener.defineListeners`.
    pub fn define_herb_clean_listener(&mut self) -> std::result::Result<(), String> {
        let option: Vec<u16> = "clean".encode_utf16().collect();
        self.item_actions
            .listeners
            .add_generic(
                &super::super::global::RootCasing,
                &option,
                listeners::Kind::Item.ordinal(),
                Function::new(FunctionKind::On, herblore::CLEAN_LAMBDA, Clean),
            )
            .map_err(|e| format!("{}: {e:?}", herblore::CLEAN_CLASS))
    }

    /// `PotionMixingListener.defineListeners`.
    pub fn define_potion_mixing_listener(&mut self) -> std::result::Result<(), String> {
        for which in Mixing::ALL {
            let (used, with) = which.ids();
            self.item_actions
                .listeners
                .clone()
                .add_use_arrays(
                    listeners::Kind::Item.ordinal(),
                    &used,
                    &with,
                    Function::new(FunctionKind::UseWith, which.lambda(), which),
                )
                .map_err(|e| format!("{}: {e:?}", herblore::MIXING_CLASS))?;
        }
        Ok(())
    }

    /// The "clean" lambda.
    pub fn herb_clean(&mut self, player: &Node, node: &Node) -> Bound<bool> {
        let Node::Player(player) = player else {
            return Err(bind("the clean option by a node that is not a player"));
        };
        // `node as Item`.
        let Node::Item(item) = node else {
            return Err(bind(Error::Cast));
        };
        let p = P(player.clone());
        let id = item.read().id();
        let mut host = Herbalist {
            live: self,
            p: p.clone(),
        };
        let result = herblore::clean(&mut host, id, item);
        self.inner.drain(&p);
        result.map_err(bind)
    }

    /// One of the two mixing lambdas.
    pub fn potion_mixing_use(
        &mut self,
        which: Mixing,
        player: &Node,
        used: &Node,
        with: &Node,
    ) -> Bound<bool> {
        let Node::Player(player) = player else {
            return Err(bind("a potion mixed by a node that is not a player"));
        };
        // `if (used !is Item || with !is Item) return@onUseWith false`.
        let (Node::Item(used), Node::Item(with)) = (used, with) else {
            return Ok(false);
        };
        let p = P(player.clone());
        let (used, with) = (used.read().id(), with.read().id());
        let mut host = Herbalist {
            live: self,
            p: p.clone(),
        };
        let result = herblore::use_with(&mut host, which, used, with);
        self.inner.drain(&p);
        result.map_err(bind)
    }

    /// `player.pulseManager.run(pulse)`: the constructor leaves the pulse
    /// stopped; `start()` runs it when the requirements hold.
    fn run_herblore_pulse(&mut self, p: &P, mut pulse: Pulse) -> Bound<()> {
        self.round
            .herblore
            .pulses
            .retain(|(handle, ..)| handle.read().running);
        // `Pulse(1, player, node)`: the node is an Item, which is never
        // deactivated, and the player leaving stops the pulse.
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            herblore::PULSE_CLASS.into(),
        ));
        let mut started = Ok(());
        let submitted = self
            .run_content_pulse(p, handle.clone(), |live| {
                let mut host = Herbalist { live, p: p.clone() };
                started = pulse.start(&mut host);
                started.is_ok() && pulse.running
            })
            .map_err(bind)?;
        if let Err(error) = started {
            pulses::stop_base(&handle);
            return Err(bind(error));
        }
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            self.round.herblore.pulses.push((handle, p.node(), pulse));
        }
        Ok(())
    }

    /// A herblore pulse by identity: `Some` with whether the runner drops
    /// it.
    pub fn herblore_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.herblore.pulses;
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
        let mut host = Herbalist { live: self, p };
        let result = task.pulse(&mut host);
        // `setDelay`: the pulse's own change of its delay.
        handle.write().delay = task.delay;
        match result {
            Ok(false) => {
                self.round.herblore.pulses.push((handle, owner, task));
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

    /// A herblore pulse a pulse manager stopped: its `stop()`.
    pub fn herblore_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Bound<()>> {
        let pulses = &mut self.round.herblore.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (_, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let mut host = Herbalist { live: self, p };
        Some(task.stop(&mut host).map_err(bind))
    }
}
