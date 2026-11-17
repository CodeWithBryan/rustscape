//! Native replay of the dialogue DSL corpus. `Model` is a recording host whose
//! services mirror the JVM stand-ins in Standins.java/Adapters.kt call for call;
//! the scripted labeller, builder and skill handler mirror Script.kt and drive
//! the real native DialogueLabeller, DialogueBuilder, SkillDialogueHandler,
//! SkillDialoguePlugin, ActivateEnchantedGem and banker conversation. Labels
//! are diagnostic output only and never select behaviour.
use rs09_server::world::{
    dialogue::{
        self as d, Arg, Attribute, Entity, Error, Event, FacialExpression, Lines, LogLevel,
        NodeHandle, Result, ScriptValue, Selection, Text,
    },
    dialogue_dsl::{
        self as dsl, CloseEvent, CloseHook, Dialogue, Host as _, InputType, Response, RunScript,
        Value, api,
        banker::{self, Activation, BankHost},
        builder::{self, Builder, Placeholder},
        gem,
        labeller::{self, Conversation, DialogueOption, Dsl},
        skill::{self, HandlerBody, SkillDialogue},
    },
    doors::Location,
    player_walk::State,
};
use serde_json::{Value as J, json};
use std::{collections::BTreeMap, collections::HashMap, sync::Arc};

type File = d::File<Model>;
type Plugin = d::Plugin<Model>;
type Handler = skill::Handler<Model>;

fn int(v: &J) -> i32 {
    v.as_i64().unwrap() as i32
}
fn text(v: &J) -> Option<Text> {
    if v.is_null() {
        None
    } else if let Some(s) = v.as_str() {
        Some(s.encode_utf16().collect())
    } else {
        Some(
            v.as_array()
                .unwrap()
                .iter()
                .map(|u| u.as_u64().unwrap() as u16)
                .collect(),
        )
    }
}
fn texts(v: &J) -> Vec<Text> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|t| text(t).unwrap())
        .collect()
}
fn units(t: &[u16]) -> J {
    if t.iter().any(|c| (0xD800..=0xDFFF).contains(c)) {
        json!(t)
    } else {
        json!(String::from_utf16(t).unwrap())
    }
}
fn anim(v: &J) -> FacialExpression {
    FacialExpression::value_of(v.as_str().unwrap()).unwrap()
}
fn flag(o: &[J], i: usize) -> bool {
    o.get(i) == Some(&json!(true))
}
fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Index => "Index",
        Error::Cast => "Cast",
        Error::Number => "Number",
        Error::IllegalArgument(_) => "IllegalArgument",
        Error::Binding(_) => "Binding",
    }
}
fn failure() -> Error {
    Error::Binding("recording failure".into())
}
fn quest(name: &str) -> &'static str {
    match name {
        "COOKS_ASSISTANT" => "COOKS_ASSISTANT",
        "DRAGON_SLAYER" => "DRAGON_SLAYER",
        other => panic!("unknown quest {other}"),
    }
}
fn response(r: &Option<Response>) -> J {
    match r {
        None => J::Null,
        Some(Response::Int(v)) => json!(v),
        Some(Response::Text(t)) => json!(["s", units(t)]),
    }
}

struct Npc {
    id: i32,
}
struct Item {
    id: i32,
    amount: i32,
    name: Option<Text>,
}
struct Comp {
    id: i32,
    close: Option<CloseEvent>,
}
enum Attr {
    Json(J),
    Script(RunScript<Model>),
}

pub struct Model {
    source: J,
    events: Vec<J>,
    count: usize,
    fail_at: usize,
    node: NodeHandle,
    _state: State,
    interp: d::Interpreter<Model>,
    chat: usize,
    comps: Vec<Comp>,
    npcs: Vec<Npc>,
    items: Vec<Item>,
    attrs: BTreeMap<String, Attr>,
    hooks: Vec<CloseHook<Model>>,
    bank: serde_json::Map<String, J>,
    seen: Vec<(Plugin, String)>,
    counts: BTreeMap<String, usize>,
    files: Vec<File>,
    handlers: Vec<Handler>,
}

/// Script.kt's ScriptLabeller.
struct ScriptConversation(J);
impl Conversation<Model> for ScriptConversation {
    fn class_name(&self) -> &str {
        "ScriptLabeller"
    }
    fn add_conversation(&self, host: &mut Model, c: &Dsl<'_, Model>) -> Result<()> {
        run(host, c, &self.0)
    }
}

fn cond(host: &mut Model, raw: &J, player: &NodeHandle, npc: Option<usize>) -> Result<bool> {
    if let Some(b) = raw.as_bool() {
        return Ok(b);
    }
    if let Some(key) = raw.get("attr") {
        return host.attribute_bool(player, key.as_str().unwrap(), false);
    }
    if let Some(id) = raw.get("npc") {
        let npc = npc.ok_or(Error::Null)?;
        return Ok(d::Host::npc_id(host, &npc)? == int(id));
    }
    if let Some(inner) = raw.get("not") {
        return Ok(!cond(host, inner, player, npc)?);
    }
    panic!("unknown condition {raw}")
}
fn value(raw: &J) -> Value<Model> {
    match raw {
        J::Bool(b) => Value::Bool(*b),
        J::Number(n) => Value::Int(n.as_i64().unwrap() as i32),
        other => panic!("unsupported attribute value {other}"),
    }
}
fn target(host: &mut Model, raw: &J) -> Result<Dialogue<Model>> {
    if let Some(key) = raw.get("key") {
        return Ok(Dialogue::Key(int(key)));
    }
    Ok(Dialogue::File(host.new_file(int(&raw["file"]))?))
}

