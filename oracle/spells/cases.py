"""The designed spell corpus: every case is (case, sweep), where sweep names the
operation whose recorded calls each fail once in turn (None for no sweep)."""
import random
import struct

NEEDED_EVENTS = ['ticks', 'dev_mode', 'name', 'location', 'level', 'level_with', 'add_experience', 'add_experience2', 'equipment',
                 'equipment_new', 'inventory_amount', 'inventory_remove', 'send_message', 'message', 'interface_config', 'dispatch',
                 'face_location', 'face', 'graphics', 'animate', 'force_animation', 'global_audio', 'projectile_magic', 'projectile',
                 'get_attribute', 'set_attribute', 'remove_attribute', 'autocast', 'set_autocast', 'set_spell', 'is_attacking',
                 'set_victim', 'update_style', 'pulse_start', 'pulse_attack', 'attack_style', 'attack_style_index', 'magic_animation',
                 'attack_animation', 'multi_zone', 'bonuses', 'in_zone', 'timer_active', 'god_casts', 'npc_id', 'npc_task',
                 'combat_graphics', 'multi_targets', 'open_tab', 'set_varbit', 'component_visibility', 'set_attack_style', 'math_random']

AIR, WATER, EARTH, FIRE, MIND, NATURE, CHAOS, DEATH, COSMIC, BLOOD, SOUL, ASTRAL, LAW, BODY = (
    556, 555, 557, 554, 558, 561, 562, 560, 564, 565, 566, 9075, 563, 559)
LAVA, STEAM, MIST, DUST, SMOKE, MUD, ELEMENTAL, CATALYTIC = 4699, 4694, 4695, 4696, 4697, 4698, 12850, 12851
COMBOS = [LAVA, STEAM, MIST, DUST, SMOKE, MUD, ELEMENTAL, CATALYTIC]
RUNE_IDS = [AIR, WATER, EARTH, FIRE, MIND, NATURE, CHAOS, DEATH, COSMIC, BLOOD, SOUL, ASTRAL, LAW, BODY]
RUNE_NAMES = ['AIR_RUNE', 'WATER_RUNE', 'EARTH_RUNE', 'FIRE_RUNE', 'MIND_RUNE', 'NATURE_RUNE', 'CHAOS_RUNE', 'DEATH_RUNE', 'COSMIC_RUNE',
              'BLOOD_RUNE', 'SOUL_RUNE', 'ASTRAL_RUNE', 'LAW_RUNE', 'BODY_RUNE', 'SARADOMIN_STAFF', 'GUTHIX_STAFF', 'ZAMORAK_STAFF',
              'ZURIELS_STAFF']
TYPES = ['STRIKE', 'BOLT', 'CRUMBLE_UNDEAD', 'BLAST', 'WAVE', 'RUSH', 'BURST', 'BLITZ', 'BARRAGE', 'CONFUSE', 'WEAKEN', 'CURSE',
         'VULNERABILITY', 'ENFEEBLE', 'STUN', 'GOD_STRIKE', 'BIND', 'SNARE', 'ENTANGLE', 'MAGIC_DART', 'IBANS_BLAST', 'TRIDENT_OF_SEAS',
         'TELEBLOCK', 'NULL']
STAVES = [1381, 1397, 1405, 21777, 1383, 1395, 1403, 6563, 6562, 1385, 1399, 1407, 3053, 3054, 3055, 3056, 1387, 1393, 1401, 11736,
          11738, 557, 2415, 2416, 2417, 13867, 4170, 8841, 14726, 1409, 4675, 0]
NAMES = {
    '0': 'Dwarf remains', '556': 'Air rune', '555': 'Water rune', '557': 'Earth rune', '554': 'Fire rune', '558': 'Mind rune',
    '561': 'Nature rune', '562': 'Chaos rune', '560': 'Death rune', '564': 'Cosmic rune', '565': 'Blood rune', '566': 'Soul rune',
    '9075': 'Astral rune', '563': 'Law rune', '559': 'Body rune', '4170': "Slayer's staff", '8841': 'Void knight mace',
    '2415': 'Saradomin staff', '2416': 'Guthix staff', '2417': 'Zamorak staff', '4675': 'Ancient staff', '13867': "Zuriel's staff",
    '13869': "Zuriel's staff (deg)", '1381': 'Staff of air', '1409': "Iban's staff", '14726': 'Staff of light', '777': 'Chaos gauntlets',
    '1387': 'Staff of fire', '4699': 'Lava rune', '12850': 'Elemental rune', '12851': 'Catalytic rune',
}
# (book, button, level) of every registered combat spell.
MODERN = [(1, 1), (2, 3), (4, 5), (6, 9), (7, 11), (8, 13), (10, 17), (11, 19), (12, 20), (14, 23), (17, 29), (20, 35), (22, 39),
          (24, 41), (27, 47), (29, 50), (30, 50), (31, 50), (33, 53), (38, 59), (41, 60), (42, 60), (43, 60), (45, 62), (48, 65),
          (50, 66), (52, 70), (53, 73), (55, 75), (56, 79), (57, 80), (63, 85), (63232, 1)]
