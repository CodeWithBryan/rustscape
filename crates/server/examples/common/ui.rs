//! Post-tutorial gameframe binding using original component metadata and
//! canonical player state.
use super::{Frame, Host, Node, Probe, State};
use rs09_protocol::game::{actions::ClientAction, updates};
use rs09_server::{
    definitions::item,
    inventory::container::{Container, ContainerType, Factory, SortType},
    player::details::ClientInfo,
    world::interfaces::{
        self as ui, Context, Error, Handle, LoginContext, Manager, Registry, Result,
    },
};
use std::sync::Arc;
use std::{collections::BTreeMap, path::Path};
#[path = "ui_bank.rs"]
mod bank_binding;
#[path = "ui_equipment.rs"]
mod equipment_binding;
#[path = "ui_inventory.rs"]
mod inventory_binding;
#[path = "ui_shop.rs"]
pub(super) mod shops;
#[path = "ui_skills.rs"]
mod skills_binding;
pub(super) struct Ui {
    registry: Registry,
    listeners: rs09_server::world::interface_listeners::Registry,
    bank_probe: bool,
    factory: Factory,
    definitions: Arc<item::Registry>,
    definitions_set: bool,
    variable_assets: Option<Arc<rs09_cache::archive::Archive>>,
    players: BTreeMap<Node, Player>,
    shops: shops::Catalog,
}
struct Player {
    manager: Manager,
    client: ClientInfo,
    weapon: Option<Handle>,
    restore_special_at: i32,
    inventory_binding: inventory_binding::Capture,
    skills: Option<rs09_server::player::skills::Skills>,
    last_ping: i64,
    last_walk: Option<serde_json::Value>,
    bank_probe_opened: bool,
    shop_route: Option<shops::Route>,
    shop_view: Option<shops::View>,
}
impl Player {
    fn inventory(&self) -> Container {
        self.inventory_binding.inventory()
    }
    #[cfg(test)]
    fn has_pouch(&self) -> bool {
        self.inventory_binding.has_pouch()
    }
    fn equipment(&self) -> Container {
        self.inventory_binding.equipment()
    }
    fn settings(&self) -> rs09_server::player::settings::Settings {
        self.inventory_binding.settings()
    }
}
impl Ui {
    pub(super) fn load(path: &Path) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let mut registry = Registry::default();
        let count = registry.load(&std::fs::read_to_string(path)?)?;
        println!("Native interface metadata: {count} pinned rows");
        let listeners = rs09_server::world::interface_listeners::Registry::default();
        rs09_server::world::bank_interface::Program::default().define(&listeners)?;
        bank_binding::define_player_buttons(&listeners)?;
        Ok(Self {
            registry,
            listeners,
            bank_probe: false,
            factory: Factory::new(&item::Registry::default()),
            definitions: Arc::new(item::Registry::default()),
            definitions_set: false,
            variable_assets: None,
            players: BTreeMap::new(),
            shops: shops::Catalog::default(),
        })
    }
    pub(super) fn enable_shops(
        &mut self,
        path: &Path,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.shops = shops::Catalog::load(&std::fs::read_to_string(path)?)?;
        Ok(())
    }
    pub(super) fn use_definitions(&mut self, definitions: Arc<item::Registry>) -> ui::Result<()> {
        if self.definitions_set || !self.players.is_empty() {
            return Err(Error::Binding(
                "item registry is fixed before player admission".into(),
            ));
        }
        self.factory = Factory::new(&definitions);
        self.definitions = definitions;
        self.definitions_set = true;
        Ok(())
    }
    pub(super) fn enable_bank_probe(&mut self) -> ui::Result<()> {
        if !self.players.is_empty() {
            return Err(Error::Binding("bank probe must precede admission".into()));
        }
        self.bank_probe = true;
        Ok(())
    }
    pub(super) fn use_variable_assets(
        &mut self,
        archive: Arc<rs09_cache::archive::Archive>,
    ) -> ui::Result<()> {
        if self.variable_assets.is_some() || !self.players.is_empty() {
            return Err(Error::Binding(
                "variable archive is fixed before player admission".into(),
            ));
        }
        self.variable_assets = Some(archive);
        Ok(())
    }
}
struct VariableAssets(Option<Arc<rs09_cache::archive::Archive>>);
impl rs09_server::definitions::varbit::Assets for VariableAssets {
    fn file(
        &mut self,
        registry: &rs09_server::definitions::varbit::Registry,
        group: u32,
        file: u32,
    ) -> rs09_server::definitions::varbit::Result<Option<Vec<u8>>> {
        rs09_server::definitions::varbit::Assets::file(
            self.0.as_mut().ok_or_else(|| {
                rs09_server::definitions::varbit::Error::Binding(
                    "variable cache archive is unavailable".into(),
                )
            })?,
            registry,
            group,
            file,
        )
    }
}
struct UiContext<'a> {
    state: &'a mut State,
    host: &'a mut Host,
    node: Node,
}
impl<'a> UiContext<'a> {
    fn new(p: &'a mut Probe, node: Node) -> Self {
        Self::world(&mut p.state, &mut p.host, node)
    }
    fn world(state: &'a mut State, host: &'a mut Host, node: Node) -> Self {
        Self { state, host, node }
    }
}
impl UiContext<'_> {
    fn row(&self) -> Result<&Player> {
        self.host
            .ui
            .as_ref()
            .and_then(|u| u.players.get(&self.node))
            .ok_or(Error::Null)
    }
    fn row_mut(&mut self) -> Result<&mut Player> {
        self.host
            .ui
            .as_mut()
            .and_then(|u| u.players.get_mut(&self.node))
            .ok_or(Error::Null)
    }
}
impl Context for UiContext<'_> {
    fn validate_manager(&mut self, manager: &Manager) -> Result<()> {
        let owner = manager.player().ok_or(Error::Null)?;
        let fields = self
            .state
            .player_fields(&owner)
            .map_err(|e| Error::Binding(e.to_string()))?;
        if owner.node != self.node || !fields.manager().fields.same(&manager.fields) {
            return Err(Error::Binding(
                "interface context belongs to another player".into(),
            ));
        }
        Ok(())
    }
    fn has_runscript(&mut self) -> Result<bool> {
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let fields = self
            .state
            .player_fields(&owner)
            .map_err(|e| Error::Binding(e.to_string()))?;
        Ok(fields.attributes().get("runscript").is_some())
    }
    fn remove_runscript(&mut self) -> Result<()> {
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let fields = self
            .state
            .player_fields(&owner)
            .map_err(|e| Error::Binding(e.to_string()))?;
        fields.attributes().remove("runscript");
        Ok(())
    }
    fn component(&mut self, id: i32) -> Result<Handle> {
        Ok(self
            .host
            .ui
            .as_mut()
            .ok_or(Error::Null)?
            .registry
            .component(id))
    }
    fn resizable(&mut self) -> Result<bool> {
        Ok(self.row()?.client.is_resizable())
    }
    fn send(&mut self, f: Frame) -> Result<()> {
        self.host.send(self.node, f);
        Ok(())
    }
    fn log(&mut self, s: String) -> Result<()> {
        eprintln!("gameframe metadata: {s}");
        Ok(())
    }
    fn listener_open(&mut self, component: &Handle) -> Result<()> {
        let registry = self.host.ui.as_ref().ok_or(Error::Null)?.listeners.clone();
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        registry
            .run_open(self, Some(&owner), Some(component))
            .map(|_| ())
    }
    fn listener_close(&mut self, component: &Handle) -> Result<bool> {
        if component.read().id == 620 {
            shops::close(self)?;
        }
        let registry = self.host.ui.as_ref().ok_or(Error::Null)?.listeners.clone();
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        registry.run_close(self, Some(&owner), Some(component))
    }
    fn plugin_open(&mut self, plugin: ui::Plugin, _: &Handle) -> Result<()> {
        // BankPinInterface inherits ComponentPlugin.open's exact empty body.
        if rs09_server::player::bank_pin::Program::from_component_plugin(&plugin).is_some() {
            return Ok(());
        }
        Err(Error::Binding(
            "interface content plugins are not installed in fixture".into(),
        ))
    }
    fn close_event(&mut self, event: ui::CloseEvent, _: &Handle) -> Result<bool> {
        if let Some(event) = event.value::<rs09_server::world::component_objects::BankClose>() {
            if let rs09_server::inventory::bank::CloseEvent::Bank(bank) = &event.0 {
                if let Some(owner) = bank.owner() {
                    self.state
                        .player_fields(&owner)
                        .map_err(|e| Error::Binding(e.to_string()))?;
                }
            }
            let owner = self
                .state
                .retain_node(self.node)
                .map_err(|e| Error::Binding(e.to_string()))?;
            return event
                .0
                .close(Some(&owner), self)
                .map_err(|e| Error::Binding(format!("{e:?}")));
        }
        if let Some(event) = rs09_server::player::bank_pin::CloseEvent::from_component_event(&event)
        {
            if let Some(owner) = event.manager().player() {
                self.state
                    .player_fields(&owner)
                    .map_err(|e| Error::Binding(e.to_string()))?;
            }
            return Ok(event.close());
        }
        if event
            .value::<rs09_server::world::component_objects::Unclosable>()
            .is_some()
        {
            return self.unclosable_attribute().map(|value| !value);
        }
        Err(Error::Binding(
            "interface close plugins are not installed in fixture".into(),
        ))
    }
    fn open_component(&mut self, m: &Manager, c: &Handle) -> Result<()> {
        ui::component_open(self, m, c)
    }
    fn dispatch_open(&mut self, _: Handle) -> Result<()> {
        Ok(())
    }
    fn dispatch_close(&mut self, _: Option<Handle>) -> Result<()> {
        // This controlled post-tutorial fixture has no registered game event listeners.
        Ok(())
    }
    fn set_varp(&mut self, id: i32, value: i32) -> Result<()> {
        let mut assets = VariableAssets(
            self.host
                .ui
                .as_ref()
                .ok_or(Error::Null)?
                .variable_assets
                .clone(),
        );
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let fields = self
            .state
            .player_fields(&owner)
            .map_err(|e| Error::Binding(e.to_string()))?;
        fields
            .apply_variables(self.state, &mut self.host.frames, &mut assets, |ctx| {
                rs09_server::player::variables::set_varp(ctx, Some(&owner), id, value, false)
            })
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
    fn weapon_extension(&mut self) -> Result<Option<Handle>> {
        Ok(self.row()?.weapon.clone())
    }
    fn create_weapon(&mut self) -> Result<Handle> {
        let c = self.component(equipment_binding::group(self)?)?;
        let mut fields = c.write();
        fields.weapon = true;
        let definition = fields.definition.as_ref().ok_or(Error::Null)?;
        {
            let mut definition = definition.write();
            definition.kind = Some(ui::Type::Tab);
            definition.tab_index = 0;
            definition.walkable = true;
        }
        drop(fields);
        Ok(c)
    }
    fn add_weapon(&mut self, c: Handle) -> Result<()> {
        self.row_mut()?.weapon = Some(c);
        Ok(())
    }
    fn spellbook(&mut self) -> Result<i32> {
        Ok(192)
    }
    fn has_familiar(&mut self) -> Result<bool> {
        Ok(false)
    }
    fn friends_component(&mut self) -> Result<Handle> {
        self.component(550)
    }
    fn autocast(&mut self) -> Result<Option<usize>> {
        Ok(None)
    }
    fn auto_spell_id(&mut self, _: &Handle, _: usize) -> Result<i32> {
        Err(Error::Binding("autocast content is not installed".into()))
    }
    fn select_auto_spell(&mut self, _: &Handle, _: i32) -> Result<()> {
        Err(Error::Binding("autocast content is not installed".into()))
    }
    fn window_mode(&mut self) -> Result<i32> {
        Ok(self.row()?.client.window_mode)
    }
    fn set_window_mode(&mut self, mode: i32) -> Result<()> {
        self.row_mut()?.client.window_mode = mode;
        Ok(())
    }
    fn tutorial_complete(&mut self) -> Result<bool> {
        Ok(true)
    }
    fn tutorial_tabs(&mut self) -> Result<()> {
        Err(Error::Binding(
            "post-tutorial fixture has no tutorial content".into(),
        ))
    }
    fn set_unclosable_attribute(&mut self, value: bool) -> Result<()> {
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let fields = self
            .state
            .player_fields(&owner)
            .map_err(|e| Error::Binding(e.to_string()))?;
        fields.attributes().set_boolean("close_c_", value);
        Ok(())
    }
    fn unclosable_attribute(&mut self) -> Result<bool> {
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let fields = self
            .state
            .player_fields(&owner)
            .map_err(|e| Error::Binding(e.to_string()))?;
        fields
            .attributes()
            .boolean("close_c_", false)
            .map_err(|e| match e {
                rs09_server::player::attributes::Error::Type => Error::Type,
                _ => Error::Binding(format!("{e:?}")),
            })
    }
}
impl LoginContext for UiContext<'_> {
    fn welcome(&mut self) -> Result<()> {
        Ok(())
    }
    fn configure(&mut self, m: &Manager) -> Result<()> {
        inventory_binding::refresh(self.state, self.host, self.node)?;
        skills_binding::refresh(self)?;
        let player = self
            .state
            .actor(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?
            .queue
            .entity();
        self.send(updates::run_energy(player.energy))?;
        self.send(updates::varp(173, i32::from(player.run_toggled)))?;
        let style = self.row()?.settings().fields.read().attack_style_index;
        self.send(updates::varp(43, style))?;
        let energy = self.row()?.settings().fields.read().special_energy;
        self.send(updates::varp(300, energy * 10))?;
        ui::open_default_tabs(self, m)?;
        ui::open_info_bars(self, m)
    }
    fn login_plugins(&mut self) -> Result<()> {
        Ok(())
    }
    fn appearance_sync(&mut self) -> Result<()> {
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let player = self
            .state
            .player_fields(&owner)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let equipment = player.equipment();
        if !equipment.empty() {
            let weapon = equipment.get(rs09_server::inventory::equipment::WEAPON);
            equipment_binding::refresh(self, &player, weapon.as_ref())?;
        }
        Ok(())
    }
}
#[cfg(test)]
pub(super) fn admit(p: &mut Probe, node: Node, client: ClientInfo) -> Result<()> {
    admit_with_profile(p, node, client, None)
}

