//! The right-click menu while it is open.
//!
//! A port of the menu half of `Protocol.processDefaultAction`,
//! `ScriptRunner.layoutMiniMenu`, `MiniMenu.getOp`/`remove`,
//! `InterfaceList.isInterfaceAction` and the trimming loop that
//! `Protocol.openTopLevelInterface` and `InterfaceList.closeInterface` share.
//! Entries are in `MiniMenu` order: index 0 is drawn at the bottom and the
//! last entry is the left-click default.
//!
//! Not modelled here: the `clickProcessingState` hand-over that defers a click
//! by one frame when the pointer moved between the press and the frame that
//! handles it, and the styled menu of CS2 5424 (`InterfaceList.useStyledMenu`:
//! 26 instead of 22 for the height, 35 instead of 31 for the row baseline).
//! No script in the pinned cache runs 5424.
use crate::interaction::{Menu, MenuEntry};

/// `LocalizedText.CHOOSE_OPTION`.
pub const CHOOSE_OPTION: &str = "Choose Option";

/// `MiniMenu.getOp`: the option, and the target after a separator only when
/// there is one.
pub fn op_text(entry: &MenuEntry) -> String {
    if entry.target.is_empty() {
        entry.option.clone()
    } else {
        format!("{} {}", entry.option, entry.target)
    }
}

/// The entry a left click runs and the hover text and cursor describe:
/// `size - 1`, or `size - 2` while shift-click applies and there are more
/// than two entries. None for an empty menu.
///
/// `shift_click` is `Cheat.shiftClick && Keyboard.pressedKeys[KEY_SHIFT]`:
/// the 2009scape switch (`::shiftclick`, off unless turned on) with shift
/// held. The original tests it, with `size > 2`, in the three places that
/// all go through this function: `MiniMenu.processClick`
/// (MiniMenu.java:1673-1685), `MiniMenu.getTooltipText` (1387-1391) and
/// `LoginManager.getActiveCursorId` (LoginManager.java:1497).
///
/// `Protocol.processDefaultAction`'s own tests before the click (the
/// inventory drag and whether the menu opens instead, Protocol.java:
/// 3543-3566) read `size - 1` whatever shift does.
pub fn top_entry(size: usize, shift_click: bool) -> Option<usize> {
    if shift_click && size > 2 {
        Some(size - 2)
    } else {
        size.checked_sub(1)
    }
}

/// `InterfaceList.isInterfaceAction`: actions that belong to a component.
pub fn is_interface_action(action: i32) -> bool {
    matches!(
        action,
        47 | 5
            | 43
            | 35
            | 58
            | 22
            | 40
            | 3
            | 9
            | 12
            | 1006
            | 1003
            | 25
            | 23
            | 48
            | 7
            | 13
            | 8
            | 32
            | 28
            | 59
            | 51
            | 41
    )
}

/// The removal loop of `openTopLevelInterface`/`closeInterface`.
///
/// The original walks the indices of the size it started with and calls
/// `MiniMenu.remove(i)` on each interface action it reads, without stepping
/// back. So the entry that slides into `i` is skipped, and indices at or past
/// the shrinking size still read what the shifts left behind there: each such
/// stale interface action takes one more live entry off the end.
pub fn remove_interface_actions(entries: &mut Vec<MenuEntry>) {
    let original = entries.len();
    let mut size = original;
    for i in 0..original {
        if !is_interface_action(entries[i].action) {
            continue;
        }
        // MiniMenu.remove(i). A menu always holds Cancel, which is not an
        // interface action, so the size cannot pass zero here.
        size = size.saturating_sub(1);
        // ArrayUtils.copy(.., i + 1, .., i, size - i): nothing when the count
        // is zero or negative, and the old last slot keeps its value.
        if i < size {
            entries[i..=size].rotate_left(1);
            entries[size] = entries[size - 1].clone();
        }
    }
    entries.truncate(size);
}

