use super::*;
use rs09_protocol::{
    Result as ProtocolResult,
    game::{Decoder, ground as client},
};
use rs09_server::definitions::item::Shared;
use rs09_server::{
    auth::account::UserAccountInfo,
    definitions::{item::ItemDefinition, property::Property},
    world::{ground_item as gi, ground_manager as gm, player_walk},
};

fn fixture() -> (super::super::Probe, [NodeHandle; 2]) {
    fixture_extra(&[])
}
fn fixture_extra(extra: &[i32]) -> (super::super::Probe, [NodeHandle; 2]) {
    let (mut p, node, _) = super::super::tests::probe();
    let mut definitions = item::Registry::default();
    for id in (1..=4).chain(extra.iter().copied()) {
        let mut d = ItemDefinition {
            id,
            ..Default::default()
        };
        d.handlers.insert("tradeable".into(), Property::Bool(true));
        definitions.insert(id, Shared::new(d));
    }
    let mut ground = Ground::new(definitions);
    ground.repository = p.renderables.clone();
    p.host.ground = Some(ground);
    let other = p.state.add(2, Location::new(3208, 3216, 0), [1, 2, 3, 4]);
    let nodes = [
        p.state.retain_node(node).unwrap(),
        p.state.retain_node(other).unwrap(),
    ];
    for (i, h) in nodes.iter().enumerate() {
        p.state.actor_mut(h.node).unwrap().view.active = true;
        let mut account = UserAccountInfo::create_default(0);
        account.uid = 7 + i as i32 * 2;
        account.username = format!("p{i}");
        let render = if i == 0 {
            p.actors[&1].player.clone()
        } else {
            Shared::new(player_render::Player::new(
                2,
                Location::new(3208, 3216, 0),
                2000,
            ))
        };
        render.write().last_scene = Some(Location::new(3207, 3216, 0));
        admit(&mut p.host, h.node, account.shared(), render);
        p.host.maps.players.insert(h.node);
        p.host.maps.names.insert(h.node, format!("p{i}"));
    }
    (p, nodes)
}
struct Metadata;
impl client::Context for Metadata {
    fn info(&mut self, _: &mut client::State, _: i32) -> ProtocolResult<client::Info> {
        Ok(client::Info {
            cost: 1,
            stackable: 0,
        })
    }
    // Decoder test only; zero is not a claim about real cache terrain height.
    fn height(&mut self, _: &mut client::State, _: i32, _: i32, _: i32) -> ProtocolResult<i32> {
        Ok(0)
    }
}
fn receive(
    host: &mut Host,
    nodes: &[NodeHandle; 2],
    clients: &mut [client::State; 2],
) -> Vec<(Node, Vec<u8>)> {
    let sent = std::mem::take(&mut host.frames);
    let mut traces = vec![];
    for (node, frames) in sent {
        let target = nodes.iter().position(|p| p.node == node).unwrap();
        let mut bytes = vec![];
        for f in frames {
            f.write(Direction::ServerToClient, None, &mut bytes)
                .unwrap();
        }
        let mut decoder = Decoder::new(Direction::ServerToClient, 65536, 65536);
        for byte in &bytes {
            decoder.push(&[*byte]).unwrap();
            while let Some(f) = decoder.next_frame().unwrap() {
                clients[target].apply_frame(&f, &mut Metadata).unwrap();
            }
        }
        assert_eq!(decoder.buffered(), 0);
        traces.push((node, bytes));
    }
    traces
}
fn tile(x: i32, y: i32) -> client::Tile {
    client::Tile {
        plane: 0,
        x: x - 3152,
        y: y - 3168,
    }
}
fn amount(c: &client::State, t: client::Tile, id: i32) -> Option<i32> {
    c.stack(t).unwrap().and_then(|s| {
        s.items()
            .iter()
            .find(|i| i.get().id == id)
            .map(|i| i.get().amount)
    })
}

