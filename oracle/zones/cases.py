"""The designed corpus of the zones and region-activity oracle.

Each case is one fresh world: nodes, zones, an action list and optional
scripted answers, hooks (actions run when an event is emitted) and failures
(an event that throws on its n-th call).
"""

REGION = 12850  # tiles 3200..3263 on both axes
NORTH = 12851
EAST = 13106
MAX = 2147483647
MIN = -2147483648


def a(op, **kw):
    return dict(op=op, **kw)


def player(id, location, **kw):
    return dict(id=id, kind='player', location=location, **kw)


def npc(id, location, **kw):
    return dict(id=id, kind='npc', location=location, **kw)


def familiar(id, location, **kw):
    return dict(id=id, kind='familiar', location=location, **kw)


def zone(id, cls='scripted', **kw):
    return dict(id=id, **{'class': cls}, **kw)


def borders(id, *args, **kw):
    return a('newBorders', id=id, args=list(args), **kw)


def groups(cases):
    counts = {}
    for case in cases:
        counts[case['group']] = counts.get(case['group'], 0) + 1
    return dict(sorted(counts.items()))


def edges(low, high):
    return sorted({low - 1, low, low + 1, high - 1, high, high + 1})


def geometry(cases):
    def add(actions, **kw):
        cases.append(dict(group='borders', actions=actions, **kw))

    specs = [
        dict(args=[3200, 3210, 3210, 3220]),
        dict(args=[3210, 3220, 3200, 3210]),
        dict(args=[3205, 3205, 3205, 3205]),
        dict(args=[-5, -7, 3, 2]),
        dict(args=[3200, 3210, 3210, 3220, 0]),
        dict(args=[3200, 3210, 3210, 3220, 1]),
        dict(args=[3200, 3210, 3210, 3220, 2], zeroCheck=False),
        dict(args=[3200, 3210, 3210, 3220, 0], zeroCheck=True),
        dict(args=[3200, 3210, 3210, 3220, 3], zeroCheck=True),
        dict(args=[3200, 3210, 3210, 3220, 5]),
        dict(args=[3200, 3210, 3210, 3220, -1], zeroCheck=True),
        dict(args=[MIN, MIN, MAX, MAX]),
    ]
    for spec in specs:
        x1, y1, x2, y2 = spec['args'][:4]
        low_x, high_x, low_y, high_y = min(x1, x2), max(x1, x2), min(y1, y2), max(y1, y2)
        actions = [a('newBorders', id='b', **spec), a('corners', borders='b')]
        if low_x == MIN:
            probes = [[0, 0, 0], [MIN, MAX, 3], [MAX, MIN, 1], [-1, -1, 2]]
        else:
            probes = [[x, y, z] for x in edges(low_x, high_x) for y in edges(low_y, high_y) for z in [0, 1, 2, 3]]
            probes += [[low_x, low_y, z] for z in [-1, 4, 5, 7]]
        actions += [a('inside', borders='b', at=at) for at in probes]
        actions += [a('insideXY', borders='b', at=[x, y]) for x in [low_x, high_x] for y in [low_y, high_y]]
        if low_x != MIN:
            actions += [a('regionIds', borders='b')]
        actions += [a('bordersHash', borders='b'), a('bordersString', borders='b')]
        add(actions)

    # Exceptions, their planes, and an exception of an exception.
    setup = [borders('o', 3200, 3200, 3263, 3263), borders('e1', 3210, 3210, 3220, 3220), borders('e2', 3230, 3230, 3240, 3240, 1),
             borders('e3', 3212, 3212, 3214, 3214), a('corners', borders='o'), a('bordersString', borders='o'), a('bordersHash', borders='o'),
             a('addException', borders='o', exception='e1'), a('corners', borders='o'), a('addException', borders='o', exception='e2')]
    probes = [[x, y, 0] for x in edges(3210, 3220) for y in [3209, 3210, 3215, 3220, 3221]]
    probes += [[x, 3235, z] for x in [3229, 3230, 3240, 3241] for z in [0, 1, 2]]
    probes += [[3213, 3213, 0], [3211, 3213, 0]]
    inside = [a('inside', borders='o', at=at) for at in probes]
    add(setup + inside + [a('addException', borders='e1', exception='e3')] + inside
        + [a('bordersString', borders='o'), a('bordersHash', borders='o'), a('corners', borders='o'), a('corners', borders='e1')])
    # The same exception object twice, and an exception wider than its owner.
    add([borders('o', 3200, 3200, 3210, 3210), borders('e', 3190, 3190, 3220, 3220), a('addException', borders='o', exception='e'),
         a('addException', borders='o', exception='e'), a('corners', borders='o'), a('inside', borders='o', at=[3205, 3205, 0]),
         a('setPlane', borders='e', plane=2), a('inside', borders='o', at=[3205, 3205, 0]), a('inside', borders='o', at=[3205, 3205, 2]),
         a('bordersString', borders='o'), a('bordersHash', borders='o')])

    # equals and hashCode, field by field.
    base = [3200, 3210, 3210, 3220]
    others = [
        ([3200, 3210, 3210, 3220], {}),
        ([3210, 3220, 3200, 3210], {}),
        ([3201, 3210, 3210, 3220], {}),
        ([3200, 3211, 3210, 3220], {}),
        ([3200, 3210, 3211, 3220], {}),
        ([3200, 3210, 3210, 3221], {}),
        ([3200, 3210, 3210, 3220, 0], {}),
        ([3200, 3210, 3210, 3220, 1], {}),
        ([3200, 3210, 3210, 3220, 0], dict(zeroCheck=True)),
        ([3200, 3210, 3210, 3220, 0], dict(zeroCheck=False)),
    ]
    for args, extra in others:
        add([borders('x', *base), a('newBorders', id='y', args=args, **extra), a('bordersEquals', a='x', b='y'), a('bordersEquals', a='y', b='x'),
             a('bordersEquals', a='x', b='x'), a('bordersHash', borders='x'), a('bordersHash', borders='y')])
    for left, right in [(['e'], ['e']), (['e'], ['f']), (['e'], []), ([], ['e']), (['e', 'f'], ['e', 'f']), (['e', 'f'], ['f', 'e']), (['e'], ['g']),
                        (['e'], ['e', 'e'])]:
        actions = [borders('x', *base), borders('y', *base), borders('e', 1, 1, 2, 2), borders('f', 1, 1, 2, 3), borders('g', 1, 1, 2, 2)]
        actions += [a('addException', borders='x', exception=e) for e in left] + [a('addException', borders='y', exception=e) for e in right]
        actions += [a('bordersEquals', a='x', b='y'), a('bordersEquals', a='y', b='x'), a('bordersHash', borders='x'), a('bordersHash', borders='y'),
                    a('bordersString', borders='x'), a('bordersString', borders='y')]
        add(actions)

    # The Location constructor and the region id forms.
    for first, second in [([3200, 3210, 0], [3210, 3220, 0]), ([3210, 3220, 2], [3200, 3210, 1]), ([3200, 3210, 3], [3200, 3210, 0]),
                          ([-3, -4, 1], [5, 6, 2])]:
        add([a('between', id='b', a=first, b=second), a('corners', borders='b')]
            + [a('inside', borders='b', at=[first[0], first[1], z]) for z in [0, 1, 2, 3]] + [a('bordersHash', borders='b')])
    for region in [REGION, 0, 255, 256, 65535, 65536 + REGION, -1, MIN, MAX, 13105]:
        add([a('forRegion', id='b', region=region), a('corners', borders='b'), a('regionIds', borders='b'), a('bordersString', borders='b')])
    for args in [[3200, 3200, 3263, 3263], [3200, 3200, 3264, 3263], [3199, 3200, 3263, 3264], [3136, 3136, 3327, 3327], [3200, 3200, 3200, 3200],
                 [-1, -1, 0, 0], [-64, -65, -1, 63], [0, 0, 16383, 63], [3263, 3263, 3264, 3264], [100, 3200, 500, 3210]]:
        add([borders('b', *args), a('regionIds', borders='b')])

    # Node and location forms.
    nodes = [player(1, [3207, 3216, 0]), npc(2, [3207, 3216, 1]), player(3, None), npc(4, [3200, 3210, 3])]
    actions = [borders('b', 3200, 3210, 3210, 3220), borders('p', 3200, 3210, 3210, 3220, 1), borders('c', 400, 401, 400, 402),
               borders('q', 400, 401, 400, 402, 1)]
    for name in ['b', 'p', 'c', 'q']:
        actions += [a('insideLocation', borders=name, location=l) for l in [[3200, 3210, 0], [3200, 3210, 1], [3211, 3210, 0], None]]
        actions += [a('insideNode', borders=name, node=n) for n in [1, 2, 3, 4]]
        actions += [a('insideRegion', borders=name, node=n) for n in [1, 2, 3, 4]]
    add(actions, nodes=nodes)
    add([borders('b', 3200, 3210, 3210, 3220), a('insideNode', borders='b', node=1), a('insideRegion', borders='b', node=1)],
        nodes=[player(1, [3207, 3216, 0])], failures=dict(location=1))
    add([borders('c', 400, 401, 400, 402), a('insideRegion', borders='c', node=1)], nodes=[player(1, [3207, 3216, 0])], failures=dict(location=2))
    add([borders('c', 400, 401, 400, 402), a('insideRegion', borders='c', node=1)], nodes=[player(1, [3207, 3216, 0])],
        hooks=[dict(event='location', actions=[a('set', node=1, location=[3200, 3300, 0])])])


