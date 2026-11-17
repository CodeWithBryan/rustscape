//! `OptionHandler.getDestination` of the global handlers' scenery handlers
//! (LadderManagingPlugin) during a movement pulse: a `global_handlers::Host`
//! over the cache scene only (the walking call holds no more of the world).
//! The geometry services (node kind and location, scenery size, rotation,
//! walking flag, teleport permission and clipping) answer from the scene;
//! every other service refuses by name.
use super::walk::Walk;
use crate::world::{
    doors::{Handle as Scenery, Location},
    global_handlers::{
        self as gh, ClanField, ClanRank, DialogueArg, Error, Expression, Extension, IronmanMode,
        Kind, LogLevel, ObjectKind, PulseType, Quest, Result, ScriptArg, Song, SpellBook, Store,
        Task, TeleportType, Toggle, Value, climb,
    },
    prayer::PrayerType,
};

fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(Some(format!(
        "not ported in the live world: global handler service {what} during a movement destination"
    ))))
}

/// The scenery a destination is asked for.
#[derive(Clone)]
pub struct SceneNode(pub Scenery);
impl PartialEq for SceneNode {
    fn eq(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
}
impl std::fmt::Debug for SceneNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scenery({})", self.0.read().id)
    }
}

pub struct Geometry<'a> {
    pub walk: &'a mut Walk,
    /// Field reads of the host (no Result) that getDestination never makes;
    /// one reached fails the destination.
    pub failures: Vec<&'static str>,
}

