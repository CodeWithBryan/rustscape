"""Designed cases for the bots oracle (see generate.py).

A case gives the name files AIPlayer reads, the world's nodes, item definitions, the content scripts, the answers of
the recording stand-ins and the operations to run against the unchanged bot engine. `sweep` marks the operation whose
recorded calls fail one at a time in the second pass (`(index, stride)` samples a long one). An operation that is
expected to throw names the failure class in `expect`.

Answers are keyed `Class.method`, `Class.method@firstArgument` or `Class.method@firstArgument#secondArgument`; a list
answers one element per call and repeats its last. Draws that are not answered are 0: the first name of the list, a
male bot facing north-west.
"""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
BOTS = ROOT / 'reference/server/Server/src/main/core/game/bots'

P = [3200, 3200, 0]
NAMES = 'alpha\nbeta\ngamma\ndelta\nepsilon\n'
REGION = 12850  # the region of P

CASES = []


def add(name, group, ops, sweep=None, **kw):
    case = dict(name=name, group=group, ops=ops)
    case['files'] = {'botnames.txt': NAMES}
    for k, v in kw.items():
        case[k] = v
    if case['files'] is None:
        del case['files']
    CASES.append((case, sweep))


def new(loc=P, **kw):
    return dict(op='newAIPlayer', loc=loc, **kw)


def o(operation, /, **kw):
    return dict(op=operation, **kw)


def player(name='alice', loc=P):
    return dict(kind='player', name=name, loc=loc)


def npc(id, name, loc, **kw):
    return dict(kind='npc', id=id, name=name, loc=loc, **kw)


def scenery(id, name, loc, **kw):
    return dict(kind='scenery', id=id, name=name, loc=loc, **kw)


def ground(id, loc, dropper='p', amount=1):
    return dict(kind='ground', id=id, loc=loc, dropper=dropper, amount=amount)


# ================================================================ AIPlayer: construction and placement
add('construct/basic', 'lifecycle', [new()], sweep=0)
add('construct/draws', 'lifecycle', [new([3222, 3218, 1])],
    answers={'RandomFunction.random@5': 3, 'RandomFunction.random@3': 1, 'Random.nextInt': 6, 'RandomFunction.random2': 7})
add('construct/sequence', 'lifecycle', [new(), new([3201, 3200, 0]), new([3202, 3200, 0]), o('deregister', uid=2), new([3203, 3200, 0]),
                                        o('get', uid=1), o('get', uid=2), o('get', uid=4), o('get', uid=9)],
    answers={'RandomFunction.random@5': 4, 'RandomFunction.random@4': 0, 'RandomFunction.random@3': [2, 0], 'RandomFunction.random@2': 1})
add('construct/names-run-out', 'lifecycle', [new(), new(), new(expect='Index'), new(expect='Index')], files={'botnames.txt': 'one\ntwo'})
add('construct/index-too-high', 'lifecycle', [new(expect='Index'), new()], answers={'RandomFunction.random@2': [2, 1]}, files={'botnames.txt': 'one\ntwo\n'})
add('construct/index-negative', 'lifecycle', [new(expect='Index'), new()], answers={'RandomFunction.random@2': [-1, 0]}, files={'botnames.txt': 'one\ntwo\n'})
add('construct/no-names-file', 'lifecycle', [new(expect='Index'), o('get', uid=1)], files=None)
add('construct/no-data-path', 'lifecycle', [o('get', uid=1), new(expect='Index')], config=dict(dataPath=False))
add('construct/display-names', 'lifecycle', [new(), new(), new(), new(), new()],
    files={'botnames.txt': 'the_OLD_nite\nx\nMr  Two Spaces\n_lead\n9lives\n'})
add('construct/direction-too-high', 'lifecycle', [new(expect='Index'), o('get', uid=1), new()], answers={'Random.nextInt': [8, 7]})
add('construct/direction-negative', 'lifecycle', [new(expect='Index'), new()], answers={'Random.nextInt': [-1, 1]})
add('construct/null-location', 'lifecycle', [new(None), o('randomWalk', bot='bot#1', x=2, y=2, expect='Null'), o('reset', bot='bot#1'),
                                             o('getClosesCreature', bot='bot#1', radius=3),
                                             o('getClosesCreature', bot='bot#1', radius=3, expect='Null')],
    nodes={'n1': npc(1, 'Cow', P)}, answers={'PlayerFlags.isUpdateSceneGraph': True, 'RegionManager.getLocalNPCs': [[], ['n1']]})
add('construct/list-full', 'lifecycle', [new(), new(), new(), o('deregister', uid=1), new(), new()], config=dict(capacity=2))
add('construct/list-without-slots', 'lifecycle', [new(), o('clear', bot='bot#1'), o('deregister', uid=1)], config=dict(capacity=0))
for which, text in [
    ('crlf', 'one\r\ntwo\r\nthree\r\n'),
    ('lone-cr', 'lost\rkept\nlast\r'),
    ('double-cr', 'lost\r\rkept'),
    ('blank-lines', '\n\none\n\n'),
    ('no-final-newline', 'one\ntwo'),
    ('unicode-separators', 'one two three\u0085four'),
    ('spaces', '  padded  \n\t\n'),
    ('empty', ''),
    ('only-cr', '\r'),
]:
    add(f'names/{which}', 'names', [o('get', uid=1), o('loadNames', file='botnames.txt'), o('loadNames', file='missing.txt')], files={'botnames.txt': text})
add('names/second-file', 'names', [o('loadNames', file='more.txt'), o('getRandomName'), o('getRandomName'), o('getRandomName'), o('getRandomName', expect='Index')],
    files={'botnames.txt': 'a\n', 'more.txt': 'b\nc\n'}, answers={'RandomFunction.random@3': 2, 'RandomFunction.random@2': 0, 'RandomFunction.random@1': 0})
add('names/get-random-name', 'names', [o('getRandomName')], sweep=0, answers={'RandomFunction.random': 1})

ARMOUR = 'sicriona:103:1163:1023:1725:1333:1127:1201:0:1079:0:2922:1061:0:\n'
add('file/construct', 'file', [o('newAIPlayerFile', file='armour.txt', loc=P), o('pvmBotsFile', file='armour.txt', loc=P)], sweep=0,
    files={'botnames.txt': 'a\n', 'armour.txt': ARMOUR})
add('file/reservoir', 'file', [o('updateRandomOSRScopyLine', file='armour.txt'), o('updateRandomOSRScopyLine', file='armour.txt'),
                               o('updateRandomOSRScopyLine', file='armour.txt')],
    files={'botnames.txt': 'a\n', 'armour.txt': 'first:1\nsecond:2\nthird:3\nfourth:4\n'}, answers={'Random.nextInt': [0, 1, 0, 2, 5, 0, 0, 0, 1, 1, 1, 0]})
add('file/skipped-lines', 'file', [o('updateRandomOSRScopyLine', file='armour.txt')],
    files={'botnames.txt': 'a\n', 'armour.txt': '# comment\nab\n\n   \nkept:9\n#last\nxy\n   \n\n'})
add('file/retry-until-clean', 'file', [o('newAIPlayerFile', file='armour.txt', loc=P)],
    files={'botnames.txt': 'a\n', 'armour.txt': 'under_score:1\nwith space:2\nclean:3\n'},
    answers={'Random.nextInt': [0, 1, 1, 1, 0, 1, 1, 1, 0]})
add('file/missing', 'file', [o('updateRandomOSRScopyLine', file='nothing.txt'), o('newAIPlayerFile', file='nothing.txt', loc=P, expect='Null')],
    files={'botnames.txt': 'a\n'})
add('file/kept-from-before', 'file', [o('updateRandomOSRScopyLine', file='armour.txt'), o('newAIPlayerFile', file='nothing.txt', loc=P)],
    files={'botnames.txt': 'a\n', 'armour.txt': 'earlier:5\n'})
add('file/only-colons', 'file', [o('newAIPlayerFile', file='armour.txt', loc=P, expect='Index')], files={'botnames.txt': 'a\n', 'armour.txt': ':::\n'})
add('file/carriage-returns', 'file', [o('updateRandomOSRScopyLine', file='armour.txt')],
    files={'botnames.txt': 'a\n', 'armour.txt': 'good:1\nlast\r\r'})
# A token is anything Character.isWhitespace does not cover: a no-break space is one, an em space is not, and the
# next-line control ends a line without being whitespace.
add('file/whitespace-kinds', 'file', [o('updateRandomOSRScopyLine', file='a.txt'), o('updateRandomOSRScopyLine', file='b.txt'), o('updateRandomOSRScopyLine', file='c.txt'),
                                      o('updateRandomOSRScopyLine', file='d.txt')],
    files={'botnames.txt': 'a\n', 'a.txt': 'kept:1\n\u2003\u2003\u2003\n\u3000\n', 'b.txt': 'first:1\n\u00a0\u00a0\u00a0\n', 'c.txt': 'first:1\n\u001c\u001d\u001e\u001f\u000b\u000c\n',
           'd.txt': 'first:1\u0085\u0085\u0085second:2\u2007\u2007\u2007\n\u202f\u202f\u202f'})
add('file/draw-fails', 'file', [o('updateRandomOSRScopyLine', file='armour.txt')], sweep=0, files={'botnames.txt': 'a\n', 'armour.txt': 'one:1\ntwo:2\n'})
add('file/no-colon', 'file', [o('newAIPlayerFile', file='armour.txt', loc=P)], files={'botnames.txt': 'a\n', 'armour.txt': 'plainname\n'})

# The private members nothing calls, reached by reflection.
for which, line, answers, expect in [
    ('typical', ARMOUR, {'RandomFunction.random2': [60, 40], 'RandomFunction.linearDecreaseRand': [5, 9, 30], 'RandomFunction.random': [20, 33],
                         'RandomFunction.normalRandDist': 44}, None),
    ('low-level', 'weak:3:0:0:0:0:0:0:0:0:0:0:0:0:\n', {'RandomFunction.random2': [2, 3]}, None),
    ('spent', 'mid:20:0:0:0:0:0:0:0:0:0:0:0:0:\n', {'RandomFunction.random2': [15, 80]}, None),
    ('prayer-spends', 'mid:30:0:0:0:0:0:0:0:0:0:0:0:0:\n', {'RandomFunction.random2': [30, 20], 'RandomFunction.random': [200, 7]}, None),
    ('capped', 'huge:900:0:0:0:0:0:0:0:0:0:0:0:0:\n', {'RandomFunction.random2': [99, 10], 'RandomFunction.random': [1, 1], 'RandomFunction.normalRandDist': 60}, None),
    ('negative', 'odd:-5:0:0:0:0:0:0:0:0:0:0:0:0:\n', {}, None),
    ('not-a-number', 'bad:abc:1\n', {}, 'Number'),
    ('no-level', 'lonely\n', {}, 'Index'),
]:
    add(f'private/setLevels/{which}', 'private', [o('newAIPlayerFile', file='armour.txt', loc=P), o('setLevels', bot='bot#1', **({'expect': expect} if expect else {}))],
        sweep=1 if which == 'typical' else None, files={'botnames.txt': 'a\n', 'armour.txt': line}, answers=answers)
for which, line, items, expect in [
    ('typical', ARMOUR, {}, None),
    ('null-names', 'n:1:1163:1023:0:1333:0:0:0:0:0:0:0:0:\n', {'1163': dict(name='null'), '1023': dict(name='NuLL')}, None),
    # 66699 and 1163 share their low 16 bits; the name is the definition's of the id as written.
    ('wide-id', 'n:1:66699:-1:132235:0:0:0:0:0:0:0:0:0:\n', {'66699': dict(name='null')}, None),
    ('wide-id-named', 'n:1:66699:-1:132235:0:0:0:0:0:0:0:0:0:\n', {'1163': dict(name='null')}, None),
    ('short-line', 'n:1:1163:1023\n', {}, 'Index'),
    ('not-a-number', 'n:1:x:1023:0:0:0:0:0:0:0:0:0:0:\n', {}, 'Number'),
    ('plus-sign', 'n:1:+7:-0:0:0:0:0:0:0:0:0:0:0:\n', {}, None),
    ('too-large', 'n:1:2147483648:0:0:0:0:0:0:0:0:0:0:0:\n', {}, 'Number'),
    ('smallest', 'n:1:-2147483648:2147483647:0:0:0:0:0:0:0:0:0:0:\n', {}, None),
]:
    add(f'private/giveArmor/{which}', 'private', [o('newAIPlayerFile', file='armour.txt', loc=P), o('giveArmor', bot='bot#1', **({'expect': expect} if expect else {}))],
        sweep=1 if which == 'typical' else None, files={'botnames.txt': 'a\n', 'armour.txt': line}, items=items)
add('private/no-line', 'private', [new(), o('setLevels', bot='bot#1', expect='Null'), o('giveArmor', bot='bot#1', expect='Null')])

