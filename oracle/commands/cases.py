"""The commands corpus: authored cases for the command system, the four ported sets, abuse reports and quick chat.

Each case is a small world (accounts, players, attributes, service answers) and a list of operations; cases() yields
(case, sweep) where sweep is the index of an operation whose every recorded call is then made to fail in turn.
"""

ADMIN, MOD, REGULAR = 2, 1, 0


def accounts(**extra):
    out = {
        'alice': dict(username='alice', rights=ADMIN, password='secret1'),
        'bob': dict(username='bob', rights=REGULAR, password='hunter2'),
        'carol': dict(username='carol', rights=MOD, password='pa55word'),
    }
    out.update(extra)
    return out


def players(**extra):
    out = {
        'p1': dict(name='alice', username='Alice', account='alice', location=[3222, 3218, 0]),
        'p2': dict(name='bob', username='Bob', account='bob', location=[3200, 3200, 0]),
        'p3': dict(name='carol', username='Carol', account='carol', location=[3100, 3100, 1]),
    }
    out.update(extra)
    return out


def case(name, ops, answers=None, values=None, accts=None, people=None, extra=None, files=None, group=None):
    v = dict(playerCommands=False, cheater=False, settings=dict(devMode=False, beta=False), home=[3222, 3218, 0], dataPath='data',
             castleWars=[2443, 3083, 0],
             clock=[1000000, 2000000, 3000000])
    v.update(values or {})
    return dict(name=name, group=group or name.split('-')[0], values=v, accounts=accts or accounts(), players=people or players(),
                answers=answers or {}, extra=extra or [], files=files or {}, ops=ops)


def cmd(line, player='p1'):
    return dict(op='command', player=player, line=line)


def parse(line, player='p1'):
    return dict(op='parse', player=player, line=line)


def hexed(data):
    return data.hex()


def packet(opcode, data, player='p1'):
    return dict(op='packet', opcode=opcode, hex=hexed(data), player=player)


VALID = '_abcdefghijklmnopqrstuvwxyz0123456789'


def name_long(text):
    value = 0
    for ch in text[:12]:
        value *= 37
        if 'A' <= ch <= 'Z':
            value += 1 + ord(ch) - 65
        elif 'a' <= ch <= 'z':
            value += 1 + ord(ch) - 97
        elif '0' <= ch <= '9':
            value += 27 + ord(ch) - 48
    while value % 37 == 0 and value != 0:
        value //= 37
    return value


def report(target, rule, mute, player='p1', raw=None):
    value = name_long(target) if raw is None else raw
    return packet(99, value.to_bytes(8, 'big', signed=True) + bytes([rule & 0xFF, 1 if mute else 0]), player)


EXTRA = [
    ['bank', 'ADMIN', 'core.game.system.command.sets.FunCommandSet', 3],
    ['loc', 'STANDARD', 'core.game.system.command.sets.MiscCommandSet', 5],
    ['players', 'MODERATOR', 'core.game.system.command.sets.MiscCommandSet', 12],
]


