"""The designed prayer corpus: every case is (case, sweep), where sweep names the
operation whose recorded calls each fail once in turn (None for no sweep)."""
import itertools
import random
import struct

TYPES = ['THICK_SKIN', 'BURST_OF_STRENGTH', 'CLARITY_OF_THOUGHT', 'SHARP_EYE', 'MYSTIC_WILL', 'ROCK_SKIN', 'SUPERHUMAN_STRENGTH',
         'IMPROVED_REFLEXES', 'RAPID_RESTORE', 'RAPID_HEAL', 'PROTECT_ITEMS', 'HAWK_EYE', 'MYSTIC_LORE', 'STEEL_SKIN', 'ULTIMATE_STRENGTH',
         'INCREDIBLE_REFLEXES', 'PROTECT_FROM_SUMMONING', 'PROTECT_FROM_MAGIC', 'PROTECT_FROM_MISSILES', 'PROTECT_FROM_MELEE', 'EAGLE_EYE',
         'MYSTIC_MIGHT', 'RETRIBUTION', 'REDEMPTION', 'SMITE', 'CHIVALRY', 'PIETY']
LEVEL = dict(zip(TYPES, [1, 4, 7, 8, 9, 10, 13, 16, 19, 22, 25, 26, 27, 28, 31, 34, 35, 37, 40, 43, 44, 45, 46, 49, 52, 60, 70]))
BUTTON = dict(zip(TYPES, [5, 7, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35, 53, 37, 39, 41, 43, 45, 47, 49, 51, 55, 57]))
DEFENCE_REQ = {'CHIVALRY': 65, 'PIETY': 70}
STYLES = ['MELEE', 'RANGE', 'MAGIC']
PROTECT = ['PROTECT_FROM_MELEE', 'PROTECT_FROM_MISSILES', 'PROTECT_FROM_MAGIC', 'PROTECT_FROM_SUMMONING']
OVERHEAD = ['RETRIBUTION', 'REDEMPTION', 'SMITE']


def hx(value):
    return struct.pack('>d', value).hex()


def bonuses(prayer=0, length=15):
    b = [0] * length
    if length > 12:
        b[12] = prayer
    return b


def player(prayer=99, defence=99, points=99.0, **extra):
    spec = dict(kind='player', location=[3200, 3200, 0], static={'5': prayer, '1': defence}, points=hx(points), lifepoints=99,
                bonuses=bonuses(), kingsRansom=True, varbit3909=8)
    spec['static']['3'] = extra.pop('hitpoints', 99)
    spec.update(extra)
    return spec


def npc(location=(3201, 3200, 0), protect=None, **extra):
    spec = dict(kind='npc', location=list(location) if location is not None else None, protect=protect)
    spec.update(extra)
    return spec


def case(group, ops, entities=None, seed=0, **extra):
    c = dict(group=group, seed=seed, entities=entities or [player()], ops=ops)
    c.update(extra)
    return c


def on(*types):
    return [dict(op='toggle', type=t) for t in types]


def state_ops():
    return [dict(op='has_icon')] + [dict(op='skill_bonus', skill=s) for s in [0, 1, 2, 4, 6]]


def table_cases():
    ops = [dict(op='table'), dict(op='melee_types'), dict(op='range_types'), dict(op='magic_types')]
    for ids in [[], [0], [1], [2], [4], [6], [5], [0, 2], [2, 0], [0, 0], [1, 2, 0], [4, 6], [3, 23], [-1]]:
        ops.append(dict(op='by_bonus', ids=ids))
    for button in list(range(-2, 61)) + [100, 271, 2 ** 31 - 1, -2 ** 31]:
        ops.append(dict(op='for_button', button=button))
    for t in TYPES:
        ops.append(dict(op='icon', type=t))
        ops.append(dict(op='get', type=t))
    yield case('table', ops), None