/// ScriptLabeller.sub: exec/manual/afterClose callbacks.
fn sub(
    host: &mut Model,
    c: Option<&Dsl<'_, Model>>,
    ops: &J,
    player: &NodeHandle,
    npc: Option<usize>,
) -> Result<Option<usize>> {
    let mut component = None;
    for o in ops.as_array().unwrap() {
        let o = o.as_array().unwrap();
        match o[0].as_str().unwrap() {
            "loadLabel" => c.unwrap().load_label(player, o[1].as_str().unwrap()),
            "goto" => c.unwrap().goto(o[1].as_str().unwrap()),
            "emit" => host.emit("cb", vec![o[1].clone(), json!(0), json!(npc)])?,
            "setAttr" => host.set_value(player, o[1].as_str().unwrap(), value(&o[2]))?,
            "if" => {
                component = if cond(host, &o[1], player, npc)? {
                    sub(host, c, &o[2], player, npc)?
                } else {
                    sub(host, c, &o[3], player, npc)?
                }
            }
            "fail" => return Err(failure()),
            "openDialogue" => {
                let dialogue = target(host, &o[1])?;
                c.unwrap().open_dialogue(host, player, &dialogue, &[])?;
            }
            "send" => {
                let interpreter = c.unwrap().file().interpreter().ok_or(Error::Null)?;
                component = interpreter.send_dialogue(host, Some(&dsl::lines(&texts(&o[1]))))?;
            }
            "emitInput" => {
                let c = c.unwrap();
                let (input, button) = (response(&c.opt_input()), c.opt_button());
                host.emit("input", vec![input, json!(button)])?
            }
            other => panic!("unknown callback op {other}"),
        }
    }
    Ok(component)
}

fn option(raw: &J) -> DialogueOption<Model> {
    let option = text(&raw["option"]).unwrap();
    let mut out = DialogueOption::new(raw["goto"].as_str().unwrap(), "");
    out.option = option.clone();
    out.spoken_text = text(&raw["spoken"]).unwrap_or(option);
    if !raw["expr"].is_null() {
        out = out.expression(anim(&raw["expr"]));
    }
    out = out.skip_player(raw["skip"] == json!(true));
    if !raw["if"].is_null() {
        let condition = raw["if"].clone();
        out = out.only_if(move |host: &mut Model, p, n| cond(host, &condition, p, Some(*n)));
    }
    out
}

/// ScriptLabeller.run: one pass of addConversation.
fn run(host: &mut Model, c: &Dsl<'_, Model>, ops: &J) -> Result<()> {
    for o in ops.as_array().unwrap() {
        let o = o.as_array().unwrap();
        let s = |i: usize| o[i].as_str().unwrap();
        match s(0) {
            "label" => {
                let names: Vec<String> = o[1]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_str().unwrap().to_owned())
                    .collect();
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                c.label(&names);
                if o.len() > 2 {
                    run(host, c, &o[2])?;
                }
            }
            "goto" => c.goto(s(1)),
            "assign" => c.assign_to_ids(int(&o[1])),
            "npc" => c.npc_lines(host, anim(&o[1]), &texts(&o[2]), flag(o, 3))?,
            "npcPlain" => c.npc(host, &texts(&o[1]))?,
            "npcAs" => c.npc_as(host, anim(&o[1]), int(&o[2]), &texts(&o[3]), flag(o, 4))?,
            "npcId" => c.npc_id(host, int(&o[1]), &texts(&o[2]), flag(o, 3))?,
            "player" => c.player_lines(host, anim(&o[1]), &texts(&o[2]), flag(o, 3))?,
            "playerPlain" => c.player(host, &texts(&o[1]))?,
            "item" => {
                let item = int(&o[1]) as usize;
                let message = text(&o[3]).unwrap_or_default();
                c.item(host, &item, &texts(&o[2]), &message, flag(o, 4))?
            }
            "item2" => {
                let (a, b) = (int(&o[1]) as usize, int(&o[2]) as usize);
                let message = text(&o[4]).unwrap_or_default();
                c.item2(host, &a, &b, &texts(&o[3]), &message, flag(o, 5))?
            }
            "line" => c.line(host, &texts(&o[1]), flag(o, 2))?,
            "options" => {
                let list: Vec<Arc<DialogueOption<Model>>> = o[1]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| Arc::new(option(r)))
                    .collect();
                let title = text(&o[2]).map_or_else(
                    || "Select an Option".to_owned(),
                    |t| String::from_utf16(&t).unwrap(),
                );
                c.options(host, &list, &title, flag(o, 3))?
            }
            "input" => {
                let kind = match s(1) {
                    "AMOUNT" => InputType::Amount,
                    "NUMERIC" => InputType::Numeric,
                    "STRING_SHORT" => InputType::Short,
                    "STRING_LONG" => InputType::Long,
                    "MESSAGE" => InputType::Message,
                    other => panic!("{other}"),
                };
                let prompt = text(&o[2]).map_or_else(
                    || "Enter the amount".to_owned(),
                    |t| String::from_utf16(&t).unwrap(),
                );
                c.input(host, kind, &prompt)?
            }
            "inputBool" => {
                let prompt = text(&o[2]).map_or_else(
                    || "Enter the amount".to_owned(),
                    |t| String::from_utf16(&t).unwrap(),
                );
                c.input_numeric(host, o[1] == json!(true), &prompt)?
            }
            "exec" => c.exec(host, |host, p, n| {
                sub(host, Some(c), &o[1], p, Some(*n)).map(|_| ())
            })?,
            "manual" => c.manual(host, flag(o, 1), |host, p, n| {
                sub(host, Some(c), &o[2], p, Some(*n))
            })?,
            "afterClose" => {
                let ops = o[1].clone();
                c.after_close(host, move |host: &mut Model, p| {
                    sub(host, None, &ops, p, None).map(|_| ())
                })?
            }
            "open" => {
                let t = &o[1];
                let player = c.file().player().ok_or(Error::Null)?;
                let dialogue = target(host, t)?;
                if t["npc"] == json!(true) {
                    let npc = c.file().npc().ok_or(Error::Null)?;
                    c.open(host, &player, &dialogue, &[Arg::Npc(npc)])?
                } else {
                    c.open(host, &player, &dialogue, &[])?
                }
            }
            other => panic!("unknown labeller op {other}"),
        }
    }
    Ok(())
}