def framework():
    yield case('framework-null-player', [dict(op='parse', player=None, line='kick bob')]), None
    yield case('framework-registry', [dict(op='names'), dict(op='pages', rights=0), dict(op='pages', rights=1), dict(op='pages', rights=2)],
               extra=EXTRA), None
    for who in ['p1', 'p2', 'p3']:
        yield case(f'framework-unknown-{who}', [parse('nosuchcommand a b', who), cmd('nosuch', who)]), None
    yield case('framework-unknown-zone', [parse('zonecmd', 'p2')], answers={'zone': True}), None
    yield case('framework-unknown-legacy', [parse('dump', 'p1')], answers={'legacy:core.game.system.command.oldsys.SimpleDumpingCommands': True}), 0
    yield case('framework-unknown-legacy-first', [parse('dump', 'p1')], answers={'legacy:core.game.system.command.MapDumpCommand': True}), None
    yield case('framework-unknown-null-settings', [parse('nosuch', 'p2'), parse('nosuch', 'p1')], values=dict(settings=None)), None
    yield case('framework-unknown-beta', [parse('nosuch', 'p2')], values=dict(settings=dict(devMode=False, beta=True))), None
    yield case('framework-unknown-dev', [parse('nosuch', 'p2')], values=dict(settings=dict(devMode=True, beta=False))), None
    yield case('framework-bad-rights', [parse('kick bob', 'p2'), parse('nosuch', 'p2')], accts=accounts(bob=dict(username='bob', rights=7))), None
    yield case('framework-empty', [parse('', 'p1'), parse(' ', 'p1'), parse('  kick', 'p1')]), None
    for who in ['p1', 'p2', 'p3']:
        for commands in [False, True]:
            yield case(f'framework-rights-{who}-{"on" if commands else "off"}',
                       [parse('loc', who), parse('players', who), parse('bank', who), parse('kick nobody', who)],
                       values=dict(playerCommands=commands), extra=EXTRA), None
    yield case('framework-dev', [parse('kick nobody', 'p2'), parse('players', 'p2')], values=dict(settings=dict(devMode=True, beta=False)),
               extra=EXTRA), None
    yield case('framework-null-settings-known', [parse('kick nobody', 'p2'), parse('bank', 'p2')], values=dict(settings=None), extra=EXTRA), None
    yield case('framework-cheater', [parse('kick nobody', 'p2')], values=dict(cheater=True)), None
    yield case('framework-content', [parse('loc a  b', 'p1'), parse('loc', 'p1')], extra=EXTRA,
               answers={'contentReject:core.game.system.command.sets.MiscCommandSet$defineCommands$5': [False, True]}), 0
    yield case('framework-bank-silly', [parse('bank', 'p2')], extra=EXTRA), 0
    yield case('framework-monitor', [cmd('kick bob'), cmd('nosuch 1 2', 'p2')], answers={'byName:bob': 'p2'}), 0
    for name, data in [('plain', b'kick bob\x00'), ('upper', b'KiCk BoB\x00'), ('high', b'tele \xc0\xff\x80\x00'), ('short', b'k'),
                       ('empty', b''), ('one', b'\x00'), ('two', b'k\x00'), ('unterminated', b'kick bob'), ('trailing', b'nosuch\x00junk')]:
        yield case(f'framework-packet-{name}', [packet(44, data)], answers={'byName:bob': 'p2'}), (0 if name == 'plain' else None)


