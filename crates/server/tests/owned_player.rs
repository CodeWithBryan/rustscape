#[path = "../../../oracle/owned-player/native.rs"]
mod native;
use rs09_server::{
    definitions::item::{Registry, Shared},
    inventory::{
        container::{ContainerType, Factory, SortType},
        container_listener::Handle,
        listener::Listener,
        player_binding::Capture,
    },
    player::owned::Player,
    world::{
        appearance,
        doors::Location,
        interfaces::Manager,
        player_render,
        player_walk::{NodeHandle, State},
        prepared_masks::PreparedMasks,
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
const CORPUS: &str = include_str!("../../../fixtures/owned-player/oracle.jsonl");
#[test]
fn original_distinct_player_receiver_fields_and_packet_recipients_match() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "owned player original case{i}: {}",
            row["op"]
        );
    }
}
#[test]
fn original_player_receiver_sources_fixtures_and_previous_baseline_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/owned-player/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    assert_eq!(
        p["unchangedPlayerInventoryBaseline"],
        format!(
            "{:x}",
            Sha256::digest(
                std::fs::read(root.join("fixtures/player-inventory/oracle.jsonl")).unwrap()
            )
        )
    );
    for (name, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
}

fn player(state: &mut State, index: i32) -> (NodeHandle, Player) {
    let definitions = Arc::new(Registry::default());
    let factory = Factory::new(&definitions);
    player_with(state, index, definitions, &factory)
}
fn player_with(
    state: &mut State,
    index: i32,
    definitions: Arc<Registry>,
    factory: &Factory,
) -> (NodeHandle, Player) {
    let location = Location::new(3207, 3216, 0);
    let node = state.add(index, location.clone(), [1, 2, 3, 4]);
    let owner = state.retain_node(node).unwrap();
    let entity_scripts = state.scripts(&owner).unwrap();
    let mut inventory = factory
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap();
    inventory.register(Some(Handle::new(Listener::new(Some(owner.clone())))));
    let equipment = factory
        .create(14, ContainerType::Default, SortType::Id)
        .unwrap();
    let core = Capture::new(
        owner.clone(),
        inventory,
        equipment,
        Default::default(),
        factory.null_item.clone(),
    );
    let masks = PreparedMasks::new(
        Shared::new(player_render::Player::new(index, location, 2000)),
        Shared::new(appearance::Data::naked("owned".into(), 0)),
    );
    let fields = Player::new(
        state.identity(),
        core,
        definitions,
        Manager::default(),
        Shared::new(false),
        masks,
    )
    .unwrap();
    assert!(entity_scripts.same(&fields.scripts()));
    state.bind_player_fields(fields.clone()).unwrap();
    (owner, fields)
}

#[test]
fn bare_and_npc_entity_scripts_keep_their_exact_owner_then_release_with_a_retained_world_and_clocks()
 {
    for npc in [false, true] {
        let mut state = State::default();
        let identity = state.identity();
        let n = if npc {
            state.add_npc(1, Location::new(1, 1, 0), [1, 2, 3, 4])
        } else {
            state.add(1, Location::new(1, 1, 0), [1, 2, 3, 4])
        };
        let owner = state.retain_node(n).unwrap();
        let processor = state.scripts(&owner).unwrap();
        let weak = processor.fields.downgrade();
        let clocks = state.clocks(&owner).unwrap();
        clocks.write()[0] = 123;
        state.actor_mut(n).unwrap().connected = false;
        drop(owner);
        state.collect();
        assert!(state.actor(n).is_ok());
        assert!(processor.domain().unwrap().same(&identity));
        drop(processor);
        state.collect();
        assert!(state.actor(n).is_err());
        assert!(weak.upgrade().is_none());
        assert_eq!(clocks.read()[0], 123);
        let replacement = state.add(1, Location::new(2, 2, 0), [1, 2, 3, 4]);
        assert_ne!(replacement, n);
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn bare_and_npc_native_function_script_cycles_collect_without_player_inventory_fields() {
    use rs09_server::{
        scripts::Form,
        world::{
            component_objects::Captures,
            interaction_listeners::{Function, FunctionKind},
            script_runtime::Native,
        },
    };
    for npc in [false, true] {
        let mut state = State::default();
        let n = if npc {
            state.add_npc(1, Location::new(1, 1, 0), [1, 2, 3, 4])
        } else {
            state.add(1, Location::new(1, 1, 0), [1, 2, 3, 4])
        };
        let owner = state.retain_node(n).unwrap();
        let processor = state.scripts(&owner).unwrap();
        let function = Function::new(
            FunctionKind::Void,
            "native.cycle",
            Native::<()>::new(
                Captures {
                    processors: vec![processor.clone()],
                    ..Default::default()
                },
                |_, _, _, _, _| Ok(true),
            ),
        );
        let weak_function = function.header().downgrade();
        let script = processor
            .create_script(Form::Basic, Some(function), false)
            .unwrap();
        let weak_script = script.downgrade();
        processor.set_current(Some(script.clone()));
        state.actor_mut(n).unwrap().connected = false;
        drop(owner);
        drop(processor);
        drop(script);
        state.collect();
        assert!(state.actor(n).is_err());
        assert!(weak_script.upgrade().is_none());
        assert!(weak_function.upgrade().is_none());
    }
}

#[test]
fn native_capture_validation_rejects_foreign_bank_pin_processor_and_script_before_body_effects() {
    use rs09_server::{
        scripts::{Arguments, Form},
        world::{component_objects::Captures, script_runtime::Native},
    };
    let mut state = State::default();
    let (owner, _local) = player(&mut state, 1);
    let mut other = State::default();
    let (_, foreign) = player(&mut other, 1);
    let script = foreign
        .scripts()
        .create_script(Form::Basic, None, false)
        .unwrap();
    let cases = [
        Captures {
            containers: vec![foreign.bank().container()],
            ..Default::default()
        },
        Captures {
            pins: vec![foreign.bank_pin()],
            ..Default::default()
        },
        Captures {
            processors: vec![foreign.scripts()],
            ..Default::default()
        },
        Captures {
            scripts: vec![script],
            ..Default::default()
        },
    ];
    let mut effects = 0usize;
    for fields in cases {
        let body = Native::new(fields, |_, _, effects: &mut usize, _, _| {
            *effects += 1;
            Ok(true)
        });
        assert!(
            body.invoke(
                &mut state,
                &mut effects,
                &owner,
                Arguments::Void { stage: 0 }
            )
            .is_err()
        );
        assert_eq!(effects, 0);
    }
}

#[test]
fn exposed_stateless_action_lock_map_and_lock_views_do_not_anchor_the_storing_player() {
    use rs09_server::world::action_locks::{Clock, Kind};
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let actions = state.locks(&owner).unwrap();
    let weak = actions.fields.downgrade();
    let lock = actions.get(Kind::Movement).unwrap();
    let map = actions.custom();
    lock.lock(12, &mut state, &mut Clock).unwrap();
    let node = owner.node;
    state.actor_mut(node).unwrap().connected = false;
    drop(owner);
    drop(fields);
    drop(actions);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
    assert_eq!(lock.expiration(), 12);
    assert!(map.read().is_empty());
}

#[test]
fn real_elapse_action_lock_cycles_collect_after_the_last_lock_payload_or_map_view() {
    use rs09_server::world::{
        action_locks::{Clock, Kind, Native as Elapse},
        component_objects::{Captures, Plugin},
    };
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let actions = state.locks(&owner).unwrap();
    let lock = actions
        .lock_custom(Some(vec![97]), 5, None, &mut state, &mut Clock)
        .unwrap();
    let weak_lock = lock.fields.downgrade();
    actions
        .custom()
        .write()
        .insert(Some(vec![98]), lock.clone());
    actions.set_equipment(Some(lock.clone()));
    let event = Plugin::new(Elapse::<()>::new(
        Captures {
            nodes: vec![owner.clone()],
            action_locks: vec![actions.clone()],
            locks: vec![lock.clone()],
            ..Default::default()
        },
        |_, _, _, _, _| Ok(()),
    ));
    let weak_event = event.downgrade();
    lock.set_elapse(Some(event));
    let node = owner.node;
    state.actor_mut(node).unwrap().connected = false;
    drop(owner);
    drop(fields);
    drop(actions);
    state.collect();
    assert!(state.actor(node).is_ok());
    let payload = weak_event.upgrade().unwrap().value::<Elapse<()>>().unwrap();
    drop(lock);
    state.collect();
    assert!(state.actor(node).is_ok());
    let map = payload.fields().action_locks[0].custom();
    assert!(
        payload.fields().action_locks[0]
            .get(Kind::Equipment)
            .unwrap()
            .fields
            .same(&weak_lock.upgrade().unwrap())
    );
    drop(payload);
    state.collect();
    assert!(state.actor(node).is_ok());
    drop(map);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak_lock.upgrade().is_none());
    assert!(weak_event.upgrade().is_none());
}

#[test]
fn actual_foreign_lock_captures_are_rejected_before_native_body_effects() {
    use rs09_server::{
        scripts::Arguments,
        world::{action_locks::Kind, component_objects::Captures, script_runtime::Native},
    };
    let mut state = State::default();
    let (owner, _fields) = player(&mut state, 1);
    let mut foreign = State::default();
    let (other, _fields) = player(&mut foreign, 1);
    let locks = foreign.locks(&other).unwrap();
    let mut count = 0;
    for captures in [
        Captures {
            action_locks: vec![locks.clone()],
            ..Default::default()
        },
        Captures {
            locks: vec![locks.get(Kind::Movement).unwrap()],
            ..Default::default()
        },
    ] {
        let function = Native::new(captures, |_, _, count: &mut i32, _, _| {
            *count += 1;
            Ok(true)
        });
        assert!(
            function
                .invoke(&mut state, &mut count, &owner, Arguments::Void { stage: 0 })
                .is_err()
        );
        assert_eq!(count, 0);
    }
}

#[test]
fn local_collection_leaves_foreign_lock_cycles_and_their_exact_local_captures_untouched() {
    use rs09_server::{
        scripts::Form,
        world::{
            action_locks::{ActionLocks, Clock, Native as Elapse},
            component_objects::{Captures, Plugin},
            interaction_listeners::{Function, FunctionKind},
            script_runtime::Native,
        },
    };
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let node = owner.node;
    let other = State::default();
    let foreign = ActionLocks::new_in(other.identity());
    let weak_foreign = foreign.fields.downgrade();
    let lock = foreign
        .lock_custom(Some(vec![97]), 7, None, &mut state, &mut Clock)
        .unwrap();
    let event = Plugin::new(Elapse::<()>::new(
        Captures {
            nodes: vec![owner.clone()],
            action_locks: vec![foreign.clone()],
            ..Default::default()
        },
        |_, _, _, _, _| Ok(()),
    ));
    let weak_event = event.downgrade();
    lock.set_elapse(Some(event));
    let processor = fields.scripts();
    let function = Function::new(
        FunctionKind::Void,
        "native.foreign.locks",
        Native::<()>::new(
            Captures {
                action_locks: vec![foreign.clone()],
                ..Default::default()
            },
            |_, _, _, _, _| Ok(true),
        ),
    );
    let script = processor
        .create_script(Form::Basic, Some(function), false)
        .unwrap();
    processor.set_current(Some(script));
    state.actor_mut(node).unwrap().connected = false;
    drop(owner);
    drop(fields);
    drop(foreign);
    drop(lock);
    drop(processor);
    state.collect();
    assert!(state.actor(node).is_ok());
    assert!(weak_event.upgrade().is_some());
    let foreign = ActionLocks {
        fields: weak_foreign.upgrade().unwrap(),
    };
    let lock = foreign
        .custom()
        .read()
        .get(&Some(vec![97]))
        .unwrap()
        .clone();
    assert_eq!(lock.expiration(), 7);
    assert!(lock.elapse().is_some());
    lock.set_elapse(None);
    drop(lock);
    drop(foreign);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak_foreign.upgrade().is_none());
    assert!(weak_event.upgrade().is_none());
}

