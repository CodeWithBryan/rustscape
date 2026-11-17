//! DialoguePlugin base behaviour, EmptyPlugin and SimpleDialoguePlugin.
//! Content supplies a [`PluginBody`]; the inherited fields and helpers live on
//! the [`Plugin`] handle so original-style stage code can be written natively.
use super::{
    Arg, DIALOGUE_INITIAL_OPTIONS_HANDLE, Entity, Error, FacialExpression, File, Host, Interpreter,
    Lines, LogLevel, NodeHandle, Registry, Result, SIMPLE_DIALOGUE, START_DIALOGUE, Target, Text,
    Topic, read, text, write,
};
use crate::world::plain_dialogue as plain;
use std::{
    any::Any,
    sync::{Arc, RwLock},
};

/// The abstract and overridable methods of DialoguePlugin. `open` and `close`
/// default to the base class bodies.
pub trait PluginBody<H: Host>: Send + Sync + 'static {
    /// `getClass().getSimpleName()`: debug, log and duplicate-key text only.
    fn class_name(&self) -> &str;
    fn ids(&self, host: &mut H, this: &Plugin<H>) -> Result<Vec<i32>>;
    /// `newInstance(player)`. `Ok(None)` is the null the reflective base
    /// method returns after printing a failed construction.
    fn new_instance(
        &self,
        host: &mut H,
        this: &Plugin<H>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Plugin<H>>>;
    fn handle(
        &self,
        host: &mut H,
        this: &Plugin<H>,
        interface_id: i32,
        button_id: i32,
    ) -> Result<bool>;
    fn open(&self, host: &mut H, this: &Plugin<H>, args: &[Arg<H>]) -> Result<bool> {
        this.base_open(host, args)
    }
    fn close(&self, host: &mut H, this: &Plugin<H>) -> Result<bool> {
        this.base_close(host)
    }
    fn as_any(&self) -> &dyn Any;
}

struct State<H: Host> {
    player: Option<NodeHandle>,
    interpreter: Option<Interpreter<H>>,
    file: Option<File<H>>,
    option_names: Vec<Text>,
    option_files: Vec<Option<File<H>>>,
    npc: Option<H::Npc>,
    stage: i32,
    finished: bool,
}
struct Object<H: Host> {
    state: RwLock<State<H>>,
    body: Box<dyn PluginBody<H>>,
}
/// DialoguePlugin handle; clones are the same object.
pub struct Plugin<H: Host>(Arc<Object<H>>);
impl<H: Host> Clone for Plugin<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

fn split(message: Option<&[u16]>) -> Result<Lines> {
    let message = message.ok_or(Error::Null)?;
    let lines = plain::split_lines(message, 54).map_err(|e| Error::Binding(e.to_string()))?;
    Ok(Lines::new(lines.into_iter().map(Some).collect()))
}

impl<H: Host> Plugin<H> {
    fn build(
        player: Option<NodeHandle>,
        interpreter: Option<Interpreter<H>>,
        body: impl PluginBody<H>,
    ) -> Self {
        Self(Arc::new(Object {
            state: RwLock::new(State {
                player,
                interpreter,
                file: None,
                option_names: Vec::new(),
                option_files: Vec::new(),
                npc: None,
                stage: 0,
                finished: false,
            }),
            body: Box::new(body),
        }))
    }
    /// The no-argument constructor used for registered prototypes.
    pub fn prototype(body: impl PluginBody<H>) -> Self {
        Self::build(None, None, body)
    }
    /// `DialoguePlugin(Player)`: captures the player's interpreter at
    /// construction when the player is not null.
    pub fn new(host: &mut H, player: Option<NodeHandle>, body: impl PluginBody<H>) -> Result<Self> {
        let interpreter = match &player {
            Some(player) => host.interpreter(player)?,
            None => None,
        };
        Ok(Self::build(player, interpreter, body))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    /// The content payload, for content's own typed state.
    pub fn body<T: 'static>(&self) -> Option<&T> {
        self.0.body.as_any().downcast_ref()
    }
    pub fn class_name(&self) -> &str {
        self.0.body.class_name()
    }