def randoms(cases):
    def add(actions, **kw):
        cases.append(dict(group='random', actions=actions, **kw))

    shapes = [[3205, 3205, 3205, 3205], [3200, 3205, 3210, 3205], [3205, 3200, 3205, 3230], [3200, 3210, 3203, 3212], [0, 0, 16000, 9000],
              [-40, -20, 40, 20], [3200, 3210, 3201, 3210]]
    for seed in [1, 530, 12345, -77]:
        for shape in shapes:
            actions = [borders('b', *shape)]
            for intensity in [1, 2, 3, 10, 1, -1, -3]:
                actions.append(a('weighted', borders='b', intensity=intensity))
            add(actions, seed=seed)
    for shape in shapes:
        add([borders('b', *shape), a('weighted', borders='b', intensity=0), a('weighted', borders='b', intensity=2)])
    for shape in [[MIN, 0, MAX, 0], [0, MIN, 0, MAX], [0, 0, MAX, 5], [MIN, MIN, MAX, MAX], [-1, 0, MAX, 3]]:
        add([borders('b', *shape), a('weighted', borders='b', intensity=1), a('weighted', borders='b', intensity=3),
             a('randomLoc', borders='b', count=1), a('randomWalkable', borders='b')])

    for shape in [[3205, 3205, 3205, 3205], [3200, 3205, 3201, 3205], [3205, 3200, 3205, 3202], [3200, 3210, 3201, 3212], [3200, 3210, 3260, 3250],
                  [-2, -2, 0, -1]]:
        add([borders('b', *shape), a('randomLoc', borders='b', count=96)])
        for plane, check in [(1, None), (2, True), (5, None), (-1, None), (4, True)]:
            extra = {} if check is None else dict(zeroCheck=check)
            add([a('newBorders', id='b', args=shape + [plane], **extra), a('randomLoc', borders='b', count=96)])
    for permitted in [[True], [False, True], [False, False, False, True], [False] * 19 + [True], [False] * 20 + [True], [False] * 21 + [True], [False]]:
        for shape in [[3205, 3205, 3205, 3205], [3200, 3210, 3201, 3212], [3200, 3210, 3260, 3250]]:
            add([borders('b', *shape), a('randomWalkable', borders='b'), a('randomWalkable', borders='b')], permitted=permitted)
    add([borders('b', 3200, 3210, 3201, 3212), a('randomWalkable', borders='b')], permitted=[False], failures=dict(teleportPermitted=3))


def info(*ids):
    return [a('zoneInfo', zone=id) for id in ids]


def zone_base(cases):
    def add(actions, **kw):
        cases.append(dict(group='zone', actions=actions, **kw))

    add([a('enums')])
    all_names = ['FOLLOWERS', 'RANDOM_EVENTS', 'FIRES', 'MEMBERS', 'CANNON', 'GRAVES', 'TELEPORT', 'OFF_MAP']
    for restrictions in [[], ['FOLLOWERS'], ['TELEPORT', 'OFF_MAP'], all_names, ['FIRES', 'FIRES']]:
        for cls in ['scripted', 'plain']:
            add(info('A') + [a('zoneRestricted', zone='A', flag=flag) for flag in [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 255, -1, 3]],
                zones=[zone('A', cls, restrictions=restrictions, overlappable=cls == 'plain')])
    for name in ['', 'a', 'Wilderness', 'Moderator Zone', 'zöne 中文', 'pair 😀', 'Aa', 'BB', None]:
        add(info('A') + [a('configure', zone='A')] + info('A'), zones=[zone('A', name=name)])
    add(info('A') + [a('zoneSet', zone='A', uid=77), a('zoneInfo', zone='A'), a('zoneSet', zone='A', name='renamed'), a('zoneInfo', zone='A'),
                     a('zoneSet', zone='A', overlappable=False, type=3), a('zoneInfo', zone='A'), a('zoneSet', zone='A', restriction=6),
                     a('zoneSet', zone='A', restrictionName='CANNON'), a('zoneInfo', zone='A'), a('zoneSet', zone='A', fire=True), a('zoneInfo', zone='A'),
                     a('zoneSet', zone='A', disableRandoms=True), a('zoneInfo', zone='A'), a('zoneSet', zone='A', name=None), a('zoneInfo', zone='A'),
                     a('configure', zone='A'), a('zoneSet', zone='A', restriction=-1, type=-5, uid=MIN), a('zoneInfo', zone='A')],
        zones=[zone('A')])

    pairs = [(zone('A'), zone('B')), (zone('A', name='same'), zone('B', name='same')), (zone('A', name='Aa'), zone('B', name='BB')),
             (zone('A', name='same'), zone('B', 'scripted2', name='same')), (zone('A', name='same'), zone('B', 'plain', name='same')),
             (zone('A', 'plain', name='same'), zone('B', 'plain', name='same')), (zone('A', name=None), zone('B', name=None)),
             (zone('A', name='same'), zone('B', name=None)), (zone('A', name=None), zone('B', 'plain', name=None)),
             (zone('A', name='same', restrictions=['FIRES']), zone('B', name='same', overlappable=False))]
    for left, right in pairs:
        actions = [a('zoneEquals', a='A', b='B'), a('zoneEquals', a='B', b='A'), a('zoneEquals', a='A', b='A')]
        actions += [borders('x', 1, 2, 3, 4), borders('y', 1, 2, 3, 4), borders('w', 1, 2, 3, 5)]
        for za, ba, zb, bb in [('A', 'x', 'B', 'x'), ('A', 'x', 'B', 'y'), ('A', 'x', 'A', 'y'), ('A', 'x', 'A', 'w'), ('A', 'x', 'A', 'x'),
                               ('B', 'w', 'A', 'x')]:
            actions.append(a('regionZone', za=za, ba=ba, zb=zb, bb=bb))
        add(actions, zones=[left, right])

    # Every overridable member of MapZone on a zone that overrides nothing.
    hooks = [dict(hook='enter'), dict(hook='leave', logout=False), dict(hook='leave', logout=True), dict(hook='death', killer=2),
             dict(hook='death', killer=None), dict(hook='interact', target=dict(node=2), option='Attack'), dict(hook='interact', target=None, option=None),
             dict(hook='pvp', target=dict(node=1), style='MELEE', message=True), dict(hook='attack', target=dict(item=4151), style='RANGE', message=False),
             dict(hook='ignoreMulti', victim=2), dict(hook='teleport', type=0, target=dict(item=1704)), dict(hook='teleport', type=-1, target=None),
             dict(hook='startDeath', killer=2), dict(hook='move', **{'from': [1, 2, 0], 'to': [3, 4, 1]}), dict(hook='move', **{'from': None, 'to': None}),
             dict(hook='locationUpdate', last=[1, 2, 3]), dict(hook='dynamic')]
    player_hooks = [dict(hook='canLogout'), dict(hook='useWith', used=590, **{'with': dict(other=1)}), dict(hook='button'),
                    dict(hook='request', request='TRADE', other=1), dict(hook='command', name='zone', args=['a', 'b'])]
    nodes = [player(1, [3207, 3216, 0]), npc(2, [3208, 3216, 0]), familiar(3, [3209, 3216, 0], owner=1), dict(id=4, kind='other', location=[1, 1, 0])]
    for cls in ['plain', 'scripted']:
        for restrictions in [[], ['FOLLOWERS'], ['RANDOM_EVENTS', 'TELEPORT']]:
            actions = [a('hook', zone='A', node=n, **h) for n in [1, 2, 3, 4] for h in hooks]
            actions += [a('hook', zone='A', node=1, **h) for h in player_hooks] + [a('configure', zone='A')]
            add(actions, nodes=nodes, zones=[zone('A', cls, restrictions=restrictions)])
    add([a('hook', zone='A', node=3, hook='enter'), a('hook', zone='A', node=3, hook='enter')], nodes=nodes,
        zones=[zone('A', 'plain', restrictions=['FOLLOWERS'])], failures=dict(setInvisible=1))
    add([a('say', node=n, text=text) for n in [1, 2, 3, 4] for text in ['Hello.', '']], nodes=nodes, zones=[zone('A')])
    add([a('say', node=1, text='x'), a('say', node=1, text='y')], nodes=nodes, zones=[zone('A')], failures=dict(packetMessage=1))