#[test]
fn replacing_an_elapse_payload_drops_it_after_releasing_the_actual_lock_guard() {
    use rs09_server::{
        definitions::item::WeakShared,
        world::{
            action_locks::{Lock, LockFields},
            component_objects::{NativeObject, Plugin},
        },
    };
    struct Payload {
        lock: WeakShared<LockFields>,
        tx: std::sync::mpsc::Sender<()>,
    }
    impl NativeObject for Payload {}
    impl Drop for Payload {
        fn drop(&mut self) {
            let lock = Lock {
                fields: self.lock.upgrade().unwrap(),
            };
            assert!(lock.elapse().is_none());
            lock.set_message(Some(vec![65].into()));
            self.tx.send(()).unwrap();
        }
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let lock = Lock::new(None);
        let event = Plugin::new(Payload {
            lock: lock.fields.downgrade(),
            tx,
        });
        lock.set_elapse(Some(event));
        lock.set_elapse(None);
        assert_eq!(lock.message().unwrap().as_ref(), &[65]);
    });
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("LockElapse destructor ran under its receiver guard");
}
fn disconnect(state: &mut State, owner: NodeHandle) -> usize {
    let node = owner.node;
    state.actor_mut(node).unwrap().connected = false;
    drop(owner);
    node
}

#[test]
fn value_attributes_preserve_null_presence_types_and_captured_player_identity() {
    use rs09_server::player::attributes::Error;
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let attributes = p.attributes();
    let weak = p.downgrade();
    attributes.set_boolean("pickup", false);
    assert!(attributes.contains_key("pickup"));
    attributes.set_null("pickup");
    assert!(attributes.contains_key("pickup"));
    assert!(attributes.get("pickup").is_none());
    attributes.set_integer("/save:droppedItem:995", 123);
    assert_eq!(attributes.integer("droppedItem:995", 0), Ok(123));
    attributes.set_boolean("wrong", true);
    assert_eq!(attributes.integer("wrong", 0), Err(Error::Type));
    drop(p);
    let node = disconnect(&mut state, owner);
    state.collect();
    let (_, replacement) = player(&mut state, 1);
    assert_eq!(
        replacement.attributes().integer("droppedItem:995", 0),
        Ok(0)
    );
    attributes.set_integer("droppedItem:995", 456);
    assert_eq!(
        weak.upgrade()
            .unwrap()
            .attributes()
            .integer("droppedItem:995", 0),
        Ok(456)
    );
    attributes.remove("pickup");
    assert!(!attributes.contains_key("pickup"));
    drop(attributes);
    state.collect();
    assert!(weak.upgrade().is_none());
    assert!(state.actor(node).is_err());
}

struct MissingVariableAssets;
impl rs09_server::definitions::varbit::Assets for MissingVariableAssets {
    fn file(
        &mut self,
        _: &rs09_server::definitions::varbit::Registry,
        _: u32,
        _: u32,
    ) -> rs09_server::definitions::varbit::Result<Option<Vec<u8>>> {
        Ok(None)
    }
}

#[test]
fn canonical_variables_use_original_custom_definitions_and_shared_interface_counter() {
    use rs09_protocol::game::updates;
    use rs09_server::{definitions::varbit, player::variables as v};
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let registry = state.identity().varbits();
    assert_eq!(
        varbit::Loader::load_text(
            &registry,
            include_str!("../../../fixtures/variables/varbit_definitions.json")
        ),
        Ok(2)
    );
    p.manager().fields.write().packet_count = i32::MAX;
    let mut frames = Default::default();
    p.apply_variables(&mut state, &mut frames, &mut MissingVariableAssets, |ctx| {
        v::set_varbit(ctx, Some(&owner), 7000, 1, true)?;
        v::set_varbit(ctx, Some(&owner), 7001, 1, false)?;
        assert_eq!(v::get_varbit(ctx, Some(&owner), 7000)?, 1);
        v::set_varc(ctx, Some(&owner), 65535, 256)?;
        v::set_varc(ctx, Some(&owner), 65536, -1)
    })
    .unwrap();
    assert_eq!(p.variables().get_varp(304), Ok(1));
    assert_eq!(p.variables().get_varp(115), Ok(1));
    assert_eq!(p.variables().saved().unwrap().get(304), Some(true));
    assert!(!p.variables().saved().unwrap().contains_key(115));
    assert_eq!(p.manager().fields.read().packet_count, i32::MIN + 1);
    assert_eq!(
        frames[&owner.node],
        vec![
            updates::varp(304, 1),
            updates::varp(115, 1),
            updates::varc(-1, 256, i32::MAX),
            updates::varc(0, -1, i32::MIN)
        ]
    );
}

#[test]
fn bank_listeners_run_from_normal_player_services_and_read_the_actual_active_bank() {
    use rs09_protocol::game::updates::{self, WireItem};
    use rs09_server::inventory::item::Item;
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    for i in 0..8 {
        state.identity().varbits().create(800 + i, 4885 + i, 0, 7);
    }
    let primary = p.bank_primary();
    primary.container().items().write()[0] =
        Some(Shared::new(Item::new(1, 7, 0, &p.definitions())));
    let active = p.bank_secondary();
    active.container().items().write()[0] = Some(Shared::new(Item::new(2, 1, 0, &p.definitions())));
    *active.tab_start_slots().write() = vec![0, 0, 3, 7, 7, 7, 7, 7, 7, 7, 7];
    p.set_use_secondary_bank(true);
    p.manager().fields.write().packet_count = i32::MAX;
    let mut frames = Default::default();
    p.apply(
        &mut state,
        &mut frames,
        || 0,
        |_, defs, ctx| primary.container().refresh(None, defs, ctx),
    )
    .unwrap();
    let mut items = vec![None; 496];
    items[0] = Some(WireItem { id: 1, amount: 7 });
    let mut expected = vec![updates::inventory_full((762 << 16) | 64000, 95, &items).unwrap()];
    for i in 0..8 {
        expected.push(updates::varp(800 + i, [3, 4, 0, 0, 0, 0, 0, 0][i as usize]));
    }
    expected.push(updates::varc(192, 1, i32::MAX));
    assert_eq!(frames[&owner.node], expected);
    assert_eq!(p.variables().get_varp(800), Ok(3));
    assert_eq!(p.variables().get_varp(801), Ok(4));
    assert!(p.variables().saved().unwrap().snapshot().is_empty());
    assert_eq!(p.manager().packet_count(0), i32::MIN);

    // While bank UI is open its exact listener also observes ordinary inventory changes.
    p.inventory().listener_list().clear();
    p.inventory().register(Some(primary.listener()));
    frames.clear();
    p.apply(
        &mut state,
        &mut frames,
        || 0,
        |inventory, defs, ctx| inventory.refresh(None, defs, ctx),
    )
    .unwrap();
    assert_eq!(
        frames[&owner.node],
        vec![updates::inventory_full((763 << 16) | 64000, 93, &vec![None; 28]).unwrap()]
    );
}