/// ScriptBuilder.callback.
fn callback(
    host: &mut Model,
    ops: &J,
    df: &File,
    player: &NodeHandle,
    cid: Option<i32>,
    bid: Option<i32>,
) -> Result<()> {
    let _ = player;
    for o in ops.as_array().unwrap() {
        let o = o.as_array().unwrap();
        match o[0].as_str().unwrap() {
            "emit" => host.emit("cb", vec![o[1].clone(), json!(0), json!(cid), json!(bid)])?,
            "stage" => df.set_stage(host, int(&o[1]))?,
            "npcl" => {
                let t = text(&o[2]).unwrap();
                df.npcl(host, Some(anim(&o[1])), Some(&t[..]))?;
            }
            "send" => {
                let interpreter = df.interpreter().ok_or(Error::Null)?;
                interpreter.send_dialogue(host, Some(&dsl::lines(&texts(&o[1]))))?;
            }
            "end" => df.end(host)?,
            "setAttr" => host.set_value(player, o[1].as_str().unwrap(), value(&o[2]))?,
            "fail" => return Err(failure()),
            other => panic!("unknown builder callback {other}"),
        }
    }
    Ok(())
}

type Holders = HashMap<String, Arc<Placeholder<Model>>>;

/// ScriptBuilder.chain.
fn chain(start: Builder<Model>, ops: &J, holders: &mut Holders) -> Result<()> {
    let mut b = start;
    for o in ops.as_array().unwrap() {
        let o = o.as_array().unwrap();
        let expr = |i: usize| {
            if o[i].is_null() {
                FacialExpression::Neutral
            } else {
                anim(&o[i])
            }
        };
        let str_at = |i: usize| String::from_utf16(&text(&o[i]).unwrap()).unwrap();
        match o[0].as_str().unwrap() {
            "npcl" => b = b.npcl(expr(1), &str_at(2))?,
            "npc" => b = b.npc(expr(1), &texts(&o[2]))?,
            "playerl" => b = b.playerl(expr(1), &str_at(2))?,
            "player" => b = b.player(expr(1), &texts(&o[2]))?,
            "item" => b = b.item(int(&o[1]), &texts(&o[2]))?,
            "iteml" => b = b.iteml(int(&o[1]), &str_at(2))?,
            "line" => b = b.line(&texts(&o[1]))?,
            "linel" => b = b.linel(&str_at(1))?,
            "between" => {
                let ops = o[1].clone();
                b = b.between_stage(move |host, df, p, c, i| {
                    callback(host, &ops, df, p, Some(c), Some(i))
                })?
            }
            "manual" => {
                let ops = o[1].clone();
                b = b.manual_stage(move |host, df, p, c, i| {
                    callback(host, &ops, df, p, Some(c), Some(i))
                })?
            }
            "manualGoto" => {
                let (ops, ret) = (o[1].clone(), o[2].clone());
                b = b.manual_stage_with_goto(move |host, df, p, c, i, s| {
                    callback(host, &ops, df, p, Some(c), Some(i))?;
                    Ok(match ret.get("plus") {
                        Some(n) => s + int(n),
                        None => int(&ret),
                    })
                })?
            }
            "end" => b.clone().end()?,
            "endWith" => {
                let ops = o[1].clone();
                b.clone()
                    .end_with(move |host, df, p| callback(host, &ops, df, p, None, None))?
            }
            "options" => {
                let mut ob = if o[1].is_null() {
                    b.clone().options()?
                } else {
                    b.clone().options_titled(&str_at(1))?
                };
                if !o[2].is_null() {
                    ob = ob.record_attribute(o[2].as_str().unwrap());
                }
                for e in o[3].as_array().unwrap() {
                    let e = e.as_array().unwrap();
                    let label = String::from_utf16(&text(&e[1]).unwrap()).unwrap();
                    let next = match e[0].as_str().unwrap() {
                        "option" => ob.option(&label)?,
                        "optionIf" => {
                            let condition = e[2].clone();
                            ob.option_if(&label, move |host, p| cond(host, &condition, p, None))?
                        }
                        "playerl" => ob.option_playerl(&label)?,
                        other => panic!("{other}"),
                    };
                    chain(next, &e[3], holders)?;
                }
            }
            "placeholder" => {
                let node = b.placeholder()?;
                holders.insert(o[1].as_str().unwrap().into(), node);
            }
            "builder" => {
                let node = holders
                    .get(o[1].as_str().unwrap())
                    .cloned()
                    .ok_or(Error::Null)?;
                chain(node.builder()?, &o[2], holders)?
            }
            "goto" => {
                let node = holders
                    .get(o[1].as_str().unwrap())
                    .cloned()
                    .ok_or(Error::Null)?;
                b = b.goto(&node)?
            }
            "branch" => {
                let key = o[1]["attrInt"].as_str().unwrap().to_owned();
                let bb = b
                    .clone()
                    .branch(move |host: &mut Model, p| host.attribute_int(p, &key, 0))?;
                for e in o[2].as_array().unwrap() {
                    chain(bb.on_value(int(&e[0]))?, &e[1], holders)?;
                }
            }
            "branchBool" => {
                let bb = b
                    .clone()
                    .branch_bool_attribute(o[1].as_str().unwrap(), o[2] == json!(true))?;
                for e in o[3].as_array().unwrap() {
                    chain(bb.on_value_bool(e[0] == json!(true))?, &e[1], holders)?;
                }
            }
            other => panic!("unknown builder op {other}"),
        }
    }
    Ok(())
}

fn create(b: &mut Builder<Model>, spec: &J) -> Result<()> {
    let mut holders = Holders::new();
    if !spec["bare"].is_null() {
        chain(b.clone(), &spec["bare"], &mut holders)?;
    }
    for clause in spec["clauses"].as_array().unwrap() {
        let when = &clause["when"];
        let builder = if when == "default" {
            b.default_dialogue()
        } else if let Some(name) = when.get("quest") {
            let stages: Vec<i32> = when["stages"].as_array().unwrap().iter().map(int).collect();
            b.on_quest_stages(quest(name.as_str().unwrap()), &stages)
        } else {
            let condition = when["cond"].clone();
            b.on_predicate(move |host, p| cond(host, &condition, p, None))
        };
        chain(builder, &clause["chain"], &mut holders)?;
    }
    Ok(())
}

