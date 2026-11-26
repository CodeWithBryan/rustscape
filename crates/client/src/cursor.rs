//! Mouse cursors chosen by the menu.
//!
//! A port of `CursorType`, `CursorTypeList.get`, the choice at the end of
//! `LoginManager.processInterface` (`getActiveCursorId`, then the script
//! cursor), `InterfaceList.setCursor` and `SoftwareSprite.toFullImage`.
//!
//! The pinned cache has no cursor types at all (configuration archive 2 has
//! no group 33), no location, NPC, item or component in it names a cursor,
//! and none of its scripts runs CS2 5426 or 5427, so nothing selects a cursor
//! in ordinary play. The original's behaviour for an id without a definition
//! is kept: a default type, which is sprite 0 with the hot spot at the corner.
use crate::{
    interaction::{Menu, MenuEntry},
    pack::Pack,
    ui::sprite::Sheet,
};
use rs09_protocol::{Result, buffer::Reader};

/// A cursor id held in state that starts as none, like the original's
/// `scriptCursorId`, `useCursor` and `examineCursor` (all initially -1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id(pub i32);
impl Default for Id {
    fn default() -> Self {
        Self(-1)
    }
}

/// `CursorType`. Fields keep Java's defaults when their opcode is absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CursorType {
    /// Sprite group in archive 8.
    pub sprite: i32,
    pub hot_spot: [i32; 2],
}
impl CursorType {
    /// `CursorType.decode`: opcode 1 is the sprite, 2 the hot spot, 0 ends,
    /// and any other opcode is skipped without operands. Reading past the end
    /// is an error here where the original would fault.
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut cursor = Self::default();
        loop {
            match r.g1()? {
                0 => return Ok(cursor),
                1 => cursor.sprite = r.g2()?,
                2 => cursor.hot_spot = [r.g1()?, r.g1()?],
                _ => {}
            }
        }
    }
    /// `CursorTypeList.get`: the file `cursor/<id>` decoded, or the default
    /// type when the cache has no such file. A pack that does not carry the
    /// cursor catalogue at all cannot tell, and gives None.
    pub fn load(pack: &Pack, id: i32) -> std::result::Result<Option<Self>, String> {
        let name = format!("cursor/{id}");
        let files = &pack.manifest.files;
        if files.contains_key(&name) {
            return Self::decode(pack.get(&name)?)
                .map(Some)
                .map_err(|e| format!("cursor {id}: {e}"));
        }
        Ok(files
            .contains_key("cursor-catalog-complete")
            .then(Self::default))
    }
}

/// `SoftwareSprite.toFullImage` of a sprite group's first frame: the whole
/// canvas as RGBA, opaque wherever the colour is not zero. The sprite's own
/// alpha channel is not used for a cursor.
pub fn full_image(sheet: &Sheet) -> Option<([usize; 2], Vec<u8>)> {
    let frame = sheet.frames.first()?;
    let [width, height] = sheet.size;
    let mut image = vec![0u8; width.checked_mul(height)?.checked_mul(4)?];
    for y in 0..frame.size[1] {
        for x in 0..frame.size[0] {
            let color = *sheet
                .palette
                .get(usize::from(frame.indices[y * frame.size[0] + x]))?;
            let (px, py) = (x + frame.offset[0], y + frame.offset[1]);
            // A frame that sticks out of its canvas is malformed.
            if color == 0 || px >= width || py >= height {
                continue;
            }
            let at = (py * width + px) * 4;
            image[at..at + 4].copy_from_slice(&[
                (color >> 16) as u8,
                (color >> 8) as u8,
                color as u8,
                255,
            ]);
        }
    }
    Some(([width, height], image))
}

/// CS2 5427's cursors for every "Use" and "Examine" entry: `MiniMenu` passes
/// `MiniMap.useCursor` with actions 22, 14, 33, 26 and 1 and
/// `MiniMap.examineCursor` with 1006, 1004, 1002 and 1007, and nothing else
/// adds those actions.
pub fn apply_script_cursors(menu: &mut Menu, [use_cursor, examine_cursor]: [Id; 2]) {
    for entry in &mut menu.entries {
        match entry.action {
            22 | 14 | 33 | 26 | 1 => entry.cursor = use_cursor.0,
            1006 | 1004 | 1002 | 1007 => entry.cursor = examine_cursor.0,
            _ => {}
        }
    }
}

