//! Test-only observations of the portable player decoder.
use rs09_protocol::game::players::{Field, State, Update};
use serde_json::{Value, json};
pub fn bytes(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn hex(s: &[u8]) -> String {
    s.iter().map(|v| format!("{v:02x}")).collect()
}
fn field(f: &Field, cycle: i32) -> Value {
    match f {
        Field::PublicChat {
            color_effect,
            rights,
            payload,
        } => {
            json!({"type":"PublicChat","color_effect":color_effect,"rights":rights,"payload":hex(payload)})
        }
        Field::Hit { damage, kind, bar } => {
            json!({"type":"Hit","damage":damage,"kind":kind,"bar":bar})
        }
        Field::Animation { id, delay } => json!({"type":"Animation","id":id,"delay":delay}),
        Field::Appearance { bytes, .. } => json!({"type":"Appearance","bytes":hex(bytes)}),
        Field::FaceEntity(value) => json!({"type":"FaceEntity","value":value}),
        Field::ForcedMovement {
            start,
            destination,
            start_cycle,
            end_cycle,
            direction,
        } => {
            json!({"type":"ForcedMovement","start":start,"destination":destination,"start_cycle":start_cycle,"end_cycle":end_cycle,"direction":direction})
        }
        Field::OverheadText(bytes) => {
            let text = bytes
                .strip_prefix(b"~")
                .unwrap_or(bytes)
                .iter()
                .map(|&b| char::from(b))
                .collect::<String>();
            json!({"type":"OverheadText","text":text})
        }
        Field::SlotAnimations(values) => {
            json!({"type":"SlotAnimations","values":values.iter().map(|a|[a.id,a.delay,a.mask]).collect::<Vec<_>>()})
        }
        Field::SpotAnimation { id, packed } => {
            json!({"type":"SpotAnimation","id":id,"height":packed>>16,"start_cycle":cycle.wrapping_add(packed&65535)})
        }
        Field::FaceTile(tile) => json!({"type":"FaceTile","tile":tile}),
    }
}
pub fn snapshot(state: &State, update: &Update, cycle: i32) -> Value {
    let players=state.players.iter().enumerate().filter_map(|(index,p)|p.as_ref().map(|p|json!({"id":index,"x":p.queue_x,"y":p.queue_y,"speed":p.queue_speed,"size":p.queue_size,"seen":p.last_seen,"angles":[p.current_angle,p.target_angle],"appearance":p.appearance.as_ref().and_then(|_|state.appearance_cache[index].as_ref()).map(|b|hex(b))}))).collect::<Vec<_>>();
    json!({"ids":state.local_ids,"plane":state.plane,"players":players,"extended":update.extended.iter().map(|m|json!({"index":m.index,"flags":m.flags,"fields":m.fields.iter().map(|f|field(f,cycle)).collect::<Vec<_>>()})).collect::<Vec<_>>(),"removed_candidates":update.removed_candidates})
}
