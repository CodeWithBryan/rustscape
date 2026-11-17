#[path = "../../../oracle/drop-tables/native.rs"]
mod native;
use rs09_server::world::drop_tables::{
    self, ClueLevel, Context, Host, LogLevel, Looter, MAX_WEIGHTED_ITEMS, NpcDropTable,
    PlayerLooter, RollError, RolledItem, SLOT_HDT, SLOT_RDT, WeightBasedTable, WeightedItem,
    parser::{self, Definitions, LoadError},
    random::{BadRange, JavaSource, Source},
    shared::{self, ParseFailure, SharedKind, SharedTables, WeightedChanceItem},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

const CORPUS: &str = include_str!("../../../fixtures/drop-tables/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/drop-tables/provenance.json");
const PINNED: &str = include_str!("../../../fixtures/drop-tables/drop_tables.json");

/// The first place two JSON values differ, so a failure names one field
/// instead of printing a table of thousands of entries.
fn difference(actual: &Value, expected: &Value, path: &str) -> Option<String> {
    match (actual, expected) {
        (Value::Array(a), Value::Array(e)) => {
            if a.len() != e.len() {
                return Some(format!("{path}: {} entries, expected {}", a.len(), e.len()));
            }
            a.iter()
                .zip(e)
                .enumerate()
                .find_map(|(i, (a, e))| difference(a, e, &format!("{path}[{i}]")))
        }
        (Value::Object(a), Value::Object(e)) => {
            if a.len() != e.len() || a.keys().any(|key| !e.contains_key(key)) {
                return Some(format!(
                    "{path}: keys {:?}, expected {:?}",
                    a.keys(),
                    e.keys()
                ));
            }
            a.iter()
                .find_map(|(key, a)| difference(a, &e[key], &format!("{path}.{key}")))
        }
        _ if actual == expected => None,
        _ => Some(format!("{path}: {actual}, expected {expected}")),
    }
}

#[test]
fn original_tables_rolls_and_draws() {
    let mut actions = 0;
    for (i, line) in CORPUS.lines().enumerate() {
        let case: Value = serde_json::from_str(line).unwrap();
        let expected = case["expected"]["steps"].as_array().unwrap();
        let draws: Vec<_> = expected
            .iter()
            .map(|step| {
                step["draws"]
                    .as_array()
                    .map(|draws| [0, 1, 2].map(|generator| draws[generator].as_u64().unwrap()))
            })
            .collect();
        let actual = native::evaluate(&case, &draws);
        assert_eq!(actual.steps.len(), expected.len(), "case {i} step count");
        for (j, (actual, expected)) in actual.steps.iter().zip(expected).enumerate() {
            let op = &case["actions"][j]["op"];
            let at = format!("case {i} step {j} ({op})");
            assert_eq!(actual.error, expected["error"], "{at} error");
            if let Some(found) = difference(&actual.value, &expected["value"], "value") {
                panic!("{at} {found}");
            }
            if let Some(found) = difference(&actual.events, &expected["events"], "events") {
                panic!("{at} {found}");
            }
            assert_eq!(
                actual.aligned, [true; 3],
                "{at} generators (RANDOM, shuffle, thread-local) after recorded draws {}",
                expected["draws"]
            );
            actions += 1;
        }
        assert_eq!(actual.probe, case["expected"]["probe"], "case {i} probe");
    }
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["coverage"]["actions"], actions);
}

#[test]
fn drop_table_fixture_and_original_sources_are_pinned() {
    let p: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(p["caseCount"], CORPUS.lines().count());
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(CORPUS)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let pinned = p["pinnedDataSha256"].as_object().unwrap();
    assert_eq!(pinned.len(), 8);
    for (name, hash) in pinned {
        let bytes = std::fs::read(root.join("fixtures/drop-tables").join(name)).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            hash.as_str().unwrap(),
            "{name}"
        );
    }
    for (path, hash) in p["sources"].as_object().unwrap() {
        let path = root.join(path);
        if path.is_file() {
            let b: Vec<_> = std::fs::read(&path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(b)),
                hash.as_str().unwrap(),
                "{}",
                path.display()
            );
        }
    }
}

/// Every id receives its own row's table; nothing else is recorded.
#[derive(Default)]
struct Tables(BTreeMap<i32, Arc<NpcDropTable>>);
impl Definitions for Tables {
    fn assign(&mut self, id: i32, table: &Arc<NpcDropTable>) -> Result<bool, LoadError> {
        self.0.insert(id, table.clone());
        Ok(true)
    }
    fn name(&mut self, _id: i32) -> Result<Option<String>, LoadError> {
        Ok(None)
    }
    fn log(&mut self, _level: LogLevel, _message: &str) {}
}