ANCIENT = [(0, 58), (1, 82), (2, 70), (3, 94), (4, 56), (5, 80), (6, 68), (7, 92), (8, 50), (9, 74), (10, 62), (11, 86), (12, 52),
           (13, 76), (14, 64), (15, 88), (16, 61), (17, 85), (18, 73), (19, 97)]
SPELLS = [('MODERN', b, l) for b, l in MODERN] + [('ANCIENT', b, l) for b, l in ANCIENT]
RUNES = {
    ('MODERN', 1): [[MIND, 1], [AIR, 1]], ('MODERN', 2): [[BODY, 1], [EARTH, 2], [WATER, 3]], ('MODERN', 4): [[MIND, 1], [WATER, 1], [AIR, 1]],
    ('MODERN', 6): [[MIND, 1], [EARTH, 2], [AIR, 1]], ('MODERN', 7): [[BODY, 1], [EARTH, 2], [WATER, 3]],
    ('MODERN', 8): [[MIND, 1], [FIRE, 3], [AIR, 2]], ('MODERN', 10): [[CHAOS, 1], [AIR, 2]], ('MODERN', 11): [[BODY, 1], [EARTH, 3], [WATER, 2]],
    ('MODERN', 12): [[NATURE, 2], [EARTH, 3], [WATER, 3]], ('MODERN', 14): [[CHAOS, 1], [WATER, 2], [AIR, 2]],
    ('MODERN', 17): [[CHAOS, 1], [EARTH, 3], [AIR, 2]], ('MODERN', 20): [[CHAOS, 1], [FIRE, 4], [AIR, 3]],
    ('MODERN', 22): [[EARTH, 2], [AIR, 2], [CHAOS, 1]], ('MODERN', 24): [[DEATH, 1], [AIR, 3]], ('MODERN', 27): [[DEATH, 1], [WATER, 3], [AIR, 3]],
    ('MODERN', 29): [[FIRE, 5], [DEATH, 1]], ('MODERN', 30): [[NATURE, 3], [EARTH, 4], [WATER, 4]], ('MODERN', 31): [[DEATH, 1], [MIND, 4]],
    ('MODERN', 33): [[DEATH, 1], [EARTH, 4], [AIR, 3]], ('MODERN', 38): [[DEATH, 1], [FIRE, 5], [AIR, 4]],
    ('MODERN', 41): [[BLOOD, 2], [FIRE, 2], [AIR, 4]], ('MODERN', 42): [[BLOOD, 2], [FIRE, 1], [AIR, 4]],
    ('MODERN', 43): [[BLOOD, 2], [FIRE, 4], [AIR, 1]], ('MODERN', 45): [[BLOOD, 1], [AIR, 5]], ('MODERN', 48): [[BLOOD, 1], [WATER, 7], [AIR, 5]],
    ('MODERN', 50): [[SOUL, 1], [EARTH, 5], [WATER, 5]], ('MODERN', 52): [[BLOOD, 1], [EARTH, 7], [AIR, 5]],
    ('MODERN', 53): [[SOUL, 1], [EARTH, 8], [WATER, 8]], ('MODERN', 55): [[BLOOD, 1], [FIRE, 7], [AIR, 5]],
    ('MODERN', 56): [[NATURE, 4], [EARTH, 5], [WATER, 5]], ('MODERN', 57): [[SOUL, 1], [EARTH, 12], [WATER, 12]],
    ('MODERN', 63): [[LAW, 1], [DEATH, 1], [CHAOS, 1]], ('MODERN', 63232): [],
    ('ANCIENT', 0): [[DEATH, 2], [CHAOS, 2], [WATER, 2]], ('ANCIENT', 1): [[BLOOD, 2], [DEATH, 2], [WATER, 3]],
    ('ANCIENT', 2): [[DEATH, 2], [CHAOS, 4], [WATER, 4]], ('ANCIENT', 3): [[BLOOD, 2], [DEATH, 4], [WATER, 6]],
    ('ANCIENT', 4): [[BLOOD, 1], [DEATH, 2], [CHAOS, 2]], ('ANCIENT', 5): [[BLOOD, 4], [DEATH, 2]],
    ('ANCIENT', 6): [[BLOOD, 2], [DEATH, 2], [CHAOS, 4]], ('ANCIENT', 7): [[SOUL, 1], [BLOOD, 4], [DEATH, 4]],
    ('ANCIENT', 8): [[DEATH, 2], [CHAOS, 2], [FIRE, 1], [AIR, 1]], ('ANCIENT', 9): [[BLOOD, 2], [DEATH, 2], [FIRE, 2], [AIR, 2]],
    ('ANCIENT', 10): [[DEATH, 2], [CHAOS, 4], [FIRE, 2], [AIR, 2]], ('ANCIENT', 11): [[BLOOD, 2], [DEATH, 4], [FIRE, 4], [AIR, 4]],
    ('ANCIENT', 12): [[SOUL, 1], [DEATH, 2], [CHAOS, 2], [AIR, 1]], ('ANCIENT', 13): [[SOUL, 2], [BLOOD, 2], [DEATH, 2], [AIR, 2]],
    ('ANCIENT', 14): [[SOUL, 2], [DEATH, 2], [CHAOS, 4], [AIR, 1]], ('ANCIENT', 15): [[SOUL, 3], [BLOOD, 2], [DEATH, 4], [AIR, 4]],
    ('ANCIENT', 16): [[EARTH, 1], [SOUL, 1], [CHAOS, 2]], ('ANCIENT', 17): [[EARTH, 3], [SOUL, 3], [BLOOD, 2]],
    ('ANCIENT', 18): [[EARTH, 2], [SOUL, 2], [CHAOS, 4]], ('ANCIENT', 19): [[EARTH, 4], [SOUL, 4], [BLOOD, 4]],
}
MAGIC = '6'


