//! `core.api`: the ContentAPI functions content is written against, and the
//! other files of the package (ApiExtensions, Commands, Container,
//! ContentInterface, DialUtils, EquipmentSlot, Event, God,
//! IfaceSettingsBuilder, InputType, the listener interfaces, MapArea,
//! regionspec and the utils Vector, WeightedTable and PlayerCamera).
//!
//! | Original | Here |
//! | --- | --- |
//! | inventory, bank, equipment and beast-of-burden functions, `note`/`unnote`, `decantContainer`, `dumpContainer` | [`items`] |
//! | animations, graphics, audio, projectiles, `impact`, scenery, ground items, charges | [`visuals`] |
//! | NPC and scenery lookups, locations, borders, movement, `face`, teleports, doors, map zones | [`world`] |
//! | interfaces, messages, dialogue boxes, input prompts, `runcs2`, item select, skill dialogue, hint icons | [`ui`] |
//! | attributes, timers, poison, locks, pulses, queued scripts, clocks, stun | [`state`] |
//! | varps and varbits (thin wrappers over [`crate::player::variables`]) | [`vars`] |
//! | quests, requirements, diaries | [`quests`] |
//! | skills, slayer, ironman, bank accounts, Grand Exchange, shops, house, credits, logging, configuration | [`services`] |
//! | the other core/api files | [`api_files`] |
//!
//! Every function is the original's body over [`Host`]. A call the original
//! makes on another class is [`Host::call`] with the original member
//! ([`Member`]) and its arguments; field reads of world state that the
//! original makes without a call are the plain [`Host`] methods. Nothing
//! here supplies a default for a service. Content callbacks, pulses and
//! queued scripts are host nodes; the bodies ContentAPI itself builds
//! ([`PulseBody`], [`ScriptBody`], [`SkillHandler`]) run through
//! [`state::run_pulse`], [`state::run_script`] and [`ui::skill_create`].
//! See docs/content-api.md for the inventory of all 261 functions and the
//! earlier slices' ports (re-exported in [`ported`]).
pub mod api_files;
pub mod items;
pub mod ported;
pub mod quests;
pub mod services;
pub mod state;
pub mod ui;
pub mod vars;
pub mod visuals;
pub mod world;

/// What stops a call, as the original's exception classes do.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// `NullPointerException` (Kotlin's null checks included).
    Null,
    /// `ClassCastException`.
    Cast,
    /// `IndexOutOfBoundsException` (arrays included).
    Index,
    /// `IllegalStateException`, with its message.
    State(String),
    /// `IllegalArgumentException`, with its message.
    Argument(String),
    /// `NoSuchElementException`.
    NoSuchElement,
    /// Kotlin's `UninitializedPropertyAccessException` (a `lateinit` read).
    Lateinit,
    /// A service failed (what it threw, if anything).
    Binding(Option<String>),
}
impl Error {
    /// The exception class's short name as the corpus records it.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Cast => "Cast",
            Self::Index => "Index",
            Self::State(_) => "State",
            Self::Argument(_) => "Argument",
            Self::NoSuchElement => "NoSuchElement",
            Self::Lateinit => "Lateinit",
            Self::Binding(_) => "Binding",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// `core.game.world.map.Location`: x and y kept as 15-bit signed values and
/// z as two bits, as `LocationHash` packs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Location(i32);
impl Location {
    /// `Location(x, y, z)`: z is `(z % 4 + 4) % 4`.
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        let z = z.rem_euclid(4);
        Self(((z & 3) << 30) | ((x & 0x7FFF) << 15) | (y & 0x7FFF))
    }
    pub fn x(self) -> i32 {
        let raw = (self.0 as u32 >> 15) as i32 & 0x7FFF;
        if raw >= 0x4000 { raw - 0x8000 } else { raw }
    }
    pub fn y(self) -> i32 {
        let raw = self.0 & 0x7FFF;
        if raw >= 0x4000 { raw - 0x8000 } else { raw }
    }
    pub fn z(self) -> i32 {
        (self.0 as u32 >> 30) as i32
    }
    /// `transform(diffX, diffY, z)`: `(this.z + z) % 4` before the
    /// constructor's normalisation.
    pub fn transform(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(
            self.x().wrapping_add(dx),
            self.y().wrapping_add(dy),
            self.z().wrapping_add(dz) % 4,
        )
    }
    /// `transform(vector)`: floored components, plane 0 (`create(x, y)`).
    pub fn transform_vector(self, v: api_files::Vector) -> Self {
        Self::new(
            self.x().wrapping_add(java_double_to_int(v.x.floor())),
            self.y().wrapping_add(java_double_to_int(v.y.floor())),
            0,
        )
    }
    /// `Location.getDelta(location, other)`.
    pub fn delta(self, other: Self) -> Self {
        Self::new(
            other.x().wrapping_sub(self.x()),
            other.y().wrapping_sub(self.y()),
            other.z().wrapping_sub(self.z()),
        )
    }
    /// `Location.getDistance(first, second)`.
    pub fn distance(self, other: Self) -> f64 {
        let dx = self.x().wrapping_sub(other.x());
        let dy = self.y().wrapping_sub(other.y());
        (dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as f64).sqrt()
    }
    /// `withinDistance(other, dist)`.
    pub fn within_distance(self, other: Self, dist: i32) -> bool {
        other.z() == self.z() && self.distance(other) <= dist as f64
    }
    /// `cardinalTiles`: west, south, east, north.
    pub fn cardinal_tiles(self) -> [Self; 4] {
        [
            self.transform(-1, 0, 0),
            self.transform(0, -1, 0),
            self.transform(1, 0, 0),
            self.transform(0, 1, 0),
        ]
    }
    /// `regionId`.
    pub fn region_id(self) -> i32 {
        (self.x() >> 6 << 8) | (self.y() >> 6)
    }
}
impl std::fmt::Display for Location {
    /// `Location.toString()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}, {}]", self.x(), self.y(), self.z())
    }
}

