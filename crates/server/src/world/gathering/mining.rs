//! MiningListener, the legacy MiningSkillPulse and ProspectListener,
//! statement for statement. The listener and the pulse differ in messages,
//! Varrock-armour thresholds, the essence shortcut and where the pickaxe is
//! re-read; each keeps its own source's behaviour.
use super::{
    Settings,
    nodes::{GEM_ROCK_GEMS, MiningNode},
    random,
    service::{Arg, Calls, Context, Error, Handle, Result, Service, Task},
    tools::{self, MINING, Tool},
};
use rs09_game_core::java_lower::lowercase_root;

const CLAY: i32 = 434;
const SOFT_CLAY: i32 = 1761;
const BRACELET_OF_CLAY: i32 = 11074;
const GOLD_ORE: i32 = 444;
const PERFECT_GOLD_ORE: i32 = 446;
const RING_OF_THE_STAR_SPRITE: i32 = 14652;
const BRACELET_KEY: &str = "jewellery-charges:bracelet-of-clay";
const SAVED_BRACELET_KEY: &str = "/save:jewellery-charges:bracelet-of-clay";
const ROCKS_KEY: &str = "stats_manager:rocks_mined";
const SAVED_ROCKS_KEY: &str = "/save:stats_manager:rocks_mined";
/// `GEM_REWARDS`: sapphire, emerald, ruby, diamond.
pub const GEM_REWARDS: [i32; 4] = [1623, 1621, 1619, 1617];
/// Perfect gold rocks in the Witchaven dungeon (MiningSkillPulse only).
pub const PERFECT_GOLD_ORE_LOCATIONS: [[i32; 3]; 4] = [
    [2735, 9695, 0],
    [2737, 9689, 0],
    [2740, 9684, 0],
    [2737, 9683, 0],
];

/// `MiningNode.gemRockGems`: static, shuffled in place by every gem-rock roll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub gem_rock_gems: Vec<(i32, i32, i32, i32)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            gem_rock_gems: GEM_ROCK_GEMS.to_vec(),
        }
    }
}

/// What `MiningListener.defineListeners` registers.
pub fn define_listeners(ctx: &mut (impl Context + ?Sized)) -> Result<()> {
    ctx.unit(
        Service::ListenersDefineInteraction,
        &[
            "SCENERY".into(),
            Arg::List(MiningNode::values().iter().map(|n| n.full.into()).collect()),
            Arg::List(vec!["mine".into()]),
            true.into(),
            1.into(),
        ],
    )
}
/// What `ProspectListener.defineListeners` registers.
pub fn define_prospect_listeners(ctx: &mut (impl Context + ?Sized)) -> Result<()> {
    ctx.unit(
        Service::ListenersOn,
        &["SCENERY".into(), Arg::List(vec!["prospect".into()])],
    )
}