def level_cases():
    """Every prayer below, at and above its prayer level, and Chivalry/Piety around their defence requirement."""
    for t in TYPES:
        level = LEVEL[t]
        for prayer in sorted({max(level - 1, 0), level, level + 1, 99}):
            ops = on(t) + [dict(op='get', type=t)] + on(t) + [dict(op='get', type=t)]
            sweep = 0 if prayer == level else None
            yield case('level', ops, [player(prayer=prayer, defence=99)]), sweep
        if t in DEFENCE_REQ:
            req = DEFENCE_REQ[t]
            for defence in [req - 1, req, req + 1, 1]:
                for prayer in [level - 1, level]:
                    yield case('defence', on(t) + [dict(op='permitted', type=t)], [player(prayer=prayer, defence=defence)]), None
            for ransom, varbit in [(False, 0), (False, 8), (True, 0), (True, 8), (True, 7), (True, 9)]:
                ops = on(t) + on(t) + on(t)
                yield case('kings-ransom', ops, [player(kingsRansom=ransom, varbit3909=varbit)]), 0 if (ransom and varbit == 0) else None
    for points in [0.0, -0.0, 5e-324, -1.0, 0.5, 1.0, float('nan')]:
        yield case('points-gate', on('THICK_SKIN', 'PIETY') + [dict(op='permitted', type='PIETY')], [player(points=points)]), None
    for prayer, defence in [(0, 0), (1, 0), (0, 1), (69, 69), (70, 69), (69, 70), (59, 64), (60, 64)]:
        ops = [dict(op='permitted', type=t) for t in ['PIETY', 'CHIVALRY', 'THICK_SKIN', 'SMITE']]
        yield case('permitted', ops, [player(prayer=prayer, defence=defence)]), 1 if (prayer, defence) == (59, 64) else None


def conflict_cases():
    """Every ordered pair, the first already on; a sample of pairs sweeps the second toggle."""
    for k, (a, b) in enumerate(itertools.product(TYPES, TYPES)):
        ops = on(a, b) + state_ops()
        yield case('pair', ops), 1 if k % 9 == 4 else None
    for a, b, c in [('PIETY', 'EAGLE_EYE', 'MYSTIC_MIGHT'), ('THICK_SKIN', 'BURST_OF_STRENGTH', 'CLARITY_OF_THOUGHT'),
                    ('STEEL_SKIN', 'ULTIMATE_STRENGTH', 'CHIVALRY'), ('SHARP_EYE', 'ROCK_SKIN', 'PIETY'), ('MYSTIC_LORE', 'IMPROVED_REFLEXES', 'HAWK_EYE'),
                    ('PROTECT_FROM_SUMMONING', 'SMITE', 'PROTECT_FROM_MELEE'), ('REDEMPTION', 'PROTECT_FROM_SUMMONING', 'RETRIBUTION')]:
        yield case('triple', on(a, b, c) + state_ops()), 2
    # The widest single activation: many actives each listed once per conflicting bonus pair.
    yield case('flag', on('THICK_SKIN', 'BURST_OF_STRENGTH', 'CLARITY_OF_THOUGHT', 'RAPID_HEAL', 'PROTECT_ITEMS', 'PROTECT_FROM_SUMMONING',
                          'PROTECT_FROM_MELEE', 'SHARP_EYE') + on('PIETY') + state_ops()), 8
    for t in TYPES:
        ops = [dict(op='set', e=0, fields={})] + on('PROTECT_ITEMS', 'RAPID_HEAL') + [dict(op='flag', type=t), dict(op='type_toggle', type=t, on=False),
                                                                                   dict(op='type_toggle', type=t, on=True)] + state_ops()
        yield case('direct', ops), 3 if t in ('PIETY', 'PROTECT_FROM_SUMMONING') else None


def sequence_cases():
    rng = random.Random(530)
    for n in range(240):
        ops = []
        for _ in range(rng.randint(3, 9)):
            r = rng.random()
            if r < 0.6:
                ops.append(dict(op='toggle', type=rng.choice(TYPES)))
            elif r < 0.8:
                ops.append(dict(op='button', button=rng.choice(list(BUTTON.values()) + [0, 1, 3, 59, 2, 54])))
            elif r < 0.9:
                ops.append(dict(op='icon', type=rng.choice(PROTECT + OVERHEAD)))
            else:
                ops.append(dict(op='type_toggle', type=rng.choice(TYPES), on=rng.random() < 0.5))
        ops += state_ops()
        prayer = rng.choice([99, 99, 45, 30, 70, 43])
        yield case('sequence', ops, [player(prayer=prayer, defence=rng.choice([99, 1, 70]))]), (len(ops) - 7) if n % 6 == 0 else None


