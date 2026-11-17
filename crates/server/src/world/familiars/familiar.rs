//! `Familiar`, the base NPC class of every familiar and pet: its fields,
//! constructor, init/spawn, the tick (points drain, special points, timer
//! varbits, warnings, assisting the owner in combat, following and
//! recalling), the attack rules both ways, calling (teleporting to the
//! owner), dismissing, special moves and the familiar hit pulse.
//!
//! A familiar object is shared: clones are the same object, and
//! [`Familiar::same`] is the original's `==`. Every field read takes the
//! lock briefly and every service call happens outside it, so a service may
//! reenter the object (a content `specialMove` that dismisses it, say).
use super::burden::{Burden, ForagerState};
use super::pet::PetState;
use super::pets::Pet;
use super::{
    CombatStyle, Details, Error, FamiliarSpecial, Host, INFINITE_SPECIAL_MOVE_ATTRIBUTE, Item, Loc,
    NodeKind, Pouch, Pulse, Result, SLAYER, STYLE_DEFENSIVE, SUMMONING, SkillBonus, distance_of,
};
use std::sync::{Arc, Mutex, MutexGuard};

/// `SMALL_SUMMON_GRAPHIC`, `LARGE_SUMMON_GRAPHIC`, `SPECIAL_ANIMATION`,
/// `SPECIAL_GRAPHIC`.
pub const SMALL_SUMMON_GRAPHIC: i32 = 1314;
pub const LARGE_SUMMON_GRAPHIC: i32 = 1315;
pub const SPECIAL_ANIMATION: i32 = 7660;
pub const SPECIAL_GRAPHIC: i32 = 1316;
/// `Sounds.SUMMON_NPC_188`, `Sounds.SPELL_4161`.
pub const SUMMON_SOUND: i32 = 188;
pub const SPECIAL_SOUND: i32 = 4161;

/// The subclass part of a familiar object.
pub enum Kind<H: Host> {
    /// A familiar class extending `Familiar` directly.
    Plain,
    /// `BurdenBeast` (and `Forager`, its subclass).
    Burden(Burden<H>),
    /// `Pet`.
    Pet(PetState),
}

/// The base constructor's arguments: `Familiar(owner, id, ticks, pouchId,
/// specialCost[, attackStyle])` (the five-argument form passes
/// `STYLE_DEFENSIVE`).
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub id: i32,
    pub ticks: i32,
    pub pouch_id: i32,
    pub special_cost: i32,
    pub attack_style: i32,
}
impl Spec {
    pub fn new(id: i32, ticks: i32, pouch_id: i32, special_cost: i32) -> Self {
        Self {
            id,
            ticks,
            pouch_id,
            special_cost,
            attack_style: STYLE_DEFENSIVE,
        }
    }
}

/// Which constructor chain a content class runs.
#[derive(Clone, Debug)]
pub enum Shape {
    /// `Familiar(...)`.
    Plain,
    /// `BurdenBeast(..., containerSize[, attackStyle])`.
    Burden { container_size: i32 },
    /// `Forager(..., [attackStyle,] items...)`: a beast of burden of 30.
    Forager { items: Vec<Item> },
}

/// The fields of one familiar object.
pub struct State<H: Host> {
    pub class: H::Class,
    /// The NPC this object is.
    pub npc: H::Node,
    pub owner: Option<H::Node>,
    pub ticks: i32,
    pub maximum_ticks: i32,
    pub special_points: i32,
    pub pouch_id: i32,
    pub special_cost: i32,
    /// The `SummoningPouch` the constructor looked up.
    pub pouch: Option<Pouch>,
    pub combat_handler: Option<H::Handler>,
    pub combat_familiar: bool,
    pub charged: bool,
    pub boosts: Vec<SkillBonus>,
    pub attack_style: i32,
    pub points_per_tick: f64,
    pub frac_drain: f64,
    pub first_call: bool,
    pub kind: Kind<H>,
}

/// One familiar object.
pub struct Familiar<H: Host>(Arc<Mutex<State<H>>>);

impl<H: Host> Clone for Familiar<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> std::fmt::Debug for Familiar<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Familiar({:?})", self.npc())
    }
}

