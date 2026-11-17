"""Run the unchanged repository and disconnection queue classes on JDK 11 and record what they do.

Compiled unchanged: NodeList.java, NodeListIterator.java, Repository.kt,
DisconnectionQueue.kt, TaskExecutor.kt, TickUtils.kt, LogoutListener.kt and
ContentInterface.kt. Everything else is an adapter from adapters.py, with the
original text of the members named in SCOPE inserted at generation time.
Launcher.java runs each case in its own class loader. --check never updates a
fixture, and nothing here downloads: the pinned jars must already be under
runs/.
"""
import argparse
import hashlib
import importlib.util
import itertools
import json
import os
import pathlib
import random
import re
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/repository-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/repository'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('repository_adapters', HERE / 'adapters.py')

ORIGINAL = [
    'core/game/world/repository/NodeList.java',
    'core/game/world/repository/NodeListIterator.java',
    'core/game/world/repository/Repository.kt',
    'core/game/world/repository/DisconnectionQueue.kt',
    'core/game/system/task/TaskExecutor.kt',
    'core/tools/TickUtils.kt',
    'core/api/LogoutListener.kt',
    'core/api/ContentInterface.kt',
]
SCOPE = (
    'Unchanged NodeList, NodeListIterator, Repository, DisconnectionQueue, TaskExecutor, TickUtils and LogoutListener on JDK 11, '
    'each case in a fresh class loader. Original text inserted at generation time: Player clear, finishClear, sendLogoutEvents, '
    'allowRemoval, getDetails, getName, getSession, setPlaying, isPlaying, getPacketDispatch, isArtificial, getCommunication and '
    'eight field declarations; PacketDispatch.sendLogout; IoSession disconnect, getPlayer, isActive, setObject and three field '
    'declarations; Node getName, getIndex, setIndex, isRenderable, setRenderable; NPC getId and its field; the Log and QueueStrength constants; the '
    'three ServerConstants values. Recording adapters: every other Player, Entity, PlayerDetails and IoSession member, combat, '
    'skills, death, teleport and script queue state, the logout packet, familiar, interfaces, Entity.clear, zones, house, '
    'UpdateSequence.renderablePlayers, the two management messages (with the generated builders\' refusal of null strings), '
    'Player.checkForWealthUpdate, the clan, PlayerDetails.save, PlayerParser.saveImmediately, the log, sendMessage, the region '
    'chunk and Location equality, the selection key and socket. kotlinx.coroutines is a stand-in that keeps a launched block '
    'until the case runs it on the calling thread. The tick counter and its driver (update, then increment) are case inputs. '
    'The map cases drive java.util.HashMap itself. Not exercised: AIPlayer.clear, a null queue entry, concurrent callers, '
    'the save itself, real sockets, packets and coroutine dispatch.'
)

INT_MIN, INT_MAX = -(1 << 31), (1 << 31) - 1
FINISH_CLEAR = ['worldLogout', 'walkReset', 'playerLogout', 'hasFamiliar', 'familiarClear', 'interfaceClose', 'closeSingleTab', 'entityClear',
                'zoneClear', 'houseLeave', 'renderableRemove', 'publishStatus', 'clan', 'worldId', 'clanOwner', 'publishClanLeave', 'wealthUpdate']
ALLOW = ['inCombat', 'lifepoints', 'isDead', 'teleporting', 'scriptQueue']


def java_hash(text):
    value = 0
    data = text.encode('utf-16-be')
    for i in range(0, len(data), 2):
        value = (value * 31 + (data[i] << 8 | data[i + 1])) & 0xffffffff
    return value


def spread(text):
    value = 0 if text is None else java_hash(text)
    return value ^ (value >> 16)


def bucket_family(mask, bucket, count, prefix):
    """Distinct keys whose spread hash has the given low bits and that differ above them."""
    found, i = [], 0
    while len(found) < count:
        key = f'{prefix}{i}'
        if spread(key) & mask == bucket:
            found.append(key)
        i += 1
    return found


SAME_HASH = [''.join(parts) for parts in itertools.product(['an', 'c0'], repeat=4)]
assert len({java_hash(name) for name in SAME_HASH}) == 1
SAME_BUCKET = bucket_family(63, 5, 40, 'k')
ZERO_BUCKET = bucket_family(63, 0, 14, 'z')
SAME_HASH_LONG = [''.join(parts) for parts in itertools.product(['an', 'c0'], repeat=6)]


def a(op, **kw):
    return dict(op=op, **kw)


def P(name, uid, **kw):
    return dict(name=name, uid=uid, **kw)


