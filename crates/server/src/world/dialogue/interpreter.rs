//! DialogueInterpreter: the per-player stage machine, its registry and every
//! `send*` form. No field guard is held across a host or content call.
use super::{
    Action, Actions, Arg, END_DIALOGUE, EmptyPlugin, Entity, Error, Event, FacialExpression, File,
    Host, Lines, NodeHandle, Plugin, Result, ScriptValue, Selection, Target, Text, Topics, read,
    substitute, text, write,
};
use crate::world::{interfaces, plain_dialogue as plain};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// The original static `PLUGINS` map. The reference keeps one per JVM; here
/// the owning world creates one and hands it to each interpreter.
pub struct Registry<H: Host>(Arc<RwLock<HashMap<i32, Option<Plugin<H>>>>>);
impl<H: Host> Clone for Registry<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> Default for Registry<H> {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
}
impl<H: Host> Registry<H> {
    pub fn new() -> Self {
        Self::default()
    }
    /// `DialogueInterpreter.add`. A null plugin is stored like any other
    /// value; a second registration of the id fails without replacing it.
    pub fn add(&self, id: i32, plugin: Option<&Plugin<H>>) -> Result<()> {
        let existing = read(&self.0).get(&id).cloned();
        if let Some(old) = existing {
            let old = old.ok_or(Error::Null)?.class_name().to_owned();
            let new = plugin.ok_or(Error::Null)?.class_name().to_owned();
            return Err(Error::IllegalArgument(format!(
                "Dialogue {} is already in use - [old={old}, new={new}]!",
                id & 0xFFFF
            )));
        }
        write(&self.0).insert(id, plugin.cloned());
        Ok(())
    }
    pub fn contains(&self, id: i32) -> bool {
        read(&self.0).contains_key(&id)
    }
    pub fn get(&self, id: i32) -> Option<Plugin<H>> {
        read(&self.0).get(&id).cloned().flatten()
    }
    /// `getDialogueKey`: `1 << 16 | name.hashCode()`.
    pub fn dialogue_key(name: &[u16]) -> i32 {
        let hash = name
            .iter()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(*c)));
        (1 << 16) | hash
    }
}

struct Inner<H: Host> {
    player: Option<NodeHandle>,
    registry: Registry<H>,
    dialogue: RwLock<Option<Plugin<H>>>,
    key: RwLock<i32>,
    actions: Actions<H>,
    topics: Topics<H>,
}
/// DialogueInterpreter handle; clones are the same object.
pub struct Interpreter<H: Host>(Arc<Inner<H>>);
impl<H: Host> Clone for Interpreter<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Lets the verified plain sender run against this module's host.
struct Plain<'a, H: Host> {
    host: &'a mut H,
    failure: Option<Error>,
}
impl<H: Host> Plain<'_, H> {
    fn lift<T>(&mut self, result: Result<T>) -> interfaces::Result<T> {
        result.map_err(|error| {
            let mapped = match error {
                Error::Null => interfaces::Error::Null,
                Error::Index => interfaces::Error::Index,
                _ => interfaces::Error::Binding("dialogue host".into()),
            };
            self.failure = Some(error);
            mapped
        })
    }
}
impl<H: Host> plain::Context for Plain<'_, H> {
    type Interpreter = Interpreter<H>;
    type Dispatch = H::Dispatch;
    type Manager = H::Manager;
    type Component = H::Component;
    fn receiver(&mut self, _: &Interpreter<H>) -> interfaces::Result<()> {
        Ok(())
    }
    fn interpreter(&mut self, p: &NodeHandle) -> interfaces::Result<Interpreter<H>> {
        let found = self.host.interpreter(p).and_then(|i| i.ok_or(Error::Null));
        self.lift(found)
    }
    fn player(&mut self, d: &Interpreter<H>) -> interfaces::Result<NodeHandle> {
        d.0.player.clone().ok_or(interfaces::Error::Null)
    }
    fn dispatch(&mut self, p: &NodeHandle) -> interfaces::Result<H::Dispatch> {
        let found = self.host.dispatch(p);
        self.lift(found)
    }
    fn text(
        &mut self,
        d: H::Dispatch,
        text: Option<Text>,
        iface: i32,
        child: i32,
    ) -> interfaces::Result<()> {
        let sent = self.host.send_string(d, text, iface, child);
        self.lift(sent)
    }
    fn manager(&mut self, p: &NodeHandle) -> interfaces::Result<H::Manager> {
        let found = self.host.manager(p);
        self.lift(found)
    }
    fn open_chatbox(&mut self, m: H::Manager, id: i32) -> interfaces::Result<()> {
        let opened = self.host.open_chatbox(m, id);
        self.lift(opened)
    }
    fn chatbox(&mut self, m: H::Manager) -> interfaces::Result<Option<H::Component>> {
        let found = self.host.chatbox(m);
        self.lift(found)
    }
    fn id(&mut self, c: H::Component) -> interfaces::Result<i32> {
        let id = self.host.component_id(&c);
        self.lift(id)
    }
    fn hidden(
        &mut self,
        d: H::Dispatch,
        id: i32,
        child: i32,
        hidden: bool,
    ) -> interfaces::Result<()> {
        let sent = self.host.send_interface_config(d, id, child, hidden);
        self.lift(sent)
    }
}