/// `sendFamiliarHit`'s anonymous pulse: its captures.
pub struct HitPulse<H: Host> {
    pub familiar: Familiar<H>,
    pub target: H::Node,
    pub max_hit: i32,
    pub graphics: Option<i32>,
}
impl<H: Host> Clone for HitPulse<H> {
    fn clone(&self) -> Self {
        Self {
            familiar: self.familiar.clone(),
            target: self.target.clone(),
            max_hit: self.max_hit,
            graphics: self.graphics,
        }
    }
}
impl<H: Host> std::fmt::Debug for HitPulse<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HitPulse({:?} -> {:?})", self.familiar, self.target)
    }
}
impl<H: Host> Clone for Pulse<H> {
    fn clone(&self) -> Self {
        match self {
            Self::Follow => Self::Follow,
            Self::Hit(hit) => Self::Hit(hit.clone()),
        }
    }
}
impl<H: Host> std::fmt::Debug for Pulse<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Follow => write!(f, "Follow"),
            Self::Hit(hit) => hit.fmt(f),
        }
    }
}

impl<H: Host> HitPulse<H> {
    /// The pulse body.
    pub fn pulse(&self, host: &mut H) -> Result<bool> {
        let npc = self.familiar.npc();
        let target = &self.target;
        let mut state = host.battle_state(&npc, target)?;
        let mut hit = 0;
        let style = self.familiar.combat_style();
        if host.accurate_impact(style, &npc, target)? {
            hit = host.randomize(self.max_hit)?;
        }
        host.set_estimated_hit(&mut state, hit)?;
        let owner = self.familiar.owner_or_null()?;
        if self.familiar.task_blocks(host, target, &owner)? {
            host.neutralize_hits(&mut state)?;
            host.set_poison_damage(&mut state, -1)?;
            if host.state_style(&mut state)? == Some(CombatStyle::Magic) {
                host.set_estimated_hit(&mut state, -1)?;
            }
        }
        host.handle_impact(target, &owner, hit, CombatStyle::Melee, &mut state)?;
        if let Some(graphics) = self.graphics {
            host.graphics(target, graphics)?;
        }
        Ok(true)
    }
}

impl<H: Host> Familiar<H> {
    fn lock(&self) -> MutexGuard<'_, State<H>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Reads or writes the fields; never call a service inside `f`.
    pub fn with<R>(&self, f: impl FnOnce(&mut State<H>) -> R) -> R {
        f(&mut self.lock())
    }
    /// The original's `==` on two familiar objects.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The base constructor, then the subclass constructor `shape` names.
    /// `super(id, null)` makes the NPC; then the field initialisers, the
    /// owner and timer, `SummoningPouch.get(pouchId)`, the special cost,
    /// `NPCDefinition.forId(getOriginalId() + 1).getName().equals(getName())`
    /// and the drain per tick (`(level - cost + 1) / ticks`, 0 without a
    /// pouch id; a missing pouch of another id throws).
    pub fn new(
        host: &mut H,
        class: H::Class,
        owner: Option<H::Node>,
        spec: Spec,
        shape: Shape,
    ) -> Result<Self> {
        let (npc, pouch, combat_familiar, points_per_tick) =
            Self::base(host, owner.as_ref(), spec)?;
        let forager = matches!(shape, Shape::Forager { .. });
        let kind = match shape {
            Shape::Plain => Kind::Plain,
            Shape::Burden { container_size } => Kind::Burden(Burden::construct(
                host,
                owner.as_ref(),
                container_size,
                None,
            )?),
            Shape::Forager { items } => {
                Kind::Burden(Burden::construct(host, owner.as_ref(), 30, Some(items))?)
            }
        };
        let familiar = Self::assemble(
            class,
            npc,
            owner,
            spec,
            pouch,
            combat_familiar,
            points_per_tick,
            kind,
        );
        if forager {
            // Forager(...): the items are set, then setRandomPassive().
            familiar.set_random_passive(host)?;
        }
        Ok(familiar)
    }

    /// `new Pet(owner, details, itemId, id)`: the base constructor with
    /// ticks, pouch id and special cost -1, then `Pets.forId(itemId)` (a
    /// missing pet throws on its growth rate).
    pub fn new_pet(
        host: &mut H,
        class: H::Class,
        owner: Option<H::Node>,
        details: Details,
        item_id: i32,
        id: i32,
    ) -> Result<Self> {
        let spec = Spec::new(id, -1, -1, -1);
        let (npc, pouch, combat_familiar, points_per_tick) =
            Self::base(host, owner.as_ref(), spec)?;
        let pet = Pet::for_id(item_id).ok_or(Error::Null)?;
        let kind = Kind::Pet(PetState {
            item_id,
            details,
            growth_rate: pet.growth_rate,
            pet,
            has_warned: 0,
        });
        Ok(Self::assemble(
            class,
            npc,
            owner,
            spec,
            pouch,
            combat_familiar,
            points_per_tick,
            kind,
        ))
    }

