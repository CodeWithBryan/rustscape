"""Designed cases for the global-items oracle (see generate.py).

A case sets up the recording world (nodes, attributes, store archives, answers, values), registers one or more original
classes (`define`), then runs operations against what they registered: an InteractionListener callback (`interact`), an
item definition's option handler (`handler`, `props`), a registered dialogue plugin (`dialogueOpen`), a stored dialogue
file or plugin (`dialogue`), or a stored queued script, pulse, dialogue action or item-select callback (`task`). `sweep`
marks the operation whose recorded calls fail one at a time in the second pass.
"""

H = 'content.global.handlers.item.'
CLASSES = {
    'ectophial': H + 'EctophialListener',
    'tablets': H + 'EnchantJewelleryTabListener',
    'jewellery': H + 'EnchantedJewelleryListener',
    'explorer': H + 'ExplorersRingPlugin',
    'house': H + 'HouseTeleTabOptionListener',
    'teletabs': H + 'TeleTabsListener',
    'falador': H + 'FaladorShieldPlugin',
    'crystal': H + 'TeleportCrystalPlugin',
}
# The start-up manifest's order: content interfaces, then @Initializable plugins.
SCAN = ['ectophial', 'tablets', 'jewellery', 'explorer', 'house', 'teletabs', 'falador', 'crystal']

NEEDED_EVENTS = ['on', 'onUseWith', 'itemHandler', 'dialogueAdd', 'pulseClear', 'message', 'messages', 'openDialogue', 'sendOptions',
                 'interpreterClose', 'zoneTeleport', 'isQuestComplete', 'hasRequirement', 'scriptDelay', 'queueScript', 'lock',
                 'visualize', 'globalAudio', 'disabledTicks', 'closeInterface', 'delayScript', 'setTeleportLocation', 'animate',
                 'graphics', 'unlock', 'teleportEvent', 'finishTask', 'stopExecuting', 'removeItem', 'replaceSlot', 'log',
                 'addItemOrDrop', 'slayerHasTask', 'slayerMaster', 'slayerTaskName', 'slayerKillsRemaining', 'slayerTaskFlags',
                 'npcDialogue', 'setVarp', 'newItem', 'closeAllInterfaces', 'audio', 'forceAnimation', 'animationDuration',
                 'playerMonitorLog', 'inInventory', 'getAttribute', 'altarRequirement', 'teleportSend', 'houseExitLocation',
                 'housePreEnter', 'houseEnterLocation', 'housePostEnter', 'checkTeleport', 'openDialoguePlugin', 'newNpc',
                 'inventoryRemove', 'inventoryAdd', 'isTeleBlocked', 'addItem', 'faceLocation', 'username', 'storeArchive',
                 'storeGet', 'storePut', 'sendDialogue', 'runEnergy', 'updateRunEnergy', 'hasLevelStat', 'addDialogueAction',
                 'sendItemSelect', 'inventoryGet', 'alchemize', 'prayerEffect', 'setAttribute', 'submitWorld', 'lockAll']


def define(*names):
    return [dict(op='define', **{'class': CLASSES[n]}) for n in names]


def interact(n, node=None, used=None, with_=None):
    op = dict(op='interact', n=n)
    if node is not None:
        op['node'] = node
    if used is not None:
        op['used'] = used
    if with_ is not None:
        op['with'] = with_
    return op


def handler(via, node, option):
    op = dict(op='handler', via=via, option=option)
    if node is not None:
        op['node'] = node
    return op


def task(n, **extra):
    return dict(op='task', n=n, **extra)


def dialogue(n, button, component=0):
    return dict(op='dialogue', n=n, component=component, button=button)


def case(group, name, ops, sweep=None, **state):
    out = dict(group=group, name=name, ops=ops)
    out.update(state)
    return out, sweep


def item(id_, name=None, slot=3, amount=1):
    out = dict(kind='item', id=id_, slot=slot, amount=amount)
    if name is not None:
        out['name'] = name
    return out


