"""The force movement corpus: every constructor and static run form, the instance runs, start, pulse,
update until the pulse stops, stop, the direction helper and the setters, over players and NPCs, starts on
and off the entity's tile, speeds of zero, negative and large, packed (15-bit) coordinates, shared and
null animations, inactive nodes and the shared walk animation.

`cases()` yields (case, sweep): when sweep is an operation index, generate.py adds one case per recorded
call of that operation in which that call fails.
"""
import random

NEEDED_EVENTS = ['location', 'animate', 'disabled_ticks', 'register_mask', 'update_region', 'teleport_location', 'lock', 'unlock',
                 'submit', 'active', 'exception']

PLAYER = {'kind': 'player', 'location': [3200, 3200, 0]}
NPC = {'kind': 'npc', 'location': [3100, 3300, 1]}
ANIMS = [[839, 'LOW'], [1115, 'MID'], [756, 'HIGH'], [2588, 'VERY_HIGH']]


def case(group, ops, entities=None, anims=None):
    return dict(group=group, entities=entities or [dict(PLAYER), dict(NPC)], anims=ANIMS if anims is None else anims, ops=ops)


def updates(n=14):
    return {'op': 'updates', 'fm': 0, 'n': n}


STARTS = [
    ([3200, 3200, 0], [3205, 3200, 0]),
    ([3200, 3200, 0], [3200, 3195, 0]),
    ([3198, 3200, 0], [3206, 3200, 0]),
    ([3200, 3203, 0], [3200, 3210, 0]),
    ([3200, 3200, 0], [3203, 3204, 0]),
    ([3200, 3200, 0], [3196, 3197, 0]),
    ([3200, 3200, 0], [3200, 3200, 0]),
    ([3190, 3190, 0], [3200, 3200, 0]),
    ([3200, 3200, 2], [3201, 3200, 2]),
    ([3200, 3200, 0], [20000, 3200, 0]),
    ([3200, 3200, 0], [-3200, 3200, 5]),
    ([16383, 0, 0], [-16384, 0, -1]),
]


