//! English revision 530 chat prefixes and dialog response wire payloads.
//! Transport owns the variable packet length prefix.
use rs09_protocol::{buffer::Writer, game::Frame, huffman::Huffman};
pub fn dialog(op: i32, text: &str) -> Result<Frame, String> {
    let bytes = text.chars().map(|c| c as u8).collect::<Vec<_>>();
    let mut w = Writer::new();
    let opcode = match op {
        3104 => {
            w.p4(text.parse::<i32>().unwrap_or(0));
            23
        }
        3105 => {
            w.p8(rs09_protocol::login::encode_name(&bytes));
            244
        }
        3106 => {
            w.pjstr(&bytes).map_err(|e| e.to_string())?;
            65
        }
        _ => return Err("Unknown dialog response".into()),
    };
    Ok(Frame {
        opcode,
        payload: w.into_vec(),
    })
}
/// `Cheat.sendCheatPacket`: CLIENT_CHEAT with the text after the `::`.
pub fn command(command: &str) -> Result<Frame, String> {
    let mut w = Writer::new();
    w.pjstr(&command.chars().map(|c| c as u8).collect::<Vec<_>>())
        .map_err(|e| e.to_string())?;
    Ok(Frame {
        opcode: 44,
        payload: w.into_vec(),
    })
}
pub fn public(
    text: &str,
    codec: &Huffman,
    rights: i32,
    restricted: bool,
    quick_chat: bool,
) -> Result<Option<Frame>, String> {
    if let Some(text) = text.strip_prefix("::") {
        return command(text).map(Some);
    }
    if rights == 0 && (restricted || quick_chat) {
        return Ok(None);
    }
    let mut body = text;
    let mut color = 0;
    let mut effect = 0;
    for (i, prefix) in [
        "yellow:", "red:", "green:", "cyan:", "purple:", "white:", "flash1:", "flash2:", "flash3:",
        "glow1:", "glow2:", "glow3:",
    ]
    .iter()
    .enumerate()
    {
        if body.to_ascii_lowercase().starts_with(prefix) {
            color = i as i32;
            body = &body[prefix.len()..];
            break;
        }
    }
    for (i, prefix) in ["wave:", "wave2:", "shake:", "scroll:", "slide:"]
        .iter()
        .enumerate()
    {
        if body.to_ascii_lowercase().starts_with(prefix) {
            effect = i as i32 + 1;
            body = &body[prefix.len()..];
            break;
        }
    }
    let bytes = body.chars().map(|c| c as u8).collect::<Vec<_>>();
    let compressed = codec.encode(&bytes).map_err(|e| e.to_string())?;
    let mut w = Writer::new();
    w.p1(color);
    w.p1(effect);
    w.psmarts(bytes.len() as i32).map_err(|e| e.to_string())?;
    let mut payload = w.into_vec();
    payload.extend(compressed);
    if payload.len() > 255 {
        return Err("Chat exceeds original byte packet bound".into());
    }
    Ok(Some(Frame {
        opcode: 237,
        payload,
    }))
}
