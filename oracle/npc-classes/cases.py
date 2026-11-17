"""Designed cases for the npc-classes oracle (see generate.py).

A case sets up the recording world (nodes, attributes, answers, values), registers original classes (`plugin`: the
`@Initializable` path, `newInstance(null)`; `listeners`: the content-interface path, `defineListeners` then
`defineDestinationOverrides`), then runs operations: `NPC.create` through the AbstractNPC registry, the subclasses'
overrides (init, tick, handleTickActions, finalizeDeath, onAttack, onImpact, the swing handler, getWalkRadius), the
registered interaction handlers and destination overrides, option handlers, IdleAbstractNPC's members and BankerNPC's
companion functions. `sweep` marks the operation whose recorded calls fail one at a time in the second pass.
"""
import pathlib
import re

H = 'content.global.handlers.npc.'
CLASSES = {
    'shop': H + 'ShopNPC',
    'banker': H + 'BankerNPC',
    'cow': H + 'CowNPC',
    'duck': H + 'DuckNPC',
    'rat': H + 'RatNPC',
    'guard': H + 'GuardNPC',
    'monk': H + 'MonkNPC',
    'highwayman': H + 'HighwayManNPC',
    'icegiant': H + 'IceGiantNPC',
    'darkwizard': H + 'DarkWizardNPC',
    'disturb': H + 'NPCDisturbPlugin',
    'haircut': H + 'HairCutPlugin',
    'talk': H + 'NPCTalkListener',
    'probe': 'oracle.ConflictProbe',
    'shopnamed': 'oracle.probe.ShopNPC',
}
ABSTRACT = ['shop', 'banker', 'cow', 'duck', 'rat', 'guard', 'monk', 'highwayman', 'icegiant', 'darkwizard']
BY_CLASS = {v: k for k, v in CLASSES.items()}

NEEDED_EVENTS = ['newNPC', 'setWalks', 'setAggressive', 'log', 'npcInit', 'npcTick', 'npcFinalizeDeath', 'npcOnImpact', 'sendChat',
                 'random', 'randomize', 'getScenery', 'setDirection', 'direction', 'setAttribute', 'getAttribute', 'removeAttribute',
                 'on', 'setDest', 'npcHandler', 'npcOptionHandler', 'labellerOpen', 'openDialogueId', 'openDialogueName', 'openFile',
                 'sendDialogue', 'apiMessage', 'playerMessage', 'apiGetAttribute', 'faceLocation', 'victim', 'inCombat',
                 'barcrawlInstance', 'barcrawlFinished', 'barcrawlStarted', 'barcrawlCompleted', 'randomIDs', 'antiMacroEventNpc',
                 'finalized', 'talkTo', 'holidayRandomIDs', 'holidayEventNpc', 'gcCompletionDialogue', 'removeItem', 'openBankAccount',
                 'openCollectionBox', 'hasSealOfPassage', 'apiOpenDialogue', 'getQuest', 'questStarted', 'groundItem', 'getDiary',
                 'diaryComplete', 'diaryUpdate', 'lifepoints', 'maximumLifepoints', 'heal', 'animate', 'setNextAttack', 'meleeSwing',
                 'setStyle', 'getSpell', 'setAutocastSpell', 'isAttacking', 'transform', 'reTransform', 'isInvisible', 'task', 'level',
                 'packetMessage', 'isDead', 'continueAttack', 'swingHandler', 'canSwing', 'setCombatFlags', 'combatAttack',
                 'npcTickActions', 'walkRadius']

SOURCES = pathlib.Path(__file__).resolve().parents[2] / 'reference/server/Server/src/main/content/global/handlers/npc'


