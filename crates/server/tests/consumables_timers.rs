//! Joins the consumables' timer services to the native engine timers
//! (`timers::content`) through `consumables::join`: an antipoison registers a
//! real PoisonImmunity (which removes the active Poison), relicym lowers a
//! real Disease's hits and cures it, and an unknown identifier spawns nothing.
use rs09_server::player::{
    settings::{self, Settings},
    skills::{self, Skills},
};
use rs09_server::timers::{
    self, Handle, Manager,
    content::{self, Arg, EntityKind, Event, EventClass, Hook, Kind, Logout, Natives, State, Tab},
    registry::{Level, Registry},
};
use rs09_server::world::combat_pulse::HitsplatType;
use rs09_server::world::consumables::{
    self, Attr, ContainerKind, Diary, DoseScript, Host, Item, Log, QueueStrength, RumPulse, Table,
    TimerArg, join,
};

struct World {
    skills: Skills,
    settings: Settings,
    clocks: [i32; 10],
    tick: i32,
    manager: Option<Manager>,
    registry: Registry,
    messages: Vec<String>,
    inventory: Vec<Option<Item>>,
}

struct Ctx;

impl skills::Context for Ctx {
    fn kind(&self) -> skills::EntityKind {
        skills::EntityKind::Player
    }
    fn version(&self) -> i32 {
        unreachable!()
    }
    fn tick(&mut self) -> i32 {
        0
    }
    fn world_tick(&mut self) -> i32 {
        unreachable!()
    }
    fn combat_level(&self) -> i32 {
        unreachable!()
    }
    fn summoning_combat_level(&self) -> i32 {
        unreachable!()
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        unreachable!()
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        Ok(false)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Ok(None)
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        unreachable!()
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        unreachable!()
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        unreachable!()
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        unreachable!()
    }
    fn username(&mut self) -> skills::Result<String> {
        unreachable!()
    }
    fn permadeath_present(&self) -> bool {
        unreachable!()
    }
    fn effect(&mut self, _: &mut Skills, _: skills::Effect) -> skills::Result<()> {
        Ok(())
    }
}

impl settings::Context for Ctx {
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        Ok(())
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn send(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: rs09_protocol::game::Frame,
    ) -> settings::Result<()> {
        unreachable!()
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        Ok(())
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        unreachable!()
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: String,
        _: i32,
        _: i32,
    ) -> settings::Result<()> {
        unreachable!()
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        unreachable!()
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        unreachable!()
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        unreachable!()
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        unreachable!()
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        unreachable!()
    }
}

fn ctx() -> &'static mut Ctx {
    // A zero-sized context: every reference is to the same empty value.
    Box::leak(Box::new(Ctx))
}

impl content::Host for World {
    type Entity = usize;
    type RandomEvent = String;
    type EventNpc = usize;
    type Weapon = i32;
    type Dialogue = String;
    type SkillsContext = Ctx;
    type SettingsContext = Ctx;