def moderation():
    on = {'byName:bob': 'p2', 'taken:bob': True}
    yield case('moderation-kick', [parse('kick bob'), parse('kick nobody'), parse('kick')], answers={'byName:bob': 'p2'}), 0
    for duration in ['30d', '5h', '10m', '15s', '1d2h', '12', '2w', 'x', 'd', '99999999999d', '7d!']:
        yield case(f'moderation-ban-{duration}', [parse(f'ban bob {duration}'), dict(op='pulse', index=0)], answers=dict(on, **{'info:bob': 'bob'})), (
            0 if duration == '30d' else None)
    yield case('moderation-ban-offline', [parse('ban dave 1h'), dict(op='pulse', index=0)], answers={'taken:dave': True, 'info:dave': 'dave'},
               accts=accounts(dave=dict(username='dave'))), 1
    yield case('moderation-ban-invalid', [parse('ban ghost 1d'), parse('ban'), parse('ban bob')], answers=on), None
    yield case('moderation-ipban', [parse('ipban 1.2.3.4 2d'), dict(op='pulse', index=0), dict(op='pulse', index=1)],
               answers={'ips:1.2.3.4': ['bob', 'dave'], 'byName:bob': 'p2', 'info:bob': 'bob', 'info:dave': 'dave'},
               accts=accounts(dave=dict(username='dave'))), 0
    yield case('moderation-ipban-none', [parse('ipban 9.9.9.9 1d'), parse('ipban 1.1.1.1 q')], answers={'ips:1.1.1.1': ['bob']}), None
    yield case('moderation-ipban-null-name', [parse('ipban 5.5.5.5 1m'), dict(op='pulse', index=0)], answers={'ips:5.5.5.5': [None]}), None
    yield case('moderation-mute-online', [parse('mute bob 48h')], answers=on), 0
    yield case('moderation-mute-offline', [parse('mute dave 3d'), dict(op='pulse', index=0)], answers={'taken:dave': True, 'info:dave': 'dave'},
               accts=accounts(dave=dict(username='dave'))), 1
    yield case('moderation-mute-invalid', [parse('mute ghost 1d'), parse('mute bob'), parse('mute bob 1y1')], answers=on), None
    yield case('moderation-jail', [parse('jail 60 bob'), dict(op='pulse', index=0), dict(op='pulse', index=0), dict(op='pulse', index=0)],
               answers={'byName:bob': 'p2'}, values=dict(clock=[1000000, 1000000, 1030000, 1070000])), 0
    yield case('moderation-jail-at-jail', [parse('jail 5 bob'), dict(op='pulse', index=0)], answers={'byName:bob': 'p2'},
               people=players(p2=dict(name='bob', username='Bob', account='bob', location=[3228, 3407, 0])),
               values=dict(clock=[1000000, 1005000])), 1
    yield case('moderation-jail-null-location', [parse('jail -5 bob'), dict(op='pulse', index=0)], answers={'byName:bob': 'p2'},
               people=players(p2=dict(name='bob', username='Bob', account='bob', location=None))), None
    yield case('moderation-jail-errors', [parse('jail 60'), parse('jail x bob'), parse('jail 1801 bob'), parse('jail 60 no body'),
                                          parse('jail 60 alice'), parse('jail 2147483648 bob')],
               answers={'byName:no_body': None, 'byName:alice': 'p1', 'byName:bob': 'p2'}), None
    yield case('moderation-jail-spaces', [parse('jail 10 bob the  builder')], answers={'byName:bob_the__builder': 'p2'}), None
    yield case('moderation-modcr', [parse('modcr bob 50'), parse('modcr BOB -20'), parse('modcr dave 5'), parse('modcr erin 5')],
               answers={'byName:bob': 'p2', 'info:dave': 'dave'}, accts=accounts(dave=dict(username='dave', credits=7))), 0
    yield case('moderation-modcr-errors', [parse('modcr'), parse('modcr bob'), parse('modcr bob x'), parse('modcr bob -2147483648'),
                                           parse('modcr bob 0')], answers={'byName:bob': 'p2'}), None
    yield case('moderation-modcr-offline', [parse('modcr dave 3')], answers={'info:dave': 'dave'}, accts=accounts(dave=dict(username='dave'))), 0
    yield case('moderation-csvmodcr', [parse('csvmodcr'), parse('csvmodcr none.csv'), dict(op='job', index=0), parse('csvmodcr list.csv'),
                                       dict(op='job', index=1)],
               answers={'byName:bob': 'p2', 'info:dave': 'dave'}, accts=accounts(dave=dict(username='dave', credits=1)),
               files={'list.csv': '# comment\nBob ,5\n dave,  -2\n'}), 4
    yield case('moderation-csvmodcr-reject', [parse('csvmodcr bad.csv'), dict(op='job', index=0)], answers={'byName:bob': 'p2'},
               files={'bad.csv': 'bob,x\nbob,1\n'}), None
    yield case('moderation-csvmodcr-short', [parse('csvmodcr short.csv'), dict(op='job', index=0)], answers={'byName:bob': 'p2'},
               files={'short.csv': 'bob,1\nbob\n'}), None
    yield case('moderation-csvmodcr-nodata', [parse('csvmodcr x.csv'), dict(op='job', index=0)], values=dict(dataPath=None)), None
    yield case('moderation-rights', [parse('kick bob', 'p3'), parse('ban bob 1d', 'p3'), parse('mute bob 1d', 'p3'), parse('kick bob', 'p2')],
               answers=on), None


