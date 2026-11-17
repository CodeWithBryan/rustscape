"""Designed cases for the global-items2 oracle (see generate.py).

A case sets up the recording world (nodes, attributes, item definitions, answers, values, generator seeds), registers
one or more original classes (`define`), then runs operations against what they registered: an InteractionListener
callback (`interact`: `on`, use-with and equip lambdas in registration order), an InterfaceListener callback
(`button`), an item definition's option handler (`handler`, `props`), a stored pulse, close event or queued script
(`task`), a skill selection (`select`), a login listener (`login`), a player save hook (`save`, `parse`) or a static
entry point other content calls (`call`). `sweep` marks the operation whose recorded calls fail one at a time in the
second pass.
"""

H = 'content.global.handlers.item.'
CLASSES = {
    'empty': H + 'EmptyOptionListener',
    'light': H + 'LightSourceListeners',
    'spade': H + 'SpadeDigOptionPlugin',
    'casket': H + 'CasketListener',
    'nestScript': H + 'BirdNestScript',
    'nestPlugin': H + 'BirdNestPlugin',
    'lamp': H + 'LampPlugin',
    'starter': H + 'StarterPackPlugin',
    'quest': H + 'ItemQuestRequirementListener',
    'morph': H + 'MorphItemListener',
    'uncharge': H + 'RingUnchargePlugin',
    'newspaper': H + 'NewspaperListener',
    'runepack': H + 'RunePackPlugin',
    'herbbox': H + 'HerbBoxPlugin',
    'cadava': 'rs09.game.interaction.object.CadavaPotionListener',
    'blamish': H + 'DrinkBlamishOilListener',
    'toys': H + 'ToyListeners',
    'horsey': H + 'ToyHorseListener',
    'water': H + 'withobject.WaterSourceListener',
    'sand': H + 'withobject.SandSourceListener',
    'chest': H + 'withobject.ChestKeyListener',
    'hatstand': H + 'withobject.HatStand',
    'sack': H + 'withobject.SackOnHay',
    'brawling': H + 'equipment.brawling_gloves.BrawlingGlovesManager',
    'fog': H + 'equipment.fistofguthixgloves.FOGGlovesListener',
    'gem': H + 'EnchantedGemListener',
    'petrock': H + 'PetRockListener',
    'diango': H + 'toys.DiangoOptionHandler',
    'reclaim': H + 'toys.DiangoReclaimInterface',
    'snowglobe': H + 'toys.SnowGlobePlugin',
}
# The start-up manifest's order: content interfaces, then @Initializable plugins (BirdNestPlugin is neither).
SCAN = ['nestScript', 'casket', 'blamish', 'empty', 'gem', 'quest', 'light', 'morph', 'newspaper', 'petrock', 'horsey', 'toys', 'brawling',
        'fog', 'chest', 'hatstand', 'sack', 'sand', 'water', 'cadava', 'herbbox', 'lamp', 'uncharge', 'runepack', 'spade', 'starter',
        'diango', 'reclaim', 'snowglobe']

NEEDED_EVENTS = ['on', 'onUseWith', 'onEquip', 'onInterface', 'onInterfaceClose', 'itemHandler', 'loginListener', 'persistPlayer',
                 'replaceSlot', 'audio', 'message', 'removeItem', 'produceGroundItem', 'animate', 'lock', 'spadeListener', 'digAction',
                 'submitWorld', 'log', 'inventory.add', 'itemMessage', 'newItem', 'inventory.freeSlots', 'inventory.replace', 'groundItem',
                 'setAttribute', 'openComponent', 'removeTabs', 'openDefaultTabs', 'removeAttribute', 'unlock', 'staticLevel', 'addExperience',
                 'inventory.remove', 'inventory.addAll', 'inventory.removeSlot', 'bank.freeSlots', 'bank.addAll', 'setCharge',
                 'itemDialogue', 'playerDialogue', 'openInterface', 'interfaceText', 'dialogueLines', 'sendDialogue', 'addItem',
                 'replaceScenery', 'hasRequirement', 'getQuests', 'questPoints', 'stopWalk', 'closeAllInterfaces', 'transformNpc',
                 'lockInteractions', 'visualize', 'getAttribute', 'pulseRun', 'jingle', 'chat', 'getDiary', 'diaryComplete', 'diaryUpdate',
                 'addItemOrDrop', 'amountInInventory', 'equipment.get', 'equipment.remove', 'debug', 'wildernessLevel',
                 'inventory.toArray', 'equipment.toArray', 'openDialogue', 'sendDialogues', 'activeTopic', 'sendOptions',
                 'slayerMaster', 'slayerTip', 'hasSlayerTask', 'slayerTaskName', 'slayerKillsRemaining', 'slayerTaskFlags', 'setVarp',
                 'username', 'interpreterClose', 'queueScript', 'stopExecuting', 'animationDuration', 'faceLocation', 'randomLocation',
                 'projectileLocation', 'projectile', 'npcHandler', 'componentPlugin', 'definePlugin', 'getOpened', 'componentClose',
                 'storageContains', 'inInventory', 'inEquipment', 'generateItems', 'closeInterface', 'equipment.containsItem', 'bank.containsItem']


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


def call(name, **extra):
    return dict(op='call', name=name, **extra)


def case(group, name, ops, sweep=None, **state):
    out = dict(group=group, name=name, ops=ops)
    out.update(state)
    return out, sweep


def item(id_, name=None, slot=3, amount=1, charge=None):
    out = dict(kind='item', id=id_, slot=slot, amount=amount)
    if name is not None:
        out['name'] = name
    if charge is not None:
        out['charge'] = charge
    return out


# ---------------------------------------------------------------------- registration
def registration():
    for key in SCAN + ['nestPlugin']:
        yield case('registration', 'define ' + key, define(key), sweep=0)
    yield case('registration', 'scan order', define(*SCAN))


    yield case('registration', 'empty with definitions', define('empty'), definitions=DEFINITIONS)
    yield case('registration', 'hat stand with definitions', define('hatstand'), definitions=DEFINITIONS)
    twelve = [dict(id=i, name=f'Odd potion {i}') for i in (17, 1, 18, 2, 19, 3, 20, 4, 21, 5, 22, 6)]
    yield case('registration', 'empty with twelve definitions', define('empty'), definitions=twelve)
    yield case('registration', 'hat stand with twelve definitions', define('hatstand'), definitions=twelve)
    yield case('registration', 'empty with thirteen definitions', define('empty'), definitions=twelve + [dict(id=23, name='Last potion')])
    for key in SCAN + ['nestPlugin']:
        yield case('registration', f'{key} twice', define(key, key))
    for key, via, node in (('spade', '952:option:dig', item(952, 'Spade')), ('lamp', '2528:option:rub', item(2528, 'Lamp')),
                           ('starter', '14775:option:open', item(14775, 'Starter pack')), ('herbbox', '14810:option:check', item(14810, 'Herb box')),
                           ('uncharge', '14807:option:uncharge', item(14807, 'Ring')), ('runepack', '14730:option:open', item(14730, 'Rune pack')),
                           ('nestPlugin', '5070:option:search', item(5070, "Bird's nest"))):
        yield case('registration', f'{key} props', define(key) + [dict(op='props', via=via, node='i')], nodes=dict(i=node))
    yield case('registration', 'unregistered handler', define('runepack') + [handler('14731:option:open', 'i', 'open')],
               nodes=dict(i=item(14731)))


