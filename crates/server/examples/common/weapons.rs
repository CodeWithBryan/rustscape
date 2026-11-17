//! WeaponInterface ordinals, styles and ordinary attack properties.
use rs09_server::{
    definitions::property::Property,
    inventory::{container::Container, equipment, item::Handle},
    world::animation::{Animation, Graphics},
};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stance {
    Accurate,
    Aggressive,
    Controlled,
    Defensive,
    RangedAccurate,
    Rapid,
    Longrange,
    DefensiveCast,
    Cast,
}

#[derive(Clone, Copy)]
pub(super) struct Style {
    pub stance: Stance,
    pub bonus: usize,
}

impl Style {
    const fn new(stance: Stance, bonus: usize) -> Self {
        Self { stance, bonus }
    }

    pub fn attack_level(self) -> i32 {
        match self.stance {
            Stance::Accurate | Stance::RangedAccurate => 3,
            Stance::Controlled => 1,
            _ => 0,
        }
    }

    pub fn strength_level(self) -> i32 {
        match self.stance {
            Stance::Aggressive | Stance::RangedAccurate => 3,
            Stance::Controlled => 1,
            _ => 0,
        }
    }

    pub fn defence_level(self) -> i32 {
        match self.stance {
            Stance::Defensive | Stance::Longrange => 3,
            Stance::Controlled => 1,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Interface {
    pub id: i32,
    pub styles: &'static [Style],
    buttons: &'static [i32],
}

impl Interface {
    const fn new(id: i32, styles: &'static [Style], buttons: &'static [i32]) -> Self {
        Self {
            id,
            styles,
            buttons,
        }
    }

    pub fn slot(self, button: i32) -> Option<usize> {
        self.buttons.iter().position(|&id| id == button)
    }

    pub fn retain_stance(self, slot: usize, previous: Stance) -> usize {
        let slot = slot.min(self.styles.len() - 1);
        if self.styles[slot].stance == previous {
            slot
        } else {
            self.styles
                .iter()
                .rposition(|s| s.stance == previous)
                .unwrap_or(slot)
        }
    }
}

use Stance::*;
const AC: Style = Style::new(Accurate, 2);
const GC: Style = Style::new(Aggressive, 2);
const DC: Style = Style::new(Defensive, 2);
const AS: Style = Style::new(Accurate, 1);
const GS: Style = Style::new(Aggressive, 1);
const DS: Style = Style::new(Defensive, 1);
const CT: Style = Style::new(Controlled, 0);
const AT: Style = Style::new(Accurate, 0);
const GT: Style = Style::new(Aggressive, 0);
const DT: Style = Style::new(Defensive, 0);
const RA: Style = Style::new(RangedAccurate, 4);
const RANGE: &[Style] = &[RA, Style::new(Rapid, 4), Style::new(Longrange, 4)];
const THREE: &[i32] = &[2, 3, 4];
const FOUR: &[i32] = &[2, 3, 4, 5];
const REORDERED: &[i32] = &[2, 4, 3];
const AXE: &[Style] = &[AS, GS, GC, DS];

// Indexed by the pinned WeaponInterfaces enum ordinal in item_configs.json.
pub(super) const INTERFACES: [Interface; 24] = [
    Interface::new(92, &[AC, GC, DC], THREE),
    Interface::new(
        90,
        &[
            AC,
            GC,
            DC,
            Style::new(DefensiveCast, 3),
            Style::new(Cast, 3),
        ],
        &[1, 2, 3, 4, 5],
    ),
    Interface::new(75, AXE, &[2, 5, 4, 3]),
    Interface::new(85, &[AC, GC, DC], THREE),
    Interface::new(83, &[AT, GT, GC, DT], FOUR),
    Interface::new(89, &[AT, GT, GS, DT], FOUR),
    Interface::new(81, &[AS, GS, CT, DS], FOUR),
    Interface::new(82, AXE, FOUR),
    Interface::new(88, &[AC, GC, CT, DC], FOUR),
    Interface::new(78, &[AS, GS, CT, DS], &[2, 5, 4, 3]),
    Interface::new(76, &[AC, GC, DC], REORDERED),
    Interface::new(93, &[AS, Style::new(Controlled, 1), DS], THREE),
    Interface::new(76, &[AC, GC, DC], THREE),
    Interface::new(91, RANGE, THREE),
    Interface::new(
        87,
        &[CT, Style::new(Controlled, 1), Style::new(Controlled, 2), DT],
        FOUR,
    ),
    Interface::new(84, &[CT, GS, DT], THREE),
    Interface::new(77, RANGE, REORDERED),
    Interface::new(79, RANGE, REORDERED),
    Interface::new(91, RANGE, THREE),
    Interface::new(473, RANGE, REORDERED),
    Interface::new(80, &[RA, GC], &[2, 3]),
    Interface::new(474, &[GS, RA, Style::new(DefensiveCast, 3)], THREE),
    Interface::new(86, &[AS, GT, GC, DS], FOUR),
    Interface::new(85, &[AC, GC, DC], THREE),
];

pub(super) fn property(item: &Handle, key: &str) -> Option<Property> {
    item.read()
        .definition
        .as_ref()?
        .read()
        .handlers
        .get(key)
        .cloned()
}

pub(super) fn number(item: &Handle, key: &str, default: i32) -> i32 {
    match property(item, key) {
        Some(Property::Int(value)) => value,
        _ => default,
    }
}

pub(super) fn ordinal(item: Option<&Handle>) -> usize {
    item.map_or(0, |i| {
        let ordinal = number(i, "weapon_interface", 0) as usize;
        if ordinal < INTERFACES.len() {
            ordinal
        } else {
            0
        }
    })
}

pub(super) fn defence_animation(equipment: &Container) -> Animation {
    for (slot, fallback) in [(equipment::SHIELD, 1156), (equipment::WEAPON, 424)] {
        if let Some(item) = equipment.get(slot) {
            return match property(&item, "defence_anim") {
                Some(Property::Animation(animation)) => Animation::from_shared(animation),
                _ => Animation::new(fallback),
            };
        }
    }
    Animation::new(397)
}

pub(super) fn attack_animation(item: Option<&Handle>, slot: usize) -> Animation {
    if let Some(Property::Animations(animations)) = item.and_then(|i| property(i, "attack_anims"))
        && let Some(Some(animation)) = animations.read().get(slot)
    {
        return Animation::from_shared(animation.clone());
    }
    use rs09_server::world::animation::Priority;
    Animation::with([422, 423, 422, 422][slot.min(3)], 0, Some(Priority::High))
}

pub(super) fn bonuses(equipment: &Container) -> [i32; 15] {
    let mut totals = [0; 15];
    for item in equipment.items().read().iter().flatten() {
        if let Some(Property::IntArray(bonuses)) = property(item, "bonuses") {
            for (total, value) in totals.iter_mut().zip(bonuses.read().iter()) {
                *total += value;
            }
        }
    }
    totals
}

pub(super) struct RangedWeapon {
    pub ammo_slot: i32,
    pub animation: i32,
    pub double_shot: bool,
    pub degrading: bool,
    pub drop_ammo: bool,
    pub ammunition: Vec<i32>,
}

struct RangedCatalog {
    weapons: BTreeMap<i32, RangedWeapon>,
    graphics: BTreeMap<i32, (i32, i32)>,
    projectiles: BTreeMap<i32, Missile>,
}

#[derive(Clone, Copy)]
pub(super) struct Missile {
    pub spot: i32,
    pub heights: [i32; 2],
    pub start: i32,
    pub speed: i32,
    pub angle: i32,
    pub distance: i32,
}

fn ranged_catalog() -> &'static RangedCatalog {
    #[derive(serde::Deserialize)]
    struct WeaponRow {
        #[serde(rename = "itemId")]
        item_id: String,
        ammo_slot: String,
        animation: String,
        weapon_type: String,
        ammunition: String,
        drop_ammo: String,
    }
    #[derive(serde::Deserialize)]
    struct AmmoRow {
        #[serde(rename = "itemId")]
        item_id: String,
        start_graphic: String,
        projectile: String,
    }
    static CATALOG: OnceLock<RangedCatalog> = OnceLock::new();
    // Unmodified Server/data/configs assets from the revision in upstream.lock.json.
    let integer = |value: &str| value.parse::<i32>().expect("pinned ranged integer");
    CATALOG.get_or_init(|| {
        let weapons: Vec<WeaponRow> = serde_json::from_str(include_str!(
            "../../../../fixtures/ranged-combat/ranged_weapon_configs.json"
        ))
        .expect("pinned ranged weapons");
        let ammo: Vec<AmmoRow> = serde_json::from_str(include_str!(
            "../../../../fixtures/ranged-combat/ammo_configs.json"
        ))
        .expect("pinned ammunition");
        let mut catalog = BTreeMap::new();
        for row in weapons {
            catalog
                .entry(integer(&row.item_id))
                .or_insert_with(|| RangedWeapon {
                    ammo_slot: integer(&row.ammo_slot),
                    animation: integer(&row.animation),
                    double_shot: row.weapon_type == "2",
                    degrading: row.weapon_type == "4",
                    drop_ammo: row.drop_ammo == "true",
                    ammunition: row.ammunition.split(',').map(integer).collect(),
                });
        }
        let mut graphics = BTreeMap::new();
        let mut projectiles = BTreeMap::new();
        for row in ammo {
            let id = integer(&row.item_id);
            let (graphic, height) = row
                .start_graphic
                .split_once(',')
                .expect("pinned ammunition graphic");
            graphics.insert(id, (integer(graphic), integer(height)));
            let values: Vec<_> = row.projectile.split(',').map(integer).collect();
            let &[
                spot,
                start_height,
                end_height,
                start,
                speed,
                angle,
                distance,
            ] = values.as_slice()
            else {
                panic!("pinned ammunition projectile")
            };
            if spot >= 0 {
                projectiles.insert(
                    id,
                    Missile {
                        spot,
                        heights: [start_height, end_height],
                        start,
                        speed,
                        angle,
                        distance,
                    },
                );
            }
        }
        RangedCatalog {
            weapons: catalog,
            graphics,
            projectiles,
        }
    })
}

pub(super) fn ranged_weapon(item: Option<&Handle>) -> Option<&'static RangedWeapon> {
    ranged_catalog().weapons.get(&item?.read().id())
}

pub(super) fn ammo_graphic(item: i32) -> Option<Graphics> {
    let &(id, height) = ranged_catalog().graphics.get(&item)?;
    (id >= 0).then(|| Graphics::new(id, height, 0))
}

pub(super) fn ammo_missile(item: i32) -> Option<Missile> {
    ranged_catalog().projectiles.get(&item).copied()
}
