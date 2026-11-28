//! Fletching logs with a knife in the live world (`world::fletching_logs`,
//! ported without a JVM corpus): LogCraftableListeners' `onUseWith` of a
//! knife and the nine logs, the SkillDialogueHandler it opens
//! (`dialogue_dsl::skill`, with SkillDialoguePlugin registered in the
//! dialogue registry at boot) and CraftItemWithLogScript's WEAK script on
//! the player's ScriptProcessor (super::super::scripts).
//!
//! Joined: the dynamic level, item names, `sendDialogue`, the quest stages
//! the save holds (Big Chompy Bird Hunting and Zogre Flesh Eaters by their
//! repository index; nothing in this world advances them), the inventory,
//! the SKILLING clock, `keepRunning`, `stopExecuting`, `delayClock` with
//! `delayScript`, the animation, `RandomFunction.random`, `rewardXP`, the
//! Seers' Village diary task and the magic shortbow attribute.
use super::{
    super::{
        host::Live,
        scripts::{Queued, ScriptCall},
    },
    Ctx, Node, P,
    everyday::Everyday,
    woodcutting::{Bound, bind},
};
use crate::{
    scripts::{self, Strength},
    world::{
        animation::Animation,
        combat_pulse, combat_swing,
        dialogue::{self as d, NodeHandle},
        dialogue_dsl::skill::{Handler, HandlerBody, SkillDialogue, SkillDialoguePlugin},
        fletching_logs::{self as logs, Error, Host, Quest, Result},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        player_managers::diary,
    },
};

/// The plugin's class (`scan.initializable`).
pub const SKILL_DIALOGUE_PLUGIN: &str = "core.game.dialogue.SkillDialoguePlugin";
/// `DiaryType.SEERS_VILLAGE`'s ordinal.
pub(super) const SEERS_VILLAGE: usize = 5;

/// The knife's `onUseWith(IntType.ITEM, ...)` lambda.
pub struct KnifeOnLogs;
impl crate::world::component_objects::NativeObject for KnifeOnLogs {}

pub(super) fn fail(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// `fletching_logs::Host` over the live world, for one player.
pub(super) struct Fletch<'a> {
    pub(super) live: &'a mut Live,
    pub(super) p: P,
}
impl Fletch<'_> {
    pub(super) fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    fn script(&mut self, call: ScriptCall) -> Result<bool> {
        self.live.script_call(&self.p, call).map_err(fail)
    }
}
impl Host for Fletch<'_> {
    fn level(&mut self) -> Result<i32> {
        combat_swing::Host::level(self.live, &self.p, logs::FLETCHING).map_err(fail)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        // `Item.getName()` of an item without a definition name is null.
        let name = item.read().name.clone();
        name.ok_or(Error::Null)
    }
    fn dialogue(&mut self, message: &str) -> Result<()> {
        let node = self.node();
        Everyday(Ctx(self.live))
            .dialogue(&node, message)
            .map_err(fail)
    }
    fn message(&mut self, message: &str) -> Result<()> {
        self.live.inner.message(&self.p, message).map_err(fail)
    }
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
    fn amount_in_inventory(&mut self, id: i32) -> Result<i32> {
        let item = Everyday(Ctx(self.live)).native(id, 1);
        let player = &self.live.inner.canonical(&self.p).player;
        Ok(player.inventory().amount(Some(&item)))
    }
    fn queue(&mut self, delay: i32, craft: usize, amount: i32) -> Result<()> {
        let function = Function::new(
            FunctionKind::Void,
            logs::QUEUED_LAMBDA,
            Queued::LogCraft(craft, amount),
        );
        self.script(ScriptCall::Queue(function, delay, Strength::Weak, false))
            .map(|_| ())
    }
    fn clock_ready(&mut self) -> Result<bool> {
        let clock = self
            .live
            .script_clock(&self.p, logs::SKILLING as usize)
            .map_err(fail)?;
        Ok(clock <= self.live.inner.ticks)
    }
    fn keep_running(&mut self) -> Result<bool> {
        self.script(ScriptCall::KeepRunning)
    }
    fn stop_executing(&mut self) -> Result<bool> {
        self.script(ScriptCall::Stop)
    }
    fn remove_item(&mut self, id: i32) -> Result<bool> {
        let node = self.node();
        let mut everyday = Everyday(Ctx(self.live));
        let item = everyday.native(id, 1);
        let removed = everyday.remove_from_inventory(&node, &item);
        self.live.inner.drain(&self.p);
        removed.map_err(fail)
    }
    fn animate(&mut self, id: i32) -> Result<()> {
        self.live
            .entity_visualize(&self.p, Some(Animation::new(id)), None)
            .map_err(fail)
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        if max <= 0 {
            return Ok(0);
        }
        self.live
            .inner
            .generators
            .shared_next_int(max)
            .ok_or(Error::Null)
    }
    fn add_item(&mut self, id: i32, amount: i32) -> Result<bool> {
        let node = self.node();
        let added = Everyday(Ctx(self.live)).add_to_inventory(&node, id, amount);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn reward_xp(&mut self, xp: f64) -> Result<()> {
        let added =
            combat_pulse::Host::add_experience(self.live, &self.p, logs::FLETCHING, xp, false);
        self.live.inner.drain(&self.p);
        added.map_err(fail)
    }
    fn location(&mut self) -> Result<[i32; 3]> {
        let at = self
            .live
            .inner
            .state
            .node_location(self.p.node())
            .map_err(fail)?
            .ok_or(Error::Null)?;
        Ok(at.coordinates())
    }
    fn seers_task_complete(&mut self, level: i32, index: i32) -> Result<bool> {
        let p = self.p.clone();
        super::super::subclasses::with_diaries(self.live, &p, |h, owner| {
            diary::has_completed_task(h, owner, Some(SEERS_VILLAGE), level, index)
        })
        .map_err(fail)
    }
    fn set_attribute_true(&mut self, key: &str) -> Result<()> {
        let attributes = self.live.inner.canonical(&self.p).player.attributes();
        attributes.set_boolean(key, true);
        Ok(())
    }
    fn delay_clock_and_script(&mut self, ticks: i32) -> Result<bool> {
        let value = self.live.inner.ticks.wrapping_add(ticks);
        self.live
            .set_script_clock(&self.p, logs::SKILLING as usize, value)
            .map_err(fail)?;
        self.script(ScriptCall::Delay(ticks))
    }
}

