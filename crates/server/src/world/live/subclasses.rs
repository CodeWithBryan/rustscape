//! The NPC class overrides in the live world: `world::npc_classes`
//! (`classes::init`, `tick`, `finalize_death`, `on_attack`, `on_impact`,
//! `swing_handler` with MonkNPC's `CombatAction`) with the live world as its
//! `Host`, reached where the original's virtual calls are:
//!
//! - `NPC.tick()` from the driver's NPC tick ([`Live::npc_class_tick`]);
//! - `NPC.finalizeDeath(killer)` from the death task's stop
//!   (`npc_death::task_stop_with` with [`npc_finalize_death`]);
//! - `Entity.onAttack(target)` from `CombatPulse.attack`, `NPC.onImpact` from
//!   the impact handler, `NPC.getSwingHandler(swing)` from the combat pulse
//!   (combat/behaviour.rs and combat/host.rs call [`on_attack`],
//!   [`on_impact`] and [`swing_handler`]);
//! - `init()` of an NPC made at runtime (super::runtime_npcs).
//!
//! The dragon classes (`dragonfire::dragons`) answer their swing handler
//! through [`super::dragons`].
//!
//! The quest and diary calls are answered from the persisted state: a quest
//! the original registers (Witch's Potion, the only one these classes name)
//! exists, and its stage is the player's persisted quest stage (0 when the
//! save holds none, as the original's repository puts 0 for every registered
//! quest); the Falador diary is the persisted diary through
//! `world::player_managers::diary` (`isComplete`, `updateTask` with its
//! messages and the diary tab's status lines). Everything that would run
//! other content (banker and talk listeners, random event and holiday NPCs,
//! barcrawl, gnome cooking, dark wizard spells, idle NPCs) refuses by name.
use super::{P, combat::Attr, host::Live};
use crate::{
    inventory::item::Item,
    world::{
        combat_swing::BattleState,
        doors::Location,
        ground_manager as gm, npc_behaviour as nb,
        npc_classes::{
            self as nc, Arg, Class, CombatStyle, Diary, Direction, Kind, Spell, Value,
            classes::{self, Swing},
            idle::Idle,
        },
        npc_death,
        player_managers::{self as pm, diary},
    },
};

/// The quests `npc_classes` names, as the original registers them:
/// `Quests` constant name and `Quest` index.
const QUESTS: [(&str, i32); 1] = [(classes::WITCHS_POTION, 31)];
/// `DiaryType.FALADOR`'s ordinal.
const FALADOR: usize = 3;

fn refuse<T>(what: &str) -> nc::Result<T> {
    Err(nc::Error::Binding(Some(format!(
        "not ported in the live world: {what}"
    ))))
}
fn bind(e: impl std::fmt::Debug) -> nc::Error {
    nc::Error::Binding(Some(format!("{e:?}")))
}

/// `npc_classes::Host` over the live world, with the battle state of an
/// `onImpact` call when there is one.
pub struct Classes<'a, 'b> {
    pub live: &'a mut Live,
    pub state: Option<&'b mut BattleState<Live>>,
    /// The diary `getDiary` last answered for (`diary.isComplete` reads it).
    diary_owner: Option<P>,
}

impl<'a> Classes<'a, '_> {
    pub fn new(live: &'a mut Live) -> Self {
        Self {
            live,
            state: None,
            diary_owner: None,
        }
    }
    fn row(&self, npc: &P) -> nc::Result<&crate::startup::npcs::Npc> {
        self.live
            .inner
            .npcs
            .rows
            .get(&npc.node())
            .ok_or(nc::Error::Null)
    }
    fn player(&self, p: &P) -> nc::Result<P> {
        if self.live.inner.is_player_node(p.node()) {
            Ok(p.clone())
        } else {
            Err(nc::Error::Cast)
        }
    }
}

/// The NPC's class among `npc_classes::Class` (a dragon's is not one).
fn npc_class(live: &Live, node: usize) -> Option<Class> {
    live.inner
        .npcs
        .rows
        .get(&node)
        .and_then(|row| row.class)
        .and_then(|c| c.npc())
}

impl Live {
    /// `NPC.tick()` with a subclass's override (CowNPC, DuckNPC).
    pub fn npc_class_tick(&mut self, npc: &P) -> crate::world::npc::Result<()> {
        if npc_class(self, npc.node()).is_none() {
            return self.npc_tick_of(npc);
        }
        classes::tick(&mut Classes::new(self), npc)
            .map_err(|e| crate::world::npc::Error::Binding(format!("{e:?}")))
    }
}

