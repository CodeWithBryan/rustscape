//! Woodcutting in the live world: `WoodcuttingSkillPulse`, started by
//! AxeOnTree (an axe used on a tree; super::withobject), and
//! `WoodcuttingListener`, the trees' own options ("Chop down" and its
//! four spellings) as an interaction script of the player's
//! ScriptProcessor (super::super::scripts). Both bodies are
//! `world::gathering::woodcutting`'s, verified by the gathering corpus
//! (docs/gathering.md); this file is their `service::Context` over the live
//! world, written without a corpus of its own.
//!
//! Answered through the native systems: the tree's id and active flag,
//! `Scenery.transform` and `SceneryBuilder.replace` over the cache scene
//! (the stump for the respawn duration), the player's location, animator,
//! messages, attributes, Skills (levels and both `addExperience` overloads),
//! the canonical inventory and equipment, the persisted Seers' Village
//! diary, the interpreter's plain dialogue, item names, the local players
//! and the repository's player count, the synth sound, and
//! `RandomFunction.RANDOM` and `ThreadLocalRandom`. For the listener also
//! the player's clocks (`finishedMoving`, `clockReady`, `delayClock`), the
//! ScriptProcessor (`restartScript`, `clearScripts`, `keepRunning`,
//! `stopExecuting`, `queueScript` with the queued lambda's captures),
//! ContentAPI `sendMessage` and `addItem`, the `instachop` attribute, the
//! persisted Lost City stage and the Firemaking experience of a log the
//! inferno adze burns (`Log.forId`).
//!
//! Not ported, and answered by name (docs/live-world.md): the familiar's
//! boost (0: no familiar), SkillcapePerks (inactive), the bird nest drop,
//! the ResourceProducedEvent and the inferno adze's projectile (skipped),
//! and the farming patch of a farmed tree (none found, so such a tree's
//! depletion ends without a stump, as the original's does for an object
//! that is not a patch). The undercover tree 10041's NPC dialogue (the
//! pulse) and Kjallak's dialogue on Miscellania and Etceteria (the
//! listener) are refusals.
use super::{
    super::{
        host::Live,
        scripts::{Interaction, Queued, ScriptCall},
    },
    Ctx, Node, P,
    everyday::Everyday,
};
use crate::{
    definitions::item::Shared,
    pulses,
    scripts::{self, Strength},
    world::{
        animation::{Animation, Priority},
        combat_pulse, combat_swing, doors,
        gathering::{
            Settings,
            binding::EQUIPMENT_SLOTS,
            service::{Arg, Context, Error, Handle, Result, Service, Task},
            woodcutting,
        },
        interaction_listeners::{self as listeners, Function, FunctionKind},
        item_on_object as io,
        packet_handlers::Container as Held,
        player_managers::diary,
        player_walk::NodeHandle,
        scenery::{self as builder, ReplaceMode},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates;

/// The pulse's class.
pub const PULSE_CLASS: &str = "content.global.skill.gather.woodcutting.WoodcuttingSkillPulse";

/// The listener's class (`scan.contentInterfaces`).
pub const LISTENER_CLASS: &str = "content.global.skill.gather.woodcutting.WoodcuttingListener";
/// The class of the lambdas the listener queues.
const QUEUED_CLASS: &str =
    "content.global.skill.gather.woodcutting.WoodcuttingListener$handleWoodcutting$1";
/// `Clocks.MOVEMENT`.
const MOVEMENT: i32 = 0;
/// `Clocks.ANIMATION_END`.
const ANIMATION_END: i32 = 1;
/// StarBonus's timer identifier.
const STAR_BONUS: &str = "shootingstar:bonus";
/// The `Quest` index of Lost City (`LostCity`'s constructor).
const LOST_CITY: i32 = 83;

/// `content.global.skill.firemaking.Log`: each log's id and experience (the
/// rest of the enum belongs to firemaking, which is not ported).
const FIREMAKING_LOGS: [(i32, f64); 18] = [
    (1511, 40.0),
    (2862, 40.0),
    (1521, 60.0),
    (1519, 90.0),
    (6333, 105.0),
    (10810, 125.0),
    (1517, 135.0),
    (6332, 157.5),
    (12581, 193.5),
    (1515, 202.5),
    (1513, 303.8),
    (13567, 303.8),
    (10329, 50.0),
    (10328, 50.0),
    (7406, 50.0),
    (7405, 50.0),
    (7404, 50.0),
    (3125, 50.0),
];

/// The running woodcutting pulses.
#[derive(Default)]
pub struct WoodcuttingWorld {
    /// By pulse object: the player's node, the tree and the pulse's state.
    pub pulses: Vec<(pulses::Handle, usize, doors::Handle, woodcutting::Pulse)>,
    /// WoodcuttingListener is registered (the trees' own options).
    pub listener: bool,
}

/// The player's token.
pub(super) const PLAYER: Handle = 1;
/// The token of the tree or rock.
pub(super) const NODE: Handle = 2;
/// The token of the last scenery the body made.
const MADE: Handle = 3;
/// Another player's token is this plus its node.
const OTHERS: Handle = 1 << 32;
/// `DiaryType.SEERS_VILLAGE`'s ordinal.
const SEERS_VILLAGE: usize = 5;

/// The gathering bodies' services over the live world, for one player and
/// one tree or rock (super::mining answers the mining bodies with it).
pub(super) struct Gather<'a> {
    pub(super) live: &'a mut Live,
    pub(super) p: P,
    /// The tree or rock.
    node: doors::Handle,
    /// The last scenery the body made (`Scenery.transform`, `Scenery(...)`).
    made: Option<doors::Handle>,
    /// The skill, for a refusal's name.
    skill: &'static str,
    /// What the last failing service said.
    error: Option<String>,
}

impl Gather<'_> {
    fn fail<T>(&mut self, what: impl std::fmt::Debug) -> Result<T> {
        self.error = Some(format!("{what:?}"));
        Err(Error::Service)
    }
    fn refuse<T>(&mut self, service: Service) -> Result<T> {
        self.error = Some(format!(
            "not ported in the live world: {} service {}",
            self.skill,
            service.name()
        ));
        Err(Error::Service)
    }
    fn player_node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    fn scenery(&mut self, token: &Arg) -> Result<doors::Handle> {
        match token {
            Arg::Int(NODE) => Ok(self.node.clone()),
            Arg::Int(MADE) => self.made.clone().ok_or(Error::Reply),
            _ => Err(Error::Reply),
        }
    }
    fn player(&mut self, token: &Arg) -> Result<P> {
        match token {
            Arg::Int(PLAYER) => Ok(self.p.clone()),
            Arg::Int(other) if *other >= OTHERS => {
                let node = (*other - OTHERS) as usize;
                match self.live.inner.players.get(&node) {
                    Some(_) => Ok(P(self
                        .live
                        .inner
                        .state
                        .retain_node(node)
                        .map_err(|_| Error::Service)?)),
                    None => Err(Error::Reply),
                }
            }
            _ => Err(Error::Reply),
        }
    }
    /// A clock of the player (`Clocks`' slot).
    fn clock(&mut self, clock: i32) -> Result<i32> {
        let index = usize::try_from(clock).or(Err(Error::IndexOutOfBounds))?;
        let p = self.p.clone();
        match self.live.script_clock(&p, index) {
            Ok(v) => Ok(v),
            Err(crate::scripts::Error::Index) => Err(Error::IndexOutOfBounds),
            Err(e) => self.fail(e),
        }
    }
    /// A ContentAPI call into the player's ScriptProcessor.
    fn script(&mut self, call: ScriptCall) -> Result<Arg> {
        let p = self.p.clone();
        match self.live.script_call(&p, call) {
            Ok(v) => Ok(v.into()),
            Err(e) => self.fail(e),
        }
    }
    fn seers_level_complete(&mut self, level: i32) -> Result<bool> {
        let p = self.p.clone();
        let answer = super::super::subclasses::with_diaries(self.live, &p, |h, owner| {
            let manager = diary::Host::diaries(h, owner)?;
            let found = manager.diaries.iter().find(|d| d.kind == SEERS_VILLAGE);
            diary::level_complete(
                found.ok_or(crate::world::player_managers::Error::Null)?,
                level,
            )
        });
        answer.or_else(|e| self.fail(e))
    }
}