def registration(cases):
    def add(actions, **kw):
        cases.append(dict(group='registration', actions=actions, **kw))

    square = borders('s', 3200, 3200, 3220, 3230)
    wide = borders('w', 3250, 3250, 3270, 3270)  # four regions
    add([square, wide, a('register', zone='A', borders='s'), a('register', zone='B', borders='w'), a('register', zone='A', borders='w'),
         a('register', zone='A', borders='s'), a('regionInfo', region=REGION), a('unregister', zone='A', borders='s'), a('regionInfo', region=REGION),
         a('unregister', zone='A', borders='s'), a('unregister', zone='A', borders='s'), a('unregister', zone='B', borders='s'),
         a('regionInfo', region=REGION), a('unregister', zone='A', borders='w'), a('regionInfo', region=EAST)],
        zones=[zone('A'), zone('B')])
    # Structural equality: an equal zone and equal borders remove another object's entry.
    add([square, borders('t', 3200, 3200, 3220, 3230), a('register', zone='A', borders='s'), a('register', zone='B', borders='s'),
         a('unregister', zone='C', borders='t'), a('regionInfo', region=REGION), a('unregister', zone='D', borders='t'), a('regionInfo', region=REGION),
         a('unregister', zone='E', borders='t'), a('regionInfo', region=REGION)],
        zones=[zone('A', name='same'), zone('B', name='other'), zone('C', name='same'), zone('D', 'scripted2', name='other'), zone('E', name='other')])
    add([square, a('register', zone='A', borders='s'), a('unregister', zone='B', borders='s'), a('unregister', zone='A', borders='s')],
        zones=[zone('A', name=None), zone('B', name=None)])
    add([square, a('registerRegion', zone='A', region=REGION), a('registerRegion', zone='A', region=65536 + REGION),
         a('registerRegionBorders', zone='B', region=REGION, borders='s'), a('registerRegionBorders', zone='B', region=NORTH, borders='s'),
         a('registerRegion', zone='A', region=REGION), a('regionInfo', region=REGION), a('unregisterRegion', zone='A', region=REGION),
         a('regionInfo', region=REGION), a('unregisterRegion', zone='A', region=REGION), a('unregisterRegion', zone='B', region=NORTH),
         a('unregisterRegion', zone='B', region=7)],
        zones=[zone('A'), zone('B')])
    # ZoneBuilder.configure runs the zone's own configure, which may register.
    add([square, wide] + info('A') + [a('configure', zone='A')] + info('A') + [a('regionInfo', region=REGION)], zones=[zone('A'), zone('B')],
        hooks=[dict(event='A.configure', actions=[a('register', zone='A', borders='s'), a('registerRegion', zone='B', region=NORTH)])])
    add([square, a('configure', zone='A'), a('configure', zone='A')], zones=[zone('A')], failures={'A.configure': 1})

    # Region.add and Region.remove walk every chunk: x, then y, then plane, NPCs before players.
    nodes = [player(1, [3205, 3205, 0]), player(2, [3260, 3201, 0]), npc(3, [3205, 3205, 0]), npc(4, [3201, 3260, 1]), player(5, [3205, 3205, 2]),
             npc(6, [3300, 3300, 0]), familiar(7, [3206, 3206, 0], owner=1), player(8, [3219, 3229, 3])]
    place = [a('chunkAdd', chunk=n['location'], list='players' if n['kind'] == 'player' else 'npcs', node=n['id']) for n in nodes]
    walk = place + [square, a('register', zone='A', borders='s'), a('registerRegionBorders', zone='B', region=REGION, borders='s'),
                    a('unregister', zone='A', borders='s'), a('unregisterRegion', zone='B', region=REGION), a('update', node=1, last=None)]
    add(walk, nodes=nodes, zones=[zone('A', restrictions=['FOLLOWERS']), zone('B')])
    add(walk, nodes=nodes, zones=[zone('A'), zone('B')], answers={'A.enter': [True, False, True], 'B.enter': [False], 'A.leave': [False, True]})
    for event, at in [('A.enter', 1), ('A.enter', 3), ('location', 2), ('entityUpdate', 4), ('A.locationUpdate', 2), ('A.leave', 2), ('B.enter', 2)]:
        add(walk, nodes=nodes, zones=[zone('A'), zone('B')], failures={event: at})
    # Null list entries: Region.add skips a null player, Region.remove does not, and neither skips a null NPC.
    for kind in ['players', 'npcs']:
        add([a('chunkAdd', chunk=[3205, 3205, 0], list='players', node=1), a('chunkAdd', chunk=[3210, 3205, 0], list=kind, node=None),
             a('chunkAdd', chunk=[3220, 3205, 0], list='players', node=2), square, a('register', zone='A', borders='s'),
             a('unregister', zone='A', borders='s')],
            nodes=[player(1, [3205, 3205, 0]), player(2, [3220, 3205, 0])], zones=[zone('A')])
    # A callback that changes the list being walked.
    for hooked in [a('chunkAdd', chunk=[3205, 3205, 0], list='players', node=2), a('chunkRemove', chunk=[3205, 3205, 0], list='players', node=1),
                   a('chunkClearList', chunk=[3205, 3205, 0], list='players'), a('register', zone='B', borders='s'),
                   a('set', node=1, location=[3300, 3300, 0]), a('unregister', zone='A', borders='s')]:
        add([a('chunkAdd', chunk=[3205, 3205, 0], list='players', node=1), a('chunkAdd', chunk=[3205, 3205, 0], list='players', node=3),
             square, a('register', zone='A', borders='s'), a('regionInfo', region=REGION)],
            nodes=[player(1, [3205, 3205, 0]), player(2, [3206, 3205, 0]), player(3, [3207, 3205, 0])], zones=[zone('A'), zone('B')],
            hooks=[dict(event='A.enter', actions=[hooked])])


def standard(extra=None, **kw):
    """Zones A and B overlap, C lies inside both, D is on plane one, E is dynamic."""
    setup = [borders('bA', 3200, 3200, 3220, 3230), borders('bB', 3210, 3200, 3230, 3230), borders('bC', 3212, 3212, 3215, 3218),
             borders('bD', 3200, 3200, 3230, 3230, 1), borders('bE', 3225, 3210, 3240, 3220),
             a('register', zone='A', borders='bA'), a('register', zone='B', borders='bB'), a('register', zone='C', borders='bC'),
             a('register', zone='D', borders='bD'), a('register', zone='E', borders='bE')]
    world = dict(zones=[zone('A'), zone('B'), zone('C'), zone('D'), zone('E')], answers={'E.dynamic': [True]})
    answers = kw.pop('answers', {})
    world['answers'].update(answers)
    world.update(kw)
    return setup + (extra or []), world