# ---------------------------------------------------------------------- registration
def registration():
    for key in SCAN:
        yield case('registration', 'define ' + key, define(key), sweep=0)
    yield case('registration', 'scan order', define(*SCAN))
    yield case('registration', 'jewellery twice', define('jewellery', 'jewellery'))
    yield case('registration', 'tablets twice', define('tablets', 'tablets'))
    yield case('registration', 'teletabs twice', define('teletabs', 'teletabs', 'house', 'house'))
    yield case('registration', 'falador props', define('falador') + [dict(op='props', via='14577:option:operate', node='i')],
               nodes=dict(i=item(14577, 'Falador shield 1')))
    yield case('registration', 'crystal props', define('crystal') + [dict(op='props', via='6099:option:activate', node='i')],
               nodes=dict(i=item(6099, 'Teleport crystal (4)')))
    yield case('registration', 'unregistered handler', define('crystal') + [handler('6103:option:activate', None, 'activate')])


# ---------------------------------------------------------------------- enchanted jewellery
JEWELLERY = [
    # name, ids, item names, option count, location count, crumble
    ('RING_OF_SLAYING', [13281, 13282, 13283, 13284, 13285, 13286, 13287, 13288], 'Ring of slaying({})', 5, 4, True),
    ('RING_OF_DUELING', [2552, 2554, 2556, 2558, 2560, 2562, 2564, 2566], 'Ring of duelling({})', 4, 3, True),
    ('AMULET_OF_GLORY', [1712, 1710, 1708, 1706, 1704], 'Amulet of glory({})', 5, 4, False),
    ('AMULET_OF_GLORY_T', [10354, 10356, 10358, 10360, 10362], 'Amulet of glory(t{})', 5, 4, False),
    ('GAMES_NECKLACE', [3853, 3855, 3857, 3859, 3861, 3863, 3865, 3867], 'Games necklace({})', 5, 5, True),
    ('DIGSITE_PENDANT', [11194, 11193, 11192, 11191, 11190], 'Digsite pendant ({})', 0, 1, True),
    ('COMBAT_BRACELET', [11118, 11120, 11122, 11124, 11126], 'Combat bracelet({})', 5, 4, False),
    ('SKILLS_NECKLACE', [11105, 11107, 11109, 11111, 11113], 'Skills necklace({})', 5, 4, False),
    ('RING_OF_LIFE', [2570], 'Ring of life', 0, 1, True),
]


def jewellery_name(fmt, ids, index):
    if '{}' not in fmt:
        return fmt
    return fmt.format(len(ids) - index) if index < len(ids) - 1 or fmt.startswith(('Ring', 'Games', 'Digsite')) else fmt.replace('({})', '').replace('(t{})', ' (t)')


def full_teleport(base, prefix, equipped_option, button, item_node='i', stages=(0, 1, 2), extra_state=None):
    """rub/operate, the options, the choice, then the queued script's stages."""
    ops = base + [interact(0 if equipped_option == 'operate' else 1, item_node)]
    ops.append(dialogue(0, 0))
    ops.append(dialogue(0, button + 1))
    for stage in stages:
        ops.append(task(1, stage=stage))
    return ops


