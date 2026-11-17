//! DialogueLabeller, DialogueOption and `DialogueLabeller.open`.
//!
//! Content implements [`Conversation`]; its `add_conversation` is the
//! original `addConversation()` and is re-run on every pass through the
//! [`Dsl`] calls, exactly as the Kotlin DSL functions are. The labeller keeps
//! its own `stage` (the current pass) beside DialogueFile's backing field
//! (`super.stage`, the next stage), the label map, the counter and the
//! option/input memory. No lock is held across a host or content call.
use super::{
    Arg, ChatAnim, Dialogue, Entity, Error, File, Host, InputType, NodeHandle, Response, Result,
    RunScript, Text, Value, api, lines, split,
};
use crate::world::dialogue::{FileBody, Lines, ScriptValue, text};
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

/// `optionIf: (player, npc) -> Boolean`.
pub type Condition<H> = Arc<
    dyn Fn(&mut H, &NodeHandle, &<H as crate::world::dialogue::Host>::Npc) -> Result<bool>
        + Send
        + Sync,
>;

/// `DialogueOption(goto, option, spokenText = option, expression = NEUTRAL,
/// skipPlayer = false, optionIf = null)`.
pub struct DialogueOption<H: Host> {
    pub goto: String,
    pub option: Text,
    pub spoken_text: Text,
    pub expression: ChatAnim,
    pub skip_player: bool,
    pub option_if: Option<Condition<H>>,
}
impl<H: Host> DialogueOption<H> {
    pub fn new(goto: &str, option: &str) -> Self {
        Self {
            goto: goto.into(),
            option: text(option),
            spoken_text: text(option),
            expression: ChatAnim::Neutral,
            skip_player: false,
            option_if: None,
        }
    }
    pub fn spoken(mut self, spoken: &str) -> Self {
        self.spoken_text = text(spoken);
        self
    }
    pub fn expression(mut self, expression: ChatAnim) -> Self {
        self.expression = expression;
        self
    }
    pub fn skip_player(mut self, skip: bool) -> Self {
        self.skip_player = skip;
        self
    }
    pub fn only_if(
        mut self,
        f: impl Fn(&mut H, &NodeHandle, &<H as crate::world::dialogue::Host>::Npc) -> Result<bool>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.option_if = Some(Arc::new(f));
        self
    }
}

/// The DialogueLabeller subclass: `addConversation()` and the class name.
pub trait Conversation<H: Host>: Send + Sync + 'static {
    fn class_name(&self) -> &str;
    fn add_conversation(&self, host: &mut H, c: &Dsl<'_, H>) -> Result<()>;
}

struct State<H: Host> {
    label_stage_map: HashMap<String, i32>,
    dialogue_counter: i32,
    starting_stage: Option<i32>,
    stage: i32,
    stage_hit: bool,
    jump_to: Option<i32>,
    button_id: Option<i32>,
    filtered_options: Vec<Arc<DialogueOption<H>>>,
    opt_button: Option<i32>,
    opt_input: Option<Response>,
}

/// The DialogueLabeller fields, as read by observers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fields {
    pub label_stage_map: Vec<(String, i32)>,
    pub dialogue_counter: i32,
    pub starting_stage: Option<i32>,
    pub stage: i32,
    pub stage_hit: bool,
    pub jump_to: Option<i32>,
    pub button_id: Option<i32>,
    /// The `goto` of each filtered option.
    pub filtered_options: Vec<String>,
    pub opt_button: Option<i32>,
    pub opt_input: Option<Response>,
}

/// The DialogueLabeller file body.
pub struct Labeller<H: Host> {
    state: Mutex<State<H>>,
    conversation: Box<dyn Conversation<H>>,
}

/// A new DialogueLabeller file around this conversation.
pub fn file<H: Host>(conversation: impl Conversation<H>) -> File<H> {
    File::new(Labeller {
        state: Mutex::new(State {
            label_stage_map: HashMap::new(),
            dialogue_counter: 0,
            starting_stage: None,
            stage: -1,
            stage_hit: false,
            jump_to: None,
            button_id: None,
            filtered_options: Vec::new(),
            opt_button: None,
            opt_input: None,
        }),
        conversation: Box::new(conversation),
    })
}