fn node_id(ctx: &mut (impl Context + ?Sized), node: Handle) -> Result<i32> {
    ctx.int(Service::NodeId, &[node.into()])
}
fn message(ctx: &mut (impl Context + ?Sized), player: Handle, text: &str) -> Result<()> {
    ctx.unit(Service::ApiSendMessage, &[player.into(), text.into()])
}
fn dialogue(ctx: &mut (impl Context + ?Sized), text: &str) -> Result<()> {
    ctx.unit(Service::ApiSendDialogue, &[text.into()])
}
fn dyn_level(ctx: &mut (impl Context + ?Sized)) -> Result<i32> {
    ctx.int(Service::ApiGetDynLevel, &[MINING.into()])
}
fn free_slots(ctx: &mut (impl Context + ?Sized)) -> Result<i32> {
    ctx.int(Service::ApiFreeSlots, &[])
}
fn reward_xp(ctx: &mut (impl Context + ?Sized), amount: f64) -> Result<()> {
    ctx.unit(Service::ApiRewardXp, &[MINING.into(), amount.into()])
}
fn item_name(ctx: &mut (impl Context + ?Sized), id: i32) -> Result<String> {
    ctx.text(Service::ApiGetItemName, &[id.into()])
}
fn equipped(ctx: &mut (impl Context + ?Sized), slot: &str) -> Result<Option<i32>> {
    ctx.optional_int(Service::ApiGetItemFromEquipment, &[slot.into()])
}
fn is_essence(resource: &MiningNode) -> bool {
    resource.name == "RUNE_ESSENCE"
}
fn gem_identifier() -> i8 {
    MiningNode::by_name("GEM_ROCK_0").map_or(13, |n| n.identifier)
}
/// `core.tools.prependArticle`. An empty noun indexes out of bounds, as in the original.
pub fn prepend_article(noun: &str) -> Result<String> {
    let lower = lowercase_root(noun);
    let exception = match lower.as_str() {
        "unicorn" => Some("a"),
        "herb" | "hour" => Some("an"),
        _ => None,
    };
    if let Some(article) = exception {
        return Ok(format!("{article} {noun}"));
    }
    match noun.chars().next().ok_or(Error::IndexOutOfBounds)? {
        'a' | 'e' | 'i' | 'o' | 'u' => Ok(format!("an {noun}")),
        _ => Ok(format!("a {noun}")),
    }
}
fn anim(ctx: &mut (impl Context + ?Sized), tool: Tool) -> Result<()> {
    if ctx.boolean(Service::ApiAnimationFinished, &[])? {
        ctx.unit(
            Service::ApiAnimate,
            &[tool.animation.into(), Arg::Null, false.into()],
        )?;
    }
    Ok(())
}
/// `getDelay`: only essence varies, and the mining adze draws for it.
pub fn get_delay(
    ctx: &mut (impl Context + ?Sized),
    resource: &MiningNode,
    tool: Tool,
) -> Result<i32> {
    if is_essence(resource) {
        return Ok(match tool.name {
            "BRONZE_PICKAXE" => 7,
            "IRON_PICKAXE" => 6,
            "STEEL_PICKAXE" => 5,
            "MITHRIL_PICKAXE" => 4,
            "ADAMANT_PICKAXE" => 3,
            "RUNE_PICKAXE" => 2,
            "INFERNO_ADZE2" => {
                if random::random(ctx, 2)? == 0 {
                    1
                } else {
                    2
                }
            }
            _ => 4,
        });
    }
    Ok(4)
}
fn delay_script(
    ctx: &mut (impl Context + ?Sized),
    resource: &MiningNode,
    tool: Tool,
) -> Result<bool> {
    let delay = get_delay(ctx, resource, tool)?;
    ctx.boolean(Service::ApiDelayScript, &[delay.into()])
}
fn varrock_roll(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    reward: i32,
    pass: impl Fn(i32, i32) -> bool,
) -> Result<i32> {
    let tier = match reward {
        434 | 436 | 438 | 3211 | 668 | 440 | 2892 | 442 | 453 => 0,
        444 | 6979 | 6981 | 6983 | 447 => 1,
        449 => 2,
        _ => return Ok(0),
    };
    if ctx.int(Service::DiaryGetArmour, &[])? >= tier && pass(tier, random::random(ctx, 100)?) {
        message(
            ctx,
            player,
            "The Varrock armour allows you to mine an additional ore.",
        )?;
        return Ok(1);
    }
    Ok(0)
}
/// `calculateRewardAmount`. `pass(tier, draw)` is the source's own threshold.
fn calculate_reward_amount(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    essence: bool,
    reward: i32,
    pass: impl Fn(i32, i32) -> bool,
) -> Result<i32> {
    let mut amount = 1i32;
    if !essence && ctx.int(Service::DiaryGetLevel, &["VARROCK".into()])? != -1 {
        amount += varrock_roll(ctx, player, reward, pass)?;
    }
    if ctx.boolean(Service::ApiHasTimerActive, &["StarBonus".into()])?
        && random::get_random(ctx, 5)? == 3
    {
        message(
            ctx,
            player,
            "...you manage to mine a second ore thanks to the Star Sprite.",
        )?;
        amount += 1;
    }
    Ok(amount)
}
fn calculate_reward(
    ctx: &mut (impl Context + ?Sized),
    state: &mut State,
    resource: &MiningNode,
    essence: bool,
    gems: bool,
    mut reward: i32,
) -> Result<i32> {
    if resource.name == "SANDSTONE" || resource.name == "GRANITE" {
        let value = random::randomize(ctx, if resource.name == "GRANITE" { 3 } else { 4 })?;
        reward = reward.wrapping_add(value << 1);
        reward_xp(ctx, f64::from(value) * 10.0)?;
    } else if essence && dyn_level(ctx)? >= 30 {
        reward = 7936;
    } else if gems {
        reward = random::roll_weighted_chance_table(ctx, &mut state.gem_rock_gems)?
            .ok_or(Error::NullPointer)?
            .0;
    }
    Ok(reward)
}
/// The bracelet of clay: softens the clay and spends a charge.
fn soften(ctx: &mut (impl Context + ?Sized), player: Handle, reward: &mut i32) -> Result<()> {
    if *reward == CLAY && equipped(ctx, "HANDS")? == Some(BRACELET_OF_CLAY) {
        let mut charges = ctx
            .int(
                Service::PlayerGetAttribute,
                &[BRACELET_KEY.into(), 28.into()],
            )?
            .wrapping_sub(1);
        *reward = SOFT_CLAY;
        message(
            ctx,
            player,
            "Your bracelet of clay softens the clay for you.",
        )?;
        if charges <= 0
            && ctx.boolean(
                Service::ApiRemoveItem,
                &[BRACELET_OF_CLAY.into(), "EQUIPMENT".into()],
            )?
        {
            message(ctx, player, "Your bracelet of clay crumbles to dust.")?;
            charges = 28;
        }
        ctx.unit(
            Service::PlayerSetAttribute,
            &[SAVED_BRACELET_KEY.into(), charges.into()],
        )?;
    }
    Ok(())
}
fn count_rocks(ctx: &mut (impl Context + ?Sized), amount: i32) -> Result<()> {
    let mined = ctx.int(Service::ApiGetAttribute, &[ROCKS_KEY.into(), 0.into()])?;
    ctx.unit(
        Service::ApiSetAttribute,
        &[SAVED_ROCKS_KEY.into(), mined.wrapping_add(amount).into()],
    )
}
/// The bonus gem: ring and necklace each divide the 1/282 chance by 1.5.
fn bonus_gem(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    found: impl Fn(&str) -> Result<String>,
) -> Result<()> {
    let mut chance = 282i32;
    let ring = equipped(ctx, "RING")?;
    let wealthy = match ring {
        Some(id) => {
            lowercase_root(&ctx.text(Service::ItemName, &[id.into()])?).contains("ring of wealth")
        }
        None => false,
    };
    if wealthy
        || ctx.boolean(
            Service::ApiInEquipment,
            &[RING_OF_THE_STAR_SPRITE.into(), 1.into()],
        )?
    {
        chance = (f64::from(chance) / 1.5) as i32;
    }
    if equipped(ctx, "NECK")?.is_some_and(|id| (1705..=1713).contains(&id)) {
        chance = (f64::from(chance) / 1.5) as i32;
    }
    if random::roll(ctx, chance)? {
        // Kotlin's Array.random() draws from the platform default generator.
        let pick = ctx.int(
            Service::RngThreadLocalNextInt,
            &[(GEM_REWARDS.len() as i32).into()],
        )?;
        let gem = *GEM_REWARDS
            .get(usize::try_from(pick).map_err(|_| Error::Reply)?)
            .ok_or(Error::Reply)?;
        let name = ctx.text(Service::ItemName, &[gem.into()])?;
        message(ctx, player, &found(&name)?)?;
        if free_slots(ctx)? == 0 {
            message(
                ctx,
                player,
                "You do not have enough space in your inventory, so you drop the gem on the floor.",
            )?;
        }
        ctx.unit(Service::ApiAddItemOrDrop, &[gem.into(), 1.into()])?;
    }
    Ok(())
}
/// Replaces the rock with its depleted object for the respawn duration.
fn deplete(
    ctx: &mut (impl Context + ?Sized),
    settings: &Settings,
    node: Handle,
    resource: &MiningNode,
) -> Result<()> {
    let location = ctx.location(Service::NodeLocation, &[node.into()])?;
    let kind = ctx.int(Service::SceneryType, &[node.into()])?;
    let rotation = ctx.int(Service::SceneryRotation, &[node.into()])?;
    let empty = ctx.handle(
        Service::SceneryNew,
        &[
            resource.empty.into(),
            location.into(),
            kind.into(),
            rotation.into(),
        ],
    )?;
    let players = ctx.int(Service::RepositoryPlayersSize, &[])?;
    let duration = resource.respawn_duration(settings.max_players, players);
    ctx.boolean(
        Service::SceneryBuilderReplace,
        &[node.into(), empty.into(), duration.into()],
    )?;
    ctx.unit(Service::NodeSetActive, &[node.into(), false.into()])
}

