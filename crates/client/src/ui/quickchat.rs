//! Quick chat phrase and category definitions (`QuickChatPhraseType`,
//! `QuickChatCatType`) and the dynamic-value text of a received phrase.
//!
//! IDs below 32768 come from the local archive (24) and the rest from the
//! global one (25); the pack names them `qcphrase/ID` and `qccat/ID` with the
//! full ID. Global definitions have 0x8000 set on the IDs they refer to, as
//! the original `postDecode` does.
use super::{latin1, state::State};
use rs09_protocol::buffer::Reader;

/// Bytes each dynamic command occupies in a received phrase.
pub const DECODE_BYTES: [usize; 16] = [2, 2, 4, 2, 1, 8, 4, 1, 4, 4, 2, 1, 1, 1, 4, 1];
/// Bytes each dynamic command occupies in a phrase the client sends.
pub const ENCODE_BYTES: [usize; 16] = [2, 2, 4, 0, 1, 8, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0];
const PARAM_COUNTS: [usize; 16] = [1, 0, 0, 0, 1, 0, 2, 1, 1, 1, 0, 2, 0, 0, 1, 0];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhraseType {
    /// The phrase text split at each `<`; a dynamic value follows every
    /// segment but the last.
    pub segments: Option<Vec<String>>,
    pub automatic_responses: Option<Vec<i32>>,
    /// Command type and parameters for each dynamic value.
    pub commands: Option<Vec<(i32, Vec<i32>)>>,
    pub searchable: bool,
}
impl Default for PhraseType {
    fn default() -> Self {
        Self {
            segments: None,
            automatic_responses: None,
            commands: None,
            searchable: true,
        }
    }
}
impl PhraseType {
    pub fn decode(data: &[u8], global: bool) -> rs09_protocol::Result<Self> {
        let mut r = Reader::new(data);
        let mut p = Self::default();
        loop {
            match r.g1()? {
                0 => break,
                1 => p.segments = Some(latin1(r.gjstr()?).split('<').map(str::to_owned).collect()),
                2 => {
                    let mut responses = vec![];
                    for _ in 0..r.g1()? {
                        responses.push(r.g2()?);
                    }
                    p.automatic_responses = Some(responses);
                }
                3 => {
                    let mut commands = vec![];
                    for _ in 0..r.g1()? {
                        let kind = r.g2()?;
                        let count =
                            *PARAM_COUNTS
                                .get(kind as usize)
                                .ok_or(rs09_protocol::Error::new(
                                    r.position(),
                                    "quick chat command type",
                                ))?;
                        let mut params = Vec::with_capacity(count);
                        for _ in 0..count {
                            params.push(r.g2()?);
                        }
                        commands.push((kind, params));
                    }
                    p.commands = Some(commands);
                }
                4 => p.searchable = false,
                // Other opcodes carry no operands in the original decoder.
                _ => {}
            }
        }
        if global {
            for response in p.automatic_responses.iter_mut().flatten() {
                *response |= 0x8000;
            }
        }
        Ok(p)
    }
    /// `getText`: the segments joined by the "..." placeholder.
    pub fn text(&self) -> String {
        self.segments
            .as_ref()
            .map_or_else(String::new, |segments| segments.join("..."))
    }
    /// `decodeMessage`: each segment followed by its formatted value. `format`
    /// receives the command type, its parameters and the raw value, and returns
    /// None where the original decoder has no special text.
    pub fn message(
        &self,
        body: &[u8],
        mut format: impl FnMut(i32, &[i32], i64) -> Result<Option<String>, String>,
    ) -> Result<String, String> {
        let segments = self
            .segments
            .as_ref()
            .ok_or("Quick chat phrase has no text")?;
        let mut r = Reader::new(body);
        let mut out = String::new();
        for (i, (kind, params)) in self.commands.iter().flatten().enumerate() {
            out.push_str(segments.get(i).ok_or("Quick chat phrase segment")?);
            let value = r
                .g_var_long(DECODE_BYTES[*kind as usize])
                .map_err(|e| e.to_string())?;
            out.push_str(&format(*kind, params, value)?.unwrap_or_else(|| value.to_string()));
        }
        out.push_str(segments.last().ok_or("Quick chat phrase segment")?);
        Ok(out)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CatType {
    pub description: Option<String>,
    /// `(category, shortcut key)`.
    pub subcategories: Option<Vec<(i32, i32)>>,
    /// `(phrase, shortcut key)`.
    pub phrases: Option<Vec<(i32, i32)>>,
}
impl CatType {
    pub fn decode(data: &[u8], global: bool) -> rs09_protocol::Result<Self> {
        let mut r = Reader::new(data);
        let mut c = Self::default();
        loop {
            match r.g1()? {
                0 => break,
                1 => c.description = Some(latin1(r.gjstr()?)),
                op @ (2 | 3) => {
                    let mut entries = vec![];
                    for _ in 0..r.g1()? {
                        let id = r.g2()?;
                        entries.push((id, r.g1()?));
                    }
                    if op == 2 {
                        c.subcategories = Some(entries);
                    } else {
                        c.phrases = Some(entries);
                    }
                }
                _ => {}
            }
        }
        if global {
            for (id, _) in c
                .subcategories
                .iter_mut()
                .flatten()
                .chain(c.phrases.iter_mut().flatten())
            {
                *id |= 0x8000;
            }
        }
        Ok(c)
    }
    pub fn phrase_by_shortcut(&self, shortcut: i32) -> i32 {
        self.phrases
            .iter()
            .flatten()
            .find(|(_, key)| *key == shortcut)
            .map_or(-1, |(id, _)| *id)
    }
    pub fn subcategory_by_shortcut(&self, shortcut: i32) -> i32 {
        self.subcategories
            .iter()
            .flatten()
            .find(|(_, key)| *key == shortcut)
            .map_or(-1, |(id, _)| *id)
    }
}

/// A definition the cache lacks decodes as the empty default, as in the
/// original lists; a pack without the quick chat families is an error.
fn bytes<'a>(state: &'a State, family: &str, id: i32) -> Result<Option<&'a [u8]>, String> {
    let name = format!("{family}/{id}");
    if state.pack.manifest.files.contains_key(&name) {
        return state.pack.get(&name).map(Some);
    }
    if state.pack.manifest.files.contains_key("quickchat/counts") {
        Ok(None)
    } else {
        Err("Quick chat definitions are not in this asset pack".into())
    }
}
pub fn phrase(state: &State, id: i32) -> Result<PhraseType, String> {
    match bytes(state, "qcphrase", id)? {
        Some(data) => PhraseType::decode(data, id >= 32768).map_err(|e| e.to_string()),
        None => Ok(PhraseType::default()),
    }
}
pub fn category(state: &State, id: i32) -> Result<CatType, String> {
    match bytes(state, "qccat", id)? {
        Some(data) => CatType::decode(data, id >= 32768).map_err(|e| e.to_string()),
        None => Ok(CatType::default()),
    }
}

