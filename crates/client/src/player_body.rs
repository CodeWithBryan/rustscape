//! Original raw wearable/identity-kit human body assembly.
use crate::{
    animation::sin_cos,
    appearance::{self, IdentityKit},
    model::Model,
    pack::Pack,
    ui::item::{Catalog, ItemType},
};
use std::collections::BTreeMap;
pub type Transforms = [Option<[i32; 6]>; 12];

/// BasType opcode27, retaining the absent-versus-empty outer array distinction.
/// Other original fields are consumed here; movement semantics are independent.
pub fn bas_transforms(data: &[u8]) -> rs09_protocol::Result<Option<Transforms>> {
    use rs09_protocol::{Error, buffer::Reader};
    let mut r = Reader::new(data);
    let mut slots = None;
    loop {
        match r.g1()? {
            0 => return Ok(slots),
            1 => {
                r.take(4)?;
            }
            2..=9 | 30 | 32 | 33 | 35 | 36 | 38..=45 => {
                r.take(2)?;
            }
            26 => {
                r.take(2)?;
            }
            27 => {
                let slot = r.g1()? as usize;
                let entry = slots
                    .get_or_insert([None; 12])
                    .get_mut(slot)
                    .ok_or(Error::new(
                        r.position() - 1,
                        "BAS body slot outside original array",
                    ))?;
                let mut transform = [0; 6];
                for n in &mut transform {
                    *n = r.g2b()?;
                }
                *entry = Some(transform);
            }
            29 | 31 | 34 | 37 => {
                r.g1()?;
            }
            _ => return Err(Error::new(r.position() - 1, "Unknown BAS opcode")),
        }
    }
}
fn changes(m: &mut Model, recolor: &[[u16; 2]], retexture: &[[u16; 2]]) {
    for pair in recolor {
        m.recolor(&[*pair]);
    }
    for &[a, b] in retexture {
        for value in &mut m.textures {
            if *value as i16 == a as i16 {
                *value = b as i16 as i32;
            }
        }
    }
}
pub fn item_body(
    t: &ItemType,
    models: &BTreeMap<u32, Model>,
    female: bool,
) -> Result<Option<Model>, String> {
    let (ids, offset) = if female {
        (
            [t.womanwear, t.womanwear2, t.womanwear3],
            [t.womanWearXOff, t.womanWearYOff, t.womanWearZOff],
        )
    } else {
        (
            [t.manwear, t.manwear2, t.manwear3],
            [t.manWearXOff, t.manWearYOff, t.manWearZOff],
        )
    };
    if ids[0] == -1 {
        return Ok(None);
    }
    let get = |id: i32| {
        models
            .get(&(id as u32))
            .cloned()
            .ok_or_else(|| format!("Missing wearable model{id}"))
    };
    let mut model = get(ids[0])?;
    if ids[1] != -1 {
        let mut parts = vec![model, get(ids[1])?];
        if ids[2] != -1 {
            parts.push(get(ids[2])?);
        }
        model = Model::combine(&parts);
    }
    for point in &mut model.vertices {
        for axis in 0..3 {
            point[axis] = point[axis].wrapping_add(offset[axis]);
        }
    }
    let recolor: Vec<_> = t
        .recol_s
        .iter()
        .flatten()
        .zip(t.recol_d.iter().flatten())
        .map(|(&a, &b)| [a as u16, b as u16])
        .collect();
    let retexture: Vec<_> = t
        .retex_s
        .iter()
        .flatten()
        .zip(t.retex_d.iter().flatten())
        .map(|(&a, &b)| [a as u16, b as u16])
        .collect();
    changes(&mut model, &recolor, &retexture);
    Ok(Some(model))
}
pub fn kit_body(k: &IdentityKit, models: &BTreeMap<u32, Model>) -> Result<Option<Model>, String> {
    if !k.body_present {
        return Ok(None);
    }
    let parts = k
        .models
        .iter()
        .map(|id| {
            models
                .get(id)
                .cloned()
                .ok_or_else(|| format!("Missing kit model{id}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut m = if parts.len() == 1 {
        parts.into_iter().next().unwrap()
    } else {
        Model::combine(&parts)
    };
    changes(&mut m, &k.recolor, &k.retexture);
    Ok(Some(m))
}
pub fn rotate_translate(m: &mut Model, p: [i32; 6]) -> Result<(), String> {
    let [tx, ty, tz, rx, ry, rz] = p;
    if [rx, ry, rz].into_iter().any(|r| !(0..2048).contains(&r)) {
        return Err("BAS angle outside original trig table".into());
    }
    for point in &mut m.vertices {
        let [x, y, z] = *point;
        let (mut x, mut y, mut z) = (x, y, z);
        if rz != 0 {
            let (s, c) = sin_cos(rz);
            let a = y.wrapping_mul(s).wrapping_add(x.wrapping_mul(c)) >> 16;
            y = y.wrapping_mul(c).wrapping_sub(x.wrapping_mul(s)) >> 16;
            x = a;
        }
        if rx != 0 {
            let (s, c) = sin_cos(rx);
            let a = y.wrapping_mul(c).wrapping_sub(z.wrapping_mul(s)) >> 16;
            z = y.wrapping_mul(s).wrapping_add(z.wrapping_mul(c)) >> 16;
            y = a;
        }
        if ry != 0 {
            let (s, c) = sin_cos(ry);
            let a = z.wrapping_mul(s).wrapping_add(x.wrapping_mul(c)) >> 16;
            z = z.wrapping_mul(c).wrapping_sub(x.wrapping_mul(s)) >> 16;
            x = a;
        }
        *point = [x.wrapping_add(tx), y.wrapping_add(ty), z.wrapping_add(tz)];
    }
    m.surfaces.clear();
    m.draw_order.clear();
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub fn human_body(
    pack: &Pack,
    models: &BTreeMap<u32, Model>,
    items: &mut Catalog,
    mut slots: [i32; 12],
    colors: [i32; 5],
    female: bool,
    overrides: [i32; 2],
    transforms: [Option<[i32; 6]>; 12],
) -> Result<Model, String> {
    // The original SeqType fields target slots5 and3 in this order.
    for (slot, id) in [5, 3].into_iter().zip(overrides) {
        if id >= 0 {
            slots[slot] = if id == 65535 { 0 } else { id | 0x40000000 };
        }
    }
    let mut parts = Vec::new();
    for (i, value) in slots.into_iter().enumerate() {
        let part = if value & 0x40000000 != 0 {
            item_body(&items.get(pack, value & 0x3fffffff)?, models, female)?
        } else if value & i32::MIN != 0 {
            let kit = IdentityKit::decode(pack.get(&format!("kit/{}", value & 0x3fffffff))?)
                .map_err(|e| e.to_string())?;
            kit_body(&kit, models)?
        } else {
            None
        };
        if let Some(mut part) = part {
            if let Some(t) = transforms[i] {
                rotate_translate(&mut part, t)?;
            }
            parts.push(Some(part));
        } else {
            parts.push(None);
        }
    }
    let mut body = Model::combine_slots(&parts.iter().map(Option::as_ref).collect::<Vec<_>>());
    body.part_matrices = parts
        .iter()
        .zip(transforms)
        .map(|(part, t)| {
            if part.is_some() {
                t.map(crate::animation_parts::basis)
            } else {
                None
            }
        })
        .collect();
    appearance::recolor(&mut body, colors);
    body.lighting = [64, 850];
    body.sort_by_priority = true;
    Ok(body)
}
