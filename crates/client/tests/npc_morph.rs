use rs09_client::npc::Definition;
use serde_json::{Value, json};
#[test]
fn original_morph_priority_fallback_hidden_and_validity_match() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/morph.json")).unwrap();
    for row in f["cases"].as_array().unwrap() {
        let n = |k: &str| row[k].as_i64().unwrap() as i32;
        let mut d = Definition::decode(&[0]).unwrap();
        d.morph_varbit = n("varbit");
        d.morph_varp = n("varp");
        d.children = serde_json::from_value(row["children"].clone()).unwrap();
        let mut queries = Vec::new();
        let mut read = |bit: bool, id: i32| {
            if bit {
                queries.push(id);
                n("bit_value")
            } else {
                n("varp_value")
            }
        };
        let selected = d.select_morph(&mut read);
        let valid = d.morph_valid(read);
        assert_eq!(json!([selected, valid, queries]), row["expected"]);
    }
}