pub(super) fn admit_with_profile(
    p: &mut Probe,
    node: Node,
    client: ClientInfo,
    saved: Option<&super::profiles::Snapshot>,
) -> Result<()> {
    if let Some(archive) = p.host.ui.as_ref().and_then(|ui| ui.variable_assets.clone()) {
        p.state
            .bind_varbit_assets(archive)
            .map_err(|e| Error::Binding(e.to_string()))?;
    }
    // LoginConfiguration.configureLobby starts with updateSceneGraph(true),
    // before either lobby or game configuration. This controlled session enters
    // the game directly; retain the original rebuild-before-interface order.
    let index = p
        .state
        .actor(node)
        .map_err(|e| Error::Binding(e.to_string()))?
        .wire_index;
    let actor = p.actors.get(&index).ok_or(Error::Null)?;
    let masks = actor.masks.clone();
    let initial = super::render(&p.actors, &p.order, &p.keys, &actor.player, true)
        .map_err(|e| Error::Binding(e.to_string()))?;
    p.host.frames.entry(node).or_default().extend(initial);
    let owner = p
        .state
        .retain_node(node)
        .map_err(|e| Error::Binding(e.to_string()))?;
    let store = p.host.ui.as_mut().ok_or(Error::Null)?;
    let mut inventory = store
        .factory
        .create(28, ContainerType::Default, SortType::Id)
        .map_err(|e| Error::Binding(format!("{e:?}")))?;
    inventory.register(Some(
        rs09_server::inventory::container_listener::Handle::new(
            rs09_server::inventory::listener::Listener::new(Some(owner.clone())),
        ),
    ));
    let equipment = store
        .factory
        .create(14, ContainerType::Default, SortType::Id)
        .map_err(|e| Error::Binding(format!("{e:?}")))?;
    let m = Manager::default();
    let inventory_binding = rs09_server::inventory::player_binding::Capture::new(
        owner,
        inventory,
        equipment,
        Default::default(),
        store.factory.null_item.clone(),
    );
    let has_pouch = item::Shared::new(false);
    let owned = rs09_server::player::owned::Player::new(
        p.state.identity(),
        inventory_binding,
        store.definitions.clone(),
        m.clone(),
        has_pouch.clone(),
        masks,
    )
    .map_err(|e| Error::Binding(format!("{e:?}")))?;
    p.state
        .bind_player_fields(owned.clone())
        .map_err(|e| Error::Binding(e.to_string()))?;
    let manager = owned.manager();
    let inventory_binding = inventory_binding::Capture::new(owned);
    store.players.insert(
        node,
        Player {
            manager,
            client,
            weapon: None,
            restore_special_at: p.state.ticks.wrapping_add(51),
            inventory_binding,
            skills: None,
            last_ping: super::now(),
            last_walk: None,
            bank_probe_opened: false,
            shop_route: None,
            shop_view: None,
        },
    );
    let mut ctx = UiContext::new(p, node);
    skills_binding::initialize(&mut ctx)?;
    if let Some(saved) = saved {
        restore_core(&mut ctx, saved)?;
        skills_binding::restore(&mut ctx, &saved.skills)?;
    }
    ui::send_game_configuration(&mut ctx, &m)
}