# Item definitions a few cases read (EmptyItem's name scan and HatStand's head slot), in the HashMap's fill order.
DEFINITIONS = [
    dict(id=121, name='Attack potion(3)'), dict(id=4608, name='Super kebab'), dict(id=11788, name='Voice of doom potion'),
    dict(id=3410, name='Serum 207 (3)'), dict(id=2430, name='Restore potion(4)'), dict(id=1, name='Toolkit', slot=3),
    dict(id=1153, name='Iron full helm', slot=0), dict(id=1155, name='Bronze full helm', slot=0), dict(id=5935, name='Coconut milk'),
    dict(id=10915, name='Mixture - step 1(1)'), dict(id=99999, name='Balm of something'), dict(id=7000, name='Mixed cure', slot=4),
    dict(id=40, name='Poisoned SERUM'), dict(id=41, name=''), dict(id=1201, name='Rune kiteshield', slot=5),
]


# ---------------------------------------------------------------------- EmptyOptionListener
def empty():
    base = define('empty')
    rows = [(1933, 'Pot of flour'), (1783, 'Bucket of sand'), (1929, 'Bucket of water'), (7692, 'Pot of tea (4)'), (2175, 'Burnt gnomebowl'),
            (121, 'Attack potion(3)'), (2436, 'Super attack(4)'), (10915, 'Mixture - step 1(1)'), (91, 'Guam potion (unf)'),
            (3410, 'Serum 207 (3)'), (5935, 'Coconut milk'), (4608, 'Super kebab'), (9999, 'Thing'), (1927, 'Zamorak brew(3)'),
            (6032, 'Compost'), (11262, 'Imp repellent'), (7094, 'Burnt mushroom'), (2329, 'Burnt pie'), (40, 'POISON'), (41, 'Relicym balm'),
            (42, 'cure'), (43, 'mix'), (6034, 'Supercompost'), (6032, 'Superb compost')]
    for k, (id_, name) in enumerate(rows):
        yield case('empty', f'empty {id_} {name}', base + [interact(0, 'i')], sweep=len(base) if k in (0, 1, 5) else None,
                   nodes=dict(i=item(id_, name, slot=7)))
    yield case('empty', 'no name', base + [interact(0, 'i')], nodes=dict(i=item(1933, slot=7)))
    yield case('empty', 'npc node', base + [interact(0, 'n')], nodes=dict(n=dict(kind='npc', id=1933, name='Pot of flour')))
    yield case('empty', 'npc potion', base + [interact(0, 'n')], nodes=dict(n=dict(kind='npc', id=1, name='Potion man')))
    yield case('empty', 'replace answered', base + [interact(0, 'i')], answers={'replaceSlot': None},
               nodes=dict(i=item(1929, 'Bucket of water', slot=2)))


# ---------------------------------------------------------------------- LightSourceListeners
def light():
    base = define('light')
    defs = [dict(id=33, name='Lit candle'), dict(id=594, name='Lit torch'), dict(id=5013, name='Mining helmet'), dict(id=4550, name='Bullseye lantern')]
    temple = [2650, 9760, 0]
    for id_, name, where, held, removed in (
            (33, 'Lit candle', None, ['i'], True), (5013, 'Mining helmet', None, ['i'], True), (5013, 'Mining helmet', None, ['i'], False),
            (594, 'Lit torch', temple, ['i'], True), (594, 'Lit torch', temple, ['j', 'i'], True), (5013, 'Mining helmet', temple, ['i'], True),
            (4550, 'Bullseye lantern', temple, [], True)):
        state = dict(nodes=dict(i=item(id_, name), j=item(33, 'Lit candle'), p=dict(location=where or [3222, 3218, 0])),
                     answers={'inventory.toArray': held, 'equipment.toArray': ['i'], 'removeItem': removed}, definitions=defs)
        yield case('light source', f'drop {id_} at {where} holding {held} removed {removed}', base + [interact(0, 'i')],
                   sweep=len(base) if where and held == ['i'] else None, **state)
    yield case('light source', 'nothing lit', base + [interact(0, 'i')], definitions=defs,
               nodes=dict(i=item(33, 'Lit candle')), answers={'inventory.toArray': [], 'equipment.toArray': []})
    yield case('light source', 'lit in equipment', base + [interact(0, 'i')], definitions=defs,
               nodes=dict(i=item(33, 'Lit candle'), k=item(36, 'Candle'), j=item(5013, 'Mining helmet'), p=dict(location=temple)),
               answers={'inventory.toArray': ['k', 'x'], 'equipment.toArray': ['x', 'j']})
    yield case('light source', 'unnamed product', base + [interact(0, 'i')],
               nodes=dict(i=item(33, 'Lit candle'), p=dict(location=temple)), answers={'inventory.toArray': ['i']})
    yield case('light source', 'no location', base + [interact(0, 'i')], definitions=defs,
               nodes=dict(i=item(33), p=dict(location=None)), answers={'inventory.toArray': ['i']})


# ---------------------------------------------------------------------- spade, SpadeDigListener, DigSpadeHandler
def spade():
    base = define('spade')
    dig = handler('952:option:dig', 's', 'dig')
    here = dict(x=3222, y=3218, z=0)
    there = dict(x=3000, y=3000, z=0)
    s = dict(nodes=dict(s=item(952, 'Spade')))
    yield case('spade', 'nothing', base + [dig], sweep=len(base), **s)
    yield case('spade', 'listener', base + [call('spadeRegister', tag='a', **here), call('spadeRegister', tag='b', **here), dig],
               sweep=len(base) + 2, **s)
    yield case('spade', 'listener elsewhere', base + [call('spadeRegister', tag='a', **there), dig], **s)
    yield case('spade', 'action', base + [call('digRegister', tag='a', **here), call('digRegister', tag='b', **here), dig, task(0)],
               sweep=len(base) + 2, **s)
    yield case('spade', 'listener before action', base + [call('digRegister', tag='a', **here), call('spadeRegister', tag='b', **here), dig], **s)
    yield case('spade', 'action other plane', base + [call('digRegister', tag='a', x=3222, y=3218, z=1), dig], **s)
    yield case('spade', 'run listener', [call('spadeRegister', tag='a', **there), call('spadeRun', **there), call('spadeRun', **here)])
    yield case('spade', 'no location', base + [call('digRegister', tag='a', **here), dig], nodes=dict(s=item(952), p=dict(location=None)))
    yield case('spade', 'register failing log', [call('digRegister', tag='a', **here), call('digRegister', tag='b', **here)], sweep=1)


