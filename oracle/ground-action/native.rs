//! Real manager/actor/chunk replay; interaction/content boundaries are recorded.
use super::*;
use rs09_server::world::{
    ground_action::{self as action, Context as ActionContext, Listener},
    ground_manager as gm,
    interaction::OptionHandle,
    interaction_options::{self as options, Constants, Handle as Interaction, Table},
};
pub(super) struct Extra {
    plugin: bool,
    ground: bool,
    item: bool,
    tables: [Interaction; 2],
    options: [OptionHandle; 3],
    receivers: BTreeMap<usize, Option<usize>>,
}
impl Default for Extra {
    fn default() -> Self {
        use rs09_server::world::interaction::OptionData;
        let take = Shared::new(OptionData::new(Some("Take".into()), 2));
        let use_option = Shared::new(OptionData::new(Some("Use".into()), 0));
        let tables = std::array::from_fn(|_| Table::new(None));
        for table in &tables {
            *table.read().options().write() = vec![None; 5];
        }
        tables[0].read().options().write()[2] = Some(take.clone());
        tables[1].read().options().write()[0] = Some(use_option.clone());
        Self {
            plugin: false,
            ground: false,
            item: false,
            tables,
            options: [take, use_option, Constants::default().null],
            receivers: BTreeMap::new(),
        }
    }
}
fn option_key(m: &Model, o: &OptionHandle) -> usize {
    m.ground_action
        .options
        .iter()
        .position(|v| v.same(o))
        .unwrap()
}
fn gm_error(e: Error) -> gm::Error {
    match e {
        Error::Null => gm::Error::Null,
        Error::Ground(e) => gm::Error::Ground(e),
        Error::Binding(s) => gm::Error::Binding(s),
    }
}
pub(super) fn act(m: &mut Model, a: &Value) -> Result<Value> {
    match a["op"].as_str().unwrap() {
        "groundProcess" => {
            let request = action::Request {
                player: m.p(a),
                id: n(a, "id", 1),
                x: n(a, "x", 3207),
                y: n(a, "y", 3216),
                option: n(a, "index", 2),
            };
            action::process(m, &request).map_err(manager::error)?;
        }
        "groundActionSet" => {
            if let Some(v) = a.get("plugin") {
                m.ground_action.plugin = *v == true;
            }
            if let Some(v) = a.get("ground") {
                m.ground_action.ground = *v == true;
            }
            if let Some(v) = a.get("item") {
                m.ground_action.item = *v == true;
            }
            if let Some(v) = a.get("interaction") {
                let g = m.g(a)?;
                m.ground_action
                    .receivers
                    .insert(g.identity(), v.as_u64().map(|v| v as usize));
            }
            if let Some(v) = a.get("options") {
                let values = v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        v.as_u64()
                            .map(|v| m.ground_action.options[v as usize].clone())
                    })
                    .collect();
                *m.ground_action.tables[n(a, "table", 0) as usize]
                    .read()
                    .options()
                    .write() = values;
            }
        }
        _ => return Err(Error::Binding("unregistered ground action".into())),
    }
    Ok(Value::Null)
}
pub(super) fn snapshot(m: &Model, s: &mut Value) {
    let e = &m.ground_action;
    let tables = e
        .tables
        .iter()
        .map(|t| {
            t.read()
                .options()
                .read()
                .iter()
                .map(|o| o.as_ref().map(|o| option_key(m, o)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let receivers = m
        .grounds
        .iter()
        .map(|g| e.receivers.get(&g.identity()).copied().unwrap_or(Some(0)))
        .collect::<Vec<_>>();
    s["groundAction"] = json!([e.plugin, e.ground, e.item, tables, receivers]);
}
impl action::Lookup for Model {
    fn action_location(&mut self, player: &NodeHandle) -> gm::Result<Option<Location>> {
        gm::Context::player_location(self, player)
    }
    fn lookup(
        &mut self,
        id: i32,
        location: Option<Location>,
        player: Option<NodeHandle>,
    ) -> gm::Result<Option<Ground>> {
        gm::get(self, id, location, player)
    }
}
impl ActionContext for Model {
    type Interaction = usize;
    fn reset_scripts(&mut self, player: &NodeHandle) -> gm::Result<()> {
        let p = self.pi(player);
        self.emit("scripts", vec![json!(p)]).map_err(gm_error)
    }
    fn clear_minimap(&mut self, player: &NodeHandle) -> gm::Result<()> {
        let p = self.pi(player);
        self.emit("minimap", vec![json!(p)]).map_err(gm_error)
    }
    fn interaction(&mut self, item: &Ground) -> gm::Result<Option<usize>> {
        self.emit("interaction", vec![json!(self.gid(item))])
            .map_err(gm_error)?;
        Ok(self
            .ground_action
            .receivers
            .get(&item.identity())
            .copied()
            .unwrap_or(Some(0)))
    }
    fn option(&mut self, interaction: &usize, index: i32) -> gm::Result<Option<OptionHandle>> {
        self.emit("option", vec![json!(interaction), json!(index)])
            .map_err(gm_error)?;
        options::get(&self.ground_action.tables[*interaction], index).map_err(|e| match e {
            options::Error::Null => gm::Error::Null,
            options::Error::Index => gm::Error::Index,
            e => gm::Error::Binding(format!("{e:?}")),
        })
    }
    fn invalid(&mut self, _: &usize, player: &NodeHandle, item: &Ground) -> gm::Result<()> {
        self.emit(
            "invalidGround",
            vec![json!(self.pi(player)), json!(self.gid(item)), json!(2)],
        )
        .map_err(gm_error)
    }
    fn plugin_interaction(
        &mut self,
        player: &NodeHandle,
        item: &Ground,
        option: &OptionHandle,
    ) -> gm::Result<bool> {
        let value = self.ground_action.plugin;
        self.emit(
            "pluginInteraction",
            vec![
                json!(self.pi(player)),
                json!(self.gid(item)),
                json!(option_key(self, option)),
            ],
        )
        .map_err(gm_error)?;
        Ok(value)
    }
    fn listeners(
        &mut self,
        id: i32,
        kind: Listener,
        name: Option<String>,
        player: &NodeHandle,
        item: &Ground,
    ) -> gm::Result<bool> {
        let (event, value) = match kind {
            Listener::GroundItem => ("groundListener", self.ground_action.ground),
            Listener::Item => ("itemListener", self.ground_action.item),
        };
        self.emit(
            event,
            vec![
                json!(id),
                json!(name),
                json!(self.pi(player)),
                json!(self.gid(item)),
            ],
        )
        .map_err(gm_error)?;
        Ok(value)
    }
    fn handle(
        &mut self,
        interaction: &usize,
        player: &NodeHandle,
        option: &OptionHandle,
    ) -> gm::Result<()> {
        self.emit(
            "handleGround",
            vec![
                json!(interaction),
                json!(self.pi(player)),
                json!(option_key(self, option)),
            ],
        )
        .map_err(gm_error)
    }
}