#[test]
fn bank_listener_failures_keep_packet_and_completed_variable_prefixes() {
    use rs09_protocol::game::updates;
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    state.identity().varbits().create(800, 4885, 0, 7);
    state.identity().varbits().create(801, 4886, 0, 7);
    *p.bank().tab_start_slots().write() = vec![0, 0, 3, 7, 7, 7, 7, 7, 7, 7, 7];
    let mut frames = Default::default();
    assert!(
        p.apply(
            &mut state,
            &mut frames,
            || 0,
            |_, defs, ctx| p.bank().container().refresh(None, defs, ctx)
        )
        .is_err()
    );
    assert_eq!(
        frames[&owner.node],
        vec![
            updates::inventory_full((762 << 16) | 64000, 95, &vec![None; 496]).unwrap(),
            updates::varp(800, 3),
            updates::varp(801, 4),
        ]
    );
    assert_eq!(p.variables().get_varp(800), Ok(3));
    assert_eq!(p.variables().get_varp(801), Ok(4));
    assert_eq!(p.manager().packet_count(0), 0);
}

#[test]
fn bank_inventory_listener_uses_its_actual_receiver_and_rejects_foreign_worlds() {
    use rs09_protocol::game::updates;
    let mut state = State::default();
    let (a, first) = player(&mut state, 1);
    let (b, second) = player(&mut state, 2);
    first.inventory().listener_list().clear();
    first.inventory().register(Some(second.bank().listener()));
    let mut frames = Default::default();
    first
        .apply(
            &mut state,
            &mut frames,
            || 0,
            |inventory, defs, ctx| inventory.refresh(None, defs, ctx),
        )
        .unwrap();
    assert!(!frames.contains_key(&a.node));
    assert_eq!(
        frames[&b.node],
        vec![updates::inventory_full((763 << 16) | 64000, 93, &vec![None; 28]).unwrap()]
    );
    let mut other = State::default();
    let (_, foreign) = player(&mut other, 2);
    first.inventory().listener_list().clear();
    first.inventory().register(Some(foreign.bank().listener()));
    frames.clear();
    assert!(
        first
            .apply(
                &mut state,
                &mut frames,
                || 0,
                |inventory, defs, ctx| inventory.refresh(None, defs, ctx)
            )
            .is_err()
    );
    assert!(frames.is_empty());
    assert!(foreign.variables().values().unwrap().snapshot().is_empty());
    first.inventory().listener_list().clear();
}

#[test]
fn canonical_variable_archive_is_fixed_before_admission_and_shared_by_real_services() {
    use rs09_cache::{Limits, archive::Archive, store::Store};
    use std::collections::BTreeMap;
    // Minimal valid empty protocol5 index, stored as archive255/group22.
    let archive = || {
        let mut data = vec![0; 1040];
        data[520..537].copy_from_slice(&[0, 22, 0, 0, 0, 0, 0, 255, 0, 0, 0, 0, 4, 5, 0, 0, 0]);
        let mut index = vec![0; 23 * 6];
        index[22 * 6..23 * 6].copy_from_slice(&[0, 0, 9, 0, 0, 1]);
        Arc::new(
            Archive::open(
                Store::from_bytes(data, BTreeMap::from([(255, index)]), Limits::default()),
                22,
                None,
                Limits::default(),
            )
            .unwrap(),
        )
    };
    let mut state = State::default();
    let assets = archive();
    state.bind_varbit_assets(assets.clone()).unwrap();
    state.bind_varbit_assets(assets.clone()).unwrap();
    assert!(state.bind_varbit_assets(archive()).is_err());
    let (owner, p) = player(&mut state, 1);
    state.bind_varbit_assets(assets.clone()).unwrap();
    let mut frames = Default::default();
    p.apply_world_variables(&mut state, &mut frames, |ctx| {
        rs09_server::player::variables::set_varbit(ctx, Some(&owner), 123, 1, false)
    })
    .unwrap();
    assert_eq!(p.variables().get_varp(0), Ok(1));
    assert!(Arc::ptr_eq(
        &state.identity().varbit_assets().unwrap(),
        &assets
    ));
    let mut late = State::default();
    let (_, _) = player(&mut late, 1);
    assert!(late.bind_varbit_assets(assets).is_err());
    assert!(late.identity().varbit_assets().is_none());
}

#[test]
fn variable_failures_preserve_mutations_and_foreign_worlds_reject_before_entry() {
    use rs09_server::player::variables as v;
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let mut frames = Default::default();
    let mut foreign = State::default();
    let mut entered = false;
    let error = p.apply_variables(
        &mut foreign,
        &mut frames,
        &mut MissingVariableAssets,
        |ctx| {
            entered = true;
            v::set_varp(ctx, Some(&owner), 3, 77, true)
        },
    );
    assert!(matches!(error, Err(v::Error::Binding(_))));
    assert!(!entered);
    assert_eq!(p.variables().get_varp(3), Ok(0));
    p.variables().set_saved(None);
    assert_eq!(
        p.apply_variables(&mut state, &mut frames, &mut MissingVariableAssets, |ctx| {
            v::set_varp(ctx, Some(&owner), 3, 88, false)
        }),
        Err(v::Error::Null)
    );
    assert_eq!(p.variables().get_varp(3), Ok(88));
    assert!(frames.is_empty());
    p.variables().set_saved(Some(Default::default()));
    state.actor_mut(owner.node).unwrap().connected = false;
    let (_, replacement) = player(&mut state, 1);
    assert!(matches!(
        p.apply_variables(&mut state, &mut frames, &mut MissingVariableAssets, |ctx| {
            v::set_varp(ctx, Some(&owner), 3, 99, true)
        }),
        Err(v::Error::Binding(_))
    ));
    assert_eq!(p.variables().get_varp(3), Ok(99));
    assert_eq!(p.variables().saved().unwrap().get(3), Some(true));
    assert_eq!(replacement.variables().get_varp(3), Ok(0));
    assert!(frames.is_empty());
}

#[test]
fn varp_manager_retains_its_exact_player_after_disconnect_and_releases_final_cycle() {
    use rs09_game_core::java_json;
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let weak = p.downgrade();
    let manager = p.varp_manager();
    assert!(manager.same(&p.varp_manager()));
    drop(p);
    let old = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(old).is_ok());
    let (_, replacement) = player(&mut state, 1);
    let retained = state.player_fields(&manager.owner()).unwrap();
    assert!(!retained.same(&replacement));
    let data =
        java_json::parse("[{\"index\":3,\"bitArray\":[{\"offset\":31,\"value\":1}]}]").unwrap();
    let mut frames = Default::default();
    retained
        .apply_variables(&mut state, &mut frames, &mut MissingVariableAssets, |ctx| {
            manager.parse(Some(&data), ctx)
        })
        .unwrap();
    assert_eq!(retained.variables().get_varp(3), Ok(i32::MIN));
    assert_eq!(replacement.variables().get_varp(3), Ok(0));
    assert!(frames.is_empty());
    drop(retained);
    drop(manager);
    state.collect();
    assert!(state.actor(old).is_err());
    assert!(weak.upgrade().is_none());
}

#[test]
fn variable_views_retain_player_but_detached_primitive_maps_do_not() {
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let weak = p.downgrade();
    let view = p.variables();
    let values = view.values().unwrap();
    let saved = view.saved().unwrap();
    values.put(4, Some(7));
    saved.put(4, Some(true));
    drop(p);
    let old = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(old).is_ok());
    let (_, replacement) = player(&mut state, 1);
    assert_eq!(view.get_varp(4), Ok(7));
    assert_eq!(replacement.variables().get_varp(4), Ok(0));
    drop(view);
    state.collect();
    assert!(state.actor(old).is_err());
    assert!(weak.upgrade().is_none());
    assert_eq!(values.get(4), Some(7));
    assert_eq!(saved.get(4), Some(true));
    values.put(4, Some(9));
}