def steps(node, path, start=None):
    actions, last = [], start
    for place in path:
        actions += [a('set', node=node, location=place), a('update', node=node, last=last)]
        last = place
    return actions


def monitor_moves(cases):
    def add(actions, **kw):
        cases.append(dict(group='moves', actions=actions, **kw))

    east = [[x, 3215, 0] for x in [3198, 3199, 3200, 3205, 3209, 3210, 3211, 3212, 3215, 3216, 3220, 3221, 3224, 3225, 3230, 3231, 3240, 3241, 3250]]
    west = list(reversed(east))
    for kind in [player, npc, familiar]:
        setup, world = standard()
        add(setup + steps(1, east) + steps(1, west, east[-1]) + [a('clear', node=1)], nodes=[kind(1, [3198, 3215, 0])], **world)
    # Teleports, plane changes, another region, logout.
    hops = [[3213, 3213, 0], [3213, 3213, 1], [3213, 3213, 2], [3213, 3213, 0], [3300, 3300, 0], [3213, 3213, 0], [3229, 3212, 0], [3229, 3212, 1],
            [3213, 3300, 0], [3220, 3230, 0], [3221, 3231, 0], [3212, 3218, 0], [3212, 3219, 0]]
    for artificial in [False, True]:
        setup, world = standard()
        add(setup + steps(1, hops) + [a('clear', node=1), a('update', node=1, last=None), a('clear', node=1), a('clear', node=1)],
            nodes=[player(1, [3198, 3215, 0], artificial=artificial)], **world)
    # A zone registered for one region only does not reach an entity in the next one.
    add([borders('b', 3250, 3250, 3270, 3270), a('registerRegionBorders', zone='A', region=REGION, borders='b')]
        + steps(1, [[3255, 3255, 0], [3263, 3263, 0], [3264, 3263, 0], [3263, 3264, 0], [3270, 3270, 0], [3262, 3262, 0]]),
        nodes=[player(1, [3255, 3255, 0])], zones=[zone('A')])
    add([borders('b', 3250, 3250, 3270, 3270), a('register', zone='A', borders='b')]
        + steps(1, [[3255, 3255, 0], [3263, 3263, 0], [3264, 3263, 0], [3263, 3264, 0], [3270, 3270, 0], [3271, 3270, 0]]),
        nodes=[player(1, [3255, 3255, 0])], zones=[zone('A')])

    # Refusals: everything before the refusing callback stays done.
    middle = [[3198, 3215, 0], [3213, 3213, 0], [3198, 3215, 0], [3213, 3213, 0]]
    for answers in [{'A.enter': [False]}, {'B.enter': [False]}, {'C.enter': [False, True]}, {'A.leave': [False]}, {'B.leave': [False, True]},
                    {'C.leave': [False]}, {'A.enter': [True, False], 'C.leave': [False]}, {'E.dynamic': [False]}, {'A.dynamic': [True]},
                    {'A.dynamic': [True, False], 'B.dynamic': [True]}]:
        setup, world = standard(answers=answers)
        add(setup + steps(1, middle) + steps(1, [[3230, 3215, 0], [3260, 3215, 0], [3230, 3215, 0]], middle[-1]) + [a('clear', node=1)],
            nodes=[player(1, [3198, 3215, 0])], **world)
    events = ['A.enter', 'B.enter', 'C.enter', 'A.leave', 'B.leave', 'C.leave', 'A.locationUpdate', 'B.locationUpdate', 'C.locationUpdate',
              'entityUpdate', 'location', 'artificial', 'musicPlaying', 'playDefault', 'E.dynamic']
    for event in events:
        for at in [1, 2, 3]:
            setup, world = standard()
            add(setup + steps(1, middle) + [a('clear', node=1)], nodes=[player(1, [3198, 3215, 0])], failures={event: at}, **world)
    for at in [1, 2, 3, 4, 5, 6, 7, 8]:
        setup, world = standard()
        add(setup + steps(1, middle[:2]), nodes=[player(1, [3198, 3215, 0])], failures=dict(location=at), **world)

    # Callbacks that change the lists being walked, or the entity.
    reentrant = [
        ('A.enter', a('removeZone', node=1, zone='A')),
        ('B.enter', a('removeZone', node=1, zone='A')),
        ('B.enter', a('monitorClear', node=1)),
        ('A.enter', a('monitorAdd', node=1, zone='E', borders='bE')),
        ('A.enter', a('register', zone='F', borders='bA')),
        ('C.enter', a('register', zone='F', borders='bA')),
        ('A.enter', a('unregister', zone='B', borders='bB')),
        ('A.enter', a('unregisterRegion', zone='C', region=REGION)),
        ('C.enter', a('unregisterRegion', zone='C', region=REGION)),
        ('A.leave', a('removeZone', node=1, zone='A')),
        ('A.leave', a('removeZone', node=1, zone='B')),
        ('B.leave', a('removeZone', node=1, zone='A')),
        ('C.leave', a('monitorClear', node=1)),
        ('A.leave', a('monitorAdd', node=1, zone='E', borders='bE')),
        ('B.leave', a('register', zone='F', borders='bA')),
        ('A.locationUpdate', a('set', node=1, location=[3300, 3300, 0])),
        ('A.enter', a('set', node=1, location=[3198, 3215, 0])),
        ('A.leave', a('set', node=1, location=[3213, 3213, 0])),
        ('entityUpdate', a('set', node=1, location=[3225, 3215, 0])),
        ('entityUpdate', a('set', node=1, location=None)),
        ('B.locationUpdate', a('set', node=1, location=None)),
        ('A.enter', a('update', node=1, last=[1, 1, 1])),
        ('A.leave', a('update', node=1, last=[2, 2, 2])),
        ('A.leave', a('clear', node=1)),
        ('C.enter', a('clear', node=1)),
    ]
    for event, action in reentrant:
        setup, world = standard()
        world['zones'].append(zone('F'))
        add(setup + steps(1, middle) + steps(1, [[3230, 3215, 0]], middle[-1]), nodes=[player(1, [3198, 3215, 0])],
            hooks=[dict(event=event, actions=[action])], **world)
    setup, world = standard()
    add(setup + [a('set', node=1, location=None), a('update', node=1, last=None), a('set', node=1, location=[3213, 3213, 0]),
                 a('update', node=1, last=None), a('set', node=1, location=None), a('update', node=1, last=[3213, 3213, 0]), a('clear', node=1)],
        nodes=[npc(1, None)], **world)

    # Music: region track, tile zones, the monitor's own list.
    def music(nodes, extra, **kw):
        base = [borders('m1', 3200, 3200, 3210, 3210), borders('m2', 3205, 3205, 3215, 3215, 1), a('newMusic', id='M1', music=11, borders='m1'),
                a('newMusic', id='M2', music=22, borders='m2')]
        cases.append(dict(group='music', actions=base + extra, nodes=nodes, **kw))

    for playing in [False, True]:
        for kind, artificial in [(player, False), (player, True), (npc, False)]:
            spec = kind(1, [3205, 3205, 0], playing=playing)
            if kind is player:
                spec['artificial'] = artificial
            music([spec], [a('update', node=1, last=None), a('checkMusic', node=1), a('setMusic', region=REGION, music=76), a('update', node=1, last=None),
                           a('setMusic', region=REGION, music=0), a('checkMusic', node=1), a('setMusic', region=REGION, music=-1),
                           a('musicZone', region=REGION, music='M2'), a('musicZone', region=REGION, music='M1'), a('checkMusic', node=1),
                           a('set', node=1, location=[3212, 3212, 3]), a('checkMusic', node=1), a('set', node=1, location=[3216, 3216, 0]),
                           a('checkMusic', node=1), a('musicAdd', node=1, music='M1'), a('musicAdd', node=1, music='M2'), a('musicAdd', node=1, music='M1'),
                           a('checkMusic', node=1), a('set', node=1, location=[3207, 3207, 2]), a('checkMusic', node=1), a('clear', node=1)])
    music([player(1, None), npc(2, None)], [a('checkMusic', node=1), a('checkMusic', node=2), a('musicAdd', node=1, music='M1'), a('checkMusic', node=1)])
    music([player(1, [3205, 3205, 0]), npc(2, [3205, 3205, 0])], [a('musicEnter', music='M1', node=1), a('musicEnter', music='M1', node=2)])
    for event, at in [('location', 1), ('location', 2), ('unlock', 1), ('musicPlaying', 1), ('playDefault', 1)]:
        music([player(1, [3205, 3205, 0])], [a('checkMusic', node=1), a('musicZone', region=REGION, music='M1'), a('checkMusic', node=1)],
              failures={event: at})
    for action in [a('set', node=1, location=None), a('set', node=1, location=[3300, 3300, 0]), a('musicAdd', node=1, music='M2')]:
        music([player(1, [3205, 3205, 0])], [a('musicAdd', node=1, music='M2'), a('musicZone', region=REGION, music='M1'), a('checkMusic', node=1)],
              hooks=[dict(event='location', at=1, actions=[action])])