/// `DialogueLabeller.open(player, dialogue, npc)`: the NPC faces the player's
/// location, takes the player as its dialogue player, stops its walking
/// queue and pulses, then `openDialogue(player, dialogue, npc)`.
pub fn open<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    dialogue: &Dialogue<H>,
    npc: &H::Npc,
) -> Result<()> {
    // face(npc, player.location): a null location fails the non-null parameter.
    let location = host.location(player)?.ok_or(Error::Null)?;
    host.face_location(npc, &location)?;
    host.set_dialogue_player(npc, Some(player))?;
    let queue = host.walking_queue(npc)?;
    host.reset_walking_queue(queue)?;
    let pulses = host.pulse_manager(npc)?;
    host.clear_pulses(pulses)?;
    api::open_dialogue(host, player, dialogue, &[Arg::Npc(npc.clone())])
}

impl<H: Host> Labeller<H> {
    fn st(&self) -> MutexGuard<'_, State<H>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn conversation(&self) -> &dyn Conversation<H> {
        &*self.conversation
    }
    pub fn fields(&self) -> Fields {
        let s = self.st();
        let mut map: Vec<(String, i32)> = s
            .label_stage_map
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        map.sort();
        Fields {
            label_stage_map: map,
            dialogue_counter: s.dialogue_counter,
            starting_stage: s.starting_stage,
            stage: s.stage,
            stage_hit: s.stage_hit,
            jump_to: s.jump_to,
            button_id: s.button_id,
            filtered_options: s.filtered_options.iter().map(|o| o.goto.clone()).collect(),
            opt_button: s.opt_button,
            opt_input: s.opt_input.clone(),
        }
    }
}