# ================================================================ AIPlayer: tick, clear, reset, death
add('tick/plain', 'tick', [new(), o('tick', bot='bot#1'), o('tick', bot='bot#1')], sweep=1,
    answers={'SkullManager.isWilderness': [True, False], 'WildernessZone.getWilderness': 12, 'Skills.getLifepoints': 0})
add('tick/combat-alive', 'tick', [o('combatBot', loc=P), o('tick', bot='bot#1'), o('tick', bot='bot#1')], answers={'Skills.getLifepoints': [10, 10, 10, 1]})
add('tick/combat-dies', 'tick', [o('combatBot', loc=P), new([3201, 3200, 0]), o('tick', bot='bot#1'), o('tick', bot='bot#1'), o('get', uid=1), new([3205, 3200, 0])],
    sweep=2, answers={'Skills.getLifepoints': [5, 0, 0, 0]})
add('tick/pvm', 'tick', [o('pvmBots', loc=P)] + [o('tick', bot='bot#1')] * 4, sweep=1, answers={'Skills.getLifepoints': [50, 5, 9, 6, 0, 0, 100, 4]})
add('tick/pvm-wraps', 'tick', [o('pvmBots', loc=P)] + [o('tick', bot='bot#1')] * 101, answers={'Skills.getLifepoints': 30})
add('tick/combat-counts', 'tick', [o('combatBot', loc=P)] + [o('tick', bot='bot#1')] * 3, answers={'Skills.getLifepoints': 7, 'RandomFunction.random@5': 1, 'Random.nextInt': 3})
add('clear/plain', 'clear', [new(), new(), o('clear', bot='bot#1'), o('get', uid=1), o('deregister', uid=1), o('clear', bot='bot#1'), o('deregister', uid=2)], sweep=2)
add('clear/familiar-and-clan', 'clear', [new(), o('clear', bot='bot#1')], sweep=1,
    answers={'FamiliarManager.hasFamiliar': True, 'CommunicationInfo.getClan': 'clan', 'ClanRepository.getOwner': 'Owner', 'GameSettings.getWorldId': 7})
add('clear/clan-without-owner', 'clear', [new(), o('clear', bot='bot#1', expect='Null')], answers={'CommunicationInfo.getClan': 'clan'})
add('clear/clan-gone-between-reads', 'clear', [new(), o('clear', bot='bot#1', expect='Null')], answers={'CommunicationInfo.getClan': ['clan', None]})
add('deregister/known', 'clear', [new(), new(), o('deregister', uid=1), o('deregister', uid=1), o('deregister', uid=7), new()], sweep=2)
add('reset', 'tick', [new(), o('reset', bot='bot#1'), o('setLocation', node='bot#1', loc=[3210, 3211, 2]), o('reset', bot='bot#1'), o('reset', bot='bot#1')], sweep=3,
    answers={'PlayerFlags.isUpdateSceneGraph': [False, True, True]})
add('finalize-death', 'tick', [new(), o('finalizeDeath', bot='bot#1', killer='p'), o('finalizeDeath', bot='bot#1', killer=None)], sweep=1,
    nodes={'p': player()})
add('init-again', 'lifecycle', [new(), new(), o('init', bot='bot#1'), o('updateRandomValues', bot='bot#2'), o('update', bot='bot#1'),
                                o('interactNoop', bot='bot#1', node='p'), o('setHighPriority', bot='bot#1', value=True), o('setControler', bot='bot#2', player='p'),
                                o('setControler', bot='bot#2', player=None), o('session')],
    nodes={'p': player()}, answers={'RandomFunction.random@3': 1, 'Random.nextInt': 4})
add('combat-random-values', 'lifecycle', [o('combatBot', loc=P), o('combatBot', loc=P), o('updateRandomValues', bot='bot#1')], sweep=0,
    answers={'RandomFunction.random@5': [0, 1], 'RandomFunction.random@4': 1, 'Random.nextInt': [2, 5, 0]})

# ================================================================ AIPlayer: movement and searches
WALK_NODES = {'p': player(), 'n1': npc(1, 'Cow', [3203, 3200, 0])}
add('follow', 'walk', [new(), o('follow', bot='bot#1', entity='n1'), o('runPulse', pulse='last'), o('stopPulse', pulse='last')], sweep=1, nodes=WALK_NODES)
add('follow/pulse-fails', 'walk', [new(), o('follow', bot='bot#1', entity='n1'), o('runPulse', pulse='last')], sweep=2, nodes=WALK_NODES)
add('random-walk-around-point', 'walk', [new(), o('randomWalkAroundPoint', bot='bot#1', point=[3100, 3100, 2], radius=5)], sweep=1,
    answers={'RandomFunction.random2': [-3, 4]})
add('random-walk-around-point/no-point', 'walk', [new(), o('randomWalkAroundPoint', bot='bot#1', point=None, radius=5, expect='Null')])
add('random-walk', 'walk', [new([3210, 3220, 3]), o('randomWalk', bot='bot#1', x=4, y=9), o('randomWalk', bot='bot#1', x=-2147483648, y=0)], sweep=1,
    answers={'RandomFunction.random2': [2, -9, 100, 1], 'Pathfinder.find': True})
add('walk-to-pos', 'walk', [new(), o('walkToPosSmartXY', bot='bot#1', x=3300, y=3301), o('walkToPosSmart', bot='bot#1', loc=[3300, 3301, 2]), o('walkPos', bot='bot#1', x=1, y=2)],
    sweep=1)
add('walk-pos/fails', 'walk', [new(), o('walkPos', bot='bot#1', x=1, y=2)], sweep=1)
add('victim', 'search', [new(), o('checkVictimIsPlayer', bot='bot#1'), o('checkVictimIsPlayer', bot='bot#1'), o('checkVictimIsPlayer', bot='bot#1'),
                         o('checkVictimIsPlayer', bot='bot#1', expect='Null')], sweep=2,
    nodes=WALK_NODES, answers={'CombatPulse.getVictim': [None, 'p', 'p', 'n1', 'n1', 'p', None]})
add('item-by-id', 'search', [new(), o('getItemById', bot='bot#1', id=995), o('getItemById', bot='bot#1', id=4151), o('getItemById', bot='bot#1', id=1)], sweep=1,
    answers={'Container.get@bot#1.inventory#0': [[4151, 1]], 'Container.get@bot#1.inventory#3': [[995, 250]], 'Container.get@bot#1.inventory#27': [[995, 9]]})
SEARCH_NODES = {
    'n1': npc(81, 'Cow', [3202, 3200, 0]), 'n2': npc(81, 'Cow', [3200, 3201, 0]), 'n3': npc(7, 'Farmer', [3200, 3200, 0]), 'n4': npc(81, 'Cow', [3209, 3209, 0]),
    'o1': scenery(1276, 'Tree', [3201, 3201, 0], direction='NORTH'), 'o2': scenery(1276, 'Tree', [3200, 3202, 0], direction='SOUTH'),
    'o3': scenery(1278, 'Tree', [3199, 3200, 0], direction='NORTH'), 'o4': scenery(1276, 'Tree', [3198, 3200, 0], direction='NORTH'),
}
SEARCH_ANSWERS = {
    'RegionManager.getLocalNPCs': [['n1', 'n2', 'n3', 'n4']],
    'RegionManager.getObject@3201#3201': 'o1', 'RegionManager.getObject@3200#3202': 'o2', 'RegionManager.getObject@3199#3200': 'o3',
    'RegionManager.getObject@3198#3200': 'o4',
}
add('closest-node', 'search', [new(), o('getClosestNodeWithEntry', bot='bot#1', range=3, entry=1276), o('getClosestNodeWithEntry', bot='bot#1', range=3, entry=81),
                               o('getClosestNodeWithEntry', bot='bot#1', range=3, entry=5), o('getClosestNodeWithEntries', bot='bot#1', range=3, entries=[1278, 7]),
                               o('getClosestNodeWithEntries', bot='bot#1', range=4, entries=[]),
                               o('getClosestNodeWithEntryAndDirection', bot='bot#1', range=3, entry=1276, direction='SOUTH'),
                               o('getClosestNodeWithEntryAndDirection', bot='bot#1', range=3, entry=1276, direction='EAST'),
                               o('getClosestNodeWithEntryAndDirection', bot='bot#1', range=3, entry=81, direction='NORTH'),
                               o('getClosestNodeWithEntry', bot='bot#1', range=0, entry=81), o('getClosestNodeWithEntry', bot='bot#1', range=-2, entry=1276)],
    sweep=(1, 3), nodes=SEARCH_NODES, answers=SEARCH_ANSWERS)
add('closest-node/moved', 'search', [new(), o('setLocation', node='bot#1', loc=[3199, 3201, 0]), o('getClosestNodeWithEntry', bot='bot#1', range=3, entry=1276),
                                     o('setLocation', node='o1', loc=None), o('getClosestNodeWithEntry', bot='bot#1', range=3, entry=1276, expect='Null')],
    nodes=SEARCH_NODES, answers=SEARCH_ANSWERS)
add('closest-creature', 'search', [new(), o('getClosesCreature', bot='bot#1', radius=5), o('getClosesCreatureEntry', bot='bot#1', radius=5, entry=81),
                                   o('getClosesCreatureEntries', bot='bot#1', radius=5, entries=[81, 7]), o('getClosesCreatureEntries', bot='bot#1', radius=5, entries=[]),
                                   o('getClosesCreatureEntry', bot='bot#1', radius=0, entry=81), o('getClosesCreature', bot='bot#1', radius=0),
                                   o('getClosesCreature', bot='bot#1', radius=-1), o('getClosesCreatureEntry', bot='bot#1', radius=1, entry=81)],
    sweep=1, nodes=SEARCH_NODES, answers=dict(SEARCH_ANSWERS, **{'RegionManager.getLocalNPCs': [['n4', 'n1', 'n2', 'n3']]}))
# The best distance is kept as a truncated int: after a diagonal neighbour (1.41 -> 1) an adjacent one is not closer.
add('closest-creature/truncation', 'search', [new(), o('getClosesCreature', bot='bot#1', radius=5), o('getClosesCreature', bot='bot#1', radius=5)],
    nodes=dict(SEARCH_NODES, n5=npc(9, 'Guard', [3201, 3201, 0])), answers={'RegionManager.getLocalNPCs': [['n5', 'n2'], ['n2', 'n5']]})
add('closest-creature/unplaced-npc', 'search', [new(), o('setLocation', node='n2', loc=None), o('getClosesCreatureEntry', bot='bot#1', radius=5, entry=7, expect='Null')],
    nodes=SEARCH_NODES, answers=SEARCH_ANSWERS)

# ================================================================ AIRepository
REPO_NODES = {
    'p': player(), 'q': player('bob', [3210, 3200, 0]),
    'g1': ground(995, [3201, 3200, 0]), 'g2': ground(526, [3202, 3200, 0]), 'g3': ground(995, [3203, 3200, 0], dropper='q'),
    'g4': ground(1, [3200, 3200, 0], dropper=None),
}
SCRIPTS = {'s1': dict(), 's2': {'class': 'Idler'}, 's3': {'class': 'Adventurer', 'state': 'TELEPORTING'}, 's4': {'class': 'Adventurer'}}
add('repository/items', 'repository', [o('getItems', player='p'), o('addItem', item='g1'), o('addItem', item='g2'), o('addItem', item='g3'),
                                       o('addItem', item='g4', expect='Null'), o('addItem', item='g1'), o('getItems', player='p'), o('getItems', player='q')], nodes=REPO_NODES)
add('repository/offers', 'repository', [o('getOffer', player='p'), o('addOffer', player='p', id=995, amount=5), o('addOffer', player='q', id=1, amount=2),
                                        o('addOffer', player='p', id=4151, amount=1), o('getOffer', player='p'), o('getOffer', player='q'),
                                        o('setOffer', player='q', completed=2)], nodes=REPO_NODES)
add('repository/bot-info', 'repository',
    [new(), o('sendBotInfo', player='p', bot='bot#1'), o('gbcBot', script='s1', bot='bot#1'), o('sendBotInfo', player='p', bot='bot#1'),
     o('sendBotInfo', player='p', bot='bot#1'), o('sendBotInfo', player='p', bot='bot#1', expect='Null'), o('sendBotInfo', player='p', bot='bot#1', expect='Null')],
    sweep=4, nodes=REPO_NODES, scripts=SCRIPTS,
    answers={'PulseManager.hasPulseRunning@bot#1': [False, True, True, False], 'PulseManager.getCurrent@bot#1': ['foreign:plain', None],
             'ScriptProcessor.getActiveScript@bot#1': [None, None, 'active', 'active', 'active', 'active:null'], 'ScriptProcessor.getDelay': [0, 12]})