def class_ids(name):
    """A class's ids as its source lists them (BankerNPC by the NPCs constants' suffixes)."""
    text = (SOURCES / name).read_text(encoding='utf-8')
    if name == 'BankerNPC.kt':
        block = text[text.index('val NPC_IDS'):text.index('companion')]
        return [int(x) for x in re.findall(r'NPCs\.[A-Z_]+_(\d+)', block)]
    found = re.search(r'(?:ID = |return new int\[\] )\{([^}]*)\}', text, re.S)
    return [int(x) for x in re.findall(r'-?\d+', re.sub(r'//[^\n]*', '', found.group(1)))]


FILES = {'shop': 'ShopNPC.java', 'banker': 'BankerNPC.kt', 'cow': 'CowNPC.java', 'duck': 'DuckNPC.java', 'rat': 'RatNPC.java',
         'guard': 'GuardNPC.java', 'monk': 'MonkNPC.java', 'highwayman': 'HighwayManNPC.java', 'icegiant': 'IceGiantNPC.java',
         'darkwizard': 'DarkWizardNPC.java'}


def plugin(*names):
    return [dict(op='plugin', **{'class': CLASSES[n]}) for n in names]


def listeners(*names):
    return [dict(op='listeners', **{'class': CLASSES[n]}) for n in names]


def case(group, name, ops, sweep=None, **state):
    out = dict(group=group, name=name, ops=ops)
    out.update(state)
    return out, sweep


def last(ops):
    return len(ops) - 1


def create(id, key='n', location=(3200, 3200, 0), **extra):
    return dict(op='create', id=id, key=key, location=None if location is None else list(location), **extra)


def scan_order():
    """The original's scan order (crates/server/src/startup/scan_order.txt, from the world-trace boot record)."""
    text = (pathlib.Path(__file__).resolve().parents[2] / 'crates/server/src/startup/scan_order.txt').read_text(encoding='utf-8')
    section, order = None, []
    for line in text.splitlines():
        if line.startswith('['):
            section = line
        elif section in ('[scan.contentInterfaces]', '[scan.initializable]') and line in BY_CLASS:
            order.append((section, line))
    return order


def registration():
    for n in CLASSES:
        if n in ('talk',):
            continue
        yield case('registration', n, plugin(n), sweep=0)
    yield case('registration', 'talk listeners', listeners('talk'), sweep=0)
    yield case('registration', 'banker listeners', listeners('banker'), sweep=0)
    # The original's scan: content interfaces (defineListeners/defineDestinationOverrides), then @Initializable plugins.
    ops = []
    for section, cls in scan_order():
        ops.append(dict(op='listeners' if section == '[scan.contentInterfaces]' else 'plugin', **{'class': cls}))
    for n in ABSTRACT:
        for id in class_ids(FILES[n]):
            ops.append(dict(op='forId', id=id))
    for id in (0, -1, 1, 7727, 4999999, 69669):
        ops.append(dict(op='forId', id=id))
    yield case('registration', 'scan order', ops)
    # The same class twice: the second instance replaces the first silently.
    yield case('registration', 'same class twice', plugin('shop') + [dict(op='forId', id=520)] + plugin('shop') + [dict(op='forId', id=520)],
               sweep=2)
    # Another class claiming an id first keeps it (logged once per id); a class named like ShopNPC is still another class.
    yield case('registration', 'conflicts', plugin('shop', 'probe', 'shopnamed') + [dict(op='forId', id=i) for i in (520, 4999999, 4999998, 46)],
               sweep=1)
    yield case('registration', 'conflicts reversed', plugin('probe', 'shop', 'duck') + [dict(op='forId', id=i) for i in (520, 46, 2693)])
    yield case('registration', 'forId empty', [dict(op='forId', id=520)])


