"""The designed corpus of the social oracle.

Each case gives a world, a cast of players (their saved communication fields,
rights, mute end, chat settings, session kind and attributes), the players
online in the repository and their order, account-storage answers, the clock,
the seeds handed to unseeded java.util.Random constructions, failures to
inject into recorded calls, and a list of actions. Packet actions carry the
client's bytes; `chat` and `pm` actions are encoded by the original
StringUtils.encryptPlayerChat inside the harness, and the bytes are recorded.
"""

REQUIRED_EVENTS = [
    'clock', 'random', 'getAttribute', 'setAttribute', 'playerMessage', 'dispatchMessage', 'apiMessage', 'sendString', 'open', 'isLocked',
    'lock', 'varp', 'submit', 'pulseStop', 'log', 'logMS', 'logChat', 'logPrivateChat', 'cipher', 'write', 'msWrite', 'mask', 'take',
    'decoded', 'accountInfo', 'accountUpdate', 'createAccount', 'define',
]

CHARS = '_abcdefghijklmnopqrstuvwxyz0123456789'


def name_long(name):
    """StringUtils.stringToLong."""
    value = 0
    for ch in name[:12]:
        value *= 37
        if 'A' <= ch <= 'Z':
            value += ord(ch) - 64
        elif 'a' <= ch <= 'z':
            value += ord(ch) - 96
        elif '0' <= ch <= '9':
            value += ord(ch) - 21
    while value % 37 == 0 and value != 0:
        value //= 37
    return value & 0xffffffffffffffff


def long_hex(value):
    return '%016x' % (value & 0xffffffffffffffff)


def a(op, **kw):
    return dict(op=op, **kw)


def add_friend(p, name):
    return a('packet', player=p, opcode=120, hex=long_hex(name_long(name)))


def remove_friend(p, name):
    return a('packet', player=p, opcode=57, hex=long_hex(name_long(name)))


def add_ignore(p, name):
    return a('packet', player=p, opcode=34, hex=long_hex(name_long(name)))


def remove_ignore(p, name):
    return a('packet', player=p, opcode=213, hex=long_hex(name_long(name)))


def join_clan(p, name):
    return a('packet', player=p, opcode=104, hex=long_hex(name_long(name)) if name else long_hex(0))


def kick_clan(p, name):
    return a('packet', player=p, opcode=162, hex=long_hex(name_long(name)))


def set_rank(p, name, rank):
    return a('packet', player=p, opcode=188, hex='%02x' % ((rank + 128) & 0xff) + long_hex(name_long(name)))


def chat_setting(p, public, private, trade):
    return a('packet', player=p, opcode=157, hex=''.join('%02x' % (v & 0xff) for v in (public, private, trade)))


def chat(p, text, effects=(0, 0), **kw):
    return a('chat', player=p, text=text, effects=list(effects), **kw)


def pm(p, to, text, **kw):
    return a('pm', player=p, to=to, text=text, **kw)


def ev(kind, **fields):
    return {'@': kind, **fields}


def world(**kw):
    base = dict(worldId=1, name='2009Scape', devMode=False, defaultClan=True, serverName='2009Scape', msKey='secret', globalChat=True,
                ms='CONNECTING', msSession=False)
    base.update(kw)
    return base


NAMES = ['alice', 'bob', 'carol', 'dave', 'erin', 'frank', 'grace', 'heidi']


def cast(count=4, **over):
    players = {}
    for i in range(count):
        label = 'abcdefgh'[i]
        players[label] = dict(name=NAMES[i])
    for label, changes in over.items():
        players.setdefault(label, {}).update(changes)
    return players


def contacts(*pairs):
    return '~'.join('{%s,%d}' % (name, rank) for name, rank in pairs)