fn restore_core(ctx: &mut UiContext<'_>, saved: &super::profiles::Snapshot) -> Result<()> {
    let owner = ctx
        .state
        .retain_node(ctx.node)
        .map_err(|e| Error::Binding(e.to_string()))?;
    let fields = ctx
        .state
        .player_fields(&owner)
        .map_err(|e| Error::Binding(e.to_string()))?;
    for (mut c, value) in [
        (fields.inventory(), &saved.inventory),
        (fields.equipment(), &saved.equipment),
        (fields.bank_primary().container(), &saved.primary),
        (fields.bank_secondary().container(), &saved.secondary),
    ] {
        c.parse(value, &fields.definitions())
            .map_err(|e| Error::Binding(format!("{e:?}")))?;
    }
    *fields.bank_primary().tab_start_slots().write() = saved.primary_tabs.clone();
    *fields.bank_secondary().tab_start_slots().write() = saved.secondary_tabs.clone();
    fields.set_use_secondary_bank(saved.use_secondary);
    let weapon = fields
        .equipment()
        .get(rs09_server::inventory::equipment::WEAPON);
    let interface = super::weapons::INTERFACES[super::weapons::ordinal(weapon.as_ref())];
    fields.settings().fields.write().attack_style_index =
        saved.attack_style.min(interface.styles.len() - 1) as i32;
    fields.settings().fields.write().special_energy = saved.special_energy.clamp(0, 100);
    Ok(())
}

pub(super) fn snapshot(p: &mut Probe, name: &str, node: Node) -> Result<super::profiles::Snapshot> {
    use rs09_server::inventory::container::Container;
    let owner = p
        .state
        .retain_node(node)
        .map_err(|e| Error::Binding(e.to_string()))?;
    let fields = p
        .state
        .player_fields(&owner)
        .map_err(|e| Error::Binding(e.to_string()))?;
    let location = p
        .state
        .node_location(node)
        .map_err(|e| Error::Binding(e.to_string()))?
        .ok_or(Error::Null)?
        .coordinates();
    let ui = p
        .host
        .ui
        .as_ref()
        .and_then(|u| u.players.get(&node))
        .ok_or(Error::Null)?;
    let mut skills = serde_json::Map::new();
    ui.skills
        .as_ref()
        .ok_or(Error::Null)?
        .save_profile(&mut skills);
    let items = |c: Container| {
        serde_json::Value::Array(c.items().read().iter().enumerate().filter_map(|(slot, item)| {
        let item = item.as_ref()?.read();
        Some(serde_json::json!({"slot":slot,"id":item.id(),"amount":item.amount(),"charge":item.charge()}))
    }).collect())
    };
    Ok(super::profiles::Snapshot {
        version: 1,
        name: name.into(),
        location,
        skills: skills.into(),
        inventory: items(fields.inventory()),
        equipment: items(fields.equipment()),
        primary: items(fields.bank_primary().container()),
        secondary: items(fields.bank_secondary().container()),
        primary_tabs: fields.bank_primary().tab_start_slots().read().clone(),
        secondary_tabs: fields.bank_secondary().tab_start_slots().read().clone(),
        use_secondary: fields.use_secondary_bank(),
        attack_style: fields.settings().fields.read().attack_style_index.max(0) as usize,
        special_energy: fields.settings().fields.read().special_energy,
    })
}
pub(super) fn melee_stats(p: &Probe, node: Node) -> Option<([i32; 4], i32, i32)> {
    let player = p.host.ui.as_ref()?.players.get(&node)?;
    let skills = player.skills.as_ref()?;
    let style = player.settings().fields.read().attack_style_index;
    Some((
        [
            skills.dynamic_levels[0],
            skills.dynamic_levels[1],
            skills.dynamic_levels[2],
            skills.dynamic_levels[4],
        ],
        skills.lifepoints,
        style,
    ))
}

pub(super) fn equipment(p: &Probe, node: Node) -> Option<Container> {
    Some(p.host.ui.as_ref()?.players.get(&node)?.equipment())
}

pub(super) fn special_armed(p: &Probe, node: Node) -> bool {
    p.host
        .ui
        .as_ref()
        .and_then(|ui| ui.players.get(&node))
        .is_some_and(|row| row.settings().fields.read().special_toggled)
}