def creation():
    one = {'shop': 520, 'banker': 494, 'cow': 81, 'duck': 46, 'rat': 47, 'guard': 9, 'monk': 7727, 'highwayman': 180,
           'icegiant': 111, 'darkwizard': 172}
    for n, id in one.items():
        ops = plugin(n) + [create(id), dict(op='radius', node='n')]
        yield case('create', n, ops, sweep=1)
    # A registered id with a null location: the Kotlin BankerNPC construct refuses it before the constructor.
    yield case('create', 'banker null location', plugin('banker') + [create(494, location=None)], sweep=1)
    yield case('create', 'shop null location', plugin('shop') + [create(520, location=None)])
    # Unregistered ids, with and without a direction; construct returning null falls back to a base NPC.
    yield case('create', 'base', [create(1), create(2, key='m', direction='NORTH_EAST'), dict(op='radius', node='n')], sweep=0)
    yield case('create', 'construct null', plugin('shopnamed') + [create(4999998), create(520, key='m')], sweep=1)
    yield case('create', 'probe', plugin('probe') + [create(4999999, direction='WEST')], sweep=1)
    yield case('create', 'every class every id',
               [op for n in ABSTRACT for op in plugin(n)] + [create(id, key=f'{n}{id}') for n in ABSTRACT for id in class_ids(FILES[n])[:3]])


def inits():
    for n, id in [('shop', 520), ('cow', 81), ('rat', 47), ('guard', 9), ('monk', 7727)]:
        yield case('init', n, plugin(n) + [create(id), dict(op='init', node='n')], sweep=2)
    # BankerNPC.init: the first booth among south, west, north, east (an object that is no booth is passed over).
    at = (3200, 3200, 0)
    sides = {'SOUTH': (0, -1), 'WEST': (-1, 0), 'NORTH': (0, 1), 'EAST': (1, 0)}
    for side, (dx, dy) in sides.items():
        key = f'scenery:{at[0] + dx},{at[1] + dy},{at[2]}'
        yield case('init', f'banker booth {side}', plugin('banker') + [create(494, location=at), dict(op='init', node='n')], sweep=2,
                   nodes=dict(booth=dict(kind='scenery', id=2213)), answers={key: 'booth'})
    yield case('init', 'banker no booth', plugin('banker') + [create(494, location=at), dict(op='init', node='n')], sweep=2)
    yield case('init', 'banker not a booth then booth', plugin('banker') + [create(494, location=at), dict(op='init', node='n')], sweep=2,
               nodes=dict(table=dict(kind='scenery', id=1), booth=dict(kind='scenery', id=37474)),
               answers={'scenery:3200,3199,0': 'table', 'scenery:3200,3201,0': 'booth', 'scenery:3201,3200,0': 'booth'})
    yield case('init', 'banker plane 3 edge', plugin('banker') + [create(494, location=(0, 16383, 3)), dict(op='init', node='n')],
               nodes=dict(booth=dict(kind='scenery', id=2214)), answers={'scenery:0,-16384,3': 'booth'})
    # DarkWizardNPC.init: magic style, then the default autocast spell (6 for 172, 4 for 174).
    for id in (172, 174):
        yield case('init', f'dark wizard {id}', plugin('darkwizard') + [create(id), dict(op='init', node='n')], sweep=2)
    yield case('init', 'dark wizard no spell', plugin('darkwizard') + [create(172), dict(op='init', node='n')], answers={'getSpell:6': None})
    yield case('init', 'dark wizard not a combat spell', plugin('darkwizard') + [create(174), dict(op='init', node='n')],
               answers={'getSpell:4': 'other'})