/// `NPC.finalizeDeath(killer)` with a subclass's override (RatNPC,
/// HighwayManNPC, IceGiantNPC), for `npc_death::task_stop_with`.
pub fn npc_finalize_death(
    live: &mut Live,
    npc: &crate::world::player_walk::NodeHandle,
    killer: &crate::world::player_walk::NodeHandle,
) -> npc_death::Result<()> {
    if npc_class(live, npc.node).is_none() {
        return npc_death::npc_finalize_death(live, npc, killer);
    }
    let (npc, killer) = (P(npc.clone()), P(killer.clone()));
    classes::finalize_death(&mut Classes::new(live), &npc, Some(&killer))
        .map_err(|e| npc_death::Error::Binding(format!("{e:?}")))
}

/// `entity.onAttack(target)`: HighwayManNPC's chat; `Entity`'s empty body
/// otherwise.
pub fn on_attack(
    live: &mut Live,
    entity: &P,
    target: &P,
) -> crate::world::combat_swing::Result<()> {
    if npc_class(live, entity.node()).is_none() {
        return Ok(());
    }
    classes::on_attack(&mut Classes::new(live), entity, Some(target))
        .map_err(|e| crate::world::combat_swing::Error::Binding(format!("{e:?}")))
}

/// `NPC.onImpact(entity, state)` with DarkWizardNPC's override.
pub fn on_impact(
    live: &mut Live,
    victim: &P,
    attacker: Option<&P>,
    state: &mut BattleState<Live>,
) -> crate::world::combat_swing::Result<()> {
    if npc_class(live, victim.node()).is_none() {
        return nb::combat::on_impact(live, victim, attacker, state);
    }
    let mut h = Classes {
        live,
        state: Some(state),
        diary_owner: None,
    };
    classes::on_impact(&mut h, victim, attacker)
        .map_err(|e| crate::world::combat_swing::Error::Binding(format!("{e:?}")))
}

/// `NPC.getSwingHandler(swing)` with MonkNPC's and the dragons' overrides.
pub fn swing_handler(
    live: &mut Live,
    npc: &P,
    swing: bool,
) -> crate::world::combat_swing::Result<usize> {
    let class = live
        .inner
        .npcs
        .rows
        .get(&npc.node())
        .and_then(|row| row.class);
    match class {
        None => nb::combat::npc_swing_handler(live, npc),
        Some(crate::world::creatures::classes::WorldClass::Dragon(dragon)) => {
            super::dragons::combat_action(live, npc, dragon)
        }
        Some(_) => {
            let chosen = classes::swing_handler(&mut Classes::new(live), npc, swing)
                .map_err(|e| crate::world::combat_swing::Error::Binding(format!("{e:?}")))?;
            match chosen {
                Swing::Other(handler) => Ok(handler),
                Swing::Monk => Ok(super::dragons::monk_handler(live)),
            }
        }
    }
}

/// MonkNPC's `CombatAction.swing`, as the override of its handler instance.
pub fn monk_swing(
    live: &mut Live,
    _: &usize,
    entity: Option<&P>,
    victim: Option<&P>,
    state: &mut BattleState<Live>,
) -> crate::world::combat_swing::Result<i32> {
    let entity = entity.ok_or(crate::world::combat_swing::Error::Null)?;
    let mut h = Classes {
        live,
        state: Some(state),
        diary_owner: None,
    };
    classes::monk_swing(&mut h, entity, victim)
        .map_err(|e| crate::world::combat_swing::Error::Binding(format!("{e:?}")))
}

impl nc::Factory for Classes<'_, '_> {
    type Npc = P;
    type Class = Class;
    fn new_npc(
        &mut self,
        _: Option<Class>,
        _: i32,
        _: Option<[i32; 3]>,
        _: Option<Direction>,
    ) -> nc::Result<P> {
        refuse("constructing an AbstractNPC from a class body")
    }
    fn set_walks(&mut self, npc: &P, walks: bool) -> nc::Result<()> {
        self.row(npc)?.brain.fields.write().walks = walks;
        Ok(())
    }
    fn set_aggressive(&mut self, npc: &P, aggressive: bool) -> nc::Result<()> {
        self.row(npc)?.brain.fields.write().aggressive = aggressive;
        Ok(())
    }
    fn log(&mut self, origin: &str, _: nc::LogLevel, message: &str) -> nc::Result<()> {
        self.live.inner.log_line(format!("[{origin}] {message}"));
        Ok(())
    }
}