pub(super) fn drain_special(p: &mut Probe, node: Node, cost: i32) -> Result<bool> {
    let mut ctx = UiContext::new(p, node);
    let settings = ctx.row()?.settings();
    let energy = {
        let mut fields = settings.fields.write();
        fields.special_toggled = false;
        if fields.special_energy >= cost {
            fields.special_energy -= cost;
            Some(fields.special_energy)
        } else {
            None
        }
    };
    ctx.send(updates::varp(301, 0))?;
    if let Some(value) = energy {
        ctx.send(updates::varp(300, value * 10))?;
    } else {
        super::combat_services::message(
            p,
            node,
            "You do not have enough special attack energy left.",
        );
    }
    Ok(energy.is_some())
}

pub(super) fn restore_special(p: &mut Probe, node: Node) -> Result<()> {
    let now = p.state.ticks;
    let mut ctx = UiContext::new(p, node);
    let Ok(row) = ctx.row_mut() else {
        return Ok(());
    };
    if now.wrapping_sub(row.restore_special_at) < 0 {
        return Ok(());
    }
    row.restore_special_at = now.wrapping_add(51);
    let settings = row.settings();
    let mut fields = settings.fields.write();
    if fields.special_energy < 100 {
        fields.special_energy = (fields.special_energy + 10).min(100);
        ctx.send(updates::varp(300, fields.special_energy * 10))?;
    }
    Ok(())
}

pub(super) fn consume_ammo(p: &mut Probe, node: Node, slot: i32, amount: i32) -> Result<()> {
    equipment_binding::consume_ammo(&mut UiContext::new(p, node), slot, amount)
}

pub(super) fn skill_level(p: &Probe, node: Node, slot: usize) -> Option<i32> {
    p.host
        .ui
        .as_ref()?
        .players
        .get(&node)?
        .skills
        .as_ref()?
        .dynamic_levels
        .get(slot)
        .copied()
}

pub(super) fn experience(p: &mut Probe, node: Node, slot: i32, xp: f64) -> Result<()> {
    skills_binding::award(&mut UiContext::new(p, node), slot, xp)
}

pub(super) fn melee_experience(
    p: &mut Probe,
    node: Node,
    style: super::weapons::Stance,
    hit: i32,
) -> Result<()> {
    skills_binding::award_combat(&mut UiContext::new(p, node), style, hit)
}

pub(super) fn melee_damage(p: &mut Probe, node: Node, damage: i32) -> Result<(i32, i32)> {
    skills_binding::take_melee_hit(&mut UiContext::new(p, node), damage)
}

pub(super) fn revive_melee_player(p: &mut Probe, node: Node) -> Result<()> {
    skills_binding::revive_melee_player(&mut UiContext::new(p, node))
}

