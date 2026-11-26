//! Eating and drinking in the live world (`world::consumables`,
//! docs/consumables.md): ConsumableListener's `on(IntType.ITEM, "eat",
//! "drink")` in the native InteractionListeners and `handleConsumable` with
//! [`Eating`] as its `Host` for one player: the native Skills (taken out of
//! the profile for the call, as the engine timers do) and Settings, the
//! player's clocks, the canonical inventory with its listeners, the animator,
//! the engine timers (`consumables::join`), the combat pulse and the impact
//! handler. A potion's dose message is a SOFT script kept by the world and
//! run at the start of the player's `Player.tick` once due (as DropListener's
//! is).
use super::{P, host::Live, inner::placeholder_skills, now_millis, timers::Ctx};
use crate::{
    definitions::item::Shared,
    inventory::item::{Handle as ItemHandle, Item as NativeItem},
    player::{attributes::AttributeValue, settings::Settings, skills::Skills},
    timers::Handle as Timer,
    world::{
        combat_pulse::HitsplatType,
        consumables::{
            self as c, Attr, ContainerKind, DoseScript, Error, Item, Log, Node, QueueStrength,
            Result, RumPulse, Table, TimerArg, join,
        },
    },
};
use rs09_protocol::game::updates;

/// The consumables' state in the live world.
pub struct EatWorld {
    /// The `Consumables` table.
    pub table: std::sync::Arc<Table>,
    /// The potions' dose scripts: due tick, player node, script.
    pub doses: Vec<(i32, usize, DoseScript)>,
}
impl Default for EatWorld {
    fn default() -> Self {
        Self {
            table: std::sync::Arc::new(Table::new()),
            doses: Vec::new(),
        }
    }
}

/// ConsumableListener's function in the InteractionListeners registry.
pub struct ConsumableListener;
impl crate::world::component_objects::NativeObject for ConsumableListener {}
pub const CONSUMABLE_LISTENER: &str = "content.global.handlers.item.ConsumableListener";

/// A familiar: none exists in this world.
pub enum NoFamiliar {}

/// `consumables::Host` for one player.
pub struct Eating<'a> {
    ctx: Ctx<'a>,
    skills: Skills,
    clocks: [i32; 10],
    before: [i32; 10],
}

fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}
fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

impl<'a> Eating<'a> {
    fn new(live: &'a mut Live, p: P) -> Result<Self> {
        let skills = live
            .inner
            .with_profile(&p, |profile| {
                std::mem::replace(&mut profile.skills, placeholder_skills())
            })
            .map_err(bind)?;
        let shared = live.inner.state.clocks(&p.0).map_err(bind)?;
        let mut clocks = [0; 10];
        for (slot, value) in clocks.iter_mut().zip(shared.read().iter()) {
            *slot = *value;
        }
        Ok(Self {
            ctx: Ctx {
                live,
                p,
                disabled: false,
            },
            skills,
            clocks,
            before: clocks,
        })
    }
    /// Puts the skills back and writes the clocks this call changed.
    fn finish(self) -> Result<()> {
        let Self {
            ctx,
            skills,
            clocks,
            before,
        } = self;
        ctx.live
            .inner
            .with_profile(&ctx.p, |profile| profile.skills = skills)
            .map_err(bind)?;
        let shared = ctx.live.inner.state.clocks(&ctx.p.0).map_err(bind)?;
        let mut shared = shared.write();
        for i in 0..10 {
            if clocks[i] != before[i] {
                if let Some(slot) = shared.get_mut(i) {
                    *slot = clocks[i];
                }
            }
        }
        Ok(())
    }
    /// A call into a system that reads the player's skills from its profile:
    /// the skills go back for the call.
    fn outside<T>(&mut self, f: impl FnOnce(&mut Live, &P) -> T) -> Result<T> {
        let p = self.ctx.p.clone();
        let held = std::mem::replace(&mut self.skills, placeholder_skills());
        self.ctx
            .live
            .inner
            .with_profile(&p, |profile| profile.skills = held)
            .map_err(bind)?;
        let value = f(self.ctx.live, &p);
        self.skills = self
            .ctx
            .live
            .inner
            .with_profile(&p, |profile| {
                std::mem::replace(&mut profile.skills, placeholder_skills())
            })
            .map_err(bind)?;
        Ok(value)
    }
    fn container(&self, kind: ContainerKind) -> crate::inventory::container::Container {
        let player = &self.ctx.live.inner.canonical(&self.ctx.p).player;
        match kind {
            ContainerKind::Inventory => player.inventory(),
            ContainerKind::Equipment => player.equipment(),
        }
    }
    fn with_container<T>(
        &mut self,
        kind: ContainerKind,
        f: impl FnOnce(
            &mut crate::inventory::container::Container,
            &crate::definitions::item::Registry,
            &mut crate::inventory::player_binding::Context<
                '_,
                '_,
                crate::player::owned::Services<'_>,
            >,
        ) -> crate::inventory::Result<T>,
    ) -> Result<T> {
        let mut target = self.container(kind);
        let p = self.ctx.p.clone();
        let canonical = self.ctx.live.inner.canonical(&p).player.clone();
        let inner = &mut self.ctx.live.inner;
        let result = canonical.apply(
            &mut inner.state,
            &mut inner.frames,
            now_millis,
            |_, defs, ctx| f(&mut target, defs, ctx),
        );
        inner.drain(&p);
        result.map_err(bind)
    }
    fn native(&self, item: &Item) -> ItemHandle {
        Shared::new(NativeItem::new(
            item.definition,
            item.amount,
            1000,
            &self.ctx.live.inner.items,
        ))
    }
}

