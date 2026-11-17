//! Lists the world map labels the minimap draws (flag 4), for choosing a
//! capture start tile: `cargo run -p rs09-client --example minimap_labels [CACHE]`.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/cache".into());
    let labels = rs09_client::minimap::Labels::from_cache(directory.as_ref())?;
    for (map, list) in labels.lists() {
        let Some(list) = list else {
            println!("{}: no labels group", map.group);
            continue;
        };
        let shown = (0..list.count())
            .filter(|&k| list.is_minimap_label_visible(k))
            .collect::<Vec<_>>();
        let mut flags = std::collections::BTreeMap::<i8, usize>::new();
        for &flag in &list.flags {
            *flags.entry(flag).or_default() += 1;
        }
        println!(
            "{}: valid {} labels {} on the minimap {} flag counts {flags:?}",
            map.group,
            map.valid,
            list.count(),
            shown.len()
        );
        for k in shown {
            let i = k as usize;
            println!(
                "  {},{} size {} colour {:x} {}",
                list.coord_x[i],
                list.coord_y[i],
                list.get_label_size(k),
                list.colors[i],
                rs09_client::world_map::latin1(&list.names[i])
            );
        }
    }
    Ok(())
}
#[cfg(target_arch = "wasm32")]
fn main() {}