def icon_cases():
    for combo in itertools.chain.from_iterable(itertools.permutations(PROTECT + OVERHEAD, r) for r in (1, 2, 3)):
        ops = on(*combo) + [dict(op='icon', type=t) for t in PROTECT + OVERHEAD] + [dict(op='has_icon')]
        ops += [dict(op='toggle', type=combo[0])]
        yield case('icons', ops), (len(combo) - 1) if len(combo) == 2 and 'PROTECT_FROM_SUMMONING' in combo else None
    for start in [-1, 0, 7, 3, 99]:
        for t in PROTECT + ['THICK_SKIN']:
            for active in [[], ['PROTECT_FROM_SUMMONING'], ['PROTECT_FROM_SUMMONING', 'PROTECT_FROM_MAGIC'], ['PROTECT_FROM_MELEE']]:
                ops = [dict(op='set', e=0, fields={'headIcon': start})] + on(*active) + [dict(op='set', e=0, fields={'headIcon': start}),
                                                                                        dict(op='find_next_icon', type=t)]
                yield case('next-icon', ops), (len(ops) - 1) if t == 'PROTECT_FROM_SUMMONING' and start == 7 else None


def drain_cases():
    """Drain over tick runs with prayer bonuses at the boundaries until points run out."""
    sets = [['THICK_SKIN'], ['RAPID_RESTORE'], ['RAPID_HEAL'], ['PROTECT_FROM_MELEE'], ['PIETY'], ['PROTECT_FROM_SUMMONING'],
            ['PIETY', 'PROTECT_FROM_MAGIC'], ['STEEL_SKIN', 'ULTIMATE_STRENGTH', 'INCREDIBLE_REFLEXES', 'PROTECT_ITEMS', 'RAPID_HEAL', 'RAPID_RESTORE'],
            ['EAGLE_EYE', 'SMITE', 'PROTECT_ITEMS'], ['MYSTIC_MIGHT', 'REDEMPTION', 'RAPID_HEAL', 'PROTECT_ITEMS', 'ROCK_SKIN']]
    for bonus in [-31, -30, -29, -15, -1, 0, 1, 2, 15, 29, 30, 31, 60, 2 ** 31 - 1, -2 ** 31]:
        for k, active in enumerate(sets):
            ops = on(*active) + [dict(op='tick')] * 60
            yield case('drain', ops, [player(points=6.0, bonuses=bonuses(bonus))]), (len(active) + 1) if (k == 6 and bonus in (0, 30)) else None
    for points, active, ticks in [(1.0, ['THICK_SKIN'], 50), (0.3, ['PIETY'], 12), (0.05, ['THICK_SKIN'], 8), (0.6, ['PROTECT_FROM_MELEE'], 12),
                                  (2.5, ['PIETY', 'PROTECT_FROM_MELEE'], 30), (99.0, ['PIETY', 'PROTECT_FROM_MELEE', 'RAPID_HEAL'], 700),
                                  (99.0, ['RAPID_RESTORE'], 160), (12.0, ['SMITE', 'PIETY'], 90), (5e-324, ['THICK_SKIN'], 6), (0.0, [], 6)]:
        for bonus in [0, 12]:
            ops = on(*active) + [dict(op='tick')] * ticks + [dict(op='pulse'), dict(op='tick'), dict(op='pulse')] + on(*active) + [dict(op='tick')] * 4
            yield case('run-out', ops, [player(points=points, bonuses=bonuses(bonus))]), None
    # Points reaching exactly zero, a toggle at an odd tick, a deactivation restarting the count.
    ops = on('THICK_SKIN') + [dict(op='tick')] * 3 + on('PIETY') + [dict(op='tick')] * 3 + on('PIETY') + [dict(op='tick')] * 2 + on('THICK_SKIN') + \
        [dict(op='tick')] * 3 + on('ROCK_SKIN') + [dict(op='tick')] * 4
    yield case('tick-count', ops, [player(points=20.0)]), None
    ops = on('PROTECT_FROM_MELEE') + [dict(op='set', e=0, fields={'points': hx(0.0)})] + [dict(op='tick')] * 2 + [dict(op='pulse')] + [dict(op='tick')] * 3
    yield case('zero', ops), 3
    for active in [['THICK_SKIN'], ['PIETY', 'PROTECT_FROM_MAGIC', 'RAPID_HEAL']]:
        for b in [None, [0] * 12, [0] * 13, bonuses(7, 20)]:
            ops = on(*active) + [dict(op='tick')] * 4
            yield case('bonus-array', ops, [player(bonuses=b)]), 2 if b is not None and len(b) == 13 else None
    for seed in range(12):
        ops = on('PIETY', 'PROTECT_FROM_MELEE') + [dict(op='tick')] * 40
        yield case('divine-favor', ops, [player(perks=['DIVINE_FAVOR'], points=30.0)], seed=seed), 3 if seed == 0 else None
    ops = on('PIETY') + [dict(op='tick')] * 2 + [dict(op='set', e=0, fields={'perks': ['DIVINE_FAVOR']})] + [dict(op='tick')] * 6
    yield case('divine-favor', ops, seed=99), None
    ops = on('PIETY') + [dict(op='tick')] * 1999
    yield case('long-run', ops, [player(points=99.0)]), None


