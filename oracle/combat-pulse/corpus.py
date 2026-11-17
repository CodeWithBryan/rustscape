"""The designed combat-pulse corpus: world inputs plus operations per case.

A case is (input, sweep): `sweep` names one operation whose every service call
and draw fails once in a second pass. Nothing here is a port; entity fields
are answers the recording stand-ins give.
"""
import itertools
import struct

MARKERS = ('tick', 'update', 'runnerError')
SETS = dict(
    DHAROK=['4716', '4718', '4720', '4722'], GUTHAN=['4724', '4726', '4728', '4730'], VERAC=['4753', '4755', '4757', '4759'],
    TORAG=['4745', '4747', '4749', '4751'], BERSERKER_NECKLACE_SETS=['11128', '6523'], AHRIM=['4708', '4710', '4712', '4714'],
    KARIL=['4732', '4734', '4736', '4738'])
PREVENT = dict(N=0x12c0120, E=0x12c0180, NE=0x12c01e0, S=0x12c0102, SE=0x12c0183, W=0x12c0108, SW=0x12c010e, NW=0x12c0138)
BOW = dict(itemId=841, slot=13, type=0, ammunition=[882, 884], animation=426)
DARK = dict(itemId=11235, slot=13, type=2, ammunition=[882, 11212], animation=426)
CRYSTAL = dict(itemId=4212, slot=13, type=4, ammunition=[], animation=426)
KNIFE = dict(itemId=864, slot=3, type=3, ammunition=[864], animation=806)
CROSSBOW = dict(itemId=9185, slot=13, type=1, ammunition=[9144, 9243], animation=4230, dropAmmo=False)
AMMO = [
    dict(itemId=882, start=[19, 96, 0], darkBow=[1104, 96, 0], projectile=10),
    dict(itemId=884, start=[18, 96, 0], projectile=9, poisonDamage=10),
    dict(itemId=11212, start=[1116, 96, 0], darkBow=[1111, 96, 0], projectile=1120),
    dict(itemId=864, start=[219, 96, 0], projectile=212),
    dict(itemId=9144, projectile=27),
    dict(itemId=9243, projectile=27),
    dict(itemId=4740, start=[20, 96, 0]),
]
RANGE = dict(weapons=[BOW, DARK, CRYSTAL, KNIFE, CROSSBOW], ammo=AMMO)


def H(value):
    return struct.pack('>d', float(value)).hex()


def P(**fields):
    return dict(kind='player', **fields)


def N(**fields):
    return dict(kind='npc', **fields)


def at(x, y, z=0):
    return [3200 + x, 3200 + y, z]


def levels(**named):
    ids = dict(attack=0, defence=1, strength=2, hitpoints=3, range=4, prayer=5, magic=6, agility=16)
    return {str(ids[name]): value for name, value in named.items()}


def done(**fields):
    """A player past the tutorial, so impacts apply."""
    attrs = dict(fields.pop('attrs', {}))
    attrs.setdefault('tutorial:complete', True)
    return P(attrs=attrs, **fields)


def archer(weapon=841, ammo=882, amount=5, style=(4, 4), **fields):
    equipment = {'3': [weapon, 1, 1000, 'bow']}
    if ammo is not None:
        equipment['13'] = [ammo, amount, 1000, 'ammo']
    equipment.update(fields.pop('equipment', {}))
    made = dict(equipment=equipment, attackStyle=list(style), handler=1, weaponInterface='UNARMED')
    made.update(fields)
    return done(**made)


def cases():
    # Two WeaponInterfaces names: one whose interface id is 91 and one whose is not.
    short, other = 'THROWN_WEAPONS', 'BOW'
    rows = []

    def add(entities, ops, sweep=None, **extra):
        rows.append((dict(entities=entities, ops=ops, **extra), sweep))

    def each(entities, ops, sweep=False, **extra):
        """One case per operation, for operations that change state."""
        for op in ops:
            add(entities, [op], 0 if sweep else None, **extra)

    add([], [dict(op='tables')])
    reach(add, short, other)
    adjust(add, each)
    experience(add)
    ammunition(add, each)
    visuals(add, each)
    impacts(add, each)
    impact_handler(add, each)
    pulses(add, each, short)
    fights(add, short)
    return rows


def offsets(radius):
    return [(x, y) for x in range(-radius, radius + 1) for y in range(-radius, radius + 1)]