def jewellery():
    base = define('jewellery')
    # Every piece, every charge, both options, the first location (or the only one).
    for name, ids, fmt, options, locations, crumble in JEWELLERY:
        for index, id_ in enumerate(ids):
            label = jewellery_name(fmt, ids, index)
            for option, n in (('operate', 0), ('rub', 1)):
                state = dict(nodes=dict(i=item(id_, label)))
                if options:
                    ops = base + [interact(n, 'i'), dialogue(0, 0), dialogue(0, 1), task(1, stage=0), task(1, stage=1), task(1, stage=2)]
                else:
                    ops = base + [interact(n, 'i'), task(0, stage=0), task(0, stage=1), task(0, stage=2)]
                sweep = len(base) + 1 if (index in (0, len(ids) - 1) and option == 'rub') else None
                yield case('jewellery charges', f'{name} {index} {option}', ops, sweep=sweep, **state)
    # Every location of every piece with options, and the choice past the last location.
    for name, ids, fmt, options, locations, crumble in JEWELLERY:
        if not options:
            continue
        for button in range(0, options + 2):
            state = dict(nodes=dict(i=item(ids[1], jewellery_name(fmt, ids, 1))))
            ops = base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, button), task(1, stage=0), task(1, stage=1)]
            yield case('jewellery locations', f'{name} button {button}', ops, sweep=len(base) + 2 if button == 1 else None, **state)
    # Equipped (operate) teleports replace in the equipment.
    for name, ids, fmt, options, locations, crumble in JEWELLERY:
        for index in sorted({0, len(ids) - 2, len(ids) - 1}):
            if index < 0:
                continue
            state = dict(nodes=dict(i=item(ids[index], jewellery_name(fmt, ids, index), slot=12)))
            if options:
                ops = base + [interact(0, 'i'), dialogue(0, 0), dialogue(0, 2), task(1, stage=1), task(1, stage=2)]
            else:
                ops = base + [interact(0, 'i'), task(0, stage=1), task(0, stage=2)]
            yield case('jewellery equipped', f'{name} {index} equipped', ops, **state)
    slay = dict(nodes=dict(i=item(13288, 'Ring of slaying(1)', slot=5)))
    # The last ring of slaying: equipped removal, its failure and every fallback.
    for equipped in (True, False):
        n = 0 if equipped else 1
        for answers in ({}, {'removeItem': False}, {'removeItem': False, 'replaceSlot': None}, {'replaceSlot': None},
                        {'removeItem': [False, True]}, {'removeItem': [False, False], 'replaceSlot': None}, {'delayScript': True},
                        {'stopExecuting': False}):
            ops = base + [interact(n, 'i'), dialogue(0, 0), dialogue(0, 1), task(1, stage=0), task(1, stage=1), task(1, stage=2), task(1, stage=3)]
            yield case('jewellery slaying', f'last slaying ring equipped={equipped} {answers}', ops, sweep=len(base) + 4 if not answers else None,
                       answers=answers, **slay)
    # Crumbling jewellery whose removal fails.
    for name, ids, fmt, options, locations, crumble in JEWELLERY:
        if not crumble or name in ('RING_OF_SLAYING', 'RING_OF_LIFE'):
            continue
        for n in (0, 1):
            state = dict(nodes=dict(i=item(ids[-1], jewellery_name(fmt, ids, len(ids) - 1))))
            if options:
                ops = base + [interact(n, 'i'), dialogue(0, 0), dialogue(0, 1), task(1, stage=1)]
            else:
                ops = base + [interact(n, 'i'), task(0, stage=1)]
            yield case('jewellery crumble', f'{name} crumble fails {n}', ops, answers={'removeItem': False}, **state)
    # Refusals: the ring of life, a charge lost, the zone, Tarn's Lair, a negative choice.
    yield case('jewellery refusals', 'ring of life operate', base + [interact(0, 'i'), interact(1, 'i')], sweep=len(base),
               nodes=dict(i=item(2570, 'Ring of life')))
    for id_, label in ((1704, 'Amulet of glory'), (10362, 'Amulet of glory (t)'), (11126, 'Combat bracelet'), (11113, 'Skills necklace')):
        yield case('jewellery refusals', f'no charge {id_}', base + [interact(1, 'i'), interact(0, 'i')], sweep=len(base),
                   nodes=dict(i=item(id_, label)))
    yield case('jewellery refusals', 'not jewellery', base + [interact(1, 'i')], nodes=dict(i=item(995, 'Coins')))
    yield case('jewellery refusals', 'not an item', base + [interact(1, 'n')], nodes=dict(n=dict(kind='npc', id=1)))
    yield case('jewellery refusals', 'no name', base + [interact(1, 'i')], nodes=dict(i=item(1710)))
    yield case('jewellery refusals', 'one word name', base + [interact(1, 'i'), interact(0, 'j')],
               nodes=dict(i=item(1710, 'GLORY'), j=item(2554, ' Ring')))
    for answers in ({'zoneTeleport': False}, {'isQuestComplete:HAUNTED_MINE': False}, {'isQuestComplete:HAUNTED_MINE': True}):
        ops = base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 4), task(1, stage=0)]
        yield case('jewellery refusals', f'slaying tarn {answers}', ops, sweep=len(base) + 2, answers=answers,
                   nodes=dict(i=item(13282, 'Ring of slaying(7)')))
    for button in (0, -1):
        ops = base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, button)]
        yield case('jewellery refusals', f'glory button {button}', ops, nodes=dict(i=item(1710, 'Amulet of glory(3)')))
    yield case('jewellery refusals', 'zone refuses glory', base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 1)],
               answers={'zoneTeleport': False}, nodes=dict(i=item(1710, 'Amulet of glory(3)')))
    # The ring of slaying's progress dialogue.
    for answers in ({}, {'slayerHasTask': False}, {'slayerMaster': None}, {'slayerHasTask': False, 'slayerMaster': None},
                    {'slayerTaskName': 'black bear'}, {'slayerTaskName': 'cyclops', 'slayerKillsRemaining': 1}, {'slayerTaskName': 'dwarf'},
                    {'slayerTaskName': 'nechryael'}, {'slayerTaskName': 'turoth'}, {'slayerTaskName': 'tzhaar-mej'},
                    {'slayerTaskName': 'werewolf'}, {'slayerTaskName': 'wolf'}, {'slayerTaskName': 'guard dog'},
                    {'slayerTaskName': 'elf warrior'}, {'slayerTaskName': 'jelly'}, {'slayerTaskName': 'kalphite worker'},
                    {'slayerTaskName': 'scarab swarm'}, {'slayerTaskName': ''}, {'slayerTaskFlags': -37}, {'slayerTaskFlags': 1234567}):
        ops = base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 5)]
        yield case('jewellery slayer progress', f'progress {answers}', ops, sweep=len(base) + 2 if not answers else None,
                   answers=answers, nodes=dict(i=item(13283, 'Ring of slaying(6)')))
    yield case('jewellery slayer progress', 'dueling nowhere', base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 4)],
               nodes=dict(i=item(2552, 'Ring of duelling(8)')))
    # The dialogue file: stage after a failing sendOptions, a third handle, a slot changed between stages.
    yield case('jewellery dialogue', 'stages', base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 1), dialogue(0, 1), dialogue(0, 1)],
               nodes=dict(i=item(3855, 'Games necklace(7)')))
    yield case('jewellery dialogue', 'slot moves', base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 3), task(1, stage=0),
                                                           dict(op='set', nodes=dict(i=dict(slot=9))), task(1, stage=1)],
               nodes=dict(i=item(3855, 'Games necklace(7)')))
    yield case('jewellery dialogue', 'ticks', base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 3)],
               values=dict(ticks=2147483646), nodes=dict(i=item(3855, 'Games necklace(7)')))
    yield case('jewellery dialogue', 'no location', base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 3), task(1, stage=0)],
               nodes=dict(p=dict(location=None), i=item(3855, 'Games necklace(7)')))
    yield case('jewellery dialogue', 'unreplaced', base + [interact(1, 'i'), dialogue(0, 0), dialogue(0, 1), task(1, stage=1)],
               answers={'replaceSlot': None}, nodes=dict(i=item(1712, 'Amulet of glory(4)')))
    yield case('jewellery dialogue', 'digsite stop', base + [interact(1, 'i'), task(0, stage=1)], answers={'stopExecuting': False},
               nodes=dict(i=item(11190, 'Digsite pendant (1)')))


