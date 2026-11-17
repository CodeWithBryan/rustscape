"""The designed corpus of the wilderness and dark zone oracle.

Each case is one fresh world: nodes, zones, an action list and optional
scripted answers, hooks (actions run when an event is emitted) and failures
(an event that throws on its n-th call).
"""

MAX = 2147483647
MIN = -2147483648
# A small wilderness: tiles 3200..3215 x 3600..3631 (levels 11..14), region 12856.
SMALL = [3200, 3600, 3215, 3631]
# Inside DarkZone's region 12693 (3136..3199 x 9536..9599); OUTSIDE is the region south of it.
DARK = [3140, 9540, 3160, 9560]
OUTSIDE = [3150, 9500, 0]
INSIDE_DARK = [3150, 9550, 0]

LIT = dict(candle=33, black_candle=32, torch=594, candle_lantern=4531, oil_lamp=4524, oil_lantern=4539, bullseye=4550,
           sapphire=4702, emerald=9065, mining_helmet=5013)
UNLIT = [36, 38, 596, 4529, 4522, 4537, 4548, 4701, 9064, 5014]


def a(op, **kw):
    return dict(op=op, **kw)


def player(id, location, **kw):
    return dict(id=id, kind='player', location=location, **kw)


def npc(id, location, **kw):
    return dict(id=id, kind='npc', location=location, **kw)


def familiar(id, location, **kw):
    return dict(id=id, kind='familiar', location=location, **kw)


def groups(cases):
    counts = {}
    for case in cases:
        counts[case['group']] = counts.get(case['group'], 0) + 1
    return dict(sorted(counts.items()))


def edges(low, high):
    return sorted({low - 1, low, low + 1, high - 1, high, high + 1})


def small_wild(id='w', borders=None):
    return dict(id=id, **{'class': 'wildernessBorders'}, borders=[borders or SMALL])


def levels(cases):
    def add(actions, **kw):
        cases.append(dict(group='levels', actions=actions, **kw))

    ys = sorted({y for k in range(-2, 60) for y in (3520 + 8 * k - 1, 3520 + 8 * k, 3520 + 8 * k + 1)}
                | {y for k in range(-2, 60) for y in (9920 + 8 * k - 1, 9920 + 8 * k, 9920 + 8 * k + 1)}
                | {6399, 6400, 6401, 0, -1, -7, -8, -9, -16384, 16383, 16384, 32767, 10303, 10304, 10367})
    for start in range(0, len(ys), 48):
        actions = []
        for y in ys[start:start + 48]:
            actions += [a('set', node=1, location=[3000, y, 0]), a('wildLevel', node=1)]
        add(actions, nodes=[player(1, [3000, 3000, 0])])
    add([a('wildLevel', node=1)], nodes=[player(1, None)])
    add([a('wildLevel', node=1), a('wildLevel', node=2), a('wildLevel', node=3)],
        nodes=[npc(1, [3100, 3700, 0]), familiar(2, [3100, 9990, 1]), dict(id=3, kind='other', location=[3100, 3601, 2])])
    add([a('wildLevel', node=1)], nodes=[player(1, [3100, 3700, 0])], failures=dict(location=1))

    borders = [[2944, 3525, 3400, 3975], [3070, 9924, 3135, 10002], [3008, 10240, 3071, 10303], [3008, 10304, 3071, 10367],
               [2944, 10304, 3007, 10367]]
    for b in borders:
        probes = [[x, y, z] for x in edges(b[0], b[2]) for y in edges(b[1], b[3]) for z in [0, 3]]
        for start in range(0, len(probes), 24):
            actions = []
            for at in probes[start:start + 24]:
                actions += [a('set', node=1, location=at), a('inZone', node=1), a('inZoneLocation', location=at)]
            add(actions, nodes=[player(1, [0, 0, 0])])
    add([a('inZone', node=1), a('inZoneLocation', location=None)], nodes=[player(1, None)])
    add([a('inZone', node=1), a('inZone', node=2)], nodes=[npc(1, [3000, 3600, 0]), player(2, [3000, 3600, 0])],
        failures=dict(location=3))
    add([a('wildBorders'), a('constants'), a('gloves'), a('zoneInfo', zone='w'), a('zoneInfo', zone='c')],
        zones=[dict(id='w'), small_wild('c')])
    rates = sorted({MIN, -1, 0, 1, 2, 3, 10, 50, 98, 99, 100, 126, 138, 200, 250, 300, 350, 390, 395, 396, 397, 398, 399, 400,
                    500, 1000, 5000, MAX} | set(range(1, 400, 7)))
    add([a('dropRate', level=level) for level in rates])
    add([a('configure', zone='w'), a('zoneInfo', zone='w')], zones=[dict(id='w')])
    add([a('configure', zone='c'), a('zoneInfo', zone='c')], zones=[dict(id='c', **{'class': 'wildernessBorders'},
                                                                        borders=[[3200, 3600, 3300, 3700], [3300, 3600, 3200, 3500, 1]])])


