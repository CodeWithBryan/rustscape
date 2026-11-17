//! WoodcuttingListener (the modern persistent interaction) and the legacy
//! WoodcuttingSkillPulse, statement for statement.
//!
//! The two bodies differ on purpose: sound draw order, the adze roll, bark
//! experience, the full-inventory message channel and the felling identifiers
//! are each kept as their own source has them.
use super::{
    Settings,
    nodes::WoodcuttingNode,
    random,
    service::{Arg, Calls, Context, Error, Handle, Result, Service, Task, region_id},
    tools::{self, FIREMAKING, Tool, WOODCUTTING},
};

/// `Sounds.WOODCUTTING_HIT_3038..3042`.
pub const HIT_SOUNDS: [i32; 5] = [3038, 3039, 3040, 3041, 3042];
/// `Sounds.TREE_FALLING_2734`.
pub const TREE_FALLING: i32 = 2734;
pub const INFERNO_ADZE: i32 = 13661;
pub const BARK: i32 = 3239;
const SKILLING: i32 = 7;
const SLOT_HAT: i32 = 0;
const LOGS_KEY: &str = "stats_manager:logs_chopped";
const SAVED_LOGS_KEY: &str = "/save:stats_manager:logs_chopped";

/// What `WoodcuttingListener.defineListeners` registers.
pub struct Registration {
    /// `WoodcuttingNode.values().map { it.id }`, repeats included.
    pub ids: Vec<i32>,
    pub options: [&'static str; 5],
    pub persistent: bool,
    pub allowed_distance: i32,
}
pub fn registration() -> Registration {
    Registration {
        ids: WoodcuttingNode::values().iter().map(|n| n.full).collect(),
        options: ["chop-down", "chop", "chop down", "cut down", "cut-branch"],
        persistent: true,
        allowed_distance: 1,
    }
}
/// `defineListeners`: one SCENERY interaction registration.
pub fn define_listeners(ctx: &mut (impl Context + ?Sized)) -> Result<()> {
    let r = registration();
    ctx.unit(
        Service::ListenersDefineInteraction,
        &[
            "SCENERY".into(),
            Arg::List(r.ids.iter().map(|&i| i.into()).collect()),
            Arg::List(r.options.iter().map(|&o| o.into()).collect()),
            r.persistent.into(),
            r.allowed_distance.into(),
        ],
    )
}

fn node_id(ctx: &mut (impl Context + ?Sized), node: Handle) -> Result<i32> {
    ctx.int(Service::NodeId, &[node.into()])
}
fn node_active(ctx: &mut (impl Context + ?Sized), node: Handle) -> Result<bool> {
    ctx.boolean(Service::NodeIsActive, &[node.into()])
}
fn player_location(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<[i32; 3]> {
    ctx.location(Service::PlayerLocation, &[player.into()])
}
fn packet_message(ctx: &mut (impl Context + ?Sized), text: String) -> Result<()> {
    ctx.unit(Service::PacketDispatchSendMessage, &[text.into()])
}
fn api_message(ctx: &mut (impl Context + ?Sized), player: Handle, text: &str) -> Result<()> {
    ctx.unit(Service::ApiSendMessage, &[player.into(), text.into()])
}
fn delay_clock(ctx: &mut (impl Context + ?Sized), ticks: i32) -> Result<bool> {
    ctx.boolean(
        Service::ApiDelayClock,
        &[SKILLING.into(), ticks.into(), false.into()],
    )
}
fn instachop(ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
    ctx.boolean(
        Service::ApiGetAttribute,
        &["instachop".into(), false.into()],
    )
}
fn reward_name(ctx: &mut (impl Context + ?Sized), reward: i32) -> Result<String> {
    let name = ctx.text(Service::ItemDefinitionName, &[reward.into()])?;
    ctx.lower_default(name)
}
fn seers_complete(ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
    ctx.boolean(
        Service::DiaryIsComplete,
        &["SEERS_VILLAGE".into(), 1.into()],
    )
}
/// The shared success roll: level and familiar boost, one thread-local draw,
/// then the tool tier arithmetic (a null tool fails only at `tool.ordinal`).
fn check_reward(
    ctx: &mut (impl Context + ?Sized),
    resource: Option<&WoodcuttingNode>,
    tool: Option<Tool>,
) -> Result<bool> {
    let level = ctx
        .int(Service::SkillsGetLevel, &[WOODCUTTING.into()])?
        .wrapping_add(ctx.int(Service::FamiliarGetBoost, &[WOODCUTTING.into()])?);
    let host_ratio = random::random_double(ctx, 100.0)?;
    let resource = resource.ok_or(Error::NullPointer)?;
    let black = tool.is_some_and(|t| t.name == "BLACK_AXE");
    let low_mod = if black {
        resource.tier_mod_low / 2.0
    } else {
        resource.tier_mod_low
    };
    let ordinal = tool.ok_or(Error::NullPointer)?.ordinal as f64;
    let low = resource.base_low + ordinal * low_mod;
    let high_mod = if black {
        resource.tier_mod_high / 2.0
    } else {
        resource.tier_mod_high
    };
    let high = resource.base_high + ordinal * high_mod;
    Ok(host_ratio < random::skill_success_chance(low, high, level))
}
/// `calculateRewardAmount`, identical in both sources.
fn calculate_reward_amount(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    reward: i32,
) -> Result<i32> {
    let mut amount = 1;
    if reward == BARK && random::random(ctx, 100)? >= 10 {
        amount = 0;
    }
    if reward == 1511 && seers_complete(ctx)? && region_id(player_location(ctx, player)?) == 10806 {
        amount = 2;
    }
    Ok(amount)
}
fn maple_bonus(ctx: &mut (impl Context + ?Sized), reward: i32) -> Result<bool> {
    Ok(reward == 1517
        && seers_complete(ctx)?
        && ctx
            .optional_int(Service::EquipmentGet, &[SLOT_HAT.into()])?
            .is_some()
        && ctx.optional_int(Service::EquipmentGet, &[SLOT_HAT.into()])? == Some(14631))
}
fn replace_with_stump(
    ctx: &mut (impl Context + ?Sized),
    settings: &Settings,
    player: Handle,
    node: Handle,
    resource: &WoodcuttingNode,
) -> Result<()> {
    let empty = if resource.empty > -1 {
        resource.empty
    } else {
        0
    };
    let stump = ctx.handle(Service::SceneryTransform, &[node.into(), empty.into()])?;
    let duration = respawn_duration(ctx, settings, resource)?;
    ctx.boolean(
        Service::SceneryBuilderReplace,
        &[node.into(), stump.into(), duration.into()],
    )?;
    ctx.unit(Service::NodeSetActive, &[node.into(), false.into()])?;
    ctx.unit(Service::ApiPlayAudio, &[player.into(), TREE_FALLING.into()])
}
fn respawn_duration(
    ctx: &mut (impl Context + ?Sized),
    settings: &Settings,
    resource: &WoodcuttingNode,
) -> Result<i32> {
    let players = ctx.int(Service::RepositoryPlayersSize, &[])?;
    Ok(resource.respawn_duration(settings.max_players, players))
}
fn bird_nest(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<()> {
    if random::random(ctx, 282)? == 141 {
        if ctx.boolean(Service::SkillcapePerksIsActive, &["NEST_HUNTER".into()])? {
            let nest = match ctx.call(Service::BirdNestGetRandomNest, &[false.into()])? {
                Arg::List(v) if v.len() == 2 => v,
                _ => return Err(Error::Reply),
            };
            if !ctx.boolean(Service::InventoryAdd, &nest)? {
                ctx.unit(Service::BirdNestDrop, &[player.into()])?;
            }
        } else {
            ctx.unit(Service::BirdNestDrop, &[player.into()])?;
        }
    }
    Ok(())
}
fn count_log(ctx: &mut (impl Context + ?Sized)) -> Result<()> {
    let cut = ctx.int(Service::PlayerGetAttribute, &[LOGS_KEY.into(), 0.into()])?;
    ctx.unit(
        Service::PlayerSetAttribute,
        &[SAVED_LOGS_KEY.into(), cut.wrapping_add(1).into()],
    )
}
fn produced(
    ctx: &mut (impl Context + ?Sized),
    reward: i32,
    amount: i32,
    node: Handle,
) -> Result<()> {
    ctx.unit(
        Service::PlayerDispatch,
        &[reward.into(), amount.into(), node.into(), (-1).into()],
    )
}

// ---- WoodcuttingListener ------------------------------------------------

/// `handleWoodcutting(player, node, state)`.
pub fn handle_woodcutting(
    ctx: &mut (impl Context + ?Sized),
    settings: &Settings,
    player: Handle,
    node: Handle,
    state: i32,
) -> Result<bool> {
    let resource = WoodcuttingNode::for_id(node_id(ctx, node)?);
    let tool = tools::get_hatchet(ctx, player)?;
    if !ctx.boolean(Service::ApiFinishedMoving, &[])? {
        return ctx.boolean(Service::ApiRestartScript, &[]);
    }
    if state == 0 {
        // The public Kotlin function rejects a null resource before its body runs.
        let resource = resource.ok_or(Error::NullPointer)?;
        if !check_woodcutting_requirements(ctx, player, resource, node)? {
            return ctx.boolean(Service::ApiClearScripts, &[]);
        }
        api_message(ctx, player, "You swing your axe at the tree...")?;
    }
    if ctx.boolean(Service::ApiClockReady, &[SKILLING.into()])? {
        animate_woodcutting(ctx, player)?;
        if let Some(r) = resource.filter(|r| r.name == "DRAMEN_TREE" || r.name == "SWAYING_TREE") {
            // Reward after one chop and then abort chopping.
            ctx.defer(
                Service::ApiQueueScript,
                &[1.into(), "STRONG".into(), false.into()],
                Task::BranchReward {
                    player,
                    dramen: r.name == "DRAMEN_TREE",
                    reward: r.reward,
                },
            )?;
            return delay_clock(ctx, 1);
        }
        // Kotlin checks platform-typed arguments where they are passed, so a
        // null resource or tool fails here, before the roll reads anything.
        let (Some(resource), Some(tool)) = (resource, tool) else {
            return Err(Error::NullPointer);
        };
        if !check_reward(ctx, Some(resource), Some(tool))? && !instachop(ctx)? {
            return delay_clock(ctx, 3);
        }
        let reward = resource.reward;
        if tool.id == INFERNO_ADZE && reward != BARK && random::roll(ctx, 4)? {
            api_message(
                ctx,
                player,
                "You chop some logs. The heat of the inferno adze incinerates them.",
            )?;
            ctx.unit(
                Service::ProjectileCreate,
                &[
                    player.into(),
                    Arg::Null,
                    1776.into(),
                    35.into(),
                    30.into(),
                    20.into(),
                    25.into(),
                ],
            )?;
            let l = player_location(ctx, player)?;
            ctx.unit(
                Service::ProjectileTransform,
                &[
                    player.into(),
                    [l[0].wrapping_add(2), l[1], l[2]].into(),
                    true.into(),
                    25.into(),
                    25.into(),
                ],
            )?;
            ctx.unit(Service::ProjectileSend, &[])?;
            let amount = calculate_reward_amount(ctx, player, reward)?;
            let experience = calculate_experience(ctx, player, resource, amount)?;
            ctx.unit(
                Service::SkillsAddExperience3,
                &[WOODCUTTING.into(), experience.into(), true.into()],
            )?;
            if let Some(xp) = ctx.optional_double(Service::FiremakingLogForId, &[reward.into()])? {
                ctx.unit(
                    Service::SkillsAddExperience2,
                    &[FIREMAKING.into(), xp.into()],
                )?;
            }
            delay_clock(ctx, 3)?;
            return roll_depletion(ctx, settings, player, node, resource);
        }
        if reward > 0 {
            let amount = calculate_reward_amount(ctx, player, reward)?;
            let experience = calculate_experience(ctx, player, resource, amount)?;
            ctx.unit(
                Service::SkillsAddExperience3,
                &[WOODCUTTING.into(), experience.into(), true.into()],
            )?;
            if reward == BARK && amount == 0 {
                packet_message(
                    ctx,
                    "You chop away some bark, but it falls to pieces before you can pick it up."
                        .into(),
                )?;
            } else {
                let name = reward_name(ctx, reward)?;
                packet_message(ctx, format!("You get some {name}."))?;
            }
            ctx.boolean(Service::InventoryAdd, &[reward.into(), amount.into()])?;
            produced(ctx, reward, amount, node)?;
            count_log(ctx)?;
            bird_nest(ctx, player)?;
        }
        delay_clock(ctx, 3)?;
        roll_depletion(ctx, settings, player, node, resource)?;
        if !check_woodcutting_requirements(ctx, player, resource, node)? {
            return ctx.boolean(Service::ApiClearScripts, &[]);
        }
    }
    ctx.boolean(Service::ApiKeepRunning, &[])
}

/// `animateWoodcutting`: local players are read before the sound index is drawn.
pub fn animate_woodcutting(ctx: &mut (impl Context + ?Sized), player: Handle) -> Result<()> {
    if !ctx.boolean(Service::AnimatorIsAnimating, &[])? {
        let tool = tools::get_hatchet(ctx, player)?.ok_or(Error::NullPointer)?;
        ctx.unit(Service::PlayerAnimate, &[tool.animation.into(), Arg::Null])?;
        let location = player_location(ctx, player)?;
        let around = local_players(ctx, location)?;
        let index = random::random_range(ctx, 0, HIT_SOUNDS.len() as i32)?;
        for p in around.into_iter().filter(|&p| p != player) {
            ctx.unit(
                Service::ApiPlayAudio,
                &[p.into(), HIT_SOUNDS[index as usize].into()],
            )?;
        }
    }
    Ok(())
}
fn local_players(ctx: &mut (impl Context + ?Sized), location: [i32; 3]) -> Result<Vec<Handle>> {
    match ctx.call(Service::RegionLocalPlayers, &[location.into(), 2.into()])? {
        Arg::List(v) => v
            .into_iter()
            .map(|p| match p {
                Arg::Int(p) => Ok(p),
                _ => Err(Error::Reply),
            })
            .collect(),
        _ => Err(Error::Reply),
    }
}

/// `checkWoodcuttingRequirements(player, resource, node)`.
pub fn check_woodcutting_requirements(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    resource: &WoodcuttingNode,
    node: Handle,
) -> Result<bool> {
    let region = region_id(player_location(ctx, player)?);
    if region == 10300 || region == 10044 {
        // Miscellania, then Etceteria.
        let npc = if region == 10300 { 3916 } else { 1395 };
        let at = player_location(ctx, player)?;
        ctx.unit(
            Service::ApiOpenDialogue,
            &["KjallakOnChopDialogue".into(), npc.into(), at.into()],
        )?;
        return Ok(false);
    }
    if ctx.int(Service::SkillsGetLevel, &[WOODCUTTING.into()])? < resource.level {
        packet_message(
            ctx,
            format!(
                "You need a woodcutting level of {} to chop this tree.",
                resource.level
            ),
        )?;
        return Ok(false);
    }
    if tools::get_hatchet(ctx, player)?.is_none() {
        packet_message(ctx, "You do not have an axe to use.".into())?;
        return Ok(false);
    }
    if node_id(ctx, node)? == 1292
        && ctx.int(Service::ApiGetQuestStage, &["LOST_CITY".into()])? <= 20
    {
        return Ok(false);
    }
    if ctx.int(Service::InventoryFreeSlots, &[])? < 1 && node_active(ctx, node)? {
        let name = reward_name(ctx, resource.reward)?;
        ctx.unit(
            Service::PlayerSendMessage,
            &[format!("Your inventory is too full to hold any more {name}.").into()],
        )?;
        return Ok(false);
    }
    node_active(ctx, node)
}

/// The listener's `calculateExperience`: bark pays 82.5 only when received.
fn calculate_experience(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    resource: &WoodcuttingNode,
    amount: i32,
) -> Result<f64> {
    let mut experience = resource.experience;
    let reward = resource.reward;
    if region_id(player_location(ctx, player)?) == 10300 {
        return Ok(1.0);
    }
    if reward == BARK {
        if amount >= 1 {
            experience = 82.5;
        } else {
            return Ok(0.0);
        }
    }
    if maple_bonus(ctx, reward)? {
        experience *= 1.10;
    }
    Ok(experience * f64::from(amount))
}

/// The listener's `rollDepletion`: identifiers 1, 2, 3, 4 and 6 always fall,
/// and the farming branch never replaces the scenery.
pub fn roll_depletion(
    ctx: &mut (impl Context + ?Sized),
    settings: &Settings,
    player: Handle,
    node: Handle,
    resource: &WoodcuttingNode,
) -> Result<bool> {
    if resource.respawn_rate > 0
        && (random::roll(ctx, 8)?
            || [1, 2, 3, 4, 6].contains(&i32::from(resource.identifier))
            || instachop(ctx)?)
    {
        if resource.farming {
            if let Some(varbit) =
                ctx.optional_int(Service::FarmingPatchForObject, &[node.into()])?
            {
                ctx.unit(
                    Service::FarmingPatchGetPatchFor,
                    &[varbit.into(), true.into()],
                )?;
                let state = ctx.int(Service::PatchGetCurrentState, &[])?;
                ctx.unit(
                    Service::PatchSetCurrentState,
                    &[state.wrapping_add(1).into()],
                )?;
                ctx.unit(Service::NodeSetActive, &[node.into(), false.into()])?;
                ctx.unit(Service::ApiPlayAudio, &[player.into(), TREE_FALLING.into()])?;
                ctx.defer(
                    Service::ApiQueueScript,
                    &[2.into(), "SOFT".into(), false.into()],
                    Task::ReactivateNode { player, node },
                )?;
                ctx.unit(Service::ApiGetOrStartTimer, &["StumpGrowth".into()])?;
                let duration = respawn_duration(ctx, settings, resource)?;
                ctx.unit(
                    Service::StumpGrowthAddStump,
                    &[varbit.into(), duration.into()],
                )?;
            }
            return Ok(true);
        }
        replace_with_stump(ctx, settings, player, node, resource)?;
        return Ok(true);
    }
    Ok(false)
}

/// Runs a body the listener queued. `stage` is unused by both bodies.
pub fn run_task(ctx: &mut (impl Context + ?Sized), task: &Task, _stage: i32) -> Result<bool> {
    match *task {
        Task::BranchReward {
            player,
            dramen,
            reward,
        } => {
            let tree = if dramen {
                "Dramen"
            } else {
                "strangely musical"
            };
            api_message(
                ctx,
                player,
                &format!("You cut a branch from the {tree} tree."),
            )?;
            ctx.boolean(
                Service::ApiAddItem,
                &[reward.into(), 1.into(), "INVENTORY".into()],
            )?;
            ctx.boolean(Service::ApiClearScripts, &[])
        }
        Task::ReactivateNode { node, .. } => {
            ctx.unit(Service::NodeSetActive, &[node.into(), true.into()])?;
            ctx.boolean(Service::ApiStopExecuting, &[])
        }
        Task::ProspectMessage { .. } | Task::BarehandCatch { .. } => Err(Error::ClassCast),
    }
}

// ---- WoodcuttingSkillPulse ----------------------------------------------

/// The subclass state of a WoodcuttingSkillPulse. `running` is the base
/// Pulse flag its constructor, `start` and `stop` write; base `update`
/// scheduling stays with `server::pulses`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pulse {
    pub player: Handle,
    pub node: Handle,
    pub resource: Option<&'static WoodcuttingNode>,
    pub ticks: i32,
    pub reset_animation: bool,
    pub running: bool,
}
impl Pulse {
    /// The constructor: `super(1, player, node)` then `super.stop()`.
    pub fn new(player: Handle, node: Handle) -> Self {
        Self {
            player,
            node,
            resource: None,
            ticks: 0,
            reset_animation: true,
            running: false,
        }
    }
    pub const DELAY: i32 = 1;

    pub fn start(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        self.resource = WoodcuttingNode::for_id(node_id(ctx, self.node)?);
        if self.resource.is_none() {
            return Ok(());
        }
        if self.check_requirements(ctx)? {
            self.running = true;
            packet_message(ctx, "You swing your axe at the tree...".into())?;
        }
        Ok(())
    }
    pub fn pulse(
        &mut self,
        ctx: &mut (impl Context + ?Sized),
        settings: &Settings,
    ) -> Result<bool> {
        if !self.check_requirements(ctx)? {
            return Ok(true);
        }
        self.animate(ctx)?;
        self.reward(ctx, settings)
    }
    pub fn stop(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        if self.reset_animation {
            ctx.unit(Service::PlayerAnimate, &[(-1).into(), "HIGH".into()])?;
        }
        self.running = false;
        Ok(())
    }
    pub fn check_requirements(&self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        let level = ctx.int(Service::SkillsGetLevel, &[WOODCUTTING.into()])?;
        let resource = self.resource.ok_or(Error::NullPointer)?;
        if level < resource.level {
            packet_message(
                ctx,
                format!(
                    "You need a woodcutting level of {} to chop this tree.",
                    resource.level
                ),
            )?;
            return Ok(false);
        }
        if tools::get_hatchet(ctx, self.player)?.is_none() {
            packet_message(ctx, "You do not have an axe to use.".into())?;
            return Ok(false);
        }
        if ctx.int(Service::InventoryFreeSlots, &[])? < 1 {
            let name = reward_name(ctx, resource.reward)?;
            ctx.unit(
                Service::DialogueSendDialogue,
                &[Arg::List(vec![
                    format!("Your inventory is too full to hold any more {name}.").into(),
                ])],
            )?;
            return Ok(false);
        }
        Ok(true)
    }
    /// The pulse draws the sound index before it reads the local players.
    pub fn animate(&self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        if !ctx.boolean(Service::AnimatorIsAnimating, &[])? {
            let tool = tools::get_hatchet(ctx, self.player)?.ok_or(Error::NullPointer)?;
            ctx.unit(Service::PlayerAnimate, &[tool.animation.into(), Arg::Null])?;
            let index = random::random_range(ctx, 0, HIT_SOUNDS.len() as i32)?;
            let location = player_location(ctx, self.player)?;
            for p in local_players(ctx, location)? {
                if p == self.player {
                    continue;
                }
                ctx.unit(
                    Service::ApiPlayAudio,
                    &[p.into(), HIT_SOUNDS[index as usize].into()],
                )?;
            }
        }
        Ok(())
    }
    pub fn reward(
        &mut self,
        ctx: &mut (impl Context + ?Sized),
        settings: &Settings,
    ) -> Result<bool> {
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % 4 != 0 {
            return Ok(false);
        }
        if node_id(ctx, self.node)? == 10041 {
            let line = if random::random(ctx, 2)? == 1 {
                "You'll blow my cover! I'm meant to be hidden!"
            } else {
                "Will you stop that?"
            };
            ctx.unit(
                Service::DialogueSendDialogues,
                &[2574.into(), "FURIOUS".into(), Arg::List(vec![line.into()])],
            )?;
            return Ok(true);
        }
        let tool = tools::get_hatchet(ctx, self.player)?;
        if !check_reward(ctx, self.resource, tool)? {
            return Ok(false);
        }
        let resource = self.resource.ok_or(Error::NullPointer)?;
        // "20% chance" in the source comment; the code rolls below 25 of 100.
        let tool = tools::get_hatchet(ctx, self.player)?.ok_or(Error::NullPointer)?;
        if tool.id == INFERNO_ADZE && random::random(ctx, 100)? < 25 {
            ctx.unit(
                Service::PlayerSendMessage,
                &["You chop some logs. The heat of the inferno adze incinerates them.".into()],
            )?;
            ctx.unit(
                Service::ProjectileCreate,
                &[
                    self.player.into(),
                    Arg::Null,
                    1776.into(),
                    35.into(),
                    30.into(),
                    20.into(),
                    25.into(),
                ],
            )?;
            // new Location(x + 2, y): two location reads, plane zero.
            let x = player_location(ctx, self.player)?[0].wrapping_add(2);
            let y = player_location(ctx, self.player)?[1];
            ctx.unit(
                Service::ProjectileTransform,
                &[
                    self.player.into(),
                    [x, y, 0].into(),
                    true.into(),
                    25.into(),
                    25.into(),
                ],
            )?;
            ctx.unit(Service::ProjectileSend, &[])?;
            ctx.unit(
                Service::SkillsAddExperience2,
                &[WOODCUTTING.into(), resource.experience.into()],
            )?;
            ctx.unit(
                Service::SkillsAddExperience2,
                &[FIREMAKING.into(), resource.experience.into()],
            )?;
            return self.roll_depletion(ctx, settings);
        }
        let reward = resource.reward;
        if reward > 0 {
            let amount = calculate_reward_amount(ctx, self.player, reward)?;
            let experience = self.calculate_experience(ctx, resource, amount)?;
            ctx.unit(
                Service::SkillsAddExperience3,
                &[WOODCUTTING.into(), experience.into(), true.into()],
            )?;
            let name = reward_name(ctx, reward)?;
            packet_message(ctx, format!("You get some {name}."))?;
            ctx.boolean(Service::InventoryAdd, &[reward.into(), amount.into()])?;
            produced(ctx, reward, amount, self.node)?;
            count_log(ctx)?;
            bird_nest(ctx, self.player)?;
        }
        self.roll_depletion(ctx, settings)
    }
    /// The pulse's `calculateExperience`: bark pays 275.2 when received and the
    /// base amount once when it is not.
    fn calculate_experience(
        &self,
        ctx: &mut (impl Context + ?Sized),
        resource: &WoodcuttingNode,
        mut amount: i32,
    ) -> Result<f64> {
        let mut experience = resource.experience;
        let reward = resource.reward;
        if region_id(player_location(ctx, self.player)?) == 10300 {
            return Ok(1.0);
        }
        if reward == BARK {
            if amount >= 1 {
                experience = 275.2;
            } else {
                amount = 1;
            }
        }
        if maple_bonus(ctx, reward)? {
            experience *= 1.10;
        }
        Ok(experience * f64::from(amount))
    }
    /// The pulse's `rollDepletion`: identifiers 1, 2, 3 and 6 always fall
    /// (4 does not), with no instachop attribute.
    fn roll_depletion(
        &self,
        ctx: &mut (impl Context + ?Sized),
        settings: &Settings,
    ) -> Result<bool> {
        let resource = self.resource.ok_or(Error::NullPointer)?;
        if resource.respawn_rate > 0
            && (random::roll(ctx, 8)? || [1, 2, 3, 6].contains(&i32::from(resource.identifier)))
        {
            if resource.farming {
                if let Some(varbit) =
                    ctx.optional_int(Service::FarmingPatchForObject, &[self.node.into()])?
                {
                    ctx.unit(
                        Service::FarmingPatchGetPatchFor,
                        &[varbit.into(), true.into()],
                    )?;
                    let state = ctx.int(Service::PatchGetCurrentState, &[])?;
                    ctx.unit(
                        Service::PatchSetCurrentState,
                        &[state.wrapping_add(1).into()],
                    )?;
                }
                return Ok(true);
            }
            replace_with_stump(ctx, settings, self.player, self.node, resource)?;
            return Ok(true);
        }
        Ok(false)
    }
}