    #[allow(clippy::type_complexity)]
    fn base(
        host: &mut H,
        _owner: Option<&H::Node>,
        spec: Spec,
    ) -> Result<(H::Node, Option<Pouch>, bool, f64)> {
        let npc = host.new_npc(spec.id)?;
        let pouch = host.pouch(spec.pouch_id)?;
        let original = host.original_id(&npc)?;
        let definition = host.npc_definition_name(original.wrapping_add(1))?;
        let own = host.name(&npc)?;
        // String.equals: a null definition name throws, a null own name is unequal.
        let combat_familiar = match definition {
            None => return Err(Error::Null),
            Some(name) => own.is_some_and(|own| own == name),
        };
        let points_per_tick = if spec.pouch_id == -1 {
            0.0
        } else {
            let pouch = pouch.ok_or(Error::Null)?;
            let drain = pouch
                .level_required
                .wrapping_sub(pouch.summon_cost)
                .wrapping_add(1);
            drain as f64 / spec.ticks as f64
        };
        Ok((npc, pouch, combat_familiar, points_per_tick))
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        class: H::Class,
        npc: H::Node,
        owner: Option<H::Node>,
        spec: Spec,
        pouch: Option<Pouch>,
        combat_familiar: bool,
        points_per_tick: f64,
        kind: Kind<H>,
    ) -> Self {
        Self(Arc::new(Mutex::new(State {
            class,
            npc,
            owner,
            ticks: spec.ticks,
            maximum_ticks: spec.ticks,
            special_points: 60,
            pouch_id: spec.pouch_id,
            special_cost: spec.special_cost,
            pouch,
            combat_handler: None,
            combat_familiar,
            charged: false,
            boosts: Vec::with_capacity(20),
            attack_style: spec.attack_style,
            points_per_tick,
            frac_drain: 0.0,
            first_call: true,
            kind,
        })))
    }

    // ------------------------------------------------------------ fields
    pub fn npc(&self) -> H::Node {
        self.lock().npc.clone()
    }
    pub fn class(&self) -> H::Class {
        self.lock().class.clone()
    }
    /// `getOwner()`.
    pub fn owner(&self) -> Option<H::Node> {
        self.lock().owner.clone()
    }
    pub(crate) fn owner_or_null(&self) -> Result<H::Node> {
        self.owner().ok_or(Error::Null)
    }
    /// `setOwner(owner)`.
    pub fn set_owner(&self, owner: Option<H::Node>) {
        self.lock().owner = owner;
    }
    /// `getTicks()`.
    pub fn ticks(&self) -> i32 {
        self.lock().ticks
    }
    /// The `ticks` field written directly (`FamiliarManager.parse`).
    pub fn set_ticks(&self, ticks: i32) {
        self.lock().ticks = ticks;
    }
    pub fn maximum_ticks(&self) -> i32 {
        self.lock().maximum_ticks
    }
    /// `getSpecialPoints()`.
    pub fn special_points(&self) -> i32 {
        self.lock().special_points
    }
    /// The `specialPoints` field written directly.
    pub fn set_special_points(&self, points: i32) {
        self.lock().special_points = points;
    }
    /// `getPouchId()`.
    pub fn pouch_id(&self) -> i32 {
        self.lock().pouch_id
    }
    pub fn special_cost(&self) -> i32 {
        self.lock().special_cost
    }
    /// `getAttackStyle()`.
    pub fn attack_style(&self) -> i32 {
        self.lock().attack_style
    }
    /// `getCombatHandler()`.
    pub fn combat_handler(&self) -> Option<H::Handler> {
        self.lock().combat_handler.clone()
    }
    /// `setCombatHandler(handler)`.
    pub fn set_combat_handler(&self, handler: Option<H::Handler>) {
        self.lock().combat_handler = handler;
    }
    /// `setCharged(charged)`.
    pub fn set_charged(&self, charged: bool) {
        self.lock().charged = charged;
    }
    /// `charge()`.
    pub fn charge(&self) {
        self.set_charged(true);
    }
    /// The `charged` field.
    pub fn charged(&self) -> bool {
        self.lock().charged
    }
    /// `boosts.add(bonus)` (the list is the content classes').
    pub fn add_boost(&self, bonus: SkillBonus) {
        self.lock().boosts.push(bonus);
    }
    pub fn points_per_tick(&self) -> f64 {
        self.lock().points_per_tick
    }
    pub fn frac_drain(&self) -> f64 {
        self.lock().frac_drain
    }
    pub fn first_call(&self) -> bool {
        self.lock().first_call
    }
    pub fn combat_familiar_field(&self) -> bool {
        self.lock().combat_familiar
    }
    /// `this instanceof Pet`.
    pub fn is_pet(&self) -> bool {
        matches!(self.lock().kind, Kind::Pet(_))
    }
    /// `this instanceof Forager`.
    pub fn is_forager(&self) -> bool {
        matches!(
            self.lock().kind,
            Kind::Burden(Burden {
                forager: Some(_),
                ..
            })
        )
    }
    /// `isBurdenBeast()`.
    pub fn is_burden_beast(&self) -> bool {
        matches!(self.lock().kind, Kind::Burden(_))
    }
    /// `isPoisonImmune()` where the class overrides it (a beast of burden).
    pub fn poison_immune_override(&self) -> Option<bool> {
        self.is_burden_beast().then_some(true)
    }
    /// The beast of burden's container (`getContainer()`), or a cast failure.
    pub fn container(&self) -> Result<H::Container> {
        match &self.lock().kind {
            Kind::Burden(burden) => Ok(burden.container.clone()),
            _ => Err(Error::Cast),
        }
    }
    pub(crate) fn forager(&self) -> Option<ForagerState> {
        match &self.lock().kind {
            Kind::Burden(Burden {
                forager: Some(f), ..
            }) => Some(f.clone()),
            _ => None,
        }
    }
    pub(crate) fn pet_state(&self) -> Option<PetState> {
        match &self.lock().kind {
            Kind::Pet(p) => Some(p.clone()),
            _ => None,
        }
    }
    pub(crate) fn pet_set_has_warned(&self, value: i32) {
        if let Kind::Pet(p) = &mut self.lock().kind {
            p.has_warned = value;
        }
    }