def entering(cases):
    def add(actions, **kw):
        cases.append(dict(group='enter', actions=actions, **kw))

    inside = [3205, 3610, 0]
    deeper = [3205, 3625, 0]
    out = [3220, 3610, 0]
    walk = [a('configure', zone='w'), a('update', node=1, last=None), a('set', node=1, location=inside),
            a('update', node=1, last=out), a('set', node=1, location=deeper), a('update', node=1, last=inside),
            a('set', node=1, location=out), a('update', node=1, last=deeper)]
    variants = [
        dict(),
        dict(artificial=True),
        dict(wildDisabled=True),
        dict(familiar=2),
        dict(familiar=2, pet=True),
        dict(hasFamiliar=True),
        dict(familiar=2, artificial=True),
        dict(wildDisabled=True, artificial=True),
        dict(playing=True),
    ]
    for variant in variants:
        for pvp in [True, False]:
            nodes = [player(1, out, **variant), familiar(2, [3000, 3000, 0], owner=1)]
            add(walk, nodes=nodes, zones=[small_wild()], pvp=pvp)
    add(walk, nodes=[player(1, out), familiar(2, [3000, 3000, 0], combatFamiliar=True)], zones=[small_wild()])
    add(walk, nodes=[player(1, out, familiar=2), familiar(2, [3000, 3000, 0], combatFamiliar=True)], zones=[small_wild()])
    add(walk, nodes=[player(1, out)], zones=[small_wild()], noSettings=True)
    for kind in ['npc', 'familiar', 'other']:
        for option in [True, False]:
            for aggressive in [True, False]:
                add(walk, nodes=[dict(id=1, kind=kind, location=out, attackOption=option, aggressive=aggressive)], zones=[small_wild()])
    # Logging out leaves without the player traces.
    add(walk[:4] + [a('clear', node=1)], nodes=[player(1, out, familiar=2), familiar(2, [3000, 3000, 0], combatFamiliar=True)],
        zones=[small_wild()])
    # Direct hooks, each event failing in turn.
    hooks = [a('hook', zone='w', hook='enter', node=1), a('hook', zone='w', hook='locationUpdate', node=1, last=None),
             a('hook', zone='w', hook='leave', node=1, logout=False), a('hook', zone='w', hook='leave', node=1, logout=True),
             a('show', node=1), a('leavePlayer', zone='w', node=1)]
    events = ['artificial', 'wildernessDisabled', 'openOverlay', 'location', 'setSkullLevel', 'skullLevel', 'sendString', 'wildPvp',
              'optionSet', 'setWilderness', 'hasFamiliar', 'hasPet', 'getFamiliar', 'transform', 'sync', 'closeComponent',
              'componentClose', 'optionRemove', 'combatFamiliar', 'reTransform']
    for event in events:
        for at in [1, 2]:
            add(hooks, nodes=[player(1, [3205, 3613, 0], familiar=2), familiar(2, [3000, 3000, 0], combatFamiliar=True)],
                zones=[small_wild()], failures={event: at})
    for event in ['attackOption', 'aggressive', 'aggressiveHandler']:
        add([a('hook', zone='w', hook='enter', node=1)], nodes=[npc(1, [3205, 3613, 0], aggressive=True)],
            zones=[small_wild()], failures={event: 1})
    add([a('hook', zone='w', hook='enter', node=1), a('hook', zone='w', hook='leave', node=1)],
        nodes=[player(1, [3205, 3613, 0], hasFamiliar=True)], zones=[small_wild()])
    add([a('hook', zone='w', hook='enter', node=1)], nodes=[player(1, None)], zones=[small_wild()])
    add([a('hook', zone='w', hook='enter', node=1)], nodes=[player(1, None, artificial=True)], zones=[small_wild()])
    add([a('hook', zone='w', hook='locationUpdate', node=1, last=[1, 2, 3]) for _ in range(2)],
        nodes=[player(1, [3205, 9990, 0])], zones=[small_wild()])
    add([a('hook', zone='w', hook='locationUpdate', node=1, last=None)],
        nodes=[player(1, [3205, 9990, 0], artificial=True)], zones=[small_wild()])
    add([a('hook', zone='w', hook='locationUpdate', node=1, last=None)], nodes=[npc(1, [3205, 9990, 0])], zones=[small_wild()])
    # The old overlay refusing to close keeps the overlay; closing a fresh 381 leaves the manager's field.
    add([a('openOverlay', node=1, component=96), a('show', node=1), a('leavePlayer', zone='w', node=1)],
        nodes=[player(1, [3205, 3613, 0])], zones=[small_wild()], answers=dict(componentClose=[False, True]))
    add([a('show', node=1), a('leavePlayer', zone='w', node=1), a('show', node=1)],
        nodes=[player(1, [3205, 3613, 0])], zones=[small_wild()], answers=dict(componentClose=[True, False]))
    # The real INSTANCE with its five borders: enter at the south edge, walk north, leave, then underground.
    instance = [a('configure', zone='w')]
    for y in [3524, 3525, 3527, 3528, 3600, 3975, 3976]:
        instance += [a('set', node=1, location=[3100, y, 0]), a('update', node=1, last=None)]
    for location in [[3069, 9930, 0], [3070, 9930, 0], [3100, 10002, 1], [3100, 10003, 0], [3030, 10250, 2], [2950, 10320, 0]]:
        instance += [a('set', node=1, location=location), a('update', node=1, last=None)]
    add(instance, nodes=[player(1, [3100, 3500, 0], playing=True)], zones=[dict(id='w')])
    add(instance, nodes=[npc(1, [3100, 3500, 0], aggressive=True)], zones=[dict(id='w')])
    # A callback that clears the monitor while the walk is leaving.
    add(walk, nodes=[player(1, out)], zones=[small_wild()],
        hooks=[dict(event='optionRemove', at=1, actions=[a('clear', node=1)])])
    add(walk, nodes=[player(1, out)], zones=[small_wild(), dict(id='p', **{'class': 'plain'})],
        hooks=[dict(event='sendString', at=1, actions=[a('register', zone='p', borders=SMALL)])])


