//! The action text at the top-left of the game view ("Talk-to Hans / 2 more
//! options").
//!
//! A port of `MiniMenu.getTooltipText` and `renderTooltip`,
//! `Font.renderWavyText` and `renderWavyTextAligned`, the clock seed
//! `client.mainLoop` refreshes, and the client-code-1406 component that
//! `Cs1ScriptRunner.renderComponent` records as the text's host.
//!
//! The text is stretched sideways: every glyph is shifted right by a running
//! offset that grows at random, and the whole line is slightly transparent.
//! Both come from a `java.util.Random` reseeded with the clock seed on every
//! draw, so the line looks the same until the seed changes.
use super::{
    font::Metrics,
    state::{DrawNode, State},
};
use crate::interaction::MenuEntry;
use rs09_game_core::random::JavaRandom;

/// `Component.clientCode` of the component the text is drawn inside.
pub const HOST_CLIENT_CODE: i32 = 1406;
/// `Fonts.b12Full`, the font of the text in the game view.
pub const FONT: i32 = 496;

/// What is selected for use on something else.
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection<'a> {
    /// `selectedObjText` while `itemTargetMode == 1`.
    pub item: Option<&'a str>,
    /// `targetVerb` and `targetOpBase` while `isTargeting`.
    pub target: Option<(&'a str, &'a str)>,
}

/// `renderTooltip`'s first test: nothing is drawn for a lone Cancel with
/// nothing selected.
pub fn visible(size: usize, selection: Selection) -> bool {
    size >= 2 || selection.item.is_some() || selection.target.is_some()
}

/// `MiniMenu.getTooltipText`. `shift_click` is
/// `Cheat.shiftClick && Keyboard.pressedKeys[KEY_SHIFT]`; see
/// [`crate::context_menu::top_entry`].
pub fn text(entries: &[MenuEntry], selection: Selection, shift_click: bool) -> String {
    let size = entries.len();
    let mut text = match (selection.item, selection.target) {
        (Some(item), _) if size < 2 => format!("Use {item} ->"),
        (_, Some((verb, base))) if size < 2 => format!("{verb} {base} ->"),
        _ => crate::context_menu::top_entry(size, shift_click)
            .map(|index| crate::context_menu::op_text(&entries[index]))
            .unwrap_or_default(),
    };
    if size > 2 {
        text.push_str(&format!("<col=ffffff> / {} more options", size - 2));
    }
    text
}

/// `client.mainLoop`: `HOUR_OF_DAY * 600 + MINUTE * 10 + SECOND / 6` of the
/// local wall clock.
pub fn date_seed(hour: i32, minute: i32, second: i32) -> i32 {
    hour.wrapping_mul(600)
        .wrapping_add(minute.wrapping_mul(10))
        .wrapping_add(second / 6)
}

/// `client.loop` and `MiniMenu.gregorianDateSeed`: the seed is read from the
/// clock on the first loop and then on every thousandth.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeedClock {
    pub loops: i32,
    pub seed: i32,
}
impl SeedClock {
    /// One client loop. `clock` returns the local hour, minute and second.
    pub fn tick(&mut self, clock: impl FnOnce() -> [i32; 3]) {
        self.loops = self.loops.wrapping_add(1);
        if self.loops % 1000 == 1 {
            let [hour, minute, second] = clock();
            self.seed = date_seed(hour, minute, second);
        }
    }
}

/// What both wavy renderers draw from the generator before any glyph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wavy {
    /// Text alpha out of 256: 192 to 223.
    pub alpha: i32,
    /// The shift of each glyph, indexed by glyph (an inline icon counts as
    /// one). One value is drawn per character of the raw text, tags included,
    /// so the array is at least as long as the glyph count.
    pub offsets: Vec<i32>,
    /// The final offset: the extra width of the line.
    pub extra: i32,
}

/// The shared opening of `Font.renderWavyText` and `renderWavyTextAligned`
/// for a text of `length` characters.
pub fn wavy(length: usize, seed: i32) -> Wavy {
    let mut random = JavaRandom::new(i64::from(seed));
    let alpha = (random.next_i32() & 0x1f) + 192;
    let mut offsets = Vec::with_capacity(length);
    let mut extra = 0;
    for _ in 0..length {
        offsets.push(extra);
        if random.next_i32() & 3 == 0 {
            extra += 1;
        }
    }
    Wavy {
        alpha,
        offsets,
        extra,
    }
}

/// One line for `Font.renderOffset`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draw {
    pub text: String,
    /// Pen start and baseline.
    pub position: [i32; 2],
    pub color: i32,
    /// Shadow colour, or -1 for none.
    pub shadow: i32,
    pub alpha: i32,
    /// Horizontal shift per glyph.
    pub offsets: Vec<i32>,
    /// The redrawn area: x, y, width, height.
    pub bounds: [i32; 4],
}

