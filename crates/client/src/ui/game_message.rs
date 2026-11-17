//! MESSAGE_GAME (opcode 70) request suffixes, from `Protocol.readPacketInternal`.
//!
//! A game message ending in one of ten suffixes becomes a typed chat line
//! instead of plain type-0 text. The suffixes are tested in the original's
//! order with `JagString.endsWith`, and the fields are cut with its
//! `indexOf`, which finds the first occurrence, not the suffix itself.
use super::{latin1, vm::ChatLine};

/// How a suffix row builds its chat line.
#[derive(Clone, Copy)]
enum Row {
    /// The name before the first colon, a fixed text, the ignore list and the
    /// Tutorial Island test.
    Request,
    /// As `Request`, with the text between the first colon and the suffix.
    Detail,
    /// No name; the text before the first occurrence of the suffix.
    Notice { tutorial_hides: bool },
}

/// `(suffix, chat type, row)` in the order `readPacketInternal` tests them.
const ROWS: [(&[u8], i32, Row); 10] = [
    (b":tradereq:", 4, Row::Request),
    (b":chalreq:", 8, Row::Detail),
    (b":assistreq:", 10, Row::Request),
    (
        b":clan:",
        11,
        Row::Notice {
            tutorial_hides: false,
        },
    ),
    (
        b":trade:",
        12,
        Row::Notice {
            tutorial_hides: true,
        },
    ),
    (
        b":assist:",
        13,
        Row::Notice {
            tutorial_hides: true,
        },
    ),
    (b":duelstake:", 14, Row::Request),
    (b":duelfriend:", 15, Row::Request),
    (b":clanreq:", 16, Row::Request),
    (b":allyreq:", 21, Row::Detail),
];

/// `LocalizedText.TRADEREQ` for `client.language` (0 English, 1 German,
/// 2 French; the original accepts no other value).
pub fn trade_request_text(language: i32) -> &'static str {
    match language {
        1 => "m\u{f6}chte mit Ihnen handeln.",
        2 => "voudrait faire un \u{e9}change avec vous.",
        _ => "wishes to trade with you.",
    }
}

/// The chat line a game message adds, or None when the ignore list or
/// Tutorial Island suppresses it.
///
/// `ignored` receives the base-37 key of the requesting name
/// (`JagString.encode37`, at most twelve characters) and is asked only for the
/// rows that consult `IgnoreList`. A `:chalreq:` or `:allyreq:` message with
/// no colon before the suffix makes the original's `substring` allocate a
/// negative length; that failure is reported here as an error, and only where
/// the original reaches it (after the ignore and Tutorial Island tests).
pub fn classify(
    message: &[u8],
    language: i32,
    tutorial: bool,
    ignored: impl Fn(i64) -> bool,
) -> Result<Option<ChatLine>, String> {
    let line = |kind: i32, name: &[u8], text: String| ChatLine {
        kind,
        text,
        name: latin1(name),
        clan: String::new(),
        phrase: -1,
    };
    for (suffix, kind, row) in ROWS {
        if !message.ends_with(suffix) {
            continue;
        }
        return Ok(match row {
            Row::Notice { tutorial_hides } => {
                let end = message
                    .windows(suffix.len())
                    .position(|window| window == suffix)
                    .unwrap_or(message.len() - suffix.len());
                (!(tutorial_hides && tutorial)).then(|| line(kind, b"", latin1(&message[..end])))
            }
            Row::Request | Row::Detail => {
                // The suffix starts with a colon, so one is always found.
                let colon = message
                    .iter()
                    .position(|&byte| byte == b':')
                    .unwrap_or(message.len() - suffix.len());
                let name = &message[..colon];
                if ignored(rs09_protocol::login::encode_name(name)) || tutorial {
                    None
                } else if matches!(row, Row::Request) {
                    let text = if kind == 4 {
                        trade_request_text(language)
                    } else {
                        ""
                    };
                    Some(line(kind, name, text.into()))
                } else {
                    // Both detail suffixes are nine characters long, which is
                    // the literal the original subtracts.
                    let detail = message
                        .get(colon + 1..message.len() - 9)
                        .ok_or("Game message request has no detail field")?;
                    Some(line(kind, name, latin1(detail)))
                }
            }
        });
    }
    Ok(Some(line(0, b"", latin1(message))))
}