# ---------------------------------------------------------------------- casket
CASKET_NAMES = [dict(id=995, name='Coins'), dict(id=1623, name='Uncut sapphire'), dict(id=1621, name='Uncut emerald'),
                dict(id=1619, name='Uncut ruby'), dict(id=1617, name='Uncut diamond'), dict(id=1454, name='Cosmic talisman'),
                dict(id=987, name='Loop half of a key'), dict(id=985, name='Tooth half of a key')]


def casket():
    base = define('casket')
    c = dict(nodes=dict(c=item(405, 'Casket')), definitions=CASKET_NAMES)
    for seed in range(40):
        yield case('casket', f'open seed {seed}', base + [interact(0, 'c')], sweep=len(base) if seed in (0, 7) else None,
                   seeds=dict(random=1000 + seed, shuffle=2000 + seed, tlr=3000 + seed * 7919), **c)
    yield case('casket', 'twice', base + [interact(0, 'c'), interact(0, 'c')], **c)
    yield case('casket', 'not removed', base + [interact(0, 'c')], answers={'removeItem': False}, **c)
    yield case('casket', 'no names', base + [interact(0, 'c')], nodes=dict(c=item(405)))
    yield case('casket', 'npc', base + [interact(0, 'n')], nodes=dict(n=dict(kind='npc', id=405)))


# ---------------------------------------------------------------------- bird nests
NEST_NAMES = [dict(id=5076, name='Bird egg'), dict(id=5078, name='Bird egg'), dict(id=5077, name='Bird egg'), dict(id=11964, name='Raven egg'),
              dict(id=5312, name='Acorn'), dict(id=5283, name='Apple tree seed'), dict(id=5313, name='Willow seed'),
              dict(id=5284, name='Banana tree seed'), dict(id=5285, name='Orange tree seed'), dict(id=5286, name='Curry tree seed'),
              dict(id=5314, name='Maple seed'), dict(id=5287, name='Pineapple seed'), dict(id=5288, name='Papaya tree seed'),
              dict(id=5315, name='Yew seed'), dict(id=5289, name='Palm tree seed'), dict(id=5290, name='Calquat tree seed'),
              dict(id=5317, name='Spirit seed'), dict(id=5316, name='Magic seed'), dict(id=1635, name='Gold ring'),
              dict(id=1637, name='Sapphire ring'), dict(id=1639, name='Emerald ring'), dict(id=1641, name='Ruby ring'),
              dict(id=1643, name='Diamond ring'), dict(id=5318, name='Potato seed'), dict(id=5320, name='Sweetcorn seed'),
              dict(id=5322, name='Tomato seed'), dict(id=5324, name='Cabbage seed'), dict(id=5100, name='Limpwurt seed'),
              dict(id=5319, name='Onion seed'), dict(id=5323, name='Strawberry seed'), dict(id=5321, name='Watermelon seed'),
              dict(id=5295, name='Ranarr seed')]
NESTS = [('SEED', 5073), ('RING', 5074), ('RED', 5070), ('GREEN', 5071), ('BLUE', 5072), ('RAVEN', 11966), ('WYSON', 7413)]


def nests():
    base = define('nestScript')
    for name, id_ in NESTS:
        for seed in range(6 if name in ('SEED', 'RING', 'WYSON') else 1):
            yield case('bird nest', f'search {name} seed {seed}', base + [interact(0, 'n')], sweep=len(base) if seed == 0 else None,
                       seeds=dict(random=40 + seed, shuffle=80 + seed * 3, tlr=5), definitions=NEST_NAMES,
                       nodes=dict(n=item(id_, "Bird's nest", slot=9)))
    yield case('bird nest', 'full', base + [interact(0, 'n')], answers={'inventory.freeSlots': 0}, definitions=NEST_NAMES,
               nodes=dict(n=item(5073, "Bird's nest", slot=9)))
    yield case('bird nest', 'not a nest', base + [interact(0, 'n')], nodes=dict(n=item(5075, "Bird's nest")))
    yield case('bird nest', 'npc', base + [interact(0, 'n')], nodes=dict(n=dict(kind='npc', id=5073)))
    yield case('bird nest', 'unnamed loot', base + [interact(0, 'n')], nodes=dict(n=item(5070, "Bird's nest")))
    plugin = define('nestPlugin')
    yield case('bird nest', 'plugin search', plugin + [handler('5074:option:search', 'n', 'search')], sweep=len(plugin), definitions=NEST_NAMES,
               nodes=dict(n=item(5074, "Bird's nest", slot=2)))
    yield case('bird nest', 'plugin not a nest', plugin + [handler('5074:option:search', 'n', 'search')], nodes=dict(n=item(995)))
    yield case('bird nest', 'plugin npc', plugin + [handler('5074:option:search', 'n', 'search')], nodes=dict(n=dict(kind='npc', id=5074)))
    for seed in range(12):
        yield case('bird nest', f'drop seed {seed}', [call('nestDrop')], sweep=0 if seed == 0 else None, seeds=dict(random=seed, shuffle=seed + 100, tlr=1))
        yield case('bird nest', f'random nest seed {seed}', [call('nestRandom', wyson=True), call('nestRandom', wyson=False)],
                   seeds=dict(random=seed * 13, shuffle=seed * 17, tlr=1))
    yield case('bird nest', 'for nest', [call('nestFor', node='a'), call('nestFor', node='b'), call('nestSearch', nest='GREEN', node='a')],
               definitions=NEST_NAMES, nodes=dict(a=item(5071, 'Nest', slot=4), b=item(995, 'Coins')))


# ---------------------------------------------------------------------- lamps
LAMPS = [2528, 12627, 12628, 13227, 4447, 11137, 11139, 11141, 11753, 11754, 11755, 11185, 11186, 11187, 14580, 14581, 14582, 14574, 14575,
         14576, 14633, 14634, 14635]