#[test]
fn pinned_configuration_counts() {
    let p: Value = serde_json::from_str(PROVENANCE).unwrap();
    let mut tables = Tables::default();
    let count = parser::load(PINNED, &mut JavaSource::new(1, 2, 3), &mut tables).unwrap();
    assert_eq!(p["pinnedNpcIds"], count);
    assert_eq!(p["pinnedNpcIds"], tables.0.len());
    let mut distinct: Vec<&Arc<NpcDropTable>> = Vec::new();
    for table in tables.0.values() {
        if !distinct.iter().any(|seen| Arc::ptr_eq(seen, table)) {
            distinct.push(table);
        }
    }
    assert_eq!(p["pinnedConfigRows"], distinct.len());
    let entries: usize = distinct
        .iter()
        .map(|table| {
            [&table.main, table.charm_drops(), table.tertiary_drops()]
                .iter()
                .map(|part| part.items.len() + part.guaranteed_items.len())
                .sum::<usize>()
        })
        .sum();
    assert_eq!(p["pinnedEntries"], entries);
    // The parser only makes guaranteed entries from a row's "default" list.
    assert!(distinct.iter().all(|table| {
        table.charm_drops().guaranteed_items.is_empty()
            && table.tertiary_drops().guaranteed_items.is_empty()
    }));
}

/// Hands out listed draws and fails the test if the roll asks for a different
/// kind of draw, a different bound, or more draws than listed.
#[derive(Debug, PartialEq)]
enum Draw {
    Int(i32, i32),
    Boolean(bool),
    Double(f64, f64),
    Shuffle(i32, i32),
}
struct Scripted(std::collections::VecDeque<Draw>);
impl Scripted {
    fn new(draws: impl IntoIterator<Item = Draw>) -> Self {
        Self(draws.into_iter().collect())
    }
}
impl Source for Scripted {
    fn next_int(&mut self, bound: i32) -> i32 {
        match self.0.pop_front() {
            Some(Draw::Int(expected, value)) if expected == bound => value,
            other => panic!("nextInt({bound}) but the script has {other:?}"),
        }
    }
    fn next_boolean(&mut self) -> bool {
        match self.0.pop_front() {
            Some(Draw::Boolean(value)) => value,
            other => panic!("nextBoolean() but the script has {other:?}"),
        }
    }
    fn next_double(&mut self, bound: f64) -> Result<f64, BadRange> {
        match self.0.pop_front() {
            Some(Draw::Double(expected, value)) if expected == bound => Ok(value),
            other => panic!("nextDouble({bound}) but the script has {other:?}"),
        }
    }
    fn shuffle_int(&mut self, bound: i32) -> i32 {
        match self.0.pop_front() {
            Some(Draw::Shuffle(expected, value)) if expected == bound => value,
            other => panic!("shuffle nextInt({bound}) but the script has {other:?}"),
        }
    }
}