add('repository/bot-info-adventurer', 'repository',
    [new(), new(), o('gbcBot', script='s3', bot='bot#1'), o('gbcBot', script='s4', bot='bot#2'), o('setScript', script='s3', running=False),
     o('setPulseDelay', pulse='pulse#1', value=31), o('sendBotInfo', player='p', bot='bot#1'), o('sendBotInfo', player='p', bot='bot#2')],
    nodes=REPO_NODES, scripts=SCRIPTS, answers={'PulseManager.hasPulseRunning@bot#2': True, 'PulseManager.getCurrent@bot#2': 'foreign:movement'})
add('repository/bot-info-own-pulse', 'repository',
    [new(), o('gbcBot', script='s1', bot='bot#1'), o('follow', bot='bot#1', entity='p'), o('transitionNew', script='s2'), o('sendBotInfo', player='p', bot='bot#1'),
     o('sendBotInfo', player='p', bot='bot#1'), o('sendBotInfo', player='p', bot='bot#1')],
    nodes=REPO_NODES, scripts=SCRIPTS, answers={'PulseManager.hasPulseRunning@bot#1': True, 'PulseManager.getCurrent@bot#1': ['pulse#2', 'pulse#1', 'pulse#3']})
add('repository/clear-all', 'repository',
    [new(), new(), new(), o('gbcBot', script='s1', bot='bot#1'), o('gbcBot', script='s2', bot='bot#2'), o('gbcLoc', script='s3', loc=P), o('clearAllBots'), o('clearAllBots'),
     o('deregister', uid=1), o('deregister', uid=3), o('get', uid=4)],
    sweep=(6, 2), scripts=SCRIPTS)
add('repository/clear-all-server-stopped', 'repository',
    [new(), new(), o('gbcBot', script='s1', bot='bot#1'), o('gbcBot', script='s2', bot='bot#2'), o('clearAllBots'), o('clearAllBots')],
    scripts=SCRIPTS, config=dict(serverRunning=False), answers={'RandomFunction.roll': [True, False]})
add('repository/clear-all-many', 'repository',
    [new() for _ in range(14)] + [o('gbcBot', script=f's{k}', bot=f'bot#{k}') for k in range(1, 15)] + [o('clearAllBots')],
    scripts={f's{k}': dict() for k in range(1, 15)},
    files={'botnames.txt': ''.join(n + '\n' for n in ['zed', 'Yan', 'x_ray', 'will', 'vic', 'una', 'tom', 'sam', 'rob', 'quin', 'pat', 'oz', 'ned', 'max'])})
add('repository/clear-all-player-bot', 'repository',
    [new(), o('gbcBot', script='s1', bot='bot#1'), o('transitionNew', script='s1'), o('runPulse', pulse='pulse#2'), o('clearAllBots', expect='Cast'), o('clearAllBots')],
    nodes=REPO_NODES, scripts={'s1': dict(), 's5': dict(bot='p')}, answers={'Script.newInstance': 's5', 'RandomFunction.random2': 77})
add('repository/clear-all-no-bot', 'repository',
    [new(), o('gbcBot', script='s1', bot='bot#1'), o('setScript', script='s1', bot=None), o('clearAllBots', expect='Null')], scripts=SCRIPTS)
add('player-scripts', 'repository',
    [o('playerScript', identifier='fisher', description=['Fishes', 'at Draynor'], name='Fisher', script='s1'),
     o('playerScript', identifier='idle', description=[], name='Idles', script='s2'),
     o('playerScript', identifier='fisher', description=['Again'], name='Fisher two', script='s3')], scripts=SCRIPTS)

# ================================================================ Script
FULL_SCRIPT = dict(inventory=[[995, 1000], [379, 5]], equipment=[[4151, 1], [1163, 1]],
                   skills=[[0, 99], [16, 50], [1, 40], [23, 10], [17, 3], [32, 1], [2, 60]], quests=['COOKS_ASSISTANT', 'DEMON_SLAYER'])
add('script/init', 'script', [new(), o('setScript', script='s1', bot='bot#1'), o('scriptInit', script='s1', isPlayer=False), o('scriptInit', script='s1', isPlayer=True)],
    sweep=2, scripts={'s1': FULL_SCRIPT}, answers={'QuestRepository.getQuest': ['quest:cook', None]})
add('script/init-many-skills', 'script', [new(), o('setScript', script='s1', bot='bot#1'), o('scriptInit', script='s1', isPlayer=False)],
    scripts={'s1': dict(skills=[[k, 30 + k] for k in [23, 5, 17, 0, 12, 21, 3, 9, 16, 1, 20, 7, 14, 2]])})
add('script/init-player', 'script', [o('scriptInit', script='s1', isPlayer=True), o('scriptInit', script='s2', isPlayer=False), o('scriptInit', script='s3', isPlayer=False, expect='Null'),
                                     o('scriptInit', script='s3', isPlayer=True, expect='Null')],
    nodes={'p': player()}, scripts={'s1': dict(FULL_SCRIPT, bot='p'), 's2': dict(bot='p'), 's3': dict(FULL_SCRIPT)})
add('script/text', 'script', [new(), o('scriptToString', script='s1', expect='Null'), o('setScript', script='s1', bot='bot#1'), o('scriptToString', script='s1'),
                              o('scriptToString', script='s1'), o('setScript', script='s2', bot='p'), o('scriptToString', script='s2'),
                              o('setLocation', node='p', loc=None), o('scriptToString', script='s2', expect='Null')],
    sweep=3, nodes={'p': player()}, scripts=SCRIPTS, answers={'PulseManager.getCurrent@bot#1': [None, 'foreign:plain'], 'PulseManager.getCurrent@p': 'foreign:movement'})
add('script/set-level', 'script', [new(), o('setScript', script='s1', bot='bot#1'), o('scriptSetLevel', script='s1', skill=6, level=77),
                                   o('scriptSetLevel', script='s2', skill=6, level=77, expect='Null')], sweep=2, scripts=SCRIPTS)

# ================================================================ GeneralBotCreator
CREATOR_NODES = {'p': player(), 'q': player('Bob Two', None)}
add('creator/at-location', 'creator', [o('gbcLoc', script='s1', loc=P), o('runPulse', pulse='pulse#1'), o('stopPulse', pulse='pulse#1')], sweep=(0, 2),
    scripts={'s1': FULL_SCRIPT}, answers={'getWorldTicks': 41})
add('creator/for-bot', 'creator', [new(), o('gbcBot', script='s1', bot='bot#1'), o('gbcBot', script='s2', bot=None, expect='Null'), o('gbcBot', script='s3', bot='bot#1')],
    sweep=1, scripts=SCRIPTS, answers={'getWorldTicks': [5, 6]})
add('creator/for-player', 'creator', [o('gbcPlayer', script='s1', player='p', isPlayer=True), o('gbcPlayer', script='s2', player='p', isPlayer=False),
                                      o('gbcPlayer', script='s3', player='q', isPlayer=True, expect='Null'), o('runPulse', pulse='pulse#1'),
                                      o('stopPulse', pulse='pulse#2'), o('stopPulse', pulse='pulse#1')],
    sweep=1, nodes=CREATOR_NODES, scripts=dict(SCRIPTS, s2=FULL_SCRIPT))


def pulse_case(name, ops, sweep=None, script='s1', before=(), **kw):
    kw.setdefault('scripts', SCRIPTS)
    kw.setdefault('nodes', {'p': player()})
    add(f'pulse/{name}', 'pulse', [new(), *before, o('gbcBot', script=script, bot='bot#1'), *ops], sweep=sweep, **kw)


RUN = o('runPulse', pulse='pulse#1')
pulse_case('ticks', [RUN, RUN], sweep=2)
pulse_case('random-delay', [RUN, RUN, RUN, RUN, RUN, RUN], sweep=2, answers={'RandomFunction.roll': [True, False], 'RandomFunction.random2': [0, 3]})
pulse_case('idler-never-waits', [RUN, RUN], script='s2', answers={'RandomFunction.roll': True, 'RandomFunction.random2': 30})
pulse_case('priority-skips-roll', [o('setHighPriority', bot='bot#1', value=True), RUN, o('setHighPriority', bot='bot#1', value=False), RUN],
           answers={'RandomFunction.roll': True, 'RandomFunction.random2': [0, 49]})
pulse_case('not-running', [o('setScript', script='s1', running=False), RUN, o('setScript', script='s1', running=True), RUN])
pulse_case('limit', [o('setTriggered', value=74), RUN, RUN, RUN, o('gbcTick'), RUN, o('setTriggered', value=2147483647), RUN, RUN])
pulse_case('has-pulse', [RUN, RUN], answers={'PulseManager.hasPulseRunning': [True, True, False, True], 'PulseManager.getCurrent': [None]})
pulse_case('stuck', [RUN, RUN, RUN], sweep=2,
           answers={'PulseManager.hasPulseRunning': [True, False, True, False, True, False], 'PulseManager.getCurrent': 'foreign:movement', 'getWorldTicks': [100, 105, 106, 300]})
pulse_case('stuck-own-movement', [o('follow', bot='bot#1', entity='p'), RUN, RUN],
           answers={'PulseManager.hasPulseRunning': True, 'PulseManager.getCurrent': ['pulse#2', 'pulse#2', 'pulse#1'], 'getWorldTicks': [0, 6]})
pulse_case('stuck-pulse-gone', [o('runPulse', pulse='pulse#1', expect='Null')],
           answers={'PulseManager.hasPulseRunning': True, 'PulseManager.getCurrent': ['foreign:movement', None], 'getWorldTicks': [0, 9]})
pulse_case('moved', [o('setLocation', node='bot#1', loc=[3201, 3200, 0]), RUN, RUN, o('setLocation', node='bot#1', loc=[3201, 3200, 1]), RUN],
           answers={'PulseManager.hasPulseRunning': True, 'PulseManager.getCurrent': 'foreign:movement', 'getWorldTicks': [10, 20, 26, 26, 2147483647, -2147483648, 3]})
pulse_case('moved-nowhere', [o('setLocation', node='bot#1', loc=None), o('runPulse', pulse='pulse#1', expect='Null')],
           answers={'PulseManager.hasPulseRunning': True, 'PulseManager.getCurrent': 'foreign:movement'})
pulse_case('plain-pulse-running', [o('setLocation', node='bot#1', loc=None), RUN],
           answers={'PulseManager.hasPulseRunning': True, 'PulseManager.getCurrent': 'foreign:plain'})
pulse_case('dialogue', [RUN, RUN, o('setScript', script='s1', endDialogue=False), RUN, RUN], sweep=2,
           answers={'ScriptProcessor.getActiveScript': ['active', 'active', 'active', None, 'active', 'active:null', None, None],
                    'Player.hasModalOpen': [True, False, True]})
pulse_case('stop', [o('stopPulse', pulse='pulse#1'), o('stopPulse', pulse='pulse#1'), RUN], sweep=2, answers={'RandomFunction.roll': [False, True]})
pulse_case('stop-server-stopped', [o('stopPulse', pulse='pulse#1')], config=dict(serverRunning=False))
pulse_case('stop-without-bot', [o('setScript', script='s1', bot=None), o('stopPulse', pulse='pulse#1', expect='Null'), o('runPulse', pulse='pulse#1', expect='Null')])
pulse_case('stop-while-waiting', [RUN, o('stopPulse', pulse='pulse#1')], answers={'RandomFunction.roll': True, 'RandomFunction.random2': [0, 20]})
pulse_case('plain-player-bot', [o('setScript', script='s1', bot='p'), RUN, o('stopPulse', pulse='pulse#1')])
pulse_case('transition', [o('transitionNew', script='s2'), o('setScript', script='s3', bot='bot#1'), o('runPulse', pulse='pulse#2'), o('runPulse', pulse='pulse#2', expect='Null'),
                          o('runPulse', pulse='pulse#2', expect='Null'), o('stopPulse', pulse='pulse#2')],
           sweep=4, answers={'Script.newInstance': ['s3', None, 's4'], 'RandomFunction.random2': [0, 133]})

