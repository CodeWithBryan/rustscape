//! `WoodcuttingSkillPulse` in the live world, started by AxeOnTree (an axe
//! used on a tree; super::withobject). The pulse is
//! `world::gathering::woodcutting::Pulse`, verified by the gathering corpus
//! (docs/gathering.md); this file is its `service::Context` over the live
//! world, written without a corpus of its own.
//!
//! Answered through the native systems: the tree's id and active flag,
//! `Scenery.transform` and `SceneryBuilder.replace` over the cache scene
//! (the stump for the respawn duration), the player's location, animator,
//! messages, attributes, Skills (levels and both `addExperience` overloads),
//! the canonical inventory and equipment, the persisted Seers' Village
//! diary, the interpreter's plain dialogue, item names, the local players
//! and the repository's player count, the synth sound, and
//! `RandomFunction.RANDOM` and `ThreadLocalRandom`.
//!
//! Not ported, and answered by name (docs/live-world.md): the familiar's
//! boost (0: no familiar), SkillcapePerks (inactive), the bird nest drop,
//! the ResourceProducedEvent and the inferno adze's projectile (skipped),
//! and the farming patch of a farmed tree (none found, so such a tree's
//! depletion ends the pulse without a stump, as the original's does for an
//! object that is not a patch). The undercover tree 10041's NPC dialogue is
//! a refusal. The trees' own "Chop down" option is WoodcuttingListener's
//! script, which is not installed.
use super::{super::host::Live, Ctx, Node, P, everyday::Everyday};
use crate::{
    definitions::item::Shared,
    pulses,
    world::{
        animation::{Animation, Priority},
        combat_pulse, combat_swing, doors,
        gathering::{
            Settings,
            service::{Arg, Context, Error, Handle, Result, Service, Task},
            woodcutting,
        },
        item_on_object as io,
        packet_handlers::Container as Held,
        player_managers::diary,
        scenery::{self as builder, ReplaceMode},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates;

/// The pulse's class.
pub const PULSE_CLASS: &str = "content.global.skill.gather.woodcutting.WoodcuttingSkillPulse";

/// The running woodcutting pulses.
#[derive(Default)]
pub struct WoodcuttingWorld {
    /// By pulse object: the player's node, the tree and the pulse's state.
    pub pulses: Vec<(pulses::Handle, usize, doors::Handle, woodcutting::Pulse)>,
}

/// The player's token.
const PLAYER: Handle = 1;
/// The tree's token.
const TREE: Handle = 2;
/// The token of the last `Scenery.transform` result.
const STUMP: Handle = 3;
/// Another player's token is this plus its node.
const OTHERS: Handle = 1 << 32;
/// `DiaryType.SEERS_VILLAGE`'s ordinal.
const SEERS_VILLAGE: usize = 5;

/// The pulse's services over the live world, for one player and tree.
struct Chop<'a> {
    live: &'a mut Live,
    p: P,
    tree: doors::Handle,
    stump: Option<doors::Handle>,
    /// What the last failing service said.
    error: Option<String>,
}

impl Chop<'_> {
    fn fail<T>(&mut self, what: impl std::fmt::Debug) -> Result<T> {
        self.error = Some(format!("{what:?}"));
        Err(Error::Service)
    }
    fn refuse<T>(&mut self, service: Service) -> Result<T> {
        self.error = Some(format!(
            "not ported in the live world: woodcutting service {}",
            service.name()
        ));
        Err(Error::Service)
    }
    fn node(&self) -> Node {
        Node::Player(self.p.0.clone())
    }
    fn scenery(&mut self, token: &Arg) -> Result<doors::Handle> {
        match token {
            Arg::Int(TREE) => Ok(self.tree.clone()),
            Arg::Int(STUMP) => self.stump.clone().ok_or(Error::Reply),
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

impl Context for Chop<'_> {
    fn call(&mut self, service: Service, args: &[Arg]) -> Result<Arg> {
        use Service as S;
        let p = self.p.clone();
        match service {
            S::NodeId => {
                let object = self.scenery(args.first().ok_or(Error::Reply)?)?;
                let id = object.read().id;
                Ok(id.into())
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
            S::PlayerSetAttribute => {
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
            S::SkillsGetLevel => {
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
            S::SkillsAddExperience2 | S::SkillsAddExperience3 => {
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
            S::InventoryFreeSlots => {
                let node = self.node();
                match Ctx(self.live).container(&node, Held::Inventory) {
                    Ok(inventory) => Ok(inventory.free_slots().into()),
                    Err(e) => self.fail(e),
                }
            }
            S::InventoryContains => {
                let node = self.node();
                match Ctx(self.live).container(&node, Held::Inventory) {
                    Ok(inventory) => Ok(inventory
                        .contains(int(args.first())?, int(args.get(1))?)
                        .into()),
                    Err(e) => self.fail(e),
                }
            }
            S::InventoryAdd => {
                let node = self.node();
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
            S::FamiliarGetBoost => {
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
            S::ItemDefinitionName => {
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
                        self.stump = Some(stump);
                        Ok(STUMP.into())
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
    fn defer(&mut self, service: Service, _: &[Arg], _: Task) -> Result<()> {
        // The pulse defers nothing; the listener's scripts are not installed.
        self.refuse(service)
    }
}

type Bound<T> = std::result::Result<T, io::Error>;

fn bind(e: impl std::fmt::Debug) -> io::Error {
    io::Error::Binding(Some(format!("{e:?}")))
}

impl Live {
    fn chop<T>(
        &mut self,
        p: &P,
        tree: &doors::Handle,
        f: impl FnOnce(&mut Chop<'_>) -> Result<T>,
    ) -> Bound<T> {
        let mut host = Chop {
            live: self,
            p: p.clone(),
            tree: tree.clone(),
            stump: None,
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
        let mut pulse = woodcutting::Pulse::new(PLAYER, TREE);
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
}
