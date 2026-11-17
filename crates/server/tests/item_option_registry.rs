#[path = "../../../oracle/item-option-registry/native.rs"]
mod native;
use rs09_server::definitions::{
    item::{Registry, Shared},
    option_handler::{self as option, Context, Defaults, Result},
    property::Property,
};
use rs09_server::world::pickup;
use serde_json::Value;
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/item-option-registry/oracle.jsonl");
#[test]
fn original_item_option_registry_precedence_aliases_types_nulls_and_reentry() {
    for (i, line) in CORPUS.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(
            native::evaluate(&row),
            row["expected"],
            "item registry case{i}: {line}"
        );
    }
}
#[test]
fn item_option_registry_fixture_and_source_hashes_are_pinned() {
    let p: Value = serde_json::from_str(include_str!(
        "../../../fixtures/item-option-registry/provenance.json"
    ))
    .unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, expected) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let data: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(data)),
                expected.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}
struct World {
    definitions: Registry,
}
impl Context for World {
    fn definition(&mut self, id: i32) -> Result<Option<rs09_server::definitions::item::Handle>> {
        Ok(Some(self.definitions.for_id(id)))
    }
    fn missing_definition(&mut self, _: i32) -> Result<()> {
        panic!("original forId fallback is nonnull")
    }
}
#[test]
fn pickup_registers_the_actual_native_program_and_definition_override_retains_identity() {
    let mut world = World {
        definitions: Registry::default(),
    };
    let definition = world.definitions.for_id(1);
    world.definitions.insert(1, definition.clone());
    let mut defaults = Shared::new(Defaults::default());
    let handler = pickup::native_handler();
    let returned = pickup::register(&mut defaults, handler.clone()).unwrap();
    assert!(returned.same(&handler));
    let selected = option::item_handler(&mut world, &defaults, 1, Some("take"))
        .unwrap()
        .unwrap();
    assert!(selected.same(&handler));
    assert!(selected.read().value::<pickup::Program>().is_some());
    assert!(
        option::item_handler(&mut world, &defaults, 22937, Some("take"))
            .unwrap()
            .unwrap()
            .same(&handler)
    );
    let program = *selected.read().value::<pickup::Program>().unwrap();
    assert!(program.destination().is_none());
    let override_handler = pickup::native_handler();
    definition.write().handlers.insert(
        "option:take".into(),
        Property::OptionHandler(override_handler.clone()),
    );
    assert!(
        option::item_handler(&mut world, &defaults, 1, Some("take"))
            .unwrap()
            .unwrap()
            .same(&override_handler)
    );
    defaults.write().clear();
    assert!(
        option::item_handler(&mut world, &defaults, 1, Some("take"))
            .unwrap()
            .unwrap()
            .same(&override_handler)
    );
}