def system():
    yield case('system-update', [parse('update'), parse('update 30'), parse('update x'), parse('cancelupdate')]), 1
    yield case('system-resetpassword', [parse('resetpassword', 'p2'), dict(op='input', index=0, value='hunter2'), dict(op='task', index=0),
                                        dict(op='input', index=1, value='hunter2'), dict(op='input', index=1, value='abcd'),
                                        dict(op='input', index=1, value='a' * 21), dict(op='input', index=1, value='bob'),
                                        dict(op='input', index=1, value='newpass1'), dict(op='input', index=1, value=12345)],
               answers={'checkPassword': True}, values=dict(playerCommands=True)), 7
    yield case('system-resetpassword-wrong', [parse('resetpassword', 'p2'), dict(op='input', index=0, value=1234), dict(op='task', index=0)],
               answers={'checkPassword': False}, values=dict(playerCommands=True)), 2
    yield case('system-resetpassword-name', [parse('resetpassword', 'p2'), dict(op='input', index=0, value='x'), dict(op='task', index=0),
                                             dict(op='input', index=1, value='bobby')],
               accts=accounts(bob=dict(username='bobby', password='x')), answers={'checkPassword': True}, values=dict(playerCommands=True)), None
    yield case('system-setpasswordother', [parse('setpasswordother'), parse('setpasswordother ghost newpass'), parse('setpasswordother bob abc'),
                                           parse('setpasswordother bob ' + 'b' * 21), parse('setpasswordother bobby1 bobby1'),
                                           parse('setpasswordother bob goodpass')],
               answers={'taken:bob': True, 'taken:bobby1': True}), 5
    defs = {'itemDefs': 15000, 'itemName:4151': 'Abyssal whip', 'itemName:995': 'Coins'}
    yield case('system-giveitem', [parse('giveitem bob 4151'), parse('giveitem bob 995 1000'), parse('giveitem bob 995 x')],
               answers=dict(defs, **{'byName:bob': 'p2', 'isFull:p2:inventory': [False, True, False]})), 1
    yield case('system-giveitem-errors', [parse('giveitem'), parse('giveitem bob'), parse('giveitem ghost 1'), parse('giveitem bob x'),
                                          parse('giveitem bob 0'), parse('giveitem bob 15001'), parse('giveitem bob 15000 0'),
                                          parse('giveitem bob 5 -1'), parse('giveitem bob 1 2 3 4')],
               answers=dict(defs, **{'byName:bob': 'p2'})), None
    for loc in ['i', 'inv', 'inventory', 'b', 'bk', 'bank', 'e', 'equip', 'EQUIPMENT', 'x']:
        yield case(f'system-removeitem-{loc}', [parse(f'removeitem {loc} bob 4151 3'), parse(f'removeitem {loc} bob 4151')],
                   answers=dict(defs, **{'byName:bob': 'p2', 'getItem:p2:inventory:4151': 2, 'getItem:p2:bank:4151': 9,
                                         'getItem:p2:equipment:4151': 1})), (0 if loc == 'inv' else None)
    yield case('system-removeitem-errors', [parse('removeitem'), parse('removeitem i ghost 1'), parse('removeitem i bob x'),
                                            parse('removeitem i bob 1 0'), parse('removeitem i bob 7 1')],
               answers=dict(defs, **{'byName:bob': 'p2'})), None
    yield case('system-removeitemall', [parse('removeitemall bob 4151')],
               answers=dict(defs, **{'byName:bob': 'p2', 'noteChange:4151': 4152, 'amount:p2:inventory:4151': 2, 'amount:p2:bank:4151': 5,
                                     'amount:p2:equipment:4151': 1, 'amountItem:p2:bank:4152': 10, 'amountItem:p2:equipment:4152': 0})), 0
    yield case('system-removeitemall-none', [parse('removeitemall bob 4151'), parse('removeitemall'), parse('removeitemall ghost 1'),
                                             parse('removeitemall bob 0')], answers=dict(defs, **{'byName:bob': 'p2'})), None
    yield case('system-potato', [parse('potato'), parse('potato', 'p2')]), 0
    slots = {'slots:p1:inventory': [None, [4151, 0]], 'slots:p1:equipment': [None, None, None, [4718, 250]],
             'slots:p1:bankPrimary': [[1704, 0]], 'slots:p1:bankSecondary': [[11283, 20]]}
    charge = dict(defs, **slots, **{'itemName:4718': "Dharok's greataxe", 'itemName:1704': 'Amulet of glory', 'itemName:11283': 'Dragonfire shield',
                                    'chargedItem:1704': True, 'chargedCharge:1704': 0, 'forCharge:1704:4': 1712, 'chargedCharge:1712': 4})
    yield case('system-charge-get', [parse('charge'), parse('charge 4151'), parse('charge weapon'), parse('charge 1704 d'), parse('charge 11283 d'),
                                     parse('charge head'), parse('charge nothing'), parse('charge -1'), parse('charge 15000'), parse('charge 999')],
               answers=charge), 3
    yield case('system-charge-set', [parse('charge weapon s 500'), parse('charge weapon s 0'), parse('charge weapon s'), parse('charge weapon s x'),
                                     parse('charge 1704 sd 4'), parse('charge 1704 ds 4'), parse('charge 4151 sd 4'), parse('charge 4151 q')],
               answers=charge), 4