/// The listener's anonymous SkillDialogueHandler: the log and its crafts.
struct LogHandler {
    log: i32,
    crafts: Vec<usize>,
}
pub(super) fn dialogue_error(e: Error) -> d::Error {
    match e {
        Error::Null => d::Error::Null,
        Error::Index => d::Error::Index,
        Error::Cast => d::Error::Cast,
        Error::Binding(text) => d::Error::Binding(text),
    }
}
impl LogHandler {
    fn host<'a>(live: &'a mut Live, player: &NodeHandle) -> d::Result<Fletch<'a>> {
        if !live.inner.players.contains_key(&player.node) {
            return Err(d::Error::Cast);
        }
        Ok(Fletch {
            live,
            p: P(player.clone()),
        })
    }
}
impl HandlerBody<Live> for LogHandler {
    fn create(
        &self,
        host: &mut Live,
        handler: &Handler<Live>,
        amount: i32,
        index: i32,
    ) -> d::Result<()> {
        let mut fletch = Self::host(host, handler.player())?;
        logs::create(&mut fletch, &self.crafts, amount, index).map_err(dialogue_error)
    }
    fn get_all(&self, host: &mut Live, handler: &Handler<Live>, _: i32) -> d::Result<i32> {
        // amountInInventory(player, log.id).
        let mut fletch = Self::host(host, handler.player())?;
        fletch.amount_in_inventory(self.log).map_err(dialogue_error)
    }
}

impl Live {
    /// `SkillDialoguePlugin.init()`: the plugin under `SKILL_DIALOGUE` in
    /// `DialogueInterpreter.PLUGINS`.
    pub fn define_skill_dialogue_plugin(&mut self) -> std::result::Result<(), String> {
        let registry = self.round.talk.registry.clone();
        SkillDialoguePlugin::<Live>::prototype()
            .init(self, &registry)
            .map_err(|e| format!("{SKILL_DIALOGUE_PLUGIN}: {e:?}"))
    }

    /// `LogCraftableListeners.defineListeners`.
    pub fn define_log_fletching_listener(&mut self) -> std::result::Result<(), String> {
        self.item_actions
            .listeners
            .clone()
            .add_use_arrays(
                listeners::Kind::Item.ordinal(),
                &[logs::KNIFE],
                &logs::log_ids(),
                Function::new(FunctionKind::UseWith, logs::LAMBDA, KnifeOnLogs),
            )
            .map_err(|e| format!("{}: {e:?}", logs::LISTENER_CLASS))
    }

    /// The lambda: the SkillDialogueHandler of the log's crafts, opened.
    pub fn log_fletching_use(&mut self, player: &Node, log: &Node) -> Bound<bool> {
        let (Node::Player(player), Node::Item(log)) = (player, log) else {
            return Err(bind("a knife used with a node that is not an item"));
        };
        let log = log.read().id();
        let Some(crafts) = logs::for_log_id(log) else {
            return Ok(false);
        };
        let items = {
            let everyday = Everyday(Ctx(self));
            crafts
                .iter()
                .map(|craft| everyday.native(logs::CRAFTS[*craft].finished, 1))
                .collect::<Vec<_>>()
        };
        let kind = SkillDialogue::for_length(crafts.len() as i32);
        let handler = Handler::new(player.clone(), kind, items, LogHandler { log, crafts });
        let opened = handler.open(self);
        self.inner.drain(&P(player.clone()));
        opened.map_err(bind)?;
        Ok(true)
    }

    /// The script `CraftItemWithLogScript.invoke()` queued.
    pub fn log_fletching_queued(
        &mut self,
        p: &P,
        craft: usize,
        amount: i32,
        stage: i32,
    ) -> scripts::Result<bool> {
        let mut host = Fletch {
            live: self,
            p: p.clone(),
        };
        let result = logs::script(&mut host, craft, amount, stage);
        self.inner.drain(p);
        result.map_err(|e| scripts::Error::Binding(format!("CraftItemWithLogScript: {e:?}")))
    }
}