# ================================================================ ScriptAPI
API_NODES = {
    'p': player(),
    'n1': npc(81, 'Cow', [3202, 3200, 0]), 'n2': npc(81, 'Cow', [3200, 3203, 0]), 'n3': npc(7, 'Farmer', [3201, 3201, 0]), 'n4': npc(81, 'cow', [3230, 3230, 0]),
    'n5': npc(2, 'Man', [3199, 3199, 0], active=False), 'n6': npc(3, 'Woman', [3204, 3204, 0]), 'n7': npc(81, 'COW', [3200, 3197, 0]), 'n8': npc(5, None, [3200, 3201, 0]),
    'n9': npc(6, 'Nowhere', None),
    'o1': scenery(2213, 'Bank booth', [3205, 3200, 0]), 'o2': scenery(2214, 'Bank Booth', [3201, 3200, 0]), 'o3': scenery(28089, 'Desk', [3200, 3206, 0]),
    'o4': scenery(1276, 'Tree', [3203, 3203, 0]), 'o5': scenery(1277, 'Tree', [3202, 3202, 0]), 'o6': scenery(1276, 'Tree', [3200, 3201, 0]),
    'g1': ground(995, [3201, 3200, 0]), 'g2': ground(995, [3203, 3200, 0]), 'g3': ground(526, [3200, 3201, 0]), 'g4': ground(995, [3215, 3200, 0]),
    'g5': ground(995, [3209, 3200, 0]), 'g6': ground(995, [3210, 3200, 0]),
    'i1': dict(kind='item', id=1511, amount=3),
}
NODE_LIST = f'Region.assembleNodeList@{REGION}#0'
OBJECT_LIST = f'Region.assembleObjectList@{REGION}#0'


def api(name, ops, sweep=None, **kw):
    kw.setdefault('nodes', API_NODES)
    add(f'api/{name}', 'api', ops, sweep=sweep, **kw)


def a(operation, /, **kw):
    return dict(op=operation, api='p', **kw)


api('distance', [a('apiDistance', a='p', b='n3'), a('apiDistance', a='n4', b='p'), a('apiDistance', a='p', b='p'), a('apiDistance', a='p', b='n9', expect='Null'),
                 a('apiDistance', a='i1', b='p', expect='Null')])
OPTIONS = {'Interaction.getOptions@o1': [['Walk here', None, 'Open', 'open']], 'Interaction.getOptions@n1': [['?', 'Attack']], 'Interaction.getOptions@i1': [['drop']]}
api('interact', [a('interact', player='p', node='o1', option='open'), a('interact', player='p', node=None, option='open'), a('interact', player='p', node='p', option='open'),
                 a('interact', player='p', node='n1', option='ATTACK'), a('interact', player='p', node='i1', option='Drop'), a('interact', player='p', node='g1', option='take'),
                 a('interact', player='p', node='o1', option='close'), a('interact', player='p', node='o2', option='open')],
    sweep=0, answers=dict(OPTIONS, **{'InteractionListeners.run@1511': True}))
api('interact/listener-handles', [a('interact', player='p', node='o1', option='OPEN')], sweep=0, answers=dict(OPTIONS, **{'InteractionListeners.run': True}))
api('interact/no-plugin', [a('interact', player='p', node='o1', option='open', expect='Null'), a('interact', player='p', node='o1', option='open', expect='Null'),
                           a('interact', player='p', node='o1', option='nothing')],
    answers=dict(OPTIONS, **{'Node.getInteraction@o1': [False, True, False, True]}))
api('interact/other-player', [new(), dict(op='interact', api='bot#1', player='p', node='n1', option='attack')], answers=OPTIONS)

USE = {'Container.getItem@p.inventory': [[1511, 4]]}
api('use-with', [a('useWith', player='p', item=1511, node='o4'), a('useWith', player='p', item=1511, node=None), a('useWith', player='p', item=1511, node='p'),
                 a('useWith', player='p', item=1511, node='n1', expect='Cast'), a('useWith', player='p', item=1511, node='i1', expect='Cast'),
                 a('useWith', player='p', item=1511, node='g1', expect='Cast')], sweep=0, answers=USE)
api('use-with/listener-handles', [a('useWith', player='p', item=70000, node='o4')], sweep=0, answers=dict(USE, **{'InteractionListeners.runUse': True}))
api('use-with/child', [a('useWith', player='p', item=1511, node='o4'), a('useWith', player='p', item=1511, node='o4'), a('useWith', player='p', item=1511, node='o4'),
                       a('useWith', player='p', item=1511, node='o5')], sweep=1,
    answers=dict(USE, **{'Scenery.getChild@o4': ['o5', 'o5', 'o6'], 'Scenery.getChild@o5': 'o5', 'InteractionListeners.runUse': [False, False, False, True, False, False]}))
api('use-with/plugin-handles', [a('useWith', player='p', item=1511, node='o4')], sweep=0, answers=dict(USE, **{'PluginInteractionManager.handle': True}))
api('use-with/no-item', [a('useWith', player='p', item=1511, node='o4', expect='Null'), a('useWith', player='p', item=1511, node='n1', expect='Cast')],
    answers={'Scenery.getChild@o4': 'o5'})
api('send-chat', [a('sendChat', message='Selling lobs 250 ea'), a('sendChat', message='')], sweep=0)

NEAREST = {NODE_LIST: [['n2', 'n1', None, 'n4', 'n5', 'n7', 'n3', 'n8']], OBJECT_LIST: [['o1', 'o2', 'o3', 'o4', 'o5', 'o6']]}
api('nearest/by-names', [a('getNearestNodeFromList', names=['cow'], isObject=False), a('getNearestNodeFromList', names=['Farmer', 'COW'], isObject=False),
                         a('getNearestNodeFromList', names=['bank booth'], isObject=True), a('getNearestNodeFromList', names=[], isObject=False),
                         a('getNearestNodeFromList', names=['tree', 'desk'], isObject=True)], sweep=0, answers=NEAREST)
api('nearest/move-near', [a('getNearestNodeFromList', names=['cow'], isObject=False), a('getNearestNodeName', name='Cow'), a('getNearestNodeNameObj', name='Cow', isObject=True)],
    answers=dict(NEAREST, **{'Pathfinder.find@p#n1': True, 'Pathfinder.find@p#n7': [False, True, True]}))
api('nearest/by-id', [a('getNearestNodeId', id=81, isObject=False), a('getNearestNodeId', id=2213, isObject=True), a('getNearestNodeId', id=-1, isObject=True),
                      a('getNearestNodeId', id=-1, isObject=False), a('getNearestNodeId', id=99, isObject=False)], sweep=0, answers=NEAREST)
api('nearest/short-reach', [a('getNearestNodeId', id=1276, isObject=True), a('getNearestNodeFromList', names=['Desk'], isObject=True)],
    answers=NEAREST, config=dict(maxPathfind=4))
api('nearest/no-reach', [a('getNearestNodeId', id=-1, isObject=False)], answers=NEAREST, config=dict(maxPathfind=-1))
api('nearest/predicate', [a('getNearestObjectByPredicate'), a('getNearestObjectByPredicate'), a('getNearestObjectByPredicate')], sweep=0,
    answers=dict(NEAREST, **{'Predicate.invoke@o4': True, 'Predicate.invoke@o6': [True, True, False], 'Pathfinder.find@p#o6': [False, True]}))
api('nearest/unplaced-node', [a('getNearestNodeName', name='nowhere', expect='Null'), a('setLocation', node='p', loc=None), a('getNearestNodeName', name='cow', expect='Null')],
    answers={NODE_LIST: [['n5', 'n9']]})
api('nearest/other-plane', [a('setLocation', node='p', loc=[3200, 3200, 2]), a('getNearestNodeName', name='cow'), a('getNearestNodeId', id=2213, isObject=True)],
    answers={f'Region.assembleNodeList@{REGION}#2': [['n1']], f'Region.assembleObjectList@{REGION}#2': [['o1']]})
api('viability', [a('evaluateViability', node=None, min=10.0, max=10.0), a('evaluateViability', node='n5', min=10.0, max=10.0),
                  a('evaluateViability', node='n1', min=10.0, max=10.0), a('evaluateViability', node='n1', min=10.0, max=1.5),
                  a('evaluateViability', node='n1', min=1.5, max=10.0), a('evaluateViability', node='n1', min=2.0, max=2.0),
                  a('evaluateViability', node='n1', min=9.0, max=9.0, id=81, names=['pig']), a('evaluateViability', node='n1', min=9.0, max=9.0, id=81, names=['pig', 'cOw']),
                  a('evaluateViability', node='n1', min=9.0, max=9.0, id=80, names=['cow']), a('evaluateViability', node='n1', min=9.0, max=9.0, id=81, predicate=True),
                  a('evaluateViability', node='n1', min=9.0, max=9.0, names=['cow'], predicate=True), a('evaluateViability', node='n8', min=9.0, max=9.0, names=['x']),
                  a('evaluateViability', node='n8', min=9.0, max=9.0), a('evaluateViability', node='n1', min=9.0, max=9.0, id=80, predicate=True),
                  a('setActive', node='n1', value=False), a('evaluateViability', node='n1', min=9.0, max=9.0), a('setActive', node='n1', value=True),
                  a('evaluateViability', node='n1', min=9.0, max=9.0)],
    sweep=9, answers={'Predicate.invoke': [True, False]})

DROPS = {'GroundItemManager.contains': True}
api('ground/listed', [a('addItem', item='g2'), a('addItem', item='g3'), a('addItem', item='g4'), a('addItem', item='g1'), a('takeNearestGroundItem', id=995),
                      a('takeNearestGroundItem', id=526), a('takeNearestGroundItem', id=4151)], sweep=4, answers=dict(DROPS, **{'Interaction.get@g1#2': 'take'}))
api('ground/gone', [a('addItem', item='g1'), a('addItem', item='g2'), a('takeNearestGroundItem', id=995), a('takeNearestGroundItem', id=995),
                    a('takeNearestGroundItem', id=995)], answers={'GroundItemManager.contains': [False, True, False]})
api('ground/distance', [a('addItem', item='g6'), a('addItem', item='g5'), a('addItem', item='g4'), a('takeNearestGroundItem', id=995),
                        a('setLocation', node='p', loc=[3201, 3200, 0]), a('takeNearestGroundItem', id=995), a('setLocation', node='p', loc=[3220, 3200, 0]),
                        a('takeNearestGroundItem', id=995)], answers=DROPS)
api('ground/attribute', [a('takeNearestGroundItem', id=995), a('takeNearestGroundItem', id=995), a('takeNearestGroundItem', id=995), a('takeNearestGroundItem', id=995),
                         a('takeNearestGroundItem', id=526)], sweep=1,
    answers={'Entity.getAttribute@p': [None, ['g4', 'g3', 'g2', 'g1', 'g2'], ['g4', 'g3'], [], ['g1', 'g3', 'g3']], 'Interaction.get': [None, 'take']})
api('ground/no-plugin', [a('addItem', item='g1'), a('takeNearestGroundItem', id=995), a('takeNearestGroundItem', id=995, expect='Null')],
    answers=dict(DROPS, **{'Node.getInteraction@g1': [False, True, False]}))
api('ground/unplaced', [a('addItem', item='g1'), a('setLocation', node='g1', loc=None), a('takeNearestGroundItem', id=995, expect='Null'),
                        a('setLocation', node='g1', loc=P), a('setLocation', node='p', loc=None), a('takeNearestGroundItem', id=995, expect='Null')])
api('game-object', [a('getNearestGameObject', loc=P, id=1276), a('getNearestGameObject', loc=[3200, 3200, 1], id=1276), a('getNearestGameObject', loc=P, id=5),
                    a('getNearestGameObject', loc=[1, 1, 3], id=1277)], sweep=0,
    answers={OBJECT_LIST: [['o4', None, 'o5', 'o6', 'o1']], f'Region.assembleObjectList@{REGION}#1': [[]], 'Region.assembleObjectList@0#3': [['o5']]})

TARGETS = {'RegionManager.getLocalNPCs': [['n1', 'n5', 'n2', 'n3', 'n7', 'n4']], 'NPCDefinition.hasAction': True}
api('attack/in-radius', [a('attackNpcsInRadius', player='p', radius=7), a('attackNpcsInRadius', player='p', radius=7)], sweep=0,
    answers=dict(TARGETS, **{'RandomFunction.getRandom': [1, 0], 'Properties.isMultiZone@n2': True, 'Entity.inCombat@n2': True, 'Entity.inCombat@n3': True,
                             'NPCDefinition.hasAction@n7': False}))
api('attack/busy', [a('attackNpcsInRadius', player='p', radius=7), a('attackNpcInRadius', player='p', name='Cow', radius=7)], answers=dict(TARGETS, **{'Entity.inCombat@p': True}))
api('attack/nothing', [a('attackNpcsInRadius', player='p', radius=3), a('attackNpcsInRadius', player='p', radius=3), a('attackNpcInRadius', player='p', name='Goblin', radius=3),
                       a('attackNpcInRadius', player='p', name='cow', radius=3)],
    answers={'RegionManager.getLocalNPCs': [[], ['n5'], ['n1', 'n2'], ['n1', 'n2', 'n3', 'n5', 'n5', 'n4']], 'NPCDefinition.hasAction': True})
api('attack/named', [a('attackNpcInRadius', player='p', name='Cow', radius=9), a('attackNpcInRadius', player='p', name='Cow', radius=9, expect='Index'),
                     a('attackNpcInRadius', player='p', name='Cow', radius=9, expect='Index')], sweep=0,
    answers=dict(TARGETS, **{'RandomFunction.getRandom': [1, 2, -1]}))