// ---- MiningListener -------------------------------------------------------

/// `handleMining(player, node, state)`.
pub fn handle_mining(
    ctx: &mut (impl Context + ?Sized),
    settings: &Settings,
    world: &mut State,
    player: Handle,
    node: Handle,
    state: i32,
) -> Result<bool> {
    let resource = MiningNode::for_id(node_id(ctx, node)?);
    let tool = tools::get_pickaxe(ctx, player)?;
    let resource = resource.ok_or(Error::NullPointer)?;
    let essence = is_essence(resource);
    let gems = resource.identifier == gem_identifier();
    if !ctx.boolean(Service::ApiFinishedMoving, &[])? {
        return Ok(true);
    }
    if state == 0 {
        if !check_requirements(ctx, player, resource, node)? {
            ctx.unit(Service::ScriptsReset, &[])?;
            return Ok(true);
        }
        let tool = tool.ok_or(Error::NullPointer)?;
        anim(ctx, tool)?;
        message(ctx, player, "You swing your pickaxe at the rock.")?;
        return delay_script(ctx, resource, tool);
    }
    let tool = tool.ok_or(Error::NullPointer)?;
    anim(ctx, tool)?;
    if !check_reward(ctx, resource, tool)? {
        return delay_script(ctx, resource, tool);
    }
    let mut reward = resource.reward;
    if reward > 0 {
        reward = calculate_reward(ctx, world, resource, essence, gems, reward)?;
        let amount = calculate_reward_amount(ctx, player, essence, reward, |tier, draw| {
            draw < [4, 3, 2][tier as usize]
        })?;
        ctx.unit(
            Service::PlayerDispatch,
            &[reward.into(), amount.into(), node.into(), (-1).into()],
        )?;
        reward_xp(ctx, resource.experience * f64::from(amount))?;
        soften(ctx, player, &mut reward)?;
        // Family Crest: gold mined inside the Witchaven dungeon area is perfect gold.
        if reward == GOLD_ORE
            && ctx.boolean(
                Service::ApiInBorders,
                &[2733.into(), 9695.into(), 2741.into(), 9683.into()],
            )?
        {
            reward = PERFECT_GOLD_ORE;
        }
        let name = item_name(ctx, reward)?;
        if gems {
            let name = lowercase_root(&name.replace("Uncut ", ""));
            let with_article = prepend_article(&name)?;
            let (article, gem) = with_article
                .split_once(' ')
                .ok_or(Error::IndexOutOfBounds)?;
            let mut letters = gem.chars();
            let gem = match letters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
                None => String::new(),
            };
            message(ctx, player, &format!("You just mined {article} {gem}!"))?;
        } else if !essence {
            let name = lowercase_root(name.split(" (").next().unwrap_or(""));
            let name = name.strip_suffix(" ore").unwrap_or(&name);
            message(ctx, player, &format!("You manage to mine some {name}."))?;
        }
        ctx.unit(Service::ApiAddItemOrDrop, &[reward.into(), amount.into()])?;
        count_rocks(ctx, amount)?;
        if !essence {
            bonus_gem(ctx, player, |name| {
                Ok(format!(
                    "You just found {}!",
                    prepend_article(&lowercase_root(&name.replace("Uncut ", "")))?
                ))
            })?;
        }
        if !essence && resource.respawn_rate != 0 {
            deplete(ctx, settings, node, resource)?;
            return Ok(true);
        }
        if essence {
            if free_slots(ctx)? == 0 {
                return Ok(true);
            }
            return delay_script(ctx, resource, tool);
        }
    }
    Ok(true)
}