def creatures():
    base = lambda n, id: plugin(n) + [create(id)]
    # CowNPC / DuckNPC tick: one random(45) per tick, chat only at 5 (never for the calf 1766), then NPC.tick.
    for id, roll in [(81, 5), (81, 4), (1766, 5), (3309, 44)]:
        yield case('tick', f'cow {id} roll {roll}', base('cow', id) + [dict(op='tick', node='n')], sweep=2, answers=dict(random=roll))
    for roll in (5, 0):
        yield case('tick', f'duck roll {roll}', base('duck', 46) + [dict(op='tick', node='n')], sweep=2, answers=dict(random=roll))
    yield case('tick', 'shop tick is NPC.tick', base('shop', 520) + [dict(op='tick', node='n'), dict(op='tickActions', node='n')])
    killer = dict(k=dict(kind='player', location=[3200, 3200, 0]), o=dict(kind='npc', id=1))
    # RatNPC.finalizeDeath: a rat tail for a player killer with Witch's Potion started.
    for started in (True, False):
        yield case('death', f'rat started {started}', base('rat', 47) + [dict(op='death', node='n', killer='k')], sweep=2, nodes=killer,
                   answers=dict(questStarted=started))
    yield case('death', 'rat npc killer', base('rat', 47) + [dict(op='death', node='n', killer='o')], nodes=killer)
    yield case('death', 'rat null killer', base('rat', 47) + [dict(op='death', node='n')], nodes=killer)
    yield case('death', 'rat quest missing', base('rat', 47) + [dict(op='death', node='n', killer='k')], nodes=killer,
               answers=dict(questMissing=True))
    # HighwayManNPC: the Falador diary task only for 180 and a player killer.
    for id, complete in [(180, False), (180, True), (2677, False)]:
        yield case('death', f'highwayman {id} complete {complete}', base('highwayman', id) + [dict(op='death', node='n', killer='k')],
                   sweep=2, nodes=killer, answers=dict(diaryComplete=complete))
    yield case('death', 'highwayman npc killer', base('highwayman', 180) + [dict(op='death', node='n', killer='o')], nodes=killer)
    yield case('death', 'highwayman diary missing', base('highwayman', 180) + [dict(op='death', node='n', killer='k')], nodes=killer,
               answers=dict(diaryMissing=True))
    yield case('attacked', 'highwayman', base('highwayman', 180) + [dict(op='attacked', node='n', target='k')], sweep=2, nodes=killer)
    # IceGiantNPC: within 100 of (3052, 9573, 0) on the same plane.
    for name, at, complete in [('near', (3052, 9573, 0), False), ('edge', (3152, 9573, 0), False), ('beyond', (3123, 9645, 0), False),
                               ('diagonal edge', (3122, 9644, 0), False), ('other plane', (3052, 9573, 1), False),
                               ('complete', (3060, 9573, 0), True)]:
        yield case('death', f'ice giant {name}', plugin('icegiant') + [create(111, location=at), dict(op='death', node='n', killer='k')],
                   sweep=2, nodes=killer, answers=dict(diaryComplete=complete))
    yield case('death', 'ice giant npc killer', base('icegiant', 111) + [dict(op='death', node='n', killer='o')], nodes=killer)
    yield case('death', 'base npc', [create(1), dict(op='death', node='n', killer='k')], nodes=killer)
    # MonkNPC's swing handler: heal (animation 709, heal 2, next attack 4, -1) when hurt and randomize(10) < 2.
    for name, life, maximum, roll in [('heals', 5, 10, 1), ('heals at 0', 5, 10, 0), ('roll 2', 5, 10, 2), ('full', 10, 10, 0)]:
        yield case('swing', f'monk {name}', base('monk', 7727) + [dict(op='swing', node='n', victim='k')], sweep=2, nodes=killer,
                   answers=dict(lifepoints=life, maximumLifepoints=maximum, randomize=roll))
    yield case('swing', 'monk twice', base('monk', 7727) + [create(7727, key='m'), dict(op='swing', node='n', victim='k'),
                                                            dict(op='swing', node='m', victim='k')], nodes=killer)
    yield case('swing', 'base npc', [create(1), dict(op='swing', node='n', victim='k')], nodes=killer)
    # DarkWizardNPC.onImpact: after a switch the default spell returns; otherwise random(6) > 4 picks one of its two.
    for name, attrs, rolls in [('switch back', {'switched': True}, [0]), ('no switch', {}, [4]), ('switch', {}, [5, 1]),
                               ('switch first', {}, [6, 0]), ('switched false', {'switched': False}, [5, 0]),
                               ('switched not boolean', {'switched': 1}, [0])]:
        yield case('impact', f'dark wizard {name}', base('darkwizard', 172) + [dict(op='set', attributes=dict(n=attrs)),
                                                                                 dict(op='impact', node='n', entity='k')],
                   sweep=3, nodes=killer, answers=dict(random=rolls))
    yield case('impact', 'dark wizard 174 switch', base('darkwizard', 174) + [dict(op='impact', node='n', entity='k')], sweep=2,
               nodes=killer, answers=dict(random=[5, 0]))


