//! The chat blocks of PLAYER_INFO, from `Protocol.readExtendedPlayerInfo`:
//! flag 0x80 (public chat, typed or quick) and flag 0x20 (forced chat).
//!
//! These functions decide and build; the caller owns the player list, the
//! overhead text and the chat box.
use super::{latin1, vm::ChatLine};

/// Set in the colour/effect word of a public chat block that carries a quick
/// chat phrase instead of packed text.
pub const QUICK_CHAT: i32 = 0x8000;

/// What the session knows when a public chat block is read.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Context {
    /// `playerUnderage && !parentalChatConsent || mapQuickChat`.
    pub restricted: bool,
    /// `Player.inTutorialIsland != 0`.
    pub tutorial: bool,
    /// The speaker's base-37 name is on the ignore list.
    pub ignored: bool,
}

/// A public chat block that is shown: the overhead text with its colour and
/// effect, and the chat-box line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shown {
    /// `player.chatMessage`: the message with surrounding blanks trimmed.
    pub overhead: String,
    pub color: i32,
    pub effect: i32,
    pub line: ChatLine,
}

/// Whether a public chat block is shown at all. Staff (rights above 1) skip
/// both the chat restriction and the ignore list; quick chat skips only the
/// restriction; Tutorial Island hides everything.
pub fn wanted(color_effect: i32, rights: i32, context: Context) -> bool {
    let quick = color_effect & QUICK_CHAT != 0;
    let ignore = rights <= 1
        && if !quick && context.restricted {
            true
        } else {
            context.ignored
        };
    !ignore && !context.tutorial
}

/// `JagString.trim`: bytes 0 to 32 and the no-break space at either end.
fn trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ' || c == '\u{a0}')
}

/// Build what a wanted public chat block shows. `name` is the speaker's
/// display name and `payload` the block's bytes in reading order. Typed chat
/// uses `typed` (the unpacked, sentence-cased and escaped text); quick chat
/// reads its phrase id from the first two bytes and asks `phrase` for the
/// text of the remaining bytes.
pub fn show(
    color_effect: i32,
    rights: i32,
    name: &str,
    payload: &[u8],
    typed: impl FnOnce(&[u8]) -> String,
    phrase: impl FnOnce(i32, &[u8]) -> Result<String, String>,
) -> Result<Shown, String> {
    let quick = color_effect & QUICK_CHAT != 0;
    let (text, id, word) = if quick {
        let [high, low, body @ ..] = payload else {
            return Err("Public quick chat has no phrase id".into());
        };
        let id = i32::from(*high) << 8 | i32::from(*low);
        // The flag is cleared before the colour and effect are taken.
        (phrase(id, body)?, id, color_effect & 0x7fff)
    } else {
        (typed(payload), -1, color_effect)
    };
    let kind = match (quick, rights) {
        (true, _) => 17,
        (false, 1 | 2) => 1,
        (false, _) => 2,
    };
    let name = match rights {
        2 => format!("<img=1>{name}"),
        1 => format!("<img=0>{name}"),
        _ => name.to_owned(),
    };
    Ok(Shown {
        overhead: trim(&text).to_owned(),
        color: word >> 8,
        effect: word & 0xff,
        line: ChatLine {
            kind,
            text,
            name,
            clan: String::new(),
            phrase: id,
        },
    })
}

/// Forced chat (flag 0x20): the overhead text and, when the text starts with
/// `~` or the speaker is the local player, a type-2 chat-box line. The `~` is
/// removed from both. Unlike public chat, neither the ignore list nor
/// Tutorial Island applies. An empty text, on which the original's
/// `charAt(0)` fails, is treated as plain text here.
pub fn forced(text: &[u8], name: &str, is_self: bool) -> (String, Option<ChatLine>) {
    let (text, marked) = match text.strip_prefix(b"~") {
        Some(rest) => (latin1(rest), true),
        None => (latin1(text), false),
    };
    let line = (marked || is_self).then(|| ChatLine {
        kind: 2,
        text: text.clone(),
        name: name.to_owned(),
        clan: String::new(),
        phrase: -1,
    });
    (text, line)
}

#[cfg(test)]
mod tests {
    //! Transcription tests against `Protocol.readExtendedPlayerInfo` (flags
    //! 0x80 and 0x20), `QuickChatPhraseType.decodePhrase` and
    //! `JagString.trim`; the original classes are not executed.
    use super::*;

    const FREE: Context = Context {
        restricted: false,
        tutorial: false,
        ignored: false,
    };

