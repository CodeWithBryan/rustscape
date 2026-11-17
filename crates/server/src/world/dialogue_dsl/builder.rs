//! DialogueBuilder, DialogueBuilderFile, DialogueClause, the node classes,
//! DialogueOptionsBuilder and DialogueBranchBuilder.
//!
//! Content builds the clauses once, when the file is constructed (the
//! original `init { create(DialogueBuilder(this)) }`); handling walks the
//! first clause whose predicate holds and runs the node at the stage. The
//! debug graph printer (`DEBUG_DIALOGUE` is false) and the nodes' `toString`
//! are not ported; [`structure`] is an observer.
use super::{Error, File, Host, NodeHandle, Result, Text, Value, lines, split, spread};
use crate::world::dialogue::{END_DIALOGUE, FacialExpression, FileBody, Lines, text};
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError, RwLock},
};

pub type Predicate<H> = Arc<dyn Fn(&mut H, &NodeHandle) -> Result<bool> + Send + Sync>;
/// `(DialogueFile, Player, componentID, buttonID) -> Unit`.
pub type StageFn<H> =
    Arc<dyn Fn(&mut H, &File<H>, &NodeHandle, i32, i32) -> Result<()> + Send + Sync>;
/// `(DialogueFile, Player, componentID, buttonID, stage) -> Int`.
pub type GotoFn<H> =
    Arc<dyn Fn(&mut H, &File<H>, &NodeHandle, i32, i32, i32) -> Result<i32> + Send + Sync>;
/// `(DialogueFile, Player) -> Unit`.
pub type EndFn<H> = Arc<dyn Fn(&mut H, &File<H>, &NodeHandle) -> Result<()> + Send + Sync>;
/// `(Player) -> Int`.
pub type BranchFn<H> = Arc<dyn Fn(&mut H, &NodeHandle) -> Result<i32> + Send + Sync>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `OptionEntry(text, nextStage, predicate = { true })`; `None` is the
/// default predicate.
pub struct OptionEntry<H: Host> {
    pub text: Text,
    pub next_stage: i32,
    pub predicate: Option<Predicate<H>>,
}
impl<H: Host> Clone for OptionEntry<H> {
    fn clone(&self) -> Self {
        Self {
            text: self.text.clone(),
            next_stage: self.next_stage,
            predicate: self.predicate.clone(),
        }
    }
}
/// `OptionsData(header, entries, attr)`, shared by an OptionsNode and its
/// OptionsDispatchNode.
pub struct OptionsData<H: Host> {
    header: Text,
    entries: Mutex<Vec<OptionEntry<H>>>,
    attr: Mutex<Option<String>>,
}
/// `BranchesData(branches, f)`.
pub struct BranchesData<H: Host> {
    branches: Mutex<HashMap<i32, i32>>,
    f: BranchFn<H>,
}
/// PlaceholderNode: its target is the clause size when `builder()` is called.
pub struct Placeholder<H: Host> {
    data: Data<H>,
    clause: i32,
    target: Mutex<i32>,
}
impl<H: Host> Placeholder<H> {
    pub fn target_stage(&self) -> i32 {
        *lock(&self.target)
    }
    /// `builder()`: targets the next node of the clause and continues there.
    pub fn builder(&self) -> Result<Builder<H>> {
        let size = node_count(&self.data, self.clause)?;
        *lock(&self.target) = size;
        Ok(Builder {
            data: self.data.clone(),
            clause: self.clause,
        })
    }
}