def lamps():
    base = define('lamp')
    for id_ in LAMPS:
        via = f'{id_}:option:rub'
        ops = base + [handler(via, 'l', 'rub'), dict(op='select', via=via, skill=5)]
        yield case('lamp', f'rub {id_}', ops, sweep=len(base) + 1 if id_ in (2528, 11141) else None, answers={'staticLevel': 45},
                   nodes=dict(l=item(id_, 'Lamp', slot=6)))
    l = dict(nodes=dict(l=item(11141, 'Antique lamp', slot=6)))
    via = '11141:option:rub'
    yield case('lamp', 'rub and close', base + [handler(via, 'l', 'rub'), task(0, component=134)], sweep=len(base) + 1, **l)
    yield case('lamp', 'too low', base + [handler(via, 'l', 'rub'), dict(op='select', via=via, skill=3)], sweep=len(base) + 1,
               answers={'staticLevel': 49}, **l)
    yield case('lamp', 'not removed', base + [handler(via, 'l', 'rub'), dict(op='select', via=via, skill=3)],
               answers={'staticLevel': 50, 'inventory.remove': False}, **l)
    yield case('lamp', 'genie no lamp attribute', base + [dict(op='select', via='2528:option:rub', skill=0)], answers={'staticLevel': 7})
    yield case('lamp', 'genie bad skill', base + [handler('2528:option:rub', 'l', 'rub'), dict(op='select', via='2528:option:rub', skill=24)],
               answers={'staticLevel': [1, 99]}, nodes=dict(l=item(2528, 'Lamp')))
    yield case('lamp', 'low bad skill', base + [handler(via, 'l', 'rub'), dict(op='select', via=via, skill=25)], answers={'staticLevel': 1}, **l)
    yield case('lamp', 'unknown lamp', base + [handler(via, 'l', 'rub'), dict(op='select', via=via, skill=1)], nodes=dict(l=item(995, 'Coins')))
    yield case('lamp', 'npc lamp', base + [handler(via, 'n', 'rub'), dict(op='select', via=via, skill=1)], nodes=dict(n=dict(kind='npc', id=11141)))
    yield case('lamp', 'null node', base + [handler(via, None, 'rub'), dict(op='select', via=via, skill=1)], answers={'staticLevel': 3})
    yield case('lamp', 'other plugin select', define('runepack') + [dict(op='select', via='14730:option:open', skill=1)])
    yield case('lamp', 'close then select', base + [handler(via, 'l', 'rub'), task(0, component=134), dict(op='select', via=via, skill=2)],
               answers={'staticLevel': 60}, **l)


# ---------------------------------------------------------------------- starter pack, ring uncharge, rune packs
def packs():
    base = define('starter')
    via = '14775:option:open'
    s = dict(nodes=dict(b=item(14775, 'Starter pack', slot=1)))
    for answers in ({}, {'inventory.freeSlots': 17}, {'inventory.freeSlots': [18, 5]}, {'inventory.freeSlots': 18, 'inventory.remove': False},
                    {'inventory.freeSlots': -3}):
        yield case('starter pack', f'open {answers}', base + [handler(via, 'b', 'open')], sweep=len(base) if not answers else None,
                   answers=answers, **s)
    yield case('starter pack', 'other option', base + [handler(via, 'b', 'drop')], **s)
    yield case('starter pack', 'null option', base + [handler(via, 'b', None)], **s)
    yield case('starter pack', 'npc', base + [handler(via, 'n', 'open')], nodes=dict(n=dict(kind='npc', id=14775)))
    base = define('uncharge')
    for id_ in (14807, 14808, 14809, 14810):
        yield case('ring uncharge', f'uncharge {id_}', base + [handler(f'{id_}:option:uncharge', 'r', 'uncharge')], sweep=len(base) if id_ == 14808 else None,
                   nodes=dict(r=item(id_, 'Ring')))
    yield case('ring uncharge', 'not removed', base + [handler('14807:option:uncharge', 'r', 'uncharge')], answers={'inventory.remove': False},
               nodes=dict(r=item(14807, 'Ring')))
    yield case('ring uncharge', 'other item', base + [handler('14807:option:uncharge', 'r', 'uncharge')], nodes=dict(r=item(995, 'Coins')))
    yield case('ring uncharge', 'null node', base + [handler('14807:option:uncharge', None, 'uncharge')])
    yield case('ring uncharge', 'npc', base + [handler('14807:option:uncharge', 'n', 'uncharge')], nodes=dict(n=dict(kind='npc', id=1)))
    base = define('runepack')
    for id_ in (14730, 14732, 14734, 14736, 14738, 14740):
        yield case('rune pack', f'open {id_}', base + [handler(f'{id_}:option:open', 'r', 'open')], sweep=len(base) if id_ == 14736 else None,
                   nodes=dict(r=item(id_, 'Rune pack')))
    yield case('rune pack', 'not removed', base + [handler('14730:option:open', 'r', 'open')], answers={'inventory.remove': False},
               nodes=dict(r=item(14730, 'Rune pack')))
    yield case('rune pack', 'other item', base + [handler('14730:option:open', 'r', 'open')], nodes=dict(r=item(995, 'Coins')))
    yield case('rune pack', 'null node', base + [handler('14730:option:open', None, 'open')])
    yield case('rune pack', 'npc', base + [handler('14730:option:open', 'n', 'open')], nodes=dict(n=dict(kind='npc', id=1)))


# ---------------------------------------------------------------------- herb box
def herbbox():
    base = define('herbbox')
    for charge in (1000, 995, 991, 990, 989, 1001, 980):
        b = dict(nodes=dict(b=item(14810, 'Herb box', slot=8, charge=charge)))
        for option in ('check', 'take-one', 'bank-all', 'other'):
            yield case('herb box', f'{option} charge {charge}', base + [handler(f'14810:option:{option if option != "other" else "check"}', 'b', option)],
                       sweep=len(base) if charge == 995 and option != 'other' else None, seeds=dict(random=charge, shuffle=charge * 3, tlr=0), **b)
    b = dict(nodes=dict(b=item(14810, 'Herb box', slot=8, charge=996)))
    yield case('herb box', 'bank full', base + [handler('14810:option:bank-all', 'b', 'bank-all')], answers={'bank.freeSlots': 5}, **b)
    yield case('herb box', 'bank exact', base + [handler('14810:option:bank-all', 'b', 'bank-all')], answers={'bank.freeSlots': 6}, **b)
    yield case('herb box', 'bank not removed', base + [handler('14810:option:bank-all', 'b', 'bank-all')], answers={'inventory.removeSlot': False}, **b)
    yield case('herb box', 'inventory full', base + [handler('14810:option:take-one', 'b', 'take-one')], answers={'inventory.freeSlots': 0}, **b)
    yield case('herb box', 'take until empty', base + [handler('14810:option:take-one', 'b', 'take-one')] * 7, **b)
    yield case('herb box', 'null option', base + [handler('14810:option:check', 'b', None)], **b)
    yield case('herb box', 'npc', base + [handler('14810:option:check', 'n', 'check')], nodes=dict(n=dict(kind='npc', id=1)))


