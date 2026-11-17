//! DialogueFile base behaviour. Content supplies a [`FileBody`]; the stage
//! property is virtual, as the Kotlin `open var stage` is, so a body can keep
//! its own stage the way DialogueLabeller does.
use super::{
    Entity, Error, FacialExpression, Host, Interpreter, Lines, NodeHandle, Plugin, Result,
    START_DIALOGUE, Target, Text, Topic, read, text, write,
};
use crate::world::plain_dialogue as plain;
use std::{
    any::Any,
    sync::{Arc, RwLock},
};

/// The abstract and overridden members of DialogueFile used by the interpreter
/// and the base helpers. The message helpers themselves are not virtual here.
pub trait FileBody<H: Host>: Send + Sync + 'static {
    fn class_name(&self) -> &str;
    fn handle(&self, host: &mut H, this: &File<H>, component_id: i32, button_id: i32)
    -> Result<()>;
    /// `getStage()`.
    fn stage(&self, _host: &mut H, this: &File<H>) -> Result<i32> {
        Ok(this.base_stage())
    }
    /// `setStage(value)`.
    fn set_stage(&self, _host: &mut H, this: &File<H>, value: i32) -> Result<()> {
        this.set_base_stage(value);
        Ok(())
    }
    /// `getCurrentStage()`; the base body reads the virtual stage.
    fn current_stage(&self, host: &mut H, this: &File<H>) -> Result<i32> {
        self.stage(host, this)
    }
    /// `end()`.
    fn end(&self, host: &mut H, this: &File<H>) -> Result<()> {
        this.base_end(host)
    }
    fn as_any(&self) -> &dyn Any;
}

struct State<H: Host> {
    player: Option<NodeHandle>,
    npc: Option<H::Npc>,
    interpreter: Option<Interpreter<H>>,
    stage: i32,
    dialogue_plugin: Option<Plugin<H>>,
}
struct Object<H: Host> {
    state: RwLock<State<H>>,
    body: Box<dyn FileBody<H>>,
}
/// DialogueFile handle; clones are the same object.
pub struct File<H: Host>(Arc<Object<H>>);
impl<H: Host> Clone for File<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// `splitLines(msg!!)` followed by the Kotlin spread into a new array.
fn split(message: Option<&[u16]>) -> Result<Lines> {
    let message = message.ok_or(Error::Null)?;
    let lines = plain::split_lines(message, 54).map_err(|e| Error::Binding(e.to_string()))?;
    Ok(Lines::new(lines.into_iter().map(Some).collect()))
}
/// Kotlin's `*messages` spread: the callee receives a copy of the array, so
/// later changes to the caller's array are not sent.
fn spread(lines: &Lines) -> Lines {
    Lines::new(lines.read().clone())
}