    pub fn player(&self) -> Option<NodeHandle> {
        read(&self.0.state).player.clone()
    }
    pub fn interpreter(&self) -> Option<Interpreter<H>> {
        read(&self.0.state).interpreter.clone()
    }
    pub fn file(&self) -> Option<File<H>> {
        read(&self.0.state).file.clone()
    }
    pub fn set_file(&self, file: Option<File<H>>) {
        let old = std::mem::replace(&mut write(&self.0.state).file, file);
        drop(old);
    }
    pub fn npc(&self) -> Option<H::Npc> {
        read(&self.0.state).npc.clone()
    }
    pub fn set_npc(&self, npc: Option<H::Npc>) {
        write(&self.0.state).npc = npc;
    }
    pub fn stage(&self) -> i32 {
        read(&self.0.state).stage
    }
    pub fn set_stage(&self, stage: i32) {
        write(&self.0.state).stage = stage;
    }
    pub fn is_finished(&self) -> bool {
        read(&self.0.state).finished
    }
    pub fn set_finished(&self, finished: bool) {
        write(&self.0.state).finished = finished;
    }
    pub fn option_names(&self) -> Vec<Text> {
        read(&self.0.state).option_names.clone()
    }
    pub fn option_files(&self) -> Vec<Option<File<H>>> {
        read(&self.0.state).option_files.clone()
    }
    fn interp(&self) -> Result<Interpreter<H>> {
        self.interpreter().ok_or(Error::Null)
    }

    // Virtual dispatch.
    pub fn ids(&self, host: &mut H) -> Result<Vec<i32>> {
        self.0.body.ids(host, self)
    }
    pub fn new_instance(&self, host: &mut H, player: Option<NodeHandle>) -> Result<Option<Self>> {
        self.0.body.new_instance(host, self, player)
    }
    pub fn handle(&self, host: &mut H, interface_id: i32, button_id: i32) -> Result<bool> {
        self.0.body.handle(host, self, interface_id, button_id)
    }
    pub fn open(&self, host: &mut H, args: &[Arg<H>]) -> Result<bool> {
        self.0.body.open(host, self, args)
    }
    pub fn close(&self, host: &mut H) -> Result<bool> {
        self.0.body.close(host, self)
    }

    /// `init()`: registers this object under each of its ids, stopping at the
    /// first id already in use.
    pub fn init(&self, host: &mut H, registry: &Registry<H>) -> Result<()> {
        for id in self.ids(host)? {
            registry.add(id, Some(self))?;
        }
        Ok(())
    }
    /// DialoguePlugin.open: the base body.
    pub fn base_open(&self, host: &mut H, args: &[Arg<H>]) -> Result<bool> {
        let player = self.player().ok_or(Error::Null)?;
        let topics = host
            .interpreter(&player)?
            .ok_or(Error::Null)?
            .active_topics();
        let old = std::mem::take(&mut *topics.write());
        drop(old);
        if let Some(Arg::Npc(npc)) = args.first() {
            self.set_npc(Some(npc.clone()));
        }
        if self.npc().is_none() {
            let name = args
                .first()
                .ok_or(Error::Index)?
                .simple_name()
                .ok_or(Error::Null)?;
            let message = format!("{name}Is not assigning an NPC. Whoever did that should fix it.");
            host.log(self.class_name(), LogLevel::Warn, text(&message))?;
        }
        host.interpreter(&player)?
            .ok_or(Error::Null)?
            .handle(host, 0, 0)?;
        Ok(true)
    }
    /// DialoguePlugin.close: closes, but does not end, the dialogue.
    pub fn base_close(&self, host: &mut H) -> Result<bool> {
        let player = self.player().ok_or(Error::Null)?;
        let manager = host.manager(&player)?;
        host.close_chatbox(manager)?;
        let manager = host.manager(&player)?;
        host.open_chatbox(manager, 137)?;
        if let Some(file) = self.file() {
            file.end(host)?;
        }
        self.set_finished(true);
        Ok(true)
    }
    /// `end()`: closes through the captured interpreter, if there is one.
    pub fn end(&self, host: &mut H) -> Result<()> {
        if let Some(interpreter) = self.interpreter() {
            interpreter.close(host)?;
        }
        Ok(())
    }
    pub fn finish(&self) {
        self.set_stage(-1);
    }
    /// `next()`.
    pub fn next(&self) {
        let mut state = write(&self.0.state);
        state.stage = state.stage.wrapping_add(1);
    }
    /// `increment()`.
    pub fn increment(&self) {
        self.next();
    }
    /// `getAndIncrement()`: returns the stage before the increment.
    pub fn get_and_increment(&self) -> i32 {
        let mut state = write(&self.0.state);
        let old = state.stage;
        state.stage = old.wrapping_add(1);
        old
    }
    /// `pirateGender()`.
    pub fn pirate_gender(&self, host: &mut H) -> Result<Text> {
        let player = self.player().ok_or(Error::Null)?;
        Ok(text(if host.is_male(&player)? {
            "lad"
        } else {
            "lass"
        }))
    }
    fn own_entity(&self) -> Option<Entity<H>> {
        self.player().map(Entity::Player)
    }