#[cfg(test)]
mod tests {
    //! Transcription tests: expected values are worked out by hand from
    //! `Protocol.java` (MESSAGE_GAME), `JagString.java` (`endsWith`,
    //! `indexOf`, `substring`, `encode37`) and `LocalizedText.java`. They do
    //! not execute the original classes.
    use super::*;

    fn shown(message: &str) -> (i32, String, String) {
        let line = classify(message.as_bytes(), 0, false, |_| false)
            .unwrap()
            .unwrap();
        assert_eq!((line.phrase, line.clan.as_str()), (-1, ""));
        (line.kind, line.name, line.text)
    }
    fn own(kind: i32, name: &str, text: &str) -> (i32, String, String) {
        (kind, name.into(), text.into())
    }

    #[test]
    fn every_suffix_has_its_chat_type_name_and_text() {
        assert_eq!(
            shown("Bob:tradereq:"),
            own(4, "Bob", "wishes to trade with you.")
        );
        assert_eq!(shown("Bob:5 vs 5:chalreq:"), own(8, "Bob", "5 vs 5"));
        assert_eq!(shown("Bob:assistreq:"), own(10, "Bob", ""));
        assert_eq!(
            shown("Now talking in clan channel Zed:clan:"),
            own(11, "", "Now talking in clan channel Zed")
        );
        assert_eq!(
            shown("Sending trade offer...:trade:"),
            own(12, "", "Sending trade offer...")
        );
        assert_eq!(
            shown("Sending assistance request...:assist:"),
            own(13, "", "Sending assistance request...")
        );
        assert_eq!(shown("Bob:duelstake:"), own(14, "Bob", ""));
        assert_eq!(shown("Bob:duelfriend:"), own(15, "Bob", ""));
        assert_eq!(shown("Bob:clanreq:"), own(16, "Bob", ""));
        assert_eq!(shown("Bob:Team A:allyreq:"), own(21, "Bob", "Team A"));
        assert_eq!(
            shown("Welcome to RuneScape."),
            own(0, "", "Welcome to RuneScape.")
        );
        // A colon alone is not a request; the whole text is kept.
        assert_eq!(shown("Bob: hello"), own(0, "", "Bob: hello"));
        assert_eq!(shown(""), own(0, "", ""));
    }

    #[test]
    fn fields_are_cut_at_the_first_occurrence() {
        // The name ends at the first colon, wherever the suffix is.
        assert_eq!(
            shown("A:B:tradereq:"),
            own(4, "A", "wishes to trade with you.")
        );
        // The detail runs from the first colon to the suffix and may itself
        // hold colons; an empty detail is allowed.
        assert_eq!(shown("A:b:c:chalreq:"), own(8, "A", "b:c"));
        assert_eq!(shown("A::chalreq:"), own(8, "A", ""));
        // A notice is cut at the first occurrence of its suffix.
        assert_eq!(shown("x:clan:y:clan:"), own(11, "", "x"));
        assert_eq!(shown(":trade:"), own(12, "", ""));
        // With only the suffix the name is empty.
        assert_eq!(shown(":duelstake:"), own(14, "", ""));
        // Suffixes are tested in the original's order, and none of them ends
        // with another, so ":tradereq:" is never read as ":trade:".
        assert_eq!(shown("a:trade:tradereq:").0, 4);
        // Latin-1 bytes survive in both fields.
        let line = classify(b"J\xf6:d\xe9tail:allyreq:", 0, false, |_| false)
            .unwrap()
            .unwrap();
        assert_eq!(
            (line.name.as_str(), line.text.as_str()),
            ("J\u{f6}", "d\u{e9}tail")
        );
    }