impl nc::Host for Classes<'_, '_> {
    type Handler = usize;
    type File = ();

    fn kind(&self, node: &P) -> Kind {
        if self.live.inner.is_player_node(node.node()) {
            Kind::Player
        } else if self.live.inner.is_npc_node(node.node()) {
            Kind::Npc
        } else if self.live.inner.state.scenery(node.node()).is_ok() {
            Kind::Scenery
        } else {
            Kind::Item
        }
    }
    fn class(&self, npc: &P) -> Option<Class> {
        npc_class(self.live, npc.node())
    }
    fn same(&self, a: &P, b: &P) -> bool {
        a.node() == b.node()
    }
    fn id(&mut self, node: &P) -> nc::Result<i32> {
        if self.live.inner.is_player_node(node.node()) {
            return Ok(-1);
        }
        if let Ok(object) = self.live.inner.state.scenery(node.node()) {
            return Ok(object.read().id);
        }
        Ok(self.row(node)?.id)
    }
    fn location(&mut self, node: &P) -> nc::Result<Option<[i32; 3]>> {
        Ok(self.live.cloc(node).map(|l| l.coordinates()))
    }

    fn base_init(&mut self, npc: &P) -> nc::Result<()> {
        self.live.npc_init_base(npc.node()).map_err(bind)
    }
    fn base_tick(&mut self, npc: &P) -> nc::Result<()> {
        self.live.npc_tick_of(npc).map_err(bind)
    }
    fn base_tick_actions(&mut self, _: &P) -> nc::Result<()> {
        refuse("NPC.handleTickActions outside the NPC tick kernel")
    }
    fn base_finalize_death(&mut self, npc: &P, killer: Option<&P>) -> nc::Result<()> {
        let killer = killer.ok_or(nc::Error::Null)?;
        npc_death::npc_finalize_death(self.live, &npc.0, &killer.0).map_err(bind)
    }
    fn base_on_attack(&mut self, _: &P, _: Option<&P>) -> nc::Result<()> {
        // Entity.onAttack: an empty body.
        Ok(())
    }
    fn base_on_impact(&mut self, npc: &P, entity: Option<&P>) -> nc::Result<()> {
        let state = self.state.as_deref_mut().ok_or(nc::Error::Null)?;
        nb::combat::on_impact(self.live, npc, entity, state).map_err(bind)
    }
    fn base_walk_radius(&mut self, npc: &P) -> nc::Result<i32> {
        Ok(self.row(npc)?.brain.fields.read().radius)
    }
    fn idle(&mut self, _: &P) -> nc::Result<Option<Idle>> {
        // No IdleAbstractNPC class is registered in this world.
        Ok(None)
    }
    fn set_idle(&mut self, _: &P, _: Idle) -> nc::Result<()> {
        refuse("IdleAbstractNPC (no idle class is registered)")
    }
    fn handler_swing(&mut self, _: &usize, _: &P, _: Option<&P>) -> nc::Result<i32> {
        refuse("a swing handler's swing outside the combat pulse")
    }
    fn base_swing_handler(&mut self, npc: &P, _: bool) -> nc::Result<usize> {
        nb::combat::npc_swing_handler(self.live, npc).map_err(bind)
    }
    fn swing_handler(&mut self, entity: &P, swing: bool) -> nc::Result<usize> {
        crate::world::combat_pulse::Host::swing_handler(self.live, entity, swing).map_err(bind)
    }
    fn can_swing(&mut self, _: &usize, _: &P, _: &P) -> nc::Result<Option<nc::InteractionType>> {
        refuse("canSwing from an NPC class body (BankerNPC/idle content)")
    }
    fn melee_swing(&mut self, entity: &P, victim: Option<&P>) -> nc::Result<i32> {
        // MeleeSwingHandler.swing(entity, victim, state): the melee
        // singleton's body over this call's battle state.
        let state = self.state.as_deref_mut().ok_or(nc::Error::Null)?;
        let mut instance =
            crate::world::combat_pulse::Host::handler(self.live, &super::combat::MELEE).clone();
        let delay = instance.swing(self.live, Some(entity), victim, state);
        *crate::world::combat_pulse::Host::handler(self.live, &super::combat::MELEE) = instance;
        delay.map_err(bind)
    }
    fn transform(&mut self, npc: &P, id: i32) -> nc::Result<()> {
        self.live.npc_transform(npc.node(), id).map_err(bind)
    }
    fn re_transform(&mut self, _: &P) -> nc::Result<()> {
        refuse("NPC.reTransform from an NPC class body")
    }
    fn is_invisible(&mut self, _: &P) -> nc::Result<bool> {
        refuse("NPC.isInvisible from an NPC class body (idle content)")
    }
    fn task_level(&mut self, _: &P) -> nc::Result<Option<i32>> {
        // NPC.getTask(): Tasks.forId is not ported (no task).
        self.live
            .inner
            .not_ported
            .default_answer("NPC.getTask (null: Tasks.forId is not ported)");
        Ok(None)
    }
    fn send_chat(&mut self, entity: &P, text: &str) -> nc::Result<bool> {
        if self.live.inner.is_npc_node(entity.node()) {
            return self
                .live
                .npc_chat(entity.node(), text.to_owned())
                .map_err(bind);
        }
        refuse("Player.sendChat from an NPC class body")
    }
    fn direction(&mut self, entity: &P) -> nc::Result<Option<Direction>> {
        Ok(self
            .live
            .inner
            .state
            .actor(entity.node())
            .map_err(bind)?
            .queue
            .entity()
            .direction)
    }
    fn set_direction(&mut self, entity: &P, direction: Option<Direction>) -> nc::Result<()> {
        // Entity.setDirection: the walking entity's direction field.
        self.live
            .inner
            .state
            .actor(entity.node())
            .map_err(bind)?
            .queue
            .state
            .write()
            .entity
            .direction = direction;
        Ok(())
    }
    fn face_location(&mut self, entity: &P, location: Option<[i32; 3]>) -> nc::Result<()> {
        if self.live.inner.is_npc_node(entity.node()) {
            let at = location.map(|[x, y, z]| Location::new(x, y, z));
            return self
                .live
                .npc_face_location(entity.node(), at)
                .map(|_| ())
                .map_err(bind);
        }
        refuse("a player's faceLocation from an NPC class body")
    }
    fn in_combat(&mut self, entity: &P) -> nc::Result<bool> {
        crate::world::npc_behaviour::Host::is_attacking(self.live, entity).map_err(bind)
    }
    fn animate(&mut self, entity: &P, animation: i32) -> nc::Result<()> {
        self.live
            .entity_visualize(
                entity,
                Some(crate::world::animation::Animation::new(animation)),
                None,
            )
            .map_err(bind)
    }
    fn get_attribute(&mut self, entity: &P, key: &str) -> nc::Result<Option<Value>> {
        Ok(match self.live.inner.attribute(entity.node(), key) {
            None => None,
            Some(Attr::Bool(v)) => Some(Value::Bool(v)),
            Some(Attr::Int(v)) => Some(Value::Int(v)),
            Some(Attr::Long(v)) => Some(Value::Long(v)),
            Some(Attr::Entity(_)) => return Err(nc::Error::Cast),
        })
    }
    fn set_attribute(&mut self, entity: &P, key: &str, value: Value) -> nc::Result<()> {
        let value = match value {
            Value::Bool(v) => Attr::Bool(v),
            Value::Int(v) => Attr::Int(v),
            Value::Long(v) => Attr::Long(v),
            other => return refuse(&format!("an attribute value {other:?}")),
        };
        self.live.inner.set_attribute(entity.node(), key, value);
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &P, key: &str) -> nc::Result<()> {
        self.live.inner.remove_attribute(entity.node(), key);
        Ok(())
    }
    fn api_get_attribute(&mut self, entity: &P, key: &str) -> nc::Result<Option<Value>> {
        nc::Host::get_attribute(self, entity, key)
    }