# ---------------------------------------------------------------------- simple listeners
def simple():
    base = define('cadava')
    yield case('cadava potion', 'drink and look', base + [interact(0, 'i'), interact(1, 'i')], sweep=len(base), nodes=dict(i=item(756, 'Cadava potion')))
    base = define('blamish')
    yield case('blamish oil', 'drink', base + [interact(0, 'i')], sweep=len(base), nodes=dict(i=item(1582, 'Blamish oil')))
    base = define('newspaper')
    yield case('newspaper', 'read both', base + [interact(0, 'i'), interact(1, 'j')], sweep=len(base), nodes=dict(i=item(11169), j=item(7922)))
    yield case('newspaper', 'flyer sweep', base + [interact(1, 'j')], sweep=len(base), nodes=dict(j=item(7922)))
    base = define('hatstand')
    yield case('hat stand', 'use', base + [interact(0, used='h', with_='s')], sweep=len(base), definitions=DEFINITIONS,
               nodes=dict(h=item(1153, 'Iron full helm'), s=dict(kind='scenery', id=374)))
    base = define('sack')
    s = dict(nodes=dict(k=item(5418, 'Empty sack', slot=5), h=dict(kind='scenery', id=300)))
    yield case('sack on hay', 'fill', base + [interact(0, used='k', with_='h')], sweep=len(base), **s)
    yield case('sack on hay', 'not removed', base + [interact(0, used='k', with_='h')], answers={'removeItem': False}, **s)
    yield case('sack on hay', 'npc used', base + [interact(0, used='n', with_='h')], nodes=dict(n=dict(kind='npc', id=5418), h=dict(kind='scenery', id=300)))
    base = define('chest')
    s = dict(nodes=dict(k=item(432, 'Chest key'), c=dict(kind='scenery', id=2079, location=[3000, 3200, 0])))
    yield case('chest key', 'unlock', base + [interact(0, used='k', with_='c')], sweep=len(base), **s)
    yield case('chest key', 'not removed', base + [interact(0, used='k', with_='c')], answers={'removeItem': False}, **s)
    yield case('chest key', 'with npc', base + [interact(0, used='k', with_='n')], nodes=dict(k=item(432, 'Chest key'), n=dict(kind='npc', id=2079)))


# ---------------------------------------------------------------------- equip requirements
QUESTS_USED = ['THE_FREMENNIK_TRIALS', 'THE_FREMENNIK_ISLES', 'ANIMAL_MAGNETISM', 'LOST_CITY', 'LEGENDS_QUEST', 'PRIEST_IN_PERIL',
               'ANOTHER_SLICE_OF_HAM', 'DESERT_TREASURE', 'ELEMENTAL_WORKSHOP_I', 'ROVING_ELVES', 'DRAGON_SLAYER', 'MONKEY_MADNESS',
               'RECIPE_FOR_DISASTER', 'SMOKING_KILLS', 'REGICIDE', 'DEATH_PLATEAU', 'HORROR_FROM_THE_DEEP', 'ICTHLARINS_LITTLE_HELPER',
               'RECRUITMENT_DRIVE', 'THE_SLUG_MENACE', 'SUMMERS_END', 'HEROES_QUEST', 'SHADOW_OF_THE_STORM']
EQUIP_COUNT = 166


def equips():
    base = define('quest')
    ops = base + [interact(n, 'i') for n in range(EQUIP_COUNT)]
    yield case('equip requirements', 'every lambda met', ops, nodes=dict(i=item(1, 'Thing')))
    yield case('equip requirements', 'every lambda unmet', ops, answers={f'hasRequirement:{q}': False for q in QUESTS_USED},
               values=dict(), nodes=dict(i=item(1, 'Thing')))
    for points, questPoints in ((6, [1, 2, 3]), (5, [1, 2, 3]), (0, []), (-1, []), (7, [4, 3])):
        yield case('equip requirements', f'quest cape points {points} of {questPoints}', base + [interact(81, 'i'), interact(82, 'i')],
                   sweep=len(base) if points == 5 else None, answers={'points': points, 'questPoints': questPoints}, nodes=dict(i=item(9813, 'Quest cape')))
    yield case('equip requirements', 'fremennik sweep', base + [interact(0, 'i')], sweep=len(base), nodes=dict(i=item(3751, 'Helm')))


# ---------------------------------------------------------------------- morphing rings
def morph():
    base = define('morph')
    for id_, n in ((6583, 0), (7927, 1)):
        for seed in range(4):
            yield case('morph', f'equip {id_} seed {seed}', base + [interact(n, 'r'), dict(op='button', n=0, opcode=155, button=3, slot=-1, item=-1),
                                                                     dict(op='button', n=1)],
                       sweep=len(base) if seed == 0 else None, seeds=dict(random=1, shuffle=2, tlr=seed * 1000003), nodes=dict(r=item(id_, 'Ring')))
    for button in (0, 1, 2, 4):
        yield case('morph', f'button {button}', base + [dict(op='button', n=0, opcode=155, button=button, slot=0, item=0)])
    yield case('morph', 'close sweep', base + [dict(op='button', n=1)], sweep=len(base))
    yield case('morph', 'other ring', base + [interact(0, 'r')], nodes=dict(r=item(995, 'Coins')))