    #[test]
    fn restriction_ignore_list_and_tutorial_island_decide_what_is_shown() {
        let restricted = Context {
            restricted: true,
            ..FREE
        };
        let ignored = Context {
            ignored: true,
            ..FREE
        };
        let tutorial = Context {
            tutorial: true,
            ..FREE
        };
        for rights in 0..=3 {
            assert!(wanted(0x0102, rights, FREE));
            // Tutorial Island hides typed and quick chat, staff included.
            assert!(!wanted(0x0102, rights, tutorial));
            assert!(!wanted(QUICK_CHAT, rights, tutorial));
            let staff = rights > 1;
            assert_eq!(wanted(0, rights, restricted), staff);
            assert_eq!(wanted(0, rights, ignored), staff);
            // Quick chat is exempt from the restriction but not the list.
            assert!(wanted(QUICK_CHAT, rights, restricted));
            assert_eq!(wanted(QUICK_CHAT, rights, ignored), staff);
        }
        // With both, a typed message is dropped by the restriction before the
        // list is read; a quick one is dropped by the list.
        let both = Context {
            restricted: true,
            ignored: true,
            tutorial: false,
        };
        assert!(!wanted(0, 0, both) && !wanted(QUICK_CHAT, 1, both));
    }

    #[test]
    fn typed_chat_keeps_its_colour_effect_and_crown() {
        let typed = |payload: &[u8]| format!(" <lt>{}> ", payload.len());
        let none = |_: i32, _: &[u8]| -> Result<String, String> { panic!("not quick chat") };
        let shown = show(0x0b03, 0, "Bob", &[1, 2, 3], typed, none).unwrap();
        // The chat box keeps the text as decoded; the overhead copy is trimmed.
        assert_eq!(shown.line.text, " <lt>3> ");
        assert_eq!(shown.overhead, "<lt>3>");
        assert_eq!((shown.color, shown.effect), (11, 3));
        assert_eq!(
            (shown.line.kind, shown.line.name.as_str(), shown.line.phrase),
            (2, "Bob", -1)
        );
        for (rights, kind, name) in [
            (1, 1, "<img=0>Bob"),
            (2, 1, "<img=1>Bob"),
            // Rights 3 and above are not crowned and use the plain type.
            (3, 2, "Bob"),
        ] {
            let shown = show(0, rights, "Bob", &[], typed, none).unwrap();
            assert_eq!((shown.line.kind, shown.line.name.as_str()), (kind, name));
        }
    }

    #[test]
    fn quick_chat_reads_the_phrase_id_and_drops_the_flag() {
        let typed = |_: &[u8]| -> String { panic!("not typed chat") };
        let phrase = |id: i32, body: &[u8]| Ok(format!("phrase {id} {body:?} "));
        // Colour 0x7f survives; 0x80 of the high byte is the flag.
        let shown = show(0xff05, 2, "Bob", &[0x81, 0x02, 9, 8], typed, phrase).unwrap();
        assert_eq!(shown.line.text, "phrase 33026 [9, 8] ");
        assert_eq!(shown.overhead, "phrase 33026 [9, 8]");
        assert_eq!((shown.color, shown.effect), (0x7f, 5));
        assert_eq!(
            (shown.line.kind, shown.line.name.as_str(), shown.line.phrase),
            (17, "<img=1>Bob", 33026)
        );
        // Type 17 whatever the rights; only the crown differs.
        let shown = show(QUICK_CHAT, 0, "Bob", &[0, 7], typed, phrase).unwrap();
        assert_eq!(
            (shown.line.kind, shown.line.name.as_str(), shown.line.phrase),
            (17, "Bob", 7)
        );
        assert_eq!((shown.color, shown.effect), (0, 0));
        // A block too short for the id, or a phrase that cannot be decoded.
        assert!(show(QUICK_CHAT, 0, "Bob", &[1], typed, phrase).is_err());
        let broken = |_: i32, _: &[u8]| Err("no such phrase".to_string());
        assert_eq!(
            show(QUICK_CHAT, 0, "Bob", &[0, 7], typed, broken),
            Err("no such phrase".into())
        );
    }

    #[test]
    fn trimming_follows_the_original_blank_set() {
        assert_eq!(trim("\u{a0} \t a b \u{1}\u{a0}"), "a b");
        assert_eq!(trim("   "), "");
        // Characters above 32 other than the no-break space stay.
        assert_eq!(trim("\u{a1}x\u{a1}"), "\u{a1}x\u{a1}");
    }

    #[test]
    fn forced_chat_reaches_the_chat_box_for_a_tilde_or_the_local_player() {
        let line = |text: &str| ChatLine {
            kind: 2,
            text: text.into(),
            name: "Bob".into(),
            clan: String::new(),
            phrase: -1,
        };
        // Another player's plain forced chat is overhead only.
        assert_eq!(forced(b"Ouch!", "Bob", false), ("Ouch!".into(), None));
        // A tilde is removed and the text is also logged.
        assert_eq!(
            forced(b"~Ouch!", "Bob", false),
            ("Ouch!".into(), Some(line("Ouch!")))
        );
        // The local player's own forced chat is always logged.
        assert_eq!(
            forced(b"Ouch!", "Bob", true),
            ("Ouch!".into(), Some(line("Ouch!")))
        );
        assert_eq!(
            forced(b"~Ouch!", "Bob", true),
            ("Ouch!".into(), Some(line("Ouch!")))
        );
        // Only the first tilde is a marker; nothing is escaped or trimmed.
        assert_eq!(
            forced(b"~~<x> ", "Bob", false),
            ("~<x> ".into(), Some(line("~<x> ")))
        );
        assert_eq!(forced(b"", "Bob", false), (String::new(), None));
    }
}
