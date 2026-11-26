//! Individual special handlers from the pinned 2009scape NPC combat path.
// Shared by several examples; each uses part of it.
#![allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Effect {
    None,
    DrainByHit,
    DrainThirtyPercent,
}

#[derive(Clone, Copy)]
pub(super) struct Special {
    pub cost: i32,
    pub animation: i32,
    pub graphic: i32,
    pub graphic_height: i32,
    pub graphic_delay: i32,
    pub graphic_on_target: bool,
    pub sound: i32,
    pub accuracy: f64,
    pub defence: f64,
    pub damage: f64,
    pub hits: i32,
    pub instant: bool,
    pub effect: Effect,
}

fn ordinary(cost: i32, animation: i32, graphic: i32, sound: i32) -> Special {
    Special {
        cost,
        animation,
        graphic,
        graphic_height: 96,
        graphic_delay: 0,
        graphic_on_target: false,
        sound,
        accuracy: 1.0,
        defence: 1.0,
        damage: 1.0,
        hits: 1,
        instant: false,
        effect: Effect::None,
    }
}

pub(super) fn weapon(id: i32) -> Option<Special> {
    Some(match id {
        // Puncture, Cleave, Energy Drain, Sever, Shatter, Impale and Quick Smash.
        1215 | 1231 | 5680 | 5698 | 13465..=13468 => Special {
            accuracy: 1.15,
            damage: 1.15,
            hits: 2,
            ..ordinary(25, 1062, 252, 2537)
        },
        1305 | 13475 => Special {
            damage: 1.25,
            ..ordinary(25, 1058, 248, 2529)
        },
        4151 => Special {
            accuracy: 1.25,
            graphic_on_target: true,
            ..ordinary(50, 1658, 341, 2713)
        },
        4587 | 13477 => Special {
            accuracy: 1.25,
            ..ordinary(55, 1872, 347, 2540)
        },
        1434 | 13479 => Special {
            accuracy: 1.25,
            damage: 1.5,
            ..ordinary(25, 1060, 251, 2541)
        },
        3101 => Special {
            accuracy: 1.10,
            defence: 0.98,
            damage: 1.1,
            ..ordinary(25, 923, 274, 2534)
        },
        4153 => Special {
            instant: true,
            ..ordinary(50, 1667, 340, 2715)
        },
        // Backstab, Smash and Snapshot.
        8872 | 8874 | 8876 | 8878 => Special {
            graphic_height: 0,
            effect: Effect::DrainByHit,
            ..ordinary(75, 4198, 704, 1084)
        },
        13902 | 13904 => Special {
            graphic_height: 0,
            graphic_delay: 16,
            effect: Effect::DrainThirtyPercent,
            ..ordinary(35, 10501, 1840, 2520)
        },
        861 => Special {
            accuracy: 1.43,
            hits: 2,
            ..ordinary(55, 1074, 249, 2545)
        },
        _ => return None,
    })
}