# ---------------------------------------------------------------------- toys
def toys():
    base = define('toys')
    t = lambda id_: dict(nodes=dict(t=item(id_, 'Toy')))
    yield case('toys', 'chocatrice', base + [interact(0, 't')], sweep=len(base), **t(12645))
    for id_ in (6867, 6866, 6865, 995):
        for option in ('jump', 'walk', 'bow', 'dance', 'wave', None):
            attrs = dict(p={'interact:option': option}) if option else {}
            yield case('toys', f'marionette {id_} {option}', base + [interact(1, 't')], sweep=len(base) if id_ == 6866 and option == 'bow' else None,
                       attributes=attrs, **t(id_))
    yield case('toys', 'marionette int option', base + [interact(1, 't')], attributes=dict(p={'interact:option': 5}), **t(6867))
    yield case('toys', 'reindeer hat', base + [interact(2, 't')], sweep=len(base), **t(10507))
    yield case('toys', 'snow globe', base + [interact(3, 't'), task(0), task(1)], sweep=len(base), **t(11949))
    yield case('toys', 'snow globe stages', base + [interact(3, 't'), task(0), task(0), task(1), task(1)], sweep=len(base) + 1, **t(11949))
    yield case('toys', 'spinning plate', base + [interact(4, 't'), task(0), task(0)], sweep=len(base), **t(4613))
    yield case('toys', 'kite', base + [interact(5, 't')], sweep=len(base), **t(12844))
    for option in ('play', 'loop', 'walk', 'crazy', 'spin', None):
        attrs = dict(p={'interact:option': option}) if option else {}
        yield case('toys', f'yo-yo {option}', base + [interact(6, 't')], sweep=len(base) if option == 'crazy' else None, attributes=attrs, **t(4079))
    for option in ('talk-at', 'display', 'question', None):
        attrs = dict(p={'interact:option': option}) if option else {}
        yield case('toys', f'zombie head {option}', base + [interact(7, 't')], sweep=len(base) if option != 'question' else None, attributes=attrs,
                   **t(6722))
    yield case('toys', 'rubber chicken', base + [interact(8, 't')], sweep=len(base), **t(4566))
    base = define('horsey')
    for id_ in (2520, 2522, 2524, 2526):
        for seed in range(3):
            yield case('toy horsey', f'play {id_} seed {seed}', base + [interact(0, 't')], sweep=len(base) if seed == 0 and id_ == 2520 else None,
                       seeds=dict(random=1, shuffle=2, tlr=seed * 977 + id_), **t(id_))
    yield case('toy horsey', 'other id', base + [interact(0, 't')], **t(995))


# ---------------------------------------------------------------------- water and sand
VESSELS = [(1925, 'Bucket', 1929), (229, 'Vial', 227), (1935, 'Jug', 1937), (1923, 'Bowl', 1921), (5333, 'Watering can(1)', 5340),
           (1831, 'Waterskin(0)', 1823), (6667, 'Fishbowl', 6668), (7688, 'Kettle', 7690), (1980, 'Empty cup', 4458)]


def water():
    base = define('water')
    for id_, name, _ in VESSELS:
        for with_name, with_id in (('Fountain', 153), ('Well', 878)):
            s = dict(nodes=dict(v=item(id_, name, slot=4), w=dict(kind='scenery', id=with_id, name=with_name)))
            ops = base + [interact(0, used='v', with_='w'), task(0), task(0)]
            yield case('water source', f'fill {name} at {with_name}', ops, sweep=len(base) + 1 if id_ in (1925, 229) else None,
                       answers={'amountInInventory': [2, 0]}, **s)
    s = dict(nodes=dict(v=item(1925, 'Bucket', slot=4), w=dict(kind='scenery', id=11661, name='Waterpump')))
    for answers in ({}, {'diaryComplete': True}):
        yield case('water source', f'falador pump {answers}', base + [interact(0, used='v', with_='w')], sweep=len(base) if not answers else None,
                   answers=answers, **s)
    s = dict(nodes=dict(v=item(229, 'Vial', slot=4), w=dict(kind='scenery', id=11661, name='Waterpump')))
    yield case('water source', 'vial at falador pump', base + [interact(0, used='v', with_='w')], **s)
    yield case('water source', 'not removed', base + [interact(0, used='v', with_='w'), task(0)], answers={'removeItem': False, 'amountInInventory': 3},
               nodes=dict(v=item(1935, 'Jug', slot=4), w=dict(kind='scenery', id=153, name='Fountain')))
    yield case('water source', 'not a vessel', base + [interact(0, used='v', with_='w')],
               nodes=dict(v=item(995, 'Coins'), w=dict(kind='scenery', id=153, name='Fountain')))
    yield case('water source', 'unnamed vessel at well', base + [interact(0, used='v', with_='w')],
               nodes=dict(v=item(229), w=dict(kind='scenery', id=878, name='well')))
    yield case('water source', 'unnamed source', base + [interact(0, used='v', with_='w')],
               nodes=dict(v=item(1925, 'Bucket'), w=dict(kind='scenery', id=153)))
    yield case('water source', 'parenthesis name', base + [interact(0, used='v', with_='w'), task(0)],
               nodes=dict(v=item(5339, 'WATERING Can(7)', slot=1), w=dict(kind='scenery', id=153, name='WELL of souls')))
    base = define('sand')
    s = dict(nodes=dict(b=item(1925, 'Bucket', slot=2), p0=dict(kind='scenery', id=2645)))
    for count in (3, 1, 0, -2):
        ops = base + [interact(0, used='b', with_='p0')] + [task(0)] * 4
        yield case('sand source', f'fill {count}', ops, sweep=len(base) + 1 if count == 3 else None, answers={'amountInInventory': count}, **s)
    yield case('sand source', 'refusals', base + [interact(0, used='b', with_='p0'), task(0), task(0), task(0)],
               answers={'amountInInventory': 3, 'removeItem': [True, False, True], 'addItem': [False, True]}, **s)


# ---------------------------------------------------------------------- gloves
FOG = list(range(12856, 12866))
MAX = [100, 100, 100, 100, 1000, 1000, 1000, 1000, 1000, 1000]