/// The DialogueNode implementations.
pub enum Node<H: Host> {
    NpcL(FacialExpression, Text),
    Npc(FacialExpression, Vec<Text>),
    Item(i32, Vec<Text>),
    PlayerL(FacialExpression, Text),
    Player(FacialExpression, Vec<Text>),
    BetweenStage(StageFn<H>),
    ManualStage(StageFn<H>),
    ManualStageWithGoto(GotoFn<H>),
    Placeholder(Arc<Placeholder<H>>),
    Goto(Arc<Placeholder<H>>),
    Options(Arc<OptionsData<H>>),
    OptionsDispatch(Arc<OptionsData<H>>),
    Branch(Arc<BranchesData<H>>),
    EndWith(EndFn<H>),
}
impl<H: Host> Clone for Node<H> {
    fn clone(&self) -> Self {
        match self {
            Self::NpcL(e, v) => Self::NpcL(*e, v.clone()),
            Self::Npc(e, v) => Self::Npc(*e, v.clone()),
            Self::Item(i, v) => Self::Item(*i, v.clone()),
            Self::PlayerL(e, v) => Self::PlayerL(*e, v.clone()),
            Self::Player(e, v) => Self::Player(*e, v.clone()),
            Self::BetweenStage(f) => Self::BetweenStage(f.clone()),
            Self::ManualStage(f) => Self::ManualStage(f.clone()),
            Self::ManualStageWithGoto(f) => Self::ManualStageWithGoto(f.clone()),
            Self::Placeholder(p) => Self::Placeholder(p.clone()),
            Self::Goto(p) => Self::Goto(p.clone()),
            Self::Options(o) => Self::Options(o.clone()),
            Self::OptionsDispatch(o) => Self::OptionsDispatch(o.clone()),
            Self::Branch(b) => Self::Branch(b.clone()),
            Self::EndWith(f) => Self::EndWith(f.clone()),
        }
    }
}

/// `DialogueClause(predicate, nodes)`.
pub struct Clause<H: Host> {
    predicate: Predicate<H>,
    nodes: Vec<Node<H>>,
}
type Data<H> = Arc<RwLock<Vec<Clause<H>>>>;

fn node_count<H: Host>(data: &Data<H>, clause: i32) -> Result<i32> {
    let d = data.read().unwrap_or_else(PoisonError::into_inner);
    let c = usize::try_from(clause)
        .ok()
        .and_then(|i| d.get(i))
        .ok_or(Error::Index)?;
    Ok(c.nodes.len() as i32)
}
fn push<H: Host>(data: &Data<H>, clause: i32, node: Node<H>) -> Result<()> {
    let mut d = data.write().unwrap_or_else(PoisonError::into_inner);
    let c = usize::try_from(clause)
        .ok()
        .and_then(|i| d.get_mut(i))
        .ok_or(Error::Index)?;
    c.nodes.push(node);
    Ok(())
}

/// The DialogueBuilderFile body.
pub struct BuilderFile<H: Host> {
    class: &'static str,
    data: Data<H>,
}

/// Constructs a DialogueBuilderFile subclass: `create(DialogueBuilder(this))`
/// runs in the constructor; a failure there fails the construction.
pub fn file<H: Host>(
    class: &'static str,
    create: impl FnOnce(&mut Builder<H>) -> Result<()>,
) -> Result<File<H>> {
    let data: Data<H> = Arc::new(RwLock::new(Vec::new()));
    let mut b = Builder {
        data: data.clone(),
        clause: -1,
    };
    create(&mut b)?;
    Ok(File::new(BuilderFile { class, data }))
}

/// One node as an observer sees it: its class and the stages it targets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeView {
    pub kind: &'static str,
    pub targets: Vec<i32>,
    pub attr: Option<String>,
}
/// The clause structure, for observers.
pub fn structure<H: Host>(file: &File<H>) -> Option<Vec<Vec<NodeView>>> {
    let body = file.body::<BuilderFile<H>>()?;
    let d = body.data.read().unwrap_or_else(PoisonError::into_inner);
    Some(
        d.iter()
            .map(|c| {
                c.nodes
                    .iter()
                    .map(|n| {
                        let (kind, targets, attr) = match n {
                            Node::NpcL(..) => ("NpcLNode", vec![], None),
                            Node::Npc(..) => ("NpcNode", vec![], None),
                            Node::Item(..) => ("ItemNode", vec![], None),
                            Node::PlayerL(..) => ("PlayerLNode", vec![], None),
                            Node::Player(..) => ("PlayerNode", vec![], None),
                            Node::BetweenStage(_) => ("BetweenStageNode", vec![], None),
                            Node::ManualStage(_) => ("ManualStageNode", vec![], None),
                            Node::ManualStageWithGoto(_) => {
                                ("ManualStageWithGotoNode", vec![], None)
                            }
                            Node::Placeholder(p) => {
                                ("PlaceholderNode", vec![p.target_stage()], None)
                            }
                            Node::Goto(p) => ("GotoNode", vec![p.target_stage()], None),
                            Node::Options(o) => (
                                "OptionsNode",
                                lock(&o.entries).iter().map(|e| e.next_stage).collect(),
                                lock(&o.attr).clone(),
                            ),
                            Node::OptionsDispatch(o) => (
                                "OptionsDispatchNode",
                                lock(&o.entries).iter().map(|e| e.next_stage).collect(),
                                lock(&o.attr).clone(),
                            ),
                            Node::Branch(b) => {
                                let mut pairs: Vec<(i32, i32)> =
                                    lock(&b.branches).iter().map(|(k, v)| (*k, *v)).collect();
                                pairs.sort();
                                (
                                    "BranchNode",
                                    pairs.into_iter().flat_map(|(k, v)| [k, v]).collect(),
                                    None,
                                )
                            }
                            Node::EndWith(_) => ("EndWithNode", vec![], None),
                        };
                        NodeView {
                            kind,
                            targets,
                            attr,
                        }
                    })
                    .collect()
            })
            .collect(),
    )
}

