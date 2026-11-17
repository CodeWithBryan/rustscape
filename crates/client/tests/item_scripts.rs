use rs09_client::{
    pack::Pack,
    ui::{
        HookArg,
        item::ItemType,
        state::{Hook, Key, State},
        vm::Vm,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[test]
fn original_item_script_branches_match_through_native_vm() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/item-scripts.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source =
        std::fs::read(root.join("reference/client/client/src/main/java/rt4/ScriptRunner.java"))
            .unwrap()
            .into_iter()
            .filter(|&b| b != b'\r')
            .collect::<Vec<_>>();
    assert_eq!(
        format!("{:x}", Sha256::digest(source)),
        f["sources"]["ScriptRunner"].as_str().unwrap()
    );
    for row in f["cases"].as_array().unwrap() {
        let op = row["opcode"].as_i64().unwrap() as u16;
        let m = &row["metadata"];
        let n = |name: &str| m[name].as_i64().unwrap() as i32;
        let item = ItemType {
            id: n("id"),
            name: m["name"].as_str().unwrap().into(),
            cost: n("cost"),
            stackable: n("stackable"),
            certtemplate: n("certtemplate"),
            certlink: n("certlink"),
            members: m["members"].as_bool().unwrap(),
            ops: serde_json::from_value(m["ops"].clone()).unwrap(),
            iops: serde_json::from_value(m["iops"].clone()).unwrap(),
            ..ItemType::default()
        };
        let mut script = vec![0];
        let mut count = 0i32;
        let mut push = |opcode: u16, operand: &[u8]| {
            script.extend(opcode.to_be_bytes());
            script.extend(operand);
            count += 1;
        };
        for arg in row["args"].as_array().unwrap() {
            push(0, &(arg.as_i64().unwrap() as i32).to_be_bytes());
        }
        push(op, &[0]);
        push(if op <= 4202 { 48 } else { 43 }, &999i32.to_be_bytes());
        push(21, &[0]);
        script.extend(count.to_be_bytes());
        script.extend([0; 9]);
        script.extend(1u16.to_be_bytes());
        let manifest=serde_json::to_vec(&json!({"source":"Explicit resolved-item metadata service","files":{"script/1":[0,script.len()]}})).unwrap();
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        bytes.extend(script);
        let mut state = State::new(Pack::decode(bytes).unwrap());
        state.items.types.insert(item.id, item);
        let mut vm = Vm::default();
        vm.run(
            &mut state,
            &Hook::new(Key::component(0), vec![HookArg::Int(1)]),
            100,
        )
        .unwrap_or_else(|e| panic!("{row}: {e}"));
        let actual = if op <= 4202 {
            json!(vm.varcstrs[&999])
        } else {
            json!(vm.varcs[&999])
        };
        assert_eq!(actual, row["expected"], "{row}");
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 1024);
}