api('attack/not-an-npc', [a('attackNpcsInRadius', player='p', radius=3, expect='Cast'), a('attackNpcsInRadius', player='p', radius=3)],
    answers={'RegionManager.getLocalNPCs': [['n1', 'p'], ['n1', 'n2', 'n3', 'n7', 'n6', 'p']], 'NPCDefinition.hasAction': True})
api('attack/unplaced', [a('setLocation', node='p', loc=None), a('attackNpcsInRadius', player='p', radius=3)], answers=TARGETS)

api('walk-to', [a('walkTo', loc=[3300, 3250, 0]), a('runPulse', pulse='last'), a('walkTo', loc=[3203, 3202, 0]), a('walkTo', loc=P), a('walkTo', loc=[3200, 3100, 3]),
                a('walkTo', loc=[3100, 3100, 0]), a('walkTo', loc=[3201, 3200, 0])], sweep=0, answers={'WalkingQueue.isMoving': [False, False, False, False, True, False]})
api('walk-to/planes', [a('setLocation', node='p', loc=[3200, 3200, 2]), a('walkTo', loc=[3210, 3190, 1]), a('setLocation', node='p', loc=[10, 10, 0]), a('walkTo', loc=[-10, -3, 0]),
                       a('setLocation', node='p', loc=None), a('walkTo', loc=P, expect='Null')])