/// Java's `(int) double`: NaN is 0, out-of-range values saturate.
pub fn java_double_to_int(v: f64) -> i32 {
    if v.is_nan() {
        0
    } else if v >= i32::MAX as f64 {
        i32::MAX
    } else if v <= i32::MIN as f64 {
        i32::MIN
    } else {
        v as i32
    }
}

/// `Animator.Priority`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Mid,
    High,
    VeryHigh,
}
impl Priority {
    pub const ALL: [Self; 4] = [Self::Low, Self::Mid, Self::High, Self::VeryHigh];
    pub fn name(self) -> &'static str {
        ["LOW", "MID", "HIGH", "VERY_HIGH"][self as usize]
    }
}

/// An `Animation` value (`Animation(id, priority)`; `setObject` sets the
/// scenery it animates).
#[derive(Clone, Debug, PartialEq)]
pub struct Anim<N> {
    pub id: i32,
    pub priority: Priority,
    pub object: Option<N>,
}
impl<N> Anim<N> {
    /// `Animation(id)` (priority MID).
    pub fn new(id: i32) -> Self {
        Self {
            id,
            priority: Priority::Mid,
            object: None,
        }
    }
    pub fn with(id: i32, priority: Priority) -> Self {
        Self {
            id,
            priority,
            object: None,
        }
    }
}

/// A `Graphics` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gfx {
    pub id: i32,
    pub height: i32,
    pub delay: i32,
}
impl Gfx {
    /// `Graphics(id)`.
    pub fn new(id: i32) -> Self {
        Self {
            id,
            height: 0,
            delay: 0,
        }
    }
}

/// A value of a generic (`T`, `Any`) parameter or an attribute.
#[derive(Clone, Debug, PartialEq)]
pub enum Value<N> {
    Null,
    Int(i32),
    Long(i64),
    Double(f64),
    Bool(bool),
    Text(String),
    Loc(Location),
    Node(N),
    Anim(Anim<N>),
    Gfx(Gfx),
    /// An enum constant (`InputType`).
    Enum(&'static str),
}

/// An argument of a recorded call, as the corpus prints it.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg<N> {
    Null,
    Node(N),
    Int(i32),
    Bool(bool),
    Text(String),
    Double(f64),
    Long(i64),
    Short(i16),
    Loc(Location),
    /// An enum constant's name.
    Enum(&'static str),
    /// A list: arrays, varargs and the value objects the originals build
    /// (`Animation`, `Graphics`, `Component`, `Audio`, packet contexts).
    List(Vec<Arg<N>>),
    /// A value printed by its `toString` (`ForceMoveCtx`).
    Display(String),
}
impl<N> Arg<N> {
    pub fn text(s: &str) -> Self {
        Self::Text(s.to_string())
    }
    pub fn opt_node(n: Option<&N>) -> Self
    where
        N: Clone,
    {
        n.map_or(Self::Null, |n| Self::Node(n.clone()))
    }
    pub fn loc(l: Option<Location>) -> Self {
        l.map_or(Self::Null, Self::Loc)
    }
    pub fn ints(v: &[i32]) -> Self {
        Self::List(v.iter().map(|i| Self::Int(*i)).collect())
    }
    pub fn texts<S: AsRef<str>>(v: &[S]) -> Self {
        Self::List(
            v.iter()
                .map(|s| Self::Text(s.as_ref().to_string()))
                .collect(),
        )
    }
    pub fn anim(a: &Anim<N>) -> Self
    where
        N: Clone,
    {
        Self::List(vec![
            Self::text("anim"),
            Self::Int(a.id),
            Self::Enum(a.priority.name()),
            Self::opt_node(a.object.as_ref()),
        ])
    }
    pub fn gfx(g: Gfx) -> Self {
        Self::List(vec![
            Self::text("gfx"),
            Self::Int(g.id),
            Self::Int(g.height),
            Self::Int(g.delay),
        ])
    }
    pub fn component(id: i32) -> Self {
        Self::List(vec![Self::text("component"), Self::Int(id)])
    }
    pub fn borders(b: [i32; 5]) -> Self {
        let mut v = vec![Self::text("borders")];
        v.extend(b.iter().map(|i| Self::Int(*i)));
        Self::List(v)
    }
    pub fn value(v: &Value<N>) -> Self
    where
        N: Clone,
    {
        match v {
            Value::Null => Self::Null,
            Value::Int(i) => Self::Int(*i),
            Value::Long(l) => Self::Long(*l),
            Value::Double(d) => Self::Double(*d),
            Value::Bool(b) => Self::Bool(*b),
            Value::Text(t) => Self::Text(t.clone()),
            Value::Loc(l) => Self::Loc(*l),
            Value::Node(n) => Self::Node(n.clone()),
            Value::Anim(a) => Self::anim(a),
            Value::Gfx(g) => Self::gfx(*g),
            Value::Enum(e) => Self::Enum(e),
        }
    }
}

/// A requirement object (`core.game.requirement`) as ContentAPI handles it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Requirement {
    Skill {
        skill: i32,
        level: i32,
        soft: bool,
    },
    /// `QuestReq(questReq, stageRequired)`: the `QuestRequirements` constant.
    Quest {
        requirement: String,
        stage: i32,
    },
    QpReq(i32),
    QpCumulative(i32),
}
impl Requirement {
    pub fn arg<N>(&self) -> Arg<N> {
        match self {
            Self::Skill { skill, level, soft } => Arg::List(vec![
                Arg::text("SkillReq"),
                Arg::Int(*skill),
                Arg::Int(*level),
                Arg::Bool(*soft),
            ]),
            Self::Quest { requirement, stage } => Arg::List(vec![
                Arg::text("QuestReq"),
                Arg::Text(requirement.clone()),
                Arg::Int(*stage),
            ]),
            Self::QpReq(a) => Arg::List(vec![Arg::text("QPReq"), Arg::Int(*a)]),
            Self::QpCumulative(a) => Arg::List(vec![Arg::text("QPCumulative"), Arg::Int(*a)]),
        }
    }
}