    fn combat_victim(&mut self, entity: &P) -> nc::Result<Option<P>> {
        crate::world::npc_behaviour::Host::combat_victim(self.live, entity).map_err(bind)
    }
    fn is_attacking(&mut self, entity: &P) -> nc::Result<bool> {
        crate::world::npc_behaviour::Host::is_attacking(self.live, entity).map_err(bind)
    }
    fn set_next_attack(&mut self, entity: &P, ticks: i32) -> nc::Result<()> {
        // combatPulse.setNextAttack(ticks): GameWorld.getTicks() + ticks.
        let now = self.live.inner.ticks;
        crate::world::combat_pulse::Host::combat_pulse(self.live, entity).next_attack =
            now.wrapping_add(ticks);
        Ok(())
    }
    fn set_combat_style(&mut self, _: &P, _: CombatStyle) -> nc::Result<()> {
        refuse("combatPulse.setStyle from DarkWizardNPC (not registered)")
    }
    fn set_combat_flags(&mut self, _: &P, _: &P) -> nc::Result<()> {
        refuse("combatPulse.setCombatFlags from an NPC class body (idle content)")
    }
    fn combat_attack(&mut self, _: &P, _: Option<&P>) -> nc::Result<()> {
        refuse("combatPulse.attack from an NPC class body (idle content)")
    }
    fn modern_spell(&mut self, _: i32) -> nc::Result<Option<Spell>> {
        refuse("SpellBook.MODERN (spells are DarkWizardNPC's, not registered)")
    }
    fn set_autocast_spell(&mut self, _: &P, _: Option<i32>) -> nc::Result<()> {
        refuse("properties.setAutocastSpell (DarkWizardNPC, not registered)")
    }
    fn lifepoints(&mut self, entity: &P) -> nc::Result<i32> {
        crate::world::combat_swing::Host::lifepoints(self.live, entity).map_err(bind)
    }
    fn maximum_lifepoints(&mut self, entity: &P) -> nc::Result<i32> {
        crate::world::combat_swing::Host::maximum_lifepoints(self.live, entity).map_err(bind)
    }
    fn heal(&mut self, entity: &P, amount: i32) -> nc::Result<()> {
        crate::world::combat_pulse::Host::heal(self.live, entity, amount)
            .map(|_| ())
            .map_err(bind)
    }
    fn level(&mut self, entity: &P, skill: i32) -> nc::Result<i32> {
        crate::world::combat_swing::Host::level(self.live, entity, skill).map_err(bind)
    }
    fn is_dead(&mut self, entity: &P) -> nc::Result<bool> {
        crate::world::npc_behaviour::Host::is_dead(self.live, entity).map_err(bind)
    }
    fn continue_attack(
        &mut self,
        _: &P,
        _: &P,
        _: Option<CombatStyle>,
        _: bool,
    ) -> nc::Result<bool> {
        refuse("ZoneMonitor.continueAttack from an NPC class body (idle content)")
    }
    fn random(&mut self, bound: i32) -> nc::Result<i32> {
        // RandomFunction.random(bound): RANDOM.nextInt, 0 for no bound.
        if bound <= 0 {
            return Ok(0);
        }
        self.live
            .inner
            .generators
            .shared_next_int(bound)
            .ok_or(nc::Error::Binding(Some("nextInt bound".into())))
    }
    fn randomize(&mut self, bound: i32) -> nc::Result<i32> {
        nc::Host::random(self, bound)
    }
    fn current_time_millis(&mut self) -> nc::Result<i64> {
        Ok(super::now_millis())
    }