api('walk-to/reach', [a('walkTo', loc=[3300, 3200, 0]), a('walkTo', loc=[3202, 3200, 0])], config=dict(maxPathfind=1))
api('walk-to/long-reach', [a('walkTo', loc=[3300, 3301, 0])], config=dict(maxPathfind=1000))
api('walk-to/no-reach', [a('walkTo', loc=[3300, 3200, 0])], config=dict(maxPathfind=0))
STEPS = [[3210, 3200, 0], [3220, 3200, 0], [3230, 3200, 0]]
api('walk-array', [a('walkArray', steps=STEPS), a('runPulse', pulse='pulse#1'), a('setLocation', node='p', loc=[3206, 3200, 0]), a('runPulse', pulse='pulse#1'),
                   a('runPulse', pulse='pulse#1'), a('setLocation', node='p', loc=[3216, 3200, 0]), a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1'),
                   a('setLocation', node='p', loc=[3228, 3200, 0]), a('runPulse', pulse='pulse#1'), a('setLocation', node='p', loc=[3229, 3201, 1]),
                   a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1')],
    sweep=1, answers={'WalkingQueue.isMoving': [False, True, False]})
api('walk-array/empty', [a('walkArray', steps=[]), a('runPulse', pulse='pulse#1'), a('walkArray', steps=[P]), a('runPulse', pulse='pulse#2'),
                         a('setLocation', node='p', loc=None), a('runPulse', pulse='pulse#2', expect='Null'), a('runPulse', pulse='pulse#1')], sweep=0)
api('random-walk-to', [a('randomWalkTo', loc=[3300, 3300, 0], radius=6), a('randomWalkTo', loc=[3300, 3300, 0], radius=6), a('randomWalkTo', loc=P, radius=-2147483648)],
    sweep=0, answers={'WalkingQueue.isMoving': [False, True, False], 'RandomFunction.random2': [6, -6, 0]})
# kotlin.random.Random cannot be seeded: every range here holds one value, so the original's draw is known.
api('randomize-location', [a('randomizeLocationInRanges', loc=[3200, 3200, 0], xMin=-3, xMax=-2, yMin=10, yMax=11, z=2),
                           a('randomizeLocationInRanges', loc=[100, 200, 3], xMin=0, xMax=1, yMin=-1, yMax=0, z=5),
                           a('randomizeLocationInRanges', loc=P, xMin=5, xMax=5, yMin=0, yMax=9, z=0, expect='Argument'),
                           a('randomizeLocationInRanges', loc=P, xMin=7, xMax=8, yMin=3, yMax=2, z=0, expect='Argument'),
                           a('randomizeLocationInRanges', loc=P, xMin=6, xMax=-6, yMin=0, yMax=1, z=0, expect='Argument'),
                           a('randomizeLocationInRanges', loc=P, xMin=-100, xMax=-99, yMin=5000, yMax=5001, z=-1)])
api('teleport', [a('teleportToGE'), a('runPulse', pulse='pulse#1'), a('teleport', loc=[2964, 3378, 1]), a('runPulse', pulse='pulse#2'), a('teleportToGE'),
                 a('teleport', loc=P), a('stopPulse', pulse='pulse#2')], sweep=0, answers={'Entity.isTeleBlocked': [False, False, True]})
api('teleport/to', [a('teleport', loc=[2964, 3378, 1])], sweep=0)
api('teleport/pulse', [a('teleportToGE'), a('runPulse', pulse='pulse#1')], sweep=1)

BANK = {'Container.getAmount@p.bank': 30}
api('sell/one', [a('sellOnGE', id=377), a('runPulse', pulse='pulse#1')], sweep=1,
    answers=dict(BANK, **{'ItemDefinition.getNoteId@377': [377, 378], 'GrandExchange.addBotOffer': True, 'ItemDefinition.getAlchemyValue': 160}),
    items={'378': dict(name='Raw LOBSTER')})
api('sell/one-variants', [a('sellOnGE', id=377), a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1'),
                          a('runPulse', pulse='pulse#1'), a('sellOnGE', id=66000), a('runPulse', pulse='pulse#2')],
    answers=dict(BANK, **{'ItemDefinition.getNoteId@377': [-1, 377, -1, 377, 377, 0, 5], 'GrandExchange.addBotOffer': [True, True, False, True, True],
                          'ItemDefinition.getAlchemyValue': [16, 17, 2000, 71582789], 'ItemDefinition.getNoteId@66000': [66000, -1]}))
api('sell/limit', [a('sellOnGE', id=377), a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1')], config=dict(geLimit=31, geName='GE Bot'),
    answers=dict(BANK, **{'GrandExchange.addBotOffer': True, 'ItemDefinition.getAlchemyValue': [1, 2]}))
api('sell/no-settings', [a('saleIsBigNews', id=1, amount=10), a('saleIsBigNews', id=1, amount=10), a('saleIsBigNews', id=1, amount=-2147483648),
                         a('saleIsBigNews', id=1, amount=3)], sweep=0, config=dict(geLimit=None), answers={'ItemDefinition.getAlchemyValue': [50, 49, 2, 1431655766]})
BANK_ITEMS = [[995, 100], None, [379, 5], [373, 2], [385, 1], [1511, 40], [4151, 1], [1513, 3], [2, 2147483647], [1515, 9]]
SELL_ALL = {'Container.toArray@p.bank': [BANK_ITEMS], 'ItemDefinition.isTradeable@995': False, 'GrandExchange.addBotOffer': True, 'ItemDefinition.getAlchemyValue': 300,
            'ItemDefinition.getNoteId@4151': [4151, 4152], 'ItemDefinition.getNoteId@1513': [1513, -1], 'GrandExchange.addBotOffer@4152': False,
            'ItemDefinition.getAlchemyValue@1511': 1}
api('sell/all', [a('sellAllOnGe'), a('runPulse', pulse='pulse#1')], sweep=(1, 2), answers=SELL_ALL)
api('sell/all-adv', [a('sellAllOnGeAdv'), a('runPulse', pulse='pulse#1')], sweep=(1, 2), answers=dict(SELL_ALL, **{NODE_LIST: [['o3', 'n1']]}))
api('sell/all-adv-no-desk', [a('sellAllOnGeAdv'), a('sellAllOnGeAdv', expect='Cast')], sweep=0,
    nodes=dict(API_NODES, n10=npc(1, 'Desk', [3200, 3201, 0])), answers={NODE_LIST: [['o1', 'n1'], ['n10']]})
INVENTORY = [[1511, 5], None, [1359, 1], [590, 1], [1271, 1], [995, 77], [379, 3], [1511, 5]]
api('deposit', [a('depositAtBank'), a('runPulse', pulse='pulse#1')], sweep=1, answers={NODE_LIST: [['o1', 'o2']], 'Container.toArray@p.inventory': [INVENTORY]})
api('deposit/no-bank', [a('depositAtBank'), a('depositAtBank', expect='Cast'), a('depositAtBank')], sweep=2,
    nodes=dict(API_NODES, n10=npc(1, 'bank booth', [3200, 3201, 0])), answers={NODE_LIST: [['n1'], ['n10'], ['o1']]})
api('bank-item', [a('bankItem', item=1511), a('runPulse', pulse='pulse#1'), a('bankItem', item=-1), a('runPulse', pulse='pulse#2')], sweep=1,
    answers={'Container.getAmount@p.inventory': [27, 0]})
api('bank-all', [a('bankAll', onComplete=True), a('runPulse', pulse='pulse#1'), a('bankAll', onComplete=False), a('runPulse', pulse='pulse#2')], sweep=1,
    answers={'Container.toArray@p.inventory': [INVENTORY, [None, [4151, 1]]], 'Container.remove@p.inventory': [True, False, True]})
api('bank-all/hand-off', [a('bankAll', onComplete=True)], sweep=0)

HUNGRY = {'Skills.getStaticLevel@p#3': 40, 'RandomFunction.randomDouble': 0.5, 'Skills.getLifepoints@p': 20, 'Container.containsItem@p.inventory': True,
          'Container.getItem@p.inventory': [[379, 1]]}
api('eat', [a('eat', food=379)], sweep=0, answers=dict(HUNGRY, **{'Consumables.getConsumableById@379': 'LOBSTER'}))
api('eat/thresholds', [a('eat', food=379), a('eat', food=379), a('eat', food=379), a('eat', food=379)],
    answers=dict(HUNGRY, **{'RandomFunction.randomDouble': [0.5, 0.75, 0.6, 0.524999], 'Skills.getLifepoints@p': [21, 30, 24, 21], 'Container.containsItem@p.inventory': [False, True]}))
api('eat/plain-food', [a('eat', food=1965), a('eat', food=66001, expect='Null')], sweep=0,
    answers=dict(HUNGRY, **{'Container.getItem@p.inventory': [[1965, 2], None]}))
api('force-eat', [a('forceEat', food=379), a('forceEat', food=379), a('forceEat', food=66001), a('forceEat', food=7)], sweep=0,
    answers={'Container.containsItem@p.inventory': [True, False, True, True], 'Container.getItem@p.inventory': [[379, 1], [465, 1], None],
             'Consumables.getConsumableById@379': 'LOBSTER'})

BUY_HOOK = [dict(event='PulseManager.run', at=1, actions=[dict(op='setOffer', player='p', completed=7), dict(op='runPulse', pulse='last')])]
api('buy', [a('buyFromGE', player='p', id=314, amount=7), a('runLaunched', task=1), a('getOffer', player='p')], sweep=1, hooks=BUY_HOOK,
    answers={'ItemDefinition.getConfiguration@314': 12})
api('buy/hand-off', [a('buyFromGE', player='p', id=314, amount=7)], sweep=0)
api('buy/incomplete', [a('buyFromGE', player='p', id=314, amount=8), a('runLaunched', task=1), a('buyFromGE', player='p', id=4151, amount=7), a('runLaunched', task=2)],
    hooks=BUY_HOOK + [dict(event='PulseManager.run', at=2, actions=[dict(op='setOffer', player='p', completed=7), dict(op='runPulse', pulse='last')])],
    answers={'ItemDefinition.getValue': 55})
api('buy/for-another', [new(), dict(op='buyFromGE', api='bot#1', player='p', id=1, amount=0), a('runLaunched', task=1)],
    hooks=[dict(event='PulseManager.run', at=1, actions=[dict(op='runPulse', pulse='last')])])
api('withdraw', [a('withdraw', id=995, amount=500), a('withdraw', id=995, amount=500), a('withdraw', id=995, amount=500), a('withdraw', id=995, amount=500),
                 a('withdraw', id=995, amount=0), a('withdraw', id=66531, amount=3)], sweep=1,
    answers={'Container.containsItem@p.bank': [True, False, False, True, True, True], 'Container.getAmount@p.bank': [120, 0],
             'Container.hasSpaceFor@p.inventory': [True, False, False, True], 'Container.getMaximumAdd@p.inventory': [14, -3]})

EQUIP_ITEMS = {
    '4151': dict(name='Abyssal whip', equipment_slot=3, requirements=[[0, 70]]),
    '1163': dict(name='Rune full helm', equipment_slot=0, requirements=[[1, 40]]),
    '4675': dict(name='Ancient staff', equipment_slot=3, requirements=[[6, 50], [0, 50], [22, 1], [16, 7], [17, 8], [32, 9]]),
    '1': dict(name='Toolkit'),
    '2': dict(name='Cannonball', equipment_slot='thirteen'),
    '3': dict(name='Odd', equipment_slot=5, requirements='levels'),
    '4': dict(name='Free', equipment_slot=12),
    '5': dict(name='Many', equipment_slot=1, requirements=[[k, 10 + k] for k in [23, 5, 17, 0, 12, 21, 3, 9, 16, 1, 20, 7, 14, 2]]),
}
api('equip', [a('equipAndSetStats', item=[4675, 1]), a('equipAndSetStats', item=[1, 1]), a('equipAndSetStats', item=[2, 1], expect='Cast'),
              a('equipAndSetStats', item=[3, 1], expect='Cast'), a('equipAndSetStats', item=[4, 9]), a('equipAndSetStats', item=[5, 1]),
              a('equipAndSetStatsList', items=None), a('equipAndSetStatsList', items=[]), a('equipAndSetStatsList', items=[[4151, 1], [1, 1], [1163, 1]]),
              a('equipAndSetStatsList', items=[[1163, 1], [2, 1], [4151, 1]], expect='Cast')], sweep=0, items=EQUIP_ITEMS)
APPEARANCE = '{"appearance":{"gender":1,"skin":3},"equipment":[{"slot":0,"id":1163},{"slot":3,"id":4151}]}'
LOAD = {'Container.capacity@p.equipment': 4, 'Container.get@p.equipment#0': [[1163, 1]], 'Container.get@p.equipment#3': [[4151, 1]], 'Container.get@p.equipment#2': [[1, 1]],
        'Skills.getHighestCombatSkillId': 2, 'Skills.getStaticLevel@p#2': 70, 'Skills.getStaticLevel@p#0': 70, 'Skills.getStaticLevel@p#1': 40, 'Skills.getStaticLevel@p#4': 53}
api('load-appearance', [a('loadAppearanceAndEquipment', json=APPEARANCE), a('loadAppearanceAndEquipment', json=None)], sweep=(0, 2), answers=LOAD, items=EQUIP_ITEMS)
api('load-appearance/shapes', [a('loadAppearanceAndEquipment', json='{"equipment":[]}', expect='Null'),
                               a('loadAppearanceAndEquipment', json='{"appearance":[],"equipment":[]}', expect='Cast'),
                               a('loadAppearanceAndEquipment', json='{"appearance":null,"equipment":[]}', expect='Null'),
                               a('loadAppearanceAndEquipment', json='{"appearance":{}}', expect='Null'),
                               a('loadAppearanceAndEquipment', json='{"appearance":{},"equipment":{}}', expect='Cast'),
                               a('loadAppearanceAndEquipment', json='{"appearance":{},"equipment":7}', expect='Cast'),
                               a('loadAppearanceAndEquipment', json='{"appearance":{},"equipment":[]}'),
                               a('loadAppearanceAndEquipment', json='{"appearance":{},"equipment":[]}', expect='Cast')],
    answers={'Container.capacity@p.equipment': [0, 14], 'Container.get@p.equipment#13': [[2, 1]], 'Skills.getHighestCombatSkillId': 6, 'Skills.getStaticLevel@p#6': 3},
    items=EQUIP_ITEMS)
api('overlay', [a('overlay', title='Fishing %R now', label='%00ff00lobs', amount=-12)], sweep=0)
api('price', [a('checkPriceOverrides', id=314), a('checkPriceOverrides', id=315)], sweep=0, answers={'ItemDefinition.getConfiguration@314': 9})

# ================================================================ AIPBuilder, CombatBot, PvMBots, PvMBotsBuilder
add('builder/create', 'builder', [o('aipCreate', loc=[3094, 3491, 0]), o('aipCopy', player='p'), o('aipCopyAt', player='p', loc=[3100, 3100, 1]),
                                  o('aipCopy', player='q')], sweep=1, nodes={'p': player(), 'q': player('Bob', None)})
add('builder/afk', 'builder', [o('randomAfkPlayer', loc=P), o('randomAfkPlayer', loc=None)], sweep=0,
    answers={'RandomFunction.random@3': [0, 1, 0, 0], 'RandomFunction.getRandom': [99, 0, 50, 1, 2, 3]})
add('builder/immersive', 'builder', [o('immersiveSpawns')], files={'botnames.txt': ''.join(f'name{k}\n' for k in range(12))},
    answers={'RandomFunction.random@3': [1, 0, 1], 'RandomFunction.getRandom': [10, 20, 30, 40, 50, 60, 70]})
add('builder/immersive-runs-out', 'builder', [o('immersiveSpawns', expect='Index')], files={'botnames.txt': 'a\nb\nc\n'})
EAT = {'Skills.getStaticLevel': 60, 'Skills.getLifepoints': 20, 'Container.containsItem': True, 'Container.getItem': [[379, 2]]}
add('combat-bot/eat', 'builder', [o('combatBot', loc=P), o('eatBot', bot='bot#1', food=379)], sweep=1,
    answers=dict(EAT, **{'Consumables.getConsumableById@379': 'LOBSTER'}))
add('combat-bot/eat-variants', 'builder',
    [o('combatBot', loc=P), o('eatBot', bot='bot#1', food=4), o('eatBot', bot='bot#1', food=379), o('eatBot', bot='bot#1', food=379), o('eatBot', bot='bot#1', food=379),
     o('eatBot', bot='bot#1', food=379, expect='Null'), o('eatBot', bot='bot#1', food=379)],
    nodes={'p': player()},
    answers={'Skills.getStaticLevel': 60, 'Skills.getLifepoints': [10, 10, 10, 21, 20, -5], 'Container.containsItem': [True, False, True, True],
             'Container.getItem': [[4, 1], [0, 1], None, [379, 1]], 'CombatPulse.getVictim': [None, 'p', 'p', None], 'Container.contains': [False, True, False],
             'Consumables.getConsumableById@379': 'LOBSTER'})
add('pvm/eat', 'builder', [o('pvmBots', loc=P), o('eatBot', bot='bot#1', food=379), o('eatBot', bot='bot#1', food=379, expect='Null'),
                           o('eatBot', bot='bot#1', food=379, expect='Null'), o('eatBot', bot='bot#1', food=379)], sweep=1,
    answers=dict(EAT, **{'Consumables.getConsumableById': ['LOBSTER', None, 'LOBSTER'], 'Container.getItem': [[379, 2], [379, 2], None], 'Skills.getLifepoints': [10, 10, 20, 21]}))
add('prayer', 'builder', [o('combatBot', loc=P), o('pvmBots', loc=P), o('checkPrayer', bot='bot#1', types=['PIETY', None, 'THICK_SKIN']),
                          o('checkPrayer', bot='bot#2', types=['PROTECT_FROM_MELEE']), o('checkPrayer', bot='bot#2', types=[])], sweep=2)
add('pvm/targets', 'builder',
    [o('pvmBots', loc=P), o('findTargets', bot='bot#1', entity='p', radius=6), o('findTargets', bot='bot#1', entity='bot#1', radius=6),
     o('findTargets', bot='bot#1', entity='p', radius=6, expect='Cast'), o('checkValidTargets', bot='bot#1', target='n5'), o('checkValidTargets', bot='bot#1', target='n1'),
     o('attackNpcsInRadiusPvm', bot='bot#1', radius=4), o('attackNpcsInRadiusPvm', bot='bot#1', player='p', radius=4),
     o('attackNpcsInRadiusPvm', bot='bot#1', player='p', radius=4), o('attackNpcsInRadiusPvm', bot='bot#1', radius=4, expect='Index')],
    sweep=1, nodes=API_NODES,
    answers={'RegionManager.getLocalNPCs': [['n1', 'n5', 'n2', 'n3', 'n7', 'n4'], [], ['p'], ['n1', 'n2'], ['n1', 'n2'], ['n3']], 'NPCDefinition.hasAction': True,
             'Properties.isMultiZone@n2': True, 'Entity.inCombat@n2': True, 'Entity.inCombat@n3': [True, False], 'NPCDefinition.hasAction@n7': False,
             'Entity.inCombat@bot#1': [False, False], 'Entity.inCombat@p': [True, False], 'RandomFunction.getRandom': [1, 1]})
add('pvm/builder', 'builder', [o('pvmCreate', loc=P), o('pvmCreatePc', second=False, loc=[2657, 2639, 0]), o('pvmCreatePc', second=True, loc=[2657, 2639, 0]),
                               o('pvmCreatePc', second=True, loc=None, expect='Null'), o('pvmCreatePc', second=False, loc=None, expect='Null'), o('pvmCreate', loc=None)],
    sweep=1)

# ================================================================ CombatBotAssembler, SkillingBotAssembler
def _tables():
    """The item tables of the two assemblers, read from the pinned sources: which ids a case must define."""
    combat = (BOTS / 'CombatBotAssembler.kt').read_text(encoding='utf-8')
    tables = {name: [int(x) for x in re.findall(r'\d+', body)] for name, body in re.findall(r'^\s*val ([A-Z_]+)\s*=\s*arrayOf\(([^)]*)\)', combat, re.M)}
    skilling = (BOTS / 'SkillingBotAssembler.kt').read_text(encoding='utf-8')
    for name in ['POORSETS', 'AVGSETS', 'RICHSETS']:
        block = skilling[skilling.index(f'val {name} = arrayOf('):]
        block = block[:block.index('\n\n')] if '\n\n' in block else block
        tables[name] = [[int(x) for x in re.findall(r'Items\.[A-Z0-9_]+?_(\d+)\b', m)] for m in re.findall(r'listOf\((.*?)\)', block)]
    return tables


TABLES = _tables()
# The first item of every combat table asks for nothing, so each table always has an item that fits.
FREE = {ids[0] for name, ids in TABLES.items() if not name.endswith('SETS')}


def spec(id):
    """A made-up definition for an item id: a slot, and for most ids one to three level requirements."""
    h = (id * 2654435761) & 0xFFFFFFFF
    out = dict(equipment_slot=id % 14)
    kind = 0 if id in FREE else h % 6
    first = [[0, 1, 2, 4, 6][(h >> 8) % 5], 1 + (h >> 12) % 70]
    if kind in (1, 2, 3):
        out['requirements'] = [first]
    elif kind == 4:
        out['requirements'] = [first, [[1, 16, 23][(h >> 20) % 3], 1 + (h >> 22) % 60]]
    elif kind == 5:
        out['requirements'] = [[17, 1 + (h >> 9) % 50], first, [5, 1 + (h >> 16) % 45]]
    return out


def items_for(*names, **override):
    ids = sorted({i for name in names for i in TABLES[name]})
    items = {str(i): spec(i) for i in ids}
    for k, v in override.items():
        items[k.lstrip('i')] = v
    return items


MELEE = ['MELEE_HELMS', 'MELEE_LEG', 'MELEE_SHIELD', 'MELEE_TOP', 'CAPE', 'NGLOVES', 'NBOOTS', 'MELEE_WEP']
RANGED = ['RANGE_HELMS', 'RANGE_TOPS', 'RANGE_LEGS', 'CAPE', 'NGLOVES', 'NRBOOTS', 'CROSSBOWS', 'MELEE_SHIELD', 'BOWS']
PC_RANGED = ['PCRANGE_HELMS', 'PCRANGE_TOPS', 'PCRANGE_BACK', 'PCRANGE_LEGS', 'NECK', 'GLOVES', 'BOOTS', 'RING_ARCH', 'PCCROSSBOWS', 'MELEE_SHIELD', 'PCBOWS']
PC_MELEE = ['PCMELEE_HELMS', 'PCMELEE_LEG', 'PCMELEE_SHIELD', 'PCMELEE_TOP', 'PCMELEE_WEP', 'CAPE', 'NECK', 'GLOVES', 'BOOTS', 'RING_BERS']
# The pick among equally good items is the last of them.
TIES = {f'kotlin.Random.index@{n}': n - 1 for n in range(2, 30)}
CB = o('combatBot', loc=P)


def asm(name, ops, sweep=None, **kw):
    add(f'assemble/{name}', 'assemble', ops, sweep=sweep, **kw)


asm('stats/low', [CB, o('generateStats', bot='bot#1', tier='LOW', skills=[0, 2, 1])], sweep=1,
    answers={'RandomFunction.random@33': 20, 'RandomFunction.randomDouble': [0.25, -0.5, 0.49], 'Skills.getExperience': [4470.0, 13.5, 101333.25], 'Skills.levelFromXP': 31})
asm('stats/med', [CB, o('generateStats', bot='bot#1', tier='MED', skills=[4, 1])],
    answers={'RandomFunction.random2': 40, 'RandomFunction.randomDouble': [0.5, -0.5], 'Skills.getExperience': [0.1, 0.2], 'Skills.levelFromXP': -3})
asm('stats/high', [CB, o('generateStats', bot='bot#1', tier='HIGH', skills=[0, 1, 2, 6, 4])],
    answers={'RandomFunction.random2': 99, 'RandomFunction.randomDouble': [0.031, -0.03, 0.0304, -4.0, 1.0e10], 'Skills.getExperience': 13034431.0, 'Skills.levelFromXP': 99})
asm('stats/pure', [CB, o('generateStats', bot='bot#1', tier='PURE', skills=[0])], sweep=1,
    answers={'RandomFunction.random2': 90, 'RandomFunction.randomDouble': -0.3, 'Skills.getExperience': 5346332.0, 'Skills.levelFromXP': 70})
asm('stats/no-skills', [CB, o('generateStats', bot='bot#1', tier='LOW', skills=[]), o('generateStats', bot='bot#1', tier='PURE', skills=[])],
    answers={'RandomFunction.random@33': 0})
asm('stats/clamped', [CB, o('generateStats', bot='bot#1', tier='LOW', skills=[0, 1, 2, 3]), o('generateStats', bot='bot#1', tier='MED', skills=[0, 1])],
    answers={'RandomFunction.random@33': -7, 'RandomFunction.random2': 2147483647, 'RandomFunction.randomDouble': [0.0, 100.0, -100.0, 0.3, 0.5, -0.5]})
LVL = {'10': dict(equipment_slot=1), '11': dict(equipment_slot=1, requirements=[[0, 40]]), '12': dict(equipment_slot=1, requirements=[[0, 40], [1, 40]]),
       '13': dict(requirements=[[0, 99], [1, 99], [2, 99], [16, 1]]), '14': dict(requirements='none'), '15': dict(requirements=[]), '65546': dict(requirements=[[0, 9]]),
       '16': dict(requirements=[[0, 2147483647], [1, 1]]), '17': dict(requirements=[[0, -5]])}
asm('lvl-avg', [o('lvlAvg', item=[10, 1]), o('lvlAvg', item=[11, 1]), o('lvlAvg', item=[12, 1]), o('lvlAvg', item=[13, 1]), o('lvlAvg', item=[14, 1], expect='Cast'),
                o('lvlAvg', item=[15, 1]), o('lvlAvg', item=[16, 1]), o('lvlAvg', item=[17, 1]), o('lvlAvg', item=[65546, 1])], sweep=1, items=LVL)
for level in (1, 45, 99):
    asm(f'gear-melee/level-{level}', [CB, o('gearMeleeBot', bot='bot#1')], sweep=(1, 23) if level == 45 else None, items=items_for(*MELEE),
        answers=dict(TIES, **{'Skills.getLevel': level}))
asm('gear-melee/first-pick', [CB, o('gearMeleeBot', bot='bot#1')], items=items_for(*MELEE), answers={'Skills.getLevel': 60})
asm('gear-melee/mixed-levels', [CB, o('gearMeleeBot', bot='bot#1')], items=items_for(*MELEE),
    answers=dict(TIES, **{'Skills.getLevel@bot#1#0': 70, 'Skills.getLevel@bot#1#1': 20, 'Skills.getLevel@bot#1#2': 99, 'Skills.getLevel@bot#1#17': 50,
                          'Skills.getLevel@bot#1#5': 43, 'Skills.getLevel@bot#1#16': 30, 'Skills.getLevel@bot#1#23': 1}))
HELMS = TABLES['MELEE_HELMS']
asm('gear-melee/nothing-fits', [CB, o('gearMeleeBot', bot='bot#1', expect='NoSuchElement')],
    items=dict(items_for(*MELEE), **{str(i): dict(equipment_slot=0, requirements=[[1, 2]]) for i in HELMS}))
asm('gear-melee/no-slot', [CB, o('gearMeleeBot', bot='bot#1', expect='Null')],
    items=dict(items_for(*MELEE), **{str(i): dict() for i in HELMS}))
asm('gear-melee/slot-is-text', [CB, o('gearMeleeBot', bot='bot#1', expect='Cast')],
    items=dict(items_for(*MELEE), **{str(i): dict(equipment_slot='head') for i in HELMS}))
asm('gear-melee/requirements-are-text', [CB, o('gearMeleeBot', bot='bot#1', expect='Cast')],
    items=dict(items_for(*MELEE), **{str(HELMS[2]): dict(equipment_slot=0, requirements='strong')}))
asm('gear-melee/pick-out-of-range', [CB, o('gearMeleeBot', bot='bot#1', expect='Index'), o('gearMeleeBot', bot='bot#1', expect='Index')],
    items=dict(items_for(*MELEE), **{str(i): dict(equipment_slot=0) for i in HELMS}), answers={'kotlin.Random.index': [len(HELMS), -1]})
asm('gear-ranged/bow', [CB, o('gearRangedBot', bot='bot#1', crossbow=False), o('gearRangedBot', bot='bot#1', crossbow=None)], sweep=(1, 19),
    items=items_for(*RANGED), answers=dict(TIES, **{'Skills.getLevel': 50}))
asm('gear-ranged/crossbow', [CB, o('gearRangedBot', bot='bot#1', crossbow=True)], sweep=(1, 29), items=items_for(*RANGED), answers=dict(TIES, **{'Skills.getLevel': 61}))
asm('produce', [o('produce', type='RANGE', tier='LOW', loc=P), o('produce', type='MELEE', tier='MED', loc=[3210, 3210, 0]), o('produce', type='MAGE', tier='HIGH', loc=[3211, 3210, 0]),
                o('produce', type='MELEE', tier='LOW', loc=None, expect='Null'), o('combatBot', loc=None, expect='Null')],
    items=items_for(*MELEE, *RANGED), answers=dict(TIES, **{'Skills.getLevel': 40, 'Random.nextInt0': 2, 'RandomFunction.random2': 50}))
asm('ranged-bot/draws', [o('assembleRangedBot', tier='LOW', loc=P, crossbow=None), o('assembleRangedBot', tier='LOW', loc=P, crossbow=None),
                         o('assembleRangedBot', tier='LOW', loc=P, crossbow=None), o('assembleRangedBot', tier='LOW', loc=P, crossbow=True),
                         o('assembleRangedBot', tier='LOW', loc=P, crossbow=False)],
    items=items_for(*RANGED), answers=dict(TIES, **{'Skills.getLevel': 70, 'Random.nextInt0': [-2147483648, 7, -3]}))
asm('ranged-bot/fails', [o('assembleRangedBot', tier='PURE', loc=P, crossbow=None)], sweep=(0, 37), items=items_for(*RANGED),
    answers=dict(TIES, **{'Skills.getLevel': 70, 'Random.nextInt0': 4}))
asm('melee-bot', [o('assembleMeleeBot', tier='HIGH', loc=P)], sweep=(0, 41), items=items_for(*MELEE), answers=dict(TIES, **{'Skills.getLevel': 80, 'RandomFunction.random2': 70}))
asm('melee-adventurer', [o('meleeAdventurer', tier='MED', loc=P)], sweep=(0, 43), items=items_for(*MELEE, 'NNECK'),
    answers=dict(TIES, **{'Skills.getLevel': 55, 'RandomFunction.random2': [44, 50]}))
asm('range-adventurer', [o('rangeAdventurer', tier='LOW', loc=P)], sweep=(0, 47),
    items=items_for('RANGE_HELMS', 'RANGE_TOPS', 'RANGE_LEGS', 'CROSSBOWS', 'CAPE', 'NRANGENECK', 'NRANGESHIELD', 'PCRANGE_BACK', 'NGLOVES', 'NRBOOTS'),
    answers=dict(TIES, **{'Skills.getLevel': 99, 'RandomFunction.random2': [69, 20]}))
asm('melee-dragon', [o('assembleMeleeDragonBot', tier='HIGH', loc=P)], sweep=(0, 53), items=items_for(*MELEE), answers=dict(TIES, **{'Skills.getLevel': 99}))
asm('range-dragon', [o('assembleRangeDragonBot', tier='MED', loc=P)], sweep=(0, 59), items=items_for(*RANGED), answers=dict(TIES, **{'Skills.getLevel': 99}))
asm('caps/too-low', [o('assembleRangeDragonBot', tier='MED', loc=P, expect='NoSuchElement')],
    items=dict(items_for(*RANGED), **{str(i): dict(equipment_slot=0, requirements=[[4, 51]]) for i in TABLES['RANGE_HELMS']}), answers={'Skills.getLevel': 99})
asm('caps/at-the-cap', [o('assembleRangeDragonBot', tier='MED', loc=P)],
    items=dict(items_for(*RANGED), **{str(i): dict(equipment_slot=0, requirements=[[4, 50]]) for i in TABLES['RANGE_HELMS']}), answers={'Skills.getLevel': 99})
asm('pc/novice-ranged', [CB, o('gearPCnRangedBot', bot='bot#1', crossbow=False), o('gearPCnRangedBot', bot='bot#1', crossbow=True)], sweep=(1, 31),
    items=items_for(*PC_RANGED), answers=dict(TIES, **{'Skills.getLevel': 50, 'RandomFunction.random2': [30, 70]}))
asm('pc/novice-melee', [CB, o('gearPCnMeleeBot', bot='bot#1')], sweep=(1, 61), items=items_for(*PC_MELEE),
    answers=dict(TIES, **{'Skills.getLevel': 75, 'RandomFunction.random2': 75}))
asm('pc/intermediate-ranged', [CB, o('gearPCiRangedBot', bot='bot#1', crossbow=True), o('gearPCiRangedBot', bot='bot#1', crossbow=None)], sweep=(1, 67),
    items=items_for(*PC_RANGED), answers=dict(TIES, **{'Skills.getLevel': 80, 'RandomFunction.random2': [50, 80]}))
asm('pc/intermediate-melee', [CB, o('gearPCiMeleeBot', bot='bot#1')], sweep=(1, 71), items=items_for(*PC_MELEE),
    answers=dict(TIES, **{'Skills.getLevel': 95, 'RandomFunction.random2': 95}))
asm('pc/ring-does-not-fit', [CB, o('gearPCnMeleeBot', bot='bot#1', expect='NoSuchElement'), o('gearPCiRangedBot', bot='bot#1', crossbow=False, expect='NoSuchElement')],
    items=dict(items_for(*PC_MELEE, *PC_RANGED), **{'6737': dict(equipment_slot=12, requirements=[[0, 100]]), '6733': dict(equipment_slot=12, requirements=[[4, 100]])}),
    answers=dict(TIES, **{'Skills.getLevel': 99}))


def set_items(ids, **override):
    items = {str(i): spec(i) for i in ids}
    for k, v in override.items():
        items[k.lstrip('i')] = v
    return items


POOR, AVERAGE, RICH = TABLES['POORSETS'], TABLES['AVGSETS'], TABLES['RICHSETS']
add('skilling/poor', 'assemble', [o('skillingProduce', wealth='POOR', loc=P), o('skillingProduce', wealth='POOR', loc=P), o('skillingProduce', wealth='POOR', loc=P)], sweep=1,
    items=set_items(POOR[0] + POOR[18] + POOR[len(POOR) - 2]), answers={f'kotlin.Random.index@{len(POOR)}': [0, 18, len(POOR) - 2]})
add('skilling/average', 'assemble', [o('skillingProduce', wealth='AVERAGE', loc=P), o('skillingProduce', wealth='AVERAGE', loc=None, expect='Null')],
    items=set_items(AVERAGE[32] + AVERAGE[len(AVERAGE) - 1]), answers={f'kotlin.Random.index@{len(AVERAGE)}': [32, len(AVERAGE) - 1]})
add('skilling/rich', 'assemble', [o('skillingProduce', wealth='RICH', loc=P), o('skillingProduce', wealth='RICH', loc=P, expect='Index'),
                                  o('skillingProduce', wealth='RICH', loc=P, expect='Index'), o('assembleBot', bot='bot#1', wealth='RICH'),
                                  o('assembleBot', bot='bot#2', wealth='POOR'), o('assembleBot', bot='bot#3', wealth='AVERAGE')],
    items=set_items(RICH[33] + RICH[30] + POOR[5] + AVERAGE[1]),
    answers={f'kotlin.Random.index@{len(RICH)}': [33, len(RICH), -1, 30], f'kotlin.Random.index@{len(POOR)}': 5, f'kotlin.Random.index@{len(AVERAGE)}': 1})
# Every set of the three tables, on one bot.
for wealth, sets in [('POOR', POOR), ('AVERAGE', AVERAGE), ('RICH', RICH)]:
    add(f'skilling/every-{wealth.lower()}-set', 'assemble', [new()] + [o('assembleBot', bot='bot#1', wealth=wealth) for _ in sets],
        items=set_items(sorted({i for ids in sets for i in ids})), answers={'kotlin.Random.index': list(range(len(sets)))})
SET = [4151, 1163, 9999, 1127, 4151, 1079]
add('skilling/equip-set', 'assemble', [new(), o('equipSet', bot='bot#1', set=SET), o('equipSet', bot='bot#1', set=[]), o('equipSet', bot='bot#1', set=[2, 4151], expect='Cast'),
                                       o('equipSet', bot='bot#1', set=[1163, 3, 4151], expect='Cast')], sweep=1,
    items=dict(EQUIP_ITEMS, **{'1127': dict(equipment_slot=4, requirements=[[1, 40], [17, 2], [16, 3]]), '1079': dict(equipment_slot=7)}),
    answers={'Container.get@bot#1.inventory#3': [None, [995, 1]], 'Container.get@bot#1.inventory#4': [[1511, 1]]})

# ================================================================ cases added for deliberate breaks the corpus first missed
add('file/blank-line-kept', 'file', [o('updateRandomOSRScopyLine', file='a.txt'), o('updateRandomOSRScopyLine', file='b.txt')],
    files={'botnames.txt': 'a\n', 'a.txt': 'kept:1\n   \nx\n', 'b.txt': 'first:1\n   '}, answers={'Random.nextInt': [1, 0, 1, 0, 0]})
add('closest-node/ties', 'search', [new(), o('getClosestNodeWithEntry', bot='bot#1', range=3, entry=81), o('getClosestNodeWithEntry', bot='bot#1', range=3, entry=81),
                                    o('getClosestNodeWithEntryAndDirection', bot='bot#1', range=3, entry=1276, direction='NORTH')],
    nodes=dict(SEARCH_NODES, n6=npc(81, 'Cow', [3198, 3200, 0]), o5=scenery(1276, 'Tree', [3200, 3198, 0], direction='NORTH')),
    answers=dict(SEARCH_ANSWERS, **{'RegionManager.getLocalNPCs': [['n1', 'n6'], ['n6', 'n1'], []], 'RegionManager.getObject@3200#3198': 'o5'}))
api('walk-array/edges', [a('walkArray', steps=[[3210, 3200, 0], [3220, 3200, 0]]), a('setLocation', node='p', loc=[3204, 3200, 0]), a('runPulse', pulse='pulse#1'),
                         a('setLocation', node='p', loc=[3205, 3200, 0]), a('runPulse', pulse='pulse#1'), a('setLocation', node='p', loc=[3217, 3200, 0]),
                         a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1'), a('runPulse', pulse='pulse#1')])
api('walk-array/near-the-end', [a('walkArray', steps=[[3210, 3200, 0], [3220, 3200, 0]]), a('setLocation', node='p', loc=[3217, 3200, 0]), a('runPulse', pulse='pulse#1'),
                                a('setLocation', node='p', loc=[3218, 3200, 0]), a('runPulse', pulse='pulse#1')])
api('sell/note-zero', [a('sellOnGE', id=379), a('runPulse', pulse='pulse#1')],
    answers=dict(BANK, **{'ItemDefinition.getNoteId@379': [379, 0], 'GrandExchange.addBotOffer': True, 'ItemDefinition.getAlchemyValue': 1}))
api('buy/over-complete', [a('buyFromGE', player='p', id=314, amount=8), a('runLaunched', task=1)],
    hooks=[dict(event='PulseManager.run', at=1, actions=[dict(op='setOffer', player='p', completed=9), dict(op='runPulse', pulse='last')])])
WEAPONS = TABLES['MELEE_WEP']
asm('caps/weapon-above-the-cap', [o('assembleMeleeDragonBot', tier='HIGH', loc=P), o('assembleMeleeDragonBot', tier='HIGH', loc=P)],
    items=dict(items_for(*MELEE), **{str(WEAPONS[5]): dict(equipment_slot=3, requirements=[[0, 61], [2, 61]]), str(WEAPONS[9]): dict(equipment_slot=3, requirements=[[0, 60], [2, 60]])}),
    answers=dict(TIES, **{'Skills.getLevel': 99}))
add('repository/player-bot-name', 'repository',
    [new(), o('gbcBot', script='s1', bot='bot#1'), o('transitionNew', script='s1'), o('runPulse', pulse='pulse#2'), o('scriptToString', script='s5'),
     o('scriptToString', script='s6'), o('stopPulse', pulse='pulse#3')],
    nodes={'q': player('Bob_TWO', [3210, 3200, 0]), 'x': player(None, [1, 2, 3])}, scripts={'s1': dict(), 's5': dict(bot='q'), 's6': dict(bot='x')},
    answers={'Script.newInstance': 's5', 'RandomFunction.random2': 61})
add('pvm/pest-control-count', 'builder', [o('pvmCreatePc', second=False, loc=None, expect='Null'), o('pvmCreatePc', second=True, loc=P)], sweep=1)

# Every call a stand-in can record (but the IoSession base members, which ArtificialSession overrides) and every
# operation of Support.java: generation fails when the corpus stops reaching one.
NEEDED_EVENTS = [
    '@kotlin.Random.nextInt', '@printStackTrace', 'Animator.reset', 'Appearance.copy', 'Appearance.parse', 'Appearance.setGender', 'Appearance.sync',
    'CharacterDesign.randomize', 'ClanRepository.getOwner', 'CombatPulse.delayNextAttack', 'CombatPulse.getVictim', 'CommunicationInfo.getClan',
    'Consumable.consume', 'Consumables.getConsumableById', 'Container.add', 'Container.capacity', 'Container.clear', 'Container.contains',
    'Container.containsItem', 'Container.copy', 'Container.get', 'Container.getAmount', 'Container.getAmountItem', 'Container.getItem',
    'Container.getMaximumAdd', 'Container.hasSpaceFor', 'Container.parse', 'Container.refresh', 'Container.remove', 'Container.replace',
    'Container.toArray', 'DialogueInterpreter.close', 'Entity.attack', 'Entity.clear', 'Entity.face', 'Entity.faceLocation', 'Entity.getAttribute',
    'Entity.inCombat', 'Entity.isTeleBlocked', 'Entity.lock', 'Entity.lockFor', 'Entity.sendChat', 'Entity.setAttribute', 'Entity.unlock',
    'Entity.visualize', 'EquipmentContainer.add3', 'EquipmentContainer.add4', 'Familiar.clear', 'FamiliarManager.hasFamiliar', 'GameSettings.getWorldId',
    'GlobalScope.launch', 'GrandExchange.addBotOffer', 'GroundItemManager.contains', 'HouseManager.leave', 'ImpactHandler.setDisabledTicks',
    'Interaction.get', 'Interaction.getOptions', 'Interaction.handle', 'InteractionListeners.run', 'InteractionListeners.runUse',
    'InterfaceManager.close', 'InterfaceManager.closeChatbox', 'InterfaceManager.closeSingleTab', 'InterfaceManager.openChatbox',
    'InterfaceManager.openDefaultTabs', 'InterfaceManager.openOverlay', 'ItemDefinition.getAlchemyValue', 'ItemDefinition.getConfiguration',
    'ItemDefinition.getNoteId', 'ItemDefinition.getValue', 'ItemDefinition.handlers.get', 'ItemDefinition.isTradeable', 'ManagementEvents.publish',
    'MovementPulse.new', 'NPCDefinition.hasAction', 'Node.getInteraction', 'Node.setDirection', 'Node.setLocation', 'OnComplete.invoke',
    'PacketDispatch.sendInterfaceConfig', 'PacketDispatch.sendString', 'Path.walk', 'Pathfinder.find', 'PestControlTestBot.new',
    'Player.checkForWealthUpdate', 'Player.debug', 'Player.finalizeDeath', 'Player.fullRestore', 'Player.hasModalOpen', 'Player.init', 'Player.new',
    'Player.reset', 'Player.tick', 'PlayerDetails.save', 'PlayerDetails.setSession', 'PlayerFlags.isUpdateSceneGraph', 'PlayerFlags.setLastSceneGraph',
    'PluginInteractionManager.handle', 'Prayer.toggle', 'Predicate.invoke', 'Properties.isMultiZone', 'Properties.setSpawnLocation',
    'Properties.setTeleportLocation', 'Pulse.stop', 'PulseManager.clear', 'PulseManager.getCurrent', 'PulseManager.hasPulseRunning', 'PulseManager.run',
    'Pulser.submit', 'QuestRepository.getQuest', 'QuestRepository.setStage', 'Random.nextInt', 'Random.nextInt0', 'RandomFunction.getRandom',
    'RandomFunction.linearDecreaseRand', 'RandomFunction.normalRandDist', 'RandomFunction.random', 'RandomFunction.random2',
    'RandomFunction.randomDouble', 'RandomFunction.roll', 'Region.assembleNodeList', 'Region.assembleObjectList', 'RegionManager.forId',
    'RegionManager.getLocalNPCs', 'RegionManager.getObject', 'Repository.sendGrandExchangeNews', 'Scenery.getChild', 'Script.newInstance', 'Script.tick',
    'ScriptProcessor.getActiveScript', 'ScriptProcessor.getDelay', 'Settings.setRunToggled', 'Skills.addExperience', 'Skills.copy',
    'Skills.getExperience', 'Skills.getHighestCombatSkillId', 'Skills.getLevel', 'Skills.getLifepoints', 'Skills.getStaticLevel', 'Skills.levelFromXP',
    'Skills.setLevel', 'Skills.setLifepoints', 'Skills.setStaticLevel', 'Skills.updateCombatLevel', 'SkullManager.isWilderness', 'SkullManager.setLevel',
    'UpdateMasks.register', 'UpdateSequence.renderablePlayers.remove', 'UseWithHandler.run', 'WalkingQueue.isMoving', 'WalkingQueue.reset',
    'WildernessZone.getWilderness', 'ZoneMonitor.clear', 'getWorldTicks', 'kotlin.Random.index', 'log'
]
NEEDED_OPERATIONS = [
    'addItem', 'addOffer', 'aipCopy', 'aipCopyAt', 'aipCreate', 'apiDistance', 'assembleBot', 'assembleMeleeBot', 'assembleMeleeDragonBot',
    'assembleRangeDragonBot', 'assembleRangedBot', 'attackNpcInRadius', 'attackNpcsInRadius', 'attackNpcsInRadiusPvm', 'bankAll', 'bankItem', 'buyFromGE',
    'checkPrayer', 'checkPriceOverrides', 'checkValidTargets', 'checkVictimIsPlayer', 'clear', 'clearAllBots', 'combatBot', 'depositAtBank', 'deregister',
    'eat', 'eatBot', 'equipAndSetStats', 'equipAndSetStatsList', 'equipSet', 'evaluateViability', 'finalizeDeath', 'findTargets', 'follow', 'forceEat',
    'gbcBot', 'gbcLoc', 'gbcPlayer', 'gbcTick', 'gearMeleeBot', 'gearPCiMeleeBot', 'gearPCiRangedBot', 'gearPCnMeleeBot', 'gearPCnRangedBot',
    'gearRangedBot', 'generateStats', 'get', 'getClosesCreature', 'getClosesCreatureEntries', 'getClosesCreatureEntry', 'getClosestNodeWithEntries',
    'getClosestNodeWithEntry', 'getClosestNodeWithEntryAndDirection', 'getItemById', 'getItems', 'getNearestGameObject', 'getNearestNodeFromList',
    'getNearestNodeId', 'getNearestNodeName', 'getNearestNodeNameObj', 'getNearestObjectByPredicate', 'getOffer', 'getRandomName', 'giveArmor',
    'immersiveSpawns', 'init', 'interact', 'interactNoop', 'loadAppearanceAndEquipment', 'loadNames', 'lvlAvg', 'meleeAdventurer', 'newAIPlayer',
    'newAIPlayerFile', 'overlay', 'playerScript', 'produce', 'pvmBots', 'pvmBotsFile', 'pvmCreate', 'pvmCreatePc', 'randomAfkPlayer', 'randomWalk',
    'randomWalkAroundPoint', 'randomWalkTo', 'randomizeLocationInRanges', 'rangeAdventurer', 'reset', 'runLaunched', 'runPulse', 'saleIsBigNews',
    'scriptInit', 'scriptSetLevel', 'scriptToString', 'sellAllOnGe', 'sellAllOnGeAdv', 'sellOnGE', 'sendBotInfo', 'sendChat', 'session', 'setActive',
    'setControler', 'setHighPriority', 'setLevels', 'setLocation', 'setOffer', 'setPulseDelay', 'setScript', 'setTriggered', 'skillingProduce',
    'stopPulse', 'takeNearestGroundItem', 'teleport', 'teleportToGE', 'tick', 'transitionNew', 'update', 'updateRandomOSRScopyLine', 'updateRandomValues',
    'useWith', 'walkArray', 'walkPos', 'walkTo', 'walkToPosSmart', 'walkToPosSmartXY', 'withdraw'
]


def cases():
    return CASES


def groups(cases):
    out = {}
    for case in cases:
        out[case['group']] = out.get(case['group'], 0) + 1
    return dict(sorted(out.items()))