/// A `Path` answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Path<N> {
    pub handle: N,
    pub successful: bool,
    pub move_near: bool,
}

/// What a recorded call answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply<N> {
    Unit,
    Bool(bool),
    Int(i32),
    Double(f64),
    Text(Option<String>),
    Texts(Option<Vec<String>>),
    Node(Option<N>),
    Nodes(Vec<Option<N>>),
    Ints(Option<Vec<i32>>),
    Loc(Option<Location>),
    /// An attribute read: `None` when the entity holds no value (the
    /// original then answers the caller's default).
    Value(Option<Value<N>>),
    Path(Option<Path<N>>),
    Borders(Option<[i32; 5]>),
    Evaluation(Option<(bool, Vec<Requirement>)>),
    Bits(Option<(i64, i64)>),
    /// `QuestRepository.getQuests()`: (Quests constant, quest node), in the
    /// map's order.
    QuestMap(Vec<(String, N)>),
    /// An enum constant's name (`IronmanMode`, `SkillingTool`, `Tasks`,
    /// `Master`).
    Enum(Option<String>),
    /// `Component`'s id, if one was returned.
    Component(Option<i32>),
    OptInt(Option<i32>),
}
impl<N> Reply<N> {
    fn mismatch(&self) -> Error {
        Error::Binding(Some(format!(
            "unexpected reply {}",
            std::any::type_name::<Self>()
        )))
    }
    pub fn bool(self) -> Result<bool> {
        match self {
            Self::Bool(b) => Ok(b),
            other => Err(other.mismatch()),
        }
    }
    pub fn int(self) -> Result<i32> {
        match self {
            Self::Int(i) => Ok(i),
            other => Err(other.mismatch()),
        }
    }
    pub fn node(self) -> Result<Option<N>> {
        match self {
            Self::Node(n) => Ok(n),
            other => Err(other.mismatch()),
        }
    }
    pub fn nodes(self) -> Result<Vec<Option<N>>> {
        match self {
            Self::Nodes(n) => Ok(n),
            other => Err(other.mismatch()),
        }
    }
    pub fn text(self) -> Result<Option<String>> {
        match self {
            Self::Text(t) => Ok(t),
            other => Err(other.mismatch()),
        }
    }
    pub fn texts(self) -> Result<Option<Vec<String>>> {
        match self {
            Self::Texts(t) => Ok(t),
            other => Err(other.mismatch()),
        }
    }
    pub fn ints(self) -> Result<Option<Vec<i32>>> {
        match self {
            Self::Ints(t) => Ok(t),
            other => Err(other.mismatch()),
        }
    }
    pub fn loc(self) -> Result<Option<Location>> {
        match self {
            Self::Loc(l) => Ok(l),
            other => Err(other.mismatch()),
        }
    }
    pub fn value(self) -> Result<Option<Value<N>>> {
        match self {
            Self::Value(v) => Ok(v),
            other => Err(other.mismatch()),
        }
    }
    pub fn path(self) -> Result<Option<Path<N>>> {
        match self {
            Self::Path(p) => Ok(p),
            other => Err(other.mismatch()),
        }
    }
    pub fn borders(self) -> Result<Option<[i32; 5]>> {
        match self {
            Self::Borders(b) => Ok(b),
            other => Err(other.mismatch()),
        }
    }
    pub fn evaluation(self) -> Result<Option<(bool, Vec<Requirement>)>> {
        match self {
            Self::Evaluation(e) => Ok(e),
            other => Err(other.mismatch()),
        }
    }
    pub fn bits(self) -> Result<Option<(i64, i64)>> {
        match self {
            Self::Bits(b) => Ok(b),
            other => Err(other.mismatch()),
        }
    }
    pub fn quest_map(self) -> Result<Vec<(String, N)>> {
        match self {
            Self::QuestMap(m) => Ok(m),
            other => Err(other.mismatch()),
        }
    }
    pub fn constant(self) -> Result<Option<String>> {
        match self {
            Self::Enum(e) => Ok(e),
            other => Err(other.mismatch()),
        }
    }
    pub fn opt_int(self) -> Result<Option<i32>> {
        match self {
            Self::OptInt(i) => Ok(i),
            other => Err(other.mismatch()),
        }
    }
    pub fn component(self) -> Result<Option<i32>> {
        match self {
            Self::Component(c) => Ok(c),
            other => Err(other.mismatch()),
        }
    }
}

/// The kind of [`Reply`] a [`Member`] answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyKind {
    Unit,
    Bool,
    Int,
    Double,
    Text,
    Texts,
    Node,
    Nodes,
    Ints,
    Loc,
    Value,
    Path,
    Borders,
    Evaluation,
    Bits,
    QuestMap,
    Enum,
    Component,
    OptInt,
}