#[test]
fn pin_managers_and_distinct_close_events_keep_the_exact_player_until_final_release() {
    use rs09_server::player::bank_pin::{CloseEvent, Status};
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let weak = p.downgrade();
    let manager = p.bank_pin();
    assert!(manager.same(&p.bank_pin()));
    assert!(manager.is_unlocked());
    let fields = manager.fields();
    fields.write().status = Some(Status::Active);
    fields.write().change_state = 2;
    fields.write().deleting = true;
    let digits = fields.read().digits.clone();
    let first = CloseEvent::new(manager.clone());
    let alias = first.clone();
    let distinct = CloseEvent::new(manager.clone());
    assert!(first.same(&alias));
    assert!(!first.same(&distinct));
    drop(p);
    drop(manager);
    let old = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(old).is_ok());
    let (_, replacement) = player(&mut state, 1);
    assert!(!first.manager().same(&replacement.bank_pin()));
    replacement.bank_pin().fields().write().change_state = 7;
    replacement.bank_pin().fields().write().deleting = true;
    assert!(first.close());
    assert_eq!(fields.read().change_state, 0);
    assert!(!fields.read().deleting);
    assert_eq!(replacement.bank_pin().fields().read().change_state, 7);
    assert!(replacement.bank_pin().fields().read().deleting);
    drop(fields);
    drop(first);
    drop(distinct);
    state.collect();
    assert!(state.actor(old).is_ok());
    drop(alias);
    state.collect();
    assert!(state.actor(old).is_err());
    assert!(weak.upgrade().is_none());
    assert_eq!(digits, (0..10).collect::<Vec<_>>());
}

#[test]
fn internal_component_pin_cycles_collect_and_external_component_event_or_array_views_retain() {
    use rs09_server::{player::bank_pin::CloseEvent, world::interfaces::Registry};
    for exposed in 0..4 {
        let mut state = State::default();
        let (owner, p) = player(&mut state, 1);
        let weak = p.downgrade();
        let mut registry = Registry::default();
        let c = registry.component(13);
        let event = CloseEvent::new(p.bank_pin()).component_event();
        let weak_c = c.downgrade();
        let weak_event = event.downgrade();
        c.write().close_event = Some(event.clone());
        let manager = p.manager();
        manager.fields.write().opened = Some(c.clone());
        manager.fields.write().chatbox = Some(c.clone());
        manager.fields.read().tabs.as_ref().unwrap().write()[3] = Some(c.clone());
        let component = (exposed == 1).then(|| c.clone());
        let callback = (exposed == 2).then(|| event.clone());
        let array = (exposed == 3).then(|| manager.fields.read().tabs.clone());
        drop(manager);
        drop(c);
        drop(event);
        drop(p);
        let old = disconnect(&mut state, owner);
        state.collect();
        assert_eq!(state.actor(old).is_ok(), exposed != 0, "exposed={exposed}");
        if exposed != 0 {
            let (_, replacement) = player(&mut state, 1);
            assert!(
                !weak
                    .upgrade()
                    .unwrap()
                    .bank_pin()
                    .same(&replacement.bank_pin())
            );
        }
        drop(component);
        drop(callback);
        drop(array);
        state.collect();
        assert!(state.actor(old).is_err());
        assert!(weak.upgrade().is_none());
        assert!(weak_c.upgrade().is_none());
        assert!(weak_event.upgrade().is_none());
    }
}

#[test]
fn mutual_component_pin_captures_keep_real_targets_and_release_both_disconnected_players() {
    use rs09_server::{player::bank_pin::CloseEvent, world::interfaces::Registry};
    for expose in [false, true] {
        let mut state = State::default();
        let (a, pa) = player(&mut state, 1);
        let (b, pb) = player(&mut state, 2);
        let wa = pa.downgrade();
        let wb = pb.downgrade();
        let mut registry = Registry::default();
        let ca = registry.component(13);
        let cb = registry.component(14);
        let ea = CloseEvent::new(pb.bank_pin()).component_event();
        let eb = CloseEvent::new(pa.bank_pin()).component_event();
        ca.write().close_event = Some(ea.clone());
        cb.write().close_event = Some(eb.clone());
        pa.manager().fields.write().opened = Some(ca.clone());
        pb.manager().fields.write().opened = Some(cb.clone());
        let external = expose.then(|| ea.clone());
        drop(ea);
        drop(eb);
        drop(ca);
        drop(cb);
        drop(pa);
        drop(pb);
        let na = disconnect(&mut state, a);
        let nb = disconnect(&mut state, b);
        state.collect();
        assert_eq!(state.actor(na).is_ok(), expose);
        assert_eq!(state.actor(nb).is_ok(), expose);
        drop(external);
        state.collect();
        assert!(state.actor(na).is_err());
        assert!(state.actor(nb).is_err());
        assert!(wa.upgrade().is_none());
        assert!(wb.upgrade().is_none());
    }
}

#[test]
fn component_bank_captures_are_internal_container_fields_and_release_without_false_roots() {
    use rs09_server::{
        inventory::bank,
        world::{
            component_objects::{BankClose, Close},
            interfaces::Registry,
        },
    };
    for expose in [false, true] {
        let mut state = State::default();
        let (owner, p) = player(&mut state, 1);
        let weak = p.downgrade();
        let weak_bank = p.bank_primary().container().downgrade();
        let mut registry = Registry::default();
        let component = registry.component(762);
        component.write().close_event = Some(Close::new(BankClose(bank::CloseEvent::Bank(
            p.bank_primary(),
        ))));
        p.manager().fields.write().opened = Some(component.clone());
        let external = expose.then(|| component.clone());
        drop(component);
        drop(p);
        let node = disconnect(&mut state, owner);
        state.collect();
        assert_eq!(state.actor(node).is_ok(), expose);
        drop(external);
        state.collect();
        assert!(state.actor(node).is_err());
        assert!(weak.upgrade().is_none());
        assert!(weak_bank.upgrade().is_none());
    }
}