def monitor_queries(cases):
    def add(actions, **kw):
        cases.append(dict(group='queries', actions=actions, **kw))

    inside = [a('set', node=1, location=[3213, 3213, 0]), a('update', node=1, last=None)]
    queries = [
        ('canLogout', a('canLogout', node=1), 'refuse'),
        ('death', a('death', node=1, killer=2), 'accept'),
        ('pvp', a('pvp', node=1, player=2, target=dict(node=2), style='MAGIC', message=True), 'accept'),
        ('interact', a('interact', node=1, target=dict(item=995), option='Take'), 'accept'),
        ('useWith', a('useWith', node=1, used=590, **{'with': dict(other=1)}), 'accept'),
        ('button', a('button', node=1), 'accept'),
        ('ignoreMulti', a('ignoreMulti', node=1, victim=2), 'accept'),
        ('teleport', a('teleport', node=1, type=0, target=dict(item=8007)), 'refuse'),
        ('startDeath', a('startDeath', node=1, entity=1, killer=2), 'refuse'),
        ('move', a('moveCheck', node=1, **{'from': [3213, 3213, 0], 'to': [3214, 3213, 0]}), 'refuse'),
        ('command', a('command', node=1, player=1, name='zone', args=['x']), 'accept'),
        ('request', a('canRequest', node=1, request='TRADE', target=2), 'refuse'),
        ('attack', a('continueAttack', node=1, target=dict(item=4151), style='MELEE', message=False), 'refuse'),
        ('leave', a('clear', node=1), 'refuse'),
    ]
    nodes = [player(1, [3198, 3215, 0]), player(2, [3213, 3214, 0])]
    for hook, action, sense in queries:
        setup, world = standard()
        add(setup + [action] + inside + [action, action], nodes=nodes, **world)
        for which in ['A', 'B', 'C']:
            setup, world = standard(answers={f'{which}.{hook}': [sense == 'accept']})
            add(setup + inside + [action, action], nodes=nodes, **world)
            setup, world = standard()
            add(setup + inside + [action, action], nodes=nodes, failures={f'{which}.{hook}': 1}, **world)
        # The callback removes its own zone from the list being walked.
        for which in ['A', 'B', 'C']:
            setup, world = standard()
            add(setup + inside + [action, action], nodes=nodes, hooks=[dict(event=f'{which}.{hook}', actions=[a('removeZone', node=1, zone=which)])],
                **world)
    # Player-only queries on an NPC: the cast fails only when a zone is present.
    for action in [a('canLogout', node=1), a('useWith', node=1, used=1, **{'with': None}), a('button', node=1), a('canRequest', node=1, request=None, target=None)]:
        setup, world = standard()
        add(setup + [action] + inside + [action], nodes=[npc(1, [3198, 3215, 0])], **world)

    # Null arguments pass through untouched.
    setup, world = standard()
    add(setup + inside + [a('death', node=1, killer=None), a('pvp', node=1, player=None, target=None, style=None, message=False),
                          a('interact', node=1, target=None, option=None), a('useWith', node=1, used=None, **{'with': None}),
                          a('ignoreMulti', node=1, victim=None), a('startDeath', node=1, entity=None, killer=None),
                          a('moveCheck', node=1, **{'from': None, 'to': None}), a('command', node=1, player=None, name=None, args=[]),
                          a('canRequest', node=1, request=None, target=None)], nodes=nodes, **world)

    # Zone type, restrictions, random events, names.
    plain = [borders('b', 3200, 3200, 3230, 3230)] + [a('register', zone=z, borders='b') for z in ['A', 'B', 'C']]
    checks = [a('type', node=1), a('canFire', node=1)] + [a('restricted', node=1, flag=f) for f in [1, 2, 4, 64, 128, 6, 0, -1]]
    checks += [a('inZone', node=1, name=name) for name in ['A', 'B', 'C', 'Aa', 'BB', 'nowhere', '']]
    variants = [
        [zone('A'), zone('B'), zone('C')],
        [zone('A', restrictions=['FOLLOWERS']), zone('B', restrictions=['TELEPORT', 'FIRES']), zone('C', name='Aa')],
        [zone('A', name='BB'), zone('B', 'plain', name='x'), zone('C', 'plain', restrictions=['OFF_MAP'])],
    ]
    for zones in variants:
        for sets in [[], [a('zoneSet', zone='B', type=3), a('zoneSet', zone='C', type=5)], [a('zoneSet', zone='A', type=-2)],
                     [a('zoneSet', zone='A', fire=True)], [a('zoneSet', zone='A', fire=True), a('zoneSet', zone='B', fire=True), a('zoneSet', zone='C', fire=True)]
                     if all(z['class'] == 'scripted' for z in zones) else [], [a('zoneSet', zone='A', name=None)]]:
            add(plain + checks + inside + sets + checks + [a('inZone', node=1, name=None)], nodes=nodes, zones=zones)
    setup, world = standard()
    add(setup + inside + [a('monitorAdd', node=1, zone='A', borders='bA'), a('removeZone', node=1, zone='A'), a('removeZone', node=1, zone='E'),
                          a('removeZone', node=1, zone='A'), a('removeZone', node=1, zone='A'), a('removeZone', node=1, zone='C')], nodes=nodes, **world)
    setup, world = standard()
    add(setup + inside + [borders('m', 1, 1, 2, 2), a('newMusic', id='M', music=3, borders='m'), a('musicAdd', node=1, music='M'), a('clear', node=1),
                          a('clear', node=1)], nodes=nodes, answers={'B.leave': [False, True]}, zones=world['zones'])


def teleports(cases):
    def add(actions, **kw):
        cases.append(dict(group='teleport', actions=actions, **kw))

    reg = [borders('b', 3200, 3200, 3230, 3230), a('register', zone='A', borders='b'), a('register', zone='B', borders='b'),
           a('set', node=1, location=[3213, 3213, 0]), a('update', node=1, last=None)]
    targets = [dict(item=1704), dict(item=1712), dict(item=2570), dict(item=11126), dict(item=10362), dict(item=11105), None, dict(node=2), dict(other=1)]
    for kind in [player, npc]:
        for timer in [False, True]:
            for locked in [False, True]:
                for restricted in [False, True]:
                    for level in [0, 1, 30, 31]:
                        if kind is npc and level != 0:
                            continue
                        zones = [zone('A', restrictions=['TELEPORT'] if restricted else []), zone('B')]
                        spec = kind(1, [3198, 3215, 0], teleTimer=timer, teleportLocked=locked)
                        if kind is player:
                            spec['skullLevel'] = level
                        actions = reg + [a('teleBlocked', node=1)]
                        actions += [a('teleport', node=1, type=t, target=target) for t in [-1, 0, 1, 2, 3] for target in (targets if t == 1 else targets[:2] + [None])]
                        add(actions, nodes=[spec, npc(2, [1, 1, 0])], zones=zones)
    for answers in [{'A.teleport': [False]}, {'B.teleport': [False]}, {'A.teleport': [True, False], 'B.teleport': [False, True]}]:
        add(reg + [a('teleport', node=1, type=t, target=dict(item=1704)) for t in [-1, 0, 1]],
            nodes=[player(1, [3198, 3215, 0]), npc(2, [1, 1, 0])], zones=[zone('A'), zone('B')], answers=answers)
    for event in ['timer', 'teleportLocked', 'skullLevel', 'playerMessage', 'A.teleport']:
        for at in [1, 2, 3]:
            add(reg + [a('teleport', node=1, type=1, target=dict(item=1704)), a('teleport', node=1, type=0, target=None), a('teleBlocked', node=1)],
                nodes=[player(1, [3198, 3215, 0], teleportLocked=True, skullLevel=20), npc(2, [1, 1, 0])], zones=[zone('A'), zone('B')],
                failures={event: at})