/// `renderTooltip` without a host component: `renderWavyText` at four pixels
/// right of and fifteen below the game view's corner, white with a black
/// shadow. `width` is the text's width in [`FONT`].
pub fn in_view(text: String, view: [i32; 2], width: i32, seed: i32) -> Draw {
    let wavy = wavy(text.chars().count(), seed);
    let x = view[0].wrapping_add(4);
    Draw {
        text,
        position: [x, view[1].wrapping_add(15)],
        color: 0xffffff,
        shadow: 0,
        alpha: wavy.alpha,
        offsets: wavy.offsets,
        bounds: [x, view[1], width.wrapping_add(wavy.extra), 15],
    }
}

/// One thing `Font.renderOffset` does while walking a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// A formatting tag (without its brackets) for `Font.parseTag`.
    Tag(String),
    /// A glyph. `x` is where its cell is drawn, shift included; the sprite's
    /// own offset is still to be added. `pen` and `advance` place the
    /// strikethrough and underline, which are not shifted. A space is listed
    /// but has nothing to draw.
    Glyph {
        glyph: u8,
        x: i32,
        pen: i32,
        advance: i32,
    },
    /// The inline name icon `index`, drawn at `x`.
    Icon { index: usize, x: i32 },
}

/// `Font.renderOffset` with horizontal shifts: where every glyph and icon of
/// `text` goes when the pen starts at `start`.
///
/// Every glyph takes the next shift, and so does every `<img=..>` tag, even
/// one whose icon does not exist (the original has advanced its index before
/// the lookup fails). Shifts move a glyph without moving the pen.
pub fn steps(text: &str, metrics: &Metrics, start: i32, offsets: &[i32]) -> Vec<Step> {
    let mut steps = Vec::new();
    let (mut pen, mut index, mut previous) = (start, 0usize, 0usize);
    let mut shift = || {
        let shift = offsets.get(index).copied().unwrap_or(0);
        index += 1;
        shift
    };
    for (raw, glyph, tag) in super::font::tokens(text) {
        if tag && glyph.is_none() {
            let inner = &raw[1..raw.len() - 1];
            let Some(icon) = inner.strip_prefix("img=") else {
                steps.push(Step::Tag(inner.to_owned()));
                continue;
            };
            let x = pen.wrapping_add(shift());
            let icon = icon.parse::<usize>().ok();
            if let Some((icon, &width)) = icon.and_then(|i| Some((i, metrics.name_icons.get(i)?))) {
                steps.push(Step::Icon { index: icon, x });
                pen = pen.wrapping_add(width);
                previous = 0;
            }
            continue;
        }
        let Some(glyph) = glyph else {
            continue;
        };
        let id = usize::from(glyph);
        if previous != 0 {
            if let Some(kerning) = &metrics.kerning {
                pen = pen.wrapping_add(i32::from(kerning[previous * 256 + id]));
            }
        }
        let advance = metrics.advances[id];
        steps.push(Step::Glyph {
            glyph,
            x: pen.wrapping_add(shift()),
            pen,
            advance,
        });
        pen = pen.wrapping_add(advance);
        previous = id;
    }
    steps
}

/// The 1406 component the text is drawn inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Host {
    /// The component's screen position and size.
    pub rect: [i32; 4],
    /// Font id, -1 for [`FONT`].
    pub font: i32,
    pub color: i32,
    pub shadow: i32,
    pub halign: i32,
    pub valign: i32,
}

/// The last drawn component with client code 1406, as `renderComponent`
/// leaves it in `LoginManager.tooltipComponent`. An IF3 component with an
/// empty clip is not reached there.
pub fn host(state: &State, nodes: &[DrawNode]) -> Option<Host> {
    nodes.iter().rev().find_map(|node| {
        let c = state.components.get(&node.key)?;
        (c.client_code == HOST_CLIENT_CODE && (!c.if3 || (node.clip.w > 0 && node.clip.h > 0)))
            .then_some(Host {
                rect: [node.rect.x, node.rect.y, c.width, c.height],
                font: c.font,
                color: c.color,
                shadow: c.shadow_color,
                halign: c.halign,
                valign: c.valign,
            })
    })
}

