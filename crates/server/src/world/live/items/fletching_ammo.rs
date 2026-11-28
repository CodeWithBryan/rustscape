//! Bolts, darts, crossbow limbs, gem bolt tips and the mithril grapple in
//! the live world (`world::fletching_ammo`, whose library has the JVM
//! corpus `fixtures/fletching`; this binding has none): the five listener
//! classes' seven `onUseWith` lambdas, the SkillDialogueHandlers they open
//! and their WEAK scripts on the player's ScriptProcessor
//! (super::super::scripts).
//!
//! Joined: everything `super::fletching` joins for the logs, `hasSpaceFor`,
//! the removal of an amount, `delayScript`, `delayClock` without the script
//! delay, the Tourist Trap stage the save holds (index 123; nothing in this
//! world advances it, so darts are refused unless a save holds 100) and the
//! RepositionChild the limbing listener sends after its dialogue.
//!
//! Not ported: SlayerFlags. `isBroadsUnlocked()` answers false by name, so
//! broad bolts are always refused with the original's dialogue.
use super::{
    super::{
        host::Live,
        scripts::{Queued, ScriptCall},
    },
    Ctx, Node, P,
    everyday::Everyday,
    fletching::{Fletch, dialogue_error, fail},
    woodcutting::{Bound, bind},
};
use crate::{
    scripts::{self, Strength},
    world::{
        dialogue::{self as d, NodeHandle},
        dialogue_dsl::skill::{Handler, HandlerBody},
        fletching_ammo::{self as ammo, Craft, Host, Script},
        fletching_logs::{self as logs, Result},
        interaction_listeners::{self as listeners, Function, FunctionKind},
    },
};

/// The seven lambdas' marker.
pub use ammo::Listener as Ammo;
impl crate::world::component_objects::NativeObject for Ammo {}

/// TouristTrap.java's index in the quest repository.
const TOURIST_TRAP: i32 = 123;

/// `fletching_ammo::Host` over the live world, for one player: the logs'
/// host with the calls only these scripts make.
struct Maker<'a>(Fletch<'a>);
impl Host for Maker<'_> {
    fn level(&mut self) -> Result<i32> {
        logs::Host::level(&mut self.0)
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        logs::Host::dialogue(&mut self.0, message)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        logs::Host::message(&mut self.0, message)
    }
    fn tourist_trap_stage(&mut self) -> Result<i32> {
        // QuestRepository.getStage: the stage the save holds, 0 without one.
        let stage = self
            .0
            .live
            .inner
            .with_profile(&self.0.p, |profile| {
                profile.persisted.quests.stages.get(TOURIST_TRAP).copied()
            })
            .map_err(fail)?;
        Ok(stage.unwrap_or(0))
    }
    fn broads_unlocked(&mut self) -> Result<bool> {
        // SlayerFlags is not ported: `rewardFlags` of a fresh manager is 0.
        self.0
            .live
            .inner
            .not_ported
            .default_answer("SlayerFlags.isBroadsUnlocked");
        Ok(false)
    }
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        logs::Host::amount_in_inventory(&mut self.0, id)
    }
    fn has_space_for(&mut self, id: i32) -> Result<bool> {
        let item = Everyday(Ctx(self.0.live)).native(id, 1);
        let player = &self.0.live.inner.canonical(&self.0.p).player;
        player.inventory().has_space_for(Some(&item)).map_err(fail)
    }
    fn queue(&mut self, delay: i32, script: Script, amount: i32) -> Result<()> {
        let function = Function::new(
            FunctionKind::Void,
            script.queued_lambda(),
            Queued::Ammo(script, amount),
        );
        self.0
            .live
            .script_call(
                &self.0.p,
                ScriptCall::Queue(function, delay, Strength::Weak, false),
            )
            .map(|_| ())
            .map_err(fail)
    }
    fn clock_ready(&mut self) -> Result<bool> {
        logs::Host::clock_ready(&mut self.0)
    }
    fn keep_running(&mut self) -> Result<bool> {
        logs::Host::keep_running(&mut self.0)
    }
    fn stop_executing(&mut self) -> Result<bool> {
        logs::Host::stop_executing(&mut self.0)
    }
    fn remove_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        let node = self.0.node();
        let mut everyday = Everyday(Ctx(self.0.live));
        let item = everyday.native(id, amount);
        let removed = everyday.remove_from_inventory(&node, &item);
        self.0.live.inner.drain(&self.0.p);
        removed.map_err(fail)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        logs::Host::animate(&mut self.0, id)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        logs::Host::add_item(&mut self.0, id, amount)
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        logs::Host::reward_xp(&mut self.0, xp)
    }
    fn delay_clock(&mut self, ticks: i32, delay_script: bool) -> Result<bool> {
        if delay_script {
            return logs::Host::delay_clock_and_script(&mut self.0, ticks);
        }
        let value = self.0.live.inner.ticks.wrapping_add(ticks);
        self.0
            .live
            .set_script_clock(&self.0.p, logs::SKILLING as usize, value)
            .map_err(fail)?;
        Ok(false)
    }
    fn delay_script(&mut self, ticks: i32) -> Result<bool> {
        self.0
            .live
            .script_call(&self.0.p, ScriptCall::Delay(ticks))
            .map_err(fail)
    }
}