# ---------------------------------------------------------------------- jewellery enchanting tablets
TABLETS = [
    (8016, [(1637, 2550), (1656, 3853), (1694, 1727), (11072, 11074)]),
    (8017, [(1639, 2552), (1658, 5521), (1696, 1729), (11076, 11079)]),
    (8018, [(1641, 2568), (1660, 11194), (1698, 1725), (11085, 11088)]),
    (8019, [(1643, 2570), (1662, 11090), (1700, 1731), (11092, 11095)]),
    (8020, [(1645, 2572), (1664, 11113), (1702, 1704), (11115, 11126)]),
    (8021, [(6575, 6583), (6577, 11128), (6581, 6585), (11130, 11133)]),
]


def tablets():
    base = define('tablets')
    for t, (tablet, mapping) in enumerate(TABLETS):
        yield case('tablets', f'break {tablet}', base + [interact(5 * t, 't')], nodes=dict(t=item(tablet, 'Enchant tablet')))
        for k, (plain, enchanted) in enumerate(mapping):
            state = dict(nodes=dict(t=item(tablet, 'Enchant tablet', slot=0), j=item(plain, 'Jewel', slot=4)),
                         answers={'duration:4069': 3})
            ops = base + [interact(5 * t + 1 + k, used='t', with_='j'), task(0, stage=0)]
            yield case('tablets', f'enchant {plain}', ops, sweep=len(base) + 1 if k == 0 else None, **state)
    state = dict(nodes=dict(t=item(8016, 'Enchant sapphire', slot=0), j=item(1637, 'Sapphire ring', slot=4, amount=2),
                            k=item(1639, 'Emerald ring', slot=4)))
    yield case('tablets', 'no tablet', base + [interact(1, used='t', with_='j')], answers={'removeItem': False}, **state)
    for answers in ({'replaceSlot': None}, {'replaceSlot': 'k'}, {'replaceSlot': 'j'}, {'duration:4069': -4}, {'stopExecuting': False}):
        yield case('tablets', f'replace {answers}', base + [interact(1, used='t', with_='j'), task(0, stage=7)], sweep=len(base) + 1,
                   answers=answers, **state)
    yield case('tablets', 'no names', base + [interact(1, used='t', with_='j'), task(0, stage=0)], answers={'replaceSlot': 'k'},
               nodes=dict(t=item(8016, slot=0), j=item(1637, slot=4), k=item(1639, slot=2)))
    yield case('tablets', 'with not an item', base + [interact(1, used='t', with_='n'), task(0, stage=0)],
               nodes=dict(t=item(8016, 'Enchant sapphire'), n=dict(kind='npc', id=4)))