/// `renderTooltip` with a host: `renderWavyTextAligned` inside the component.
/// `padding` is the font's paragraph top and bottom padding.
pub fn hosted(text: String, host: &Host, metrics: &Metrics, padding: [i32; 2], seed: i32) -> Draw {
    let wavy = wavy(text.chars().count(), seed);
    let [x, y, w, h] = host.rect;
    let [top, bottom] = padding;
    let mut baseline = y.wrapping_add(top);
    if host.valign == 1 {
        baseline = baseline.wrapping_add(h.wrapping_sub(top).wrapping_sub(bottom) / 2);
    } else if host.valign == 2 {
        baseline = y.wrapping_add(h).wrapping_sub(bottom);
    }
    let total = metrics.width(&text).wrapping_add(wavy.extra);
    let start = match host.halign {
        1 => x.wrapping_add(w.wrapping_sub(total) / 2),
        2 => x.wrapping_add(w).wrapping_sub(total),
        _ => x,
    };
    Draw {
        text,
        position: [start, baseline],
        color: host.color,
        shadow: host.shadow,
        alpha: wavy.alpha,
        offsets: wavy.offsets,
        bounds: [
            start,
            baseline.wrapping_sub(top),
            total,
            top.wrapping_add(bottom),
        ],
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests for the text and the seed; the generator, offsets,
    //! alpha and placement are compared with the JVM in `tests/menus.rs`.
    use super::*;

    fn entry(option: &str, target: &str) -> MenuEntry {
        MenuEntry {
            option: option.into(),
            target: target.into(),
            action: 0,
            key: 0,
            args: [0; 2],
            cursor: -1,
        }
    }

    #[test]
    fn text_is_the_top_entry_and_a_count_of_the_rest() {
        let none = Selection::default();
        let cancel = entry("Cancel", "");
        let walk = entry("Walk here", "");
        let talk = entry("Talk-to", "<col=ffff00>Hans");
        assert!(!visible(1, none));
        assert!(visible(2, none));
        assert_eq!(
            text(&[cancel.clone(), walk.clone()], none, false),
            "Walk here"
        );
        assert_eq!(
            text(&[cancel.clone(), walk.clone(), talk.clone()], none, false),
            "Talk-to <col=ffff00>Hans<col=ffffff> / 1 more options"
        );
        let four = [cancel.clone(), walk.clone(), talk.clone(), talk.clone()];
        assert!(text(&four, none, false).ends_with("<col=ffffff> / 2 more options"));
        // No entry at all is malformed; the original would fault.
        assert_eq!(text(&[], none, false), "");
    }

    #[test]
    fn shift_click_names_the_entry_below_the_top_one() {
        // MiniMenu.java:1387-1394: `getOp(size - 2)` with more than two
        // entries; the count of further options is unchanged.
        let none = Selection::default();
        let cancel = entry("Cancel", "");
        let walk = entry("Walk here", "");
        let talk = entry("Talk-to", "<col=ffff00>Hans");
        assert_eq!(
            text(&[cancel.clone(), walk.clone(), talk.clone()], none, true),
            "Walk here<col=ffffff> / 1 more options"
        );
        // Two entries: the top one, as without shift.
        assert_eq!(
            text(&[cancel.clone(), walk.clone()], none, true),
            "Walk here"
        );
        // A selection with nothing else under the pointer comes first.
        let item = Selection {
            item: Some("Bucket"),
            target: None,
        };
        assert_eq!(text(&[cancel], item, true), "Use Bucket ->");
    }

    #[test]
    fn a_selection_shows_only_while_nothing_else_is_under_the_pointer() {
        let item = Selection {
            item: Some("<col=ff9040>Bucket"),
            target: None,
        };
        let spell = Selection {
            item: None,
            target: Some(("Cast", "<col=00ff00>Wind Strike")),
        };
        let cancel = entry("Cancel", "");
        let other = entry("Use", "Bucket -> Hans");
        assert!(visible(1, item) && visible(1, spell));
        assert_eq!(
            text(std::slice::from_ref(&cancel), item, false),
            "Use <col=ff9040>Bucket ->"
        );
        assert_eq!(
            text(std::slice::from_ref(&cancel), spell, false),
            "Cast <col=00ff00>Wind Strike ->"
        );
        // With a second entry the top entry is shown instead.
        assert_eq!(
            text(&[cancel.clone(), other.clone()], item, false),
            "Use Bucket -> Hans"
        );
        assert_eq!(text(&[cancel, other], spell, false), "Use Bucket -> Hans");
    }

    #[test]
    fn the_seed_is_read_on_the_first_loop_and_every_thousandth() {
        assert_eq!(date_seed(0, 0, 0), 0);
        assert_eq!(date_seed(23, 59, 59), 23 * 600 + 590 + 9);
        assert_eq!(date_seed(13, 7, 5), 7870);
        let mut clock = SeedClock::default();
        let mut reads = 0;
        for loops in 1..=2001 {
            clock.tick(|| {
                reads += 1;
                [reads, 0, 0]
            });
            assert_eq!(clock.loops, loops);
            let expected = match loops {
                1..=1000 => 600,
                1001..=2000 => 1200,
                _ => 1800,
            };
            assert_eq!(clock.seed, expected, "loop {loops}");
        }
        assert_eq!(reads, 3);
    }
}
