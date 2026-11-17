use rs09_client::{
    pack::Pack,
    ui::{
        Component,
        item::ItemType,
        state::{Key, State},
        vm::Vm,
    },
};
use serde_json::Value;
fn ints(v: &Value) -> Vec<i32> {
    serde_json::from_value(v.clone()).unwrap()
}
fn pack() -> Pack {
    let manifest =
        br#"{"source":"Explicit CS1 varbit metadata","files":{"varbit/0":[0,6],"varbit/1":[6,6]}}"#;
    let mut bytes = b"RS09VIS1".to_vec();
    bytes.extend((manifest.len() as u32).to_le_bytes());
    bytes.extend(manifest);
    bytes.extend([1, 0, 2, 2, 5, 0, 1, 0, 0, 0, 31, 0]);
    Pack::decode(bytes).unwrap()
}
#[test]
fn complete_original_cs1_execution_and_conditions_match() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui/cs1.json");
    let corpus: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
    for (i, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let mut state = State::new(pack());
        state.loaded.insert(1);
        state.components.insert(
            Key::component(65538),
            Component {
                id: 65538,
                kind: 2,
                obj_types: serde_json::from_value(case["ids"].clone()).unwrap(),
                obj_counts: serde_json::from_value(case["counts"].clone()).unwrap(),
                ..Component::default()
            },
        );
        for id in [100, 101, 102] {
            state.items.types.insert(
                id,
                ItemType {
                    id,
                    members: id == 101,
                    ..ItemType::default()
                },
            );
        }
        let x = ints(&case["context"]);
        let mut vm = Vm::default();
        vm.members = x[0] != 0;
        vm.combat_level = x[1];
        vm.run_energy = x[2];
        vm.weight = x[3];
        vm.location = [
            (x[4] >> 7).wrapping_add(x[6]),
            (x[5] >> 7).wrapping_add(x[7]),
            0,
        ];
        vm.boosted_levels = ints(&case["boosted"]).try_into().unwrap();
        vm.base_levels = ints(&case["base"]).try_into().unwrap();
        vm.experience = ints(&case["xp"]).try_into().unwrap();
        for (id, n) in [0, 1, 2, 3499].into_iter().zip(ints(&case["varps"])) {
            vm.varps.insert(id, n);
        }
        // The two real casket-condition cases use the same controlled value.
        vm.varps.insert(261, ints(&case["varps"])[0]);
        let c = Component {
            cs1_scripts: serde_json::from_value(case["scripts"].clone()).unwrap(),
            cs1_comparison_opcodes: serde_json::from_value(case["opcodes"].clone()).unwrap(),
            cs1_comparison_operands: serde_json::from_value(case["operands"].clone()).unwrap(),
            ..Component::default()
        };
        let result =
            rs09_client::ui::cs1::run(&mut state, &vm, case["index"].as_i64().unwrap() as i32, &c);
        let active = rs09_client::ui::cs1::is_true(&mut state, &vm, &c)
            .map(Value::Bool)
            .unwrap_or_else(|_| Value::String("error".into()));
        assert_eq!(
            serde_json::json!([result, active]),
            case["expected"],
            "CS1case{i}"
        );
    }
}