    // ------------------------------------------------------------ init and tick
    /// `init(loc, call)`.
    pub fn init_at(&self, host: &mut H, loc: Option<Loc>, call: bool) -> Result<()> {
        let npc = self.npc();
        host.set_location(&npc, loc)?;
        if host.location(&npc)?.is_none() {
            let owner = self.owner_or_null()?;
            let at = host.location(&owner)?;
            host.set_location(&npc, at)?;
            host.set_invisible(&npc, true)?;
        }
        host.npc_init(&npc)?;
        self.start_following(host)?;
        self.send_configuration(host)?;
        if call {
            self.call(host)?;
        }
        let owner = self.owner_or_null()?;
        host.open_info_bars(&owner)?;
        if host.in_zone(&npc, "Wilderness")? {
            self.transform(host)?;
        }
        Ok(())
    }

    /// `init()`: `init(getSpawnLocation(), true)`.
    pub fn init(&self, host: &mut H) -> Result<()> {
        let loc = self.spawn_location(host)?;
        self.init_at(host, loc, true)
    }

    /// `handleTickActions()`, the class's (a pet's is its own).
    pub fn handle_tick_actions(&self, host: &mut H) -> Result<()> {
        if self.is_pet() {
            return self.pet_tick_actions(host);
        }
        let owner = self.owner_or_null()?;
        let drain = self.with(|s| {
            s.ticks = s.ticks.wrapping_sub(1);
            s.frac_drain += s.points_per_tick;
            if s.frac_drain > 1.0 && s.ticks > 0 {
                s.frac_drain -= 1.0;
                true
            } else {
                false
            }
        });
        if drain {
            host.update_level(&owner, SUMMONING, -1, 0)?;
        }
        if self.ticks() % 50 == 0 {
            self.update_special_points(host, -15)?;
            let text = host.text(self)?;
            if !text.is_empty() {
                let text = host.text(self)?;
                host.send_chat(&self.npc(), &text)?;
            }
        }
        self.send_time_remaining(host)?;
        match self.ticks() {
            100 => host.packet_message(
                &owner,
                "<col=ff0000>You have 1 minute before your familiar vanishes.",
            )?,
            50 => host.packet_message(
                &owner,
                "<col=ff0000>You have 30 seconds before your familiar vanishes.",
            )?,
            0 => {
                let carrying = if self.is_burden_beast() {
                    let container = self.container()?;
                    !host.is_empty(&container)?
                } else {
                    false
                };
                if carrying {
                    host.packet_message(
                        &owner,
                        "<col=ff0000>Your familiar has dropped all the items it was holding.",
                    )?;
                } else {
                    host.packet_message(&owner, "<col=ff0000>Your familiar has vanished.")?;
                }
                return self.dismiss(host);
            }
            _ => {}
        }
        self.assist_owner(host, &owner)?;
        self.follow_or_recall(host, &owner)?;
        self.handle_familiar_tick(host)
    }