# ---------------------------------------------------------------------- teleport tablets
TELETABS = [8011, 8010, 8009, 8008, 8007, 8012, 13598, 13599, 13600, 13601, 13602, 13603, 13604, 13605, 13606, 13607, 13608, 13609,
            13610, 13611]


def teletabs():
    base = define('teletabs')
    for tab in TELETABS:
        attrs = dict(p={'Ardougne:teleport': True}) if tab == 8011 else {}
        yield case('teletabs', f'tab {tab}', base + [interact(0, 't')], sweep=len(base) if tab in (8011, 8010, 13599) else None,
                   attributes=attrs, nodes=dict(t=item(tab, 'Teleport tablet')))
        yield case('teletabs', f'tab {tab} refused', base + [interact(0, 't')],
                   answers={'altarRequirement': False}, attributes={}, nodes=dict(t=item(tab, 'Teleport tablet')))
        yield case('teletabs', f'tab {tab} teleport fails', base + [interact(0, 't')], answers={'teleportSend': False},
                   attributes=attrs, nodes=dict(t=item(tab, 'Teleport tablet')))
    for answers in ({'inInventory': False}, {'inInventory': [True, False]}):
        yield case('teletabs', f'inventory {answers}', base + [interact(0, 't')], answers=answers, nodes=dict(t=item(8007, 'Varrock teleport')))
    for value in (False, 1, 'yes'):
        yield case('teletabs', f'ardougne attribute {value}', base + [interact(0, 't')], attributes=dict(p={'Ardougne:teleport': value}),
                   nodes=dict(t=item(8011, 'Ardougne teleport')))
    yield case('teletabs', 'not a tab', base + [interact(0, 't')], nodes=dict(t=item(8013, 'Teleport to house')))
    yield case('teletabs', 'npc node', base + [interact(0, 'n')], nodes=dict(n=dict(kind='npc', id=8007)))
    base = define('house')
    for answers in ({}, {'houseExitLocation': [[2953, 3224, 0]]}, {'houseExitLocation': [[2953, 3224, 0]], 'inInventory': False},
                    {'houseExitLocation': [[2953, 3224, 0]], 'teleportSend': False},
                    {'houseExitLocation': [[2953, 3224, 0]], 'houseEnterLocation': None}):
        yield case('house tablet', f'house {answers}', base + [interact(0, 't')], sweep=len(base) if len(answers) == 1 else None,
                   answers=answers, nodes=dict(t=item(8013, 'Teleport to house')))


# ---------------------------------------------------------------------- teleport crystal
def crystal():
    base = define('crystal')
    for id_ in (6099, 6100, 6101, 6102):
        state = dict(nodes=dict(i=item(id_, 'Teleport crystal')))
        ops = base + [handler(f'{id_}:option:activate', 'i', 'activate'), dict(op='dialogueOpen', key=3999111, args=[1, id_]),
                      dialogue(0, 1)]
        yield case('teleport crystal', f'crystal {id_}', ops, sweep=len(base + [0, 0]), **state)
        yield case('teleport crystal', f'crystal {id_} refused teleport', ops, answers={'teleportSend': False}, **state)
    state = dict(nodes=dict(i=item(6101, 'Teleport crystal')))
    for answers in ({'hasRequirement:MOURNINGS_END_PART_I': False}, {'checkTeleport': False}, {'openDialoguePlugin': False}):
        yield case('teleport crystal', f'refused {answers}', base + [handler('6101:option:activate', 'i', 'activate')], sweep=len(base),
                   answers=answers, **state)
    yield case('teleport crystal', 'null node', base + [handler('6101:option:activate', None, 'activate')])
    yield case('teleport crystal', 'npc node', base + [handler('6101:option:activate', 'n', 'activate')], nodes=dict(n=dict(kind='npc', id=1)))
    for args in ([2, 6100], [1], [], ['x', 6100], [1, 'x'], [0, 6101]):
        yield case('teleport crystal', f'open {args}', base + [dict(op='dialogueOpen', key=3999111, args=args), dialogue(0, 1)])
    for button in (2, 3, 0):
        yield case('teleport crystal', f'button {button}', base + [dict(op='dialogueOpen', key=3999111, args=[1, 6100]), dialogue(0, button),
                                                                   dialogue(0, 1)], sweep=len(base) + 1 if button == 2 else None)
    yield case('teleport crystal', 'degrade fails', base + [dict(op='dialogueOpen', key=3999111, args=[1, 6102]), dialogue(0, 1)],
               answers={'inventoryRemove': False, 'inventoryAdd': False})
    yield case('teleport crystal', 'other key', base + [dict(op='dialogueOpen', key=3999112, args=[1, 6100])])