BARCRAWL = [733, 848, 735, 739, 737, 738, 731, 568, 3217, 736, 734]
GNOME_JOBS = [3811, 4588, 602, 601, 670, 4597, 4587, 4599, 4580, 3810, 3809, 469, 3817, 4586, 4585]


def talk():
    reg = listeners('talk')
    npc = lambda id, kind='npc', **extra: dict(kind=kind, id=id, location=[3201, 3200, 0], **extra)
    # The barcrawl handler (registration index 0).
    for id in BARCRAWL:
        yield case('talk', f'barcrawl {id}', reg + [dict(op='interact', n=0, node='b')], sweep=1, nodes=dict(b=npc(id)),
                   answers=dict(barcrawlStarted=True))
    for name, answers in [('finished', dict(barcrawlFinished=True)), ('not started', dict(barcrawlStarted=False)),
                          ('completed', dict(barcrawlStarted=True, barcrawlCompleted=True))]:
        yield case('talk', f'barcrawl {name}', reg + [dict(op='interact', n=0, node='b')], sweep=1, nodes=dict(b=npc(735)), answers=answers)
    yield case('talk', 'barcrawl unknown id', reg + [dict(op='interact', n=0, node='b')], nodes=dict(b=npc(1)))
    # The catch-all handler (index 1).
    t = lambda name, nodes, answers=None, values=None, attributes=None, sweep=1: case(
        'talk', name, reg + [dict(op='interact', n=1, node='t')], sweep=sweep, nodes=nodes, answers=answers or {}, values=values or {},
        attributes=attributes or {})
    yield t('plain', dict(t=npc(2)))
    yield t('plain closed', dict(t=npc(2)), answers=dict(openDialogueId=False))
    yield t('not an npc', dict(t=dict(kind='scenery', id=5)))
    rv = dict(randomIDs=[2, 3117, 409])
    yield t('random event none', dict(t=npc(409)), values=rv)
    yield t('random event sandwich lady', dict(t=npc(3117)), values=rv)
    yield t('random event other npc', dict(t=npc(409), e=npc(409, 'randomNpc')), answers=dict(antiMacroEventNpc='e'), values=rv)
    yield t('random event finalized', dict(t=npc(409, 'randomNpc')), answers={'antiMacroEventNpc': 't', 'finalized:t': True}, values=rv)
    yield t('random event talk', dict(t=npc(409, 'randomNpc')), answers={'antiMacroEventNpc': 't'}, values=rv)
    yield t('random event sandwich lady own', dict(t=npc(3117, 'randomNpc')), answers={'antiMacroEventNpc': 't', 'finalized:t': True},
            values=rv)
    hv = dict(holidayRandomIDs=[5, 6])
    yield t('holiday not holiday npc', dict(t=npc(5)), values=hv)
    yield t('holiday none', dict(t=npc(5, 'holidayNpc')), values=hv)
    yield t('holiday other', dict(t=npc(5, 'holidayNpc'), e=npc(6, 'holidayNpc')), answers=dict(holidayEventNpc='e'), values=hv)
    yield t('holiday finalized', dict(t=npc(5, 'holidayNpc')), answers={'holidayEventNpc': 't', 'finalized:t': True}, values=hv)
    yield t('holiday talk', dict(t=npc(5, 'holidayNpc')), answers={'holidayEventNpc': 't'}, values=hv)
    yield t('holiday extra npc', dict(t=npc(7), e=npc(6, 'holidayNpc')), answers=dict(holidayEventNpc='e'),
            attributes=dict(t={'holiday_random_extra_npc': True}))
    yield t('holiday extra npc without event', dict(t=npc(7)), attributes=dict(t={'holiday_random_extra_npc': True}))
    yield t('holiday extra not boolean', dict(t=npc(7)), attributes=dict(t={'holiday_random_extra_npc': 1}))
    yield t('facing booth', dict(t=npc(494)), attributes=dict(t={'facing_booth': True}))
    yield t('facing booth not boolean', dict(t=npc(494)), attributes=dict(t={'facing_booth': 'yes'}))
    yield t('victim', dict(t=npc(2)), answers={'victim:p': 't'})
    yield t('in combat', dict(t=npc(2)), answers={'inCombat:t': True})
    for ordinal, id in enumerate(GNOME_JOBS):
        yield t(f'gnome job {ordinal}', dict(t=npc(id)), attributes=dict(p={'gnome_cooking:job:job_ordinal': ordinal}),
                sweep=1 if ordinal in (0, 14) else None)
    yield t('gnome job other npc', dict(t=npc(2)), attributes=dict(p={'gnome_cooking:job:job_ordinal': 3}))
    yield t('gnome job complete', dict(t=npc(601)), attributes=dict(p={'gnome_cooking:job:job_ordinal': 3, 'gnome_cooking:job:job_complete': True}))
    yield t('gnome job out of range', dict(t=npc(2)), attributes=dict(p={'gnome_cooking:job:job_ordinal': 15}))
    yield t('gnome job negative', dict(t=npc(2)), attributes=dict(p={'gnome_cooking:job:job_ordinal': -2}))
    yield t('gnome job not an int', dict(t=npc(2)), attributes=dict(p={'gnome_cooking:job:job_ordinal': 'x'}))
    yield case('talk', 'other player', reg + [dict(op='interact', n=1, node='t', player='q')], sweep=1,
               nodes=dict(t=npc(2), q=dict(kind='player', location=[3000, 3000, 1])))