    fn get_quest(&mut self, player: &P, quest: &str) -> nc::Result<bool> {
        self.player(player)?;
        // getQuestRepository().getQuest(quest): the quests the original
        // registers exist.
        Ok(QUESTS.iter().any(|(name, _)| *name == quest))
    }
    fn quest_started(&mut self, player: &P, quest: &str) -> nc::Result<bool> {
        let p = self.player(player)?;
        let index = QUESTS
            .iter()
            .find(|(name, _)| *name == quest)
            .map(|(_, index)| *index)
            .ok_or(nc::Error::Null)?;
        // Quest.isStarted: 0 < getStage(player) < 100, the stage the
        // player's QuestRepository holds (0 for a quest its save lacks).
        let stage = self
            .live
            .inner
            .with_profile(&p, |profile| {
                profile.persisted.quests.stages.get(index).copied()
            })
            .map_err(bind)?
            .unwrap_or(0);
        Ok(stage > 0 && stage < 100)
    }
    fn get_diary(&mut self, player: &P, diary: Diary) -> nc::Result<bool> {
        let p = self.player(player)?;
        match diary {
            Diary::Falador => {
                self.diary_owner = Some(p);
                Ok(true)
            }
        }
    }
    fn diary_complete(&mut self, diary: Diary, level: i32, task: i32) -> nc::Result<bool> {
        let Diary::Falador = diary;
        let owner = self.diary_owner.clone().ok_or(nc::Error::Null)?;
        with_diaries(self.live, &owner, |h, owner| {
            diary::has_completed_task(h, owner, Some(FALADOR), level, task)
        })
        .map_err(bind)
    }
    fn diary_update(
        &mut self,
        diary: Diary,
        player: &P,
        level: i32,
        index: i32,
        complete: bool,
    ) -> nc::Result<()> {
        let Diary::Falador = diary;
        let owner = self.diary_owner.clone().ok_or(nc::Error::Null)?;
        let player = self.player(player)?;
        with_diaries(self.live, &owner, |h, owner| {
            diary::update_task(h, owner, &player, FALADOR, level, index, complete)
        })
        .map_err(bind)
    }
    fn ground_item(
        &mut self,
        id: i32,
        amount: i32,
        location: Option<[i32; 3]>,
        player: &P,
    ) -> nc::Result<()> {
        let item = crate::definitions::item::Shared::new(Item::new(
            id,
            amount,
            1000,
            &self.live.inner.items,
        ));
        let manager = self.live.walk.ground.manager.clone();
        let at = location.map(|[x, y, z]| Location::new(x, y, z));
        gm::create_at(self.live, &manager, Some(item), at, Some(player.0.clone()))
            .map(|_| ())
            .map_err(bind)
    }
    fn get_scenery(&mut self, [x, y, z]: [i32; 3]) -> nc::Result<Option<P>> {
        // ContentAPI.getScenery(location): RegionManager.getObject over the
        // scene, as a scenery node of the canonical state.
        let walk = &mut self.live.walk;
        let object = walk
            .scene
            .context(&mut walk.maps)
            .object(Location::new(x, y, z), -1, -1)
            .map_err(|e| nc::Error::Binding(Some(e.0)))?;
        match object {
            None => Ok(None),
            Some(object) => Ok(Some(P(self
                .live
                .inner
                .state
                .scenery_target(object)
                .map_err(bind)?))),
        }
    }