fn int(arg: Option<&Arg>) -> Result<i32> {
    match arg {
        Some(Arg::Int(v)) => i32::try_from(*v).map_err(|_| Error::Reply),
        _ => Err(Error::Reply),
    }
}
fn text(arg: Option<&Arg>) -> Result<&str> {
    match arg {
        Some(Arg::Text(v)) => Ok(v),
        _ => Err(Error::Reply),
    }
}
fn double(arg: Option<&Arg>) -> Result<f64> {
    match arg {
        Some(Arg::Double(v)) => Ok(*v),
        _ => Err(Error::Reply),
    }
}

impl Context for Gather<'_> {
    fn call(&mut self, service: Service, args: &[Arg]) -> Result<Arg> {
        use Service as S;
        let p = self.p.clone();
        match service {
            S::NodeId => {
                let object = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let id = object.read().id;
                Ok(id.into())
            }
            S::NodeIsActive => {
                let object = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let active = object.read().active;
                Ok(active.into())
            }
            S::NodeSetActive => {
                let object = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let Some(Arg::Bool(active)) = args.get(1) else {
                    return Err(Error::Reply);
                };
                let walk = &mut self.live.walk;
                let mut world = walk.scene.context(&mut walk.maps);
                builder::set_active(&mut world, &object, *active).or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::PlayerLocation => {
                self.player(args.first().ok_or(Error::Reply)?)?;
                match self.live.inner.state.node_location(p.node()) {
                    Ok(Some(l)) => Ok(l.coordinates().into()),
                    other => self.fail(other.map(|_| "no location")),
                }
            }
            S::PlayerAnimate => {
                let id = int(args.first())?;
                let animation = match args.get(1) {
                    Some(Arg::Null) => Animation::new(id),
                    Some(Arg::Text(priority)) if priority == "HIGH" => {
                        Animation::with(id, 0, Some(Priority::High))
                    }
                    _ => return Err(Error::Reply),
                };
                self.live
                    .entity_visualize(&p, Some(animation), None)
                    .or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::PacketDispatchSendMessage | S::PlayerSendMessage => {
                let line = text(args.first())?;
                self.live
                    .inner
                    .message(&p, line)
                    .or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::PlayerDispatch => {
                self.live
                    .inner
                    .not_ported
                    .skip("ResourceProducedEvent dispatch (entity event hooks not ported)");
                Ok(Arg::Null)
            }
            S::PlayerGetAttribute => {
                let attributes = self.live.inner.canonical(&p).player.attributes();
                match attributes.integer(text(args.first())?, int(args.get(1))?) {
                    Ok(v) => Ok(v.into()),
                    // A value of another type is the read's ClassCastException.
                    Err(_) => Err(Error::ClassCast),
                }
            }
            S::PlayerSetAttribute | S::ApiSetAttribute => {
                let attributes = self.live.inner.canonical(&p).player.attributes();
                attributes.set_integer(text(args.first())?, int(args.get(1))?);
                Ok(Arg::Null)
            }
            S::AnimatorIsAnimating => {
                let animator = self.live.inner.canonical(&p).animator.clone();
                match animator.is_animating(&mut self.live.walk.animations) {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::SkillsGetLevel | S::ApiGetDynLevel => {
                match combat_swing::Host::level(self.live, &p, int(args.first())?) {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::SkillsGetStaticLevel => {
                match combat_swing::Host::static_level(self.live, &p, int(args.first())?) {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::SkillsAddExperience2 | S::SkillsAddExperience3 | S::ApiRewardXp => {
                // `addExperience(slot, experience)` is the three-argument
                // one with `playerMod` false.
                let player_mod = match args.get(2) {
                    Some(Arg::Bool(v)) => *v,
                    None => false,
                    _ => return Err(Error::Reply),
                };
                let added = combat_pulse::Host::add_experience(
                    self.live,
                    &p,
                    int(args.first())?,
                    double(args.get(1))?,
                    player_mod,
                );
                self.live.inner.drain(&p);
                added.or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::InventoryFreeSlots | S::ApiFreeSlots => {
                let node = self.player_node();
                match Ctx(self.live).container(&node, Held::Inventory) {
                    Ok(inventory) => Ok(inventory.free_slots().into()),
                    Err(e) => self.fail(e),
                }
            }
            S::InventoryContains => {
                let node = self.player_node();
                match Ctx(self.live).container(&node, Held::Inventory) {
                    Ok(inventory) => Ok(inventory
                        .contains(int(args.first())?, int(args.get(1))?)
                        .into()),
                    Err(e) => self.fail(e),
                }
            }
            S::InventoryAdd => {
                let node = self.player_node();
                let (id, amount) = (int(args.first())?, int(args.get(1))?);
                match Everyday(Ctx(self.live)).add_to_inventory(&node, id, amount) {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::EquipmentGetNew => {
                match combat_swing::Host::equipment_new(self.live, &p, int(args.first())?) {
                    Ok(item) => Ok(item.read().id().into()),
                    Err(e) => self.fail(e),
                }
            }
            S::EquipmentGet => {
                match combat_swing::Host::equipment(self.live, &p, int(args.first())?) {
                    Ok(Some(item)) => Ok(item.read().id().into()),
                    Ok(None) => Ok(Arg::Null),
                    Err(e) => self.fail(e),
                }
            }
            S::FamiliarGetBoost | S::ApiGetFamiliarBoost => {
                self.live
                    .inner
                    .not_ported
                    .default_answer("FamiliarManager.getBoost (familiars not ported: 0)");
                Ok(0.into())
            }
            S::DiaryIsComplete => match text(args.first())? {
                "SEERS_VILLAGE" => Ok(self.seers_level_complete(int(args.get(1))?)?.into()),
                _ => self.refuse(service),
            },
            S::DialogueSendDialogue => {
                // `dialogueInterpreter.sendDialogue(String...)`.
                let Some(Arg::List(lines)) = args.first() else {
                    return Err(Error::Reply);
                };
                let mut units = Vec::new();
                for line in lines {
                    units.push(text(Some(line))?.encode_utf16().collect::<Vec<u16>>());
                }
                let interpreter = self.live.interpreter_of(&p);
                let sent = interpreter
                    .send_dialogue(self.live, Some(&crate::world::dialogue_dsl::lines(&units)))
                    .map(|_| ());
                self.live.inner.drain(&p);
                sent.or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::ItemDefinitionName | S::ApiGetItemName | S::ItemName => {
                let name = self
                    .live
                    .inner
                    .items
                    .for_id(int(args.first())?)
                    .read()
                    .name
                    .clone();
                name.map(Arg::Text).ok_or(Error::NullPointer)
            }
            S::LowercaseDefault => Ok(lowercase_root(text(args.first())?).into()),
            S::SkillcapePerksIsActive => {
                self.live
                    .inner
                    .not_ported
                    .default_answer("SkillcapePerks.isActive (perks not ported: false)");
                Ok(false.into())
            }
            S::BirdNestDrop => {
                self.live
                    .inner
                    .not_ported
                    .skip("BirdNest.drop (bird nests not ported)");
                Ok(Arg::Null)
            }
            S::ProjectileCreate | S::ProjectileTransform | S::ProjectileSend => {
                self.live
                    .inner
                    .not_ported
                    .skip("the inferno adze's projectile (content projectiles not joined)");
                Ok(Arg::Null)
            }
            S::FiremakingLogForId => {
                // `Log.forId(reward)?.xp`.
                let id = int(args.first())?;
                Ok(FIREMAKING_LOGS
                    .iter()
                    .find(|(log, _)| *log == id)
                    .map_or(Arg::Null, |(_, xp)| (*xp).into()))
            }
            S::FarmingPatchForObject => {
                self.live
                    .inner
                    .not_ported
                    .default_answer("FarmingPatch.forObject (farming not ported: null)");
                Ok(Arg::Null)
            }
            S::SceneryTransform => {
                // `node.transform(id)`: the id at the node's location, with
                // its type and rotation.
                let object = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let (location, shape, rotation) = {
                    let object = object.read();
                    (object.location.clone(), object.shape, object.rotation)
                };
                let constructed = crate::world::scenery_node::construct(
                    &mut self.live.walk.scene.definitions,
                    int(args.get(1))?,
                    location,
                    shape,
                    rotation,
                    false,
                );
                match constructed {
                    Ok(stump) => {
                        self.made = Some(stump);
                        Ok(MADE.into())
                    }
                    Err(e) => self.fail(e),
                }
            }
            S::SceneryBuilderReplace => {
                // `SceneryBuilder.replace(remove, construct, restoreTicks)`.
                let remove = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let construct = self.scenery(args.get(1).ok_or(Error::Reply)?)?;
                let ticks = int(args.get(2))?;
                let walk = &mut self.live.walk;
                let mut world = walk.scene.context(&mut walk.maps);
                let mode = ReplaceMode::Temporary { clip: true, ticks };
                match builder::replace(&mut world, Some(&remove), Some(&construct), mode) {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::RepositoryPlayersSize => Ok((self.live.inner.players.len() as i32).into()),
            S::RegionLocalPlayers => {
                let Some(Arg::List(at)) = args.first() else {
                    return Err(Error::Reply);
                };
                let at = doors::Location::new(int(at.first())?, int(at.get(1))?, int(at.get(2))?);
                let found = self.live.local(
                    at,
                    int(args.get(1))?,
                    crate::world::entity_regions::Select::Players,
                );
                match found {
                    Ok(players) => Ok(Arg::List(
                        players
                            .iter()
                            .map(|h| {
                                if h.node == p.node() {
                                    PLAYER.into()
                                } else {
                                    Arg::Int(OTHERS + h.node as i64)
                                }
                            })
                            .collect(),
                    )),
                    Err(e) => self.fail(e),
                }
            }
            S::ApiPlayAudio => {
                // `playAudio(player, id)`: the synth sound, once, no delay.
                let to = self.player(args.first().ok_or(Error::Reply)?)?;
                let id = int(args.get(1))?;
                self.live
                    .inner
                    .queue_native(&to, updates::synth_sound(id, 1, 0));
                Ok(Arg::Null)
            }
            // ---- WoodcuttingListener's ContentAPI calls: the player's
            // clocks and ScriptProcessor (super::super::scripts).
            S::ApiFinishedMoving => {
                // clocks[MOVEMENT] < GameWorld.ticks.
                let clock = self.clock(MOVEMENT)?;
                Ok((clock < self.live.inner.ticks).into())
            }
            S::ApiClockReady => {
                let clock = self.clock(int(args.first())?)?;
                Ok((clock <= self.live.inner.ticks).into())
            }
            S::ApiDelayClock => {
                // delayClock(player, clock, ticks, delayScript).
                let (clock, ticks) = (int(args.first())?, int(args.get(1))?);
                let Some(Arg::Bool(delay)) = args.get(2) else {
                    return Err(Error::Reply);
                };
                let value = self.live.inner.ticks.wrapping_add(ticks);
                let index = usize::try_from(clock).or(Err(Error::IndexOutOfBounds))?;
                self.live
                    .set_script_clock(&p, index, value)
                    .or_else(|e| self.fail(e))?;
                if *delay {
                    return self.script(ScriptCall::Delay(ticks));
                }
                Ok(false.into())
            }
            S::ApiRestartScript => self.script(ScriptCall::Restart),
            S::ApiClearScripts => self.script(ScriptCall::Clear),
            S::ApiKeepRunning => self.script(ScriptCall::KeepRunning),
            S::ApiStopExecuting => self.script(ScriptCall::Stop),
            S::ApiSendMessage => {
                // sendMessage(player, message).
                let to = self.player(args.first().ok_or(Error::Reply)?)?;
                let line = text(args.get(1))?;
                // `player.sendMessages(*splitLines(message, 86))`.
                Everyday(Ctx(self.live))
                    .message(&Node::Player(to.0), line)
                    .or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::ApiGetAttribute => {
                // getAttribute(player, key, default) of a Boolean or an Int.
                let attributes = self.live.inner.canonical(&p).player.attributes();
                let key = text(args.first())?;
                let read = match args.get(1) {
                    Some(Arg::Bool(default)) => attributes.boolean(key, *default).map(Arg::from),
                    Some(Arg::Int(_)) => attributes.integer(key, int(args.get(1))?).map(Arg::from),
                    _ => return Err(Error::Reply),
                };
                // A value of another type is the read's ClassCastException.
                read.or(Err(Error::ClassCast))
            }
            S::ApiAddItem => {
                // addItem(player, id, amount, Container.INVENTORY).
                if text(args.get(2))? != "INVENTORY" {
                    return self.refuse(service);
                }
                let node = self.player_node();
                let (id, amount) = (int(args.first())?, int(args.get(1))?);
                match Everyday(Ctx(self.live)).add_to_inventory(&node, id, amount) {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::ApiGetQuestStage => {
                // getQuestStage(player, quest): the stage the player's
                // QuestRepository holds for the quest's index (0 for a
                // quest its save lacks).
                let index = match text(args.first())? {
                    "LOST_CITY" => LOST_CITY,
                    _ => return self.refuse(service),
                };
                let stage = self.live.inner.with_profile(&p, |profile| {
                    profile.persisted.quests.stages.get(index).copied()
                });
                match stage {
                    Ok(stage) => Ok(stage.unwrap_or(0).into()),
                    Err(e) => self.fail(e),
                }
            }
            // ---- MiningListener's and ProspectListener's calls
            // (super::mining).
            S::NodeLocation => {
                let object = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let at = object.read().location.coordinates();
                Ok(at.into())
            }
            S::SceneryType => {
                let object = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let shape = object.read().shape;
                Ok(shape.into())
            }
            S::SceneryRotation => {
                let object = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let rotation = object.read().rotation;
                Ok(rotation.into())
            }
            S::SceneryNew => {
                // Scenery(id, location, type, rotation).
                let Some(Arg::List(at)) = args.get(1) else {
                    return Err(Error::Reply);
                };
                let at = doors::Location::new(int(at.first())?, int(at.get(1))?, int(at.get(2))?);
                let constructed = crate::world::scenery_node::construct(
                    &mut self.live.walk.scene.definitions,
                    int(args.first())?,
                    at,
                    int(args.get(2))?,
                    int(args.get(3))?,
                    false,
                );
                match constructed {
                    Ok(made) => {
                        self.made = Some(made);
                        Ok(MADE.into())
                    }
                    Err(e) => self.fail(e),
                }
            }
            S::ApiAnimationFinished => {
                // clocks[ANIMATION_END] < GameWorld.ticks.
                let clock = self.clock(ANIMATION_END)?;
                Ok((clock < self.live.inner.ticks).into())
            }
            S::ApiAnimate => {
                // animate(entity, anim, forced): `animator.animate`.
                let id = int(args.first())?;
                let animation = match args.get(1) {
                    Some(Arg::Null) => Animation::new(id),
                    Some(Arg::Text(priority)) if priority == "HIGH" => {
                        Animation::with(id, 0, Some(Priority::High))
                    }
                    _ => return Err(Error::Reply),
                };
                if args.get(2) != Some(&Arg::Bool(false)) {
                    // `animator.forceAnimation`: no gathering body asks.
                    return self.refuse(service);
                }
                self.live
                    .entity_visualize(&p, Some(animation), None)
                    .or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::ApiDelayScript => self.script(ScriptCall::Delay(int(args.first())?)),
            S::ScriptsReset => self.script(ScriptCall::Clear).map(|_| Arg::Null),
            S::ApiGetItemFromEquipment => {
                // `player.equipment.get(slot.ordinal)`.
                let name = text(args.first())?;
                let slot = EQUIPMENT_SLOTS
                    .iter()
                    .position(|slot| *slot == name)
                    .ok_or(Error::Reply)?;
                match combat_swing::Host::equipment(self.live, &p, slot as i32) {
                    Ok(Some(item)) => Ok(item.read().id().into()),
                    Ok(None) => Ok(Arg::Null),
                    Err(e) => self.fail(e),
                }
            }
            S::ApiInEquipment => {
                // inEquipment(player, id, amount): the item in the slot the
                // definition's `equipment_slot` names, if it is this id.
                let (id, amount) = (int(args.first())?, int(args.get(1))?);
                let slot = {
                    let definition = self.live.inner.items.for_id(id);
                    let definition = definition.read();
                    match definition.handlers.get("equipment_slot") {
                        Some(crate::definitions::property::Property::Int(slot)) => *slot,
                        None => -1,
                        Some(_) => return Err(Error::ClassCast),
                    }
                };
                let worn = if slot < 0 {
                    None
                } else {
                    match combat_swing::Host::equipment(self.live, &p, slot) {
                        Ok(item) => item,
                        Err(e) => return self.fail(e),
                    }
                };
                let held = worn.map_or(0, |item| {
                    let item = item.read();
                    if item.id() == id { item.amount() } else { 0 }
                });
                Ok((held >= amount).into())
            }
            S::ApiRemoveItem => {
                // removeItem(player, id, container): `remove(Item(id))`.
                let node = self.player_node();
                let item = Everyday(Ctx(self.live)).native(int(args.first())?, 1);
                let held = match text(args.get(1))? {
                    "INVENTORY" => Held::Inventory,
                    "EQUIPMENT" => Held::Equipment,
                    _ => return self.refuse(service),
                };
                let removed = Ctx(self.live).with_container(&node, held, |c, defs, ctx| {
                    c.remove(Some(&item), None, true, defs, ctx)
                });
                self.live.inner.drain(&p);
                match removed {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::ApiAddItemOrDrop => {
                let node = self.player_node();
                let (id, amount) = (int(args.first())?, int(args.get(1))?);
                let added = crate::world::global_items2::Host::add_item_or_drop(
                    &mut Everyday(Ctx(self.live)),
                    &node,
                    id,
                    amount,
                );
                self.live.inner.drain(&p);
                added.or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::ApiSendDialogue => {
                // sendDialogue(player, message): splitLines, then the
                // interpreter's plain dialogue.
                let node = self.player_node();
                Everyday(Ctx(self.live))
                    .dialogue(&node, text(args.first())?)
                    .or_else(|e| self.fail(e))?;
                Ok(Arg::Null)
            }
            S::ApiInBorders => {
                let borders = [
                    int(args.first())?,
                    int(args.get(1))?,
                    int(args.get(2))?,
                    int(args.get(3))?,
                ];
                match self.live.in_borders(&p, borders) {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::ApiHasTimerActive => match text(args.first())? {
                // hasTimerActive<StarBonus>(player): the player's timer
                // of that class (its identifier is unique to it).
                "StarBonus" => match self.live.has_timer_of(p.node(), STAR_BONUS) {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                },
                _ => self.refuse(service),
            },
            S::DiaryGetArmour => {
                // achievementDiaryManager.armour.
                let answer = super::super::subclasses::with_diaries(self.live, &p, |h, owner| {
                    diary::armour(h, owner)
                });
                match answer {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::DiaryGetLevel => {
                // achievementDiaryManager.getDiary(type).level.
                let kind = match text(args.first())? {
                    "VARROCK" => diary::VARROCK,
                    _ => return self.refuse(service),
                };
                let answer = super::super::subclasses::with_diaries(self.live, &p, |h, owner| {
                    let manager = diary::Host::diaries(h, owner)?;
                    let found = manager.diaries.iter().find(|d| d.kind == kind);
                    diary::completed_level(found.ok_or(crate::world::player_managers::Error::Null)?)
                });
                match answer {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            S::RngMathNextDouble => Ok(self.live.inner.generators.math_random().into()),
            S::RngShuffleNextInt => {
                match self
                    .live
                    .inner
                    .generators
                    .shuffle_next_int(int(args.first())?)
                {
                    Some(v) => Ok(v.into()),
                    None => self.fail("Random.nextInt of a bound below 1"),
                }
            }
            S::RngThreadLocalNextInt => {
                match self
                    .live
                    .inner
                    .generators
                    .thread_local_next_int(int(args.first())?)
                {
                    Some(v) => Ok(v.into()),
                    None => self.fail("ThreadLocalRandom.nextInt of a bound below 1"),
                }
            }
            S::RngSharedNextInt => {
                match self
                    .live
                    .inner
                    .generators
                    .shared_next_int(int(args.first())?)
                {
                    Some(v) => Ok(v.into()),
                    None => self.fail("Random.nextInt of a bound below 1"),
                }
            }
            S::RngThreadLocalNextDouble => {
                let (origin, bound) = (double(args.first())?, double(args.get(1))?);
                match self
                    .live
                    .inner
                    .generators
                    .thread_local_next_double(origin, bound)
                {
                    Ok(v) => Ok(v.into()),
                    Err(e) => self.fail(e),
                }
            }
            _ => self.refuse(service),
        }
    }
    fn defer(&mut self, service: Service, args: &[Arg], task: Task) -> Result<()> {
        // queueScript(player, delay, strength) { ... }: the lambda with
        // the tree it captured, in the player's script queue.
        if service == Service::PulseManagerRun {
            // player.pulseManager.run(object : Pulse(delay) { ... }):
            // ProspectListener's message (super::mining).
            let (p, rock) = (self.p.clone(), self.node.clone());
            let delay = int(args.first())?;
            return self
                .live
                .run_prospect_pulse(&p, &rock, delay, task)
                .or_else(|e| self.fail(e));
        }
        if service != Service::ApiQueueScript || self.skill != "woodcutting" {
            return self.refuse(service);
        }
        let delay = int(args.first())?;
        let strength = match text(args.get(1))? {
            "WEAK" => Strength::Weak,
            "NORMAL" => Strength::Normal,
            "STRONG" => Strength::Strong,
            "SOFT" => Strength::Soft,
            _ => return Err(Error::Reply),
        };
        let Some(Arg::Bool(persist)) = args.get(2) else {
            return Err(Error::Reply);
        };
        let function = Function::new(
            FunctionKind::Void,
            QUEUED_CLASS,
            Queued::Woodcutting(task, self.node.clone()),
        );
        self.script(ScriptCall::Queue(function, delay, strength, *persist))
            .map(|_| ())
    }
}

pub(super) type Bound<T> = std::result::Result<T, io::Error>;

pub(super) fn bind(e: impl std::fmt::Debug) -> io::Error {
    io::Error::Binding(Some(format!("{e:?}")))
}

impl Live {
    fn chop<T>(
        &mut self,
        p: &P,
        tree: &doors::Handle,
        f: impl FnOnce(&mut Gather<'_>) -> Result<T>,
    ) -> Bound<T> {
        self.gather(p, tree, "woodcutting", f)
    }

    /// A gathering body over the live world, for one player and one tree
    /// or rock.
    pub(super) fn gather<T>(
        &mut self,
        p: &P,
        node: &doors::Handle,
        skill: &'static str,
        f: impl FnOnce(&mut Gather<'_>) -> Result<T>,
    ) -> Bound<T> {
        let mut host = Gather {
            live: self,
            p: p.clone(),
            node: node.clone(),
            made: None,
            skill,
            error: None,
        };
        f(&mut host).map_err(|e| match host.error.take() {
            Some(text) => io::Error::Binding(Some(text)),
            None => bind(e),
        })
    }

    /// `player.pulseManager.run(WoodcuttingSkillPulse(player, scenery))`:
    /// the constructor leaves the pulse stopped, `start()` reads the tree
    /// and, when the requirements hold, runs it with "You swing your axe at
    /// the tree...".
    pub fn run_woodcutting_pulse(&mut self, p: &P, tree: &doors::Handle) -> Bound<()> {
        let mut pulse = woodcutting::Pulse::new(PLAYER, NODE);
        // `Pulse(1, player, node)`; the tree's active flag is read by the
        // body below, the player leaving stops the pulse
        // (super::super::round).
        let handle = Shared::new(pulses::Pulse::new(
            woodcutting::Pulse::DELAY,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            PULSE_CLASS.into(),
        ));
        let mut started = Ok(());
        let submitted = self
            .run_content_pulse(p, handle.clone(), |live| {
                started = live.chop(p, tree, |host| pulse.start(host));
                started.is_ok() && pulse.running
            })
            .map_err(bind)?;
        if let Err(error) = started {
            // `start()` threw inside PulseManager.run: nothing is submitted.
            pulses::stop_base(&handle);
            return Err(error);
        }
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            self.round
                .woodcutting
                .pulses
                .push((handle, p.node(), tree.clone(), pulse));
        }
        Ok(())
    }

    /// A woodcutting pulse by identity: `Some` with whether the runner
    /// drops it. `Pulse.update`: an inactive tree stops the pulse before
    /// its body; a body that finishes or throws stops it.
    pub fn woodcutting_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.woodcutting.pulses;
        let at = pulses.iter().position(|(h, _, _, _)| h.same(pulse))?;
        let (handle, owner, tree, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        let settings = Settings {
            max_players: crate::world::repository::MAX_PLAYERS,
        };
        let active = tree.read().active;
        let done = if active {
            self.chop(&p, &tree, |host| task.pulse(host, &settings))
        } else {
            Ok(true)
        };
        match done {
            Ok(false) => {
                self.round
                    .woodcutting
                    .pulses
                    .push((handle, owner, tree, task));
                Some(Ok(false))
            }
            finished => {
                // `stop()`: the animation reset, then the base flag.
                let stopped = self.chop(&p, &tree, |host| task.stop(host));
                pulses::stop_base(&handle);
                Some(finished.and_then(|_| stopped.map(|()| true)))
            }
        }
    }

    /// A woodcutting pulse a pulse manager stopped: its `stop()`.
    pub fn woodcutting_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Bound<()>> {
        let pulses = &mut self.round.woodcutting.pulses;
        let at = pulses.iter().position(|(h, _, _, _)| h.same(pulse))?;
        let (_, owner, tree, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        Some(self.chop(&p, &tree, |host| task.stop(host)))
    }

    /// `WoodcuttingListener.defineListeners`: the trees' five options as
    /// one SCENERY interaction (persistent, distance 1) whose handler is
    /// [`Interaction::Woodcutting`].
    pub fn define_woodcutting_listener(&mut self) {
        let registration = woodcutting::registration();
        let options: Vec<Vec<u16>> = registration
            .options
            .iter()
            .map(|o| o.encode_utf16().collect())
            .collect();
        self.item_actions.listeners.add_metadata(
            &super::super::global::RootCasing,
            &registration.ids,
            listeners::Kind::Scenery,
            &options,
            Shared::new(listeners::Metadata {
                function: Interaction::Woodcutting.function(),
                distance: registration.allowed_distance,
                persist: registration.persistent,
            }),
        );
        self.round.woodcutting.listener = true;
    }

    /// `handleWoodcutting(player, node, state)`: one execution of the
    /// interaction script, with the processor's target as the tree.
    pub fn woodcutting_script(
        &mut self,
        p: &P,
        target: &NodeHandle,
        stage: i32,
    ) -> scripts::Result<bool> {
        let tree = self
            .inner
            .state
            .scenery(target.node)
            .map_err(crate::world::script_runtime::error)?;
        let settings = Settings {
            max_players: crate::world::repository::MAX_PLAYERS,
        };
        self.chop(p, &tree, |host| {
            woodcutting::handle_woodcutting(host, &settings, PLAYER, NODE, stage)
        })
        .map_err(crate::world::script_runtime::error)
    }

    /// A lambda the listener queued (`queueScript`), with its tree.
    pub fn woodcutting_queued(
        &mut self,
        p: &P,
        task: &Task,
        tree: &doors::Handle,
        stage: i32,
    ) -> scripts::Result<bool> {
        self.chop(p, tree, |host| woodcutting::run_task(host, task, stage))
            .map_err(crate::world::script_runtime::error)
    }
}