def static_runs():
    forms = ['r2', 'r3', 'r4', 'r5', 'r5a', 'r6', 'r7', 'r8', 'r9', 'rSpeeds']
    k = 0
    for form in forms:
        for s, (start, dest) in enumerate(STARTS):
            for e in (0, 1):
                if (k + s + e) % 2 and form not in ('r9', 'r8'):
                    k += 1
                    continue
                op = {'op': 'run', 'form': form, 'e': e, 'start': start, 'dest': dest, 'startAnim': 'a%d' % (k % 4), 'anim': 'a%d' % ((k + 1) % 4),
                      'direction': ['NORTH', 'SOUTH_EAST', 'WEST', 'EAST'][k % 4], 'speed': [10, 20, 5, 1][k % 4],
                      'commence': [10, 0, 20, 3][(k // 2) % 4], 'path': [10, 20, 1, 0][(k // 3) % 4], 'unlock': k % 3 != 0}
                ops = [op, updates()]
                yield case('static-run', ops), (0 if s in (0, 2) and e == 0 else (1 if s == 2 and e == 1 and form in ('r9', 'r2') else None))
                k += 1


def constructors():
    forms = ['full', 'speed', 'speeds', 'animation', 'detached']
    k = 0
    for form in forms:
        for s, (start, dest) in enumerate(STARTS[:8]):
            for follow in ('start', 'runWith', 'runEntity', 'runSelf'):
                base = {'op': 'construct', 'form': form, 'e': k % 2, 'start': start, 'dest': dest, 'startAnim': 'walk' if k % 3 else 'a2',
                        'anim': ['a0', 'walk', None, 'a3'][k % 4], 'direction': ['NORTH_WEST', None, 'SOUTH', 'EAST'][k % 4] if form == 'full' else None,
                        'commence': [10, 0, 7, 25][k % 4], 'path': [10, 15, 0, 3][(k // 2) % 4], 'speed': [10, 0, 4, 30][k % 4], 'unlock': k % 2 == 0}
                if follow == 'start':
                    second = {'op': 'start', 'fm': 0}
                elif follow == 'runWith':
                    second = {'op': 'runWith', 'fm': 0, 'e': (k + 1) % 2, 'speed': [3, 0, 12, -2][k % 4]}
                elif follow == 'runEntity':
                    second = {'op': 'runEntity', 'fm': 0, 'e': k % 2}
                else:
                    second = {'op': 'runSelf', 'fm': 0}
                ops = [base, second, updates(), {'op': 'stop', 'fm': 0}]
                sweep = 1 if (s in (0, 7) and follow in ('start', 'runWith')) else None
                yield case('constructors', ops), sweep
                k += 1


def lifecycles():
    """Pulse lifecycles: inactive nodes, end animations, pulse and stop directly, the setters and getters."""
    run = {'op': 'run', 'form': 'r9', 'e': 0, 'start': [3198, 3200, 0], 'dest': [3204, 3200, 0], 'startAnim': 'a0', 'anim': 'a1',
           'direction': 'EAST', 'commence': 10, 'path': 10, 'unlock': True}
    yield case('lifecycle', [run, {'op': 'setEndAnimation', 'fm': 0, 'value': 'a2'}, updates()]), None
    yield case('lifecycle', [run, {'op': 'set', 'e': 0, 'fields': {'active': False}}, {'op': 'update', 'fm': 0}]), 2
    yield case('lifecycle', [run, {'op': 'update', 'fm': 0}, {'op': 'set', 'e': 0, 'fields': {'active': False}}, updates(4)]), None
    yield case('lifecycle', [run, {'op': 'pulse', 'fm': 0}, {'op': 'pulse', 'fm': 0}, {'op': 'stop', 'fm': 0}, {'op': 'stop', 'fm': 0}]), 1
    yield case('lifecycle', [run, {'op': 'setEndAnimation', 'fm': 0, 'value': 'walk'}, {'op': 'stop', 'fm': 0}]), 2
    yield case('lifecycle', [dict(run, unlock=False), updates()]), None
    yield case('lifecycle', [dict(run, e=1), updates()]), None
    for field, value in [('setStart', [3000, 3000, 0]), ('setDestination', [3001, 3009, 3]), ('setDirection', 'SOUTH_WEST'), ('setDirection', None),
                         ('setCommenceSpeed', 0), ('setPathSpeed', -5), ('setEntity', 1), ('setEntity', None), ('setStartAnim', 'a3'),
                         ('setAnimation', None), ('setEndAnimation', 'a1'), ('setStart', None), ('setDestination', None)]:
        ops = [{'op': 'construct', 'form': 'full', 'e': 0, 'start': [3200, 3200, 0], 'dest': [3203, 3200, 0], 'startAnim': 'a0', 'anim': 'a1',
                'direction': 'EAST', 'commence': 10, 'path': 10, 'unlock': True},
               {'op': field, 'fm': 0, 'value': value}]
        ops += [{'op': 'get', 'fm': 0, 'field': f} for f in ['start', 'destination', 'direction', 'commenceSpeed', 'pathSpeed', 'entity',
                                                             'startAnim', 'animation', 'endAnimation']]
        ops += [{'op': 'start', 'fm': 0}, updates(), {'op': 'get', 'fm': 0, 'field': 'commenceSpeed'}, {'op': 'get', 'fm': 0, 'field': 'pathSpeed'}]
        yield case('setters', ops), None
    # The same animation object as start and path animation, and the walk animation shared by two movements.
    yield case('shared', [dict(run, startAnim='a0', anim='a0'), updates()]), None
    yield case('shared', [{'op': 'construct', 'form': 'animation', 'e': 0, 'start': [3200, 3200, 0], 'dest': [3202, 3200, 0], 'anim': 'a1'},
                          {'op': 'run', 'form': 'r2', 'e': 1, 'dest': [3105, 3300, 1]},
                          {'op': 'start', 'fm': 0}, {'op': 'updates', 'fm': 1, 'n': 10}]), None
    # A missing start or destination: the entity's location is read before the start is used.
    for field in ('setStart', 'setDestination'):
        for follow in ({'op': 'start', 'fm': 0}, {'op': 'runWith', 'fm': 0, 'e': 1, 'speed': 4}, {'op': 'runSelf', 'fm': 0}):
            ops = [{'op': 'construct', 'form': 'full', 'e': 0, 'start': [3200, 3200, 0], 'dest': [3203, 3200, 0], 'startAnim': 'a0', 'anim': 'a1',
                    'direction': 'EAST', 'commence': 10, 'path': 10, 'unlock': True}, {'op': field, 'fm': 0, 'value': None}, follow]
            yield case('nulls', ops), None
    # A location the entity does not have (null) and an NPC start check.
    yield case('nulls', [run], entities=[{'kind': 'player'}, dict(NPC)]), None
    yield case('nulls', [dict(run, form='r2', e=None)]), None
    yield case('nulls', [{'op': 'construct', 'form': 'detached', 'start': [3200, 3200, 0], 'dest': [3202, 3200, 0], 'anim': 'a1'},
                         {'op': 'start', 'fm': 0}, {'op': 'runSelf', 'fm': 0}, {'op': 'runEntity', 'fm': 0, 'e': 0}, updates(6)]), None
    yield case('nulls', [{'op': 'construct', 'form': 'detached', 'start': [3200, 3200, 0], 'dest': [3202, 3200, 0], 'anim': 'a1'},
                         {'op': 'setEntity', 'fm': 0, 'value': 1}, {'op': 'start', 'fm': 0}, updates(6)]), None


def speeds():
    """Speed arithmetic: zero (infinite and NaN ceilings), negative, fractional and large speeds over many distances."""
    for commence in (0, 1, 3, 10, 20, -10, 1000000, 2147483647):
        for path in (0, 1, 7, 10, -3, 2147483647):
            for offset in (0, 2, 9):
                ops = [{'op': 'run', 'form': 'r9', 'e': 0, 'start': [3200 - offset, 3200, 0], 'dest': [3200 + 6, 3200 + offset, 0],
                        'startAnim': 'a0', 'anim': 'a1', 'direction': 'EAST', 'commence': commence, 'path': path, 'unlock': True}, updates(4)]
                yield case('speeds', ops), None
    for speed in (0, 1, 5, -1, 2147483647):
        for commence, path in ((10, 10), (0, 10), (10, 0), (0, 0), (3, 7)):
            ops = [{'op': 'construct', 'form': 'full', 'e': 0, 'start': [3196, 3200, 0], 'dest': [3205, 3203, 0], 'startAnim': 'a0', 'anim': 'a1',
                    'direction': 'NORTH', 'commence': commence, 'path': path, 'unlock': False},
                   {'op': 'runWith', 'fm': 0, 'e': 0, 'speed': speed}, updates(4)]
            yield case('instance-speeds', ops), None


def directions():
    points = [-20000, -16384, -5, -1, 0, 1, 4, 16383, 16384, 30000]
    ops = []
    for dx in points:
        for dy in points:
            ops.append({'op': 'direction', 'start': [0, 0, 0], 'dest': [dx, dy, 0]})
    yield case('directions', ops), None
    r = random.Random(5)
    ops = [{'op': 'direction', 'start': [r.randint(-300, 4000), r.randint(-300, 4000), r.randint(-2, 6)],
            'dest': [r.randint(-300, 4000), r.randint(-300, 4000), r.randint(-2, 6)]} for _ in range(300)]
    yield case('directions', ops), None
    # In-range endpoints whose difference leaves the fifteen-bit range: getDelta's packing wraps it.
    far = [-16384, -16000, -9000, -1, 0, 9000, 16000, 16383]
    ops = [{'op': 'direction', 'start': [sx, sy, 0], 'dest': [dx, dy, 3]} for sx in far for dx in far for sy, dy in ((0, 0), (-16000, 16000), (5, -3), (16383, -16384))]
    yield case('directions', ops), None


def sequences():
    r = random.Random(11)
    for seed in range(40):
        ops = []
        count = 0
        for _ in range(r.randint(3, 9)):
            roll = r.random()
            if roll < 0.35 or count == 0:
                ops.append({'op': 'run', 'form': r.choice(['r2', 'r3', 'r4', 'r9', 'rSpeeds']), 'e': r.choice([0, 1]),
                            'start': [3200 + r.randint(-4, 4), 3200 + r.randint(-4, 4), 0], 'dest': [3200 + r.randint(-9, 9), 3200 + r.randint(-9, 9), 0],
                            'startAnim': r.choice(['a0', 'a1', 'walk', None]), 'anim': r.choice(['a2', 'a3', 'walk', None]),
                            'direction': r.choice(['NORTH', 'EAST', 'SOUTH_WEST']), 'commence': r.randint(0, 20), 'path': r.randint(0, 20),
                            'unlock': r.random() < 0.8})
                count += 1
            elif roll < 0.8:
                ops.append({'op': 'updates', 'fm': r.randrange(count), 'n': r.randint(1, 6)})
            elif roll < 0.9:
                ops.append({'op': 'set', 'e': r.choice([0, 1]), 'fields': {'location': [3200 + r.randint(-5, 5), 3200 + r.randint(-5, 5), 0]}})
            else:
                ops.append({'op': 'stop', 'fm': r.randrange(count)})
        yield case('sequences', ops), None


def cases():
    for generator in (static_runs, constructors, lifecycles, speeds, directions, sequences):
        yield from generator()


def groups(all_cases):
    out = {}
    for c in all_cases:
        out[c['group']] = out.get(c['group'], 0) + 1
    return dict(sorted(out.items()))