def combat(cases):
    def add(actions, **kw):
        cases.append(dict(group='combat', actions=actions, **kw))

    reg = [borders('b', 3200, 3200, 3230, 3230), a('register', zone='A', borders='b'), a('register', zone='B', borders='b'),
           a('set', node=1, location=[3213, 3213, 0]), a('update', node=1, last=None), a('set', node=2, location=[3214, 3213, 0]),
           a('update', node=2, last=None)]
    FAR = 9223372036854775807
    PAST = [None, -1, 0, 1, -9223372036854775808]

    def fight(first, second, extra=None, **kw):
        nodes = [first, second, player(3, [3215, 3213, 0]), npc(4, [3216, 3213, 0]), familiar(5, [3217, 3213, 0], owner=3),
                 familiar(6, [3218, 3213, 0], owner=1), familiar(7, [3219, 3213, 0], owner=None), player(8, [3220, 3213, 0], active=False)]
        actions = reg + (extra or [])
        for message in [False, True]:
            actions += [a('checkMulti', node=1, target=2, message=message), a('continueAttack', node=1, target=dict(node=2), style='MELEE', message=message)]
        add(actions, nodes=nodes, zones=[zone('A'), zone('B')], **kw)

    kinds = [player, npc]
    for first in kinds:
        for second in kinds:
            fight(first(1, [3198, 3215, 0]), second(2, [3199, 3215, 0]))
            fight(first(1, [3198, 3215, 0], multiZone=True), second(2, [3199, 3215, 0], multiZone=True))
            fight(first(1, [3198, 3215, 0], multiZone=True), second(2, [3199, 3215, 0]))
            fight(first(1, [3198, 3215, 0]), second(2, [3199, 3215, 0], multiZone=True))
            fight(first(1, [3198, 3215, 0], ignoreMulti=True), second(2, [3199, 3215, 0]))
            fight(first(1, [3198, 3215, 0]), second(2, [3199, 3215, 0]), answers={'B.ignoreMulti': [True]})
            fight(first(1, [3198, 3215, 0], continueAttack=False), second(2, [3199, 3215, 0]))
            fight(first(1, [3198, 3215, 0]), second(2, [3199, 3215, 0]), answers={'B.attack': [False]})
            for wild in [(True, True), (True, False), (False, True)]:
                fight(first(1, [3198, 3215, 0], wilderness=wild[0]), second(2, [3199, 3215, 0], wilderness=wild[1]))
            # Someone else is fighting the target.
            for attacker in [None, 1, 3, 4, 5, 6, 7, 8]:
                for time in [FAR] + PAST[:2]:
                    fight(first(1, [3198, 3215, 0], wilderness=True), second(2, [3199, 3215, 0], wilderness=True, attacker=attacker, combatTime=time))
            # The attacker is already under attack.
            for attacker in [None, 2, 3, 4, 5, 7, 8]:
                for time in [FAR] + PAST[2:]:
                    for npc_id in [0, 1613, 1614]:
                        if npc_id and (second is player or time != FAR):
                            continue
                        fight(first(1, [3198, 3215, 0], wilderness=True, attacker=attacker, combatTime=time),
                              second(2, [3199, 3215, 0], wilderness=True, npcId=npc_id))
    # Familiars on either side resolve to their owners.
    for first, second in [(familiar(1, [3198, 3215, 0], owner=3), npc(2, [3199, 3215, 0], attacker=3, combatTime=FAR)),
                          (familiar(1, [3198, 3215, 0], owner=3), npc(2, [3199, 3215, 0], attacker=5, combatTime=FAR)),
                          (familiar(1, [3198, 3215, 0], owner=3), npc(2, [3199, 3215, 0], attacker=6, combatTime=FAR)),
                          (familiar(1, [3198, 3215, 0], owner=None), npc(2, [3199, 3215, 0], attacker=7, combatTime=FAR)),
                          (player(1, [3198, 3215, 0], attacker=5, combatTime=FAR, wilderness=True), familiar(2, [3199, 3215, 0], owner=3)),
                          (player(1, [3198, 3215, 0], attacker=3, combatTime=FAR, wilderness=True), familiar(2, [3199, 3215, 0], owner=3)),
                          (player(1, [3198, 3215, 0], attacker=4, combatTime=FAR, wilderness=True), familiar(2, [3199, 3215, 0], owner=None)),
                          (npc(1, [3198, 3215, 0], attacker=7, combatTime=FAR), familiar(2, [3199, 3215, 0], owner=None))]:
        fight(first, second)
    for target in [None, dict(item=4151), dict(other=1), dict(node=4), dict(node=1)]:
        add(reg + [a('continueAttack', node=1, target=target, style=style, message=True) for style in ['MELEE', 'RANGE', 'MAGIC', None]],
            nodes=[player(1, [3198, 3215, 0]), player(2, [3199, 3215, 0]), player(3, [1, 1, 0]), npc(4, [1, 1, 0])], zones=[zone('A'), zone('B')])
    events = ['entityContinueAttack', 'A.attack', 'wilderness', 'packetMessage', 'multiZone', 'entityIgnoreMulti', 'A.ignoreMulti', 'owner',
              'attribute', 'active', 'id']
    for event in events:
        for at in [1, 2, 3, 4]:
            fight(player(1, [3198, 3215, 0], wilderness=True, attacker=5, combatTime=FAR), npc(2, [3199, 3215, 0], attacker=6, combatTime=at % 2 and FAR or 0),
                  failures={event: at})