def teleport():
    names = ['corp', 'corporal', 'corporeal', 'pc', 'pest control', 'pest', 'al kharid', 'alkharid', 'kharid', 'lumbridge', 'lumby',
             'wizard tower', 'wizards tower', 'tower', 'wizards', 'draynor', 'draynor village', 'port sarim', 'sarim', 'rimmington', 'fally',
             'falador', 'taverley', 'barbarian village', 'barb', 'varrock', 'grand exchange', 'ge', 'karamja', 'tzhaar', 'brimhaven',
             'shilo village', 'shilo', 'yanille', 'ardougne', 'ardy', 'gnome stronghold', 'gnome', 'camelot', 'cammy', 'seers', 'catherby',
             'rellekka', 'trollheim', 'godwars', 'gwd', 'god wars', 'bounty hunter', 'bh', 'clan wars', 'clw', 'mage arena', 'mage',
             'magearena', 'arena', 'king black dragon', 'kbd', 'digsite', 'canifis', 'slayer tower', 'slayer', 'kalphite queen', 'kq',
             'kalphite hive', 'kalphite', 'pyramid', 'nardah', 'uzer', 'pollnivneach', 'poln', 'sophanem', 'burthorpe', 'burthorp', 'edge',
             'edgeville', 'bedabin', 'barrows', 'bkf', 'black knights fortress', 'monastery', 'blast furnace', 'blast', 'zanaris',
             'ectofuntus', 'ecto', 'tower of life lower', 'test area', 'quest the golem 1', 'dorgeshuun', 'dorg', 'brine rats', 'piscatoris',
             'fishing trawler', 'trawler', 'mountain camp', 'clocktower', 'waterbirth', 'dks', 'cwars', 'castle wars', 'castle war',
             'castlewars', 'castlewar', 'castle']
    yield case('teleport-to-all', [parse('to ' + n) for n in names]), None
    yield case('teleport-to-unknown', [parse('to'), parse('to atlantis'), parse('to  lumbridge'), parse('to Lumbridge')]), 0
    yield case('teleport-to-refused', [parse('to varrock')], answers={'teleport': False}), 0
    yield case('teleport-tele', [parse('tele 3222 3218'), parse('tele 3222 3218 1'), parse('tele 1,50,50,10,20'), parse('tele 50_50'),
                                 parse('tele 1_50_50'), parse('tele 50_50_3_4'), parse('tele 2_50_50_3_4'), parse('tele 1_2_3_4_5_6'),
                                 parse('tele 1,2,3'), parse('tele 1,a,3,4,5'), parse('tele a_b'), parse('tele'), parse('tele 1'),
                                 parse('tele x 2'), parse('tele 1 2 x'), parse('tele 1,2,3,4,5,'), parse('tele -1 -2 -3'),
                                 parse('tele 3200 3200 0 9'), parse('tele 1_2,3')]), 0
    tele = {'byName:bob': 'p2', 'byName:carol': 'p3', 'byName:': None}
    yield case('teleport-teleto', [parse('teleto bob'), parse('teletome bob'), parse('teleto'), parse('teleto carol'), parse('teletome carol')],
               answers=tele, people=players(p3=dict(name='carol', username='Carol', account='carol', location=None,
                                                    attributes={'fc_wave': {'int': 3}}))), 0
    yield case('teleport-teleto-spaces', [parse('teleto bob the builder'), parse('teletome no one')], answers={'byName:bob_the_builder': 'p2'}), None
    yield case('teleport-home', [parse('home'), parse('home', 'p2')]), 0
    yield case('teleport-home-null', [parse('home')], values=dict(home=None)), None


