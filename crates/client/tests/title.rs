//! JVM differential for the loading bar: `tests/title/loading_bar.txt` is the
//! output of `oracle/title/TitleOracle.java`, which runs the unchanged
//! `rt4.LoadingBar` against recording stand-ins for the sprites, the font and
//! both rasters (576 cases: eight canvas sizes, eight percentages, three logo
//! widths, and the hardware, software-with-background and software-without
//! paths). [`LoadingBar::layout`] must give every number the original passed.
//!
//! What the stand-ins do with the calls (stretching, clipping, text
//! measurement) is not observed.
use rs09_client::title::LoadingBar;

const TRACE: &str = include_str!("title/loading_bar.txt");

fn field<'a>(line: &'a str, name: &str) -> &'a str {
    line.split(' ')
        .find_map(|part| part.strip_prefix(name)?.strip_prefix('='))
        .unwrap_or_else(|| panic!("{name} in {line}"))
}
fn numbers(text: &str) -> Vec<i32> {
    text.split(',').map(|n| n.parse().unwrap()).collect()
}
fn list(values: &[i32]) -> String {
    values
        .iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn loading_bar_layout_matches_the_original() {
    let mut cases = 0;
    for line in TRACE.lines() {
        let (case, drawn) = line.split_once(" :").expect("a case and its calls");
        let canvas = numbers(field(case, "canvas"));
        let percent: i32 = field(case, "percent").parse().unwrap();
        let logo: i32 = field(case, "logo").parse().unwrap();
        let mode: i32 = field(case, "mode").parse().unwrap();
        let bar = LoadingBar::layout([canvas[0], canvas[1]], percent, logo);
        // Mode 0 is the hardware renderer, which always draws the picture;
        // 1 the software renderer asked to draw it; 2 the software renderer
        // drawing the bar alone. The bar's numbers are the same in all three.
        let raster = if mode == 0 { "" } else { "soft." };
        let mut expected = String::new();
        if mode != 2 {
            expected += &format!(
                " background={} logo={}",
                list(&bar.background),
                list(&bar.logo)
            );
        }
        let (red, text) = (LoadingBar::RED as i32, LoadingBar::TEXT as i32);
        expected += &format!(
            " title={},{text},-1 {raster}drawRect={},{red} {raster}drawRect={},0 \
             {raster}fillRect={},{red} {raster}fillRect={},0 text={},{text},-1",
            list(&bar.title),
            list(&bar.outline),
            list(&bar.inner_outline),
            list(&bar.filled),
            list(&bar.empty),
            list(&bar.text),
        );
        assert_eq!(drawn, expected, "{case}");
        cases += 1;
    }
    assert_eq!(cases, 576);
}