/// `InterfaceList.menuX`, `menuY`, `menuWidth` and `menuHeight`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl Bounds {
    /// The baseline of entry `index` of `size`, as `drawA` and the click test
    /// compute it.
    pub fn row_baseline(&self, size: usize, index: usize) -> i32 {
        (size as i32 - index as i32 - 1)
            .wrapping_mul(15)
            .wrapping_add(self.y)
            .wrapping_add(31)
    }
    /// The entry under `point`: strictly inside the left and right edges and
    /// within the 15 rows of pixels of one entry. The last match wins, as in
    /// the original loop.
    pub fn row_at(&self, size: usize, point: [i32; 2]) -> Option<usize> {
        let mut found = None;
        for index in 0..size {
            let baseline = self.row_baseline(size, index);
            if point[0] > self.x
                && self.x.wrapping_add(self.width) > point[0]
                && baseline.wrapping_sub(13) < point[1]
                && baseline.wrapping_add(3) > point[1]
            {
                found = Some(index);
            }
        }
        found
    }
    /// Whether the pointer has left the menu by more than ten pixels.
    pub fn left(&self, mouse: [i32; 2]) -> bool {
        mouse[0] < self.x.wrapping_sub(10)
            || mouse[0] > self.width.wrapping_add(self.x).wrapping_add(10)
            || self.y.wrapping_sub(10) > mouse[1]
            || mouse[1] > self.height.wrapping_add(self.y).wrapping_add(10)
    }
}

/// The widest of the heading and every entry, plus eight.
pub fn width(entries: &[MenuEntry], measure: impl Fn(&str) -> i32) -> i32 {
    entries
        .iter()
        .map(|entry| measure(&op_text(entry)))
        .fold(measure(CHOOSE_OPTION), i32::max)
        .wrapping_add(8)
}

/// `MiniMenu.size * 15 + 22`, the height the menu is drawn and tested with.
pub fn height(size: usize) -> i32 {
    (size as i32).wrapping_mul(15).wrapping_add(22)
}

/// `ScriptRunner.layoutMiniMenu`: centred under the pointer and kept on the
/// canvas. The bottom clamp uses a height one less than the one that is stored.
pub fn layout(size: usize, width: i32, mouse: [i32; 2], canvas: [i32; 2]) -> Bounds {
    let clamp_height = (size as i32).wrapping_mul(15).wrapping_add(21);
    let mut y = mouse[1];
    let mut x = mouse[0].wrapping_sub(width / 2);
    if y.wrapping_add(clamp_height) > canvas[1] {
        y = canvas[1].wrapping_sub(clamp_height);
    }
    if canvas[0] < x.wrapping_add(width) {
        x = canvas[0].wrapping_sub(width);
    }
    if x < 0 {
        x = 0;
    }
    if y < 0 {
        y = 0;
    }
    Bounds {
        x,
        y,
        width,
        height: height(size),
    }
}

/// `Mouse.clickButton` for one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    None,
    Left,
    Right,
}