    /// `sendNormalDialogue(entity, expression, messages)`.
    pub fn send_normal_dialogue(
        &self,
        host: &mut H,
        entity: Option<&Entity<H>>,
        expression: Option<FacialExpression>,
        lines: &Lines,
    ) -> Result<()> {
        self.interp()?.send_dialogues_for(
            host,
            entity,
            FacialExpression::or_unset(expression),
            lines,
        )?;
        Ok(())
    }
    /// `npc(String... messages)`: FRIENDLY, or OLD_NORMAL above id 8591. With
    /// no NPC the first registered id speaks, and `getIds()` is read twice.
    pub fn npc_lines(&self, host: &mut H, lines: &Lines) -> Result<Option<H::Component>> {
        let pick = |id: i32| {
            if id > 8591 {
                FacialExpression::OldNormal
            } else {
                FacialExpression::Friendly
            }
        };
        match self.npc() {
            None => {
                let id = *self.ids(host)?.first().ok_or(Error::Index)?;
                let expression = pick(*self.ids(host)?.first().ok_or(Error::Index)?);
                self.interp()?
                    .send_dialogues(host, id, expression.animation_id(), false, lines)
            }
            Some(npc) => {
                let expression = pick(host.npc_id(&npc)?);
                self.interp()?.send_dialogues_for(
                    host,
                    Some(&Entity::Npc(npc)),
                    expression.animation_id(),
                    lines,
                )
            }
        }
    }
    /// `npc(int id, String... messages)`.
    pub fn npc_id(&self, host: &mut H, id: i32, lines: &Lines) -> Result<Option<H::Component>> {
        let expression = FacialExpression::Friendly.animation_id();
        self.interp()?
            .send_dialogues(host, id, expression, false, lines)
    }
    /// `npc(FacialExpression expression, String... messages)`.
    pub fn npc_expr(
        &self,
        host: &mut H,
        expression: Option<FacialExpression>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let expression = FacialExpression::or_unset(expression);
        match self.npc() {
            None => {
                let id = *self.ids(host)?.first().ok_or(Error::Index)?;
                self.interp()?
                    .send_dialogues(host, id, expression, false, lines)
            }
            Some(npc) => {
                self.interp()?
                    .send_dialogues_for(host, Some(&Entity::Npc(npc)), expression, lines)
            }
        }
    }
    /// `player(String... messages)`.
    pub fn player_lines(&self, host: &mut H, lines: &Lines) -> Result<Option<H::Component>> {
        self.player_expr(host, None, lines)
    }
    /// `player(FacialExpression expression, String... messages)`.
    pub fn player_expr(
        &self,
        host: &mut H,
        expression: Option<FacialExpression>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let entity = self.own_entity();
        self.interp()?.send_dialogues_for(
            host,
            entity.as_ref(),
            FacialExpression::or_unset(expression),
            lines,
        )
    }
    /// `options(String... options)` under the title "Select an Option".
    pub fn options(&self, host: &mut H, lines: &Lines) -> Result<Option<H::Component>> {
        self.interp()?
            .send_options(host, Some(text("Select an Option")), lines)
    }
    /// `sendDialogue(String... messages)`.
    pub fn send_dialogue(
        &self,
        host: &mut H,
        lines: Option<&Lines>,
    ) -> Result<Option<H::Component>> {
        self.interp()?.send_dialogue(host, lines)
    }
    /// `npcl(expr, msg)`: 54-unit automatic line splitting.
    pub fn npcl(
        &self,
        host: &mut H,
        expression: Option<FacialExpression>,
        message: Option<&[u16]>,
    ) -> Result<Option<H::Component>> {
        let lines = split(message)?;
        self.npc_expr(host, expression, &lines)
    }
    /// `playerl(expr, msg)`.
    pub fn playerl(
        &self,
        host: &mut H,
        expression: Option<FacialExpression>,
        message: Option<&[u16]>,
    ) -> Result<Option<H::Component>> {
        let lines = split(message)?;
        self.player_expr(host, expression, &lines)
    }
    /// `loadFile(file)`: diverts handling to the file and resets this
    /// plugin's stage. A null file is ignored.
    pub fn load_file(&self, file: Option<&File<H>>) -> Result<()> {
        let Some(file) = file else {
            return Ok(());
        };
        let loaded = file.load(self.player(), self.npc(), self.interpreter())?;
        self.set_file(Some(loaded));
        self.file()
            .ok_or(Error::Null)?
            .set_dialogue_plugin(Some(self.clone()));
        self.set_stage(START_DIALOGUE);
        Ok(())
    }
    /// `addOption(name, file)`.
    pub fn add_option(&self, name: Option<&[u16]>, file: Option<File<H>>) {
        let mut label = text("Talk about ");
        match name {
            Some(name) => label.extend_from_slice(name),
            None => label.extend(text("null")),
        }
        let mut state = write(&self.0.state);
        state.option_names.push(label);
        state.option_files.push(file);
    }
    /// `sendChoices()`: true only when an options interface was sent.
    pub fn send_choices(&self, host: &mut H) -> Result<bool> {
        let names = self.option_names();
        if names.len() == 1 {
            let file = self.option_files().first().cloned().ok_or(Error::Index)?;
            self.load_file(file.as_ref())?;
            Ok(false)
        } else if names.is_empty() {
            self.set_stage(START_DIALOGUE);
            Ok(false)
        } else {
            self.options(host, &Lines::new(names.into_iter().map(Some).collect()))?;
            self.set_stage(DIALOGUE_INITIAL_OPTIONS_HANDLE);
            Ok(true)
        }
    }
    /// `showTopics(topics)`. With one valid topic the reference follows the
    /// first entry of the interpreter's active list, which can be an older
    /// topic, and loads a file through the interpreter's current dialogue.
    pub fn show_topics(&self, host: &mut H, topics: &[Topic<H>]) -> Result<bool> {
        let mut valid = Vec::new();
        for topic in topics {
            if topic.show_condition == Some(false) {
                continue;
            }
            self.interp()?.active_topics().write().push(topic.clone());
            valid.push(topic.text.clone());
        }
        if valid.is_empty() {
            return Ok(true);
        }
        if valid.len() == 1 {
            let interpreter = self.interp()?;
            let topic = interpreter
                .active_topics()
                .read()
                .first()
                .cloned()
                .ok_or(Error::Index)?;
            match &topic.to_stage {
                Target::File(file) => {
                    interpreter
                        .dialogue()
                        .ok_or(Error::Null)?
                        .load_file(Some(file))?;
                }
                Target::Stage(stage) => self.set_stage(*stage),
                Target::Other => {}
            }
            self.player_lines(host, &super::line(&topic.text))?;
            let active = self.interp()?.active_topics();
            let old = std::mem::take(&mut *active.write());
            drop(old);
            return Ok(false);
        }
        self.options(host, &Lines::new(valid.into_iter().map(Some).collect()))?;
        Ok(false)
    }
}