def combat(cases):
    def add(actions, **kw):
        cases.append(dict(group='attack', actions=actions, **kw))

    def attack(target, message=True, style='MELEE'):
        return a('hook', zone='w', hook='attack', node=1, target=dict(node=target), style=style, message=message)

    for level, other in [(5, 5), (5, 9), (9, 5), (0, 3), (1, 0), (56, 20)]:
        for combat_level in [3, 70, 126]:
            low = min(level, other)
            actions = []
            for target_combat in sorted({combat_level - low - 1, combat_level - low, combat_level - low + 1, combat_level,
                                         combat_level + low - 1, combat_level + low, combat_level + low + 1}):
                actions += [a('set', node=2, combat=target_combat), attack(2), attack(2, message=False)]
            add(actions, nodes=[player(1, [3200, 3600, 0], skullLevel=level, combat=combat_level),
                                player(2, [3200, 3600, 0], skullLevel=other)], zones=[small_wild()])
    for combat_level, level, target_combat in [(MAX, 1, MIN), (MIN, 1, MAX), (MAX, MAX, 0), (MIN, MIN, 0), (0, MIN, 5), (5, -3, 5),
                                               (5, -3, 3), (5, -3, 7)]:
        add([attack(2), attack(2, message=False)],
            nodes=[player(1, [3200, 3600, 0], skullLevel=level, combat=combat_level),
                   player(2, [3200, 3600, 0], skullLevel=MAX, combat=target_combat)], zones=[small_wild()])
    # Revenants: the NPC's wilderness level against the player's skull level.
    for name in ['Revenant imp', 'Revenant knight', 'revenant imp', 'Imp', 'Dark Revenant', None]:
        for npc_y, skull in [(3600, 20), (3700, 5), (3527, 9), (9990, 30), (3400, 2)]:
            actions = []
            for npc_combat in [1, 7, 10, 14, 15, 16, 20, 25, 35, 40, 41, 45, 46, 50, 70]:
                actions += [a('set', node=2, npcCombat=npc_combat), attack(2), attack(2, message=False)]
            add(actions, nodes=[player(1, [3200, 3600, 0], skullLevel=skull, combat=25),
                                npc(2, [3200, npc_y, 0], name=name)], zones=[small_wild()])
    add([attack(2)], nodes=[player(1, [3200, 3600, 0], skullLevel=10, combat=25),
                            familiar(2, [3200, 3700, 0], name='Revenant pet', npcCombat=99)], zones=[small_wild()])
    add([attack(2)], nodes=[player(1, [3200, 3600, 0], skullLevel=10, combat=25), npc(2, None, name='Revenant', npcCombat=99)],
        zones=[small_wild()])
    add([attack(2), attack(3)], nodes=[npc(1, [3200, 3600, 0]), player(2, [3200, 3600, 0], combat=126), npc(3, [3200, 3600, 0], name='Revenant')],
        zones=[small_wild()])
    add([a('hook', zone='w', hook='attack', node=1, target=None, message=True),
         a('hook', zone='w', hook='attack', node=1, target=dict(item=4151), message=True),
         a('hook', zone='w', hook='attack', node=1, target=dict(other=True), message=True)],
        nodes=[player(1, [3200, 3600, 0])], zones=[small_wild()])
    for event in ['skullLevel', 'combatLevel', 'packetMessage', 'npcName', 'location', 'npcCombat']:
        for at in [1, 2, 3]:
            add([attack(2), attack(3)],
                nodes=[player(1, [3200, 3600, 0], skullLevel=5, combat=100), player(2, [3200, 3600, 0], skullLevel=3, combat=3),
                       npc(3, [3200, 3700, 0], name='Revenant ork', npcCombat=1)], zones=[small_wild()], failures={event: at})
    # Through the monitor: the entity's own rule, the zone, the wilderness flags, checkMulti.
    enter = [a('configure', zone='w'), a('update', node=1, last=None), a('update', node=2, last=None)]
    for p1, p2 in [(dict(), dict()), (dict(combat=126), dict(combat=3)), (dict(wilderness=False), dict()), (dict(), dict(skullLevel=1, combat=90))]:
        base1 = dict(dict(skullLevel=12, combat=60, playing=True), **p1)
        base2 = dict(dict(skullLevel=12, combat=60, playing=True), **p2)
        add(enter + [a('set', node=1, wilderness=base1.get('wilderness', True)),
                     a('continueAttack', node=1, target=dict(node=2), style='RANGE', message=True),
                     a('continueAttack', node=1, target=dict(node=3), style='MAGIC', message=True)],
            nodes=[player(1, [3205, 3610, 0], **base1), player(2, [3206, 3610, 0], **base2),
                   npc(3, [3207, 3700, 0], name='Revenant goblin', npcCombat=60)], zones=[small_wild()])