/// The open menu: the entries frozen when it opened, and where it is.
#[derive(Clone, Default)]
pub struct ContextMenu {
    pub menu: Menu,
    pub bounds: Bounds,
}
impl ContextMenu {
    /// The opening half of `layoutMiniMenu`. `menu` is the sorted list built
    /// for this frame; `width` comes from [`width`].
    pub fn open(&mut self, mut menu: Menu, width: i32, mouse: [i32; 2], canvas: [i32; 2]) {
        self.bounds = layout(menu.entries.len(), width, mouse, canvas);
        menu.open = true;
        self.menu = menu;
    }
    pub fn close(&mut self) {
        self.menu.open = false;
    }
    /// `Protocol.processDefaultAction` while the menu is open. Without a left
    /// click the menu closes once the pointer is more than ten pixels outside
    /// it, so a right click does nothing else. A left click runs the entry
    /// under it, if any, and always closes the menu.
    pub fn step(&mut self, button: Button, mouse: [i32; 2], click: [i32; 2]) -> Option<MenuEntry> {
        if !self.menu.open {
            return None;
        }
        if button != Button::Left {
            if self.bounds.left(mouse) {
                self.menu.open = false;
            }
            return None;
        }
        let chosen = self
            .bounds
            .row_at(self.menu.entries.len(), click)
            .map(|index| self.menu.entries[index].clone());
        self.menu.open = false;
        chosen
    }
    /// A sub-interface opened or closed: drop the open menu's component
    /// entries with [`remove_interface_actions`], close it when exactly one
    /// entry is left, and otherwise resize it in place.
    ///
    /// The original runs the same loop over the closed menu's list too. That
    /// list is rebuilt before this client reads it again, so only the open
    /// menu is trimmed here.
    pub fn interface_changed(&mut self, measure: impl Fn(&str) -> i32) {
        if !self.menu.open {
            return;
        }
        remove_interface_actions(&mut self.menu.entries);
        if self.menu.entries.len() == 1 {
            self.menu.open = false;
        } else {
            self.bounds.width = width(&self.menu.entries, measure);
            self.bounds.height = height(self.menu.entries.len());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests: the expected values were worked out by hand from
    //! the Java named in the module documentation; no original class runs.
    use super::*;

    fn entry(option: &str, target: &str, action: i32) -> MenuEntry {
        MenuEntry {
            option: option.into(),
            target: target.into(),
            action,
            key: 0,
            args: [0; 2],
            cursor: -1,
        }
    }
    fn menu(actions: &[(&str, i32)]) -> Menu {
        Menu {
            entries: actions
                .iter()
                .map(|&(name, action)| entry(name, "", action))
                .collect(),
            open: false,
        }
    }
    fn names(entries: &[MenuEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.option.as_str()).collect()
    }
    /// Six pixels a character, like a fixed-width font.
    fn measure(text: &str) -> i32 {
        text.len() as i32 * 6
    }

    #[test]
    fn op_text_adds_the_separator_only_before_a_target() {
        assert_eq!(op_text(&entry("Walk here", "", 60)), "Walk here");
        assert_eq!(
            op_text(&entry("Talk-to", "<col=ffff00>Hans", 17)),
            "Talk-to <col=ffff00>Hans"
        );
        // Spell-on-tile's target is " ->", so the text has two spaces.
        assert_eq!(op_text(&entry("Cast", " ->", 11)), "Cast  ->");
        assert_eq!(top_entry(0, false), None);
        assert_eq!(top_entry(3, false), Some(2));
    }

    #[test]
    fn shift_click_takes_the_entry_below_the_top_of_a_longer_menu() {
        // MiniMenu.java:1673-1685, 1387-1391 and LoginManager.java:1497:
        // `Cheat.shiftClick && pressedKeys[KEY_SHIFT] && size > 2` picks
        // `size - 2`, anything else `size - 1`.
        assert_eq!(top_entry(3, true), Some(1));
        assert_eq!(top_entry(9, true), Some(7));
        // Cancel and one option: the option, shift or not.
        assert_eq!(top_entry(2, true), Some(1));
        assert_eq!(top_entry(1, true), Some(0));
        assert_eq!(top_entry(0, true), None);
        for size in 0..12 {
            assert_eq!(top_entry(size, false), size.checked_sub(1));
        }
    }

    #[test]
    fn interface_actions_are_the_original_list() {
        let listed = [
            47, 5, 43, 35, 58, 22, 40, 3, 9, 12, 1006, 1003, 25, 23, 48, 7, 13, 8, 32, 28, 59, 51,
            41,
        ];
        for action in -5..3100 {
            assert_eq!(
                is_interface_action(action),
                listed.contains(&action),
                "{action}"
            );
        }
    }

    #[test]
    fn removal_skips_the_entry_that_slides_into_the_removed_slot() {
        // Two neighbouring interface actions: the second slides into index 1,
        // is not looked at again there, and is then read once more as the
        // stale copy at index 2, which costs the live last entry.
        let mut entries = menu(&[("Cancel", 1005), ("A", 9), ("B", 9)]).entries;
        remove_interface_actions(&mut entries);
        assert_eq!(names(&entries), ["Cancel"]);
        // Without a stale interface action behind it the skipped one survives.
        let mut entries = menu(&[("Cancel", 1005), ("A", 9), ("B", 9), ("Walk", 60)]).entries;
        remove_interface_actions(&mut entries);
        assert_eq!(names(&entries), ["Cancel", "B", "Walk"]);
    }

    #[test]
    fn stale_slots_past_the_new_size_remove_live_entries() {
        // i=1 removes A: [Cancel, Walk, B | B]. i=2 removes B and the size is
        // 2. i=3 reads the stale B and takes the size to 1: Walk is lost.
        let mut entries = menu(&[("Cancel", 1005), ("A", 9), ("Walk", 60), ("B", 9)]).entries;
        remove_interface_actions(&mut entries);
        assert_eq!(names(&entries), ["Cancel"]);
        // A single interface action at the end is removed and nothing else.
        let mut entries = menu(&[("Cancel", 1005), ("Walk", 60), ("A", 9)]).entries;
        remove_interface_actions(&mut entries);
        assert_eq!(names(&entries), ["Cancel", "Walk"]);
        // One in the middle: the tail shifts down and its stale last copy is
        // not an interface action.
        let mut entries = menu(&[("Cancel", 1005), ("A", 9), ("Walk", 60), ("Talk", 17)]).entries;
        remove_interface_actions(&mut entries);
        assert_eq!(names(&entries), ["Cancel", "Walk", "Talk"]);
        // No interface action: untouched.
        let mut entries = menu(&[("Cancel", 1005), ("Walk", 60)]).entries;
        remove_interface_actions(&mut entries);
        assert_eq!(names(&entries), ["Cancel", "Walk"]);
        // Without Cancel every entry can go; the size stops at zero.
        let mut entries = menu(&[("A", 9), ("B", 9), ("C", 9)]).entries;
        remove_interface_actions(&mut entries);
        assert!(entries.is_empty());
    }

    #[test]
    fn layout_centres_under_the_pointer_and_clamps_with_the_shorter_height() {
        // Three entries: stored height 3*15+22, clamp height 3*15+21.
        assert_eq!(
            layout(3, 101, [300, 200], [765, 503]),
            Bounds {
                x: 250,
                y: 200,
                width: 101,
                height: 67
            }
        );
        // 437 + 66 == 503 still fits; one pixel lower is pulled up to 437.
        assert_eq!(layout(3, 100, [300, 437], [765, 503]).y, 437);
        assert_eq!(layout(3, 100, [300, 438], [765, 503]).y, 437);
        // Right, left and top edges.
        assert_eq!(layout(3, 100, [760, 10], [765, 503]).x, 665);
        assert_eq!(layout(3, 100, [20, 10], [765, 503]).x, 0);
        assert_eq!(layout(40, 100, [20, 10], [765, 503]).y, 0);
        // The width is the widest text plus eight; the heading counts.
        let entries = menu(&[("Cancel", 1005), ("Walk here", 60)]).entries;
        assert_eq!(width(&entries, measure), 13 * 6 + 8);
        let mut wide = entries.clone();
        wide.push(entry("Talk-to", "A very long name", 17));
        assert_eq!(width(&wide, measure), 24 * 6 + 8);
    }

    #[test]
    fn rows_are_fifteen_pixels_strictly_inside_the_side_edges() {
        let bounds = Bounds {
            x: 100,
            y: 50,
            width: 80,
            height: height(3),
        };
        // The last entry (index 2) is the top row: baseline y + 31.
        assert_eq!(bounds.row_baseline(3, 2), 81);
        assert_eq!(bounds.row_at(3, [140, 68]), None);
        assert_eq!(bounds.row_at(3, [140, 69]), Some(2));
        assert_eq!(bounds.row_at(3, [140, 83]), Some(2));
        assert_eq!(bounds.row_at(3, [140, 84]), Some(1));
        assert_eq!(bounds.row_at(3, [140, 98]), Some(1));
        assert_eq!(bounds.row_at(3, [140, 99]), Some(0));
        assert_eq!(bounds.row_at(3, [140, 113]), Some(0));
        // The last three pixel rows of the box belong to no entry.
        assert_eq!(bounds.row_at(3, [140, 114]), None);
        assert_eq!(bounds.row_at(3, [100, 90]), None);
        assert_eq!(bounds.row_at(3, [101, 90]), Some(1));
        assert_eq!(bounds.row_at(3, [179, 90]), Some(1));
        assert_eq!(bounds.row_at(3, [180, 90]), None);
    }

    #[test]
    fn the_open_menu_closes_ten_pixels_away_and_ignores_a_right_click() {
        let list = menu(&[("Cancel", 1005), ("Walk here", 60), ("Talk-to", 17)]);
        let mut state = ContextMenu::default();
        // Closed: nothing happens.
        assert!(state.step(Button::Left, [0, 0], [0, 0]).is_none());
        state.open(list.clone(), 80, [140, 50], [765, 503]);
        assert!(state.menu.open);
        assert_eq!(
            state.bounds,
            Bounds {
                x: 100,
                y: 50,
                width: 80,
                height: 67
            }
        );
        // Exactly ten pixels outside on each side still keeps it open.
        for mouse in [[90, 60], [190, 60], [140, 40], [140, 127]] {
            assert!(state.step(Button::None, mouse, [0, 0]).is_none());
            assert!(state.menu.open, "{mouse:?}");
        }
        // A right click inside does not reopen or move it.
        assert!(state.step(Button::Right, [150, 60], [150, 60]).is_none());
        assert!(state.menu.open);
        assert_eq!(state.bounds.x, 100);
        // Eleven pixels outside closes it, on every side, with any button but
        // the left one.
        for (button, mouse) in [
            (Button::None, [89, 60]),
            (Button::None, [191, 60]),
            (Button::Right, [140, 39]),
            (Button::None, [140, 128]),
        ] {
            let mut state = ContextMenu::default();
            state.open(list.clone(), 80, [140, 50], [765, 503]);
            assert!(state.step(button, mouse, mouse).is_none());
            assert!(!state.menu.open, "{mouse:?}");
        }
    }

    #[test]
    fn a_left_click_runs_the_row_under_it_and_always_closes() {
        let list = menu(&[("Cancel", 1005), ("Walk here", 60), ("Talk-to", 17)]);
        let mut state = ContextMenu::default();
        state.open(list.clone(), 80, [140, 50], [765, 503]);
        // The click position decides, not where the pointer is now.
        let chosen = state.step(Button::Left, [500, 400], [140, 90]);
        assert_eq!(chosen.unwrap().option, "Walk here");
        assert!(!state.menu.open);
        // On the heading, or outside the box: closes without an action.
        for click in [[140, 60], [300, 300], [100, 90]] {
            state.open(list.clone(), 80, [140, 50], [765, 503]);
            assert!(state.step(Button::Left, click, click).is_none());
            assert!(!state.menu.open, "{click:?}");
        }
    }

    #[test]
    fn a_sub_interface_change_trims_the_open_menu() {
        let list = Menu {
            entries: vec![
                entry("Cancel", "", 1005),
                entry("Walk here", "", 60),
                entry("Talk-to", "Hans the servant", 17),
                entry("Drop", "Bucket", 58),
            ],
            open: false,
        };
        let mut state = ContextMenu::default();
        state.open(
            list.clone(),
            width(&list.entries, measure),
            [300, 100],
            [765, 503],
        );
        let before = state.bounds;
        state.interface_changed(measure);
        assert!(state.menu.open);
        assert_eq!(
            names(&state.menu.entries),
            ["Cancel", "Walk here", "Talk-to"]
        );
        // Resized in place: the position is not recomputed.
        assert_eq!([state.bounds.x, state.bounds.y], [before.x, before.y]);
        assert_eq!(state.bounds.height, height(3));
        assert_eq!(state.bounds.width, 24 * 6 + 8);
        // Only Cancel left: the menu closes.
        let list = menu(&[("Cancel", 1005), ("Drop", 58)]);
        state.open(list, 80, [300, 100], [765, 503]);
        state.interface_changed(measure);
        assert!(!state.menu.open);
        assert_eq!(names(&state.menu.entries), ["Cancel"]);
        // A closed menu is left alone.
        let mut closed = ContextMenu {
            menu: menu(&[("Cancel", 1005), ("Drop", 58)]),
            bounds: Bounds::default(),
        };
        closed.interface_changed(measure);
        assert_eq!(closed.menu.entries.len(), 2);
    }
}