def hx(value):
    return struct.pack('>d', value).hex()


def player(magic=99, **extra):
    spec = dict(kind='player', name='Caster', location=[3200, 3200, 0], levels={MAGIC: magic, '18': 99}, worn={}, inventory=[],
                style=8, styleIndex=0, godCasts=[100, 100, 100])
    spec.update(extra)
    return spec


def npc(npc_id=1, **extra):
    spec = dict(kind='npc', name='Target', npcId=npc_id, location=[3201, 3200, 0], levels={MAGIC: 50},
                combatGraphics=[[2728, 96, 0], [2729, 40, 0], [2737, 96, 0]], bonuses=[0] * 15)
    spec.update(extra)
    return spec


def node(**extra):
    spec = dict(kind='node', name='Scenery', location=[3202, 3200, 0])
    spec.update(extra)
    return spec


def case(group, ops, entities=None, **extra):
    c = dict(group=group, entities=entities or [player()], ops=ops, ticks=100)
    c.update(extra)
    return c


def full(spell):
    return [[i, a * 10] for i, a in RUNES[spell]]


def ref(book, button):
    return [book, button]


def static_cases():
    yield case('table', [dict(op='table')]), None
    ids = sorted(set(RUNE_IDS + COMBOS + STAVES + [-1, -2 ** 31, 2 ** 31 - 1, 553, 567, 9074, 9076, 2414, 2418, 13866, 13868, 12849, 12852]))
    yield case('runes', [dict(op='runes_for_id', id=i) for i in ids] + [dict(op='staff_for_id', rune=i) for i in ids]
               + [dict(op='eligible_for', rune=r) for r in RUNE_NAMES + [None]]), None
    weapons = [None, 1381, 1397, 1405, 1383, 1395, 1403, 1385, 1399, 1407, 1387, 1393, 1401, 2415, 2416, 2417, 13867, 0, 557, 8841, 14726]
    type_sets = [None, [], ['GOD_STRIKE'], ['BARRAGE'], ['BLITZ'], ['RUSH'], ['BURST'], ['STRIKE'], ['WAVE'], ['GOD_STRIKE', 'GOD_STRIKE'],
                 ['BARRAGE', 'BLITZ']]
    for rune in RUNE_NAMES + [None]:
        ops = [dict(op='is_infinite', rune=rune, weapon=w, types=t) for w in weapons for t in type_sets]
        yield case('infinite', ops), None
    for t in TYPES:
        yield case('types', [dict(op='type_impact', type=t, e=0, victim=1, base=b) for b in [-1, 0, 1, 2, 4, 5, 2 ** 31 - 1]],
                   [player(), npc()]), None


def type_cases():
    bases = [-2 ** 31, -1, 0, 3, 2 ** 31 - 1]
    for t in TYPES:
        for magic in [-1, 0, 9, 10, 74, 75, 76, 77, 78, 99, 255] if t in ('MAGIC_DART', 'TRIDENT_OF_SEAS') else [99]:
            entities = [player(magic=magic, worn={'9': 777, '1': 2412}, timers=['magic:spellcharge']), npc(205), npc(204), player(magic=magic)]
            ops = [dict(op='type_impact', type=t, e=e, victim=v, base=b) for e in [0, 1, 3, None] for v in [1, 2, 0, None] for b in bases]
            yield case('types', ops, entities), None
    for cape in [2411, 2412, 2413, 2414, 2415, None]:
        for timer in [True, False]:
            worn = {} if cape is None else {'1': cape}
            entities = [player(worn=worn, timers=['magic:spellcharge'] if timer else []), npc()]
            yield case('god_strike', [dict(op='type_impact', type='GOD_STRIKE', e=0, victim=1, base=0),
                                      dict(op='type_impact', type='GOD_STRIKE', e=1, victim=0, base=0)], entities), 0 if cape == 2413 and timer else None
    for gloves in [776, 777, 778, None]:
        worn = {} if gloves is None else {'9': gloves}
        yield case('bolt', [dict(op='type_impact', type='BOLT', e=0, victim=1, base=b) for b in [1, 2, 3, 4]], [player(worn=worn), npc()]), \
            0 if gloves == 777 else None


def staff_cases():
    runes = sorted(set(RUNE_IDS + COMBOS + [554, 555, 556, 557, 0, -1, 2415, 13867]))
    for weapon in STAVES + [None]:
        worn = {} if weapon is None else {'3': weapon}
        ops = []
        for r in runes:
            ops.append(dict(op='using_staff', spell=ref('MODERN', 1), p=0, rune=r))
            ops.append(dict(op='utils_using_staff', p=0, rune=r))
        yield case('staves', ops, [player(worn=worn)]), None


def inventory(rng, ids, max_amount=12):
    inv = []
    for i in ids:
        if rng.random() < 0.6:
            inv.append([i, rng.randint(0, max_amount)])
            if rng.random() < 0.2:
                inv.append([i, rng.randint(1, 5)])
    rng.shuffle(inv)
    return inv