/// The cursor wanted this frame, before the preference: none while the menu
/// is open, else the top entry's (an entry without one takes the targeting
/// default, as `MiniMenu.add` stores it), and the CS2 5426 script cursor when
/// that is none.
///
/// `shift_click` is `Cheat.shiftClick && Keyboard.pressedKeys[KEY_SHIFT]`:
/// `LoginManager.getActiveCursorId` then reads the entry below the top one
/// of a menu with more than two (see [`crate::context_menu::top_entry`]).
pub fn select(
    entries: &[MenuEntry],
    shift_click: bool,
    menu_open: bool,
    default_cursor: i32,
    script_cursor: i32,
) -> i32 {
    let mut cursor = -1;
    if !menu_open && let Some(index) = crate::context_menu::top_entry(entries.len(), shift_click) {
        cursor = entries[index].cursor;
        if cursor == -1 {
            cursor = default_cursor;
        }
    }
    if cursor == -1 {
        cursor = script_cursor;
    }
    cursor
}

/// What the platform must do after [`Current::set`].
#[derive(Debug, PartialEq, Eq)]
pub enum Change<T> {
    /// Nothing: the cursor is already the wanted one, or could not be loaded
    /// while the system cursor is showing.
    Keep,
    /// Show this cursor.
    Custom(T),
    /// Back to the system cursor.
    System,
}

