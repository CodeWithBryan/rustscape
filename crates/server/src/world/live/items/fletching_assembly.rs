//! Stringing bows and assembling arrows in the live world
//! (`world::fletching_assembly`, ported without a JVM corpus):
//! StringingListeners' and ArrowListeners' four `onUseWith` lambdas, the
//! SkillDialogueHandlers they open and their four WEAK scripts on the
//! player's ScriptProcessor (super::super::scripts).
//!
//! Joined: everything `super::fletching` joins for the logs, `hasSpaceFor`,
//! the removal of an amount, the magic shortbow attribute read back, the
//! Seers' Village `finishTask` and the RepositionChild the stringing
//! listener sends after its dialogue.
//!
//! Not ported: SlayerFlags. `isBroadsUnlocked()` answers false by name, so
//! broad arrows are always refused with the original's dialogue.
use super::{
    super::{
        host::Live,
        scripts::{Queued, ScriptCall},
    },
    Ctx, Node, P,
    everyday::Everyday,
    fletching::{Fletch, SEERS_VILLAGE, dialogue_error, fail},
    woodcutting::{Bound, bind},
};
use crate::{
    scripts::{self, Strength},
    world::{
        dialogue::{self as d, NodeHandle},
        dialogue_dsl::skill::{Handler, HandlerBody, SkillDialogue},
        fletching_assembly::{self as assembly, Host, Job},
        fletching_logs::{self as logs, Error, Quest, Result},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        player_managers::diary,
    },
};

/// The four `onUseWith(IntType.ITEM, ...)` lambdas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assembly {
    /// A string with an unstrung bow.
    Stringing,
    /// An arrow shaft with a feather.
    Headless,
    /// A headless arrow with arrow tips.
    Tipped,
    /// An ogre arrow shaft with a feather.
    Ogre,
}
impl crate::world::component_objects::NativeObject for Assembly {}

/// `fletching_assembly::Host` over the live world, for one player: the
/// logs' host with the calls only these scripts make.
struct Assemble<'a>(Fletch<'a>);
impl Host for Assemble<'_> {
    fn level(&mut self) -> Result<i32> {
        logs::Host::level(&mut self.0)
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        logs::Host::dialogue(&mut self.0, message)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        logs::Host::message(&mut self.0, message)
    }
    fn quest_stage(&mut self, quest: Quest) -> Result<i32> {
        logs::Host::quest_stage(&mut self.0, quest)
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
    fn queue(&mut self, delay: i32, job: Job, amount: i32) -> Result<()> {
        let function = Function::new(
            FunctionKind::Void,
            job.queued_lambda(),
            Queued::Assembly(job, amount),
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
    fn location(&mut self) -> Result<[i32; 3]> {
        logs::Host::location(&mut self.0)
    }
    fn attribute(&mut self, key: &str) -> Result<bool> {
        let attributes = self.0.live.inner.canonical(&self.0.p).player.attributes();
        // A value that is not a Boolean is the Kotlin cast's exception.
        attributes.boolean(key, false).or(Err(Error::Cast))
    }
    fn finish_seers_task(&mut self, level: i32, index: i32) -> Result<()> {
        let p = self.0.p.clone();
        let finished = super::super::subclasses::with_diaries(self.0.live, &p, |h, owner| {
            diary::finish_task(h, owner, owner, Some(SEERS_VILLAGE), level, index)
        });
        self.0.live.inner.drain(&p);
        finished.map_err(fail)
    }
    fn delay_clock_and_script(&mut self, ticks: i32) -> Result<bool> {
        logs::Host::delay_clock_and_script(&mut self.0, ticks)
    }
}

/// A listener's anonymous SkillDialogueHandler: its job and, for
/// stringing, the string used.
struct AssemblyHandler {
    job: Job,
    string: i32,
}
impl AssemblyHandler {
    fn host<'a>(live: &'a mut Live, player: &NodeHandle) -> d::Result<Assemble<'a>> {
        if !live.inner.players.contains_key(&player.node) {
            return Err(d::Error::Cast);
        }
        Ok(Assemble(Fletch {
            live,
            p: P(player.clone()),
        }))
    }
}
impl HandlerBody<Live> for AssemblyHandler {
    fn create(
        &self,
        host: &mut Live,
        handler: &Handler<Live>,
        amount: i32,
        _: i32,
    ) -> d::Result<()> {
        let mut assemble = Self::host(host, handler.player())?;
        assembly::create(&mut assemble, self.job, amount).map_err(dialogue_error)
    }
    fn get_all(&self, host: &mut Live, handler: &Handler<Live>, _: i32) -> d::Result<i32> {
        let mut assemble = Self::host(host, handler.player())?;
        assembly::get_all(&mut assemble, self.job, self.string).map_err(dialogue_error)
    }
}