def requests():
    cases = []

    def add(name, actions, players=None, online=None, **kw):
        players = players if players is not None else cast()
        online = online if online is not None else sorted(players)
        case = dict(name=name, world=kw.pop('world', world()), players=players, online=online, seeds=kw.pop('seeds', [11, 22, 33, 44, 55, 66, 77, 88]),
                    clock=kw.pop('clock', 1_000_000), actions=actions)
        case.update(kw)
        cases.append(case)

    # --- Friends: adding, duplicates, full lists, offline targets, name forms.
    add('add-online-friend', [add_friend('a', 'bob'), a('flush')])
    add('add-offline-friend', [add_friend('a', 'zed'), a('flush')])
    add('add-duplicate', [add_friend('a', 'bob'), add_friend('a', 'bob'), a('flush')], players=cast(b=dict(contacts=contacts(('alice', 1)))))
    add('add-empty-name', [add_friend('a', ''), a('add', player='a', name=''), a('flush')])
    full = contacts(*[('friend%03d' % i, 1) for i in range(200)])
    add('add-list-full', [add_friend('a', 'bob'), a('flush')], players=cast(a=dict(contacts=full)))
    add('add-list-199', [add_friend('a', 'bob'), add_friend('a', 'carol'), a('flush')],
        players=cast(a=dict(contacts=contacts(*[('friend%03d' % i, 1) for i in range(199)]))))
    add('add-name-forms', [a('add', player='a', name='Bob'), a('add', player='a', name='bob'), a('add', player='a', name='BOB'),
                           a('add', player='a', name='dave smith'), a('add', player='a', name='dave_smith'), a('add', player='a', name='Carol'),
                           a('hasContact', player='a', name='bob'), a('hasContact', player='a', name='Bob'), a('showActive', player='a', name='Dave Smith'),
                           a('flush')],
        players=cast(d=dict(name='dave_smith')))
    add('add-hash-order', [add_friend('a', n) for n in ['zz', 'aa', 'mm', 'b0', 'c0', 'an', 'bob', 'Ab', 'BA', 'q9']] + [a('flush')])
    add('add-many-then-remove', [add_friend('a', 'p%02d' % i) for i in range(20)] + [remove_friend('a', 'p%02d' % i) for i in range(0, 20, 3)]
        + [add_friend('a', 'p05'), a('flush')])
    for adder in range(3):
        for target in range(3):
            for has in [False, True]:
                for blocks in [False, True]:
                    b = dict(settings=[0, target, 0])
                    if has:
                        b['contacts'] = contacts(('alice', 1))
                    if blocks:
                        b['blocked'] = 'alice'
                    add(f'add-privacy-{adder}-{target}-{int(has)}-{int(blocks)}',
                        [add_friend('a', 'bob'), a('showActive', player='a', target='b'), a('showActive', player='b', target='a'), a('flush')],
                        players=cast(a=dict(settings=[0, adder, 0]), b=b))
    add('add-self', [add_friend('a', 'alice'), a('showActive', player='a', target='a'), a('flush')])
    add('add-artificial-and-ai', [add_friend('a', 'bob'), add_friend('a', 'carol'), add_friend('b', 'alice'), add_friend('c', 'alice'),
                                   add_friend('d', 'alice'), a('flush')],
        players=cast(a=dict(settings=[0, 1, 0]), b=dict(kind='artificial'), c=dict(kind='ai'), d=dict(session=False)))
    add('add-ms-available', [add_friend('a', 'bob'), remove_friend('a', 'bob'), add_ignore('a', 'bob'), remove_ignore('a', 'bob'), a('flush')],
        world=world(ms='AVAILABLE', msSession=True))
    add('add-ms-available-no-session', [add_friend('a', 'bob'), add_ignore('a', 'carol'), a('flush')], world=world(ms='AVAILABLE'))
    add('add-ms-not-available', [add_friend('a', 'bob'), a('flush')], world=world(ms='NOT_AVAILABLE', msSession=True))
    add('add-with-own-clan', [add_friend('a', 'bob'), add_friend('a', 'carol'), a('flush')],
        players=cast(a=dict(clanName='Alices', contacts=contacts(('dave', 3)))))
    add('add-cipher', [add_friend('a', 'bob'), add_friend('a', 'carol'), a('flush')],
        players=cast(a=dict(cipher=[5, -3, 250, 128])))
    add('add-failure-message', [add_friend('a', 'bob'), add_friend('a', 'bob'), a('flush')], failures=dict(playerMessage=[1], dispatchMessage=[1]))
    add('add-failure-flush', [add_friend('a', 'bob'), add_friend('a', 'carol'), add_friend('a', 'dave'), a('flush'), a('flush')],
        failures=dict(write=[2]))
    add('add-failure-cipher', [add_friend('a', 'bob'), add_friend('a', 'carol'), a('flush')], failures=dict(cipher=[1]))

    # --- Removing friends and ignores.
    add('remove-friend-states', [remove_friend('a', 'zed'), remove_friend('a', 'bob'), remove_friend('a', 'bob'), a('remove', player='a', name=''),
                                 a('flush')],
        players=cast(a=dict(contacts=contacts(('bob', 2), ('carol', 1)))))
    for setting in range(3):
        add(f'remove-friend-private-{setting}', [remove_friend('a', 'bob'), remove_friend('a', 'zed'), a('flush')],
            players=cast(a=dict(settings=[0, setting, 0], contacts=contacts(('bob', 1), ('zed', 1)))))
    add('remove-friend-with-clan', [remove_friend('a', 'bob'), remove_friend('a', 'carol'), a('flush')],
        players=cast(a=dict(clanName='Alices', contacts=contacts(('bob', 2), ('carol', 1)), clanReqs='2,0,8,9')))
    for has in [False, True]:
        for setting in range(3):
            add(f'remove-ignore-{int(has)}-{setting}', [remove_ignore('a', 'bob'), remove_ignore('a', 'bob'), a('flush')],
                players=cast(a=dict(blocked='bob,carol', settings=[0, setting, 0]),
                             b=dict(contacts=contacts(('alice', 1)) if has else '', settings=[0, 1, 0])))
    add('remove-ignore-name-forms', [a('remove', player='a', name='Bob', block=True), a('remove', player='a', name='bob', block=True),
                                     a('remove', player='a', name='', block=True), a('flush')],
        players=cast(a=dict(blocked='Bob,bob,bob')))

    # --- Ignores.
    add('block-basic', [add_ignore('a', 'bob'), add_ignore('a', 'bob'), add_ignore('a', 'zed'), a('add', player='a', name='x'),
                        a('block', player='a', name=''), a('flush')],
        players=cast(b=dict(contacts=contacts(('alice', 1)))))
    add('block-full', [add_ignore('a', 'bob'), a('flush')], players=cast(a=dict(blocked=','.join('ig%03d' % i for i in range(200)))))
    add('block-199', [add_ignore('a', 'bob'), add_ignore('a', 'carol')], players=cast(a=dict(blocked=','.join('ig%03d' % i for i in range(198)))))
    add('block-name-forms', [a('block', player='a', name='Bob'), a('block', player='a', name='bob'), a('block', player='a', name='Bob Smith'),
                             a('showActive', player='b', target='a'), a('flush')],
        players=cast(b=dict(contacts=contacts(('alice', 1)))))

    # --- Clan ranks of contacts.
    for rank in [-2, -1, 0, 1, 6, 7, 8, 9]:
        add(f'set-rank-{rank}', [set_rank('a', 'bob', rank), a('flush')], players=cast(a=dict(contacts=contacts(('bob', 1)))))
    add('set-rank-missing', [set_rank('a', 'zed', 2), a('updateRank', player='a', name='', rank=3), a('flush')])
    add('set-rank-clan', [set_rank('a', 'bob', 3), set_rank('a', 'bob', 3), set_rank('a', 'carol', -1), a('flush')],
        players=cast(a=dict(clanName='Alices', contacts=contacts(('bob', 1), ('carol', 2)), clanReqs='2,0,8,9')))
    add('set-rank-world', [a('publish', event=ev('PlayerStatusUpdate', username='bob', world=1, notifyFriendsOnly=False)), a('pump'),
                           set_rank('a', 'bob', 2),
                           a('flush')],
        players=cast(a=dict(contacts=contacts(('bob', 1)))))

    # --- Private messages.
    add('pm-online', [pm('a', 'bob', 'hello there'), a('pump'), a('flush'), pm('a', 'bob', 'Second one!'), a('pump'), a('flush')],
        seeds=[1, 2, 3, 4])
    add('pm-offline', [pm('a', 'zed', 'anyone?'), a('pump'), a('flush')])
    add('pm-ignored-hidden', [pm('a', 'bob', 'you blocked me'), pm('a', 'carol', 'you hide'), a('pump'), a('flush')],
        players=cast(b=dict(blocked='alice'), c=dict(settings=[0, 2, 0])))
    add('pm-muted', [pm('a', 'bob', 'muted words'), a('pump'), a('flush')], players=cast(a=dict(muteEnd=2_000_000)))
    add('pm-mute-expired', [pm('a', 'bob', 'free again'), a('pump'), a('flush')], players=cast(a=dict(muteEnd=1_000_000)))
    add('pm-self-and-ranks', [pm('a', 'alice', 'note to self'), pm('b', 'carol', 'from a mod'), pm('c', 'bob', 'from an admin'), a('pump'),
                              a('flush')],
        players=cast(b=dict(rights=1), c=dict(rights=2, attributes={'visible_rank': {'rights': 0}})))
    add('pm-indices-and-reply', [pm('a', 'bob', 'one'), pm('c', 'bob', 'two'), a('pump'), a('flush'), pm('a', 'bob', 'three'), a('pump'),
                                 a('flush')],
        players=cast(b=dict(attributes={'pm_index': 41})), seeds=[5, 6, 7, 8, 9, 10])
    add('pm-text-forms', [pm('a', 'bob', 'UPPER lower 0123 !?.,'), pm('a', 'bob', 'x' * 80), pm('a', 'bob', ''), pm('a', 'bob', 'café €'),
                          pm('a', 'bob', 'abc', count=10), pm('a', 'bob', 'abcdef', count=2), a('pump'), a('flush')],
        seeds=list(range(100, 120)))
    add('pm-artificial-receivers', [pm('a', 'bob', 'to a bot'), pm('a', 'carol', 'to an ai'), pm('a', 'dave', 'no session'), a('pump'),
                                    a('flush')],
        players=cast(b=dict(kind='artificial'), c=dict(kind='ai'), d=dict(session=False)))
    add('pm-receiver-logs-out', [pm('a', 'bob', 'are you there'), a('removePlayer', player='b'), a('pump'), a('flush')])
    add('pm-direct-names', [a('sendMessage', player='a', name='Bob', text='Capital'), a('sendMessage', player='a', name='bob smith', text='space'),
                            a('pump'), a('flush')],
        players=cast(b=dict(name='bob_smith')))
    add('pm-failures', [pm('a', 'bob', 'first'), pm('a', 'bob', 'second'), a('pump'), a('flush')],
        failures=dict(logPrivateChat=[1], setAttribute=[2], logMS=[1]), seeds=list(range(1, 9)))
    add('pm-failure-getattribute', [pm('a', 'bob', 'first'), a('pump'), a('flush')], failures=dict(getAttribute=[2]), seeds=list(range(1, 9)))
    add('pm-decode-errors', [a('packet', player='a', opcode=201, hex='0000000000'),a('packet', player='a', opcode=201, hex=long_hex(name_long('bob'))),
                             a('packet', player='a', opcode=201, hex=long_hex(name_long('bob')) + '05'),
                             a('packet', player='a', opcode=201, hex=long_hex(name_long('bob')) + '0000'),
                             a('packet', player='a', opcode=201, hex=long_hex(name_long('bob')) + '03ffffff'), a('pump'), a('flush')],
        seeds=list(range(1, 9)))

    # --- Login and logout notifications.
    def friends_of(name, rank=1):
        return contacts((name, rank))

    fan = cast(6, a=dict(contacts=contacts(('bob', 1), ('carol', 2), ('zed', 0)), blocked='erin,,frank'),
               b=dict(contacts=friends_of('alice')), c=dict(contacts=friends_of('alice', 4), settings=[0, 2, 0]),
               d=dict(contacts=friends_of('alice'), blocked='alice'), e=dict(contacts=contacts(('Alice', 1))), f=dict())
    accounts = dict(alice=dict(contacts=contacts(('bob', 1), ('carol', 2), ('zed', 0), ('dave', 5)), blocked='erin,frank'))
    add('login-fanout', [a('login', player='a'), a('pump'), a('flush')], players=fan, accounts=accounts)
    add('login-logout-fanout', [a('login', player='a'), a('pump'), a('flush'), a('logout', player='a'), a('removePlayer', player='a'), a('pump'),
                                a('flush')],
        players=fan, accounts=accounts)
    add('logout-before-removal', [a('logout', player='a'), a('pump'), a('removePlayer', player='a'), a('flush')], players=fan, accounts=accounts)
    add('login-order', [a('login', player=p) for p in 'fedcba'] + [a('pump'), a('flush')], players=fan, accounts=accounts,
        online=['f', 'e', 'd', 'c', 'b', 'a'])
    add('login-no-account', [a('login', player='b'), a('pump'), a('flush')], players=fan)
    add('login-current-clan', [a('login', player='b'), a('pump'), a('flush')],
        players=cast(a=dict(clanName='Alices', clanReqs='0,0,8,9'), b=dict(currentClan='alice', contacts=friends_of('alice'))),
        accounts=dict(bob=dict(contacts=friends_of('alice'))))
    add('login-current-clan-offline-owner', [a('login', player='b'), a('pump'), a('flush')],
        players=cast(b=dict(currentClan='zed')), online=['b'],
        accounts=dict(zed=dict(clanName='Zeds', clanReqs='0,0,8,9', contacts=contacts(('bob', 3)))))
    add('login-pending-without-pump', [a('login', player='a'), a('login', player='b'), a('flush')], players=fan, accounts=accounts)
    add('login-failures', [a('login', player='a'), a('pump'), a('flush')], players=fan, accounts=accounts, failures=dict(accountInfo=[1], write=[1]))
    add('status-other-world', [a('publish', event=ev('PlayerStatusUpdate', username='alice', world=7, notifyFriendsOnly=False)),
                               a('publish', event=ev('PlayerStatusUpdate', username='alice', world=0, notifyFriendsOnly=True)),
                               a('pump'), a('flush')],
        players=fan, onlineFriends=dict(alice=['bob', 'dave', 'zed', 'erin']))
    add('contact-info-direct', [a('publish', event=ev('SendContactInfo', username='alice', contacts=[dict(username='bob', world=1, rank=3),
                                                                                                       dict(username='Zed', world=0, rank=0)],
                                                      blocked=['carol', '', 'Dave'])),
                                a('publish', event=ev('SendContactInfo', username='nobody')), a('pump'), a('flush')],
        players=fan)
    add('friend-update-noop', [a('publish', event=ev('FriendUpdate', username='alice', friend='bob', type='REMOVE')), a('pump'), a('flush')],
        players=fan)
    big = {}
    for i in range(24):
        big['p%02d' % i] = dict(name='u%02d' % i, contacts=contacts(('alice', 1)) if i % 3 else '')
    big['p00'] = dict(name='alice', contacts=contacts(*[('u%02d' % i, 1) for i in range(1, 24)]))
    add('login-large-order', [a('login', player='p00'), a('pump'), a('flush')], players=big,
        accounts=dict(alice=dict(contacts=contacts(*[('u%02d' % i, 1) for i in range(1, 24)]))))

    # --- Clans: entry, requirements, members, packets.
    owner = dict(clanName='Alices', clanReqs='0,0,8,9', contacts=contacts(('bob', 1), ('carol', 3), ('dave', 7)))
    add('clan-get-create', [a('clanGet', owner='alice'), a('clanGet', owner='bob'), a('clanGet', owner='bob', create=True),
                            a('clanGet', owner='zed'), a('clanGet', owner='zed', create=True), a('clanGet', owner='zed')],
        players=cast(a=owner))
    for req in range(10):
        add(f'clan-join-req-{req}', [join_clan(p, 'alice') for p in 'abcde'] + [a('pump'), a('flush'),
                                                                                a('clanGetRank', clan='alice', player='b')],
            players=cast(5, a=dict(owner, clanReqs=f'{req},0,8,9'), e=dict(rights=2)))
    add('clan-join-offline-owner', [join_clan('b', 'zed'), join_clan('c', 'zed'), a('pump'), a('flush')],
        accounts=dict(zed=dict(clanName='Zeds', clanReqs='0,0,8,9', contacts=contacts(('bob', 4)))))
    add('clan-join-missing', [join_clan('b', 'zed'), a('pump'), a('flush'), join_clan('b', 'zed'), a('pump')])
    add('clan-join-default', [join_clan('b', '2009Scape'), a('pump'), a('flush')])
    add('clan-join-default-existing-account', [join_clan('b', '2009scape'), a('pump'), a('flush')],
        accounts={'2009scape': dict(clanReqs='1,0,8,9', rights=2)})
    add('clan-join-default-disabled', [join_clan('b', '2009scape'), a('pump'), a('flush')], world=world(defaultClan=False))
    add('clan-join-blocked-banned', [join_clan('b', 'alice'), a('pump'), a('clanKick', clan='alice', player='a', target='b'), a('flush'),
                                     join_clan('b', 'alice'), join_clan('c', 'alice'), a('pump'), a('clock', value=4_600_000),
                                     a('clanIsBanned', clan='alice', name='bob'), a('clock', value=4_600_001), join_clan('b', 'alice'),
                                     a('pump'), a('flush')],
        players=cast(a=dict(owner, blocked='carol')))
    add('clan-enter-twice', [join_clan('b', 'alice'), a('pump'), join_clan('b', 'alice'), a('pump'), a('flush')], players=cast(a=owner))
    add('clan-full', [a('clanGet', owner='alice')] + [a('publish', event=ev('ClanJoinNotification', username='x%03d' % i, world=2, clanName='alice'))
                                                       for i in range(100)]
        + [a('pump'), join_clan('b', 'alice'), a('pump'), a('flush')],
        players=cast(a=owner))
    add('clan-default-unlimited', [join_clan('b', '2009scape')]
        + [a('publish', event=ev('ClanJoinNotification', username='y%03d' % i, world=3, clanName='2009scape')) for i in range(101)]
        + [a('pump'), join_clan('c', '2009scape'), a('pump'), a('flush')],
        accounts={'2009scape': dict(clanName='Global', clanReqs='-1,-1,7,7')})
    add('clan-messages', [join_clan(p, 'alice') for p in 'abcd'] + [a('pump'), a('flush'), chat('b', '/hello clan'), chat('c', '/  '),
                                                                     chat('d', '/rank too low'), a('pump'), a('flush'),
                                                                     a('clanMessage', clan='alice', player='b', text='direct'),
                                                                     a('clanMessage', clan='alice', player='b', text='locked'),
                                                                     a('unlock', player='b', key='cc_message'),
                                                                     a('clanMessage', clan='alice', player='d', text='low rank'),
                                                                     a('clanMessage', clan='alice', player='a', text='owner'), a('flush')],
        players=cast(a=dict(owner, clanReqs='0,3,8,9')), seeds=list(range(1, 40)))
    add('clan-message-long', [join_clan('b', 'alice'), a('pump'), chat('b', '/' + ' '.join(['word%d' % i for i in range(30)])), a('pump'),
                              a('flush')],
        players=cast(a=dict(owner, clanName='A very long clan name')), seeds=list(range(1, 20)))
    add('clan-message-no-clan', [chat('b', '/not in a clan'), a('flush')])
    add('clan-kick-rules', [join_clan(p, 'alice') for p in 'abcde'] + [a('pump'), a('flush'),
                                                                        a('clanKick', clan='alice', player='b', target='c'),
                                                                        a('clanKick', clan='alice', player='c', target='c'),
                                                                        a('clanKick', clan='alice', player='c', target='e'),
                                                                        a('clanKick', clan='alice', player='c', target='a'),
                                                                        a('clanKick', clan='alice', player='d', target='b'),
                                                                        a('clanKick', clan='alice', player='e', target='a'),
                                                                        kick_clan('a', 'carol'), kick_clan('a', 'zed'), kick_clan('b', 'bob'),
                                                                        a('pump'), a('flush')],
        players=cast(5, a=dict(owner, clanReqs='0,0,4,9'), e=dict(rights=2)), seeds=list(range(1, 60)))
    add('clan-leave', [join_clan(p, 'alice') for p in 'abc'] + [a('pump'), a('flush'), join_clan('b', ''), join_clan('d', ''), a('pump'),
                                                               a('flush'), a('clanLeave', clan='alice', player='c'),
                                                               a('clanLeave', clan='alice', player='c', remove=True, message='Bye.:clan:'),
                                                               a('pump'), a('flush')],
        players=cast(a=owner))
    add('clan-leave-unknown', [join_clan('b', ''), a('pump'), a('flush')], players=cast(b=dict(currentClan='zed')))
    add('clan-rank-clean', [join_clan(p, 'alice') for p in 'abcd'] + [a('pump'), a('flush'), set_rank('a', 'carol', 0), set_rank('a', 'dave', -1),
                                                                     a('flush'), a('clanRank', clan='alice', name='bob', rank=0),
                                                                     a('clanRank', clan='alice', name='bob', rank=2), a('flush'),
                                                                     a('pump')],
        players=cast(a=dict(owner, clanReqs='2,0,8,9')))
    add('clan-settings', [join_clan(p, 'alice') for p in 'abcd'] + [a('pump'), a('flush'), a('clanSet', clan='alice', which='kick', rank=5),
                                                                   a('clanSet', clan='alice', which='message', rank=3),
                                                                   a('clanSet', clan='alice', which='loot', rank=9),
                                                                   a('clanSet', clan='alice', which='name', name='Renamed'),
                                                                   a('clanSet', clan='alice', which='join', rank=3), a('flush'),
                                                                   a('openSettings', player='a'), a('openSettings', player='b'), a('flush')],
        players=cast(a=owner))
    add('clan-delete', [join_clan(p, 'alice') for p in 'abc'] + [a('pump'), a('flush'), a('clanDelete', clan='alice'), a('flush'), a('pump'),
                                                               a('clanGet', owner='alice'), join_clan('b', 'alice'), a('pump'), a('flush')],
        players=cast(a=owner))
    add('clan-clean-ms', [join_clan(p, 'alice') for p in 'abc'] + [a('pump'), a('flush'), a('clanClean', clan='alice', disable=True), a('flush')],
        players=cast(a=owner), world=world(ms='AVAILABLE', msSession=True))
    add('clan-other-worlds', [join_clan('b', 'alice'), a('pump'),
                              a('publish', event=ev('ClanJoinNotification', username='remote', world=2, clanName='alice')),
                              a('publish', event=ev('ClanJoinNotification', username='bob', world=1, clanName='alice')),
                              a('publish', event=ev('ClanLeaveNotification', username='remote', world=2, clanName='alice')),
                              a('publish', event=ev('ClanLeaveNotification', username='ghost', world=2, clanName='alice')),
                              a('pump'), a('flush'),
                              a('publish', event=ev('RequestClanInfo', clanOwner='alice', world=2)),
                              a('publish', event=ev('RequestClanInfo', clanOwner='nobody', world=2)), a('pump'), a('flush')],
        players=cast(a=owner))
    add('clan-info-direct', [a('publish', event=ev('SendClanInfo', hasInfo=True, clanOwner='zed', clanName='Zeds', joinRequirement=0,
                                                   messageRequirement=1, kickRequirement=7, lootRequirement=9,
                                                   members=[dict(username='bob', world=1, rank=2), dict(username='zed', world=2, rank=8),
                                                            dict(username='nobody', world=1, rank=0)])),
                             a('pump'), a('flush'), chat('b', '/hi from bob'), a('pump'), a('flush')],
        seeds=list(range(1, 10)))
    add('clan-messages-queued', [a('publish', event=ev('ClanMessage', sender='alice', clanName='zed', message='early', rank=0)),
                                 a('publish', event=ev('LeaveClanRequest', username='bob', clanName='zed')),
                                 a('publish', event=ev('ClanLeaveNotification', username='bob', world=2, clanName='zed')),
                                 a('publish', event=ev('ClanJoinNotification', username='bob', world=2, clanName='zed')),
                                 join_clan('b', 'zed'), a('pump'), a('flush')],
        accounts=dict(zed=dict(clanName='Zeds', clanReqs='0,0,8,9')), seeds=list(range(1, 10)))
    add('clan-logout', [join_clan(p, 'alice') for p in 'abc'] + [a('pump'), a('flush'), a('logout', player='b'), a('removePlayer', player='b'),
                                                               a('pump'), a('flush')],
        players=cast(a=owner))
    add('clan-set-clan-direct', [a('clanGet', owner='alice'), a('setClan', player='b', clan='alice'), a('setClan', player='b'),
                                 a('setClan', player='c', clan='alice'), a('logout', player='c'), a('pump')],
        players=cast(a=owner))
    add('clan-update-direct', [join_clan('b', 'alice'), a('pump'), a('clanUpdate', clan='alice'), a('flush'),
                               a('clanLeave', clan='alice', player='a', remove=True), a('flush')],
        players=cast(a=owner))
    add('clan-failures', [join_clan(p, 'alice') for p in 'abc'] + [a('pump'), a('flush'), a('clanMessage', clan='alice', player='b', text='x'),
                                                                 a('openSettings', player='a'), a('flush')],
        players=cast(a=owner), failures=dict(dispatchMessage=[3], write=[2], logChat=[1], open=[1], accountInfo=[1]), seeds=list(range(1, 40)))
    add('clan-kick-message-failure', [join_clan(p, 'alice') for p in 'ab'] + [a('pump'), a('clanKick', clan='alice', player='a', target='b'),
                                                                              a('flush')],
        players=cast(a=owner), failures=dict(dispatchMessage=[3]), seeds=list(range(1, 40)))

    add('clan-remote-entries-fail', [join_clan('b', 'alice'), a('pump'),
                                     a('publish', event=ev('ClanJoinNotification', username='remote', world=2, clanName='alice')),
                                     a('pump'), a('flush'), a('clanKick', clan='alice', player='a', target='b'), a('flush'),
                                     a('clanRank', clan='alice', name='zed', rank=4), a('clanClean', clan='alice', disable=True),
                                     a('clanGetRank', clan='alice', player='a'), a('flush')],
        players=cast(a=owner), seeds=list(range(1, 20)))
    add('management-bad-ranks', [a('publish', event=ev('SendContactInfo', username='alice', contacts=[dict(username='bob', world=1, rank=2),
                                                                                                          dict(username='carol', world=1, rank=12),
                                                                                                          dict(username='dave', world=1, rank=1)],
                                                       blocked=['x'])),
                                 a('publish', event=ev('SendClanInfo', hasInfo=True, clanOwner='zed', clanName='Zeds', joinRequirement=0,
                                                       messageRequirement=1, kickRequirement=11, lootRequirement=9,
                                                       members=[dict(username='bob', world=1, rank=2)])),
                                 a('publish', event=ev('SendClanInfo', hasInfo=True, clanOwner='yul', clanName='Yuls', joinRequirement=0,
                                                       messageRequirement=1, kickRequirement=7, lootRequirement=9,
                                                       members=[dict(username='bob', world=1, rank=2), dict(username='carol', world=1, rank=-1)])),
                                 a('pump'), a('flush')])
    add('rank-shown-world', [a('publish', event=ev('PlayerStatusUpdate', username='bob', world=1, notifyFriendsOnly=False)),
                             a('publish', event=ev('PlayerStatusUpdate', username='carol', world=5, notifyFriendsOnly=False)), a('pump'),
                             set_rank('a', 'bob', 4), set_rank('a', 'carol', 4), set_rank('a', 'dave', 4), a('flush')],
        players=cast(a=dict(contacts=contacts(('bob', 1), ('carol', 1), ('dave', 1))), c=dict(settings=[0, 2, 0])))
    add('global-default-clan-join', [join_clan('b', '2009scape'), a('pump'), chat('b', '/hello global'), a('pump'), a('flush')],
        accounts={'2009scape': dict(clanName='Global', clanReqs='-1,-1,7,7')}, seeds=list(range(1, 20)))

    # --- Loot share.
    add('loot-share', [a('toggleLoot', player='a'), a('toggleLoot', player='a'), a('toggleLoot', player='a'), a('runPulse', id=1),
                       a('toggleLoot', player='a'), a('toggleLoot', player='a'), a('runPulse', id=1)])
    add('loot-share-dev', [a('toggleLoot', player='a'), a('runPulse', id=0), a('toggleLoot', player='a')], world=world(devMode=True))
    add('loot-share-failure', [a('toggleLoot', player='a'), a('toggleLoot', player='a'), a('toggleLoot', player='a')],
        failures=dict(submit=[1], varp=[3]))

    # --- Public chat, global chat and settings.
    add('public-chat', [chat('a', 'Hello world', (2, 1)), chat('b', 'Admin talk', (11, 5)), chat('c', 'hidden admin'), chat('d', 'mod'),
                        chat('a', 'clamped', (-4, 99)), a('flush')],
        players=cast(b=dict(rights=2), c=dict(rights=2, attributes={'visible_rank': {'rights': 0}}), d=dict(rights=1)))
    add('public-chat-muted', [chat('a', 'muted'), chat('a', '//muted global')], players=cast(a=dict(muteEnd=9_000_000)))
    add('public-chat-forms', [chat('a', ''), chat('a', 'abc', count=0), chat('a', 'abc', count=9), chat('a', 'x' * 150),
                              a('packet', player='a', opcode=237, hex='0000'), a('packet', player='a', opcode=237, hex='00'),
                              a('packet', player='a', opcode=237, hex='000180')])
    add('global-chat', [a('defineGlobal'), chat('a', '//hello everyone'), chat('b', '//from admin'), a('command', name='muteglobal', player='c'),
                        chat('a', '//after mute'), chat('c', '//muted sender'), chat('a', '//' + ' '.join(['w%d' % i for i in range(40)])),
                        chat('a', '//   ')],
        players=cast(a=dict(resizable=True), b=dict(rights=2)))
    add('global-chat-disabled', [chat('a', '//no global')], world=world(globalChat=False))
    add('global-process', [a('globalProcess', sender='Some One', text='%R red %ffffff hex', rights=0),
                           a('globalProcess', sender='x', text='icons', rights=2)],
        players=cast(a=dict(resizable=True), b=dict(attributes={'globalmute': True})))
    add('chat-settings', [chat_setting('a', 0, 0, 0), chat_setting('a', 1, 2, 0), chat_setting('a', 1, 1, 2), chat_setting('a', -1, 5, 9)])
    add('chat-settings-ms', [chat_setting('a', 1, 0, 0), chat_setting('a', 1, 2, 1)], world=world(msSession=True))
    add('chat-split', [a('split', text=text, clanLength=length, rank=rank)
                       for text in ['', 'short', 'a ' * 50, 'x' * 100, 'one two three ' * 7, ' leading', 'trailing ']
                       for length in [0, 10, 70, 90] for rank in [False, True]])
    add('chat-message-context', [a('chatMessage', player='a', text='hi', effects=257, count=2),
                                 a('chatMessage', player='b', text='', effects=0, count=0)],
        players=cast(b=dict(rights=1, attributes={'visible_rank': {'rights': 2}})))
    add('chat-failures', [chat('a', 'one'), chat('a', 'two')], failures=dict(logChat=[1], mask=[1]))

    # --- Decoders: truncation and unknown opcodes.
    add('decode-errors', [a('packet', player='a', opcode=120, hex='0102'), a('packet', player='a', opcode=188, hex='81'),
                          a('packet', player='a', opcode=157, hex='01'), a('packet', player='a', opcode=104, hex=''),
                          a('packet', player='a', opcode=162, hex='ffffffffffffffff'), a('packet', player='a', opcode=99, hex='00'),
                          a('packet', player='a', opcode=120, hex='8000000000000000'), a('packet', player='a', opcode=120,
                                                                                         hex=long_hex(name_long('abcdefghijkl') * 37))])
    return cases