/// The listener's `checkReward`: essence always succeeds; otherwise two
/// `Math.random()` draws with the perk query between them.
fn check_reward(
    ctx: &mut (impl Context + ?Sized),
    resource: &MiningNode,
    tool: Tool,
) -> Result<bool> {
    if is_essence(resource) {
        return Ok(true);
    }
    let level = level(ctx)?;
    let host_ratio = ctx.double(Service::RngMathNextDouble, &[])? * (100.0 * resource.rate);
    let mut tool_ratio = tool.ratio;
    if ctx.boolean(Service::SkillcapePerksIsActive, &["PRECISION_MINER".into()])? {
        tool_ratio += 0.075;
    }
    let client_ratio = ctx.double(Service::RngMathNextDouble, &[])?
        * (f64::from(level.wrapping_sub(resource.level)) * (1.0 + tool_ratio));
    Ok(host_ratio < client_ratio)
}
fn level(ctx: &mut (impl Context + ?Sized)) -> Result<i32> {
    Ok(1i32
        .wrapping_add(dyn_level(ctx)?)
        .wrapping_add(ctx.int(Service::ApiGetFamiliarBoost, &[MINING.into()])?))
}

/// The listener's `checkRequirements(player, resource, node)`.
pub fn check_requirements(
    ctx: &mut (impl Context + ?Sized),
    player: Handle,
    resource: &MiningNode,
    node: Handle,
) -> Result<bool> {
    if dyn_level(ctx)? < resource.level {
        message(
            ctx,
            player,
            &format!(
                "You need a mining level of {} to mine this rock.",
                resource.level
            ),
        )?;
        return Ok(false);
    }
    if tools::get_pickaxe(ctx, player)?.is_none() {
        message(ctx, player, "You do not have a pickaxe to use.")?;
        return Ok(false);
    }
    if free_slots(ctx)? == 0 {
        if resource.identifier == 13 {
            dialogue(ctx, "Your inventory is too full to hold any more gems.")?;
            return Ok(false);
        }
        if is_essence(resource) {
            dialogue(ctx, "Your inventory is too full to hold any more essence.")?;
            return Ok(false);
        }
        full_inventory(ctx, resource)?;
        return Ok(false);
    }
    ctx.boolean(Service::NodeIsActive, &[node.into()])
}
fn full_inventory(ctx: &mut (impl Context + ?Sized), resource: &MiningNode) -> Result<()> {
    let name = lowercase_root(&ctx.text(Service::ItemDefinitionName, &[resource.reward.into()])?);
    dialogue(
        ctx,
        &format!("Your inventory is too full to hold any more {name}."),
    )
}