    fn api_message(&mut self, player: &P, text: &str) -> nc::Result<()> {
        self.live.inner.message(player, text).map_err(bind)
    }
    fn player_message(&mut self, player: &P, text: &str) -> nc::Result<()> {
        self.live.inner.message(player, text).map_err(bind)
    }
    fn packet_message(&mut self, player: &P, text: &str) -> nc::Result<()> {
        self.live.inner.message(player, text).map_err(bind)
    }
    fn open_dialogue_id(&mut self, player: &P, id: i32, args: &[Arg<P>]) -> nc::Result<bool> {
        // player.getDialogueInterpreter().open(id, args...): no content
        // dialogue plugin is registered, so an NPC's key answers false after
        // the NPC is stopped (super::talk).
        let args = dialogue_args(args)?;
        let interpreter = self.live.interpreter_of(player);
        let opened = interpreter.open(self.live, id, &args).map_err(bind)?;
        if !opened {
            self.live.inner.not_ported.default_answer(
                "DialogueInterpreter.open of an NPC key (no content dialogue plugin registered: false)",
            );
        }
        Ok(opened)
    }
    fn open_dialogue_name(&mut self, player: &P, name: &str, args: &[Arg<P>]) -> nc::Result<bool> {
        let args = dialogue_args(args)?;
        let interpreter = self.live.interpreter_of(player);
        let key: Vec<u16> = name.encode_utf16().collect();
        interpreter
            .open_named(self.live, Some(&key), &args)
            .map_err(bind)
    }
    fn send_dialogue(&mut self, player: &P, lines: &[&str]) -> nc::Result<()> {
        let interpreter = self.live.interpreter_of(player);
        interpreter
            .send_dialogue(self.live, Some(&super::talk::lines(lines)))
            .map(|_| ())
            .map_err(bind)
    }
    fn gnome_completion_dialogue(&mut self, _: &'static str) -> nc::Result<()> {
        refuse("GCCompletionDialogue (gnome cooking content)")
    }
    fn open_file(&mut self, _: &P, _: ()) -> nc::Result<()> {
        refuse("dialogueInterpreter.open(file) (gnome cooking content)")
    }
    fn banker_conversation(&mut self, player: &P, npc: &P) -> nc::Result<()> {
        // DialogueLabeller.open(player, BankerDialogueLabellerFile(), npc).
        crate::world::dialogue_dsl::banker::talk_to(self.live, &player.0, npc)
            .map(|_| ())
            .map_err(bind)
    }
    fn api_open_dialogue(&mut self, player: &P, id: i32, npc: &P) -> nc::Result<()> {
        crate::world::dialogue_dsl::api::open_dialogue(
            self.live,
            &player.0,
            &crate::world::dialogue_dsl::Dialogue::Key(id),
            &[crate::world::dialogue::Arg::Npc(npc.clone())],
        )
        .map_err(bind)
    }
    fn remove_item(&mut self, player: &P, id: i32, amount: i32) -> nc::Result<bool> {
        // removeItem(player, Item(id, amount)) from the inventory.
        let item = crate::definitions::item::Shared::new(Item::new(
            id,
            amount,
            1000,
            &self.live.inner.items,
        ));
        self.live.remove_from_inventory(player, &item).map_err(bind)
    }
    fn open_bank_account(&mut self, player: &P) -> nc::Result<()> {
        self.live.open_bank_account(player).map_err(bind)
    }
    fn open_collection_box(&mut self, player: &P) -> nc::Result<()> {
        self.live.ge_open_collection_box(player).map_err(bind)
    }
    fn has_seal_of_passage(&mut self, _: &P) -> nc::Result<bool> {
        refuse("hasSealOfPassage (Lunar Isle content)")
    }
    fn random_event_ids(&mut self) -> nc::Result<Vec<i32>> {
        Ok(super::talk::RANDOM_EVENT_IDS.to_vec())
    }
    fn anti_macro_event_npc(&mut self, _: &P) -> nc::Result<Option<P>> {
        // No random event NPC is ever spawned for a player here (the
        // events are content): AntiMacro.getEventNpc answers null.
        self.live
            .inner
            .not_ported
            .default_answer("AntiMacro.getEventNpc (random event content not ported: null)");
        Ok(None)
    }
    fn finalized(&mut self, _: &P) -> nc::Result<bool> {
        refuse("RandomEventNPC.finalized (random event content)")
    }
    fn talk_to(&mut self, _: &P, _: &P) -> nc::Result<()> {
        refuse("RandomEventNPC.talkTo (random event content)")
    }
    fn holiday_random_ids(&mut self) -> nc::Result<Vec<i32>> {
        Ok(super::talk::HOLIDAY_RANDOM_IDS.to_vec())
    }
    fn holiday_event_npc(&mut self, _: &P) -> nc::Result<Option<P>> {
        self.live
            .inner
            .not_ported
            .default_answer("HolidayRandoms.getEventNpc (holiday content not ported: null)");
        Ok(None)
    }
    fn barcrawl_instance(&mut self, _: &P) -> nc::Result<()> {
        refuse("BarcrawlManager (quest content)")
    }
    fn barcrawl_finished(&mut self, _: &P) -> nc::Result<bool> {
        refuse("BarcrawlManager (quest content)")
    }
    fn barcrawl_started(&mut self, _: &P) -> nc::Result<bool> {
        refuse("BarcrawlManager (quest content)")
    }
    fn barcrawl_completed(&mut self, _: &P, _: i32) -> nc::Result<bool> {
        refuse("BarcrawlManager (quest content)")
    }
}