pub(super) fn process(p: &mut Probe, node: Node, action: &ClientAction) -> Result<bool> {
    if p.host.ui.is_none() {
        return Ok(false);
    }
    let mut ctx = UiContext::new(p, node);
    match action {
        ClientAction::ItemExamine { id } => shops::examine(&mut ctx, *id),
        ClientAction::ChatSetting {
            public,
            private,
            trade,
        } => {
            ctx.row()?
                .settings()
                .set_chat_settings(*public, *private, *trade);
            ctx.send(Frame {
                opcode: 232,
                payload: vec![*public as u8, *private as u8, *trade as u8],
            })?;
            Ok(true)
        }
        ClientAction::ItemAction {
            opt_index,
            item_id,
            slot,
            iface: 149,
            ..
        } => {
            let Some(item) = ctx.row()?.inventory().get(*slot) else {
                return Ok(false);
            };
            if item.read().id() != *item_id || !equipment_binding::equip_option(&item, *opt_index) {
                return Ok(false);
            }
            if super::script_services::locked(
                ctx.state,
                node,
                rs09_server::world::action_locks::Kind::Interaction,
            )
            .map_err(|e| Error::Binding(format!("{e:?}")))?
            {
                return Ok(true);
            }
            let owner = ctx
                .state
                .retain_node(node)
                .map_err(|e| Error::Binding(e.to_string()))?;
            let player = ctx
                .state
                .player_fields(&owner)
                .map_err(|e| Error::Binding(e.to_string()))?;
            equipment_binding::change(&mut ctx, &player, *slot, *item_id, true)?;
            Ok(true)
        }
        ClientAction::InputIntResponse { response } => {
            ctx.input_response(rs09_server::world::input_prompt::Response::Int(*response))?;
            Ok(true)
        }
        ClientAction::InputStringResponse { response } => {
            ctx.input_response(rs09_server::world::input_prompt::Response::Text(
                response.encode_utf16().collect(),
            ))?;
            Ok(true)
        }
        ClientAction::ContinueOption {
            iface,
            child,
            slot,
            opcode,
        } => {
            let owner = ctx
                .state
                .retain_node(node)
                .map_err(|e| Error::Binding(e.to_string()))?;
            rs09_server::world::interface_packets::continue_option(
                &mut ctx,
                &owner,
                rs09_server::world::interface_packets::Action {
                    iface: *iface,
                    child: *child,
                    slot: *slot,
                    opcode: *opcode,
                    opt_index: 0,
                    item: -1,
                },
            )?;
            Ok(true)
        }
        ClientAction::IfAction {
            opcode,
            opt_index,
            iface,
            child,
            slot,
            item_id,
        } => {
            let owner = ctx
                .state
                .retain_node(node)
                .map_err(|e| Error::Binding(e.to_string()))?;
            rs09_server::world::interface_packets::if_action(
                &mut ctx,
                &owner,
                rs09_server::world::interface_packets::Action {
                    opcode: *opcode,
                    opt_index: *opt_index,
                    iface: *iface,
                    child: *child,
                    slot: *slot,
                    item: *item_id,
                },
            )?;
            Ok(true)
        }
        ClientAction::CloseIface => {
            let owner = ctx
                .state
                .retain_node(node)
                .map_err(|e| Error::Binding(e.to_string()))?;
            rs09_server::world::interface_packets::close_iface(&mut ctx, &owner)?;
            Ok(true)
        }
        ClientAction::Ping => {
            ctx.row_mut()?.last_ping = super::now();
            Ok(true)
        }
        ClientAction::TrackingDisplayUpdate {
            window_mode,
            screen_width,
            screen_height,
            display_mode,
        } => {
            {
                let c = &mut ctx.row_mut()?.client;
                c.screen_width = *screen_width;
                c.screen_height = *screen_height;
                c.display_mode = *display_mode;
            }
            let m = ctx.row()?.manager.clone();
            ui::switch_window_mode(&mut ctx, &m, *window_mode)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
pub(super) fn reap(host: &mut Host, state: &super::State) {
    if let Some(ui) = &mut host.ui {
        // Metadata must not root disconnected actors. Exposed inventory/settings
        // views retain the exact retired component independently of this table.
        ui.players
            .retain(|n, _| state.actor(*n).is_ok_and(|a| a.connected));
    }
}
/// Opt-in test controller: choose bank-open timing after an actual1351 pickup.
/// It never inserts items or handles transactions outside the original listener.
pub(super) fn bank_probe(p: &mut Probe) -> Result<()> {
    let nodes = p
        .host
        .ui
        .as_ref()
        .filter(|ui| ui.bank_probe)
        .map(|ui| {
            ui.players
                .iter()
                .filter_map(|(&node, row)| (!row.bank_probe_opened).then_some(node))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for node in nodes {
        let owner = p
            .state
            .retain_node(node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        let player = p
            .state
            .player_fields(&owner)
            .map_err(|e| Error::Binding(e.to_string()))?;
        if player.inventory().contains(1351, 1) {
            p.host
                .ui
                .as_mut()
                .ok_or(Error::Null)?
                .players
                .get_mut(&node)
                .ok_or(Error::Null)?
                .bank_probe_opened = true;
            player
                .bank()
                .open_bank(&mut UiContext::new(p, node))
                .map_err(|e| Error::Binding(format!("{e:?}")))?;
        }
    }
    Ok(())
}
pub(super) fn record_action(p: &mut Probe, node: Node, opcode: u8, action: &ClientAction) {
    let Some(ui) = &mut p.host.ui else {
        return;
    };
    let Some(row) = ui.players.get_mut(&node) else {
        return;
    };
    let (dest_x, dest_y, is_run) = match action {
        ClientAction::WorldspaceWalk {
            dest_x,
            dest_y,
            is_run,
        }
        | ClientAction::MinimapWalk {
            dest_x,
            dest_y,
            is_run,
            ..
        }
        | ClientAction::InteractWalk {
            dest_x,
            dest_y,
            is_run,
        } => (*dest_x, *dest_y, *is_run),
        _ => return,
    };
    row.last_walk = Some(
        serde_json::json!({"opcode":opcode,"destination":[dest_x,dest_y],"running":is_run,"handledAtTick":p.state.ticks}),
    );
}
pub(super) fn trace_state(host: &Host, node: Node) -> Option<serde_json::Value> {
    let row = host.ui.as_ref()?.players.get(&node)?;
    Some(
        serde_json::json!({"root":row.manager.pane_id(),"counter":row.manager.fields.read().packet_count,"mode":row.client.window_mode,"width":row.client.screen_width,"height":row.client.screen_height,"inventoryOccupied":row.inventory().items().read().iter().flatten().count(),"lastWalk":row.last_walk,"skills":row.skills.as_ref().map(|s|serde_json::json!({"static":s.static_levels,"dynamic":s.dynamic_levels,"experience":s.experience,"prayer":s.prayer_points,"hitpoints":s.lifepoints}))}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::game::updates::ServerUpdate;

    #[test]
    fn run_buttons_change_the_authoritative_queue_and_report_energy_use() {
        use rs09_server::world::{update::Context as _, walking::Point};

        let (mut p, node, _) = super::super::tests::probe();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        p.host.ui = Some(Ui::load(&path).unwrap());
        admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1024,
                screen_height: 768,
            },
        )
        .unwrap();
        let mut queue = p.state.actor(node).unwrap().queue.clone();
        queue.state.write().entity.update_scene = false;
        queue
            .points()
            .write()
            .push_back(item::Shared::new(Point::origin(3207, 3216)));
        for x in 3208..=3211 {
            queue.add_point(x, 3216, false);
        }

        let click = |p: &mut Probe, iface, child| {
            let mut writer = rs09_protocol::buffer::Writer::new();
            writer.p4((iface << 16) | child);
            writer.p2(-1);
            let action = ClientAction::decode(
                155,
                &mut rs09_protocol::buffer::Reader::new(&writer.into_vec()),
            )
            .unwrap()
            .unwrap();
            assert!(process(p, node, &action).unwrap());
        };
        p.host.frames.clear();
        click(&mut p, 750, 0);
        assert!(queue.entity().run_toggled);
        assert!(p.host.frames[&node].contains(&updates::varp(173, 1)));
        p.player_tick(node).unwrap();
        assert_eq!(
            queue.entity().location.unwrap().coordinates(),
            [3209, 3216, 0]
        );
        assert!(queue.entity().energy < 100.0);
        p.player_reset(node).unwrap();

        click(&mut p, 261, 3);
        assert!(!queue.entity().run_toggled);
        assert!(p.host.frames[&node].contains(&updates::varp(173, 0)));
        p.player_tick(node).unwrap();
        assert_eq!(
            queue.entity().location.unwrap().coordinates(),
            [3210, 3216, 0]
        );
    }

    #[test]
    fn manager_lifecycle_closes_real_pin_state_and_rejects_foreign_receivers_before_packets() {
        let (mut p, node, _) = super::super::tests::probe();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        p.host.ui = Some(Ui::load(&path).unwrap());
        admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let owner = p.state.retain_node(node).unwrap();
        let fields = p.state.player_fields(&owner).unwrap();
        let manager = fields.manager();
        let pin = fields.bank_pin();
        pin.fields().write().change_state = 2;
        pin.fields().write().deleting = true;
        fields.attributes().set_integer("runscript", 7);
        let component = p.host.ui.as_mut().unwrap().registry.component(13);
        component.write().close_event =
            Some(rs09_server::player::bank_pin::CloseEvent::new(pin.clone()).component_event());
        manager.fields.write().opened = Some(component);
        p.host.frames.clear();
        let count = manager.packet_count(0);
        {
            let mut ctx = UiContext::new(&mut p, node);
            assert!(ui::close(&mut ctx, &manager).unwrap());
        }
        assert!(manager.fields.read().opened.is_none());
        assert!(!fields.attributes().contains_key("runscript"));
        assert_eq!(pin.fields().read().change_state, 0);
        assert!(!pin.fields().read().deleting);
        assert_eq!(
            p.host.frames[&node],
            vec![
                updates::run_client_script(101, b"", &[], count).unwrap(),
                updates::close_interface(count.wrapping_add(1), (746 << 16) | 6),
            ]
        );
        let (mut foreign, foreign_node, _) = super::super::tests::probe();
        foreign.host.ui = Some(Ui::load(&path).unwrap());
        admit(
            &mut foreign,
            foreign_node,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let foreign_owner = foreign.state.retain_node(foreign_node).unwrap();
        let foreign_fields = foreign.state.player_fields(&foreign_owner).unwrap();
        foreign_fields.attributes().set_integer("runscript", 8);
        let foreign_manager = foreign_fields.manager();
        let component = p.host.ui.as_mut().unwrap().registry.component(999);
        let before = component.read().definition.as_ref().unwrap().read().kind;
        p.host.frames.clear();
        {
            let mut ctx = UiContext::new(&mut p, node);
            assert!(matches!(
                ui::close(&mut ctx, &foreign_manager),
                Err(Error::Binding(_))
            ));
            assert!(matches!(
                ui::open_single_tab(&mut ctx, &foreign_manager, &component),
                Err(Error::Binding(_))
            ));
        }
        assert_eq!(
            component.read().definition.as_ref().unwrap().read().kind,
            before
        );
        assert_eq!(foreign_fields.attributes().integer("runscript", 0), Ok(8));
        assert!(p.host.frames.is_empty());
    }
    #[test]
    fn typed_pin_plugins_close_callbacks_and_unclosable_guards_use_real_canonical_state() {
        use rs09_server::player::bank_pin::{CloseEvent, Program};
        let (mut p, node, _) = super::super::tests::probe();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        p.host.ui = Some(Ui::load(&path).unwrap());
        admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 1,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let owner = p.state.retain_node(node).unwrap();
        let fields = p.state.player_fields(&owner).unwrap();
        let program = Program::default();
        let plugin = program.component_plugin();
        {
            let registry = &mut p.host.ui.as_mut().unwrap().registry;
            registry.for_id(13).write().plugin = Some(plugin.clone());
            registry.for_id(14).write().plugin = Some(plugin.clone());
        }
        let component = p.host.ui.as_mut().unwrap().registry.component(13);
        assert!(component.read().plugin.as_ref().unwrap().same(&plugin));
        assert!(
            Program::from_component_plugin(component.read().plugin.as_ref().unwrap())
                .unwrap()
                .same(&program)
        );
        let event = CloseEvent::new(fields.bank_pin()).component_event();
        component.write().close_event = Some(event.clone());
        fields.bank_pin().fields().write().change_state = 2;
        fields.bank_pin().fields().write().deleting = true;
        p.host.frames.clear();
        {
            let manager = fields.manager();
            let mut ctx = UiContext::new(&mut p, node);
            ui::component_open(&mut ctx, &manager, &component).unwrap();
            assert!(ui::component_close(&mut ctx, &component).unwrap());
        }
        assert!(p.host.frames[&node].iter().any(|frame| frame.opcode == 155));
        assert_eq!(fields.bank_pin().fields().read().change_state, 0);
        assert!(!fields.bank_pin().fields().read().deleting);
        {
            let mut ctx = UiContext::new(&mut p, node);
            assert!(matches!(
                ui::set_unclosable(&mut ctx, None),
                Err(Error::Null)
            ));
            assert_eq!(fields.attributes().boolean("close_c_", false), Ok(true));
            ui::set_unclosable(&mut ctx, Some(&component)).unwrap();
            assert!(!ui::component_close(&mut ctx, &component).unwrap());
            fields.attributes().set_boolean("close_c_", false);
            assert!(ui::component_close(&mut ctx, &component).unwrap());
            fields.attributes().set_integer("close_c_", 1);
            assert!(matches!(
                ui::component_close(&mut ctx, &component),
                Err(Error::Type)
            ));
        }
        fields.bank_pin().fields().write().change_state = 7;
        {
            let mut ctx = UiContext::new(&mut p, node);
            assert!(ctx.close_event(event, &component).unwrap());
        }
        assert_eq!(fields.bank_pin().fields().read().change_state, 0);
    }
    #[test]
    fn foreign_pin_close_callbacks_fail_before_mutating_their_captured_manager() {
        use rs09_server::player::bank_pin::CloseEvent;
        fn setup() -> (Probe, Node) {
            let (mut p, node, _) = super::super::tests::probe();
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/interfaces/interface_configs.json");
            p.host.ui = Some(Ui::load(&path).unwrap());
            admit(
                &mut p,
                node,
                ClientInfo {
                    display_mode: 0,
                    window_mode: 1,
                    screen_width: 1280,
                    screen_height: 720,
                },
            )
            .unwrap();
            (p, node)
        }
        let (mut local, node) = setup();
        let (mut foreign, foreign_node) = setup();
        let owner = foreign.state.retain_node(foreign_node).unwrap();
        let fields = foreign.state.player_fields(&owner).unwrap();
        fields.bank_pin().fields().write().change_state = 9;
        fields.bank_pin().fields().write().deleting = true;
        let component = local.host.ui.as_mut().unwrap().registry.component(13);
        component.write().close_event = Some(CloseEvent::new(fields.bank_pin()).component_event());
        let mut ctx = UiContext::new(&mut local, node);
        assert!(matches!(
            ui::component_close(&mut ctx, &component),
            Err(Error::Binding(_))
        ));
        assert_eq!(fields.bank_pin().fields().read().change_state, 9);
        assert!(fields.bank_pin().fields().read().deleting);
    }
    #[test]
    fn original_chatbox_varp_mutates_the_canonical_store_and_sends_its_frame() {
        let (mut p, node, _) = super::super::tests::probe();
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        p.host.ui = Some(Ui::load(&path).unwrap());
        admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let owner = p.state.retain_node(node).unwrap();
        let fields = p.state.player_fields(&owner).unwrap();
        fields.variables().values().unwrap().clear();
        p.host.frames.clear();
        let manager = fields.manager();
        let mut ctx = UiContext::new(&mut p, node);
        let component = ctx.component(137).unwrap();
        ui::open_chatbox(&mut ctx, &manager, &component).unwrap();
        assert_eq!(fields.variables().get_varp(334), Ok(1));
        assert!(!fields.variables().saved().unwrap().contains_key(334));
        assert_eq!(p.host.frames[&node].last(), Some(&updates::varp(334, 1)));
    }
    #[test]
    fn a_retained_listener_alone_reaches_the_original_inventory_settings_body_and_counter() {
        use rs09_server::{
            definitions::{item::ItemDefinition, property::Property},
            inventory::{item::Item, listener::Listener},
        };
        let (mut p, node, _) = super::super::tests::probe();
        let mut defs = item::Registry::default();
        let mut d = ItemDefinition {
            id: 12047,
            ..Default::default()
        };
        d.handlers.insert("weight".into(), Property::Double(1.75));
        defs.insert(12047, item::Shared::new(d));
        let defs = Arc::new(defs);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        let mut ui = Ui::load(&path).unwrap();
        ui.use_definitions(defs.clone()).unwrap();
        p.host.ui = Some(ui);
        admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 1,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let listener = p.host.ui.as_ref().unwrap().players[&node]
            .inventory()
            .listener_list()
            .get(0)
            .unwrap()
            .unwrap();
        let fields = p
            .state
            .player_fields(&listener.value::<Listener>().unwrap().owner().unwrap())
            .unwrap();
        let weak_fields = fields.downgrade();
        let weak_body = fields.masks().body().downgrade();
        let counter = fields.manager().fields.read().packet_count;
        drop(fields);
        p.remove(1, node).unwrap();
        for _ in 0..2 {
            super::super::tick::handle(&mut p, false).unwrap();
            reap(&mut p.host, &p.state);
            p.state.collect();
        }
        assert!(p.actors.is_empty());
        assert!(p.host.ui.as_ref().unwrap().players.is_empty());
        assert!(p.state.actor(node).is_ok());
        p.host.frames.clear();
        let captured = inventory_binding::from_listener(&p, &listener).unwrap();
        let pouch = item::Shared::new(Item::new(12047, 1, 1000, &defs));
        captured
            .apply(&mut p, |inventory, defs, ctx| {
                inventory.add(Some(&pouch), true, -1, defs, ctx)
            })
            .unwrap();
        assert_eq!(captured.settings().fields.read().weight, 1.75);
        assert!(weak_body.upgrade().unwrap().read().using_summoning);
        assert!(
            matches!(ServerUpdate::decode(&p.host.frames[&node][1]).unwrap(), Some(ServerUpdate::InterfaceText { text, counter: current, .. }) if text == b"1 kg" && current == (counter & 65535))
        );
        super::super::tick::Context::flush(&mut p).unwrap();
        drop(captured);
        p.state.collect();
        assert!(p.state.actor(node).is_ok());
        drop(listener);
        p.state.collect();
        assert!(p.state.actor(node).is_err());
        assert!(weak_fields.upgrade().is_none());
        assert!(weak_body.upgrade().is_none());
    }
    #[test]
    fn captured_inventory_keeps_retired_fields_and_never_sends_to_a_reused_wire_index() {
        use rs09_server::{
            definitions::{item::ItemDefinition, property::Property},
            inventory::item::Item,
            world::{appearance, doors::Location, player_render::Player as RenderPlayer},
        };
        let (mut p, node, _) = super::super::tests::probe();
        let mut definitions = item::Registry::default();
        let mut d = ItemDefinition {
            id: 12047,
            ..Default::default()
        };
        d.handlers.insert("weight".into(), Property::Double(1.75));
        definitions.insert(12047, item::Shared::new(d));
        let definitions = Arc::new(definitions);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        let mut ui = Ui::load(&path).unwrap();
        ui.use_definitions(definitions.clone()).unwrap();
        p.host.ui = Some(ui);
        let client = ClientInfo {
            display_mode: 0,
            window_mode: 1,
            screen_width: 1280,
            screen_height: 720,
        };
        admit(&mut p, node, client.clone()).unwrap();
        let captured = inventory_binding::capture(&mut p, node).unwrap();
        let old_counter = p.host.ui.as_ref().unwrap().players[&node]
            .manager
            .fields
            .read()
            .packet_count;
        let (mut foreign, foreign_node, _) = super::super::tests::probe();
        assert_eq!(foreign_node, node);
        let mut entered = false;
        assert!(
            captured
                .apply(&mut foreign, |_, _, _| {
                    entered = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!entered);
        assert!(foreign.host.frames.is_empty());
        let (old_inventory, old_settings) = captured
            .apply(&mut p, |inventory, _, ctx| {
                Ok((inventory.clone(), ctx.links.settings.clone()))
            })
            .unwrap();
        let old_player = p.actors[&1].player.downgrade();
        let old_body = p.actors[&1].masks.body().downgrade();
        p.remove(1, node).unwrap();
        for _ in 0..2 {
            super::super::tick::handle(&mut p, false).unwrap();
            reap(&mut p.host, &p.state);
            p.state.collect();
        }
        assert!(p.actors.is_empty());
        assert!(p.host.ui.as_ref().unwrap().players.is_empty());
        assert!(p.state.actor(node).is_ok());
        let location = Location::new(3207, 3216, 0);
        let next = p.state.add(1, location.clone(), [1, 2, 3, 4]);
        let (_, input) = tokio::sync::mpsc::channel(1);
        let (output, mut received) = tokio::sync::mpsc::channel(2);
        let actor = super::super::Actor::new(
            item::Shared::new(RenderPlayer::new(1, location, 2000)),
            appearance::Data::naked("replacement".into(), 0),
            input,
            output,
            "replacement".into(),
            &p.host.animations.factory,
        );
        p.host.masks.insert(next, actor.masks.clone());
        p.host.animators.insert(next, actor.animator.clone());
        p.actors.insert(1, actor);
        p.nodes.insert(1, next);
        p.order.push(1);
        p.leases.insert(next, p.state.retain_node(next).unwrap());
        admit(&mut p, next, client).unwrap();
        p.host.frames.clear();
        let replacement = p.host.ui.as_ref().unwrap().players[&next].inventory();
        assert!(!old_inventory.same(&replacement));
        let new_counter = p.host.ui.as_ref().unwrap().players[&next]
            .manager
            .fields
            .read()
            .packet_count;
        let pouch = item::Shared::new(Item::new(12047, 1, 1000, &definitions));
        captured
            .apply(&mut p, |inventory, defs, ctx| {
                inventory.add(Some(&pouch), true, -1, defs, ctx)
            })
            .unwrap();
        assert_eq!(old_inventory.get(0).unwrap().read().id(), 12047);
        assert_eq!(old_settings.fields.read().weight, 1.75);
        assert!(old_body.upgrade().unwrap().read().using_summoning);
        assert!(replacement.empty());
        assert_eq!(
            p.host.ui.as_ref().unwrap().players[&next]
                .settings()
                .fields
                .read()
                .weight,
            0.0
        );
        assert!(!p.actors[&1].masks.body().read().using_summoning);
        assert_eq!(
            p.host.ui.as_ref().unwrap().players[&next]
                .manager
                .fields
                .read()
                .packet_count,
            new_counter
        );
        assert_eq!(
            p.host.frames[&node]
                .iter()
                .map(|f| f.opcode)
                .collect::<Vec<_>>(),
            vec![22, 171]
        );
        assert!(
            matches!(ServerUpdate::decode(&p.host.frames[&node][1]).unwrap(),Some(ServerUpdate::InterfaceText{text,counter,..})if text==b"1 kg" && counter==(old_counter&65535))
        );
        super::super::tick::Context::flush(&mut p).unwrap();
        assert!(received.try_recv().is_err());
        assert!(p.host.frames.is_empty());
        drop(captured);
        p.state.collect();
        assert!(p.state.actor(node).is_ok());
        assert!(old_player.upgrade().is_some());
        drop(old_inventory);
        drop(old_settings);
        p.state.collect();
        assert!(p.state.actor(node).is_err());
        assert!(old_player.upgrade().is_none());
        assert!(old_body.upgrade().is_none());
        assert!(p.host.errors.is_empty());
    }
    #[test]
    fn native_inventory_listener_uses_shared_definitions_weight_and_real_appearance() {
        use rs09_server::{
            definitions::{
                item::{ItemDefinition, Shared},
                property::Property,
            },
            inventory::item::Item,
        };
        let (mut p, node, _) = super::super::tests::probe();
        let mut definitions = item::Registry::default();
        for (id, weight, stackable) in [(1, 2.5, true), (2, -5.0, false), (12047, 1.75, false)] {
            let mut d = ItemDefinition {
                id,
                stackable,
                ..Default::default()
            };
            d.handlers.insert("weight".into(), Property::Double(weight));
            definitions.insert(id, Shared::new(d));
        }
        let definitions = Arc::new(definitions);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        let mut ui = Ui::load(&path).unwrap();
        ui.use_definitions(definitions.clone()).unwrap();
        p.host.ui = Some(ui);
        admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let initial = p.host.frames.remove(&node).unwrap();
        let inventory = initial.iter().position(|f| f.opcode == 105).unwrap();
        let weight = initial.iter().position(|f| matches!(ServerUpdate::decode(f).unwrap(),Some(ServerUpdate::InterfaceText{pointer,..})if pointer==(667<<16|32))).unwrap();
        let skills = initial
            .iter()
            .position(|f| {
                matches!(
                    ServerUpdate::decode(f).unwrap(),
                    Some(ServerUpdate::Skill { .. })
                )
            })
            .unwrap();
        assert!(inventory < weight && weight < skills);
        {
            let row = p.host.ui.as_mut().unwrap().players.get_mut(&node).unwrap();
            row.equipment().items().write()[0] =
                Some(Shared::new(Item::new(2, 1, 1000, &definitions)));
            row.equipment().items().write()[12] =
                Some(Shared::new(Item::new(1, 1, 1000, &definitions)));
        }
        let ordinary = Shared::new(Item::new(1, 100, 1000, &definitions));
        inventory_binding::apply(&mut p, node, |inventory, definitions, context| {
            inventory.add(Some(&ordinary), true, -1, definitions, context)
        })
        .unwrap();
        assert_eq!(
            p.host.ui.as_ref().unwrap().players[&node]
                .settings()
                .fields
                .read()
                .weight,
            -2.5
        );
        assert_eq!(
            p.host.frames[&node]
                .iter()
                .map(|f| f.opcode)
                .collect::<Vec<_>>(),
            vec![22, 171]
        );
        assert!(!p.host.ui.as_ref().unwrap().players[&node].has_pouch());
        assert!(!p.actors[&1].masks.body().read().using_summoning);
        p.host.frames.remove(&node);
        // The actual definition handle is shared; the next scan observes its
        // changed field, with no copied definition table or weight snapshot.
        definitions
            .for_id(1)
            .write()
            .handlers
            .insert("weight".into(), Property::Double(4.75));
        let pouch = Shared::new(Item::new(12047, 1, 1000, &definitions));
        p.actors[&1].masks.reset();
        inventory_binding::apply(&mut p, node, |inventory, definitions, context| {
            inventory.add(Some(&pouch), true, -1, definitions, context)
        })
        .unwrap();
        assert_eq!(
            p.host.ui.as_ref().unwrap().players[&node]
                .settings()
                .fields
                .read()
                .weight,
            1.5
        );
        assert!(p.host.ui.as_ref().unwrap().players[&node].has_pouch());
        assert!(p.actors[&1].masks.body().read().using_summoning);
        assert!(p.actors[&1].player.read().masks.required);
        inventory_binding::apply(&mut p, node, |inventory, definitions, context| {
            inventory.clear(false, definitions, context)?;
            inventory.refresh(None, definitions, context)
        })
        .unwrap();
        assert_eq!(
            p.host.ui.as_ref().unwrap().players[&node]
                .settings()
                .fields
                .read()
                .weight,
            -5.0
        );
        assert!(!p.host.ui.as_ref().unwrap().players[&node].has_pouch());
        assert!(!p.actors[&1].masks.body().read().using_summoning);
        p.remove(1, node).unwrap();
        super::super::tick::handle(&mut p, false).unwrap();
        reap(&mut p.host, &p.state);
        p.state.collect();
        assert!(p.host.ui.as_ref().unwrap().players.is_empty());
        assert!(
            p.state.actor(node).is_err(),
            "listener metadata must release its node lease"
        );
    }
    #[test]
    fn initial_scene_precedes_gameframe_and_next_tick_player_appearance() {
        let (mut p, node, mut output) = super::super::tests::probe();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        p.host.ui = Some(Ui::load(&root).unwrap());
        admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        super::super::tick::handle(&mut p, false).unwrap();
        let initial = output.try_recv().unwrap();
        assert_eq!(
            initial[0].opcode, 162,
            "strict client bootstrap requires rebuild first"
        );
        let rebuild = rs09_protocol::game::rebuild::decode_normal(&initial[0]).unwrap();
        assert_eq!(rebuild.base(), [3152, 3168]);
        assert_eq!(initial[1].opcode, 145);
        assert!(!initial.iter().any(|f| f.opcode == 225));
        super::super::tick::handle(&mut p, false).unwrap();
        let next = output.try_recv().unwrap();
        assert_eq!(next[0].opcode, 162);
        let player = next.iter().find(|f| f.opcode == 225).unwrap();
        let mut reader = rs09_protocol::game::players::State::new(0, 0);
        reader.decode_frame(player, 1).unwrap();
        assert!(reader.self_player().appearance.is_some());
        assert!(p.host.errors.is_empty(), "{:?}", p.host.errors);
    }
    #[test]
    fn original_gameframe_prefix_empty_inventory_mode_resize_ping_and_retirement() {
        let (mut p, node, _) = super::super::tests::probe();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        p.host.ui = Some(Ui::load(&root).unwrap());
        admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 1,
                window_mode: 2,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let frames = p.host.frames.remove(&node).unwrap();
        let decoded: Vec<_> = frames
            .iter()
            .filter_map(|f| ServerUpdate::decode(f).unwrap())
            .collect();
        assert!(matches!(
            decoded[0],
            ServerUpdate::WindowsPane {
                interface: 746,
                counter: 0,
                ..
            }
        ));
        let counters: Vec<_> = decoded
            .iter()
            .filter_map(|f| match f {
                ServerUpdate::WindowsPane { counter, .. }
                | ServerUpdate::OpenInterface { counter, .. }
                | ServerUpdate::InterfaceText { counter, .. } => Some(*counter),
                _ => None,
            })
            .collect();
        assert_eq!(counters, (0..counters.len() as i32).collect::<Vec<_>>());
        let inventories: Vec<_> = decoded
            .iter()
            .filter_map(|f| {
                if let ServerUpdate::InventoryFull {
                    pointer,
                    container,
                    items,
                } = f
                {
                    Some((*pointer, *container, items))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(inventories.len(), 1);
        let (pointer, container, items) = inventories[0];
        assert_eq!((pointer, container), (149 << 16, 93));
        assert_eq!(items.len(), 28);
        assert!(items.iter().all(|i| i.id == -1 && i.amount == 0));
        let skill_packets: Vec<_> = decoded
            .iter()
            .filter_map(|f| {
                if let ServerUpdate::Skill {
                    skill,
                    boosted,
                    experience,
                } = f
                {
                    Some((*skill, *boosted, *experience))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(skill_packets.len(), 24);
        for (slot, (skill, boosted, xp)) in skill_packets.into_iter().enumerate() {
            assert_eq!(skill, slot as i32);
            assert_eq!(boosted, if slot == 3 { 10 } else { 1 });
            assert_eq!(xp, if slot == 3 { 1154 } else { 0 });
        }
        let before = p.host.ui.as_ref().unwrap().players[&node]
            .manager
            .fields
            .read()
            .packet_count;
        let display = ClientAction::TrackingDisplayUpdate {
            window_mode: 2,
            screen_width: 900,
            screen_height: 600,
            display_mode: 0,
        };
        assert!(process(&mut p, node, &display).unwrap());
        assert!(p.host.frames.get(&node).is_none());
        let display = ClientAction::TrackingDisplayUpdate {
            window_mode: 1,
            screen_width: 765,
            screen_height: 503,
            display_mode: 0,
        };
        assert!(process(&mut p, node, &display).unwrap());
        let frames = p.host.frames.remove(&node).unwrap();
        assert!(
            matches!(ServerUpdate::decode(&frames[0]).unwrap(),Some(ServerUpdate::WindowsPane{interface:548,counter,..})if counter==before)
        );
        p.host
            .ui
            .as_mut()
            .unwrap()
            .players
            .get_mut(&node)
            .unwrap()
            .last_ping = 0;
        assert!(process(&mut p, node, &ClientAction::Ping).unwrap());
        assert!(p.host.ui.as_ref().unwrap().players[&node].last_ping > 0);
        p.remove(1, node).unwrap();
        super::super::tick::handle(&mut p, false).unwrap();
        p.state.collect();
        reap(&mut p.host, &p.state);
        assert!(p.host.ui.as_ref().unwrap().players.is_empty());
    }
}