impl<H: Host> File<H> {
    pub fn new(body: impl FileBody<H>) -> Self {
        Self(Arc::new(Object {
            state: RwLock::new(State {
                player: None,
                npc: None,
                interpreter: None,
                stage: START_DIALOGUE,
                dialogue_plugin: None,
            }),
            body: Box::new(body),
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn body<T: 'static>(&self) -> Option<&T> {
        self.0.body.as_any().downcast_ref()
    }
    pub fn class_name(&self) -> &str {
        self.0.body.class_name()
    }
    pub fn player(&self) -> Option<NodeHandle> {
        read(&self.0.state).player.clone()
    }
    pub fn npc(&self) -> Option<H::Npc> {
        read(&self.0.state).npc.clone()
    }
    pub fn interpreter(&self) -> Option<Interpreter<H>> {
        read(&self.0.state).interpreter.clone()
    }
    pub fn dialogue_plugin(&self) -> Option<Plugin<H>> {
        read(&self.0.state).dialogue_plugin.clone()
    }
    pub fn set_dialogue_plugin(&self, plugin: Option<Plugin<H>>) {
        let old = std::mem::replace(&mut write(&self.0.state).dialogue_plugin, plugin);
        drop(old);
    }
    /// The DialogueFile backing field, which an overriding body can bypass.
    pub fn base_stage(&self) -> i32 {
        read(&self.0.state).stage
    }
    pub fn set_base_stage(&self, stage: i32) {
        write(&self.0.state).stage = stage;
    }
    fn interp(&self) -> Result<Interpreter<H>> {
        self.interpreter().ok_or(Error::Null)
    }

    // Virtual dispatch.
    pub fn handle(&self, host: &mut H, component_id: i32, button_id: i32) -> Result<()> {
        self.0.body.handle(host, self, component_id, button_id)
    }
    pub fn stage(&self, host: &mut H) -> Result<i32> {
        self.0.body.stage(host, self)
    }
    pub fn set_stage(&self, host: &mut H, value: i32) -> Result<()> {
        self.0.body.set_stage(host, self, value)
    }
    pub fn current_stage(&self, host: &mut H) -> Result<i32> {
        self.0.body.current_stage(host, self)
    }
    pub fn end(&self, host: &mut H) -> Result<()> {
        self.0.body.end(host, self)
    }
    /// DialogueFile.end: the base body.
    pub fn base_end(&self, host: &mut H) -> Result<()> {
        if let Some(interpreter) = self.interpreter() {
            interpreter.close(host)?;
        }
        Ok(())
    }

    /// `load(player, npc, interpreter)`: player and interpreter are non-null
    /// Kotlin parameters, checked before any field is written.
    pub fn load(
        &self,
        player: Option<NodeHandle>,
        npc: Option<H::Npc>,
        interpreter: Option<Interpreter<H>>,
    ) -> Result<Self> {
        let player = player.ok_or(Error::Null)?;
        let interpreter = interpreter.ok_or(Error::Null)?;
        let old = {
            let mut state = write(&self.0.state);
            (
                state.player.replace(player),
                std::mem::replace(&mut state.npc, npc),
                state.interpreter.replace(interpreter.clone()),
            )
        };
        drop(old);
        let topics = interpreter.active_topics();
        let old = std::mem::take(&mut *topics.write());
        drop(old);
        Ok(self.clone())
    }

    /// `npc(vararg messages)`: silent without an NPC.
    pub fn npc_lines(&self, host: &mut H, lines: &Lines) -> Result<Option<H::Component>> {
        let Some(npc) = self.npc() else {
            return Ok(None);
        };
        let interpreter = self.interp()?;
        let expression = if host.npc_id(&npc)? > 8591 {
            FacialExpression::OldNormal
        } else {
            FacialExpression::Friendly
        };
        interpreter.send_dialogues_for(
            host,
            Some(&Entity::Npc(npc)),
            expression.animation_id(),
            &spread(lines),
        )
    }
    /// `npc(id, vararg messages)`.
    pub fn npc_id(&self, host: &mut H, id: i32, lines: &Lines) -> Result<Option<H::Component>> {
        self.npc_id_expr(host, id, Some(FacialExpression::Friendly), lines)
    }
    /// `npc(expression, vararg messages)`: without an NPC the head is NPC id 0.
    pub fn npc_expr(
        &self,
        host: &mut H,
        expression: Option<FacialExpression>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let expression = FacialExpression::or_unset(expression);
        match self.npc() {
            None => self
                .interp()?
                .send_dialogues(host, 0, expression, false, &spread(lines)),
            Some(npc) => self.interp()?.send_dialogues_for(
                host,
                Some(&Entity::Npc(npc)),
                expression,
                &spread(lines),
            ),
        }
    }
    /// `npc(id, expression, vararg messages)`.
    pub fn npc_id_expr(
        &self,
        host: &mut H,
        id: i32,
        expression: Option<FacialExpression>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        self.interp()?.send_dialogues(
            host,
            id,
            FacialExpression::or_unset(expression),
            false,
            &spread(lines),
        )
    }
    /// `npc(id, title, expression, vararg messages)`: replaces the name line
    /// after the dialogue is sent, and fails when nothing was sent.
    pub fn npc_titled(
        &self,
        host: &mut H,
        id: i32,
        title: &[u16],
        expression: Option<FacialExpression>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let component = self.npc_id_expr(host, id, expression, lines)?;
        let player = self.player().ok_or(Error::Null)?;
        let dispatch = host.dispatch(&player)?;
        let id = host.component_id(component.as_ref().ok_or(Error::Null)?)?;
        host.send_string(dispatch, Some(title.to_vec()), id, 3)?;
        Ok(component)
    }
    /// `player(vararg messages)`.
    pub fn player_lines(&self, host: &mut H, lines: &Lines) -> Result<Option<H::Component>> {
        self.player_expr(host, None, lines)
    }
    /// `player(expression, vararg messages)`.
    pub fn player_expr(
        &self,
        host: &mut H,
        expression: Option<FacialExpression>,
        lines: &Lines,
    ) -> Result<Option<H::Component>> {
        let interpreter = self.interp()?;
        let entity = self.player().map(Entity::Player);
        interpreter.send_dialogues_for(
            host,
            entity.as_ref(),
            FacialExpression::or_unset(expression),
            &spread(lines),
        )
    }
    /// `npcl(expr, msg)`.
    pub fn npcl(
        &self,
        host: &mut H,
        expression: Option<FacialExpression>,
        message: Option<&[u16]>,
    ) -> Result<Option<H::Component>> {
        self.npc_expr(host, expression, &split(message)?)
    }
    /// `npcl(id, expr, msg)`.
    pub fn npcl_id(
        &self,
        host: &mut H,
        id: i32,
        expression: Option<FacialExpression>,
        message: Option<&[u16]>,
    ) -> Result<Option<H::Component>> {
        self.npc_id_expr(host, id, expression, &split(message)?)
    }
    /// `npcl(id, title, expr, msg)`.
    pub fn npcl_titled(
        &self,
        host: &mut H,
        id: i32,
        title: &[u16],
        expression: Option<FacialExpression>,
        message: Option<&[u16]>,
    ) -> Result<Option<H::Component>> {
        self.npc_titled(host, id, title, expression, &split(message)?)
    }
    /// `npcl(msg)`.
    pub fn npcl_plain(
        &self,
        host: &mut H,
        message: Option<&[u16]>,
    ) -> Result<Option<H::Component>> {
        self.npc_lines(host, &split(message)?)
    }
    /// `playerl(expr, msg)`.
    pub fn playerl(
        &self,
        host: &mut H,
        expression: Option<FacialExpression>,
        message: Option<&[u16]>,
    ) -> Result<Option<H::Component>> {
        self.player_expr(host, expression, &split(message)?)
    }
    /// `playerl(msg)`.
    pub fn playerl_plain(
        &self,
        host: &mut H,
        message: Option<&[u16]>,
    ) -> Result<Option<H::Component>> {
        self.player_lines(host, &split(message)?)
    }
    /// `sendNormalDialogue(entity, expression, vararg messages)`.
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
            &spread(lines),
        )?;
        Ok(())
    }
    /// `options(vararg options, title)`; the Kotlin default title is
    /// "Select an Option".
    pub fn options(&self, host: &mut H, lines: &Lines, title: Option<&[u16]>) -> Result<()> {
        let title = title.map_or_else(|| text("Select an Option"), <[u16]>::to_vec);
        self.interp()?
            .send_options(host, Some(title), &spread(lines))?;
        Ok(())
    }
    /// `endFile()`: hands handling back to the interpreter's current plugin.
    pub fn end_file(&self) -> Result<()> {
        self.interp()?.dialogue().ok_or(Error::Null)?.set_file(None);
        Ok(())
    }
    /// `returnAtStage(toStage)`.
    pub fn return_at_stage(&self, to_stage: i32) -> Result<()> {
        self.dialogue_plugin().ok_or(Error::Null)?.set_file(None);
        self.dialogue_plugin()
            .ok_or(Error::Null)?
            .set_stage(to_stage);
        Ok(())
    }
    /// `abandonFile()`.
    pub fn abandon_file(&self, host: &mut H) -> Result<()> {
        self.end_file()?;
        self.player_lines(host, &super::line(&text("Huh. Nevermind.")))?;
        Ok(())
    }
    /// `dialogue(vararg messages)`: a plain message through the player's
    /// current interpreter; silent when the player or that interpreter is null.
    pub fn dialogue(&self, host: &mut H, lines: &Lines) -> Result<()> {
        let Some(player) = self.player() else {
            return Ok(());
        };
        let Some(interpreter) = host.interpreter(&player)? else {
            return Ok(());
        };
        interpreter.send_dialogue(host, Some(&spread(lines)))?;
        Ok(())
    }
    /// `showTopics(vararg topics, title)`. With one valid topic the reference
    /// follows the first topic of the whole argument list whose text equals
    /// it, which can be an IfTopic that was filtered out.
    pub fn show_topics(
        &self,
        host: &mut H,
        topics: &[Topic<H>],
        title: Option<&[u16]>,
    ) -> Result<bool> {
        let mut valid: Vec<Text> = Vec::new();
        for topic in topics.iter().filter(|t| t.show_condition != Some(false)) {
            self.interp()?.active_topics().write().push(topic.clone());
            valid.push(topic.text.clone());
        }
        if valid.is_empty() {
            return Ok(true);
        }
        if valid.len() == 1 {
            let topic = topics
                .iter()
                .find(|t| t.text == valid[0])
                .cloned()
                .ok_or(Error::Index)?;
            match &topic.to_stage {
                Target::File(file) => {
                    self.interp()?
                        .dialogue()
                        .ok_or(Error::Null)?
                        .load_file(Some(file))?;
                }
                Target::Stage(stage) => self.set_stage(host, *stage)?,
                Target::Other => {}
            }
            self.player_lines(host, &super::line(&topic.text))?;
            let active = self.interp()?.active_topics();
            let old = std::mem::take(&mut *active.write());
            drop(old);
            return Ok(false);
        }
        let options = Lines::new(valid.into_iter().map(Some).collect());
        self.options(host, &options, title)?;
        Ok(false)
    }
}