#[allow(unused_variables)]
impl gh::Host for Geometry<'_> {
    type Node = SceneNode;
    type Container = super::global::Absent;
    type Pulse = super::global::Absent;
    type Weapon = super::global::Absent;
    type Special = super::global::Absent;
    type Operate = super::global::Absent;
    type Quest = super::global::Absent;
    type Diary = super::global::Absent;
    type Clan = super::global::Absent;
    type Session = super::global::Absent;
    type Object = super::global::Absent;
    fn kind(&self, node: &Self::Node) -> Kind {
        let _ = node;
        Kind::Scenery
    }
    fn id(&mut self, node: &Self::Node) -> Result<i32> {
        Ok(node.0.read().id)
    }
    fn name(&mut self, node: &Self::Node) -> Result<Option<String>> {
        let definition = node.0.read().definition.clone().ok_or(Error::Null)?;
        let name = definition.read().name.clone();
        Ok(name)
    }
    fn location(&mut self, node: &Self::Node) -> Result<Option<[i32; 3]>> {
        Ok(Some(node.0.read().location.coordinates()))
    }
    fn amount(&mut self, item: &Self::Node) -> Result<i32> {
        refuse("amount")
    }
    fn lowercase_default(&self, text: &str) -> String {
        rs09_game_core::java_lower::lowercase_root(text)
    }
    fn server_name(&mut self) -> String {
        self.failures.push("server_name");
        Default::default()
    }
    fn booth_quick_open(&mut self) -> bool {
        self.failures.push("booth_quick_open");
        Default::default()
    }
    fn booth_note_enabled(&mut self) -> bool {
        self.failures.push("booth_note_enabled");
        Default::default()
    }
    fn booth_note_uim(&mut self) -> bool {
        self.failures.push("booth_note_uim");
        Default::default()
    }
    fn ticks(&mut self) -> i32 {
        self.failures.push("ticks");
        Default::default()
    }
    fn current_time_millis(&mut self) -> i64 {
        self.failures.push("current_time_millis");
        Default::default()
    }
    fn world_id(&mut self) -> Result<i32> {
        refuse("world_id")
    }
    fn get_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>> {
        refuse("get_attribute")
    }
    fn set_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
        value: Value<Self::Object>,
    ) -> Result<()> {
        refuse("set_attribute")
    }
    fn remove_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<()> {
        refuse("remove_attribute")
    }
    fn attributes_get(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>> {
        refuse("attributes_get")
    }
    fn api_get_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>> {
        refuse("api_get_attribute")
    }
    fn api_set_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
        value: Value<Self::Object>,
    ) -> Result<()> {
        refuse("api_set_attribute")
    }
    fn api_remove_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<()> {
        refuse("api_remove_attribute")
    }
    fn api_remove_attributes(&mut self, entity: &Self::Node, keys: &[&str]) -> Result<()> {
        refuse("api_remove_attributes")
    }
    fn object_kind(&mut self, object: &Self::Object) -> ObjectKind {
        match *object {}
    }
    fn selection_callback(
        &mut self,
        object: &Self::Object,
        skill: i32,
        player: &Self::Node,
    ) -> Result<()> {
        refuse("selection_callback")
    }
    fn caller_invoke(
        &mut self,
        object: &Self::Object,
        skill: i32,
        player: &Self::Node,
    ) -> Result<()> {
        refuse("caller_invoke")
    }
    fn item_select_invoke(&mut self, object: &Self::Object, slot: i32, option: i32) -> Result<()> {
        refuse("item_select_invoke")
    }
    fn api_message(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        refuse("api_message")
    }
    fn message(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        refuse("message")
    }
    fn packet_message(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        refuse("packet_message")
    }
    fn packet_message_delay(&mut self, player: &Self::Node, text: &str, ticks: i32) -> Result<()> {
        refuse("packet_message_delay")
    }
    fn packet_string(
        &mut self,
        player: &Self::Node,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        refuse("packet_string")
    }
    fn set_interface_text(
        &mut self,
        player: &Self::Node,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        refuse("set_interface_text")
    }
    fn dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        refuse("dialogue")
    }
    fn npc_dialogue(
        &mut self,
        player: &Self::Node,
        npc: i32,
        text: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()> {
        refuse("npc_dialogue")
    }
    fn item_dialogue(&mut self, player: &Self::Node, item: i32, text: &str) -> Result<()> {
        refuse("item_dialogue")
    }
    fn double_item_dialogue(
        &mut self,
        player: &Self::Node,
        first: i32,
        second: i32,
        text: &str,
    ) -> Result<()> {
        refuse("double_item_dialogue")
    }
    fn open_dialogue(
        &mut self,
        player: &Self::Node,
        dialogue: i32,
        args: Vec<DialogueArg>,
    ) -> Result<()> {
        refuse("open_dialogue")
    }
    fn dialogue_contains(&mut self, id: i32) -> Result<bool> {
        refuse("dialogue_contains")
    }
    fn open_interface(&mut self, player: &Self::Node, id: i32) -> Result<()> {
        refuse("open_interface")
    }
    fn close_interface(&mut self, player: &Self::Node) -> Result<()> {
        refuse("close_interface")
    }
    fn open_single_tab(&mut self, player: &Self::Node, id: i32) -> Result<()> {
        refuse("open_single_tab")
    }
    fn close_tab_interface(&mut self, player: &Self::Node) -> Result<()> {
        refuse("close_tab_interface")
    }
    fn open(&mut self, player: &Self::Node, id: i32) -> Result<bool> {
        refuse("open")
    }
    fn set_close_event(
        &mut self,
        player: &Self::Node,
        id: i32,
        event: Task<Self::Node, Self::Clan>,
    ) -> Result<()> {
        refuse("set_close_event")
    }
    fn opened(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("opened")
    }
    fn close_manager(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("close_manager")
    }
    fn close_single_tab(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("close_single_tab")
    }
    fn remove_tabs(&mut self, player: &Self::Node, tabs: &[i32]) -> Result<()> {
        refuse("remove_tabs")
    }
    fn open_default_tabs(&mut self, player: &Self::Node) -> Result<()> {
        refuse("open_default_tabs")
    }
    fn open_tab(&mut self, player: &Self::Node, slot: i32, id: i32) -> Result<()> {
        refuse("open_tab")
    }
    fn open_tab_weapon(&mut self, player: &Self::Node, weapon: &Self::Weapon) -> Result<()> {
        refuse("open_tab_weapon")
    }
    fn open_windows_pane(&mut self, player: &Self::Node, id: i32, kind: Option<i32>) -> Result<()> {
        refuse("open_windows_pane")
    }
    fn is_resizable(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("is_resizable")
    }
    fn component_open(&mut self, player: &Self::Node, id: i32) -> Result<()> {
        refuse("component_open")
    }
    fn run_script(
        &mut self,
        player: &Self::Node,
        id: i32,
        types: &str,
        args: Vec<ScriptArg>,
    ) -> Result<()> {
        refuse("run_script")
    }
    fn iface_settings(
        &mut self,
        player: &Self::Node,
        hash: i32,
        child: i32,
        iface: i32,
        offset: i32,
        length: i32,
    ) -> Result<()> {
        refuse("iface_settings")
    }
    fn interface_config(
        &mut self,
        player: &Self::Node,
        iface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()> {
        refuse("interface_config")
    }
    fn script_configs(
        &mut self,
        player: &Self::Node,
        id: i32,
        value: i32,
        kind: &str,
        params: Vec<ScriptArg>,
    ) -> Result<()> {
        refuse("script_configs")
    }
    fn generate_items(
        &mut self,
        player: &Self::Node,
        items: Vec<Option<Self::Node>>,
        options: &[&str],
        iface: i32,
        child: i32,
        x: i32,
        y: i32,
        key: Option<i32>,
    ) -> Result<()> {
        refuse("generate_items")
    }
    fn update_scene_graph(&mut self, player: &Self::Node, login: bool) -> Result<()> {
        refuse("update_scene_graph")
    }
    fn log(&mut self, level: LogLevel, text: &str) -> Result<()> {
        refuse("log")
    }
    fn debug(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        refuse("debug")
    }
    fn movement_pulse(
        &mut self,
        mover: &Self::Node,
        target: Option<&Self::Node>,
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<Self::Pulse> {
        refuse("movement_pulse")
    }
    fn new_pulse(&mut self, body: Task<Self::Node, Self::Clan>) -> Result<Self::Pulse> {
        refuse("new_pulse")
    }
    fn run_pulse(
        &mut self,
        player: &Self::Node,
        pulse: &Self::Pulse,
        kind: Option<PulseType>,
    ) -> Result<()> {
        refuse("run_pulse")
    }
    fn base_stop(&mut self, pulse: &Self::Pulse) -> Result<()> {
        refuse("base_stop")
    }
    fn movement_stop(&mut self, pulse: &Self::Pulse) -> Result<()> {
        refuse("movement_stop")
    }
    fn submit(
        &mut self,
        delay: i32,
        checks: &[&Self::Node],
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<()> {
        refuse("submit")
    }
    fn run_task(
        &mut self,
        entity: &Self::Node,
        delay: i32,
        repeat: i32,
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<()> {
        refuse("run_task")
    }
    fn run_world_task(&mut self, body: Task<Self::Node, Self::Clan>) -> Result<()> {
        refuse("run_world_task")
    }
    fn input_dialogue(
        &mut self,
        player: &Self::Node,
        numeric: bool,
        prompt: &str,
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<()> {
        refuse("input_dialogue")
    }
    fn attack(&mut self, player: &Self::Node, node: Option<&Self::Node>) -> Result<()> {
        refuse("attack")
    }
    fn face(&mut self, entity: &Self::Node, target: Option<&Self::Node>) -> Result<()> {
        refuse("face")
    }
    fn face_location(&mut self, entity: &Self::Node, location: Option<[i32; 3]>) -> Result<()> {
        refuse("face_location")
    }
    fn in_combat(&mut self, entity: &Self::Node) -> Result<bool> {
        refuse("in_combat")
    }
    fn lock_all(&mut self, entity: &Self::Node) -> Result<()> {
        refuse("lock_all")
    }
    fn lock_ticks(&mut self, entity: &Self::Node, ticks: i32) -> Result<()> {
        refuse("lock_ticks")
    }
    fn unlock(&mut self, entity: &Self::Node) -> Result<()> {
        refuse("unlock")
    }
    fn animate_entity(&mut self, entity: &Self::Node, animation: Option<i32>) -> Result<()> {
        refuse("animate_entity")
    }
    fn api_animate(&mut self, entity: &Self::Node, animation: i32, forced: bool) -> Result<()> {
        refuse("api_animate")
    }
    fn visualize(&mut self, entity: &Self::Node, animation: i32, graphics: i32) -> Result<()> {
        refuse("visualize")
    }
    fn lock_key(&mut self, player: &Self::Node, key: &str, ticks: i32) -> Result<()> {
        refuse("lock_key")
    }
    fn is_locked(&mut self, player: &Self::Node, key: &str) -> Result<bool> {
        refuse("is_locked")
    }
    fn interaction_locked(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("interaction_locked")
    }
    fn movement_locked(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("movement_locked")
    }
    fn teleport(
        &mut self,
        player: &Self::Node,
        location: Option<[i32; 3]>,
        kind: TeleportType,
    ) -> Result<bool> {
        refuse("teleport")
    }
    fn play_audio(&mut self, player: &Self::Node, id: i32, delay: i32, loops: i32) -> Result<()> {
        refuse("play_audio")
    }
    fn is_artificial(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("is_artificial")
    }
    fn valid_login(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("valid_login")
    }
    fn dev_mode(&mut self) -> Result<bool> {
        refuse("dev_mode")
    }
    fn toggle_debug(&mut self, player: &Self::Node) -> Result<()> {
        refuse("toggle_debug")
    }
    fn is_admin(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("is_admin")
    }
    fn username(&mut self, player: &Self::Node) -> Result<String> {
        refuse("username")
    }
    fn rights(&mut self, player: &Self::Node) -> Result<crate::player::details::Rights> {
        refuse("rights")
    }
    fn is_teleporting(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("is_teleporting")
    }
    fn can_logout(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("can_logout")
    }
    fn zone_type(&mut self, player: &Self::Node) -> Result<i32> {
        refuse("zone_type")
    }
    fn disconnect(&mut self, player: &Self::Node) -> Result<()> {
        refuse("disconnect")
    }
    fn toggle(&mut self, player: &Self::Node, toggle: Toggle) -> Result<()> {
        refuse("toggle")
    }
    fn special_toggled(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("special_toggled")
    }
    fn check_restriction(&mut self, player: &Self::Node, mode: IronmanMode) -> Result<bool> {
        refuse("check_restriction")
    }
    fn is_skulled(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("is_skulled")
    }
    fn prayer_active(&mut self, player: &Self::Node, prayer: PrayerType) -> Result<bool> {
        refuse("prayer_active")
    }
    fn prayer_type(&mut self, button: i32) -> Result<Option<PrayerType>> {
        refuse("prayer_type")
    }
    fn prayer_toggle(&mut self, player: &Self::Node, prayer: PrayerType) -> Result<bool> {
        refuse("prayer_toggle")
    }
    fn has_familiar(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("has_familiar")
    }
    fn is_burden_beast(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("is_burden_beast")
    }
    fn beast_container(&mut self, player: &Self::Node) -> Result<Self::Container> {
        refuse("beast_container")
    }
    fn has_timer(&mut self, player: &Self::Node, id: &str) -> Result<bool> {
        refuse("has_timer")
    }
    fn dragon_slayer_attribute(&mut self, player: &Self::Node, name: &str) -> Result<bool> {
        refuse("dragon_slayer_attribute")
    }
    fn loot_share_delay(&mut self, player: &Self::Node) -> Result<i64> {
        refuse("loot_share_delay")
    }
    fn set_loot_share_delay(&mut self, player: &Self::Node, value: i64) -> Result<()> {
        refuse("set_loot_share_delay")
    }
    fn loot_share_points(&mut self, player: &Self::Node) -> Result<i32> {
        refuse("loot_share_points")
    }
    fn set_loot_share_points(&mut self, player: &Self::Node, value: i32) -> Result<()> {
        refuse("set_loot_share_points")
    }
    fn set_disable_death_screen(&mut self, player: &Self::Node, value: bool) -> Result<()> {
        refuse("set_disable_death_screen")
    }
    fn activity_start(&mut self, player: &Self::Node, name: &str, login: bool) -> Result<bool> {
        refuse("activity_start")
    }
    fn set_varp(&mut self, player: &Self::Node, index: i32, value: i32, save: bool) -> Result<()> {
        refuse("set_varp")
    }
    fn set_varbit(&mut self, player: &Self::Node, id: i32, value: i32, save: bool) -> Result<()> {
        refuse("set_varbit")
    }
    fn flashing_icons(&mut self, player: &Self::Node, skill: i32) -> Result<()> {
        refuse("flashing_icons")
    }
    fn emote(&mut self, player: &Self::Node, button: i32) -> Result<()> {
        refuse("emote")
    }
    fn character_design(&mut self, player: &Self::Node, button: i32) -> Result<()> {
        refuse("character_design")
    }
    fn configure_game_world(&mut self, player: &Self::Node) -> Result<()> {
        refuse("configure_game_world")
    }
    fn random(&mut self, low: i32, high: i32) -> Result<i32> {
        refuse("random")
    }
    fn request(
        &mut self,
        player: &Self::Node,
        target: &Self::Node,
        kind: crate::world::trade::RequestType,
    ) -> Result<bool> {
        refuse("request")
    }
    fn bot_info(&mut self, player: &Self::Node, bot: &Self::Node) -> Result<()> {
        refuse("bot_info")
    }
    fn assist_session(&mut self, player: &Self::Node) -> Result<Option<Self::Session>> {
        refuse("assist_session")
    }
    fn assist_player(&mut self, session: &Self::Session) -> Result<Option<Self::Node>> {
        refuse("assist_player")
    }
    fn assist_toggle(&mut self, session: &Self::Session, index: i32) -> Result<()> {
        refuse("assist_toggle")
    }
    fn assist_refresh(&mut self, session: &Self::Session) -> Result<()> {
        refuse("assist_refresh")
    }
    fn inventory(&mut self, player: &Self::Node) -> Result<Self::Container> {
        refuse("inventory")
    }
    fn equipment(&mut self, player: &Self::Node) -> Result<Self::Container> {
        refuse("equipment")
    }
    fn death_containers(&mut self, player: &Self::Node) -> Result<[Self::Container; 2]> {
        refuse("death_containers")
    }
    fn get(&mut self, container: &Self::Container, slot: i32) -> Result<Option<Self::Node>> {
        refuse("get")
    }
    fn get_new_id(&mut self, container: &Self::Container, slot: i32) -> Result<i32> {
        refuse("get_new_id")
    }
    fn item_count(&mut self, container: &Self::Container) -> Result<i32> {
        refuse("item_count")
    }
    fn container_id(&mut self, container: &Self::Container, slot: i32) -> Result<i32> {
        refuse("container_id")
    }
    fn is_empty(&mut self, container: &Self::Container) -> Result<bool> {
        refuse("is_empty")
    }
    fn to_array(&mut self, container: &Self::Container) -> Result<Vec<Option<Self::Node>>> {
        refuse("to_array")
    }
    fn get_amount(&mut self, container: &Self::Container, item: &Self::Node) -> Result<i32> {
        refuse("get_amount")
    }
    fn refresh(&mut self, container: &Self::Container) -> Result<()> {
        refuse("refresh")
    }
    fn contain_items(&mut self, container: &Self::Container, ids: &[i32]) -> Result<bool> {
        refuse("contain_items")
    }
    fn add(&mut self, container: &Self::Container, id: i32, amount: i32) -> Result<bool> {
        refuse("add")
    }
    fn equipment_update(&mut self, player: &Self::Node) -> Result<()> {
        refuse("equipment_update")
    }
    fn unequip(&mut self, player: &Self::Node, slot: i32, item: i32) -> Result<()> {
        refuse("unequip")
    }
    fn listeners(
        &mut self,
        id: i32,
        kind: crate::world::interaction_listeners::Kind,
        option: &str,
        player: &Self::Node,
        node: &Self::Node,
    ) -> Result<bool> {
        refuse("listeners")
    }
    fn operate_handler(&mut self, item: &Self::Node) -> Result<Option<Self::Operate>> {
        refuse("operate_handler")
    }
    fn operate_handle(
        &mut self,
        handler: &Self::Operate,
        player: &Self::Node,
        item: &Self::Node,
        option: &str,
    ) -> Result<bool> {
        refuse("operate_handle")
    }
    fn examine(&mut self, item: &Self::Node) -> Result<Option<String>> {
        refuse("examine")
    }
    fn inventory_options(&mut self, item: &Self::Node) -> Result<Option<Vec<Option<String>>>> {
        refuse("inventory_options")
    }
    fn note_change(&mut self, item: &Self::Node) -> Result<i32> {
        refuse("note_change")
    }
    fn is_unnoted(&mut self, item: &Self::Node) -> Result<bool> {
        refuse("is_unnoted")
    }
    fn any_in_equipment(&mut self, player: &Self::Node, ids: &[i32]) -> Result<bool> {
        refuse("any_in_equipment")
    }
    fn remove_item(
        &mut self,
        player: &Self::Node,
        id: i32,
        amount: i32,
        store: Store,
    ) -> Result<bool> {
        refuse("remove_item")
    }
    fn add_item(
        &mut self,
        player: &Self::Node,
        id: i32,
        amount: i32,
        store: Store,
    ) -> Result<bool> {
        refuse("add_item")
    }
    fn amount_in_inventory(&mut self, player: &Self::Node, id: i32) -> Result<i32> {
        refuse("amount_in_inventory")
    }
    fn free_slots(&mut self, player: &Self::Node) -> Result<i32> {
        refuse("free_slots")
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        refuse("item_name")
    }
    fn has_option(&mut self, node: &Self::Node, option: &str) -> Result<bool> {
        refuse("has_option")
    }
    fn open_bank(&mut self, player: &Self::Node) -> Result<()> {
        refuse("open_bank")
    }
    fn open_collection_box(&mut self, player: &Self::Node) -> Result<()> {
        refuse("open_collection_box")
    }
    fn open_deposit_box(&mut self, player: &Self::Node) -> Result<()> {
        refuse("open_deposit_box")
    }
    fn bank_add_item(&mut self, player: &Self::Node, slot: i32, amount: i32) -> Result<()> {
        refuse("bank_add_item")
    }
    fn refresh_deposit_box(&mut self, player: &Self::Node) -> Result<()> {
        refuse("refresh_deposit_box")
    }
    fn transfer_x(
        &mut self,
        player: &Self::Node,
        slot: i32,
        withdraw: bool,
        after: Option<Task<Self::Node, Self::Clan>>,
    ) -> Result<()> {
        refuse("transfer_x")
    }
    fn dump_beast_of_burden(&mut self, player: &Self::Node) -> Result<()> {
        refuse("dump_beast_of_burden")
    }
    fn lunar_restriction(&mut self, player: &Self::Node, node: &Self::Node) -> Result<bool> {
        refuse("lunar_restriction")
    }
    fn find_npc(&mut self, location: [i32; 3]) -> Result<Option<Self::Node>> {
        refuse("find_npc")
    }
    fn weapon_interface(&mut self, player: &Self::Node) -> Result<Option<Extension<Self::Weapon>>> {
        refuse("weapon_interface")
    }
    fn is_special_bar(&mut self, weapon: &Self::Weapon) -> Result<bool> {
        refuse("is_special_bar")
    }
    fn set_attack_style(&mut self, weapon: &Self::Weapon, button: i32) -> Result<bool> {
        refuse("set_attack_style")
    }
    fn open_autocast_select(&mut self, weapon: &Self::Weapon) -> Result<()> {
        refuse("open_autocast_select")
    }
    fn select_auto_spell(
        &mut self,
        weapon: &Self::Weapon,
        button: i32,
        adjust: bool,
        forget: bool,
    ) -> Result<()> {
        refuse("select_auto_spell")
    }
    fn update_weapon_interface(&mut self, weapon: &Self::Weapon) -> Result<()> {
        refuse("update_weapon_interface")
    }
    fn has_autocast_spell(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("has_autocast_spell")
    }
    fn special_handler(&mut self, item: i32) -> Result<Option<Self::Special>> {
        refuse("special_handler")
    }
    fn special_instant(&mut self, handler: &Self::Special, player: &Self::Node) -> Result<bool> {
        refuse("special_instant")
    }
    fn combat_victim(&mut self, player: &Self::Node) -> Result<Option<Self::Node>> {
        refuse("combat_victim")
    }
    fn special_swing(
        &mut self,
        handler: &Self::Special,
        player: &Self::Node,
        victim: Option<&Self::Node>,
    ) -> Result<()> {
        refuse("special_swing")
    }
    fn spell_listeners(
        &mut self,
        button: i32,
        kind: i32,
        book: &str,
        player: &Self::Node,
        node: Option<&Self::Node>,
    ) -> Result<()> {
        refuse("spell_listeners")
    }
    fn cast_spell(
        &mut self,
        player: &Self::Node,
        book: SpellBook,
        spell: i32,
        target: &Self::Node,
    ) -> Result<bool> {
        refuse("cast_spell")
    }
    fn attack_animation(&mut self, player: &Self::Node) -> Result<i32> {
        refuse("attack_animation")
    }
    fn combat_level(&mut self, player: &Self::Node) -> Result<i32> {
        refuse("combat_level")
    }
    fn attack_style(&mut self, player: &Self::Node) -> Result<i32> {
        refuse("attack_style")
    }
    fn add_experience(&mut self, player: &Self::Node, skill: i32, experience: f64) -> Result<()> {
        refuse("add_experience")
    }
    fn diary_open_tab(&mut self, player: &Self::Node) -> Result<()> {
        refuse("diary_open_tab")
    }
    fn quest_for_button(
        &mut self,
        player: &Self::Node,
        button: i32,
    ) -> Result<Option<Self::Quest>> {
        refuse("quest_for_button")
    }
    fn quest_stage(&mut self, quest: &Self::Quest, player: &Self::Node) -> Result<i32> {
        refuse("quest_stage")
    }
    fn draw_journal(&mut self, quest: &Self::Quest, player: &Self::Node, stage: i32) -> Result<()> {
        refuse("draw_journal")
    }
    fn quest_requirements(&mut self, player: &Self::Node, button: i32) -> Result<()> {
        refuse("quest_requirements")
    }
    fn quest_sync_tab(&mut self, player: &Self::Node) -> Result<()> {
        refuse("quest_sync_tab")
    }
    fn quest_complete(&mut self, player: &Self::Node, quest: Quest) -> Result<bool> {
        refuse("quest_complete")
    }
    fn has_completed_all(&mut self, player: &Self::Node) -> Result<bool> {
        refuse("has_completed_all")
    }
    fn diary_for_child(&mut self, child: i32) -> Result<Option<usize>> {
        refuse("diary_for_child")
    }
    fn get_diary(
        &mut self,
        player: &Self::Node,
        kind: Option<usize>,
    ) -> Result<Option<Self::Diary>> {
        refuse("get_diary")
    }
    fn diary_open(&mut self, diary: &Self::Diary, player: &Self::Node) -> Result<()> {
        refuse("diary_open")
    }
    fn toggle_looping(&mut self, player: &Self::Node) -> Result<()> {
        refuse("toggle_looping")
    }
    fn unlocked(&mut self, player: &Self::Node, slot: i32) -> Result<Option<Song>> {
        refuse("unlocked")
    }
    fn play(&mut self, player: &Self::Node, song: Option<Song>) -> Result<()> {
        refuse("play")
    }
    fn songs(&mut self) -> Result<Vec<Song>> {
        refuse("songs")
    }
    fn unlock_song(&mut self, player: &Self::Node, id: i32) -> Result<()> {
        refuse("unlock_song")
    }
    fn clan_open_settings(&mut self, player: &Self::Node) -> Result<()> {
        refuse("clan_open_settings")
    }
    fn toggle_lootshare(&mut self, player: &Self::Node) -> Result<()> {
        refuse("toggle_lootshare")
    }
    fn clan_get(&mut self, owner: Option<&str>, create: bool) -> Result<Option<Self::Clan>> {
        refuse("clan_get")
    }
    fn clan_name(&mut self, clan: &Self::Clan) -> Result<Option<String>> {
        refuse("clan_name")
    }
    fn clan_set_name(&mut self, clan: &Self::Clan, name: &str) -> Result<()> {
        refuse("clan_set_name")
    }
    fn clan_rank(&mut self, clan: &Self::Clan, field: ClanField) -> Result<Option<ClanRank>> {
        refuse("clan_rank")
    }
    fn clan_set_rank(
        &mut self,
        clan: &Self::Clan,
        field: ClanField,
        rank: Option<ClanRank>,
    ) -> Result<()> {
        refuse("clan_set_rank")
    }
    fn clan_update_settings(&mut self, clan: &Self::Clan, player: &Self::Node) -> Result<()> {
        refuse("clan_update_settings")
    }
    fn clan_update(&mut self, clan: &Self::Clan) -> Result<()> {
        refuse("clan_update")
    }
    fn clan_delete(&mut self, clan: &Self::Clan) -> Result<()> {
        refuse("clan_delete")
    }
    fn comm_clan_name(&mut self, player: &Self::Node) -> Result<Option<String>> {
        refuse("comm_clan_name")
    }
    fn comm_set_clan_name(&mut self, player: &Self::Node, name: &str) -> Result<()> {
        refuse("comm_set_clan_name")
    }
    fn comm_set_rank(
        &mut self,
        player: &Self::Node,
        field: ClanField,
        rank: Option<ClanRank>,
    ) -> Result<()> {
        refuse("comm_set_rank")
    }
    fn world_communicator_enabled(&mut self) -> Result<bool> {
        refuse("world_communicator_enabled")
    }
    fn ms_clan_rename(&mut self, player: &Self::Node, name: Option<&str>) -> Result<()> {
        refuse("ms_clan_rename")
    }
    fn ms_clan_setting(
        &mut self,
        player: &Self::Node,
        kind: i32,
        rank: Option<ClanRank>,
    ) -> Result<()> {
        refuse("ms_clan_setting")
    }
    fn special_destination(&mut self, at: [i32; 3]) -> Result<Option<[i32; 3]>> {
        refuse("special_destination")
    }
    fn special_ladder(&mut self, at: [i32; 3]) -> Result<Option<&'static climb::SpecialLadder>> {
        refuse("special_ladder")
    }
    fn scenery_size(&mut self, scenery: &Self::Node) -> Result<[i32; 2]> {
        let definition = scenery.0.read().definition.clone().ok_or(Error::Null)?;
        let d = definition.read();
        Ok([d.size_x, d.size_y])
    }
    fn scenery_rotation(&mut self, scenery: &Self::Node) -> Result<i32> {
        Ok(scenery.0.read().rotation)
    }
    fn scenery_walking_flag(&mut self, scenery: &Self::Node) -> Result<i32> {
        let definition = scenery.0.read().definition.clone().ok_or(Error::Null)?;
        let flag = definition.read().walking_flag;
        Ok(flag)
    }
    fn scenery_options(&mut self, scenery: &Self::Node) -> Result<Option<Vec<Option<String>>>> {
        refuse("scenery_options")
    }
    fn get_object(&mut self, at: [i32; 3]) -> Result<Option<Self::Node>> {
        refuse("get_object")
    }
    fn teleport_permitted(&mut self, at: [i32; 3]) -> Result<bool> {
        let [x, y, z] = at;
        self.walk
            .scene
            .context(&mut self.walk.maps)
            .teleport_permitted(Some(&Location::new(x, y, z)))
            .map_err(|e| Error::Binding(Some(e.0)))
    }
    fn clipping_flag(&mut self, at: [i32; 3]) -> Result<i32> {
        let [x, y, z] = at;
        self.walk
            .scene
            .context(&mut self.walk.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| Error::Binding(Some(e.0)))
    }
    fn set_teleport_location(&mut self, player: &Self::Node, at: [i32; 3]) -> Result<()> {
        refuse("set_teleport_location")
    }
    fn finish_task(
        &mut self,
        player: &Self::Node,
        kind: usize,
        level: i32,
        task: i32,
    ) -> Result<()> {
        refuse("finish_task")
    }
    fn zogre_ladder(&mut self, player: &Self::Node) -> Result<()> {
        refuse("zogre_ladder")
    }
    fn send_options(&mut self, player: &Self::Node, title: &str, options: &[&str]) -> Result<()> {
        refuse("send_options")
    }
    fn set_dialogue(
        &mut self,
        player: &Self::Node,
        dialogue: climb::ClimbDialogue<Self::Node>,
    ) -> Result<()> {
        refuse("set_dialogue")
    }
    fn dialogue_end(&mut self, player: &Self::Node) -> Result<()> {
        refuse("dialogue_end")
    }
}