    #[test]
    fn the_ignore_list_is_compared_on_base37_keys() {
        // encode37 folds case and maps other characters to the blank digit,
        // so "Mod Bob" and "mod_bob" are the same key.
        let key = rs09_protocol::login::encode_name(b"mod_bob");
        for suffix in [
            ":tradereq:",
            ":assistreq:",
            ":duelstake:",
            ":duelfriend:",
            ":clanreq:",
        ] {
            let message = format!("Mod Bob{suffix}");
            assert_eq!(
                classify(message.as_bytes(), 0, false, |k| k == key).unwrap(),
                None,
                "{suffix}"
            );
            assert!(
                classify(message.as_bytes(), 0, false, |k| k == key + 1)
                    .unwrap()
                    .is_some()
            );
        }
        for suffix in [":chalreq:", ":allyreq:"] {
            let message = format!("Mod Bob:x{suffix}");
            assert_eq!(
                classify(message.as_bytes(), 0, false, |k| k == key).unwrap(),
                None
            );
        }
        // Notices and plain messages never consult the list.
        for message in [
            "Mod Bob:clan:",
            "Mod Bob:trade:",
            "Mod Bob:assist:",
            "Mod Bob",
        ] {
            let line = classify(message.as_bytes(), 0, false, |_| {
                panic!("the ignore list is not consulted for {message}")
            });
            assert!(line.unwrap().is_some());
        }
        // Only the first twelve characters of a name take part.
        let long = rs09_protocol::login::encode_name(b"abcdefghijkl");
        assert_eq!(
            classify(b"abcdefghijklmnop:tradereq:", 0, false, |k| k == long).unwrap(),
            None
        );
    }

    #[test]
    fn tutorial_island_hides_every_row_but_clan_notices_and_plain_text() {
        for message in [
            "Bob:tradereq:",
            "Bob:x:chalreq:",
            "Bob:assistreq:",
            "x:trade:",
            "x:assist:",
            "Bob:duelstake:",
            "Bob:duelfriend:",
            "Bob:clanreq:",
            "Bob:x:allyreq:",
        ] {
            assert_eq!(
                classify(message.as_bytes(), 0, true, |_| false).unwrap(),
                None,
                "{message}"
            );
        }
        for (message, kind) in [("x:clan:", 11), ("Plain text", 0)] {
            let line = classify(message.as_bytes(), 0, true, |_| false).unwrap();
            assert_eq!(line.unwrap().kind, kind);
        }
    }

    #[test]
    fn a_detail_request_without_a_detail_fails_only_where_the_original_does() {
        // "Bob:chalreq:" asks substring for (12 - 9) - (3 + 1) = -1 bytes.
        for message in ["Bob:chalreq:", ":chalreq:", "Bob:allyreq:"] {
            assert!(
                classify(message.as_bytes(), 0, false, |_| false).is_err(),
                "{message}"
            );
            // The ignore and Tutorial Island tests come first and skip it.
            assert_eq!(
                classify(message.as_bytes(), 0, false, |_| true).unwrap(),
                None
            );
            assert_eq!(
                classify(message.as_bytes(), 0, true, |_| false).unwrap(),
                None
            );
        }
    }

    #[test]
    fn the_trade_request_text_follows_the_client_language() {
        let text = |language| {
            classify(b"Bob:tradereq:", language, false, |_| false)
                .unwrap()
                .unwrap()
                .text
        };
        assert_eq!(text(0), "wishes to trade with you.");
        // "m-Ochte mit Ihnen handeln)3" and "voudrait faire un -Bchange avec
        // vous)3" after JagString.parse's escapes: -O is 246, -B is 233 and
        // )3 is a full stop.
        assert_eq!(text(1), "m\u{f6}chte mit Ihnen handeln.");
        assert_eq!(text(2), "voudrait faire un \u{e9}change avec vous.");
    }
}