impl<H: Host> FileBody<H> for BuilderFile<H> {
    fn class_name(&self) -> &str {
        self.class
    }
    /// The first clause whose predicate holds handles the stage; END ends.
    fn handle(&self, host: &mut H, this: &File<H>, component: i32, button: i32) -> Result<()> {
        let mut index = 0usize;
        loop {
            let predicate = {
                let d = self.data.read().unwrap_or_else(PoisonError::into_inner);
                match d.get(index) {
                    Some(clause) => clause.predicate.clone(),
                    None => return Ok(()),
                }
            };
            let player = this.player().ok_or(Error::Null)?;
            if predicate(host, &player)? {
                let stage = this.stage(host)?;
                let next = self.clause_handle(host, this, index, component, button, stage)?;
                this.set_stage(host, next)?;
                if this.stage(host)? == END_DIALOGUE {
                    this.end(host)?;
                }
                return Ok(());
            }
            index += 1;
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<H: Host> BuilderFile<H> {
    /// `DialogueClause.handle`: the node at the stage, END past the end.
    fn clause_handle(
        &self,
        host: &mut H,
        df: &File<H>,
        clause: usize,
        component: i32,
        button: i32,
        stage: i32,
    ) -> Result<i32> {
        let node = {
            let d = self.data.read().unwrap_or_else(PoisonError::into_inner);
            let nodes = &d.get(clause).ok_or(Error::Index)?.nodes;
            if stage >= nodes.len() as i32 {
                return Ok(END_DIALOGUE);
            }
            nodes
                .get(usize::try_from(stage).map_err(|_| Error::Index)?)
                .cloned()
                .ok_or(Error::Index)?
        };
        node_handle(host, df, &node, component, button, stage)
    }
}

fn player_of<H: Host>(df: &File<H>) -> Result<NodeHandle> {
    df.player().ok_or(Error::Null)
}
fn filtered<H: Host>(
    host: &mut H,
    df: &File<H>,
    data: &OptionsData<H>,
) -> Result<Vec<OptionEntry<H>>> {
    let entries = lock(&data.entries).clone();
    let mut out = Vec::new();
    for entry in entries {
        let player = player_of(df)?;
        let keep = match &entry.predicate {
            Some(p) => p(host, &player)?,
            None => true,
        };
        if keep {
            out.push(entry);
        }
    }
    Ok(out)
}

/// `DialogueNode.handle(df, componentID, buttonID, stage)`.
fn node_handle<H: Host>(
    host: &mut H,
    df: &File<H>,
    node: &Node<H>,
    component: i32,
    button: i32,
    stage: i32,
) -> Result<i32> {
    let next = stage.wrapping_add(1);
    match node {
        Node::NpcL(expr, value) => {
            df.npcl(host, Some(*expr), Some(&value[..]))?;
            Ok(next)
        }
        Node::Npc(expr, values) => {
            df.npc_expr(host, Some(*expr), &lines(values))?;
            Ok(next)
        }
        Node::Item(item, values) => {
            let interpreter = df.interpreter().ok_or(Error::Null)?;
            interpreter.send_item_message_id(host, *item, &lines(values))?;
            Ok(next)
        }
        Node::PlayerL(expr, value) => {
            df.playerl(host, Some(*expr), Some(&value[..]))?;
            Ok(next)
        }
        Node::Player(expr, values) => {
            df.player_expr(host, Some(*expr), &lines(values))?;
            Ok(next)
        }
        Node::BetweenStage(f) => {
            df.set_stage(host, next)?;
            let player = player_of(df)?;
            f(host, df, &player, component, button)?;
            df.handle(host, component, button)?;
            Ok(stage.wrapping_add(2))
        }
        Node::ManualStage(f) => {
            let player = player_of(df)?;
            f(host, df, &player, component, button)?;
            Ok(next)
        }
        Node::ManualStageWithGoto(f) => {
            let player = player_of(df)?;
            f(host, df, &player, component, button, stage)
        }
        Node::Placeholder(p) | Node::Goto(p) => {
            df.set_stage(host, p.target_stage())?;
            df.handle(host, component, button)?;
            df.stage(host)
        }
        Node::Options(data) => {
            let player = player_of(df)?;
            let entries = lock(&data.entries).clone();
            let mut names = Vec::new();
            for entry in &entries {
                let keep = match &entry.predicate {
                    Some(p) => p(host, &player)?,
                    None => true,
                };
                if keep {
                    names.push(entry.text.clone());
                }
            }
            if names.len() > 5 {
                host.stdout(text(
                    "Only a maximum of 5 options are supported by the interface",
                ))?;
                Ok(END_DIALOGUE)
            } else if names.len() > 1 {
                let interpreter = df.interpreter().ok_or(Error::Null)?;
                interpreter.send_options(host, Some(data.header.clone()), &lines(&names))?;
                Ok(next)
            } else if names.len() == 1 {
                let tmp = filtered(host, df, data)?;
                let first = tmp.first().ok_or(Error::Index)?;
                df.set_stage(host, first.next_stage)?;
                df.handle(host, component, 0)?;
                df.stage(host)
            } else {
                Ok(END_DIALOGUE)
            }
        }
        Node::OptionsDispatch(data) => {
            let tmp = filtered(host, df, data)?;
            let attr = lock(&data.attr).clone();
            if let Some(attr) = attr {
                let player = player_of(df)?;
                host.set_value(&player, &attr, Value::Int(button.wrapping_sub(1)))?;
            }
            let index = usize::try_from(button.wrapping_sub(1)).map_err(|_| Error::Index)?;
            let target = tmp.get(index).ok_or(Error::Index)?.next_stage;
            df.set_stage(host, target)?;
            df.handle(host, component, 0)?;
            df.stage(host)
        }
        Node::Branch(data) => {
            let player = player_of(df)?;
            let value = (data.f)(host, &player)?;
            let target = lock(&data.branches)
                .get(&value)
                .copied()
                .unwrap_or(END_DIALOGUE);
            df.set_stage(host, target)?;
            df.handle(host, component, button)?;
            df.stage(host)
        }
        Node::EndWith(f) => {
            let player = player_of(df)?;
            f(host, df, &player)?;
            Ok(END_DIALOGUE)
        }
    }
}

/// `DialogueBuilder(target, clauseIndex)`.
pub struct Builder<H: Host> {
    data: Data<H>,
    clause: i32,
}
impl<H: Host> Clone for Builder<H> {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            clause: self.clause,
        }
    }
}

impl<H: Host> Builder<H> {
    fn add(self, node: Node<H>) -> Result<Self> {
        push(&self.data, self.clause, node)?;
        Ok(self)
    }
    /// `onPredicate(predicate)`: a new clause; this builder now adds to it.
    pub fn on_predicate(
        &mut self,
        f: impl Fn(&mut H, &NodeHandle) -> Result<bool> + Send + Sync + 'static,
    ) -> Self {
        let mut d = self.data.write().unwrap_or_else(PoisonError::into_inner);
        d.push(Clause {
            predicate: Arc::new(f),
            nodes: Vec::new(),
        });
        self.clause = d.len() as i32 - 1;
        drop(d);
        self.clone()
    }
    /// `defaultDialogue()`.
    pub fn default_dialogue(&mut self) -> Self {
        self.on_predicate(|_, _| Ok(true))
    }
    /// `onQuestStages(name, vararg stages)`.
    pub fn on_quest_stages(&mut self, quest: &'static str, stages: &[i32]) -> Self {
        let stages = stages.to_vec();
        self.on_predicate(move |host, player| {
            let stage = host.quest_stage(player, quest)?;
            Ok(stages.contains(&stage))
        })
    }
    pub fn playerl(self, expression: FacialExpression, value: &str) -> Result<Self> {
        self.add(Node::PlayerL(expression, text(value)))
    }
    pub fn player(self, expression: FacialExpression, values: &[Text]) -> Result<Self> {
        self.add(Node::Player(expression, values.to_vec()))
    }
    pub fn npcl(self, expression: FacialExpression, value: &str) -> Result<Self> {
        self.add(Node::NpcL(expression, text(value)))
    }
    pub fn npc(self, expression: FacialExpression, values: &[Text]) -> Result<Self> {
        self.add(Node::Npc(expression, values.to_vec()))
    }
    pub fn item(self, item: i32, values: &[Text]) -> Result<Self> {
        self.add(Node::Item(item, values.to_vec()))
    }
    /// `iteml(item, value)`: split while building.
    pub fn iteml(self, item: i32, value: &str) -> Result<Self> {
        let split = split(&text(value))?;
        let values: Vec<Text> = split
            .read()
            .iter()
            .map(|v| v.clone().unwrap_or_default())
            .collect();
        self.item(item, &values)
    }
    /// `line(vararg values)`: a manual stage sending them as a plain dialogue.
    pub fn line(self, values: &[Text]) -> Result<Self> {
        let values = values.to_vec();
        self.manual_stage(move |host, df, _, _, _| {
            let interpreter = df.interpreter().ok_or(Error::Null)?;
            interpreter.send_dialogue(host, Some(&lines(&values)))?;
            Ok(())
        })
    }
    /// `linel(value)`: split when the stage runs.
    pub fn linel(self, value: &str) -> Result<Self> {
        let value = text(value);
        self.manual_stage(move |host, df, _, _, _| {
            let interpreter = df.interpreter().ok_or(Error::Null)?;
            let l: Lines = spread(&split(&value)?);
            interpreter.send_dialogue(host, Some(&l))?;
            Ok(())
        })
    }
    /// `endWith(f)`.
    pub fn end_with(
        self,
        f: impl Fn(&mut H, &File<H>, &NodeHandle) -> Result<()> + Send + Sync + 'static,
    ) -> Result<()> {
        self.add(Node::EndWith(Arc::new(f))).map(|_| ())
    }
    /// `end()`: `endWith(::endWithNoop)`.
    pub fn end(self) -> Result<()> {
        self.end_with(|_, _, _| Ok(()))
    }
    pub fn between_stage(
        self,
        f: impl Fn(&mut H, &File<H>, &NodeHandle, i32, i32) -> Result<()> + Send + Sync + 'static,
    ) -> Result<Self> {
        self.add(Node::BetweenStage(Arc::new(f)))
    }
    pub fn manual_stage(
        self,
        f: impl Fn(&mut H, &File<H>, &NodeHandle, i32, i32) -> Result<()> + Send + Sync + 'static,
    ) -> Result<Self> {
        self.add(Node::ManualStage(Arc::new(f)))
    }
    /// `manualStageWithGoto(f, g)`: `g` is unused by the original.
    pub fn manual_stage_with_goto(
        self,
        f: impl Fn(&mut H, &File<H>, &NodeHandle, i32, i32, i32) -> Result<i32> + Send + Sync + 'static,
    ) -> Result<Self> {
        self.add(Node::ManualStageWithGoto(Arc::new(f)))
    }
    /// `options()`.
    pub fn options(self) -> Result<OptionsBuilder<H>> {
        self.options_titled("Select an Option")
    }
    /// `options(header)`: an OptionsNode and its OptionsDispatchNode.
    pub fn options_titled(self, header: &str) -> Result<OptionsBuilder<H>> {
        let data = Arc::new(OptionsData {
            header: text(header),
            entries: Mutex::new(Vec::new()),
            attr: Mutex::new(None),
        });
        push(&self.data, self.clause, Node::Options(data.clone()))?;
        push(&self.data, self.clause, Node::OptionsDispatch(data.clone()))?;
        Ok(OptionsBuilder {
            data: self.data,
            clause: self.clause,
            options: data,
        })
    }
    /// `placeholder()`: targets END until its `builder()` is called.
    pub fn placeholder(&self) -> Result<Arc<Placeholder<H>>> {
        let node = Arc::new(Placeholder {
            data: self.data.clone(),
            clause: self.clause,
            target: Mutex::new(END_DIALOGUE),
        });
        push(&self.data, self.clause, Node::Placeholder(node.clone()))?;
        Ok(node)
    }
    /// `goto(node)`.
    pub fn goto(self, node: &Arc<Placeholder<H>>) -> Result<Self> {
        self.add(Node::Goto(node.clone()))
    }
    /// `branch(f)`.
    pub fn branch(
        self,
        f: impl Fn(&mut H, &NodeHandle) -> Result<i32> + Send + Sync + 'static,
    ) -> Result<BranchBuilder<H>> {
        let branches = Arc::new(BranchesData {
            branches: Mutex::new(HashMap::new()),
            f: Arc::new(f),
        });
        push(&self.data, self.clause, Node::Branch(branches.clone()))?;
        Ok(BranchBuilder {
            data: self.data,
            clause: self.clause,
            branches,
        })
    }
    /// `branchStages(f)`: the same as `branch`.
    pub fn branch_stages(
        self,
        f: impl Fn(&mut H, &NodeHandle) -> Result<i32> + Send + Sync + 'static,
    ) -> Result<BranchBuilder<H>> {
        self.branch(f)
    }
    /// `branchBoolAttribute(attrName, defaultVal)`.
    pub fn branch_bool_attribute(self, name: &str, default: bool) -> Result<BranchBuilder<H>> {
        let name = name.to_owned();
        self.branch(move |host, player| Ok(i32::from(host.attribute_bool(player, &name, default)?)))
    }
}

/// `DialogueOptionsBuilder`.
pub struct OptionsBuilder<H: Host> {
    data: Data<H>,
    clause: i32,
    options: Arc<OptionsData<H>>,
}
impl<H: Host> OptionsBuilder<H> {
    fn entry(&self, value: &str, predicate: Option<Predicate<H>>) -> Result<Builder<H>> {
        let next_stage = node_count(&self.data, self.clause)?;
        lock(&self.options.entries).push(OptionEntry {
            text: text(value),
            next_stage,
            predicate,
        });
        Ok(Builder {
            data: self.data.clone(),
            clause: self.clause,
        })
    }
    /// `option(value)`: the entry continues at the clause's next node.
    pub fn option(&self, value: &str) -> Result<Builder<H>> {
        self.entry(value, None)
    }
    pub fn option_if(
        &self,
        value: &str,
        predicate: impl Fn(&mut H, &NodeHandle) -> Result<bool> + Send + Sync + 'static,
    ) -> Result<Builder<H>> {
        self.entry(value, Some(Arc::new(predicate)))
    }
    /// `option_playerl(value)`.
    pub fn option_playerl(&self, value: &str) -> Result<Builder<H>> {
        self.option(value)?
            .playerl(FacialExpression::Neutral, value)
    }
    /// `recordAttribute(name)`: the dispatch stores `buttonID - 1` there.
    pub fn record_attribute(self, name: &str) -> Self {
        *lock(&self.options.attr) = Some(name.into());
        self
    }
}

/// `DialogueBranchBuilder`.
pub struct BranchBuilder<H: Host> {
    data: Data<H>,
    clause: i32,
    branches: Arc<BranchesData<H>>,
}
impl<H: Host> BranchBuilder<H> {
    pub fn on_value_bool(&self, value: bool) -> Result<Builder<H>> {
        self.on_value(i32::from(value))
    }
    /// `onValue(value)`: that value continues at the clause's next node.
    pub fn on_value(&self, value: i32) -> Result<Builder<H>> {
        let index = node_count(&self.data, self.clause)?;
        lock(&self.branches.branches).insert(value, index);
        Ok(Builder {
            data: self.data.clone(),
            clause: self.clause,
        })
    }
}