struct NoClues;
impl Host for NoClues {
    fn clue_scrolls(&self, _level: ClueLevel) -> Option<&[i32]> {
        None
    }
    fn log(&mut self, _origin: &'static str, _level: LogLevel, _message: &str) {}
    fn stack_trace(&mut self, _origin: &'static str, _failure: ParseFailure) {}
}

fn item(id: i32, amount: i32) -> RolledItem {
    RolledItem { id, amount }
}

#[test]
fn a_roll_follows_the_numbers_it_is_given() {
    // Insertion: the second entry draws position 0 of 1 and so swaps to the
    // front; the third draws position 1 of 2.
    let mut build = Scripted::new([Draw::Int(1, 0), Draw::Int(2, 1)]);
    let mut table = WeightBasedTable::new();
    table.add(&mut build, WeightedItem::new(100, 1, 1, 1.0, false));
    table.add(&mut build, WeightedItem::new(200, 2, 5, 2.0, false));
    table.add(&mut build, WeightedItem::new(300, 1, 1, 5.0, false));
    table.add(&mut build, WeightedItem::new(900, 4, 4, 0.0, true));
    assert!(build.0.is_empty());
    let order: Vec<_> = table.items.iter().map(|item| item.id).collect();
    assert_eq!(order, [200, 300, 100]);
    assert_eq!(table.total_weight, 8.0);

    let mut shared = SharedTables::new();
    let mut host = NoClues;
    // One double per time, both before any amount. 2.0 lands exactly on the
    // first entry's boundary and stays with it; 7.5 passes 200 and 300 and
    // ends on 100. Then the items are made in order: only 200 has a range
    // (2 up to but not including 5), so it alone draws.
    let mut draws = Scripted::new([
        Draw::Double(8.0, 2.0),
        Draw::Double(8.0, 7.5),
        Draw::Int(3, 2),
    ]);
    let mut ctx = Context::new(&mut draws, &mut shared, &mut host);
    let rolled = table.roll(&mut ctx, Looter::None, 2).unwrap();
    assert_eq!(
        rolled,
        [item(900, 4), item(200, 4), item(900, 4), item(100, 1)]
    );
    assert!(draws.0.is_empty());
}

#[test]
#[should_panic(expected = "nextDouble(8) but the script has Some(Int(3, 2))")]
fn selection_draws_come_before_amount_draws() {
    let mut table = WeightBasedTable::new();
    let mut build = Scripted::new([]);
    table.add(&mut build, WeightedItem::new(200, 2, 5, 8.0, false));
    let mut shared = SharedTables::new();
    let mut host = NoClues;
    // Wrong order on purpose: an amount between the two selection doubles.
    let mut draws = Scripted::new([
        Draw::Double(8.0, 2.0),
        Draw::Int(3, 2),
        Draw::Double(8.0, 7.5),
    ]);
    let mut ctx = Context::new(&mut draws, &mut shared, &mut host);
    let _ = table.roll(&mut ctx, Looter::None, 2);
}

/// A player whose answers are fixed and whose questions are counted.
#[derive(Default)]
struct Wearer {
    wealth: bool,
    gloves: Option<i32>,
    asked: Vec<i32>,
    charges_used: u32,
}
impl PlayerLooter for Wearer {
    fn removes_nothings(&mut self) -> bool {
        self.wealth
    }
    fn in_equipment(&mut self, item_id: i32) -> bool {
        self.asked.push(item_id);
        self.gloves == Some(item_id)
    }
    fn update_glove_charges(&mut self) {
        self.charges_used += 1;
    }
}

#[test]
fn hand_offs_draw_from_their_own_generators() {
    let mut host = NoClues;
    let mut shared = SharedTables::new();
    let entry = |id, minimum_amount, maximum_amount, weight| WeightedChanceItem {
        id,
        minimum_amount,
        maximum_amount,
        weight,
    };
    shared.herb = vec![entry(199, 1, 1, 3), entry(201, 2, 4, 1)];
    let slot = [WeightedItem::new(SLOT_HDT, 1, 1, 1.0, false)];

    // Not a player: the coin is still flipped, then the herb table is rolled.
    // Pick 3 of 4, then the shuffle swaps the two entries, so the walk meets
    // 201 (3 > 1, pick becomes 2) and then 199 (2 <= 3).
    let mut draws = Scripted::new([Draw::Boolean(true), Draw::Int(4, 3), Draw::Shuffle(2, 0)]);
    let mut ctx = Context::new(&mut draws, &mut shared, &mut host);
    let rolled = drop_tables::convert_weighted_items(&mut ctx, &slot, Looter::Entity).unwrap();
    assert_eq!(rolled, [item(199, 1)]);
    assert!(draws.0.is_empty());
    // The shuffle was in place.
    assert_eq!(shared.herb, [entry(201, 2, 4, 1), entry(199, 1, 1, 3)]);

    // A player who wins the coin and wears kwuarm gloves: asked about irit,
    // avantoe and kwuarm in that order, one charge used, no table roll.
    let mut player = Wearer {
        gloves: Some(12858),
        ..Wearer::default()
    };
    let mut draws = Scripted::new([Draw::Boolean(true)]);
    let mut ctx = Context::new(&mut draws, &mut shared, &mut host);
    let rolled =
        drop_tables::convert_weighted_items(&mut ctx, &slot, Looter::Player(&mut player)).unwrap();
    assert_eq!(rolled, [item(213, 1)]);
    assert_eq!(player.asked, [12856, 12857, 12858]);
    assert_eq!(player.charges_used, 1);
    assert!(draws.0.is_empty());

    // The same player losing the coin is not asked anything. The pick 0
    // takes the first entry after the shuffle, here 201 with a range of 2.
    let mut draws = Scripted::new([
        Draw::Boolean(false),
        Draw::Int(4, 0),
        Draw::Shuffle(2, 1),
        Draw::Int(2, 1),
    ]);
    let mut ctx = Context::new(&mut draws, &mut shared, &mut host);
    let rolled =
        drop_tables::convert_weighted_items(&mut ctx, &slot, Looter::Player(&mut player)).unwrap();
    assert_eq!(rolled, [item(201, 3)]);
    assert_eq!(player.asked.len(), 3);
    assert!(draws.0.is_empty());

    // Rare table: 100 (weight 2) and a nothing entry (weight 6). The second
    // entry loaded draws its position and swaps to the front.
    let xml = r#"<RDT><item id="100" minAmt="1" maxAmt="1" weight="2"/><item id="0" minAmt="1" maxAmt="1" weight="6"/></RDT>"#;
    let mut draws = Scripted::new([Draw::Int(1, 0)]);
    assert_eq!(
        shared.parse(SharedKind::Rare, xml, &mut draws, &mut host),
        None
    );
    assert_eq!(shared.rare.total_weight, 8.0);
    assert_eq!(shared.rare.items[0].id, 0);
    let rare = [WeightedItem::new(SLOT_RDT, 1, 1, 1.0, false)];
    // Without the ring the double is drawn against all 8; 2.5 passes 100.
    let mut draws = Scripted::new([Draw::Double(8.0, 2.5), Draw::Shuffle(2, 1)]);
    let mut ctx = Context::new(&mut draws, &mut shared, &mut host);
    let rolled = drop_tables::convert_weighted_items(&mut ctx, &rare, Looter::Entity).unwrap();
    assert!(rolled.is_empty());
    assert!(draws.0.is_empty());
    // With it the nothing weight leaves the total: any draw below 2 is 100.
    let mut player = Wearer {
        wealth: true,
        ..Wearer::default()
    };
    let mut draws = Scripted::new([Draw::Double(2.0, 1.5), Draw::Shuffle(2, 0)]);
    let mut ctx = Context::new(&mut draws, &mut shared, &mut host);
    let rolled =
        drop_tables::convert_weighted_items(&mut ctx, &rare, Looter::Player(&mut player)).unwrap();
    assert_eq!(rolled, [item(100, 1)]);
    assert!(draws.0.is_empty());
}

#[test]
fn native_bounds_replace_unbounded_original_work() {
    let mut shared = SharedTables::new();
    let mut host = NoClues;
    let mut source = JavaSource::new(1, 2, 3);

    // A herb slot whose amount range asks for more hand-offs than the bound.
    let mut table = WeightBasedTable::new();
    let many = MAX_WEIGHTED_ITEMS as i32 + 1;
    table.add(
        &mut source,
        WeightedItem::new(SLOT_HDT, many, many, 1.0, true),
    );
    let mut ctx = Context::new(&mut source, &mut shared, &mut host);
    assert_eq!(
        table.roll(&mut ctx, Looter::None, 1),
        Err(RollError::TooManyItems)
    );
    // An ordinary entry repeated by the times argument hits the same bound.
    let mut table = WeightBasedTable::new();
    table.add(&mut source, WeightedItem::new(995, 1, 1, 1.0, true));
    let mut ctx = Context::new(&mut source, &mut shared, &mut host);
    assert_eq!(
        table.roll(&mut ctx, Looter::None, many),
        Err(RollError::TooManyItems)
    );
    assert_eq!(
        table
            .roll(&mut ctx, Looter::None, MAX_WEIGHTED_ITEMS as i32)
            .map(|items| items.len()),
        Ok(MAX_WEIGHTED_ITEMS)
    );
    // An empty table does no work for any count.
    let mut ctx = Context::new(&mut source, &mut shared, &mut host);
    assert_eq!(
        WeightBasedTable::new().roll(&mut ctx, Looter::None, i32::MAX),
        Ok(vec![])
    );

    // A rare table that can only hand off to itself.
    let xml = format!(r#"<RDT><item id="{SLOT_RDT}" minAmt="1" maxAmt="1" weight="1"/></RDT>"#);
    assert_eq!(
        shared.parse(SharedKind::Rare, &xml, &mut source, &mut host),
        None
    );
    let mut ctx = Context::new(&mut source, &mut shared, &mut host);
    assert_eq!(
        shared::retrieve_rare(&mut ctx, Looter::Entity),
        Err(RollError::RareDepth)
    );
    // The bound is per roll: the same context then rolls a table that ends.
    ctx.shared.rare = WeightBasedTable::new();
    ctx.shared
        .rare
        .add(ctx.random, WeightedItem::new(995, 7, 7, 1.0, false));
    assert_eq!(
        shared::retrieve_rare(&mut ctx, Looter::Entity),
        Ok(Some(item(995, 7)))
    );
}

#[test]
fn configuration_file_is_read_from_the_prefix() {
    let folder = std::env::temp_dir().join(format!("rs09-drop-tables-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let prefix = format!("{}{}", folder.display(), std::path::MAIN_SEPARATOR);
    std::fs::write(
        folder.join("drop_tables.json"),
        r#"[{"ids":"7,8","main":[],"charm":[],"default":[{"id":"526","minAmount":"1","maxAmount":"1","weight":"1.0"}]}]"#,
    )
    .unwrap();
    let mut tables = Tables::default();
    let count = parser::load_config(Some(&prefix), &mut JavaSource::new(1, 2, 3), &mut tables);
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(count.unwrap(), 2);
    assert!(Arc::ptr_eq(&tables.0[&7], &tables.0[&8]));
    assert_eq!(tables.0[&7].main.guaranteed_items.len(), 1);
    // A null prefix is the literal text "null", as in the original.
    assert!(matches!(
        parser::load_config(None, &mut JavaSource::new(1, 2, 3), &mut tables),
        Err(LoadError::File(_))
    ));
}