def attributes():
    people = players(p2=dict(name='bob', username='Bob', account='bob', location=[3200, 3200, 0],
                             attributes={'i': {'int': 5}, 'f': {'float': '1.5'}, 'b': {'bool': True}, 't': {'text': 'hello'},
                                         'r': {'rights': 'PLAYER_MODERATOR'}, 'n': None}))
    yield case('attributes-get', [parse('getattribute'), parse('getattribute missing'), parse('getattribute bob i'), parse('getattribute bob f'),
                                  parse('getattribute bob b'), parse('getattribute bob t'), parse('getattribute bob r'), parse('getattribute bob n'),
                                  parse('getattribute bob missing'), parse('getattribute ghost i'), parse('getattribute a b c')],
               answers={'byName:bob': 'p2'}, people=people), 2
    values = ['5', '-7', '+3', '2147483648', '1.5', '1e3', '2f', '0x10', 'NaN', 'Infinity', '-0.0', '.5', '1d', 'true', 'false', 'TRUE', 'hello', '']
    ops = [parse(f'setattribute key {v}') for v in values] + [parse(f'setattribute bob key {v}') for v in ['9', 'false', 'x']]
    ops += [parse('setattribute'), parse('setattribute key'), parse('setattribute ghost key 1'), parse('setattribute a b c d e')]
    yield case('attributes-set', ops, answers={'byName:bob': 'p2'}), 0


def abuse():
    base = {'taken:bob': True}
    yield case('abuse-plain', [report('bob', 3, False)], answers=base), 0
    yield case('abuse-rules', [report('bob', r, False) for r in [0, 1, 6, 12, 13, -1, 127]], answers=base), None
    yield case('abuse-self', [report('alice', 2, False)], answers={'taken:alice': True}), 0
    yield case('abuse-invalid', [report('ghost', 1, False), report('', 1, False, raw=0), report('x', 1, False, raw=-5),
                                 report('x', 1, False, raw=37 ** 12)], answers=base), None
    yield case('abuse-mute', [report('bob', 0, True)], answers=dict(base, **{'byName:bob': 'p2'})), 0
    yield case('abuse-mute-offline', [report('bob', 0, True), dict(op='pulse', index=0)], answers=dict(base, **{'info:bob': 'bob'})), 0
    yield case('abuse-mute-rights', [report('bob', 0, True, 'p3'), report('bob', 0, True, 'p2')], answers=dict(base, **{'byName:bob': 'p2'})), None
    yield case('abuse-mute-reject', [report('bob', 0, True)], answers={'taken:bob': [True, False]}), None
    yield case('abuse-mute-commands-on', [report('bob', 0, True, 'p2')], answers=base, values=dict(playerCommands=True)), None
    yield case('abuse-short', [packet(99, b'\x00\x00\x00'), packet(99, b'\x00' * 9)]), None
    yield case('abuse-name-digits', [report('mod_2009', 5, False)], answers={'taken:mod_2009': True}), None


def phrase(text):
    return text.encode('latin-1').hex() + '00'


def qc(data, player='p1', handler=False):
    return dict(op='qcHandler' if handler else 'packet', opcode=167, hex=hexed(bytes(data)), player=player)


