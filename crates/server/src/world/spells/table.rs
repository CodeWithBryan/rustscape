//! The combat spells the standard and ancient books register: the
//! constructor arguments of each `register(button, spell)` call in the
//! seventeen combat spell classes, as data. The spells corpus compares every
//! field with the objects the unchanged classes register.
//!
//! Not here: the non-combat spells (teleports, alchemy, enchanting, charge,
//! telekinetic grab, bounty locate, the whole lunar book). They are content
//! and stay required services; a registry built from this table holds no
//! entry at their buttons.
use super::{
    Animation, Audio, DEFAULT_AUDIO_RADIUS, Graphics, Projectile, Spell, SpellBook, SpellClass,
    SpellType, graphics,
    runes::Rune::{self, *},
};

fn projectile(label: &str, id: i32) -> Option<Projectile> {
    Some(Projectile {
        label: label.to_owned(),
        projectile_id: id,
        start_height: 40,
        end_height: 36,
        start_delay: 52,
        speed: 75,
        angle: 15,
        distance: 11,
    })
}

fn g(id: i32, height: i32) -> Option<Graphics> {
    Some(graphics(id, height, 0))
}

/// `CombatSpell(type, book, level, baseExperience, castAudio, impactAudio,
/// animation, startGraphic, projectile, endGraphic, runes...)`.
#[allow(clippy::too_many_arguments)]
fn spell(
    class: SpellClass,
    book: SpellBook,
    kind: SpellType,
    level: i32,
    experience: f64,
    cast_audio: i32,
    impact_audio: i32,
    animation: Option<Animation>,
    graphic: Option<Graphics>,
    projectile: Option<Projectile>,
    end_graphic: Option<Graphics>,
    runes: &[(Rune, i32)],
) -> Spell {
    Spell {
        class,
        book,
        level,
        experience,
        animation,
        graphic,
        audio: Audio {
            id: cast_audio,
            delay: 1,
            loops: 0,
            radius: DEFAULT_AUDIO_RADIUS,
        },
        runes: Some(runes.iter().map(|(r, n)| (r.id(), *n)).collect()),
        spell_id: 0,
        kind,
        projectile,
        end_graphic,
        impact_audio,
        default_projectile: None,
    }
}

/// One elemental spell row: button, experience, cast sound, level, the start,
/// projectile and end graphic ids, and up to three runes (amount 0 pads).
type Elemental = (i32, f64, i32, i32, [i32; 3], [(Rune, i32); 3]);

