//! DialogueInterpreter.doSubstitutions: `@name`, `@servername` and the
//! gendered `@g[male,female]` form, over UTF-16 code units.
use super::{Error, Host, NodeHandle, Result, Text};

fn units(value: &str) -> Text {
    value.encode_utf16().collect()
}
fn find(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()] == *needle)
}
/// `String.replace(CharSequence, CharSequence)` for a non-empty literal.
fn replace(message: &[u16], target: &[u16], replacement: &[u16]) -> Text {
    let mut out = Vec::with_capacity(message.len());
    let mut at = 0;
    while let Some(found) = find(message, target, at) {
        out.extend_from_slice(&message[at..found]);
        out.extend_from_slice(replacement);
        at = found + target.len();
    }
    out.extend_from_slice(&message[at..]);
    out
}
fn digit(unit: u16) -> Option<usize> {
    (u16::from(b'0')..=u16::from(b'9'))
        .contains(&unit)
        .then(|| usize::from(unit - u16::from(b'0')))
}
fn illegal(message: &str) -> Error {
    Error::IllegalArgument(message.into())
}
/// `Matcher.appendReplacement`'s expansion. The chosen group text is itself
/// the replacement string, so a backslash escapes the next unit and `$`
/// starts a group reference into `groups` (whole match, male, female).
fn expand(replacement: &[u16], groups: [&[u16]; 3]) -> Result<Text> {
    let mut out = Vec::with_capacity(replacement.len());
    let mut cursor = 0;
    while cursor < replacement.len() {
        let unit = replacement[cursor];
        if unit == u16::from(b'\\') {
            cursor += 1;
            let escaped = replacement
                .get(cursor)
                .ok_or_else(|| illegal("character to be escaped is missing"))?;
            out.push(*escaped);
            cursor += 1;
        } else if unit == u16::from(b'$') {
            cursor += 1;
            let next = *replacement
                .get(cursor)
                .ok_or_else(|| illegal("Illegal group reference: group index is missing"))?;
            if next == u16::from(b'{') {
                // The pattern declares no named groups, so every `${...}` form
                // is rejected; only which message it carries differs.
                return Err(illegal("No group with that name"));
            }
            let mut reference = digit(next).ok_or_else(|| illegal("Illegal group reference"))?;
            cursor += 1;
            while let Some(more) = replacement.get(cursor).copied().and_then(digit) {
                let wider = reference * 10 + more;
                if wider > 2 {
                    break;
                }
                reference = wider;
                cursor += 1;
            }
            // `start(group)` rejects a group above groupCount().
            out.extend_from_slice(groups.get(reference).ok_or(Error::Index)?);
        } else {
            out.push(unit);
            cursor += 1;
        }
    }
    Ok(out)
}
/// `Pattern.compile("@g\\[([^,]*),([^\\]]*)\\]")` find/appendReplacement loop.
/// Both groups are possessive in effect: the first cannot hold a comma and the
/// second cannot hold `]`, so each match is fixed by the first comma after
/// `@g[` and the first `]` after that comma.
fn gendered(message: &[u16], group: usize) -> Result<Text> {
    let open = units("@g[");
    let mut out = Vec::with_capacity(message.len());
    let mut appended = 0;
    let mut from = 0;
    while let Some(start) = find(message, &open, from) {
        let Some(comma) = (start + 3..message.len()).find(|&i| message[i] == u16::from(b','))
        else {
            break;
        };
        let Some(end) = (comma + 1..message.len()).find(|&i| message[i] == u16::from(b']')) else {
            break;
        };
        let groups = [
            &message[start..=end],
            &message[start + 3..comma],
            &message[comma + 1..end],
        ];
        let expanded = expand(groups[group], groups)?;
        out.extend_from_slice(&message[appended..start]);
        out.extend(expanded);
        appended = end + 1;
        from = end + 1;
    }
    out.extend_from_slice(&message[appended..]);
    Ok(out)
}
/// `doSubstitutions(player, msg)`. The username and server-name getters run
/// before the null checks of the strings they are substituted into, and
/// `isMale` is read even when the message has no gendered form.
pub fn substitute<H: Host>(
    host: &mut H,
    player: Option<&NodeHandle>,
    message: Option<&[u16]>,
) -> Result<Text> {
    let player = player.ok_or(Error::Null)?;
    let name = host.username(player)?;
    let message = message.ok_or(Error::Null)?;
    let message = replace(message, &units("@name"), &name.ok_or(Error::Null)?);
    let server = host.server_name()?.ok_or(Error::Null)?;
    let message = replace(&message, &units("@servername"), &server);
    let group = if host.is_male(player)? { 1 } else { 2 };
    gendered(&message, group)
}
