use rs09_server::world::door_claims;
use serde_json::Value;
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("../../../fixtures/door-claims/claims.txt");
const PROVENANCE: &str = include_str!("../../../fixtures/door-claims/provenance.json");
const TABLE: &str = include_str!("../src/world/door_claims.txt");

#[test]
fn the_fixture_is_the_one_its_provenance_names() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    let digest: String = Sha256::digest(FIXTURE.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(provenance["fixtureSha256"], digest);
    let rows = FIXTURE
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .count();
    assert_eq!(provenance["rows"].as_u64().unwrap() as usize, rows);
}

/// The table the server refuses by is the booted original's record, byte
/// for byte, and every row of it is read.
#[test]
fn the_native_table_is_the_recorded_one() {
    assert_eq!(TABLE, FIXTURE);
    let mut rows = 0;
    for line in FIXTURE.lines().filter(|l| !l.starts_with('#')) {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        let id: i32 = parts[0].parse().unwrap();
        let owner = door_claims::claim(id, parts[1]).unwrap();
        // The first row of a pair is the one the lookup answers.
        assert!(
            door_claims::rows().any(|row| row == (id, parts[1], owner)),
            "{line}"
        );
        rows += 1;
    }
    assert_eq!(rows, door_claims::rows().count());
    assert!(rows > 100, "only {rows} rows");
    // DoorManagingPlugin is the default of all four options.
    for option in ["open", "close", "shut", "go-through"] {
        let line = format!("# default {option} content.global.handlers.scenery.DoorManagingPlugin");
        assert!(FIXTURE.lines().any(|l| l == line), "{line}");
    }
    // No catch-all listener takes an option from every door (id -1 is a
    // definition the original made for scenery -1).
    assert!(
        door_claims::rows()
            .filter(|row| row.0 < 0)
            .all(|row| row.2.starts_with("definition "))
    );
}