COMBO_SPELLS = [('MODERN', 1), ('MODERN', 8), ('MODERN', 12), ('MODERN', 22), ('MODERN', 41), ('MODERN', 52), ('MODERN', 57),
                ('ANCIENT', 3), ('ANCIENT', 11), ('ANCIENT', 13), ('ANCIENT', 19)]


def rune_cases():
    # Every spell's own runes: exact, one short in each rune, with each elemental staff and each combination rune.
    for spell in sorted(RUNES):
        req = RUNES[spell]
        if not req:
            continue
        exact = [[i, a] for i, a in req]
        yield case('runes_exact', [dict(op='has_runes', p=0, runes=req)], [player(inventory=exact)]), 0
        for k in range(len(req)):
            short = [[i, a - 1 if j == k else a] for j, (i, a) in enumerate(req)]
            yield case('runes_short', [dict(op='has_runes', p=0, runes=req)], [player(inventory=short)]), None
        for staff in [1381, 1385, 3053, 6562]:
            yield case('runes_staff', [dict(op='has_runes', p=0, runes=req)], [player(inventory=exact, worn={'3': staff})]), None
        if spell not in COMBO_SPELLS:
            continue
        for combo in COMBOS:
            for amount in [0, 1, 100]:
                inv = [[i, a] for i, a in req] + [[combo, amount]]
                yield case('runes_combo', [dict(op='has_runes', p=0, runes=req)], [player(inventory=inv)]), None
                only = [[i, a] for i, a in req if i not in (AIR, WATER, EARTH, FIRE, MIND, CHAOS, DEATH, BLOOD, SOUL, ASTRAL)] + [[combo, amount]]
                yield case('runes_combo', [dict(op='has_runes', p=0, runes=req)], [player(inventory=only)]), None
    rng = random.Random(530)
    pool = RUNE_IDS + [2415, 13867]
    for n in range(600):
        req = [[rng.choice(pool), rng.randint(-1, 8)] for _ in range(rng.randint(0, 6))]
        inv = inventory(rng, RUNE_IDS + COMBOS)
        worn = {'3': rng.choice(STAVES)} if rng.random() < 0.5 else {}
        sweep = 0 if n % 60 == 0 else None
        yield case('runes_random', [dict(op='has_runes', p=0, runes=req)], [player(inventory=inv, worn=worn)]), sweep
    # Large amounts: wrapping cost sums and combination usage.
    for amounts in [[2 ** 31 - 1, 1], [2 ** 31 - 1, 2 ** 31 - 1], [-5, 3], [-(2 ** 31), -1]]:
        req = [[AIR, amounts[0]], [AIR, amounts[1]], [WATER, 1]]
        yield case('runes_wrap', [dict(op='has_runes', p=0, runes=req)], [player(inventory=[[AIR, 5], [MIST, 2 ** 31 - 1], [WATER, 1]])]), None


def meets_cases():
    for book, button, level in SPELLS:
        s = ref(book, button)
        req = RUNES[(book, button)]
        exact = [[i, a] for i, a in req]
        worn = {'3': {41: 2415, 42: 2416, 43: 2417, 31: 4170}.get(button, 1381)} if book == 'MODERN' else {'3': 4675}
        for magic in sorted({max(level - 1, 0), level, level + 1}):
            for message in [True, False]:
                for remove in [True, False]:
                    ops = [dict(op='meets', spell=s, e=0, message=message, remove=remove), dict(op='check_level', spell=s, e=0, message=message)]
                    sweep = 0 if magic == level and message and remove else None
                    yield case('meets', ops, [player(magic=magic, inventory=exact, worn=worn)]), sweep
        # Missing runes with and without the message; an NPC caster; a plain entity-less node is never a caster.
        if req:
            short = [[i, a - 1] for i, a in req]
            for message in [True, False]:
                yield case('meets_missing', [dict(op='meets', spell=s, e=0, message=message, remove=True)],
                           [player(inventory=short, worn=worn)]), None
        yield case('meets_npc', [dict(op='meets', spell=s, e=0, message=True, remove=True), dict(op='check_level', spell=s, e=0, message=True)],
                   [npc()]), None
    # Autocast staff checks: Magic Dart needs a Slayer's staff, Claws of Guthix a Void knight mace, others nothing.
    for auto in [ref('MODERN', 31), ref('MODERN', 42), ref('MODERN', 1), None]:
        for weapon in [4170, 8841, 1381, None]:
            worn = {} if weapon is None else {'3': weapon}
            yield case('meets_autocast', [dict(op='meets', spell=ref('MODERN', 1), e=0, message=True, remove=True)],
                       [player(worn=worn, autocast=auto, inventory=[[MIND, 5], [AIR, 5]])]), 0 if auto == ref('MODERN', 31) and weapon == 4170 else None
    # Binding spells: the entangle delay against the tick.
    for button in [12, 30, 56, 1]:
        for delay in [None, 99, 100, 101]:
            attrs = {} if delay is None else {'entangleDelay': delay}
            req = RUNES[('MODERN', button)]
            yield case('meets_bind', [dict(op='meets', spell=ref('MODERN', button), e=0, message=True, remove=True)],
                       [player(attributes=attrs, inventory=[[i, a] for i, a in req])]), 0 if delay == 101 else None
    # Staff of light: 13 in 101 draws negate the runes.
    for draw in [0.0, 0.1188, 0.1287, 0.12871287128712872, 0.1288, 0.5, 0.9999999]:
        for inv in [[], [[MIND, 1], [AIR, 1]]]:
            yield case('meets_light', [dict(op='meets', spell=ref('MODERN', 1), e=0, message=True, remove=True)],
                       [player(worn={'3': 14726}, inventory=inv)], math=[hx(draw)]), 0 if draw == 0.0 and not inv else None
    # God spells: casts inside the arena, staves, the Void knight mace for Guthix claws.
    for button, index in [(41, 0), (42, 1), (43, 2)]:
        req = RUNES[('MODERN', button)]
        for casts in [0, 99, 100, 150]:
            for arena in [True, False]:
                for weapon in [2415, 2416, 2417, 8841, None]:
                    gc = [100, 100, 100]
                    gc[index] = casts
                    worn = {} if weapon is None else {'3': weapon}
                    zones = ['mage arena'] if arena else []
                    sweep = 0 if casts == 99 and not arena and weapon == 2415 else None
                    yield case('meets_god', [dict(op='meets', spell=ref('MODERN', button), e=0, message=True, remove=True)],
                               [player(godCasts=gc, zones=zones, worn=worn, inventory=[[i, a] for i, a in req])]), sweep
        yield case('meets_god', [dict(op='meets', spell=ref('MODERN', button), e=0, message=False, remove=False)],
                   [player(worn={'3': 1381}, inventory=[[i, a] for i, a in req])]), None
        yield case('meets_god', [dict(op='meets', spell=ref('MODERN', button), e=0, message=True, remove=True)], [npc()]), None