/// EmptyPlugin: the carrier `open(DialogueFile, args)` wraps a file in.
pub struct EmptyPlugin<H: Host> {
    file: Option<File<H>>,
}
impl<H: Host> EmptyPlugin<H> {
    /// `EmptyPlugin(player, file)`.
    pub fn create(
        host: &mut H,
        player: Option<NodeHandle>,
        file: Option<File<H>>,
    ) -> Result<Plugin<H>> {
        Plugin::new(host, player, Self { file })
    }
}
impl<H: Host> PluginBody<H> for EmptyPlugin<H> {
    fn class_name(&self) -> &str {
        "EmptyPlugin"
    }
    fn ids(&self, _: &mut H, _: &Plugin<H>) -> Result<Vec<i32>> {
        Ok(vec![i32::MAX])
    }
    fn new_instance(
        &self,
        host: &mut H,
        _: &Plugin<H>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Plugin<H>>> {
        Self::create(host, player, None).map(Some)
    }
    fn handle(&self, _: &mut H, _: &Plugin<H>, _: i32, _: i32) -> Result<bool> {
        Ok(true)
    }
    fn open(&self, host: &mut H, this: &Plugin<H>, args: &[Arg<H>]) -> Result<bool> {
        if let Some(Arg::Npc(npc)) = args.first() {
            this.set_npc(Some(npc.clone()));
        }
        this.set_stage(START_DIALOGUE);
        this.load_file(self.file.as_ref())?;
        this.interpreter().ok_or(Error::Null)?.handle(host, 0, 0)?;
        Ok(true)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// SimpleDialoguePlugin: shows its String arguments as one plain message and
/// ends on the next click.
pub struct SimpleDialoguePlugin;
impl SimpleDialoguePlugin {
    pub fn prototype<H: Host>() -> Plugin<H> {
        Plugin::prototype(Self)
    }
}
impl<H: Host> PluginBody<H> for SimpleDialoguePlugin {
    fn class_name(&self) -> &str {
        "SimpleDialoguePlugin"
    }
    fn ids(&self, _: &mut H, _: &Plugin<H>) -> Result<Vec<i32>> {
        Ok(vec![SIMPLE_DIALOGUE])
    }
    fn new_instance(
        &self,
        host: &mut H,
        _: &Plugin<H>,
        player: Option<NodeHandle>,
    ) -> Result<Option<Plugin<H>>> {
        Plugin::new(host, player, Self).map(Some)
    }
    fn handle(&self, host: &mut H, this: &Plugin<H>, _: i32, _: i32) -> Result<bool> {
        this.end(host)?;
        Ok(true)
    }
    fn open(&self, host: &mut H, this: &Plugin<H>, args: &[Arg<H>]) -> Result<bool> {
        let mut messages = Vec::with_capacity(args.len());
        for arg in args {
            messages.push(match arg {
                Arg::Null => None,
                Arg::Text(value) => Some(value.clone()),
                _ => return Err(Error::Cast),
            });
        }
        this.send_dialogue(host, Some(&Lines::new(messages)))?;
        Ok(true)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