def banker():
    reg = listeners('banker')
    npc = lambda id, at=(3200, 3200, 0), kind='npc', **extra: dict(kind=kind, id=id, location=list(at), **extra)
    lunar = (2098, 3920, 0)
    # 'bank' (0), 'collect' (1), 'talk-to' (2).
    for n, name in [(0, 'bank'), (1, 'collect')]:
        yield case('banker', f'{name}', reg + [dict(op='interact', n=n, node='b')], sweep=1, nodes=dict(b=npc(494)))
        yield case('banker', f'{name} lunar no seal', reg + [dict(op='interact', n=n, node='b')], sweep=1, nodes=dict(b=npc(4519, lunar)))
        yield case('banker', f'{name} lunar seal', reg + [dict(op='interact', n=n, node='b')], sweep=1, nodes=dict(b=npc(4519, lunar)),
                   answers=dict(hasSealOfPassage=True))
        yield case('banker', f'{name} not an npc', reg + [dict(op='interact', n=n, node='s')], nodes=dict(s=dict(kind='scenery', id=1)))
    for paid in (True, False):
        yield case('banker', f'odovacar paid {paid}', reg + [dict(op='interact', n=0, node='b')], sweep=1, nodes=dict(b=npc(5383)),
                   answers=dict(removeItem=paid))
    yield case('banker', 'odovacar collect', reg + [dict(op='interact', n=1, node='b')], nodes=dict(b=npc(5383)))
    yield case('banker', 'talk-to', reg + [dict(op='interact', n=2, node='b')], sweep=1, nodes=dict(b=npc(494)))
    yield case('banker', 'talk-to not an npc', reg + [dict(op='interact', n=2, node='s')], nodes=dict(s=dict(kind='scenery', id=1)))
    # The destination override.
    d = lambda name, nodes, answers=None, sweep=1: case('banker', f'dest {name}', reg + [dict(op='dest', n=0, entity='p', node='b')],
                                                       sweep=sweep, nodes=nodes, answers=answers or {})
    yield d('ogress', dict(b=npc(7049)))
    yield d('ogress 7050', dict(b=npc(7050, (100, 16383, 3))))
    yield d('magnus', dict(b=npc(5488)))
    for id in (6532, 6533, 6534, 6535):
        yield d(f'{id}', dict(b=npc(id, direction='EAST')))
    yield d('6532 null direction', dict(b=npc(6532, direction=None)))
    yield d('plain npc', dict(b=npc(494)))
    yield d('not an npc', dict(b=dict(kind='scenery', id=1, location=[1, 1, 0])))
    create_banker = [dict(op='plugin', **{'class': CLASSES['banker']}), create(494, key='b')]
    yield case('banker', 'dest banker no booth', reg + create_banker + [dict(op='dest', n=0, entity='p', node='b')], sweep=3)
    yield case('banker', 'dest banker booth west', reg + create_banker + [dict(op='dest', n=0, entity='p', node='b')], sweep=3,
               nodes=dict(booth=dict(kind='scenery', id=2213)), answers={'scenery:3199,3200,0': 'booth'})
    yield case('banker', 'dest banker booth north', reg + create_banker + [dict(op='dest', n=0, entity='p', node='b')],
               nodes=dict(booth=dict(kind='scenery', id=6084)), answers={'scenery:3200,3201,0': 'booth'})
    # The companion functions.
    yield case('banker', 'companion', [dict(op='lunar', node='b'), dict(op='access', node='b'), dict(op='bank', node='b'),
                                       dict(op='collect', node='b')], nodes=dict(b=npc(5383, lunar)), answers=dict(removeItem=[True, False]))
    yield case('banker', 'lunar region edge', [dict(op='lunar', node='b'), dict(op='lunar', node='c')],
               nodes=dict(b=npc(4519, (2047, 3903, 0)), c=npc(4519, (2048, 3904, 0))))


