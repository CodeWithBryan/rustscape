use super::{AttributeValue, Error, GameAttributes};
use rs09_game_core::java_string::{parse_i32, parse_i64};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyLoad {
    NotUsed,
    Missing,
    Loaded,
    Failed(Error),
}
impl GameAttributes {
    pub fn parse_legacy_file(&mut self, prefix: &str, file: &str) -> LegacyLoad {
        let path = std::path::PathBuf::from(format!("{prefix}{file}"));
        if !path.exists() {
            return LegacyLoad::Missing;
        }
        match std::fs::read(path) {
            Ok(bytes) => match decode(&bytes) {
                Ok(xml) => self.parse_legacy_xml(&xml),
                Err(error) => LegacyLoad::Failed(error),
            },
            Err(_) => LegacyLoad::Failed(Error::Io),
        }
    }
    pub fn parse_legacy_xml(&mut self, xml: &str) -> LegacyLoad {
        let document = match roxmltree::Document::parse_with_options(
            xml,
            roxmltree::ParsingOptions {
                allow_dtd: true,
                ..Default::default()
            },
        ) {
            Ok(doc) => doc,
            Err(_) => return LegacyLoad::Failed(Error::Xml),
        };
        for node in document.descendants().filter(|node| node.is_element()) {
            let start = &xml[node.range().start..];
            let Some(tail) = start.strip_prefix("<GameAttribute") else {
                continue;
            };
            if !tail.starts_with(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>') {
                continue;
            }
            let key = node.attribute("key").unwrap_or("");
            let kind = node.attribute("type").unwrap_or("");
            let text = node.attribute("value").unwrap_or("");
            let value = match kind {
                "bool" => Some(Ok(AttributeValue::Bool(text.eq_ignore_ascii_case("true")))),
                "string" => Some(Ok(AttributeValue::Text(text.into()))),
                "int" => Some(
                    parse_i32(text)
                        .map(AttributeValue::Int)
                        .ok_or(Error::Number),
                ),
                "long" => Some(
                    parse_i64(text)
                        .map(AttributeValue::Long)
                        .ok_or(Error::Number),
                ),
                "short" => Some(
                    parse_i32(text)
                        .and_then(|v| i16::try_from(v).ok())
                        .map(AttributeValue::Short)
                        .ok_or(Error::Number),
                ),
                "byte" => Some(
                    parse_i32(text)
                        .and_then(|v| i8::try_from(v).ok())
                        .map(AttributeValue::Byte)
                        .ok_or(Error::Number),
                ),
                _ => None,
            };
            if let Some(value) = value {
                let value = match value {
                    Ok(value) => value,
                    Err(error) => return LegacyLoad::Failed(error),
                };
                let key = Some(key.to_owned());
                self.values
                    .insert(key.as_ref().unwrap().clone(), Some(value));
                if !self.saved.contains(&key) {
                    self.saved.push(key);
                }
            }
        }
        LegacyLoad::Loaded
    }
}
fn decode(bytes: &[u8]) -> Result<String, Error> {
    let (bytes, little) = if let Some(bytes) = bytes.strip_prefix(&[0xff, 0xfe]) {
        (bytes, Some(true))
    } else if let Some(bytes) = bytes.strip_prefix(&[0xfe, 0xff]) {
        (bytes, Some(false))
    } else {
        (
            bytes,
            if bytes.starts_with(b"<\0") {
                Some(true)
            } else if bytes.starts_with(b"\0<") {
                Some(false)
            } else {
                None
            },
        )
    };
    if let Some(little) = little {
        if bytes.len() % 2 != 0 {
            return Err(Error::Xml);
        }
        let words = bytes
            .chunks_exact(2)
            .map(|b| {
                if little {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect::<Vec<_>>();
        return String::from_utf16(&words).map_err(|_| Error::Xml);
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let encoding = if bytes.starts_with(b"<?xml") {
        let end = bytes
            .windows(2)
            .position(|value| value == b"?>")
            .ok_or(Error::Xml)?;
        let declaration = std::str::from_utf8(&bytes[..end]).map_err(|_| Error::Xml)?;
        declaration
            .find("encoding")
            .map(|index| {
                let remaining = declaration[index + 8..].trim_start();
                let remaining = remaining.strip_prefix('=').ok_or(Error::Xml)?.trim_start();
                let quote = remaining.chars().next().ok_or(Error::Xml)?;
                if quote != '\'' && quote != '"' {
                    return Err(Error::Xml);
                }
                remaining[1..]
                    .split(quote)
                    .next()
                    .filter(|value| !value.is_empty())
                    .ok_or(Error::Xml)
            })
            .transpose()?
    } else {
        None
    };
    if let Some(label) = encoding {
        if matches!(
            label.to_ascii_lowercase().as_str(),
            "iso-8859-1" | "iso_8859-1" | "iso_8859-1:1987" | "latin1" | "l1" | "ibm819" | "cp819"
        ) {
            return Ok(bytes.iter().map(|&byte| char::from(byte)).collect());
        }
        let encoding = encoding_rs::Encoding::for_label(label.as_bytes()).ok_or(Error::Xml)?;
        return encoding
            .decode_without_bom_handling_and_without_replacement(bytes)
            .map(|text| text.into_owned())
            .ok_or(Error::Xml);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| Error::Xml)
}