def gloves():
    base = define('fog')
    for id_ in (12856, 12860, 12865):
        for charge in (1000, 50, 0):
            yield case('fog gloves', f'inspect {id_} charge {charge}', base + [interact(0, 'g')], sweep=len(base) if charge == 50 else None,
                       nodes=dict(g=item(id_, 'Irit gloves', charge=charge)))
    yield case('fog gloves', 'inspect other id', base + [interact(0, 'g')], nodes=dict(g=item(12866, 'Gloves')))
    yield case('fog gloves', 'inspect npc', base + [interact(0, 'n')], nodes=dict(n=dict(kind='npc', id=12856, name='Npc')))
    for id_ in (12856, 12863):
        for charge, used in ((1000, 1), (101, 1), (2, 1), (1, 1), (5, 5), (5, 7), (0, 1), (300, -2)):
            yield case('fog gloves', f'update {id_} charge {charge} by {used}', [call('fogUpdate', charges=used)],
                       sweep=0 if (charge, used) in ((1000, 1), (1, 1)) else None,
                       answers={'equipment.get': 'g'}, nodes=dict(g=item(id_, 'Gloves', charge=charge)))
    yield case('fog gloves', 'update no gloves', [call('fogUpdate', charges=1)])
    yield case('fog gloves', 'update other gloves', [call('fogUpdate', charges=1)], answers={'equipment.get': 'g'}, nodes=dict(g=item(1059, 'Leather gloves')))
    base = define('brawling')
    names = [dict(id=13845 + k, name=n) for k, n in enumerate(['Brawling gloves (melee)', 'Brawling gloves (ranged)', 'Brawling gloves (magic)',
                                                                  'Brawling gloves (prayer)', 'Brawling gloves (agility)', 'Brawling gloves (wc)',
                                                                  'Brawling gloves (fm)', 'Brawling gloves (mining)', 'Brawling gloves (hunter)',
                                                                  'Brawling gloves (thieving)', 'Brawling gloves (smithing)',
                                                                  'Brawling gloves (fishing)', 'Brawling gloves (cooking)'])]
    login = dict(op='login', n=0)
    yield case('brawling gloves', 'login', base + [login], sweep=len(base), definitions=names)
    every = [call('bgRegister', id=13845 + k) for k in range(13)]
    yield case('brawling gloves', 'register all and save', base + [login] + every + [dict(op='save', n=0), call('bgCharges')], sweep=len(base) + 14,
               definitions=names)
    data = {'brawlingGloves': [{'gloveId': '1', 'charges': '400'}, {'gloveId': '13', 'charges': '7'}, {'gloveId': '1', 'charges': '9'}]}
    yield case('brawling gloves', 'parse and save', base + [login, dict(op='parse', n=0, data=data), dict(op='save', n=0), call('bgCharges')],
               sweep=len(base) + 1, definitions=names)
    yield case('brawling gloves', 'parse without login', base + [dict(op='parse', n=0, data=data), dict(op='save', n=0)], definitions=names)
    yield case('brawling gloves', 'parse nothing', base + [login, dict(op='parse', n=0, data={}), dict(op='save', n=0)])
    for bad in ({'brawlingGloves': [{'gloveId': '14', 'charges': '1'}]}, {'brawlingGloves': [{'gloveId': 'x', 'charges': '1'}]},
                {'brawlingGloves': [{'gloveId': '2', 'charges': '1.5'}]}, {'brawlingGloves': [{'gloveId': '2'}]},
                {'brawlingGloves': [{'gloveId': 3, 'charges': 12}]}):
        yield case('brawling gloves', f'parse {bad}', base + [login, dict(op='parse', n=0, data=bad), call('bgCharges')])
    for start, used in ((500, 1), (451, 1), (450, 1), (2, 1), (1, 1), (10, 10), (10, 11), (51, 1), (101, 1), (100, 1), (52, 2), (500, -1)):
        ops = base + [login, call('bgRegister', id=13845, charges=start), call('bgUpdate', glove=13845, charges=used), call('bgCharges')]
        yield case('brawling gloves', f'update {start} by {used}', ops, sweep=len(base) + 2 if (start, used) in ((451, 1), (1, 1)) else None,
                   definitions=names)
    yield case('brawling gloves', 'update unregistered', base + [login, call('bgUpdate', glove=13846, charges=1)])
    yield case('brawling gloves', 'register unknown', base + [login, call('bgRegister', id=995), call('bgRegister', id=13850), call('bgCharges')])
    yield case('brawling gloves', 'register twice', base + [login, call('bgRegister', id=13850, charges=3), call('bgRegister', id=13850), call('bgCharges')])
    yield case('brawling gloves', 'no manager', [call('bgRegister', id=13845), call('bgUpdate', glove=13845, charges=1), call('bgCharges'),
                                                 call('bgBonus')])
    for level in (0, 1, -1, 30):
        yield case('brawling gloves', f'bonus level {level}', base + [login, call('bgBonus')], answers={'wildernessLevel': level})
    yield case('brawling gloves', 'lookups', [call('bgForId', value=13857), call('bgForId', value=1), call('bgForIndicator', value=13),
                                               call('bgForIndicator', value=0), call('bgForSkill', value=24), call('bgForSkill', value=0),
                                               call('bgForSkill', value=3), call('bgName', glove='COOKING')], definitions=names)


# ---------------------------------------------------------------------- the enchanted gem's dialogue
MASTERS = [(8273, 'Turael'), (1598, 'Chaeldar'), (8275, 'Duradel'), (8274, 'Mazchna'), (1597, 'Vannaka'), (1, 'Nobody')]


def dialogue(n, button, component=0):
    return dict(op='dialogue', n=n, component=component, button=button)


def stage(n, value):
    return dict(op='stage', n=n, value=value)


def gem():
    base = define('gem')
    master = dict(kind='npc', id=8273, name='Turael')
    g = dict(nodes=dict(g=item(4155, 'Enchanted gem'), master=master))
    talk = base + [interact(0, 'g'), dialogue(0, 1), dialogue(0, 1)]
    yield case('enchanted gem', 'greeting and topics', talk, sweep=len(base) + 1, **g)
    for at in (100, 200, 300, 400):
        ops = talk + [stage(0, at), dialogue(0, 1), dialogue(0, 1)]
        yield case('enchanted gem', f'topic {at}', ops, sweep=len(base) + 4, **g)
    yield case('enchanted gem', 'tips then thanks', talk + [stage(0, 400), dialogue(0, 1), dialogue(0, 1), dialogue(0, 2)],
               sweep=len(base) + 6, answers={'slayerTip': ['Only one tip, and it is quite a long one that might need splitting over lines.']}, **g)
    yield case('enchanted gem', 'no task', talk + [stage(0, 100), dialogue(0, 1)], answers={'hasSlayerTask': False}, sweep=len(base) + 4, **g)
    for name, kills, flags in (('black bear', 1, 37), ('dwarf', 40, -37), ('scarab swarm', 7, 1234567),
                               ('a creature whose name is long enough to need a second line of dialogue', 99999, 16)):
        yield case('enchanted gem', f'progress {name}', talk + [stage(0, 100), dialogue(0, 1)],
                   answers={'slayerTaskName': name, 'slayerKillsRemaining': kills, 'slayerTaskFlags': flags}, **g)
    for id_, name in MASTERS:
        yield case('enchanted gem', f'master {name}', talk + [stage(0, 200), dialogue(0, 1), stage(0, 300), dialogue(0, 1)],
                   nodes=dict(g=item(4155, 'Enchanted gem'), master=dict(kind='npc', id=id_, name=name)))
    yield case('enchanted gem', 'no master', talk, answers={'slayerMaster': None}, **g)
    yield case('enchanted gem', 'unnamed master', talk + [stage(0, 200), dialogue(0, 1)],
               nodes=dict(g=item(4155, 'Enchanted gem'), master=dict(kind='npc', id=8273)))
    yield case('enchanted gem', 'username', talk, values=dict(username='MiXeD Name'), **g)
    yield case('enchanted gem', 'other stage', talk + [stage(0, 7), dialogue(0, 1), stage(0, 402), dialogue(0, 1)], **g)