/// Every combat spell registration, in the order the classes register them:
/// `(book, button, spell)` with the spell id already set to the button.
pub fn combat_spells() -> Vec<(SpellBook, i32, Spell)> {
    use SpellBook::{Ancient as A, Modern as M};
    use SpellClass as C;
    use SpellType as T;
    let cast = Some(Animation::high(711));
    let mut out = Vec::new();
    let mut add = |book: SpellBook, button: i32, mut s: Spell| {
        s.spell_id = button;
        out.push((book, button, s));
    };
    // AirSpell, WaterSpell, EarthSpell and FireSpell: sound + 1 is the impact audio.
    let elemental: [(SpellClass, &str, [Elemental; 4]); 4] = [
        (
            C::Air,
            "AirSpell",
            [
                (
                    1,
                    5.5,
                    220,
                    1,
                    [90, 91, 92],
                    [(Mind, 1), (Air, 1), (Air, 0)],
                ),
                (
                    10,
                    13.5,
                    218,
                    17,
                    [117, 118, 119],
                    [(Chaos, 1), (Air, 2), (Air, 0)],
                ),
                (
                    24,
                    25.5,
                    216,
                    41,
                    [132, 133, 134],
                    [(Death, 1), (Air, 3), (Air, 0)],
                ),
                (
                    45,
                    36.0,
                    222,
                    62,
                    [158, 159, 160],
                    [(Blood, 1), (Air, 5), (Air, 0)],
                ),
            ],
        ),
        (
            C::Water,
            "WaterSpell",
            [
                (
                    4,
                    7.5,
                    211,
                    5,
                    [93, 94, 95],
                    [(Mind, 1), (Water, 1), (Air, 1)],
                ),
                (
                    14,
                    16.5,
                    209,
                    23,
                    [120, 121, 122],
                    [(Chaos, 1), (Water, 2), (Air, 2)],
                ),
                (
                    27,
                    28.5,
                    207,
                    47,
                    [135, 136, 137],
                    [(Death, 1), (Water, 3), (Air, 3)],
                ),
                (
                    48,
                    37.5,
                    213,
                    65,
                    [161, 162, 163],
                    [(Blood, 1), (Water, 7), (Air, 5)],
                ),
            ],
        ),
        (
            C::Earth,
            "EarthSpell",
            [
                (
                    6,
                    9.5,
                    132,
                    9,
                    [96, 97, 98],
                    [(Mind, 1), (Earth, 2), (Air, 1)],
                ),
                (
                    17,
                    19.5,
                    130,
                    29,
                    [123, 124, 125],
                    [(Chaos, 1), (Earth, 3), (Air, 2)],
                ),
                (
                    33,
                    31.5,
                    128,
                    53,
                    [138, 139, 140],
                    [(Death, 1), (Earth, 4), (Air, 3)],
                ),
                (
                    52,
                    40.0,
                    134,
                    70,
                    [164, 165, 166],
                    [(Blood, 1), (Earth, 7), (Air, 5)],
                ),
            ],
        ),
        (
            C::Fire,
            "FireSpell",
            [
                (
                    8,
                    11.5,
                    160,
                    13,
                    [99, 100, 101],
                    [(Mind, 1), (Fire, 3), (Air, 2)],
                ),
                (
                    20,
                    22.5,
                    157,
                    35,
                    [126, 127, 128],
                    [(Chaos, 1), (Fire, 4), (Air, 3)],
                ),
                (
                    38,
                    34.5,
                    155,
                    59,
                    [129, 130, 131],
                    [(Death, 1), (Fire, 5), (Air, 4)],
                ),
                (
                    55,
                    42.5,
                    162,
                    75,
                    [155, 156, 157],
                    [(Blood, 1), (Fire, 7), (Air, 5)],
                ),
            ],
        ),
    ];
    let names = ["STRIKE", "BOLT", "BLAST", "WAVE"];
    let kinds = [T::Strike, T::Bolt, T::Blast, T::Wave];
    for (class, prefix, rows) in elemental {
        for (k, (button, xp, sound, level, gfx, runes)) in rows.into_iter().enumerate() {
            // Air spells name two runes; the third slot is padding.
            let runes: Vec<(Rune, i32)> = runes.into_iter().filter(|(_, n)| *n > 0).collect();
            let label = format!("{prefix}.{}_PROJECTILE", names[k]);
            add(
                M,
                button,
                spell(
                    class,
                    M,
                    kinds[k],
                    level,
                    xp,
                    sound,
                    sound + 1,
                    cast,
                    g(gfx[0], 96),
                    projectile(&label, gfx[1]),
                    g(gfx[2], 96),
                    &runes,
                ),
            );
        }
    }
    // BindSpell.
    let bind = Some(Animation::high(710));
    add(
        M,
        12,
        spell(
            C::Bind,
            M,
            T::Bind,
            20,
            30.0,
            101,
            99,
            bind,
            g(177, 96),
            projectile("BindSpell.BIND_PROJECTILE", 178),
            g(181, 96),
            &[(Nature, 2), (Earth, 3), (Water, 3)],
        ),
    );
    add(
        M,
        30,
        spell(
            C::Bind,
            M,
            T::Snare,
            50,
            60.0,
            3003,
            3002,
            bind,
            g(177, 96),
            projectile("BindSpell.SNARE_PROJECTILE", 178),
            g(180, 96),
            &[(Nature, 3), (Earth, 4), (Water, 4)],
        ),
    );
    add(
        M,
        56,
        spell(
            C::Bind,
            M,
            T::Entangle,
            79,
            89.0,
            151,
            153,
            bind,
            g(177, 96),
            projectile("BindSpell.ENTANGLE_PROJECTILE", 178),
            g(179, 96),
            &[(Nature, 4), (Earth, 5), (Water, 5)],
        ),
    );
    // CurseSpells: types up to CURSE use the low animation.
    let low = Some(Animation::high(716));
    let high = Some(Animation::high(729));
    add(
        M,
        2,
        spell(
            C::Curse,
            M,
            T::Confuse,
            3,
            13.0,
            119,
            121,
            low,
            g(102, 96),
            projectile("CurseSpells.CONFUSE_PROJECTILE", 103),
            g(104, 96),
            &[(Body, 1), (Earth, 2), (Water, 3)],
        ),
    );
    add(
        M,
        7,
        spell(
            C::Curse,
            M,
            T::Weaken,
            11,
            21.0,
            3011,
            3010,
            low,
            g(105, 96),
            projectile("CurseSpells.WEAKEN_PROJECTILE", 106),
            g(107, 96),
            &[(Body, 1), (Earth, 2), (Water, 3)],
        ),
    );
    add(
        M,
        11,
        spell(
            C::Curse,
            M,
            T::Curse,
            19,
            29.0,
            127,
            126,
            low,
            g(108, 96),
            projectile("CurseSpells.CURSE_PROJECTILE", 109),
            g(110, 96),
            &[(Body, 1), (Earth, 3), (Water, 2)],
        ),
    );
    add(
        M,
        50,
        spell(
            C::Curse,
            M,
            T::Vulnerability,
            66,
            76.0,
            3009,
            3008,
            high,
            g(167, 96),
            projectile("CurseSpells.VULNER_PROJECTILE", 168),
            g(169, 96),
            &[(Soul, 1), (Earth, 5), (Water, 5)],
        ),
    );
    add(
        M,
        53,
        spell(
            C::Curse,
            M,
            T::Enfeeble,
            73,
            83.0,
            148,
            150,
            high,
            g(170, 96),
            projectile("CurseSpells.ENFEEBLE_PROJECTILE", 171),
            g(172, 96),
            &[(Soul, 1), (Earth, 8), (Water, 8)],
        ),
    );
    add(
        M,
        57,
        spell(
            C::Curse,
            M,
            T::Stun,
            80,
            90.0,
            3004,
            3005,
            high,
            g(173, 96),
            projectile("CurseSpells.STUN_PROJECTILE", 174),
            g(107, 96),
            &[(Soul, 1), (Earth, 12), (Water, 12)],
        ),
    );
    // GodSpells: level 60 and 35 experience for all three, no start graphic or projectile.
    let god = Some(Animation::high(811));
    add(
        M,
        41,
        spell(
            C::God,
            M,
            T::GodStrike,
            60,
            35.0,
            -1,
            1659,
            god,
            None,
            None,
            g(76, 0),
            &[(Blood, 2), (Fire, 2), (Air, 4)],
        ),
    );
    add(
        M,
        42,
        spell(
            C::God,
            M,
            T::GodStrike,
            60,
            35.0,
            -1,
            1653,
            god,
            None,
            None,
            g(77, 0),
            &[(Blood, 2), (Fire, 1), (Air, 4)],
        ),
    );
    add(
        M,
        43,
        spell(
            C::God,
            M,
            T::GodStrike,
            60,
            35.0,
            -1,
            1655,
            god,
            None,
            None,
            g(78, 0),
            &[(Blood, 2), (Fire, 4), (Air, 1)],
        ),
    );
    add(
        M,
        22,
        spell(
            C::CrumbleUndead,
            M,
            T::CrumbleUndead,
            39,
            24.5,
            122,
            124,
            Some(Animation::high(724)),
            g(145, 96),
            projectile("CrumbleUndead.new", 146),
            g(147, 96),
            &[(Earth, 2), (Air, 2), (Chaos, 1)],
        ),
    );
    add(
        M,
        29,
        spell(
            C::IbanBlast,
            M,
            T::IbansBlast,
            50,
            60.5,
            162,
            163,
            Some(Animation::high(708)),
            g(87, 96),
            projectile("IbanBlast.new", 88),
            g(89, 96),
            &[(Fire, 5), (Death, 1)],
        ),
    );
    add(
        M,
        31,
        spell(
            C::MagicDart,
            M,
            T::MagicDart,
            50,
            30.0,
            218,
            219,
            Some(Animation::high(1576)),
            None,
            projectile("MagicDart.new", 330),
            g(331, 96),
            &[(Death, 1), (Mind, 4)],
        ),
    );
    add(
        M,
        63,
        spell(
            C::Teleblock,
            M,
            T::Teleblock,
            85,
            80.0,
            202,
            203,
            Some(Animation::high(10503)),
            g(1841, 0),
            projectile("TeleblockSpell.TELEBLOCK_ORB", 1842),
            g(1843, 0),
            &[(Law, 1), (Death, 1), (Chaos, 1)],
        ),
    );
    let mut sonic = spell(
        C::SonicWave,
        M,
        T::Strike,
        1,
        0.0,
        -1,
        0,
        None,
        g(337, 0),
        projectile("SonicWaveSpell.STRIKE_PROJECTILE", 337),
        None,
        &[],
    );
    if let Some(p) = sonic.projectile.as_mut() {
        (p.start_height, p.end_height, p.speed, p.distance) = (8, 8, 100, 1);
    }
    add(M, 63232, sonic);
    // Ancient books: rush and blitz use animation 1978, burst and barrage 1979.
    let one = Some(Animation::high(1978));
    let two = Some(Animation::high(1979));
    add(
        A,
        0,
        spell(
            C::Ice,
            A,
            T::Rush,
            58,
            34.0,
            171,
            173,
            one,
            None,
            projectile("IceSpells.RUSH_PROJECTILE", 360),
            g(361, 96),
            &[(Death, 2), (Chaos, 2), (Water, 2)],
        ),
    );
    add(
        A,
        2,
        spell(
            C::Ice,
            A,
            T::Burst,
            70,
            40.0,
            171,
            170,
            two,
            None,
            projectile("IceSpells.BURST_PROJECTILE", 362),
            g(363, 0),
            &[(Death, 2), (Chaos, 4), (Water, 4)],
        ),
    );
    add(
        A,
        1,
        spell(
            C::Ice,
            A,
            T::Blitz,
            82,
            46.0,
            171,
            169,
            one,
            g(366, 96),
            None,
            g(367, 96),
            &[(Blood, 2), (Death, 2), (Water, 3)],
        ),
    );
    add(
        A,
        3,
        spell(
            C::Ice,
            A,
            T::Barrage,
            94,
            52.0,
            171,
            168,
            two,
            None,
            projectile("IceSpells.BARRAGE_PROJECTILE", 368),
            g(369, 0),
            &[(Blood, 2), (Death, 4), (Water, 6)],
        ),
    );
    add(
        A,
        4,
        spell(
            C::Blood,
            A,
            T::Rush,
            56,
            33.0,
            108,
            110,
            one,
            None,
            projectile("BloodSpells.RUSH_PROJECTILE", 372),
            g(373, 96),
            &[(Blood, 1), (Death, 2), (Chaos, 2)],
        ),
    );
    add(
        A,
        6,
        spell(
            C::Blood,
            A,
            T::Burst,
            68,
            39.0,
            106,
            105,
            two,
            None,
            None,
            g(376, 0),
            &[(Blood, 2), (Death, 2), (Chaos, 4)],
        ),
    );
    add(
        A,
        5,
        spell(
            C::Blood,
            A,
            T::Blitz,
            80,
            45.0,
            106,
            104,
            one,
            None,
            projectile("BloodSpells.BLITZ_PROJECTILE", 374),
            g(375, 96),
            &[(Blood, 4), (Death, 2)],
        ),
    );
    add(
        A,
        7,
        spell(
            C::Blood,
            A,
            T::Barrage,
            92,
            51.0,
            106,
            102,
            two,
            None,
            None,
            g(377, 0),
            &[(Soul, 1), (Blood, 4), (Death, 4)],
        ),
    );
    add(
        A,
        12,
        spell(
            C::Shadow,
            A,
            T::Rush,
            52,
            31.0,
            178,
            179,
            one,
            None,
            projectile("ShadowSpells.RUSH_PROJECTILE", 378),
            g(379, 96),
            &[(Soul, 1), (Death, 2), (Chaos, 2), (Air, 1)],
        ),
    );
    add(
        A,
        14,
        spell(
            C::Shadow,
            A,
            T::Burst,
            64,
            37.0,
            178,
            177,
            two,
            None,
            projectile("ShadowSpells.BURST_PROJECTILE", 380),
            g(381, 0),
            &[(Soul, 2), (Death, 2), (Chaos, 4), (Air, 1)],
        ),
    );
    add(
        A,
        13,
        spell(
            C::Shadow,
            A,
            T::Blitz,
            76,
            43.0,
            178,
            176,
            one,
            None,
            None,
            g(382, 96),
            &[(Soul, 2), (Blood, 2), (Death, 2), (Air, 2)],
        ),
    );
    add(
        A,
        15,
        spell(
            C::Shadow,
            A,
            T::Barrage,
            88,
            48.0,
            178,
            175,
            two,
            None,
            None,
            g(383, 0),
            &[(Soul, 3), (Blood, 2), (Death, 4), (Air, 4)],
        ),
    );
    add(
        A,
        8,
        spell(
            C::Smoke,
            A,
            T::Rush,
            50,
            30.0,
            183,
            185,
            one,
            None,
            projectile("SmokeSpells.RUSH_PROJECTILE", 384),
            g(385, 96),
            &[(Death, 2), (Chaos, 2), (Fire, 1), (Air, 1)],
        ),
    );
    add(
        A,
        10,
        spell(
            C::Smoke,
            A,
            T::Burst,
            62,
            36.0,
            183,
            182,
            two,
            None,
            projectile("SmokeSpells.BURST_PROJECTILE", 386),
            g(387, 0),
            &[(Death, 2), (Chaos, 4), (Fire, 2), (Air, 2)],
        ),
    );
    add(
        A,
        9,
        spell(
            C::Smoke,
            A,
            T::Blitz,
            74,
            42.0,
            183,
            181,
            one,
            None,
            projectile("SmokeSpells.BLITZ_PROJECTILE", 389),
            g(388, 96),
            &[(Blood, 2), (Death, 2), (Fire, 2), (Air, 2)],
        ),
    );
    add(
        A,
        11,
        spell(
            C::Smoke,
            A,
            T::Barrage,
            86,
            48.0,
            183,
            180,
            two,
            None,
            projectile("SmokeSpells.BARRAGE_PROJECTILE", 391),
            g(390, 0),
            &[(Blood, 2), (Death, 4), (Fire, 4), (Air, 4)],
        ),
    );
    add(
        A,
        16,
        spell(
            C::Miasmic,
            A,
            T::Rush,
            61,
            36.0,
            5368,
            5365,
            Some(Animation::high(10513)),
            Some(graphics(1845, 0, 15)),
            None,
            g(1847, 40),
            &[(Earth, 1), (Soul, 1), (Chaos, 2)],
        ),
    );
    add(
        A,
        18,
        spell(
            C::Miasmic,
            A,
            T::Burst,
            73,
            42.0,
            5366,
            5372,
            Some(Animation::high(10516)),
            g(1848, 0),
            None,
            Some(graphics(1849, 20, 30)),
            &[(Earth, 2), (Soul, 2), (Chaos, 4)],
        ),
    );
    add(
        A,
        17,
        spell(
            C::Miasmic,
            A,
            T::Blitz,
            85,
            48.0,
            5370,
            5367,
            Some(Animation::high(10524)),
            g(1850, 15),
            None,
            g(1851, 0),
            &[(Earth, 3), (Soul, 3), (Blood, 2)],
        ),
    );
    add(
        A,
        19,
        spell(
            C::Miasmic,
            A,
            T::Barrage,
            97,
            54.0,
            5371,
            5369,
            Some(Animation::high(10518)),
            g(1853, 0),
            None,
            Some(graphics(1854, 0, 30)),
            &[(Earth, 4), (Soul, 4), (Blood, 4)],
        ),
    );
    out
}

/// The canonical player's save registry (`player::persisted::spells`)
/// filled from this table: each registration with its own book ordinal and
/// `combat = true`. Non-combat spells are absent, so loading an autocast at
/// one of their buttons reads a null spell here where the full original
/// registry would fail the cast.
pub fn persisted_registry() -> crate::player::persisted::spells::Registry {
    let mut registry = crate::player::persisted::spells::Registry::default();
    for (book, button, spell) in combat_spells() {
        registry.spells.insert(
            (book.ordinal(), button),
            crate::player::persisted::spells::Spell {
                book: spell.book.ordinal(),
                combat: true,
            },
        );
    }
    registry
}