def reach(add, short, other):
    grid = offsets(3)

    def sweep_of(maker, entities):
        ops = []
        for dx, dy in grid:
            ops.append(dict(op='set', e=1, key='location', value=at(dx, dy)))
            ops.append(maker())
        add(entities, ops)

    melee = lambda: dict(op='canSwing', h=0, e=0, v=1)
    sweep_of(melee, entities=[P(), N()])
    sweep_of(melee, entities=[P(equipment={'3': [3190, 1, 1000, 'Bronze halberd']}), N()])
    sweep_of(melee, entities=[P(equipment={'3': [6599, 1, 1000, 'White halberd']}), N(size=2)])
    sweep_of(melee, entities=[P(), N(moving=True)])
    sweep_of(melee, entities=[P(runningBoth=True, runDir=2), N(moving=True, runDir=3)])
    sweep_of(melee, entities=[P(moving=True, runDir=1), N()])
    sweep_of(melee, entities=[P(moving=True), N(runDir=4, moving=True)])
    sweep_of(melee, entities=[P(), N(size=2)])
    sweep_of(melee, entities=[P(), N(size=3)])
    sweep_of(melee, entities=[N(size=2), P()])
    sweep_of(melee, entities=[N(size=3, id=8612), P(moving=True)])
    sweep_of(melee, entities=[N(id=7135), P()])
    sweep_of(melee, entities=[P(equipment={'3': [1277, 1, 1000, 'Sword']}), P()])
    sweep_of(lambda: dict(op='canMelee', e=0, v=1, distance=1), entities=[P(), N()])
    sweep_of(lambda: dict(op='canMelee', e=0, v=1, distance=2), entities=[N(size=2), N(size=2)])
    sweep_of(lambda: dict(op='canMelee', e=0, v=1, distance=3), entities=[P(), N(size=3)])
    sweep_of(lambda: dict(op='canMelee', e=0, v=1, distance=1), entities=[N(size=3), P()])
    sweep_of(lambda: dict(op='canStepTowards', h=0, e=0, v=1), entities=[N(size=2), N(size=3)])
    sweep_of(lambda: dict(op='isAttackable', h=0, e=0, v=1), entities=[P(), N(size=2)])
    sweep_of(lambda: dict(op='isAttackable', h=1, e=0, v=1), entities=[N(size=2), P()])
    sweep_of(lambda: dict(op='isAttackable', h=2, e=0, v=1), entities=[P(), P()])
    # The victim is fighting the attacker: index and client index order decide adjacency and overlap.
    for first, second in [(1, 2), (2, 1), (5, 5)]:
        for victim_handler in [0, 1]:
            ops = [dict(op='setVictim', e=1, v=0)]
            for dx, dy in grid:
                ops += [dict(op='set', e=1, key='location', value=at(dx, dy)), dict(op='canSwing', h=0, e=0, v=1)]
            add([P(index=first, clientIndex=first), P(index=second, clientIndex=second, handler=victim_handler)], ops)
    # A victim standing inside a larger attacker is neither adjacent nor on its tile: the index order decides.
    for first, second, handler in itertools.product([1, 2], [1, 2], [0, 1]):
        for dx, dy in [(1, 0), (0, 1), (1, 1), (0, 0)]:
            add([N(size=2, index=first), P(index=second, handler=handler, location=at(dx, dy))],
                [dict(op='canMelee', e=0, v=1, distance=1), dict(op='setVictim', e=1, v=0), dict(op='canMelee', e=0, v=1, distance=1),
                 dict(op='canSwing', h=0, e=0, v=1)], 2 if (first, second, handler, dx) == (1, 2, 0, 1) else None)
    # A wall on the tile stepped onto, on either corner tile, and on the projectile path.
    for (dx, dy), name in zip([(0, 1), (1, 0), (0, -1), (-1, 0), (1, 1), (-1, 1), (1, -1), (-1, -1)], ['N', 'E', 'S', 'W', 'NE', 'NW', 'SE', 'SW']):
        tiles = {(dx, dy), (dx, 0), (0, dy), (2 * dx, 2 * dy), (2 * dx, dy), (dx, 2 * dy)}
        for tile in sorted(tiles):
            for flag in sorted({PREVENT[name], PREVENT['E'], PREVENT['S'], 0x20000}):
                key = '%d,%d,0' % (3200 + tile[0], 3200 + tile[1])
                ops = [dict(op='canStepTowards', h=0, e=0, v=1), dict(op='canSwing', h=0, e=0, v=1),
                       dict(op='projectile', e=0, v=1, close=True), dict(op='projectile', e=0, v=1, close=False)]
                add([P(), N(location=at(2 * dx, 2 * dy))], ops, clip={key: flag})
                add([P(equipment={'3': [3190, 1, 1000, 'Halberd']}), N(location=at(2 * dx, 2 * dy))], ops[1:], pclip={key: flag})
    # Long and oblique approaches: the skipped gap, the non-converging walk, a blocked last step.
    far = [(5, 0), (0, 8), (7, 7), (5, 2), (10, 3), (-9, 4), (3, -11), (-6, -6), (4, 1), (2, 3), (12, 12), (-1, 5), (3, 3), (-3, 4)]
    for dx, dy in far:
        for size in [1, 2]:
            for blocked in [None, 0x12c0120, 0x12c0180, 0x20000]:
                clip = {}
                if blocked is not None:
                    for x in range(-2, 3):
                        for y in range(-2, 3):
                            clip['%d,%d,0' % (3200 + dx + x, 3200 + dy + y)] = blocked
                add([P(size=size), N(location=at(dx, dy), size=size)],
                    [dict(op='canStepTowards', h=0, e=0, v=1), dict(op='canSwing', h=0, e=0, v=1), dict(op='projectile', e=0, v=1, close=False)],
                    clip=clip, pclip=clip)
    # isAttackable's own branches.
    spells = [dict(spellId=0), dict(spellId=7)]
    for attrs, autocast, interface, handler in itertools.product(
            [{}, {'autocast_component': True}], [None, 0, 1], [None, other], [0, 2]):
        entity = P(attrs=attrs, handler=handler)
        if autocast is not None:
            entity['autocast'] = autocast
        if interface is not None:
            entity['weaponInterface'] = interface
        add([entity, N(location=at(1, 0))], [dict(op='isAttackable', h=handler, e=0, v=1), dict(op='isAttackable', h=handler, e=0, v=1)],
            1 if attrs and autocast is None and interface else None, spells=spells)
    add([N(attrs={'autocast_component': True}), N(location=at(1, 0))], [dict(op='isAttackable', h=2, e=0, v=1)])
    for victim in [N(attrs={'return-to-spawn': True}, location=at(1, 0)), N(attrs={'return-to-spawn': False}, location=at(1, 0)),
                   N(location=at(0, 0, 1)), N(location=at(1, 0, 1)), N(location=None), N(location=at(-1, -1), size=2),
                   N(location=at(-2, -2), size=3), N(location=at(0, 0), size=0)]:
        for entity in [P(), P(location=None), N(size=2)]:
            for h in [0, 1, 2]:
                add([entity, victim], [dict(op='isAttackable', h=h, e=0, v=1), dict(op='canSwing', h=h, e=0, v=1)])
    # Range reach along a line and a diagonal.
    for entity in [archer(weaponInterface=short), archer(weaponInterface=other), archer(weaponInterface=other, style=(6, 4)),
                   archer(weaponInterface=short, style=(6, 4)), archer(weapon=11235, weaponInterface=other),
                   archer(weapon=4212, ammo=None, weaponInterface=short, style=(6, 4)), archer(weapon=1277, weaponInterface=other),
                   N(handler=1), N(handler=1, combatDistance=3), N(handler=1, combatDistance=12, size=3)]:
        for victim in [N(), N(moving=True), N(size=3), P(moving=True, size=1)]:
            ops = []
            for step in [1, 4, 5, 6, 7, 8, 9, 10, 11, 12]:
                for dx, dy in [(step, 0), (-step, step // 2)]:
                    ops += [dict(op='set', e=1, key='location', value=at(dx, dy)), dict(op='canSwing', h=1, e=0, v=1)]
            add([entity, victim], ops, range=RANGE)
    add([archer(weaponInterface=None), N(location=at(3, 0))], [dict(op='canSwing', h=1, e=0, v=1)], 0, range=RANGE)
    add([archer(weaponInterface='null'), N(location=at(3, 0))], [dict(op='canSwing', h=1, e=0, v=1)], range=RANGE)
    add([archer(weaponInterface=short, attackStyle=None), N(location=at(3, 0))], [dict(op='canSwing', h=1, e=0, v=1)], range=RANGE)
    add([archer(weaponInterface=short), N(location=at(4, 2))], [dict(op='canSwing', h=1, e=0, v=1)], 0, range=RANGE)
    add([archer(weaponInterface=short), N(location=at(8, 0), moving=True)], [dict(op='canSwing', h=1, e=0, v=1)], 0, range=RANGE)
    bad = dict(weapons=[dict(itemId=841, slot=13, type=9, ammunition=[882])], ammo=AMMO)
    add([archer(weaponInterface=short), N(location=at(4, 2))], [dict(op='canSwing', h=1, e=0, v=1)], range=bad)
    # Magic reach.
    for entity in [P(handler=2, autocast=0), N(handler=2, combatDistance=4), N(handler=2, size=2)]:
        for victim in [N(), N(moving=True), P(size=2), N(location=None)]:
            ops = []
            for step in range(7, 15):
                for dx, dy in [(step, 0), (step, step), (0, -step)]:
                    if victim.get('location', 1) is not None:
                        ops.append(dict(op='set', e=1, key='location', value=at(dx, dy)))
                    ops.append(dict(op='canSwing', h=2, e=0, v=1))
            add([entity, victim], ops, spells=[dict(spellId=3)])
    add([P(handler=2, autocast=0), N(location=at(6, 3), moving=True)], [dict(op='canSwing', h=2, e=0, v=1)], 0, spells=[dict(spellId=3)])
    add([P(), N(location=at(1, 0))], [dict(op='canSwing', h=0, e=0, v=1)], 0)
    add([P(moving=True, runDir=1), N(location=at(2, 0), moving=True)], [dict(op='canSwing', h=0, e=0, v=1)], 0)
    add([N(size=2), P(location=at(2, 2))], [dict(op='canSwing', h=0, e=0, v=1)], 0)
    # Projectile footprints.
    for a, b in itertools.product([1, 2, 3], [1, 2, 3]):
        for dx, dy in [(4, 0), (3, 3), (-4, 2), (0, 0), (1, 1)]:
            for key in [None, '%d,%d,0' % (3200 + (dx > 0) - (dx < 0), 3200 + (dy > 0) - (dy < 0)), '%d,%d,0' % (3200 + dx, 3200 + dy)]:
                clip = {} if key is None else {key: 0x20000}
                add([N(size=a), N(size=b, location=at(dx, dy))],
                    [dict(op='projectile', e=0, v=1, close=True), dict(op='projectile', e=0, v=1, close=False)], pclip=clip)
    add([N(size=2), N(location=at(3, 1))], [dict(op='projectile', e=0, v=None, close=False)])
    add([N(location=None), N(location=at(3, 1))], [dict(op='projectile', e=0, v=1, close=False)])
    add([N(size=2), N(size=2, location=at(3, 1))], [dict(op='projectile', e=0, v=1, close=True)], 0, clip={'3202,3201,0': 0x20000})


def adjust(add, each):
    hits = [(-1, -1), (0, -1), (1, -1), (5, -1), (14, 3), (15, 15), (16, 40), (7, 0), (200, 100)]
    victims = [
        N(lifepoints=30, maxLifepoints=30), N(id=4474, lifepoints=20), N(id=7891, lifepoints=20), N(id=757, lifepoints=9),
        N(protectStyle='MELEE', lifepoints=50), N(protectStyle='RANGE', lifepoints=50), N(protectStyle='MAGIC', lifepoints=50),
        done(lifepoints=40, equipment={'5': [13742, 1, 1000, 'Elysian']}, prayerPoints=H(20)),
        done(lifepoints=40, equipment={'5': [13740, 1, 1000, 'Divine']}, prayerPoints=H(20)),
        done(lifepoints=40, equipment={'5': [13740, 1, 1000, 'Divine']}, prayerPoints=H(0.5)),
        done(lifepoints=3, equipment={'5': [1201, 1, 1000, 'Kite']}, prayers=['PROTECT_FROM_SUMMONING']),
    ]
    attackers = [
        (0, P()), (0, P(equipment={'3': [1263, 1, 1000, 'Rune dagger(p)']})), (0, P(equipment={'3': [5680, 1, 1000, 'Dragon dagger(p++)']})),
        (0, P(equipment={'3': [5670, 1, 1000, 'Iron dagger(p+)']})), (0, P(equipment={'3': [8872, 1, 1000, 'Bone dagger (s)']})),
        (0, P(equipment={'3': [1249, 1, 1000, 'Spear(kp)']})), (0, P(prayers=['SMITE'], attrs={'1hko': True})),
        (0, P(prayers=['SMITE'], hitFactor=H(0.5))), (0, N(poisonous=True, poisonSeverity=12)), (0, N(poisonous=True)),
        (0, N(poisonSeverity=5)), (0, dict(kind='familiar')), (0, dict(kind='other')),
        (1, P(equipment={'13': [884, 5, 1000, 'Arrow(p)']})), (1, N(poisonous=True, poisonSeverity=0)), (1, dict(kind='familiar', poisonous=True, poisonSeverity=3)),
        (2, P(equipment={'3': [14726, 1, 1000, 'Staff']}, hitFactor=H(1.25))), (2, N()), (2, P(equipment={'3': [1381, 1, 1000, 'Staff of air']})),
    ]
    for (h, attacker), victim in itertools.product(attackers, victims):
        ops = []
        for seed, (estimated, secondary) in enumerate(hits):
            state = dict(attacker=0, victim=1, estimatedHit=estimated, secondaryHit=secondary, style=['MELEE', 'RANGE', 'MAGIC'][h])
            if h == 1 and attacker.get('equipment'):
                state['ammunition'] = 884
            if h == 2:
                state['spell'] = 0
            ops.append(dict(op='adjust', h=h, e=0, v=1, state=state))
        add([attacker, victim], ops, rng=dict(intSeed=len(victim) * 7 + h), range=RANGE, spells=[{}])
    # Every draw of the poison and spirit-shield rolls.
    for draw in range(10):
        add([P(equipment={'3': [1263, 1, 1000, 'Dagger(p)']}), N()],
            [dict(op='adjust', h=0, e=0, v=1, state=dict(attacker=0, victim=1, estimatedHit=4))], rng=dict(ints=[draw]))
        add([N(poisonous=True, poisonSeverity=7), N()],
            [dict(op='adjust', h=1, e=0, v=1, state=dict(attacker=0, victim=1, estimatedHit=0))], rng=dict(ints=[draw]))
    for draw in [0, 70, 71, 99]:
        for hit in [1, 2, 3, 4, 40, 41]:
            add([N(), done(lifepoints=99, equipment={'5': [13742, 1, 1000, 'Elysian']})],
                [dict(op='adjust', h=0, e=0, v=1, state=dict(attacker=0, victim=1, estimatedHit=hit, secondaryHit=hit))], rng=dict(ints=[9, draw, draw]))
    for points in [0.0, 0.14, 0.15, 0.16, 1.0, 5.9, 6.0, 6.1]:
        for hit in [1, 6, 7, 40]:
            add([N(), done(lifepoints=99, equipment={'5': [13740, 1, 1000, 'Divine']}, prayerPoints=H(points))],
                [dict(op='adjust', h=0, e=0, v=1, state=dict(attacker=0, victim=1, estimatedHit=hit))], rng=dict(ints=[9]))
    for points in [0.0, 0.25, 3.0]:
        for hit in [1, 9]:
            add([P(prayers=['SMITE']), done(lifepoints=99, prayerPoints=H(points))],
                [dict(op='adjust', h=0, e=0, v=1, state=dict(attacker=0, victim=1, estimatedHit=hit, secondaryHit=hit))])
    # Scripted sendImpact/checkImpact results are re-read; factors round toward zero.
    for factor in [0.0, 0.5, 0.99, 1.0, 1.5, -1.0]:
        add([P(hitFactor=H(factor), sendImpact=dict(estimatedHit=9)), N(lifepoints=6, checkImpact=dict(secondaryHit=4))],
            [dict(op='adjust', h=0, e=0, v=1, state=dict(attacker=0, victim=1, estimatedHit=3, secondaryHit=-1))])
    # Target arrays: none, only the state itself, children, nulls, nesting.
    child = lambda victim, hit: dict(attacker=0, victim=victim, estimatedHit=hit, spell=0, style='MAGIC')
    shapes = [None, [], ['this'], [child(1, 5)], ['this', child(2, 6)], [None, child(2, 7), None], [child(1, 3), child(2, 9), 'this'],
              [dict(child(2, 4), targets=[child(1, 2), 'this'])], [child(None, 4)], [dict(attacker=0, victim=2, estimatedHit=2)]]
    for h, shape in itertools.product([0, 1, 2], shapes):
        state = dict(attacker=0, victim=1, estimatedHit=8, secondaryHit=2, style='MAGIC')
        if shape is not None:
            state['targets'] = shape
        for with_spell in [True, False]:
            if with_spell:
                state = dict(state, spell=0)
            add([P(equipment={'3': [14726, 1, 1000, 'Staff(p)']}), N(lifepoints=6), N(lifepoints=5, id=757)],
                [dict(op='adjust', h=h, e=0, v=1, state=state)], rng=dict(intSeed=3), spells=[{}], range=RANGE)
    sweeps = [
        (0, P(equipment={'3': [1263, 1, 1000, 'Dagger(p)']}, prayers=['SMITE'], attrs={'1hko': True}),
         done(lifepoints=40, equipment={'5': [13740, 1, 1000, 'Divine']}, prayerPoints=H(20))),
        (0, N(poisonous=True, poisonSeverity=4), done(lifepoints=40, equipment={'5': [13742, 1, 1000, 'Elysian']})),
        (1, P(equipment={'13': [884, 5, 1000, 'Arrow(p)']}), N(id=7891, protectStyle='RANGE')),
        (2, P(equipment={'3': [14726, 1, 1000, 'Staff']}), N(id=4474, lifepoints=3)),
        (2, dict(kind='familiar'), done(prayers=['PROTECT_FROM_SUMMONING'])),
    ]
    for h, attacker, victim in sweeps:
        state = dict(attacker=0, victim=1, estimatedHit=16, secondaryHit=20, style=['MELEE', 'RANGE', 'MAGIC'][h], ammunition=884, spell=0,
                     targets=['this', dict(attacker=0, victim=1, estimatedHit=5, spell=0)])
        add([attacker, victim], [dict(op='adjust', h=h, e=0, v=1, state=state)], 0, rng=dict(intSeed=5), range=RANGE, spells=[{}])


def experience(add):
    damage = [dict(estimatedHit=0), dict(estimatedHit=1), dict(estimatedHit=7, secondaryHit=3), dict(estimatedHit=-4, secondaryHit=-1),
              dict(estimatedHit=99, targets=[dict(estimatedHit=1), 'this', dict(estimatedHit=30, secondaryHit=2)]), 'none']
    victims = [N(), N(xpMultiplier=H(0.5)), N(xpMultiplier=H(2.5)), P(), None, dict(kind='familiar', xpMultiplier=H(0.0))]
    for style, victim in itertools.product(range(10), victims):
        entities = [P(attackStyle=[style, 2])] + ([victim] if victim else [])
        ops = []
        for mod in [4.0, 0.1, 0.01]:
            ops.append(dict(op='setXpMod', mod=H(mod)))
            for state in damage:
                ops.append(dict(op='addXp', h=0, e=0, v=1 if victim else None, state=state))
        add(entities, ops)
    # Repeated grants without an adjust in between keep multiplying the shared modifier.
    add([P(attackStyle=[1, 2]), N(xpMultiplier=H(1.5))], [dict(op='addXp', h=0, e=0, v=1, state=dict(estimatedHit=10))] * 5)
    for h in [0, 1, 2]:
        for attacker in [P(attackStyle=[8, 3]), P(attackStyle=[7, 3], ironman=True), P(attackStyle=None), N(), dict(kind='other'),
                         dict(kind='familiar', owner=2, famAttackStyle=6), dict(kind='familiar', owner=None, famAttackStyle=1),
                         dict(kind='familiar', owner=2, famAttackStyle=2), dict(kind='familiar', owner=2, famAttackStyle=4)]:
            for victim in [N(xpMultiplier=H(3.0)), P(), P(ironman=True)]:
                for state in [dict(estimatedHit=12, secondaryHit=5), dict(estimatedHit=12, spell=0), 'none']:
                    add([attacker, victim, P(attackStyle=[3, 0])], [dict(op='addXp', h=h, e=0, v=1, state=state)], spells=[{}])
    add([P()], [dict(op='addXp', h=0, e=None, v=None, state=dict(estimatedHit=3))])
    for style in [2, 6, 7, 0]:
        add([P(attackStyle=[style, 1]), N(xpMultiplier=H(1.25))], [dict(op='addXp', h=0, e=0, v=1, state=dict(estimatedHit=9))], 0)
    add([dict(kind='familiar', owner=2, famAttackStyle=5), N(), P()], [dict(op='addXp', h=1, e=0, v=1, state=dict(estimatedHit=9))], 0)
    add([P(ironman=True), P()], [dict(op='addXp', h=0, e=0, v=1, state=dict(estimatedHit=9))], 0)
    add([P(), N()], [dict(op='addXp', h=2, e=0, v=1, state=dict(estimatedHit=9, spell=0))], 0, spells=[{}])


def ammunition(add, each):
    def weapon(kind, slot=13, ammo=True):
        return dict(item=[0, 3], slot=slot, ammo=[0, slot] if ammo else None, type=kind)

    capes = [None, 10498, 10499, 6570]
    torsos = [None, [1127, 1, 1000, 'Platebody', None, dict(model=301)], [1115, 1, 1000, 'Iron', None, dict(model=306)],
              [1117, 1, 1000, 'Bronze', None, dict(model=3379)], [1129, 1, 1000, 'Leather', None, dict(model=300)], [1131, 1, 1000, 'Hard']]
    for cape, torso, bow in itertools.product(capes, torsos, [841, 10034, 10033]):
        equipment = {}
        if cape:
            equipment['1'] = [cape, 1, 1000, 'cape']
        if torso:
            equipment['4'] = torso
        entity = archer(weapon=bow, equipment=equipment, levels=levels(range=50))
        state = dict(attacker=0, victim=1, weapon=weapon('DEFAULT'), rangeWeapon=841, ammunition=882)
        for draws in [[79, 0], [80, 1], [0, 3]]:
            add([entity, N(location=at(3, 3))], [dict(op='postSwing', h=1, e=0, v=1, state=state)], rng=dict(ints=draws + [1, 1]), range=RANGE)
    states = [
        dict(weapon=weapon('DEFAULT'), rangeWeapon=841, ammunition=882), dict(weapon=weapon('DOUBLE_SHOT'), rangeWeapon=11235, ammunition=882),
        dict(weapon=weapon('DEGRADING'), rangeWeapon=4212), dict(weapon=weapon('THROWN', 3), rangeWeapon=864, ammunition=864),
        dict(weapon=weapon('CROSSBOW'), rangeWeapon=9185, ammunition=9144), dict(weapon=weapon(None), rangeWeapon=841),
        dict(weapon=weapon('DEFAULT', -1), rangeWeapon=841), dict(weapon=weapon('DEFAULT', 13, False), rangeWeapon=841),
        dict(weapon=weapon('DEFAULT')), dict(rangeWeapon=841), dict(weapon=dict(item=[0, 3]), rangeWeapon=841),
    ]
    for state, amount, level in itertools.product(states, [1, 2], [1, 99]):
        state = dict(state, attacker=0, victim=1)
        for entity, (location, clip) in zip(
                [archer(amount=amount, levels=levels(range=level)), archer(amount=amount, levels=levels(range=level), dropNull=True),
                 archer(amount=amount, levels=levels(range=level), attrs={'duel:ammo': True}), archer(amount=amount, levels=levels(range=level)),
                 archer(amount=amount, levels=levels(range=level)), archer(ammo=4740, amount=amount, equipment={'1': [10499, 1, 1000, 'Ava']}),
                 archer(amount=amount, levels=levels(range=level), attrs={'duel:ammo': True})],
                [(at(2, 0), {}), (at(2, 0), {}), (at(2, 0), {}), (at(2, 0), {'3202,3200,0': 0x200000}), (at(2, 0), {'3202,3200,0': 0x1fffff}),
                 (at(2, 0), {}), (None, {})]):
            if True:
                add([entity, N(location=location)],
                    [dict(op='postSwing', h=1, e=0, v=1, state=state), dict(op='useAmmo', e=0, location=at(1, 1), state=state)],
                    rng=dict(intSeed=amount + level), range=RANGE, clip=clip)
    for h in [0, 2]:
        add([archer(), N()], [dict(op='postSwing', h=h, e=0, v=1, state=dict(attacker=0, victim=1))], range=RANGE)
    add([archer(), N()], [dict(op='postSwing', h=1, e=None, v=1, state=dict()), dict(op='postSwing', h=1, e=0, v=None, state=dict())], range=RANGE)
    add([N(), N()], [dict(op='postSwing', h=1, e=0, v=1, state=dict()), dict(op='useAmmo', e=0, state=dict())], range=RANGE)
    sweeps = [
        (archer(amount=1, levels=levels(range=99), attrs={'duel:ammo': True}), [99, 3]),
        (archer(amount=4, equipment={'1': [10498, 1, 1000, 'Ava'], '4': [1127, 1, 1000, 'Plate', None, dict(model=301)]}), [0, 2]),
        (archer(amount=4, equipment={'1': [10498, 1, 1000, 'Ava'], '4': [1129, 1, 1000, 'Leather']}), [0]),
    ]
    for entity, draws in sweeps:
        add([entity, N(location=at(2, 0))],
            [dict(op='postSwing', h=1, e=0, v=1, state=dict(attacker=0, victim=1, weapon=weapon('DEFAULT'), rangeWeapon=841, ammunition=882))],
            0, rng=dict(ints=draws), range=RANGE)


def visuals(add, each):
    for anim in [422, 423, 451, None]:
        entity = dict(attackAnim=anim)
        for h, state in [(0, dict()), (1, dict(weapon=dict(item=[0, 3], slot=13, ammo=[0, 13], type='DEFAULT'), ammunition=882)),
                         (1, dict(weapon=dict(item=[0, 3], slot=13, ammo=[0, 13], type='DOUBLE_SHOT'), ammunition=882)),
                         (1, dict(weapon=dict(item=[0, 3], slot=13, ammo=[0, 13], type='DOUBLE_SHOT'), ammunition=884)),
                         (1, dict(weapon=dict(item=[0, 3], slot=13, ammo=[0, 13]), ammunition=4740)),
                         (1, dict(ammunition=11212)), (1, dict(weapon=dict(item=[0, 3]))), (1, dict()), (2, dict(spell=0)), (2, dict())]:
            for weapon in [841, 1277]:
                add([archer(weapon=weapon, **entity), N(location=at(4, 3))],
                    [dict(op='visualize', h=h, e=0, v=1, state=dict(state, attacker=0, victim=1)),
                     dict(op='visualize', h=h, e=0, v=None, state=dict(state, attacker=0))], range=RANGE, spells=[{}])
    graphics = [None, 'absent', [], [[100, 90, 0]], [[100, 90, 0], [200, 43, 5]], [None, [200, 43, 5], None], [[1, 2, 3], None]]
    for shown in graphics:
        entity = N(handler=1)
        if shown != 'absent':
            entity['combatGraphics'] = shown
        add([entity, P(location=at(2, 2))], [dict(op='visualize', h=1, e=0, v=1, state=dict(attacker=0, victim=1))], 0 if shown == graphics[4] else None)
    add([archer(), N(location=at(4, 3))],
        [dict(op='visualize', h=1, e=0, v=1, state=dict(attacker=0, victim=1, weapon=dict(item=[0, 3], slot=13, ammo=[0, 13], type='DOUBLE_SHOT'), ammunition=882))],
        0, range=RANGE)
    # Impact visuals.
    child = lambda victim: dict(attacker=0, victim=victim)
    for h in [0, 1, 2]:
        for state in [dict(), dict(spell=0), dict(targets=[]), dict(spell=0, targets=['this', child(2), None, child(None)]),
                      dict(targets=[child(1)]), dict(spell=0, targets=[dict(child(2), targets=[child(1)])])]:
            for victim in [N(), N(defenceAnim=None), N(defenceAnim=1156)]:
                add([P(), victim, N()], [dict(op='visualizeImpact', h=h, e=0, v=1, state=dict(state, attacker=0, victim=1)),
                                         dict(op='visualizeImpact', h=h, e=None, v=None, state=dict(state))], spells=[{}])
    # Attack audio.
    audios = [None, [], [2500], [2500, 2501, 2502], [0, 2501], [None, 2501], [2500, None], [0], [None]]
    for index, chosen in itertools.product([0, 1, 2, 5, -1], audios):
        row = [1277, 1, 1000, 'Sword', None, {} if chosen is None else dict(audios=chosen)]
        entity = P(attackStyleIndex=index, equipment={'3': row})
        add([entity, N()], [dict(op='visualizeAudio', h=0, e=0, v=1, state=dict(weapon=dict(item=[0, 3])))])
    for h in [0, 1, 2]:
        for entity in [P(), P(location=None), N(audio=[511, 512]), N(audio=[None, 512]), N(), N(audio=[77], location=None), dict(kind='other')]:
            for state in [dict(), dict(weapon=dict(item=None))]:
                add([entity, N()], [dict(op='visualizeAudio', h=h, e=0, v=1, state=state)])
    add([P(attackStyleIndex=1, equipment={'3': [1277, 1, 1000, 'Sword', None, dict(audios=[2500, 2501])]}), N()],
        [dict(op='visualizeAudio', h=0, e=0, v=1, state=dict(weapon=dict(item=[0, 3])))], 0)
    # getAttackAnimation.
    for entity in [P(), P(attackAnim=None), N(), N(combatAnims=[401, None, 403]), N(combatAnims=[None, 402]), N(attackAnim=None, combatAnims=[None, None, None])]:
        for h in [0, 1, 2]:
            ops = [dict(op='attackAnimation', h=h, e=0, style=style) for style in ['MELEE', 'RANGE', 'MAGIC', None]]
            ops += [dict(op='setType', h=h, style=None)] + [dict(op='attackAnimation', h=h, e=0, style=style) for style in ['MELEE', None]]
            add([entity], ops)


def impacts(add, each):
    child = lambda victim, hit, second=-1: dict(attacker=0, victim=victim, estimatedHit=hit, secondaryHit=second)
    states = [dict(estimatedHit=-1), dict(estimatedHit=0), dict(estimatedHit=5), dict(estimatedHit=5, secondaryHit=0),
              dict(estimatedHit=-1, secondaryHit=6), dict(estimatedHit=40, secondaryHit=40), dict(estimatedHit=3, targets=[]),
              dict(estimatedHit=3, secondaryHit=1, targets=['this']), dict(estimatedHit=3, targets=[child(2, 4, 2), 'this']),
              dict(estimatedHit=3, targets=[child(2, 4), None]), dict(estimatedHit=3, targets=[child(None, 4)]),
              dict(estimatedHit=3, targets=[child(2, -1, 7), child(1, 9)]), dict(estimatedHit=2, ammunition=9243),
              dict(estimatedHit=2, ammunition=882, secondaryHit=3)]
    victims = [N(lifepoints=30, maxLifepoints=30), done(lifepoints=30, maxLifepoints=30), P(lifepoints=30), N(lifepoints=4, retaliate=True),
               N(dead=True), N(lifepoints=30, audio=[None, 77])]
    for h, state, victim in itertools.product([0, 1, 2], states, victims):
        full = dict(state, attacker=0, victim=1)
        add([P(), victim, N(lifepoints=5)],
            [dict(op='impact', h=h, e=0, v=1, state=full), dict(op='tick', n=2), dict(op='impact', h=h, e=None, v=None, state=full)], range=RANGE)
        add([N(), victim, done(lifepoints=5)], [dict(op='onImpact', h=h, e=0, v=1, state=full), dict(op='onImpact', h=h, e=0, v=None, state=full)], range=RANGE)
    for attacker in [P(), N(), None]:
        add([P(), N(), N()], [dict(op='onImpact', h=0, e=0 if attacker else None, v=1, state='none')])
    add([P(), N(lifepoints=3), N()], [dict(op='impact', h=0, e=0, v=1, state=dict(attacker=0, victim=1, estimatedHit=5, secondaryHit=2))], 0)
    add([P(), N(lifepoints=30), N()], [dict(op='impact', h=1, e=0, v=1, state=dict(attacker=0, victim=1, estimatedHit=5, secondaryHit=2))], 0)
    add([P(), done(lifepoints=30), N(lifepoints=9)],
        [dict(op='impact', h=2, e=0, v=1, state=dict(attacker=0, victim=1, targets=[child(1, 3), child(2, 20)]))], 0)
    add([P(), N(retaliate=True), N()], [dict(op='onImpact', h=0, e=0, v=1, state=dict(attacker=0, victim=1, targets=[child(2, 1), 'this']))], 0)
    # Armour set effects.
    for name in SETS:
        for hit, draw in itertools.product([-1, 0, 5], [24, 25, 0]):
            for victim in [N(levels=levels(agility=57)), P(levels=levels(agility=4)), N(dynamicLevels=None), N(dynamicLevels=16)]:
                add([P(lifepoints=3, maxLifepoints=20), victim],
                    [dict(op='setEffect', set=name, e=0, v=1, state=dict(estimatedHit=hit)), dict(op='setVisualize', set=name, e=0, v=1)],
                    0 if (hit, draw) == (5, 24) and victim.get('kind') == 'player' else None, rng=dict(ints=[draw]))


def impact_handler(add, each):
    sources = [done(uid=77), N(), dict(kind='familiar', owner=0), dict(kind='familiar', owner=None), None]
    owners = [done(uid=77, hasFamiliar=True), done(uid=77, hasFamiliar=True, familiarPet=True), done(uid=77)]
    victims = [N(lifepoints=20, maxLifepoints=20), done(lifepoints=20, maxLifepoints=20), P(lifepoints=20), dict(kind='aiplayer', lifepoints=20),
               N(lifepoints=20, dead=True), done(lifepoints=20, attrs={'godMode': True}), N(lifepoints=1, maxLifepoints=50)]
    kinds = [None, 'NORMAL', 'POISON', 'DISEASE', 'MISS']
    for victim, kind, style in itertools.product(victims, kinds, [None, 'MELEE']):
        for source_at, owner in [(0, owners[0]), (1, owners[0]), (2, owners[0]), (2, owners[1]), (2, owners[2]), (3, owners[0]), (4, owners[0])]:
            entities = [owner, N(), dict(kind='familiar', owner=0), dict(kind='familiar', owner=None), victim]
            ops = [dict(op='handleImpact', e=4, source=None if source_at == 4 else source_at, hit=hit, style=style, type=kind)
                   for hit in [0, 1, -3, 19, 21]]
            add(entities, ops)
    del sources
    # Every hitsplat type, with and without damage.
    for kind in ['MISS', 'NORMAL', 'POISON', 'DISEASE', 'NORMAL_1', 'VENOM', None]:
        add([N(), N(lifepoints=20, maxLifepoints=20)],
            [dict(op='handleImpact', e=1, source=0, hit=hit, style=style, type=kind, secondary=True) for hit in [0, 3] for style in [None, 'MAGIC']])
    # Lifepoint thresholds: death, redemption and the ring of life.
    for lifepoints, maximum, hit in itertools.product([10, 11, 100], [100, 110], [0, 1, 2, 9, 10, 90, 91, 100]):
        for victim in [done(prayers=['REDEMPTION']), done(), N(), done(contains=['2570'], staticLevels=levels(hitpoints=maximum)),
                       done(contains=['2570'], staticLevels=levels(hitpoints=maximum), ringOfLife=True, zoneType=0),
                       done(contains=['2570'], staticLevels=levels(hitpoints=99), ringOfLife=True, zoneType=1)]:
            victim = dict(victim, lifepoints=lifepoints, maxLifepoints=maximum)
            add([N(), victim], [dict(op='handleImpact', e=1, source=0, hit=hit, style='RANGE')])
    add([done(uid=5), done(lifepoints=9, maxLifepoints=100, prayers=['REDEMPTION'], contains=['2570'], staticLevels=levels(hitpoints=99), ringOfLife=True)],
        [dict(op='handleImpact', e=1, source=0, hit=2, style='MAGIC')], 0)
    add([dict(kind='familiar', owner=2), N(lifepoints=5), done(uid=9, hasFamiliar=True)],
        [dict(op='handleImpact', e=1, source=0, hit=9, style='MELEE', type='NORMAL')], 0, uids={'9': 2})
    add([N(), done(lifepoints=5, dead=True)], [dict(op='setVictim', e=1, v=0), dict(op='handleImpact', e=1, source=0, hit=1, style=None)], 1)
    # Disabled ticks.
    for now, disabled in [(10, 0), (10, 1), (10, -1), (2147483630, 10)]:
        add([N(), N(lifepoints=30)],
            [dict(op='setDisabled', e=1, ticks=disabled), dict(op='handleImpact', e=1, source=0, hit=3, style=None), dict(op='setTick', tick=now + disabled),
             dict(op='handleImpact', e=1, source=0, hit=3, style=None), dict(op='setTick', tick=now + disabled - 1),
             dict(op='handleImpact', e=1, source=0, hit=3, style=None), dict(op='isHitUpdate', e=1), dict(op='pollImpacts', e=1), dict(op='isHitUpdate', e=1)],
            tick=now)
    # The disabled deadline wraps as an int.
    add([N(), N(lifepoints=30)], [dict(op='setDisabled', e=1, ticks=5), dict(op='handleImpact', e=1, source=0, hit=3, style=None),
                                  dict(op='setDisabled', e=1, ticks=-2147483648), dict(op='handleImpact', e=1, source=0, hit=3, style=None)], tick=2147483647)
    # manualHit, immediate and delayed.
    for hit, ticks in itertools.product([0, 4, 30, 31, 500], [None, 0, 1, 3, -2]):
        for kind in [None, 'POISON', 'DISEASE']:
            op = dict(op='manualHit', e=1, source=0, hit=hit, type=kind)
            if ticks is not None:
                op['ticks'] = ticks
            add([done(uid=3), N(lifepoints=30), N()], [op, dict(op='tick', n=2), dict(op='set', e=1, key='active', value=False), dict(op='tick', n=3)])
    add([done(uid=3), N(lifepoints=30)], [dict(op='manualHit', e=1, source=0, hit=40, type='NORMAL')], 0)
    add([done(uid=3), N(lifepoints=30)], [dict(op='manualHit', e=1, source=0, hit=40, type='NORMAL', ticks=2), dict(op='tick', n=3)], 1)
    # Recoil.
    for owner in [done(lifepoints=50), done(lifepoints=50, recoilDamage=1), done(lifepoints=50, recoilDamage=5), N(lifepoints=50)]:
        for hit in [-30, -1, 0, 1, 10, 11, 49, 50, 51, 395, 400, 401]:
            add([owner, N(lifepoints=60), N()], [dict(op='recoil', e=0, attacker=1, hit=hit), dict(op='recoil', e=0, attacker=1, hit=hit)])
    add([done(lifepoints=50, recoilDamage=3), N(lifepoints=2)], [dict(op='recoil', e=0, attacker=1, hit=35)], 0)
    add([done(lifepoints=50), N(lifepoints=60)], [dict(op='recoil', e=0, attacker=1, hit=35)], 0)
    # Kill credit: the damage logs and their HashMap iteration order.
    for count, spread in itertools.product([1, 2, 3, 13, 14], ['plain', 'bucket', 'same', 'wide']):
        # Distinct sources may share a hash code; no bucket ever holds more than eight, so no tree bin forms.
        hashes = dict(plain=lambda k: 31 * k + 7, bucket=lambda k: 16 * k + 5, same=lambda k: 16 * (k // 2) + 5, wide=lambda k: (k * 0x9E3779B1) % 0x100000000 - 0x80000000)[spread]
        npcs = [N(hash=hashes(k), lifepoints=5) for k in range(count)]
        victim = N(lifepoints=10000, maxLifepoints=10000)
        ops = []
        for k in range(count):
            ops.append(dict(op='handleImpact', e=count, source=k, hit=7 if k % 4 else 9, style=None))
        ops.append(dict(op='mostDamage', e=count, killer=0))
        ops.append(dict(op='handleImpact', e=count, source=count, hit=500, style=None))
        ops.append(dict(op='handleImpact', e=count, source=None, hit=9, style=None))
        ops.append(dict(op='mostDamage', e=count, killer=None))
        ops.append(dict(op='handleImpact', e=count, source=count - 1, hit=2, style=None))
        ops.append(dict(op='mostDamage', e=count, killer=count))
        ops += [dict(op='clearLogs', e=count), dict(op='handleImpact', e=count, source=0, hit=1, style=None),
                dict(op='handleImpact', e=count, source=count - 1, hit=1, style=None), dict(op='mostDamage', e=count, killer=0)]
        add(npcs + [victim], ops)
        uids = dict(plain=lambda k: k + 1, bucket=lambda k: 16 * k + 3, same=lambda k: 64 * k + 9, wide=lambda k: (k * 0x9E3779B1) % 0x100000000 - 0x80000000)[spread]
        players = [done(uid=uids(k)) for k in range(count)]
        ops = []
        for k in range(count):
            ops.append(dict(op='handleImpact', e=count, source=k, hit=7 if k % 4 else 9, style='MELEE'))
        ops.append(dict(op='mostDamage', e=count, killer=0))
        ops.append(dict(op='handleImpact', e=count, source=count - 1, hit=2, style='MELEE'))
        ops.append(dict(op='mostDamage', e=count, killer=None))
        ops += [dict(op='clearLogs', e=count), dict(op='handleImpact', e=count, source=count - 1, hit=1, style=None), dict(op='mostDamage', e=count, killer=0)]
        add(players + [victim], ops, uids={str(uids(k)): k for k in range(0, count, 2)})
    for killer in [0, 1, 2, None]:
        add([done(uid=4), dict(kind='aiplayer', uid=8), N(), N(lifepoints=50), done(lifepoints=50)],
            [dict(op='handleImpact', e=3, source=2, hit=5, style=None), dict(op='mostDamage', e=3, killer=killer), dict(op='mostDamage', e=4, killer=killer),
             dict(op='handleImpact', e=3, source=1, hit=5, style='MELEE'), dict(op='mostDamage', e=3, killer=killer),
             dict(op='handleImpact', e=3, source=0, hit=2147483647, style='MELEE'), dict(op='handleImpact', e=3, source=1, hit=40, style='MELEE'),
             dict(op='mostDamage', e=3, killer=killer)], uids={'4': 0, '8': 1})
    # Overflowing totals wrap.
    add([done(uid=4), N(lifepoints=2147483647, maxLifepoints=2147483647)],
        [dict(op='handleImpact', e=1, source=0, hit=2147483000, style=None), dict(op='set', e=1, key='lifepoints', value=2147483647),
         dict(op='handleImpact', e=1, source=0, hit=2147483000, style=None), dict(op='mostDamage', e=1, killer=0)], uids={'4': 0})


def pulses(add, each, short):
    # attack: every early return and the black mask.
    add([P(), N()], [dict(op='attack', e=0, v=None), dict(op='attack', e=0, v=1), dict(op='attack', e=0, v=1), dict(op='isAttacking', e=0),
                     dict(op='stop', e=0), dict(op='attack', e=0, v=1), dict(op='isInCombat', e=1), dict(op='isInCombat', e=0)])
    add([P(interactionLocked=True), N()], [dict(op='attack', e=0, v=1)])
    for entity_id in [4474, 7891, 1]:
        add([N(id=entity_id), P(), N()], [dict(op='attack', e=0, v=1), dict(op='attack', e=0, v=2)])
        for level in [29, 30, 126]:
            add([P(combatLevel=level), N(id=entity_id), P(id=entity_id)], [dict(op='attack', e=0, v=1), dict(op='attack', e=0, v=2)])
    for mask, draw in itertools.product([8900, 8901, 8903, 8918, 8919, 8920], [0, 1, 49]):
        for defence in [1, 13, 14, 98, 99, 140]:
            entity = P(equipment={'0': [mask, 1, 1000, 'Black mask']})
            victim = N(levels=levels(defence=defence), staticLevels=levels(defence=defence))
            add([entity, victim, N()], [dict(op='attack', e=0, v=1), dict(op='attack', e=0, v=2), dict(op='attack', e=0, v=1)],
                0 if (mask, draw, defence) == (8903, 0, 99) else None, rng=dict(ints=[draw, draw, draw]))
    add([P(equipment={'0': [8905, 1, 1000, 'Black mask']}), N(movementLocked=True), N(active=False), P()],
        [dict(op='attack', e=0, v=1), dict(op='attack', e=0, v=2), dict(op='isAttacking', e=0), dict(op='attack', e=0, v=3),
         dict(op='stop', e=0), dict(op='attack', e=0, v=3), dict(op='attack', e=0, v=1)], rng=dict(ints=[0, 0, 0, 0]))
    add([N(), P()], [dict(op='attack', e=0, v=1)], 0)
    add([P(), P()], [dict(op='attack', e=0, v=1), dict(op='attack', e=0, v=1)], 1)
    # Plain members.
    add([P(), N(), N()], [dict(op='setVictim', e=0, v=1), dict(op='start', e=0), dict(op='setVictim', e=0, v=2), dict(op='stop', e=0),
                           dict(op='stop', e=0), dict(op='setVictim', e=0, v=None), dict(op='start', e=0), dict(op='canInteract', e=0),
                           dict(op='setNextAttack', e=0, ticks=5), dict(op='delayNextAttack', e=0, ticks=-9), dict(op='delayNextAttack', e=0, ticks=2147483647),
                           dict(op='setNextAttack', e=0, ticks=2147483647)], tick=100)
    add([P(), N()], [dict(op='stop', e=0)], 0)
    add([P(), N()], [dict(op='setVictim', e=0, v=1), dict(op='start', e=0)], 1)
    for bonus in range(5):
        for extra in [dict(), dict(spell=0), dict(autocast=0), dict(attackStyle=None)]:
            entity = dict(P(attackStyle=[0, bonus]), **extra)
            add([entity, N(attackStyle=[0, 4])], [dict(op='updateStyle', e=0), dict(op='updateStyle', e=1), dict(op='setSpell', e=0),
                                                  dict(op='updateStyle', e=0)], spells=[{}])
    add([P(attackStyle=[0, 4])], [dict(op='updateStyle', e=0)], 0)
    for entity, victim in itertools.product([P(), P(attrs={'keepDialogueAlive': True}), P(wilderness=True), N()],
                                           [P(), P(attrs={'keepDialogueAlive': False}), N(), N(movingPulse=True), N(movingPulse=False)]):
        add([entity, victim, N()], [dict(op='setCombatFlags', e=0, v=1), dict(op='setCombatFlags', e=0, v=None), dict(op='attack', e=1, v=2),
                                    dict(op='setCombatFlags', e=0, v=1), dict(op='isInCombat', e=1)])
    add([P(wilderness=True), P()], [dict(op='attack', e=1, v=0), dict(op='setCombatFlags', e=0, v=1)], 1)
    add([P(levels=levels(attack=60, strength=60)), N(location=at(1, 0), lifepoints=90)],
        [dict(op='isInCombat', e=1), dict(op='attack', e=0, v=1), dict(op='tick', n=1), dict(op='isInCombat', e=1), dict(op='isInCombat', e=0),
         dict(op='set', e=1, key='active', value=False), dict(op='isInCombat', e=1), dict(op='set', e=1, key='active', value=True),
         dict(op='stop', e=0), dict(op='isInCombat', e=1)], rng=dict(mathSeed=2, intSeed=2))
    # Special attack handlers: one map per handler instance, created on first use.
    extra = [dict(kind='salamander'), dict(kind='melee'), dict(kind='magic', flags=['IGNORE_STAT_REDUCTION'])]
    add([], [dict(op='getSpecial', h=0, itemId=4151), dict(op='register', h=0, itemId=4151, special=3), dict(op='getSpecial', h=0, itemId=4151),
             dict(op='register', h=0, itemId=4151, special=4), dict(op='register', h=0, itemId=4151, special=3), dict(op='getSpecial', h=0, itemId=4151),
             dict(op='getSpecial', h=1, itemId=4151), dict(op='register', h=1, itemId=4151, special=5), dict(op='register', h=1, itemId=-1, special=1),
             dict(op='register', h=1, itemId=-1, special=0), dict(op='getSpecial', h=1, itemId=-1), dict(op='register', h=4, itemId=7, special=4),
             dict(op='register', h=4, itemId=7, special=2), dict(op='getSpecial', h=4, itemId=7), dict(op='register', h=2, itemId=0, special=5),
             dict(op='register', h=2, itemId=0, special=5), dict(op='getSpecial', h=3, itemId=0), dict(op='register', h=3, itemId=0, special=0),
             dict(op='register', h=3, itemId=0, special=1)], handlers=extra)
    add([], [dict(op='register', h=0, itemId=4151, special=3), dict(op='register', h=0, itemId=4151, special=4)], 1, handlers=extra)
    # canInteract and the temporary handler.
    for temp in [None, 0, 1, 2, 3]:
        add([archer(weaponInterface=short, autocast=0), N(location=at(4, 0))],
            [dict(op='canInteract', e=0), dict(op='setVictim', e=0, v=1), dict(op='setTemp', e=0, h=temp), dict(op='canInteract', e=0)],
            range=RANGE, handlers=[dict(kind='salamander')], spells=[dict(spellId=4)])
    # The companion swing.
    melee = [P(), P(contains=SETS['DHAROK']), P(contains=SETS['GUTHAN'], lifepoints=3), P(contains=SETS['VERAC']), P(contains=SETS['TORAG']),
             P(contains=SETS['BERSERKER_NECKLACE_SETS']), N(id=2030), N(id=2027), N(id=2029), N(audio=[90])]
    for entity, victim in itertools.product(melee, [N(lifepoints=40), done(lifepoints=40), N(lifepoints=40, id=7891), N(lifepoints=1),
                                                   done(lifepoints=40, protection=['MELEE'])]):
        for seed in range(3):
            add([dict(entity, levels=levels(attack=70, strength=70)), victim],
                [dict(op='swing', e=0, v=1, h=0), dict(op='tick', n=2)], rng=dict(mathSeed=seed, intSeed=seed + 9))
    add([P(), N()], [dict(op='swing', e=0, v=None, h=0)])
    add([P(location=None), N()], [dict(op='swing', e=0, v=1, h=0)])
    ranged = [archer(), archer(weapon=11235, amount=6), archer(weapon=11235, amount=1), archer(weapon=4212, ammo=None), archer(weapon=864, ammo=None),
              archer(amount=0), archer(ammo=None), archer(weapon=1277), archer(ammo=9144), archer(weapon=9185, ammo=9243, amount=3),
              archer(contains=SETS['KARIL']), N(handler=1, combatGraphics=[[7, 8, 9], [10, 11, 12]])]
    for entity, distance in itertools.product(ranged, [1, 3, 4, 7, 10]):
        if entity['kind'] == 'player' and entity['equipment']['3'][0] == 864:
            entity = dict(entity, equipment={'3': [864, 4, 1000, 'Knife']})
        for seed in range(2):
            add([dict(entity, levels=levels(range=80)), N(lifepoints=60, location=at(distance, 0))],
                [dict(op='swing', e=0, v=1, h=1), dict(op='tick', n=5)], rng=dict(mathSeed=seed + 20, intSeed=seed), range=RANGE)
    casts = [dict(maxImpact=8), dict(maxImpact=20, type='BLITZ'), dict(meets=False), dict(maxImpact=12, targets=[[1], [2], None, [1]]),
             dict(maxImpact=5, targets=[]), dict(maxImpact=1)]
    for spell, caster in itertools.product(range(len(casts)), ['spell', 'autocast', 'both', 'none', 'npc']):
        entity = N(handler=2, autocast=spell) if caster == 'npc' else P(handler=2, levels=levels(magic=70), weaponInterface='UNARMED')
        if caster in ('spell', 'both'):
            entity['spell'] = spell
        if caster in ('autocast', 'both'):
            entity['autocast'] = spell
        for distance in [1, 6]:
            add([entity, N(lifepoints=60, location=at(distance, 0)), done(lifepoints=60, location=at(distance, 1))],
                [dict(op='swing', e=0, v=1, h=2), dict(op='tick', n=7)], rng=dict(mathSeed=spell, intSeed=distance), spells=casts)
    add([P(levels=levels(attack=70, strength=70), contains=SETS['GUTHAN']), N(lifepoints=40)], [dict(op='swing', e=0, v=1, h=0)], 0, rng=dict(mathSeed=1, intSeed=24))
    add([archer(weapon=11235, amount=6, levels=levels(range=80)), N(lifepoints=60, location=at(4, 0))], [dict(op='swing', e=0, v=1, h=1)], 0,
        rng=dict(mathSeed=3, intSeed=3), range=RANGE)
    add([P(handler=2, levels=levels(magic=70), autocast=0), N(lifepoints=60, location=at(3, 0))], [dict(op='swing', e=0, v=1, h=2)], 0,
        rng=dict(mathSeed=3, intSeed=3), spells=casts)
    # pulse(): every early return in order.
    target = lambda **fields: N(**dict(dict(location=at(1, 0), lifepoints=40), **fields))
    early = [
        ([P(), target()], []),
        ([P(dead=True), target()], [dict(op='setVictim', e=0, v=1)]),
        ([P(), target(dead=True)], [dict(op='setVictim', e=0, v=1)]),
        ([P(location=at(700, 700)), target()], [dict(op='setVictim', e=0, v=1)]),
        ([P(), N(location=at(700, 700), lifepoints=40)], [dict(op='setVictim', e=0, v=1)]),
        ([P(location=None), target()], [dict(op='setVictim', e=0, v=1)]),
        ([P(), N(location=None)], [dict(op='setVictim', e=0, v=1)]),
        ([N(hidden=True), P(location=at(1, 0))], [dict(op='setVictim', e=0, v=1)]),
        ([P(), target(hidden=True)], [dict(op='setVictim', e=0, v=1)]),
        ([N(canStartCombat=False), P(location=at(1, 0))], [dict(op='setVictim', e=0, v=1)]),
        ([N(canStartCombat=False), N(location=at(1, 0))], [dict(op='setVictim', e=0, v=1)]),
        ([P(moving=True), N(location=at(5, 0))], [dict(op='setVictim', e=0, v=1)]),
        ([P(combatTimeOut=1), N(location=at(5, 0))], [dict(op='setVictim', e=0, v=1)]),
        ([P(movementLocked=True, combatTimeOut=0), N(location=at(5, 0))], [dict(op='setVictim', e=0, v=1)]),
        ([P(combatTimeOut=-1), N(location=at(2, 0), moving=True)], [dict(op='setVictim', e=0, v=1)]),
        ([P(), target()], [dict(op='setVictim', e=0, v=1), dict(op='setNextAttack', e=0, ticks=3)]),
        ([P(), target(attackable=False)], [dict(op='setVictim', e=0, v=1)]),
        ([dict(kind='randomEvent', location=at(0, 0)), P(location=at(1, 0), attackable=False, attrs={'re-npc': dict(entity=0)})], [dict(op='setVictim', e=0, v=1)]),
        ([N(), P(location=at(1, 0), attackable=False, attrs={'re-npc': dict(entity=2)}), dict(kind='randomEvent')], [dict(op='setVictim', e=0, v=1)]),
        ([P(handler=2), target()], [dict(op='setVictim', e=0, v=1)]),
        ([P(lifepoints=0), target()], [dict(op='setVictim', e=0, v=1)]),
        ([P(), target(lifepoints=0)], [dict(op='setVictim', e=0, v=1)]),
        ([P(), N(location=at(1, 0), lifepoints=40, active=False)], [dict(op='setVictim', e=0, v=1)]),
        ([P(active=False), target()], [dict(op='setVictim', e=0, v=1)]),
    ]
    heavy = {17, 20, 21, 22, 23}
    for number, (entities, before) in enumerate(early):
        tail = [dict(op='pulse', e=0), dict(op='pulse', e=0), dict(op='pulse', e=0), dict(op='start', e=0), dict(op='update', e=0), dict(op='update', e=0),
                dict(op='update', e=0)]
        add(entities, before + tail, rng=dict(mathSeed=4, intSeed=4), tick=50, inactiveRegions=[15420])
        if number in heavy or number == 0:
            continue
        add(entities, before + [dict(op='pulse', e=0)], len(before), rng=dict(mathSeed=4, intSeed=4), tick=50, inactiveRegions=[15420])
        add(entities, before + [dict(op='start', e=0), dict(op='update', e=0)], len(before) + 1, rng=dict(mathSeed=4, intSeed=4), tick=50, inactiveRegions=[15420])
    # Attack speed sources.
    for speed, style, extra in itertools.product([3, 4, 5, 7], [(0, 2), (5, 4), (4, 4)], [dict(), dict(miasmic=True)]):
        for temp in [None, 3] + ([4] if speed == 4 else []):
            for entity in [archer(attackSpeed=speed, style=style, **extra), dict(archer(attackSpeed=speed, style=style, **extra), kind='npc'),
                           P(attackSpeed=speed, attackStyle=list(style), handler=2, autocast=0, **extra),
                           N(attackSpeed=speed, attackStyle=list(style), handler=2, autocast=0, **extra)]:
                # The salamander stand-in inherits the range swing: without a range weapon it would log under the
                # stand-in's own class name, which says nothing about the original class.
                if temp == 3 and entity['kind'] == 'player' and entity['handler'] == 2:
                    continue
                add([dict(entity, weaponInterface=short), target()],
                    [dict(op='setVictim', e=0, v=1), dict(op='setTemp', e=0, h=temp), dict(op='pulse', e=0), dict(op='pulse', e=0)],
                    rng=dict(mathSeed=speed, intSeed=speed), range=RANGE, spells=[dict(maxImpact=6)], tick=9,
                    handlers=[dict(kind='salamander'), dict(kind='melee', flags=['IGNORE_PRAYER_BOOSTS_DAMAGE'])])


def fights(add, short):
    strong = levels(attack=75, strength=75, defence=60, range=75, magic=75)
    plans = []
    for speed, style in itertools.product([4, 5, 6], [(0, 2), (1, 2), (2, 1), (3, 0)]):
        plans.append(([done(attackSpeed=speed, attackStyle=list(style), levels=strong, lifepoints=60, maxLifepoints=60),
                       N(location=at(1, 0), lifepoints=25, maxLifepoints=25, levels=levels(defence=5))], {}))
    # Both sides fight; the second attacks back on its first impact.
    for a, b in [(4, 4), (4, 6), (7, 3)]:
        plans.append(([N(attackSpeed=a, levels=levels(attack=50, strength=50), lifepoints=40, maxLifepoints=40, audio=[5, 6]),
                       done(location=at(1, 0), attackSpeed=b, levels=strong, lifepoints=45, maxLifepoints=45, retaliate=True)], {}))
        plans.append(([done(attackSpeed=a, levels=strong, lifepoints=30, maxLifepoints=30, wilderness=True, prayers=['SMITE'], uid=11),
                       done(location=at(0, 1), attackSpeed=b, levels=strong, lifepoints=30, maxLifepoints=30, retaliate=True, prayerPoints=H(9),
                            prayers=['REDEMPTION'], contains=['2570'], staticLevels=levels(hitpoints=99), uid=12)], dict(uids={'11': 0, '12': 1})))
    for name in ['DHAROK', 'GUTHAN', 'VERAC', 'TORAG']:
        plans.append(([done(contains=SETS[name], levels=strong, lifepoints=20, maxLifepoints=70),
                       done(location=at(1, 0), lifepoints=70, maxLifepoints=70, protection=['MELEE'])], {}))
    for distance, entity in itertools.product([1, 4, 7], [
            archer(amount=7, levels=strong, weaponInterface=short), archer(amount=9, levels=strong, style=(5, 4), weaponInterface=short),
            archer(amount=9, levels=strong, style=(6, 4), weaponInterface=short), archer(weapon=11235, amount=7, levels=strong, attackSpeed=9),
            archer(weapon=4212, ammo=None, levels=strong, attackSpeed=5), archer(ammo=884, amount=30, levels=strong, weaponInterface=short, attackSpeed=3),
            archer(weapon=9185, ammo=9243, amount=30, levels=strong, attackSpeed=6), dict(archer(levels=strong, contains=SETS['KARIL']), miasmic=True)]):
        plans.append(([entity, N(location=at(distance, 0), lifepoints=45, maxLifepoints=45, levels=levels(defence=10, agility=40))], dict(range=RANGE)))
    plans.append(([done(equipment={'3': [864, 6, 1000, 'Knife']}, attackStyle=[5, 4], handler=1, weaponInterface=short, levels=strong, attackSpeed=3),
                   N(location=at(3, 1), lifepoints=45, maxLifepoints=45)], dict(range=RANGE)))
    plans.append(([N(handler=1, attackSpeed=5, levels=levels(range=60), combatGraphics=[[7, 8, 9], [10, 11, 12]], poisonous=True, poisonSeverity=6),
                   done(location=at(5, 2), lifepoints=40, maxLifepoints=40, retaliate=True, levels=strong)], dict(range=RANGE)))
    casts = [dict(maxImpact=14), dict(maxImpact=24, type='BLITZ'), dict(maxImpact=10, targets=[[1], [2]]), dict(meets=False), dict(maxImpact=0)]
    for distance, (spell, autocast) in itertools.product([1, 5, 10], [(None, 0), (None, 1), (0, None), (1, 2), (None, 2), (3, 0), (None, 4)]):
        entity = done(handler=2, levels=strong, weaponInterface='UNARMED', attackStyle=[8, 3], prayers=['SMITE'])
        if spell is not None:
            entity['spell'] = spell
        if autocast is not None:
            entity['autocast'] = autocast
        plans.append(([entity, N(location=at(distance, 0), lifepoints=45, maxLifepoints=45), N(location=at(distance, 1), lifepoints=12, maxLifepoints=12)],
                      dict(spells=casts)))
    plans.append(([N(handler=2, autocast=0, attackSpeed=6, levels=levels(magic=60)), done(location=at(4, 4), lifepoints=40, maxLifepoints=40)], dict(spells=casts)))
    for number, (entities, extra) in enumerate(plans):
        add(entities, [dict(op='attack', e=0, v=1), dict(op='tick', n=22)], rng=dict(mathSeed=number * 2, intSeed=number * 3), **extra)
    # The world changes under a running fight.
    base = [done(levels=strong, lifepoints=60, maxLifepoints=60, combatTimeOut=3), N(location=at(1, 0), lifepoints=500, maxLifepoints=500), N(location=at(0, 1), lifepoints=500)]
    changes = [
        [dict(op='set', e=1, key='location', value=at(6, 0))], [dict(op='set', e=1, key='location', value=at(6, 0)), dict(op='set', e=0, key='moving', value=True)],
        [dict(op='set', e=1, key='active', value=False)], [dict(op='set', e=0, key='active', value=False)], [dict(op='set', e=1, key='dead', value=True)],
        [dict(op='set', e=0, key='lifepoints', value=0)], [dict(op='set', e=1, key='attackable', value=False)], [dict(op='attack', e=0, v=2)],
        [dict(op='attack', e=0, v=1)], [dict(op='stop', e=0), dict(op='attack', e=0, v=1)], [dict(op='stop', e=0), dict(op='attack', e=0, v=2)],
        [dict(op='set', e=1, key='location', value=None)], [dict(op='set', e=0, key='attackStyle', value=None)], [dict(op='setTemp', e=0, h=3)],
        [dict(op='set', e=1, key='hidden', value=True)], [dict(op='set', e=0, key='movementLocked', value=True), dict(op='set', e=1, key='location', value=at(9, 9))],
        [dict(op='manualHit', e=1, source=0, hit=30, type='POISON', ticks=2), dict(op='manualHit', e=0, source=None, hit=3, type='DISEASE', ticks=1)],
        [dict(op='setDisabled', e=1, ticks=4)], [dict(op='set', e=1, key='moving', value=True), dict(op='set', e=1, key='location', value=at(2, 0))],
    ]
    for number, change in enumerate(changes):
        for wait in [1, 4]:
            add(base, [dict(op='attack', e=0, v=1), dict(op='tick', n=wait)] + change + [dict(op='tick', n=9), dict(op='set', e=1, key='location', value=at(1, 0)),
                       dict(op='set', e=1, key='active', value=True), dict(op='tick', n=6)],
                rng=dict(mathSeed=number, intSeed=wait), handlers=[dict(kind='magic')], spells=[dict(maxImpact=3)])
    # Failures inside a running fight are caught by the pulse or by the runner.
    add(base, [dict(op='attack', e=0, v=1), dict(op='tick', n=2)], 1, rng=dict(mathSeed=8, intSeed=8))