def reset_cases():
    for active in [[], ['THICK_SKIN'], ['PIETY', 'PROTECT_FROM_MELEE', 'RAPID_HEAL'], ['PROTECT_FROM_SUMMONING', 'PROTECT_FROM_MAGIC'], ['SMITE']]:
        yield case('reset', on(*active) + [dict(op='reset'), dict(op='reset'), dict(op='pulse'), dict(op='pulse')] + state_ops()), len(active)
        yield case('full-restore', on(*active) + [dict(op='full_restore'), dict(op='pulse')]), len(active)
        for playing in [True, False]:
            for killer in [1, None, 0]:
                ents = [player(playing=playing), npc()]
                yield case('commence-death', on(*active, 'RETRIBUTION') + [dict(op='commence_death', killer=killer)], ents), None
            yield case('commence-death', on(*active) + [dict(op='commence_death', killer=1)], [player(playing=playing), npc()]), None
    yield case('commence-death', on('RETRIBUTION') + [dict(op='commence_death', killer=1)], [player(), npc()]), 1


def redemption_cases():
    for prayer in [0, 1, 3, 4, 7, 8, 49, 50, 99, 255]:
        for lifepoints, hitpoints, perks in [(1, 99, []), (99, 99, []), (90, 99, ['DAMAGE_SPONG']), (99, 99, ['DAMAGE_SPONG']), (2147483647, 99, []),
                                             (5, 10, [])]:
            ents = [player(prayer=prayer, hitpoints=hitpoints, lifepoints=lifepoints, perks=perks)]
            ops = on('REDEMPTION', 'PIETY') + [dict(op='redemption'), dict(op='pulse')]
            yield case('redemption', ops, ents), 2 if (prayer, lifepoints) in ((99, 90), (8, 1)) else None


def retribution_cases():
    near = [(3201, 3200, 0), (3200, 3201, 0), (3201, 3201, 0), (3199, 3199, 0), (3202, 3200, 0), (3201, 3200, 1), (3200, 3200, 0)]
    for prayer in [0, 3, 4, 5, 8, 9, 46, 99]:
        for k, at in enumerate(near):
            ents = [player(prayer=prayer), npc(location=at)]
            yield case('retribution', [dict(op='retribution', killer=1)], ents, seed=prayer * 10 + k), 0 if (prayer, k) in ((99, 0), (4, 2)) else None
    yield case('retribution', [dict(op='retribution', killer=0)], [player()]), 0
    yield case('retribution', [dict(op='retribution', killer=None)], [player()]), None
    yield case('retribution', [dict(op='retribution', killer=1)], [player(), npc(location=None)]), None
    yield case('retribution', [dict(op='retribution', killer=1)], [player(location=None), npc()]), None
    yield case('retribution', [dict(op='retribution', killer=1)], [player(location=[3200, 3200, 3]), npc(location=(3200, 3201, 3))]), None
    # Multi-way: NPC killers hit nearby NPCs, player killers nearby players; the player and the killer are passed over.
    for multi in [False, True]:
        for seed in range(3):
            ents = [player(multi=multi, prayer=99), npc(), npc(location=(3199, 3200, 0)), npc(location=(3200, 3199, 0), attackable=False),
                    player(location=[3201, 3201, 0]), player(location=[3199, 3201, 0], attackable=False)]
            yield case('retribution-multi', [dict(op='retribution', killer=1)], ents, seed=seed), 0 if (multi and seed == 0) else None
            yield case('retribution-multi', [dict(op='retribution', killer=4)], ents, seed=seed), 0 if (multi and seed == 1) else None
            yield case('retribution-multi', [dict(op='retribution', killer=0)], ents, seed=seed), None
            order = dict(localNpcs=[3, 1, 2, 2], localPlayers=[5, 0, 4, 4])
            yield case('retribution-multi', [dict(op='retribution', killer=1), dict(op='retribution', killer=4)], ents, seed=seed, **order), None


