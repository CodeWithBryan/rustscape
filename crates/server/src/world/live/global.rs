//! The global handlers (`world::global_handlers`, docs/global-handlers.md)
//! installed in the live world: `registry::register_scanned` over the live
//! InterfaceListeners, the interface definitions' component plugins, the
//! native InteractionListeners registry and the scenery option defaults at
//! boot; the player option handlers (Attack, Follow) and the per-definition
//! scenery handlers bound; the interface buttons, open and close listeners
//! and component plugins dispatched through `registry::run_button`,
//! `run_open` and `run_plugin`; option handlers through `run_handler`; the
//! bodies' deferred tasks (`Task`) kept with their world pulses and run with
//! `run_task`.
//!
//! [`GHost`] is the library's `Host` over the live world. The services with a
//! native system here answer through it (attributes, messages, the
//! interface manager, the weapon, emote and music tabs, settings toggles,
//! locks, animations, the teleport location, scenery geometry, the climbing
//! tables); every other service is a refusal by name ("not ported in the
//! live world: global handler service NAME"), listed in docs/live-world.md.
use super::{P, host::Live, inner::SettingsContext, tabs::Tabs};
use crate::world::{
    dialogue::{FacialExpression, Lines},
    interfaces,
};
use crate::{
    definitions::item::Shared,
    inventory::{container::Container, item::Handle as ItemHandle},
    player::attributes::AttributeValue,
    pulses,
    world::{
        action_locks,
        doors::{Handle as Scenery, Location},
        global_handlers::{
            self as gh, ClanField, ClanRank, DialogueArg, Error, Expression, Extension,
            IronmanMode, Kind, LogLevel, ObjectKind, Outcome, PulseType, Quest, Result, ScriptArg,
            Song, SpellBook, Store, Task, TeleportType, Toggle, Value, climb, registry,
        },
        interface_listeners::Button,
        player_tabs,
        player_walk::NodeHandle,
        prayer::PrayerType,
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates;

/// A node as the handlers see it.
#[derive(Clone)]
pub enum GNode {
    /// A player or an NPC (the canonical actor's node handle).
    Entity(P),
    Scenery(Scenery),
    Item(ItemHandle),
}
impl PartialEq for GNode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Entity(a), Self::Entity(b)) => a == b,
            (Self::Scenery(a), Self::Scenery(b)) => a.same(b),
            (Self::Item(a), Self::Item(b)) => a.same(b),
            _ => false,
        }
    }
}
impl std::fmt::Debug for GNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Entity(p) => write!(f, "{p:?}"),
            Self::Scenery(s) => write!(f, "Scenery({})", s.read().id),
            Self::Item(i) => write!(f, "Item({})", i.read().id()),
        }
    }
}

/// Handler types the live world does not have (special attacks, operate
/// handlers, quests, diaries, clans, assist sessions, object attributes).
#[derive(Clone, Debug, PartialEq)]
pub enum Absent {}

/// The global handlers' world state.
#[derive(Default)]
pub struct GlobalState {
    /// World pulses the bodies submitted, with their task and player.
    pub pulses: Vec<(pulses::Handle, Task<GNode, Absent>)>,
    /// `LoginConfiguration.getLoginPlugins()` entries the scan added.
    pub login_plugins: Vec<registry::LoginPlugin>,
    /// `ServerConstants.BANK_BOOTH_QUICK_OPEN`, `BANK_BOOTH_NOTE_ENABLED`,
    /// `BANK_BOOTH_NOTE_UIM`.
    pub booth: [bool; 3],
    /// `SpecialLadders` (one table for the world).
    pub ladders: Option<climb::Ladders>,
    /// The classes `register_scanned` registered, in order.
    pub registered: Vec<registry::Class>,
    /// `InterfaceContainer.index` (super::global_ui), set at install.
    pub container_index: i32,
}

fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(Some(format!(
        "not ported in the live world: global handler service {what}"
    ))))
}
fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}
/// `DialUtils.splitLines(message, 54)` as the dialogue's lines.
fn split_lines(message: &str) -> Result<Lines> {
    let utf16: Vec<u16> = message.encode_utf16().collect();
    let lines = crate::world::plain_dialogue::split_lines(&utf16, 54).map_err(bind)?;
    Ok(Lines::new(lines.into_iter().map(Some).collect()))
}

/// `Host` over the live world.
pub struct GHost<'a> {
    pub live: &'a mut Live,
}

impl GHost<'_> {
    fn player(&self, node: &GNode) -> Result<P> {
        match node {
            GNode::Entity(p) if self.live.inner.players.contains_key(&p.node()) => Ok(p.clone()),
            _ => Err(Error::Cast),
        }
    }
    fn tabs(&mut self, p: &P) -> Tabs<'_> {
        let live = &mut *self.live;
        Tabs {
            inner: &mut live.inner,
            p: p.clone(),
            walk: Some((&mut live.walk, &mut live.deferred)),
        }
    }
    fn scenery(node: &GNode) -> Result<&Scenery> {
        match node {
            GNode::Scenery(s) => Ok(s),
            _ => Err(Error::Cast),
        }
    }
}

fn value_of(v: AttributeValue) -> Result<Value<Absent>> {
    Ok(match v {
        AttributeValue::Int(i) => Value::Int(i),
        AttributeValue::Long(l) => Value::Long(l),
        AttributeValue::Short(s) => Value::Int(i32::from(s)),
        AttributeValue::Byte(b) => Value::Int(i32::from(b)),
        AttributeValue::Bool(b) => Value::Bool(b),
        AttributeValue::Text(t) => Value::Text(t),
        AttributeValue::Location(l) => Value::Location(l.coordinates()),
        // An object attribute of another system (none of these handlers
        // stores one here).
        AttributeValue::Object(_) => return refuse("object attributes"),
    })
}
fn attribute_of(v: Value<Absent>) -> Result<AttributeValue> {
    Ok(match v {
        Value::Int(i) => AttributeValue::Int(i),
        Value::Long(l) => AttributeValue::Long(l),
        Value::Bool(b) => AttributeValue::Bool(b),
        Value::Text(t) => AttributeValue::Text(t),
        Value::Location([x, y, z]) => {
            AttributeValue::Location(crate::player::attributes::AttributeLocation::new(x, y, z))
        }
        Value::Object(o) => match o {},
    })
}