def quick_chat():
    standard = {'qcFile:3': phrase('Hello there!'), 'qcFile:260': phrase('I need 1 more xp'), 'qcFile:0': phrase('Hi'),
                'qcFile:255': phrase('Bye'), 'qcFile:511': phrase('Bye2'), 'qcFile:767': phrase('Bye3'), 'qcFile:1023': phrase('Bye4')}
    yield case('qc-standard', [qc([0, 0, 3]), dict(op='pulse', index=0), qc([0, 0, 3, 9]), qc([0, 1, 4])], answers=standard), 1
    yield case('qc-negative-offsets', [qc([0, 0, 0xFF]), qc([0, 1, 0xFF]), qc([0, 2, 0xFF]), qc([0, 3, 0xFF]), qc([0, 0x80, 0xFF])],
               answers=standard), None
    yield case('qc-clan', [qc([1, 0, 3])], answers=dict(standard, **{'clan:p1': 'Some Owner'})), 0
    yield case('qc-clan-none', [qc([1, 0, 3]), qc([2, 0, 3])], answers=standard), None
    yield case('qc-sizes', [qc([0, 0]), qc([0, 0, 3, 1, 2, 3]), qc([0, 0, 3, 1, 2, 3, 4, 5]), qc([]), qc([0, 0, 3, 0, 1, 0])],
               answers=standard), None
    yield case('qc-missing', [qc([0, 0, 50]), qc([0, 0, 51])], answers={'qcFile:51': '414243'}), None
    xp = {'qcFile:10': phrase('I need < xp to get my next Mining level.'), 'qcFile:11': phrase('My Woodcutting level is <.'),
          'qcFile:12': phrase('I need < xp to get my next Sailing level.'), 'qcFile:13': phrase('My Summoning level is <, my level is <'),
          'experience:p1:14': '1500.5', 'staticLevel:p1:14': 10, 'staticLevel:p1:8': 42, 'staticLevel:p1:23': 7}
    yield case('qc-skills', [qc([0, 0, 10]), dict(op='pulse', index=0), qc([0, 0, 11]), dict(op='pulse', index=1), qc([1, 0, 13])],
               answers=dict(xp, **{'clan:p1': 'Owner'})), 0
    yield case('qc-skills-unknown', [qc([0, 0, 12])], answers=xp), None
    slayer = {'qcFile:20': phrase('My current Slayer assignment is complete.')}
    yield case('qc-slayer', [qc([1, 0, 20]), qc([1, 0, 20]), qc([0, 0, 20]), dict(op='pulse', index=0)],
               answers=dict(slayer, **{'slayerAmount:p1': [12, 0, 5], 'slayerTask:p1': ['COWS', 'GOBLINS', None], 'clan:p1': 'Owner'})), 0
    items = {'qcFile:30': phrase('Can I buy your <?'), 'qcFile:31': phrase('I have an item.'), 'qcFile:32': phrase('Would you like to borrow <?'),
             'itemDefName:4151': 'Abyssal whip', 'itemDefName:-1': None}
    yield case('qc-item', [qc([1, 0, 30, 0x10, 0x37]), qc([1, 0, 31, 0x10, 0x37]), qc([1, 0, 32, 0xFF, 0xFF]),
                           qc([1, 0, 30, 0, 5]), qc([0, 0, 30, 0x10, 0x37]), dict(op='pulse', index=0)],
               answers=dict(items, **{'itemDefExists:5': False, 'clan:p1': 'Owner'})), 0
    single = [('I\'d like the loan duration to be <', 1642), ('Let\'s go to Agility course: <', 1505), ('Try training your Agility at: <', 1505),
              ('Try training on <', 1524), ('Try ranging <', 1525), ('Make a flat-pack <', 1490), ('I\'m cooking <', 1492),
              ('Would you please cook me <', 1492), ('Try cooking <', 1492), ('I am crafting <', 1493), ('Try crafting <', 1493),
              ('Would you please craft me <', 1493), ('I\'m growing crop <', 1494), ('Try growing crop <', 1494), ('I\'m burning logs <', 1495),
              ('Try burning logs at <', 1544), ('I\'m fishing <', 1491), ('Would you please fish me <', 1491), ('Try fishing for <', 1491),
              ('I\'m fletching <', 1496), ('Try fletching <', 1496), ('Would you please fetch me <', 1496), ('I\'m mixing potion <', 1526),
              ('Would you please mix me a potion <', 1526), ('Try mixing potion <', 1526), ('Where can I get the secondary ingredient <', 1516),
              ('Would you please hunt me <', 1543), ('Try hunting <', 1543), ('I\'m casting spell <', 1527), ('Would you please cast <', 1527),
              ('I am on spell book <', 1528), ('I\'m mining ore <', 1529), ('I\'m using a pick <', 1531), ('Try mining at <', 1533),
              ('Use your prayer <', 1534), ('I\'m going to craft rune <', 1535), ('Try crafting runes at <', 1536),
              ('You should use the Slayer master in <', 2139), ('Do you have spare Slayer equipment <', 1538), ('I like the familiar <', 1539),
              ('I can summon up to <', 1539), ('Good charm droppers are <', 1499), ('Try thieving from <', 1540),
              ('I\'m using a woodcutting axe made <', 1532), ('Try training Woodcutting at <', 1500), ('Nice level in < <', 1517),
              ('No selection here <', None)]
    answers, ops = {}, []
    for i, (text, data_map) in enumerate(single):
        answers[f'qcFile:{100 + i}'] = phrase(text)
        if data_map is not None:
            answers[f'dataMap:{data_map}:{i}'] = f'value{i}'
        ops.append(qc([1, 0, 100 + i, 0, i]))
    answers['clan:p1'] = 'Owner'
    yield case('qc-single-maps', ops, answers=answers), None
    yield case('qc-single-errors', [qc([0, 0, 100, 0, 77]), qc([0, 0, 101, 0, 78])],
               answers={'qcFile:100': phrase(single[0][0]), 'qcFile:101': phrase(single[1][0]), 'dataMap:1505:78': 9}), None
    double = {'qcFile:40': phrase('That is < of <.'), 'qcFile:41': phrase('I am smithing < <'), 'qcFile:42': phrase('Try smithing < bars <'),
              'qcFile:43': phrase('Would you please smith me < <'), 'qcFile:44': phrase('Nothing < here'),
              'dataMap:1502:1': 'north', 'dataMap:1504:3': 'Varrock', 'dataMap:1530:2': 'bronze', 'dataMap:1498:4': 'daggers'}
    yield case('qc-double', [qc([1, 0, 40, 0, 1, 0, 3]), qc([1, 0, 41, 9, 2, 9, 4]), qc([1, 0, 42, 0, 2, 0, 4]), qc([1, 0, 43, 0, 2, 0, 4]),
                             qc([1, 0, 44, 0, 2, 0, 4]), qc([1, 0, 40, 0, 1, 0, 9]), qc([0, 0, 40, 0, 1, 0, 3]), dict(op='pulse', index=0)],
               answers=dict(double, **{'clan:p1': 'Owner'})), 0
    yield case('qc-handler', [qc([0, 0, 3], handler=True), qc([1, 0, 3, 7, 7, 7], handler=True), qc([0, 0], handler=True)],
               answers=dict(standard, **{'clan:p1': 'Owner'}), values=dict(settings=dict(devMode=True, beta=False))), 0
    yield case('qc-handler-live', [qc([0, 0, 3], handler=True)], answers=standard), None
    yield case('qc-icon', [qc([0, 0, 3], 'p2'), qc([0, 0, 3], 'p3'), dict(op='pulse', index=1)], answers=standard,
               people=players(p3=dict(name='carol', username='Carol', account='carol', location=[1, 1, 0],
                                      attributes={'visible_rank': {'rights': 'REGULAR_PLAYER'}}))), None
    yield case('qc-bad-rights', [qc([1, 0, 3], 'p2'), qc([0, 0, 3], 'p2')], answers=dict(standard, **{'clan:p2': 'x'}),
               accts=accounts(bob=dict(username='bob', rights=5))), None


