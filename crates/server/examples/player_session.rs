//! Controlled stationary-player transport fixture for visual/headless integration.
//! Uses real native auth, original rebuild/renderer/appearance encoders and the
//! pinned cache. It does not replace profile/content/movement initialization.
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_protocol::{game::Frame, login::AuthResponse};
use rs09_server::{
    auth::{Authenticator, Mode, password::Passwords, storage::InMemoryStorage},
    catalog::Catalog,
    definitions::{animation::CacheSource, item::Shared},
    player::Prepared,
    runtime::{Config, Service},
    world::{
        animation::{Animation, Animator, Graphics},
        appearance,
        doors::Location,
        player_render::{self as render, Handle, Player},
        prepared_animation::{self, Owner},
        prepared_masks::PreparedMasks,
        rebuild,
        session::{self, Joined},
    },
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{mpsc, oneshot};
#[path = "common/player_spots.rs"]
mod player_spots;
fn millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
pub(crate) struct Join {
    pub(crate) prepared: Prepared,
    pub(crate) reply: oneshot::Sender<std::result::Result<Joined, AuthResponse>>,
}
pub(crate) struct Fixture {
    pub(crate) joins: mpsc::Sender<Join>,
}
impl session::World for Fixture {
    fn enter(&self, prepared: Prepared) -> session::Admission<'_> {
        Box::pin(async move {
            let (reply, receiver) = oneshot::channel();
            self.joins
                .send(Join { prepared, reply })
                .await
                .map_err(|_| AuthResponse::LoginServerOffline)?;
            receiver
                .await
                .unwrap_or(Err(AuthResponse::LoginServerOffline))
        })
    }
}
pub(crate) struct Actor {
    pub(crate) player: Handle,
    pub(crate) input: mpsc::Receiver<Frame>,
    pub(crate) output: mpsc::Sender<Vec<Frame>>,
    pub(crate) username: String,
    #[allow(dead_code)]
    pub(crate) chat_rights: u8,
    #[allow(dead_code)]
    pub(crate) muted: bool,
    pub(crate) masks: PreparedMasks,
    pub(crate) animator: Animator<Owner>,
}
impl Actor {
    pub(crate) fn new(
        player: Handle,
        appearance: appearance::Data,
        input: mpsc::Receiver<Frame>,
        output: mpsc::Sender<Vec<Frame>>,
        username: String,
        animations: &prepared_animation::Factory,
    ) -> Self {
        Self::with_clocks(
            player,
            appearance,
            input,
            output,
            username,
            animations,
            Shared::new(vec![0; 10]),
        )
    }
    pub(crate) fn with_clocks(
        player: Handle,
        appearance: appearance::Data,
        input: mpsc::Receiver<Frame>,
        output: mpsc::Sender<Vec<Frame>>,
        username: String,
        animations: &prepared_animation::Factory,
        clocks: Shared<Vec<i32>>,
    ) -> Self {
        let appearance = Shared::new(appearance);
        let masks = PreparedMasks::new(player.clone(), appearance);
        let animator = animations.actor_with_clocks(masks.clone(), clocks);
        masks
            .sync_appearance(millis)
            .expect("fixed player provider and clock");
        Self {
            player,
            input,
            output,
            username,
            chat_rights: 0,
            muted: false,
            masks,
            animator,
        }
    }
}
impl Drop for Actor {
    fn drop(&mut self) {
        // This private fixture exposes bytes, not player handles. Detach its
        // retired wire-cache edges so mutually local players cannot form Arc
        // cycles when the last connected actors leave in the same cycle.
        let info = {
            let mut p = self.player.write();
            p.active = false;
            p.info.clone()
        };
        info.read().local_players.write().clear();
    }
}
struct Renderer<'a> {
    actors: &'a BTreeMap<i32, Actor>,
    order: &'a [i32],
    keys: &'a HashMap<i32, [i32; 4]>,
    frames: Vec<Frame>,
}
impl render::Context for Renderer<'_> {
    fn candidates(&mut self, _: Location, _: i32) -> render::Result<Vec<Option<Handle>>> {
        Ok(self
            .order
            .iter()
            .filter_map(|i| self.actors.get(i))
            .map(|a| Some(a.player.clone()))
            .collect())
    }
    fn masks(
        &mut self,
        viewer: &Handle,
        p: &Handle,
        out: &mut Vec<u8>,
        appearance: bool,
        sync: bool,
    ) -> render::Result<()> {
        let a = self
            .actors
            .get(&p.read().index)
            .ok_or(render::Error::Null)?;
        if !a.player.same(p) {
            return Err(render::Error::Binding(
                "retired player cannot rebind by reused wire index".into(),
            ));
        }
        a.masks
            .write(viewer.clone(), out, appearance, sync)
            .map_err(|e| render::Error::Binding(e.to_string()))
    }
    fn send(&mut self, _: &Handle, f: Frame) -> render::Result<()> {
        self.frames.push(f);
        Ok(())
    }
}
impl rebuild::Context for Renderer<'_> {
    fn keys(&mut self, id: i32) -> render::Result<[i32; 4]> {
        Ok(self.keys.get(&id).copied().unwrap_or([0; 4]))
    }
    fn send(&mut self, _: &Handle, f: Frame) -> render::Result<()> {
        self.frames.push(f);
        Ok(())
    }
}
pub(crate) fn render(
    actors: &BTreeMap<i32, Actor>,
    order: &[i32],
    keys: &HashMap<i32, [i32; 4]>,
    player: &Handle,
    first: bool,
) -> render::Result<Vec<Frame>> {
    let mut ctx = Renderer {
        actors,
        order,
        keys,
        frames: vec![],
    };
    if first {
        rebuild::send(&mut ctx, player)?;
        return Ok(ctx.frames);
    }
    render::render(&mut ctx, player)?;
    Ok(ctx.frames)
}
async fn world(
    mut joins: mpsc::Receiver<Join>,
    animation_assets: CacheSource,
    keys: HashMap<i32, [i32; 4]>,
    animation_probe: Option<i32>,
    sound_probe: Option<i32>,
    spot_probe: Option<player_spots::Timeline>,
) {
    let mut animations = prepared_animation::World::new(animation_assets);
    let mut actors = BTreeMap::<i32, Actor>::new();
    let mut order = vec![];
    let mut ticker = tokio::time::interval(Duration::from_millis(600));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut ticks = 0u64;
    let mut spot_ages = BTreeMap::<i32, u64>::new();
    loop {
        tokio::select! {
            join=joins.recv()=>{
                let Some(Join{prepared,reply})=join else{return;};
                if reply.is_closed(){continue;}
                if actors.values().any(|a|a.username==prepared.login.username){let _=reply.send(Err(AuthResponse::AlreadyOnline));continue;}
                let Some(index)=(1..2000).find(|i|!actors.contains_key(i))else{let _=reply.send(Err(AuthResponse::FullWorld));continue;};
                let rights=prepared.details.rights().map_or(0,|r|r.ordinal());let name=prepared.login.username;
                let p=Shared::new(Player::new(index,Location::new(3207,3216,0),2000));{let mut p=p.write();p.update_scene=true;p.direction=6;}
                let(incoming,input)=mpsc::channel(40);let(output,outgoing)=mpsc::channel(2);
                actors.insert(index,Actor::new(p.clone(),appearance::Data::naked(name.clone(),0),input,output,name.clone(),&animations.factory));spot_ages.remove(&index);order.push(index);
                match render(&actors,&order,&keys,&p,true){
                    Ok(frames)=>{let a=&actors[&index];let _=a.output.try_send(frames);if reply.send(Ok(Joined{rights,index,incoming,outgoing})).is_err(){actors.remove(&index);order.retain(|i|*i!=index);}else{println!("stationary fixture joined: {name} index={index}");}},
                    Err(_)=>{actors.remove(&index);order.retain(|i|*i!=index);let _=reply.send(Err(AuthResponse::ErrorLoadingProfile));}
                }
            },
            _=ticker.tick()=>{
                ticks = ticks.wrapping_add(1);
                animations.ticks=ticks as i32;
                if let Some(timeline)=&spot_probe {
                    for(&index,actor)in &mut actors {
                        let age=spot_ages.entry(index).or_default();*age=age.wrapping_add(1);
                        if let Some(spot)=timeline.at(*age){
                            actor.animator.animate(&mut animations,None,Some(Graphics::new(spot.id,spot.height,spot.delay))).expect("controlled spot context");actor.masks.sync_appearance(millis).expect("controlled appearance context");
                            println!("{}",serde_json::json!({"scope":"controlled PLAYER_INFO spot probe","fixtureTick":ticks,"actorAgeTicks":age,"index":index,"id":spot.id,"height":spot.height,"delay":spot.delay,"packed":spot.packed()}));
                        }
                    }
                }
                if ticks.is_multiple_of(10)
                    && let Some(id) = animation_probe {
                        for a in actors.values_mut() {
                            a.animator.animate(&mut animations,Some(Animation::new(id)),None).expect("controlled animation context");
                            a.masks.sync_appearance(millis).expect("controlled appearance context");
                        }
                        if !actors.is_empty() { println!("visual animation probe: id={id}, delay=0, fixture tick={ticks}"); }
                    }
                let mut remove=vec![];
                for(&id,a)in &mut actors{
                    loop{match a.input.try_recv(){Ok(frame)=>{if ![22,21,245,93,20,110,75,98].contains(&frame.opcode){eprintln!("stationary fixture ignores opcode {}: gameplay handlers are not configured",frame.opcode);}},Err(mpsc::error::TryRecvError::Empty)=>break,Err(mpsc::error::TryRecvError::Disconnected)=>{remove.push(id);break;}}}
                }
                for id in remove.drain(..){if let Some(a)=actors.remove(&id){a.player.write().active=false;}order.retain(|i|*i!=id);}
                for a in actors.values(){a.masks.prepare().expect("fresh owned impact queue");}
                for &id in &order{
                    let a=&actors[&id];
                    match render(&actors,&order,&keys,&a.player,false) {
                        Ok(mut frames) => {
                            if ticks.is_multiple_of(10)
                                && let Some(sound) = sound_probe {
                                    frames.push(rs09_protocol::game::updates::synth_sound(sound,1,0));
                                }
                            if a.output.try_send(frames).is_err() { remove.push(id); }
                        }
                        Err(e) => { eprintln!("stationary render failed: {e:?}"); remove.push(id); }
                    }
                }
                for a in actors.values_mut(){a.masks.reset();let mut p=a.player.write();p.update_scene=false;let l=p.location.clone();p.info.write().update_information(l);}
                for id in remove{if let Some(a)=actors.remove(&id){a.player.write().active=false;}order.retain(|i|*i!=id);}
                spot_ages.retain(|index,_|actors.contains_key(index));
            }
        }
    }
}
#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
pub(crate) async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let keys_file = option("--keys").ok_or("--keys FILE is required (pinned XTEA JSON)")?;
    let data: Value = serde_json::from_slice(&std::fs::read(keys_file)?)?;
    let mut keys = HashMap::new();
    for row in data["xteas"].as_array().ok_or("missing xteas")? {
        let id = row["regionId"]
            .as_str()
            .ok_or("missing regionId")?
            .parse()?;
        let values: Vec<i32> = row["keys"]
            .as_str()
            .ok_or("missing keys")?
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        keys.insert(id, values.try_into().map_err(|_| "invalid key count")?);
    }
    let limits = Limits::default();
    let store = Store::open(
        option("--cache")
            .map_or("assets/cache", String::as_str)
            .as_ref(),
        StorageMode::Resident,
        limits,
    )?;
    let animation_assets = CacheSource(Archive::open(store.clone(), 20, None, limits)?);
    let catalog = Arc::new(Catalog::new(store, limits, 4096, 64 * 1024 * 1024)?);
    let (joins, receiver) = mpsc::channel(40);
    let animation_probe = option("--animation-probe")
        .map(|id| id.parse::<u16>().map(i32::from))
        .transpose()?;
    let sound_probe = option("--sound-probe")
        .map(|id| id.parse::<u16>().map(i32::from))
        .transpose()?;
    let spot_probe = option("--spot-probe")
        .map(|path| {
            std::fs::read(path)
                .map_err(Box::<dyn Error>::from)
                .and_then(|bytes| player_spots::Timeline::parse(&bytes))
        })
        .transpose()?;
    let has_spots = spot_probe.is_some();
    let owner = tokio::spawn(world(
        receiver,
        animation_assets,
        keys,
        animation_probe,
        sound_probe,
        spot_probe,
    ));
    let service = Service::new(
        catalog,
        Config {
            authenticator: Some(Arc::new(Authenticator::new(
                Mode::Development {
                    default_admin: false,
                },
                Arc::new(InMemoryStorage::default()),
                Arc::new(Passwords::default()),
            ))),
            world: Some(Arc::new(Fixture { joins })),
            ..Config::default()
        },
        530,
    )?;
    let server = service
        .start(
            option("--tcp")
                .map_or("127.0.0.1:43596", String::as_str)
                .parse()?,
            Some(
                option("--websocket")
                    .map_or("127.0.0.1:43597", String::as_str)
                    .parse()?,
            ),
        )
        .await?;
    println!(
        "Stationary transport fixture TCP={} WS={:?}. Native development auth; use username visual, password testing. No profile persistence, NPCs, movement, content, bank/shop or authoritative action handlers.",
        server.tcp, server.websocket
    );
    if let Some(id) = animation_probe {
        println!(
            "Explicit visual animation probe: mask 0x8, id {id}, delay 0, every 10 fixture ticks (~6s). Rendering test only; no gameplay action/content implementation."
        );
    }
    if let Some(id) = sound_probe {
        println!(
            "Explicit audio probe: SYNTH_SOUND 172, id {id}, loops 1, delay 0, every 10 fixture ticks (~6s). Audio delivery test only."
        );
    }
    if has_spots {
        println!(
            "Explicit PLAYER_INFO spot timeline: mask0x100, per-actor admission-relative ticks. Controlled rendering input; no spell/combat/content handler."
        );
    }
    tokio::signal::ctrl_c().await?;
    server.shutdown().await?;
    owner.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn actor(index: i32) -> Actor {
        let (_, input) = mpsc::channel(1);
        let (output, _) = mpsc::channel(1);
        Actor::new(
            Shared::new(Player::new(index, Location::new(3207, 3216, 0), 2000)),
            appearance::Data::default(),
            input,
            output,
            index.to_string(),
            &prepared_animation::Factory::default(),
        )
    }
    #[test]
    fn simultaneous_fixture_disconnects_release_mutual_local_player_caches() {
        let a = actor(1);
        let b = actor(2);
        let weak_a = a.player.downgrade();
        let weak_b = b.player.downgrade();
        a.player
            .read()
            .info
            .read()
            .local_players
            .write()
            .push(Some(b.player.clone()));
        b.player
            .read()
            .info
            .read()
            .local_players
            .write()
            .push(Some(a.player.clone()));
        drop(a);
        drop(b);
        assert!(weak_a.upgrade().is_none());
        assert!(weak_b.upgrade().is_none());
    }
    #[test]
    fn explicit_animation_probe_preserves_original_mask_order_and_player_frame() {
        use rs09_protocol::game::players::{Field, State};
        let mut actors = BTreeMap::new();
        let a = actor(1);
        a.masks.animate(863, 0).unwrap();
        let p = a.player.clone();
        {
            let mut player = p.write();
            player.last_scene = player.location.clone();
            player.update_scene = true;
        }
        actors.insert(1, a);
        let frames = render(&actors, &[1], &HashMap::new(), &p, false).unwrap();
        let update = State::new(55, 48).decode_frame(&frames[0], 1).unwrap();
        let fields = &update.extended[0].fields;
        assert!(matches!(fields[0], Field::Animation { id: 863, delay: 0 }));
        assert!(matches!(fields[1], Field::Appearance { .. }));
        assert_eq!(update.extended[0].flags, 12);
    }
    #[test]
    fn public_chat_is_delivered_once_to_the_sender_and_nearby_players() {
        use rs09_protocol::game::{
            chat,
            players::{Field, SELF, State},
        };
        let speaker = actor(1);
        let listener = actor(2);
        let mut payload = vec![3, 1, 5];
        payload.extend(chat::WORDPACK.encode(b"hello").unwrap());
        let (effects, text) = chat::public_mask(&Frame {
            opcode: chat::PUBLIC,
            payload,
        })
        .unwrap()
        .unwrap();
        speaker.masks.chat(effects, 0, text).unwrap();
        for p in [&speaker.player, &listener.player] {
            let mut p = p.write();
            p.last_scene = p.location.clone();
            p.update_scene = true;
        }
        let actors = BTreeMap::from([(1, speaker), (2, listener)]);
        for (viewer, source) in [(1, SELF), (2, 1)] {
            let frames = render(
                &actors,
                &[1, 2],
                &HashMap::new(),
                &actors[&viewer].player,
                false,
            )
            .unwrap();
            let update = State::new(55, 48).decode_frame(&frames[0], 1).unwrap();
            let message = update.extended.iter().find(|e| e.index == source).unwrap();
            assert!(
                matches!(&message.fields[0], Field::PublicChat { color_effect: 769, rights: 0, payload }
                if chat::public_text(payload).unwrap() == b"Hello")
            );
        }
        actors[&1].masks.reset();
        assert_eq!(actors[&1].masks.status().presence & 0x80, 0);
    }

    #[test]
    fn player_spot_composes_extended_header_animation_appearance_and_nearby_masks() {
        use rs09_protocol::game::players::{Field, SELF, State};
        let a = actor(1);
        let b = actor(2);
        a.masks.animate(863, 0).unwrap();
        a.masks
            .spot(appearance::SpotAnimation {
                id: 111,
                height: 128,
                delay: 60,
            })
            .unwrap();
        b.masks
            .spot(appearance::SpotAnimation {
                id: 369,
                height: 256,
                delay: 30,
            })
            .unwrap();
        for p in [&a.player, &b.player] {
            let mut p = p.write();
            p.last_scene = p.location.clone();
            p.update_scene = true;
        }
        let player = a.player.clone();
        let actors = BTreeMap::from([(1, a), (2, b)]);
        let frames = render(&actors, &[1, 2], &HashMap::new(), &player, false).unwrap();
        let update = State::new(55, 48).decode_frame(&frames[0], 1).unwrap();
        let own = update.extended.iter().find(|e| e.index == SELF).unwrap();
        assert_eq!(own.flags, 0x11c);
        assert!(matches!(
            own.fields[0],
            Field::Animation { id: 863, delay: 0 }
        ));
        assert!(matches!(own.fields[1], Field::Appearance { .. }));
        assert!(matches!(own.fields[2],Field::SpotAnimation{id:111,packed}if packed==0x0080003c));
        let other = update.extended.iter().find(|e| e.index == 2).unwrap();
        assert!(matches!(other.fields[0], Field::Appearance { .. }));
        assert!(matches!(other.fields[1],Field::SpotAnimation{id:369,packed}if packed==0x0100001e));
    }
    #[test]
    fn spot_probe_omits_transient_masks_between_events_and_sends_an_explicit_clear() {
        use rs09_protocol::game::players::{Field, State};
        let a = actor(1);
        a.masks
            .spot(appearance::SpotAnimation {
                id: 111,
                height: 0,
                delay: 0,
            })
            .unwrap();
        let p = a.player.clone();
        {
            let mut p = p.write();
            p.last_scene = p.location.clone();
            p.update_scene = true;
        }
        let actors = BTreeMap::from([(1, a)]);
        let mut state = State::new(55, 48);
        let frames = render(&actors, &[1], &HashMap::new(), &p, false).unwrap();
        state.decode_frame(&frames[0], 1).unwrap();
        actors[&1].masks.reset();
        {
            let mut p = p.write();
            p.update_scene = false;
            let l = p.location.clone();
            p.info.write().update_information(l);
        }
        let frames = render(&actors, &[1], &HashMap::new(), &p, false).unwrap();
        let update = state.decode_frame(&frames[0], 2).unwrap();
        assert!(update.extended.is_empty());
        actors[&1]
            .masks
            .spot(appearance::SpotAnimation {
                id: -1,
                height: 0,
                delay: 0,
            })
            .unwrap();
        actors[&1].masks.sync_appearance(millis).unwrap();
        let frames = render(&actors, &[1], &HashMap::new(), &p, false).unwrap();
        let update = state.decode_frame(&frames[0], 3).unwrap();
        assert_eq!(update.extended[0].flags, 0x114);
        assert!(matches!(
            update.extended[0].fields.last().unwrap(),
            Field::SpotAnimation { id: -1, packed: 0 }
        ));
    }
    #[test]
    fn a_new_viewer_receives_persistent_facing_after_the_owners_cycle_reset() {
        use rs09_protocol::game::players::{Field, State};
        let a = actor(1);
        let b = actor(2);
        a.masks
            .face_location(Some(Location::new(3210, 3216, 0)))
            .unwrap();
        a.masks.prepare().unwrap();
        a.masks.reset();
        assert!(!a.player.read().masks.required);
        assert!(a.player.read().masks.synced);
        let viewer = b.player.clone();
        {
            let mut v = viewer.write();
            v.last_scene = v.location.clone();
            v.update_scene = true;
        }
        let actors = BTreeMap::from([(1, a), (2, b)]);
        let frames = render(&actors, &[1, 2], &HashMap::new(), &viewer, false).unwrap();
        let update = State::new(55, 48).decode_frame(&frames[0], 1).unwrap();
        let other = update.extended.iter().find(|e| e.index == 1).unwrap();
        assert_eq!(other.flags, 0x44);
        assert!(matches!(other.fields[0], Field::Appearance { .. }));
        assert!(matches!(other.fields[1], Field::FaceTile([6421, 6433])));
    }
    #[test]
    fn rendering_rejects_a_retired_owner_instead_of_using_a_reused_wire_index() {
        let old = actor(1);
        let replacement = actor(1);
        let old_view = old.player.clone();
        let actors = BTreeMap::from([(1, replacement)]);
        let keys = HashMap::new();
        let mut renderer = Renderer {
            actors: &actors,
            order: &[1],
            keys: &keys,
            frames: vec![],
        };
        let mut bytes = vec![];
        assert!(
            render::Context::masks(&mut renderer, &old_view, &old_view, &mut bytes, true, true)
                .is_err()
        );
        assert!(bytes.is_empty());
    }
    #[test]
    fn player_spot_schedule_is_admission_relative_and_repeats_without_implicit_clears() {
        let timeline = player_spots::Timeline::parse(include_bytes!(
            "../../../fixtures/player-spot-probe/timeline.json"
        ))
        .unwrap();
        let observed: Vec<_> = (1..=32)
            .filter_map(|age| {
                timeline
                    .at(age)
                    .map(|spot| (age, spot.id, spot.height, spot.delay))
            })
            .collect();
        assert_eq!(observed.len(), 12);
        assert_eq!(observed[0], (2, 111, 0, 0));
        assert_eq!(observed[1], (4, 369, 128, 60));
        assert_eq!(observed[3], (7, -1, 0, 0));
        assert_eq!(observed[6], (18, 111, 0, 0));
        assert!(timeline.at(3).is_none());
        for invalid in [
            r#"{"repeatTicks":0,"events":[]}"#,
            r#"{"repeatTicks":2,"events":[{"tick":2,"id":111}]}"#,
            r#"{"events":[{"tick":1,"id":111},{"tick":1,"id":369}]}"#,
        ] {
            assert!(player_spots::Timeline::parse(invalid.as_bytes()).is_err());
        }
    }
}
