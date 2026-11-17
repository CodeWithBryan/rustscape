//! Original ordinary rectangle/text widget selection and byte-string formatting.
use crate::ui::Component;

pub struct Item<'a> {
    pub name: Option<&'a str>,
    pub stackable: i32,
}
#[derive(Clone, Copy)]
pub struct Dns<'a> {
    pub ip: i32,
    pub result: Option<&'a str>,
}
pub struct Text {
    pub content: String,
    pub color: i32,
}
pub fn color(c: &Component, active: bool, hovered: bool) -> i32 {
    let (base, over) = if active {
        (c.active_color, c.active_over_color)
    } else {
        (c.color, c.over_color)
    };
    if hovered && over != 0 { over } else { base }
}
/// The original function groups the signed decimal string before applying
/// length thresholds; negative values and the leading color-prefix space stay.
pub fn item_count(count: i32) -> String {
    let mut grouped = count.to_string();
    let mut i = grouped.len() as i32 - 3;
    while i > 0 {
        grouped.insert(i as usize, ',');
        i -= 3;
    }
    if grouped.len() > 9 {
        format!(
            " <col=00ff80>{}M ({grouped})</col>",
            &grouped[..grouped.len() - 8]
        )
    } else if grouped.len() > 6 {
        format!(
            " <col=ffffff>{}K ({grouped})</col>",
            &grouped[..grouped.len() - 4]
        )
    } else {
        format!(" <col=ffff00>{grouped}</col>")
    }
}
pub fn interpolate(
    text: &str,
    mut expression: impl FnMut(usize) -> i32,
    dns: Option<Dns<'_>>,
) -> Result<String, String> {
    if !text.contains('%') {
        return Ok(text.to_owned());
    }
    let mut text = text.to_owned();
    let mut work = 0;
    for index in 0..6 {
        let token = if index == 5 {
            "%dns".to_owned()
        } else {
            format!("%{}", index + 1)
        };
        while let Some(at) = text.find(&token) {
            work += 1;
            if work > 262144 || text.len() > 4 * 1024 * 1024 {
                return Err("Widget text interpolation limit exceeded".into());
            }
            let replacement = if index < 5 {
                let value = expression(index);
                if value >= 999999999 {
                    "*".to_owned()
                } else {
                    value.to_string()
                }
            } else if let Some(dns) = dns {
                if let Some(result) = dns.result {
                    // String.getBytes(ISO_8859_1), then JagString.decodeString:
                    // replace unmappable characters, discard zero bytes.
                    result
                        .chars()
                        .filter(|&c| c != '\0')
                        .map(|c| if u32::from(c) <= 255 { c } else { '?' })
                        .collect()
                } else {
                    let [a, b, c, d] = dns.ip.to_be_bytes();
                    format!("{a}.{b}.{c}.{d}")
                }
            } else {
                String::new()
            };
            text.replace_range(at..at + token.len(), &replacement);
        }
    }
    Ok(text)
}
/// Font readiness, CS1 truth, item lookup, waiting/hover object identities and
/// language are caller services. Text glyphs use the original opaque font call,
/// independent of component alpha; markup can still change font transparency.
#[allow(clippy::too_many_arguments)]
pub fn text(
    c: &Component,
    active: bool,
    hovered: bool,
    waiting: bool,
    item: Option<Item<'_>>,
    expression: impl FnMut(usize) -> i32,
    dns: Option<Dns<'_>>,
    please_wait: &str,
) -> Result<Text, String> {
    let mut color = color(c, active, hovered);
    let mut content = if active && !c.active_text.is_empty() {
        &c.active_text
    } else {
        &c.text
    }
    .to_owned();
    if c.if3 && c.obj_id != -1 {
        let item = item.ok_or("Missing widget text item metadata")?;
        content = item.name.unwrap_or("null").to_owned();
        if (item.stackable == 1 || c.obj_count != 1) && c.obj_count != -1 {
            content = format!("<col=ff9040>{content}</col> x{}", item_count(c.obj_count));
        }
    }
    if waiting {
        color = c.color;
        content = please_wait.to_owned();
    }
    if !c.if3 {
        content = interpolate(&content, expression, dns)?;
    }
    Ok(Text { content, color })
}