    /// The combat block of `handleTickActions`.
    fn assist_owner(&self, host: &mut H, owner: &H::Node) -> Result<()> {
        let npc = self.npc();
        if host.is_invisible(&npc)? || host.combat_attacking(&npc)? {
            return Ok(());
        }
        if !(host.combat_attacking(owner)? || host.in_combat(owner)?) {
            return Ok(());
        }
        let mut victim = host.combat_victim(owner)?;
        if victim.is_none() {
            victim = host.attribute_node(owner, "combat-attacker")?;
        }
        if host.combat_victim(owner)?.as_ref() == Some(&npc) {
            return Ok(());
        }
        let Some(victim) = victim else {
            return Ok(());
        };
        if !host.is_invisible(&victim)?
            && host.multi_zone(&npc)?
            && host.multi_zone(owner)?
            && host.multi_zone(&victim)?
            && self.is_combat_familiar()
            && !self.is_burden_beast()
            && !self.is_peaceful_familiar()?
        {
            host.combat_attack(&npc, &victim)?;
        }
        Ok(())
    }

    /// The last block of the tick: recall a familiar that is far away or
    /// hidden every 25 ticks, or follow again.
    pub(crate) fn follow_or_recall(&self, host: &mut H, owner: &H::Node) -> Result<()> {
        let npc = self.npc();
        let far = if !host.is_invisible(&npc)? {
            let a = host.location(owner)?;
            let b = host.location(&npc)?;
            distance_of(a, b)? > 12.0
        } else {
            false
        };
        let recall = far || (host.is_invisible(&npc)? && self.ticks() % 25 == 0);
        if recall {
            if !self.call(host)? {
                host.set_invisible(&npc, true)?;
            }
        } else if !host.has_pulse_running(&npc)? {
            self.start_following(host)?;
        }
        Ok(())
    }