impl<H: Host> Interpreter<H> {
    /// `new DialogueInterpreter(player)`; the reference accepts a null player.
    pub fn new(player: Option<NodeHandle>, registry: Registry<H>) -> Self {
        Self(Arc::new(Inner {
            player,
            registry,
            dialogue: RwLock::new(None),
            key: RwLock::new(0),
            actions: Actions::<H>::new(Vec::new()),
            topics: Topics::<H>::new(Vec::new()),
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn player(&self) -> Option<NodeHandle> {
        self.0.player.clone()
    }
    pub fn registry(&self) -> Registry<H> {
        self.0.registry.clone()
    }
    /// `getDialogue()`.
    pub fn dialogue(&self) -> Option<Plugin<H>> {
        read(&self.0.dialogue).clone()
    }
    pub fn set_dialogue(&self, dialogue: Option<Plugin<H>>) {
        let old = std::mem::replace(&mut *write(&self.0.dialogue), dialogue);
        drop(old);
    }
    /// The private `key` field: the last key passed to `open`.
    pub fn key(&self) -> i32 {
        *read(&self.0.key)
    }
    /// The live `getActions()` list.
    pub fn actions(&self) -> Actions<H> {
        self.0.actions.clone()
    }
    pub fn add_action(&self, action: Option<Action<H>>) {
        self.0.actions.write().push(action);
    }
    /// The live public `activeTopics` list.
    pub fn active_topics(&self) -> Topics<H> {
        self.0.topics.clone()
    }
    fn clear_topics(&self) {
        let old = std::mem::take(&mut *self.0.topics.write());
        drop(old);
    }
    fn own_player(&self) -> Result<NodeHandle> {
        self.0.player.clone().ok_or(Error::Null)
    }
    fn dispatch(&self, host: &mut H) -> Result<H::Dispatch> {
        host.dispatch(&self.own_player()?)
    }
    fn manager(&self, host: &mut H) -> Result<H::Manager> {
        host.manager(&self.own_player()?)
    }
    fn chatbox(&self, host: &mut H) -> Result<Option<H::Component>> {
        let manager = self.manager(host)?;
        host.chatbox(manager)
    }
    fn string(&self, host: &mut H, value: Option<Text>, interface: i32, child: i32) -> Result<()> {
        let dispatch = self.dispatch(host)?;
        host.send_string(dispatch, value, interface, child)
    }
    fn config(&self, host: &mut H, interface: i32, child: i32, hidden: bool) -> Result<()> {
        let dispatch = self.dispatch(host)?;
        host.send_interface_config(dispatch, interface, child, hidden)
    }
    /// Sends `messages[i]` for each index, reading the element after the
    /// dispatch getter as the reference's argument evaluation does.
    fn strings(&self, host: &mut H, lines: &Lines, interface: i32, first: i32) -> Result<()> {
        let count = lines.read().len();
        for i in 0..count {
            let dispatch = self.dispatch(host)?;
            let value = lines.read().get(i).cloned().ok_or(Error::Index)?;
            host.send_string(dispatch, value, interface, first.wrapping_add(i as i32))?;
        }
        Ok(())
    }
    fn open_and_return(&self, host: &mut H, interface: i32) -> Result<Option<H::Component>> {
        let manager = self.manager(host)?;
        host.open_chatbox(manager, interface)?;
        self.chatbox(host)
    }

    /// `open(String dialogueType, Object... args)`.
    pub fn open_named(&self, host: &mut H, name: Option<&[u16]>, args: &[Arg<H>]) -> Result<bool> {
        let key = Registry::<H>::dialogue_key(name.ok_or(Error::Null)?);
        self.open(host, key, args)
    }
    /// `open(int dialogueKey, Object... args)`. The previous dialogue is
    /// overwritten, not closed, and stays in place if `newInstance` fails.
    pub fn open(&self, host: &mut H, key: i32, args: &[Arg<H>]) -> Result<bool> {
        *write(&self.0.key) = key;
        let mut args = args.to_vec();
        if let Some(Arg::Npc(npc)) = args.first() {
            host.set_dialogue_player(npc, self.0.player.as_ref())?;
            // One try block: a failed queue reset also skips the pulse clear.
            let stopped = (|| {
                let queue = host.walking_queue(npc)?;
                host.reset_walking_queue(queue)?;
                let pulses = host.pulse_manager(npc)?;
                host.clear_pulses(pulses)
            })();
            if let Err(error) = stopped {
                host.stack_trace(&error)?;
            }
        } else if args.is_empty() {
            args.push(Arg::Int(key));
        }
        let Some(prototype) = self.0.registry.get(key) else {
            return Ok(false);
        };
        let player = self.own_player()?;
        if host.is_debug(&player)? {
            let message = format!("Dialogue opening - {}, key={key}", prototype.class_name());
            host.send_message(&player, text(&message))?;
        }
        let made = prototype.new_instance(host, self.0.player.clone())?;
        self.set_dialogue(made);
        let opened = match self.dialogue() {
            Some(dialogue) => dialogue.open(host, &args)?,
            None => false,
        };
        if !opened {
            self.set_dialogue(None);
            return Ok(false);
        }
        if let Some(dialogue) = self.dialogue() {
            host.dispatch_event(&player, Event::Open(dialogue))?;
        }
        Ok(true)
    }
    /// `open(DialogueFile file, Object... args)`.
    pub fn open_file(&self, host: &mut H, file: Option<&File<H>>, args: &[Arg<H>]) -> Result<()> {
        let plugin = EmptyPlugin::create(host, self.0.player.clone(), file.cloned())?;
        self.set_dialogue(Some(plugin));
        self.dialogue().ok_or(Error::Null)?.open(host, args)?;
        Ok(())
    }

    /// `handle(componentId, buttonId)`: one Continue or option click.
    pub fn handle(&self, host: &mut H, component_id: i32, button_id: i32) -> Result<()> {
        let player = self.own_player()?;
        host.set_attribute(
            &player,
            "chatbox-buttonid",
            super::Attribute::Int(button_id),
        )?;
        // The reference reads the player's current interpreter, not `this`.
        let current = |host: &mut H| -> Result<Plugin<H>> {
            host.interpreter(&player)?
                .ok_or(Error::Null)?
                .dialogue()
                .ok_or(Error::Null)
        };
        let mut ended = false;
        if current(host)?.file().is_some() {
            let file = current(host)?.file().ok_or(Error::Null)?;
            ended = file.current_stage(host)? == END_DIALOGUE;
        }
        if !ended {
            ended = current(host)?.stage() == END_DIALOGUE;
        }
        if ended {
            let manager = host.manager(&player)?;
            host.close_chatbox(manager)?;
            let manager = host.manager(&player)?;
            host.open_chatbox(manager, 137)?;
            current(host)?.set_finished(true);
            self.close(host)?;
            return Ok(());
        }
        let file = self.dialogue().ok_or(Error::Null)?.file();
        if button_id >= 2 && !self.0.topics.read().is_empty() {
            let topic = self
                .0
                .topics
                .read()
                .get((button_id - 2) as usize)
                .cloned()
                .ok_or(Error::Index)?;
            if !topic.skip_player {
                self.send_dialogues_for(
                    host,
                    Some(&Entity::Player(player.clone())),
                    topic.expr.animation_id(),
                    &super::line(&topic.text),
                )?;
            }
            match &topic.to_stage {
                Target::File(next) => {
                    self.dialogue().ok_or(Error::Null)?.load_file(Some(next))?;
                }
                Target::Stage(stage) => match &file {
                    None => self.dialogue().ok_or(Error::Null)?.set_stage(*stage),
                    Some(file) => file.set_stage(host, *stage)?,
                },
                Target::Other => {}
            }
            self.clear_topics();
            if topic.skip_player {
                self.handle(host, component_id, button_id)?;
            }
            return Ok(());
        }
        let option_id = button_id.wrapping_sub(1);
        if let Some(file) = file {
            let current_stage = file.stage(host)?;
            host.dispatch_event(
                &player,
                Event::OptionSelection {
                    dialogue: Selection::File(file.clone()),
                    current_stage,
                    option_id,
                },
            )?;
            file.handle(host, component_id, option_id)
        } else {
            let dialogue = self.dialogue().ok_or(Error::Null)?;
            let current_stage = dialogue.stage();
            host.dispatch_event(
                &player,
                Event::OptionSelection {
                    dialogue: Selection::Plugin(dialogue),
                    current_stage,
                    option_id,
                },
            )?;
            let dialogue = self.dialogue().ok_or(Error::Null)?;
            dialogue.handle(host, component_id, option_id).map(|_| ())
        }
    }

    /// `close()`. A chatbox component with a close event keeps the dialogue
    /// and still reports success; the queued actions are cleared either way.
    pub fn close(&self, host: &mut H) -> Result<bool> {
        if self.dialogue().is_some() {
            let old = std::mem::take(&mut *self.0.actions.write());
            drop(old);
            if self.chatbox(host)?.is_some() {
                let chatbox = self.chatbox(host)?.ok_or(Error::Null)?;
                if host.has_close_event(&chatbox)? {
                    return Ok(true);
                }
            }
            let closing = write(&self.0.dialogue).take();
            if let Some(dialogue) = closing {
                dialogue.close(host)?;
                let player = self.own_player()?;
                host.dispatch_event(&player, Event::Close(dialogue))?;
            }
        }
        self.clear_topics();
        Ok(self.dialogue().is_none())
    }

    /// `sendDialogue(String... messages)`: the verified plain sender.
    pub fn send_dialogue(
        &self,
        host: &mut H,
        lines: Option<&Lines>,
    ) -> Result<Option<H::Component>> {
        let mut bridge = Plain {
            host,
            failure: None,
        };
        let sent = plain::send_lines(&mut bridge, self.clone(), lines);
        sent.map_err(|error| {
            bridge.failure.take().unwrap_or(match error {
                interfaces::Error::Null => Error::Null,
                interfaces::Error::Index => Error::Index,
                other => Error::Binding(other.to_string()),
            })
        })
    }
    /// `sendPlainMessage(hideContinue, messages)`.
    pub fn send_plain_message(
        &self,
        host: &mut H,
        hide_continue: bool,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        self.send_dialogue(host, Some(lines))?;
        let dispatch = self.dispatch(host)?;
        let chatbox = self.chatbox(host)?.ok_or(Error::Null)?;
        let id = host.component_id(&chatbox)?;
        let child = (lines.read().len() as i32).wrapping_add(1);
        host.send_interface_config(dispatch, id, child, hide_continue)?;
        self.chatbox(host)
    }
    /// `sendDestroyItem(id, message)`; the message argument is unused.
    pub fn send_destroy_item(&self, host: &mut H, id: i32) -> Result<Option<H::Component>> {
        let definition = host.item_definition(id)?;
        let mut message = host
            .destroy_message(&definition)?
            .unwrap_or_else(|| text("Are you sure you want to destroy this object?"));
        if message.len() > 200 {
            // String.split(" ") drops trailing empty strings only.
            let mut words: Vec<&[u16]> = message.split(|c| *c == u16::from(b' ')).collect();
            while words.last().is_some_and(|w| w.is_empty()) {
                words.pop();
            }
            let mut joined = words.first().ok_or(Error::Index)?.to_vec();
            for (i, word) in words.iter().enumerate().skip(1) {
                if i == words.len() / 2 {
                    joined.extend(text("<br>"));
                } else {
                    joined.push(u16::from(b' '));
                }
                joined.extend_from_slice(word);
            }
            message = joined;
        }
        self.string(host, Some(message), 94, 7)?;
        let dispatch = self.dispatch(host)?;
        let definition = host.item_definition(id)?;
        let name = host.definition_name(&definition)?;
        host.send_string(dispatch, name, 94, 8)?;
        let dispatch = self.dispatch(host)?;
        host.send_item_on_interface(dispatch, id, 1, 94, 9)?;
        self.open_and_return(host, 94)
    }
    /// `sendPlaneMessageWithBlueTitle(title, messages)`.
    pub fn send_plane_message_with_blue_title(
        &self,
        host: &mut H,
        title: Option<Text>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        self.string(host, title, 372, 0)?;
        self.strings(host, lines, 372, 1)?;
        self.open_and_return(host, 372)
    }
    /// `sendScrollMessageWithBlueTitle(title, messages)`.
    pub fn send_scroll_message_with_blue_title(
        &self,
        host: &mut H,
        title: Option<Text>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        for i in 0..11 {
            self.string(host, Some(text(" ")), 421, i + 2)?;
        }
        self.string(host, title, 421, 1)?;
        self.strings(host, lines, 421, 2)?;
        self.open_and_return(host, 421)
    }
    /// `sendItemMessage(int itemId, String... messages)`.
    pub fn send_item_message_id(
        &self,
        host: &mut H,
        item: i32,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let item = host.new_item(item)?;
        self.send_item_message(host, &item, lines)
    }
    /// `sendItemMessage(Item item, String... messages)`. The message count is
    /// not range checked: the interface is always 240 plus the count.
    pub fn send_item_message(
        &self,
        host: &mut H,
        item: &H::Item,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let interface = 240i32.wrapping_add(lines.read().len() as i32);
        self.config(host, interface, 1, false)?;
        self.config(host, interface, 2, false)?;
        self.string(host, Some(Vec::new()), interface, 3)?;
        self.strings(host, lines, interface, 4)?;
        let definition = {
            let id = host.item_id(item)?;
            host.item_definition(id)?
        };
        let dispatch = self.dispatch(host)?;
        let (id, amount) = (host.item_id(item)?, host.item_amount(item)?);
        host.send_item_on_interface(dispatch, id, amount, interface, 2)?;
        let dispatch = self.dispatch(host)?;
        let zoom = host.model_zoom(&definition)? / 2;
        let (pitch, yaw) = (
            host.model_rotation_x(&definition)?,
            host.model_rotation_y(&definition)?,
        );
        host.send_angle_on_interface(dispatch, interface, 2, zoom, pitch, yaw)?;
        let dispatch = self.dispatch(host)?;
        host.send_reposition_on_interface(dispatch, interface, 2, 45, 45)?;
        self.config(host, interface, 1, true)?;
        self.open_and_return(host, interface)
    }
    /// `sendDoubleItemMessage(int first, int second, String... message)`.
    pub fn send_double_item_message_ids(
        &self,
        host: &mut H,
        first: i32,
        second: i32,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let first = host.new_item(first)?;
        let second = host.new_item(second)?;
        self.send_double_item_message(host, &first, &second, lines)
    }
    /// `sendDoubleItemMessage(Item first, Item second, String... messages)`.
    pub fn send_double_item_message(
        &self,
        host: &mut H,
        first: &H::Item,
        second: &H::Item,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let interface = 240i32.wrapping_add(lines.read().len() as i32);
        self.config(host, interface, 1, false)?;
        self.config(host, interface, 2, false)?;
        self.string(host, Some(Vec::new()), interface, 3)?;
        self.strings(host, lines, interface, 4)?;
        for (item, child, x, y) in [(first, 1, 40, 40), (second, 2, 60, 65)] {
            let definition = {
                let id = host.item_id(item)?;
                host.item_definition(id)?
            };
            let dispatch = self.dispatch(host)?;
            let (id, amount) = (host.item_id(item)?, host.item_amount(item)?);
            host.send_item_on_interface(dispatch, id, amount, interface, child)?;
            let dispatch = self.dispatch(host)?;
            // `(int) (zoom / 1.5)`: double division, truncated toward zero.
            let zoom = (f64::from(host.model_zoom(&definition)?) / 1.5) as i32;
            let (pitch, yaw) = (
                host.model_rotation_x(&definition)?,
                host.model_rotation_y(&definition)?,
            );
            host.send_angle_on_interface(dispatch, interface, child, zoom, pitch, yaw)?;
            let dispatch = self.dispatch(host)?;
            host.send_reposition_on_interface(dispatch, interface, child, x, y)?;
        }
        self.open_and_return(host, interface)
    }

    /// `sendDialogues(Entity, FacialExpression, String...)` and
    /// `sendDialogues(Entity, int, String...)`: a player speaks as -1, an NPC
    /// as the id of the NPC shown to this interpreter's player.
    pub fn send_dialogues_for(
        &self,
        host: &mut H,
        entity: Option<&Entity<H>>,
        expression: i32,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let npc_id = match entity.ok_or(Error::Null)? {
            Entity::Player(_) => -1,
            Entity::Npc(npc) => {
                let shown = host.shown_npc(npc, self.0.player.as_ref())?;
                host.npc_id(&shown)?
            }
        };
        self.send_dialogues(host, npc_id, expression, false, lines)
    }
    /// `sendDialogues(Entity, FacialExpression|int, boolean hide, String...)`:
    /// unlike the form without `hide`, this one uses `entity.getId()`.
    pub fn send_dialogues_for_hidden(
        &self,
        host: &mut H,
        entity: Option<&Entity<H>>,
        expression: i32,
        hide: bool,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let id = match entity.ok_or(Error::Null)? {
            Entity::Player(player) => host.player_id(player)?,
            Entity::Npc(npc) => host.npc_id(npc)?,
        };
        self.send_dialogues(host, id, expression, hide, lines)
    }
    /// `sendDialogues(int npcId, int expression, boolean hide, String...)`,
    /// the body every other overload reaches. Pass
    /// `FacialExpression::or_unset` for the FacialExpression overloads.
    pub fn send_dialogues(
        &self,
        host: &mut H,
        npc_id: i32,
        expression: i32,
        hide: bool,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let count = lines.read().len();
        if !(1..=4).contains(&count) {
            host.stderr(text(&format!("Invalid amount of messages: {count}")))?;
            return Ok(None);
        }
        let npc = npc_id > -1;
        let interface = if npc { 240 } else { 63 } + count as i32 + if hide { 4 } else { 0 };
        let expression = if expression == -1 {
            FacialExpression::HalfGuilty.animation_id()
        } else {
            expression
        };
        self.config(host, interface, 1, true)?;
        self.config(host, interface, 2, false)?;
        let dispatch = self.dispatch(host)?;
        host.send_animation_interface(dispatch, expression, interface, 2)?;
        if npc {
            let dispatch = self.dispatch(host)?;
            host.send_item_on_interface(dispatch, -1, 1, interface, 1)?;
            let dispatch = self.dispatch(host)?;
            host.send_item_on_interface(dispatch, -1, 1, interface, 2)?;
            let dispatch = self.dispatch(host)?;
            host.send_npc_on_interface(dispatch, npc_id, interface, 2)?;
            let dispatch = self.dispatch(host)?;
            host.send_reposition_on_interface(dispatch, interface, 2, 45, 45)?;
            let dispatch = self.dispatch(host)?;
            host.send_angle_on_interface(dispatch, interface, 2, 2150, 40, 1882)?;
            let dispatch = self.dispatch(host)?;
            let name = host.npc_name(npc_id)?;
            host.send_string(dispatch, name, interface, 3)?;
        } else {
            let dispatch = self.dispatch(host)?;
            host.send_reposition_on_interface(dispatch, interface, 2, 426, 45)?;
            let dispatch = self.dispatch(host)?;
            host.send_player_on_interface(dispatch, interface, 2)?;
            let dispatch = self.dispatch(host)?;
            let name = host.username(&self.own_player()?)?;
            host.send_string(dispatch, name, interface, 3)?;
        }
        for i in 0..count {
            let dispatch = self.dispatch(host)?;
            let message = lines.read().get(i).cloned().ok_or(Error::Index)?;
            let message = substitute(host, self.0.player.as_ref(), message.as_deref())?;
            host.send_string(dispatch, Some(message), interface, i as i32 + 4)?;
        }
        let dispatch = self.dispatch(host)?;
        let chatbox = self.chatbox(host)?.ok_or(Error::Null)?;
        let id = host.component_id(&chatbox)?;
        host.send_interface_config(dispatch, id, 3, false)?;
        self.open_and_return(host, interface)
    }

    /// `sendOptions(Object title, String... options)`; `title` is the result
    /// of the original `title.toString()`, or `None` for a null title.
    pub fn send_options(
        &self,
        host: &mut H,
        title: Option<Text>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let count = lines.read().len();
        if !(2..=5).contains(&count) {
            return Ok(None);
        }
        let interface = 224 + 2 * count as i32;
        if let Some(title) = title {
            self.string(host, Some(title), interface, 1)?;
        }
        for i in 0..count {
            let dispatch = self.dispatch(host)?;
            // `options[i].toString()`: a null element fails here.
            let option = lines
                .read()
                .get(i)
                .cloned()
                .ok_or(Error::Index)?
                .ok_or(Error::Null)?;
            host.send_string(dispatch, Some(option), interface, i as i32 + 2)?;
        }
        self.open_and_return(host, interface)
    }
    /// `sendInput(boolean string, Object... objects)`: script 109 or 108.
    pub fn send_input(&self, host: &mut H, string: bool, objects: Vec<ScriptValue>) -> Result<()> {
        let dispatch = self.dispatch(host)?;
        host.send_run_script(dispatch, if string { 109 } else { 108 }, "s", objects)
    }
    /// `sendLongInput(Object... objects)`: script 110.
    pub fn send_long_input(&self, host: &mut H, objects: Vec<ScriptValue>) -> Result<()> {
        let dispatch = self.dispatch(host)?;
        host.send_run_script(dispatch, 110, "s", objects)
    }
    /// `sendMessageInput(String reciever)`: script 107.
    pub fn send_message_input(&self, host: &mut H, receiver: Option<Text>) -> Result<()> {
        let dispatch = self.dispatch(host)?;
        let argument = receiver.map_or(ScriptValue::Null, ScriptValue::Text);
        host.send_run_script(dispatch, 107, "s", vec![argument])
    }
}