def removal_cases():
    # Repeated casts take runes from split stacks until they run out; a combination rune covers both parts once.
    stacks = [
        [[MIND, 1], [AIR, 1], [MIND, 1], [AIR, 2]],
        [[DUST, 2], [MIND, 3]],
        [[ELEMENTAL, 1], [AIR, 1], [MIND, 2]],
        [[MIST, 1], [SMOKE, 1], [AIR, 3], [MIND, 1], [CATALYTIC, 1]],
        [[CATALYTIC, 2], [AIR, 1], [AIR, 1]],
    ]
    for inv in stacks:
        ops = [dict(op='meets', spell=ref('MODERN', 1), e=0, message=True, remove=True) for _ in range(4)]
        yield case('removal', ops, [player(inventory=inv)]), 0
    for spell in [('MODERN', 8), ('MODERN', 52), ('ANCIENT', 11), ('ANCIENT', 19)]:
        req = RUNES[spell]
        inv = [[i, a] for i, a in req] + [[i, 1] for i, _ in req] + [[LAVA, 3], [SMOKE, 2], [DUST, 1], [ELEMENTAL, 2]]
        ops = [dict(op='meets', spell=ref(*spell), e=0, message=True, remove=True) for _ in range(4)]
        yield case('removal', ops, [player(inventory=inv)]), None
    # A stale spell:runes list survives a staff of light negation and a failed check.
    yield case('removal', [dict(op='meets', spell=ref('MODERN', 1), e=0, message=True, remove=False),
                           dict(op='set', e=0, fields=dict(worn={'3': 14726})),
                           dict(op='meets', spell=ref('MODERN', 1), e=0, message=True, remove=True),
                           dict(op='set', e=0, fields=dict(worn={})),
                           dict(op='meets', spell=ref('MODERN', 1), e=0, message=True, remove=True)],
               [player(inventory=[[MIND, 1], [AIR, 1]])], math=[hx(0.01)]), None
    # A non-list spell:runes attribute fails the removal's cast.
    yield case('removal', [dict(op='set', e=0, fields=dict(attributes={'spell:runes': 5})),
                           dict(op='has_runes', p=0, runes=[]),
                           dict(op='set', e=0, fields=dict(attributes={'spell:runes': 5})),
                           dict(op='meets', spell=ref('ANCIENT', 0), e=0, message=True, remove=True)],
               [player(inventory=[[DEATH, 2], [CHAOS, 2], [WATER, 2]])]), None


def experience_cases():
    for book, button, _ in SPELLS:
        for kind, style in [('player', 7), ('player', 8), ('player', None), ('npc', 8)]:
            e = player(style=style) if kind == 'player' else npc(style=style)
            ops = [dict(op='add_experience', spell=ref(book, button), e=0, hit=hit, experienceMod=hx(mod))
                   for mod in [4.0, 0.1, 1.0 / 3] for hit in [-1, 0, 1, 2, 37]]
            ops.append(dict(op='experience', spell=ref(book, button), p=0))
            sweep = 14 if kind == 'player' and style == 7 and button in (1, 2) else None
            yield case('experience', ops, [e]), sweep