def activity(cases):
    def add(actions, **kw):
        cases.append(dict(group='activity', actions=actions, **kw))

    info = a('regionInfo', region=REGION)
    add([info, a('viewed', region=REGION), a('view', region=REGION, name='ann', delta=1), a('view', region=REGION, name='ann', delta=1),
         a('view', region=REGION, name='bob', delta=1), a('viewed', region=REGION), info, a('view', region=REGION, name='zed', delta=-1),
         a('view', region=REGION, name='ann', delta=-1), a('view', region=REGION, name='ann', delta=-1), info, a('view', region=REGION, name='bob', delta=-1),
         a('viewed', region=REGION), a('view', region=REGION, name='', delta=1), a('view', region=NORTH, name='ann', delta=1), info,
         a('regionInfo', region=NORTH)])

    def ticks(count, every=1):
        out = []
        for _ in range(count // every):
            out += [a('tick', count=every), info]
        return out

    # The plain life cycle: active, empty, fifty runner updates, unloaded, active again.
    add([info, a('flagActive', region=REGION), info, a('flagActive', region=REGION), a('checkInactive', region=REGION), info, a('checkInactive', region=REGION),
         a('tick', count=48), info, a('tick'), info, a('tick'), info, a('tick'), info, a('checkInactive', region=REGION), a('flagActive', region=REGION), info,
         a('tick'), info])
    add([a('isInactive', region=REGION, run=False), info, a('tick', count=60), info, a('isInactive', region=REGION, run=True), info, a('tick', count=50), info,
         a('flagInactive', region=REGION, force=None), info, a('flagInactive', region=REGION, force=False), a('flagInactive', region=REGION, force=True),
         a('unload', region=REGION, force=False), a('unload', region=REGION, force=True), info])
    for timeout in [3, 1, 0, -4, 51]:
        add([a('flagActive', region=REGION), a('timeout', region=REGION, ticks=timeout), a('checkInactive', region=REGION)] + ticks(4)
            + [a('tick', count=60), info])
    # A stopped pulse that is still queued is submitted a second time: both copies count.
    add([a('flagActive', region=REGION), a('checkInactive', region=REGION), a('tick', count=10), info, a('flagActive', region=REGION), info,
         a('checkInactive', region=REGION), info] + ticks(30, 5) + [a('checkInactive', region=REGION), info] + ticks(60, 10))
    add([a('flagActive', region=REGION), a('checkInactive', region=REGION), a('flagActive', region=REGION), a('checkInactive', region=REGION),
         a('flagActive', region=REGION), a('checkInactive', region=REGION), info] + ticks(20, 4))
    add([a('flagActive', region=REGION), a('checkInactive', region=REGION), a('tick', count=10), a('flagActive', region=REGION), a('tick'), info,
         a('checkInactive', region=REGION), info] + ticks(52, 13))

    # Viewers and players refuse the unload and flag the region active again.
    crowd = [player(1, [3205, 3205, 0]), player(2, [3206, 3205, 0]), npc(3, [3205, 3205, 0]), npc(4, [3260, 3260, 3], active=False), npc(5, [3201, 3260, 1]),
             player(6, [3262, 3262, 2])]
    place = [a('chunkAdd', chunk=n['location'], list='players' if n['kind'] == 'player' else 'npcs', node=n['id']) for n in crowd]
    npcs_only = [p for p, n in zip(place, crowd) if n['kind'] == 'npc']
    stuff = [a('chunkStuff', chunk=[3205, 3205, 0]), a('chunkStuff', chunk=[3260, 3260, 3]), a('chunkStuff', chunk=[3230, 3230, 1])]
    add(place + stuff + [a('flagActive', region=REGION), info, a('checkInactive', region=REGION), a('isInactive', region=REGION, run=True),
                         a('unload', region=REGION, force=False), info, a('flagInactive', region=REGION, force=None), info,
                         a('chunkRemove', chunk=[3205, 3205, 0], list='players', node=1), a('unload', region=REGION, force=False),
                         a('chunkRemove', chunk=[3205, 3205, 0], list='players', node=2), a('unload', region=REGION, force=False),
                         a('chunkRemove', chunk=[3262, 3262, 2], list='players', node=6), a('checkInactive', region=REGION), info, a('tick', count=50), info,
                         a('flagActive', region=REGION), info], nodes=crowd)
    add(place + stuff + [a('flagActive', region=REGION), a('unload', region=REGION, force=True), info, a('flagInactive', region=REGION, force=True), info],
        nodes=crowd)
    add(npcs_only + stuff + [a('flagActive', region=REGION), a('view', region=REGION, name='ann', delta=1), a('checkInactive', region=REGION),
                             a('unload', region=REGION, force=False), a('flagInactive', region=REGION, force=None), info,
                             a('flagInactive', region=REGION, force=True), info, a('flagActive', region=REGION), info,
                             a('view', region=REGION, name='ann', delta=-1), a('checkInactive', region=REGION), a('tick', count=25), info,
                             a('view', region=REGION, name='bob', delta=1), a('tick', count=25), info, a('tick', count=3), info,
                             a('view', region=REGION, name='bob', delta=-1), a('checkInactive', region=REGION), a('tick', count=50), info], nodes=crowd)
    # A player arrives while the pulse runs: its body is refused, the pulse still ends.
    add(npcs_only + [a('flagActive', region=REGION), a('checkInactive', region=REGION), a('tick', count=49), a('chunkAdd', chunk=[3205, 3205, 0], list='players', node=1),
                     info, a('tick'), info, a('tick'), info], nodes=crowd)
    # The region load that activation runs: its lookup, its failure, and no second load while active.
    add([a('flagActive', region=REGION), info, a('flagActive', region=REGION), info, a('flagInactive', region=REGION, force=True), a('flagActive', region=REGION), info],
        failures=dict(archiveId=1))
    add([a('flagActive', region=REGION), a('flagActive', region=NORTH), a('flagActive', region=65536 + REGION), a('flagActive', region=7), info,
         a('regionInfo', region=NORTH), a('regionInfo', region=7)])
    # Failures midway keep what was already done.
    for event, at in [('active', 1), ('active', 2), ('renderable', 1), ('renderable', 2), ('regionInactivity', 1), ('regionInactivity', 2),
                      ('regionInactivity', 3), ('log', 1), ('archiveId', 2)]:
        add(npcs_only + stuff + [a('flagActive', region=REGION), info, a('checkInactive', region=REGION), a('tick', count=50), info,
                                 a('view', region=REGION, name='ann', delta=1), a('unload', region=REGION, force=False), info,
                                 a('flagInactive', region=REGION, force=True), info, a('flagActive', region=REGION), info],
            nodes=crowd, failures={event: at})
    for kind in ['npcs', 'players']:
        add([a('chunkAdd', chunk=[3205, 3205, 0], list='npcs', node=3), a('chunkAdd', chunk=[3210, 3205, 0], list=kind, node=None),
             a('chunkAdd', chunk=[3220, 3205, 0], list='npcs', node=5), a('flagActive', region=REGION), info, a('checkInactive', region=REGION),
             a('unload', region=REGION, force=False), a('unload', region=REGION, force=True), info], nodes=crowd)
    # Callbacks that change the lists being walked.
    for event, action in [('renderable', a('chunkAdd', chunk=[3205, 3205, 0], list='npcs', node=5)),
                          ('active', a('chunkClearList', chunk=[3205, 3205, 0], list='npcs')),
                          ('active', a('chunkRemove', chunk=[3205, 3205, 0], list='npcs', node=3)),
                          ('regionInactivity', a('chunkRemove', chunk=[3205, 3205, 0], list='npcs', node=3)),
                          ('regionInactivity', a('chunkAdd', chunk=[3262, 3262, 3], list='players', node=1)),
                          ('regionInactivity', a('flagActive', region=REGION)),
                          ('regionInactivity', a('view', region=REGION, name='late', delta=1)),
                          ('archiveId', a('flagActive', region=REGION)),
                          ('archiveId', a('checkInactive', region=REGION)),
                          ('log', a('view', region=REGION, name='ann', delta=-1))]:
        add(npcs_only + [a('chunkAdd', chunk=[3205, 3205, 0], list='npcs', node=4), a('flagActive', region=REGION), info, a('view', region=REGION, name='ann', delta=1),
                         a('unload', region=REGION, force=False), a('view', region=REGION, name='ann', delta=-1), a('unload', region=REGION, force=False), info,
                         a('tick', count=50), info], nodes=crowd, hooks=[dict(event=event, actions=[action])])
    # Zones stay on the region through unload and reactivation; monitors are not told.
    setup, world = standard()
    add(setup + [a('chunkAdd', chunk=[3213, 3213, 0], list='npcs', node=1), a('set', node=1, location=[3213, 3213, 0]), a('update', node=1, last=None),
                 a('flagActive', region=REGION), a('checkInactive', region=REGION), a('tick', count=50), info, a('flagActive', region=REGION), info,
                 a('update', node=1, last=None)], nodes=[npc(1, [3198, 3215, 0])], **world)


def concrete(cases):
    def add(actions, **kw):
        cases.append(dict(group='concrete', actions=actions, **kw))

    # BankZone.
    walk = steps(1, [[3178, 3432, 0], [3179, 3432, 0], [3194, 3446, 0], [3195, 3446, 0], [3250, 3416, 0], [3257, 3423, 1], [3258, 3423, 0]])
    add(info('Z') + [a('configure', zone='Z')] + info('Z') + [a('regionInfo', region=12597), a('regionInfo', region=12853)] + walk
        + [a('canFire', node=1), a('type', node=1), a('inZone', node=1, name='bank'), a('clear', node=1)],
        nodes=[player(1, [3178, 3432, 0])], zones=[zone('Z', 'bank')])
    # KaramjaZone.
    for rum in [0, 1, 7]:
        add(info('Z') + [a('configure', zone='Z'), a('regionInfo', region=11057), a('regionInfo', region=11309)]
            + steps(1, [[2816, 3200, 0]]) + steps(2, [[2817, 3200, 0]])
            + [a('teleport', node=n, type=t, target=target) for n in [1, 2] for t in [-1, 0, 1] for target in [None, dict(item=431)]]
            + [a('hook', zone='Z', node=1, hook='teleport', type=2, target=None), a('inZone', node=1, name='karamja')],
            nodes=[player(1, [2816, 3200, 0], rum=rum), npc(2, [2817, 3200, 0], rum=rum)], zones=[zone('Z', 'karamja')])
    for event, at in [('inventoryAmount', 1), ('inventoryRemove', 1), ('packetMessage', 1)]:
        add([a('configure', zone='Z')] + steps(1, [[2816, 3200, 0]]) + [a('teleport', node=1, type=0, target=None), a('teleport', node=1, type=0, target=None)],
            nodes=[player(1, [2816, 3200, 0], rum=2)], zones=[zone('Z', 'karamja')], failures={event: at})
    # ModeratorZone.
    centre = [2846, 5213, 0]
    for rights in ['REGULAR_PLAYER', 'PLAYER_MODERATOR', 'ADMINISTRATOR']:
        for home in [None, [3222, 3218, 0]]:
            nodes = [player(1, [2839, 5204, 0], rights=rights), player(2, centre, rights='ADMINISTRATOR'), npc(3, centre),
                     player(4, centre, rights='REGULAR_PLAYER'), player(5, centre, rights='PLAYER_MODERATOR')]
            add(info('Z') + [a('configure', zone='Z'), a('regionInfo', region=11345), a('setHome', location=home)]
                + steps(1, [[2840, 5204, 0], [2839, 5204, 0], [2853, 5224, 0], [2854, 5224, 0]], [2839, 5204, 0]) + steps(3, [centre, [2839, 5204, 0]])
                + [a('toggle', node=2, on=False)] + steps(1, [[2840, 5204, 0], [2846, 5213, 0], [2839, 5204, 0]], [2854, 5224, 0])
                + [a('toggle', node=2, on=True)] + steps(1, [[2840, 5204, 0], [2839, 5204, 0]], [2839, 5204, 0])
                + [a('home', node=1), a('modTeleport', node=1), a('clear', node=1)],
                nodes=nodes, zones=[zone('Z', 'moderator')], localPlayers=[2, 4, 5, 1])
    for event, at in [('rights', 1), ('rights', 2), ('packetMessage', 1), ('teleporterSend', 1), ('restoreTabs', 1), ('localPlayers', 1)]:
        add([a('configure', zone='Z')] + steps(1, [[2840, 5204, 0], [2839, 5204, 0]]) + [a('toggle', node=2, on=False)]
            + steps(1, [[2840, 5204, 0], [2839, 5204, 0]]),
            nodes=[player(1, [2839, 5204, 0]), player(2, centre, rights='ADMINISTRATOR'), player(4, centre)], zones=[zone('Z', 'moderator')],
            localPlayers=[4, 2], failures={event: at})
    # MultiwayCombatZone.
    add(info('Z') + [a('configure', zone='Z')] + info('Z') + [a('regionInfo', region=r) for r in [13105, 12341, 12598, 11318, 10140]],
        zones=[zone('Z', 'multiway')])
    spot = [[3115, 3240, 0], [3107, 3234, 0], [3106, 3234, 0], [3134, 3259, 3], [3135, 3259, 0]]
    for resizable in [False, True]:
        nodes = [player(1, [3106, 3234, 0], resizable=resizable, windowPane=746 if resizable else 548), npc(2, [3106, 3234, 0]),
                 familiar(3, [3106, 3234, 0], owner=1)]
        add([a('configure', zone='Z')] + steps(1, spot) + steps(2, spot) + steps(3, spot) + steps(1, spot[:1], spot[-1]) + [a('clear', node=1)]
            + steps(2, spot[:1], spot[-1]) + [a('clear', node=2)], nodes=nodes, zones=[zone('Z', 'multiway')])
    add([a('configure', zone='Z'), a('set', node=1, location=[3160, 3490, 2]), a('update', node=1, last=None), a('set', node=1, location=[3160, 3490, 0]),
         a('update', node=1, last=None), a('set', node=1, location=[3080, 5530, 1]), a('update', node=1, last=None)],
        nodes=[player(1, [3106, 3234, 0])], zones=[zone('Z', 'multiway')])
    for event, at in [('resizable', 1), ('windowPane', 1), ('packet', 1), ('interfaceConfig', 1), ('setMultiZone', 1), ('setMultiZone', 2), ('packet', 2)]:
        add([a('configure', zone='Z')] + steps(1, spot[:3]) + [a('clear', node=1)], nodes=[player(1, [3106, 3234, 0])], zones=[zone('Z', 'multiway')],
            failures={event: at})
    # The NPC stacking rule of MultiwayCombatZone.move.
    enter = [a('configure', zone='Z')] + steps(1, [[3115, 3240, 0]])
    moves = [a('moveCheck', node=1, **{'from': [3115, 3240, 0], 'to': to}) for to in [[3116, 3240, 0], [3117, 3240, 0], [3118, 3241, 0], [3120, 3240, 0],
                                                                                    [3114, 3239, 0], None]]
    others = [
        [npc(2, [3117, 3240, 0])],
        [npc(2, [3117, 3240, 0], preventStacking=False)],
        [npc(2, [3117, 3240, 0], invisible=True)],
        [npc(2, [3117, 3240, 0], attackOption=False)],
        [npc(2, [3117, 3240, 0], size=2)],
        [npc(2, [3117, 3240, 0], size=3), npc(3, [3113, 3238, 0], size=2)],
        [npc(2, None)],
        [npc(2, [3200, 3200, 0], preventStacking=False), npc(3, [3118, 3241, 1])],
        [],
    ]
    for mover in [player(1, [3115, 3240, 0]), player(1, [3115, 3240, 0], size=2), npc(1, [3115, 3240, 0], size=3), player(1, [3115, 3240, 0], npcWalkable=True),
                  player(1, None)]:
        for group in others:
            ids = [n['id'] for n in group]
            start = enter if mover['location'] else [a('configure', zone='Z'), a('monitorAdd', node=1, zone='Z', borders='b')]
            add([borders('b', 3107, 3234, 3134, 3259)] + start + moves, nodes=[mover] + group, zones=[zone('Z', 'multiway')], localNpcs=ids + ([1] if mover['kind'] == 'npc' else []))
    for event, at in [('npcWalkable', 1), ('localNpcs', 1), ('invisible', 2), ('attackOption', 1), ('preventStacking', 2), ('size', 1), ('size', 2), ('location', 6)]:
        add(enter + moves[:3], nodes=[player(1, [3115, 3240, 0]), npc(2, [3117, 3240, 0]), npc(3, [3116, 3240, 0])], zones=[zone('Z', 'multiway')],
            localNpcs=[2, 3], failures={event: at})
    grid = [[x, y, sx, sy, 10, 10, w, h] for x in [8, 9, 10, 11, 12] for y in [8, 10, 12] for sx, sy in [(1, 1), (2, 3)] for w, h in [(1, 1), (2, 2), (3, 1)]]
    grid += [[MAX, 0, 1, 1, 0, 0, 1, 1], [0, MAX, 1, 2, 0, MAX, 1, 1], [MIN, MIN, MAX, MAX, MAX, MAX, MIN, MIN], [0, 0, 0, 0, 0, 0, 0, 0], [5, 5, -1, -1, 5, 5, 2, 2]]
    add([a('standingIn', args=args) for args in grid])
    # ZoneBuilder.configure on each ported zone object, twice.
    for cls in ['bank', 'karamja', 'moderator']:
        add([a('configure', zone='Z'), a('configure', zone='Z')] + info('Z') + [a('zoneEquals', a='Z', b='Y'), a('zoneEquals', a='Z', b='X')],
            zones=[zone('Z', cls), zone('Y', cls), zone('X', 'plain', name='bank')])


def requests():
    cases = []
    geometry(cases)
    randoms(cases)
    zone_base(cases)
    registration(cases)
    monitor_moves(cases)
    monitor_queries(cases)
    teleports(cases)
    combat(cases)
    activity(cases)
    concrete(cases)
    return cases