fn pickup_fixture() -> (
    super::super::Probe,
    [NodeHandle; 2],
    rs09_server::player::owned::Player,
) {
    pickup_fixture_extra(&[])
}
fn pickup_fixture_extra(
    extra: &[i32],
) -> (
    super::super::Probe,
    [NodeHandle; 2],
    rs09_server::player::owned::Player,
) {
    use rs09_server::{
        inventory::{
            container::{ContainerType, Factory, SortType},
            container_listener, listener, player_binding,
        },
        player::owned::Player,
        world::interfaces::Manager,
    };
    let (mut p, nodes) = fixture_extra(extra);
    p.state
        .actor(nodes[0].node)
        .unwrap()
        .queue
        .state
        .write()
        .entity
        .update_scene = false;
    let definitions = p.host.ground.as_ref().unwrap().items.definitions.clone();
    definitions
        .for_id(2)
        .write()
        .handlers
        .insert("weight".into(), Property::Double(2.5));
    let factory = Factory::new(&definitions);
    let mut inventory = factory
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap();
    inventory.register(Some(container_listener::Handle::new(
        listener::Listener::new(Some(nodes[0].clone())),
    )));
    let equipment = factory
        .create(14, ContainerType::Default, SortType::Id)
        .unwrap();
    let core = player_binding::Capture::new(
        nodes[0].clone(),
        inventory,
        equipment,
        Default::default(),
        factory.null_item.clone(),
    );
    let fields = Player::new(
        p.state.identity(),
        core,
        definitions,
        Manager::default(),
        Shared::new(false),
        p.host.masks[&nodes[0].node].clone(),
    )
    .unwrap();
    p.state.bind_player_fields(fields.clone()).unwrap();
    p.host.activate(&Location::new(3207, 3216, 0)).unwrap();
    // Portable test cache has no terrain; use an explicitly authored walkable
    // patch, then exercise the actual Scene permission and movement kernels.
    p.host.scene.region((50 << 8) | 50).write().has_flags = true;
    for x in 3205..3210 {
        for y in 3214..3219 {
            let c = p.host.scene.chunk(&Location::new(x, y, 0)).unwrap();
            let flags = c.read().flags.clone();
            flags
                .clear(&mut p.host.scene.context(&mut p.host.maps), x & 7, y & 7)
                .unwrap();
            flags.mark_landscape(x & 7, y & 7, true).unwrap();
        }
    }
    load(
        &mut p.state,
        &mut p.host,
        r#"[{"item_id":2,"loc_data":"1,3207,3216,0,2"}]"#,
    )
    .unwrap();
    (p, nodes, fields)
}
fn take_packet(p: &mut super::super::Probe, node: Node) {
    take_packet_id(p, node, 2)
}
fn take_packet_id(p: &mut super::super::Probe, node: Node, id: i32) {
    use super::super::{Ingress, QueuedIngress};
    let frame = Frame {
        opcode: 66,
        payload: vec![0x87, 0x0c, (id >> 8) as u8, id as u8, 0x10, 0x0c],
    };
    let permit = std::sync::Arc::new(tokio::sync::Semaphore::new(1))
        .try_acquire_owned()
        .unwrap();
    rs09_server::world::packet_queue::Context::process(
        p,
        &QueuedIngress {
            event: Ingress::Frame(1, node, frame),
            _permit: permit,
        },
    )
    .unwrap();
}
#[test]
fn god_cape_take_uses_canonical_active_bank_inventory_and_equipment_ownership() {
    use rs09_server::inventory::item::Item;
    for store in [
        "primary",
        "secondary",
        "inventory",
        "equipment",
        "inactive",
        "dropLog",
    ] {
        let (mut p, nodes, fields) = pickup_fixture_extra(&[2412, 2413, 2414]);
        let definitions = fields.definitions();
        let cape = Shared::new(Item::new(2412, 1, 1000, &definitions));
        let target = match store {
            "primary" => fields.bank_primary().container(),
            "secondary" => {
                fields.set_use_secondary_bank(true);
                fields.bank_secondary().container()
            }
            "inventory" => fields.inventory(),
            "equipment" => fields.equipment(),
            "inactive" => {
                fields.set_use_secondary_bank(true);
                fields.bank_primary().container()
            }
            _ => fields.drop_log().container(),
        };
        target.items().write()[0] = Some(cape);
        load(
            &mut p.state,
            &mut p.host,
            r#"[{"item_id":2413,"loc_data":"1,3207,3216,0,2"}]"#,
        )
        .unwrap();
        p.host.frames.clear();
        take_packet_id(&mut p, nodes[0].node, 2413);
        p.state.context(&mut p.host).pulses().unwrap();
        let blocked = !["inactive", "dropLog"].contains(&store);
        assert_eq!(
            fields.inventory().contains(2413, 1),
            !blocked,
            "store{store}"
        );
        let items = p.host.ground.as_ref().unwrap().items.manager.get_items();
        assert_eq!(
            items
                .read()
                .iter()
                .flatten()
                .any(|g| gi::as_item(g).read().id() == 2413),
            blocked,
            "store{store}"
        );
        let frames = &p.host.frames[&nodes[0].node];
        if blocked {
            assert_eq!(
                frames
                    .iter()
                    .filter(|f| f.opcode == 70)
                    .map(|f| &f.payload[..f.payload.len() - 1])
                    .collect::<Vec<_>>(),
                [
                    b"You may only possess one sacred cape at a time.".as_slice(),
                    b"The conflicting powers of the capes drive them apart.".as_slice()
                ]
            );
            assert!(frames.iter().all(|f| f.opcode != 172));
        } else {
            assert!(
                frames
                    .iter()
                    .any(|f| *f == rs09_protocol::game::updates::synth_sound(2582, 1, 0))
            );
        }
        assert!(p.host.errors.is_empty(), "store{store}");
    }
}
#[test]
fn actual_take_packet_arrival_updates_real_inventory_weight_and_both_ground_viewers() {
    let (mut p, nodes, fields) = pickup_fixture();
    let mut clients = std::array::from_fn(|_| client::State::default());
    for node in &nodes {
        render(&mut p.state, &mut p.host, node).unwrap();
    }
    receive(&mut p.host, &nodes, &mut clients);
    p.host.scene.clear_updates();
    for c in &clients {
        assert_eq!(amount(c, tile(3207, 3216), 2), Some(1));
    }
    take_packet(&mut p, nodes[0].node);
    assert!(
        fields
            .inventory()
            .items()
            .read()
            .iter()
            .all(Option::is_none)
    );
    p.state.context(&mut p.host).pulses().unwrap();
    let added = fields.inventory().items().read()[0].clone().unwrap();
    assert_eq!((added.read().id(), added.read().amount()), (2, 1));
    assert_eq!(fields.settings().fields.read().weight, 2.5);
    let private = &p.host.frames[&nodes[0].node];
    assert!(private.iter().any(|f| f.opcode == 22));
    assert!(
        private
            .iter()
            .any(|f| *f == rs09_protocol::game::updates::synth_sound(2582, 1, 0))
    );
    assert!(private.iter().any(|f|matches!(rs09_protocol::game::updates::ServerUpdate::decode(f).unwrap(),Some(rs09_protocol::game::updates::ServerUpdate::InterfaceText{text,..})if text==b"2 kg")));
    for node in &nodes {
        render(&mut p.state, &mut p.host, node).unwrap();
    }
    assert!(!p.host.scene.dirty_chunks.is_empty());
    assert!(
        p.host.frames[&nodes[1].node]
            .iter()
            .all(|f| ![22, 171, 172].contains(&f.opcode))
    );
    receive(&mut p.host, &nodes, &mut clients);
    for c in &clients {
        assert_eq!(amount(c, tile(3207, 3216), 2), None);
    }
    assert!(p.host.errors.is_empty());
}
#[test]
fn actual_pickup_attribute_presence_short_circuits_even_for_false_or_null() {
    for null in [false, true] {
        let (mut p, nodes, fields) = pickup_fixture();
        if null {
            fields.attributes().set_null("pickup")
        } else {
            fields.attributes().set_boolean("pickup", false)
        }
        take_packet(&mut p, nodes[0].node);
        p.state.context(&mut p.host).pulses().unwrap();
        assert!(
            fields
                .inventory()
                .items()
                .read()
                .iter()
                .all(Option::is_none)
        );
        assert!(fields.attributes().contains_key("pickup"));
        assert!(
            p.host
                .frames
                .get(&nodes[0].node)
                .into_iter()
                .flatten()
                .all(|f| ![22, 172].contains(&f.opcode))
        );
        assert!(p.host.errors.is_empty());
    }
}