/// `InterfaceList.currentCursorId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Current(pub i32);
impl Default for Current {
    fn default() -> Self {
        Self(-1)
    }
}
impl Current {
    /// `InterfaceList.setCursor`. `enabled` is the cursors preference; `load`
    /// resolves a cursor's type and sprite and gives None when the sprite
    /// cannot be had, which falls back to the system cursor.
    pub fn set<T>(
        &mut self,
        mut id: i32,
        enabled: bool,
        load: impl FnOnce(i32) -> Option<T>,
    ) -> Change<T> {
        if !enabled {
            id = -1;
        }
        if id == self.0 {
            return Change::Keep;
        }
        let mut change = Change::Keep;
        if id != -1 {
            match load(id) {
                Some(cursor) => {
                    change = Change::Custom(cursor);
                    self.0 = id;
                }
                None => id = -1,
            }
        }
        if id == -1 && self.0 != -1 {
            change = Change::System;
            self.0 = -1;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests: expected values were worked out by hand from the
    //! Java named in the module documentation. `CursorType.decode` is also
    //! compared with the JVM in `tests/menus.rs`.
    use super::*;
    use crate::ui::sprite::Frame;

    fn entry(action: i32, cursor: i32) -> MenuEntry {
        MenuEntry {
            option: String::new(),
            target: String::new(),
            action,
            key: 0,
            args: [0; 2],
            cursor,
        }
    }

    #[test]
    fn decode_reads_the_two_opcodes_and_skips_the_rest() {
        assert_eq!(CursorType::decode(&[0]).unwrap(), CursorType::default());
        assert_eq!(
            CursorType::decode(&[1, 1, 44, 2, 7, 200, 0]).unwrap(),
            CursorType {
                sprite: 300,
                hot_spot: [7, 200]
            }
        );
        // Opcode 9 has no operand; a later opcode 1 replaces the sprite.
        assert_eq!(
            CursorType::decode(&[9, 1, 0, 5, 1, 255, 255, 0, 1]).unwrap(),
            CursorType {
                sprite: 65535,
                hot_spot: [0, 0]
            }
        );
        assert!(CursorType::decode(&[]).is_err());
        assert!(CursorType::decode(&[1, 1]).is_err());
        assert!(CursorType::decode(&[2, 3, 4]).is_err());
    }

    fn pack(files: &[(&str, &[u8])]) -> Pack {
        Pack::lazy(
            "cursor test".into(),
            std::iter::empty(),
            files
                .iter()
                .map(|(name, data)| ((*name).to_owned(), data.to_vec()))
                .collect(),
            Box::new(|name| Err(format!("{name} is not resolved"))),
        )
    }

    #[test]
    fn a_missing_definition_is_the_default_type_only_with_the_whole_catalogue() {
        let whole = pack(&[
            ("cursor/3", &[1, 0, 9, 0]),
            ("cursor-catalog-complete", &[]),
        ]);
        assert_eq!(
            CursorType::load(&whole, 3).unwrap(),
            Some(CursorType {
                sprite: 9,
                hot_spot: [0, 0]
            })
        );
        assert_eq!(
            CursorType::load(&whole, 4).unwrap(),
            Some(CursorType::default())
        );
        assert_eq!(
            CursorType::load(&whole, 65535).unwrap(),
            Some(CursorType::default())
        );
        let partial = pack(&[("cursor/3", &[1, 0, 9])]);
        assert_eq!(CursorType::load(&partial, 4).unwrap(), None);
        assert!(CursorType::load(&partial, 3).is_err());
    }

    #[test]
    fn the_full_image_is_the_canvas_with_opaque_coloured_pixels() {
        let sheet = Sheet {
            size: [4, 3],
            palette: vec![0, 0x112233, 1],
            frames: vec![Frame {
                offset: [1, 1],
                size: [2, 2],
                indices: vec![1, 0, 2, 1],
                // Ignored: a cursor is cut out by colour alone.
                alpha: vec![10, 20, 30, 40],
                has_alpha: true,
            }],
        };
        let (size, image) = full_image(&sheet).unwrap();
        assert_eq!(size, [4, 3]);
        let pixel = |x: usize, y: usize| &image[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
        assert_eq!(pixel(0, 0), [0, 0, 0, 0]);
        assert_eq!(pixel(1, 1), [0x11, 0x22, 0x33, 255]);
        assert_eq!(pixel(2, 1), [0, 0, 0, 0]);
        assert_eq!(pixel(1, 2), [0, 0, 1, 255]);
        assert_eq!(pixel(2, 2), [0x11, 0x22, 0x33, 255]);
        assert_eq!(pixel(3, 2), [0, 0, 0, 0]);
        let empty = Sheet {
            size: [4, 3],
            palette: vec![0],
            frames: vec![],
        };
        assert!(full_image(&empty).is_none());
    }

    #[test]
    fn use_and_examine_entries_take_the_script_cursors() {
        let mut menu = Menu {
            entries: [1005, 60, 22, 14, 33, 26, 1, 1006, 1004, 1002, 1007, 40, 9]
                .map(|action| entry(action, -1))
                .to_vec(),
            open: false,
        };
        menu.entries[12].cursor = 4;
        apply_script_cursors(&mut menu, [Id(7), Id(8)]);
        assert_eq!(
            menu.entries.iter().map(|e| e.cursor).collect::<Vec<_>>(),
            [-1, -1, 7, 7, 7, 7, 7, 8, 8, 8, 8, -1, 4]
        );
        // The default of 5427 never run: both stay none.
        assert_eq!(Id::default(), Id(-1));
        apply_script_cursors(&mut menu, [Id::default(); 2]);
        assert!(menu.entries[2..11].iter().all(|e| e.cursor == -1));
    }

    #[test]
    fn selection_prefers_the_top_entry_then_the_default_then_the_script() {
        let entries = [entry(1005, -1), entry(60, -1), entry(17, 6)];
        assert_eq!(select(&entries, false, false, -1, 2), 6);
        // The menu is open: only the script cursor.
        assert_eq!(select(&entries, false, true, 9, 2), 2);
        assert_eq!(select(&entries, false, true, 9, -1), -1);
        // A top entry without a cursor takes the targeting default.
        assert_eq!(select(&entries[..2], false, false, 9, 2), 9);
        assert_eq!(select(&entries[..2], false, false, -1, 2), 2);
        assert_eq!(select(&entries[..2], false, false, -1, -1), -1);
        // An explicit cursor 0 is a cursor, not "none".
        assert_eq!(select(&[entry(1005, 0)], false, false, 9, 2), 0);
        assert_eq!(select(&[], false, false, 9, 2), 2);
    }

    #[test]
    fn shift_click_selects_the_cursor_of_the_entry_below_the_top_one() {
        // LoginManager.java:1497.
        let entries = [entry(1005, -1), entry(60, 3), entry(17, 6)];
        assert_eq!(select(&entries, true, false, -1, 2), 3);
        // Not with two entries, and not under the open menu.
        assert_eq!(select(&entries[..2], true, false, -1, 2), 3);
        assert_eq!(select(&entries[1..], true, false, -1, 2), 6);
        assert_eq!(select(&entries, true, true, -1, 2), 2);
    }

    #[test]
    fn set_cursor_follows_the_preference_and_falls_back_when_the_sprite_is_missing() {
        let mut current = Current::default();
        let mut loads = 0;
        let mut load = |id: i32| {
            loads += 1;
            (id != 13).then_some(id * 10)
        };
        // Disabled: nothing is even looked up.
        assert_eq!(current.set(5, false, &mut load), Change::Keep);
        assert_eq!(current.set(5, true, &mut load), Change::Custom(50));
        assert_eq!(current, Current(5));
        // Unchanged: no second lookup.
        assert_eq!(current.set(5, true, &mut load), Change::Keep);
        assert_eq!(current.set(6, true, &mut load), Change::Custom(60));
        // A cursor whose sprite is missing restores the system cursor.
        assert_eq!(current.set(13, true, &mut load), Change::System);
        assert_eq!(current, Current(-1));
        // And is looked up again every frame without any change.
        assert_eq!(current.set(13, true, &mut load), Change::Keep);
        assert_eq!(current.set(6, true, &mut load), Change::Custom(60));
        assert_eq!(current.set(-1, true, &mut load), Change::System);
        assert_eq!(current.set(-1, true, &mut load), Change::Keep);
        // Turning the preference off drops a custom cursor.
        assert_eq!(current.set(6, true, &mut load), Change::Custom(60));
        assert_eq!(current.set(6, false, &mut load), Change::System);
        assert_eq!(loads, 6);
    }
}