def teleports(cases):
    def add(actions, **kw):
        cases.append(dict(group='teleport', actions=actions, **kw))

    names = {'1704': 'Amulet of glory(4)', '11105': 'Skills necklace(4)', '13281': 'Ring of slaying(8)', '4155': 'Enchanted gem',
             '1712': 'Amulet of Glory', '99': None, '2552': 'Ring of duelling(8)', '1': 'gloryslaying'}
    targets = [None, dict(item=1704), dict(item=13281), dict(item=4155), dict(item=1712), dict(item=2552), dict(item=1),
               dict(node=2), dict(other=True)]
    for rights in ['REGULAR_PLAYER', 'PLAYER_MODERATOR', 'ADMINISTRATOR']:
        for disabled in [False, True]:
            actions = []
            for level in [0, 19, 20, 21, 29, 30, 31, 56, -1, MAX]:
                actions.append(a('set', node=1, skullLevel=level))
                for target in targets:
                    actions.append(a('hook', zone='w', hook='teleport', node=1, type=0, target=target))
            add(actions, nodes=[player(1, [3200, 3700, 0], rights=rights, wildDisabled=disabled), npc(2, [3200, 3700, 0])],
                zones=[small_wild()], names=names)
    add([a('hook', zone='w', hook='teleport', node=2, type=0, target=None),
         a('hook', zone='w', hook='teleport', node=1, type=0, target=dict(item=99))],
        nodes=[player(1, [3200, 3700, 0], skullLevel=25), npc(2, [3200, 3700, 0])], zones=[small_wild()], names=names)
    for level in [0, 20, 21, 30, 31]:
        add([a('checkTeleport', node=1, level=20), a('checkTeleport', node=1, level=30), a('checkTeleport', node=1, level=MIN),
             a('checkTeleport', node=1, level=MAX)], nodes=[player(1, [3200, 3700, 0], skullLevel=level)])
    for event in ['rights', 'itemGetName', 'skullLevel', 'wildernessDisabled', 'packetMessage']:
        for at in [1, 2]:
            add([a('hook', zone='w', hook='teleport', node=1, type=0, target=dict(item=13281)),
                 a('hook', zone='w', hook='teleport', node=1, type=0, target=dict(item=4155))],
                nodes=[player(1, [3200, 3700, 0], skullLevel=40)], zones=[small_wild()], names=names, failures={event: at})
    # ZoneMonitor.teleport: the tele-block and jewellery rule before the zone; type -1 skips the tele-block.
    enter = [a('configure', zone='w'), a('update', node=1, last=None)]
    for state in [dict(), dict(teleTimer=True), dict(teleportLocked=True), dict(teleportLocked=True, rights='ADMINISTRATOR')]:
        for skull in [5, 20, 21, 30, 31]:
            actions = list(enter) + [a('set', node=1, skullLevel=skull)]
            for kind in [-1, 0, 1, 2]:
                for target in [None, dict(item=1704), dict(item=4155)]:
                    actions.append(a('teleport', node=1, type=kind, target=target))
            add(actions, nodes=[player(1, [3205, 3610, 0], playing=True, **state)], zones=[small_wild()], names=names)