struct ComponentNodes(Vec<NodeHandle>);
impl rs09_server::world::component_objects::NativeObject for ComponentNodes {
    fn captures(&self) -> rs09_server::world::component_objects::Captures {
        rs09_server::world::component_objects::Captures {
            nodes: self.0.clone(),
            ..Default::default()
        }
    }
}
#[test]
fn callback_aliases_count_each_physical_node_field_once_and_respect_an_external_node_lease() {
    use rs09_server::world::{component_objects::Plugin, interfaces::Registry};
    for same in [false, true] {
        let mut state = State::default();
        let (owner, p) = player(&mut state, 1);
        let external = owner.clone();
        let weak = p.downgrade();
        let first = Plugin::new(ComponentNodes(vec![owner.clone(), owner.clone()]));
        let second = if same {
            first.clone()
        } else {
            Plugin::new(ComponentNodes(vec![owner.clone(), owner.clone()]))
        };
        assert_eq!(first.same(&second), same);
        let mut registry = Registry::default();
        registry.for_id(149).write().plugin = Some(first.clone());
        let component = registry.component(149);
        component.write().plugin = Some(second.clone());
        registry.definitions.clear();
        p.manager().fields.write().opened = Some(component.clone());
        drop(first);
        drop(second);
        drop(component);
        drop(p);
        let node = disconnect(&mut state, owner);
        state.collect();
        assert!(state.actor(node).is_ok());
        drop(external);
        state.collect();
        assert!(state.actor(node).is_err());
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn a_component_or_stateless_unclosable_event_does_not_capture_the_player_who_opened_it() {
    use rs09_server::world::{
        component_objects::{Close, Unclosable},
        interfaces::Registry,
    };
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let weak = p.downgrade();
    let mut registry = Registry::default();
    let component = registry.component(149);
    let event = Close::new(Unclosable);
    component.write().close_event = Some(event.clone());
    p.manager().fields.write().opened = Some(component.clone());
    drop(p);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
    assert_eq!(component.read().id, 149);
    assert!(event.value::<Unclosable>().is_some());
}

#[test]
fn a_global_definition_roots_its_captured_player_until_the_actual_registry_reference_is_removed() {
    use rs09_server::world::{component_objects::Plugin, interfaces::Registry};
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let weak = p.downgrade();
    let mut registry = Registry::default();
    let plugin = Plugin::new(ComponentNodes(vec![owner.clone()]));
    let weak_plugin = plugin.downgrade();
    registry.for_id(149).write().plugin = Some(plugin.clone());
    let component = registry.component(149);
    p.manager().fields.write().opened = Some(component.clone());
    drop(component);
    drop(plugin);
    drop(p);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    registry.definitions.clear();
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
    assert!(weak_plugin.upgrade().is_none());
}

#[test]
fn global_interface_callback_aliases_keep_only_their_actual_captured_player() {
    use rs09_server::world::interface_listeners::{Callback, Registry};
    let mut state = State::default();
    let (owner, player) = player(&mut state, 1);
    let registry = Registry::default();
    let callback = Callback::both(ComponentNodes(vec![owner.clone()]));
    registry.add(149, None, callback.clone()).unwrap();
    registry.add_open(149, callback.clone()).unwrap();
    assert!(
        registry
            .get(149, None)
            .unwrap()
            .unwrap()
            .same(&registry.get_open(149).unwrap().unwrap())
    );
    let weak = player.downgrade();
    drop(callback);
    drop(player);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    registry.open_listeners.write().clear();
    state.collect();
    assert!(state.actor(node).is_ok());
    registry.button_listeners.write().clear();
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
}

#[derive(Default)]
struct AttributeFields {
    nodes: Vec<NodeHandle>,
    objects: Vec<rs09_server::world::component_objects::Plugin>,
    attributes: Vec<Arc<dyn rs09_server::player::attributes::AttributeObject>>,
}
impl rs09_server::player::attributes::AttributeObject for AttributeFields {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn java_string(&self) -> String {
        "attribute fixture".into()
    }
    fn captures(&self) -> rs09_server::world::component_objects::Captures {
        rs09_server::world::component_objects::Captures {
            nodes: self.nodes.clone(),
            objects: self.objects.clone(),
            attributes: self.attributes.clone(),
            ..Default::default()
        }
    }
}
impl rs09_server::world::component_objects::NativeObject for AttributeFields {
    fn captures(&self) -> rs09_server::world::component_objects::Captures {
        rs09_server::player::attributes::AttributeObject::captures(self)
    }
}
#[test]
fn native_and_attribute_trait_views_of_one_arc_count_one_physical_capture_object() {
    use rs09_server::world::{
        component_objects::{Close, Plugin},
        interfaces::Registry,
    };
    for close in [false, true] {
        for keep in [false, true] {
            let mut state = State::default();
            let (owner, p) = player(&mut state, 1);
            let mut registry = Registry::default();
            let component = registry.component(149);
            let payload = AttributeFields {
                nodes: vec![owner.clone(), owner.clone()],
                ..Default::default()
            };
            let value = if close {
                let event = Close::new(payload);
                let value = event.value::<AttributeFields>().unwrap();
                component.write().close_event = Some(event);
                value
            } else {
                let plugin = Plugin::new(payload);
                let value = plugin.value::<AttributeFields>().unwrap();
                component.write().plugin = Some(plugin);
                value
            };
            let weak = Arc::downgrade(&value);
            p.attributes()
                .set_value("runscript", Some(object_value(value.clone())));
            p.attributes()
                .set_value("alias", Some(object_value(value.clone())));
            p.manager().fields.write().opened = Some(component.clone());
            let external = keep.then(|| owner.clone());
            drop(value);
            drop(component);
            drop(registry);
            drop(p);
            let node = disconnect(&mut state, owner);
            state.collect();
            assert_eq!(state.actor(node).is_ok(), keep);
            drop(external);
            state.collect();
            assert!(state.actor(node).is_err());
            assert!(weak.upgrade().is_none());
        }
    }
}
fn object_value(
    value: Arc<dyn rs09_server::player::attributes::AttributeObject>,
) -> rs09_server::player::attributes::AttributeValue {
    rs09_server::player::attributes::AttributeValue::Object(value)
}

#[derive(Default)]
struct ScriptFields {
    nodes: Vec<NodeHandle>,
    processors: Vec<rs09_server::scripts::Runtime>,
    scripts: Vec<rs09_server::scripts::RuntimeScript>,
}
impl rs09_server::world::component_objects::NativeObject for ScriptFields {
    fn captures(&self) -> rs09_server::world::component_objects::Captures {
        rs09_server::world::component_objects::Captures {
            nodes: self.nodes.clone(),
            processors: self.processors.clone(),
            scripts: self.scripts.clone(),
            ..Default::default()
        }
    }
}
#[test]
fn canonical_script_processor_aliases_retain_exact_owner_and_current_target_then_release() {
    let mut state = State::default();
    let (one, p) = player(&mut state, 1);
    let (two, q) = player(&mut state, 2);
    let scripts = p.scripts();
    assert!(scripts.same(&p.scripts()));
    assert!(scripts.entity().is_some_and(|n| n == one));
    scripts.set_target(Some(two.clone()));
    let alias = scripts.clone();
    alias.set_delay(42);
    assert_eq!(p.scripts().delay(), 42);
    drop(p);
    drop(q);
    let a = disconnect(&mut state, one);
    let b = disconnect(&mut state, two);
    state.collect();
    assert!(state.actor(a).is_ok() && state.actor(b).is_ok());
    drop(scripts);
    state.collect();
    assert!(state.actor(a).is_ok() && state.actor(b).is_ok());
    alias.set_target(None);
    state.collect();
    assert!(state.actor(a).is_ok() && state.actor(b).is_err());
    drop(alias);
    state.collect();
    assert!(state.actor(a).is_err());
}
#[test]
fn native_script_function_processor_cycle_collects_and_external_script_or_payload_views_root_it() {
    use rs09_server::{
        scripts::Form,
        world::interaction_listeners::{Function, FunctionKind},
    };
    for keep in 0..3 {
        let mut state = State::default();
        let (owner, p) = player(&mut state, 1);
        let weak_player = p.downgrade();
        let processor = p.scripts();
        let f = Function::new(
            FunctionKind::Interaction,
            "owned.ScriptFunction",
            ScriptFields {
                nodes: vec![owner.clone(), owner.clone()],
                processors: vec![processor.clone(), processor.clone()],
                ..Default::default()
            },
        );
        let weak_function = f.header().downgrade();
        let script = processor
            .create_script(Form::Interaction { distance: 1 }, Some(f.clone()), false)
            .unwrap();
        let weak_script = script.downgrade();
        processor.set_approach(Some(script.clone()));
        processor.set_operable(Some(script.clone()));
        processor.set_current(Some(script.clone()));
        processor.set_target(Some(owner.clone()));
        let held_script = (keep == 1).then(|| script.clone());
        let held_payload = (keep == 2).then(|| f.value::<ScriptFields>().unwrap());
        drop(script);
        drop(f);
        drop(processor);
        drop(p);
        let node = disconnect(&mut state, owner);
        state.collect();
        assert_eq!(state.actor(node).is_ok(), keep != 0);
        assert_eq!(weak_player.upgrade().is_some(), keep != 0);
        drop(held_script);
        drop(held_payload);
        state.collect();
        assert!(state.actor(node).is_err() && weak_player.upgrade().is_none());
        assert!(weak_script.upgrade().is_none());
        assert!(weak_function.upgrade().is_none());
    }
}
#[test]
fn script_form_and_native_header_fields_keep_their_actual_players_without_duplicates() {
    use rs09_server::{
        scripts::Form,
        world::interaction_listeners::{Function, FunctionKind},
    };
    let mut state = State::default();
    let (one, p) = player(&mut state, 1);
    let (two, q) = player(&mut state, 2);
    let processor = p.scripts();
    let f = Function::new(
        FunctionKind::UseInteraction,
        "owned.UseFunction",
        ScriptFields {
            nodes: vec![two.clone()],
            ..Default::default()
        },
    );
    let script = processor
        .create_script(
            Form::UseWith {
                distance: 1,
                used: one.clone(),
                with: two.clone(),
            },
            Some(f),
            false,
        )
        .unwrap();
    processor.set_approach(Some(script.clone()));
    drop(p);
    drop(q);
    drop(processor);
    let a = disconnect(&mut state, one);
    let b = disconnect(&mut state, two);
    state.collect();
    assert!(state.actor(a).is_ok() && state.actor(b).is_ok());
    // A detached Script does not acquire a hidden owner anchor. Its form and
    // actual function captures alone retain these exact nodes.
    script.write().form = Form::Basic;
    state.collect();
    assert!(state.actor(a).is_err() && state.actor(b).is_ok());
    script.write().execution = None;
    state.collect();
    assert!(state.actor(b).is_err());
}
#[test]
fn local_collection_never_clears_a_foreign_processor_or_its_script_fields() {
    use rs09_server::{
        scripts::Form,
        world::component_objects::Plugin,
        world::interaction_listeners::{Function, FunctionKind},
    };
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let mut other = State::default();
    let (foreign, q) = player(&mut other, 1);
    let processor =
        rs09_server::scripts::Runtime::with_entity_in(foreign.clone(), other.identity());
    let f = Function::new(
        FunctionKind::Interaction,
        "foreign.Script",
        ScriptFields {
            processors: vec![processor.clone()],
            ..Default::default()
        },
    );
    let script = processor
        .create_script(Form::Interaction { distance: 1 }, Some(f), false)
        .unwrap();
    processor.set_approach(Some(script.clone()));
    processor.set_delay(77);
    let weak = processor.fields.downgrade();
    let weak_script = script.downgrade();
    let object = Plugin::new(ScriptFields {
        processors: vec![processor.clone()],
        scripts: vec![script.clone()],
        ..Default::default()
    });
    p.manager().fields.write().opened =
        Some(rs09_server::world::interfaces::Registry::default().component(210));
    p.manager()
        .fields
        .read()
        .opened
        .as_ref()
        .unwrap()
        .write()
        .plugin = Some(object);
    drop(p);
    drop(processor);
    drop(script);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_err());
    let processor = rs09_server::scripts::Runtime {
        fields: weak
            .upgrade()
            .expect("foreign processor was incorrectly reaped"),
    };
    let script = weak_script
        .upgrade()
        .expect("foreign script was incorrectly reaped");
    assert_eq!(processor.delay(), 77);
    assert!(processor.approach().is_some());
    assert!(script.read().execution.is_some());
    assert!(other.actor(foreign.node).is_ok());
    processor.set_approach(None);
    drop(script);
    drop(processor);
    drop(q);
}
#[test]
fn canonical_script_domain_and_targets_reject_before_entering_mutating_operations() {
    use rs09_server::{
        scripts::{Form, Runtime, Script},
        world::interaction_listeners::Function,
    };
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let processor = p.scripts();
    let mut other = State::default();
    let (foreign, q) = player(&mut other, 1);
    assert_eq!(owner.node, foreign.node);
    processor.set_delay(7);
    assert!(
        processor
            .with_world(&mut other, |p, _| {
                p.set_delay(999);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(processor.delay(), 7);
    processor
        .with_world(&mut state, |p, _| {
            p.set_delay(8);
            Ok(())
        })
        .unwrap();
    assert_eq!(processor.delay(), 8);
    let forged = Runtime::with_entity_in(foreign.clone(), state.identity());
    assert!(forged.validate_in(&state).is_err());
    let foreign_script = q.scripts().create_script(Form::Basic, None, false).unwrap();
    assert!(
        processor
            .validate_script_in(&state, &foreign_script)
            .is_err()
    );
    let foreign_target = processor
        .create_script(
            Form::UseWith {
                distance: 1,
                used: owner.clone(),
                with: foreign,
            },
            None,
            false,
        )
        .unwrap();
    assert!(
        processor
            .validate_script_in(&state, &foreign_target)
            .is_err()
    );
    let unbound = Shared::new(Script::<Function, NodeHandle>::new(
        Form::Basic,
        None,
        false,
    ));
    assert!(processor.validate_script_in(&state, &unbound).is_err());
    assert!(
        processor.current().is_none()
            && processor.approach().is_none()
            && processor.target().is_none()
    );
}

struct ReapDrop {
    processor: rs09_server::scripts::Runtime,
    seen: Arc<std::sync::atomic::AtomicBool>,
}
impl rs09_server::world::component_objects::NativeObject for ReapDrop {
    fn captures(&self) -> rs09_server::world::component_objects::Captures {
        rs09_server::world::component_objects::Captures {
            processors: vec![self.processor.clone()],
            ..Default::default()
        }
    }
}
impl Drop for ReapDrop {
    fn drop(&mut self) {
        assert_eq!(self.processor.delay(), 91);
        self.seen.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
#[test]
fn unreachable_script_cycle_payload_destructors_can_inspect_their_original_receiver() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use rs09_server::{
            scripts::Form,
            world::interaction_listeners::{Function, FunctionKind},
        };
        let mut state = State::default();
        let (owner, p) = player(&mut state, 1);
        let processor = p.scripts();
        processor.set_delay(91);
        let seen = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = Function::new(
            FunctionKind::Interaction,
            "native.GcDrop",
            ReapDrop {
                processor: processor.clone(),
                seen: seen.clone(),
            },
        );
        let script = processor
            .create_script(Form::Interaction { distance: 1 }, Some(f), false)
            .unwrap();
        processor.set_approach(Some(script));
        drop(processor);
        drop(p);
        let node = disconnect(&mut state, owner);
        state.collect();
        assert!(state.actor(node).is_err());
        assert!(seen.load(std::sync::atomic::Ordering::SeqCst));
        tx.send(()).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("collector dropped a native callback while holding its receiver guard");
}
#[test]
fn detached_stateless_script_never_anchors_the_player_that_stored_it() {
    use rs09_server::{
        scripts::Form,
        world::interaction_listeners::{Function, FunctionKind},
    };
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let processor = p.scripts();
    let f = Function::new(
        FunctionKind::Interaction,
        "native.Stateless",
        ScriptFields::default(),
    );
    let script = processor
        .create_script(Form::Interaction { distance: 1 }, Some(f), false)
        .unwrap();
    processor.set_approach(Some(script.clone()));
    drop(processor);
    drop(p);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(script.read().execution.is_some());
}
#[test]
fn foreign_script_scope_conservatively_preserves_its_actual_local_captures() {
    use rs09_server::{
        scripts::Form,
        world::{
            component_objects::Plugin,
            interaction_listeners::{Function, FunctionKind},
        },
    };
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let mut other = State::default();
    let (_, q) = player(&mut other, 1);
    let f = Function::new(
        FunctionKind::Interaction,
        "foreign.LocalCapture",
        ScriptFields {
            nodes: vec![owner.clone()],
            ..Default::default()
        },
    );
    let script = q
        .scripts()
        .create_script(Form::Interaction { distance: 1 }, Some(f), false)
        .unwrap();
    let c = rs09_server::world::interfaces::Registry::default().component(210);
    c.write().plugin = Some(Plugin::new(ScriptFields {
        scripts: vec![script],
        ..Default::default()
    }));
    p.manager().fields.write().opened = Some(c);
    drop(p);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    // Raw foreign captures are a conservative boundary. Removing the actual
    // reference releases the local actor; local collection never edits it.
    let retained = state.retain_node(node).unwrap();
    let p = state.player_fields(&retained).unwrap();
    p.manager().fields.write().opened = None;
    drop(p);
    drop(retained);
    state.collect();
    assert!(state.actor(node).is_err());
}
#[test]
fn attribute_aliases_count_physical_fields_and_release_self_captures() {
    for keep in [false, true] {
        let mut state = State::default();
        let (owner, p) = player(&mut state, 1);
        let weak = p.downgrade();
        let callback = Arc::new(AttributeFields {
            nodes: vec![owner.clone(), owner.clone()],
            ..Default::default()
        });
        p.attributes()
            .set_value("runscript", Some(object_value(callback.clone())));
        p.attributes()
            .set_value("alias", Some(object_value(callback.clone())));
        let external = keep.then(|| callback.clone());
        drop(callback);
        drop(p);
        let node = disconnect(&mut state, owner);
        state.collect();
        assert_eq!(state.actor(node).is_ok(), keep);
        drop(external);
        state.collect();
        assert!(state.actor(node).is_err() && weak.upgrade().is_none());
    }
}
#[test]
fn attributes_trace_cross_player_cycles_and_actual_external_alias_roots() {
    for keep in [false, true] {
        let mut state = State::default();
        let (a, one) = player(&mut state, 1);
        let (b, two) = player(&mut state, 2);
        let first = Arc::new(AttributeFields {
            nodes: vec![b.clone()],
            ..Default::default()
        });
        let second = Arc::new(AttributeFields {
            nodes: vec![a.clone()],
            ..Default::default()
        });
        one.attributes()
            .set_value("runscript", Some(object_value(first.clone())));
        two.attributes()
            .set_value("runscript", Some(object_value(second.clone())));
        let external = keep.then(|| first.clone());
        drop(first);
        drop(second);
        drop(one);
        drop(two);
        let a = disconnect(&mut state, a);
        let b = disconnect(&mut state, b);
        state.collect();
        assert_eq!(state.actor(a).is_ok(), keep);
        assert_eq!(state.actor(b).is_ok(), keep);
        drop(external);
        state.collect();
        assert!(state.actor(a).is_err() && state.actor(b).is_err());
    }
}
#[test]
fn nested_attribute_and_plugin_fields_share_one_object_without_false_roots() {
    use rs09_server::world::component_objects::Plugin;
    for keep in [false, true] {
        let mut state = State::default();
        let (a, one) = player(&mut state, 1);
        let (b, two) = player(&mut state, 2);
        let plugin = Plugin::new(ComponentNodes(vec![b.clone(), b.clone()]));
        let child = Arc::new(AttributeFields {
            objects: vec![plugin.clone(), plugin.clone()],
            ..Default::default()
        });
        let parent = Arc::new(AttributeFields {
            attributes: vec![child.clone(), child.clone()],
            ..Default::default()
        });
        one.attributes()
            .set_value("runscript", Some(object_value(parent.clone())));
        two.attributes()
            .set_value("alias", Some(object_value(child.clone())));
        let external = keep.then(|| plugin.clone());
        drop(parent);
        drop(child);
        drop(plugin);
        drop(one);
        drop(two);
        let a = disconnect(&mut state, a);
        let b = disconnect(&mut state, b);
        state.collect();
        assert!(state.actor(a).is_err());
        assert_eq!(state.actor(b).is_ok(), keep);
        drop(external);
        state.collect();
        assert!(state.actor(b).is_err());
    }
}
#[test]
fn a_returned_stateless_attribute_value_does_not_anchor_its_owner() {
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let object = Arc::new(AttributeFields::default());
    p.attributes()
        .set_value("/save:object", Some(object_value(object.clone())));
    let retained = p.attributes().get("object").unwrap();
    let rs09_server::player::attributes::AttributeValue::Object(alias) = &retained else {
        panic!()
    };
    assert!(std::ptr::eq(
        alias.as_any().downcast_ref::<AttributeFields>().unwrap(),
        object.as_ref()
    ));
    drop(p);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_err());
    drop(retained);
    drop(object);
}
#[test]
fn replacing_an_attribute_preserves_only_the_exposed_old_objects_captures() {
    let mut state = State::default();
    let (a, one) = player(&mut state, 1);
    let (b, two) = player(&mut state, 2);
    one.attributes().set_value(
        "runscript",
        Some(object_value(Arc::new(AttributeFields {
            nodes: vec![b.clone()],
            ..Default::default()
        }))),
    );
    let old = one.attributes().get("runscript");
    one.attributes().set_value(
        "runscript",
        Some(object_value(Arc::new(AttributeFields::default()))),
    );
    drop(one);
    drop(two);
    let a = disconnect(&mut state, a);
    let b = disconnect(&mut state, b);
    state.collect();
    assert!(state.actor(a).is_err());
    assert!(state.actor(b).is_ok());
    drop(old);
    state.collect();
    assert!(state.actor(b).is_err());
}
struct InspectAttribute {
    owner: NodeHandle,
    player: rs09_server::player::owned::WeakPlayer,
}
impl rs09_server::player::attributes::AttributeObject for InspectAttribute {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn java_string(&self) -> String {
        "inspect fixture".into()
    }
    fn captures(&self) -> rs09_server::world::component_objects::Captures {
        let p = self.player.upgrade().unwrap();
        assert_eq!(p.attributes().integer("sentinel", -1).unwrap(), 42);
        rs09_server::world::component_objects::Captures {
            nodes: vec![self.owner.clone()],
            ..Default::default()
        }
    }
}
#[test]
fn attribute_capture_metadata_runs_after_player_field_guards_are_released() {
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    p.attributes().set_integer("sentinel", 42);
    p.attributes().set_value(
        "runscript",
        Some(object_value(Arc::new(InspectAttribute {
            owner: owner.clone(),
            player: p.downgrade(),
        }))),
    );
    drop(p);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_err());
}

#[test]
fn real_bank_transfer_fields_trace_captured_player_and_after_function_separately() {
    use rs09_server::world::{component_objects::Plugin, input_prompt::Transfer};
    for keep_after in [false, true] {
        let mut state = State::default();
        let (a, one) = player(&mut state, 1);
        let (b, two) = player(&mut state, 2);
        let after = Plugin::new(ComponentNodes(vec![b.clone()]));
        let transfer = Plugin::new(Transfer {
            player: a.clone(),
            slot: 8,
            withdraw: false,
            after: Some(after.clone()),
        });
        one.attributes().set_value(
            "runscript",
            Some(object_value(Arc::new(AttributeFields {
                objects: vec![transfer.clone()],
                ..Default::default()
            }))),
        );
        let exposed = keep_after.then(|| after.clone());
        drop(transfer);
        drop(after);
        drop(one);
        drop(two);
        let a = disconnect(&mut state, a);
        let b = disconnect(&mut state, b);
        state.collect();
        assert!(state.actor(a).is_err());
        assert_eq!(state.actor(b).is_ok(), keep_after);
        drop(exposed);
        state.collect();
        assert!(state.actor(b).is_err());
    }
}

#[test]
fn a_shared_tab_array_roots_only_the_players_actually_captured_by_its_components() {
    use rs09_server::{
        definitions::item::Shared, player::bank_pin::CloseEvent, world::interfaces::Registry,
    };
    let mut state = State::default();
    let (a, pa) = player(&mut state, 1);
    let (b, pb) = player(&mut state, 2);
    let wa = pa.downgrade();
    let wb = pb.downgrade();
    let mut registry = Registry::default();
    let component = registry.component(13);
    component.write().close_event = Some(CloseEvent::new(pa.bank_pin()).component_event());
    let array = Shared::new(vec![Some(component.clone())]);
    pa.manager().fields.write().tabs = Some(array.clone());
    pb.manager().fields.write().tabs = Some(array.clone());
    drop(component);
    drop(pa);
    drop(pb);
    let na = disconnect(&mut state, a);
    let nb = disconnect(&mut state, b);
    state.collect();
    assert!(state.actor(na).is_ok());
    assert!(state.actor(nb).is_err());
    assert!(wb.upgrade().is_none());
    drop(array);
    state.collect();
    assert!(state.actor(na).is_err());
    assert!(wa.upgrade().is_none());
}

#[test]
fn active_bank_ownership_queries_use_real_primary_secondary_and_equipment_state() {
    use rs09_server::inventory::{self, item::Item};
    let mut state = State::default();
    let (_, p) = player(&mut state, 1);
    let primary = p.bank_primary();
    let secondary = p.bank_secondary();
    let drop_log = p.drop_log();
    assert!(p.bank().same(&primary));
    assert!(!p.use_secondary_bank());
    assert!(!primary.same(&secondary) && !primary.same(&drop_log) && !secondary.same(&drop_log));
    for bank in [&primary, &secondary, &drop_log] {
        assert_eq!(bank.container().capacity(), 496);
        assert_eq!(bank.container().kind(), ContainerType::AlwaysStack);
        assert_eq!(bank.container().sort(), SortType::Hash);
        assert!(bank.owner().as_ref() == Some(&p.owner()));
    }
    let item = Shared::new(Item::new(1, 1, 1000, &p.definitions()));
    assert!(!p.has_item(Some(&item)).unwrap());
    drop_log.container().items().write()[0] = Some(item.clone());
    assert!(!p.has_item(Some(&item)).unwrap());
    primary.container().items().write()[0] = Some(item.clone());
    assert!(p.has_item(Some(&item)).unwrap());
    p.set_use_secondary_bank(true);
    assert!(p.bank().same(&secondary));
    assert!(!p.has_item(Some(&item)).unwrap());
    secondary.container().items().write()[0] = Some(item.clone());
    assert!(p.has_item(Some(&item)).unwrap());
    secondary.container().items().write()[0] = None;
    p.equipment().items().write()[3] = Some(item.clone());
    assert!(p.has_item(Some(&item)).unwrap());
    p.equipment().items().write()[3] = None;
    p.inventory().items().write()[0] = Some(item.clone());
    assert!(p.has_item(Some(&item)).unwrap());
    assert_eq!(p.has_item(None), Err(inventory::Error::Null));
}

#[test]
fn bank_body_and_final_listener_retain_exact_disconnected_owner_and_release_cycles() {
    for selected in 0..3 {
        let mut state = State::default();
        let (owner, p) = player(&mut state, 1);
        let bank = match selected {
            0 => p.bank_primary(),
            1 => p.bank_secondary(),
            _ => p.drop_log(),
        };
        let weak = p.downgrade();
        let weak_bank = bank.container().downgrade();
        let listener = bank.listener();
        let tabs = bank.tab_start_slots();
        bank.container().listener_list().clear();
        drop(p);
        let old = disconnect(&mut state, owner);
        state.collect();
        assert!(state.actor(old).is_ok());
        let (new_owner, new_player) = player(&mut state, 1);
        assert_ne!(bank.owner().unwrap().node, new_owner.node);
        assert!(
            state
                .player_fields(&bank.owner().unwrap())
                .unwrap()
                .same(&weak.upgrade().unwrap())
        );
        assert!(
            !state
                .player_fields(&bank.owner().unwrap())
                .unwrap()
                .same(&new_player)
        );
        drop(bank);
        state.collect();
        assert!(state.actor(old).is_ok());
        drop(listener);
        state.collect();
        assert!(state.actor(old).is_err());
        assert!(weak.upgrade().is_none());
        assert!(weak_bank.upgrade().is_none());
        // The detached primitive tab array has no player backreference.
        assert_eq!(tabs.read().len(), 11);
    }
}

#[test]
fn ironman_manager_retains_its_actual_player_then_releases_without_a_backreference_cycle() {
    use rs09_server::player::ironman::Mode;
    let mut state = State::default();
    let (owner, p) = player(&mut state, 1);
    let weak = p.downgrade();
    let manager = p.ironman();
    assert!(manager.same(&p.ironman()));
    assert_eq!(manager.mode(), Some(Mode::None));
    manager.set_mode(Some(Mode::Hardcore));
    drop(p);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert_eq!(
        state
            .player_fields(&manager.player().unwrap())
            .unwrap()
            .ironman()
            .mode(),
        Some(Mode::Hardcore)
    );
    let (_, replacement) = player(&mut state, 1);
    assert!(!manager.same(&replacement.ironman()));
    assert_eq!(replacement.ironman().mode(), Some(Mode::None));
    drop(manager);
    state.collect();
    assert!(weak.upgrade().is_none());
    assert!(state.actor(node).is_err());
}

#[test]
fn a_captured_typed_listener_keeps_exact_canonical_player_fields_after_disconnect() {
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let weak = fields.downgrade();
    let listener = fields.inventory().listener_list().get(0).unwrap().unwrap();
    let receiver = listener.value::<Listener>().unwrap();
    drop(listener);
    drop(fields);
    let node = disconnect(&mut state, owner);
    state.collect();
    let retained = state.player_fields(&receiver.owner().unwrap()).unwrap();
    retained.settings().fields.write().weight = 17.5;
    assert_eq!(
        weak.upgrade().unwrap().settings().fields.read().weight,
        17.5
    );
    drop(retained);
    drop(receiver);
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
}

#[test]
fn an_exposed_inventory_list_retains_listeners_but_an_empty_list_does_not_root_the_player() {
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let weak = fields.downgrade();
    let list = fields.inventory().listener_list();
    drop(fields);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    list.clear();
    state.collect();
    assert!(state.actor(node).is_err());
    assert!(weak.upgrade().is_none());
    assert!(list.is_empty());
}

#[test]
fn an_inventory_reference_follows_its_actual_listener_edges_after_removal() {
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let inventory = fields.inventory();
    drop(fields);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    inventory.listener_list().clear();
    state.collect();
    assert!(state.actor(node).is_err());
    assert_eq!(inventory.capacity(), 28);
    assert!(inventory.empty());
}

#[test]
fn settings_and_interface_receivers_keep_original_player_fields_without_listeners() {
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    let weak = fields.downgrade();
    let settings = fields.settings();
    let manager = fields.manager();
    fields.inventory().listener_list().clear();
    drop(fields);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_ok());
    drop(settings);
    state.collect();
    assert!(state.actor(node).is_ok());
    assert_eq!(manager.packet_count(1), 0);
    drop(manager);
    state.collect();
    assert!(weak.upgrade().is_none());
    assert!(state.actor(node).is_err());
}

#[test]
fn interface_manager_has_one_fixed_physical_player_field_and_rejects_rebinding() {
    let mut state = State::default();
    let (a, first) = player(&mut state, 1);
    let (b, second) = player(&mut state, 2);
    let manager = first.manager();
    assert!(manager.player().as_ref() == Some(&a));
    assert!(
        Player::new(
            state.identity(),
            second.core(),
            second.definitions(),
            manager.clone(),
            second.pouch(),
            second.masks()
        )
        .is_err()
    );
    assert!(manager.player().as_ref() == Some(&a));
    first.inventory().listener_list().clear();
    second.inventory().listener_list().clear();
    let weak = first.downgrade();
    let weak_other = second.downgrade();
    drop(first);
    drop(second);
    let a_node = disconnect(&mut state, a);
    let b_node = disconnect(&mut state, b);
    state.collect();
    assert!(state.actor(a_node).is_ok());
    assert!(state.actor(b_node).is_err());
    assert!(weak_other.upgrade().is_none());
    drop(manager);
    state.collect();
    assert!(state.actor(a_node).is_err());
    assert!(weak.upgrade().is_none());
}

#[test]
fn single_overlay_and_wilderness_fields_trace_only_their_actual_captured_player() {
    use rs09_server::{player::bank_pin::CloseEvent, world::interfaces::Registry};
    for field in 0..3 {
        let mut state = State::default();
        let (a, first) = player(&mut state, 1);
        let (b, second) = player(&mut state, 2);
        let component = Registry::default().component(149);
        component.write().close_event = Some(CloseEvent::new(second.bank_pin()).component_event());
        let manager = first.manager();
        {
            let mut fields = manager.fields.write();
            match field {
                0 => fields.single_tab = Some(component.clone()),
                1 => fields.overlay = Some(component.clone()),
                _ => fields.wildy_overlay = Some(component.clone()),
            }
            fields.tabs = None;
        }
        drop(manager);
        drop(first);
        drop(second);
        let a_node = disconnect(&mut state, a);
        let b_node = disconnect(&mut state, b);
        state.collect();
        assert!(state.actor(a_node).is_err());
        assert!(state.actor(b_node).is_ok());
        drop(component);
        state.collect();
        assert!(state.actor(b_node).is_err());
    }
}

#[test]
fn disconnected_player_listener_cycles_release_and_external_list_roots_trace_both_players() {
    for keep_list in [false, true] {
        let mut state = State::default();
        let (a, first) = player(&mut state, 1);
        let (b, second) = player(&mut state, 2);
        let first_weak = first.downgrade();
        let second_weak = second.downgrade();
        let one = first.inventory().listener_list();
        let two = second.inventory().listener_list();
        let a_listener = one.get(0).unwrap();
        let b_listener = two.get(0).unwrap();
        one.clear();
        two.clear();
        one.add(b_listener);
        two.add(a_listener);
        let exposed = keep_list.then(|| one.clone());
        drop(one);
        drop(two);
        drop(first);
        drop(second);
        let a = disconnect(&mut state, a);
        let b = disconnect(&mut state, b);
        state.collect();
        assert_eq!(state.actor(a).is_ok(), keep_list);
        assert_eq!(state.actor(b).is_ok(), keep_list);
        drop(exposed);
        state.collect();
        assert!(state.actor(a).is_err() && state.actor(b).is_err());
        assert!(first_weak.upgrade().is_none() && second_weak.upgrade().is_none());
    }
}

#[test]
fn original_self_facing_contexts_are_internal_edges_and_do_not_root_a_disconnected_player() {
    let mut state = State::default();
    let (owner, fields) = player(&mut state, 1);
    fields
        .masks()
        .face_entity(Some(
            rs09_server::world::prepared_masks::FaceTarget::Player(
                owner.clone(),
                fields.masks().owner(),
            ),
        ))
        .unwrap();
    drop(fields);
    let node = disconnect(&mut state, owner);
    state.collect();
    assert!(state.actor(node).is_err());
}

#[test]
fn fields_cannot_rebind_to_a_foreign_world_or_replace_a_canonical_player() {
    let mut first = State::default();
    let (a, fields) = player(&mut first, 1);
    let mut other = State::default();
    let (b, foreign) = player(&mut other, 1);
    assert_eq!(a.node, b.node);
    assert!(first.bind_player_fields(foreign).is_err());
    assert!(first.player_fields(&b).is_err());
    first.bind_player_fields(fields.clone()).unwrap();
    let replacement = Player::new(
        first.identity(),
        fields.core(),
        fields.definitions(),
        fields.manager(),
        fields.pouch(),
        fields.masks(),
    )
    .unwrap();
    assert!(first.bind_player_fields(replacement).is_err());
    assert!(first.player_fields(&a).unwrap().same(&fields));
}

#[test]
fn a_listener_on_another_container_uses_its_own_players_weight_counter_and_appearance() {
    use rs09_protocol::game::updates::ServerUpdate;
    use rs09_server::{
        definitions::{item::ItemDefinition, property::Property},
        inventory::item::Item,
    };
    let mut defs = Registry::default();
    for (id, weight) in [(1, 4.25), (12047, 1.75)] {
        let mut d = ItemDefinition {
            id,
            ..Default::default()
        };
        d.handlers.insert("weight".into(), Property::Double(weight));
        defs.insert(id, Shared::new(d));
    }
    let defs = Arc::new(defs);
    let factory = Factory::new(&defs);
    let mut state = State::default();
    let (_, a) = player_with(&mut state, 1, defs.clone(), &factory);
    let (b_owner, b) = player_with(&mut state, 2, defs.clone(), &factory);
    b.inventory().items().write()[0] = Some(Shared::new(Item::new(1, 100, 1000, &defs)));
    let list = a.inventory().listener_list();
    list.clear();
    list.add(b.inventory().listener_list().get(0).unwrap());
    let pouch = Shared::new(Item::new(12047, 1, 1000, &defs));
    let mut frames = std::collections::BTreeMap::new();
    a.apply(
        &mut state,
        &mut frames,
        || 2000,
        |inventory, defs, context| inventory.add(Some(&pouch), true, -1, defs, context),
    )
    .unwrap();
    assert_eq!(a.settings().fields.read().weight, 0.0);
    assert_eq!(b.settings().fields.read().weight, 4.25);
    assert!(!*a.pouch().read());
    assert!(*b.pouch().read());
    assert!(!a.masks().body().read().using_summoning);
    assert!(b.masks().body().read().using_summoning);
    assert_eq!(a.manager().fields.read().packet_count, 0);
    assert_eq!(b.manager().fields.read().packet_count, 1);
    assert_eq!(frames.len(), 1);
    assert_eq!(
        frames[&b_owner.node]
            .iter()
            .map(|f| f.opcode)
            .collect::<Vec<_>>(),
        [22, 171]
    );
    assert!(
        matches!(ServerUpdate::decode(&frames[&b_owner.node][1]).unwrap(), Some(ServerUpdate::InterfaceText { text, counter: 0, .. }) if text == b"4 kg")
    );
}