/// Script.kt's ScriptSkill.
struct ScriptSkill {
    spec: J,
    label: String,
}
impl HandlerBody<Model> for ScriptSkill {
    fn create(&self, host: &mut Model, _: &Handler, amount: i32, index: i32) -> Result<()> {
        host.emit(
            "create",
            vec![json!(self.label), json!(amount), json!(index)],
        )?;
        if self.spec["createFails"] == json!(true) {
            return Err(failure());
        }
        Ok(())
    }
    fn get_all(&self, host: &mut Model, handler: &Handler, index: i32) -> Result<i32> {
        if self.spec["getAll"].is_null() {
            return handler.base_get_all(host, index);
        }
        host.emit("getAll", vec![json!(self.label), json!(index)])?;
        Ok(int(&self.spec["getAll"]))
    }
    fn get_name(&self, host: &mut Model, handler: &Handler, item: &usize) -> Result<Text> {
        if self.spec["getName"] != json!(true) {
            return handler.base_get_name(host, item);
        }
        host.emit("getName", vec![json!(self.label), json!(item)])?;
        Ok(format!("Custom {}", self_id(host, item)?)
            .encode_utf16()
            .collect())
    }
}
fn self_id(host: &mut Model, item: &usize) -> Result<i32> {
    d::Host::item_id(host, item)
}

impl Model {
    fn emit(&mut self, name: &str, args: Vec<J>) -> Result<()> {
        let mut row = vec![json!(name)];
        row.extend(args);
        self.events.push(json!(row));
        self.count += 1;
        if self.fail_at > 0 && self.count == self.fail_at {
            return Err(failure());
        }
        Ok(())
    }
    fn flag(&self, key: &str) -> bool {
        self.bank.get(key) == Some(&json!(true))
    }
    fn attribute_int(&mut self, _p: &NodeHandle, key: &str, default: i32) -> Result<i32> {
        self.emit("getAttribute", vec![json!(0), json!(key)])?;
        match self.attrs.get(key) {
            None => Ok(default),
            Some(Attr::Json(J::Number(n))) => Ok(n.as_i64().unwrap() as i32),
            Some(_) => Err(Error::Cast),
        }
    }
    fn new_file(&mut self, k: i32) -> Result<File> {
        let spec = self.source["files"][k as usize].clone();
        let file = match spec["kind"].as_str().unwrap() {
            "banker" => labeller::file(banker::BankerConversation),
            "labeller" => labeller::file(ScriptConversation(spec["ops"].clone())),
            _ => builder::file("ScriptBuilder", |b| create(b, &spec))?,
        };
        self.files.push(file.clone());
        Ok(file)
    }
    fn label(&mut self, plugin: Option<&Plugin>) -> J {
        let Some(plugin) = plugin else {
            return J::Null;
        };
        if let Some((_, label)) = self.seen.iter().find(|(p, _)| p.same(plugin)) {
            return json!(label);
        }
        let class = plugin.class_name().to_owned();
        let count = self.counts.entry(class.clone()).or_default();
        *count += 1;
        let label = format!("{class}#{count}");
        self.seen.push((plugin.clone(), label.clone()));
        json!(label)
    }
    fn file_label(&self, file: Option<&File>) -> J {
        let Some(file) = file else {
            return J::Null;
        };
        match self.files.iter().position(|f| f.same(file)) {
            Some(i) => json!(format!("F{i}")),
            None => json!(file.class_name()),
        }
    }
    fn describe(&self, v: &Attr) -> J {
        match v {
            Attr::Json(j) => j.clone(),
            Attr::Script(_) => json!("fn"),
        }
    }

    fn act(&mut self, a: &J) -> Result<J> {
        let player = self.node.clone();
        let interp = self.interp.clone();
        let npc = a["npc"].as_u64().map(|n| n as usize);
        Ok(match a["op"].as_str().unwrap() {
            "talk" => {
                let file = self.new_file(int(&a["file"]))?;
                labeller::open(self, &player, &Dialogue::File(file), &npc.unwrap())?;
                J::Null
            }
            "open" => {
                let file = self.new_file(int(&a["file"]))?;
                let args: Vec<Arg<Model>> = npc.map(Arg::Npc).into_iter().collect();
                api::open_dialogue(self, &player, &Dialogue::File(file), &args)?;
                J::Null
            }
            "openKey" => {
                api::open_dialogue(self, &player, &Dialogue::Key(int(&a["key"])), &[])?;
                J::Null
            }
            "openOther" => {
                api::open_dialogue(self, &player, &Dialogue::Other("String"), &[])?;
                J::Null
            }
            "handle" => {
                interp.handle(self, a["c"].as_i64().unwrap_or(0) as i32, int(&a["b"]))?;
                J::Null
            }
            "close" => json!(interp.close(self)?),
            "input" => {
                let script = match self.attrs.get("runscript") {
                    Some(Attr::Script(s)) => s.clone(),
                    _ => return Ok(json!("none")),
                };
                let value = match &a["v"] {
                    J::Number(n) => Response::Int(n.as_i64().unwrap() as i32),
                    other => Response::Text(text(other).unwrap()),
                };
                script.invoke(self, value)?;
                J::Null
            }
            "closeEvent" => match self.comps[self.chat].close {
                None => json!("none"),
                Some(event) => json!(dsl::close_event(self, event, &player)?),
            },
            "skill" => {
                let handler = self.handler(&a["handler"]);
                if a["via"] == json!("api") {
                    api::open_dialogue(self, &player, &Dialogue::Skill(handler.clone()), &[])?;
                } else {
                    handler.open(self)?;
                }
                json!(format!("H{}", self.handlers.len() - 1))
            }
            "skillTable" => {
                let handler = self.handler(&json!({"type": a["type"], "items": a["items"]}));
                let kind = handler.kind().ok_or(Error::Null)?;
                let mut out = Vec::new();
                for i in 0..=26 {
                    let amount = kind.get_amount(self, &handler, i)?;
                    out.push(json!([amount, kind.get_index(i)]));
                }
                json!(out)
            }
            "forLength" => json!(SkillDialogue::for_length(int(&a["n"])).map(SkillDialogue::name)),
            "gem" => json!(gem::handle(self, &player)?),
            "gemInit" => {
                gem::new_instance(self)?;
                json!(gem::is_walk())
            }
            other => panic!("unknown action {other}"),
        })
    }
    fn handler(&mut self, spec: &J) -> Handler {
        let kind = spec["type"]
            .as_str()
            .map(|t| SkillDialogue::value_of(t).unwrap());
        let items: Vec<usize> = spec["items"]
            .as_array()
            .map(|l| l.iter().map(|i| int(i) as usize).collect())
            .unwrap_or_default();
        let label = format!("H{}", self.handlers.len());
        let handler = Handler::new(
            self.node.clone(),
            kind,
            items,
            ScriptSkill {
                spec: spec.clone(),
                label,
            },
        );
        self.handlers.push(handler.clone());
        handler
    }