fn of_native(item: &ItemHandle) -> Item {
    let item = item.read();
    Item::at(item.id(), item.amount(), item.index)
}

impl<'a> c::Host for Eating<'a> {
    type SkillsContext = Ctx<'a>;
    type SettingsContext = Ctx<'a>;
    type Timer = Timer;
    type Familiar = NoFamiliar;

    fn skills(&mut self) -> (&mut Skills, &mut Ctx<'a>) {
        (&mut self.skills, &mut self.ctx)
    }
    fn settings(&mut self) -> (Settings, &mut Ctx<'a>) {
        let settings = self.ctx.live.inner.canonical(&self.ctx.p).player.settings();
        (settings, &mut self.ctx)
    }
    fn clocks(&mut self) -> &mut [i32; 10] {
        &mut self.clocks
    }
    fn item_name(&self, definition: i32) -> String {
        self.ctx
            .live
            .inner
            .items
            .for_id(definition)
            .read()
            .name
            .clone()
            .unwrap_or_else(|| "null".into())
    }
    fn disease_hits_left(&mut self, timer: &Timer) -> Result<i32> {
        join::disease_hits_left::<super::timers::Host<'_>>(timer).map_err(bind)
    }
    fn set_disease_hits_left(&mut self, timer: &Timer, value: i32) -> Result<()> {
        join::set_disease_hits_left::<super::timers::Host<'_>>(timer, value).map_err(bind)
    }
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.ctx.live.inner.ticks)
    }
    fn attribute(&mut self, key: &str, default: &Attr) -> Result<Attr> {
        let value = self
            .ctx
            .live
            .inner
            .canonical(&self.ctx.p)
            .player
            .attributes()
            .get(key);
        Ok(match value {
            None => default.clone(),
            Some(AttributeValue::Int(i)) => Attr::Int(i),
            Some(AttributeValue::Bool(b)) => Attr::Bool(b),
            Some(AttributeValue::Text(t)) => Attr::Str(t),
            Some(AttributeValue::Long(_)) => Attr::Other("Long".into()),
            Some(AttributeValue::Short(_)) => Attr::Other("Short".into()),
            Some(AttributeValue::Byte(_)) => Attr::Other("Byte".into()),
            Some(AttributeValue::Location(_)) => Attr::Other("Location".into()),
            Some(AttributeValue::Object(_)) => Attr::Other("Object".into()),
        })
    }
    fn set_attribute(&mut self, key: &str, value: Attr) -> Result<()> {
        let value = match value {
            Attr::Int(i) => AttributeValue::Int(i),
            Attr::Bool(b) => AttributeValue::Bool(b),
            Attr::Str(s) => AttributeValue::Text(s),
            Attr::Null | Attr::Other(_) => {
                return refuse(
                    "an attribute value a consumable effect stores other than Int, Boolean or String",
                );
            }
        };
        self.ctx
            .live
            .inner
            .canonical(&self.ctx.p)
            .player
            .attributes()
            .set_value(key, Some(value));
        Ok(())
    }
    fn animate(&mut self, animation: Option<i32>) -> Result<bool> {
        let p = self.ctx.p.clone();
        let animation = animation.map(crate::world::animation::Animation::new);
        let live = &mut *self.ctx.live;
        let animator = live.inner.canonical(&p).animator.clone();
        animator
            .animate(&mut live.walk.animations, animation, None)
            .map_err(bind)
    }
    fn play_audio(&mut self, id: i32, delay: i32, loops: i32) -> Result<()> {
        let p = self.ctx.p.clone();
        self.ctx
            .live
            .inner
            .queue_native(&p, updates::synth_sound(id, loops, delay));
        Ok(())
    }
    fn packet_message(&mut self, text: &str) -> Result<()> {
        let p = self.ctx.p.clone();
        self.ctx.live.inner.message(&p, text).map_err(bind)
    }
    fn send_message(&mut self, text: &str) -> Result<()> {
        // ContentAPI.sendMessage: player.sendMessages(*splitLines(message, 86)).
        let p = self.ctx.p.clone();
        let units: Vec<u16> = text.encode_utf16().collect();
        let lines = crate::world::plain_dialogue::split_lines(&units, 86).map_err(bind)?;
        for line in lines {
            self.ctx
                .live
                .inner
                .message(&p, &String::from_utf16_lossy(&line))
                .map_err(bind)?;
        }
        Ok(())
    }
    fn manual_hit(&mut self, hit: i32, kind: HitsplatType) -> Result<()> {
        self.outside(|live, p| live.timer_manual_hit(p, p, hit, kind))?
            .map_err(Error::Binding)
    }
    fn send_chat(&mut self, _: &str) -> Result<bool> {
        refuse("Player.sendChat from a consumable (force chat on a player)")
    }
    fn teleport(&mut self, _: [i32; 3]) -> Result<()> {
        refuse("player.teleport from a consumable (the Trouble Brewing rum)")
    }
    fn run_pulse(&mut self, _: RumPulse) -> Result<()> {
        refuse("TroubleBrewingRumEffect's pulses (content)")
    }
    fn finish_task(&mut self, _: c::Diary, _: i32, _: i32) -> Result<()> {
        refuse("AchievementDiaryManager.finishTask from a consumable (diary content)")
    }
    fn familiar(&mut self) -> Result<Option<NoFamiliar>> {
        self.ctx
            .live
            .inner
            .not_ported
            .default_answer("FamiliarManager.familiar (familiars not installed: null)");
        Ok(None)
    }
    fn update_special_points(&mut self, familiar: &NoFamiliar, _: i32) -> Result<()> {
        match *familiar {}
    }
    fn replace(
        &mut self,
        container: ContainerKind,
        item: Option<Item>,
        slot: i32,
    ) -> Result<Option<Item>> {
        let item = item.map(|i| self.native(&i));
        let old = self.with_container(container, |c, defs, ctx| {
            c.replace(item.as_ref(), slot, true, defs, ctx)
        })?;
        Ok(old.as_ref().map(of_native))
    }
    fn remove(&mut self, container: ContainerKind, item: &Item) -> Result<bool> {
        let item = self.native(item);
        self.with_container(container, |c, defs, ctx| {
            c.remove(Some(&item), None, true, defs, ctx)
        })
    }
    fn remove_slot(
        &mut self,
        container: ContainerKind,
        item: &Item,
        slot: i32,
        fire: bool,
    ) -> Result<bool> {
        let item = self.native(item);
        self.with_container(container, |c, defs, ctx| {
            c.remove(Some(&item), Some(slot), fire, defs, ctx)
        })
    }
    fn contains(&mut self, container: ContainerKind, id: i32, amount: i32) -> Result<bool> {
        Ok(self.container(container).contains(id, amount))
    }
    fn dupe_alert(&mut self, details: &str) -> Result<()> {
        let name = self.ctx.live.inner.slot(&self.ctx.p).name.clone();
        self.ctx
            .live
            .inner
            .log_line(format!("PlayerMonitor DUPE_ALERT {name}: {details}"));
        self.ctx
            .live
            .inner
            .not_ported
            .skip("PlayerMonitor drop-trade log and DUPE_ALERT (logged only)");
        Ok(())
    }
    fn spawn_timer(&mut self, identifier: &str, args: &[TimerArg]) -> Result<Option<Timer>> {
        self.outside(|live, p| {
            live.with_timers(p, |natives, manager| {
                join::spawn_timer(natives, manager, identifier, args)
            })
        })?
        .map_err(bind)
    }
    fn register_timer(&mut self, timer: Option<&Timer>) -> Result<()> {
        self.outside(|live, p| {
            live.with_timers(p, |natives, manager| {
                join::register_timer(natives, manager, timer)
            })
        })?
        .map_err(bind)
    }
    fn remove_timer(&mut self, identifier: &str) -> Result<()> {
        self.outside(|live, p| {
            live.with_timers(p, |natives, manager| {
                join::remove_timer(natives, manager, identifier)
            })
        })?
        .map_err(bind)
    }
    fn disease(&mut self) -> Result<Option<Timer>> {
        self.outside(|live, p| live.with_timers(p, |_, manager| join::disease(manager)))?
            .map_err(bind)
    }
    fn remove_disease(&mut self) -> Result<()> {
        self.outside(|live, p| {
            live.with_timers(p, |natives, manager| join::remove_disease(natives, manager))
        })?
        .map_err(bind)
    }
    fn active_script_persist(&mut self) -> Result<Option<bool>> {
        // entity.scripts.getActiveScript(): the dose script is run by the
        // world (not by the canonical script queue), which runs no other
        // queued script: none is active.
        Ok(None)
    }
    fn add_to_queue(
        &mut self,
        script: DoseScript,
        strength: QueueStrength,
        next_execution: i32,
        persist: bool,
    ) -> Result<()> {
        if strength != QueueStrength::Soft || persist {
            return refuse("a queued script other than the potion's SOFT dose message");
        }
        let node = self.ctx.p.node();
        self.ctx
            .live
            .round
            .eat
            .doses
            .push((next_execution, node, script));
        Ok(())
    }
    fn reset_scripts(&mut self) -> Result<()> {
        refuse("entity.scripts.reset from stopExecuting (no persistent script runs here)")
    }
    fn log(&mut self, level: Log, message: &str) -> Result<()> {
        self.ctx
            .live
            .inner
            .log_line(format!("[Consumables] {} {message}", level.name()));
        Ok(())
    }
    fn delay_next_attack(&mut self, ticks: i32) -> Result<()> {
        // entity.properties.combatPulse.delayNextAttack(ticks).
        let p = self.ctx.p.clone();
        crate::world::combat_pulse::pulse::delay_next_attack(self.ctx.live, &p, ticks);
        Ok(())
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        self.ctx
            .live
            .inner
            .generators
            .shared_next_int(bound)
            .ok_or_else(|| Error::Binding("RANDOM.nextInt: bound must be positive".into()))
    }
}