def cases():
    for group in [framework, moderation, system, teleport, attributes, abuse, quick_chat]:
        yield from group()


def groups(rows):
    out = {}
    for row in rows:
        out[row['group']] = out.get(row['group'], 0) + 1
    return dict(sorted(out.items()))


# Records the original cannot fail at; failure injection skips them.
NOTES = ('clock', 'decoded')

NEEDED_EVENTS = ['message', 'chat', 'monitor', 'zone', 'legacy', 'content', 'decoded', 'byName', 'clear', 'taken', 'accountInfo',
                 'accountUpdate', 'ips', 'store', 'submit', 'clock', 'teleport', 'teleportLocation', 'getAttribute', 'apiGetAttribute',
                 'apiSetAttribute', 'apiMessage', 'packetMessage', 'discord', 'input', 'runTask', 'dialogue', 'checkPassword',
                 'updatePassword', 'launch', 'countdown', 'flag', 'cancelUpdate', 'itemDefs', 'itemName', 'noteChange', 'isFull', 'add',
                 'remove', 'getItem', 'amount', 'amountItem', 'slots', 'get', 'setCharge', 'replace', 'chargedItem', 'forCharge',
                 'chargedCharge', 'qcFile', 'dataMap', 'itemDef', 'experience', 'staticLevel', 'slayerAmount', 'slayerTask', 'setAttribute',
                 'publish', 'registerChat', 'log']