def visual_cases():
    for book, button, _ in SPELLS:
        s = ref(book, button)
        for kind in ['player', 'npc']:
            for auto in [s, ref('MODERN', 31), None]:
                for animation in [None, [7000, 0, 'MID']]:
                    if kind == 'player' and animation is not None:
                        continue
                    e = player(autocast=auto) if kind == 'player' else npc(1, autocast=auto, magicAnimation=animation)
                    sweep = 0 if auto is None and kind == 'player' and button in (1, 41, 0, 16) else None
                    yield case('visualize', [dict(op='visualize', spell=s, e=0, target=t) for t in [1, 2, None]], [e, npc(2), node()]), sweep
        ops = [dict(op='visualize_impact', spell=s, e=entity, target=target, hit=hit, frozen=frozen)
               for hit in [-1, 0, 5] for frozen in [False, True] for entity in [0, 1, None] for target in [1, None]]
        sweep = 0 if button in (1, 3, 41) else None
        yield case('impact', ops, [player(godCasts=[98, 99, 100]), npc(2)]), sweep
    # God spells cast by the god NPCs force their attack animation.
    for npc_id in range(905, 917):
        yield case('visualize_god_npc', [dict(op='visualize', spell=ref('MODERN', button), e=0, target=1) for button in (41, 42, 43)],
                   [npc(npc_id, attackAnimation=[811, 0, 'HIGH']), player()]), None
    # Repeated god casts count to 100 and announce it once.
    for casts in [97, 98, 99]:
        ops = [dict(op='visualize_impact', spell=ref('MODERN', b), e=0, target=1, hit=h, frozen=False) for b in (41, 42, 43) for h in (5, -1)]
        yield case('god_casts', ops, [player(godCasts=[casts, casts, casts]), npc(2)]), None


def impact_cases():
    for book, button, _ in SPELLS:
        s = ref(book, button)
        ops = []
        for e in [0, 1, 2, None]:
            for v in [1, 3, 0, None]:
                ops.append(dict(op='max_impact', spell=s, e=e, victim=v))
        ops.append(dict(op='accuracy', spell=s))
        yield case('max_impact', ops, [player(worn={'9': 777, '1': 2414}, timers=['magic:spellcharge']), npc(5), player(magic=1), npc(205)]), None
        for magic in [1, 9, 10, 55, 99]:
            yield case('max_impact', [dict(op='max_impact', spell=s, e=0, victim=1)], [player(magic=magic), npc(205)]), None


def target_cases():
    for book, button, _ in SPELLS:
        s = ref(book, button)
        for multi_e in [True, False]:
            for multi_t in [True, False]:
                for listed in [[], [1], [1, 2, 3]]:
                    entities = [player(multi=multi_e), npc(2, multi=multi_t, multiTargets=listed), npc(3), player()]
                    sweep = 0 if multi_e and multi_t and listed == [1, 2, 3] and button in (2, 18) else None
                    yield case('targets', [dict(op='targets', spell=s, e=0, target=1)], entities), sweep
        yield case('targets', [dict(op='targets', spell=s, e=0, target=None)], [player(multi=True)]), None


def cast_cases():
    for book, button, level in SPELLS:
        s = ref(book, button)
        req = RUNES[(book, button)]
        worn = {'3': {41: 2415, 42: 2416, 43: 2417, 31: 4170, 29: 1409}.get(button, 1381)} if book == 'MODERN' else {'3': 13867}
        for attacking in [True, False]:
            for target in [1, 2, 3]:
                entities = [player(worn=worn, attacking=attacking, inventory=[[i, a] for i, a in req], zones=['Wilderness']),
                            npc(2044 if button == 22 else 1, task=True), node(), player(zones=['Wilderness'])]
                sweep = 0 if not attacking and target in (1, 3) and button in (1, 22, 29, 31, 63, 16) else None
                yield case('cast', [dict(op='cast', spell=s, e=0, target=target)], entities), sweep
        yield case('cast_npc', [dict(op='cast', spell=s, e=0, target=1)], [npc(5), player(), npc(6)]), None
        yield case('cast_low', [dict(op='cast', spell=s, e=0, target=1)], [player(magic=max(level - 1, 0), worn=worn), npc(2)]), None
    # Crumble undead: zogres, skogres, Slash Bash, undead tasks and players.
    for npc_id in [2043, 2044, 2049, 2050, 2051, 2055, 2056, 2057, 2058, 2060, 1]:
        for task in [True, False, None]:
            yield case('crumble', [dict(op='cast', spell=ref('MODERN', 22), e=0, target=1)],
                       [player(inventory=[[EARTH, 2], [AIR, 2], [CHAOS, 1]]), npc(npc_id, task=task)]), None
    yield case('crumble', [dict(op='cast', spell=ref('MODERN', 22), e=0, target=1)], [player(), player()]), None
    yield case('crumble', [dict(op='cast', spell=ref('MODERN', 22), e=0, target=1)], [npc(), npc(1, task=False)]), None
    # Iban blast, magic dart, teleblock and miasmic gates.
    for weapon in [1409, 1408, None]:
        yield case('iban', [dict(op='cast', spell=ref('MODERN', 29), e=0, target=1)],
                   [player(worn={} if weapon is None else {'3': weapon}, inventory=[[FIRE, 5], [DEATH, 1]]), npc()]), None
    for slayer in [54, 55, 56]:
        for weapon in [4170, 4171]:
            yield case('dart', [dict(op='cast', spell=ref('MODERN', 31), e=0, target=1)],
                       [player(levels={MAGIC: 99, '18': slayer}, worn={'3': weapon}, inventory=[[DEATH, 1], [MIND, 4]]), npc()]), None
    yield case('dart', [dict(op='cast', spell=ref('MODERN', 31), e=0, target=1)], [npc(levels={'18': 54}), player()]), None
    yield case('dart', [dict(op='cast', spell=ref('MODERN', 31), e=0, target=1)], [npc(levels={'18': 99}), player()]), None
    for own in [True, False]:
        for theirs in [True, False]:
            for timer in [True, False]:
                yield case('teleblock', [dict(op='cast', spell=ref('MODERN', 63), e=0, target=1)],
                           [player(zones=['Wilderness'] if own else [], inventory=[[LAW, 1], [DEATH, 1], [CHAOS, 1]]),
                            player(zones=['Wilderness'] if theirs else [], timers=['teleblock'] if timer else [])]), None
    yield case('teleblock', [dict(op='cast', spell=ref('MODERN', 63), e=0, target=1)], [player(), npc()]), None
    yield case('teleblock', [dict(op='cast', spell=ref('MODERN', 63), e=0, target=1)], [npc(), npc()]), None
    for weapon in [13867, 13869, 13841, 13843, 4675, None]:
        for dev in [True, False]:
            yield case('miasmic', [dict(op='cast', spell=ref('ANCIENT', 16), e=0, target=1)],
                       [player(worn={} if weapon is None else {'3': weapon}, inventory=[[EARTH, 1], [SOUL, 1], [CHAOS, 2]]), npc()],
                       devMode=dev), None