    fn file_state(&mut self, file: &File) -> J {
        let mut m = serde_json::Map::new();
        m.insert("base".into(), json!(file.base_stage()));
        m.insert("player".into(), json!(file.player().map(|_| 0)));
        m.insert("npc".into(), json!(file.npc()));
        m.insert(
            "interp".into(),
            json!(
                file.interpreter()
                    .map(|i| if i.same(&self.interp) { 0 } else { -1 })
            ),
        );
        if let Some(l) = file.body::<labeller::Labeller<Model>>() {
            let f = l.fields();
            m.insert("stage".into(), json!(f.stage));
            m.insert("counter".into(), json!(f.dialogue_counter));
            m.insert("start".into(), json!(f.starting_stage));
            m.insert("hit".into(), json!(f.stage_hit));
            m.insert("jump".into(), json!(f.jump_to));
            m.insert("button".into(), json!(f.button_id));
            m.insert("optButton".into(), json!(f.opt_button));
            m.insert("optInput".into(), response(&f.opt_input));
            m.insert("filtered".into(), json!(f.filtered_options));
            m.insert(
                "labels".into(),
                json!(
                    f.label_stage_map
                        .iter()
                        .map(|(k, v)| json!([k, v]))
                        .collect::<Vec<_>>()
                ),
            );
        } else {
            let clauses = builder::structure(file).unwrap();
            m.insert(
                "clauses".into(),
                json!(
                    clauses
                        .iter()
                        .map(|c| c
                            .iter()
                            .map(|n| json!([n.kind, n.targets, n.attr]))
                            .collect::<Vec<_>>())
                        .collect::<Vec<_>>()
                ),
            );
        }
        J::Object(m)
    }
    fn snapshot(&mut self, result: J, error: Option<&Error>, mark: usize) -> J {
        let dialogue = self.interp.dialogue();
        let interp = json!([
            self.label(dialogue.as_ref()),
            self.interp.key(),
            self.interp.active_topics().read().len()
        ]);
        let files: Vec<File> = self.files.clone();
        let files: Vec<J> = files.iter().map(|f| self.file_state(f)).collect();
        let mut plugins = Vec::new();
        let mut i = 0;
        while i < self.seen.len() {
            let p = self.seen[i].0.clone();
            let label = self.label(Some(&p));
            plugins.push(json!([
                label,
                p.stage(),
                p.is_finished(),
                self.file_label(p.file().as_ref())
            ]));
            i += 1;
        }
        let attrs: serde_json::Map<String, J> = self
            .attrs
            .iter()
            .map(|(k, v)| (k.clone(), self.describe(v)))
            .collect();
        json!({
            "error": error.map(error_name),
            "result": result,
            "interp": interp,
            "files": files,
            "plugins": plugins,
            "attrs": attrs,
            "chat": self.chat,
            "comps": self.comps.iter().map(|c| json!([c.id, c.close.is_some()])).collect::<Vec<_>>(),
            "hooks": self.hooks.len(),
            "bank": [self.flag("secondary")],
            "events": self.events[mark..].to_vec(),
        })
    }
    fn new(row: &J) -> Self {
        let mut state = State::default();
        let id = state.add(0, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
        let node = state.retain_node(id).unwrap();
        let registry = d::Registry::new();
        let interp = d::Interpreter::new(Some(node.clone()), registry.clone());
        let npcs = row["npcs"]
            .as_array()
            .map_or_else(|| vec![494], |l| l.iter().map(int).collect());
        let mut model = Self {
            source: row.clone(),
            events: Vec::new(),
            count: 0,
            fail_at: 0,
            node,
            _state: state,
            interp,
            chat: 0,
            comps: vec![Comp {
                id: 137,
                close: None,
            }],
            npcs: npcs.into_iter().map(|id| Npc { id }).collect(),
            items: row["items"]
                .as_array()
                .map(|l| {
                    l.iter()
                        .map(|it| Item {
                            id: int(&it[0]),
                            amount: int(&it[1]),
                            name: match it.get(2) {
                                None => {
                                    Some(format!("Item {}", int(&it[0])).encode_utf16().collect())
                                }
                                Some(n) => text(n),
                            },
                        })
                        .collect()
                })
                .unwrap_or_default(),
            attrs: row["attrs"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.clone(), Attr::Json(v.clone())))
                        .collect()
                })
                .unwrap_or_default(),
            hooks: Vec::new(),
            bank: row["bank"].as_object().cloned().unwrap_or_default(),
            seen: Vec::new(),
            counts: BTreeMap::new(),
            files: Vec::new(),
            handlers: Vec::new(),
        };
        if row["skill"] == json!(true) {
            let prototype = skill::SkillDialoguePlugin::prototype();
            prototype.init(&mut model, &registry).unwrap();
        }
        model.events.clear();
        model
    }
}

impl d::Host for Model {
    type Npc = usize;
    type Item = usize;
    type ItemDefinition = i32;
    type Dispatch = usize;
    type Manager = usize;
    type Component = usize;
    type WalkingQueue = usize;
    type PulseManager = usize;