// ---------------------------------------------------------------- diaries

/// `player.getAchievementDiaryManager()`: the persisted diaries taken out of
/// the profile for one call and put back after it.
pub(super) struct Diaries<'a> {
    live: &'a mut Live,
    owner: P,
    manager: crate::player::persisted::diaries::Manager,
}

pub(super) fn with_diaries<T>(
    live: &mut Live,
    owner: &P,
    f: impl FnOnce(&mut Diaries<'_>, &P) -> pm::Result<T>,
) -> pm::Result<T> {
    let manager = live
        .inner
        .with_profile(owner, |profile| {
            std::mem::take(&mut profile.persisted.diaries)
        })
        .map_err(|e| pm::Error::Binding(format!("{e:?}")))?;
    let mut h = Diaries {
        live,
        owner: owner.clone(),
        manager,
    };
    let result = f(&mut h, owner);
    let Diaries { live, manager, .. } = h;
    live.inner
        .with_profile(owner, |profile| profile.persisted.diaries = manager)
        .map_err(|e| pm::Error::Binding(format!("{e:?}")))?;
    live.inner.drain(owner);
    result
}

fn prefuse<T>(what: &str) -> pm::Result<T> {
    Err(pm::Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

impl diary::Host for Diaries<'_> {
    type Player = P;
    fn diaries(
        &mut self,
        player: &P,
    ) -> pm::Result<&mut crate::player::persisted::diaries::Manager> {
        if player.node() != self.owner.node() {
            return prefuse("another player's diaries in the same call");
        }
        Ok(&mut self.manager)
    }
    fn completed_levels(&mut self) -> &mut Vec<i32> {
        &mut self.live.inner.world.completed_levels
    }
    fn send_string(
        &mut self,
        player: &P,
        text: &str,
        interface: i32,
        child: i32,
    ) -> pm::Result<()> {
        self.live
            .inner
            .text(player, text, interface, child)
            .map_err(|e| pm::Error::Binding(format!("{e:?}")))
    }
    fn message(&mut self, player: &P, text: &str) -> pm::Result<()> {
        self.live
            .inner
            .message(player, text)
            .map_err(|e| pm::Error::Binding(format!("{e:?}")))
    }
    fn messages(&mut self, player: &P, lines: &[String]) -> pm::Result<()> {
        for line in lines {
            diary::Host::message(self, player, line)?;
        }
        Ok(())
    }
    fn npc_name(&mut self, id: i32) -> pm::Result<Option<String>> {
        self.live
            .npc_definition_name(id)
            .map_err(|e| pm::Error::Binding(format!("{e:?}")))
    }
    fn open_tab(&mut self, _: &P, _: i32, _: i32) -> pm::Result<()> {
        prefuse("the diary tab's openTab")
    }
    fn is_opened(&mut self, _: &P) -> pm::Result<bool> {
        prefuse("the diary interface")
    }
    fn open_component(&mut self, _: &P, _: i32) -> pm::Result<()> {
        prefuse("the diary interface")
    }
    fn run_script(&mut self, _: &P, _: i32, _: &str, _: &[i32]) -> pm::Result<()> {
        prefuse("the diary interface's script")
    }
    fn is_artificial(&mut self, _: &P) -> pm::Result<bool> {
        Ok(false)
    }
    fn debug_enabled(&mut self, player: &P) -> pm::Result<bool> {
        self.live
            .inner
            .canonical(player)
            .player
            .attributes()
            .boolean("debug", false)
            .map_err(|_| pm::Error::Cast)
    }
    fn debug_message(&mut self, player: &P, text: &str) -> pm::Result<()> {
        diary::Host::message(self, player, text)
    }
    fn contains(&mut self, _: &P, _: pm::Kind, _: pm::Item) -> pm::Result<bool> {
        prefuse("diary rewards (containers)")
    }
    fn remove(&mut self, _: &P, _: pm::Kind, _: pm::Item) -> pm::Result<bool> {
        prefuse("diary rewards (containers)")
    }
    fn add(&mut self, _: &P, _: pm::Kind, _: pm::Item) -> pm::Result<bool> {
        prefuse("diary rewards (containers)")
    }
    fn free_slots(&mut self, _: &P, _: pm::Kind) -> pm::Result<i32> {
        prefuse("diary rewards (containers)")
    }
    fn get(&mut self, _: &P, _: pm::Kind, _: i32) -> pm::Result<Option<pm::Item>> {
        prefuse("diary rewards (containers)")
    }
    fn item_name(&mut self, id: i32) -> pm::Result<Option<String>> {
        Ok(self.live.inner.items.for_id(id).read().name.clone())
    }
}

impl Live {
    /// `NPCDefinition.forId(id).getName()`.
    pub fn npc_definition_name(&mut self, id: i32) -> Result<Option<String>, String> {
        let definition = self.npc_definition(id)?;
        let name = definition.read().name.clone();
        Ok(name)
    }
}

/// The Object... args of a dialogue open from an NPC class body.
fn dialogue_args(args: &[Arg<P>]) -> nc::Result<Vec<crate::world::dialogue::Arg<Live>>> {
    use crate::world::dialogue::Arg as D;
    args.iter()
        .map(|a| {
            Ok(match a {
                Arg::Node(None) => D::Null,
                Arg::Node(Some(n)) => D::Npc(n.clone()),
                Arg::Int(i) => D::Int(*i),
                Arg::Bool(b) => D::Bool(*b),
                Arg::Barcrawl(_) => D::Other("BarcrawlType"),
            })
        })
        .collect()
}