impl<H: Host> FileBody<H> for Labeller<H> {
    fn class_name(&self) -> &str {
        self.conversation.class_name()
    }
    /// `handle(componentID, buttonID)`: the label pass on the first call,
    /// then up to eleven passes following `goto` jumps; no stage hit ends.
    fn handle(&self, host: &mut H, this: &File<H>, _component: i32, button_id: i32) -> Result<()> {
        {
            let mut s = self.st();
            s.button_id = Some(button_id);
            s.starting_stage = None;
        }
        let c = Dsl {
            file: this,
            labeller: self,
        };
        if self.st().stage == -1 {
            self.st().dialogue_counter = 0;
            self.conversation.add_conversation(host, &c)?;
            let start = self.st().starting_stage.unwrap_or(0);
            this.set_base_stage(start);
        }
        for _ in 0..=10 {
            let jump = self.st().jump_to.take();
            if let Some(jump) = jump {
                this.set_base_stage(jump);
            }
            let next = this.base_stage();
            {
                let mut s = self.st();
                s.stage_hit = false;
                s.stage = next;
                s.dialogue_counter = 0;
            }
            self.conversation.add_conversation(host, &c)?;
            let s = self.st();
            if s.jump_to.is_none() || s.jump_to == Some(s.stage) {
                break;
            }
        }
        let hit = self.st().stage_hit;
        if !hit {
            this.end(host)?;
        }
        Ok(())
    }
    fn stage(&self, _host: &mut H, _this: &File<H>) -> Result<i32> {
        Ok(self.st().stage)
    }
    fn set_stage(&self, _host: &mut H, _this: &File<H>, value: i32) -> Result<()> {
        self.st().stage = value;
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The DSL receiver `addConversation` runs against: the labeller and its file.
pub struct Dsl<'a, H: Host> {
    file: &'a File<H>,
    labeller: &'a Labeller<H>,
}

impl<H: Host> Dsl<'_, H> {
    fn st(&self) -> MutexGuard<'_, State<H>> {
        self.labeller.st()
    }
    pub fn file(&self) -> &File<H> {
        self.file
    }
    /// `optButton`.
    pub fn opt_button(&self) -> Option<i32> {
        self.st().opt_button
    }
    /// `optInput`.
    pub fn opt_input(&self) -> Option<Response> {
        self.st().opt_input.clone()
    }
    /// `buttonID`.
    pub fn button_id(&self) -> Option<i32> {
        self.st().button_id
    }

    /// `assignIndividualStage(unclosable, callback)`.
    fn assign(
        &self,
        host: &mut H,
        unclosable: bool,
        callback: impl FnOnce(&mut H) -> Result<Option<H::Component>>,
    ) -> Result<()> {
        let run = {
            let mut s = self.st();
            if s.starting_stage.is_none() {
                s.starting_stage = Some(0);
            }
            s.stage == s.dialogue_counter && s.jump_to.is_none()
        };
        if run {
            let component = callback(host)?;
            if unclosable {
                if let Some(component) = &component {
                    let player = self.file.player();
                    api::set_unclosable(host, player.as_ref(), component)?;
                }
            }
            self.file
                .set_base_stage(self.file.base_stage().wrapping_add(1));
            self.st().stage_hit = true;
        }
        let mut s = self.st();
        s.dialogue_counter = s.dialogue_counter.wrapping_add(1);
        Ok(())
    }
    /// `formatMessages(messages, message)`.
    fn format(messages: &[Text], message: &[u16]) -> Result<Lines> {
        match messages.len() {
            0 => split(message),
            1 => split(&messages[0]),
            _ => Ok(lines(messages)),
        }
    }
    fn interpreter(&self) -> Result<crate::world::dialogue::Interpreter<H>> {
        self.file.interpreter().ok_or(Error::Null)
    }
    fn player_ref(&self) -> Result<NodeHandle> {
        self.file.player().ok_or(Error::Null)
    }

    /// `assignToIds(npcid)`: does nothing.
    pub fn assign_to_ids(&self, _npc: i32) {}

    /// `label(vararg label)`; the nesting block is the code that follows.
    pub fn label(&self, names: &[&str]) {
        let mut s = self.st();
        if s.starting_stage.is_none() {
            s.starting_stage = Some(1);
        }
        s.dialogue_counter = s.dialogue_counter.wrapping_add(1);
        let counter = s.dialogue_counter;
        for name in names {
            s.label_stage_map.insert((*name).into(), counter);
        }
    }
    /// `goto(label)`: a missing label leaves no jump.
    pub fn goto(&self, label: &str) {
        let mut s = self.st();
        if s.stage == s.dialogue_counter && s.jump_to.is_none() {
            s.jump_to = s.label_stage_map.get(label).copied();
        }
    }
    /// `loadLabel(player, label)`.
    pub fn load_label(&self, _player: &NodeHandle, label: &str) {
        self.goto(label);
    }
    /// `exec { player, npc -> }`.
    pub fn exec(
        &self,
        host: &mut H,
        callback: impl FnOnce(&mut H, &NodeHandle, &H::Npc) -> Result<()>,
    ) -> Result<()> {
        let run = {
            let mut s = self.st();
            if s.starting_stage.is_none() {
                s.starting_stage = Some(0);
            }
            s.stage == s.dialogue_counter && s.jump_to.is_none()
        };
        if run {
            let player = self.player_ref()?;
            let npc = self.file.npc().ok_or(Error::Null)?;
            callback(host, &player, &npc)?;
        }
        Ok(())
    }
    /// `manual(unclosable) { player, npc -> component }`.
    pub fn manual(
        &self,
        host: &mut H,
        unclosable: bool,
        callback: impl FnOnce(&mut H, &NodeHandle, &H::Npc) -> Result<Option<H::Component>>,
    ) -> Result<()> {
        self.assign(host, unclosable, |host| {
            let player = self.player_ref()?;
            let npc = self.file.npc().ok_or(Error::Null)?;
            callback(host, &player, &npc)
        })
    }
    /// `player(chatAnim, vararg messages, unclosable)`.
    pub fn player_lines(
        &self,
        host: &mut H,
        anim: ChatAnim,
        messages: &[Text],
        unclosable: bool,
    ) -> Result<()> {
        self.assign(host, unclosable, |host| {
            let interpreter = self.interpreter()?;
            let player = self.file.player().map(Entity::Player);
            let lines = Self::format(messages, &[])?;
            interpreter.send_dialogues_for(host, player.as_ref(), anim.animation_id(), &lines)
        })
    }
    /// `player(vararg messages)`.
    pub fn player(&self, host: &mut H, messages: &[Text]) -> Result<()> {
        self.player_lines(host, ChatAnim::Neutral, messages, false)
    }
    /// `npc(chatAnim, vararg messages, unclosable)`.
    pub fn npc_lines(
        &self,
        host: &mut H,
        anim: ChatAnim,
        messages: &[Text],
        unclosable: bool,
    ) -> Result<()> {
        self.assign(host, unclosable, |host| {
            let interpreter = self.interpreter()?;
            let npc = self.file.npc().map(Entity::Npc);
            let lines = Self::format(messages, &[])?;
            interpreter.send_dialogues_for(host, npc.as_ref(), anim.animation_id(), &lines)
        })
    }
    /// `npc(vararg messages)`.
    pub fn npc(&self, host: &mut H, messages: &[Text]) -> Result<()> {
        self.npc_lines(host, ChatAnim::Neutral, messages, false)
    }
    /// `npc(chatAnim, npcId, vararg messages, unclosable)`: a new NPC of that id
    /// speaks. The Kotlin default `npc!!.id` is the caller's to evaluate.
    pub fn npc_as(
        &self,
        host: &mut H,
        anim: ChatAnim,
        npc_id: i32,
        messages: &[Text],
        unclosable: bool,
    ) -> Result<()> {
        self.assign(host, unclosable, |host| {
            let interpreter = self.interpreter()?;
            let npc = Entity::Npc(host.new_npc(npc_id)?);
            let lines = Self::format(messages, &[])?;
            interpreter.send_dialogues_for(host, Some(&npc), anim.animation_id(), &lines)
        })
    }
    /// `npc(npcId, vararg messages, unclosable)`.
    pub fn npc_id(
        &self,
        host: &mut H,
        npc_id: i32,
        messages: &[Text],
        unclosable: bool,
    ) -> Result<()> {
        self.npc_as(host, ChatAnim::Neutral, npc_id, messages, unclosable)
    }
    /// `item(item, vararg messages, message, unclosable)`.
    pub fn item(
        &self,
        host: &mut H,
        item: &H::Item,
        messages: &[Text],
        message: &[u16],
        unclosable: bool,
    ) -> Result<()> {
        self.assign(host, unclosable, |host| {
            let interpreter = self.interpreter()?;
            let lines = Self::format(messages, message)?;
            interpreter.send_item_message(host, item, &lines)
        })
    }
    /// `item(item, item2, vararg messages, message, unclosable)`.
    pub fn item2(
        &self,
        host: &mut H,
        item: &H::Item,
        item2: &H::Item,
        messages: &[Text],
        message: &[u16],
        unclosable: bool,
    ) -> Result<()> {
        self.assign(host, unclosable, |host| {
            let interpreter = self.interpreter()?;
            let lines = Self::format(messages, message)?;
            interpreter.send_double_item_message(host, item, item2, &lines)
        })
    }
    /// `line(vararg messages, unclosable)`: the messages are not split.
    pub fn line(&self, host: &mut H, messages: &[Text], unclosable: bool) -> Result<()> {
        self.assign(host, unclosable, |host| {
            let interpreter = self.interpreter()?;
            interpreter.send_dialogue(host, Some(&lines(messages)))
        })
    }
    /// `options(vararg options, title, unclosable)`: the option list, the
    /// spoken choice, then the jump to the chosen label.
    pub fn options(
        &self,
        host: &mut H,
        options: &[Arc<DialogueOption<H>>],
        title: &str,
        unclosable: bool,
    ) -> Result<()> {
        self.assign(host, unclosable, |host| {
            let mut filtered = Vec::new();
            for option in options {
                let keep = match &option.option_if {
                    Some(condition) => {
                        let player = self.player_ref()?;
                        let npc = self.file.npc().ok_or(Error::Null)?;
                        condition(host, &player, &npc)?
                    }
                    None => true,
                };
                if keep {
                    filtered.push(option.clone());
                }
            }
            let mut s = self.st();
            s.filtered_options = filtered;
            if s.filtered_options.len() == 1 {
                s.jump_to = s.label_stage_map.get(&s.filtered_options[0].goto).copied();
                return Ok(None);
            }
            let names = Lines::new(
                s.filtered_options
                    .iter()
                    .map(|o| Some(o.option.clone()))
                    .collect(),
            );
            drop(s);
            let interpreter = self.interpreter()?;
            interpreter.send_options(host, Some(text(title)), &names)
        })?;
        let chosen = {
            let s = self.st();
            match s.button_id {
                Some(b) if b >= 1 && b as usize <= s.filtered_options.len() => {
                    Some(s.filtered_options[b as usize - 1].clone())
                }
                _ => None,
            }
        };
        self.assign(host, unclosable, |host| {
            let mut component = None;
            if chosen.as_ref().is_some_and(|o| o.skip_player) {
                let mut s = self.st();
                s.jump_to = Some(s.stage.wrapping_add(1));
            } else {
                let interpreter = self.interpreter()?;
                let player = self.file.player().map(Entity::Player);
                let anim = chosen.as_ref().map_or(ChatAnim::Neutral, |o| o.expression);
                let spoken = chosen
                    .as_ref()
                    .map_or_else(|| text(" "), |o| o.spoken_text.clone());
                let lines = split(&spoken)?;
                component = interpreter.send_dialogues_for(
                    host,
                    player.as_ref(),
                    anim.animation_id(),
                    &lines,
                )?;
            }
            let mut s = self.st();
            s.opt_button = s.button_id;
            Ok(component)
        })?;
        let mut s = self.st();
        if s.stage == s.dialogue_counter {
            if let Some(b) = s.opt_button {
                if b >= 1 && b as usize <= s.filtered_options.len() {
                    let goto = s.filtered_options[b as usize - 1].goto.clone();
                    s.jump_to = s.label_stage_map.get(&goto).copied();
                }
            }
        }
        s.dialogue_counter = s.dialogue_counter.wrapping_add(1);
        Ok(())
    }
    /// `input(type, prompt)`: the prompt, `parseamount` for AMOUNT, then a
    /// runscript that stores `optInput` and continues through the
    /// interpreter with the chatbox id and button 2.
    pub fn input(&self, host: &mut H, kind: InputType, prompt: &str) -> Result<()> {
        self.assign(host, false, |host| {
            let prompt = text(prompt);
            let objects = vec![ScriptValue::Text(prompt.clone())];
            match kind {
                InputType::Amount | InputType::Short => {
                    self.interpreter()?.send_input(host, true, objects)?;
                }
                InputType::Numeric => self.interpreter()?.send_input(host, false, objects)?,
                InputType::Long => self.interpreter()?.send_long_input(host, objects)?,
                InputType::Message => self.interpreter()?.send_message_input(host, Some(prompt))?,
            }
            if kind == InputType::Amount {
                host.set_value(&self.player_ref()?, "parseamount", Value::Bool(true))?;
            }
            let file = self.file.clone();
            let script = RunScript::new(move |host: &mut H, value: Response| {
                let labeller = file.body::<Labeller<H>>().ok_or(Error::Cast)?;
                labeller.st().opt_input = Some(value);
                let interpreter = file.interpreter().ok_or(Error::Null)?;
                let player = file.player().ok_or(Error::Null)?;
                let manager = host.manager(&player)?;
                let chatbox = host.chatbox(manager)?.ok_or(Error::Null)?;
                let id = host.component_id(&chatbox)?;
                interpreter.handle(host, id, 2)
            });
            host.set_value(&self.player_ref()?, "runscript", Value::Script(script))?;
            host.set_value(&self.player_ref()?, "input-type", Value::InputType(kind))?;
            Ok(None)
        })
    }
    /// `input(numeric, prompt)`.
    pub fn input_numeric(&self, host: &mut H, numeric: bool, prompt: &str) -> Result<()> {
        let kind = if numeric {
            InputType::Numeric
        } else {
            InputType::Short
        };
        self.input(host, kind, prompt)
    }
    /// `afterClose { player -> }`: hooked once, right after the stage that
    /// was hit in this pass, unless a jump is pending.
    pub fn after_close(
        &self,
        host: &mut H,
        callback: impl Fn(&mut H, &NodeHandle) -> Result<()> + Send + Sync + 'static,
    ) -> Result<()> {
        {
            let s = self.st();
            if !s.stage_hit || self.file.base_stage() != s.dialogue_counter || s.jump_to.is_some() {
                return Ok(());
            }
        }
        let hook = super::CloseHook::new(callback);
        let player = self.player_ref()?;
        host.hook_dialogue_closed(&player, hook)
    }
    /// `open(player, dialogue, vararg args)`: a stage that opens another
    /// dialogue through `openDialogue`.
    pub fn open(
        &self,
        host: &mut H,
        player: &NodeHandle,
        dialogue: &Dialogue<H>,
        args: &[Arg<H>],
    ) -> Result<()> {
        self.assign(host, false, |host| {
            api::open_dialogue(host, player, dialogue, args)?;
            Ok(None)
        })
    }
    /// `openDialogue(player, dialogue, vararg args)`: immediately.
    pub fn open_dialogue(
        &self,
        host: &mut H,
        player: &NodeHandle,
        dialogue: &Dialogue<H>,
        args: &[Arg<H>],
    ) -> Result<()> {
        api::open_dialogue(host, player, dialogue, args)
    }
}
