//! A development aid beside the frame dump (native only). The lines of a
//! file named `inject` in the folder `RS09_FRAME_DUMP` names are server
//! packets, `OPCODE HEX-PAYLOAD`; the next cycle that reads the connection
//! takes them as received after what the server sent, and the file is
//! removed. A live run can so put the client through what the example
//! servers never send, such as the self-teleport onto another level that
//! climbing stairs is (PLAYER_INFO 225, `Player.plane` read with the tile).
//! Nothing is sent to the server: it still holds the player where it did.
use rs09_protocol::game::Frame;

/// The packets waiting in the `inject` file, if there is one.
pub(super) fn take() -> Vec<Frame> {
    let Some(folder) = std::env::var_os("RS09_FRAME_DUMP") else {
        return Vec::new();
    };
    let path = std::path::Path::new(&folder).join("inject");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let _ = std::fs::remove_file(&path);
    parse(&text).unwrap_or_else(|error| {
        eprintln!("inject: {error}");
        Vec::new()
    })
}

fn parse(text: &str) -> Result<Vec<Frame>, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (opcode, hex) = line.split_once(' ').unwrap_or((line, ""));
            let opcode = opcode
                .parse::<u8>()
                .map_err(|_| format!("{line}: no opcode"))?;
            let hex = hex.trim();
            if hex.len() % 2 != 0 {
                return Err(format!("{line}: odd number of hex digits"));
            }
            let payload = (0..hex.len())
                .step_by(2)
                .map(|at| u8::from_str_radix(&hex[at..at + 2], 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| format!("{line}: not hex"))?;
            Ok(Frame { opcode, payload })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_opcodes_and_hex_payloads() {
        let frames = parse("225 e0c4b000\n\n 93 \n").unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].opcode, 225);
        assert_eq!(frames[0].payload, [0xe0, 0xc4, 0xb0, 0x00]);
        assert_eq!(frames[1].opcode, 93);
        assert!(frames[1].payload.is_empty());
        assert!(parse("225 e0c").is_err());
        assert!(parse("x 00").is_err());
    }
}