    /// `handleFamiliarTick()`: the content class's body, then a forager's.
    pub fn handle_familiar_tick(&self, host: &mut H) -> Result<()> {
        host.familiar_tick(self)?;
        if self.forager().is_some() {
            self.forager_tick(host)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------ attack rules
    /// `isAttackable(entity, style, message)`.
    pub fn is_attackable(
        &self,
        host: &mut H,
        entity: &H::Node,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        if self.is_attackable_by(host, entity, style, message)? {
            return Ok(false);
        }
        host.npc_is_attackable(&self.npc(), entity, style, message)
    }

    /// `continueAttack(target, style, message)`.
    pub fn continue_attack(
        &self,
        host: &mut H,
        target: &H::Node,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        if host.combat_victim(&self.npc())?.as_ref() == Some(target) {
            return self.can_attack_target(host, target, style, message);
        }
        if Some(target) == self.owner().as_ref() {
            if message {
                host.api_message(
                    &self.owner_or_null()?,
                    "You can't just betray your own familiar like that!",
                )?;
            }
            return Ok(false);
        }
        Ok(true)
    }

    fn can_attack_target(
        &self,
        host: &mut H,
        target: &H::Node,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        let owner = self.owner();
        if Some(target) == owner.as_ref() {
            if message {
                host.api_message(target, "You can't just betray your own familiar like that!")?;
            }
            return Ok(false);
        }
        if host.kind(target)? == NodeKind::Player {
            if !host.skull_wilderness(target)? {
                if message {
                    let owner = owner.ok_or(Error::Null)?;
                    host.api_message(
                        &owner,
                        "You have to be in the wilderness to attack a player's familiar.",
                    )?;
                }
                return Ok(false);
            }
            let owner = owner.ok_or(Error::Null)?;
            if !host.skull_wilderness(&owner)? {
                if message {
                    host.api_message(&owner, "This familiar's owner is not in the wilderness.")?;
                }
                return Ok(false);
            }
            if !host.is_attackable(&owner, target, style, false)? {
                return Ok(false);
            }
            if !host.multi_zone(&owner)? {
                if message {
                    host.api_message(
                        &owner,
                        "You have to be in a multicombat zone to attack another player with your familiar.",
                    )?;
                }
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `isAttackableBy`: true when the attacker may not attack this familiar.
    fn is_attackable_by(
        &self,
        host: &mut H,
        attacker: &H::Node,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        let owner = self.owner();
        if Some(attacker) == owner.as_ref() {
            if message {
                host.api_message(
                    attacker,
                    "You can't just betray your own familiar like that!",
                )?;
            }
            return Ok(true);
        }
        if host.kind(attacker)? == NodeKind::Player {
            if !host.skull_wilderness(attacker)? {
                if message {
                    host.api_message(
                        attacker,
                        "You have to be in the wilderness to attack a player's familiar.",
                    )?;
                }
                return Ok(true);
            }
            let owner = owner.ok_or(Error::Null)?;
            if !host.skull_wilderness(&owner)? {
                if message {
                    host.api_message(attacker, "This familiar's owner is not in the wilderness.")?;
                }
                return Ok(true);
            }
            if !host.is_attackable(&owner, attacker, style, false)? {
                return Ok(true);
            }
            if !host.multi_zone(attacker)? {
                if message {
                    host.api_message(
                        attacker,
                        "You have to be in a multicombat zone to attack a player's familiar.",
                    )?;
                }
                return Ok(true);
            }
            if !host.multi_zone(&self.npc())? {
                if message {
                    host.api_message(attacker, "This familiar is not in a multicombat zone.")?;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `onRegionInactivity()`.
    pub fn on_region_inactivity(&self, host: &mut H) -> Result<()> {
        if self.call(host)? {
            host.walking_update(&self.npc())?;
        }
        Ok(())
    }

    /// `getSwingHandler(swing)`.
    pub fn swing_handler(&self, host: &mut H, swing: bool) -> Result<H::Handler> {
        if let Some(handler) = self.combat_handler() {
            return Ok(handler);
        }
        host.npc_swing_handler(&self.npc(), swing)
    }

    /// `transform()`: the Wilderness combat form, the next NPC id.
    pub fn transform(&self, host: &mut H) -> Result<()> {
        if self.is_combat_familiar() {
            let npc = self.npc();
            let original = host.original_id(&npc)?;
            host.npc_transform(&npc, original.wrapping_add(1))?;
        }
        Ok(())
    }

    /// `refreshTimer()`.
    pub fn refresh_timer(&self) {
        self.with(|s| s.ticks = s.maximum_ticks);
    }

    /// `sendTimeRemaining()`: varbit 4534 minutes, 4290 the half minute.
    fn send_time_remaining(&self, host: &mut H) -> Result<()> {
        let ticks = self.ticks();
        let minutes = ticks / 100;
        let centiminutes = ticks % 100;
        let owner = self.owner_or_null()?;
        host.set_varbit(&owner, 4534, minutes)?;
        host.set_varbit(&owner, 4290, if centiminutes > 49 { 1 } else { 0 })
    }

    /// `executeSpecialMove(special)`.
    pub fn execute_special_move(&self, host: &mut H, special: &FamiliarSpecial<H>) -> Result<bool> {
        let npc = self.npc();
        if special.node.as_ref() == Some(&npc) {
            return Ok(false);
        }
        let owner = self.owner_or_null()?;
        if self.special_cost() > self.special_points() {
            host.packet_message(
                &owner,
                "Your familiar does not have enough special move points left.",
            )?;
            return Ok(false);
        }
        let pouch_id = self.pouch_id();
        let Some(scroll) = host.scroll(pouch_id)? else {
            host.packet_message(
                &owner,
                &format!("Invalid scroll for pouch {pouch_id} - report!"),
            )?;
            return Ok(false);
        };
        let inventory = host.inventory(&owner)?;
        if !host.contains(&inventory, scroll.item_id, 1)? {
            host.packet_message(
                &owner,
                "You do not have enough scrolls left to do this special move.",
            )?;
            return Ok(false);
        }
        let a = host.location(&owner)?;
        let b = host.location(&npc)?;
        if distance_of(a, b)? > 15.0 {
            host.packet_message(
                &owner,
                "Your familiar is too far away to use that scroll, or it cannot see you.",
            )?;
            return Ok(false);
        }
        if host.special_move(self, special)? {
            let delay = host.ticks()?.wrapping_add(3);
            host.set_attribute_int(&npc, "special-delay", delay)?;
            let inventory = host.inventory(&owner)?;
            host.remove(&inventory, &Item::one(scroll.item_id), None)?;
            host.play_audio(&owner, SPECIAL_SOUND)?;
            self.visualize_special_move(host)?;
            if !host.attribute_bool(&owner, INFINITE_SPECIAL_MOVE_ATTRIBUTE, false)? {
                let cost = self.special_cost();
                self.update_special_points(host, cost)?;
            }
            host.add_experience(&owner, SUMMONING, scroll.experience, Some(true))?;
        }
        Ok(true)
    }

    /// `visualizeSpecialMove()`.
    pub fn visualize_special_move(&self, host: &mut H) -> Result<()> {
        host.visualize(&self.owner_or_null()?, SPECIAL_ANIMATION, SPECIAL_GRAPHIC)
    }

    /// `sendFamiliarHit(target, maxHit[, graphics])`.
    pub fn send_familiar_hit(
        &self,
        host: &mut H,
        target: &H::Node,
        max_hit: i32,
        graphics: Option<i32>,
    ) -> Result<()> {
        let npc = self.npc();
        let a = host.location(&npc)?;
        let b = host.location(target)?;
        let ticks = 2 + (distance_of(a, b)? * 0.5).floor() as i32;
        host.combat_next_attack(&npc, 4)?;
        let pulse = Pulse::Hit(HitPulse {
            familiar: self.clone(),
            target: target.clone(),
            max_hit,
            graphics,
        });
        host.submit_pulse(ticks, &[npc, target.clone()], pulse)
    }

    /// `projectile(target, projectileId)`.
    pub fn projectile(&self, host: &mut H, target: &H::Node, projectile: i32) -> Result<()> {
        host.projectile(&self.npc(), target, projectile, 40, 36, 51, 10)
    }

    /// The slayer task test of the hit pulse and `canCombatSpecial`:
    /// `target instanceof NPC && getTask() != null && getTask().levelReq >
    /// owner's Slayer level` (`getTask()` read twice).
    fn task_blocks(&self, host: &mut H, target: &H::Node, owner: &H::Node) -> Result<bool> {
        if host.kind(target)? != NodeKind::Npc {
            return Ok(false);
        }
        if host.npc_task(target)?.is_none() {
            return Ok(false);
        }
        let level = host.npc_task(target)?.ok_or(Error::Null)?;
        Ok(level > host.skill_level(owner, SLAYER)?)
    }

    /// `canAttack(target, message)`.
    pub fn can_attack_with(&self, host: &mut H, target: &H::Node, message: bool) -> Result<bool> {
        let owner = self.owner_or_null()?;
        let style = host.combat_style(&owner)?;
        if !host.is_attackable(target, &owner, style, true)? {
            return Ok(false);
        }
        let npc = self.npc();
        let a = host.location(target)?;
        let b = host.location(&npc)?;
        if distance_of(a, b)? > 8.0 {
            if message {
                host.api_message(&owner, "That target is too far.")?;
            }
            return Ok(false);
        }
        if host.kind(target)? == NodeKind::Player && !host.multi_zone(&owner)? {
            if message {
                host.api_message(
                    &owner,
                    "You have to be in a multicombat zone to attack another player with your familiar.",
                )?;
            }
            return Ok(false);
        }
        if host.interaction_locked(target)?
            || !host.is_attackable(target, &npc, CombatStyle::Magic, message)?
        {
            return Ok(false);
        }
        Ok(self.is_combat_familiar())
    }

    /// `canAttack(target)`: `canAttack(target, true)`.
    pub fn can_attack(&self, host: &mut H, target: &H::Node) -> Result<bool> {
        self.can_attack_with(host, target, true)
    }

    /// `canCombatSpecial(target, message)`.
    pub fn can_combat_special(
        &self,
        host: &mut H,
        target: &H::Node,
        message: bool,
    ) -> Result<bool> {
        if !self.can_attack_with(host, target, message)? {
            return Ok(false);
        }
        if !self.is_owner_attackable(host)? {
            return Ok(false);
        }
        let npc = self.npc();
        if host.attribute_int(&npc, "special-delay", 0)? > host.ticks()? {
            return Ok(false);
        }
        let owner = self.owner_or_null()?;
        if self.task_blocks(host, target, &owner)? {
            return Ok(false);
        }
        Ok(true)
    }

    /// `isOwnerAttackable()`.
    pub fn is_owner_attackable(&self, host: &mut H) -> Result<bool> {
        let owner = self.owner_or_null()?;
        if !host.combat_attacking(&owner)?
            && !host.in_combat(&owner)?
            && !host.combat_attacking(&self.npc())?
        {
            host.packet_message(
                &owner,
                "Your familiar cannot fight whilst you are not in combat.",
            )?;
            return Ok(false);
        }
        Ok(true)
    }

    /// `getCombatStyle()` (MAGIC; no content class of the corpus overrides it).
    pub fn combat_style(&self) -> CombatStyle {
        CombatStyle::Magic
    }

    /// `startFollowing()`: the movement pulse, then `face(owner)`.
    pub fn start_following(&self, host: &mut H) -> Result<()> {
        let npc = self.npc();
        let owner = self.owner();
        host.pulse_run(&npc, Pulse::Follow, &npc, owner.as_ref())?;
        host.face(&npc, owner.as_ref())
    }

    /// `finalizeDeath(killer)`.
    pub fn finalize_death(&self, host: &mut H) -> Result<()> {
        self.dismiss(host)
    }

    /// `isCombatFamiliar()` (a pet's answers false).
    pub fn is_combat_familiar(&self) -> bool {
        let s = self.lock();
        !matches!(s.kind, Kind::Pet(_)) && s.combat_familiar
    }

    /// `sendConfiguration()` (a pet's is its own).
    pub fn send_configuration(&self, host: &mut H) -> Result<()> {
        if self.is_pet() {
            return self.pet_send_configuration(host);
        }
        let owner = self.owner_or_null()?;
        host.set_varp(&owner, 448, self.pouch_id())?;
        let original = host.original_id(&self.npc())?;
        host.set_varp(&owner, 1174, original)?;
        host.set_varp(&owner, 1175, self.special_cost().wrapping_shl(23))?;
        self.send_time_remaining(host)?;
        self.update_special_points(host, 0)
    }

    /// `call()`: teleport next to the owner (hidden in a follower-free
    /// zone); false when there is no room.
    pub fn call(&self, host: &mut H) -> Result<bool> {
        let Some(destination) = self.spawn_location(host)? else {
            return Ok(false);
        };
        let owner = self.owner_or_null()?;
        let npc = self.npc();
        let invisible =
            host.restricted_followers(&owner)? && !host.is_locked(&owner, "enable_summoning")?;
        host.set_invisible(&npc, invisible)?;
        if host.is_invisible(&npc)? {
            return Ok(true);
        }
        host.set_teleport_location(&npc, destination)?;
        if !self.is_pet() {
            // Both branches play the same sound; the first call then clears firstCall.
            let first = self.first_call();
            host.play_audio(&owner, SUMMON_SOUND)?;
            if first {
                self.with(|s| s.first_call = false);
            }
            if host.size(&npc)? > 1 {
                host.graphics(&npc, LARGE_SUMMON_GRAPHIC)?;
            } else {
                host.graphics(&npc, SMALL_SUMMON_GRAPHIC)?;
            }
        }
        if host.combat_attacking(&npc)? {
            self.start_following(host)?;
        } else {
            host.face(&npc, Some(&owner))?;
        }
        if !host.is_renderable(&npc)? && host.is_active(&owner)? {
            host.walking_update(&npc)?;
            host.masks_prepare(&npc)?;
        }
        Ok(true)
    }

    /// `getSpawnLocation()`.
    pub fn spawn_location(&self, host: &mut H) -> Result<Option<Loc>> {
        let owner = self.owner_or_null()?;
        host.spawn_location(&owner, &self.npc())
    }

    /// `dismiss()` (a beast of burden drops its items first).
    pub fn dismiss(&self, host: &mut H) -> Result<()> {
        if self.is_burden_beast() {
            self.burden_dismiss(host)?;
        }
        let npc = self.npc();
        host.npc_clear(&npc)?;
        host.pulse_clear(&npc)?;
        let owner = self.owner_or_null()?;
        host.remove_tabs(&owner, &[7])?;
        host.manager(&owner)?.set_familiar(None);
        host.set_varp(&owner, 448, -1)?;
        host.set_varp(&owner, 1176, 0)?;
        host.set_varp(&owner, 1175, 182986)?;
        host.set_varp(&owner, 1174, -1)?;
        host.appearance_sync(&owner)?;
        host.set_viewed_tab(&owner, 3)
    }

    /// `updateSpecialPoints(diff)`: subtract, cap at 60, varp 1177.
    pub fn update_special_points(&self, host: &mut H, diff: i32) -> Result<()> {
        let points = self.with(|s| {
            s.special_points = s.special_points.wrapping_sub(diff);
            if s.special_points > 60 {
                s.special_points = 60;
            }
            s.special_points
        });
        host.set_varp(&self.owner_or_null()?, 1177, points)
    }

    /// `isCharged()`.
    pub fn is_charged(&self, host: &mut H) -> Result<bool> {
        if self.charged() {
            host.packet_message(
                &self.owner_or_null()?,
                "Your familiar is already charging its attack!",
            )?;
            return Ok(true);
        }
        Ok(false)
    }

    /// `getBoost(skill)`: the first boost of the skill, truncated.
    pub fn get_boost(&self, skill: i32) -> i32 {
        self.lock()
            .boosts
            .iter()
            .find(|b| b.skill == skill)
            .map_or(0, |b| b.bonus as i32)
    }

    /// `isPeacefulFamiliar()`: the pouch's flag (no pouch throws).
    pub fn is_peaceful_familiar(&self) -> Result<bool> {
        Ok(self.lock().pouch.ok_or(Error::Null)?.peaceful)
    }
}
