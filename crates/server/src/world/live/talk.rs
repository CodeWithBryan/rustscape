//! Talking in the live world: a `DialogueInterpreter` per player
//! (`world::dialogue`) with the live world as its `Host`, the dialogue DSLs
//! (`world::dialogue_dsl`) and the banker conversation
//! (`dialogue_dsl::banker::talk_to`, which answers npc_classes'
//! `banker_conversation`), the Continue packet
//! (`interface_packets::continue_option`), input prompts
//! (`world::input_prompt`: `RunScript.processInput`, the InputPromptResponse
//! branch, `BankUtils.transferX`) and the content listeners' walk to an NPC
//! (`InteractionListeners.run`'s movement pulse, [`super::round::Approaches`]).
//!
//! The interpreter is made the first time a player's is asked for
//! (`new DialogueInterpreter(player)` in the Player constructor) and dropped
//! when the player leaves. The dialogue plugin registry (the static
//! `DialogueInterpreter.PLUGINS`) holds no content plugin: dialogue content
//! other than the banker conversation and the DSLs is not ported, so
//! `open(key)` of an NPC answers false as the original does with nothing
//! registered for that key.
use super::{P, host::Live};
use crate::{
    definitions::item::{Handle as Definition, Shared},
    inventory::item::{Handle as ItemHandle, Item},
    player::attributes::{AttributeObject, AttributeValue},
    world::{
        dialogue::{self as d, Interpreter, Lines, ScriptValue, Text},
        dialogue_dsl::{self as dsl, CloseEvent, CloseHook, RunScript, banker},
        doors::Location,
        input_prompt::{self as input, InputType, Response, Transfer},
        interfaces,
        player_walk::NodeHandle,
    },
};
use rs09_protocol::game::{Frame, updates};
use std::{collections::HashMap, sync::Arc};

/// The dialogue state of the live world.
#[derive(Default)]
pub struct TalkWorld {
    /// `DialogueInterpreter.PLUGINS` (no content plugin registered).
    pub registry: d::Registry<Live>,
    /// Each player's interpreter, by node.
    pub interpreters: HashMap<usize, Interpreter<Live>>,
    /// The `DialogueClosed` event hooks of each player (`Entity.hook`).
    pub close_hooks: HashMap<usize, Vec<CloseHook<Live>>>,
    /// The `runscript` callbacks the DSLs store, by player node (the
    /// attribute holds a [`ScriptMark`] for them).
    pub scripts: HashMap<usize, Script>,
}

/// `RandomEvents.randomIDs`: each constant's NPC id in declaration order
/// (from the pinned RandomEvents.kt and the event NPC classes' constructors).
pub const RANDOM_EVENT_IDS: [i32; 22] = [
    3117, 409, 3206, 2538, 410, 410, 2790, 2463, 407, 411, 2478, 956, 2476, 410, 2458, 2791, 438,
    2477, 391, 413, 425, 419,
];
/// `HolidayRandomEvents.holidayRandomIDs` (HolidayRandomEvents.kt and the
/// holiday NPC classes' constructors).
pub const HOLIDAY_RANDOM_IDS: [i32; 14] = [
    4607, 61, 2716, 2714, 611, 2862, 1023, 3312, 8540, 6742, 8517, 6746, 6740, 4239,
];

/// The `runscript` attribute of a DSL prompt: its callback is in
/// [`TalkWorld::scripts`]. Its text is the pinned Function1's.
pub struct ScriptMark;
impl AttributeObject for ScriptMark {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn java_string(&self) -> String {
        "(kotlin.Any) -> kotlin.Unit".into()
    }
    fn function_arity(&self) -> Option<u8> {
        Some(1)
    }
}

