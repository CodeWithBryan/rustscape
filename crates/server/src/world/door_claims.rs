//! The door options the original gives to other content.
//!
//! DoorManagingPlugin is the default handler of scenery's "open", "close",
//! "shut" and "go-through" options. A content listener for a scenery id, or
//! a handler on the scenery's definition, takes the option before it. This
//! table names every such pair the original registers at start-up
//! (`fixtures/door-claims`, recorded from the booted original;
//! `tests/door_claims.rs` checks the table against the fixture), so a world
//! that does not install the content can refuse the option instead of
//! opening the scenery as an ordinary door.
const TABLE: &str = include_str!("door_claims.txt");

/// The original content that owns `option` of scenery `id`, if any.
pub fn claim(id: i32, option: &str) -> Option<&'static str> {
    rows()
        .find(|row| row.0 == id && row.1 == option)
        .map(|row| row.2)
}

/// Every row: scenery id, option, the owner.
pub fn rows() -> impl Iterator<Item = (i32, &'static str, &'static str)> {
    TABLE
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.splitn(3, ' ');
            let id = parts.next()?.parse().ok()?;
            Some((id, parts.next()?, parts.next()?))
        })
}