# ---------------------------------------------------------------------- the pet rock's dialogue
def petrock():
    base = define('petrock')
    r = dict(nodes=dict(r=item(3695, 'Pet rock')), answers={'duration:1333': 4, 'randomLocation': [[3224, 3220, 0], [3226, 3215, 0]]})
    for button in range(0, 7):
        ops = base + [interact(0, 'r'), dialogue(0, 0), dialogue(0, button)]
        if button == 2:
            ops.append(task(1, stage=0))
        yield case('pet rock', f'option {button}', ops, sweep=len(base) + 2 if button in (1, 2, 4, 5) else None, **r)
    yield case('pet rock', 'stroke script sweep', base + [interact(0, 'r'), dialogue(0, 0), dialogue(0, 2), task(1, stage=3)],
               sweep=len(base) + 3, **r)
    yield case('pet rock', 'stroke not stopped', base + [interact(0, 'r'), dialogue(0, 0), dialogue(0, 2), task(1, stage=0)],
               answers=dict(r['answers'], stopExecuting=False), nodes=r['nodes'])
    yield case('pet rock', 'after the end', base + [interact(0, 'r'), dialogue(0, 0), dialogue(0, 1), dialogue(0, 1)], **r)
    yield case('pet rock', 'no random location', base + [interact(0, 'r'), dialogue(0, 0), dialogue(0, 4)],
               answers={'randomLocation': None}, nodes=r['nodes'])


# ---------------------------------------------------------------------- Diango and the snow globe
HOLIDAY = [4079, 10507, 10508, 4566, 6722, 6857, 6856, 6858, 6859, 6860, 6861, 6862, 6863, 9920, 9921, 9922, 9923, 9924, 9925, 11019,
           11020, 11021, 11022, 11789, 11949, 12645, 14076, 14077, 14081, 14595, 14602, 14603, 14605, 14654, 14596, 6867, 6866, 6865]
HOLIDAY_DEFS = [dict(id=i, name=f'Toy {i}', examine=f'A holiday item ({i}).') for i in HOLIDAY]


def diango():
    via = '970:option:holiday-items'
    both = define('diango', 'reclaim')
    d = dict(definitions=HOLIDAY_DEFS, nodes=dict(n=dict(kind='npc', id=970, name='Diango')))
    yield case('diango', 'open', both + [dict(op='npcHandler', via=via, node='n', option='holiday-items')], sweep=len(both), **d)
    yield case('diango', 'open without reclaim', define('diango') + [dict(op='npcHandler', via=via, node='n', option='holiday-items')], **d)
    owned = {'inventory.containsItem': [False, True], 'equipment.containsItem': [False, False, True], 'bank.containsItem': [False] * 5 + [True],
             'storageContains:6857': True, 'opened': 134}
    yield case('diango', 'open owned', both + [dict(op='npcHandler', via=via, node='n', option='holiday-items')], answers=owned, **d)
    for attrs, answers in (({'sotr:purchased': True}, {}), ({'sotr:purchased': True}, {'inInventory': True}),
                           ({'sotr:purchased': True}, {'inEquipment': True}), ({'sotr:purchased': False}, {})):
        yield case('diango', f'staff of the raven {attrs} {answers}', both + [dict(op='npcHandler', via=via, node='n', option='holiday-items')],
                   attributes=dict(p=attrs), answers=answers, **d)
    take = dict(op='component', id=468, opcode=155, button=2, slot=3, item=0)
    examine = dict(op='component', id=468, opcode=196, button=2, slot=0, item=0)
    opened = both + [dict(op='npcHandler', via=via, node='n', option='holiday-items')]
    yield case('diango', 'take', opened + [take], sweep=len(both) + 1, **d)
    yield case('diango', 'examine', opened + [examine], sweep=len(both) + 1, **d)
    yield case('diango', 'other opcode', opened + [dict(take, opcode=9)], **d)
    yield case('diango', 'slot past the end', opened + [dict(take, slot=40)], **d)
    yield case('diango', 'negative slot', opened + [dict(take, slot=-1)], **d)
    yield case('diango', 'take without opening', both + [take], **d)
    yield case('diango', 'reclaim twice', define('diango', 'reclaim', 'reclaim') + [dict(op='npcHandler', via=via, node='n', option='holiday-items'),
                                                                                    dict(take, slot=40)], **d)
    yield case('diango', 'nothing to reclaim', both + [dict(op='npcHandler', via=via, node='n', option='holiday-items')],
               answers={'inventory.containsItem': True}, **d)
    base = define('snowglobe')
    yield case('snow globe', 'continue', base + [dict(op='component', id=659, opcode=155, button=2, slot=0, item=0), task(0)], sweep=len(base),
               answers={'inventory.freeSlots': 17})
    for button in (1, 3, 0):
        yield case('snow globe', f'button {button}', base + [dict(op='component', id=659, opcode=155, button=button, slot=0, item=0)],
                   sweep=len(base) if button == 1 else None)
    yield case('snow globe', 'pulse sweep', base + [dict(op='component', id=659, opcode=155, button=2, slot=0, item=0), task(0)], sweep=len(base) + 1)


# ---------------------------------------------------------------------- the generators themselves
def generators():
    ops = [call('tlrInt', bound=b) for b in (1, 2, 3, 4, 6, 8, 16, 1 << 30, 1000000007, 2147483647)]
    ops += [call('tlrDouble', max=m) for m in (55.0, 1.0, 0.001, 1e9)]
    ops += [call('random', bound=b) for b in (1, 2, 7, 8, 64, 0, -5, 1 << 30, 1000000007)]
    ops += [call('randomRange', a=a, b=b) for a, b in ((0, 1), (0, 8), (20, 640), (640, 20), (5, 5), (-3, 3), (0, -2147483648))]
    for seed in range(6):
        yield case('generators', f'draws seed {seed}', ops, seeds=dict(random=seed * 7, shuffle=seed, tlr=seed * 0x9E3779B97F4A7C15 % (1 << 62)))


def cases():
    yield from registration()
    yield from empty()
    yield from light()
    yield from spade()
    yield from casket()
    yield from nests()
    yield from lamps()
    yield from packs()
    yield from herbbox()
    yield from simple()
    yield from equips()
    yield from morph()
    yield from toys()
    yield from water()
    yield from gloves()
    yield from gem()
    yield from petrock()
    yield from diango()
    yield from generators()


def injects(row):
    return any(any(a.get('op') == 'fail' for a in h.get('actions', [])) for h in row.get('hooks', []))


def groups(cases_):
    out = {}
    for case_ in cases_:
        out[case_['group']] = out.get(case_['group'], 0) + 1
    return dict(sorted(out.items()))