fn bind(e: impl std::fmt::Debug) -> d::Error {
    d::Error::Binding(format!("{e:?}"))
}
fn refuse<T>(what: &str) -> d::Result<T> {
    Err(d::Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}
fn string(text: &[u16]) -> String {
    String::from_utf16_lossy(text)
}
fn utf16(text: &str) -> Text {
    text.encode_utf16().collect()
}

impl Live {
    fn talk_player(&self, player: &NodeHandle) -> d::Result<P> {
        if self.inner.players.contains_key(&player.node) {
            Ok(P(player.clone()))
        } else {
            Err(d::Error::Cast)
        }
    }
    fn talk_npc(&self, npc: &P) -> d::Result<&crate::startup::npcs::Npc> {
        self.inner.npcs.rows.get(&npc.node()).ok_or(d::Error::Null)
    }
    /// `player.getDialogueInterpreter()`.
    pub fn interpreter_of(&mut self, p: &P) -> Interpreter<Live> {
        let registry = self.round.talk.registry.clone();
        self.round
            .talk
            .interpreters
            .entry(p.node())
            .or_insert_with(|| Interpreter::new(Some(p.0.clone()), registry))
            .clone()
    }
    /// A frame that takes `InterfaceManager.getPacketCount(1)`.
    fn counted(&mut self, p: &P, frame: impl FnOnce(i32) -> Frame) {
        let counter = self.inner.canonical(p).player.manager().packet_count(1);
        let frame = frame(counter);
        self.inner.queue_native(p, frame);
    }
    fn manager_owner(&self, m: &interfaces::Manager) -> d::Result<P> {
        let owner = m.player().ok_or(d::Error::Null)?;
        self.talk_player(&owner)
    }
}

impl d::Host for Live {
    type Npc = P;
    type Item = ItemHandle;
    type ItemDefinition = Definition;
    type Dispatch = P;
    type Manager = interfaces::Manager;
    type Component = interfaces::Handle;
    type WalkingQueue = P;
    type PulseManager = P;

    fn interpreter(&mut self, player: &NodeHandle) -> d::Result<Option<Interpreter<Self>>> {
        let p = self.talk_player(player)?;
        Ok(Some(self.interpreter_of(&p)))
    }
    fn dispatch(&mut self, player: &NodeHandle) -> d::Result<P> {
        self.talk_player(player)
    }
    fn manager(&mut self, player: &NodeHandle) -> d::Result<interfaces::Manager> {
        let p = self.talk_player(player)?;
        Ok(self.inner.canonical(&p).player.manager())
    }
    fn username(&mut self, player: &NodeHandle) -> d::Result<Option<Text>> {
        let p = self.talk_player(player)?;
        Ok(Some(utf16(&self.inner.slot(&p).name)))
    }
    fn is_male(&mut self, player: &NodeHandle) -> d::Result<bool> {
        let p = self.talk_player(player)?;
        Ok(self.inner.canonical(&p).player.appearance().is_male())
    }
    fn is_debug(&mut self, player: &NodeHandle) -> d::Result<bool> {
        // Player.isDebug(): an administrator with the debug attribute.
        let p = self.talk_player(player)?;
        let admin = self
            .inner
            .with_profile(&p, |profile| profile.details.rights())
            .map_err(bind)?
            .map_err(bind)?
            == crate::player::details::Rights::Administrator;
        if !admin {
            return Ok(false);
        }
        self.inner
            .canonical(&p)
            .player
            .attributes()
            .boolean("debug", false)
            .map_err(|_| d::Error::Cast)
    }
    fn player_id(&mut self, player: &NodeHandle) -> d::Result<i32> {
        let p = self.talk_player(player)?;
        Ok(self.inner.slot(&p).index)
    }
    fn send_message(&mut self, player: &NodeHandle, message: Text) -> d::Result<()> {
        let p = self.talk_player(player)?;
        self.inner.message(&p, &string(&message)).map_err(bind)
    }
    fn set_attribute(
        &mut self,
        player: &NodeHandle,
        key: &str,
        value: d::Attribute,
    ) -> d::Result<()> {
        let p = self.talk_player(player)?;
        let value = match value {
            d::Attribute::Int(i) => AttributeValue::Int(i),
            d::Attribute::Bool(b) => AttributeValue::Bool(b),
        };
        self.inner
            .canonical(&p)
            .player
            .attributes()
            .set_value(key, Some(value));
        Ok(())
    }
    fn dispatch_event(&mut self, player: &NodeHandle, event: d::Event<Self>) -> d::Result<()> {
        match event {
            d::Event::Close(_) => {
                // Entity.dispatch(DialogueCloseEvent): the hooks of a copy of
                // the player's list (DialogueLabeller.afterClose's).
                let hooks = self
                    .round
                    .talk
                    .close_hooks
                    .get(&player.node)
                    .cloned()
                    .unwrap_or_default();
                for hook in hooks {
                    hook.process(self, player)?;
                }
                Ok(())
            }
            d::Event::Open(_) => {
                self.inner
                    .not_ported
                    .skip("DialogueOpenEvent dispatch (no hook other than the DSLs' close hook)");
                Ok(())
            }
            d::Event::OptionSelection { .. } => {
                self.inner.not_ported.skip(
                    "DialogueOptionSelectionEvent dispatch (no hook other than the DSLs' close hook)",
                );
                Ok(())
            }
        }
    }
    fn send_string(
        &mut self,
        p: P,
        text: Option<Text>,
        interface: i32,
        child: i32,
    ) -> d::Result<()> {
        let text = string(&text.ok_or(d::Error::Null)?);
        self.inner.text(&p, &text, interface, child).map_err(bind)
    }
    fn send_interface_config(
        &mut self,
        p: P,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> d::Result<()> {
        self.counted(&p, |c| {
            updates::interface_hidden(hidden, c, interface.wrapping_shl(16) | child)
        });
        Ok(())
    }
    fn send_animation_interface(
        &mut self,
        p: P,
        animation: i32,
        interface: i32,
        child: i32,
    ) -> d::Result<()> {
        self.counted(&p, |c| {
            updates::interface_animation(interface.wrapping_shl(16) | child, animation, c)
        });
        Ok(())
    }
    fn send_item_on_interface(
        &mut self,
        p: P,
        item: i32,
        amount: i32,
        interface: i32,
        child: i32,
    ) -> d::Result<()> {
        self.counted(&p, |c| {
            updates::interface_object(interface.wrapping_shl(16) | child, item, amount, c)
        });
        Ok(())
    }
    fn send_npc_on_interface(
        &mut self,
        p: P,
        npc: i32,
        interface: i32,
        child: i32,
    ) -> d::Result<()> {
        self.counted(&p, |c| {
            updates::interface_npc_head(interface.wrapping_shl(16) | child, npc, c)
        });
        Ok(())
    }
    fn send_player_on_interface(&mut self, p: P, interface: i32, child: i32) -> d::Result<()> {
        self.counted(&p, |c| {
            updates::interface_player_head(interface.wrapping_shl(16) | child, c)
        });
        Ok(())
    }
    fn send_reposition_on_interface(
        &mut self,
        p: P,
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> d::Result<()> {
        self.counted(&p, |c| {
            updates::interface_position(interface.wrapping_shl(16) | child, x, y, c)
        });
        Ok(())
    }
    fn send_angle_on_interface(
        &mut self,
        p: P,
        interface: i32,
        child: i32,
        zoom: i32,
        pitch: i32,
        yaw: i32,
    ) -> d::Result<()> {
        self.counted(&p, |c| {
            updates::interface_angle(interface.wrapping_shl(16) | child, pitch, yaw, zoom, c)
        });
        Ok(())
    }
    fn send_run_script(
        &mut self,
        p: P,
        id: i32,
        types: &str,
        arguments: Vec<ScriptValue>,
    ) -> d::Result<()> {
        let mut objects = Vec::with_capacity(arguments.len());
        for argument in arguments {
            objects.push(match argument {
                ScriptValue::Int(i) => updates::ScriptArg::Int(i),
                ScriptValue::Bool(b) => updates::ScriptArg::Int(i32::from(b)),
                ScriptValue::Text(t) => updates::ScriptArg::Text(string(&t).into_bytes()),
                ScriptValue::Null => return Err(d::Error::Null),
            });
        }
        let counter = self.inner.canonical(&p).player.manager().packet_count(1);
        let frame =
            updates::run_client_script(id, types.as_bytes(), &objects, counter).map_err(bind)?;
        self.inner.queue_native(&p, frame);
        Ok(())
    }
    fn open_chatbox(&mut self, manager: interfaces::Manager, interface: i32) -> d::Result<()> {
        let p = self.manager_owner(&manager)?;
        self.inner.current = Some(p.clone());
        let component = self.inner.registry.component(interface);
        interfaces::open_chatbox(&mut self.inner, &manager, &component).map_err(bind)?;
        self.inner.drain(&p);
        Ok(())
    }
    fn close_chatbox(&mut self, manager: interfaces::Manager) -> d::Result<()> {
        let p = self.manager_owner(&manager)?;
        self.inner.current = Some(p.clone());
        interfaces::close_chatbox(&mut self.inner, &manager).map_err(bind)?;
        self.inner.drain(&p);
        Ok(())
    }
    fn chatbox(&mut self, manager: interfaces::Manager) -> d::Result<Option<interfaces::Handle>> {
        Ok(manager.fields.read().chatbox.clone())
    }
    fn component_id(&mut self, component: &interfaces::Handle) -> d::Result<i32> {
        Ok(component.read().id)
    }
    fn has_close_event(&mut self, component: &interfaces::Handle) -> d::Result<bool> {
        Ok(component.read().close_event.is_some())
    }
    fn npc_id(&mut self, npc: &P) -> d::Result<i32> {
        Ok(self.talk_npc(npc)?.id)
    }
    fn shown_npc(&mut self, npc: &P, _: Option<&NodeHandle>) -> d::Result<P> {
        // NPC.getShownNPC: the NPC itself unless its definition morphs.
        if self
            .talk_npc(npc)?
            .definition
            .read()
            .child_npc_ids
            .is_some()
        {
            return refuse("NPC.getShownNPC for a morphing NPC (child ids)");
        }
        Ok(npc.clone())
    }
    fn set_dialogue_player(&mut self, npc: &P, player: Option<&NodeHandle>) -> d::Result<()> {
        // NPC.setDialoguePlayer: the NPC tick kernel's field (the NPC
        // stands still while its dialogue player has a chatbox open).
        self.talk_npc(npc)?.brain.fields.write().dialogue = player.map(|p| p.node);
        Ok(())
    }
    fn walking_queue(&mut self, npc: &P) -> d::Result<P> {
        self.talk_npc(npc)?;
        Ok(npc.clone())
    }
    fn reset_walking_queue(&mut self, queue: P) -> d::Result<()> {
        let node = queue.node();
        self.with_walk(|world| crate::world::movement::Context::walk_reset(world, None, node, None))
            .map_err(bind)
    }
    fn pulse_manager(&mut self, npc: &P) -> d::Result<P> {
        self.talk_npc(npc)?;
        Ok(npc.clone())
    }
    fn clear_pulses(&mut self, manager: P) -> d::Result<()> {
        let node = manager.node();
        self.with_walk(|world| world.clear_pulses(node))
            .map_err(bind)
    }
    fn npc_name(&mut self, id: i32) -> d::Result<Option<Text>> {
        Ok(self
            .npc_definition_name(id)
            .map_err(bind)?
            .map(|n| utf16(&n)))
    }
    fn item_definition(&mut self, id: i32) -> d::Result<Definition> {
        Ok(self.inner.items.for_id(id))
    }
    fn model_zoom(&mut self, definition: &Definition) -> d::Result<i32> {
        Ok(definition.read().model_zoom)
    }
    fn model_rotation_x(&mut self, definition: &Definition) -> d::Result<i32> {
        Ok(definition.read().model_rotation_x)
    }
    fn model_rotation_y(&mut self, definition: &Definition) -> d::Result<i32> {
        Ok(definition.read().model_rotation_y)
    }
    fn definition_name(&mut self, definition: &Definition) -> d::Result<Option<Text>> {
        Ok(definition.read().name.as_deref().map(utf16))
    }
    fn destroy_message(&mut self, definition: &Definition) -> d::Result<Option<Text>> {
        use crate::definitions::property::Property;
        Ok(match definition.read().handlers.get("destroy_message") {
            Some(Property::Text(t)) => Some(utf16(t)),
            _ => None,
        })
    }
    fn new_item(&mut self, id: i32) -> d::Result<ItemHandle> {
        // new Item(id): amount 1, charge 1000.
        Ok(Shared::new(Item::new(id, 1, 1000, &self.inner.items)))
    }
    fn item_id(&mut self, item: &ItemHandle) -> d::Result<i32> {
        Ok(item.read().id())
    }
    fn item_amount(&mut self, item: &ItemHandle) -> d::Result<i32> {
        Ok(item.read().amount())
    }
    fn server_name(&mut self) -> d::Result<Option<Text>> {
        Ok(Some(utf16(&self.inner.constants.server_name)))
    }
    fn log(&mut self, origin: &str, level: d::LogLevel, message: Text) -> d::Result<()> {
        self.inner
            .log_line(format!("[{origin}] {level:?} {}", string(&message)));
        Ok(())
    }
    fn stderr(&mut self, message: Text) -> d::Result<()> {
        self.inner
            .log_line(format!("System.err {}", string(&message)));
        Ok(())
    }
    fn stack_trace(&mut self, error: &d::Error) -> d::Result<()> {
        self.inner.log_line(format!("printStackTrace {error:?}"));
        Ok(())
    }
}

impl dsl::Host for Live {
    type Location = Location;

    fn location(&mut self, player: &NodeHandle) -> d::Result<Option<Location>> {
        self.inner.state.node_location(player.node).map_err(bind)
    }
    fn face_location(&mut self, npc: &P, location: &Location) -> d::Result<()> {
        if self.inner.is_npc_node(npc.node()) {
            return self
                .npc_face_location(npc.node(), Some(location.clone()))
                .map(|_| ())
                .map_err(bind);
        }
        refuse("faceLocation of a player from a dialogue")
    }
    fn new_npc(&mut self, _: i32) -> d::Result<P> {
        refuse("new NPC(id) from dialogue content")
    }
    fn new_item_amount(&mut self, id: i32, amount: i32) -> d::Result<ItemHandle> {
        Ok(Shared::new(Item::new(id, amount, 1000, &self.inner.items)))
    }
    fn set_value(
        &mut self,
        player: &NodeHandle,
        key: &str,
        value: dsl::Value<Self>,
    ) -> d::Result<()> {
        let p = self.talk_player(player)?;
        let value = match value {
            dsl::Value::Bool(b) => AttributeValue::Bool(b),
            dsl::Value::Int(i) => AttributeValue::Int(i),
            dsl::Value::InputType(kind) => AttributeValue::Object(Arc::new(kind)),
            dsl::Value::Script(script) => {
                self.round
                    .talk
                    .scripts
                    .insert(p.node(), Script::Dsl(script));
                AttributeValue::Object(Arc::new(ScriptMark))
            }
        };
        self.inner
            .canonical(&p)
            .player
            .attributes()
            .set_value(key, Some(value));
        Ok(())
    }
    fn attribute_bool(&mut self, player: &NodeHandle, key: &str, default: bool) -> d::Result<bool> {
        let p = self.talk_player(player)?;
        self.inner
            .canonical(&p)
            .player
            .attributes()
            .boolean(key, default)
            .map_err(|_| d::Error::Cast)
    }
    fn set_close_event(
        &mut self,
        component: &interfaces::Handle,
        event: CloseEvent,
    ) -> d::Result<()> {
        let CloseEvent::Unclosable = event;
        component.write().close_event = Some(interfaces::CloseEvent::new(
            crate::world::component_objects::Unclosable,
        ));
        Ok(())
    }
    fn hook_dialogue_closed(
        &mut self,
        player: &NodeHandle,
        hook: CloseHook<Self>,
    ) -> d::Result<()> {
        self.talk_player(player)?;
        self.round
            .talk
            .close_hooks
            .entry(player.node)
            .or_default()
            .push(hook);
        Ok(())
    }
    fn unhook(&mut self, player: &NodeHandle, hook: &CloseHook<Self>) -> d::Result<()> {
        if let Some(hooks) = self.round.talk.close_hooks.get_mut(&player.node)
            && let Some(at) = hooks.iter().position(|h| h.same(hook))
        {
            hooks.remove(at);
        }
        Ok(())
    }
    fn quest_stage(&mut self, player: &NodeHandle, quest: &str) -> d::Result<i32> {
        // The quest registry holds no quest (quests are content): the
        // persisted stage of a quest the DSL names.
        let _ = (player, quest);
        refuse("QuestRepository.getStage from a dialogue (quests are content)")
    }
    fn stdout(&mut self, message: Text) -> d::Result<()> {
        self.inner
            .log_line(format!("System.out {}", string(&message)));
        Ok(())
    }
    fn debug(&mut self, player: &NodeHandle, message: Text) -> d::Result<()> {
        // player.debug(message): sent when the player is in debug mode.
        if d::Host::is_debug(self, player)? {
            let p = self.talk_player(player)?;
            self.inner
                .message(&p, &format!("[debug] {}", string(&message)))
                .map_err(bind)?;
        }
        Ok(())
    }
    fn item_name(&mut self, item: &ItemHandle) -> d::Result<Option<Text>> {
        let definition = item.read().definition.clone().ok_or(d::Error::Null)?;
        let name = definition.read().name.clone();
        Ok(name.as_deref().map(utf16))
    }
    fn send_item_zoom_on_interface(
        &mut self,
        p: P,
        item: i32,
        zoom: i32,
        interface: i32,
        child: i32,
    ) -> d::Result<()> {
        self.counted(&p, |c| {
            updates::interface_object(interface.wrapping_shl(16) | child, item, zoom, c)
        });
        Ok(())
    }
    fn send_child_position(
        &mut self,
        player: &NodeHandle,
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> d::Result<()> {
        let p = self.talk_player(player)?;
        self.counted(&p, |c| {
            updates::interface_position(interface.wrapping_shl(16) | child, x, y, c)
        });
        Ok(())
    }
    fn inventory_amount(&mut self, player: &NodeHandle, item: &ItemHandle) -> d::Result<i32> {
        let p = self.talk_player(player)?;
        Ok(self
            .inner
            .canonical(&p)
            .player
            .inventory()
            .amount(Some(item)))
    }
    fn dispatch_message(&mut self, p: P, message: Text) -> d::Result<()> {
        self.inner.message(&p, &string(&message)).map_err(bind)
    }
    fn slayer_started(&mut self, _: &NodeHandle) -> d::Result<bool> {
        refuse("SlayerManager (slayer content)")
    }
    fn register_item_option(&mut self, _: i32, _: &str) -> d::Result<()> {
        refuse("ActivateEnchantedGem's item option registration (slayer content)")
    }
}

impl banker::BankHost for Live {
    fn has_seal_of_passage(&mut self, _: &NodeHandle) -> d::Result<bool> {
        refuse("hasSealOfPassage (Lunar Isle content)")
    }
    fn has_ultimate_ironman_restriction(&mut self, player: &NodeHandle) -> d::Result<bool> {
        let p = self.talk_player(player)?;
        self.has_ironman_restriction(&p, crate::player::ironman::Mode::Ultimate)
            .map_err(bind)
    }
    fn has_awaiting_grand_exchange_collections(&mut self, player: &NodeHandle) -> d::Result<bool> {
        let p = self.talk_player(player)?;
        self.ge_awaiting_collections(&p).map_err(bind)
    }
    fn open_bank_account(&mut self, player: &NodeHandle) -> d::Result<()> {
        let p = self.talk_player(player)?;
        Live::open_bank_account(self, &p).map_err(bind)
    }
    fn activate_secondary_bank_account(&mut self, _: &NodeHandle) -> d::Result<banker::Activation> {
        refuse(
            "activateSecondaryBankAccount (buying the secondary bank: amountInBank and the 5m removal are not ported)",
        )
    }
    fn toggle_bank_account(&mut self, player: &NodeHandle) -> d::Result<()> {
        // restrictForIronman(player, ULTIMATE) { if activated, flip
        // player.useSecondaryBank }.
        let p = self.talk_player(player)?;
        let manager = self.inner.canonical(&p).player.ironman();
        let restricted = manager
            .check(
                &mut super::bank::BankUi::new(&mut self.inner),
                Some(crate::player::ironman::Mode::Ultimate),
            )
            .map_err(bind)?;
        self.inner.drain(&p);
        if restricted || !banker::BankHost::has_activated_secondary_bank_account(self, player)? {
            return Ok(());
        }
        let player = self.inner.canonical(&p).player.clone();
        player.set_use_secondary_bank(!player.use_secondary_bank());
        Ok(())
    }
    fn is_using_secondary_bank_account(&mut self, player: &NodeHandle) -> d::Result<bool> {
        let p = self.talk_player(player)?;
        Ok(self.inner.canonical(&p).player.use_secondary_bank())
    }
    fn has_activated_secondary_bank_account(&mut self, player: &NodeHandle) -> d::Result<bool> {
        // getAttribute(player, "UnlockedSecondaryBank", false).
        let p = self.talk_player(player)?;
        self.inner
            .canonical(&p)
            .player
            .attributes()
            .boolean("UnlockedSecondaryBank", false)
            .map_err(|_| d::Error::Cast)
    }
    fn open_bank_pin_settings(&mut self, _: &NodeHandle) -> d::Result<()> {
        refuse("openBankPinSettings (the bank PIN settings interface is not installed)")
    }
    fn open_grand_exchange_collection_box(&mut self, player: &NodeHandle) -> d::Result<()> {
        let p = self.talk_player(player)?;
        self.ge_open_collection_box(&p).map_err(bind)
    }
    fn remove_item(&mut self, player: &NodeHandle, item: &ItemHandle) -> d::Result<bool> {
        let p = self.talk_player(player)?;
        self.remove_from_inventory(&p, item).map_err(bind)
    }
    fn api_message(&mut self, player: &NodeHandle, message: Text) -> d::Result<()> {
        let p = self.talk_player(player)?;
        self.inner.message(&p, &string(&message)).map_err(bind)
    }
    fn second_bank(&mut self) -> d::Result<bool> {
        Ok(self.round.second_bank)
    }
}

// ---------------------------------------------------------------- Continue

/// `PacketProcessor`'s ContinueOption branch over the player's interpreter
/// (`interface_packets::continue_option`).
impl Live {
    pub fn continue_option(
        &mut self,
        p: &P,
        action: crate::world::interface_packets::Action,
    ) -> Result<(), String> {
        let owner = p.0.clone();
        let result = crate::world::interface_packets::continue_option(
            &mut super::buttons::Buttons { live: self },
            &owner,
            action,
        );
        self.inner.drain(p);
        result.map_err(|e| format!("{e:?}"))
    }
    /// `player.getDialogueInterpreter().close()`.
    pub fn close_dialogue_of(&mut self, p: &P) -> d::Result<bool> {
        let interpreter = self.interpreter_of(p);
        let result = interpreter.close(self);
        self.inner.drain(p);
        result
    }
    /// `player.getDialogueInterpreter().getDialogue() != null`.
    pub fn has_dialogue(&mut self, p: &P) -> bool {
        self.interpreter_of(p).dialogue().is_some()
    }
}

// ---------------------------------------------------------------- input prompts

/// A `runscript` attribute's callback.
#[derive(Clone)]
pub enum Script {
    /// `BankUtils.transferX`'s captured lambda.
    Transfer(Arc<Transfer>),
    /// A callback a DSL stored (`sendInputDialogue`).
    Dsl(RunScript<Live>),
    /// The shops' Buy-X/Sell-X handler (`shop::run_amount`).
    Shop(crate::world::shops::shop::AmountScript),
    /// A trade's Offer-X/Remove-X handler (`trade::plugin::run_amount`).
    Trade(crate::world::trade::AmountScript<NodeHandle>),
    /// CookingDialogue's amount handler (`cooking::AmountInput::run`).
    Cook(super::items::cooking::CookScript),
    /// SmeltingInterface's amount handler (`smelting::AmountInput::run`).
    Smelt(super::items::smelting::SmeltScript),
    /// SmithingInterface's amount handler (`smithing::AmountInput::run`).
    Smith(super::items::smithing::SmithScript),
}

struct Prompt<'a> {
    live: &'a mut Live,
}

fn ierror(e: impl std::fmt::Debug) -> input::Error {
    input::Error::Binding(format!("{e:?}"))
}

impl Prompt<'_> {
    fn p(&self, owner: &NodeHandle) -> input::Result<P> {
        self.live.talk_player(owner).map_err(ierror)
    }
}

impl input::Context for Prompt<'_> {
    type Value = Response;
    type Script = Script;
    type Interpreter = Interpreter<Live>;
    type Bank = crate::inventory::bank::Bank;

    fn script(&mut self, owner: &NodeHandle) -> input::Result<Option<Script>> {
        let p = self.p(owner)?;
        match self
            .live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .get("runscript")
        {
            None => Ok(None),
            Some(AttributeValue::Object(object)) => {
                if object.as_any().downcast_ref::<ScriptMark>().is_some() {
                    return self
                        .live
                        .round
                        .talk
                        .scripts
                        .get(&p.node())
                        .cloned()
                        .map(Some)
                        .ok_or(input::Error::Null);
                }
                if let Some(transfer) = object.as_any().downcast_ref::<Transfer>() {
                    return Ok(Some(Script::Transfer(Arc::new(transfer.clone()))));
                }
                Err(input::Error::Type)
            }
            Some(_) => Err(input::Error::Type),
        }
    }
    fn interaction_locked(&mut self, owner: &NodeHandle) -> input::Result<bool> {
        let state = &mut self.live.inner.state;
        state
            .locks(owner)
            .map_err(ierror)?
            .is_kind_locked(
                crate::world::action_locks::Kind::Interaction,
                state,
                &mut crate::world::action_locks::Clock,
            )
            .map_err(ierror)
    }
    fn input_type(&mut self, owner: &NodeHandle) -> input::Result<InputType> {
        let p = self.p(owner)?;
        match self
            .live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .get("input-type")
        {
            None => Ok(InputType::Numeric),
            Some(AttributeValue::Object(v)) => v
                .as_any()
                .downcast_ref::<InputType>()
                .copied()
                .ok_or(input::Error::Type),
            Some(_) => Err(input::Error::Type),
        }
    }
    fn parse_amount(&mut self, owner: &NodeHandle) -> input::Result<bool> {
        let p = self.p(owner)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .boolean("parseamount", false)
            .map_err(|_| input::Error::Type)
    }
    fn string(&mut self, value: &Response) -> input::Result<Vec<u16>> {
        Ok(match value {
            Response::Int(n) => n.to_string().encode_utf16().collect(),
            Response::Text(s) => s.clone(),
        })
    }
    fn invoke(&mut self, script: Script, value: Response) -> input::Result<()> {
        match script {
            Script::Transfer(transfer) => input::invoke_transfer(self, &transfer, &value),
            Script::Dsl(script) => script.invoke(self.live, value).map_err(|e| match e {
                d::Error::Number => input::Error::Number,
                d::Error::Cast => input::Error::Type,
                other => ierror(other),
            }),
            Script::Trade(script) => self
                .live
                .trade_amount(&script, &value)
                .map_err(|e| match e {
                    crate::world::trade::Error::Number => input::Error::Number,
                    crate::world::trade::Error::ClassCast => input::Error::Type,
                    other => ierror(other),
                }),
            Script::Cook(script) => {
                let value = match &value {
                    Response::Int(n) => crate::world::cooking::Input::Int(*n),
                    Response::Text(text) => {
                        crate::world::cooking::Input::Text(String::from_utf16_lossy(text))
                    }
                };
                self.live.cooking_amount(&script, &value).map_err(ierror)
            }
            Script::Smelt(script) => {
                let value = match &value {
                    Response::Int(n) => crate::world::smelting::Input::Int(*n),
                    Response::Text(text) => {
                        crate::world::smelting::Input::Text(String::from_utf16_lossy(text))
                    }
                };
                self.live.smelting_amount(&script, &value).map_err(ierror)
            }
            Script::Smith(script) => {
                let value = match &value {
                    Response::Int(n) => crate::world::smelting::Input::Int(*n),
                    Response::Text(text) => {
                        crate::world::smelting::Input::Text(String::from_utf16_lossy(text))
                    }
                };
                self.live.smithing_amount(&script, &value).map_err(ierror)
            }
            Script::Shop(script) => self
                .live
                .shop_call(None, |host| {
                    crate::world::shops::shop::run_amount(host, &script, &value)
                })
                .map_err(|e| match e {
                    crate::world::shops::Error::Number => input::Error::Number,
                    crate::world::shops::Error::ClassCast => input::Error::Type,
                    other => ierror(other),
                }),
        }
    }
    fn dialogue(&mut self, owner: &NodeHandle, text: Vec<u16>) -> input::Result<()> {
        let p = self.p(owner)?;
        let interpreter = self.live.interpreter_of(&p);
        interpreter
            .send_dialogue(self.live, Some(&d::line(&text)))
            .map(|_| ())
            .map_err(ierror)
    }
    fn remove(&mut self, owner: &NodeHandle, key: &str) -> input::Result<()> {
        let p = self.p(owner)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .remove(key);
        if key == "runscript" {
            self.live.round.talk.scripts.remove(&p.node());
        }
        Ok(())
    }
    fn set_parse_amount(&mut self, owner: &NodeHandle, value: bool) -> input::Result<()> {
        let p = self.p(owner)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .set_boolean("parseamount", value);
        Ok(())
    }
    fn interpreter(&mut self, owner: &NodeHandle) -> input::Result<Interpreter<Live>> {
        let p = self.p(owner)?;
        Ok(self.live.interpreter_of(&p))
    }
    fn send_input(
        &mut self,
        d: Interpreter<Live>,
        string: bool,
        prompt: Vec<u16>,
    ) -> input::Result<()> {
        d.send_input(self.live, string, vec![ScriptValue::Text(prompt)])
            .map_err(ierror)
    }
    fn send_long(&mut self, d: Interpreter<Live>, prompt: Vec<u16>) -> input::Result<()> {
        d.send_long_input(self.live, vec![ScriptValue::Text(prompt)])
            .map_err(ierror)
    }
    fn send_message(&mut self, d: Interpreter<Live>, prompt: Vec<u16>) -> input::Result<()> {
        d.send_message_input(self.live, Some(prompt))
            .map_err(ierror)
    }
    fn set_script(&mut self, owner: &NodeHandle, script: Script) -> input::Result<()> {
        let p = self.p(owner)?;
        let value = match script {
            Script::Transfer(transfer) => AttributeValue::Object(transfer),
            other => {
                self.live.round.talk.scripts.insert(p.node(), other);
                AttributeValue::Object(Arc::new(ScriptMark))
            }
        };
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .set_value("runscript", Some(value));
        Ok(())
    }
    fn set_type(&mut self, owner: &NodeHandle, kind: InputType) -> input::Result<()> {
        let p = self.p(owner)?;
        self.live
            .inner
            .canonical(&p)
            .player
            .attributes()
            .set_value("input-type", Some(AttributeValue::Object(Arc::new(kind))));
        Ok(())
    }
    fn make_transfer(&mut self, transfer: Transfer) -> input::Result<Script> {
        self.p(&transfer.player)?;
        if transfer.after.is_some() {
            return Err(input::Error::Binding(
                "not ported in the live world: a transfer's after callback".into(),
            ));
        }
        Ok(Script::Transfer(Arc::new(transfer)))
    }
    fn bank(&mut self, owner: &NodeHandle) -> input::Result<crate::inventory::bank::Bank> {
        let p = self.p(owner)?;
        Ok(self.live.inner.canonical(&p).player.bank())
    }
    fn take(
        &mut self,
        bank: crate::inventory::bank::Bank,
        slot: i32,
        amount: i32,
    ) -> input::Result<()> {
        let owner = bank.owner().ok_or(input::Error::Null)?;
        let p = self.p(&owner)?;
        self.live.bank_take(&p, &bank, slot, amount).map_err(ierror)
    }
    fn deposit(
        &mut self,
        bank: crate::inventory::bank::Bank,
        slot: i32,
        amount: i32,
    ) -> input::Result<()> {
        let owner = bank.owner().ok_or(input::Error::Null)?;
        let p = self.p(&owner)?;
        self.live
            .bank_deposit(&p, &bank, slot, amount)
            .map_err(ierror)
    }
    fn last_amount(
        &mut self,
        bank: crate::inventory::bank::Bank,
        amount: i32,
    ) -> input::Result<()> {
        let owner = bank.owner().ok_or(input::Error::Null)?;
        let p = self.p(&owner)?;
        self.live
            .bank_last_amount(&p, &bank, amount)
            .map_err(ierror)
    }
    fn after(&mut self, _: crate::world::component_objects::Plugin) -> input::Result<()> {
        Err(input::Error::Binding(
            "not ported in the live world: a transfer's after callback".into(),
        ))
    }
}

impl Live {
    /// The InputPromptResponse branch (`RunScript.processInput` through
    /// `input_prompt::receive`).
    pub fn input_response(&mut self, p: &P, value: Response) -> Result<(), String> {
        let owner = p.0.clone();
        let result = input::receive(&mut Prompt { live: self }, &owner, value);
        self.inner.drain(p);
        result.map_err(|e| format!("{e:?}"))
    }
    /// `sendInputDialogue(player, true, prompt, handler)` of a
    /// shop's Buy-X or Sell-X.
    pub fn shop_prompt(
        &mut self,
        p: &P,
        prompt: &str,
        script: crate::world::shops::shop::AmountScript,
    ) -> input::Result<()> {
        let owner = p.0.clone();
        let result = input::send(
            &mut Prompt { live: self },
            &owner,
            InputType::Amount,
            utf16(prompt),
            Script::Shop(script),
        );
        self.inner.drain(p);
        result
    }
    /// `sendInputDialogue(player, true, prompt, handler)` of
    /// CookingDialogue's Cook-X.
    pub fn cook_prompt(
        &mut self,
        p: &P,
        prompt: &str,
        script: super::items::cooking::CookScript,
    ) -> input::Result<()> {
        let owner = p.0.clone();
        let result = input::send(
            &mut Prompt { live: self },
            &owner,
            InputType::Numeric,
            utf16(prompt),
            Script::Cook(script),
        );
        self.inner.drain(p);
        result
    }
    /// `sendInputDialogue(player, true, prompt, handler)` of
    /// SmeltingInterface's Smelt-X.
    pub fn smelt_prompt(
        &mut self,
        p: &P,
        prompt: &str,
        script: super::items::smelting::SmeltScript,
    ) -> input::Result<()> {
        let owner = p.0.clone();
        let result = input::send(
            &mut Prompt { live: self },
            &owner,
            InputType::Numeric,
            utf16(prompt),
            Script::Smelt(script),
        );
        self.inner.drain(p);
        result
    }
    /// `sendInputDialogue(player, true, prompt, handler)` of
    /// SmithingInterface's Make-X.
    pub fn smith_prompt(
        &mut self,
        p: &P,
        prompt: &str,
        script: super::items::smithing::SmithScript,
    ) -> input::Result<()> {
        let owner = p.0.clone();
        let result = input::send(
            &mut Prompt { live: self },
            &owner,
            InputType::Numeric,
            utf16(prompt),
            Script::Smith(script),
        );
        self.inner.drain(p);
        result
    }
    /// `sendInputDialogue(player, numeric, prompt, handler)` of a trade's
    /// Offer-X or Remove-X.
    pub fn trade_prompt(
        &mut self,
        p: &P,
        numeric: bool,
        prompt: &str,
        script: crate::world::trade::AmountScript<NodeHandle>,
    ) -> input::Result<()> {
        let owner = p.0.clone();
        let kind = if numeric {
            InputType::Numeric
        } else {
            InputType::Short
        };
        let result = input::send(
            &mut Prompt { live: self },
            &owner,
            kind,
            utf16(prompt),
            Script::Trade(script),
        );
        self.inner.drain(p);
        result
    }
    /// `BankUtils.transferX(player, slot, withdraw)`.
    pub fn transfer_x(&mut self, p: &P, slot: i32, withdraw: bool) -> input::Result<()> {
        let owner = p.0.clone();
        let result = input::transfer_x(&mut Prompt { live: self }, &owner, slot, withdraw, None);
        self.inner.drain(p);
        result
    }
}

// ---------------------------------------------------------------- the walk to an NPC

use super::items::Node as INode;
use super::round::{Pending, PendingArrival};
use crate::world::{
    interaction::{Approach, ApproachKind, OptionData},
    interaction_listeners::{self as listeners, Arrival, Function, FunctionKind, Kind as IntType},
    movement::{self, Movement},
    npc_classes::{
        self as nc,
        listeners::{Destination, Interaction},
    },
};

fn lerr(e: impl std::fmt::Debug) -> listeners::Error {
    listeners::Error::Binding(format!("{e:?}"))
}

impl Live {
    /// `InteractionListeners.run`'s `MovementPulse(player, node, flag,
    /// destOverride)`: a movement of the canonical state toward the node
    /// whose arrival is kept by a fresh option handle. A destination
    /// override is evaluated now, when the pulse is made (the original
    /// evaluates it at each path search; the overrides installed here are
    /// BankerNPC's, over NPCs that do not walk while talked to).
    pub fn listener_movement(
        &mut self,
        arrival: Arrival<INode, INode>,
        flag: Option<listeners::Flag>,
        dest: Option<Function>,
    ) -> listeners::Result<Movement> {
        let (player, target) = match &arrival {
            Arrival::Empty { player, target } | Arrival::On { player, target, .. } => {
                (player.clone(), target.clone())
            }
            Arrival::Use { player, with, .. } => (player.clone(), with.clone()),
        };
        let INode::Player(player) = player else {
            return Err(listeners::Error::Type);
        };
        let override_method = match dest {
            None => None,
            Some(function) => {
                let at = self.listener_destination(&function, &target)?;
                let approaches = &mut self.walk.approaches;
                approaches.next_token = approaches.next_token.wrapping_add(1);
                let token = approaches.next_token;
                if let Some(old) = approaches.player_tokens.insert(player.node, token) {
                    approaches.destinations.remove(&old);
                }
                approaches.destinations.insert(token, (function, at));
                Some(token)
            }
        };
        let flag = flag.map(|f| match f {
            listeners::Flag::Entity => movement::Flag::Entity,
            listeners::Flag::Item => movement::Flag::Item,
            listeners::Flag::Object => movement::Flag::Object,
        });
        self.approach_movement(
            player,
            &target,
            flag,
            override_method,
            Pending::Listener(arrival),
        )
    }
    /// A `MovementPulse(player, node[, flag, destOverride])` toward an NPC,
    /// a player or a scene object whose arrival body is `pending`: a
    /// movement of the canonical state whose arrival is kept by a fresh
    /// option handle ([`super::round::Approaches`]).
    pub fn approach_movement(
        &mut self,
        player: crate::world::player_walk::NodeHandle,
        target: &INode,
        flag: Option<movement::Flag>,
        override_method: Option<usize>,
        pending: Pending,
    ) -> listeners::Result<Movement> {
        let (target_node, lease) = match target {
            INode::Npc(h) | INode::Player(h) => (h.node, None),
            // A scene object's node in the canonical state (the one its
            // options walk to); the lease keeps it until the arrival.
            INode::Scenery(object) => {
                let lease = self
                    .inner
                    .state
                    .scenery_target(object.clone())
                    .map_err(lerr)?;
                (lease.node, Some(lease))
            }
            _ => {
                return Err(listeners::Error::Binding(
                    "not ported in the live world: a content listener's walk to an item".into(),
                ));
            }
        };
        let option = Shared::new(OptionData::new(None, -1));
        let approaches = &mut self.walk.approaches;
        approaches.arrivals.retain(|a| a.player != player.node);
        approaches.arrivals.push(PendingArrival {
            option: option.clone(),
            player: player.node,
            pending,
            lease,
        });
        let approach = Approach {
            player: player.node,
            target: Some(target_node),
            option: Some(option),
            kind: ApproachKind::Option,
        };
        self.with_walk(|world| world.listener_motion(approach, flag, override_method))
            .map_err(lerr)
    }
    /// `pulseManager.run(pulse)` of a content listener's movement.
    pub fn run_listener_movement(&mut self, pulse: Movement) -> listeners::Result<()> {
        let node = pulse.mover().ok_or(listeners::Error::Null)?;
        self.with_walk(|world| world.run_motion(node, pulse))
            .map_err(lerr)
    }
    /// A destination override (`setDest`) with the node.
    fn listener_destination(
        &mut self,
        function: &Function,
        target: &INode,
    ) -> listeners::Result<Option<Location>> {
        if function
            .value::<super::shopping::ShopDestination>()
            .is_some()
        {
            let INode::Npc(npc) = target else {
                return Err(listeners::Error::Type);
            };
            let [x, y, z] = self.shop_destination(&P(npc.clone())).map_err(lerr)?;
            return Ok(Some(Location::new(x, y, z)));
        }
        let Some(destination) = function.value::<Destination>() else {
            return Err(listeners::Error::Binding(
                "not ported in the live world: a destination override this world did not register"
                    .into(),
            ));
        };
        let INode::Npc(npc) = target else {
            return Err(listeners::Error::Type);
        };
        self.join_banker(npc.node).map_err(lerr)?;
        let npc = P(npc.clone());
        let at = nc::listeners::run_destination(
            &mut super::subclasses::Classes::new(self),
            *destination,
            Some(&npc),
        )
        .map_err(lerr)?;
        Ok(at.map(|[x, y, z]| Location::new(x, y, z)))
    }
    /// A content listener's movement arrived: `InteractionListeners`'
    /// arrival body (zone, face, the handler) with the whole world.
    pub fn listener_arrival(
        &mut self,
        option: &crate::world::interaction::OptionHandle,
    ) -> listeners::Result<()> {
        let Some(at) = self
            .walk
            .approaches
            .arrivals
            .iter()
            .position(|a| a.option.same(option))
        else {
            return Ok(());
        };
        let PendingArrival {
            player: node,
            pending,
            lease,
            ..
        } = self.walk.approaches.arrivals.remove(at);
        if let Some(token) = self.walk.approaches.player_tokens.remove(&node) {
            self.walk.approaches.destinations.remove(&token);
        }
        let result = match pending {
            Pending::Listener(arrival) => {
                listeners::arrival(&mut super::items::Ctx(self), arrival).map(|_| ())
            }
            // UseWithHandler's MovementPulse arrived: its pulse body.
            Pending::UseWith(body) => {
                crate::world::packet_handlers::use_with::pulse(&mut super::items::Ctx(self), &body)
                    .map(|_| ())
                    .map_err(lerr)
            }
        };
        drop(lease);
        if let Some(p) = self.player_of(node) {
            self.inner.drain(&p);
        }
        result
    }
    /// An NPC listener of NPCTalkListener or BankerNPC
    /// (`npc_classes::listeners::run_interaction`) with the live world as
    /// its host.
    pub fn npc_listener(
        &mut self,
        interaction: Interaction,
        player: &INode,
        node: &INode,
    ) -> listeners::Result<bool> {
        let (INode::Player(player), INode::Npc(npc)) = (player, node) else {
            return Err(listeners::Error::Type);
        };
        let (player, npc) = (P(player.clone()), P(npc.clone()));
        let result = nc::listeners::run_interaction(
            &mut super::subclasses::Classes::new(self),
            interaction,
            &player,
            Some(&npc),
        )
        .map_err(lerr);
        self.inner.drain(&player);
        result
    }

    /// The install round's start-up: BankInterface's listeners, the shared
    /// InterfaceListeners registry for `runOpen`, and the NPC listeners of
    /// NPCTalkListener and BankerNPC (`defineListeners` and
    /// `defineDestinationOverrides`, in the scan's order: BankerNPC first).
    pub fn install_round(&mut self) -> Result<(), String> {
        self.define_bank_interface()
            .map_err(|e| format!("BankInterface: {e:?}"))?;
        self.inner.interface_listeners = self.listeners.clone();
        let registry = self.item_actions.listeners.clone();
        let text = |s: &str| -> listeners::Text { s.encode_utf16().collect() };
        let texts = |o: &[&str]| -> Vec<listeners::Text> { o.iter().map(|s| text(s)).collect() };
        let on = |live: &mut Live, ids: Option<&[i32]>, options: &[&str], f: Interaction| {
            let function = Function::new(FunctionKind::On, f.class(), f);
            let options = texts(options);
            let kind = IntType::Npc.ordinal();
            match ids {
                Some(ids) => {
                    registry.add_ids(&super::items::Ctx(live), ids, kind, &options, function)
                }
                None => registry.add_generics(&super::items::Ctx(live), &options, kind, function),
            }
            .map_err(|e| format!("{e:?}"))
        };
        for name in crate::startup::scan::section("scan.contentInterfaces") {
            match nc::listeners::Listener::named(name) {
                Some(nc::listeners::Listener::Banker) => {
                    // BankerNPC(): its construction as an AbstractNPC
                    // prototype is not made (the class is not registered in
                    // this world's start-up).
                    self.inner
                        .not_ported
                        .skip("BankerNPC's prototype construction (the class is not registered)");
                    let ids = nc::Class::Banker.ids();
                    on(self, Some(ids), &["bank"], Interaction::Bank)?;
                    on(self, Some(ids), &["collect"], Interaction::Collect)?;
                    on(self, Some(ids), &["talk-to"], Interaction::BankerTalk)?;
                    let function = Function::new(
                        FunctionKind::Destination,
                        Destination::Banker.class(),
                        Destination::Banker,
                    );
                    registry
                        .add_destinations(
                            &super::items::Ctx(self),
                            IntType::Npc.ordinal(),
                            Some(ids),
                            &texts(&["bank", "collect", "talk-to"]),
                            function,
                        )
                        .map_err(|e| format!("{e:?}"))?;
                }
                Some(nc::listeners::Listener::Talk) => {
                    on(
                        self,
                        Some(&nc::talk::BARCRAWL_NPCS),
                        &["talk-to", "talk"],
                        Interaction::Barcrawl,
                    )?;
                    on(
                        self,
                        None,
                        &["talk-to", "talk", "talk to"],
                        Interaction::Talk,
                    )?;
                }
                None => {}
            }
        }
        // ConsumableListener.
        self.install_consumables()?;
        // TradeModule's components and TradeInterfacePlugin.
        self.install_trade();
        // Shops: defineInterfaceListeners, the NPC listeners, startup().
        self.install_shops()
    }

    /// A start-up banker as a `BankerNPC`, the first time it is interacted
    /// with. The start-up constructs the NPCs of `BankerNPC.NPC_IDS` as
    /// plain NPCs (it holds no region objects for the class's `init`); here
    /// the NPC gets the class and the booth part of `BankerNPC.init`
    /// (`findAdjacentBankBoothLocation` over the scene: the direction,
    /// `isWalks = false`, `facing_booth`), without repeating
    /// `super.init()`.
    pub fn join_banker(&mut self, node: usize) -> Result<(), String> {
        let ids = nc::Class::Banker.ids();
        match self.inner.npcs.rows.get_mut(&node) {
            Some(row) if row.class.is_none() && ids.contains(&row.id) => {
                row.class = Some(nc::Class::Banker.into());
            }
            _ => return Ok(()),
        }
        let npc = P(self
            .inner
            .state
            .retain_node(node)
            .map_err(|e| format!("{e:?}"))?);
        let mut host = super::subclasses::Classes::new(self);
        let booth = nc::banker::find_adjacent_bank_booth(&mut host, &npc)
            .map_err(|e| format!("BankerNPC.init: {e:?}"))?;
        if let Some((side, _)) = booth {
            nc::Host::set_direction(&mut host, &npc, Some(side)).map_err(|e| format!("{e:?}"))?;
            nc::Factory::set_walks(&mut host, &npc, false).map_err(|e| format!("{e:?}"))?;
            nc::Host::set_attribute(&mut host, &npc, "facing_booth", nc::Value::Bool(true))
                .map_err(|e| format!("{e:?}"))?;
        }
        Ok(())
    }
}
/// Lines of one text, for the dialogue senders.
pub fn lines(texts: &[&str]) -> Lines {
    dsl::lines(&dsl::texts(texts))
}