def idle():
    # IdleAbstractNPC through a concrete subclass (oracle.IdleProbe): idle id 1265, active id 1266.
    probe = lambda idle=True, **extra: dict(kind='idle', id=1265, activeId=1266, location=[2700, 3700, 0], idle=idle, **extra)
    k = dict(kind='player', location=[2701, 3700, 0])
    tick = dict(op='tickActions', node='r')
    for name, idle_, attrs, answers in [('idle', True, {}, {}), ('attacking', False, {}, {'isAttacking:r': True}),
                                        ('timed out', False, {'combat-time': {'clockPlusSeconds': -100}}, {}),
                                        ('recent', False, {'combat-time': {'clockPlusSeconds': -2}}, {}),
                                        ('no time', False, {}, {}),
                                        ('future', False, {'combat-time': {'clockPlusSeconds': 50}}, {}),
                                        ('far past', False, {'combat-time': {'long': 0}}, {}),
                                        ('not a long', False, {'combat-time': 5}, {})]:
        yield case('idle', f'tick {name}', [tick], sweep=0 if name in ('idle', 'timed out') else None, nodes=dict(r=probe(idle_)),
                   attributes=dict(r=attrs), answers=answers)
    yield case('idle', 'tick timeout 0', [tick], nodes=dict(r=probe(False, timeout=0)), attributes=dict(r={'combat-time': {'clockPlusSeconds': -2}}))
    a = lambda name, answers, style='MELEE', message=True, other='k', nodes=None: case(
        'idle', f'attackable {name}', [dict(op='idle', node='r', call='isAttackable', entity=other, style=style, message=message)], sweep=0,
        nodes=dict(r=probe(), k=k, **(nodes or {})), answers=answers)
    yield a('plain', {})
    yield a('invisible', dict(isInvisible=True))
    yield a('task low', dict(task=50, level=60))
    yield a('task high', dict(task=60, level=50))
    yield a('task high quiet', dict(task=60, level=50), message=False)
    yield a('task equal', dict(task=50, level=50))
    yield a('task npc attacker', dict(task=60), other='o', nodes=dict(o=dict(kind='npc', id=3)))
    yield a('dead', dict(isDead=True))
    yield a('zone refuses', dict(continueAttack=False), style=None)
    for answer in ('STILL_INTERACT', 'NO_INTERACT', None):
        yield case('idle', f'disturbing range {answer}', [dict(op='idle', node='r', call='inDisturbingRange', entity='k')], sweep=0,
                   nodes=dict(r=probe(), k=k), answers=dict(canSwing=answer))
    yield case('idle', 'disturbing range active', [dict(op='idle', node='r', call='inDisturbingRange', entity='k')],
               nodes=dict(r=probe(False), k=k))
    yield case('idle', 'can disturb', [dict(op='idle', node='r', call='canDisturb', entity='k'), dict(op='set', nodes=dict(r=probe(False))),
                                       dict(op='idle', node='r', call='canDisturb', entity='k')], nodes=dict(r=probe(), k=k))
    yield case('idle', 'disturb', [dict(op='idle', node='r', call='disturb', entity='k')], sweep=0, nodes=dict(r=probe(), k=k))
    yield case('idle', 'disturb by nobody', [dict(op='idle', node='r', call='disturb')], sweep=0, nodes=dict(r=probe()))
    yield case('idle', 'go idle', [dict(op='idle', node='r', call='goIdle'), dict(op='idle', node='r', call='goIdle')], sweep=0,
               nodes=dict(r=probe(False)))
    yield case('idle', 'create', [dict(op='radius', node='r'), dict(op='tick', node='r'), dict(op='init', node='r')], nodes=dict(r=probe()))


