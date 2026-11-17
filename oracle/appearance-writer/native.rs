//! Recording receivers around the production late writer and actual Appearance.
#[allow(dead_code)]
#[path = "../player-appearance/native.rs"]
mod base;
use rs09_server::{
    definitions::property::Property,
    player::{
        appearance::{self as app, Gender},
        appearance_writer::{self as writer, Buffer, Input},
    },
    world::player_walk::NodeHandle,
};
use serde_json::{Value, json};
fn n(v: &Value, k: &str, d: i32) -> i32 {
    v[k].as_i64().map_or(d, |n| n as i32)
}
struct Wire {
    state: Value,
    buffer: Buffer,
}
fn wire(c: &base::Context) -> &Wire {
    c.extension.as_ref().unwrap().downcast_ref().unwrap()
}
fn wire_mut(c: &mut base::Context) -> &mut Wire {
    c.extension.as_mut().unwrap().downcast_mut().unwrap()
}
fn initial(w: &Value) -> Value {
    let mut v = json!({"world":[],"skull":[],"properties":[],"familiar":[],"skills":[],"active":{"world":0,"skull":0,"properties":0,"familiar":0,"skills":0},"username":w.get("username").cloned().unwrap_or(json!("self"))});
    for i in 0..2 {
        let b = |key: &str| {
            if i == 0 {
                w[key] == true
            } else {
                w[key] != true
            }
        };
        v["world"]
            .as_array_mut()
            .unwrap()
            .push(json!({"pvp":b("pvp"),"wild":b("wild")}));
        v["skull"].as_array_mut().unwrap().push(json!({"wilderness":b("wilderness"),"disabled":b("disabled"),"level":n(w,"level",17).wrapping_add(i*13)}));
        v["properties"].as_array_mut().unwrap().push(json!({"combat":n(w,"combat",51).wrapping_add(i*13),"current":n(w,"current",57).wrapping_add(i*13)}));
        v["familiar"]
            .as_array_mut()
            .unwrap()
            .push(json!({"using":b("using"),"summoning":n(w,"summoning",6).wrapping_add(i*13)}));
        v["skills"]
            .as_array_mut()
            .unwrap()
            .push(json!({"total":n(w,"total",2001).wrapping_add(i*13)}));
    }
    v
}
fn hook(c: &mut base::Context, key: &str) -> app::Result<()> {
    let hooks = c.row["wireHooks"].as_array().unwrap().clone();
    let at = c.hits[key];
    for h in hooks {
        if h["when"] != key || n(&h, "at", 1) != at {
            continue;
        }
        match h["action"].as_str().unwrap() {
            "fail" => return Err(app::Error::Binding("controlled".into())),
            "redirect" => c.players[0].appearance = "B".into(),
            "nullAppearance" => c.players[0].appearance.clear(),
            "size" => c.players[0].size = n(&h, "value", 0),
            "color" => {
                c.apps["A"].parts().read()[n(&h, "index", 0) as usize]
                    .as_ref()
                    .ok_or(app::Error::Null)?
                    .write()
                    .color = n(&h, "value", 0)
            }
            "override" => {
                c.override_ = serde_json::from_value::<Property>(h["value"].clone()).unwrap()
            }
            "buffer" => wire(c).buffer.p1(n(&h, "value", 0)),
            "position" => wire(c).buffer.set_position(n(&h, "value", 0) as usize)?,
            "reenter" => {
                let b = wire(c).buffer.clone();
                let p = c.players[0].owner.clone();
                writer::write_to(c, Some(&b), Input::Player(&p))?;
            }
            "active" => {
                wire_mut(c).state["active"][h["family"].as_str().unwrap()] = h["value"].clone()
            }
            "set" => {
                wire_mut(c).state[h["family"].as_str().unwrap()][n(&h, "slot", 0) as usize]
                    [h["field"].as_str().unwrap()] = h["value"].clone()
            }
            v => panic!("unknown writer hook {v}"),
        }
    }
    Ok(())
}
fn receiver(
    c: &mut base::Context,
    p: &NodeHandle,
    family: &str,
    key: &str,
) -> app::Result<Option<usize>> {
    let p = c.owner(p)?;
    c.hit(key, vec![json!(p)])?;
    let slot = if p == 0 {
        n(&wire(c).state["active"], family, 0)
    } else {
        0
    };
    Ok((slot >= 0).then_some(p * 10 + slot as usize))
}
fn value(
    c: &mut base::Context,
    receiver: Option<usize>,
    family: &str,
    field: &str,
    key: &str,
) -> app::Result<Value> {
    let r = receiver.ok_or(app::Error::Null)?;
    let slot = r % 10;
    c.hit(key, vec![json!(slot)])?;
    if r >= 10 {
        return Ok(match field {
            "pvp" | "wild" | "wilderness" | "disabled" | "using" => json!(false),
            "level" => json!(17),
            "combat" => json!(51),
            "current" => json!(57),
            "summoning" => json!(6),
            "total" => json!(2001),
            _ => unreachable!(),
        });
    }
    Ok(wire(c).state[family][slot][field].clone())
}
macro_rules! boolean {
    ($name:ident,$receiver:ty,$family:literal,$field:literal,$key:literal) => {
        fn $name(&mut self, r: $receiver) -> app::Result<bool> {
            Ok(value(self, r, $family, $field, $key)?.as_bool().unwrap())
        }
    };
}
macro_rules! integer {
    ($name:ident,$family:literal,$field:literal,$key:literal) => {
        fn $name(&mut self, r: Option<usize>) -> app::Result<i32> {
            Ok(value(self, r, $family, $field, $key)?.as_i64().unwrap() as i32)
        }
    };
}
impl writer::Context for base::Context {
    type Skull = Option<usize>;
    type WorldSettings = Option<usize>;
    type Familiar = Option<usize>;
    type Skills = Option<usize>;
    fn skull(&mut self, p: &NodeHandle) -> app::Result<Self::Skull> {
        receiver(self, p, "skull", "skull")
    }
    boolean!(
        wilderness,
        Option<usize>,
        "skull",
        "wilderness",
        "wilderness"
    );
    boolean!(
        wilderness_disabled,
        Option<usize>,
        "skull",
        "disabled",
        "disabled"
    );
    integer!(wilderness_level, "skull", "level", "level");
    fn player_size(&mut self, p: &NodeHandle) -> app::Result<i32> {
        let p = self.owner(p)?;
        self.hit("bodySize", vec![json!(p)])?;
        Ok(self.players[p].size)
    }
    fn username(&mut self, p: &NodeHandle) -> app::Result<String> {
        let p = self.owner(p)?;
        self.hit("username", vec![json!(p)])?;
        if p == 0 {
            wire(self).state["username"]
                .as_str()
                .map(String::from)
                .ok_or(app::Error::Null)
        } else {
            Ok("self".into())
        }
    }
    fn world_settings(&mut self) -> app::Result<Self::WorldSettings> {
        self.hit("world", vec![])?;
        let slot = n(&wire(self).state["active"], "world", 0);
        Ok((slot >= 0).then_some(slot as usize))
    }
    boolean!(pvp, Option<usize>, "world", "pvp", "pvp");
    boolean!(wild_pvp, Option<usize>, "world", "wild", "wild");
    fn familiar(&mut self, p: &NodeHandle) -> app::Result<Self::Familiar> {
        receiver(self, p, "familiar", "familiar")
    }
    boolean!(using_summoning, Option<usize>, "familiar", "using", "using");
    integer!(summoning_combat_level, "familiar", "summoning", "summoning");
    fn combat_level(&mut self, r: usize) -> app::Result<i32> {
        Ok(
            value(self, Some(r - 100), "properties", "combat", "combatLevel")?
                .as_i64()
                .unwrap() as i32,
        )
    }
    fn current_combat_level(&mut self, r: usize) -> app::Result<i32> {
        Ok(
            value(self, Some(r - 100), "properties", "current", "current")?
                .as_i64()
                .unwrap() as i32,
        )
    }
    fn skills(&mut self, p: &NodeHandle) -> app::Result<Self::Skills> {
        receiver(self, p, "skills", "skills")
    }
    integer!(total_level, "skills", "total", "total");
    fn invalid_type(&mut self, kind: Option<&str>) -> app::Result<()> {
        self.hit("invalidType",vec![json!(format!("Invalid context of type {} passed to Appearance flag which expects core.game.node.entity.player.Player.",kind.unwrap_or("null")))])
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut c = base::setup(row);
    let w = &row["wire"];
    let a = c.apps["A"].clone();
    if w.get("gender").is_some() {
        let _ = a.set_gender(if w["gender"].is_null() {
            None
        } else {
            Some(if n(w, "gender", 0) == 0 {
                Gender::Male
            } else {
                Gender::Female
            })
        });
    }
    if n(w, "npc", -1) != -1 {
        a.transform_npc(&mut c, n(w, "npc", -1)).unwrap();
    }
    if w.get("size").is_some() {
        c.players[0].size = n(w, "size", 1);
    }
    if let Some(colors) = w["colors"].as_array() {
        for (i, v) in colors.iter().enumerate() {
            a.parts().read()[i].as_ref().unwrap().write().color = v.as_i64().unwrap() as i32;
        }
    }
    if let Some(body) = w["body"].as_array() {
        for (i, v) in body.iter().enumerate() {
            a.body().write()[i] = v.as_i64().unwrap() as i32;
        }
    }
    if w.get("nullPart").is_some() {
        a.parts().write()[n(w, "nullPart", 0) as usize] = None;
    }
    if w.get("render").is_some() {
        a.set_render_animation(
            &mut c,
            Some(&rs09_server::world::animation::Animation::new(n(
                w, "render", 1426,
            ))),
        )
        .unwrap();
    }
    a.set_skull_icon(n(w, "skull", -1));
    a.set_head_icon(n(w, "head", -1));
    c.players[0].prepared[0] = w["prepared"] == true;
    let buffer = Buffer::from_bytes(
        row["prefix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect(),
    );
    c.extension = Some(Box::new(Wire {
        state: initial(w),
        buffer: buffer.clone(),
    }));
    c.extension_hit = Some(hook);
    c.properties_lookup = Some(|c, p| {
        let slot = if p == 0 {
            n(&wire(c).state["active"], "properties", 0)
        } else {
            0
        };
        if slot < 0 {
            Err(app::Error::Null)
        } else {
            Ok(100 + p * 10 + slot as usize)
        }
    });
    c.trace.clear();
    c.hits.clear();
    let mut results = vec![];
    for w in row["writes"].as_array().unwrap() {
        let p = c.players[n(w, "player", 0) as usize].owner.clone();
        let input = match w["invalid"].as_str() {
            Some("null") => Input::Invalid(None),
            Some(v) => Input::Invalid(Some(v)),
            None => Input::Player(&p),
        };
        let result = writer::write_to(
            &mut c,
            if w["nullBuffer"] == true {
                None
            } else {
                Some(&buffer)
            },
            input,
        );
        let error = result.err().map(|e| match e {
            app::Error::Null => "Null",
            app::Error::Index => "Index",
            app::Error::Type => "Type",
            app::Error::Number => "Number",
            _ => "Binding",
        });
        let bytes = buffer
            .bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let state = c.snapshot();
        results.push(json!({"bytes":bytes,"position":buffer.position(),"error":error,"state":state,"wire":wire(&c).state}));
    }
    json!({"results":results,"trace":c.trace})
}