impl Live {
    fn define_assembly(
        &mut self,
        class: &str,
        used: &[i32],
        with: &[i32],
        lambda: &'static str,
        which: Assembly,
    ) -> std::result::Result<(), String> {
        self.item_actions
            .listeners
            .clone()
            .add_use_arrays(
                listeners::Kind::Item.ordinal(),
                used,
                with,
                Function::new(FunctionKind::UseWith, lambda, which),
            )
            .map_err(|e| format!("{class}: {e:?}"))
    }

    /// `StringingListeners.defineListeners`.
    pub fn define_stringing_listener(&mut self) -> std::result::Result<(), String> {
        self.define_assembly(
            assembly::STRINGING_CLASS,
            &assembly::STRINGS,
            &assembly::unstrung_ids(),
            assembly::STRINGING_LAMBDA,
            Assembly::Stringing,
        )
    }

    /// `ArrowListeners.defineListeners`.
    pub fn define_arrow_listeners(&mut self) -> std::result::Result<(), String> {
        let class = assembly::ARROW_CLASS;
        self.define_assembly(
            class,
            &[assembly::ARROW_SHAFT],
            &assembly::FEATHERS,
            assembly::HEADLESS_LAMBDA,
            Assembly::Headless,
        )?;
        self.define_assembly(
            class,
            &[assembly::HEADLESS_ARROW],
            &assembly::tip_ids(),
            assembly::TIPPED_LAMBDA,
            Assembly::Tipped,
        )?;
        self.define_assembly(
            class,
            &[assembly::OGRE_ARROW_SHAFT],
            &assembly::FEATHERS,
            assembly::OGRE_LAMBDA,
            Assembly::Ogre,
        )
    }

    /// One of the four lambdas: its checks, then its SkillDialogueHandler,
    /// opened.
    pub fn fletching_assembly_use(
        &mut self,
        which: Assembly,
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
        let job = match which {
            Assembly::Stringing => {
                let mut host = Assemble(Fletch {
                    live: self,
                    p: p.clone(),
                });
                let checked = assembly::string_use(&mut host, used, with);
                self.inner.drain(&p);
                match checked.map_err(bind)? {
                    None => return Ok(false),
                    Some(None) => return Ok(true),
                    Some(Some(bow)) => Job::String(bow),
                }
            }
            Assembly::Headless => Job::Headless(with),
            Assembly::Tipped => match assembly::from_tip(with) {
                Some(arrow) => Job::Tipped(arrow),
                None => return Ok(false),
            },
            Assembly::Ogre => Job::Ogre(with),
        };
        let kind = match which {
            Assembly::Stringing => SkillDialogue::OneOption,
            _ => SkillDialogue::MakeSetOneOption,
        };
        let product = job.product().map_err(bind)?;
        let items = vec![Everyday(Ctx(self)).native(product, 1)];
        let body = AssemblyHandler { job, string: used };
        let handler = Handler::new(player.clone(), Some(kind), items, body);
        let mut opened = handler.open(self);
        if opened.is_ok() && which == Assembly::Stringing {
            // fixSpacingBetweenTextAndCraftedItemIcon(player).
            opened = d::Host::send_reposition_on_interface(self, p.clone(), 309, 2, 215, 10);
        }
        self.inner.drain(&p);
        opened.map_err(bind)?;
        Ok(true)
    }

    /// The script a job's `invoke()` queued.
    pub fn fletching_assembly_queued(
        &mut self,
        p: &P,
        job: Job,
        amount: i32,
        stage: i32,
    ) -> scripts::Result<bool> {
        let mut host = Assemble(Fletch {
            live: self,
            p: p.clone(),
        });
        let result = assembly::script(&mut host, job, amount, stage);
        self.inner.drain(p);
        result.map_err(|e| scripts::Error::Binding(format!("{}: {e:?}", job.queued_lambda())))
    }
}