# ---------------------------------------------------------------------- ectophial
def ectophial():
    base = define('ectophial')
    state = dict(nodes=dict(e=item(4251, 'Ectophial', slot=6), f=item(4252, 'Ectophial', slot=6),
                            o=dict(kind='scenery', id=5282, location=[3660, 3520, 0])),
                 answers={'duration:832': 2, 'duration:9609': 5})
    ops = base + [interact(1, 'e'), task(0, stage=0), task(0, stage=1), task(0, stage=2), task(0, stage=3)]
    yield case('ectophial', 'empty and refill', ops, sweep=len(base), **state)
    for stage in (0, 1, 2):
        yield case('ectophial', f'stage {stage} sweep', ops, sweep=len(base) + 1 + stage, **state)
    for answers in ({'hasRequirement:GHOSTS_AHOY': False}, {'isTeleBlocked': True}, {'teleportSend': False}, {'removeItem': False},
                    {'addItem': False}, {'delayScript': True}, {'stopExecuting': False}, {'replaceSlot': None}):
        yield case('ectophial', f'empty {answers}', ops, answers=dict(state['answers'], **answers), nodes=state['nodes'])
    yield case('ectophial', 'refill', base + [interact(0, used='f', with_='o')], sweep=len(base), **state)
    for answers in ({'removeItem': False}, {'addItem': False}, {'duration:832': 0}):
        yield case('ectophial', f'refill {answers}', base + [interact(0, used='f', with_='o')], answers=dict(state['answers'], **answers),
                   nodes=state['nodes'])
    yield case('ectophial', 'not an item', base + [interact(1, 'o'), task(0, stage=0)], **state)


# ---------------------------------------------------------------------- explorer's ring
def explorer():
    base = define('explorer')
    ring = lambda id_: dict(nodes=dict(r=item(id_, "Explorer's ring")))
    archive = 'daily-explorer-ring'
    for id_ in (13560, 13561, 13562):
        for stored in (None, 0, 1, 2, 3, {'long': 1}, {'double': 1.9}, {'float': 2.5}, 'x', True, {'long': 4294967297}):
            stores = {archive: {'tracer:run': stored}} if stored is not None else {}
            yield case('explorers ring', f'replenish {id_} {stored}', base + [interact(0, 'r'), interact(0, 'r')],
                       sweep=len(base) if stored is None and id_ == 13561 else None, stores=stores, **ring(id_))
        yield case('explorers ring', f'replenish {id_} rested', base + [interact(0, 'r')], answers={'runEnergy': 100.0}, **ring(id_))
        yield case('explorers ring', f'cabbage {id_}', base + [interact(2, 'r'), interact(3, 'r')], sweep=len(base) + 1 if id_ == 13562 else None,
                   **ring(id_))
        yield case('explorers ring', f'cabbage {id_} refused', base + [interact(2, 'r'), interact(3, 'r')], answers={'teleportSend': False},
                   **ring(id_))
    yield case('explorers ring', 'replenish username', base + [interact(0, 'r')], values=dict(username='MiXeD Case'), **ring(13562))
    yield case('explorers ring', 'replenish max', base + [interact(0, 'r')], stores={archive: {'tracer:run': -2147483648}},
               answers={'runEnergy': 99.99}, **ring(13562))
    yield case('explorers ring', 'not a ring', base + [interact(0, 'r'), interact(3, 'r')], **ring(995))
    for stored in (None, 30, 1, 0, -3, {'long': 7}, {'double': 0.5}, 'x'):
        stores = {archive: {'tracer:alchs': stored}} if stored is not None else {}
        ops = base + [interact(1, 'r'), task(0, button=2), task(1, slot=4, option=0), task(1, slot=5, option=1)]
        yield case('explorers ring', f'alchemy {stored}', ops, sweep=len(base) + 2 if stored is None else None, stores=stores,
                   answers={'inventoryGet': ['coin', None]}, nodes=dict(r=item(13560, "Explorer's ring"), coin=item(1215, 'Dagger', slot=4)))
    for answers in ({'hasLevelStat': False}, {'alchemize': False}, {'inventoryGet': None}):
        ops = base + [interact(1, 'r'), task(0, button=2), task(1, slot=4, option=0)]
        yield case('explorers ring', f'alchemy {answers}', ops, answers=dict({'inventoryGet': 'coin'}, **answers),
                   nodes=dict(r=item(13560, "Explorer's ring"), coin=item(1215, 'Dagger', slot=4)))
    yield case('explorers ring', 'alchemy stale count', base + [interact(1, 'r'), task(0, button=1), dict(op='set', stores={archive: {'tracer:alchs': 5}}),
                                                                task(1, slot=4, option=0)],
               stores={archive: {'tracer:alchs': 12}}, answers={'inventoryGet': 'coin'},
               nodes=dict(r=item(13560, "Explorer's ring"), coin=item(1215, 'Dagger', slot=4)))