def deaths(cases):
    def add(actions, **kw):
        kw.setdefault('zones', [small_wild()])
        cases.append(dict(group='death', actions=actions, **kw))

    names = {str(i): f'Brawling gloves ({n})' for i, n in zip(range(13845, 13858), ['melee', 'ranged', 'magic', 'prayer', 'agility', 'wc', 'fm',
                                                                                       'mining', 'hunter', 'thieving', 'smithing', 'fishing', 'cooking'])}
    names.update({'13887': "Vesta's chainbody", '13879': "Morrigan's javelin", '13883': "Morrigan's throwing axe", '13858': None})
    death = lambda killer=2: a('hook', zone='w', hook='death', node=1, killer=killer)
    for npc_id, name in [(3200, 'Chaos Elemental'), (6604, 'Revenant imp'), (6605, 'REVENANT'), (1, 'Man'), (6606, None)]:
        for combat_level in [1, 50, 100, 300, 396, 397, 398, 1000]:
            for deep, risk in [(False, None), (True, 100000), (True, 100001), (True, None), (False, 5000000)]:
                nodes = [npc(1, [3200, 3700, 0], npcId=npc_id, name=name, npcCombat=combat_level, drop=[3201, 3701, 0]),
                         player(2, [3200, 3700, 0], deep=deep, risk=risk, username='Zezima')]
                add([death()], nodes=nodes, names=names, seed=combat_level * 7 + npc_id)
    for seed in range(40):
        add([death() for _ in range(5)],
            nodes=[npc(1, [3200, 3700, 0], npcId=3200, name='Chaos Elemental', npcCombat=305, drop=[3201, 3701, 0]),
                   player(2, [3200, 3700, 0], username=None if seed % 5 == 0 else 'Zezima')], names=names, seed=seed)
    for seed in range(20):
        add([death() for _ in range(4)],
            nodes=[npc(1, [3200, 3700, 0], npcId=6610, name='Revenant dragon', npcCombat=135 + seed * 13),
                   player(2, [3200, 3700, 0], deep=seed % 2 == 0, risk=200000)], names=names, seed=1000 + seed)
    # Revenants whose gear rate is a few hundred: the 16-roll glove rate is small enough to hit.
    for combat_level in range(378, 397, 2):
        for seed in range(3):
            add([death() for _ in range(6)],
                nodes=[npc(1, [3200, 3700, 0], npcId=6611, name='Revenant ork', npcCombat=combat_level),
                       player(2, [3200, 3700, 0], deep=seed == 2, risk=100000 + seed)], names=names, seed=combat_level * 3 + seed)
    add([death(None), death(3), death(1), a('hook', zone='w', hook='death', node=2, killer=1)],
        nodes=[npc(1, [3200, 3700, 0], npcId=3200, npcCombat=400), player(2, [3200, 3700, 0]), npc(3, [3200, 3700, 0])])
    add([death(), death()], nodes=[familiar(1, [3200, 3700, 0], npcId=6604, name='Revenant pet', npcCombat=400),
                                   player(2, [3200, 3700, 0])], names=names)
    for event in ['id', 'npcName', 'npcCombat', 'deepWilderness', 'attribute', 'dropLocation', 'groundItem', 'username',
                  'itemGetName', 'news']:
        for at in [1, 2, 3]:
            add([death()], nodes=[npc(1, [3200, 3700, 0], npcId=3200, name='Chaos Elemental', npcCombat=400),
                                  player(2, [3200, 3700, 0], deep=True, risk=1)], names=names, failures={event: at}, seed=3)
    # Through the monitor: handleDeath asks every zone; the wilderness never takes over.
    add([a('configure', zone='w'), a('update', node=1, last=None), a('death', node=1, killer=2), a('death', node=2, killer=1)],
        nodes=[npc(1, [3205, 3610, 0], npcId=6604, name='Revenant imp', npcCombat=400), player(2, [3205, 3610, 0])],
        zones=[small_wild()], names=names)