def button_cases():
    for button in sorted(set(BUTTON.values()) | {-1, 0, 1, 2, 3, 4, 6, 52, 54, 56, 58, 59, 60, 271}):
        for prayer in [1, 99]:
            ops = [dict(op='button', button=button), dict(op='button', button=button)] + state_ops()
            yield case('button', ops, [player(prayer=prayer)]), 0 if (prayer == 99 and button in (41, 57)) else None
    yield case('button', [dict(op='button', button=5)], [player(points=0.0)]), None


def combat_cases():
    """The reads the combat slices ask for, answered by the same state."""
    actives = [[], ['PROTECT_FROM_MELEE'], ['PROTECT_FROM_MISSILES'], ['PROTECT_FROM_MAGIC'], ['PROTECT_FROM_SUMMONING'],
               ['PROTECT_FROM_SUMMONING', 'PROTECT_FROM_MAGIC'], ['SMITE'], ['REDEMPTION'], ['PIETY', 'PROTECT_FROM_MELEE']]
    for index, active in enumerate(actives):
        ents = [player(), player(location=[3201, 3200, 0]), npc(protect='MELEE'), npc(protect=None), npc(protect='MAGIC')]
        ops = [dict(op='toggle', p=1, type=t) for t in active]
        for e in range(5):
            for s in STYLES + [None]:
                ops.append(dict(op='protection', e=e, style=s))
        for t in ['SMITE', 'REDEMPTION', 'PROTECT_FROM_SUMMONING']:
            ops.append(dict(op='get', p=1, type=t))
        hits = [0, 1, 2, 3, 4, 5, 7, 10, 99, -1, -7, 2 ** 31 - 1, -2 ** 31] if index < 5 else [0, 1, 5, -1]
        for attacker in [0, 2, None]:
            for victim in [1, 2, 3, None]:
                for s in STYLES + [None]:
                    for armour in [None, 'VERAC', 'DHAROK']:
                        for hit in hits:
                            ops.append(dict(op='formatted_hit', on=0, attacker=attacker, victim=victim, style=s, armour=armour, hit=hit))
        yield case('protection', ops, ents), None
    levels = list(range(1, 21)) + [33, 34, 35, 39, 40, 41, 59, 60, 61, 66, 67, 79, 80, 81, 98, 99, 100, 112, 118, 120, 127, 255, 0, -1, -5]
    bonus_sets = [[], ['THICK_SKIN'], ['ROCK_SKIN'], ['STEEL_SKIN'], ['BURST_OF_STRENGTH'], ['SUPERHUMAN_STRENGTH'], ['ULTIMATE_STRENGTH'],
                  ['CLARITY_OF_THOUGHT'], ['IMPROVED_REFLEXES'], ['INCREDIBLE_REFLEXES'], ['SHARP_EYE'], ['HAWK_EYE'], ['EAGLE_EYE'], ['MYSTIC_WILL'],
                  ['MYSTIC_LORE'], ['MYSTIC_MIGHT'], ['CHIVALRY'], ['PIETY'], ['STEEL_SKIN', 'ULTIMATE_STRENGTH', 'INCREDIBLE_REFLEXES'],
                  ['THICK_SKIN', 'SUPERHUMAN_STRENGTH', 'CLARITY_OF_THOUGHT'], ['ROCK_SKIN', 'BURST_OF_STRENGTH', 'IMPROVED_REFLEXES']]
    for active in bonus_sets:
        ops = on(*active) + [dict(op='effective', skill=s, level=level) for s in [0, 1, 2, 4, 6] for level in levels]
        yield case('multiplier', ops), None


def cases():
    out = []
    for source in [table_cases, level_cases, conflict_cases, sequence_cases, icon_cases, drain_cases, reset_cases, redemption_cases,
                   retribution_cases, button_cases, combat_cases]:
        out.extend(source())
    return out


def groups(all_cases):
    counts = {}
    for c in all_cases:
        counts[c['group']] = counts.get(c['group'], 0) + 1
    return dict(sorted(counts.items()))