def cast_spell_cases():
    for book, button, _ in SPELLS[:6] + SPELLS[33:37]:
        req = RUNES[(book, button)]
        worn = {'3': 1381} if book == 'MODERN' else {'3': 13867}
        for delay in [None, 99, 100, 101]:
            for own in ['MODERN', 'ANCIENT', 'LUNAR']:
                attrs = {} if delay is None else {'magic-delay': delay}
                sweep = 0 if delay == 100 and own == book else None
                yield case('cast_spell', [dict(op='cast_spell', book=book, button=button, p=0, target=1)],
                           [player(book=own, attributes=attrs, worn=worn, inventory=[[i, a * 3] for i, a in req]), npc()]), sweep
    for location in [[3215, 3200, 0], [3216, 3200, 0], [3210, 3211, 0], [3211, 3211, 0], [3201, 3200, 1], None]:
        for target in [1, 2, 0]:
            entities = [player(inventory=[[MIND, 9], [AIR, 9]]), npc(location=location), node(location=location)]
            yield case('cast_spell_range', [dict(op='cast_spell', book='MODERN', button=1, p=0, target=target)], entities), None
    # Unknown buttons, a book the spell does not belong to, three casts across ticks.
    yield case('cast_spell', [dict(op='cast_spell', book='MODERN', button=b, p=0, target=1) for b in [0, 3, 64, -1]], [player(), npc()]), None
    yield case('cast_spell', [dict(op='cast_spell', book='LUNAR', button=1, p=0, target=1)], [player(book='LUNAR'), npc()]), None
    yield case('cast_spell', [dict(op='cast_spell', book='MODERN', button=1, p=0, target=None)], [player(), npc()]), None
    ops = []
    for tick in [100, 101, 102, 103, 104]:
        ops.append(dict(op='world', ticks=tick))
        ops.append(dict(op='cast_spell', book='MODERN', button=1, p=0, target=1))
    yield case('cast_spell_ticks', ops, [player(inventory=[[MIND, 9], [AIR, 9]]), npc()]), None


def book_cases():
    ops = [dict(op='for_interface', id=i) for i in [-1, 0, 191, 192, 193, 194, 429, 430, 431, 2 ** 31 - 1]]
    ops += [dict(op='get_spell', book=b, button=i) for b in ['MODERN', 'ANCIENT', 'LUNAR'] for i in list(range(-1, 66)) + [63232, 63231]]
    yield case('books', ops), None
    for start in ['MODERN', 'ANCIENT', 'LUNAR']:
        ops = []
        for book in ['ANCIENT', 'LUNAR', 'MODERN', 'MODERN', 'LUNAR']:
            ops.append(dict(op='set_book', p=0, book=book))
            ops.append(dict(op='book_update', p=0))
        yield case('books', ops, [player(book=start)]), 1 if start == 'MODERN' else None


def block_cases():
    entities = [player(name='Caster'), npc(name='Guard'), npc(name='Guard'), node(name='Door'), npc(name=None), node(name='guard')]
    ops = [dict(op='is_blocked', spellId=1, node=0)]
    for spell_id, n in [(1, 1), (1, 3), (2, 4), (3, 1), (3, 1)]:
        ops.append(dict(op='block_register', spellId=spell_id, node=n))
        ops += [dict(op='is_blocked', spellId=s, node=k) for s in [1, 2, 3, 4] for k in range(6)]
    yield case('blocks', ops, entities), None
    yield case('blocks', [dict(op='block_register', spellId=7, node=1), dict(op='is_blocked', spellId=7, node=2)], entities), 1