def interactions(cases):
    def add(actions, **kw):
        cases.append(dict(group='interact', actions=actions, **kw))

    def wild(target, node=1):
        return a('hook', zone='w', hook='interact', node=node, target=target, option='Attack')

    add([wild(dict(node=2)), wild(dict(node=3)), wild(dict(node=4)), wild(None), wild(dict(item=4151)), wild(dict(other=True)),
         wild(dict(node=5)), wild(dict(node=6)), wild(dict(node=2), node=3)],
        nodes=[player(1, [3200, 3700, 0]), npc(2, [3200, 3700, 0], name='Revenant hellhound'), npc(3, [3200, 3700, 0], name='Hellhound'),
               player(4, [3200, 3700, 0]), npc(5, [3200, 3700, 0], name=None), familiar(6, [3200, 3700, 0], name='a Revenant')],
        zones=[small_wild()])
    for event in ['npcName', 'attack']:
        add([wild(dict(node=2))], nodes=[player(1, [3200, 3700, 0]), npc(2, [3200, 3700, 0], name='Revenant')],
            zones=[small_wild()], failures={event: 1})
    add([a('configure', zone='w'), a('update', node=1, last=None), a('interact', node=1, target=dict(node=2), option='Attack'),
         a('interact', node=1, target=dict(node=3), option='Attack')],
        nodes=[player(1, [3205, 3610, 0], playing=True), npc(2, [3205, 3611, 0], name='Revenant cyclops'), npc(3, [3205, 3611, 0])],
        zones=[small_wild()])

    def dark(target, option, node=1):
        return a('hook', zone='d', hook='interact', node=node, target=target, option=option)

    for options in [['drop', 'Drop', 'DROP', 'extinguish', 'Extinguish', 'EXTINGUISH', 'Light', 'Use', 'dropp', ' drop']]:
        for ids in [list(LIT.values()), UNLIT + [0, -1, 4151]]:
            actions = []
            for item in ids:
                for option in options:
                    actions.append(dark(dict(item=item), option))
            add(actions, nodes=[player(1, INSIDE_DARK)], zones=[dict(id='d', **{'class': 'dark'})])
    add([dark(dict(item=33), None), dark(dict(item=36), None), dark(dict(item=33), 'drop', node=2), dark(dict(item=33), 'Light', node=2),
         dark(dict(node=1), 'drop'), dark(None, 'drop'), dark(dict(other=True), 'drop')],
        nodes=[player(1, INSIDE_DARK), npc(2, INSIDE_DARK)], zones=[dict(id='d', **{'class': 'dark'})])
    add([a('configure', zone='d'), a('update', node=1, last=None), a('interact', node=1, target=dict(item=594), option='Drop'),
         a('interact', node=1, target=dict(item=4151), option='Drop')],
        nodes=[player(1, INSIDE_DARK, playing=True, inventory=[594])], zones=[dict(id='d', **{'class': 'dark'})])


