//! Native side of the unchanged Settings oracle. Owning-world services only.
use rs09_game_core::{java_double, java_json};
use rs09_protocol::game::Frame;
use rs09_server::player::settings::{ContainerKind, Context, Error, Result, Settings};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
fn bits(value: f64) -> String {
    format!(
        "{:016x}",
        if value.is_nan() {
            f64::NAN.to_bits()
        } else {
            value.to_bits()
        }
    )
}
fn integer(v: &Value) -> i32 {
    v.as_i64().unwrap_or(0) as i32
}
fn decimal(v: &Value) -> Result<f64> {
    java_double::parse(
        &v.as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| v.to_string()),
    )
    .ok_or(Error::Number)
}
struct Env {
    config: Value,
    trace: Vec<Value>,
    counts: BTreeMap<String, i32>,
    fired: BTreeSet<usize>,
    retaliating: bool,
    containers: BTreeMap<usize, Vec<Option<usize>>>,
}
impl Env {
    fn new(config: Value) -> Self {
        Self {
            retaliating: config["retaliating"] == true,
            config,
            trace: vec![],
            counts: BTreeMap::new(),
            fired: BTreeSet::new(),
            containers: BTreeMap::from([(0, vec![None; 28]), (1, vec![None; 14])]),
        }
    }
    fn flag(&self, key: &str) -> bool {
        self.config[key] == true
    }
    fn emit(&mut self, state: &Settings, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.trace.push(json!(row));
        let count = self.counts.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.config["hooks"].as_array().cloned().unwrap_or_default();
        for (index, hook) in hooks.iter().enumerate() {
            if !self.fired.contains(&index)
                && hook["event"] == event
                && hook["nth"].as_i64().unwrap_or(1) == i64::from(count)
            {
                self.fired.insert(index);
                for action in hook["actions"].as_array().unwrap() {
                    self.act(state, action)?;
                }
            }
        }
        let failure = &self.config["failure"];
        if failure["event"] == event && failure["nth"].as_i64().unwrap_or(1) == i64::from(count) {
            return Err(Error::Binding);
        }
        Ok(())
    }
    fn fields(&mut self, state: &Settings, values: &Value) -> Result<()> {
        let mut f = state.fields.write();
        macro_rules! ints {($($java:literal=>$rust:ident),*$(,)?)=>{$(if let Some(v)=values.get($java){f.$rust=integer(v);})*};}
        macro_rules! bools {($($java:literal=>$rust:ident),*$(,)?)=>{$(if let Some(v)=values.get($java){f.$rust=v.as_bool().ok_or(Error::Type)?;})*};}
        ints!("brightness"=>brightness,"musicVolume"=>music_volume,"soundEffectVolume"=>sound_effect_volume,"areaSoundVolume"=>area_sound_volume,"publicChatSetting"=>public_chat_setting,"privateChatSetting"=>private_chat_setting,"clanChatSetting"=>clan_chat_setting,"tradeSetting"=>trade_setting,"assistSetting"=>assist_setting,"specialEnergy"=>special_energy,"attackStyleIndex"=>attack_style_index);
        bools!("singleMouseButton"=>single_mouse_button,"disableChatEffects"=>disable_chat_effects,"splitPrivateChat"=>split_private_chat,"acceptAid"=>accept_aid,"runToggled"=>run_toggled,"specialToggled"=>special_toggled);
        if let Some(v) = values.get("runEnergy") {
            f.run_energy = decimal(v)?;
        }
        if let Some(v) = values.get("weight") {
            f.weight = decimal(v)?;
        }
        Ok(())
    }
    fn act(&mut self, s: &Settings, a: &Value) -> Result<Value> {
        let value = &a["value"];
        let v = integer(value);
        match a["op"].as_str().unwrap() {
            "update" => s.update(self)?,
            "chat" => s.update_chat_settings(self)?,
            "chatValues" => s.update_chat_values(
                self,
                integer(&a["public"]),
                integer(&a["private"]),
                integer(&a["trade"]),
            )?,
            "setChat" => s.set_chat_settings(
                integer(&a["public"]),
                integer(&a["private"]),
                integer(&a["trade"]),
            ),
            "parse" => s.parse(self, &java_json::parse(&a["data"].to_string()).unwrap())?,
            "brightness" => s.set_brightness(self, v)?,
            "music" => s.set_music_volume(self, v)?,
            "sound" => s.set_sound_effect_volume(self, v)?,
            "area" => s.set_area_sound_volume(self, v)?,
            "public" => s.set_public_chat_setting(self, v)?,
            "private" => s.set_private_chat_setting(self, v)?,
            "trade" => s.set_trade_setting(self, v)?,
            "clan" => s.set_clan_chat_setting(self, v)?,
            "assist" => s.set_assist_setting(self, v)?,
            "attackToggle" => s.toggle_attack_style_index(self, v)?,
            "attackSet" => s.fields.write().attack_style_index = v,
            "specialToggle" => s.toggle_special_bar(self)?,
            "specialSet" => s.set_special_toggled(self, value.as_bool().unwrap())?,
            "specialDrain" => return Ok(json!(s.drain_special(self, v)?)),
            "specialEnergy" => s.set_special_energy(self, v)?,
            "retaliate" => s.toggle_retaliating(self)?,
            "mouse" => s.toggle_mouse_button(self)?,
            "effects" => s.toggle_chat_effects(self)?,
            "split" => s.toggle_split_private_chat(self)?,
            "aid" => s.toggle_accept_aid(self)?,
            "acceptAid" => return Ok(json!(s.accept_aid(self)?)),
            "runToggle" => s.toggle_run(self)?,
            "runSet" => s.set_run_toggled(self, value.as_bool().unwrap())?,
            "energy" => s.update_run_energy(self, decimal(value)?)?,
            "energySet" => s.fields.write().run_energy = decimal(value)?,
            "weight" => s.update_weight(self)?,
            "state" => self.fields(s, &a["fields"])?,
            "configure" => {
                for (k, v) in a["fields"].as_object().unwrap() {
                    self.config
                        .as_object_mut()
                        .unwrap()
                        .insert(k.clone(), v.clone());
                }
            }
            "retaliation" => self.retaliating = value.as_bool().unwrap(),
            "slots" => {
                self.containers.insert(
                    integer(&a["id"]) as usize,
                    a["slots"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| {
                            if v.is_null() {
                                None
                            } else {
                                Some(integer(v) as usize)
                            }
                        })
                        .collect(),
                );
            }
            op => panic!("Unknown oracle action {op}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, s: &Settings) -> Value {
        let f = s.fields.read();
        json!({"fields":{"runEnergy":bits(f.run_energy),"weight":bits(f.weight),"brightness":f.brightness,"musicVolume":f.music_volume,"soundEffectVolume":f.sound_effect_volume,"areaSoundVolume":f.area_sound_volume,"singleMouseButton":f.single_mouse_button,"disableChatEffects":f.disable_chat_effects,"splitPrivateChat":f.split_private_chat,"acceptAid":f.accept_aid,"runToggled":f.run_toggled,"publicChatSetting":f.public_chat_setting,"privateChatSetting":f.private_chat_setting,"clanChatSetting":f.clan_chat_setting,"tradeSetting":f.trade_setting,"assistSetting":f.assist_setting,"specialToggled":f.special_toggled,"specialEnergy":f.special_energy,"attackStyleIndex":f.attack_style_index},"retaliating":self.retaliating,"trace":self.trace})
    }
}
impl Context for Env {
    fn varp(&mut self, s: &Settings, id: i32, value: i32) -> Result<()> {
        self.emit(s, "varp", vec![json!(id), json!(value)])
    }
    fn session(&mut self, s: &Settings) -> Result<Option<usize>> {
        self.emit(s, "session", vec![])?;
        Ok(if self.flag("sessionNull") {
            None
        } else {
            Some(0)
        })
    }
    fn send(&mut self, s: &Settings, session: Option<usize>, frame: Frame) -> Result<()> {
        session.ok_or(Error::Null)?;
        self.emit(s, "send", vec![json!(frame.opcode), json!(frame.payload)])
    }
    fn dispatcher(&mut self, s: &Settings) -> Result<Option<usize>> {
        self.emit(s, "dispatcher", vec![])?;
        Ok(if self.flag("dispatcherNull") {
            None
        } else {
            Some(0)
        })
    }
    fn run_energy(&mut self, s: &Settings, dispatcher: Option<usize>) -> Result<()> {
        dispatcher.ok_or(Error::Null)?;
        let energy = bits(s.fields.read().run_energy);
        self.emit(s, "energy", vec![json!(energy)])
    }
    fn message(&mut self, s: &Settings, dispatcher: Option<usize>, text: &str) -> Result<()> {
        dispatcher.ok_or(Error::Null)?;
        self.emit(s, "message", vec![json!(text)])
    }
    fn string(
        &mut self,
        s: &Settings,
        dispatcher: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> Result<()> {
        dispatcher.ok_or(Error::Null)?;
        self.emit(
            s,
            "string",
            vec![json!(text), json!(interface), json!(child)],
        )
    }
    fn properties(&mut self, state: &Settings) -> Result<Option<usize>> {
        self.emit(state, "properties", vec![])?;
        Ok(if self.flag("propertiesNull") {
            None
        } else {
            Some(0)
        })
    }
    fn retaliating(&mut self, s: &Settings, properties: Option<usize>) -> Result<bool> {
        properties.ok_or(Error::Null)?;
        self.emit(s, "retaliating", vec![])?;
        Ok(self.retaliating)
    }
    fn set_retaliating(
        &mut self,
        s: &Settings,
        properties: Option<usize>,
        value: bool,
    ) -> Result<()> {
        properties.ok_or(Error::Null)?;
        self.emit(s, "setRetaliating", vec![json!(value)])?;
        self.retaliating = value;
        Ok(())
    }
    fn ironman(&mut self, s: &Settings) -> Result<bool> {
        self.emit(s, "ironmanManager", vec![])?;
        if self.flag("ironmanNull") {
            return Err(Error::Null);
        }
        self.emit(s, "ironman", vec![])?;
        Ok(self.flag("ironman"))
    }
    fn container(&mut self, s: &Settings, kind: ContainerKind) -> Result<usize> {
        let (key, fallback) = match kind {
            ContainerKind::Inventory => ("inventory", 0),
            ContainerKind::Equipment => ("equipment", 1),
        };
        self.emit(s, key, vec![])?;
        Ok(self.config[format!("{key}Id")].as_i64().unwrap_or(fallback) as usize)
    }
    fn item(&mut self, s: &Settings, container: usize, slot: i32) -> Result<Option<usize>> {
        if !self.containers.contains_key(&container) {
            return Err(Error::Null);
        }
        self.emit(s, "item", vec![json!(container as i32), json!(slot)])?;
        self.containers[&container]
            .get(slot as usize)
            .copied()
            .ok_or(Error::Index)
    }
    fn weight(&mut self, s: &Settings, item: usize) -> Result<f64> {
        let id = item as i32;
        self.emit(s, "definition", vec![json!(id)])?;
        self.emit(s, "weight", vec![json!(id)])?;
        let Some(v) = self.config["weights"].get(id.to_string()) else {
            return Ok(0.0);
        };
        if v.is_null() {
            return Err(Error::Null);
        }
        match v["kind"].as_str().unwrap_or("double") {
            "double" => decimal(&v["value"]),
            _ => Err(Error::Type),
        }
    }
}
pub fn run(request: &Value) -> Value {
    let mut env = Env::new(request["config"].clone());
    let state = Settings::default();
    let mut results = vec![];
    for action in request["actions"].as_array().unwrap() {
        let result = env.act(&state, action);
        let snapshot = env.snapshot(&state);
        results.push(match result {
            Ok(v) => json!({"value":v,"state":snapshot}),
            Err(e) => json!({"error":format!("{e:?}"),"state":snapshot}),
        });
    }
    json!({"request":request,"results":results})
}