fn maker<'a>(live: &'a mut Live, p: &P) -> Maker<'a> {
    Maker(Fletch { live, p: p.clone() })
}

/// A listener's anonymous SkillDialogueHandler: its craft and the two items
/// its lambda was given.
struct AmmoHandler {
    craft: Craft,
    used: i32,
    with: i32,
}
impl AmmoHandler {
    fn host<'a>(live: &'a mut Live, player: &NodeHandle) -> d::Result<Maker<'a>> {
        if !live.inner.players.contains_key(&player.node) {
            return Err(d::Error::Cast);
        }
        Ok(maker(live, &P(player.clone())))
    }
}
impl HandlerBody<Live> for AmmoHandler {
    fn create(
        &self,
        host: &mut Live,
        handler: &Handler<Live>,
        amount: i32,
        _: i32,
    ) -> d::Result<()> {
        let mut maker = Self::host(host, handler.player())?;
        ammo::create(&mut maker, self.craft, amount).map_err(dialogue_error)
    }
    fn get_all(&self, host: &mut Live, handler: &Handler<Live>, _: i32) -> d::Result<i32> {
        let mut maker = Self::host(host, handler.player())?;
        ammo::get_all(&mut maker, self.craft, self.used, self.with).map_err(dialogue_error)
    }
}

impl Live {
    /// `defineListeners` of the listener class `class`: each of its lambdas.
    pub fn define_fletching_ammo_listeners(
        &mut self,
        class: &str,
    ) -> std::result::Result<(), String> {
        for which in Ammo::ALL {
            if which.class() != class {
                continue;
            }
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
                .map_err(|e| format!("{class}: {e:?}"))?;
        }
        Ok(())
    }

    /// One of the seven lambdas: its checks, then its SkillDialogueHandler,
    /// opened (the grapple lambdas queue their script instead).
    pub fn fletching_ammo_use(
        &mut self,
        which: Ammo,
        player: &Node,
        used: &Node,
        with: &Node,
    ) -> Bound<bool> {
        let (Node::Player(player), Node::Item(used), Node::Item(with)) = (player, used, with)
        else {
            return Err(bind(
                "a fletching item used with a node that is not an item",
            ));
        };
        let p = P(player.clone());
        let (used, with) = (used.read().id(), with.read().id());
        let checked = ammo::use_with(&mut maker(self, &p), which, used, with);
        self.inner.drain(&p);
        let craft = match checked.map_err(bind)? {
            None => return Ok(false),
            Some(None) => return Ok(true),
            Some(Some(craft)) => craft,
        };
        let product = craft.product().map_err(bind)?;
        let items = vec![Everyday(Ctx(self)).native(product, 1)];
        let body = AmmoHandler { craft, used, with };
        let handler = Handler::new(player.clone(), Some(craft.kind()), items, body);
        let mut opened = handler.open(self);
        if let (Ok(()), Some([interface, child, x, y])) = (&opened, craft.reposition()) {
            // fixTextOverlappingTheCrossbowIcon(player).
            opened = d::Host::send_reposition_on_interface(self, p.clone(), interface, child, x, y);
        }
        self.inner.drain(&p);
        opened.map_err(bind)?;
        Ok(true)
    }

    /// A script one of these classes queued.
    pub fn fletching_ammo_queued(
        &mut self,
        p: &P,
        script: &Script,
        amount: i32,
        stage: i32,
    ) -> scripts::Result<bool> {
        let result = ammo::script(&mut maker(self, p), script, amount, stage);
        self.inner.drain(p);
        result.map_err(|e| scripts::Error::Binding(format!("{}: {e:?}", script.queued_lambda())))
    }
}