    fn kind(&self, _: &usize) -> EntityKind {
        EntityKind::Player
    }
    fn skills(&mut self, _: &usize) -> (&mut Skills, &mut Ctx) {
        (&mut self.skills, ctx())
    }
    fn settings(&mut self, _: &usize) -> (Settings, &mut Ctx) {
        (self.settings.clone(), ctx())
    }
    fn spell_book(&mut self, _: &usize) -> &mut i32 {
        unreachable!()
    }
    fn random_event_name(&self, _: &String) -> String {
        unreachable!()
    }
    fn random_event_value_of(&self, _: &str) -> Option<String> {
        unreachable!()
    }
    fn ticks(&mut self) -> timers::Result<i32> {
        Ok(self.tick)
    }
    fn modal_open(&mut self, _: &usize) -> timers::Result<bool> {
        Ok(false)
    }
    fn script_delay(&mut self, _: &usize) -> timers::Result<i32> {
        Ok(0)
    }
    fn remove_weak_scripts(&mut self, _: &usize) -> timers::Result<()> {
        unreachable!()
    }
    fn name(&mut self, _: &usize) -> timers::Result<Option<String>> {
        unreachable!()
    }
    fn location(&mut self, _: &usize) -> timers::Result<Option<[i32; 3]>> {
        unreachable!()
    }
    fn log(&mut self, _: Level, _: &str) -> timers::Result<()> {
        Ok(())
    }
    fn exception(&mut self, e: &timers::Error) -> timers::Result<()> {
        panic!("{e:?}")
    }
    fn send_message(&mut self, _: &usize, text: &str) -> timers::Result<()> {
        self.messages.push(text.into());
        Ok(())
    }
    fn debug(&mut self, _: &usize, text: &str) -> timers::Result<()> {
        self.messages.push(text.into());
        Ok(())
    }
    fn play_audio(&mut self, _: &usize, _: i32, _: i32, _: i32) -> timers::Result<()> {
        unreachable!()
    }
    fn next_int(&mut self, _: i32) -> timers::Result<i32> {
        unreachable!()
    }
    fn manual_hit(&mut self, _: &usize, _: &usize, _: i32, _: HitsplatType) -> timers::Result<()> {
        unreachable!()
    }
    fn worn(&mut self, _: &usize, _: i32) -> timers::Result<Option<i32>> {
        unreachable!()
    }
    fn artificial(&mut self, _: &usize) -> timers::Result<bool> {
        unreachable!()
    }
    fn administrator(&mut self, _: &usize) -> timers::Result<bool> {
        unreachable!()
    }
    fn uid(&mut self, _: &usize) -> timers::Result<i32> {
        unreachable!()
    }
    fn player_by_uid(&mut self, _: i32) -> timers::Result<Option<usize>> {
        unreachable!()
    }
    fn npc_task(&mut self, _: &usize) -> timers::Result<Option<i32>> {
        unreachable!()
    }
    fn familiar_owner(&mut self, _: &usize) -> timers::Result<Option<usize>> {
        unreachable!()
    }
    fn poison_immune(&mut self, _: &usize) -> timers::Result<bool> {
        unreachable!()
    }
    fn set_skull_icon(&mut self, _: &usize, _: i32) -> timers::Result<()> {
        unreachable!()
    }
    fn set_skulled(&mut self, _: &usize, _: bool) -> timers::Result<()> {
        unreachable!()
    }
    fn reset_skull(&mut self, _: &usize) -> timers::Result<()> {
        unreachable!()
    }
    fn hook(&mut self, _: &usize, _: EventClass, _: Hook) -> timers::Result<()> {
        unreachable!()
    }
    fn unhook(&mut self, _: &usize, _: &Hook) -> timers::Result<()> {
        unreachable!()
    }
    fn dispatch(
        &mut self,
        _: &usize,
        _: Event<String>,
        _: &mut Manager,
        _: &Registry,
    ) -> timers::Result<()> {
        unreachable!()
    }
    fn register_logout_listener(&mut self, _: &usize, _: &str, _: Logout) -> timers::Result<()> {
        unreachable!()
    }
    fn clear_logout_listener(&mut self, _: &usize, _: &str) -> timers::Result<()> {
        unreachable!()
    }
    fn weapon_interface(&mut self, _: &usize) -> timers::Result<Option<i32>> {
        unreachable!()
    }
    fn autocast_set(&mut self, _: &usize) -> timers::Result<bool> {
        unreachable!()
    }
    fn select_auto_spell(
        &mut self,
        _: &usize,
        _: &i32,
        _: i32,
        _: bool,
        _: bool,
    ) -> timers::Result<()> {
        unreachable!()
    }
    fn open_tab(&mut self, _: &usize, _: Tab<'_, i32>) -> timers::Result<()> {
        unreachable!()
    }
    fn set_varbit(&mut self, _: &usize, _: i32, _: i32, _: bool) -> timers::Result<()> {
        unreachable!()
    }
    fn attribute_flag(&mut self, _: &usize, _: &str, _: bool) -> timers::Result<bool> {
        unreachable!()
    }
    fn remove_attribute(&mut self, _: &usize, _: &str) -> timers::Result<()> {
        unreachable!()
    }
    fn event_npc(&mut self, _: &usize) -> timers::Result<Option<usize>> {
        unreachable!()
    }
    fn set_event_npc(&mut self, _: &usize, _: &usize) -> timers::Result<()> {
        unreachable!()
    }
    fn random_events_restricted(&mut self, _: &usize) -> timers::Result<bool> {
        unreachable!()
    }
    fn skill_random_event(&mut self, _: i32) -> timers::Result<Option<String>> {
        unreachable!()
    }
    fn non_skill_random_event(&mut self) -> timers::Result<String> {
        unreachable!()
    }
    fn create_event_npc(&mut self, _: &String, _: &usize) -> timers::Result<usize> {
        unreachable!()
    }
    fn spawn_location_missing(&mut self, _: &usize) -> timers::Result<bool> {
        unreachable!()
    }
    fn init_event_npc(&mut self, _: &usize) -> timers::Result<()> {
        unreachable!()
    }
    fn terminate_event_npc(&mut self, _: &usize) -> timers::Result<()> {
        unreachable!()
    }
    fn roll_event_loot(&mut self, _: &usize, _: &usize) -> timers::Result<Option<Vec<(i32, i32)>>> {
        unreachable!()
    }
    fn dialogue_ids(&mut self, _: &String) -> timers::Result<Option<Vec<i32>>> {
        unreachable!()
    }
}

fn timer_error(e: timers::Error) -> consumables::Error {
    consumables::Error::Binding(format!("timers: {e:?}"))
}