// ---- ProspectListener -----------------------------------------------------

/// The `prospect` option body.
pub fn prospect(ctx: &mut (impl Context + ?Sized), player: Handle, node: Handle) -> Result<bool> {
    let Some(rock) = MiningNode::for_id(node_id(ctx, node)?) else {
        message(
            ctx,
            player,
            "There is no ore currently available in this rock.",
        )?;
        return Ok(true);
    };
    let again = MiningNode::for_id(node_id(ctx, node)?).ok_or(Error::NullPointer)?;
    let reward = if again.identifier == 13 {
        None
    } else {
        Some(rock.reward)
    };
    message(ctx, player, "You examine the rock for ores...")?;
    ctx.defer(
        Service::PulseManagerRun,
        &[3.into()],
        Task::ProspectMessage { player, reward },
    )?;
    Ok(true)
}
/// Runs the delayed prospect message; always finishes.
pub fn run_task(ctx: &mut (impl Context + ?Sized), task: &Task) -> Result<bool> {
    match *task {
        Task::ProspectMessage { player, reward } => {
            match reward {
                None => message(ctx, player, "This rock contains gems.")?,
                Some(id) => {
                    let name = lowercase_root(&ctx.text(Service::ItemName, &[id.into()])?);
                    message(ctx, player, &format!("This rock contains {name}."))?;
                }
            }
            Ok(true)
        }
        _ => Err(Error::ClassCast),
    }
}

