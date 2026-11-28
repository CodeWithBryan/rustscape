//! Required services of the gathering ports.
//!
//! Every call the original listeners and pulses make into a system outside
//! this slice is one [`Service`] request, issued in the original order with
//! the original arguments. Nothing here performs or defaults a service: an
//! owner must answer each request, and a failure propagates with the completed
//! prefix left in place. Service names are diagnostics for recording and
//! comparison; they never select gameplay.

/// An argument or reply value. Doubles keep their exact bits.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    Text(String),
    List(Vec<Arg>),
}
impl From<i32> for Arg {
    fn from(v: i32) -> Self {
        Arg::Int(i64::from(v))
    }
}
impl From<i64> for Arg {
    fn from(v: i64) -> Self {
        Arg::Int(v)
    }
}
impl From<bool> for Arg {
    fn from(v: bool) -> Self {
        Arg::Bool(v)
    }
}
impl From<f64> for Arg {
    fn from(v: f64) -> Self {
        Arg::Double(v)
    }
}
impl From<&str> for Arg {
    fn from(v: &str) -> Self {
        Arg::Text(v.into())
    }
}
impl From<String> for Arg {
    fn from(v: String) -> Self {
        Arg::Text(v)
    }
}
impl From<[i32; 3]> for Arg {
    fn from(v: [i32; 3]) -> Self {
        Arg::List(v.iter().map(|&x| x.into()).collect())
    }
}
impl From<Option<&str>> for Arg {
    fn from(v: Option<&str>) -> Self {
        v.map_or(Arg::Null, Into::into)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The owner's service failed; earlier effects remain.
    Service,
    /// The owner answered with a value of the wrong shape.
    Reply,
    /// The original dereferences null here (unknown node or no usable tool).
    NullPointer,
    /// The original indexes outside an array here.
    IndexOutOfBounds,
    /// The original cast fails here.
    ClassCast,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// Opaque owner token for a player or node. The ports never interpret it.
pub type Handle = i64;

macro_rules! services {
    ($($variant:ident => $name:literal,)*) => {
        /// One request per distinct original call site target.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Service { $($variant,)* }
        impl Service {
            pub const ALL: &'static [Service] = &[$(Service::$variant,)*];
            pub fn name(self) -> &'static str {
                match self { $(Service::$variant => $name,)* }
            }
        }
    };
}
services! {
    // Target node and scenery objects.
    NodeId => "node.id",
    NodeIsActive => "node.isActive",
    NodeSetActive => "node.setActive",
    NodeLocation => "node.location",
    SceneryType => "scenery.type",
    SceneryRotation => "scenery.rotation",
    SceneryNew => "scenery.new",
    SceneryTransform => "scenery.transform",
    SceneryBuilderReplace => "sceneryBuilder.replace",
    // Player state.
    PlayerLocation => "player.location",
    PlayerAnimate => "player.animate",
    PlayerSendMessage => "player.sendMessage",
    PlayerDispatch => "player.dispatch",
    PlayerGetAttribute => "player.getAttribute",
    PlayerSetAttribute => "player.setAttribute",
    PacketDispatchSendMessage => "packetDispatch.sendMessage",
    AnimatorIsAnimating => "animator.isAnimating",
    SkillsGetLevel => "skills.getLevel",
    SkillsGetStaticLevel => "skills.getStaticLevel",
    SkillsAddExperience3 => "skills.addExperience3",
    SkillsAddExperience2 => "skills.addExperience2",
    InventoryFreeSlots => "inventory.freeSlots",
    InventoryContains => "inventory.contains",
    InventoryAdd => "inventory.add",
    EquipmentGetNew => "equipment.getNew",
    EquipmentGet => "equipment.get",
    FamiliarGetBoost => "familiar.getBoost",
    DiaryIsComplete => "diary.isComplete",
    DiaryGetLevel => "diary.getLevel",
    DiaryGetArmour => "diary.getArmour",
    DialogueSendDialogue => "dialogue.sendDialogue",
    DialogueSendDialogues => "dialogue.sendDialogues",
    ScriptsReset => "scripts.reset",
    // Definitions and content outside the slice.
    ItemDefinitionName => "itemDefinition.name",
    ItemName => "item.name",
    BirdNestGetRandomNest => "birdNest.getRandomNest",
    BirdNestDrop => "birdNest.drop",
    FiremakingLogForId => "firemakingLog.forId",
    FarmingPatchForObject => "farmingPatch.forObject",
    FarmingPatchGetPatchFor => "farmingPatch.getPatchFor",
    PatchGetCurrentState => "patch.getCurrentState",
    PatchSetCurrentState => "patch.setCurrentState",
    StumpGrowthAddStump => "stumpGrowth.addStump",
    SkillcapePerksIsActive => "skillcapePerks.isActive",
    ProjectileCreate => "projectile.create",
    ProjectileTransform => "projectile.transform",
    ProjectileSend => "projectile.send",
    RegionLocalPlayers => "region.localPlayers",
    RepositoryPlayersSize => "repository.players.size",
    // ContentAPI functions.
    ApiFinishedMoving => "api.finishedMoving",
    ApiAnimationFinished => "api.animationFinished",
    ApiClockReady => "api.clockReady",
    ApiDelayClock => "api.delayClock",
    ApiDelayScript => "api.delayScript",
    ApiKeepRunning => "api.keepRunning",
    ApiClearScripts => "api.clearScripts",
    ApiRestartScript => "api.restartScript",
    ApiStopExecuting => "api.stopExecuting",
    ApiSendMessage => "api.sendMessage",
    ApiSendDialogue => "api.sendDialogue",
    ApiAddItem => "api.addItem",
    ApiAddItemOrDrop => "api.addItemOrDrop",
    ApiFreeSlots => "api.freeSlots",
    ApiGetAttribute => "api.getAttribute",
    ApiSetAttribute => "api.setAttribute",
    ApiPlayAudio => "api.playAudio",
    ApiOpenDialogue => "api.openDialogue",
    ApiGetQuestStage => "api.getQuestStage",
    ApiGetOrStartTimer => "api.getOrStartTimer",
    ApiHasTimerActive => "api.hasTimerActive",
    ApiGetDynLevel => "api.getDynLevel",
    ApiGetFamiliarBoost => "api.getFamiliarBoost",
    ApiRewardXp => "api.rewardXP",
    ApiGetItemName => "api.getItemName",
    ApiGetItemFromEquipment => "api.getItemFromEquipment",
    ApiInEquipment => "api.inEquipment",
    ApiRemoveItem => "api.removeItem",
    ApiInBorders => "api.inBorders",
    ApiAnimate => "api.animate",
    // Registration and scheduling.
    ListenersDefineInteraction => "listeners.defineInteraction",
    ListenersOn => "listeners.on",
    ApiQueueScript => "api.queueScript",
    PulseManagerRun => "pulseManager.run",
    // The server's generators. `shared` is RandomFunction.RANDOM, `math` is
    // Math.random's generator, `shuffle` is Collections.shuffle's own generator
    // and `threadLocal` is ThreadLocalRandom.current().
    RngSharedNextInt => "rng.shared.nextInt",
    RngMathNextDouble => "rng.math.nextDouble",
    RngShuffleNextInt => "rng.shuffle.nextInt",
    RngThreadLocalNextDouble => "rng.threadLocal.nextDouble",
    RngThreadLocalNextInt => "rng.threadLocal.nextInt",
    // String.lowercase(Locale.getDefault()) / toLowerCase(): the JVM default locale.
    LowercaseDefault => "string.lowercaseDefault",
    // Fishing: the spot NPC, familiar, world and content calls of
    // FishingListener, FishingPulse and FishingOption.
    NpcLocation => "npc.location",
    NpcIsInvisible => "npc.isInvisible",
    ApiGetUsedOption => "api.getUsedOption",
    ApiInInventory => "api.inInventory",
    ApiHasLevelDyn => "api.hasLevelDyn",
    ApiHasSpaceFor => "api.hasSpaceFor",
    InventoryHasSpaceFor => "inventory.hasSpaceFor",
    FamiliarHasFamiliar => "familiar.hasFamiliar",
    FamiliarGetFamiliar => "familiar.getFamiliar",
    ForagerHandlePassiveAction => "forager.handlePassiveAction",
    PlayerDirection => "player.direction",
    PathfinderFind => "pathfinder.find",
    PathfinderFindFrom => "pathfinder.findFrom",
    PathWalk => "path.walk",
    PlayerClocks => "player.clocks",
    GameWorldTicks => "gameWorld.ticks",
    GameWorldSettings => "gameWorld.settings",
    GameSettingsIsDevMode => "gameSettings.isDevMode",
    PlayerIncrementAttribute => "player.incrementAttribute",
    PlayerDebug => "player.debug",
    DiaryFinishTask => "diary.finishTask",
    FogGlovesUpdateCharges => "fogGloves.updateCharges",
    SkillTasksHasTask => "skillTasks.hasTask",
    SkillTasksDecreaseTask => "skillTasks.decreaseTask",
    PulserSubmit => "pulser.submit",
    RngSharedNextDouble => "rng.shared.nextDouble",
    // Asked for by the binding when `addItemOrDrop` cannot place an item:
    // `GroundItemManager.create(item, player)`.
    GroundItemManagerCreate => "groundItemManager.create",
}

/// A deferred native body handed to the owner's script queue or pulse manager.
#[derive(Clone, Debug, PartialEq)]
pub enum Task {
    /// WoodcuttingListener's one-chop branch reward (`queueScript(player, 1, STRONG)`).
    BranchReward {
        player: Handle,
        dramen: bool,
        reward: i32,
    },
    /// WoodcuttingListener's farming-patch reactivation (`queueScript(player, 2, SOFT)`).
    ReactivateNode { player: Handle, node: Handle },
    /// ProspectListener's delayed message (`Pulse(3)`).
    ProspectMessage { player: Handle, reward: Option<i32> },
    /// FishingPulse.animate's anonymous `Pulse(1)` handed to `GameWorld.Pulser`.
    /// It never finishes; `counter` is its own field and the body acts once,
    /// on the call that finds it at five.
    BarehandCatch {
        player: Handle,
        node: Handle,
        counter: i32,
    },
    /// `BarbFishingPulse(player)` (super::barbarian).
    BarbarianFishing { player: Handle },
    /// `FishCuttingPulse(player, fish)` (super::barbarian).
    FishCutting { player: Handle, fish: i32 },
}

pub trait Context {
    fn call(&mut self, service: Service, args: &[Arg]) -> Result<Arg>;
    /// Hands over a deferred body. `service` is `ApiQueueScript` (arguments
    /// delay, strength name, persist), `PulseManagerRun` or `PulserSubmit`
    /// (argument delay).
    fn defer(&mut self, service: Service, args: &[Arg], task: Task) -> Result<()>;
}

/// Typed access to replies.
pub(crate) trait Calls: Context {
    fn unit(&mut self, s: Service, a: &[Arg]) -> Result<()> {
        self.call(s, a).map(|_| ())
    }
    fn boolean(&mut self, s: Service, a: &[Arg]) -> Result<bool> {
        match self.call(s, a)? {
            Arg::Bool(v) => Ok(v),
            _ => Err(Error::Reply),
        }
    }
    fn int(&mut self, s: Service, a: &[Arg]) -> Result<i32> {
        match self.call(s, a)? {
            Arg::Int(v) => i32::try_from(v).map_err(|_| Error::Reply),
            _ => Err(Error::Reply),
        }
    }
    fn handle(&mut self, s: Service, a: &[Arg]) -> Result<Handle> {
        match self.call(s, a)? {
            Arg::Int(v) => Ok(v),
            _ => Err(Error::Reply),
        }
    }
    fn optional_int(&mut self, s: Service, a: &[Arg]) -> Result<Option<i32>> {
        match self.call(s, a)? {
            Arg::Null => Ok(None),
            Arg::Int(v) => i32::try_from(v).map(Some).map_err(|_| Error::Reply),
            _ => Err(Error::Reply),
        }
    }
    fn double(&mut self, s: Service, a: &[Arg]) -> Result<f64> {
        match self.call(s, a)? {
            Arg::Double(v) => Ok(v),
            _ => Err(Error::Reply),
        }
    }
    fn optional_double(&mut self, s: Service, a: &[Arg]) -> Result<Option<f64>> {
        match self.call(s, a)? {
            Arg::Null => Ok(None),
            Arg::Double(v) => Ok(Some(v)),
            _ => Err(Error::Reply),
        }
    }
    fn text(&mut self, s: Service, a: &[Arg]) -> Result<String> {
        match self.call(s, a)? {
            Arg::Text(v) => Ok(v),
            _ => Err(Error::Reply),
        }
    }
    fn location(&mut self, s: Service, a: &[Arg]) -> Result<[i32; 3]> {
        match self.call(s, a)? {
            Arg::List(v) => match v.as_slice() {
                [Arg::Int(x), Arg::Int(y), Arg::Int(z)] => Ok([*x as i32, *y as i32, *z as i32]),
                _ => Err(Error::Reply),
            },
            _ => Err(Error::Reply),
        }
    }
    fn lower_default(&mut self, text: String) -> Result<String> {
        self.text(Service::LowercaseDefault, &[text.into()])
    }
}
impl<C: Context + ?Sized> Calls for C {}

/// Location.getRegionId().
pub fn region_id(location: [i32; 3]) -> i32 {
    (location[0] >> 6) << 8 | (location[1] >> 6)
}