macro_rules! members {
    ($($(#[$doc:meta])* $v:ident = $name:literal => $reply:ident,)*) => {
        /// A member of another class that a core/api body calls, named as
        /// the original (`Class.member`; a constructor is the class name).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Member { $($(#[$doc])* $v,)* }
        impl Member {
            pub const ALL: &'static [Member] = &[$(Member::$v,)*];
            pub fn name(self) -> &'static str { match self { $(Member::$v => $name,)* } }
            pub fn reply(self) -> ReplyKind { match self { $(Member::$v => ReplyKind::$reply,)* } }
        }
    };
}

members! {
    DiaryFinishTask = "AchievementDiaryManager.finishTask" => Unit,
    LockInteractions = "ActionLocks.lockInteractions" => Unit,
    LockMovement = "ActionLocks.lockMovement" => Unit,
    AnimationCycles = "AnimationDefinition.getCycles" => Int,
    AnimationDurationTicks = "AnimationDefinition.getDurationTicks" => Int,
    AnimatorAnimate = "Animator.animate" => Bool,
    AnimatorForce = "Animator.forceAnimation" => Unit,
    AppearanceSetAnimations = "Appearance.setAnimations" => Unit,
    AppearanceDefaultAnimations = "Appearance.setDefaultAnimations" => Unit,
    AppearanceSync = "Appearance.sync" => Unit,
    BankOpen = "BankContainer.open" => Unit,
    BankOpenDepositBox = "BankContainer.openDepositBox" => Unit,
    BankPinOpenSettings = "BankPinManager.openSettings" => Unit,
    BookcaseGet = "BookcaseBitfields.get" => Bits,
    BookcaseIsBitSet = "BookcaseBitfields.isBitSet" => Bool,
    BookcaseSet = "BookcaseBitfields.set" => Unit,
    BookcaseSetBit = "BookcaseBitfields.setBit" => Bits,
    DelayNextAttack = "CombatPulse.delayNextAttack" => Unit,
    ProjectileClipped = "CombatSwingHandler.isProjectileClipped" => Bool,
    CommandRegister = "CommandMapping.register" => Unit,
    ConsumableById = "Consumables.getConsumableById" => Node,
    ContainerAdd = "Container.add" => Bool,
    ContainerCanAdd = "Container.canAdd" => Bool,
    ContainerContainItems = "Container.containItems" => Bool,
    ContainerContains = "Container.contains" => Bool,
    ContainerFreeSlots = "Container.freeSlots" => Int,
    ContainerGet = "Container.get" => Node,
    ContainerGetAll = "Container.getAll" => Nodes,
    ContainerGetAmount = "Container.getAmount" => Int,
    ContainerHasSpaceFor = "Container.hasSpaceFor" => Bool,
    ContainerIsEmpty = "Container.isEmpty" => Bool,
    ContainerItemCount = "Container.itemCount" => Int,
    ContainerRemove = "Container.remove" => Bool,
    ContainerReplace = "Container.replace" => Node,
    ContainerToArray = "Container.toArray" => Nodes,
    ContainerUpdate = "Container.update" => Unit,
    DialogueAddAction = "DialogueInterpreter.addAction" => Unit,
    DialogueClose = "DialogueInterpreter.close" => Bool,
    /// `open(int dialogueKey, Object... args)`.
    DialogueOpenKey = "DialogueInterpreter.open" => Bool,
    /// `open(DialogueFile file, Object... args)`.
    DialogueOpenFile = "DialogueInterpreter.open" => Unit,
    DialogueSendDialogue = "DialogueInterpreter.sendDialogue" => Component,
    DialogueSendDialogues = "DialogueInterpreter.sendDialogues" => Component,
    DialogueSendDoubleItem = "DialogueInterpreter.sendDoubleItemMessage" => Component,
    DialogueSendInput = "DialogueInterpreter.sendInput" => Unit,
    DialogueSendItem = "DialogueInterpreter.sendItemMessage" => Component,
    DialogueSendLongInput = "DialogueInterpreter.sendLongInput" => Unit,
    DialogueSendMessageInput = "DialogueInterpreter.sendMessageInput" => Unit,
    DialogueSendOptions = "DialogueInterpreter.sendOptions" => Component,
    DoorEndLocation = "DoorActionHandler.getEndLocation" => Loc,
    DoorAutowalk = "DoorActionHandler.handleAutowalkDoorWithLocation" => Bool,
    DoorHandle = "DoorActionHandler.handleDoor" => Unit,
    DynamicRegionNew = "DynamicRegion" => Node,
    DynamicRegionCreate = "DynamicRegion.create" => Node,
    DynamicRegionReserve = "DynamicRegion.reserveArea" => Borders,
    DynamicRegionReplaceChunk = "DynamicRegion.replaceChunk" => Unit,
    EntityAnimate = "Entity.animate" => Bool,
    EntityFace = "Entity.face" => Bool,
    EntityFaceLocation = "Entity.faceLocation" => Unit,
    EntityFaceTemporary = "Entity.faceTemporary" => Bool,
    EntityGetAttribute = "Entity.getAttribute" => Value,
    EntityDragonfireProtection = "Entity.getDragonfireProtection" => Int,
    EntityGraphics = "Entity.graphics" => Bool,
    EntityPoisonImmune = "Entity.isPoisonImmune" => Bool,
    EntityLock = "Entity.lock" => Unit,
    EntityRemoveAttribute = "Entity.removeAttribute" => Unit,
    EntitySendChat = "Entity.sendChat" => Bool,
    EntitySetAttribute = "Entity.setAttribute" => Unit,
    EntityUnlock = "Entity.unlock" => Unit,
    EntityVisualize = "Entity.visualize" => Bool,
    FamiliarBoost = "FamiliarManager.getBoost" => Int,
    FamiliarGet = "FamiliarManager.getFamiliar" => Node,
    FamiliarHas = "FamiliarManager.hasFamiliar" => Bool,
    ForceMovementRun = "ForceMovement.run" => Unit,
    GeRecordsInstance = "GrandExchangeRecords.getInstance" => Unit,
    GeRecordsOffer = "GrandExchangeRecords.getOffer" => Node,
    GeRecordsOpenCollection = "GrandExchangeRecords.openCollectionBox" => Unit,
    GraphicsSend = "Graphics.send" => Unit,
    GroundCreate = "GroundItemManager.create" => Node,
    GroundDestroy = "GroundItemManager.destroy" => Node,
    GroundItems = "GroundItemManager.getItems" => Nodes,
    HintFreeSlot = "HintIconManager.freeSlot" => Int,
    HintRegister = "HintIconManager.registerHintIcon" => Int,
    HintRemove = "HintIconManager.removeHintIcon" => Unit,
    HouseHas = "HouseManager.hasHouse" => Bool,
    ManualHit = "ImpactHandler.manualHit" => Unit,
    ListenersRun = "InteractionListeners.run" => Bool,
    InterfaceClose = "InterfaceManager.close" => Bool,
    InterfaceCloseChatbox = "InterfaceManager.closeChatbox" => Unit,
    InterfaceCloseOverlay = "InterfaceManager.closeOverlay" => Unit,
    InterfaceCloseSingleTab = "InterfaceManager.closeSingleTab" => Bool,
    InterfaceOpen = "InterfaceManager.open" => Unit,
    InterfaceOpenOverlay = "InterfaceManager.openOverlay" => Unit,
    InterfaceOpenSingleTab = "InterfaceManager.openSingleTab" => Unit,
    InterfaceRemoveTabs = "InterfaceManager.removeTabs" => Unit,
    InterfaceRestoreTabs = "InterfaceManager.restoreTabs" => Unit,
    IronmanCheck = "IronmanManager.checkRestriction" => Bool,
    IronmanMode = "IronmanManager.getMode" => Enum,
    IronmanIs = "IronmanManager.isIronman" => Bool,
    /// `new Item(id, amount, charge)`; the definition lookup answers no
    /// node for a missing definition.
    NewItem = "Item" => Node,
    ItemSetCharge = "Item.setCharge" => Unit,
    ItemHasAction = "ItemDefinition.hasAction" => Bool,
    MapZoneRegister = "MapZone.register" => Unit,
    NpcClear = "NPC.clear" => Unit,
    NpcReTransform = "NPC.reTransform" => Unit,
    NpcSetInvisible = "NPC.setInvisible" => Unit,
    NpcTransform = "NPC.transform" => Unit,
    NpcTransformHp = "NPC.transformWithHpCarryover" => Unit,
    NpcHasAction = "NPCDefinition.hasAction" => Bool,
    SendAnimationInterface = "PacketDispatch.sendAnimationInterface" => Unit,
    SendIfaceSettings = "PacketDispatch.sendIfaceSettings" => Unit,
    SendInterfaceConfig = "PacketDispatch.sendInterfaceConfig" => Unit,
    SendItemOnInterface = "PacketDispatch.sendItemOnInterface" => Unit,
    SendItemZoomOnInterface = "PacketDispatch.sendItemZoomOnInterface" => Unit,
    SendPlayerOnInterface = "PacketDispatch.sendPlayerOnInterface" => Unit,
    SendRunScript = "PacketDispatch.sendRunScript" => Unit,
    SendSceneryAnimation = "PacketDispatch.sendSceneryAnimation" => Unit,
    SendString = "PacketDispatch.sendString" => Unit,
    SendVarc = "PacketDispatch.sendVarcUpdate" => Unit,
    SendVarp = "PacketDispatch.sendVarp" => Unit,
    PacketSend = "PacketRepository.send" => Unit,
    PathWalk = "Path.walk" => Unit,
    PathfinderFind = "Pathfinder.find" => Path,
    PlayerSendMessage = "Player.sendMessage" => Unit,
    PlayerSendMessages = "Player.sendMessages" => Unit,
    MonitorLog = "PlayerMonitor.log" => Unit,
    StatsRareDrop = "PlayerStatsCounter.incrementRareDrop" => Unit,
    PoisonSetSeverity = "Poison.setSeverity" => Unit,
    PrayerGet = "Prayer.get" => Bool,
    ProjectileCreate = "Projectile.create" => Node,
    ProjectileSend = "Projectile.send" => Unit,
    SetTeleportLocation = "Properties.setTeleportLocation" => Unit,
    PulseManagerClear = "PulseManager.clear" => Unit,
    QuestFinish = "Quest.finish" => Unit,
    QuestHasRequirements = "Quest.hasRequirements" => Bool,
    QuestStart = "Quest.start" => Unit,
    QuestAvailablePoints = "QuestRepository.getAvailablePoints" => Int,
    QuestPoints = "QuestRepository.getPoints" => Int,
    QuestGet = "QuestRepository.getQuest" => Node,
    QuestAll = "QuestRepository.getQuests" => QuestMap,
    QuestStage = "QuestRepository.getStage" => Int,
    QuestSetStage = "QuestRepository.setStage" => Unit,
    QuestSyncTab = "QuestRepository.syncronizeTab" => Unit,
    RegionIsActive = "Region.isActive" => Bool,
    RegionLoad = "Region.load" => Unit,
    ChunkFlag = "RegionChunk.flag" => Unit,
    ChunkCopy = "RegionChunk.copy" => Node,
    LocalNpcs = "RegionManager.getLocalNPCs" => Nodes,
    LocalPlayers = "RegionManager.getLocalPlayers" => Nodes,
    GetObject = "RegionManager.getObject" => Node,
    RegionChunkOf = "RegionManager.getRegionChunk" => Node,
    FindNpc = "Repository.findNPC" => Node,
    RepositoryNpcs = "Repository.getNpcs" => Nodes,
    SendNews = "Repository.sendNews" => Unit,
    Evaluate = "Requirement.evaluate" => Evaluation,
    NewScenery = "Scenery" => Node,
    ScenerySetActive = "Scenery.setActive" => Unit,
    ScenerySetCharge = "Scenery.setCharge" => Unit,
    SceneryTransform = "Scenery.transform" => Node,
    SceneryAdd = "SceneryBuilder.add" => Unit,
    SceneryRemove = "SceneryBuilder.remove" => Bool,
    SceneryReplace = "SceneryBuilder.replace" => Bool,
    SceneryDefinition = "SceneryDefinition.forId" => Node,
    SceneryChildren = "SceneryDefinition.getChildrenIds" => Ints,
    SceneryHasAction = "SceneryDefinition.hasAction" => Bool,
    ActiveScript = "ScriptProcessor.getActiveScript" => Node,
    ScriptsReset = "ScriptProcessor.reset" => Unit,
    ScriptsApRange = "ScriptProcessor.setApRange" => Unit,
    ScriptsApRangeCalled = "ScriptProcessor.setApRangeCalled" => Unit,
    ScriptsDelay = "ScriptProcessor.setDelay" => Unit,
    ShopOpen = "Shop.openFor" => Unit,
    HatchetOf = "SkillingTool.getHatchet" => Enum,
    PickaxeOf = "SkillingTool.getPickaxe" => Enum,
    AddExperience = "Skills.addExperience" => Unit,
    DecrementPrayer = "Skills.decrementPrayerPoints" => Unit,
    GetLevel = "Skills.getLevel" => Int,
    MasteredSkills = "Skills.getMasteredSkills" => Int,
    GetStaticLevel = "Skills.getStaticLevel" => Int,
    Heal = "Skills.heal" => Int,
    IncrementPrayer = "Skills.incrementPrayerPoints" => Unit,
    SetLevel = "Skills.setLevel" => Unit,
    IsSlayerEq = "SlayerEquipmentFlags.isSlayerEq" => Bool,
    SlayerUpdateFlags = "SlayerEquipmentFlags.updateFlags" => Unit,
    SlayerTaskFlags = "SlayerFlags.getTaskFlags" => Int,
    SlayerActiveTask = "SlayerManager.getActiveTask" => Enum,
    SlayerAmount = "SlayerManager.getAmount" => Int,
    SlayerInstance = "SlayerManager.getInstance" => Unit,
    SlayerMaster = "SlayerManager.getMaster" => Enum,
    SlayerTaskName = "SlayerManager.getTaskName" => Text,
    SlayerHasTask = "SlayerManager.hasTask" => Bool,
    LadderAdd = "SpecialLadders.add" => Unit,
    StockMarketOpen = "StockMarket.openFor" => Unit,
    StorageAddItem = "StorageContainer.addItem" => Unit,
    StorageContains = "StorageContainer.contains" => Bool,
    /// Answers the storable's `bitIndex`, if one is found.
    StorableByDisplayId = "StorageInterface.findStorableByDisplayId" => OptInt,
    StorageContainerOf = "StorageInterface.getStorageContainer" => Unit,
    Log = "SystemLogger.processLogEntry" => Unit,
    TaskTip = "Tasks.getTip" => Texts,
    TeleportSend = "TeleportManager.send" => Bool,
    TimerGet = "TimerManager.getTimer" => Node,
    TimerRegister = "TimerManager.registerTimer" => Unit,
    TimerRemove = "TimerManager.removeTimer" => Unit,
    TimerInstance = "TimerRegistry.getTimerInstance" => Node,
    NewToml = "Toml" => Node,
    UpdateMaskRegister = "UpdateMasks.register" => Bool,
    CreditsGet = "UserAccountInfo.getCredits" => Int,
    CreditsSet = "UserAccountInfo.setCredits" => Unit,
    WalkingReset = "WalkingQueue.reset" => Unit,
    BordersForRegion = "ZoneBorders.forRegion" => Borders,
    BordersRandomLoc = "ZoneBorders.getRandomLoc" => Loc,
    BordersInside = "ZoneBorders.insideBorder" => Bool,
    ZoneConfigure = "ZoneBuilder.configure" => Unit,
    /// A content callback answering nothing.
    Callback = "@lambda" => Unit,
    /// A content callback answering a Boolean (a queued script body).
    CallbackBool = "@lambda" => Bool,
    /// A content callback answering an Int.
    CallbackInt = "@lambda" => Int,
    /// A content callback answering an object (`FillChunkContract`'s delegate).
    CallbackNode = "@lambda" => Node,
}

/// What a node is, for the original's `is` checks and casts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Npc,
    /// A familiar that is not a beast of burden.
    Familiar,
    BurdenBeast,
    Item,
    GroundItem,
    Scenery,
    /// A `DialogueFile`.
    DialogueFile,
    /// A `SkillDialogueHandler`.
    SkillHandler,
    /// Any other object (timers, scripts, callbacks, regions).
    Other,
}
impl Kind {
    pub fn is_entity(self) -> bool {
        matches!(
            self,
            Self::Player | Self::Npc | Self::Familiar | Self::BurdenBeast
        )
    }
    pub fn is_npc(self) -> bool {
        matches!(self, Self::Npc | Self::Familiar | Self::BurdenBeast)
    }
    pub fn is_item(self) -> bool {
        matches!(self, Self::Item | Self::GroundItem)
    }
    /// `javaClass.simpleName` of a node of this kind.
    pub fn simple_name(self) -> &'static str {
        match self {
            Self::Player => "Player",
            Self::Npc => "NPC",
            Self::Familiar => "Familiar",
            Self::BurdenBeast => "BurdenBeast",
            Self::Item => "Item",
            Self::GroundItem => "GroundItem",
            Self::Scenery => "Scenery",
            Self::DialogueFile => "DialogueFile",
            Self::SkillHandler => "SkillDialogueHandler",
            Self::Other => "Object",
        }
    }
}