#[test]
fn actual_pickup_keeps_the_item_when_inventory_is_full_or_deadline_is_future() {
    for full in [false, true] {
        let (mut p, nodes, fields) = pickup_fixture();
        if full {
            let defs = fields.definitions();
            let array = fields.inventory().items();
            let mut array = array.write();
            for slot in array.iter_mut() {
                *slot = Some(Shared::new(rs09_server::inventory::item::Item::new(
                    1, 1, 1000, &defs,
                )));
            }
        } else {
            fields.attributes().set_integer("droppedItem:2", 100);
        }
        let before: Vec<_> = fields
            .inventory()
            .items()
            .read()
            .iter()
            .map(|v| v.as_ref().map(|v| v.identity()))
            .collect();
        take_packet(&mut p, nodes[0].node);
        p.state.context(&mut p.host).pulses().unwrap();
        let after: Vec<_> = fields
            .inventory()
            .items()
            .read()
            .iter()
            .map(|v| v.as_ref().map(|v| v.identity()))
            .collect();
        assert_eq!(before, after);
        assert!(
            p.host
                .frames
                .get(&nodes[0].node)
                .into_iter()
                .flatten()
                .all(|f| ![22, 172].contains(&f.opcode))
        );
        let manager = &p.host.ground.as_ref().unwrap().items.manager;
        assert!(
            manager
                .get_items()
                .read()
                .iter()
                .flatten()
                .any(|g| ground_item_visible(g))
        );
        assert!(p.host.errors.is_empty());
    }
}
fn ground_item_visible(h: &gi::Handle) -> bool {
    !h.read().removed
}
#[test]
fn owning_scene_viewports_private_amount_publication_and_expiry_reach_both_clients() {
    let (mut p, nodes) = fixture();
    let mut clients = std::array::from_fn(|_| client::State::default());
    for h in &nodes {
        render(&mut p.state, &mut p.host, h).unwrap();
    }
    receive(&mut p.host, &nodes, &mut clients);
    assert_eq!(
        load(
            &mut p.state,
            &mut p.host,
            r#"[{"item_id":2,"loc_data":"1,3227,3216,0,2-1,3271,3216,0,2-1,3207,3216,1,2"}]"#
        )
        .unwrap(),
        3
    );
    // A public item20tiles away is inside the original104tile build area.
    // A radius15 fanout would incorrectly hide it.
    let private = world(&mut p.state, &mut p.host, |w| {
        let input = gi::Context::copy_item(w, 1, 1, 1000).map_err(Error::Ground)?;
        let manager = w.world.items.manager.clone();
        gm::create_at(
            w,
            &manager,
            Some(input),
            Some(Location::new(3207, 3216, 0)),
            Some(nodes[0].clone()),
        )
    })
    .unwrap();
    let direct = receive(&mut p.host, &nodes, &mut clients);
    assert_eq!(direct.len(), 1);
    assert_eq!(direct[0].0, nodes[0].node);
    assert_eq!(direct[0].1[0], 26);
    assert_eq!(amount(&clients[0], tile(3207, 3216), 1), Some(1));
    assert_eq!(amount(&clients[1], tile(3207, 3216), 1), None);
    render(&mut p.state, &mut p.host, &nodes[0]).unwrap();
    assert!(
        !p.host.scene.dirty_chunks.is_empty(),
        "first viewer must not reset shared flags"
    );
    render(&mut p.state, &mut p.host, &nodes[1]).unwrap();
    receive(&mut p.host, &nodes, &mut clients);
    for c in &clients {
        assert_eq!(amount(c, tile(3227, 3216), 2), Some(1));
        assert!(c.occupied().iter().all(|t| t.plane == 0));
    }
    let real = p.host.scene.chunk(&Location::new(3227, 3216, 0)).unwrap();
    let g = p.host.ground.as_ref().unwrap();
    assert!(
        g.profiles[&nodes[0].node]
            .viewport
            .read()
            .iter()
            .flatten()
            .any(|row| row.read().iter().flatten().any(|c| c.same(&real)))
    );
    p.host.scene.clear_updates();
    world(&mut p.state, &mut p.host, |w| {
        let input = gi::Context::copy_item(w, 1, 2, 1000).map_err(Error::Ground)?;
        let manager = w.world.items.manager.clone();
        gm::increase(
            w,
            &manager,
            Some(input),
            Some(Location::new(3207, 3216, 0)),
            Some(nodes[0].clone()),
        )
        .map(|_| ())
    })
    .unwrap();
    receive(&mut p.host, &nodes, &mut clients);
    assert_eq!(amount(&clients[0], tile(3207, 3216), 1), Some(3));
    assert_eq!(amount(&clients[1], tile(3207, 3216), 1), None);
    p.state.ticks = 100;
    manager_pulse(&mut p.state, &mut p.host).unwrap();
    for h in &nodes {
        render(&mut p.state, &mut p.host, h).unwrap();
    }
    receive(&mut p.host, &nodes, &mut clients);
    for c in &clients {
        assert_eq!(amount(c, tile(3207, 3216), 1), Some(3));
        assert_eq!(c.stack(tile(3207, 3216)).unwrap().unwrap().items().len(), 1);
    }
    p.host.scene.clear_updates();
    p.state.ticks = 200;
    manager_pulse(&mut p.state, &mut p.host).unwrap();
    for h in &nodes {
        render(&mut p.state, &mut p.host, h).unwrap();
    }
    receive(&mut p.host, &nodes, &mut clients);
    for c in &clients {
        assert_eq!(amount(c, tile(3207, 3216), 1), None);
        assert_eq!(amount(c, tile(3227, 3216), 2), Some(1));
    }
    assert!(
        p.host
            .scene
            .chunk(&Location::new(3207, 3216, 0))
            .unwrap()
            .read()
            .items
            .as_ref()
            .unwrap()
            .read()
            .iter()
            .all(|g| g.as_ref().is_none_or(|g| !g.same(&private)))
    );
    assert!(p.host.errors.is_empty());
}
#[test]
fn spawned_items_run_on_the_shared_actor_scheduler_and_reject_foreign_domains() {
    let (mut p, nodes) = fixture();
    load(
        &mut p.state,
        &mut p.host,
        r#"[{"item_id":2,"loc_data":"1,3207,3216,0,2"}]"#,
    )
    .unwrap();
    let original = p
        .host
        .ground
        .as_ref()
        .unwrap()
        .items
        .manager
        .get_items()
        .read()
        .get(0)
        .unwrap()
        .clone()
        .unwrap();
    world(&mut p.state, &mut p.host, |w| {
        let manager = w.world.items.manager.clone();
        gm::destroy(w, &manager, Some(original.clone())).map(|_| ())
    })
    .unwrap();
    assert_eq!(p.state.pending().len(), 1);
    assert!(p.host.scene.runner.same(&p.state.runner()));
    assert!(
        p.host
            .ground
            .as_ref()
            .unwrap()
            .items
            .manager
            .get_items()
            .read()
            .is_empty()
    );
    p.state.context(&mut p.host).pulses().unwrap();
    assert!(
        p.host
            .ground
            .as_ref()
            .unwrap()
            .items
            .manager
            .get_items()
            .read()
            .is_empty()
    );
    p.state.context(&mut p.host).pulses().unwrap();
    let items = p.host.ground.as_ref().unwrap().items.manager.get_items();
    assert!(
        items
            .read()
            .get(0)
            .unwrap()
            .as_ref()
            .unwrap()
            .same(&original)
    );
    assert!(p.state.pending().is_empty());
    let mut foreign = player_walk::State::default();
    let id = foreign.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let foreign_node = foreign.retain_node(id).unwrap();
    assert!(
        world(&mut p.state, &mut p.host, |w| gi::Context::uid(
            w,
            &foreign_node
        )
        .map_err(Error::Ground))
        .is_err()
    );
    assert!(
        world(&mut p.state, &mut p.host, |w| gi::Context::uid(
            w, &nodes[0]
        )
        .map_err(Error::Ground))
        .is_ok()
    );
    assert!(
        Parts::new(
            &mut p.host.scene,
            &mut foreign,
            &p.host.ground.as_ref().unwrap().items
        )
        .is_err()
    );
    let fake = Shared::new(pulses::Pulse::new(
        1,
        None,
        pulses::Class::Other,
        "foreign".into(),
    ));
    assert!(stop(&mut p.state, &mut p.host, &fake).is_err());
    assert!(p.host.errors.is_empty());
}