    fn interpreter(&mut self, _: &NodeHandle) -> Result<Option<d::Interpreter<Self>>> {
        self.emit("interpreter", vec![json!(0)])?;
        Ok(Some(self.interp.clone()))
    }
    fn dispatch(&mut self, _: &NodeHandle) -> Result<usize> {
        self.emit("dispatch", vec![json!(0)])?;
        Ok(0)
    }
    fn manager(&mut self, _: &NodeHandle) -> Result<usize> {
        self.emit("manager", vec![json!(0)])?;
        Ok(0)
    }
    fn username(&mut self, _: &NodeHandle) -> Result<Option<Text>> {
        self.emit("username", vec![json!(0)])?;
        Ok(Some("Player0".encode_utf16().collect()))
    }
    fn is_male(&mut self, _: &NodeHandle) -> Result<bool> {
        self.emit("male", vec![json!(0)])?;
        Ok(self.source["male"] != json!(false))
    }
    fn is_debug(&mut self, _: &NodeHandle) -> Result<bool> {
        self.emit("isDebug", vec![json!(0)])?;
        Ok(false)
    }
    fn player_id(&mut self, _: &NodeHandle) -> Result<i32> {
        self.emit("playerId", vec![json!(0)])?;
        Ok(0)
    }
    fn send_message(&mut self, _: &NodeHandle, message: Text) -> Result<()> {
        self.emit("message", vec![json!(0), units(&message)])
    }
    fn set_attribute(&mut self, _: &NodeHandle, key: &str, value: Attribute) -> Result<()> {
        let value = match value {
            Attribute::Int(v) => json!(v),
            Attribute::Bool(v) => json!(v),
        };
        self.emit("setAttribute", vec![json!(0), json!(key), value.clone()])?;
        self.attrs.insert(key.into(), Attr::Json(value));
        Ok(())
    }
    fn dispatch_event(&mut self, player: &NodeHandle, event: Event<Self>) -> Result<()> {
        let (described, close) = match event {
            Event::Open(plugin) => (json!(["open", self.label(Some(&plugin))]), false),
            Event::Close(plugin) => (json!(["close", self.label(Some(&plugin))]), true),
            Event::OptionSelection {
                dialogue,
                current_stage,
                option_id,
            } => {
                let label = match dialogue {
                    Selection::Plugin(plugin) => self.label(Some(&plugin)),
                    Selection::File(file) => self.file_label(Some(&file)),
                };
                (json!(["select", label, current_stage, option_id]), false)
            }
        };
        self.emit("event", vec![json!(0), described])?;
        if close {
            for hook in self.hooks.clone() {
                hook.process(self, player)?;
            }
        }
        Ok(())
    }
    fn send_string(&mut self, d: usize, value: Option<Text>, iface: i32, child: i32) -> Result<()> {
        let value = value.as_deref().map_or(J::Null, units);
        self.emit("text", vec![json!(d), value, json!(iface), json!(child)])
    }
    fn send_interface_config(
        &mut self,
        d: usize,
        iface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()> {
        self.emit(
            "hidden",
            vec![json!(d), json!(iface), json!(child), json!(hidden)],
        )
    }
    fn send_animation_interface(
        &mut self,
        d: usize,
        animation: i32,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        self.emit(
            "anim",
            vec![json!(d), json!(animation), json!(iface), json!(child)],
        )
    }
    fn send_item_on_interface(
        &mut self,
        d: usize,
        item: i32,
        amount: i32,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        self.emit(
            "itemOn",
            vec![
                json!(d),
                json!(item),
                json!(amount),
                json!(iface),
                json!(child),
            ],
        )
    }
    fn send_npc_on_interface(&mut self, d: usize, npc: i32, iface: i32, child: i32) -> Result<()> {
        self.emit(
            "npcOn",
            vec![json!(d), json!(npc), json!(iface), json!(child)],
        )
    }
    fn send_player_on_interface(&mut self, d: usize, iface: i32, child: i32) -> Result<()> {
        self.emit("playerOn", vec![json!(d), json!(iface), json!(child)])
    }
    fn send_reposition_on_interface(
        &mut self,
        d: usize,
        iface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<()> {
        self.emit(
            "reposition",
            vec![json!(d), json!(iface), json!(child), json!(x), json!(y)],
        )
    }
    fn send_angle_on_interface(
        &mut self,
        d: usize,
        iface: i32,
        child: i32,
        zoom: i32,
        pitch: i32,
        yaw: i32,
    ) -> Result<()> {
        self.emit(
            "angle",
            vec![
                json!(d),
                json!(iface),
                json!(child),
                json!(zoom),
                json!(pitch),
                json!(yaw),
            ],
        )
    }
    fn send_run_script(
        &mut self,
        d: usize,
        id: i32,
        types: &str,
        arguments: Vec<ScriptValue>,
    ) -> Result<()> {
        let described: Vec<J> = arguments
            .iter()
            .map(|v| match v {
                ScriptValue::Null => J::Null,
                ScriptValue::Int(v) => json!(v),
                ScriptValue::Bool(v) => json!(v),
                ScriptValue::Text(v) => json!(["s", units(v)]),
            })
            .collect();
        self.emit(
            "runScript",
            vec![json!(d), json!(id), json!(types), json!(described)],
        )
    }
    fn open_chatbox(&mut self, m: usize, iface: i32) -> Result<()> {
        self.comps.push(Comp {
            id: iface,
            close: None,
        });
        self.chat = self.comps.len() - 1;
        self.emit("open", vec![json!(m), json!(iface), json!(self.chat)])
    }
    fn close_chatbox(&mut self, m: usize) -> Result<()> {
        self.emit("closeChatbox", vec![json!(m)])?;
        self.chat = 0;
        Ok(())
    }
    fn chatbox(&mut self, m: usize) -> Result<Option<usize>> {
        self.emit("chatbox", vec![json!(m)])?;
        Ok(Some(self.chat))
    }
    fn component_id(&mut self, c: &usize) -> Result<i32> {
        self.emit("id", vec![json!(c)])?;
        Ok(self.comps[*c].id)
    }
    fn has_close_event(&mut self, c: &usize) -> Result<bool> {
        self.emit("closeEvent", vec![json!(c)])?;
        Ok(self.comps[*c].close.is_some())
    }
    fn npc_id(&mut self, npc: &usize) -> Result<i32> {
        self.emit("npcId", vec![json!(npc)])?;
        Ok(self.npcs[*npc].id)
    }
    fn shown_npc(&mut self, npc: &usize, _: Option<&NodeHandle>) -> Result<usize> {
        self.emit("shownNpc", vec![json!(npc), json!(0)])?;
        Ok(*npc)
    }
    fn set_dialogue_player(&mut self, npc: &usize, player: Option<&NodeHandle>) -> Result<()> {
        self.emit("dialoguePlayer", vec![json!(npc), json!(player.map(|_| 0))])
    }
    fn walking_queue(&mut self, npc: &usize) -> Result<usize> {
        self.emit("walkingQueue", vec![json!(npc)])?;
        Ok(*npc)
    }
    fn reset_walking_queue(&mut self, queue: usize) -> Result<()> {
        self.emit("walkReset", vec![json!(queue)])
    }
    fn pulse_manager(&mut self, npc: &usize) -> Result<usize> {
        self.emit("pulseManager", vec![json!(npc)])?;
        Ok(*npc)
    }
    fn clear_pulses(&mut self, manager: usize) -> Result<()> {
        self.emit("pulseClear", vec![json!(manager)])
    }
    fn npc_name(&mut self, id: i32) -> Result<Option<Text>> {
        self.emit("npcDef", vec![json!(id)])?;
        Ok(Some(format!("Npc {id}").encode_utf16().collect()))
    }
    fn item_definition(&mut self, id: i32) -> Result<i32> {
        self.emit("itemDef", vec![json!(id)])?;
        Ok(id)
    }
    fn model_zoom(&mut self, definition: &i32) -> Result<i32> {
        Ok(2000i32.wrapping_add(*definition))
    }
    fn model_rotation_x(&mut self, definition: &i32) -> Result<i32> {
        Ok(definition.wrapping_mul(3))
    }
    fn model_rotation_y(&mut self, definition: &i32) -> Result<i32> {
        Ok(definition.wrapping_mul(7))
    }
    fn definition_name(&mut self, definition: &i32) -> Result<Option<Text>> {
        Ok(Some(format!("Def {definition}").encode_utf16().collect()))
    }
    fn destroy_message(&mut self, _: &i32) -> Result<Option<Text>> {
        Ok(None)
    }
    fn new_item(&mut self, id: i32) -> Result<usize> {
        dsl::Host::new_item_amount(self, id, 1)
    }
    fn item_id(&mut self, item: &usize) -> Result<i32> {
        Ok(self.items[*item].id)
    }
    fn item_amount(&mut self, item: &usize) -> Result<i32> {
        Ok(self.items[*item].amount)
    }
    fn server_name(&mut self) -> Result<Option<Text>> {
        self.emit("settings", vec![])?;
        Ok(Some("2009scape".encode_utf16().collect()))
    }
    fn log(&mut self, origin: &str, level: LogLevel, message: Text) -> Result<()> {
        let level = match level {
            LogLevel::Fine => "FINE",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Err => "ERR",
            LogLevel::Debug => "DEBUG",
        };
        self.emit("log", vec![json!(origin), json!(level), units(&message)])
    }
    fn stderr(&mut self, message: Text) -> Result<()> {
        self.emit("stderr", vec![units(&message)])
    }
    fn stack_trace(&mut self, error: &Error) -> Result<()> {
        self.emit("trace", vec![json!(error_name(error))])
    }
}

impl dsl::Host for Model {
    type Location = (i32, i32, i32);
    fn location(&mut self, _: &NodeHandle) -> Result<Option<(i32, i32, i32)>> {
        self.emit("location", vec![json!(0)])?;
        Ok((self.source["noLocation"] != json!(true)).then_some((3222, 3218, 0)))
    }
    fn face_location(&mut self, npc: &usize, l: &(i32, i32, i32)) -> Result<()> {
        self.emit("faceLocation", vec![json!(npc), json!([l.0, l.1, l.2])])
    }
    fn new_npc(&mut self, id: i32) -> Result<usize> {
        self.npcs.push(Npc { id });
        let oid = self.npcs.len() - 1;
        self.emit("newNpc", vec![json!(oid), json!(id)])?;
        Ok(oid)
    }
    fn new_item_amount(&mut self, id: i32, amount: i32) -> Result<usize> {
        self.items.push(Item {
            id,
            amount,
            name: Some(format!("Item {id}").encode_utf16().collect()),
        });
        let r = self.items.len() - 1;
        self.emit("newItem", vec![json!(r), json!(id), json!(amount)])?;
        Ok(r)
    }
    fn set_value(&mut self, _: &NodeHandle, key: &str, value: Value<Self>) -> Result<()> {
        let attr = match value {
            Value::Bool(b) => Attr::Json(json!(b)),
            Value::Int(v) => Attr::Json(json!(v)),
            Value::InputType(kind) => Attr::Json(json!(kind.to_string())),
            Value::Script(s) => Attr::Script(s),
        };
        let described = self.describe(&attr);
        self.emit("setAttribute", vec![json!(0), json!(key), described])?;
        self.attrs.insert(key.into(), attr);
        Ok(())
    }
    fn attribute_bool(&mut self, _: &NodeHandle, key: &str, default: bool) -> Result<bool> {
        self.emit("getAttribute", vec![json!(0), json!(key)])?;
        match self.attrs.get(key) {
            None => Ok(default),
            Some(Attr::Json(J::Bool(b))) => Ok(*b),
            Some(_) => Err(Error::Cast),
        }
    }
    fn set_close_event(&mut self, component: &usize, event: CloseEvent) -> Result<()> {
        self.emit("setCloseEvent", vec![json!(component)])?;
        self.comps[*component].close = Some(event);
        Ok(())
    }
    fn hook_dialogue_closed(&mut self, _: &NodeHandle, hook: CloseHook<Self>) -> Result<()> {
        self.emit("hook", vec![json!(0), json!("DialogueCloseEvent")])?;
        if !self.hooks.iter().any(|h| h.same(&hook)) {
            self.hooks.push(hook);
        }
        Ok(())
    }
    fn unhook(&mut self, _: &NodeHandle, hook: &CloseHook<Self>) -> Result<()> {
        self.emit("unhook", vec![json!(0)])?;
        if let Some(i) = self.hooks.iter().position(|h| h.same(hook)) {
            self.hooks.remove(i);
        }
        Ok(())
    }
    fn quest_stage(&mut self, _: &NodeHandle, quest: &str) -> Result<i32> {
        self.emit("questStage", vec![json!(0), json!(quest)])?;
        Ok(self.source["quests"][quest].as_i64().unwrap_or(0) as i32)
    }
    fn stdout(&mut self, message: Text) -> Result<()> {
        self.emit("stdout", vec![units(&message)])
    }
    fn debug(&mut self, _: &NodeHandle, message: Text) -> Result<()> {
        self.emit("debug", vec![json!(0), units(&message)])
    }
    fn item_name(&mut self, item: &usize) -> Result<Option<Text>> {
        self.emit("itemName", vec![json!(item)])?;
        Ok(self.items[*item].name.clone())
    }
    fn send_item_zoom_on_interface(
        &mut self,
        d: usize,
        item: i32,
        zoom: i32,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        self.emit(
            "itemZoom",
            vec![
                json!(d),
                json!(item),
                json!(zoom),
                json!(iface),
                json!(child),
            ],
        )
    }
    fn send_child_position(
        &mut self,
        _: &NodeHandle,
        iface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<()> {
        self.emit(
            "childPosition",
            vec![
                json!("RepositionChild"),
                json!(0),
                json!(iface),
                json!(child),
                json!(x),
                json!(y),
            ],
        )
    }
    fn inventory_amount(&mut self, _: &NodeHandle, item: &usize) -> Result<i32> {
        self.emit("invAmount", vec![json!(0), json!(item)])?;
        let id = self.items[*item].id;
        Ok(self.source["inventory"][id.to_string()]
            .as_i64()
            .unwrap_or(0) as i32)
    }
    fn dispatch_message(&mut self, d: usize, message: Text) -> Result<()> {
        self.emit("dmessage", vec![json!(d), units(&message)])
    }
    fn slayer_started(&mut self, _: &NodeHandle) -> Result<bool> {
        self.emit("slayerStarted", vec![json!(0)])?;
        Ok(self.flag("slayer"))
    }
    fn register_item_option(&mut self, item: i32, key: &str) -> Result<()> {
        self.emit("itemDef", vec![json!(item)])?;
        self.emit(
            "handlerPut",
            vec![json!(item), json!(key), json!("ActivateEnchantedGem")],
        )
    }
}

impl BankHost for Model {
    fn has_seal_of_passage(&mut self, _: &NodeHandle) -> Result<bool> {
        self.emit("seal", vec![json!(0)])?;
        Ok(self.flag("seal"))
    }
    fn has_ultimate_ironman_restriction(&mut self, _: &NodeHandle) -> Result<bool> {
        self.emit("ironman", vec![json!(0), json!("ULTIMATE")])?;
        Ok(self.flag("uim"))
    }
    fn has_awaiting_grand_exchange_collections(&mut self, _: &NodeHandle) -> Result<bool> {
        self.emit("geAwaiting", vec![json!(0)])?;
        Ok(self.flag("ge"))
    }
    fn open_bank_account(&mut self, _: &NodeHandle) -> Result<()> {
        self.emit("openBank", vec![json!(0)])
    }
    fn activate_secondary_bank_account(&mut self, _: &NodeHandle) -> Result<Activation> {
        self.emit("activate", vec![json!(0)])?;
        match self.bank.get("activation").and_then(J::as_str) {
            Some("ALREADY_ACTIVE") => Ok(Activation::AlreadyActive),
            Some("INTERNAL_FAILURE") => Ok(Activation::InternalFailure),
            Some("NOT_ENOUGH_MONEY") => Ok(Activation::NotEnoughMoney),
            Some("SUCCESS") => Ok(Activation::Success),
            // Enum.valueOf(null).
            None => Err(Error::Null),
            Some(other) => panic!("{other}"),
        }
    }
    fn toggle_bank_account(&mut self, _: &NodeHandle) -> Result<()> {
        self.emit("toggleBank", vec![json!(0)])?;
        let now = !self.flag("secondary");
        self.bank.insert("secondary".into(), json!(now));
        Ok(())
    }
    fn is_using_secondary_bank_account(&mut self, _: &NodeHandle) -> Result<bool> {
        self.emit("usingSecondary", vec![json!(0)])?;
        Ok(self.flag("secondary"))
    }
    fn has_activated_secondary_bank_account(&mut self, _: &NodeHandle) -> Result<bool> {
        self.emit("activated", vec![json!(0)])?;
        Ok(self.flag("activated"))
    }
    fn open_bank_pin_settings(&mut self, _: &NodeHandle) -> Result<()> {
        self.emit("pinSettings", vec![json!(0)])
    }
    fn open_grand_exchange_collection_box(&mut self, _: &NodeHandle) -> Result<()> {
        self.emit("collectionBox", vec![json!(0)])
    }
    fn remove_item(&mut self, _: &NodeHandle, item: &usize) -> Result<bool> {
        self.emit("removeItem", vec![json!(0), json!(item)])?;
        Ok(self.flag("remove"))
    }
    fn api_message(&mut self, _: &NodeHandle, message: Text) -> Result<()> {
        self.emit("apiMessage", vec![json!(0), units(&message)])
    }
    fn second_bank(&mut self) -> Result<bool> {
        Ok(self.flag("secondBank"))
    }
}

/// Replays one case; a failure case returns only its swept action's snapshot.
pub fn evaluate(row: &J) -> J {
    let mut model = Model::new(row);
    let mut out = Vec::new();
    for action in row["actions"].as_array().unwrap() {
        let mark = model.events.len();
        model.count = 0;
        model.fail_at = action["failAt"].as_u64().unwrap_or(0) as usize;
        let (result, error) = match model.act(action) {
            Ok(result) => (result, None),
            Err(error) => (J::Null, Some(error)),
        };
        model.fail_at = 0;
        out.push(model.snapshot(result, error.as_ref(), mark));
    }
    if row["onlyLast"] == json!(true) {
        out = out.split_off(out.len() - 1);
    }
    let _ = Lines::new(Vec::new());
    let _ = Entity::<Model>::Npc(0);
    json!(out)
}
