//! DialUtils splitting and the original plain DialogueInterpreter message body.
//! Interpreter/packet/manager receivers stay explicit and late-bound.
use super::{
    interfaces::{Error, Result},
    player_walk::NodeHandle,
};
pub type Text = Vec<u16>;
pub type Lines = crate::definitions::item::Shared<Vec<Option<Text>>>;
fn tags(text: &[u16]) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut i = 0;
    while i < text.len() {
        if text[i] != u16::from(b'<') {
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < text.len()
            && (matches!(text[end], 47 | 61)
                || text[end] <= 127 && (text[end] as u8).is_ascii_alphanumeric())
        {
            end += 1;
        }
        if end > i + 1 && text.get(end) == Some(&u16::from(b'>')) {
            out.push((i, end + 1));
            i = end + 1;
        } else {
            i += 1;
        }
    }
    out
}
fn visible(text: &[u16]) -> usize {
    text.len() - tags(text).into_iter().map(|(a, b)| b - a).sum::<usize>()
}
fn push(
    lines: &mut Vec<Text>,
    index: &mut usize,
    line: &mut Text,
    accumulator: &mut i32,
    open: &mut Vec<Text>,
) -> Result<()> {
    if line.is_empty() {
        return Ok(());
    }
    for (a, b) in tags(line) {
        let tag = &line[a..b];
        if tag[1] == u16::from(b'/') {
            for i in (0..open.len()).rev() {
                let name = &tag[2..tag.len() - 1];
                let open_name = open[i].get(1..tag.len() - 2).ok_or(Error::Index)?;
                if name == open_name {
                    // LinkedList.remove(Object) removes the first equal tag,
                    // even when the descending iterator found a later alias.
                    let first = open.iter().position(|t| *t == open[i]).unwrap();
                    open.remove(first);
                    break;
                }
            }
        } else {
            open.push(tag.to_vec());
        }
    }
    if lines.len() == *index {
        lines.push(line.clone());
    } else {
        lines[*index] = line.clone();
    }
    *index += 1;
    line.clear();
    *accumulator = 0;
    for tag in open.iter() {
        line.extend_from_slice(tag);
    }
    open.clear();
    Ok(())
}
pub fn split_lines(message: &[u16], limit: i32) -> Result<Vec<Text>> {
    if limit <= 0 {
        return Err(Error::Binding("nonpositive dialogue line limit".into()));
    }
    let count = (((visible(message) as f32) / (limit as f32)).ceil() as i32) as usize;
    let mut lines = vec![vec![]; count];
    if count == 1 {
        lines[0] = message.to_vec();
        return Ok(lines);
    }
    let tokens = message
        .split(|c| *c == u16::from(b' '))
        .map(|s| (s, visible(s) as i32))
        .collect::<Vec<_>>();
    let mut next = 0;
    let mut index = 0;
    let mut line = vec![];
    let mut accumulator = 0i32;
    let mut open = vec![];
    while let Some(&(token, width)) = tokens.get(next) {
        let space = visible(&line) != 0;
        accumulator = accumulator
            .wrapping_add(width)
            .wrapping_add(i32::from(space));
        if accumulator > limit {
            if line.is_empty() {
                // Original pushLine returns without resetting the accumulator.
                // Skip identical pure retries until its signed Int wraps; do
                // not silently wrap/reflow an overlong token differently.
                let distance = i64::from(i32::MAX) + 1 - i64::from(accumulator);
                let retries = (distance + i64::from(width) - 1) / i64::from(width);
                accumulator = accumulator.wrapping_add(width.wrapping_mul(retries as i32));
            } else {
                let old = line.clone();
                push(
                    &mut lines,
                    &mut index,
                    &mut line,
                    &mut accumulator,
                    &mut open,
                )?;
                if line == old && width > limit {
                    return Err(Error::Binding("non-progressing tagged dialogue".into()));
                }
                continue;
            }
        }
        if space {
            line.push(u16::from(b' '));
        }
        line.extend_from_slice(token);
        next += 1;
    }
    push(
        &mut lines,
        &mut index,
        &mut line,
        &mut accumulator,
        &mut open,
    )?;
    if lines.len() > 4 {
        let fifth = lines[4].clone();
        lines[3].extend("<br>".encode_utf16());
        lines[3].extend(fifth);
        lines.truncate(4);
    }
    Ok(lines)
}
pub trait Context {
    type Interpreter;
    type Dispatch;
    type Manager;
    type Component;
    fn receiver(&mut self, d: &Self::Interpreter) -> Result<()>;
    fn interpreter(&mut self, p: &NodeHandle) -> Result<Self::Interpreter>;
    fn player(&mut self, d: &Self::Interpreter) -> Result<NodeHandle>;
    fn dispatch(&mut self, p: &NodeHandle) -> Result<Self::Dispatch>;
    fn text(&mut self, d: Self::Dispatch, text: Option<Text>, iface: i32, child: i32)
    -> Result<()>;
    fn manager(&mut self, p: &NodeHandle) -> Result<Self::Manager>;
    fn open_chatbox(&mut self, m: Self::Manager, id: i32) -> Result<()>;
    fn chatbox(&mut self, m: Self::Manager) -> Result<Option<Self::Component>>;
    fn id(&mut self, c: Self::Component) -> Result<i32>;
    fn hidden(&mut self, d: Self::Dispatch, id: i32, child: i32, hidden: bool) -> Result<()>;
}
pub fn send_lines<C: Context>(
    ctx: &mut C,
    d: C::Interpreter,
    lines: Option<&Lines>,
) -> Result<Option<C::Component>> {
    ctx.receiver(&d)?;
    let lines = lines.ok_or(Error::Null)?;
    let count = lines.read().len();
    if !(1..=4).contains(&count) {
        return Ok(None);
    }
    let id = 209 + count as i32;
    for i in 0..count {
        let p = ctx.player(&d)?;
        let dispatch = ctx.dispatch(&p)?;
        let line = lines.read().get(i).cloned().ok_or(Error::Index)?;
        ctx.text(dispatch, line, id, i as i32 + 1)?;
    }
    let p = ctx.player(&d)?;
    let manager = ctx.manager(&p)?;
    ctx.open_chatbox(manager, id)?;
    let p = ctx.player(&d)?;
    let dispatch = ctx.dispatch(&p)?;
    let p = ctx.player(&d)?;
    let manager = ctx.manager(&p)?;
    let c = ctx.chatbox(manager)?.ok_or(Error::Null)?;
    let id = ctx.id(c)?;
    ctx.hidden(dispatch, id, 1, false)?;
    let p = ctx.player(&d)?;
    let manager = ctx.manager(&p)?;
    ctx.chatbox(manager)
}
pub fn send<C: Context>(
    ctx: &mut C,
    p: &NodeHandle,
    message: &[u16],
) -> Result<Option<C::Component>> {
    let d = ctx.interpreter(p)?;
    let lines = split_lines(message, 54)?
        .into_iter()
        .map(Some)
        .collect::<Vec<_>>();
    send_lines(ctx, d, Some(&Lines::new(lines)))
}