def darkness(cases):
    def add(actions, **kw):
        cases.append(dict(group='dark', actions=actions, **kw))

    zone = dict(id='d', **{'class': 'dark'}, )
    enter = [a('configure', zone='d'), a('update', node=1, last=None)]
    leave = [a('set', node=1, location=OUTSIDE), a('update', node=1, last=INSIDE_DARK)]
    holdings = [dict(), dict(inventory=[LIT['candle']]), dict(inventory=[None, 4151, LIT['torch']]), dict(inventory=[LIT['bullseye']]),
                dict(inventory=[LIT['oil_lamp'], LIT['candle']]), dict(equipment=[None, LIT['mining_helmet']]),
                dict(inventory=UNLIT, equipment=[LIT['emerald']]), dict(inventory=[LIT['sapphire']], equipment=[LIT['candle']]),
                dict(glow=True), dict(glow=True, inventory=[LIT['candle']])]
    for holding in holdings:
        nodes = [player(1, INSIDE_DARK, playing=True, **holding)]
        add(enter + [a('tick', count=3)] + leave + [a('tick', count=2)], nodes=nodes, zones=[zone])
    # The insect pulse: messages at its first and sixth run, a hit every run after, every two ticks.
    add(enter + [a('tick', count=20)] + leave + [a('tick', count=4)], nodes=[player(1, INSIDE_DARK, playing=True)], zones=[zone])
    add(enter + [a('tick', count=7), a('set', node=1, active=False), a('tick', count=3)], nodes=[player(1, INSIDE_DARK, playing=True)],
        zones=[zone])
    add(enter + [a('tick', count=16)], nodes=[player(1, INSIDE_DARK, playing=True)], zones=[zone], failures=dict(manualHit=2))
    add(enter + [a('tick', count=4)], nodes=[player(1, INSIDE_DARK, playing=True)], zones=[zone], failures=dict(packetMessage=1))
    # DARKNESS_OVERLAY directly: a running pulse is not replaced, a stopped one is.
    add([a('darkOpen', node=1), a('darkOpen', node=1), a('darkClose', node=1), a('darkOpen', node=1), a('tick', count=3),
         a('darkClose', node=1), a('tick', count=3)], nodes=[player(1, INSIDE_DARK)])
    add([a('darkOpen', node=1), a('darkClose', node=1), a('darkClose', node=1), a('tick', count=3)], nodes=[player(1, INSIDE_DARK)],
        answers=dict(componentClose=[False, True]))
    for event in ['getExtension', 'addExtension', 'componentOpen', 'componentClose']:
        add([a('darkOpen', node=1), a('darkClose', node=1), a('tick', count=3)], nodes=[player(1, INSIDE_DARK)], failures={event: 1})
    # updateOverlay over every source and every current overlay, with and without the perk.
    for holding in holdings:
        for current in [None, 'darkness', 97, 98, 381]:
            actions = []
            if current is not None:
                actions.append(a('openOverlay', node=1, component=current))
            actions += [a('updateOverlay', zone='d', node=1), a('tick', count=2)]
            if holding.get('glow'):
                actions.append(a('runScript', index=0, stage=0))
            add(actions, nodes=[player(1, INSIDE_DARK, **holding)], zones=[zone])
    add([a('openOverlay', node=1, component='darkness'), a('updateOverlay', zone='d', node=1), a('closeOverlay', node=1),
         a('runScript', index=0)], nodes=[player(1, INSIDE_DARK, glow=True)], zones=[zone])
    add([a('openOverlay', node=1, component='darkness'), a('updateOverlay', zone='d', node=1), a('runScript', index=0)],
        nodes=[player(1, INSIDE_DARK, glow=True)], zones=[zone], answers=dict(stopExecuting=[False]))
    # checkDarkArea and the tinderbox hook.
    names = {'590': 'Tinderbox', '36': 'Candle', '1': None, '2': 'tinderbox'}
    for used, with_ in [(590, 36), (36, 590), (36, 36), (2, 36), (1, 590), (590, 1), (36, 1)]:
        add(enter + [a('process', zone='d', node=1, used=used, **{'with': with_}), a('runTask', index=0), a('tick', count=2)],
            nodes=[player(1, INSIDE_DARK, playing=True, inventory=[LIT['candle']])], zones=[zone], names=names)
    add([a('process', zone='d', node=2, used=590, **{'with': 36})], nodes=[player(1, INSIDE_DARK), npc(2, INSIDE_DARK)],
        zones=[zone], names=names)
    add(enter + [a('checkDark', node=1), a('set', node=1, inventory=[]), a('checkDark', node=1)],
        nodes=[player(1, INSIDE_DARK, playing=True, inventory=[LIT['oil_lantern']])], zones=[zone])
    add([a('checkDark', node=1)], nodes=[player(1, INSIDE_DARK)], zones=[zone])
    add([a('configure', zone='p'), a('configure', zone='d'), a('update', node=1, last=None), a('checkDark', node=1),
         a('set', node=1, glow=True), a('checkDark', node=1), a('runScript', index=0)],
        nodes=[player(1, INSIDE_DARK, playing=True)], zones=[dict(id='p', **{'class': 'plain'}), zone])
    # Two DarkZones entered: only the first updates the overlay.
    add([a('configure', zone='d'), a('configure', zone='e'), a('update', node=1, last=None), a('checkDark', node=1)],
        nodes=[player(1, INSIDE_DARK, playing=True, inventory=[LIT['torch']])], zones=[zone, dict(id='e', **{'class': 'dark'})])
    # A plain zone registered over the dark area, entered first.
    add([a('register', zone='p', borders=DARK), a('configure', zone='d'), a('update', node=1, last=None), a('checkDark', node=1)],
        nodes=[player(1, INSIDE_DARK, playing=True)], zones=[dict(id='p', **{'class': 'plain'}), zone])
    # NPCs and other entities only hook and unhook.
    add(enter + leave, nodes=[npc(1, INSIDE_DARK)], zones=[zone])
    add(enter + leave, nodes=[dict(id=1, kind='other', location=INSIDE_DARK)], zones=[zone])
    # continueAttack: refused only while the overlay is DARKNESS_OVERLAY itself.
    for current in [None, 'darkness', 96, 97]:
        actions = [] if current is None else [a('openOverlay', node=1, component=current)]
        actions += [a('hook', zone='d', hook='attack', node=1, target=dict(node=2), message=True),
                    a('hook', zone='d', hook='attack', node=2, target=dict(node=1), message=True)]
        add(actions, nodes=[player(1, INSIDE_DARK), npc(2, INSIDE_DARK)], zones=[zone])
    add(enter + [a('continueAttack', node=1, target=dict(node=2), style='MELEE', message=True)],
        nodes=[player(1, INSIDE_DARK, playing=True), npc(2, INSIDE_DARK)], zones=[zone])
    # Each event of enter and leave failing.
    for event in ['toArray', 'perk', 'openOverlay', 'getExtension', 'addExtension', 'componentOpen', 'hook', 'closeOverlay',
                  'closeComponent', 'componentClose', 'unhook', 'getOverlay', 'queueScript']:
        add([a('hook', zone='d', hook='enter', node=1), a('updateOverlay', zone='d', node=1),
             a('hook', zone='d', hook='leave', node=1, logout=False)],
            nodes=[player(1, INSIDE_DARK, glow=event == 'queueScript')], zones=[zone], failures={event: 1})
    add([a('hook', zone='d', hook='leave', node=1, logout=True)], nodes=[player(1, INSIDE_DARK)], zones=[zone])
    # A callback that clears the monitor while checkDarkArea walks it.
    add(enter + [a('checkDark', node=1)], nodes=[player(1, INSIDE_DARK, playing=True)], zones=[zone],
        hooks=[dict(event='toArray', at=3, actions=[a('clear', node=1)])])


