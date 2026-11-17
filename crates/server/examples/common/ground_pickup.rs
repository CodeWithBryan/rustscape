//! Controlled live Pickup bindings. Full content/dialogue/bank/audit startup is
//! still explicit; no missing restriction service returns a fabricated result.
use super::*;
use rs09_protocol::game::{actions::ClientAction, updates};
use rs09_server::{
    inventory::item as inventory_item,
    world::{
        animation::Animation,
        ground_action::{self, Listener},
        ground_item, ground_manager as manager, ground_runtime,
        interaction::OptionHandle,
        legacy_interaction as legacy, pickup as take, pickup_runtime,
    },
};
impl pickup_runtime::Services<scene::CacheAssets> for Services<'_> {
    fn player_frames(&mut self) -> &mut BTreeMap<Node, Vec<Frame>> {
        self.frames
    }
    fn player_clock(&self) -> fn() -> i64 {
        now
    }
    fn pickup_has_item(
        &mut self,
        world: &mut Parts<'_, scene::CacheAssets>,
        player: &NodeHandle,
        item: &inventory_item::Handle,
    ) -> Result<bool> {
        world
            .actors
            .player_fields(player)
            .map_err(error)?
            .has_item(Some(item))
            .map_err(|e| match e {
                rs09_server::inventory::Error::Null => manager::Error::Null,
                rs09_server::inventory::Error::Type => manager::Error::Type,
                e => error(format!("{e:?}")),
            })
    }
    fn pickup_plugin(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: (),
        _: &NodeHandle,
        _: &Handle,
        _: i32,
    ) -> Result<bool> {
        Err(error("item pickup plugin is not bound"))
    }
    fn packet_message(
        &mut self,
        w: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
        s: &str,
    ) -> Result<()> {
        w.actors.player_fields(p).map_err(error)?;
        let mut payload = s.as_bytes().to_vec();
        payload.push(0);
        self.frames.entry(p.node).or_default().push(Frame {
            opcode: 70,
            payload,
        });
        Ok(())
    }
    fn player_message(
        &mut self,
        w: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
        s: &str,
    ) -> Result<()> {
        pickup_runtime::Services::packet_message(self, w, p, s)
    }
    fn player_messages(
        &mut self,
        w: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
        text: &[&str],
    ) -> Result<()> {
        for s in text {
            pickup_runtime::Services::player_message(self, w, p, s)?;
        }
        Ok(())
    }
    fn pickup_guild_dialogue(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: &NodeHandle,
    ) -> Result<()> {
        Err(error("guild dialogue is not bound"))
    }
    fn pickup_player_name(
        &mut self,
        w: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
    ) -> Result<Option<String>> {
        gw::Services::username(self, w, p)
    }
    fn pickup_log(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: Option<NodeHandle>,
        _: &str,
    ) -> Result<()> {
        Err(error("drop trade audit logger is not bound"))
    }
    fn pickup_animate(
        &mut self,
        w: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
        id: i32,
    ) -> Result<()> {
        w.actors.player_fields(p).map_err(error)?;
        let animator = self.animators.get(&p.node).ok_or(Error::Null)?.clone();
        self.animations.ticks = w.actors.ticks;
        animator
            .animate(self.animations, Some(Animation::new(id)), None)
            .map_err(error)?;
        Ok(())
    }
    fn pickup_audio(
        &mut self,
        w: &mut Parts<'_, scene::CacheAssets>,
        p: &NodeHandle,
        id: i32,
    ) -> Result<()> {
        w.actors.player_fields(p).map_err(error)?;
        self.frames
            .entry(p.node)
            .or_default()
            .push(updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn pickup_event(
        &mut self,
        _: &mut Parts<'_, scene::CacheAssets>,
        _: &NodeHandle,
        _: i32,
    ) -> Result<()> {
        // The controlled endpoint, like its existing interaction/zone services,
        // declares an empty content event registry. Full content startup is open.
        Ok(())
    }
}
pub(in super::super) fn program(host: &Host, id: usize) -> legacy::Result<Option<take::Program>> {
    let Some(ground) = &host.ground else {
        return Ok(None);
    };
    let Ok(handler) = ground.options.handler(id) else {
        return Ok(None);
    };
    let program = handler.read().value::<take::Program>().copied();
    program
        .map(Some)
        .ok_or_else(|| legacy::Error::Binding(Some("unbound native ground handler".into())))
}
pub(in super::super) fn process(
    state: &mut State,
    host: &mut Host,
    node: Node,
    action: &ClientAction,
) -> Result<bool> {
    if host.ground.is_none() || !matches!(action, ClientAction::GroundItemAction { .. }) {
        return Ok(false);
    }
    let player = state.retain_node(node).map_err(error)?;
    let request =
        ground_action::Request::from_action(player, action).expect("matched ground action");
    ground_runtime::process(state, host, &request)?;
    Ok(true)
}
impl ground_runtime::Bindings for Host {
    fn ground_lookup(
        &mut self,
        s: &mut State,
        id: i32,
        l: Option<Location>,
        p: Option<NodeHandle>,
    ) -> Result<Option<Handle>> {
        world(s, self, |w| manager::get(w, id, l, p))
    }
    fn ground_interaction(
        &mut self,
        s: &mut State,
        h: &Handle,
    ) -> Result<ground_options::Captured> {
        let g = self.ground.as_ref().ok_or(Error::Null)?;
        g.options.capture(s, &g.items.spawns, h).map_err(error)
    }
    fn ground_plugin(
        &mut self,
        _: &mut State,
        _: &NodeHandle,
        _: &Handle,
        _: &OptionHandle,
    ) -> Result<bool> {
        Ok(false)
    }
    fn ground_listeners(
        &mut self,
        _: &mut State,
        _: i32,
        _: Listener,
        _: Option<String>,
        _: &NodeHandle,
        _: &Handle,
    ) -> Result<bool> {
        Ok(false)
    }
}
fn inventory(state: &State, p: &NodeHandle) -> Result<Vec<Option<[i32; 3]>>> {
    Ok(state
        .player_fields(p)
        .map_err(error)?
        .inventory()
        .items()
        .read()
        .iter()
        .map(|i| {
            i.as_ref().map(|i| {
                let i = i.read();
                [i.id(), i.amount(), i.charge()]
            })
        })
        .collect())
}
pub(in super::super) fn pickup(
    state: &mut State,
    host: &mut Host,
    program: take::Program,
    p: Option<Node>,
    target: Option<Node>,
    option: &str,
) -> Result<bool> {
    let p = state.retain_node(p.ok_or(Error::Null)?).map_err(error)?;
    let target = target.ok_or(Error::Null)?;
    let item = state.ground(target).map_err(error)?;
    let trace = host
        .ground
        .as_ref()
        .is_some_and(|g| g.pickup_trace.is_some());
    let before = trace.then(|| inventory(state, &p)).transpose()?;
    let weight_before = trace
        .then(|| {
            state
                .player_fields(&p)
                .map(|p| p.settings().fields.read().weight)
        })
        .transpose()
        .map_err(error)?;
    let frame_start = host.frames.get(&p.node).map_or(0, Vec::len);
    let result = world(state, host, |w| {
        let manager = w.world.items.manager.clone();
        program.handle(w, &manager, Some(&p), Some(&item))
    });
    let after = trace.then(|| inventory(state, &p)).transpose()?;
    let weight_after = trace
        .then(|| {
            state
                .player_fields(&p)
                .map(|p| p.settings().fields.read().weight)
        })
        .transpose()
        .map_err(error)?;
    if trace {
        use std::io::Write;
        let frames:Vec<_>=host.frames.get(&p.node).into_iter().flatten().skip(frame_start).map(|f|serde_json::json!({"opcode":f.opcode,"payload":f.payload.iter().map(|b|format!("{b:02x}")).collect::<String>()})).collect();
        let present = host
            .ground
            .as_ref()
            .unwrap()
            .items
            .manager
            .get_items()
            .read()
            .iter()
            .flatten()
            .any(|h| h.same(&item));
        let value = ground_item::as_item(&item);
        let value = value.read();
        let row = serde_json::json!({"event":"pickup","tick":state.ticks,"player":p.node,"target":target,"id":value.id(),"amount":value.amount(),"option":option,"handled":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:?}")),"inventoryBefore":before,"inventoryAfter":after,"weightBefore":weight_before,"weightAfter":weight_after,"groundPresentAfter":present,"frames":frames});
        drop(value);
        writeln!(
            host.ground.as_mut().unwrap().pickup_trace.as_mut().unwrap(),
            "{row}"
        )
        .map_err(error)?;
    }
    result
}