/// The containers a player holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holder {
    Inventory,
    Equipment,
    /// `player.bank`: the secondary bank while `useSecondaryBank` is set.
    Bank,
    BankPrimary,
    BankSecondary,
    PohProxy,
}

/// An item definition as ContentAPI reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemDef<N> {
    pub name: Option<String>,
    pub stackable: bool,
    pub unnoted: bool,
    pub note_id: i32,
    /// `getConfiguration(EQUIP_SLOT, -1)`'s stored value, if any.
    pub equip_slot: Option<Value<N>>,
    /// `getConfiguration(RARE_ITEM, false)`'s stored value, if any.
    pub rare: Option<Value<N>>,
}

/// The pulses ContentAPI builds, with what they captured.
#[derive(Clone, Debug, PartialEq)]
pub enum PulseBody<N> {
    /// `poofClear`'s world pulse.
    PoofClear { npc: N, counter: i32 },
    /// `transformNpc`'s restore pulse.
    TransformRestore { npc: N },
    /// `runTask`'s pulse: `task(); return --cycles <= 0`.
    RunTask { cycles: i32, task: N },
    /// `runWorldTask`'s pulse: `task(); return true`.
    WorldTask { task: N },
    /// A content pulse handed to `submitWorldPulse`/`submitIndividualPulse`.
    Content(N),
}