def autocast_cases():
    weapons = [None, 1381, 4170, 8841, 4675, 13867, 13869]
    for book in ['MODERN', 'ANCIENT', 'LUNAR']:
        for weapon in weapons:
            worn = {} if weapon is None else {'3': weapon}
            ops = [dict(op='autospell_id', p=0, spellId=i) for i in list(range(-1, 60)) + [63232]]
            ops += [dict(op='can_autocast', p=0, message=m) for m in [True, False]]
            yield case('autocast_ids', ops, [player(book=book, worn=worn)]), None
            for staff in [True, False]:
                yield case('autocast_can', [dict(op='can_autocast', p=0, message=True)], [player(book=book, worn=worn, staffInterface=staff)]), None
    for book in ['MODERN', 'ANCIENT']:
        for weapon in [None, 4170, 8841, 4675]:
            worn = {} if weapon is None else {'3': weapon}
            for style in [0, 3]:
                for current in [None, ref('MODERN', 1), ref('MODERN', 31), ref('ANCIENT', 8)]:
                    ops = []
                    for button in [-2, -1, 0, 1, 2, 3, 4, 5, 6, 15, 16, 17, 18, 19, 20, 21]:
                        for adjust, forget in [(True, True), (False, False), (True, False)]:
                            ops.append(dict(op='select_auto', p=0, button=button, adjust=adjust, forget=forget))
                    sweep = 4 if weapon == 4170 and style == 3 and current == ref('MODERN', 1) else None
                    yield case('autocast_select', ops, [player(book=book, worn=worn, styleIndex=style, autocast=current)]), sweep
    # Book switches with autocast on and off.
    ops = [dict(op='select_auto', p=0, button=3, adjust=False, forget=False), dict(op='set_book', p=0, book='ANCIENT'),
           dict(op='can_autocast', p=0, message=True), dict(op='select_auto', p=0, button=3, adjust=True, forget=False),
           dict(op='set', e=0, fields=dict(worn={'3': 4675})), dict(op='can_autocast', p=0, message=True),
           dict(op='select_auto', p=0, button=19, adjust=False, forget=False), dict(op='set_book', p=0, book='MODERN'),
           dict(op='select_auto', p=0, button=-1, adjust=True, forget=True), dict(op='set_book', p=0, book='LUNAR'),
           dict(op='can_autocast', p=0, message=True), dict(op='select_auto', p=0, button=2, adjust=True, forget=True)]
    yield case('autocast_switch', ops, [player(worn={'3': 1381}, styleIndex=3)]), 3


def default_cases():
    graphics = [
        [[2728, 96, 0], [2729, 40, 0], [2737, 96, 0]],
        [[2728, 96, 0], None, [2737, 96, 0]],
        [None, None, None],
        [[2728, 96, 0], [2729, 40, 0]],
        [],
        None,
    ]
    for g in graphics:
        for animation in [None, [711, 0, 'HIGH']]:
            ops = [dict(op='make_default', npc=0), dict(op='visualize', spell=['DEFAULT', 0], e=0, target=1),
                   dict(op='visualize', spell=['DEFAULT', 0], e=0, target=2),
                   dict(op='visualize_impact', spell=['DEFAULT', 0], e=0, target=1, hit=-1, frozen=False),
                   dict(op='visualize_impact', spell=['DEFAULT', 0], e=0, target=1, hit=3, frozen=False),
                   dict(op='meets', spell=['DEFAULT', 0], e=0, message=True, remove=True),
                   dict(op='cast', spell=['DEFAULT', 0], e=0, target=1),
                   dict(op='targets', spell=['DEFAULT', 0], e=0, target=1),
                   dict(op='add_experience', spell=['DEFAULT', 0], e=0, hit=10)]
            sweep = 0 if g == graphics[0] and animation is not None else None
            if g in graphics[3:]:
                # The constructor fails: no spell object exists for the later operations.
                ops = ops[:1]
            yield case('default', ops, [npc(7, combatGraphics=g, magicAnimation=animation), player(), node()]), sweep
    for level in [0, 1, 50, 99, 2 ** 31 - 1]:
        for bonuses in [[0] * 14, [0] * 13 + [8], [0] * 13 + [2 ** 31 - 1], [0] * 13 + [-9], [0] * 13, None]:
            ops = [dict(op='make_default', npc=0), dict(op='max_impact', spell=['DEFAULT', 0], e=0, victim=1),
                   dict(op='max_impact', spell=['DEFAULT', 0], e=1, victim=0)]
            yield case('default_impact', ops, [npc(levels={MAGIC: level}, bonuses=bonuses), player(magic=level, bonuses=bonuses)]), None
    # A player with a default spell as its autocast; the spell id is 0.
    ops = [dict(op='make_default', npc=1), dict(op='set', e=0, fields=dict(autocast=['DEFAULT', 0])),
           dict(op='visualize', spell=['DEFAULT', 0], e=0, target=1), dict(op='meets', spell=['DEFAULT', 0], e=0, message=True, remove=True)]
    yield case('default', ops, [player(), npc(7)]), None


def cases():
    for source in [static_cases, type_cases, staff_cases, rune_cases, meets_cases, removal_cases, experience_cases, visual_cases,
                   impact_cases, target_cases, cast_cases, cast_spell_cases, book_cases, block_cases, autocast_cases, default_cases]:
        yield from source()


def groups(cases):
    out = {}
    for c in cases:
        out[c['group']] = out.get(c['group'], 0) + 1
    return dict(sorted(out.items()))
