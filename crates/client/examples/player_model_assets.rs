//! Discover resolved original wearable model dependencies.
use rs09_client::{pack::Pack, ui::item::Catalog};
use std::collections::BTreeSet;
fn main() {
    let p = Pack::decode(
        std::fs::read(
            std::env::args()
                .nth(1)
                .unwrap_or_else(|| "assets/visual/ui-models.pack".into()),
        )
        .unwrap(),
    )
    .unwrap();
    let mut catalog = Catalog::default();
    let mut models = BTreeSet::new();
    let ids: Vec<i32> = p
        .manifest
        .files
        .keys()
        .filter_map(|n| n.strip_prefix("item/")?.parse().ok())
        .collect();
    let mut items = 0;
    for id in ids {
        let t = catalog.get(&p, id).unwrap();
        for group in [
            [t.manwear, t.manwear2, t.manwear3],
            [t.womanwear, t.womanwear2, t.womanwear3],
        ] {
            if group[0] != -1 {
                models.extend(group.into_iter().filter(|&n| n != -1));
            }
        }
        items += 1;
    }
    println!("{}", serde_json::json!({"models":models,"items":items}));
}