/// `Js5QuickChatCommandDecoder.decode`: enum strings for commands 0, 6, 7 and
/// 11, item names for 1 and 10, and no special text otherwise.
pub fn format(
    state: &mut State,
    kind: i32,
    params: &[i32],
    value: i64,
) -> Result<Option<String>, String> {
    match kind {
        0 | 6 | 7 | 11 => {
            let id = *params.first().ok_or("Quick chat enum parameter")?;
            match super::vm::enum_value(state, id, value as i32, 0, 115, false)? {
                super::HookArg::Text(text) => Ok(Some(text)),
                _ => Ok(Some("null".into())),
            }
        }
        1 | 10 => Ok(Some(state.items.get(&state.pack, value as i32)?.name)),
        _ => Ok(None),
    }
}

/// The text of a received quick chat phrase.
pub fn message(state: &mut State, id: i32, body: &[u8]) -> Result<String, String> {
    let phrase = phrase(state, id)?;
    phrase.message(body, |kind, params, value| {
        format(state, kind, params, value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_text_commands_and_global_response_flags() {
        // "My <skill> level is <level>." with commands 0 (enum 173) and 4.
        let data = [
            &[1][..],
            b"My < level is <.\0",
            &[2, 2, 0, 5, 0, 9],
            &[3, 2, 0, 0, 0, 173, 0, 4, 0, 1],
            &[4, 0],
        ]
        .concat();
        let local = PhraseType::decode(&data, false).unwrap();
        assert_eq!(
            local.segments.as_deref().unwrap(),
            ["My ", " level is ", "."]
        );
        assert_eq!(local.automatic_responses, Some(vec![5, 9]));
        assert_eq!(local.commands, Some(vec![(0, vec![173]), (4, vec![1])]));
        assert!(!local.searchable);
        assert_eq!(local.text(), "My ... level is ....");
        // Global phrases mark the phrases they answer with 0x8000.
        let global = PhraseType::decode(&data, true).unwrap();
        assert_eq!(global.automatic_responses, Some(vec![0x8005, 0x8009]));

        // Command 0 reads two bytes and command 4 reads one.
        let text = local
            .message(&[0, 16, 99], |kind, params, value| {
                Ok((kind == 0).then(|| format!("enum{}[{value}]", params[0])))
            })
            .unwrap();
        assert_eq!(text, "My enum173[16] level is 99.");
        assert!(local.message(&[0, 16], |_, _, _| Ok(None)).is_err());
        // A phrase without dynamic values is just its text.
        let plain = PhraseType::decode(b"\x01Hello!\0\0", false).unwrap();
        assert_eq!(plain.message(&[], |_, _, _| Ok(None)).unwrap(), "Hello!");
        assert!(plain.searchable);
        assert!(
            PhraseType::default()
                .message(&[], |_, _, _| Ok(None))
                .is_err()
        );
        // Command type 16 is outside the original tables.
        assert!(PhraseType::decode(&[3, 1, 0, 16, 0], false).is_err());
    }

    #[test]
    fn eight_byte_values_and_default_number_text() {
        // Command 5 carries eight bytes; with no special text it prints the number.
        let phrase = PhraseType::decode(&[1, b'<', 0, 3, 1, 0, 5, 0], false).unwrap();
        let body = 1_234_567_890_123i64.to_be_bytes();
        assert_eq!(
            phrase.message(&body, |_, _, _| Ok(None)).unwrap(),
            "1234567890123"
        );
    }

    #[test]
    fn categories_and_shortcuts() {
        let data = [
            &[1][..],
            b"General\0",
            &[2, 2, 0, 3, b'a', 0, 4, 200],
            &[3, 1, 0, 7, b'h'],
            &[0],
        ]
        .concat();
        let local = CatType::decode(&data, false).unwrap();
        assert_eq!(local.description.as_deref(), Some("General"));
        // Shortcut bytes are unsigned.
        assert_eq!(local.subcategories, Some(vec![(3, 97), (4, 200)]));
        assert_eq!(local.subcategory_by_shortcut(200), 4);
        assert_eq!(local.subcategory_by_shortcut(1), -1);
        assert_eq!(local.phrase_by_shortcut(i32::from(b'h')), 7);
        let global = CatType::decode(&data, true).unwrap();
        assert_eq!(
            global.subcategories,
            Some(vec![(0x8003, 97), (0x8004, 200)])
        );
        assert_eq!(global.phrases, Some(vec![(0x8007, 104)]));
        assert_eq!(CatType::default().phrase_by_shortcut(1), -1);
    }

    #[test]
    fn byte_tables_match_the_original() {
        assert_eq!(DECODE_BYTES.len(), 16);
        assert_eq!(ENCODE_BYTES.iter().sum::<usize>(), 19);
        assert_eq!(DECODE_BYTES.iter().sum::<usize>(), 42);
        assert_eq!(PARAM_COUNTS.iter().sum::<usize>(), 10);
    }
}