def lights(cases):
    def add(actions, **kw):
        cases.append(dict(group='light', actions=actions, **kw))

    ids = sorted(set(LIT.values()) | set(UNLIT) | {0, -1, 4151, 590, MAX, MIN})
    add([a('lightTable')] + [a('lightFor', id=i) for i in ids])
    holdings = [dict(), dict(inventory=[None, None]), dict(inventory=[4151, 36]), dict(inventory=[4151, 33]),
                dict(inventory=[5013, 33]), dict(equipment=[4550]), dict(inventory=[36], equipment=[None, 9065, 33]),
                dict(inventory=[4702], equipment=[33])]
    for holding in holdings:
        add([a('activeLight', node=1), a('hasLight', node=1)], nodes=[player(1, INSIDE_DARK, **holding)])
    add([a('activeLight', node=1)], nodes=[player(1, INSIDE_DARK, inventory=[36])], failures=dict(toArray=2))


def startup(cases):
    cases.append(dict(group='startup', actions=[a('startup'), a('zoneInfo', zone='w')], zones=[dict(id='w')]))
    cases.append(dict(group='startup', actions=[a('configure', zone='d'), a('zoneInfo', zone='d')],
                      zones=[dict(id='d', **{'class': 'dark'})]))
    # Tiles in each registered dark area, entered through the monitor.
    actions = [a('configure', zone='d')]
    for location in [[1728, 5120, 0], [1791, 5247, 3], [1727, 5120, 0], [3222, 9600, 0], [3306, 9661, 0], [3307, 9661, 0],
                     [3717, 9346, 0], [3841, 9473, 1], [3200, 9600, 0]]:
        actions += [a('set', node=1, location=location), a('update', node=1, last=None)]
    cases.append(dict(group='startup', actions=actions, nodes=[npc(1, [0, 0, 0])], zones=[dict(id='d', **{'class': 'dark'})]))


def requests():
    cases = []
    levels(cases)
    entering(cases)
    combat(cases)
    teleports(cases)
    deaths(cases)
    interactions(cases)
    darkness(cases)
    lights(cases)
    startup(cases)
    return cases
