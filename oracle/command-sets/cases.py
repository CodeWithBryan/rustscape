"""The command-sets corpus: authored cases for the Misc, Spawn and Stats command sets, the ::update/::cancelupdate/
::shutdown paths and the system manager.

Each case is a small world (accounts, players with their saved data and attributes, case values and service
answers) and a list of operations; cases() yields (case, sweep) where sweep is the index of an operation whose every
recorded call is then made to fail in turn.
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
ADMIN, MOD, REGULAR = 2, 1, 0
NOTES = ('clock', 'println', 'printStackTrace')

MISC = ['debug', 'calcaccuracy', 'anmacs', 'setsaveversion', 'loc', 'pos', 'coords', 'calcmaxhit', 'calcdefence', 'empty', 'announce',
        'players', 'botinfo', 'shop', 'ge', 'commands', 'commandsearch', 'reply', 'xface', 'completediaries', 'permadeath', 'log',
        'logdone', 'rolltrawlerloot', 'fillbank', 'emptybank', 'setconfig', 'getobjectvarp', 'define_varbit', 'setvarbit',
        'setvarbits', 'getvarbit', 'setvarp', 'setvarc', 'grow', 'finishbins', 'resetbins', 'diseasecrops', 'addcredits',
        'getnpcparent', 'infinitespecial', 'infinitespecialmove', 'allowaggro', 'rules', 'confirmrules', 'setplayerstrong',
        'setplaqueread', 'brewkit', 'brew']
SPAWN = ['npc', 'item', 'items', 'object', 'objectgrid', 'removeobject', 'objects']
STATS = ['stats']
SYSTEM = ['update', 'cancelupdate', 'shutdown']
PORTED = MISC + SPAWN + STATS + SYSTEM
OWN = set(PORTED) | {'resetpassword', 'setpasswordother', 'giveitem', 'removeitem', 'removeitemall', 'potato', 'charge'}

NEEDED_EVENTS = ('message', 'apiMessage', 'packetMessage', 'sendString', 'open', 'close', 'closeDialogue', 'input', 'launch',
                 'openBook', 'pageSetup', 'setAttribute', 'getAttribute', 'removeAttribute', 'hasAttribute', 'news', 'privateMessage',
                 'setVarp', 'setVarbit', 'getVarbit', 'varc', 'interfaceConfig', 'newNpc', 'init', 'newScenery', 'sceneryAdd',
                 'sceneryRemove', 'chunkObjects', 'systemUpdate', 'submit', 'workerStart', 'workerStop', 'serverRunning',
                 'reactorTerminate', 'saveDetails', 'clear', 'persistSave', 'shutdownListener', 'monitorFlush', 'queueUpdate',
                 'queueClear', 'log', 'accuracy', 'defence', 'hit', 'finishTask', 'permadeath', 'trawlerLoot', 'brew', 'binFinish',
                 'binReset', 'nextGrowth', 'diseaseMod', 'cropRun', 'kills', 'rareDrops', 'clues', 'addItemOrDrop', 'maximumAdd',
                 'add', 'refresh', 'update', 'unlock', 'dialogue', 'cutscene', 'setVersion', 'println', 'printStackTrace')


def accounts(**extra):
    out = {
        'alice': dict(username='alice', rights=ADMIN, password='secret1', credits=40),
        'bob': dict(username='bob', rights=REGULAR, password='hunter2'),
        'carol': dict(username='carol', rights=MOD, password='pa55word'),
    }
    out.update(extra)
    return out


BOSSES = [3, 0, 7, 8, 9, 11, 2, 13, 14, 15, 16, 4, 5, 6, 1, 0]


def players(**extra):
    out = {
        'p1': dict(name='alice', username='Alice', account='alice', location=[3222, 3218, 0], direction='EAST', lastSceneGraph=[3176, 3168, 0],
                   version=2, testStage=1, readPlaques=[True, False, True], bossCounters=BOSSES, barrowsLoots=12, diaries=[[2, 1], [1]]),
        'p2': dict(name='bob', username='Bob', account='bob', location=[3200, 3200, 0], direction='NORTH_WEST', lastSceneGraph=[3152, 3152, 0],
                   readPlaques=[True, True], bossCounters=BOSSES, diaries=[]),
        'p3': dict(name='carol', username='Carol', account='carol', location=[3100, 3100, 1], lastSceneGraph=[3048, 3048, 1],
                   readPlaques=[], bossCounters=[], diaries=[[0]]),
    }
    out.update(extra)
    return out


def case(name, ops, answers=None, values=None, accts=None, people=None, extra=None, group=None):
    v = dict(playerCommands=True, cheater=False, settings=dict(devMode=False, beta=False), home=[3222, 3218, 0], dataPath='data',
             clock=[1000000, 2000000, 3000000], online=['p1', 'p2', 'p3'])
    v.update(values or {})
    return dict(name=name, group=group or name.split('-')[0], values=v, accounts=accts or accounts(), players=people or players(),
                answers=answers or {}, extra=extra or [], ops=ops)


def parse(line, player='p1'):
    return dict(op='parse', player=player, line=line)


def registry_extra():
    data = json.loads((ROOT / 'fixtures/commands/registry.json').read_text(encoding='utf-8'))
    out = []
    for name, _, privilege, usage, description, origin in data['registry']:
        if name in OWN:
            continue
        cls, lam = origin.split('$defineCommands$')
        out.append([name, privilege, cls, int(lam), usage, description])
    return out


FULL = registry_extra()


def misc():
    yield case('debug-toggle', [parse('debug', 'p2'), parse('debug', 'p2'), parse('debug x y', 'p2')]), 0
    yield case('debug-preset', [parse('debug', 'p1')], people=players(p1=dict(players()['p1'], attributes={'debug': {'bool': True}}))), None
    yield case('debug-null', [parse('debug', 'p1')], people=players(p1=dict(players()['p1'], attributes={'debug': None}))), None
    yield case('debug-denied', [parse('debug', 'p2')], values=dict(playerCommands=False)), None
    # calcaccuracy
    yield case('calcaccuracy-self', [parse('calcaccuracy', 'p2')], answers={'swingHandler': 'range', 'accuracy': 77}), 0
    yield case('calcaccuracy-npc', [parse('calcaccuracy 50'), parse('calcaccuracy 50 9', 'p2')],
               answers={'accuracy': [1200, 40], 'defence': [300, 900], 'npcName': 'King Black Dragon', 'level': 240, 'swingHandler': 'magic'}), 0
    yield case('calcaccuracy-equal', [parse('calcaccuracy 1')], answers={'accuracy': 500, 'defence': 500}), None
    # accuracy == defence: both formulas agree in exact arithmetic but not in doubles for 2.
    yield case('calcaccuracy-equal-2', [parse('calcaccuracy 1'), parse('calcaccuracy 1')], answers={'accuracy': [2, 6], 'defence': [2, 6]}), None
    yield case('calcaccuracy-zero', [parse('calcaccuracy 1'), parse('calcaccuracy 2')], answers={'accuracy': [0, -3], 'defence': [0, 7]}), None
    yield case('calcaccuracy-bad', [parse('calcaccuracy x'), parse('calcaccuracy 99999999999')]), None
    # calcmaxhit
    for label in ['melee', 'range', 'magic', 'ma_melee', 'ma_ranged', 'ra_melee']:
        yield case(f'calcmaxhit-{label}', [parse('calcmaxhit', 'p2')], answers={'swingHandler': label, 'hit': 33}), (0 if label == 'melee' else None)
    # calcdefence
    yield case('calcdefence-self', [parse('calcdefence', 'p2')], answers={'accuracy': [1000, 250, 50], 'defence': [400, 400, 1000]}), 0
    yield case('calcdefence-npc', [parse('calcdefence 50'), parse('calcdefence 50 ranged'), parse('calcdefence 50 MA_MELEE')],
               answers={'npcName': 'King Black Dragon', 'accuracy': [3000, 60, 600, 1234, 7], 'defence': [100, 2000, 700, 801, 12]},
               values={'bonuses:npc0': [1, 1]}), 0
    for name, bonuses in [('first', [10, 40, 40, 0, -5, 99]), ('negative', [-5, -9, -2, -9, -8]), ('empty', []), ('short', [3, 9]),
                          ('min', [-2147483648, -2147483648, -2147483648])]:
        yield case(f'calcdefence-bonuses-{name}', [parse('calcdefence 2 melee')], values={'bonuses:npc0': bonuses},
                   answers={'accuracy': 10, 'defence': 10}), None
    yield case('calcdefence-rounding', [parse('calcdefence 1 magic'), parse('calcdefence 1 melee'), parse('calcdefence 1 range')],
               answers={'accuracy': [7, 1, 3], 'defence': [3, 1, 2]}), None
    for a, d in [(1, 2), (3, 7), (999, 1000), (2, 1), (12345, 9876), (0, 0), (1, 0), (2147483647, 1), (5, 2147483647), (-1, 5)]:
        yield case(f'calcdefence-chance-{a}-{d}', [parse('calcdefence 3 melee')], answers={'accuracy': a, 'defence': d}), None
    yield case('calcdefence-infinite', [parse('calcdefence 3 melee'), parse('calcdefence 3 magic')], answers={'accuracy': [-3, 0], 'defence': [-1, -1]}), None
    yield case('calcdefence-nan', [parse('calcdefence 3 melee')], answers={'accuracy': 0, 'defence': -1}), None
    yield case('calcdefence-bad', [parse('calcdefence x'), parse('calcdefence 1 sword'), parse('calcdefence 1 ranged extra')]), None
    yield case('calcdefence-ranged-key', [parse('calcdefence 1 RANGED')]), None
    # loc / pos / coords
    yield case('loc-plain', [parse('loc', 'p2')], answers={'object': None, 'teleportPermitted': True, 'clippingFlag': 256, 'landscape': False,
                                                           'regionActive': True}), 0
    yield case('loc-object', [parse('loc', 'p1')], answers={'object': [[1530, [3222, 3218, 0], 0, 1, False, True, True]], 'sceneryConfigFile': 900,
                                                            'varbitVarp': 33, 'varbitStart': 4, 'dynamic': True}), 0
    yield case('loc-object-no-config', [parse('loc', 'p3')], answers={'object': [[1530, [3100, 3100, 1], 10, 0, False, True, True], None, None]}), None
    yield case('pos-coords', [parse('pos', 'p2'), parse('coords', 'p3'), parse('pos extra', 'p1')]), 0
    yield case('anmacs', [parse('anmacs')]), 0
    # setsaveversion
    yield case('setsaveversion', [parse('setsaveversion 7'), parse('setsaveversion x'), parse('setsaveversion'), parse('setsaveversion -3')]), 0
    # empty / announce / addcredits / fillbank / emptybank
    yield case('empty', [parse('empty'), parse('empty all')]), 0
    yield case('announce', [parse('announce hello  world'), parse('announce'), parse('announce <col=ff0000>X')]), 0
    yield case('addcredits', [parse('addcredits'), parse('addcredits 5', 'p3')], values=dict(playerCommands=False)), 0
    yield case('addcredits-wrap', [parse('addcredits')], accts=accounts(alice=dict(username='alice', rights=ADMIN, credits=2147483600))), None
    yield case('fillbank', [parse('fillbank')], values=dict(bankSize=4), answers={'add:p1:bank:2': False}), 0
    yield case('fillbank-empty', [parse('fillbank')], values=dict(bankSize=0)), None
    yield case('emptybank', [parse('emptybank')]), 0
    # players
    yield case('players-mod', [parse('players', 'p3')], answers={'artificial:p2': True, 'ipAddress': '10.0.0.1', 'compName': 'desk'}), (0, 12)
    yield case('players-admin', [parse('players', 'p1')],
               people=players(p2=dict(players()['p2'], attributes={'visible_rank': {'rights': 'ADMINISTRATOR'}}),
                              p3=dict(players()['p3'], attributes={'visible_rank': {'rights': 'REGULAR_PLAYER'}}))), None
    yield case('players-regular', [parse('players', 'p2')], values=dict(online=['p2'])), None
    for name, answers in [('opened', {'isOpened': True, 'opened': 300}), ('journal', {'isOpened': True, 'opened': 275}),
                          ('moving', {'movementLocked': True}), ('teleport', {'teleportLocked': True})]:
        yield case(f'players-busy-{name}', [parse('players', 'p1')], answers=answers), None
    # botinfo
    yield case('botinfo', [parse('botinfo Lumbridge  Bot'), parse('botinfo nobody'), parse('botinfo')],
               answers={'botLookup:lumbridge  bot': 'ScriptX[' + 'state=' + 'a' * 150 + ']'}), 0
    yield case('botinfo-exact', [parse('botinfo short')], answers={'botLookup': 'x' * 80}), None
    yield case('botinfo-empty', [parse('botinfo b')], answers={'botLookup': ''}), None
    # shop
    yield case('shop', [parse('shop', 'p2')]), 0
    yield case('shop-locked', [parse('shop', 'p2'), parse('shop', 'p3')], answers={'interactionLocked:p2': True, 'movementLocked:p3': True}), 0
    # ge
    offers = [[4151, 5, 2000000, True, 3], [4151, 2, 1900000, True, 2], [995, 1000, 1, False, 900], [1511, 10, 50, False, 10],
              [4151, 1, 2100000, False, 1], [1511, 3, 40, True, 3], [995, 7, 2, False, 7]]
    names = {'itemDefName:4151': 'Abyssal whip', 'itemDefName:995': 'Coins', 'itemDefName:1511': 'Logs', 'itemDefName:1513': 'Magic logs'}
    yield case('ge-usage', [parse('ge', 'p2'), parse('ge nothing', 'p2')]), 0
    yield case('ge-buying', [parse('ge buying', 'p2'), dict(op='book', index=0, player='p2', page=0, button=0)], values=dict(validOffers=offers), answers=names), 0
    yield case('ge-selling', [parse('ge selling', 'p2'), dict(op='book', index=0, player='p2', page=0, button=0)], values=dict(validOffers=offers), answers=names), 1
    yield case('ge-empty', [parse('ge selling'), parse('ge buying'), parse('ge bots')], values=dict(validOffers=[[1, 1, 1, False, 1]])), None
    many = [[1000 + i, i + 1, 10 * i, i % 2 == 0, i] for i in range(40)]
    yield case('ge-pages', [parse('ge selling'), parse('ge buying'), dict(op='book', index=0, player='p1', page=1, button=63),
                             dict(op='book', index=1, player='p1', page=0, button=0)], values=dict(validOffers=many)), None
    yield case('ge-bots', [parse('ge bots', 'p2')], values=dict(botOffers=[[1511, 100, 30, True, 0], [1513, 4, 900, True, 0], [1511, 7, 31, True, 0]]),
               answers=names), 0
    yield case('ge-bots-book', [parse('ge bots', 'p2'), dict(op='book', index=0, player='p2', page=0, button=0)],
               values=dict(botOffers=[[1511, 100, 30, True, 0], [1513, 4, 900, True, 0], [1511, 7, 31, True, 0]]), answers=names), 1
    yield case('ge-search', [parse('ge search Abyssal', 'p2'), parse('ge search logs', 'p2'), parse('ge search zzz', 'p2'), parse('ge search', 'p2'),
                             dict(op='input', index=0, value='coins'), dict(op='book', index=0, player='p2', page=0, button=0),
                             dict(op='book', index=2, player='p2', page=0, button=0)], values=dict(validOffers=offers), answers=names), 0
    yield case('ge-search-input-int', [parse('ge search'), dict(op='input', index=0, value=7)], values=dict(validOffers=offers), answers=names), None
    yield case('ge-botsearch', [parse('ge botsearch magic', 'p2'), parse('ge botsearch', 'p2'), dict(op='input', index=0, value='LOGS'),
                                parse('ge botsearch nope', 'p2')],
               values=dict(botOffers=[[1511, 100, 30, True, 0], [1513, 4, 900, True, 0]]), answers=names), 0
    yield case('ge-mute', [parse('ge mute', 'p2'), parse('ge mute', 'p2'), parse('ge mute', 'p2')]), 0
    yield case('ge-mute-saved', [parse('ge mute')], people=players(p1=dict(players()['p1'], attributes={'/save:ge:news-mute': {'bool': True}}))), None
    # commands / commandsearch
    yield case('commands-small', [parse('commands', 'p2'), parse('commands', 'p1'), parse('commands 1', 'p3')]), (0, 12)
    yield case('commands-full', [parse('commands', 'p1'), parse('commands 2', 'p1'), parse('commands 4', 'p1'), parse('commands 5', 'p1'),
                                 parse('commands 6', 'p1'), parse('commands 0', 'p1'), parse('commands x', 'p2'), parse('commands 2', 'p2'),
                                 parse('commands -1', 'p2')], extra=FULL), None
    yield case('commandsearch', [parse('commandsearch', 'p2'), parse('commandsearch --chat', 'p2'), parse('commandsearch a', 'p2'),
                                 parse('commandsearch var', 'p2'), parse('commandsearch var', 'p1'), parse('commandsearch zzzz', 'p1'),
                                 parse('commandsearch VAR --CHAT', 'p1'), parse('commandsearch var --chat', 'p2'), parse('commandsearch item spawn --chat', 'p1')],
               extra=FULL), (4, 12)
    yield case('commandsearch-wide', [parse('commandsearch e', 'p1'), parse('commandsearch e ', 'p1'), parse('commandsearch the', 'p1'),
                                      parse('commandsearch the', 'p3'), parse('commandsearch player --chat', 'p3')], extra=FULL), None
    # reply / xface
    yield case('reply', [parse('reply', 'p2'), dict(op='input', index=0, value='hi there')],
               people=players(p2=dict(players()['p2'], attributes={'replyTo': {'text': 'some_body_X'}}))), 0
    yield case('reply-int', [parse('reply', 'p2'), dict(op='input', index=0, value=12)],
               people=players(p2=dict(players()['p2'], attributes={'replyTo': {'text': 'x'}}))), None
    yield case('reply-cast', [parse('reply', 'p2')], people=players(p2=dict(players()['p2'], attributes={'replyTo': {'int': 4}}))), None
    yield case('reply-none', [parse('reply', 'p2'), parse('reply', 'p1')], answers={'isOpened:p1': True}), 0
    yield case('reply-blank', [parse('reply', 'p2')], people=players(p2=dict(players()['p2'], attributes={'replyTo': None}))), None
    yield case('xface', [parse('xface', 'p2')]), 0
    # completediaries / permadeath
    yield case('completediaries', [parse('completediaries'), parse('completediaries', 'p3')]), 0
    yield case('permadeath', [parse('permadeath'), parse('permadeath bob'), parse('permadeath no body'), parse('permadeath Some Name')],
               answers={'byName:bob': 'p2'}), 1
    # log / logdone
    yield case('log', [parse('log'), parse('log'), parse('logdone'), parse('logdone', 'p3')]), 1
    yield case('log-cast', [parse('log'), parse('logdone')], people=players(p1=dict(players()['p1'], attributes={'loc-log': {'text': 'x'}}))), None
    yield case('log-existing', [parse('log')], people=players(p1=dict(players()['p1'], attributes={'loc-log': {'list': ['{1,2,0,1,0}']}}))), 0
    # rolltrawlerloot
    yield case('rolltrawlerloot', [parse('rolltrawlerloot'), parse('rolltrawlerloot 5'), parse('rolltrawlerloot x')], answers={'level': 81}), 0
    # setconfig / varps
    yield case('setconfig', [parse('setconfig 1 2'), parse('setconfig 1'), parse('setconfig a 2'), parse('setconfig 3 -4 junk')]), 0
    yield case('getobjectvarp', [parse('getobjectvarp 1530'), parse('getobjectvarp'), parse('getobjectvarp x')],
               answers={'sceneryVarbit': 2000, 'varbitForObject': 2000, 'varbitVarp': 678}), 0
    yield case('getobjectvarp-null', [parse('getobjectvarp 1')], answers={'varbitForObject': None}), None
    yield case('define_varbit', [parse('define_varbit 5'), parse('define_varbit'), parse('define_varbit 6')],
               answers={'varbitForId:5': 5, 'varbitText': 'VarbitDefinition[id=5]', 'varbitForId:6': None}), 0
    yield case('setvarbit', [parse('setvarbit 10 3'), parse('setvarbit 10'), parse('setvarbit a 3'), parse('setvarbit 10 b'), parse('setvarbit 1 2 3')]), 0
    yield case('setvarbits', [parse('setvarbits 3 5 1'), parse('setvarbits 5 3 1'), parse('setvarbits 3 3 x'), parse('setvarbits 1 2')]), 0
    yield case('getvarbit', [parse('getvarbit 7'), parse('getvarbit x'), parse('getvarbit'), parse('getvarbit 7 8')], answers={'getVarbit': 3}), 0
    yield case('setvarp', [parse('setvarp 100 4 3'), parse('setvarp 100 33 1'), parse('setvarp 100 -1 1'), parse('setvarp 100 4'),
                           parse('setvarp 1 x 2'), parse('setvarp 1 2 3 4')]), 0
    yield case('setvarc', [parse('setvarc 12 99'), parse('setvarc 40000 1'), parse('setvarc -5 1'), parse('setvarc 1'), parse('setvarc 1 x'),
                           parse('setvarc 32767 -1')]), 0
    # farming / brewing
    yield case('grow', [parse('grow')], values=dict(patches=3), answers={'cropRun': False}), 0
    yield case('grow-none', [parse('grow'), parse('diseasecrops')], values=dict(patches=0)), None
    yield case('diseasecrops', [parse('diseasecrops')], values=dict(patches=2)), 0
    yield case('bins', [parse('finishbins'), parse('resetbins')], values=dict(bins=4),
               answers={'binFinished:p1:1': True, 'binClosed:p1:0': True, 'binClosed:p1:1': True, 'binClosed:p1:2': True}), 0
    yield case('brew', [parse('brew'), parse('brew 0'), parse('brew 1'), parse('brew 2'), parse('brew 3'), parse('brew 4'), parse('brew x')],
               values=dict(vats=2)), 2
    # getnpcparent
    defs = [[1, None], [2, [5, 6]], None, [3, [7]], [4, [7, 8]]]
    yield case('getnpcparent', [parse('getnpcparent 7'), dict(op='job', index=0), parse('getnpcparent 9'), dict(op='job', index=1),
                                parse('getnpcparent x'), parse('getnpcparent')], values=dict(npcDefinitions=defs)), 1
    # toggles
    for command, attribute in [('infinitespecial', 'infinite-special'), ('infinitespecialmove', 'infinite-special-move'),
                               ('allowaggro', '/save:allow_admin_aggression')]:
        yield case(f'toggle-{command}', [parse(command), parse(command), parse(f'{command} TRUE'), parse(f'{command} false'),
                                         parse(f'{command} yes'), parse(f'{command} true extra')]), 0
    # rules / confirmrules
    yield case('rules', [parse('rules', 'p2')]), 0
    yield case('confirmrules', [parse('confirmrules', 'p2'), parse('confirmrules 12', 'p2'), parse('confirmrules x', 'p2'),
                                parse('confirmrules 4321', 'p2'), parse('confirmrules 4321', 'p2')],
               people=players(p2=dict(players()['p2'], attributes={'rules:pin': {'int': 4321}})), values=dict(newPlayerAnnouncement=True)), 3
    yield case('confirmrules-cast', [parse('confirmrules 1', 'p2'), parse('confirmrules 1', 'p3')],
               people=players(p2=dict(players()['p2'], attributes={'rules:pin': {'text': '1'}}), p3=dict(players()['p3'], attributes={'rules:confirmed': {'int': 1}}))), None
    yield case('confirmrules-quiet', [parse('confirmrules -1', 'p2')]), None
    # stronghold
    yield case('setplayerstrong', [parse('setplayerstrong'), parse('setplayerstrong 0'), parse('setplayerstrong 3'), parse('setplayerstrong 4'),
                                   parse('setplayerstrong 5'), parse('setplayerstrong x'), parse('setplayerstrong 2')]), 3
    yield case('setplaqueread', [parse('setplaqueread'), parse('setplaqueread true'), parse('setplaqueread'), parse('setplaqueread false'),
                                 parse('setplaqueread maybe'), parse('setplaqueread true', 'p3'), parse('setplaqueread', 'p3')]), 1
    # brewkit
    yield case('brewkit', [parse('brewkit')] + [parse(f'brewkit {k}') for k in range(12)] + [parse('brewkit x')]), 7


def spawn():
    yield case('npc', [parse('npc 1'), parse('npc 50 2 true'), parse('npc'), parse('npc 1 0'), parse('npc 1 901'), parse('npc 1 900 maybe'),
                       parse('npc x 1 false'), parse('npc 1 1 ')]), 0
    yield case('npc-walk-moved', [parse('npc 3 2 false', 'p2')], answers={'npcLocation': [3201, 3199, 0]}), None
    yield case('item', [parse('item 4151'), parse('item 995 5000'), parse('item'), parse('item x'), parse('item 1 y'), parse('item 30000')],
               answers={'itemDefinitionsSize': 23000, 'maximumAdd': [1, 2000]}), 1
    yield case('item-boundary', [parse('item 23000'), parse('item 23001'), parse('item -1 -5')], answers={'itemDefinitionsSize': 23000}), None
    yield case('items', [parse('items 1 3'), parse('items 1 3 7'), parse('items 3 1'), parse('items 22999 23002 2'), parse('items 1'),
                         parse('items x 2'), parse('items 1 2 z')], answers={'itemDefinitionsSize': 23000, 'maximumAdd': [5, 1, 100]}), 1
    yield case('object', [parse('object 1530'), parse('object 1530 2'), parse('object 1530 10 3'), parse('object 0 22 -1'), parse('object'),
                          parse('object x')]), 2
    yield case('objectgrid', [parse('objectgrid 100 112 10 0'), parse('objectgrid 1 2 3'), parse('objectgrid a 2 3 4'), parse('objectgrid 5 4 10 1')]), 0
    yield case('removeobject', [parse('removeobject'), parse('removeobject x'), parse('removeobject')],
               answers={'object': [[1530, [3222, 3218, 0], 10, 0, False, True, True], None], 'sceneryRemove': False}), 0
    statics = [[0, 0, 1, [1, [3200, 3200, 0], 10, 0, False, True, False]], [0, 0, 3, [2, [3200, 3200, 0], 22, 1, True, False, True]],
               [6, 2, 3, [3, [3222, 3218, 0], 22, 1, False, False, True]]]
    yield case('objects', [parse('objects', 'p2'), parse('objects x', 'p2')],
               values={'statObjects:400,400,0': statics, 'chunkObjects:400,400,0': [None, [9, [3200, 3200, 0], 10, 2, True, True, True], None, None]},
               answers={'object': [[5, [3200, 3200, 0], 10, 0, True, False, False]]}), 0
    yield case('objects-empty', [parse('objects', 'p1')]), None


def stats():
    yield case('stats', [parse('stats', 'p2'), dict(op='book', index=0, player='p2', page=0, button=0), dict(op='book', index=0, player='p2', page=1, button=0),
                         dict(op='book', index=0, player='p2', page=2, button=0), dict(op='book', index=0, player='p2', page=3, button=0)],
               answers={'clues': None, 'slayerCompleted': 17, 'questPoints': 33, 'ironmanMode': 'HARDCORE', 'kills': 12, 'rareDrops': 1},
               values={'clues:p2': [4, 5, 6]}, people=players(p2=dict(players()['p2'], attributes={'stats_manager:deaths': {'int': 3},
                                                                                                    'stats_manager:logs_chopped': {'int': 900}}))), 1
    yield case('stats-other', [parse('stats bob', 'p1'), dict(op='book', index=0, player='p1', page=0, button=0), parse('stats nobody', 'p1'),
                               parse('stats a b', 'p1')],
               answers={'byName:bob': 'p2', 'ironmanMode': 'NONE'}, values={'clues:p2': [1, 2, 3]}), 2
    yield case('stats-short', [parse('stats', 'p3'), dict(op='book', index=0, player='p3', page=0, button=0)], values={'clues:p3': [1, 2]}), None
    yield case('stats-no-query', [parse('stats', 'p2'), dict(op='book', index=0, player='p3', page=0, button=0)]), None
    yield case('stats-counters', [parse('stats', 'p3'), dict(op='book', index=0, player='p3', page=0, button=0)], values={'clues:p3': [1, 2, 3]}), None


def system():
    worker = dict(shutdownListeners=['grand-exchange', 'audio'], worldPersists=['clans', 'ServerStore', 'zones'], cleanDirs=['data'])
    yield case('update-command', [parse('update'), dict(op='update'), parse('cancelupdate'), dict(op='update')], values=worker), 0
    yield case('update-seconds', [parse('update 30'), parse('update x'), parse('update -5')], values=worker), 0
    yield case('update-denied', [parse('update', 'p2'), parse('cancelupdate', 'p2')], values=dict(playerCommands=False)), None
    yield case('shutdown-command', [parse('shutdown'), parse('shutdown', 'p2')], values=worker), 0
    ticks = [dict(op='countdown', ticks=3), dict(op='flag', state='UPDATING')] + [dict(op='update')] * 5
    yield case('update-countdown', ticks + [dict(op='query')], values=dict(worker, online=['p1', 'p2'])), 1
    yield case('update-backup', [dict(op='backup', value=True), dict(op='countdown', ticks=12), dict(op='flag', state='UPDATING'), dict(op='notify')]
               + [dict(op='update')] * 4 + [dict(op='query')], values=dict(worker, existingDirs=['data'])), 3
    yield case('update-backup-short', [dict(op='backup', value=True), dict(op='countdown', ticks=4), dict(op='countdown', ticks=25),
                                       dict(op='backup', value=False), dict(op='countdown', ticks=7), dict(op='query')], values=worker), None
    yield case('update-backup-fail', [dict(op='backup', value=True), dict(op='flag', state='UPDATING')] + [dict(op='update')] * 2,
               values=dict(worker, dataPath=None)), None
    yield case('flag-states', [dict(op='flag', state='ACTIVE'), dict(op='flag', state='ACTIVE'), dict(op='flag', state='PRIVATE'),
                               dict(op='query'), dict(op='flag', state='UPDATING'), dict(op='flag', state='UPDATING'),
                               dict(op='cancel'), dict(op='query'), dict(op='flag', state='TERMINATED'), dict(op='query')], values=worker), 2
    yield case('notify-error', [dict(op='notify')], values=dict(worker, online=['p1', 'p2']), answers={}), 0
    yield case('hook', [dict(op='hook'), dict(op='hook')], values=worker), 0
    yield case('hook-active', [dict(op='flag', state='ACTIVE'), dict(op='hook'), dict(op='query')],
               values=dict(worker, online=['p1', 'p2', 'p3'], websocket=True), answers={'artificial:p3': True}), 1
    yield case('terminate', [dict(op='terminate')], values=dict(worker, websocket=True, dataPath='saves', cleanDirs=['saves'])), 0
    yield case('terminate-no-data', [dict(op='terminate')], values=dict(worker, dataPath=None)), None
    yield case('terminate-player-failure', [dict(op='terminate')], values=dict(worker, online=['p1', 'p2']), answers={}), None
    yield case('save-queue', [dict(op='save', directory='data')], values=dict(worker, existingDirs=['data'], clock=[0, 10, 20, 6000, 7000]),
               answers={'queueEmpty': [False, False, True]}), 0
    yield case('save-timeout', [dict(op='save', directory='data')], values=dict(worker, clock=[0, 4999, 5000]), answers={'queueEmpty': False}), None
    yield case('worker-not-started-cancel', [dict(op='cancel'), dict(op='query')], values=dict(worker, workerStarted=False)), None
    yield case('config', [dict(op='config', what='validLogin', player='p2'), dict(op='config', what='doubleExp'),
                          dict(op='config', what='beta', name='bob'), dict(op='config', what='get', key='dxp'),
                          dict(op='config', what='getOr', key='dxp', fail='x'), dict(op='config', what='sizes'),
                          dict(op='config', what='split', data='a,b,,c,,', regex=','), dict(op='config', what='split', data='abc', regex=','),
                          dict(op='config', what='split', data='  a;b  ', regex=';'), dict(op='config', what='split', data=' x ', regex=' '),
                          dict(op='config', what='split', data='', regex=','), dict(op='config', what='split', data=',', regex=','),
                          dict(op='config', what='split', data='a::b::c', regex='::'), dict(op='config', what='split', data='a b', regex='q')]), None


def packets():
    def pk(kind, player='p1', **fields):
        return dict(op='packet', kind=kind, player=player, **fields)
    yield case('packets-system-update', [pk('systemUpdate', time=t) for t in [0, 1, 100, 255, 256, 32767, 65535, 65536, -1, 600]]), 0
    cams = []
    for t in ['POSITION', 'ROTATION', 'SET', 'SHAKE', 'RESET']:
        cams += [pk('camera', type=t, x=3222, y=3218, height=400, speed=5, zoomSpeed=10),
                 pk('camera', type=t, x=-1, y=70000, height=-2, speed=300, zoomSpeed=-129)]
    yield case('packets-camera', cams, people=players(p1=dict(players()['p1'], packetCount=65534))), 0
    yield case('packets-camera-no-scene', [pk('camera', 'p2', type=t, x=1, y=2, height=3, speed=4, zoomSpeed=5) for t in ['RESET', 'POSITION', 'SET']],
               people=players(p2=dict(players()['p2'], lastSceneGraph=None))), None
    yield case('packets-cs-config', [pk('csConfig', id=i, value=v) for i, v in [(0, 0), (1, 1), (1199, 255), (65535, -1), (70000, 128), (5, 32768)]],
               people=players(p1=dict(players()['p1'], packetCount=7))), 0
    locs = [[3222, 3218, 0], [3176, 3168, 0], [3200, 3263, 1], [0, 0, 0], [3300, 3100, 3]]
    yield case('packets-area-position', [pk('areaPosition', location=l) for l in locs] + [pk('chunkBuffer', location=l) for l in locs]), 0
    objects = [[1530, [3222, 3218, 0], 10, 1, False, True, True], [9, [3229, 3223, 2], 22, 3, False, True, True], [7, [3176, 3168, 0], 0, 5, False, True, True],
               [70000, [3201, 3207, 1], 63, -1, False, True, True]]
    yield case('packets-animate-object', [pk('animateObject', object=o, animation=a) for o in objects for a in [0, 4567, 65535, -1]]), 0


def cases():
    for group in [misc, spawn, stats, system, packets]:
        yield from group()


def groups(cases):
    out = {}
    for case in cases:
        out[case['group']] = out.get(case['group'], 0) + 1
    return dict(sorted(out.items()))


def commands(cases):
    seen = set()
    for case in cases:
        for op in case['ops']:
            if op['op'] == 'parse':
                name = op['line'].split(' ')[0]
                if name in PORTED:
                    seen.add(name)
    return sorted(seen)