#[allow(unused_variables)]
impl gh::Host for GHost<'_> {
    type Node = GNode;
    type Container = Container;
    type Pulse = pulses::Handle;
    type Weapon = P;
    type Special = usize;
    type Operate = Absent;
    type Quest = Absent;
    type Diary = Absent;
    type Clan = Absent;
    type Session = Absent;
    type Object = Absent;

    fn kind(&self, node: &Self::Node) -> Kind {
        match node {
            GNode::Entity(p) if self.live.inner.players.contains_key(&p.node()) => Kind::Player,
            GNode::Entity(_) => Kind::Npc,
            GNode::Scenery(_) => Kind::Scenery,
            GNode::Item(_) => Kind::Item,
        }
    }
    fn id(&mut self, node: &Self::Node) -> Result<i32> {
        match node {
            GNode::Entity(p) => match self.live.inner.npcs.rows.get(&p.node()) {
                Some(row) => Ok(row.id),
                None => refuse("id of a player"),
            },
            GNode::Scenery(s) => Ok(s.read().id),
            GNode::Item(i) => Ok(i.read().id()),
        }
    }
    fn name(&mut self, node: &Self::Node) -> Result<Option<String>> {
        match node {
            GNode::Entity(p) => match self.live.inner.players.get(&p.node()) {
                Some(slot) => Ok(Some(slot.name.clone())),
                None => refuse("name of an NPC"),
            },
            GNode::Scenery(s) => {
                let definition = s.read().definition.clone().ok_or(Error::Null)?;
                let name = definition.read().name.clone();
                Ok(name)
            }
            GNode::Item(_) => refuse("name of an item"),
        }
    }
    fn location(&mut self, node: &Self::Node) -> Result<Option<[i32; 3]>> {
        match node {
            GNode::Entity(p) => Ok(self
                .live
                .inner
                .state
                .node_location(p.node())
                .map_err(bind)?
                .map(|l| l.coordinates())),
            GNode::Scenery(s) => Ok(Some(s.read().location.coordinates())),
            GNode::Item(_) => refuse("location of an item"),
        }
    }
    fn amount(&mut self, item: &Self::Node) -> Result<i32> {
        refuse("amount")
    }
    fn lowercase_default(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn server_name(&mut self) -> String {
        self.live.inner.constants.server_name.clone()
    }
    fn booth_quick_open(&mut self) -> bool {
        self.live.global.booth[0]
    }
    fn booth_note_enabled(&mut self) -> bool {
        self.live.global.booth[1]
    }
    fn booth_note_uim(&mut self) -> bool {
        self.live.global.booth[2]
    }
    fn ticks(&mut self) -> i32 {
        self.live.inner.ticks
    }
    fn current_time_millis(&mut self) -> i64 {
        super::now_millis()
    }
    fn world_id(&mut self) -> Result<i32> {
        Ok(self.live.inner.constants.world_id)
    }
    fn get_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>> {
        let p = self.player(entity)?;
        let value = self.live.inner.canonical(&p).player.attributes().get(key);
        value.map(value_of).transpose()
    }
    fn set_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
        value: Value<Self::Object>,
    ) -> Result<()> {
        let p = self.player(entity)?;
        let value = attribute_of(value)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .set_value(key, Some(value));
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<()> {
        let p = self.player(entity)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .remove(key);
        Ok(())
    }
    fn attributes_get(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>> {
        let p = self.player(entity)?;
        let value = self.live.inner.canonical(&p).player.attributes().get(key);
        value.map(value_of).transpose()
    }
    fn api_get_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
    ) -> Result<Option<Value<Self::Object>>> {
        let p = self.player(entity)?;
        let value = self.live.inner.canonical(&p).player.attributes().get(key);
        value.map(value_of).transpose()
    }
    fn api_set_attribute(
        &mut self,
        entity: &Self::Node,
        key: &str,
        value: Value<Self::Object>,
    ) -> Result<()> {
        let p = self.player(entity)?;
        let value = attribute_of(value)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .set_value(key, Some(value));
        Ok(())
    }
    fn api_remove_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<()> {
        let p = self.player(entity)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .remove(key);
        Ok(())
    }
    fn api_remove_attributes(&mut self, entity: &Self::Node, keys: &[&str]) -> Result<()> {
        let p = self.player(entity)?;
        for key in keys {
            self.live
                .inner
                .canonical(&p)
                .player
                .attributes()
                .remove(key);
        }
        Ok(())
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
        let p = self.player(player)?;
        self.live.inner.message(&p, text).map_err(bind)
    }
    fn message(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        let p = self.player(player)?;
        self.live.inner.message(&p, text).map_err(bind)
    }
    fn packet_message(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        let p = self.player(player)?;
        self.live.inner.message(&p, text).map_err(bind)
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
        let p = self.player(player)?;
        self.live.inner.text(&p, text, iface, child).map_err(bind)
    }
    fn set_interface_text(
        &mut self,
        player: &Self::Node,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        let p = self.player(player)?;
        self.live.inner.text(&p, text, iface, child).map_err(bind)
    }
    fn dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        // ContentAPI.sendDialogue: interpreter.sendDialogue(*splitLines(msg))
        // (super::global_ui).
        let p = self.player(player)?;
        let lines = split_lines(text)?;
        let interpreter = self.live.interpreter_of(&p);
        let result = interpreter.send_dialogue(self.live, Some(&lines));
        self.live.drain_all();
        result.map(|_| ()).map_err(bind)
    }
    fn npc_dialogue(
        &mut self,
        player: &Self::Node,
        npc: i32,
        text: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()> {
        // ContentAPI.sendNPCDialogue: interpreter.sendDialogues(npc, expr,
        // hideContinue, *splitLines(msg)).
        let p = self.player(player)?;
        let lines = split_lines(text)?;
        let expression = match expression {
            Expression::Neutral => FacialExpression::Neutral,
            Expression::Annoyed => FacialExpression::Annoyed,
        };
        let interpreter = self.live.interpreter_of(&p);
        let result = interpreter.send_dialogues(
            self.live,
            npc,
            FacialExpression::or_unset(Some(expression)),
            hide,
            &lines,
        );
        self.live.drain_all();
        result.map(|_| ()).map_err(bind)
    }
    fn item_dialogue(&mut self, player: &Self::Node, item: i32, text: &str) -> Result<()> {
        // ContentAPI.sendItemDialogue with an Int: Item(item), then
        // interpreter.sendItemMessage(item, *splitLines(message)).
        let p = self.player(player)?;
        let item = crate::world::dialogue::Host::new_item(self.live, item).map_err(bind)?;
        let lines = split_lines(text)?;
        let interpreter = self.live.interpreter_of(&p);
        let result = interpreter.send_item_message(self.live, &item, &lines);
        self.live.drain_all();
        result.map(|_| ()).map_err(bind)
    }
    fn double_item_dialogue(
        &mut self,
        player: &Self::Node,
        first: i32,
        second: i32,
        text: &str,
    ) -> Result<()> {
        // ContentAPI.sendDoubleItemDialogue with Ints: Item(first),
        // Item(second), then interpreter.sendDoubleItemMessage(first, second,
        // *splitLines(message)).
        let p = self.player(player)?;
        let first = crate::world::dialogue::Host::new_item(self.live, first).map_err(bind)?;
        let second = crate::world::dialogue::Host::new_item(self.live, second).map_err(bind)?;
        let lines = split_lines(text)?;
        let interpreter = self.live.interpreter_of(&p);
        let result = interpreter.send_double_item_message(self.live, &first, &second, &lines);
        self.live.drain_all();
        result.map(|_| ()).map_err(bind)
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
        // ContentAPI.openInterface: interfaceManager.open(Component(id))
        // with the world's interface context (super::global_ui).
        let p = self.player(player)?;
        self.live
            .with_world_ui(&p, |ui, m| {
                let c = interfaces::Context::component(ui, id)?;
                interfaces::open(ui, m, Some(c)).map(|_| ())
            })
            .map_err(bind)
    }
    fn close_interface(&mut self, player: &Self::Node) -> Result<()> {
        // ContentAPI.closeInterface: interfaceManager.close(), with the
        // world's interface context (close listeners and events run).
        let p = self.player(player)?;
        self.live
            .with_world_ui(&p, |ui, m| interfaces::close(ui, m).map(|_| ()))
            .map_err(bind)
    }
    fn open_single_tab(&mut self, player: &Self::Node, id: i32) -> Result<()> {
        // ContentAPI.openSingleTab: interfaceManager.openSingleTab(Component(id)).
        let p = self.player(player)?;
        self.live
            .with_world_ui(&p, |ui, m| {
                let c = interfaces::Context::component(ui, id)?;
                interfaces::open_single_tab(ui, m, &c).map(|_| ())
            })
            .map_err(bind)
    }
    fn close_tab_interface(&mut self, player: &Self::Node) -> Result<()> {
        // ContentAPI.closeTabInterface: interfaceManager.closeSingleTab().
        let p = self.player(player)?;
        self.live
            .with_world_ui(&p, |ui, m| interfaces::close_single_tab(ui, m).map(|_| ()))
            .map_err(bind)
    }
    fn open(&mut self, player: &Self::Node, id: i32) -> Result<bool> {
        // interfaceManager.open(Component(id)) != null.
        let p = self.player(player)?;
        self.live
            .with_world_ui(&p, |ui, m| {
                let c = interfaces::Context::component(ui, id)?;
                interfaces::open(ui, m, Some(c)).map(|o| o.is_some())
            })
            .map_err(bind)
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
        let p = self.player(player)?;
        let manager = self.live.inner.canonical(&p).player.manager();
        Ok(interfaces::is_opened(&manager))
    }
    fn close_manager(&mut self, player: &Self::Node) -> Result<bool> {
        let p = self.player(player)?;
        self.live
            .with_world_ui(&p, |ui, m| interfaces::close(ui, m))
            .map_err(bind)
    }
    fn close_single_tab(&mut self, player: &Self::Node) -> Result<bool> {
        let p = self.player(player)?;
        self.live
            .with_world_ui(&p, |ui, m| interfaces::close_single_tab(ui, m))
            .map_err(bind)
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
        // packetDispatch.sendRunScript(id, types, args...).
        let p = self.player(player)?;
        let objects: Vec<updates::ScriptArg> = args
            .into_iter()
            .map(|a| match a {
                ScriptArg::Int(i) => updates::ScriptArg::Int(i),
                ScriptArg::Text(t) => updates::ScriptArg::Text(t.into_bytes()),
            })
            .collect();
        let counter = self
            .live
            .inner
            .canonical(&p)
            .player
            .manager()
            .packet_count(1);
        let frame =
            updates::run_client_script(id, types.as_bytes(), &objects, counter).map_err(bind)?;
        self.live.inner.queue_native(&p, frame);
        Ok(())
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
        // packetDispatch.sendIfaceSettings(hash, child, iface, offset, length).
        let p = self.player(player)?;
        let counter = self
            .live
            .inner
            .canonical(&p)
            .player
            .manager()
            .packet_count(1);
        let frame = updates::interface_events(
            iface.wrapping_shl(16) | child,
            offset,
            length,
            hash,
            counter,
        );
        self.live.inner.queue_native(&p, frame);
        Ok(())
    }
    fn interface_config(
        &mut self,
        player: &Self::Node,
        iface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()> {
        // packetDispatch.sendInterfaceConfig(iface, child, hide).
        let p = self.player(player)?;
        let counter = self
            .live
            .inner
            .canonical(&p)
            .player
            .manager()
            .packet_count(1);
        let frame = updates::interface_hidden(hide, counter, iface.wrapping_shl(16) | child);
        self.live.inner.queue_native(&p, frame);
        Ok(())
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
        // InterfaceContainer.generateItems with a key (the equipment
        // screen's inventory side, key 93); the unkeyed overloads draw the
        // key from the index first and are not reached by these handlers.
        let Some(key) = key else {
            return refuse("generate_items without a key");
        };
        let p = self.player(player)?;
        super::global_ui::generate_items(self.live, &p, &items, options, iface, child, x, y, key)
            .map(|_| ())
    }
    fn update_scene_graph(&mut self, player: &Self::Node, login: bool) -> Result<()> {
        refuse("update_scene_graph")
    }
    fn log(&mut self, level: LogLevel, text: &str) -> Result<()> {
        self.live.inner.log_line(format!("{} {text}", level.name()));
        Ok(())
    }
    fn debug(&mut self, player: &Self::Node, text: &str) -> Result<()> {
        let p = self.player(player)?;
        let debug = self
            .live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .boolean("debug", false)
            .map_err(|_| Error::Cast)?;
        if debug {
            self.live.inner.message(&p, text).map_err(bind)?;
        }
        Ok(())
    }
    fn movement_pulse(
        &mut self,
        mover: &Self::Node,
        target: Option<&Self::Node>,
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<Self::Pulse> {
        // FollowOptionPlugin's MovementPulse(player, target, FOLLOW_ENTITY)
        // (super::follow).
        let mover = self.player(mover)?;
        let target = self.player(target.ok_or(Error::Null)?)?;
        self.live.follow_movement(&mover, &target, &body)
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
        if kind != Some(PulseType::Standard) {
            return refuse("run_pulse of a type other than STANDARD");
        }
        self.live.run_follow(pulse)
    }
    fn base_stop(&mut self, pulse: &Self::Pulse) -> Result<()> {
        pulses::stop_base(pulse);
        Ok(())
    }
    fn movement_stop(&mut self, pulse: &Self::Pulse) -> Result<()> {
        pulses::stop_base(pulse);
        Ok(())
    }
    fn submit(
        &mut self,
        delay: i32,
        checks: &[&Self::Node],
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<()> {
        let nodes: Vec<Option<usize>> = checks
            .iter()
            .map(|n| match n {
                GNode::Entity(p) => Some(p.node()),
                _ => None,
            })
            .collect();
        let class = match &body {
            Task::Retaliate { .. } => {
                "content.global.handlers.iface.tabs.CombatTabInterface$handle$1"
            }
            Task::SpecialBar { .. } => {
                "content.global.handlers.iface.tabs.CombatTabInterface$handle$2"
            }
            Task::Climb { .. } => "core.game.global.action.ClimbActionHandler$climb$1",
            // ClimbDialogue.handle's two anonymous pulses: case 1 (climb-up)
            // is $1, case 2 (climb-down) is $2.
            Task::ClimbChoice { option, .. } if *option == "climb-up" => {
                "core.game.global.action.ClimbActionHandler$ClimbDialogue$1"
            }
            Task::ClimbChoice { .. } => {
                "core.game.global.action.ClimbActionHandler$ClimbDialogue$2"
            }
            _ => "content.global.handlers (a deferred body)",
        };
        let handle = Shared::new(pulses::Pulse::new(
            delay,
            Some(Shared::new(nodes)),
            pulses::Class::Other,
            class.into(),
        ));
        self.live
            .driver
            .pulser()
            .submit(Some(handle.clone()))
            .map_err(bind)?;
        self.live.global.pulses.push((handle, body));
        Ok(())
    }
    fn run_task(
        &mut self,
        entity: &Self::Node,
        delay: i32,
        repeat: i32,
        body: Task<Self::Node, Self::Clan>,
    ) -> Result<()> {
        // ContentAPI.runTask: `pulseManager.run(Pulse(delay))` whose pulse
        // runs the body and ends when `--cycles <= 0`. A body run more than
        // once is not bound; the pulse goes to GameWorld.Pulser directly
        // (not through the entity's pulse manager, so a walk does not stop
        // it: a named difference in docs/live-world.md).
        if repeat > 1 {
            return refuse("runTask with more than one repetition");
        }
        self.submit(delay, &[entity], body)
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
        // player.attack(node): the combat pulse (super::combat).
        let p = self.player(player)?;
        let GNode::Entity(target) = node.ok_or(Error::Null)? else {
            return Err(Error::Cast);
        };
        self.live.combat_attack(&p, target).map_err(bind)
    }
    fn face(&mut self, entity: &Self::Node, target: Option<&Self::Node>) -> Result<()> {
        let p = self.player(entity)?;
        let target = match target {
            None => None,
            Some(GNode::Entity(t)) => Some(t.clone()),
            Some(_) => return Err(Error::Cast),
        };
        self.live.face_entity(&p, target.as_ref()).map_err(bind)
    }
    fn face_location(&mut self, entity: &Self::Node, location: Option<[i32; 3]>) -> Result<()> {
        refuse("face_location")
    }
    fn in_combat(&mut self, entity: &Self::Node) -> Result<bool> {
        // CombatPulse.isInCombat: the combat-attacker attribute's pulse is
        // attacking (super::combat).
        let p = self.player(entity)?;
        Ok(
            match self.live.inner.attribute(p.node(), "combat-attacker") {
                Some(super::combat::Attr::Entity(attacker)) => {
                    self.live.inner.combat_is_attacking(attacker.node())
                }
                _ => false,
            },
        )
    }
    fn lock_all(&mut self, entity: &Self::Node) -> Result<()> {
        refuse("lock_all")
    }
    fn lock_ticks(&mut self, entity: &Self::Node, ticks: i32) -> Result<()> {
        let p = match entity {
            GNode::Entity(p) => p.clone(),
            _ => return Err(Error::Cast),
        };
        let state = &mut self.live.inner.state;
        let locks = state.locks(&p.0).map_err(bind)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(bind)
    }
    fn unlock(&mut self, entity: &Self::Node) -> Result<()> {
        let p = self.player(entity)?;
        self.live.inner.state.locks(&p.0).map_err(bind)?.unlock();
        Ok(())
    }
    fn animate_entity(&mut self, entity: &Self::Node, animation: Option<i32>) -> Result<()> {
        let p = self.player(entity)?;
        let animator = self.live.inner.canonical(&p).animator.clone();
        animator
            .animate(
                &mut self.live.walk.animations,
                animation.map(crate::world::animation::Animation::new),
                None,
            )
            .map(|_| ())
            .map_err(bind)
    }
    fn api_animate(&mut self, entity: &Self::Node, animation: i32, forced: bool) -> Result<()> {
        let p = self.player(entity)?;
        let animator = self.live.inner.canonical(&p).animator.clone();
        animator
            .animate(
                &mut self.live.walk.animations,
                Some(crate::world::animation::Animation::new(animation)),
                None,
            )
            .map(|_| ())
            .map_err(bind)
    }
    fn visualize(&mut self, entity: &Self::Node, animation: i32, graphics: i32) -> Result<()> {
        refuse("visualize")
    }
    fn lock_key(&mut self, player: &Self::Node, key: &str, ticks: i32) -> Result<()> {
        // player.locks.lock(key, ticks): the canonical actor's keyed locks.
        let p = self.player(player)?;
        crate::world::social::Host::lock(&mut self.live.inner, &p, key, ticks).map_err(bind)
    }
    fn is_locked(&mut self, player: &Self::Node, key: &str) -> Result<bool> {
        let p = self.player(player)?;
        crate::world::social::Host::is_locked(&mut self.live.inner, &p, key).map_err(bind)
    }
    fn interaction_locked(&mut self, player: &Self::Node) -> Result<bool> {
        let p = self.player(player)?;
        let state = &mut self.live.inner.state;
        state
            .locks(&p.0)
            .map_err(bind)?
            .is_kind_locked(
                action_locks::Kind::Interaction,
                state,
                &mut action_locks::Clock,
            )
            .map_err(bind)
    }
    fn movement_locked(&mut self, player: &Self::Node) -> Result<bool> {
        let p = self.player(player)?;
        let state = &mut self.live.inner.state;
        state
            .locks(&p.0)
            .map_err(bind)?
            .is_kind_locked(
                action_locks::Kind::Movement,
                state,
                &mut action_locks::Clock,
            )
            .map_err(bind)
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
        self.player(player)?;
        // Every player of this world is a session's player.
        Ok(false)
    }
    fn valid_login(&mut self, player: &Self::Node) -> Result<bool> {
        // SystemManager.getSystemConfig().validLogin(player): true.
        self.player(player)?;
        Ok(true)
    }
    fn dev_mode(&mut self) -> Result<bool> {
        Ok(self.live.inner.settings.dev_mode)
    }
    fn toggle_debug(&mut self, player: &Self::Node) -> Result<()> {
        // Player.toggleDebug: the debug attribute flipped, then the message.
        let p = self.player(player)?;
        let attributes = self.live.inner.canonical(&p).player.attributes();
        let debug = attributes.boolean("debug", false).map_err(bind)?;
        attributes.set_value(
            "debug",
            Some(crate::player::attributes::AttributeValue::Bool(!debug)),
        );
        self.live
            .inner
            .message(&p, &format!("Your debug mode is toggled to {}.", !debug))
            .map_err(bind)
    }
    fn is_admin(&mut self, player: &Self::Node) -> Result<bool> {
        let p = self.player(player)?;
        let rights = self
            .live
            .inner
            .with_profile(&p, |profile| profile.details.rights())
            .map_err(bind)?
            .map_err(bind)?;
        Ok(rights == crate::player::details::Rights::Administrator)
    }
    fn username(&mut self, player: &Self::Node) -> Result<String> {
        let p = self.player(player)?;
        Ok(self.live.inner.slot(&p).name.clone())
    }
    fn rights(&mut self, player: &Self::Node) -> Result<crate::player::details::Rights> {
        let p = self.player(player)?;
        self.live
            .inner
            .with_profile(&p, |profile| profile.details.rights())
            .map_err(bind)?
            .map_err(bind)
    }
    fn is_teleporting(&mut self, player: &Self::Node) -> Result<bool> {
        let p = self.player(player)?;
        self.live
            .entity_is_teleporting(&p)
            .map_err(|e| Error::Binding(Some(e)))
    }
    fn can_logout(&mut self, player: &Self::Node) -> Result<bool> {
        let p = self.player(player)?;
        let monitor = self.live.monitor(&p.0);
        monitor.can_logout(self.live).map_err(bind)
    }
    fn zone_type(&mut self, player: &Self::Node) -> Result<i32> {
        // player.zoneMonitor.type.
        let p = self.player(player)?;
        let monitor = self.live.monitor(&p.0);
        monitor.get_type().map_err(bind)
    }
    fn disconnect(&mut self, player: &Self::Node) -> Result<()> {
        let p = self.player(player)?;
        let repository = self.live.repository.clone();
        crate::world::disconnection::send_logout(self.live, &repository, &p).map_err(bind)
    }
    fn toggle(&mut self, player: &Self::Node, toggle: Toggle) -> Result<()> {
        let p = self.player(player)?;
        let settings = self.live.inner.canonical(&p).player.settings();
        let mut ctx = SettingsContext {
            inner: &mut self.live.inner,
            p: p.clone(),
        };
        match toggle {
            Toggle::Retaliating => settings.toggle_retaliating(&mut ctx).map_err(bind)?,
            Toggle::SpecialBar => settings.toggle_special_bar(&mut ctx).map_err(bind)?,
            other => return refuse(&format!("the settings toggle {other:?}")),
        }
        self.live.inner.drain(&p);
        Ok(())
    }
    fn special_toggled(&mut self, player: &Self::Node) -> Result<bool> {
        let p = self.player(player)?;
        Ok(self
            .live
            .inner
            .canonical(&p)
            .player
            .settings()
            .fields
            .read()
            .special_toggled)
    }
    fn check_restriction(&mut self, player: &Self::Node, mode: IronmanMode) -> Result<bool> {
        refuse("check_restriction")
    }
    fn is_skulled(&mut self, player: &Self::Node) -> Result<bool> {
        // player.skullManager.isSkulled (world::player_managers::skull).
        let p = self.player(player)?;
        crate::world::player_managers::skull::answers::is_skulled(self.live, &p.0).map_err(bind)
    }
    fn prayer_active(&mut self, player: &Self::Node, prayer: PrayerType) -> Result<bool> {
        // player.prayer.get(type) (world::prayer, super::combat::prayer).
        let p = self.player(player)?;
        let state = self.live.prayer_state(&p).map_err(bind)?;
        Ok(state.get(prayer))
    }
    fn prayer_type(&mut self, button: i32) -> Result<Option<PrayerType>> {
        // PrayerType.get(buttonID).
        Ok(PrayerType::for_button(button))
    }
    fn prayer_toggle(&mut self, player: &Self::Node, prayer: PrayerType) -> Result<bool> {
        // player.prayer.toggle(type).
        let p = self.player(player)?;
        self.live.prayer_toggle(&p, prayer).map_err(bind)
    }
    fn has_familiar(&mut self, player: &Self::Node) -> Result<bool> {
        // FamiliarManager.hasFamiliar: summoning is not installed, so no
        // player of this world has a familiar (the same named answer as the
        // interface context's has_familiar).
        self.player(player)?;
        self.live
            .inner
            .not_ported
            .default_answer("FamiliarManager.hasFamiliar (not ported: false)");
        Ok(false)
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
        // getSavedData().getQuestData().getDragonSlayerAttribute(name).
        let p = self.player(player)?;
        self.live
            .inner
            .with_profile(&p, |profile| {
                profile
                    .saved
                    .quest
                    .dragon_slayer_attribute(crate::player::saved_data::QuestName::Literal(name))
            })
            .map_err(bind)
    }
    fn loot_share_delay(&mut self, player: &Self::Node) -> Result<i64> {
        let p = self.player(player)?;
        self.live
            .inner
            .with_profile(&p, |profile| profile.saved.global.loot_share_delay)
            .map_err(bind)
    }
    fn set_loot_share_delay(&mut self, player: &Self::Node, value: i64) -> Result<()> {
        let p = self.player(player)?;
        self.live
            .inner
            .with_profile(&p, |profile| profile.saved.global.loot_share_delay = value)
            .map_err(bind)
    }
    fn loot_share_points(&mut self, player: &Self::Node) -> Result<i32> {
        let p = self.player(player)?;
        self.live
            .inner
            .with_profile(&p, |profile| profile.saved.global.loot_share_points)
            .map_err(bind)
    }
    fn set_loot_share_points(&mut self, player: &Self::Node, value: i32) -> Result<()> {
        let p = self.player(player)?;
        self.live
            .inner
            .with_profile(&p, |profile| profile.saved.global.loot_share_points = value)
            .map_err(bind)
    }
    fn set_disable_death_screen(&mut self, player: &Self::Node, value: bool) -> Result<()> {
        let p = self.player(player)?;
        crate::world::player_death::Host::set_death_screen_disabled(self.live, &p.0, value)
            .map_err(bind)
    }
    fn activity_start(&mut self, player: &Self::Node, name: &str, login: bool) -> Result<bool> {
        refuse("activity_start")
    }
    fn set_varp(&mut self, player: &Self::Node, index: i32, value: i32, save: bool) -> Result<()> {
        // ContentAPI.setVarp(player, index, value, save).
        let p = self.player(player)?;
        self.live
            .inner
            .variables(&p, |ctx, owner| {
                crate::player::variables::set_varp(ctx, Some(owner), index, value, save)
            })
            .map_err(bind)
    }
    fn set_varbit(&mut self, player: &Self::Node, id: i32, value: i32, save: bool) -> Result<()> {
        // ContentAPI.setVarbit(player, id, value, save).
        let p = self.player(player)?;
        self.live
            .inner
            .variables(&p, |ctx, owner| {
                crate::player::variables::set_varbit(ctx, Some(owner), id, value, save)
            })
            .map_err(bind)
    }
    fn flashing_icons(&mut self, player: &Self::Node, skill: i32) -> Result<()> {
        // LevelUp.sendFlashingIcons(player, skill), native.
        let p = self.player(player)?;
        crate::world::player_tabs::level_up::send_flashing_icons(
            &mut super::tabs::Tabs::new(&mut self.live.inner, p),
            skill,
        )
        .map_err(bind)
    }
    fn emote(&mut self, player: &Self::Node, button: i32) -> Result<()> {
        let p = self.player(player)?;
        self.tabs(&p)
            .emotes(|tabs, manager| player_tabs::emotes::handle(tabs, manager, button))
            .map_err(bind)
    }
    fn character_design(&mut self, player: &Self::Node, button: i32) -> Result<()> {
        refuse("character_design")
    }
    fn configure_game_world(&mut self, player: &Self::Node) -> Result<()> {
        // LoginConfiguration.configureGameWorld after the lobby's play
        // button: the login library over the live world.
        let p = self.player(player)?;
        let repository = self.live.repository.clone();
        crate::world::login::configuration::configure_game_world(self.live, &repository, &p)
            .map_err(bind)
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
        // player.requestManager.request(target, type) (super::trading).
        let p = self.player(player)?;
        let target = self.player(target)?;
        self.live.trade_request(&p, &target, kind).map_err(bind)
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
        let p = self.player(player)?;
        Ok(self.live.inner.canonical(&p).player.inventory())
    }
    fn equipment(&mut self, player: &Self::Node) -> Result<Self::Container> {
        let p = self.player(player)?;
        Ok(self.live.inner.canonical(&p).player.equipment())
    }
    fn death_containers(&mut self, player: &Self::Node) -> Result<[Self::Container; 2]> {
        // DeathTask.getContainers(player) (world::player_death), with the
        // player as the container operations' subject.
        let p = self.player(player)?;
        let factory = crate::inventory::container::Factory {
            null_item: self.live.inner.factory.null_item.clone(),
        };
        let definitions = self.live.inner.items.clone();
        let previous = self.live.inner.combat.death_subject.replace(p.clone());
        let result =
            crate::world::player_death::get_containers(self.live, &p.0, &factory, &definitions);
        self.live.inner.combat.death_subject = previous;
        result.map_err(bind)
    }
    fn get(&mut self, container: &Self::Container, slot: i32) -> Result<Option<Self::Node>> {
        refuse("get")
    }
    fn get_new_id(&mut self, container: &Self::Container, slot: i32) -> Result<i32> {
        // container.getNew(slot).getId().
        let item = container
            .get_new(slot, &self.live.inner.items)
            .map_err(bind)?;
        let id = item.read().id();
        Ok(id)
    }
    fn item_count(&mut self, container: &Self::Container) -> Result<i32> {
        Ok(container.count())
    }
    fn container_id(&mut self, container: &Self::Container, slot: i32) -> Result<i32> {
        container.get_id(slot).map_err(bind)
    }
    fn is_empty(&mut self, container: &Self::Container) -> Result<bool> {
        Ok(container.empty())
    }
    fn to_array(&mut self, container: &Self::Container) -> Result<Vec<Option<Self::Node>>> {
        // container.toArray(): the live item array.
        Ok(container
            .items()
            .read()
            .iter()
            .map(|i| i.clone().map(GNode::Item))
            .collect())
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
        // EquipmentContainer.update(player) (inventory::equipment) with the
        // live equipment context.
        let p = self.player(player)?;
        let listeners = Some(self.live.item_actions.listeners.clone());
        let mut ctx = super::inner::EquipmentContext {
            inner: &mut self.live.inner,
            p: p.clone(),
            listeners,
        };
        let result = crate::inventory::equipment::update(&p.0, &mut ctx);
        self.live.inner.drain(&p);
        result.map_err(bind)
    }
    fn unequip(&mut self, player: &Self::Node, slot: i32, item: i32) -> Result<()> {
        let p = self.player(player)?;
        self.live.unequip(&p, slot, item).map_err(bind)
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
        // ContentAPI openBankAccount (super::bank).
        let p = self.player(player)?;
        self.live.open_bank_account(&p).map_err(bind)
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
        let p = self.player(player)?;
        Ok(self
            .live
            .inner
            .canonical(&p)
            .weapon_tab
            .is_some()
            .then(|| Extension::Weapon(p.clone())))
    }
    fn is_special_bar(&mut self, weapon: &Self::Weapon) -> Result<bool> {
        Ok(self
            .live
            .inner
            .canonical(weapon)
            .weapon_tab
            .as_ref()
            .is_some_and(|w| w.is_special_bar()))
    }
    fn set_attack_style(&mut self, weapon: &Self::Weapon, button: i32) -> Result<bool> {
        let p = weapon.clone();
        self.tabs(&p)
            .weapon(|tabs, w| w.set_attack_style(tabs, button))
            .map_err(bind)
    }
    fn open_autocast_select(&mut self, weapon: &Self::Weapon) -> Result<()> {
        let p = weapon.clone();
        self.tabs(&p)
            .weapon(|tabs, w| w.open_autocast_select(tabs))
            .map_err(bind)
    }
    fn select_auto_spell(
        &mut self,
        weapon: &Self::Weapon,
        button: i32,
        adjust: bool,
        forget: bool,
    ) -> Result<()> {
        let p = weapon.clone();
        self.tabs(&p)
            .weapon(|tabs, w| w.select_auto_spell(tabs, button, adjust, forget))
            .map_err(bind)
    }
    fn update_weapon_interface(&mut self, weapon: &Self::Weapon) -> Result<()> {
        refuse("update_weapon_interface")
    }
    fn has_autocast_spell(&mut self, player: &Self::Node) -> Result<bool> {
        use crate::world::player_tabs::weapon::Host as _;
        let p = self.player(player)?;
        Ok(self.tabs(&p).autocast().map_err(bind)?.is_some())
    }
    fn special_handler(&mut self, item: i32) -> Result<Option<Self::Special>> {
        // CombatStyle.MELEE.swingHandler.getSpecial(itemId).
        Ok(self.live.melee_special(item))
    }
    fn special_instant(&mut self, handler: &Self::Special, _: &Self::Node) -> Result<bool> {
        // fireEvent("instant_spec", player) == Boolean.TRUE.
        let special = self
            .live
            .inner
            .combat
            .handlers
            .get(*handler)
            .and_then(|s| s.special)
            .ok_or(Error::Cast)?;
        let osrs = self.live.inner.combat.osrs_dragon_axe;
        Ok(special.fire_event("instant_spec", osrs) == crate::world::special_attacks::Fired::True)
    }
    fn combat_victim(&mut self, player: &Self::Node) -> Result<Option<Self::Node>> {
        let p = self.player(player)?;
        let victim = self.live.inner.combat_entry(&p).state.victim.clone();
        Ok(victim.map(GNode::Entity))
    }
    fn special_swing(
        &mut self,
        handler: &Self::Special,
        player: &Self::Node,
        victim: Option<&Self::Node>,
    ) -> Result<()> {
        // handleInstantSpec: handler.swing(p, victim, null).
        let p = self.player(player)?;
        let victim = match victim {
            None => None,
            Some(GNode::Entity(v)) => Some(v.clone()),
            Some(_) => return Err(Error::Cast),
        };
        let result = crate::world::special_attacks::swing(
            self.live,
            handler,
            Some(&p),
            victim.as_ref(),
            None,
        );
        self.live.sync_combat();
        result.map(|_| ()).map_err(bind)
    }
    fn spell_listeners(
        &mut self,
        button: i32,
        kind: i32,
        book: &str,
        player: &Self::Node,
        node: Option<&Self::Node>,
    ) -> Result<()> {
        let p = self.player(player)?;
        // The magic tab runs the NONE listeners (no node); a node here
        // would be a listener with a target, which these handlers never
        // pass.
        if node.is_some() {
            return refuse("SpellListeners.run with a node from the global handlers");
        }
        super::magic::spell_listeners(self.live, button, kind, book, &p, -1)
            .map_err(|e| Error::Binding(Some(e)))
    }
    fn cast_spell(
        &mut self,
        player: &Self::Node,
        book: SpellBook,
        spell: i32,
        target: &Self::Node,
    ) -> Result<bool> {
        let p = self.player(player)?;
        let target = match target {
            GNode::Entity(e) => Some(e.clone()),
            _ => None,
        };
        super::magic::cast_spell(self.live, &p, book, spell, target.as_ref())
            .map_err(|e| Error::Binding(Some(e)))
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
        // QuestRepository.hasCompletedAll over the live quest registry (empty:
        // quest classes are content; see docs/live-world.md).
        let p = self.player(player)?;
        let registry = self.live.inner.world.quests.clone();
        self.live
            .inner
            .with_profile(&p, |profile| {
                profile.persisted.quests.has_completed_all(&registry)
            })
            .map_err(bind)
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
        let p = self.player(player)?;
        self.tabs(&p)
            .music(|tabs, music| music.toggle_looping(tabs))
            .map_err(bind)
    }
    fn unlocked(&mut self, player: &Self::Node, slot: i32) -> Result<Option<Song>> {
        let p = self.player(player)?;
        let id = self
            .tabs(&p)
            .music(|_, music| Ok(music.unlocked.unlocked.get(slot).copied()))
            .map_err(bind)?;
        Ok(id.map(|id| Song {
            id,
            index: self
                .live
                .inner
                .world
                .songs
                .songs
                .get(&id)
                .map_or(-1, |s| s.index),
        }))
    }
    fn play(&mut self, player: &Self::Node, song: Option<Song>) -> Result<()> {
        let p = self.player(player)?;
        let song = song.ok_or(Error::Null)?;
        self.tabs(&p)
            .music(|tabs, music| music.play(tabs, song.id))
            .map_err(bind)
    }
    fn songs(&mut self) -> Result<Vec<Song>> {
        Ok(self
            .live
            .inner
            .world
            .songs
            .songs
            .iter()
            .map(|(id, s)| Song {
                id: *id,
                index: s.index,
            })
            .collect())
    }
    fn unlock_song(&mut self, player: &Self::Node, id: i32) -> Result<()> {
        let p = self.player(player)?;
        self.tabs(&p)
            .music(|tabs, music| music.unlock(tabs, id, true))
            .map_err(bind)
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
        Ok(self
            .live
            .global
            .ladders
            .get_or_insert_with(climb::Ladders::original)
            .destination(at))
    }
    fn special_ladder(&mut self, at: [i32; 3]) -> Result<Option<&'static climb::SpecialLadder>> {
        Ok(self
            .live
            .global
            .ladders
            .get_or_insert_with(climb::Ladders::original)
            .special(at))
    }
    fn scenery_size(&mut self, scenery: &Self::Node) -> Result<[i32; 2]> {
        let definition = Self::scenery(scenery)?
            .read()
            .definition
            .clone()
            .ok_or(Error::Null)?;
        let d = definition.read();
        Ok([d.size_x, d.size_y])
    }
    fn scenery_rotation(&mut self, scenery: &Self::Node) -> Result<i32> {
        Ok(Self::scenery(scenery)?.read().rotation)
    }
    fn scenery_walking_flag(&mut self, scenery: &Self::Node) -> Result<i32> {
        let definition = Self::scenery(scenery)?
            .read()
            .definition
            .clone()
            .ok_or(Error::Null)?;
        let flag = definition.read().walking_flag;
        Ok(flag)
    }
    fn scenery_options(&mut self, scenery: &Self::Node) -> Result<Option<Vec<Option<String>>>> {
        let definition = Self::scenery(scenery)?
            .read()
            .definition
            .clone()
            .ok_or(Error::Null)?;
        let options = definition.read().options.clone();
        Ok(options.map(|o| o.read().clone()))
    }
    fn get_object(&mut self, at: [i32; 3]) -> Result<Option<Self::Node>> {
        let [x, y, z] = at;
        let live = &mut *self.live;
        Ok(live
            .walk
            .scene
            .context(&mut live.walk.maps)
            .object(Location::new(x, y, z), -1, -1)
            .map_err(|e| bind(e.0))?
            .map(GNode::Scenery))
    }
    fn teleport_permitted(&mut self, at: [i32; 3]) -> Result<bool> {
        let [x, y, z] = at;
        let live = &mut *self.live;
        live.walk
            .scene
            .context(&mut live.walk.maps)
            .teleport_permitted(Some(&Location::new(x, y, z)))
            .map_err(|e| bind(e.0))
    }
    fn clipping_flag(&mut self, at: [i32; 3]) -> Result<i32> {
        let [x, y, z] = at;
        let live = &mut *self.live;
        live.walk
            .scene
            .context(&mut live.walk.maps)
            .clipping(Location::new(x, y, z), false)
            .map_err(|e| bind(e.0))
    }
    fn set_teleport_location(&mut self, player: &Self::Node, at: [i32; 3]) -> Result<()> {
        let p = self.player(player)?;
        let [x, y, z] = at;
        self.live
            .inner
            .state
            .actor(p.node())
            .map_err(bind)?
            .queue
            .state
            .write()
            .entity
            .teleport_location = Some(Location::new(x, y, z));
        Ok(())
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
        // interpreter.sendOptions(title, options...) (super::global_ui).
        let p = self.player(player)?;
        let lines = Lines::new(
            options
                .iter()
                .map(|o| Some(o.encode_utf16().collect()))
                .collect(),
        );
        let interpreter = self.live.interpreter_of(&p);
        let result =
            interpreter.send_options(self.live, Some(title.encode_utf16().collect()), &lines);
        self.live.drain_all();
        result.map(|_| ()).map_err(bind)
    }
    fn set_dialogue(
        &mut self,
        player: &Self::Node,
        dialogue: climb::ClimbDialogue<Self::Node>,
    ) -> Result<()> {
        // player.getDialogueInterpreter().setDialogue(ClimbDialogue).
        let p = self.player(player)?;
        self.live.set_climb_dialogue(&p, dialogue).map_err(bind)
    }
    fn dialogue_end(&mut self, player: &Self::Node) -> Result<()> {
        // DialoguePlugin.end(): the captured interpreter's close(), which is
        // the player's own interpreter.
        let p = self.player(player)?;
        let interpreter = self.live.interpreter_of(&p);
        let result = interpreter.close(self.live);
        self.live.drain_all();
        result.map(|_| ()).map_err(bind)
    }
}

/// The world's lower-casing for the InteractionListeners keys (as the item
/// listeners use it).
pub(super) struct RootCasing;
impl crate::world::interaction_listeners::Casing for RootCasing {
    fn lowercase_default(&self, s: &[u16]) -> crate::world::interaction_listeners::Text {
        lowercase_root(&String::from_utf16_lossy(s))
            .encode_utf16()
            .collect()
    }
    fn lowercase_root(&self, s: &[u16]) -> crate::world::interaction_listeners::Text {
        lowercase_root(&String::from_utf16_lossy(s))
            .encode_utf16()
            .collect()
    }
}

/// An option handler of these handlers, run right after the walking call that
/// reached it returns (it needs the whole world).
#[derive(Clone)]
pub struct HandlerCall {
    pub plugin: super::scenery::SceneryPlugin,
    pub player: NodeHandle,
    pub target: Option<Scenery>,
    pub option: String,
}

impl Live {
    /// `registry::register_scanned` over the live registries, then the
    /// bindings the registrations collected.
    pub fn install_global_handlers(&mut self, booth: [bool; 3]) -> std::result::Result<(), String> {
        let defaults = self.walk.scenery.defaults();
        let mut native = registry::Native {
            listeners: &self.listeners,
            components: &mut self.inner.registry,
            interactions: &self.item_actions.listeners,
            casing: &RootCasing,
            scenery_defaults: &defaults,
            player_options: Vec::new(),
            scenery_handlers: Vec::new(),
            login_plugins: Vec::new(),
            note_enabled: booth[1],
        };
        let classes = registry::register_scanned(&mut native).map_err(|e| format!("{e:?}"))?;
        let registry::Native {
            player_options,
            scenery_handlers,
            login_plugins,
            ..
        } = native;
        // SceneryDefinition.forId(id).getHandlers().put(key, handler).
        for (id, key, handler) in scenery_handlers {
            let definition = self.walk.scene.definitions.for_id(id);
            let handlers = definition.read().handlers.clone();
            handlers.write().insert(
                key,
                crate::definitions::property::Property::OptionHandler(handler),
            );
        }
        // Option._P_ATTACK/_P_FOLLOW.setHandler (super::follow).
        self.bind_player_options(&player_options);
        self.walk.interactions = Some(self.item_actions.listeners.clone());
        self.global.login_plugins = login_plugins;
        self.global.registered = classes;
        self.global.booth = booth;
        self.global.container_index = super::global_ui::CONTAINER_INDEX_START;
        Ok(())
    }

    /// A submitted body's pulse: `true` when it finished.
    pub fn global_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<driver_result::R> {
        let at = self.global.pulses.iter().position(|(h, _)| h.same(pulse))?;
        let (_, task) = self.global.pulses[at].clone();
        let player = match &task {
            Task::Retaliate { player }
            | Task::SpecialBar { player }
            | Task::Climb { player, .. }
            | Task::Follow { player, .. }
            | Task::ItemSelect { player }
            | Task::ConfigureWorld { player }
            | Task::Rules { player }
            | Task::Deposit { player, .. }
            | Task::RefreshDepositBox { player }
            | Task::ClanPrefix { player, .. }
            | Task::ClimbChoice { player, .. } => Some(player.clone()),
            Task::ReportClose | Task::DepositBoxClose => None,
        };
        let result = (|| -> Result<bool> {
            let player = player.ok_or(Error::Null)?;
            if let GNode::Entity(p) = &player
                && !self.inner.players.contains_key(&p.node())
            {
                return Ok(true);
            }
            let outcome = gh::run_task(&mut GHost { live: self }, &task, &player, None)?;
            Ok(match outcome {
                Outcome::Bool(done) => done,
                Outcome::Unit => true,
            })
        })();
        let done = !matches!(result, Ok(false));
        if done {
            self.global.pulses.remove(at);
        }
        let players: Vec<P> = self
            .inner
            .players
            .values()
            .map(|s| P(s.canonical.owner.clone()))
            .collect();
        for p in players {
            self.inner.drain(&p);
        }
        Some(result.map_err(|e| format!("{e:?}")))
    }

    /// Runs the option handler calls the walking calls deferred.
    pub fn run_handler_calls(&mut self, calls: Vec<HandlerCall>) {
        for call in calls {
            let p = P(call.player.clone());
            if !self.inner.players.contains_key(&p.node()) {
                continue;
            }
            let player = GNode::Entity(p.clone());
            let target = call.target.clone().map(GNode::Scenery);
            let result = match call.plugin {
                super::scenery::SceneryPlugin::Global(plugin) => registry::run_handler(
                    &mut GHost { live: self },
                    plugin,
                    &player,
                    target.as_ref(),
                    Some(&call.option),
                ),
                // CowMilkingPlugin.handle(player, node, option).
                super::scenery::SceneryPlugin::CowMilking => {
                    crate::world::creatures::cow_milking::handle(
                        self,
                        &p,
                        target.is_some(),
                        Some(&call.option),
                    )
                    .map_err(|e| Error::Binding(Some(format!("{e:?}"))))
                }
                // FurnaceOptionPlugin.handle(player, node, option).
                super::scenery::SceneryPlugin::Furnace => match call.target.clone() {
                    Some(furnace) => self
                        .furnace_handle(&p, &super::items::Node::Scenery(furnace))
                        .map_err(|e| Error::Binding(Some(format!("{e:?}")))),
                    None => Err(Error::Null),
                },
                // SmithingPlugin's OptionHandler: the plugin's handle with
                // the hammer as the used item.
                super::scenery::SceneryPlugin::Anvil => match call.target.as_ref() {
                    Some(anvil) => {
                        let id = anvil.read().id;
                        self.anvil_option(&p, id)
                            .map_err(|e| Error::Binding(Some(format!("{e:?}"))))
                    }
                    None => Err(Error::Null),
                },
            };
            match result {
                // InteractPlugin: a handler answering false is "Nothing
                // interesting happens."
                Ok(false) => {
                    let _ = self
                        .inner
                        .message(&p, crate::world::legacy_interaction::NOTHING);
                }
                Ok(true) => {}
                Err(e) => self
                    .inner
                    .log_line(format!("{} failed: {e:?}", call.plugin.class())),
            }
            self.inner.drain(&p);
        }
    }

    /// An interface button's InterfaceListeners callback of these handlers.
    pub fn global_button(
        &mut self,
        listener: registry::Listener,
        p: &NodeHandle,
        b: Button,
    ) -> Result<bool> {
        let player = GNode::Entity(P(p.clone()));
        registry::run_button(&mut GHost { live: self }, listener, &player, b)
    }
    /// An open or close callback of these handlers.
    pub fn global_open(
        &mut self,
        listener: registry::Listener,
        p: &NodeHandle,
        component: i32,
    ) -> Result<bool> {
        let player = GNode::Entity(P(p.clone()));
        registry::run_open(&mut GHost { live: self }, listener, &player, component)
    }
    /// `plugin.newInstance(player)` of a LOGIN plugin these handlers added
    /// (`LoginConfiguration.getLoginPlugins()`), by its class name; `None`
    /// when no registered login plugin has that name.
    pub fn global_login_plugin(&mut self, class: &str, p: &P) -> Option<Result<()>> {
        let plugin = self
            .global
            .login_plugins
            .iter()
            .copied()
            .find(|plugin| match plugin {
                registry::LoginPlugin::Validation => {
                    class == "content.global.handlers.player.LoginValidationPlugin"
                }
            })?;
        let player = GNode::Entity(p.clone());
        let result = registry::run_login(&mut GHost { live: self }, plugin, &player);
        self.inner.drain(p);
        Some(result)
    }
    /// A ComponentPlugin of these handlers.
    pub fn global_plugin(
        &mut self,
        plugin: registry::ComponentPlugin,
        p: &NodeHandle,
        b: Button,
    ) -> Result<bool> {
        let player = GNode::Entity(P(p.clone()));
        registry::run_plugin(&mut GHost { live: self }, plugin, &player, b)
    }
}

mod driver_result {
    pub type R = std::result::Result<bool, String>;
}