/// A pulse and its delay.
#[derive(Clone, Debug, PartialEq)]
pub struct Pulse<N> {
    pub delay: i32,
    pub body: PulseBody<N>,
}

/// The queued script bodies ContentAPI builds.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptBody<N> {
    /// `sendChat(entity, message, delay)`'s SOFT script.
    SendChat { entity: N, message: String },
    /// `transformNpcWithHpCarryover`'s SOFT script.
    TransformHp { npc: N, ticks: i32 },
    /// `forceMove`'s SOFT script and its captured state.
    ForceMove(Box<world::ForceMove<N>>),
    /// A content script body (`queueScript`).
    Content(N),
}

/// A `QueuedScript` as `queueScript` builds it.
#[derive(Clone, Debug, PartialEq)]
pub struct QueuedScript<N> {
    pub body: ScriptBody<N>,
    pub strength: state::QueueStrength,
    pub persist: bool,
    pub next_execution: i32,
}

/// `sendSkillDialogue`'s anonymous SkillDialogueHandler with its builder.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillHandler<N> {
    pub player: N,
    pub items: Vec<N>,
    /// `builder.creationCallback` (`None`: the default `{_, _ -> }`).
    pub create: Option<N>,
    /// `builder.totalAmountCallback` (`None`: `amountInInventory`).
    pub max: Option<N>,
}