impl World {
    fn with_timers<T>(
        &mut self,
        f: impl FnOnce(&mut Natives<'_, World>, &mut Manager) -> timers::Result<T>,
    ) -> consumables::Result<T> {
        let mut manager = self.manager.take().unwrap();
        let registry = self.registry.clone();
        let result = f(
            &mut Natives {
                host: self,
                entity: 0,
                registry,
            },
            &mut manager,
        );
        self.manager = Some(manager);
        result.map_err(timer_error)
    }
}

impl Host for World {
    type SkillsContext = Ctx;
    type SettingsContext = Ctx;
    type Timer = Handle;
    type Familiar = ();

    fn skills(&mut self) -> (&mut Skills, &mut Ctx) {
        (&mut self.skills, ctx())
    }
    fn settings(&mut self) -> (Settings, &mut Ctx) {
        (self.settings.clone(), ctx())
    }
    fn clocks(&mut self) -> &mut [i32; 10] {
        &mut self.clocks
    }
    fn item_name(&self, definition: i32) -> String {
        match definition {
            2446 => "Antipoison(4)".into(),
            4842 => "Relicym's balm(4)".into(),
            _ => "null".into(),
        }
    }
    fn disease_hits_left(&mut self, timer: &Handle) -> consumables::Result<i32> {
        join::disease_hits_left::<World>(timer).map_err(timer_error)
    }
    fn set_disease_hits_left(&mut self, timer: &Handle, value: i32) -> consumables::Result<()> {
        join::set_disease_hits_left::<World>(timer, value).map_err(timer_error)
    }
    fn ticks(&mut self) -> consumables::Result<i32> {
        Ok(self.tick)
    }
    fn attribute(&mut self, _: &str, default: &Attr) -> consumables::Result<Attr> {
        Ok(default.clone())
    }
    fn set_attribute(&mut self, _: &str, _: Attr) -> consumables::Result<()> {
        unreachable!()
    }
    fn animate(&mut self, _: Option<i32>) -> consumables::Result<bool> {
        Ok(true)
    }
    fn play_audio(&mut self, _: i32, _: i32, _: i32) -> consumables::Result<()> {
        Ok(())
    }
    fn packet_message(&mut self, text: &str) -> consumables::Result<()> {
        self.messages.push(text.into());
        Ok(())
    }
    fn send_message(&mut self, text: &str) -> consumables::Result<()> {
        self.messages.push(text.into());
        Ok(())
    }
    fn manual_hit(&mut self, _: i32, _: HitsplatType) -> consumables::Result<()> {
        unreachable!()
    }
    fn send_chat(&mut self, _: &str) -> consumables::Result<bool> {
        unreachable!()
    }
    fn teleport(&mut self, _: [i32; 3]) -> consumables::Result<()> {
        unreachable!()
    }
    fn run_pulse(&mut self, _: RumPulse) -> consumables::Result<()> {
        unreachable!()
    }
    fn finish_task(&mut self, _: Diary, _: i32, _: i32) -> consumables::Result<()> {
        unreachable!()
    }
    fn familiar(&mut self) -> consumables::Result<Option<()>> {
        unreachable!()
    }
    fn update_special_points(&mut self, _: &(), _: i32) -> consumables::Result<()> {
        unreachable!()
    }
    fn replace(
        &mut self,
        _: ContainerKind,
        item: Option<Item>,
        slot: i32,
    ) -> consumables::Result<Option<Item>> {
        let old = self.inventory[slot as usize].take();
        self.inventory[slot as usize] = item;
        Ok(old)
    }
    fn remove(&mut self, _: ContainerKind, _: &Item) -> consumables::Result<bool> {
        unreachable!()
    }
    fn remove_slot(
        &mut self,
        _: ContainerKind,
        _: &Item,
        _: i32,
        _: bool,
    ) -> consumables::Result<bool> {
        unreachable!()
    }
    fn contains(&mut self, _: ContainerKind, _: i32, _: i32) -> consumables::Result<bool> {
        unreachable!()
    }
    fn dupe_alert(&mut self, _: &str) -> consumables::Result<()> {
        unreachable!()
    }
    fn spawn_timer(
        &mut self,
        identifier: &str,
        args: &[TimerArg],
    ) -> consumables::Result<Option<Handle>> {
        self.with_timers(|n, m| join::spawn_timer(n, m, identifier, args))
    }
    fn register_timer(&mut self, timer: Option<&Handle>) -> consumables::Result<()> {
        self.with_timers(|n, m| join::register_timer(n, m, timer))
    }
    fn remove_timer(&mut self, identifier: &str) -> consumables::Result<()> {
        self.with_timers(|n, m| join::remove_timer(n, m, identifier))
    }
    fn disease(&mut self) -> consumables::Result<Option<Handle>> {
        join::disease(self.manager.as_ref().unwrap()).map_err(timer_error)
    }
    fn remove_disease(&mut self) -> consumables::Result<()> {
        self.with_timers(join::remove_disease)
    }
    fn active_script_persist(&mut self) -> consumables::Result<Option<bool>> {
        Ok(None)
    }
    fn add_to_queue(
        &mut self,
        _: DoseScript,
        _: QueueStrength,
        _: i32,
        _: bool,
    ) -> consumables::Result<()> {
        Ok(())
    }
    fn reset_scripts(&mut self) -> consumables::Result<()> {
        unreachable!()
    }
    fn log(&mut self, _: Log, _: &str) -> consumables::Result<()> {
        unreachable!()
    }
    fn delay_next_attack(&mut self, _: i32) -> consumables::Result<()> {
        unreachable!()
    }
    fn next_int(&mut self, _: i32) -> consumables::Result<i32> {
        unreachable!()
    }
}

fn world() -> World {
    let mut skills = Skills::new(ctx()).unwrap();
    skills.lifepoints = 50;
    let mut world = World {
        skills,
        settings: Settings::default(),
        clocks: [0; 10],
        tick: 100,
        manager: Some(Manager::default()),
        registry: Registry::default(),
        messages: Vec::new(),
        inventory: vec![None; 28],
    };
    world.with_timers(content::register_prototypes).unwrap();
    world
}

fn active(world: &World) -> Vec<String> {
    let m = world.manager.as_ref().unwrap();
    m.active
        .read()
        .values()
        .iter()
        .chain(m.pending.read().values().iter())
        .flatten()
        .map(|t| t.read().identifier.clone())
        .collect()
}

#[test]
fn antipoison_registers_the_native_immunity_and_relicym_cures_the_native_disease() {
    let table = Table::new();
    let mut w = world();
    // An active poison and disease, registered as the original's applyPoison/Disease would be.
    let poison = w
        .with_timers(|n, m| content::spawn(n, m, Kind::Poison, &[Arg::Entity(0), Arg::Int(30)]))
        .unwrap();
    w.with_timers(|n, m| join::register_timer(n, m, Some(&poison)))
        .unwrap();
    let disease = w
        .with_timers(|n, m| content::spawn(n, m, Kind::Disease, &[]))
        .unwrap();
    w.with_timers(|n, m| join::register_timer(n, m, Some(&disease)))
        .unwrap();
    join::set_disease_hits_left::<World>(&disease, 12).unwrap();
    assert!(active(&w).contains(&"poison".to_string()));

    // Antipoison(4): AddTimerEffect("poison:immunity", 150).
    w.inventory[0] = Some(Item::at(2446, 1, 0));
    let row = &table.rows[table.lookup(2446).unwrap()].consumable;
    row.consume(&table, &Item::at(2446, 1, 0), &mut w).unwrap();
    let ids = active(&w);
    assert!(ids.contains(&"poison:immunity".to_string()), "{ids:?}");
    // PoisonImmunity.onRegister removes Poison: the manager takes it off at its next process.
    let removing: Vec<String> = w
        .manager
        .as_ref()
        .unwrap()
        .removing
        .read()
        .values()
        .iter()
        .flatten()
        .map(|t| t.read().identifier.clone())
        .collect();
    assert_eq!(removing, ["poison"], "{ids:?}");
    let immunity = content::get(w.manager.as_ref().unwrap(), Kind::PoisonImmunity)
        .unwrap()
        .unwrap();
    let ticks = content::with_state::<usize, String, _>(&immunity, |s| match s {
        State::Remaining { ticks } => *ticks,
        _ => -1,
    })
    .unwrap();
    assert_eq!(ticks, 150);
    assert_eq!(w.inventory[0], Some(Item::new(175, 1)));

    // Relicym's balm(4): CureDiseaseEffect lowers hitsLeft by 9 (12 -> 3), then cures.
    let relicym = &table.rows[table.lookup(4842).unwrap()].consumable;
    relicym
        .consume(&table, &Item::at(4842, 1, 1), &mut w)
        .unwrap();
    assert_eq!(join::disease_hits_left::<World>(&disease).unwrap(), 3);
    assert!(
        w.messages
            .contains(&"You feel slightly better.".to_string())
    );
    relicym
        .consume(&table, &Item::at(4844, 1, 1), &mut w)
        .unwrap();
    assert!(
        w.messages
            .contains(&"The disease has been cured.".to_string())
    );
    assert!(
        join::disease(w.manager.as_ref().unwrap())
            .unwrap()
            .is_none()
    );

    // An identifier the registry does not hold spawns nothing.
    assert!(w.spawn_timer("no such timer", &[]).unwrap().is_none());
}