def requests():
    cases = []

    def add(actions, **kw):
        cases.append(dict(actions=actions, **kw))

    def cast(count=6, **over):
        """Players a..: lower-case names, uids from 1, each with its own inactive session."""
        names = ['alice', 'bob', 'carol', 'dave', 'erin', 'frank', 'grace', 'heidi']
        players, sessions = {}, {}
        for i in range(count):
            label = 'abcdefgh'[i]
            players[label] = P(names[i], i + 1, session='s' + label)
            sessions['s' + label] = dict(active=False, object=label, username=names[i])
        for label, changes in over.items():
            players[label].update(changes)
        return players, sessions

    def world(count=6, **kw):
        players, sessions = cast(count)
        for label, changes in kw.pop('players', {}).items():
            players.setdefault(label, {}).update(changes)
        for label, changes in kw.pop('sessions', {}).items():
            sessions.setdefault(label, {}).update(changes)
        base = dict(maxPlayers=5, maxNpcs=4, players=players, sessions=sessions)
        base.update(kw)
        return base

    # --- NodeList: allocation, reuse, bounds, the null quirks, bulk calls, the snapshot iterator.
    add([a('nlAdd', node='a'), a('nlAdd', node='b'), a('nlAdd', node='c'), a('nlAdd', node='d'), a('nlSize'), a('nlLength'), a('nlIsEmpty'),
         a('nlGet', index=0), a('nlGet', index=1), a('nlGet', index=3), a('nlGet', index=4), a('nlGet', index=-1), a('nlGet', index=INT_MAX),
         a('nlGet', index=INT_MIN), a('nlIndexOf', node='d'), a('nlRemoveAt', index=2), a('nlIndexOf', node='b'), a('nlAdd', node='d'),
         a('nlRemoveAt', index=0), a('nlRemoveAt', index=4), a('nlRemoveAt', index=-1), a('nlRemoveAt', index=2), a('nlRemoveAt', index=2),
         a('nlRemove', node='a'), a('nlRemove', node='a'), a('nlContains', node='a'), a('nlContains', node='c'), a('nlToArray'),
         a('nlAdd', node='e'), a('nlAdd', node='f'), a('nlAdd', node='a'), a('nlToArray'), a('nlClear'), a('nlIsEmpty'), a('nlToArray'),
         a('nlIndexOf', node='e'), a('nlAdd', node='f')], **world(maxPlayers=3))
    add([a('nlContains', node=None), a('nlRemove', node=None), a('nlSize'), a('nlIsEmpty'), a('nlToArray'), a('itNew', it='i', list='players'),
         a('addPlayer', node='a'), a('nlSize'), a('nlIsEmpty'), a('nlToArray'), a('findNpcId', id=1), a('nlAdd', node='b'), a('nlToArray'),
         a('nlAdd', node='c'), a('nlContains', node=None), a('nlRemove', node=None), a('nlSize'), a('nlToArray'), a('nlRemoveAt', index=1),
         a('nlRemove', node=None), a('nlRemove', node=None), a('nlSize'), a('nlIsEmpty'), a('addPlayer', node='d'), a('addPlayer', node='e'),
         a('nlClear'), a('nlSize'), a('nlRemove', node=None), a('nlRemove', node=None), a('addPlayer', node='f'), a('nlSize')],
        **world(maxPlayers=3))
    add([a('nlAdd', node='a'), a('nlAdd', node='a'), a('nlIndexOf', node='a'), a('nlSize'), a('nlRemove', node='a'), a('nlContains', node='a'),
         a('nlGet', index=1), a('nlGet', index=2), a('nlRemove', node='a'), a('nlContains', node='a'), a('nlSize'), a('nlIndexOf', node='a')],
        **world(maxPlayers=3))
    add([a('nlAddAll', nodes=['a', 'b', 'c', 'd']), a('nlAddAll', nodes=['e']), a('nlAddAll', nodes=[]), a('nlContainsAll', nodes=['a', 'b']),
         a('nlContainsAll', nodes=['a', 'e']), a('nlContainsAll', nodes=[]), a('nlRemoveAll', nodes=['a', 'e']), a('nlRemoveAll', nodes=['e']),
         a('nlRemoveAll', nodes=[]), a('nlAddAll', nodes=['e', 'e', 'f']), a('nlRetainAll', nodes=['b', 'f']), a('nlRetainAll', nodes=['b', 'e']),
         a('nlRetainAll', nodes=['b']), a('nlRetainAll', nodes=[]), a('nlRetainAll', nodes=[]), a('nlSize'), a('nlAddAll', nodes=['c', 'c', 'c', 'c'])],
        **world(maxPlayers=3))
    add([a('nlAdd', node='a'), a('nlAdd', node='b'), a('nlAdd', node='c'), a('itNew', it='i', list='players'), a('nlRemove', node='b'),
         a('nlAdd', node='d'), a('itHasNext', it='i'), a('itNext', it='i'), a('itNext', it='i'), a('itRemove', it='i'), a('itNext', it='i'),
         a('itRemove', it='i'), a('itRemove', it='i'), a('itHasNext', it='i'), a('itNext', it='i'), a('itNext', it='i'), a('itRemove', it='i'),
         a('itNew', it='j', list='players'), a('itRemove', it='j'), a('itHasNext', it='j'), a('itHasNext', it='j'), a('itNext', it='j'),
         a('nlClear'), a('itNext', it='j'), a('itRemove', it='j'), a('itHasNext', it='j'), a('itNew', it='k', list='players'),
         a('itHasNext', it='k'), a('itNext', it='k'), a('itRemove', it='k')], **world(maxPlayers=3))
    for step in range(4):
        # Removal while an iterator made before it is walked: the iterator is a copy.
        walk = [a('itNext', it='i')] * step + [a('nlRemoveAt', index=2), a('nlRemove', node='d'), a('nlAdd', node='e')]
        add([a('nlAddAll', nodes=['a', 'b', 'c', 'd']), a('itNew', it='i', list='players'), *walk, a('itNext', it='i'), a('itRemove', it='i'),
             a('itNext', it='i'), a('itRemove', it='i'), a('itNext', it='i'), a('itNext', it='i'), a('itNext', it='i'), a('nlToArray')],
            **world(maxPlayers=4))
    npcs = {f'n{i}': dict(id=i, location=[3200 + i, 3200, 0]) for i in range(1, 7)}
    add([a('nlLength', list='npcs'), a('nlAdd', list='npcs', node='n1'), a('nlGet', list='npcs', index=0), a('nlGet', list='npcs', index=1),
         a('nlRemoveAt', list='npcs', index=0), a('nlRemoveAt', list='npcs', index=1), a('nlContains', list='npcs', node=None),
         a('nlRemove', list='npcs', node=None), a('nlToArray', list='npcs'), a('nlClear', list='npcs'), a('nlIsEmpty', list='npcs'),
         a('nlSize', list='npcs'), a('itNew', it='i', list='npcs'), a('itHasNext', it='i'), a('itNext', it='i'), a('findNpcId', id=1),
         a('nlAddAll', list='npcs', nodes=['n1', 'n2']), a('nlRetainAll', list='npcs', nodes=[])], **world(maxNpcs=-1, npcs=npcs))
    add([a('nlAdd', node='a'), a('nlLength'), a('nlGet', index=1), a('nlRemoveAt', index=0), a('nlRemoveAt', index=1), a('addPlayer', node='a'),
         a('byName', name='alice'), a('byUid', uid=1), a('nlIndexOf', node='a'), a('removePlayer', node='a'), a('byName', name='alice'),
         a('nlContains', node=None), a('nlRemove', node=None), a('nlSize')], **world(maxPlayers=0, maxNpcs=0))
    for capacity in [1, 2]:
        add([a('addPlayer', node='a'), a('addPlayer', node='b'), a('addPlayer', node='c'), a('removePlayer', node='a'), a('addPlayer', node='c'),
             a('removePlayer', node='b'), a('addPlayer', node='a'), a('nlToArray')], **world(maxPlayers=capacity))

    # --- The pinned capacities: 2000 players, 32000 NPCs.
    crowd = {f'p{i:04d}': P(f'player{i:04d}', i) for i in range(1, 2004)}
    crowd['p0500']['session'] = 'gone'
    crowd['p2002']['uid'] = 1999
    crowd['p2003']['uid'] = 2000
    first = [f'p{i:04d}' for i in range(1, 2001)]
    add([a('nlLength'), a('nlLength', list='npcs'), a('nlAddAll', nodes=first, quiet=True), a('nlSize', quiet=True), a('nlAdd', node='p2001', quiet=True),
         a('nlGet', index=2000, quiet=True), a('nlGet', index=2001, quiet=True), a('nlRemoveAt', index=1000, quiet=True),
         a('nlAdd', node='p2001', quiet=True), a('nlIndexOf', node='p2001'), a('nlRemoveAt', index=2001, quiet=True),
         a('nlAdd', list='npcs', node='n1', quiet=True), a('nlGet', list='npcs', index=32000, quiet=True),
         a('nlGet', list='npcs', index=32001, quiet=True), a('nlRemoveAt', list='npcs', index=32000, quiet=True),
         a('nlRemoveAt', list='npcs', index=32001, quiet=True)], players=crowd, npcs=dict(n1=dict(id=1)))
    add([*[a('addPlayer', node=label, quiet=True) for label in first], a('nlSize'), a('addPlayer', node='p2001', quiet=True),
         a('nlIndexOf', node='p2001', quiet=True), a('byName', name='PLAYER2001', quiet=True), a('byUid', uid=2001, quiet=True),
         # Slot 2000 is outside `1 until size`: its uid is never compared.
         a('addPlayer', node='p2003', quiet=True), a('byUid', uid=2000, quiet=True), a('nlGet', index=2000, quiet=True),
         a('addPlayer', node='p2002', quiet=True), a('byUid', uid=1999, quiet=True), a('nlGet', index=1999, quiet=True),
         a('qContains', name='player1999', quiet=True), a('removePlayer', node='p0500', quiet=True), a('addPlayer', node='p2001', quiet=True),
         a('nlIndexOf', node='p2001', quiet=True), a('nlSize')],
        players=crowd, sessions=dict(gone=dict(active=False, object='p0500', username='player0500')))

    # --- Repository: names, uids, which map wins.
    add([a('addPlayer', node='a'), a('addPlayer', node='b'), a('byName', name='alice'), a('byName', name='ALICE'), a('byName', name='Alice'),
         a('byName', name='big bob'), a('byName', name='Big Bob'), a('byName', name='big_bob'), a('byName', name='BIG_BOB'), a('byName', name='big  bob'),
         a('byName', name=' big bob'), a('byName', name='big\tbob'), a('byName', name=None), a('byName', name=''), a('byName', name='carol'),
         a('byUid', uid=1), a('byUid', uid=2), a('byUid', uid=3), a('byUid', uid=0), a('byUid', uid=-1), a('addPlayer', node='c'),
         a('byName', name='Mixed'), a('byName', name='mixed'), a('addPlayer', node='d'), a('byName', name='two words'), a('byName', name='two_words'),
         a('removePlayer', node='a'), a('byName', name='alice'), a('byUid', uid=1), a('nlIndexOf', node='a'), a('removePlayer', node='a'),
         a('removePlayer', node='e')], **world(players=dict(b=dict(name='big_bob'), c=dict(name='Mixed'), d=dict(name='two words'))))
    # The duplicate uid check only looks at slots 1 .. size - 1.
    for order, extra in [(['a'], 'b'), (['a', 'c'], 'b'), (['c', 'a'], 'b'), (['c', 'd', 'a'], 'b'), (['a', 'c', 'd'], 'b'),
                         (['c', 'a', 'd', 'e'], 'b'), (['a', 'b'], 'c'), (['a', 'c', 'd', 'e', 'f'], 'b'), (['c', 'd', 'e', 'f', 'a'], 'b')]:
        add([*[a('addPlayer', node=label) for label in order], a('addPlayer', node=extra), a('byUid', uid=1), a('qContains', name='alice'),
             a('byName', name='alice'), a('byName', name='bob')], **world(players=dict(b=dict(uid=1))))
    # Holes move the range away from the occupied slots.
    add([a('addPlayer', node='c'), a('addPlayer', node='a'), a('addPlayer', node='d'), a('removePlayer', node='c'), a('addPlayer', node='b'),
         a('byUid', uid=1), a('nlToArray')], **world(players=dict(b=dict(uid=1))))
    add([a('addPlayer', node='a'), a('addPlayer', node='c'), a('addPlayer', node='d'), a('removePlayer', node='c'), a('addPlayer', node='b'),
         a('byUid', uid=1), a('nlToArray'), a('qGet', name='alice')], **world(players=dict(b=dict(uid=1))))
    add([a('addPlayer', node='c'), a('addPlayer', node='d'), a('addPlayer', node='a'), a('addPlayer', node='e'), a('removePlayer', node='c'),
         a('removePlayer', node='d'), a('addPlayer', node='b'), a('byUid', uid=1), a('nlToArray')], **world(players=dict(b=dict(uid=1))))
    # The same player added again, several holders of one uid, an artificial old player, a failing clear.
    add([a('addPlayer', node='a'), a('addPlayer', node='c'), a('addPlayer', node='a'), a('nlToArray'), a('qGet', name='alice'), a('addPlayer', node='a'),
         a('nlToArray')], **world())
    add([a('addPlayer', node='a'), a('addPlayer', node='b'), a('addPlayer', node='c'), a('addPlayer', node='d'), a('nlToArray'),
         a('removePlayer', node='b'), a('byUid', uid=1), a('removePlayer', node='a'), a('nlToArray')],
        **world(players=dict(b=dict(uid=1), c=dict(uid=1), d=dict(uid=1))))
    add([a('addPlayer', node='a'), a('addPlayer', node='c'), a('addPlayer', node='b'), a('nlToArray')],
        **world(players=dict(a=dict(artificial=True, clan='keldagrim', familiar=True, listeners=['x']), b=dict(uid=1)), worldListeners=['w']))
    for kind in ['exception', 'error']:
        add([a('addPlayer', node='a'), a('addPlayer', node='c'), a('addPlayer', node='b'), a('nlToArray'), a('byUid', uid=1)],
            **world(players=dict(b=dict(uid=1)), failures=dict(detailsSave={'1': kind})))
        add([a('addPlayer', node='a'), a('addPlayer', node='c'), a('addPlayer', node='b'), a('nlToArray'), a('byUid', uid=1)],
            **world(players=dict(a=dict(artificial=True), b=dict(uid=1)), failures=dict(entityClear={'1': kind})))
    # One name, two players: the name map keeps the last, and removing either removes the entry.
    add([a('addPlayer', node='a'), a('addPlayer', node='b'), a('byName', name='alice'), a('removePlayer', node='a'), a('byName', name='alice'),
         a('nlToArray'), a('addPlayer', node='a'), a('byName', name='alice'), a('removePlayer', node='b'), a('byName', name='alice'), a('byUid', uid=1),
         a('byUid', uid=2)], **world(players=dict(b=dict(name='alice'))))
    add([a('addPlayer', node='a'), a('addPlayer', node='b'), a('byName', name=None), a('qAdd', node='a'), a('qContains', name=None),
         a('qGet', name=None), a('qAdd', node='b'), a('removePlayer', node='a'), a('qRemove', name=None), a('qContains', name=None),
         a('tick', count=1), a('clear', node='b')], **world(players=dict(a=dict(name=None, username='Nameless'), b=dict(name=None, username=None))))
    # Late reads: the uid and name are read again when the player leaves.
    add([a('addPlayer', node='a'), a('set', node='a', field='uid', value=9), a('set', node='a', field='name', value='renamed'),
         a('byUid', uid=1), a('byName', name='alice'), a('removePlayer', node='a'), a('byUid', uid=1), a('byName', name='alice'),
         a('addPlayer', node='a'), a('byUid', uid=9), a('byName', name='renamed'), a('removePlayer', node='a')], **world())

    # --- removePlayer and the session.
    for session in [dict(active=True), dict(active=True, key=True), dict(active=False, key=True), dict(active=True, object=None),
                    dict(active=True, object=None, username='bob'), dict(active=True, object=None, username='Big Bob'), dict(active=True, object='other'),
                    dict(active=True, object='b'), dict(active=True, object=None, username=None)]:
        add([a('addPlayer', node='a'), a('addPlayer', node='b'), a('removePlayer', node='a'), a('qGet', name='alice'), a('qGet', name='bob'),
             a('removePlayer', node='a'), a('sessionPlayer', session='sa'), a('disconnect', session='sa')],
            **world(sessions=dict(sa=session), players=dict(b=dict(name='big_bob') if session.get('username') == 'Big Bob' else {})))
    add([a('addPlayer', node='a'), a('set', node='a', field='session', value=None), a('removePlayer', node='a'), a('byName', name='alice'),
         a('nlToArray')], **world())
    for event, kinds in [('renderableRemove', ['exception', 'error']), ('keyCancel', ['exception', 'error']), ('socketClose', ['io', 'exception', 'error']),
                         ('detailsSave', ['exception', 'error']), ('log', ['exception', 'error'])]:
        for kind in kinds:
            add([a('addPlayer', node='a'), a('removePlayer', node='a'), a('qGet', name='alice'), a('disconnect', session='sa'),
                 a('setSession', session='sa', active=True), a('disconnect', session='sa')],
                **world(sessions=dict(sa=dict(active=True, key=True)), failures={event: {'1': kind}}))
    # A dirty disconnect (SessionDirtyDisconnectTests): the socket fails to close, the player is queued all the same.
    add([a('addPlayer', node='a'), a('disconnect', session='sa'), a('qContains', name='alice'), a('byName', name='alice'), a('disconnect', session='sa')],
        **world(sessions=dict(sa=dict(active=True, key=True)), failures=dict(socketClose={'1': 'io'})))
    for kind in ['exception', 'error']:
        add([a('addPlayer', node='a'), a('disconnect', session='sa'), a('qContains', name='alice')],
            **world(sessions=dict(sa=dict(active=True, key=True)), failures=dict(socketClose={'1': 'io'}, detailsSave={'1': kind})))

    # --- News.
    news = world(globalChat=True, players=dict(c=dict(geMuted=True), e=dict(geMuted=True)))
    join = [a('addPlayer', node=label) for label in 'abcde']
    add([*join, a('news1', text='hello'), a('newsDefault', text='hello'), a('news2', text='two', icon=3), a('news3', text='three', icon=0, color='ff0000'),
         a('news4', text='four', icon=-1, color='', ge=False), a('news4', text='ge', icon=7, color='00ff00', ge=True),
         a('geNewsDefault', text='market'), a('geNews3', text='market', icon=1, color='abc'), a('removePlayer', node='b'), a('news1', text='after')], **news)
    add([*join, a('news1', text='off'), a('geNewsDefault', text='off')], **world(players=dict(c=dict(geMuted=True))))
    add([a('news1', text='nobody'), a('addPlayer', node='a'), a('addPlayer', node='b'), a('news1', text='shared')],
        **world(globalChat=True, players=dict(b=dict(name='alice'))))
    for event in ['message', 'attribute']:
        for at in [1, 3]:
            for kind in ['exception', 'error']:
                add([*join, a('news4', text='x', icon=1, color='c', ge=True)], failures={event: {str(at): kind}}, **news)
    many = {f'q{i:02d}': P(name, 100 + i, session='sx') for i, name in enumerate(SAME_HASH + SAME_BUCKET[:12])}
    add([*[a('addPlayer', node=label, quiet=True) for label in sorted(many)], a('news1', text='order'),
         *[a('removePlayer', node=label, quiet=True) for label in sorted(many)[::3]], a('news1', text='order')],
        maxPlayers=40, maxNpcs=1, globalChat=True, players=many, sessions=dict(sx=dict(active=False)))

    # --- NPCs.
    npc_world = world(npcs=dict(n1=dict(id=5, location=[3200, 3200, 0]), n2=dict(id=5, location=[3200, 3200, 0]), n3=dict(id=7, location=[3201, 3200, 0]),
                                n4=dict(id=9, location=None), r1=dict(id=11, revenant=True, location=[3200, 3200, 1])),
                      chunk=['n4', 'n3', 'n2', 'n1', 'r1'], maxNpcs=4)
    add([a('addRenderable', node='n1'), a('addRenderable', node='n1'), a('addRenderable', node='r1'), a('addRenderable', node='n2'),
         a('renderableNpcs'), a('removeRenderable', node='n3'), a('removeRenderable', node='r1'), a('removeRenderable', node='n1'),
         a('removeRenderable', node='n1'), a('renderableNpcs'), a('addRenderable', node='n1'), a('renderableNpcs')], **npc_world)
    add([a('findNpcId', id=5), a('nlAdd', list='npcs', node='n3'), a('nlAdd', list='npcs', node='n2'), a('nlAdd', list='npcs', node='n1'),
         a('nlAdd', list='npcs', node='r1'), a('nlAdd', list='npcs', node='n4'), a('findNpcId', id=5), a('findNpcId', id=7), a('findNpcId', id=11),
         a('findNpcId', id=9), a('findNpcId', id=0), a('nlRemoveAt', list='npcs', index=2), a('findNpcId', id=5), a('set', node='n3', field='id', value=5),
         a('findNpcId', id=5), a('nlAdd', list='npcs', node='n4'), a('findNpcId', id=9), a('nlIndexOf', list='npcs', node='n4'),
         a('nlClear', list='npcs'), a('findNpcId', id=5)], **npc_world)
    add([a('findNpcAt', location=[3200, 3200, 0]), a('findNpcAt', location=[3201, 3200, 0]), a('findNpcAt', location=[3200, 3200, 1]),
         a('findNpcAt', location=[1, 2, 3]), a('set', node='n4', field='location', value=[1, 2, 3]), a('findNpcAt', location=[1, 2, 3]),
         a('set', node='n2', field='location', value=None), a('findNpcAt', location=[3200, 3200, 0])], **npc_world)
    for kind in ['exception', 'error']:
        add([a('findNpcAt', location=[3200, 3200, 0])], failures=dict(regionChunk={'1': kind}), **npc_world)
    add([a('lobbyAdd', node='a'), a('lobbyAdd', node='a'), a('lobbyAdd', node='b'), a('lobbyRemove', node='a'), a('lobbyRemove', node='c'),
         a('lobbyRemove', node='a'), a('lobbyRemove', node='a')], **world())

    # --- The queue's small calls.
    add([a('qIsEmpty'), a('qAdd', node='a'), a('qIsEmpty'), a('qAdd', node='a', clear=True), a('qAdd', node='b', clear=True), a('qAdd', node='b', clear=False),
         a('qGet', name='alice'), a('qGet', name='Alice'), a('qGet', name='nobody'), a('qGet', name=None), a('qContains', name='alice'),
         a('qContains', name='ALICE'), a('qContains', name=None), a('qRemove', name='nobody'), a('qRemove', name=None), a('qRemove', name='alice'),
         a('qRemove', name='alice'), a('qContains', name='alice'), a('qAdd', node='a', clear=True), a('qSave', node='a', sql=True),
         a('qSave', node='a', sql=False), a('qRemove', name='bob'), a('qRemove', name='alice'), a('qIsEmpty')], **world())
    for kind in ['exception', 'error']:
        add([a('qAdd', node='a'), a('qContains', name='alice'), a('qAdd', node='a'), a('qAdd', node='b')], failures=dict(log={'1': kind}), **world())
        add([a('qSave', node='a', sql=True), a('qSave', node='a', sql=False)], failures=dict(saveImmediately={'1': kind}), **world())
    add([a('sendLogout', node='a'), a('sendLogout', node='a'), a('qGet', name='alice'), a('clear', node='a'), a('qGet', name='alice'),
         a('sendLogout', node='a'), a('clear', node='b'), a('clear', node='c'), a('qGet', name='carol')],
        **world(players=dict(c=dict(artificial=True))))
    for event in ['logoutPacket', 'log', 'detailsSave']:
        for kind in ['exception', 'error']:
            add([a('sendLogout', node='a'), a('qContains', name='alice'), a('clear', node='b'), a('qContains', name='bob')],
                failures={event: {'1': kind}}, **world())

    # --- allowRemoval: every condition both ways, alone and behind the others.
    flags = ['combat', 'dead', 'teleporting', 'soft']
    for bits in range(16):
        for lifepoints in [10, 1, 0, -5]:
            if lifepoints in [1, -5] and bits not in [0, 15]:
                continue
            changes = {flag: bool(bits >> i & 1) for i, flag in enumerate(flags)}
            add([a('allowRemoval', node='a'), a('qAdd', node='a'), a('tick', count=1), a('qContains', name='alice')],
                **world(players=dict(a=dict(lifepoints=lifepoints, **changes))))
    for event in ALLOW:
        for kind in ['exception', 'error']:
            add([a('allowRemoval', node='a'), a('qAdd', node='a'), a('qAdd', node='b'), a('tick', count=1), a('tick', count=3), a('qContains', name='bob')],
                failures={event: {'2': kind}}, **world())

    # --- The every-third-tick gate, over long runs and across the wrap of the counter.
    for start in [0, 1, 2, 3, 4, 5, -1, -2, -3, -4, INT_MAX - 2, INT_MAX - 1, INT_MAX, INT_MIN, INT_MIN + 1, INT_MIN + 2, 299, 300, 301]:
        add([a('tick', count=3), a('qAdd', node='a'), *[a('tick', count=1)] * 7, a('set', node='a', field='combat', value=False), *[a('tick', count=1)] * 4,
             a('runTasks')], ticks=start, **world(players=dict(a=dict(combat=True))))
    add([a('qAdd', node='a'), a('tick', count=40), a('qAdd', node='b'), a('tick', count=40), a('set', node='a', field='combat', value=False),
         a('tick', count=20), a('runTasks')], ticks=INT_MAX - 30, **world(players=dict(a=dict(combat=True), b=dict(combat=True))))
    add([a('tick', count=5), a('qUpdate'), a('qUpdate'), a('setTicks', value=7), a('qUpdate'), a('qAdd', node='a'), a('qUpdate'), a('setTicks', value=9),
         a('qUpdate'), a('qUpdate'), a('qRemove', name='alice'), a('qUpdate')], **world(players=dict(a=dict(combat=True))))

    # --- The forced clear: 30 seconds for an AFK logout, 5 minutes otherwise, counted three ticks at a time.
    # A gate adds three, so the 17th gate (tick 48) reaches 51 >= 50 and the 167th (tick 498) reaches 501 >= 500.
    for afk, ticks in [(True, 48), (False, 498), (None, 498)]:
        for change in [-3, 0, 3]:
            add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=ticks + change), a('qContains', name='alice'), a('tick', count=1),
                 a('qContains', name='alice'), a('tick', count=3), a('qContains', name='alice'), a('byName', name='alice'), a('tick', count=6), a('runTasks')],
                **world(players=dict(a=dict(combat=True, afk=afk, clan='varrock', familiar=True, listeners=['one'])), worldListeners=['w1'],
                        sessions=dict(sa=dict(active=True, key=True))))
    for start in [1, 2, INT_MAX - 20]:
        add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=60), a('qContains', name='alice'), a('byName', name='alice')],
            ticks=start, **world(players=dict(a=dict(combat=True, afk=True))))
    # The flag is read at every gate, so it can shorten or lengthen the wait while the player is queued.
    add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=90), a('set', node='a', field='afk', value=True), a('tick', count=1),
         a('qContains', name='alice'), a('tick', count=3), a('qContains', name='alice')], **world(players=dict(a=dict(combat=True))))
    add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=45), a('set', node='a', field='afk', value=False), a('tick', count=30),
         a('qContains', name='alice'), a('set', node='a', field='afk', value=True), a('tick', count=3), a('qContains', name='alice')],
        **world(players=dict(a=dict(combat=True, afk=True))))
    # The counter outlives a finished entry (only remove(name) drops it), so the next wait starts where the last stopped.
    add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=30), a('set', node='a', field='combat', value=False), a('tick', count=3),
         a('qContains', name='alice'), a('set', node='a', field='combat', value=True), a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=21),
         a('qContains', name='alice'), a('tick', count=3), a('qContains', name='alice')], **world(players=dict(a=dict(combat=True, afk=True))))
    add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=30), a('qRemove', name='alice'), a('qAdd', node='a'), a('tick', count=45),
         a('qContains', name='alice'), a('tick', count=9), a('qContains', name='alice')], **world(players=dict(a=dict(combat=True, afk=True))))
    add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=30), a('clear', node='a'), a('tick', count=45), a('qContains', name='alice'),
         a('tick', count=9), a('qContains', name='alice')], **world(players=dict(a=dict(combat=True, afk=True))))
    add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=30), a('qClear'), a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=21),
         a('qContains', name='alice'), a('tick', count=3), a('qContains', name='alice')], **world(players=dict(a=dict(combat=True, afk=True))))
    forced = world(players=dict(a=dict(combat=True, afk=True, clan='varrock', familiar=True, listeners=['one']), b=dict(combat=True, afk=True),
                                c=dict(combat=True, afk=True)), worldListeners=['w1'], sessions=dict(sa=dict(active=True, key=True)))
    forced_run = [a('addPlayer', node='a'), a('addPlayer', node='b'), a('addPlayer', node='c'), a('qAdd', node='a'), a('qAdd', node='b'),
                  a('qAdd', node='c'), a('tick', count=48), a('tick', count=1), a('qContains', name='alice'), a('qContains', name='bob'),
                  a('tick', count=3), a('nlToArray'), a('tick', count=3), a('nlToArray')]
    add(forced_run, **forced)
    for event in FINISH_CLEAR + ['keyCancel', 'socketClose', 'detailsSave', 'log']:
        for at in ([1, 2] if event in ['renderableRemove', 'clan', 'log'] else [1]):
            for kind in ['exception', 'error'] + (['io'] if event == 'socketClose' else []):
                add(forced_run, failures={event: {str(at): kind}}, **forced)
    for pair in [dict(walkReset={'1': 'exception'}, renderableRemove={'1': 'error'}), dict(houseLeave={'1': 'error'}, keyCancel={'1': 'exception'}),
                 dict(entityClear={'1': 'exception'}, socketClose={'1': 'io'}, detailsSave={'1': 'exception'})]:
        add(forced_run, failures=pair, **forced)

    # --- finish: the order of everything a normal removal does, and each call failing.
    full = world(players=dict(a=dict(clan='varrock', familiar=True, listeners=['one', 'two'], afk=True), b=dict(combat=True)),
                 worldListeners=['w1', 'w2'], sessions=dict(sa=dict(active=True, key=True)))
    finish_run = [a('addPlayer', node='a'), a('addPlayer', node='b'), a('qAdd', node='b'), a('qAdd', node='a'), a('tick', count=4),
                  a('qContains', name='alice'), a('byName', name='alice'), a('byUid', uid=1), a('nlContains', node='a'), a('runTasks'), a('tick', count=3),
                  a('runTasks'), a('nlToArray')]
    add(finish_run, **full)
    add(finish_run, worldId=None, **full)
    add(finish_run, worldId=-7, **full)
    for players in [dict(a=dict(artificial=True)), dict(a=dict(artificial=True, clan='c', familiar=True, listeners=['k'])), dict(a=dict(clan=None)),
                    dict(a=dict(listeners=SAME_HASH[:11])), dict(a=dict(username='Shown Name')), dict(a=dict(username=None)),
                    dict(a=dict(playing=False, renderable=False))]:
        add(finish_run, **world(players=players, worldListeners=['w1']))
    for session in [dict(active=False), dict(active=True, key=False), dict(active=True, object=None), dict(active=True, object='b')]:
        add(finish_run, **world(sessions=dict(sa=session), players=dict(b=dict(combat=True))))
    for event in ALLOW + FINISH_CLEAR + ['logoutPacket', 'keyCancel', 'socketClose', 'detailsSave', 'clanLeave', 'launch', 'saveImmediately', 'log']:
        occurrences = {'renderableRemove': [1, 2], 'clan': [1, 2, 3, 4], 'log': [1, 2, 3, 4, 5], 'worldLogout': [1, 2], 'playerLogout': [1, 2]}.get(event, [1])
        for at in occurrences:
            for kind in ['exception', 'error'] + (['io'] if event == 'socketClose' else []):
                add(finish_run, failures={event: {str(at): kind}}, **full)
    # SessionClearingTests: an AFK logout whose client ignores the packet, and a logout listener that throws.
    add([a('addPlayer', node='a'), a('sendLogout', node='a'), a('qContains', name='alice'), a('byName', name='alice'), a('tick', count=4),
         a('qContains', name='alice'), a('nlContains', node='a'), a('byName', name='alice'), a('byUid', uid=1)],
        ticks=1, **world(players=dict(a=dict(afk=True)), sessions=dict(sa=dict(active=True))))
    add([a('addPlayer', node='a'), a('qAdd', node='a'), a('tick', count=4), a('qContains', name='alice'), a('nlContains', node='a'),
         a('byName', name='alice'), a('byUid', uid=1)],
        ticks=2, failures=dict(playerLogout={'1': 'exception'}), **world(players=dict(a=dict(listeners=['test-failure'])), sessions=dict(sa=dict(active=True))))

    # --- clear(): every entry finished by force, saved on the calling thread, timers kept.
    clear_run = [a('addPlayer', node='a'), a('addPlayer', node='b'), a('addPlayer', node='c'), a('qAdd', node='a'), a('qAdd', node='b'), a('qAdd', node='c'),
                 a('tick', count=7), a('qClear'), a('qIsEmpty'), a('nlToArray'), a('runTasks'), a('qClear'), a('qAdd', node='b'), a('tick', count=3)]
    clearing = world(players=dict(a=dict(combat=True, clan='varrock'), b=dict(combat=True, artificial=True), c=dict(dead=True)),
                     sessions=dict(sa=dict(active=True, key=True), sc=dict(active=True)))
    add(clear_run, **clearing)
    for event in ['logoutPacket', 'walkReset', 'wealthUpdate', 'renderableRemove', 'clan', 'clanLeave', 'saveImmediately', 'log', 'detailsSave', 'socketClose']:
        for at in ([1, 2] if event in ['renderableRemove', 'clan'] else [1, 4] if event == 'log' else [1]):
            for kind in ['exception', 'error']:
                add(clear_run, failures={event: {str(at): kind}}, **clearing)

    # --- Calls that come back into the queue or the repository while they are being walked.
    hooks = [[a('qRemove', name='alice')], [a('qRemove', name='bob')], [a('qAdd', node='d')], [a('removePlayer', node='b')], [a('addPlayer', node='d')],
             [a('set', node='a', field='name', value='renamed')], [a('set', node='a', field='uid', value=99)],
             [a('set', node='a', field='username', value='Late')], [a('clear', node='a')], [a('clear', node='b')], [a('qClear')], [a('qUpdate')],
             [a('set', node='b', field='combat', value=False)], [a('set', node='a', field='artificial', value=True)], [a('setTicks', value=1)]]
    hooked = world(players=dict(a=dict(clan='varrock'), b=dict(combat=True, afk=True), d=dict(uid=1)), sessions=dict(sa=dict(active=True)))
    for event in ['inCombat', 'logoutPacket', 'walkReset', 'renderableRemove', 'detailsSave', 'clanLeave', 'launch', 'log']:
        for actions in hooks:
            add([a('addPlayer', node='a'), a('addPlayer', node='b'), a('qAdd', node='a'), a('qAdd', node='b'), a('tick', count=1), a('runTasks'),
                 a('tick', count=3), a('runTasks'), a('nlToArray'), a('byUid', uid=1), a('byUid', uid=99)],
                hooks=[dict(event=event, at=1, actions=actions)], **hooked)
    for event in ['walkReset', 'renderableRemove', 'log']:
        for actions in hooks[:5]:
            add([a('addPlayer', node='a'), a('addPlayer', node='b'), a('qAdd', node='a'), a('qAdd', node='b'), a('tick', count=49), a('tick', count=3),
                 a('nlToArray')], hooks=[dict(event=event, at=1, actions=actions)],
                **world(players=dict(a=dict(combat=True, afk=True), b=dict(combat=True, afk=True), d=dict(uid=1)), sessions=dict(sa=dict(active=True))))

    # --- Many entries: the queue is walked in HashMap order, also through a tree bin.
    rng = random.Random(530)
    for round_ in range(12):
        names = (SAME_HASH if round_ % 2 == 0 else SAME_HASH[:9] + SAME_BUCKET[:16])[:]
        rng.shuffle(names)
        count = [3, 9, 12, 16, 20, 25][round_ // 2]
        players = {f'q{i:02d}': P(name, 100 + i, session='sx', combat=rng.random() < 0.5, afk=rng.random() < 0.5) for i, name in enumerate(names[:count])}
        labels = sorted(players)
        actions = [a('addPlayer', node=label, quiet=True) for label in labels] + [a('qAdd', node=label, quiet=True) for label in labels]
        actions += [a('tick', count=1)]
        for label in rng.sample(labels, max(1, count // 3)):
            actions += [a('qRemove', name=players[label]['name'], quiet=True)]
        actions += [a('tick', count=3)]
        for label in rng.sample(labels, max(1, count // 2)):
            actions += [a('qAdd', node=label, quiet=True)]
        actions += [a('tick', count=3), a('news1', text='who is left'), a('tick', count=60), a('runTasks', quiet=True), a('qClear'), a('nlToArray')]
        add(actions, maxPlayers=30, maxNpcs=1, globalChat=True, players=players, sessions=dict(sx=dict(active=False)))

    # --- java.util.HashMap itself: put, remove and clear over lists, tree bins, resizes and splits.
    def steps(keys, count, removal, clear_at=None):
        out, value = [], 0
        for i in range(count):
            key = rng.choice(keys)
            roll = rng.random()
            if roll < removal:
                out.append(['remove', key])
            elif roll < removal + 0.05:
                out.append(['get', key])
            else:
                value += 1
                out.append(['put', key, value])
            if i == clear_at:
                out += [['dump'], ['clear'], ['dump']]
            if i % (9 if len(keys) <= 20 else 25 if len(keys) <= 70 else 60) == 8:
                out.append(['dump'])
        return out + [['dump']]

    plain = [f'user{i}' for i in range(200)]
    for keys, count, removal in [(plain[:10], 60, 0.4), (plain[:40], 200, 0.3), (plain, 600, 0.3), (plain, 900, 0.55), (SAME_HASH, 120, 0.4),
                                 (SAME_HASH, 300, 0.5), (SAME_HASH_LONG, 400, 0.3), (SAME_HASH_LONG, 900, 0.5), (SAME_HASH_LONG, 900, 0.65),
                                 (SAME_BUCKET, 300, 0.3), (SAME_BUCKET, 700, 0.5), (SAME_BUCKET + plain[:60], 700, 0.4),
                                 (SAME_HASH_LONG[:20] + SAME_BUCKET + plain[:30], 900, 0.45), (ZERO_BUCKET + ['', None], 300, 0.4),
                                 (ZERO_BUCKET + ['', None], 600, 0.55), (SAME_HASH + [None, ''], 200, 0.35)]:
        for capacity in [None, 1, 3, 64, 2000]:
            for repeat in range(2):
                if repeat and capacity is not None:
                    continue
                cases.append(dict(kind='map', capacity=capacity, steps=steps(keys, count, removal, clear_at=count // 2 if repeat else None)))
    # Growing past a tree bin: the bin splits, and each half is a list again at six or fewer.
    for size in [7, 9, 13, 14, 20, 40]:
        for fill in [0, 40, 100]:
            grow = [['put', key, i] for i, key in enumerate(SAME_BUCKET[:size])] + [['dump']]
            grow += [['put', key, i] for i, key in enumerate(plain[:fill])] + [['dump']]
            grow += [['remove', key] for key in SAME_BUCKET[:size:2]] + [['dump']]
            grow += [['put', key, i] for i, key in enumerate(plain[fill:fill + 150])] + [['dump']]
            cases.append(dict(kind='map', capacity=None, steps=grow))
    integers = [i * 2048 + 7 for i in range(-10, 30)] + list(range(-5, 60)) + [INT_MIN, INT_MAX, 65536, 65543, -65536]
    for capacity in [2000, None, 0, 1, 5]:
        for removal in [0.3, 0.55]:
            int_steps = steps(integers, 500, removal)
            cases.append(dict(kind='intMap', capacity=capacity, steps=int_steps))
    return cases


def line(text, pattern):
    match = re.search(pattern, text, re.M)
    if not match:
        raise RuntimeError(f'{pattern} not found')
    return match.group().strip()


def markers():
    def source(name):
        return c.read(SERVER / name)

    player = source('core/game/node/entity/player/Player.java')
    node = source('core/game/node/Node.java')
    session = source('core/net/IoSession.java')
    npc = source('core/game/node/entity/npc/NPC.java')
    constants = source('core/ServerConstants.kt')
    strength = re.search(r'enum class QueueStrength \{([^}]*)\}', source('core/game/interaction/Script.kt'))
    return {
        'ORIGINAL_MAX_PLAYERS': re.search(r'var MAX_PLAYERS = (\d+)', constants).group(1),
        'ORIGINAL_MAX_NPCS': re.search(r'var MAX_NPCS = (\d+)', constants).group(1),
        'ORIGINAL_ENABLE_GLOBAL_CHAT': re.search(r'var ENABLE_GLOBAL_CHAT = (true|false)', constants).group(1),
        'ORIGINAL_NPC_ID_FIELD': line(npc, r'^\s*private int id;'),
        'ORIGINAL_NPC_GET_ID': c.method(npc, 'public int getId()'),
        'ORIGINAL_NODE_GET_NAME': c.method(node, 'public String getName()'),
        'ORIGINAL_NODE_GET_INDEX': c.method(node, 'public int getIndex()'),
        'ORIGINAL_NODE_SET_INDEX': c.method(node, 'public void setIndex(int index)'),
        'ORIGINAL_NODE_IS_RENDERABLE': c.method(node, 'public boolean isRenderable()'),
        'ORIGINAL_NODE_SET_RENDERABLE': c.method(node, 'public void setRenderable(boolean renderable)'),
        'ORIGINAL_PLAYER_DETAILS_FIELD': line(player, r'^\s*private PlayerDetails details;'),
        'ORIGINAL_PLAYER_LOGOUT_LISTENERS_FIELD': line(player, r'^\s*public HashMap<String,Function1<Player, Unit>> logoutListeners = new HashMap<>\(\);'),
        'ORIGINAL_PLAYER_AFK_FIELD': line(player, r'^\s*public Boolean isAfkLogout;'),
        'ORIGINAL_PLAYER_PACKET_DISPATCH_FIELD': line(player, r'^\s*private final PacketDispatch packetDispatch = new PacketDispatch\(this\);'),
        'ORIGINAL_PLAYER_INTERFACE_MANAGER_FIELD': line(player, r'^\s*private final InterfaceManager interfaceManager = new InterfaceManager\(this\);'),
        'ORIGINAL_PLAYER_FAMILIAR_MANAGER_FIELD': line(player, r'^\s*private final FamiliarManager familiarManager = new FamiliarManager\(this\);'),
        'ORIGINAL_PLAYER_PLAYING_FIELD': line(player, r'^\s*private boolean playing;'),
        'ORIGINAL_PLAYER_ARTIFICIAL_FIELD': line(player, r'^\s*protected boolean artificial;'),
        'ORIGINAL_PLAYER_CLEAR': c.method(player, 'public void clear()'),
        'ORIGINAL_PLAYER_FINISH_CLEAR': c.method(player, 'public void finishClear()'),
        'ORIGINAL_PLAYER_SEND_LOGOUT_EVENTS': c.method(player, 'private void sendLogoutEvents()'),
        'ORIGINAL_PLAYER_ALLOW_REMOVAL': c.method(player, 'public boolean allowRemoval()'),
        'ORIGINAL_PLAYER_GET_DETAILS': c.method(player, 'public PlayerDetails getDetails()'),
        'ORIGINAL_PLAYER_GET_NAME': c.method(player, 'public String getName()'),
        'ORIGINAL_PLAYER_GET_SESSION': c.method(player, 'public IoSession getSession()'),
        'ORIGINAL_PLAYER_SET_PLAYING': c.method(player, 'public void setPlaying(boolean playing)'),
        'ORIGINAL_PLAYER_IS_PLAYING': c.method(player, 'public boolean isPlaying()'),
        'ORIGINAL_PLAYER_GET_PACKET_DISPATCH': c.method(player, 'public PacketDispatch getPacketDispatch()'),
        'ORIGINAL_PLAYER_IS_ARTIFICIAL': c.method(player, 'public boolean isArtificial()'),
        'ORIGINAL_PLAYER_GET_COMMUNICATION': c.method(player, 'public CommunicationInfo getCommunication()'),
        'ORIGINAL_DISPATCH_SEND_LOGOUT': c.method(source('core/game/node/entity/player/link/PacketDispatch.java'), 'public void sendLogout()'),
        'ORIGINAL_SESSION_KEY_FIELD': line(session, r'^\s*private final SelectionKey key;'),
        'ORIGINAL_SESSION_OBJECT_FIELD': line(session, r'^\s*private Object object;'),
        'ORIGINAL_SESSION_ACTIVE_FIELD': line(session, r'^\s*private boolean active = true;'),
        'ORIGINAL_SESSION_DISCONNECT': c.method(session, 'public void disconnect()'),
        'ORIGINAL_SESSION_IS_ACTIVE': c.method(session, 'public boolean isActive()'),
        'ORIGINAL_SESSION_GET_PLAYER': c.method(session, 'public Player getPlayer()'),
        'ORIGINAL_SESSION_SET_OBJECT': c.method(session, 'public void setObject(Object object)'),
        'ORIGINAL_LOG_ENUM': c.method(source('core/tools/SystemLogger.kt'), 'enum class Log {'),
        'ORIGINAL_QUEUE_STRENGTH': strength.group(1).strip(),
    }


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)

    lock_names = json.loads(c.read(ROOT / 'oracle/login/kotlin.lock.json'))
    for name, entry in lock_names.items():
        jar = ROOT / 'runs/kotlin-1.8.20' / name
        if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != entry['sha256']:
            raise RuntimeError(f'{jar} is missing or differs from its pin; this generator never downloads')
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    pin = json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))
    json_jar = RUN / 'json-simple-1.1.1.jar'
    if not json_jar.is_file() or hashlib.sha256(json_jar.read_bytes()).hexdigest() != pin['sha256']:
        raise RuntimeError(f'{json_jar} is missing or differs from its pin; this generator never downloads')
    libraries = [stdlib, json_jar]

    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(adapters.ADAPTERS.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'adapters.py')
    c.read(HERE / 'Support.java')
    c.read(HERE / 'Launcher.java')
    java.append(HERE / 'Support.java')

    classes, launcher = RUN / 'classes', RUN / 'launcher'
    for path in [classes, launcher]:
        if path.exists():
            shutil.rmtree(path)
        path.mkdir()
    c.run(['java', '-Xmx768m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, libraries)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-d', launcher, HERE / 'Launcher.java'])

    cases = requests()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', launcher, 'oracle.Launcher', classes, *libraries]
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=1200)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:] + result.stdout.decode(errors='replace')[-500:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')

    events, errors, operations, map_steps, map_cases = {}, {}, {}, 0, 0
    for row in rows:
        if 'kind' in row:
            map_cases += 1
            map_steps += len(row['steps'])
            continue
        for action, step in zip(row['actions'], row['expected']):
            operations[action['op']] = operations.get(action['op'], 0) + 1
            if step['error'] is not None:
                category = step['error'].split(':')[0]
                errors[category] = errors.get(category, 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(
        scenarios=len(rows) - map_cases, actions=sum(operations.values()), operations=dict(sorted(operations.items())),
        serviceCalls=sum(events.values()), events=dict(sorted(events.items())), errors=dict(sorted(errors.items())),
        mapCases=map_cases, mapSteps=map_steps, injectedFailures=sum(len(at) for row in rows for at in row.get('failures', {}).values()),
    )
    for needed in FINISH_CLEAR + ALLOW + ['logoutPacket', 'keyCancel', 'socketClose', 'detailsSave', 'clanLeave', 'launch', 'saveImmediately', 'log',
                                          'stackTrace', 'message', 'attribute', 'regionChunk']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    for needed in ['IndexOutOfBounds', 'ArrayIndexOutOfBounds', 'NegativeArraySize', 'NullPointer', 'NoSuchElement', 'IllegalState', 'exception', 'error']:
        if needed not in errors:
            raise RuntimeError(f'the corpus never fails with {needed}')
    forced = sum(1 for row in rows if 'kind' not in row for step in row['expected'] for event in step['events']
                 if event[0] == 'log' and event[3].startswith('Force-clearing'))
    coverage['forcedClears'] = forced
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', scope=SCOPE)
    summary = (f'{len(rows)} original repository and disconnection queue cases ({coverage["scenarios"]} scenarios with {coverage["actions"]} actions and '
               f'{coverage["serviceCalls"]} recorded calls, {map_cases} HashMap cases with {map_steps} steps)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Repository fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary} under {FIXTURES}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