/// Where a pulse goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseTarget<'a, N> {
    /// `GameWorld.Pulser.submit(pulse)`.
    World,
    /// `entity.pulseManager.run(pulse)`.
    Entity(&'a N),
}

/// The services core/api calls, in the original's terms.
pub trait Host {
    /// A node or object handle (players, NPCs, items, scenery, containers,
    /// timers, scripts, pulses, content callbacks); `==` is identity.
    type Node: Clone + PartialEq + std::fmt::Debug;

    /// A recorded call of `member` with its arguments.
    fn call(&mut self, member: Member, args: Vec<Arg<Self::Node>>) -> Result<Reply<Self::Node>>;

    // ------------------------------------------------------------ objects ContentAPI builds
    /// `entity.pulseManager.run(pulse)` or `Pulser.submit(pulse)`: the
    /// pulse is stored and run by the world through [`state::run_pulse`].
    fn submit_pulse(
        &mut self,
        target: PulseTarget<'_, Self::Node>,
        pulse: Pulse<Self::Node>,
    ) -> Result<Self::Node>;
    /// `entity.scripts.addToQueue(script, strength)`.
    fn add_to_queue(&mut self, entity: &Self::Node, script: QueuedScript<Self::Node>)
    -> Result<()>;
    /// `SkillDialogueHandler.open()` of the handler `sendSkillDialogue` builds.
    fn open_skill_dialogue(
        &mut self,
        handler: SkillHandler<Self::Node>,
        kind: &'static str,
    ) -> Result<()>;
    /// `e.printStackTrace()` inside a catch.
    fn print_stack_trace(&mut self, error: &Error) -> Result<()>;
    /// `exceptionToString(Exception(message))`: the trace text (its frames
    /// are the platform's).
    fn exception_to_string(&mut self, message: &str) -> String;