impl Live {
    /// ConsumableListener's `handleConsumable(player, node)`.
    pub fn consume(&mut self, p: &P, item: &ItemHandle) -> Result<bool> {
        let table = self.round.eat.table.clone();
        let node = Node::Item(of_native(item));
        let mut host = Eating::new(self, p.clone())?;
        let result = c::handle(&table, &node, &mut host);
        let finished = host.finish();
        self.inner.drain(p);
        let value = result?;
        finished?;
        Ok(value)
    }
    /// The potions' dose scripts due for a player at its tick.
    pub fn run_dose_scripts(&mut self, p: &P) -> Result<()> {
        let ticks = self.inner.ticks;
        let node = p.node();
        let mut due = Vec::new();
        self.round.eat.doses.retain(|(at, owner, script)| {
            if *owner == node && *at <= ticks {
                due.push(script.clone());
                false
            } else {
                true
            }
        });
        for script in due {
            let mut host = Eating::new(self, p.clone())?;
            let result = script.run(&mut host, 0);
            let finished = host.finish();
            self.inner.drain(p);
            result?;
            finished?;
        }
        Ok(())
    }
    /// ConsumableListener.defineListeners: `on(IntType.ITEM, "eat", "drink")`.
    pub fn install_consumables(&mut self) -> std::result::Result<(), String> {
        use crate::world::interaction_listeners::{Function, FunctionKind, Kind, Text};
        let registry = self.item_actions.listeners.clone();
        c::define_listeners(|_, options| {
            let options: Vec<Text> = options.iter().map(|o| o.encode_utf16().collect()).collect();
            let function = Function::new(FunctionKind::On, CONSUMABLE_LISTENER, ConsumableListener);
            registry
                .add_generics(
                    &super::items::Ctx(self),
                    &options,
                    Kind::Item.ordinal(),
                    function,
                )
                .map_err(|e| format!("{e:?}"))
        })
    }
}
