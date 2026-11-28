//! Battlestaves, glassblowing, molten glass and spiky vambraces in the
//! live world (`world::crafting_wares`, whose library has the JVM corpus
//! `fixtures/crafting-wares`; this binding has none): BattlestaffListener's,
//! GlassCraftingListener's and GlassMakeListener's `onUseWith`
//! registrations, GlassCraftingListener's `on(542)`, the two skill
//! dialogues (`dialogue_dsl::skill`), the amount prompt, `runTask`'s pulse
//! and the two glass pulses on the player's pulse manager, and
//! SpikyVambraces' `UseWithHandler` registrations.
//!
//! Joined: the canonical inventory, the dynamic level, messages, the
//! experience, the Varrock diary, the animator, the synth sound and the
//! interface manager. Not joined: the ResourceProducedEvent (entity event
//! hooks are not ported; skipped by name).
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
        crafting_wares::{
            self as wares, Battlestaff, Error, Glass, GlassMakePulse, GlassPulse, Host, Result,
        },
        dialogue::{self as d, NodeHandle},
        dialogue_dsl::skill::{Handler, HandlerBody, SkillDialogue},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        interface_listeners::{Button, Callback},
        packet_handlers::use_with::{Event, HandlerData, ITEM_TYPE},
        player_managers::diary,
    },
};
use rs09_protocol::game::updates;

/// SpikyVambraces' handler id in this world (`UseWithHandler.handle`,
/// super::milking's `use_handler_handle`).
pub const VAMBRACES_HANDLER: usize = 9;

/// The three listeners' `onUseWith` lambdas and the `on(542)` lambda.
pub struct BattlestaffUse;
impl crate::world::component_objects::NativeObject for BattlestaffUse {}
pub struct GlassUse;
impl crate::world::component_objects::NativeObject for GlassUse {}
pub struct GlassMakeUse;
impl crate::world::component_objects::NativeObject for GlassMakeUse {}
pub struct GlassButtons;
impl crate::world::component_objects::NativeObject for GlassButtons {}

/// The amount prompt's script: the player and the product.
#[derive(Clone)]
pub struct GlassScript(pub P, pub &'static Glass);

/// One of the three pulses, as a pulse manager holds it.
pub enum Pulse {
    /// `runTask`'s pulse: the product, the task's amount and the cycles
    /// left.
    Task(&'static Battlestaff, i32, i32),
    Glass(GlassPulse),
    GlassMake(GlassMakePulse),
}
impl Pulse {
    fn class_name(&self) -> &'static str {
        match self {
            Self::Task(..) => "core.api.ContentAPIKt$runTask$1",
            Self::Glass(_) => wares::GLASS_PULSE_CLASS,
            Self::GlassMake(_) => wares::GLASS_MAKE_PULSE_CLASS,
        }
    }
    fn delay(&self) -> i32 {
        match self {
            Self::Task(..) => 2,
            Self::Glass(p) => p.delay,
            Self::GlassMake(p) => p.delay,
        }
    }
    fn pulse<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        match self {
            Self::Task(staff, amount, cycles) => {
                // `task.invoke()`, then `--cycles <= 0`.
                wares::battlestaff_task(h, staff, *amount)?;
                *cycles = cycles.wrapping_sub(1);
                Ok(*cycles <= 0)
            }
            Self::Glass(p) => p.pulse(h),
            Self::GlassMake(p) => p.pulse(h),
        }
    }
}

/// The state of the pulses in this world.
#[derive(Default)]
pub struct CraftingWaresWorld {
    /// The running pulses with their player.
    pulses: Vec<(pulses::Handle, usize, Pulse)>,
}

fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `crafting_wares::Host` over the live world, for one player.
struct Crafter<'a> {
    live: &'a mut Live,
    p: P,
}
impl Crafter<'_> {
    fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    fn open_skill_dialogue(&mut self, item: i32, body: Dialogue) -> Result<()> {
        // sendSkillDialogue with one item: ONE_OPTION.
        let items = vec![Everyday(Ctx(self.live)).native(item, 1)];
        let handler = Handler::new(
            self.p.0.clone(),
            Some(SkillDialogue::OneOption),
            items,
            body,
        );
        let opened = handler.open(self.live);
        self.live.inner.drain(&self.p);
        opened.map_err(fail)
    }
}
impl Host for Crafter<'_> {
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().amount(Some(&item)))
    }
    fn in_inventory(&mut self, id: i32) -> Result<bool> {
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().contains(id, 1))
    }
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, wares::CRAFTING).map_err(fail)
    }
    fn send_message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
    fn remove_item(&mut self, id: i32) -> Result<bool> {
        let node = self.node();
        let mut everyday = Everyday(Ctx(self.live));
        let item = everyday.native(id, 1);
        let removed = everyday.remove_from_inventory(&node, &item);
        self.live.inner.drain(&self.p);
        removed.map_err(fail)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<()> {
        let node = self.node();
        let added = Everyday(Ctx(self.live)).add_to_inventory(&node, id, amount);
        self.live.inner.drain(&self.p);
        added.map(|_| ()).map_err(fail)
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        // `addExperience(skill, xp)`: `playerMod` false.
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, wares::CRAFTING, xp, false);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn finish_varrock_task(&mut self) -> Result<()> {
        // DiaryType.VARROCK is the second constant.
        let finished = super::super::subclasses::with_diaries(self.live, &self.p, |h, owner| {
            diary::finish_task(h, owner, owner, Some(1), 2, 6)
        });
        self.live.inner.drain(&self.p);
        finished.map_err(fail)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn play_audio(&mut self, id: i32) -> Result<()> {
        // `playAudio(player, id)`: the synth sound, once, no delay.
        self.live
            .inner
            .queue_native(&self.p, updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn resource_produced(&mut self, _id: i32, _amount: i32) -> Result<()> {
        self.live
            .inner
            .not_ported
            .skip("ResourceProducedEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn open_interface(&mut self, id: i32) -> Result<()> {
        crate::world::player_death::Host::open_interface(self.live, &self.p.0, id).map_err(fail)
    }
    fn close_interface(&mut self) -> Result<()> {
        crate::world::player_death::Host::close_interface(self.live, &self.p.0).map_err(fail)
    }
    fn open_battlestaff_dialogue(
        &mut self,
        staff: &'static Battlestaff,
        used: i32,
        with: i32,
    ) -> Result<()> {
        self.open_skill_dialogue(staff.product, Dialogue::Staff(staff, used, with))
    }
    fn run_battlestaff_task(
        &mut self,
        staff: &'static Battlestaff,
        repeats: i32,
        amount: i32,
    ) -> Result<()> {
        let p = self.p.clone();
        self.live
            .run_crafting_ware_pulse(&p, Pulse::Task(staff, amount, repeats))
            .map_err(fail)
    }
    fn send_glass_input(&mut self, glass: &'static Glass) -> Result<()> {
        let p = self.p.clone();
        self.live
            .glass_prompt(&p, "Enter the amount:", GlassScript(p.clone(), glass))
            .map_err(fail)
    }
    fn submit_glass_pulse(&mut self, pulse: GlassPulse) -> Result<()> {
        let p = self.p.clone();
        self.live
            .run_crafting_ware_pulse(&p, Pulse::Glass(pulse))
            .map_err(fail)
    }
    fn open_glass_make_dialogue(&mut self) -> Result<()> {
        self.open_skill_dialogue(wares::MOLTEN_GLASS, Dialogue::GlassMake)
    }
    fn submit_glass_make_pulse(&mut self, pulse: GlassMakePulse) -> Result<()> {
        let p = self.p.clone();
        self.live
            .run_crafting_ware_pulse(&p, Pulse::GlassMake(pulse))
            .map_err(fail)
    }
}

/// `sendSkillDialogue`'s anonymous SkillDialogueHandler with one of the
/// two builders.
enum Dialogue {
    Staff(&'static Battlestaff, i32, i32),
    GlassMake,
}
fn dialogue_error(e: Error) -> d::Error {
    match e {
        Error::Null => d::Error::Null,
        Error::Cast => d::Error::Cast,
        Error::Binding(text) => d::Error::Binding(text),
    }
}
fn crafter<'a>(live: &'a mut Live, player: &NodeHandle) -> d::Result<Crafter<'a>> {
    if !live.inner.players.contains_key(&player.node) {
        return Err(d::Error::Cast);
    }
    Ok(Crafter {
        live,
        p: P(player.clone()),
    })
}
impl HandlerBody<Live> for Dialogue {
    fn create(
        &self,
        host: &mut Live,
        handler: &Handler<Live>,
        amount: i32,
        index: i32,
    ) -> d::Result<()> {
        // `builder.items[index].id`: the one item.
        if index != 0 {
            return Err(d::Error::Binding(format!(
                "ArrayIndexOutOfBoundsException: skill dialogue item {index}"
            )));
        }
        let mut host = crafter(host, handler.player())?;
        match self {
            Self::Staff(staff, used, with) => {
                wares::battlestaff_create(&mut host, staff, *used, *with, amount)
            }
            Self::GlassMake => wares::glass_make_create(&mut host, wares::MOLTEN_GLASS, amount),
        }
        .map_err(dialogue_error)
    }
    fn get_all(&self, host: &mut Live, handler: &Handler<Live>, index: i32) -> d::Result<i32> {
        if index != 0 {
            return Err(d::Error::Binding(format!(
                "ArrayIndexOutOfBoundsException: skill dialogue item {index}"
            )));
        }
        let mut host = crafter(host, handler.player())?;
        match self {
            Self::Staff(_, used, with) => wares::battlestaff_max(&mut host, *used, *with),
            Self::GlassMake => wares::glass_make_max(&mut host),
        }
        .map_err(dialogue_error)
    }
}

impl Live {
    /// `SpikyVambraces.newInstance(null)`, if the class scan names it.
    pub fn install_crafting_wares(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if !scanned.contains(&wares::VAMBRACES_CLASS) {
            return Ok(());
        }
        let handler = Shared::new(HandlerData {
            name: "SpikyVambraces".into(),
            allowed_nodes: Some(vec![wares::KEBBIT_CLAWS]),
            dynamic: false,
            handler: VAMBRACES_HANDLER,
        });
        for (vamb, ..) in wares::VAMBRACES {
            self.item_actions
                .use_handlers
                .add_handler(vamb, ITEM_TYPE, &handler);
        }
        Ok(())
    }

    /// `BattlestaffListener.defineListeners`.
    pub fn define_battlestaff_listener(&mut self) -> std::result::Result<(), String> {
        self.item_actions
            .listeners
            .clone()
            .add_use_arrays(
                listeners::Kind::Item.ordinal(),
                &wares::ORBS,
                &[wares::BATTLESTAFF],
                Function::new(
                    FunctionKind::UseWith,
                    wares::BATTLESTAFF_LAMBDA,
                    BattlestaffUse,
                ),
            )
            .map_err(|e| format!("{}: {e:?}", wares::BATTLESTAFF_CLASS))
    }

    /// `GlassCraftingListener.defineListeners` and
    /// `defineInterfaceListeners`.
    pub fn define_glass_listener(&mut self) -> std::result::Result<(), String> {
        self.item_actions
            .listeners
            .clone()
            .add_use_ids(
                listeners::Kind::Item.ordinal(),
                wares::GLASSBLOWING_PIPE,
                &[wares::MOLTEN_GLASS],
                Function::new(FunctionKind::UseWith, wares::GLASS_LAMBDA, GlassUse),
            )
            .map_err(|e| format!("{}: {e:?}", wares::GLASS_CLASS))?;
        self.listeners
            .add(wares::GLASS_INTERFACE, None, Callback::button(GlassButtons))
            .map_err(|e| format!("{}: {e:?}", wares::GLASS_CLASS))
    }

    /// `GlassMakeListener.defineListeners`.
    pub fn define_glass_make_listener(&mut self) -> std::result::Result<(), String> {
        self.item_actions
            .listeners
            .clone()
            .add_use_arrays(
                listeners::Kind::Scenery.ordinal(),
                &wares::GLASS_MAKE_INPUTS,
                &wares::FURNACES,
                Function::new(
                    FunctionKind::UseWith,
                    wares::GLASS_MAKE_LAMBDA,
                    GlassMakeUse,
                ),
            )
            .map_err(|e| format!("{}: {e:?}", wares::GLASS_MAKE_CLASS))
    }

    /// One of the three use-with lambdas, if the function is one.
    pub fn crafting_ware_use(
        &mut self,
        f: &Function,
        player: &Node,
        used: &Node,
        with: &Node,
    ) -> Option<Bound<bool>> {
        let staff = f.value::<BattlestaffUse>().is_some();
        let glass = f.value::<GlassUse>().is_some();
        let make = f.value::<GlassMakeUse>().is_some();
        if !staff && !glass && !make {
            return None;
        }
        let Node::Player(player) = player else {
            return Some(Err(bind(
                "a crafting ware used by a node that is not a player",
            )));
        };
        let p = P(player.clone());
        let mut host = Crafter {
            live: self,
            p: p.clone(),
        };
        let result = if staff {
            // `used.id` and `with.id`: the registration is of items.
            let (Node::Item(used), Node::Item(with)) = (used, with) else {
                return Some(Err(bind(
                    "a battlestaff made with a node that is not an item",
                )));
            };
            let (used, with) = (used.read().id(), with.read().id());
            wares::battlestaff_use(&mut host, used, with)
        } else if glass {
            wares::glass_use(&mut host)
        } else {
            wares::glass_make_use(&mut host)
        };
        self.inner.drain(&p);
        Some(result.map_err(bind))
    }

    /// GlassCraftingListener's `on(542)`, if the callback is it.
    pub fn glass_button(
        &mut self,
        callback: &Callback,
        owner: &NodeHandle,
        button: Button,
    ) -> Option<Bound<bool>> {
        callback.value::<GlassButtons>()?;
        let p = P(owner.clone());
        let mut host = Crafter {
            live: self,
            p: p.clone(),
        };
        let result = wares::glass_button(&mut host, button.opcode, button.button);
        self.inner.drain(&p);
        Some(result.map_err(bind))
    }

    /// The glassblowing amount prompt answered with a number.
    pub fn glass_amount(&mut self, script: &GlassScript, value: i32) -> Bound<()> {
        let p = script.0.clone();
        let mut host = Crafter {
            live: self,
            p: p.clone(),
        };
        let result = wares::glass_input(&mut host, script.1, value);
        self.inner.drain(&p);
        result.map_err(bind)
    }

    /// `SpikyVambraces.handle(event)`.
    pub fn crafting_ware_handle(&mut self, player: &P, event: &Event<Node>) -> Bound<bool> {
        // `event.usedWith.id`: the handlers are keyed by ITEM_TYPE.
        let Node::Item(with) = &event.with else {
            return Err(bind(Error::Null));
        };
        let with = with.read().id();
        let mut host = Crafter {
            live: self,
            p: player.clone(),
        };
        let result = wares::vambraces_handle(&mut host, with);
        self.inner.drain(player);
        result.map_err(bind)
    }

    /// `entity.pulseManager.run(pulse)` of one of the three pulses; none
    /// overrides `start()`.
    fn run_crafting_ware_pulse(&mut self, p: &P, pulse: Pulse) -> Bound<()> {
        self.round
            .crafting_wares
            .pulses
            .retain(|(handle, ..)| handle.read().running);
        // `Pulse()` and `Pulse(delay)`: no node is checked, and the player
        // leaving stops the pulse.
        let handle = Shared::new(pulses::Pulse::new(
            pulse.delay(),
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            pulse.class_name().into(),
        ));
        let submitted = self
            .run_content_pulse(p, handle.clone(), |_| true)
            .map_err(bind)?;
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            self.round
                .crafting_wares
                .pulses
                .push((handle, p.node(), pulse));
        }
        Ok(())
    }

    /// One of the three pulses by identity: `Some` with whether the runner
    /// drops it.
    pub fn crafting_ware_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.crafting_wares.pulses;
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
        let mut host = Crafter { live: self, p };
        let result = task.pulse(&mut host);
        // `delay = 3` and `delay = 2`: the pulse's own change of its delay.
        handle.write().delay = task.delay();
        match result {
            Ok(false) => {
                self.round.crafting_wares.pulses.push((handle, owner, task));
                Some(Ok(false))
            }
            finished => {
                pulses::stop_base(&handle);
                Some(finished.map(|_| true).map_err(bind))
            }
        }
    }

    /// One of the three pulses a pulse manager stopped: none overrides
    /// `stop()`.
    pub fn crafting_ware_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Bound<()>> {
        let pulses = &mut self.round.crafting_wares.pulses;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        pulses.remove(at);
        Some(Ok(()))
    }
}