    // ------------------------------------------------------------ field reads (not calls)
    fn kind(&self, node: &Self::Node) -> Kind;
    /// `node.getId()` (an item's `idHash >> 16 & 0xFFFF`).
    fn id(&self, node: &Self::Node) -> i32;
    /// `node.getName()`.
    fn name(&self, node: &Self::Node) -> Option<String>;
    fn location(&self, node: &Self::Node) -> Option<Location>;
    fn size(&self, node: &Self::Node) -> i32;
    fn item_amount(&self, item: &Self::Node) -> i32;
    /// An item's or a scenery's `charge`.
    fn item_charge(&self, node: &Self::Node) -> i32;
    /// A scenery's `rotation`.
    fn rotation(&self, scenery: &Self::Node) -> i32;
    fn item_slot(&self, item: &Self::Node) -> i32;
    /// `ItemDefinition.forId(id)`.
    fn item_definition(&self, id: i32) -> Option<ItemDef<Self::Node>>;
    /// `player.inventory` and the other containers.
    fn container(&self, player: &Self::Node, which: Holder) -> Self::Node;
    /// `(familiar as BurdenBeast).container`.
    fn burden_container(&self, beast: &Self::Node) -> Self::Node;
    /// `container is EquipmentContainer`.
    fn is_equipment_container(&self, container: &Self::Node) -> bool;
    fn is_male(&self, player: &Self::Node) -> bool;
    fn username(&self, player: &Self::Node) -> String;
    fn use_secondary_bank(&self, player: &Self::Node) -> bool;
    fn set_use_secondary_bank(&mut self, player: &Self::Node, value: bool);
    /// `entity.clocks[index]`.
    fn clock(&self, entity: &Self::Node, index: i32) -> Result<i32>;
    fn set_clock(&mut self, entity: &Self::Node, index: i32, value: i32) -> Result<()>;
    /// `GameWorld.ticks`.
    fn ticks(&self) -> i32;
    /// `GameWorld.settings?.isDevMode`.
    fn dev_mode(&self) -> Option<bool>;
    /// `ServerConstants.MAX_PATHFIND_DISTANCE`.
    fn max_pathfind_distance(&self) -> i32;
    /// A script's `persist`.
    fn script_persist(&self, script: &Self::Node) -> bool;
    fn script_state(&self, script: &Self::Node) -> i32;
    fn set_script_next_execution(&mut self, script: &Self::Node, value: i32);
    fn set_script_state(&mut self, script: &Self::Node, value: i32);
    /// `script::class.java` of a script, for `log`.
    fn script_class(&self, script: &Self::Node) -> String;
    /// The `Poison` timer's `severity`.
    fn poison_severity(&self, timer: &Self::Node) -> i32;
    /// `poison.damageSource = source` (a plain field write).
    fn set_poison_source(&mut self, timer: &Self::Node, source: &Self::Node);
    /// The class of a timer (`Poison`, `PoisonImmunity`, ...), for the
    /// reified casts.
    fn timer_class(&self, timer: &Self::Node) -> String;
    /// `player.logoutListeners[key] = handler` / `.remove(key)`.
    fn set_logout_listener(&mut self, player: &Self::Node, key: &str, handler: Option<Self::Node>);
    /// `Shops.shopsByNpc[npc]`.
    fn shop_by_npc(&self, npc: i32) -> Option<Self::Node>;
    /// `GrandExchangeRecords.offerRecords` (after `getInstance`'s call).
    fn offer_records(&self, player: &Self::Node) -> Result<Vec<Option<Self::Node>>>;
    /// `offer.withdraw[0]` is present.
    fn offer_has_withdraw(&self, offer: &Self::Node) -> bool;
    /// `ServerConfigParser.tomlData`.
    fn toml_data(&self) -> Option<Self::Node>;
    /// `Consumable.ids` of a consumable node, if it is a `Potion`.
    fn potion_ids(&self, consumable: &Self::Node) -> Option<Vec<i32>>;
    /// A consumable's identity hash (`HashMap<Consumable, Int>` keys).
    fn identity_hash(&self, node: &Self::Node) -> i32;
    /// `DiaryType.levelNames`.
    fn diary_level_names(&self, diary: &str) -> Vec<String>;
    /// Kotlin's `Random.Default.nextInt(bound)` (the thread's ThreadLocalRandom).
    fn kotlin_random_int(&mut self, bound: i32) -> i32;
    /// `MapArea.zoneMaps` (a JDK HashMap of zones by the area's name).
    fn zone_maps(&mut self) -> &mut Vec<(String, Self::Node)>;
    /// `Quests.toString()` (the quest's name).
    fn quest_name(&self, quest: &str) -> String;
    /// `Master.npc` of a slayer master constant.
    fn master_npc(&self, master: &str) -> i32;
}

pub(crate) fn unit<H: Host + ?Sized>(
    h: &mut H,
    member: Member,
    args: Vec<Arg<H::Node>>,
) -> Result<()> {
    h.call(member, args).map(|_| ())
}
pub(crate) fn b<H: Host + ?Sized>(
    h: &mut H,
    member: Member,
    args: Vec<Arg<H::Node>>,
) -> Result<bool> {
    h.call(member, args)?.bool()
}
pub(crate) fn i<H: Host + ?Sized>(
    h: &mut H,
    member: Member,
    args: Vec<Arg<H::Node>>,
) -> Result<i32> {
    h.call(member, args)?.int()
}
pub(crate) fn n<H: Host + ?Sized>(
    h: &mut H,
    member: Member,
    args: Vec<Arg<H::Node>>,
) -> Result<Option<H::Node>> {
    h.call(member, args)?.node()
}
pub(crate) fn ns<H: Host + ?Sized>(
    h: &mut H,
    member: Member,
    args: Vec<Arg<H::Node>>,
) -> Result<Vec<Option<H::Node>>> {
    h.call(member, args)?.nodes()
}
pub(crate) fn nd<N: Clone>(n: &N) -> Arg<N> {
    Arg::Node(n.clone())
}

/// `Item(id, amount, charge)`.
pub fn new_item<H: Host + ?Sized>(h: &mut H, id: i32, amount: i32, charge: i32) -> Result<H::Node> {
    h.call(
        Member::NewItem,
        vec![Arg::Int(id), Arg::Int(amount), Arg::Int(charge)],
    )?
    .node()?
    .ok_or(Error::Null)
}

/// `node.location` with Kotlin's non-null use.
pub(crate) fn loc_of<H: Host + ?Sized>(h: &H, n: &H::Node) -> Result<Location> {
    h.location(n).ok_or(Error::Null)
}

pub use api_files::{Container, EquipmentSlot, GameTab, God, InputType};
pub use world::ForceMove;