// ---- MiningSkillPulse -----------------------------------------------------

/// The subclass state of a MiningSkillPulse; `running` is the base Pulse
/// flag. Base `update` scheduling stays with `server::pulses`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pulse {
    pub player: Handle,
    pub node: Handle,
    pub resource: Option<&'static MiningNode>,
    pub is_mining_essence: bool,
    pub is_mining_gems: bool,
    pub ticks: i32,
    pub reset_animation: bool,
    pub running: bool,
}
impl Pulse {
    pub const DELAY: i32 = 1;
    /// The constructor: `Pulse(1, player, node)` then `super.stop()`.
    pub fn new(player: Handle, node: Handle) -> Self {
        Self {
            player,
            node,
            resource: None,
            is_mining_essence: false,
            is_mining_gems: false,
            ticks: 0,
            reset_animation: true,
            running: false,
        }
    }
    pub fn start(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        self.resource = MiningNode::for_id(node_id(ctx, self.node)?);
        if MiningNode::is_empty(node_id(ctx, self.node)?) {
            message(ctx, self.player, "This rock contains no ore.")?;
        }
        let Some(mut resource) = self.resource else {
            return Ok(());
        };
        if resource.full == 2099 {
            let at = ctx.location(Service::NodeLocation, &[self.node.into()])?;
            if !PERFECT_GOLD_ORE_LOCATIONS.contains(&at) {
                // Perfect gold ids outside Witchaven become a normal gold rock.
                self.resource = MiningNode::for_id(2098);
                resource = self.resource.ok_or(Error::NullPointer)?;
            }
        }
        if resource.full == 2491 {
            self.is_mining_essence = true;
        }
        if resource.identifier == gem_identifier() {
            self.is_mining_gems = true;
        }
        if self.check_requirements(ctx)? {
            self.running = true;
            message(ctx, self.player, "You swing your pickaxe at the rock...")?;
        }
        Ok(())
    }
    pub fn pulse(
        &mut self,
        ctx: &mut (impl Context + ?Sized),
        settings: &Settings,
        world: &mut State,
    ) -> Result<bool> {
        if !self.check_requirements(ctx)? {
            return Ok(true);
        }
        let tool = tools::get_pickaxe(ctx, self.player)?.ok_or(Error::NullPointer)?;
        ctx.unit(
            Service::ApiAnimate,
            &[tool.animation.into(), Arg::Null, false.into()],
        )?;
        self.reward(ctx, settings, world)
    }
    pub fn stop(&mut self, ctx: &mut (impl Context + ?Sized)) -> Result<()> {
        if self.reset_animation {
            ctx.unit(
                Service::ApiAnimate,
                &[(-1).into(), "HIGH".into(), false.into()],
            )?;
        }
        self.running = false;
        Ok(())
    }
    pub fn check_requirements(&self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        let level = dyn_level(ctx)?;
        let resource = self.resource.ok_or(Error::NullPointer)?;
        if level < resource.level {
            message(
                ctx,
                self.player,
                &format!(
                    "You need a mining level of {} to mine this rock.",
                    resource.level
                ),
            )?;
            return Ok(false);
        }
        if tools::get_pickaxe(ctx, self.player)?.is_none() {
            message(ctx, self.player, "You do not have a pickaxe to use.")?;
            return Ok(false);
        }
        if free_slots(ctx)? == 0 {
            if resource.identifier == 13 {
                dialogue(ctx, "Your inventory is too full to hold any more gems.")?;
                return Ok(false);
            }
            full_inventory(ctx, resource)?;
            return Ok(false);
        }
        Ok(true)
    }
    pub fn reward(
        &mut self,
        ctx: &mut (impl Context + ?Sized),
        settings: &Settings,
        world: &mut State,
    ) -> Result<bool> {
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % if self.is_mining_essence { 3 } else { 4 } != 0 {
            return Ok(false);
        }
        if !self.check_reward(ctx)? {
            return Ok(false);
        }
        let resource = self.resource.ok_or(Error::NullPointer)?;
        let mut reward = resource.reward;
        if reward > 0 {
            reward = calculate_reward(
                ctx,
                world,
                resource,
                self.is_mining_essence,
                self.is_mining_gems,
                reward,
            )?;
            let amount = calculate_reward_amount(
                ctx,
                self.player,
                self.is_mining_essence,
                reward,
                |_, draw| draw <= 10,
            )?;
            ctx.unit(
                Service::PlayerDispatch,
                &[reward.into(), amount.into(), self.node.into(), (-1).into()],
            )?;
            reward_xp(ctx, resource.experience * f64::from(amount))?;
            soften(ctx, self.player, &mut reward)?;
            let name = lowercase_root(&item_name(ctx, reward)?);
            if self.is_mining_gems {
                message(
                    ctx,
                    self.player,
                    &format!("You get {}.", prepend_article(&name)?),
                )?;
            } else {
                message(
                    ctx,
                    self.player,
                    &format!("You get some {}.", lowercase_root(&name)),
                )?;
            }
            ctx.unit(Service::ApiAddItemOrDrop, &[reward.into(), amount.into()])?;
            count_rocks(ctx, amount)?;
            if !self.is_mining_essence {
                bonus_gem(ctx, self.player, |name| Ok(format!("You find a {name}!")))?;
            }
            if !self.is_mining_essence && resource.respawn_rate != 0 {
                deplete(ctx, settings, self.node, resource)?;
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// The pulse's `checkReward`: no essence shortcut, and the pickaxe is
    /// re-read between the two `Math.random()` draws.
    fn check_reward(&self, ctx: &mut (impl Context + ?Sized)) -> Result<bool> {
        let level = level(ctx)?;
        let resource = self.resource.ok_or(Error::NullPointer)?;
        let host_ratio = ctx.double(Service::RngMathNextDouble, &[])? * (100.0 * resource.rate);
        let mut tool_ratio = tools::get_pickaxe(ctx, self.player)?
            .ok_or(Error::NullPointer)?
            .ratio;
        if ctx.boolean(Service::SkillcapePerksIsActive, &["PRECISION_MINER".into()])? {
            tool_ratio += 0.075;
        }
        let client_ratio = ctx.double(Service::RngMathNextDouble, &[])?
            * (f64::from(level.wrapping_sub(resource.level)) * (1.0 + tool_ratio));
        Ok(host_ratio < client_ratio)
    }
}