# ---------------------------------------------------------------------- falador shield
def falador():
    base = define('falador')
    for level, id_ in enumerate((14577, 14578, 14579)):
        state = dict(nodes=dict(s=item(id_, 'Falador shield')))
        for attr in (None, {'long': 0}, {'relative': -172800}, {'relative': -60}, {'relative': 60}, {'relative': 172800},
                     {'long': 9223372036854775807}):
            attributes = dict(p={'diary:falador:shield-restore-time': attr}) if attr is not None else {}
            yield case('falador shield', f'restore {id_} {attr}', base + [handler(f'{id_}:option:prayer-restore', 's', 'prayer-restore')],
                       sweep=len(base) if attr is None else None, attributes=attributes, **state)
        ops = base + [handler(f'{id_}:option:operate', 's', 'operate')] + [task(0) for _ in range(6)]
        yield case('falador shield', f'operate {id_}', ops, sweep=len(base) + 1, **state)
        yield case('falador shield', f'other option {id_}', base + [handler(f'{id_}:option:operate', 's', 'wear')], **state)
    yield case('falador shield', 'restore int attribute', base + [handler('14577:option:prayer-restore', 's', 'prayer-restore')],
               attributes=dict(p={'diary:falador:shield-restore-time': 5}), nodes=dict(s=item(14577, 'Falador shield 1')))
    yield case('falador shield', 'restore saved key', base + [handler('14578:option:prayer-restore', 's', 'prayer-restore')] * 2,
               nodes=dict(s=item(14578, 'Falador shield 2')))
    yield case('falador shield', 'unknown shield restore', base + [handler('14577:option:prayer-restore', 's', 'prayer-restore')],
               nodes=dict(s=item(14580, 'Falador shield 4')))
    yield case('falador shield', 'unknown shield operate', base + [handler('14577:option:operate', 's', 'operate'), task(0), task(0)],
               nodes=dict(s=item(14580, 'Falador shield 4')))
    yield case('falador shield', 'null option', base + [handler('14577:option:operate', 's', None)], nodes=dict(s=item(14577, 'Falador shield 1')))
    yield case('falador shield', 'null node', base + [handler('14577:option:operate', None, 'operate')])
    yield case('falador shield', 'npc node', base + [handler('14577:option:operate', 'n', 'operate')], nodes=dict(n=dict(kind='npc', id=1)))


def cases():
    yield from registration()
    yield from jewellery()
    yield from tablets()
    yield from teletabs()
    yield from crystal()
    yield from ectophial()
    yield from explorer()
    yield from falador()


def injects(row):
    return any(any(a.get('op') == 'fail' for a in h.get('actions', [])) for h in row.get('hooks', []))


def groups(cases_):
    out = {}
    for case_ in cases_:
        out[case_['group']] = out.get(case_['group'], 0) + 1
    return dict(sorted(out.items()))