#[test]
fn scene_rebuild_and_plane_change_replace_canonical_viewports_and_send_clear_before_sync() {
    let (mut p, nodes) = fixture();
    let mut clients = std::array::from_fn(|_| client::State::default());
    load(&mut p.state,&mut p.host,r#"[{"item_id":2,"loc_data":"1,3227,3216,0,2-1,3271,3216,0,2"},{"item_id":3,"loc_data":"5,3271,3216,1,2"}]"#).unwrap();
    for h in &nodes {
        render(&mut p.state, &mut p.host, h).unwrap();
    }
    receive(&mut p.host, &nodes, &mut clients);
    p.host.scene.clear_updates();
    assert_eq!(amount(&clients[0], tile(3227, 3216), 2), Some(1));
    let next = Location::new(3264, 3216, 0);
    p.state
        .actor(nodes[0].node)
        .unwrap()
        .queue
        .state
        .write()
        .entity
        .location = Some(next.clone());
    let render_handle = p.host.ground.as_ref().unwrap().profiles[&nodes[0].node]
        .render
        .clone();
    render_handle.write().last_scene = Some(next);
    clients[0].rebase_stacks([64, 0]);
    render(&mut p.state, &mut p.host, &nodes[0]).unwrap();
    receive(&mut p.host, &nodes, &mut clients);
    let local = client::Tile {
        plane: 0,
        x: 55,
        y: 48,
    };
    assert_eq!(
        amount(&clients[0], local, 2),
        Some(1),
        "newly entered chunk must synchronize"
    );
    assert_eq!(
        amount(
            &clients[0],
            client::Tile {
                plane: 0,
                x: 11,
                y: 48
            },
            2
        ),
        Some(1)
    );
    let above = Location::new(3264, 3216, 1);
    p.state
        .actor(nodes[0].node)
        .unwrap()
        .queue
        .state
        .write()
        .entity
        .location = Some(above.clone());
    // The original renderer uses current plane with retained last-scene x/y.
    assert_eq!(
        render_handle
            .read()
            .last_scene
            .as_ref()
            .unwrap()
            .coordinates()[2],
        0
    );
    clients[0].plane = 1;
    render(&mut p.state, &mut p.host, &nodes[0]).unwrap();
    let frames = &p.host.frames[&nodes[0].node];
    assert_eq!(frames.iter().take_while(|f| f.opcode == 112).count(), 169);
    assert!(frames.iter().skip(169).any(|f| f.opcode == 230));
    receive(&mut p.host, &nodes, &mut clients);
    assert_eq!(
        amount(&clients[0], client::Tile { plane: 1, ..local }, 3),
        Some(5)
    );
    assert_eq!(amount(&clients[1], tile(3227, 3216), 2), Some(1));
    let real = p.host.scene.chunk(&above).unwrap();
    let viewport = &p.host.ground.as_ref().unwrap().profiles[&nodes[0].node].viewport;
    assert!(
        viewport.read().iter().flatten().any(|row| row
            .read()
            .iter()
            .flatten()
            .any(|c| c.same(&real)))
    );
    assert!(viewport.read().iter().flatten().all(|row| {
        row.read()
            .iter()
            .flatten()
            .all(|c| c.read().current_base.coordinates()[2] == 1)
    }));
    assert!(p.host.errors.is_empty());
}

#[test]
fn constructed_ground_uses_real_default_table_and_native_pickup_program() {
    let (mut p, nodes) = fixture();
    let item = world(&mut p.state, &mut p.host, |w| {
        let input = gi::Context::copy_item(w, 1, 1, 1000).map_err(Error::Ground)?;
        gi::construct(
            w,
            &input,
            Some(Location::new(3207, 3216, 0)),
            200,
            Some(nodes[0].clone()),
        )
        .map_err(Error::Ground)
    })
    .unwrap();
    let ground = p.host.ground.as_ref().unwrap();
    let capture = ground
        .options
        .capture(&mut p.state, &ground.items.spawns, &item)
        .unwrap();
    assert!(capture.table.read().initialized);
    let option = rs09_server::world::interaction_options::get(&capture.table, 2)
        .unwrap()
        .unwrap();
    assert_eq!(option.read().name().as_deref(), Some("take"));
    let handler = capture.handler(option.read().handler.unwrap()).unwrap();
    assert!(
        handler
            .read()
            .value::<rs09_server::world::pickup::Program>()
            .is_some()
    );
    assert!(p.state.ground(capture.target.node).unwrap().same(&item));
    assert!(p.host.errors.is_empty());
}

#[test]
fn disconnected_dropper_keeps_profile_until_cached_uid_rebind_releases_the_actor() {
    let (mut p, nodes) = fixture();
    let [owner, observer] = nodes;
    let owner_id = owner.node;
    let ground = world(&mut p.state, &mut p.host, |w| {
        let input = gi::Context::copy_item(w, 1, 1, 1000).map_err(Error::Ground)?;
        let manager = w.world.items.manager.clone();
        gm::create_at(
            w,
            &manager,
            Some(input),
            Some(Location::new(3207, 3216, 0)),
            Some(owner.clone()),
        )
    })
    .unwrap();
    p.host.frames.clear();
    p.state.context(&mut p.host).disconnect(owner_id).unwrap();
    super::super::spatial_services::detach(&mut p.state, &mut p.host, owner_id).unwrap();
    p.leases.remove(&owner_id);
    drop(owner);
    p.state.collect();
    reap(&mut p.host, &p.state);
    assert!(p.state.actor(owner_id).is_ok());
    assert!(
        p.host
            .ground
            .as_ref()
            .unwrap()
            .profiles
            .contains_key(&owner_id)
    );
    assert!(
        world(&mut p.state, &mut p.host, |w| gi::java_string(w, &ground)
            .map_err(Error::Ground))
        .unwrap()
        .contains("dropper=p0")
    );
    let account = p.host.ground.as_ref().unwrap().profiles[&observer.node]
        .account
        .clone();
    account.lock().unwrap().uid = 7;
    assert!(
        world(&mut p.state, &mut p.host, |w| gi::dropped_by(
            w, &ground, &observer
        )
        .map_err(Error::Ground))
        .unwrap()
    );
    assert_eq!(ground.read().dropper.as_ref().unwrap().node, observer.node);
    p.state.collect();
    reap(&mut p.host, &p.state);
    assert!(
        p.state.actor(owner_id).is_err(),
        "profile metadata must not own a node lease"
    );
    assert!(
        !p.host
            .ground
            .as_ref()
            .unwrap()
            .profiles
            .contains_key(&owner_id)
    );
    assert!(p.host.errors.is_empty());
}