def handlers():
    k = dict(kind='player', location=[2701, 3700, 0])
    probe = lambda idle=True: dict(kind='idle', id=1265, activeId=1266, location=[2700, 3700, 0], idle=idle)
    reg = plugin('disturb')
    for name, nodes, answers in [('idle', dict(r=probe()), {}), ('active', dict(r=probe(False)), {}),
                                 ('plain npc', dict(r=dict(kind='npc', id=1)), {}), ('scenery', dict(r=dict(kind='scenery', id=1)), {})]:
        yield case('handlers', f'disturb {name}', reg + [dict(op='handler', via='option:disturb', node='r', option='disturb')], sweep=1,
                   nodes=dict(nodes, k=k), answers=answers)
        yield case('handlers', f'disturb walk {name}', reg + [dict(op='isWalk', via='option:disturb', node='r')], sweep=1,
                   nodes=dict(nodes, k=k), answers=dict(canSwing='NO_INTERACT'))
    yield case('handlers', 'disturb walk in range', reg + [dict(op='isWalk', via='option:disturb', node='r')],
               nodes=dict(r=probe()), answers=dict(canSwing='STILL_INTERACT'))
    yield case('handlers', 'disturb null node', reg + [dict(op='handler', via='option:disturb', option='disturb')])
    reg = plugin('haircut')
    yield case('handlers', 'haircut', reg + [dict(op='handler', via='npc:598:option:hair-cut', node='h', option='hair-cut')], sweep=1,
               nodes=dict(h=dict(kind='npc', id=598)))
    yield case('handlers', 'haircut walk', reg + [dict(op='isWalk', via='npc:598:option:hair-cut', node='h')], nodes=dict(h=dict(kind='npc', id=598)))
    yield case('handlers', 'haircut not an npc', reg + [dict(op='handler', via='npc:598:option:hair-cut', node='h', option='hair-cut')],
               nodes=dict(h=dict(kind='scenery', id=598)))
    yield case('handlers', 'haircut null', reg + [dict(op='handler', via='npc:598:option:hair-cut', option='hair-cut')])


def cases():
    yield from registration()
    yield from creation()
    yield from inits()
    yield from creatures()
    yield from talk()
    yield from banker()
    yield from idle()
    yield from handlers()


def injects(row):
    return False


def groups(cases):
    out = {}
    for c in cases:
        out[c['group']] = out.get(c['group'], 0) + 1
    return dict(sorted(out.items()))
